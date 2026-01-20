# aegis-memory

Memory management utilities including arena allocators and memory pools.

## Overview

The `aegis-memory` crate provides efficient memory management primitives for the Aegis database. It implements arena-based allocation for fast, bulk deallocatable memory regions.

## Modules

### arena.rs
Arena-based memory allocator:

**Features:**
- O(1) bump allocation
- Bulk deallocation (reset entire arena)
- Chunk-based growth
- Configurable chunk size

**Arena Struct:**
```rust
pub struct Arena {
    chunks: Vec<Chunk>,
    current_chunk: usize,
    chunk_size: usize,
}

impl Arena {
    pub fn new() -> Self;
    pub fn with_chunk_size(size: usize) -> Self;
    pub fn alloc(&mut self, size: usize) -> *mut u8;
    pub fn alloc_slice<T: Copy>(&mut self, len: usize) -> &mut [T];
    pub fn reset(&mut self);
    pub fn allocated(&self) -> usize;
}
```

**Use Cases:**
- Query execution temporary allocations
- Parse tree construction
- Batch processing buffers

### lib.rs
Module exports and memory utilities.

## Usage

```rust
use aegis_memory::Arena;

let mut arena = Arena::with_chunk_size(4096);

// Allocate memory
let ptr = arena.alloc(256);

// Allocate typed slice
let slice: &mut [u64] = arena.alloc_slice(100);

// Reset all allocations at once
arena.reset();
```

## Performance

Arena allocation provides significant performance benefits over individual heap allocations:
- No per-object deallocation overhead
- Cache-friendly sequential allocation
- Reduced fragmentation
- Ideal for request-scoped memory

## Tests

5 tests covering allocation, alignment, and reset functionality.
