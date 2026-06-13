---
layout: default
title: aegis-vault
parent: Crate Documentation
nav_order: 11
description: "Integrated Rust-native encrypted secrets manager"
---

# aegis-vault

Integrated, Rust-native encrypted secrets manager (published on crates.io as
`aegis-db-vault`). Auto-initializes at startup and provides versioned secret
storage, transit encryption, access control, and audit logging — no external
Vault server required.

## Overview

`aegis-vault` is a self-contained secrets engine embedded in the server. All
secret values are encrypted at rest with AES-256-GCM under a PBKDF2-derived
master key that follows a seal/unseal lifecycle.

Key features:

- **AES-256-GCM** encryption for every secret value
- **PBKDF2-derived master key** with a seal/unseal lifecycle
- **Versioned secrets** with configurable retention
- **Transit encryption** engine (encryption-as-a-service — encrypt/decrypt
  without exposing the key)
- **Component-based access control** policies
- **Audit log** (bounded, in-memory) for every operation
- **Disk persistence** with atomic writes

## Modules

| Module | Responsibility |
|--------|----------------|
| `master_key.rs` | PBKDF2 master key derivation, seal/unseal lifecycle |
| `secret.rs` | Versioned secret values and retention |
| `store.rs` | Encrypted on-disk store with atomic writes |
| `transit.rs` | Transit encryption (encrypt/decrypt as a service) |
| `access.rs` | Component-based `AccessPolicy` access control |
| `audit.rs` | Bounded in-memory audit log |
| `rotation.rs` | Secret/key rotation |
| `provider.rs` | Pluggable secret provider abstraction |
| `config.rs` | `VaultConfig` (incl. `access_default_deny`) |

## Security Hardening (v0.3.1)

- **Never regenerate the master key on a failed unseal.** A wrong or transient
  passphrase on an existing vault previously regenerated and overwrote
  `vault.key`, orphaning every stored secret. The vault now starts **sealed**,
  preserves the existing key blob, logs an error, and records an audit failure.
  Recovery is a normal unseal with the correct passphrase.
- **Durable key on `new_auto`.** Loads the persisted key and honors
  `AEGIS_VAULT_PASSPHRASE`, persisting on first run instead of generating an
  ephemeral key every restart.
- **Deny-by-default mode.** With `VaultConfig.access_default_deny` (or
  `AEGIS_VAULT_DEFAULT_DENY=true`) every operation requires an explicit
  `AccessPolicy` grant. Manage grants with `add_access_policy` /
  `remove_access_policy` / `list_access_policies`. Default remains allow-all
  for backwards compatibility.

> **Published as `aegis-db-vault` 0.3.1.** Versions 0.2.3–0.3.0 are **yanked**
> because they carried the wrong-passphrase data-loss bug fixed above.

## Tests

808 tests (workspace total) covering master-key seal/unseal, encrypted store
round-trips, transit encryption, access policies, rotation, and the
wrong-passphrase-preserves-data regression.
