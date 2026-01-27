---
layout: default
title: aegis-replication
parent: Crate Documentation
nav_order: 8
description: "Clustering and replication"
---

# aegis-replication

Distributed Replication and Raft Consensus for Aegis Database Platform.

## Overview

Raft-based consensus and replication for distributed Aegis deployments.
Provides leader election, log replication, and distributed state machine.

## Modules

### node.rs
Node identification and management:
- `NodeId` - Unique node identifier
- `NodeStatus` - Starting, Healthy, Suspect, Down, Leaving, Left
- `NodeRole` - Follower, Candidate, Leader
- `NodeInfo` - Address, port, status, role, metadata
- `NodeMetadata` - Datacenter, rack, zone, tags, capacity
- `NodeHealth` - Health check result with metrics

### log.rs
Replicated log for Raft consensus:
- `LogIndex` - Index in the replicated log
- `Term` - Raft term number
- `LogEntry` - Entry with index, term, type, data, timestamp
- `EntryType` - Command, ConfigChange, NoOp
- `ReplicatedLog` - Append, get, commit, truncate, compact operations
- Conflict detection for log reconciliation

### state.rs
State machine abstraction:
- `Command` - Get, Set, Delete, CompareAndSwap, Increment, Custom
- `CommandType` - Operation type enum
- `CommandResult` - Success/error with value and applied index
- `StateMachine` - Key-value store with versioning
- `Snapshot` - State machine snapshot for recovery

### raft.rs
Core Raft consensus algorithm:
- `RaftConfig` - Election timeout, heartbeat interval, snapshot threshold
- `RaftState` - Persistent state (term, voted_for, commit_index)
- `VoteRequest` / `VoteResponse` - Leader election
- `AppendEntriesRequest` / `AppendEntriesResponse` - Log replication
- `InstallSnapshotRequest` / `InstallSnapshotResponse` - Snapshot transfer
- `RaftNode` - Full Raft implementation with election and replication

### cluster.rs
Cluster coordination:
- `ClusterConfig` - Min/max nodes, heartbeat, failure timeout, quorum
- `ClusterState` - Initializing, Forming, Healthy, Degraded, NoQuorum
- `Cluster` - Node management, health tracking, leader management
- `ClusterStats` - Cluster health statistics
- `MembershipChange` - Add, remove, update node operations

### transport.rs
Network transport layer:
- `MessageType` - VoteRequest, AppendEntries, Heartbeat, etc.
- `Message` - Raft protocol message with payload
- `MessagePayload` - Serialized request/response data
- `ClientRequest` / `ClientResponse` - Client operations
- `Transport` trait - Send, receive, broadcast interface
- `InMemoryTransport` - Testing transport
- `ConnectionPool` - Peer connection management

### engine.rs
Main replication engine:
- `ReplicationConfig` - Combined Raft and cluster config
- `ReplicationEngine` - Coordinates Raft and cluster
- `TickResult` - Event loop tick output
- State machine operations (propose, get, set, delete)
- Message processing for distributed communication

## Usage Example

```rust
use aegis_replication::*;

// Create replication engine
let node = NodeInfo::new("node1", "127.0.0.1", 5000);
let mut engine = ReplicationEngine::new(node, ReplicationConfig::default());

// Add peers
let peer = NodeInfo::new("node2", "127.0.0.1", 5001);
engine.add_peer(peer)?;

// Become leader (after election)
let request = engine.start_election();
// ... handle votes from peers ...

// If leader, propose commands
if engine.is_leader() {
    let index = engine.set("key1", b"value1".to_vec())?;

    // Apply committed entries
    let results = engine.apply_committed();
}

// Read values
let value = engine.get("key1");
```

## Raft Node Example

```rust
use aegis_replication::*;

// Create Raft node
let node = RaftNode::new("node1", RaftConfig::default());
node.add_peer(NodeId::new("node2"));
node.add_peer(NodeId::new("node3"));

// Start election
let vote_request = node.start_election();

// Handle vote response
let vote_response = VoteResponse {
    term: 1,
    vote_granted: true,
    voter_id: NodeId::new("node2"),
};
let became_leader = node.handle_vote_response(&vote_response);

// If leader, propose commands
if node.is_leader() {
    let command = Command::set("key", b"value".to_vec());
    let index = node.propose(command)?;
}

// Apply committed entries
let results = node.apply_committed();
```

## Cluster Management Example

```rust
use aegis_replication::*;

// Create cluster
let local_node = NodeInfo::new("node1", "127.0.0.1", 5000);
let config = ClusterConfig::new("my-cluster")
    .with_replication_factor(3)
    .with_heartbeat_interval(Duration::from_secs(1));

let cluster = Cluster::new(local_node, config);

// Add nodes
cluster.add_node(NodeInfo::new("node2", "127.0.0.1", 5001))?;
cluster.add_node(NodeInfo::new("node3", "127.0.0.1", 5002))?;

// Check cluster health
let stats = cluster.stats();
println!("Healthy: {}, Has quorum: {}", stats.healthy_nodes, stats.has_quorum);

// Handle heartbeats
cluster.heartbeat(&NodeId::new("node2"));

// Check for failures
let failed = cluster.check_failures();
```

## Tests

49 tests covering all modules:
- Node management and health
- Replicated log operations
- State machine commands
- Raft leader election
- Log replication
- Cluster coordination
- Transport messaging
- Replication engine
