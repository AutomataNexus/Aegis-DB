//! Aegis Client - Database Client SDK
//!
//! Native Rust client library for connecting to Aegis database instances.
//! Provides async/await API, connection pooling, and automatic failover.
//!
//! Key Features:
//! - Async-first API with tokio integration
//! - Connection pooling and management
//! - Automatic retry and failover
//! - Query builder with type safety
//! - Cluster-aware routing
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

pub mod config;
pub mod connection;
pub mod error;
pub mod pool;
pub mod query;
pub mod result;
pub mod transaction;

pub use config::{ClientConfig, ConnectionConfig};
pub use connection::Connection;
pub use error::ClientError;
pub use pool::ConnectionPool;
pub use query::{Query, QueryBuilder};
pub use result::{QueryResult, Row, Value};
pub use transaction::Transaction;

/// The main client for interacting with Aegis databases.
pub struct AegisClient {
    #[allow(dead_code)]
    config: ClientConfig,
    pool: ConnectionPool,
}

impl AegisClient {
    /// Create a new client with the given configuration.
    pub async fn new(config: ClientConfig) -> Result<Self, ClientError> {
        let pool =
            ConnectionPool::with_connection_config(config.pool.clone(), config.connection.clone())
                .await?;
        Ok(Self { config, pool })
    }

    /// Connect to a database with default configuration.
    pub async fn connect(url: &str) -> Result<Self, ClientError> {
        let config = ClientConfig::from_url(url)?;
        Self::new(config).await
    }

    /// Execute a query and return results.
    pub async fn query(&self, sql: &str) -> Result<QueryResult, ClientError> {
        let conn = self.pool.get().await?;
        conn.query(sql).await
    }

    /// Execute a query with parameters.
    pub async fn query_with_params(
        &self,
        sql: &str,
        params: Vec<Value>,
    ) -> Result<QueryResult, ClientError> {
        let conn = self.pool.get().await?;
        conn.query_with_params(sql, params).await
    }

    /// Execute a statement (INSERT, UPDATE, DELETE).
    pub async fn execute(&self, sql: &str) -> Result<u64, ClientError> {
        let conn = self.pool.get().await?;
        conn.execute(sql).await
    }

    /// Execute a statement with parameters.
    pub async fn execute_with_params(
        &self,
        sql: &str,
        params: Vec<Value>,
    ) -> Result<u64, ClientError> {
        let conn = self.pool.get().await?;
        conn.execute_with_params(sql, params).await
    }

    /// Start a new transaction.
    pub async fn begin(&self) -> Result<Transaction, ClientError> {
        let conn = self.pool.get().await?;
        Transaction::begin(conn).await
    }

    /// Create a query builder.
    pub fn query_builder(&self) -> QueryBuilder {
        QueryBuilder::new()
    }

    /// Get connection pool statistics.
    pub fn pool_stats(&self) -> pool::PoolStats {
        self.pool.stats()
    }

    /// Check if the client is connected.
    pub async fn is_connected(&self) -> bool {
        self.pool.is_healthy().await
    }

    /// Close all connections.
    pub async fn close(&self) {
        self.pool.close().await;
    }

    // ---- Key-Value ----------------------------------------------------------

    /// Get a key's entry, or `None` if absent.
    pub async fn kv_get(&self, key: &str) -> Result<Option<serde_json::Value>, ClientError> {
        self.pool.get().await?.kv_get(key).await
    }

    /// Set a key with an optional TTL (seconds).
    pub async fn kv_set(
        &self,
        key: &str,
        value: serde_json::Value,
        ttl: Option<u64>,
    ) -> Result<(), ClientError> {
        self.pool.get().await?.kv_set(key, value, ttl).await
    }

    /// Delete a key.
    pub async fn kv_delete(&self, key: &str) -> Result<(), ClientError> {
        self.pool.get().await?.kv_delete(key).await
    }

    /// List all key entries.
    pub async fn kv_list(&self) -> Result<serde_json::Value, ClientError> {
        self.pool.get().await?.kv_list().await
    }

    /// Get many keys at once.
    pub async fn kv_batch_get(&self, keys: &[&str]) -> Result<serde_json::Value, ClientError> {
        self.pool.get().await?.kv_batch_get(keys).await
    }

    /// Set many keys at once. Each entry is `(key, value, ttl_seconds)`.
    pub async fn kv_batch_set(
        &self,
        entries: Vec<(String, serde_json::Value, Option<u64>)>,
    ) -> Result<serde_json::Value, ClientError> {
        self.pool.get().await?.kv_batch_set(entries).await
    }

    /// Delete many keys at once.
    pub async fn kv_batch_delete(&self, keys: &[&str]) -> Result<serde_json::Value, ClientError> {
        self.pool.get().await?.kv_batch_delete(keys).await
    }

    // ---- Documents ----------------------------------------------------------

    /// List document collections.
    pub async fn list_collections(&self) -> Result<serde_json::Value, ClientError> {
        self.pool.get().await?.list_collections().await
    }

    /// Create a document collection.
    pub async fn create_collection(&self, name: &str) -> Result<serde_json::Value, ClientError> {
        self.pool.get().await?.create_collection(name).await
    }

    /// Insert a document, optionally with an explicit id.
    pub async fn insert_document(
        &self,
        collection: &str,
        document: serde_json::Value,
        id: Option<&str>,
    ) -> Result<serde_json::Value, ClientError> {
        self.pool
            .get()
            .await?
            .insert_document(collection, document, id)
            .await
    }

    /// Get a document by id, or `None` if absent.
    pub async fn get_document(
        &self,
        collection: &str,
        id: &str,
    ) -> Result<Option<serde_json::Value>, ClientError> {
        self.pool.get().await?.get_document(collection, id).await
    }

    /// Replace a document (full update).
    pub async fn update_document(
        &self,
        collection: &str,
        id: &str,
        document: serde_json::Value,
    ) -> Result<serde_json::Value, ClientError> {
        self.pool
            .get()
            .await?
            .update_document(collection, id, document)
            .await
    }

    /// Partially update (merge) a document.
    pub async fn patch_document(
        &self,
        collection: &str,
        id: &str,
        partial: serde_json::Value,
    ) -> Result<serde_json::Value, ClientError> {
        self.pool
            .get()
            .await?
            .patch_document(collection, id, partial)
            .await
    }

    /// Delete a document.
    pub async fn delete_document(&self, collection: &str, id: &str) -> Result<(), ClientError> {
        self.pool.get().await?.delete_document(collection, id).await
    }

    /// Query documents with a MongoDB-style filter.
    pub async fn query_documents(
        &self,
        collection: &str,
        filter: serde_json::Value,
        limit: Option<usize>,
        skip: Option<usize>,
    ) -> Result<serde_json::Value, ClientError> {
        self.pool
            .get()
            .await?
            .query_documents(collection, filter, limit, skip)
            .await
    }

    /// Insert many documents into a collection in one call.
    pub async fn bulk_insert_documents(
        &self,
        collection: &str,
        documents: Vec<serde_json::Value>,
    ) -> Result<serde_json::Value, ClientError> {
        self.pool
            .get()
            .await?
            .bulk_insert_documents(collection, documents)
            .await
    }

    /// Delete many documents by id in one call.
    pub async fn bulk_delete_documents(
        &self,
        collection: &str,
        ids: &[&str],
    ) -> Result<serde_json::Value, ClientError> {
        self.pool
            .get()
            .await?
            .bulk_delete_documents(collection, ids)
            .await
    }

    // ---- Time series --------------------------------------------------------

    /// Register a metric.
    pub async fn register_metric(
        &self,
        name: &str,
        metric_type: &str,
    ) -> Result<serde_json::Value, ClientError> {
        self.pool
            .get()
            .await?
            .register_metric(name, metric_type)
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
        self.pool
            .get()
            .await?
            .ts_write(metric, value, timestamp, tags)
            .await
    }

    /// Query a time series within an optional `[start, end]` window.
    pub async fn ts_query(
        &self,
        metric: &str,
        start: Option<i64>,
        end: Option<i64>,
        limit: Option<usize>,
    ) -> Result<serde_json::Value, ClientError> {
        self.pool
            .get()
            .await?
            .ts_query(metric, start, end, limit)
            .await
    }

    // ---- Graph --------------------------------------------------------------

    /// Get the full graph (nodes + edges).
    pub async fn graph_data(&self) -> Result<serde_json::Value, ClientError> {
        self.pool.get().await?.graph_data().await
    }

    /// Create a graph node.
    pub async fn create_node(
        &self,
        label: &str,
        properties: serde_json::Value,
    ) -> Result<serde_json::Value, ClientError> {
        self.pool.get().await?.create_node(label, properties).await
    }

    /// Update a graph node.
    pub async fn update_node(
        &self,
        node_id: &str,
        label: Option<&str>,
        properties: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, ClientError> {
        self.pool
            .get()
            .await?
            .update_node(node_id, label, properties)
            .await
    }

    /// Delete a graph node.
    pub async fn delete_node(&self, node_id: &str) -> Result<(), ClientError> {
        self.pool.get().await?.delete_node(node_id).await
    }

    /// Create a graph edge.
    pub async fn create_edge(
        &self,
        source: &str,
        target: &str,
        relationship: &str,
    ) -> Result<serde_json::Value, ClientError> {
        self.pool
            .get()
            .await?
            .create_edge(source, target, relationship)
            .await
    }

    /// Update a graph edge's relationship.
    pub async fn update_edge(
        &self,
        edge_id: &str,
        relationship: &str,
    ) -> Result<serde_json::Value, ClientError> {
        self.pool
            .get()
            .await?
            .update_edge(edge_id, relationship)
            .await
    }

    /// Delete a graph edge.
    pub async fn delete_edge(&self, edge_id: &str) -> Result<(), ClientError> {
        self.pool.get().await?.delete_edge(edge_id).await
    }

    // ---- Schema / health / metrics ------------------------------------------

    /// Server health.
    pub async fn health(&self) -> Result<serde_json::Value, ClientError> {
        self.pool.get().await?.health().await
    }

    /// List tables in the current database.
    pub async fn list_tables(&self) -> Result<serde_json::Value, ClientError> {
        self.pool.get().await?.list_tables().await
    }

    /// Get a table's schema and row count.
    pub async fn get_table(&self, name: &str) -> Result<serde_json::Value, ClientError> {
        self.pool.get().await?.get_table(name).await
    }

    /// Current server metrics snapshot.
    pub async fn metrics(&self) -> Result<serde_json::Value, ClientError> {
        self.pool.get().await?.metrics().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_config_from_url() {
        let config = ClientConfig::from_url("aegis://localhost:9090/testdb")
            .expect("Should parse valid URL");
        assert_eq!(config.connection.host, "localhost");
        assert_eq!(config.connection.port, 9090);
        assert_eq!(config.connection.database, "testdb");
    }
}
