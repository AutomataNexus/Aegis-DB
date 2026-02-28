#!/usr/bin/env python3
"""
Aegis Database Platform - System Architecture Documentation

Comprehensive technical architecture specification for the unified
database platform, covering system design, component interactions,
and implementation strategies.

Key Components:
- Multi-paradigm storage engine architecture
- Distributed systems design patterns
- API and SDK architecture
- Deployment model specifications
- Security and compliance framework

@version 2.2.0
@author Andrew Jewell Sr / AutomataNexus LLC
"""

# Aegis Database Platform
## System Architecture Documentation

**Version:** 2.2.0
**Author:** Andrew Jewell Sr / AutomataNexus LLC
**Updated:** February 28, 2026
**Status:** All Phases Complete — Production Deployed

---

## Implementation Status

### Phase 1: Core Foundation - COMPLETE

| Component | Crate | Status | Tests |
|-----------|-------|--------|-------|
| Storage Abstraction | aegis-storage | Complete | 48 |
| Memory Management | aegis-memory | Complete | 5 |
| MVCC Transactions | aegis-storage | Complete | - |
| Write-Ahead Log | aegis-storage | Complete | - |
| SQL Parser | aegis-query | Complete | 41 |
| Semantic Analyzer | aegis-query | Complete | - |
| Query Planner | aegis-query | Complete | - |
| Query Executor | aegis-query | Complete | - |
| B-tree & Hash Indexes | aegis-query | Complete | - |
| Plan Cache (LRU 1024) | aegis-query | Complete | - |
| Direct Execution API | aegis-query | Complete | - |
| REST API | aegis-server | Complete | 116 |
| Common Types | aegis-common | Complete | 4 |

### Phase 2: Multi-Paradigm Support - COMPLETE

| Component | Crate | Status | Tests |
|-----------|-------|--------|-------|
| Time Series Engine | aegis-timeseries | Complete | 36 |
| Document Store | aegis-document | Complete | 36 |
| Real-time Streaming | aegis-streaming | Complete | 31 |
| Graph Database | aegis-server | Complete | - |

### Phase 3: Distribution & Scale - COMPLETE

| Component | Crate | Status | Tests |
|-----------|-------|--------|-------|
| Raft Consensus | aegis-replication | Complete | 165 |
| Sharding & Partitioning | aegis-replication | Complete | - |
| Distributed Transactions | aegis-replication | Complete | - |
| CRDTs & Vector Clocks | aegis-replication | Complete | - |
| Monitoring & Tracing | aegis-monitoring | Complete | 35 |
| Client SDK | aegis-client | Complete | 42 |
| Web Dashboard | aegis-dashboard | Complete | - |
| E2E Integration Tests | aegis-server | Complete | 54 |

### Phase 4-5: Enterprise Features - COMPLETE

| Component | Crate/Location | Status | Tests |
|-----------|---------------|--------|-------|
| Enterprise Auth (LDAP, OAuth2, OIDC) | aegis-server | Complete | - |
| Role-Based Access Control (RBAC) | aegis-server | Complete | - |
| Audit Logging & Compliance | aegis-server | Complete | - |
| Multi-database Support | aegis-server | Complete | - |
| Bulk Data Import (CSV/JSON) | aegis-server | Complete | - |
| VACUUM/Compaction | aegis-server | Complete | - |
| Python & JavaScript SDKs | sdks/ | Complete | - |
| Grafana Data Source Plugin | - | Complete | - |

### Phase 6: Security & Compliance Hardening - COMPLETE

- **TLS/HTTPS**: Native TLS support via axum-server with rustls (TLSv1.2/1.3)
- **Password Security**: Argon2id hashing (19 MiB memory, 2 iterations, unique random salts)
- **Rate Limiting**: Token bucket algorithm (1000/min API, 30/min login)
- **Secrets Management**: HashiCorp Vault integration (AppRole, Kubernetes auth)
- **Secure Tokens**: Cryptographically secure random token generation
- **Breach Detection**: Real-time monitoring with webhook notifications
- **Consent Management**: 11 consent purposes with full audit trail
- **PHI/Data Classification**: 6-level system (Public, Internal, Confidential, PHI, PII, Restricted)
- **Query Safety Limits**: max_result_rows (default 100K), query_timeout_secs (default 30)
- **Log Rotation**: Daily rotating log files via tracing-appender
- **GDPR Right to Erasure**: Data subject deletion with cryptographic deletion certificates
- **CCPA Do Not Sell**: Opt-out tracking and compliance

### Phase 7: Edge Deployment & OTA Updates - COMPLETE

| Component | Crate | Status | Tests |
|-----------|-------|--------|-------|
| OTA Rolling Updates | aegis-updates | Complete | 14 |

- **Edge Runtime**: Runs on Raspberry Pi 4/5 (ARM64) alongside other workloads
- **OTA Rolling Updates**: Binary download, checksum verify, follower-first deploy, health check, auto-rollback
- **CRDT Replication**: GCounter, PNCounter, ORSet, LWWRegister, LWWMap for conflict-free edge sync
- **Production Deployment**: 50+ Raspberry Pi nodes across commercial facilities reporting to central NexusBMS server

**Total: 634 tests passing across all crates**

---

## Installation & Operations

### Prerequisites

| Requirement | Version | Installation |
|-------------|---------|--------------|
| Rust        | 1.75+   | https://rustup.rs/ |
| Trunk       | Latest  | `cargo install trunk` |

### Quick Install

```bash
git clone https://github.com/automatanexus/aegis-db.git
cd aegis-db
./install.sh
```

The installer builds both the server and dashboard, then installs the `aegis` CLI command globally.

### Aegis CLI Commands

```bash
# Service Management
aegis start              # Start server + dashboard
aegis stop               # Stop all services
aegis restart            # Restart all services
aegis status             # Show service status

# Build
aegis build              # Build server + dashboard (release)

# Individual Services
aegis server start       # Start API server only (port 9090)
aegis server stop        # Stop API server
aegis dashboard start    # Start dashboard only (port 8000)
aegis dashboard stop     # Stop dashboard

# Logging
aegis logs               # View recent logs (both)
aegis logs server        # Follow server logs
aegis logs dashboard     # Follow dashboard logs
```

### Service Endpoints

| Service     | Port | URL                          |
|-------------|------|------------------------------|
| API Server  | 9090 | http://localhost:9090        |
| Dashboard   | 8000 | http://localhost:8000        |
| Health      | 9090 | http://localhost:9090/health |

### Credentials Configuration

Set credentials via environment variables before starting:
```bash
export AEGIS_ADMIN_USERNAME=your_admin_username
export AEGIS_ADMIN_PASSWORD=your_secure_password
```

### File Locations

| File | Purpose |
|------|---------|
| `/tmp/aegis-server.log` | Server logs |
| `/tmp/aegis-dashboard.log` | Dashboard logs |
| `/tmp/aegis-server.pid` | Server PID file |
| `/tmp/aegis-dashboard.pid` | Dashboard PID file |
| `~/.local/bin/aegis` | Installed CLI command |

### Uninstall

```bash
./install.sh uninstall
```

---

## Executive Summary

Aegis is a unified database platform that combines multiple database paradigms (relational, time series, document, graph, real-time streaming) into a single, high-performance system built in Rust. The architecture emphasizes performance, safety, scalability, and developer experience while supporting cloud, on-premise, and embedded deployment models.

### Core Design Principles

- **Unified Storage**: Single storage layer supporting multiple data models
- **Performance First**: Rust implementation with zero-cost abstractions
- **Safety**: Memory safety without garbage collection overhead
- **Scalability**: Horizontal scaling from single nodes to 1000+ clusters
- **Flexibility**: Multiple deployment models with consistent APIs
- **Multi-Database Isolation**: Per-application database instances with auto-provisioning

---

# =============================================================================
# System Overview
# =============================================================================

## High-Level Architecture

```
+-----------------------------------------------------------------------------+
|                           Aegis Database Platform                           |
+-----------------------------------------------------------------------------+
|                              Client Layer                                   |
+---------------+-----------------+-----------------+-------------------------+
|   Web UI      |   CLI Tools     |   SDKs          |   Third-party Tools     |
|   Dashboard   |   aegis-cli     |   Rust/Python   |   Grafana/Tableau      |
+---------------+-----------------+-----------------+-------------------------+
|                              API Gateway (port 9090)                        |
+-----------------+-----------------+-----------------+-----------------------+
|   REST API      |   WebSocket     |   Bulk Import   |   OTA Updates         |
+-----------------+-----------------+-----------------+-----------------------+
|                           Query Processing Layer                            |
+---------+-----------+-----------+-----------+-----------+-------------------+
|   SQL   | TimeSeries| Document  | Graph     | Streaming | Plan Cache (LRU)  |
|  Engine | Processor | Processor | Engine    | Processor | B-tree/Hash Index |
+---------+-----------+-----------+-----------+-----------+-------------------+
|                            Storage Engine                                   |
+-----------------+-----------------+-----------------+-----------------------+
|   Unified       |   Indexing      |   Replication   |   Consensus           |
|   Storage       |   (B-tree/Hash) |   Engine        |   Layer (Raft)        |
+-----------------+-----------------+-----------------+-----------------------+
|                          Infrastructure Layer                               |
+-----------------+-----------------+-----------------+-----------------------+
|   Monitoring    |   Security      |   Backup/       |   Resource            |
|   & Metrics     |   & Compliance  |   Recovery      |   Management          |
+-----------------+-----------------+-----------------+-----------------------+
```

## Component Responsibilities

**Client Layer:**
- User interfaces and developer tools
- SDK libraries for multiple programming languages (Rust, Python, JavaScript)
- Third-party tool integrations (Grafana data source plugin)

**API Gateway (port 9090):**
- REST API protocol translation and routing
- Authentication and authorization (Local, LDAP, OAuth2/OIDC)
- Rate limiting (token bucket) and request validation
- Bulk data import (CSV/JSON for SQL, documents, KV)
- OTA update management

**Query Processing Layer:**
- SQL parsing, optimization, and execution with B-tree and Hash index acceleration
- Plan cache (LRU, 1024 entries) for query plan reuse
- Direct execution API (closure-based updates bypassing SQL parsing)
- Time series query processing and aggregation (Gorilla compression)
- Document query execution (JSONPath, full-text search)
- Graph traversal (nodes, edges, adjacency lists with O(degree) traversal)
- Real-time stream processing and event handling
- Query safety limits (max_result_rows: 100K, query_timeout_secs: 30)

**Storage Engine:**
- Unified storage abstraction across data models
- Multi-algorithm compression (LZ4, Zstd, Snappy)
- Advanced indexing for multiple access patterns (B-tree, Hash, inverted)
- Distributed replication and Raft consensus
- Transaction management with MVCC and ACID guarantees
- VACUUM/Compaction (dead row removal, index rebuild)

**Infrastructure Layer:**
- System monitoring and observability
- Security enforcement, audit logging, and compliance (GDPR, HIPAA, CCPA, SOC 2, FERPA)
- PHI/Data classification (6-level: Public, Internal, Confidential, PHI, PII, Restricted)
- Breach detection and notification
- Consent management with audit trail
- Backup, recovery, and disaster management
- Log rotation (daily rotating files via tracing-appender)
- Resource allocation and performance optimization

---

# =============================================================================
# Storage Engine Architecture
# =============================================================================

## Unified Storage Layer

### Storage Abstraction Design

The storage layer provides a unified interface that supports multiple data models while maintaining optimal performance for each paradigm.

```rust
pub trait StorageBackend {
    // Core storage operations
    fn write_block(&mut self, block: Block) -> Result<BlockId>;
    fn read_block(&self, id: BlockId) -> Result<Block>;
    fn delete_block(&mut self, id: BlockId) -> Result<()>;

    // Transaction support
    fn begin_transaction(&mut self) -> Result<TransactionId>;
    fn commit_transaction(&mut self, tx_id: TransactionId) -> Result<()>;
    fn rollback_transaction(&mut self, tx_id: TransactionId) -> Result<()>;

    // Replication hooks
    fn replicate_block(&self, block: Block, replicas: &[NodeId]) -> Result<()>;
    fn receive_replicated_block(&mut self, block: Block) -> Result<()>;
}
```

### Data Model Integration

**Relational Tables:**
- Row-oriented storage with columnar optimization
- B-tree and Hash indexes for primary and foreign keys
- Index-accelerated SELECT, UPDATE, and DELETE operations
- Compression using dictionary encoding and delta compression

**Time Series Data:**
- Time-partitioned storage with automatic aging
- Specialized compression (Gorilla algorithm for floating point, delta-of-delta for timestamps)
- Downsampling policies with configurable retention

**Document Collections:**
- Schema-flexible JSON/BSON storage
- Inverted indexes for full-text search
- Nested object indexing with JSONPath optimization

**Graph Data:**
- Node and edge storage with adjacency lists
- O(degree) traversal performance
- Relationship-type filtering and property queries

**Real-time Streams:**
- Write-ahead log (WAL) based storage
- Event sourcing with snapshot capabilities
- Pub/sub topic management with persistent subscriptions

### Block Structure and Format

```rust
pub struct Block {
    pub header: BlockHeader,
    pub data: Vec<u8>,
    pub checksum: u64,
}

pub struct BlockHeader {
    pub block_type: BlockType,
    pub timestamp: i64,
    pub compression: CompressionType,
    pub encryption: EncryptionType,
    pub size: u32,
    pub version: u16,
}

pub enum BlockType {
    TableData,
    TimeSeriesData,
    DocumentData,
    IndexData,
    LogEntry,
    Metadata,
}

pub enum CompressionType {
    None,
    Lz4,
    Zstd,
    Snappy,
}
```

## Indexing System

### Multi-Model Indexing Strategy

**B-tree Indexes:**
- Primary indexes for relational data
- Range queries and exact matches
- Index-accelerated SELECT, UPDATE, and DELETE
- Bulk loading and incremental updates

**Hash Indexes:**
- O(1) exact-match lookups
- Optimal for equality predicates on high-cardinality columns

**LSM Tree Indexes:**
- Write-optimized for high-throughput ingestion
- Time series data and log-structured storage
- Compaction strategies for space efficiency

**Inverted Indexes:**
- Full-text search for document fields
- Tokenization and stemming support
- Phrase and fuzzy matching capabilities

**Bitmap Indexes:**
- Low-cardinality categorical data
- Fast aggregations and filtering
- Compressed bitmap storage (RoaringBitmap)

### Index Management

```rust
pub struct IndexManager {
    btree_indexes: HashMap<String, BTreeIndex>,
    hash_indexes: HashMap<String, HashIndex>,
    lsm_indexes: HashMap<String, LSMIndex>,
    inverted_indexes: HashMap<String, InvertedIndex>,
    bitmap_indexes: HashMap<String, BitmapIndex>,
}

impl IndexManager {
    pub fn create_index(&mut self, spec: IndexSpec) -> Result<()>;
    pub fn drop_index(&mut self, name: &str) -> Result<()>;
    pub fn rebuild_index(&mut self, name: &str) -> Result<()>;
    pub fn optimize_indexes(&mut self) -> Result<()>;
}
```

## Memory Management

### Arena-Based Allocation

```rust
pub struct MemoryArena {
    blocks: Vec<Block>,
    free_list: Vec<usize>,
    allocation_size: usize,
}

impl MemoryArena {
    pub fn new(initial_size: usize) -> Self;
    pub fn allocate(&mut self, size: usize) -> *mut u8;
    pub fn deallocate(&mut self, ptr: *mut u8);
    pub fn reset(&mut self);
}
```

### Buffer Pool Management

```rust
pub struct BufferPool {
    pages: Vec<Page>,
    page_table: HashMap<PageId, usize>,
    lru_list: LinkedList<usize>,
    free_list: Vec<usize>,
}

impl BufferPool {
    pub fn get_page(&mut self, page_id: PageId) -> Result<&mut Page>;
    pub fn flush_page(&mut self, page_id: PageId) -> Result<()>;
    pub fn evict_pages(&mut self, count: usize) -> Result<()>;
}
```

---

# =============================================================================
# Query Processing Architecture
# =============================================================================

## SQL Query Engine

### Parser and Analyzer

```rust
pub struct QueryParser {
    lexer: SqlLexer,
    grammar: SqlGrammar,
}

pub struct QueryAnalyzer {
    catalog: SystemCatalog,
    type_checker: TypeChecker,
}

pub enum QueryPlan {
    // DML Operations
    Select(SelectPlan),
    Insert(InsertPlan),    // Supports VALUES and SELECT subqueries
    Update(UpdatePlan),
    Delete(DeletePlan),
    // DDL Operations
    CreateTable(CreateTablePlan),
    DropTable(DropTablePlan),
    AlterTable(AlterTablePlan),  // ADD/DROP/RENAME COLUMN, etc.
    CreateIndex(CreateIndexPlan),
    DropIndex(DropIndexPlan),
    // Transaction Control
    Begin, Commit, Rollback,
}
```

### Query Optimization

**Cost-Based Optimizer:**
- Statistics collection and maintenance
- Join order optimization
- Index selection and pushdown predicates (B-tree and Hash index aware)
- Parallel execution planning

**Rule-Based Transformations:**
- Common subexpression elimination
- Predicate pushdown and projection pruning
- Join reordering and elimination
- Constant folding and simplification

**Plan Cache (LRU):**
- 1024-entry LRU cache for compiled query plans
- Eliminates repeated parse/plan overhead for frequent queries
- Automatic invalidation on schema changes

### Execution Engine

```rust
pub trait QueryExecutor {
    fn execute(&mut self, plan: QueryPlan) -> Result<QueryResult>;
}

pub struct VectorizedExecutor {
    batch_size: usize,
    operators: Vec<Box<dyn Operator>>,
}

pub trait Operator {
    fn open(&mut self) -> Result<()>;
    fn next_batch(&mut self) -> Result<Option<Batch>>;
    fn close(&mut self) -> Result<()>;
}
```

### Direct Execution API

Bypass SQL parsing for programmatic updates using closure-based execution:

```rust
// Closure-based direct execution - no SQL parsing overhead
engine.execute_direct(|table| {
    table.update_where(
        |row| row.get("status") == "pending",
        |row| row.set("status", "processed"),
    )
});
```

### Query Safety Limits

| Parameter | Default | Description |
|-----------|---------|-------------|
| `max_result_rows` | 100,000 | Maximum rows returned per query |
| `query_timeout_secs` | 30 | Query execution timeout in seconds |

## Time Series Processing

### Time-Based Partitioning

```rust
pub struct TimePartition {
    start_time: DateTime<Utc>,
    end_time: DateTime<Utc>,
    storage_backend: Box<dyn StorageBackend>,
    compression_config: CompressionConfig,
}

pub struct TimeSeriesEngine {
    partitions: BTreeMap<DateTime<Utc>, TimePartition>,
    retention_policies: Vec<RetentionPolicy>,
    downsampling_rules: Vec<DownsamplingRule>,
}
```

### Aggregation Framework

```rust
pub enum Aggregation {
    Sum,
    Mean,
    Min,
    Max,
    Count,
    Percentile(f64),
    StdDev,
    Custom(Box<dyn AggregationFunction>),
}

pub struct TimeSeriesQuery {
    metric: String,
    time_range: TimeRange,
    aggregations: Vec<Aggregation>,
    group_by: Vec<String>,
    filters: Vec<Filter>,
}
```

## Document Processing

### JSON Processing Pipeline

```rust
pub struct DocumentProcessor {
    json_parser: JsonParser,
    schema_validator: SchemaValidator,
    indexer: DocumentIndexer,
}

pub struct JsonPath {
    segments: Vec<PathSegment>,
}

pub enum PathSegment {
    Field(String),
    ArrayIndex(usize),
    ArraySlice(Option<usize>, Option<usize>),
    Wildcard,
    RecursiveDescent,
}
```

### Full-Text Search

```rust
pub struct FullTextSearchEngine {
    tokenizer: Box<dyn Tokenizer>,
    stemmer: Box<dyn Stemmer>,
    inverted_index: InvertedIndex,
    scoring_algorithm: ScoringAlgorithm,
}

pub struct SearchQuery {
    terms: Vec<String>,
    phrases: Vec<String>,
    fuzzy_terms: Vec<FuzzyTerm>,
    filters: Vec<Filter>,
    boost_fields: HashMap<String, f64>,
}
```

## Graph Database

### Graph Storage and Traversal

Graph data is stored using adjacency lists for O(degree) traversal performance. The graph engine supports:

- **Nodes**: Typed vertices with arbitrary JSON properties
- **Edges**: Directed relationships with type labels and properties
- **Traversal**: BFS/DFS traversal with relationship-type filtering
- **Adjacency Lists**: O(degree) neighbor lookup

### Graph API

Accessible via REST endpoints at `/api/v1/graph/*`:
- Create, read, and delete nodes
- Create edges between nodes with typed relationships
- Query full graph data (nodes + edges)
- Adjacency list queries for neighbor discovery

## Real-Time Stream Processing

### Event Processing Architecture

```rust
pub struct StreamProcessor {
    input_streams: Vec<InputStream>,
    output_streams: Vec<OutputStream>,
    processing_topology: ProcessingTopology,
}

pub struct ProcessingTopology {
    nodes: Vec<ProcessingNode>,
    edges: Vec<Edge>,
}

pub enum ProcessingNode {
    Source(SourceNode),
    Transform(TransformNode),
    Filter(FilterNode),
    Aggregate(AggregateNode),
    Join(JoinNode),
    Sink(SinkNode),
}
```

### Change Data Capture (CDC)

```rust
pub struct CDCEngine {
    change_log: WriteAheadLog,
    subscribers: HashMap<String, Vec<Subscriber>>,
    filters: Vec<ChangeFilter>,
}

pub struct ChangeEvent {
    table: String,
    operation: Operation,
    old_row: Option<Row>,
    new_row: Option<Row>,
    timestamp: DateTime<Utc>,
    transaction_id: TransactionId,
}

pub enum Operation {
    Insert,
    Update,
    Delete,
    Truncate,
}
```

---

# =============================================================================
# Distributed Systems Architecture
# =============================================================================

## Consensus and Coordination

### Raft Consensus Implementation

```rust
pub struct RaftNode {
    id: NodeId,
    state: NodeState,
    log: ReplicatedLog,
    peers: HashMap<NodeId, PeerConnection>,
    election_timeout: Duration,
    heartbeat_interval: Duration,
}

pub enum NodeState {
    Follower,
    Candidate,
    Leader,
}

pub struct ReplicatedLog {
    entries: Vec<LogEntry>,
    commit_index: usize,
    last_applied: usize,
}
```

### Distributed Transaction Coordination

```rust
pub struct DistributedTransactionManager {
    local_transactions: HashMap<TransactionId, LocalTransaction>,
    coordinator: TransactionCoordinator,
    participants: Vec<NodeId>,
}

pub struct TwoPhaseCommit {
    transaction_id: TransactionId,
    participants: Vec<NodeId>,
    phase: CommitPhase,
    votes: HashMap<NodeId, Vote>,
}

pub enum CommitPhase {
    Prepare,
    Commit,
    Abort,
}
```

## Multi-Database Support

Each application gets its own isolated database instance with independent tables, KV store, documents, and graph data. Databases are auto-provisioned on first connection.

```
Cluster
  +-- Dashboard (port 9090, Leader)
  |     +-- tables, kv, documents, graph (isolated)
  +-- NexusScribe (port 9091, Follower)
  |     +-- tables, kv, documents, graph (isolated)
  +-- AxonML (port 7001, Follower)
        +-- tables, kv, documents, graph (isolated)
```

## Sharding and Partitioning

### Consistent Hashing

```rust
pub struct ConsistentHashRing {
    ring: BTreeMap<u64, NodeId>,
    virtual_nodes: u32,
    replication_factor: u32,
}

impl ConsistentHashRing {
    pub fn get_nodes(&self, key: &[u8]) -> Vec<NodeId>;
    pub fn add_node(&mut self, node: NodeId);
    pub fn remove_node(&mut self, node: NodeId);
    pub fn rebalance(&mut self) -> Vec<MigrationTask>;
}
```

### Shard Management

```rust
pub struct ShardManager {
    shards: HashMap<ShardId, Shard>,
    routing_table: RoutingTable,
    rebalancer: ShardRebalancer,
}

pub struct Shard {
    id: ShardId,
    key_range: KeyRange,
    replicas: Vec<NodeId>,
    primary: NodeId,
    state: ShardState,
}

pub enum ShardState {
    Active,
    Migrating,
    Splitting,
    Merging,
    Offline,
}
```

## Replication Strategy

### Multi-Master Replication

```rust
pub struct ReplicationEngine {
    replication_log: ReplicationLog,
    conflict_resolver: ConflictResolver,
    topology: ReplicationTopology,
}

pub enum ReplicationTopology {
    MasterSlave,
    MasterMaster,
    Chain,
    Star,
}

pub struct ConflictResolver {
    strategies: HashMap<String, ConflictResolutionStrategy>,
}

pub enum ConflictResolutionStrategy {
    LastWriteWins,
    FirstWriteWins,
    MergeValues,
    Custom(Box<dyn ConflictResolutionFunction>),
}
```

## OTA Rolling Updates (aegis-updates)

The `aegis-updates` crate provides zero-downtime rolling update capability for clustered deployments:

1. **Binary Download**: Fetch new release binary from configured update server
2. **Checksum Verification**: SHA-256 integrity check before deployment
3. **Follower-First Deploy**: Update follower nodes before the leader to maintain availability
4. **Health Check**: Verify each updated node passes health checks before proceeding
5. **Auto-Rollback**: Automatic rollback if health checks fail after update

Managed via `/api/v1/updates/*` REST endpoints (authenticated).

---

# =============================================================================
# API and SDK Architecture
# =============================================================================

## API Gateway Design

### Complete REST API Endpoints (aegis-server, port 9090)

**Health & Core:**
- `GET /health` - Health check
- `POST /api/v1/query` - Execute SQL query
- `GET /api/v1/tables` - List tables
- `GET /api/v1/tables/:name` - Get table info
- `GET /api/v1/metrics` - Server metrics

**Authentication API:**
- `POST /api/v1/auth/login` - User login (rate limited: 30/min)
- `POST /api/v1/auth/mfa/verify` - MFA verification (rate limited: 30/min)
- `POST /api/v1/auth/logout` - User logout
- `GET /api/v1/auth/session` - Validate session
- `GET /api/v1/auth/me` - Current user info

**Admin API (authenticated):**
- `GET /api/v1/admin/cluster` - Cluster information
- `GET /api/v1/admin/dashboard` - Dashboard summary
- `GET /api/v1/admin/nodes` - Node list with metrics
- `POST /api/v1/admin/nodes/:node_id/restart` - Restart a node
- `POST /api/v1/admin/nodes/:node_id/drain` - Drain a node
- `GET /api/v1/admin/nodes/:node_id/logs` - Get node logs
- `DELETE /api/v1/admin/nodes/:node_id` - Remove a node
- `GET /api/v1/admin/storage` - Storage information
- `GET /api/v1/admin/stats` - Query statistics
- `GET /api/v1/admin/database` - Database statistics
- `GET /api/v1/admin/alerts` - Active alerts
- `GET /api/v1/admin/activities` - Recent activity log
- `GET /api/v1/admin/settings` - Get server settings
- `PUT /api/v1/admin/settings` - Update server settings
- `GET /api/v1/admin/users` - List users
- `POST /api/v1/admin/users` - Create user
- `PUT /api/v1/admin/users/:username` - Update user
- `DELETE /api/v1/admin/users/:username` - Delete user
- `GET /api/v1/admin/roles` - List roles
- `POST /api/v1/admin/roles` - Create role
- `DELETE /api/v1/admin/roles/:name` - Delete role
- `POST /api/v1/admin/metrics/timeseries` - Metrics time series data
- `POST /api/v1/admin/backup` - Create backup
- `GET /api/v1/admin/backups` - List backups
- `POST /api/v1/admin/restore` - Restore backup
- `DELETE /api/v1/admin/backup/:id` - Delete backup

**Key-Value Store API:**
- `GET /api/v1/kv/keys` - List all keys
- `POST /api/v1/kv/keys` - Create/update key
- `GET /api/v1/kv/keys/:key` - Get key value
- `DELETE /api/v1/kv/keys/:key` - Delete key

**Document Store API:**
- `GET /api/v1/documents/collections` - List collections
- `POST /api/v1/documents/collections` - Create collection
- `GET /api/v1/documents/collections/:name` - Get collection documents
- `GET /api/v1/documents/collections/:name/documents` - List documents
- `POST /api/v1/documents/collections/:name/documents` - Insert document
- `GET /api/v1/documents/collections/:name/documents/:id` - Get document
- `PUT /api/v1/documents/collections/:name/documents/:id` - Update document
- `PATCH /api/v1/documents/collections/:name/documents/:id` - Patch document
- `DELETE /api/v1/documents/collections/:name/documents/:id` - Delete document
- `POST /api/v1/documents/collections/:name/query` - Query documents

**Time Series API:**
- `GET /api/v1/timeseries/metrics` - List metrics
- `POST /api/v1/timeseries/metrics` - Register metric
- `POST /api/v1/timeseries/write` - Write time series data
- `POST /api/v1/timeseries/query` - Query time series data

**Streaming API:**
- `GET /api/v1/streaming/channels` - List channels
- `POST /api/v1/streaming/channels` - Create channel
- `POST /api/v1/streaming/publish` - Publish event
- `GET /api/v1/streaming/channels/:channel/history` - Channel history

**Graph Database API:**
- `GET /api/v1/graph/data` - Get nodes and edges
- `POST /api/v1/graph/nodes` - Create graph node
- `DELETE /api/v1/graph/nodes/:node_id` - Delete graph node
- `POST /api/v1/graph/edges` - Create graph edge

**Query Builder API:**
- `POST /api/v1/query-builder/execute` - Execute builder query

**OTA Update API (authenticated):**
- `GET /api/v1/updates/version` - Get current version
- `POST /api/v1/updates/plan` - Create update plan
- `POST /api/v1/updates/execute` - Execute update plan
- `GET /api/v1/updates/status/:plan_id` - Get update status
- `GET /api/v1/updates/history` - List update history

**Bulk Import API:**
- `POST /api/v1/import/sql` - Bulk import rows into SQL table (CSV or JSON)
- `POST /api/v1/import/documents` - Bulk import documents into collection (JSON)
- `POST /api/v1/import/kv` - Bulk import key-value pairs (JSON)

**GDPR/CCPA Compliance API (authenticated):**
- `DELETE /api/v1/compliance/data-subject/:identifier` - Delete data subject (GDPR Art. 17 right to erasure)
- `POST /api/v1/compliance/export` - Export data subject data (GDPR Art. 20 portability)
- `GET /api/v1/compliance/certificates` - List deletion certificates
- `GET /api/v1/compliance/certificates/:cert_id` - Get deletion certificate
- `GET /api/v1/compliance/certificates/:cert_id/verify` - Verify deletion certificate
- `GET /api/v1/compliance/audit/:subject_id` - Get deletion audit trail
- `GET /api/v1/compliance/audit/verify` - Verify audit integrity
- `POST /api/v1/compliance/consent` - Record consent
- `GET /api/v1/compliance/consent/stats` - Get consent statistics
- `GET /api/v1/compliance/consent/:subject_id` - Get consent status
- `DELETE /api/v1/compliance/consent/:subject_id` - Delete consent data
- `GET /api/v1/compliance/consent/:subject_id/history` - Consent history
- `GET /api/v1/compliance/consent/:subject_id/export` - Export consent data
- `GET /api/v1/compliance/consent/:subject_id/check/:purpose` - Check consent for purpose
- `DELETE /api/v1/compliance/consent/:subject_id/:purpose` - Withdraw consent
- `GET /api/v1/compliance/do-not-sell` - CCPA Do Not Sell list
- `GET /api/v1/compliance/breaches` - List breaches
- `GET /api/v1/compliance/breaches/stats` - Breach statistics
- `POST /api/v1/compliance/breaches/cleanup` - Trigger breach cleanup
- `GET /api/v1/compliance/breaches/:id` - Get breach details
- `POST /api/v1/compliance/breaches/:id/acknowledge` - Acknowledge breach
- `POST /api/v1/compliance/breaches/:id/resolve` - Resolve breach
- `GET /api/v1/compliance/breaches/:id/report` - Get breach report
- `GET /api/v1/compliance/security-events` - List security events

**Cluster Peer Management (internal):**
- `GET /api/v1/cluster/info` - Node info
- `POST /api/v1/cluster/join` - Join cluster
- `POST /api/v1/cluster/heartbeat` - Heartbeat
- `GET /api/v1/cluster/peers` - List peers
- `POST /api/v1/cluster/shutdown` - Graceful shutdown

### Rate Limiting and Throttling

```rust
pub struct RateLimiter {
    entries: Arc<RwLock<HashMap<String, RateLimitEntry>>>,
    max_requests: u32,
    window_secs: u64,
}

pub struct RateLimitEntry {
    count: u32,
    window_start: Instant,
}

// Token bucket algorithm implementation
// Default limits:
// - Login endpoints: 30 requests/minute/IP
// - General API: 1000 requests/minute/IP
```

## SDK Architecture

### Core SDK Design

```rust
pub struct AegisClient {
    connection: Connection,
    config: ClientConfig,
    query_cache: QueryCache,
    connection_pool: ConnectionPool,
}

impl AegisClient {
    pub async fn connect(config: ClientConfig) -> Result<Self>;
    pub async fn execute_query(&self, query: &str) -> Result<QueryResult>;
    pub async fn begin_transaction(&self) -> Result<Transaction>;
    pub async fn stream_changes(&self, filter: ChangeFilter) -> Result<ChangeStream>;
}
```

### Language-Specific Bindings

**Python Bindings (PyO3):**
```python
# Python interface example
import aegis

client = await aegis.connect("aegis://localhost:9090/mydb")
result = await client.execute("SELECT * FROM users WHERE age > 18")
async for row in result:
    print(row)
```

**JavaScript Bindings (NAPI):**
```javascript
// Node.js interface example
const aegis = require('aegis-db');

const client = await aegis.connect('aegis://localhost:9090/mydb');
const result = await client.execute('SELECT * FROM users WHERE age > 18');
for await (const row of result) {
    console.log(row);
}
```

### Connection Management

```rust
pub struct ConnectionPool {
    connections: Vec<Connection>,
    available: VecDeque<usize>,
    max_connections: usize,
    idle_timeout: Duration,
}

pub struct Connection {
    socket: TcpStream,
    state: ConnectionState,
    last_activity: Instant,
    transaction: Option<TransactionId>,
}

pub enum ConnectionState {
    Idle,
    Active,
    InTransaction,
    Broken,
}
```

---

# =============================================================================
# Security Architecture
# =============================================================================

## Authentication System

### Password Security

Passwords are hashed using Argon2id, the winner of the Password Hashing Competition:

```rust
// Argon2id configuration for production security
const ARGON2_MEMORY_COST: u32 = 19_456; // 19 MiB
const ARGON2_TIME_COST: u32 = 2;        // 2 iterations
const ARGON2_PARALLELISM: u32 = 1;      // 1 thread
const ARGON2_OUTPUT_LEN: usize = 32;    // 256-bit hash

pub fn hash_password(password: &str) -> Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::new(
        Algorithm::Argon2id,
        Version::V0x13,
        Params::new(ARGON2_MEMORY_COST, ARGON2_TIME_COST, ARGON2_PARALLELISM, Some(ARGON2_OUTPUT_LEN))
    );
    Ok(argon2.hash_password(password.as_bytes(), &salt)?.to_string())
}
```

### Multi-Factor Authentication

```rust
pub struct AuthenticationManager {
    providers: HashMap<String, Box<dyn AuthProvider>>,
    mfa_handlers: HashMap<String, Box<dyn MFAHandler>>,
    session_manager: SessionManager,
}

pub trait AuthProvider {
    fn authenticate(&self, credentials: Credentials) -> Result<User>;
    fn supports_mfa(&self) -> bool;
}

pub struct SessionManager {
    active_sessions: HashMap<SessionId, Session>,
    session_store: Box<dyn SessionStore>,
    expiry_queue: BinaryHeap<SessionExpiry>,
}
```

### Secrets Management (HashiCorp Vault)

```rust
pub trait SecretsProvider: Send + Sync {
    fn get(&self, key: &str) -> Option<String>;
    fn get_or(&self, key: &str, default: &str) -> String;
    fn exists(&self, key: &str) -> bool;
}

pub struct VaultSecretsProvider {
    config: VaultConfig,
    client: reqwest::Client,
    token: RwLock<Option<String>>,
    cache: RwLock<HashMap<String, String>>,
}

pub struct SecretsManager {
    providers: Vec<Box<dyn SecretsProvider>>, // Vault -> Env -> Default
}

// Supported Vault authentication methods:
// - Token-based (VAULT_TOKEN)
// - AppRole (VAULT_ROLE_ID + VAULT_SECRET_ID)
// - Kubernetes (VAULT_KUBERNETES_ROLE)
```

### Authorization Framework

```rust
pub struct AuthorizationEngine {
    policies: Vec<Policy>,
    role_hierarchy: RoleHierarchy,
    attribute_store: AttributeStore,
}

pub struct Policy {
    id: String,
    effect: Effect,
    actions: Vec<String>,
    resources: Vec<ResourcePattern>,
    conditions: Vec<Condition>,
}

pub enum Effect {
    Allow,
    Deny,
}
```

## PHI/Data Classification

Aegis supports a 6-level data classification system for regulatory compliance:

| Level | Classification | Description |
|-------|---------------|-------------|
| 1 | Public | Freely distributable data |
| 2 | Internal | Internal-use only |
| 3 | Confidential | Business-sensitive data |
| 4 | PHI | Protected Health Information (HIPAA) |
| 5 | PII | Personally Identifiable Information |
| 6 | Restricted | Highest security classification |

Classification levels drive access control policies, audit logging verbosity, encryption requirements, and retention rules.

## Compliance Framework

### GDPR Right to Erasure (Article 17)

- Full data subject deletion across all paradigms (SQL, KV, documents, graph)
- Cryptographic deletion certificates with SHA-256 verification
- Complete audit trail with integrity verification
- Data portability export (Article 20)

### Consent Management

- 11 consent purposes with full audit trail
- Per-subject consent status tracking and history
- Consent withdrawal and data deletion
- CCPA Do Not Sell list management

### Breach Detection and Notification

- Real-time security event monitoring
- Automated breach detection with configurable thresholds
- Breach acknowledgment and resolution workflow
- Compliance reporting (HIPAA 72-hour, GDPR 72-hour notification requirements)
- Webhook notifications for security events

## Encryption and Key Management

### Data Encryption

```rust
pub struct EncryptionManager {
    key_store: Box<dyn KeyStore>,
    encryption_algorithms: HashMap<String, Box<dyn EncryptionAlgorithm>>,
    key_rotation_policy: KeyRotationPolicy,
}

pub trait EncryptionAlgorithm {
    fn encrypt(&self, data: &[u8], key: &Key) -> Result<Vec<u8>>;
    fn decrypt(&self, data: &[u8], key: &Key) -> Result<Vec<u8>>;
}

pub struct KeyRotationPolicy {
    rotation_period: Duration,
    key_versions_to_keep: u32,
    automatic_rotation: bool,
}
```

### Network Security

```rust
pub struct NetworkSecurity {
    tls_config: TlsConfig,
    firewall_rules: Vec<FirewallRule>,
    ddos_protection: DdosProtection,
    rate_limiter: RateLimiter,
}

pub struct TlsConfig {
    certificates: Vec<Certificate>,
    cipher_suites: Vec<CipherSuite>,
    protocol_versions: Vec<TlsVersion>,
    client_auth: ClientAuthMode,
}

// Native TLS support via axum-server with rustls
// Supports TLSv1.2 and TLSv1.3
// Modern cipher suites: ECDHE-RSA-AES128-GCM-SHA256, etc.
```

---

# =============================================================================
# Monitoring and Observability
# =============================================================================

## Metrics Collection

### System Metrics

```rust
pub struct MetricsCollector {
    collectors: Vec<Box<dyn MetricCollector>>,
    exporters: Vec<Box<dyn MetricExporter>>,
    aggregation_rules: Vec<AggregationRule>,
}

pub trait MetricCollector {
    fn collect(&self) -> Vec<Metric>;
    fn name(&self) -> &str;
}

pub struct SystemMetrics {
    cpu_usage: Gauge,
    memory_usage: Gauge,
    disk_io: Counter,
    network_io: Counter,
    query_latency: Histogram,
    query_throughput: Counter,
}
```

### Distributed Tracing

```rust
pub struct TracingSystem {
    tracer: Box<dyn Tracer>,
    span_processor: SpanProcessor,
    samplers: Vec<Box<dyn Sampler>>,
}

pub struct Span {
    trace_id: TraceId,
    span_id: SpanId,
    parent_span_id: Option<SpanId>,
    operation_name: String,
    start_time: Instant,
    end_time: Option<Instant>,
    tags: HashMap<String, String>,
    logs: Vec<LogEntry>,
}
```

### Log Rotation

Server logs use daily rotating files via `tracing-appender`:

```rust
// Daily rotating log files
let file_appender = tracing_appender::rolling::daily("/var/log/aegis", "aegis-server.log");
```

## Alerting System

### Alert Management

```rust
pub struct AlertManager {
    rules: Vec<AlertRule>,
    notification_channels: HashMap<String, Box<dyn NotificationChannel>>,
    alert_state: HashMap<String, AlertState>,
}

pub struct AlertRule {
    name: String,
    condition: AlertCondition,
    severity: Severity,
    notification_channels: Vec<String>,
    cooldown_period: Duration,
}

pub enum AlertCondition {
    Threshold(ThresholdCondition),
    Anomaly(AnomalyCondition),
    Composite(CompositeCondition),
}
```

---

# =============================================================================
# Performance Benchmarks
# =============================================================================

## Benchmark Results

| Benchmark | Result | Notes |
|-----------|--------|-------|
| Fund Transfer TPS (0% contention) | 758,000 TPS | MVCC transaction throughput |
| High Contention TPS | 2,500,000 TPS | Concurrent transaction processing |
| KV Read Throughput | 12,300,000 reads/sec | In-memory key-value lookups |
| SQL Insert Throughput | 223,000 inserts/sec | Batched row insertion |
| KV over HTTP | 203,000 ops/sec | Key-value operations via REST API |

## Storage Optimization

### Compression Algorithms

Three compression algorithms are supported, selectable per block or globally:

| Algorithm | Use Case | Ratio | Speed |
|-----------|----------|-------|-------|
| LZ4 | Default, balanced speed/ratio | Moderate | Very Fast |
| Zstd | Archival, cold data | High | Moderate |
| Snappy | Hot data, low latency | Lower | Fastest |

```rust
pub enum CompressionType {
    None,
    Lz4,
    Zstd,
    Snappy,
}

pub trait CompressionAlgorithm {
    fn compress(&self, data: &[u8]) -> Result<Vec<u8>>;
    fn decompress(&self, data: &[u8]) -> Result<Vec<u8>>;
    fn compression_ratio(&self) -> f64;
}
```

### VACUUM/Compaction

Dead row removal and index rebuild for reclaiming storage space:

```
POST /api/v1/admin/vacuum
```

Operations performed:
- Remove rows marked as deleted by MVCC
- Rebuild fragmented indexes
- Reclaim disk space from deleted blocks
- Update storage statistics

---

# =============================================================================
# Deployment Architecture
# =============================================================================

## Cloud Deployment

### Kubernetes Integration

```yaml
# Example Kubernetes deployment
apiVersion: apps/v1
kind: StatefulSet
metadata:
  name: aegis-cluster
spec:
  serviceName: aegis
  replicas: 3
  selector:
    matchLabels:
      app: aegis
  template:
    metadata:
      labels:
        app: aegis
    spec:
      containers:
      - name: aegis
        image: aegis:latest
        ports:
        - containerPort: 9090
        env:
        - name: AEGIS_NODE_ID
          valueFrom:
            fieldRef:
              fieldPath: metadata.name
        volumeMounts:
        - name: data
          mountPath: /data
  volumeClaimTemplates:
  - metadata:
      name: data
    spec:
      accessModes: ["ReadWriteOnce"]
      resources:
        requests:
          storage: 100Gi
```

### Auto-scaling Configuration

```rust
pub struct AutoScaler {
    scaling_policies: Vec<ScalingPolicy>,
    metrics_source: Box<dyn MetricsSource>,
    cluster_manager: ClusterManager,
}

pub struct ScalingPolicy {
    metric: String,
    target_value: f64,
    scale_up_threshold: f64,
    scale_down_threshold: f64,
    cooldown_period: Duration,
}
```

## On-Premise Deployment

### Installation Package

```rust
pub struct Installer {
    config_templates: HashMap<String, ConfigTemplate>,
    system_requirements: SystemRequirements,
    dependency_checker: DependencyChecker,
}

pub struct SystemRequirements {
    min_cpu_cores: u32,
    min_memory_gb: u32,
    min_storage_gb: u64,
    supported_os: Vec<OperatingSystem>,
    required_ports: Vec<u16>,   // Default: [9090]
}
```

### Configuration Management

```toml
# aegis.toml configuration example
[cluster]
node_id = "node-001"
bind_address = "0.0.0.0:9090"
advertise_address = "192.168.1.100:9090"
data_directory = "/var/lib/aegis"

[cluster.peers]
"node-002" = "192.168.1.101:9090"
"node-003" = "192.168.1.102:9090"

[storage]
backend = "local"
compression = "lz4"          # lz4, zstd, or snappy
encryption_at_rest = true
backup_directory = "/var/lib/aegis/backups"

[security]
tls_enabled = true
certificate_file = "/etc/aegis/tls.crt"
private_key_file = "/etc/aegis/tls.key"
auth_method = "ldap"

[performance]
max_connections = 1000
query_cache_size = "1GB"
buffer_pool_size = "4GB"
worker_threads = 8
max_result_rows = 100000
query_timeout_secs = 30
```

## Embedded/Edge Deployment

### Lightweight Runtime

```rust
pub struct EmbeddedRuntime {
    storage_backend: EmbeddedStorage,
    query_engine: LightweightQueryEngine,
    sync_manager: SyncManager,
    resource_limits: ResourceLimits,
}

pub struct ResourceLimits {
    max_memory_usage: usize,
    max_storage_usage: usize,
    max_cpu_usage: f64,
    network_bandwidth_limit: Option<usize>,
}
```

### Synchronization with Cloud

```rust
pub struct SyncManager {
    sync_policies: Vec<SyncPolicy>,
    conflict_resolver: ConflictResolver,
    network_monitor: NetworkMonitor,
}

pub struct SyncPolicy {
    tables: Vec<String>,
    sync_interval: Duration,
    sync_direction: SyncDirection,
    bandwidth_limit: Option<usize>,
}

pub enum SyncDirection {
    UploadOnly,
    DownloadOnly,
    Bidirectional,
}
```

---

# =============================================================================
# Development and Testing Strategy
# =============================================================================

## Testing Architecture

### Test Distribution by Crate

| Crate | Tests | Category |
|-------|-------|----------|
| aegis-replication | 165 | Raft, sharding, CRDTs, 2PC |
| aegis-server (unit) | 116 | Handlers, middleware, auth |
| aegis-server (integration) | 54 | E2E API tests |
| aegis-storage | 48 | Backends, WAL, MVCC |
| aegis-client | 42 | SDK, connection management |
| aegis-query | 41 | Parser, planner, executor, indexes |
| aegis-document | 36 | Collections, JSONPath, search |
| aegis-timeseries | 36 | Gorilla compression, aggregation |
| aegis-monitoring | 35 | Metrics, tracing, alerts |
| aegis-streaming | 31 | Channels, pub/sub, CDC |
| aegis-updates | 14 | OTA updates, rollback |
| aegis-memory | 5 | Arena allocator, buffer pool |
| aegis-common | 4 | Shared types, errors |
| **Total** | **634** | |

### Multi-Level Testing

```rust
pub struct TestSuite {
    unit_tests: Vec<UnitTest>,
    integration_tests: Vec<IntegrationTest>,
    performance_tests: Vec<PerformanceTest>,
    chaos_tests: Vec<ChaosTest>,
}

pub struct ChaosTest {
    name: String,
    failure_scenarios: Vec<FailureScenario>,
    expected_behavior: ExpectedBehavior,
    recovery_criteria: RecoveryCriteria,
}
```

### Performance Benchmarking

```rust
pub struct BenchmarkSuite {
    workloads: Vec<Workload>,
    environments: Vec<TestEnvironment>,
    metrics: Vec<PerformanceMetric>,
}

pub struct Workload {
    name: String,
    query_patterns: Vec<QueryPattern>,
    data_size: usize,
    concurrency_level: usize,
    duration: Duration,
}
```

## Development Tools

### Database Schema Migration

```rust
pub struct MigrationManager {
    migrations: Vec<Migration>,
    schema_history: SchemaHistory,
    rollback_strategies: HashMap<String, RollbackStrategy>,
}

pub struct Migration {
    version: String,
    up_script: String,
    down_script: String,
    dependencies: Vec<String>,
}
```

---

## Crate Dependency Graph

```
aegis-dashboard (WASM, built separately with Trunk)
aegis-cli
aegis-server (REST API - Axum on port 9090)
    |
    v
aegis-client
aegis-query (SQL parser/planner/executor, B-tree/Hash indexes, plan cache)
aegis-timeseries, aegis-document, aegis-streaming
aegis-replication (Raft, sharding, 2PC, CRDTs)
aegis-updates (OTA rolling updates)
aegis-monitoring
    |
    v
aegis-storage (backends, WAL, MVCC, LZ4/Zstd/Snappy compression)
aegis-memory (arena allocators, buffer pool)
    |
    v
aegis-common (shared types, errors, config)
```

---

**Architecture Status**: v2.2.0 - All Phases Complete (Phase 1-7)
**Production**: 50+ edge nodes (Raspberry Pi 4/5) + central NexusBMS server
**Maintainers**: Andrew Jewell Sr / AutomataNexus LLC

*This document provides the comprehensive technical architecture for Aegis database platform. All implementation should follow this architectural specification and maintain consistency with the design principles outlined herein.*
