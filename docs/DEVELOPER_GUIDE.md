<p align="center">
  <img src="../AegisDB-logo.png" alt="AegisDB Logo" width="400">
</p>

<h1 align="center">AegisDB Developer Guide</h1>

<p align="center">
  <strong>Comprehensive guide for developing, contributing to, and extending AegisDB</strong>
</p>

---

## Table of Contents

1. [Introduction](#introduction)
2. [Architecture Overview](#architecture-overview)
   - [System Architecture](#system-architecture)
   - [Data Flow](#data-flow)
   - [Component Interaction](#component-interaction)
3. [Project Structure](#project-structure)
4. [Development Environment Setup](#development-environment-setup)
   - [Prerequisites](#prerequisites)
   - [IDE Setup](#ide-setup)
   - [Building from Source](#building-from-source)
5. [Crate Deep Dives](#crate-deep-dives)
   - [aegis-common](#aegis-common)
   - [aegis-storage](#aegis-storage)
   - [aegis-memory](#aegis-memory)
   - [aegis-query](#aegis-query)
   - [aegis-server](#aegis-server)
   - [aegis-client](#aegis-client)
   - [aegis-replication](#aegis-replication)
   - [aegis-timeseries](#aegis-timeseries)
   - [aegis-document](#aegis-document)
   - [aegis-streaming](#aegis-streaming)
   - [aegis-monitoring](#aegis-monitoring)
   - [aegis-dashboard](#aegis-dashboard)
6. [Code Style & Conventions](#code-style--conventions)
7. [Testing](#testing)
   - [Unit Tests](#unit-tests)
   - [Integration Tests](#integration-tests)
   - [End-to-End Tests](#end-to-end-tests)
   - [Benchmarks](#benchmarks)
8. [API Development](#api-development)
   - [Adding New Endpoints](#adding-new-endpoints)
   - [Request/Response Types](#requestresponse-types)
   - [Error Handling](#error-handling)
9. [Storage Engine Development](#storage-engine-development)
   - [Adding a New Backend](#adding-a-new-backend)
   - [Transaction Implementation](#transaction-implementation)
   - [Write-Ahead Log](#write-ahead-log)
10. [Query Engine Development](#query-engine-development)
    - [Parser Extensions](#parser-extensions)
    - [Adding Operators](#adding-operators)
    - [Query Optimization](#query-optimization)
11. [Distributed Systems Development](#distributed-systems-development)
    - [Raft Consensus](#raft-consensus)
    - [Sharding](#sharding)
    - [CRDTs](#crdts)
12. [Dashboard Development](#dashboard-development)
    - [Leptos/WASM](#leptoswasm)
    - [Component Development](#component-development)
    - [State Management](#state-management)
13. [Performance Optimization](#performance-optimization)
14. [Debugging](#debugging)
15. [Release Process](#release-process)
16. [Contributing](#contributing)

---

## Introduction

This guide is for developers who want to:
- Contribute to AegisDB core development
- Understand the internal architecture
- Build extensions or plugins
- Debug and troubleshoot issues
- Optimize performance

AegisDB is written in Rust, leveraging its safety guarantees and performance characteristics. The codebase follows modern Rust idioms and best practices.

### Technology Stack

| Component | Technology |
|-----------|------------|
| Language | Rust 1.75+ |
| Async Runtime | Tokio |
| HTTP Framework | Axum |
| Serialization | Serde |
| SQL Parser | sqlparser-rs |
| Dashboard | Leptos (WASM) |
| Build Tool | Cargo |
| WASM Build | Trunk |

---

## Architecture Overview

### System Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                           Client Layer                               │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌────────────┐  │
│  │  REST API   │  │    CLI      │  │  Dashboard  │  │    SDKs    │  │
│  │   Clients   │  │   (aegis)   │  │   (WASM)    │  │ (Py, JS)   │  │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘  └─────┬──────┘  │
└─────────┼────────────────┼────────────────┼───────────────┼─────────┘
          │                │                │               │
          └────────────────┼────────────────┼───────────────┘
                           │                │
┌──────────────────────────┼────────────────┼─────────────────────────┐
│                          ▼                ▼                         │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                     aegis-server (Axum)                      │   │
│  │  ┌─────────────────────────────────────────────────────────┐ │   │
│  │  │                    Middleware Layer                      │ │   │
│  │  │  Auth │ CORS │ Rate Limiting │ Tracing │ Request ID     │ │   │
│  │  └─────────────────────────────────────────────────────────┘ │   │
│  │  ┌─────────────────────────────────────────────────────────┐ │   │
│  │  │                     Route Handlers                       │ │   │
│  │  │  Query │ KV │ Documents │ TimeSeries │ Streaming │ Admin │ │   │
│  │  └─────────────────────────────────────────────────────────┘ │   │
│  │  ┌─────────────────────────────────────────────────────────┐ │   │
│  │  │                    Application State                     │ │   │
│  │  │    Config │ Auth │ Engines │ Activity │ Metrics         │ │   │
│  │  └─────────────────────────────────────────────────────────┘ │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                              API Layer                              │
└─────────────────────────────────────────────────────────────────────┘
                                   │
┌──────────────────────────────────┼──────────────────────────────────┐
│                                  ▼                                  │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │                       Engine Layer                             │  │
│  │                                                                │  │
│  │  ┌────────────┐ ┌────────────┐ ┌────────────┐ ┌────────────┐  │  │
│  │  │  aegis-    │ │  aegis-    │ │  aegis-    │ │  aegis-    │  │  │
│  │  │  query     │ │ timeseries │ │  document  │ │ streaming  │  │  │
│  │  │            │ │            │ │            │ │            │  │  │
│  │  │ ┌────────┐ │ │ ┌────────┐ │ │ ┌────────┐ │ │ ┌────────┐ │  │  │
│  │  │ │ Parser │ │ │ │Compress│ │ │ │ Query  │ │ │ │Channel │ │  │  │
│  │  │ │Analyzer│ │ │ │Partition│ │ │ │ Index  │ │ │ │  CDC   │ │  │  │
│  │  │ │Planner │ │ │ │Aggreg. │ │ │ │ Full-  │ │ │ │ Event  │ │  │  │
│  │  │ │Executor│ │ │ │Retention│ │ │ │ Text   │ │ │ │ Store  │ │  │  │
│  │  │ └────────┘ │ │ └────────┘ │ │ └────────┘ │ │ └────────┘ │  │  │
│  │  └────────────┘ └────────────┘ └────────────┘ └────────────┘  │  │
│  └───────────────────────────────────────────────────────────────┘  │
│                            Engine Layer                             │
└─────────────────────────────────────────────────────────────────────┘
                                   │
┌──────────────────────────────────┼──────────────────────────────────┐
│                                  ▼                                  │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │                     Distributed Layer                          │  │
│  │                     (aegis-replication)                        │  │
│  │                                                                │  │
│  │  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐              │  │
│  │  │    Raft     │ │   Shard     │ │ Transaction │              │  │
│  │  │  Consensus  │ │   Router    │ │ Coordinator │              │  │
│  │  └─────────────┘ └─────────────┘ └─────────────┘              │  │
│  │  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐              │  │
│  │  │   Hash      │ │   Vector    │ │    CRDT     │              │  │
│  │  │   Ring      │ │   Clocks    │ │   Engine    │              │  │
│  │  └─────────────┘ └─────────────┘ └─────────────┘              │  │
│  └───────────────────────────────────────────────────────────────┘  │
│                          Distributed Layer                          │
└─────────────────────────────────────────────────────────────────────┘
                                   │
┌──────────────────────────────────┼──────────────────────────────────┐
│                                  ▼                                  │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │                       Storage Layer                            │  │
│  │                       (aegis-storage)                          │  │
│  │                                                                │  │
│  │  ┌─────────────────────────────────────────────────────────┐  │  │
│  │  │               Transaction Manager (MVCC)                 │  │  │
│  │  └─────────────────────────────────────────────────────────┘  │  │
│  │  ┌─────────────────────────────────────────────────────────┐  │  │
│  │  │               Buffer Pool (LRU Cache)                    │  │  │
│  │  └─────────────────────────────────────────────────────────┘  │  │
│  │  ┌─────────────────────────────────────────────────────────┐  │  │
│  │  │               Write-Ahead Log (WAL)                      │  │  │
│  │  └─────────────────────────────────────────────────────────┘  │  │
│  │  ┌─────────────────┐           ┌─────────────────┐          │  │
│  │  │  Memory Backend │           │  Local FS Backend│          │  │
│  │  └─────────────────┘           └─────────────────┘          │  │
│  └───────────────────────────────────────────────────────────────┘  │
│                            Storage Layer                            │
└─────────────────────────────────────────────────────────────────────┘
```

### Data Flow

**Query Execution Flow:**
```
1. Client Request (HTTP)
         │
         ▼
2. Axum Router → Handler
         │
         ▼
3. SQL Parser (sqlparser-rs)
         │
         ▼
4. Semantic Analyzer (type checking, schema validation)
         │
         ▼
5. Query Planner (cost-based optimization)
         │
         ▼
6. Query Executor (vectorized execution)
         │
         ▼
7. Storage Engine (data access)
         │
         ▼
8. Response Serialization
         │
         ▼
9. HTTP Response
```

### Component Interaction

```rust
// Simplified request flow in aegis-server

// 1. Router receives request
pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/api/v1/query", post(handlers::execute_query))
        .with_state(state)
}

// 2. Handler processes request
pub async fn execute_query(
    State(state): State<AppState>,
    Json(request): Json<QueryRequest>,
) -> impl IntoResponse {
    // 3. Execute via state
    let result = state.execute_query(&request.sql).await;

    // 4. Return response
    Json(QueryResponse { data: result })
}

// 3. AppState orchestrates engines
impl AppState {
    pub async fn execute_query(&self, sql: &str) -> Result<QueryResult> {
        // Parse → Analyze → Plan → Execute
        let ast = self.parser.parse(sql)?;
        let plan = self.planner.plan(ast)?;
        let result = self.executor.execute(plan)?;
        Ok(result)
    }
}
```

---

## Project Structure

```
aegis-db/
├── Cargo.toml                 # Workspace manifest
├── Cargo.lock                 # Dependency lock file
├── README.md                  # Project overview
├── LICENSE.md                 # Apache 2.0 license
├── install.sh                 # Installation script
├── AegisDB-logo.png          # Project logo
│
├── crates/                    # Rust crates (libraries/binaries)
│   ├── aegis-common/         # Shared types and utilities
│   │   ├── Cargo.toml
│   │   ├── README.md
│   │   └── src/
│   │       ├── lib.rs        # Crate root
│   │       ├── config.rs     # Configuration types
│   │       ├── error.rs      # Error definitions
│   │       ├── types.rs      # Common types
│   │       └── utils.rs      # Utility functions
│   │
│   ├── aegis-storage/        # Storage engine
│   │   ├── Cargo.toml
│   │   ├── README.md
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── backend/      # Storage backends
│   │       │   ├── mod.rs
│   │       │   ├── memory.rs # In-memory storage
│   │       │   └── local.rs  # Filesystem storage
│   │       ├── block.rs      # Block structures
│   │       ├── buffer.rs     # Buffer pool
│   │       ├── page.rs       # Page management
│   │       ├── transaction.rs # MVCC transactions
│   │       └── wal.rs        # Write-ahead log
│   │
│   ├── aegis-memory/         # Memory management
│   │   ├── Cargo.toml
│   │   ├── README.md
│   │   └── src/
│   │       ├── lib.rs
│   │       └── arena.rs      # Arena allocator
│   │
│   ├── aegis-query/          # SQL query engine
│   │   ├── Cargo.toml
│   │   ├── README.md
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── parser.rs     # SQL parsing
│   │       ├── ast.rs        # Abstract syntax tree
│   │       ├── analyzer.rs   # Semantic analysis
│   │       ├── planner.rs    # Query planning
│   │       └── executor.rs   # Query execution
│   │
│   ├── aegis-server/         # HTTP API server
│   │   ├── Cargo.toml
│   │   ├── README.md
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── main.rs       # Server entry point
│   │       ├── router.rs     # Route definitions
│   │       ├── handlers.rs   # Request handlers
│   │       ├── state.rs      # Application state
│   │       ├── config.rs     # Server config
│   │       ├── auth.rs       # Authentication
│   │       ├── middleware.rs # HTTP middleware
│   │       ├── admin.rs      # Admin types
│   │       └── activity.rs   # Activity logging
│   │   └── tests/
│   │       └── integration_test.rs  # E2E tests
│   │
│   ├── aegis-client/         # Client SDK
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── connection.rs
│   │       ├── pool.rs
│   │       ├── query.rs
│   │       └── transaction.rs
│   │
│   ├── aegis-cli/            # Command-line interface
│   │   └── src/
│   │       └── main.rs
│   │
│   ├── aegis-replication/    # Distributed systems
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── raft/         # Raft consensus
│   │       ├── cluster.rs    # Cluster management
│   │       ├── shard.rs      # Sharding
│   │       ├── hash.rs       # Consistent hashing
│   │       ├── transaction.rs # Distributed transactions
│   │       ├── crdt.rs       # CRDTs
│   │       └── vector_clock.rs
│   │
│   ├── aegis-timeseries/     # Time series engine
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── engine.rs
│   │       ├── compression.rs # Gorilla compression
│   │       ├── partition.rs
│   │       ├── aggregation.rs
│   │       └── query.rs
│   │
│   ├── aegis-document/       # Document store
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── engine.rs
│   │       ├── collection.rs
│   │       ├── query.rs
│   │       ├── index.rs
│   │       └── validation.rs
│   │
│   ├── aegis-streaming/      # Real-time streaming
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── engine.rs
│   │       ├── channel.rs
│   │       ├── stream.rs
│   │       ├── subscriber.rs
│   │       └── event.rs
│   │
│   ├── aegis-monitoring/     # Observability
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── metrics.rs
│   │       ├── tracing.rs
│   │       └── health.rs
│   │
│   └── aegis-dashboard/      # Web UI (Leptos/WASM)
│       ├── Cargo.toml
│       ├── Trunk.toml
│       ├── index.html
│       ├── assets/
│       │   └── styles.css
│       └── src/
│           ├── lib.rs
│           ├── api.rs
│           ├── types.rs
│           ├── state.rs
│           └── pages/
│               └── dashboard.rs
│
├── docs/                      # Documentation
│   ├── AegisQL.md            # Query language reference
│   ├── USER_GUIDE.md         # End user guide
│   ├── DEVELOPER_GUIDE.md    # This file
│   └── *.md                  # Crate-specific docs
│
├── deploy/                    # Deployment configurations
│   └── helm/
│       └── aegis-db/         # Kubernetes Helm charts
│
├── integrations/              # Third-party integrations
│   └── grafana-datasource/   # Grafana plugin
│
├── sdks/                      # Language SDKs
│   ├── python/               # Python SDK
│   └── javascript/           # JavaScript/TypeScript SDK
│
└── scripts/                   # Development scripts
    ├── test.sh
    ├── bench.sh
    └── release.sh
```

---

## Development Environment Setup

### Prerequisites

1. **Install Rust:**
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# Verify installation
rustc --version  # Should be 1.75.0 or higher
cargo --version
```

2. **Install Additional Targets (for WASM):**
```bash
rustup target add wasm32-unknown-unknown
```

3. **Install Development Tools:**
```bash
# Trunk for WASM builds
cargo install trunk

# Code formatting
rustup component add rustfmt

# Linting
rustup component add clippy

# Code coverage (optional)
cargo install cargo-tarpaulin

# Dependency audit (optional)
cargo install cargo-audit

# Watch mode for development (optional)
cargo install cargo-watch

# Benchmarking (optional)
cargo install cargo-criterion
```

### IDE Setup

**VS Code (Recommended):**

Install extensions:
- `rust-analyzer` - Language server
- `Even Better TOML` - Cargo.toml support
- `crates` - Dependency version info
- `Error Lens` - Inline error display

`.vscode/settings.json`:
```json
{
    "rust-analyzer.cargo.features": "all",
    "rust-analyzer.checkOnSave.command": "clippy",
    "rust-analyzer.inlayHints.parameterHints.enable": true,
    "rust-analyzer.inlayHints.typeHints.enable": true,
    "editor.formatOnSave": true,
    "[rust]": {
        "editor.defaultFormatter": "rust-lang.rust-analyzer"
    }
}
```

**JetBrains RustRover / IntelliJ + Rust Plugin:**
- Enable all inspections
- Configure Clippy as external linter
- Set up file watchers for formatting

### Building from Source

```bash
# Clone repository
git clone https://github.com/automatanexus/aegis-db.git
cd aegis-db

# Build all crates (debug)
cargo build --workspace

# Build all crates (release)
cargo build --workspace --release

# Build specific crate
cargo build -p aegis-server

# Build with all features
cargo build --workspace --all-features

# Build dashboard (WASM)
cd crates/aegis-dashboard
trunk build
trunk build --release  # Production build
cd ../..

# Run server
cargo run -p aegis-server

# Run with arguments
cargo run -p aegis-server -- --host 0.0.0.0 --port 9090
```

### Development Workflow

```bash
# Watch mode - rebuild on changes
cargo watch -x check -x test -x run -p aegis-server

# Run specific tests
cargo test -p aegis-server test_health_endpoint

# Run tests with output
cargo test -- --nocapture

# Format code
cargo fmt --all

# Run linter
cargo clippy --workspace --all-targets --all-features

# Check for dependency vulnerabilities
cargo audit

# Update dependencies
cargo update
```

---

## Crate Deep Dives

### aegis-common

Shared types and utilities used across all crates.

**Key Types:**
```rust
// Error handling
pub type AegisResult<T> = Result<T, AegisError>;

#[derive(Debug, thiserror::Error)]
pub enum AegisError {
    #[error("Storage error: {0}")]
    Storage(String),
    #[error("Query error: {0}")]
    Query(String),
    #[error("Authentication error: {0}")]
    Auth(String),
    // ...
}

// Common identifiers
pub struct NodeId(pub u64);
pub struct BlockId(pub u64);
pub struct TransactionId(pub u64);
```

**Utilities:**
```rust
// ID generation
pub fn generate_id() -> String;

// Hashing
pub fn hash_bytes(data: &[u8]) -> u64;

// Configuration
pub struct AegisConfig {
    pub server: ServerConfig,
    pub storage: StorageConfig,
    pub auth: AuthConfig,
}
```

### aegis-storage

Core storage engine with pluggable backends.

**Backend Trait:**
```rust
pub trait StorageBackend: Send + Sync {
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>>;
    fn put(&self, key: &[u8], value: &[u8]) -> Result<()>;
    fn delete(&self, key: &[u8]) -> Result<()>;
    fn scan(&self, start: &[u8], end: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>>;
}
```

**Memory Backend:**
```rust
pub struct MemoryBackend {
    data: RwLock<BTreeMap<Vec<u8>, Vec<u8>>>,
}
```

**Transaction Manager (MVCC):**
```rust
pub struct TransactionManager {
    active_transactions: RwLock<HashMap<TransactionId, Transaction>>,
    version_store: RwLock<BTreeMap<(Key, Version), Value>>,
}

impl TransactionManager {
    pub fn begin(&self) -> Transaction;
    pub fn commit(&self, tx: Transaction) -> Result<()>;
    pub fn rollback(&self, tx: Transaction) -> Result<()>;
}
```

### aegis-memory

Memory management primitives.

**Arena Allocator:**
```rust
pub struct Arena {
    blocks: Vec<Box<[u8]>>,
    current_block: usize,
    offset: usize,
    block_size: usize,
}

impl Arena {
    pub fn new(block_size: usize) -> Self;
    pub fn alloc(&mut self, size: usize) -> &mut [u8];
    pub fn alloc_aligned(&mut self, size: usize, align: usize) -> &mut [u8];
    pub fn reset(&mut self);
    pub fn bytes_allocated(&self) -> usize;
}
```

### aegis-query

SQL query engine with full pipeline.

**Parser:**
```rust
use sqlparser::parser::Parser;
use sqlparser::dialect::GenericDialect;

pub fn parse(sql: &str) -> Result<Vec<Statement>> {
    let dialect = GenericDialect {};
    Parser::parse_sql(&dialect, sql)
        .map_err(|e| QueryError::Parse(e.to_string()))
}
```

**Analyzer:**
```rust
pub struct Analyzer {
    schema: Schema,
}

impl Analyzer {
    pub fn analyze(&self, stmt: Statement) -> Result<ValidatedStatement> {
        // Type checking
        // Schema validation
        // Semantic analysis
    }
}
```

**Planner:**
```rust
pub struct QueryPlanner {
    statistics: TableStatistics,
}

impl QueryPlanner {
    pub fn plan(&self, stmt: ValidatedStatement) -> Result<ExecutionPlan> {
        // Cost estimation
        // Join reordering
        // Predicate pushdown
        // Index selection
    }
}
```

**Executor:**
```rust
pub struct QueryExecutor {
    storage: Arc<StorageEngine>,
}

impl QueryExecutor {
    pub fn execute(&self, plan: ExecutionPlan) -> Result<QueryResult> {
        match plan {
            ExecutionPlan::Scan(table) => self.execute_scan(table),
            ExecutionPlan::Filter(child, predicate) => self.execute_filter(child, predicate),
            ExecutionPlan::Join(left, right, condition) => self.execute_join(left, right, condition),
            // ...
        }
    }
}
```

### aegis-server

HTTP API server built on Axum.

**Router Setup:**
```rust
pub fn create_router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/health", get(handlers::health_check))
        .nest("/api/v1", api_routes())
        .nest("/api/v1/admin", admin_routes())
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(state)
}
```

**Handler Pattern:**
```rust
pub async fn execute_query(
    State(state): State<AppState>,
    Json(request): Json<QueryRequest>,
) -> impl IntoResponse {
    let start = Instant::now();

    let result = state.execute_query(&request.sql).await;
    let duration_ms = start.elapsed().as_millis() as u64;

    match result {
        Ok(data) => {
            state.record_request(duration_ms, true).await;
            (StatusCode::OK, Json(QueryResponse::success(data, duration_ms)))
        }
        Err(e) => {
            state.record_request(duration_ms, false).await;
            (StatusCode::BAD_REQUEST, Json(QueryResponse::error(e)))
        }
    }
}
```

**Application State:**
```rust
pub struct AppState {
    pub config: ServerConfig,
    pub auth: Arc<AuthService>,
    pub document_engine: Arc<DocumentEngine>,
    pub timeseries_engine: Arc<TimeSeriesEngine>,
    pub streaming_engine: Arc<StreamingEngine>,
    pub activity: Arc<ActivityLog>,
    pub metrics: Arc<MetricsCollector>,
}
```

### aegis-client

Rust client SDK with connection pooling.

**Client API:**
```rust
pub struct AegisClient {
    pool: ConnectionPool,
    config: ClientConfig,
}

impl AegisClient {
    pub async fn connect(config: ClientConfig) -> Result<Self>;
    pub async fn query(&self, sql: &str) -> Result<QueryResult>;
    pub async fn execute(&self, sql: &str) -> Result<u64>;
    pub async fn begin(&self) -> Result<Transaction>;
}
```

### aegis-replication

Distributed systems primitives.

**Raft Node:**
```rust
pub struct RaftNode {
    id: NodeId,
    state: RwLock<RaftState>,
    log: RwLock<Vec<LogEntry>>,
    peers: Vec<NodeId>,
}

#[derive(Clone, Copy, PartialEq)]
pub enum RaftState {
    Follower,
    Candidate,
    Leader,
}

impl RaftNode {
    pub async fn start(&self);
    pub async fn propose(&self, command: Command) -> Result<()>;
    pub fn is_leader(&self) -> bool;
}
```

**Hash Ring:**
```rust
pub struct HashRing {
    nodes: BTreeMap<u64, NodeId>,
    virtual_nodes: usize,
}

impl HashRing {
    pub fn add_node(&mut self, node: NodeId);
    pub fn remove_node(&mut self, node: NodeId);
    pub fn get_node(&self, key: &[u8]) -> Option<NodeId>;
    pub fn get_nodes(&self, key: &[u8], count: usize) -> Vec<NodeId>;
}
```

**CRDTs:**
```rust
// G-Counter
pub struct GCounter {
    counts: HashMap<NodeId, u64>,
}

impl GCounter {
    pub fn increment(&mut self, node: NodeId);
    pub fn value(&self) -> u64;
    pub fn merge(&mut self, other: &GCounter);
}

// LWW-Register
pub struct LWWRegister<T> {
    value: T,
    timestamp: u64,
}

// OR-Set
pub struct ORSet<T> {
    elements: HashMap<T, HashSet<(NodeId, u64)>>,
    tombstones: HashMap<T, HashSet<(NodeId, u64)>>,
}
```

### aegis-timeseries

Time series engine with Gorilla compression.

**Data Model:**
```rust
pub struct DataPoint {
    pub timestamp: DateTime<Utc>,
    pub value: f64,
    pub tags: Tags,
}

pub struct Metric {
    pub name: String,
    pub metric_type: MetricType,
    pub description: Option<String>,
    pub unit: Option<String>,
}

pub enum MetricType {
    Counter,
    Gauge,
    Histogram,
    Summary,
}
```

**Gorilla Compression:**
```rust
pub struct GorillaEncoder {
    output: BitWriter,
    prev_timestamp: i64,
    prev_delta: i64,
    prev_value: u64,
}

impl GorillaEncoder {
    pub fn encode_timestamp(&mut self, ts: i64);
    pub fn encode_value(&mut self, val: f64);
}
```

### aegis-document

Document store with full-text search.

**Document Model:**
```rust
pub struct Document {
    pub id: DocumentId,
    pub data: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub struct Query {
    pub filter: Option<Filter>,
    pub sort: Option<Sort>,
    pub skip: usize,
    pub limit: usize,
    pub projection: Vec<String>,
}
```

**Query Execution:**
```rust
impl DocumentEngine {
    pub fn find(&self, collection: &str, query: &Query) -> Result<QueryResult> {
        let collection = self.get_collection(collection)?;

        // Apply filter
        let filtered = match &query.filter {
            Some(f) => collection.documents.iter()
                .filter(|d| self.matches_filter(d, f))
                .collect(),
            None => collection.documents.iter().collect(),
        };

        // Apply sort, skip, limit
        // Return results
    }
}
```

### aegis-streaming

Real-time event streaming.

**Event Model:**
```rust
pub struct Event {
    pub id: EventId,
    pub channel: ChannelId,
    pub event_type: EventType,
    pub data: EventData,
    pub timestamp: DateTime<Utc>,
}

pub struct Channel {
    pub id: ChannelId,
    pub name: String,
    pub events: VecDeque<Event>,
    pub subscribers: Vec<SubscriberId>,
}
```

**Pub/Sub:**
```rust
impl StreamingEngine {
    pub fn publish(&self, channel: &str, event: Event) -> Result<EventId>;
    pub fn subscribe(&self, channel: &str) -> Receiver<Event>;
    pub fn get_history(&self, channel: &str, limit: usize) -> Vec<Event>;
}
```

### aegis-monitoring

Observability primitives.

**Metrics:**
```rust
pub trait Metric: Send + Sync {
    fn name(&self) -> &str;
    fn help(&self) -> &str;
    fn metric_type(&self) -> MetricType;
}

pub struct Counter {
    value: AtomicU64,
}

pub struct Gauge {
    value: AtomicI64,
}

pub struct Histogram {
    buckets: Vec<AtomicU64>,
    sum: AtomicU64,
    count: AtomicU64,
}
```

### aegis-dashboard

Web dashboard using Leptos (Rust → WASM).

**Component Pattern:**
```rust
use leptos::*;

#[component]
pub fn Dashboard() -> impl IntoView {
    let (cluster_info, set_cluster_info) = create_signal(None::<ClusterInfo>);

    // Fetch data on mount
    create_effect(move |_| {
        spawn_local(async move {
            let info = api::fetch_cluster_info().await;
            set_cluster_info(info.ok());
        });
    });

    view! {
        <div class="dashboard">
            <Header/>
            <Suspense fallback=|| view! { <Loading/> }>
                {move || cluster_info().map(|info| view! {
                    <ClusterOverview info=info/>
                })}
            </Suspense>
        </div>
    }
}
```

---

## Code Style & Conventions

### Rust Style Guide

Follow the official [Rust Style Guide](https://doc.rust-lang.org/nightly/style-guide/) with these additions:

**Formatting:**
```bash
# Use rustfmt for all code
cargo fmt --all
```

**Linting:**
```bash
# Run clippy with all warnings
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

**Naming Conventions:**
| Item | Convention | Example |
|------|------------|---------|
| Crates | `snake_case` | `aegis_server` |
| Modules | `snake_case` | `query_executor` |
| Types | `PascalCase` | `QueryResult` |
| Functions | `snake_case` | `execute_query` |
| Constants | `SCREAMING_SNAKE_CASE` | `MAX_CONNECTIONS` |
| Traits | `PascalCase` | `StorageBackend` |

**Documentation:**
```rust
/// Executes a SQL query and returns the result.
///
/// # Arguments
///
/// * `sql` - The SQL query string to execute
///
/// # Returns
///
/// A `Result` containing `QueryResult` on success or `QueryError` on failure.
///
/// # Examples
///
/// ```
/// let result = engine.execute("SELECT * FROM users")?;
/// println!("Found {} rows", result.row_count);
/// ```
///
/// # Errors
///
/// Returns `QueryError::Parse` if the SQL syntax is invalid.
/// Returns `QueryError::Execute` if execution fails.
pub fn execute(&self, sql: &str) -> Result<QueryResult, QueryError> {
    // Implementation
}
```

**Error Handling:**
```rust
// Use thiserror for library errors
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("Key not found: {0}")]
    KeyNotFound(String),

    #[error("Transaction conflict")]
    TransactionConflict,

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

// Use anyhow for application errors (main, tests)
use anyhow::{Context, Result};

fn main() -> Result<()> {
    let config = load_config()
        .context("Failed to load configuration")?;
    Ok(())
}
```

**Module Organization:**
```rust
// lib.rs
pub mod engine;
pub mod query;
pub mod types;

// Re-exports for convenient imports
pub use engine::DocumentEngine;
pub use query::{Query, QueryResult};
pub use types::{Document, DocumentId};
```

---

## Testing

### Unit Tests

Located alongside the code in `mod tests`:

```rust
// src/query.rs
pub fn parse_filter(input: &str) -> Result<Filter> {
    // Implementation
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_filter() {
        let filter = parse_filter("status = 'active'").unwrap();
        assert_eq!(filter.field, "status");
        assert_eq!(filter.operator, Operator::Eq);
    }

    #[test]
    fn test_parse_invalid_filter() {
        let result = parse_filter("invalid syntax !!!");
        assert!(result.is_err());
    }
}
```

### Integration Tests

Located in `tests/` directory:

```rust
// crates/aegis-server/tests/integration_test.rs
use axum::http::StatusCode;
use aegis_server::{create_router, AppState};

#[tokio::test]
async fn test_health_endpoint() {
    let state = AppState::new(Default::default());
    let app = create_router(state);

    let response = app
        .oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_query_execution() {
    let state = shared_state();
    let mut app = app_with_state(state);

    let (status, json) = post_json(
        &mut app,
        "/api/v1/query",
        json!({"sql": "SELECT 1 + 1 AS result"}),
    ).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["success"], true);
}
```

### End-to-End Tests

Full system tests with real HTTP requests:

```rust
#[tokio::test]
async fn test_full_workflow_e2e() {
    // Start server
    let server = TestServer::start().await;

    // Create collection
    let resp = server.post("/api/v1/documents/collections", json!({"name": "test"})).await;
    assert_eq!(resp.status(), 201);

    // Insert document
    let resp = server.post("/api/v1/documents/collections/test/documents", json!({
        "document": {"name": "Test", "value": 42}
    })).await;
    assert_eq!(resp.status(), 201);

    // Query document
    let resp = server.get("/api/v1/documents/collections/test").await;
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await;
    assert_eq!(body["documents"].as_array().unwrap().len(), 1);
}
```

### Running Tests

```bash
# Run all tests
cargo test --workspace

# Run tests for specific crate
cargo test -p aegis-server

# Run specific test
cargo test -p aegis-server test_health_endpoint

# Run with output
cargo test -- --nocapture

# Run ignored tests
cargo test -- --ignored

# Run tests in parallel
cargo test -- --test-threads=4

# Generate coverage report
cargo tarpaulin --workspace --out Html
```

### Benchmarks

Using Criterion for benchmarks:

```rust
// benches/storage_bench.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use aegis_storage::MemoryBackend;

fn bench_put(c: &mut Criterion) {
    let backend = MemoryBackend::new();

    c.bench_function("put 1KB value", |b| {
        let key = b"test_key";
        let value = vec![0u8; 1024];
        b.iter(|| {
            backend.put(black_box(key), black_box(&value)).unwrap();
        });
    });
}

criterion_group!(benches, bench_put);
criterion_main!(benches);
```

```bash
# Run benchmarks
cargo bench --workspace

# Run specific benchmark
cargo bench -p aegis-storage
```

---

## API Development

### Adding New Endpoints

1. **Define types in `handlers.rs`:**
```rust
#[derive(Debug, Deserialize)]
pub struct CreateWidgetRequest {
    pub name: String,
    pub config: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct WidgetResponse {
    pub id: String,
    pub name: String,
    pub created_at: String,
}
```

2. **Implement handler:**
```rust
pub async fn create_widget(
    State(state): State<AppState>,
    Json(request): Json<CreateWidgetRequest>,
) -> impl IntoResponse {
    // Validate input
    if request.name.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "Name required"})));
    }

    // Create widget
    let widget = Widget {
        id: generate_id(),
        name: request.name,
        config: request.config,
        created_at: Utc::now(),
    };

    // Save to storage
    state.widget_store.create(widget.clone())?;

    // Log activity
    state.activity.log(ActivityType::Write, &format!("Created widget: {}", widget.id));

    // Return response
    (StatusCode::CREATED, Json(WidgetResponse::from(widget)))
}
```

3. **Add route:**
```rust
// router.rs
let widget_routes = Router::new()
    .route("/widgets", get(handlers::list_widgets))
    .route("/widgets", post(handlers::create_widget))
    .route("/widgets/:id", get(handlers::get_widget))
    .route("/widgets/:id", delete(handlers::delete_widget));

Router::new()
    .nest("/api/v1", api_routes)
    .nest("/api/v1/widgets", widget_routes)
```

4. **Add tests:**
```rust
#[tokio::test]
async fn test_create_widget() {
    let state = shared_state();
    let mut app = app_with_state(state);

    let (status, json) = post_json(
        &mut app,
        "/api/v1/widgets",
        json!({"name": "Test Widget", "config": {}}),
    ).await;

    assert_eq!(status, StatusCode::CREATED);
    assert!(json["id"].is_string());
}
```

### Request/Response Types

Use Serde for serialization:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct QueryParams {
    #[serde(default = "default_limit")]
    pub limit: usize,
    pub offset: Option<usize>,
    pub filter: Option<String>,
}

fn default_limit() -> usize { 100 }

#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub execution_time_ms: u64,
}
```

### Error Handling

```rust
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};

pub enum ApiError {
    BadRequest(String),
    NotFound(String),
    Unauthorized,
    Internal(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            ApiError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            ApiError::Unauthorized => (StatusCode::UNAUTHORIZED, "Unauthorized".to_string()),
            ApiError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };

        (status, Json(json!({"error": message}))).into_response()
    }
}

// Usage in handler
pub async fn get_widget(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<WidgetResponse>, ApiError> {
    let widget = state.widget_store
        .get(&id)
        .ok_or_else(|| ApiError::NotFound(format!("Widget not found: {}", id)))?;

    Ok(Json(WidgetResponse::from(widget)))
}
```

---

## Storage Engine Development

### Adding a New Backend

1. **Implement the trait:**
```rust
// src/backend/redis.rs
use async_trait::async_trait;
use redis::Client;

pub struct RedisBackend {
    client: Client,
}

#[async_trait]
impl StorageBackend for RedisBackend {
    async fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        let mut conn = self.client.get_async_connection().await?;
        let value: Option<Vec<u8>> = conn.get(key).await?;
        Ok(value)
    }

    async fn put(&self, key: &[u8], value: &[u8]) -> Result<()> {
        let mut conn = self.client.get_async_connection().await?;
        conn.set(key, value).await?;
        Ok(())
    }

    async fn delete(&self, key: &[u8]) -> Result<()> {
        let mut conn = self.client.get_async_connection().await?;
        conn.del(key).await?;
        Ok(())
    }

    async fn scan(&self, start: &[u8], end: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>> {
        // Implementation
    }
}
```

2. **Register in factory:**
```rust
// src/backend/mod.rs
pub fn create_backend(config: &StorageConfig) -> Box<dyn StorageBackend> {
    match config.backend.as_str() {
        "memory" => Box::new(MemoryBackend::new()),
        "local" => Box::new(LocalBackend::new(&config.data_directory)),
        "redis" => Box::new(RedisBackend::new(&config.redis_url)),
        _ => panic!("Unknown backend: {}", config.backend),
    }
}
```

### Transaction Implementation

```rust
pub struct Transaction {
    pub id: TransactionId,
    pub start_time: Instant,
    pub read_set: HashSet<Key>,
    pub write_set: HashMap<Key, Value>,
    pub status: TransactionStatus,
}

impl TransactionManager {
    pub fn begin(&self) -> Transaction {
        let id = self.next_transaction_id();
        Transaction {
            id,
            start_time: Instant::now(),
            read_set: HashSet::new(),
            write_set: HashMap::new(),
            status: TransactionStatus::Active,
        }
    }

    pub fn commit(&self, mut tx: Transaction) -> Result<()> {
        // Validate read set (no conflicts)
        for key in &tx.read_set {
            if self.has_newer_version(key, tx.id)? {
                return Err(StorageError::TransactionConflict);
            }
        }

        // Write all changes
        let commit_version = self.next_version();
        for (key, value) in tx.write_set.drain() {
            self.write_version(key, value, commit_version)?;
        }

        tx.status = TransactionStatus::Committed;
        Ok(())
    }
}
```

### Write-Ahead Log

```rust
pub struct WAL {
    file: File,
    buffer: Vec<u8>,
    sequence: AtomicU64,
}

#[derive(Serialize, Deserialize)]
pub struct WALEntry {
    pub sequence: u64,
    pub operation: WALOperation,
    pub key: Vec<u8>,
    pub value: Option<Vec<u8>>,
    pub checksum: u32,
}

impl WAL {
    pub fn append(&mut self, op: WALOperation, key: &[u8], value: Option<&[u8]>) -> Result<u64> {
        let seq = self.sequence.fetch_add(1, Ordering::SeqCst);

        let entry = WALEntry {
            sequence: seq,
            operation: op,
            key: key.to_vec(),
            value: value.map(|v| v.to_vec()),
            checksum: 0, // Calculate CRC32
        };

        let bytes = bincode::serialize(&entry)?;
        self.buffer.extend_from_slice(&bytes);

        if self.buffer.len() >= FLUSH_THRESHOLD {
            self.flush()?;
        }

        Ok(seq)
    }

    pub fn flush(&mut self) -> Result<()> {
        self.file.write_all(&self.buffer)?;
        self.file.sync_all()?;
        self.buffer.clear();
        Ok(())
    }

    pub fn recover(&self) -> Result<Vec<WALEntry>> {
        // Read and replay WAL entries
    }
}
```

---

## Query Engine Development

### Parser Extensions

Adding new SQL syntax:

```rust
// Extend AST
pub enum Statement {
    Select(SelectStatement),
    Insert(InsertStatement),
    // Add new statement type
    CreateStream(CreateStreamStatement),
}

pub struct CreateStreamStatement {
    pub name: String,
    pub columns: Vec<ColumnDef>,
    pub source: StreamSource,
}

// Parse in parser.rs
fn parse_statement(&mut self) -> Result<Statement> {
    let token = self.peek_token();
    match token {
        Token::Keyword(Keyword::CREATE) => {
            self.next_token();
            let next = self.peek_token();
            match next {
                Token::Keyword(Keyword::TABLE) => self.parse_create_table(),
                Token::Keyword(Keyword::STREAM) => self.parse_create_stream(),
                _ => Err(ParseError::UnexpectedToken(next)),
            }
        }
        // ...
    }
}
```

### Adding Operators

```rust
// New execution node
pub enum ExecutionPlan {
    Scan(ScanNode),
    Filter(Box<ExecutionPlan>, Expression),
    Project(Box<ExecutionPlan>, Vec<Expression>),
    // Add new operator
    Window(Box<ExecutionPlan>, WindowSpec),
}

pub struct WindowNode {
    pub child: Box<ExecutionPlan>,
    pub partition_by: Vec<Expression>,
    pub order_by: Vec<OrderByExpr>,
    pub frame: WindowFrame,
}

// Implement execution
impl QueryExecutor {
    fn execute_window(&self, node: WindowNode) -> Result<QueryResult> {
        let input = self.execute(*node.child)?;

        // Group by partition keys
        let partitions = self.partition_rows(&input.rows, &node.partition_by)?;

        // Apply window function to each partition
        let mut results = Vec::new();
        for partition in partitions {
            let windowed = self.apply_window_function(partition, &node)?;
            results.extend(windowed);
        }

        Ok(QueryResult { rows: results, ..input })
    }
}
```

### Query Optimization

```rust
pub struct QueryOptimizer {
    rules: Vec<Box<dyn OptimizationRule>>,
}

pub trait OptimizationRule {
    fn apply(&self, plan: ExecutionPlan) -> ExecutionPlan;
}

// Predicate pushdown
pub struct PredicatePushdown;

impl OptimizationRule for PredicatePushdown {
    fn apply(&self, plan: ExecutionPlan) -> ExecutionPlan {
        match plan {
            ExecutionPlan::Filter(child, predicate) => {
                match *child {
                    ExecutionPlan::Join(left, right, condition) => {
                        // Push predicate to appropriate side
                        let (left_pred, right_pred) = split_predicate(&predicate);
                        ExecutionPlan::Join(
                            Box::new(ExecutionPlan::Filter(left, left_pred)),
                            Box::new(ExecutionPlan::Filter(right, right_pred)),
                            condition,
                        )
                    }
                    _ => ExecutionPlan::Filter(child, predicate),
                }
            }
            _ => plan,
        }
    }
}
```

---

## Distributed Systems Development

### Raft Consensus

```rust
// State machine
impl RaftNode {
    async fn run(&self) {
        loop {
            match self.state() {
                RaftState::Follower => self.run_follower().await,
                RaftState::Candidate => self.run_candidate().await,
                RaftState::Leader => self.run_leader().await,
            }
        }
    }

    async fn run_follower(&self) {
        let timeout = random_election_timeout();
        tokio::select! {
            _ = tokio::time::sleep(timeout) => {
                // Election timeout - become candidate
                self.set_state(RaftState::Candidate);
            }
            msg = self.receive_message() => {
                self.handle_message(msg).await;
            }
        }
    }

    async fn run_leader(&self) {
        // Send heartbeats
        let heartbeat_interval = Duration::from_millis(50);
        loop {
            self.send_heartbeats().await;
            tokio::time::sleep(heartbeat_interval).await;
        }
    }
}
```

### Sharding

```rust
pub struct ShardRouter {
    hash_ring: HashRing,
    shard_map: HashMap<ShardId, Vec<NodeId>>,
}

impl ShardRouter {
    pub fn route(&self, key: &[u8]) -> ShardId {
        let hash = self.hash_ring.hash(key);
        ShardId(hash % self.shard_count)
    }

    pub fn get_nodes(&self, shard: ShardId) -> Vec<NodeId> {
        self.shard_map.get(&shard).cloned().unwrap_or_default()
    }

    pub fn rebalance(&mut self) {
        // Redistribute shards when nodes change
    }
}
```

### CRDTs

```rust
// G-Counter implementation
#[derive(Clone, Default)]
pub struct GCounter {
    counts: HashMap<NodeId, u64>,
}

impl GCounter {
    pub fn increment(&mut self, node: NodeId) {
        *self.counts.entry(node).or_insert(0) += 1;
    }

    pub fn value(&self) -> u64 {
        self.counts.values().sum()
    }

    pub fn merge(&mut self, other: &GCounter) {
        for (node, &count) in &other.counts {
            let entry = self.counts.entry(*node).or_insert(0);
            *entry = (*entry).max(count);
        }
    }
}

// PN-Counter (supports decrement)
pub struct PNCounter {
    increments: GCounter,
    decrements: GCounter,
}

impl PNCounter {
    pub fn increment(&mut self, node: NodeId) {
        self.increments.increment(node);
    }

    pub fn decrement(&mut self, node: NodeId) {
        self.decrements.increment(node);
    }

    pub fn value(&self) -> i64 {
        self.increments.value() as i64 - self.decrements.value() as i64
    }
}
```

---

## Dashboard Development

### Leptos/WASM

Project setup:
```toml
# Cargo.toml
[dependencies]
leptos = { version = "0.6", features = ["csr"] }
wasm-bindgen = "0.2"
web-sys = { version = "0.3", features = ["Window", "Document"] }
gloo-net = "0.5"
```

### Component Development

```rust
use leptos::*;

// Reactive state
#[derive(Clone)]
pub struct AppState {
    pub user: RwSignal<Option<UserInfo>>,
    pub theme: RwSignal<Theme>,
}

// Component with props
#[component]
pub fn NodeCard(
    #[prop(into)] node: MaybeSignal<NodeInfo>,
    #[prop(optional)] on_click: Option<Callback<NodeId>>,
) -> impl IntoView {
    view! {
        <div
            class="node-card"
            class:healthy=move || node().status == NodeStatus::Healthy
            on:click=move |_| {
                if let Some(cb) = on_click {
                    cb(node().id);
                }
            }
        >
            <h3>{move || node().name}</h3>
            <StatusBadge status=node().status />
            <div class="metrics">
                <Metric label="CPU" value=move || format!("{}%", node().cpu) />
                <Metric label="Memory" value=move || format!("{}%", node().memory) />
            </div>
        </div>
    }
}
```

### State Management

```rust
// Global state provider
#[component]
pub fn AppProvider(children: Children) -> impl IntoView {
    let state = AppState {
        user: create_rw_signal(None),
        theme: create_rw_signal(Theme::Light),
    };

    provide_context(state);

    children()
}

// Using context in components
#[component]
pub fn UserMenu() -> impl IntoView {
    let state = expect_context::<AppState>();

    view! {
        <Show
            when=move || state.user.get().is_some()
            fallback=|| view! { <LoginButton/> }
        >
            {move || {
                let user = state.user.get().unwrap();
                view! {
                    <div class="user-menu">
                        <span>{user.username}</span>
                        <LogoutButton/>
                    </div>
                }
            }}
        </Show>
    }
}
```

---

## Performance Optimization

### Profiling

```bash
# CPU profiling with perf
perf record --call-graph=dwarf cargo run --release -p aegis-server
perf report

# Memory profiling with heaptrack
heaptrack cargo run --release -p aegis-server

# Flamegraph
cargo install flamegraph
cargo flamegraph -p aegis-server
```

### Common Optimizations

**Avoid unnecessary allocations:**
```rust
// Bad
fn process_items(items: Vec<Item>) -> Vec<ProcessedItem> {
    items.iter().map(|i| process(i)).collect()
}

// Good - reuse capacity
fn process_items(items: Vec<Item>) -> Vec<ProcessedItem> {
    let mut results = Vec::with_capacity(items.len());
    for item in items {
        results.push(process(item));
    }
    results
}
```

**Use references when possible:**
```rust
// Bad
fn find_by_name(items: &[Item], name: String) -> Option<&Item> {
    items.iter().find(|i| i.name == name)
}

// Good
fn find_by_name(items: &[Item], name: &str) -> Option<&Item> {
    items.iter().find(|i| i.name == name)
}
```

**Batch operations:**
```rust
// Bad - individual writes
for item in items {
    storage.put(&item.key, &item.value)?;
}

// Good - batch write
storage.put_batch(items.iter().map(|i| (&i.key, &i.value)))?;
```

---

## Debugging

### Logging

```rust
use tracing::{debug, error, info, instrument, warn};

#[instrument(skip(self))]
pub async fn execute_query(&self, sql: &str) -> Result<QueryResult> {
    debug!(sql = %sql, "Executing query");

    let start = Instant::now();
    let result = self.inner_execute(sql).await;
    let duration = start.elapsed();

    match &result {
        Ok(r) => info!(rows = r.row_count, duration_ms = duration.as_millis(), "Query completed"),
        Err(e) => error!(error = %e, "Query failed"),
    }

    result
}
```

```bash
# Run with debug logging
RUST_LOG=debug cargo run -p aegis-server

# Specific module logging
RUST_LOG=aegis_query=trace,aegis_storage=debug cargo run -p aegis-server
```

### Debug Builds

```toml
# Cargo.toml
[profile.dev]
opt-level = 0
debug = true

[profile.release]
opt-level = 3
debug = false  # Set to true for release debugging
```

### Common Debug Techniques

```rust
// Debug derive
#[derive(Debug)]
pub struct QueryPlan {
    // fields
}

// Custom Debug implementation
impl std::fmt::Debug for LargeStruct {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LargeStruct")
            .field("id", &self.id)
            .field("data_len", &self.data.len())  // Don't print entire data
            .finish()
    }
}

// dbg! macro
let result = dbg!(compute_something());

// Conditional debugging
#[cfg(debug_assertions)]
fn debug_validate(&self) {
    assert!(self.invariants_hold());
}
```

---

## Release Process

### Version Bumping

```bash
# Update version in all Cargo.toml files
./scripts/bump-version.sh 0.2.0
```

### Release Checklist

1. **Code Quality:**
   ```bash
   cargo fmt --all --check
   cargo clippy --workspace --all-targets -- -D warnings
   cargo test --workspace
   cargo audit
   ```

2. **Update Documentation:**
   - Update CHANGELOG.md
   - Update version in README badges
   - Update API documentation

3. **Create Release:**
   ```bash
   git tag -a v0.2.0 -m "Release v0.2.0"
   git push origin v0.2.0
   ```

4. **Build Artifacts:**
   ```bash
   ./scripts/release.sh
   # Creates:
   # - aegis-db-0.2.0-linux-x86_64.tar.gz
   # - aegis-db-0.2.0-macos-x86_64.tar.gz
   # - aegis-db-0.2.0-windows-x86_64.zip
   ```

5. **Publish to crates.io (if applicable):**
   ```bash
   cargo publish -p aegis-common
   cargo publish -p aegis-storage
   # ... in dependency order
   ```

---

## Contributing

### Getting Started

1. Fork the repository
2. Create a feature branch: `git checkout -b feature/amazing-feature`
3. Make your changes
4. Run tests: `cargo test --workspace`
5. Run lints: `cargo clippy --workspace`
6. Commit: `git commit -m 'Add amazing feature'`
7. Push: `git push origin feature/amazing-feature`
8. Open a Pull Request

### Pull Request Guidelines

- Include tests for new functionality
- Update documentation as needed
- Follow existing code style
- Keep changes focused and atomic
- Write meaningful commit messages

### Code Review

All PRs require review before merging. Reviewers will check:
- Code correctness
- Test coverage
- Documentation
- Performance implications
- Security considerations

### Community

- **GitHub Issues**: Bug reports and feature requests
- **Discord**: Real-time discussion
- **Discussions**: Long-form conversations

---

<p align="center">
  <strong>Happy Coding!</strong><br>
  AegisDB Development Team
</p>
