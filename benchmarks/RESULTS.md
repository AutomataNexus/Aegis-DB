# Aegis-DB Benchmark Results

## System Specifications

| Spec | Value |
|------|-------|
| CPU | Intel Core Ultra 9 275HX (24 threads) |
| RAM | 48 GB |
| OS | WSL2 (Linux 6.6.87.2-microsoft-standard-WSL2) |
| Rust | 1.95.0 |
| Aegis-DB | v0.3.1 |

> All numbers below were re-measured on **v0.3.1** (2026-06-13). The engine-level
> fund transfer uses `Executor::execute_transfer_indexed` via
> `QueryEngine::get_executor` — a real atomic transfer (read both balances →
> verify sufficient funds → debit → credit) performed under a single held table
> write lock, so it is atomic and isolated, not a pair of blind writes.

---

## Engine-Level Results (Criterion)

Direct engine calls, no HTTP/network overhead. This is the fair comparison against SpacetimeDB
since SpacetimeDB runs application logic in-process as WASM modules.

Run with: `cargo bench -p aegis-server`

### SQL Insert Throughput

| Workload | Time/op | Rows/sec |
|----------|---------|----------|
| Single row insert | 4.91 μs | **203,700/s** |
| Batch 10 rows | 57.0 μs | **175,600/s** |
| Batch 100 rows | 607.0 μs | **164,700/s** |
| Batch 1000 rows | 5.96 ms | **167,800/s** |

### SQL Read Throughput

| Workload | Time/op | Queries/sec |
|----------|---------|-------------|
| Point query (10k rows) | 2.99 ms | **335/s** |
| Full scan (10k rows) | 4.89 ms | **205/s** |
| Filtered scan (10k rows) | 3.43 ms | **292/s** |

### Fund Transfer (SpacetimeDB Comparison)

| Workload | Aegis-DB (TPS) | SpacetimeDB (TPS) | Ratio |
|----------|----------------|--------------------|-------|
| 0% contention (100k accounts) | **971,000** | 107,850 | **9.0x** |
| High contention / Zipf (100k accounts) | **2,542,000** | 103,590 | **24.5x** |

> Each transfer does the full transactional work — look up sender and receiver by
> the `id` index (O(1) hash / O(log N) B-tree), **read both balances, verify the
> sender has sufficient funds, then debit and credit** — all while holding the
> table's write lock, so the operation is atomic and isolated (no other writer can
> observe or interleave a partial transfer). No SQL parsing, no plan construction.
> Accounts are seeded with a non-drainable balance so every measured transfer is a
> real committed transfer, not a rejected one. High-contention is faster because
> the hot accounts stay resident in CPU cache. This is the honest analog of an
> in-process SpacetimeDB transfer reducer (compiled logic, in-memory, atomic).

### KV Operations

| Workload | Time/op | Ops/sec |
|----------|---------|---------|
| Set 64B | 244 ns | **4,095,000/s** |
| Set 1KB | 385 ns | **2,595,000/s** |
| Set 16KB | 1.85 μs | **540,000/s** |
| Get 64B | 92.9 ns | **10,762,000/s** |
| Get 1KB | 153 ns | **6,523,000/s** |
| Get 16KB | 1.52 μs | **657,000/s** |
| Delete | 36.3 ns | **27,557,000/s** |

### Concurrent Mixed (80% Read / 20% Write, 1k rows, 100 ops/task)

| Concurrency | Time/batch | Total Ops/sec |
|-------------|------------|---------------|
| 1 task | 505 μs | **198,000/s** |
| 10 tasks | 1.03 ms | **971,000/s** |
| 50 tasks | 3.19 ms | **1,567,000/s** |
| 100 tasks | 3.03 ms | **3,297,000/s** |

---

## HTTP API Results (50 concurrent connections, 10s duration)

Real-world API performance including network stack, JSON serialization, and middleware.

Run with: `cargo run --release -p aegis-benchmarks -- --concurrency 50 --duration 10`

| Benchmark | Ops/sec | Avg (μs) | P50 (μs) | P95 (μs) | P99 (μs) |
|-----------|---------|----------|----------|----------|----------|
| SQL Insert | **72,441** | 689 | 628 | 1,222 | 1,880 |
| SQL Read | **71,587** | 697 | 632 | 1,240 | 1,993 |
| Fund Transfer | **18,008** | 2,776 | 2,643 | 3,996 | 5,101 |
| KV Get | **76,523** | 652 | 588 | 1,213 | 1,806 |
| Mixed 80/20 | **64,466** | 775 | 702 | 1,393 | 2,171 |

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
| Storage | In-memory rows + optional disk | In-memory (committed log) |
| Transfer atomicity / isolation | Single held table write lock (atomic, isolated) | Serializable transaction |
| Durability | In-memory (no commit-log append on the hot path) | In-memory committed log |
| Network overhead | None (engine-level) / Axum HTTP (API) | None (in-process) |
| Index support | B-tree + hash indexes (SELECT, UPDATE, DELETE) | B-tree indexes |
| Plan caching | LRU cache for SQL path | N/A (compiled WASM) |
| Transfer logic | `execute_transfer_indexed` (read + verify + atomic debit/credit) | Native WASM reducer |
| Index-aware updates | Skip index maintenance for non-indexed cols | Built-in |
| Arithmetic | Native integer preservation (no float coercion) | Native types |

### Key Observations

- **Fund transfer dominates SpacetimeDB**: 971K TPS zero contention (9.0x), 2.5M TPS high contention (24.5x) — measuring the *full* transactional transfer (read both balances, verify funds, atomic debit/credit), not blind writes. The single-lock-hold amortizes lock acquisition and index resolution across both legs, and there is no SQL parse or plan construction. Caveat: Aegis's number is in-memory with no commit-log append, whereas SpacetimeDB appends to an in-memory committed log — so this is a transfer-logic/throughput comparison, not a durability one.
- **KV is blazing fast**: 10.8M reads/sec, 4.1M writes/sec, and 27.6M deletes/sec at the engine level; ~77K reads/sec over HTTP.
- **SQL inserts are strong**: ~204K single-row inserts/sec engine-level; ~72K/sec over HTTP.
- **Concurrent throughput scales well**: the in-process async mixed workload reaches ~3.3M ops/sec at concurrency 100 (1k-row table).
- **HTTP overhead is reasonable**: ~689μs avg for inserts, ~652μs avg for KV gets — the Axum stack adds modest latency. (HTTP throughput is dominated by per-request network + JSON cost, so KV-get and SQL numbers converge.)
- SpacetimeDB runs application logic _inside_ the database as WASM, eliminating all network overhead. Aegis-DB's direct execution API is the closest comparison (both in-process, no SQL parsing).

### Optimization Status (Completed)

1. ~~**B-tree + hash indexes**~~ — Index-accelerated SELECT, UPDATE, and DELETE.
2. ~~**Prepared statement / plan cache**~~ — LRU plan cache skips re-parsing for repeated SQL.
3. ~~**UPDATE with column expressions**~~ — `SET balance = balance - 1` evaluated server-side.
4. ~~**Indexed closure UPDATE**~~ — `execute_update_indexed_fn()` does index lookup + in-place mutation, no SQL parse.
5. ~~**Skip index maintenance**~~ — When updated columns aren't indexed, skip index remove/insert.
6. ~~**Integer arithmetic**~~ — Preserve native Integer type for int+int operations.
7. ~~**Graph adjacency lists**~~ — O(degree) traversal. Label index, relationship index, batch operations.
8. ~~**Document index-accelerated queries**~~ — `find()` routes Eq filters to hash/btree indexes.
9. ~~**TimeSeries / Streaming atomic stats**~~ — Atomic counters in place of stats locks.

### Remaining Opportunities

1. **Batch/pipeline API** — Allow multiple statements in a single request.
2. **Connection pooling** in the HTTP load test for a fairer API comparison.
3. **TimeSeries sharded writes** — Partition `series_data` for parallel ingestion.
4. **Graph property indexing** — Index node/edge properties for filtered traversals.

---

## How to Reproduce

```bash
# Engine-level benchmarks (Criterion)
cargo bench -p aegis-server

# View detailed HTML report
open target/criterion/report/index.html

# HTTP load test (requires running server)
AEGIS_OPEN_BOOTSTRAP=true cargo run --release -p aegis-server -- --port 9097 &
sleep 3
cargo run --release -p aegis-benchmarks -- --concurrency 50 --duration 10 --url http://localhost:9097

# Run specific benchmark only
cargo run --release -p aegis-benchmarks -- --benchmark transfer --concurrency 100
```
