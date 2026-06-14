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

        Ok(parse_query_result(&response_body))
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

    /// Get many keys at once (missing keys are omitted from the result).
    pub async fn kv_batch_get(&self, keys: &[&str]) -> Result<serde_json::Value, ClientError> {
        self.send_json(
            reqwest::Method::POST,
            "/api/v1/kv/batch/get",
            Some(serde_json::json!({ "keys": keys })),
        )
        .await
    }

    /// Set many keys at once. Each entry is `(key, value, ttl_seconds)`.
    pub async fn kv_batch_set(
        &self,
        entries: Vec<(String, serde_json::Value, Option<u64>)>,
    ) -> Result<serde_json::Value, ClientError> {
        let arr: Vec<serde_json::Value> = entries
            .into_iter()
            .map(|(key, value, ttl)| serde_json::json!({ "key": key, "value": value, "ttl": ttl }))
            .collect();
        self.send_json(
            reqwest::Method::POST,
            "/api/v1/kv/batch/set",
            Some(serde_json::json!({ "entries": arr })),
        )
        .await
    }

    /// Delete many keys at once. Returns `{ deleted: N }`.
    pub async fn kv_batch_delete(&self, keys: &[&str]) -> Result<serde_json::Value, ClientError> {
        self.send_json(
            reqwest::Method::POST,
            "/api/v1/kv/batch/delete",
            Some(serde_json::json!({ "keys": keys })),
        )
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
            Some(serde_json::json!({ "document": document })),
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
            Some(serde_json::json!({ "document": partial })),
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

    /// Query documents with a MongoDB-style filter, optional limit/skip, and an
    /// optional opaque pagination `cursor` (from a prior response's
    /// `next_cursor`). The response includes `next_cursor` when more pages exist.
    pub async fn query_documents(
        &self,
        collection: &str,
        filter: serde_json::Value,
        limit: Option<usize>,
        skip: Option<usize>,
        cursor: Option<&str>,
    ) -> Result<serde_json::Value, ClientError> {
        let body =
            serde_json::json!({ "filter": filter, "limit": limit, "skip": skip, "cursor": cursor });
        self.send_json(
            reqwest::Method::POST,
            &format!("/api/v1/documents/collections/{}/query", collection),
            Some(body),
        )
        .await
    }

    /// Insert many documents into a collection in one call.
    pub async fn bulk_insert_documents(
        &self,
        collection: &str,
        documents: Vec<serde_json::Value>,
    ) -> Result<serde_json::Value, ClientError> {
        self.send_json(
            reqwest::Method::POST,
            &format!("/api/v1/documents/collections/{}/batch-insert", collection),
            Some(serde_json::json!({ "documents": documents })),
        )
        .await
    }

    /// Delete many documents by id in one call. Returns `{ deleted: N }`.
    pub async fn bulk_delete_documents(
        &self,
        collection: &str,
        ids: &[&str],
    ) -> Result<serde_json::Value, ClientError> {
        self.send_json(
            reqwest::Method::POST,
            &format!("/api/v1/documents/collections/{}/batch-delete", collection),
            Some(serde_json::json!({ "ids": ids })),
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

    // ---- Prepared statements ------------------------------------------------

    /// Prepare a statement (parsed and planned once server-side). Returns the
    /// statement id to use with [`Connection::execute_prepared`].
    pub async fn prepare(&self, sql: &str) -> Result<String, ClientError> {
        let body = serde_json::json!({ "database": &self.config.database, "sql": sql });
        let response = self
            .send_json(reqwest::Method::POST, "/api/v1/prepare", Some(body))
            .await?;
        response
            .get("statement_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| ClientError::QueryFailed("no statement_id in response".to_string()))
    }

    /// Execute a prepared statement with bound parameters.
    pub async fn execute_prepared(
        &self,
        statement_id: &str,
        params: Vec<Value>,
    ) -> Result<QueryResult, ClientError> {
        let body = serde_json::json!({
            "statement_id": statement_id,
            "params": params.iter().map(value_to_json).collect::<Vec<_>>(),
        });
        let response = self
            .send_json(
                reqwest::Method::POST,
                "/api/v1/prepared/execute",
                Some(body),
            )
            .await?;
        Ok(parse_query_result(&response))
    }

    /// Deallocate a prepared statement.
    pub async fn deallocate(&self, statement_id: &str) -> Result<(), ClientError> {
        self.send_json(
            reqwest::Method::DELETE,
            &format!("/api/v1/prepared/{}", statement_id),
            None,
        )
        .await?;
        Ok(())
    }

    // ---- Vector / KNN -------------------------------------------------------

    /// Create a vector collection with a fixed `dim` and `metric`
    /// (`"cosine"`, `"l2"`, or `"dot"`).
    pub async fn create_vector_collection(
        &self,
        name: &str,
        dim: usize,
        metric: &str,
    ) -> Result<serde_json::Value, ClientError> {
        self.send_json(
            reqwest::Method::POST,
            "/api/v1/vector/collections",
            Some(serde_json::json!({ "name": name, "dim": dim, "metric": metric })),
        )
        .await
    }

    /// List vector collections.
    pub async fn list_vector_collections(&self) -> Result<serde_json::Value, ClientError> {
        self.send_json(reqwest::Method::GET, "/api/v1/vector/collections", None)
            .await
    }

    /// Stats for a vector collection (dim, metric, count).
    pub async fn vector_collection_stats(
        &self,
        name: &str,
    ) -> Result<serde_json::Value, ClientError> {
        self.send_json(
            reqwest::Method::GET,
            &format!("/api/v1/vector/collections/{}", name),
            None,
        )
        .await
    }

    /// Drop a vector collection.
    pub async fn drop_vector_collection(&self, name: &str) -> Result<(), ClientError> {
        self.send_json(
            reqwest::Method::DELETE,
            &format!("/api/v1/vector/collections/{}", name),
            None,
        )
        .await?;
        Ok(())
    }

    /// Upsert a single vector with optional JSON metadata.
    pub async fn vector_upsert(
        &self,
        collection: &str,
        id: &str,
        vector: &[f32],
        metadata: serde_json::Value,
    ) -> Result<(), ClientError> {
        self.send_json(
            reqwest::Method::POST,
            &format!("/api/v1/vector/collections/{}/upsert", collection),
            Some(serde_json::json!({ "id": id, "vector": vector, "metadata": metadata })),
        )
        .await?;
        Ok(())
    }

    /// Batch-upsert many vectors. Each item is `{ id, vector, metadata? }`.
    pub async fn vector_upsert_batch(
        &self,
        collection: &str,
        vectors: serde_json::Value,
    ) -> Result<serde_json::Value, ClientError> {
        self.send_json(
            reqwest::Method::POST,
            &format!("/api/v1/vector/collections/{}/batch", collection),
            Some(serde_json::json!({ "vectors": vectors })),
        )
        .await
    }

    /// Get a stored vector by id, or `None` if absent.
    pub async fn get_vector(
        &self,
        collection: &str,
        id: &str,
    ) -> Result<Option<serde_json::Value>, ClientError> {
        if !self.is_connected() {
            return Err(ClientError::NotConnected);
        }
        self.mark_used();
        let url = format!(
            "{}/api/v1/vector/collections/{}/vectors/{}",
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
                "get_vector failed: {}",
                response.status()
            )));
        }
        let value: serde_json::Value = response
            .json()
            .await
            .map_err(|e| ClientError::QueryFailed(e.to_string()))?;
        Ok(Some(value))
    }

    /// Delete a vector by id.
    pub async fn delete_vector(&self, collection: &str, id: &str) -> Result<(), ClientError> {
        self.send_json(
            reqwest::Method::DELETE,
            &format!("/api/v1/vector/collections/{}/vectors/{}", collection, id),
            None,
        )
        .await?;
        Ok(())
    }

    /// KNN search: returns the response `{ hits: [{ id, score, distance,
    /// metadata }], count }`. `filter` is an exact-match metadata object (use
    /// `Value::Null` for none).
    pub async fn vector_search(
        &self,
        collection: &str,
        query: &[f32],
        k: usize,
        ef: Option<usize>,
        filter: serde_json::Value,
    ) -> Result<serde_json::Value, ClientError> {
        self.send_json(
            reqwest::Method::POST,
            &format!("/api/v1/vector/collections/{}/search", collection),
            Some(serde_json::json!({ "vector": query, "k": k, "ef": ef, "filter": filter })),
        )
        .await
    }

    // ---- Full-text search ---------------------------------------------------

    /// Create a full-text (BM25) index.
    pub async fn create_fts_index(&self, name: &str) -> Result<serde_json::Value, ClientError> {
        self.send_json(
            reqwest::Method::POST,
            "/api/v1/fts/indexes",
            Some(serde_json::json!({ "name": name })),
        )
        .await
    }

    /// List full-text indexes.
    pub async fn list_fts_indexes(&self) -> Result<serde_json::Value, ClientError> {
        self.send_json(reqwest::Method::GET, "/api/v1/fts/indexes", None)
            .await
    }

    /// Full-text index stats.
    pub async fn fts_index_stats(&self, name: &str) -> Result<serde_json::Value, ClientError> {
        self.send_json(
            reqwest::Method::GET,
            &format!("/api/v1/fts/indexes/{}", name),
            None,
        )
        .await
    }

    /// Drop a full-text index.
    pub async fn drop_fts_index(&self, name: &str) -> Result<(), ClientError> {
        self.send_json(
            reqwest::Method::DELETE,
            &format!("/api/v1/fts/indexes/{}", name),
            None,
        )
        .await?;
        Ok(())
    }

    /// Index (insert or replace) a document with optional metadata.
    pub async fn fts_index_document(
        &self,
        index: &str,
        id: &str,
        text: &str,
        metadata: serde_json::Value,
    ) -> Result<(), ClientError> {
        self.send_json(
            reqwest::Method::POST,
            &format!("/api/v1/fts/indexes/{}/documents", index),
            Some(serde_json::json!({ "id": id, "text": text, "metadata": metadata })),
        )
        .await?;
        Ok(())
    }

    /// Get an indexed document by id, or `None` if absent.
    pub async fn fts_get_document(
        &self,
        index: &str,
        id: &str,
    ) -> Result<Option<serde_json::Value>, ClientError> {
        if !self.is_connected() {
            return Err(ClientError::NotConnected);
        }
        self.mark_used();
        let url = format!(
            "{}/api/v1/fts/indexes/{}/documents/{}",
            self.base_url, index, id
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
                "fts_get_document failed: {}",
                response.status()
            )));
        }
        let value: serde_json::Value = response
            .json()
            .await
            .map_err(|e| ClientError::QueryFailed(e.to_string()))?;
        Ok(Some(value))
    }

    /// Delete a document from a full-text index.
    pub async fn fts_delete_document(&self, index: &str, id: &str) -> Result<(), ClientError> {
        self.send_json(
            reqwest::Method::DELETE,
            &format!("/api/v1/fts/indexes/{}/documents/{}", index, id),
            None,
        )
        .await?;
        Ok(())
    }

    /// BM25 search over a full-text index. `filter` is an exact-match metadata
    /// object (`Value::Null` for none).
    pub async fn fts_search(
        &self,
        index: &str,
        query: &str,
        k: usize,
        filter: serde_json::Value,
    ) -> Result<serde_json::Value, ClientError> {
        self.send_json(
            reqwest::Method::POST,
            &format!("/api/v1/fts/indexes/{}/search", index),
            Some(serde_json::json!({ "query": query, "k": k, "filter": filter })),
        )
        .await
    }

    // ---- Geospatial (grid index + Haversine) ----------------------------

    /// Create a geo collection.
    pub async fn create_geo_collection(
        &self,
        name: &str,
    ) -> Result<serde_json::Value, ClientError> {
        self.send_json(
            reqwest::Method::POST,
            "/api/v1/geo/collections",
            Some(serde_json::json!({ "name": name })),
        )
        .await
    }

    /// List geo collections.
    pub async fn list_geo_collections(&self) -> Result<serde_json::Value, ClientError> {
        self.send_json(reqwest::Method::GET, "/api/v1/geo/collections", None)
            .await
    }

    /// Geo collection stats.
    pub async fn geo_collection_stats(&self, name: &str) -> Result<serde_json::Value, ClientError> {
        self.send_json(
            reqwest::Method::GET,
            &format!("/api/v1/geo/collections/{}", name),
            None,
        )
        .await
    }

    /// Drop a geo collection.
    pub async fn drop_geo_collection(&self, name: &str) -> Result<(), ClientError> {
        self.send_json(
            reqwest::Method::DELETE,
            &format!("/api/v1/geo/collections/{}", name),
            None,
        )
        .await?;
        Ok(())
    }

    /// Upsert a feature `(id, lat, lon)` with optional metadata.
    pub async fn geo_upsert_feature(
        &self,
        collection: &str,
        id: &str,
        lat: f64,
        lon: f64,
        metadata: serde_json::Value,
    ) -> Result<(), ClientError> {
        self.send_json(
            reqwest::Method::POST,
            &format!("/api/v1/geo/collections/{}/features", collection),
            Some(serde_json::json!({ "id": id, "lat": lat, "lon": lon, "metadata": metadata })),
        )
        .await?;
        Ok(())
    }

    /// Get a feature by id, or `None` if absent.
    pub async fn geo_get_feature(
        &self,
        collection: &str,
        id: &str,
    ) -> Result<Option<serde_json::Value>, ClientError> {
        if !self.is_connected() {
            return Err(ClientError::NotConnected);
        }
        self.mark_used();
        let url = format!(
            "{}/api/v1/geo/collections/{}/features/{}",
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
                "geo_get_feature failed: {}",
                response.status()
            )));
        }
        let value: serde_json::Value = response
            .json()
            .await
            .map_err(|e| ClientError::QueryFailed(e.to_string()))?;
        Ok(Some(value))
    }

    /// Delete a feature by id.
    pub async fn geo_delete_feature(&self, collection: &str, id: &str) -> Result<(), ClientError> {
        self.send_json(
            reqwest::Method::DELETE,
            &format!("/api/v1/geo/collections/{}/features/{}", collection, id),
            None,
        )
        .await?;
        Ok(())
    }

    /// Features within `radius_m` metres of `(lat, lon)`, nearest first.
    /// `filter` is an exact-match metadata object (`Value::Null` for none).
    pub async fn geo_radius(
        &self,
        collection: &str,
        lat: f64,
        lon: f64,
        radius_m: f64,
        filter: serde_json::Value,
    ) -> Result<serde_json::Value, ClientError> {
        self.send_json(
            reqwest::Method::POST,
            &format!("/api/v1/geo/collections/{}/radius", collection),
            Some(serde_json::json!({ "lat": lat, "lon": lon, "radius_m": radius_m, "filter": filter })),
        )
        .await
    }

    /// Features inside a bounding box. `filter` is an exact-match metadata
    /// object (`Value::Null` for none).
    pub async fn geo_bbox(
        &self,
        collection: &str,
        min_lat: f64,
        min_lon: f64,
        max_lat: f64,
        max_lon: f64,
        filter: serde_json::Value,
    ) -> Result<serde_json::Value, ClientError> {
        self.send_json(
            reqwest::Method::POST,
            &format!("/api/v1/geo/collections/{}/bbox", collection),
            Some(serde_json::json!({
                "min_lat": min_lat, "min_lon": min_lon,
                "max_lat": max_lat, "max_lon": max_lon, "filter": filter
            })),
        )
        .await
    }

    /// The `k` nearest features to `(lat, lon)`. `filter` is an exact-match
    /// metadata object (`Value::Null` for none).
    pub async fn geo_nearest(
        &self,
        collection: &str,
        lat: f64,
        lon: f64,
        k: usize,
        filter: serde_json::Value,
    ) -> Result<serde_json::Value, ClientError> {
        self.send_json(
            reqwest::Method::POST,
            &format!("/api/v1/geo/collections/{}/nearest", collection),
            Some(serde_json::json!({ "lat": lat, "lon": lon, "k": k, "filter": filter })),
        )
        .await
    }

    // ---- Columnar / OLAP ------------------------------------------------

    /// Create a columnar table. `columns` is a list of `{name, type}` where
    /// `type` is one of `int` / `float` / `text` / `bool`.
    pub async fn create_columnar_table(
        &self,
        name: &str,
        columns: serde_json::Value,
    ) -> Result<serde_json::Value, ClientError> {
        self.send_json(
            reqwest::Method::POST,
            "/api/v1/columnar/tables",
            Some(serde_json::json!({ "name": name, "columns": columns })),
        )
        .await
    }

    /// List columnar tables.
    pub async fn list_columnar_tables(&self) -> Result<serde_json::Value, ClientError> {
        self.send_json(reqwest::Method::GET, "/api/v1/columnar/tables", None)
            .await
    }

    /// Columnar table stats (row count + schema).
    pub async fn columnar_table_stats(&self, name: &str) -> Result<serde_json::Value, ClientError> {
        self.send_json(
            reqwest::Method::GET,
            &format!("/api/v1/columnar/tables/{}", name),
            None,
        )
        .await
    }

    /// Drop a columnar table.
    pub async fn drop_columnar_table(&self, name: &str) -> Result<(), ClientError> {
        self.send_json(
            reqwest::Method::DELETE,
            &format!("/api/v1/columnar/tables/{}", name),
            None,
        )
        .await?;
        Ok(())
    }

    /// Insert many rows into a columnar table.
    pub async fn columnar_insert(
        &self,
        table: &str,
        rows: serde_json::Value,
    ) -> Result<serde_json::Value, ClientError> {
        self.send_json(
            reqwest::Method::POST,
            &format!("/api/v1/columnar/tables/{}/rows", table),
            Some(serde_json::json!({ "rows": rows })),
        )
        .await
    }

    /// Scan rows with optional column projection, filter, and limit.
    /// `filter` is a list of `{column, op, value}` conditions (ANDed).
    pub async fn columnar_scan(
        &self,
        table: &str,
        columns: serde_json::Value,
        filter: serde_json::Value,
        limit: Option<usize>,
    ) -> Result<serde_json::Value, ClientError> {
        self.send_json(
            reqwest::Method::POST,
            &format!("/api/v1/columnar/tables/{}/scan", table),
            Some(serde_json::json!({ "columns": columns, "filter": filter, "limit": limit })),
        )
        .await
    }

    /// Group-by aggregation. `aggregates` is a list of `{func, column}` where
    /// `func` is one of `count`/`sum`/`min`/`max`/`avg`.
    pub async fn columnar_aggregate(
        &self,
        table: &str,
        group_by: serde_json::Value,
        aggregates: serde_json::Value,
        filter: serde_json::Value,
    ) -> Result<serde_json::Value, ClientError> {
        self.send_json(
            reqwest::Method::POST,
            &format!("/api/v1/columnar/tables/{}/aggregate", table),
            Some(serde_json::json!({
                "group_by": group_by, "aggregates": aggregates, "filter": filter
            })),
        )
        .await
    }

    /// Distinct non-null values of a column.
    pub async fn columnar_distinct(
        &self,
        table: &str,
        column: &str,
    ) -> Result<serde_json::Value, ClientError> {
        self.send_json(
            reqwest::Method::GET,
            &format!("/api/v1/columnar/tables/{}/distinct/{}", table, column),
            None,
        )
        .await
    }

    // ---- Object / blob store --------------------------------------------

    /// Create an object bucket.
    pub async fn create_bucket(&self, name: &str) -> Result<serde_json::Value, ClientError> {
        self.send_json(
            reqwest::Method::POST,
            "/api/v1/objects/buckets",
            Some(serde_json::json!({ "name": name })),
        )
        .await
    }

    /// List object buckets.
    pub async fn list_buckets(&self) -> Result<serde_json::Value, ClientError> {
        self.send_json(reqwest::Method::GET, "/api/v1/objects/buckets", None)
            .await
    }

    /// Bucket stats (object count + total bytes).
    pub async fn bucket_stats(&self, name: &str) -> Result<serde_json::Value, ClientError> {
        self.send_json(
            reqwest::Method::GET,
            &format!("/api/v1/objects/buckets/{}", name),
            None,
        )
        .await
    }

    /// Drop an object bucket.
    pub async fn drop_bucket(&self, name: &str) -> Result<(), ClientError> {
        self.send_json(
            reqwest::Method::DELETE,
            &format!("/api/v1/objects/buckets/{}", name),
            None,
        )
        .await?;
        Ok(())
    }

    /// List object metadata in a bucket (optional key prefix + limit).
    pub async fn list_objects(
        &self,
        bucket: &str,
        prefix: Option<&str>,
        limit: Option<usize>,
    ) -> Result<serde_json::Value, ClientError> {
        let mut path = format!("/api/v1/objects/buckets/{}/objects", bucket);
        let mut q: Vec<String> = Vec::new();
        if let Some(p) = prefix {
            q.push(format!("prefix={}", p));
        }
        if let Some(l) = limit {
            q.push(format!("limit={}", l));
        }
        if !q.is_empty() {
            path.push('?');
            path.push_str(&q.join("&"));
        }
        self.send_json(reqwest::Method::GET, &path, None).await
    }

    /// Store (or replace) an object — the raw bytes are the content. Returns the
    /// stored object's metadata (including its ETag).
    pub async fn put_object(
        &self,
        bucket: &str,
        key: &str,
        data: Vec<u8>,
        content_type: Option<&str>,
        metadata: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, ClientError> {
        if !self.is_connected() {
            return Err(ClientError::NotConnected);
        }
        self.mark_used();
        let url = format!(
            "{}/api/v1/objects/buckets/{}/object/{}",
            self.base_url, bucket, key
        );
        let mut req = self
            .add_auth(self.http_client.put(&url))
            .header(
                reqwest::header::CONTENT_TYPE,
                content_type.unwrap_or("application/octet-stream"),
            )
            .body(data);
        if let Some(meta) = metadata {
            req = req.header("X-Aegis-Meta", meta.to_string());
        }
        let response = req
            .send()
            .await
            .map_err(|e| ClientError::QueryFailed(e.to_string()))?;
        if !response.status().is_success() {
            return Err(ClientError::QueryFailed(format!(
                "put_object failed: {}",
                response.status()
            )));
        }
        response
            .json()
            .await
            .map_err(|e| ClientError::QueryFailed(e.to_string()))
    }

    /// Fetch an object's raw bytes, or `None` if absent.
    pub async fn get_object(
        &self,
        bucket: &str,
        key: &str,
    ) -> Result<Option<Vec<u8>>, ClientError> {
        if !self.is_connected() {
            return Err(ClientError::NotConnected);
        }
        self.mark_used();
        let url = format!(
            "{}/api/v1/objects/buckets/{}/object/{}",
            self.base_url, bucket, key
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
                "get_object failed: {}",
                response.status()
            )));
        }
        let bytes = response
            .bytes()
            .await
            .map_err(|e| ClientError::QueryFailed(e.to_string()))?;
        Ok(Some(bytes.to_vec()))
    }

    /// Fetch an object's metadata only (HEAD), or `None` if absent.
    pub async fn head_object(
        &self,
        bucket: &str,
        key: &str,
    ) -> Result<Option<serde_json::Value>, ClientError> {
        if !self.is_connected() {
            return Err(ClientError::NotConnected);
        }
        self.mark_used();
        let url = format!(
            "{}/api/v1/objects/buckets/{}/object/{}?meta=1",
            self.base_url, bucket, key
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
                "head_object failed: {}",
                response.status()
            )));
        }
        let value = response
            .json()
            .await
            .map_err(|e| ClientError::QueryFailed(e.to_string()))?;
        Ok(Some(value))
    }

    /// Delete an object.
    pub async fn delete_object(&self, bucket: &str, key: &str) -> Result<(), ClientError> {
        self.send_json(
            reqwest::Method::DELETE,
            &format!("/api/v1/objects/buckets/{}/object/{}", bucket, key),
            None,
        )
        .await?;
        Ok(())
    }

    // ---- Wide-column ----------------------------------------------------

    /// Create a wide-column table.
    pub async fn create_wide_table(&self, name: &str) -> Result<serde_json::Value, ClientError> {
        self.send_json(
            reqwest::Method::POST,
            "/api/v1/widecolumn/tables",
            Some(serde_json::json!({ "name": name })),
        )
        .await
    }

    /// List wide-column tables.
    pub async fn list_wide_tables(&self) -> Result<serde_json::Value, ClientError> {
        self.send_json(reqwest::Method::GET, "/api/v1/widecolumn/tables", None)
            .await
    }

    /// Wide-column table stats (rows + cells).
    pub async fn wide_table_stats(&self, name: &str) -> Result<serde_json::Value, ClientError> {
        self.send_json(
            reqwest::Method::GET,
            &format!("/api/v1/widecolumn/tables/{}", name),
            None,
        )
        .await
    }

    /// Drop a wide-column table.
    pub async fn drop_wide_table(&self, name: &str) -> Result<(), ClientError> {
        self.send_json(
            reqwest::Method::DELETE,
            &format!("/api/v1/widecolumn/tables/{}", name),
            None,
        )
        .await?;
        Ok(())
    }

    /// Set columns on a row (last-write-wins; optional explicit timestamp).
    pub async fn wide_put_row(
        &self,
        table: &str,
        row: &str,
        columns: serde_json::Value,
        timestamp: Option<u64>,
    ) -> Result<serde_json::Value, ClientError> {
        self.send_json(
            reqwest::Method::PUT,
            &format!("/api/v1/widecolumn/tables/{}/rows/{}", table, row),
            Some(serde_json::json!({ "columns": columns, "timestamp": timestamp })),
        )
        .await
    }

    /// Get a row, optionally projecting a subset of columns.
    pub async fn wide_get_row(
        &self,
        table: &str,
        row: &str,
        columns: &[&str],
    ) -> Result<Option<serde_json::Value>, ClientError> {
        if !self.is_connected() {
            return Err(ClientError::NotConnected);
        }
        self.mark_used();
        let mut url = format!(
            "{}/api/v1/widecolumn/tables/{}/rows/{}",
            self.base_url, table, row
        );
        if !columns.is_empty() {
            url.push_str("?columns=");
            url.push_str(&columns.join(","));
        }
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
                "wide_get_row failed: {}",
                response.status()
            )));
        }
        let value = response
            .json()
            .await
            .map_err(|e| ClientError::QueryFailed(e.to_string()))?;
        Ok(Some(value))
    }

    /// Delete a row.
    pub async fn wide_delete_row(&self, table: &str, row: &str) -> Result<(), ClientError> {
        self.send_json(
            reqwest::Method::DELETE,
            &format!("/api/v1/widecolumn/tables/{}/rows/{}", table, row),
            None,
        )
        .await?;
        Ok(())
    }

    /// Delete a single column (cell) from a row.
    pub async fn wide_delete_cell(
        &self,
        table: &str,
        row: &str,
        column: &str,
    ) -> Result<(), ClientError> {
        self.send_json(
            reqwest::Method::DELETE,
            &format!(
                "/api/v1/widecolumn/tables/{}/rows/{}/columns/{}",
                table, row, column
            ),
            None,
        )
        .await?;
        Ok(())
    }

    /// Scan rows in key order (range / prefix / projection / limit).
    pub async fn wide_scan(
        &self,
        table: &str,
        body: serde_json::Value,
    ) -> Result<serde_json::Value, ClientError> {
        self.send_json(
            reqwest::Method::POST,
            &format!("/api/v1/widecolumn/tables/{}/scan", table),
            Some(body),
        )
        .await
    }

    // ---- Ledger / append-only -------------------------------------------

    /// Create a ledger.
    pub async fn create_ledger(&self, name: &str) -> Result<serde_json::Value, ClientError> {
        self.send_json(
            reqwest::Method::POST,
            "/api/v1/ledger/ledgers",
            Some(serde_json::json!({ "name": name })),
        )
        .await
    }

    /// List ledgers.
    pub async fn list_ledgers(&self) -> Result<serde_json::Value, ClientError> {
        self.send_json(reqwest::Method::GET, "/api/v1/ledger/ledgers", None)
            .await
    }

    /// Ledger stats (entry count + chain-tip hash).
    pub async fn ledger_stats(&self, name: &str) -> Result<serde_json::Value, ClientError> {
        self.send_json(
            reqwest::Method::GET,
            &format!("/api/v1/ledger/ledgers/{}", name),
            None,
        )
        .await
    }

    /// Drop a ledger.
    pub async fn drop_ledger(&self, name: &str) -> Result<(), ClientError> {
        self.send_json(
            reqwest::Method::DELETE,
            &format!("/api/v1/ledger/ledgers/{}", name),
            None,
        )
        .await?;
        Ok(())
    }

    /// Append a payload to a ledger; returns the immutable entry (seq + hash).
    pub async fn ledger_append(
        &self,
        ledger: &str,
        payload: serde_json::Value,
        timestamp: Option<u64>,
    ) -> Result<serde_json::Value, ClientError> {
        self.send_json(
            reqwest::Method::POST,
            &format!("/api/v1/ledger/ledgers/{}/entries", ledger),
            Some(serde_json::json!({ "payload": payload, "timestamp": timestamp })),
        )
        .await
    }

    /// Read entries from `start`, capped at `limit`.
    pub async fn ledger_entries(
        &self,
        ledger: &str,
        start: u64,
        limit: Option<usize>,
    ) -> Result<serde_json::Value, ClientError> {
        let mut path = format!("/api/v1/ledger/ledgers/{}/entries?start={}", ledger, start);
        if let Some(l) = limit {
            path.push_str(&format!("&limit={}", l));
        }
        self.send_json(reqwest::Method::GET, &path, None).await
    }

    /// Get a single entry by sequence number, or `None` if absent.
    pub async fn ledger_get_entry(
        &self,
        ledger: &str,
        seq: u64,
    ) -> Result<Option<serde_json::Value>, ClientError> {
        if !self.is_connected() {
            return Err(ClientError::NotConnected);
        }
        self.mark_used();
        let url = format!(
            "{}/api/v1/ledger/ledgers/{}/entries/{}",
            self.base_url, ledger, seq
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
                "ledger_get_entry failed: {}",
                response.status()
            )));
        }
        let value = response
            .json()
            .await
            .map_err(|e| ClientError::QueryFailed(e.to_string()))?;
        Ok(Some(value))
    }

    /// Verify a ledger's hash chain; returns `{ valid, entries, broken_at }`.
    pub async fn ledger_verify(&self, ledger: &str) -> Result<serde_json::Value, ClientError> {
        self.send_json(
            reqwest::Method::GET,
            &format!("/api/v1/ledger/ledgers/{}/verify", ledger),
            None,
        )
        .await
    }
}

// =============================================================================
// Value Conversion
// =============================================================================

/// Parse a `{ "data": { columns, rows } }` query response body into a typed
/// [`QueryResult`]. Shared by `query` and `execute_prepared`.
fn parse_query_result(response_body: &serde_json::Value) -> QueryResult {
    let data = response_body.get("data");

    let columns: Vec<Column> = data
        .and_then(|d| d.get("columns"))
        .and_then(|c| c.as_array())
        .map(|cols| {
            cols.iter()
                .map(|c| Column::new(c.as_str().unwrap_or(""), DataType::Text))
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

    QueryResult::new(columns, rows)
}

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
