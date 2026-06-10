//! Aegis Vault - Integrated Secrets Manager
//!
//! A Rust-native encrypted secrets vault that auto-initializes at startup.
//! Provides versioned secret storage, transit encryption, access control,
//! and audit logging.
//!
//! Key Features:
//! - AES-256-GCM encryption for all secret values
//! - PBKDF2-derived master key with seal/unseal lifecycle
//! - Versioned secrets with configurable retention
//! - Transit encryption engine (encryption-as-a-service)
//! - Component-based access control policies
//! - In-memory audit log with bounded capacity
//! - Disk persistence with atomic writes
//! - Auto-unseal on startup for development convenience
//!
//! @version 0.2.2
//! @author AutomataNexus Development Team

pub mod access;
pub mod audit;
pub mod config;
pub mod error;
pub mod master_key;
pub mod provider;
pub mod rotation;
pub mod secret;
pub mod store;
pub mod transit;

pub use access::AccessPolicy;
pub use config::VaultConfig;
pub use error::VaultError;
pub use provider::AegisVaultProvider;

use std::sync::Arc;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use crate::access::AccessController;
use crate::audit::{VaultAuditEntry, VaultAuditLog, VaultOperation};
use crate::master_key::SealManager;
use crate::store::VaultStore;
use crate::transit::TransitEngine;

/// Status information about the vault.
#[derive(Debug, Clone)]
pub struct VaultStatus {
    pub sealed: bool,
    pub secret_count: usize,
    pub transit_key_count: usize,
    pub uptime_secs: Option<u64>,
}

/// The main vault facade. Combines secret storage, transit encryption,
/// seal management, access control, and audit logging.
pub struct AegisVault {
    pub store: Arc<VaultStore>,
    pub transit: Arc<TransitEngine>,
    config: VaultConfig,
}

impl AegisVault {
    /// Initialize the vault with the given configuration.
    ///
    /// 1. Creates the SealManager
    /// 2. If data_dir exists and has vault data, loads the encrypted key blob from disk
    /// 3. If auto_unseal is true: derives key from passphrase or generates a new one
    /// 4. Creates VaultStore, TransitEngine, AuditLog
    /// 5. Loads persisted secrets if unsealed
    /// 6. Returns the initialized AegisVault
    pub async fn init(config: VaultConfig) -> Result<Self, VaultError> {
        let seal_manager = Arc::new(SealManager::new());
        let audit_log = Arc::new(VaultAuditLog::new(config.audit_log_max_entries));

        // Enable file-based audit log if data_dir is configured
        if let Some(ref path) = config.audit_log_path() {
            audit_log.set_log_file(path.clone());
            tracing::info!("Audit log file enabled: {:?}", path);
        }

        let access_controller = if config.access_default_deny {
            tracing::info!("Vault access control: deny-by-default (policies required)");
            Arc::new(AccessController::new_deny_by_default())
        } else {
            Arc::new(AccessController::new())
        };

        // Determine store path
        let store_path = config
            .vault_file_path()
            .unwrap_or_else(|| std::path::PathBuf::from("vault.dat"));
        let key_path = config.key_file_path();

        // Try to load existing encrypted key blob from disk
        let has_existing_key = if let Some(ref kp) = key_path {
            if kp.exists() {
                match std::fs::read(kp) {
                    Ok(blob) => {
                        seal_manager.set_encrypted_key_blob(blob);
                        tracing::info!("Loaded existing vault key from disk");
                        true
                    }
                    Err(e) => {
                        tracing::warn!("Failed to read vault key file: {}", e);
                        false
                    }
                }
            } else {
                false
            }
        } else {
            false
        };

        // Auto-unseal if configured
        if config.auto_unseal {
            if has_existing_key {
                // Unseal with passphrase — require explicit passphrase, no defaults
                let passphrase = config.passphrase.as_deref().ok_or_else(|| {
                    VaultError::Other(
                        "auto-unseal requires a passphrase: set via config or AEGIS_VAULT_PASSPHRASE env var".into(),
                    )
                })?;
                match seal_manager.unseal(passphrase) {
                    Ok(()) => {
                        audit_log.record_success(VaultOperation::Unseal, None, Some("system"));
                        tracing::info!("Vault auto-unsealed with existing key");
                    }
                    Err(e) => {
                        // CRITICAL: never regenerate/overwrite the master key here.
                        // The existing key blob decrypts the on-disk vault.dat; a wrong
                        // or transient passphrase must NOT destroy it. Leave the vault
                        // SEALED and surface the failure for operator intervention.
                        tracing::error!(
                            "Failed to unseal vault with the configured passphrase: {}. \
                             Leaving the vault SEALED and preserving the existing key. \
                             Check AEGIS_VAULT_PASSPHRASE; the master key was NOT regenerated.",
                            e
                        );
                        audit_log.record_failure(
                            VaultOperation::Unseal,
                            None,
                            Some("system"),
                            &e.to_string(),
                        );
                    }
                }
            } else {
                // First run: generate new key
                seal_manager.auto_unseal(config.passphrase.as_deref())?;
                audit_log.record_success(VaultOperation::Unseal, None, Some("system"));

                // Persist the encrypted key blob
                if let Some(ref kp) = key_path {
                    if let Some(blob) = seal_manager.get_encrypted_key_blob() {
                        if let Some(parent) = kp.parent() {
                            std::fs::create_dir_all(parent)?;
                        }
                        std::fs::write(kp, &blob)?;
                        #[cfg(unix)]
                        std::fs::set_permissions(kp, std::fs::Permissions::from_mode(0o600))?;
                        tracing::info!("Vault key persisted to disk");
                    }
                }
            }
        }

        let store = Arc::new(VaultStore::new(
            Arc::clone(&seal_manager),
            store_path,
            Arc::clone(&audit_log),
            Arc::clone(&access_controller),
        ));

        // Load persisted secrets if unsealed and data file exists
        if !seal_manager.is_sealed() {
            if let Err(e) = store.load_from_disk() {
                tracing::debug!("No persisted vault data loaded: {}", e);
            }
        }

        let transit = Arc::new(TransitEngine::new());

        // Load persisted transit keys if unsealed
        if !seal_manager.is_sealed() {
            if let Some(ref tp) = config.transit_keys_path() {
                if let Err(e) = transit.load_from_disk(&seal_manager, tp) {
                    tracing::debug!("No persisted transit keys loaded: {}", e);
                }
            }
        }

        tracing::info!(
            "Vault initialized (sealed={}, secrets={})",
            seal_manager.is_sealed(),
            store.secret_count()
        );

        Ok(Self {
            store,
            transit,
            config,
        })
    }

    /// Get a secret value by key.
    pub fn get(&self, key: &str, component: &str) -> Result<String, VaultError> {
        self.store.get(key, component)
    }

    /// Set a secret value.
    pub fn set(&self, key: &str, value: &str, component: &str) -> Result<(), VaultError> {
        self.store.set(key, value, component)?;

        // Auto-persist if data_dir is configured
        if self.config.data_dir.is_some() {
            if let Err(e) = self.store.save_to_disk() {
                tracing::warn!("Failed to persist vault data: {}", e);
            }
        }

        Ok(())
    }

    /// Delete a secret by key.
    pub fn delete(&self, key: &str, component: &str) -> Result<(), VaultError> {
        self.store.delete(key, component)?;

        if self.config.data_dir.is_some() {
            if let Err(e) = self.store.save_to_disk() {
                tracing::warn!("Failed to persist vault data after delete: {}", e);
            }
        }

        Ok(())
    }

    /// List secret keys matching a prefix.
    pub fn list(&self, prefix: &str, component: &str) -> Result<Vec<String>, VaultError> {
        self.store.list(prefix, component)
    }

    /// Seal the vault. All operations requiring the master key will fail.
    pub fn seal(&self) -> Result<(), VaultError> {
        self.store.seal_manager().seal()?;
        self.store
            .audit_log()
            .record_success(VaultOperation::Seal, None, Some("system"));
        tracing::info!("Vault sealed");
        Ok(())
    }

    /// Unseal the vault with a passphrase.
    pub fn unseal(&self, passphrase: &str) -> Result<(), VaultError> {
        self.store.seal_manager().unseal(passphrase)?;
        self.store
            .audit_log()
            .record_success(VaultOperation::Unseal, None, Some("system"));

        // Reload persisted data
        if let Err(e) = self.store.load_from_disk() {
            tracing::debug!("No persisted vault data to load after unseal: {}", e);
        }

        tracing::info!("Vault unsealed");
        Ok(())
    }

    /// Check if the vault is currently sealed.
    pub fn is_sealed(&self) -> bool {
        self.store.seal_manager().is_sealed()
    }

    /// Get vault status information.
    pub fn status(&self) -> VaultStatus {
        VaultStatus {
            sealed: self.is_sealed(),
            secret_count: self.store.secret_count(),
            transit_key_count: self.transit.key_count(),
            uptime_secs: self.store.seal_manager().uptime_secs(),
        }
    }

    /// Get recent audit log entries.
    pub fn audit_entries(&self, limit: usize) -> Vec<VaultAuditEntry> {
        self.store.audit_log().entries(limit)
    }

    /// Get the vault configuration.
    pub fn config(&self) -> &VaultConfig {
        &self.config
    }

    /// Add or replace a named access policy.
    ///
    /// Policies are the grant mechanism when the vault runs with
    /// `access_default_deny: true` (every operation is denied unless a policy
    /// allows it). In the default allow-all mode policies have no restricting
    /// effect; flip `access_default_deny` to enforce them.
    pub fn add_access_policy(&self, policy: AccessPolicy) {
        self.store.access_controller().add_policy(policy);
    }

    /// Remove an access policy by name. Returns true if it existed.
    pub fn remove_access_policy(&self, name: &str) -> bool {
        self.store.access_controller().remove_policy(name)
    }

    /// List the names of all configured access policies.
    pub fn list_access_policies(&self) -> Vec<String> {
        self.store.access_controller().list_policies()
    }

    /// Synchronous auto-initialization constructor for sync contexts like
    /// `AppState::new()`. Key handling mirrors the async [`init`](Self::init)
    /// so the production server gets durable, passphrase-protected secrets
    /// instead of a throwaway key:
    ///
    /// - If `data_dir/vault.key` exists, the persisted master key is reused.
    ///   It is unsealed with `AEGIS_VAULT_PASSPHRASE` when set; if the
    ///   passphrase is missing or wrong, the vault starts **sealed** and the
    ///   existing key is left intact (never regenerated/overwritten).
    /// - On first run (no key file) with a passphrase set, a new key is
    ///   generated and persisted (0600) so it survives restarts.
    /// - On first run with no passphrase, an ephemeral key is generated and
    ///   the vault runs unsealed for this process only (dev convenience), and
    ///   no key file is written.
    ///
    /// Access control: allow-all by default; set AEGIS_VAULT_DEFAULT_DENY=true
    /// to start deny-by-default (the embedding application must then seed
    /// policies via [`add_access_policy`](Self::add_access_policy)).
    pub fn new_auto(data_dir: Option<std::path::PathBuf>) -> Self {
        let seal_manager = Arc::new(SealManager::new());
        let audit_log = Arc::new(VaultAuditLog::new(10000));
        let default_deny = std::env::var("AEGIS_VAULT_DEFAULT_DENY")
            .map(|v| v.eq_ignore_ascii_case("true") || v == "1")
            .unwrap_or(false);
        let access_controller = if default_deny {
            tracing::info!("Vault access control: deny-by-default (policies required)");
            Arc::new(AccessController::new_deny_by_default())
        } else {
            Arc::new(AccessController::new())
        };

        let passphrase = std::env::var("AEGIS_VAULT_PASSPHRASE").ok();
        let key_path = data_dir.as_ref().map(|d| d.join("vault.key"));

        // Load an existing persisted key blob, if any.
        let has_existing_key = match key_path.as_ref() {
            Some(kp) if kp.exists() => match std::fs::read(kp) {
                Ok(blob) => {
                    seal_manager.set_encrypted_key_blob(blob);
                    true
                }
                Err(e) => {
                    tracing::warn!("Failed to read vault key file {:?}: {}", kp, e);
                    false
                }
            },
            _ => false,
        };

        if has_existing_key {
            // Reuse the persisted key. Only a correct passphrase can unseal it;
            // never regenerate it, or the on-disk vault.dat becomes unreadable.
            match passphrase.as_deref() {
                Some(pp) => match seal_manager.unseal(pp) {
                    Ok(()) => {
                        audit_log.record_success(VaultOperation::Unseal, None, Some("system"));
                        tracing::info!("Vault unsealed with existing persisted key");
                    }
                    Err(e) => {
                        tracing::error!(
                            "Failed to unseal vault with AEGIS_VAULT_PASSPHRASE: {}. \
                             Starting SEALED; the existing key was NOT regenerated.",
                            e
                        );
                        audit_log.record_failure(
                            VaultOperation::Unseal,
                            None,
                            Some("system"),
                            &e.to_string(),
                        );
                    }
                },
                None => {
                    tracing::error!(
                        "A persisted vault key exists but AEGIS_VAULT_PASSPHRASE is not set. \
                         Starting SEALED. Set the passphrase to unseal."
                    );
                    audit_log.record_failure(
                        VaultOperation::Unseal,
                        None,
                        Some("system"),
                        "missing passphrase for existing key",
                    );
                }
            }
        } else {
            // First run: generate a key.
            match seal_manager.auto_unseal(passphrase.as_deref()) {
                Ok(()) => {
                    audit_log.record_success(VaultOperation::Unseal, None, Some("system"));
                    // Persist the key blob ONLY when a passphrase protects it, so it
                    // can actually be unsealed after a restart. Without a passphrase
                    // the key is encrypted under a random, unrecoverable secret, so
                    // persisting it would be useless — keep it ephemeral instead.
                    if passphrase.is_some() {
                        if let (Some(kp), Some(blob)) =
                            (key_path.as_ref(), seal_manager.get_encrypted_key_blob())
                        {
                            let persisted = (|| -> std::io::Result<()> {
                                if let Some(parent) = kp.parent() {
                                    std::fs::create_dir_all(parent)?;
                                }
                                std::fs::write(kp, &blob)?;
                                #[cfg(unix)]
                                std::fs::set_permissions(
                                    kp,
                                    std::fs::Permissions::from_mode(0o600),
                                )?;
                                Ok(())
                            })();
                            match persisted {
                                Ok(()) => tracing::info!("Vault key persisted to disk"),
                                Err(e) => {
                                    tracing::error!("Failed to persist vault key {:?}: {}", kp, e)
                                }
                            }
                        }
                    } else {
                        tracing::warn!(
                            "Vault running with an ephemeral key (AEGIS_VAULT_PASSPHRASE unset); \
                             secrets will NOT survive a restart. Set a passphrase for durability."
                        );
                    }
                }
                Err(e) => {
                    tracing::warn!("Vault auto-unseal failed: {}. Starting sealed.", e);
                    audit_log.record_failure(
                        VaultOperation::Unseal,
                        None,
                        Some("system"),
                        &e.to_string(),
                    );
                }
            }
        }

        let store_path = data_dir
            .as_ref()
            .map(|d| d.join("vault.dat"))
            .unwrap_or_else(|| std::path::PathBuf::from("vault.dat"));

        let store = Arc::new(VaultStore::new(
            Arc::clone(&seal_manager),
            store_path,
            Arc::clone(&audit_log),
            Arc::clone(&access_controller),
        ));

        // Try loading persisted data if unsealed and data dir exists
        if !seal_manager.is_sealed() {
            if let Some(ref dir) = data_dir {
                if dir.exists() {
                    let _ = store.load_from_disk();
                }
            }
        }

        let config = VaultConfig {
            data_dir,
            auto_unseal: true,
            access_default_deny: default_deny,
            ..VaultConfig::default()
        };

        Self {
            store,
            transit: Arc::new(TransitEngine::new()),
            config,
        }
    }

    /// Encrypt data using a named transit key.
    pub fn transit_encrypt(&self, key_name: &str, plaintext: &[u8]) -> Result<Vec<u8>, VaultError> {
        self.transit.encrypt(key_name, plaintext)
    }

    /// Decrypt data using a named transit key.
    pub fn transit_decrypt(
        &self,
        key_name: &str,
        ciphertext: &[u8],
    ) -> Result<Vec<u8>, VaultError> {
        self.transit.decrypt(key_name, ciphertext)
    }

    /// Create a new named transit key.
    pub fn transit_create_key(&self, name: &str) -> Result<(), VaultError> {
        self.transit.create_key(name)?;
        self.persist_transit_keys();
        Ok(())
    }

    /// Persist transit keys to disk if data_dir is configured.
    fn persist_transit_keys(&self) {
        if let Some(ref tp) = self.config.transit_keys_path() {
            if let Err(e) = self.transit.save_to_disk(self.store.seal_manager(), tp) {
                tracing::warn!("Failed to persist transit keys: {}", e);
            }
        }
    }

    /// List all transit key names.
    pub fn transit_list_keys(&self) -> Vec<String> {
        self.transit.list_keys()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_init_default() {
        let vault = AegisVault::init(VaultConfig::for_testing()).await.unwrap();
        assert!(!vault.is_sealed());
        assert_eq!(vault.status().secret_count, 0);
    }

    #[tokio::test]
    async fn test_set_get_delete() {
        let vault = AegisVault::init(VaultConfig::for_testing()).await.unwrap();

        vault.set("db/password", "hunter2", "server").unwrap();
        assert_eq!(vault.get("db/password", "server").unwrap(), "hunter2");

        vault.delete("db/password", "server").unwrap();
        assert!(vault.get("db/password", "server").is_err());
    }

    #[tokio::test]
    async fn test_list() {
        let vault = AegisVault::init(VaultConfig::for_testing()).await.unwrap();

        vault.set("app/key1", "v1", "test").unwrap();
        vault.set("app/key2", "v2", "test").unwrap();
        vault.set("other/key3", "v3", "test").unwrap();

        let mut keys = vault.list("app/", "test").unwrap();
        keys.sort();
        assert_eq!(keys, vec!["app/key1", "app/key2"]);
    }

    #[tokio::test]
    async fn test_seal_unseal() {
        let config = VaultConfig {
            passphrase: Some("seal_test_pass".into()),
            ..VaultConfig::for_testing()
        };
        let vault = AegisVault::init(config).await.unwrap();

        vault.set("secret", "value", "test").unwrap();
        vault.seal().unwrap();
        assert!(vault.is_sealed());

        // Operations fail when sealed
        assert!(vault.get("secret", "test").is_err());
        assert!(vault.set("new", "val", "test").is_err());

        vault.unseal("seal_test_pass").unwrap();
        assert!(!vault.is_sealed());
    }

    #[tokio::test]
    async fn test_status() {
        let vault = AegisVault::init(VaultConfig::for_testing()).await.unwrap();
        vault.set("k1", "v1", "test").unwrap();

        let status = vault.status();
        assert!(!status.sealed);
        assert_eq!(status.secret_count, 1);
        assert_eq!(status.transit_key_count, 0);
        assert!(status.uptime_secs.is_some());
    }

    #[tokio::test]
    async fn test_audit_entries() {
        let vault = AegisVault::init(VaultConfig::for_testing()).await.unwrap();
        vault.set("k", "v", "test").unwrap();
        vault.get("k", "test").unwrap();

        let entries = vault.audit_entries(10);
        // At least: unseal + set + get
        assert!(entries.len() >= 3);
    }

    #[tokio::test]
    async fn test_transit_integration() {
        let vault = AegisVault::init(VaultConfig::for_testing()).await.unwrap();

        vault.transit.create_key("app_key").unwrap();
        assert_eq!(vault.status().transit_key_count, 1);

        let ct = vault.transit.encrypt("app_key", b"hello").unwrap();
        let pt = vault.transit.decrypt("app_key", &ct).unwrap();
        assert_eq!(pt, b"hello");
    }

    #[tokio::test]
    async fn test_persistence_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let data_dir = dir.path().to_path_buf();

        // Create vault, add secret, drop it
        {
            let config = VaultConfig {
                data_dir: Some(data_dir.clone()),
                auto_unseal: true,
                passphrase: Some("persist_pass".into()),
                ..VaultConfig::for_testing()
            };
            let vault = AegisVault::init(config).await.unwrap();
            vault
                .set("persistent_secret", "secret_value", "test")
                .unwrap();
        }

        // Re-create vault from same directory
        {
            let config = VaultConfig {
                data_dir: Some(data_dir),
                auto_unseal: true,
                passphrase: Some("persist_pass".into()),
                ..VaultConfig::for_testing()
            };
            let vault = AegisVault::init(config).await.unwrap();
            let val = vault.get("persistent_secret", "test").unwrap();
            assert_eq!(val, "secret_value");
        }
    }

    #[tokio::test]
    async fn test_multiple_versions() {
        let vault = AegisVault::init(VaultConfig::for_testing()).await.unwrap();

        vault.set("rotating", "v1", "test").unwrap();
        vault.set("rotating", "v2", "test").unwrap();
        vault.set("rotating", "v3", "test").unwrap();

        // Current should be v3
        assert_eq!(vault.get("rotating", "test").unwrap(), "v3");
    }

    #[tokio::test]
    async fn test_access_default_deny_blocks_until_policy_granted() {
        let config = VaultConfig {
            access_default_deny: true,
            ..VaultConfig::for_testing()
        };
        let vault = AegisVault::init(config).await.unwrap();

        // No policies: every operation is denied.
        assert!(matches!(
            vault.set("db/password", "x", "app"),
            Err(VaultError::AccessDenied(_))
        ));
        assert!(matches!(
            vault.list("", "app"),
            Err(VaultError::AccessDenied(_))
        ));

        // Grant the "app" component read+write on db/ only.
        vault.add_access_policy(AccessPolicy {
            name: "app-db".into(),
            allowed_components: ["app"].into_iter().map(String::from).collect(),
            allowed_prefixes: vec!["db/".into()],
            read: true,
            write: true,
            delete: false,
        });
        assert_eq!(vault.list_access_policies(), vec!["app-db"]);

        vault.set("db/password", "hunter2", "app").unwrap();
        assert_eq!(vault.get("db/password", "app").unwrap(), "hunter2");

        // Outside the granted prefix, other components, and delete: denied.
        assert!(matches!(
            vault.set("api/key", "x", "app"),
            Err(VaultError::AccessDenied(_))
        ));
        assert!(matches!(
            vault.get("db/password", "intruder"),
            Err(VaultError::AccessDenied(_))
        ));
        assert!(matches!(
            vault.delete("db/password", "app"),
            Err(VaultError::AccessDenied(_))
        ));

        // Removing the policy returns the vault to deny-everything.
        assert!(vault.remove_access_policy("app-db"));
        assert!(matches!(
            vault.get("db/password", "app"),
            Err(VaultError::AccessDenied(_))
        ));
    }

    #[tokio::test]
    async fn test_default_allow_unchanged_by_default() {
        let vault = AegisVault::init(VaultConfig::for_testing()).await.unwrap();
        assert!(!vault.config().access_default_deny);
        // Any component may read/write without policies (backwards compat).
        vault.set("open/key", "v", "anyone").unwrap();
        assert_eq!(vault.get("open/key", "someone_else").unwrap(), "v");
    }
}
