//! Security policy evaluation.

use crate::config::SecurityPreset;
use crate::threat::ThreatAction;
use serde::{Deserialize, Serialize};

/// A user-defined custom security rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomRule {
    pub name: String,
    pub path_pattern: Option<String>,
    pub max_score: u32,
    pub action: ThreatAction,
}

/// The active security policy combining preset defaults and custom rules.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityPolicy {
    pub preset: SecurityPreset,
    pub sql_injection_enabled: bool,
    pub anomaly_detection_enabled: bool,
    pub ip_reputation_enabled: bool,
    pub fingerprinting_enabled: bool,
    pub auto_blocking_enabled: bool,
    pub custom_rules: Vec<CustomRule>,
}

impl SecurityPolicy {
    /// Build a policy from a preset with no custom rules.
    pub fn from_preset(preset: SecurityPreset) -> Self {
        let (sql, anomaly, ip_rep, fp, auto_block) = match preset {
            SecurityPreset::Strict => (true, true, true, true, true),
            SecurityPreset::Moderate => (true, true, true, true, true),
            SecurityPreset::Permissive => (true, true, true, true, false),
        };
        Self {
            preset,
            sql_injection_enabled: sql,
            anomaly_detection_enabled: anomaly,
            ip_reputation_enabled: ip_rep,
            fingerprinting_enabled: fp,
            auto_blocking_enabled: auto_block,
            custom_rules: Vec::new(),
        }
    }

    /// Evaluate a threat score and request path to determine the action.
    /// Custom rules are checked first (first match wins), then fall back
    /// to preset-based thresholds.
    pub fn evaluate(&self, score: u32, path: Option<&str>) -> ThreatAction {
        // Check custom rules first
        for rule in &self.custom_rules {
            let path_matches = match (&rule.path_pattern, path) {
                (Some(pattern), Some(p)) => p.starts_with(pattern.as_str()),
                (Some(_), None) => false,
                (None, _) => true, // rule applies to all paths
            };

            if path_matches && score >= rule.max_score {
                return rule.action;
            }
        }

        // Default thresholds by preset
        let (block_threshold, rate_limit_threshold) = match self.preset {
            SecurityPreset::Strict => (60, 30),
            SecurityPreset::Moderate => (80, 50),
            SecurityPreset::Permissive => (95, 70),
        };

        if score >= block_threshold {
            ThreatAction::Blocked
        } else if score >= rate_limit_threshold {
            ThreatAction::RateLimited
        } else {
            ThreatAction::Allowed
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strict_blocks_at_60() {
        let policy = SecurityPolicy::from_preset(SecurityPreset::Strict);
        assert_eq!(policy.evaluate(60, None), ThreatAction::Blocked);
        assert_eq!(policy.evaluate(30, None), ThreatAction::RateLimited);
        assert_eq!(policy.evaluate(10, None), ThreatAction::Allowed);
    }

    #[test]
    fn test_moderate_blocks_at_80() {
        let policy = SecurityPolicy::from_preset(SecurityPreset::Moderate);
        assert_eq!(policy.evaluate(80, None), ThreatAction::Blocked);
        assert_eq!(policy.evaluate(50, None), ThreatAction::RateLimited);
        assert_eq!(policy.evaluate(20, None), ThreatAction::Allowed);
    }

    #[test]
    fn test_permissive_blocks_at_95() {
        let policy = SecurityPolicy::from_preset(SecurityPreset::Permissive);
        assert_eq!(policy.evaluate(95, None), ThreatAction::Blocked);
        assert_eq!(policy.evaluate(70, None), ThreatAction::RateLimited);
        assert_eq!(policy.evaluate(50, None), ThreatAction::Allowed);
    }

    #[test]
    fn test_custom_rule_takes_precedence() {
        let mut policy = SecurityPolicy::from_preset(SecurityPreset::Moderate);
        policy.custom_rules.push(CustomRule {
            name: "admin_strict".to_string(),
            path_pattern: Some("/api/v1/admin".to_string()),
            max_score: 30,
            action: ThreatAction::Blocked,
        });
        // Score 30 on admin path should block due to custom rule
        assert_eq!(
            policy.evaluate(30, Some("/api/v1/admin/settings")),
            ThreatAction::Blocked
        );
        // Same score on other path uses default (allowed)
        assert_eq!(
            policy.evaluate(30, Some("/api/v1/query")),
            ThreatAction::Allowed
        );
    }
}
