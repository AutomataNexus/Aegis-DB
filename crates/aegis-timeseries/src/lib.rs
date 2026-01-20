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

pub mod types;
pub mod partition;
pub mod compression;
pub mod aggregation;
pub mod retention;
pub mod index;
pub mod query;
pub mod engine;

pub use types::{DataPoint, Series, Metric, MetricType, Tags};
pub use partition::{Partition, PartitionManager, PartitionConfig};
pub use compression::{Compressor, Decompressor};
pub use aggregation::{Aggregator, AggregateFunction, Downsampler};
pub use retention::{RetentionPolicy, RetentionManager};
pub use index::TimeSeriesIndex;
pub use query::{TimeSeriesQuery, QueryResult};
pub use engine::TimeSeriesEngine;
