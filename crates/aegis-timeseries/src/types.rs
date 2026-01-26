//! Aegis Time Series Types
//!
//! Core data types for time series storage and querying.
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// =============================================================================
// Data Point
// =============================================================================

/// A single time series data point.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DataPoint {
    pub timestamp: DateTime<Utc>,
    pub value: f64,
}

impl DataPoint {
    pub fn new(timestamp: DateTime<Utc>, value: f64) -> Self {
        Self { timestamp, value }
    }

    pub fn now(value: f64) -> Self {
        Self {
            timestamp: Utc::now(),
            value,
        }
    }

    pub fn timestamp_nanos(&self) -> i64 {
        self.timestamp.timestamp_nanos_opt().unwrap_or(0)
    }

    pub fn timestamp_millis(&self) -> i64 {
        self.timestamp.timestamp_millis()
    }
}

// =============================================================================
// Tags
// =============================================================================

/// Key-value tags for metric identification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Tags(pub HashMap<String, String>);

impl Tags {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self(HashMap::with_capacity(capacity))
    }

    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) -> &mut Self {
        self.0.insert(key.into(), value.into());
        self
    }

    pub fn get(&self, key: &str) -> Option<&String> {
        self.0.get(key)
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.0.contains_key(key)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &String)> {
        self.0.iter()
    }

    /// Generate a unique series key from tags.
    pub fn series_key(&self) -> String {
        let mut pairs: Vec<_> = self.0.iter().collect();
        pairs.sort_by(|a, b| a.0.cmp(b.0));
        pairs
            .into_iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join(",")
    }

    /// Check if this tag set matches a filter.
    pub fn matches(&self, filter: &Tags) -> bool {
        filter.0.iter().all(|(k, v)| self.0.get(k) == Some(v))
    }
}

impl From<HashMap<String, String>> for Tags {
    fn from(map: HashMap<String, String>) -> Self {
        Self(map)
    }
}

// =============================================================================
// Metric Type
// =============================================================================

/// Type of metric for semantic interpretation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[derive(Default)]
pub enum MetricType {
    /// Monotonically increasing counter (e.g., request count).
    Counter,
    /// Point-in-time measurement (e.g., temperature).
    #[default]
    Gauge,
    /// Distribution of values (e.g., latency histogram).
    Histogram,
    /// Summary statistics (e.g., percentiles).
    Summary,
}


// =============================================================================
// Metric
// =============================================================================

/// Metric metadata and configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Metric {
    pub name: String,
    pub metric_type: MetricType,
    pub description: Option<String>,
    pub unit: Option<String>,
}

impl Metric {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            metric_type: MetricType::Gauge,
            description: None,
            unit: None,
        }
    }

    pub fn counter(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            metric_type: MetricType::Counter,
            description: None,
            unit: None,
        }
    }

    pub fn gauge(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            metric_type: MetricType::Gauge,
            description: None,
            unit: None,
        }
    }

    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    pub fn with_unit(mut self, unit: impl Into<String>) -> Self {
        self.unit = Some(unit.into());
        self
    }
}

// =============================================================================
// Series
// =============================================================================

/// A time series with metric metadata, tags, and data points.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Series {
    pub metric: Metric,
    pub tags: Tags,
    pub points: Vec<DataPoint>,
}

impl Series {
    pub fn new(metric: Metric, tags: Tags) -> Self {
        Self {
            metric,
            tags,
            points: Vec::new(),
        }
    }

    pub fn with_points(metric: Metric, tags: Tags, points: Vec<DataPoint>) -> Self {
        Self {
            metric,
            tags,
            points,
        }
    }

    /// Add a data point to the series.
    pub fn push(&mut self, point: DataPoint) {
        self.points.push(point);
    }

    /// Add a value at the current time.
    pub fn push_now(&mut self, value: f64) {
        self.points.push(DataPoint::now(value));
    }

    /// Get the unique series identifier.
    pub fn series_id(&self) -> String {
        format!("{}:{}", self.metric.name, self.tags.series_key())
    }

    /// Get the time range of this series.
    pub fn time_range(&self) -> Option<(DateTime<Utc>, DateTime<Utc>)> {
        if self.points.is_empty() {
            return None;
        }
        let first = self.points.first()?.timestamp;
        let last = self.points.last()?.timestamp;
        Some((first, last))
    }

    /// Sort points by timestamp.
    pub fn sort(&mut self) {
        self.points.sort_by_key(|p| p.timestamp);
    }

    /// Check if points are sorted by timestamp.
    pub fn is_sorted(&self) -> bool {
        self.points.windows(2).all(|w| w[0].timestamp <= w[1].timestamp)
    }

    /// Get points within a time range.
    pub fn range(&self, start: DateTime<Utc>, end: DateTime<Utc>) -> Vec<&DataPoint> {
        self.points
            .iter()
            .filter(|p| p.timestamp >= start && p.timestamp < end)
            .collect()
    }

    /// Get the latest value.
    pub fn latest(&self) -> Option<&DataPoint> {
        self.points.last()
    }

    /// Get the earliest value.
    pub fn earliest(&self) -> Option<&DataPoint> {
        self.points.first()
    }
}

// =============================================================================
// Sample
// =============================================================================

/// A single sample for ingestion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sample {
    pub metric_name: String,
    pub tags: Tags,
    pub timestamp: DateTime<Utc>,
    pub value: f64,
}

impl Sample {
    pub fn new(
        metric_name: impl Into<String>,
        tags: Tags,
        timestamp: DateTime<Utc>,
        value: f64,
    ) -> Self {
        Self {
            metric_name: metric_name.into(),
            tags,
            timestamp,
            value,
        }
    }

    pub fn series_id(&self) -> String {
        format!("{}:{}", self.metric_name, self.tags.series_key())
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_data_point() {
        let point = DataPoint::now(42.5);
        assert_eq!(point.value, 42.5);
        assert!(point.timestamp_millis() > 0);
    }

    #[test]
    fn test_tags() {
        let mut tags = Tags::new();
        tags.insert("host", "server1");
        tags.insert("region", "us-east");

        assert_eq!(tags.get("host"), Some(&"server1".to_string()));
        assert_eq!(tags.series_key(), "host=server1,region=us-east");
    }

    #[test]
    fn test_tags_matching() {
        let mut tags = Tags::new();
        tags.insert("host", "server1");
        tags.insert("region", "us-east");
        tags.insert("env", "prod");

        let mut filter = Tags::new();
        filter.insert("host", "server1");
        filter.insert("env", "prod");

        assert!(tags.matches(&filter));

        filter.insert("region", "us-west");
        assert!(!tags.matches(&filter));
    }

    #[test]
    fn test_series() {
        let metric = Metric::gauge("cpu_usage").with_unit("percent");
        let mut tags = Tags::new();
        tags.insert("host", "server1");

        let mut series = Series::new(metric, tags);
        series.push_now(45.5);
        series.push_now(50.2);

        assert_eq!(series.points.len(), 2);
        assert!(series.is_sorted());
        assert_eq!(series.series_id(), "cpu_usage:host=server1");
    }

    #[test]
    fn test_metric_types() {
        let counter = Metric::counter("requests_total");
        assert_eq!(counter.metric_type, MetricType::Counter);

        let gauge = Metric::gauge("temperature");
        assert_eq!(gauge.metric_type, MetricType::Gauge);
    }
}
