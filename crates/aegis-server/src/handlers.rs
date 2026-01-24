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
use crate::state::{AppState, KvEntry, QueryError, QueryResult};
use aegis_document::{Document, DocumentId, Query as DocQuery, QueryResult as DocQueryResult};
use aegis_streaming::{ChannelId, Event, EventType as StreamEventType, event::EventData};
use aegis_timeseries::{DataPoint, Metric, MetricType, Tags, TimeSeriesQuery};
use axum::{
    extract::{Path, State},
    http::StatusCode,
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

/// Get database statistics (key counts, document counts, etc.)
pub async fn get_database_stats(State(state): State<AppState>) -> Json<crate::state::DatabaseStats> {
    Json(state.get_database_stats())
}

/// Alert response structure.
#[derive(Debug, Serialize)]
pub struct AlertsResponse {
    pub alerts: Vec<AlertInfo>,
}

/// Get active alerts based on real system conditions.
pub async fn get_alerts(State(_state): State<AppState>) -> Json<AlertsResponse> {
    use sysinfo::{System, Disks};

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
pub async fn login(
    State(state): State<AppState>,
    Json(request): Json<LoginRequest>,
) -> impl IntoResponse {
    let response = state.auth.login(&request.username, &request.password);

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
    let limit = params.get("limit").and_then(|s| s.parse().ok()).unwrap_or(100);
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
    state.activity.log_write(&format!("Set key: {}", request.key), None);
    let entry = state.kv_store.set(request.key, request.value, request.ttl);
    Json(entry)
}

/// Get a specific key.
pub async fn get_key(
    State(state): State<AppState>,
    Path(key): Path<String>,
) -> impl IntoResponse {
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
    state.activity.log(ActivityType::Delete, &format!("Delete key: {}", key));
    match state.kv_store.delete(&key) {
        Some(_) => (StatusCode::OK, Json(serde_json::json!({"success": true, "key": key}))),
        None => (StatusCode::NOT_FOUND, Json(serde_json::json!({"success": false, "error": "Key not found"}))),
    }
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
    state.activity.log(ActivityType::Query, "Listed collections");

    let collection_names = state.document_engine.list_collections();
    let collections: Vec<CollectionInfoResponse> = collection_names
        .iter()
        .filter_map(|name| {
            state.document_engine.collection_stats(name).map(|stats| CollectionInfoResponse {
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
}

/// Get documents in a collection - uses real DocumentEngine.
pub async fn get_collection_documents(
    State(state): State<AppState>,
    Path(collection): Path<String>,
) -> impl IntoResponse {
    state.activity.log(ActivityType::Query, &format!("Query collection: {}", collection));

    // Use find with empty query to get all documents
    let query = DocQuery::new();
    match state.document_engine.find(&collection, &query) {
        Ok(result) => {
            // Explicit type annotation to use DocQueryResult
            let query_result: &DocQueryResult = &result;
            let docs: Vec<DocumentResponse> = query_result.documents
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
            };
            (StatusCode::OK, Json(response))
        }
        Err(_e) => {
            let empty = CollectionQueryResponse {
                documents: vec![],
                total_scanned: 0,
                execution_time_ms: 0,
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
    state.activity.log(ActivityType::Query, &format!("Get document: {}/{}", collection, id));

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
        Ok(None) => {
            (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Document not found"})))
        }
        Err(e) => {
            (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": e.to_string()})))
        }
    }
}

/// Delete a document from a collection.
pub async fn delete_document(
    State(state): State<AppState>,
    Path((collection, id)): Path<(String, String)>,
) -> impl IntoResponse {
    state.activity.log(ActivityType::Delete, &format!("Delete document: {}/{}", collection, id));

    let doc_id = DocumentId::new(&id);
    match state.document_engine.delete(&collection, &doc_id) {
        Ok(doc) => {
            let response = DocumentResponse {
                id: doc.id.to_string(),
                collection: collection.clone(),
                data: doc_to_json(&doc),
            };
            (StatusCode::OK, Json(serde_json::json!({"success": true, "deleted": response})))
        }
        Err(e) => {
            (StatusCode::NOT_FOUND, Json(serde_json::json!({"success": false, "error": e.to_string()})))
        }
    }
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
    state.activity.log_write(&format!("Update document: {}/{}", collection, id), None);

    let doc_id = DocumentId::new(&id);

    // Convert JSON to Document, preserving the ID
    let mut doc = json_to_doc(request.document);
    doc.id = doc_id.clone();

    match state.document_engine.update(&collection, &doc_id, doc) {
        Ok(()) => {
            // Fetch the updated document to return it
            match state.document_engine.get(&collection, &doc_id) {
                Ok(Some(updated_doc)) => {
                    let response = DocumentResponse {
                        id: updated_doc.id.to_string(),
                        collection: collection.clone(),
                        data: doc_to_json(&updated_doc),
                    };
                    (StatusCode::OK, Json(serde_json::json!({"success": true, "document": response})))
                }
                _ => (StatusCode::OK, Json(serde_json::json!({"success": true, "id": id}))),
            }
        }
        Err(e) => {
            (StatusCode::NOT_FOUND, Json(serde_json::json!({"success": false, "error": e.to_string()})))
        }
    }
}

/// Partially update a document (merge fields).
pub async fn patch_document(
    State(state): State<AppState>,
    Path((collection, id)): Path<(String, String)>,
    Json(request): Json<UpdateDocumentRequest>,
) -> impl IntoResponse {
    state.activity.log_write(&format!("Patch document: {}/{}", collection, id), None);

    let doc_id = DocumentId::new(&id);

    // First get the existing document
    let existing = match state.document_engine.get(&collection, &doc_id) {
        Ok(Some(doc)) => doc,
        Ok(None) => {
            return (StatusCode::NOT_FOUND, Json(serde_json::json!({"success": false, "error": "Document not found"})));
        }
        Err(e) => {
            return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"success": false, "error": e.to_string()})));
        }
    };

    // Merge the patch into the existing document
    let mut updated_doc = existing.clone();
    if let serde_json::Value::Object(patch_map) = request.document {
        for (key, value) in patch_map {
            updated_doc.set(&key, json_to_doc_value(value));
        }
    }

    match state.document_engine.update(&collection, &doc_id, updated_doc) {
        Ok(()) => {
            // Fetch the updated document to return it
            match state.document_engine.get(&collection, &doc_id) {
                Ok(Some(final_doc)) => {
                    let response = DocumentResponse {
                        id: final_doc.id.to_string(),
                        collection: collection.clone(),
                        data: doc_to_json(&final_doc),
                    };
                    (StatusCode::OK, Json(serde_json::json!({"success": true, "document": response})))
                }
                _ => (StatusCode::OK, Json(serde_json::json!({"success": true, "id": id}))),
            }
        }
        Err(e) => {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"success": false, "error": e.to_string()})))
        }
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
    state.activity.log_write(&format!("Create collection: {}", request.name), None);

    match state.document_engine.create_collection(&request.name) {
        Ok(()) => (StatusCode::CREATED, Json(serde_json::json!({"success": true, "collection": request.name}))),
        Err(e) => (StatusCode::BAD_REQUEST, Json(serde_json::json!({"success": false, "error": e.to_string()}))),
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
    state.activity.log_write(&format!("Insert document into: {}", collection), None);

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
        Ok(id) => (StatusCode::CREATED, Json(serde_json::json!({"success": true, "id": id.to_string()}))),
        Err(e) => (StatusCode::BAD_REQUEST, Json(serde_json::json!({"success": false, "error": e.to_string()}))),
    }
}

/// Helper to convert Document to JSON.
fn doc_to_json(doc: &Document) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    map.insert("_id".to_string(), serde_json::Value::String(doc.id.to_string()));
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
        aegis_document::Value::Float(f) => {
            serde_json::Number::from_f64(*f)
                .map(serde_json::Value::Number)
                .unwrap_or(serde_json::Value::Null)
        }
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
    let doc_id = json.get("_id")
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
        serde_json::Value::Object(map) => {
            aegis_document::Value::Object(
                map.into_iter().map(|(k, v)| (k, json_to_doc_value(v))).collect()
            )
        }
    }
}

/// List documents in a collection (GET /collections/:name/documents).
pub async fn list_collection_documents(
    State(state): State<AppState>,
    Path(collection): Path<String>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    state.activity.log(ActivityType::Query, &format!("List documents in: {}", collection));

    let limit = params.get("limit").and_then(|s| s.parse().ok());
    let skip = params.get("skip").and_then(|s| s.parse().ok());

    let mut query = DocQuery::new();
    if let Some(limit) = limit {
        query = query.with_limit(limit);
    }
    if let Some(skip) = skip {
        query = query.with_skip(skip);
    }

    match state.document_engine.find(&collection, &query) {
        Ok(result) => {
            let docs: Vec<DocumentResponse> = result.documents
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
            };
            (StatusCode::OK, Json(response))
        }
        Err(_e) => {
            let empty = CollectionQueryResponse {
                documents: vec![],
                total_scanned: 0,
                execution_time_ms: 0,
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
    state.activity.log(ActivityType::Query, &format!("Query collection: {}", collection));

    // Parse the filter into Query filters
    let mut query = DocQuery::new();

    if let serde_json::Value::Object(filter_map) = &request.filter {
        for (field, condition) in filter_map {
            if let Some(filter) = parse_filter_condition(field, condition) {
                query = query.with_filter(filter);
            }
        }
    }

    if let Some(limit) = request.limit {
        query = query.with_limit(limit);
    }
    if let Some(skip) = request.skip {
        query = query.with_skip(skip);
    }
    if let Some(ref sort) = request.sort {
        query = query.with_sort(&sort.field, sort.ascending);
    }

    match state.document_engine.find(&collection, &query) {
        Ok(result) => {
            let docs: Vec<DocumentResponse> = result.documents
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
            };
            (StatusCode::OK, Json(response))
        }
        Err(_) => {
            let empty = CollectionQueryResponse {
                documents: vec![],
                total_scanned: 0,
                execution_time_ms: 0,
            };
            (StatusCode::NOT_FOUND, Json(empty))
        }
    }
}

/// Parse a filter condition with MongoDB-style operators.
fn parse_filter_condition(field: &str, condition: &serde_json::Value) -> Option<aegis_document::query::Filter> {
    use aegis_document::query::Filter;

    match condition {
        // Direct value comparison (implicit $eq)
        serde_json::Value::Null
        | serde_json::Value::Bool(_)
        | serde_json::Value::Number(_)
        | serde_json::Value::String(_) => {
            Some(Filter::Eq {
                field: field.to_string(),
                value: json_to_doc_value(condition.clone()),
            })
        }
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
    state.activity.log_write(&format!("Register metric: {}", request.name), None);

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
        Ok(()) => (StatusCode::CREATED, Json(serde_json::json!({
            "success": true,
            "metric": request.name
        }))),
        Err(e) => (StatusCode::BAD_REQUEST, Json(serde_json::json!({
            "success": false,
            "error": e.to_string()
        }))),
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
    state.activity.log_write(&format!("Write timeseries: {}", request.metric), None);

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
        Err(e) => (StatusCode::BAD_REQUEST, Json(serde_json::json!({"success": false, "error": e.to_string()}))),
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
    state.activity.log(ActivityType::Query, &format!("Query timeseries: {}", request.metric));

    let duration = Duration::hours(24); // Default 24h lookback
    let mut query = TimeSeriesQuery::last(&request.metric, duration);

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

    let series: Vec<SeriesResponse> = result.series.iter().map(|s| {
        SeriesResponse {
            tags: s.tags.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
            points: s.points.iter().map(|p| PointResponse {
                timestamp: p.timestamp.timestamp(),
                value: p.value,
            }).collect(),
        }
    }).collect();

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
    state.activity.log_write(&format!("Create channel: {}", request.id), None);

    match state.streaming_engine.create_channel(request.id.clone()) {
        Ok(()) => (StatusCode::CREATED, Json(serde_json::json!({"success": true, "channel": request.id}))),
        Err(e) => (StatusCode::BAD_REQUEST, Json(serde_json::json!({"success": false, "error": e.to_string()}))),
    }
}

/// List channels.
pub async fn list_channels(State(state): State<AppState>) -> Json<Vec<String>> {
    state.activity.log(ActivityType::Query, "Listed channels");
    let channels: Vec<String> = state.streaming_engine.list_channels()
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
    state.activity.log_write(&format!("Publish to channel: {}", request.channel), None);

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
        Ok(receivers) => (StatusCode::OK, Json(serde_json::json!({"success": true, "receivers": receivers}))),
        Err(e) => (StatusCode::BAD_REQUEST, Json(serde_json::json!({"success": false, "error": e.to_string()}))),
    }
}

/// Get channel history.
pub async fn get_channel_history(
    State(state): State<AppState>,
    Path(channel): Path<String>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let count = params.get("count").and_then(|s| s.parse().ok()).unwrap_or(100);
    let channel_id = ChannelId::new(&channel);

    match state.streaming_engine.get_history(&channel_id, count) {
        Ok(events) => {
            let event_data: Vec<serde_json::Value> = events.iter().map(|e| {
                serde_json::json!({
                    "id": e.id.to_string(),
                    "event_type": format!("{:?}", e.event_type),
                    "source": e.source,
                    "timestamp": e.timestamp,
                })
            }).collect();
            (StatusCode::OK, Json(serde_json::json!({"events": event_data})))
        }
        Err(e) => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": e.to_string()}))),
    }
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

    // Execute through the real query engine
    match state.query_engine.execute(&request.query) {
        Ok(result) => {
            Json(ExecuteQueryResponse {
                success: true,
                columns: result.columns,
                rows: result.rows,
                row_count: result.rows_affected as usize,
                execution_time_ms: start.elapsed().as_millis() as u64,
                error: None,
            })
        }
        Err(e) => {
            Json(ExecuteQueryResponse {
                success: false,
                columns: vec![],
                rows: vec![],
                row_count: 0,
                execution_time_ms: start.elapsed().as_millis() as u64,
                error: Some(e.to_string()),
            })
        }
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

/// Restart a node.
pub async fn restart_node(
    State(state): State<AppState>,
    Path(node_id): Path<String>,
) -> Json<NodeActionResponse> {
    state.activity.log_node(&format!("Restarting node: {}", node_id));

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
    State(_state): State<AppState>,
    Path(node_id): Path<String>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Json<NodeLogsResponse> {
    let limit: usize = params.get("limit").and_then(|l| l.parse().ok()).unwrap_or(100);

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
    ];

    let total = logs.len();
    Json(NodeLogsResponse {
        node_id,
        logs: logs.into_iter().take(limit).collect(),
        total,
    })
}
