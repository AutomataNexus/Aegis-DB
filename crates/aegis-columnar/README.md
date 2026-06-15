<p align="center">
  <img src="https://img.shields.io/badge/crate-0.5.0-green.svg" alt="Version">
</p>

# aegis-columnar

Columnar / OLAP engine for Aegis-DB — named tables with a fixed typed schema,
stored **column-major** (one vector per column), with predicate scans and
group-by aggregation (`count` / `sum` / `min` / `max` / `avg`). The tenth data
paradigm in the Aegis engine, and the analytical counterpart to the row-oriented
SQL engine.

## Features

- **Column-major storage** — each column is its own vector, so an aggregation
  touches only the columns it references (not whole rows).
- **Typed schema** — `int` / `float` / `text` / `bool` columns; values are
  coerced on insert and a bad value leaves the table unchanged.
- **Predicate scans** — projection + a list of `{column, op, value}` conditions
  (`eq`/`ne`/`lt`/`lte`/`gt`/`gte`, ANDed) + optional row limit.
- **Group-by aggregation** — zero or more group-by columns × any mix of
  `count` / `sum` / `min` / `max` / `avg`; `count(*)` counts rows.
- **Distinct** — distinct non-null values of a column.
- **Snapshot persistence** — the whole table set serializes to a snapshot the
  server stores and reloads on startup.

## Example

```rust
use aegis_columnar::{ColumnarEngine, ColumnDef, ColumnType, AggSpec, AggFunc};

let olap = ColumnarEngine::new();
olap.create_table("sales", vec![
    ColumnDef { name: "region".into(), ty: ColumnType::Text },
    ColumnDef { name: "amount".into(), ty: ColumnType::Float },
])?;

olap.insert_many("sales", &[
    serde_json::json!({"region": "east", "amount": 100.0}),
    serde_json::json!({"region": "west", "amount": 200.0}),
    serde_json::json!({"region": "east", "amount": 75.0}),
])?;

// sum(amount) per region
let groups = olap.aggregate(
    "sales",
    &["region".into()],
    &[AggSpec { func: AggFunc::Sum, column: "amount".into() }],
    &[],
)?;
```

## HTTP API (via `aegis-server`)

| Method | Path | Description |
|--------|------|-------------|
| GET/POST | `/api/v1/columnar/tables` | List / create a table (`{name, columns}`) |
| GET/DELETE | `/api/v1/columnar/tables/:name` | Stats (rows + schema) / drop |
| POST | `/api/v1/columnar/tables/:name/rows` | Insert `{rows: [...]}` |
| POST | `/api/v1/columnar/tables/:name/scan` | Scan `{columns?, filter?, limit?}` |
| POST | `/api/v1/columnar/tables/:name/aggregate` | Aggregate `{group_by?, aggregates, filter?}` |
| GET | `/api/v1/columnar/tables/:name/distinct/:column` | Distinct column values |

## Tests

Workspace total includes global + grouped aggregation, filtered min/max with
range predicates, projection + limit scans, distinct, schema/type validation
errors, and snapshot round-trip.
