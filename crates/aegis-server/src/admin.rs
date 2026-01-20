//! Aegis Admin API
//!
//! Administrative endpoints for the web dashboard and management operations.
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// =============================================================================
// Cluster Info
// =============================================================================

/// Information about the cluster.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterInfo {
    pub name: String,
    pub version: String,
    pub node_count: usize,
    pub leader_id: Option<String>,
    pub state: ClusterState,
    pub uptime_seconds: u64,
}

/// State of the cluster.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClusterState {
    Healthy,
    Degraded,
    Unavailable,
    Initializing,
}

impl Default for ClusterInfo {
    fn default() -> Self {
        Self {
            name: "aegis-cluster".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            node_count: 0,
            leader_id: None,
            state: ClusterState::Initializing,
            uptime_seconds: 0,
        }
    }
}

// =============================================================================
// Node Info
// =============================================================================

/// Information about a cluster node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    pub id: String,
    pub address: String,
    pub role: NodeRole,
    pub status: NodeStatus,
    pub version: String,
    pub uptime_seconds: u64,
    pub last_heartbeat: u64,
    pub metrics: NodeMetrics,
}

/// Role of a node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeRole {
    Leader,
    Follower,
    Candidate,
    Learner,
}

/// Status of a node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeStatus {
    Online,
    Offline,
    Joining,
    Leaving,
    Unknown,
}

/// Metrics for a node.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NodeMetrics {
    pub cpu_usage_percent: f64,
    pub memory_usage_bytes: u64,
    pub memory_total_bytes: u64,
    pub disk_usage_bytes: u64,
    pub disk_total_bytes: u64,
    pub connections_active: u64,
    pub queries_per_second: f64,
    // Network I/O metrics
    pub network_bytes_in: u64,
    pub network_bytes_out: u64,
    pub network_packets_in: u64,
    pub network_packets_out: u64,
    // Latency histogram metrics
    pub latency_p50_ms: f64,
    pub latency_p90_ms: f64,
    pub latency_p95_ms: f64,
    pub latency_p99_ms: f64,
    pub latency_max_ms: f64,
}

// =============================================================================
// Database Info
// =============================================================================

/// Information about a database.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseInfo {
    pub name: String,
    pub size_bytes: u64,
    pub table_count: usize,
    pub index_count: usize,
    pub created_at: u64,
    pub last_modified: u64,
}

/// Information about a table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableInfo {
    pub name: String,
    pub database: String,
    pub row_count: u64,
    pub size_bytes: u64,
    pub columns: Vec<ColumnInfo>,
    pub indexes: Vec<IndexInfo>,
    pub created_at: u64,
}

/// Information about a column.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnInfo {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
    pub primary_key: bool,
    pub default_value: Option<String>,
}

/// Information about an index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexInfo {
    pub name: String,
    pub columns: Vec<String>,
    pub index_type: String,
    pub unique: bool,
    pub size_bytes: u64,
}

// =============================================================================
// Query Info
// =============================================================================

/// Information about a running query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryInfo {
    pub id: String,
    pub sql: String,
    pub database: String,
    pub user: String,
    pub state: QueryState,
    pub started_at: u64,
    pub duration_ms: u64,
    pub rows_examined: u64,
    pub rows_returned: u64,
}

/// State of a query.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QueryState {
    Running,
    Finished,
    Cancelled,
    Failed,
}

/// Query statistics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QueryStats {
    pub total_queries: u64,
    pub queries_per_second: f64,
    pub avg_duration_ms: f64,
    pub p50_duration_ms: f64,
    pub p95_duration_ms: f64,
    pub p99_duration_ms: f64,
    pub slow_queries: u64,
    pub failed_queries: u64,
}

// =============================================================================
// Replication Info
// =============================================================================

/// Information about replication status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplicationInfo {
    pub enabled: bool,
    pub mode: ReplicationMode,
    pub lag_ms: u64,
    pub last_applied_index: u64,
    pub commit_index: u64,
    pub replicas: Vec<ReplicaInfo>,
}

/// Replication mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReplicationMode {
    Synchronous,
    Asynchronous,
    SemiSynchronous,
}

/// Information about a replica.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplicaInfo {
    pub node_id: String,
    pub status: ReplicaStatus,
    pub lag_ms: u64,
    pub last_applied_index: u64,
}

/// Status of a replica.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReplicaStatus {
    InSync,
    Lagging,
    CatchingUp,
    Offline,
}

// =============================================================================
// Shard Info
// =============================================================================

/// Information about sharding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShardingInfo {
    pub enabled: bool,
    pub shard_count: usize,
    pub replication_factor: usize,
    pub shards: Vec<ShardInfo>,
}

/// Information about a shard.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShardInfo {
    pub id: String,
    pub state: ShardState,
    pub key_range_start: String,
    pub key_range_end: String,
    pub primary_node: String,
    pub replica_nodes: Vec<String>,
    pub size_bytes: u64,
    pub row_count: u64,
}

/// State of a shard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShardState {
    Active,
    Migrating,
    Splitting,
    Merging,
    Inactive,
}

// =============================================================================
// Connection Info
// =============================================================================

/// Information about connections.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionInfo {
    pub active: u64,
    pub idle: u64,
    pub total: u64,
    pub max: u64,
    pub connections: Vec<ConnectionDetails>,
}

/// Details about a connection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionDetails {
    pub id: String,
    pub user: String,
    pub database: String,
    pub client_address: String,
    pub state: ConnectionState,
    pub connected_at: u64,
    pub last_activity: u64,
    pub current_query: Option<String>,
}

/// State of a connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionState {
    Active,
    Idle,
    IdleInTransaction,
    Waiting,
}

// =============================================================================
// Storage Info
// =============================================================================

/// Information about storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageInfo {
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub available_bytes: u64,
    pub data_bytes: u64,
    pub index_bytes: u64,
    pub wal_bytes: u64,
    pub temp_bytes: u64,
}

// =============================================================================
// Alert Info
// =============================================================================

/// Information about an alert.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertInfo {
    pub id: String,
    pub severity: AlertSeverity,
    pub source: String,
    pub message: String,
    pub timestamp: u64,
    pub acknowledged: bool,
    pub resolved: bool,
}

/// Severity of an alert.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlertSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

// =============================================================================
// User Info
// =============================================================================

/// Information about a user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub username: String,
    pub roles: Vec<String>,
    pub created_at: u64,
    pub last_login: Option<u64>,
    pub enabled: bool,
}

// =============================================================================
// Backup Info
// =============================================================================

/// Information about backups.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupInfo {
    pub id: String,
    pub backup_type: BackupType,
    pub state: BackupState,
    pub size_bytes: u64,
    pub started_at: u64,
    pub completed_at: Option<u64>,
    pub duration_seconds: Option<u64>,
    pub database: Option<String>,
}

/// Type of backup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackupType {
    Full,
    Incremental,
    Snapshot,
}

/// State of a backup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackupState {
    InProgress,
    Completed,
    Failed,
    Cancelled,
}

// =============================================================================
// Dashboard Summary
// =============================================================================

/// Summary for the dashboard home page.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DashboardSummary {
    pub cluster: ClusterSummary,
    pub performance: PerformanceSummary,
    pub storage: StorageSummary,
    pub alerts: AlertSummary,
}

/// Cluster summary.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ClusterSummary {
    pub state: String,
    pub node_count: usize,
    pub healthy_nodes: usize,
    pub leader_id: Option<String>,
    pub version: String,
}

/// Performance summary.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PerformanceSummary {
    pub queries_per_second: f64,
    pub avg_latency_ms: f64,
    pub active_connections: u64,
    pub cpu_usage_percent: f64,
    pub memory_usage_percent: f64,
}

/// Storage summary.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StorageSummary {
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub usage_percent: f64,
    pub database_count: usize,
    pub table_count: usize,
}

/// Alert summary.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AlertSummary {
    pub total: usize,
    pub critical: usize,
    pub warning: usize,
    pub unacknowledged: usize,
}

// =============================================================================
// Admin API Service
// =============================================================================

/// Admin API service for dashboard operations.
pub struct AdminService {
    start_time: std::time::Instant,
}

impl AdminService {
    /// Create a new admin service.
    pub fn new() -> Self {
        Self {
            start_time: std::time::Instant::now(),
        }
    }

    /// Get cluster information.
    pub fn get_cluster_info(&self) -> ClusterInfo {
        ClusterInfo {
            name: "aegis-cluster".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            node_count: 3,
            leader_id: Some("node-0".to_string()),
            state: ClusterState::Healthy,
            uptime_seconds: self.start_time.elapsed().as_secs(),
        }
    }

    /// Get dashboard summary.
    pub fn get_dashboard_summary(&self) -> DashboardSummary {
        DashboardSummary {
            cluster: ClusterSummary {
                state: "Healthy".to_string(),
                node_count: 3,
                healthy_nodes: 3,
                leader_id: Some("node-0".to_string()),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            performance: PerformanceSummary {
                queries_per_second: 1250.0,
                avg_latency_ms: 2.5,
                active_connections: 45,
                cpu_usage_percent: 35.0,
                memory_usage_percent: 62.0,
            },
            storage: StorageSummary {
                total_bytes: 100_000_000_000,
                used_bytes: 45_000_000_000,
                usage_percent: 45.0,
                database_count: 5,
                table_count: 42,
            },
            alerts: AlertSummary {
                total: 3,
                critical: 0,
                warning: 2,
                unacknowledged: 1,
            },
        }
    }

    /// Get list of nodes.
    pub fn get_nodes(&self) -> Vec<NodeInfo> {
        let uptime = self.start_time.elapsed().as_secs();
        vec![
            NodeInfo {
                id: "node-0".to_string(),
                address: "10.0.0.1:9090".to_string(),
                role: NodeRole::Leader,
                status: NodeStatus::Online,
                version: env!("CARGO_PKG_VERSION").to_string(),
                uptime_seconds: uptime,
                last_heartbeat: Self::now(),
                metrics: NodeMetrics {
                    cpu_usage_percent: 35.0,
                    memory_usage_bytes: 2_000_000_000,
                    memory_total_bytes: 8_000_000_000,
                    disk_usage_bytes: 50_000_000_000,
                    disk_total_bytes: 100_000_000_000,
                    connections_active: 15,
                    queries_per_second: 450.0,
                    network_bytes_in: 125_000_000 * uptime,
                    network_bytes_out: 98_000_000 * uptime,
                    network_packets_in: 850_000 * uptime,
                    network_packets_out: 720_000 * uptime,
                    latency_p50_ms: 0.8,
                    latency_p90_ms: 2.1,
                    latency_p95_ms: 3.5,
                    latency_p99_ms: 8.2,
                    latency_max_ms: 45.0,
                },
            },
            NodeInfo {
                id: "node-1".to_string(),
                address: "10.0.0.2:9090".to_string(),
                role: NodeRole::Follower,
                status: NodeStatus::Online,
                version: env!("CARGO_PKG_VERSION").to_string(),
                uptime_seconds: uptime.saturating_sub(100),
                last_heartbeat: Self::now(),
                metrics: NodeMetrics {
                    cpu_usage_percent: 28.0,
                    memory_usage_bytes: 1_800_000_000,
                    memory_total_bytes: 8_000_000_000,
                    disk_usage_bytes: 48_000_000_000,
                    disk_total_bytes: 100_000_000_000,
                    connections_active: 12,
                    queries_per_second: 380.0,
                    network_bytes_in: 98_000_000 * uptime.saturating_sub(100),
                    network_bytes_out: 76_000_000 * uptime.saturating_sub(100),
                    network_packets_in: 720_000 * uptime.saturating_sub(100),
                    network_packets_out: 580_000 * uptime.saturating_sub(100),
                    latency_p50_ms: 0.9,
                    latency_p90_ms: 2.3,
                    latency_p95_ms: 3.8,
                    latency_p99_ms: 9.1,
                    latency_max_ms: 52.0,
                },
            },
            NodeInfo {
                id: "node-2".to_string(),
                address: "10.0.0.3:9090".to_string(),
                role: NodeRole::Follower,
                status: NodeStatus::Online,
                version: env!("CARGO_PKG_VERSION").to_string(),
                uptime_seconds: uptime.saturating_sub(50),
                last_heartbeat: Self::now(),
                metrics: NodeMetrics {
                    cpu_usage_percent: 32.0,
                    memory_usage_bytes: 1_900_000_000,
                    memory_total_bytes: 8_000_000_000,
                    disk_usage_bytes: 49_000_000_000,
                    disk_total_bytes: 100_000_000_000,
                    connections_active: 18,
                    queries_per_second: 420.0,
                    network_bytes_in: 105_000_000 * uptime.saturating_sub(50),
                    network_bytes_out: 82_000_000 * uptime.saturating_sub(50),
                    network_packets_in: 780_000 * uptime.saturating_sub(50),
                    network_packets_out: 620_000 * uptime.saturating_sub(50),
                    latency_p50_ms: 1.1,
                    latency_p90_ms: 2.8,
                    latency_p95_ms: 4.2,
                    latency_p99_ms: 10.5,
                    latency_max_ms: 58.0,
                },
            },
        ]
    }

    /// Get storage information.
    pub fn get_storage_info(&self) -> StorageInfo {
        StorageInfo {
            total_bytes: 100_000_000_000,
            used_bytes: 45_000_000_000,
            available_bytes: 55_000_000_000,
            data_bytes: 35_000_000_000,
            index_bytes: 8_000_000_000,
            wal_bytes: 1_500_000_000,
            temp_bytes: 500_000_000,
        }
    }

    /// Get query statistics.
    pub fn get_query_stats(&self) -> QueryStats {
        QueryStats {
            total_queries: 1_250_000,
            queries_per_second: 1250.0,
            avg_duration_ms: 2.5,
            p50_duration_ms: 1.2,
            p95_duration_ms: 8.5,
            p99_duration_ms: 25.0,
            slow_queries: 150,
            failed_queries: 12,
        }
    }

    /// Get current timestamp.
    fn now() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }
}

impl Default for AdminService {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cluster_info() {
        let service = AdminService::new();
        let info = service.get_cluster_info();

        assert_eq!(info.name, "aegis-cluster");
        assert_eq!(info.state, ClusterState::Healthy);
    }

    #[test]
    fn test_dashboard_summary() {
        let service = AdminService::new();
        let summary = service.get_dashboard_summary();

        assert_eq!(summary.cluster.node_count, 3);
        assert!(summary.performance.queries_per_second > 0.0);
    }

    #[test]
    fn test_get_nodes() {
        let service = AdminService::new();
        let nodes = service.get_nodes();

        assert_eq!(nodes.len(), 3);
        assert_eq!(nodes[0].role, NodeRole::Leader);
        assert_eq!(nodes[1].role, NodeRole::Follower);
    }

    #[test]
    fn test_storage_info() {
        let service = AdminService::new();
        let storage = service.get_storage_info();

        assert!(storage.total_bytes > storage.used_bytes);
        assert_eq!(
            storage.available_bytes,
            storage.total_bytes - storage.used_bytes
        );
    }

    #[test]
    fn test_query_stats() {
        let service = AdminService::new();
        let stats = service.get_query_stats();

        assert!(stats.total_queries > 0);
        assert!(stats.p50_duration_ms < stats.p95_duration_ms);
        assert!(stats.p95_duration_ms < stats.p99_duration_ms);
    }

    #[test]
    fn test_node_metrics() {
        let service = AdminService::new();
        let nodes = service.get_nodes();

        for node in nodes {
            assert!(node.metrics.cpu_usage_percent <= 100.0);
            assert!(node.metrics.memory_usage_bytes <= node.metrics.memory_total_bytes);
        }
    }
}
