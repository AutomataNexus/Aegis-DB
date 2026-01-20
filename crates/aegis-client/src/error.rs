//! Aegis Client Error Types
//!
//! Error types for client operations.
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use std::fmt;

// =============================================================================
// Client Error
// =============================================================================

/// Errors that can occur during client operations.
#[derive(Debug, Clone)]
pub enum ClientError {
    /// Connection failed.
    ConnectionFailed(String),
    /// Connection timeout.
    ConnectionTimeout,
    /// Connection closed.
    ConnectionClosed,
    /// Query execution failed.
    QueryFailed(String),
    /// Transaction failed.
    TransactionFailed(String),
    /// Invalid configuration.
    InvalidConfig(String),
    /// Invalid URL format.
    InvalidUrl(String),
    /// Authentication failed.
    AuthenticationFailed(String),
    /// Pool exhausted.
    PoolExhausted,
    /// Pool timeout.
    PoolTimeout,
    /// Serialization error.
    SerializationError(String),
    /// IO error.
    IoError(String),
    /// Protocol error.
    ProtocolError(String),
    /// Server error.
    ServerError { code: i32, message: String },
    /// Not connected.
    NotConnected,
    /// Transaction not started.
    NoTransaction,
    /// Transaction already started.
    TransactionAlreadyStarted,
}

impl fmt::Display for ClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConnectionFailed(msg) => write!(f, "Connection failed: {}", msg),
            Self::ConnectionTimeout => write!(f, "Connection timeout"),
            Self::ConnectionClosed => write!(f, "Connection closed"),
            Self::QueryFailed(msg) => write!(f, "Query failed: {}", msg),
            Self::TransactionFailed(msg) => write!(f, "Transaction failed: {}", msg),
            Self::InvalidConfig(msg) => write!(f, "Invalid configuration: {}", msg),
            Self::InvalidUrl(msg) => write!(f, "Invalid URL: {}", msg),
            Self::AuthenticationFailed(msg) => write!(f, "Authentication failed: {}", msg),
            Self::PoolExhausted => write!(f, "Connection pool exhausted"),
            Self::PoolTimeout => write!(f, "Connection pool timeout"),
            Self::SerializationError(msg) => write!(f, "Serialization error: {}", msg),
            Self::IoError(msg) => write!(f, "IO error: {}", msg),
            Self::ProtocolError(msg) => write!(f, "Protocol error: {}", msg),
            Self::ServerError { code, message } => {
                write!(f, "Server error [{}]: {}", code, message)
            }
            Self::NotConnected => write!(f, "Not connected"),
            Self::NoTransaction => write!(f, "No transaction in progress"),
            Self::TransactionAlreadyStarted => write!(f, "Transaction already started"),
        }
    }
}

impl std::error::Error for ClientError {}

impl ClientError {
    /// Check if the error is retryable.
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::ConnectionTimeout
                | Self::ConnectionClosed
                | Self::PoolExhausted
                | Self::PoolTimeout
                | Self::IoError(_)
        )
    }

    /// Check if the error is a connection error.
    pub fn is_connection_error(&self) -> bool {
        matches!(
            self,
            Self::ConnectionFailed(_)
                | Self::ConnectionTimeout
                | Self::ConnectionClosed
                | Self::NotConnected
        )
    }

    /// Check if the error is a server error.
    pub fn is_server_error(&self) -> bool {
        matches!(self, Self::ServerError { .. })
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = ClientError::ConnectionFailed("refused".to_string());
        assert_eq!(err.to_string(), "Connection failed: refused");

        let err = ClientError::ServerError {
            code: 500,
            message: "Internal error".to_string(),
        };
        assert_eq!(err.to_string(), "Server error [500]: Internal error");
    }

    #[test]
    fn test_is_retryable() {
        assert!(ClientError::ConnectionTimeout.is_retryable());
        assert!(ClientError::PoolExhausted.is_retryable());
        assert!(!ClientError::QueryFailed("syntax".to_string()).is_retryable());
    }

    #[test]
    fn test_is_connection_error() {
        assert!(ClientError::ConnectionFailed("refused".to_string()).is_connection_error());
        assert!(ClientError::NotConnected.is_connection_error());
        assert!(!ClientError::QueryFailed("error".to_string()).is_connection_error());
    }
}
