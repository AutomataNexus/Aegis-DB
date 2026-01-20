<p align="center">
  <img src="https://raw.githubusercontent.com/AutomataNexus/Aegis-DB/main/AegisDB-logo.png" alt="AegisDB Logo" width="300">
</p>

# aegis-cli

<p align="center">
  <a href="https://opensource.org/licenses/Apache-2.0"><img src="https://img.shields.io/badge/License-Apache%202.0-blue.svg" alt="License"></a>
  <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/Rust-1.75%2B-orange.svg" alt="Rust"></a>
  <img src="https://img.shields.io/badge/crate-0.1.0-green.svg" alt="Version">
  <a href="../../README.md"><img src="https://img.shields.io/badge/part%20of-AegisDB-teal.svg" alt="AegisDB"></a>
</p>

Command-line interface for the Aegis Database Platform.

## Overview

`aegis-cli` provides a powerful command-line tool for interacting with Aegis databases. It supports interactive queries, data import/export, and administrative operations.

## Features

- **Interactive Shell** - REPL for SQL queries
- **Query Execution** - Run queries from command line or files
- **Data Import/Export** - CSV, JSON, and SQL formats
- **Admin Commands** - Cluster and node management
- **Output Formats** - Table, JSON, CSV output

## Installation

The CLI is installed automatically with Aegis:

```bash
./install.sh
```

Or build manually:

```bash
cargo build -p aegis-cli --release
```

## Usage

### Basic Commands

```bash
# Start interactive shell
aegis shell

# Execute a query
aegis query "SELECT * FROM users"

# Execute from file
aegis query -f queries.sql

# Connect to specific server
aegis --host localhost --port 9090 shell
```

### Command Reference

```
aegis [OPTIONS] <COMMAND>

Commands:
  shell       Start interactive SQL shell
  query       Execute a SQL query
  import      Import data from file
  export      Export data to file
  status      Show cluster status
  help        Print help

Options:
  -h, --host <HOST>    Server host [default: localhost]
  -p, --port <PORT>    Server port [default: 9090]
  -u, --user <USER>    Username
  -P, --password       Prompt for password
  -f, --format <FMT>   Output format: table, json, csv [default: table]
      --help           Print help
```

### Interactive Shell

```
$ aegis shell
Connected to Aegis v2.0.0 at localhost:9090

aegis> SELECT * FROM users LIMIT 5;
┌────┬─────────┬─────────────────────┐
│ id │ name    │ email               │
├────┼─────────┼─────────────────────┤
│ 1  │ Alice   │ alice@example.com   │
│ 2  │ Bob     │ bob@example.com     │
│ 3  │ Charlie │ charlie@example.com │
└────┴─────────┴─────────────────────┘
3 rows (0.012s)

aegis> \help
Available commands:
  \help       Show this help
  \tables     List all tables
  \describe   Describe a table
  \format     Change output format
  \quit       Exit the shell

aegis> \quit
Goodbye!
```

### Data Import

```bash
# Import CSV
aegis import users.csv --table users --format csv

# Import JSON
aegis import users.json --table users --format json

# Import with options
aegis import data.csv \
  --table users \
  --format csv \
  --delimiter ";" \
  --header true \
  --batch-size 1000
```

### Data Export

```bash
# Export to CSV
aegis export "SELECT * FROM users" -o users.csv --format csv

# Export to JSON
aegis export "SELECT * FROM users" -o users.json --format json

# Export entire table
aegis export --table users -o backup.csv
```

### Admin Commands

```bash
# Cluster status
aegis status

# Node information
aegis status --nodes

# Detailed stats
aegis status --verbose
```

## Configuration

Create `~/.aegis/config.toml`:

```toml
[connection]
host = "localhost"
port = 9090
user = "admin"

[shell]
history_file = "~/.aegis/history"
history_size = 1000
prompt = "aegis> "

[output]
format = "table"
max_width = 120
null_string = "NULL"
```

## Environment Variables

| Variable | Description |
|----------|-------------|
| `AEGIS_HOST` | Server host |
| `AEGIS_PORT` | Server port |
| `AEGIS_USER` | Username |
| `AEGIS_PASSWORD` | Password |

## Examples

```bash
# One-liner query with JSON output
aegis query "SELECT * FROM metrics WHERE time > NOW() - INTERVAL '1 hour'" -f json

# Pipe query from stdin
echo "SELECT COUNT(*) FROM users" | aegis query

# Export with compression
aegis export --table logs -o logs.csv.gz --compress

# Batch insert from file
cat inserts.sql | aegis query
```

## License

Apache-2.0
