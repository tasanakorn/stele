# PRD — Mailbox Watcher & Task Delegation

**Status:** Draft
**Target version:** v0.9.0 (next minor — new feature surface)
**Scope:** `steop` CLI + `plugins/steop` skill
**Builds on:** [prd-001-mailbox-v2.md](prd-001-mailbox-v2.md)
**Author:** —

---

## 1. Goals

1. Define the `TASK:*` message type vocabulary and lifecycle for async task delegation between Claude Code sessions.
2. Add `steop mailbox` CLI subcommand tree (list, get, send, read, archive, watch) to expose mailbox operations from skills and shell.
3. Create `/steop:st-watch` skill that monitors a session's mailbox for incoming `TASK:REQUEST` messages and processes them.
4. Support both project-level (any watcher claims) and session-level (targeted) task addressing using the existing composite ID grammar.
5. Integrate cleanly with existing hooks (Stop, SessionEnd) for graceful task lifecycle handling on session teardown.
6. Keep the design poll-based and append-only — no schema changes to `steop_mailbox`, no server-side push.

## 2. Non-goals

- Real-time push (SSE, WebSocket) — server remains poll-based per prd-001 §2.
- Schema changes to `steop_mailbox` — the v0.8.0 schema is sufficient.
- Task queue guarantees beyond at-most-once delivery.
- Cross-project task delegation (tasks stay within one `host:project_dir`).
- Task priority or scheduling — all tasks are FIFO within a mailbox.
- Persistent watcher daemon outside Claude Code sessions.
- Task cancellation (sender-initiated abort) — deferred to future PRD.

## 3. Background & Motivation

### 3.1 Current state

prd-001 (v0.8.0) established the mailbox with a unified `from`/`to` composite identifier grammar, a three-state `NEW → READ → ARCHIVE` status machine, and a clean RPC surface (`mailbox.send`, `mailbox.list`, `mailbox.get`, `mailbox.read`, `mailbox.archive`). Hooks currently write `HOOK:Stop` and `HOOK:SessionEnd` messages to the project inbox on every session teardown. The `TASK:*` namespace is reserved in §4.3 of prd-001 but has no defined vocabulary — the names `TASK:Result` and `TASK:Progress` were listed as placeholders only. No session actively listens for incoming messages; the mailbox is write-only from the sessions' perspective.

### 3.2 Pain points

| # | Pain point                                                  | Remedy                                               |
| - | ----------------------------------------------------------- | ---------------------------------------------------- |
| 1 | No way to delegate work from one session to another         | `TASK:REQUEST` message type + watcher skill          |
| 2 | No CLI access to mailbox operations                         | `steop mailbox` subcommand tree                      |
| 3 | `TASK:*` namespace reserved but undefined                   | Formal vocabulary in §4                              |
| 4 | No task lifecycle tracking (started? done? failed?)         | Lifecycle state machine via message types (§5)       |
| 5 | Mailbox messages accumulate unread with no consumer         | Watcher skill actively polls and processes           |

## 4. Task Message Type Vocabulary

The following message types define the `TASK:*` namespace for v0.9.0. The server does not enforce this vocabulary — it is a convention between senders and watchers, consistent with the approach established in prd-001 §4.3.

| `message_type`  | Direction         | Purpose                          | Required `meta` fields                                            | `payload` structure       |
| --------------- | ----------------- | -------------------------------- | ----------------------------------------------------------------- | ------------------------- |
| `TASK:REQUEST`  | sender → watcher  | Initial task assignment          | `task_id` (UUID), `description` (string)                         | Free-form task parameters |
| `TASK:CHECKIN`  | watcher → sender  | Watcher claims the task          | `task_id`, `request_message_id` (int)                            | `{}`                      |
| `TASK:PROGRESS` | watcher → sender  | Optional progress update         | `task_id`                                                        | Free-form progress data   |
| `TASK:DONE`     | watcher → sender  | Task completed successfully      | `task_id`, `request_message_id`                                  | Task result               |
| `TASK:FAILED`   | watcher → sender  | Task failed                      | `task_id`, `request_message_id`, `error` (string)                | Error details             |

**`task_id`** is a client-generated UUID that correlates all messages for one logical task across the full REQUEST → CHECKIN → PROGRESS → DONE/FAILED arc. The sender generates it before calling `mailbox.send`; the watcher echoes it on every response message.

**`request_message_id`** is the `message_id` (integer PK) of the original `TASK:REQUEST` row in `steop_mailbox`. It provides an unambiguous pointer back to the triggering message and allows the sender to correlate CHECKIN/DONE/FAILED responses to the exact row they sent even if they issued multiple requests with the same `task_id` (which they should not, but the field defends against accidents).

**`description`** in `meta` is a human-readable summary of what the task asks for. It is the primary field the watcher uses to decide how to process the task; the `payload` carries any additional structured parameters.

**`payload`** is opaque to the server — its structure is a convention between sender and watcher. A simple text description task might use `{}`. A task that points to files might use `{"files":["src/auth.rs"],"context":"..."}`. The watcher is responsible for interpreting it.

## 5. Task Lifecycle State Machine

```
                  mailbox.read              mailbox.send             mailbox.send (0..N)
TASK:REQUEST ────────────────► [claimed] ────────────────► TASK:CHECKIN ──────────────► TASK:PROGRESS
    (NEW)         (NEW → READ)                                                               │
                                                                                             │
                                                             mailbox.send                    │
                                                   ┌─────────────────────────────────────────┘
                                                   ▼
                                          TASK:DONE  or  TASK:FAILED
                                                   │
                                             mailbox.archive
                                                   ▼
                                      (original REQUEST → ARCHIVE)
```

**Transition rules:**

- Watcher discovers a `TASK:REQUEST` via `mailbox.list --type TASK:REQUEST --status NEW`.
- Watcher calls `mailbox.read <message_id>` to claim — transitions the original message NEW → READ.
- If `mailbox.read` returns 409: another watcher claimed it first — skip silently and continue polling.
- Watcher sends `TASK:CHECKIN` back to the `from` of the original REQUEST (new mailbox row; the original REQUEST row is not modified).
- Watcher optionally sends `TASK:PROGRESS` messages during execution (each is a new mailbox row addressed to the original sender).
- On completion: watcher sends `TASK:DONE` or `TASK:FAILED` (new mailbox row addressed to the original sender).
- Watcher calls `mailbox.archive <message_id>` on the original REQUEST — transitions READ → ARCHIVE.
- All CHECKIN/PROGRESS/DONE/FAILED messages are independent rows in `steop_mailbox` addressed to the original sender's `from` identifier.

**Key invariant:** the original `TASK:REQUEST` message transitions NEW → READ → ARCHIVE. All other lifecycle messages are new rows flowing in the reverse direction (watcher → sender). The original REQUEST row is never modified except for its `status` field.

## 6. Addressing Semantics

### 6.1 Project-level addressing

`to: "host:project_dir"` (2-segment composite identifier):

- Any watcher polling the project inbox can discover and claim the task.
- "First to `mailbox.read` wins" — at-most-once delivery is enforced by the 409 conflict on duplicate `mailbox.read` calls.
- Use case: "I need help with X, any available watcher in this project can take it."

### 6.2 Session-level addressing

`to: "host:project_dir:uuid"` (3-segment composite identifier):

- Only the specific session whose UUID matches the `to` field will see and claim the task. The session polls its own inbox, which is filtered to its full 3-segment composite identifier.
- Use case: "Session ABC, please do X" — targeted delegation where the sender knows which session should handle the work.
- Sender must know the target session's UUID. This is discoverable via `steop monitor` output or `steop session list` (future).

### 6.3 Reply addressing convention

The watcher always sends CHECKIN/PROGRESS/DONE/FAILED to the `from` field of the original `TASK:REQUEST` row. Because prd-001 §7.1 guarantees that `from` is always populated (either explicitly by the sender or derived by the server from the sender's `id`), the watcher never needs to construct the reply address by hand — it reads it directly from the received message.

This creates a natural request-response pattern: the sender's identity is embedded in the message it sent, and the watcher reflects it back as the reply target. No out-of-band address exchange is needed.

## 7. CLI Surface: `steop mailbox`

A new `mailbox` subcommand is registered in `main.go` and dispatched via `cmd_mailbox.go`. It groups six sub-subcommands covering all mailbox operations.

### `steop mailbox list`

```
steop mailbox list [--type TYPE] [--status STATUS] [--limit N] [--json]
```

- Lists messages in the caller's inbox (default: `to` = caller's session composite ID derived from the configured profile).
- `--type`: filter by `message_type`. Accepts prefix match (e.g. `TASK:` matches all task types). Optional.
- `--status`: filter by status. Default: `NEW`. Accepts `NEW`, `READ`, `ARCHIVE`, or `ALL`.
- `--limit`: maximum messages to return. Default: 20.
- `--json`: output as a JSON array. Default: human-readable table.
- Calls `client.MailboxList()` internally.

### `steop mailbox get <message_id>`

```
steop mailbox get <message_id> [--json]
```

- Fetches a single message by its integer PK. Side-effect free — does not transition status (consistent with prd-001 §6 "Implicit transitions: none").
- `<message_id>` is a required positional argument (integer).
- Calls `client.MailboxGet()`.

### `steop mailbox send`

```
steop mailbox send --to TO [--type TYPE] [--subject SUBJECT] [--meta JSON] [--payload JSON]
```

- Sends a message. `--to` is required; all other flags are optional.
- `--type`: `message_type` value. Default: `NOTE`.
- `--subject`: one-line human summary. Default: empty string.
- `--meta`: JSON object string for structured metadata. Default: `{}`.
- `--payload`: JSON value string for opaque application payload. Default: `{}`.
- Calls `client.MailboxSend()`.

### `steop mailbox read <message_id>`

```
steop mailbox read <message_id>
```

- Marks message as READ (NEW → READ transition). Returns the updated status.
- Returns 409 if the message is not currently in `NEW` status (already claimed by another watcher).
- Calls `client.MailboxRead()`.

### `steop mailbox archive <message_id>`

```
steop mailbox archive <message_id>
```

- Marks message as ARCHIVE. Returns the updated status.
- Returns 409 if the message is already in `ARCHIVE` status.
- Calls `client.MailboxArchive()`.

### `steop mailbox watch`

```
steop mailbox watch [--type TYPE] [--interval SECONDS] [--json]
```

This is the key primitive for the watcher skill. It is a long-running poll loop designed for consumption by Claude Code's Monitor tool.

**Behavior:**

- Polls `mailbox.list` at the specified interval (default: 10s).
- Tracks all seen `message_id` values in an in-memory set (map[int]bool) to emit only genuinely new messages since the process started.
- Each new message is printed as a single JSON object on its own line to stdout (newline-delimited JSON — one complete object per line).
- `--type`: filter passed through to `mailbox.list`. Recommended value for watcher use: `TASK:REQUEST`.
- `--interval`: polling interval in seconds. Default: 10. Minimum: 2. Maximum: 300.
- `--json` is implied — the output is always JSON lines regardless of whether the flag is passed.
- Exits cleanly on SIGINT or SIGTERM.
- Does NOT call `mailbox.read` — that is the consumer's responsibility. Separation of concerns: `watch` discovers and reports; the consumer (the skill) decides whether and how to claim.

**Example output line:**

```jsonl
{"message_id":42,"from":"macbook:myproject:abc-123","to":"macbook:myproject","subject":"Refactor auth module","message_type":"TASK:REQUEST","meta":{"task_id":"d4e5f6a7-1234-5678-9abc-def012345678","description":"Refactor auth module to use JWT"},"payload":{"files":["src/auth.rs"],"instructions":"..."},"created_at":"2026-04-12T10:30:00Z","status":"NEW"}
```

Each line is a complete `MailboxRow` JSON object. The consumer can parse it with `JSON.parse()` or equivalent.

## 8. Skill Design: `/steop:st-watch`

**Skill metadata:**

- Name: `st-watch`
- Description: Monitor mailbox for task requests and process them autonomously
- Location: `plugins/steop/skills/st-watch/SKILL.md`

**Frontmatter:**

```yaml
---
name: st-watch
description: Monitor mailbox for incoming task requests and process them autonomously. Starts a polling loop that watches for TASK:REQUEST messages, claims them, and executes them via st-flow.
---
```

**Skill behavior:**

The skill instructs Claude Code to perform the following steps:

**Step 1 — Start the watcher process.**

```bash
steop mailbox watch --type TASK:REQUEST --interval 10
```

Run this as a background Bash process with `run_in_background: true`. The process will run until the session ends.

**Step 2 — Monitor for incoming tasks.**

Use the Claude Code Monitor tool to stream stdout lines from the background process. Each line is a complete JSON object representing a new `TASK:REQUEST` message that has appeared in the inbox since the watcher started.

**Step 3 — On receiving a TASK:REQUEST line.**

a. Parse the JSON line to extract `message_id`, `from`, `meta.task_id`, `meta.description`, and `payload`.

b. Claim the task:
```bash
steop mailbox read <message_id>
```
If the response is HTTP 409, another watcher claimed it first — skip this message and continue monitoring.

c. Send CHECKIN to the original sender:
```bash
steop mailbox send \
  --to <from> \
  --type TASK:CHECKIN \
  --meta '{"task_id":"<task_id>","request_message_id":<message_id>}'
```

d. Process the task. Evaluate `meta.description` and `payload` to determine the work to be done. Default behavior: launch `/steop:st-flow` with `meta.description` as the prompt and `payload` as additional context.

e. On success — send DONE:
```bash
steop mailbox send \
  --to <from> \
  --type TASK:DONE \
  --subject "Completed: <description>" \
  --meta '{"task_id":"<task_id>","request_message_id":<message_id>}' \
  --payload '<result_json>'
```

f. On failure — send FAILED:
```bash
steop mailbox send \
  --to <from> \
  --type TASK:FAILED \
  --subject "Failed: <description>" \
  --meta '{"task_id":"<task_id>","request_message_id":<message_id>,"error":"<error_summary>"}' \
  --payload '<error_details_json>'
```

g. Archive the original REQUEST:
```bash
steop mailbox archive <message_id>
```

**Step 4 — Persist active tasks.**

After claiming a task (step 3b), store the `task_id` in session storage so the SessionEnd hook can issue FAILED messages for any tasks that were in-progress when the session was torn down:

```bash
steop storage put --key watcher:active_tasks --value '<updated_json_array>'
```

Read the current value first (`steop storage get --key watcher:active_tasks`), append the new `task_id`, then write the updated array back.

Remove the `task_id` from the array after archiving (step 3g).

**Step 5 — Serial processing.**

Process one task at a time. Do not claim a new `TASK:REQUEST` from the Monitor stream while a task is currently being processed. Buffer incoming Monitor events and process them sequentially after the current task completes.

## 9. Integration with Existing Hooks

### 9.1 Stop hook (`HOOK:Stop`)

The Stop hook fires when the user presses Ctrl+C or the session is otherwise interrupted (defined in `plugins/steop/hooks/hooks.json` as the `Stop` event). After the existing `HOOK:Stop` message is sent to the project inbox, the Stop handler should:

1. Call `steop storage get --key watcher:active_tasks` to retrieve any tasks the session's watcher had claimed.
2. For each `task_id` in the array: call `steop storage get` to retrieve the corresponding `request_message_id` and `from`, then send `TASK:FAILED` with `error: "session stopped before task completed"`.
3. Clear the `watcher:active_tasks` key in session storage.

**Implementation:** add the cleanup block in `apps/steop/internal/hooks/stop.go` after the existing `HOOK:Stop` message send.

### 9.2 SessionEnd hook (`HOOK:SessionEnd`)

The SessionEnd hook fires when the session terminates normally. Same cleanup logic as the Stop hook:

1. Check `watcher:active_tasks` in session storage.
2. Send `TASK:FAILED` for each uncompleted task.
3. Clear the storage key.

**Implementation:** add the cleanup block in `apps/steop/internal/hooks/session_end.go`. This is best-effort — the hook has a 30-second timeout per the existing steop hook contract.

### 9.3 PreCompact hook

The PreCompact hook currently sends no mailbox message. It could potentially notify the sender that the watcher is about to context-compact and may lose local state, but this use case is out of scope for v0.9.0. Defer to a future PRD.

## 10. API Ergonomics Examples

### Example 1: Session A delegates a task to the project inbox

```bash
# Session A sends a task request to the shared project inbox (2-segment to = any watcher can claim)
steop mailbox send \
  --to "macbook:myproject" \
  --type TASK:REQUEST \
  --subject "Refactor auth module" \
  --meta '{"task_id":"d4e5f6a7-1234-5678-9abc-def012345678","description":"Refactor the auth module in src/auth.rs to use JWT tokens instead of session cookies"}' \
  --payload '{"files":["src/auth.rs","src/middleware.rs"],"context":"We are migrating from cookie-based sessions to JWT"}'
```

### Example 2: Watcher session B claims and processes the task

```bash
# Watcher sees the task via `steop mailbox watch` output (one JSON line per new message)

# Watcher claims it (NEW → READ):
steop mailbox read 42
# → {"message_id":42,"status":"READ"}

# Watcher sends CHECKIN to the original sender:
steop mailbox send \
  --to "macbook:myproject:abc-sender-uuid" \
  --type TASK:CHECKIN \
  --meta '{"task_id":"d4e5f6a7-1234-5678-9abc-def012345678","request_message_id":42}'

# ... watcher processes the task via st-flow ...

# Watcher sends DONE:
steop mailbox send \
  --to "macbook:myproject:abc-sender-uuid" \
  --type TASK:DONE \
  --subject "Completed: Refactor auth module" \
  --meta '{"task_id":"d4e5f6a7-1234-5678-9abc-def012345678","request_message_id":42}' \
  --payload '{"files_modified":["src/auth.rs","src/middleware.rs"],"summary":"Replaced session cookies with JWT. Updated middleware to validate bearer tokens."}'

# Watcher archives the original REQUEST (READ → ARCHIVE):
steop mailbox archive 42
```

### Example 3: Targeted delegation to a specific session

```bash
# Session A knows Session B's UUID (e.g. from steop monitor output or session list)
steop mailbox send \
  --to "macbook:myproject:b-session-uuid" \
  --type TASK:REQUEST \
  --subject "Run tests for auth module" \
  --meta '{"task_id":"e5f6a7b8-5678-9abc-def0-123456789abc","description":"Run cargo test for the auth module and report results"}'
# Only session b-session-uuid will see this in its inbox — no other watcher can claim it
```

### Example 4: Race condition — two watchers compete for the same task

```bash
# Watcher B and Watcher C both poll the project inbox and see message_id 42 in their `steop mailbox watch` output

# Watcher B calls mailbox.read first:
steop mailbox read 42
# → {"message_id":42,"status":"READ"}  — success, Watcher B owns the task

# Watcher C calls mailbox.read moments later:
steop mailbox read 42
# → HTTP 409 Conflict  — Watcher C skips silently and waits for the next message
```

## 11. Open Questions

1. **Polling interval default.** Recommend 10 seconds as the default for `steop mailbox watch --interval`. Fast enough for interactive delegation, slow enough to avoid putting unnecessary load on the Stele server during quiet periods. **Confirm.**

2. **TASK:PROGRESS mandatory or optional?** Recommend optional. Short tasks (under 30 seconds) can go directly CHECKIN → DONE without emitting any PROGRESS messages. Long tasks (over 30 seconds, or tasks with meaningful discrete phases) should send PROGRESS at meaningful milestones. This is a convention enforced by the watcher implementation, not by the server. **Confirm.**

3. **Auto-launch st-flow or report-and-wait?** When the watcher receives a `TASK:REQUEST`, should it automatically launch `/steop:st-flow` with `meta.description` as the prompt, or display the task to the human and wait for explicit direction? Recommend auto-launch — the skill is named `st-watch` and is designed for autonomous operation. Add an `--interactive` flag to `steop mailbox watch` (and correspondingly to the `st-watch` skill) that disables auto-launch and instead presents the task for human approval before proceeding. **Decide.**

4. **Max concurrent tasks per watcher.** Recommend 1 (serial processing). Claude Code sessions process one conversation turn at a time; parallelism would require spawning subagents, which adds significant complexity. If a second `TASK:REQUEST` arrives while a task is in progress, the watcher should buffer it (or simply let it remain NEW in the inbox for another watcher to claim) and process it after the current task completes. Revisit in a future PRD if high-throughput task delegation becomes a use case. **Decide.**

5. **TTL for unclaimed TASK:REQUESTs.** Should unclaimed `TASK:REQUEST` messages expire after some time if no watcher is running? Recommend no TTL in v0.9.0 — senders can manually archive stale requests via `steop mailbox archive <message_id>`. Server-side TTL or expiry would require schema changes (a `ttl` column or `expires_at` column), which is explicitly out of scope per §2. **Decide.**

6. **Resume across restarts.** If the watcher process dies and the `st-watch` skill is re-invoked, should it pick up where it left off (i.e. not re-emit messages it already saw)? Recommend adding a `--since <message_id>` flag to `steop mailbox watch` that instructs the watcher to initialize its seen-set with all messages at or below the given `message_id`. The watcher persists the last-seen `message_id` in session storage (`watcher:last_message_id`) on each poll cycle. On restart, the skill reads this value and passes it as `--since`. **Decide.**

## 12. Out of Scope

- Server-side push (SSE, WebSocket) — poll-based architecture is a deliberate constraint inherited from prd-001 §2.
- Task priority or scheduling — all tasks are FIFO within the inbox as ordered by `created_at ASC`.
- Cross-project task delegation — tasks are scoped to `host:project_dir`; the composite ID grammar does not support cross-project routing.
- Persistent task queue semantics (retry, backoff, dead-letter queuing).
- Multi-watcher coordination beyond the atomic claim mechanism (no load balancing or work stealing beyond "first to `mailbox.read` wins").
- Task cancellation (sender-initiated `TASK:CANCEL`) — deferred to a future PRD.
- Server-side TTL or message expiry — would require schema changes to `steop_mailbox`.
- Watcher authentication or authorization beyond the existing `X-Stele-Key` mechanism.

## 13. Implementation Notes

The following file-by-file breakdown is for the implementation cycle. Not all items need to be done in one PR — the CLI surface and the skill are independently shippable.

- **`apps/steop/main.go`** — add `"mailbox"` case to the subcommand switch (approximately line 25). Route to new `runMailbox()` dispatcher function defined in `cmd_mailbox.go`.

- **`apps/steop/cmd_mailbox.go`** — new file. Implements `runMailbox()` that parses the sub-subcommand argument and dispatches to `runMailboxList()`, `runMailboxGet()`, `runMailboxSend()`, `runMailboxRead()`, `runMailboxArchive()`, or `runMailboxWatch()`. Each dispatch function calls the corresponding `client.Mailbox*` method from `apps/steop/internal/client/mailbox.go`.

- **`apps/steop/cmd_mailbox_watch.go`** — new file. Implements the `watch` sub-subcommand. Responsibilities: flag parsing (`--type`, `--interval`, `--since`); initialize seen-set from `--since` if provided; start `time.Ticker` at the configured interval; on each tick call `client.MailboxList()` with the type filter and `status=NEW`; compare returned `message_id` values against the seen-set; print new messages as JSON lines to stdout; update session storage with `last_message_id`; register SIGINT/SIGTERM handler for clean shutdown.

- **`plugins/steop/skills/st-watch/SKILL.md`** — new skill file with frontmatter and step-by-step instructions as specified in §8. Located alongside the existing skill directories under `plugins/steop/skills/`.

- **`plugins/steop/hooks/hooks.json`** — no changes in v0.9.0. Hook cleanup logic (the `watcher:active_tasks` drain) is best-effort and can be added as a v0.9.1 patch once the core skill is validated.

- **`apps/steop/internal/hooks/stop.go`** — add watcher cleanup block after the existing `HOOK:Stop` message send: read `watcher:active_tasks` from session storage; for each entry send `TASK:FAILED` via `client.MailboxSend()`; clear the storage key.

- **`apps/steop/internal/hooks/session_end.go`** — add the same watcher cleanup block after the existing `HOOK:SessionEnd` message send.

- **`CLAUDE.md`** — add `/steop:st-watch` to the steop skills list in the "Skills" subsection.

- **`docs/steop/DESIGN.md`** — update §6.1 (Mailbox RPC / `message_type` vocabulary) to reference the formal `TASK:*` vocabulary defined in this PRD rather than the placeholder names (`TASK:Result`, `TASK:Progress`) that were listed in v0.8.0.

## 14. References

- [prd-001-mailbox-v2.md](prd-001-mailbox-v2.md) — mailbox v2 schema, RPC surface, composite identifier grammar, and status state machine
- [docs/steop/DESIGN.md](../steop/DESIGN.md) — steop RPC surface, composite ID grammar, mailbox schema (§5.4, §6.1)
- `apps/steop/internal/client/mailbox.go` — Go client bindings for all mailbox RPC methods (`MailboxSend`, `MailboxList`, `MailboxGet`, `MailboxRead`, `MailboxArchive`)
- `apps/steop/main.go` — CLI entry point and subcommand registration
- `plugins/steop/skills/` — existing skill directory structure and SKILL.md format
- `plugins/steop/hooks/hooks.json` — hook event registration (Stop, SessionEnd, PreCompact)
- Claude Code Monitor tool — built-in tool that streams stdout lines from background processes, used by `st-watch` to receive messages from `steop mailbox watch`
