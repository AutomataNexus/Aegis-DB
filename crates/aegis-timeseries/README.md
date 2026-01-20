<p align="center">
  <img src="../../AegisDB-logo.png" alt="AegisDB Logo" width="300">
</p>

# aegis-timeseries

<p align="center">
  <a href="https://opensource.org/licenses/Apache-2.0"><img src="https://img.shields.io/badge/License-Apache%202.0-blue.svg" alt="License"></a>
  <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/Rust-1.75%2B-orange.svg" alt="Rust"></a>
  <img src="https://img.shields.io/badge/crate-0.1.0-green.svg" alt="Version">
  <a href="../../README.md"><img src="https://img.shields.io/badge/part%20of-AegisDB-teal.svg" alt="AegisDB"></a>
</p>

Time series engine for the Aegis Database Platform.

## Overview

`aegis-timeseries` provides specialized storage and query capabilities for time-stamped data. It features Gorilla compression, time-based partitioning, automatic retention policies, and efficient aggregation functions.

## Features

- **Gorilla Compression** - Facebook's algorithm for floating-point time series
- **Time Partitioning** - Automatic partition management by time range
- **Retention Policies** - Automatic data expiration
- **Downsampling** - Configurable data rollup
- **Aggregations** - Sum, mean, min, max, percentiles, and more

## Architecture

```
┌─────────────────────────────────────────────────┐
│              Time Series Engine                  │
├─────────────────────────────────────────────────┤
│                Query Processor                   │
│  ┌──────────┬──────────────┬─────────────────┐  │
│  │  Parser  │  Aggregator  │  Downsampler    │  │
│  └──────────┴──────────────┴─────────────────┘  │
├─────────────────────────────────────────────────┤
│              Partition Manager                   │
│  ┌─────────┬─────────┬─────────┬─────────────┐  │
│  │ Active  │ Recent  │  Cold   │  Archive    │  │
│  │Partition│Partitions│Partitions│            │  │
│  └─────────┴─────────┴─────────┴─────────────┘  │
├─────────────────────────────────────────────────┤
│              Compression Layer                   │
│         (Gorilla / Delta / Dictionary)          │
└─────────────────────────────────────────────────┘
```

## Modules

| Module | Description |
|--------|-------------|
| `engine` | Main time series engine |
| `compression` | Gorilla and delta compression |
| `partition` | Time-based partition management |
| `aggregation` | Aggregation functions |
| `retention` | Retention policy enforcement |
| `query` | Time series query execution |
| `index` | Time-based indexing |
| `types` | Time series data types |

## Usage

```toml
[dependencies]
aegis-timeseries = { path = "../aegis-timeseries" }
```

### Writing Data

```rust
use aegis_timeseries::{TimeSeriesEngine, DataPoint};
use chrono::Utc;

let engine = TimeSeriesEngine::new(config)?;

// Write a single point
engine.write(DataPoint {
    metric: "cpu.usage",
    timestamp: Utc::now(),
    value: 75.5,
    tags: vec![("host", "server-1"), ("dc", "us-east")],
})?;

// Batch write
let points = vec![
    DataPoint::new("memory.used", Utc::now(), 8_000_000_000.0),
    DataPoint::new("memory.free", Utc::now(), 4_000_000_000.0),
];
engine.write_batch(points)?;
```

### Querying Data

```rust
use aegis_timeseries::query::{Query, Aggregation, TimeRange};

// Simple query
let results = engine.query(Query {
    metric: "cpu.usage",
    time_range: TimeRange::last_hours(24),
    tags: vec![("host", "server-1")],
    aggregation: None,
})?;

// With aggregation
let results = engine.query(Query {
    metric: "cpu.usage",
    time_range: TimeRange::last_days(7),
    tags: vec![],
    aggregation: Some(Aggregation {
        function: AggFunc::Mean,
        interval: Duration::from_secs(3600), // 1 hour buckets
        group_by: vec!["host"],
    }),
})?;
```

### Aggregation Functions

```rust
use aegis_timeseries::aggregation::AggFunc;

// Available aggregations
AggFunc::Sum       // Sum of values
AggFunc::Mean      // Average
AggFunc::Min       // Minimum
AggFunc::Max       // Maximum
AggFunc::Count     // Count of points
AggFunc::First     // First value in window
AggFunc::Last      // Last value in window
AggFunc::StdDev    // Standard deviation
AggFunc::Percentile(95.0)  // 95th percentile
```

### Retention Policies

```rust
use aegis_timeseries::retention::{RetentionPolicy, RetentionAction};

// Keep raw data for 7 days, then downsample
let policy = RetentionPolicy {
    name: "default",
    duration: Duration::from_days(7),
    action: RetentionAction::Downsample {
        target_interval: Duration::from_hours(1),
        aggregation: AggFunc::Mean,
    },
};

engine.set_retention_policy("cpu.*", policy)?;

// Delete data older than 30 days
let delete_policy = RetentionPolicy {
    name: "delete-old",
    duration: Duration::from_days(30),
    action: RetentionAction::Delete,
};
```

### Downsampling

```rust
use aegis_timeseries::downsampling::{DownsampleConfig, DownsampleRule};

let config = DownsampleConfig {
    rules: vec![
        DownsampleRule {
            source_interval: Duration::from_secs(10),
            target_interval: Duration::from_mins(1),
            after: Duration::from_hours(24),
            aggregations: vec![AggFunc::Mean, AggFunc::Max],
        },
        DownsampleRule {
            source_interval: Duration::from_mins(1),
            target_interval: Duration::from_hours(1),
            after: Duration::from_days(7),
            aggregations: vec![AggFunc::Mean],
        },
    ],
};

engine.configure_downsampling(config)?;
```

## Compression

Gorilla compression achieves ~12x compression ratio for typical time series data:

| Data Type | Compression Ratio |
|-----------|------------------|
| Timestamps | 1.37 bits/point |
| Float values | 0.92 bits/point (for similar values) |
| Overall | ~12x typical |

## Configuration

```toml
[timeseries]
partition_duration = "1d"      # Partition size
compression = "gorilla"        # gorilla, delta, none
write_buffer_size = "64MB"

[timeseries.retention]
default_duration = "30d"
enforce_interval = "1h"

[timeseries.downsampling]
enabled = true
schedule = "0 * * * *"         # Hourly
```

## Tests

```bash
cargo test -p aegis-timeseries
```

**Test count:** 31 tests

## License

Apache-2.0
