//! Aegis Shard Router
//!
//! Query routing to appropriate shards.
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use crate::hash::{ConsistentHash, HashRing};
use crate::node::NodeId;
use crate::partition::{PartitionKey, PartitionStrategy};
use crate::shard::{Shard, ShardId, ShardManager};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::RwLock;

// =============================================================================
// Route Decision
// =============================================================================

/// A routing decision for a query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RouteDecision {
    /// Route to a single shard.
    Single {
        shard_id: ShardId,
        node_id: NodeId,
    },
    /// Route to multiple shards (scatter-gather).
    Multi {
        routes: Vec<ShardRoute>,
    },
    /// Route to all shards (broadcast).
    Broadcast {
        shards: Vec<ShardId>,
    },
    /// Route to primary only.
    Primary {
        shard_id: ShardId,
        node_id: NodeId,
    },
    /// Route to any replica (for read queries).
    AnyReplica {
        shard_id: ShardId,
        candidates: Vec<NodeId>,
    },
}

/// A route to a specific shard.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShardRoute {
    pub shard_id: ShardId,
    pub node_id: NodeId,
    pub is_primary: bool,
}

// =============================================================================
// Routing Table
// =============================================================================

/// A routing table for shard lookups.
#[derive(Debug, Clone)]
pub struct RoutingTable {
    entries: HashMap<ShardId, RoutingEntry>,
    version: u64,
}

/// An entry in the routing table.
#[derive(Debug, Clone)]
pub struct RoutingEntry {
    pub shard_id: ShardId,
    pub primary: NodeId,
    pub replicas: Vec<NodeId>,
    pub key_range_start: u64,
    pub key_range_end: u64,
}

impl RoutingTable {
    /// Create a new routing table.
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            version: 0,
        }
    }

    /// Add or update an entry.
    pub fn upsert(&mut self, entry: RoutingEntry) {
        self.entries.insert(entry.shard_id.clone(), entry);
        self.version += 1;
    }

    /// Remove an entry.
    pub fn remove(&mut self, shard_id: &ShardId) -> Option<RoutingEntry> {
        let entry = self.entries.remove(shard_id);
        if entry.is_some() {
            self.version += 1;
        }
        entry
    }

    /// Get an entry.
    pub fn get(&self, shard_id: &ShardId) -> Option<&RoutingEntry> {
        self.entries.get(shard_id)
    }

    /// Find the shard for a key hash.
    pub fn find_shard(&self, key_hash: u64) -> Option<&RoutingEntry> {
        self.entries
            .values()
            .find(|e| key_hash >= e.key_range_start && key_hash < e.key_range_end)
    }

    /// Get all entries.
    pub fn all_entries(&self) -> impl Iterator<Item = &RoutingEntry> {
        self.entries.values()
    }

    /// Get the table version.
    pub fn version(&self) -> u64 {
        self.version
    }

    /// Get the number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the table is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Build from a shard manager.
    pub fn from_shards(shards: &[Shard]) -> Self {
        let mut table = Self::new();

        for shard in shards {
            table.upsert(RoutingEntry {
                shard_id: shard.id.clone(),
                primary: shard.primary_node.clone(),
                replicas: shard.replica_nodes.clone(),
                key_range_start: shard.key_range_start.unwrap_or(0),
                key_range_end: shard.key_range_end.unwrap_or(u64::MAX),
            });
        }

        table
    }
}

impl Default for RoutingTable {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Shard Router
// =============================================================================

/// Routes queries to appropriate shards.
pub struct ShardRouter {
    routing_table: RwLock<RoutingTable>,
    hash_ring: RwLock<HashRing>,
    partition_strategy: PartitionStrategy,
    prefer_local: bool,
    local_node: Option<NodeId>,
}

impl ShardRouter {
    /// Create a new shard router.
    pub fn new(strategy: PartitionStrategy) -> Self {
        Self {
            routing_table: RwLock::new(RoutingTable::new()),
            hash_ring: RwLock::new(HashRing::default()),
            partition_strategy: strategy,
            prefer_local: true,
            local_node: None,
        }
    }

    /// Create a router with a local node preference.
    pub fn with_local_node(strategy: PartitionStrategy, local_node: NodeId) -> Self {
        Self {
            routing_table: RwLock::new(RoutingTable::new()),
            hash_ring: RwLock::new(HashRing::default()),
            partition_strategy: strategy,
            prefer_local: true,
            local_node: Some(local_node),
        }
    }

    /// Update the routing table from shards.
    pub fn update_routing(&self, shards: &[Shard]) {
        let table = RoutingTable::from_shards(shards);
        *self.routing_table.write().unwrap() = table;

        // Update hash ring
        let mut ring = self.hash_ring.write().unwrap();
        *ring = HashRing::default();
        for shard in shards {
            ring.add_node(shard.primary_node.clone());
            for replica in &shard.replica_nodes {
                ring.add_node(replica.clone());
            }
        }
    }

    /// Route a query with a partition key.
    pub fn route(&self, key: &PartitionKey) -> RouteDecision {
        let hash = key.hash_value();
        let table = self.routing_table.read().unwrap();

        if let Some(entry) = table.find_shard(hash) {
            RouteDecision::Single {
                shard_id: entry.shard_id.clone(),
                node_id: self.select_node(entry),
            }
        } else {
            // Fallback to hash ring
            let ring = self.hash_ring.read().unwrap();
            if let Some(node) = ring.get_node(&format!("{}", hash)) {
                RouteDecision::Single {
                    shard_id: ShardId::new(0),
                    node_id: node.clone(),
                }
            } else {
                RouteDecision::Broadcast {
                    shards: table.entries.keys().cloned().collect(),
                }
            }
        }
    }

    /// Route for a write operation (always to primary).
    pub fn route_write(&self, key: &PartitionKey) -> RouteDecision {
        let hash = key.hash_value();
        let table = self.routing_table.read().unwrap();

        if let Some(entry) = table.find_shard(hash) {
            RouteDecision::Primary {
                shard_id: entry.shard_id.clone(),
                node_id: entry.primary.clone(),
            }
        } else {
            RouteDecision::Broadcast {
                shards: table.entries.keys().cloned().collect(),
            }
        }
    }

    /// Route for a read operation (can use replicas).
    pub fn route_read(&self, key: &PartitionKey) -> RouteDecision {
        let hash = key.hash_value();
        let table = self.routing_table.read().unwrap();

        if let Some(entry) = table.find_shard(hash) {
            let mut candidates = vec![entry.primary.clone()];
            candidates.extend(entry.replicas.iter().cloned());

            RouteDecision::AnyReplica {
                shard_id: entry.shard_id.clone(),
                candidates,
            }
        } else {
            self.route(key)
        }
    }

    /// Route to multiple shards for a range query.
    pub fn route_range(&self, start_key: &PartitionKey, end_key: &PartitionKey) -> RouteDecision {
        let start_hash = start_key.hash_value();
        let end_hash = end_key.hash_value();

        let table = self.routing_table.read().unwrap();
        let mut routes = Vec::new();

        for entry in table.all_entries() {
            // Check if shard overlaps with query range
            if entry.key_range_end > start_hash && entry.key_range_start < end_hash {
                routes.push(ShardRoute {
                    shard_id: entry.shard_id.clone(),
                    node_id: self.select_node(entry),
                    is_primary: true,
                });
            }
        }

        if routes.is_empty() {
            RouteDecision::Broadcast {
                shards: table.entries.keys().cloned().collect(),
            }
        } else if routes.len() == 1 {
            let route = routes.remove(0);
            RouteDecision::Single {
                shard_id: route.shard_id,
                node_id: route.node_id,
            }
        } else {
            RouteDecision::Multi { routes }
        }
    }

    /// Route to all shards (for queries without partition key).
    pub fn route_all(&self) -> RouteDecision {
        let table = self.routing_table.read().unwrap();

        let routes: Vec<_> = table
            .all_entries()
            .map(|entry| ShardRoute {
                shard_id: entry.shard_id.clone(),
                node_id: self.select_node(entry),
                is_primary: true,
            })
            .collect();

        if routes.is_empty() {
            RouteDecision::Broadcast { shards: vec![] }
        } else {
            RouteDecision::Multi { routes }
        }
    }

    /// Select a node for a routing entry.
    fn select_node(&self, entry: &RoutingEntry) -> NodeId {
        // Prefer local node if available
        if self.prefer_local {
            if let Some(ref local) = self.local_node {
                if &entry.primary == local {
                    return entry.primary.clone();
                }
                if entry.replicas.contains(local) {
                    return local.clone();
                }
            }
        }

        // Default to primary
        entry.primary.clone()
    }

    /// Get the current routing table version.
    pub fn routing_version(&self) -> u64 {
        self.routing_table.read().unwrap().version()
    }

    /// Get the partition strategy.
    pub fn strategy(&self) -> &PartitionStrategy {
        &self.partition_strategy
    }

    /// Check if routing table is initialized.
    pub fn is_initialized(&self) -> bool {
        !self.routing_table.read().unwrap().is_empty()
    }
}

// =============================================================================
// Query Analyzer
// =============================================================================

/// Analyzes queries to extract partition keys.
pub struct QueryAnalyzer {
    partition_columns: Vec<String>,
}

impl QueryAnalyzer {
    /// Create a new query analyzer.
    pub fn new(partition_columns: Vec<String>) -> Self {
        Self { partition_columns }
    }

    /// Analyze a query to extract routing information.
    pub fn analyze(&self, query: &str) -> QueryRouting {
        let query_upper = query.to_uppercase();

        // Determine query type
        let query_type = if query_upper.starts_with("SELECT") {
            QueryType::Read
        } else if query_upper.starts_with("INSERT") {
            QueryType::Write
        } else if query_upper.starts_with("UPDATE") {
            QueryType::Write
        } else if query_upper.starts_with("DELETE") {
            QueryType::Write
        } else {
            QueryType::Admin
        };

        // Try to extract partition key from WHERE clause
        let partition_key = self.extract_partition_key(query);
        let requires_all_shards = partition_key.is_none() && query_type == QueryType::Read;

        QueryRouting {
            query_type,
            partition_key,
            requires_all_shards,
        }
    }

    fn extract_partition_key(&self, query: &str) -> Option<PartitionKey> {
        let query_lower = query.to_lowercase();

        for col in &self.partition_columns {
            let col_lower = col.to_lowercase();

            // Look for "column = 'value'" or "column = value"
            let patterns = [
                format!("{} = '", col_lower),
                format!("{} ='", col_lower),
                format!("{}='", col_lower),
                format!("{} = ", col_lower),
            ];

            for pattern in &patterns {
                if let Some(start) = query_lower.find(pattern) {
                    let value_start = start + pattern.len();
                    let remaining = &query[value_start..];

                    let value = if remaining.starts_with('\'') {
                        // String value
                        remaining[1..]
                            .split('\'')
                            .next()
                            .map(|s| s.to_string())
                    } else {
                        // Numeric or unquoted
                        remaining
                            .split(|c: char| c.is_whitespace() || c == ')' || c == ';')
                            .next()
                            .map(|s| s.trim_matches('\'').to_string())
                    };

                    if let Some(v) = value {
                        if !v.is_empty() {
                            // Try to parse as integer
                            if let Ok(i) = v.parse::<i64>() {
                                return Some(PartitionKey::Int(i));
                            }
                            return Some(PartitionKey::String(v));
                        }
                    }
                }
            }
        }

        None
    }
}

/// Query routing information.
#[derive(Debug, Clone)]
pub struct QueryRouting {
    pub query_type: QueryType,
    pub partition_key: Option<PartitionKey>,
    pub requires_all_shards: bool,
}

/// Type of query.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryType {
    Read,
    Write,
    Admin,
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_shards() -> Vec<Shard> {
        vec![
            Shard::with_range(
                ShardId::new(0),
                NodeId::new("node1"),
                0,
                u64::MAX / 2,
            ),
            Shard::with_range(
                ShardId::new(1),
                NodeId::new("node2"),
                u64::MAX / 2,
                u64::MAX,
            ),
        ]
    }

    #[test]
    fn test_routing_table() {
        let shards = create_test_shards();
        let table = RoutingTable::from_shards(&shards);

        assert_eq!(table.len(), 2);
        assert!(table.get(&ShardId::new(0)).is_some());
    }

    #[test]
    fn test_routing_table_find_shard() {
        let shards = create_test_shards();
        let table = RoutingTable::from_shards(&shards);

        let entry1 = table.find_shard(100).unwrap();
        assert_eq!(entry1.shard_id.as_u32(), 0);

        let entry2 = table.find_shard(u64::MAX - 100).unwrap();
        assert_eq!(entry2.shard_id.as_u32(), 1);
    }

    #[test]
    fn test_shard_router() {
        let strategy = PartitionStrategy::hash(vec!["id".to_string()], 2);
        let router = ShardRouter::new(strategy);

        let shards = create_test_shards();
        router.update_routing(&shards);

        assert!(router.is_initialized());
    }

    #[test]
    fn test_router_route() {
        let strategy = PartitionStrategy::hash(vec!["id".to_string()], 2);
        let router = ShardRouter::new(strategy);

        let shards = create_test_shards();
        router.update_routing(&shards);

        let key = PartitionKey::string("test_key");
        let decision = router.route(&key);

        match decision {
            RouteDecision::Single { shard_id, node_id } => {
                assert!(shard_id.as_u32() < 2);
                assert!(["node1", "node2"].contains(&node_id.as_str()));
            }
            _ => panic!("Expected single route"),
        }
    }

    #[test]
    fn test_router_route_write() {
        let strategy = PartitionStrategy::hash(vec!["id".to_string()], 2);
        let router = ShardRouter::new(strategy);

        let mut shards = create_test_shards();
        shards[0].add_replica(NodeId::new("node3"));
        router.update_routing(&shards);

        let key = PartitionKey::int(1);
        let decision = router.route_write(&key);

        match decision {
            RouteDecision::Primary { node_id, .. } => {
                // Should route to primary, not replica
                assert!(["node1", "node2"].contains(&node_id.as_str()));
            }
            _ => panic!("Expected primary route"),
        }
    }

    #[test]
    fn test_router_route_read() {
        let strategy = PartitionStrategy::hash(vec!["id".to_string()], 2);
        let router = ShardRouter::new(strategy);

        let mut shards = create_test_shards();
        shards[0].add_replica(NodeId::new("node3"));
        router.update_routing(&shards);

        let key = PartitionKey::int(1);
        let decision = router.route_read(&key);

        match decision {
            RouteDecision::AnyReplica { candidates, .. } => {
                assert!(!candidates.is_empty());
            }
            _ => panic!("Expected any replica route"),
        }
    }

    #[test]
    fn test_router_route_all() {
        let strategy = PartitionStrategy::hash(vec!["id".to_string()], 2);
        let router = ShardRouter::new(strategy);

        let shards = create_test_shards();
        router.update_routing(&shards);

        let decision = router.route_all();

        match decision {
            RouteDecision::Multi { routes } => {
                assert_eq!(routes.len(), 2);
            }
            _ => panic!("Expected multi route"),
        }
    }

    #[test]
    fn test_query_analyzer() {
        let analyzer = QueryAnalyzer::new(vec!["user_id".to_string()]);

        let routing = analyzer.analyze("SELECT * FROM users WHERE user_id = 123");
        assert_eq!(routing.query_type, QueryType::Read);
        assert!(routing.partition_key.is_some());

        let routing = analyzer.analyze("INSERT INTO users VALUES (1, 'Alice')");
        assert_eq!(routing.query_type, QueryType::Write);
    }

    #[test]
    fn test_query_analyzer_no_key() {
        let analyzer = QueryAnalyzer::new(vec!["user_id".to_string()]);

        let routing = analyzer.analyze("SELECT * FROM users");
        assert!(routing.partition_key.is_none());
        assert!(routing.requires_all_shards);
    }

    #[test]
    fn test_route_decision_types() {
        let single = RouteDecision::Single {
            shard_id: ShardId::new(0),
            node_id: NodeId::new("node1"),
        };

        let broadcast = RouteDecision::Broadcast {
            shards: vec![ShardId::new(0), ShardId::new(1)],
        };

        match single {
            RouteDecision::Single { .. } => {}
            _ => panic!("Expected single"),
        }

        match broadcast {
            RouteDecision::Broadcast { shards } => {
                assert_eq!(shards.len(), 2);
            }
            _ => panic!("Expected broadcast"),
        }
    }
}
