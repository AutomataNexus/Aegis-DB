//! Aegis Activity Logging Module
//!
//! Tracks and stores system activity for auditing and monitoring.
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

// =============================================================================
// Activity Types
// =============================================================================

/// Type of activity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ActivityType {
    Query,
    Write,
    Delete,
    Config,
    Node,
    Auth,
    System,
}

impl std::fmt::Display for ActivityType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ActivityType::Query => write!(f, "query"),
            ActivityType::Write => write!(f, "write"),
            ActivityType::Delete => write!(f, "delete"),
            ActivityType::Config => write!(f, "config"),
            ActivityType::Node => write!(f, "node"),
            ActivityType::Auth => write!(f, "auth"),
            ActivityType::System => write!(f, "system"),
        }
    }
}

/// Activity entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Activity {
    pub id: String,
    #[serde(rename = "type")]
    pub activity_type: ActivityType,
    pub description: String,
    pub timestamp: String,
    pub duration: Option<u64>,
    pub user: Option<String>,
    pub source: Option<String>,
    pub details: Option<serde_json::Value>,
}

/// Internal activity record with instant for TTL.
#[derive(Debug, Clone)]
struct ActivityRecord {
    activity: Activity,
    created_at: Instant,
}

// =============================================================================
// Activity Logger Service
// =============================================================================

/// Activity logging service.
pub struct ActivityLogger {
    activities: RwLock<VecDeque<ActivityRecord>>,
    next_id: AtomicU64,
    max_entries: usize,
    retention_duration: Duration,
}

impl ActivityLogger {
    /// Create a new activity logger.
    pub fn new() -> Self {
        Self {
            activities: RwLock::new(VecDeque::with_capacity(1000)),
            next_id: AtomicU64::new(1),
            max_entries: 1000,
            retention_duration: Duration::from_secs(24 * 60 * 60), // 24 hours
        }
    }

    /// Log a new activity.
    pub fn log(&self, activity_type: ActivityType, description: &str) -> String {
        self.log_with_details(activity_type, description, None, None, None, None)
    }

    /// Log a query activity with duration.
    pub fn log_query(&self, sql: &str, duration_ms: u64, user: Option<&str>) {
        self.log_with_details(
            ActivityType::Query,
            sql,
            Some(duration_ms),
            user,
            None,
            None,
        );
    }

    /// Log a write activity.
    pub fn log_write(&self, description: &str, user: Option<&str>) {
        self.log_with_details(
            ActivityType::Write,
            description,
            None,
            user,
            None,
            None,
        );
    }

    /// Log a configuration change.
    pub fn log_config(&self, description: &str, user: Option<&str>) {
        self.log_with_details(
            ActivityType::Config,
            description,
            None,
            user,
            None,
            None,
        );
    }

    /// Log a node event.
    pub fn log_node(&self, description: &str) {
        self.log_with_details(
            ActivityType::Node,
            description,
            None,
            None,
            Some("cluster"),
            None,
        );
    }

    /// Log an authentication event.
    pub fn log_auth(&self, description: &str, user: Option<&str>) {
        self.log_with_details(
            ActivityType::Auth,
            description,
            None,
            user,
            None,
            None,
        );
    }

    /// Log a system event.
    pub fn log_system(&self, description: &str) {
        self.log_with_details(
            ActivityType::System,
            description,
            None,
            None,
            Some("system"),
            None,
        );
    }

    /// Log an activity with full details.
    pub fn log_with_details(
        &self,
        activity_type: ActivityType,
        description: &str,
        duration: Option<u64>,
        user: Option<&str>,
        source: Option<&str>,
        details: Option<serde_json::Value>,
    ) -> String {
        let id = format!("act-{:08}", self.next_id.fetch_add(1, Ordering::SeqCst));
        let activity = Activity {
            id: id.clone(),
            activity_type,
            description: description.to_string(),
            timestamp: format_timestamp(now_timestamp()),
            duration,
            user: user.map(|s| s.to_string()),
            source: source.map(|s| s.to_string()),
            details,
        };

        let record = ActivityRecord {
            activity,
            created_at: Instant::now(),
        };

        let mut activities = self.activities.write();

        // Remove old entries if at capacity
        while activities.len() >= self.max_entries {
            activities.pop_front();
        }

        activities.push_back(record);
        id
    }

    /// Get recent activities.
    pub fn get_recent(&self, limit: usize) -> Vec<Activity> {
        self.cleanup_expired();

        let activities = self.activities.read();
        activities
            .iter()
            .rev()
            .take(limit)
            .map(|r| r.activity.clone())
            .collect()
    }

    /// Get activities by type.
    pub fn get_by_type(&self, activity_type: ActivityType, limit: usize) -> Vec<Activity> {
        self.cleanup_expired();

        let activities = self.activities.read();
        activities
            .iter()
            .rev()
            .filter(|r| r.activity.activity_type == activity_type)
            .take(limit)
            .map(|r| r.activity.clone())
            .collect()
    }

    /// Get activities by user.
    pub fn get_by_user(&self, username: &str, limit: usize) -> Vec<Activity> {
        self.cleanup_expired();

        let activities = self.activities.read();
        activities
            .iter()
            .rev()
            .filter(|r| r.activity.user.as_deref() == Some(username))
            .take(limit)
            .map(|r| r.activity.clone())
            .collect()
    }

    /// Get activity count.
    pub fn count(&self) -> usize {
        self.activities.read().len()
    }

    /// Clean up expired activities.
    fn cleanup_expired(&self) {
        let now = Instant::now();
        let mut activities = self.activities.write();

        while let Some(front) = activities.front() {
            if now.duration_since(front.created_at) > self.retention_duration {
                activities.pop_front();
            } else {
                break;
            }
        }
    }

    /// Clear all activities.
    pub fn clear(&self) {
        self.activities.write().clear();
    }
}

impl Default for ActivityLogger {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Get current timestamp in milliseconds.
fn now_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Format a timestamp to RFC3339 string.
fn format_timestamp(timestamp_ms: u64) -> String {
    let secs = timestamp_ms / 1000;
    let datetime = UNIX_EPOCH + Duration::from_secs(secs);
    let duration = datetime.duration_since(UNIX_EPOCH).unwrap_or_default();
    let total_secs = duration.as_secs();

    let days_since_epoch = total_secs / 86400;
    let secs_today = total_secs % 86400;

    let hours = secs_today / 3600;
    let minutes = (secs_today % 3600) / 60;
    let seconds = secs_today % 60;

    let mut year = 1970u64;
    let mut remaining_days = days_since_epoch;

    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if remaining_days < days_in_year {
            break;
        }
        remaining_days -= days_in_year;
        year += 1;
    }

    let days_in_months: [u64; 12] = if is_leap_year(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };

    let mut month = 1u64;
    for &days in &days_in_months {
        if remaining_days < days {
            break;
        }
        remaining_days -= days;
        month += 1;
    }
    let day = remaining_days + 1;

    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", year, month, day, hours, minutes, seconds)
}

fn is_leap_year(year: u64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_activity() {
        let logger = ActivityLogger::new();
        let id = logger.log(ActivityType::Query, "SELECT * FROM users");
        assert!(!id.is_empty());
        assert_eq!(logger.count(), 1);
    }

    #[test]
    fn test_get_recent() {
        let logger = ActivityLogger::new();
        logger.log(ActivityType::Query, "Query 1");
        logger.log(ActivityType::Write, "Write 1");
        logger.log(ActivityType::Query, "Query 2");

        let recent = logger.get_recent(2);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].description, "Query 2");
        assert_eq!(recent[1].description, "Write 1");
    }

    #[test]
    fn test_get_by_type() {
        let logger = ActivityLogger::new();
        logger.log(ActivityType::Query, "Query 1");
        logger.log(ActivityType::Write, "Write 1");
        logger.log(ActivityType::Query, "Query 2");

        let queries = logger.get_by_type(ActivityType::Query, 10);
        assert_eq!(queries.len(), 2);
    }

    #[test]
    fn test_log_query_with_duration() {
        let logger = ActivityLogger::new();
        logger.log_query("SELECT * FROM metrics", 42, Some("admin"));

        let recent = logger.get_recent(1);
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].duration, Some(42));
        assert_eq!(recent[0].user, Some("admin".to_string()));
    }

    #[test]
    fn test_max_entries() {
        let logger = ActivityLogger::new();

        // Log more than max entries
        for i in 0..1100 {
            logger.log(ActivityType::Query, &format!("Query {}", i));
        }

        // Should be capped at max
        assert!(logger.count() <= 1000);
    }
}
