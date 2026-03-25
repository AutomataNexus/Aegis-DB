//! Rollback support for failed updates.

use std::path::{Path, PathBuf};

use tracing::{error, info, warn};

use crate::health::{wait_for_healthy, HealthCheck};
use crate::UpdateError;

/// Restore a backed-up binary to the target path.
///
/// Copies the backup binary over the target and ensures it is executable.
pub fn restore_backup(backup_path: &Path, target: &Path) -> Result<(), UpdateError> {
    if !backup_path.exists() {
        return Err(UpdateError::RollbackFailed(format!(
            "Backup file not found: {}",
            backup_path.display()
        )));
    }

    info!(
        backup = %backup_path.display(),
        target = %target.display(),
        "Restoring backup binary"
    );

    std::fs::copy(backup_path, target).map_err(|e| {
        UpdateError::RollbackFailed(format!(
            "Failed to restore {} -> {}: {e}",
            backup_path.display(),
            target.display()
        ))
    })?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o755);
        std::fs::set_permissions(target, perms)
            .map_err(|e| UpdateError::RollbackFailed(format!("Failed to set permissions: {e}")))?;
    }

    info!(target = %target.display(), "Backup restored successfully");
    Ok(())
}

/// Roll back a single node by restoring its backup binary and waiting for it to recover.
///
/// This function:
/// 1. Restores the backup binary to the node's binary path.
/// 2. Signals the node to restart (via POST /api/v1/admin/restart).
/// 3. Waits for the node to come back healthy.
///
/// `backup_path` is the path to the backup binary on the local filesystem.
/// `target_binary` is the path where the running binary should be placed.
/// `node_address` is the HTTP address of the node for health checking.
pub async fn rollback_node(
    node_address: &str,
    backup_path: &Path,
    target_binary: &Path,
) -> Result<(), UpdateError> {
    info!(
        address = %node_address,
        backup = %backup_path.display(),
        "Rolling back node"
    );

    // Restore the backup binary
    restore_backup(backup_path, target_binary)?;

    // Try to signal the node to restart so PM2 picks up the restored binary
    let restart_url = format!(
        "{}/api/v1/admin/restart",
        node_address.trim_end_matches('/')
    );

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| UpdateError::RollbackFailed(format!("Failed to build HTTP client: {e}")))?;

    match client.post(&restart_url).send().await {
        Ok(_) => {
            info!(address = %node_address, "Restart signal sent for rollback");
        }
        Err(e) => {
            warn!(
                address = %node_address,
                error = %e,
                "Failed to send restart signal; node may already be down, PM2 will restart it"
            );
        }
    }

    // Wait for the node to come back healthy
    let health_check = HealthCheck::new(node_address, "")
        .with_retries(30)
        .with_timeout(std::time::Duration::from_secs(5));

    match wait_for_healthy(&health_check).await {
        Ok(true) => {
            info!(address = %node_address, "Node recovered after rollback");
            Ok(())
        }
        Ok(false) => {
            error!(address = %node_address, "Node did not recover after rollback");
            Err(UpdateError::RollbackFailed(format!(
                "Node {node_address} did not recover after rollback"
            )))
        }
        Err(e) => {
            error!(address = %node_address, error = %e, "Rollback health check failed");
            Err(UpdateError::RollbackFailed(format!(
                "Health check failed during rollback of {node_address}: {e}"
            )))
        }
    }
}

/// Information needed to roll back a specific node.
#[derive(Debug, Clone)]
pub struct RollbackEntry {
    /// Network address of the node.
    pub node_address: String,
    /// Path to the backup binary for this node.
    pub backup_path: PathBuf,
    /// Path where the running binary is installed.
    pub target_binary: PathBuf,
}

/// Roll back multiple nodes, typically called when a rolling update fails partway through.
///
/// Attempts to roll back each node independently. Returns a list of nodes that
/// failed to roll back.
pub async fn rollback_nodes(entries: &[RollbackEntry]) -> Vec<UpdateError> {
    let mut errors = Vec::new();

    for entry in entries {
        match rollback_node(
            &entry.node_address,
            &entry.backup_path,
            &entry.target_binary,
        )
        .await
        {
            Ok(()) => {
                info!(address = %entry.node_address, "Node rolled back successfully");
            }
            Err(e) => {
                error!(
                    address = %entry.node_address,
                    error = %e,
                    "Failed to rollback node"
                );
                errors.push(e);
            }
        }
    }

    errors
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_restore_backup() {
        let dir = tempfile::tempdir().unwrap();
        let backup = dir.path().join("aegis-server.backup");
        let target = dir.path().join("aegis-server");

        std::fs::write(&backup, b"old binary").unwrap();
        std::fs::write(&target, b"new binary").unwrap();

        restore_backup(&backup, &target).unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"old binary");
    }

    #[test]
    fn test_restore_backup_missing() {
        let dir = tempfile::tempdir().unwrap();
        let backup = dir.path().join("nonexistent");
        let target = dir.path().join("target");

        let result = restore_backup(&backup, &target);
        assert!(result.is_err());
    }
}
