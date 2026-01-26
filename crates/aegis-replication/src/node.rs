//! Aegis Replication Node
//!
//! Node identification and management for distributed clusters.
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::time::{SystemTime, UNIX_EPOCH};

// =============================================================================
// Node ID
// =============================================================================

/// Unique identifier for a node in the cluster.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub String);

impl NodeId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn generate() -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        Self(format!("node_{:016x}", timestamp as u64))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for NodeId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for NodeId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

// =============================================================================
// Node Status
// =============================================================================

/// Status of a node in the cluster.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[derive(Default)]
pub enum NodeStatus {
    /// Node is starting up.
    #[default]
    Starting,
    /// Node is healthy and operational.
    Healthy,
    /// Node is suspected to be failing.
    Suspect,
    /// Node is confirmed down.
    Down,
    /// Node is being removed from cluster.
    Leaving,
    /// Node has left the cluster.
    Left,
}


// =============================================================================
// Node Role
// =============================================================================

/// Role of a node in the Raft cluster.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[derive(Default)]
pub enum NodeRole {
    /// Follower node that replicates from leader.
    #[default]
    Follower,
    /// Candidate node during leader election.
    Candidate,
    /// Leader node that handles writes.
    Leader,
}


// =============================================================================
// Node Info
// =============================================================================

/// Information about a node in the cluster.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    pub id: NodeId,
    pub address: String,
    pub port: u16,
    pub status: NodeStatus,
    pub role: NodeRole,
    pub joined_at: u64,
    pub last_heartbeat: u64,
    pub metadata: NodeMetadata,
}

impl NodeInfo {
    /// Create a new node info.
    pub fn new(id: impl Into<NodeId>, address: impl Into<String>, port: u16) -> Self {
        let now = current_timestamp();
        Self {
            id: id.into(),
            address: address.into(),
            port,
            status: NodeStatus::Starting,
            role: NodeRole::Follower,
            joined_at: now,
            last_heartbeat: now,
            metadata: NodeMetadata::default(),
        }
    }

    /// Get the socket address.
    pub fn socket_addr(&self) -> Option<SocketAddr> {
        format!("{}:{}", self.address, self.port).parse().ok()
    }

    /// Update the heartbeat timestamp.
    pub fn heartbeat(&mut self) {
        self.last_heartbeat = current_timestamp();
        if self.status == NodeStatus::Suspect {
            self.status = NodeStatus::Healthy;
        }
    }

    /// Mark the node as healthy.
    pub fn mark_healthy(&mut self) {
        self.status = NodeStatus::Healthy;
        self.heartbeat();
    }

    /// Mark the node as suspect.
    pub fn mark_suspect(&mut self) {
        if self.status == NodeStatus::Healthy {
            self.status = NodeStatus::Suspect;
        }
    }

    /// Mark the node as down.
    pub fn mark_down(&mut self) {
        self.status = NodeStatus::Down;
    }

    /// Check if the node is available for requests.
    pub fn is_available(&self) -> bool {
        matches!(self.status, NodeStatus::Healthy)
    }

    /// Check if the node is the leader.
    pub fn is_leader(&self) -> bool {
        self.role == NodeRole::Leader
    }

    /// Time since last heartbeat in milliseconds.
    pub fn heartbeat_age(&self) -> u64 {
        current_timestamp().saturating_sub(self.last_heartbeat)
    }
}

// =============================================================================
// Node Metadata
// =============================================================================

/// Additional metadata about a node.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NodeMetadata {
    pub datacenter: Option<String>,
    pub rack: Option<String>,
    pub zone: Option<String>,
    pub tags: Vec<String>,
    pub capacity: Option<NodeCapacity>,
}

impl NodeMetadata {
    pub fn with_datacenter(mut self, dc: impl Into<String>) -> Self {
        self.datacenter = Some(dc.into());
        self
    }

    pub fn with_rack(mut self, rack: impl Into<String>) -> Self {
        self.rack = Some(rack.into());
        self
    }

    pub fn with_zone(mut self, zone: impl Into<String>) -> Self {
        self.zone = Some(zone.into());
        self
    }

    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }
}

// =============================================================================
// Node Capacity
// =============================================================================

/// Capacity information for a node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeCapacity {
    pub cpu_cores: u32,
    pub memory_mb: u64,
    pub storage_gb: u64,
    pub network_mbps: u32,
}

impl NodeCapacity {
    pub fn new(cpu_cores: u32, memory_mb: u64, storage_gb: u64) -> Self {
        Self {
            cpu_cores,
            memory_mb,
            storage_gb,
            network_mbps: 1000,
        }
    }
}

// =============================================================================
// Node Health
// =============================================================================

/// Health check result for a node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeHealth {
    pub node_id: NodeId,
    pub healthy: bool,
    pub cpu_usage: f64,
    pub memory_usage: f64,
    pub disk_usage: f64,
    pub latency_ms: u64,
    pub checked_at: u64,
}

impl NodeHealth {
    pub fn healthy(node_id: NodeId) -> Self {
        Self {
            node_id,
            healthy: true,
            cpu_usage: 0.0,
            memory_usage: 0.0,
            disk_usage: 0.0,
            latency_ms: 0,
            checked_at: current_timestamp(),
        }
    }

    pub fn unhealthy(node_id: NodeId) -> Self {
        Self {
            node_id,
            healthy: false,
            cpu_usage: 0.0,
            memory_usage: 0.0,
            disk_usage: 0.0,
            latency_ms: 0,
            checked_at: current_timestamp(),
        }
    }
}

fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_id() {
        let id1 = NodeId::generate();
        let id2 = NodeId::generate();
        assert_ne!(id1, id2);
        assert!(id1.as_str().starts_with("node_"));
    }

    #[test]
    fn test_node_info() {
        let mut node = NodeInfo::new("node1", "127.0.0.1", 5432);

        assert_eq!(node.status, NodeStatus::Starting);
        assert_eq!(node.role, NodeRole::Follower);

        node.mark_healthy();
        assert_eq!(node.status, NodeStatus::Healthy);
        assert!(node.is_available());

        node.mark_suspect();
        assert_eq!(node.status, NodeStatus::Suspect);

        node.heartbeat();
        assert_eq!(node.status, NodeStatus::Healthy);
    }

    #[test]
    fn test_socket_addr() {
        let node = NodeInfo::new("node1", "127.0.0.1", 5432);
        let addr = node.socket_addr().unwrap();
        assert_eq!(addr.port(), 5432);
    }

    #[test]
    fn test_node_metadata() {
        let metadata = NodeMetadata::default()
            .with_datacenter("us-east-1")
            .with_zone("a")
            .with_tag("production");

        assert_eq!(metadata.datacenter, Some("us-east-1".to_string()));
        assert_eq!(metadata.zone, Some("a".to_string()));
        assert!(metadata.tags.contains(&"production".to_string()));
    }

    #[test]
    fn test_node_health() {
        let health = NodeHealth::healthy(NodeId::new("node1"));
        assert!(health.healthy);

        let unhealthy = NodeHealth::unhealthy(NodeId::new("node2"));
        assert!(!unhealthy.healthy);
    }
}
