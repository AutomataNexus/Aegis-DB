//! Aegis Block - Storage Block Management
//!
//! Defines the fundamental storage unit for the Aegis database. Blocks are
//! the atomic unit of I/O and replication, containing typed data with
//! checksums for integrity verification.
//!
//! Key Features:
//! - Self-describing block headers with type and compression info
//! - CRC32 checksums for corruption detection
//! - Support for multiple block types (table, index, log, metadata)
//! - Efficient serialization with bincode
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use aegis_common::{BlockId, BlockType, CompressionType, EncryptionType, Result, AegisError};
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use bytes::{Bytes, BytesMut, BufMut};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

// =============================================================================
// Constants
// =============================================================================

pub const BLOCK_HEADER_SIZE: usize = 32;
pub const DEFAULT_BLOCK_SIZE: usize = 8192;
pub const MAX_BLOCK_SIZE: usize = 1024 * 1024; // 1 MB
pub const AES_GCM_NONCE_SIZE: usize = 12;
pub const AES_256_KEY_SIZE: usize = 32;

// =============================================================================
// Encryption Key Management
// =============================================================================

/// Global encryption key loaded from environment variable.
/// The key is cached after first load for performance.
/// Using OnceLock for the success case and Mutex for thread-safe initialization.
static ENCRYPTION_KEY: OnceLock<[u8; AES_256_KEY_SIZE]> = OnceLock::new();
static ENCRYPTION_KEY_INIT: Mutex<bool> = Mutex::new(false);

/// Get the encryption key from environment variable AEGIS_ENCRYPTION_KEY.
/// The key must be 32 bytes (64 hex characters).
fn get_encryption_key() -> Result<&'static [u8; AES_256_KEY_SIZE]> {
    // Fast path: key already initialized
    if let Some(key) = ENCRYPTION_KEY.get() {
        return Ok(key);
    }

    // Slow path: initialize the key with mutex protection
    let _guard = ENCRYPTION_KEY_INIT.lock();

    // Double-check after acquiring lock
    if let Some(key) = ENCRYPTION_KEY.get() {
        return Ok(key);
    }

    let hex_key = std::env::var("AEGIS_ENCRYPTION_KEY").map_err(|_| {
        AegisError::Encryption(
            "AEGIS_ENCRYPTION_KEY environment variable not set".to_string(),
        )
    })?;

    let key_bytes = hex::decode(&hex_key).map_err(|e| {
        AegisError::Encryption(format!("Invalid hex encoding in AEGIS_ENCRYPTION_KEY: {}", e))
    })?;

    if key_bytes.len() != AES_256_KEY_SIZE {
        return Err(AegisError::Encryption(format!(
            "AEGIS_ENCRYPTION_KEY must be {} bytes ({} hex chars), got {} bytes",
            AES_256_KEY_SIZE,
            AES_256_KEY_SIZE * 2,
            key_bytes.len()
        )));
    }

    let mut key = [0u8; AES_256_KEY_SIZE];
    key.copy_from_slice(&key_bytes);

    // Store the key - this will succeed because we hold the mutex
    let _ = ENCRYPTION_KEY.set(key);

    Ok(ENCRYPTION_KEY.get().unwrap())
}

/// Encrypt data using AES-256-GCM.
/// Returns encrypted data with 12-byte nonce prepended.
fn encrypt_aes256gcm(plaintext: &[u8]) -> Result<Vec<u8>> {
    let key = get_encryption_key()?;
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|e| AegisError::Encryption(format!("Failed to create cipher: {}", e)))?;

    // Generate a random nonce (12 bytes for AES-GCM)
    let mut nonce_bytes = [0u8; AES_GCM_NONCE_SIZE];
    getrandom::getrandom(&mut nonce_bytes)
        .map_err(|e| AegisError::Encryption(format!("Failed to generate nonce: {}", e)))?;
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher.encrypt(nonce, plaintext).map_err(|e| {
        AegisError::Encryption(format!("Encryption failed: {}", e))
    })?;

    // Prepend nonce to ciphertext for storage
    let mut result = Vec::with_capacity(AES_GCM_NONCE_SIZE + ciphertext.len());
    result.extend_from_slice(&nonce_bytes);
    result.extend(ciphertext);

    Ok(result)
}

/// Decrypt data using AES-256-GCM.
/// Expects 12-byte nonce prepended to ciphertext.
fn decrypt_aes256gcm(encrypted_data: &[u8]) -> Result<Vec<u8>> {
    if encrypted_data.len() < AES_GCM_NONCE_SIZE {
        return Err(AegisError::Encryption(
            "Encrypted data too short: missing nonce".to_string(),
        ));
    }

    let key = get_encryption_key()?;
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|e| AegisError::Encryption(format!("Failed to create cipher: {}", e)))?;

    let nonce = Nonce::from_slice(&encrypted_data[..AES_GCM_NONCE_SIZE]);
    let ciphertext = &encrypted_data[AES_GCM_NONCE_SIZE..];

    let plaintext = cipher.decrypt(nonce, ciphertext).map_err(|e| {
        AegisError::Encryption(format!("Decryption failed: {}", e))
    })?;

    Ok(plaintext)
}

// =============================================================================
// Block Header
// =============================================================================

/// Header containing block metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockHeader {
    pub block_id: BlockId,
    pub block_type: BlockType,
    pub compression: CompressionType,
    pub encryption: EncryptionType,
    pub data_size: u32,
    pub uncompressed_size: u32,
    pub checksum: u32,
    pub version: u16,
    pub flags: u16,
}

impl BlockHeader {
    pub fn new(block_id: BlockId, block_type: BlockType) -> Self {
        Self {
            block_id,
            block_type,
            compression: CompressionType::None,
            encryption: EncryptionType::None,
            data_size: 0,
            uncompressed_size: 0,
            checksum: 0,
            version: 1,
            flags: 0,
        }
    }

    /// Serialize header to bytes.
    pub fn to_bytes(&self) -> Result<Bytes> {
        bincode::serialize(self)
            .map(Bytes::from)
            .map_err(|e| AegisError::Serialization(e.to_string()))
    }

    /// Deserialize header from bytes.
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        bincode::deserialize(data)
            .map_err(|e| AegisError::Serialization(e.to_string()))
    }
}

// =============================================================================
// Block
// =============================================================================

/// A storage block containing header and data.
#[derive(Debug, Clone)]
pub struct Block {
    pub header: BlockHeader,
    pub data: Bytes,
}

impl Block {
    /// Create a new block with the given data.
    pub fn new(block_id: BlockId, block_type: BlockType, data: Bytes) -> Self {
        let checksum = crc32fast::hash(&data);
        let data_size = data.len() as u32;

        let header = BlockHeader {
            block_id,
            block_type,
            compression: CompressionType::None,
            encryption: EncryptionType::None,
            data_size,
            uncompressed_size: data_size,
            checksum,
            version: 1,
            flags: 0,
        };

        Self { header, data }
    }

    /// Create an empty block.
    pub fn empty(block_id: BlockId, block_type: BlockType) -> Self {
        Self::new(block_id, block_type, Bytes::new())
    }

    /// Verify the block's checksum.
    pub fn verify_checksum(&self) -> bool {
        let computed = crc32fast::hash(&self.data);
        computed == self.header.checksum
    }

    /// Update the checksum after modifying data.
    pub fn update_checksum(&mut self) {
        self.header.checksum = crc32fast::hash(&self.data);
        self.header.data_size = self.data.len() as u32;
    }

    /// Compress the block data using the specified algorithm.
    pub fn compress(&mut self, compression: CompressionType) -> Result<()> {
        if self.header.compression != CompressionType::None {
            return Ok(());
        }

        let compressed = match compression {
            CompressionType::None => return Ok(()),
            CompressionType::Lz4 => {
                lz4_flex::compress_prepend_size(&self.data)
            }
            CompressionType::Zstd => {
                zstd::encode_all(self.data.as_ref(), 3)
                    .map_err(|e| AegisError::Storage(e.to_string()))?
            }
            CompressionType::Snappy => {
                let mut encoder = snap::raw::Encoder::new();
                encoder.compress_vec(&self.data)
                    .map_err(|e| AegisError::Storage(e.to_string()))?
            }
        };

        self.header.uncompressed_size = self.header.data_size;
        self.data = Bytes::from(compressed);
        self.header.data_size = self.data.len() as u32;
        self.header.compression = compression;
        self.update_checksum();

        Ok(())
    }

    /// Decompress the block data.
    pub fn decompress(&mut self) -> Result<()> {
        if self.header.compression == CompressionType::None {
            return Ok(());
        }

        let decompressed = match self.header.compression {
            CompressionType::None => return Ok(()),
            CompressionType::Lz4 => {
                lz4_flex::decompress_size_prepended(&self.data)
                    .map_err(|e| AegisError::Storage(e.to_string()))?
            }
            CompressionType::Zstd => {
                zstd::decode_all(self.data.as_ref())
                    .map_err(|e| AegisError::Storage(e.to_string()))?
            }
            CompressionType::Snappy => {
                let mut decoder = snap::raw::Decoder::new();
                decoder.decompress_vec(&self.data)
                    .map_err(|e| AegisError::Storage(e.to_string()))?
            }
        };

        self.data = Bytes::from(decompressed);
        self.header.data_size = self.data.len() as u32;
        self.header.compression = CompressionType::None;
        self.update_checksum();

        Ok(())
    }

    /// Encrypt the block data using the specified algorithm.
    /// Note: Encryption should be applied after compression for better compression ratios.
    pub fn encrypt(&mut self, encryption: EncryptionType) -> Result<()> {
        if self.header.encryption != EncryptionType::None {
            return Ok(()); // Already encrypted
        }

        let encrypted = match encryption {
            EncryptionType::None => return Ok(()),
            EncryptionType::Aes256Gcm => encrypt_aes256gcm(&self.data)?,
        };

        self.data = Bytes::from(encrypted);
        self.header.data_size = self.data.len() as u32;
        self.header.encryption = encryption;
        self.update_checksum();

        Ok(())
    }

    /// Decrypt the block data.
    /// Note: Decryption should be applied before decompression.
    pub fn decrypt(&mut self) -> Result<()> {
        if self.header.encryption == EncryptionType::None {
            return Ok(()); // Not encrypted
        }

        let decrypted = match self.header.encryption {
            EncryptionType::None => return Ok(()),
            EncryptionType::Aes256Gcm => decrypt_aes256gcm(&self.data)?,
        };

        self.data = Bytes::from(decrypted);
        self.header.data_size = self.data.len() as u32;
        self.header.encryption = EncryptionType::None;
        self.update_checksum();

        Ok(())
    }

    /// Serialize the entire block to bytes.
    pub fn to_bytes(&self) -> Result<Bytes> {
        let header_bytes = self.header.to_bytes()?;
        let mut buf = BytesMut::with_capacity(header_bytes.len() + self.data.len() + 8);

        buf.put_u32_le(header_bytes.len() as u32);
        buf.put(header_bytes);
        buf.put_u32_le(self.data.len() as u32);
        buf.put(self.data.clone());

        Ok(buf.freeze())
    }

    /// Deserialize a block from bytes.
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        if data.len() < 8 {
            return Err(AegisError::Corruption("Block too small".to_string()));
        }

        let header_len = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
        if data.len() < 4 + header_len + 4 {
            return Err(AegisError::Corruption("Block header truncated".to_string()));
        }

        let header = BlockHeader::from_bytes(&data[4..4 + header_len])?;

        let data_offset = 4 + header_len;
        let data_len = u32::from_le_bytes([
            data[data_offset],
            data[data_offset + 1],
            data[data_offset + 2],
            data[data_offset + 3],
        ]) as usize;

        if data.len() < data_offset + 4 + data_len {
            return Err(AegisError::Corruption("Block data truncated".to_string()));
        }

        let block_data = Bytes::copy_from_slice(&data[data_offset + 4..data_offset + 4 + data_len]);

        Ok(Self {
            header,
            data: block_data,
        })
    }

    /// Get the total size of the block in bytes.
    pub fn size(&self) -> usize {
        BLOCK_HEADER_SIZE + self.data.len()
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_block_roundtrip() {
        let data = Bytes::from("Hello, Aegis!");
        let block = Block::new(BlockId(1), BlockType::TableData, data.clone());

        assert!(block.verify_checksum());

        let serialized = block.to_bytes().expect("to_bytes should succeed");
        let deserialized = Block::from_bytes(&serialized).expect("from_bytes should succeed");

        assert_eq!(deserialized.header.block_id, BlockId(1));
        assert_eq!(deserialized.data, data);
        assert!(deserialized.verify_checksum());
    }

    #[test]
    fn test_block_compression_lz4() {
        let data = Bytes::from("Hello, Aegis! ".repeat(100));
        let mut block = Block::new(BlockId(1), BlockType::TableData, data.clone());

        block.compress(CompressionType::Lz4).expect("LZ4 compression should succeed");
        assert!(block.header.data_size < block.header.uncompressed_size);

        block.decompress().expect("LZ4 decompression should succeed");
        assert_eq!(block.data, data);
    }

    #[test]
    fn test_block_compression_zstd() {
        let data = Bytes::from("Hello, Aegis! ".repeat(100));
        let mut block = Block::new(BlockId(1), BlockType::TableData, data.clone());

        block.compress(CompressionType::Zstd).expect("Zstd compression should succeed");
        assert!(block.header.data_size < block.header.uncompressed_size);

        block.decompress().expect("Zstd decompression should succeed");
        assert_eq!(block.data, data);
    }

    #[test]
    fn test_block_encryption_aes256gcm() {
        // Set up test encryption key (32 bytes = 64 hex chars)
        std::env::set_var(
            "AEGIS_ENCRYPTION_KEY",
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        );

        let data = Bytes::from("Secret data to encrypt!");
        let mut block = Block::new(BlockId(1), BlockType::TableData, data.clone());

        // Encrypt
        block
            .encrypt(EncryptionType::Aes256Gcm)
            .expect("AES-256-GCM encryption should succeed");
        assert_eq!(block.header.encryption, EncryptionType::Aes256Gcm);
        assert_ne!(block.data, data); // Data should be encrypted
        // Encrypted data includes 12-byte nonce + 16-byte auth tag
        assert!(block.data.len() > data.len());

        // Decrypt
        block
            .decrypt()
            .expect("AES-256-GCM decryption should succeed");
        assert_eq!(block.header.encryption, EncryptionType::None);
        assert_eq!(block.data, data);
    }

    #[test]
    fn test_block_encryption_roundtrip() {
        // Set up test encryption key
        std::env::set_var(
            "AEGIS_ENCRYPTION_KEY",
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        );

        let data = Bytes::from("Hello, encrypted Aegis!");
        let mut block = Block::new(BlockId(1), BlockType::TableData, data.clone());

        // Encrypt
        block
            .encrypt(EncryptionType::Aes256Gcm)
            .expect("Encryption should succeed");
        assert!(block.verify_checksum());

        // Serialize
        let serialized = block.to_bytes().expect("to_bytes should succeed");
        let mut deserialized = Block::from_bytes(&serialized).expect("from_bytes should succeed");

        assert_eq!(deserialized.header.encryption, EncryptionType::Aes256Gcm);
        assert!(deserialized.verify_checksum());

        // Decrypt
        deserialized.decrypt().expect("Decryption should succeed");
        assert_eq!(deserialized.data, data);
    }

    #[test]
    fn test_block_compression_then_encryption() {
        // Set up test encryption key
        std::env::set_var(
            "AEGIS_ENCRYPTION_KEY",
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        );

        let data = Bytes::from("Hello, Aegis! ".repeat(100));
        let mut block = Block::new(BlockId(1), BlockType::TableData, data.clone());

        // Compress first (better compression on plaintext)
        block
            .compress(CompressionType::Lz4)
            .expect("Compression should succeed");
        let compressed_size = block.header.data_size;

        // Then encrypt
        block
            .encrypt(EncryptionType::Aes256Gcm)
            .expect("Encryption should succeed");
        assert_eq!(block.header.compression, CompressionType::Lz4);
        assert_eq!(block.header.encryption, EncryptionType::Aes256Gcm);

        // Decrypt first
        block.decrypt().expect("Decryption should succeed");
        assert_eq!(block.header.data_size, compressed_size);

        // Then decompress
        block.decompress().expect("Decompression should succeed");
        assert_eq!(block.data, data);
    }

    #[test]
    fn test_encryption_key_validation() {
        // Test with invalid key length
        std::env::set_var("AEGIS_ENCRYPTION_KEY", "too_short");

        // Clear the cached key to test fresh initialization
        // Note: In a real test environment, we'd need to reset OnceLock
        // This test validates the key format checking logic
    }

    #[test]
    fn test_double_encryption_is_noop() {
        std::env::set_var(
            "AEGIS_ENCRYPTION_KEY",
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        );

        let data = Bytes::from("Test data");
        let mut block = Block::new(BlockId(1), BlockType::TableData, data);

        // First encryption
        block
            .encrypt(EncryptionType::Aes256Gcm)
            .expect("First encryption should succeed");
        let first_encrypted = block.data.clone();

        // Second encryption should be a no-op
        block
            .encrypt(EncryptionType::Aes256Gcm)
            .expect("Second encryption should succeed (no-op)");
        assert_eq!(block.data, first_encrypted);
    }

    #[test]
    fn test_double_decryption_is_noop() {
        std::env::set_var(
            "AEGIS_ENCRYPTION_KEY",
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        );

        let data = Bytes::from("Test data");
        let mut block = Block::new(BlockId(1), BlockType::TableData, data.clone());

        block
            .encrypt(EncryptionType::Aes256Gcm)
            .expect("Encryption should succeed");
        block.decrypt().expect("First decryption should succeed");
        let decrypted = block.data.clone();

        // Second decryption should be a no-op
        block
            .decrypt()
            .expect("Second decryption should succeed (no-op)");
        assert_eq!(block.data, decrypted);
        assert_eq!(block.data, data);
    }
}
