//! Aegis Time Series Index
//!
//! Indexing structures for efficient series lookup and filtering.
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use crate::types::{Metric, Tags};
use std::collections::{HashMap, HashSet};
use std::sync::RwLock;

// =============================================================================
// Time Series Index
// =============================================================================

/// Index for time series metadata and tag-based lookup.
pub struct TimeSeriesIndex {
    series_by_id: RwLock<HashMap<String, SeriesMetadata>>,
    series_by_metric: RwLock<HashMap<String, HashSet<String>>>,
    series_by_tag: RwLock<HashMap<String, HashMap<String, HashSet<String>>>>,
}

impl TimeSeriesIndex {
    pub fn new() -> Self {
        Self {
            series_by_id: RwLock::new(HashMap::new()),
            series_by_metric: RwLock::new(HashMap::new()),
            series_by_tag: RwLock::new(HashMap::new()),
        }
    }

    /// Register a new series in the index.
    pub fn register(&self, metric: &Metric, tags: &Tags) -> String {
        let series_id = format!("{}:{}", metric.name, tags.series_key());

        let metadata = SeriesMetadata {
            series_id: series_id.clone(),
            metric_name: metric.name.clone(),
            tags: tags.clone(),
        };

        {
            let mut by_id = self.series_by_id.write().expect("series_by_id lock poisoned");
            by_id.insert(series_id.clone(), metadata);
        }

        {
            let mut by_metric = self.series_by_metric.write().expect("series_by_metric lock poisoned");
            by_metric
                .entry(metric.name.clone())
                .or_default()
                .insert(series_id.clone());
        }

        {
            let mut by_tag = self.series_by_tag.write().expect("series_by_tag lock poisoned");
            for (key, value) in tags.iter() {
                by_tag
                    .entry(key.clone())
                    .or_default()
                    .entry(value.clone())
                    .or_default()
                    .insert(series_id.clone());
            }
        }

        series_id
    }

    /// Get series metadata by ID.
    pub fn get(&self, series_id: &str) -> Option<SeriesMetadata> {
        let by_id = self.series_by_id.read().expect("series_by_id lock poisoned");
        by_id.get(series_id).cloned()
    }

    /// Find series by metric name.
    pub fn find_by_metric(&self, metric_name: &str) -> Vec<String> {
        let by_metric = self.series_by_metric.read().expect("series_by_metric lock poisoned");
        by_metric
            .get(metric_name)
            .map(|set| set.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Find series by tag key-value pair.
    pub fn find_by_tag(&self, key: &str, value: &str) -> Vec<String> {
        let by_tag = self.series_by_tag.read().expect("series_by_tag lock poisoned");
        by_tag
            .get(key)
            .and_then(|values| values.get(value))
            .map(|set| set.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Find series matching all tag filters.
    pub fn find_by_tags(&self, tags: &Tags) -> Vec<String> {
        let mut result: Option<HashSet<String>> = None;

        let by_tag = self.series_by_tag.read().expect("series_by_tag lock poisoned");

        for (key, value) in tags.iter() {
            let matching = by_tag
                .get(key)
                .and_then(|values| values.get(value))
                .cloned()
                .unwrap_or_default();

            result = Some(match result {
                Some(current) => current.intersection(&matching).cloned().collect(),
                None => matching,
            });
        }

        result.map(|s| s.into_iter().collect()).unwrap_or_default()
    }

    /// Get all tag keys.
    pub fn tag_keys(&self) -> Vec<String> {
        let by_tag = self.series_by_tag.read().expect("series_by_tag lock poisoned");
        by_tag.keys().cloned().collect()
    }

    /// Get all values for a tag key.
    pub fn tag_values(&self, key: &str) -> Vec<String> {
        let by_tag = self.series_by_tag.read().expect("series_by_tag lock poisoned");
        by_tag
            .get(key)
            .map(|values| values.keys().cloned().collect())
            .unwrap_or_default()
    }

    /// Get all metric names.
    pub fn metric_names(&self) -> Vec<String> {
        let by_metric = self.series_by_metric.read().expect("series_by_metric lock poisoned");
        by_metric.keys().cloned().collect()
    }

    /// Get the number of indexed series.
    pub fn len(&self) -> usize {
        let by_id = self.series_by_id.read().expect("series_by_id lock poisoned");
        by_id.len()
    }

    /// Check if the index is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Remove a series from the index.
    pub fn remove(&self, series_id: &str) -> bool {
        let metadata = {
            let mut by_id = self.series_by_id.write().expect("series_by_id lock poisoned");
            by_id.remove(series_id)
        };

        let Some(metadata) = metadata else {
            return false;
        };

        {
            let mut by_metric = self.series_by_metric.write().expect("series_by_metric lock poisoned");
            if let Some(set) = by_metric.get_mut(&metadata.metric_name) {
                set.remove(series_id);
            }
        }

        {
            let mut by_tag = self.series_by_tag.write().expect("series_by_tag lock poisoned");
            for (key, value) in metadata.tags.iter() {
                if let Some(values) = by_tag.get_mut(key) {
                    if let Some(set) = values.get_mut(value) {
                        set.remove(series_id);
                    }
                }
            }
        }

        true
    }
}

impl Default for TimeSeriesIndex {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Series Metadata
// =============================================================================

/// Metadata for an indexed series.
#[derive(Debug, Clone)]
pub struct SeriesMetadata {
    pub series_id: String,
    pub metric_name: String,
    pub tags: Tags,
}

// =============================================================================
// Label Matcher
// =============================================================================

/// Matcher for label-based filtering.
#[derive(Debug, Clone)]
pub enum LabelMatcher {
    Equal(String, String),
    NotEqual(String, String),
    Regex(String, String),
    NotRegex(String, String),
}

impl LabelMatcher {
    pub fn matches(&self, tags: &Tags) -> bool {
        match self {
            Self::Equal(key, value) => tags.get(key) == Some(value),
            Self::NotEqual(key, value) => tags.get(key) != Some(value),
            Self::Regex(key, pattern) => {
                if let Some(value) = tags.get(key) {
                    regex::Regex::new(pattern)
                        .map(|re| re.is_match(value))
                        .unwrap_or(false)
                } else {
                    false
                }
            }
            Self::NotRegex(key, pattern) => {
                if let Some(value) = tags.get(key) {
                    regex::Regex::new(pattern)
                        .map(|re| !re.is_match(value))
                        .unwrap_or(true)
                } else {
                    true
                }
            }
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_index() -> TimeSeriesIndex {
        let index = TimeSeriesIndex::new();

        let metric = Metric::gauge("cpu_usage");

        let mut tags1 = Tags::new();
        tags1.insert("host", "server1");
        tags1.insert("region", "us-east");
        index.register(&metric, &tags1);

        let mut tags2 = Tags::new();
        tags2.insert("host", "server2");
        tags2.insert("region", "us-east");
        index.register(&metric, &tags2);

        let mut tags3 = Tags::new();
        tags3.insert("host", "server3");
        tags3.insert("region", "us-west");
        index.register(&metric, &tags3);

        index
    }

    #[test]
    fn test_register_and_get() {
        let index = create_test_index();

        assert_eq!(index.len(), 3);

        let series = index.find_by_metric("cpu_usage");
        assert_eq!(series.len(), 3);
    }

    #[test]
    fn test_find_by_tag() {
        let index = create_test_index();

        let series = index.find_by_tag("region", "us-east");
        assert_eq!(series.len(), 2);

        let series = index.find_by_tag("host", "server1");
        assert_eq!(series.len(), 1);
    }

    #[test]
    fn test_find_by_tags() {
        let index = create_test_index();

        let mut filter = Tags::new();
        filter.insert("region", "us-east");

        let series = index.find_by_tags(&filter);
        assert_eq!(series.len(), 2);
    }

    #[test]
    fn test_tag_keys_values() {
        let index = create_test_index();

        let keys = index.tag_keys();
        assert!(keys.contains(&"host".to_string()));
        assert!(keys.contains(&"region".to_string()));

        let values = index.tag_values("region");
        assert!(values.contains(&"us-east".to_string()));
        assert!(values.contains(&"us-west".to_string()));
    }
}
