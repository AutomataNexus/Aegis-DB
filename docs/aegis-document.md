# aegis-document

Document Store Engine for Aegis Database Platform.

## Overview

Flexible schema document storage with JSON support, full-text search,
and schema validation capabilities.

## Modules

### types.rs
Core document types:
- `DocumentId` - Unique document identifier (auto-generated or custom)
- `Value` - JSON-compatible value (Null, Bool, Int, Float, String, Array, Object)
- `Document` - Document with ID, data, and metadata
- Path-based field access (e.g., "user.address.city")

### collection.rs
Collection management:
- `Collection` - Document container with indexing
- CRUD operations (insert, get, update, delete)
- Query operations (find, find_one, count_matching)
- Index management (create_index, drop_index)
- Schema validation support

### index.rs
Document indexing:
- `IndexType` - Hash, BTree, FullText, Unique
- `DocumentIndex` - Field-based indexing
- `InvertedIndex` - Full-text search with tokenization
- `IndexKey` - Hashable key for lookups

### query.rs
Query language:
- `Query` - Filter, sort, skip, limit, projection
- `Filter` - Comparison operators:
  - Eq, Ne, Gt, Gte, Lt, Lte
  - In, Nin (not in)
  - Exists
  - Regex, Contains, StartsWith, EndsWith
  - And, Or, Not (logical)
- `QueryBuilder` - Fluent query construction
- `QueryResult` - Results with execution statistics

### validation.rs
Schema validation:
- `Schema` - Document structure definition
- `FieldSchema` - Field type and constraints
- `FieldType` - String, Int, Float, Number, Bool, Array, Object, Any
- Constraints: nullable, min/max, min_length/max_length, pattern, enum_values
- Array item validation
- Object property validation

### engine.rs
Core engine:
- `DocumentEngine` - Main entry point
- Collection management (create, drop, list)
- Document CRUD with validation
- Query execution
- Index management
- Statistics tracking

## Usage Example

```rust
use aegis_document::*;

// Create engine
let engine = DocumentEngine::new();

// Create collection
engine.create_collection("users")?;

// Insert document
let mut doc = Document::new();
doc.set("name", "Alice");
doc.set("age", 30i64);
doc.set("email", "alice@example.com");
let id = engine.insert("users", doc)?;

// Query documents
let query = QueryBuilder::new()
    .eq("name", "Alice")
    .gt("age", 25i64)
    .build();
let result = engine.find("users", &query)?;

// Create full-text index
engine.create_index("users", "email", IndexType::FullText)?;

// Update document
let mut updated = Document::with_id(id.clone());
updated.set("name", "Alice Smith");
updated.set("age", 31i64);
engine.update("users", &id, updated)?;

// Delete document
engine.delete("users", &id)?;
```

## Schema Validation Example

```rust
use aegis_document::*;

// Define schema
let schema = SchemaBuilder::new("User")
    .required_field("name", FieldSchema::string().min_length(1))
    .required_field("age", FieldSchema::int().min(0.0).max(150.0))
    .field("email", FieldSchema::string().pattern(r"^[^@]+@[^@]+$"))
    .additional_properties(false)
    .build();

// Create collection with schema
engine.create_collection_with_schema("users", schema)?;

// Validation happens automatically on insert/update
```

## Tests

36 tests covering all modules.
