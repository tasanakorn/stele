# Local SQLite Backend (v0.16.0+)

## Overview

Starting at v0.16.0, the steop session/project/phase/storage/log surfaces moved from `stele-server` to a per-host SQLite database owned by the `steop` binary. The five surfaces that moved are: session registry and state (`steop_sessions`), session-scoped KV (`steop_storage_session`), project-scoped KV (`steop_storage_project`), and the structured event log (`steop_logs`). The cross-agent surface — `steop_mailbox` and `steop.notify` — remains on `stele-server` unchanged. The primary motivation is collapsing the `HandlePostToolUse` hot path from three sequential HTTP round-trips to a single `BEGIN IMMEDIATE` transaction against the local database, delivering the ≤ 2 ms p50 per-hook budget without a daemon process. Full design rationale is in [PRD-020](../prd/prd-020-steop-local-backend.md).

## Path resolution

The database lives at the first non-empty path in this precedence order:

1. `$STEOP_DB` — used verbatim if set. The parent directory is created with `0700` if it does not exist.
2. `$XDG_DATA_HOME/steop/steop.db` — used if `XDG_DATA_HOME` is set.
3. `$HOME/.local/share/steop/steop.db` — the default on all platforms.

macOS does **not** get an `Application Support` fallback. The `stele` CLI config already follows XDG on macOS (`~/.config/stele/config.toml`), and steop data follows the same rule for consistency. The `steop/` data directory is steop-owned and separate from the `stele/` config directory.

To inspect the resolved path without opening a session:

```bash
steop db path
```

## Pragmas

Every database handle is opened through `internal/store.Open`, which attaches the following pragmas via the DSN so the connection starts correctly without extra round-trips:

| Pragma          | Value    | Rationale                                                                                    |
| --------------- | -------- | -------------------------------------------------------------------------------------------- |
| `journal_mode`  | `WAL`    | Concurrent readers during writes; required under the fresh-process-per-hook model.           |
| `busy_timeout`  | `5000`   | Fresh hook processes contend on file locks; wait up to 5 s before returning `SQLITE_BUSY`.  |
| `synchronous`   | `NORMAL` | WAL-safe and ~2x faster commits than `FULL`. Acceptable crash recovery for hook writes.      |
| `foreign_keys`  | `ON`     | Catches schema bugs early; mirrors stele-server behaviour.                                   |
| `temp_store`    | `MEMORY` | Avoids disk temp files for small ad-hoc sorts.                                               |

## Schema

The local database contains four tables. The minimal shape (primary key only) is:

```sql
-- Session registry, state, and counters.
steop_sessions         PRIMARY KEY (host, project_dir, session_id)

-- Session-scoped KV.
steop_storage_session  PRIMARY KEY (host, project_dir, session_id, key)

-- Project-scoped KV.
steop_storage_project  PRIMARY KEY (host, project_dir, key)

-- Append-only event log.
steop_logs             id INTEGER PRIMARY KEY AUTOINCREMENT
                       -- indexed on (host, project_dir, session_id, id)
```

Full DDL (column types, defaults, indexes) is in [PRD-020 §4.3](../prd/prd-020-steop-local-backend.md).

All four tables use the same three-column identity key `(host, project_dir, session_id)` as the stele-server tables they replaced, so a future cross-host sync PRD can replicate rows without another schema migration.

## Migration framework

Schema versioning is managed by a linear migration registry keyed off SQLite's `PRAGMA user_version`. The registry is a package-level slice in `internal/store/migrations.go`:

```go
// Append-only. Index == target user_version.
var migrations = []func(*sql.Tx) error{
    initialSchema,   // 0 → 1  (v0.16.0 initial tables)
}
```

`store.Open` runs `migrate()` unconditionally on every open:

- **Hot path** (`user_version == len(migrations)`): single `PRAGMA` read, returns immediately — sub-microsecond overhead.
- **First-ever open** (`user_version == 0`): runs `initialSchema` in one transaction and bumps to `1`. One-shot cost of ~5–10 ms.
- **Future schema change**: contributor appends one entry to the slice and ships a new binary. The next open applies the delta in a single transaction.
- **Binary older than DB** (`user_version > len(migrations)`): migration returns `errSchemaNewer`. Hook handlers swallow + return `Allow()` (see Error policy below); CLI subcommands exit 1 with a clear message: `"DB created by a newer steop; upgrade or move $STEOP_DB aside"`.

The registry is **append-only** — entries are never reordered or rewritten. All pending migrations run in a single transaction; a crash mid-upgrade rolls back cleanly and the next open retries from the same `user_version`.

Full design is in [PRD-020 §4.3](../prd/prd-020-steop-local-backend.md).

## Error policy

Hook handlers (`HandlePreToolUse`, `HandlePostToolUse`, `HandleUserPromptSubmit`, `HandlePermissionRequest`, `runStatusline`) must not block Claude Code on local-DB failures. The contract:

- DB open error → log to stderr, return `Allow()` (for hooks) or empty string (for statusline).
- Transaction error → one retry after ~50 ms, then log + allow.
- `SQLITE_BUSY` after 5 s `busy_timeout` → give up immediately, log + allow.
- No panics. No non-zero exit codes from hook paths on DB failure.

CLI subcommands (`steop state get`, `steop storage list`, etc.) exit 1 with a human-readable error on stderr. This matches the existing HTTP-error posture in `client.Client` (swallow, log, allow on hook paths; exit 1 on CLI paths). See [PRD-020 §4.6](../prd/prd-020-steop-local-backend.md).

## Cross-host limitations

Session lookup by UUID is host-local. A session UUID minted on host A cannot be resolved on host B via `steop session get` or `steop state get` — those commands read from the local database only. This is intentional and acceptable: no current skill reads state that was written on a different host, and the composite `(host, project_dir, session_id)` key is structured to support a future cross-host sync PRD without another schema migration. The limitation is documented here; it will be revisited when that PRD is authored. See [PRD-020 §4.8](../prd/prd-020-steop-local-backend.md).

## Install

The `/steop:install` skill builds the binary and then runs `steop db init` to create the database and apply the initial schema before the first hook fires:

```bash
# What /steop:install does after go build:
steop db init
```

To run it manually (e.g. after moving `$STEOP_DB` aside):

```bash
steop db init    # create DB and apply schema
steop db path    # print resolved path (no file created)
```

`steop db init` is idempotent — safe to run on an existing database. It applies any pending migrations but does not drop existing data.

## Recovery

If the database is corrupt or in an inconsistent state, delete it along with its WAL and shared-memory sidecars, then re-run `steop db init`:

```bash
# Locate the DB.
DB=$(steop db path)

# Remove DB and sidecars.
rm -f "$DB" "${DB}-wal" "${DB}-shm"

# Re-create with schema.
steop db init
```

Session state, storage, and logs are rebuilt per-session as hooks fire. Because there is no data migration from stele-server, historical data from v0.15.x is not recoverable this way — only data written after the v0.16.0 upgrade is in the local database.
