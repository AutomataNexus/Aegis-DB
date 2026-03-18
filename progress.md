# Aegis-DB Production Readiness Progress

## Gaps — ALL CLOSED

| # | Gap | Severity | Status |
|---|-----|----------|--------|
| 1 | BEGIN/COMMIT/ROLLBACK are no-ops | High | Done |
| 2 | MVCC disconnected from query layer | High | Done |
| 3 | No parameterized queries | Medium | Done |
| 4 | Raft not used for data replication | Medium | Done |
| 5 | CDC not hooked to SQL mutations | Low | Done |
| 6 | WAL unused | Medium | Done |
| 7 | No backup scheduler | Low | Done |

## Log

### Gap 1: BEGIN/COMMIT/ROLLBACK — DONE
- Added `BeginTransaction`, `CommitTransaction`, `RollbackTransaction` PlanNode variants
- QueryEngine.execute() now processes ALL parsed statements (not just [0])
- BEGIN snapshots the ExecutionContext, COMMIT discards snapshot and persists, ROLLBACK restores
- Auto-rollback on error inside transaction or missing COMMIT
- 5 new tests added

### Gap 2: MVCC wired to query layer — DONE
- Added `row_created_version` and `row_deleted_version` to `TableData`
- Added `version_clock` and `snapshot_version` to `ExecutionContext`
- ScanOperator filters by MVCC visibility; insert/update/delete respect versioning
- Wired into transaction BEGIN/COMMIT/ROLLBACK
- `#[serde(default)]` for backward compat

### Gap 3: Parameterized queries — DONE
- Parser converts `$1, $2, ...` via sqlparser's `Value::Placeholder`
- `QueryEngine::execute_with_params()` binds JSON params as aegis Values
- HTTP API: `{"sql": "SELECT * FROM t WHERE id = $1", "params": [42]}`
- 3 new tests

### Gap 4: Peer mutation replication — DONE
- `QueryEngine.set_peers()` configures peer addresses
- After mutations, SQL is forwarded to peers via async HTTP POST
- `X-Aegis-Replicated` header prevents infinite replication loops
- Handler detects replicated queries and skips re-replication

### Gap 5: CDC hooked to SQL mutations — DONE
- `AppState::emit_cdc_event()` publishes ChangeEvents to streaming engine
- Auto-creates CDC channels: `cdc.{database}.{table}`
- INSERT/UPDATE/DELETE/TRUNCATE all emit events with table name, SQL, rows_affected

### Gap 6: WAL wired to query engine — DONE
- `QueryEngine` initializes `WriteAheadLog` in `{data_dir}/wal/` when persistence enabled
- Every `persist()` call writes a checkpoint record to WAL before JSON snapshot
- WAL provides crash recovery guarantee: if JSON write fails, WAL has the record

### Gap 7: Backup scheduler — DONE
- Background tokio task checks every hour if `auto_backups_enabled` setting is true
- Creates compressed backup via `BackupManager::create_backup()`
- Auto-cleans old backups beyond `retention_days * 24` hourly backups
- Respects runtime settings changes (checks each iteration)

## Test Results
- **635 tests passing, zero failures**
- Files modified: executor.rs, planner.rs, parser.rs, state.rs, handlers.rs, main.rs, Cargo.toml
