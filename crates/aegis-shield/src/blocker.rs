//! Auto-blocker with allowlist support.

use crate::threat::ThreatLevel;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// An active block entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockEntry {
    pub ip: String,
    pub reason: String,
    pub blocked_at: u64,
    pub expires_at: Option<u64>,
    pub threat_level: ThreatLevel,
}

/// Manages IP blocking and allowlisting.
pub struct AutoBlocker {
    blocked: RwLock<HashMap<String, BlockEntry>>,
    allowlist: RwLock<HashSet<String>>,
}

impl Default for AutoBlocker {
    fn default() -> Self {
        Self::new()
    }
}

impl AutoBlocker {
    pub fn new() -> Self {
        Self {
            blocked: RwLock::new(HashMap::new()),
            allowlist: RwLock::new(HashSet::new()),
        }
    }

    /// Check if an IP is currently blocked. Returns the block entry if so.
    pub fn should_block(&self, ip: &str) -> Option<BlockEntry> {
        // Allowlisted IPs are never blocked
        if self.is_allowlisted(ip) {
            return None;
        }

        let map = self.blocked.read();
        if let Some(entry) = map.get(ip) {
            let now = chrono::Utc::now().timestamp() as u64;
            if let Some(expires) = entry.expires_at {
                if now >= expires {
                    return None; // expired
                }
            }
            return Some(entry.clone());
        }
        None
    }

    /// Block an IP with optional expiry.
    pub fn block(
        &self,
        ip: &str,
        reason: &str,
        duration_secs: Option<u64>,
        threat_level: ThreatLevel,
    ) {
        let now = chrono::Utc::now().timestamp() as u64;
        let entry = BlockEntry {
            ip: ip.to_string(),
            reason: reason.to_string(),
            blocked_at: now,
            expires_at: duration_secs.map(|d| now + d),
            threat_level,
        };
        self.blocked.write().insert(ip.to_string(), entry);
    }

    /// Remove a block. Returns true if a block was removed.
    pub fn unblock(&self, ip: &str) -> bool {
        self.blocked.write().remove(ip).is_some()
    }

    /// Add an IP to the allowlist.
    pub fn add_to_allowlist(&self, ip: &str) {
        self.allowlist.write().insert(ip.to_string());
    }

    /// Remove an IP from the allowlist.
    pub fn remove_from_allowlist(&self, ip: &str) {
        self.allowlist.write().remove(ip);
    }

    /// Check if an IP is on the allowlist.
    pub fn is_allowlisted(&self, ip: &str) -> bool {
        self.allowlist.read().contains(ip)
    }

    /// Get all currently active (non-expired) block entries.
    pub fn get_blocked(&self) -> Vec<BlockEntry> {
        let now = chrono::Utc::now().timestamp() as u64;
        self.blocked
            .read()
            .values()
            .filter(|e| e.expires_at.is_none_or(|exp| now < exp))
            .cloned()
            .collect()
    }

    /// Get all allowlisted IPs.
    pub fn get_allowlist(&self) -> Vec<String> {
        self.allowlist.read().iter().cloned().collect()
    }

    /// Remove expired block entries.
    pub fn cleanup_expired(&self) {
        let now = chrono::Utc::now().timestamp() as u64;
        self.blocked
            .write()
            .retain(|_, e| e.expires_at.is_none_or(|exp| now < exp));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_block_and_check() {
        let blocker = AutoBlocker::new();
        blocker.block("10.0.0.1", "testing", Some(3600), ThreatLevel::High);
        assert!(blocker.should_block("10.0.0.1").is_some());
        assert!(blocker.should_block("10.0.0.2").is_none());
    }

    #[test]
    fn test_unblock() {
        let blocker = AutoBlocker::new();
        blocker.block("10.0.0.1", "test", Some(3600), ThreatLevel::Medium);
        assert!(blocker.unblock("10.0.0.1"));
        assert!(blocker.should_block("10.0.0.1").is_none());
    }

    #[test]
    fn test_allowlist_bypasses_block() {
        let blocker = AutoBlocker::new();
        blocker.block("10.0.0.1", "test", Some(3600), ThreatLevel::High);
        blocker.add_to_allowlist("10.0.0.1");
        assert!(blocker.should_block("10.0.0.1").is_none());
    }

    #[test]
    fn test_allowlist_operations() {
        let blocker = AutoBlocker::new();
        blocker.add_to_allowlist("127.0.0.1");
        assert!(blocker.is_allowlisted("127.0.0.1"));
        blocker.remove_from_allowlist("127.0.0.1");
        assert!(!blocker.is_allowlisted("127.0.0.1"));
    }

    #[test]
    fn test_get_blocked_list() {
        let blocker = AutoBlocker::new();
        blocker.block("10.0.0.1", "a", Some(3600), ThreatLevel::Low);
        blocker.block("10.0.0.2", "b", Some(3600), ThreatLevel::Medium);
        let list = blocker.get_blocked();
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn test_permanent_block() {
        let blocker = AutoBlocker::new();
        blocker.block("10.0.0.1", "permanent", None, ThreatLevel::Critical);
        assert!(blocker.should_block("10.0.0.1").is_some());
    }
}
