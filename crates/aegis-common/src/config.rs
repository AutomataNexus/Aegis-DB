//! Aegis Config - Configuration Structures
//!
//! Configuration types for all Aegis components. Supports loading from
//! environment variables, TOML files, and programmatic construction.
//! Provides sensible defaults for development and production deployments.
//!
//! Key Features:
//! - Storage configuration (paths, compression, encryption)
//! - Network configuration (bind addresses, TLS settings)
//! - Cluster configuration (node identity, peer discovery)
//! - Performance tuning (buffer sizes, thread counts)
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use crate::types::{CompressionType, EncryptionType, NodeId};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

// =============================================================================
// Storage Configuration
// =============================================================================

/// Configuration for the storage layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    pub data_directory: PathBuf,
    pub wal_directory: PathBuf,
    pub compression: CompressionType,
    pub encryption: EncryptionType,
    pub sync_writes: bool,
    pub max_file_size: u64,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            data_directory: PathBuf::from("./data"),
            wal_directory: PathBuf::from("./data/wal"),
            compression: CompressionType::Lz4,
            encryption: EncryptionType::None,
            sync_writes: true,
            max_file_size: 256 * 1024 * 1024, // 256 MB
        }
    }
}

// =============================================================================
// Memory Configuration
// =============================================================================

/// Configuration for memory management.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    pub buffer_pool_size: usize,
    pub query_memory_limit: usize,
    pub arena_block_size: usize,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            buffer_pool_size: 1024 * 1024 * 1024, // 1 GB
            query_memory_limit: 256 * 1024 * 1024, // 256 MB
            arena_block_size: 64 * 1024,           // 64 KB
        }
    }
}

// =============================================================================
// Network Configuration
// =============================================================================

/// Configuration for network services.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    pub bind_address: SocketAddr,
    pub advertise_address: Option<SocketAddr>,
    pub max_connections: usize,
    pub connection_timeout: Duration,
    pub request_timeout: Duration,
    pub tls_enabled: bool,
    pub tls_cert_path: Option<PathBuf>,
    pub tls_key_path: Option<PathBuf>,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            bind_address: "0.0.0.0:5432".parse().unwrap(),
            advertise_address: None,
            max_connections: 1000,
            connection_timeout: Duration::from_secs(30),
            request_timeout: Duration::from_secs(300),
            tls_enabled: false,
            tls_cert_path: None,
            tls_key_path: None,
        }
    }
}

// =============================================================================
// Cluster Configuration
// =============================================================================

/// Configuration for cluster membership.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterConfig {
    pub node_id: NodeId,
    pub peers: Vec<PeerConfig>,
    pub replication_factor: u8,
    pub election_timeout: Duration,
    pub heartbeat_interval: Duration,
}

impl Default for ClusterConfig {
    fn default() -> Self {
        Self {
            node_id: NodeId("node-1".to_string()),
            peers: Vec::new(),
            replication_factor: 3,
            election_timeout: Duration::from_millis(1000),
            heartbeat_interval: Duration::from_millis(100),
        }
    }
}

/// Configuration for a peer node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerConfig {
    pub node_id: NodeId,
    pub address: SocketAddr,
}

// =============================================================================
// Query Configuration
// =============================================================================

/// Configuration for the query engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryConfig {
    pub max_query_length: usize,
    pub default_limit: usize,
    pub statement_timeout: Duration,
    pub enable_query_cache: bool,
    pub query_cache_size: usize,
    pub parallel_workers: usize,
}

impl Default for QueryConfig {
    fn default() -> Self {
        Self {
            max_query_length: 1024 * 1024, // 1 MB
            default_limit: 10000,
            statement_timeout: Duration::from_secs(300),
            enable_query_cache: true,
            query_cache_size: 1000,
            parallel_workers: num_cpus::get(),
        }
    }
}

// =============================================================================
// Server Configuration
// =============================================================================

/// Top-level server configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ServerConfig {
    pub storage: StorageConfig,
    pub memory: MemoryConfig,
    pub network: NetworkConfig,
    pub cluster: ClusterConfig,
    pub query: QueryConfig,
}

impl ServerConfig {
    /// Load configuration from a TOML file.
    pub fn from_file(path: &PathBuf) -> crate::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        toml::from_str(&content).map_err(|e| crate::AegisError::Configuration(e.to_string()))
    }

    /// Create configuration with development defaults.
    pub fn development() -> Self {
        Self::default()
    }

    /// Create configuration optimized for production.
    pub fn production() -> Self {
        Self {
            storage: StorageConfig {
                sync_writes: true,
                compression: CompressionType::Zstd,
                ..Default::default()
            },
            memory: MemoryConfig {
                buffer_pool_size: 4 * 1024 * 1024 * 1024, // 4 GB
                ..Default::default()
            },
            network: NetworkConfig {
                tls_enabled: true,
                ..Default::default()
            },
            ..Default::default()
        }
    }
}
