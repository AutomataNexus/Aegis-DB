use std::path::PathBuf;

/// Configuration for the integrated vault.
#[derive(Debug, Clone)]
pub struct VaultConfig {
    /// Directory for vault persistence. If None, vault operates in-memory only.
    pub data_dir: Option<PathBuf>,
    /// Whether to automatically unseal on startup (default: true).
    pub auto_unseal: bool,
    /// Passphrase for sealing/unsealing. Read from AEGIS_VAULT_PASSPHRASE env var if not set.
    pub passphrase: Option<String>,
    /// Maximum number of versions to retain per secret (default: 10).
    pub max_versions: u32,
    /// Interval in seconds between rotation checks (default: 3600).
    pub rotation_check_interval_secs: u64,
    /// Maximum number of audit log entries to keep in memory (default: 10000).
    pub audit_log_max_entries: usize,
    /// When true, the access controller denies every operation unless an
    /// explicit [`AccessPolicy`](crate::access::AccessPolicy) grants it
    /// (add policies via `AegisVault::add_access_policy`). Default false
    /// (allow-all, backwards compatible). Recommended for production.
    pub access_default_deny: bool,
}

impl Default for VaultConfig {
    fn default() -> Self {
        Self {
            data_dir: None,
            auto_unseal: true,
            passphrase: std::env::var("AEGIS_VAULT_PASSPHRASE").ok(),
            max_versions: 10,
            rotation_check_interval_secs: 3600,
            audit_log_max_entries: 10_000,
            access_default_deny: false,
        }
    }
}

impl VaultConfig {
    /// Create a config for testing (in-memory, auto-unseal with fixed passphrase).
    pub fn for_testing() -> Self {
        Self {
            data_dir: None,
            auto_unseal: true,
            passphrase: Some("test_passphrase".into()),
            max_versions: 10,
            rotation_check_interval_secs: 3600,
            audit_log_max_entries: 1000,
            access_default_deny: false,
        }
    }

    /// Get the vault data file path.
    pub fn vault_file_path(&self) -> Option<PathBuf> {
        self.data_dir.as_ref().map(|d| d.join("vault.dat"))
    }

    /// Get the sealed key file path.
    pub fn key_file_path(&self) -> Option<PathBuf> {
        self.data_dir.as_ref().map(|d| d.join("vault.key"))
    }

    /// Get the transit keys file path.
    pub fn transit_keys_path(&self) -> Option<PathBuf> {
        self.data_dir.as_ref().map(|d| d.join("transit.dat"))
    }

    /// Get the audit log file path.
    pub fn audit_log_path(&self) -> Option<PathBuf> {
        self.data_dir.as_ref().map(|d| d.join("audit.log"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = VaultConfig::default();
        assert!(config.auto_unseal);
        assert!(config.data_dir.is_none());
        assert_eq!(config.max_versions, 10);
        assert_eq!(config.rotation_check_interval_secs, 3600);
        assert_eq!(config.audit_log_max_entries, 10_000);
    }

    #[test]
    fn test_for_testing() {
        let config = VaultConfig::for_testing();
        assert!(config.auto_unseal);
        assert!(config.passphrase.is_some());
    }

    #[test]
    fn test_file_paths() {
        let config = VaultConfig {
            data_dir: Some(PathBuf::from("/var/lib/aegis")),
            ..Default::default()
        };

        assert_eq!(
            config.vault_file_path(),
            Some(PathBuf::from("/var/lib/aegis/vault.dat"))
        );
        assert_eq!(
            config.key_file_path(),
            Some(PathBuf::from("/var/lib/aegis/vault.key"))
        );
    }

    #[test]
    fn test_no_file_paths_without_data_dir() {
        let config = VaultConfig::default();
        assert!(config.vault_file_path().is_none());
        assert!(config.key_file_path().is_none());
    }
}
