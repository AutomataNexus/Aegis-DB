---
layout: default
title: aegis-ledger
parent: Crate Documentation
nav_order: 22
description: "Ledger / append-only engine (hash-chained, verifiable)"
---

# aegis-ledger

Ledger / append-only engine — named ledgers of immutable, hash-chained entries.
Each entry's hash covers the previous entry's hash, so the log is tamper-evident
and verifiable end to end. The thirteenth data paradigm.

## Overview

`aegis-ledger` stores an append-only sequence of entries per ledger. Each
`append` adds an entry whose hash is computed over the previous hash, the
sequence number, the timestamp, and the canonical payload — forming a chain. A
retroactive edit to any entry changes its hash and breaks every subsequent link,
which `verify` detects and localizes.

## Modules

| Module | Responsibility |
|--------|----------------|
| `types.rs` | `LedgerEntry`, `VerifyResult`, `LedgerError`, `chain_hash` (SHA-256), `chain_hash_fnv1a` (legacy v1), `genesis_hash` |
| `engine.rs` | `LedgerEngine` — ledgers, append, get/range, verify, stats (tip hash), snapshot + logical clock |

## Hash chain

The entry hash is SHA-256 (64 hex chars) over
`prev_hash || seq || timestamp || canonical(payload)`. The first entry chains
off a genesis hash of 64 zeros.

Each ledger records a `hash_version`. Ledgers created from v0.6 on are version 2
(SHA-256). Ledgers from older snapshots have no version field and load as
version 1, the original 128-bit FNV-1a digest (two lanes, 32 hex chars, 32-zero
genesis). They keep verifying and chaining under that algorithm, so an upgrade
never reports a broken chain. `verify` recomputes each entry's hash and checks that
its `prev_hash` matches the prior entry's hash and its `seq` matches its
position, returning the first broken sequence if the chain is invalid.

> SHA-256 makes the chain tamper-evident: altering any entry breaks every later
> link. Legacy version 1 ledgers use a non-cryptographic digest that detects
> corruption and naive tampering, not a forging adversary.

## API Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET / POST | `/api/v1/ledger/ledgers` | List / create (`{name}`) |
| GET / DELETE | `/api/v1/ledger/ledgers/:name` | Stats / drop |
| GET / POST | `/api/v1/ledger/ledgers/:name/entries` | Read range (`?start=&limit=`) / append `{payload, timestamp?}` |
| GET | `/api/v1/ledger/ledgers/:name/entries/:seq` | Get entry by sequence |
| GET | `/api/v1/ledger/ledgers/:name/verify` | Verify the hash chain |

All endpoints require authentication. `verify` returns
`{ valid, entries, broken_at }`.

## Persistence

The full ledger set plus the logical clock is written to `ledger.ncb` as a
NexusCompress blob frame on graceful shutdown and reloaded on startup; appends
after a restore continue the same chain.

## Tests

Workspace total includes sequence/hash chaining, full-chain verification,
tamper-detection, range + stats, error paths, and snapshot round-trip with
continued appends.
