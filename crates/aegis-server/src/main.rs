//! Aegis Server Binary
//!
//! API server for Aegis database with REST endpoints for the dashboard.
//! Supports both HTTP and HTTPS (TLS) connections.
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use aegis_server::{create_router, AppState, ServerConfig, ClusterTlsConfig};
use aegis_server::secrets::{self, SecretsProvider};
use axum_server::tls_rustls::RustlsConfig;
use clap::Parser;
use std::net::SocketAddr;
use std::path::PathBuf;
use tokio::signal;

#[derive(Parser)]
#[command(name = "aegis-server")]
#[command(about = "Aegis Database API Server")]
struct Args {
    /// Host to bind to
    #[arg(short = 'H', long, default_value = "127.0.0.1")]
    host: String,

    /// Port to listen on
    #[arg(short, long, default_value = "9090")]
    port: u16,

    /// Data directory for persistence (enables disk storage)
    #[arg(short, long)]
    data_dir: Option<String>,

    /// Unique node ID (auto-generated if not provided)
    #[arg(long)]
    node_id: Option<String>,

    /// Node name for display (e.g., "AxonML", "NexusScribe")
    #[arg(long)]
    node_name: Option<String>,

    /// Comma-separated list of peer addresses to join (e.g., "127.0.0.1:9090,127.0.0.1:9091")
    #[arg(long)]
    peers: Option<String>,

    /// Cluster name
    #[arg(long, default_value = "aegis-cluster")]
    cluster: String,

    /// Enable TLS/HTTPS
    #[arg(long)]
    tls: bool,

    /// TLS certificate file path (PEM format)
    #[arg(long)]
    tls_cert: Option<String>,

    /// TLS private key file path (PEM format)
    #[arg(long)]
    tls_key: Option<String>,

    /// Enable TLS for cluster communication (uses same certs as server TLS by default)
    #[arg(long)]
    cluster_tls: bool,

    /// CA certificate for verifying cluster peer certificates (PEM format)
    #[arg(long)]
    cluster_ca_cert: Option<String>,

    /// Client certificate for cluster mTLS (PEM format, optional)
    #[arg(long)]
    cluster_client_cert: Option<String>,

    /// Client private key for cluster mTLS (PEM format, optional)
    #[arg(long)]
    cluster_client_key: Option<String>,

    /// Skip certificate verification for cluster TLS (INSECURE - only for testing)
    #[arg(long)]
    cluster_tls_insecure: bool,
}

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    let args = Args::parse();

    // Production mode TLS requirement check
    // In production, TLS must be enabled unless explicitly overridden
    let is_production = std::env::var("AEGIS_PRODUCTION")
        .map(|v| v.eq_ignore_ascii_case("true") || v == "1")
        .unwrap_or(false);
    let allow_insecure = std::env::var("AEGIS_ALLOW_INSECURE")
        .map(|v| v.eq_ignore_ascii_case("true") || v == "1")
        .unwrap_or(false);

    if is_production && !args.tls && !allow_insecure {
        eprintln!();
        eprintln!("╔══════════════════════════════════════════════════════════════════════╗");
        eprintln!("║                    PRODUCTION SECURITY ERROR                         ║");
        eprintln!("╠══════════════════════════════════════════════════════════════════════╣");
        eprintln!("║  TLS is required in production mode but is not enabled.              ║");
        eprintln!("║                                                                      ║");
        eprintln!("║  To fix this, enable TLS with:                                       ║");
        eprintln!("║    --tls --tls-cert /path/to/cert.pem --tls-key /path/to/key.pem     ║");
        eprintln!("║                                                                      ║");
        eprintln!("║  Or set environment variables:                                       ║");
        eprintln!("║    AEGIS_TLS_CERT=/path/to/cert.pem                                  ║");
        eprintln!("║    AEGIS_TLS_KEY=/path/to/key.pem                                    ║");
        eprintln!("║                                                                      ║");
        eprintln!("║  To bypass this check (NOT RECOMMENDED), set:                        ║");
        eprintln!("║    AEGIS_ALLOW_INSECURE=true                                         ║");
        eprintln!("╚══════════════════════════════════════════════════════════════════════╝");
        eprintln!();
        std::process::exit(1);
    }

    if is_production && allow_insecure && !args.tls {
        tracing::warn!("╔══════════════════════════════════════════════════════════════════════╗");
        tracing::warn!("║  WARNING: Running production mode WITHOUT TLS (AEGIS_ALLOW_INSECURE) ║");
        tracing::warn!("║  This is a security risk! Enable TLS for production deployments.     ║");
        tracing::warn!("╚══════════════════════════════════════════════════════════════════════╝");
    }

    // Initialize secrets manager (supports Vault or env vars)
    let secrets_manager = secrets::init_secrets_manager().await;

    // Parse peer addresses
    let peers: Vec<String> = args.peers
        .map(|p| p.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect())
        .unwrap_or_default();

    // Build cluster TLS configuration if enabled
    let cluster_tls_config = if args.cluster_tls || args.tls {
        // If cluster_tls is explicitly enabled, or if server TLS is enabled (inherit by default)
        let enabled = args.cluster_tls || args.tls;
        Some(ClusterTlsConfig {
            enabled,
            ca_cert_path: args.cluster_ca_cert.clone(),
            client_cert_path: args.cluster_client_cert.clone().or_else(|| args.tls_cert.clone()),
            client_key_path: args.cluster_client_key.clone().or_else(|| args.tls_key.clone()),
            danger_accept_invalid_certs: args.cluster_tls_insecure,
        })
    } else {
        None
    };

    // Build server configuration
    let config = ServerConfig::new(&args.host, args.port)
        .with_data_dir(args.data_dir.clone())
        .with_node_id(args.node_id.clone())
        .with_node_name(args.node_name.clone())
        .with_cluster_name(args.cluster.clone())
        .with_peers(peers.clone())
        .with_cluster_tls(cluster_tls_config);

    let addr: SocketAddr = config.socket_addr();

    if let Some(ref data_dir) = args.data_dir {
        tracing::info!("Persistence enabled, data directory: {}", data_dir);
    } else {
        tracing::warn!("No data directory specified, running in-memory only (data will be lost on restart)");
    }

    if !peers.is_empty() {
        tracing::info!("Cluster mode enabled, peers: {:?}", peers);
        if config.cluster_tls_enabled() {
            tracing::info!("Cluster TLS enabled for inter-node communication");
            if let Some(ref cluster_tls) = config.cluster_tls {
                if cluster_tls.danger_accept_invalid_certs {
                    tracing::warn!("Cluster TLS certificate verification DISABLED (insecure)");
                }
                if cluster_tls.ca_cert_path.is_some() {
                    tracing::info!("  Using custom CA certificate for peer verification");
                }
                if cluster_tls.client_cert_path.is_some() {
                    tracing::info!("  mTLS enabled with client certificate");
                }
            }
        } else {
            tracing::warn!("Cluster TLS disabled - inter-node communication is unencrypted");
        }
    }

    tracing::info!("Starting Aegis Server on {}", addr);
    tracing::info!("Node ID: {}", config.node_id);
    if let Some(ref name) = config.node_name {
        tracing::info!("Node Name: {}", name);
    }

    let state = AppState::new(config);
    let state_for_shutdown = state.clone();
    let app = create_router(state.clone());

    // Start periodic save task if persistence is enabled
    let state_for_save = state_for_shutdown.clone();
    if args.data_dir.is_some() {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));
            loop {
                interval.tick().await;
                if let Err(e) = state_for_save.save_to_disk() {
                    tracing::error!("Failed to save data: {}", e);
                }
            }
        });
    }

    // Start peer discovery and heartbeat task
    let state_for_cluster = state_for_shutdown.clone();
    let peers_for_task = peers.clone();
    if !peers_for_task.is_empty() {
        tokio::spawn(async move {
            // Initial delay to let server start
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

            // Join peers on startup
            for peer_addr in &peers_for_task {
                join_peer(&state_for_cluster, peer_addr).await;
            }

            // Periodic heartbeat loop
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(5));
            loop {
                interval.tick().await;
                send_heartbeats(&state_for_cluster).await;
            }
        });
    }

    // Determine TLS configuration
    let tls_config = if args.tls {
        // Try command line args first, then environment/Vault
        let cert_path = args.tls_cert
            .or_else(|| secrets_manager.get(secrets::keys::TLS_CERT_PATH))
            .expect("TLS enabled but no certificate path provided. Use --tls-cert or set AEGIS_TLS_CERT");

        let key_path = args.tls_key
            .or_else(|| secrets_manager.get(secrets::keys::TLS_KEY_PATH))
            .expect("TLS enabled but no key path provided. Use --tls-key or set AEGIS_TLS_KEY");

        // Verify files exist
        if !PathBuf::from(&cert_path).exists() {
            panic!("TLS certificate file not found: {}", cert_path);
        }
        if !PathBuf::from(&key_path).exists() {
            panic!("TLS key file not found: {}", key_path);
        }

        Some((cert_path, key_path))
    } else {
        None
    };

    // Run server with or without TLS
    if let Some((cert_path, key_path)) = tls_config {
        // TLS/HTTPS mode
        tracing::info!("TLS enabled, loading certificates...");
        tracing::info!("  Certificate: {}", cert_path);
        tracing::info!("  Private key: {}", key_path);

        let rustls_config = RustlsConfig::from_pem_file(&cert_path, &key_path)
            .await
            .expect("Failed to load TLS configuration");

        tracing::info!("Aegis Server listening on https://{}", addr);
        tracing::info!("Dashboard API ready at https://{}/api/v1", addr);

        let handle = axum_server::Handle::new();
        let handle_for_shutdown = handle.clone();

        // Spawn shutdown handler
        tokio::spawn(async move {
            shutdown_signal(state_for_shutdown).await;
            handle_for_shutdown.graceful_shutdown(Some(std::time::Duration::from_secs(30)));
        });

        axum_server::bind_rustls(addr, rustls_config)
            .handle(handle)
            .serve(app.into_make_service())
            .await
            .expect("Server error");
    } else {
        // HTTP mode
        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .expect("Failed to bind to address");

        tracing::info!("Aegis Server listening on http://{}", addr);
        tracing::info!("Dashboard API ready at http://{}/api/v1", addr);

        axum::serve(listener, app)
            .with_graceful_shutdown(shutdown_signal(state_for_shutdown))
            .await
            .expect("Server error");
    }
}

/// Build a reqwest client configured for cluster communication.
/// Supports TLS with optional CA certificate and client certificates for mTLS.
fn build_cluster_client(config: &ClusterTlsConfig) -> Result<reqwest::Client, reqwest::Error> {
    let mut builder = reqwest::Client::builder();

    // Handle certificate verification
    if config.danger_accept_invalid_certs {
        builder = builder.danger_accept_invalid_certs(true);
    }

    // Add custom CA certificate if provided
    if let Some(ref ca_path) = config.ca_cert_path {
        if let Ok(ca_cert_pem) = std::fs::read(ca_path) {
            if let Ok(ca_cert) = reqwest::Certificate::from_pem(&ca_cert_pem) {
                builder = builder.add_root_certificate(ca_cert);
                tracing::debug!("Added custom CA certificate from {}", ca_path);
            } else {
                tracing::warn!("Failed to parse CA certificate from {}", ca_path);
            }
        } else {
            tracing::warn!("Failed to read CA certificate file: {}", ca_path);
        }
    }

    // Add client certificate for mTLS if provided
    if let (Some(ref cert_path), Some(ref key_path)) = (&config.client_cert_path, &config.client_key_path) {
        if let (Ok(cert_pem), Ok(key_pem)) = (std::fs::read(cert_path), std::fs::read(key_path)) {
            // Combine cert and key into a single PEM for reqwest Identity
            let mut combined_pem = cert_pem;
            combined_pem.extend_from_slice(b"\n");
            combined_pem.extend_from_slice(&key_pem);

            if let Ok(identity) = reqwest::Identity::from_pem(&combined_pem) {
                builder = builder.identity(identity);
                tracing::debug!("Added client certificate for mTLS");
            } else {
                tracing::warn!("Failed to parse client certificate/key for mTLS");
            }
        } else {
            tracing::warn!("Failed to read client certificate or key files");
        }
    }

    builder.build()
}

/// Get the URL scheme (http or https) based on cluster TLS configuration.
fn get_cluster_scheme(config: &aegis_server::ServerConfig) -> &'static str {
    if config.cluster_tls_enabled() {
        "https"
    } else {
        "http"
    }
}

/// Join a peer node in the cluster.
async fn join_peer(state: &AppState, peer_addr: &str) {
    let self_info = state.admin.get_self_info();
    let scheme = get_cluster_scheme(&state.config);
    let url = format!("{}://{}/api/v1/cluster/join", scheme, peer_addr);

    // Build the appropriate client based on TLS configuration
    let client = if let Some(ref cluster_tls) = state.config.cluster_tls {
        if cluster_tls.enabled {
            match build_cluster_client(cluster_tls) {
                Ok(c) => c,
                Err(e) => {
                    tracing::error!("Failed to build TLS client for cluster communication: {}", e);
                    return;
                }
            }
        } else {
            reqwest::Client::new()
        }
    } else {
        reqwest::Client::new()
    };

    let body = serde_json::json!({
        "node_id": self_info.id,
        "node_name": self_info.name,
        "address": self_info.address,
    });

    match client.post(&url).json(&body).timeout(std::time::Duration::from_secs(5)).send().await {
        Ok(response) => {
            if response.status().is_success() {
                if let Ok(data) = response.json::<serde_json::Value>().await {
                    tracing::info!("Successfully joined peer at {} ({})", peer_addr, scheme);
                    // Register any peers returned by the join response
                    if let Some(peers) = data.get("peers").and_then(|p| p.as_array()) {
                        for peer in peers {
                            if let (Some(id), Some(addr)) = (peer.get("id").and_then(|v| v.as_str()), peer.get("address").and_then(|v| v.as_str())) {
                                if addr != self_info.address {
                                    let name = peer.get("name").and_then(|v| v.as_str()).map(|s| s.to_string());
                                    state.admin.register_peer(aegis_server::admin::PeerNode {
                                        id: id.to_string(),
                                        name,
                                        address: addr.to_string(),
                                        status: aegis_server::admin::NodeStatus::Online,
                                        role: aegis_server::admin::NodeRole::Follower,
                                        last_seen: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64,
                                        version: env!("CARGO_PKG_VERSION").to_string(),
                                        uptime_seconds: 0,
                                        metrics: None,
                                    });
                                    state.admin.add_peer_address(addr.to_string());
                                }
                            }
                        }
                    }
                }
            } else {
                tracing::warn!("Failed to join peer at {}: HTTP {}", peer_addr, response.status());
            }
        }
        Err(e) => {
            tracing::warn!("Failed to connect to peer at {}: {}", peer_addr, e);
        }
    }
}

/// Send heartbeats to all known peers.
async fn send_heartbeats(state: &AppState) {
    let self_info = state.admin.get_self_info();
    let peers = state.admin.get_peers();
    let scheme = get_cluster_scheme(&state.config);

    // Build the appropriate client based on TLS configuration
    let client = if let Some(ref cluster_tls) = state.config.cluster_tls {
        if cluster_tls.enabled {
            match build_cluster_client(cluster_tls) {
                Ok(c) => c,
                Err(e) => {
                    tracing::error!("Failed to build TLS client for heartbeats: {}", e);
                    return;
                }
            }
        } else {
            reqwest::Client::new()
        }
    } else {
        reqwest::Client::new()
    };

    for peer in peers {
        let url = format!("{}://{}/api/v1/cluster/heartbeat", scheme, peer.address);
        let body = serde_json::json!({
            "node_id": self_info.id,
            "node_name": self_info.name,
            "address": self_info.address,
            "uptime_seconds": self_info.uptime_seconds,
            "metrics": self_info.metrics,
        });

        match client.post(&url).json(&body).timeout(std::time::Duration::from_secs(3)).send().await {
            Ok(response) => {
                if !response.status().is_success() {
                    tracing::debug!("Heartbeat to {} failed: HTTP {}", peer.address, response.status());
                    state.admin.mark_peer_offline(&peer.id);
                }
            }
            Err(_) => {
                tracing::debug!("Heartbeat to {} failed: connection error", peer.address);
                state.admin.mark_peer_offline(&peer.id);
            }
        }
    }

    // Also try to discover new peers from configured addresses
    for addr in state.admin.peer_addresses() {
        let existing = state.admin.get_peers();
        if !existing.iter().any(|p| p.address == addr) {
            join_peer(state, &addr).await;
        }
    }
}

async fn shutdown_signal(state: AppState) {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!("Shutdown signal received, saving data...");
    if let Err(e) = state.save_to_disk() {
        tracing::error!("Failed to save data on shutdown: {}", e);
    } else {
        tracing::info!("Data saved successfully");
    }
}
