<p align="center">
  <img src="https://img.shields.io/badge/crate-0.5.0-green.svg" alt="Version">
</p>

# aegis-ledger

Ledger / append-only engine for Aegis-DB — named ledgers of immutable,
**hash-chained** entries. Each entry's hash covers the previous entry's hash, so
the log is tamper-evident: any retroactive edit breaks every link after it and is
caught by verification. The thirteenth data paradigm in the Aegis engine.

## Features

- **Append-only** — entries are never updated or deleted; each `append` adds a
  new entry with the next sequence number.
- **Hash chain** — each entry's hash is computed over `prev_hash`, `seq`,
  `timestamp`, and the canonical payload, linking it to the prior entry (the
  first links to a fixed genesis hash).
- **Verification** — `verify` walks the chain end to end, recomputing every hash
  and checking every link, and reports the first broken sequence (if any).
- **Reads** — fetch a single entry by sequence, or read a range from an offset
  with a limit.
- **Chain tip** — ledger stats expose the entry count and the tip hash.
- **Snapshot persistence** — the whole ledger set (and the logical clock)
  serializes to a snapshot the server stores and reloads on startup; appends
  after a restore continue the same chain.

> The chain hash is a fast 128-bit non-cryptographic digest (FNV-1a, two lanes).
> It detects corruption and accidental / naive tampering by re-verification; it
> is not a defense against a forging adversary.

## Example

```rust
use aegis_ledger::LedgerEngine;

let ledger = LedgerEngine::new();
ledger.create_ledger("audit")?;

let e0 = ledger.append("audit", serde_json::json!({"event": "create"}), None)?;
let e1 = ledger.append("audit", serde_json::json!({"event": "update"}), None)?;
assert_eq!(e1.prev_hash, e0.hash);          // chained

let report = ledger.verify("audit")?;        // { valid: true, entries: 2, broken_at: None }
```

## HTTP API (via `aegis-server`)

| Method | Path | Description |
|--------|------|-------------|
| GET/POST | `/api/v1/ledger/ledgers` | List / create a ledger (`{name}`) |
| GET/DELETE | `/api/v1/ledger/ledgers/:name` | Stats (count + tip hash) / drop |
| GET/POST | `/api/v1/ledger/ledgers/:name/entries` | Read range (`?start=&limit=`) / append `{payload, timestamp?}` |
| GET | `/api/v1/ledger/ledgers/:name/entries/:seq` | Get an entry by sequence |
| GET | `/api/v1/ledger/ledgers/:name/verify` | Verify the hash chain |

## Tests

Workspace total includes sequence/hash chaining, full-chain verification,
tamper-detection (a doctored entry breaks `verify` at the right sequence),
range + stats, error paths, and snapshot round-trip with continued appends.
