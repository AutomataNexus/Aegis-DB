<p align="center">
  <img src="https://raw.githubusercontent.com/AutomataNexus/Aegis-DB/main/AegisDB-logo.png" alt="AegisDB Logo" width="300">
</p>

# aegis-cli

<p align="center">
  <a href="https://opensource.org/licenses/Apache-2.0"><img src="https://img.shields.io/badge/License-Apache%202.0-blue.svg" alt="License"></a>
  <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/Rust-1.75%2B-orange.svg" alt="Rust"></a>
  <img src="https://img.shields.io/badge/crate-0.1.7-green.svg" alt="Version">
  <a href="../../README.md"><img src="https://img.shields.io/badge/part%20of-AegisDB-teal.svg" alt="AegisDB"></a>
</p>

Command-line interface for the Aegis Database Platform.

## Overview

`aegis-cli` provides a command-line tool for interacting with Aegis database servers. It supports SQL queries, key-value operations, cluster management, and an interactive shell.

## Features

- **SQL Query Execution** - Execute queries from command line
- **Interactive Shell** - REPL for SQL queries
- **Key-Value Store** - Get, set, delete, and list keys
- **Cluster Management** - View cluster and node status
- **Multiple Output Formats** - Table, JSON, CSV output
- **Database Shortcuts** - Connect using shorthand names (nexusscribe, axonml, dashboard)

## Installation

Build from source:

```bash
cargo build -p aegisdb-cli --release
cp target/release/aegis ~/.local/bin/aegis-client
```

## Usage

### Command Reference

```
aegis-client [OPTIONS] <COMMAND>

Commands:
  query    Execute a SQL query
  status   Check server health and status
  metrics  Get server metrics
  tables   List all tables
  kv       Key-value store operations
  cluster  Cluster information
  shell    Interactive SQL shell
  help     Print help

Options:
  -s, --server <SERVER>      Server URL [default: http://localhost:9090]
  -d, --database <DATABASE>  Database: shorthand (nexusscribe, axonml, dashboard),
                             URL (aegis://host:port/db), or host:port
  -h, --help                 Print help
  -V, --version              Print version
```

### Database Connection

```bash
# Using shorthand names
aegis-client -d nexusscribe query "SELECT 1"
aegis-client -d axonml query "SELECT * FROM users"
aegis-client -d dashboard status

# Using aegis:// URL
aegis-client -d aegis://localhost:9091/mydb query "SELECT 1"

# Using host:port
aegis-client -d localhost:7001 query "SELECT 1"

# Using server flag
aegis-client -s http://localhost:9091 query "SELECT 1"
```

**Shorthand Names:**
| Name | Port | Description |
|------|------|-------------|
| `dashboard` | 9090 | Main dashboard server |
| `local` | 9090 | Alias for dashboard |
| `axonml` | 7001 | AxonML node |
| `nexusscribe` | 9091 | NexusScribe node |

### SQL Queries

```bash
# Simple query
aegis-client -d nexusscribe query "SELECT 1 + 1 AS result"

# Create table
aegis-client -d nexusscribe query "CREATE TABLE users (id INT, name VARCHAR(100), email VARCHAR(200))"

# Insert data
aegis-client -d nexusscribe query "INSERT INTO users VALUES (1, 'Alice', 'alice@example.com')"

# Select with columns
aegis-client -d nexusscribe query "SELECT id, name, email FROM users"

# Output as JSON
aegis-client -d nexusscribe query "SELECT * FROM users" --format json

# Output as CSV
aegis-client -d nexusscribe query "SELECT * FROM users" --format csv
```

### Interactive Shell

```bash
aegis-client -d nexusscribe shell
```

```
Aegis SQL Shell
Connected to: http://localhost:9091
Type 'exit' or 'quit' to exit, 'help' for commands.

aegis> SELECT * FROM users;
+----+-------+-------------------+
| id | name  | email             |
+====+=======+===================+
| 1  | Alice | alice@example.com |
+----+-------+-------------------+

1 row(s) returned in 0 ms

aegis> \d
(lists tables)

aegis> \q
Bye!
```

**Shell Commands:**
| Command | Description |
|---------|-------------|
| `\q`, `exit`, `quit` | Exit the shell |
| `\d` | List tables |
| `\h`, `help` | Show help |

### Key-Value Operations

```bash
# Set a key
aegis-client -d nexusscribe kv set mykey "myvalue"

# Get a key
aegis-client -d nexusscribe kv get mykey

# Delete a key
aegis-client -d nexusscribe kv delete mykey

# List all keys
aegis-client -d nexusscribe kv list

# List keys with prefix
aegis-client -d nexusscribe kv list --prefix "user:"
```

### Status and Metrics

```bash
# Server health
aegis-client -d nexusscribe status

# Server metrics
aegis-client -d nexusscribe metrics

# Cluster info
aegis-client -d nexusscribe cluster

# List tables
aegis-client -d nexusscribe tables
```

## Node Management

The CLI supports dynamic node registration. Nodes can be added manually or synced automatically from a running cluster.

### Sync Nodes from Cluster

```bash
# Auto-discover all nodes from a running cluster
aegis-client nodes sync

# Sync from a specific server
aegis-client nodes sync --from http://localhost:9090
```

This queries the cluster's `/api/v1/admin/nodes` endpoint and automatically creates shorthands based on node names.

### Manual Node Management

```bash
# List all registered nodes
aegis-client nodes list

# Add a node manually
aegis-client nodes add mynode http://localhost:9095

# Remove a node
aegis-client nodes remove mynode
```

### Config File

Node shorthands are stored in `~/.aegis/nodes.json`. Example:

```json
{
  "nodes": {
    "dashboard": {"url": "http://127.0.0.1:9090", "node_id": "node-abc123"},
    "nexusscribe": {"url": "http://127.0.0.1:9091", "node_id": "node-def456"},
    "axonml": {"url": "http://127.0.0.1:7001", "node_id": "node-ghi789"}
  }
}
```

## Examples

```bash
# Auto-discover nodes from cluster
aegis-client nodes sync

# Quick health check across all nodes
aegis-client -d dashboard status
aegis-client -d axonml status
aegis-client -d nexusscribe status

# Create and populate a table
aegis-client -d nexusscribe query "CREATE TABLE products (id INT, name VARCHAR(100), price DECIMAL)"
aegis-client -d nexusscribe query "INSERT INTO products VALUES (1, 'Widget', 9.99), (2, 'Gadget', 19.99)"
aegis-client -d nexusscribe query "SELECT * FROM products WHERE price > 10"

# Export query results as JSON
aegis-client -d nexusscribe query "SELECT * FROM products" -f json > products.json
```

## License

Apache-2.0
