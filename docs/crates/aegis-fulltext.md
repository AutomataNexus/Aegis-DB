---
layout: default
title: aegis-fulltext
parent: Crate Documentation
nav_order: 17
description: "Full-text search engine (inverted index + BM25)"
---

# aegis-fulltext

Full-text search engine — named indexes of `{id, text, metadata}` documents with
an inverted index and Okapi BM25 ranking. The eighth data paradigm.

## Overview

`aegis-fulltext` tokenizes document text into an inverted index and answers
ranked relevance queries with BM25, the standard scoring model used by
Elasticsearch / Lucene.

## Modules

| Module | Responsibility |
|--------|----------------|
| `tokenize.rs` | Lowercase + ASCII-alphanumeric tokenization, stopword removal |
| `index.rs` | `InvertedIndex` — postings, doc lengths, BM25 scoring (`Bm25Params`) |
| `engine.rs` | `FullTextEngine` — named indexes, upsert/get/delete, ranked search, metadata filter, snapshot |

## Ranking

BM25 with `k1 = 1.2`, `b = 0.75`:

```
score(q, d) = Σ_t  IDF(t) · (tf · (k1+1)) / (tf + k1·(1 − b + b·|d|/avgdl))
IDF(t)      = ln(1 + (N − df + 0.5) / (df + 0.5))
```

Each `SearchHit` carries the BM25 `score` (higher = more relevant) and the
document's metadata.

## API Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET / POST | `/api/v1/fts/indexes` | List / create (`{name}`) |
| GET / DELETE | `/api/v1/fts/indexes/:name` | Stats / drop |
| POST | `/api/v1/fts/indexes/:name/documents` | Index `{id, text, metadata?}` |
| GET / DELETE | `/api/v1/fts/indexes/:name/documents/:id` | Get / delete |
| POST | `/api/v1/fts/indexes/:name/search` | BM25 search `{query, k, filter?}` |

All endpoints require authentication.

## Persistence

An index snapshot (documents) is written to `fulltext.ncb` as a NexusCompress
blob frame on graceful shutdown and the inverted index is rebuilt from it on
startup.

## Tests

Workspace total includes BM25 ranking, tokenization/stopwords, CRUD + metadata
filter, exact delete, error paths, and snapshot round-trip.
