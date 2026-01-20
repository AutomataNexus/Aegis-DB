//! Aegis Client Transaction Management
//!
//! Transaction handling for database operations.
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use crate::connection::PooledConnection;
use crate::error::ClientError;
use crate::result::{QueryResult, Value};
use std::sync::atomic::{AtomicBool, Ordering};

// =============================================================================
// Transaction
// =============================================================================

/// A database transaction.
pub struct Transaction {
    connection: PooledConnection,
    committed: AtomicBool,
    rolled_back: AtomicBool,
}

impl Transaction {
    /// Begin a new transaction.
    pub async fn begin(connection: PooledConnection) -> Result<Self, ClientError> {
        connection.execute("BEGIN").await?;

        Ok(Self {
            connection,
            committed: AtomicBool::new(false),
            rolled_back: AtomicBool::new(false),
        })
    }

    /// Check if the transaction is active.
    pub fn is_active(&self) -> bool {
        !self.committed.load(Ordering::SeqCst) && !self.rolled_back.load(Ordering::SeqCst)
    }

    /// Execute a query within the transaction.
    pub async fn query(&self, sql: &str) -> Result<QueryResult, ClientError> {
        self.check_active()?;
        self.connection.query(sql).await
    }

    /// Execute a query with parameters.
    pub async fn query_with_params(
        &self,
        sql: &str,
        params: Vec<Value>,
    ) -> Result<QueryResult, ClientError> {
        self.check_active()?;
        self.connection.query_with_params(sql, params).await
    }

    /// Execute a statement within the transaction.
    pub async fn execute(&self, sql: &str) -> Result<u64, ClientError> {
        self.check_active()?;
        self.connection.execute(sql).await
    }

    /// Execute a statement with parameters.
    pub async fn execute_with_params(
        &self,
        sql: &str,
        params: Vec<Value>,
    ) -> Result<u64, ClientError> {
        self.check_active()?;
        self.connection.execute_with_params(sql, params).await
    }

    /// Commit the transaction.
    pub async fn commit(self) -> Result<(), ClientError> {
        self.check_active()?;
        self.connection.execute("COMMIT").await?;
        self.committed.store(true, Ordering::SeqCst);
        Ok(())
    }

    /// Rollback the transaction.
    pub async fn rollback(self) -> Result<(), ClientError> {
        self.check_active()?;
        self.connection.execute("ROLLBACK").await?;
        self.rolled_back.store(true, Ordering::SeqCst);
        Ok(())
    }

    /// Create a savepoint.
    pub async fn savepoint(&self, name: &str) -> Result<Savepoint<'_>, ClientError> {
        self.check_active()?;
        self.connection
            .execute(&format!("SAVEPOINT {}", name))
            .await?;
        Ok(Savepoint {
            transaction: self,
            name: name.to_string(),
            released: AtomicBool::new(false),
        })
    }

    fn check_active(&self) -> Result<(), ClientError> {
        if !self.is_active() {
            return Err(ClientError::NoTransaction);
        }
        Ok(())
    }
}

impl Drop for Transaction {
    fn drop(&mut self) {
        // If the transaction is still active when dropped, it should be rolled back
        // In async context, we can't do async operations in Drop
        // A production implementation would use a background task or similar
        if self.is_active() {
            self.rolled_back.store(true, Ordering::SeqCst);
        }
    }
}

// =============================================================================
// Savepoint
// =============================================================================

/// A savepoint within a transaction.
pub struct Savepoint<'a> {
    transaction: &'a Transaction,
    name: String,
    released: AtomicBool,
}

impl<'a> Savepoint<'a> {
    /// Release the savepoint (commit changes since savepoint).
    pub async fn release(self) -> Result<(), ClientError> {
        if self.released.load(Ordering::SeqCst) {
            return Err(ClientError::NoTransaction);
        }
        self.transaction
            .connection
            .execute(&format!("RELEASE SAVEPOINT {}", self.name))
            .await?;
        self.released.store(true, Ordering::SeqCst);
        Ok(())
    }

    /// Rollback to the savepoint.
    pub async fn rollback(self) -> Result<(), ClientError> {
        if self.released.load(Ordering::SeqCst) {
            return Err(ClientError::NoTransaction);
        }
        self.transaction
            .connection
            .execute(&format!("ROLLBACK TO SAVEPOINT {}", self.name))
            .await?;
        self.released.store(true, Ordering::SeqCst);
        Ok(())
    }

    /// Get the savepoint name.
    pub fn name(&self) -> &str {
        &self.name
    }
}

// =============================================================================
// Transaction Options
// =============================================================================

/// Options for transaction behavior.
#[derive(Debug, Clone, Default)]
pub struct TransactionOptions {
    pub isolation_level: IsolationLevel,
    pub read_only: bool,
    pub deferrable: bool,
}

impl TransactionOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_isolation(mut self, level: IsolationLevel) -> Self {
        self.isolation_level = level;
        self
    }

    pub fn read_only(mut self) -> Self {
        self.read_only = true;
        self
    }

    pub fn deferrable(mut self) -> Self {
        self.deferrable = true;
        self
    }

    /// Generate the BEGIN statement for these options.
    pub fn begin_statement(&self) -> String {
        let mut parts = vec!["BEGIN".to_string()];

        match self.isolation_level {
            IsolationLevel::ReadCommitted => {
                parts.push("ISOLATION LEVEL READ COMMITTED".to_string());
            }
            IsolationLevel::RepeatableRead => {
                parts.push("ISOLATION LEVEL REPEATABLE READ".to_string());
            }
            IsolationLevel::Serializable => {
                parts.push("ISOLATION LEVEL SERIALIZABLE".to_string());
            }
            IsolationLevel::ReadUncommitted => {
                parts.push("ISOLATION LEVEL READ UNCOMMITTED".to_string());
            }
        }

        if self.read_only {
            parts.push("READ ONLY".to_string());
        }

        if self.deferrable {
            parts.push("DEFERRABLE".to_string());
        }

        parts.join(" ")
    }
}

/// Transaction isolation levels.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum IsolationLevel {
    ReadUncommitted,
    #[default]
    ReadCommitted,
    RepeatableRead,
    Serializable,
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ConnectionConfig, PoolConfig};
    use crate::pool::ConnectionPool;

    async fn create_transaction() -> Transaction {
        let config = PoolConfig::default();
        let pool = ConnectionPool::with_connection_config(config, ConnectionConfig::default())
            .await
            .unwrap();
        let conn = pool.get().await.unwrap();
        Transaction::begin(conn).await.unwrap()
    }

    #[tokio::test]
    async fn test_transaction_begin() {
        let tx = create_transaction().await;
        assert!(tx.is_active());
    }

    #[tokio::test]
    async fn test_transaction_commit() {
        let tx = create_transaction().await;
        tx.commit().await.unwrap();
    }

    #[tokio::test]
    async fn test_transaction_rollback() {
        let tx = create_transaction().await;
        tx.rollback().await.unwrap();
    }

    #[tokio::test]
    async fn test_transaction_execute() {
        let tx = create_transaction().await;
        let affected = tx.execute("INSERT INTO test VALUES (1)").await.unwrap();
        assert_eq!(affected, 1);
        tx.commit().await.unwrap();
    }

    #[test]
    fn test_transaction_options() {
        let opts = TransactionOptions::new()
            .with_isolation(IsolationLevel::Serializable)
            .read_only();

        let stmt = opts.begin_statement();
        assert!(stmt.contains("SERIALIZABLE"));
        assert!(stmt.contains("READ ONLY"));
    }

    #[test]
    fn test_isolation_levels() {
        let opts = TransactionOptions::new()
            .with_isolation(IsolationLevel::RepeatableRead);

        assert!(opts.begin_statement().contains("REPEATABLE READ"));
    }
}
