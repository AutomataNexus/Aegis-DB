<p align="center">
  <img src="https://raw.githubusercontent.com/AutomataNexus/assets/main/AegisDB-logo.png" alt="AegisDB Logo" width="300">
</p>

# aegis-document

<p align="center">
  <a href="https://opensource.org/licenses/Apache-2.0"><img src="https://img.shields.io/badge/License-Apache%202.0-blue.svg" alt="License"></a>
  <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/Rust-1.75%2B-orange.svg" alt="Rust"></a>
  <img src="https://img.shields.io/badge/crate-0.1.0-green.svg" alt="Version">
  <a href="../../README.md"><img src="https://img.shields.io/badge/part%20of-AegisDB-teal.svg" alt="AegisDB"></a>
</p>

Document store engine for the Aegis Database Platform.

## Overview

`aegis-document` provides a flexible document storage system with schema-optional JSON storage, full-text search, JSONPath queries, and schema validation. It's designed for semi-structured data that doesn't fit traditional relational models.

## Features

- **Schema-Optional** - Store any JSON document
- **Collections** - Logical grouping of documents
- **Full-Text Search** - Tokenized text search with ranking
- **JSONPath Queries** - Query nested document fields
- **Schema Validation** - Optional JSON Schema enforcement
- **Secondary Indexes** - Index any field for fast lookups

## Architecture

```
┌─────────────────────────────────────────────────┐
│              Document Engine                     │
├─────────────────────────────────────────────────┤
│               Query Processor                    │
│  ┌──────────┬──────────────┬─────────────────┐  │
│  │ JSONPath │  Full-Text   │  Aggregation    │  │
│  │  Parser  │   Search     │    Engine       │  │
│  └──────────┴──────────────┴─────────────────┘  │
├─────────────────────────────────────────────────┤
│                Index Manager                     │
│  ┌──────────┬──────────────┬─────────────────┐  │
│  │  B-Tree  │   Inverted   │   Composite     │  │
│  │  Index   │    Index     │    Index        │  │
│  └──────────┴──────────────┴─────────────────┘  │
├─────────────────────────────────────────────────┤
│             Collection Manager                   │
│          (Schema Validation Layer)               │
└─────────────────────────────────────────────────┘
```

## Modules

| Module | Description |
|--------|-------------|
| `engine` | Main document engine |
| `collection` | Collection management |
| `query` | Document query execution |
| `index` | Secondary index management |
| `validation` | JSON Schema validation |
| `types` | Document type definitions |

## Usage

```toml
[dependencies]
aegis-document = { path = "../aegis-document" }
```

### Collections

```rust
use aegis_document::{DocumentEngine, Collection, CollectionOptions};

let engine = DocumentEngine::new(storage)?;

// Create a collection
engine.create_collection("users", CollectionOptions {
    schema: None,  // Schema-optional
    indexes: vec![
        Index::new("email").unique(true),
        Index::new("name"),
    ],
})?;

// Create with schema validation
let schema = json!({
    "type": "object",
    "required": ["name", "email"],
    "properties": {
        "name": { "type": "string" },
        "email": { "type": "string", "format": "email" },
        "age": { "type": "integer", "minimum": 0 }
    }
});

engine.create_collection("validated_users", CollectionOptions {
    schema: Some(schema),
    indexes: vec![],
})?;
```

### CRUD Operations

```rust
// Insert
let doc_id = engine.insert("users", json!({
    "name": "Alice",
    "email": "alice@example.com",
    "tags": ["admin", "developer"],
    "profile": {
        "bio": "Software engineer",
        "location": "San Francisco"
    }
}))?;

// Find by ID
let doc = engine.find_by_id("users", &doc_id)?;

// Update
engine.update("users", &doc_id, json!({
    "$set": { "profile.location": "New York" },
    "$push": { "tags": "speaker" }
}))?;

// Delete
engine.delete("users", &doc_id)?;
```

### Querying

```rust
use aegis_document::query::{Query, Filter, Sort};

// Simple filter
let results = engine.find("users", Query {
    filter: Filter::eq("email", "alice@example.com"),
    ..Default::default()
})?;

// Complex query
let results = engine.find("users", Query {
    filter: Filter::and(vec![
        Filter::gt("age", 21),
        Filter::in_array("tags", vec!["developer", "admin"]),
        Filter::regex("email", r".*@company\.com$"),
    ]),
    sort: Sort::desc("created_at"),
    skip: 0,
    limit: 100,
    projection: vec!["name", "email"],
})?;

// Nested field query (JSONPath)
let results = engine.find("users", Query {
    filter: Filter::eq("profile.location", "San Francisco"),
    ..Default::default()
})?;
```

### Full-Text Search

```rust
use aegis_document::search::{TextSearch, SearchOptions};

// Create text index
engine.create_index("articles", Index::text(vec!["title", "content"]))?;

// Search
let results = engine.search("articles", TextSearch {
    query: "rust database performance",
    options: SearchOptions {
        fuzzy: true,
        highlight: true,
        ..Default::default()
    },
})?;

for result in results {
    println!("Score: {}, Title: {}", result.score, result.doc["title"]);
    println!("Highlights: {:?}", result.highlights);
}
```

### Aggregation

```rust
use aegis_document::aggregation::{Pipeline, Stage};

let results = engine.aggregate("orders", Pipeline::new()
    .match_stage(Filter::gte("created_at", "2024-01-01"))
    .group(json!({
        "_id": "$customer_id",
        "total": { "$sum": "$amount" },
        "count": { "$count": {} }
    }))
    .sort(Sort::desc("total"))
    .limit(10)
)?;
```

### Update Operators

| Operator | Description | Example |
|----------|-------------|---------|
| `$set` | Set field value | `{"$set": {"name": "Bob"}}` |
| `$unset` | Remove field | `{"$unset": {"temp": ""}}` |
| `$inc` | Increment number | `{"$inc": {"count": 1}}` |
| `$push` | Add to array | `{"$push": {"tags": "new"}}` |
| `$pull` | Remove from array | `{"$pull": {"tags": "old"}}` |
| `$addToSet` | Add unique to array | `{"$addToSet": {"tags": "x"}}` |

## Configuration

```toml
[document]
default_collection_options = { max_documents = 1000000 }
text_search_language = "english"

[document.indexes]
auto_create = true
background_build = true
```

## Tests

```bash
cargo test -p aegis-document
```

**Test count:** 36 tests

## License

Apache-2.0
