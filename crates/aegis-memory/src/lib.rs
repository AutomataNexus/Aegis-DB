//! Aegis Memory - Memory Management System
//!
//! High-performance memory allocation for the Aegis database.
//! Provides arena allocators for efficient, bump-pointer memory allocation
//! during query execution and temporary data processing.
//!
//! Key Features:
//! - Arena-based bump allocation with configurable chunk sizes
//! - Thread-safe via interior mutability (Mutex-protected)
//! - Typed allocation (values and slices) with proper alignment
//! - Memory usage tracking (total allocated and used bytes)
//! - Overflow-safe size calculations
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

pub mod arena;

pub use arena::MemoryArena;
