<p align="center">
  <img src="https://img.shields.io/badge/crate-0.4.4-green.svg" alt="Version">
</p>

# aegis-vector

Vector / KNN engine for Aegis-DB — dense-embedding collections with approximate
nearest-neighbor search backed by a **from-scratch HNSW index**. The seventh
data paradigm in the Aegis engine.

## Features

- **HNSW index** (Malkov & Yashunin, 2018), implemented from scratch — multi-layer
  navigable-small-world graph, level-decaying insertion, greedy descent through
  upper layers, and the diversity neighbor-selection heuristic (their Algorithm 4)
  for high recall at low degree. Validated by a recall@10 > 0.90 test against
  exact brute-force search.
- **Three metrics** — `cosine` (vectors L2-normalized on insert), squared-`l2`,
  and inner-product (`dot`).
- **Collections** with a fixed dimensionality + metric.
- **Upsert / get / delete** by opaque string id, **batch upsert**, and **soft
  deletes** (tombstoned, excluded from results).
- **KNN search** returning ranked hits with a similarity `score`, raw `distance`,
  and per-record JSON `metadata`, plus **exact-match metadata filtering**.
- **Snapshot persistence** — a serializable snapshot (config + records) that the
  server stores as a NexusCompress blob frame and rebuilds into HNSW on load.

## Example

```rust
use aegis_vector::{Metric, VectorEngine};

let engine = VectorEngine::new();
engine.create_collection("docs", 384, Metric::Cosine)?;
engine.upsert("doc-1", &embedding, serde_json::json!({ "source": "wiki" }))?;

let hits = engine.search("docs", &query, 10, None, &serde_json::json!({ "source": "wiki" }))?;
for hit in hits {
    println!("{}  score={:.3}", hit.id, hit.score);
}
```

## HTTP API (via `aegis-server`)

| Method | Path | Description |
|--------|------|-------------|
| GET/POST | `/api/v1/vector/collections` | List / create a collection (`{name, dim, metric}`) |
| GET/DELETE | `/api/v1/vector/collections/:name` | Stats / drop |
| POST | `/api/v1/vector/collections/:name/upsert` | Upsert `{id, vector, metadata?}` |
| POST | `/api/v1/vector/collections/:name/batch` | Batch upsert `{vectors: [...]}` |
| GET/DELETE | `/api/v1/vector/collections/:name/vectors/:id` | Get / delete a vector |
| POST | `/api/v1/vector/collections/:name/search` | KNN `{vector, k, ef?, filter?}` |

## Tests

808+ tests (workspace total) including HNSW recall vs brute force, metric
ranking, CRUD + metadata filter, and snapshot round-trip.
