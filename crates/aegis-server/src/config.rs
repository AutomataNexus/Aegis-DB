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
        }
    }
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
