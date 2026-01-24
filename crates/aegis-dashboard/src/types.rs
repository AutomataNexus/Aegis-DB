//! Type definitions for the Aegis DB Dashboard

use serde::{Deserialize, Serialize};

/// User information
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct User {
    pub id: String,
    pub username: String,
    pub email: String,
    pub role: UserRole,
    pub mfa_enabled: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum UserRole {
    Admin,
    Operator,
    Viewer,
}

impl std::fmt::Display for UserRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UserRole::Admin => write!(f, "admin"),
            UserRole::Operator => write!(f, "operator"),
            UserRole::Viewer => write!(f, "viewer"),
        }
    }
}

/// Cluster node information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterNode {
    pub id: String,
    pub address: String,
    pub role: NodeRole,
    pub status: NodeStatus,
    pub region: String,
    pub uptime: u64,
    pub last_heartbeat: String,
    pub metrics: NodeMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum NodeRole {
    Leader,
    Follower,
    Candidate,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum NodeStatus {
    Healthy,
    Degraded,
    Offline,
}

/// Node metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeMetrics {
    pub cpu_usage: f64,
    pub memory_usage: f64,
    pub disk_usage: f64,
    pub network_in: u64,
    pub network_out: u64,
    pub ops_per_second: u64,
    pub latency_p50: f64,
    pub latency_p99: f64,
    pub connections: u64,
}

/// Overall cluster status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterStatus {
    pub name: String,
    pub version: String,
    pub total_nodes: u32,
    pub healthy_nodes: u32,
    pub leader_id: String,
    pub term: u64,
    pub commit_index: u64,
    pub shard_count: u32,
    pub replication_factor: u32,
}

/// Database statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseStats {
    pub total_keys: u64,
    pub total_documents: u64,
    pub total_graph_nodes: u64,
    pub total_graph_edges: u64,
    pub storage_used: u64,
    pub storage_total: u64,
    pub data_bytes: u64,
    pub wal_bytes: u64,
    pub index_bytes: u64,
    pub cache_hit_rate: f64,
    pub ops_last_minute: u64,
}

/// Recent activity entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentActivity {
    pub id: String,
    #[serde(rename = "type")]
    pub activity_type: ActivityType,
    pub description: String,
    pub timestamp: String,
    pub duration: Option<u64>,
    pub user: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ActivityType {
    Query,
    Write,
    Delete,
    Config,
    Node,
}

/// Alert information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    pub id: String,
    pub severity: AlertSeverity,
    pub message: String,
    pub source: String,
    pub timestamp: String,
    pub acknowledged: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AlertSeverity {
    Critical,
    Warning,
    Info,
}

/// Login credentials
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginCredentials {
    pub username: String,
    pub password: String,
}

/// MFA setup data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MfaSetupData {
    pub secret: String,
    pub qr_code: String,
    pub backup_codes: Vec<String>,
}

/// Authentication response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthResponse {
    pub token: Option<String>,
    pub user: Option<User>,
    pub requires_mfa: Option<bool>,
    pub requires_mfa_setup: Option<bool>,
    pub mfa_setup_data: Option<MfaSetupData>,
    pub error: Option<String>,
}

// =============================================================================
// Database Browser Types
// =============================================================================

/// Key-Value entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyValueEntry {
    pub key: String,
    pub value: serde_json::Value,
    pub size_bytes: u64,
    pub created_at: String,
    pub updated_at: String,
    pub ttl: Option<u64>,
}

/// Document collection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentCollection {
    pub name: String,
    pub document_count: u64,
    pub size_bytes: u64,
    pub indexes: Vec<String>,
}

/// Document entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentEntry {
    pub id: String,
    pub data: serde_json::Value,
    pub created_at: String,
    pub updated_at: String,
}

/// Graph node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: String,
    pub label: String,
    pub properties: serde_json::Value,
}

/// Graph edge
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    pub label: String,
    pub properties: serde_json::Value,
}

/// Graph data response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphData {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

/// Query builder result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryBuilderResult {
    pub success: bool,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<serde_json::Value>>,
    pub rows_affected: u64,
    pub execution_time_ms: u64,
    pub error: Option<String>,
}
