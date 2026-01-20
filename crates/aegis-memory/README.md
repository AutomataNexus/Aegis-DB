<p align="center">
  <img src="https://raw.githubusercontent.com/AutomataNexus/Aegis-DB/main/AegisDB-logo.png" alt="AegisDB Logo" width="300">
</p>

# aegis-memory

<p align="center">
  <a href="https://opensource.org/licenses/Apache-2.0"><img src="https://img.shields.io/badge/License-Apache%202.0-blue.svg" alt="License"></a>
  <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/Rust-1.75%2B-orange.svg" alt="Rust"></a>
  <img src="https://img.shields.io/badge/crate-0.1.0-green.svg" alt="Version">
  <a href="../../README.md"><img src="https://img.shields.io/badge/part%20of-AegisDB-teal.svg" alt="AegisDB"></a>
</p>

Memory management and allocation for the Aegis Database Platform.

## Overview

`aegis-memory` provides efficient memory management primitives including arena allocators and buffer pools. It's designed for high-performance database operations where memory allocation patterns are predictable.

## Features

- **Arena Allocators** - Fast bump allocation with batch deallocation
- **Memory Pools** - Pre-allocated fixed-size block pools
- **Zero-Copy Operations** - Minimize memory copies where possible
- **Thread-Safe** - Lock-free and lock-based options

## Modules

| Module | Description |
|--------|-------------|
| `arena` | Arena-based memory allocator |

## Usage

```toml
[dependencies]
aegis-memory = { path = "../aegis-memory" }
```

### Arena Allocator

The arena allocator is ideal for request-scoped allocations where all memory can be freed at once:

```rust
use aegis_memory::arena::Arena;

// Create an arena with 1MB initial capacity
let arena = Arena::new(1024 * 1024);

// Allocate memory (very fast - just bumps a pointer)
let buffer = arena.alloc(1024);

// Use the buffer...
buffer[0] = 42;

// Reset frees all allocations at once (O(1))
arena.reset();
```

### Benefits

| Operation | Arena | Standard Allocator |
|-----------|-------|-------------------|
| Allocate | O(1) bump | O(log n) search |
| Deallocate | Deferred | O(log n) coalesce |
| Reset | O(1) | N/A |

## Design

```
Arena Memory Layout:
┌────────────────────────────────────────────┐
│ Block 1                                    │
│ ┌─────────┬─────────┬─────────┬──────────┐│
│ │ Alloc 1 │ Alloc 2 │ Alloc 3 │   Free   ││
│ └─────────┴─────────┴─────────┴──────────┘│
├────────────────────────────────────────────┤
│ Block 2 (allocated when Block 1 full)     │
│ ┌─────────┬──────────────────────────────┐│
│ │ Alloc 4 │           Free               ││
│ └─────────┴──────────────────────────────┘│
└────────────────────────────────────────────┘
```

## Benchmarks

Run benchmarks:

```bash
cargo bench -p aegis-memory
```

Typical results:
- Arena allocation: ~5ns per allocation
- Standard allocation: ~25ns per allocation
- Arena reset: ~100ns (constant, regardless of allocation count)

## Tests

```bash
cargo test -p aegis-memory
```

**Test count:** 5 tests

## License

Apache-2.0
