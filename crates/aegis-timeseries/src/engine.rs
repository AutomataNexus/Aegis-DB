//! Aegis Time Series Engine
//!
//! Core engine that coordinates all time series operations including
//! ingestion, storage, compression, and querying.
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use crate::aggregation::AggregateFunction;
use crate::cold::{ColdCompactReport, ColdStore};
use crate::compression::{
    compress_points_nexus, decode_block, is_nexus_block, CompressedBlock, Compressor,
};
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
    /// Days of raw data kept resident in RAM (the "hot" window). Older blocks are
    /// evicted by `enforce_retention` — into the cold tier when it is enabled.
    pub hot_retention_days: i64,
    /// Days to keep evicted blocks on disk in the cold tier (`None` = no cold tier:
    /// evicted data is discarded, the pre-cold-tier behaviour). Requires `data_path`.
    pub cold_retention_days: Option<i64>,
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
            hot_retention_days: 7,
            cold_retention_days: None,
        }
    }
}

/// Outcome of a `compress_section` call (returned to the admin endpoint).
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct CompressionReport {
    pub blocks_scanned: usize,
    pub blocks_recompressed: usize,
    pub blocks_already: usize,
    pub bytes_before: u64,
    pub bytes_after: u64,
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
    cold: Option<ColdStore>,
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

        let cold = match (&config.data_path, config.cold_retention_days) {
            (Some(path), Some(days)) if days > 0 => match ColdStore::open(path.join("cold")) {
                Ok(c) => Some(c),
                Err(e) => {
                    eprintln!("Failed to open timeseries cold tier at {:?}: {}", path, e);
                    None
                }
            },
            _ => None,
        };

        let mut engine = Self {
            partition_manager: PartitionManager::new(config.partition_config.clone()),
            config,
            index: TimeSeriesIndex::new(),
            retention_manager: RetentionManager::new(),
            series_data: RwLock::new(HashMap::new()),
            metrics: RwLock::new(HashMap::new()),
            stats: RwLock::new(EngineStats::default()),
            persistence,
            cold,
        };

        // Load persisted data if available
        engine.load_from_disk();
        // Cold-only series (evicted from the hot map entirely) must stay discoverable.
        if let Some(cold) = &engine.cold {
            for (_id, meta) in cold.series() {
                engine.index.register(&meta.metric, &meta.tags);
            }
        }
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

        let hot_cutoff = Utc::now() - Duration::days(self.config.hot_retention_days);
        let reach_cold = self.cold.as_ref().filter(|_| query.start < hot_cutoff);

        let series: Vec<Series> = {
            let data = self.series_data.read();
            series_ids
                .iter()
                .filter_map(|id| {
                    let hot = data
                        .get(id)
                        .map(|buffer| buffer.to_series_in_range(query.start, query.end));
                    let Some(cold) = reach_cold else { return hot };
                    let blocks = cold.read_range(
                        id,
                        query.start.timestamp_millis(),
                        query.end.timestamp_millis(),
                    );
                    if blocks.is_empty() {
                        return hot;
                    }
                    let mut points: Vec<DataPoint> = blocks
                        .iter()
                        .flat_map(decode_block)
                        .filter(|p| p.timestamp >= query.start && p.timestamp < query.end)
                        .collect();
                    match hot {
                        Some(mut s) => {
                            points.append(&mut s.points);
                            points.sort_by_key(|p| p.timestamp);
                            points.dedup_by_key(|p| p.timestamp);
                            Some(Series::with_points(s.metric, s.tags, points))
                        }
                        None => {
                            points.sort_by_key(|p| p.timestamp);
                            let meta = self.index.get(id)?;
                            let metric = self
                                .metrics
                                .read()
                                .get(&meta.metric_name)
                                .cloned()
                                .unwrap_or_else(|| Metric::gauge(&meta.metric_name));
                            Some(Series::with_points(metric, meta.tags, points))
                        }
                    }
                })
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

    /// Recompress cold Gorilla blocks of `metric` that overlap `[start_ms, end_ms]`
    /// with NexusCompress for a higher ratio. An empty `metric` matches all series.
    /// Reads stay transparent (blocks are decoded by codec on query), and the
    /// authoritative range bounds are preserved so block-skipping is unaffected.
    /// Already-NexusCompress blocks are skipped, and any block whose recompression
    /// fails is left untouched (fail-safe — never loses data). The caller should
    /// `flush()` afterward so the recompressed blocks survive a restart.
    ///
    /// The expensive decode+re-encode runs WITHOUT any lock held: candidate
    /// blocks are snapshotted under a brief read lock, recompressed lock-free,
    /// then swapped back in under a brief write lock by matching the original
    /// block (checksum + bounds). So ingestion/queries are only blocked for the
    /// two short critical sections, not for the whole CPU-bound recompression.
    pub fn compress_section(&self, metric: &str, start_ms: i64, end_ms: i64) -> CompressionReport {
        let mut report = CompressionReport::default();

        // Phase 1 — brief READ lock: snapshot candidate (cold, in-range, Gorilla)
        // blocks. Cloning cold block bytes is cheap relative to holding the lock.
        let candidates: Vec<(String, CompressedBlock)> = {
            let data = self.series_data.read();
            let mut c = Vec::new();
            for (sid, buffer) in data.iter() {
                if !metric.is_empty() && buffer.metric.name != metric {
                    continue;
                }
                for block in &buffer.compressed_blocks {
                    report.blocks_scanned += 1;
                    if is_nexus_block(block) {
                        report.blocks_already += 1;
                        continue;
                    }
                    if block.last_timestamp < start_ms || block.first_timestamp > end_ms {
                        continue;
                    }
                    c.push((sid.clone(), block.clone()));
                }
            }
            c
        };

        // Phase 2 — NO lock: the CPU-bound decode + NexusCompress re-encode.
        let mut prepared: Vec<(String, CompressedBlock, CompressedBlock)> =
            Vec::with_capacity(candidates.len());
        for (sid, old) in candidates {
            let points = decode_block(&old);
            if let Some(mut nb) = compress_points_nexus(&points) {
                // Preserve the original bounds so range-skipping stays correct.
                nb.first_timestamp = old.first_timestamp;
                nb.last_timestamp = old.last_timestamp;
                prepared.push((sid, old, nb));
            }
        }

        // Phase 3 — brief WRITE lock: swap each recompressed block in by matching
        // the original (checksum + bounds, still Gorilla). If a block changed or
        // was pruned (retention/compaction) between phases, it simply isn't found
        // and is skipped — never corrupting newer state.
        if !prepared.is_empty() {
            let mut data = self.series_data.write();
            for (sid, old, nb) in prepared {
                let Some(buffer) = data.get_mut(&sid) else {
                    continue;
                };
                let after = nb.data.len() as u64;
                if let Some(block) = buffer.compressed_blocks.iter_mut().find(|b| {
                    !is_nexus_block(b)
                        && b.checksum == old.checksum
                        && b.first_timestamp == old.first_timestamp
                        && b.last_timestamp == old.last_timestamp
                        && b.count == old.count
                }) {
                    report.bytes_before += block.data.len() as u64;
                    report.bytes_after += after;
                    *block = nb;
                    report.blocks_recompressed += 1;
                }
            }
        }
        report
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

    /// Age-based retention on the resident series map: drop every point/block older
    /// than `cutoff`, removing series that become empty. This is what was never
    /// applied — the map grew unbounded until the memory cap OOM-killed the process.
    /// Returns (compressed_blocks_dropped, series_removed).
    pub fn enforce_retention(&self, cutoff: DateTime<Utc>) -> (usize, usize) {
        let cutoff_ms = cutoff.timestamp_millis();
        let mut blocks_dropped = 0usize;
        let mut series_removed = 0usize;
        // Evicted blocks are collected under the lock and written to the cold tier
        // after it is released, so a slow disk never stalls ingestion.
        let mut evicted: Vec<(String, Metric, Tags, Vec<CompressedBlock>)> = Vec::new();
        {
            let mut data = self.series_data.write();
            data.retain(|id, buf| {
                let blocks = buf.prune_before(cutoff_ms, cutoff);
                blocks_dropped += blocks.len();
                if self.cold.is_some() && !blocks.is_empty() {
                    evicted.push((id.clone(), buf.metric.clone(), buf.tags.clone(), blocks));
                }
                if buf.points.is_empty() && buf.compressed_blocks.is_empty() {
                    series_removed += 1;
                    false
                } else {
                    true
                }
            });
        }
        if let Some(cold) = &self.cold {
            for (id, metric, tags, blocks) in evicted {
                if let Err(e) = cold.append(&id, &metric, &tags, &blocks) {
                    eprintln!(
                        "cold tier: failed to append {} blocks for {}: {}",
                        blocks.len(),
                        id,
                        e
                    );
                }
            }
        }
        (blocks_dropped, series_removed)
    }

    /// Enforce cold-tier retention (`cold_retention_days`): drop cold frames older than
    /// the cutoff and rewrite/remove the affected files. No-op without a cold tier.
    pub fn compact_cold(&self, now: DateTime<Utc>) -> Option<ColdCompactReport> {
        let cold = self.cold.as_ref()?;
        let days = self.config.cold_retention_days?;
        let cutoff = (now - Duration::days(days)).timestamp_millis();
        let report = cold.compact(cutoff);
        // Series that are now gone from both tiers should stop resolving in queries.
        if report.files_removed > 0 {
            let data = self.series_data.read();
            for id in self.index.all_series_ids() {
                if !data.contains_key(&id) && !cold.contains(&id) {
                    self.index.remove(&id);
                }
            }
        }
        Some(report)
    }

    /// Cold-tier summary: (series with cold data, bytes on disk). `None` if disabled.
    pub fn cold_stats(&self) -> Option<(usize, u64)> {
        self.cold.as_ref().map(|c| (c.len(), c.disk_bytes()))
    }

    /// Hot window length in days (what `enforce_retention` callers should use).
    pub fn hot_retention_days(&self) -> i64 {
        self.config.hot_retention_days
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

/// Buffer for a single time series.
struct SeriesBuffer {
    metric: Metric,
    tags: Tags,
    points: Vec<DataPoint>,
    compressed_blocks: Vec<CompressedBlock>,
}

impl SeriesBuffer {
    fn new(metric: Metric, tags: Tags) -> Self {
        Self {
            metric,
            tags,
            points: Vec::new(),
            compressed_blocks: Vec::new(),
        }
    }

    fn add_point(&mut self, point: DataPoint) {
        self.points.push(point);
    }

    fn compress(&mut self) {
        if self.points.is_empty() {
            return;
        }

        let mut compressor = Compressor::new();
        for point in &self.points {
            compressor.compress(point);
        }

        let block = compressor.finish();
        self.compressed_blocks.push(block);
        self.points.clear();
    }

    /// Drop all data older than `cutoff`: whole compressed blocks whose newest
    /// point predates it, plus any uncompressed tail points before it (those are
    /// compressed into one block so nothing is lost). Returns the evicted blocks,
    /// oldest first, for the caller to hand to the cold tier or discard.
    fn prune_before(&mut self, cutoff_ms: i64, cutoff: DateTime<Utc>) -> Vec<CompressedBlock> {
        let mut evicted = Vec::new();
        let (old, keep): (Vec<CompressedBlock>, Vec<CompressedBlock>) = self
            .compressed_blocks
            .drain(..)
            .partition(|b| b.last_timestamp < cutoff_ms);
        self.compressed_blocks = keep;
        evicted.extend(old);

        let (old_pts, keep_pts): (Vec<DataPoint>, Vec<DataPoint>) =
            self.points.drain(..).partition(|p| p.timestamp < cutoff);
        self.points = keep_pts;
        if !old_pts.is_empty() {
            let mut c = Compressor::new();
            for p in &old_pts {
                c.compress(p);
            }
            evicted.push(c.finish());
        }
        evicted
    }

    fn to_series(&self) -> Series {
        let mut all_points = Vec::new();

        for block in &self.compressed_blocks {
            all_points.extend(decode_block(block));
        }

        all_points.extend(self.points.clone());
        all_points.sort_by_key(|p| p.timestamp);

        Series::with_points(self.metric.clone(), self.tags.clone(), all_points)
    }

    /// Query-optimized version that skips blocks outside the time range.
    fn to_series_in_range(&self, start: DateTime<Utc>, end: DateTime<Utc>) -> Series {
        let mut all_points = Vec::new();
        let start_ms = start.timestamp_millis();
        let end_ms = end.timestamp_millis();

        for block in &self.compressed_blocks {
            // Skip blocks entirely outside the query range
            if block.last_timestamp < start_ms || block.first_timestamp > end_ms {
                continue;
            }

            all_points.extend(
                decode_block(block)
                    .into_iter()
                    .filter(|p| p.timestamp >= start && p.timestamp < end),
            );
        }

        all_points.extend(
            self.points
                .iter()
                .filter(|p| p.timestamp >= start && p.timestamp < end)
                .cloned(),
        );
        all_points.sort_by_key(|p| p.timestamp);

        Series::with_points(self.metric.clone(), self.tags.clone(), all_points)
    }

    fn memory_usage(&self) -> usize {
        let points_size = self.points.len() * std::mem::size_of::<DataPoint>();
        let blocks_size: usize = self.compressed_blocks.iter().map(|b| b.data.len()).sum();
        points_size + blocks_size
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
            tags.insert("host", format!("server{}", i));
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

    #[test]
    fn test_compress_section_transparent_and_survives_restart() {
        fn snapshot(engine: &TimeSeriesEngine, metric: &str) -> Vec<(i64, f64)> {
            let mut out: Vec<(i64, f64)> = engine
                .get_series(metric)
                .iter()
                .flat_map(|s| {
                    s.points
                        .iter()
                        .map(|p| (p.timestamp.timestamp_millis(), p.value))
                })
                .collect();
            out.sort_by_key(|(t, _)| *t);
            out
        }

        let dir = tempfile::TempDir::new().expect("tempdir");
        let config = EngineConfig {
            data_path: Some(dir.path().to_path_buf()),
            compression_threshold: 100,
            ..Default::default()
        };

        let baseline = {
            let engine = TimeSeriesEngine::with_config(config.clone());
            let mut tags = Tags::new();
            tags.insert("host", "server1");
            let base = Utc::now() - Duration::seconds(500);
            for i in 0..300 {
                engine
                    .write(
                        "metric_x",
                        tags.clone(),
                        DataPoint {
                            timestamp: base + Duration::seconds(i),
                            value: 50.0 + (i as f64 % 7.0),
                        },
                    )
                    .expect("write");
            }
            engine.compact(); // force any buffered points into a Gorilla block
            let before = snapshot(&engine, "metric_x");
            assert!(!before.is_empty());

            // Recompress the whole range with NexusCompress.
            let report = engine.compress_section("metric_x", i64::MIN, i64::MAX);
            assert!(
                report.blocks_recompressed >= 1,
                "expected at least one block recompressed, got {:?}",
                report
            );
            assert!(report.bytes_after > 0);

            // Reads are transparent: data is identical after recompression.
            assert_eq!(before, snapshot(&engine, "metric_x"));

            engine.flush();
            before
        };

        // Restart: a fresh engine over the same data_path must read identically,
        // proving the NexusCompress blocks survived the bincode persistence cycle.
        let reloaded = TimeSeriesEngine::with_config(config);
        assert_eq!(baseline, snapshot(&reloaded, "metric_x"));
    }
}

#[cfg(test)]
mod cold_tier_tests {
    use super::*;

    #[test]
    fn retention_evicts_to_cold_and_queries_merge() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = EngineConfig {
            data_path: Some(dir.path().to_path_buf()),
            hot_retention_days: 7,
            cold_retention_days: Some(365),
            compression_threshold: 5,
            ..Default::default()
        };
        let engine = TimeSeriesEngine::with_config(cfg.clone());
        let mut tags = Tags::default();
        tags.0.insert("unit".into(), "ahu1".into());
        let now = Utc::now();
        // 30 old points (10 days back, 1/min) → compressed into blocks + 30 recent.
        let old: Vec<DataPoint> = (0..30)
            .map(|i| DataPoint {
                timestamp: now - Duration::days(10) + Duration::minutes(i),
                value: i as f64,
            })
            .collect();
        let recent: Vec<DataPoint> = (0..30)
            .map(|i| DataPoint {
                timestamp: now - Duration::hours(1) + Duration::minutes(i),
                value: 100.0 + i as f64,
            })
            .collect();
        engine
            .write_batch("supply_temp", tags.clone(), old)
            .unwrap();
        engine
            .write_batch("supply_temp", tags.clone(), recent)
            .unwrap();

        let (evicted, removed) = engine.enforce_retention(now - Duration::days(7));
        assert!(evicted >= 1, "old blocks should be evicted");
        assert_eq!(removed, 0);
        let (cold_series, bytes) = engine.cold_stats().unwrap();
        assert_eq!(cold_series, 1);
        assert!(bytes > 0);

        // Recent-only query never touches cold.
        let q = TimeSeriesQuery::new(
            "supply_temp",
            now - Duration::days(1),
            now + Duration::minutes(1),
        );
        let r = engine.query(&q);
        assert_eq!(r.series[0].points.len(), 30);

        // Long-range query merges hot + cold: all 60 points, ordered.
        let q = TimeSeriesQuery::new(
            "supply_temp",
            now - Duration::days(20),
            now + Duration::minutes(1),
        );
        let r = engine.query(&q);
        assert_eq!(r.series.len(), 1);
        let pts = &r.series[0].points;
        assert_eq!(pts.len(), 60);
        assert!(pts.windows(2).all(|w| w[0].timestamp < w[1].timestamp));
        assert_eq!(pts[0].value, 0.0);
        assert_eq!(pts[59].value, 129.0);

        // Evict everything hot → series is cold-only, still discoverable after a restart.
        let (_, removed) = engine.enforce_retention(now + Duration::days(1));
        assert_eq!(removed, 1);
        drop(engine);
        let engine = TimeSeriesEngine::with_config(cfg);
        let q = TimeSeriesQuery::new(
            "supply_temp",
            now - Duration::days(20),
            now + Duration::days(2),
        );
        let r = engine.query(&q);
        assert_eq!(r.series.len(), 1);
        assert_eq!(r.series[0].points.len(), 60);

        // Cold compaction past everything removes the series entirely.
        let rep = engine.compact_cold(now + Duration::days(400)).unwrap();
        assert_eq!(rep.files_removed, 1);
        assert_eq!(engine.series_count(), 0);
    }
}
