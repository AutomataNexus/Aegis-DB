//! Aegis Consistent Hashing
//!
//! Consistent hashing ring for distributed shard assignment.
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use crate::node::NodeId;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};

// =============================================================================
// Hash Ring
// =============================================================================

/// A consistent hash ring for distributing keys across nodes.
#[derive(Debug, Clone)]
pub struct HashRing {
    ring: BTreeMap<u64, VirtualNode>,
    virtual_nodes_per_node: usize,
    nodes: Vec<NodeId>,
}

impl HashRing {
    /// Create a new hash ring.
    pub fn new(virtual_nodes_per_node: usize) -> Self {
        Self {
            ring: BTreeMap::new(),
            virtual_nodes_per_node,
            nodes: Vec::new(),
        }
    }

    /// Create a hash ring with default settings (150 virtual nodes).
    pub fn default_ring() -> Self {
        Self::new(150)
    }

    /// Add a node to the ring.
    pub fn add_node(&mut self, node_id: NodeId) {
        if self.nodes.contains(&node_id) {
            return;
        }

        for i in 0..self.virtual_nodes_per_node {
            let vnode = VirtualNode {
                node_id: node_id.clone(),
                replica_index: i,
            };
            let hash = vnode.hash_position();
            self.ring.insert(hash, vnode);
        }

        self.nodes.push(node_id);
    }

    /// Remove a node from the ring.
    pub fn remove_node(&mut self, node_id: &NodeId) {
        self.ring.retain(|_, vnode| &vnode.node_id != node_id);
        self.nodes.retain(|n| n != node_id);
    }

    /// Get the node responsible for a key.
    pub fn get_node(&self, key: &str) -> Option<&NodeId> {
        if self.ring.is_empty() {
            return None;
        }

        let hash = hash_key(key);

        // Find the first node with hash >= key hash (clockwise traversal)
        if let Some((_, vnode)) = self.ring.range(hash..).next() {
            return Some(&vnode.node_id);
        }

        // Wrap around to the beginning
        self.ring.values().next().map(|vnode| &vnode.node_id)
    }

    /// Get N nodes for a key (for replication).
    pub fn get_nodes(&self, key: &str, count: usize) -> Vec<&NodeId> {
        if self.ring.is_empty() || count == 0 {
            return Vec::new();
        }

        let hash = hash_key(key);
        let mut result = Vec::with_capacity(count);
        let mut seen = std::collections::HashSet::new();

        // Iterate from the key's position clockwise
        for (_, vnode) in self.ring.range(hash..) {
            if seen.insert(&vnode.node_id) {
                result.push(&vnode.node_id);
                if result.len() >= count {
                    return result;
                }
            }
        }

        // Wrap around
        for (_, vnode) in self.ring.iter() {
            if seen.insert(&vnode.node_id) {
                result.push(&vnode.node_id);
                if result.len() >= count {
                    return result;
                }
            }
        }

        result
    }

    /// Get all nodes in the ring.
    pub fn nodes(&self) -> &[NodeId] {
        &self.nodes
    }

    /// Get the number of nodes.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Get the number of virtual nodes.
    pub fn virtual_node_count(&self) -> usize {
        self.ring.len()
    }

    /// Check if the ring is empty.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Get the hash position for a key.
    pub fn key_position(&self, key: &str) -> u64 {
        hash_key(key)
    }
}

impl Default for HashRing {
    fn default() -> Self {
        Self::default_ring()
    }
}

// =============================================================================
// Virtual Node
// =============================================================================

/// A virtual node in the hash ring.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VirtualNode {
    pub node_id: NodeId,
    pub replica_index: usize,
}

impl VirtualNode {
    /// Calculate the hash position for this virtual node.
    pub fn hash_position(&self) -> u64 {
        let key = format!("{}:{}", self.node_id.as_str(), self.replica_index);
        hash_key(&key)
    }
}

// =============================================================================
// Consistent Hash Trait
// =============================================================================

/// Trait for consistent hashing implementations.
pub trait ConsistentHash {
    /// Get the node for a key.
    fn route(&self, key: &str) -> Option<NodeId>;

    /// Get multiple nodes for a key.
    fn route_replicas(&self, key: &str, count: usize) -> Vec<NodeId>;

    /// Add a node.
    fn add(&mut self, node: NodeId);

    /// Remove a node.
    fn remove(&mut self, node: &NodeId);
}

impl ConsistentHash for HashRing {
    fn route(&self, key: &str) -> Option<NodeId> {
        self.get_node(key).cloned()
    }

    fn route_replicas(&self, key: &str, count: usize) -> Vec<NodeId> {
        self.get_nodes(key, count).into_iter().cloned().collect()
    }

    fn add(&mut self, node: NodeId) {
        self.add_node(node);
    }

    fn remove(&mut self, node: &NodeId) {
        self.remove_node(node);
    }
}

// =============================================================================
// Jump Consistent Hash
// =============================================================================

/// Jump consistent hash for fixed number of buckets.
pub struct JumpHash {
    num_buckets: u32,
}

impl JumpHash {
    /// Create a new jump hash.
    pub fn new(num_buckets: u32) -> Self {
        Self { num_buckets }
    }

    /// Get the bucket for a key.
    pub fn bucket(&self, key: &str) -> u32 {
        let hash = hash_key(key);
        jump_consistent_hash(hash, self.num_buckets)
    }

    /// Get the bucket for a u64 key.
    pub fn bucket_u64(&self, key: u64) -> u32 {
        jump_consistent_hash(key, self.num_buckets)
    }
}

/// Jump consistent hash algorithm.
fn jump_consistent_hash(mut key: u64, num_buckets: u32) -> u32 {
    let mut b: i64 = -1;
    let mut j: i64 = 0;

    while j < num_buckets as i64 {
        b = j;
        key = key.wrapping_mul(2862933555777941757).wrapping_add(1);
        j = ((b.wrapping_add(1) as f64) * (1i64 << 31) as f64
            / ((key >> 33).wrapping_add(1) as f64)) as i64;
    }

    b as u32
}

// =============================================================================
// Rendezvous Hash
// =============================================================================

/// Rendezvous (highest random weight) hashing.
pub struct RendezvousHash {
    nodes: Vec<NodeId>,
}

impl RendezvousHash {
    /// Create a new rendezvous hash.
    pub fn new() -> Self {
        Self { nodes: Vec::new() }
    }

    /// Add a node.
    pub fn add_node(&mut self, node: NodeId) {
        if !self.nodes.contains(&node) {
            self.nodes.push(node);
        }
    }

    /// Remove a node.
    pub fn remove_node(&mut self, node: &NodeId) {
        self.nodes.retain(|n| n != node);
    }

    /// Get the node for a key.
    pub fn get_node(&self, key: &str) -> Option<&NodeId> {
        self.nodes
            .iter()
            .max_by_key(|node| {
                let combined = format!("{}:{}", key, node.as_str());
                hash_key(&combined)
            })
    }

    /// Get N nodes for a key (sorted by weight).
    pub fn get_nodes(&self, key: &str, count: usize) -> Vec<&NodeId> {
        let mut weighted: Vec<_> = self
            .nodes
            .iter()
            .map(|node| {
                let combined = format!("{}:{}", key, node.as_str());
                (hash_key(&combined), node)
            })
            .collect();

        weighted.sort_by(|a, b| b.0.cmp(&a.0));

        weighted.into_iter().take(count).map(|(_, node)| node).collect()
    }
}

impl Default for RendezvousHash {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Hash Functions
// =============================================================================

/// Hash a key using xxHash-style algorithm.
fn hash_key(key: &str) -> u64 {
    let mut hasher = XxHasher::new();
    key.hash(&mut hasher);
    hasher.finish()
}

/// Simple xxHash-style hasher.
struct XxHasher {
    state: u64,
}

impl XxHasher {
    fn new() -> Self {
        Self {
            state: 0xcbf29ce484222325,
        }
    }
}

impl Hasher for XxHasher {
    fn finish(&self) -> u64 {
        self.state
    }

    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.state ^= *byte as u64;
            self.state = self.state.wrapping_mul(0x100000001b3);
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_ring_basic() {
        let mut ring = HashRing::new(10);

        ring.add_node(NodeId::new("node1"));
        ring.add_node(NodeId::new("node2"));
        ring.add_node(NodeId::new("node3"));

        assert_eq!(ring.node_count(), 3);
        assert_eq!(ring.virtual_node_count(), 30);
    }

    #[test]
    fn test_hash_ring_get_node() {
        let mut ring = HashRing::new(100);

        ring.add_node(NodeId::new("node1"));
        ring.add_node(NodeId::new("node2"));
        ring.add_node(NodeId::new("node3"));

        let node = ring.get_node("test_key").unwrap();
        assert!(["node1", "node2", "node3"].contains(&node.as_str()));

        // Same key should return same node
        let node2 = ring.get_node("test_key").unwrap();
        assert_eq!(node, node2);
    }

    #[test]
    fn test_hash_ring_distribution() {
        let mut ring = HashRing::new(150);

        ring.add_node(NodeId::new("node1"));
        ring.add_node(NodeId::new("node2"));
        ring.add_node(NodeId::new("node3"));

        let mut counts = std::collections::HashMap::new();

        for i in 0..1000 {
            let key = format!("key_{}", i);
            let node = ring.get_node(&key).unwrap();
            *counts.entry(node.as_str().to_string()).or_insert(0) += 1;
        }

        // Check that distribution is reasonably balanced
        for count in counts.values() {
            assert!(*count > 200, "Distribution too uneven: {:?}", counts);
            assert!(*count < 500, "Distribution too uneven: {:?}", counts);
        }
    }

    #[test]
    fn test_hash_ring_remove_node() {
        let mut ring = HashRing::new(10);

        ring.add_node(NodeId::new("node1"));
        ring.add_node(NodeId::new("node2"));

        let key = "test_key";
        let before = ring.get_node(key).unwrap().clone();

        ring.remove_node(&NodeId::new("node1"));
        assert_eq!(ring.node_count(), 1);

        let after = ring.get_node(key).unwrap();
        assert_eq!(after.as_str(), "node2");
    }

    #[test]
    fn test_hash_ring_get_replicas() {
        let mut ring = HashRing::new(50);

        ring.add_node(NodeId::new("node1"));
        ring.add_node(NodeId::new("node2"));
        ring.add_node(NodeId::new("node3"));

        let nodes = ring.get_nodes("test_key", 2);
        assert_eq!(nodes.len(), 2);
        assert_ne!(nodes[0], nodes[1]);
    }

    #[test]
    fn test_jump_hash() {
        let hash = JumpHash::new(10);

        let bucket1 = hash.bucket("key1");
        let bucket2 = hash.bucket("key1");

        assert_eq!(bucket1, bucket2);
        assert!(bucket1 < 10);
    }

    #[test]
    fn test_jump_hash_distribution() {
        let hash = JumpHash::new(5);
        let mut counts = vec![0; 5];

        for i in 0..1000 {
            let bucket = hash.bucket(&format!("key_{}", i)) as usize;
            counts[bucket] += 1;
        }

        // Check reasonably balanced
        for count in &counts {
            assert!(*count > 100, "Jump hash distribution uneven: {:?}", counts);
            assert!(*count < 300, "Jump hash distribution uneven: {:?}", counts);
        }
    }

    #[test]
    fn test_rendezvous_hash() {
        let mut hash = RendezvousHash::new();

        hash.add_node(NodeId::new("node1"));
        hash.add_node(NodeId::new("node2"));
        hash.add_node(NodeId::new("node3"));

        let node = hash.get_node("test_key").unwrap();
        assert!(["node1", "node2", "node3"].contains(&node.as_str()));

        // Same key should return same node
        let node2 = hash.get_node("test_key").unwrap();
        assert_eq!(node, node2);
    }

    #[test]
    fn test_rendezvous_get_multiple() {
        let mut hash = RendezvousHash::new();

        hash.add_node(NodeId::new("node1"));
        hash.add_node(NodeId::new("node2"));
        hash.add_node(NodeId::new("node3"));

        let nodes = hash.get_nodes("test_key", 2);
        assert_eq!(nodes.len(), 2);
        assert_ne!(nodes[0], nodes[1]);
    }

    #[test]
    fn test_consistent_hash_trait() {
        let mut ring = HashRing::new(50);

        ring.add(NodeId::new("node1"));
        ring.add(NodeId::new("node2"));

        let node = ring.route("key").unwrap();
        assert!(["node1", "node2"].contains(&node.as_str()));

        let replicas = ring.route_replicas("key", 2);
        assert_eq!(replicas.len(), 2);
    }
}
