//! Aegis Common - Shared Types and Utilities
//!
//! Foundational types, error handling, and utilities used across all Aegis
//! database platform components. Provides the core abstractions that enable
//! consistent behavior across storage, query, and replication layers.
//!
//! Key Features:
//! - Unified error types with retryable error detection
//! - Core data types (BlockId, PageId, TransactionId, etc.)
//! - Configuration structures for all components
//! - Utility functions for hashing, serialization, and validation
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

pub mod config;
pub mod error;
pub mod types;
pub mod utils;

pub use error::{AegisError, Result};
pub use types::*;
