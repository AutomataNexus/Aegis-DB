# Changelog

All notable changes to Aegis-DB are documented here. This project adheres to
[Semantic Versioning](https://semver.org/) and the format of
[Keep a Changelog](https://keepachangelog.com/).

## [Unreleased]

### Added
- **Ledger / append-only paradigm** (`aegis-ledger`, the 13th engine) — named
  ledgers of immutable, **hash-chained** entries (QLDB-style audit logs). Each
  entry's hash covers the previous entry's hash, so the log is tamper-evident:
  any retroactive edit breaks every link after it and is caught by `verify`,
  which walks the chain end to end and reports the first broken sequence.
  Append-only (no entry update/delete), sequence + range reads, chain-tip hash in
  stats, snapshot persistence (appends after a restore continue the chain). REST
  under `/api/v1/ledger/*` (ledgers, entries, verify); wrapped in the
  Rust/JS/Python SDKs. The chain hash is a fast 128-bit non-cryptographic digest
  (detects corruption / naive tampering, not a forging adversary). Validated by
  chaining, verification, and tamper-detection tests.
- **Wide-column paradigm** (`aegis-widecolumn`, the 12th engine) — Cassandra /
  Bigtable-style tables: rows keyed by a row key, each row a **sparse, dynamic**
  set of columns (no fixed schema). Per-cell write timestamps with
  **last-write-wins** conflict resolution, partial-update merges, ordered range /
  prefix scans (with projection + limit), single-cell and whole-row deletes, and
  snapshot persistence (including the logical clock). REST under
  `/api/v1/widecolumn/*` (tables, rows, cells, scan); wrapped in the
  Rust/JS/Python SDKs. Validated by LWW, sparse-column, and ordered-scan tests.
- **Object / blob paradigm** (`aegis-object`, the 11th engine) — S3-style
  **buckets** of binary objects, each with a content type, a content-addressed
  **ETag** (FNV-1a fingerprint), and JSON metadata. put / get / head / delete and
  lexical prefix listing (range scan + limit), with snapshot persistence. REST
  under `/api/v1/objects/*` — the raw request body is the object content; object
  keys may contain slashes (`*key` wildcard); `GET` returns bytes with the stored
  `Content-Type` + `ETag` (or `?meta=1` for metadata). Wrapped in the
  Rust/JS/Python SDKs (binary-safe put/get). Validated by round-trip + ETag,
  overwrite, prefix-listing, and snapshot tests.
- **Columnar / OLAP paradigm** (`aegis-columnar`, the 10th engine) — named tables
  with a typed schema (`int`/`float`/`text`/`bool`), stored **column-major** (one
  vector per column) so analytical queries touch only the referenced columns.
  Predicate scans (`{column, op, value}` conditions, ANDed) with projection +
  limit, group-by aggregation (`count`/`sum`/`min`/`max`/`avg`), distinct, and
  snapshot persistence. REST under `/api/v1/columnar/*` (tables, rows, scan,
  aggregate, distinct); wrapped in the Rust/JS/Python SDKs. Validated by global +
  grouped aggregation and filtered range-predicate tests.
- **Geospatial paradigm** (`aegis-geo`, the 9th engine) — named collections of
  `{id, lat, lon, metadata}` features on a uniform grid spatial index, with
  radius / bounding-box / nearest-k queries over great-circle (**Haversine**)
  distance. Exact nearest-k (expanding-radius), exact-match metadata filtering,
  and snapshot persistence (grid rebuilt on load). REST under `/api/v1/geo/*`
  (collections, feature upsert/get/delete, radius/bbox/nearest); wrapped in the
  Rust/JS/Python SDKs. Validated by Haversine-accuracy and nearest-k-vs-brute-force
  tests.
- **Full-text search paradigm** (`aegis-fulltext`, the 8th engine) — named
  indexes of `{id, text, metadata}` documents with an inverted index and Okapi
  **BM25** ranking (the model behind Elasticsearch/Lucene). Tokenizer with
  stopword removal, exact deletes (no tombstones), exact-match metadata
  filtering, snapshot persistence. REST under `/api/v1/fts/*` (indexes, document
  upsert/get/delete, search); wrapped in the Rust/JS/Python SDKs. Validated by a
  BM25 relevance-ranking test.
- **Vector / KNN paradigm** (`aegis-vector`, the 7th engine) — dense-embedding
  collections with approximate nearest-neighbor search backed by a from-scratch
  **HNSW** index (cosine / squared-L2 / inner-product metrics), per-record JSON
  metadata with exact-match filtering, soft deletes, and snapshot persistence
  (rebuilt into HNSW on load). REST under `/api/v1/vector/*` (collections,
  upsert, batch, get/delete, search); wrapped in the Rust/JS/Python SDKs.
  HNSW validated by a recall@10 > 0.90 test against exact brute force.

## [0.4.4]

### Changed
- Re-cut of 0.4.3 (functionally identical) after a history purge that removed an
  internal pricing-config file accidentally committed in 0.4.3. No code changes.

## [0.4.3]

### Security
- Bumped `reqwest` 0.11 → 0.12 across all crates, eliminating the transitive
  `rustls` 0.21 / `rustls-webpki` 0.101 (name-constraint advisories). TLS stack
  unified on `rustls` 0.23.

## [0.4.2]

### Security
- Updated vulnerable dependencies: `openssl` 0.10.76 → 0.10.80 (multiple advisories
  incl. AES key-wrap OOB), `rustls-webpki` → 0.103.13 (CRL panic DoS), `rand`
  → 0.8.6 / 0.9.3, and the `vitest` dev-dependency → 3.2.6 (UI server RCE).

## [0.4.1]

### Added
- **Drop collection** over HTTP: `DELETE /api/v1/documents/collections/:name`
  removes a collection and all its documents (the `DocumentEngine::drop_collection`
  capability existed but had no route). Returns `{success, collection}`.

## [0.4.0] - 2026-06-14

### Added
- **SSE streaming** for channels: `GET /api/v1/streaming/channels/:channel/sse`
  opens a `text/event-stream` and pushes every published event in real time
  (built on the engine's existing broadcast subscribe). Wrapped as async
  iterators in the JS (`subscribeChannel`) and Python (`subscribe_channel`)
  SDKs. Verified end-to-end.
- **Cursor pagination** for document queries: `query`/`list` accept an opaque
  `cursor` and return `next_cursor` when a full page was returned. Offset-backed
  (stable with a `sort`). Threaded through all three SDK `query_documents` calls.
- **Prepared statements**: `POST /api/v1/prepare` (parse + plan once, returns an
  id), `POST /api/v1/prepared/execute` (bind params, skips re-parse/plan), and
  `DELETE /api/v1/prepared/:id`. Wrapped in all three SDKs
  (`prepare`/`execute_prepared`/`deallocate`). Note: prepared mutations persist
  locally but are not peer-replicated.
- **Graph API completeness**: `PUT /api/v1/graph/nodes/:id` and
  `PUT` + `DELETE /api/v1/graph/edges/:id` (edge deletion existed in the engine
  but had no route); `GraphStore::update_node` / `update_edge`.
- **Bulk / batch endpoints**:
  - KV: `POST /api/v1/kv/batch/{get,set,delete}`.
  - Documents: `POST /api/v1/documents/collections/:name/batch-insert` and
    `.../batch-delete`.
- **Rust client parity**: `aegis-client` is no longer SQL-only — added KV,
  document (CRUD + query), time-series, graph (incl. mutations), schema, health,
  metrics, and the new bulk helpers, matching the JavaScript/Python SDKs.
- **SDK bulk helpers**: `kvBatchGet/Set/Delete` and `bulkInsert/bulkDelete`
  (JavaScript) and `kv_batch_get/set/delete`, `bulk_insert/bulk_delete`
  (Python).
- **General blob compression** (`compress::encode_blob`/`decode_blob`): KV /
  graph / relational snapshots persist through a self-describing NexusCompress
  `Record`-domain blob frame (adaptive per-field entropy), with a transparent
  legacy raw-JSON fallback. Round-trip + fallback tests added.

### Changed
- Bumped the **NexusCompress** dependency to the **v0.4.0** tag (from a pre-0.2
  git rev). v0.4.0 adds adaptive entropy selection and per-field pipeline choice
  (every domain now beats zstd-3) plus the `zstd_max`/`brotli_max` codec aliases,
  so document, time-series, and blob frames get higher ratios transparently.
  Frames remain self-describing/plan-driven.
- **JavaScript & Python SDK parity**: document CRUD + query
  (create/insert/get/update/patch/delete/query), time series
  (register_metric / write / query), and graph mutations
  (create/update/delete nodes and edges) — the JS request layer also gained
  `PUT`/`PATCH` support.

### Fixed
- **Document update/patch body**: the Rust client sent the bare document on
  `PUT`/`PATCH` instead of `{ "document": ... }`, so updates were rejected.
  Corrected to match the server contract (verified end-to-end).
- **JavaScript & Python `query()` were broken against the server**: they sent
  `{ query, params: {} }` (named/object params) but the server expects
  `{ sql, params: [] }` (positional array) and wraps results in `data`. Fixed the
  request/response shape, switched the `params` argument and both query builders
  to positional `$1, $2, …` with an ordered array, and fixed JS `kvGet` to use
  the direct key endpoint. Verified end-to-end by running both SDKs against a
  live server (query+params, query builder, prepared statements, KV).
- **HTTP benchmark KV path**: the load test hit `/api/v1/kv/:key` (no such
  route) instead of `/api/v1/kv/keys/:key`, so KV requests 404'd and the 404s
  were counted as successful ops. Corrected the paths and added
  `error_for_status()` to the bench HTTP helpers so a wrong path fails instead of
  inflating throughput.

## [0.3.1] - 2026-06-13

### Security
- **Vault data-loss fix.** A wrong or transient passphrase on an existing vault
  no longer regenerates and overwrites the master key (which orphaned every
  stored secret). The vault now starts **sealed**, preserves the existing key,
  and records an audit failure; recovery is a normal unseal.
- **Durable vault key.** `new_auto` loads the persisted key and honors
  `AEGIS_VAULT_PASSPHRASE` instead of generating an ephemeral key each restart.
- **Fail-closed bootstrap.** With no users configured, authenticated routes
  return `503` until an admin is provisioned (`AEGIS_ADMIN_USERNAME` /
  `AEGIS_ADMIN_PASSWORD`); `/health` and `/login` stay open. Legacy fail-open is
  opt-in via `AEGIS_OPEN_BOOTSTRAP=true` and logs a warning per request.
- **RBAC enforcement.** `require_admin` is enforced on all privileged mutation
  routes (user/role mgmt, node lifecycle, cluster shutdown, vault secrets/seal/
  transit, OTA, backup/restore, shield mutations, GDPR erase/export); non-admins
  receive `403`. Reads remain open to any authenticated role.
- **Vault deny-by-default.** Opt-in `VaultConfig.access_default_deny` /
  `AEGIS_VAULT_DEFAULT_DENY=true` requires an explicit `AccessPolicy` grant for
  every vault op; `add/remove/list_access_policies` API added.
- **Brute-force + injection detection.** Failed logins feed the Shield
  brute-force detector (`record_failed_auth`); user SQL is screened by the
  Shield injection detector (detect-and-log).

### Fixed
- **GROUP BY** now produces one row per group (was collapsing all rows to one).
- **RIGHT/FULL OUTER JOIN** is explicitly rejected instead of silently degrading
  to INNER and dropping rows.
- **KV TTL** is enforced on `get`/`list`/`count` (was stored but never applied).
- **Backup** records `compressed=false` honestly (compression is a no-op stub).

### Added
- Direct-execution engine fast paths: `QueryEngine::get_executor`,
  `Executor::execute_update_indexed_fn` (closure-based indexed UPDATE), and
  `Executor::execute_transfer_indexed` (atomic indexed transfer — read both
  balances, verify funds, debit + credit under a single held write lock).
- Engine Criterion benchmark harness wired up; the `benchmarks` crate is now a
  workspace member, so both benchmark harnesses build from a clean checkout.
- Crate documentation pages for `aegis-vault`, `aegis-shield`,
  `aegis-monitoring`, `aegis-cli`, and `aegis-dashboard`.

### Changed
- Workspace bumped to **0.3.1**; `aegis-common` and `aegis-db-vault` published to
  crates.io at 0.3.1. **0.2.3–0.3.0 yanked** due to the vault data-loss bug.
- Benchmarks re-measured on 0.3.1 (see `benchmarks/RESULTS.md`): atomic fund
  transfer 971K TPS at 0% contention, 2.54M TPS under high contention.
- Documentation refreshed to 0.3.1 (808 tests, 16 crates).
- JavaScript, JavaScript-admin, Python, and Python-admin SDKs and the Grafana
  data-source plugin bumped to **1.1.0**.

## [0.3.0] - 2026-06-04

### Added
- **Section compression (NexusCompress).** New admin endpoint
  `POST /api/v1/admin/compress` recompresses cold data into NexusCompress
  frames for a higher ratio, in two paradigms:
  - **Time series** — cold blocks are re-encoded as NexusCompress frames.
    Reads stay transparent (each block is decoded by its codec on query), and
    recompressed blocks are flushed so they survive a restart, minimizing lock
    hold time during recompression.
  - **Document collections** — stored at rest as a single compressed
    NexusCompress `Record` frame (zstd, `NCZL` magic). Decoding transparently
    accepts both NexusCompress frames and legacy uncompressed bytes.

### Changed
- `aegis-server` and `aegis-timeseries` now depend on the published
  [`nexuscompress`](https://crates.io/crates/nexuscompress) crate (`0.1`)
  instead of a git revision, so the workspace builds from a clean
  `cargo build` with no git credentials required.
- Workspace bumped to **0.3.0**; all crates published to crates.io at 0.3.0.

## [0.2.7] - 2026-05-30

### Fixed
- **time series**: `QueryExecutor` now returns the most-recent `N` points per
  series when a `limit` is set, instead of the oldest `N`. Points are stored
  ascending by timestamp, so the previous `truncate(limit)` surfaced a stale
  "latest" reading to consumers that read `points.last()` — which could hide
  actively-reporting series downstream. Added regression test
  `test_query_limit_keeps_newest`.
- **SLSA provenance**: base64-encode the subject hash for the provenance generator.

### Changed
- **MSRV** bumped to Rust **1.85**.
- CI workflow rebuilt; resolved all workspace `clippy -D warnings` (with the
  project's documented lint allow-list) and confirmed `cargo fmt` clean.

### Added
- Admin SDKs for **Python** and **JavaScript/TypeScript**.
- SLSA L3 release artifacts published on tag `v*`.

## [0.2.6] - 2026-03-29

### Changed
- Wired the dashboard to real APIs; removed all stubbed/mock data paths.

## [0.2.5] - 2026-03-26

### Changed
- New logo; refreshed docs and README badges.

---

Earlier releases predate this changelog. Aegis-DB is a single-binary,
multi-paradigm database (SQL · Documents · Key-Value · Time Series · Graph ·
Streaming, plus Vault and Shield) written in Rust — 13 crates, ~60,000 LOC.
