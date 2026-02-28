//! Aegis-DB HTTP Load Test
//!
//! Standalone binary that benchmarks Aegis-DB via its HTTP API.
//! Requires a running Aegis-DB server.
//!
//! Usage:
//!   cargo run -p aegis-benchmarks -- --concurrency 50 --url http://localhost:9090

use clap::Parser;
use rand::prelude::*;
use serde_json::json;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

#[derive(Parser)]
#[command(name = "aegis-benchmarks", about = "Aegis-DB HTTP Load Test")]
struct Args {
    /// Server URL
    #[arg(short, long, default_value = "http://localhost:9090")]
    url: String,

    /// Number of concurrent tasks
    #[arg(short, long, default_value_t = 50)]
    concurrency: usize,

    /// Duration of each benchmark in seconds
    #[arg(short, long, default_value_t = 10)]
    duration: u64,

    /// Specific benchmark to run (all, insert, read, transfer, kv, mixed)
    #[arg(short, long, default_value = "all")]
    benchmark: String,
}

struct BenchResult {
    name: String,
    total_ops: u64,
    duration: Duration,
    latencies_us: Vec<u64>,
}

impl BenchResult {
    fn ops_per_sec(&self) -> f64 {
        self.total_ops as f64 / self.duration.as_secs_f64()
    }

    fn p50_us(&self) -> u64 {
        percentile(&self.latencies_us, 50.0)
    }

    fn p95_us(&self) -> u64 {
        percentile(&self.latencies_us, 95.0)
    }

    fn p99_us(&self) -> u64 {
        percentile(&self.latencies_us, 99.0)
    }

    fn avg_us(&self) -> u64 {
        if self.latencies_us.is_empty() {
            return 0;
        }
        self.latencies_us.iter().sum::<u64>() / self.latencies_us.len() as u64
    }
}

fn percentile(sorted: &[u64], pct: f64) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    let idx = ((pct / 100.0) * (sorted.len() - 1) as f64) as usize;
    sorted[idx.min(sorted.len() - 1)]
}

async fn query(client: &reqwest::Client, url: &str, sql: &str) -> Result<(), reqwest::Error> {
    client
        .post(&format!("{}/api/v1/query", url))
        .json(&json!({ "sql": sql }))
        .send()
        .await?;
    Ok(())
}

async fn kv_set(
    client: &reqwest::Client,
    url: &str,
    key: &str,
    value: &str,
) -> Result<(), reqwest::Error> {
    client
        .post(&format!("{}/api/v1/kv/{}", url, key))
        .json(&json!({ "value": value }))
        .send()
        .await?;
    Ok(())
}

async fn kv_get(client: &reqwest::Client, url: &str, key: &str) -> Result<(), reqwest::Error> {
    client
        .get(&format!("{}/api/v1/kv/{}", url, key))
        .send()
        .await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Benchmark runner
// ---------------------------------------------------------------------------

async fn run_benchmark<F, Fut>(
    name: &str,
    concurrency: usize,
    duration_secs: u64,
    setup: impl FnOnce() -> F,
) -> BenchResult
where
    F: Fn(usize) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = ()> + Send,
{
    let work_fn = Arc::new(setup());
    let total_ops = Arc::new(AtomicU64::new(0));
    let latencies = Arc::new(Mutex::new(Vec::<u64>::new()));
    let deadline = Instant::now() + Duration::from_secs(duration_secs);

    let mut handles = Vec::with_capacity(concurrency);
    for task_id in 0..concurrency {
        let work = work_fn.clone();
        let ops = total_ops.clone();
        let lats = latencies.clone();

        handles.push(tokio::spawn(async move {
            let mut local_lats = Vec::with_capacity(4096);
            while Instant::now() < deadline {
                let start = Instant::now();
                work(task_id).await;
                let elapsed = start.elapsed().as_micros() as u64;
                local_lats.push(elapsed);
                ops.fetch_add(1, Ordering::Relaxed);
            }
            lats.lock().await.extend(local_lats);
        }));
    }

    for h in handles {
        let _ = h.await;
    }

    let elapsed = Duration::from_secs(duration_secs);
    let mut lats = Arc::try_unwrap(latencies).unwrap().into_inner();
    lats.sort_unstable();

    BenchResult {
        name: name.to_string(),
        total_ops: total_ops.load(Ordering::Relaxed),
        duration: elapsed,
        latencies_us: lats,
    }
}

// ---------------------------------------------------------------------------
// Individual Benchmarks
// ---------------------------------------------------------------------------

async fn bench_insert(url: &str, concurrency: usize, duration: u64) -> BenchResult {
    let client = reqwest::Client::new();
    let base = url.to_string();

    // Setup table
    let _ = query(&client, &base, "CREATE TABLE http_insert (id INT, name VARCHAR(100), value INT)").await;

    let counter = Arc::new(AtomicU64::new(0));

    run_benchmark("SQL Insert (HTTP)", concurrency, duration, move || {
        let client = client.clone();
        let base = base.clone();
        let ctr = counter.clone();
        move |_task_id: usize| {
            let client = client.clone();
            let base = base.clone();
            let ctr = ctr.clone();
            async move {
                let id = ctr.fetch_add(1, Ordering::Relaxed);
                let sql = format!("INSERT INTO http_insert VALUES ({}, 'name_{}', {})", id, id, id);
                let _ = query(&client, &base, &sql).await;
            }
        }
    })
    .await
}

async fn bench_read(url: &str, concurrency: usize, duration: u64) -> BenchResult {
    let client = reqwest::Client::new();
    let base = url.to_string();

    // Setup table with data
    let _ = query(&client, &base, "CREATE TABLE http_read (id INT, name VARCHAR(100), value INT)").await;
    for i in 0..1000 {
        let _ = query(
            &client,
            &base,
            &format!("INSERT INTO http_read VALUES ({}, 'row_{}', {})", i, i, i * 10),
        )
        .await;
    }

    run_benchmark("SQL Read (HTTP)", concurrency, duration, move || {
        let client = client.clone();
        let base = base.clone();
        move |task_id: usize| {
            let client = client.clone();
            let base = base.clone();
            async move {
                let mut rng = StdRng::seed_from_u64(task_id as u64 * 1000 + rand::random::<u64>());
                let id = rng.gen_range(0..1000);
                let sql = format!("SELECT * FROM http_read WHERE id = {}", id);
                let _ = query(&client, &base, &sql).await;
            }
        }
    })
    .await
}

async fn bench_fund_transfer(url: &str, concurrency: usize, duration: u64) -> BenchResult {
    let client = reqwest::Client::new();
    let base = url.to_string();

    // Setup accounts
    let _ = query(&client, &base, "CREATE TABLE http_accounts (id INT, balance INT)").await;
    eprintln!("  Populating 10,000 accounts (HTTP)...");
    for i in 0..10_000 {
        let _ = query(
            &client,
            &base,
            &format!("INSERT INTO http_accounts VALUES ({}, 10000)", i),
        )
        .await;
    }
    eprintln!("  Done populating.");

    run_benchmark(
        "Fund Transfer (HTTP)",
        concurrency,
        duration,
        move || {
            let client = client.clone();
            let base = base.clone();
            move |task_id: usize| {
                let client = client.clone();
                let base = base.clone();
                async move {
                    let mut rng =
                        StdRng::seed_from_u64(task_id as u64 * 1000 + rand::random::<u64>());
                    let sender = rng.gen_range(0..10_000);
                    let mut receiver = rng.gen_range(0..10_000);
                    while receiver == sender {
                        receiver = rng.gen_range(0..10_000);
                    }

                    // Read + compute + write (engine doesn't support SET col = col - expr)
                    let _ = query(
                        &client,
                        &base,
                        &format!("SELECT * FROM http_accounts WHERE id = {}", sender),
                    )
                    .await;
                    let _ = query(
                        &client,
                        &base,
                        &format!("SELECT * FROM http_accounts WHERE id = {}", receiver),
                    )
                    .await;
                    let _ = query(
                        &client,
                        &base,
                        &format!(
                            "UPDATE http_accounts SET balance = 9999 WHERE id = {}",
                            sender
                        ),
                    )
                    .await;
                    let _ = query(
                        &client,
                        &base,
                        &format!(
                            "UPDATE http_accounts SET balance = 10001 WHERE id = {}",
                            receiver
                        ),
                    )
                    .await;
                }
            }
        },
    )
    .await
}

async fn bench_kv(url: &str, concurrency: usize, duration: u64) -> BenchResult {
    let client = reqwest::Client::new();
    let base = url.to_string();

    // Pre-populate KV
    for i in 0..1000 {
        let _ = kv_set(&client, &base, &format!("bench_kv_{}", i), "test_value").await;
    }

    run_benchmark("KV Get (HTTP)", concurrency, duration, move || {
        let client = client.clone();
        let base = base.clone();
        move |task_id: usize| {
            let client = client.clone();
            let base = base.clone();
            async move {
                let mut rng = StdRng::seed_from_u64(task_id as u64 * 1000 + rand::random::<u64>());
                let key = format!("bench_kv_{}", rng.gen_range(0..1000));
                let _ = kv_get(&client, &base, &key).await;
            }
        }
    })
    .await
}

async fn bench_mixed(url: &str, concurrency: usize, duration: u64) -> BenchResult {
    let client = reqwest::Client::new();
    let base = url.to_string();

    // Setup
    let _ = query(&client, &base, "CREATE TABLE http_mixed (id INT, value INT)").await;
    for i in 0..1000 {
        let _ = query(
            &client,
            &base,
            &format!("INSERT INTO http_mixed VALUES ({}, {})", i, i),
        )
        .await;
    }

    run_benchmark("Mixed 80/20 (HTTP)", concurrency, duration, move || {
        let client = client.clone();
        let base = base.clone();
        move |task_id: usize| {
            let client = client.clone();
            let base = base.clone();
            async move {
                let mut rng = StdRng::seed_from_u64(task_id as u64 * 1000 + rand::random::<u64>());
                if rng.gen_ratio(80, 100) {
                    let id = rng.gen_range(0..1000);
                    let _ = query(
                        &client,
                        &base,
                        &format!("SELECT * FROM http_mixed WHERE id = {}", id),
                    )
                    .await;
                } else {
                    let id = rng.gen_range(0..1000);
                    let val = rng.gen_range(0..100000);
                    let _ = query(
                        &client,
                        &base,
                        &format!("UPDATE http_mixed SET value = {} WHERE id = {}", val, id),
                    )
                    .await;
                }
            }
        }
    })
    .await
}

// ---------------------------------------------------------------------------
// Output
// ---------------------------------------------------------------------------

fn print_results(results: &[BenchResult]) {
    println!();
    println!("╔══════════════════════════════╦════════════╦══════════╦══════════╦══════════╦══════════╗");
    println!("║ Benchmark                    ║   Ops/sec  ║  Avg(μs) ║ P50(μs)  ║ P95(μs)  ║ P99(μs)  ║");
    println!("╠══════════════════════════════╬════════════╬══════════╬══════════╬══════════╬══════════╣");

    for r in results {
        println!(
            "║ {:<28} ║ {:>10.0} ║ {:>8} ║ {:>8} ║ {:>8} ║ {:>8} ║",
            r.name,
            r.ops_per_sec(),
            r.avg_us(),
            r.p50_us(),
            r.p95_us(),
            r.p99_us(),
        );
    }

    println!("╚══════════════════════════════╩════════════╩══════════╩══════════╩══════════╩══════════╝");
    println!();

    // SpacetimeDB comparison
    println!("SpacetimeDB Reference (in-process WASM, no HTTP):");
    println!("  Fund Transfer 0%  contention: 107,850 TPS");
    println!("  Fund Transfer 80% contention: 103,590 TPS");
    println!("  Rust module throughput:        ~170,000 TPS");
    println!();
    println!("Note: HTTP benchmarks include network overhead. For fair comparison,");
    println!("use engine-level Criterion benchmarks: cargo bench -p aegis-server");
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() {
    let args = Args::parse();

    println!("Aegis-DB HTTP Load Test");
    println!("=======================");
    println!("Server:      {}", args.url);
    println!("Concurrency: {}", args.concurrency);
    println!("Duration:    {}s per benchmark", args.duration);
    println!();

    // Verify server is reachable
    let client = reqwest::Client::new();
    match client.get(&format!("{}/health", args.url)).send().await {
        Ok(resp) if resp.status().is_success() => {
            println!("Server is healthy. Starting benchmarks...");
        }
        Ok(resp) => {
            eprintln!("Server returned status: {}. Continuing anyway.", resp.status());
        }
        Err(e) => {
            eprintln!("ERROR: Cannot reach server at {}: {}", args.url, e);
            eprintln!("Make sure aegis-server is running: cargo run -p aegis-server");
            std::process::exit(1);
        }
    }

    let mut results = Vec::new();
    let benchmarks: Vec<&str> = if args.benchmark == "all" {
        vec!["insert", "read", "transfer", "kv", "mixed"]
    } else {
        vec![args.benchmark.as_str()]
    };

    for bench in benchmarks {
        eprintln!("Running: {}...", bench);
        let result = match bench {
            "insert" => bench_insert(&args.url, args.concurrency, args.duration).await,
            "read" => bench_read(&args.url, args.concurrency, args.duration).await,
            "transfer" => bench_fund_transfer(&args.url, args.concurrency, args.duration).await,
            "kv" => bench_kv(&args.url, args.concurrency, args.duration).await,
            "mixed" => bench_mixed(&args.url, args.concurrency, args.duration).await,
            _ => {
                eprintln!("Unknown benchmark: {}. Options: insert, read, transfer, kv, mixed, all", bench);
                continue;
            }
        };
        results.push(result);
    }

    print_results(&results);
}
