---
layout: default
title: aegis-widecolumn
parent: Crate Documentation
nav_order: 21
description: "Wide-column engine (row-keyed sparse columns, per-cell timestamps, LWW)"
---

# aegis-widecolumn

Wide-column engine — Cassandra / Bigtable-style tables: rows keyed by a row key,
each row a sparse, dynamic set of columns, with per-cell timestamps and
last-write-wins conflict resolution. The twelfth data paradigm.

## Overview

`aegis-widecolumn` stores rows keyed by a string row key; each row is a sorted
map of column name → cell, and cells are added dynamically (no fixed schema).
Every cell records the timestamp of the write that set it; a later write only
wins when its timestamp is strictly greater. Rows are kept sorted by key, so
range and prefix scans are ordered.

## Modules

| Module | Responsibility |
|--------|----------------|
| `types.rs` | `Cell` (value + timestamp), `RowResult`, `WideColumnError` |
| `engine.rs` | `WideColumnEngine` — tables, put (LWW), get/get_cell, delete cell/row, ordered scan, snapshot + logical clock |

## Conflict resolution

Each `put` stamps every written column with a timestamp — supplied explicitly or
drawn from a monotonic logical clock. An existing cell is overwritten only when
the new timestamp is greater; equal timestamps keep the existing value. This is
classic last-write-wins, the same model Cassandra uses for cell reconciliation.

## API Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET / POST | `/api/v1/widecolumn/tables` | List / create (`{name}`) |
| GET / DELETE | `/api/v1/widecolumn/tables/:name` | Stats / drop |
| POST | `/api/v1/widecolumn/tables/:name/scan` | Scan `{start?, end?, prefix?, columns?, limit?}` |
| PUT | `/api/v1/widecolumn/tables/:name/rows/:row` | Set columns `{columns, timestamp?}` |
| GET | `/api/v1/widecolumn/tables/:name/rows/:row` | Get a row (`?columns=a,b`) |
| DELETE | `/api/v1/widecolumn/tables/:name/rows/:row` | Delete a row |
| DELETE | `/api/v1/widecolumn/tables/:name/rows/:row/columns/:column` | Delete a cell |

All endpoints require authentication. A row response carries `columns` (name →
value) and `timestamps` (name → write timestamp).

## Persistence

The full table set plus the logical clock is written to `widecolumn.ncb` as a
NexusCompress blob frame on graceful shutdown and reloaded on startup.

## Tests

Workspace total includes sparse/dynamic columns, partial-update merge,
last-write-wins, projection + cell get, ordered range/prefix scans, cell + row
deletes, error paths, and snapshot round-trip.
