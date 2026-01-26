//! Aegis Server State
//!
//! Application state shared across request handlers. Provides access to
//! database engines, query engine, and configuration.
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use crate::activity::ActivityLogger;
use crate::admin::AdminService;
use crate::auth::{AuthService, RbacManager};
use crate::config::ServerConfig;
use crate::handlers::{MetricsDataPoint, ServerSettings};
use aegis_document::{Document, DocumentEngine};
use aegis_query::{Executor, Parser, Planner};
use aegis_query::planner::PlannerSchema;
use aegis_query::executor::ExecutionContext;
use aegis_streaming::StreamingEngine;
use aegis_timeseries::TimeSeriesEngine;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::RwLock;
use parking_lot::RwLock as SyncRwLock;
use chrono::Utc;

// =============================================================================
// Application State
// =============================================================================

/// Shared application state with real engine integrations.
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<ServerConfig>,
    pub query_engine: Arc<QueryEngine>,
    pub document_engine: Arc<DocumentEngine>,
    pub timeseries_engine: Arc<TimeSeriesEngine>,
    pub streaming_engine: Arc<StreamingEngine>,
    pub kv_store: Arc<KvStore>,
    pub metrics: Arc<RwLock<Metrics>>,
    pub admin: Arc<AdminService>,
    pub auth: Arc<AuthService>,
    pub activity: Arc<ActivityLogger>,
    pub settings: Arc<RwLock<ServerSettings>>,
    pub metrics_history: Arc<RwLock<Vec<MetricsDataPoint>>>,
    pub graph_store: Arc<GraphStore>,
    pub rbac: Arc<RbacManager>,
    data_dir: Option<PathBuf>,
}

impl AppState {
    /// Create new application state with the given configuration.
    pub fn new(config: ServerConfig) -> Self {
        let activity = Arc::new(ActivityLogger::new());
        let data_dir = config.data_dir.as_ref().map(PathBuf::from);

        // Create data directory if specified
        if let Some(ref dir) = data_dir {
            if let Err(e) = std::fs::create_dir_all(dir) {
                tracing::error!("Failed to create data directory {:?}: {}", dir, e);
            }
        }

        // Initialize engines
        let document_engine = Arc::new(DocumentEngine::new());
        let kv_store = Arc::new(KvStore::new());

        // Load persisted data if data directory is specified
        if let Some(ref dir) = data_dir {
            // Load KV store
            let kv_path = dir.join("kv_store.json");
            if kv_path.exists() {
                if let Ok(data) = std::fs::read_to_string(&kv_path) {
                    if let Ok(entries) = serde_json::from_str::<Vec<KvEntry>>(&data) {
                        for entry in entries {
                            kv_store.set(entry.key, entry.value, entry.ttl);
                        }
                        tracing::info!("Loaded {} KV entries from disk", kv_store.count());
                    }
                }
            }

            // Load document collections
            let docs_dir = dir.join("documents");
            if docs_dir.exists() {
                if let Ok(entries) = std::fs::read_dir(&docs_dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.extension().map_or(false, |e| e == "json") {
                            if let Some(collection_name) = path.file_stem().and_then(|s| s.to_str()) {
                                if let Ok(data) = std::fs::read_to_string(&path) {
                                    if let Ok(docs) = serde_json::from_str::<Vec<serde_json::Value>>(&data) {
                                        let _ = document_engine.create_collection(collection_name);
                                        let mut count = 0;
                                        for doc_json in docs {
                                            let doc = json_to_document(doc_json);
                                            if document_engine.insert(collection_name, doc).is_ok() {
                                                count += 1;
                                            }
                                        }
                                        tracing::info!("Loaded {} documents into collection '{}'", count, collection_name);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Log server startup
        let node_name_display = config.node_name.as_ref().map(|n| format!(" ({})", n)).unwrap_or_default();
        activity.log_system(&format!("Aegis DB server started - Node: {}{}", config.node_id, node_name_display));

        // Create graph store with initial data
        let graph_store = Arc::new(GraphStore::new());
        graph_store.create_node("Person", serde_json::json!({"name": "Alice", "age": 30}));
        graph_store.create_node("Person", serde_json::json!({"name": "Bob", "age": 25}));
        graph_store.create_node("Person", serde_json::json!({"name": "Charlie", "age": 35}));
        graph_store.create_node("Company", serde_json::json!({"name": "TechCorp", "industry": "Technology"}));
        graph_store.create_node("Project", serde_json::json!({"name": "Project Alpha", "status": "active"}));

        // Create initial edges
        let nodes = graph_store.list_nodes();
        if nodes.len() >= 5 {
            let _ = graph_store.create_edge(&nodes[0].id, &nodes[3].id, "WORKS_AT");
            let _ = graph_store.create_edge(&nodes[1].id, &nodes[3].id, "WORKS_AT");
            let _ = graph_store.create_edge(&nodes[0].id, &nodes[1].id, "KNOWS");
            let _ = graph_store.create_edge(&nodes[0].id, &nodes[4].id, "WORKS_ON");
            let _ = graph_store.create_edge(&nodes[2].id, &nodes[4].id, "WORKS_ON");
        }

        // Initialize metrics history with some data points
        let metrics_history = Arc::new(RwLock::new(Vec::new()));

        // Start metrics collection background task
        let metrics_history_clone = metrics_history.clone();
        tokio::spawn(async move {
            Self::collect_metrics_loop(metrics_history_clone).await;
        });

        // Create admin service with cluster config
        let admin = Arc::new(AdminService::with_config(
            &config.node_id,
            config.node_name.clone(),
            &config.address(),
            &config.cluster_name,
            config.peers.clone(),
        ));

        Self {
            config: Arc::new(config),
            query_engine: Arc::new(QueryEngine::new()),
            document_engine,
            timeseries_engine: Arc::new(TimeSeriesEngine::new()),
            streaming_engine: Arc::new(StreamingEngine::new()),
            kv_store,
            metrics: Arc::new(RwLock::new(Metrics::default())),
            admin,
            auth: Arc::new(AuthService::new()),
            activity,
            settings: Arc::new(RwLock::new(ServerSettings::default())),
            metrics_history,
            graph_store,
            rbac: Arc::new(RbacManager::new()),
            data_dir,
        }
    }

    /// Save all data to disk (if data_dir is configured).
    pub fn save_to_disk(&self) -> std::io::Result<()> {
        let Some(ref dir) = self.data_dir else {
            return Ok(());
        };

        // Save KV store
        let kv_path = dir.join("kv_store.json");
        let entries = self.kv_store.list(None, usize::MAX);
        let json = serde_json::to_string_pretty(&entries)?;
        std::fs::write(&kv_path, json)?;
        tracing::debug!("Saved {} KV entries to disk", entries.len());

        // Save document collections
        let docs_dir = dir.join("documents");
        std::fs::create_dir_all(&docs_dir)?;

        for collection_name in self.document_engine.list_collections() {
            let query = aegis_document::Query::new();
            if let Ok(result) = self.document_engine.find(&collection_name, &query) {
                let docs: Vec<serde_json::Value> = result.documents
                    .iter()
                    .map(document_to_json)
                    .collect();
                let json = serde_json::to_string_pretty(&docs)?;
                let path = docs_dir.join(format!("{}.json", collection_name));
                std::fs::write(&path, json)?;
                tracing::debug!("Saved {} documents to collection '{}'", docs.len(), collection_name);
            }
        }

        Ok(())
    }

    /// Execute a SQL query.
    pub async fn execute_query(&self, sql: &str) -> Result<QueryResult, QueryError> {
        self.query_engine.execute(sql)
    }

    /// Record a request metric.
    pub async fn record_request(&self, duration_ms: u64, success: bool) {
        let mut metrics = self.metrics.write().await;
        metrics.total_requests += 1;
        metrics.total_duration_ms += duration_ms;
        if !success {
            metrics.failed_requests += 1;
        }
    }

    /// Background task to collect metrics periodically.
    async fn collect_metrics_loop(metrics_history: Arc<RwLock<Vec<MetricsDataPoint>>>) {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60));

        loop {
            interval.tick().await;

            let now = Utc::now().timestamp();

            // Collect system metrics (simulated for now, but uses real time)
            let point = MetricsDataPoint {
                timestamp: now,
                cpu_percent: 15.0 + (now % 30) as f64 + ((now % 7) as f64 * 2.0),
                memory_percent: 45.0 + ((now / 60) % 20) as f64,
                queries_per_second: 100.0 + ((now % 50) as f64 * 3.0),
                latency_ms: 2.5 + ((now % 10) as f64 * 0.3),
                connections: 25 + ((now / 120) as u64 % 50),
                bytes_in: 1_000_000 + ((now as u64 % 500_000)),
                bytes_out: 2_000_000 + ((now as u64 % 1_000_000)),
            };

            let mut history = metrics_history.write().await;

            // Keep last 30 days of minute-resolution data (43200 points)
            if history.len() >= 43200 {
                history.remove(0);
            }
            history.push(point);
        }
    }

    /// Initialize metrics history with historical data.
    pub async fn init_metrics_history(&self) {
        let now = Utc::now().timestamp();
        let mut history = self.metrics_history.write().await;

        // Generate 24 hours of historical data at 1-minute intervals
        for i in (0..1440).rev() {
            let timestamp = now - (i * 60);
            let hour_factor = ((timestamp / 3600) % 24) as f64;

            let point = MetricsDataPoint {
                timestamp,
                cpu_percent: 15.0 + hour_factor * 1.5 + ((timestamp % 30) as f64),
                memory_percent: 45.0 + ((timestamp / 60) % 20) as f64,
                queries_per_second: 80.0 + hour_factor * 8.0 + ((timestamp % 50) as f64 * 2.0),
                latency_ms: 2.0 + ((timestamp % 10) as f64 * 0.2),
                connections: 20 + (hour_factor as u64 * 3) + ((timestamp / 120) as u64 % 30),
                bytes_in: 800_000 + (hour_factor as u64 * 50_000) + (timestamp as u64 % 300_000),
                bytes_out: 1_500_000 + (hour_factor as u64 * 100_000) + (timestamp as u64 % 600_000),
            };
            history.push(point);
        }
    }

    /// Get comprehensive database statistics.
    pub fn get_database_stats(&self) -> DatabaseStats {
        // Count KV entries
        let total_keys = self.kv_store.count();

        // Count documents across all collections
        let collections = self.document_engine.list_collections();
        let total_documents: usize = collections.iter()
            .filter_map(|name| self.document_engine.collection_stats(name))
            .map(|stats| stats.document_count)
            .sum();

        // Get engine stats
        let engine_stats = self.document_engine.stats();

        DatabaseStats {
            total_keys,
            total_documents,
            collection_count: collections.len(),
            documents_inserted: engine_stats.documents_inserted,
            documents_updated: engine_stats.documents_updated,
            documents_deleted: engine_stats.documents_deleted,
            queries_executed: engine_stats.queries_executed,
        }
    }
}

/// Comprehensive database statistics.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DatabaseStats {
    pub total_keys: usize,
    pub total_documents: usize,
    pub collection_count: usize,
    pub documents_inserted: u64,
    pub documents_updated: u64,
    pub documents_deleted: u64,
    pub queries_executed: u64,
}

// =============================================================================
// Key-Value Store
// =============================================================================

/// In-memory key-value store with real persistence.
pub struct KvStore {
    data: SyncRwLock<HashMap<String, KvEntry>>,
}

/// Key-value entry with metadata.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct KvEntry {
    pub key: String,
    pub value: serde_json::Value,
    pub ttl: Option<u64>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl KvStore {
    pub fn new() -> Self {
        Self {
            data: SyncRwLock::new(HashMap::new()),
        }
    }

    /// Set a key-value pair.
    pub fn set(&self, key: String, value: serde_json::Value, ttl: Option<u64>) -> KvEntry {
        let now = chrono::Utc::now();
        let mut data = self.data.write();

        let entry = if let Some(existing) = data.get(&key) {
            KvEntry {
                key: key.clone(),
                value,
                ttl,
                created_at: existing.created_at,
                updated_at: now,
            }
        } else {
            KvEntry {
                key: key.clone(),
                value,
                ttl,
                created_at: now,
                updated_at: now,
            }
        };

        data.insert(key, entry.clone());
        entry
    }

    /// Get a value by key.
    pub fn get(&self, key: &str) -> Option<KvEntry> {
        let data = self.data.read();
        data.get(key).cloned()
    }

    /// Delete a key.
    pub fn delete(&self, key: &str) -> Option<KvEntry> {
        let mut data = self.data.write();
        data.remove(key)
    }

    /// List all keys with optional prefix filter.
    pub fn list(&self, prefix: Option<&str>, limit: usize) -> Vec<KvEntry> {
        let data = self.data.read();
        let iter = data.values();

        if let Some(p) = prefix {
            iter.filter(|e| e.key.starts_with(p))
                .take(limit)
                .cloned()
                .collect()
        } else {
            iter.take(limit).cloned().collect()
        }
    }

    /// Get total count of keys.
    pub fn count(&self) -> usize {
        self.data.read().len()
    }
}

impl Default for KvStore {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Graph Store
// =============================================================================

/// Graph node for visualization.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GraphNode {
    pub id: String,
    pub label: String,
    pub properties: serde_json::Value,
}

/// Graph edge for visualization.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GraphEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    pub relationship: String,
}

/// In-memory graph store.
pub struct GraphStore {
    nodes: SyncRwLock<HashMap<String, GraphNode>>,
    edges: SyncRwLock<HashMap<String, GraphEdge>>,
    node_counter: AtomicU64,
    edge_counter: AtomicU64,
}

impl GraphStore {
    pub fn new() -> Self {
        Self {
            nodes: SyncRwLock::new(HashMap::new()),
            edges: SyncRwLock::new(HashMap::new()),
            node_counter: AtomicU64::new(1),
            edge_counter: AtomicU64::new(1),
        }
    }

    /// Create a new node.
    pub fn create_node(&self, label: &str, properties: serde_json::Value) -> GraphNode {
        let id = format!("{}:{}", label.to_lowercase(), self.node_counter.fetch_add(1, Ordering::SeqCst));
        let node = GraphNode {
            id: id.clone(),
            label: label.to_string(),
            properties,
        };
        self.nodes.write().insert(id, node.clone());
        node
    }

    /// Create a new edge between nodes.
    pub fn create_edge(&self, source: &str, target: &str, relationship: &str) -> Result<GraphEdge, String> {
        let nodes = self.nodes.read();
        if !nodes.contains_key(source) {
            return Err(format!("Source node '{}' not found", source));
        }
        if !nodes.contains_key(target) {
            return Err(format!("Target node '{}' not found", target));
        }
        drop(nodes);

        let id = format!("e{}", self.edge_counter.fetch_add(1, Ordering::SeqCst));
        let edge = GraphEdge {
            id: id.clone(),
            source: source.to_string(),
            target: target.to_string(),
            relationship: relationship.to_string(),
        };
        self.edges.write().insert(id, edge.clone());
        Ok(edge)
    }

    /// Get a node by ID.
    pub fn get_node(&self, id: &str) -> Option<GraphNode> {
        self.nodes.read().get(id).cloned()
    }

    /// Delete a node and its edges.
    pub fn delete_node(&self, id: &str) -> Result<(), String> {
        let mut nodes = self.nodes.write();
        if nodes.remove(id).is_none() {
            return Err(format!("Node '{}' not found", id));
        }
        drop(nodes);

        // Remove edges connected to this node
        let mut edges = self.edges.write();
        edges.retain(|_, e| e.source != id && e.target != id);
        Ok(())
    }

    /// Delete an edge.
    pub fn delete_edge(&self, id: &str) -> Result<(), String> {
        if self.edges.write().remove(id).is_none() {
            return Err(format!("Edge '{}' not found", id));
        }
        Ok(())
    }

    /// List all nodes.
    pub fn list_nodes(&self) -> Vec<GraphNode> {
        self.nodes.read().values().cloned().collect()
    }

    /// List all edges.
    pub fn list_edges(&self) -> Vec<GraphEdge> {
        self.edges.read().values().cloned().collect()
    }

    /// Get all nodes and edges.
    pub fn get_all(&self) -> (Vec<GraphNode>, Vec<GraphEdge>) {
        (self.list_nodes(), self.list_edges())
    }

    /// Search nodes by label.
    pub fn find_by_label(&self, label: &str) -> Vec<GraphNode> {
        self.nodes.read()
            .values()
            .filter(|n| n.label.to_lowercase() == label.to_lowercase())
            .cloned()
            .collect()
    }

    /// Get edges for a node.
    pub fn get_edges_for_node(&self, node_id: &str) -> Vec<GraphEdge> {
        self.edges.read()
            .values()
            .filter(|e| e.source == node_id || e.target == node_id)
            .cloned()
            .collect()
    }
}

impl Default for GraphStore {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Query Engine Wrapper
// =============================================================================

/// Query engine for executing SQL statements.
/// Maintains a persistent ExecutionContext so DDL operations persist across queries.
pub struct QueryEngine {
    parser: Parser,
    planner: Planner,
    context: Arc<std::sync::RwLock<ExecutionContext>>,
}

impl QueryEngine {
    pub fn new() -> Self {
        let schema = Arc::new(PlannerSchema::new());
        Self {
            parser: Parser::new(),
            planner: Planner::new(schema),
            context: Arc::new(std::sync::RwLock::new(ExecutionContext::new())),
        }
    }

    /// Execute a SQL query.
    pub fn execute(&self, sql: &str) -> Result<QueryResult, QueryError> {
        let statements = self.parser.parse(sql)
            .map_err(|e| QueryError::Parse(e.to_string()))?;

        if statements.is_empty() {
            return Ok(QueryResult {
                columns: vec![],
                rows: vec![],
                rows_affected: 0,
            });
        }

        let statement = &statements[0];
        let plan = self.planner.plan(statement)
            .map_err(|e| QueryError::Plan(e.to_string()))?;

        // Use shared context so DDL operations persist across queries
        let executor = Executor::with_shared_context(self.context.clone());
        let result = executor.execute(&plan)
            .map_err(|e| QueryError::Execute(e.to_string()))?;

        Ok(QueryResult {
            columns: result.columns,
            rows: result.rows.into_iter().map(|r| {
                r.values.into_iter().map(value_to_json).collect()
            }).collect(),
            rows_affected: result.rows_affected,
        })
    }

    /// List all tables in the database.
    pub fn list_tables(&self) -> Vec<String> {
        self.context.read()
            .map(|ctx| ctx.list_tables())
            .unwrap_or_default()
    }

    /// Get table schema information.
    pub fn get_table_info(&self, name: &str) -> Option<TableInfo> {
        let ctx = self.context.read().ok()?;
        let schema = ctx.get_table_schema(name)?;
        let table_data = ctx.get_table(name)?;
        let row_count = table_data.read().ok().map(|t| t.rows.len() as u64);

        Some(TableInfo {
            name: schema.name.clone(),
            columns: schema.columns.iter().map(|c| ColumnInfo {
                name: c.name.clone(),
                data_type: format!("{:?}", c.data_type),
                nullable: c.nullable,
            }).collect(),
            row_count,
        })
    }
}

/// Table information for API response.
#[derive(Debug, Clone, serde::Serialize)]
pub struct TableInfo {
    pub name: String,
    pub columns: Vec<ColumnInfo>,
    pub row_count: Option<u64>,
}

/// Column information for API response.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ColumnInfo {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
}

impl Default for QueryEngine {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Query Result
// =============================================================================

/// Result of a query execution.
#[derive(Debug, Clone, serde::Serialize)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<serde_json::Value>>,
    pub rows_affected: u64,
}

/// Convert aegis Value to JSON.
fn value_to_json(value: aegis_common::Value) -> serde_json::Value {
    match value {
        aegis_common::Value::Null => serde_json::Value::Null,
        aegis_common::Value::Boolean(b) => serde_json::Value::Bool(b),
        aegis_common::Value::Integer(i) => serde_json::Value::Number(i.into()),
        aegis_common::Value::Float(f) => {
            serde_json::Number::from_f64(f)
                .map(serde_json::Value::Number)
                .unwrap_or(serde_json::Value::Null)
        }
        aegis_common::Value::String(s) => serde_json::Value::String(s),
        aegis_common::Value::Bytes(b) => {
            serde_json::Value::String(base64_encode(&b))
        }
        aegis_common::Value::Timestamp(t) => {
            serde_json::Value::String(t.to_rfc3339())
        }
        aegis_common::Value::Array(arr) => {
            serde_json::Value::Array(arr.into_iter().map(value_to_json).collect())
        }
        aegis_common::Value::Object(obj) => {
            let map: serde_json::Map<String, serde_json::Value> = obj
                .into_iter()
                .map(|(k, v)| (k, value_to_json(v)))
                .collect();
            serde_json::Value::Object(map)
        }
    }
}

fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::new();

    for chunk in data.chunks(3) {
        let b0 = chunk[0] as usize;
        let b1 = chunk.get(1).copied().unwrap_or(0) as usize;
        let b2 = chunk.get(2).copied().unwrap_or(0) as usize;

        result.push(CHARS[b0 >> 2] as char);
        result.push(CHARS[((b0 & 0x03) << 4) | (b1 >> 4)] as char);

        if chunk.len() > 1 {
            result.push(CHARS[((b1 & 0x0f) << 2) | (b2 >> 6)] as char);
        } else {
            result.push('=');
        }

        if chunk.len() > 2 {
            result.push(CHARS[b2 & 0x3f] as char);
        } else {
            result.push('=');
        }
    }

    result
}

// =============================================================================
// Query Error
// =============================================================================

/// Errors during query execution.
#[derive(Debug, thiserror::Error)]
pub enum QueryError {
    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Planning error: {0}")]
    Plan(String),

    #[error("Execution error: {0}")]
    Execute(String),
}

// =============================================================================
// Metrics
// =============================================================================

/// Server metrics.
#[derive(Debug, Default, Clone, serde::Serialize)]
pub struct Metrics {
    pub total_requests: u64,
    pub failed_requests: u64,
    pub total_duration_ms: u64,
}

impl Metrics {
    /// Calculate average request duration.
    pub fn avg_duration_ms(&self) -> f64 {
        if self.total_requests == 0 {
            0.0
        } else {
            self.total_duration_ms as f64 / self.total_requests as f64
        }
    }

    /// Calculate success rate.
    pub fn success_rate(&self) -> f64 {
        if self.total_requests == 0 {
            1.0
        } else {
            1.0 - (self.failed_requests as f64 / self.total_requests as f64)
        }
    }
}

// =============================================================================
// Document Persistence Helpers
// =============================================================================

/// Convert JSON to Document for loading from disk.
fn json_to_document(json: serde_json::Value) -> Document {
    let mut doc = if let Some(id) = json.get("_id").and_then(|v| v.as_str()) {
        Document::with_id(id)
    } else {
        Document::new()
    };

    if let serde_json::Value::Object(map) = json {
        for (key, value) in map {
            if key != "_id" {
                doc.set(&key, json_to_doc_value(value));
            }
        }
    }
    doc
}

/// Convert Document to JSON for saving to disk.
fn document_to_json(doc: &Document) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    map.insert("_id".to_string(), serde_json::Value::String(doc.id.to_string()));
    for (key, value) in &doc.data {
        map.insert(key.clone(), doc_value_to_json(value));
    }
    serde_json::Value::Object(map)
}

/// Convert JSON value to aegis_document::Value.
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

/// Convert aegis_document::Value to JSON.
fn doc_value_to_json(value: &aegis_document::Value) -> serde_json::Value {
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
            serde_json::Value::Array(arr.iter().map(doc_value_to_json).collect())
        }
        aegis_document::Value::Object(obj) => {
            let map: serde_json::Map<String, serde_json::Value> = obj
                .iter()
                .map(|(k, v)| (k.clone(), doc_value_to_json(v)))
                .collect();
            serde_json::Value::Object(map)
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
    fn test_metrics_calculations() {
        let mut metrics = Metrics::default();
        metrics.total_requests = 100;
        metrics.failed_requests = 10;
        metrics.total_duration_ms = 5000;

        assert_eq!(metrics.avg_duration_ms(), 50.0);
        assert!((metrics.success_rate() - 0.9).abs() < 0.001);
    }

    #[test]
    fn test_value_to_json() {
        let value = aegis_common::Value::String("test".to_string());
        let json = value_to_json(value);
        assert_eq!(json, serde_json::Value::String("test".to_string()));
    }

    #[test]
    fn test_kv_store_operations() {
        let store = KvStore::new();

        // Set
        let entry = store.set("key1".to_string(), serde_json::json!("value1"), None);
        assert_eq!(entry.key, "key1");
        assert_eq!(entry.value, serde_json::json!("value1"));

        // Get
        let retrieved = store.get("key1").unwrap();
        assert_eq!(retrieved.value, serde_json::json!("value1"));

        // List
        store.set("key2".to_string(), serde_json::json!("value2"), None);
        let all = store.list(None, 100);
        assert_eq!(all.len(), 2);

        // Delete
        let deleted = store.delete("key1");
        assert!(deleted.is_some());
        assert!(store.get("key1").is_none());
    }
}
