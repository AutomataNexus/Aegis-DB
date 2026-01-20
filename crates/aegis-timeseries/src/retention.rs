//! Aegis Time Series Retention
//!
//! Retention policy management for automatic data lifecycle.
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use crate::aggregation::AggregateFunction;
use crate::partition::PartitionManager;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

// =============================================================================
// Retention Policy
// =============================================================================

/// Retention policy for time series data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionPolicy {
    pub name: String,
    pub duration: Duration,
    pub downsample: Option<DownsampleConfig>,
    pub delete_after: bool,
}

impl RetentionPolicy {
    /// Create a simple retention policy that deletes data after the duration.
    pub fn delete_after(name: impl Into<String>, duration: Duration) -> Self {
        Self {
            name: name.into(),
            duration,
            downsample: None,
            delete_after: true,
        }
    }

    /// Create a retention policy that downsamples data.
    pub fn downsample(
        name: impl Into<String>,
        duration: Duration,
        interval: Duration,
        function: AggregateFunction,
    ) -> Self {
        Self {
            name: name.into(),
            duration,
            downsample: Some(DownsampleConfig { interval, function }),
            delete_after: false,
        }
    }

    /// Check if data at this timestamp should be affected by this policy.
    pub fn applies_to(&self, timestamp: DateTime<Utc>) -> bool {
        let cutoff = Utc::now() - self.duration;
        timestamp < cutoff
    }
}

/// Configuration for downsampling.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownsampleConfig {
    pub interval: Duration,
    pub function: AggregateFunction,
}

// =============================================================================
// Retention Tier
// =============================================================================

/// A tier in a multi-tier retention configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionTier {
    pub name: String,
    pub age: Duration,
    pub resolution: Duration,
    pub function: AggregateFunction,
}

impl RetentionTier {
    pub fn new(
        name: impl Into<String>,
        age: Duration,
        resolution: Duration,
        function: AggregateFunction,
    ) -> Self {
        Self {
            name: name.into(),
            age,
            resolution,
            function,
        }
    }
}

// =============================================================================
// Multi-Tier Retention
// =============================================================================

/// Multi-tier retention configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiTierRetention {
    pub tiers: Vec<RetentionTier>,
    pub final_retention: Duration,
}

impl MultiTierRetention {
    /// Create a common configuration for monitoring data.
    pub fn monitoring() -> Self {
        Self {
            tiers: vec![
                RetentionTier::new("raw", Duration::hours(24), Duration::zero(), AggregateFunction::Last),
                RetentionTier::new("5min", Duration::days(7), Duration::minutes(5), AggregateFunction::Avg),
                RetentionTier::new("1hour", Duration::days(30), Duration::hours(1), AggregateFunction::Avg),
                RetentionTier::new("1day", Duration::days(365), Duration::days(1), AggregateFunction::Avg),
            ],
            final_retention: Duration::days(365),
        }
    }

    /// Create a common configuration for IoT data.
    pub fn iot() -> Self {
        Self {
            tiers: vec![
                RetentionTier::new("raw", Duration::hours(1), Duration::zero(), AggregateFunction::Last),
                RetentionTier::new("1min", Duration::days(1), Duration::minutes(1), AggregateFunction::Avg),
                RetentionTier::new("15min", Duration::days(7), Duration::minutes(15), AggregateFunction::Avg),
                RetentionTier::new("1hour", Duration::days(90), Duration::hours(1), AggregateFunction::Avg),
            ],
            final_retention: Duration::days(90),
        }
    }

    /// Get the tier for a given data age.
    pub fn tier_for_age(&self, age: Duration) -> Option<&RetentionTier> {
        self.tiers.iter().find(|t| age < t.age)
    }
}

// =============================================================================
// Retention Manager
// =============================================================================

/// Manages retention policies and executes cleanup.
pub struct RetentionManager {
    policies: Vec<RetentionPolicy>,
    multi_tier: Option<MultiTierRetention>,
}

impl RetentionManager {
    pub fn new() -> Self {
        Self {
            policies: Vec::new(),
            multi_tier: None,
        }
    }

    /// Add a retention policy.
    pub fn add_policy(&mut self, policy: RetentionPolicy) {
        self.policies.push(policy);
    }

    /// Set multi-tier retention.
    pub fn set_multi_tier(&mut self, config: MultiTierRetention) {
        self.multi_tier = Some(config);
    }

    /// Apply retention policies to a partition manager.
    pub fn apply(&self, partition_manager: &PartitionManager) -> RetentionResult {
        let mut result = RetentionResult::default();
        let now = Utc::now();

        for policy in &self.policies {
            if policy.delete_after {
                let cutoff = now - policy.duration;
                let removed = partition_manager.remove_partitions_before(cutoff);
                result.partitions_deleted += removed;
            }
        }

        if let Some(ref multi_tier) = self.multi_tier {
            let cutoff = now - multi_tier.final_retention;
            let removed = partition_manager.remove_partitions_before(cutoff);
            result.partitions_deleted += removed;
        }

        result
    }

    /// Get the cutoff time for data deletion.
    pub fn deletion_cutoff(&self) -> Option<DateTime<Utc>> {
        let now = Utc::now();

        let policy_cutoff = self
            .policies
            .iter()
            .filter(|p| p.delete_after)
            .map(|p| now - p.duration)
            .min();

        let tier_cutoff = self.multi_tier.as_ref().map(|t| now - t.final_retention);

        match (policy_cutoff, tier_cutoff) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        }
    }
}

impl Default for RetentionManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of applying retention policies.
#[derive(Debug, Default)]
pub struct RetentionResult {
    pub partitions_deleted: usize,
    pub points_deleted: usize,
    pub points_downsampled: usize,
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retention_policy() {
        let policy = RetentionPolicy::delete_after("raw", Duration::days(7));

        let recent = Utc::now() - Duration::hours(1);
        let old = Utc::now() - Duration::days(10);

        assert!(!policy.applies_to(recent));
        assert!(policy.applies_to(old));
    }

    #[test]
    fn test_multi_tier_retention() {
        let config = MultiTierRetention::monitoring();

        assert_eq!(config.tiers.len(), 4);

        let tier = config.tier_for_age(Duration::hours(12));
        assert!(tier.is_some());
        assert_eq!(tier.unwrap().name, "raw");

        let tier = config.tier_for_age(Duration::days(3));
        assert!(tier.is_some());
        assert_eq!(tier.unwrap().name, "5min");
    }

    #[test]
    fn test_retention_manager() {
        let mut manager = RetentionManager::new();
        manager.add_policy(RetentionPolicy::delete_after("test", Duration::days(30)));

        let cutoff = manager.deletion_cutoff();
        assert!(cutoff.is_some());
    }
}
