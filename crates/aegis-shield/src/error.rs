//! Shield Error Types

use aegis_common::AegisError;
use thiserror::Error;

/// Errors specific to the security shield subsystem.
#[derive(Error, Debug)]
pub enum ShieldError {
    /// A request was blocked by the shield.
    #[error("blocked IP {ip}: {reason}")]
    Blocked { ip: String, reason: String },

    /// A security policy was violated.
    #[error("policy violation: {0}")]
    PolicyViolation(String),

    /// Shield configuration error.
    #[error("config error: {0}")]
    ConfigError(String),

    /// Other shield errors.
    #[error("shield error: {0}")]
    Other(String),
}

impl From<ShieldError> for AegisError {
    fn from(err: ShieldError) -> Self {
        AegisError::Shield(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blocked_error_display() {
        let err = ShieldError::Blocked {
            ip: "10.0.0.1".to_string(),
            reason: "brute force".to_string(),
        };
        assert!(err.to_string().contains("10.0.0.1"));
        assert!(err.to_string().contains("brute force"));
    }

    #[test]
    fn test_policy_violation_display() {
        let err = ShieldError::PolicyViolation("rate exceeded".to_string());
        assert!(err.to_string().contains("rate exceeded"));
    }

    #[test]
    fn test_into_aegis_error() {
        let err = ShieldError::Other("test".to_string());
        let aegis_err: AegisError = err.into();
        match aegis_err {
            AegisError::Shield(msg) => assert!(msg.contains("test")),
            _ => panic!("expected Shield variant"),
        }
    }

    #[test]
    fn test_config_error() {
        let err = ShieldError::ConfigError("bad preset".to_string());
        assert!(err.to_string().contains("bad preset"));
    }
}
