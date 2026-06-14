<p align="center">
  <img src="https://raw.githubusercontent.com/AutomataNexus/assets/main/AegisDB-logo.png" alt="AegisDB Logo" width="300">
</p>

# aegis-vault

<p align="center">
  <a href="https://opensource.org/licenses/Apache-2.0"><img src="https://img.shields.io/badge/License-Apache%202.0-blue.svg" alt="License"></a>
  <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/Rust-1.75%2B-orange.svg" alt="Rust"></a>
  <img src="https://img.shields.io/badge/crate-0.3.1-green.svg" alt="Version">
  <img src="https://img.shields.io/badge/tests-63%20passing-brightgreen.svg" alt="Tests">
  <a href="../../README.md"><img src="https://img.shields.io/badge/part%20of-AegisDB-teal.svg" alt="AegisDB"></a>
</p>

Integrated Rust-native secrets management for the Aegis Database Platform.

## Overview

`aegis-vault` is a pure-Rust encrypted secrets manager that auto-initializes at server startup. It eliminates the need for an external HashiCorp Vault deployment for most use cases while remaining fully compatible when one is available.

Secrets are encrypted at rest using AES-256-GCM with master keys derived via PBKDF2. The vault supports secret versioning, transit encryption (encrypt/decrypt without storing), access control policies, and comprehensive audit logging.

## Architecture

```
┌──────────────────────────────────────────────────────┐
│                    AegisVault                          │
├──────────────────────────────────────────────────────┤
│  ┌──────────────┐ ┌───────────────┐ ┌─────────────┐ │
│  │ Seal Manager  │ │ Transit       │ │ Audit Log   │ │
│  │ AES-256-GCM  │ │ Engine        │ │ (bounded    │ │
│  │ + PBKDF2     │ │ (named keys)  │ │  ring buf)  │ │
│  └──────┬───────┘ └───────────────┘ └─────────────┘ │
│         │                                             │
│  ┌──────▼────────────────────────────────────────┐   │
│  │            Encrypted Store (VaultStore)         │   │
│  │  ┌─────────┐ ┌───────────┐ ┌────────────────┐ │   │
│  │  │ Secrets  │ │ Versions  │ │ Access Control │ │   │
│  │  │ (KV Map) │ │ (N hist)  │ │ (Policies)     │ │   │
│  │  └─────────┘ └───────────┘ └────────────────┘ │   │
│  └────────────────────┬──────────────────────────┘   │
│                       │                               │
│  ┌────────────────────▼──────────────────────────┐   │
│  │  Atomic Disk Persistence (vault.dat)           │   │
│  │  write-tmp → rename (crash safe)               │   │
│  └───────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────┘
```

## Features

| Feature | Description |
|---------|-------------|
| **AES-256-GCM encryption** | Every secret value encrypted individually with random nonces |
| **Auto-initialization** | Generates master key on first run, no manual setup required |
| **Seal/Unseal** | Passphrase-derived keys via PBKDF2 (ring crate, 100K iterations) |
| **Secret versioning** | Keeps N previous versions per secret (default 10, configurable) |
| **Transit encryption** | Encrypt/decrypt arbitrary data with named keys without storing it |
| **Access policies** | Component-based + prefix-based ACL (e.g., "db/*", "tls/*") |
| **Audit logging** | Every get/set/delete/seal/unseal recorded with timestamp and component |
| **Atomic persistence** | Write-tmp-then-rename pattern prevents corruption on crash |
| **Provider chain** | Integrates as first provider: vault -> external Vault -> env vars |
| **Zero external deps** | No separate process, no JVM, no Go runtime — pure Rust |

## Modules

| Module | Lines | Description |
|--------|-------|-------------|
| `lib.rs` | 484 | AegisVault facade, init/new_auto, all public methods |
| `store.rs` | 443 | Encrypted KV store with versioning and disk persistence |
| `master_key.rs` | 387 | SealManager, PBKDF2 key derivation, AES encrypt/decrypt |
| `access.rs` | 239 | AccessPolicy, AccessController with component/prefix ACL |
| `transit.rs` | 200 | TransitEngine with named AES-256-GCM keys |
| `audit.rs` | 194 | VaultAuditLog, bounded VecDeque, VaultOperation enum |
| `secret.rs` | 150 | Secret, SecretVersion, SecretMetadata types |
| `rotation.rs` | 108 | TTL checking, rotation loop scheduler |
| `config.rs` | 101 | VaultConfig with env var support |
| `provider.rs` | 101 | SecretsProvider wrapper for integration |
| `error.rs` | 68 | VaultError enum with AegisError conversion |

## API Endpoints

All endpoints require authentication and are mounted at `/api/v1/vault/`.

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/status` | Vault status (sealed, secret count, uptime) |
| `POST` | `/seal` | Seal the vault |
| `POST` | `/unseal` | Unseal with passphrase |
| `GET` | `/secrets?prefix=` | List secret keys |
| `GET` | `/secrets/:key` | Get secret value |
| `PUT` | `/secrets/:key` | Set/update secret |
| `DELETE` | `/secrets/:key` | Delete secret |
| `POST` | `/transit/encrypt` | Encrypt data with named key |
| `POST` | `/transit/decrypt` | Decrypt data with named key |
| `POST` | `/transit/keys` | Create transit key |
| `GET` | `/transit/keys` | List transit keys |
| `GET` | `/audit?limit=100` | Get audit log entries |

## Configuration

| Field | Type | Default | Env Var | Description |
|-------|------|---------|---------|-------------|
| `data_dir` | `Option<PathBuf>` | None | — | Directory for vault data files |
| `auto_unseal` | `bool` | `true` | — | Auto-generate/load master key at startup |
| `passphrase` | `Option<String>` | None | `AEGIS_VAULT_PASSPHRASE` | Passphrase for key derivation |
| `max_versions` | `u32` | `10` | — | Max secret versions to retain |
| `rotation_check_interval_secs` | `u64` | `3600` | — | How often to check for expired secrets |
| `audit_log_max_entries` | `usize` | `10000` | — | Max audit log entries in memory |

## Usage

```rust
use aegis_vault::{AegisVault, VaultConfig};

// Auto-initialize (sync, for embedding)
let vault = AegisVault::new_auto(Some("/data/vault".into()));

// Store a secret
vault.set("db/password", "s3cret!", "my-app").unwrap();

// Retrieve it
let password = vault.get("db/password", "my-app").unwrap();

// Transit encryption (encrypt without storing)
vault.transit_create_key("my-key").unwrap();
let encrypted = vault.transit_encrypt("my-key", b"sensitive data").unwrap();
let decrypted = vault.transit_decrypt("my-key", &encrypted).unwrap();
```

## Tests

```bash
cargo test -p aegis-vault
# 63 tests, 0 failures
```

## License

Apache-2.0
