//! Aegis Client Connection
//!
//! Database connection handling.
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use crate::config::ConnectionConfig;
use crate::error::ClientError;
use crate::result::{Column, DataType, QueryResult, Row, Value};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

// =============================================================================
// Connection
// =============================================================================

/// A database connection.
pub struct Connection {
    id: u64,
    #[allow(dead_code)]
    config: ConnectionConfig,
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

        let conn = Self {
            id: CONN_ID.fetch_add(1, Ordering::SeqCst),
            config,
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

    /// Connect to the database.
    async fn connect(&self) -> Result<(), ClientError> {
        // Simulate connection establishment
        // In a real implementation, this would establish a TCP connection
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

    /// Execute a query.
    pub async fn query(&self, sql: &str) -> Result<QueryResult, ClientError> {
        self.query_with_params(sql, vec![]).await
    }

    /// Execute a query with parameters.
    pub async fn query_with_params(
        &self,
        sql: &str,
        _params: Vec<Value>,
    ) -> Result<QueryResult, ClientError> {
        if !self.is_connected() {
            return Err(ClientError::NotConnected);
        }

        self.mark_used();
        self.queries_executed.fetch_add(1, Ordering::SeqCst);

        // Parse SQL to determine query type and generate mock result
        let sql_upper = sql.trim().to_uppercase();

        if sql_upper.starts_with("SELECT") {
            // Return mock SELECT result
            Ok(self.mock_select_result(sql))
        } else {
            Err(ClientError::QueryFailed(
                "Use execute() for non-SELECT queries".to_string(),
            ))
        }
    }

    /// Execute a statement (INSERT, UPDATE, DELETE).
    pub async fn execute(&self, sql: &str) -> Result<u64, ClientError> {
        self.execute_with_params(sql, vec![]).await
    }

    /// Execute a statement with parameters.
    pub async fn execute_with_params(
        &self,
        sql: &str,
        _params: Vec<Value>,
    ) -> Result<u64, ClientError> {
        if !self.is_connected() {
            return Err(ClientError::NotConnected);
        }

        self.mark_used();
        self.queries_executed.fetch_add(1, Ordering::SeqCst);

        let sql_upper = sql.trim().to_uppercase();

        if sql_upper.starts_with("INSERT")
            || sql_upper.starts_with("UPDATE")
            || sql_upper.starts_with("DELETE")
        {
            // Return mock affected rows
            Ok(1)
        } else if sql_upper.starts_with("BEGIN") {
            self.in_transaction.store(true, Ordering::SeqCst);
            Ok(0)
        } else if sql_upper.starts_with("COMMIT") || sql_upper.starts_with("ROLLBACK") {
            self.in_transaction.store(false, Ordering::SeqCst);
            Ok(0)
        } else {
            Ok(0)
        }
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
        if !self.is_connected() {
            return Err(ClientError::NotConnected);
        }
        self.mark_used();
        Ok(())
    }

    /// Close the connection.
    pub async fn close(&self) {
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

    fn mock_select_result(&self, _sql: &str) -> QueryResult {
        // Generate a simple mock result
        let columns = vec![
            Column::new("id", DataType::Integer),
            Column::new("name", DataType::Text),
        ];

        let rows = vec![
            Row::new(
                vec!["id".to_string(), "name".to_string()],
                vec![Value::Int(1), Value::String("test".to_string())],
            ),
        ];

        QueryResult::new(columns, rows)
    }
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
pub struct PooledConnection {
    connection: Arc<Connection>,
    pool_return: Option<Box<dyn FnOnce(Arc<Connection>) + Send>>,
}

impl PooledConnection {
    /// Create a new pooled connection.
    pub fn new<F>(connection: Arc<Connection>, on_return: F) -> Self
    where
        F: FnOnce(Arc<Connection>) + Send + 'static,
    {
        Self {
            connection,
            pool_return: Some(Box::new(on_return)),
        }
    }

    /// Execute a query.
    pub async fn query(&self, sql: &str) -> Result<QueryResult, ClientError> {
        self.connection.query(sql).await
    }

    /// Execute a query with parameters.
    pub async fn query_with_params(
        &self,
        sql: &str,
        params: Vec<Value>,
    ) -> Result<QueryResult, ClientError> {
        self.connection.query_with_params(sql, params).await
    }

    /// Execute a statement.
    pub async fn execute(&self, sql: &str) -> Result<u64, ClientError> {
        self.connection.execute(sql).await
    }

    /// Execute a statement with parameters.
    pub async fn execute_with_params(
        &self,
        sql: &str,
        params: Vec<Value>,
    ) -> Result<u64, ClientError> {
        self.connection.execute_with_params(sql, params).await
    }

    /// Get the underlying connection.
    pub fn inner(&self) -> &Connection {
        &self.connection
    }
}

impl Drop for PooledConnection {
    fn drop(&mut self) {
        if let Some(return_fn) = self.pool_return.take() {
            return_fn(Arc::clone(&self.connection));
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_connection_create() {
        let config = ConnectionConfig::default();
        let conn = Connection::new(config).await.unwrap();

        assert!(conn.is_connected());
        assert!(!conn.in_transaction());
    }

    #[tokio::test]
    async fn test_connection_query() {
        let config = ConnectionConfig::default();
        let conn = Connection::new(config).await.unwrap();

        let result = conn.query("SELECT * FROM test").await.unwrap();
        assert!(!result.is_empty());
    }

    #[tokio::test]
    async fn test_connection_execute() {
        let config = ConnectionConfig::default();
        let conn = Connection::new(config).await.unwrap();

        let affected = conn.execute("INSERT INTO test VALUES (1)").await.unwrap();
        assert_eq!(affected, 1);
    }

    #[tokio::test]
    async fn test_connection_transaction() {
        let config = ConnectionConfig::default();
        let conn = Connection::new(config).await.unwrap();

        conn.begin_transaction().await.unwrap();
        assert!(conn.in_transaction());

        conn.commit().await.unwrap();
        assert!(!conn.in_transaction());
    }

    #[tokio::test]
    async fn test_connection_rollback() {
        let config = ConnectionConfig::default();
        let conn = Connection::new(config).await.unwrap();

        conn.begin_transaction().await.unwrap();
        conn.rollback().await.unwrap();
        assert!(!conn.in_transaction());
    }

    #[tokio::test]
    async fn test_connection_stats() {
        let config = ConnectionConfig::default();
        let conn = Connection::new(config).await.unwrap();

        conn.query("SELECT 1").await.unwrap();
        conn.query("SELECT 2").await.unwrap();

        let stats = conn.stats();
        assert_eq!(stats.queries_executed, 2);
        assert!(stats.connected);
    }

    #[tokio::test]
    async fn test_not_connected_error() {
        let config = ConnectionConfig::default();
        let conn = Connection::new(config).await.unwrap();

        conn.close().await;
        let result = conn.query("SELECT 1").await;
        assert!(matches!(result, Err(ClientError::NotConnected)));
    }
}
