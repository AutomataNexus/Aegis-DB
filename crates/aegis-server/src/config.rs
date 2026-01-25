//! Aegis Server Configuration
//!
//! Server configuration management for binding, TLS, and operational settings.
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

// =============================================================================
// Server Configuration
// =============================================================================

/// Server configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub max_connections: usize,
    pub request_timeout_secs: u64,
    pub body_limit_bytes: usize,
    pub enable_cors: bool,
    pub tls: Option<TlsConfig>,
    pub data_dir: Option<String>,
    /// Unique node ID
    pub node_id: String,
    /// Human-readable node name (e.g., "AxonML", "NexusScribe")
    pub node_name: Option<String>,
    /// Cluster name
    pub cluster_name: String,
    /// Peer addresses for cluster membership
    pub peers: Vec<String>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 3000,
            max_connections: 10000,
            request_timeout_secs: 30,
            body_limit_bytes: 10 * 1024 * 1024, // 10MB
            enable_cors: true,
            tls: None,
            data_dir: None,
            node_id: generate_node_id(),
            node_name: None,
            cluster_name: "aegis-cluster".to_string(),
            peers: Vec::new(),
        }
    }
}

/// Generate a unique node ID based on timestamp and random suffix
fn generate_node_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    format!("node-{:x}", timestamp as u64)
}

impl ServerConfig {
    /// Create a new server config with the specified host and port.
    pub fn new(host: &str, port: u16) -> Self {
        Self {
            host: host.to_string(),
            port,
            ..Default::default()
        }
    }

    /// Get the socket address for binding.
    pub fn socket_addr(&self) -> SocketAddr {
        format!("{}:{}", self.host, self.port)
            .parse()
            .unwrap_or_else(|_| SocketAddr::from(([127, 0, 0, 1], self.port)))
    }

    /// Enable TLS with the specified certificate and key paths.
    pub fn with_tls(mut self, cert_path: &str, key_path: &str) -> Self {
        self.tls = Some(TlsConfig {
            cert_path: cert_path.to_string(),
            key_path: key_path.to_string(),
        });
        self
    }

    /// Set the maximum number of concurrent connections.
    pub fn with_max_connections(mut self, max: usize) -> Self {
        self.max_connections = max;
        self
    }

    /// Set the request timeout in seconds.
    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.request_timeout_secs = secs;
        self
    }

    /// Set the data directory for persistence.
    pub fn with_data_dir(mut self, data_dir: Option<String>) -> Self {
        self.data_dir = data_dir;
        self
    }

    /// Set the node ID.
    pub fn with_node_id(mut self, node_id: Option<String>) -> Self {
        if let Some(id) = node_id {
            self.node_id = id;
        }
        self
    }

    /// Set the node name.
    pub fn with_node_name(mut self, node_name: Option<String>) -> Self {
        self.node_name = node_name;
        self
    }

    /// Set the cluster name.
    pub fn with_cluster_name(mut self, cluster_name: String) -> Self {
        self.cluster_name = cluster_name;
        self
    }

    /// Set the peer addresses.
    pub fn with_peers(mut self, peers: Vec<String>) -> Self {
        self.peers = peers;
        self
    }

    /// Get the full address of this node.
    pub fn address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

// =============================================================================
// TLS Configuration
// =============================================================================

/// TLS configuration for HTTPS.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsConfig {
    pub cert_path: String,
    pub key_path: String,
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ServerConfig::default();
        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.port, 3000);
        assert!(config.tls.is_none());
    }

    #[test]
    fn test_socket_addr() {
        let config = ServerConfig::new("0.0.0.0", 8080);
        let addr = config.socket_addr();
        assert_eq!(addr.port(), 8080);
    }
}
