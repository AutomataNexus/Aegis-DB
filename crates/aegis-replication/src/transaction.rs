//! Aegis Distributed Transactions
//!
//! Two-Phase Commit (2PC) protocol for distributed transaction coordination.
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use crate::node::NodeId;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

// =============================================================================
// Transaction ID
// =============================================================================

/// Unique identifier for distributed transactions.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TransactionId(String);

impl TransactionId {
    /// Create a new transaction ID.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Generate a unique transaction ID.
    pub fn generate() -> Self {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        Self(format!("txn_{:x}", timestamp))
    }

    /// Get the ID as a string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for TransactionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// =============================================================================
// Transaction State
// =============================================================================

/// State of a distributed transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransactionState {
    /// Transaction is being prepared.
    Preparing,
    /// Transaction is prepared and ready to commit.
    Prepared,
    /// Transaction is committing.
    Committing,
    /// Transaction has been committed.
    Committed,
    /// Transaction is aborting.
    Aborting,
    /// Transaction has been aborted.
    Aborted,
    /// Transaction state is unknown (recovery needed).
    Unknown,
}

impl TransactionState {
    /// Check if the transaction is in a terminal state.
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Committed | Self::Aborted)
    }

    /// Check if the transaction can be committed.
    pub fn can_commit(&self) -> bool {
        matches!(self, Self::Prepared)
    }

    /// Check if the transaction can be aborted.
    pub fn can_abort(&self) -> bool {
        !matches!(self, Self::Committed)
    }
}

// =============================================================================
// Participant Vote
// =============================================================================

/// Vote from a participant in 2PC.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ParticipantVote {
    /// Participant votes to commit.
    Commit,
    /// Participant votes to abort.
    Abort,
    /// Participant hasn't voted yet.
    Pending,
}

// =============================================================================
// Transaction Participant
// =============================================================================

/// A participant in a distributed transaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionParticipant {
    pub node_id: NodeId,
    pub vote: ParticipantVote,
    pub prepared: bool,
    pub committed: bool,
    pub last_contact: Option<u64>,
}

impl TransactionParticipant {
    /// Create a new participant.
    pub fn new(node_id: NodeId) -> Self {
        Self {
            node_id,
            vote: ParticipantVote::Pending,
            prepared: false,
            committed: false,
            last_contact: None,
        }
    }

    /// Record a prepare vote.
    pub fn record_prepare(&mut self, vote: ParticipantVote) {
        self.vote = vote;
        self.prepared = vote == ParticipantVote::Commit;
    }

    /// Record commit acknowledgment.
    pub fn record_commit(&mut self) {
        self.committed = true;
    }
}

// =============================================================================
// Distributed Transaction
// =============================================================================

/// A distributed transaction managed by 2PC.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistributedTransaction {
    pub id: TransactionId,
    pub coordinator: NodeId,
    pub participants: HashMap<NodeId, TransactionParticipant>,
    pub state: TransactionState,
    pub created_at: u64,
    pub timeout_ms: u64,
    pub operations: Vec<TransactionOperation>,
}

impl DistributedTransaction {
    /// Create a new distributed transaction.
    pub fn new(id: TransactionId, coordinator: NodeId, timeout_ms: u64) -> Self {
        let created_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        Self {
            id,
            coordinator,
            participants: HashMap::new(),
            state: TransactionState::Preparing,
            created_at,
            timeout_ms,
            operations: Vec::new(),
        }
    }

    /// Add a participant to the transaction.
    pub fn add_participant(&mut self, node_id: NodeId) {
        if !self.participants.contains_key(&node_id) {
            self.participants
                .insert(node_id.clone(), TransactionParticipant::new(node_id));
        }
    }

    /// Add an operation to the transaction.
    pub fn add_operation(&mut self, operation: TransactionOperation) {
        self.operations.push(operation);
    }

    /// Check if all participants are prepared.
    pub fn all_prepared(&self) -> bool {
        self.participants.values().all(|p| p.prepared)
    }

    /// Check if all participants have committed.
    pub fn all_committed(&self) -> bool {
        self.participants.values().all(|p| p.committed)
    }

    /// Check if any participant voted to abort.
    pub fn any_abort(&self) -> bool {
        self.participants
            .values()
            .any(|p| p.vote == ParticipantVote::Abort)
    }

    /// Check if the transaction has timed out.
    pub fn is_timed_out(&self) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        now - self.created_at > self.timeout_ms
    }

    /// Get participant count.
    pub fn participant_count(&self) -> usize {
        self.participants.len()
    }

    /// Get prepared count.
    pub fn prepared_count(&self) -> usize {
        self.participants.values().filter(|p| p.prepared).count()
    }
}

// =============================================================================
// Transaction Operation
// =============================================================================

/// An operation within a distributed transaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransactionOperation {
    /// Read operation.
    Read {
        key: String,
        shard_id: String,
    },
    /// Write operation.
    Write {
        key: String,
        value: Vec<u8>,
        shard_id: String,
    },
    /// Delete operation.
    Delete {
        key: String,
        shard_id: String,
    },
    /// Compare and swap operation.
    CompareAndSwap {
        key: String,
        expected: Option<Vec<u8>>,
        new_value: Vec<u8>,
        shard_id: String,
    },
}

impl TransactionOperation {
    /// Get the shard ID for this operation.
    pub fn shard_id(&self) -> &str {
        match self {
            Self::Read { shard_id, .. } => shard_id,
            Self::Write { shard_id, .. } => shard_id,
            Self::Delete { shard_id, .. } => shard_id,
            Self::CompareAndSwap { shard_id, .. } => shard_id,
        }
    }

    /// Get the key for this operation.
    pub fn key(&self) -> &str {
        match self {
            Self::Read { key, .. } => key,
            Self::Write { key, .. } => key,
            Self::Delete { key, .. } => key,
            Self::CompareAndSwap { key, .. } => key,
        }
    }

    /// Check if this is a write operation.
    pub fn is_write(&self) -> bool {
        !matches!(self, Self::Read { .. })
    }
}

// =============================================================================
// 2PC Messages
// =============================================================================

/// Messages for 2PC protocol.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TwoPhaseMessage {
    /// Prepare request from coordinator.
    PrepareRequest {
        txn_id: TransactionId,
        operations: Vec<TransactionOperation>,
    },
    /// Prepare response from participant.
    PrepareResponse {
        txn_id: TransactionId,
        vote: ParticipantVote,
        participant: NodeId,
    },
    /// Commit request from coordinator.
    CommitRequest {
        txn_id: TransactionId,
    },
    /// Commit acknowledgment from participant.
    CommitAck {
        txn_id: TransactionId,
        participant: NodeId,
    },
    /// Abort request from coordinator.
    AbortRequest {
        txn_id: TransactionId,
    },
    /// Abort acknowledgment from participant.
    AbortAck {
        txn_id: TransactionId,
        participant: NodeId,
    },
    /// Query transaction status (for recovery).
    StatusQuery {
        txn_id: TransactionId,
    },
    /// Status response.
    StatusResponse {
        txn_id: TransactionId,
        state: TransactionState,
    },
}

// =============================================================================
// Transaction Coordinator
// =============================================================================

/// Coordinator for distributed transactions using 2PC.
pub struct TransactionCoordinator {
    node_id: NodeId,
    transactions: RwLock<HashMap<TransactionId, DistributedTransaction>>,
    default_timeout_ms: u64,
    prepared_log: RwLock<HashSet<TransactionId>>,
}

impl TransactionCoordinator {
    /// Create a new transaction coordinator.
    pub fn new(node_id: NodeId) -> Self {
        Self {
            node_id,
            transactions: RwLock::new(HashMap::new()),
            default_timeout_ms: 30000,
            prepared_log: RwLock::new(HashSet::new()),
        }
    }

    /// Create with custom timeout.
    pub fn with_timeout(node_id: NodeId, timeout_ms: u64) -> Self {
        Self {
            node_id,
            transactions: RwLock::new(HashMap::new()),
            default_timeout_ms: timeout_ms,
            prepared_log: RwLock::new(HashSet::new()),
        }
    }

    /// Begin a new distributed transaction.
    pub fn begin_transaction(&self) -> TransactionId {
        let txn_id = TransactionId::generate();
        let txn = DistributedTransaction::new(
            txn_id.clone(),
            self.node_id.clone(),
            self.default_timeout_ms,
        );
        self.transactions
            .write()
            .unwrap()
            .insert(txn_id.clone(), txn);
        txn_id
    }

    /// Begin a transaction with a specific ID.
    pub fn begin_transaction_with_id(&self, txn_id: TransactionId) {
        let txn = DistributedTransaction::new(
            txn_id.clone(),
            self.node_id.clone(),
            self.default_timeout_ms,
        );
        self.transactions.write().unwrap().insert(txn_id, txn);
    }

    /// Add a participant to a transaction.
    pub fn add_participant(&self, txn_id: &TransactionId, node_id: NodeId) -> bool {
        if let Some(txn) = self.transactions.write().unwrap().get_mut(txn_id) {
            txn.add_participant(node_id);
            true
        } else {
            false
        }
    }

    /// Add an operation to a transaction.
    pub fn add_operation(&self, txn_id: &TransactionId, operation: TransactionOperation) -> bool {
        if let Some(txn) = self.transactions.write().unwrap().get_mut(txn_id) {
            txn.add_operation(operation);
            true
        } else {
            false
        }
    }

    /// Phase 1: Prepare - Generate prepare requests for all participants.
    pub fn prepare(&self, txn_id: &TransactionId) -> Option<Vec<(NodeId, TwoPhaseMessage)>> {
        let txns = self.transactions.read().unwrap();
        let txn = txns.get(txn_id)?;

        if txn.state != TransactionState::Preparing {
            return None;
        }

        let messages: Vec<_> = txn
            .participants
            .keys()
            .map(|node_id| {
                (
                    node_id.clone(),
                    TwoPhaseMessage::PrepareRequest {
                        txn_id: txn_id.clone(),
                        operations: txn.operations.clone(),
                    },
                )
            })
            .collect();

        Some(messages)
    }

    /// Handle prepare response from a participant.
    pub fn handle_prepare_response(
        &self,
        txn_id: &TransactionId,
        participant: &NodeId,
        vote: ParticipantVote,
    ) -> Option<TransactionState> {
        let mut txns = self.transactions.write().unwrap();
        let txn = txns.get_mut(txn_id)?;

        if let Some(p) = txn.participants.get_mut(participant) {
            p.record_prepare(vote);
        }

        // Check if we can make a decision
        if txn.any_abort() {
            txn.state = TransactionState::Aborting;
            Some(TransactionState::Aborting)
        } else if txn.all_prepared() {
            txn.state = TransactionState::Prepared;
            self.prepared_log.write().unwrap().insert(txn_id.clone());
            Some(TransactionState::Prepared)
        } else {
            None
        }
    }

    /// Phase 2: Commit - Generate commit requests for all participants.
    pub fn commit(&self, txn_id: &TransactionId) -> Option<Vec<(NodeId, TwoPhaseMessage)>> {
        let mut txns = self.transactions.write().unwrap();
        let txn = txns.get_mut(txn_id)?;

        if !txn.state.can_commit() {
            return None;
        }

        txn.state = TransactionState::Committing;

        let messages: Vec<_> = txn
            .participants
            .keys()
            .map(|node_id| {
                (
                    node_id.clone(),
                    TwoPhaseMessage::CommitRequest {
                        txn_id: txn_id.clone(),
                    },
                )
            })
            .collect();

        Some(messages)
    }

    /// Handle commit acknowledgment from a participant.
    pub fn handle_commit_ack(
        &self,
        txn_id: &TransactionId,
        participant: &NodeId,
    ) -> Option<TransactionState> {
        let mut txns = self.transactions.write().unwrap();
        let txn = txns.get_mut(txn_id)?;

        if let Some(p) = txn.participants.get_mut(participant) {
            p.record_commit();
        }

        if txn.all_committed() {
            txn.state = TransactionState::Committed;
            Some(TransactionState::Committed)
        } else {
            None
        }
    }

    /// Abort a transaction - Generate abort requests.
    pub fn abort(&self, txn_id: &TransactionId) -> Option<Vec<(NodeId, TwoPhaseMessage)>> {
        let mut txns = self.transactions.write().unwrap();
        let txn = txns.get_mut(txn_id)?;

        if !txn.state.can_abort() {
            return None;
        }

        txn.state = TransactionState::Aborting;

        let messages: Vec<_> = txn
            .participants
            .keys()
            .map(|node_id| {
                (
                    node_id.clone(),
                    TwoPhaseMessage::AbortRequest {
                        txn_id: txn_id.clone(),
                    },
                )
            })
            .collect();

        Some(messages)
    }

    /// Handle abort acknowledgment.
    pub fn handle_abort_ack(&self, txn_id: &TransactionId, _participant: &NodeId) -> bool {
        let mut txns = self.transactions.write().unwrap();
        if let Some(txn) = txns.get_mut(txn_id) {
            txn.state = TransactionState::Aborted;
            true
        } else {
            false
        }
    }

    /// Get transaction state.
    pub fn get_state(&self, txn_id: &TransactionId) -> Option<TransactionState> {
        self.transactions
            .read()
            .unwrap()
            .get(txn_id)
            .map(|t| t.state)
    }

    /// Get transaction details.
    pub fn get_transaction(&self, txn_id: &TransactionId) -> Option<DistributedTransaction> {
        self.transactions.read().unwrap().get(txn_id).cloned()
    }

    /// Check for timed out transactions.
    pub fn check_timeouts(&self) -> Vec<TransactionId> {
        self.transactions
            .read()
            .unwrap()
            .iter()
            .filter(|(_, txn)| txn.is_timed_out() && !txn.state.is_terminal())
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Clean up completed transactions.
    pub fn cleanup_completed(&self) -> usize {
        let mut txns = self.transactions.write().unwrap();
        let before = txns.len();
        txns.retain(|_, txn| !txn.state.is_terminal());
        before - txns.len()
    }

    /// Get active transaction count.
    pub fn active_count(&self) -> usize {
        self.transactions
            .read()
            .unwrap()
            .values()
            .filter(|t| !t.state.is_terminal())
            .count()
    }

    /// Check if a transaction was prepared (for recovery).
    pub fn was_prepared(&self, txn_id: &TransactionId) -> bool {
        self.prepared_log.read().unwrap().contains(txn_id)
    }
}

// =============================================================================
// Transaction Participant Handler
// =============================================================================

/// Handler for transaction participants.
pub struct ParticipantHandler {
    node_id: NodeId,
    pending_prepares: RwLock<HashMap<TransactionId, Vec<TransactionOperation>>>,
    prepared: RwLock<HashSet<TransactionId>>,
    committed: RwLock<HashSet<TransactionId>>,
}

impl ParticipantHandler {
    /// Create a new participant handler.
    pub fn new(node_id: NodeId) -> Self {
        Self {
            node_id,
            pending_prepares: RwLock::new(HashMap::new()),
            prepared: RwLock::new(HashSet::new()),
            committed: RwLock::new(HashSet::new()),
        }
    }

    /// Handle prepare request.
    pub fn handle_prepare(
        &self,
        txn_id: &TransactionId,
        operations: Vec<TransactionOperation>,
    ) -> TwoPhaseMessage {
        // In a real implementation, this would:
        // 1. Acquire locks on all keys
        // 2. Validate all operations can succeed
        // 3. Write to WAL
        // For now, we simulate success

        self.pending_prepares
            .write()
            .unwrap()
            .insert(txn_id.clone(), operations);
        self.prepared.write().unwrap().insert(txn_id.clone());

        TwoPhaseMessage::PrepareResponse {
            txn_id: txn_id.clone(),
            vote: ParticipantVote::Commit,
            participant: self.node_id.clone(),
        }
    }

    /// Handle prepare with validation function.
    pub fn handle_prepare_with_validation<F>(
        &self,
        txn_id: &TransactionId,
        operations: Vec<TransactionOperation>,
        validator: F,
    ) -> TwoPhaseMessage
    where
        F: FnOnce(&[TransactionOperation]) -> bool,
    {
        let vote = if validator(&operations) {
            self.pending_prepares
                .write()
                .unwrap()
                .insert(txn_id.clone(), operations);
            self.prepared.write().unwrap().insert(txn_id.clone());
            ParticipantVote::Commit
        } else {
            ParticipantVote::Abort
        };

        TwoPhaseMessage::PrepareResponse {
            txn_id: txn_id.clone(),
            vote,
            participant: self.node_id.clone(),
        }
    }

    /// Handle commit request.
    pub fn handle_commit(&self, txn_id: &TransactionId) -> TwoPhaseMessage {
        // Apply the prepared operations
        self.pending_prepares.write().unwrap().remove(txn_id);
        self.prepared.write().unwrap().remove(txn_id);
        self.committed.write().unwrap().insert(txn_id.clone());

        TwoPhaseMessage::CommitAck {
            txn_id: txn_id.clone(),
            participant: self.node_id.clone(),
        }
    }

    /// Handle abort request.
    pub fn handle_abort(&self, txn_id: &TransactionId) -> TwoPhaseMessage {
        // Rollback any prepared state
        self.pending_prepares.write().unwrap().remove(txn_id);
        self.prepared.write().unwrap().remove(txn_id);

        TwoPhaseMessage::AbortAck {
            txn_id: txn_id.clone(),
            participant: self.node_id.clone(),
        }
    }

    /// Check if a transaction is prepared.
    pub fn is_prepared(&self, txn_id: &TransactionId) -> bool {
        self.prepared.read().unwrap().contains(txn_id)
    }

    /// Check if a transaction is committed.
    pub fn is_committed(&self, txn_id: &TransactionId) -> bool {
        self.committed.read().unwrap().contains(txn_id)
    }

    /// Get pending prepare count.
    pub fn pending_count(&self) -> usize {
        self.pending_prepares.read().unwrap().len()
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transaction_id() {
        let id1 = TransactionId::new("txn_1");
        let id2 = TransactionId::generate();

        assert_eq!(id1.as_str(), "txn_1");
        assert!(id2.as_str().starts_with("txn_"));
    }

    #[test]
    fn test_transaction_state() {
        assert!(!TransactionState::Preparing.is_terminal());
        assert!(TransactionState::Committed.is_terminal());
        assert!(TransactionState::Aborted.is_terminal());

        assert!(TransactionState::Prepared.can_commit());
        assert!(!TransactionState::Preparing.can_commit());

        assert!(TransactionState::Preparing.can_abort());
        assert!(!TransactionState::Committed.can_abort());
    }

    #[test]
    fn test_distributed_transaction() {
        let txn_id = TransactionId::new("txn_1");
        let coordinator = NodeId::new("coord");
        let mut txn = DistributedTransaction::new(txn_id, coordinator, 30000);

        assert_eq!(txn.state, TransactionState::Preparing);
        assert_eq!(txn.participant_count(), 0);

        txn.add_participant(NodeId::new("node1"));
        txn.add_participant(NodeId::new("node2"));

        assert_eq!(txn.participant_count(), 2);
        assert!(!txn.all_prepared());

        txn.participants
            .get_mut(&NodeId::new("node1"))
            .unwrap()
            .record_prepare(ParticipantVote::Commit);

        assert!(!txn.all_prepared());
        assert_eq!(txn.prepared_count(), 1);

        txn.participants
            .get_mut(&NodeId::new("node2"))
            .unwrap()
            .record_prepare(ParticipantVote::Commit);

        assert!(txn.all_prepared());
        assert!(!txn.any_abort());
    }

    #[test]
    fn test_transaction_abort_vote() {
        let txn_id = TransactionId::new("txn_1");
        let mut txn = DistributedTransaction::new(txn_id, NodeId::new("coord"), 30000);

        txn.add_participant(NodeId::new("node1"));
        txn.add_participant(NodeId::new("node2"));

        txn.participants
            .get_mut(&NodeId::new("node1"))
            .unwrap()
            .record_prepare(ParticipantVote::Commit);
        txn.participants
            .get_mut(&NodeId::new("node2"))
            .unwrap()
            .record_prepare(ParticipantVote::Abort);

        assert!(!txn.all_prepared());
        assert!(txn.any_abort());
    }

    #[test]
    fn test_transaction_operation() {
        let write_op = TransactionOperation::Write {
            key: "user:1".to_string(),
            value: vec![1, 2, 3],
            shard_id: "shard_1".to_string(),
        };

        assert_eq!(write_op.key(), "user:1");
        assert_eq!(write_op.shard_id(), "shard_1");
        assert!(write_op.is_write());

        let read_op = TransactionOperation::Read {
            key: "user:2".to_string(),
            shard_id: "shard_2".to_string(),
        };

        assert!(!read_op.is_write());
    }

    #[test]
    fn test_coordinator_begin_transaction() {
        let coord = TransactionCoordinator::new(NodeId::new("coord"));
        let txn_id = coord.begin_transaction();

        assert!(coord.get_state(&txn_id).is_some());
        assert_eq!(
            coord.get_state(&txn_id).unwrap(),
            TransactionState::Preparing
        );
    }

    #[test]
    fn test_coordinator_add_participant() {
        let coord = TransactionCoordinator::new(NodeId::new("coord"));
        let txn_id = coord.begin_transaction();

        assert!(coord.add_participant(&txn_id, NodeId::new("node1")));
        assert!(coord.add_participant(&txn_id, NodeId::new("node2")));

        let txn = coord.get_transaction(&txn_id).unwrap();
        assert_eq!(txn.participant_count(), 2);
    }

    #[test]
    fn test_coordinator_prepare() {
        let coord = TransactionCoordinator::new(NodeId::new("coord"));
        let txn_id = coord.begin_transaction();

        coord.add_participant(&txn_id, NodeId::new("node1"));
        coord.add_participant(&txn_id, NodeId::new("node2"));

        let messages = coord.prepare(&txn_id).unwrap();
        assert_eq!(messages.len(), 2);

        for (_, msg) in &messages {
            match msg {
                TwoPhaseMessage::PrepareRequest { txn_id: id, .. } => {
                    assert_eq!(id, &txn_id);
                }
                _ => panic!("Expected PrepareRequest"),
            }
        }
    }

    #[test]
    fn test_coordinator_full_commit() {
        let coord = TransactionCoordinator::new(NodeId::new("coord"));
        let txn_id = coord.begin_transaction();

        let node1 = NodeId::new("node1");
        let node2 = NodeId::new("node2");

        coord.add_participant(&txn_id, node1.clone());
        coord.add_participant(&txn_id, node2.clone());

        // Phase 1: Prepare
        coord.prepare(&txn_id);

        // Both participants vote commit
        coord.handle_prepare_response(&txn_id, &node1, ParticipantVote::Commit);
        let state = coord.handle_prepare_response(&txn_id, &node2, ParticipantVote::Commit);

        assert_eq!(state, Some(TransactionState::Prepared));

        // Phase 2: Commit
        let messages = coord.commit(&txn_id).unwrap();
        assert_eq!(messages.len(), 2);

        // Both participants acknowledge
        coord.handle_commit_ack(&txn_id, &node1);
        let final_state = coord.handle_commit_ack(&txn_id, &node2);

        assert_eq!(final_state, Some(TransactionState::Committed));
    }

    #[test]
    fn test_coordinator_abort_on_vote() {
        let coord = TransactionCoordinator::new(NodeId::new("coord"));
        let txn_id = coord.begin_transaction();

        let node1 = NodeId::new("node1");
        let node2 = NodeId::new("node2");

        coord.add_participant(&txn_id, node1.clone());
        coord.add_participant(&txn_id, node2.clone());

        coord.prepare(&txn_id);

        coord.handle_prepare_response(&txn_id, &node1, ParticipantVote::Commit);
        let state = coord.handle_prepare_response(&txn_id, &node2, ParticipantVote::Abort);

        assert_eq!(state, Some(TransactionState::Aborting));
    }

    #[test]
    fn test_participant_handler() {
        let handler = ParticipantHandler::new(NodeId::new("node1"));
        let txn_id = TransactionId::new("txn_1");

        let ops = vec![TransactionOperation::Write {
            key: "key1".to_string(),
            value: vec![1, 2, 3],
            shard_id: "shard_1".to_string(),
        }];

        // Handle prepare
        let response = handler.handle_prepare(&txn_id, ops);
        match response {
            TwoPhaseMessage::PrepareResponse { vote, .. } => {
                assert_eq!(vote, ParticipantVote::Commit);
            }
            _ => panic!("Expected PrepareResponse"),
        }

        assert!(handler.is_prepared(&txn_id));
        assert!(!handler.is_committed(&txn_id));

        // Handle commit
        let commit_response = handler.handle_commit(&txn_id);
        match commit_response {
            TwoPhaseMessage::CommitAck { .. } => {}
            _ => panic!("Expected CommitAck"),
        }

        assert!(!handler.is_prepared(&txn_id));
        assert!(handler.is_committed(&txn_id));
    }

    #[test]
    fn test_participant_abort() {
        let handler = ParticipantHandler::new(NodeId::new("node1"));
        let txn_id = TransactionId::new("txn_1");

        let ops = vec![TransactionOperation::Write {
            key: "key1".to_string(),
            value: vec![1, 2, 3],
            shard_id: "shard_1".to_string(),
        }];

        handler.handle_prepare(&txn_id, ops);
        assert!(handler.is_prepared(&txn_id));

        let abort_response = handler.handle_abort(&txn_id);
        match abort_response {
            TwoPhaseMessage::AbortAck { .. } => {}
            _ => panic!("Expected AbortAck"),
        }

        assert!(!handler.is_prepared(&txn_id));
        assert!(!handler.is_committed(&txn_id));
    }

    #[test]
    fn test_coordinator_cleanup() {
        let coord = TransactionCoordinator::new(NodeId::new("coord"));

        // Create and complete a transaction
        let txn_id = coord.begin_transaction();
        coord.add_participant(&txn_id, NodeId::new("node1"));
        coord.prepare(&txn_id);
        coord.handle_prepare_response(&txn_id, &NodeId::new("node1"), ParticipantVote::Commit);
        coord.commit(&txn_id);
        coord.handle_commit_ack(&txn_id, &NodeId::new("node1"));

        assert_eq!(coord.get_state(&txn_id), Some(TransactionState::Committed));

        let cleaned = coord.cleanup_completed();
        assert_eq!(cleaned, 1);
        assert!(coord.get_state(&txn_id).is_none());
    }

    #[test]
    fn test_active_count() {
        let coord = TransactionCoordinator::new(NodeId::new("coord"));

        let txn1 = coord.begin_transaction();
        let txn2 = coord.begin_transaction();

        assert_eq!(coord.active_count(), 2);

        // Complete txn1
        coord.add_participant(&txn1, NodeId::new("node1"));
        coord.prepare(&txn1);
        coord.handle_prepare_response(&txn1, &NodeId::new("node1"), ParticipantVote::Commit);
        coord.commit(&txn1);
        coord.handle_commit_ack(&txn1, &NodeId::new("node1"));

        assert_eq!(coord.active_count(), 1);
    }
}
