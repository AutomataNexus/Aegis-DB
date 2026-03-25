//! Aegis Shield - Integrated security shield for Aegis database.
//!
//! Provides SQL injection detection, IP reputation tracking, request
//! fingerprinting, anomaly detection, auto-blocking, and a live threat feed.

pub mod anomaly;
pub mod blocker;
pub mod config;
pub mod error;
pub mod feed;
pub mod fingerprint;
pub mod ip_reputation;
pub mod policy;
pub mod sql_injection;
pub mod threat;

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

// Re-export key types for external use
pub use crate::anomaly::QueryAnomalyDetector;
pub use crate::blocker::{AutoBlocker, BlockEntry};
pub use crate::config::{SecurityPreset, ShieldConfig};
pub use crate::feed::{ThreatFeed, ThreatStats};
pub use crate::fingerprint::{RequestFingerprinter, UserAgentClass};
pub use crate::ip_reputation::{IpReputation, IpReputationTracker};
pub use crate::policy::SecurityPolicy;
pub use crate::sql_injection::SqlInjectionDetector;
pub use crate::threat::{ThreatAction, ThreatEvent, ThreatLevel, ThreatType};

/// Contextual information about an incoming request.
#[derive(Debug, Clone)]
pub struct RequestContext {
    pub source_ip: String,
    pub path: String,
    pub method: String,
    pub user_agent: Option<String>,
    pub auth_user: Option<String>,
    pub body_size: usize,
    pub headers: HashMap<String, String>,
}

/// The verdict returned by the shield after analyzing a request or query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ShieldVerdict {
    Allow,
    Block {
        reason: String,
        threat_level: ThreatLevel,
    },
    RateLimit {
        delay_ms: u64,
    },
}

/// Summary status of the shield engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShieldStatus {
    pub enabled: bool,
    pub preset: SecurityPreset,
    pub uptime_secs: u64,
    pub total_requests_analyzed: u64,
    pub total_threats_detected: u64,
    pub active_bans: usize,
    pub blocked_ips: usize,
}

/// The main security shield facade.
pub struct ShieldEngine {
    config: RwLock<ShieldConfig>,
    sql_detector: SqlInjectionDetector,
    anomaly_detector: QueryAnomalyDetector,
    ip_reputation: IpReputationTracker,
    fingerprinter: RequestFingerprinter,
    blocker: AutoBlocker,
    policy: RwLock<SecurityPolicy>,
    feed: ThreatFeed,
    started_at: Instant,
    total_requests: AtomicU64,
    total_threats: AtomicU64,
}

impl ShieldEngine {
    /// Create a new shield engine with the given configuration.
    pub fn new(config: ShieldConfig) -> Self {
        let policy = SecurityPolicy::from_preset(config.preset);
        let anomaly = QueryAnomalyDetector::new(
            config.anomaly_learning_period_secs,
            config.anomaly_deviation_threshold,
        );
        let feed = ThreatFeed::new(config.max_events_in_memory);

        info!(
            "Shield engine initialized: preset={:?}, auto_block={}",
            config.preset, config.auto_blocking_enabled
        );

        Self {
            config: RwLock::new(config),
            sql_detector: SqlInjectionDetector::new(),
            anomaly_detector: anomaly,
            ip_reputation: IpReputationTracker::new(),
            fingerprinter: RequestFingerprinter::new(),
            blocker: AutoBlocker::new(),
            policy: RwLock::new(policy),
            feed,
            started_at: Instant::now(),
            total_requests: AtomicU64::new(0),
            total_threats: AtomicU64::new(0),
        }
    }

    /// Analyze an incoming request. Called by middleware BEFORE handlers.
    pub fn analyze_request(&self, ctx: &RequestContext) -> ShieldVerdict {
        let config = self.config.read().clone();
        if !config.enabled {
            return ShieldVerdict::Allow;
        }

        self.total_requests.fetch_add(1, Ordering::Relaxed);

        // 1. Allowlist bypass
        if self.blocker.is_allowlisted(&ctx.source_ip) {
            debug!(ip = %ctx.source_ip, "allowlisted, skipping checks");
            return ShieldVerdict::Allow;
        }

        // 2. Check if IP is banned
        if self.ip_reputation.is_banned(&ctx.source_ip) {
            self.ip_reputation.record_blocked(&ctx.source_ip);
            self.record_threat_event(
                ThreatType::ReputationBlock,
                90,
                ctx,
                "IP is banned".to_string(),
                ThreatAction::Blocked,
            );
            return ShieldVerdict::Block {
                reason: "IP is banned".to_string(),
                threat_level: ThreatLevel::High,
            };
        }

        // 3. Check auto-blocker
        if let Some(entry) = self.blocker.should_block(&ctx.source_ip) {
            self.ip_reputation.record_blocked(&ctx.source_ip);
            return ShieldVerdict::Block {
                reason: entry.reason.clone(),
                threat_level: entry.threat_level,
            };
        }

        let mut combined_score: u32 = 0;

        // 4. Fingerprint the request
        if config.fingerprinting_enabled {
            let fp = self
                .fingerprinter
                .analyze(ctx.user_agent.as_deref(), &ctx.headers);
            if fp.user_agent_class == UserAgentClass::Scanner {
                combined_score += fp.suspicion_score;
                self.record_threat_event(
                    ThreatType::SuspiciousFingerprint,
                    fp.suspicion_score,
                    ctx,
                    format!("scanner detected: {:?}", fp.user_agent_class),
                    ThreatAction::Blocked,
                );
            } else {
                combined_score += fp.suspicion_score / 2;
            }
        }

        // 5. IP reputation check
        if config.ip_reputation_enabled {
            self.ip_reputation.record_request(&ctx.source_ip);
            if let Some(rep) = self.ip_reputation.get_reputation(&ctx.source_ip) {
                if rep.score < -50 {
                    combined_score += 30;
                } else if rep.score < -20 {
                    combined_score += 15;
                }
            }
        }

        let capped = combined_score.min(100);

        // 6. Apply policy
        let action = self.policy.read().evaluate(capped, Some(&ctx.path));
        match action {
            ThreatAction::Blocked | ThreatAction::Banned => {
                let level = ThreatLevel::from_score(capped);
                self.total_threats.fetch_add(1, Ordering::Relaxed);

                // Auto-block if enabled and above threshold
                if config.auto_blocking_enabled && capped >= config.auto_block_threshold {
                    self.blocker.block(
                        &ctx.source_ip,
                        "auto-blocked by shield",
                        Some(config.default_ban_duration_secs),
                        level,
                    );
                    self.ip_reputation.ban(
                        &ctx.source_ip,
                        config.default_ban_duration_secs,
                        "auto-blocked",
                    );
                }

                self.record_threat_event(
                    ThreatType::SuspiciousFingerprint,
                    capped,
                    ctx,
                    format!("request blocked (score={})", capped),
                    ThreatAction::Blocked,
                );

                ShieldVerdict::Block {
                    reason: format!("threat score {} exceeds threshold", capped),
                    threat_level: level,
                }
            }
            ThreatAction::RateLimited => {
                self.total_threats.fetch_add(1, Ordering::Relaxed);
                ShieldVerdict::RateLimit {
                    delay_ms: (capped as u64) * 10,
                }
            }
            ThreatAction::Allowed => ShieldVerdict::Allow,
        }
    }

    /// Analyze a SQL query for injection. Called from query handler.
    pub fn analyze_query(&self, query: &str, ctx: &RequestContext) -> ShieldVerdict {
        let config = self.config.read().clone();
        if !config.enabled {
            return ShieldVerdict::Allow;
        }

        let mut combined_score: u32 = 0;

        // 1. SQL injection detection
        if config.sql_injection_enabled {
            let result = self.sql_detector.analyze(query);
            if result.is_suspicious {
                combined_score += result.score;
                if result.score >= 40 {
                    self.record_threat_event(
                        ThreatType::SqlInjection,
                        result.score,
                        ctx,
                        format!(
                            "SQL injection patterns: {}",
                            result.matched_patterns.join(", ")
                        ),
                        ThreatAction::Blocked,
                    );
                }
            }
        }

        // 2. Anomaly detection
        if config.anomaly_detection_enabled {
            let identifier = ctx.auth_user.as_deref().unwrap_or(&ctx.source_ip);
            self.anomaly_detector.record_query(identifier, None);
            // Use a simple rate estimate (1 query per call)
            let result = self.anomaly_detector.analyze(identifier, 1.0);
            if result.is_anomalous {
                combined_score += result.score;
                if result.score >= 30 {
                    self.record_threat_event(
                        ThreatType::QueryAnomaly,
                        result.score,
                        ctx,
                        format!("anomaly: {}", result.reasons.join("; ")),
                        ThreatAction::RateLimited,
                    );
                }
            }
        }

        let capped = combined_score.min(100);

        // 3. Apply policy
        let action = self.policy.read().evaluate(capped, Some(&ctx.path));
        match action {
            ThreatAction::Blocked | ThreatAction::Banned => {
                let level = ThreatLevel::from_score(capped);
                self.total_threats.fetch_add(1, Ordering::Relaxed);

                self.ip_reputation.record_threat(&ctx.source_ip, capped);

                if config.auto_blocking_enabled && capped >= config.auto_block_threshold {
                    self.blocker.block(
                        &ctx.source_ip,
                        &format!("SQL injection score {}", capped),
                        Some(config.default_ban_duration_secs),
                        level,
                    );
                    self.ip_reputation.ban(
                        &ctx.source_ip,
                        config.default_ban_duration_secs,
                        "sql injection",
                    );
                }

                ShieldVerdict::Block {
                    reason: format!("query blocked (score={})", capped),
                    threat_level: level,
                }
            }
            ThreatAction::RateLimited => {
                self.total_threats.fetch_add(1, Ordering::Relaxed);
                self.ip_reputation.record_threat(&ctx.source_ip, capped);
                ShieldVerdict::RateLimit {
                    delay_ms: (capped as u64) * 10,
                }
            }
            ThreatAction::Allowed => ShieldVerdict::Allow,
        }
    }

    /// Record a failed authentication attempt.
    pub fn record_failed_auth(&self, ip: &str, username: &str) {
        self.ip_reputation.record_failed_auth(ip);
        self.total_threats.fetch_add(1, Ordering::Relaxed);

        let event = ThreatEvent::new(
            ThreatType::BruteForce,
            50,
            ip.to_string(),
            format!("failed auth for user '{}'", username),
            "/api/v1/auth/login".to_string(),
            None,
            ThreatAction::Allowed,
        )
        .with_details(serde_json::json!({ "username": username }));
        self.feed.record_event(event);

        // Check if we should auto-ban after repeated failures
        let config = self.config.read().clone();
        if config.auto_blocking_enabled {
            if let Some(rep) = self.ip_reputation.get_reputation(ip) {
                if rep.failed_auths >= 10 {
                    warn!(ip = %ip, "auto-banning after {} failed auths", rep.failed_auths);
                    self.blocker.block(
                        ip,
                        "brute force detected",
                        Some(config.default_ban_duration_secs),
                        ThreatLevel::High,
                    );
                    self.ip_reputation
                        .ban(ip, config.default_ban_duration_secs, "brute force");
                }
            }
        }
    }

    /// Record a successful request (improves reputation).
    pub fn record_success(&self, ctx: &RequestContext) {
        self.ip_reputation.record_request(&ctx.source_ip);
    }

    // ---- Dashboard / API methods ----

    /// Get aggregated threat statistics.
    pub fn get_stats(&self) -> ThreatStats {
        let blocked = self.blocker.get_blocked().len() as u64;
        let bans = self.ip_reputation.get_all_banned().len() as u64;
        self.feed.update_block_stats(blocked, bans);
        self.feed.get_stats()
    }

    /// Get the most recent threat events.
    pub fn get_recent_events(&self, limit: usize) -> Vec<ThreatEvent> {
        self.feed.get_recent(limit)
    }

    /// Get all currently blocked IPs.
    pub fn get_blocked_ips(&self) -> Vec<BlockEntry> {
        self.blocker.get_blocked()
    }

    /// Unblock an IP. Returns true if it was blocked.
    pub fn unblock_ip(&self, ip: &str) -> bool {
        let a = self.blocker.unblock(ip);
        let b = self.ip_reputation.unban(ip);
        a || b
    }

    /// Add an IP to the allowlist.
    pub fn add_to_allowlist(&self, ip: &str) {
        self.blocker.add_to_allowlist(ip);
    }

    /// Remove an IP from the allowlist.
    pub fn remove_from_allowlist(&self, ip: &str) {
        self.blocker.remove_from_allowlist(ip);
    }

    /// Get the allowlist.
    pub fn get_allowlist(&self) -> Vec<String> {
        self.blocker.get_allowlist()
    }

    /// Get IP reputation details.
    pub fn get_ip_reputation(&self, ip: &str) -> Option<IpReputation> {
        self.ip_reputation.get_reputation(ip)
    }

    /// Replace the active security policy.
    pub fn update_policy(&self, policy: SecurityPolicy) {
        *self.policy.write() = policy;
    }

    /// Get a clone of the active security policy.
    pub fn get_policy(&self) -> SecurityPolicy {
        self.policy.read().clone()
    }

    /// Get the current shield status summary.
    pub fn get_status(&self) -> ShieldStatus {
        let config = self.config.read();
        ShieldStatus {
            enabled: config.enabled,
            preset: config.preset,
            uptime_secs: self.started_at.elapsed().as_secs(),
            total_requests_analyzed: self.total_requests.load(Ordering::Relaxed),
            total_threats_detected: self.total_threats.load(Ordering::Relaxed),
            active_bans: self.ip_reputation.get_all_banned().len(),
            blocked_ips: self.blocker.get_blocked().len(),
        }
    }

    /// Manually block an IP address.
    pub fn manual_block(&self, ip: &str, reason: &str, duration_secs: u64) {
        self.blocker
            .block(ip, reason, Some(duration_secs), ThreatLevel::High);
        self.ip_reputation.record_blocked(ip);
    }

    /// Clean up expired blocks and bans.
    pub fn cleanup_expired(&self) {
        self.blocker.cleanup_expired();
        self.ip_reputation.cleanup_expired_bans();
    }

    // ---- Internal helpers ----

    fn record_threat_event(
        &self,
        threat_type: ThreatType,
        score: u32,
        ctx: &RequestContext,
        description: String,
        action: ThreatAction,
    ) {
        let event = ThreatEvent::new(
            threat_type,
            score,
            ctx.source_ip.clone(),
            description,
            ctx.path.clone(),
            ctx.user_agent.clone(),
            action,
        );
        self.feed.record_event(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_ctx(ip: &str) -> RequestContext {
        RequestContext {
            source_ip: ip.to_string(),
            path: "/api/v1/query".to_string(),
            method: "POST".to_string(),
            user_agent: Some("Mozilla/5.0 Chrome/120".to_string()),
            auth_user: None,
            body_size: 100,
            headers: HashMap::new(),
        }
    }

    #[test]
    fn test_engine_creation() {
        let engine = ShieldEngine::new(ShieldConfig::default());
        let status = engine.get_status();
        assert!(status.enabled);
        assert_eq!(status.preset, SecurityPreset::Moderate);
    }

    #[test]
    fn test_allow_clean_request() {
        let engine = ShieldEngine::new(ShieldConfig::default());
        let ctx = test_ctx("10.0.0.1");
        let verdict = engine.analyze_request(&ctx);
        assert!(matches!(verdict, ShieldVerdict::Allow));
    }

    #[test]
    fn test_allow_clean_query() {
        let engine = ShieldEngine::new(ShieldConfig::default());
        let ctx = test_ctx("10.0.0.1");
        let verdict = engine.analyze_query("SELECT id, name FROM users WHERE id = 1", &ctx);
        assert!(matches!(verdict, ShieldVerdict::Allow));
    }

    #[test]
    fn test_block_sql_injection() {
        let engine = ShieldEngine::new(ShieldConfig::from_preset(SecurityPreset::Strict));
        let ctx = test_ctx("10.0.0.1");
        let verdict = engine.analyze_query(
            "SELECT * FROM users WHERE name='admin' OR 1=1; DROP TABLE users; DELETE FROM sessions",
            &ctx,
        );
        assert!(matches!(verdict, ShieldVerdict::Block { .. }));
    }

    #[test]
    fn test_allowlist_bypasses_checks() {
        let engine = ShieldEngine::new(ShieldConfig::default());
        engine.add_to_allowlist("10.0.0.1");
        let ctx = test_ctx("10.0.0.1");
        let verdict = engine.analyze_request(&ctx);
        assert!(matches!(verdict, ShieldVerdict::Allow));
    }

    #[test]
    fn test_ban_blocks_request() {
        let engine = ShieldEngine::new(ShieldConfig::default());
        engine.ip_reputation.ban("10.0.0.1", 3600, "test");
        let ctx = test_ctx("10.0.0.1");
        let verdict = engine.analyze_request(&ctx);
        assert!(matches!(verdict, ShieldVerdict::Block { .. }));
    }

    #[test]
    fn test_scanner_user_agent() {
        let engine = ShieldEngine::new(ShieldConfig::from_preset(SecurityPreset::Strict));
        let mut ctx = test_ctx("10.0.0.1");
        ctx.user_agent = Some("sqlmap/1.6".to_string());
        let verdict = engine.analyze_request(&ctx);
        // Scanner gets high score, should at least be rate limited or blocked
        assert!(!matches!(verdict, ShieldVerdict::Allow));
    }

    #[test]
    fn test_failed_auth_tracking() {
        let engine = ShieldEngine::new(ShieldConfig::default());
        for _ in 0..5 {
            engine.record_failed_auth("10.0.0.1", "admin");
        }
        let rep = engine.get_ip_reputation("10.0.0.1").unwrap();
        assert_eq!(rep.failed_auths, 5);
        assert!(rep.score < 0);
    }

    #[test]
    fn test_brute_force_auto_ban() {
        let engine = ShieldEngine::new(ShieldConfig::default());
        for _ in 0..12 {
            engine.record_failed_auth("10.0.0.1", "admin");
        }
        // After 10+ failures, should be auto-banned
        assert!(engine.ip_reputation.is_banned("10.0.0.1"));
    }

    #[test]
    fn test_unblock_ip() {
        let engine = ShieldEngine::new(ShieldConfig::default());
        engine
            .blocker
            .block("10.0.0.1", "test", Some(3600), ThreatLevel::High);
        assert!(engine.unblock_ip("10.0.0.1"));
        assert!(engine.blocker.should_block("10.0.0.1").is_none());
    }

    #[test]
    fn test_disabled_shield_allows_all() {
        let mut config = ShieldConfig::default();
        config.enabled = false;
        let engine = ShieldEngine::new(config);
        let ctx = test_ctx("10.0.0.1");
        let verdict = engine.analyze_query("'; DROP TABLE users; --", &ctx);
        assert!(matches!(verdict, ShieldVerdict::Allow));
    }

    #[test]
    fn test_get_stats() {
        let engine = ShieldEngine::new(ShieldConfig::default());
        let ctx = test_ctx("10.0.0.1");
        engine.analyze_request(&ctx);
        let stats = engine.get_stats();
        assert_eq!(stats.total_events, 0); // clean request produces no events
    }

    #[test]
    fn test_policy_update() {
        let engine = ShieldEngine::new(ShieldConfig::default());
        let new_policy = SecurityPolicy::from_preset(SecurityPreset::Strict);
        engine.update_policy(new_policy);
        let pol = engine.get_policy();
        assert_eq!(pol.preset, SecurityPreset::Strict);
    }

    #[test]
    fn test_cleanup_expired() {
        let engine = ShieldEngine::new(ShieldConfig::default());
        // Just ensure it doesn't panic
        engine.cleanup_expired();
    }
}
