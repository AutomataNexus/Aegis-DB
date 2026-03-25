use std::sync::Arc;

use crate::AegisVault;

/// Provider that wraps AegisVault for use by other components.
/// Provides a simplified interface for secret retrieval.
pub struct AegisVaultProvider {
    vault: Arc<AegisVault>,
    component: String,
}

impl AegisVaultProvider {
    /// Create a new provider for a specific component.
    pub fn new(vault: Arc<AegisVault>, component: &str) -> Self {
        Self {
            vault,
            component: component.to_string(),
        }
    }

    /// Get a secret value by key. Returns None if the secret doesn't exist
    /// or the vault is sealed.
    pub fn get(&self, key: &str) -> Option<String> {
        self.vault.get(key, &self.component).ok()
    }

    /// Set a secret value by key.
    pub fn set(&self, key: &str, value: &str) -> Result<(), crate::error::VaultError> {
        self.vault.set(key, value, &self.component)
    }

    /// Delete a secret by key.
    pub fn delete(&self, key: &str) -> Result<(), crate::error::VaultError> {
        self.vault.delete(key, &self.component)
    }

    /// List secrets matching a prefix.
    pub fn list(&self, prefix: &str) -> Vec<String> {
        self.vault.list(prefix, &self.component).unwrap_or_default()
    }

    /// Check if the vault is sealed.
    pub fn is_sealed(&self) -> bool {
        self.vault.is_sealed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::VaultConfig;

    #[tokio::test]
    async fn test_provider_get_set() {
        let vault = AegisVault::init(VaultConfig::for_testing()).await.unwrap();
        let vault = Arc::new(vault);
        let provider = AegisVaultProvider::new(Arc::clone(&vault), "test_component");

        assert!(provider.get("nonexistent").is_none());

        provider.set("api_key", "abc123").unwrap();
        assert_eq!(provider.get("api_key"), Some("abc123".to_string()));
    }

    #[tokio::test]
    async fn test_provider_delete() {
        let vault = AegisVault::init(VaultConfig::for_testing()).await.unwrap();
        let vault = Arc::new(vault);
        let provider = AegisVaultProvider::new(Arc::clone(&vault), "test");

        provider.set("temp", "value").unwrap();
        assert!(provider.get("temp").is_some());

        provider.delete("temp").unwrap();
        assert!(provider.get("temp").is_none());
    }

    #[tokio::test]
    async fn test_provider_list() {
        let vault = AegisVault::init(VaultConfig::for_testing()).await.unwrap();
        let vault = Arc::new(vault);
        let provider = AegisVaultProvider::new(Arc::clone(&vault), "test");

        provider.set("db/host", "localhost").unwrap();
        provider.set("db/port", "5432").unwrap();
        provider.set("api/key", "xyz").unwrap();

        let mut db_keys = provider.list("db/");
        db_keys.sort();
        assert_eq!(db_keys, vec!["db/host", "db/port"]);
    }

    #[tokio::test]
    async fn test_provider_sealed_state() {
        let vault = AegisVault::init(VaultConfig::for_testing()).await.unwrap();
        let vault = Arc::new(vault);
        let provider = AegisVaultProvider::new(Arc::clone(&vault), "test");

        assert!(!provider.is_sealed());
    }
}
