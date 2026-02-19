//! Aegis Updates - OTA Update Orchestration
//!
//! Provides rolling update orchestration for Aegis-DB clusters managed by PM2.
//! Handles binary download, verification, staging, rolling deployment with
//! follower-first ordering, health verification, and automatic rollback on failure.

pub mod binary;
pub mod health;
pub mod orchestrator;
pub mod rollback;
pub mod version;

use thiserror::Error;

/// Errors that can occur during the OTA update process.
#[derive(Debug, Error)]
pub enum UpdateError {
    #[error("Download failed: {0}")]
    DownloadFailed(String),

    #[error("Checksum mismatch: expected {expected}, got {actual}")]
    ChecksumMismatch { expected: String, actual: String },

    #[error("Staging failed: {0}")]
    StagingFailed(String),

    #[error("Node unreachable: {0}")]
    NodeUnreachable(String),

    #[error("Health check failed: {0}")]
    HealthCheckFailed(String),

    #[error("Rollback failed: {0}")]
    RollbackFailed(String),

    #[error("Plan not found: {0}")]
    PlanNotFound(String),

    #[error("Update in progress")]
    UpdateInProgress,

    #[error("IO error: {0}")]
    Io(String),
}

impl From<std::io::Error> for UpdateError {
    fn from(err: std::io::Error) -> Self {
        UpdateError::Io(err.to_string())
    }
}

impl From<reqwest::Error> for UpdateError {
    fn from(err: reqwest::Error) -> Self {
        UpdateError::DownloadFailed(err.to_string())
    }
}
