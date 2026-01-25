//! Aegis Server Binary
//!
//! API server for Aegis database with REST endpoints for the dashboard.
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use aegis_server::{create_router, AppState, ServerConfig};
use clap::Parser;
use std::net::SocketAddr;
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

    // Parse peer addresses
    let peers: Vec<String> = args.peers
        .map(|p| p.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect())
        .unwrap_or_default();

    let config = ServerConfig::new(&args.host, args.port)
        .with_data_dir(args.data_dir.clone())
        .with_node_id(args.node_id.clone())
        .with_node_name(args.node_name.clone())
        .with_cluster_name(args.cluster.clone())
        .with_peers(peers.clone());
    let addr: SocketAddr = config.socket_addr();

    if let Some(ref data_dir) = args.data_dir {
        tracing::info!("Persistence enabled, data directory: {}", data_dir);
    } else {
        tracing::warn!("No data directory specified, running in-memory only (data will be lost on restart)");
    }

    if !peers.is_empty() {
        tracing::info!("Cluster mode enabled, peers: {:?}", peers);
    }

    tracing::info!("Starting Aegis Server on {}", addr);
    tracing::info!("Node ID: {}", config.node_id);
    if let Some(ref name) = config.node_name {
        tracing::info!("Node Name: {}", name);
    }

    let state = AppState::new(config);
    let state_for_shutdown = state.clone();
    let app = create_router(state);

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind to address");

    tracing::info!("Aegis Server listening on http://{}", addr);
    tracing::info!("Dashboard API ready at http://{}/api/v1", addr);

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

    // Run server with graceful shutdown
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(state_for_shutdown))
        .await
        .expect("Server error");
}

/// Join a peer node in the cluster.
async fn join_peer(state: &AppState, peer_addr: &str) {
    let self_info = state.admin.get_self_info();
    let url = format!("http://{}/api/v1/cluster/join", peer_addr);

    let client = reqwest::Client::new();
    let body = serde_json::json!({
        "node_id": self_info.id,
        "node_name": self_info.name,
        "address": self_info.address,
    });

    match client.post(&url).json(&body).timeout(std::time::Duration::from_secs(5)).send().await {
        Ok(response) => {
            if response.status().is_success() {
                if let Ok(data) = response.json::<serde_json::Value>().await {
                    tracing::info!("Successfully joined peer at {}", peer_addr);
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
    let client = reqwest::Client::new();

    for peer in peers {
        let url = format!("http://{}/api/v1/cluster/heartbeat", peer.address);
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
