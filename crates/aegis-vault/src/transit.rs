use std::collections::HashMap;
use std::path::PathBuf;

use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Nonce};
use parking_lot::RwLock;
use rand::RngCore;
use serde::{Deserialize, Serialize};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use crate::error::VaultError;
use crate::master_key::SealManager;

const NONCE_LEN: usize = 12;

/// A named encryption key used by the transit engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransitKey {
    pub name: String,
    #[serde(with = "key_bytes_serde")]
    pub key: [u8; 32],
    pub version: u32,
    pub created_at: u64,
}

/// Serde helper for fixed-size key arrays (base64 encoded).
mod key_bytes_serde {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer>(key: &[u8; 32], serializer: S) -> Result<S::Ok, S::Error> {
        use base64::Engine;
        let encoded = base64::engine::general_purpose::STANDARD.encode(key);
        encoded.serialize(serializer)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<[u8; 32], D::Error> {
        use base64::Engine;
        let encoded = String::deserialize(deserializer)?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(&encoded)
            .map_err(serde::de::Error::custom)?;
        let arr: [u8; 32] = bytes
            .try_into()
            .map_err(|_| serde::de::Error::custom("expected 32 bytes for transit key"))?;
        Ok(arr)
    }
}

#[derive(Serialize, Deserialize)]
struct TransitKeysData {
    keys: HashMap<String, TransitKey>,
}

/// Encryption-as-a-service engine. Provides named keys for encrypt/decrypt
/// without exposing the raw key material to callers.
pub struct TransitEngine {
    keys: RwLock<HashMap<String, TransitKey>>,
}

impl TransitEngine {
    pub fn new() -> Self {
        Self {
            keys: RwLock::new(HashMap::new()),
        }
    }

    /// Create a new named transit key.
    pub fn create_key(&self, name: &str) -> Result<(), VaultError> {
        let mut keys = self.keys.write();
        if keys.contains_key(name) {
            return Err(VaultError::AlreadyExists(format!("transit key '{}'", name)));
        }

        let mut key_bytes = [0u8; 32];
        OsRng.fill_bytes(&mut key_bytes);

        keys.insert(
            name.to_string(),
            TransitKey {
                name: name.to_string(),
                key: key_bytes,
                version: 1,
                created_at: chrono::Utc::now().timestamp() as u64,
            },
        );

        tracing::info!("Transit key '{}' created", name);
        Ok(())
    }

    /// Encrypt plaintext with a named key. Returns nonce + ciphertext.
    pub fn encrypt(&self, key_name: &str, plaintext: &[u8]) -> Result<Vec<u8>, VaultError> {
        let keys = self.keys.read();
        let transit_key = keys
            .get(key_name)
            .ok_or_else(|| VaultError::SecretNotFound(format!("transit key '{}'", key_name)))?;

        let cipher = Aes256Gcm::new_from_slice(&transit_key.key)
            .map_err(|e| VaultError::Encryption(e.to_string()))?;

        let mut nonce_bytes = [0u8; NONCE_LEN];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher
            .encrypt(nonce, plaintext)
            .map_err(|e| VaultError::Encryption(e.to_string()))?;

        let mut result = Vec::with_capacity(NONCE_LEN + ciphertext.len());
        result.extend_from_slice(&nonce_bytes);
        result.extend_from_slice(&ciphertext);
        Ok(result)
    }

    /// Decrypt ciphertext with a named key. Input: nonce (12) + ciphertext.
    pub fn decrypt(&self, key_name: &str, data: &[u8]) -> Result<Vec<u8>, VaultError> {
        if data.len() < NONCE_LEN + 1 {
            return Err(VaultError::Encryption("ciphertext too short".into()));
        }

        let keys = self.keys.read();
        let transit_key = keys
            .get(key_name)
            .ok_or_else(|| VaultError::SecretNotFound(format!("transit key '{}'", key_name)))?;

        let cipher = Aes256Gcm::new_from_slice(&transit_key.key)
            .map_err(|e| VaultError::Encryption(e.to_string()))?;

        let nonce = Nonce::from_slice(&data[..NONCE_LEN]);
        let ciphertext = &data[NONCE_LEN..];

        cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| VaultError::Encryption(e.to_string()))
    }

    /// List all transit key names.
    pub fn list_keys(&self) -> Vec<String> {
        self.keys.read().keys().cloned().collect()
    }

    /// Delete a transit key by name.
    pub fn delete_key(&self, name: &str) -> Result<(), VaultError> {
        let mut keys = self.keys.write();
        if keys.remove(name).is_none() {
            return Err(VaultError::SecretNotFound(format!(
                "transit key '{}'",
                name
            )));
        }
        tracing::info!("Transit key '{}' deleted", name);
        Ok(())
    }

    /// Get the number of keys.
    pub fn key_count(&self) -> usize {
        self.keys.read().len()
    }

    /// Persist transit keys to disk, encrypted with the master key.
    pub fn save_to_disk(
        &self,
        seal_manager: &SealManager,
        path: &PathBuf,
    ) -> Result<(), VaultError> {
        let keys = self.keys.read().clone();
        let data = TransitKeysData { keys };

        let json = serde_json::to_vec(&data)
            .map_err(|e| VaultError::Other(format!("transit key serialization failed: {e}")))?;

        let encrypted = seal_manager.encrypt(&json)?;

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let tmp_path = path.with_extension("tmp");
        std::fs::write(&tmp_path, &encrypted)?;

        #[cfg(unix)]
        std::fs::set_permissions(&tmp_path, std::fs::Permissions::from_mode(0o600))?;

        std::fs::rename(&tmp_path, path)?;

        tracing::debug!("Transit keys saved to disk ({} keys)", data.keys.len());
        Ok(())
    }

    /// Load transit keys from disk, decrypting with the master key.
    pub fn load_from_disk(
        &self,
        seal_manager: &SealManager,
        path: &PathBuf,
    ) -> Result<(), VaultError> {
        if !path.exists() {
            tracing::debug!("No transit keys file found, starting fresh");
            return Ok(());
        }

        let encrypted = std::fs::read(path)?;
        if encrypted.len() < 28 {
            return Err(VaultError::Other(
                "transit keys file is too small or corrupt".into(),
            ));
        }

        let json = seal_manager.decrypt(&encrypted)?;

        let json_str = std::str::from_utf8(&json).map_err(|_| {
            VaultError::Other("decrypted transit keys data is not valid UTF-8".into())
        })?;

        let data: TransitKeysData = serde_json::from_str(json_str).map_err(|e| {
            tracing::error!("Transit key deserialization failed: {e}");
            VaultError::Other("transit key deserialization failed — file may be corrupt".into())
        })?;

        *self.keys.write() = data.keys;

        tracing::info!(
            "Transit keys loaded from disk ({} keys)",
            self.keys.read().len()
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_and_list_keys() {
        let engine = TransitEngine::new();
        engine.create_key("key1").unwrap();
        engine.create_key("key2").unwrap();

        let mut keys = engine.list_keys();
        keys.sort();
        assert_eq!(keys, vec!["key1", "key2"]);
        assert_eq!(engine.key_count(), 2);
    }

    #[test]
    fn test_create_duplicate_key() {
        let engine = TransitEngine::new();
        engine.create_key("mykey").unwrap();
        let result = engine.create_key("mykey");
        assert!(matches!(result, Err(VaultError::AlreadyExists(_))));
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let engine = TransitEngine::new();
        engine.create_key("test_key").unwrap();

        let plaintext = b"sensitive data here";
        let ciphertext = engine.encrypt("test_key", plaintext).unwrap();

        // Ciphertext should differ from plaintext
        assert_ne!(&ciphertext[NONCE_LEN..], plaintext.as_slice());

        let decrypted = engine.decrypt("test_key", &ciphertext).unwrap();
        assert_eq!(plaintext.as_slice(), &decrypted);
    }

    #[test]
    fn test_encrypt_nonexistent_key() {
        let engine = TransitEngine::new();
        let result = engine.encrypt("no_such_key", b"data");
        assert!(matches!(result, Err(VaultError::SecretNotFound(_))));
    }

    #[test]
    fn test_delete_key() {
        let engine = TransitEngine::new();
        engine.create_key("ephemeral").unwrap();
        assert_eq!(engine.key_count(), 1);

        engine.delete_key("ephemeral").unwrap();
        assert_eq!(engine.key_count(), 0);

        let result = engine.delete_key("ephemeral");
        assert!(matches!(result, Err(VaultError::SecretNotFound(_))));
    }

    #[test]
    fn test_decrypt_with_wrong_key() {
        let engine = TransitEngine::new();
        engine.create_key("key_a").unwrap();
        engine.create_key("key_b").unwrap();

        let ciphertext = engine.encrypt("key_a", b"secret").unwrap();
        let result = engine.decrypt("key_b", &ciphertext);
        assert!(matches!(result, Err(VaultError::Encryption(_))));
    }
}
