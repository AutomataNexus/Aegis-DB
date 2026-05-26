use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Nonce};
use parking_lot::RwLock;
use rand::RngCore;
use ring::pbkdf2;
use zeroize::Zeroize;

use crate::error::VaultError;

const PBKDF2_ITERATIONS: u32 = 100_000;
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;

static PBKDF2_ALG: pbkdf2::Algorithm = pbkdf2::PBKDF2_HMAC_SHA256;

/// A 256-bit master encryption key that is zeroized on drop.
pub struct MasterKey {
    key: [u8; 32],
}

impl MasterKey {
    pub fn new(key: [u8; 32]) -> Self {
        Self { key }
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.key
    }
}

impl Drop for MasterKey {
    fn drop(&mut self) {
        self.key.zeroize();
    }
}

impl Clone for MasterKey {
    fn clone(&self) -> Self {
        Self { key: self.key }
    }
}

/// Seal status of the vault.
#[derive(Debug)]
pub enum SealStatus {
    Sealed,
    Unsealed { since: std::time::Instant },
}

/// Manages the seal/unseal lifecycle and encryption operations.
pub struct SealManager {
    status: RwLock<SealStatus>,
    master_key: RwLock<Option<MasterKey>>,
    /// The encrypted master key blob (salt + nonce + ciphertext), persisted so we can unseal later.
    encrypted_key_blob: RwLock<Option<Vec<u8>>>,
}

impl Default for SealManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SealManager {
    pub fn new() -> Self {
        Self {
            status: RwLock::new(SealStatus::Sealed),
            master_key: RwLock::new(None),
            encrypted_key_blob: RwLock::new(None),
        }
    }

    /// Check whether the vault is currently sealed.
    pub fn is_sealed(&self) -> bool {
        matches!(*self.status.read(), SealStatus::Sealed)
    }

    /// Get uptime in seconds since unseal, if unsealed.
    pub fn uptime_secs(&self) -> Option<u64> {
        match &*self.status.read() {
            SealStatus::Unsealed { since } => Some(since.elapsed().as_secs()),
            SealStatus::Sealed => None,
        }
    }

    /// Unseal the vault using a passphrase. The encrypted key blob must have been
    /// previously set (from disk or from initial creation).
    pub fn unseal(&self, passphrase: &str) -> Result<(), VaultError> {
        let blob = self
            .encrypted_key_blob
            .read()
            .clone()
            .ok_or_else(|| VaultError::Other("no encrypted key blob available".into()))?;

        let key = decrypt_master_key(&blob, passphrase)?;

        *self.master_key.write() = Some(key);
        *self.status.write() = SealStatus::Unsealed {
            since: std::time::Instant::now(),
        };

        tracing::info!("Vault unsealed successfully");
        Ok(())
    }

    /// Seal the vault, zeroizing the master key from memory.
    pub fn seal(&self) -> Result<(), VaultError> {
        *self.master_key.write() = None;
        *self.status.write() = SealStatus::Sealed;
        tracing::info!("Vault sealed");
        Ok(())
    }

    /// Auto-unseal for first run: generates a new master key and stores the
    /// encrypted blob. If a passphrase is provided, the key is encrypted with it;
    /// otherwise a random passphrase is generated (logged as warning).
    pub fn auto_unseal(&self, passphrase: Option<&str>) -> Result<(), VaultError> {
        let key = generate_key();

        let actual_passphrase = match passphrase {
            Some(p) => p.to_string(),
            None => {
                let p = generate_random_passphrase();
                tracing::warn!(
                    "Vault auto-unsealed with generated passphrase. \
                     Set AEGIS_VAULT_PASSPHRASE for production use."
                );
                p
            }
        };

        let encrypted = encrypt_master_key(&key, &actual_passphrase);
        *self.encrypted_key_blob.write() = Some(encrypted);
        *self.master_key.write() = Some(key);
        *self.status.write() = SealStatus::Unsealed {
            since: std::time::Instant::now(),
        };

        tracing::info!("Vault auto-unsealed");
        Ok(())
    }

    /// Set the encrypted key blob (loaded from disk).
    pub fn set_encrypted_key_blob(&self, blob: Vec<u8>) {
        *self.encrypted_key_blob.write() = Some(blob);
    }

    /// Get the encrypted key blob for persistence.
    pub fn get_encrypted_key_blob(&self) -> Option<Vec<u8>> {
        self.encrypted_key_blob.read().clone()
    }

    /// Encrypt plaintext using the master key. Returns nonce + ciphertext.
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>, VaultError> {
        let guard = self.master_key.read();
        let key = guard.as_ref().ok_or(VaultError::Sealed)?;

        encrypt_with_key(key.as_bytes(), plaintext)
    }

    /// Decrypt ciphertext (nonce + ciphertext) using the master key.
    pub fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>, VaultError> {
        let guard = self.master_key.read();
        let key = guard.as_ref().ok_or(VaultError::Sealed)?;

        decrypt_with_key(key.as_bytes(), ciphertext)
    }
}

/// Derive a master key from a passphrase and salt using PBKDF2-HMAC-SHA256.
pub fn derive_key(passphrase: &str, salt: &[u8]) -> MasterKey {
    let mut key_bytes = [0u8; 32];
    pbkdf2::derive(
        PBKDF2_ALG,
        std::num::NonZeroU32::new(PBKDF2_ITERATIONS).expect("iterations must be non-zero"),
        salt,
        passphrase.as_bytes(),
        &mut key_bytes,
    );
    MasterKey::new(key_bytes)
}

/// Generate a random 256-bit master key.
pub fn generate_key() -> MasterKey {
    let mut key_bytes = [0u8; 32];
    OsRng.fill_bytes(&mut key_bytes);
    MasterKey::new(key_bytes)
}

/// Encrypt the master key with a passphrase. Returns: salt (16) + nonce (12) + ciphertext.
pub fn encrypt_master_key(key: &MasterKey, passphrase: &str) -> Vec<u8> {
    let mut salt = [0u8; SALT_LEN];
    OsRng.fill_bytes(&mut salt);

    let derived = derive_key(passphrase, &salt);
    let cipher =
        Aes256Gcm::new_from_slice(derived.as_bytes()).expect("AES-256-GCM key must be 32 bytes");

    let mut nonce_bytes = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, key.as_bytes().as_ref())
        .expect("AES-256-GCM encryption should not fail");

    let mut result = Vec::with_capacity(SALT_LEN + NONCE_LEN + ciphertext.len());
    result.extend_from_slice(&salt);
    result.extend_from_slice(&nonce_bytes);
    result.extend_from_slice(&ciphertext);
    result
}

/// Decrypt the master key from the encrypted blob using a passphrase.
pub fn decrypt_master_key(encrypted: &[u8], passphrase: &str) -> Result<MasterKey, VaultError> {
    if encrypted.len() < SALT_LEN + NONCE_LEN + 1 {
        return Err(VaultError::Encryption("encrypted blob too short".into()));
    }

    let salt = &encrypted[..SALT_LEN];
    let nonce_bytes = &encrypted[SALT_LEN..SALT_LEN + NONCE_LEN];
    let ciphertext = &encrypted[SALT_LEN + NONCE_LEN..];

    let derived = derive_key(passphrase, salt);
    let cipher = Aes256Gcm::new_from_slice(derived.as_bytes())
        .map_err(|e| VaultError::Encryption(e.to_string()))?;

    let nonce = Nonce::from_slice(nonce_bytes);
    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| VaultError::InvalidPassphrase)?;

    if plaintext.len() != 32 {
        return Err(VaultError::Encryption(
            "decrypted key has wrong length".into(),
        ));
    }

    let mut key_bytes = [0u8; 32];
    key_bytes.copy_from_slice(&plaintext);
    Ok(MasterKey::new(key_bytes))
}

/// Encrypt arbitrary data with a 32-byte key. Returns nonce (12) + ciphertext.
pub fn encrypt_with_key(key: &[u8; 32], plaintext: &[u8]) -> Result<Vec<u8>, VaultError> {
    let cipher =
        Aes256Gcm::new_from_slice(key).map_err(|e| VaultError::Encryption(e.to_string()))?;

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

/// Decrypt data encrypted with encrypt_with_key. Input: nonce (12) + ciphertext.
pub fn decrypt_with_key(key: &[u8; 32], data: &[u8]) -> Result<Vec<u8>, VaultError> {
    if data.len() < NONCE_LEN + 1 {
        return Err(VaultError::Encryption("ciphertext too short".into()));
    }

    let nonce_bytes = &data[..NONCE_LEN];
    let ciphertext = &data[NONCE_LEN..];

    let cipher =
        Aes256Gcm::new_from_slice(key).map_err(|e| VaultError::Encryption(e.to_string()))?;
    let nonce = Nonce::from_slice(nonce_bytes);

    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| VaultError::Encryption(e.to_string()))
}

fn generate_random_passphrase() -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    hex::encode(bytes)
}

/// Simple hex encoding (no external dep needed).
mod hex {
    pub fn encode(bytes: [u8; 32]) -> String {
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_derive_key_deterministic() {
        let salt = b"test_salt_16byte";
        let k1 = derive_key("my_passphrase", salt);
        let k2 = derive_key("my_passphrase", salt);
        assert_eq!(k1.as_bytes(), k2.as_bytes());
    }

    #[test]
    fn test_derive_key_different_passphrases() {
        let salt = b"test_salt_16byte";
        let k1 = derive_key("passphrase_a", salt);
        let k2 = derive_key("passphrase_b", salt);
        assert_ne!(k1.as_bytes(), k2.as_bytes());
    }

    #[test]
    fn test_generate_key_random() {
        let k1 = generate_key();
        let k2 = generate_key();
        assert_ne!(k1.as_bytes(), k2.as_bytes());
    }

    #[test]
    fn test_encrypt_decrypt_master_key() {
        let key = generate_key();
        let passphrase = "super_secret";
        let encrypted = encrypt_master_key(&key, passphrase);
        let decrypted = decrypt_master_key(&encrypted, passphrase).unwrap();
        assert_eq!(key.as_bytes(), decrypted.as_bytes());
    }

    #[test]
    fn test_decrypt_master_key_wrong_passphrase() {
        let key = generate_key();
        let encrypted = encrypt_master_key(&key, "correct");
        let result = decrypt_master_key(&encrypted, "wrong");
        assert!(matches!(result, Err(VaultError::InvalidPassphrase)));
    }

    #[test]
    fn test_seal_manager_lifecycle() {
        let sm = SealManager::new();
        assert!(sm.is_sealed());

        sm.auto_unseal(Some("test_pass")).unwrap();
        assert!(!sm.is_sealed());

        // Encrypt/decrypt round-trip
        let plaintext = b"hello world";
        let ciphertext = sm.encrypt(plaintext).unwrap();
        let decrypted = sm.decrypt(&ciphertext).unwrap();
        assert_eq!(plaintext.as_slice(), &decrypted);

        // Seal
        sm.seal().unwrap();
        assert!(sm.is_sealed());

        // Encrypt should fail when sealed
        assert!(sm.encrypt(plaintext).is_err());
    }

    #[test]
    fn test_seal_unseal_with_passphrase() {
        let sm = SealManager::new();
        sm.auto_unseal(Some("my_pass")).unwrap();

        let plaintext = b"secret data";
        let ciphertext = sm.encrypt(plaintext).unwrap();

        sm.seal().unwrap();
        assert!(sm.decrypt(&ciphertext).is_err());

        sm.unseal("my_pass").unwrap();
        let decrypted = sm.decrypt(&ciphertext).unwrap();
        assert_eq!(plaintext.as_slice(), &decrypted);
    }

    #[test]
    fn test_encrypt_decrypt_with_key() {
        let key = generate_key();
        let plaintext = b"test data for encryption";
        let ciphertext = encrypt_with_key(key.as_bytes(), plaintext).unwrap();
        let decrypted = decrypt_with_key(key.as_bytes(), &ciphertext).unwrap();
        assert_eq!(plaintext.as_slice(), &decrypted);
    }

    #[test]
    fn test_uptime() {
        let sm = SealManager::new();
        assert!(sm.uptime_secs().is_none());

        sm.auto_unseal(Some("pass")).unwrap();
        assert!(sm.uptime_secs().is_some());
    }
}
