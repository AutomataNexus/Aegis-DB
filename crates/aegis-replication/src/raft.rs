//! Aegis Raft Consensus
//!
//! Core Raft consensus algorithm implementation.
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use crate::log::{LogEntry, LogIndex, ReplicatedLog, Term};
use crate::node::{NodeId, NodeRole};
use crate::state::{Command, CommandResult, Snapshot, StateMachine, StateMachineBackend};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

/// Default chunk size for snapshot transfers (64KB).
const SNAPSHOT_CHUNK_SIZE: usize = 64 * 1024;

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
    /// Duration for which a leader lease is valid after receiving majority acks.
    /// Should be slightly longer than heartbeat_interval to avoid flapping.
    pub lease_duration: Duration,
}

impl Default for RaftConfig {
    fn default() -> Self {
        Self {
            election_timeout_min: Duration::from_millis(150),
            election_timeout_max: Duration::from_millis(300),
            heartbeat_interval: Duration::from_millis(50),
            max_entries_per_request: 100,
            snapshot_threshold: 10000,
            lease_duration: Duration::from_millis(200),
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
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RaftState {
    pub current_term: Term,
    pub voted_for: Option<NodeId>,
    pub commit_index: LogIndex,
    pub last_applied: LogIndex,
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
    /// CRC32 checksum of the full snapshot data (set when done=true).
    /// None for backward compatibility with older nodes.
    #[serde(default)]
    pub checksum: Option<u32>,
}

/// Response to an install snapshot request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallSnapshotResponse {
    pub term: Term,
}

// =============================================================================
// Raft Node
// =============================================================================

/// Snapshot metadata for tracking snapshot state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotMetadata {
    pub last_included_index: LogIndex,
    pub last_included_term: Term,
    pub size: u64,
}

/// In-progress snapshot being received from leader.
struct PendingSnapshot {
    metadata: SnapshotMetadata,
    data: Vec<u8>,
    offset: u64,
}

/// A node in the Raft cluster.
/// Supports pluggable state machine backends for flexible storage integration.
pub struct RaftNode {
    id: NodeId,
    config: RaftConfig,
    state: RwLock<RaftState>,
    role: RwLock<NodeRole>,
    log: Arc<ReplicatedLog>,
    /// The replicated state machine - can be any implementation of StateMachineBackend.
    state_machine: Arc<dyn StateMachineBackend>,
    peers: RwLock<HashSet<NodeId>>,
    leader_id: RwLock<Option<NodeId>>,
    next_index: RwLock<HashMap<NodeId, LogIndex>>,
    match_index: RwLock<HashMap<NodeId, LogIndex>>,
    last_heartbeat: RwLock<Instant>,
    votes_received: RwLock<HashSet<NodeId>>,
    /// Current snapshot metadata (if a snapshot exists)
    snapshot_metadata: RwLock<Option<SnapshotMetadata>>,
    /// In-progress snapshot being received
    pending_snapshot: RwLock<Option<PendingSnapshot>>,
    /// Leader lease expiry time. Only meaningful when this node is the leader.
    /// If the lease has expired, the leader should step down.
    lease_expiry: RwLock<Option<Instant>>,
}

impl RaftNode {
    /// Create a new Raft node with the default in-memory state machine.
    pub fn new(id: impl Into<NodeId>, config: RaftConfig) -> Self {
        Self::with_state_machine(id, config, Arc::new(StateMachine::new()))
    }

    /// Create a new Raft node with a custom state machine backend.
    /// Use this to integrate Raft with your storage layer.
    pub fn with_state_machine(
        id: impl Into<NodeId>,
        config: RaftConfig,
        state_machine: Arc<dyn StateMachineBackend>,
    ) -> Self {
        Self {
            id: id.into(),
            config,
            state: RwLock::new(RaftState::default()),
            role: RwLock::new(NodeRole::Follower),
            log: Arc::new(ReplicatedLog::new()),
            state_machine,
            peers: RwLock::new(HashSet::new()),
            leader_id: RwLock::new(None),
            next_index: RwLock::new(HashMap::new()),
            match_index: RwLock::new(HashMap::new()),
            last_heartbeat: RwLock::new(Instant::now()),
            votes_received: RwLock::new(HashSet::new()),
            snapshot_metadata: RwLock::new(None),
            pending_snapshot: RwLock::new(None),
            lease_expiry: RwLock::new(None),
        }
    }

    /// Get the node ID.
    pub fn id(&self) -> NodeId {
        self.id.clone()
    }

    /// Get the current role.
    pub fn role(&self) -> NodeRole {
        *self.role.read().expect("raft role lock poisoned")
    }

    /// Get the current term.
    pub fn current_term(&self) -> Term {
        self.state
            .read()
            .expect("raft state lock poisoned")
            .current_term
    }

    /// Get the current leader ID.
    pub fn leader_id(&self) -> Option<NodeId> {
        self.leader_id
            .read()
            .expect("raft leader_id lock poisoned")
            .clone()
    }

    /// Check if this node is the leader.
    pub fn is_leader(&self) -> bool {
        self.role() == NodeRole::Leader
    }

    /// Check if this node is the leader and holds a valid lease.
    /// Unlike `is_leader()`, this also verifies that the leader has recently
    /// received acknowledgements from a majority, preventing split-brain.
    pub fn is_leader_with_lease(&self) -> bool {
        self.is_leader() && self.has_valid_lease()
    }

    /// Extend the leader lease to now + lease_duration.
    /// Called when the leader receives successful acks from a majority.
    pub fn extend_lease(&self) {
        let expiry = Instant::now() + self.config.lease_duration;
        *self
            .lease_expiry
            .write()
            .expect("raft lease_expiry lock poisoned") = Some(expiry);
    }

    /// Check if the leader lease is still valid.
    pub fn has_valid_lease(&self) -> bool {
        match *self
            .lease_expiry
            .read()
            .expect("raft lease_expiry lock poisoned")
        {
            Some(expiry) => Instant::now() < expiry,
            None => false,
        }
    }

    /// Periodic check: if this node is leader but the lease has expired,
    /// step down to follower to prevent split-brain.
    pub fn check_lease(&self) {
        if self.is_leader() && !self.has_valid_lease() {
            *self.role.write().expect("raft role lock poisoned") = NodeRole::Follower;
            *self
                .leader_id
                .write()
                .expect("raft leader_id lock poisoned") = None;
            // Clear the lease
            *self
                .lease_expiry
                .write()
                .expect("raft lease_expiry lock poisoned") = None;
        }
    }

    /// Add a peer to the cluster.
    pub fn add_peer(&self, peer_id: NodeId) {
        let mut peers = self.peers.write().expect("raft peers lock poisoned");
        peers.insert(peer_id.clone());

        let last_log = self.log.last_index();
        self.next_index
            .write()
            .expect("raft next_index lock poisoned")
            .insert(peer_id.clone(), last_log + 1);
        self.match_index
            .write()
            .expect("raft match_index lock poisoned")
            .insert(peer_id, 0);
    }

    /// Remove a peer from the cluster.
    pub fn remove_peer(&self, peer_id: &NodeId) {
        let mut peers = self.peers.write().expect("raft peers lock poisoned");
        peers.remove(peer_id);
        self.next_index
            .write()
            .expect("raft next_index lock poisoned")
            .remove(peer_id);
        self.match_index
            .write()
            .expect("raft match_index lock poisoned")
            .remove(peer_id);
    }

    /// Get the list of peers.
    pub fn peers(&self) -> Vec<NodeId> {
        self.peers
            .read()
            .expect("raft peers lock poisoned")
            .iter()
            .cloned()
            .collect()
    }

    /// Get the cluster size (including self).
    pub fn cluster_size(&self) -> usize {
        self.peers.read().expect("raft peers lock poisoned").len() + 1
    }

    /// Get the quorum size.
    pub fn quorum_size(&self) -> usize {
        (self.cluster_size() / 2) + 1
    }

    /// Reset the heartbeat timer.
    pub fn reset_heartbeat(&self) {
        *self
            .last_heartbeat
            .write()
            .expect("raft last_heartbeat lock poisoned") = Instant::now();
    }

    /// Check if the election timeout has elapsed.
    pub fn election_timeout_elapsed(&self) -> bool {
        let elapsed = self
            .last_heartbeat
            .read()
            .expect("raft last_heartbeat lock poisoned")
            .elapsed();
        elapsed >= self.config.election_timeout_min
    }

    // =========================================================================
    // Leader Election
    // =========================================================================

    /// Start an election as a candidate.
    pub fn start_election(&self) -> VoteRequest {
        let mut state = self.state.write().expect("raft state lock poisoned");
        state.current_term += 1;
        state.voted_for = Some(self.id.clone());

        *self.role.write().expect("raft role lock poisoned") = NodeRole::Candidate;
        *self
            .leader_id
            .write()
            .expect("raft leader_id lock poisoned") = None;

        let mut votes = self
            .votes_received
            .write()
            .expect("raft votes_received lock poisoned");
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
        let mut state = self.state.write().expect("raft state lock poisoned");

        if request.term > state.current_term {
            state.current_term = request.term;
            state.voted_for = None;
            *self.role.write().expect("raft role lock poisoned") = NodeRole::Follower;
            *self
                .leader_id
                .write()
                .expect("raft leader_id lock poisoned") = None;
        }

        let vote_granted = request.term >= state.current_term
            && (state.voted_for.is_none()
                || state.voted_for.as_ref() == Some(&request.candidate_id))
            && self
                .log
                .is_up_to_date(request.last_log_index, request.last_log_term);

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
            let mut state = self.state.write().expect("raft state lock poisoned");

            if response.term > state.current_term {
                state.current_term = response.term;
                state.voted_for = None;
                *self.role.write().expect("raft role lock poisoned") = NodeRole::Follower;
                *self
                    .leader_id
                    .write()
                    .expect("raft leader_id lock poisoned") = None;
                return false;
            }

            if self.role() != NodeRole::Candidate || response.term != state.current_term {
                return false;
            }

            state.current_term
        };

        if response.vote_granted {
            self.votes_received
                .write()
                .expect("raft votes_received lock poisoned")
                .insert(response.voter_id.clone());
        }

        let votes = self
            .votes_received
            .read()
            .expect("raft votes_received lock poisoned")
            .len();
        if votes >= self.quorum_size() {
            self.become_leader_with_term(current_term);
            return true;
        }

        false
    }

    /// Become the leader.
    #[allow(dead_code)]
    fn become_leader(&self) {
        let term = self.current_term();
        self.become_leader_with_term(term);
    }

    /// Become the leader with a specific term (avoids deadlock when called with state lock held).
    fn become_leader_with_term(&self, term: Term) {
        *self.role.write().expect("raft role lock poisoned") = NodeRole::Leader;
        *self
            .leader_id
            .write()
            .expect("raft leader_id lock poisoned") = Some(self.id.clone());

        let last_log = self.log.last_index();
        let peers: Vec<_> = self
            .peers
            .read()
            .expect("raft peers lock poisoned")
            .iter()
            .cloned()
            .collect();

        let mut next_index = self
            .next_index
            .write()
            .expect("raft next_index lock poisoned");
        let mut match_index = self
            .match_index
            .write()
            .expect("raft match_index lock poisoned");

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

        let next_index = *self
            .next_index
            .read()
            .expect("raft next_index lock poisoned")
            .get(peer_id)?;
        let prev_log_index = next_index.saturating_sub(1);
        let prev_log_term = self.log.term_at(prev_log_index).unwrap_or(0);

        let entries = self.log.get_range(next_index, self.log.last_index() + 1);
        let entries: Vec<_> = entries
            .into_iter()
            .take(self.config.max_entries_per_request)
            .collect();

        let state = self.state.read().expect("raft state lock poisoned");

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
        let mut state = self.state.write().expect("raft state lock poisoned");

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

        *self.role.write().expect("raft role lock poisoned") = NodeRole::Follower;
        *self
            .leader_id
            .write()
            .expect("raft leader_id lock poisoned") = Some(request.leader_id.clone());
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
                request
                    .entries
                    .last()
                    .expect("entries confirmed non-empty")
                    .index
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
        let mut state = self.state.write().expect("raft state lock poisoned");

        if response.term > state.current_term {
            state.current_term = response.term;
            state.voted_for = None;
            *self.role.write().expect("raft role lock poisoned") = NodeRole::Follower;
            *self
                .leader_id
                .write()
                .expect("raft leader_id lock poisoned") = None;
            return;
        }

        if !self.is_leader() {
            return;
        }

        let mut next_index = self
            .next_index
            .write()
            .expect("raft next_index lock poisoned");
        let mut match_index = self
            .match_index
            .write()
            .expect("raft match_index lock poisoned");

        if response.success {
            match_index.insert(peer_id.clone(), response.match_index);
            next_index.insert(peer_id.clone(), response.match_index + 1);

            // Check if we have acks from a majority to extend the lease.
            // Count successful peers (match_index > 0 means they've acked at least once).
            let ack_count = match_index.values().filter(|&&idx| idx > 0).count() + 1; // +1 for self
            let quorum = self.quorum_size();

            drop(next_index);
            drop(match_index);
            drop(state);

            if ack_count >= quorum {
                self.extend_lease();
            }

            self.try_advance_commit_index();
        } else if let Some(conflict_index) = response.conflict_index {
            next_index.insert(peer_id.clone(), conflict_index);
        } else {
            let current = *next_index.get(peer_id).unwrap_or(&1);
            next_index.insert(peer_id.clone(), current.saturating_sub(1).max(1));
        }
    }

    /// Try to advance the commit index based on match indices.
    fn try_advance_commit_index(&self) {
        let match_indices: Vec<_> = {
            let match_index = self
                .match_index
                .read()
                .expect("raft match_index lock poisoned");
            let mut indices: Vec<_> = match_index.values().copied().collect();
            indices.push(self.log.last_index());
            indices.sort_unstable();
            indices
        };

        let quorum_index = match_indices.len() / 2;
        let new_commit = match_indices[quorum_index];

        let mut state = self.state.write().expect("raft state lock poisoned");
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

                let mut state = self.state.write().expect("raft state lock poisoned");
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
    pub fn state_machine(&self) -> &dyn StateMachineBackend {
        self.state_machine.as_ref()
    }

    // =========================================================================
    // Snapshot Operations
    // =========================================================================

    /// Check if a snapshot should be taken based on log size.
    pub fn should_snapshot(&self) -> bool {
        let log_size = self.log.len() as u64;
        log_size >= self.config.snapshot_threshold
    }

    /// Take a snapshot of the current state and compact the log.
    /// Returns the snapshot metadata if successful.
    pub fn take_snapshot(&self) -> Option<SnapshotMetadata> {
        let state = self.state.read().expect("raft state lock poisoned");
        let last_applied = state.last_applied;

        if last_applied == 0 {
            return None;
        }

        // Get the term of the last applied entry
        let last_applied_term = self.log.term_at(last_applied)?;

        // Take snapshot of state machine
        let snapshot = self.state_machine.snapshot();
        let snapshot_bytes = snapshot.to_bytes();
        let size = snapshot_bytes.len() as u64;

        // Compact the log up to last_applied
        self.log.compact(last_applied, last_applied_term);

        let metadata = SnapshotMetadata {
            last_included_index: last_applied,
            last_included_term: last_applied_term,
            size,
        };

        *self
            .snapshot_metadata
            .write()
            .expect("raft snapshot_metadata lock poisoned") = Some(metadata.clone());
        Some(metadata)
    }

    /// Get the current snapshot data (for sending to followers).
    pub fn get_snapshot_data(&self) -> Option<(SnapshotMetadata, Vec<u8>)> {
        let metadata = self
            .snapshot_metadata
            .read()
            .expect("raft snapshot_metadata lock poisoned")
            .clone()?;
        let snapshot = self.state_machine.snapshot();
        let data = snapshot.to_bytes();
        Some((metadata, data))
    }

    /// Create an InstallSnapshot request for a lagging peer.
    /// Returns None if no snapshot is available or peer doesn't need it.
    pub fn create_install_snapshot(
        &self,
        peer_id: &NodeId,
        offset: u64,
    ) -> Option<InstallSnapshotRequest> {
        if !self.is_leader() {
            return None;
        }

        let next_index = *self
            .next_index
            .read()
            .expect("raft next_index lock poisoned")
            .get(peer_id)?;
        let (metadata, data) = self.get_snapshot_data()?;

        // Only send snapshot if peer needs entries before our snapshot
        if next_index > metadata.last_included_index {
            return None;
        }

        let term = self.current_term();

        // Calculate chunk boundaries
        let start = offset as usize;
        let end = std::cmp::min(start + SNAPSHOT_CHUNK_SIZE, data.len());
        let chunk = data[start..end].to_vec();
        let done = end >= data.len();

        // Compute CRC32 checksum of the full snapshot data when this is the last chunk
        let checksum = if done {
            Some(crc32fast::hash(&data))
        } else {
            None
        };

        Some(InstallSnapshotRequest {
            term,
            leader_id: self.id.clone(),
            last_included_index: metadata.last_included_index,
            last_included_term: metadata.last_included_term,
            offset,
            data: chunk,
            done,
            checksum,
        })
    }

    /// Handle an InstallSnapshot request from the leader.
    pub fn handle_install_snapshot(
        &self,
        request: &InstallSnapshotRequest,
    ) -> InstallSnapshotResponse {
        let mut state = self.state.write().expect("raft state lock poisoned");

        // Reply immediately if term < currentTerm
        if request.term < state.current_term {
            return InstallSnapshotResponse {
                term: state.current_term,
            };
        }

        // Update term if necessary
        if request.term > state.current_term {
            state.current_term = request.term;
            state.voted_for = None;
            *self.role.write().expect("raft role lock poisoned") = NodeRole::Follower;
        }

        // Update leader and reset heartbeat
        *self
            .leader_id
            .write()
            .expect("raft leader_id lock poisoned") = Some(request.leader_id.clone());
        self.reset_heartbeat();

        let mut pending = self
            .pending_snapshot
            .write()
            .expect("raft pending_snapshot lock poisoned");

        // If offset is 0, create a new pending snapshot
        if request.offset == 0 {
            *pending = Some(PendingSnapshot {
                metadata: SnapshotMetadata {
                    last_included_index: request.last_included_index,
                    last_included_term: request.last_included_term,
                    size: 0, // Will be updated when done
                },
                data: Vec::new(),
                offset: 0,
            });
        }

        // Validate we have a pending snapshot and offset matches
        if let Some(ref mut snapshot) = *pending {
            if request.offset != snapshot.offset {
                // Offset mismatch - reset and wait for offset 0
                *pending = None;
                return InstallSnapshotResponse {
                    term: state.current_term,
                };
            }

            // Append data chunk
            snapshot.data.extend_from_slice(&request.data);
            snapshot.offset += request.data.len() as u64;

            // If this is the last chunk, apply the snapshot
            if request.done {
                let snapshot_data = std::mem::take(&mut snapshot.data);
                let metadata = snapshot.metadata.clone();
                drop(pending);

                // Verify CRC32 checksum if provided
                if let Some(expected_checksum) = request.checksum {
                    let actual_checksum = crc32fast::hash(&snapshot_data);
                    if actual_checksum != expected_checksum {
                        // Checksum mismatch - discard the snapshot
                        *self
                            .pending_snapshot
                            .write()
                            .expect("raft pending_snapshot lock poisoned") = None;
                        return InstallSnapshotResponse {
                            term: state.current_term,
                        };
                    }
                }

                // Deserialize and apply the snapshot
                if let Some(restored_snapshot) = Snapshot::from_bytes(&snapshot_data) {
                    self.state_machine.restore(restored_snapshot);

                    // Update log state
                    self.log
                        .compact(metadata.last_included_index, metadata.last_included_term);

                    // Update commit and applied indices
                    state.commit_index =
                        std::cmp::max(state.commit_index, metadata.last_included_index);
                    state.last_applied = metadata.last_included_index;
                    self.log.set_commit_index(state.commit_index);
                    self.log.set_last_applied(state.last_applied);

                    // Store snapshot metadata
                    *self
                        .snapshot_metadata
                        .write()
                        .expect("raft snapshot_metadata lock poisoned") = Some(SnapshotMetadata {
                        last_included_index: metadata.last_included_index,
                        last_included_term: metadata.last_included_term,
                        size: snapshot_data.len() as u64,
                    });
                }

                // Clear pending snapshot
                *self
                    .pending_snapshot
                    .write()
                    .expect("raft pending_snapshot lock poisoned") = None;
            }
        }

        InstallSnapshotResponse {
            term: state.current_term,
        }
    }

    /// Handle an InstallSnapshot response from a follower.
    pub fn handle_install_snapshot_response(
        &self,
        peer_id: &NodeId,
        response: &InstallSnapshotResponse,
        _last_chunk_offset: u64,
        was_last_chunk: bool,
    ) {
        let mut state = self.state.write().expect("raft state lock poisoned");

        // Step down if we see a higher term
        if response.term > state.current_term {
            state.current_term = response.term;
            state.voted_for = None;
            *self.role.write().expect("raft role lock poisoned") = NodeRole::Follower;
            *self
                .leader_id
                .write()
                .expect("raft leader_id lock poisoned") = None;
            return;
        }

        if !self.is_leader() {
            return;
        }

        // If the snapshot was fully received, update next_index and match_index
        if was_last_chunk {
            if let Some(ref metadata) = *self
                .snapshot_metadata
                .read()
                .expect("raft snapshot_metadata lock poisoned")
            {
                self.next_index
                    .write()
                    .expect("raft next_index lock poisoned")
                    .insert(peer_id.clone(), metadata.last_included_index + 1);
                self.match_index
                    .write()
                    .expect("raft match_index lock poisoned")
                    .insert(peer_id.clone(), metadata.last_included_index);
            }
        }
    }

    /// Check if a peer needs a snapshot (their next_index is before our first log entry).
    pub fn peer_needs_snapshot(&self, peer_id: &NodeId) -> bool {
        let next_index = match self
            .next_index
            .read()
            .expect("raft next_index lock poisoned")
            .get(peer_id)
        {
            Some(&idx) => idx,
            None => return false,
        };

        // If we have a snapshot and peer needs entries before the snapshot
        if let Some(ref metadata) = *self
            .snapshot_metadata
            .read()
            .expect("raft snapshot_metadata lock poisoned")
        {
            return next_index <= metadata.last_included_index;
        }

        false
    }

    /// Get current snapshot metadata.
    pub fn snapshot_metadata(&self) -> Option<SnapshotMetadata> {
        self.snapshot_metadata
            .read()
            .expect("raft snapshot_metadata lock poisoned")
            .clone()
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

        let request = leader
            .create_append_entries(&NodeId::new("follower"))
            .unwrap();
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

    #[test]
    fn test_should_snapshot() {
        let config = RaftConfig {
            snapshot_threshold: 3,
            ..RaftConfig::default()
        };
        let node = RaftNode::new("node1", config);

        // Initially should not need snapshot
        assert!(!node.should_snapshot());

        // Add some entries
        use crate::log::LogEntry;
        node.log.append(LogEntry::command(1, 1, vec![1]));
        node.log.append(LogEntry::command(2, 1, vec![2]));
        assert!(!node.should_snapshot());

        // Add one more to exceed threshold
        node.log.append(LogEntry::command(3, 1, vec![3]));
        assert!(node.should_snapshot());
    }

    #[test]
    fn test_take_snapshot() {
        let node = RaftNode::new("node1", RaftConfig::default());

        // Apply some commands to state machine
        let cmd1 = Command::set("key1", b"value1".to_vec());
        let cmd2 = Command::set("key2", b"value2".to_vec());

        // Add entries to log
        use crate::log::LogEntry;
        node.log.append(LogEntry::command(1, 1, cmd1.to_bytes()));
        node.log.append(LogEntry::command(2, 1, cmd2.to_bytes()));
        node.log.set_commit_index(2);

        // Apply entries
        node.state_machine.apply(&cmd1, 1);
        node.state_machine.apply(&cmd2, 2);

        {
            let mut state = node.state.write().unwrap();
            state.last_applied = 2;
            state.commit_index = 2;
        }

        // Take snapshot
        let metadata = node.take_snapshot();
        assert!(metadata.is_some());

        let metadata = metadata.unwrap();
        assert_eq!(metadata.last_included_index, 2);
        assert_eq!(metadata.last_included_term, 1);
        assert!(metadata.size > 0);

        // Verify snapshot metadata is stored
        assert!(node.snapshot_metadata().is_some());
    }

    #[test]
    fn test_install_snapshot_single_chunk() {
        let leader = RaftNode::new("leader", RaftConfig::default());
        let follower = RaftNode::new("follower", RaftConfig::default());

        // Set up leader state
        {
            let mut state = leader.state.write().unwrap();
            state.current_term = 2;
        }
        *leader.role.write().unwrap() = NodeRole::Leader;

        // Apply commands to leader
        let cmd1 = Command::set("key1", b"value1".to_vec());
        let cmd2 = Command::set("key2", b"value2".to_vec());
        leader.state_machine.apply(&cmd1, 1);
        leader.state_machine.apply(&cmd2, 2);

        // Create snapshot data
        let snapshot = leader.state_machine.snapshot();
        let snapshot_bytes = snapshot.to_bytes();

        // Send install snapshot to follower (single chunk)
        let request = InstallSnapshotRequest {
            term: 2,
            leader_id: NodeId::new("leader"),
            last_included_index: 2,
            last_included_term: 1,
            offset: 0,
            data: snapshot_bytes,
            done: true,
            checksum: None,
        };

        let response = follower.handle_install_snapshot(&request);
        assert_eq!(response.term, 2);

        // Verify follower state was restored
        assert_eq!(follower.state_machine.get("key1").unwrap(), b"value1");
        assert_eq!(follower.state_machine.get("key2").unwrap(), b"value2");

        // Verify follower indices were updated
        let state = follower.state.read().unwrap();
        assert_eq!(state.last_applied, 2);
        assert!(state.commit_index >= 2);
    }

    #[test]
    fn test_install_snapshot_multiple_chunks() {
        let follower = RaftNode::new("follower", RaftConfig::default());

        // Create a snapshot to install
        let mut data = std::collections::HashMap::new();
        data.insert("key1".to_string(), b"value1".to_vec());
        data.insert("key2".to_string(), b"value2".to_vec());

        let snapshot = crate::state::Snapshot {
            data,
            last_applied: 5,
            version: 2,
        };
        let snapshot_bytes = snapshot.to_bytes();

        // Split into chunks (simulate chunked transfer)
        let chunk_size = snapshot_bytes.len() / 2;
        let chunk1 = &snapshot_bytes[..chunk_size];
        let chunk2 = &snapshot_bytes[chunk_size..];

        // Send first chunk
        let request1 = InstallSnapshotRequest {
            term: 1,
            leader_id: NodeId::new("leader"),
            last_included_index: 5,
            last_included_term: 1,
            offset: 0,
            data: chunk1.to_vec(),
            done: false,
            checksum: None,
        };
        let response1 = follower.handle_install_snapshot(&request1);
        assert_eq!(response1.term, 1);

        // Send second chunk
        let request2 = InstallSnapshotRequest {
            term: 1,
            leader_id: NodeId::new("leader"),
            last_included_index: 5,
            last_included_term: 1,
            offset: chunk_size as u64,
            data: chunk2.to_vec(),
            done: true,
            checksum: None,
        };
        let response2 = follower.handle_install_snapshot(&request2);
        assert_eq!(response2.term, 1);

        // Verify state was restored
        assert_eq!(follower.state_machine.get("key1").unwrap(), b"value1");
        assert_eq!(follower.state_machine.get("key2").unwrap(), b"value2");
        assert_eq!(follower.state_machine.last_applied(), 5);
    }

    #[test]
    fn test_install_snapshot_rejects_old_term() {
        let follower = RaftNode::new("follower", RaftConfig::default());

        // Set follower to higher term
        {
            let mut state = follower.state.write().unwrap();
            state.current_term = 5;
        }

        let request = InstallSnapshotRequest {
            term: 3,
            leader_id: NodeId::new("old_leader"),
            last_included_index: 10,
            last_included_term: 2,
            offset: 0,
            data: vec![1, 2, 3],
            done: true,
            checksum: None,
        };

        let response = follower.handle_install_snapshot(&request);
        assert_eq!(response.term, 5);

        // State machine should not have been modified
        assert!(follower.state_machine.is_empty());
    }

    #[test]
    fn test_peer_needs_snapshot() {
        let leader = RaftNode::new("leader", RaftConfig::default());
        leader.add_peer(NodeId::new("follower"));

        // Initially peer doesn't need snapshot (no snapshot exists)
        assert!(!leader.peer_needs_snapshot(&NodeId::new("follower")));

        // Set up a snapshot
        *leader.snapshot_metadata.write().unwrap() = Some(SnapshotMetadata {
            last_included_index: 100,
            last_included_term: 5,
            size: 1000,
        });

        // Peer's next_index is 1, which is before snapshot - needs snapshot
        assert!(leader.peer_needs_snapshot(&NodeId::new("follower")));

        // Update peer's next_index to after snapshot
        leader
            .next_index
            .write()
            .unwrap()
            .insert(NodeId::new("follower"), 101);
        assert!(!leader.peer_needs_snapshot(&NodeId::new("follower")));
    }

    #[test]
    fn test_create_install_snapshot() {
        let leader = RaftNode::new("leader", RaftConfig::default());
        leader.add_peer(NodeId::new("follower"));

        // Set up as leader
        {
            let mut state = leader.state.write().unwrap();
            state.current_term = 2;
        }
        *leader.role.write().unwrap() = NodeRole::Leader;

        // Apply some data to state machine
        let cmd = Command::set("test_key", b"test_value".to_vec());
        leader.state_machine.apply(&cmd, 1);

        // Add log entry and set applied
        use crate::log::LogEntry;
        leader.log.append(LogEntry::command(1, 2, cmd.to_bytes()));
        leader.log.set_commit_index(1);
        {
            let mut state = leader.state.write().unwrap();
            state.last_applied = 1;
            state.commit_index = 1;
        }

        // Take snapshot
        leader.take_snapshot();

        // Set peer's next_index to 0 to trigger snapshot need
        leader
            .next_index
            .write()
            .unwrap()
            .insert(NodeId::new("follower"), 0);

        // Create install snapshot request
        let request = leader.create_install_snapshot(&NodeId::new("follower"), 0);
        assert!(request.is_some());

        let request = request.unwrap();
        assert_eq!(request.term, 2);
        assert_eq!(request.leader_id.as_str(), "leader");
        assert_eq!(request.last_included_index, 1);
        assert_eq!(request.last_included_term, 2);
        assert_eq!(request.offset, 0);
        assert!(!request.data.is_empty());
    }

    // =========================================================================
    // Leader Lease Tests
    // =========================================================================

    #[test]
    fn test_leader_lease_not_valid_initially() {
        let node = RaftNode::new("node1", RaftConfig::default());
        assert!(!node.has_valid_lease());
        assert!(!node.is_leader_with_lease());
    }

    #[test]
    fn test_extend_and_check_lease() {
        let node = RaftNode::new("node1", RaftConfig::default());
        node.extend_lease();
        assert!(node.has_valid_lease());
    }

    #[test]
    fn test_is_leader_with_lease() {
        let node = RaftNode::new("node1", RaftConfig::default());
        node.add_peer(NodeId::new("node2"));

        // Become leader
        node.start_election();
        let response = VoteResponse {
            term: 1,
            vote_granted: true,
            voter_id: NodeId::new("node2"),
        };
        node.handle_vote_response(&response);
        assert!(node.is_leader());

        // No lease yet
        assert!(!node.is_leader_with_lease());

        // Extend lease
        node.extend_lease();
        assert!(node.is_leader_with_lease());
    }

    #[test]
    fn test_check_lease_steps_down_leader() {
        let config = RaftConfig {
            lease_duration: Duration::from_millis(1),
            ..RaftConfig::default()
        };
        let node = RaftNode::new("node1", config);
        node.add_peer(NodeId::new("node2"));

        // Become leader
        node.start_election();
        let response = VoteResponse {
            term: 1,
            vote_granted: true,
            voter_id: NodeId::new("node2"),
        };
        node.handle_vote_response(&response);
        assert!(node.is_leader());

        // Lease is not set, so check_lease should step down
        node.check_lease();
        assert!(!node.is_leader());
        assert_eq!(node.role(), NodeRole::Follower);
    }

    #[test]
    fn test_lease_extended_on_majority_ack() {
        let node = RaftNode::new("leader", RaftConfig::default());
        node.add_peer(NodeId::new("follower1"));
        node.add_peer(NodeId::new("follower2"));

        // Become leader (need 2 votes out of 3)
        node.start_election();
        node.handle_vote_response(&VoteResponse {
            term: 1,
            vote_granted: true,
            voter_id: NodeId::new("follower1"),
        });
        assert!(node.is_leader());

        // Propose a command so there's something to replicate
        let command = Command::set("key", b"value".to_vec());
        node.propose(command).unwrap();

        // Simulate successful append entries response from follower1
        let response = AppendEntriesResponse {
            term: 1,
            success: true,
            match_index: 2, // noop + command
            conflict_index: None,
            conflict_term: None,
        };
        node.handle_append_entries_response(&NodeId::new("follower1"), &response);

        // With leader + follower1 acking, that's 2/3 = majority, lease should be extended
        assert!(node.has_valid_lease());
        assert!(node.is_leader_with_lease());
    }

    // =========================================================================
    // Snapshot Checksum Tests
    // =========================================================================

    #[test]
    fn test_create_install_snapshot_has_checksum() {
        let leader = RaftNode::new("leader", RaftConfig::default());
        leader.add_peer(NodeId::new("follower"));

        // Set up as leader
        {
            let mut state = leader.state.write().unwrap();
            state.current_term = 2;
        }
        *leader.role.write().unwrap() = NodeRole::Leader;

        // Apply some data
        let cmd = Command::set("test_key", b"test_value".to_vec());
        leader.state_machine.apply(&cmd, 1);

        use crate::log::LogEntry;
        leader.log.append(LogEntry::command(1, 2, cmd.to_bytes()));
        leader.log.set_commit_index(1);
        {
            let mut state = leader.state.write().unwrap();
            state.last_applied = 1;
            state.commit_index = 1;
        }

        leader.take_snapshot();

        leader
            .next_index
            .write()
            .unwrap()
            .insert(NodeId::new("follower"), 0);

        // The snapshot is small enough to fit in one chunk, so done=true and checksum should be set
        let request = leader
            .create_install_snapshot(&NodeId::new("follower"), 0)
            .unwrap();
        assert!(request.done);
        assert!(request.checksum.is_some());

        // Verify checksum matches the full snapshot data
        let (_, full_data) = leader.get_snapshot_data().unwrap();
        let expected_checksum = crc32fast::hash(&full_data);
        assert_eq!(request.checksum.unwrap(), expected_checksum);
    }

    #[test]
    fn test_snapshot_checksum_mismatch_discards() {
        let follower = RaftNode::new("follower", RaftConfig::default());

        // Create valid snapshot data
        let mut data = std::collections::HashMap::new();
        data.insert("key1".to_string(), b"value1".to_vec());
        let snapshot = crate::state::Snapshot {
            data,
            last_applied: 3,
            version: 2,
        };
        let snapshot_bytes = snapshot.to_bytes();

        // Send with a wrong checksum
        let request = InstallSnapshotRequest {
            term: 1,
            leader_id: NodeId::new("leader"),
            last_included_index: 3,
            last_included_term: 1,
            offset: 0,
            data: snapshot_bytes,
            done: true,
            checksum: Some(0xDEADBEEF), // intentionally wrong
        };

        let _response = follower.handle_install_snapshot(&request);

        // State machine should NOT have been modified because checksum failed
        assert!(follower.state_machine.is_empty());
    }

    #[test]
    fn test_snapshot_checksum_valid_applies() {
        let follower = RaftNode::new("follower", RaftConfig::default());

        let mut data = std::collections::HashMap::new();
        data.insert("key1".to_string(), b"value1".to_vec());
        let snapshot = crate::state::Snapshot {
            data,
            last_applied: 3,
            version: 2,
        };
        let snapshot_bytes = snapshot.to_bytes();
        let correct_checksum = crc32fast::hash(&snapshot_bytes);

        let request = InstallSnapshotRequest {
            term: 1,
            leader_id: NodeId::new("leader"),
            last_included_index: 3,
            last_included_term: 1,
            offset: 0,
            data: snapshot_bytes,
            done: true,
            checksum: Some(correct_checksum),
        };

        let _response = follower.handle_install_snapshot(&request);

        // State machine should have been restored
        assert_eq!(follower.state_machine.get("key1").unwrap(), b"value1");
    }
}
