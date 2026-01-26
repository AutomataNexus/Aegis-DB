//! Aegis Time Series Partitioning
//!
//! Time-based partitioning for efficient data management and queries.
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use crate::types::DataPoint;
use chrono::{DateTime, Duration, Utc, Timelike, Datelike};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

// =============================================================================
// Partition Configuration
// =============================================================================

/// Configuration for partition management.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartitionConfig {
    pub interval: PartitionInterval,
    pub max_open_partitions: usize,
    pub compaction_threshold: usize,
}

impl Default for PartitionConfig {
    fn default() -> Self {
        Self {
            interval: PartitionInterval::Hour,
            max_open_partitions: 24,
            compaction_threshold: 1000,
        }
    }
}

/// Time interval for partitioning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PartitionInterval {
    Minute,
    Hour,
    Day,
    Week,
    Month,
}

impl PartitionInterval {
    /// Get the duration of this interval.
    pub fn duration(&self) -> Duration {
        match self {
            Self::Minute => Duration::minutes(1),
            Self::Hour => Duration::hours(1),
            Self::Day => Duration::days(1),
            Self::Week => Duration::weeks(1),
            Self::Month => Duration::days(30),
        }
    }

    /// Get the partition key for a timestamp.
    pub fn partition_key(&self, timestamp: DateTime<Utc>) -> String {
        match self {
            Self::Minute => timestamp.format("%Y%m%d%H%M").to_string(),
            Self::Hour => timestamp.format("%Y%m%d%H").to_string(),
            Self::Day => timestamp.format("%Y%m%d").to_string(),
            Self::Week => {
                let week = timestamp.iso_week().week();
                format!("{}{:02}", timestamp.year(), week)
            }
            Self::Month => timestamp.format("%Y%m").to_string(),
        }
    }

    /// Get the start time of the partition containing this timestamp.
    pub fn partition_start(&self, timestamp: DateTime<Utc>) -> DateTime<Utc> {
        match self {
            Self::Minute => timestamp
                .with_second(0)
                .and_then(|t| t.with_nanosecond(0))
                .unwrap_or(timestamp),
            Self::Hour => timestamp
                .with_minute(0)
                .and_then(|t| t.with_second(0))
                .and_then(|t| t.with_nanosecond(0))
                .unwrap_or(timestamp),
            Self::Day => timestamp
                .with_hour(0)
                .and_then(|t| t.with_minute(0))
                .and_then(|t| t.with_second(0))
                .and_then(|t| t.with_nanosecond(0))
                .unwrap_or(timestamp),
            Self::Week => {
                let days_from_monday = timestamp.weekday().num_days_from_monday();
                let monday = timestamp - Duration::days(days_from_monday as i64);
                monday
                    .with_hour(0)
                    .and_then(|t| t.with_minute(0))
                    .and_then(|t| t.with_second(0))
                    .and_then(|t| t.with_nanosecond(0))
                    .unwrap_or(timestamp)
            }
            Self::Month => timestamp
                .with_day(1)
                .and_then(|t| t.with_hour(0))
                .and_then(|t| t.with_minute(0))
                .and_then(|t| t.with_second(0))
                .and_then(|t| t.with_nanosecond(0))
                .unwrap_or(timestamp),
        }
    }
}

// =============================================================================
// Partition
// =============================================================================

/// A time-bounded partition of time series data.
#[derive(Debug, Clone)]
pub struct Partition {
    pub key: String,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub series: HashMap<String, Vec<DataPoint>>,
    pub point_count: usize,
    pub is_sealed: bool,
}

impl Partition {
    pub fn new(key: String, start_time: DateTime<Utc>, interval: PartitionInterval) -> Self {
        let end_time = start_time + interval.duration();
        Self {
            key,
            start_time,
            end_time,
            series: HashMap::new(),
            point_count: 0,
            is_sealed: false,
        }
    }

    /// Check if a timestamp falls within this partition.
    pub fn contains(&self, timestamp: DateTime<Utc>) -> bool {
        timestamp >= self.start_time && timestamp < self.end_time
    }

    /// Add a data point to the partition.
    pub fn insert(&mut self, series_id: &str, point: DataPoint) -> bool {
        if self.is_sealed || !self.contains(point.timestamp) {
            return false;
        }

        self.series
            .entry(series_id.to_string())
            .or_default()
            .push(point);
        self.point_count += 1;
        true
    }

    /// Get points for a series within a time range.
    pub fn get_range(
        &self,
        series_id: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Vec<&DataPoint> {
        self.series
            .get(series_id)
            .map(|points| {
                points
                    .iter()
                    .filter(|p| p.timestamp >= start && p.timestamp < end)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Seal the partition (no more writes).
    pub fn seal(&mut self) {
        self.is_sealed = true;
        for points in self.series.values_mut() {
            points.sort_by_key(|p| p.timestamp);
        }
    }

    /// Get all series IDs in this partition.
    pub fn series_ids(&self) -> Vec<&String> {
        self.series.keys().collect()
    }

    /// Get the memory size estimate.
    pub fn size_bytes(&self) -> usize {
        self.point_count * std::mem::size_of::<DataPoint>()
            + self.series.len() * 64
    }
}

// =============================================================================
// Partition Manager
// =============================================================================

/// Manages partitions for time series data.
pub struct PartitionManager {
    config: PartitionConfig,
    partitions: RwLock<HashMap<String, Arc<RwLock<Partition>>>>,
    current_partition: RwLock<Option<String>>,
}

impl PartitionManager {
    pub fn new(config: PartitionConfig) -> Self {
        Self {
            config,
            partitions: RwLock::new(HashMap::new()),
            current_partition: RwLock::new(None),
        }
    }

    /// Get or create the partition for a timestamp.
    pub fn get_partition(&self, timestamp: DateTime<Utc>) -> Arc<RwLock<Partition>> {
        let key = self.config.interval.partition_key(timestamp);

        {
            let partitions = self.partitions.read().expect("partitions lock poisoned");
            if let Some(partition) = partitions.get(&key) {
                return Arc::clone(partition);
            }
        }

        let mut partitions = self.partitions.write().expect("partitions lock poisoned");

        if let Some(partition) = partitions.get(&key) {
            return Arc::clone(partition);
        }

        let start = self.config.interval.partition_start(timestamp);
        let partition = Partition::new(key.clone(), start, self.config.interval);
        let partition = Arc::new(RwLock::new(partition));
        partitions.insert(key.clone(), Arc::clone(&partition));

        let mut current = self.current_partition.write().expect("current_partition lock poisoned");
        *current = Some(key);

        partition
    }

    /// Get partitions overlapping a time range.
    pub fn get_partitions_in_range(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Vec<Arc<RwLock<Partition>>> {
        let partitions = self.partitions.read().expect("partitions lock poisoned");
        partitions
            .values()
            .filter(|p| {
                let partition = p.read().expect("partition lock poisoned");
                partition.start_time < end && partition.end_time > start
            })
            .cloned()
            .collect()
    }

    /// Seal old partitions.
    pub fn seal_old_partitions(&self, before: DateTime<Utc>) {
        let partitions = self.partitions.read().expect("partitions lock poisoned");
        for partition in partitions.values() {
            let mut p = partition.write().expect("partition lock poisoned");
            if p.end_time <= before && !p.is_sealed {
                p.seal();
            }
        }
    }

    /// Remove partitions older than a timestamp.
    pub fn remove_partitions_before(&self, before: DateTime<Utc>) -> usize {
        let mut partitions = self.partitions.write().expect("partitions lock poisoned");
        let to_remove: Vec<_> = partitions
            .iter()
            .filter(|(_, p)| p.read().expect("partition lock poisoned").end_time <= before)
            .map(|(k, _)| k.clone())
            .collect();

        let count = to_remove.len();
        for key in to_remove {
            partitions.remove(&key);
        }
        count
    }

    /// Get partition statistics.
    pub fn stats(&self) -> PartitionStats {
        let partitions = self.partitions.read().expect("partitions lock poisoned");
        let mut total_points = 0;
        let mut total_series = 0;
        let mut total_bytes = 0;

        for partition in partitions.values() {
            let p = partition.read().expect("partition lock poisoned");
            total_points += p.point_count;
            total_series += p.series.len();
            total_bytes += p.size_bytes();
        }

        PartitionStats {
            partition_count: partitions.len(),
            total_points,
            total_series,
            total_bytes,
        }
    }
}

/// Partition statistics.
#[derive(Debug, Clone)]
pub struct PartitionStats {
    pub partition_count: usize,
    pub total_points: usize,
    pub total_series: usize,
    pub total_bytes: usize,
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_partition_interval_key() {
        let timestamp = DateTime::parse_from_rfc3339("2026-01-19T15:30:45Z")
            .expect("failed to parse test timestamp")
            .with_timezone(&Utc);

        assert_eq!(
            PartitionInterval::Hour.partition_key(timestamp),
            "2026011915"
        );
        assert_eq!(
            PartitionInterval::Day.partition_key(timestamp),
            "20260119"
        );
        assert_eq!(
            PartitionInterval::Month.partition_key(timestamp),
            "202601"
        );
    }

    #[test]
    fn test_partition_insert() {
        let start = Utc::now();
        let mut partition = Partition::new(
            "test".to_string(),
            start,
            PartitionInterval::Hour,
        );

        let point = DataPoint::new(start + Duration::minutes(30), 42.0);
        assert!(partition.insert("cpu:host=server1", point));
        assert_eq!(partition.point_count, 1);
    }

    #[test]
    fn test_partition_manager() {
        let config = PartitionConfig::default();
        let manager = PartitionManager::new(config);

        let now = Utc::now();
        let partition = manager.get_partition(now);

        {
            let mut p = partition.write().expect("partition lock poisoned");
            p.insert("test:host=a", DataPoint::now(1.0));
        }

        let stats = manager.stats();
        assert_eq!(stats.partition_count, 1);
        assert_eq!(stats.total_points, 1);
    }
}
