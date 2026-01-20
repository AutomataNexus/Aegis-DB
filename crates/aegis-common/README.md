<p align="center">
  <img src="../../AegisDB-logo.png" alt="AegisDB Logo" width="300">
</p>

# aegis-common

<p align="center">
  <a href="https://opensource.org/licenses/Apache-2.0"><img src="https://img.shields.io/badge/License-Apache%202.0-blue.svg" alt="License"></a>
  <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/Rust-1.75%2B-orange.svg" alt="Rust"></a>
  <img src="https://img.shields.io/badge/crate-0.1.0-green.svg" alt="Version">
  <a href="../../README.md"><img src="https://img.shields.io/badge/part%20of-AegisDB-teal.svg" alt="AegisDB"></a>
</p>

Common types, utilities, and shared functionality for the Aegis Database Platform.

## Overview

`aegis-common` provides the foundational types and utilities used across all Aegis crates. It ensures consistency and reduces code duplication throughout the platform.

## Features

- **Configuration Management** - TOML-based configuration loading and validation
- **Error Handling** - Unified error types with `thiserror`
- **Common Types** - Shared data structures (IDs, timestamps, results)
- **Utilities** - Hashing (xxHash), checksums (CRC32), UUID generation

## Modules

| Module | Description |
|--------|-------------|
| `config` | Configuration structures and loading |
| `error` | Common error types and Result aliases |
| `types` | Shared type definitions (NodeId, BlockId, etc.) |
| `utils` | Utility functions (hashing, encoding, time) |

## Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
aegis-common = { path = "../aegis-common" }
```

### Example

```rust
use aegis_common::{AegisError, AegisResult, NodeId};
use aegis_common::config::AegisConfig;
use aegis_common::utils::{generate_id, hash_bytes};

// Generate a unique ID
let id = generate_id();

// Hash some data
let hash = hash_bytes(b"hello world");

// Load configuration
let config = AegisConfig::from_file("aegis.toml")?;
```

## Dependencies

- `serde` / `serde_json` - Serialization
- `thiserror` - Error handling
- `chrono` - Date/time utilities
- `uuid` - Unique identifier generation
- `xxhash-rust` - Fast hashing
- `crc32fast` - Checksum computation

## Tests

```bash
cargo test -p aegis-common
```

## License

Apache-2.0
