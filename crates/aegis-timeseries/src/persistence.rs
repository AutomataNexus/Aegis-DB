//! Aegis Time Series Persistence
//!
//! Serialization and deserialization of time series data for crash recovery.
//! Uses bincode for compact binary encoding of series buffers and compressed blocks.
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use crate::compression::CompressedBlock;
use crate::types::{DataPoint, Metric, Tags};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

// =============================================================================
// Persisted Data Structures
// =============================================================================

/// Serializable representation of all time series data.
#[derive(Serialize, Deserialize)]
pub struct PersistedState {
    pub version: u32,
    pub metrics: Vec<Metric>,
    pub series: Vec<PersistedSeries>,
}

/// Serializable representation of a single series buffer.
#[derive(Serialize, Deserialize)]
pub struct PersistedSeries {
    pub series_id: String,
    pub metric: Metric,
    pub tags: Tags,
    pub points: Vec<DataPoint>,
    pub compressed_blocks: Vec<PersistedBlock>,
}

/// Serializable representation of a compressed block.
#[derive(Serialize, Deserialize)]
pub struct PersistedBlock {
    pub data: Vec<u8>,
    pub first_timestamp: i64,
    pub last_timestamp: i64,
    pub count: usize,
    pub checksum: u32,
}

impl From<&CompressedBlock> for PersistedBlock {
    fn from(block: &CompressedBlock) -> Self {
        Self {
            data: block.data.clone(),
            first_timestamp: block.first_timestamp,
            last_timestamp: block.last_timestamp,
            count: block.count,
            checksum: block.checksum,
        }
    }
}

impl From<PersistedBlock> for CompressedBlock {
    fn from(pb: PersistedBlock) -> Self {
        Self {
            data: pb.data,
            first_timestamp: pb.first_timestamp,
            last_timestamp: pb.last_timestamp,
            count: pb.count,
            checksum: pb.checksum,
        }
    }
}

// =============================================================================
// Persistence Manager
// =============================================================================

/// Manages reading and writing time series data to disk.
pub struct PersistenceManager {
    data_path: PathBuf,
}

impl PersistenceManager {
    /// Create a new persistence manager for the given directory.
    pub fn new(data_path: impl Into<PathBuf>) -> std::io::Result<Self> {
        let data_path = data_path.into();
        std::fs::create_dir_all(&data_path)?;
        Ok(Self { data_path })
    }

    /// Path to the main data file.
    fn data_file(&self) -> PathBuf {
        self.data_path.join("timeseries.bin")
    }

    /// Path to the temporary write file (for atomic writes).
    fn temp_file(&self) -> PathBuf {
        self.data_path.join("timeseries.bin.tmp")
    }

    /// Save state to disk using atomic write (write to temp, then rename).
    pub fn save(&self, state: &PersistedState) -> Result<(), PersistenceError> {
        let encoded = bincode::serialize(state)
            .map_err(|e| PersistenceError::SerializationError(e.to_string()))?;

        let temp_path = self.temp_file();
        let data_path = self.data_file();

        // Write to temp file first
        std::fs::write(&temp_path, &encoded)
            .map_err(|e| PersistenceError::IoError(e.to_string()))?;

        // Atomic rename
        std::fs::rename(&temp_path, &data_path)
            .map_err(|e| PersistenceError::IoError(e.to_string()))?;

        Ok(())
    }

    /// Load state from disk.
    pub fn load(&self) -> Result<Option<PersistedState>, PersistenceError> {
        let data_path = self.data_file();

        if !data_path.exists() {
            return Ok(None);
        }

        let data = std::fs::read(&data_path)
            .map_err(|e| PersistenceError::IoError(e.to_string()))?;

        if data.is_empty() {
            return Ok(None);
        }

        let state: PersistedState = bincode::deserialize(&data)
            .map_err(|e| PersistenceError::DeserializationError(e.to_string()))?;

        Ok(Some(state))
    }

    /// Check if persisted data exists.
    pub fn exists(&self) -> bool {
        self.data_file().exists()
    }

    /// Get the data directory path.
    pub fn data_path(&self) -> &Path {
        &self.data_path
    }
}

// =============================================================================
// Persistence Error
// =============================================================================

/// Errors that can occur during persistence operations.
#[derive(Debug, Clone)]
pub enum PersistenceError {
    IoError(String),
    SerializationError(String),
    DeserializationError(String),
}

impl std::fmt::Display for PersistenceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IoError(msg) => write!(f, "I/O error: {}", msg),
            Self::SerializationError(msg) => write!(f, "Serialization error: {}", msg),
            Self::DeserializationError(msg) => write!(f, "Deserialization error: {}", msg),
        }
    }
}

impl std::error::Error for PersistenceError {}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{DataPoint, Metric, Tags};
    use chrono::Utc;

    #[test]
    fn test_save_and_load() {
        let dir = tempfile::tempdir().unwrap();
        let pm = PersistenceManager::new(dir.path().join("ts")).unwrap();

        let mut tags = Tags::new();
        tags.insert("host", "server1");

        let state = PersistedState {
            version: 1,
            metrics: vec![Metric::gauge("cpu_usage")],
            series: vec![PersistedSeries {
                series_id: "cpu_usage:host=server1".to_string(),
                metric: Metric::gauge("cpu_usage"),
                tags,
                points: vec![DataPoint {
                    timestamp: Utc::now(),
                    value: 42.5,
                }],
                compressed_blocks: vec![],
            }],
        };

        pm.save(&state).unwrap();
        assert!(pm.exists());

        let loaded = pm.load().unwrap().unwrap();
        assert_eq!(loaded.version, 1);
        assert_eq!(loaded.metrics.len(), 1);
        assert_eq!(loaded.series.len(), 1);
        assert_eq!(loaded.series[0].points[0].value, 42.5);
    }

    #[test]
    fn test_load_nonexistent() {
        let dir = tempfile::tempdir().unwrap();
        let pm = PersistenceManager::new(dir.path().join("ts")).unwrap();
        let loaded = pm.load().unwrap();
        assert!(loaded.is_none());
    }

    #[test]
    fn test_persisted_block_roundtrip() {
        let block = CompressedBlock {
            data: vec![1, 2, 3, 4],
            first_timestamp: 1000,
            last_timestamp: 2000,
            count: 10,
            checksum: 12345,
        };

        let persisted: PersistedBlock = (&block).into();
        let restored: CompressedBlock = persisted.into();

        assert_eq!(restored.data, block.data);
        assert_eq!(restored.first_timestamp, block.first_timestamp);
        assert_eq!(restored.last_timestamp, block.last_timestamp);
        assert_eq!(restored.count, block.count);
        assert_eq!(restored.checksum, block.checksum);
    }
}
