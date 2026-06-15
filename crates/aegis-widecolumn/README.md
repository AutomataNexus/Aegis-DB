<p align="center">
  <img src="https://img.shields.io/badge/crate-0.5.0-green.svg" alt="Version">
</p>

# aegis-widecolumn

Wide-column engine for Aegis-DB — Cassandra / Bigtable-style tables: rows keyed
by a **row key**, each row a **sparse, dynamic** set of columns. Every cell
carries a write timestamp and conflicting writes resolve **last-write-wins**.
The twelfth data paradigm in the Aegis engine.

## Features

- **Sparse, dynamic columns** — each row may carry a different set of columns;
  no fixed schema. Columns are added simply by writing them.
- **Per-cell timestamps + LWW** — every cell records the timestamp of the write
  that set it; a write only overwrites a cell when its timestamp is greater
  (equal timestamps keep the existing value). Timestamps may be supplied or
  assigned from a monotonic logical clock.
- **Partial updates** — a `put` merges columns into a row without disturbing the
  ones it doesn't mention.
- **Ordered range / prefix scans** — rows are stored sorted by key, so scans by
  `[start, end)` range or key prefix are ordered range scans, with projection
  and a result limit.
- **Cell + row deletes** — delete a single column or a whole row; deleting a
  row's last column drops the row.
- **Snapshot persistence** — the whole table set (and the logical clock)
  serializes to a snapshot the server stores and reloads on startup.

## Example

```rust
use aegis_widecolumn::WideColumnEngine;

let wc = WideColumnEngine::new();
wc.create_table("users")?;

// Sparse rows — different columns per row.
let mut cols = serde_json::Map::new();
cols.insert("name".into(), serde_json::json!("Alice"));
cols.insert("age".into(), serde_json::json!(30));
wc.put("users", "user:1", cols, None)?;

let row = wc.get("users", "user:1", &[])?.unwrap();    // all columns
let name = wc.get_cell("users", "user:1", "name")?;     // single cell

// Ordered prefix scan.
let page = wc.scan("users", None, None, Some("user:"), &[], Some(50))?;
```

## HTTP API (via `aegis-server`)

| Method | Path | Description |
|--------|------|-------------|
| GET/POST | `/api/v1/widecolumn/tables` | List / create a table (`{name}`) |
| GET/DELETE | `/api/v1/widecolumn/tables/:name` | Stats (rows + cells) / drop |
| POST | `/api/v1/widecolumn/tables/:name/scan` | Scan `{start?, end?, prefix?, columns?, limit?}` |
| PUT | `/api/v1/widecolumn/tables/:name/rows/:row` | Set columns `{columns, timestamp?}` |
| GET | `/api/v1/widecolumn/tables/:name/rows/:row` | Get a row (`?columns=a,b`) |
| DELETE | `/api/v1/widecolumn/tables/:name/rows/:row` | Delete a row |
| DELETE | `/api/v1/widecolumn/tables/:name/rows/:row/columns/:column` | Delete a single cell |

## Tests

Workspace total includes sparse/dynamic columns, partial-update merge,
last-write-wins by timestamp, projection + single-cell get, ordered range/prefix
scans, cell + row deletes, error paths, and snapshot round-trip.
