# PRD-008 — Watcher Lifecycle State + Heartbeat

**Status:** Implemented (v0.11.0)
**Target version:** v0.11.0
**Scope:** `apps/steop/cmd_mailbox_watch.go`, `internal/hooks/stop.go`, `plugins/steop/skills/st-watch/SKILL.md`
**Author:** Tasanakorn (design) + Claude Code (PRD authoring)

---

## 1. Goals

1. **Track watcher lifecycle state.** Introduce `watcher:state` and `watcher:heartbeat` keys in session-scoped storage. The watcher records whether it is idle (`watching`) or processing a task (`running`).
2. **Heartbeat for liveness detection.** `watcher:heartbeat` is updated on every poll tick. Consumers compare the timestamp age against the poll interval to detect stale (crashed) watchers.
3. **Cross-session discoverability.** Other sessions resolve to the watcher's session ID (via PRD-007's `ResolveTarget`), then read that session's storage to check watcher status.

## 2. Non-goals

- Reusing or modifying the existing flow phase/mode state in `steop_sessions.data`.
- Server-side TTL or auto-expiry of stale heartbeats.
- Changing the existing Stop hook `cleanupWatcherTasks()` behavior.
- Multi-watcher coordination or leader election.

## 3. Background & Motivation

The `steop mailbox watch` command is a minimal poll loop with no concept of its own lifecycle. Neither the CLI nor the `st-watch` skill publishes state that other sessions can read. When a user runs `steop send <project> "do something"`, there is no way to verify the target has an active watcher.

### Separation from flow state

Flow state (`phase`, `mode`, `step`) lives in `steop_sessions.data`, accessed via `steop state`. Watcher lifecycle uses `steop_storage_session` (session-scoped KV), accessed via `steop storage`. Different tables, different CLI subcommands, no conflict.

| Concern          | Table                      | CLI                | ID format            |
| ---------------- | -------------------------- | ------------------ | -------------------- |
| Flow phase/mode  | `steop_sessions.data`      | `steop state`      | `host:project:UUID`  |
| Watcher lifecycle | `steop_storage_session`   | `steop storage`    | `host:project:UUID`  |
| Watcher tasks    | `steop_storage_session`    | `steop storage`    | `host:project:UUID`  |

All watcher keys (`watcher:state`, `watcher:heartbeat`, `watcher:active_tasks`, `watcher:last_message_id`) are session-scoped — consistent scope, no cross-session clobber.

## 4. Design

### 4.1 Storage keys

Two new session-scoped storage keys (same scope as existing `watcher:active_tasks`):

#### `watcher:state`

```json
{
  "status": "watching",
  "task": null,
  "updated_at": "2026-04-12T10:30:00Z"
}
```

| Field        | Type                         | Description                                      |
| ------------ | ---------------------------- | ------------------------------------------------ |
| `status`     | `"watching"` or `"running"`  | Current lifecycle phase                          |
| `task`       | string or null               | Description of current task (null when idle)     |
| `updated_at` | RFC 3339 timestamp           | When this state was last written                 |

#### `watcher:heartbeat`

Plain RFC 3339 timestamp string, updated on every poll tick:

```
2026-04-12T10:30:10Z
```

Consumers determine liveness by comparing `now - heartbeat` against a threshold (e.g. `2 * poll_interval`).

### 4.2 CLI watch loop changes (`cmd_mailbox_watch.go`)

Three insertion points in the existing loop:

1. **Before first poll:** Write `watcher:state` with `status: "watching"` and `watcher:heartbeat`. Uses session-scoped ID from `mailboxClientAndID()`.

2. **On each tick (after `poll()`):** Update `watcher:heartbeat` with current timestamp.

3. **On signal exit:** Delete `watcher:state` and `watcher:heartbeat`.

All lifecycle writes are best-effort — failures are logged at debug level but never prevent the watch loop from operating.

### 4.3 Skill-driven transitions (`st-watch/SKILL.md`)

**Before processing a task (between steps 4d and 4e):**

```bash
steop storage put watcher:state '{"status":"running","task":"<description>","updated_at":"<now>"}'
```

**After task completes (between steps 4g and 4h):**

```bash
steop storage put watcher:state '{"status":"watching","task":null,"updated_at":"<now>"}'
```

These are session-scoped writes (hook injects `--x-session-id`).

### 4.4 Stop hook cleanup (`internal/hooks/stop.go`)

After the existing `cleanupWatcherTasks()` call, delete the lifecycle keys:

```go
if _, err := c.StorageDelete(sid, "watcher:state"); err != nil {
    logging.Debugf("stop: delete watcher:state: %v", err)
}
if _, err := c.StorageDelete(sid, "watcher:heartbeat"); err != nil {
    logging.Debugf("stop: delete watcher:heartbeat: %v", err)
}
```

Session-scoped — each session only deletes its own keys. No guard needed.

### 4.5 Lifecycle state diagram

```
Session starts st-watch
        │
        ▼
  ┌─────────────┐     watcher:state = {status:"watching"}
  │  WATCHING    │◄──────────────────────────────────────┐
  │  (idle poll) │                                        │
  └──────┬───────┘                                        │
         │ task arrives                                   │
         ▼                                                │
  ┌─────────────┐     watcher:state = {status:"running"}  │
  │  RUNNING     │                                        │
  │  (task exec) │────────────────────────────────────────┘
  └──────┬───────┘     task done → status back to "watching"
         │
         │ signal / session end
         ▼
  ┌─────────────┐     delete watcher:state + watcher:heartbeat
  │  STOPPED     │
  │  (no keys)   │
  └─────────────┘
```

## 5. Changes by Component

| Component                                     | Change                                                                                   |
| --------------------------------------------- | ---------------------------------------------------------------------------------------- |
| `apps/steop/cmd_mailbox_watch.go`             | Write `watcher:state` + `watcher:heartbeat` on start; heartbeat on tick; delete on signal |
| `apps/steop/internal/hooks/stop.go`           | Delete `watcher:state` and `watcher:heartbeat` (session-scoped, no guard needed)          |
| `plugins/steop/skills/st-watch/SKILL.md`      | Add state transitions: status=running before task, status=watching after task              |
| `docs/prd/prd-008-watcher-lifecycle.md` (new) | This PRD                                                                                 |
| `docs/README.md`                              | Add PRD-008 row                                                                          |

## 6. Edge Cases

| Scenario                                 | Behavior                                                                                 |
| ---------------------------------------- | ---------------------------------------------------------------------------------------- |
| Watcher crashes (SIGKILL, machine off)   | Keys remain in session storage. Consumers detect staleness by heartbeat age.             |
| Two watchers on same project             | Each writes to its own session storage. No clobber. Consumers check the resolved session. |
| Stop hook fires for non-watcher session  | `StorageDelete` for non-existent keys returns silently. No side effects.                 |
| Storage server unreachable during startup | Writes fail (logged at debug). Watch loop proceeds — lifecycle is advisory.              |
| Heartbeat write fails on a tick          | Logged at debug, no retry. Next tick retries. One miss doesn't trigger false "stale".    |
| Keys exist from previous crash           | Overwritten on next watcher start.                                                       |

## 7. Migration

No migration required. New keys are written to the existing `steop_storage_session` table. No schema changes. Existing watcher keys (`watcher:active_tasks`, `watcher:last_message_id`) are unchanged.

## 8. Testing

### 8.1 Manual smoke tests

```bash
# Terminal 1: Start watcher
steop mailbox watch --type TASK:REQUEST --interval 10

# Terminal 2: Check lifecycle state
steop storage get watcher:state
# Expected: {"status":"watching","task":null,"updated_at":"..."}

steop storage get watcher:heartbeat
# Expected: RFC 3339 timestamp, recent (within ~10 seconds)

# Terminal 1: Ctrl+C to stop watcher
# Terminal 2:
steop storage get watcher:state
# Expected: 404 / not found
```

### 8.2 Skill transition tests

```bash
# Start st-watch, send a task from another session
steop send stele "Update the README"

# While task is processing:
steop storage get watcher:state
# Expected: {"status":"running","task":"Update the README","updated_at":"..."}

# After task completes:
steop storage get watcher:state
# Expected: {"status":"watching","task":null,"updated_at":"..."}
```

### 8.3 Crash recovery test

```bash
# Start watcher, kill -9 it, verify keys are stale
# Start a new watcher session — fresh keys overwrite stale ones
```
