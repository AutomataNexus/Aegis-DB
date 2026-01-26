//! Aegis Secrets Management
//!
//! Secure secrets management with support for environment variables and HashiCorp Vault.
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::RwLock;

// =============================================================================
// Secrets Provider Trait
// =============================================================================

/// Trait for secrets providers.
pub trait SecretsProvider: Send + Sync {
    /// Get a secret value by key.
    fn get(&self, key: &str) -> Option<String>;

    /// Get a secret with a default fallback.
    fn get_or(&self, key: &str, default: &str) -> String {
        self.get(key).unwrap_or_else(|| default.to_string())
    }

    /// Check if a secret exists.
    fn exists(&self, key: &str) -> bool {
        self.get(key).is_some()
    }
}

// =============================================================================
// Environment Variables Provider
// =============================================================================

/// Secrets provider that reads from environment variables.
#[derive(Debug, Default)]
pub struct EnvSecretsProvider;

impl SecretsProvider for EnvSecretsProvider {
    fn get(&self, key: &str) -> Option<String> {
        std::env::var(key).ok()
    }
}

// =============================================================================
// HashiCorp Vault Provider
// =============================================================================

/// Configuration for HashiCorp Vault.
#[derive(Debug, Clone)]
pub struct VaultConfig {
    /// Vault server address (e.g., "https://vault.example.com:8200")
    pub address: String,
    /// Authentication token
    pub token: Option<String>,
    /// AppRole role_id for authentication
    pub role_id: Option<String>,
    /// AppRole secret_id for authentication
    pub secret_id: Option<String>,
    /// Kubernetes auth role (for K8s deployments)
    pub k8s_role: Option<String>,
    /// Secret engine mount path (default: "secret")
    pub mount_path: String,
    /// Secret path prefix (e.g., "aegis/prod")
    pub secret_path: String,
    /// Whether to use TLS verification
    pub tls_verify: bool,
}

impl Default for VaultConfig {
    fn default() -> Self {
        Self {
            address: std::env::var("VAULT_ADDR")
                .unwrap_or_else(|_| "http://127.0.0.1:8200".to_string()),
            token: std::env::var("VAULT_TOKEN").ok(),
            role_id: std::env::var("VAULT_ROLE_ID").ok(),
            secret_id: std::env::var("VAULT_SECRET_ID").ok(),
            k8s_role: std::env::var("VAULT_K8S_ROLE").ok(),
            mount_path: std::env::var("VAULT_MOUNT_PATH")
                .unwrap_or_else(|_| "secret".to_string()),
            secret_path: std::env::var("VAULT_SECRET_PATH")
                .unwrap_or_else(|_| "aegis".to_string()),
            tls_verify: std::env::var("VAULT_TLS_VERIFY")
                .map(|v| v != "false" && v != "0")
                .unwrap_or(true),
        }
    }
}

/// Secrets provider that reads from HashiCorp Vault.
pub struct VaultSecretsProvider {
    config: VaultConfig,
    client: reqwest::Client,
    token: RwLock<Option<String>>,
    cache: RwLock<HashMap<String, String>>,
}

impl VaultSecretsProvider {
    /// Create a new Vault secrets provider.
    pub fn new(config: VaultConfig) -> Self {
        let client = reqwest::Client::builder()
            .danger_accept_invalid_certs(!config.tls_verify)
            .build()
            .expect("Failed to create HTTP client");

        let token = config.token.clone();

        Self {
            config,
            client,
            token: RwLock::new(token),
            cache: RwLock::new(HashMap::new()),
        }
    }

    /// Create from environment variables.
    pub fn from_env() -> Option<Self> {
        let config = VaultConfig::default();

        // Only create if Vault address is configured
        if std::env::var("VAULT_ADDR").is_err() {
            return None;
        }

        Some(Self::new(config))
    }

    /// Authenticate with Vault using configured method.
    pub async fn authenticate(&self) -> Result<(), String> {
        // If we already have a token, we're good
        if self.token.read().is_some() {
            return Ok(());
        }

        // Try AppRole authentication
        if let (Some(role_id), Some(secret_id)) = (&self.config.role_id, &self.config.secret_id) {
            return self.auth_approle(role_id, secret_id).await;
        }

        // Try Kubernetes authentication
        if let Some(k8s_role) = &self.config.k8s_role {
            return self.auth_kubernetes(k8s_role).await;
        }

        Err("No authentication method configured. Set VAULT_TOKEN, VAULT_ROLE_ID/VAULT_SECRET_ID, or VAULT_K8S_ROLE".to_string())
    }

    /// Authenticate using AppRole.
    async fn auth_approle(&self, role_id: &str, secret_id: &str) -> Result<(), String> {
        let url = format!("{}/v1/auth/approle/login", self.config.address);
        let body = serde_json::json!({
            "role_id": role_id,
            "secret_id": secret_id
        });

        let response = self.client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Vault AppRole auth failed: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("Vault AppRole auth failed: HTTP {}", response.status()));
        }

        let data: serde_json::Value = response.json().await
            .map_err(|e| format!("Failed to parse Vault response: {}", e))?;

        let token = data["auth"]["client_token"]
            .as_str()
            .ok_or("No token in Vault response")?
            .to_string();

        *self.token.write() = Some(token);
        tracing::info!("Successfully authenticated with Vault using AppRole");

        Ok(())
    }

    /// Authenticate using Kubernetes service account.
    async fn auth_kubernetes(&self, role: &str) -> Result<(), String> {
        // Read the service account token
        let jwt = std::fs::read_to_string("/var/run/secrets/kubernetes.io/serviceaccount/token")
            .map_err(|e| format!("Failed to read K8s service account token: {}", e))?;

        let url = format!("{}/v1/auth/kubernetes/login", self.config.address);
        let body = serde_json::json!({
            "role": role,
            "jwt": jwt
        });

        let response = self.client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Vault K8s auth failed: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("Vault K8s auth failed: HTTP {}", response.status()));
        }

        let data: serde_json::Value = response.json().await
            .map_err(|e| format!("Failed to parse Vault response: {}", e))?;

        let token = data["auth"]["client_token"]
            .as_str()
            .ok_or("No token in Vault response")?
            .to_string();

        *self.token.write() = Some(token);
        tracing::info!("Successfully authenticated with Vault using Kubernetes");

        Ok(())
    }

    /// Read a secret from Vault (KV v2).
    pub async fn read_secret(&self, key: &str) -> Result<String, String> {
        // Check cache first
        if let Some(value) = self.cache.read().get(key) {
            return Ok(value.clone());
        }

        let token = self.token.read().clone()
            .ok_or("Not authenticated with Vault")?;

        let url = format!(
            "{}/v1/{}/data/{}/{}",
            self.config.address,
            self.config.mount_path,
            self.config.secret_path,
            key
        );

        let response = self.client
            .get(&url)
            .header("X-Vault-Token", &token)
            .send()
            .await
            .map_err(|e| format!("Vault read failed: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("Vault read failed: HTTP {}", response.status()));
        }

        let data: serde_json::Value = response.json().await
            .map_err(|e| format!("Failed to parse Vault response: {}", e))?;

        // KV v2 format: data.data.{key}
        let value = data["data"]["data"]["value"]
            .as_str()
            .ok_or_else(|| format!("Secret '{}' not found or has no 'value' field", key))?
            .to_string();

        // Cache the value
        self.cache.write().insert(key.to_string(), value.clone());

        Ok(value)
    }

    /// Read all secrets at a path and cache them.
    pub async fn load_secrets(&self) -> Result<(), String> {
        let token = self.token.read().clone()
            .ok_or("Not authenticated with Vault")?;

        let url = format!(
            "{}/v1/{}/data/{}",
            self.config.address,
            self.config.mount_path,
            self.config.secret_path
        );

        let response = self.client
            .get(&url)
            .header("X-Vault-Token", &token)
            .send()
            .await
            .map_err(|e| format!("Vault read failed: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("Vault read failed: HTTP {}", response.status()));
        }

        let data: serde_json::Value = response.json().await
            .map_err(|e| format!("Failed to parse Vault response: {}", e))?;

        // KV v2 format: data.data contains all key-value pairs
        if let Some(secrets) = data["data"]["data"].as_object() {
            let mut cache = self.cache.write();
            for (key, value) in secrets {
                if let Some(v) = value.as_str() {
                    cache.insert(key.clone(), v.to_string());
                }
            }
            tracing::info!("Loaded {} secrets from Vault", cache.len());
        }

        Ok(())
    }

    /// Get a cached secret (synchronous).
    pub fn get_cached(&self, key: &str) -> Option<String> {
        self.cache.read().get(key).cloned()
    }
}

impl SecretsProvider for VaultSecretsProvider {
    fn get(&self, key: &str) -> Option<String> {
        // First check cache
        if let Some(value) = self.get_cached(key) {
            return Some(value);
        }

        // Fall back to environment variable
        std::env::var(key).ok()
    }
}

// =============================================================================
// Composite Secrets Manager
// =============================================================================

/// Manages secrets from multiple providers with fallback chain.
pub struct SecretsManager {
    providers: Vec<Arc<dyn SecretsProvider>>,
}

impl SecretsManager {
    /// Create a new secrets manager with the given providers.
    /// Providers are checked in order; first match wins.
    pub fn new(providers: Vec<Arc<dyn SecretsProvider>>) -> Self {
        Self { providers }
    }

    /// Create a secrets manager with environment variables only.
    pub fn env_only() -> Self {
        Self {
            providers: vec![Arc::new(EnvSecretsProvider)],
        }
    }

    /// Create a secrets manager that tries Vault first, then environment variables.
    pub fn with_vault_fallback(vault: VaultSecretsProvider) -> Self {
        Self {
            providers: vec![
                Arc::new(vault),
                Arc::new(EnvSecretsProvider),
            ],
        }
    }
}

impl SecretsProvider for SecretsManager {
    fn get(&self, key: &str) -> Option<String> {
        for provider in &self.providers {
            if let Some(value) = provider.get(key) {
                return Some(value);
            }
        }
        None
    }
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Initialize secrets manager from environment configuration.
/// Returns a Vault-backed manager if VAULT_ADDR is set, otherwise env-only.
pub async fn init_secrets_manager() -> SecretsManager {
    // Check if Vault is configured
    if let Some(vault) = VaultSecretsProvider::from_env() {
        // Try to authenticate
        if let Err(e) = vault.authenticate().await {
            tracing::warn!("Vault authentication failed: {}. Falling back to environment variables.", e);
            return SecretsManager::env_only();
        }

        // Try to load secrets
        if let Err(e) = vault.load_secrets().await {
            tracing::warn!("Failed to load secrets from Vault: {}. Will fetch on demand.", e);
        }

        tracing::info!("Secrets manager initialized with Vault backend");
        SecretsManager::with_vault_fallback(vault)
    } else {
        tracing::info!("Secrets manager initialized with environment variables only");
        SecretsManager::env_only()
    }
}

/// Standard secret keys used by Aegis.
pub mod keys {
    /// Admin username for initial setup
    pub const ADMIN_USERNAME: &str = "AEGIS_ADMIN_USERNAME";
    /// Admin password for initial setup
    pub const ADMIN_PASSWORD: &str = "AEGIS_ADMIN_PASSWORD";
    /// Admin email for initial setup
    pub const ADMIN_EMAIL: &str = "AEGIS_ADMIN_EMAIL";
    /// TLS certificate path
    pub const TLS_CERT_PATH: &str = "AEGIS_TLS_CERT";
    /// TLS private key path
    pub const TLS_KEY_PATH: &str = "AEGIS_TLS_KEY";
    /// Cluster TLS CA certificate path
    pub const CLUSTER_CA_CERT_PATH: &str = "AEGIS_CLUSTER_CA_CERT";
    /// Cluster TLS client certificate path (for mTLS)
    pub const CLUSTER_CLIENT_CERT_PATH: &str = "AEGIS_CLUSTER_CLIENT_CERT";
    /// Cluster TLS client key path (for mTLS)
    pub const CLUSTER_CLIENT_KEY_PATH: &str = "AEGIS_CLUSTER_CLIENT_KEY";
    /// Database encryption key
    pub const ENCRYPTION_KEY: &str = "AEGIS_ENCRYPTION_KEY";
    /// JWT signing secret
    pub const JWT_SECRET: &str = "AEGIS_JWT_SECRET";
    /// LDAP bind password
    pub const LDAP_BIND_PASSWORD: &str = "AEGIS_LDAP_BIND_PASSWORD";
    /// OAuth2 client secret
    pub const OAUTH_CLIENT_SECRET: &str = "AEGIS_OAUTH_CLIENT_SECRET";
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_env_provider() {
        std::env::set_var("TEST_SECRET_KEY", "test_value");
        let provider = EnvSecretsProvider;
        assert_eq!(provider.get("TEST_SECRET_KEY"), Some("test_value".to_string()));
        assert_eq!(provider.get("NONEXISTENT_KEY"), None);
        std::env::remove_var("TEST_SECRET_KEY");
    }

    #[test]
    fn test_secrets_manager_fallback() {
        std::env::set_var("TEST_FALLBACK_KEY", "fallback_value");
        let manager = SecretsManager::env_only();
        assert_eq!(manager.get("TEST_FALLBACK_KEY"), Some("fallback_value".to_string()));
        std::env::remove_var("TEST_FALLBACK_KEY");
    }

    #[test]
    fn test_get_or_default() {
        let provider = EnvSecretsProvider;
        assert_eq!(provider.get_or("NONEXISTENT", "default"), "default");
    }

    #[test]
    fn test_vault_config_from_env() {
        // Clear any existing vars
        std::env::remove_var("VAULT_ADDR");

        let config = VaultConfig::default();
        assert_eq!(config.address, "http://127.0.0.1:8200");
        assert_eq!(config.mount_path, "secret");
        assert_eq!(config.secret_path, "aegis");
        assert!(config.tls_verify);
    }
}
