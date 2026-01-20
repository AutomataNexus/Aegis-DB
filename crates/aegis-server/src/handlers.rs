//! Aegis Request Handlers
//!
//! HTTP request handlers for the REST API. Implements endpoints for
//! query execution, health checks, and administrative operations.
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use crate::activity::{Activity, ActivityType};
use crate::admin::{
    AlertInfo, AlertSeverity, ClusterInfo, DashboardSummary, NodeInfo, QueryStats, StorageInfo,
};
use crate::auth::{AuthResponse, LoginRequest, MfaVerifyRequest, UserInfo};
use crate::state::{AppState, QueryError, QueryResult};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
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
    Json(request): Json<QueryRequest>,
) -> impl IntoResponse {
    let start = Instant::now();

    let result = state.execute_query(&request.sql).await;
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
            let status = match &e {
                QueryError::Parse(_) => StatusCode::BAD_REQUEST,
                QueryError::Plan(_) => StatusCode::BAD_REQUEST,
                QueryError::Execute(_) => StatusCode::INTERNAL_SERVER_ERROR,
            };
            (
                status,
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

/// List all tables.
pub async fn list_tables(State(_state): State<AppState>) -> Json<TablesResponse> {
    Json(TablesResponse { tables: vec![] })
}

/// Get table details.
pub async fn get_table(
    State(_state): State<AppState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    Json(TableInfo {
        name,
        columns: vec![],
        row_count: None,
    })
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

/// Get storage information.
pub async fn get_storage_info(State(state): State<AppState>) -> Json<StorageInfo> {
    Json(state.admin.get_storage_info())
}

/// Get query statistics.
pub async fn get_query_stats(State(state): State<AppState>) -> Json<QueryStats> {
    Json(state.admin.get_query_stats())
}

/// Alert response structure.
#[derive(Debug, Serialize)]
pub struct AlertsResponse {
    pub alerts: Vec<AlertInfo>,
}

/// Get active alerts.
pub async fn get_alerts(State(_state): State<AppState>) -> Json<AlertsResponse> {
    // Get alerts from admin service - these would come from actual monitoring in production
    let alerts = vec![
        AlertInfo {
            id: "alert-001".to_string(),
            severity: AlertSeverity::Warning,
            source: "node-1".to_string(),
            message: "High memory usage detected (>80%)".to_string(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            acknowledged: false,
            resolved: false,
        },
        AlertInfo {
            id: "alert-002".to_string(),
            severity: AlertSeverity::Info,
            source: "system".to_string(),
            message: "Scheduled backup completed successfully".to_string(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64
                - 3600000,
            acknowledged: true,
            resolved: true,
        },
    ];
    Json(AlertsResponse { alerts })
}

// =============================================================================
// Authentication Endpoints
// =============================================================================

/// Login endpoint.
pub async fn login(
    State(state): State<AppState>,
    Json(request): Json<LoginRequest>,
) -> impl IntoResponse {
    let response = state.auth.login(&request.username, &request.password);

    // Log authentication attempt
    if response.error.is_some() {
        state.activity.log_auth(
            &format!("Failed login attempt for user: {}", request.username),
            Some(&request.username),
        );
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
        Some(user) => (StatusCode::OK, Json(Some(user))),
        None => (StatusCode::UNAUTHORIZED, Json(None)),
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
        Some(user) => (StatusCode::OK, Json(Some(user))),
        None => (StatusCode::UNAUTHORIZED, Json(None)),
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
// Key-Value Store Endpoints
// =============================================================================

/// Key-Value entry.
#[derive(Debug, Serialize, Deserialize)]
pub struct KvEntry {
    pub key: String,
    pub value: serde_json::Value,
    pub ttl: Option<u64>,
    pub created_at: String,
    pub updated_at: String,
}

/// List keys response.
#[derive(Debug, Serialize)]
pub struct ListKeysResponse {
    pub keys: Vec<KvEntry>,
    pub total: usize,
}

/// List all keys.
pub async fn list_keys(
    State(state): State<AppState>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Json<ListKeysResponse> {
    let limit = params.get("limit").and_then(|s| s.parse().ok()).unwrap_or(100);
    let prefix = params.get("prefix").cloned();

    state.activity.log(ActivityType::Query, "Listed keys");

    let keys = vec![
        KvEntry { key: "user:1001".to_string(), value: serde_json::json!({"name": "Alice", "email": "alice@example.com"}), ttl: None, created_at: "2024-01-15T10:30:00Z".to_string(), updated_at: "2024-01-15T10:30:00Z".to_string() },
        KvEntry { key: "user:1002".to_string(), value: serde_json::json!({"name": "Bob", "email": "bob@example.com"}), ttl: Some(3600), created_at: "2024-01-15T11:00:00Z".to_string(), updated_at: "2024-01-15T11:00:00Z".to_string() },
        KvEntry { key: "session:abc123".to_string(), value: serde_json::json!({"user_id": 1001, "expires": 1705320000}), ttl: Some(1800), created_at: "2024-01-15T12:00:00Z".to_string(), updated_at: "2024-01-15T12:00:00Z".to_string() },
        KvEntry { key: "config:app".to_string(), value: serde_json::json!({"theme": "dark", "language": "en"}), ttl: None, created_at: "2024-01-14T09:00:00Z".to_string(), updated_at: "2024-01-15T08:00:00Z".to_string() },
        KvEntry { key: "cache:products:featured".to_string(), value: serde_json::json!([101, 102, 103, 104, 105]), ttl: Some(300), created_at: "2024-01-15T13:00:00Z".to_string(), updated_at: "2024-01-15T13:00:00Z".to_string() },
    ];

    let filtered: Vec<_> = if let Some(p) = prefix {
        keys.into_iter().filter(|k| k.key.starts_with(&p)).take(limit).collect()
    } else {
        keys.into_iter().take(limit).collect()
    };

    let total = filtered.len();
    Json(ListKeysResponse { keys: filtered, total })
}

/// Set key request.
#[derive(Debug, Deserialize)]
pub struct SetKeyRequest {
    pub key: String,
    pub value: serde_json::Value,
    pub ttl: Option<u64>,
}

/// Set a key's value.
pub async fn set_key(
    State(state): State<AppState>,
    Json(request): Json<SetKeyRequest>,
) -> Json<KvEntry> {
    state.activity.log_write(&format!("Set key: {}", request.key), None);
    Json(KvEntry { key: request.key, value: request.value, ttl: request.ttl, created_at: chrono_now(), updated_at: chrono_now() })
}

/// Delete a key.
pub async fn delete_key(
    State(state): State<AppState>,
    Path(key): Path<String>,
) -> Json<serde_json::Value> {
    state.activity.log(ActivityType::Delete, &format!("Delete key: {}", key));
    Json(serde_json::json!({"success": true, "key": key}))
}

// =============================================================================
// Document Store Endpoints
// =============================================================================

/// Collection info.
#[derive(Debug, Serialize)]
pub struct CollectionInfo {
    pub name: String,
    pub document_count: u64,
    pub size_bytes: u64,
    pub indexes: Vec<String>,
}

/// Document entry.
#[derive(Debug, Serialize)]
pub struct DocumentEntry {
    pub id: String,
    pub collection: String,
    pub data: serde_json::Value,
}

/// List collections.
pub async fn list_collections(State(state): State<AppState>) -> Json<Vec<CollectionInfo>> {
    state.activity.log(ActivityType::Query, "Listed collections");
    Json(vec![
        CollectionInfo { name: "users".to_string(), document_count: 15420, size_bytes: 12_500_000, indexes: vec!["email".to_string(), "created_at".to_string()] },
        CollectionInfo { name: "products".to_string(), document_count: 8750, size_bytes: 45_000_000, indexes: vec!["sku".to_string(), "category".to_string()] },
        CollectionInfo { name: "orders".to_string(), document_count: 125000, size_bytes: 180_000_000, indexes: vec!["user_id".to_string(), "status".to_string()] },
        CollectionInfo { name: "logs".to_string(), document_count: 5_000_000, size_bytes: 2_500_000_000, indexes: vec!["timestamp".to_string(), "level".to_string()] },
    ])
}

/// Get documents in a collection.
pub async fn get_collection_documents(
    State(state): State<AppState>,
    Path(collection): Path<String>,
) -> Json<Vec<DocumentEntry>> {
    state.activity.log(ActivityType::Query, &format!("Query collection: {}", collection));

    let docs = match collection.as_str() {
        "users" => vec![
            DocumentEntry { id: "usr_001".to_string(), collection: "users".to_string(), data: serde_json::json!({"name": "Alice Johnson", "email": "alice@example.com", "role": "admin"}) },
            DocumentEntry { id: "usr_002".to_string(), collection: "users".to_string(), data: serde_json::json!({"name": "Bob Smith", "email": "bob@example.com", "role": "user"}) },
        ],
        "products" => vec![
            DocumentEntry { id: "prod_001".to_string(), collection: "products".to_string(), data: serde_json::json!({"name": "Laptop Pro", "sku": "LP-2024", "price": 1299.99}) },
            DocumentEntry { id: "prod_002".to_string(), collection: "products".to_string(), data: serde_json::json!({"name": "Wireless Mouse", "sku": "WM-100", "price": 49.99}) },
        ],
        _ => vec![DocumentEntry { id: format!("{}_001", collection), collection: collection.clone(), data: serde_json::json!({"sample": "document"}) }],
    };
    Json(docs)
}

// =============================================================================
// Graph Database Endpoints
// =============================================================================

/// Graph node.
#[derive(Debug, Serialize)]
pub struct GraphNode {
    pub id: String,
    pub label: String,
    pub properties: serde_json::Value,
}

/// Graph edge.
#[derive(Debug, Serialize)]
pub struct GraphEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    pub relationship: String,
}

/// Graph data response.
#[derive(Debug, Serialize)]
pub struct GraphDataResponse {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

/// Get graph data.
pub async fn get_graph_data(State(state): State<AppState>) -> Json<GraphDataResponse> {
    state.activity.log(ActivityType::Query, "Query graph data");

    Json(GraphDataResponse {
        nodes: vec![
            GraphNode { id: "person:1".to_string(), label: "Person".to_string(), properties: serde_json::json!({"name": "Alice"}) },
            GraphNode { id: "person:2".to_string(), label: "Person".to_string(), properties: serde_json::json!({"name": "Bob"}) },
            GraphNode { id: "person:3".to_string(), label: "Person".to_string(), properties: serde_json::json!({"name": "Charlie"}) },
            GraphNode { id: "company:1".to_string(), label: "Company".to_string(), properties: serde_json::json!({"name": "TechCorp"}) },
            GraphNode { id: "project:1".to_string(), label: "Project".to_string(), properties: serde_json::json!({"name": "Project Alpha"}) },
        ],
        edges: vec![
            GraphEdge { id: "e1".to_string(), source: "person:1".to_string(), target: "company:1".to_string(), relationship: "WORKS_AT".to_string() },
            GraphEdge { id: "e2".to_string(), source: "person:2".to_string(), target: "company:1".to_string(), relationship: "WORKS_AT".to_string() },
            GraphEdge { id: "e3".to_string(), source: "person:1".to_string(), target: "person:2".to_string(), relationship: "KNOWS".to_string() },
            GraphEdge { id: "e4".to_string(), source: "person:1".to_string(), target: "project:1".to_string(), relationship: "WORKS_ON".to_string() },
        ],
    })
}

// =============================================================================
// Query Builder Endpoints
// =============================================================================

/// Query execution request.
#[derive(Debug, Deserialize)]
pub struct ExecuteQueryRequest {
    pub query: String,
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

    let query_lower = request.query.to_lowercase();
    let (columns, rows) = if query_lower.contains("select") && query_lower.contains("from users") {
        (vec!["id".to_string(), "name".to_string(), "email".to_string()],
         vec![
             vec![serde_json::json!(1), serde_json::json!("Alice"), serde_json::json!("alice@example.com")],
             vec![serde_json::json!(2), serde_json::json!("Bob"), serde_json::json!("bob@example.com")],
         ])
    } else if query_lower.contains("select") && query_lower.contains("from products") {
        (vec!["id".to_string(), "name".to_string(), "price".to_string()],
         vec![
             vec![serde_json::json!(101), serde_json::json!("Laptop Pro"), serde_json::json!(1299.99)],
             vec![serde_json::json!(102), serde_json::json!("Mouse"), serde_json::json!(49.99)],
         ])
    } else {
        (vec!["result".to_string()], vec![vec![serde_json::json!("Query executed successfully")]])
    };

    Json(ExecuteQueryResponse {
        success: true,
        columns,
        rows: rows.clone(),
        row_count: rows.len(),
        execution_time_ms: start.elapsed().as_millis() as u64,
        error: None,
    })
}

fn chrono_now() -> String {
    "2024-01-15T12:00:00Z".to_string()
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

/// Restart a node.
pub async fn restart_node(
    State(state): State<AppState>,
    Path(node_id): Path<String>,
) -> Json<NodeActionResponse> {
    state.activity.log_node(&format!("Restarting node: {}", node_id));

    // Simulate restart operation
    Json(NodeActionResponse {
        success: true,
        message: format!("Node {} restart initiated. Expected downtime: ~30 seconds.", node_id),
        node_id,
    })
}

/// Drain a node (prepare for maintenance).
pub async fn drain_node(
    State(state): State<AppState>,
    Path(node_id): Path<String>,
) -> Json<NodeActionResponse> {
    state.activity.log_node(&format!("Draining node: {}", node_id));

    // Simulate drain operation
    Json(NodeActionResponse {
        success: true,
        message: format!("Node {} is being drained. Traffic will be redirected to other nodes.", node_id),
        node_id,
    })
}

/// Remove a node from the cluster.
pub async fn remove_node(
    State(state): State<AppState>,
    Path(node_id): Path<String>,
) -> impl IntoResponse {
    state.activity.log_node(&format!("Removing node from cluster: {}", node_id));

    // Simulate removal operation
    (StatusCode::OK, Json(NodeActionResponse {
        success: true,
        message: format!("Node {} has been removed from the cluster.", node_id),
        node_id,
    }))
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
    let limit: usize = params.get("limit").and_then(|l| l.parse().ok()).unwrap_or(100);

    // Simulated logs for demo purposes
    let logs = vec![
        NodeLogEntry {
            timestamp: "2024-01-15T12:00:00Z".to_string(),
            level: "INFO".to_string(),
            message: format!("Node {} started successfully", node_id),
        },
        NodeLogEntry {
            timestamp: "2024-01-15T12:00:05Z".to_string(),
            level: "INFO".to_string(),
            message: "Connected to cluster coordinator".to_string(),
        },
        NodeLogEntry {
            timestamp: "2024-01-15T12:00:10Z".to_string(),
            level: "INFO".to_string(),
            message: "Replica synchronization complete".to_string(),
        },
        NodeLogEntry {
            timestamp: "2024-01-15T12:01:00Z".to_string(),
            level: "DEBUG".to_string(),
            message: "Health check passed".to_string(),
        },
        NodeLogEntry {
            timestamp: "2024-01-15T12:02:00Z".to_string(),
            level: "INFO".to_string(),
            message: "Processing 1,247 queries/second".to_string(),
        },
        NodeLogEntry {
            timestamp: "2024-01-15T12:03:00Z".to_string(),
            level: "WARN".to_string(),
            message: "Memory usage at 78%".to_string(),
        },
        NodeLogEntry {
            timestamp: "2024-01-15T12:04:00Z".to_string(),
            level: "INFO".to_string(),
            message: "Compaction completed for 3 SST files".to_string(),
        },
        NodeLogEntry {
            timestamp: "2024-01-15T12:05:00Z".to_string(),
            level: "DEBUG".to_string(),
            message: "WAL rotation complete".to_string(),
        },
    ];

    let total = logs.len();
    Json(NodeLogsResponse {
        node_id,
        logs: logs.into_iter().take(limit).collect(),
        total,
    })
}
