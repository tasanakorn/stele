# Steop Design (v3, 0.16.1)

## 1. Purpose

Steop is a workflow pipeline harness for Claude Code. It hooks into Claude Code lifecycle events (SessionStart, PostToolUse, Stop, etc.) to:

1. Track session state — current phase, mode, step, counters.
2. Surface a compact status line in the Claude Code terminal.
3. Archive session summaries to a shared mailbox for `/stele:sync` and retrospective tooling.
4. Persist arbitrary key-value storage for skills and hooks to share across tool calls.

Steop is deliberately thin on the hook side and fat on the read side: hooks fire-and-forget; skills read state to resume mid-session.

## 2. Non-goals

- Not a general-purpose logging service. Use the stele memory layer for structured knowledge.
- Not a real-time event bus. The mailbox and log are polled, not pushed.
- Not a process supervisor. Steop tracks Claude Code sessions, not shell processes.
- Not a multi-tenant system. A single stele server typically serves a team on a local network or a single developer on localhost. Cross-machine visibility is by design.

## 3. Architecture

Steop has three layers:

1. **Go binary (`steop`)** — installed to `~/.local/bin/steop`. Dispatches hooks, evaluates PreToolUse safety rules, maintains the current-session sentinel file. Reads config from `~/.config/stele/config.toml`. Compiled from `apps/steop/`.

2. **Claude Code hooks** — registered in `plugins/steop/hooks/hooks.json`. On every Claude Code lifecycle event, the hook shell invokes `steop hook <event>` with JSON on stdin.

3. **Local SQLite database** — session/project/phase/storage/log state lives in `~/.local/share/steop/steop.db` (override via `$STEOP_DB`). Every hook reads and writes this database directly via `internal/store`. See [local-storage.md](local-storage.md) for path resolution, schema, pragmas, and error policy.

4. **Stele server API** (`/api/v1/steop/*`) — cross-agent surface only. Retained methods: `steop.mailbox.*` and `steop.notify`. The `steop_mailbox` table lives on `stele-server` alongside the existing `memories`, `entities`, `relations` tables. Session, state, storage, and log surfaces moved to the local database at v0.16.0 per [PRD-020](../prd/prd-020-steop-local-backend.md).

```
Claude Code lifecycle
       │
       ▼
steop hook <event>  ←── hooks.json
       │
       ├─── local DB ops ──────────────────────────────────────────────────────────┐
       │    (session / state / storage / logs)                                     │
       │                                                                           ▼
       │                                                    ~/.local/share/steop/steop.db
       │
       └─── HTTP POST /api/v1/steop/<method> ─────────────────────────────────────┐
            (mailbox.* / notify only)                                              │
                                                                                   ▼
                                                                         stele-server process
                                                                                   │
                                                                                   ▼
                                                                        SQLite (steop_mailbox)
```

## 4. Identity model

Steop addresses every resource with an **SSH/SCP-style composite identifier** encoded as a single colon-separated string. There are no implicit defaults and no header-based identity — v0.7 is body-only and uses a single `id` field per call.

### Identifier grammar

```
project_id  = host ":" project_dir
session_id  = host ":" project_dir ":" uuid
user_id     = host ":" project_dir ":" "USER"
```

Examples:

- `vm-02:/home/tas/stele`                                     — project id
- `vm-02:/home/tas/stele:a1b2c3d4-5678-4abc-9def-0123456789ab` — session id
- `laptop:/Users/tas/work:9f8e7d6c-5b4a-4321-8765-abcdef012345` — session id on another host
- `laptop:/Users/tas/work:USER`                               — user id (singleton per host:project_dir)

`host` is the machine name (e.g. `os.Hostname()` in Go, `gethostname()` in Rust), with `:` characters stripped at the client so it is safe as a segment. `project_dir` is an absolute path. The session segment is always a canonical Claude Code UUID in 8-4-4-4-12 form. `USER` is the literal four-character string (uppercase ASCII) — it is a singleton per `host:project_dir`, not a named user.

### Parsing

The server splits the composite id deterministically:

1. The first `:` splits `host` from the remainder.
2. If the remainder has no further `:`, the id is project-level; `project_dir` = remainder. END.
3. Find the **last** `:` in the remainder. Let `tail` be everything after it.
4. If `tail` matches the UUID regex (`^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$`), the id is session-level; `project_dir` = substring before that last `:`. END.
5. If `tail` == `"USER"` (exact, case-sensitive), the id is user-level; `project_dir` = substring before that last `:`. END.
6. Otherwise → **400** `"id 3rd segment must be a session UUID or the literal 'USER'"`.

This is a **tightening vs v0.7**: v0.7 silently accepted any non-UUID 3rd segment as a project-level path extension. v0.8 closes the 3rd-segment set to `{UUID, USER}` only. See `docs/prd/prd-001-mailbox-v2.md` §5 for the normative parser spec and error message catalogue.

This lets `project_dir` safely contain `:` characters (e.g. Windows-style) as long as no project path ends with a literal 36-char UUID or the four-char string `USER`.

### Arity dispatch

Storage methods (`storage.put`/`get`/`delete`/`list`) accept either arity of id. As of v0.16.0 this dispatch happens **inside the `steop` binary** (`apps/steop/internal/store`) against the local SQLite DB — `stele-server` no longer has the `steop_storage_*` tables. A 2-segment id routes to the project KV table; a 3-segment id routes to the session KV table. See [local-storage.md](local-storage.md) for schema. Every other id-bearing method requires the full 3-segment form; an incomplete id returns `HTTP 400 {"error":"id must be 3-segment (host:project_dir:session_uuid)"}` (for the remaining server-side methods) or the equivalent local error.

### No server-side validation beyond parsing

The server does not validate that `host` looks like a hostname, that `project_dir` is absolute, or that the UUID refers to a real Claude Code session. It only enforces the segment grammar above. Clients are responsible for completeness and semantic consistency across related calls.

### No headers

v0.5 used `X-Steop-Host` and `X-Steop-Project-Dir` headers as an implicit identity channel. **v0.6 ignored these headers and used structured `{host, project_dir, session_id}` triples.** v0.7 collapses the triple into a single `id` string in every request body. All identity is explicit in the request body.

## 5. Persistence model

As of v0.16.0, steop uses a **split persistence model**: host-local state lives in a local SQLite database (`~/.local/share/steop/steop.db`); cross-agent messaging lives on `stele-server`. The `steop` binary holds both a `*store.DB` handle (local) and a `*client.Client` handle (stele HTTP) and routes each operation to the correct backend.

> **Moved local (v0.16.0):** `steop_sessions`, `steop_storage_session`, `steop_storage_project`, `steop_logs` — see [local-storage.md](local-storage.md).

The local tables are no longer created by `ensure_steop_schema()` on stele-server. Fresh `stele-server` installs never create them; existing installs drop them on first v0.16.0 boot (`DROP TABLE IF EXISTS` runs unconditionally in `ensure_steop_schema()`). Full DDL for all four local tables is in [PRD-020 §4.3](../prd/prd-020-steop-local-backend.md).

The only `steop_*` table that remains on stele-server is `steop_mailbox`, documented below.

### 5.1 `steop_mailbox` — inter-session messaging (stele-server)

Rewritten in v0.8.0 (drop-and-recreate; v0.7 rows are not preserved — see `docs/prd/prd-001-mailbox-v2.md` §9.1). Messages may flow between any combination of principals.

#### Schema

```sql
CREATE TABLE IF NOT EXISTS steop_mailbox (
    message_id   INTEGER PRIMARY KEY AUTOINCREMENT,
    from_id      TEXT NOT NULL,
    to_id        TEXT NOT NULL,
    subject      TEXT NOT NULL DEFAULT '',
    message_type TEXT NOT NULL DEFAULT 'NOTE',
    meta         TEXT NOT NULL DEFAULT '{}',   -- JSON: server-queryable metadata
    payload      TEXT NOT NULL DEFAULT '{}',   -- JSON: opaque application data
    created_at   TEXT NOT NULL,
    status       TEXT NOT NULL DEFAULT 'NEW'   -- 'NEW' | 'READ' | 'ARCHIVE'
);
CREATE INDEX IF NOT EXISTS idx_steop_mailbox_recipient
    ON steop_mailbox(to_id, status, created_at);
CREATE INDEX IF NOT EXISTS idx_steop_mailbox_sender
    ON steop_mailbox(from_id, created_at);
```

SQL columns use `from_id`/`to_id`; the wire format exposes them as `from`/`to` (the `_id` suffix is dropped at the HTTP boundary to avoid collision with the composite-string `id` field present on every steop RPC request).

#### Addressing rules

`from_id` and `to_id` are composite identifiers in any of the three forms: project (`host:project_dir`), session (`host:project_dir:uuid`), or user (`host:project_dir:USER`). The sender and recipient may each be any principal — project-level senders, session-level senders, and user-principal senders are all valid.

#### Implicit `from` derivation

When `mailbox.send` omits the `from` field, the server derives it from the mandatory `id` field of the request body. Explicit `from` in the body overrides the implicit value. This means hooks can send mail without constructing a `from` string by hand — the session's own `id` is enough.

#### Status lifecycle

```
         send                 mailbox.read                 mailbox.archive
(none) ──────► NEW ──────────────────────► READ ─────────────────────────► ARCHIVE
                │                                                                ▲
                └────────────── mailbox.archive ────────────────────────────────┘
```

Legal transitions: `NEW → READ`, `NEW → ARCHIVE`, `READ → ARCHIVE`. Illegal transitions return 409. `mailbox.list` does **not** flip status — it is side-effect free. `mailbox.get` is likewise side-effect free.

#### `meta` vs `payload`

- **`meta`** — server-queryable structured metadata. JSON object, default `{}`. Use for fields the server or other callers might filter on (priority, tags, correlation IDs).
- **`payload`** — opaque application data. JSON value (object, array, or scalar), default `{}`. Only meaningful to the final consumer.

#### `message_type` vocabulary

Unchanged from v0.7 `kind` (renamed for clarity). Reserved namespaces:

- `HOOK:*` — reserved but unused as of v0.16.1 (no hook emits it — `HandleStop` notifies only, `HandleSessionEnd` logs locally only, `HandlePreCompact` logs locally only)
- `TASK:*` — skill or agent task messages (`TASK:Result`, `TASK:Progress`)
- `NOTE:*` — human or skill notes (`NOTE:INFO`, `NOTE:WARN`)
- `CHAT:MESSAGE` — direct session-to-session chat

The server does not enforce the vocabulary; it is convention.

### 5.2 Local tables (moved)

> **Moved local (v0.16.0):** `steop_sessions`, `steop_storage_session`, `steop_storage_project`, `steop_logs` — see [local-storage.md](local-storage.md) for the full DDL and migration framework.

## 6. RPC API

As of v0.16.0, only the cross-agent surface (`mailbox.*`, `notify`) remains on stele-server at `POST /api/v1/steop/<method>`. The session, state, storage, and log surfaces moved to the local SQLite database per [PRD-020](../prd/prd-020-steop-local-backend.md) — access them via CLI subcommands (`steop state get`, `steop storage list`, etc.) or through `internal/store` in Go code.

Every method is `POST /api/v1/steop/<method>` with `Content-Type: application/json`. No path parameters, no query parameters, no header-based identity.

### 6.1 Method catalogue

#### Mailbox

| Method                  | Body                                                      | Returns                       |
| ----------------------- | --------------------------------------------------------- | ----------------------------- |
| `steop.mailbox.send`    | `{id, to, from?, subject?, message_type?, meta?, payload?}` | `{message_id, from, to, created_at, ...}` |
| `steop.mailbox.list`    | `{id, to?, status?, message_type?, limit?}`               | `{messages: MailboxRow[]}`    |
| `steop.mailbox.get`     | `{id, message_id}`                                        | `MailboxRow` or 404           |
| `steop.mailbox.read`    | `{id, message_id}`                                        | `{message_id, status:"READ"}` |
| `steop.mailbox.archive` | `{id, message_id}`                                        | `{message_id, status:"ARCHIVE"}` |

`id` is the caller's own composite identifier (mandatory on all steop RPC calls). `from` defaults to the caller's `id` when omitted. `to` may be any principal (project, session, or user). `status` on `mailbox.list` defaults to `["NEW"]`. Filter by recipient + status set. Insert default: `status=NEW`. Illegal status transitions return 409. `mailbox.get` is side-effect free. Ordered by `created_at` ASC (FIFO).

#### Notifications

| Method         | Body                                        | Returns    |
| -------------- | ------------------------------------------- | ---------- |
| `steop.notify` | `{title?, body?, subtitle?, sound?=false}`  | `{}` / 501 |

Unchanged from v0.5 semantics. No identity fields (notifications are local to the server host).

### 6.2 Response types

```json
// MailboxRow
{
  "message_id":   1234,
  "from":         "host:project_dir[:uuid|:USER]",
  "to":           "host:project_dir[:uuid|:USER]",
  "subject":      "string",
  "message_type": "string",
  "meta":         {},
  "payload":      {},
  "created_at":   "string (RFC3339)",
  "status":       "NEW | READ | ARCHIVE"
}
```

### 6.3 Removed surfaces

**Removed in v0.16.0** (moved to local SQLite — see [local-storage.md](local-storage.md)):

```
POST /api/v1/steop/steop.session.start    POST /api/v1/steop/steop.session.stop
POST /api/v1/steop/steop.session.touch    POST /api/v1/steop/steop.session.get
POST /api/v1/steop/steop.session.list     POST /api/v1/steop/steop.project.list
POST /api/v1/steop/steop.state.get        POST /api/v1/steop/steop.state.put
POST /api/v1/steop/steop.state.incr       POST /api/v1/steop/steop.state.reset
POST /api/v1/steop/steop.state.delete     POST /api/v1/steop/steop.status.get
POST /api/v1/steop/steop.storage.put      POST /api/v1/steop/steop.storage.get
POST /api/v1/steop/steop.storage.delete   POST /api/v1/steop/steop.storage.list
POST /api/v1/steop/steop.log.append       POST /api/v1/steop/steop.log.query
```

These endpoints had no external consumers beyond the `steop` binary itself. They are removed without a deprecation shim in the same v0.16.0 release.

**Removed in v0.7 / v0.6** (path/query/header-based routes from v0.5):

Clients that were relying on `X-Steop-Host` / `X-Steop-Project-Dir` headers were required to migrate to body-based identity by v0.6.

## 7. Hook taxonomy

"Local" operations use `*store.DB` (local SQLite). "Stele" operations use `*client.Client` (HTTP to stele-server).

| Event               | Handler                    | Backends          | Behavior (v0.16.1+)                                                                                                                    |
| ------------------- | -------------------------- | ----------------- | -------------------------------------------------------------------------------------------------------------------------------------- |
| `UserPromptSubmit`  | `HandleUserPromptSubmit`   | —                 | Writes session ID to sentinel file. Injects SKILL.md body if prompt matches skill trigger regex.                                       |
| `PreToolUse`        | `HandlePreToolUse`         | —                 | Regex-matches Bash commands for dangerous patterns; returns `DenyPreToolUse` or `Allow`.                                               |
| `PostToolUse`       | `HandlePostToolUse`        | Local             | One `BEGIN IMMEDIATE` transaction: increment `tool_calls` counter, update `data.{last_tool, last_tool_at}`, append log row.            |
| `Stop`              | `HandleStop`               | Local + Stele (notify only) | Local: clear phase/mode, cleanup watcher tasks, cleanup watcher state/heartbeat storage. Stele: `steop.notify` desktop notification only. |
| `SessionStart`      | `HandleSessionStart`       | Local             | `store.Sessions.Start {cwd, permission_mode}` + `store.Logs.Append {event:"session_start"}`.                                          |
| `SessionEnd`        | `HandleSessionEnd`         | Local             | Local: append `session_end` log, cleanup watcher tasks, mark session stopped.                                                           |
| `PermissionRequest` | `HandlePermissionRequest`  | —                 | Returns `Allow()` unconditionally (observe-only, v1).                                                                                  |
| `PostToolUseFailure`| `HandlePostToolUseFailure` | Local             | `store.Logs.Append {event:"post_tool_use_failure", data:{tool_name, error, is_interrupt}}`.                                            |
| `SubagentStart`     | `HandleSubagentStart`      | Local             | `store.Logs.Append {event:"subagent_start", data:{agent_id, agent_type, model, prompt (truncated)}}`.                                 |
| `SubagentStop`      | `HandleSubagentStop`       | Local             | `store.Logs.Append {event:"subagent_stop", data:{agent_id, agent_type, output (truncated), success}}`.                                |
| `PreCompact`        | `HandlePreCompact`         | Local             | `store.Logs.Append {event:"pre_compact", data:{trigger, cwd}}`.                                                                        |

## 8. Phase roadmap

- **v0.1–0.4** — initial spike, hook skeleton, state API, counters.
- **v0.5 (previous)** — REST API. Log + inbox append-only. Composite session identity (`host` + `project_dir`) via `X-Steop-*` headers. `steop_state` + `steop_counters` separate tables. PreToolUse safety rules. `persistent_mode` flag stored but not honored.
- **v0.6** — RPC redesign. Breaking API migration: all `/api/v1/steop/*` endpoints became `POST /api/v1/steop/<method>` RPC with body-only input. Structured `{host, project_dir, session_id}` triples carried composite identity. New tables: `steop_sessions` (merges state + counters), `steop_storage_session`, `steop_storage_project`, `steop_mailbox` (replaces `steop_inbox`). Explicit `session.start`/`stop`/`touch` lifecycle. Mailbox with project-level and session-level addressing plus explicit ack. Go and Rust clients migrated.
- **v0.7** — composite id wire format. The `{host, project_dir, session_id}` triple collapses into a single colon-separated `id` string at the wire layer (`host:project_dir` or `host:project_dir:uuid`). Schema unchanged. Short-form session lookups removed — `session.get`/`state.get`/`status.get` all require the full 3-segment id. Go and Rust clients rewritten.
- **v0.16.0 (current)** — local SQLite backend. Session/state/storage/log surfaces move from stele-server to `~/.local/share/steop/steop.db`. PostToolUse collapses from 3 HTTP RTTs to 1 local transaction. Cross-agent surface (mailbox, notify) stays on stele-server unchanged. See [PRD-020](../prd/prd-020-steop-local-backend.md).
- **v1.0** — release surface. Prebuilt binaries, optional MCP tool wrappers, FTS over log payloads.

## 9. Versioning

The RPC contract under `/api/v1/steop/*` is versioned together with the stele-server workspace version. v0.6.0 was a breaking migration from v0.5. v0.7.0 collapsed the `{host, project_dir, session_id}` triple into a single composite `id` string. v0.16.0 is a further hard break: 18 routes are removed from stele-server (session/project/state/status/storage/log) and the local SQLite backend replaces them. The remaining stele-server surface (mailbox, notify) is additive-stable going forward. Future new methods bump minor; breaking changes bump minor until v1.0, then require `/api/v2/steop/*`.

The stele workspace version, the steop plugin version, and the Go binary version must always match. Use `scripts/bump-version.py` to move them in lock-step.

## 10. Smoke tests

See [smoke-tests.md](smoke-tests.md) for curl sequences that exercise the stele-backed surface (mailbox, notify). For the local SQLite surfaces (session/state/storage/logs), use `steop state get`, `steop storage list`, etc. — see [local-storage.md](local-storage.md).

## 11. Known limitations

- No server-side auth enforcement yet on steop endpoints beyond what the existing stele auth middleware provides.
- Session lookup by UUID is host-local in the local SQLite backend. A UUID minted on host A cannot be resolved on host B. See [local-storage.md §Cross-host limitations](local-storage.md).
- The `steop` binary must be rebuilt after every Go source change. No auto-rebuild on install.
- Local DB logs and stele mailbox are append-only with no TTL. Mailbox rows stay in the table after archive (for audit); rows accumulate until manually pruned.
- The server only validates that the composite `id` parses into the expected number of segments. It does not validate that `host` looks like a hostname or that `project_dir` is absolute. Clients must take care.
- `PermissionRequest` handler is observe-only.
