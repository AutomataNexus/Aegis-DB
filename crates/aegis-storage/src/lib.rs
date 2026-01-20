//! Aegis Storage - Multi-Paradigm Storage Engine
//!
//! Unified storage layer supporting multiple data models (relational, time series,
//! document, streaming) with pluggable backends. Provides ACID transactions,
//! compression, encryption, and efficient indexing strategies.
//!
//! Key Features:
//! - Pluggable storage backends (local filesystem, memory, distributed)
//! - Write-ahead logging for durability and crash recovery
//! - Block-based storage with configurable compression
//! - Buffer pool for efficient page caching
//! - Multi-version concurrency control (MVCC)
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

pub mod backend;
pub mod block;
pub mod buffer;
pub mod page;
pub mod transaction;
pub mod wal;

pub use backend::StorageBackend;
pub use block::{Block, BlockHeader};
pub use buffer::BufferPool;
pub use page::Page;
pub use transaction::{TransactionManager, Transaction, IsolationLevel};
pub use wal::WriteAheadLog;
