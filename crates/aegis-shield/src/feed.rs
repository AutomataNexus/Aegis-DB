//! Threat event feed and statistics.

use crate::threat::{ThreatEvent, ThreatLevel};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};

/// Aggregated threat statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreatStats {
    pub total_events: u64,
    pub events_by_level: HashMap<String, u64>,
    pub events_by_type: HashMap<String, u64>,
    pub blocked_ips_count: u64,
    pub active_bans: u64,
    pub top_offending_ips: Vec<(String, u32)>,
    pub last_critical_event: Option<i64>,
}

impl ThreatStats {
    fn new() -> Self {
        Self {
            total_events: 0,
            events_by_level: HashMap::new(),
            events_by_type: HashMap::new(),
            blocked_ips_count: 0,
            active_bans: 0,
            top_offending_ips: Vec::new(),
            last_critical_event: None,
        }
    }
}

/// In-memory threat event feed with rolling window and stats.
pub struct ThreatFeed {
    events: RwLock<VecDeque<ThreatEvent>>,
    stats: RwLock<ThreatStats>,
    max_events: usize,
    /// Per-IP event count for top offenders tracking.
    ip_event_counts: RwLock<HashMap<String, u32>>,
}

impl ThreatFeed {
    pub fn new(max_events: usize) -> Self {
        Self {
            events: RwLock::new(VecDeque::with_capacity(max_events.min(10000))),
            stats: RwLock::new(ThreatStats::new()),
            max_events,
            ip_event_counts: RwLock::new(HashMap::new()),
        }
    }

    /// Record a new threat event.
    pub fn record_event(&self, event: ThreatEvent) {
        // Update stats
        {
            let mut stats = self.stats.write();
            stats.total_events += 1;

            let level_key = event.level.as_str().to_string();
            *stats.events_by_level.entry(level_key).or_insert(0) += 1;

            let type_key = format!("{:?}", event.threat_type);
            *stats.events_by_type.entry(type_key).or_insert(0) += 1;

            if event.level == ThreatLevel::Critical {
                stats.last_critical_event = Some(event.timestamp);
            }
        }

        // Track per-IP counts
        {
            let mut counts = self.ip_event_counts.write();
            *counts.entry(event.source_ip.clone()).or_insert(0) += 1;
        }

        // Add event to ring buffer
        {
            let mut events = self.events.write();
            if events.len() >= self.max_events {
                events.pop_front();
            }
            events.push_back(event);
        }
    }

    /// Get the most recent events (up to `limit`).
    pub fn get_recent(&self, limit: usize) -> Vec<ThreatEvent> {
        let events = self.events.read();
        events.iter().rev().take(limit).cloned().collect()
    }

    /// Get current statistics. Includes top offending IPs.
    pub fn get_stats(&self) -> ThreatStats {
        let mut stats = self.stats.read().clone();

        // Compute top offending IPs
        let counts = self.ip_event_counts.read();
        let mut entries: Vec<(String, u32)> = counts
            .iter()
            .map(|(ip, count)| (ip.clone(), *count))
            .collect();
        entries.sort_by_key(|e| std::cmp::Reverse(e.1));
        entries.truncate(10);
        stats.top_offending_ips = entries;

        stats
    }

    /// Update the blocked/banned counts (called externally).
    pub fn update_block_stats(&self, blocked_count: u64, ban_count: u64) {
        let mut stats = self.stats.write();
        stats.blocked_ips_count = blocked_count;
        stats.active_bans = ban_count;
    }

    /// Look up a single event by ID.
    pub fn get_event(&self, id: &str) -> Option<ThreatEvent> {
        self.events.read().iter().find(|e| e.id == id).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::threat::{ThreatAction, ThreatEvent, ThreatType};

    fn make_event(ip: &str, score: u32, tt: ThreatType) -> ThreatEvent {
        ThreatEvent::new(
            tt,
            score,
            ip.to_string(),
            "test".to_string(),
            "/test".to_string(),
            None,
            ThreatAction::Blocked,
        )
    }

    #[test]
    fn test_record_and_get_recent() {
        let feed = ThreatFeed::new(100);
        feed.record_event(make_event("10.0.0.1", 50, ThreatType::SqlInjection));
        feed.record_event(make_event("10.0.0.2", 60, ThreatType::BruteForce));
        let recent = feed.get_recent(10);
        assert_eq!(recent.len(), 2);
        // Most recent first
        assert_eq!(recent[0].source_ip, "10.0.0.2");
    }

    #[test]
    fn test_stats_updated() {
        let feed = ThreatFeed::new(100);
        feed.record_event(make_event("10.0.0.1", 95, ThreatType::SqlInjection));
        let stats = feed.get_stats();
        assert_eq!(stats.total_events, 1);
        assert_eq!(*stats.events_by_type.get("SqlInjection").unwrap(), 1);
        assert!(stats.last_critical_event.is_some());
    }

    #[test]
    fn test_max_events_eviction() {
        let feed = ThreatFeed::new(3);
        for i in 0..5 {
            feed.record_event(make_event(
                &format!("10.0.0.{}", i),
                30,
                ThreatType::QueryAnomaly,
            ));
        }
        let recent = feed.get_recent(100);
        assert_eq!(recent.len(), 3);
    }

    #[test]
    fn test_top_offending_ips() {
        let feed = ThreatFeed::new(100);
        for _ in 0..5 {
            feed.record_event(make_event("10.0.0.1", 50, ThreatType::BruteForce));
        }
        feed.record_event(make_event("10.0.0.2", 50, ThreatType::BruteForce));
        let stats = feed.get_stats();
        assert_eq!(stats.top_offending_ips[0].0, "10.0.0.1");
        assert_eq!(stats.top_offending_ips[0].1, 5);
    }

    #[test]
    fn test_get_event_by_id() {
        let feed = ThreatFeed::new(100);
        let event = make_event("10.0.0.1", 50, ThreatType::SqlInjection);
        let id = event.id.clone();
        feed.record_event(event);
        assert!(feed.get_event(&id).is_some());
        assert!(feed.get_event("nonexistent").is_none());
    }
}
