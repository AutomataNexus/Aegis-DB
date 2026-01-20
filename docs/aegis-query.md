# aegis-query

SQL query engine with parsing, analysis, planning, and execution.

## Overview

The `aegis-query` crate implements a complete SQL query processing pipeline. It parses SQL statements, performs semantic analysis, generates optimized query plans, and executes them using a Volcano-style iterator model.

## Modules

### parser.rs
SQL parser using sqlparser-rs:

**Parser Struct:**
```rust
pub struct Parser {
    dialect: GenericDialect,
}

impl Parser {
    pub fn new() -> Self;
    pub fn parse(&self, sql: &str) -> Result<Vec<Statement>>;
    pub fn parse_single(&self, sql: &str) -> Result<Statement>;
}
```

**Supported Statements:**
- SELECT (with JOIN, WHERE, GROUP BY, HAVING, ORDER BY, LIMIT)
- INSERT (VALUES and SELECT)
- UPDATE
- DELETE
- CREATE TABLE
- DROP TABLE
- CREATE INDEX
- DROP INDEX
- BEGIN, COMMIT, ROLLBACK

### ast.rs
Internal AST representation:

**Statement Types:**
- `SelectStatement` - SELECT queries
- `InsertStatement` - INSERT operations
- `UpdateStatement` - UPDATE operations
- `DeleteStatement` - DELETE operations
- `CreateTableStatement` - DDL for tables
- `CreateIndexStatement` - DDL for indexes

**Expression Types:**
- Literals (Null, Boolean, Integer, Float, String)
- Column references
- Binary operations (+, -, *, /, =, <>, <, >, AND, OR, etc.)
- Unary operations (NOT, -)
- Function calls
- CASE expressions
- CAST expressions
- Subqueries
- IN, BETWEEN, LIKE, IS NULL

### analyzer.rs
Semantic analysis and type checking:

**Analyzer Struct:**
```rust
pub struct Analyzer {
    catalog: Catalog,
}

impl Analyzer {
    pub fn new(catalog: Catalog) -> Self;
    pub fn analyze(&self, stmt: &Statement) -> Result<AnalyzedStatement>;
}
```

**Features:**
- Table and column resolution
- Type inference
- Semantic validation
- Schema compatibility checking

### planner.rs
Cost-based query planner:

**Plan Nodes:**
- `ScanNode` - Table scan (sequential or index)
- `FilterNode` - Predicate filtering
- `ProjectNode` - Column projection
- `JoinNode` - Join operations
- `AggregateNode` - Aggregation
- `SortNode` - Ordering
- `LimitNode` - Row limiting

**Join Strategies:**
- NestedLoop
- HashJoin
- MergeJoin

**Cost Estimation:**
- Row count estimation
- Operation cost modeling
- Join strategy selection

### executor.rs
Volcano-style query executor:

**Operator Trait:**
```rust
pub trait Operator: Send {
    fn next_batch(&mut self) -> ExecutorResult<Option<ResultBatch>>;
    fn columns(&self) -> &[String];
}
```

**Operators:**
- `ScanOperator` - Table scanning
- `FilterOperator` - Row filtering
- `ProjectOperator` - Column projection
- `JoinOperator` - Join execution
- `AggregateOperator` - Aggregation with accumulators
- `SortOperator` - In-memory sorting
- `LimitOperator` - Row limiting with offset

**Aggregate Functions:**
- COUNT, SUM, AVG, MIN, MAX

**Scalar Functions:**
- UPPER, LOWER, LENGTH, ABS, COALESCE

## Usage

```rust
use aegis_query::{Parser, Analyzer, Planner, Executor};

let parser = Parser::new();
let stmt = parser.parse_single("SELECT * FROM users WHERE age > 18")?;

let analyzer = Analyzer::new(catalog);
let analyzed = analyzer.analyze(&stmt)?;

let planner = Planner::new(schema);
let plan = planner.plan(&analyzed)?;

let executor = Executor::new(context);
let result = executor.execute(&plan)?;
```

## Tests

17 tests covering parsing, analysis, planning, and execution.
