use serde::{Deserialize, Serialize};

/// A secret stored in the vault, with versioned encrypted values.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Secret {
    pub key: String,
    pub versions: Vec<SecretVersion>,
    pub metadata: SecretMetadata,
}

/// A single version of a secret's encrypted value.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretVersion {
    pub version: u32,
    pub encrypted_value: Vec<u8>,
    pub created_at: u64,
    pub created_by: String,
    pub expires_at: Option<u64>,
}

/// Metadata about a secret.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretMetadata {
    pub key: String,
    pub current_version: u32,
    pub created_at: u64,
    pub updated_at: u64,
    pub access_policy: Option<String>,
    pub rotation_ttl_secs: Option<u64>,
    pub max_versions: u32,
}

impl SecretMetadata {
    pub fn new(key: String) -> Self {
        let now = chrono::Utc::now().timestamp() as u64;
        Self {
            key,
            current_version: 0,
            created_at: now,
            updated_at: now,
            access_policy: None,
            rotation_ttl_secs: None,
            max_versions: 10,
        }
    }
}

impl Secret {
    /// Create a new secret with no versions.
    pub fn new(key: String) -> Self {
        let metadata = SecretMetadata::new(key.clone());
        Self {
            key,
            versions: Vec::new(),
            metadata,
        }
    }

    /// Get the current (latest) version.
    pub fn current_version(&self) -> Option<&SecretVersion> {
        self.versions.last()
    }

    /// Get a specific version.
    pub fn get_version(&self, version: u32) -> Option<&SecretVersion> {
        self.versions.iter().find(|v| v.version == version)
    }

    /// Add a new version and prune old ones beyond max_versions.
    pub fn add_version(&mut self, encrypted_value: Vec<u8>, created_by: String) {
        let next_version = self.metadata.current_version + 1;
        let now = chrono::Utc::now().timestamp() as u64;

        self.versions.push(SecretVersion {
            version: next_version,
            encrypted_value,
            created_at: now,
            created_by,
            expires_at: self.metadata.rotation_ttl_secs.map(|ttl| now + ttl),
        });

        self.metadata.current_version = next_version;
        self.metadata.updated_at = now;

        // Prune old versions
        let max = self.metadata.max_versions as usize;
        if self.versions.len() > max {
            let drain_count = self.versions.len() - max;
            self.versions.drain(0..drain_count);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_secret_creation() {
        let secret = Secret::new("db_password".into());
        assert_eq!(secret.key, "db_password");
        assert!(secret.versions.is_empty());
        assert_eq!(secret.metadata.current_version, 0);
        assert_eq!(secret.metadata.max_versions, 10);
    }

    #[test]
    fn test_add_version() {
        let mut secret = Secret::new("api_key".into());
        secret.add_version(vec![1, 2, 3], "admin".into());
        assert_eq!(secret.metadata.current_version, 1);
        assert_eq!(secret.versions.len(), 1);

        secret.add_version(vec![4, 5, 6], "admin".into());
        assert_eq!(secret.metadata.current_version, 2);
        assert_eq!(secret.versions.len(), 2);

        let v1 = secret.get_version(1).unwrap();
        assert_eq!(v1.encrypted_value, vec![1, 2, 3]);
    }

    #[test]
    fn test_version_pruning() {
        let mut secret = Secret::new("key".into());
        secret.metadata.max_versions = 3;

        for i in 0..5 {
            secret.add_version(vec![i as u8], "admin".into());
        }

        assert_eq!(secret.versions.len(), 3);
        // Oldest versions should have been pruned
        assert!(secret.get_version(1).is_none());
        assert!(secret.get_version(2).is_none());
        assert!(secret.get_version(3).is_some());
        assert!(secret.get_version(5).is_some());
    }

    #[test]
    fn test_current_version() {
        let mut secret = Secret::new("key".into());
        assert!(secret.current_version().is_none());

        secret.add_version(vec![1], "admin".into());
        secret.add_version(vec![2], "admin".into());
        let current = secret.current_version().unwrap();
        assert_eq!(current.version, 2);
        assert_eq!(current.encrypted_value, vec![2]);
    }
}
