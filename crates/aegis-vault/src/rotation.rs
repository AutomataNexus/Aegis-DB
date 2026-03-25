use std::sync::Arc;
use std::time::Duration;

use crate::store::VaultStore;

/// Checks all secrets for expired rotation TTLs and logs warnings.
pub fn check_rotations(store: &VaultStore) {
    let now = chrono::Utc::now().timestamp() as u64;
    let secrets = store.secrets_snapshot();

    for (key, secret) in &secrets {
        if let Some(ttl) = secret.metadata.rotation_ttl_secs {
            let age = now.saturating_sub(secret.metadata.updated_at);
            if age > ttl {
                tracing::warn!(
                    "Secret '{}' is past its rotation TTL ({} secs old, TTL is {} secs)",
                    key,
                    age,
                    ttl
                );
            }
        }

        // Check for expired versions
        if let Some(current) = secret.current_version() {
            if let Some(expires_at) = current.expires_at {
                if now > expires_at {
                    tracing::warn!(
                        "Secret '{}' version {} has expired (expired at {})",
                        key,
                        current.version,
                        expires_at
                    );
                }
            }
        }
    }
}

/// Run an async loop that periodically checks for secrets needing rotation.
pub async fn run_rotation_loop(store: Arc<VaultStore>, interval: Duration) {
    loop {
        tokio::time::sleep(interval).await;

        if store.seal_manager().is_sealed() {
            tracing::debug!("Vault is sealed, skipping rotation check");
            continue;
        }

        check_rotations(&store);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::access::AccessController;
    use crate::audit::VaultAuditLog;
    use crate::master_key::SealManager;
    use std::path::PathBuf;

    fn make_test_store() -> VaultStore {
        let seal = Arc::new(SealManager::new());
        seal.auto_unseal(Some("test")).unwrap();
        let audit = Arc::new(VaultAuditLog::new(100));
        let access = Arc::new(AccessController::new());
        VaultStore::new(seal, PathBuf::from("/tmp/test_vault"), audit, access)
    }

    #[test]
    fn test_check_rotations_empty() {
        let store = make_test_store();
        // Should not panic on empty store
        check_rotations(&store);
    }

    #[test]
    fn test_check_rotations_with_secrets() {
        let store = make_test_store();
        store.set("test_key", "test_value", "test").unwrap();
        // Should not panic with secrets present
        check_rotations(&store);
    }

    #[tokio::test]
    async fn test_rotation_loop_sealed() {
        let seal = Arc::new(SealManager::new());
        // Vault stays sealed
        let audit = Arc::new(VaultAuditLog::new(100));
        let access = Arc::new(AccessController::new());
        let store = Arc::new(VaultStore::new(
            seal,
            PathBuf::from("/tmp/test_vault"),
            audit,
            access,
        ));

        // Run one iteration via timeout
        let result = tokio::time::timeout(
            Duration::from_millis(200),
            run_rotation_loop(store, Duration::from_millis(50)),
        )
        .await;

        // Should timeout (loop runs forever), which is expected
        assert!(result.is_err());
    }
}
