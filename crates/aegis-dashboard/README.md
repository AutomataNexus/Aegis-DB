<p align="center">
  <img src="../../AegisDB-logo.png" alt="AegisDB Logo" width="300">
</p>

# aegis-dashboard

<p align="center">
  <a href="https://opensource.org/licenses/Apache-2.0"><img src="https://img.shields.io/badge/License-Apache%202.0-blue.svg" alt="License"></a>
  <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/Rust-1.75%2B-orange.svg" alt="Rust"></a>
  <img src="https://img.shields.io/badge/crate-0.1.0-green.svg" alt="Version">
  <a href="../../README.md"><img src="https://img.shields.io/badge/part%20of-AegisDB-teal.svg" alt="AegisDB"></a>
</p>

Web dashboard for the Aegis Database Platform.

## Overview

`aegis-dashboard` provides a modern, responsive web interface built with Leptos (Rust WASM). It offers cluster monitoring, database browsing, query execution, and administrative functions.

## Features

- **Cluster Overview** - Real-time node status and health
- **Database Browser** - KV, Document, Graph data exploration
- **Query Builder** - Interactive SQL and query execution
- **Metrics & Monitoring** - Performance charts and statistics
- **User Management** - RBAC and 2FA configuration
- **Settings** - Cluster configuration and security

## Architecture

```
┌─────────────────────────────────────────────────┐
│                 Leptos App                       │
│                 (WASM/CSR)                       │
├─────────────────────────────────────────────────┤
│                   Pages                          │
│  ┌─────────┬──────────┬──────────┬──────────┐  │
│  │Overview │ Database │ Metrics  │ Settings │  │
│  └─────────┴──────────┴──────────┴──────────┘  │
├─────────────────────────────────────────────────┤
│                Components                        │
│  ┌─────────┬──────────┬──────────┬──────────┐  │
│  │ Modals  │  Charts  │  Tables  │  Forms   │  │
│  └─────────┴──────────┴──────────┴──────────┘  │
├─────────────────────────────────────────────────┤
│              API Client Layer                    │
│           (Fetch to aegis-server)               │
└─────────────────────────────────────────────────┘
```

## Pages

| Page | Description |
|------|-------------|
| **Overview** | Cluster status, node health, recent activity |
| **Database** | KV Browser, Collections, Graph, Query Builder |
| **Metrics** | Performance, Storage, Network charts |
| **Nodes** | Detailed node management |
| **Settings** | General, Security, Users, RBAC |

## Tech Stack

- **Leptos 0.6** - Rust reactive web framework
- **WASM** - WebAssembly for browser execution
- **Trunk** - Build tool for Rust WASM apps
- **CSS** - Custom NexusForge theme (cream/teal/terracotta)

## Project Structure

```
aegis-dashboard/
├── src/
│   ├── lib.rs          # App entry point
│   ├── api.rs          # API client functions
│   ├── types.rs        # Type definitions
│   ├── state.rs        # Global state management
│   └── pages/
│       └── dashboard.rs # Main dashboard component
├── assets/
│   └── styles.css      # Global styles
├── index.html          # HTML template
├── Cargo.toml
└── Trunk.toml          # Trunk configuration
```

## Development

### Prerequisites

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Add WASM target
rustup target add wasm32-unknown-unknown

# Install Trunk
cargo install trunk
```

### Running Locally

```bash
cd crates/aegis-dashboard

# Development server with hot reload
trunk serve

# Or with release optimizations
trunk serve --release
```

Dashboard will be available at http://localhost:8000

### Building for Production

```bash
trunk build --release
```

Output is in `dist/` directory.

## Configuration

### Trunk.toml

```toml
[build]
target = "index.html"
dist = "dist"

[watch]
watch = ["src", "assets"]

[serve]
address = "127.0.0.1"
port = 8000
```

### API Endpoint

The dashboard connects to the Aegis server API. Configure the endpoint in `src/api.rs`:

```rust
const API_BASE: &str = "http://localhost:9090/api/v1";
```

## Styling

The dashboard uses a custom theme with CSS variables:

```css
:root {
    --background: #faf9f6;     /* Cream background */
    --card-bg: #f5ebe0;        /* Card background */
    --teal: #14b8a6;           /* Primary accent */
    --terracotta: #c4a484;     /* Secondary accent */
    --text-primary: #111827;   /* Main text */
    --success: #10b981;        /* Green */
    --warning: #f59e0b;        /* Yellow */
    --error: #ef4444;          /* Red */
}
```

## Features in Detail

### Cluster Overview
- Real-time node status indicators
- CPU, memory, disk usage gauges
- Ops/second throughput metrics
- Recent activity feed
- Active alerts display

### Database Browser
- **KV Store**: Browse, add, edit, delete key-value pairs
- **Collections**: Document collection browser with search
- **Graph**: Node and edge visualization
- **Query Builder**: SQL editor with syntax highlighting

### Metrics Dashboard
- Interactive SVG charts
- Time range selection (1h, 6h, 24h, 7d, 30d)
- Performance, Storage, Network tabs
- Export reports to JSON

### Settings
- Cluster configuration (replication, shards)
- Backup & recovery settings
- Security (TLS, authentication)
- 2FA configuration
- User management (CRUD)
- RBAC role management

## API Integration

The dashboard communicates with these API endpoints:

```
GET  /api/v1/admin/cluster     # Cluster status
GET  /api/v1/admin/nodes       # Node list
GET  /api/v1/admin/stats       # Statistics
GET  /api/v1/admin/alerts      # Alerts
GET  /api/v1/kv/keys           # KV entries
GET  /api/v1/documents/collections
POST /api/v1/query-builder/execute
POST /api/v1/auth/login
```

## Browser Support

- Chrome/Edge 90+
- Firefox 90+
- Safari 14+
- (Requires WebAssembly support)

## Tests

The dashboard is tested via end-to-end tests in `aegis-server`.

```bash
cargo test -p aegis-server --test e2e
```

## License

Apache-2.0
