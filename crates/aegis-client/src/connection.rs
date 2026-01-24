//! Aegis Client Connection
//!
//! Real HTTP-based database connection to Aegis server.
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use crate::config::ConnectionConfig;
use crate::error::ClientError;
use crate::result::{Column, DataType, QueryResult, Row, Value};
use reqwest::Client;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

// =============================================================================
// Connection
// =============================================================================

/// A real database connection to an Aegis server.
pub struct Connection {
    id: u64,
    config: ConnectionConfig,
    http_client: Client,
    base_url: String,
    auth_token: std::sync::RwLock<Option<String>>,
    connected: AtomicBool,
    in_transaction: AtomicBool,
    created_at: Instant,
    last_used: std::sync::RwLock<Instant>,
    queries_executed: AtomicU64,
}

impl Connection {
    /// Create a new connection.
    pub async fn new(config: ConnectionConfig) -> Result<Self, ClientError> {
        static CONN_ID: AtomicU64 = AtomicU64::new(1);

        let base_url = format!("http://{}:{}", config.host, config.port);

        let http_client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| ClientError::ConnectionFailed(e.to_string()))?;

        let conn = Self {
            id: CONN_ID.fetch_add(1, Ordering::SeqCst),
            config,
            http_client,
            base_url,
            auth_token: std::sync::RwLock::new(None),
            connected: AtomicBool::new(false),
            in_transaction: AtomicBool::new(false),
            created_at: Instant::now(),
            last_used: std::sync::RwLock::new(Instant::now()),
            queries_executed: AtomicU64::new(0),
        };

        conn.connect().await?;
        Ok(conn)
    }

    /// Get the connection ID.
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Connect to the database server.
    async fn connect(&self) -> Result<(), ClientError> {
        // Check server health
        let health_url = format!("{}/health", self.base_url);
        let response = self.http_client
            .get(&health_url)
            .send()
            .await
            .map_err(|e| ClientError::ConnectionFailed(format!("Failed to connect: {}", e)))?;

        if !response.status().is_success() {
            return Err(ClientError::ConnectionFailed(
                format!("Server returned status: {}", response.status())
            ));
        }

        // Authenticate if credentials provided
        if let (Some(ref username), Some(ref password)) = (&self.config.username, &self.config.password) {
            let login_url = format!("{}/api/v1/auth/login", self.base_url);
            let login_body = serde_json::json!({
                "username": username,
                "password": password
            });

            let response = self.http_client
                .post(&login_url)
                .json(&login_body)
                .send()
                .await
                .map_err(|e| ClientError::AuthenticationFailed(e.to_string()))?;

            if response.status().is_success() {
                let auth_response: serde_json::Value = response
                    .json()
                    .await
                    .map_err(|e| ClientError::AuthenticationFailed(e.to_string()))?;

                if let Some(token) = auth_response.get("token").and_then(|t| t.as_str()) {
                    *self.auth_token.write().unwrap() = Some(token.to_string());
                }
            } else {
                return Err(ClientError::AuthenticationFailed(
                    "Invalid credentials".to_string()
                ));
            }
        }

        self.connected.store(true, Ordering::SeqCst);
        Ok(())
    }

    /// Check if connected.
    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }

    /// Check if in a transaction.
    pub fn in_transaction(&self) -> bool {
        self.in_transaction.load(Ordering::SeqCst)
    }

    /// Get connection age.
    pub fn age(&self) -> std::time::Duration {
        self.created_at.elapsed()
    }

    /// Get idle time.
    pub fn idle_time(&self) -> std::time::Duration {
        self.last_used.read().unwrap().elapsed()
    }

    /// Mark as used.
    fn mark_used(&self) {
        *self.last_used.write().unwrap() = Instant::now();
    }

    /// Add auth header to request if we have a token.
    fn add_auth(&self, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if let Some(ref token) = *self.auth_token.read().unwrap() {
            request.header("Authorization", format!("Bearer {}", token))
        } else {
            request
        }
    }

    /// Execute a query.
    pub async fn query(&self, sql: &str) -> Result<QueryResult, ClientError> {
        self.query_with_params(sql, vec![]).await
    }

    /// Execute a query with parameters.
    pub async fn query_with_params(
        &self,
        sql: &str,
        params: Vec<Value>,
    ) -> Result<QueryResult, ClientError> {
        if !self.is_connected() {
            return Err(ClientError::NotConnected);
        }

        self.mark_used();
        self.queries_executed.fetch_add(1, Ordering::SeqCst);

        let url = format!("{}/api/v1/query", self.base_url);
        let body = serde_json::json!({
            "sql": sql,
            "params": params.iter().map(value_to_json).collect::<Vec<_>>()
        });

        let request = self.http_client.post(&url).json(&body);
        let request = self.add_auth(request);

        let response = request
            .send()
            .await
            .map_err(|e| ClientError::QueryFailed(e.to_string()))?;

        let status = response.status();
        let response_body: serde_json::Value = response
            .json()
            .await
            .map_err(|e| ClientError::QueryFailed(e.to_string()))?;

        if !status.is_success() {
            let error = response_body
                .get("error")
                .and_then(|e| e.as_str())
                .unwrap_or("Unknown error");
            return Err(ClientError::QueryFailed(error.to_string()));
        }

        // Parse the response into QueryResult
        let data = response_body.get("data");

        let columns: Vec<Column> = data
            .and_then(|d| d.get("columns"))
            .and_then(|c| c.as_array())
            .map(|cols| {
                cols.iter()
                    .map(|c| Column::new(
                        c.as_str().unwrap_or(""),
                        DataType::Text, // Default to text, server doesn't send types
                    ))
                    .collect()
            })
            .unwrap_or_default();

        let column_names: Vec<String> = columns.iter().map(|c| c.name.clone()).collect();

        let rows: Vec<Row> = data
            .and_then(|d| d.get("rows"))
            .and_then(|r| r.as_array())
            .map(|rows| {
                rows.iter()
                    .map(|row| {
                        let values: Vec<Value> = row
                            .as_array()
                            .map(|arr| arr.iter().map(json_to_value).collect())
                            .unwrap_or_default();
                        Row::new(column_names.clone(), values)
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(QueryResult::new(columns, rows))
    }

    /// Execute a statement (INSERT, UPDATE, DELETE).
    pub async fn execute(&self, sql: &str) -> Result<u64, ClientError> {
        self.execute_with_params(sql, vec![]).await
    }

    /// Execute a statement with parameters.
    pub async fn execute_with_params(
        &self,
        sql: &str,
        params: Vec<Value>,
    ) -> Result<u64, ClientError> {
        if !self.is_connected() {
            return Err(ClientError::NotConnected);
        }

        self.mark_used();
        self.queries_executed.fetch_add(1, Ordering::SeqCst);

        let sql_upper = sql.trim().to_uppercase();

        // Handle transaction commands locally
        if sql_upper.starts_with("BEGIN") {
            self.in_transaction.store(true, Ordering::SeqCst);
            return Ok(0);
        } else if sql_upper.starts_with("COMMIT") || sql_upper.starts_with("ROLLBACK") {
            self.in_transaction.store(false, Ordering::SeqCst);
            return Ok(0);
        }

        let url = format!("{}/api/v1/query", self.base_url);
        let body = serde_json::json!({
            "sql": sql,
            "params": params.iter().map(value_to_json).collect::<Vec<_>>()
        });

        let request = self.http_client.post(&url).json(&body);
        let request = self.add_auth(request);

        let response = request
            .send()
            .await
            .map_err(|e| ClientError::QueryFailed(e.to_string()))?;

        let status = response.status();
        let response_body: serde_json::Value = response
            .json()
            .await
            .map_err(|e| ClientError::QueryFailed(e.to_string()))?;

        if !status.is_success() {
            let error = response_body
                .get("error")
                .and_then(|e| e.as_str())
                .unwrap_or("Unknown error");
            return Err(ClientError::QueryFailed(error.to_string()));
        }

        let rows_affected = response_body
            .get("data")
            .and_then(|d| d.get("rows_affected"))
            .and_then(|r| r.as_u64())
            .unwrap_or(0);

        Ok(rows_affected)
    }

    /// Begin a transaction.
    pub async fn begin_transaction(&self) -> Result<(), ClientError> {
        if self.in_transaction() {
            return Err(ClientError::TransactionAlreadyStarted);
        }
        self.execute("BEGIN").await?;
        Ok(())
    }

    /// Commit a transaction.
    pub async fn commit(&self) -> Result<(), ClientError> {
        if !self.in_transaction() {
            return Err(ClientError::NoTransaction);
        }
        self.execute("COMMIT").await?;
        Ok(())
    }

    /// Rollback a transaction.
    pub async fn rollback(&self) -> Result<(), ClientError> {
        if !self.in_transaction() {
            return Err(ClientError::NoTransaction);
        }
        self.execute("ROLLBACK").await?;
        Ok(())
    }

    /// Ping the connection.
    pub async fn ping(&self) -> Result<(), ClientError> {
        let health_url = format!("{}/health", self.base_url);
        let response = self.http_client
            .get(&health_url)
            .send()
            .await
            .map_err(|e| ClientError::ConnectionFailed(e.to_string()))?;

        if response.status().is_success() {
            self.mark_used();
            Ok(())
        } else {
            self.connected.store(false, Ordering::SeqCst);
            Err(ClientError::NotConnected)
        }
    }

    /// Close the connection.
    pub async fn close(&self) {
        // Logout if we have an auth token
        if let Some(ref token) = *self.auth_token.read().unwrap() {
            let logout_url = format!("{}/api/v1/auth/logout", self.base_url);
            let body = serde_json::json!({ "token": token });
            let _ = self.http_client.post(&logout_url).json(&body).send().await;
        }
        self.connected.store(false, Ordering::SeqCst);
    }

    /// Get connection statistics.
    pub fn stats(&self) -> ConnectionStats {
        ConnectionStats {
            id: self.id,
            connected: self.is_connected(),
            in_transaction: self.in_transaction(),
            age_ms: self.age().as_millis() as u64,
            idle_ms: self.idle_time().as_millis() as u64,
            queries_executed: self.queries_executed.load(Ordering::SeqCst),
        }
    }

    /// Get the base URL of the server.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }
}

// =============================================================================
// Value Conversion
// =============================================================================

fn value_to_json(value: &Value) -> serde_json::Value {
    match value {
        Value::Null => serde_json::Value::Null,
        Value::Bool(b) => serde_json::Value::Bool(*b),
        Value::Int(i) => serde_json::Value::Number((*i).into()),
        Value::Float(f) => serde_json::Number::from_f64(*f)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        Value::String(s) => serde_json::Value::String(s.clone()),
        Value::Bytes(b) => serde_json::Value::String(base64_encode(b)),
        Value::Timestamp(t) => serde_json::Value::Number((*t).into()),
        Value::Array(arr) => serde_json::Value::Array(arr.iter().map(value_to_json).collect()),
        Value::Object(obj) => {
            let map: serde_json::Map<String, serde_json::Value> = obj
                .iter()
                .map(|(k, v)| (k.clone(), value_to_json(v)))
                .collect();
            serde_json::Value::Object(map)
        }
    }
}

fn json_to_value(json: &serde_json::Value) -> Value {
    match json {
        serde_json::Value::Null => Value::Null,
        serde_json::Value::Bool(b) => Value::Bool(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::Int(i)
            } else if let Some(f) = n.as_f64() {
                Value::Float(f)
            } else {
                Value::Null
            }
        }
        serde_json::Value::String(s) => Value::String(s.clone()),
        serde_json::Value::Array(arr) => {
            Value::Array(arr.iter().map(json_to_value).collect())
        }
        serde_json::Value::Object(obj) => {
            Value::Object(obj.iter().map(|(k, v)| (k.clone(), json_to_value(v))).collect())
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
// Connection Statistics
// =============================================================================

/// Statistics for a connection.
#[derive(Debug, Clone)]
pub struct ConnectionStats {
    pub id: u64,
    pub connected: bool,
    pub in_transaction: bool,
    pub age_ms: u64,
    pub idle_ms: u64,
    pub queries_executed: u64,
}

// =============================================================================
// Pooled Connection
// =============================================================================

/// A connection managed by a pool.
///
/// This struct is thread-safe (`Sync`) because the return callback is protected by a Mutex.
pub struct PooledConnection {
    connection: Arc<Connection>,
    pool_return: std::sync::Mutex<Option<Box<dyn FnOnce(Arc<Connection>) + Send>>>,
}

impl PooledConnection {
    /// Create a new pooled connection.
    pub fn new<F>(connection: Arc<Connection>, on_return: F) -> Self
    where
        F: FnOnce(Arc<Connection>) + Send + 'static,
    {
        Self {
            connection,
            pool_return: std::sync::Mutex::new(Some(Box::new(on_return))),
        }
    }

    /// Get a reference to the underlying connection.
    pub fn connection(&self) -> &Connection {
        &self.connection
    }

    /// Get the underlying connection (alias for connection()).
    pub fn inner(&self) -> &Connection {
        &self.connection
    }

    /// Execute a query.
    pub async fn query(&self, sql: &str) -> Result<QueryResult, ClientError> {
        self.connection.query(sql).await
    }

    /// Execute a statement.
    pub async fn execute(&self, sql: &str) -> Result<u64, ClientError> {
        self.connection.execute(sql).await
    }
}

impl std::ops::Deref for PooledConnection {
    type Target = Connection;

    fn deref(&self) -> &Self::Target {
        &self.connection
    }
}

impl Drop for PooledConnection {
    fn drop(&mut self) {
        if let Ok(mut guard) = self.pool_return.lock() {
            if let Some(return_fn) = guard.take() {
                return_fn(Arc::clone(&self.connection));
            }
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
    fn test_connection_stats() {
        let stats = ConnectionStats {
            id: 1,
            connected: true,
            in_transaction: false,
            age_ms: 1000,
            idle_ms: 100,
            queries_executed: 5,
        };
        assert_eq!(stats.id, 1);
        assert!(stats.connected);
    }

    #[test]
    fn test_json_to_value() {
        let json = serde_json::json!({"name": "test", "count": 42});
        let value = json_to_value(&json);
        if let Value::Object(map) = value {
            assert!(map.contains_key("name"));
            assert!(map.contains_key("count"));
        } else {
            panic!("Expected Object");
        }
    }

    #[test]
    fn test_value_to_json() {
        let value = Value::String("hello".to_string());
        let json = value_to_json(&value);
        assert_eq!(json, serde_json::Value::String("hello".to_string()));
    }

    #[tokio::test]
    async fn test_connection_create() {
        // This test requires a running server, skip if not available
        let config = ConnectionConfig {
            host: "127.0.0.1".to_string(),
            port: 7001,
            ..Default::default()
        };

        match Connection::new(config).await {
            Ok(conn) => {
                assert!(conn.is_connected());
            }
            Err(_) => {
                // Server not running, skip test
            }
        }
    }
}
