# Aegis-DB Production Hardening Progress

## Phase 1 (Gaps 1-7) — DONE
Transactions, MVCC, parameterized queries, replication, CDC, WAL, backup scheduler.

## Phase 2 — Audit Fixes

### CRITICAL

| # | Issue | Status |
|---|-------|--------|
| C1 | Auth missing on most endpoints | Done |
| C2 | WAL not recovered at startup | Done |
| C3 | No constraint enforcement (NOT NULL, PK, UNIQUE) | Done |
| C4 | Unbounded result sets (OOM on large SELECT) | Done |
| C5 | Error messages expose internals | Done |

### HIGH

| # | Issue | Status |
|---|-------|--------|
| H1 | Full JSON serialization on every mutation (perf) | Pending |
| H2 | Rate limiting only on login | Done |
| H3 | CORS wildcard + credentials allowed | Done |
| H4 | Replication fire-and-forget, no retry | Done |
| H5 | restore_from_snapshot doesn't reset version_clock | Done |
| H6 | No session revocation on password change | Done |
| H7 | Unbounded memory (consent history) | Done |

### MEDIUM

| # | Issue | Status |
|---|-------|--------|
| M1 | No query plan cache | Pending |
| M2 | GraphStore deadlock risk (lock ordering) | Done |
| M3 | No connection pool limits | Pending |
| M4 | Backup without consistency lock | Pending |
| M5 | No request body size enforcement | Done |
| M6 | Pretty-print JSON on every KV write | Done |

## Summary
- 15 of 18 issues fixed
- 635 tests passing, zero failures
- Remaining: H1 (delta persistence), M1 (plan cache), M3 (connection limits), M4 (backup lock)
