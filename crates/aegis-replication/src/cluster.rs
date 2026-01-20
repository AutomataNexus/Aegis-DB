//! Aegis Cluster Management
//!
//! Cluster coordination and membership management.
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use crate::node::{NodeHealth, NodeId, NodeInfo, NodeRole, NodeStatus};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::RwLock;
use std::time::Duration;

// =============================================================================
// Cluster Configuration
// =============================================================================

/// Configuration for a cluster.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterConfig {
    pub cluster_id: String,
    pub min_nodes: usize,
    pub max_nodes: usize,
    pub heartbeat_interval: Duration,
    pub failure_timeout: Duration,
    pub replication_factor: usize,
    pub quorum_size: usize,
}

impl Default for ClusterConfig {
    fn default() -> Self {
        Self {
            cluster_id: "aegis-cluster".to_string(),
            min_nodes: 1,
            max_nodes: 100,
            heartbeat_interval: Duration::from_secs(1),
            failure_timeout: Duration::from_secs(5),
            replication_factor: 3,
            quorum_size: 2,
        }
    }
}

impl ClusterConfig {
    pub fn new(cluster_id: impl Into<String>) -> Self {
        Self {
            cluster_id: cluster_id.into(),
            ..Default::default()
        }
    }

    pub fn with_replication_factor(mut self, factor: usize) -> Self {
        self.replication_factor = factor;
        self.quorum_size = (factor / 2) + 1;
        self
    }

    pub fn with_heartbeat_interval(mut self, interval: Duration) -> Self {
        self.heartbeat_interval = interval;
        self
    }

    pub fn with_failure_timeout(mut self, timeout: Duration) -> Self {
        self.failure_timeout = timeout;
        self
    }
}

// =============================================================================
// Cluster State
// =============================================================================

/// State of the cluster.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClusterState {
    /// Cluster is initializing.
    Initializing,
    /// Cluster is forming (waiting for quorum).
    Forming,
    /// Cluster is healthy and operational.
    Healthy,
    /// Cluster is degraded (some nodes down).
    Degraded,
    /// Cluster has lost quorum.
    NoQuorum,
    /// Cluster is shutting down.
    ShuttingDown,
}

impl Default for ClusterState {
    fn default() -> Self {
        Self::Initializing
    }
}

// =============================================================================
// Membership Change
// =============================================================================

/// Type of membership change.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MembershipChange {
    AddNode(NodeInfo),
    RemoveNode(NodeId),
    UpdateNode(NodeInfo),
}

/// A membership change request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MembershipChangeRequest {
    pub change_id: String,
    pub change: MembershipChange,
    pub requested_at: u64,
}

// =============================================================================
// Cluster
// =============================================================================

/// A distributed cluster of nodes.
pub struct Cluster {
    config: ClusterConfig,
    state: RwLock<ClusterState>,
    nodes: RwLock<HashMap<NodeId, NodeInfo>>,
    leader_id: RwLock<Option<NodeId>>,
    local_node_id: NodeId,
    health_checks: RwLock<HashMap<NodeId, NodeHealth>>,
}

impl Cluster {
    /// Create a new cluster.
    pub fn new(local_node: NodeInfo, config: ClusterConfig) -> Self {
        let local_id = local_node.id.clone();
        let mut nodes = HashMap::new();
        nodes.insert(local_id.clone(), local_node);

        Self {
            config,
            state: RwLock::new(ClusterState::Initializing),
            nodes: RwLock::new(nodes),
            leader_id: RwLock::new(None),
            local_node_id: local_id,
            health_checks: RwLock::new(HashMap::new()),
        }
    }

    /// Get the cluster ID.
    pub fn id(&self) -> &str {
        &self.config.cluster_id
    }

    /// Get the cluster configuration.
    pub fn config(&self) -> &ClusterConfig {
        &self.config
    }

    /// Get the current cluster state.
    pub fn state(&self) -> ClusterState {
        *self.state.read().unwrap()
    }

    /// Set the cluster state.
    pub fn set_state(&self, state: ClusterState) {
        *self.state.write().unwrap() = state;
    }

    /// Get the local node ID.
    pub fn local_node_id(&self) -> &NodeId {
        &self.local_node_id
    }

    /// Get the current leader ID.
    pub fn leader_id(&self) -> Option<NodeId> {
        self.leader_id.read().unwrap().clone()
    }

    /// Set the leader ID.
    pub fn set_leader(&self, leader_id: Option<NodeId>) {
        *self.leader_id.write().unwrap() = leader_id;
    }

    /// Check if this node is the leader.
    pub fn is_leader(&self) -> bool {
        self.leader_id()
            .map(|id| id == self.local_node_id)
            .unwrap_or(false)
    }

    // =========================================================================
    // Node Management
    // =========================================================================

    /// Add a node to the cluster.
    pub fn add_node(&self, node: NodeInfo) -> Result<(), ClusterError> {
        let mut nodes = self.nodes.write().unwrap();

        if nodes.len() >= self.config.max_nodes {
            return Err(ClusterError::MaxNodesReached);
        }

        if nodes.contains_key(&node.id) {
            return Err(ClusterError::NodeAlreadyExists(node.id.clone()));
        }

        nodes.insert(node.id.clone(), node);
        drop(nodes);

        self.update_cluster_state();
        Ok(())
    }

    /// Remove a node from the cluster.
    pub fn remove_node(&self, node_id: &NodeId) -> Result<NodeInfo, ClusterError> {
        if node_id == &self.local_node_id {
            return Err(ClusterError::CannotRemoveLocalNode);
        }

        let mut nodes = self.nodes.write().unwrap();
        let node = nodes
            .remove(node_id)
            .ok_or_else(|| ClusterError::NodeNotFound(node_id.clone()))?;

        drop(nodes);
        self.update_cluster_state();
        Ok(node)
    }

    /// Get a node by ID.
    pub fn get_node(&self, node_id: &NodeId) -> Option<NodeInfo> {
        self.nodes.read().unwrap().get(node_id).cloned()
    }

    /// Get all nodes in the cluster.
    pub fn nodes(&self) -> Vec<NodeInfo> {
        self.nodes.read().unwrap().values().cloned().collect()
    }

    /// Get all node IDs.
    pub fn node_ids(&self) -> Vec<NodeId> {
        self.nodes.read().unwrap().keys().cloned().collect()
    }

    /// Get the number of nodes.
    pub fn node_count(&self) -> usize {
        self.nodes.read().unwrap().len()
    }

    /// Get peer nodes (excluding local).
    pub fn peers(&self) -> Vec<NodeInfo> {
        self.nodes
            .read()
            .unwrap()
            .values()
            .filter(|n| n.id != self.local_node_id)
            .cloned()
            .collect()
    }

    /// Get peer IDs.
    pub fn peer_ids(&self) -> Vec<NodeId> {
        self.nodes
            .read()
            .unwrap()
            .keys()
            .filter(|id| *id != &self.local_node_id)
            .cloned()
            .collect()
    }

    // =========================================================================
    // Health Management
    // =========================================================================

    /// Update node health.
    pub fn update_health(&self, health: NodeHealth) {
        let node_id = health.node_id.clone();
        let mut checks = self.health_checks.write().unwrap();
        checks.insert(node_id.clone(), health.clone());
        drop(checks);

        let mut nodes = self.nodes.write().unwrap();
        if let Some(node) = nodes.get_mut(&node_id) {
            if health.healthy {
                node.mark_healthy();
            } else {
                node.mark_suspect();
            }
        }
        drop(nodes);

        self.update_cluster_state();
    }

    /// Get health for a node.
    pub fn get_health(&self, node_id: &NodeId) -> Option<NodeHealth> {
        self.health_checks.read().unwrap().get(node_id).cloned()
    }

    /// Process heartbeat from a node.
    pub fn heartbeat(&self, node_id: &NodeId) {
        let mut nodes = self.nodes.write().unwrap();
        if let Some(node) = nodes.get_mut(node_id) {
            node.heartbeat();
        }
    }

    /// Check for failed nodes based on timeout.
    pub fn check_failures(&self) -> Vec<NodeId> {
        let timeout_ms = self.config.failure_timeout.as_millis() as u64;
        let mut failed = Vec::new();

        let mut nodes = self.nodes.write().unwrap();
        for node in nodes.values_mut() {
            if node.id == self.local_node_id {
                continue;
            }

            if node.heartbeat_age() > timeout_ms {
                if node.status == NodeStatus::Healthy {
                    node.mark_suspect();
                } else if node.status == NodeStatus::Suspect {
                    node.mark_down();
                    failed.push(node.id.clone());
                }
            }
        }

        drop(nodes);

        if !failed.is_empty() {
            self.update_cluster_state();
        }

        failed
    }

    // =========================================================================
    // Cluster State Management
    // =========================================================================

    /// Update the cluster state based on node health.
    fn update_cluster_state(&self) {
        let nodes = self.nodes.read().unwrap();
        let total = nodes.len();
        let healthy = nodes
            .values()
            .filter(|n| n.status == NodeStatus::Healthy || n.status == NodeStatus::Starting)
            .count();

        drop(nodes);

        let new_state = if total < self.config.min_nodes {
            ClusterState::Forming
        } else if healthy >= self.config.quorum_size {
            if healthy == total {
                ClusterState::Healthy
            } else {
                ClusterState::Degraded
            }
        } else {
            ClusterState::NoQuorum
        };

        self.set_state(new_state);
    }

    /// Check if cluster has quorum.
    pub fn has_quorum(&self) -> bool {
        let nodes = self.nodes.read().unwrap();
        let healthy = nodes
            .values()
            .filter(|n| n.status == NodeStatus::Healthy)
            .count();
        healthy >= self.config.quorum_size
    }

    /// Get cluster statistics.
    pub fn stats(&self) -> ClusterStats {
        let nodes = self.nodes.read().unwrap();
        let total = nodes.len();
        let healthy = nodes
            .values()
            .filter(|n| n.status == NodeStatus::Healthy)
            .count();
        let suspect = nodes
            .values()
            .filter(|n| n.status == NodeStatus::Suspect)
            .count();
        let down = nodes
            .values()
            .filter(|n| n.status == NodeStatus::Down)
            .count();

        ClusterStats {
            cluster_id: self.config.cluster_id.clone(),
            state: self.state(),
            total_nodes: total,
            healthy_nodes: healthy,
            suspect_nodes: suspect,
            down_nodes: down,
            has_leader: self.leader_id().is_some(),
            has_quorum: healthy >= self.config.quorum_size,
        }
    }

    // =========================================================================
    // Leader Election Support
    // =========================================================================

    /// Get nodes that can vote.
    pub fn voting_members(&self) -> Vec<NodeId> {
        self.nodes
            .read()
            .unwrap()
            .values()
            .filter(|n| n.is_available())
            .map(|n| n.id.clone())
            .collect()
    }

    /// Update node role.
    pub fn set_node_role(&self, node_id: &NodeId, role: NodeRole) {
        let mut nodes = self.nodes.write().unwrap();
        if let Some(node) = nodes.get_mut(node_id) {
            node.role = role;
        }
    }

    /// Get the current leader node.
    pub fn leader(&self) -> Option<NodeInfo> {
        let leader_id = self.leader_id()?;
        self.get_node(&leader_id)
    }
}

// =============================================================================
// Cluster Statistics
// =============================================================================

/// Statistics about the cluster.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterStats {
    pub cluster_id: String,
    pub state: ClusterState,
    pub total_nodes: usize,
    pub healthy_nodes: usize,
    pub suspect_nodes: usize,
    pub down_nodes: usize,
    pub has_leader: bool,
    pub has_quorum: bool,
}

// =============================================================================
// Cluster Error
// =============================================================================

/// Errors that can occur in cluster operations.
#[derive(Debug, Clone)]
pub enum ClusterError {
    NodeNotFound(NodeId),
    NodeAlreadyExists(NodeId),
    MaxNodesReached,
    CannotRemoveLocalNode,
    NoQuorum,
    NotLeader,
    ConfigurationError(String),
}

impl std::fmt::Display for ClusterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NodeNotFound(id) => write!(f, "Node not found: {}", id),
            Self::NodeAlreadyExists(id) => write!(f, "Node already exists: {}", id),
            Self::MaxNodesReached => write!(f, "Maximum number of nodes reached"),
            Self::CannotRemoveLocalNode => write!(f, "Cannot remove local node"),
            Self::NoQuorum => write!(f, "Cluster has no quorum"),
            Self::NotLeader => write!(f, "Not the leader"),
            Self::ConfigurationError(msg) => write!(f, "Configuration error: {}", msg),
        }
    }
}

impl std::error::Error for ClusterError {}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn create_node(id: &str) -> NodeInfo {
        NodeInfo::new(id, "127.0.0.1", 5000)
    }

    #[test]
    fn test_cluster_config() {
        let config = ClusterConfig::new("test-cluster")
            .with_replication_factor(3)
            .with_heartbeat_interval(Duration::from_secs(2));

        assert_eq!(config.cluster_id, "test-cluster");
        assert_eq!(config.replication_factor, 3);
        assert_eq!(config.quorum_size, 2);
    }

    #[test]
    fn test_cluster_creation() {
        let node = create_node("node1");
        let cluster = Cluster::new(node, ClusterConfig::default());

        assert_eq!(cluster.node_count(), 1);
        assert_eq!(cluster.state(), ClusterState::Initializing);
        assert!(cluster.leader_id().is_none());
    }

    #[test]
    fn test_add_remove_node() {
        let node1 = create_node("node1");
        let cluster = Cluster::new(node1, ClusterConfig::default());

        let node2 = create_node("node2");
        cluster.add_node(node2).unwrap();
        assert_eq!(cluster.node_count(), 2);

        cluster.remove_node(&NodeId::new("node2")).unwrap();
        assert_eq!(cluster.node_count(), 1);
    }

    #[test]
    fn test_cannot_remove_local_node() {
        let node = create_node("node1");
        let cluster = Cluster::new(node, ClusterConfig::default());

        let result = cluster.remove_node(&NodeId::new("node1"));
        assert!(matches!(result, Err(ClusterError::CannotRemoveLocalNode)));
    }

    #[test]
    fn test_peers() {
        let node1 = create_node("node1");
        let cluster = Cluster::new(node1, ClusterConfig::default());

        cluster.add_node(create_node("node2")).unwrap();
        cluster.add_node(create_node("node3")).unwrap();

        let peers = cluster.peers();
        assert_eq!(peers.len(), 2);

        let peer_ids = cluster.peer_ids();
        assert!(peer_ids.contains(&NodeId::new("node2")));
        assert!(peer_ids.contains(&NodeId::new("node3")));
        assert!(!peer_ids.contains(&NodeId::new("node1")));
    }

    #[test]
    fn test_cluster_stats() {
        let mut node1 = create_node("node1");
        node1.mark_healthy();

        let config = ClusterConfig::default().with_replication_factor(3);
        let cluster = Cluster::new(node1, config);

        let mut node2 = create_node("node2");
        node2.mark_healthy();
        cluster.add_node(node2).unwrap();

        let stats = cluster.stats();
        assert_eq!(stats.total_nodes, 2);
        assert_eq!(stats.healthy_nodes, 2);
        assert!(stats.has_quorum);
    }

    #[test]
    fn test_heartbeat() {
        let node1 = create_node("node1");
        let cluster = Cluster::new(node1, ClusterConfig::default());

        let mut node2 = create_node("node2");
        node2.status = NodeStatus::Suspect;
        cluster.add_node(node2).unwrap();

        cluster.heartbeat(&NodeId::new("node2"));

        let node = cluster.get_node(&NodeId::new("node2")).unwrap();
        assert_eq!(node.status, NodeStatus::Healthy);
    }

    #[test]
    fn test_leader_management() {
        let node1 = create_node("node1");
        let cluster = Cluster::new(node1, ClusterConfig::default());

        assert!(!cluster.is_leader());

        cluster.set_leader(Some(NodeId::new("node1")));
        assert!(cluster.is_leader());

        cluster.set_leader(Some(NodeId::new("node2")));
        assert!(!cluster.is_leader());
    }

    #[test]
    fn test_max_nodes() {
        let node1 = create_node("node1");
        let config = ClusterConfig {
            max_nodes: 2,
            ..Default::default()
        };
        let cluster = Cluster::new(node1, config);

        cluster.add_node(create_node("node2")).unwrap();
        let result = cluster.add_node(create_node("node3"));
        assert!(matches!(result, Err(ClusterError::MaxNodesReached)));
    }
}
