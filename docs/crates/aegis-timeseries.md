---
layout: default
title: aegis-timeseries
parent: Crate Documentation
nav_order: 6
description: "Time series engine"
---

# aegis-timeseries

Time Series Engine for Aegis Database Platform.

## Overview

High-performance time series storage and query engine with Gorilla-style
compression, time-based partitioning, and flexible retention policies.

## Modules

### types.rs
Core data types for time series:
- `DataPoint` - Timestamp and value pair
- `Series` - Collection of data points with metric and tags
- `Metric` - Metric metadata (name, type, description, unit)
- `MetricType` - Counter, Gauge, Histogram, Summary
- `Tags` - Key-value labels for series identification

### partition.rs
Time-based partitioning:
- `PartitionInterval` - Minute, Hour, Day, Week, Month
- `Partition` - Time-bounded data container
- `PartitionManager` - Manages partition lifecycle
- `PartitionConfig` - Configuration for partitioning

### compression.rs
Gorilla-style compression:
- `Compressor` - Delta-of-delta timestamp and XOR value compression
- `Decompressor` - Decompression for query access
- `CompressedBlock` - Compressed data with metadata and CRC32 checksums
- Partition pruning via `last_timestamp` on compressed blocks
- Typically achieves 2-10x compression ratio

### aggregation.rs
Aggregation functions:
- `AggregateFunction` - Sum, Count, Min, Max, Avg, First, Last, Median, StdDev, Variance, Rate, Increase
- `Aggregator` - Streaming aggregation
- `Downsampler` - Reduce data resolution
- `RollingWindow` - O(n) sliding window aggregation (two-pointer algorithm)

### retention.rs
Data lifecycle management:
- `RetentionPolicy` - Define when to delete or downsample
- `RetentionTier` - Multi-resolution storage tiers
- `MultiTierRetention` - Preset configs for monitoring/IoT
- `RetentionManager` - Apply policies to partitions

### index.rs
Series indexing:
- `TimeSeriesIndex` - Index by ID, metric, tags
- `SeriesMetadata` - Indexed series information
- `LabelMatcher` - Equal, NotEqual, Regex, NotRegex filters
- Compiled regex cache for query performance

### query.rs
Query execution:
- `TimeSeriesQuery` - Query builder with filters and aggregation
- `QueryAggregation` - Downsample, RollingWindow, Instant
- `QueryResult` - Results with timing and statistics
- `QueryExecutor` - Execute queries on series data
- `InstantQuery` - Point-in-time queries

### engine.rs
Core engine:
- `TimeSeriesEngine` - Main entry point
- `EngineConfig` - Configuration options (including `data_path` for persistence)
- `SeriesBuffer` - In-memory buffer with auto-compression
- `EngineStats` - Performance metrics
- `flush()` - Persist all data to disk via PersistenceManager
- `load_from_disk()` - Restore data from persisted state on startup
- Cardinality limit enforcement (`max_series_per_metric`)

### persistence.rs
Crash-safe data persistence:
- `PersistenceManager` - Manages atomic save/load operations
- `PersistedState` - Full engine state snapshot (metrics, series, blocks)
- `PersistedSeries` - Serialized series with tags and data points
- `PersistedBlock` - Serialized compressed blocks
- Uses bincode serialization for efficient binary encoding
- Atomic writes (write to temp file, then rename) to prevent corruption

## Usage Example

```rust
use aegis_timeseries::*;
use chrono::Duration;

// Create engine
let engine = TimeSeriesEngine::new();

// Write data
let mut tags = Tags::new();
tags.insert("host", "server1");
engine.write_now("cpu_usage", tags.clone(), 45.5)?;

// Query data
let query = TimeSeriesQuery::last("cpu_usage", Duration::hours(1))
    .downsample(Duration::minutes(5), AggregateFunction::Avg)
    .with_limit(100);
let result = engine.query(&query);

// Get latest value
let latest = engine.latest("cpu_usage", Some(&tags));

// Aggregate over time range
let avg = engine.aggregate("cpu_usage", Duration::hours(1), AggregateFunction::Avg);
```

## Tests

36 tests covering all modules including persistence roundtrips.
