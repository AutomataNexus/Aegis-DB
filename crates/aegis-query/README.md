# aegis-query

SQL query engine for the Aegis Database Platform.

## Overview

`aegis-query` provides a complete SQL query pipeline including parsing, semantic analysis, query planning, and execution. It supports standard SQL with extensions for time series and document queries.

## Features

- **SQL Parser** - Full SQL syntax support via `sqlparser`
- **Semantic Analyzer** - Type checking and schema validation
- **Query Planner** - Cost-based optimization with rule transformations
- **Query Executor** - Vectorized execution engine

## Architecture

```
SQL Query
    │
    ▼
┌─────────────┐
│   Parser    │  SQL text → AST
└──────┬──────┘
       │
       ▼
┌─────────────┐
│  Analyzer   │  AST → Validated AST (types, schema)
└──────┬──────┘
       │
       ▼
┌─────────────┐
│  Planner    │  Validated AST → Optimized Plan
└──────┬──────┘
       │
       ▼
┌─────────────┐
│  Executor   │  Plan → Results
└─────────────┘
```

## Modules

| Module | Description |
|--------|-------------|
| `parser` | SQL parsing and tokenization |
| `ast` | Abstract syntax tree definitions |
| `analyzer` | Semantic analysis and type checking |
| `planner` | Query planning and optimization |
| `executor` | Query execution engine |

## Usage

```toml
[dependencies]
aegis-query = { path = "../aegis-query" }
```

### Example

```rust
use aegis_query::{QueryEngine, QueryResult};

let engine = QueryEngine::new(storage)?;

// Execute a query
let result = engine.execute("SELECT * FROM users WHERE age > 21")?;

// Iterate results
for row in result.rows() {
    println!("{:?}", row);
}
```

### Supported SQL

**DDL:**
```sql
CREATE TABLE users (
    id INT PRIMARY KEY,
    name VARCHAR(100) NOT NULL,
    email VARCHAR(255) UNIQUE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

DROP TABLE users;
ALTER TABLE users ADD COLUMN status VARCHAR(20);
```

**DML:**
```sql
INSERT INTO users (name, email) VALUES ('Alice', 'alice@example.com');
UPDATE users SET status = 'active' WHERE id = 1;
DELETE FROM users WHERE status = 'inactive';
```

**Queries:**
```sql
SELECT u.name, COUNT(o.id) as order_count
FROM users u
LEFT JOIN orders o ON u.id = o.user_id
WHERE u.created_at > '2024-01-01'
GROUP BY u.name
HAVING COUNT(o.id) > 5
ORDER BY order_count DESC
LIMIT 10;
```

## Query Optimization

The planner applies several optimization rules:

- **Predicate Pushdown** - Move filters closer to data source
- **Projection Pruning** - Only read needed columns
- **Join Reordering** - Optimize join order based on cardinality
- **Index Selection** - Choose optimal indexes for predicates
- **Constant Folding** - Evaluate constant expressions at plan time

## Tests

```bash
cargo test -p aegis-query
```

**Test count:** 17 tests

## License

Apache-2.0
