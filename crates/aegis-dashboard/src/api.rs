//! API client for Aegis DB Dashboard
//!
//! Makes real HTTP calls to the Aegis DB server API endpoints.

use crate::types::{
    ActivityType, Alert, AlertSeverity, AuthResponse, ClusterNode, ClusterStatus,
    DatabaseStats, DocumentCollection, DocumentEntry, GraphData, KeyValueEntry,
    MfaSetupData, NodeMetrics, NodeRole, NodeStatus, QueryBuilderResult, RecentActivity, User, UserRole,
};
use gloo_net::http::Request;
use web_sys::window;

/// Get the API base URL.
/// Uses the current window origin by default (same-origin requests).
/// Can be overridden by setting data-api-url attribute on document root.
fn get_api_base_url() -> String {
    // First try to get from document attribute (for explicit configuration)
    if let Some(win) = window() {
        if let Some(doc) = win.document() {
            if let Some(root) = doc.document_element() {
                if let Some(url) = root.get_attribute("data-api-url") {
                    if !url.is_empty() {
                        return url;
                    }
                }
            }
        }
        // Fall back to same-origin
        if let Ok(origin) = win.location().origin() {
            return origin;
        }
    }
    // Fallback for development
    "http://127.0.0.1:9090".to_string()
}

/// API error type.
#[derive(Debug, Clone)]
pub struct ApiError {
    pub message: String,
    pub status_code: Option<u16>,
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl From<gloo_net::Error> for ApiError {
    fn from(err: gloo_net::Error) -> Self {
        ApiError {
            message: err.to_string(),
            status_code: None,
        }
    }
}

/// API client for Aegis DB.
pub struct AegisClient {
    base_url: String,
    token: Option<String>,
}

impl AegisClient {
    /// Create a new client with the default base URL (same-origin or configured).
    /// Automatically loads token from localStorage if present.
    pub fn new() -> Self {
        // Try to get token from localStorage
        let token = if let Some(win) = window() {
            win.local_storage()
                .ok()
                .flatten()
                .and_then(|storage| storage.get_item("aegis_token").ok().flatten())
                .or_else(|| {
                    // Fallback to sessionStorage
                    win.session_storage()
                        .ok()
                        .flatten()
                        .and_then(|storage| storage.get_item("aegis_token").ok().flatten())
                })
        } else {
            None
        };

        Self {
            base_url: get_api_base_url(),
            token,
        }
    }

    /// Create a new client with a custom base URL.
    pub fn with_base_url(base_url: &str) -> Self {
        Self {
            base_url: base_url.to_string(),
            token: None,
        }
    }

    /// Set the authentication token.
    pub fn set_token(&mut self, token: String) {
        self.token = Some(token);
    }

    /// Build a request with common headers.
    fn build_request(&self, url: &str) -> gloo_net::http::RequestBuilder {
        let mut req = Request::get(url);
        if let Some(token) = &self.token {
            req = req.header("Authorization", &format!("Bearer {}", token));
        }
        req
    }

    /// Get cluster status from the server.
    pub async fn get_cluster_status(&self) -> Result<ClusterStatus, ApiError> {
        let url = format!("{}/api/v1/admin/cluster", self.base_url);
        let response = self.build_request(&url).send().await?;

        if !response.ok() {
            return Err(ApiError {
                message: format!("Server returned status {}", response.status()),
                status_code: Some(response.status()),
            });
        }

        // Parse the server's ClusterInfo response and convert to dashboard's ClusterStatus
        let cluster_info: ServerClusterInfo = response.json().await?;
        Ok(ClusterStatus {
            name: cluster_info.name,
            version: cluster_info.version,
            total_nodes: cluster_info.node_count as u32,
            healthy_nodes: cluster_info.node_count as u32, // All nodes are healthy for now
            leader_id: cluster_info.leader_id.unwrap_or_else(|| "unknown".to_string()),
            term: 1, // Raft term - would come from actual consensus module
            commit_index: 0, // Would come from actual consensus module
            shard_count: 1, // Would come from sharding module
            replication_factor: 3,
        })
    }

    /// Get all nodes from the server.
    pub async fn get_nodes(&self) -> Result<Vec<ClusterNode>, ApiError> {
        let url = format!("{}/api/v1/admin/nodes", self.base_url);
        let response = self.build_request(&url).send().await?;

        if !response.ok() {
            return Err(ApiError {
                message: format!("Server returned status {}", response.status()),
                status_code: Some(response.status()),
            });
        }

        // Parse the server's NodeInfo response and convert to dashboard's ClusterNode
        let nodes: Vec<ServerNodeInfo> = response.json().await?;
        Ok(nodes.into_iter().map(|n| ClusterNode {
            id: n.id,
            address: n.address,
            role: match n.role.as_str() {
                "Leader" => NodeRole::Leader,
                "Candidate" => NodeRole::Candidate,
                _ => NodeRole::Follower,
            },
            status: match n.status.as_str() {
                "Online" => NodeStatus::Healthy,
                "Offline" => NodeStatus::Offline,
                _ => NodeStatus::Degraded,
            },
            region: "default".to_string(),
            uptime: n.uptime_seconds,
            last_heartbeat: format_timestamp(n.last_heartbeat),
            metrics: NodeMetrics {
                cpu_usage: n.metrics.cpu_usage_percent,
                memory_usage: (n.metrics.memory_usage_bytes as f64 / n.metrics.memory_total_bytes as f64) * 100.0,
                disk_usage: (n.metrics.disk_usage_bytes as f64 / n.metrics.disk_total_bytes as f64) * 100.0,
                network_in: n.metrics.network_bytes_in,
                network_out: n.metrics.network_bytes_out,
                ops_per_second: n.metrics.queries_per_second as u64,
                latency_p50: n.metrics.latency_p50_ms,
                latency_p99: n.metrics.latency_p99_ms,
                connections: n.metrics.connections_active,
            },
        }).collect())
    }

    /// Get dashboard summary from the server.
    pub async fn get_dashboard_summary(&self) -> Result<DashboardSummary, ApiError> {
        let url = format!("{}/api/v1/admin/dashboard", self.base_url);
        let response = self.build_request(&url).send().await?;

        if !response.ok() {
            return Err(ApiError {
                message: format!("Server returned status {}", response.status()),
                status_code: Some(response.status()),
            });
        }

        response.json().await.map_err(Into::into)
    }

    /// Get database statistics from the server.
    pub async fn get_database_stats(&self) -> Result<DatabaseStats, ApiError> {
        let url = format!("{}/api/v1/admin/storage", self.base_url);
        let response = self.build_request(&url).send().await?;

        if !response.ok() {
            return Err(ApiError {
                message: format!("Server returned status {}", response.status()),
                status_code: Some(response.status()),
            });
        }

        let storage: ServerStorageInfo = response.json().await?;
        Ok(DatabaseStats {
            total_keys: 0, // Would need separate endpoint
            total_documents: 0,
            total_graph_nodes: 0,
            total_graph_edges: 0,
            storage_used: storage.used_bytes,
            storage_total: storage.total_bytes,
            data_bytes: storage.data_bytes,
            wal_bytes: storage.wal_bytes,
            index_bytes: storage.index_bytes,
            cache_hit_rate: 0.0, // Would need cache metrics endpoint
            ops_last_minute: 0, // Would need query metrics endpoint
        })
    }

    /// Get query statistics from the server.
    pub async fn get_query_stats(&self) -> Result<ServerQueryStats, ApiError> {
        let url = format!("{}/api/v1/admin/stats", self.base_url);
        let response = self.build_request(&url).send().await?;

        if !response.ok() {
            return Err(ApiError {
                message: format!("Server returned status {}", response.status()),
                status_code: Some(response.status()),
            });
        }

        response.json().await.map_err(Into::into)
    }

    /// Get alerts from the server.
    pub async fn get_alerts(&self) -> Result<Vec<Alert>, ApiError> {
        let url = format!("{}/api/v1/admin/alerts", self.base_url);
        let response = self.build_request(&url).send().await?;

        if !response.ok() {
            return Err(ApiError {
                message: format!("Server returned status {}", response.status()),
                status_code: Some(response.status()),
            });
        }

        let alerts_response: ServerAlertsResponse = response.json().await?;
        Ok(alerts_response.alerts.into_iter().map(|a| Alert {
            id: a.id,
            severity: match a.severity.as_str() {
                "Critical" => AlertSeverity::Critical,
                "Warning" => AlertSeverity::Warning,
                _ => AlertSeverity::Info,
            },
            message: a.message,
            source: a.source,
            timestamp: format_timestamp(a.timestamp),
            acknowledged: a.acknowledged,
        }).collect())
    }

    /// Execute a SQL query.
    pub async fn execute_query(&self, sql: &str) -> Result<QueryResponse, ApiError> {
        let url = format!("{}/api/v1/query", self.base_url);
        let response = Request::post(&url)
            .header("Content-Type", "application/json")
            .json(&QueryRequest { sql: sql.to_string(), params: vec![] })?
            .send()
            .await?;

        if !response.ok() {
            return Err(ApiError {
                message: format!("Query failed with status {}", response.status()),
                status_code: Some(response.status()),
            });
        }

        response.json().await.map_err(Into::into)
    }

    /// Check server health.
    pub async fn health_check(&self) -> Result<HealthResponse, ApiError> {
        let url = format!("{}/health", self.base_url);
        let response = Request::get(&url).send().await?;

        if !response.ok() {
            return Err(ApiError {
                message: format!("Health check failed with status {}", response.status()),
                status_code: Some(response.status()),
            });
        }

        response.json().await.map_err(Into::into)
    }
}

impl Default for AegisClient {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Server Response Types
// =============================================================================

/// Server's cluster info format (matches aegis-server/src/admin.rs)
#[derive(Debug, Clone, serde::Deserialize)]
#[allow(dead_code)]
struct ServerClusterInfo {
    name: String,
    version: String,
    node_count: usize,
    leader_id: Option<String>,
    state: String,
    uptime_seconds: u64,
}

/// Server's node info format
#[derive(Debug, Clone, serde::Deserialize)]
#[allow(dead_code)]
struct ServerNodeInfo {
    id: String,
    address: String,
    role: String,
    status: String,
    version: String,
    uptime_seconds: u64,
    last_heartbeat: u64,
    metrics: ServerNodeMetrics,
}

/// Server's node metrics format
#[derive(Debug, Clone, serde::Deserialize)]
#[allow(dead_code)]
struct ServerNodeMetrics {
    cpu_usage_percent: f64,
    memory_usage_bytes: u64,
    memory_total_bytes: u64,
    disk_usage_bytes: u64,
    disk_total_bytes: u64,
    connections_active: u64,
    queries_per_second: f64,
    // Network I/O
    network_bytes_in: u64,
    network_bytes_out: u64,
    network_packets_in: u64,
    network_packets_out: u64,
    // Latency histogram
    latency_p50_ms: f64,
    latency_p90_ms: f64,
    latency_p95_ms: f64,
    latency_p99_ms: f64,
    latency_max_ms: f64,
}

/// Server's storage info format
#[derive(Debug, Clone, serde::Deserialize)]
#[allow(dead_code)]
struct ServerStorageInfo {
    total_bytes: u64,
    used_bytes: u64,
    available_bytes: u64,
    data_bytes: u64,
    index_bytes: u64,
    wal_bytes: u64,
    temp_bytes: u64,
}

/// Server's alert info format
#[derive(Debug, Clone, serde::Deserialize)]
#[allow(dead_code)]
struct ServerAlertInfo {
    id: String,
    severity: String,
    source: String,
    message: String,
    timestamp: u64,
    acknowledged: bool,
    resolved: bool,
}

/// Server's alerts response format
#[derive(Debug, Clone, serde::Deserialize)]
struct ServerAlertsResponse {
    alerts: Vec<ServerAlertInfo>,
}

/// Server's query stats format
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ServerQueryStats {
    pub total_queries: u64,
    pub queries_per_second: f64,
    pub avg_duration_ms: f64,
    pub p50_duration_ms: f64,
    pub p95_duration_ms: f64,
    pub p99_duration_ms: f64,
    pub slow_queries: u64,
    pub failed_queries: u64,
}

/// Dashboard summary from server
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct DashboardSummary {
    pub cluster: ClusterSummary,
    pub performance: PerformanceSummary,
    pub storage: StorageSummary,
    pub alerts: AlertSummary,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ClusterSummary {
    pub state: String,
    pub node_count: usize,
    pub healthy_nodes: usize,
    pub leader_id: Option<String>,
    pub version: String,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct PerformanceSummary {
    pub queries_per_second: f64,
    pub avg_latency_ms: f64,
    pub active_connections: u64,
    pub cpu_usage_percent: f64,
    pub memory_usage_percent: f64,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct StorageSummary {
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub usage_percent: f64,
    pub database_count: usize,
    pub table_count: usize,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct AlertSummary {
    pub total: usize,
    pub critical: usize,
    pub warning: usize,
    pub unacknowledged: usize,
}

/// Query request to server
#[derive(Debug, Clone, serde::Serialize)]
struct QueryRequest {
    sql: String,
    params: Vec<serde_json::Value>,
}

/// Query response from server
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct QueryResponse {
    pub success: bool,
    pub data: Option<QueryResult>,
    pub error: Option<String>,
    pub execution_time_ms: u64,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<serde_json::Value>>,
    pub rows_affected: u64,
}

/// Health check response
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
}

// =============================================================================
// Authentication API
// =============================================================================

/// Login request to server.
#[derive(Debug, Clone, serde::Serialize)]
struct ServerLoginRequest {
    username: String,
    password: String,
}

/// Server auth response format.
#[derive(Debug, Clone, serde::Deserialize)]
struct ServerAuthResponse {
    token: Option<String>,
    user: Option<ServerUserInfo>,
    requires_mfa: Option<bool>,
    requires_mfa_setup: Option<bool>,
    mfa_setup_data: Option<MfaSetupData>,
    error: Option<String>,
}

/// Server user info format.
#[derive(Debug, Clone, serde::Deserialize)]
struct ServerUserInfo {
    id: String,
    username: String,
    email: String,
    role: String,
    mfa_enabled: bool,
    created_at: String,
}

/// Login to the Aegis DB server.
pub async fn login(username: &str, password: &str) -> Result<AuthResponse, String> {
    let url = format!("{}/api/v1/auth/login", &get_api_base_url());

    let response = Request::post(&url)
        .header("Content-Type", "application/json")
        .json(&ServerLoginRequest {
            username: username.to_string(),
            password: password.to_string(),
        })
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let server_response: ServerAuthResponse = response.json().await.map_err(|e| e.to_string())?;

    // Convert server response to dashboard AuthResponse
    Ok(AuthResponse {
        token: server_response.token,
        user: server_response.user.map(|u| User {
            id: u.id,
            username: u.username,
            email: u.email,
            role: match u.role.as_str() {
                "admin" => UserRole::Admin,
                "operator" => UserRole::Operator,
                _ => UserRole::Viewer,
            },
            mfa_enabled: u.mfa_enabled,
            created_at: u.created_at,
        }),
        requires_mfa: server_response.requires_mfa,
        requires_mfa_setup: server_response.requires_mfa_setup,
        mfa_setup_data: server_response.mfa_setup_data,
        error: server_response.error,
    })
}

/// MFA verify request to server.
#[derive(Debug, Clone, serde::Serialize)]
struct ServerMfaVerifyRequest {
    code: String,
    token: String,
}

/// Verify MFA code against the server.
pub async fn verify_mfa(code: &str, token: &str) -> Result<AuthResponse, String> {
    let url = format!("{}/api/v1/auth/mfa/verify", &get_api_base_url());

    let response = Request::post(&url)
        .header("Content-Type", "application/json")
        .json(&ServerMfaVerifyRequest {
            code: code.to_string(),
            token: token.to_string(),
        })
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let server_response: ServerAuthResponse = response.json().await.map_err(|e| e.to_string())?;

    // Convert server response to dashboard AuthResponse
    Ok(AuthResponse {
        token: server_response.token,
        user: server_response.user.map(|u| User {
            id: u.id,
            username: u.username,
            email: u.email,
            role: match u.role.as_str() {
                "admin" => UserRole::Admin,
                "operator" => UserRole::Operator,
                _ => UserRole::Viewer,
            },
            mfa_enabled: u.mfa_enabled,
            created_at: u.created_at,
        }),
        requires_mfa: server_response.requires_mfa,
        requires_mfa_setup: server_response.requires_mfa_setup,
        mfa_setup_data: server_response.mfa_setup_data,
        error: server_response.error,
    })
}

/// Logout from the server.
pub async fn logout(token: &str) -> Result<bool, String> {
    let url = format!("{}/api/v1/auth/logout", &get_api_base_url());

    #[derive(serde::Serialize)]
    struct LogoutRequest {
        token: String,
    }

    #[derive(serde::Deserialize)]
    struct LogoutResponse {
        success: bool,
    }

    let response = Request::post(&url)
        .header("Content-Type", "application/json")
        .json(&LogoutRequest { token: token.to_string() })
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let logout_response: LogoutResponse = response.json().await.map_err(|e| e.to_string())?;
    Ok(logout_response.success)
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Format a Unix timestamp (milliseconds) to RFC3339 string.
fn format_timestamp(timestamp_ms: u64) -> String {
    use chrono::{TimeZone, Utc};
    let secs = (timestamp_ms / 1000) as i64;
    let nsecs = ((timestamp_ms % 1000) * 1_000_000) as u32;
    Utc.timestamp_opt(secs, nsecs)
        .single()
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_else(|| "Invalid timestamp".to_string())
}

// =============================================================================
// Convenience Functions (matching old mock API interface)
// =============================================================================

/// Global client instance for convenience functions.
fn client() -> AegisClient {
    AegisClient::new()
}

/// Get cluster status (convenience wrapper).
pub async fn get_cluster_status() -> Result<ClusterStatus, String> {
    client().get_cluster_status().await.map_err(|e| e.to_string())
}

/// Get nodes (convenience wrapper).
pub async fn get_nodes() -> Result<Vec<ClusterNode>, String> {
    client().get_nodes().await.map_err(|e| e.to_string())
}

/// Get database stats (convenience wrapper).
pub async fn get_database_stats() -> Result<DatabaseStats, String> {
    client().get_database_stats().await.map_err(|e| e.to_string())
}

/// Get alerts (convenience wrapper).
pub async fn get_alerts() -> Result<Vec<Alert>, String> {
    client().get_alerts().await.map_err(|e| e.to_string())
}

/// Server activity format.
#[derive(Debug, Clone, serde::Deserialize)]
#[allow(dead_code)]
struct ServerActivity {
    id: String,
    #[serde(rename = "type")]
    activity_type: String,
    description: String,
    timestamp: String,
    duration: Option<u64>,
    user: Option<String>,
    source: Option<String>,
}

/// Get recent activity from the server.
pub async fn get_recent_activity() -> Result<Vec<RecentActivity>, String> {
    let url = format!("{}/api/v1/admin/activities?limit=20", &get_api_base_url());

    let response = Request::get(&url)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.ok() {
        return Err(format!("Server returned status {}", response.status()));
    }

    let activities: Vec<ServerActivity> = response.json().await.map_err(|e| e.to_string())?;

    Ok(activities.into_iter().map(|a| RecentActivity {
        id: a.id,
        activity_type: match a.activity_type.as_str() {
            "query" => ActivityType::Query,
            "write" => ActivityType::Write,
            "delete" => ActivityType::Delete,
            "config" => ActivityType::Config,
            "node" => ActivityType::Node,
            _ => ActivityType::Query,
        },
        description: a.description,
        timestamp: a.timestamp,
        duration: a.duration,
        user: a.user,
    }).collect())
}

// =============================================================================
// Key-Value Store API
// =============================================================================

/// Get all keys from the KV store.
pub async fn list_keys() -> Result<Vec<KeyValueEntry>, String> {
    let url = format!("{}/api/v1/kv/keys", &get_api_base_url());

    let response = Request::get(&url)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.ok() {
        return Err(format!("Server returned status {}", response.status()));
    }

    response.json().await.map_err(|e| e.to_string())
}

/// Set a key in the KV store.
pub async fn set_key(key: &str, value: serde_json::Value, ttl: Option<u64>) -> Result<KeyValueEntry, String> {
    let url = format!("{}/api/v1/kv/keys", &get_api_base_url());

    #[derive(serde::Serialize)]
    struct SetKeyRequest {
        key: String,
        value: serde_json::Value,
        ttl: Option<u64>,
    }

    let response = Request::post(&url)
        .header("Content-Type", "application/json")
        .json(&SetKeyRequest {
            key: key.to_string(),
            value,
            ttl,
        })
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.ok() {
        return Err(format!("Server returned status {}", response.status()));
    }

    response.json().await.map_err(|e| e.to_string())
}

/// Delete a key from the KV store.
pub async fn delete_key(key: &str) -> Result<bool, String> {
    let url = format!("{}/api/v1/kv/keys/{}", &get_api_base_url(), key);

    let response = Request::delete(&url)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.ok() {
        return Err(format!("Server returned status {}", response.status()));
    }

    #[derive(serde::Deserialize)]
    struct DeleteResponse {
        success: bool,
    }

    let resp: DeleteResponse = response.json().await.map_err(|e| e.to_string())?;
    Ok(resp.success)
}

// =============================================================================
// Document Store API
// =============================================================================

/// Get all collections from the document store.
pub async fn list_collections() -> Result<Vec<DocumentCollection>, String> {
    let url = format!("{}/api/v1/documents/collections", &get_api_base_url());

    let response = Request::get(&url)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.ok() {
        return Err(format!("Server returned status {}", response.status()));
    }

    response.json().await.map_err(|e| e.to_string())
}

/// Get documents from a specific collection.
pub async fn get_collection_documents(collection_name: &str) -> Result<Vec<DocumentEntry>, String> {
    let url = format!("{}/api/v1/documents/collections/{}", &get_api_base_url(), collection_name);

    let response = Request::get(&url)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.ok() {
        return Err(format!("Server returned status {}", response.status()));
    }

    response.json().await.map_err(|e| e.to_string())
}

// =============================================================================
// Graph Database API
// =============================================================================

/// Get graph data (nodes and edges).
pub async fn get_graph_data() -> Result<GraphData, String> {
    let url = format!("{}/api/v1/graph/data", &get_api_base_url());

    let response = Request::get(&url)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.ok() {
        return Err(format!("Server returned status {}", response.status()));
    }

    response.json().await.map_err(|e| e.to_string())
}

// =============================================================================
// Query Builder API
// =============================================================================

/// Execute a query via the query builder.
pub async fn execute_builder_query(query: &str, paradigm: &str) -> Result<QueryBuilderResult, String> {
    let url = format!("{}/api/v1/query-builder/execute", &get_api_base_url());

    #[derive(serde::Serialize)]
    struct QueryBuilderRequest {
        query: String,
        paradigm: String,
    }

    let response = Request::post(&url)
        .header("Content-Type", "application/json")
        .json(&QueryBuilderRequest {
            query: query.to_string(),
            paradigm: paradigm.to_string(),
        })
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.ok() {
        return Err(format!("Server returned status {}", response.status()));
    }

    response.json().await.map_err(|e| e.to_string())
}

// =============================================================================
// Node Action API
// =============================================================================

/// Node action response from server.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct NodeActionResponse {
    pub success: bool,
    pub message: String,
    pub node_id: String,
}

/// Node log entry from server.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct NodeLogEntry {
    pub timestamp: String,
    pub level: String,
    pub message: String,
}

/// Node logs response from server.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct NodeLogsResponse {
    pub node_id: String,
    pub logs: Vec<NodeLogEntry>,
    pub total: usize,
}

/// Restart a node.
pub async fn restart_node(node_id: &str) -> Result<NodeActionResponse, String> {
    let url = format!("{}/api/v1/admin/nodes/{}/restart", &get_api_base_url(), node_id);

    let response = Request::post(&url)
        .header("Content-Type", "application/json")
        .body("{}")
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.ok() {
        return Err(format!("Server returned status {}", response.status()));
    }

    response.json().await.map_err(|e| e.to_string())
}

/// Drain a node (prepare for maintenance).
pub async fn drain_node(node_id: &str) -> Result<NodeActionResponse, String> {
    let url = format!("{}/api/v1/admin/nodes/{}/drain", &get_api_base_url(), node_id);

    let response = Request::post(&url)
        .header("Content-Type", "application/json")
        .body("{}")
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.ok() {
        return Err(format!("Server returned status {}", response.status()));
    }

    response.json().await.map_err(|e| e.to_string())
}

/// Remove a node from the cluster.
pub async fn remove_node(node_id: &str) -> Result<NodeActionResponse, String> {
    let url = format!("{}/api/v1/admin/nodes/{}", &get_api_base_url(), node_id);

    let response = Request::delete(&url)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.ok() {
        return Err(format!("Server returned status {}", response.status()));
    }

    response.json().await.map_err(|e| e.to_string())
}

/// Get logs for a specific node.
pub async fn get_node_logs(node_id: &str, limit: Option<usize>) -> Result<NodeLogsResponse, String> {
    let url = match limit {
        Some(l) => format!("{}/api/v1/admin/nodes/{}/logs?limit={}", &get_api_base_url(), node_id, l),
        None => format!("{}/api/v1/admin/nodes/{}/logs", &get_api_base_url(), node_id),
    };

    let response = Request::get(&url)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.ok() {
        return Err(format!("Server returned status {}", response.status()));
    }

    response.json().await.map_err(|e| e.to_string())
}

// =============================================================================
// Settings API
// =============================================================================

/// Server settings structure.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ServerSettings {
    pub replication_factor: u8,
    pub auto_backups_enabled: bool,
    pub backup_schedule: String,
    pub retention_days: u32,
    pub tls_enabled: bool,
    pub auth_required: bool,
    pub session_timeout_minutes: u32,
    pub require_2fa: bool,
    pub audit_logging_enabled: bool,
}

impl Default for ServerSettings {
    fn default() -> Self {
        Self {
            replication_factor: 3,
            auto_backups_enabled: true,
            backup_schedule: "0 2 * * *".to_string(),
            retention_days: 30,
            tls_enabled: false,
            auth_required: true,
            session_timeout_minutes: 60,
            require_2fa: false,
            audit_logging_enabled: true,
        }
    }
}

/// Get server settings.
pub async fn get_settings() -> Result<ServerSettings, String> {
    let url = format!("{}/api/v1/admin/settings", &get_api_base_url());

    let response = Request::get(&url)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.ok() {
        return Err(format!("Server returned status {}", response.status()));
    }

    response.json().await.map_err(|e| e.to_string())
}

/// Update server settings.
pub async fn update_settings(settings: &ServerSettings) -> Result<(), String> {
    let url = format!("{}/api/v1/admin/settings", &get_api_base_url());

    let response = Request::put(&url)
        .header("Content-Type", "application/json")
        .json(settings)
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.ok() {
        return Err(format!("Server returned status {}", response.status()));
    }

    Ok(())
}

// =============================================================================
// User Management API
// =============================================================================

/// User info for API responses.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UserListItem {
    pub id: String,
    pub username: String,
    pub email: String,
    pub role: String,
    pub mfa_enabled: bool,
    pub enabled: bool,
    pub created_at: String,
    pub last_login: Option<String>,
}

/// List all users.
pub async fn list_users() -> Result<Vec<UserListItem>, String> {
    let url = format!("{}/api/v1/admin/users", &get_api_base_url());

    let response = Request::get(&url)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.ok() {
        return Err(format!("Server returned status {}", response.status()));
    }

    response.json().await.map_err(|e| e.to_string())
}

/// Create a new user.
pub async fn create_user(username: &str, email: &str, password: &str, role: &str) -> Result<UserListItem, String> {
    let url = format!("{}/api/v1/admin/users", &get_api_base_url());

    #[derive(serde::Serialize)]
    struct CreateUserRequest {
        username: String,
        email: String,
        password: String,
        role: String,
    }

    let response = Request::post(&url)
        .header("Content-Type", "application/json")
        .json(&CreateUserRequest {
            username: username.to_string(),
            email: email.to_string(),
            password: password.to_string(),
            role: role.to_string(),
        })
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.ok() {
        let text = response.text().await.unwrap_or_default();
        return Err(format!("Server returned status {}: {}", response.status(), text));
    }

    #[derive(serde::Deserialize)]
    struct CreateUserResponse {
        success: bool,
        user: Option<UserListItem>,
        error: Option<String>,
    }

    let resp: CreateUserResponse = response.json().await.map_err(|e| e.to_string())?;
    if resp.success {
        resp.user.ok_or_else(|| "No user returned".to_string())
    } else {
        Err(resp.error.unwrap_or_else(|| "Unknown error".to_string()))
    }
}

/// User update request.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UserUpdate {
    pub email: Option<String>,
    pub role: Option<String>,
    pub enabled: Option<bool>,
    pub password: Option<String>,
}

/// Update a user.
pub async fn update_user(username: &str, updates: &UserUpdate) -> Result<UserListItem, String> {
    let url = format!("{}/api/v1/admin/users/{}", &get_api_base_url(), username);

    let response = Request::put(&url)
        .header("Content-Type", "application/json")
        .json(updates)
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.ok() {
        return Err(format!("Server returned status {}", response.status()));
    }

    #[derive(serde::Deserialize)]
    struct UpdateUserResponse {
        success: bool,
        user: Option<UserListItem>,
        error: Option<String>,
    }

    let resp: UpdateUserResponse = response.json().await.map_err(|e| e.to_string())?;
    if resp.success {
        resp.user.ok_or_else(|| "No user returned".to_string())
    } else {
        Err(resp.error.unwrap_or_else(|| "Unknown error".to_string()))
    }
}

/// Delete a user.
pub async fn delete_user(username: &str) -> Result<(), String> {
    let url = format!("{}/api/v1/admin/users/{}", &get_api_base_url(), username);

    let response = Request::delete(&url)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.ok() {
        return Err(format!("Server returned status {}", response.status()));
    }

    #[derive(serde::Deserialize)]
    struct DeleteResponse {
        success: bool,
        error: Option<String>,
    }

    let resp: DeleteResponse = response.json().await.map_err(|e| e.to_string())?;
    if resp.success {
        Ok(())
    } else {
        Err(resp.error.unwrap_or_else(|| "Unknown error".to_string()))
    }
}

// =============================================================================
// Role Management API
// =============================================================================

/// Role info for API responses.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RoleInfo {
    pub name: String,
    pub description: String,
    pub permissions: Vec<String>,
    pub created_at: String,
    pub is_builtin: bool,
}

/// List all roles.
pub async fn list_roles() -> Result<Vec<RoleInfo>, String> {
    let url = format!("{}/api/v1/admin/roles", &get_api_base_url());

    let response = Request::get(&url)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.ok() {
        return Err(format!("Server returned status {}", response.status()));
    }

    response.json().await.map_err(|e| e.to_string())
}

/// Create a new role.
pub async fn create_role(name: &str, description: &str, permissions: &[String]) -> Result<RoleInfo, String> {
    let url = format!("{}/api/v1/admin/roles", &get_api_base_url());

    #[derive(serde::Serialize)]
    struct CreateRoleRequest {
        name: String,
        description: String,
        permissions: Vec<String>,
    }

    let response = Request::post(&url)
        .header("Content-Type", "application/json")
        .json(&CreateRoleRequest {
            name: name.to_string(),
            description: description.to_string(),
            permissions: permissions.to_vec(),
        })
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.ok() {
        return Err(format!("Server returned status {}", response.status()));
    }

    // The server returns a simplified response, construct RoleInfo
    Ok(RoleInfo {
        name: name.to_string(),
        description: description.to_string(),
        permissions: permissions.to_vec(),
        created_at: chrono::Utc::now().to_rfc3339(),
        is_builtin: false,
    })
}

/// Delete a role.
pub async fn delete_role(name: &str) -> Result<(), String> {
    let url = format!("{}/api/v1/admin/roles/{}", &get_api_base_url(), name);

    let response = Request::delete(&url)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.ok() {
        return Err(format!("Server returned status {}", response.status()));
    }

    #[derive(serde::Deserialize)]
    struct DeleteResponse {
        success: bool,
        error: Option<String>,
    }

    let resp: DeleteResponse = response.json().await.map_err(|e| e.to_string())?;
    if resp.success {
        Ok(())
    } else {
        Err(resp.error.unwrap_or_else(|| "Unknown error".to_string()))
    }
}

// =============================================================================
// Metrics Timeseries API
// =============================================================================

/// Metrics data point.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MetricsDataPoint {
    pub timestamp: i64,
    pub cpu_percent: f64,
    pub memory_percent: f64,
    pub queries_per_second: f64,
    pub latency_ms: f64,
    pub connections: u64,
    pub bytes_in: u64,
    pub bytes_out: u64,
}

/// Metrics timeseries response.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MetricsTimeseriesResponse {
    pub time_range: String,
    pub data_points: Vec<MetricsDataPoint>,
}

/// Get metrics timeseries data for a given time range.
pub async fn get_metrics_timeseries(time_range: &str) -> Result<Vec<MetricsDataPoint>, String> {
    let url = format!("{}/api/v1/admin/metrics/timeseries", &get_api_base_url());

    #[derive(serde::Serialize)]
    struct MetricsTimeseriesRequest {
        time_range: String,
    }

    let response = Request::post(&url)
        .header("Content-Type", "application/json")
        .json(&MetricsTimeseriesRequest {
            time_range: time_range.to_string(),
        })
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.ok() {
        return Err(format!("Server returned status {}", response.status()));
    }

    let resp: MetricsTimeseriesResponse = response.json().await.map_err(|e| e.to_string())?;
    Ok(resp.data_points)
}

// =============================================================================
// Graph Management API
// =============================================================================

/// Graph node for the API.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GraphNodeInfo {
    pub id: String,
    pub label: String,
    pub properties: serde_json::Value,
}

/// Graph edge for the API.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GraphEdgeInfo {
    pub id: String,
    pub source: String,
    pub target: String,
    pub relationship: String,
}

/// Create a new graph node.
pub async fn create_graph_node(label: &str, properties: serde_json::Value) -> Result<GraphNodeInfo, String> {
    let url = format!("{}/api/v1/graph/nodes", &get_api_base_url());

    #[derive(serde::Serialize)]
    struct CreateNodeRequest {
        label: String,
        properties: serde_json::Value,
    }

    let response = Request::post(&url)
        .header("Content-Type", "application/json")
        .json(&CreateNodeRequest {
            label: label.to_string(),
            properties,
        })
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.ok() {
        return Err(format!("Server returned status {}", response.status()));
    }

    #[derive(serde::Deserialize)]
    #[allow(dead_code)]
    struct CreateNodeResponse {
        success: bool,
        node: Option<GraphNodeInfo>,
    }

    let resp: CreateNodeResponse = response.json().await.map_err(|e| e.to_string())?;
    resp.node.ok_or_else(|| "No node returned".to_string())
}

/// Create a new graph edge.
pub async fn create_graph_edge(source: &str, target: &str, relationship: &str) -> Result<GraphEdgeInfo, String> {
    let url = format!("{}/api/v1/graph/edges", &get_api_base_url());

    #[derive(serde::Serialize)]
    struct CreateEdgeRequest {
        source: String,
        target: String,
        relationship: String,
    }

    let response = Request::post(&url)
        .header("Content-Type", "application/json")
        .json(&CreateEdgeRequest {
            source: source.to_string(),
            target: target.to_string(),
            relationship: relationship.to_string(),
        })
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.ok() {
        return Err(format!("Server returned status {}", response.status()));
    }

    #[derive(serde::Deserialize)]
    #[allow(dead_code)]
    struct CreateEdgeResponse {
        success: bool,
        edge: Option<GraphEdgeInfo>,
    }

    let resp: CreateEdgeResponse = response.json().await.map_err(|e| e.to_string())?;
    resp.edge.ok_or_else(|| "No edge returned".to_string())
}

/// Delete a graph node.
pub async fn delete_graph_node(node_id: &str) -> Result<(), String> {
    let url = format!("{}/api/v1/graph/nodes/{}", &get_api_base_url(), node_id);

    let response = Request::delete(&url)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.ok() {
        return Err(format!("Server returned status {}", response.status()));
    }

    Ok(())
}

// =============================================================================
// Document Collection Management API
// =============================================================================

/// Create a new collection.
pub async fn create_collection(name: &str) -> Result<DocumentCollection, String> {
    let url = format!("{}/api/v1/documents/collections", &get_api_base_url());

    #[derive(serde::Serialize)]
    struct CreateCollectionRequest {
        name: String,
    }

    let response = Request::post(&url)
        .header("Content-Type", "application/json")
        .json(&CreateCollectionRequest {
            name: name.to_string(),
        })
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.ok() {
        return Err(format!("Server returned status {}", response.status()));
    }

    response.json().await.map_err(|e| e.to_string())
}

/// Insert a document into a collection.
pub async fn insert_document(collection: &str, document: serde_json::Value) -> Result<DocumentEntry, String> {
    let url = format!("{}/api/v1/documents/collections/{}/documents", &get_api_base_url(), collection);

    let response = Request::post(&url)
        .header("Content-Type", "application/json")
        .json(&document)
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.ok() {
        return Err(format!("Server returned status {}", response.status()));
    }

    response.json().await.map_err(|e| e.to_string())
}

/// Delete a document from a collection.
pub async fn delete_document(collection: &str, doc_id: &str) -> Result<(), String> {
    let url = format!("{}/api/v1/documents/collections/{}/documents/{}", &get_api_base_url(), collection, doc_id);

    let response = Request::delete(&url)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.ok() {
        return Err(format!("Server returned status {}", response.status()));
    }

    Ok(())
}
