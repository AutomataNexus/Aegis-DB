//! Binary download, verification, staging, and application.

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;
use tracing::{info, warn};

use crate::UpdateError;

/// A binary that has been downloaded, verified, and staged for deployment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StagedBinary {
    /// Path to the staged binary file.
    pub path: PathBuf,
    /// Version string for the staged binary.
    pub version: String,
    /// SHA-256 hex digest of the staged binary.
    pub sha256: String,
    /// When the binary was staged.
    pub staged_at: DateTime<Utc>,
    /// Path to the backup of the current binary, if one was created.
    pub backup_path: Option<PathBuf>,
}

/// Download a binary from `url` into the directory `dest`.
///
/// Returns the path to the downloaded file. Uses streaming to avoid loading
/// the entire binary into memory at once.
pub async fn download_binary(url: &str, dest: &Path) -> Result<PathBuf, UpdateError> {
    info!(url = url, dest = %dest.display(), "Downloading binary");

    tokio::fs::create_dir_all(dest).await.map_err(|e| {
        UpdateError::Io(format!("Failed to create download directory: {e}"))
    })?;

    let file_name = url
        .rsplit('/')
        .next()
        .unwrap_or("aegis-server");
    let download_path = dest.join(file_name);

    let client = reqwest::Client::new();
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| UpdateError::DownloadFailed(format!("HTTP request failed: {e}")))?;

    if !response.status().is_success() {
        return Err(UpdateError::DownloadFailed(format!(
            "HTTP {} from {url}",
            response.status()
        )));
    }

    let mut file = tokio::fs::File::create(&download_path).await.map_err(|e| {
        UpdateError::Io(format!("Failed to create file {}: {e}", download_path.display()))
    })?;

    let mut stream = response.bytes_stream();
    let mut total_bytes: u64 = 0;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| {
            UpdateError::DownloadFailed(format!("Stream error: {e}"))
        })?;
        file.write_all(&chunk).await.map_err(|e| {
            UpdateError::Io(format!("Write error: {e}"))
        })?;
        total_bytes += chunk.len() as u64;
    }

    file.flush().await.map_err(|e| UpdateError::Io(format!("Flush error: {e}")))?;

    info!(
        path = %download_path.display(),
        bytes = total_bytes,
        "Binary downloaded successfully"
    );

    Ok(download_path)
}

/// Compute the SHA-256 digest of the file at `path` and compare it to `expected`.
///
/// Returns `true` if the digests match, `false` otherwise.
pub fn verify_sha256(path: &Path, expected: &str) -> Result<bool, UpdateError> {
    let data = std::fs::read(path).map_err(|e| {
        UpdateError::Io(format!("Failed to read {}: {e}", path.display()))
    })?;

    let mut hasher = Sha256::new();
    hasher.update(&data);
    let actual = hex::encode(hasher.finalize());

    if actual == expected.to_lowercase() {
        info!(path = %path.display(), "SHA-256 verification passed");
        Ok(true)
    } else {
        warn!(
            path = %path.display(),
            expected = expected,
            actual = actual,
            "SHA-256 verification failed"
        );
        Ok(false)
    }
}

/// Copy the downloaded binary into the staging directory and return a `StagedBinary`.
pub fn stage_binary(
    download_path: &Path,
    stage_dir: &Path,
) -> Result<StagedBinary, UpdateError> {
    std::fs::create_dir_all(stage_dir).map_err(|e| {
        UpdateError::StagingFailed(format!("Failed to create staging directory: {e}"))
    })?;

    let file_name = download_path
        .file_name()
        .ok_or_else(|| UpdateError::StagingFailed("No file name in download path".into()))?;

    let staged_path = stage_dir.join(file_name);

    std::fs::copy(download_path, &staged_path).map_err(|e| {
        UpdateError::StagingFailed(format!(
            "Failed to copy {} -> {}: {e}",
            download_path.display(),
            staged_path.display()
        ))
    })?;

    // Compute sha256 of staged binary
    let data = std::fs::read(&staged_path).map_err(|e| {
        UpdateError::Io(format!("Failed to read staged binary: {e}"))
    })?;
    let mut hasher = Sha256::new();
    hasher.update(&data);
    let sha256 = hex::encode(hasher.finalize());

    // Make the staged binary executable on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o755);
        std::fs::set_permissions(&staged_path, perms).map_err(|e| {
            UpdateError::StagingFailed(format!("Failed to set executable permission: {e}"))
        })?;
    }

    info!(path = %staged_path.display(), sha256 = %sha256, "Binary staged");

    Ok(StagedBinary {
        path: staged_path,
        version: String::new(), // Caller should set this
        sha256,
        staged_at: Utc::now(),
        backup_path: None,
    })
}

/// Create a backup of the current binary before applying an update.
///
/// Returns the path to the backup copy.
pub fn backup_current_binary(
    current_path: &Path,
    backup_dir: &Path,
) -> Result<PathBuf, UpdateError> {
    std::fs::create_dir_all(backup_dir).map_err(|e| {
        UpdateError::Io(format!("Failed to create backup directory: {e}"))
    })?;

    let timestamp = Utc::now().format("%Y%m%d%H%M%S");
    let file_name = current_path
        .file_name()
        .unwrap_or_else(|| std::ffi::OsStr::new("aegis-server"));
    let backup_name = format!("{}.backup.{timestamp}", file_name.to_string_lossy());
    let backup_path = backup_dir.join(backup_name);

    std::fs::copy(current_path, &backup_path).map_err(|e| {
        UpdateError::Io(format!(
            "Failed to backup {} -> {}: {e}",
            current_path.display(),
            backup_path.display()
        ))
    })?;

    info!(
        source = %current_path.display(),
        backup = %backup_path.display(),
        "Current binary backed up"
    );

    Ok(backup_path)
}

/// Atomically replace the target binary with the staged binary.
///
/// Uses `std::fs::rename` for an atomic operation on the same filesystem.
/// If the staged binary is on a different filesystem, falls back to copy + remove.
pub fn apply_binary(staged: &StagedBinary, target: &Path) -> Result<(), UpdateError> {
    info!(
        staged = %staged.path.display(),
        target = %target.display(),
        "Applying staged binary"
    );

    // Try atomic rename first
    match std::fs::rename(&staged.path, target) {
        Ok(()) => {
            info!(target = %target.display(), "Binary applied via atomic rename");
            Ok(())
        }
        Err(_rename_err) => {
            // Fallback: copy then remove staged
            std::fs::copy(&staged.path, target).map_err(|e| {
                UpdateError::StagingFailed(format!(
                    "Failed to copy staged binary to target: {e}"
                ))
            })?;
            let _ = std::fs::remove_file(&staged.path);

            // Ensure executable permission on Unix
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let perms = std::fs::Permissions::from_mode(0o755);
                std::fs::set_permissions(target, perms).map_err(|e| {
                    UpdateError::StagingFailed(format!("Failed to set permissions: {e}"))
                })?;
            }

            info!(target = %target.display(), "Binary applied via copy");
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_verify_sha256_match() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test_binary");
        let mut f = std::fs::File::create(&file_path).unwrap();
        f.write_all(b"hello world").unwrap();
        drop(f);

        // SHA-256 of "hello world"
        let expected = "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9";
        assert!(verify_sha256(&file_path, expected).unwrap());
    }

    #[test]
    fn test_verify_sha256_mismatch() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test_binary");
        let mut f = std::fs::File::create(&file_path).unwrap();
        f.write_all(b"hello world").unwrap();
        drop(f);

        assert!(!verify_sha256(&file_path, "0000000000000000").unwrap());
    }

    #[test]
    fn test_stage_and_apply() {
        let dir = tempfile::tempdir().unwrap();
        let download_path = dir.path().join("aegis-server");
        std::fs::write(&download_path, b"binary content").unwrap();

        let stage_dir = dir.path().join("staging");
        let staged = stage_binary(&download_path, &stage_dir).unwrap();
        assert!(staged.path.exists());

        let target = dir.path().join("target-binary");
        apply_binary(&staged, &target).unwrap();
        assert!(target.exists());
        assert_eq!(std::fs::read(&target).unwrap(), b"binary content");
    }

    #[test]
    fn test_backup_current_binary() {
        let dir = tempfile::tempdir().unwrap();
        let current = dir.path().join("aegis-server");
        std::fs::write(&current, b"current binary").unwrap();

        let backup_dir = dir.path().join("backups");
        let backup_path = backup_current_binary(&current, &backup_dir).unwrap();
        assert!(backup_path.exists());
        assert_eq!(std::fs::read(&backup_path).unwrap(), b"current binary");
    }
}
