//! Core threat types and events.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Severity level of a detected threat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ThreatLevel {
    Critical,
    High,
    Medium,
    Low,
    Info,
}

impl ThreatLevel {
    /// Derive a threat level from a numeric score (0-100).
    pub fn from_score(score: u32) -> Self {
        if score >= 90 {
            ThreatLevel::Critical
        } else if score >= 70 {
            ThreatLevel::High
        } else if score >= 40 {
            ThreatLevel::Medium
        } else if score >= 20 {
            ThreatLevel::Low
        } else {
            ThreatLevel::Info
        }
    }

    /// Return a human-readable label.
    pub fn as_str(&self) -> &'static str {
        match self {
            ThreatLevel::Critical => "critical",
            ThreatLevel::High => "high",
            ThreatLevel::Medium => "medium",
            ThreatLevel::Low => "low",
            ThreatLevel::Info => "info",
        }
    }
}

/// Category of the detected threat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ThreatType {
    SqlInjection,
    QueryAnomaly,
    BruteForce,
    RateLimitAbuse,
    SuspiciousFingerprint,
    ReputationBlock,
    UnauthorizedAccess,
    PortScan,
}

/// Action taken in response to a threat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ThreatAction {
    Allowed,
    RateLimited,
    Blocked,
    Banned,
}

/// A recorded threat event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreatEvent {
    pub id: String,
    pub timestamp: i64,
    pub threat_type: ThreatType,
    pub level: ThreatLevel,
    pub score: u32,
    pub source_ip: String,
    pub description: String,
    pub request_path: String,
    pub user_agent: Option<String>,
    pub details: serde_json::Value,
    pub action_taken: ThreatAction,
}

impl ThreatEvent {
    /// Create a new threat event with a generated UUID and current timestamp.
    pub fn new(
        threat_type: ThreatType,
        score: u32,
        source_ip: String,
        description: String,
        request_path: String,
        user_agent: Option<String>,
        action_taken: ThreatAction,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            threat_type,
            level: ThreatLevel::from_score(score),
            score,
            source_ip,
            description,
            request_path,
            user_agent,
            details: serde_json::Value::Null,
            action_taken,
        }
    }

    /// Attach extra JSON details.
    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = details;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_threat_level_from_score() {
        assert_eq!(ThreatLevel::from_score(100), ThreatLevel::Critical);
        assert_eq!(ThreatLevel::from_score(90), ThreatLevel::Critical);
        assert_eq!(ThreatLevel::from_score(89), ThreatLevel::High);
        assert_eq!(ThreatLevel::from_score(70), ThreatLevel::High);
        assert_eq!(ThreatLevel::from_score(69), ThreatLevel::Medium);
        assert_eq!(ThreatLevel::from_score(40), ThreatLevel::Medium);
        assert_eq!(ThreatLevel::from_score(39), ThreatLevel::Low);
        assert_eq!(ThreatLevel::from_score(20), ThreatLevel::Low);
        assert_eq!(ThreatLevel::from_score(19), ThreatLevel::Info);
        assert_eq!(ThreatLevel::from_score(0), ThreatLevel::Info);
    }

    #[test]
    fn test_threat_event_creation() {
        let event = ThreatEvent::new(
            ThreatType::SqlInjection,
            85,
            "10.0.0.1".to_string(),
            "SQL injection detected".to_string(),
            "/api/v1/query".to_string(),
            Some("curl/7.81".to_string()),
            ThreatAction::Blocked,
        );
        assert_eq!(event.level, ThreatLevel::High);
        assert_eq!(event.score, 85);
        assert!(!event.id.is_empty());
    }

    #[test]
    fn test_threat_event_with_details() {
        let event = ThreatEvent::new(
            ThreatType::BruteForce,
            50,
            "10.0.0.2".to_string(),
            "brute force".to_string(),
            "/api/v1/auth/login".to_string(),
            None,
            ThreatAction::RateLimited,
        )
        .with_details(serde_json::json!({"attempts": 15}));
        assert_eq!(event.details["attempts"], 15);
    }

    #[test]
    fn test_threat_level_as_str() {
        assert_eq!(ThreatLevel::Critical.as_str(), "critical");
        assert_eq!(ThreatLevel::Info.as_str(), "info");
    }
}
