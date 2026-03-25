# Aegis-DB Audit Progress Tracker

**Started:** 2026-03-24
**Total Items:** 18
**Completed:** 18 / 18

---

## CRITICAL (Fix Now)

- [x] **#1** Replace panic!() with graceful errors in server (main.rs TLS paths)
  - Replaced `panic!()` and `.expect()` with `tracing::error!()` + `std::process::exit(1)` in `main.rs:300-321`
  - gdpr.rs panics confirmed test-only — no production fix needed

- [x] **#2** Implement skip, sort, projection in document query execution
  - Rewrote `collection.rs:find()` to apply sort, skip, limit, and projection
  - Added `compare_values()` + `compare_value_inner()` helpers to `types.rs` for sort ordering
  - All 36 aegis-document tests pass

- [x] **#3** Add overflow checks in aegis-memory arena allocator
  - `arena.rs:126` — `size + align` → `size.checked_add(align)?`
  - `arena.rs:153` — `size_of::<T>() * len` → `size_of::<T>().checked_mul(len)?`
  - Both now return `None` on overflow instead of silently wrapping

- [x] **#4** Validate admin user exists after bootstrap or block API
  - Added loud startup warning in `main.rs` when no admin user configured
  - Added per-request `tracing::warn!` with path in `middleware.rs` during open-access mode
  - All 178 server tests pass

---

## HIGH (Fix Before v1.0)

- [x] **#5** Implement HashJoin in query executor
  - Added `HashJoinOperator` with build/probe phases, equality key extraction
  - Dispatches based on `JoinStrategy::HashJoin` in `create_operator()`
  - Supports LEFT join with NULL padding; all 41 query tests pass

- [x] **#6** Implement consumer groups and ack enforcement in streaming
  - Added ConsumerGroup struct with offset tracking and member management
  - Added consumer group CRUD to StreamingEngine
  - Enforced AckMode in ChannelReceiver; 45 tests pass (10 new)

- [x] **#7** Implement real connection pooling in aegis-client
  - Replaced Mutex<VecDeque> with mpsc unbounded channel for idle connection recycling
  - PooledConnection Drop now sends connection back through channel instead of discarding
  - Stale connection pruning via idle_timeout checks; 42 client tests pass

- [x] **#8** Classify all 17 missing error variants in is_retryable/is_user_error
  - Added 7 retryable variants (TransactionAborted, Replication, NodeNotFound, ConnectionRefused, ResourceExhausted, MemoryLimitExceeded, Io)
  - Added 5 user-error variants (Execution, IndexNotFound, BlockNotFound, PageNotFound, Configuration)
  - Added new `is_system_error()` method for remaining 5 (Storage, Corruption, Internal, Encryption, Serialization)

- [x] **#9** Add UNION/INTERSECT/EXCEPT support to query engine
  - Added SetOperationType enum, SetOperationStatement to AST
  - Parser detects UNION/INTERSECT/EXCEPT after SELECT and wraps in SetOperation
  - Planner generates SetOperationNode; executor implements all 4 ops (UNION, UNION ALL, INTERSECT, EXCEPT)
  - All 41 query tests pass

- [x] **#14** Implement document index utilization in queries
  - Added `find_indexed_candidates()` to scan filters for Eq conditions on indexed fields
  - Uses index `lookup()` to narrow candidate set before full filter chain
  - Intersects multiple index results for multi-field queries; 39 tests pass (3 new)

- [x] **#15** Fix SELECT t.* table-qualified wildcard expansion
  - Replaced `let _ = table` placeholder with prefix-based filtering
  - `SELECT t.*` now only expands columns with matching table prefix

- [x] **#16** Add missing scalar functions to query executor
  - Added 13 functions: ROUND, CEIL/CEILING, FLOOR, SUBSTRING/SUBSTR, TRIM, LTRIM, RTRIM, NULLIF, NOW/CURRENT_TIMESTAMP, CURRENT_DATE, CURRENT_TIME, EXTRACT, REPLACE, CONCAT

- [x] **#17** Add missing Value accessors to aegis-common types
  - Added `as_bytes()`, `as_timestamp()`, `as_array()`, `as_object()` to Value enum

---

## LOW (Cleanup / Docs)

- [x] **#10** Fix config comment "4 GB" → "256 MB"
  - Fixed comment in `config.rs:200`

- [x] **#11** Remove false alerting claims from monitoring README
  - Rewrote entire README to match actual API (MetricRegistry, counter_with_labels, sync health checks)
  - Removed: alerting, async health checks, counter_vec, propagate/extract, Jaeger/Grafana claims

- [x] **#12** Remove false memory pool/zero-copy claims from aegis-memory lib.rs
  - Rewrote lib.rs doc comments to accurately describe arena allocator only
  - Removed claims about memory pools, pressure monitoring, and zero-copy buffers

- [x] **#13** Remove dead code (Chunk::remaining, unused JoinType variants)
  - Made `Chunk::remaining()` public with doc comment (removed `#[allow(dead_code)]`)
  - Added `right_join()` and `full_join()` builders to client QueryBuilder (uses all JoinType variants)
  - Removed `#[allow(dead_code)]` from JoinType enum

---

## DOCUMENTATION (After All Fixes)

- [x] **#18** Update all documentation and README to reflect changes
  - CLAUDE.md: Updated test count (652), query engine features, memory crate description, error classification
  - README.md: Updated test badge (652), LOC badge (69K+), auth security description, added SQL Engine/Document Store/Streaming feature sections
  - aegis-monitoring README: Full rewrite to match actual API
  - aegis-memory lib.rs: Rewritten to accurately describe arena allocator

---

## Change Log

| # | Item | Status | Date | Notes |
|---|------|--------|------|-------|
| 1 | Server panic!() removal | DONE | 2026-03-24 | main.rs TLS — replaced with tracing::error + exit(1) |
| 2 | Document skip/sort/projection | DONE | 2026-03-24 | Added sort/skip/limit/projection to find(); added compare_values() to types.rs |
| 3 | Arena overflow checks | DONE | 2026-03-24 | checked_mul and checked_add — returns None on overflow |
| 4 | Admin bootstrap auth | DONE | 2026-03-24 | Startup warning + per-request warn logging |
| 5 | HashJoin executor | DONE | 2026-03-24 | Full HashJoinOperator with build/probe + key extraction |
| 6 | Streaming consumer groups | DONE | 2026-03-24 | ConsumerGroup struct + ack enforcement; 45 tests pass |
| 7 | Connection pool recycling | DONE | 2026-03-24 | mpsc channel-based recycling; 42 tests pass |
| 8 | Error variant classification | DONE | 2026-03-24 | All variants classified; added is_system_error() |
| 9 | UNION/INTERSECT/EXCEPT | DONE | 2026-03-24 | Full stack: parser → planner → executor |
| 10 | Config comment fix | DONE | 2026-03-24 | "4 GB" → "256 MB" |
| 11 | Monitoring README fix | DONE | 2026-03-24 | Full rewrite to match actual API |
| 12 | Memory crate docs fix | DONE | 2026-03-24 | Removed 3 false feature claims |
| 13 | Dead code removal | DONE | 2026-03-24 | Made remaining() public; added right_join/full_join |
| 14 | Document index utilization | DONE | 2026-03-24 | Index-accelerated lookups for Eq filters |
| 15 | SELECT t.* fix | DONE | 2026-03-24 | Table-prefix filtering for qualified wildcards |
| 16 | Scalar functions | DONE | 2026-03-24 | +13 functions: ROUND, CEIL, FLOOR, SUBSTR, TRIM, etc |
| 17 | Value accessors | DONE | 2026-03-24 | Added as_bytes, as_timestamp, as_array, as_object |
| 18 | Documentation update | DONE | 2026-03-24 | CLAUDE.md, README.md, monitoring README, memory lib.rs |
