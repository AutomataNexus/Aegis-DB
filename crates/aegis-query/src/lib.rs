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

pub mod analyzer;
pub mod ast;
pub mod executor;
pub mod index;
pub mod parser;
pub mod planner;

pub use analyzer::Analyzer;
pub use ast::*;
pub use executor::{Executor, TransferOutcome};
pub use index::{
    BTreeIndex, HashIndex, IndexError, IndexKey, IndexType, IndexValue, TableIndexManager,
};
pub use parser::Parser;
pub use planner::{Planner, QueryPlan};
