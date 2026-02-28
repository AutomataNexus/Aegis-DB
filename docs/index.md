---
layout: default
title: Home
nav_order: 1
description: "Aegis-DB - Multi-paradigm database built for regulated industries"
permalink: /
---

# Aegis-DB Documentation
{: .fs-9 }

Multi-paradigm database built for regulated industries
{: .fs-6 .fw-300 }

[Get Started]({% link getting-started.md %}){: .btn .btn-primary .fs-5 .mb-4 .mb-md-0 .mr-2 }
[View on GitHub](https://github.com/AutomataNexus/Aegis-DB){: .btn .fs-5 .mb-4 .mb-md-0 }

---

## Overview

Aegis-DB is a unified, multi-paradigm database platform built in Rust. It combines relational (SQL), time series, document, graph, and real-time streaming capabilities into a single system with enterprise-grade security and compliance features.

**Version 0.2.2** -- 13 crates, 634 tests
{: .label .label-green }

### Key Features

- **Multi-Paradigm** - SQL, Document, TimeSeries, Graph, and Streaming in one platform
- **Multi-Database** - Isolated databases per application with shared infrastructure
- **Enterprise Security** - TLS, MFA, RBAC, audit logging, encryption at rest
- **Compliance Ready** - HIPAA, GDPR, CCPA, SOC 2, FERPA support with PHI tagging and query limits
- **Self-Hosted** - Complete data sovereignty, no cloud dependencies
- **High Performance** - MVCC, vectorized execution, B-tree/Hash indexes, LZ4/Zstd compression
- **OTA Updates** - Rolling cluster updates with automatic rollback
- **Bulk Import** - CSV and JSON bulk data import
- **VACUUM** - Storage reclamation and compaction
- **Graph Engine** - Nodes, edges, adjacency lists, label/relationship indexes

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         Aegis-DB Suite                           │
├─────────────┬─────────────┬─────────────┬───────────────────────┤
│  Dashboard  │   Server    │   Client    │        CLI            │
│   (WASM)    │  (REST API) │    (SDK)    │                       │
│  Port 8000  │  Port 9090  │             │                       │
└─────────────┴──────┬──────┴─────────────┴───────────────────────┘
                     │
┌────────────────────┴────────────────────────────────────────────┐
│                       Query Engine                               │
│            SQL Parser → Planner → Vectorized Executor            │
├─────────────┬─────────────┬─────────────┬───────────────────────┤
│  Document   │  TimeSeries │   Graph     │      Streaming        │
│   Store     │    Engine   │   Engine    │       Engine          │
└─────────────┴─────────────┴─────────────┴───────────────────────┘
                     │
┌────────────────────┴────────────────────────────────────────────┐
│                     Storage Layer                                │
│           WAL → MVCC → Block Storage → Compression               │
├─────────────┬─────────────┬─────────────────────────────────────┤
│   Memory    │  LocalFS    │         Replication                  │
│  Backend    │  Backend    │    (Raft, 2PC, CRDTs)                │
└─────────────┴─────────────┴─────────────────────────────────────┘
```

## Quick Links

| Section | Description |
|:--------|:------------|
| [Getting Started]({% link getting-started.md %}) | Installation and initial setup |
| [User Guide]({% link USER_GUIDE.md %}) | Configuration and usage |
| [AegisQL Reference]({% link AegisQL.md %}) | Query language documentation |
| [Security]({% link SECURITY.md %}) | Security features and encryption |
| [Compliance]({% link COMPLIANCE.md %}) | HIPAA, GDPR, SOC 2 compliance |
| [Crate Documentation]({% link crates/index.md %}) | Rust crate architecture |

## System Requirements

| Component | Minimum | Recommended |
|:----------|:--------|:------------|
| CPU | 2 cores | 4+ cores |
| RAM | 4 GB | 8+ GB |
| Storage | 20 GB SSD | 100+ GB NVMe |
| OS | Ubuntu 22.04 / Debian 12 | Ubuntu 24.04 |

## License

Aegis-DB is licensed under the [Business Source License 1.1](https://github.com/AutomataNexus/Aegis-DB/blob/main/LICENSING.md).

- **Use Limitation**: Free for everything except reselling as a managed Database-as-a-Service
- **Change Date**: January 26, 2030
- **Change License**: Apache License 2.0

For commercial DBaaS licensing, contact [devops@automatanexus.com](mailto:devops@automatanexus.com).
