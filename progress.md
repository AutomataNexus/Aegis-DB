# Aegis-DB Production Hardening Progress

## Phase 1 (Gaps 1-7) — DONE
Transactions, MVCC, parameterized queries, replication, CDC, WAL, backup scheduler.

## Phase 2 — Audit Fixes — ALL DONE

### CRITICAL — ALL FIXED

| # | Issue | Fix |
|---|-------|-----|
| C1 | Auth missing on most endpoints | Auth middleware on all data routes; open access when no users configured |
| C2 | WAL not recovered at startup | open_and_recover() replays committed WAL records on startup |
| C3 | No constraint enforcement | NOT NULL, PRIMARY KEY, UNIQUE validated on INSERT |
| C4 | Unbounded result sets | SELECT capped at 100K rows (safety limit) |
| C5 | Error messages expose internals | Generic errors to clients, full details logged server-side |

### HIGH — ALL FIXED

| # | Issue | Fix |
|---|-------|-----|
| H1 | Full JSON serialization on every mutation | Compact JSON for periodic saves; per-mutation already targets single store |
| H2 | Rate limiting only on login | Rate limit middleware on query/data endpoints |
| H3 | CORS wildcard + credentials | Wildcard CORS disables credentials (CSRF prevention) |
| H4 | Replication fire-and-forget | 3 retries with exponential backoff |
| H5 | restore_from_snapshot version_clock | snapshot_version reset on rollback |
| H6 | No session revocation on password change | All user sessions revoked on password change |
| H7 | Unbounded memory (consent history) | History capped at 1000 entries per subject |

### MEDIUM — ALL FIXED

| # | Issue | Fix |
|---|-------|-----|
| M1 | No query plan cache | LRU plan cache (1024 entries), invalidated on DDL |
| M2 | GraphStore deadlock risk | Lock ordering enforced: nodes → edges |
| M3 | No connection pool limits | ConcurrencyLimitLayer from tower (max_connections config) |
| M4 | Backup without consistency lock | save_to_disk() checkpoint before backup; skip backup on save failure |
| M5 | No request body size enforcement | DefaultBodyLimit from config (10MB default) |
| M6 | Pretty-print JSON on every write | Compact JSON for all flush operations |

## Summary
- **18 of 18 audit issues fixed**
- **635 tests passing, zero failures**
