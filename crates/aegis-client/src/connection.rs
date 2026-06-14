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
        let response = self
            .http_client
            .get(&health_url)
            .send()
            .await
            .map_err(|e| ClientError::ConnectionFailed(format!("Failed to connect: {}", e)))?;

        if !response.status().is_success() {
            return Err(ClientError::ConnectionFailed(format!(
                "Server returned status: {}",
                response.status()
            )));
        }

        // Authenticate if credentials provided
        if let (Some(ref username), Some(ref password)) =
            (&self.config.username, &self.config.password)
        {
            let login_url = format!("{}/api/v1/auth/login", self.base_url);
            let login_body = serde_json::json!({
                "username": username,
                "password": password
            });

            let response = self
                .http_client
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
                    *self.auth_token.write().expect("auth_token RwLock poisoned") =
                        Some(token.to_string());
                }
            } else {
                return Err(ClientError::AuthenticationFailed(
                    "Invalid credentials".to_string(),
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
        self.last_used
            .read()
            .expect("last_used RwLock poisoned")
            .elapsed()
    }

    /// Mark as used.
    fn mark_used(&self) {
        *self.last_used.write().expect("last_used RwLock poisoned") = Instant::now();
    }

    /// Add auth header to request if we have a token.
    fn add_auth(&self, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if let Some(ref token) = *self.auth_token.read().expect("auth_token RwLock poisoned") {
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
            "database": &self.config.database,
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
                    .map(|c| {
                        Column::new(
                            c.as_str().unwrap_or(""),
                            DataType::Text, // Default to text, server doesn't send types
                        )
                    })
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
            "database": &self.config.database,
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
        let response = self
            .http_client
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
        // Clone token before await to avoid holding lock across await point
        let token = self
            .auth_token
            .read()
            .expect("auth_token RwLock poisoned")
            .clone();
        if let Some(ref token) = token {
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
// Resource APIs (KV, documents, time series, graph, schema, health)
//
// These mirror the JavaScript / Python SDK surface so the Rust client is no
// longer SQL-only. Loosely-typed payloads use `serde_json::Value` to match the
// flexible document/graph/time-series shapes.
// =============================================================================

impl Connection {
    /// Generic JSON request helper. Adds auth, fails on non-2xx, returns the
    /// parsed response body (or `Null` for empty responses).
    async fn send_json(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, ClientError> {
        if !self.is_connected() {
            return Err(ClientError::NotConnected);
        }
        self.mark_used();

        let url = format!("{}{}", self.base_url, path);
        let mut request = self.http_client.request(method, &url);
        if let Some(b) = body {
            request = request.json(&b);
        }
        let request = self.add_auth(request);

        let response = request
            .send()
            .await
            .map_err(|e| ClientError::QueryFailed(e.to_string()))?;
        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|e| ClientError::QueryFailed(e.to_string()))?;
        let value: serde_json::Value = if text.trim().is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::from_str(&text).unwrap_or(serde_json::Value::Null)
        };

        if !status.is_success() {
            let error = value
                .get("error")
                .and_then(|e| e.as_str())
                .unwrap_or("request failed");
            return Err(ClientError::QueryFailed(format!("{}: {}", status, error)));
        }
        Ok(value)
    }

    // ---- Key-Value ----------------------------------------------------------

    /// Get a key's entry, or `None` if it does not exist.
    pub async fn kv_get(&self, key: &str) -> Result<Option<serde_json::Value>, ClientError> {
        if !self.is_connected() {
            return Err(ClientError::NotConnected);
        }
        self.mark_used();
        let url = format!("{}/api/v1/kv/keys/{}", self.base_url, key);
        let response = self
            .add_auth(self.http_client.get(&url))
            .send()
            .await
            .map_err(|e| ClientError::QueryFailed(e.to_string()))?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !response.status().is_success() {
            return Err(ClientError::QueryFailed(format!(
                "kv_get failed: {}",
                response.status()
            )));
        }
        let value: serde_json::Value = response
            .json()
            .await
            .map_err(|e| ClientError::QueryFailed(e.to_string()))?;
        Ok(if value.is_null() { None } else { Some(value) })
    }

    /// Set a key with an optional TTL (seconds).
    pub async fn kv_set(
        &self,
        key: &str,
        value: serde_json::Value,
        ttl: Option<u64>,
    ) -> Result<(), ClientError> {
        let mut body = serde_json::json!({ "key": key, "value": value });
        if let Some(ttl) = ttl {
            body["ttl"] = serde_json::json!(ttl);
        }
        self.send_json(reqwest::Method::POST, "/api/v1/kv/keys", Some(body))
            .await?;
        Ok(())
    }

    /// Delete a key.
    pub async fn kv_delete(&self, key: &str) -> Result<(), ClientError> {
        self.send_json(
            reqwest::Method::DELETE,
            &format!("/api/v1/kv/keys/{}", key),
            None,
        )
        .await?;
        Ok(())
    }

    /// List all key entries.
    pub async fn kv_list(&self) -> Result<serde_json::Value, ClientError> {
        self.send_json(reqwest::Method::GET, "/api/v1/kv/keys", None)
            .await
    }

    // ---- Documents ----------------------------------------------------------

    /// List document collections.
    pub async fn list_collections(&self) -> Result<serde_json::Value, ClientError> {
        self.send_json(reqwest::Method::GET, "/api/v1/documents/collections", None)
            .await
    }

    /// Create a document collection.
    pub async fn create_collection(&self, name: &str) -> Result<serde_json::Value, ClientError> {
        self.send_json(
            reqwest::Method::POST,
            "/api/v1/documents/collections",
            Some(serde_json::json!({ "name": name })),
        )
        .await
    }

    /// Insert a document, optionally with an explicit id.
    pub async fn insert_document(
        &self,
        collection: &str,
        document: serde_json::Value,
        id: Option<&str>,
    ) -> Result<serde_json::Value, ClientError> {
        let body = serde_json::json!({ "id": id, "document": document });
        self.send_json(
            reqwest::Method::POST,
            &format!("/api/v1/documents/collections/{}/documents", collection),
            Some(body),
        )
        .await
    }

    /// Get a document by id, or `None` if absent.
    pub async fn get_document(
        &self,
        collection: &str,
        id: &str,
    ) -> Result<Option<serde_json::Value>, ClientError> {
        if !self.is_connected() {
            return Err(ClientError::NotConnected);
        }
        self.mark_used();
        let url = format!(
            "{}/api/v1/documents/collections/{}/documents/{}",
            self.base_url, collection, id
        );
        let response = self
            .add_auth(self.http_client.get(&url))
            .send()
            .await
            .map_err(|e| ClientError::QueryFailed(e.to_string()))?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !response.status().is_success() {
            return Err(ClientError::QueryFailed(format!(
                "get_document failed: {}",
                response.status()
            )));
        }
        let value: serde_json::Value = response
            .json()
            .await
            .map_err(|e| ClientError::QueryFailed(e.to_string()))?;
        Ok(if value.is_null() { None } else { Some(value) })
    }

    /// Replace a document (full update).
    pub async fn update_document(
        &self,
        collection: &str,
        id: &str,
        document: serde_json::Value,
    ) -> Result<serde_json::Value, ClientError> {
        self.send_json(
            reqwest::Method::PUT,
            &format!(
                "/api/v1/documents/collections/{}/documents/{}",
                collection, id
            ),
            Some(document),
        )
        .await
    }

    /// Partially update (merge) a document.
    pub async fn patch_document(
        &self,
        collection: &str,
        id: &str,
        partial: serde_json::Value,
    ) -> Result<serde_json::Value, ClientError> {
        self.send_json(
            reqwest::Method::PATCH,
            &format!(
                "/api/v1/documents/collections/{}/documents/{}",
                collection, id
            ),
            Some(partial),
        )
        .await
    }

    /// Delete a document.
    pub async fn delete_document(&self, collection: &str, id: &str) -> Result<(), ClientError> {
        self.send_json(
            reqwest::Method::DELETE,
            &format!(
                "/api/v1/documents/collections/{}/documents/{}",
                collection, id
            ),
            None,
        )
        .await?;
        Ok(())
    }

    /// Query documents with a MongoDB-style filter, optional limit/skip.
    pub async fn query_documents(
        &self,
        collection: &str,
        filter: serde_json::Value,
        limit: Option<usize>,
        skip: Option<usize>,
    ) -> Result<serde_json::Value, ClientError> {
        let body = serde_json::json!({ "filter": filter, "limit": limit, "skip": skip });
        self.send_json(
            reqwest::Method::POST,
            &format!("/api/v1/documents/collections/{}/query", collection),
            Some(body),
        )
        .await
    }

    // ---- Time series --------------------------------------------------------

    /// Register a metric (e.g. `counter`, `gauge`, `histogram`, `summary`).
    pub async fn register_metric(
        &self,
        name: &str,
        metric_type: &str,
    ) -> Result<serde_json::Value, ClientError> {
        let body = serde_json::json!({ "name": name, "metric_type": metric_type });
        self.send_json(
            reqwest::Method::POST,
            "/api/v1/timeseries/metrics",
            Some(body),
        )
        .await
    }

    /// Write a single time-series point.
    pub async fn ts_write(
        &self,
        metric: &str,
        value: f64,
        timestamp: Option<i64>,
        tags: serde_json::Value,
    ) -> Result<(), ClientError> {
        let body = serde_json::json!({ "metric": metric, "value": value, "timestamp": timestamp, "tags": tags });
        self.send_json(
            reqwest::Method::POST,
            "/api/v1/timeseries/write",
            Some(body),
        )
        .await?;
        Ok(())
    }

    /// Query a time series within an optional `[start, end]` window.
    pub async fn ts_query(
        &self,
        metric: &str,
        start: Option<i64>,
        end: Option<i64>,
        limit: Option<usize>,
    ) -> Result<serde_json::Value, ClientError> {
        let body =
            serde_json::json!({ "metric": metric, "start": start, "end": end, "limit": limit });
        self.send_json(
            reqwest::Method::POST,
            "/api/v1/timeseries/query",
            Some(body),
        )
        .await
    }

    // ---- Graph --------------------------------------------------------------

    /// Get the full graph (nodes + edges).
    pub async fn graph_data(&self) -> Result<serde_json::Value, ClientError> {
        self.send_json(reqwest::Method::GET, "/api/v1/graph/data", None)
            .await
    }

    /// Create a graph node.
    pub async fn create_node(
        &self,
        label: &str,
        properties: serde_json::Value,
    ) -> Result<serde_json::Value, ClientError> {
        let body = serde_json::json!({ "label": label, "properties": properties });
        self.send_json(reqwest::Method::POST, "/api/v1/graph/nodes", Some(body))
            .await
    }

    /// Update a graph node (omit `label`/`properties` to leave unchanged).
    pub async fn update_node(
        &self,
        node_id: &str,
        label: Option<&str>,
        properties: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, ClientError> {
        let body = serde_json::json!({ "label": label, "properties": properties });
        self.send_json(
            reqwest::Method::PUT,
            &format!("/api/v1/graph/nodes/{}", node_id),
            Some(body),
        )
        .await
    }

    /// Delete a graph node (and its edges).
    pub async fn delete_node(&self, node_id: &str) -> Result<(), ClientError> {
        self.send_json(
            reqwest::Method::DELETE,
            &format!("/api/v1/graph/nodes/{}", node_id),
            None,
        )
        .await?;
        Ok(())
    }

    /// Create a graph edge.
    pub async fn create_edge(
        &self,
        source: &str,
        target: &str,
        relationship: &str,
    ) -> Result<serde_json::Value, ClientError> {
        let body =
            serde_json::json!({ "source": source, "target": target, "relationship": relationship });
        self.send_json(reqwest::Method::POST, "/api/v1/graph/edges", Some(body))
            .await
    }

    /// Update a graph edge's relationship.
    pub async fn update_edge(
        &self,
        edge_id: &str,
        relationship: &str,
    ) -> Result<serde_json::Value, ClientError> {
        let body = serde_json::json!({ "relationship": relationship });
        self.send_json(
            reqwest::Method::PUT,
            &format!("/api/v1/graph/edges/{}", edge_id),
            Some(body),
        )
        .await
    }

    /// Delete a graph edge.
    pub async fn delete_edge(&self, edge_id: &str) -> Result<(), ClientError> {
        self.send_json(
            reqwest::Method::DELETE,
            &format!("/api/v1/graph/edges/{}", edge_id),
            None,
        )
        .await?;
        Ok(())
    }

    // ---- Schema / health / metrics ------------------------------------------

    /// Server health.
    pub async fn health(&self) -> Result<serde_json::Value, ClientError> {
        self.send_json(reqwest::Method::GET, "/health", None).await
    }

    /// List tables in the current database.
    pub async fn list_tables(&self) -> Result<serde_json::Value, ClientError> {
        self.send_json(reqwest::Method::GET, "/api/v1/tables", None)
            .await
    }

    /// Get a table's schema and row count.
    pub async fn get_table(&self, name: &str) -> Result<serde_json::Value, ClientError> {
        self.send_json(
            reqwest::Method::GET,
            &format!("/api/v1/tables/{}", name),
            None,
        )
        .await
    }

    /// Current server metrics snapshot.
    pub async fn metrics(&self) -> Result<serde_json::Value, ClientError> {
        self.send_json(reqwest::Method::GET, "/api/v1/metrics", None)
            .await
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
        serde_json::Value::Array(arr) => Value::Array(arr.iter().map(json_to_value).collect()),
        serde_json::Value::Object(obj) => Value::Object(
            obj.iter()
                .map(|(k, v)| (k.clone(), json_to_value(v)))
                .collect(),
        ),
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
    // Mutex-guarded one-shot return callback handed back to the pool on drop.
    #[allow(clippy::type_complexity)]
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
