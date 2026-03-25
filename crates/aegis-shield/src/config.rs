//! Shield configuration and presets.

use serde::{Deserialize, Serialize};

/// Security preset that controls default thresholds.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SecurityPreset {
    Strict,
    Moderate,
    Permissive,
}

/// Full configuration for the shield engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShieldConfig {
    pub enabled: bool,
    pub preset: SecurityPreset,
    pub sql_injection_enabled: bool,
    pub anomaly_detection_enabled: bool,
    pub ip_reputation_enabled: bool,
    pub fingerprinting_enabled: bool,
    pub auto_blocking_enabled: bool,
    pub auto_block_threshold: u32,
    pub default_ban_duration_secs: u64,
    pub max_ban_duration_secs: u64,
    pub escalation_multiplier: f64,
    pub max_events_in_memory: usize,
    pub anomaly_learning_period_secs: u64,
    pub anomaly_deviation_threshold: f64,
    pub cleanup_interval_secs: u64,
}

impl Default for ShieldConfig {
    fn default() -> Self {
        Self::from_preset(SecurityPreset::Moderate)
    }
}

impl ShieldConfig {
    /// Build a configuration from a named preset.
    pub fn from_preset(preset: SecurityPreset) -> Self {
        match preset {
            SecurityPreset::Strict => Self {
                enabled: true,
                preset,
                sql_injection_enabled: true,
                anomaly_detection_enabled: true,
                ip_reputation_enabled: true,
                fingerprinting_enabled: true,
                auto_blocking_enabled: true,
                auto_block_threshold: 60,
                default_ban_duration_secs: 7200,
                max_ban_duration_secs: 172800,
                escalation_multiplier: 3.0,
                max_events_in_memory: 20000,
                anomaly_learning_period_secs: 1800,
                anomaly_deviation_threshold: 2.0,
                cleanup_interval_secs: 120,
            },
            SecurityPreset::Moderate => Self {
                enabled: true,
                preset,
                sql_injection_enabled: true,
                anomaly_detection_enabled: true,
                ip_reputation_enabled: true,
                fingerprinting_enabled: true,
                auto_blocking_enabled: true,
                auto_block_threshold: 80,
                default_ban_duration_secs: 3600,
                max_ban_duration_secs: 86400,
                escalation_multiplier: 2.0,
                max_events_in_memory: 10000,
                anomaly_learning_period_secs: 3600,
                anomaly_deviation_threshold: 3.0,
                cleanup_interval_secs: 300,
            },
            SecurityPreset::Permissive => Self {
                enabled: true,
                preset,
                sql_injection_enabled: true,
                anomaly_detection_enabled: true,
                ip_reputation_enabled: true,
                fingerprinting_enabled: true,
                auto_blocking_enabled: false,
                auto_block_threshold: 95,
                default_ban_duration_secs: 300,
                max_ban_duration_secs: 3600,
                escalation_multiplier: 1.5,
                max_events_in_memory: 5000,
                anomaly_learning_period_secs: 7200,
                anomaly_deviation_threshold: 5.0,
                cleanup_interval_secs: 600,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_is_moderate() {
        let cfg = ShieldConfig::default();
        assert_eq!(cfg.preset, SecurityPreset::Moderate);
        assert_eq!(cfg.auto_block_threshold, 80);
        assert!(cfg.enabled);
    }

    #[test]
    fn test_strict_preset() {
        let cfg = ShieldConfig::from_preset(SecurityPreset::Strict);
        assert_eq!(cfg.auto_block_threshold, 60);
        assert_eq!(cfg.default_ban_duration_secs, 7200);
        assert!(cfg.auto_blocking_enabled);
    }

    #[test]
    fn test_permissive_preset() {
        let cfg = ShieldConfig::from_preset(SecurityPreset::Permissive);
        assert_eq!(cfg.auto_block_threshold, 95);
        assert_eq!(cfg.default_ban_duration_secs, 300);
        assert!(!cfg.auto_blocking_enabled);
    }

    #[test]
    fn test_serde_roundtrip() {
        let cfg = ShieldConfig::default();
        let json = serde_json::to_string(&cfg).expect("serialize");
        let cfg2: ShieldConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(cfg2.preset, cfg.preset);
        assert_eq!(cfg2.auto_block_threshold, cfg.auto_block_threshold);
    }
}
