//! Aegis Document - Document Store Engine
//!
//! Flexible schema document storage with JSON/BSON support. Provides
//! nested queries, full-text search, and schema validation.
//!
//! Key Features:
//! - Schema-flexible JSON/BSON storage
//! - JSONPath query support
//! - Full-text search with inverted indexes
//! - Nested object indexing
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

pub mod types;
pub mod collection;
pub mod index;
pub mod query;
pub mod validation;
pub mod engine;

pub use types::{Document, DocumentId, Value};
pub use collection::Collection;
pub use index::{DocumentIndex, IndexType};
pub use query::{Query, QueryResult};
pub use validation::{Schema, ValidationResult};
pub use engine::DocumentEngine;
