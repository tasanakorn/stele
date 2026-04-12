# PRD — Mailbox v2

**Status:** Implemented in v0.8.0
**Target version:** v0.8.0 (next minor after v0.7.0; breaking under pre-1.0 SemVer)
**Scope:** `steop_mailbox` table + `steop.mailbox.*` RPC surface
**Supersedes:** `docs/steop/DESIGN.md` §5.4 and §6.1 mailbox rows (once implemented)
**Author:** repo owner (raw design) + Claude Code (PRD authoring)

---

## 1. Goals

1. **Simple and generic.** One `from` column, one `to` column, one status column — no 6-column identity sprawl, no boolean-ish `acked_at NULL` lifecycle.
2. **Unified principal grammar.** A single composite-identifier format describes every valid sender and recipient: project, session, or named user within a project.
3. **Richer lifecycle.** Three-state `NEW → READ → ARCHIVE` replaces the current binary acked/unacked, enabling future features like "pending read", "archive browse", and retention policies without another schema churn.
4. **Ergonomic API.** The server derives `from` automatically from the request's existing mandatory identity fields (`host`, `project_dir`, `session_id`) so the client almost never has to construct it by hand. Explicit `from` stays available for proxying and tooling.
5. **Structured metadata.** A dedicated `meta` JSON column separates server-queryable metadata (tags, priority, correlation IDs) from opaque application payload.

## 2. Non-goals

- Cross-host transport, encryption, push delivery, fan-out, or real-time delivery — mailbox remains polled, local-only, per the v0.5 and v0.7 design intent.
- Auth / access control on mailbox methods — continues to piggyback on stele's existing `X-Stele-Key` middleware.
- Indexed full-text search across `meta` or `payload`.
- Persistent at-least-once delivery guarantees across server restarts beyond what SQLite already provides.
- Migration of v0.7.0 mailbox rows into the v0.8.0 schema (see §9).

## 3. Background & motivation

### 3.1 Current state (v0.7.0)

`steop_mailbox` table (DESIGN.md §5.4) has **six identity columns** plus an envelope:

```
from_host, from_project_dir, from_session_id,
to_host, to_project_dir, to_session_id,
kind, subject, payload, created_at, acked_at
```

Addressing rules: sender is always a session (3-tuple); recipient is either a project (`to_session_id=''`) or a session. RPC surface uses composite `from_id` / `to_id` string fields on the wire but the table columns stay split. Ack is a nullable timestamp.

### 3.2 Pain points

| # | Pain point                                                                                                | v2 remedy                                                           |
| - | --------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------- |
| 1 | 6 identity columns for a concept that's already a single string on the wire                              | One `from`, one `to`                                                |
| 2 | `acked_at NULL` is an implicit two-state lifecycle with no room for "read but not archived"              | Explicit `status NEW\|READ\|ARCHIVE` column                         |
| 3 | No way to address a named user — sessions are UUIDs, projects are directories, users have no principal   | New `HOST:PROJECT_DIR:USER` form in the composite grammar           |
| 4 | No structured metadata surface — callers overload `payload` for both opaque data and filter-worthy hints | New `meta` JSON column for server-queryable structured metadata     |
| 5 | Client has to construct `from_id` by hand on every send, even when the server already knows the identity | Server derives `from` from mandatory request identity fields        |
| 6 | `kind` vocabulary (`HOOK:Stop` etc.) is fine but the field name `kind` is vague                          | Rename to `message_type` (same vocabulary, clearer name)            |

## 4. Schema

### 4.1 Columns

| Column         | Type            | Nullable | Default    | Notes                                                             |
| -------------- | --------------- | -------- | ---------- | ----------------------------------------------------------------- |
| `message_id`   | INTEGER PK      | no       | autoincr   | Row PK. Always an integer. Exposed on the wire as `message_id`.   |
| `from`         | TEXT            | no       | —          | Composite identifier (see §5). Always 2- or 3-segment.            |
| `to`           | TEXT            | no       | —          | Composite identifier. Always 2- or 3-segment.                     |
| `subject`      | TEXT            | no       | `''`       | One-line human summary. Empty string allowed.                     |
| `message_type` | TEXT            | no       | `'NOTE'`   | Structured type (see §4.3). Replaces v0.7 `kind`.                 |
| `meta`         | TEXT (JSON)     | no       | `'{}'`     | Server-queryable structured metadata. JSON object.                |
| `payload`      | TEXT (JSON)     | no       | `'{}'`     | Opaque application payload. JSON value (object, array, scalar).   |
| `created_at`   | TEXT (RFC3339)  | no       | —          | Set by server at insert.                                          |
| `status`       | TEXT            | no       | `'NEW'`    | One of `NEW`, `READ`, `ARCHIVE`. See §6.                          |

**Naming convention — `id` vs `message_id`.** The PRD follows one strict rule to avoid overloading:

- **`id`** (on the RPC wire, request body only) is **always** a composite identifier **string** — the caller's own `HOST:PROJECT_DIR[:SESSION_UUID|:USER]`. Same convention as every other steop RPC method since v0.7.0.
- **`message_id`** is **always** an **integer** row PK of `steop_mailbox`. Appears in schema, in request bodies of per-message methods (`mailbox.get`, `mailbox.read`, `mailbox.archive`), and in every response that returns a row.

Never conflate the two. A request body may carry both (`id` = who I am, `message_id` = which row I'm acting on).

### 4.2 Indexes

- Primary: `(message_id)` — autoincr.
- Recipient lookup: `(to, status, created_at)` — drives `mailbox.list` in O(log n) for the common "show me NEW messages for this principal" query.
- Sender lookup (outbox view, future): `(from, created_at)` — cheap to add, useful for audit.

### 4.3 `message_type` vocabulary

Unchanged from v0.7 `kind`. Reserved namespaces:

- `HOOK:*` — hook-originated messages (`HOOK:Stop`, `HOOK:SessionEnd`, `HOOK:PreCompact`, …).
- `TASK:*` — skill or agent task-related messages (`TASK:Result`, `TASK:Progress`).
- `NOTE:*` — human or skill notes (`NOTE:INFO`, `NOTE:WARN`).
- `CHAT:MESSAGE` — direct session-to-session chat.
- `LEGACY:UNKNOWN` — migrated-but-uncategorized (dead under v0.8 because we drop-and-recreate, retained for reference only).

The server does not enforce the vocabulary; it is convention.

### 4.4 `meta` vs `payload`

| Field     | Purpose                                                | Examples                                                                  |
| --------- | ------------------------------------------------------ | ------------------------------------------------------------------------- |
| `meta`    | Metadata the **server or other callers** may filter on | `{"priority":"high","tags":["build","failed"],"correlation_id":"abc123"}` |
| `payload` | Opaque application data                                | Hook output, task result JSON, the actual message the caller wants to send |

Rule of thumb: if a caller might ever `WHERE` on it or another recipient might want to decide whether to read based on it, it goes in `meta`. If it's only meaningful to the final consumer, it goes in `payload`.

## 5. Composite identifier grammar

### 5.1 Shapes

A `from` or `to` value is **always** one of exactly three shapes:

```
HOST:PROJECT_DIR                          # project-level principal
HOST:PROJECT_DIR:<session_uuid>           # session-level principal
HOST:PROJECT_DIR:USER                     # user-level principal (literal "USER")
```

Segment definitions:

- `HOST` — alphanumerics, dots, dashes. No colons. Sanitized client-side (`:` replaced with `-`).
- `PROJECT_DIR` — any path. May contain `:` internally; the parser's rightmost-split rule handles this safely (see §5.2).
- `<session_uuid>` — Claude Code session UUID. 36 chars, regex `^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$`.
- `USER` — the **literal constant string `USER`** (four uppercase ASCII characters). It is **not** a placeholder for a named user. There is exactly one user principal per `HOST:PROJECT_DIR` — the singleton human operating that project on that host. Multi-user systems disambiguate at the `HOST` level (different OS users produce different `HOST` values via `STELE_HOST` / `os.Hostname()` or per-user config profiles), not by encoding a name in this segment.

Rationale for the singleton USER design: mailbox v2 is intentionally simple. A named-user directory (`alice`, `bob`, …) would require a user registry, name validation, and identity mapping — none of which steop has or wants. The singleton `USER` principal just says "this message is addressed to the human, not to a session or to project shared state".

### 5.2 Parser rule (disambiguation)

The 3rd segment has **exactly two valid values**: a session UUID or the literal string `USER`. Anything else is a parse error.

```
Input: host:rest

1. If rest has no ':'                      → project-level (2-segment). project_dir = rest. END.
2. Find the LAST ':' in rest. Let tail be everything after it.
3. If tail matches UUID regex              → session-level. project_dir = before-of-last-colon, session_uuid = tail. END.
4. If tail == "USER" (exact match)         → user-level.    project_dir = before-of-last-colon, user principal. END.
5. Otherwise                               → 400 "id 3rd segment must be a session UUID or the literal 'USER'"
```

Examples:

- `laptop:/tmp/demo` → project-level, `project_dir = /tmp/demo`
- `laptop:/tmp/demo:0d3f4000-8000-4000-8000-000000000001` → session-level
- `laptop:/tmp/demo:USER` → user-level
- `laptop:/tmp/demo:alice` → **400** (not a UUID, not the literal `USER`)
- `laptop:/tmp/demo:user` → **400** (case-sensitive; must be `USER` uppercase)
- `laptop:/tmp/demo:USER:trailing` → parser splits on last `:`, so `project_dir = /tmp/demo:USER`, tail = `trailing` → **400** (not UUID, not `USER`). This is an unusual path containing `:USER:` but is handled correctly by the rightmost-colon rule.

This rule is **proposed**, not decided. See Open Question #6.

### 5.3 Error cases

- Empty host segment → 400 `"id host segment is empty"`
- Empty project_dir segment → 400 `"id project_dir segment is empty"`
- Empty 3rd segment (trailing `:`) → 400 `"id 3rd segment is empty"`
- Missing `:` entirely → 400 `"id missing ':' separator"`
- 3rd segment is neither a UUID nor the literal `USER` → 400 `"id 3rd segment must be a session UUID or the literal 'USER'"`

The last case is new in v0.8 — v0.7 accepted any 3-segment form as session because there was no USER principal and therefore no need to validate the third segment's contents. v0.8 tightens the parser to reject unknown 3rd-segment shapes.

## 6. Status state machine

```
        send                 mailbox.read                 mailbox.archive
(none) ──────► NEW ──────────────────────► READ ─────────────────────────► ARCHIVE
                │                                                                ▲
                └────────────── mailbox.archive ────────────────────────────────┘
```

- **`NEW`** — default on insert. Visible to `mailbox.list` by default.
- **`READ`** — set by explicit `mailbox.read {id}` call. Recipient acknowledges they have processed the message. Still visible to `mailbox.list` if caller opts in.
- **`ARCHIVE`** — set by explicit `mailbox.archive {id}` call. Hidden from `mailbox.list` by default. Rows are retained forever (see Open Question #4).

**Transition rules:**

- Only `NEW → READ`, `NEW → ARCHIVE`, and `READ → ARCHIVE` are legal.
- No `READ → NEW` (unread). Caller can create a new message if they want to "re-raise" something.
- No `ARCHIVE → READ/NEW` unarchive (v0.8.0 scope; may revisit in v0.9).
- Illegal transition → 400 `"invalid mailbox status transition: X → Y"`.

**Implicit transitions:** none. `mailbox.list` does **not** flip status. The list call is idempotent and audit-friendly. This is a conscious departure from POP-style "fetching = reading" semantics. See Open Question #2.

## 7. RPC surface

All methods are `POST /api/v1/steop/steop.mailbox.<verb>` with a JSON body, consistent with v0.7.

### 7.1 Mandatory common API fields

Every steop RPC request — not just mailbox — carries a **mandatory** `id` field at the top level of the request body. This is the v0.7.0 baseline contract and mailbox v2 inherits it unchanged:

| Field | Required | Type   | Meaning                                                          |
| ----- | -------- | ------ | ---------------------------------------------------------------- |
| `id`  | **yes**  | string | Caller's composite identifier — `HOST:PROJECT_DIR[:SESSION_UUID\|:USER]` |

Rules:

- The server rejects any steop RPC request missing `id` with `400 "id: missing"`.
- `id` must be a valid 2-segment or 3-segment composite identifier (see §5) or the server rejects with `400 "id: <parse error>"`.
- No steop request carries separate `host` / `project_dir` / `session_id` fields — those were collapsed into the composite `id` during the v0.7.0 rewrite and MUST NOT be reintroduced by v0.8.
- `id` is always **who the caller is**, never "which row I'm acting on" — for per-message operations use `message_id` (see §4.1 naming convention).

**Implicit `from` derivation.** Because every mailbox.send request is already guaranteed to carry the caller's full identity in `id`, the server derives the `from` field automatically when the caller does not specify it explicitly:

- Request `id` is 3-segment session form → implicit `from = <that id>`
- Request `id` is 2-segment project form → implicit `from = <that id>` (project-as-sender)
- Explicit `from` in the body overrides the implicit value

Rationale: 95% of mailbox sends originate from hooks that already identify themselves fully. Forcing the hook to construct `from` manually is needless ceremony. The baseline `id` field is enough to construct `from` without any extra client code.

The same baseline `id` is also used to derive the implicit recipient filter on `mailbox.list` (defaults to the caller's own inbox — see §7.2).

### 7.2 Methods

#### `steop.mailbox.send`

| Field          | Required | Type    | Notes                                                                                |
| -------------- | -------- | ------- | ------------------------------------------------------------------------------------ |
| `id`           | yes      | string  | Caller's own composite identifier. Used to derive implicit `from`.                   |
| `to`           | yes      | string  | Recipient's composite identifier (any of the three shapes).                          |
| `from`         | no       | string  | Override. If omitted, server derives from `id`.                                      |
| `subject`      | yes      | string  | May be empty string.                                                                 |
| `message_type` | no       | string  | Default `'NOTE'`.                                                                    |
| `meta`         | no       | object  | Default `{}`.                                                                        |
| `payload`      | no       | any     | Default `{}`.                                                                        |

Returns: `{ "message_id": <int>, "from": "...", "to": "...", "created_at": "..." }`.

#### `steop.mailbox.list`

| Field            | Required | Type              | Notes                                                                     |
| ---------------- | -------- | ----------------- | ------------------------------------------------------------------------- |
| `id`             | yes      | string            | Caller's own composite identifier. Used as implicit recipient filter if `to` is omitted. |
| `to`             | no       | string            | Recipient filter. Defaults to caller's `id` (the inbox view).             |
| `status`         | no       | string or array   | One or more of `NEW \| READ \| ARCHIVE`. Default `["NEW"]`.               |
| `message_type`   | no       | string            | Optional filter (exact match or prefix like `HOOK:*`).                    |
| `limit`          | no       | integer           | Default 200, max 1000.                                                    |

Returns: `{ "messages": [MailboxRow, ...] }`, ordered by `created_at ASC` (FIFO).

#### `steop.mailbox.get`

| Field        | Required | Type    | Notes                                |
| ------------ | -------- | ------- | ------------------------------------ |
| `id`         | yes      | string  | Caller's composite identifier.       |
| `message_id` | yes      | integer | Row PK of the message to fetch.      |

Returns: full `MailboxRow` (see §7.3). 404 if not found. **Does not** transition status — see §6 "Implicit transitions".

#### `steop.mailbox.read`

| Field        | Required | Type    | Notes                                 |
| ------------ | -------- | ------- | ------------------------------------- |
| `id`         | yes      | string  | Caller's composite identifier.        |
| `message_id` | yes      | integer | Row PK of the message to mark as read.|

Returns: `{ "message_id": <int>, "status": "READ" }`. 409 if current status is not `NEW`.

#### `steop.mailbox.archive`

| Field        | Required | Type    | Notes                                 |
| ------------ | -------- | ------- | ------------------------------------- |
| `id`         | yes      | string  | Caller's composite identifier.        |
| `message_id` | yes      | integer | Row PK of the message to archive.     |

Returns: `{ "message_id": <int>, "status": "ARCHIVE" }`. 409 if current status is already `ARCHIVE`.

#### Removed

- `steop.mailbox.ack` — replaced by the explicit `read` + `archive` pair.

### 7.3 Response type

```json
// MailboxRow
{
  "message_id":    1234,
  "from":          "laptop:/tmp/demo:0d3f...uuid",
  "to":            "laptop:/tmp/demo",
  "subject":       "demo message",
  "message_type":  "NOTE:INFO",
  "meta":          { "tags": ["demo"], "priority": "low" },
  "payload":       { "phase": "validate", "tool_calls": 42 },
  "created_at":    "2026-04-12T10:30:00Z",
  "status":        "NEW"
}
```

Note: `message_id` is the row PK (integer). `from` and `to` are composite identifier strings. A `MailboxRow` never has a bare `id` field — the naming convention from §4.1 is strictly enforced on the wire too.

## 8. API ergonomics examples

### 8.1 Hook posts a project-level summary (implicit `from`)

```json
POST /api/v1/steop/steop.mailbox.send
{
  "id":           "laptop:/tmp/demo:0d3f-4000-8000-000000000001",
  "to":           "laptop:/tmp/demo",
  "subject":      "session ended: 42 tool calls",
  "message_type": "HOOK:Stop",
  "payload":      { "last_tool": "Bash", "tool_calls": 42 }
}
```

Server inserts a row with `from = "laptop:/tmp/demo:0d3f-4000-8000-000000000001"`.

### 8.2 Drain inbox (implicit `to`)

```json
POST /api/v1/steop/steop.mailbox.list
{
  "id": "laptop:/tmp/demo"
}
```

Returns all `NEW` messages addressed to `laptop:/tmp/demo` (project-level inbox).

### 8.3 Explicit `from` override (tool proxying on behalf of another sender)

```json
POST /api/v1/steop/steop.mailbox.send
{
  "id":    "laptop:/tmp/demo:0d3f-4000-8000-000000000001",
  "from":  "laptop:/tmp/demo:alice",
  "to":    "laptop:/tmp/demo:bob",
  "subject": "user-to-user message",
  "message_type": "CHAT:MESSAGE",
  "payload": { "text": "hey" }
}
```

`from` is explicit so the server does not derive it. Used by CLI tooling acting on behalf of a named user.

## 9. Migration from v0.7.0

### 9.1 Chosen strategy: drop-and-recreate

- `steop_mailbox` table is dropped and recreated with the v0.8 schema on first server boot of v0.8.0.
- **No data preservation.** Any pending v0.7 messages are lost on upgrade.
- Rationale: mailbox is an ephemeral inter-session messaging channel, not long-term state. v0.6 → v0.7 already took a hard break under pre-1.0 SemVer. A migration script would be ~200 lines of one-off Rust for data that has a natural half-life of hours and is easily regenerated by hook re-firing on the next session.

### 9.2 Rejected alternative: in-place migration

- Alternative: `ALTER TABLE` or `INSERT INTO ... SELECT FROM` to carry v0.7 rows forward.
- Rejected because: (a) 6 identity columns → 2 composite strings is non-trivial SQL; (b) `acked_at` → `status` has no natural mapping for "read but not archived"; (c) breakage risk far outweighs benefit; (d) the user's previous turn in this session confirmed a willingness to manually clear the mailbox, which implies low data-preservation expectation.

### 9.3 No `migration subsystem` is added

v0.8.0 continues the v0.7.0 stance: there is no migration framework. Breaking schema changes drop and recreate. This stays until v1.0.0 at the earliest.

### 9.4 RPC wire-format renames (client-breaking)

v0.8.0 also renames JSON fields on the RPC surface, independent of the schema:

| v0.7.0 field (on the wire)  | v0.8.0 field | Notes                                      |
| --------------------------- | ------------ | ------------------------------------------ |
| `from_id`                   | `from`       | `mailbox.send` request body + response     |
| `to_id`                     | `to`         | `mailbox.send`, `mailbox.list` request + response |
| `kind`                      | `message_type` | `mailbox.send` request + response        |
| `message_id` (on `ack`)     | removed      | `mailbox.ack` replaced by `read` + `archive` with `message_id` on both |
| `acked_at` (response only)  | removed      | Lifecycle tracked via `status` column      |

All Go client structs (`MailboxMessage` fields `FromID`/`ToID`/`Kind`) must be renamed in lock-step. Every hook call site that constructs request bodies must be updated. Smoke-tests.md mailbox section must be rewritten.

## 10. Open questions

These MUST be resolved before kicking off the implementation `/steop:st-flow`. Each has a recommendation but is not decided.

1. **`READ` vs `READED` spelling.** Raw design used `READED` (likely transcription). PRD uses `READ`. **Confirm** the correction.
2. **`NEW → READ` transition trigger.** PRD recommends explicit only (`mailbox.read` RPC). Alternative: implicit on `mailbox.get`. **Confirm** explicit.
3. **`READ → ARCHIVE` transition.** PRD recommends explicit only (`mailbox.archive` RPC). No implicit path. **Confirm.**
4. **Archived retention.** PRD recommends keep-forever (v0.7 precedent). Alternatives: TTL column, manual purge RPC. **Decide.**
5. **~~`USER` resolution.~~** **Resolved:** `USER` is the literal constant string `USER`, not a named user. One singleton user principal per `HOST:PROJECT_DIR`. Multi-user systems disambiguate at the `HOST` level. No `$USER` env, no OS user lookup, no Claude Code account. See §5.1.
6. **Parser 3rd-segment rule.** PRD enforces a closed set: 3rd segment is either a session UUID or the literal string `USER` (case-sensitive). Anything else is a 400 parse error. **Confirm** — alternative is to also reject by requiring an explicit prefix like `:s:<uuid>` / `:u:USER`, but that adds ceremony without clarity since the set is closed.
7. **Can non-session principals send?** PRD recommends yes (project- and user-level senders allowed). v0.7 forbade non-session senders. **Confirm** the relaxation.
8. **`message_type` reserved vocabulary and namespaces.** PRD proposes keeping v0.7's `HOOK:*` vocabulary unchanged AND carrying forward the existing `TASK:*`, `NOTE:*`, `CHAT:MESSAGE` reserved namespaces that were already documented in v0.7 §5.4 envelope notes. **Confirm** (a) no renames of existing values, and (b) whether these reserved namespaces should stay reserved or be dropped in v0.8 (the user's raw design did not list them explicitly).
9. **`mailbox.list` default status filter.** PRD recommends `["NEW"]`. Alternatives: `["NEW","READ"]`, all-with-explicit-filter. **Decide.**
10. **`meta` indexing.** PRD recommends plain JSON blob in v0.8.0; promote common keys (priority, correlation_id) to columns in v0.9+ if query patterns emerge. **Confirm.**
11. **RPC method name reuse.** PRD reuses `steop.mailbox.send` / `list` / `read` / `archive` — breaking the shape, not the names. Alternative: version the names (`steop.mailbox.v2.send`) so old clients error with "method not found" instead of "bad request". **Decide.**
12. **`mailbox.get` should-transition debate.** PRD keeps `get` side-effect-free. Alternative: `get` flips `NEW → READ` implicitly. **Confirm** side-effect-free.
13. **Bulk operations.** Not in PRD scope. Should v0.8 also add `mailbox.read_all` / `mailbox.archive_all` for the common "inbox zero" workflow? **Decide.** Recommendation: defer to v0.9.

14. **`USER` principals as recipients vs senders.** §5.1 allows the singleton `HOST:PROJECT_DIR:USER` for both `from` and `to`. The interesting cases:
    - `to = ...:USER` — hooks or sessions notify the human (e.g. "build failed, review this"). Strong use case.
    - `from = ...:USER` — human sends a message back, typically via a CLI command like `steop mailbox send`. Weaker use case — the human can just talk to Claude Code directly.

    Recommendation: allow both for symmetry, but expect `to = ...:USER` to be the common direction.

15. **`read` / `archive` response shape.** PRD currently has both methods echo `{"message_id": <int>, "status": "READ"|"ARCHIVE"}` for audit-logging convenience. **Confirm** this is the shape you want, or drop back to a bare `{"status": ...}` if the row id echo is unneeded.

## 11. Out of scope (v0.8.0)

- Cross-host transport, encryption, signing.
- Push delivery / WebSocket / SSE fan-out.
- Full-text search across subject / payload / meta.
- Unarchive (`ARCHIVE → READ`).
- Soft-delete separate from `ARCHIVE` (tombstone retention).
- Thread / reply-to chaining between messages.
- Per-recipient broadcast (multi-`to`).
- Retention TTLs / autovacuum.
- Access control / capability-based permissions on mailbox principals.
- Auditing beyond `created_at` + `status` (no `read_at`, `archived_at` timestamps — add in v0.9 if demanded).

## 12. Implementation notes (for the future execute cycle, not decided here)

- Rust: `SteopMailboxRow` struct reshape in `db.rs`; `steop_api.rs` handlers for `send` / `list` / `get` / `read` / `archive`; drop `mailbox_ack` handler and route.
- SQL: `DROP TABLE steop_mailbox; CREATE TABLE steop_mailbox (...)` in the schema-init block of `db.rs`. No `IF EXISTS` preservation.
- Go client: `internal/client/mailbox.go` reshape — new method names, new response struct fields, new `meta` helper type.
- Hook call sites: `internal/hooks/stop.go` and `internal/hooks/session_end.go` switch from `MailboxSendFromSelf` to the new implicit-`from` form. Both become shorter.
- Docs: rewrite `docs/steop/DESIGN.md` §5.4 and §6.1 mailbox rows; update `docs/steop/smoke-tests.md` mailbox section; update `CLAUDE.md` Steop method table.
- Version bump: v0.7.0 → v0.8.0 via `scripts/bump-version.py`.
- Smoke tests: every transition path (NEW → READ, NEW → ARCHIVE, READ → ARCHIVE) plus illegal transitions (READ → NEW should 400, ARCHIVE → READ should 400).

## 13. References

- `docs/steop/DESIGN.md` §5.4 — current `steop_mailbox` schema.
- `docs/steop/DESIGN.md` §6.1 Mailbox RPC table — current surface.
- `apps/stele/crates/stele-server/src/steop_api.rs` `parse_id` / `parse_full_id` — current composite identifier parser (to be extended with user-form branch).
- `apps/stele/crates/stele-server/src/db.rs` `SteopMailboxRow`, `steop_mailbox_send`, `steop_mailbox_list`, `steop_mailbox_ack` — current DB layer (to be rewritten).
- `apps/steop/internal/client/mailbox.go` — current Go client bindings (to be rewritten).
- `CLAUDE.md` "Steop (workflow pipeline)" section — versioning precedent.
