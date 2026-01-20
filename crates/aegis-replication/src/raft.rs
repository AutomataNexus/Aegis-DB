//! Aegis Raft Consensus
//!
//! Core Raft consensus algorithm implementation.
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use crate::log::{LogEntry, LogIndex, ReplicatedLog, Term};
use crate::node::{NodeId, NodeRole};
use crate::state::{Command, CommandResult, StateMachine};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

// =============================================================================
// Raft Configuration
// =============================================================================

/// Configuration for the Raft consensus algorithm.
#[derive(Debug, Clone)]
pub struct RaftConfig {
    pub election_timeout_min: Duration,
    pub election_timeout_max: Duration,
    pub heartbeat_interval: Duration,
    pub max_entries_per_request: usize,
    pub snapshot_threshold: u64,
}

impl Default for RaftConfig {
    fn default() -> Self {
        Self {
            election_timeout_min: Duration::from_millis(150),
            election_timeout_max: Duration::from_millis(300),
            heartbeat_interval: Duration::from_millis(50),
            max_entries_per_request: 100,
            snapshot_threshold: 10000,
        }
    }
}

impl RaftConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_election_timeout(mut self, min: Duration, max: Duration) -> Self {
        self.election_timeout_min = min;
        self.election_timeout_max = max;
        self
    }

    pub fn with_heartbeat_interval(mut self, interval: Duration) -> Self {
        self.heartbeat_interval = interval;
        self
    }
}

// =============================================================================
// Raft State
// =============================================================================

/// Persistent state for Raft consensus.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RaftState {
    pub current_term: Term,
    pub voted_for: Option<NodeId>,
    pub commit_index: LogIndex,
    pub last_applied: LogIndex,
}

impl Default for RaftState {
    fn default() -> Self {
        Self {
            current_term: 0,
            voted_for: None,
            commit_index: 0,
            last_applied: 0,
        }
    }
}

// =============================================================================
// Vote Request/Response
// =============================================================================

/// Request for a vote during leader election.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoteRequest {
    pub term: Term,
    pub candidate_id: NodeId,
    pub last_log_index: LogIndex,
    pub last_log_term: Term,
}

/// Response to a vote request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoteResponse {
    pub term: Term,
    pub vote_granted: bool,
    pub voter_id: NodeId,
}

// =============================================================================
// Append Entries Request/Response
// =============================================================================

/// Request to append entries to the log.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppendEntriesRequest {
    pub term: Term,
    pub leader_id: NodeId,
    pub prev_log_index: LogIndex,
    pub prev_log_term: Term,
    pub entries: Vec<LogEntry>,
    pub leader_commit: LogIndex,
}

/// Response to an append entries request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppendEntriesResponse {
    pub term: Term,
    pub success: bool,
    pub match_index: LogIndex,
    pub conflict_index: Option<LogIndex>,
    pub conflict_term: Option<Term>,
}

// =============================================================================
// Install Snapshot Request/Response
// =============================================================================

/// Request to install a snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallSnapshotRequest {
    pub term: Term,
    pub leader_id: NodeId,
    pub last_included_index: LogIndex,
    pub last_included_term: Term,
    pub offset: u64,
    pub data: Vec<u8>,
    pub done: bool,
}

/// Response to an install snapshot request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallSnapshotResponse {
    pub term: Term,
}

// =============================================================================
// Raft Node
// =============================================================================

/// A node in the Raft cluster.
pub struct RaftNode {
    id: NodeId,
    config: RaftConfig,
    state: RwLock<RaftState>,
    role: RwLock<NodeRole>,
    log: Arc<ReplicatedLog>,
    state_machine: Arc<StateMachine>,
    peers: RwLock<HashSet<NodeId>>,
    leader_id: RwLock<Option<NodeId>>,
    next_index: RwLock<HashMap<NodeId, LogIndex>>,
    match_index: RwLock<HashMap<NodeId, LogIndex>>,
    last_heartbeat: RwLock<Instant>,
    votes_received: RwLock<HashSet<NodeId>>,
}

impl RaftNode {
    /// Create a new Raft node.
    pub fn new(id: impl Into<NodeId>, config: RaftConfig) -> Self {
        Self {
            id: id.into(),
            config,
            state: RwLock::new(RaftState::default()),
            role: RwLock::new(NodeRole::Follower),
            log: Arc::new(ReplicatedLog::new()),
            state_machine: Arc::new(StateMachine::new()),
            peers: RwLock::new(HashSet::new()),
            leader_id: RwLock::new(None),
            next_index: RwLock::new(HashMap::new()),
            match_index: RwLock::new(HashMap::new()),
            last_heartbeat: RwLock::new(Instant::now()),
            votes_received: RwLock::new(HashSet::new()),
        }
    }

    /// Get the node ID.
    pub fn id(&self) -> NodeId {
        self.id.clone()
    }

    /// Get the current role.
    pub fn role(&self) -> NodeRole {
        *self.role.read().unwrap()
    }

    /// Get the current term.
    pub fn current_term(&self) -> Term {
        self.state.read().unwrap().current_term
    }

    /// Get the current leader ID.
    pub fn leader_id(&self) -> Option<NodeId> {
        self.leader_id.read().unwrap().clone()
    }

    /// Check if this node is the leader.
    pub fn is_leader(&self) -> bool {
        self.role() == NodeRole::Leader
    }

    /// Add a peer to the cluster.
    pub fn add_peer(&self, peer_id: NodeId) {
        let mut peers = self.peers.write().unwrap();
        peers.insert(peer_id.clone());

        let last_log = self.log.last_index();
        self.next_index.write().unwrap().insert(peer_id.clone(), last_log + 1);
        self.match_index.write().unwrap().insert(peer_id, 0);
    }

    /// Remove a peer from the cluster.
    pub fn remove_peer(&self, peer_id: &NodeId) {
        let mut peers = self.peers.write().unwrap();
        peers.remove(peer_id);
        self.next_index.write().unwrap().remove(peer_id);
        self.match_index.write().unwrap().remove(peer_id);
    }

    /// Get the list of peers.
    pub fn peers(&self) -> Vec<NodeId> {
        self.peers.read().unwrap().iter().cloned().collect()
    }

    /// Get the cluster size (including self).
    pub fn cluster_size(&self) -> usize {
        self.peers.read().unwrap().len() + 1
    }

    /// Get the quorum size.
    pub fn quorum_size(&self) -> usize {
        (self.cluster_size() / 2) + 1
    }

    /// Reset the heartbeat timer.
    pub fn reset_heartbeat(&self) {
        *self.last_heartbeat.write().unwrap() = Instant::now();
    }

    /// Check if the election timeout has elapsed.
    pub fn election_timeout_elapsed(&self) -> bool {
        let elapsed = self.last_heartbeat.read().unwrap().elapsed();
        elapsed >= self.config.election_timeout_min
    }

    // =========================================================================
    // Leader Election
    // =========================================================================

    /// Start an election as a candidate.
    pub fn start_election(&self) -> VoteRequest {
        let mut state = self.state.write().unwrap();
        state.current_term += 1;
        state.voted_for = Some(self.id.clone());

        *self.role.write().unwrap() = NodeRole::Candidate;
        *self.leader_id.write().unwrap() = None;

        let mut votes = self.votes_received.write().unwrap();
        votes.clear();
        votes.insert(self.id.clone());

        self.reset_heartbeat();

        VoteRequest {
            term: state.current_term,
            candidate_id: self.id.clone(),
            last_log_index: self.log.last_index(),
            last_log_term: self.log.last_term(),
        }
    }

    /// Handle a vote request.
    pub fn handle_vote_request(&self, request: &VoteRequest) -> VoteResponse {
        let mut state = self.state.write().unwrap();

        if request.term > state.current_term {
            state.current_term = request.term;
            state.voted_for = None;
            *self.role.write().unwrap() = NodeRole::Follower;
            *self.leader_id.write().unwrap() = None;
        }

        let vote_granted = request.term >= state.current_term
            && (state.voted_for.is_none() || state.voted_for.as_ref() == Some(&request.candidate_id))
            && self.log.is_up_to_date(request.last_log_index, request.last_log_term);

        if vote_granted {
            state.voted_for = Some(request.candidate_id.clone());
            self.reset_heartbeat();
        }

        VoteResponse {
            term: state.current_term,
            vote_granted,
            voter_id: self.id.clone(),
        }
    }

    /// Handle a vote response.
    pub fn handle_vote_response(&self, response: &VoteResponse) -> bool {
        let current_term = {
            let mut state = self.state.write().unwrap();

            if response.term > state.current_term {
                state.current_term = response.term;
                state.voted_for = None;
                *self.role.write().unwrap() = NodeRole::Follower;
                *self.leader_id.write().unwrap() = None;
                return false;
            }

            if self.role() != NodeRole::Candidate || response.term != state.current_term {
                return false;
            }

            state.current_term
        };

        if response.vote_granted {
            self.votes_received.write().unwrap().insert(response.voter_id.clone());
        }

        let votes = self.votes_received.read().unwrap().len();
        if votes >= self.quorum_size() {
            self.become_leader_with_term(current_term);
            return true;
        }

        false
    }

    /// Become the leader.
    fn become_leader(&self) {
        let term = self.current_term();
        self.become_leader_with_term(term);
    }

    /// Become the leader with a specific term (avoids deadlock when called with state lock held).
    fn become_leader_with_term(&self, term: Term) {
        *self.role.write().unwrap() = NodeRole::Leader;
        *self.leader_id.write().unwrap() = Some(self.id.clone());

        let last_log = self.log.last_index();
        let peers: Vec<_> = self.peers.read().unwrap().iter().cloned().collect();

        let mut next_index = self.next_index.write().unwrap();
        let mut match_index = self.match_index.write().unwrap();

        for peer in peers {
            next_index.insert(peer.clone(), last_log + 1);
            match_index.insert(peer, 0);
        }

        drop(next_index);
        drop(match_index);

        let noop = LogEntry::noop(last_log + 1, term);
        self.log.append(noop);
    }

    // =========================================================================
    // Log Replication
    // =========================================================================

    /// Propose a command (leader only).
    pub fn propose(&self, command: Command) -> Result<LogIndex, String> {
        if !self.is_leader() {
            return Err("Not the leader".to_string());
        }

        let term = self.current_term();
        let index = self.log.last_index() + 1;
        let entry = LogEntry::command(index, term, command.to_bytes());

        self.log.append(entry);
        Ok(index)
    }

    /// Create an append entries request for a peer.
    pub fn create_append_entries(&self, peer_id: &NodeId) -> Option<AppendEntriesRequest> {
        if !self.is_leader() {
            return None;
        }

        let next_index = *self.next_index.read().unwrap().get(peer_id)?;
        let prev_log_index = next_index.saturating_sub(1);
        let prev_log_term = self.log.term_at(prev_log_index).unwrap_or(0);

        let entries = self.log.get_range(next_index, self.log.last_index() + 1);
        let entries: Vec<_> = entries
            .into_iter()
            .take(self.config.max_entries_per_request)
            .collect();

        let state = self.state.read().unwrap();

        Some(AppendEntriesRequest {
            term: state.current_term,
            leader_id: self.id.clone(),
            prev_log_index,
            prev_log_term,
            entries,
            leader_commit: state.commit_index,
        })
    }

    /// Handle an append entries request.
    pub fn handle_append_entries(&self, request: &AppendEntriesRequest) -> AppendEntriesResponse {
        let mut state = self.state.write().unwrap();

        if request.term < state.current_term {
            return AppendEntriesResponse {
                term: state.current_term,
                success: false,
                match_index: 0,
                conflict_index: None,
                conflict_term: None,
            };
        }

        if request.term > state.current_term {
            state.current_term = request.term;
            state.voted_for = None;
        }

        *self.role.write().unwrap() = NodeRole::Follower;
        *self.leader_id.write().unwrap() = Some(request.leader_id.clone());
        self.reset_heartbeat();

        if request.prev_log_index > 0 {
            match self.log.term_at(request.prev_log_index) {
                None => {
                    return AppendEntriesResponse {
                        term: state.current_term,
                        success: false,
                        match_index: self.log.last_index(),
                        conflict_index: Some(self.log.last_index() + 1),
                        conflict_term: None,
                    };
                }
                Some(term) if term != request.prev_log_term => {
                    let conflict_index = self.find_first_index_of_term(term);
                    return AppendEntriesResponse {
                        term: state.current_term,
                        success: false,
                        match_index: 0,
                        conflict_index: Some(conflict_index),
                        conflict_term: Some(term),
                    };
                }
                _ => {}
            }
        }

        if !request.entries.is_empty() {
            if let Some(conflict) = self.log.find_conflict(&request.entries) {
                self.log.truncate_from(conflict);
            }

            let existing_last = self.log.last_index();
            let new_entries: Vec<_> = request
                .entries
                .iter()
                .filter(|e| e.index > existing_last)
                .cloned()
                .collect();

            if !new_entries.is_empty() {
                self.log.append_entries(new_entries);
            }
        }

        if request.leader_commit > state.commit_index {
            let last_new_index = if request.entries.is_empty() {
                request.prev_log_index
            } else {
                request.entries.last().unwrap().index
            };
            state.commit_index = std::cmp::min(request.leader_commit, last_new_index);
            self.log.set_commit_index(state.commit_index);
        }

        AppendEntriesResponse {
            term: state.current_term,
            success: true,
            match_index: self.log.last_index(),
            conflict_index: None,
            conflict_term: None,
        }
    }

    /// Handle an append entries response.
    pub fn handle_append_entries_response(
        &self,
        peer_id: &NodeId,
        response: &AppendEntriesResponse,
    ) {
        let mut state = self.state.write().unwrap();

        if response.term > state.current_term {
            state.current_term = response.term;
            state.voted_for = None;
            *self.role.write().unwrap() = NodeRole::Follower;
            *self.leader_id.write().unwrap() = None;
            return;
        }

        if !self.is_leader() {
            return;
        }

        let mut next_index = self.next_index.write().unwrap();
        let mut match_index = self.match_index.write().unwrap();

        if response.success {
            match_index.insert(peer_id.clone(), response.match_index);
            next_index.insert(peer_id.clone(), response.match_index + 1);
            drop(next_index);
            drop(match_index);
            drop(state);
            self.try_advance_commit_index();
        } else {
            if let Some(conflict_index) = response.conflict_index {
                next_index.insert(peer_id.clone(), conflict_index);
            } else {
                let current = *next_index.get(peer_id).unwrap_or(&1);
                next_index.insert(peer_id.clone(), current.saturating_sub(1).max(1));
            }
        }
    }

    /// Try to advance the commit index based on match indices.
    fn try_advance_commit_index(&self) {
        let match_indices: Vec<_> = {
            let match_index = self.match_index.read().unwrap();
            let mut indices: Vec<_> = match_index.values().copied().collect();
            indices.push(self.log.last_index());
            indices.sort_unstable();
            indices
        };

        let quorum_index = match_indices.len() / 2;
        let new_commit = match_indices[quorum_index];

        let mut state = self.state.write().unwrap();
        if new_commit > state.commit_index {
            if let Some(term) = self.log.term_at(new_commit) {
                if term == state.current_term {
                    state.commit_index = new_commit;
                    self.log.set_commit_index(new_commit);
                }
            }
        }
    }

    fn find_first_index_of_term(&self, term: Term) -> LogIndex {
        let mut index = self.log.last_index();
        while index > 0 {
            if let Some(t) = self.log.term_at(index) {
                if t != term {
                    return index + 1;
                }
            }
            index -= 1;
        }
        1
    }

    // =========================================================================
    // State Machine Application
    // =========================================================================

    /// Apply committed entries to the state machine.
    pub fn apply_committed(&self) -> Vec<CommandResult> {
        let mut results = Vec::new();

        while self.log.has_entries_to_apply() {
            if let Some(entry) = self.log.next_to_apply() {
                if let Some(command) = Command::from_bytes(&entry.data) {
                    let result = self.state_machine.apply(&command, entry.index);
                    results.push(result);
                }
                self.log.set_last_applied(entry.index);

                let mut state = self.state.write().unwrap();
                state.last_applied = entry.index;
            }
        }

        results
    }

    /// Get a value from the state machine.
    pub fn get(&self, key: &str) -> Option<Vec<u8>> {
        self.state_machine.get(key)
    }

    /// Get the log.
    pub fn log(&self) -> &ReplicatedLog {
        &self.log
    }

    /// Get the state machine.
    pub fn state_machine(&self) -> &StateMachine {
        &self.state_machine
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_raft_config() {
        let config = RaftConfig::default();
        assert_eq!(config.election_timeout_min, Duration::from_millis(150));
        assert_eq!(config.heartbeat_interval, Duration::from_millis(50));
    }

    #[test]
    fn test_raft_node_creation() {
        let node = RaftNode::new("node1", RaftConfig::default());
        assert_eq!(node.id().as_str(), "node1");
        assert_eq!(node.role(), NodeRole::Follower);
        assert_eq!(node.current_term(), 0);
        assert!(!node.is_leader());
    }

    #[test]
    fn test_add_peer() {
        let node = RaftNode::new("node1", RaftConfig::default());
        node.add_peer(NodeId::new("node2"));
        node.add_peer(NodeId::new("node3"));

        assert_eq!(node.cluster_size(), 3);
        assert_eq!(node.quorum_size(), 2);
        assert_eq!(node.peers().len(), 2);
    }

    #[test]
    fn test_start_election() {
        let node = RaftNode::new("node1", RaftConfig::default());
        let request = node.start_election();

        assert_eq!(request.term, 1);
        assert_eq!(request.candidate_id.as_str(), "node1");
        assert_eq!(node.role(), NodeRole::Candidate);
        assert_eq!(node.current_term(), 1);
    }

    #[test]
    fn test_vote_request_handling() {
        let node1 = RaftNode::new("node1", RaftConfig::default());
        let node2 = RaftNode::new("node2", RaftConfig::default());

        let request = node1.start_election();
        let response = node2.handle_vote_request(&request);

        assert!(response.vote_granted);
        assert_eq!(response.term, 1);
    }

    #[test]
    fn test_become_leader() {
        let node = RaftNode::new("node1", RaftConfig::default());
        node.add_peer(NodeId::new("node2"));

        let request = node.start_election();
        let response = VoteResponse {
            term: request.term,
            vote_granted: true,
            voter_id: NodeId::new("node2"),
        };

        let became_leader = node.handle_vote_response(&response);
        assert!(became_leader);
        assert!(node.is_leader());
        assert_eq!(node.leader_id(), Some(NodeId::new("node1")));
    }

    #[test]
    fn test_propose_command() {
        let node = RaftNode::new("node1", RaftConfig::default());
        node.add_peer(NodeId::new("node2"));

        node.start_election();
        let response = VoteResponse {
            term: 1,
            vote_granted: true,
            voter_id: NodeId::new("node2"),
        };
        node.handle_vote_response(&response);

        let command = Command::set("key1", b"value1".to_vec());
        let result = node.propose(command);
        assert!(result.is_ok());
    }

    #[test]
    fn test_append_entries() {
        let leader = RaftNode::new("leader", RaftConfig::default());
        let follower = RaftNode::new("follower", RaftConfig::default());

        leader.add_peer(NodeId::new("follower"));
        leader.start_election();
        let vote = VoteResponse {
            term: 1,
            vote_granted: true,
            voter_id: NodeId::new("follower"),
        };
        leader.handle_vote_response(&vote);

        let command = Command::set("key", b"value".to_vec());
        leader.propose(command).unwrap();

        let request = leader.create_append_entries(&NodeId::new("follower")).unwrap();
        let response = follower.handle_append_entries(&request);

        assert!(response.success);
        assert_eq!(follower.log().last_index(), leader.log().last_index());
    }

    #[test]
    fn test_follower_rejects_old_term() {
        let follower = RaftNode::new("follower", RaftConfig::default());

        {
            let mut state = follower.state.write().unwrap();
            state.current_term = 5;
        }

        let request = AppendEntriesRequest {
            term: 3,
            leader_id: NodeId::new("old_leader"),
            prev_log_index: 0,
            prev_log_term: 0,
            entries: vec![],
            leader_commit: 0,
        };

        let response = follower.handle_append_entries(&request);
        assert!(!response.success);
        assert_eq!(response.term, 5);
    }
}
