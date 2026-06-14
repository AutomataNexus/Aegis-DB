<p align="center">
  <img src="https://img.shields.io/badge/crate-0.4.4-green.svg" alt="Version">
</p>

# aegis-fulltext

Full-text search engine for Aegis-DB — named indexes of `{id, text, metadata}`
documents with an inverted index and **Okapi BM25** ranking. The eighth data
paradigm in the Aegis engine.

## Features

- **Inverted index** (`term → postings`) with per-document lengths, so BM25's
  length normalization is O(1).
- **BM25 ranking** (`k1=1.2`, `b=0.75` defaults) — the standard relevance model.
- **Tokenizer** — lowercase, ASCII-alphanumeric runs, English stopword removal,
  1-char tokens dropped.
- **Exact deletes** — a document is removed by re-applying its token list, so
  results stay exact (no tombstones); upsert = delete-then-add.
- **Metadata filtering** — exact-match JSON filter on results.
- **Snapshot persistence** — a serializable snapshot the server stores and
  rebuilds the index from on load.

## Example

```rust
use aegis_fulltext::FullTextEngine;

let fts = FullTextEngine::new();
fts.create_index("articles")?;
fts.upsert("a1", "Aegis is a multi-paradigm database in Rust", serde_json::json!({"lang": "rust"}))?;

let hits = fts.search("articles", "rust database", 10, &serde_json::Value::Null)?;
for hit in hits {
    println!("{}  bm25={:.3}", hit.id, hit.score);
}
```

## HTTP API (via `aegis-server`)

| Method | Path | Description |
|--------|------|-------------|
| GET/POST | `/api/v1/fts/indexes` | List / create an index (`{name}`) |
| GET/DELETE | `/api/v1/fts/indexes/:name` | Stats / drop |
| POST | `/api/v1/fts/indexes/:name/documents` | Index `{id, text, metadata?}` |
| GET/DELETE | `/api/v1/fts/indexes/:name/documents/:id` | Get / delete a document |
| POST | `/api/v1/fts/indexes/:name/search` | BM25 search `{query, k, filter?}` |

## Tests

Workspace total includes BM25 relevance ranking, tokenization/stopwords, CRUD +
metadata filter, exact delete, and snapshot round-trip.
