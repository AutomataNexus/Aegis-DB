//! Aegis Client Configuration
//!
//! Configuration types for client connections.
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use crate::error::ClientError;
use serde::{Deserialize, Serialize};
use std::time::Duration;

// =============================================================================
// Client Configuration
// =============================================================================

/// Complete client configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[derive(Default)]
pub struct ClientConfig {
    pub connection: ConnectionConfig,
    pub pool: PoolConfig,
    pub retry: RetryConfig,
    pub timeout: TimeoutConfig,
}


impl ClientConfig {
    /// Create a new client configuration.
    pub fn new(host: impl Into<String>, port: u16, database: impl Into<String>) -> Self {
        Self {
            connection: ConnectionConfig {
                host: host.into(),
                port,
                database: database.into(),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    /// Parse configuration from a URL.
    pub fn from_url(url: &str) -> Result<Self, ClientError> {
        let url = url
            .strip_prefix("aegis://")
            .ok_or_else(|| ClientError::InvalidUrl("URL must start with aegis://".to_string()))?;

        let (auth_host, path) = url
            .split_once('/')
            .unwrap_or((url, ""));

        let (auth, host_port) = if auth_host.contains('@') {
            let parts: Vec<&str> = auth_host.splitn(2, '@').collect();
            (Some(parts[0]), parts[1])
        } else {
            (None, auth_host)
        };

        let (host, port) = if host_port.contains(':') {
            let parts: Vec<&str> = host_port.splitn(2, ':').collect();
            let port: u16 = parts[1]
                .parse()
                .map_err(|_| ClientError::InvalidUrl("Invalid port".to_string()))?;
            (parts[0].to_string(), port)
        } else {
            (host_port.to_string(), 9090) // Aegis default port
        };

        let database = if path.is_empty() {
            "default".to_string()
        } else {
            path.split('?').next().unwrap_or("default").to_string()
        };

        let (username, password) = if let Some(auth) = auth {
            if auth.contains(':') {
                let parts: Vec<&str> = auth.splitn(2, ':').collect();
                (Some(parts[0].to_string()), Some(parts[1].to_string()))
            } else {
                (Some(auth.to_string()), None)
            }
        } else {
            (None, None)
        };

        Ok(Self {
            connection: ConnectionConfig {
                host,
                port,
                database,
                username,
                password,
                ..Default::default()
            },
            ..Default::default()
        })
    }

    /// Set the pool size.
    pub fn with_pool_size(mut self, min: usize, max: usize) -> Self {
        self.pool.min_connections = min;
        self.pool.max_connections = max;
        self
    }

    /// Set connection timeout.
    pub fn with_connect_timeout(mut self, timeout: Duration) -> Self {
        self.timeout.connect = timeout;
        self
    }

    /// Set query timeout.
    pub fn with_query_timeout(mut self, timeout: Duration) -> Self {
        self.timeout.query = timeout;
        self
    }
}

// =============================================================================
// Connection Configuration
// =============================================================================

/// Configuration for a single connection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionConfig {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: Option<String>,
    pub password: Option<String>,
    pub ssl_mode: SslMode,
    pub application_name: Option<String>,
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            host: "localhost".to_string(),
            port: 9090, // Aegis default port
            database: "default".to_string(),
            username: None,
            password: None,
            ssl_mode: SslMode::Prefer,
            application_name: None,
        }
    }
}

impl ConnectionConfig {
    /// Create a connection string.
    pub fn connection_string(&self) -> String {
        let mut parts = vec![format!("host={}", self.host), format!("port={}", self.port)];

        parts.push(format!("dbname={}", self.database));

        if let Some(ref user) = self.username {
            parts.push(format!("user={}", user));
        }

        if let Some(ref app) = self.application_name {
            parts.push(format!("application_name={}", app));
        }

        parts.push(format!("sslmode={}", self.ssl_mode.as_str()));

        parts.join(" ")
    }

    /// Get the host:port address.
    pub fn address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

// =============================================================================
// SSL Mode
// =============================================================================

/// SSL connection mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[derive(Default)]
pub enum SslMode {
    Disable,
    #[default]
    Prefer,
    Require,
    VerifyCa,
    VerifyFull,
}


impl SslMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Disable => "disable",
            Self::Prefer => "prefer",
            Self::Require => "require",
            Self::VerifyCa => "verify-ca",
            Self::VerifyFull => "verify-full",
        }
    }
}

// =============================================================================
// Pool Configuration
// =============================================================================

/// Connection pool configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolConfig {
    pub min_connections: usize,
    pub max_connections: usize,
    pub acquire_timeout: Duration,
    pub idle_timeout: Duration,
    pub max_lifetime: Duration,
    pub test_on_acquire: bool,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            min_connections: 1,
            max_connections: 10,
            acquire_timeout: Duration::from_secs(30),
            idle_timeout: Duration::from_secs(600),
            max_lifetime: Duration::from_secs(1800),
            test_on_acquire: true,
        }
    }
}

// =============================================================================
// Retry Configuration
// =============================================================================

/// Retry configuration for failed operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    pub max_retries: u32,
    pub initial_delay: Duration,
    pub max_delay: Duration,
    pub multiplier: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(10),
            multiplier: 2.0,
        }
    }
}

impl RetryConfig {
    /// Calculate delay for a given retry attempt.
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        let delay_ms = self.initial_delay.as_millis() as f64
            * self.multiplier.powi(attempt as i32);
        let delay = Duration::from_millis(delay_ms as u64);
        delay.min(self.max_delay)
    }
}

// =============================================================================
// Timeout Configuration
// =============================================================================

/// Timeout configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeoutConfig {
    pub connect: Duration,
    pub query: Duration,
    pub statement: Duration,
}

impl Default for TimeoutConfig {
    fn default() -> Self {
        Self {
            connect: Duration::from_secs(10),
            query: Duration::from_secs(30),
            statement: Duration::from_secs(300),
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_config_default() {
        let config = ClientConfig::default();
        assert_eq!(config.connection.host, "localhost");
        assert_eq!(config.connection.port, 9090);
        assert_eq!(config.pool.max_connections, 10);
    }

    #[test]
    fn test_from_url_simple() {
        let config = ClientConfig::from_url("aegis://localhost:9090/testdb").expect("Should parse simple URL");
        assert_eq!(config.connection.host, "localhost");
        assert_eq!(config.connection.port, 9090);
        assert_eq!(config.connection.database, "testdb");
    }

    #[test]
    fn test_from_url_with_auth() {
        let config = ClientConfig::from_url("aegis://user:pass@localhost:9090/testdb").expect("Should parse URL with auth");
        assert_eq!(config.connection.host, "localhost");
        assert_eq!(config.connection.username, Some("user".to_string()));
        assert_eq!(config.connection.password, Some("pass".to_string()));
    }

    #[test]
    fn test_from_url_default_port() {
        let config = ClientConfig::from_url("aegis://localhost/testdb").expect("Should parse URL with default port");
        assert_eq!(config.connection.port, 9090);
    }

    #[test]
    fn test_connection_string() {
        let config = ConnectionConfig {
            host: "db.example.com".to_string(),
            port: 5433,
            database: "mydb".to_string(),
            username: Some("admin".to_string()),
            password: None,
            ssl_mode: SslMode::Require,
            application_name: Some("myapp".to_string()),
        };

        let conn_str = config.connection_string();
        assert!(conn_str.contains("host=db.example.com"));
        assert!(conn_str.contains("port=5433"));
        assert!(conn_str.contains("dbname=mydb"));
        assert!(conn_str.contains("user=admin"));
        assert!(conn_str.contains("sslmode=require"));
    }

    #[test]
    fn test_retry_delay() {
        let config = RetryConfig::default();
        let delay0 = config.delay_for_attempt(0);
        let delay1 = config.delay_for_attempt(1);
        let delay2 = config.delay_for_attempt(2);

        assert_eq!(delay0, Duration::from_millis(100));
        assert_eq!(delay1, Duration::from_millis(200));
        assert_eq!(delay2, Duration::from_millis(400));
    }

    #[test]
    fn test_retry_max_delay() {
        let config = RetryConfig {
            max_delay: Duration::from_millis(500),
            ..Default::default()
        };

        let delay10 = config.delay_for_attempt(10);
        assert_eq!(delay10, Duration::from_millis(500));
    }
}
