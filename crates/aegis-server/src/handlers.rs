//! Aegis Request Handlers
//!
//! HTTP request handlers for the REST API. Implements endpoints for
//! query execution, health checks, and administrative operations.
//! All handlers use real engine integrations - no mock data.
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use crate::activity::{Activity, ActivityType};
use crate::admin::{
    AlertInfo, AlertSeverity, ClusterInfo, DashboardSummary, NodeInfo, QueryStats, StorageInfo,
};
use crate::auth::{LoginRequest, MfaVerifyRequest, UserInfo};
use crate::state::{AppState, GraphEdge, GraphNode, KvEntry, QueryError, QueryResult};
use aegis_document::{Document, DocumentId, Query as DocQuery, QueryResult as DocQueryResult};
use aegis_streaming::{event::EventData, ChannelId, Event, EventType as StreamEventType};
use aegis_timeseries::{DataPoint, Metric, MetricType, Tags, TimeSeriesQuery};
use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use std::time::Instant;

// =============================================================================
// Health Check
// =============================================================================

/// Health check response.
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
}

/// Health check endpoint.
pub async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

// =============================================================================
// Query Endpoints
// =============================================================================

/// Query request body.
#[derive(Debug, Deserialize)]
pub struct QueryRequest {
    /// Target database name (optional, defaults to "default")
    #[serde(default)]
    pub database: Option<String>,
    pub sql: String,
    #[serde(default)]
    pub params: Vec<serde_json::Value>,
}

/// Query response.
#[derive(Debug, Serialize)]
pub struct QueryResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<QueryResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub execution_time_ms: u64,
}

/// Execute a SQL query.
pub async fn execute_query(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(request): Json<QueryRequest>,
) -> impl IntoResponse {
    let start = Instant::now();
    let is_replicated = headers.get("x-aegis-replicated").is_some();

    // Defense-in-depth: run user SQL through the Shield's injection detector so
    // it actually inspects query bodies (the request middleware never sees them)
    // and records reputation/events. DETECT-AND-LOG only for now — enforcement is
    // intentionally off to avoid false-positives blocking legitimate SQL during a
    // rolling deploy. To enforce, return 403 on the Block arm below.
    if !is_replicated {
        let ctx = aegis_shield::RequestContext {
            source_ip: client_ip_from_headers(&headers),
            path: "/api/v1/query".to_string(),
            method: "POST".to_string(),
            user_agent: headers
                .get("user-agent")
                .and_then(|h| h.to_str().ok())
                .map(|s| s.to_string()),
            auth_user: None,
            body_size: request.sql.len(),
            headers: std::collections::HashMap::new(),
        };
        if let aegis_shield::ShieldVerdict::Block {
            reason,
            threat_level,
        } = state.shield.analyze_query(&request.sql, &ctx)
        {
            tracing::warn!(
                level = ?threat_level,
                "Shield flagged query (detect-only, not blocked): {}",
                reason
            );
        }
    }

    let result = if is_replicated {
        // Replicated query — execute locally, don't re-replicate
        state
            .execute_query_replicated(&request.sql, request.database.as_deref())
            .await
    } else if !request.params.is_empty() {
        state
            .execute_query_with_params(&request.sql, request.database.as_deref(), &request.params)
            .await
    } else {
        state
            .execute_query(&request.sql, request.database.as_deref())
            .await
    };
    let duration_ms = start.elapsed().as_millis() as u64;

    match result {
        Ok(data) => {
            state.record_request(duration_ms, true).await;
            (
                StatusCode::OK,
                Json(QueryResponse {
                    success: true,
                    data: Some(data),
                    error: None,
                    execution_time_ms: duration_ms,
                }),
            )
        }
        Err(e) => {
            state.record_request(duration_ms, false).await;
            let (status, client_msg) = match &e {
                QueryError::Parse(_) => (StatusCode::BAD_REQUEST, "Query syntax error"),
                QueryError::Plan(_) => (StatusCode::BAD_REQUEST, "Query planning error"),
                QueryError::Execute(_) => {
                    (StatusCode::INTERNAL_SERVER_ERROR, "Query execution error")
                }
            };
            tracing::warn!("Query failed: {}", e);
            (
                status,
                Json(QueryResponse {
                    success: false,
                    data: None,
                    error: Some(client_msg.to_string()),
                    execution_time_ms: duration_ms,
                }),
            )
        }
    }
}

// =============================================================================
// Prepared Statements
// =============================================================================

/// Prepare-statement request.
#[derive(Debug, Deserialize)]
pub struct PrepareRequest {
    #[serde(default)]
    pub database: Option<String>,
    pub sql: String,
}

/// Execute-prepared request.
#[derive(Debug, Deserialize)]
pub struct ExecutePreparedRequest {
    pub statement_id: String,
    #[serde(default)]
    pub params: Vec<serde_json::Value>,
}

/// Parse + plan a statement once and return an id for repeated execution.
pub async fn prepare_statement(
    State(state): State<AppState>,
    Json(request): Json<PrepareRequest>,
) -> impl IntoResponse {
    match state
        .query_engine
        .prepare(&request.sql, request.database.as_deref())
    {
        Ok(id) => (
            StatusCode::CREATED,
            Json(serde_json::json!({ "success": true, "statement_id": id })),
        ),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "success": false, "error": e.to_string() })),
        ),
    }
}

/// Execute a previously prepared statement with bound parameters.
pub async fn execute_prepared(
    State(state): State<AppState>,
    Json(request): Json<ExecutePreparedRequest>,
) -> impl IntoResponse {
    let start = Instant::now();
    let result = state
        .query_engine
        .execute_prepared(&request.statement_id, &request.params);
    let duration_ms = start.elapsed().as_millis() as u64;

    match result {
        Ok(data) => {
            state.record_request(duration_ms, true).await;
            (
                StatusCode::OK,
                Json(QueryResponse {
                    success: true,
                    data: Some(data),
                    error: None,
                    execution_time_ms: duration_ms,
                }),
            )
        }
        Err(e) => {
            state.record_request(duration_ms, false).await;
            (
                StatusCode::BAD_REQUEST,
                Json(QueryResponse {
                    success: false,
                    data: None,
                    error: Some(e.to_string()),
                    execution_time_ms: duration_ms,
                }),
            )
        }
    }
}

/// Deallocate a prepared statement by id.
pub async fn deallocate_prepared(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    if state.query_engine.deallocate(&id) {
        (StatusCode::OK, Json(serde_json::json!({ "success": true })))
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "success": false, "error": "unknown prepared statement" })),
        )
    }
}

// =============================================================================
// Table Endpoints
// =============================================================================

/// List tables response.
#[derive(Debug, Serialize)]
pub struct TablesResponse {
    pub tables: Vec<TableInfo>,
}

/// Table information.
#[derive(Debug, Serialize)]
pub struct TableInfo {
    pub name: String,
    pub columns: Vec<ColumnInfo>,
    pub row_count: Option<u64>,
}

/// Column information.
#[derive(Debug, Serialize)]
pub struct ColumnInfo {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
}

/// List all tables (from default database, use query endpoint with database param for others).
pub async fn list_tables(State(state): State<AppState>) -> Json<TablesResponse> {
    let table_names = state.query_engine.list_tables(None);
    let tables: Vec<TableInfo> = table_names
        .into_iter()
        .filter_map(|name| state.query_engine.get_table_info(&name, None))
        .map(|info| TableInfo {
            name: info.name,
            columns: info
                .columns
                .into_iter()
                .map(|c| ColumnInfo {
                    name: c.name,
                    data_type: c.data_type,
                    nullable: c.nullable,
                })
                .collect(),
            row_count: info.row_count,
        })
        .collect();
    Json(TablesResponse { tables })
}

/// Get table details (from default database).
pub async fn get_table(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    match state.query_engine.get_table_info(&name, None) {
        Some(info) => Json(TableInfo {
            name: info.name,
            columns: info
                .columns
                .into_iter()
                .map(|c| ColumnInfo {
                    name: c.name,
                    data_type: c.data_type,
                    nullable: c.nullable,
                })
                .collect(),
            row_count: info.row_count,
        }),
        None => Json(TableInfo {
            name,
            columns: vec![],
            row_count: None,
        }),
    }
}

// =============================================================================
// Metrics Endpoint
// =============================================================================

/// Metrics response.
#[derive(Debug, Serialize)]
pub struct MetricsResponse {
    pub total_requests: u64,
    pub failed_requests: u64,
    pub avg_duration_ms: f64,
    pub success_rate: f64,
}

/// Get server metrics.
pub async fn get_metrics(State(state): State<AppState>) -> Json<MetricsResponse> {
    let metrics = state.metrics.read().await;
    Json(MetricsResponse {
        total_requests: metrics.total_requests,
        failed_requests: metrics.failed_requests,
        avg_duration_ms: metrics.avg_duration_ms(),
        success_rate: metrics.success_rate(),
    })
}

// =============================================================================
// Error Response
// =============================================================================

/// Generic error response.
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub code: String,
}

impl ErrorResponse {
    pub fn new(error: impl ToString, code: impl ToString) -> Self {
        Self {
            error: error.to_string(),
            code: code.to_string(),
        }
    }
}

/// Not found handler.
pub async fn not_found() -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        Json(ErrorResponse::new("Not found", "NOT_FOUND")),
    )
}

// =============================================================================
// Admin Endpoints
// =============================================================================

/// Get cluster information.
pub async fn get_cluster_info(State(state): State<AppState>) -> Json<ClusterInfo> {
    Json(state.admin.get_cluster_info())
}

/// Get dashboard summary.
pub async fn get_dashboard_summary(State(state): State<AppState>) -> Json<DashboardSummary> {
    Json(state.admin.get_dashboard_summary())
}

/// Get all nodes.
pub async fn get_nodes(State(state): State<AppState>) -> Json<Vec<NodeInfo>> {
    Json(state.admin.get_nodes())
}

// =============================================================================
// Cluster Peer Management
// =============================================================================

/// Request to join a cluster.
#[derive(Debug, Deserialize)]
pub struct JoinClusterRequest {
    pub node_id: String,
    pub node_name: Option<String>,
    pub address: String,
}

/// Response from joining a cluster.
#[derive(Debug, Serialize)]
pub struct JoinClusterResponse {
    pub success: bool,
    pub message: String,
    pub peers: Vec<PeerInfo>,
}

/// Peer info for cluster responses.
#[derive(Debug, Serialize)]
pub struct PeerInfo {
    pub id: String,
    pub name: Option<String>,
    pub address: String,
}

/// Get this node's info for peer discovery.
pub async fn get_node_info(State(state): State<AppState>) -> Json<crate::admin::PeerNode> {
    Json(state.admin.get_self_info())
}

/// Join/register with this node (called by other nodes).
pub async fn cluster_join(
    State(state): State<AppState>,
    Json(req): Json<JoinClusterRequest>,
) -> Json<JoinClusterResponse> {
    use crate::admin::{NodeRole, NodeStatus, PeerNode};

    // Register the requesting node as a peer
    let peer = PeerNode {
        id: req.node_id.clone(),
        name: req.node_name.clone(),
        address: req.address.clone(),
        status: NodeStatus::Online,
        role: NodeRole::Follower,
        last_seen: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64,
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_seconds: 0,
        metrics: None,
    };

    state.admin.register_peer(peer);
    state.admin.add_peer_address(req.address.clone());

    tracing::info!(
        "Node joined cluster: {} ({}) at {}",
        req.node_id,
        req.node_name.as_deref().unwrap_or("unnamed"),
        req.address
    );

    // Return list of all known peers (including self)
    let self_info = state.admin.get_self_info();
    let mut peers = vec![PeerInfo {
        id: self_info.id,
        name: self_info.name,
        address: self_info.address,
    }];

    for peer in state.admin.get_peers() {
        if peer.id != req.node_id {
            peers.push(PeerInfo {
                id: peer.id,
                name: peer.name,
                address: peer.address,
            });
        }
    }

    Json(JoinClusterResponse {
        success: true,
        message: "Successfully joined cluster".to_string(),
        peers,
    })
}

/// Heartbeat from a peer node.
#[derive(Debug, Deserialize)]
pub struct HeartbeatRequest {
    pub node_id: String,
    pub node_name: Option<String>,
    pub address: String,
    pub uptime_seconds: u64,
    pub metrics: Option<crate::admin::NodeMetrics>,
}

/// Receive heartbeat from a peer.
pub async fn cluster_heartbeat(
    State(state): State<AppState>,
    Json(req): Json<HeartbeatRequest>,
) -> Json<serde_json::Value> {
    use crate::admin::{NodeRole, NodeStatus, PeerNode};

    // Update peer info
    let peer = PeerNode {
        id: req.node_id.clone(),
        name: req.node_name,
        address: req.address,
        status: NodeStatus::Online,
        role: NodeRole::Follower,
        last_seen: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64,
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_seconds: req.uptime_seconds,
        metrics: req.metrics,
    };

    state.admin.register_peer(peer);

    Json(serde_json::json!({
        "success": true,
        "timestamp": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    }))
}

/// Get list of known peers.
pub async fn get_peers(State(state): State<AppState>) -> Json<Vec<crate::admin::PeerNode>> {
    Json(state.admin.get_peers())
}

/// Get storage information.
pub async fn get_storage_info(State(state): State<AppState>) -> Json<StorageInfo> {
    Json(state.admin.get_storage_info())
}

/// Get query statistics.
pub async fn get_query_stats(State(state): State<AppState>) -> Json<QueryStats> {
    Json(state.admin.get_query_stats())
}

/// Get database statistics (key counts, document counts, etc.)
/// Use ?local=true to get only local stats (used by peer aggregation to avoid loops)
pub async fn get_database_stats(
    State(state): State<AppState>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Json<crate::state::DatabaseStats> {
    // Start with local stats
    let mut stats = state.get_database_stats();

    // If local=true, return only local stats (prevents infinite recursion when peers call each other)
    if params.get("local").map(|v| v == "true").unwrap_or(false) {
        return Json(stats);
    }

    // Aggregate stats from all cluster peers (call with ?local=true to prevent loops)
    let peers = state.admin.get_peers();
    let client = reqwest::Client::new();

    for peer in peers {
        let url = format!("http://{}/api/v1/admin/database?local=true", peer.address);
        if let Ok(response) = client
            .get(&url)
            .timeout(std::time::Duration::from_secs(2))
            .send()
            .await
        {
            if let Ok(peer_stats) = response.json::<crate::state::DatabaseStats>().await {
                stats.total_keys += peer_stats.total_keys;
                stats.total_documents += peer_stats.total_documents;
                stats.collection_count += peer_stats.collection_count;
                stats.documents_inserted += peer_stats.documents_inserted;
                stats.documents_updated += peer_stats.documents_updated;
                stats.documents_deleted += peer_stats.documents_deleted;
                stats.queries_executed += peer_stats.queries_executed;
            }
        }
    }

    Json(stats)
}

/// Alert response structure.
#[derive(Debug, Serialize)]
pub struct AlertsResponse {
    pub alerts: Vec<AlertInfo>,
}

/// Get active alerts based on real system conditions.
pub async fn get_alerts(State(_state): State<AppState>) -> Json<AlertsResponse> {
    use sysinfo::{Disks, System};

    let mut alerts = Vec::new();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    // Check memory usage
    let mut sys = System::new();
    sys.refresh_memory();

    let memory_total = sys.total_memory();
    let memory_used = sys.used_memory();
    if memory_total > 0 {
        let memory_percent = (memory_used as f64 / memory_total as f64) * 100.0;
        if memory_percent > 90.0 {
            alerts.push(AlertInfo {
                id: "mem-critical".to_string(),
                severity: AlertSeverity::Critical,
                source: "system".to_string(),
                message: format!("Critical memory usage: {:.1}%", memory_percent),
                timestamp: now,
                acknowledged: false,
                resolved: false,
            });
        } else if memory_percent > 80.0 {
            alerts.push(AlertInfo {
                id: "mem-warning".to_string(),
                severity: AlertSeverity::Warning,
                source: "system".to_string(),
                message: format!("High memory usage: {:.1}%", memory_percent),
                timestamp: now,
                acknowledged: false,
                resolved: false,
            });
        }
    }

    // Check disk usage
    let disks = Disks::new_with_refreshed_list();
    for disk in disks.list() {
        let total = disk.total_space();
        let available = disk.available_space();
        if total > 0 {
            let used_percent = ((total - available) as f64 / total as f64) * 100.0;
            let mount = disk.mount_point().to_string_lossy();
            if used_percent > 95.0 {
                alerts.push(AlertInfo {
                    id: format!("disk-critical-{}", mount.replace("/", "_")),
                    severity: AlertSeverity::Critical,
                    source: "system".to_string(),
                    message: format!("Critical disk usage on {}: {:.1}%", mount, used_percent),
                    timestamp: now,
                    acknowledged: false,
                    resolved: false,
                });
            } else if used_percent > 85.0 {
                alerts.push(AlertInfo {
                    id: format!("disk-warning-{}", mount.replace("/", "_")),
                    severity: AlertSeverity::Warning,
                    source: "system".to_string(),
                    message: format!("High disk usage on {}: {:.1}%", mount, used_percent),
                    timestamp: now,
                    acknowledged: false,
                    resolved: false,
                });
            }
        }
    }

    Json(AlertsResponse { alerts })
}

// =============================================================================
// Authentication Endpoints
// =============================================================================

/// Login endpoint.
/// Best-effort client IP from proxy headers, falling back to localhost.
/// Mirrors the middleware's extraction; only the first `X-Forwarded-For` hop
/// and `X-Real-IP` are considered.
fn client_ip_from_headers(headers: &axum::http::HeaderMap) -> String {
    if let Some(xff) = headers.get("x-forwarded-for").and_then(|h| h.to_str().ok()) {
        if let Some(first) = xff.split(',').next() {
            let ip = first.trim();
            if !ip.is_empty() {
                return ip.to_string();
            }
        }
    }
    if let Some(real) = headers.get("x-real-ip").and_then(|h| h.to_str().ok()) {
        if !real.trim().is_empty() {
            return real.trim().to_string();
        }
    }
    "127.0.0.1".to_string()
}

pub async fn login(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(request): Json<LoginRequest>,
) -> impl IntoResponse {
    let response = state.auth.login(&request.username, &request.password);

    if response.error.is_some() {
        state.activity.log_auth(
            &format!("Failed login attempt for user: {}", request.username),
            Some(&request.username),
        );
        // Feed the Shield's brute-force detector so repeated failures from an IP
        // trigger its auto-ban (the rate limiter alone can't do reputation/bans).
        let client_ip = client_ip_from_headers(&headers);
        state
            .shield
            .record_failed_auth(&client_ip, &request.username);
        (StatusCode::UNAUTHORIZED, Json(response))
    } else if response.requires_mfa == Some(true) {
        state.activity.log_auth(
            &format!("MFA required for user: {}", request.username),
            Some(&request.username),
        );
        (StatusCode::OK, Json(response))
    } else {
        state.activity.log_auth(
            &format!("User logged in: {}", request.username),
            Some(&request.username),
        );
        (StatusCode::OK, Json(response))
    }
}

/// MFA verification endpoint.
pub async fn verify_mfa(
    State(state): State<AppState>,
    Json(request): Json<MfaVerifyRequest>,
) -> impl IntoResponse {
    let response = state.auth.verify_mfa(&request.code, &request.token);

    if response.error.is_some() {
        state.activity.log_auth("Failed MFA verification", None);
        (StatusCode::UNAUTHORIZED, Json(response))
    } else {
        let username = response.user.as_ref().map(|u| u.username.as_str());
        state.activity.log_auth(
            &format!("MFA verified for user: {}", username.unwrap_or("unknown")),
            username,
        );
        (StatusCode::OK, Json(response))
    }
}

/// Logout request.
#[derive(Debug, Deserialize)]
pub struct LogoutRequest {
    pub token: String,
}

/// Logout response.
#[derive(Debug, Serialize)]
pub struct LogoutResponse {
    pub success: bool,
}

/// Logout endpoint.
pub async fn logout(
    State(state): State<AppState>,
    Json(request): Json<LogoutRequest>,
) -> Json<LogoutResponse> {
    let success = state.auth.logout(&request.token);

    if success {
        state.activity.log_auth("User logged out", None);
    }

    Json(LogoutResponse { success })
}

/// Validate session endpoint.
pub async fn validate_session(
    State(state): State<AppState>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let token = params.get("token").map(|s| s.as_str()).unwrap_or("");

    match state.auth.validate_session(token) {
        Some(user) => {
            let user_info: UserInfo = user;
            (StatusCode::OK, Json(Some(user_info)))
        }
        None => (StatusCode::UNAUTHORIZED, Json(None::<UserInfo>)),
    }
}

/// Get current user endpoint.
pub async fn get_current_user(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    let auth_header = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let token = auth_header.strip_prefix("Bearer ").unwrap_or(auth_header);

    match state.auth.validate_session(token) {
        Some(user) => {
            let user_info: UserInfo = user;
            (StatusCode::OK, Json(Some(user_info)))
        }
        None => (StatusCode::UNAUTHORIZED, Json(None::<UserInfo>)),
    }
}

// =============================================================================
// Activity Endpoints
// =============================================================================

/// Activity query parameters.
#[derive(Debug, Deserialize)]
pub struct ActivityQuery {
    #[serde(default = "default_limit")]
    pub limit: usize,
    pub activity_type: Option<String>,
    pub user: Option<String>,
}

fn default_limit() -> usize {
    50
}

/// Get recent activities.
pub async fn get_activities(
    State(state): State<AppState>,
    axum::extract::Query(params): axum::extract::Query<ActivityQuery>,
) -> Json<Vec<Activity>> {
    let activities = if let Some(ref user) = params.user {
        state.activity.get_by_user(user, params.limit)
    } else if let Some(ref activity_type) = params.activity_type {
        let at = match activity_type.as_str() {
            "query" => ActivityType::Query,
            "write" => ActivityType::Write,
            "delete" => ActivityType::Delete,
            "config" => ActivityType::Config,
            "node" => ActivityType::Node,
            "auth" => ActivityType::Auth,
            _ => ActivityType::System,
        };
        state.activity.get_by_type(at, params.limit)
    } else {
        state.activity.get_recent(params.limit)
    };

    Json(activities)
}

// =============================================================================
// Key-Value Store Endpoints (REAL IMPLEMENTATION)
// =============================================================================

/// List keys response.
#[derive(Debug, Serialize)]
pub struct ListKeysResponse {
    pub keys: Vec<KvEntry>,
    pub total: usize,
}

/// List all keys - uses real KvStore.
pub async fn list_keys(
    State(state): State<AppState>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Json<ListKeysResponse> {
    let limit = params
        .get("limit")
        .and_then(|s| s.parse().ok())
        .unwrap_or(100);
    let prefix = params.get("prefix").map(|s| s.as_str());

    state.activity.log(ActivityType::Query, "Listed keys");

    let keys = state.kv_store.list(prefix, limit);
    let total = keys.len();

    Json(ListKeysResponse { keys, total })
}

/// Set key request.
#[derive(Debug, Deserialize)]
pub struct SetKeyRequest {
    pub key: String,
    pub value: serde_json::Value,
    pub ttl: Option<u64>,
}

/// Set a key's value - uses real KvStore.
pub async fn set_key(
    State(state): State<AppState>,
    Json(request): Json<SetKeyRequest>,
) -> Json<KvEntry> {
    // Skip the high-volume controller heartbeat keys (controller/*/last_seen, written
    // on every metric batch fleet-wide) — that spam was part of the audit-bloat/OOM.
    // Meaningful KV writes (licenses, config) are still audited.
    if !request.key.contains("last_seen") {
        state
            .activity
            .log_write(&format!("Set key: {}", request.key), None);
    }
    let entry = state.kv_store.set(request.key, request.value, request.ttl);
    Json(entry)
}

/// Get a specific key.
pub async fn get_key(State(state): State<AppState>, Path(key): Path<String>) -> impl IntoResponse {
    match state.kv_store.get(&key) {
        Some(entry) => (StatusCode::OK, Json(Some(entry))),
        None => (StatusCode::NOT_FOUND, Json(None)),
    }
}

/// Delete a key - uses real KvStore.
pub async fn delete_key(
    State(state): State<AppState>,
    Path(key): Path<String>,
) -> impl IntoResponse {
    state
        .activity
        .log(ActivityType::Delete, &format!("Delete key: {}", key));
    match state.kv_store.delete(&key) {
        Some(_) => (
            StatusCode::OK,
            Json(serde_json::json!({"success": true, "key": key})),
        ),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"success": false, "error": "Key not found"})),
        ),
    }
}

/// Keys-only batch request body.
#[derive(Debug, Deserialize)]
pub struct BatchKeysRequest {
    pub keys: Vec<String>,
}

/// Batch set request body.
#[derive(Debug, Deserialize)]
pub struct BatchSetRequest {
    pub entries: Vec<SetKeyRequest>,
}

/// Get many keys at once. Missing keys are omitted from the result.
pub async fn batch_get_keys(
    State(state): State<AppState>,
    Json(request): Json<BatchKeysRequest>,
) -> impl IntoResponse {
    let entries: Vec<KvEntry> = request
        .keys
        .iter()
        .filter_map(|k| state.kv_store.get(k))
        .collect();
    let count = entries.len();
    Json(serde_json::json!({ "entries": entries, "count": count }))
}

/// Set many keys at once (each with an optional TTL).
pub async fn batch_set_keys(
    State(state): State<AppState>,
    Json(request): Json<BatchSetRequest>,
) -> impl IntoResponse {
    let count = request.entries.len();
    for entry in request.entries {
        state.kv_store.set(entry.key, entry.value, entry.ttl);
    }
    state
        .activity
        .log_write(&format!("Batch set {} keys", count), None);
    Json(serde_json::json!({ "success": true, "count": count }))
}

/// Delete many keys at once. Returns the number actually deleted.
pub async fn batch_delete_keys(
    State(state): State<AppState>,
    Json(request): Json<BatchKeysRequest>,
) -> impl IntoResponse {
    let mut deleted = 0usize;
    for key in &request.keys {
        if state.kv_store.delete(key).is_some() {
            deleted += 1;
        }
    }
    state.activity.log(
        ActivityType::Delete,
        &format!("Batch delete {} keys", deleted),
    );
    Json(serde_json::json!({ "success": true, "deleted": deleted }))
}

// =============================================================================
// Document Store Endpoints (REAL IMPLEMENTATION)
// =============================================================================

/// Collection info response.
#[derive(Debug, Serialize)]
pub struct CollectionInfoResponse {
    pub name: String,
    pub document_count: usize,
    pub index_count: usize,
}

/// List collections - uses real DocumentEngine.
pub async fn list_collections(State(state): State<AppState>) -> Json<Vec<CollectionInfoResponse>> {
    state
        .activity
        .log(ActivityType::Query, "Listed collections");

    let collection_names = state.document_engine.list_collections();
    let collections: Vec<CollectionInfoResponse> = collection_names
        .iter()
        .filter_map(|name| {
            state
                .document_engine
                .collection_stats(name)
                .map(|stats| CollectionInfoResponse {
                    name: stats.name,
                    document_count: stats.document_count,
                    index_count: stats.index_count,
                })
        })
        .collect();

    Json(collections)
}

/// Document response.
#[derive(Debug, Serialize)]
pub struct DocumentResponse {
    pub id: String,
    pub collection: String,
    pub data: serde_json::Value,
}

/// Collection query response with full result information.
#[derive(Debug, Serialize)]
pub struct CollectionQueryResponse {
    pub documents: Vec<DocumentResponse>,
    pub total_scanned: usize,
    pub execution_time_ms: u64,
    /// Cursor for the next page; present only when a full page was returned
    /// (i.e. more results may exist). Pass it back as `cursor` to continue.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

/// Get documents in a collection - uses real DocumentEngine.
pub async fn get_collection_documents(
    State(state): State<AppState>,
    Path(collection): Path<String>,
) -> impl IntoResponse {
    state.activity.log(
        ActivityType::Query,
        &format!("Query collection: {}", collection),
    );

    // Use find with empty query to get all documents
    let query = DocQuery::new();
    match state.document_engine.find(&collection, &query) {
        Ok(result) => {
            // Explicit type annotation to use DocQueryResult
            let query_result: &DocQueryResult = &result;
            let docs: Vec<DocumentResponse> = query_result
                .documents
                .iter()
                .map(|doc| DocumentResponse {
                    id: doc.id.to_string(),
                    collection: collection.clone(),
                    data: doc_to_json(doc),
                })
                .collect();
            let response = CollectionQueryResponse {
                documents: docs,
                total_scanned: query_result.total_scanned,
                execution_time_ms: query_result.execution_time_ms,
                next_cursor: None,
            };
            (StatusCode::OK, Json(response))
        }
        Err(_e) => {
            let empty = CollectionQueryResponse {
                documents: vec![],
                total_scanned: 0,
                execution_time_ms: 0,
                next_cursor: None,
            };
            (StatusCode::NOT_FOUND, Json(empty))
        }
    }
}

/// Get a single document by ID.
pub async fn get_document(
    State(state): State<AppState>,
    Path((collection, id)): Path<(String, String)>,
) -> impl IntoResponse {
    state.activity.log(
        ActivityType::Query,
        &format!("Get document: {}/{}", collection, id),
    );

    let doc_id = DocumentId::new(&id);
    match state.document_engine.get(&collection, &doc_id) {
        Ok(Some(doc)) => {
            let response = DocumentResponse {
                id: doc.id.to_string(),
                collection: collection.clone(),
                data: doc_to_json(&doc),
            };
            (StatusCode::OK, Json(serde_json::json!(response)))
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Document not found"})),
        ),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

/// Delete a document from a collection.
pub async fn delete_document(
    State(state): State<AppState>,
    Path((collection, id)): Path<(String, String)>,
) -> impl IntoResponse {
    state.activity.log(
        ActivityType::Delete,
        &format!("Delete document: {}/{}", collection, id),
    );

    let doc_id = DocumentId::new(&id);
    match state.document_engine.delete(&collection, &doc_id) {
        Ok(doc) => {
            state.flush_collection(&collection);
            let response = DocumentResponse {
                id: doc.id.to_string(),
                collection: collection.clone(),
                data: doc_to_json(&doc),
            };
            (
                StatusCode::OK,
                Json(serde_json::json!({"success": true, "deleted": response})),
            )
        }
        Err(e) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"success": false, "error": e.to_string()})),
        ),
    }
}

/// Bulk insert request body.
#[derive(Debug, Deserialize)]
pub struct BulkInsertRequest {
    pub documents: Vec<serde_json::Value>,
}

/// Bulk delete request body.
#[derive(Debug, Deserialize)]
pub struct BulkDeleteRequest {
    pub ids: Vec<String>,
}

/// Insert many documents into a collection in one call.
pub async fn bulk_insert_documents(
    State(state): State<AppState>,
    Path(collection): Path<String>,
    Json(request): Json<BulkInsertRequest>,
) -> impl IntoResponse {
    state.activity.log_write(
        &format!(
            "Bulk insert {} documents into: {}",
            request.documents.len(),
            collection
        ),
        None,
    );

    let docs: Vec<_> = request.documents.into_iter().map(json_to_doc).collect();
    match state.document_engine.insert_many(&collection, docs) {
        Ok(ids) => {
            state.flush_collection(&collection);
            let ids: Vec<String> = ids.iter().map(|i| i.to_string()).collect();
            let count = ids.len();
            (
                StatusCode::CREATED,
                Json(serde_json::json!({"success": true, "ids": ids, "count": count})),
            )
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"success": false, "error": e.to_string()})),
        ),
    }
}

/// Delete many documents by id in one call. Returns the number deleted.
pub async fn bulk_delete_documents(
    State(state): State<AppState>,
    Path(collection): Path<String>,
    Json(request): Json<BulkDeleteRequest>,
) -> impl IntoResponse {
    let mut deleted = 0usize;
    for id in &request.ids {
        let doc_id = DocumentId::new(id);
        if state.document_engine.delete(&collection, &doc_id).is_ok() {
            deleted += 1;
        }
    }
    state.flush_collection(&collection);
    state.activity.log(
        ActivityType::Delete,
        &format!("Bulk delete {} documents from: {}", deleted, collection),
    );
    Json(serde_json::json!({ "success": true, "deleted": deleted }))
}

/// Update document request.
#[derive(Debug, Deserialize)]
pub struct UpdateDocumentRequest {
    pub document: serde_json::Value,
}

/// Update a document in a collection (full replacement).
pub async fn update_document(
    State(state): State<AppState>,
    Path((collection, id)): Path<(String, String)>,
    Json(request): Json<UpdateDocumentRequest>,
) -> impl IntoResponse {
    state
        .activity
        .log_write(&format!("Update document: {}/{}", collection, id), None);

    let doc_id = DocumentId::new(&id);

    // Convert JSON to Document, preserving the ID
    let mut doc = json_to_doc(request.document);
    doc.id = doc_id.clone();

    match state.document_engine.update(&collection, &doc_id, doc) {
        Ok(()) => {
            state.flush_collection(&collection);
            // Fetch the updated document to return it
            match state.document_engine.get(&collection, &doc_id) {
                Ok(Some(updated_doc)) => {
                    let response = DocumentResponse {
                        id: updated_doc.id.to_string(),
                        collection: collection.clone(),
                        data: doc_to_json(&updated_doc),
                    };
                    (
                        StatusCode::OK,
                        Json(serde_json::json!({"success": true, "document": response})),
                    )
                }
                _ => (
                    StatusCode::OK,
                    Json(serde_json::json!({"success": true, "id": id})),
                ),
            }
        }
        Err(e) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"success": false, "error": e.to_string()})),
        ),
    }
}

/// Partially update a document (merge fields).
pub async fn patch_document(
    State(state): State<AppState>,
    Path((collection, id)): Path<(String, String)>,
    Json(request): Json<UpdateDocumentRequest>,
) -> impl IntoResponse {
    state
        .activity
        .log_write(&format!("Patch document: {}/{}", collection, id), None);

    let doc_id = DocumentId::new(&id);

    // First get the existing document
    let existing = match state.document_engine.get(&collection, &doc_id) {
        Ok(Some(doc)) => doc,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"success": false, "error": "Document not found"})),
            );
        }
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"success": false, "error": e.to_string()})),
            );
        }
    };

    // Merge the patch into the existing document
    let mut updated_doc = existing.clone();
    if let serde_json::Value::Object(patch_map) = request.document {
        for (key, value) in patch_map {
            updated_doc.set(&key, json_to_doc_value(value));
        }
    }

    match state
        .document_engine
        .update(&collection, &doc_id, updated_doc)
    {
        Ok(()) => {
            state.flush_collection(&collection);
            // Fetch the updated document to return it
            match state.document_engine.get(&collection, &doc_id) {
                Ok(Some(final_doc)) => {
                    let response = DocumentResponse {
                        id: final_doc.id.to_string(),
                        collection: collection.clone(),
                        data: doc_to_json(&final_doc),
                    };
                    (
                        StatusCode::OK,
                        Json(serde_json::json!({"success": true, "document": response})),
                    )
                }
                _ => (
                    StatusCode::OK,
                    Json(serde_json::json!({"success": true, "id": id})),
                ),
            }
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"success": false, "error": e.to_string()})),
        ),
    }
}

/// Create collection request.
#[derive(Debug, Deserialize)]
pub struct CreateCollectionRequest {
    pub name: String,
}

/// Create a new collection.
pub async fn create_collection(
    State(state): State<AppState>,
    Json(request): Json<CreateCollectionRequest>,
) -> impl IntoResponse {
    state
        .activity
        .log_write(&format!("Create collection: {}", request.name), None);

    match state.document_engine.create_collection(&request.name) {
        Ok(()) => (
            StatusCode::CREATED,
            Json(serde_json::json!({"success": true, "collection": request.name})),
        ),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"success": false, "error": e.to_string()})),
        ),
    }
}

/// Drop a collection and all its documents.
pub async fn delete_collection(
    State(state): State<AppState>,
    axum::extract::Path(name): axum::extract::Path<String>,
) -> impl IntoResponse {
    state
        .activity
        .log_write(&format!("Drop collection: {name}"), None);

    match state.document_engine.drop_collection(&name) {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({"success": true, "collection": name})),
        ),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"success": false, "error": e.to_string()})),
        ),
    }
}

/// Insert document request.
#[derive(Debug, Deserialize)]
pub struct InsertDocumentRequest {
    /// Optional explicit document ID (takes precedence over _id in document)
    pub id: Option<String>,
    pub document: serde_json::Value,
}

/// Insert a document into a collection.
pub async fn insert_document(
    State(state): State<AppState>,
    Path(collection): Path<String>,
    Json(request): Json<InsertDocumentRequest>,
) -> impl IntoResponse {
    state
        .activity
        .log_write(&format!("Insert document into: {}", collection), None);

    // If id is provided at top level, inject it into the document
    let doc_json = if let Some(id) = request.id {
        let mut doc = request.document;
        if let serde_json::Value::Object(ref mut map) = doc {
            map.insert("_id".to_string(), serde_json::Value::String(id));
        }
        doc
    } else {
        request.document
    };

    let doc = json_to_doc(doc_json);
    match state.document_engine.insert(&collection, doc) {
        Ok(id) => {
            state.flush_collection(&collection);
            (
                StatusCode::CREATED,
                Json(serde_json::json!({"success": true, "id": id.to_string()})),
            )
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"success": false, "error": e.to_string()})),
        ),
    }
}

/// Helper to convert Document to JSON.
fn doc_to_json(doc: &Document) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    map.insert(
        "_id".to_string(),
        serde_json::Value::String(doc.id.to_string()),
    );
    // Add document fields
    for (key, value) in &doc.data {
        map.insert(key.clone(), aegis_doc_value_to_json(value));
    }
    serde_json::Value::Object(map)
}

/// Helper to convert aegis_document::Value to JSON.
fn aegis_doc_value_to_json(value: &aegis_document::Value) -> serde_json::Value {
    match value {
        aegis_document::Value::Null => serde_json::Value::Null,
        aegis_document::Value::Bool(b) => serde_json::Value::Bool(*b),
        aegis_document::Value::Int(i) => serde_json::Value::Number((*i).into()),
        aegis_document::Value::Float(f) => serde_json::Number::from_f64(*f)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        aegis_document::Value::String(s) => serde_json::Value::String(s.clone()),
        aegis_document::Value::Array(arr) => {
            serde_json::Value::Array(arr.iter().map(aegis_doc_value_to_json).collect())
        }
        aegis_document::Value::Object(obj) => {
            let map: serde_json::Map<String, serde_json::Value> = obj
                .iter()
                .map(|(k, v)| (k.clone(), aegis_doc_value_to_json(v)))
                .collect();
            serde_json::Value::Object(map)
        }
    }
}

/// Helper to convert JSON to Document.
fn json_to_doc(json: serde_json::Value) -> Document {
    // Check for _id or id field to use as document ID
    // Priority: _id > id
    let doc_id = json
        .get("_id")
        .or_else(|| json.get("id"))
        .and_then(|v| v.as_str());

    let mut doc = match doc_id {
        Some(id) => Document::with_id(id),
        None => Document::new(),
    };

    if let serde_json::Value::Object(map) = json {
        for (key, value) in map {
            // Only skip _id (internal ID field), preserve all other fields including "id"
            if key != "_id" {
                doc.set(&key, json_to_doc_value(value));
            }
        }
    }
    doc
}

/// Helper to convert JSON to aegis_document::Value.
fn json_to_doc_value(json: serde_json::Value) -> aegis_document::Value {
    match json {
        serde_json::Value::Null => aegis_document::Value::Null,
        serde_json::Value::Bool(b) => aegis_document::Value::Bool(b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                aegis_document::Value::Int(i)
            } else if let Some(f) = n.as_f64() {
                aegis_document::Value::Float(f)
            } else {
                aegis_document::Value::Null
            }
        }
        serde_json::Value::String(s) => aegis_document::Value::String(s),
        serde_json::Value::Array(arr) => {
            aegis_document::Value::Array(arr.into_iter().map(json_to_doc_value).collect())
        }
        serde_json::Value::Object(map) => aegis_document::Value::Object(
            map.into_iter()
                .map(|(k, v)| (k, json_to_doc_value(v)))
                .collect(),
        ),
    }
}

/// List documents in a collection (GET /collections/:name/documents).
pub async fn list_collection_documents(
    State(state): State<AppState>,
    Path(collection): Path<String>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    state.activity.log(
        ActivityType::Query,
        &format!("List documents in: {}", collection),
    );

    let limit: Option<usize> = params.get("limit").and_then(|s| s.parse().ok());
    // `?cursor=` (opaque) supplies the offset, overriding `?skip=`.
    let effective_skip = params
        .get("cursor")
        .and_then(|c| decode_cursor(c))
        .or_else(|| params.get("skip").and_then(|s| s.parse().ok()))
        .unwrap_or(0);

    let mut query = DocQuery::new();
    if let Some(limit) = limit {
        query = query.with_limit(limit);
    }
    if effective_skip > 0 {
        query = query.with_skip(effective_skip);
    }

    match state.document_engine.find(&collection, &query) {
        Ok(result) => {
            let returned = result.documents.len();
            let next_cursor = match limit {
                Some(limit) if limit > 0 && returned >= limit => {
                    Some(encode_cursor(effective_skip + returned))
                }
                _ => None,
            };
            let docs: Vec<DocumentResponse> = result
                .documents
                .iter()
                .map(|doc| DocumentResponse {
                    id: doc.id.to_string(),
                    collection: collection.clone(),
                    data: doc_to_json(doc),
                })
                .collect();
            let response = CollectionQueryResponse {
                documents: docs,
                total_scanned: result.total_scanned,
                execution_time_ms: result.execution_time_ms,
                next_cursor,
            };
            (StatusCode::OK, Json(response))
        }
        Err(_e) => {
            let empty = CollectionQueryResponse {
                documents: vec![],
                total_scanned: 0,
                execution_time_ms: 0,
                next_cursor: None,
            };
            (StatusCode::NOT_FOUND, Json(empty))
        }
    }
}

/// Document query request with MongoDB-style filter operators.
#[derive(Debug, Deserialize)]
pub struct DocumentQueryRequest {
    #[serde(default)]
    pub filter: serde_json::Value,
    pub limit: Option<usize>,
    pub skip: Option<usize>,
    pub sort: Option<SortSpec>,
    /// Opaque pagination cursor from a previous response's `next_cursor`.
    /// When present it supplies the starting offset (overriding `skip`).
    #[serde(default)]
    pub cursor: Option<String>,
}

/// Encode an offset into an opaque (base64url) pagination cursor.
///
/// This is offset-backed: like SQL `OFFSET`, a cursor is only stable if the
/// underlying result ordering is stable between calls (provide a `sort`).
pub fn encode_cursor(offset: usize) -> String {
    data_encoding::BASE64URL_NOPAD.encode(offset.to_string().as_bytes())
}

/// Decode an opaque pagination cursor back into an offset.
pub fn decode_cursor(cursor: &str) -> Option<usize> {
    let bytes = data_encoding::BASE64URL_NOPAD
        .decode(cursor.as_bytes())
        .ok()?;
    std::str::from_utf8(&bytes).ok()?.parse::<usize>().ok()
}

/// Sort specification for queries.
#[derive(Debug, Deserialize)]
pub struct SortSpec {
    pub field: String,
    #[serde(default = "default_ascending")]
    pub ascending: bool,
}

fn default_ascending() -> bool {
    true
}

/// Query documents with filter operators (POST /collections/:name/query).
/// Supports MongoDB-style operators: $eq, $ne, $gt, $gte, $lt, $lte, $in, $nin, $exists, $regex, $and, $or
pub async fn query_collection_documents(
    State(state): State<AppState>,
    Path(collection): Path<String>,
    Json(request): Json<DocumentQueryRequest>,
) -> impl IntoResponse {
    state.activity.log(
        ActivityType::Query,
        &format!("Query collection: {}", collection),
    );

    // Parse the filter into Query filters
    let mut query = DocQuery::new();

    if let serde_json::Value::Object(filter_map) = &request.filter {
        for (field, condition) in filter_map {
            if let Some(filter) = parse_filter_condition(field, condition) {
                query = query.with_filter(filter);
            }
        }
    }

    // A cursor (if present and valid) supplies the starting offset, overriding
    // `skip`; otherwise fall back to `skip`.
    let effective_skip = request
        .cursor
        .as_deref()
        .and_then(decode_cursor)
        .or(request.skip)
        .unwrap_or(0);
    if effective_skip > 0 {
        query = query.with_skip(effective_skip);
    }
    if let Some(limit) = request.limit {
        query = query.with_limit(limit);
    }
    if let Some(ref sort) = request.sort {
        query = query.with_sort(&sort.field, sort.ascending);
    }

    match state.document_engine.find(&collection, &query) {
        Ok(result) => {
            let returned = result.documents.len();
            // Only emit a next cursor when a full page was returned (a limit was
            // set and we filled it) — a partial/last page has no continuation.
            let next_cursor = match request.limit {
                Some(limit) if limit > 0 && returned >= limit => {
                    Some(encode_cursor(effective_skip + returned))
                }
                _ => None,
            };
            let docs: Vec<DocumentResponse> = result
                .documents
                .iter()
                .map(|doc| DocumentResponse {
                    id: doc.id.to_string(),
                    collection: collection.clone(),
                    data: doc_to_json(doc),
                })
                .collect();
            let response = CollectionQueryResponse {
                documents: docs,
                total_scanned: result.total_scanned,
                execution_time_ms: result.execution_time_ms,
                next_cursor,
            };
            (StatusCode::OK, Json(response))
        }
        Err(_) => {
            let empty = CollectionQueryResponse {
                documents: vec![],
                total_scanned: 0,
                execution_time_ms: 0,
                next_cursor: None,
            };
            (StatusCode::NOT_FOUND, Json(empty))
        }
    }
}

/// Parse a filter condition with MongoDB-style operators.
fn parse_filter_condition(
    field: &str,
    condition: &serde_json::Value,
) -> Option<aegis_document::query::Filter> {
    use aegis_document::query::Filter;

    match condition {
        // Direct value comparison (implicit $eq)
        serde_json::Value::Null
        | serde_json::Value::Bool(_)
        | serde_json::Value::Number(_)
        | serde_json::Value::String(_) => Some(Filter::Eq {
            field: field.to_string(),
            value: json_to_doc_value(condition.clone()),
        }),
        // Operator object
        serde_json::Value::Object(ops) => {
            // Handle $and and $or at the top level
            if field == "$and" {
                if let serde_json::Value::Array(arr) = condition {
                    let filters: Vec<Filter> = arr
                        .iter()
                        .filter_map(|item| {
                            if let serde_json::Value::Object(obj) = item {
                                obj.iter()
                                    .filter_map(|(k, v)| parse_filter_condition(k, v))
                                    .next()
                            } else {
                                None
                            }
                        })
                        .collect();
                    return Some(Filter::And(filters));
                }
                return None;
            }
            if field == "$or" {
                if let serde_json::Value::Array(arr) = condition {
                    let filters: Vec<Filter> = arr
                        .iter()
                        .filter_map(|item| {
                            if let serde_json::Value::Object(obj) = item {
                                obj.iter()
                                    .filter_map(|(k, v)| parse_filter_condition(k, v))
                                    .next()
                            } else {
                                None
                            }
                        })
                        .collect();
                    return Some(Filter::Or(filters));
                }
                return None;
            }

            // Single operator or multiple operators on same field
            let mut filters: Vec<Filter> = Vec::new();

            for (op, value) in ops {
                let filter = match op.as_str() {
                    "$eq" => Some(Filter::Eq {
                        field: field.to_string(),
                        value: json_to_doc_value(value.clone()),
                    }),
                    "$ne" => Some(Filter::Ne {
                        field: field.to_string(),
                        value: json_to_doc_value(value.clone()),
                    }),
                    "$gt" => Some(Filter::Gt {
                        field: field.to_string(),
                        value: json_to_doc_value(value.clone()),
                    }),
                    "$gte" => Some(Filter::Gte {
                        field: field.to_string(),
                        value: json_to_doc_value(value.clone()),
                    }),
                    "$lt" => Some(Filter::Lt {
                        field: field.to_string(),
                        value: json_to_doc_value(value.clone()),
                    }),
                    "$lte" => Some(Filter::Lte {
                        field: field.to_string(),
                        value: json_to_doc_value(value.clone()),
                    }),
                    "$in" => {
                        if let serde_json::Value::Array(arr) = value {
                            Some(Filter::In {
                                field: field.to_string(),
                                values: arr.iter().map(|v| json_to_doc_value(v.clone())).collect(),
                            })
                        } else {
                            None
                        }
                    }
                    "$nin" => {
                        if let serde_json::Value::Array(arr) = value {
                            Some(Filter::Nin {
                                field: field.to_string(),
                                values: arr.iter().map(|v| json_to_doc_value(v.clone())).collect(),
                            })
                        } else {
                            None
                        }
                    }
                    "$exists" => {
                        if let serde_json::Value::Bool(b) = value {
                            Some(Filter::Exists {
                                field: field.to_string(),
                                exists: *b,
                            })
                        } else {
                            None
                        }
                    }
                    "$regex" => {
                        if let serde_json::Value::String(pattern) = value {
                            Some(Filter::Regex {
                                field: field.to_string(),
                                pattern: pattern.clone(),
                            })
                        } else {
                            None
                        }
                    }
                    "$contains" => {
                        if let serde_json::Value::String(s) = value {
                            Some(Filter::Contains {
                                field: field.to_string(),
                                value: s.clone(),
                            })
                        } else {
                            None
                        }
                    }
                    "$startsWith" => {
                        if let serde_json::Value::String(s) = value {
                            Some(Filter::StartsWith {
                                field: field.to_string(),
                                value: s.clone(),
                            })
                        } else {
                            None
                        }
                    }
                    "$endsWith" => {
                        if let serde_json::Value::String(s) = value {
                            Some(Filter::EndsWith {
                                field: field.to_string(),
                                value: s.clone(),
                            })
                        } else {
                            None
                        }
                    }
                    _ => None,
                };

                if let Some(f) = filter {
                    filters.push(f);
                }
            }

            // If multiple operators on same field, combine with AND
            match filters.len() {
                0 => None,
                1 => filters.into_iter().next(),
                _ => Some(Filter::And(filters)),
            }
        }
        serde_json::Value::Array(_) => None,
    }
}

// =============================================================================
// Time Series Endpoints (REAL IMPLEMENTATION)
// =============================================================================

/// Register metric request.
#[derive(Debug, Deserialize)]
pub struct RegisterMetricRequest {
    pub name: String,
    #[serde(default = "default_metric_type")]
    pub metric_type: String,
    pub description: Option<String>,
    pub unit: Option<String>,
}

fn default_metric_type() -> String {
    "gauge".to_string()
}

/// Register a new metric with type information.
pub async fn register_metric(
    State(state): State<AppState>,
    Json(request): Json<RegisterMetricRequest>,
) -> impl IntoResponse {
    state
        .activity
        .log_write(&format!("Register metric: {}", request.name), None);

    let metric_type = match request.metric_type.to_lowercase().as_str() {
        "counter" => MetricType::Counter,
        "gauge" => MetricType::Gauge,
        "histogram" => MetricType::Histogram,
        "summary" => MetricType::Summary,
        _ => MetricType::Gauge,
    };

    let mut metric = Metric::new(&request.name);
    metric.metric_type = metric_type;
    metric.description = request.description;
    metric.unit = request.unit;

    match state.timeseries_engine.register_metric(metric) {
        Ok(()) => (
            StatusCode::CREATED,
            Json(serde_json::json!({
                "success": true,
                "metric": request.name
            })),
        ),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "success": false,
                "error": e.to_string()
            })),
        ),
    }
}

/// Write time series data request.
#[derive(Debug, Deserialize)]
pub struct WriteTimeSeriesRequest {
    pub metric: String,
    #[serde(default)]
    pub tags: std::collections::HashMap<String, String>,
    pub value: f64,
    pub timestamp: Option<i64>,
}

/// Write time series data.
pub async fn write_timeseries(
    State(state): State<AppState>,
    Json(request): Json<WriteTimeSeriesRequest>,
) -> impl IntoResponse {
    // Metric writes are NOT audited: the whole fleet writes every 15s, and
    // hash-chaining every point into the in-RAM ledger was the audit-bloat +
    // 97%-CPU + OOM driver. Security/admin events are still audited elsewhere.
    let mut tags = Tags::new();
    for (k, v) in request.tags {
        tags.insert(&k, &v);
    }

    let point = if let Some(ts) = request.timestamp {
        DataPoint {
            timestamp: chrono::DateTime::from_timestamp(ts, 0).unwrap_or_else(Utc::now),
            value: request.value,
        }
    } else {
        DataPoint {
            timestamp: Utc::now(),
            value: request.value,
        }
    };

    match state.timeseries_engine.write(&request.metric, tags, point) {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"success": true}))),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"success": false, "error": e.to_string()})),
        ),
    }
}

/// Query time series request.
#[derive(Debug, Deserialize)]
pub struct QueryTimeSeriesRequest {
    pub metric: String,
    #[serde(default)]
    pub tags: Option<std::collections::HashMap<String, String>>,
    pub start: Option<i64>,
    pub end: Option<i64>,
    pub limit: Option<usize>,
    /// Optional downsample bucket in seconds (mean per bucket) for long windows.
    #[serde(default)]
    pub step: Option<i64>,
}

/// Time series data response.
#[derive(Debug, Serialize)]
pub struct TimeSeriesResponse {
    pub metric: String,
    pub series: Vec<SeriesResponse>,
    pub points_returned: usize,
    pub query_time_ms: u64,
}

#[derive(Debug, Serialize)]
pub struct SeriesResponse {
    pub tags: std::collections::HashMap<String, String>,
    pub points: Vec<PointResponse>,
}

#[derive(Debug, Serialize)]
pub struct PointResponse {
    pub timestamp: i64,
    pub value: f64,
}

/// Query time series data.
pub async fn query_timeseries(
    State(state): State<AppState>,
    Json(request): Json<QueryTimeSeriesRequest>,
) -> impl IntoResponse {
    state.activity.log(
        ActivityType::Query,
        &format!("Query timeseries: {}", request.metric),
    );

    // Honour the requested window (epoch seconds; millisecond stamps are accepted
    // too). Default: the last 24h. Previously start/end were parsed but ignored, so
    // no client could ever read past a day regardless of retention.
    let to_dt = |v: i64| {
        let secs = if v > 10_000_000_000 { v / 1000 } else { v };
        chrono::TimeZone::timestamp_opt(&Utc, secs, 0).single()
    };
    let end = request.end.and_then(to_dt).unwrap_or_else(Utc::now);
    let start = request
        .start
        .and_then(to_dt)
        .filter(|s| *s < end)
        .unwrap_or_else(|| end - Duration::hours(24));
    let mut query = TimeSeriesQuery::new(&request.metric, start, end);

    if let Some(step) = request.step.filter(|s| *s > 0) {
        query = query.downsample(
            Duration::seconds(step),
            aegis_timeseries::AggregateFunction::Avg,
        );
    }

    if let Some(limit) = request.limit {
        query = query.with_limit(limit);
    }

    if let Some(ref tags_map) = request.tags {
        let mut tags = Tags::new();
        for (k, v) in tags_map {
            tags.insert(k, v);
        }
        query = query.with_tags(tags);
    }

    let result = state.timeseries_engine.query(&query);

    let series: Vec<SeriesResponse> = result
        .series
        .iter()
        .map(|s| SeriesResponse {
            tags: s.tags.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
            points: s
                .points
                .iter()
                .map(|p| PointResponse {
                    timestamp: p.timestamp.timestamp(),
                    value: p.value,
                })
                .collect(),
        })
        .collect();

    let response = TimeSeriesResponse {
        metric: request.metric,
        series,
        points_returned: result.points_returned,
        query_time_ms: result.query_time_ms,
    };

    (StatusCode::OK, Json(response))
}

/// Metric info response with full type information.
#[derive(Debug, Serialize)]
pub struct MetricInfoResponse {
    pub name: String,
    pub metric_type: String,
    pub description: Option<String>,
    pub unit: Option<String>,
}

impl From<&Metric> for MetricInfoResponse {
    fn from(m: &Metric) -> Self {
        Self {
            name: m.name.clone(),
            metric_type: match m.metric_type {
                MetricType::Counter => "counter".to_string(),
                MetricType::Gauge => "gauge".to_string(),
                MetricType::Histogram => "histogram".to_string(),
                MetricType::Summary => "summary".to_string(),
            },
            description: m.description.clone(),
            unit: m.unit.clone(),
        }
    }
}

/// List metrics with full type information.
pub async fn list_metrics(State(state): State<AppState>) -> Json<Vec<MetricInfoResponse>> {
    state.activity.log(ActivityType::Query, "Listed metrics");
    let metrics = state.timeseries_engine.list_metrics();
    Json(metrics.iter().map(MetricInfoResponse::from).collect())
}

// =============================================================================
// Streaming Endpoints (REAL IMPLEMENTATION)
// =============================================================================

/// Create channel request.
#[derive(Debug, Deserialize)]
pub struct CreateChannelRequest {
    pub id: String,
}

/// Create a streaming channel.
pub async fn create_channel(
    State(state): State<AppState>,
    Json(request): Json<CreateChannelRequest>,
) -> impl IntoResponse {
    state
        .activity
        .log_write(&format!("Create channel: {}", request.id), None);

    match state.streaming_engine.create_channel(request.id.clone()) {
        Ok(()) => (
            StatusCode::CREATED,
            Json(serde_json::json!({"success": true, "channel": request.id})),
        ),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"success": false, "error": e.to_string()})),
        ),
    }
}

/// List channels.
pub async fn list_channels(State(state): State<AppState>) -> Json<Vec<String>> {
    state.activity.log(ActivityType::Query, "Listed channels");
    let channels: Vec<String> = state
        .streaming_engine
        .list_channels()
        .into_iter()
        .map(|c| c.to_string())
        .collect();
    Json(channels)
}

/// Publish event request.
#[derive(Debug, Deserialize)]
pub struct PublishEventRequest {
    pub channel: String,
    pub event_type: String,
    pub source: String,
    pub data: serde_json::Value,
}

/// Publish an event to a channel.
pub async fn publish_event(
    State(state): State<AppState>,
    Json(request): Json<PublishEventRequest>,
) -> impl IntoResponse {
    state
        .activity
        .log_write(&format!("Publish to channel: {}", request.channel), None);

    let event_type = match request.event_type.as_str() {
        "created" => StreamEventType::Created,
        "updated" => StreamEventType::Updated,
        "deleted" => StreamEventType::Deleted,
        _ => StreamEventType::Custom(request.event_type.clone()),
    };

    let data = match request.data {
        serde_json::Value::String(s) => EventData::String(s),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                EventData::Int(i)
            } else if let Some(f) = n.as_f64() {
                EventData::Float(f)
            } else {
                EventData::Null
            }
        }
        serde_json::Value::Bool(b) => EventData::Bool(b),
        serde_json::Value::Null => EventData::Null,
        _ => EventData::Json(request.data.clone()),
    };

    let event = Event::new(event_type, &request.source, data);
    let channel_id = ChannelId::new(&request.channel);

    match state.streaming_engine.publish(&channel_id, event) {
        Ok(receivers) => (
            StatusCode::OK,
            Json(serde_json::json!({"success": true, "receivers": receivers})),
        ),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"success": false, "error": e.to_string()})),
        ),
    }
}

/// Get channel history.
pub async fn get_channel_history(
    State(state): State<AppState>,
    Path(channel): Path<String>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let count = params
        .get("count")
        .and_then(|s| s.parse().ok())
        .unwrap_or(100);
    let channel_id = ChannelId::new(&channel);

    match state.streaming_engine.get_history(&channel_id, count) {
        Ok(events) => {
            let event_data: Vec<serde_json::Value> = events
                .iter()
                .map(|e| {
                    serde_json::json!({
                        "id": e.id.to_string(),
                        "event_type": format!("{:?}", e.event_type),
                        "source": e.source,
                        "timestamp": e.timestamp,
                    })
                })
                .collect();
            (
                StatusCode::OK,
                Json(serde_json::json!({"events": event_data})),
            )
        }
        Err(e) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

/// Subscribe to a streaming channel as a Server-Sent Events stream.
///
/// `GET /api/v1/streaming/channels/:channel/sse` opens a long-lived
/// `text/event-stream`; every event published to the channel is pushed to the
/// client in real time (one SSE `data:` frame per event). The channel is created
/// if it does not exist. Lagged events (slow consumer) are skipped; the stream
/// ends when the channel is closed.
pub async fn stream_channel_sse(
    State(state): State<AppState>,
    Path(channel): Path<String>,
) -> axum::response::Sse<
    impl futures_util::Stream<Item = Result<axum::response::sse::Event, std::convert::Infallible>>,
> {
    use aegis_streaming::channel::ChannelError;

    // Ensure the channel exists (idempotent), then subscribe.
    let _ = state.streaming_engine.create_channel(channel.clone());
    let channel_id = ChannelId::new(&channel);
    let receiver = state
        .streaming_engine
        .subscribe(
            &channel_id,
            aegis_streaming::subscriber::SubscriberId::generate(),
        )
        .ok();

    let stream = futures_util::stream::unfold(receiver, |state| async move {
        // `None` (subscribe failed, or stream ended) terminates the SSE stream.
        let mut rx = state?;
        loop {
            match rx.recv().await {
                Ok(event) => {
                    let data = serde_json::to_string(&event).unwrap_or_else(|_| "{}".to_string());
                    let sse = axum::response::sse::Event::default()
                        .event("message")
                        .data(data);
                    return Some((Ok(sse), Some(rx)));
                }
                // Slow consumer fell behind — skip dropped events and continue.
                Err(ChannelError::Lagged(_)) => continue,
                // Channel closed or any other error — end the stream.
                Err(_) => return None,
            }
        }
    });

    axum::response::Sse::new(stream).keep_alive(axum::response::sse::KeepAlive::default())
}

// =============================================================================
// Graph Database Endpoints
// =============================================================================

/// Graph data response (uses GraphNode and GraphEdge from state module).
#[derive(Debug, Serialize)]
pub struct GraphDataResponse {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

/// Get graph data.
pub async fn get_graph_data(State(state): State<AppState>) -> Json<GraphDataResponse> {
    state.activity.log(ActivityType::Query, "Query graph data");

    let (nodes, edges) = state.graph_store.get_all();

    Json(GraphDataResponse { nodes, edges })
}

// =============================================================================
// Query Builder Endpoints
// =============================================================================

/// Query execution request.
#[derive(Debug, Deserialize)]
pub struct ExecuteQueryRequest {
    pub query: String,
    #[serde(default)]
    pub database: Option<String>,
}

/// Query execution response.
#[derive(Debug, Serialize)]
pub struct ExecuteQueryResponse {
    pub success: bool,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<serde_json::Value>>,
    pub row_count: usize,
    pub execution_time_ms: u64,
    pub error: Option<String>,
}

/// Execute a query from the query builder.
pub async fn execute_builder_query(
    State(state): State<AppState>,
    Json(request): Json<ExecuteQueryRequest>,
) -> Json<ExecuteQueryResponse> {
    let start = std::time::Instant::now();
    state.activity.log_query(&request.query, 0, None);

    // Execute through the real query engine
    match state
        .query_engine
        .execute(&request.query, request.database.as_deref())
    {
        Ok(result) => Json(ExecuteQueryResponse {
            success: true,
            columns: result.columns,
            rows: result.rows,
            row_count: result.rows_affected as usize,
            execution_time_ms: start.elapsed().as_millis() as u64,
            error: None,
        }),
        Err(e) => Json(ExecuteQueryResponse {
            success: false,
            columns: vec![],
            rows: vec![],
            row_count: 0,
            execution_time_ms: start.elapsed().as_millis() as u64,
            error: Some(e.to_string()),
        }),
    }
}

// =============================================================================
// Node Action Endpoints
// =============================================================================

/// Generic action response.
#[derive(Debug, Serialize)]
pub struct NodeActionResponse {
    pub success: bool,
    pub message: String,
    pub node_id: String,
}

/// Restart a node by sending a shutdown signal to the target.
/// PM2's autorestart will bring it back up.
pub async fn restart_node(
    State(state): State<AppState>,
    Path(node_id): Path<String>,
) -> Json<NodeActionResponse> {
    state
        .activity
        .log_node(&format!("Restarting node: {}", node_id));

    // Find the peer's address
    let peers = state.admin.get_peers();
    let peer = peers
        .iter()
        .find(|p| p.id == node_id || p.name.as_deref() == Some(&node_id));

    if let Some(peer) = peer {
        let address = peer.address.clone();
        // Send shutdown request to the target node asynchronously
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .unwrap_or_default();
        let url = format!("{}/api/v1/cluster/shutdown", address);
        tokio::spawn(async move {
            let _ = client.post(&url).send().await;
        });

        Json(NodeActionResponse {
            success: true,
            message: format!(
                "Node {} restart initiated at {}. PM2 will auto-restart.",
                node_id, address
            ),
            node_id,
        })
    } else {
        Json(NodeActionResponse {
            success: false,
            message: format!("Node {} not found in cluster peers.", node_id),
            node_id,
        })
    }
}

/// Drain a node (mark as leaving, stop routing traffic).
pub async fn drain_node(
    State(state): State<AppState>,
    Path(node_id): Path<String>,
) -> Json<NodeActionResponse> {
    state
        .activity
        .log_node(&format!("Draining node: {}", node_id));

    // Mark the node as leaving in the peer list
    let peers = state.admin.get_peers();
    let found = peers
        .iter()
        .any(|p| p.id == node_id || p.name.as_deref() == Some(&node_id));

    if found {
        // Update peer status to Leaving so the router stops sending traffic
        state.admin.mark_peer_offline(&node_id);

        Json(NodeActionResponse {
            success: true,
            message: format!(
                "Node {} marked as draining. Traffic will be redirected to other nodes.",
                node_id
            ),
            node_id,
        })
    } else {
        Json(NodeActionResponse {
            success: false,
            message: format!("Node {} not found in cluster peers.", node_id),
            node_id,
        })
    }
}

/// Remove a node from the cluster.
pub async fn remove_node(
    State(state): State<AppState>,
    Path(node_id): Path<String>,
) -> impl IntoResponse {
    state
        .activity
        .log_node(&format!("Removing node from cluster: {}", node_id));

    // Actually remove the peer from the admin service
    state.admin.remove_peer(&node_id);

    (
        StatusCode::OK,
        Json(NodeActionResponse {
            success: true,
            message: format!("Node {} has been removed from the cluster.", node_id),
            node_id,
        }),
    )
}

/// Graceful shutdown endpoint - called by restart_node on the target.
/// Flushes data and exits; PM2 auto-restarts the process.
pub async fn cluster_shutdown(State(state): State<AppState>) -> impl IntoResponse {
    state
        .activity
        .log_node("Graceful shutdown initiated via cluster API");

    // Flush timeseries data + checkpoint the full store to disk so a restart comes
    // back FULLY INTACT (KV + documents already persist per-mutation; this is the
    // belt-and-suspenders consistency checkpoint for a graceful bounce).
    state.timeseries_engine.flush();
    if let Err(e) = state.save_to_disk() {
        state
            .activity
            .log_node(&format!("save_to_disk on shutdown failed: {e}"));
    }

    // Give a brief moment for the response to be sent
    tokio::spawn(async {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        std::process::exit(0);
    });

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "status": "shutting_down",
            "message": "Node will restart via PM2"
        })),
    )
}

/// Node logs entry.
#[derive(Debug, Serialize)]
pub struct NodeLogEntry {
    pub timestamp: String,
    pub level: String,
    pub message: String,
}

/// Node logs response.
#[derive(Debug, Serialize)]
pub struct NodeLogsResponse {
    pub node_id: String,
    pub logs: Vec<NodeLogEntry>,
    pub total: usize,
}

/// Get logs for a specific node.
pub async fn get_node_logs(
    State(state): State<AppState>,
    Path(node_id): Path<String>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Json<NodeLogsResponse> {
    let limit: usize = params
        .get("limit")
        .and_then(|l| l.parse().ok())
        .unwrap_or(100);

    // Get real activity logs from the server
    let activities = state.activity.get_recent(limit);
    let logs: Vec<NodeLogEntry> = activities
        .iter()
        .map(|a| NodeLogEntry {
            timestamp: a.timestamp.clone(),
            level: match a.activity_type {
                ActivityType::Auth | ActivityType::System => "INFO".to_string(),
                ActivityType::Write | ActivityType::Delete => "WARN".to_string(),
                ActivityType::Query | ActivityType::Config | ActivityType::Node => {
                    "INFO".to_string()
                }
            },
            message: a.description.clone(),
        })
        .collect();

    let total = logs.len();
    Json(NodeLogsResponse {
        node_id,
        logs: logs.into_iter().take(limit).collect(),
        total,
    })
}

// =============================================================================
// Settings Endpoints
// =============================================================================

/// Server settings structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
pub async fn get_settings(State(state): State<AppState>) -> Json<ServerSettings> {
    state
        .activity
        .log(ActivityType::Config, "Retrieved server settings");
    let settings = state.settings.read().await;
    Json(settings.clone())
}

/// Update server settings.
pub async fn update_settings(
    State(state): State<AppState>,
    Json(new_settings): Json<ServerSettings>,
) -> impl IntoResponse {
    state.activity.log_config("Updated server settings", None);
    let mut settings = state.settings.write().await;
    *settings = new_settings.clone();
    drop(settings);
    state.save_settings().await;
    (
        StatusCode::OK,
        Json(serde_json::json!({"success": true, "settings": new_settings})),
    )
}

// =============================================================================
// User Management Endpoints
// =============================================================================

/// User info response for list users.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
pub async fn list_users(State(state): State<AppState>) -> Json<Vec<UserListItem>> {
    state.activity.log(ActivityType::Query, "Listed users");
    let users = state.auth.list_users();
    let user_list: Vec<UserListItem> = users
        .iter()
        .map(|u| UserListItem {
            id: u.id.clone(),
            username: u.username.clone(),
            email: u.email.clone(),
            role: format!("{:?}", u.role).to_lowercase(),
            mfa_enabled: u.mfa_enabled,
            enabled: true,
            created_at: u.created_at.clone(),
            last_login: None,
        })
        .collect();
    Json(user_list)
}

/// Create user request.
#[derive(Debug, Deserialize)]
pub struct CreateUserRequest {
    pub username: String,
    pub email: String,
    pub password: String,
    pub role: String,
}

/// Create a new user.
pub async fn create_user(
    State(state): State<AppState>,
    Json(request): Json<CreateUserRequest>,
) -> impl IntoResponse {
    state
        .activity
        .log_write(&format!("Create user: {}", request.username), None);

    match state.auth.create_user(
        &request.username,
        &request.email,
        &request.password,
        &request.role,
    ) {
        Ok(user) => (
            StatusCode::CREATED,
            Json(serde_json::json!({"success": true, "user": user})),
        ),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"success": false, "error": e})),
        ),
    }
}

/// Update user request.
#[derive(Debug, Deserialize)]
pub struct UpdateUserRequest {
    pub email: Option<String>,
    pub role: Option<String>,
    pub enabled: Option<bool>,
    pub password: Option<String>,
}

/// Update a user.
pub async fn update_user(
    State(state): State<AppState>,
    Path(username): Path<String>,
    Json(request): Json<UpdateUserRequest>,
) -> impl IntoResponse {
    state
        .activity
        .log_write(&format!("Update user: {}", username), None);

    match state
        .auth
        .update_user(&username, request.email, request.role, request.password)
    {
        Ok(user) => (
            StatusCode::OK,
            Json(serde_json::json!({"success": true, "user": user})),
        ),
        Err(e) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"success": false, "error": e})),
        ),
    }
}

/// Delete a user.
pub async fn delete_user(
    State(state): State<AppState>,
    Path(username): Path<String>,
) -> impl IntoResponse {
    state
        .activity
        .log(ActivityType::Delete, &format!("Delete user: {}", username));

    match state.auth.delete_user(&username) {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"success": true}))),
        Err(e) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"success": false, "error": e})),
        ),
    }
}

// =============================================================================
// Role Management Endpoints
// =============================================================================

/// Role info for API responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleInfo {
    pub name: String,
    pub description: String,
    pub permissions: Vec<String>,
    pub created_at: String,
    pub is_builtin: bool,
}

/// List all roles.
pub async fn list_roles(State(state): State<AppState>) -> Json<Vec<RoleInfo>> {
    state.activity.log(ActivityType::Query, "Listed roles");
    let roles = state.rbac.list_roles();
    let role_list: Vec<RoleInfo> = roles
        .iter()
        .map(|r| RoleInfo {
            name: r.name.clone(),
            description: r.description.clone(),
            permissions: r
                .permissions
                .iter()
                .map(|p| format!("{:?}", p).to_lowercase())
                .collect(),
            created_at: format_timestamp_ms(r.created_at),
            is_builtin: r.name == "admin"
                || r.name == "operator"
                || r.name == "viewer"
                || r.name == "analyst",
        })
        .collect();
    Json(role_list)
}

/// Create role request.
#[derive(Debug, Deserialize)]
pub struct CreateRoleRequest {
    pub name: String,
    pub description: String,
    pub permissions: Vec<String>,
}

/// Create a new role.
pub async fn create_role(
    State(state): State<AppState>,
    Json(request): Json<CreateRoleRequest>,
) -> impl IntoResponse {
    state
        .activity
        .log_write(&format!("Create role: {}", request.name), None);

    // Parse permission strings into Permission enum
    let permissions = parse_permissions(&request.permissions);

    match state
        .rbac
        .create_role(&request.name, &request.description, permissions, "admin")
    {
        Ok(()) => {
            let role = state.rbac.get_role(&request.name);
            (
                StatusCode::CREATED,
                Json(
                    serde_json::json!({"success": true, "role": request.name, "permissions": role.map(|r| r.permissions.len()).unwrap_or(0)}),
                ),
            )
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"success": false, "error": e})),
        ),
    }
}

/// Delete a role.
pub async fn delete_role(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    state
        .activity
        .log(ActivityType::Delete, &format!("Delete role: {}", name));

    match state.rbac.delete_role(&name) {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"success": true}))),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"success": false, "error": e})),
        ),
    }
}

/// Parse permission strings to Permission enums.
fn parse_permissions(perms: &[String]) -> Vec<crate::auth::Permission> {
    use crate::auth::Permission;
    perms
        .iter()
        .filter_map(|p| match p.to_lowercase().as_str() {
            "database_create" | "databasecreate" => Some(Permission::DatabaseCreate),
            "database_drop" | "databasedrop" => Some(Permission::DatabaseDrop),
            "database_list" | "databaselist" => Some(Permission::DatabaseList),
            "table_create" | "tablecreate" => Some(Permission::TableCreate),
            "table_drop" | "tabledrop" => Some(Permission::TableDrop),
            "table_alter" | "tablealter" => Some(Permission::TableAlter),
            "table_list" | "tablelist" => Some(Permission::TableList),
            "data_select" | "dataselect" | "data:read" => Some(Permission::DataSelect),
            "data_insert" | "datainsert" | "data:write" => Some(Permission::DataInsert),
            "data_update" | "dataupdate" => Some(Permission::DataUpdate),
            "data_delete" | "datadelete" => Some(Permission::DataDelete),
            "user_create" | "usercreate" => Some(Permission::UserCreate),
            "user_delete" | "userdelete" => Some(Permission::UserDelete),
            "user_modify" | "usermodify" => Some(Permission::UserModify),
            "role_create" | "rolecreate" => Some(Permission::RoleCreate),
            "role_delete" | "roledelete" => Some(Permission::RoleDelete),
            "role_assign" | "roleassign" => Some(Permission::RoleAssign),
            "config_view" | "configview" => Some(Permission::ConfigView),
            "config_modify" | "configmodify" => Some(Permission::ConfigModify),
            "metrics_view" | "metricsview" => Some(Permission::MetricsView),
            "logs_view" | "logsview" => Some(Permission::LogsView),
            "backup_create" | "backupcreate" => Some(Permission::BackupCreate),
            "backup_restore" | "backuprestore" => Some(Permission::BackupRestore),
            "node_add" | "nodeadd" => Some(Permission::NodeAdd),
            "node_remove" | "noderemove" => Some(Permission::NodeRemove),
            "cluster_manage" | "clustermanage" => Some(Permission::ClusterManage),
            _ => None,
        })
        .collect()
}

/// Format timestamp from milliseconds to ISO string.
fn format_timestamp_ms(timestamp_ms: u64) -> String {
    let secs = timestamp_ms / 1000;
    let datetime = std::time::UNIX_EPOCH + std::time::Duration::from_secs(secs);
    let duration = datetime
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let total_secs = duration.as_secs();

    let days_since_epoch = total_secs / 86400;
    let secs_today = total_secs % 86400;
    let hours = secs_today / 3600;
    let minutes = (secs_today % 3600) / 60;
    let seconds = secs_today % 60;

    let mut year = 1970u64;
    let mut remaining_days = days_since_epoch;
    loop {
        let days_in_year = if (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0) {
            366
        } else {
            365
        };
        if remaining_days < days_in_year {
            break;
        }
        remaining_days -= days_in_year;
        year += 1;
    }

    let days_in_months: [u64; 12] = if (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };

    let mut month = 1u64;
    for &days in &days_in_months {
        if remaining_days < days {
            break;
        }
        remaining_days -= days;
        month += 1;
    }
    let day = remaining_days + 1;

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year, month, day, hours, minutes, seconds
    )
}

// =============================================================================
// Metrics Timeseries Endpoint
// =============================================================================

/// Metrics timeseries request.
#[derive(Debug, Deserialize)]
pub struct MetricsTimeseriesRequest {
    pub time_range: String,
}

/// Metrics data point.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Serialize)]
pub struct MetricsTimeseriesResponse {
    pub time_range: String,
    pub data_points: Vec<MetricsDataPoint>,
}

/// Get metrics timeseries data.
pub async fn get_metrics_timeseries(
    State(state): State<AppState>,
    Json(request): Json<MetricsTimeseriesRequest>,
) -> Json<MetricsTimeseriesResponse> {
    state.activity.log(
        ActivityType::Query,
        &format!("Query metrics timeseries: {}", request.time_range),
    );

    // Get time range in seconds
    let range_secs: i64 = match request.time_range.as_str() {
        "1h" => 3600,
        "6h" => 6 * 3600,
        "24h" => 24 * 3600,
        "7d" => 7 * 24 * 3600,
        "30d" => 30 * 24 * 3600,
        _ => 3600,
    };

    // Get metrics history from state
    let history = state.metrics_history.read().await;
    let now = Utc::now().timestamp();
    let start_time = now - range_secs;

    // Filter to requested time range
    let data_points: Vec<MetricsDataPoint> = history
        .iter()
        .filter(|p| p.timestamp >= start_time)
        .cloned()
        .collect();

    Json(MetricsTimeseriesResponse {
        time_range: request.time_range,
        data_points,
    })
}

// =============================================================================
// Graph Database Endpoints (Real Implementation)
// =============================================================================

/// Create a graph node.
#[derive(Debug, Deserialize)]
pub struct CreateNodeRequest {
    pub label: String,
    pub properties: serde_json::Value,
}

/// Create a graph edge.
#[derive(Debug, Deserialize)]
pub struct CreateEdgeRequest {
    pub source: String,
    pub target: String,
    pub relationship: String,
}

/// Update a graph node (any omitted field is left unchanged).
#[derive(Debug, Deserialize)]
pub struct UpdateNodeRequest {
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub properties: Option<serde_json::Value>,
}

/// Update a graph edge's relationship.
#[derive(Debug, Deserialize)]
pub struct UpdateEdgeRequest {
    pub relationship: String,
}

/// Create a new graph node.
pub async fn create_graph_node(
    State(state): State<AppState>,
    Json(request): Json<CreateNodeRequest>,
) -> impl IntoResponse {
    state
        .activity
        .log_write(&format!("Create graph node: {}", request.label), None);

    let node = state
        .graph_store
        .create_node(&request.label, request.properties);
    (
        StatusCode::CREATED,
        Json(serde_json::json!({"success": true, "node": node})),
    )
}

/// Create a new graph edge.
pub async fn create_graph_edge(
    State(state): State<AppState>,
    Json(request): Json<CreateEdgeRequest>,
) -> impl IntoResponse {
    state.activity.log_write(
        &format!(
            "Create graph edge: {} -> {}",
            request.source, request.target
        ),
        None,
    );

    match state
        .graph_store
        .create_edge(&request.source, &request.target, &request.relationship)
    {
        Ok(edge) => (
            StatusCode::CREATED,
            Json(serde_json::json!({"success": true, "edge": edge})),
        ),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"success": false, "error": e})),
        ),
    }
}

/// Delete a graph node.
pub async fn delete_graph_node(
    State(state): State<AppState>,
    Path(node_id): Path<String>,
) -> impl IntoResponse {
    state.activity.log(
        ActivityType::Delete,
        &format!("Delete graph node: {}", node_id),
    );

    match state.graph_store.delete_node(&node_id) {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"success": true}))),
        Err(e) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"success": false, "error": e})),
        ),
    }
}

/// Delete a graph edge.
pub async fn delete_graph_edge(
    State(state): State<AppState>,
    Path(edge_id): Path<String>,
) -> impl IntoResponse {
    state.activity.log(
        ActivityType::Delete,
        &format!("Delete graph edge: {}", edge_id),
    );

    match state.graph_store.delete_edge(&edge_id) {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"success": true}))),
        Err(e) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"success": false, "error": e})),
        ),
    }
}

/// Update a graph node's label and/or properties.
pub async fn update_graph_node(
    State(state): State<AppState>,
    Path(node_id): Path<String>,
    Json(request): Json<UpdateNodeRequest>,
) -> impl IntoResponse {
    state
        .activity
        .log_write(&format!("Update graph node: {}", node_id), None);

    match state
        .graph_store
        .update_node(&node_id, request.label, request.properties)
    {
        Ok(node) => (
            StatusCode::OK,
            Json(serde_json::json!({"success": true, "node": node})),
        ),
        Err(e) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"success": false, "error": e})),
        ),
    }
}

/// Update a graph edge's relationship.
pub async fn update_graph_edge(
    State(state): State<AppState>,
    Path(edge_id): Path<String>,
    Json(request): Json<UpdateEdgeRequest>,
) -> impl IntoResponse {
    state
        .activity
        .log_write(&format!("Update graph edge: {}", edge_id), None);

    match state
        .graph_store
        .update_edge(&edge_id, request.relationship)
    {
        Ok(edge) => (
            StatusCode::OK,
            Json(serde_json::json!({"success": true, "edge": edge})),
        ),
        Err(e) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"success": false, "error": e})),
        ),
    }
}

// =============================================================================
// Vector / KNN Endpoints
// =============================================================================

#[derive(Debug, Deserialize)]
pub struct CreateVectorCollectionRequest {
    pub name: String,
    pub dim: usize,
    #[serde(default = "default_metric")]
    pub metric: String,
}
fn default_metric() -> String {
    "cosine".to_string()
}

#[derive(Debug, Deserialize)]
pub struct UpsertVectorRequest {
    pub id: String,
    pub vector: Vec<f32>,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct BatchUpsertVectorRequest {
    pub vectors: Vec<aegis_vector::VectorRecord>,
}

#[derive(Debug, Deserialize)]
pub struct VectorSearchRequest {
    pub vector: Vec<f32>,
    #[serde(default = "default_k")]
    pub k: usize,
    pub ef: Option<usize>,
    #[serde(default)]
    pub filter: serde_json::Value,
}
fn default_k() -> usize {
    10
}

/// List vector collections.
pub async fn list_vector_collections(State(state): State<AppState>) -> impl IntoResponse {
    let collections = state.vector_engine.list_collections();
    Json(serde_json::json!({ "collections": collections }))
}

/// Create a vector collection: `{ name, dim, metric: cosine|l2|dot }`.
pub async fn create_vector_collection(
    State(state): State<AppState>,
    Json(req): Json<CreateVectorCollectionRequest>,
) -> impl IntoResponse {
    let metric = match aegis_vector::Metric::parse(&req.metric) {
        Some(m) => m,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "success": false,
                    "error": format!("unknown metric '{}' (use cosine, l2, or dot)", req.metric)
                })),
            )
        }
    };
    state
        .activity
        .log_write(&format!("Create vector collection: {}", req.name), None);
    match state
        .vector_engine
        .create_collection(req.name.clone(), req.dim, metric)
    {
        Ok(()) => (
            StatusCode::CREATED,
            Json(serde_json::json!({"success": true, "name": req.name})),
        ),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"success": false, "error": e.to_string()})),
        ),
    }
}

/// Vector collection stats.
pub async fn get_vector_collection(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    match state.vector_engine.collection_stats(&name) {
        Some(stats) => (StatusCode::OK, Json(serde_json::json!(stats))),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "collection not found"})),
        ),
    }
}

/// Drop a vector collection.
pub async fn drop_vector_collection(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    state.activity.log(
        ActivityType::Delete,
        &format!("Drop vector collection: {}", name),
    );
    match state.vector_engine.drop_collection(&name) {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"success": true}))),
        Err(e) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"success": false, "error": e.to_string()})),
        ),
    }
}

/// Upsert a single vector: `{ id, vector, metadata? }`.
pub async fn upsert_vector(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(req): Json<UpsertVectorRequest>,
) -> impl IntoResponse {
    match state
        .vector_engine
        .upsert(&name, req.id.clone(), &req.vector, req.metadata)
    {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({"success": true, "id": req.id})),
        ),
        Err(e) => vector_err(e),
    }
}

/// Batch upsert: `{ vectors: [{ id, vector, metadata? }] }`.
pub async fn batch_upsert_vectors(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(req): Json<BatchUpsertVectorRequest>,
) -> impl IntoResponse {
    match state.vector_engine.upsert_many(&name, req.vectors) {
        Ok(n) => (
            StatusCode::OK,
            Json(serde_json::json!({"success": true, "count": n})),
        ),
        Err(e) => vector_err(e),
    }
}

/// Get a stored vector by id.
pub async fn get_vector(
    State(state): State<AppState>,
    Path((name, id)): Path<(String, String)>,
) -> impl IntoResponse {
    match state.vector_engine.get(&name, &id) {
        Ok(Some(rec)) => (StatusCode::OK, Json(serde_json::json!(rec))),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "vector not found"})),
        ),
        Err(e) => vector_err(e),
    }
}

/// Delete a vector by id.
pub async fn delete_vector(
    State(state): State<AppState>,
    Path((name, id)): Path<(String, String)>,
) -> impl IntoResponse {
    match state.vector_engine.delete(&name, &id) {
        Ok(true) => (StatusCode::OK, Json(serde_json::json!({"success": true}))),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"success": false, "error": "vector not found"})),
        ),
        Err(e) => vector_err(e),
    }
}

/// KNN search: `{ vector, k, ef?, filter? }` → ranked hits with score + metadata.
pub async fn search_vectors(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(req): Json<VectorSearchRequest>,
) -> impl IntoResponse {
    match state
        .vector_engine
        .search(&name, &req.vector, req.k, req.ef, &req.filter)
    {
        Ok(hits) => {
            let count = hits.len();
            (
                StatusCode::OK,
                Json(serde_json::json!({ "hits": hits, "count": count })),
            )
        }
        Err(e) => vector_err(e),
    }
}

/// Map a `VectorError` to an HTTP status + JSON body.
fn vector_err(e: aegis_vector::VectorError) -> (StatusCode, Json<serde_json::Value>) {
    let status = match &e {
        aegis_vector::VectorError::CollectionNotFound(_)
        | aegis_vector::VectorError::VectorNotFound(_) => StatusCode::NOT_FOUND,
        _ => StatusCode::BAD_REQUEST,
    };
    (
        status,
        Json(serde_json::json!({"success": false, "error": e.to_string()})),
    )
}

// =============================================================================
// Full-Text Search Endpoints
// =============================================================================

#[derive(Debug, Deserialize)]
pub struct CreateFtsIndexRequest {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct FtsDocumentRequest {
    pub id: String,
    pub text: String,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct FtsSearchRequest {
    pub query: String,
    #[serde(default = "default_k")]
    pub k: usize,
    #[serde(default)]
    pub filter: serde_json::Value,
}

/// List full-text indexes.
pub async fn list_fts_indexes(State(state): State<AppState>) -> impl IntoResponse {
    Json(serde_json::json!({ "indexes": state.fulltext_engine.list_indexes() }))
}

/// Create a full-text index.
pub async fn create_fts_index(
    State(state): State<AppState>,
    Json(req): Json<CreateFtsIndexRequest>,
) -> impl IntoResponse {
    state
        .activity
        .log_write(&format!("Create full-text index: {}", req.name), None);
    match state.fulltext_engine.create_index(req.name.clone()) {
        Ok(()) => (
            StatusCode::CREATED,
            Json(serde_json::json!({"success": true, "name": req.name})),
        ),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"success": false, "error": e.to_string()})),
        ),
    }
}

/// Full-text index stats.
pub async fn get_fts_index(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    match state.fulltext_engine.index_stats(&name) {
        Some(stats) => (StatusCode::OK, Json(serde_json::json!(stats))),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "index not found"})),
        ),
    }
}

/// Drop a full-text index.
pub async fn drop_fts_index(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    state.activity.log(
        ActivityType::Delete,
        &format!("Drop full-text index: {}", name),
    );
    match state.fulltext_engine.drop_index(&name) {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"success": true}))),
        Err(e) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"success": false, "error": e.to_string()})),
        ),
    }
}

/// Index (insert or replace) a document: `{ id, text, metadata? }`.
pub async fn fts_upsert_document(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(req): Json<FtsDocumentRequest>,
) -> impl IntoResponse {
    match state
        .fulltext_engine
        .upsert(&name, req.id.clone(), req.text, req.metadata)
    {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({"success": true, "id": req.id})),
        ),
        Err(e) => fts_err(e),
    }
}

/// Get an indexed document by id.
pub async fn fts_get_document(
    State(state): State<AppState>,
    Path((name, id)): Path<(String, String)>,
) -> impl IntoResponse {
    match state.fulltext_engine.get(&name, &id) {
        Ok(Some(doc)) => (StatusCode::OK, Json(serde_json::json!(doc))),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "document not found"})),
        ),
        Err(e) => fts_err(e),
    }
}

/// Delete a document from the index.
pub async fn fts_delete_document(
    State(state): State<AppState>,
    Path((name, id)): Path<(String, String)>,
) -> impl IntoResponse {
    match state.fulltext_engine.delete(&name, &id) {
        Ok(true) => (StatusCode::OK, Json(serde_json::json!({"success": true}))),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"success": false, "error": "document not found"})),
        ),
        Err(e) => fts_err(e),
    }
}

/// BM25 search: `{ query, k, filter? }` → ranked hits.
pub async fn fts_search(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(req): Json<FtsSearchRequest>,
) -> impl IntoResponse {
    match state
        .fulltext_engine
        .search(&name, &req.query, req.k, &req.filter)
    {
        Ok(hits) => {
            let count = hits.len();
            (
                StatusCode::OK,
                Json(serde_json::json!({ "hits": hits, "count": count })),
            )
        }
        Err(e) => fts_err(e),
    }
}

fn fts_err(e: aegis_fulltext::FtsError) -> (StatusCode, Json<serde_json::Value>) {
    let status = match &e {
        aegis_fulltext::FtsError::IndexNotFound(_)
        | aegis_fulltext::FtsError::DocumentNotFound(_) => StatusCode::NOT_FOUND,
        _ => StatusCode::BAD_REQUEST,
    };
    (
        status,
        Json(serde_json::json!({"success": false, "error": e.to_string()})),
    )
}

// =============================================================================
// Geospatial Handlers (grid index + Haversine)
// =============================================================================

#[derive(Debug, Deserialize)]
pub struct CreateGeoCollectionRequest {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct GeoFeatureRequest {
    pub id: String,
    pub lat: f64,
    pub lon: f64,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct GeoRadiusRequest {
    pub lat: f64,
    pub lon: f64,
    pub radius_m: f64,
    #[serde(default)]
    pub filter: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct GeoBboxRequest {
    pub min_lat: f64,
    pub min_lon: f64,
    pub max_lat: f64,
    pub max_lon: f64,
    #[serde(default)]
    pub filter: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct GeoNearestRequest {
    pub lat: f64,
    pub lon: f64,
    #[serde(default = "default_k")]
    pub k: usize,
    #[serde(default)]
    pub filter: serde_json::Value,
}

/// List geo collections.
pub async fn list_geo_collections(State(state): State<AppState>) -> impl IntoResponse {
    Json(serde_json::json!({ "collections": state.geo_engine.list_collections() }))
}

/// Create a geo collection.
pub async fn create_geo_collection(
    State(state): State<AppState>,
    Json(req): Json<CreateGeoCollectionRequest>,
) -> impl IntoResponse {
    state
        .activity
        .log_write(&format!("Create geo collection: {}", req.name), None);
    match state.geo_engine.create_collection(req.name.clone()) {
        Ok(()) => (
            StatusCode::CREATED,
            Json(serde_json::json!({"success": true, "name": req.name})),
        ),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"success": false, "error": e.to_string()})),
        ),
    }
}

/// Geo collection stats.
pub async fn get_geo_collection(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    match state.geo_engine.collection_stats(&name) {
        Some(stats) => (StatusCode::OK, Json(serde_json::json!(stats))),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "collection not found"})),
        ),
    }
}

/// Drop a geo collection.
pub async fn drop_geo_collection(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    state.activity.log(
        ActivityType::Delete,
        &format!("Drop geo collection: {}", name),
    );
    match state.geo_engine.drop_collection(&name) {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"success": true}))),
        Err(e) => geo_err(e),
    }
}

/// Upsert a feature: `{ id, lat, lon, metadata? }`.
pub async fn geo_upsert_feature(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(req): Json<GeoFeatureRequest>,
) -> impl IntoResponse {
    match state
        .geo_engine
        .upsert(&name, req.id.clone(), req.lat, req.lon, req.metadata)
    {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({"success": true, "id": req.id})),
        ),
        Err(e) => geo_err(e),
    }
}

/// Get a feature by id.
pub async fn geo_get_feature(
    State(state): State<AppState>,
    Path((name, id)): Path<(String, String)>,
) -> impl IntoResponse {
    match state.geo_engine.get(&name, &id) {
        Ok(Some(f)) => (StatusCode::OK, Json(serde_json::json!(f))),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "feature not found"})),
        ),
        Err(e) => geo_err(e),
    }
}

/// Delete a feature by id.
pub async fn geo_delete_feature(
    State(state): State<AppState>,
    Path((name, id)): Path<(String, String)>,
) -> impl IntoResponse {
    match state.geo_engine.delete(&name, &id) {
        Ok(true) => (StatusCode::OK, Json(serde_json::json!({"success": true}))),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"success": false, "error": "feature not found"})),
        ),
        Err(e) => geo_err(e),
    }
}

/// Radius query: `{ lat, lon, radius_m, filter? }` → hits nearest first.
pub async fn geo_radius(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(req): Json<GeoRadiusRequest>,
) -> impl IntoResponse {
    match state
        .geo_engine
        .within_radius(&name, req.lat, req.lon, req.radius_m, &req.filter)
    {
        Ok(hits) => {
            let count = hits.len();
            (
                StatusCode::OK,
                Json(serde_json::json!({ "hits": hits, "count": count })),
            )
        }
        Err(e) => geo_err(e),
    }
}

/// Bounding-box query: `{ min_lat, min_lon, max_lat, max_lon, filter? }`.
pub async fn geo_bbox(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(req): Json<GeoBboxRequest>,
) -> impl IntoResponse {
    match state.geo_engine.within_bbox(
        &name,
        req.min_lat,
        req.min_lon,
        req.max_lat,
        req.max_lon,
        &req.filter,
    ) {
        Ok(hits) => {
            let count = hits.len();
            (
                StatusCode::OK,
                Json(serde_json::json!({ "hits": hits, "count": count })),
            )
        }
        Err(e) => geo_err(e),
    }
}

/// Nearest-k query: `{ lat, lon, k, filter? }` → k nearest hits.
pub async fn geo_nearest(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(req): Json<GeoNearestRequest>,
) -> impl IntoResponse {
    match state
        .geo_engine
        .nearest(&name, req.lat, req.lon, req.k, &req.filter)
    {
        Ok(hits) => {
            let count = hits.len();
            (
                StatusCode::OK,
                Json(serde_json::json!({ "hits": hits, "count": count })),
            )
        }
        Err(e) => geo_err(e),
    }
}

fn geo_err(e: aegis_geo::GeoError) -> (StatusCode, Json<serde_json::Value>) {
    let status = match &e {
        aegis_geo::GeoError::CollectionNotFound(_) | aegis_geo::GeoError::FeatureNotFound(_) => {
            StatusCode::NOT_FOUND
        }
        _ => StatusCode::BAD_REQUEST,
    };
    (
        status,
        Json(serde_json::json!({"success": false, "error": e.to_string()})),
    )
}

// =============================================================================
// Columnar / OLAP Handlers (column-major store + group-by aggregation)
// =============================================================================

#[derive(Debug, Deserialize)]
pub struct CreateColumnarTableRequest {
    pub name: String,
    pub columns: Vec<aegis_columnar::ColumnDef>,
}

#[derive(Debug, Deserialize)]
pub struct ColumnarInsertRequest {
    /// Many rows via `{rows: [...]}`, or a single row as the bare object body.
    #[serde(default)]
    pub rows: Vec<serde_json::Value>,
    #[serde(flatten)]
    pub single: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct ColumnarScanRequest {
    #[serde(default)]
    pub columns: Vec<String>,
    #[serde(default)]
    pub filter: Vec<aegis_columnar::Condition>,
    #[serde(default)]
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct ColumnarAggregateRequest {
    #[serde(default)]
    pub group_by: Vec<String>,
    pub aggregates: Vec<aegis_columnar::AggSpec>,
    #[serde(default)]
    pub filter: Vec<aegis_columnar::Condition>,
}

/// List columnar tables.
pub async fn list_columnar_tables(State(state): State<AppState>) -> impl IntoResponse {
    Json(serde_json::json!({ "tables": state.columnar_engine.list_tables() }))
}

/// Create a columnar table: `{ name, columns: [{name, type}] }`.
pub async fn create_columnar_table(
    State(state): State<AppState>,
    Json(req): Json<CreateColumnarTableRequest>,
) -> impl IntoResponse {
    state
        .activity
        .log_write(&format!("Create columnar table: {}", req.name), None);
    match state
        .columnar_engine
        .create_table(req.name.clone(), req.columns)
    {
        Ok(()) => (
            StatusCode::CREATED,
            Json(serde_json::json!({"success": true, "name": req.name})),
        ),
        Err(e) => columnar_err(e),
    }
}

/// Columnar table stats (row count + schema).
pub async fn get_columnar_table(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    match state.columnar_engine.table_stats(&name) {
        Some(stats) => (StatusCode::OK, Json(serde_json::json!(stats))),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "table not found"})),
        ),
    }
}

/// Drop a columnar table.
pub async fn drop_columnar_table(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    state.activity.log(
        ActivityType::Delete,
        &format!("Drop columnar table: {}", name),
    );
    match state.columnar_engine.drop_table(&name) {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"success": true}))),
        Err(e) => columnar_err(e),
    }
}

/// Insert one row (`{col: val, ...}`) or many (`{rows: [...]}`).
pub async fn columnar_insert(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(req): Json<ColumnarInsertRequest>,
) -> impl IntoResponse {
    let result = if !req.rows.is_empty() {
        state.columnar_engine.insert_many(&name, &req.rows)
    } else {
        state
            .columnar_engine
            .insert(&name, &req.single)
            .map(|_| 1usize)
    };
    match result {
        Ok(n) => (
            StatusCode::OK,
            Json(serde_json::json!({"success": true, "inserted": n})),
        ),
        Err(e) => columnar_err(e),
    }
}

/// Scan rows: `{ columns?, filter?, limit? }` → projected rows.
pub async fn columnar_scan(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(req): Json<ColumnarScanRequest>,
) -> impl IntoResponse {
    match state
        .columnar_engine
        .scan(&name, &req.columns, &req.filter, req.limit)
    {
        Ok(rows) => {
            let count = rows.len();
            (
                StatusCode::OK,
                Json(serde_json::json!({ "rows": rows, "count": count })),
            )
        }
        Err(e) => columnar_err(e),
    }
}

/// Group-by aggregation: `{ group_by?, aggregates, filter? }`.
pub async fn columnar_aggregate(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(req): Json<ColumnarAggregateRequest>,
) -> impl IntoResponse {
    match state
        .columnar_engine
        .aggregate(&name, &req.group_by, &req.aggregates, &req.filter)
    {
        Ok(groups) => {
            let count = groups.len();
            (
                StatusCode::OK,
                Json(serde_json::json!({ "groups": groups, "count": count })),
            )
        }
        Err(e) => columnar_err(e),
    }
}

/// Distinct non-null values of a column.
pub async fn columnar_distinct(
    State(state): State<AppState>,
    Path((name, column)): Path<(String, String)>,
) -> impl IntoResponse {
    match state.columnar_engine.distinct(&name, &column) {
        Ok(values) => {
            let count = values.len();
            (
                StatusCode::OK,
                Json(serde_json::json!({ "values": values, "count": count })),
            )
        }
        Err(e) => columnar_err(e),
    }
}

fn columnar_err(e: aegis_columnar::ColumnarError) -> (StatusCode, Json<serde_json::Value>) {
    let status = match &e {
        aegis_columnar::ColumnarError::TableNotFound(_) => StatusCode::NOT_FOUND,
        aegis_columnar::ColumnarError::TableExists(_) => StatusCode::CONFLICT,
        _ => StatusCode::BAD_REQUEST,
    };
    (
        status,
        Json(serde_json::json!({"success": false, "error": e.to_string()})),
    )
}

// =============================================================================
// Object / Blob Handlers (S3-style buckets + content-addressed ETags)
// =============================================================================

#[derive(Debug, Deserialize)]
pub struct CreateBucketRequest {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct ObjectListParams {
    #[serde(default)]
    pub prefix: String,
    #[serde(default)]
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct ObjectGetParams {
    /// When set (`?meta=1`), return JSON metadata instead of the object bytes.
    #[serde(default)]
    pub meta: Option<String>,
}

/// List buckets.
pub async fn list_buckets(State(state): State<AppState>) -> impl IntoResponse {
    Json(serde_json::json!({ "buckets": state.object_engine.list_buckets() }))
}

/// Create a bucket: `{ name }`.
pub async fn create_bucket(
    State(state): State<AppState>,
    Json(req): Json<CreateBucketRequest>,
) -> impl IntoResponse {
    state
        .activity
        .log_write(&format!("Create bucket: {}", req.name), None);
    match state.object_engine.create_bucket(req.name.clone()) {
        Ok(()) => (
            StatusCode::CREATED,
            Json(serde_json::json!({"success": true, "name": req.name})),
        ),
        Err(e) => object_err(e),
    }
}

/// Bucket stats (object count + total bytes).
pub async fn get_bucket(
    State(state): State<AppState>,
    Path(bucket): Path<String>,
) -> impl IntoResponse {
    match state.object_engine.bucket_stats(&bucket) {
        Some(stats) => (StatusCode::OK, Json(serde_json::json!(stats))),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "bucket not found"})),
        ),
    }
}

/// Drop a bucket.
pub async fn drop_bucket(
    State(state): State<AppState>,
    Path(bucket): Path<String>,
) -> impl IntoResponse {
    state
        .activity
        .log(ActivityType::Delete, &format!("Drop bucket: {}", bucket));
    match state.object_engine.drop_bucket(&bucket) {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"success": true}))),
        Err(e) => object_err(e),
    }
}

/// List object metadata in a bucket (`?prefix=&limit=`).
pub async fn list_objects(
    State(state): State<AppState>,
    Path(bucket): Path<String>,
    Query(params): Query<ObjectListParams>,
) -> impl IntoResponse {
    match state
        .object_engine
        .list(&bucket, &params.prefix, params.limit)
    {
        Ok(objects) => {
            let count = objects.len();
            (
                StatusCode::OK,
                Json(serde_json::json!({ "objects": objects, "count": count })),
            )
                .into_response()
        }
        Err(e) => object_err(e).into_response(),
    }
}

/// Store an object — the raw request body is the content. `Content-Type` sets
/// the stored content type; the optional `X-Aegis-Meta` header (JSON) sets
/// custom metadata.
pub async fn put_object(
    State(state): State<AppState>,
    Path((bucket, key)): Path<(String, String)>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    let content_type = headers
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    let metadata = headers
        .get("x-aegis-meta")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or(serde_json::Value::Null);
    match state
        .object_engine
        .put(&bucket, key, body.to_vec(), content_type, metadata)
    {
        Ok(meta) => (
            StatusCode::OK,
            Json(serde_json::json!({"success": true, "object": meta})),
        )
            .into_response(),
        Err(e) => object_err(e).into_response(),
    }
}

/// Fetch an object. Returns the raw bytes (with `Content-Type` + `ETag`), or —
/// with `?meta=1` — the JSON metadata only.
pub async fn get_object(
    State(state): State<AppState>,
    Path((bucket, key)): Path<(String, String)>,
    Query(params): Query<ObjectGetParams>,
) -> impl IntoResponse {
    if params.meta.is_some() {
        return match state.object_engine.head(&bucket, &key) {
            Ok(Some(meta)) => (StatusCode::OK, Json(serde_json::json!(meta))).into_response(),
            Ok(None) => (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "object not found"})),
            )
                .into_response(),
            Err(e) => object_err(e).into_response(),
        };
    }
    match state.object_engine.get(&bucket, &key) {
        Ok(Some((data, meta))) => {
            let mut resp_headers = HeaderMap::new();
            if let Ok(ct) = meta.content_type.parse() {
                resp_headers.insert(axum::http::header::CONTENT_TYPE, ct);
            }
            if let Ok(tag) = format!("\"{}\"", meta.etag).parse() {
                resp_headers.insert(axum::http::header::ETAG, tag);
            }
            (StatusCode::OK, resp_headers, data).into_response()
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "object not found"})),
        )
            .into_response(),
        Err(e) => object_err(e).into_response(),
    }
}

/// Delete an object.
pub async fn delete_object(
    State(state): State<AppState>,
    Path((bucket, key)): Path<(String, String)>,
) -> impl IntoResponse {
    match state.object_engine.delete(&bucket, &key) {
        Ok(true) => (StatusCode::OK, Json(serde_json::json!({"success": true}))).into_response(),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"success": false, "error": "object not found"})),
        )
            .into_response(),
        Err(e) => object_err(e).into_response(),
    }
}

fn object_err(e: aegis_object::ObjectError) -> (StatusCode, Json<serde_json::Value>) {
    let status = match &e {
        aegis_object::ObjectError::BucketNotFound(_)
        | aegis_object::ObjectError::ObjectNotFound(_) => StatusCode::NOT_FOUND,
        aegis_object::ObjectError::BucketExists(_) => StatusCode::CONFLICT,
        aegis_object::ObjectError::InvalidBucketName(_) => StatusCode::BAD_REQUEST,
    };
    (
        status,
        Json(serde_json::json!({"success": false, "error": e.to_string()})),
    )
}

// =============================================================================
// Wide-Column Handlers (row-keyed sparse columns, per-cell timestamps, LWW)
// =============================================================================

#[derive(Debug, Deserialize)]
pub struct CreateWideTableRequest {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct WidePutRequest {
    pub columns: serde_json::Map<String, serde_json::Value>,
    #[serde(default)]
    pub timestamp: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct WideGetParams {
    /// Comma-separated column projection; empty = all columns.
    #[serde(default)]
    pub columns: String,
}

#[derive(Debug, Deserialize)]
pub struct WideScanRequest {
    #[serde(default)]
    pub start: Option<String>,
    #[serde(default)]
    pub end: Option<String>,
    #[serde(default)]
    pub prefix: Option<String>,
    #[serde(default)]
    pub columns: Vec<String>,
    #[serde(default)]
    pub limit: Option<usize>,
}

fn split_columns(s: &str) -> Vec<String> {
    s.split(',')
        .map(|c| c.trim())
        .filter(|c| !c.is_empty())
        .map(|c| c.to_string())
        .collect()
}

/// List wide-column tables.
pub async fn list_wide_tables(State(state): State<AppState>) -> impl IntoResponse {
    Json(serde_json::json!({ "tables": state.widecolumn_engine.list_tables() }))
}

/// Create a wide-column table: `{ name }`.
pub async fn create_wide_table(
    State(state): State<AppState>,
    Json(req): Json<CreateWideTableRequest>,
) -> impl IntoResponse {
    state
        .activity
        .log_write(&format!("Create wide-column table: {}", req.name), None);
    match state.widecolumn_engine.create_table(req.name.clone()) {
        Ok(()) => (
            StatusCode::CREATED,
            Json(serde_json::json!({"success": true, "name": req.name})),
        ),
        Err(e) => wide_err(e),
    }
}

/// Wide-column table stats (rows + cells).
pub async fn get_wide_table(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    match state.widecolumn_engine.table_stats(&name) {
        Some(stats) => (StatusCode::OK, Json(serde_json::json!(stats))),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "table not found"})),
        ),
    }
}

/// Drop a wide-column table.
pub async fn drop_wide_table(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    state.activity.log(
        ActivityType::Delete,
        &format!("Drop wide-column table: {}", name),
    );
    match state.widecolumn_engine.drop_table(&name) {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"success": true}))),
        Err(e) => wide_err(e),
    }
}

/// Set columns on a row: `{ columns: {...}, timestamp? }` (last-write-wins).
pub async fn wide_put_row(
    State(state): State<AppState>,
    Path((table, row_key)): Path<(String, String)>,
    Json(req): Json<WidePutRequest>,
) -> impl IntoResponse {
    match state
        .widecolumn_engine
        .put(&table, row_key, req.columns, req.timestamp)
    {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"success": true}))),
        Err(e) => wide_err(e),
    }
}

/// Get a row (optional `?columns=a,b` projection).
pub async fn wide_get_row(
    State(state): State<AppState>,
    Path((table, row_key)): Path<(String, String)>,
    Query(params): Query<WideGetParams>,
) -> impl IntoResponse {
    let cols = split_columns(&params.columns);
    match state.widecolumn_engine.get(&table, &row_key, &cols) {
        Ok(Some(row)) => (StatusCode::OK, Json(serde_json::json!(row))).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "row not found"})),
        )
            .into_response(),
        Err(e) => wide_err(e).into_response(),
    }
}

/// Delete a row.
pub async fn wide_delete_row(
    State(state): State<AppState>,
    Path((table, row_key)): Path<(String, String)>,
) -> impl IntoResponse {
    match state.widecolumn_engine.delete_row(&table, &row_key) {
        Ok(true) => (StatusCode::OK, Json(serde_json::json!({"success": true}))).into_response(),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"success": false, "error": "row not found"})),
        )
            .into_response(),
        Err(e) => wide_err(e).into_response(),
    }
}

/// Delete a single column (cell) from a row.
pub async fn wide_delete_cell(
    State(state): State<AppState>,
    Path((table, row_key, column)): Path<(String, String, String)>,
) -> impl IntoResponse {
    match state
        .widecolumn_engine
        .delete_cell(&table, &row_key, &column)
    {
        Ok(true) => (StatusCode::OK, Json(serde_json::json!({"success": true}))).into_response(),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"success": false, "error": "cell not found"})),
        )
            .into_response(),
        Err(e) => wide_err(e).into_response(),
    }
}

/// Scan rows in key order: `{ start?, end?, prefix?, columns?, limit? }`.
pub async fn wide_scan(
    State(state): State<AppState>,
    Path(table): Path<String>,
    Json(req): Json<WideScanRequest>,
) -> impl IntoResponse {
    match state.widecolumn_engine.scan(
        &table,
        req.start.as_deref(),
        req.end.as_deref(),
        req.prefix.as_deref(),
        &req.columns,
        req.limit,
    ) {
        Ok(rows) => {
            let count = rows.len();
            (
                StatusCode::OK,
                Json(serde_json::json!({ "rows": rows, "count": count })),
            )
        }
        Err(e) => wide_err(e),
    }
}

fn wide_err(e: aegis_widecolumn::WideColumnError) -> (StatusCode, Json<serde_json::Value>) {
    let status = match &e {
        aegis_widecolumn::WideColumnError::TableNotFound(_) => StatusCode::NOT_FOUND,
        aegis_widecolumn::WideColumnError::TableExists(_) => StatusCode::CONFLICT,
        aegis_widecolumn::WideColumnError::EmptyWrite => StatusCode::BAD_REQUEST,
    };
    (
        status,
        Json(serde_json::json!({"success": false, "error": e.to_string()})),
    )
}

// =============================================================================
// Ledger Handlers (immutable, hash-chained append-only log)
// =============================================================================

#[derive(Debug, Deserialize)]
pub struct CreateLedgerRequest {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct LedgerAppendRequest {
    pub payload: serde_json::Value,
    #[serde(default)]
    pub timestamp: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct LedgerRangeParams {
    #[serde(default)]
    pub start: u64,
    #[serde(default)]
    pub limit: Option<usize>,
}

/// List ledgers.
pub async fn list_ledgers(State(state): State<AppState>) -> impl IntoResponse {
    Json(serde_json::json!({ "ledgers": state.ledger_engine.list_ledgers() }))
}

/// Create a ledger: `{ name }`.
pub async fn create_ledger(
    State(state): State<AppState>,
    Json(req): Json<CreateLedgerRequest>,
) -> impl IntoResponse {
    state
        .activity
        .log_write(&format!("Create ledger: {}", req.name), None);
    match state.ledger_engine.create_ledger(req.name.clone()) {
        Ok(()) => (
            StatusCode::CREATED,
            Json(serde_json::json!({"success": true, "name": req.name})),
        ),
        Err(e) => ledger_err(e),
    }
}

/// Ledger stats (entry count + chain-tip hash).
pub async fn get_ledger(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    match state.ledger_engine.ledger_stats(&name) {
        Some(stats) => (StatusCode::OK, Json(serde_json::json!(stats))),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "ledger not found"})),
        ),
    }
}

/// Drop a ledger.
pub async fn drop_ledger(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    state
        .activity
        .log(ActivityType::Delete, &format!("Drop ledger: {}", name));
    match state.ledger_engine.drop_ledger(&name) {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"success": true}))),
        Err(e) => ledger_err(e),
    }
}

/// Append an entry: `{ payload, timestamp? }`. Returns the immutable entry.
pub async fn ledger_append(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(req): Json<LedgerAppendRequest>,
) -> impl IntoResponse {
    match state
        .ledger_engine
        .append(&name, req.payload, req.timestamp)
    {
        Ok(entry) => (
            StatusCode::OK,
            Json(serde_json::json!({"success": true, "entry": entry})),
        ),
        Err(e) => ledger_err(e),
    }
}

/// Read entries from `?start=&limit=`.
pub async fn ledger_entries(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Query(params): Query<LedgerRangeParams>,
) -> impl IntoResponse {
    match state.ledger_engine.range(&name, params.start, params.limit) {
        Ok(entries) => {
            let count = entries.len();
            (
                StatusCode::OK,
                Json(serde_json::json!({ "entries": entries, "count": count })),
            )
        }
        Err(e) => ledger_err(e),
    }
}

/// Get a single entry by sequence number.
pub async fn ledger_get_entry(
    State(state): State<AppState>,
    Path((name, seq)): Path<(String, u64)>,
) -> impl IntoResponse {
    match state.ledger_engine.get(&name, seq) {
        Ok(Some(entry)) => (StatusCode::OK, Json(serde_json::json!(entry))).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "entry not found"})),
        )
            .into_response(),
        Err(e) => ledger_err(e).into_response(),
    }
}

/// Verify a ledger's hash chain end to end.
pub async fn ledger_verify(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    match state.ledger_engine.verify(&name) {
        Ok(result) => (StatusCode::OK, Json(serde_json::json!(result))),
        Err(e) => ledger_err(e),
    }
}

fn ledger_err(e: aegis_ledger::LedgerError) -> (StatusCode, Json<serde_json::Value>) {
    let status = match &e {
        aegis_ledger::LedgerError::LedgerNotFound(_)
        | aegis_ledger::LedgerError::EntryNotFound(_) => StatusCode::NOT_FOUND,
        aegis_ledger::LedgerError::LedgerExists(_) => StatusCode::CONFLICT,
    };
    (
        status,
        Json(serde_json::json!({"success": false, "error": e.to_string()})),
    )
}

// =============================================================================
// OTA Update Handlers
// =============================================================================

/// Get version information for all cluster nodes.
pub async fn get_update_version(State(state): State<AppState>) -> impl IntoResponse {
    let version = aegis_updates::version::VERSION;
    let node_name = state
        .config
        .node_name
        .clone()
        .unwrap_or_else(|| "unknown".to_string());

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "success": true,
            "version": version,
            "node_id": state.config.node_id,
            "node_name": node_name,
        })),
    )
}

/// Create an update plan.
#[derive(serde::Deserialize)]
pub struct CreateUpdatePlanRequest {
    pub version: String,
    pub binary_url: String,
    pub sha256: String,
}

pub async fn create_update_plan(
    State(state): State<AppState>,
    Json(request): Json<CreateUpdatePlanRequest>,
) -> impl IntoResponse {
    state.activity.log_system(&format!(
        "Creating update plan for version {}",
        request.version
    ));

    // Populate cluster nodes from admin service peers
    let peers = state.admin.get_peers();
    let mut nodes = vec![aegis_updates::orchestrator::ClusterNode {
        node_id: state.config.node_id.clone(),
        name: state
            .config
            .node_name
            .clone()
            .unwrap_or_else(|| "self".to_string()),
        address: format!("http://{}:{}", state.config.host, state.config.port),
        role: "leader".to_string(),
    }];
    for peer in &peers {
        nodes.push(aegis_updates::orchestrator::ClusterNode {
            node_id: peer.id.clone(),
            name: peer.name.clone().unwrap_or_else(|| peer.id.clone()),
            address: peer.address.clone(),
            role: "follower".to_string(),
        });
    }
    state.update_orchestrator.set_cluster_nodes(nodes).await;

    let plan = state
        .update_orchestrator
        .create_plan(request.version, request.binary_url, request.sha256)
        .await;

    (
        StatusCode::CREATED,
        Json(serde_json::json!({
            "success": true,
            "plan": plan,
        })),
    )
}

/// Execute an update plan.
#[derive(serde::Deserialize)]
pub struct ExecuteUpdateRequest {
    pub plan_id: String,
}

pub async fn execute_update_plan(
    State(state): State<AppState>,
    Json(request): Json<ExecuteUpdateRequest>,
) -> impl IntoResponse {
    state
        .activity
        .log_system(&format!("Executing update plan {}", request.plan_id));

    match state
        .update_orchestrator
        .execute_plan(&request.plan_id)
        .await
    {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "success": true,
                "message": "Update completed successfully",
            })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "success": false,
                "error": e.to_string(),
            })),
        ),
    }
}

/// Get update plan status.
pub async fn get_update_status(
    State(state): State<AppState>,
    Path(plan_id): Path<String>,
) -> impl IntoResponse {
    match state.update_orchestrator.get_plan(&plan_id).await {
        Some(plan) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "success": true,
                "plan": plan,
            })),
        ),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "success": false,
                "error": format!("Plan {} not found", plan_id),
            })),
        ),
    }
}

/// List all update plans (history).
pub async fn list_update_plans(State(state): State<AppState>) -> impl IntoResponse {
    let plans = state.update_orchestrator.list_plans().await;

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "success": true,
            "plans": plans,
        })),
    )
}
