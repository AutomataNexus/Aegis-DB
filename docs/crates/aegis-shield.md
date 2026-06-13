---
layout: default
title: aegis-shield
parent: Crate Documentation
nav_order: 12
description: "Integrated security shield — injection detection, IP reputation, anomaly detection, auto-blocking"
---

# aegis-shield

Integrated security shield for Aegis-DB. Provides SQL injection detection, IP
reputation tracking, request fingerprinting, anomaly detection, auto-blocking,
and a live threat feed.

## Overview

`aegis-shield` is the runtime security layer the server consults on the login
and query paths. It turns raw request signals (failed logins, suspicious SQL,
client fingerprints) into reputation scores and, when thresholds are crossed,
automatic IP blocks.

## Modules

| Module | Responsibility |
|--------|----------------|
| `sql_injection.rs` | SQL injection pattern detection |
| `ip_reputation.rs` | Per-IP reputation scoring |
| `fingerprint.rs` | Request fingerprinting |
| `anomaly.rs` | Behavioral anomaly detection |
| `blocker.rs` | Auto-blocking / ban enforcement |
| `feed.rs` | Live threat feed |
| `threat.rs` | Threat classification and events |
| `policy.rs` | Shield policy configuration |
| `config.rs` | Shield configuration |
| `error.rs` | Error types |

## Integration (v0.3.1)

The server wires Shield into two hot paths:

- **Brute-force detection.** Failed logins call `record_failed_auth(ip, user)`,
  feeding the reputation/auto-ban detector — reputation-based bans the rate
  limiter alone cannot provide.
- **SQL injection screening.** User SQL on the query path is run through the
  injection detector (detect-and-log; enforcement is deployment-gated so rolling
  deploys aren't broken by false positives).

Shield mutations (block/unblock, policy changes) are admin-only — `require_admin`
is enforced on those routes.

## Tests

808 tests (workspace total) covering injection detection, reputation scoring,
fingerprinting, anomaly detection, and auto-blocking.
