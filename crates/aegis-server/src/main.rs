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

    let config = ServerConfig::new(&args.host, args.port)
        .with_data_dir(args.data_dir.clone());
    let addr: SocketAddr = config.socket_addr();

    if let Some(ref data_dir) = args.data_dir {
        tracing::info!("Persistence enabled, data directory: {}", data_dir);
    } else {
        tracing::warn!("No data directory specified, running in-memory only (data will be lost on restart)");
    }

    tracing::info!("Starting Aegis Server on {}", addr);

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

    // Run server with graceful shutdown
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(state_for_shutdown))
        .await
        .expect("Server error");
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
