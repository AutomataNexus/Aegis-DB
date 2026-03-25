//! Aegis Client Connection Pool
//!
//! Connection pool management for efficient database access.
//! Uses a channel-based approach for real connection recycling.
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use crate::config::{ConnectionConfig, PoolConfig};
use crate::connection::{Connection, PooledConnection};
use crate::error::ClientError;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::Semaphore;
use tokio::time::timeout;

// =============================================================================
// Connection Pool
// =============================================================================

/// A pool of database connections.
///
/// Connections are recycled via an unbounded mpsc channel. When a `PooledConnection`
/// is dropped, its inner `Arc<Connection>` is sent back through the channel so
/// subsequent `get()` calls can reuse it instead of opening a new connection.
pub struct ConnectionPool {
    config: PoolConfig,
    connection_config: ConnectionConfig,
    /// Sender half – cloned into every `PooledConnection` for return-on-drop.
    return_tx: mpsc::UnboundedSender<Arc<Connection>>,
    /// Receiver half – drained on each `get()` to reclaim returned connections.
    return_rx: tokio::sync::Mutex<mpsc::UnboundedReceiver<Arc<Connection>>>,
    /// Limits the total number of connections that may be checked out at once.
    semaphore: Arc<Semaphore>,
    total_created: AtomicU64,
    total_acquired: AtomicU64,
    total_released: Arc<AtomicU64>,
    current_size: Arc<AtomicUsize>,
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
        let (return_tx, return_rx) = mpsc::unbounded_channel();

        let pool = Self {
            semaphore: Arc::new(Semaphore::new(config.max_connections)),
            return_tx,
            return_rx: tokio::sync::Mutex::new(return_rx),
            total_created: AtomicU64::new(0),
            total_acquired: AtomicU64::new(0),
            total_released: Arc::new(AtomicU64::new(0)),
            current_size: Arc::new(AtomicUsize::new(0)),
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
            // Seed the channel with pre-created connections
            let _ = self.return_tx.send(conn);
        }
        Ok(())
    }

    async fn create_connection(&self) -> Result<Arc<Connection>, ClientError> {
        let conn = Connection::new(self.connection_config.clone()).await?;
        self.total_created.fetch_add(1, Ordering::SeqCst);
        self.current_size.fetch_add(1, Ordering::SeqCst);
        Ok(Arc::new(conn))
    }

    /// Try to pull one usable connection from the return channel.
    ///
    /// Drains stale/disconnected connections and returns the first good one,
    /// or `None` if the channel is empty.
    async fn try_recv_usable(&self) -> Option<Arc<Connection>> {
        let mut rx = self.return_rx.lock().await;
        loop {
            match rx.try_recv() {
                Ok(conn) => {
                    if conn.is_connected() && conn.idle_time() < self.config.idle_timeout {
                        return Some(conn);
                    }
                    // Stale or disconnected – discard it
                    self.current_size.fetch_sub(1, Ordering::SeqCst);
                }
                Err(_) => return None,
            }
        }
    }

    /// Get a connection from the pool.
    pub async fn get(&self) -> Result<PooledConnection, ClientError> {
        if self.closed.load(Ordering::SeqCst) {
            return Err(ClientError::ConnectionClosed);
        }

        // Wait for a permit (limits total concurrent checkouts)
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

        // Try to reuse a recycled connection from the channel
        let conn = if let Some(conn) = self.try_recv_usable().await {
            conn
        } else {
            // No idle connections available – create a fresh one
            self.create_connection().await?
        };

        self.total_acquired.fetch_add(1, Ordering::SeqCst);

        // Clone the sender so the PooledConnection can return itself on drop
        let tx = self.return_tx.clone();
        let released = Arc::clone(&self.total_released);
        let current_size = Arc::clone(&self.current_size);
        let closed = self.closed.load(Ordering::SeqCst);

        Ok(PooledConnection::new(conn, move |conn| {
            // Release the semaphore permit so another caller can proceed
            drop(permit);

            // If pool is still open and connection is alive, recycle it
            if !closed && conn.is_connected() {
                match tx.send(conn) {
                    Ok(_) => {
                        released.fetch_add(1, Ordering::SeqCst);
                    }
                    Err(_) => {
                        // Channel closed (pool was dropped) – just let the connection drop
                        current_size.fetch_sub(1, Ordering::SeqCst);
                    }
                }
            } else {
                // Connection is dead or pool is closed – discard
                current_size.fetch_sub(1, Ordering::SeqCst);
            }
        }))
    }

    /// Return a connection to the pool (explicit async path).
    pub async fn return_connection(&self, conn: Arc<Connection>) {
        if !self.closed.load(Ordering::SeqCst) && conn.is_connected() {
            let _ = self.return_tx.send(conn);
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

        // The pool is healthy if there are connections in existence
        self.current_size.load(Ordering::SeqCst) > 0
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

        // Drain all returned connections from the channel and close them
        let mut rx = self.return_rx.lock().await;
        while let Ok(conn) = rx.try_recv() {
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

    /// Get test connection config - uses AEGIS_TEST_PORT env var or defaults to 9090
    fn test_connection_config() -> ConnectionConfig {
        let port = std::env::var("AEGIS_TEST_PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(9090);
        ConnectionConfig {
            host: "127.0.0.1".to_string(),
            port,
            ..Default::default()
        }
    }

    /// Helper to create pool with test config
    async fn create_test_pool(pool_config: PoolConfig) -> Result<ConnectionPool, ClientError> {
        ConnectionPool::with_connection_config(pool_config, test_connection_config()).await
    }

    #[tokio::test]
    async fn test_pool_creation() {
        let config = PoolConfig {
            min_connections: 2,
            max_connections: 5,
            ..Default::default()
        };

        match create_test_pool(config).await {
            Ok(pool) => assert_eq!(pool.size(), 2),
            Err(e) => eprintln!("Skipping test, server not available: {}", e),
        }
    }

    #[tokio::test]
    async fn test_pool_get_connection() {
        let config = PoolConfig::default();

        match create_test_pool(config).await {
            Ok(pool) => {
                let conn = pool.get().await.expect("Should get connection from pool");
                assert!(conn.inner().is_connected());
            }
            Err(e) => eprintln!("Skipping test, server not available: {}", e),
        }
    }

    #[tokio::test]
    async fn test_pool_stats() {
        let config = PoolConfig {
            min_connections: 1,
            max_connections: 5,
            ..Default::default()
        };

        match create_test_pool(config).await {
            Ok(pool) => {
                let stats = pool.stats();
                assert_eq!(stats.min_size, 1);
                assert_eq!(stats.max_size, 5);
                assert!(stats.total_created >= 1);
            }
            Err(e) => eprintln!("Skipping test, server not available: {}", e),
        }
    }

    #[tokio::test]
    async fn test_pool_acquire_multiple() {
        let config = PoolConfig {
            min_connections: 0,
            max_connections: 3,
            ..Default::default()
        };

        match create_test_pool(config).await {
            Ok(pool) => {
                // Try to acquire connections - may fail if server isn't running
                let c1 = match pool.get().await {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("Skipping test, server not available: {}", e);
                        return;
                    }
                };
                let c2 = pool
                    .get()
                    .await
                    .expect("Should get second connection from pool");
                let c3 = pool
                    .get()
                    .await
                    .expect("Should get third connection from pool");

                assert!(c1.inner().is_connected());
                assert!(c2.inner().is_connected());
                assert!(c3.inner().is_connected());

                let stats = pool.stats();
                assert_eq!(stats.total_acquired, 3);
            }
            Err(e) => eprintln!("Skipping test, server not available: {}", e),
        }
    }

    #[tokio::test]
    async fn test_pool_close() {
        let config = PoolConfig {
            min_connections: 2,
            ..Default::default()
        };

        match create_test_pool(config).await {
            Ok(pool) => {
                assert!(pool.size() >= 2);
                pool.close().await;
                assert!(!pool.is_healthy().await);
            }
            Err(e) => eprintln!("Skipping test, server not available: {}", e),
        }
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
