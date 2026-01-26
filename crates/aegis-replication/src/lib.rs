//! Aegis Replication - Distributed Systems
//!
//! Raft-based consensus and replication for distributed Aegis deployments.
//! Provides leader election, log replication, sharding, and distributed transactions.
//!
//! Key Features:
//! - Raft consensus algorithm implementation
//! - Multi-master replication support
//! - Consistent hashing and sharding
//! - Distributed transactions (2PC)
//! - Vector clocks for causality tracking
//! - CRDTs for conflict-free replication
//! - Automatic failover and recovery
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

pub mod raft;
pub mod log;
pub mod state;
pub mod node;
pub mod cluster;
pub mod transport;
pub mod engine;
pub mod hash;
pub mod shard;
pub mod partition;
pub mod router;
pub mod transaction;
pub mod vector_clock;
pub mod crdt;

pub use raft::{RaftNode, RaftState, RaftConfig};
pub use log::{LogEntry, LogIndex, ReplicatedLog};
pub use state::{StateMachine, StateMachineBackend, DatabaseStateMachine, DatabaseOperationHandler, NoOpDatabaseHandler, Command, CommandType, CommandResult, Snapshot};
pub use node::{NodeId, NodeInfo, NodeStatus};
pub use cluster::{Cluster, ClusterConfig, ClusterState};
pub use transport::{Message, MessageType, Transport};
pub use engine::ReplicationEngine;
pub use hash::{ConsistentHash, HashRing, VirtualNode};
pub use shard::{Shard, ShardId, ShardManager, ShardState};
pub use partition::{PartitionKey, PartitionStrategy, PartitionRange};
pub use router::{ShardRouter, RoutingTable, RouteDecision};
pub use transaction::{TransactionId, TransactionState, TransactionCoordinator, DistributedTransaction};
pub use vector_clock::{VectorClock, VectorClockOrdering, HybridClock, HybridTimestamp, LamportClock};
pub use crdt::{CRDT, GCounter, PNCounter, GSet, TwoPSet, LWWRegister, MVRegister, ORSet, LWWMap};
