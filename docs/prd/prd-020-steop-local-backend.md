# PRD-020 — Steop local SQLite backend for session/project state

- **Status:** Implemented (v0.16.0)
- **Target version:** workspace v0.16.0
- **Scope:** `apps/steop/` (new `internal/store` + `internal/datadir`), `apps/steop/internal/client/*`, `apps/steop/internal/hooks/*`, `apps/steop/cmd_*.go`, `apps/steop/go.mod`, `apps/stele/crates/stele-server/src/steop_api.rs`, `apps/stele/crates/stele-server/src/db.rs` (remove migrated endpoints + tables), `docs/steop/`, `docs/stele/http-api.md`, `docs/README.md`, `scripts/bump-version.py`, lock-step version bumps (`apps/stele/Cargo.toml`, `apps/steop/version.go`, `plugins/stele/.claude-plugin/plugin.json`, `plugins/steop/.claude-plugin/plugin.json`)
- **Author:** Tasanakorn (design) + Claude Code (PRD authoring)

---

## 1. Goals

1. **Move all intra-host steop state to a local SQLite database** at `~/.local/share/steop/steop.db` (override via `STEOP_DB`). The five surfaces that move are: session KV (`steop state`), project KV, phase state (`set-phase` / `clear-phase`), arbitrary key/value storage (`steop storage`), and the event log (`steop.log.*`).
2. **Keep cross-agent traffic on stele unchanged.** Mailbox (`steop.mailbox.*`), `steop.notify`, and the `/steop:st-send` + `/steop:st-watch` wiring that depends on them continue to call `stele-server` over HTTP. Local and remote surfaces coexist in a single `steop` binary.
3. **Collapse the PostToolUse hot path from 3 HTTP RTTs to 1 local transaction.** Today `HandlePostToolUse` emits an `incr` counters call, a `state put`, and a `log.append` — three network round-trips per Claude Code tool invocation. After v0.16.0 all three run inside a single `BEGIN IMMEDIATE` transaction against the local DB. This is the largest single-hook latency win and the primary user-visible justification for the migration.
4. **Meet a ≤ 2 ms p50 per-hook budget** for all non-network hook paths (statusline, PreToolUse, PostToolUse, UserPromptSubmit, PermissionRequest). Achieved via a pure-Go SQLite driver (`modernc.org/sqlite`), WAL mode, DSN-level pragmas, and lazy migration (no per-call schema check).
5. **Be safe under the fresh-process-per-hook model.** Claude Code spawns a new `steop` process for every hook invocation ;  concurrent hooks on the same session must never corrupt the DB, deadlock, or require CGO.
6. **Preserve composite identity semantics.** Rows continue to key on `(host, project_dir, session_id)` so a later cross-host sync PRD (deferred) can replicate without another schema migration.
7. **Clean up `stele-server` of the migrated surfaces.** The `/api/v1/steop/*` REST API is dedicated to steop — there are no other known consumers. Once steop stops calling the migrated methods, stele-server **removes** the matching routes, handlers, and SQLite tables. Stele keeps only the cross-agent surface (`mailbox.*`, `notify`) plus the `steop_mailbox` table. No deprecation shim ;  clean removal in the same release.

## 2. Non-goals

- **Not replacing stele for mailbox or cross-agent messaging.** `steop.mailbox.*`, `steop.notify`, `st-send`, and `st-watch`'s mailbox polling keep their existing HTTP paths. This PRD does not touch them.
- **Not a stylos migration.** The cross-agent redesign on zenoh (PRD-019 and successors) is a separate track ;  v0.16.0 steop still talks REST to `stele-server` for the remote surface.
- **No schema parity with `stele-server`.** Steop owns its own SQLite schema ;  no shared crate, no FK between the two databases, no shared migration tooling.
- **No runtime backend switch.** There is no `STEOP_BACKEND=stele | local` knob. Local is the new default and the only supported mode for the migrated surfaces.
- **No data migration from stele.** Historical session KV, project KV, phase state, storage blobs, and logs living on `stele-server` as of v0.15.x are **not** imported. Fresh start. Before upgrading, users who care about old state must export it manually (e.g. `sqlite3 stele.db .dump steop_sessions`) ;  after the stele-server upgrade those tables are dropped.
- **No read-through or dual-write.** Steop writes local and reads local for the migrated surfaces — full stop.
- **No deprecation shim on stele-server.** The migrated REST endpoints are removed in the same v0.16.0 release, not flagged `410 Gone` for a cycle. The `/api/v1/steop/*` surface has no known external consumers beyond `steop` itself ;  a shim would be dead code from day one.

## 3. Background & Motivation

### 3.1 Current state

Every `steop` invocation today goes over HTTP to `stele-server`:

- `steop state {get,put,incr,reset,delete}` → `POST /api/v1/steop/state.*`
- `steop set-phase` / `clear-phase` → `POST /api/v1/steop/state.put`
- `steop storage {put,get,delete,list}` → `POST /api/v1/steop/storage.*`
- `steop status` → `POST /api/v1/steop/status.get`
- `steop session {start,stop,touch,get,list}` → `POST /api/v1/steop/session.*`
- `steop mailbox {send,recv,ack,watch,update-meta}` → `POST /api/v1/steop/mailbox.*`
- `steop notify` → `POST /api/v1/steop/notify`
- Hook event log → `POST /api/v1/steop/log.append`

All of these hit a single stele-server process. The implementation detail that this PRD exploits: **the first seven surfaces only ever need host-local visibility** (one Claude Code session on one machine writes, and the same host reads back). Only mailbox and notify are genuinely cross-agent.

The problem with routing host-local traffic through an HTTP server is latency on the hook fast path. `internal/hooks/HandlePostToolUse` currently makes three sequential HTTP calls per Claude Code tool invocation (see Research Summary §B):

1. `state.incr` — bump `tool_calls` counter.
2. `state.put` — write updated per-tool state.
3. `log.append` — append an event row.

Even on localhost each RTT is ~1–2 ms, so a single tool call spends 3–6 ms in hook bookkeeping. Claude Code can fire dozens of tools per turn ;  the cumulative cost is visible in interactive feel.

`HandleStop` and `HandleSessionEnd` have the same shape plus a `cleanupWatcherTasks` helper that reads local storage, pushes a mailbox TASK:FAILED, then deletes the storage key — three ops, three RTTs.

### 3.2 Why a local SQLite backend

- **Latency.** Three in-process SQLite statements inside one `BEGIN IMMEDIATE` transaction finish in well under 1 ms on modern SSDs with WAL mode and `synchronous=NORMAL`. That is the smallest unit we can deliver the PostToolUse hook in without a daemon.
- **No daemon.** Steop is a short-lived binary spawned per hook. Running a long-lived local daemon for persistence would add install complexity and a second failure mode. SQLite with WAL + busy_timeout handles the fresh-process-per-hook model natively via POSIX file locks.
- **Composite identity is already row-local.** The three-column key `(host, project_dir, session_id)` maps 1:1 onto SQLite primary keys. The wire format (`host:project_dir[:UUID | :USER]`) stays the same at the command-line layer ;  only the storage backend changes.
- **Pure Go.** `modernc.org/sqlite` gives us WAL + FTS5 + POSIX file locking with no CGO. Cross-compile story stays simple (matches the existing `apps/steop/` build — no `CGO_ENABLED=1` today, no reason to start).

### 3.3 Why keep stele for mailbox/notify

Mailbox and notify are the two surfaces where **the reader is a different process on a different host from the writer**. A local SQLite can't serve that without shipping a replication layer. Stele already has the HTTP server, the SQLite storage, and the `/steop:st-watch` poller ;  there's no win from moving it too. The split we're drawing — host-local state vs cross-agent signal — aligns cleanly with that boundary.

### 3.4 Why a hard cutover (no read-through)

Dual-reading (check local, fall back to stele) doubles the operational surface, adds a second round-trip to every cache miss, and leaks stele dependencies into what should be a fast local path. The migrated surfaces are all session- or project-scoped state that is rebuilt as a Claude Code session runs ;  losing a few weeks of v0.15.x logs is acceptable in exchange for a clean boundary. `cleanupWatcherTasks` already tolerates missing keys, and no skill reads state that was written more than one session ago.

## 4. Design

### 4.1 Storage layout

One SQLite database per host, at:

```
$STEOP_DB                           # env override, if set
~/.local/share/steop/steop.db       # default (XDG_DATA_HOME)
```

Resolution order:

1. `$STEOP_DB` if non-empty → used verbatim. Parent directory is created with `0700`.
2. `$XDG_DATA_HOME/steop/steop.db` if `XDG_DATA_HOME` is set.
3. `$HOME/.local/share/steop/steop.db` otherwise.

macOS **does not** get an `Application Support` fallback. The config path (`~/.config/stele/config.toml`) already follows XDG on macOS ;  data follows the same rule. The `stele/` config dir is shared with the CLI client ;  the `steop/` data dir is steop-owned.

A new helper lives at `apps/steop/internal/datadir/datadir.go` and exposes:

```go
package datadir

// DBPath returns the resolved SQLite path, honoring STEOP_DB and XDG rules.
// Ensures the parent directory exists (0700) but does not open the file.
func DBPath() (string, error)
```

### 4.2 Connection & pragmas

All DB handles are opened through a single `internal/store.Open` that attaches DSN-level pragmas so the session starts correct without extra round-trips:

```
file:<path>?_pragma=journal_mode(WAL)
          &_pragma=busy_timeout(5000)
          &_pragma=synchronous(NORMAL)
          &_pragma=foreign_keys(ON)
          &_pragma=temp_store(MEMORY)
```

Rationale per pragma:

 |  Pragma                    |  Value     |  Why                                                                       | 
 |  ------------------------  |  --------  |  ------------------------------------------------------------------------  | 
 |  `journal_mode`            |  `WAL`     |  Allows concurrent readers during writes ;  required for multi-process safety.  | 
 |  `busy_timeout`            |  `5000`    |  Fresh-process-per-hook model collides on file locks ;  wait up to 5 s before returning `SQLITE_BUSY`. Absent on stele-server (single-process) — **new for steop**.  | 
 |  `synchronous`             |  `NORMAL`  |  WAL-safe ;  ~2× faster commits than `FULL`. Acceptable crash-recovery on hook writes (worst case: last transaction lost on power-cut).  | 
 |  `foreign_keys`            |  `ON`      |  Catches schema bugs early ;  matches stele-server.                          | 
 |  `temp_store`              |  `MEMORY`  |  Avoids disk temp files for small ad-hoc sorts ;  marginal win.              | 

> **Verification task (implementation phase).** Confirm `modernc.org/sqlite` parses the `_pragma=...` DSN keys with the exact syntax above. If upstream expects a different parameter name (e.g. `_pragma=journal_mode=WAL` vs function-call form), the loader falls back to issuing `PRAGMA` statements immediately after `Open` inside a `sync.Once` — still zero per-call overhead, just different wire-up.

`store.Open` is called once per process inside `main.go` (before the command dispatcher runs) and the handle is threaded through a typed `*store.DB` into whatever command needs it. The handle is **not** kept in a package-level singleton ;  each binary invocation opens and defers-closes it explicitly.

### 4.3 Schema & migration framework

Four tables, all keyed on the composite identity columns. Schema is applied through a **linear migration registry** keyed off SQLite's `user_version` pragma, so first-ever open and future schema upgrades use the same code path — no ad-hoc `IF NOT EXISTS` scattered through the codebase, no "what do we do when v0.17 ships" question.

#### Migration registry

A package-level slice in `apps/steop/internal/store/migrations.go`:

```go
// Migrations is append-only. The slice index is the target user_version.
// migrations[0] is the initial schema introduced at v0.16.0.
// Subsequent schema changes append — never reorder, never rewrite.
var migrations = []func(*sql.Tx) error{
    initialSchema,   // user_version 0 → 1
    // (future) e.g. addMailboxIndex,   // 1 → 2
}
```

`store.Open` runs `migrate()` unconditionally on every open:

```go
func migrate(db *sql.DB) error {
    var have int
    db.QueryRow("PRAGMA user_version").Scan(&have)
    if have == len(migrations) {
        return nil                   // hot path: no-op after first open
    }
    if have > len(migrations) {
        return errSchemaNewer        // binary is older than the DB
    }
    tx, _ := db.BeginTx(ctx, &sql.TxOptions{})
    for i := have; i < len(migrations); i++ {
        if err := migrations[i](tx); err != nil {
            tx.Rollback()
            return err
        }
    }
    tx.Exec(fmt.Sprintf("PRAGMA user_version = %d", len(migrations)))
    return tx.Commit()
}
```

Properties:

- **First-ever open:** `user_version = 0`, runs `initialSchema` in one transaction, bumps to `1`. ~5–10 ms one-shot.
- **Hot path:** `user_version == len(migrations)` — single `PRAGMA` read, returns immediately. Sub-microsecond.
- **Future schema change:** contributor appends one entry to the slice and ships a new binary. No separate migration tool, no version-matrix logic.
- **Binary older than DB:** migration errors out with `errSchemaNewer`. Hook handlers swallow + allow (§4.6) ;  CLI commands exit 1 with a clear message ("DB created by a newer steop ;  upgrade or move `$STEOP_DB` aside").
- **Atomicity:** all pending migrations run in a single transaction ;  a crash mid-upgrade rolls back cleanly, and the next open retries from the same `user_version`.

`initialSchema` is the `CREATE TABLE` block below, wrapped in a function that executes each statement on the passed `*sql.Tx`.

#### Table definitions (migration[0])

```sql
-- Session registry + per-session state blob.
CREATE TABLE IF NOT EXISTS steop_sessions (
    host          TEXT    NOT NULL,
    project_dir   TEXT    NOT NULL,
    session_id    TEXT    NOT NULL,           -- UUID or literal 'USER'
    state         TEXT    NOT NULL,           -- 'RUNNING'  |  'STOPPED'  |  ...
    started_at    INTEGER NOT NULL,           -- unix seconds
    last_active_at INTEGER NOT NULL,
    stopped_at    INTEGER,
    data          TEXT    NOT NULL DEFAULT '{}',  -- JSON: phase, mode, step, free-form
    counters      TEXT    NOT NULL DEFAULT '{}',  -- JSON: tool_calls, loop_count, ...
    PRIMARY KEY (host, project_dir, session_id)
) ; 
CREATE INDEX IF NOT EXISTS idx_steop_sessions_project
    ON steop_sessions(host, project_dir) ; 

-- Session-scoped KV (content-addressed by key within session).
CREATE TABLE IF NOT EXISTS steop_storage_session (
    host         TEXT    NOT NULL,
    project_dir  TEXT    NOT NULL,
    session_id   TEXT    NOT NULL,
    key          TEXT    NOT NULL,
    content      TEXT    NOT NULL,
    created_at   INTEGER NOT NULL,
    updated_at   INTEGER NOT NULL,
    PRIMARY KEY (host, project_dir, session_id, key)
) ; 

-- Project-scoped KV (no session column).
CREATE TABLE IF NOT EXISTS steop_storage_project (
    host         TEXT    NOT NULL,
    project_dir  TEXT    NOT NULL,
    key          TEXT    NOT NULL,
    content      TEXT    NOT NULL,
    created_at   INTEGER NOT NULL,
    updated_at   INTEGER NOT NULL,
    PRIMARY KEY (host, project_dir, key)
) ; 

-- Append-only event log.
CREATE TABLE IF NOT EXISTS steop_logs (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    host         TEXT    NOT NULL,
    project_dir  TEXT    NOT NULL,
    session_id   TEXT,                        -- NULL for project-level events
    event        TEXT    NOT NULL,            -- e.g. 'hook.pre_tool', 'phase.set'
    payload      TEXT    NOT NULL DEFAULT '{}', -- JSON
    created_at   INTEGER NOT NULL
) ; 
CREATE INDEX IF NOT EXISTS idx_steop_logs_session
    ON steop_logs(host, project_dir, session_id, id) ; 
CREATE INDEX IF NOT EXISTS idx_steop_logs_project
    ON steop_logs(host, project_dir, id) ; 
```

Schema shape intentionally mirrors the stele-server tables listed in `docs/steop/DESIGN.md` §5 so migration docs can cross-reference without renaming columns. `counters` stays JSON — a child counter table would let us write `UPDATE ... SET v = v + ?` atomically, but the current hook code treats counters as an open-ended map (`tool_calls`, `loop_count`, future names without schema changes) ;  preserving JSON keeps the data model flexible. The RMW cost is absorbed by `BEGIN IMMEDIATE` (see §4.5).

### 4.4 Identity parsing

The composite `id` string (e.g. `host-a:/repos/foo:a1b2c3d4-...`) is parsed at the **API boundary** — i.e. wherever the command dispatcher reads flags or environment. The three-column tuple is then passed into `store.*` as typed arguments:

```go
type Identity struct {
    Host       string
    ProjectDir string
    SessionID  string   // "" for 2-seg project-level, "USER" for user-level, UUID otherwise
}

func ParseID(s string) (Identity, error)
```

Grammar is unchanged from DESIGN.md §4: 2 segments → project-level (`SessionID == ""`), 3 segments → session-level with the third segment either a canonical 8-4-4-4-12 UUID or the literal `USER`. Anything else is a parse error. Storage dispatches on `SessionID == ""` — project ops go to `steop_storage_project` / `steop_sessions` (with a session_id projection) as appropriate.

### 4.5 Hot-path transactions

`HandlePostToolUse` becomes one transaction:

```go
tx, _ := db.BeginImmediate(ctx)
defer tx.Rollback()

// 1. Increment counters (read-modify-write on JSON).
var counters map[string]int64
tx.QueryRow(`SELECT counters FROM steop_sessions WHERE ...`).Scan(&jsonBlob)
json.Unmarshal(jsonBlob, &counters)
counters["tool_calls"]++

// 2. Update data (phase/mode/step) if changed.
tx.Exec(`UPDATE steop_sessions SET data = ?, counters = ?, last_active_at = ? WHERE ...`)

// 3. Append event log row.
tx.Exec(`INSERT INTO steop_logs (...) VALUES (?, ?, ?, ?, ?, ?)`)

tx.Commit()
```

`BEGIN IMMEDIATE` acquires the RESERVED lock at transaction start, so concurrent hook processes queue on the file lock rather than crashing each other with `SQLITE_BUSY` mid-transaction. Under the 5-second busy_timeout, queuing is effectively invisible on the hot path (typical wait << 1 ms ;  pathological case bounded at 5 s, then the hook swallows the error and returns `Allow()` per the fire-and-forget contract).

### 4.6 Hook error policy

Hook handlers (`HandlePreToolUse`, `HandlePostToolUse`, `HandleUserPromptSubmit`, `HandlePermissionRequest`, `runStatusline`) **must not block Claude Code on local-DB failures**. The contract:

- DB open error → log to stderr, return `Allow()` (hooks) or empty (statusline).
- Transaction error → one retry after a ~50 ms sleep, then give up. Log + allow.
- `SQLITE_BUSY` after 5 s busy_timeout → give up immediately. Log + allow.
- No panics, no process-level exit codes on DB failure.

This matches the current HTTP-error posture in `client.Client` (swallow, log, allow).

### 4.7 Split-brain handlers

`HandleStop` and `HandleSessionEnd` need **both** the local `*store.DB` handle AND the stele `*client.Client`:

- Local: mark session stopped, append final log event, run `cleanupWatcherTasks` storage read/delete.
- Stele: send mailbox TASK:FAILED for each orphan watcher task ;  call `steop.notify`.

`client.Client` is refactored so it no longer exposes `StatePut`, `StorageGet`, `StatePut`, `StateIncr`, `LogAppend`, `SessionStart`, `SessionStop`, `SessionTouch`, `SessionList`, `StatusGet`, `StorageList`, `StorageDelete` — those twelve methods move to `*store.DB`. It retains `MailboxSend`, `MailboxRecv`, `MailboxAck`, `MailboxWatch`, `MailboxUpdateMeta`, `Notify`. Call sites are rewritten to hold both handles.

`cleanupWatcherTasks` moves to a helper that takes both: `func cleanupWatcherTasks(ctx, db *store.DB, hc *client.Client, id Identity)`.

### 4.8 `ResolveProjectDir`

Currently uses `client.SessionList` to resolve `project_dir` from a session UUID when `CLAUDE_PROJECT_DIR` is unset. After this PRD, `SessionList` reads from `store.DB` instead. The lookup is host-local — a UUID minted on host A cannot be resolved on host B — which is acceptable because there's no path today that expects cross-host resolution. Documented as a constraint in `docs/steop/local-storage.md`.

### 4.9 Cold-start path

First-ever `steop` invocation on a machine hits the 2 ms budget hardest because:

1. The `steop/` directory doesn't exist.
2. The `steop.db` file doesn't exist.
3. The WAL + shm files are created on first write.
4. modernc.org/sqlite's pure-Go code is ~2–5 ms slower than a CGO build on first JIT/init.

Mitigations, in priority order:

1. **Lazy open.** `runStatusline` and read-only hook paths skip `store.Open` entirely if `STEOP_DB` isn't set AND the default path doesn't exist. They return empty data rather than pay the cold-create cost.
2. **Install-time warm-up.** `/steop:install` runs `steop db init` (new subcommand) after installing the binary. That creates the DB, applies the schema, and primes the page cache before the user's first real hook fires.
3. **Accept one-shot cold miss.** Worst case a single hook invocation overshoots 2 ms on first session ;  all subsequent hooks in the same boot are warm.

### 4.10 Write surface inventory

 |  Surface                          |  Before (HTTP)               |  After (local)                                    | 
 |  -------------------------------  |  --------------------------  |  -----------------------------------------------  | 
 |  `steop state get/put/incr/reset/delete`  |  `state.*` REST      |  `store.SessionState` methods                     | 
 |  `steop set-phase` / `clear-phase`        |  `state.put` REST    |  `store.SessionPhase` methods                     | 
 |  `steop storage put/get/delete/list`      |  `storage.*` REST    |  `store.Storage` methods (session + project)      | 
 |  `steop status`                           |  `status.get` REST   |  `store.SessionStatus`                            | 
 |  `steop session start/stop/touch/get/list`  |  `session.*` REST  |  `store.Sessions`                                 | 
 |  `steop.log.append` / `log.query` (internal)  |  `log.*` REST    |  `store.Logs`                                     | 
 |  `steop mailbox *`                        |  `mailbox.*` REST    |  **unchanged** — `client.Client`                  | 
 |  `steop notify`                           |  `notify` REST       |  **unchanged** — `client.Client`                  | 

### 4.11 Stele-server cleanup

The `/api/v1/steop/*` surface on `stele-server` is dedicated to steop ;  once steop migrates, the endpoints have no remaining consumer. They are removed cleanly in this same release.

**Routes removed from `apps/stele/crates/stele-server/src/steop_api.rs`:**

```
steop.session.start    steop.session.stop      steop.session.touch
steop.session.get      steop.session.list      steop.project.list
steop.state.get        steop.state.put         steop.state.incr
steop.state.reset      steop.state.delete      steop.status.get
steop.storage.put      steop.storage.get       steop.storage.delete
steop.storage.list     steop.log.append        steop.log.query
```

**Routes retained:**

```
steop.mailbox.send     steop.mailbox.list      steop.mailbox.get
steop.mailbox.read     steop.mailbox.archive   steop.mailbox.update_meta
steop.notify
```

**Handler functions removed from `apps/stele/crates/stele-server/src/db.rs`:** `steop_session_{start,stop,touch,get,list}`, `steop_project_list`, `steop_state_{put,incr,reset,delete}`, `steop_status_get`, `steop_storage_session_{put,get,delete,list}`, `steop_storage_project_{put,get,delete,list}`, `steop_log_{append,query}`, plus the `steop_ensure_session` / `steop_row_to_session` helpers if no longer referenced after the removals. `steop_compose_session_id` / `steop_compose_project_id` / `steop_now` / `steop_parse_json` / `steop_json_merge` stay — mailbox uses them.

**Schema changes in `ensure_steop_schema()`:**

1. `CREATE TABLE` statements for `steop_sessions`, `steop_storage_session`, `steop_storage_project`, `steop_logs` are removed from the function — fresh installs never create them.
2. The function additionally runs `DROP TABLE IF EXISTS steop_sessions`, `DROP TABLE IF EXISTS steop_storage_session`, `DROP TABLE IF EXISTS steop_storage_project`, `DROP TABLE IF EXISTS steop_logs` on startup. Existing installs drop the tables on the first v0.16.0 boot ;  idempotent on repeated starts.
3. `steop_mailbox` table + triggers stay untouched.

The drop is unconditional and destructive — that's the point of "no data migration" in §2. Anyone who wants a backup must take it before upgrading. Release notes call this out explicitly.

**Doc removals:** `docs/stele/http-api.md`'s steop RPC section is trimmed to mailbox + notify only. `docs/steop/DESIGN.md` §5 is rewritten to document the post-migration split (local tables owned by steop ;  `steop_mailbox` still on stele). `docs/steop/smoke-tests.md` drops the curl sequences for the removed methods.

## 5. Changes by Component

 |  Component                                       |  Change                                                                                                                                            |  Files                                                                                                      | 
 |  ----------------------------------------------  |  ------------------------------------------------------------------------------------------------------------------------------------------------  |  ---------------------------------------------------------------------------------------------------------  | 
 |  `apps/steop/internal/store/` (new)              |  New package: DB open with DSN pragmas, linear migration registry keyed off `user_version`, typed access methods for sessions/state/storage/logs, `BEGIN IMMEDIATE` helper, lazy-open shim.  |  `apps/steop/internal/store/store.go`, `migrations.go`, `sessions.go`, `state.go`, `storage.go`, `logs.go`, `tx.go`  | 
 |  `apps/steop/internal/datadir/` (new)            |  XDG-aware path resolver ;  creates `~/.local/share/steop/` with `0700`.                                                                              |  `apps/steop/internal/datadir/datadir.go`                                                                   | 
 |  `apps/steop/go.mod`                             |  Add `modernc.org/sqlite` dependency. No other additions.                                                                                           |  `apps/steop/go.mod`, `apps/steop/go.sum`                                                                   | 
 |  `apps/steop/internal/client/client.go`          |  Remove 12 HTTP methods for the migrated surfaces ;  keep 6 for mailbox/notify. Rename the type to reflect the narrowed scope (still `client.Client`).  |  `apps/steop/internal/client/client.go`                                                                     | 
 |  `apps/steop/cmd_state.go`                       |  `state get/put/incr/reset/delete`, `set-phase`, `clear-phase` dispatch through `*store.DB`.                                                        |  `apps/steop/cmd_state.go`                                                                                  | 
 |  `apps/steop/cmd_storage.go`                     |  `storage put/get/delete/list` dispatch through `*store.DB`.                                                                                        |  `apps/steop/cmd_storage.go`                                                                                | 
 |  `apps/steop/cmd_session.go`                     |  `session start/stop/touch/get/list` dispatch through `*store.DB`.                                                                                  |  `apps/steop/cmd_session.go`                                                                                | 
 |  `apps/steop/cmd_status.go`                      |  `status` dispatches through `*store.DB`.                                                                                                           |  `apps/steop/cmd_status.go`                                                                                 | 
 |  `apps/steop/cmd_db.go` (new)                    |  `steop db init` subcommand for install-time warm-up ;  optional `steop db path` for debugging.                                                       |  `apps/steop/cmd_db.go`                                                                                     | 
 |  `apps/steop/internal/hooks/*.go`                |  `HandlePreToolUse`, `HandlePostToolUse`, `HandleUserPromptSubmit`, `HandlePermissionRequest`, `HandleStop`, `HandleSessionStart`, `HandleSessionEnd`, `runStatusline` take a `*store.DB` alongside the existing `*client.Client`. `HandlePostToolUse` collapses 3 HTTP calls to 1 transaction.  |  `apps/steop/internal/hooks/pre_tool.go`, `post_tool.go`, `user_prompt.go`, `permission.go`, `stop.go`, `session_start.go`, `session_end.go`, `statusline.go`  | 
 |  `apps/steop/internal/hooks/watcher_cleanup.go`  |  `cleanupWatcherTasks` takes both handles ;  reads local storage, sends stele mailbox TASK:FAILED, deletes local storage.                             |  `apps/steop/internal/hooks/watcher_cleanup.go`                                                             | 
 |  `apps/steop/main.go`                            |  Open `*store.DB` once at startup ;  pass it into command dispatcher alongside `*client.Client`.                                                      |  `apps/steop/main.go`                                                                                       | 
 |  `apps/steop/version.go`                         |  Lock-step bump to `0.16.0`.                                                                                                                        |  `apps/steop/version.go`                                                                                    | 
 |  `apps/stele/crates/stele-server/src/steop_api.rs`  |  Delete 18 routes for session/project/state/status/storage/log surfaces. Retain 7 routes for mailbox + notify. Delete matching axum handler functions in-file.  |  `apps/stele/crates/stele-server/src/steop_api.rs`                                                          | 
 |  `apps/stele/crates/stele-server/src/db.rs`     |  Delete ~24 `steop_*` handler functions (session/state/status/storage/log). Trim `ensure_steop_schema` to mailbox-only. Add `DROP TABLE IF EXISTS` statements for the four removed tables so existing installs clean up on first boot.  |  `apps/stele/crates/stele-server/src/db.rs`                                                                 | 
 |  `docs/stele/http-api.md`                        |  Trim the steop RPC section to mailbox + notify. Add a short note that session/state/storage/log RPCs moved to a steop-local SQLite per PRD-020.     |  `docs/stele/http-api.md`                                                                                   | 
 |  `apps/stele/Cargo.toml`                         |  Workspace version `0.15.0` → `0.16.0`.                                                                                                             |  `apps/stele/Cargo.toml`                                                                                    | 
 |  `plugins/stele/.claude-plugin/plugin.json`      |  Lock-step version bump to `0.16.0`.                                                                                                                |  `plugins/stele/.claude-plugin/plugin.json`                                                                 | 
 |  `plugins/steop/.claude-plugin/plugin.json`      |  Lock-step version bump to `0.16.0`.                                                                                                                |  `plugins/steop/.claude-plugin/plugin.json`                                                                 | 
 |  `plugins/steop/skills/install/SKILL.md`         |  Add step: after `go build`, invoke `steop db init` to pre-create the DB and run schema.                                                            |  `plugins/steop/skills/install/SKILL.md`                                                                    | 
 |  `scripts/bump-version.py`                       |  No shape change ;  regular `0.16.0` bump covers the default component set (workspace + both plugins + steop version.go).                             |  `scripts/bump-version.py`                                                                                  | 
 |  `docs/steop/local-storage.md` (new)             |  Document the local backend: path resolution, schema, pragmas, error policy, migration stance, cross-host limitations.                              |  `docs/steop/local-storage.md`                                                                              | 
 |  `docs/steop/DESIGN.md`                          |  Mark §5 tables as "moved local (v0.16.0)" for sessions/storage/logs ;  keep mailbox table as stele-backed. Point at `local-storage.md`.              |  `docs/steop/DESIGN.md`                                                                                     | 
 |  `docs/steop/smoke-tests.md`                     |  Flag which curl sequences exercise stele-backed endpoints (mailbox/notify) vs which surfaces are now local-only (session/state/storage/logs). Add a parallel Go smoke command list for the local surfaces.  |  `docs/steop/smoke-tests.md`                                                                                | 
 |  `docs/README.md`                                |  Add PRD-020 row to the PRD table. Add `local-storage.md` row to the Steop docs table.                                                              |  `docs/README.md`                                                                                           | 

No changes to `plugins/stele/skills/**`, `apps/stylos/**`, or the other steop skills. The `stele-cli` crate is not touched (it never called the migrated endpoints).

## 6. Edge Cases

 |  Scenario                                                                                        |  Behavior                                                                                                                                                                  | 
 |  ----------------------------------------------------------------------------------------------  |  ------------------------------------------------------------------------------------------------------------------------------------------------------------------------  | 
 |  First-ever DB open on a fresh host                                                              |  `store.Open` creates `~/.local/share/steop/` at `0700`, creates `steop.db`, applies schema, sets `user_version`. One-shot cost ~5–10 ms on pure-Go modernc ;  subsequent opens < 1 ms. Install skill pre-warms via `steop db init`.  | 
 |  `STEOP_DB` points at a non-writable path (read-only FS, wrong permissions)                      |  `store.Open` returns an error. Hook handlers log to stderr and return `Allow()` (fire-and-forget). CLI commands (`steop state get`) print the error to stderr and exit 1.  | 
 |  DB file corruption (torn WAL, disk bit-flip)                                                    |  `PRAGMA integrity_check` is **not** run on open (too expensive). First corrupted statement returns `SQLITE_CORRUPT` ;  hooks log + allow, CLI exits 1. Recovery: user deletes `steop.db*` and `steop db init`s. Documented in `local-storage.md`.                                     | 
 |  Two concurrent hooks on the same session (PostToolUse + PreToolUse racing)                      |  `BEGIN IMMEDIATE` serializes writes via the RESERVED lock. `busy_timeout=5000` absorbs contention. WAL keeps readers non-blocking.                                         | 
 |  Two steop binaries of different versions running on one host (upgrade in progress)              |  Both honor `user_version`. Old → new is the common case: new binary runs any pending migrations atomically on first open, then both agree. New → old (downgrade scenario: `user_version` on disk > `len(migrations)` in binary) returns `errSchemaNewer` from `store.Open` — hooks log + allow, CLI exits 1 with "DB created by a newer steop ;  upgrade or move `$STEOP_DB` aside". No silent data corruption.  | 
 |  SIGKILL mid-transaction                                                                         |  WAL replay on next open recovers committed state ;  the in-flight transaction is discarded. No action required.                                                              | 
 |  `HandlePostToolUse` takes > 5 s on the busy_timeout                                             |  One retry, then log + allow. Claude Code never blocks on steop bookkeeping.                                                                                                | 
 |  modernc.org/sqlite DSN pragma syntax differs from what we assumed                               |  Loader falls back to issuing `PRAGMA` statements post-open inside `sync.Once`. Flagged in §4.2 as a verification task for implementation.                                   | 
 |  Hook fires with no `CLAUDE_PROJECT_DIR` and no session list entry                               |  `ResolveProjectDir` falls back to cwd (current behavior). Local DB handles unknown sessions the same way stele-server does — a fresh `steop_sessions` row is created on first write.  | 
 |  User deletes `~/.local/share/steop/steop.db` mid-session                                        |  Next write recreates the file + schema. Prior session state is lost. Matches current stele behavior if someone `rm`'d the stele DB.                                        | 
 |  User runs `steop state get` on a project that has never written state                           |  Returns empty (not an error). Matches current REST behavior.                                                                                                               | 
 |  stele-server is offline                                                                         |  Local surfaces work fine (no dependency). Mailbox/notify still fail, as today ;  `HandleStop`'s cleanup partially succeeds (local cleanup runs ;  mailbox TASK:FAILED fails and is logged).  | 
 |  Watcher task cleanup encounters a local storage key but the stele mailbox write fails           |  Local delete is deferred until mailbox write succeeds ;  on failure, the key is left in place and retried on the next `Stop`. `cleanupWatcherTasks` is idempotent.           | 
 |  User writes state via an old v0.15.x binary after upgrading                                     |  Old binary writes to `stele-server`, new binary reads from local — state appears missing. Documented in §7 as "do not cross-version session writes".                       | 
 |  Old `steop` binary (v0.15.x) calls new `stele-server` (v0.16.0) on a migrated endpoint          |  Stele returns `404 Not Found` (route no longer registered). The old client's HTTP wrapper logs + continues (hook contract is fire-and-forget). User sees stale state until they upgrade steop. Release notes: **upgrade stele and steop in lock-step**.  | 
 |  Existing v0.15.x `steop_sessions` / `steop_storage_*` / `steop_logs` rows on stele at upgrade   |  `ensure_steop_schema` runs `DROP TABLE IF EXISTS` on first boot of stele v0.16.0 — rows are gone. No rollback. Users who need them must export before upgrade.              | 

## 7. Migration

- **Hard cutover at v0.16.0.** Steop v0.16.0 never reads `stele-server` for session KV, project KV, phase, storage, or logs. On first launch it creates `~/.local/share/steop/steop.db` fresh. No import from `stele-server`.
- **Stele-server removes the migrated endpoints in the same release.** The 18 REST routes listed in §4.11 disappear from the axum router ;  the 4 SQLite tables are dropped via `DROP TABLE IF EXISTS` on first v0.16.0 boot. No deprecation window, no `410 Gone` shim. Any caller still hitting those paths after the stele upgrade gets a `404`. The cross-agent surface (`mailbox.*`, `notify`, `steop_mailbox` table) is untouched.
- **Pre-v0.16.0 state on stele-server is destroyed on upgrade.** Users who want to inspect or export old session KV, project KV, storage blobs, or logs must run `sqlite3 $STELE_DB .dump steop_sessions steop_storage_session steop_storage_project steop_logs > backup.sql` **before** upgrading stele. After the stele-server upgrade, those tables are gone. steop provides no migration tooling ;  this is a clean break.
- **Mailbox + notify are unchanged.** `/steop:st-watch`, `/steop:st-send`, and any skill that depends on cross-agent delivery continues to work without modification.
- **Cross-version usage within one session is unsupported.** If a user upgrades mid-session (old hook fires before upgrade, new hook fires after), the old state lives on stele-server (or is gone if stele is also upgraded) and the new state lives locally — they will not see each other. Documented in release notes.
- **Upgrade order is stele first, then steop (or lock-step).** An old steop binary hitting a new stele gets `404` on migrated methods and continues gracefully (fire-and-forget hooks). A new steop against an old stele works by accident (it never calls the removed endpoints). The hazardous path is leaving old steop installed long after stele upgrades — hooks will silently lose state.
- **Lock-step version bump.** `python scripts/bump-version.py 0.16.0` moves the workspace + both plugins + `steop/version.go` in one commit. No stylos bump (follows its own cadence).
- **Install-time step.** `/steop:install` skill gains a `steop db init` call after `go build` so the DB exists before the first hook fires. Existing installs re-run the skill on upgrade ;  users who don't run the skill hit the lazy-open path on the first real hook (one-shot cold miss).

## 8. Testing

No automated test harness exists in the repo today. This PRD adds the first Go unit + integration test files for `apps/steop/` since the binary was introduced.

1. **Unit: `internal/store` CRUD.** For each table (`sessions`, `storage_session`, `storage_project`, `logs`): put, get, list, delete, update round-trips against an in-memory SQLite (`:memory:`). Verify JSON round-trip for `data` and `counters`. File: `apps/steop/internal/store/store_test.go`.
2. **Unit: pragma verification.** Open a temp DB via `store.Open`, then `PRAGMA journal_mode`, `PRAGMA busy_timeout`, `PRAGMA synchronous`, `PRAGMA foreign_keys` — assert the expected values. If modernc.org/sqlite DSN parsing drifts from the assumption in §4.2, this test catches it.
2a. **Unit: migration framework.** (a) Fresh DB: `store.Open` yields `PRAGMA user_version = 1` and all four tables/indexes exist. (b) Idempotent re-open: second `store.Open` on the same file is a no-op (verify via a query counter or a `sync.Once` flag stub). (c) Simulated future migration: append a no-op `migrations[1] = addSentinelRow` to the slice ;  re-open and verify the sentinel lands and `user_version = 2`. (d) Downgrade guard: set `PRAGMA user_version = 99` manually, then open with the current binary → expect `errSchemaNewer`, not a panic or silent data loss.
3. **Unit: identity parsing.** `ParseID` accepts 2-seg and 3-seg (UUID + literal `USER`), rejects everything else. Mirrors DESIGN.md §4 grammar.
4. **Integration: concurrent hook simulation.** Spawn N=32 goroutines each running the PostToolUse transaction (`incr` + `state.put` + `log.append`) against one DB file. Assert: no `SQLITE_CORRUPT`, no deadlocks, `tool_calls` counter lands at exactly 32, `log` table has exactly 32 rows. Runs against a real on-disk DB (not `:memory:`) so file-lock semantics are exercised. File: `apps/steop/internal/store/concurrency_test.go`.
5. **Integration: fresh-process concurrency.** Shell test that `exec`s `steop state incr tool_calls` N=16 times in parallel (`xargs -P 16`). Assert the final counter is 16. Lives under `apps/steop/scripts/test-concurrency.sh`.
6. **Smoke parity.** Port `docs/steop/smoke-tests.md`'s curl sequences for the migrated surfaces into `apps/steop/scripts/smoke-local.sh` — same inputs and expected outputs, but invoking `steop` CLI against the local DB instead of curl against stele-server. Mailbox/notify curl sequences stay unchanged.
7. **Hook-budget micro-benchmark.** `go test -bench BenchmarkPostToolUse` in `apps/steop/internal/hooks/post_tool_test.go`: measure cold (fresh DB, empty page cache) and warm (DB pre-touched) latency. Assert warm p50 ≤ 2 ms on the dev machine. Not a CI gate (timing-sensitive) — operator-run during review.
8. **Cold-statusline check.** `go test -run TestStatuslineColdOpen` opens a fresh DB and runs `runStatusline` — verifies the lazy-open path in §4.9 returns empty without blocking when the DB file doesn't exist. Also verifies that when the DB does exist, statusline reads complete in < 2 ms warm.
9. **Manual interop test.** Install v0.16.0 on two terminals of the same host ;  run `/steop:st-send` from one to the other ;  verify cross-agent delivery (stele path) still works. Verify each session's `steop state` is independent (local path).
10. **Stele cleanup assertions.** After building `stele-server` v0.16.0: (a) `curl -sf -X POST http://localhost:3100/api/v1/steop/state.get -d '{"id":"h:p:u"}'` returns `404` ;  mailbox + notify endpoints still return `200`/valid. (b) On a v0.15.x stele DB upgraded in place, `sqlite3 stele.db ".tables"` no longer lists `steop_sessions`, `steop_storage_session`, `steop_storage_project`, `steop_logs` ;  still lists `steop_mailbox`. (c) Restart stele-server twice — `DROP TABLE IF EXISTS` is idempotent, no error on the second boot.

## 9. Open Questions

1. **modernc.org/sqlite pragma DSN syntax.** Does the driver accept `?_pragma=journal_mode(WAL)&_pragma=busy_timeout(5000)` in the exact form documented? Resolve in Pass A by writing a 10-line probe binary before the full `store` package lands. Fallback (`PRAGMA` statement after `Open` inside `sync.Once`) is prepared.
2. **Cold-start budget on first-ever install.** Is modernc's one-shot init cost ~5 ms or ~20 ms on a typical laptop SSD? If it's the high end, install-time warmup (`steop db init`) becomes load-bearing rather than a nice-to-have. Measure during Pass A ;  if unacceptable, consider eager-compile via `go build -pgo` or a CGO fallback build target.
3. **Counter schema shape.** Keep `counters` as JSON (this PRD's choice), or split into a `steop_counters(host, project_dir, session_id, name, value)` child table with `UPDATE SET value = value + ?`? Child table wins on atomicity but locks the counter set at schema-design time. Decision locked: JSON at v0.16.0 because the hook code treats counter names as open-ended ;  revisit only if JSON RMW becomes a measurable bottleneck.
4. ~~**`user_version` future migrations.**~~ **Resolved in §4.3.** A linear `migrations []func(*sql.Tx) error` registry is introduced at v0.16.0 with `migrations[0] = initialSchema`. Future schema changes append one entry ;  no separate PRD or tooling needed.
5. **Binary size impact.** modernc.org/sqlite adds ~8 MB to the `steop` binary. Acceptable for a developer tool ;  measure in Pass A and document in release notes if significantly larger than expected.
6. **Observability.** Should `steop db stats` print row counts / DB size / WAL size for debugging? Propose yes, as a trivial addition to `cmd_db.go`. Not load-bearing for v0.16.0 ;  can defer to v0.16.1.

## 10. Implementation Checklist

- [ ] **Pass A — Storage foundation.** `internal/datadir`, `internal/store` (Open + migration registry with `migrations[0] = initialSchema` + tx helpers), `cmd_db.go` with `db init` / `db path`. DSN pragma probe (Open Q §9.1). modernc.org/sqlite dep added. Unit tests: migration framework (fresh/idempotent/forward/downgrade-guard) + CRUD + pragma verification + concurrent goroutine stress.
- [ ] **Pass B — Client split.** Remove migrated methods from `client.Client`. Move `SessionList`, `StatePut`, etc. to `*store.DB` methods with identical signatures where possible. Compiler-driven refactor of call sites.
- [ ] **Pass C — Hook rewire.** PostToolUse collapsed to one transaction ;  PreToolUse/UserPromptSubmit/PermissionRequest/Stop/SessionStart/SessionEnd wired to both handles. Statusline uses lazy-open. `cleanupWatcherTasks` takes both handles.
- [ ] **Pass D — CLI + skill.** `set-phase` / `clear-phase` / `state *` / `storage *` / `session *` / `status` commands route through `*store.DB`. `/steop:install` skill calls `steop db init` post-build.
- [ ] **Pass E — Docs + version bump.** `docs/steop/local-storage.md` written ;  DESIGN.md annotated ;  smoke-tests.md split by surface ;  `docs/stele/http-api.md` trimmed to mailbox + notify ;  `docs/README.md` updated. `python scripts/bump-version.py 0.16.0`.
- [ ] **Pass F — Stele cleanup.** Remove 18 routes from `steop_api.rs` and the matching axum handlers. Remove ~24 `steop_*` functions from `db.rs`. Trim `ensure_steop_schema` to mailbox-only and add `DROP TABLE IF EXISTS` for the four removed tables. `cargo build -p stele-server` passes ;  `cargo clippy` clean. Manual: run stele-server, confirm removed endpoints return 404 and `.tables` no longer lists the dropped tables.
- [ ] **Pass G — Smoke + bench.** Run `smoke-local.sh`, `test-concurrency.sh`, `BenchmarkPostToolUse`, `TestStatuslineColdOpen`, and the stele cleanup assertions from §8.10. Verify on macOS (APFS) and Linux (ext4). Update release notes with measured numbers and the hard-break removal warning.
