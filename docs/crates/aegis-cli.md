---
layout: default
title: aegis-cli
parent: Crate Documentation
nav_order: 14
description: "Command-line interface for managing Aegis databases"
---

# aegis-cli

Command-line tool for managing and interacting with Aegis databases (binary
name `aegisdb-cli`). Provides database operations, query execution, and
administration commands against a running server.

## Overview

`aegis-cli` is a thin `clap`-based client over the REST API. It authenticates,
runs AegisQL, and drives administrative operations from the terminal.

## Usage

```bash
# Build the CLI
cargo build --release -p aegisdb-cli

# Point it at a server and authenticate
export AEGIS_URL=http://127.0.0.1:9090
export AEGIS_ADMIN_USERNAME=admin
export AEGIS_ADMIN_PASSWORD=your_secure_password

# Run a query
aegisdb-cli query "SELECT * FROM accounts LIMIT 10"

# KV operations
aegisdb-cli kv set mykey '{"hello":"world"}'
aegisdb-cli kv get mykey
```

> As of v0.3.1 the server is fail-closed: provision an admin (via
> `AEGIS_ADMIN_USERNAME` / `AEGIS_ADMIN_PASSWORD`) before privileged CLI
> commands will succeed, otherwise authenticated routes return 503 until a user
> exists. See the [Security Guide](../SECURITY.md).

## Tests

808 tests (workspace total) across the workspace; the CLI exercises the same
client/query paths covered by `aegis-client` and `aegis-query`.
