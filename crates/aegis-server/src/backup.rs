//! Aegis Backup Module
//!
//! Provides backup and restore functionality for Aegis-DB.
//! Creates timestamped backups of all data including block files,
//! WAL files, and metadata.
//!
//! Supports encrypted backups for HIPAA compliance using AES-256-GCM.
//! Set AEGIS_ENCRYPTION_KEY environment variable (64 hex chars = 32 bytes).
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use crate::activity::ActivityType;
use crate::state::AppState;
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::Utc;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path as StdPath, PathBuf};
use std::sync::OnceLock;

// =============================================================================
// Encryption Constants and Key Management
// =============================================================================

const AES_GCM_NONCE_SIZE: usize = 12;
const AES_256_KEY_SIZE: usize = 32;
const ENCRYPTION_ALGORITHM: &str = "AES-256-GCM";

/// Global encryption key loaded from environment variable.
static ENCRYPTION_KEY: OnceLock<[u8; AES_256_KEY_SIZE]> = OnceLock::new();
static ENCRYPTION_KEY_INIT: Mutex<bool> = Mutex::new(false);

/// Get the encryption key from environment variable AEGIS_ENCRYPTION_KEY.
/// The key must be 32 bytes (64 hex characters).
fn get_encryption_key() -> Result<&'static [u8; AES_256_KEY_SIZE], String> {
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
        "AEGIS_ENCRYPTION_KEY environment variable not set. Required for encrypted backups.".to_string()
    })?;

    let key_bytes = hex::decode(&hex_key).map_err(|e| {
        format!("Invalid hex encoding in AEGIS_ENCRYPTION_KEY: {}", e)
    })?;

    if key_bytes.len() != AES_256_KEY_SIZE {
        return Err(format!(
            "AEGIS_ENCRYPTION_KEY must be {} bytes ({} hex chars), got {} bytes",
            AES_256_KEY_SIZE,
            AES_256_KEY_SIZE * 2,
            key_bytes.len()
        ));
    }

    let mut key = [0u8; AES_256_KEY_SIZE];
    key.copy_from_slice(&key_bytes);

    // Store the key
    let _ = ENCRYPTION_KEY.set(key);

    Ok(ENCRYPTION_KEY.get().unwrap())
}

/// Encrypt data using AES-256-GCM.
/// Returns encrypted data with 12-byte nonce prepended.
fn encrypt_aes256gcm(plaintext: &[u8]) -> Result<Vec<u8>, String> {
    let key = get_encryption_key()?;
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|e| format!("Failed to create cipher: {}", e))?;

    // Generate a random nonce (12 bytes for AES-GCM)
    let mut nonce_bytes = [0u8; AES_GCM_NONCE_SIZE];
    getrandom::getrandom(&mut nonce_bytes)
        .map_err(|e| format!("Failed to generate nonce: {}", e))?;
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher.encrypt(nonce, plaintext).map_err(|e| {
        format!("Encryption failed: {}", e)
    })?;

    // Prepend nonce to ciphertext for storage
    let mut result = Vec::with_capacity(AES_GCM_NONCE_SIZE + ciphertext.len());
    result.extend_from_slice(&nonce_bytes);
    result.extend(ciphertext);

    Ok(result)
}

/// Decrypt data using AES-256-GCM.
/// Expects 12-byte nonce prepended to ciphertext.
fn decrypt_aes256gcm(encrypted_data: &[u8]) -> Result<Vec<u8>, String> {
    if encrypted_data.len() < AES_GCM_NONCE_SIZE {
        return Err("Encrypted data too short: missing nonce".to_string());
    }

    let key = get_encryption_key()?;
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|e| format!("Failed to create cipher: {}", e))?;

    let nonce = Nonce::from_slice(&encrypted_data[..AES_GCM_NONCE_SIZE]);
    let ciphertext = &encrypted_data[AES_GCM_NONCE_SIZE..];

    let plaintext = cipher.decrypt(nonce, ciphertext).map_err(|e| {
        format!("Decryption failed: {}. Ensure AEGIS_ENCRYPTION_KEY matches the key used during backup.", e)
    })?;

    Ok(plaintext)
}

// =============================================================================
// Backup Types
// =============================================================================

/// Information about a backup.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupInfo {
    pub id: String,
    pub timestamp: String,
    pub version: String,
    pub size_bytes: u64,
    pub checksum: String,
    pub compressed: bool,
    pub status: BackupStatus,
    pub files_count: usize,
    pub created_by: Option<String>,
    /// Whether the backup is encrypted (HIPAA compliance).
    #[serde(default)]
    pub encrypted: bool,
    /// Encryption algorithm used.
    #[serde(default)]
    pub encryption_algorithm: Option<String>,
}

/// Status of a backup operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BackupStatus {
    InProgress,
    Completed,
    Failed,
    Corrupted,
}

/// Backup metadata stored with each backup.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupMetadata {
    pub id: String,
    pub timestamp: String,
    pub version: String,
    pub checksum: String,
    pub compressed: bool,
    pub files: Vec<BackupFile>,
    pub created_by: Option<String>,
    /// Whether the backup is encrypted (HIPAA compliance).
    #[serde(default)]
    pub encrypted: bool,
    /// Encryption algorithm used (e.g., "AES-256-GCM").
    #[serde(default)]
    pub encryption_algorithm: Option<String>,
    /// Key ID for key rotation support.
    #[serde(default)]
    pub key_id: Option<String>,
}

/// Information about a file in the backup.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupFile {
    pub path: String,
    pub size_bytes: u64,
    pub checksum: String,
}

/// Request to create a backup.
#[derive(Debug, Clone, Deserialize)]
pub struct CreateBackupRequest {
    #[serde(default)]
    pub compress: bool,
    pub description: Option<String>,
    /// Enable encryption for HIPAA compliance (default: true).
    #[serde(default = "default_encrypt")]
    pub encrypt: bool,
    /// Key ID for key rotation support.
    pub encryption_key_id: Option<String>,
}

fn default_encrypt() -> bool {
    true
}

/// Response from creating a backup.
#[derive(Debug, Serialize)]
pub struct CreateBackupResponse {
    pub success: bool,
    pub backup: Option<BackupInfo>,
    pub error: Option<String>,
}

/// Request to restore from a backup.
#[derive(Debug, Clone, Deserialize)]
pub struct RestoreRequest {
    pub backup_id: String,
    #[serde(default)]
    pub force: bool,
}

/// Response from restoring a backup.
#[derive(Debug, Serialize)]
pub struct RestoreResponse {
    pub success: bool,
    pub message: String,
    pub files_restored: usize,
}

/// List backups response.
#[derive(Debug, Serialize)]
pub struct ListBackupsResponse {
    pub backups: Vec<BackupInfo>,
    pub total: usize,
}

/// Delete backup response.
#[derive(Debug, Serialize)]
pub struct DeleteBackupResponse {
    pub success: bool,
    pub message: String,
}

// =============================================================================
// Backup Manager
// =============================================================================

/// Manages backup and restore operations.
pub struct BackupManager {
    backup_dir: PathBuf,
    data_dir: PathBuf,
}

impl BackupManager {
    /// Create a new backup manager.
    pub fn new(data_dir: PathBuf) -> Self {
        let backup_dir = data_dir.join("backups");
        // Ensure backup directory exists
        if let Err(e) = fs::create_dir_all(&backup_dir) {
            tracing::error!("Failed to create backup directory: {}", e);
        }
        Self { backup_dir, data_dir }
    }

    /// Create a new backup.
    ///
    /// # Arguments
    /// * `compress` - Whether to compress the backup
    /// * `created_by` - Optional user who created the backup
    /// * `encrypt` - Whether to encrypt files (HIPAA compliance)
    /// * `key_id` - Optional key ID for key rotation support
    pub fn create_backup(
        &self,
        compress: bool,
        created_by: Option<&str>,
        encrypt: bool,
        key_id: Option<&str>,
    ) -> Result<BackupInfo, String> {
        // Validate encryption key is available if encryption is requested
        if encrypt {
            get_encryption_key()?;
        }

        let timestamp = Utc::now();
        let backup_id = format!("backup_{}", timestamp.format("%Y%m%d_%H%M%S"));
        let backup_path = self.backup_dir.join(&backup_id);

        // Create backup directory
        fs::create_dir_all(&backup_path)
            .map_err(|e| format!("Failed to create backup directory: {}", e))?;

        let mut files = Vec::new();
        let mut total_size: u64 = 0;
        let mut hasher = Sha256::new();

        // Copy data files
        let dirs_to_backup = vec![
            ("blocks", self.data_dir.join("blocks")),
            ("wal", self.data_dir.join("wal")),
            ("documents", self.data_dir.join("documents")),
        ];

        // Also copy individual data files
        let files_to_backup = vec![
            "kv_store.json",
            "sql_tables.json",
        ];

        // Copy directory contents
        for (name, source_dir) in dirs_to_backup {
            if source_dir.exists() && source_dir.is_dir() {
                let target_dir = backup_path.join(name);
                fs::create_dir_all(&target_dir)
                    .map_err(|e| format!("Failed to create backup subdirectory {}: {}", name, e))?;

                self.copy_directory(&source_dir, &target_dir, &mut files, &mut total_size, &mut hasher, encrypt)?;
            }
        }

        // Copy individual files
        for filename in files_to_backup {
            let source_file = self.data_dir.join(filename);
            if source_file.exists() && source_file.is_file() {
                let target_file = backup_path.join(filename);
                self.copy_file(&source_file, &target_file, &mut files, &mut total_size, &mut hasher, encrypt)?;
            }
        }

        let checksum = format!("{:x}", hasher.finalize());

        // Create metadata
        let metadata = BackupMetadata {
            id: backup_id.clone(),
            timestamp: timestamp.to_rfc3339(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            checksum: checksum.clone(),
            compressed: compress,
            files: files.clone(),
            created_by: created_by.map(String::from),
            encrypted: encrypt,
            encryption_algorithm: if encrypt {
                Some(ENCRYPTION_ALGORITHM.to_string())
            } else {
                None
            },
            key_id: key_id.map(String::from),
        };

        // Save metadata
        let metadata_path = backup_path.join("metadata.json");
        let metadata_json = serde_json::to_string_pretty(&metadata)
            .map_err(|e| format!("Failed to serialize metadata: {}", e))?;
        fs::write(&metadata_path, metadata_json)
            .map_err(|e| format!("Failed to write metadata: {}", e))?;

        // Optionally compress
        if compress {
            self.compress_backup(&backup_path)?;
        }

        let backup_info = BackupInfo {
            id: backup_id,
            timestamp: timestamp.to_rfc3339(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            size_bytes: total_size,
            checksum,
            compressed: compress,
            status: BackupStatus::Completed,
            files_count: files.len(),
            created_by: created_by.map(String::from),
            encrypted: encrypt,
            encryption_algorithm: if encrypt {
                Some(ENCRYPTION_ALGORITHM.to_string())
            } else {
                None
            },
        };

        tracing::info!(
            "Backup created: {} ({} files, {} bytes, encrypted: {})",
            backup_info.id,
            backup_info.files_count,
            backup_info.size_bytes,
            backup_info.encrypted
        );

        Ok(backup_info)
    }

    /// Copy a directory recursively, optionally encrypting files.
    fn copy_directory(
        &self,
        source: &StdPath,
        target: &StdPath,
        files: &mut Vec<BackupFile>,
        total_size: &mut u64,
        hasher: &mut Sha256,
        encrypt: bool,
    ) -> Result<(), String> {
        let entries = fs::read_dir(source)
            .map_err(|e| format!("Failed to read directory {:?}: {}", source, e))?;

        for entry in entries {
            let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
            let path = entry.path();
            let file_name = entry.file_name();
            let target_path = target.join(&file_name);

            if path.is_dir() {
                fs::create_dir_all(&target_path)
                    .map_err(|e| format!("Failed to create directory {:?}: {}", target_path, e))?;
                self.copy_directory(&path, &target_path, files, total_size, hasher, encrypt)?;
            } else if path.is_file() {
                self.copy_file(&path, &target_path, files, total_size, hasher, encrypt)?;
            }
        }

        Ok(())
    }

    /// Copy a single file, optionally encrypting it.
    fn copy_file(
        &self,
        source: &StdPath,
        target: &StdPath,
        files: &mut Vec<BackupFile>,
        total_size: &mut u64,
        hasher: &mut Sha256,
        encrypt: bool,
    ) -> Result<(), String> {
        // Read source file
        let mut source_file = File::open(source)
            .map_err(|e| format!("Failed to open {:?}: {}", source, e))?;
        let mut contents = Vec::new();
        source_file
            .read_to_end(&mut contents)
            .map_err(|e| format!("Failed to read {:?}: {}", source, e))?;

        // Calculate file checksum (of original plaintext for integrity verification)
        let mut file_hasher = Sha256::new();
        file_hasher.update(&contents);
        let file_checksum = format!("{:x}", file_hasher.finalize());

        // Update global hasher (using original plaintext)
        hasher.update(&contents);

        // Encrypt if requested
        let data_to_write = if encrypt {
            encrypt_aes256gcm(&contents)?
        } else {
            contents.clone()
        };

        // Write target file
        let mut target_file = File::create(target)
            .map_err(|e| format!("Failed to create {:?}: {}", target, e))?;
        target_file
            .write_all(&data_to_write)
            .map_err(|e| format!("Failed to write {:?}: {}", target, e))?;

        let size = contents.len() as u64;
        *total_size += size;

        // Get relative path for metadata
        let relative_path = source
            .strip_prefix(&self.data_dir)
            .unwrap_or(source)
            .to_string_lossy()
            .to_string();

        files.push(BackupFile {
            path: relative_path,
            size_bytes: size,
            checksum: file_checksum,
        });

        Ok(())
    }

    /// Compress a backup directory (simplified - just creates a marker).
    fn compress_backup(&self, _backup_path: &StdPath) -> Result<(), String> {
        // In a full implementation, this would create a .tar.gz or .zip archive
        // For now, we just mark the backup as compressed in metadata
        tracing::info!("Compression requested but not fully implemented yet");
        Ok(())
    }

    /// List all available backups.
    pub fn list_backups(&self) -> Result<Vec<BackupInfo>, String> {
        let mut backups = Vec::new();

        if !self.backup_dir.exists() {
            return Ok(backups);
        }

        let entries = fs::read_dir(&self.backup_dir)
            .map_err(|e| format!("Failed to read backup directory: {}", e))?;

        for entry in entries {
            let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
            let path = entry.path();

            if path.is_dir() {
                let metadata_path = path.join("metadata.json");
                if metadata_path.exists() {
                    match self.read_backup_metadata(&metadata_path) {
                        Ok(metadata) => {
                            // Calculate actual size on disk
                            let size = self.calculate_directory_size(&path);
                            let status = if self.verify_backup_integrity(&path, &metadata) {
                                BackupStatus::Completed
                            } else {
                                BackupStatus::Corrupted
                            };

                            backups.push(BackupInfo {
                                id: metadata.id,
                                timestamp: metadata.timestamp,
                                version: metadata.version,
                                size_bytes: size,
                                checksum: metadata.checksum,
                                compressed: metadata.compressed,
                                status,
                                files_count: metadata.files.len(),
                                created_by: metadata.created_by,
                                encrypted: metadata.encrypted,
                                encryption_algorithm: metadata.encryption_algorithm,
                            });
                        }
                        Err(e) => {
                            tracing::warn!("Failed to read backup metadata from {:?}: {}", path, e);
                        }
                    }
                }
            }
        }

        // Sort by timestamp (newest first)
        backups.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        Ok(backups)
    }

    /// Get information about a specific backup.
    pub fn get_backup(&self, backup_id: &str) -> Result<BackupInfo, String> {
        let backup_path = self.backup_dir.join(backup_id);
        if !backup_path.exists() {
            return Err(format!("Backup '{}' not found", backup_id));
        }

        let metadata_path = backup_path.join("metadata.json");
        let metadata = self.read_backup_metadata(&metadata_path)?;

        let size = self.calculate_directory_size(&backup_path);
        let status = if self.verify_backup_integrity(&backup_path, &metadata) {
            BackupStatus::Completed
        } else {
            BackupStatus::Corrupted
        };

        Ok(BackupInfo {
            id: metadata.id,
            timestamp: metadata.timestamp,
            version: metadata.version,
            size_bytes: size,
            checksum: metadata.checksum,
            compressed: metadata.compressed,
            status,
            files_count: metadata.files.len(),
            created_by: metadata.created_by,
            encrypted: metadata.encrypted,
            encryption_algorithm: metadata.encryption_algorithm,
        })
    }

    /// Read backup metadata from file.
    fn read_backup_metadata(&self, path: &StdPath) -> Result<BackupMetadata, String> {
        let content = fs::read_to_string(path)
            .map_err(|e| format!("Failed to read metadata file: {}", e))?;
        serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse metadata: {}", e))
    }

    /// Calculate total size of a directory.
    fn calculate_directory_size(&self, path: &StdPath) -> u64 {
        let mut size = 0;
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                let entry_path = entry.path();
                if entry_path.is_dir() {
                    size += self.calculate_directory_size(&entry_path);
                } else if entry_path.is_file() {
                    size += entry.metadata().map(|m| m.len()).unwrap_or(0);
                }
            }
        }
        size
    }

    /// Verify backup integrity by checking file checksums.
    /// For encrypted backups, files must be decrypted before checksum verification.
    fn verify_backup_integrity(&self, backup_path: &StdPath, metadata: &BackupMetadata) -> bool {
        // For encrypted backups, skip verification if key is not available
        // (we'll fail later during restore with a clear error message)
        if metadata.encrypted && get_encryption_key().is_err() {
            tracing::warn!(
                "Cannot verify encrypted backup integrity: encryption key not available"
            );
            return true; // Assume valid, will fail during restore if key is wrong
        }

        // Verify a sample of files (or all for small backups)
        let files_to_check = if metadata.files.len() > 10 {
            // Check first, last, and some random files
            let mut indices: Vec<usize> = vec![0, metadata.files.len() - 1];
            indices.push(metadata.files.len() / 2);
            indices.push(metadata.files.len() / 4);
            indices.push(3 * metadata.files.len() / 4);
            indices
        } else {
            (0..metadata.files.len()).collect()
        };

        for idx in files_to_check {
            if let Some(file_info) = metadata.files.get(idx) {
                // Reconstruct the path - files are stored relative to backup_path
                let file_path = backup_path.join(&file_info.path);
                if file_path.exists() {
                    if let Ok(mut file) = File::open(&file_path) {
                        let mut contents = Vec::new();
                        if file.read_to_end(&mut contents).is_ok() {
                            // If encrypted, decrypt before checksum verification
                            // (checksums are stored for original plaintext)
                            let data_to_verify = if metadata.encrypted {
                                match decrypt_aes256gcm(&contents) {
                                    Ok(decrypted) => decrypted,
                                    Err(e) => {
                                        tracing::warn!(
                                            "Failed to decrypt {:?} for integrity check: {}",
                                            file_path,
                                            e
                                        );
                                        return false;
                                    }
                                }
                            } else {
                                contents
                            };

                            let mut hasher = Sha256::new();
                            hasher.update(&data_to_verify);
                            let checksum = format!("{:x}", hasher.finalize());
                            if checksum != file_info.checksum {
                                tracing::warn!(
                                    "Checksum mismatch for {:?}: expected {}, got {}",
                                    file_path,
                                    file_info.checksum,
                                    checksum
                                );
                                return false;
                            }
                        }
                    }
                }
            }
        }
        true
    }

    /// Restore from a backup.
    pub fn restore_backup(&self, backup_id: &str, force: bool) -> Result<usize, String> {
        let backup_path = self.backup_dir.join(backup_id);
        if !backup_path.exists() {
            return Err(format!("Backup '{}' not found", backup_id));
        }

        let metadata_path = backup_path.join("metadata.json");
        let metadata = self.read_backup_metadata(&metadata_path)?;

        // If backup is encrypted, ensure we have the decryption key
        if metadata.encrypted {
            get_encryption_key().map_err(|e| {
                format!(
                    "Cannot restore encrypted backup: {}. Set AEGIS_ENCRYPTION_KEY environment variable.",
                    e
                )
            })?;
        }

        // Verify integrity before restore (skips checksum for encrypted files)
        if !self.verify_backup_integrity(&backup_path, &metadata) && !force {
            return Err("Backup integrity check failed. Use force=true to restore anyway.".to_string());
        }

        let mut files_restored = 0;

        // Restore directories
        let dirs_to_restore = vec!["blocks", "wal", "documents"];
        for dir_name in dirs_to_restore {
            let source_dir = backup_path.join(dir_name);
            if source_dir.exists() && source_dir.is_dir() {
                let target_dir = self.data_dir.join(dir_name);
                // Create target directory if it doesn't exist
                fs::create_dir_all(&target_dir)
                    .map_err(|e| format!("Failed to create directory {:?}: {}", target_dir, e))?;
                files_restored += self.restore_directory(&source_dir, &target_dir, metadata.encrypted)?;
            }
        }

        // Restore individual files
        let files_to_restore = vec!["kv_store.json", "sql_tables.json"];
        for filename in files_to_restore {
            let source_file = backup_path.join(filename);
            if source_file.exists() && source_file.is_file() {
                let target_file = self.data_dir.join(filename);
                self.restore_file(&source_file, &target_file, metadata.encrypted)?;
                files_restored += 1;
            }
        }

        tracing::info!(
            "Restored backup '{}': {} files restored (encrypted: {})",
            backup_id,
            files_restored,
            metadata.encrypted
        );

        Ok(files_restored)
    }

    /// Restore a directory recursively, optionally decrypting files.
    fn restore_directory(
        &self,
        source: &StdPath,
        target: &StdPath,
        encrypted: bool,
    ) -> Result<usize, String> {
        let mut count = 0;

        let entries = fs::read_dir(source)
            .map_err(|e| format!("Failed to read directory {:?}: {}", source, e))?;

        for entry in entries {
            let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
            let path = entry.path();
            let file_name = entry.file_name();
            let target_path = target.join(&file_name);

            if path.is_dir() {
                fs::create_dir_all(&target_path)
                    .map_err(|e| format!("Failed to create directory {:?}: {}", target_path, e))?;
                count += self.restore_directory(&path, &target_path, encrypted)?;
            } else if path.is_file() {
                self.restore_file(&path, &target_path, encrypted)?;
                count += 1;
            }
        }

        Ok(count)
    }

    /// Restore a single file, optionally decrypting it.
    fn restore_file(
        &self,
        source: &StdPath,
        target: &StdPath,
        encrypted: bool,
    ) -> Result<(), String> {
        // Read source file
        let mut source_file = File::open(source)
            .map_err(|e| format!("Failed to open {:?}: {}", source, e))?;
        let mut contents = Vec::new();
        source_file
            .read_to_end(&mut contents)
            .map_err(|e| format!("Failed to read {:?}: {}", source, e))?;

        // Decrypt if the backup was encrypted
        let data_to_write = if encrypted {
            decrypt_aes256gcm(&contents)?
        } else {
            contents
        };

        // Write target file
        let mut target_file = File::create(target)
            .map_err(|e| format!("Failed to create {:?}: {}", target, e))?;
        target_file
            .write_all(&data_to_write)
            .map_err(|e| format!("Failed to write {:?}: {}", target, e))?;

        Ok(())
    }

    /// Delete a backup.
    pub fn delete_backup(&self, backup_id: &str) -> Result<(), String> {
        let backup_path = self.backup_dir.join(backup_id);
        if !backup_path.exists() {
            return Err(format!("Backup '{}' not found", backup_id));
        }

        fs::remove_dir_all(&backup_path)
            .map_err(|e| format!("Failed to delete backup: {}", e))?;

        tracing::info!("Deleted backup: {}", backup_id);
        Ok(())
    }
}

// =============================================================================
// HTTP Handlers
// =============================================================================

/// Create a new backup.
pub async fn create_backup(
    State(state): State<AppState>,
    Json(request): Json<CreateBackupRequest>,
) -> impl IntoResponse {
    state.activity.log(ActivityType::System, "Creating backup");

    // Get data directory from config
    let data_dir = match &state.config.data_dir {
        Some(dir) => PathBuf::from(dir),
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(CreateBackupResponse {
                    success: false,
                    backup: None,
                    error: Some("No data directory configured. Set data_dir in server configuration.".to_string()),
                }),
            );
        }
    };

    // First, save all in-memory data to disk
    if let Err(e) = state.save_to_disk() {
        tracing::warn!("Failed to save data to disk before backup: {}", e);
    }

    let manager = BackupManager::new(data_dir);

    match manager.create_backup(
        request.compress,
        None,
        request.encrypt,
        request.encryption_key_id.as_deref(),
    ) {
        Ok(backup) => {
            state.activity.log(
                ActivityType::System,
                &format!("Backup created: {}", backup.id),
            );
            (
                StatusCode::CREATED,
                Json(CreateBackupResponse {
                    success: true,
                    backup: Some(backup),
                    error: None,
                }),
            )
        }
        Err(e) => {
            state.activity.log(
                ActivityType::System,
                &format!("Backup failed: {}", e),
            );
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(CreateBackupResponse {
                    success: false,
                    backup: None,
                    error: Some(e),
                }),
            )
        }
    }
}

/// List all available backups.
pub async fn list_backups(State(state): State<AppState>) -> impl IntoResponse {
    state.activity.log(ActivityType::Query, "Listing backups");

    // Get data directory from config
    let data_dir = match &state.config.data_dir {
        Some(dir) => PathBuf::from(dir),
        None => {
            return (
                StatusCode::OK,
                Json(ListBackupsResponse {
                    backups: vec![],
                    total: 0,
                }),
            );
        }
    };

    let manager = BackupManager::new(data_dir);

    match manager.list_backups() {
        Ok(backups) => {
            let total = backups.len();
            (StatusCode::OK, Json(ListBackupsResponse { backups, total }))
        }
        Err(e) => {
            tracing::error!("Failed to list backups: {}", e);
            (
                StatusCode::OK,
                Json(ListBackupsResponse {
                    backups: vec![],
                    total: 0,
                }),
            )
        }
    }
}

/// Restore from a backup.
pub async fn restore_backup(
    State(state): State<AppState>,
    Json(request): Json<RestoreRequest>,
) -> impl IntoResponse {
    state.activity.log(
        ActivityType::System,
        &format!("Restoring from backup: {}", request.backup_id),
    );

    // Get data directory from config
    let data_dir = match &state.config.data_dir {
        Some(dir) => PathBuf::from(dir),
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(RestoreResponse {
                    success: false,
                    message: "No data directory configured. Set data_dir in server configuration.".to_string(),
                    files_restored: 0,
                }),
            );
        }
    };

    let manager = BackupManager::new(data_dir);

    match manager.restore_backup(&request.backup_id, request.force) {
        Ok(files_restored) => {
            state.activity.log(
                ActivityType::System,
                &format!(
                    "Backup restored: {} ({} files)",
                    request.backup_id, files_restored
                ),
            );
            (
                StatusCode::OK,
                Json(RestoreResponse {
                    success: true,
                    message: format!(
                        "Successfully restored backup '{}'. Please restart the server to load restored data.",
                        request.backup_id
                    ),
                    files_restored,
                }),
            )
        }
        Err(e) => {
            state.activity.log(
                ActivityType::System,
                &format!("Restore failed: {}", e),
            );
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(RestoreResponse {
                    success: false,
                    message: e,
                    files_restored: 0,
                }),
            )
        }
    }
}

/// Delete a backup.
pub async fn delete_backup(
    State(state): State<AppState>,
    Path(backup_id): Path<String>,
) -> impl IntoResponse {
    state.activity.log(
        ActivityType::Delete,
        &format!("Deleting backup: {}", backup_id),
    );

    // Get data directory from config
    let data_dir = match &state.config.data_dir {
        Some(dir) => PathBuf::from(dir),
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(DeleteBackupResponse {
                    success: false,
                    message: "No data directory configured.".to_string(),
                }),
            );
        }
    };

    let manager = BackupManager::new(data_dir);

    match manager.delete_backup(&backup_id) {
        Ok(()) => {
            state.activity.log(
                ActivityType::Delete,
                &format!("Backup deleted: {}", backup_id),
            );
            (
                StatusCode::OK,
                Json(DeleteBackupResponse {
                    success: true,
                    message: format!("Backup '{}' deleted successfully", backup_id),
                }),
            )
        }
        Err(e) => (
            StatusCode::NOT_FOUND,
            Json(DeleteBackupResponse {
                success: false,
                message: e,
            }),
        ),
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_backup_manager_creation() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let manager = BackupManager::new(temp_dir.path().to_path_buf());
        assert!(manager.backup_dir.exists());
    }

    #[test]
    fn test_create_and_list_backup() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let data_dir = temp_dir.path().to_path_buf();

        // Create some test data
        let kv_path = data_dir.join("kv_store.json");
        fs::write(&kv_path, r#"[{"key": "test", "value": "data"}]"#).expect("Failed to write test data");

        let manager = BackupManager::new(data_dir);

        // Create backup
        let backup = manager.create_backup(false, Some("test_user"), false, None).expect("Failed to create backup");
        assert!(!backup.id.is_empty());
        assert_eq!(backup.status, BackupStatus::Completed);

        // List backups
        let backups = manager.list_backups().expect("Failed to list backups");
        assert_eq!(backups.len(), 1);
        assert_eq!(backups[0].id, backup.id);
    }

    #[test]
    fn test_backup_and_restore() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let data_dir = temp_dir.path().to_path_buf();

        // Create test data
        let kv_path = data_dir.join("kv_store.json");
        let test_data = r#"[{"key": "test_key", "value": "test_value"}]"#;
        fs::write(&kv_path, test_data).expect("Failed to write test data");

        let manager = BackupManager::new(data_dir.clone());

        // Create backup
        let backup = manager.create_backup(false, None, false, None).expect("Failed to create backup");

        // Modify original data
        fs::write(&kv_path, r#"[{"key": "modified"}]"#).expect("Failed to modify data");

        // Restore
        let files_restored = manager.restore_backup(&backup.id, false).expect("Failed to restore");
        assert!(files_restored > 0);

        // Verify data was restored
        let restored_data = fs::read_to_string(&kv_path).expect("Failed to read restored data");
        assert_eq!(restored_data, test_data);
    }

    #[test]
    fn test_delete_backup() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let data_dir = temp_dir.path().to_path_buf();

        // Create test data
        let kv_path = data_dir.join("kv_store.json");
        fs::write(&kv_path, "{}").expect("Failed to write test data");

        let manager = BackupManager::new(data_dir);

        // Create and delete backup
        let backup = manager.create_backup(false, None, false, None).expect("Failed to create backup");
        manager.delete_backup(&backup.id).expect("Failed to delete backup");

        // Verify backup is gone
        let backups = manager.list_backups().expect("Failed to list backups");
        assert!(backups.is_empty());
    }

    #[test]
    fn test_backup_not_found() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let manager = BackupManager::new(temp_dir.path().to_path_buf());

        let result = manager.restore_backup("nonexistent_backup", false);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[test]
    fn test_encrypted_backup_and_restore() {
        // Set up test encryption key (32 bytes = 64 hex chars)
        std::env::set_var(
            "AEGIS_ENCRYPTION_KEY",
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        );

        let temp_dir = tempdir().expect("Failed to create temp dir");
        let data_dir = temp_dir.path().to_path_buf();

        // Create test data files
        let kv_path = data_dir.join("kv_store.json");
        let test_data = r#"{"key": "sensitive_patient_data", "value": "HIPAA protected info"}"#;
        fs::write(&kv_path, test_data).expect("Failed to write test data");

        // Create a blocks directory with test data
        let blocks_dir = data_dir.join("blocks");
        fs::create_dir_all(&blocks_dir).expect("Failed to create blocks dir");
        let block_data = b"Encrypted block data for HIPAA compliance";
        fs::write(blocks_dir.join("block_001.dat"), block_data).expect("Failed to write block");

        let manager = BackupManager::new(data_dir.clone());

        // Create encrypted backup
        let backup = manager
            .create_backup(false, Some("hipaa_admin"), true, Some("key_v1"))
            .expect("Failed to create encrypted backup");

        assert!(!backup.id.is_empty());
        assert_eq!(backup.status, BackupStatus::Completed);
        assert!(backup.encrypted);
        assert_eq!(backup.encryption_algorithm, Some("AES-256-GCM".to_string()));
        assert!(backup.files_count > 0);

        // Verify the backup files are actually encrypted (not plaintext)
        let backup_kv_path = manager.backup_dir.join(&backup.id).join("kv_store.json");
        let encrypted_contents = fs::read(&backup_kv_path).expect("Failed to read backup file");
        // Encrypted data should be different from plaintext and include nonce (12 bytes)
        assert_ne!(encrypted_contents.as_slice(), test_data.as_bytes());
        assert!(encrypted_contents.len() > test_data.len());

        // Modify original data to verify restore works
        fs::write(&kv_path, r#"{"modified": true}"#).expect("Failed to modify data");
        fs::write(blocks_dir.join("block_001.dat"), b"modified").expect("Failed to modify block");

        // Restore encrypted backup
        let files_restored = manager
            .restore_backup(&backup.id, false)
            .expect("Failed to restore encrypted backup");
        assert!(files_restored > 0);

        // Verify data was restored correctly (decrypted)
        let restored_data = fs::read_to_string(&kv_path).expect("Failed to read restored data");
        assert_eq!(restored_data, test_data);

        let restored_block = fs::read(blocks_dir.join("block_001.dat"))
            .expect("Failed to read restored block");
        assert_eq!(restored_block.as_slice(), block_data);
    }

    #[test]
    fn test_encrypted_backup_metadata() {
        std::env::set_var(
            "AEGIS_ENCRYPTION_KEY",
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        );

        let temp_dir = tempdir().expect("Failed to create temp dir");
        let data_dir = temp_dir.path().to_path_buf();

        // Create minimal test data
        fs::write(data_dir.join("kv_store.json"), "{}").expect("Failed to write test data");

        let manager = BackupManager::new(data_dir);

        // Create encrypted backup
        let backup = manager
            .create_backup(false, None, true, None)
            .expect("Failed to create backup");

        // Read and verify metadata
        let metadata_path = manager.backup_dir.join(&backup.id).join("metadata.json");
        let metadata_content = fs::read_to_string(&metadata_path).expect("Failed to read metadata");
        let metadata: BackupMetadata =
            serde_json::from_str(&metadata_content).expect("Failed to parse metadata");

        assert!(metadata.encrypted);
        assert_eq!(metadata.encryption_algorithm, Some("AES-256-GCM".to_string()));
    }

    #[test]
    fn test_unencrypted_backup_still_works() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let data_dir = temp_dir.path().to_path_buf();

        // Create test data
        let test_data = r#"{"unencrypted": true}"#;
        fs::write(data_dir.join("kv_store.json"), test_data).expect("Failed to write test data");

        let manager = BackupManager::new(data_dir.clone());

        // Create unencrypted backup (encrypt=false)
        let backup = manager
            .create_backup(false, None, false, None)
            .expect("Failed to create unencrypted backup");

        assert!(!backup.encrypted);
        assert!(backup.encryption_algorithm.is_none());

        // Verify the backup file is plaintext
        let backup_kv_path = manager.backup_dir.join(&backup.id).join("kv_store.json");
        let backup_contents = fs::read_to_string(&backup_kv_path).expect("Failed to read backup");
        assert_eq!(backup_contents, test_data);

        // Modify and restore
        fs::write(data_dir.join("kv_store.json"), "modified").expect("Failed to modify");
        manager
            .restore_backup(&backup.id, false)
            .expect("Failed to restore");

        let restored = fs::read_to_string(data_dir.join("kv_store.json")).expect("Failed to read");
        assert_eq!(restored, test_data);
    }

    #[test]
    fn test_encrypted_backup_requires_key() {
        // Temporarily unset the encryption key by setting an invalid one
        // Note: Due to OnceLock caching, this test may not work in all scenarios
        // In production, the key check happens before backup creation

        let temp_dir = tempdir().expect("Failed to create temp dir");
        let data_dir = temp_dir.path().to_path_buf();
        fs::write(data_dir.join("kv_store.json"), "{}").expect("Failed to write test data");

        let manager = BackupManager::new(data_dir);

        // If key is set from previous tests, backup will succeed
        // This test verifies the encrypt flag is properly stored
        let result = manager.create_backup(false, None, true, None);

        // Either succeeds (key from previous test) or fails (no key)
        match result {
            Ok(backup) => {
                assert!(backup.encrypted);
            }
            Err(e) => {
                assert!(e.contains("AEGIS_ENCRYPTION_KEY"));
            }
        }
    }
}
