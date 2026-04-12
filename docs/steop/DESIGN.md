# Steop Design (v2, 0.7.0)

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

3. **Stele server API** (`/api/v1/steop/*`) — RPC-style endpoints on the existing stele-server process. Every method is `POST /api/v1/steop/<method>` with a JSON body. Tables (`steop_sessions`, `steop_storage_session`, `steop_storage_project`, `steop_mailbox`, `steop_logs`) live alongside the existing `memories`, `entities`, `relations` tables. The server is the single source of truth.

```
Claude Code lifecycle
       │
       ▼
steop hook <event>  ←── hooks.json
       │
       │   HTTP POST /api/v1/steop/<method>
       ▼
stele-server process
       │
       ▼
 SQLite  (steop_sessions / steop_storage_session / steop_storage_project / steop_mailbox / steop_logs)
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

Storage methods (`storage.put`/`get`/`delete`/`list`) accept either arity of id. A 2-segment id routes to `steop_storage_project`; a 3-segment id routes to `steop_storage_session`. Every other id-bearing method requires the full 3-segment form; the server returns `HTTP 400 {"error":"id must be 3-segment (host:project_dir:session_uuid)"}` on an incomplete id.

### No server-side validation beyond parsing

The server does not validate that `host` looks like a hostname, that `project_dir` is absolute, or that the UUID refers to a real Claude Code session. It only enforces the segment grammar above. Clients are responsible for completeness and semantic consistency across related calls.

### No headers

v0.5 used `X-Steop-Host` and `X-Steop-Project-Dir` headers as an implicit identity channel. **v0.6 ignored these headers and used structured `{host, project_dir, session_id}` triples.** v0.7 collapses the triple into a single `id` string in every request body. All identity is explicit in the request body.

## 5. Persistence model

Five tables under the `steop_*` prefix. All are created idempotently by `ensure_steop_schema()` at server startup. The **schema** still keeps `host`, `project_dir`, `session_id` as separate columns — only the **wire format** collapses them into a single composite id. The server parses the composite id on ingress and composes it back on egress.

### 5.1 `steop_sessions` — session registry + state + counters

One row per `(host, project_dir, session_id)`. Replaces the v0.5 `steop_state` + `steop_counters` tables. Counters live inside a JSON column on the session row; under the server's serialized SQLite mutex, read-modify-write on JSON is race-free.

```sql
CREATE TABLE IF NOT EXISTS steop_sessions (
    host           TEXT NOT NULL,
    project_dir    TEXT NOT NULL,
    session_id     TEXT NOT NULL,
    state          TEXT NOT NULL DEFAULT 'active',  -- 'active' | 'stopped'
    started_at     TEXT NOT NULL,
    last_active_at TEXT NOT NULL,
    stopped_at     TEXT,
    data           TEXT NOT NULL DEFAULT '{}',      -- JSON: phase, mode, step, arbitrary keys
    counters       TEXT NOT NULL DEFAULT '{}',      -- JSON: { "tool_calls": 12, "loop_count": 3 }
    PRIMARY KEY (host, project_dir, session_id)
);
CREATE INDEX IF NOT EXISTS idx_steop_sessions_host_proj  ON steop_sessions(host, project_dir);
CREATE INDEX IF NOT EXISTS idx_steop_sessions_session_id ON steop_sessions(session_id);
```

The `idx_steop_sessions_session_id` index from v0.6 is obsolete for lookup (v0.7 always provides the full triple via the composite id) but is retained for schema stability. `data` and `counters` are opaque JSON; only top-level keys are projected by `steop.status.get`.

### 5.2 `steop_storage_session` — session-scoped KV

```sql
CREATE TABLE IF NOT EXISTS steop_storage_session (
    host        TEXT NOT NULL,
    project_dir TEXT NOT NULL,
    session_id  TEXT NOT NULL,
    key         TEXT NOT NULL,
    content     TEXT NOT NULL,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL,
    PRIMARY KEY (host, project_dir, session_id, key)
);
```

### 5.3 `steop_storage_project` — project-scoped KV

```sql
CREATE TABLE IF NOT EXISTS steop_storage_project (
    host        TEXT NOT NULL,
    project_dir TEXT NOT NULL,
    key         TEXT NOT NULL,
    content     TEXT NOT NULL,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL,
    PRIMARY KEY (host, project_dir, key)
);
```

The two storage tables are dispatched by the arity of the composite `id` in the request body. A 2-segment id (`host:project_dir`) routes to `steop_storage_project`; a 3-segment id (`host:project_dir:uuid`) routes to `steop_storage_session`. There is no "global" scope — every blob is anchored to at least a project.

### 5.4 `steop_mailbox` — inter-session messaging

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

- `HOOK:*` — hook-originated messages (`HOOK:Stop`, `HOOK:SessionEnd`, `HOOK:PreCompact`, …)
- `TASK:*` — skill or agent task messages (`TASK:Result`, `TASK:Progress`)
- `NOTE:*` — human or skill notes (`NOTE:INFO`, `NOTE:WARN`)
- `CHAT:MESSAGE` — direct session-to-session chat

The server does not enforce the vocabulary; it is convention.

### 5.5 `steop_logs` — append-only structured event log

```sql
CREATE TABLE IF NOT EXISTS steop_logs (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    host        TEXT NOT NULL,
    project_dir TEXT NOT NULL,
    session_id  TEXT NOT NULL,
    event       TEXT NOT NULL,
    data        TEXT NOT NULL DEFAULT '{}',
    created_at  TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_steop_logs_session ON steop_logs(session_id, created_at);
CREATE INDEX IF NOT EXISTS idx_steop_logs_proj    ON steop_logs(host, project_dir, created_at);
```

v0.7 clients always send a 3-segment composite `id` in every log write; the server splits it into the three columns on insert.

## 6. RPC API

All methods are `POST /api/v1/steop/<method>` with `Content-Type: application/json`. The method name is dot-separated (`steop.session.start`, `steop.storage.put`, etc.). Request body is a JSON object. Response is a JSON object. Errors use `{ "error": "message" }` with an appropriate HTTP status code.

There are no path parameters, no query parameters, and no header-based identity. This makes the transport trivially proxyable and loggable.

### 6.1 Method catalogue

#### Session lifecycle

| Method                | Body                                                     | Returns                    |
| --------------------- | -------------------------------------------------------- | -------------------------- |
| `steop.session.start` | `{id, data?}`                                            | `Session`                  |
| `steop.session.stop`  | `{id}`                                                   | `Session`                  |
| `steop.session.touch` | `{id}`                                                   | `Session`                  |
| `steop.session.get`   | `{id}`                                                   | `Session` or 404           |
| `steop.session.list`  | `{host?, project_dir?, state?, limit?}`                  | `{sessions: Session[]}`    |
| `steop.project.list`  | `{host?}`                                                | `{projects: [{id}]}`       |

`id` on all lifecycle write methods is the 3-segment composite. `start` is idempotent — if the row already exists, set `state='active'`, refresh `last_active_at`, clear `stopped_at`, and merge `data` if supplied. `stop` sets `state='stopped'` and `stopped_at=now`. `touch` only updates `last_active_at`.

`session.list` takes structured filter fields and does **not** use composite ids: no fields = all sessions; `{host}` = all sessions for a host; `{host, project_dir}` = all sessions for a project; `{state:"active"}` = filter by lifecycle state. Ordered by `last_active_at` DESC, default `limit=100`. `project.list` returns a list of project-level composite ids.

#### Session state and counters

| Method               | Body                                       | Returns                  |
| -------------------- | ------------------------------------------ | ------------------------ |
| `steop.state.get`    | `{id}`                                     | `Session` or 404         |
| `steop.state.put`    | `{id, data, merge?=true}`                  | `Session`                |
| `steop.state.incr`   | `{id, counter, delta?=1}`                  | `{counter, value}`       |
| `steop.state.reset`  | `{id, counter, value?=0}`                  | `{counter, value}`       |
| `steop.state.delete` | `{id}`                                     | `{deleted: true|false}`  |

All state methods require a 3-segment `id`. `state.put` merges into the `data` JSON column (shallow merge, top-level key replacement) unless `merge:false` replaces the object entirely. `incr`/`reset` operate on the `counters` JSON column. All write methods refresh `last_active_at` and create the session row if absent (implicit start; `state='active'`).

#### Statusline projection

| Method             | Body   | Returns                              |
| ------------------ | ------ | ------------------------------------ |
| `steop.status.get` | `{id}` | `StatusProjection` (always 200)      |

`id` must be a 3-segment composite. Projects `{id, mode, phase, step, tool_calls, loop_count, step_retry, last_active_at}` from `data` + `counters`. Returns defaulted values for unknown sessions so the statusline render path has no error branch.

#### Storage (generic KV)

| Method                 | Body                      | Returns                               |
| ---------------------- | ------------------------- | ------------------------------------- |
| `steop.storage.put`    | `{id, key, content}`      | `{key, updated_at}`                   |
| `steop.storage.get`    | `{id, key}`               | `StorageBlob` or 404                  |
| `steop.storage.delete` | `{id, key}`               | `{deleted: true|false}`               |
| `steop.storage.list`   | `{id}`                    | `{items: [{key, updated_at, size}]}`  |

`id` is a 2-segment composite (project scope) or a 3-segment composite (session scope). Arity selects which table. Writes are upserts that refresh `updated_at`.

#### Log

| Method             | Body                                               | Returns             |
| ------------------ | -------------------------------------------------- | ------------------- |
| `steop.log.append` | `{id, event, data?}`                               | `{id}`              |
| `steop.log.query`  | `{host?, project_dir?, session_id?, limit?=200}`   | `{logs: LogRow[]}`  |

`log.append` requires a 3-segment session id. `log.query` keeps structured filter fields (host, project_dir, session_id are separate scalars — this stays an ad-hoc filter surface, not a composite-id lookup) so callers can drain logs across a whole host or project without composing an id. Ordered by `created_at` DESC.

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
// Session
{
  "id":             "host:project_dir:uuid",
  "state":          "active | stopped",
  "started_at":     "string (RFC3339)",
  "last_active_at": "string (RFC3339)",
  "stopped_at":     "string (RFC3339) | null",
  "data":           {},
  "counters":       { "tool_calls": 12 }
}

// StatusProjection
{
  "id":             "host:project_dir:uuid",
  "mode":           "string",
  "phase":          "string",
  "step":           "string",
  "tool_calls":     0,
  "loop_count":     0,
  "step_retry":     0,
  "last_active_at": "string (RFC3339)"
}

// StorageBlob
{
  "id":         "host:project_dir (2-seg) | host:project_dir:uuid (3-seg)",
  "key":        "string",
  "content":    "string",
  "created_at": "string (RFC3339)",
  "updated_at": "string (RFC3339)"
}

// LogRow
{
  "id":         1234,
  "session_id": "host:project_dir:uuid",
  "event":      "string",
  "data":       {},
  "created_at": "string (RFC3339)"
}

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

### 6.3 Removed v0.5 surface

All REST routes under `/api/v1/steop/*` that used path parameters, query parameters, or header-based identity are **removed** in v2. There is no deprecation window (stele is pre-1.0). The removed routes are:

```
PUT/GET/DELETE /api/v1/steop/storage?scope=&key=
GET            /api/v1/steop/storage/list?scope=
GET            /api/v1/steop/storage/scopes
GET/PUT/DELETE /api/v1/steop/state/{session_id}
POST           /api/v1/steop/state/{session_id}/incr
POST           /api/v1/steop/state/{session_id}/reset
GET            /api/v1/steop/status/{session_id}
GET            /api/v1/steop/sessions
GET            /api/v1/steop/sessions/{id}
POST           /api/v1/steop/notify
POST/GET       /api/v1/steop/log
POST/GET       /api/v1/steop/inbox
```

Clients that were relying on the `X-Steop-Host` / `X-Steop-Project-Dir` headers must be updated to send identity in the request body. The headers are ignored by v2.

## 7. Hook taxonomy

| Event               | Handler                    | Client required | Behavior                                                                                                              |
| ------------------- | -------------------------- | --------------- | --------------------------------------------------------------------------------------------------------------------- |
| `UserPromptSubmit`  | `HandleUserPromptSubmit`   | No              | Writes session ID to sentinel file. Injects SKILL.md body if prompt matches skill trigger regex.                      |
| `PreToolUse`        | `HandlePreToolUse`         | No              | Regex-matches Bash commands for dangerous patterns; returns `DenyPreToolUse` or `Allow`.                              |
| `PostToolUse`       | `HandlePostToolUse`        | Yes             | `steop.state.incr {counter:"tool_calls"}` + `steop.state.put {data:{last_tool, last_tool_at}, merge:true}` + `steop.log.append` |
| `Stop`              | `HandleStop`               | Yes             | `steop.notify` + `steop.state.get` + `steop.mailbox.send` (to project-level) + `steop.state.put {data:{phase:null, mode:null}, merge:true}` |
| `SessionStart`      | `HandleSessionStart`       | Yes             | `steop.session.start {id, data:{cwd, permission_mode}}` + `steop.log.append {id, event:"session_start"}` |
| `SessionEnd`        | `HandleSessionEnd`         | Yes             | `steop.log.append {event:"session_end", data:{reason,...}}` + `steop.mailbox.send` (project-level summary) + `steop.session.stop` |
| `PermissionRequest` | `HandlePermissionRequest`  | No              | Returns `Allow()` unconditionally (observe-only, v1).                                                                 |
| `PostToolUseFailure`| `HandlePostToolUseFailure` | Yes             | `steop.log.append {event:"post_tool_use_failure", data:{tool_name, error, is_interrupt}}`                             |
| `SubagentStart`     | `HandleSubagentStart`      | Yes             | `steop.log.append {event:"subagent_start", data:{agent_id, agent_type, model, prompt (truncated)}}`                   |
| `SubagentStop`      | `HandleSubagentStop`       | Yes             | `steop.log.append {event:"subagent_stop", data:{agent_id, agent_type, output (truncated), success}}`                  |
| `PreCompact`        | `HandlePreCompact`         | Yes             | `steop.log.append {event:"pre_compact", data:{trigger, cwd}}`                                                         |

## 8. Phase roadmap

- **v0.1–0.4** — initial spike, hook skeleton, state API, counters.
- **v0.5 (previous)** — REST API. Log + inbox append-only. Composite session identity (`host` + `project_dir`) via `X-Steop-*` headers. `steop_state` + `steop_counters` separate tables. PreToolUse safety rules. `persistent_mode` flag stored but not honored.
- **v0.6** — RPC redesign. Breaking API migration: all `/api/v1/steop/*` endpoints became `POST /api/v1/steop/<method>` RPC with body-only input. Structured `{host, project_dir, session_id}` triples carried composite identity. New tables: `steop_sessions` (merges state + counters), `steop_storage_session`, `steop_storage_project`, `steop_mailbox` (replaces `steop_inbox`). Explicit `session.start`/`stop`/`touch` lifecycle. Mailbox with project-level and session-level addressing plus explicit ack. Go and Rust clients migrated.
- **v0.7 (current)** — composite id wire format. The `{host, project_dir, session_id}` triple collapses into a single colon-separated `id` string at the wire layer (`host:project_dir` or `host:project_dir:uuid`). Schema unchanged. Short-form session lookups removed — `session.get`/`state.get`/`status.get` all require the full 3-segment id. Go and Rust clients rewritten; tests passing. Consumer work (mailbox drain into `/stele:sync`, `steop recap`, deliverable verification on `SubagentStop`) moves to v0.8.
- **v0.8** — persistent-mode honored. Stop hook returns `{"decision":"block","reason":"..."}` when `persistent_mode` flag is set, with safety guards against infinite loops.
- **v1.0** — release surface. Prebuilt binaries, optional MCP tool wrappers around the RPC methods, FTS over log payloads.

## 9. Versioning

The RPC contract under `/api/v1/steop/*` is versioned together with the stele-server workspace version. v0.6.0 was a breaking migration from v0.5 — endpoints, schema, and identity model all changed. v0.7.0 is another hard break: the wire format for identity collapsed from a `{host, project_dir, session_id}` triple into a single composite `id` string. Schema is stable across v0.6 → v0.7. Future additive changes (new methods, new optional fields) bump minor. Any further breaking change bumps minor again until v1.0, at which point SemVer kicks in and breaking changes require a `/api/v2/steop/*` prefix.

The stele workspace version, the steop plugin version, and the Go binary version must always match. Use `scripts/bump-version.py` to move them in lock-step.

## 10. Verifying v0.7 (smoke tests)

See [smoke-tests.md](smoke-tests.md) for a copy-paste curl sequence that exercises every RPC method end-to-end against a running `stele-server`.

## 11. Known limitations

- No server-side auth enforcement yet on steop endpoints beyond what the existing stele auth middleware provides.
- No migrations subsystem. v0.6.0 was a hard break from v0.5 — old `steop_storage`, `steop_state`, `steop_counters`, `steop_inbox` tables are superseded by new tables. v0.7.0 is a hard break on the wire format only; the schema is stable so existing DB files continue to work, but every client must be rebuilt in lock-step.
- The stele-server uses a shared tokio mutex around its SQLite connection, so all DB access (including steop) is serialized. Counters-as-JSON in `steop_sessions.counters` is race-free under this mutex. Fine for workflow-scale traffic; would need revisiting for high concurrency.
- The `steop` binary must be rebuilt after every Go source change. No auto-rebuild on install.
- Status projection has no background materializer; it computes on read.
- Logs and mailbox are append-only with no TTL. Mailbox rows stay in the table after ack (for audit); rows accumulate until manually pruned.
- The server only validates that the composite `id` parses into the expected number of segments. It does not validate that `host` looks like a hostname, that `project_dir` is absolute, or that the session UUID corresponds to a real Claude Code session. A client that sends `{id:"x: :uuid"}` will happily create a row with a space in `project_dir`. Clients must take care.
- `persistent_mode` flag is stored but not honored — Stop always returns `Allow()`. Full block-and-resume loop is v0.8.
- `PermissionRequest` handler is observe-only.
