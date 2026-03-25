use std::collections::VecDeque;
use std::io::Write;
use std::path::PathBuf;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

/// Types of vault operations that can be audited.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum VaultOperation {
    Get,
    Set,
    Delete,
    List,
    Seal,
    Unseal,
    Rotate,
    TransitEncrypt,
    TransitDecrypt,
}

/// A single audit log entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultAuditEntry {
    pub timestamp: u64,
    pub operation: VaultOperation,
    pub key: Option<String>,
    pub component: Option<String>,
    pub success: bool,
    pub detail: Option<String>,
}

impl VaultAuditEntry {
    pub fn new(
        operation: VaultOperation,
        key: Option<&str>,
        component: Option<&str>,
        success: bool,
        detail: Option<&str>,
    ) -> Self {
        Self {
            timestamp: chrono::Utc::now().timestamp() as u64,
            operation,
            key: key.map(|s| s.to_string()),
            component: component.map(|s| s.to_string()),
            success,
            detail: detail.map(|s| s.to_string()),
        }
    }
}

/// In-memory audit log with bounded capacity and optional file persistence.
pub struct VaultAuditLog {
    entries: RwLock<VecDeque<VaultAuditEntry>>,
    max_entries: usize,
    log_file: RwLock<Option<PathBuf>>,
}

impl VaultAuditLog {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: RwLock::new(VecDeque::with_capacity(max_entries.min(1024))),
            max_entries,
            log_file: RwLock::new(None),
        }
    }

    /// Enable file-based audit log persistence. Entries are appended as JSON lines.
    pub fn set_log_file(&self, path: PathBuf) {
        *self.log_file.write() = Some(path);
    }

    /// Append an entry to the audit log file (best-effort, failures logged but not propagated).
    fn append_to_file(&self, entry: &VaultAuditEntry) {
        let log_file = self.log_file.read();
        if let Some(ref path) = *log_file {
            match std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
            {
                Ok(mut file) => {
                    if let Ok(json) = serde_json::to_string(entry) {
                        let _ = writeln!(file, "{json}");
                    }

                    // Set permissions on first creation
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        let _ = std::fs::set_permissions(
                            path,
                            std::fs::Permissions::from_mode(0o600),
                        );
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to write audit log entry to file: {e}");
                }
            }
        }
    }

    /// Record an audit entry. Evicts oldest entries when capacity is exceeded.
    /// Also appends to file if configured.
    pub fn record(&self, entry: VaultAuditEntry) {
        self.append_to_file(&entry);
        let mut entries = self.entries.write();
        if entries.len() >= self.max_entries {
            entries.pop_front();
        }
        entries.push_back(entry);
    }

    /// Record a successful operation.
    pub fn record_success(
        &self,
        operation: VaultOperation,
        key: Option<&str>,
        component: Option<&str>,
    ) {
        self.record(VaultAuditEntry::new(operation, key, component, true, None));
    }

    /// Record a failed operation.
    pub fn record_failure(
        &self,
        operation: VaultOperation,
        key: Option<&str>,
        component: Option<&str>,
        detail: &str,
    ) {
        self.record(VaultAuditEntry::new(
            operation,
            key,
            component,
            false,
            Some(detail),
        ));
    }

    /// Get the most recent entries (up to `limit`).
    pub fn entries(&self, limit: usize) -> Vec<VaultAuditEntry> {
        let entries = self.entries.read();
        entries.iter().rev().take(limit).cloned().collect()
    }

    /// Get the total number of entries.
    pub fn len(&self) -> usize {
        self.entries.read().len()
    }

    /// Check if the log is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.read().is_empty()
    }

    /// Clear all entries.
    pub fn clear(&self) {
        self.entries.write().clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_and_retrieve() {
        let log = VaultAuditLog::new(100);
        log.record_success(VaultOperation::Get, Some("key1"), Some("server"));
        log.record_success(VaultOperation::Set, Some("key2"), Some("server"));

        let entries = log.entries(10);
        assert_eq!(entries.len(), 2);
        // Most recent first
        assert_eq!(entries[0].operation, VaultOperation::Set);
        assert_eq!(entries[1].operation, VaultOperation::Get);
    }

    #[test]
    fn test_max_entries_eviction() {
        let log = VaultAuditLog::new(3);
        for i in 0..5 {
            log.record_success(
                VaultOperation::Get,
                Some(&format!("key{}", i)),
                Some("test"),
            );
        }

        assert_eq!(log.len(), 3);
        let entries = log.entries(10);
        // Should have keys 2, 3, 4 (oldest evicted)
        assert_eq!(entries[0].key.as_deref(), Some("key4"));
        assert_eq!(entries[2].key.as_deref(), Some("key2"));
    }

    #[test]
    fn test_record_failure() {
        let log = VaultAuditLog::new(100);
        log.record_failure(
            VaultOperation::Get,
            Some("secret_key"),
            Some("unauthorized"),
            "access denied",
        );

        let entries = log.entries(1);
        assert_eq!(entries.len(), 1);
        assert!(!entries[0].success);
        assert_eq!(entries[0].detail.as_deref(), Some("access denied"));
    }

    #[test]
    fn test_clear() {
        let log = VaultAuditLog::new(100);
        log.record_success(VaultOperation::Seal, None, None);
        assert!(!log.is_empty());

        log.clear();
        assert!(log.is_empty());
        assert_eq!(log.len(), 0);
    }

    #[test]
    fn test_entries_limit() {
        let log = VaultAuditLog::new(100);
        for _ in 0..10 {
            log.record_success(VaultOperation::Get, Some("k"), Some("c"));
        }

        assert_eq!(log.entries(3).len(), 3);
        assert_eq!(log.entries(100).len(), 10);
    }
}
