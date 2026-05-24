//! Aegis Time Series Engine
//!
//! Core engine that coordinates all time series operations including
//! ingestion, storage, compression, and querying.
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use crate::aggregation::AggregateFunction;
use crate::compression::{CompressedBlock, Compressor, Decompressor};
use crate::index::TimeSeriesIndex;
use crate::partition::{PartitionConfig, PartitionManager};
use crate::persistence::{PersistedBlock, PersistedSeries, PersistedState, PersistenceManager};
use crate::query::{QueryExecutor, QueryResult, TimeSeriesQuery};
use crate::retention::{RetentionManager, RetentionResult};
use crate::types::{DataPoint, Metric, Series, Tags};
use chrono::{DateTime, Duration, Utc};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::path::PathBuf;

// =============================================================================
// Time Series Engine Configuration
// =============================================================================

/// Configuration for the time series engine.
#[derive(Debug, Clone)]
pub struct EngineConfig {
    pub partition_config: PartitionConfig,
    pub compression_enabled: bool,
    pub compression_threshold: usize,
    pub max_series_per_metric: usize,
    pub write_buffer_size: usize,
    pub data_path: Option<PathBuf>,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            partition_config: PartitionConfig::default(),
            compression_enabled: true,
            compression_threshold: 1000,
            max_series_per_metric: 100_000,
            write_buffer_size: 10_000,
            data_path: None,
        }
    }
}

// =============================================================================
// Time Series Engine
// =============================================================================

/// The main time series storage and query engine.
pub struct TimeSeriesEngine {
    config: EngineConfig,
    index: TimeSeriesIndex,
    partition_manager: PartitionManager,
    retention_manager: RetentionManager,
    series_data: RwLock<HashMap<String, SeriesBuffer>>,
    metrics: RwLock<HashMap<String, Metric>>,
    stats: RwLock<EngineStats>,
    persistence: Option<PersistenceManager>,
}

impl TimeSeriesEngine {
    /// Create a new time series engine with default configuration.
    pub fn new() -> Self {
        Self::with_config(EngineConfig::default())
    }

    /// Create a new time series engine with custom configuration.
    pub fn with_config(config: EngineConfig) -> Self {
        let persistence = config
            .data_path
            .as_ref()
            .and_then(|path| match PersistenceManager::new(path) {
                Ok(pm) => Some(pm),
                Err(e) => {
                    eprintln!(
                        "Failed to initialize timeseries persistence at {:?}: {}",
                        path, e
                    );
                    None
                }
            });

        let mut engine = Self {
            partition_manager: PartitionManager::new(config.partition_config.clone()),
            config,
            index: TimeSeriesIndex::new(),
            retention_manager: RetentionManager::new(),
            series_data: RwLock::new(HashMap::new()),
            metrics: RwLock::new(HashMap::new()),
            stats: RwLock::new(EngineStats::default()),
            persistence,
        };

        // Load persisted data if available
        engine.load_from_disk();
        engine
    }

    // -------------------------------------------------------------------------
    // Metric Registration
    // -------------------------------------------------------------------------

    /// Register a new metric.
    pub fn register_metric(&self, metric: Metric) -> Result<(), EngineError> {
        let mut metrics = self.metrics.write();

        if metrics.contains_key(&metric.name) {
            return Err(EngineError::MetricAlreadyExists(metric.name));
        }

        metrics.insert(metric.name.clone(), metric);
        Ok(())
    }

    /// Get a registered metric by name.
    pub fn get_metric(&self, name: &str) -> Option<Metric> {
        let metrics = self.metrics.read();
        metrics.get(name).cloned()
    }

    /// List all registered metrics.
    pub fn list_metrics(&self) -> Vec<Metric> {
        let metrics = self.metrics.read();
        metrics.values().cloned().collect()
    }

    // -------------------------------------------------------------------------
    // Data Ingestion
    // -------------------------------------------------------------------------

    /// Write a single data point.
    pub fn write(
        &self,
        metric_name: &str,
        tags: Tags,
        point: DataPoint,
    ) -> Result<(), EngineError> {
        self.write_batch(metric_name, tags, vec![point])
    }

    /// Write a batch of data points.
    pub fn write_batch(
        &self,
        metric_name: &str,
        tags: Tags,
        points: Vec<DataPoint>,
    ) -> Result<(), EngineError> {
        if points.is_empty() {
            return Ok(());
        }

        let metric = {
            let metrics = self.metrics.read();
            metrics.get(metric_name).cloned()
        };

        let metric = metric.unwrap_or_else(|| Metric::gauge(metric_name));

        // Enforce cardinality limit before registering new series
        let existing_series = self.index.find_by_metric(metric_name);
        let series_id = self.index.register(&metric, &tags);
        if !existing_series.contains(&series_id)
            && existing_series.len() >= self.config.max_series_per_metric
        {
            return Err(EngineError::StorageError(format!(
                "Metric '{}' exceeds max_series_per_metric limit of {}",
                metric_name, self.config.max_series_per_metric
            )));
        }

        {
            let mut data = self.series_data.write();
            let buffer = data
                .entry(series_id.clone())
                .or_insert_with(|| SeriesBuffer::new(metric.clone(), tags.clone()));

            for point in &points {
                buffer.add_point(point.clone());
            }

            if self.config.compression_enabled
                && buffer.points.len() >= self.config.compression_threshold
            {
                buffer.compress();
            }
        }

        {
            let mut stats = self.stats.write();
            stats.points_written += points.len() as u64;
            stats.bytes_written += (points.len() * 16) as u64;
        }

        // Global memory pressure check — compress all buffers when total
        // raw point memory exceeds 150MB. Prevents unbounded RSS growth
        // across hundreds of series that individually stay below the
        // per-buffer compression_threshold.
        const MEMORY_PRESSURE_BYTES: usize = 150 * 1024 * 1024;
        if self.memory_usage() > MEMORY_PRESSURE_BYTES {
            self.compact();
        }

        Ok(())
    }

    /// Write a point with the current timestamp.
    pub fn write_now(&self, metric_name: &str, tags: Tags, value: f64) -> Result<(), EngineError> {
        let point = DataPoint {
            timestamp: Utc::now(),
            value,
        };
        self.write(metric_name, tags, point)
    }

    // -------------------------------------------------------------------------
    // Querying
    // -------------------------------------------------------------------------

    /// Execute a time series query.
    pub fn query(&self, query: &TimeSeriesQuery) -> QueryResult {
        let start_time = std::time::Instant::now();

        let series_ids = if let Some(ref tags) = query.tags {
            self.index.find_by_tags(tags)
        } else {
            self.index.find_by_metric(&query.metric)
        };

        let series: Vec<Series> = {
            let data = self.series_data.read();
            series_ids
                .iter()
                .filter_map(|id| data.get(id))
                .map(|buffer| buffer.to_series_in_range(query.start, query.end))
                .collect()
        };

        let mut result = QueryExecutor::execute(query, series);
        result.query_time_ms = start_time.elapsed().as_millis() as u64;

        {
            let mut stats = self.stats.write();
            stats.queries_executed += 1;
            stats.points_scanned += result.points_scanned as u64;
        }

        result
    }

    /// Get the latest value for a metric.
    pub fn latest(&self, metric_name: &str, tags: Option<&Tags>) -> Option<DataPoint> {
        let query = TimeSeriesQuery::last(metric_name, Duration::minutes(5)).with_limit(1);

        let query = if let Some(t) = tags {
            query.with_tags(t.clone())
        } else {
            query
        };

        let result = self.query(&query);
        result.series.first().and_then(|s| s.points.last().cloned())
    }

    /// Get aggregated value over a time range.
    pub fn aggregate(
        &self,
        metric_name: &str,
        duration: Duration,
        function: AggregateFunction,
    ) -> Option<f64> {
        let query = TimeSeriesQuery::last(metric_name, duration)
            .with_aggregation(crate::query::QueryAggregation::Instant { function });

        let result = self.query(&query);
        result
            .series
            .first()
            .and_then(|s| s.points.first())
            .map(|p| p.value)
    }

    // -------------------------------------------------------------------------
    // Series Management
    // -------------------------------------------------------------------------

    /// Get all series for a metric.
    pub fn get_series(&self, metric_name: &str) -> Vec<Series> {
        let series_ids = self.index.find_by_metric(metric_name);
        let data = self.series_data.read();

        series_ids
            .iter()
            .filter_map(|id| data.get(id))
            .map(|buffer| buffer.to_series())
            .collect()
    }

    /// Get a specific series by ID.
    pub fn get_series_by_id(&self, series_id: &str) -> Option<Series> {
        let data = self.series_data.read();
        data.get(series_id).map(|buffer| buffer.to_series())
    }

    /// Delete a series.
    pub fn delete_series(&self, series_id: &str) -> bool {
        let removed = {
            let mut data = self.series_data.write();
            data.remove(series_id).is_some()
        };

        if removed {
            self.index.remove(series_id);
        }

        removed
    }

    /// Get the number of active series.
    pub fn series_count(&self) -> usize {
        self.index.len()
    }

    // -------------------------------------------------------------------------
    // Index Access
    // -------------------------------------------------------------------------

    /// Get all tag keys.
    pub fn tag_keys(&self) -> Vec<String> {
        self.index.tag_keys()
    }

    /// Get all values for a tag key.
    pub fn tag_values(&self, key: &str) -> Vec<String> {
        self.index.tag_values(key)
    }

    /// Get all metric names.
    pub fn metric_names(&self) -> Vec<String> {
        self.index.metric_names()
    }

    // -------------------------------------------------------------------------
    // Retention Management
    // -------------------------------------------------------------------------

    /// Set the retention manager.
    pub fn set_retention_manager(&mut self, manager: RetentionManager) {
        self.retention_manager = manager;
    }

    /// Apply retention policies.
    pub fn apply_retention(&self) -> RetentionResult {
        self.retention_manager.apply(&self.partition_manager)
    }

    // -------------------------------------------------------------------------
    // Statistics
    // -------------------------------------------------------------------------

    /// Get engine statistics.
    pub fn stats(&self) -> EngineStats {
        let stats = self.stats.read();
        stats.clone()
    }

    /// Reset statistics.
    pub fn reset_stats(&self) {
        let mut stats = self.stats.write();
        *stats = EngineStats::default();
    }

    /// Get memory usage estimate.
    pub fn memory_usage(&self) -> usize {
        let data = self.series_data.read();
        data.values().map(|b| b.memory_usage()).sum()
    }

    // -------------------------------------------------------------------------
    // Maintenance
    // -------------------------------------------------------------------------

    /// Compact all series buffers.
    pub fn compact(&self) {
        let mut data = self.series_data.write();
        for buffer in data.values_mut() {
            buffer.compress();
        }
    }

    /// Prune data points older than `cutoff` from all series buffers.
    /// Returns the number of points removed.
    pub fn prune_before(&self, cutoff: chrono::DateTime<Utc>) -> usize {
        let mut data = self.series_data.write();
        let mut total_removed = 0;
        let mut empty_keys = Vec::new();

        for (key, buffer) in data.iter_mut() {
            total_removed += buffer.prune_before(cutoff);
            if buffer.points.is_empty() && buffer.compressed_blocks.is_empty() {
                empty_keys.push(key.clone());
            }
        }

        for key in &empty_keys {
            data.remove(key);
        }

        total_removed
    }

    /// Flush all pending writes to disk.
    pub fn flush(&self) {
        if let Some(ref pm) = self.persistence {
            let metrics: Vec<Metric> = {
                let m = self.metrics.read();
                m.values().cloned().collect()
            };

            let series: Vec<PersistedSeries> = {
                let data = self.series_data.read();
                data.iter()
                    .map(|(id, buffer)| PersistedSeries {
                        series_id: id.clone(),
                        metric: buffer.metric.clone(),
                        tags: buffer.tags.clone(),
                        points: buffer.points.clone(),
                        compressed_blocks: buffer
                            .compressed_blocks
                            .iter()
                            .map(PersistedBlock::from)
                            .collect(),
                        nc_frames: buffer.nc_frames.clone(),
                    })
                    .collect()
            };

            let state = PersistedState {
                version: 1,
                metrics,
                series,
            };

            if let Err(e) = pm.save(&state) {
                eprintln!("Failed to flush timeseries data: {}", e);
            }
        }

        let mut stats = self.stats.write();
        stats.last_flush = Some(Utc::now());
    }

    /// Load persisted data from disk.
    fn load_from_disk(&mut self) {
        let Some(ref pm) = self.persistence else {
            return;
        };

        let state = match pm.load() {
            Ok(Some(s)) => s,
            Ok(None) => return,
            Err(e) => {
                eprintln!("Failed to load timeseries data: {}", e);
                return;
            }
        };

        // Restore metrics
        {
            let mut metrics = self.metrics.write();
            for metric in &state.metrics {
                metrics.insert(metric.name.clone(), metric.clone());
            }
        }

        // Restore series data and rebuild index
        {
            let mut data = self.series_data.write();
            for ps in state.series {
                // Re-register in index
                self.index.register(&ps.metric, &ps.tags);

                let buffer = SeriesBuffer {
                    metric: ps.metric,
                    tags: ps.tags,
                    points: ps.points,
                    compressed_blocks: ps
                        .compressed_blocks
                        .into_iter()
                        .map(CompressedBlock::from)
                        .collect(),
                    nc_frames: ps.nc_frames,
                };

                data.insert(ps.series_id, buffer);
            }
        }
    }
}

impl Default for TimeSeriesEngine {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Series Buffer
// =============================================================================

/// NexusCompress frame — stores compressed timeseries data with timestamp
/// bounds for range-skip during queries.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NcFrame {
    data: Vec<u8>,
    count: usize,
    first_ts_ms: i64,
    last_ts_ms: i64,
}

impl NcFrame {
    fn decompress_points(&self) -> Vec<DataPoint> {
        use nexuscompress_core::{DType, Domain, schema::SchemaBuilder};
        use nexuscompress_timeseries::TimeSeriesCompressor;

        let schema = SchemaBuilder::new()
            .type_name("timeseries")
            .domain(Domain::TimeSeries)
            .field("timestamp", DType::I64)
            .field("value", DType::F64)
            .build();
        let nc = TimeSeriesCompressor::from_schema(schema);
        let fields = match nc.decompress_batch(&self.data) {
            Ok(f) => f,
            Err(_) => return Vec::new(),
        };
        if fields.len() < 2 {
            return Vec::new();
        }
        let ts_bytes = &fields[0];
        let val_bytes = &fields[1];
        let n = ts_bytes.len() / 8;
        (0..n)
            .filter_map(|i| {
                let ts_ms = i64::from_le_bytes(ts_bytes[i * 8..(i + 1) * 8].try_into().ok()?);
                let val = f64::from_le_bytes(val_bytes[i * 8..(i + 1) * 8].try_into().ok()?);
                let dt = chrono::DateTime::from_timestamp_millis(ts_ms)?;
                Some(DataPoint { timestamp: dt, value: val })
            })
            .collect()
    }
}

/// Buffer for a single time series.
struct SeriesBuffer {
    metric: Metric,
    tags: Tags,
    points: Vec<DataPoint>,
    compressed_blocks: Vec<CompressedBlock>,
    nc_frames: Vec<NcFrame>,
}

impl SeriesBuffer {
    fn new(metric: Metric, tags: Tags) -> Self {
        Self {
            metric,
            tags,
            points: Vec::new(),
            compressed_blocks: Vec::new(),
            nc_frames: Vec::new(),
        }
    }

    fn add_point(&mut self, point: DataPoint) {
        self.points.push(point);
    }

    fn compress(&mut self) {
        if self.points.is_empty() {
            return;
        }

        use nexuscompress_core::{DType, Domain, schema::SchemaBuilder};
        use nexuscompress_timeseries::TimeSeriesCompressor;

        let schema = SchemaBuilder::new()
            .type_name("timeseries")
            .domain(Domain::TimeSeries)
            .field("timestamp", DType::I64)
            .field("value", DType::F64)
            .build();
        let nc = TimeSeriesCompressor::from_schema(schema);
        let timestamps: Vec<u8> = self.points.iter()
            .flat_map(|p| p.timestamp.timestamp_millis().to_le_bytes())
            .collect();
        let values: Vec<u8> = self.points.iter()
            .flat_map(|p| p.value.to_le_bytes())
            .collect();
        let first_ts = self.points.first().map(|p| p.timestamp.timestamp_millis()).unwrap_or(0);
        let last_ts = self.points.last().map(|p| p.timestamp.timestamp_millis()).unwrap_or(0);
        let count = self.points.len();

        match nc.compress_batch(&vec![timestamps, values], count as u64) {
            Ok(frame_data) => {
                self.nc_frames.push(NcFrame {
                    data: frame_data,
                    count,
                    first_ts_ms: first_ts,
                    last_ts_ms: last_ts,
                });
                self.points.clear();
            }
            Err(e) => {
                eprintln!("NexusCompress failed, falling back to Gorilla: {e}");
                let mut compressor = Compressor::new();
                for point in &self.points {
                    compressor.compress(point);
                }
                self.compressed_blocks.push(compressor.finish());
                self.points.clear();
            }
        }
    }

    fn prune_before(&mut self, cutoff: chrono::DateTime<Utc>) -> usize {
        let before = self.points.len();
        self.points.retain(|p| p.timestamp >= cutoff);
        let points_removed = before - self.points.len();

        let cutoff_ms = cutoff.timestamp_millis();

        self.compressed_blocks.retain(|block| {
            let mut d = Decompressor::new(block);
            let pts = d.decompress_all();
            pts.iter().any(|p| p.timestamp >= cutoff)
        });

        self.nc_frames.retain(|f| f.last_ts_ms >= cutoff_ms);

        points_removed
    }

    fn to_series(&self) -> Series {
        let mut all_points = Vec::new();

        for block in &self.compressed_blocks {
            let mut decompressor = Decompressor::new(block);
            all_points.extend(decompressor.decompress_all());
        }

        for frame in &self.nc_frames {
            all_points.extend(frame.decompress_points());
        }

        all_points.extend(self.points.clone());
        all_points.sort_by_key(|p| p.timestamp);

        Series::with_points(self.metric.clone(), self.tags.clone(), all_points)
    }

    fn to_series_in_range(&self, start: DateTime<Utc>, end: DateTime<Utc>) -> Series {
        let mut all_points = Vec::new();
        let start_ms = start.timestamp_millis();
        let end_ms = end.timestamp_millis();

        for block in &self.compressed_blocks {
            if block.last_timestamp < start_ms || block.first_timestamp > end_ms {
                continue;
            }
            let mut decompressor = Decompressor::new(block);
            let points = decompressor.decompress_all();
            all_points.extend(
                points.into_iter().filter(|p| p.timestamp >= start && p.timestamp < end),
            );
        }

        for frame in &self.nc_frames {
            if frame.last_ts_ms < start_ms || frame.first_ts_ms > end_ms {
                continue;
            }
            all_points.extend(
                frame.decompress_points().into_iter()
                    .filter(|p| p.timestamp >= start && p.timestamp < end),
            );
        }

        all_points.extend(
            self.points.iter()
                .filter(|p| p.timestamp >= start && p.timestamp < end)
                .cloned(),
        );
        all_points.sort_by_key(|p| p.timestamp);

        Series::with_points(self.metric.clone(), self.tags.clone(), all_points)
    }

    fn memory_usage(&self) -> usize {
        let points_size = self.points.len() * std::mem::size_of::<DataPoint>();
        let blocks_size: usize = self.compressed_blocks.iter().map(|b| b.data.len()).sum();
        let nc_size: usize = self.nc_frames.iter().map(|f| f.data.len()).sum();
        points_size + blocks_size + nc_size
    }
}

// =============================================================================
// Engine Statistics
// =============================================================================

/// Statistics for the time series engine.
#[derive(Debug, Clone, Default)]
pub struct EngineStats {
    pub points_written: u64,
    pub bytes_written: u64,
    pub points_scanned: u64,
    pub queries_executed: u64,
    pub compression_ratio: f64,
    pub last_flush: Option<DateTime<Utc>>,
}

// =============================================================================
// Engine Error
// =============================================================================

/// Errors that can occur in the time series engine.
#[derive(Debug, Clone)]
pub enum EngineError {
    MetricAlreadyExists(String),
    MetricNotFound(String),
    SeriesNotFound(String),
    InvalidDataPoint(String),
    StorageError(String),
}

impl std::fmt::Display for EngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MetricAlreadyExists(name) => write!(f, "Metric already exists: {}", name),
            Self::MetricNotFound(name) => write!(f, "Metric not found: {}", name),
            Self::SeriesNotFound(id) => write!(f, "Series not found: {}", id),
            Self::InvalidDataPoint(msg) => write!(f, "Invalid data point: {}", msg),
            Self::StorageError(msg) => write!(f, "Storage error: {}", msg),
        }
    }
}

impl std::error::Error for EngineError {}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_engine_creation() {
        let engine = TimeSeriesEngine::new();
        assert_eq!(engine.series_count(), 0);
    }

    #[test]
    fn test_write_and_query() {
        let engine = TimeSeriesEngine::new();

        let mut tags = Tags::new();
        tags.insert("host", "server1");

        for i in 0..100 {
            let point = DataPoint {
                timestamp: Utc::now() - Duration::minutes(100 - i),
                value: i as f64,
            };
            engine
                .write("cpu_usage", tags.clone(), point)
                .expect("write should succeed");
        }

        let query = TimeSeriesQuery::last("cpu_usage", Duration::hours(2));
        let result = engine.query(&query);

        assert_eq!(result.series.len(), 1);
        assert!(result.points_returned > 0);
    }

    #[test]
    fn test_latest_value() {
        let engine = TimeSeriesEngine::new();

        let mut tags = Tags::new();
        tags.insert("host", "server1");

        engine
            .write_now("temperature", tags.clone(), 23.5)
            .expect("write_now should succeed");

        let latest = engine.latest("temperature", Some(&tags));
        assert!(latest.is_some());
        assert_eq!(latest.expect("latest should have value").value, 23.5);
    }

    #[test]
    fn test_aggregation() {
        let engine = TimeSeriesEngine::new();

        let tags = Tags::new();

        for i in 0..10 {
            let point = DataPoint {
                timestamp: Utc::now() - Duration::seconds(10 - i),
                value: i as f64,
            };
            engine
                .write("values", tags.clone(), point)
                .expect("write should succeed");
        }

        let avg = engine.aggregate("values", Duration::minutes(1), AggregateFunction::Avg);
        assert!(avg.is_some());
        assert!((avg.expect("avg should have value") - 4.5).abs() < 0.001);
    }

    #[test]
    fn test_multiple_series() {
        let engine = TimeSeriesEngine::new();

        for host in &["server1", "server2", "server3"] {
            let mut tags = Tags::new();
            tags.insert("host", *host);

            engine
                .write_now("cpu", tags, 50.0)
                .expect("write_now should succeed");
        }

        assert_eq!(engine.series_count(), 3);
    }

    #[test]
    fn test_tag_queries() {
        let engine = TimeSeriesEngine::new();

        let mut tags1 = Tags::new();
        tags1.insert("region", "us-east");
        tags1.insert("host", "server1");
        engine
            .write_now("memory", tags1, 1024.0)
            .expect("write_now should succeed");

        let mut tags2 = Tags::new();
        tags2.insert("region", "us-west");
        tags2.insert("host", "server2");
        engine
            .write_now("memory", tags2, 2048.0)
            .expect("write_now should succeed");

        let keys = engine.tag_keys();
        assert!(keys.contains(&"region".to_string()));
        assert!(keys.contains(&"host".to_string()));

        let regions = engine.tag_values("region");
        assert!(regions.contains(&"us-east".to_string()));
        assert!(regions.contains(&"us-west".to_string()));
    }

    #[test]
    fn test_delete_series() {
        let engine = TimeSeriesEngine::new();

        let mut tags = Tags::new();
        tags.insert("host", "server1");
        engine
            .write_now("test_metric", tags.clone(), 100.0)
            .expect("write_now should succeed");

        assert_eq!(engine.series_count(), 1);

        let series_ids = engine.index.find_by_metric("test_metric");
        assert_eq!(series_ids.len(), 1);

        let deleted = engine.delete_series(&series_ids[0]);
        assert!(deleted);
        assert_eq!(engine.series_count(), 0);
    }

    #[test]
    fn test_cardinality_limit() {
        let config = EngineConfig {
            max_series_per_metric: 3,
            ..Default::default()
        };
        let engine = TimeSeriesEngine::with_config(config);

        for i in 0..3 {
            let mut tags = Tags::new();
            tags.insert("host", &format!("server{}", i));
            engine
                .write_now("cpu", tags, 50.0)
                .expect("write should succeed");
        }

        let mut tags = Tags::new();
        tags.insert("host", "server_new");
        let result = engine.write_now("cpu", tags, 50.0);
        assert!(
            result.is_err(),
            "Should reject writes exceeding cardinality limit"
        );
    }
}
