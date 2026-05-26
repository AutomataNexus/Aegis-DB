//! IP reputation tracking and ban management.

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Reputation record for a single IP address.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpReputation {
    pub ip: String,
    /// Score from -100 (worst) to +100 (best). Starts at 0.
    pub score: i32,
    pub total_requests: u64,
    pub failed_auths: u64,
    pub blocked_requests: u64,
    pub threat_events: u64,
    pub first_seen: i64,
    pub last_seen: i64,
    /// Epoch seconds until which this IP is banned, or None.
    pub banned_until: Option<u64>,
    pub ban_reason: Option<String>,
}

impl IpReputation {
    fn new(ip: String) -> Self {
        let now = chrono::Utc::now().timestamp();
        Self {
            ip,
            score: 0,
            total_requests: 0,
            failed_auths: 0,
            blocked_requests: 0,
            threat_events: 0,
            first_seen: now,
            last_seen: now,
            banned_until: None,
            ban_reason: None,
        }
    }

    fn clamp_score(&mut self) {
        self.score = self.score.clamp(-100, 100);
    }
}

/// Thread-safe IP reputation tracker.
pub struct IpReputationTracker {
    reputations: RwLock<HashMap<String, IpReputation>>,
}

impl Default for IpReputationTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl IpReputationTracker {
    pub fn new() -> Self {
        Self {
            reputations: RwLock::new(HashMap::new()),
        }
    }

    /// Record a normal request from an IP. Slightly improves reputation.
    pub fn record_request(&self, ip: &str) {
        let mut map = self.reputations.write();
        let entry = map
            .entry(ip.to_string())
            .or_insert_with(|| IpReputation::new(ip.to_string()));
        entry.total_requests += 1;
        entry.last_seen = chrono::Utc::now().timestamp();
        entry.score = (entry.score + 1).min(100);
    }

    /// Record a threat event. Decreases reputation by score/10.
    pub fn record_threat(&self, ip: &str, threat_score: u32) {
        let mut map = self.reputations.write();
        let entry = map
            .entry(ip.to_string())
            .or_insert_with(|| IpReputation::new(ip.to_string()));
        entry.threat_events += 1;
        entry.last_seen = chrono::Utc::now().timestamp();
        entry.score -= (threat_score / 10) as i32;
        entry.clamp_score();
    }

    /// Record a failed authentication attempt.
    pub fn record_failed_auth(&self, ip: &str) {
        let mut map = self.reputations.write();
        let entry = map
            .entry(ip.to_string())
            .or_insert_with(|| IpReputation::new(ip.to_string()));
        entry.failed_auths += 1;
        entry.last_seen = chrono::Utc::now().timestamp();
        entry.score -= 10;
        entry.clamp_score();
    }

    /// Record a blocked request.
    pub fn record_blocked(&self, ip: &str) {
        let mut map = self.reputations.write();
        let entry = map
            .entry(ip.to_string())
            .or_insert_with(|| IpReputation::new(ip.to_string()));
        entry.blocked_requests += 1;
        entry.last_seen = chrono::Utc::now().timestamp();
        entry.score -= 20;
        entry.clamp_score();
    }

    /// Get a clone of the reputation for an IP.
    pub fn get_reputation(&self, ip: &str) -> Option<IpReputation> {
        self.reputations.read().get(ip).cloned()
    }

    /// Check whether an IP is currently banned.
    pub fn is_banned(&self, ip: &str) -> bool {
        let map = self.reputations.read();
        if let Some(rep) = map.get(ip) {
            if let Some(until) = rep.banned_until {
                let now = chrono::Utc::now().timestamp() as u64;
                return now < until;
            }
        }
        false
    }

    /// Ban an IP for `duration_secs` with a given reason.
    pub fn ban(&self, ip: &str, duration_secs: u64, reason: &str) {
        let mut map = self.reputations.write();
        let entry = map
            .entry(ip.to_string())
            .or_insert_with(|| IpReputation::new(ip.to_string()));
        let now = chrono::Utc::now().timestamp() as u64;
        entry.banned_until = Some(now + duration_secs);
        entry.ban_reason = Some(reason.to_string());
    }

    /// Unban an IP. Returns true if the IP was previously banned.
    pub fn unban(&self, ip: &str) -> bool {
        let mut map = self.reputations.write();
        if let Some(entry) = map.get_mut(ip) {
            if entry.banned_until.is_some() {
                entry.banned_until = None;
                entry.ban_reason = None;
                return true;
            }
        }
        false
    }

    /// Get all currently banned IPs.
    pub fn get_all_banned(&self) -> Vec<IpReputation> {
        let now = chrono::Utc::now().timestamp() as u64;
        self.reputations
            .read()
            .values()
            .filter(|r| r.banned_until.is_some_and(|u| now < u))
            .cloned()
            .collect()
    }

    /// Remove expired bans.
    pub fn cleanup_expired_bans(&self) {
        let now = chrono::Utc::now().timestamp() as u64;
        let mut map = self.reputations.write();
        for rep in map.values_mut() {
            if let Some(until) = rep.banned_until {
                if now >= until {
                    rep.banned_until = None;
                    rep.ban_reason = None;
                }
            }
        }
    }

    /// Get the top offenders sorted by worst (lowest) score.
    pub fn get_top_offenders(&self, limit: usize) -> Vec<IpReputation> {
        let map = self.reputations.read();
        let mut entries: Vec<IpReputation> = map.values().cloned().collect();
        entries.sort_by_key(|r| r.score);
        entries.truncate(limit);
        entries
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_request_improves_score() {
        let tracker = IpReputationTracker::new();
        tracker.record_request("10.0.0.1");
        tracker.record_request("10.0.0.1");
        let rep = tracker.get_reputation("10.0.0.1").unwrap();
        assert_eq!(rep.total_requests, 2);
        assert_eq!(rep.score, 2);
    }

    #[test]
    fn test_record_threat_lowers_score() {
        let tracker = IpReputationTracker::new();
        tracker.record_request("10.0.0.1");
        tracker.record_threat("10.0.0.1", 80);
        let rep = tracker.get_reputation("10.0.0.1").unwrap();
        assert_eq!(rep.score, 1 - 8); // +1 from request, -8 from threat
        assert_eq!(rep.threat_events, 1);
    }

    #[test]
    fn test_failed_auth_decreases_score() {
        let tracker = IpReputationTracker::new();
        tracker.record_failed_auth("10.0.0.1");
        let rep = tracker.get_reputation("10.0.0.1").unwrap();
        assert_eq!(rep.score, -10);
        assert_eq!(rep.failed_auths, 1);
    }

    #[test]
    fn test_ban_and_unban() {
        let tracker = IpReputationTracker::new();
        tracker.ban("10.0.0.1", 3600, "testing");
        assert!(tracker.is_banned("10.0.0.1"));

        assert!(tracker.unban("10.0.0.1"));
        assert!(!tracker.is_banned("10.0.0.1"));
    }

    #[test]
    fn test_score_clamped() {
        let tracker = IpReputationTracker::new();
        for _ in 0..200 {
            tracker.record_request("10.0.0.1");
        }
        let rep = tracker.get_reputation("10.0.0.1").unwrap();
        assert_eq!(rep.score, 100);
    }

    #[test]
    fn test_get_top_offenders() {
        let tracker = IpReputationTracker::new();
        tracker.record_failed_auth("10.0.0.1");
        tracker.record_failed_auth("10.0.0.1");
        tracker.record_failed_auth("10.0.0.2");
        let top = tracker.get_top_offenders(2);
        assert_eq!(top.len(), 2);
        assert_eq!(top[0].ip, "10.0.0.1"); // worst score first
    }

    #[test]
    fn test_unknown_ip_not_banned() {
        let tracker = IpReputationTracker::new();
        assert!(!tracker.is_banned("192.168.1.1"));
    }
}
