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
| `types.rs` | `LedgerEntry`, `VerifyResult`, `LedgerError`, `chain_hash` (128-bit FNV-1a) |
| `engine.rs` | `LedgerEngine` — ledgers, append, get/range, verify, stats (tip hash), snapshot + logical clock |

## Hash chain

The entry hash is a 128-bit FNV-1a digest (two lanes, 32 hex chars) over
`prev_hash || seq || timestamp || canonical(payload)`. The first entry chains
off a fixed genesis hash. `verify` recomputes each entry's hash and checks that
its `prev_hash` matches the prior entry's hash and its `seq` matches its
position, returning the first broken sequence if the chain is invalid.

> This is a fast non-cryptographic digest — it detects corruption and
> accidental / naive tampering by re-verification, not a forging adversary.

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
