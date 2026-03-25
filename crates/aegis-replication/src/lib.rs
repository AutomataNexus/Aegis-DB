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

pub mod cluster;
pub mod crdt;
pub mod engine;
pub mod hash;
pub mod http_transport;
pub mod log;
pub mod node;
pub mod partition;
pub mod raft;
pub mod router;
pub mod shard;
pub mod state;
pub mod transaction;
pub mod transport;
pub mod vector_clock;

pub use cluster::{Cluster, ClusterConfig, ClusterState};
pub use crdt::{GCounter, GSet, LWWMap, LWWRegister, MVRegister, ORSet, PNCounter, TwoPSet, CRDT};
pub use engine::ReplicationEngine;
pub use hash::{ConsistentHash, HashRing, VirtualNode};
pub use http_transport::HttpTransport;
pub use log::{LogEntry, LogIndex, ReplicatedLog};
pub use node::{NodeId, NodeInfo, NodeStatus};
pub use partition::{PartitionKey, PartitionRange, PartitionStrategy};
pub use raft::{RaftConfig, RaftNode, RaftState};
pub use router::{RouteDecision, RoutingTable, ShardRouter};
pub use shard::{Shard, ShardId, ShardManager, ShardState};
pub use state::{
    Command, CommandResult, CommandType, DatabaseOperationHandler, DatabaseStateMachine,
    NoOpDatabaseHandler, Snapshot, StateMachine, StateMachineBackend,
};
pub use transaction::{
    DistributedTransaction, TransactionCoordinator, TransactionId, TransactionState,
};
pub use transport::{Message, MessageType, Transport};
pub use vector_clock::{
    HybridClock, HybridTimestamp, LamportClock, VectorClock, VectorClockOrdering,
};
