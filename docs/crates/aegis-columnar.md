---
layout: default
title: aegis-columnar
parent: Crate Documentation
nav_order: 19
description: "Columnar / OLAP engine (column-major store + group-by aggregation)"
---

# aegis-columnar

Columnar / OLAP engine — named tables with a fixed typed schema, stored
column-major (one vector per column), with predicate scans and group-by
aggregation. The tenth data paradigm, and the analytical counterpart to the
row-oriented SQL engine.

## Overview

`aegis-columnar` stores each column as its own vector, so analytical queries
(aggregations, group-bys, column scans) read only the columns they reference.
Values are typed (`int`/`float`/`text`/`bool`) and coerced on insert.

## Modules

| Module | Responsibility |
|--------|----------------|
| `types.rs` | `ColumnType` / `Value`, `Condition` (`CompareOp`), `AggSpec` (`AggFunc`), `GroupRow`, `ColumnarError` |
| `table.rs` | `Table` — column-major store, predicate `matching_rows`, scan, group-by aggregation, distinct |
| `engine.rs` | `ColumnarEngine` — named tables, insert/scan/aggregate/distinct, snapshot |

## Aggregation

Group by zero or more columns, computing any mix of `count` / `sum` / `min` /
`max` / `avg`. `count(*)` counts rows; `count(col)` counts non-null cells.
Numeric aggregates ignore null / non-numeric cells. Each result row carries the
group-by key values plus one value per requested aggregate (in request order).

## Filtering

A filter is a list of `{column, op, value}` conditions, ANDed. `op` is one of
`eq` / `ne` / `lt` / `lte` / `gt` / `gte`. Numerics compare numerically, text
lexically; null compares equal only to null.

## API Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET / POST | `/api/v1/columnar/tables` | List / create (`{name, columns}`) |
| GET / DELETE | `/api/v1/columnar/tables/:name` | Stats / drop |
| POST | `/api/v1/columnar/tables/:name/rows` | Insert `{rows: [...]}` |
| POST | `/api/v1/columnar/tables/:name/scan` | Scan `{columns?, filter?, limit?}` |
| POST | `/api/v1/columnar/tables/:name/aggregate` | Aggregate `{group_by?, aggregates, filter?}` |
| GET | `/api/v1/columnar/tables/:name/distinct/:column` | Distinct values |

All endpoints require authentication.

## Persistence

The full table set is written to `columnar.ncb` as a NexusCompress blob frame on
graceful shutdown and reloaded on startup.

## Tests

Workspace total includes global + grouped aggregation, filtered min/max range
predicates, projection + limit scans, distinct, validation errors, and snapshot
round-trip.
