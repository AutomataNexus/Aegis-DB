//! Aegis Replication Transport
//!
//! Network transport layer for Raft message passing.
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use crate::node::NodeId;
use crate::raft::{
    AppendEntriesRequest, AppendEntriesResponse, InstallSnapshotRequest, InstallSnapshotResponse,
    VoteRequest, VoteResponse,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

// =============================================================================
// Message Type
// =============================================================================

/// Types of messages in the Raft protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageType {
    VoteRequest,
    VoteResponse,
    AppendEntries,
    AppendEntriesResponse,
    InstallSnapshot,
    InstallSnapshotResponse,
    ClientRequest,
    ClientResponse,
    Heartbeat,
}

// =============================================================================
// Message
// =============================================================================

/// A message in the Raft protocol.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub message_type: MessageType,
    pub from: NodeId,
    pub to: NodeId,
    pub term: u64,
    pub payload: MessagePayload,
    pub timestamp: u64,
}

impl Message {
    /// Create a new message.
    pub fn new(
        message_type: MessageType,
        from: NodeId,
        to: NodeId,
        term: u64,
        payload: MessagePayload,
    ) -> Self {
        Self {
            message_type,
            from,
            to,
            term,
            payload,
            timestamp: current_timestamp(),
        }
    }

    /// Create a vote request message.
    pub fn vote_request(from: NodeId, to: NodeId, request: VoteRequest) -> Self {
        Self::new(
            MessageType::VoteRequest,
            from,
            to,
            request.term,
            MessagePayload::VoteRequest(request),
        )
    }

    /// Create a vote response message.
    pub fn vote_response(from: NodeId, to: NodeId, response: VoteResponse) -> Self {
        Self::new(
            MessageType::VoteResponse,
            from,
            to,
            response.term,
            MessagePayload::VoteResponse(response),
        )
    }

    /// Create an append entries message.
    pub fn append_entries(from: NodeId, to: NodeId, request: AppendEntriesRequest) -> Self {
        Self::new(
            MessageType::AppendEntries,
            from,
            to,
            request.term,
            MessagePayload::AppendEntries(request),
        )
    }

    /// Create an append entries response message.
    pub fn append_entries_response(
        from: NodeId,
        to: NodeId,
        response: AppendEntriesResponse,
    ) -> Self {
        Self::new(
            MessageType::AppendEntriesResponse,
            from,
            to,
            response.term,
            MessagePayload::AppendEntriesResponse(response),
        )
    }

    /// Create a heartbeat message.
    pub fn heartbeat(from: NodeId, to: NodeId, term: u64) -> Self {
        Self::new(
            MessageType::Heartbeat,
            from,
            to,
            term,
            MessagePayload::Heartbeat,
        )
    }

    /// Serialize the message.
    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }

    /// Deserialize a message.
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        serde_json::from_slice(bytes).ok()
    }
}

// =============================================================================
// Message Payload
// =============================================================================

/// Payload of a Raft message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessagePayload {
    VoteRequest(VoteRequest),
    VoteResponse(VoteResponse),
    AppendEntries(AppendEntriesRequest),
    AppendEntriesResponse(AppendEntriesResponse),
    InstallSnapshot(InstallSnapshotRequest),
    InstallSnapshotResponse(InstallSnapshotResponse),
    ClientRequest(ClientRequest),
    ClientResponse(ClientResponse),
    Heartbeat,
    Empty,
}

// =============================================================================
// Client Request/Response
// =============================================================================

/// A client request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientRequest {
    pub request_id: String,
    pub operation: ClientOperation,
}

/// Client operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClientOperation {
    Get { key: String },
    Set { key: String, value: Vec<u8> },
    Delete { key: String },
}

/// A client response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientResponse {
    pub request_id: String,
    pub success: bool,
    pub value: Option<Vec<u8>>,
    pub error: Option<String>,
    pub leader_hint: Option<NodeId>,
}

impl ClientResponse {
    pub fn success(request_id: String, value: Option<Vec<u8>>) -> Self {
        Self {
            request_id,
            success: true,
            value,
            error: None,
            leader_hint: None,
        }
    }

    pub fn error(request_id: String, error: impl Into<String>) -> Self {
        Self {
            request_id,
            success: false,
            value: None,
            error: Some(error.into()),
            leader_hint: None,
        }
    }

    pub fn not_leader(request_id: String, leader: Option<NodeId>) -> Self {
        Self {
            request_id,
            success: false,
            value: None,
            error: Some("Not the leader".to_string()),
            leader_hint: leader,
        }
    }
}

// =============================================================================
// Transport Trait
// =============================================================================

/// Transport layer for Raft communication.
pub trait Transport: Send + Sync {
    /// Send a message to a node.
    fn send(&self, message: Message) -> Result<(), TransportError>;

    /// Receive a message (blocking).
    fn recv(&self) -> Result<Message, TransportError>;

    /// Try to receive a message (non-blocking).
    fn try_recv(&self) -> Option<Message>;

    /// Broadcast a message to all peers.
    fn broadcast(&self, message: Message, peers: &[NodeId]) -> Vec<Result<(), TransportError>>;
}

// =============================================================================
// Transport Error
// =============================================================================

/// Errors that can occur during transport.
#[derive(Debug, Clone)]
pub enum TransportError {
    ConnectionFailed(String),
    Timeout,
    Disconnected,
    SerializationError(String),
    ChannelFull,
    Unknown(String),
}

impl std::fmt::Display for TransportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ConnectionFailed(addr) => write!(f, "Connection failed: {}", addr),
            Self::Timeout => write!(f, "Timeout"),
            Self::Disconnected => write!(f, "Disconnected"),
            Self::SerializationError(e) => write!(f, "Serialization error: {}", e),
            Self::ChannelFull => write!(f, "Channel full"),
            Self::Unknown(e) => write!(f, "Unknown error: {}", e),
        }
    }
}

impl std::error::Error for TransportError {}

// =============================================================================
// In-Memory Transport (for testing)
// =============================================================================

/// In-memory transport for testing.
pub struct InMemoryTransport {
    node_id: NodeId,
    inboxes: Arc<RwLock<HashMap<NodeId, Vec<Message>>>>,
}

impl InMemoryTransport {
    /// Create a new in-memory transport network.
    pub fn new_network(nodes: &[NodeId]) -> HashMap<NodeId, Self> {
        let inboxes = Arc::new(RwLock::new(HashMap::new()));

        for node in nodes {
            inboxes.write().expect("transport inboxes lock poisoned").insert(node.clone(), Vec::new());
        }

        nodes
            .iter()
            .map(|id| {
                (
                    id.clone(),
                    Self {
                        node_id: id.clone(),
                        inboxes: Arc::clone(&inboxes),
                    },
                )
            })
            .collect()
    }

    /// Create a single transport (for testing).
    pub fn new(node_id: NodeId) -> Self {
        let inboxes = Arc::new(RwLock::new(HashMap::new()));
        inboxes.write().expect("transport inboxes lock poisoned").insert(node_id.clone(), Vec::new());
        Self { node_id, inboxes }
    }
}

impl Transport for InMemoryTransport {
    fn send(&self, message: Message) -> Result<(), TransportError> {
        let mut inboxes = self.inboxes.write().expect("transport inboxes lock poisoned");
        if let Some(inbox) = inboxes.get_mut(&message.to) {
            inbox.push(message);
            Ok(())
        } else {
            Err(TransportError::ConnectionFailed(message.to.to_string()))
        }
    }

    fn recv(&self) -> Result<Message, TransportError> {
        loop {
            if let Some(msg) = self.try_recv() {
                return Ok(msg);
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }

    fn try_recv(&self) -> Option<Message> {
        let mut inboxes = self.inboxes.write().expect("transport inboxes lock poisoned");
        if let Some(inbox) = inboxes.get_mut(&self.node_id) {
            if !inbox.is_empty() {
                return Some(inbox.remove(0));
            }
        }
        None
    }

    fn broadcast(&self, message: Message, peers: &[NodeId]) -> Vec<Result<(), TransportError>> {
        peers
            .iter()
            .map(|peer| {
                let mut msg = message.clone();
                msg.to = peer.clone();
                self.send(msg)
            })
            .collect()
    }
}

// =============================================================================
// Connection Pool
// =============================================================================

/// Connection pool for managing peer connections.
pub struct ConnectionPool {
    connections: RwLock<HashMap<NodeId, ConnectionState>>,
    max_connections: usize,
}

/// State of a connection.
#[derive(Debug, Clone)]
pub struct ConnectionState {
    pub node_id: NodeId,
    pub address: String,
    pub connected: bool,
    pub last_activity: u64,
    pub retry_count: u32,
}

impl ConnectionPool {
    /// Create a new connection pool.
    pub fn new(max_connections: usize) -> Self {
        Self {
            connections: RwLock::new(HashMap::new()),
            max_connections,
        }
    }

    /// Add a connection.
    pub fn add(&self, node_id: NodeId, address: String) {
        let mut conns = self.connections.write().expect("connection pool lock poisoned");
        if conns.len() < self.max_connections {
            conns.insert(
                node_id.clone(),
                ConnectionState {
                    node_id,
                    address,
                    connected: false,
                    last_activity: current_timestamp(),
                    retry_count: 0,
                },
            );
        }
    }

    /// Remove a connection.
    pub fn remove(&self, node_id: &NodeId) {
        self.connections.write().expect("connection pool lock poisoned").remove(node_id);
    }

    /// Get a connection.
    pub fn get(&self, node_id: &NodeId) -> Option<ConnectionState> {
        self.connections.read().expect("connection pool lock poisoned").get(node_id).cloned()
    }

    /// Mark a connection as connected.
    pub fn mark_connected(&self, node_id: &NodeId) {
        if let Some(conn) = self.connections.write().expect("connection pool lock poisoned").get_mut(node_id) {
            conn.connected = true;
            conn.last_activity = current_timestamp();
            conn.retry_count = 0;
        }
    }

    /// Mark a connection as disconnected.
    pub fn mark_disconnected(&self, node_id: &NodeId) {
        if let Some(conn) = self.connections.write().expect("connection pool lock poisoned").get_mut(node_id) {
            conn.connected = false;
            conn.retry_count += 1;
        }
    }

    /// Get all connected nodes.
    pub fn connected_nodes(&self) -> Vec<NodeId> {
        self.connections
            .read()
            .expect("connection pool lock poisoned")
            .values()
            .filter(|c| c.connected)
            .map(|c| c.node_id.clone())
            .collect()
    }

    /// Get connection count.
    pub fn len(&self) -> usize {
        self.connections.read().expect("connection pool lock poisoned").len()
    }

    /// Check if pool is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_serialization() {
        let request = VoteRequest {
            term: 1,
            candidate_id: NodeId::new("node1"),
            last_log_index: 0,
            last_log_term: 0,
        };

        let msg = Message::vote_request(
            NodeId::new("node1"),
            NodeId::new("node2"),
            request,
        );

        let bytes = msg.to_bytes();
        let restored = Message::from_bytes(&bytes).unwrap();

        assert_eq!(restored.message_type, MessageType::VoteRequest);
        assert_eq!(restored.from.as_str(), "node1");
        assert_eq!(restored.to.as_str(), "node2");
    }

    #[test]
    fn test_in_memory_transport() {
        let nodes = vec![NodeId::new("node1"), NodeId::new("node2")];
        let transports = InMemoryTransport::new_network(&nodes);

        let t1 = &transports[&NodeId::new("node1")];
        let t2 = &transports[&NodeId::new("node2")];

        let msg = Message::heartbeat(NodeId::new("node1"), NodeId::new("node2"), 1);
        t1.send(msg).unwrap();

        let received = t2.try_recv().unwrap();
        assert_eq!(received.message_type, MessageType::Heartbeat);
        assert_eq!(received.from.as_str(), "node1");
    }

    #[test]
    fn test_broadcast() {
        let nodes = vec![
            NodeId::new("node1"),
            NodeId::new("node2"),
            NodeId::new("node3"),
        ];
        let transports = InMemoryTransport::new_network(&nodes);

        let t1 = &transports[&NodeId::new("node1")];

        let msg = Message::heartbeat(NodeId::new("node1"), NodeId::new("node1"), 1);
        let peers = vec![NodeId::new("node2"), NodeId::new("node3")];
        let results = t1.broadcast(msg, &peers);

        assert!(results.iter().all(|r| r.is_ok()));
    }

    #[test]
    fn test_connection_pool() {
        let pool = ConnectionPool::new(10);

        pool.add(NodeId::new("node1"), "127.0.0.1:5000".to_string());
        pool.add(NodeId::new("node2"), "127.0.0.1:5001".to_string());

        assert_eq!(pool.len(), 2);

        pool.mark_connected(&NodeId::new("node1"));
        let connected = pool.connected_nodes();
        assert_eq!(connected.len(), 1);
        assert_eq!(connected[0].as_str(), "node1");

        pool.mark_disconnected(&NodeId::new("node1"));
        let state = pool.get(&NodeId::new("node1")).unwrap();
        assert!(!state.connected);
        assert_eq!(state.retry_count, 1);
    }

    #[test]
    fn test_client_response() {
        let success = ClientResponse::success("req1".to_string(), Some(b"value".to_vec()));
        assert!(success.success);
        assert_eq!(success.value, Some(b"value".to_vec()));

        let error = ClientResponse::error("req2".to_string(), "failed");
        assert!(!error.success);
        assert_eq!(error.error, Some("failed".to_string()));

        let not_leader = ClientResponse::not_leader("req3".to_string(), Some(NodeId::new("leader")));
        assert!(!not_leader.success);
        assert_eq!(not_leader.leader_hint, Some(NodeId::new("leader")));
    }
}
