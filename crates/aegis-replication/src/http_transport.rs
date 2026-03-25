//! HTTP Transport for Raft Communication
//!
//! Implements the `Transport` trait using HTTP (reqwest) for inter-node
//! Raft message passing. Each node is identified by a `NodeId` mapped to
//! an HTTP URL. Messages are sent as JSON POSTs to `{url}/api/v1/cluster/raft`.
//!
//! Incoming messages are received via an internal channel — an external HTTP
//! handler (e.g., an Axum route) should push received messages into the channel
//! using `push_message()`.
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use crate::node::NodeId;
use crate::transport::{Message, Transport, TransportError};
use std::collections::HashMap;
use std::sync::mpsc;
use std::sync::{Mutex, RwLock};
use std::time::Duration;

/// HTTP-based transport for Raft consensus communication.
///
/// Sends outgoing Raft messages as JSON POST requests to peer nodes.
/// Receives incoming messages via an internal channel that should be
/// fed by an external HTTP endpoint handler.
pub struct HttpTransport {
    /// Mapping from node IDs to their HTTP base URLs (e.g., "http://127.0.0.1:9091").
    peer_urls: RwLock<HashMap<NodeId, String>>,
    /// HTTP client for sending requests to peers.
    client: reqwest::blocking::Client,
    /// Sender side of the incoming message channel.
    incoming_tx: mpsc::Sender<Message>,
    /// Receiver side of the incoming message channel.
    incoming_rx: Mutex<mpsc::Receiver<Message>>,
}

impl HttpTransport {
    /// Create a new HTTP transport with the given peer URL mappings.
    ///
    /// # Arguments
    /// * `peer_urls` - A map of `NodeId` to base HTTP URL for each peer node.
    ///
    /// # Example
    /// ```ignore
    /// use std::collections::HashMap;
    /// use aegis_replication::node::NodeId;
    /// use aegis_replication::http_transport::HttpTransport;
    ///
    /// let mut peers = HashMap::new();
    /// peers.insert(NodeId::new("node2"), "http://127.0.0.1:9091".to_string());
    /// peers.insert(NodeId::new("node3"), "http://127.0.0.1:7001".to_string());
    /// let transport = HttpTransport::new(peers);
    /// ```
    pub fn new(peer_urls: HashMap<NodeId, String>) -> Self {
        let (tx, rx) = mpsc::channel();
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(5))
            .connect_timeout(Duration::from_secs(2))
            .build()
            .expect("failed to build HTTP client");

        Self {
            peer_urls: RwLock::new(peer_urls),
            client,
            incoming_tx: tx,
            incoming_rx: Mutex::new(rx),
        }
    }

    /// Get a sender handle for pushing incoming messages into this transport.
    ///
    /// Use this to integrate with an HTTP server endpoint. When a Raft message
    /// arrives via HTTP, deserialize it and call `sender.send(message)`.
    pub fn sender(&self) -> mpsc::Sender<Message> {
        self.incoming_tx.clone()
    }

    /// Push an incoming message into the receive channel.
    ///
    /// This is the primary way to feed messages from an external HTTP handler
    /// into the transport layer.
    pub fn push_message(&self, message: Message) -> Result<(), TransportError> {
        self.incoming_tx
            .send(message)
            .map_err(|e| TransportError::Unknown(format!("Failed to push message: {}", e)))
    }

    /// Add or update a peer URL mapping.
    pub fn add_peer(&self, node_id: NodeId, url: String) {
        self.peer_urls
            .write()
            .expect("http_transport peer_urls lock poisoned")
            .insert(node_id, url);
    }

    /// Remove a peer URL mapping.
    pub fn remove_peer(&self, node_id: &NodeId) {
        self.peer_urls
            .write()
            .expect("http_transport peer_urls lock poisoned")
            .remove(node_id);
    }

    /// Get the URL for a peer node.
    pub fn peer_url(&self, node_id: &NodeId) -> Option<String> {
        self.peer_urls
            .read()
            .expect("http_transport peer_urls lock poisoned")
            .get(node_id)
            .cloned()
    }

    /// Send a message to a specific URL endpoint.
    fn send_to_url(&self, url: &str, message: &Message) -> Result<(), TransportError> {
        let endpoint = format!("{}/api/v1/cluster/raft", url.trim_end_matches('/'));

        self.client
            .post(&endpoint)
            .json(message)
            .send()
            .map_err(|e| TransportError::ConnectionFailed(format!("{}: {}", endpoint, e)))?;

        Ok(())
    }
}

impl Transport for HttpTransport {
    /// Send a message to the target node via HTTP POST.
    ///
    /// The message's `to` field is used to look up the peer's URL.
    /// The message is serialized as JSON and sent to `{peer_url}/api/v1/cluster/raft`.
    fn send(&self, message: Message) -> Result<(), TransportError> {
        let url = {
            let urls = self
                .peer_urls
                .read()
                .expect("http_transport peer_urls lock poisoned");
            urls.get(&message.to).cloned().ok_or_else(|| {
                TransportError::ConnectionFailed(format!(
                    "No URL configured for node {}",
                    message.to
                ))
            })?
        };

        self.send_to_url(&url, &message)
    }

    /// Receive a message (blocking).
    ///
    /// Blocks until a message is available in the incoming channel.
    /// Messages are pushed into this channel by external HTTP handlers
    /// via `push_message()` or the `sender()` handle.
    fn recv(&self) -> Result<Message, TransportError> {
        let rx = self
            .incoming_rx
            .lock()
            .expect("http_transport incoming_rx lock poisoned");
        rx.recv().map_err(|_| TransportError::Disconnected)
    }

    /// Try to receive a message (non-blocking).
    ///
    /// Returns `None` if no message is currently available.
    fn try_recv(&self) -> Option<Message> {
        let rx = self
            .incoming_rx
            .lock()
            .expect("http_transport incoming_rx lock poisoned");
        rx.try_recv().ok()
    }

    /// Broadcast a message to all specified peers via HTTP POST.
    ///
    /// Sends the message to each peer independently. Failures for individual
    /// peers do not affect delivery to other peers.
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
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::MessageType;

    #[test]
    fn test_http_transport_creation() {
        let mut peers = HashMap::new();
        peers.insert(NodeId::new("node2"), "http://127.0.0.1:9091".to_string());
        peers.insert(NodeId::new("node3"), "http://127.0.0.1:7001".to_string());

        let transport = HttpTransport::new(peers);

        assert_eq!(
            transport.peer_url(&NodeId::new("node2")),
            Some("http://127.0.0.1:9091".to_string())
        );
        assert_eq!(
            transport.peer_url(&NodeId::new("node3")),
            Some("http://127.0.0.1:7001".to_string())
        );
        assert_eq!(transport.peer_url(&NodeId::new("node4")), None);
    }

    #[test]
    fn test_http_transport_add_remove_peer() {
        let transport = HttpTransport::new(HashMap::new());

        transport.add_peer(NodeId::new("node2"), "http://127.0.0.1:9091".to_string());
        assert_eq!(
            transport.peer_url(&NodeId::new("node2")),
            Some("http://127.0.0.1:9091".to_string())
        );

        transport.remove_peer(&NodeId::new("node2"));
        assert_eq!(transport.peer_url(&NodeId::new("node2")), None);
    }

    #[test]
    fn test_http_transport_push_and_recv() {
        let transport = HttpTransport::new(HashMap::new());

        let msg = Message::heartbeat(NodeId::new("node1"), NodeId::new("node2"), 1);
        transport.push_message(msg).unwrap();

        let received = transport.try_recv().unwrap();
        assert_eq!(received.message_type, MessageType::Heartbeat);
        assert_eq!(received.from.as_str(), "node1");
        assert_eq!(received.to.as_str(), "node2");
    }

    #[test]
    fn test_http_transport_try_recv_empty() {
        let transport = HttpTransport::new(HashMap::new());
        assert!(transport.try_recv().is_none());
    }

    #[test]
    fn test_http_transport_sender_channel() {
        let transport = HttpTransport::new(HashMap::new());
        let sender = transport.sender();

        let msg = Message::heartbeat(NodeId::new("node1"), NodeId::new("node2"), 5);
        sender.send(msg).unwrap();

        let received = transport.try_recv().unwrap();
        assert_eq!(received.term, 5);
    }

    #[test]
    fn test_http_transport_send_no_peer_url() {
        let transport = HttpTransport::new(HashMap::new());

        let msg = Message::heartbeat(NodeId::new("node1"), NodeId::new("unknown"), 1);
        let result = transport.send(msg);

        assert!(result.is_err());
        match result.unwrap_err() {
            TransportError::ConnectionFailed(s) => {
                assert!(s.contains("No URL configured"));
            }
            other => panic!("Expected ConnectionFailed, got {:?}", other),
        }
    }

    #[test]
    fn test_http_transport_multiple_messages() {
        let transport = HttpTransport::new(HashMap::new());

        for i in 0..5 {
            let msg = Message::heartbeat(NodeId::new("node1"), NodeId::new("node2"), i);
            transport.push_message(msg).unwrap();
        }

        for i in 0..5 {
            let received = transport.try_recv().unwrap();
            assert_eq!(received.term, i);
        }

        assert!(transport.try_recv().is_none());
    }
}
