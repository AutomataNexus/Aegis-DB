# Changelog

All notable changes to Aegis-DB are documented here. This project adheres to
[Semantic Versioning](https://semver.org/) and the format of
[Keep a Changelog](https://keepachangelog.com/).

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
