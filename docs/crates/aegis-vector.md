---
layout: default
title: aegis-vector
parent: Crate Documentation
nav_order: 16
description: "Vector / KNN engine with a from-scratch HNSW index"
---

# aegis-vector

Vector / KNN engine — dense-embedding collections with approximate
nearest-neighbor search backed by a from-scratch HNSW index. The seventh data
paradigm in the Aegis engine.

## Overview

`aegis-vector` stores collections of fixed-dimensionality embeddings and answers
top-`k` nearest-neighbor queries with metadata filtering. The index is an HNSW
graph implemented from scratch (no external ANN crate), validated against exact
brute-force search.

## Modules

| Module | Responsibility |
|--------|----------------|
| `types.rs` | `Metric` (cosine / l2 / dot) + distance math, `VectorRecord`, `SearchHit`, errors |
| `hnsw.rs` | `HnswIndex` — multi-layer NSW graph, level-decaying insertion, greedy descent, diversity neighbor heuristic, soft deletes |
| `engine.rs` | `VectorEngine` — collections, upsert/get/delete/search, metadata filter, serializable snapshot |

## Metrics

- **cosine** — vectors are L2-normalized on insert; distance is `1 − cosine_sim`.
- **l2** — squared Euclidean (monotonic with L2, no sqrt).
- **dot** — negative inner product (higher dot ⇒ closer).

Each `SearchHit` carries a similarity-style `score` (higher = more similar)
alongside the raw metric `distance`.

## API Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET / POST | `/api/v1/vector/collections` | List / create (`{name, dim, metric}`) |
| GET / DELETE | `/api/v1/vector/collections/:name` | Stats / drop |
| POST | `/api/v1/vector/collections/:name/upsert` | Upsert `{id, vector, metadata?}` |
| POST | `/api/v1/vector/collections/:name/batch` | Batch upsert |
| GET / DELETE | `/api/v1/vector/collections/:name/vectors/:id` | Get / delete |
| POST | `/api/v1/vector/collections/:name/search` | KNN `{vector, k, ef?, filter?}` |

All endpoints require authentication.

## Persistence

A collection snapshot (config + live records) is written to `vectors.ncb` as a
NexusCompress blob frame on graceful shutdown and the HNSW index is rebuilt from
it on startup.

## Tests

808+ tests (workspace total) including HNSW recall@10 vs brute force, metric
ranking, CRUD + metadata filter, error paths, and snapshot round-trip.
