<p align="center">
  <img src="https://raw.githubusercontent.com/AutomataNexus/assets/main/AegisDB-logo.png" alt="AegisDB Logo" width="300">
</p>

# aegis-shield

<p align="center">
  <a href="https://opensource.org/licenses/Apache-2.0"><img src="https://img.shields.io/badge/License-Apache%202.0-blue.svg" alt="License"></a>
  <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/Rust-1.75%2B-orange.svg" alt="Rust"></a>
  <img src="https://img.shields.io/badge/crate-0.2.5-green.svg" alt="Version">
  <img src="https://img.shields.io/badge/tests-69%20passing-brightgreen.svg" alt="Tests">
  <img src="https://img.shields.io/badge/patterns-38%20SQL%20injection-red.svg" alt="Patterns">
  <a href="../../README.md"><img src="https://img.shields.io/badge/part%20of-AegisDB-teal.svg" alt="AegisDB"></a>
</p>

Real-time database security shield for the Aegis Database Platform.

## Overview

`aegis-shield` is a pure-Rust security middleware that auto-protects the database from SQL injection, brute force attacks, vulnerability scanning, and anomalous query patterns. It runs as a middleware layer on every incoming request, scoring threats in real-time and automatically blocking malicious traffic.

Zero configuration required — it starts in Moderate preset and learns baseline behavior automatically. Security teams can tune it via Strict/Permissive presets or custom rules through the API.

## Architecture

```
Incoming Request
       │
       ▼
┌──────────────────────────────────────────────────────┐
│                   ShieldEngine                        │
├──────────────────────────────────────────────────────┤
│                                                       │
│  ① Allowlist ──▶ ② Blocker ──▶ ③ IP Reputation       │
│                                       │               │
│  ④ Fingerprint    ⑤ SQL Injection    ⑥ Anomaly       │
│     Analysis         Detection        Detection       │
│         │                │                │           │
│         └────────┬───────┘────────────────┘           │
│                  ▼                                     │
│           Score Engine (0-100)                         │
│                  │                                     │
│           Policy Engine ──▶ Threat Feed               │
│           (Presets + Rules)   (Events + Stats)         │
│                  │                                     │
│              Verdict                                   │
│         Allow / Block / RateLimit                      │
└──────────────────────────────────────────────────────┘
       │
       ▼
  Handler (if allowed)
```

## Threat Detection

### SQL Injection (38 patterns)

| Category | Patterns | Score Range |
|----------|----------|-------------|
| Union-based injection | UNION SELECT, UNION ALL SELECT | 90 |
| Boolean-based blind | OR 1=1, AND 1=1, tautologies | 70-85 |
| Time-based blind | SLEEP(), BENCHMARK(), WAITFOR, pg_sleep | 80 |
| Stacked queries | ;DROP, ;DELETE, ;INSERT, ;UPDATE | 90-95 |
| Comment injection | --, /\*\*/, # comment markers | 60-65 |
| String termination | '; DROP, "; DELETE | 90 |
| File operations | LOAD_FILE, INTO OUTFILE, INTO DUMPFILE | 90 |
| Obfuscation | CHAR(), CONCAT(), hex encoding, double encoding | 65-75 |
| System commands | xp_cmdshell, EXEC(), EXECUTE | 85-95 |
| Information disclosure | INFORMATION_SCHEMA, sys.tables | 65-70 |

### IP Reputation System

- Score range: **-100** (worst) to **+100** (best), starts at 0
- Each successful request: +1 (max +100)
- Failed authentication: -10
- Blocked request: -20
- Threat event: -(score / 10)
- Auto-ban when score drops below threshold

### Request Fingerprinting

Detects 19 vulnerability scanners and 15 known bot signatures:

**Scanners detected:** sqlmap, nikto, nmap, masscan, gobuster, dirbuster, wfuzz, hydra, burpsuite, zap-proxy, acunetix, nessus, openvas, w3af, arachni, vega, skipfish, ratproxy, whatweb

**Classification:** Browser, API Client, Bot, Scanner, Unknown

### Anomaly Detection

- Learns per-IP/per-user query baselines during configurable learning period
- Tracks: query rate, common tables, query type distribution
- Alerts when deviation exceeds configurable threshold (default 3.0 std devs)
- No alerts during learning period (default 3600s)

### Auto-Blocking

- Configurable score threshold (default 80 for Moderate)
- Ban duration escalation for repeat offenders (2x multiplier)
- Maximum ban cap (default 24 hours)
- Allowlist bypass for trusted IPs
- Automatic cleanup of expired blocks

## Security Presets

| Setting | Strict | Moderate | Permissive |
|---------|--------|----------|------------|
| Auto-block threshold | 60 | 80 | 95 |
| Default ban duration | 7200s (2h) | 3600s (1h) | 300s (5m) |
| Max ban duration | 172800s (48h) | 86400s (24h) | 3600s (1h) |
| SQL injection detection | Enabled | Enabled | Enabled |
| Anomaly detection | Enabled | Enabled | Enabled |
| IP reputation | Enabled | Enabled | Enabled |
| Fingerprinting | Enabled | Enabled | Enabled |
| Auto-blocking | Enabled | Enabled | **Disabled** |

## Modules

| Module | Lines | Description |
|--------|-------|-------------|
| `lib.rs` | 617 | ShieldEngine facade, analyze_request, analyze_query |
| `sql_injection.rs` | 408 | 38 compiled regex patterns, scoring engine |
| `fingerprint.rs` | 250 | 19 scanner + 15 bot signatures, UA classification |
| `ip_reputation.rs` | 250 | Per-IP scoring, ban management, top offenders |
| `feed.rs` | 196 | ThreatFeed, ThreatStats aggregation |
| `anomaly.rs` | 172 | Baseline learning, deviation detection |
| `threat.rs` | 169 | ThreatEvent, ThreatLevel, ThreatType, ThreatAction |
| `blocker.rs` | 162 | Block/unblock, allowlist, expiry cleanup |
| `config.rs` | 134 | ShieldConfig, SecurityPreset, from_preset() |
| `policy.rs` | 129 | SecurityPolicy, custom rules, preset evaluation |
| `error.rs` | 67 | ShieldError with AegisError conversion |

## API Endpoints

All endpoints require authentication and are mounted at `/api/v1/shield/`.

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/status` | Shield status (enabled, preset, uptime, counts) |
| `GET` | `/stats` | Full threat statistics |
| `GET` | `/events?limit=50` | Recent threat events |
| `GET` | `/blocked` | List blocked IPs |
| `POST` | `/blocked` | Manually block an IP |
| `DELETE` | `/blocked/:ip` | Unblock an IP |
| `GET` | `/allowlist` | Get allowlisted IPs |
| `POST` | `/allowlist` | Add IP to allowlist |
| `DELETE` | `/allowlist/:ip` | Remove from allowlist |
| `GET` | `/policy` | Get current security policy |
| `PUT` | `/policy` | Update security policy/preset |
| `GET` | `/ip/:ip` | Get reputation details for an IP |
| `GET` | `/feed` | Threat intelligence feed (stats + recent events) |

## Configuration

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `enabled` | `bool` | `true` | Enable/disable the shield |
| `preset` | `SecurityPreset` | `Moderate` | Security preset |
| `sql_injection_enabled` | `bool` | `true` | Enable SQL injection detection |
| `anomaly_detection_enabled` | `bool` | `true` | Enable anomaly detection |
| `ip_reputation_enabled` | `bool` | `true` | Enable IP reputation tracking |
| `fingerprinting_enabled` | `bool` | `true` | Enable request fingerprinting |
| `auto_blocking_enabled` | `bool` | `true` | Enable auto-blocking |
| `auto_block_threshold` | `u32` | `80` | Score threshold for auto-block |
| `default_ban_duration_secs` | `u64` | `3600` | Default ban duration |
| `max_ban_duration_secs` | `u64` | `86400` | Maximum ban duration |
| `escalation_multiplier` | `f64` | `2.0` | Ban duration multiplier for repeat offenders |
| `max_events_in_memory` | `usize` | `10000` | Max threat events in memory |
| `anomaly_learning_period_secs` | `u64` | `3600` | Baseline learning period |
| `anomaly_deviation_threshold` | `f64` | `3.0` | Standard deviations for anomaly alert |
| `cleanup_interval_secs` | `u64` | `300` | Expired block cleanup interval |

## Usage

```rust
use aegis_shield::{ShieldEngine, ShieldConfig, RequestContext, ShieldVerdict};

let shield = ShieldEngine::new(ShieldConfig::default());

let ctx = RequestContext {
    source_ip: "192.168.1.100".to_string(),
    path: "/api/v1/query".to_string(),
    method: "POST".to_string(),
    user_agent: Some("Mozilla/5.0".to_string()),
    auth_user: None,
    body_size: 256,
    headers: Default::default(),
};

match shield.analyze_request(&ctx) {
    ShieldVerdict::Allow => println!("Request allowed"),
    ShieldVerdict::Block { reason, .. } => println!("Blocked: {}", reason),
    ShieldVerdict::RateLimit { delay_ms } => println!("Slow down: {}ms", delay_ms),
}
```

## Tests

```bash
cargo test -p aegis-shield
# 69 tests, 0 failures
```

## License

Apache-2.0
