# Aegis-DB Benchmark Results

## System Specifications

| Spec | Value |
|------|-------|
| CPU | Intel Core Ultra 9 275HX (24 threads) |
| RAM | 55 GB |
| OS | WSL2 (Linux 6.6.87.2-microsoft-standard-WSL2) |
| Rust | 1.92.0 |
| Aegis-DB | v0.2.2 |

---

## Engine-Level Results (Criterion)

Direct engine calls, no HTTP/network overhead. This is the fair comparison against SpacetimeDB
since SpacetimeDB runs application logic in-process as WASM modules.

Run with: `cargo bench -p aegis-server`

### SQL Insert Throughput

| Workload | Time/op | Rows/sec |
|----------|---------|----------|
| Single row insert | 4.48 μs | **223,410/s** |
| Batch 10 rows | 53.6 μs | **186,640/s** |
| Batch 100 rows | 528.9 μs | **189,070/s** |
| Batch 1000 rows | 5.12 ms | **195,310/s** |

### SQL Read Throughput

| Workload | Time/op | Queries/sec |
|----------|---------|-------------|
| Point query (10k rows) | 2.25 ms | **444/s** |
| Full scan (10k rows) | 3.70 ms | **270/s** |
| Filtered scan (10k rows) | 3.10 ms | **323/s** |

### Fund Transfer (SpacetimeDB Comparison)

| Workload | Aegis-DB (TPS) | SpacetimeDB (TPS) | Ratio |
|----------|----------------|--------------------|-------|
| 0% contention (100k accounts) | **758,000** | 107,850 | **7.03x** |
| High contention / Zipf (100k accounts) | **2,496,000** | 103,590 | **24.1x** |

> Ultra-fast indexed UPDATE path with closure-based value computation.
> Combined find+lookup in single B-tree lock acquisition. Pre-resolved column
> indices, zero expression evaluation overhead. Each transfer = 2 indexed UPDATEs,
> all O(log N). Executor and context cached across iterations.
> No SQL parsing, no plan construction, no predicate extraction.

### KV Operations

| Workload | Time/op | Ops/sec |
|----------|---------|---------|
| Set 64B | 252 ns | **3,970,000/s** |
| Set 1KB | 456 ns | **2,195,000/s** |
| Set 16KB | 2.46 μs | **407,000/s** |
| Get 64B | 81 ns | **12,350,000/s** |
| Get 1KB | 136 ns | **7,350,000/s** |
| Get 16KB | 1.53 μs | **655,000/s** |
| Delete | 376 ns | **2,657,000/s** |

### Concurrent Mixed (80% Read / 20% Write, 1k rows, 100 ops/task)

| Concurrency | Time/batch | Total Ops/sec |
|-------------|------------|---------------|
| 1 task | 9.80 ms | **10,200/s** |
| 10 tasks | 22.5 ms | **44,500/s** |
| 50 tasks | 104.2 ms | **48,000/s** |
| 100 tasks | 208.9 ms | **47,900/s** |

---

## HTTP API Results (50 concurrent connections, 10s duration)

Real-world API performance including network stack, JSON serialization, and middleware.

Run with: `cargo run --release -p aegis-benchmarks -- --concurrency 50 --duration 10`

| Benchmark | Ops/sec | Avg (μs) | P50 (μs) | P95 (μs) | P99 (μs) |
|-----------|---------|----------|----------|----------|----------|
| SQL Insert | **80,450** | 620 | 638 | 928 | 1,275 |
| SQL Read | **40,496** | 1,234 | 1,275 | 2,044 | 2,601 |
| Fund Transfer | **586** | 85,641 | 85,026 | 112,746 | 126,511 |
| KV Get | **203,117** | 245 | 225 | 437 | 600 |
| Mixed 80/20 | **23,868** | 2,094 | 1,987 | 3,703 | 4,752 |

---

## SpacetimeDB Reference Numbers

Source: [SpacetimeDB Keynote-2 Benchmark](https://spacetimedb.com/blog/series/benchmarks)

| Workload | SpacetimeDB TPS | Notes |
|----------|-----------------|-------|
| Fund transfer, 0% contention | 107,850 | Single-threaded WASM module, in-process |
| Fund transfer, 80% contention | 103,590 | Zipf distribution over 100k accounts |
| Rust module throughput | ~170,000 | Best case, in-memory, no network |

### Methodology Differences

| Factor | Aegis-DB (Engine) | SpacetimeDB |
|--------|-------------------|-------------|
| Execution model | Closure-based direct Rust calls (no SQL, no expression eval) | WASM modules in-process |
| Storage | In-memory HashMap + optional disk | In-memory (committed log) |
| Transaction isolation | Per-query (no explicit BEGIN/COMMIT yet) | Serializable |
| Network overhead | None (engine-level) / Axum HTTP (API) | None (in-process) |
| Concurrency model | std::sync::RwLock per DB | Serializable transactions |
| Index support | B-tree indexes (SELECT, UPDATE, DELETE) | B-tree indexes |
| Plan caching | LRU cache (1024 entries) for SQL path | N/A (compiled WASM) |
| Column expressions | `SET col = col + expr` (server-side) | Native WASM expressions |
| Index-aware updates | Skip index maintenance for non-indexed cols | Built-in |
| Arithmetic | Native integer preservation (no float coercion) | Native types |
| Lock acquisitions | 4 per transfer (combined find+lookup) | N/A (serialized) |

### Key Observations

- **Fund transfer dominates SpacetimeDB**: 758K TPS zero contention (7.03x), 2.5M TPS high contention (24.1x). Closure-based direct execution API with combined find+lookup eliminates all parsing and expression evaluation overhead.
- **KV is blazing fast**: 12.3M reads/sec and 4M writes/sec at the engine level; 203K reads/sec over HTTP
- **SQL inserts are strong**: ~223K single-row inserts/sec engine-level; 80K/sec over HTTP
- **Concurrent throughput scales well**: Mixed workload peaks at ~48K ops/sec with 50 tasks (1k row table), showing good lock contention behavior
- **HTTP overhead is reasonable**: ~620μs avg for inserts, ~245μs avg for KV gets — the Axum stack adds modest latency
- SpacetimeDB runs application logic _inside_ the database as WASM, eliminating all network overhead
- Aegis-DB direct execution API is the closest comparison (both are in-process, no SQL parsing)

### Optimization Opportunities (Completed)

1. ~~**B-tree indexes**~~ — **Done.** Index-accelerated SELECT, UPDATE, and DELETE.
2. ~~**Prepared statement cache**~~ — **Done.** LRU plan cache skips re-parsing for repeated SQL.
3. ~~**UPDATE with column expressions**~~ — **Done.** `SET balance = balance - 1` evaluated server-side.
4. ~~**Direct execution API**~~ — **Done.** `execute_update_direct()` bypasses SQL parsing entirely. 27 TPS → 435K TPS (16,100x improvement).
5. ~~**Skip index maintenance**~~ — **Done.** When updated columns aren't indexed, skip index remove/insert.
6. ~~**Integer arithmetic**~~ — **Done.** Preserve native Integer type for int+int operations (was converting to Float).
7. ~~**Combined find+lookup**~~ — **Done.** `find_and_lookup_first()` does index search + B-tree lookup in single lock acquisition. 18 → 4 RwLock acquisitions per transfer.
8. ~~**Closure-based UPDATE**~~ — **Done.** `execute_update_indexed_fn()` with pre-resolved column indices. Zero expression evaluation overhead. 435K → 758K TPS (zero contention), 824K → 2.5M TPS (high contention).

### Multi-Paradigm Optimizations (Completed)

1. ~~**Graph adjacency lists**~~ — **Done.** O(degree) traversal instead of O(E). Label index, relationship index, batch operations.
2. ~~**Document index-accelerated queries**~~ — **Done.** `find()` routes Eq filters to hash/btree indexes for O(1) lookup. Sort, skip, limit, projection support.
3. ~~**TimeSeries atomic stats**~~ — **Done.** Replaced `RwLock<EngineStats>` with atomic counters. Lazy decompression with timestamp bounds.
4. ~~**Streaming atomic stats**~~ — **Done.** Replaced stats lock with atomics. Single-lock `publish_to_many()`.

### Remaining Opportunities

1. **Batch/pipeline API** — Allow multiple statements in a single request
2. **Connection pooling** in HTTP load test for fairer API comparison
3. **TimeSeries sharded writes** — Partition series_data HashMap for parallel ingestion
4. **Graph property indexing** — Index node/edge properties for fast filtered traversals

---

## How to Reproduce

```bash
# Engine-level benchmarks (Criterion)
cargo bench -p aegis-server

# View detailed HTML report
open target/criterion/report/index.html

# HTTP load test (requires running server)
cargo run --release -p aegis-server &
sleep 2
cargo run --release -p aegis-benchmarks -- --concurrency 50 --duration 10

# Run specific benchmark only
cargo run --release -p aegis-benchmarks -- --benchmark transfer --concurrency 100
```
