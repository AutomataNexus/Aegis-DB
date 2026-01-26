//! Aegis Error - Unified Error Types
//!
//! Comprehensive error handling for all Aegis operations. Categorizes errors
//! by domain (storage, transaction, query, replication, auth) and provides
//! utilities for determining retryability and error classification.
//!
//! Key Features:
//! - Domain-specific error variants for precise error handling
//! - Retryable error detection for automatic retry logic
//! - User vs system error classification
//! - Seamless integration with std::io::Error
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use thiserror::Error;

// =============================================================================
// Error Types
// =============================================================================

/// Unified error type for all Aegis operations.
#[derive(Error, Debug)]
pub enum AegisError {
    // Storage errors
    #[error("storage error: {0}")]
    Storage(String),

    #[error("block not found: {0}")]
    BlockNotFound(u64),

    #[error("page not found: {0}")]
    PageNotFound(u64),

    #[error("corruption detected: {0}")]
    Corruption(String),

    // Transaction errors
    #[error("transaction error: {0}")]
    Transaction(String),

    #[error("transaction aborted: {0}")]
    TransactionAborted(String),

    #[error("deadlock detected")]
    Deadlock,

    #[error("lock timeout")]
    LockTimeout,

    #[error("serialization failure")]
    SerializationFailure,

    // Query errors
    #[error("parse error: {0}")]
    Parse(String),

    #[error("type error: {0}")]
    TypeError(String),

    #[error("execution error: {0}")]
    Execution(String),

    #[error("table not found: {0}")]
    TableNotFound(String),

    #[error("column not found: {0}")]
    ColumnNotFound(String),

    #[error("index not found: {0}")]
    IndexNotFound(String),

    // Constraint errors
    #[error("constraint violation: {0}")]
    ConstraintViolation(String),

    #[error("unique constraint violation: {0}")]
    UniqueViolation(String),

    #[error("foreign key violation: {0}")]
    ForeignKeyViolation(String),

    #[error("check constraint violation: {0}")]
    CheckViolation(String),

    // Replication errors
    #[error("replication error: {0}")]
    Replication(String),

    #[error("node not found: {0}")]
    NodeNotFound(String),

    #[error("quorum not reached")]
    QuorumNotReached,

    #[error("leader not elected")]
    LeaderNotElected,

    // Authentication/Authorization errors
    #[error("authentication failed: {0}")]
    AuthenticationFailed(String),

    #[error("authorization denied: {0}")]
    AuthorizationDenied(String),

    // IO errors
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    // Serialization errors
    #[error("serialization error: {0}")]
    Serialization(String),

    // Encryption errors
    #[error("encryption error: {0}")]
    Encryption(String),

    // Configuration errors
    #[error("configuration error: {0}")]
    Configuration(String),

    // Internal errors
    #[error("internal error: {0}")]
    Internal(String),

    // Resource errors
    #[error("resource exhausted: {0}")]
    ResourceExhausted(String),

    #[error("memory limit exceeded")]
    MemoryLimitExceeded,

    // Network errors
    #[error("network error: {0}")]
    Network(String),

    #[error("connection refused: {0}")]
    ConnectionRefused(String),

    #[error("timeout: {0}")]
    Timeout(String),
}

// =============================================================================
// Type Aliases
// =============================================================================

/// Result type alias for Aegis operations.
pub type Result<T> = std::result::Result<T, AegisError>;

// =============================================================================
// Error Classification
// =============================================================================

impl AegisError {
    /// Returns true if the operation can be safely retried.
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            AegisError::Deadlock
                | AegisError::LockTimeout
                | AegisError::SerializationFailure
                | AegisError::QuorumNotReached
                | AegisError::LeaderNotElected
                | AegisError::Network(_)
                | AegisError::Timeout(_)
        )
    }

    /// Returns true if this is a user error (vs system error).
    pub fn is_user_error(&self) -> bool {
        matches!(
            self,
            AegisError::Parse(_)
                | AegisError::TypeError(_)
                | AegisError::TableNotFound(_)
                | AegisError::ColumnNotFound(_)
                | AegisError::ConstraintViolation(_)
                | AegisError::UniqueViolation(_)
                | AegisError::ForeignKeyViolation(_)
                | AegisError::CheckViolation(_)
                | AegisError::AuthenticationFailed(_)
                | AegisError::AuthorizationDenied(_)
        )
    }

    /// Returns true if this is a constraint violation error.
    pub fn is_constraint_error(&self) -> bool {
        matches!(
            self,
            AegisError::ConstraintViolation(_)
                | AegisError::UniqueViolation(_)
                | AegisError::ForeignKeyViolation(_)
                | AegisError::CheckViolation(_)
        )
    }
}
