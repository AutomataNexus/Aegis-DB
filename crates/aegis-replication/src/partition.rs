//! Aegis Partitioning
//!
//! Partition key extraction and range management.
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};

// =============================================================================
// Partition Key
// =============================================================================

/// A partition key for routing data to shards.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PartitionKey {
    /// String key.
    String(String),
    /// Integer key.
    Int(i64),
    /// Composite key (multiple values).
    Composite(Vec<PartitionKey>),
    /// UUID key.
    Uuid(String),
    /// Bytes key.
    Bytes(Vec<u8>),
}

impl PartitionKey {
    /// Create a string partition key.
    pub fn string(value: impl Into<String>) -> Self {
        Self::String(value.into())
    }

    /// Create an integer partition key.
    pub fn int(value: i64) -> Self {
        Self::Int(value)
    }

    /// Create a composite partition key.
    pub fn composite(keys: Vec<PartitionKey>) -> Self {
        Self::Composite(keys)
    }

    /// Create a UUID partition key.
    pub fn uuid(value: impl Into<String>) -> Self {
        Self::Uuid(value.into())
    }

    /// Compute the hash for this partition key.
    pub fn hash_value(&self) -> u64 {
        let mut hasher = PartitionHasher::new();
        self.hash(&mut hasher);
        hasher.finish()
    }

    /// Convert to bytes representation.
    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            Self::String(s) => s.as_bytes().to_vec(),
            Self::Int(i) => i.to_le_bytes().to_vec(),
            Self::Composite(keys) => {
                let mut bytes = Vec::new();
                for key in keys {
                    bytes.extend(key.to_bytes());
                    bytes.push(0); // separator
                }
                bytes
            }
            Self::Uuid(u) => u.as_bytes().to_vec(),
            Self::Bytes(b) => b.clone(),
        }
    }
}

impl From<String> for PartitionKey {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

impl From<&str> for PartitionKey {
    fn from(value: &str) -> Self {
        Self::String(value.to_string())
    }
}

impl From<i64> for PartitionKey {
    fn from(value: i64) -> Self {
        Self::Int(value)
    }
}

impl From<i32> for PartitionKey {
    fn from(value: i32) -> Self {
        Self::Int(value as i64)
    }
}

// =============================================================================
// Partition Strategy
// =============================================================================

/// Strategy for partitioning data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PartitionStrategy {
    /// Hash-based partitioning (consistent hashing).
    Hash {
        /// Column(s) to use for hashing.
        columns: Vec<String>,
        /// Number of partitions.
        num_partitions: u32,
    },
    /// Range-based partitioning.
    Range {
        /// Column to use for range partitioning.
        column: String,
        /// Range boundaries.
        boundaries: Vec<RangeBoundary>,
    },
    /// List-based partitioning.
    List {
        /// Column to use for list partitioning.
        column: String,
        /// Value to partition mapping.
        mappings: Vec<ListMapping>,
    },
    /// Round-robin partitioning.
    RoundRobin {
        /// Number of partitions.
        num_partitions: u32,
    },
    /// Time-based partitioning.
    Time {
        /// Column containing timestamp.
        column: String,
        /// Time interval for each partition.
        interval: TimeInterval,
    },
}

impl PartitionStrategy {
    /// Create a hash partition strategy.
    pub fn hash(columns: Vec<String>, num_partitions: u32) -> Self {
        Self::Hash {
            columns,
            num_partitions,
        }
    }

    /// Create a range partition strategy.
    pub fn range(column: String, boundaries: Vec<RangeBoundary>) -> Self {
        Self::Range { column, boundaries }
    }

    /// Create a list partition strategy.
    pub fn list(column: String, mappings: Vec<ListMapping>) -> Self {
        Self::List { column, mappings }
    }

    /// Create a round-robin partition strategy.
    pub fn round_robin(num_partitions: u32) -> Self {
        Self::RoundRobin { num_partitions }
    }

    /// Create a time-based partition strategy.
    pub fn time(column: String, interval: TimeInterval) -> Self {
        Self::Time { column, interval }
    }

    /// Get the partition for a key.
    pub fn partition_for_key(&self, key: &PartitionKey) -> u32 {
        match self {
            Self::Hash { num_partitions, .. } => {
                (key.hash_value() % *num_partitions as u64) as u32
            }
            Self::Range { boundaries, .. } => {
                let hash = key.hash_value();
                for (i, boundary) in boundaries.iter().enumerate() {
                    if hash < boundary.upper_bound {
                        return i as u32;
                    }
                }
                boundaries.len() as u32
            }
            Self::List { mappings, .. } => {
                let key_str = match key {
                    PartitionKey::String(s) => s.clone(),
                    _ => format!("{:?}", key),
                };
                for mapping in mappings {
                    if mapping.values.contains(&key_str) {
                        return mapping.partition;
                    }
                }
                0 // Default partition
            }
            Self::RoundRobin { num_partitions } => {
                // For round-robin, we use the hash as a pseudo-sequence
                (key.hash_value() % *num_partitions as u64) as u32
            }
            Self::Time { interval, .. } => {
                if let PartitionKey::Int(ts) = key {
                    (*ts as u64 / interval.to_seconds()) as u32
                } else {
                    0
                }
            }
        }
    }
}

// =============================================================================
// Range Boundary
// =============================================================================

/// A boundary for range partitioning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RangeBoundary {
    pub partition_name: String,
    pub upper_bound: u64,
}

impl RangeBoundary {
    pub fn new(name: impl Into<String>, upper_bound: u64) -> Self {
        Self {
            partition_name: name.into(),
            upper_bound,
        }
    }
}

// =============================================================================
// List Mapping
// =============================================================================

/// A mapping for list partitioning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListMapping {
    pub partition: u32,
    pub values: Vec<String>,
}

impl ListMapping {
    pub fn new(partition: u32, values: Vec<String>) -> Self {
        Self { partition, values }
    }
}

// =============================================================================
// Time Interval
// =============================================================================

/// Time interval for time-based partitioning.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum TimeInterval {
    Hour,
    Day,
    Week,
    Month,
    Year,
    Custom(u64),
}

impl TimeInterval {
    /// Convert to seconds.
    pub fn to_seconds(&self) -> u64 {
        match self {
            Self::Hour => 3600,
            Self::Day => 86400,
            Self::Week => 604800,
            Self::Month => 2592000,  // 30 days
            Self::Year => 31536000,  // 365 days
            Self::Custom(s) => *s,
        }
    }
}

// =============================================================================
// Partition Range
// =============================================================================

/// A range of partition keys.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartitionRange {
    pub start: u64,
    pub end: u64,
    pub inclusive_start: bool,
    pub inclusive_end: bool,
}

impl PartitionRange {
    /// Create a new partition range.
    pub fn new(start: u64, end: u64) -> Self {
        Self {
            start,
            end,
            inclusive_start: true,
            inclusive_end: false,
        }
    }

    /// Create a range from 0 to max.
    pub fn full() -> Self {
        Self::new(0, u64::MAX)
    }

    /// Check if a hash value is in this range.
    pub fn contains(&self, hash: u64) -> bool {
        let start_check = if self.inclusive_start {
            hash >= self.start
        } else {
            hash > self.start
        };

        let end_check = if self.inclusive_end {
            hash <= self.end
        } else {
            hash < self.end
        };

        start_check && end_check
    }

    /// Split this range into N equal parts.
    pub fn split(&self, num_parts: u32) -> Vec<PartitionRange> {
        if num_parts == 0 {
            return vec![self.clone()];
        }

        let range_size = (self.end - self.start) / num_parts as u64;
        let mut ranges = Vec::with_capacity(num_parts as usize);

        for i in 0..num_parts {
            let start = self.start + (i as u64 * range_size);
            let end = if i == num_parts - 1 {
                self.end
            } else {
                self.start + ((i as u64 + 1) * range_size)
            };

            ranges.push(PartitionRange {
                start,
                end,
                inclusive_start: true,
                inclusive_end: i == num_parts - 1,
            });
        }

        ranges
    }

    /// Merge two adjacent ranges.
    pub fn merge(&self, other: &PartitionRange) -> Option<PartitionRange> {
        if self.end == other.start {
            Some(PartitionRange {
                start: self.start,
                end: other.end,
                inclusive_start: self.inclusive_start,
                inclusive_end: other.inclusive_end,
            })
        } else if other.end == self.start {
            Some(PartitionRange {
                start: other.start,
                end: self.end,
                inclusive_start: other.inclusive_start,
                inclusive_end: self.inclusive_end,
            })
        } else {
            None
        }
    }

    /// Get the size of this range.
    pub fn size(&self) -> u64 {
        self.end - self.start
    }
}

// =============================================================================
// Partition Hasher
// =============================================================================

/// Hasher for partition keys.
struct PartitionHasher {
    state: u64,
}

impl PartitionHasher {
    fn new() -> Self {
        Self {
            state: 0x517cc1b727220a95,
        }
    }
}

impl Hasher for PartitionHasher {
    fn finish(&self) -> u64 {
        self.state
    }

    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.state ^= *byte as u64;
            self.state = self.state.wrapping_mul(0x5851f42d4c957f2d);
        }
    }
}

// =============================================================================
// Key Extractor
// =============================================================================

/// Extracts partition keys from data.
pub struct KeyExtractor {
    columns: Vec<String>,
}

impl KeyExtractor {
    /// Create a new key extractor.
    pub fn new(columns: Vec<String>) -> Self {
        Self { columns }
    }

    /// Extract a partition key from a map of values.
    pub fn extract(&self, values: &std::collections::HashMap<String, String>) -> Option<PartitionKey> {
        if self.columns.len() == 1 {
            values
                .get(&self.columns[0])
                .map(|v| PartitionKey::String(v.clone()))
        } else {
            let mut keys = Vec::new();
            for col in &self.columns {
                if let Some(v) = values.get(col) {
                    keys.push(PartitionKey::String(v.clone()));
                } else {
                    return None;
                }
            }
            Some(PartitionKey::Composite(keys))
        }
    }

    /// Get the columns used for extraction.
    pub fn columns(&self) -> &[String] {
        &self.columns
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_partition_key_string() {
        let key = PartitionKey::string("user_123");
        let hash = key.hash_value();
        assert!(hash > 0);

        // Same key should produce same hash
        let key2 = PartitionKey::string("user_123");
        assert_eq!(key.hash_value(), key2.hash_value());
    }

    #[test]
    fn test_partition_key_int() {
        let key = PartitionKey::int(12345);
        let hash = key.hash_value();
        assert!(hash > 0);
    }

    #[test]
    fn test_partition_key_composite() {
        let key = PartitionKey::composite(vec![
            PartitionKey::string("tenant_1"),
            PartitionKey::int(100),
        ]);
        let hash = key.hash_value();
        assert!(hash > 0);
    }

    #[test]
    fn test_partition_strategy_hash() {
        let strategy = PartitionStrategy::hash(vec!["id".to_string()], 16);

        let key1 = PartitionKey::string("key1");
        let key2 = PartitionKey::string("key2");

        let p1 = strategy.partition_for_key(&key1);
        let p2 = strategy.partition_for_key(&key2);

        assert!(p1 < 16);
        assert!(p2 < 16);

        // Same key should return same partition
        assert_eq!(p1, strategy.partition_for_key(&key1));
    }

    #[test]
    fn test_partition_strategy_list() {
        let strategy = PartitionStrategy::list(
            "region".to_string(),
            vec![
                ListMapping::new(0, vec!["us-east".to_string(), "us-west".to_string()]),
                ListMapping::new(1, vec!["eu-west".to_string()]),
            ],
        );

        let key_us = PartitionKey::string("us-east");
        let key_eu = PartitionKey::string("eu-west");

        assert_eq!(strategy.partition_for_key(&key_us), 0);
        assert_eq!(strategy.partition_for_key(&key_eu), 1);
    }

    #[test]
    fn test_partition_range() {
        let range = PartitionRange::new(100, 200);

        assert!(range.contains(100));
        assert!(range.contains(150));
        assert!(!range.contains(200));
        assert!(!range.contains(50));
    }

    #[test]
    fn test_partition_range_split() {
        let range = PartitionRange::new(0, 1000);
        let parts = range.split(4);

        assert_eq!(parts.len(), 4);
        assert_eq!(parts[0].start, 0);
        assert_eq!(parts[0].end, 250);
        assert_eq!(parts[3].end, 1000);
    }

    #[test]
    fn test_partition_range_merge() {
        let r1 = PartitionRange::new(0, 100);
        let r2 = PartitionRange::new(100, 200);

        let merged = r1.merge(&r2).unwrap();
        assert_eq!(merged.start, 0);
        assert_eq!(merged.end, 200);
    }

    #[test]
    fn test_key_extractor() {
        let extractor = KeyExtractor::new(vec!["user_id".to_string()]);

        let mut values = std::collections::HashMap::new();
        values.insert("user_id".to_string(), "123".to_string());
        values.insert("name".to_string(), "Alice".to_string());

        let key = extractor.extract(&values).unwrap();
        assert_eq!(key, PartitionKey::String("123".to_string()));
    }

    #[test]
    fn test_key_extractor_composite() {
        let extractor = KeyExtractor::new(vec![
            "tenant_id".to_string(),
            "user_id".to_string(),
        ]);

        let mut values = std::collections::HashMap::new();
        values.insert("tenant_id".to_string(), "t1".to_string());
        values.insert("user_id".to_string(), "u1".to_string());

        let key = extractor.extract(&values).unwrap();
        match key {
            PartitionKey::Composite(keys) => {
                assert_eq!(keys.len(), 2);
            }
            _ => panic!("Expected composite key"),
        }
    }

    #[test]
    fn test_time_interval() {
        assert_eq!(TimeInterval::Hour.to_seconds(), 3600);
        assert_eq!(TimeInterval::Day.to_seconds(), 86400);
        assert_eq!(TimeInterval::Custom(7200).to_seconds(), 7200);
    }
}
