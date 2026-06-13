//! Aegis-DB Benchmark Suite
//!
//! Engine-level benchmarks for direct comparison with SpacetimeDB published numbers.
//! Uses QueryEngine directly (no HTTP overhead) for fair comparison since
//! SpacetimeDB runs logic in-process as WASM modules.
//!
//! Run: cargo bench -p aegis-server

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use rand::prelude::*;
use serde_json::json;
use std::sync::Arc;
use tokio::runtime::Runtime;

use aegis_common::Value;
use aegis_server::state::{KvStore, QueryEngine};
use aegis_server::{AppState, ServerConfig};

// =============================================================================
// Helpers
// =============================================================================

fn new_engine() -> QueryEngine {
    QueryEngine::new()
}

fn new_state() -> Arc<AppState> {
    Arc::new(AppState::new(ServerConfig::default()))
}

fn setup_accounts_table(engine: &QueryEngine, count: usize) {
    engine
        .execute("CREATE TABLE accounts (id INT, balance INT)", None)
        .expect("create accounts table");

    // Seed each account with a very large balance so that no account can be
    // drained over the millions of warmup+measurement iterations a single
    // bench setup sees. This keeps EVERY transfer a real committed transfer
    // (read + verify + debit + credit) rather than letting hot accounts hit
    // zero and short-circuit on the sufficient-funds check.
    for i in 0..count {
        engine
            .execute(
                &format!("INSERT INTO accounts VALUES ({}, 1000000000000)", i),
                None,
            )
            .expect("insert account");
    }

    // Create index on id column for O(log n) lookups instead of O(n) scans
    engine
        .execute("CREATE INDEX idx_accounts_id ON accounts(id)", None)
        .expect("create accounts index");
}

fn setup_bench_table(engine: &QueryEngine, rows: usize) {
    engine
        .execute(
            "CREATE TABLE bench (id INT, name VARCHAR(100), value INT)",
            None,
        )
        .expect("create bench table");

    for i in 0..rows {
        engine
            .execute(
                &format!("INSERT INTO bench VALUES ({}, 'row_{}', {})", i, i, i * 10),
                None,
            )
            .expect("insert row");
    }
}

// =============================================================================
// 1. SQL Insert Throughput
// =============================================================================

fn sql_insert_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("sql_insert");

    // Single-row insert
    group.throughput(Throughput::Elements(1));
    group.bench_function("single_row", |b| {
        let engine = new_engine();
        engine
            .execute(
                "CREATE TABLE insert_test (id INT, name VARCHAR(100), value INT)",
                None,
            )
            .unwrap();
        let mut counter = 0u64;

        b.iter(|| {
            counter += 1;
            let sql = format!(
                "INSERT INTO insert_test VALUES ({}, 'name_{}', {})",
                counter, counter, counter
            );
            black_box(engine.execute(&sql, None).unwrap());
        });
    });

    // Batch inserts (simulated via repeated single inserts per iteration)
    for batch_size in [10, 100, 1000] {
        group.throughput(Throughput::Elements(batch_size as u64));
        group.bench_with_input(
            BenchmarkId::new("batch", batch_size),
            &batch_size,
            |b, &size| {
                let engine = new_engine();
                engine
                    .execute(
                        "CREATE TABLE batch_test (id INT, name VARCHAR(100), value INT)",
                        None,
                    )
                    .unwrap();
                let mut counter = 0u64;

                b.iter(|| {
                    for _ in 0..size {
                        counter += 1;
                        let sql = format!(
                            "INSERT INTO batch_test VALUES ({}, 'name_{}', {})",
                            counter, counter, counter
                        );
                        black_box(engine.execute(&sql, None).unwrap());
                    }
                });
            },
        );
    }

    group.finish();
}

// =============================================================================
// 2. SQL Read Throughput
// =============================================================================

fn sql_read_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("sql_read");

    // Point query
    group.bench_function("point_query_10k", |b| {
        let engine = new_engine();
        setup_bench_table(&engine, 10_000);
        let mut rng = StdRng::seed_from_u64(42);

        b.iter(|| {
            let id = rng.gen_range(0..10_000);
            let sql = format!("SELECT * FROM bench WHERE id = {}", id);
            black_box(engine.execute(&sql, None).unwrap());
        });
    });

    // Full table scan
    group.bench_function("full_scan_10k", |b| {
        let engine = new_engine();
        setup_bench_table(&engine, 10_000);

        b.iter(|| {
            black_box(engine.execute("SELECT * FROM bench", None).unwrap());
        });
    });

    // Filtered scan
    group.bench_function("filtered_scan_10k", |b| {
        let engine = new_engine();
        setup_bench_table(&engine, 10_000);

        b.iter(|| {
            black_box(
                engine
                    .execute("SELECT * FROM bench WHERE value > 50000", None)
                    .unwrap(),
            );
        });
    });

    group.finish();
}

// =============================================================================
// 3. Fund Transfer Transaction (SpacetimeDB Keynote Benchmark)
// =============================================================================
//
// SpacetimeDB achieved:
//   - 107,850 TPS at 0% contention
//   - 103,590 TPS at 80% contention
//
// SpacetimeDB's number is a compiled in-process reducer doing a full
// transactional transfer. To compare honestly we do the SAME real work via
// `execute_transfer_indexed`, which under a single held write lock:
//   1. Reads the sender balance
//   2. Reads the receiver balance
//   3. Verifies sufficient funds
//   4. Debits the sender
//   5. Credits the receiver
// — atomically and in isolation (no other writer can interleave a partial
// transfer). This is NOT a pair of blind writes; the read+verify cost is paid.

fn fund_transfer_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("fund_transfer");
    group.sample_size(50); // Reduce sample size for heavy setup

    let account_count: usize = 100_000;
    let balance_col_idx: usize = 1; // "balance" is column index 1

    // 0% contention — uniform random account selection
    group.throughput(Throughput::Elements(1));
    group.bench_function("zero_contention_100k", |b| {
        let engine = new_engine();
        setup_accounts_table(&engine, account_count);
        let executor = engine.get_executor(None);
        let mut rng = StdRng::seed_from_u64(42);

        b.iter(|| {
            let sender = rng.gen_range(0..account_count);
            let mut receiver = rng.gen_range(0..account_count);
            while receiver == sender {
                receiver = rng.gen_range(0..account_count);
            }

            // Atomic transactional transfer: read both balances, verify funds,
            // debit + credit — all under one write lock.
            let outcome = executor
                .execute_transfer_indexed(
                    "accounts",
                    "id",
                    &Value::Integer(sender as i64),
                    &Value::Integer(receiver as i64),
                    balance_col_idx,
                    1,
                )
                .unwrap();

            black_box(outcome);
        });
    });

    // High contention — Zipf distribution (hot accounts)
    group.bench_function("high_contention_100k", |b| {
        let engine = new_engine();
        setup_accounts_table(&engine, account_count);
        let executor = engine.get_executor(None);
        let mut rng = StdRng::seed_from_u64(123);
        #[allow(deprecated)]
        let zipf = zipf::ZipfDistribution::new(account_count, 1.5).unwrap();

        b.iter(|| {
            let sender = zipf.sample(&mut rng) - 1;
            let mut receiver = zipf.sample(&mut rng) - 1;
            while receiver == sender {
                receiver = rng.gen_range(0..account_count);
            }

            let outcome = executor
                .execute_transfer_indexed(
                    "accounts",
                    "id",
                    &Value::Integer(sender as i64),
                    &Value::Integer(receiver as i64),
                    balance_col_idx,
                    1,
                )
                .unwrap();

            black_box(outcome);
        });
    });

    group.finish();
}

// =============================================================================
// 4. KV Operations
// =============================================================================

fn kv_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("kv_operations");

    // KV Set
    for value_size in [64, 1024, 16384] {
        let label = match value_size {
            64 => "64B",
            1024 => "1KB",
            16384 => "16KB",
            _ => "unknown",
        };
        group.throughput(Throughput::Elements(1));
        group.bench_with_input(BenchmarkId::new("set", label), &value_size, |b, &size| {
            let store = KvStore::new();
            let value_str: String = "x".repeat(size);
            let mut counter = 0u64;

            b.iter(|| {
                counter += 1;
                let key = format!("key_{}", counter);
                black_box(store.set(key, json!(value_str), None));
            });
        });
    }

    // KV Get (from pre-populated store)
    for value_size in [64, 1024, 16384] {
        let label = match value_size {
            64 => "64B",
            1024 => "1KB",
            16384 => "16KB",
            _ => "unknown",
        };
        group.throughput(Throughput::Elements(1));
        group.bench_with_input(BenchmarkId::new("get", label), &value_size, |b, &size| {
            let store = KvStore::new();
            let value_str: String = "x".repeat(size);

            // Pre-populate 10k keys
            for i in 0..10_000 {
                store.set(format!("key_{}", i), json!(value_str), None);
            }

            let mut rng = StdRng::seed_from_u64(42);
            b.iter(|| {
                let key = format!("key_{}", rng.gen_range(0..10_000));
                black_box(store.get(&key));
            });
        });
    }

    // KV Delete
    group.throughput(Throughput::Elements(1));
    group.bench_function("delete", |b| {
        let store = KvStore::new();
        let mut counter = 0u64;

        // Pre-populate many keys
        for i in 0..1_000_000 {
            store.set(format!("del_{}", i), json!("value"), None);
        }

        b.iter(|| {
            counter += 1;
            let key = format!("del_{}", counter % 1_000_000);
            black_box(store.delete(&key));
        });
    });

    group.finish();
}

// =============================================================================
// 5. Concurrent Mixed Workload
// =============================================================================

fn concurrent_mixed_benchmark(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("concurrent_mixed");
    group.sample_size(20);

    for concurrency in [1, 10, 50, 100] {
        let ops_per_task = 100;
        group.throughput(Throughput::Elements((concurrency * ops_per_task) as u64));
        group.bench_with_input(
            BenchmarkId::new("80read_20write", concurrency),
            &concurrency,
            |b, &conc| {
                // Create AppState inside runtime context (it spawns a tokio task)
                let state = rt.block_on(async {
                    let s = new_state();
                    // Setup table
                    s.query_engine
                        .execute("CREATE TABLE mixed (id INT, value INT)", None)
                        .unwrap();
                    for i in 0..1000 {
                        s.query_engine
                            .execute(&format!("INSERT INTO mixed VALUES ({}, {})", i, i), None)
                            .unwrap();
                    }
                    s
                });

                b.to_async(&rt).iter(|| {
                    let state = state.clone();
                    async move {
                        let mut handles = Vec::with_capacity(conc);

                        for task_id in 0..conc {
                            let s = state.clone();
                            handles.push(tokio::spawn(async move {
                                let mut rng = StdRng::seed_from_u64(task_id as u64);
                                for _ in 0..ops_per_task {
                                    if rng.gen_ratio(80, 100) {
                                        let id = rng.gen_range(0..1000);
                                        let sql = format!("SELECT * FROM mixed WHERE id = {}", id);
                                        let _ = s.query_engine.execute(&sql, None);
                                    } else {
                                        let id = rng.gen_range(0..1000);
                                        let val = rng.gen_range(0..100000);
                                        let sql = format!(
                                            "UPDATE mixed SET value = {} WHERE id = {}",
                                            val, id
                                        );
                                        let _ = s.query_engine.execute(&sql, None);
                                    }
                                }
                            }));
                        }

                        for h in handles {
                            let _ = h.await;
                        }
                    }
                });
            },
        );
    }

    group.finish();
}

// =============================================================================
// Entry Point
// =============================================================================

criterion_group!(
    benches,
    sql_insert_benchmark,
    sql_read_benchmark,
    fund_transfer_benchmark,
    kv_benchmark,
    concurrent_mixed_benchmark,
);
criterion_main!(benches);
