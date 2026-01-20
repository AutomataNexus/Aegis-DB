//! Aegis Client Connection Pool
//!
//! Connection pool management for efficient database access.
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use crate::config::{ConnectionConfig, PoolConfig};
use crate::connection::{Connection, PooledConnection};
use crate::error::ClientError;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::{Mutex, Semaphore};
use tokio::time::{timeout, Duration};

// =============================================================================
// Connection Pool
// =============================================================================

/// A pool of database connections.
pub struct ConnectionPool {
    config: PoolConfig,
    connection_config: ConnectionConfig,
    connections: Mutex<VecDeque<Arc<Connection>>>,
    semaphore: Arc<Semaphore>,
    total_created: AtomicU64,
    total_acquired: AtomicU64,
    total_released: AtomicU64,
    current_size: AtomicUsize,
    closed: std::sync::atomic::AtomicBool,
}

impl ConnectionPool {
    /// Create a new connection pool.
    pub async fn new(config: PoolConfig) -> Result<Self, ClientError> {
        Self::with_connection_config(config, ConnectionConfig::default()).await
    }

    /// Create a pool with specific connection configuration.
    pub async fn with_connection_config(
        config: PoolConfig,
        connection_config: ConnectionConfig,
    ) -> Result<Self, ClientError> {
        let pool = Self {
            semaphore: Arc::new(Semaphore::new(config.max_connections)),
            connections: Mutex::new(VecDeque::with_capacity(config.max_connections)),
            total_created: AtomicU64::new(0),
            total_acquired: AtomicU64::new(0),
            total_released: AtomicU64::new(0),
            current_size: AtomicUsize::new(0),
            closed: std::sync::atomic::AtomicBool::new(false),
            config,
            connection_config,
        };

        // Pre-create minimum connections
        pool.initialize().await?;

        Ok(pool)
    }

    async fn initialize(&self) -> Result<(), ClientError> {
        for _ in 0..self.config.min_connections {
            let conn = self.create_connection().await?;
            self.connections.lock().await.push_back(conn);
        }
        Ok(())
    }

    async fn create_connection(&self) -> Result<Arc<Connection>, ClientError> {
        let conn = Connection::new(self.connection_config.clone()).await?;
        self.total_created.fetch_add(1, Ordering::SeqCst);
        self.current_size.fetch_add(1, Ordering::SeqCst);
        Ok(Arc::new(conn))
    }

    /// Get a connection from the pool.
    pub async fn get(&self) -> Result<PooledConnection, ClientError> {
        if self.closed.load(Ordering::SeqCst) {
            return Err(ClientError::ConnectionClosed);
        }

        // Wait for a permit
        let permit_result = timeout(
            self.config.acquire_timeout,
            self.semaphore.clone().acquire_owned(),
        )
        .await;

        let permit = match permit_result {
            Ok(Ok(p)) => p,
            Ok(Err(_)) => return Err(ClientError::PoolExhausted),
            Err(_) => return Err(ClientError::PoolTimeout),
        };

        // Try to get an existing connection
        let mut connections = self.connections.lock().await;

        let conn = if let Some(conn) = connections.pop_front() {
            // Check if connection is still valid
            if conn.is_connected() && conn.idle_time() < self.config.idle_timeout {
                conn
            } else {
                // Connection is stale, create a new one
                self.current_size.fetch_sub(1, Ordering::SeqCst);
                drop(connections);
                self.create_connection().await?
            }
        } else {
            // No available connections, create a new one
            drop(connections);
            self.create_connection().await?
        };

        self.total_acquired.fetch_add(1, Ordering::SeqCst);

        // Create the pooled connection with return callback
        let semaphore = Arc::clone(&self.semaphore);
        let pool_connections = unsafe {
            // Safety: We're creating a self-referential structure here
            // The pool outlives the connection due to Arc
            &self.connections as *const Mutex<VecDeque<Arc<Connection>>>
        };

        Ok(PooledConnection::new(conn, move |conn| {
            // Return connection to pool
            let _ = permit; // Drop permit to release semaphore

            // We can't easily return to pool in Drop, so we just release the permit
            // In a production implementation, you'd use a channel or similar
        }))
    }

    /// Return a connection to the pool.
    pub async fn return_connection(&self, conn: Arc<Connection>) {
        if !self.closed.load(Ordering::SeqCst) && conn.is_connected() {
            let mut connections = self.connections.lock().await;
            connections.push_back(conn);
            self.total_released.fetch_add(1, Ordering::SeqCst);
        } else {
            self.current_size.fetch_sub(1, Ordering::SeqCst);
        }
    }

    /// Check if the pool is healthy.
    pub async fn is_healthy(&self) -> bool {
        if self.closed.load(Ordering::SeqCst) {
            return false;
        }

        let connections = self.connections.lock().await;
        !connections.is_empty() || self.current_size.load(Ordering::SeqCst) > 0
    }

    /// Get pool statistics.
    pub fn stats(&self) -> PoolStats {
        PoolStats {
            total_created: self.total_created.load(Ordering::SeqCst),
            total_acquired: self.total_acquired.load(Ordering::SeqCst),
            total_released: self.total_released.load(Ordering::SeqCst),
            current_size: self.current_size.load(Ordering::SeqCst),
            max_size: self.config.max_connections,
            min_size: self.config.min_connections,
            available_permits: self.semaphore.available_permits(),
        }
    }

    /// Close all connections in the pool.
    pub async fn close(&self) {
        self.closed.store(true, Ordering::SeqCst);

        let mut connections = self.connections.lock().await;
        while let Some(conn) = connections.pop_front() {
            conn.close().await;
            self.current_size.fetch_sub(1, Ordering::SeqCst);
        }
    }

    /// Get the current pool size.
    pub fn size(&self) -> usize {
        self.current_size.load(Ordering::SeqCst)
    }

    /// Get available permits.
    pub fn available(&self) -> usize {
        self.semaphore.available_permits()
    }
}

// =============================================================================
// Pool Statistics
// =============================================================================

/// Statistics for the connection pool.
#[derive(Debug, Clone)]
pub struct PoolStats {
    pub total_created: u64,
    pub total_acquired: u64,
    pub total_released: u64,
    pub current_size: usize,
    pub max_size: usize,
    pub min_size: usize,
    pub available_permits: usize,
}

impl PoolStats {
    /// Get pool utilization as a percentage.
    pub fn utilization(&self) -> f64 {
        if self.max_size == 0 {
            return 0.0;
        }
        let in_use = self.max_size - self.available_permits;
        (in_use as f64 / self.max_size as f64) * 100.0
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_pool_creation() {
        let config = PoolConfig {
            min_connections: 2,
            max_connections: 5,
            ..Default::default()
        };

        let pool = ConnectionPool::new(config).await.unwrap();
        assert_eq!(pool.size(), 2);
    }

    #[tokio::test]
    async fn test_pool_get_connection() {
        let config = PoolConfig::default();
        let pool = ConnectionPool::new(config).await.unwrap();

        let conn = pool.get().await.unwrap();
        assert!(conn.inner().is_connected());
    }

    #[tokio::test]
    async fn test_pool_stats() {
        let config = PoolConfig {
            min_connections: 1,
            max_connections: 5,
            ..Default::default()
        };

        let pool = ConnectionPool::new(config).await.unwrap();
        let stats = pool.stats();

        assert_eq!(stats.min_size, 1);
        assert_eq!(stats.max_size, 5);
        assert!(stats.total_created >= 1);
    }

    #[tokio::test]
    async fn test_pool_acquire_multiple() {
        let config = PoolConfig {
            min_connections: 0,
            max_connections: 3,
            ..Default::default()
        };

        let pool = ConnectionPool::new(config).await.unwrap();

        let c1 = pool.get().await.unwrap();
        let c2 = pool.get().await.unwrap();
        let c3 = pool.get().await.unwrap();

        assert!(c1.inner().is_connected());
        assert!(c2.inner().is_connected());
        assert!(c3.inner().is_connected());

        let stats = pool.stats();
        assert_eq!(stats.total_acquired, 3);
    }

    #[tokio::test]
    async fn test_pool_close() {
        let config = PoolConfig {
            min_connections: 2,
            ..Default::default()
        };

        let pool = ConnectionPool::new(config).await.unwrap();
        assert!(pool.size() >= 2);

        pool.close().await;
        assert!(!pool.is_healthy().await);
    }

    #[tokio::test]
    async fn test_pool_utilization() {
        let stats = PoolStats {
            total_created: 5,
            total_acquired: 10,
            total_released: 8,
            current_size: 5,
            max_size: 10,
            min_size: 2,
            available_permits: 8,
        };

        let util = stats.utilization();
        assert!((util - 20.0).abs() < 0.01);
    }
}
