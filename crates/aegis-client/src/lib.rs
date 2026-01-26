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
pub mod pool;
pub mod query;
pub mod result;
pub mod transaction;
pub mod error;

pub use config::{ClientConfig, ConnectionConfig};
pub use connection::Connection;
pub use pool::ConnectionPool;
pub use query::{Query, QueryBuilder};
pub use result::{QueryResult, Row, Value};
pub use transaction::Transaction;
pub use error::ClientError;

/// The main client for interacting with Aegis databases.
pub struct AegisClient {
    #[allow(dead_code)]
    config: ClientConfig,
    pool: ConnectionPool,
}

impl AegisClient {
    /// Create a new client with the given configuration.
    pub async fn new(config: ClientConfig) -> Result<Self, ClientError> {
        let pool = ConnectionPool::with_connection_config(
            config.pool.clone(),
            config.connection.clone(),
        ).await?;
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_config_from_url() {
        let config = ClientConfig::from_url("aegis://localhost:9090/testdb").expect("Should parse valid URL");
        assert_eq!(config.connection.host, "localhost");
        assert_eq!(config.connection.port, 9090);
        assert_eq!(config.connection.database, "testdb");
    }
}
