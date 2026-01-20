//! Aegis Memory - Memory Management System
//!
//! High-performance memory allocation and management for the Aegis database.
//! Provides arena allocators, memory pools, and resource tracking for
//! efficient memory utilization.
//!
//! Key Features:
//! - Arena-based memory allocation for query execution
//! - Memory pool management for fixed-size allocations
//! - Resource tracking and memory pressure monitoring
//! - Zero-copy buffer management
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

pub mod arena;

pub use arena::MemoryArena;
