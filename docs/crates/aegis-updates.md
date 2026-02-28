---
layout: default
title: aegis-updates
parent: Crate Documentation
nav_order: 9
description: "OTA rolling update system"
---

# aegis-updates

Over-the-Air (OTA) Rolling Update System for Aegis Database Platform.

## Overview

Provides zero-downtime rolling updates for Aegis clusters. Updates are applied
to follower nodes first, then the leader last, with automatic rollback if any
node fails health checks or quorum is lost.

## Modules

### version.rs
Version tracking:
- `VERSION` - Compile-time version from `CARGO_PKG_VERSION`
- `NodeVersion` - Node version info with node ID, name, version, binary hash
- `ClusterVersionInfo` - Aggregated version info across all cluster nodes

### binary.rs
Binary management:
- `download_binary()` - Download update binary from URL
- `verify_sha256()` - Verify binary integrity with SHA-256 hash
- `stage_binary()` - Stage binary in staging directory
- `backup_current_binary()` - Backup current binary before update
- `apply_binary()` - Atomic binary replacement (rename)

### orchestrator.rs
Rolling update orchestration:
- `UpdateOrchestrator` - Main coordinator for cluster-wide updates
- `UpdatePlan` - Update plan with version, URL, SHA-256, and node list
- `UpdateStatus` - Pending, InProgress, Completed, Failed, RolledBack
- `ClusterNode` - Node info for update targeting
- Rolling strategy: followers first, leader last
- Automatic rollback on node failure or quorum loss

### rollback.rs
Rollback operations:
- `restore_backup()` - Restore backed-up binary
- `rollback_node()` - Rollback a single node (restore + restart)
- `rollback_nodes()` - Rollback multiple nodes in sequence

### health.rs
Post-update health verification:
- `HealthCheck` - Health check configuration (timeout, retries, interval)
- `check_node_health()` - Single health check against node endpoint
- `wait_for_healthy()` - Retry health checks until success or timeout

## Update Flow

```
1. Create UpdatePlan (target version, binary URL, SHA-256)
2. Stage binary on each node
3. For each FOLLOWER (then leader last):
   a. Drain node (stop accepting queries)
   b. Flush data to disk
   c. Apply staged binary (atomic rename)
   d. Process restarts (PM2 auto-restart)
   e. Wait for health check with expected version
   f. Verify cluster rejoin
4. If any node fails → rollback that node
5. If quorum lost → rollback entire cluster
```

## Usage Example

```rust
use aegis_updates::orchestrator::{UpdateOrchestrator, ClusterNode};

// Create orchestrator
let orchestrator = UpdateOrchestrator::new(
    "/usr/local/bin/aegis-server",
    "/var/lib/aegis/data",
);

// Create update plan
let plan = orchestrator.create_plan(
    "0.2.2",
    "https://releases.example.com/aegis-server-0.2.2",
    "sha256hash...",
    vec![
        ClusterNode {
            node_id: "node-1".into(),
            name: "Dashboard".into(),
            address: "http://127.0.0.1:9090".into(),
            is_leader: true,
        },
        ClusterNode {
            node_id: "node-2".into(),
            name: "NexusScribe".into(),
            address: "http://127.0.0.1:9091".into(),
            is_leader: false,
        },
    ],
)?;

// Execute rolling update
orchestrator.execute_plan(&plan.id).await?;

// Check status
let status = orchestrator.get_plan(&plan.id)?;
```

## API Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/v1/updates/version` | Version info for all cluster nodes |
| POST | `/api/v1/updates/plan` | Create an update plan |
| POST | `/api/v1/updates/execute` | Execute a pending update plan |
| GET | `/api/v1/updates/status/:plan_id` | Get update plan status |
| GET | `/api/v1/updates/history` | List all update plans |

All endpoints require authentication.

## Tests

634 tests (workspace total) covering version tracking, binary operations, health checks, orchestration, and rollback.
