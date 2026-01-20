//! Aegis Server Binary
//!
//! API server for Aegis database with REST endpoints for the dashboard.
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use aegis_server::{create_router, AppState, ServerConfig};
use clap::Parser;
use std::net::SocketAddr;

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

    let config = ServerConfig::new(&args.host, args.port);
    let addr: SocketAddr = config.socket_addr();

    tracing::info!("Starting Aegis Server on {}", addr);

    let state = AppState::new(config);
    let app = create_router(state);

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind to address");

    tracing::info!("Aegis Server listening on http://{}", addr);
    tracing::info!("Dashboard API ready at http://{}/api/v1", addr);

    axum::serve(listener, app)
        .await
        .expect("Server error");
}
