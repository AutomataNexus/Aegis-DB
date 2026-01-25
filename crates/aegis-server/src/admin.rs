//! Aegis Admin API
//!
//! Administrative endpoints for the web dashboard and management operations.
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::RwLock;
use sysinfo::{System, Disks};

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

/// Admin API service for dashboard operations with real system metrics.
pub struct AdminService {
    start_time: std::time::Instant,
    node_id: String,
    node_name: Option<String>,
    bind_address: String,
    cluster_name: String,
    // Query statistics tracking
    total_queries: AtomicU64,
    failed_queries: AtomicU64,
    slow_queries: AtomicU64,
    total_query_time_ns: AtomicU64,
    active_connections: AtomicU64,
    // Latency tracking (stored as microseconds for precision)
    latencies: RwLock<Vec<u64>>,
    // Network tracking
    bytes_in: AtomicU64,
    bytes_out: AtomicU64,
    // Cluster peer tracking
    peers: RwLock<Vec<PeerNode>>,
    peer_addresses: RwLock<Vec<String>>,
}

/// Information about a peer node in the cluster.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerNode {
    pub id: String,
    pub name: Option<String>,
    pub address: String,
    pub status: NodeStatus,
    pub role: NodeRole,
    pub last_seen: u64,
    pub version: String,
    pub uptime_seconds: u64,
    pub metrics: Option<NodeMetrics>,
}

impl AdminService {
    /// Create a new admin service.
    pub fn new() -> Self {
        Self::with_config("node-0", None, "127.0.0.1:9090", "aegis-cluster", vec![])
    }

    /// Create admin service with custom node config.
    pub fn with_config(node_id: &str, node_name: Option<String>, bind_address: &str, cluster_name: &str, peers: Vec<String>) -> Self {
        Self {
            start_time: std::time::Instant::now(),
            node_id: node_id.to_string(),
            node_name,
            bind_address: bind_address.to_string(),
            cluster_name: cluster_name.to_string(),
            total_queries: AtomicU64::new(0),
            failed_queries: AtomicU64::new(0),
            slow_queries: AtomicU64::new(0),
            total_query_time_ns: AtomicU64::new(0),
            active_connections: AtomicU64::new(0),
            latencies: RwLock::new(Vec::with_capacity(1000)),
            bytes_in: AtomicU64::new(0),
            bytes_out: AtomicU64::new(0),
            peers: RwLock::new(Vec::new()),
            peer_addresses: RwLock::new(peers),
        }
    }

    /// Get this node's ID.
    pub fn node_id(&self) -> &str {
        &self.node_id
    }

    /// Get this node's name.
    pub fn node_name(&self) -> Option<&str> {
        self.node_name.as_deref()
    }

    /// Get this node's address.
    pub fn address(&self) -> &str {
        &self.bind_address
    }

    /// Get configured peer addresses.
    pub fn peer_addresses(&self) -> Vec<String> {
        self.peer_addresses.read().unwrap().clone()
    }

    /// Add a peer address.
    pub fn add_peer_address(&self, address: String) {
        let mut addrs = self.peer_addresses.write().unwrap();
        if !addrs.contains(&address) && address != self.bind_address {
            addrs.push(address);
        }
    }

    /// Register or update a peer node.
    pub fn register_peer(&self, peer: PeerNode) {
        let mut peers = self.peers.write().unwrap();
        // Update if exists, otherwise add
        if let Some(existing) = peers.iter_mut().find(|p| p.id == peer.id || p.address == peer.address) {
            *existing = peer;
        } else {
            peers.push(peer);
        }
    }

    /// Remove a peer node.
    pub fn remove_peer(&self, node_id: &str) {
        let mut peers = self.peers.write().unwrap();
        peers.retain(|p| p.id != node_id);
    }

    /// Get all known peer nodes.
    pub fn get_peers(&self) -> Vec<PeerNode> {
        self.peers.read().unwrap().clone()
    }

    /// Mark a peer as offline.
    pub fn mark_peer_offline(&self, node_id: &str) {
        let mut peers = self.peers.write().unwrap();
        if let Some(peer) = peers.iter_mut().find(|p| p.id == node_id) {
            peer.status = NodeStatus::Offline;
        }
    }

    /// Get node info for this node (for peer registration).
    pub fn get_self_info(&self) -> PeerNode {
        let uptime = self.start_time.elapsed().as_secs();
        let (cpu_usage, memory_used, memory_total, disk_used, disk_total) = self.get_system_metrics();
        let total_queries = self.total_queries.load(Ordering::Relaxed);
        let qps = if uptime > 0 { total_queries as f64 / uptime as f64 } else { 0.0 };
        let (p50, p90, p95, p99, max) = self.calculate_latency_percentiles();

        PeerNode {
            id: self.node_id.clone(),
            name: self.node_name.clone(),
            address: self.bind_address.clone(),
            status: NodeStatus::Online,
            role: NodeRole::Leader, // Will be determined by Raft later
            last_seen: Self::now(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            uptime_seconds: uptime,
            metrics: Some(NodeMetrics {
                cpu_usage_percent: cpu_usage,
                memory_usage_bytes: memory_used,
                memory_total_bytes: memory_total,
                disk_usage_bytes: disk_used,
                disk_total_bytes: disk_total,
                connections_active: self.active_connections.load(Ordering::Relaxed),
                queries_per_second: qps,
                network_bytes_in: self.bytes_in.load(Ordering::Relaxed),
                network_bytes_out: self.bytes_out.load(Ordering::Relaxed),
                network_packets_in: 0,
                network_packets_out: 0,
                latency_p50_ms: p50,
                latency_p90_ms: p90,
                latency_p95_ms: p95,
                latency_p99_ms: p99,
                latency_max_ms: max,
            }),
        }
    }

    /// Record a query execution for statistics.
    pub fn record_query(&self, duration_ms: f64, success: bool) {
        self.total_queries.fetch_add(1, Ordering::Relaxed);
        self.total_query_time_ns.fetch_add((duration_ms * 1_000_000.0) as u64, Ordering::Relaxed);

        if !success {
            self.failed_queries.fetch_add(1, Ordering::Relaxed);
        }

        // Track slow queries (> 100ms)
        if duration_ms > 100.0 {
            self.slow_queries.fetch_add(1, Ordering::Relaxed);
        }

        // Track latency for percentile calculations
        let latency_us = (duration_ms * 1000.0) as u64;
        if let Ok(mut latencies) = self.latencies.write() {
            if latencies.len() >= 10000 {
                latencies.remove(0);
            }
            latencies.push(latency_us);
        }
    }

    /// Record network bytes.
    pub fn record_network(&self, bytes_in: u64, bytes_out: u64) {
        self.bytes_in.fetch_add(bytes_in, Ordering::Relaxed);
        self.bytes_out.fetch_add(bytes_out, Ordering::Relaxed);
    }

    /// Increment active connections.
    pub fn connection_opened(&self) {
        self.active_connections.fetch_add(1, Ordering::Relaxed);
    }

    /// Decrement active connections.
    pub fn connection_closed(&self) {
        self.active_connections.fetch_sub(1, Ordering::Relaxed);
    }

    /// Get real system metrics.
    fn get_system_metrics(&self) -> (f64, u64, u64, u64, u64) {
        let mut sys = System::new();
        sys.refresh_all();

        // Get CPU usage (average across all CPUs)
        let cpu_usage = sys.cpus().iter()
            .map(|cpu| cpu.cpu_usage() as f64)
            .sum::<f64>() / sys.cpus().len().max(1) as f64;

        let memory_used = sys.used_memory();
        let memory_total = sys.total_memory();

        // Get disk metrics
        let disks = Disks::new_with_refreshed_list();
        let (disk_used, disk_total) = disks.list().iter()
            .fold((0u64, 0u64), |(used, total), disk| {
                (used + (disk.total_space() - disk.available_space()), total + disk.total_space())
            });

        (cpu_usage, memory_used, memory_total, disk_used, disk_total)
    }

    /// Get cluster information.
    pub fn get_cluster_info(&self) -> ClusterInfo {
        let peers = self.peers.read().unwrap();
        let online_peers = peers.iter().filter(|p| p.status == NodeStatus::Online).count();
        let total_nodes = 1 + peers.len(); // self + peers
        let healthy_nodes = 1 + online_peers; // self is always healthy if running

        let state = if healthy_nodes == total_nodes {
            ClusterState::Healthy
        } else if healthy_nodes > total_nodes / 2 {
            ClusterState::Degraded
        } else {
            ClusterState::Unavailable
        };

        ClusterInfo {
            name: self.cluster_name.clone(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            node_count: total_nodes,
            leader_id: Some(self.node_id.clone()),
            state,
            uptime_seconds: self.start_time.elapsed().as_secs(),
        }
    }

    /// Get dashboard summary with real metrics.
    pub fn get_dashboard_summary(&self) -> DashboardSummary {
        let (cpu_usage, memory_used, memory_total, disk_used, disk_total) = self.get_system_metrics();
        let memory_percent = if memory_total > 0 {
            (memory_used as f64 / memory_total as f64) * 100.0
        } else {
            0.0
        };
        let storage_percent = if disk_total > 0 {
            (disk_used as f64 / disk_total as f64) * 100.0
        } else {
            0.0
        };

        let uptime = self.start_time.elapsed().as_secs();
        let total_queries = self.total_queries.load(Ordering::Relaxed);
        let qps = if uptime > 0 { total_queries as f64 / uptime as f64 } else { 0.0 };
        let total_time_ns = self.total_query_time_ns.load(Ordering::Relaxed);
        let avg_latency = if total_queries > 0 {
            (total_time_ns as f64 / total_queries as f64) / 1_000_000.0
        } else {
            0.0
        };

        DashboardSummary {
            cluster: ClusterSummary {
                state: "Healthy".to_string(),
                node_count: 1,
                healthy_nodes: 1,
                leader_id: Some(self.node_id.clone()),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            performance: PerformanceSummary {
                queries_per_second: qps,
                avg_latency_ms: avg_latency,
                active_connections: self.active_connections.load(Ordering::Relaxed),
                cpu_usage_percent: cpu_usage,
                memory_usage_percent: memory_percent,
            },
            storage: StorageSummary {
                total_bytes: disk_total,
                used_bytes: disk_used,
                usage_percent: storage_percent,
                database_count: 1, // Single default database
                table_count: 0, // Would need schema tracking
            },
            alerts: AlertSummary {
                total: 0,
                critical: 0,
                warning: 0,
                unacknowledged: 0,
            },
        }
    }

    /// Get list of all nodes (self + peers).
    pub fn get_nodes(&self) -> Vec<NodeInfo> {
        let uptime = self.start_time.elapsed().as_secs();
        let (cpu_usage, memory_used, memory_total, disk_used, disk_total) = self.get_system_metrics();

        let total_queries = self.total_queries.load(Ordering::Relaxed);
        let qps = if uptime > 0 { total_queries as f64 / uptime as f64 } else { 0.0 };

        // Calculate latency percentiles
        let (p50, p90, p95, p99, max) = self.calculate_latency_percentiles();

        // Start with self
        let mut nodes = vec![
            NodeInfo {
                id: format!("{}{}", self.node_id, self.node_name.as_ref().map(|n| format!(" ({})", n)).unwrap_or_default()),
                address: self.bind_address.clone(),
                role: NodeRole::Leader,
                status: NodeStatus::Online,
                version: env!("CARGO_PKG_VERSION").to_string(),
                uptime_seconds: uptime,
                last_heartbeat: Self::now(),
                metrics: NodeMetrics {
                    cpu_usage_percent: cpu_usage,
                    memory_usage_bytes: memory_used,
                    memory_total_bytes: memory_total,
                    disk_usage_bytes: disk_used,
                    disk_total_bytes: disk_total,
                    connections_active: self.active_connections.load(Ordering::Relaxed),
                    queries_per_second: qps,
                    network_bytes_in: self.bytes_in.load(Ordering::Relaxed),
                    network_bytes_out: self.bytes_out.load(Ordering::Relaxed),
                    network_packets_in: 0,
                    network_packets_out: 0,
                    latency_p50_ms: p50,
                    latency_p90_ms: p90,
                    latency_p95_ms: p95,
                    latency_p99_ms: p99,
                    latency_max_ms: max,
                },
            },
        ];

        // Add peer nodes
        let peers = self.peers.read().unwrap();
        for peer in peers.iter() {
            nodes.push(NodeInfo {
                id: format!("{}{}", peer.id, peer.name.as_ref().map(|n| format!(" ({})", n)).unwrap_or_default()),
                address: peer.address.clone(),
                role: peer.role,
                status: peer.status,
                version: peer.version.clone(),
                uptime_seconds: peer.uptime_seconds,
                last_heartbeat: peer.last_seen,
                metrics: peer.metrics.clone().unwrap_or_default(),
            });
        }

        nodes
    }

    /// Calculate latency percentiles from recorded data.
    fn calculate_latency_percentiles(&self) -> (f64, f64, f64, f64, f64) {
        let latencies = match self.latencies.read() {
            Ok(l) => l.clone(),
            Err(_) => return (0.0, 0.0, 0.0, 0.0, 0.0),
        };

        if latencies.is_empty() {
            return (0.0, 0.0, 0.0, 0.0, 0.0);
        }

        let mut sorted = latencies.clone();
        sorted.sort_unstable();

        let len = sorted.len();
        let p50_idx = (len as f64 * 0.50) as usize;
        let p90_idx = (len as f64 * 0.90) as usize;
        let p95_idx = (len as f64 * 0.95) as usize;
        let p99_idx = (len as f64 * 0.99) as usize;

        let to_ms = |us: u64| us as f64 / 1000.0;

        (
            to_ms(sorted.get(p50_idx).copied().unwrap_or(0)),
            to_ms(sorted.get(p90_idx).copied().unwrap_or(0)),
            to_ms(sorted.get(p95_idx).copied().unwrap_or(0)),
            to_ms(sorted.get(p99_idx.min(len - 1)).copied().unwrap_or(0)),
            to_ms(sorted.last().copied().unwrap_or(0)),
        )
    }

    /// Get storage information with real disk metrics.
    pub fn get_storage_info(&self) -> StorageInfo {
        let disks = Disks::new_with_refreshed_list();

        let (total, available) = disks.list().iter()
            .fold((0u64, 0u64), |(total, avail), disk| {
                (total + disk.total_space(), avail + disk.available_space())
            });

        let used = total.saturating_sub(available);

        // Estimate breakdown (would need actual tracking for precise values)
        let data_bytes = (used as f64 * 0.75) as u64;
        let index_bytes = (used as f64 * 0.15) as u64;
        let wal_bytes = (used as f64 * 0.08) as u64;
        let temp_bytes = (used as f64 * 0.02) as u64;

        StorageInfo {
            total_bytes: total,
            used_bytes: used,
            available_bytes: available,
            data_bytes,
            index_bytes,
            wal_bytes,
            temp_bytes,
        }
    }

    /// Get query statistics with real data.
    pub fn get_query_stats(&self) -> QueryStats {
        let uptime = self.start_time.elapsed().as_secs();
        let total_queries = self.total_queries.load(Ordering::Relaxed);
        let qps = if uptime > 0 { total_queries as f64 / uptime as f64 } else { 0.0 };

        let total_time_ns = self.total_query_time_ns.load(Ordering::Relaxed);
        let avg_duration = if total_queries > 0 {
            (total_time_ns as f64 / total_queries as f64) / 1_000_000.0
        } else {
            0.0
        };

        let (p50, _, p95, p99, _) = self.calculate_latency_percentiles();

        QueryStats {
            total_queries,
            queries_per_second: qps,
            avg_duration_ms: avg_duration,
            p50_duration_ms: p50,
            p95_duration_ms: p95,
            p99_duration_ms: p99,
            slow_queries: self.slow_queries.load(Ordering::Relaxed),
            failed_queries: self.failed_queries.load(Ordering::Relaxed),
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

        // Single node mode
        assert_eq!(summary.cluster.node_count, 1);
        assert_eq!(summary.cluster.healthy_nodes, 1);
        // CPU and memory should be valid percentages
        assert!(summary.performance.cpu_usage_percent >= 0.0);
        assert!(summary.performance.cpu_usage_percent <= 100.0);
        assert!(summary.performance.memory_usage_percent >= 0.0);
        assert!(summary.performance.memory_usage_percent <= 100.0);
    }

    #[test]
    fn test_get_nodes() {
        let service = AdminService::new();
        let nodes = service.get_nodes();

        // Single node in standalone mode
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].role, NodeRole::Leader);
        assert_eq!(nodes[0].status, NodeStatus::Online);
    }

    #[test]
    fn test_storage_info() {
        let service = AdminService::new();
        let storage = service.get_storage_info();

        // Real disk metrics - total should be positive
        assert!(storage.total_bytes > 0);
        assert!(storage.available_bytes <= storage.total_bytes);
        assert_eq!(
            storage.available_bytes,
            storage.total_bytes - storage.used_bytes
        );
    }

    #[test]
    fn test_query_stats() {
        let service = AdminService::new();

        // Record some queries to have data
        service.record_query(1.5, true);
        service.record_query(2.0, true);
        service.record_query(3.5, true);
        service.record_query(150.0, false); // slow + failed

        let stats = service.get_query_stats();

        assert_eq!(stats.total_queries, 4);
        assert_eq!(stats.failed_queries, 1);
        assert_eq!(stats.slow_queries, 1);
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
