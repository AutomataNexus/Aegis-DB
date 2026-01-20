//! Aegis Shard Management
//!
//! Shard lifecycle and state management.
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use crate::node::NodeId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::RwLock;
use std::time::{SystemTime, UNIX_EPOCH};

// =============================================================================
// Shard ID
// =============================================================================

/// Unique identifier for a shard.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ShardId(pub u32);

impl ShardId {
    pub fn new(id: u32) -> Self {
        Self(id)
    }

    pub fn as_u32(&self) -> u32 {
        self.0
    }
}

impl std::fmt::Display for ShardId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "shard_{}", self.0)
    }
}

// =============================================================================
// Shard State
// =============================================================================

/// State of a shard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShardState {
    /// Shard is being created.
    Creating,
    /// Shard is active and serving requests.
    Active,
    /// Shard is being migrated to another node.
    Migrating,
    /// Shard is being split into smaller shards.
    Splitting,
    /// Shard is being merged with another shard.
    Merging,
    /// Shard is inactive (not serving requests).
    Inactive,
    /// Shard is being deleted.
    Deleting,
}

impl Default for ShardState {
    fn default() -> Self {
        Self::Creating
    }
}

// =============================================================================
// Shard
// =============================================================================

/// A data shard in the distributed system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Shard {
    pub id: ShardId,
    pub state: ShardState,
    pub primary_node: NodeId,
    pub replica_nodes: Vec<NodeId>,
    pub key_range_start: Option<u64>,
    pub key_range_end: Option<u64>,
    pub created_at: u64,
    pub updated_at: u64,
    pub size_bytes: u64,
    pub row_count: u64,
    pub metadata: HashMap<String, String>,
}

impl Shard {
    /// Create a new shard.
    pub fn new(id: ShardId, primary_node: NodeId) -> Self {
        let now = current_timestamp();
        Self {
            id,
            state: ShardState::Creating,
            primary_node,
            replica_nodes: Vec::new(),
            key_range_start: None,
            key_range_end: None,
            created_at: now,
            updated_at: now,
            size_bytes: 0,
            row_count: 0,
            metadata: HashMap::new(),
        }
    }

    /// Create a shard with a key range.
    pub fn with_range(id: ShardId, primary_node: NodeId, start: u64, end: u64) -> Self {
        let mut shard = Self::new(id, primary_node);
        shard.key_range_start = Some(start);
        shard.key_range_end = Some(end);
        shard
    }

    /// Add a replica node.
    pub fn add_replica(&mut self, node: NodeId) {
        if !self.replica_nodes.contains(&node) && node != self.primary_node {
            self.replica_nodes.push(node);
            self.updated_at = current_timestamp();
        }
    }

    /// Remove a replica node.
    pub fn remove_replica(&mut self, node: &NodeId) {
        self.replica_nodes.retain(|n| n != node);
        self.updated_at = current_timestamp();
    }

    /// Set the primary node.
    pub fn set_primary(&mut self, node: NodeId) {
        self.primary_node = node;
        self.updated_at = current_timestamp();
    }

    /// Set the shard state.
    pub fn set_state(&mut self, state: ShardState) {
        self.state = state;
        self.updated_at = current_timestamp();
    }

    /// Check if a key is in this shard's range.
    pub fn contains_key(&self, key_hash: u64) -> bool {
        match (self.key_range_start, self.key_range_end) {
            (Some(start), Some(end)) => key_hash >= start && key_hash < end,
            _ => true, // No range defined, accepts all keys
        }
    }

    /// Check if the shard is active.
    pub fn is_active(&self) -> bool {
        self.state == ShardState::Active
    }

    /// Check if the shard is available for reads.
    pub fn is_readable(&self) -> bool {
        matches!(
            self.state,
            ShardState::Active | ShardState::Migrating | ShardState::Splitting
        )
    }

    /// Check if the shard is available for writes.
    pub fn is_writable(&self) -> bool {
        self.state == ShardState::Active
    }

    /// Get all nodes (primary + replicas).
    pub fn all_nodes(&self) -> Vec<&NodeId> {
        let mut nodes = vec![&self.primary_node];
        nodes.extend(self.replica_nodes.iter());
        nodes
    }

    /// Get the replication factor (1 + replica count).
    pub fn replication_factor(&self) -> usize {
        1 + self.replica_nodes.len()
    }

    /// Update size statistics.
    pub fn update_stats(&mut self, size_bytes: u64, row_count: u64) {
        self.size_bytes = size_bytes;
        self.row_count = row_count;
        self.updated_at = current_timestamp();
    }
}

// =============================================================================
// Shard Manager
// =============================================================================

/// Manages shards across the cluster.
pub struct ShardManager {
    shards: RwLock<HashMap<ShardId, Shard>>,
    node_shards: RwLock<HashMap<NodeId, Vec<ShardId>>>,
    num_shards: u32,
    replication_factor: usize,
}

impl ShardManager {
    /// Create a new shard manager.
    pub fn new(num_shards: u32, replication_factor: usize) -> Self {
        Self {
            shards: RwLock::new(HashMap::new()),
            node_shards: RwLock::new(HashMap::new()),
            num_shards,
            replication_factor,
        }
    }

    /// Get the number of shards.
    pub fn num_shards(&self) -> u32 {
        self.num_shards
    }

    /// Get the replication factor.
    pub fn replication_factor(&self) -> usize {
        self.replication_factor
    }

    /// Create a shard.
    pub fn create_shard(&self, id: ShardId, primary_node: NodeId) -> Shard {
        let shard = Shard::new(id.clone(), primary_node.clone());

        let mut shards = self.shards.write().unwrap();
        shards.insert(id.clone(), shard.clone());

        let mut node_shards = self.node_shards.write().unwrap();
        node_shards
            .entry(primary_node)
            .or_insert_with(Vec::new)
            .push(id);

        shard
    }

    /// Create a shard with a key range.
    pub fn create_shard_with_range(
        &self,
        id: ShardId,
        primary_node: NodeId,
        start: u64,
        end: u64,
    ) -> Shard {
        let shard = Shard::with_range(id.clone(), primary_node.clone(), start, end);

        let mut shards = self.shards.write().unwrap();
        shards.insert(id.clone(), shard.clone());

        let mut node_shards = self.node_shards.write().unwrap();
        node_shards
            .entry(primary_node)
            .or_insert_with(Vec::new)
            .push(id);

        shard
    }

    /// Get a shard by ID.
    pub fn get_shard(&self, id: &ShardId) -> Option<Shard> {
        self.shards.read().unwrap().get(id).cloned()
    }

    /// Get all shards.
    pub fn get_all_shards(&self) -> Vec<Shard> {
        self.shards.read().unwrap().values().cloned().collect()
    }

    /// Get shards for a node.
    pub fn get_node_shards(&self, node_id: &NodeId) -> Vec<ShardId> {
        self.node_shards
            .read()
            .unwrap()
            .get(node_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Update shard state.
    pub fn update_shard_state(&self, id: &ShardId, state: ShardState) -> bool {
        let mut shards = self.shards.write().unwrap();
        if let Some(shard) = shards.get_mut(id) {
            shard.set_state(state);
            true
        } else {
            false
        }
    }

    /// Add a replica to a shard.
    pub fn add_replica(&self, shard_id: &ShardId, node_id: NodeId) -> bool {
        let mut shards = self.shards.write().unwrap();
        if let Some(shard) = shards.get_mut(shard_id) {
            shard.add_replica(node_id.clone());

            let mut node_shards = self.node_shards.write().unwrap();
            node_shards
                .entry(node_id)
                .or_insert_with(Vec::new)
                .push(shard_id.clone());

            true
        } else {
            false
        }
    }

    /// Remove a shard.
    pub fn remove_shard(&self, id: &ShardId) -> Option<Shard> {
        let mut shards = self.shards.write().unwrap();
        let shard = shards.remove(id)?;

        let mut node_shards = self.node_shards.write().unwrap();

        // Remove from primary
        if let Some(shards) = node_shards.get_mut(&shard.primary_node) {
            shards.retain(|s| s != id);
        }

        // Remove from replicas
        for replica in &shard.replica_nodes {
            if let Some(shards) = node_shards.get_mut(replica) {
                shards.retain(|s| s != id);
            }
        }

        Some(shard)
    }

    /// Get the shard for a key hash.
    pub fn get_shard_for_key(&self, key_hash: u64) -> Option<Shard> {
        let shards = self.shards.read().unwrap();

        // Simple modulo-based shard selection
        let shard_id = ShardId::new((key_hash % self.num_shards as u64) as u32);
        shards.get(&shard_id).cloned()
    }

    /// Get active shards.
    pub fn get_active_shards(&self) -> Vec<Shard> {
        self.shards
            .read()
            .unwrap()
            .values()
            .filter(|s| s.is_active())
            .cloned()
            .collect()
    }

    /// Get shard count.
    pub fn shard_count(&self) -> usize {
        self.shards.read().unwrap().len()
    }

    /// Initialize shards for a set of nodes.
    pub fn initialize_shards(&self, nodes: &[NodeId]) {
        if nodes.is_empty() {
            return;
        }

        let range_size = u64::MAX / self.num_shards as u64;

        for i in 0..self.num_shards {
            let shard_id = ShardId::new(i);
            let primary_idx = i as usize % nodes.len();
            let primary_node = nodes[primary_idx].clone();

            let start = i as u64 * range_size;
            let end = if i == self.num_shards - 1 {
                u64::MAX
            } else {
                (i as u64 + 1) * range_size
            };

            let mut shard = self.create_shard_with_range(shard_id.clone(), primary_node, start, end);

            // Add replicas
            for r in 1..self.replication_factor.min(nodes.len()) {
                let replica_idx = (primary_idx + r) % nodes.len();
                self.add_replica(&shard_id, nodes[replica_idx].clone());
            }

            // Activate the shard
            self.update_shard_state(&shard_id, ShardState::Active);
        }
    }

    /// Get shard statistics.
    pub fn stats(&self) -> ShardManagerStats {
        let shards = self.shards.read().unwrap();

        let active = shards.values().filter(|s| s.is_active()).count();
        let migrating = shards
            .values()
            .filter(|s| s.state == ShardState::Migrating)
            .count();
        let total_size: u64 = shards.values().map(|s| s.size_bytes).sum();
        let total_rows: u64 = shards.values().map(|s| s.row_count).sum();

        ShardManagerStats {
            total_shards: shards.len(),
            active_shards: active,
            migrating_shards: migrating,
            total_size_bytes: total_size,
            total_row_count: total_rows,
        }
    }
}

// =============================================================================
// Shard Manager Statistics
// =============================================================================

/// Statistics for the shard manager.
#[derive(Debug, Clone)]
pub struct ShardManagerStats {
    pub total_shards: usize,
    pub active_shards: usize,
    pub migrating_shards: usize,
    pub total_size_bytes: u64,
    pub total_row_count: u64,
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
    fn test_shard_id() {
        let id = ShardId::new(5);
        assert_eq!(id.as_u32(), 5);
        assert_eq!(id.to_string(), "shard_5");
    }

    #[test]
    fn test_shard_creation() {
        let shard = Shard::new(ShardId::new(0), NodeId::new("node1"));

        assert_eq!(shard.id.as_u32(), 0);
        assert_eq!(shard.primary_node.as_str(), "node1");
        assert_eq!(shard.state, ShardState::Creating);
        assert!(shard.replica_nodes.is_empty());
    }

    #[test]
    fn test_shard_with_range() {
        let shard = Shard::with_range(ShardId::new(0), NodeId::new("node1"), 0, 1000);

        assert!(shard.contains_key(500));
        assert!(!shard.contains_key(1000));
    }

    #[test]
    fn test_shard_replicas() {
        let mut shard = Shard::new(ShardId::new(0), NodeId::new("node1"));

        shard.add_replica(NodeId::new("node2"));
        shard.add_replica(NodeId::new("node3"));

        assert_eq!(shard.replica_nodes.len(), 2);
        assert_eq!(shard.replication_factor(), 3);
        assert_eq!(shard.all_nodes().len(), 3);
    }

    #[test]
    fn test_shard_state() {
        let mut shard = Shard::new(ShardId::new(0), NodeId::new("node1"));

        assert!(!shard.is_active());

        shard.set_state(ShardState::Active);
        assert!(shard.is_active());
        assert!(shard.is_readable());
        assert!(shard.is_writable());

        shard.set_state(ShardState::Migrating);
        assert!(shard.is_readable());
        assert!(!shard.is_writable());
    }

    #[test]
    fn test_shard_manager_creation() {
        let manager = ShardManager::new(16, 3);

        assert_eq!(manager.num_shards(), 16);
        assert_eq!(manager.replication_factor(), 3);
    }

    #[test]
    fn test_shard_manager_create_shard() {
        let manager = ShardManager::new(16, 3);

        let shard = manager.create_shard(ShardId::new(0), NodeId::new("node1"));

        assert_eq!(manager.shard_count(), 1);

        let retrieved = manager.get_shard(&ShardId::new(0)).unwrap();
        assert_eq!(retrieved.primary_node.as_str(), "node1");
    }

    #[test]
    fn test_shard_manager_initialize() {
        let manager = ShardManager::new(8, 2);
        let nodes = vec![
            NodeId::new("node1"),
            NodeId::new("node2"),
            NodeId::new("node3"),
        ];

        manager.initialize_shards(&nodes);

        assert_eq!(manager.shard_count(), 8);

        let active = manager.get_active_shards();
        assert_eq!(active.len(), 8);

        // Check distribution
        let node1_shards = manager.get_node_shards(&NodeId::new("node1"));
        let node2_shards = manager.get_node_shards(&NodeId::new("node2"));
        let node3_shards = manager.get_node_shards(&NodeId::new("node3"));

        assert!(!node1_shards.is_empty());
        assert!(!node2_shards.is_empty());
        assert!(!node3_shards.is_empty());
    }

    #[test]
    fn test_shard_manager_stats() {
        let manager = ShardManager::new(4, 2);
        let nodes = vec![NodeId::new("node1"), NodeId::new("node2")];

        manager.initialize_shards(&nodes);

        let stats = manager.stats();
        assert_eq!(stats.total_shards, 4);
        assert_eq!(stats.active_shards, 4);
    }
}
