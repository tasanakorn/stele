# Steop Design (v2, 0.6.0)

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

Steop addresses every resource with an **SSH/SCP-style composite identifier**. There are no implicit defaults and no header-based identity — v2 is body-only.

### Identifier grammar

```
project_ref  = host ":" project_dir
session_ref  = host ":" project_dir ":" session_id
```

Examples:

- `vm-02:/home/tas/stele`               — a project on host `vm-02`
- `vm-02:/home/tas/stele:a1b2c3d4-...`  — a specific session inside that project
- `laptop:/Users/tas/work:9f...`         — a session on a different machine

`host` is the machine name (e.g. `os.Hostname()` in Go, `gethostname()` in Rust). `project_dir` is an absolute path. `session_id` is the Claude Code session UUID.

Because Claude Code session UUIDs are globally unique in practice, **read** methods (`session.get`, `state.get`, `status.get`) may accept a bare `session_id` as a short form. **Write** methods always require the full triple (`host`, `project_dir`, `session_id`).

### No server-side validation

The server does not validate that `host` looks like a hostname, that `project_dir` is absolute, or that identity fields are consistent across related calls. Clients are responsible for completeness. Empty strings are tolerated and treated as literal values.

### No headers

v0.5 used `X-Steop-Host` and `X-Steop-Project-Dir` headers as an implicit identity channel. **v2 ignores these headers.** All identity is explicit in the request body.

## 5. Persistence model

Five tables under the `steop_*` prefix. All are created idempotently by `ensure_steop_schema()` at server startup.

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

The `session_id` index supports short-form read lookups. `data` and `counters` are opaque JSON; only top-level keys are projected by `steop.status.get`.

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

The two storage tables are dispatched by presence of `session_id` in the request body. `steop.storage.put {host, project_dir, key, content}` writes the project table; adding `session_id` routes to the session table. There is no "global" scope — every blob is anchored to at least a project.

### 5.4 `steop_mailbox` — inter-session messaging

Replaces the v0.5 `steop_inbox` table. "Mailbox" is the subsystem name (not a place); messages are addressed to either a session or a project.

```sql
CREATE TABLE IF NOT EXISTS steop_mailbox (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    from_host        TEXT NOT NULL,
    from_project_dir TEXT NOT NULL,
    from_session_id  TEXT NOT NULL,
    to_host          TEXT NOT NULL,
    to_project_dir   TEXT NOT NULL,
    to_session_id    TEXT NOT NULL DEFAULT '',  -- '' = project-level recipient
    payload          TEXT NOT NULL DEFAULT '{}',
    created_at       TEXT NOT NULL,
    acked_at         TEXT,
    kind             TEXT NOT NULL DEFAULT 'LEGACY:UNKNOWN',
    subject          TEXT NOT NULL DEFAULT ''
);
CREATE INDEX IF NOT EXISTS idx_steop_mailbox_to
    ON steop_mailbox(to_host, to_project_dir, to_session_id, created_at);
```

Addressing rules:

- **Sender** (`from_*`) is **always** a session — a message must originate from a concrete `(host, project_dir, session_id)`. There is no "project sent this" origin. Hooks and skills must have a live session to send mail.
- **Recipient** (`to_*`) is either:
  - a project — `to_session_id = ''`, inbox drains to any session in that project that polls for it
  - a session — `to_session_id` is the full UUID, delivered to that session only

Ack is explicit via `steop.mailbox.ack {id}`. Acked messages stay in the table (for audit) with a non-null `acked_at`; list operations return unacked by default.

#### Envelope fields

Every message carries two envelope fields that describe its origin and intent without requiring readers to inspect `payload`:

- **`kind`** (required, non-empty) — structured message type. Vocabulary:
  - `HOOK:Stop` — fired by the Stop hook with a session summary
  - `HOOK:SessionEnd` — fired by the SessionEnd hook when Claude Code terminates a session
  - `LEGACY:UNKNOWN` — default for rows created before envelope fields existed
  - `TASK:*` — reserved for task-level messages from skills (e.g. `TASK:Result`, `TASK:Progress`)
  - `NOTE:*` — reserved for human-authored or skill-authored notes
  - `CHAT:MESSAGE` — reserved for direct session-to-session messages
- **`subject`** (required, may be empty string) — human-readable one-line summary. For `HOOK:Stop` this is the truncated last assistant message. For `HOOK:SessionEnd` this is the session end reason or `"session ended"` if empty.

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

v2 clients always populate all three identity fields (`host`, `project_dir`, `session_id`) in every log write.

## 6. RPC API

All methods are `POST /api/v1/steop/<method>` with `Content-Type: application/json`. The method name is dot-separated (`steop.session.start`, `steop.storage.put`, etc.). Request body is a JSON object. Response is a JSON object. Errors use `{ "error": "message" }` with an appropriate HTTP status code.

There are no path parameters, no query parameters, and no header-based identity. This makes the transport trivially proxyable and loggable.

### 6.1 Method catalogue

#### Session lifecycle

| Method                | Body                                                           | Returns                              |
| --------------------- | -------------------------------------------------------------- | ------------------------------------ |
| `steop.session.start` | `{host, project_dir, session_id, data?}`                       | `Session`                            |
| `steop.session.stop`  | `{host, project_dir, session_id}`                              | `Session`                            |
| `steop.session.touch` | `{host, project_dir, session_id}`                              | `Session`                            |
| `steop.session.get`   | `{session_id}` **or** `{host, project_dir, session_id}`        | `Session` or 404                     |
| `steop.session.list`  | `{host?, project_dir?, state?, limit?}`                        | `{sessions: Session[]}`              |
| `steop.project.list`  | `{host?}`                                                      | `{projects: [{host, project_dir}]}`  |

`start` is idempotent — if the row already exists, set `state='active'`, refresh `last_active_at`, clear `stopped_at`, and merge `data` if supplied. `stop` sets `state='stopped'` and `stopped_at=now`. `touch` only updates `last_active_at`.

`session.list` filters: no fields = all sessions; `{host}` = all sessions for a host; `{host, project_dir}` = all sessions for a project; `{state:"active"}` = filter by lifecycle state. Ordered by `last_active_at` DESC, default `limit=100`.

#### Session state and counters

| Method               | Body                                                          | Returns                  |
| -------------------- | ------------------------------------------------------------- | ------------------------ |
| `steop.state.get`    | `{session_id}` **or** `{host, project_dir, session_id}`       | `Session` or 404         |
| `steop.state.put`    | `{host, project_dir, session_id, data, merge?=true}`          | `Session`                |
| `steop.state.incr`   | `{host, project_dir, session_id, counter, delta?=1}`          | `{counter, value}`       |
| `steop.state.reset`  | `{host, project_dir, session_id, counter, value?=0}`          | `{counter, value}`       |
| `steop.state.delete` | `{host, project_dir, session_id}`                             | `{deleted: true|false}`  |

`state.put` merges into the `data` JSON column (shallow merge, top-level key replacement) unless `merge:false` replaces the object entirely. `incr`/`reset` operate on the `counters` JSON column. All write methods refresh `last_active_at` and create the session row if absent (implicit start; `state='active'`).

#### Statusline projection

| Method             | Body                                                    | Returns                              |
| ------------------ | ------------------------------------------------------- | ------------------------------------ |
| `steop.status.get` | `{session_id}` **or** `{host, project_dir, session_id}` | `StatusProjection` (always 200)      |

Projects `{session_id, mode, phase, step, tool_calls, loop_count, step_retry, last_active_at}` from `data` + `counters`. Returns defaulted values for unknown sessions so the statusline render path has no error branch.

#### Storage (generic KV)

| Method                 | Body                                                     | Returns                               |
| ---------------------- | -------------------------------------------------------- | ------------------------------------- |
| `steop.storage.put`    | `{host, project_dir, key, content, session_id?}`         | `{key, updated_at}`                   |
| `steop.storage.get`    | `{host, project_dir, key, session_id?}`                  | `StorageBlob` or 404                  |
| `steop.storage.delete` | `{host, project_dir, key, session_id?}`                  | `{deleted: true|false}`               |
| `steop.storage.list`   | `{host, project_dir, session_id?}`                       | `{items: [{key, updated_at, size}]}`  |

Presence of `session_id` selects `steop_storage_session`; absence selects `steop_storage_project`. Writes are upserts that refresh `updated_at`.

#### Log

| Method             | Body                                                     | Returns             |
| ------------------ | -------------------------------------------------------- | ------------------- |
| `steop.log.append` | `{host, project_dir, session_id, event, data?}`          | `{id}`              |
| `steop.log.query`  | `{host?, project_dir?, session_id?, limit?=200}`         | `{logs: LogRow[]}`  |

`query` filters additively: no fields = all logs, `{session_id}` = one session, `{host, project_dir}` = one project, etc. Ordered by `created_at` DESC.

#### Mailbox

| Method               | Body                                                                                                                 | Returns                    |
| -------------------- | -------------------------------------------------------------------------------------------------------------------- | -------------------------- |
| `steop.mailbox.send` | `{from_host, from_project_dir, from_session_id, to_host, to_project_dir, to_session_id?, kind, subject, payload}`   | `{id}`                     |
| `steop.mailbox.list` | `{to_host, to_project_dir, to_session_id?, limit?=200, include_acked?=false}`                                        | `{messages: MailboxRow[]}` |
| `steop.mailbox.ack`  | `{id}`                                                                                                               | `{acked: true|false}`      |

A `list` call with `{to_host, to_project_dir}` returns project-level messages (those with `to_session_id=''`); adding `to_session_id` returns session-level messages addressed to that session. Short identifiers are **not** supported in mailbox methods — addressing must always be fully qualified. Ordered by `created_at` ASC (FIFO).

#### Notifications

| Method         | Body                                        | Returns    |
| -------------- | ------------------------------------------- | ---------- |
| `steop.notify` | `{title?, body?, subtitle?, sound?=false}`  | `{}` / 501 |

Unchanged from v0.5 semantics. No identity fields (notifications are local to the server host).

### 6.2 Response types

```json
// Session
{
  "host":           "string",
  "project_dir":    "string",
  "session_id":     "string",
  "state":          "active | stopped",
  "started_at":     "string (RFC3339)",
  "last_active_at": "string (RFC3339)",
  "stopped_at":     "string (RFC3339) | null",
  "data":           {},
  "counters":       { "tool_calls": 12 }
}

// StatusProjection
{
  "session_id":     "string",
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
  "host":        "string",
  "project_dir": "string",
  "session_id":  "string | null",
  "key":         "string",
  "content":     "string",
  "created_at":  "string (RFC3339)",
  "updated_at":  "string (RFC3339)"
}

// LogRow
{
  "id":          1234,
  "host":        "string",
  "project_dir": "string",
  "session_id":  "string",
  "event":       "string",
  "data":        {},
  "created_at":  "string (RFC3339)"
}

// MailboxRow
{
  "id":               1234,
  "from_host":        "string",
  "from_project_dir": "string",
  "from_session_id":  "string",
  "to_host":          "string",
  "to_project_dir":   "string",
  "to_session_id":    "string",
  "kind":             "string",
  "subject":          "string",
  "payload":          {},
  "created_at":       "string (RFC3339)",
  "acked_at":         "string (RFC3339) | null"
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
| `SessionStart`      | `HandleSessionStart`       | Yes             | `steop.session.start {host, project_dir, session_id, data:{cwd, permission_mode}}` + `steop.log.append {event:"session_start"}` |
| `SessionEnd`        | `HandleSessionEnd`         | Yes             | `steop.log.append {event:"session_end", data:{reason,...}}` + `steop.mailbox.send` (project-level summary) + `steop.session.stop` |
| `PermissionRequest` | `HandlePermissionRequest`  | No              | Returns `Allow()` unconditionally (observe-only, v1).                                                                 |
| `PostToolUseFailure`| `HandlePostToolUseFailure` | Yes             | `steop.log.append {event:"post_tool_use_failure", data:{tool_name, error, is_interrupt}}`                             |
| `SubagentStart`     | `HandleSubagentStart`      | Yes             | `steop.log.append {event:"subagent_start", data:{agent_id, agent_type, model, prompt (truncated)}}`                   |
| `SubagentStop`      | `HandleSubagentStop`       | Yes             | `steop.log.append {event:"subagent_stop", data:{agent_id, agent_type, output (truncated), success}}`                  |
| `PreCompact`        | `HandlePreCompact`         | Yes             | `steop.log.append {event:"pre_compact", data:{trigger, cwd}}`                                                         |

## 8. Phase roadmap

- **v0.1–0.4** — initial spike, hook skeleton, state API, counters.
- **v0.5 (previous)** — REST API. Log + inbox append-only. Composite session identity (`host` + `project_dir`) via `X-Steop-*` headers. `steop_state` + `steop_counters` separate tables. PreToolUse safety rules. `persistent_mode` flag stored but not honored.
- **v0.6 (current)** — RPC redesign. Breaking API migration: all `/api/v1/steop/*` endpoints are now `POST /api/v1/steop/<method>` RPC with body-only input. Composite SSH-style identity (`host:project_dir[:session_id]`) is mandatory and explicit. New tables: `steop_sessions` (merges state + counters), `steop_storage_session`, `steop_storage_project`, `steop_mailbox` (replaces `steop_inbox`). Explicit `session.start`/`stop`/`touch` lifecycle. Mailbox with project-level and session-level addressing plus explicit ack. Go and Rust clients migrated.
- **v0.7** — consumers. `/stele:sync` drains the mailbox for past-session summaries. `steop recap` skill. Deliverable verification heuristics on `SubagentStop`.
- **v0.8** — persistent-mode honored. Stop hook returns `{"decision":"block","reason":"..."}` when `persistent_mode` flag is set, with safety guards against infinite loops.
- **v1.0** — release surface. Prebuilt binaries, optional MCP tool wrappers around the RPC methods, FTS over log payloads.

## 9. Versioning

The RPC contract under `/api/v1/steop/*` is versioned together with the stele-server workspace version. v0.6.0 is a breaking migration from v0.5 — endpoints, schema, and identity model all changed. Future additive changes (new methods, new optional fields) bump minor. Any further breaking change bumps minor again until v1.0, at which point SemVer kicks in and breaking changes require a `/api/v2/steop/*` prefix.

The stele workspace version, the steop plugin version, and the Go binary version must always match. Use `scripts/bump-version.py` to move them in lock-step.

## 10. Verifying v0.6 (smoke tests)

```bash
KEY=...
URL=http://127.0.0.1:3100/api/v1/steop
H="X-Stele-Key: $KEY"
CT="Content-Type: application/json"

# session lifecycle
curl -sS -X POST "$URL/steop.session.start" -H "$H" -H "$CT" \
  -d '{"host":"laptop","project_dir":"/tmp/demo","session_id":"sess-1","data":{"phase":"plan"}}'

curl -sS -X POST "$URL/steop.session.touch" -H "$H" -H "$CT" \
  -d '{"host":"laptop","project_dir":"/tmp/demo","session_id":"sess-1"}'

curl -sS -X POST "$URL/steop.session.get" -H "$H" -H "$CT" \
  -d '{"session_id":"sess-1"}'

curl -sS -X POST "$URL/steop.session.list" -H "$H" -H "$CT" \
  -d '{"host":"laptop","project_dir":"/tmp/demo","state":"active"}'

# state + counters
curl -sS -X POST "$URL/steop.state.put" -H "$H" -H "$CT" \
  -d '{"host":"laptop","project_dir":"/tmp/demo","session_id":"sess-1","data":{"phase":"execute"},"merge":true}'

curl -sS -X POST "$URL/steop.state.incr" -H "$H" -H "$CT" \
  -d '{"host":"laptop","project_dir":"/tmp/demo","session_id":"sess-1","counter":"tool_calls","delta":1}'

curl -sS -X POST "$URL/steop.state.reset" -H "$H" -H "$CT" \
  -d '{"host":"laptop","project_dir":"/tmp/demo","session_id":"sess-1","counter":"tool_calls","value":0}'

# status
curl -sS -X POST "$URL/steop.status.get" -H "$H" -H "$CT" \
  -d '{"session_id":"sess-1"}'

# storage (session-level)
curl -sS -X POST "$URL/steop.storage.put" -H "$H" -H "$CT" \
  -d '{"host":"laptop","project_dir":"/tmp/demo","session_id":"sess-1","key":"plan","content":"{\"steps\":[1,2,3]}"}'

curl -sS -X POST "$URL/steop.storage.get" -H "$H" -H "$CT" \
  -d '{"host":"laptop","project_dir":"/tmp/demo","session_id":"sess-1","key":"plan"}'

curl -sS -X POST "$URL/steop.storage.list" -H "$H" -H "$CT" \
  -d '{"host":"laptop","project_dir":"/tmp/demo","session_id":"sess-1"}'

# storage (project-level, no session_id)
curl -sS -X POST "$URL/steop.storage.put" -H "$H" -H "$CT" \
  -d '{"host":"laptop","project_dir":"/tmp/demo","key":"brief","content":"shared"}'

# log
curl -sS -X POST "$URL/steop.log.append" -H "$H" -H "$CT" \
  -d '{"host":"laptop","project_dir":"/tmp/demo","session_id":"sess-1","event":"post_tool_use","data":{"tool_name":"Bash"}}'

curl -sS -X POST "$URL/steop.log.query" -H "$H" -H "$CT" \
  -d '{"session_id":"sess-1","limit":20}'

# mailbox
curl -sS -X POST "$URL/steop.mailbox.send" -H "$H" -H "$CT" \
  -d '{"from_host":"laptop","from_project_dir":"/tmp/demo","from_session_id":"sess-1","to_host":"laptop","to_project_dir":"/tmp/demo","kind":"NOTE:INFO","subject":"demo message","payload":{"phase":"validate","tool_calls":42}}'

curl -sS -X POST "$URL/steop.mailbox.list" -H "$H" -H "$CT" \
  -d '{"to_host":"laptop","to_project_dir":"/tmp/demo"}'

curl -sS -X POST "$URL/steop.mailbox.ack" -H "$H" -H "$CT" \
  -d '{"id":1}'

# session stop
curl -sS -X POST "$URL/steop.session.stop" -H "$H" -H "$CT" \
  -d '{"host":"laptop","project_dir":"/tmp/demo","session_id":"sess-1"}'
```

## 11. Known limitations

- No server-side auth enforcement yet on steop endpoints beyond what the existing stele auth middleware provides.
- No migrations subsystem. v0.6.0 is a hard break from v0.5 — old `steop_storage`, `steop_state`, `steop_counters`, `steop_inbox` tables are superseded by new tables. Users who need to preserve v0.5 data must export manually before upgrading.
- The stele-server uses a shared tokio mutex around its SQLite connection, so all DB access (including steop) is serialized. Counters-as-JSON in `steop_sessions.counters` is race-free under this mutex. Fine for workflow-scale traffic; would need revisiting for high concurrency.
- The `steop` binary must be rebuilt after every Go source change. No auto-rebuild on install.
- Status projection has no background materializer; it computes on read.
- Logs and mailbox are append-only with no TTL. Mailbox rows stay in the table after ack (for audit); rows accumulate until manually pruned.
- The server does not validate identifier completeness. A client that sends `{host:"", project_dir:"", session_id:"x"}` will create a row with empty host/project_dir. Clients must take care.
- `persistent_mode` flag is stored but not honored — Stop always returns `Allow()`. Full block-and-resume loop is v0.8.
- `PermissionRequest` handler is observe-only.
