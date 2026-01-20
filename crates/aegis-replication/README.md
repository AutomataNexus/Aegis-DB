<p align="center">
  <img src="https://raw.githubusercontent.com/AutomataNexus/Aegis-DB/main/AegisDB-logo.png" alt="AegisDB Logo" width="300">
</p>

# aegis-replication

<p align="center">
  <a href="https://opensource.org/licenses/Apache-2.0"><img src="https://img.shields.io/badge/License-Apache%202.0-blue.svg" alt="License"></a>
  <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/Rust-1.75%2B-orange.svg" alt="Rust"></a>
  <img src="https://img.shields.io/badge/crate-0.1.0-green.svg" alt="Version">
  <a href="../../README.md"><img src="https://img.shields.io/badge/part%20of-AegisDB-teal.svg" alt="AegisDB"></a>
</p>

Distributed replication and consensus for the Aegis Database Platform.

## Overview

`aegis-replication` provides the distributed systems layer including Raft consensus, consistent hashing, sharding, distributed transactions, and conflict-free replicated data types (CRDTs).

## Features

- **Raft Consensus** - Leader election and log replication
- **Consistent Hashing** - HashRing, JumpHash, Rendezvous hashing
- **Sharding** - Automatic data partitioning
- **Distributed Transactions** - Two-phase commit (2PC)
- **CRDTs** - Conflict-free replication for eventual consistency
- **Vector Clocks** - Causality tracking

## Architecture

```
┌────────────────────────────────────────────────────────┐
│                   Cluster Manager                       │
├────────────────────────────────────────────────────────┤
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐    │
│  │    Raft     │  │   Shard     │  │ Transaction │    │
│  │  Consensus  │  │   Router    │  │ Coordinator │    │
│  └─────────────┘  └─────────────┘  └─────────────┘    │
├────────────────────────────────────────────────────────┤
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐    │
│  │   Hash      │  │   Vector    │  │    CRDT     │    │
│  │   Ring      │  │   Clocks    │  │   Engine    │    │
│  └─────────────┘  └─────────────┘  └─────────────┘    │
├────────────────────────────────────────────────────────┤
│                    Transport Layer                      │
│              (gRPC / TCP / In-Memory)                   │
└────────────────────────────────────────────────────────┘
```

## Modules

| Module | Description |
|--------|-------------|
| `raft` | Raft consensus implementation |
| `cluster` | Cluster membership management |
| `shard` | Shard assignment and routing |
| `partition` | Data partitioning strategies |
| `hash` | Consistent hashing algorithms |
| `transaction` | Distributed transaction coordinator |
| `crdt` | CRDT implementations |
| `vector_clock` | Vector clock for causality |
| `transport` | Network communication |
| `log` | Replicated log |

## Usage

```toml
[dependencies]
aegis-replication = { path = "../aegis-replication" }
```

### Raft Consensus

```rust
use aegis_replication::raft::{RaftNode, RaftConfig};

let config = RaftConfig {
    node_id: 1,
    peers: vec![2, 3],
    election_timeout: Duration::from_millis(150..300),
    heartbeat_interval: Duration::from_millis(50),
};

let node = RaftNode::new(config, storage)?;

// Start the node
node.start().await?;

// Propose a value (only leader can propose)
if node.is_leader() {
    node.propose(command).await?;
}
```

### Consistent Hashing

```rust
use aegis_replication::hash::{HashRing, JumpHash};

// HashRing with virtual nodes
let mut ring = HashRing::new(150); // 150 virtual nodes per physical node
ring.add_node("node-1");
ring.add_node("node-2");
ring.add_node("node-3");

// Get nodes for a key (returns primary + replicas)
let nodes = ring.get_nodes("user:123", 3);

// JumpHash for fixed node count
let node_index = JumpHash::hash("user:123", 3);
```

### Distributed Transactions

```rust
use aegis_replication::transaction::{TwoPhaseCommit, TransactionCoordinator};

let coordinator = TransactionCoordinator::new(cluster);

// Begin distributed transaction
let tx = coordinator.begin().await?;

// Execute on multiple shards
tx.execute_on_shard(shard_1, operation_1).await?;
tx.execute_on_shard(shard_2, operation_2).await?;

// Two-phase commit
coordinator.commit(tx).await?;
```

### CRDTs

```rust
use aegis_replication::crdt::{GCounter, LWWRegister, ORSet};

// G-Counter (grow-only counter)
let mut counter = GCounter::new(node_id);
counter.increment(5);
counter.merge(&remote_counter);
println!("Count: {}", counter.value());

// LWW-Register (last-writer-wins)
let mut register = LWWRegister::new();
register.set("value", timestamp);

// OR-Set (observed-remove set)
let mut set = ORSet::new(node_id);
set.add("item");
set.remove("item");
```

### Vector Clocks

```rust
use aegis_replication::vector_clock::VectorClock;

let mut clock = VectorClock::new();

// Increment local time
clock.increment(node_id);

// Merge with remote clock
clock.merge(&remote_clock);

// Compare causality
match clock.compare(&other_clock) {
    Ordering::Before => println!("Happened before"),
    Ordering::After => println!("Happened after"),
    Ordering::Concurrent => println!("Concurrent events"),
}
```

## Configuration

```toml
[replication]
replication_factor = 3
consistency_level = "quorum"    # one, quorum, all

[raft]
election_timeout_min = 150
election_timeout_max = 300
heartbeat_interval = 50

[sharding]
strategy = "hash"               # hash, range
shard_count = 16
auto_rebalance = true
```

## Tests

```bash
cargo test -p aegis-replication
```

**Test count:** 136 tests

## License

Apache-2.0
