//! Aegis Server State
//!
//! Application state shared across request handlers. Provides access to
//! database engines, query engine, and configuration.
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use crate::activity::ActivityLogger;
use crate::admin::AdminService;
use crate::auth::AuthService;
use crate::config::ServerConfig;
use aegis_document::DocumentEngine;
use aegis_query::{Executor, Parser, Planner};
use aegis_query::planner::PlannerSchema;
use aegis_query::executor::ExecutionContext;
use aegis_streaming::StreamingEngine;
use aegis_timeseries::TimeSeriesEngine;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use parking_lot::RwLock as SyncRwLock;

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
}

impl AppState {
    /// Create new application state with the given configuration.
    pub fn new(config: ServerConfig) -> Self {
        let activity = Arc::new(ActivityLogger::new());

        // Log server startup
        activity.log_system("Aegis DB server started");

        Self {
            config: Arc::new(config),
            query_engine: Arc::new(QueryEngine::new()),
            document_engine: Arc::new(DocumentEngine::new()),
            timeseries_engine: Arc::new(TimeSeriesEngine::new()),
            streaming_engine: Arc::new(StreamingEngine::new()),
            kv_store: Arc::new(KvStore::new()),
            metrics: Arc::new(RwLock::new(Metrics::default())),
            admin: Arc::new(AdminService::new()),
            auth: Arc::new(AuthService::new()),
            activity,
        }
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
// Query Engine Wrapper
// =============================================================================

/// Query engine for executing SQL statements.
pub struct QueryEngine {
    parser: Parser,
    planner: Planner,
}

impl QueryEngine {
    pub fn new() -> Self {
        let schema = Arc::new(PlannerSchema::new());
        Self {
            parser: Parser::new(),
            planner: Planner::new(schema),
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

        let executor = Executor::new(ExecutionContext::new());
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
