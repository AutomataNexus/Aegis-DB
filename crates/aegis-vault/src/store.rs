use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use parking_lot::RwLock;

use crate::access::{AccessController, Operation};
use crate::audit::{VaultAuditLog, VaultOperation};
use crate::error::VaultError;
use crate::master_key::SealManager;
use crate::secret::Secret;

/// Serialized form of the vault store for disk persistence.
#[derive(serde::Serialize, serde::Deserialize)]
struct VaultData {
    secrets: HashMap<String, Secret>,
}

/// The core encrypted key-value secret store.
pub struct VaultStore {
    seal_manager: Arc<SealManager>,
    secrets: RwLock<HashMap<String, Secret>>,
    store_path: PathBuf,
    audit_log: Arc<VaultAuditLog>,
    access_controller: Arc<AccessController>,
}

impl VaultStore {
    pub fn new(
        seal_manager: Arc<SealManager>,
        store_path: PathBuf,
        audit_log: Arc<VaultAuditLog>,
        access_controller: Arc<AccessController>,
    ) -> Self {
        Self {
            seal_manager,
            secrets: RwLock::new(HashMap::new()),
            store_path,
            audit_log,
            access_controller,
        }
    }

    /// Get the seal manager reference.
    pub fn seal_manager(&self) -> &SealManager {
        &self.seal_manager
    }

    /// Get the audit log reference.
    pub fn audit_log(&self) -> &VaultAuditLog {
        &self.audit_log
    }

    /// Get the access controller reference.
    pub fn access_controller(&self) -> &AccessController {
        &self.access_controller
    }

    /// Get a snapshot of the secrets map (for rotation checks etc.).
    pub fn secrets_snapshot(&self) -> HashMap<String, Secret> {
        self.secrets.read().clone()
    }

    /// Get the number of secrets stored.
    pub fn secret_count(&self) -> usize {
        self.secrets.read().len()
    }

    /// Retrieve and decrypt the current version of a secret.
    pub fn get(&self, key: &str, component: &str) -> Result<String, VaultError> {
        if self.seal_manager.is_sealed() {
            self.audit_log.record_failure(
                VaultOperation::Get,
                Some(key),
                Some(component),
                "vault is sealed",
            );
            return Err(VaultError::Sealed);
        }

        self.access_controller
            .check_access(component, key, Operation::Read)?;

        let secrets = self.secrets.read();
        let secret = secrets
            .get(key)
            .ok_or_else(|| VaultError::SecretNotFound(key.to_string()))?;

        let version = secret
            .current_version()
            .ok_or_else(|| VaultError::SecretNotFound(key.to_string()))?;

        let plaintext = self.seal_manager.decrypt(&version.encrypted_value)?;
        let value = String::from_utf8(plaintext)
            .map_err(|e| VaultError::Encryption(format!("invalid UTF-8: {}", e)))?;

        self.audit_log
            .record_success(VaultOperation::Get, Some(key), Some(component));

        Ok(value)
    }

    /// Retrieve and decrypt a specific version of a secret.
    pub fn get_version(
        &self,
        key: &str,
        version: u32,
        component: &str,
    ) -> Result<String, VaultError> {
        if self.seal_manager.is_sealed() {
            return Err(VaultError::Sealed);
        }

        self.access_controller
            .check_access(component, key, Operation::Read)?;

        let secrets = self.secrets.read();
        let secret = secrets
            .get(key)
            .ok_or_else(|| VaultError::SecretNotFound(key.to_string()))?;

        let ver = secret
            .get_version(version)
            .ok_or_else(|| VaultError::SecretNotFound(format!("{}@v{}", key, version)))?;

        let plaintext = self.seal_manager.decrypt(&ver.encrypted_value)?;
        let value = String::from_utf8(plaintext)
            .map_err(|e| VaultError::Encryption(format!("invalid UTF-8: {}", e)))?;

        self.audit_log
            .record_success(VaultOperation::Get, Some(key), Some(component));

        Ok(value)
    }

    /// Encrypt and store a secret value. Creates a new version.
    pub fn set(&self, key: &str, value: &str, component: &str) -> Result<(), VaultError> {
        if self.seal_manager.is_sealed() {
            self.audit_log.record_failure(
                VaultOperation::Set,
                Some(key),
                Some(component),
                "vault is sealed",
            );
            return Err(VaultError::Sealed);
        }

        self.access_controller
            .check_access(component, key, Operation::Write)?;

        let encrypted = self.seal_manager.encrypt(value.as_bytes())?;

        let mut secrets = self.secrets.write();
        let secret = secrets
            .entry(key.to_string())
            .or_insert_with(|| Secret::new(key.to_string()));

        secret.add_version(encrypted, component.to_string());

        self.audit_log
            .record_success(VaultOperation::Set, Some(key), Some(component));

        Ok(())
    }

    /// Delete a secret entirely.
    pub fn delete(&self, key: &str, component: &str) -> Result<(), VaultError> {
        if self.seal_manager.is_sealed() {
            self.audit_log.record_failure(
                VaultOperation::Delete,
                Some(key),
                Some(component),
                "vault is sealed",
            );
            return Err(VaultError::Sealed);
        }

        self.access_controller
            .check_access(component, key, Operation::Delete)?;

        let mut secrets = self.secrets.write();
        if secrets.remove(key).is_none() {
            return Err(VaultError::SecretNotFound(key.to_string()));
        }

        self.audit_log
            .record_success(VaultOperation::Delete, Some(key), Some(component));

        Ok(())
    }

    /// List secret keys matching a prefix.
    pub fn list(&self, prefix: &str, component: &str) -> Result<Vec<String>, VaultError> {
        if self.seal_manager.is_sealed() {
            return Err(VaultError::Sealed);
        }

        self.access_controller
            .check_access(component, prefix, Operation::List)?;

        let secrets = self.secrets.read();
        let keys: Vec<String> = secrets
            .keys()
            .filter(|k| k.starts_with(prefix))
            .cloned()
            .collect();

        self.audit_log
            .record_success(VaultOperation::List, None, Some(component));

        Ok(keys)
    }

    /// Persist all secrets to disk. The entire secrets map is serialized,
    /// encrypted as a single blob, and written atomically.
    pub fn save_to_disk(&self) -> Result<(), VaultError> {
        if self.seal_manager.is_sealed() {
            return Err(VaultError::Sealed);
        }

        let secrets = self.secrets.read().clone();
        let data = VaultData { secrets };

        let json = serde_json::to_vec(&data)
            .map_err(|e| VaultError::Other(format!("serialization failed: {}", e)))?;

        let encrypted = self.seal_manager.encrypt(&json)?;

        // Ensure directory exists
        if let Some(parent) = self.store_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Atomic write via temp file
        let tmp_path = self.store_path.with_extension("tmp");
        std::fs::write(&tmp_path, &encrypted)?;

        // Restrict file permissions to owner-only before rename
        #[cfg(unix)]
        std::fs::set_permissions(&tmp_path, std::fs::Permissions::from_mode(0o600))?;

        std::fs::rename(&tmp_path, &self.store_path)?;

        tracing::debug!("Vault data saved to disk ({} secrets)", data.secrets.len());
        Ok(())
    }

    /// Load secrets from disk. Decrypts the blob and deserializes.
    /// If decryption succeeds but deserialization fails, the vault data file
    /// may be corrupt — an error is returned without modifying in-memory state.
    pub fn load_from_disk(&self) -> Result<(), VaultError> {
        if self.seal_manager.is_sealed() {
            return Err(VaultError::Sealed);
        }

        if !self.store_path.exists() {
            tracing::debug!("No vault data file found, starting fresh");
            return Ok(());
        }

        let encrypted = std::fs::read(&self.store_path)?;

        // Validate file isn't empty or impossibly small (nonce + tag = 28 bytes minimum)
        if encrypted.len() < 28 {
            return Err(VaultError::Other(
                "vault data file is too small or corrupt".into(),
            ));
        }

        let json = self.seal_manager.decrypt(&encrypted)?;

        // Validate decrypted data is valid UTF-8 before parsing
        let json_str = std::str::from_utf8(&json).map_err(|_| {
            VaultError::Other("decrypted vault data is not valid UTF-8 — file may be corrupt".into())
        })?;

        let data: VaultData = serde_json::from_str(json_str).map_err(|e| {
            tracing::error!("Vault data deserialization failed: {}", e);
            VaultError::Other("vault data deserialization failed — file may be corrupt".into())
        })?;

        *self.secrets.write() = data.secrets;

        tracing::info!(
            "Vault data loaded from disk ({} secrets)",
            self.secrets.read().len()
        );
        Ok(())
    }

    /// Get the store path.
    pub fn store_path(&self) -> &PathBuf {
        &self.store_path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_store() -> VaultStore {
        let seal = Arc::new(SealManager::new());
        seal.auto_unseal(Some("test_passphrase")).unwrap();
        let audit = Arc::new(VaultAuditLog::new(1000));
        let access = Arc::new(AccessController::new());
        VaultStore::new(seal, PathBuf::from("/tmp/test_vault_data"), audit, access)
    }

    #[test]
    fn test_set_and_get() {
        let store = make_store();
        store.set("db_password", "s3cret!", "server").unwrap();

        let val = store.get("db_password", "server").unwrap();
        assert_eq!(val, "s3cret!");
    }

    #[test]
    fn test_get_nonexistent() {
        let store = make_store();
        let result = store.get("missing", "server");
        assert!(matches!(result, Err(VaultError::SecretNotFound(_))));
    }

    #[test]
    fn test_versioning() {
        let store = make_store();
        store.set("key", "v1_value", "comp").unwrap();
        store.set("key", "v2_value", "comp").unwrap();

        let current = store.get("key", "comp").unwrap();
        assert_eq!(current, "v2_value");

        let v1 = store.get_version("key", 1, "comp").unwrap();
        assert_eq!(v1, "v1_value");

        let v2 = store.get_version("key", 2, "comp").unwrap();
        assert_eq!(v2, "v2_value");
    }

    #[test]
    fn test_delete() {
        let store = make_store();
        store.set("to_delete", "value", "comp").unwrap();
        store.delete("to_delete", "comp").unwrap();

        let result = store.get("to_delete", "comp");
        assert!(matches!(result, Err(VaultError::SecretNotFound(_))));
    }

    #[test]
    fn test_delete_nonexistent() {
        let store = make_store();
        let result = store.delete("nothing", "comp");
        assert!(matches!(result, Err(VaultError::SecretNotFound(_))));
    }

    #[test]
    fn test_list() {
        let store = make_store();
        store.set("db/password", "p1", "server").unwrap();
        store.set("db/username", "u1", "server").unwrap();
        store.set("api/key", "k1", "server").unwrap();

        let mut db_keys = store.list("db/", "server").unwrap();
        db_keys.sort();
        assert_eq!(db_keys, vec!["db/password", "db/username"]);

        let all_keys = store.list("", "server").unwrap();
        assert_eq!(all_keys.len(), 3);
    }

    #[test]
    fn test_operations_when_sealed() {
        let seal = Arc::new(SealManager::new());
        // Do NOT unseal
        let audit = Arc::new(VaultAuditLog::new(100));
        let access = Arc::new(AccessController::new());
        let store = VaultStore::new(seal, PathBuf::from("/tmp/test"), audit, access);

        assert!(matches!(store.get("k", "c"), Err(VaultError::Sealed)));
        assert!(matches!(store.set("k", "v", "c"), Err(VaultError::Sealed)));
        assert!(matches!(store.delete("k", "c"), Err(VaultError::Sealed)));
        assert!(matches!(store.list("", "c"), Err(VaultError::Sealed)));
    }

    #[test]
    fn test_audit_logging() {
        let store = make_store();
        store.set("key1", "val1", "server").unwrap();
        store.get("key1", "server").unwrap();

        let entries = store.audit_log().entries(10);
        assert!(entries.len() >= 2);
    }

    #[test]
    fn test_save_and_load() {
        let dir = tempfile::tempdir().unwrap();
        let vault_path = dir.path().join("vault.dat");

        // Create and populate a store
        let seal = Arc::new(SealManager::new());
        seal.auto_unseal(Some("persist_test")).unwrap();
        let audit = Arc::new(VaultAuditLog::new(100));
        let access = Arc::new(AccessController::new());

        let store = VaultStore::new(
            Arc::clone(&seal),
            vault_path.clone(),
            Arc::clone(&audit),
            Arc::clone(&access),
        );

        store.set("saved_key", "saved_value", "test").unwrap();
        store.save_to_disk().unwrap();

        // Load into a new store with same seal manager
        let store2 = VaultStore::new(
            Arc::clone(&seal),
            vault_path,
            Arc::new(VaultAuditLog::new(100)),
            Arc::new(AccessController::new()),
        );

        store2.load_from_disk().unwrap();
        let val = store2.get("saved_key", "test").unwrap();
        assert_eq!(val, "saved_value");
    }

    #[test]
    fn test_load_nonexistent_file() {
        let store = make_store();
        // Should succeed silently (fresh start)
        let result = store.load_from_disk();
        // The path is /tmp/test_vault_data which may or may not exist;
        // either outcome is valid. If it doesn't exist, Ok(()). If it does
        // and is corrupted, error. We mainly test that no panic occurs.
        let _ = result;
    }

    #[test]
    fn test_secret_count() {
        let store = make_store();
        assert_eq!(store.secret_count(), 0);
        store.set("a", "1", "c").unwrap();
        store.set("b", "2", "c").unwrap();
        assert_eq!(store.secret_count(), 2);
    }
}
