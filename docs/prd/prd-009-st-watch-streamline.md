# PRD-009 — Streamline st-watch Startup

**Status:** Implemented (v0.12.0)
**Target version:** v0.12.0
**Scope:** `apps/steop/cmd_mailbox_watch.go`, `plugins/steop/skills/st-watch/SKILL.md`
**Author:** Tasanakorn (design) + Claude Code (PRD authoring)

---

## 1. Goals

1. **CLI auto-resumes from `watcher:last_message_id` on startup.** The `steop mailbox watch` command reads the session-scoped `watcher:last_message_id` key from storage and uses it to skip already-processed messages. No `--since` flag required from the caller.
2. **CLI writes `watcher:last_message_id` after emitting each message.** The checkpoint moves from the skill into the CLI, eliminating a round-trip per message.
3. **Skill enters monitoring in a single LLM turn.** The skill issues `Bash(run_in_background)` and `Monitor` in one response. No preceding `steop storage get` step.
4. **Remove redundant skill steps.** Steps 4c (read-then-append active_tasks), 4d-1 (watcher:state transition), and 4g-1 (watcher:state reset) are removed from the skill. The CLI and existing tick loop already handle these concerns.

## 2. Non-goals

- Moving task claim, execute, or report logic into the CLI binary.
- Changing the mailbox protocol (message types, claim semantics, archive behavior).
- Adding server-side cursor tracking or push-based delivery.
- Changing the `watcher:state` / `watcher:heartbeat` lifecycle introduced in PRD-008.

## 3. Background & Motivation

The current `st-watch` startup sequence requires **2 LLM rounds** before the Monitor tool begins streaming:

1. **Round 1:** `steop storage get watcher:last_message_id` — parse result, conditionally build `--since` flag.
2. **Round 2:** `steop mailbox watch ... [--since=N]` as background process.
3. **Round 3:** `Monitor` tool to stream output.

Rounds 2 and 3 cannot be parallelized today because the skill must resolve the `--since` value first. This creates unnecessary latency on every watcher restart.

Additionally, the skill manages `watcher:last_message_id` writes (step 4h) and `watcher:state` transitions (steps 4d-1, 4g-1) that duplicate logic already present in the CLI's tick loop (PRD-008). The `watcher:active_tasks` tracking (step 4c) uses a read-then-append pattern that is more complex than necessary — the Stop hook cleanup already handles orphaned tasks.

### Cost of the current design

| Concern                         | Current                                    | After PRD-009                            |
| ------------------------------- | ------------------------------------------ | ---------------------------------------- |
| LLM rounds to enter monitoring  | 3 (storage get, background start, Monitor) | 1 (background start + Monitor parallel)  |
| `--since` pre-seeding           | Full `mailbox list` fetch to build seen set | ID comparison in poll loop (no list fetch) |
| `last_message_id` checkpoint    | Skill writes after each task               | CLI writes after each emitted message    |
| `watcher:state` transitions     | Skill writes before/after task execution   | CLI tick loop handles (PRD-008)          |
| `active_tasks` tracking         | Skill read-then-append pattern             | Skill write-only (overwrite)             |

## 4. Design

### 4.1 CLI auto-resume (`cmd_mailbox_watch.go`)

On startup, before the first poll, the CLI reads `watcher:last_message_id` from session-scoped storage:

```go
lastID := int64(0)
if val, err := c.StorageGet(id, "watcher:last_message_id"); err == nil {
    if parsed, err := strconv.ParseInt(val, 10, 64); err == nil {
        lastID = parsed
    }
}
```

The `--since` CLI flag is **removed**. The value always comes from storage.

### 4.2 ID-based filtering replaces pre-seeding

The current `--since` implementation fetches **all** messages (NEW + READ + ARCHIVE) to build a `seen` set. This is expensive and scales poorly.

With `lastID` from storage, the poll loop changes to a simple comparison:

```go
poll := func() {
    msgs, err := c.MailboxList(id, client.MailboxListOptions{
        Status:      []string{"NEW"},
        MessageType: msgType,
        Limit:       50,
    })
    if err != nil {
        return
    }
    for _, m := range msgs {
        if m.MessageID <= lastID {
            continue
        }
        if seen[m.MessageID] {
            continue
        }
        seen[m.MessageID] = true
        b, err := json.Marshal(m)
        if err != nil {
            continue
        }
        os.Stdout.Write(b)
        os.Stdout.Write([]byte("\n"))
        // Checkpoint after each emit
        lastID = m.MessageID
        c.StoragePut(id, "watcher:last_message_id", strconv.FormatInt(m.MessageID, 10))
    }
}
```

The `seen` map is kept as a secondary guard for messages within the same session (messages may arrive between the `lastID` read and the first poll), but the expensive pre-seeding list fetch is eliminated entirely.

### 4.3 Checkpoint write after each emit

After writing each message to stdout, the CLI immediately updates `watcher:last_message_id` with that message's ID. This is best-effort — a failure does not prevent the message from being emitted. On crash/restart, the worst case is re-emitting a message that was already processed, which is handled by the 409 response on `mailbox read` (claim deduplication).

### 4.4 Simplified skill startup (1 LLM turn)

The skill issues both tool calls in a single response:

```
Tool call 1 (parallel): Bash(run_in_background=true)
  steop mailbox watch --type TASK:REQUEST --interval 10

Tool call 2 (parallel): Monitor
  Stream stdout from the background process
```

No preceding storage read. No `--since` flag construction. The LLM enters monitoring immediately.

### 4.5 Skill step removals and simplifications

| Current step | Change     | Rationale                                                              |
| ------------ | ---------- | ---------------------------------------------------------------------- |
| Step 1       | **Remove** | CLI handles auto-resume from storage                                   |
| Step 4c      | **Simplify** | Write-only: `steop storage put watcher:active_tasks '[{...}]'`       |
| Step 4d-1    | **Remove** | CLI tick loop writes `watcher:state` with status=watching on every tick (PRD-008). Skill still writes status=running before task execution (4e preamble). |
| Step 4g-1    | **Remove** | Same as 4d-1 — the next tick resets to watching automatically          |
| Step 4h      | **Simplify** | Only `active_tasks` cleanup remains; `last_message_id` is CLI-managed |

### 4.6 Revised skill outline

```
Step 1 — Start Watcher + Monitor (single LLM turn)
  1a. Bash(run_in_background): steop mailbox watch --type TASK:REQUEST --interval 10
  1b. Monitor: stream stdout

Step 2 — On Receiving a Task
  2a. Parse JSON line
  2b. Claim task (steop mailbox read <message_id>; skip on 409)
  2c. Track active task (write-only, no read-then-append)
  2d. Send CHECKIN
  2e. Process task (route by meta.mode: flow or normal)
  2f. Report result (TASK:DONE or TASK:FAILED)
  2g. Archive original request
  2h. Clean up active_tasks tracking

Step 3 — Continue Monitoring
  Return to Monitor for the next message.
```

## 5. Changes by Component

| Component                                       | Change                                                                                              |
| ----------------------------------------------- | --------------------------------------------------------------------------------------------------- |
| `apps/steop/cmd_mailbox_watch.go`               | Read `watcher:last_message_id` on startup; filter by ID instead of pre-seed; write checkpoint after each emit; remove `--since` flag |
| `plugins/steop/skills/st-watch/SKILL.md`        | Merge startup into 1 step (background + Monitor); remove steps 1, 4c read, 4d-1, 4g-1; renumber    |
| `docs/prd/prd-009-st-watch-streamline.md` (new) | This PRD                                                                                            |
| `docs/README.md`                                | Add PRD-009 row                                                                                     |

## 6. Edge Cases

| Scenario                                       | Behavior                                                                                                  |
| ---------------------------------------------- | --------------------------------------------------------------------------------------------------------- |
| No `watcher:last_message_id` in storage        | `lastID` defaults to 0 — all NEW messages are emitted. First-run behavior unchanged.                     |
| Session UUID changes (new session)             | New session has no `last_message_id`. Clean start. Old session's key is unreachable. Dedup via 409.       |
| Multiple messages in one poll batch            | CLI updates `lastID` to the highest emitted ID. All messages are emitted. On restart, re-emit is safe.   |
| Checkpoint write fails                         | Message was already emitted to stdout. On restart, message may re-emit. 409 on claim handles it.          |
| Storage server unreachable on startup          | `lastID` stays 0. All NEW messages emitted. Duplicates handled by 409.                                   |
| Concurrent watchers on same project            | Each uses its own session storage. No cross-clobber. Each maintains its own `lastID`.                    |
| Messages with non-monotonic IDs               | Server-assigned IDs are monotonically increasing (SQLite AUTOINCREMENT). Not a concern.                   |

## 7. Migration

No migration required. The `--since` flag is removed from the CLI, but it was only used by the skill (not by external consumers). The skill is updated in the same release. No schema changes. No new storage keys — `watcher:last_message_id` already exists.

**Breaking change for direct `--since` users:** If anyone calls `steop mailbox watch --since=N` directly, the flag will be silently ignored (or rejected — implementation choice). Since the flag was never documented outside the skill, this is low risk. The recommended approach is to accept and ignore the flag with a stderr deprecation warning for one release, then remove.

## 8. Testing

### 8.1 Auto-resume smoke test

```bash
# Terminal 1: Start watcher, let it process one task, then Ctrl+C
steop mailbox watch --type TASK:REQUEST --interval 10

# Verify checkpoint was written
steop storage get watcher:last_message_id
# Expected: numeric message ID

# Restart watcher
steop mailbox watch --type TASK:REQUEST --interval 10
# Expected: no re-emission of already-processed messages
```

### 8.2 Cold start (no prior state)

```bash
# Ensure no watcher:last_message_id exists
steop storage delete watcher:last_message_id

# Start watcher — should emit all NEW messages
steop mailbox watch --type TASK:REQUEST --interval 10
```

### 8.3 Skill single-turn startup

Start `/steop:st-watch` and verify:
- Only 1 LLM turn is consumed before the Monitor tool begins streaming.
- No `steop storage get` call precedes the background process start.

### 8.4 Deduplication on crash recovery

```bash
# Start watcher, send a task, kill -9 during processing
# Restart watcher — task re-emits
# Skill calls `steop mailbox read <id>` — gets 409
# Skill skips the task and continues monitoring
```

### 8.5 Pre-seeding elimination verification

```bash
# With 100+ messages in mailbox, start watcher with existing last_message_id
# Verify no full list fetch occurs (check server logs or add debug logging)
# Only NEW messages with ID > last_message_id are emitted
```
