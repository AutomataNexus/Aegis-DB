//! Aegis Storage Benchmarks
//!
//! Performance benchmarks for storage operations including block I/O,
//! buffer pool management, and transaction throughput.
//!
//! @version 0.1.1
//! @author AutomataNexus Development Team

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput, BenchmarkId};
use aegis_storage::backend::MemoryBackend;
use aegis_storage::{StorageBackend, Block};
use aegis_storage::buffer::{BufferPool, BufferPoolConfig};
use aegis_storage::page::PageType;
use aegis_common::{BlockId, BlockType, PageId};
use bytes::Bytes;
use tokio::runtime::Runtime;

fn memory_backend_write_benchmark(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("memory_backend_write");

    for size in [1024, 4096, 16384, 65536].iter() {
        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let backend = MemoryBackend::new();
            let data = Bytes::from(vec![0u8; size]);

            b.to_async(&rt).iter(|| async {
                let block = Block::new(BlockId(0), BlockType::TableData, data.clone());
                let result = backend.write_block(block).await;
                black_box(result)
            });
        });
    }

    group.finish();
}

fn memory_backend_read_benchmark(c: &mut Criterion) {
    use std::sync::Arc;

    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("memory_backend_read");

    for size in [1024, 4096, 16384, 65536].iter() {
        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let backend = Arc::new(MemoryBackend::new());
            let data = Bytes::from(vec![0u8; size]);

            // Pre-populate blocks
            let block_ids: Vec<BlockId> = rt.block_on(async {
                let mut ids = Vec::new();
                for _ in 0..100 {
                    let block = Block::new(BlockId(0), BlockType::TableData, data.clone());
                    let id = backend.write_block(block).await.unwrap();
                    ids.push(id);
                }
                ids
            });

            let mut idx = 0usize;
            b.to_async(&rt).iter(|| {
                let backend = Arc::clone(&backend);
                let block_id = block_ids[idx % block_ids.len()];
                idx += 1;
                async move {
                    let result = backend.read_block(block_id).await;
                    black_box(result)
                }
            });
        });
    }

    group.finish();
}

fn buffer_pool_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("buffer_pool");

    for pool_size in [16, 64, 256, 1024].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(pool_size), pool_size, |b, &pool_size| {
            let config = BufferPoolConfig {
                pool_size,
                prefetch_size: 8,
            };
            let pool = BufferPool::new(config);

            // Pre-populate pool
            for i in 0..(pool_size / 2) {
                let _ = pool.new_page(PageId(i as u64), PageType::Data);
            }

            let mut idx = 0u64;
            b.iter(|| {
                let page_id = PageId(idx % (pool_size as u64 / 2));
                idx += 1;
                let result = pool.fetch_page(page_id);
                black_box(result)
            });
        });
    }

    group.finish();
}

fn transaction_benchmark(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("transactions");

    group.bench_function("begin_commit", |b| {
        let backend = MemoryBackend::new();

        b.to_async(&rt).iter(|| async {
            let tx_id = backend.begin_transaction().await.unwrap();
            backend.commit_transaction(tx_id).await.unwrap();
            black_box(tx_id)
        });
    });

    group.bench_function("begin_rollback", |b| {
        let backend = MemoryBackend::new();

        b.to_async(&rt).iter(|| async {
            let tx_id = backend.begin_transaction().await.unwrap();
            backend.rollback_transaction(tx_id).await.unwrap();
            black_box(tx_id)
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    memory_backend_write_benchmark,
    memory_backend_read_benchmark,
    buffer_pool_benchmark,
    transaction_benchmark
);
criterion_main!(benches);
