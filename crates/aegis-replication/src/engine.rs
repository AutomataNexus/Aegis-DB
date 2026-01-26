//! Aegis Replication Engine
//!
//! Main replication engine coordinating Raft consensus and cluster management.
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use crate::cluster::{Cluster, ClusterConfig, ClusterError, ClusterState, ClusterStats};
use crate::node::{NodeHealth, NodeId, NodeInfo, NodeRole};
use crate::raft::{AppendEntriesRequest, RaftConfig, RaftNode, VoteRequest, VoteResponse};
use crate::state::{Command, CommandResult};
use crate::transport::{Message, MessagePayload, Transport};
use std::sync::Arc;
use std::time::{Duration, Instant};

// =============================================================================
// Replication Engine Configuration
// =============================================================================

/// Configuration for the replication engine.
#[derive(Debug, Clone)]
pub struct ReplicationConfig {
    pub raft: RaftConfig,
    pub cluster: ClusterConfig,
    pub tick_interval: Duration,
    pub apply_batch_size: usize,
}

impl Default for ReplicationConfig {
    fn default() -> Self {
        Self {
            raft: RaftConfig::default(),
            cluster: ClusterConfig::default(),
            tick_interval: Duration::from_millis(10),
            apply_batch_size: 100,
        }
    }
}

impl ReplicationConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_raft(mut self, raft: RaftConfig) -> Self {
        self.raft = raft;
        self
    }

    pub fn with_cluster(mut self, cluster: ClusterConfig) -> Self {
        self.cluster = cluster;
        self
    }
}

// =============================================================================
// Replication Engine
// =============================================================================

/// The main replication engine.
pub struct ReplicationEngine {
    config: ReplicationConfig,
    raft: Arc<RaftNode>,
    cluster: Arc<Cluster>,
    last_tick: Instant,
    pending_proposals: Vec<PendingProposal>,
}

/// A pending proposal waiting to be committed.
#[derive(Debug)]
#[allow(dead_code)]
struct PendingProposal {
    index: u64,
    command: Command,
    proposed_at: Instant,
}

impl ReplicationEngine {
    /// Create a new replication engine.
    pub fn new(local_node: NodeInfo, config: ReplicationConfig) -> Self {
        let node_id = local_node.id.clone();
        let raft = Arc::new(RaftNode::new(node_id, config.raft.clone()));
        let cluster = Arc::new(Cluster::new(local_node, config.cluster.clone()));

        Self {
            config,
            raft,
            cluster,
            last_tick: Instant::now(),
            pending_proposals: Vec::new(),
        }
    }

    /// Get the node ID.
    pub fn node_id(&self) -> NodeId {
        self.raft.id()
    }

    /// Get the current role.
    pub fn role(&self) -> NodeRole {
        self.raft.role()
    }

    /// Check if this node is the leader.
    pub fn is_leader(&self) -> bool {
        self.raft.is_leader()
    }

    /// Get the current leader ID.
    pub fn leader_id(&self) -> Option<NodeId> {
        self.raft.leader_id()
    }

    /// Get the current term.
    pub fn current_term(&self) -> u64 {
        self.raft.current_term()
    }

    /// Get cluster state.
    pub fn cluster_state(&self) -> ClusterState {
        self.cluster.state()
    }

    /// Get cluster stats.
    pub fn cluster_stats(&self) -> ClusterStats {
        self.cluster.stats()
    }

    // =========================================================================
    // Cluster Management
    // =========================================================================

    /// Add a peer to the cluster.
    pub fn add_peer(&self, peer: NodeInfo) -> Result<(), ClusterError> {
        let peer_id = peer.id.clone();
        self.cluster.add_node(peer)?;
        self.raft.add_peer(peer_id);
        Ok(())
    }

    /// Remove a peer from the cluster.
    pub fn remove_peer(&self, peer_id: &NodeId) -> Result<NodeInfo, ClusterError> {
        self.raft.remove_peer(peer_id);
        self.cluster.remove_node(peer_id)
    }

    /// Get all peers.
    pub fn peers(&self) -> Vec<NodeInfo> {
        self.cluster.peers()
    }

    /// Get peer IDs.
    pub fn peer_ids(&self) -> Vec<NodeId> {
        self.cluster.peer_ids()
    }

    // =========================================================================
    // State Machine Operations
    // =========================================================================

    /// Propose a command to the cluster.
    pub fn propose(&mut self, command: Command) -> Result<u64, ReplicationError> {
        if !self.is_leader() {
            return Err(ReplicationError::NotLeader(self.leader_id()));
        }

        let index = self
            .raft
            .propose(command.clone())
            .map_err(ReplicationError::ProposalFailed)?;

        self.pending_proposals.push(PendingProposal {
            index,
            command,
            proposed_at: Instant::now(),
        });

        Ok(index)
    }

    /// Get a value from the state machine.
    pub fn get(&self, key: &str) -> Option<Vec<u8>> {
        self.raft.get(key)
    }

    /// Set a value (proposes to cluster if leader).
    pub fn set(&mut self, key: impl Into<String>, value: Vec<u8>) -> Result<u64, ReplicationError> {
        let command = Command::set(key, value);
        self.propose(command)
    }

    /// Delete a value (proposes to cluster if leader).
    pub fn delete(&mut self, key: impl Into<String>) -> Result<u64, ReplicationError> {
        let command = Command::delete(key);
        self.propose(command)
    }

    /// Apply committed entries to the state machine.
    pub fn apply_committed(&self) -> Vec<CommandResult> {
        self.raft.apply_committed()
    }

    // =========================================================================
    // Tick / Event Loop
    // =========================================================================

    /// Process a tick of the event loop.
    pub fn tick(&mut self) -> TickResult {
        let mut result = TickResult::default();

        let elapsed = self.last_tick.elapsed();
        if elapsed < self.config.tick_interval {
            return result;
        }
        self.last_tick = Instant::now();

        match self.raft.role() {
            NodeRole::Follower | NodeRole::Candidate => {
                if self.raft.election_timeout_elapsed() {
                    result.should_start_election = true;
                }
            }
            NodeRole::Leader => {
                result.should_send_heartbeats = true;
            }
        }

        let failed = self.cluster.check_failures();
        result.failed_nodes = failed;

        let applied = self.apply_committed();
        result.applied_count = applied.len();

        self.cleanup_pending_proposals();

        result
    }

    /// Start an election.
    pub fn start_election(&self) -> VoteRequest {
        self.raft.start_election()
    }

    /// Handle a vote request.
    pub fn handle_vote_request(&self, request: &VoteRequest) -> VoteResponse {
        let response = self.raft.handle_vote_request(request);
        self.sync_role_to_cluster();
        response
    }

    /// Handle a vote response.
    pub fn handle_vote_response(&self, response: &VoteResponse) -> bool {
        let became_leader = self.raft.handle_vote_response(response);
        if became_leader {
            self.cluster.set_leader(Some(self.node_id()));
            self.cluster
                .set_node_role(&self.node_id(), NodeRole::Leader);
        }
        self.sync_role_to_cluster();
        became_leader
    }

    /// Create append entries requests for all peers.
    pub fn create_append_entries_for_peers(&self) -> Vec<(NodeId, AppendEntriesRequest)> {
        self.peer_ids()
            .into_iter()
            .filter_map(|peer_id| {
                self.raft
                    .create_append_entries(&peer_id)
                    .map(|req| (peer_id, req))
            })
            .collect()
    }

    /// Handle an append entries request.
    pub fn handle_append_entries(
        &self,
        request: &AppendEntriesRequest,
    ) -> crate::raft::AppendEntriesResponse {
        let response = self.raft.handle_append_entries(request);

        if response.success {
            self.cluster.set_leader(Some(request.leader_id.clone()));
            self.cluster.heartbeat(&request.leader_id);
        }

        self.sync_role_to_cluster();
        response
    }

    /// Handle an append entries response.
    pub fn handle_append_entries_response(
        &self,
        peer_id: &NodeId,
        response: &crate::raft::AppendEntriesResponse,
    ) {
        self.raft.handle_append_entries_response(peer_id, response);
        self.sync_role_to_cluster();
    }

    fn sync_role_to_cluster(&self) {
        let role = self.raft.role();
        self.cluster.set_node_role(&self.node_id(), role);

        if role != NodeRole::Leader && self.cluster.is_leader() {
            self.cluster.set_leader(None);
        }
    }

    fn cleanup_pending_proposals(&mut self) {
        let timeout = Duration::from_secs(30);
        self.pending_proposals
            .retain(|p| p.proposed_at.elapsed() < timeout);
    }

    // =========================================================================
    // Health Management
    // =========================================================================

    /// Update health for a node.
    pub fn update_health(&self, health: NodeHealth) {
        self.cluster.update_health(health);
    }

    /// Report heartbeat from a peer.
    pub fn heartbeat(&self, peer_id: &NodeId) {
        self.cluster.heartbeat(peer_id);
        self.raft.reset_heartbeat();
    }

    // =========================================================================
    // Transport Integration
    // =========================================================================

    /// Process incoming message.
    pub fn process_message(&mut self, message: Message) -> Option<Message> {
        match message.payload {
            MessagePayload::VoteRequest(ref req) => {
                let response = self.handle_vote_request(req);
                Some(Message::vote_response(
                    self.node_id(),
                    message.from,
                    response,
                ))
            }
            MessagePayload::VoteResponse(ref resp) => {
                self.handle_vote_response(resp);
                None
            }
            MessagePayload::AppendEntries(ref req) => {
                let response = self.handle_append_entries(req);
                Some(Message::append_entries_response(
                    self.node_id(),
                    message.from,
                    response,
                ))
            }
            MessagePayload::AppendEntriesResponse(ref resp) => {
                self.handle_append_entries_response(&message.from, resp);
                None
            }
            MessagePayload::Heartbeat => {
                self.heartbeat(&message.from);
                None
            }
            _ => None,
        }
    }

    /// Send messages to peers using transport.
    pub fn send_heartbeats(&self, transport: &dyn Transport) {
        if !self.is_leader() {
            return;
        }

        for (peer_id, request) in self.create_append_entries_for_peers() {
            let msg = Message::append_entries(self.node_id(), peer_id, request);
            let _ = transport.send(msg);
        }
    }

    /// Broadcast election request.
    pub fn broadcast_election(&self, transport: &dyn Transport) {
        let request = self.start_election();
        for peer_id in self.peer_ids() {
            let msg = Message::vote_request(self.node_id(), peer_id, request.clone());
            let _ = transport.send(msg);
        }
    }

    // =========================================================================
    // Accessors
    // =========================================================================

    /// Get the Raft node.
    pub fn raft(&self) -> &RaftNode {
        &self.raft
    }

    /// Get the cluster.
    pub fn cluster(&self) -> &Cluster {
        &self.cluster
    }

    /// Get the configuration.
    pub fn config(&self) -> &ReplicationConfig {
        &self.config
    }
}

// =============================================================================
// Tick Result
// =============================================================================

/// Result of a tick operation.
#[derive(Debug, Default)]
pub struct TickResult {
    pub should_start_election: bool,
    pub should_send_heartbeats: bool,
    pub failed_nodes: Vec<NodeId>,
    pub applied_count: usize,
}

// =============================================================================
// Replication Error
// =============================================================================

/// Errors that can occur during replication.
#[derive(Debug)]
pub enum ReplicationError {
    NotLeader(Option<NodeId>),
    ProposalFailed(String),
    ClusterError(ClusterError),
    Timeout,
}

impl std::fmt::Display for ReplicationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotLeader(leader) => match leader {
                Some(id) => write!(f, "Not the leader, current leader: {}", id),
                None => write!(f, "Not the leader, no leader elected"),
            },
            Self::ProposalFailed(e) => write!(f, "Proposal failed: {}", e),
            Self::ClusterError(e) => write!(f, "Cluster error: {}", e),
            Self::Timeout => write!(f, "Operation timed out"),
        }
    }
}

impl std::error::Error for ReplicationError {}

impl From<ClusterError> for ReplicationError {
    fn from(e: ClusterError) -> Self {
        Self::ClusterError(e)
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn create_engine(id: &str) -> ReplicationEngine {
        let node = NodeInfo::new(id, "127.0.0.1", 5000);
        ReplicationEngine::new(node, ReplicationConfig::default())
    }

    #[test]
    fn test_engine_creation() {
        let engine = create_engine("node1");

        assert_eq!(engine.node_id().as_str(), "node1");
        assert_eq!(engine.role(), NodeRole::Follower);
        assert!(!engine.is_leader());
    }

    #[test]
    fn test_add_remove_peer() {
        let engine = create_engine("node1");

        let peer = NodeInfo::new("node2", "127.0.0.1", 5001);
        engine.add_peer(peer).unwrap();

        assert_eq!(engine.peers().len(), 1);

        engine.remove_peer(&NodeId::new("node2")).unwrap();
        assert_eq!(engine.peers().len(), 0);
    }

    #[test]
    fn test_propose_not_leader() {
        let mut engine = create_engine("node1");

        let command = Command::set("key", b"value".to_vec());
        let result = engine.propose(command);

        assert!(matches!(result, Err(ReplicationError::NotLeader(_))));
    }

    #[test]
    fn test_become_leader_and_propose() {
        let mut engine = create_engine("node1");

        let peer = NodeInfo::new("node2", "127.0.0.1", 5001);
        engine.add_peer(peer).unwrap();

        let request = engine.start_election();
        let response = VoteResponse {
            term: request.term,
            vote_granted: true,
            voter_id: NodeId::new("node2"),
        };
        engine.handle_vote_response(&response);

        assert!(engine.is_leader());

        let index = engine.set("key1", b"value1".to_vec()).unwrap();
        assert!(index > 0);
    }

    #[test]
    fn test_tick_result() {
        let mut engine = create_engine("node1");

        std::thread::sleep(Duration::from_millis(15));
        let result = engine.tick();

        assert!(result.should_start_election || result.applied_count == 0);
    }

    #[test]
    fn test_leader_creates_append_entries() {
        let engine = create_engine("node1");

        let peer = NodeInfo::new("node2", "127.0.0.1", 5001);
        engine.add_peer(peer).unwrap();

        engine.start_election();
        engine.handle_vote_response(&VoteResponse {
            term: 1,
            vote_granted: true,
            voter_id: NodeId::new("node2"),
        });

        let requests = engine.create_append_entries_for_peers();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].0.as_str(), "node2");
    }

    #[test]
    #[allow(unused_mut)]
    fn test_message_processing() {
        let mut engine1 = create_engine("node1");
        let mut engine2 = create_engine("node2");

        engine1
            .add_peer(NodeInfo::new("node2", "127.0.0.1", 5001))
            .unwrap();
        engine2
            .add_peer(NodeInfo::new("node1", "127.0.0.1", 5000))
            .unwrap();

        let vote_request = engine1.start_election();
        let msg = Message::vote_request(
            NodeId::new("node1"),
            NodeId::new("node2"),
            vote_request,
        );

        let response_msg = engine2.process_message(msg).unwrap();

        if let MessagePayload::VoteResponse(resp) = response_msg.payload {
            assert!(resp.vote_granted);
        } else {
            panic!("Expected VoteResponse");
        }
    }

    #[test]
    fn test_cluster_stats() {
        let engine = create_engine("node1");

        let mut peer = NodeInfo::new("node2", "127.0.0.1", 5001);
        peer.mark_healthy();
        engine.add_peer(peer).unwrap();

        let stats = engine.cluster_stats();
        assert_eq!(stats.total_nodes, 2);
    }

    #[test]
    fn test_get_set() {
        let mut engine = create_engine("node1");
        let peer = NodeInfo::new("node2", "127.0.0.1", 5001);
        engine.add_peer(peer).unwrap();

        engine.start_election();
        engine.handle_vote_response(&VoteResponse {
            term: 1,
            vote_granted: true,
            voter_id: NodeId::new("node2"),
        });

        engine.set("key1", b"value1".to_vec()).unwrap();

        engine.raft.log().set_commit_index(engine.raft.log().last_index());
        engine.apply_committed();

        let value = engine.get("key1");
        assert_eq!(value, Some(b"value1".to_vec()));
    }
}
