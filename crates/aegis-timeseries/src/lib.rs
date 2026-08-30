//! Aegis Time Series - Time Series Engine
//!
//! Specialized storage and query engine for time series data. Provides
//! high-throughput ingestion, efficient compression, and time-based queries.
//!
//! Key Features:
//! - High-frequency data ingestion (>1M points/second)
//! - Delta-of-delta timestamp compression
//! - XOR-based floating point compression
//! - Automatic downsampling and retention policies
//! - Time-based partitioning and indexing
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

pub mod aggregation;
pub mod cold;
pub mod compression;
pub mod engine;
pub mod index;
pub mod partition;
pub mod persistence;
pub mod query;
pub mod retention;
pub mod types;

pub use aggregation::{AggregateFunction, Aggregator, Downsampler};
pub use cold::{ColdCompactReport, ColdStore};
pub use compression::{Compressor, Decompressor};
pub use engine::TimeSeriesEngine;
pub use index::TimeSeriesIndex;
pub use partition::{Partition, PartitionConfig, PartitionManager};
pub use persistence::PersistenceManager;
pub use query::{QueryResult, TimeSeriesQuery};
pub use retention::{RetentionManager, RetentionPolicy};
pub use types::{DataPoint, Metric, MetricType, Series, Tags};
