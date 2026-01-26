//! Aegis Query - SQL Query Engine
//!
//! Full-featured SQL query processing including parsing, optimization,
//! and execution. Supports ANSI SQL with extensions for time series,
//! document queries, and real-time streaming.
//!
//! Key Features:
//! - ANSI SQL compliant parser with extensions
//! - Cost-based query optimization
//! - Vectorized query execution
//! - Parallel query processing
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

pub mod parser;
pub mod ast;
pub mod analyzer;
pub mod planner;
pub mod executor;
pub mod index;

pub use parser::Parser;
pub use ast::*;
pub use analyzer::Analyzer;
pub use planner::{QueryPlan, Planner};
pub use executor::Executor;
pub use index::{BTreeIndex, HashIndex, IndexType, IndexKey, IndexValue, TableIndexManager, IndexError};
