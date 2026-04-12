# PRD-010 — st-watch Fast Startup

**Status:** Implemented (v0.12.1)
**Target version:** v0.12.1
**Scope:** `apps/steop/cmd_mailbox_watch.go`, `plugins/steop/skills/st-watch/SKILL.md`
**Author:** Tasanakorn (design) + Claude Code (PRD authoring)

---

## 1. Goals

1. **Emit a JSON "ready" line to stdout after init, so Monitor gets immediate feedback.** The watcher prints a `{"type":"ready",...}` NDJSON line as soon as it enters the poll loop, before the first poll completes. The `type` field distinguishes it from message lines (which carry `message_id` as a top-level key).
2. **Parallelize and defer startup RPCs to reduce CLI init time.** The three serial `StorageGet`/`StoragePut` calls (resume cursor read, lifecycle state write, heartbeat write) currently block for up to 30 seconds worst case. The lifecycle writes are best-effort (PRD-008 section 4.2) and can fire-and-forget via `fastClone()`. The resume cursor read runs concurrently with those writes.
3. **Trim SKILL.md to reduce LLM processing overhead.** Front-load the "start NOW" instruction (Step 1) and compress the verbose bash examples in Step 2 so the LLM spends fewer tokens parsing boilerplate before entering monitoring.
4. **Cut total time-to-monitoring from ~2.5 minutes to under 1 minute.** The combination of a ready line (instant Monitor feedback), parallel RPCs (eliminating serial 10-second timeouts), and a shorter skill (fewer LLM reasoning tokens) achieves this target.

## 2. Non-goals

- Changing the mailbox protocol or message handling logic.
- Moving task processing logic (claim, execute, report) into the CLI binary.
- Server-side push or WebSocket delivery.
- Modifying the `watcher:state` / `watcher:heartbeat` lifecycle format (fixed by PRD-008).
- Changing auto-resume semantics (fixed by PRD-009).

## 3. Background & Motivation

PRD-009 reduced the LLM round-trips to enter monitoring from 3 to 1. However, the CLI binary itself still takes several seconds to become ready because of serial blocking RPCs, and the Monitor tool has no signal that the watcher has initialized — it simply waits for the first message line, which may not arrive for the full poll interval (10 seconds default).

### Current state

The startup sequence in `cmd_mailbox_watch.go` (lines 41-61) is strictly serial:

| Step | Operation                                                  | Timeout | Blocking? |
| ---- | ---------------------------------------------------------- | ------- | --------- |
| 1    | `mailboxClientAndID()` — config load + identity resolution | ~instant | Yes      |
| 2    | `c.StorageGet(id, "watcher:last_message_id")`             | 10s     | Yes       |
| 3    | `c.StoragePut(id, "watcher:state", ...)`                  | 10s     | Yes       |
| 4    | `c.StoragePut(id, "watcher:heartbeat", ...)`              | 10s     | Yes       |
| 5    | `poll()` — first mailbox list fetch                        | 10s     | Yes       |

Steps 2-4 can collectively block for up to 30 seconds before the first poll. Steps 3-4 are best-effort writes (PRD-008 section 4.2) that already have a precedent for fire-and-forget via `fastClone()` — used in `log.go` (LogAppend) and `mailbox.go` (MailboxSend from hooks).

The SKILL.md is 99 lines. Steps 2c-2h contain verbose bash examples with full flag spelling that the LLM must parse on every task. The examples duplicate the same `--to`, `--type`, `--meta` pattern across CHECKIN, DONE, and FAILED messages.

### Cost of the current design

| Concern                 | Current                                         | After PRD-010                                    |
| ----------------------- | ----------------------------------------------- | ------------------------------------------------ |
| Time before Monitor gets first line | 10-40s (poll interval or first message) | <1s (ready line emitted immediately)             |
| Startup RPC wall time   | Up to 30s (3 serial 10s-timeout calls)          | ~500ms (1 concurrent read + 2 fire-and-forget)   |
| SKILL.md token overhead | ~99 lines, verbose bash blocks                  | ~60 lines, compressed command patterns            |
| Total time-to-monitoring | ~2.5 min (LLM parse + RPC + first poll)         | <1 min (fast ready + parallel RPCs + lean skill) |

## 4. Design

### 4.1 Ready line on stdout

After `mailboxClientAndID()` completes and before the first poll, the CLI emits a single NDJSON line:

```json
{"type":"ready","last_message_id":42,"interval":10}
```

Field semantics:

| Field              | Type   | Description                                                       |
| ------------------ | ------ | ----------------------------------------------------------------- |
| `type`             | string | Always `"ready"`. Distinguishes from message lines.               |
| `last_message_id`  | int64  | Resume cursor (0 if no prior state). May be 0 if StorageGet has not yet resolved when ready is emitted (see 4.2). |
| `interval`         | int    | Poll interval in seconds.                                         |

The Monitor tool receives this line immediately and can display a status indicator to the user. Message lines continue to use their existing schema (no `type` field, `message_id` as top-level key).

### 4.2 Parallel startup RPCs

Restructure the init sequence to run the resume-cursor read concurrently with fire-and-forget lifecycle writes:

```go
c, id := mailboxClientAndID()

// --- parallel init ---
var lastID int64
var wg sync.WaitGroup

// Goroutine 1: read resume cursor (must complete before first poll).
wg.Add(1)
go func() {
    defer wg.Done()
    if blob, err := c.StorageGet(id, "watcher:last_message_id"); err == nil && blob != nil {
        if v, err := strconv.ParseInt(blob.Content, 10, 64); err == nil {
            lastID = v
        }
    }
}()

// Fire-and-forget: lifecycle writes via fastClone() (500ms timeout).
now := time.Now().UTC().Format(time.RFC3339)
watchState := fmt.Sprintf(`{"status":"watching","task":null,"updated_at":%q}`, now)
fc := c.fastClone()
go fc.StoragePut(id, "watcher:state", watchState)
go fc.StoragePut(id, "watcher:heartbeat", now)

wg.Wait()
// --- end parallel init ---

// Emit ready line (lastID is resolved at this point).
readyLine, _ := json.Marshal(map[string]interface{}{
    "type":            "ready",
    "last_message_id": lastID,
    "interval":        interval,
})
os.Stdout.Write(readyLine)
os.Stdout.Write([]byte("\n"))
```

The `StorageGet` call still uses the standard client (10s timeout) because its result is load-bearing — the resume cursor must be resolved before the first poll. The two `StoragePut` calls use `fastClone()` (500ms timeout) and their results are discarded. The tick loop continues to refresh both lifecycle keys on every interval, so a missed initial write self-heals within one tick.

**Worst-case init time:** 10 seconds (StorageGet timeout) instead of 30 seconds. Typical case: <1 second.

### 4.3 Ready line before StorageGet (alternative considered, rejected)

Emitting the ready line before `StorageGet` completes would shave another few hundred milliseconds but would report `last_message_id: 0` in the ready line. This is misleading — the Monitor consumer cannot distinguish "no prior state" from "state not yet loaded." Since the ready line's primary value is signaling that the watcher process is alive and the Monitor can start streaming, waiting for the cursor read (typically <100ms) is an acceptable trade-off.

### 4.4 SKILL.md trimming

Reduce the skill from ~99 lines to ~60 lines by:

1. **Compressing bash examples in Steps 2c-2h.** Replace multi-line `steop mailbox send` examples with a single-line pattern showing the flag structure once, then refer to it for DONE/FAILED/CHECKIN variants.
2. **Removing redundant commentary.** Lines explaining what "NDJSON" means or restating the claim deduplication semantics (already handled by the CLI + 409 pattern).
3. **Front-loading Step 1.** Keep the "Start Watcher + Monitor" block as the first thing the LLM sees, before any task-processing detail.

The ready line also changes Step 1: the skill can now instruct the LLM to wait for the `{"type":"ready"}` line as confirmation that the watcher initialized successfully, rather than blindly waiting for the first message.

### 4.5 Revised Step 1 in SKILL.md

```markdown
## Step 1 — Start Watcher + Monitor

Issue both tool calls in parallel in a single turn:

1. **Bash** (`run_in_background: true`): `steop mailbox watch --type TASK:REQUEST --interval 10`
2. **Monitor**: stream stdout from the background watcher process.

The first line will be `{"type":"ready",...}` confirming the watcher initialized.
Subsequent lines are NDJSON task messages — proceed to Step 2 for each.
```

### 4.6 Tick-loop lifecycle writes

The tick loop (line 97-108) already refreshes `watcher:state` and `watcher:heartbeat` on every interval. These calls currently use the standard client. This PRD does **not** change the tick-loop calls to `fastClone()` — the tick loop is not latency-sensitive (it runs in the background between polls), and using the standard timeout provides better delivery guarantees for ongoing liveness signals.

## 5. Changes by Component

| Component                                          | Change                                                                                                         |
| -------------------------------------------------- | -------------------------------------------------------------------------------------------------------------- |
| `apps/steop/cmd_mailbox_watch.go`                  | Parallelize startup RPCs (goroutine for StorageGet, fastClone fire-and-forget for lifecycle writes); emit `{"type":"ready",...}` NDJSON line after init completes |
| `plugins/steop/skills/st-watch/SKILL.md`           | Add ready-line expectation to Step 1; compress Steps 2c-2h bash examples; remove redundant commentary; target ~60 lines |
| `docs/prd/prd-010-st-watch-fast-startup.md` (new)  | This PRD                                                                                                       |
| `docs/README.md`                                   | Add PRD-010 row                                                                                                |

## 6. Edge Cases

| Scenario                                  | Behavior                                                                                                             |
| ----------------------------------------- | -------------------------------------------------------------------------------------------------------------------- |
| StorageGet times out (server unreachable) | `lastID` stays 0. Ready line reports `last_message_id: 0`. All NEW messages emitted. Dedup via 409 on claim.         |
| fastClone lifecycle writes fail silently  | Next tick-loop iteration (within `interval` seconds) retries with standard timeout. Self-healing.                    |
| Monitor receives ready line               | LLM sees `{"type":"ready"}` and knows watcher is live. No action needed — wait for message lines.                   |
| Consumer parses ready line as a task      | Ready line has `"type":"ready"`, not `message_id`. Skill Step 2a parse will not match. Safe to ignore.               |
| Multiple watchers on same session         | Each emits its own ready line. Monitor streams interleaved lines. Existing dedup (409) handles concurrent claims.    |
| Interval flag set to minimum (2s)         | Ready line still emits `"interval":2`. No impact on startup parallelism.                                             |
| Signal received during init goroutines    | Signal handler is registered after `mailboxClientAndID()` but before goroutines complete. Cleanup runs normally.     |

## 7. Migration

No migration required. The ready line is additive — existing consumers that parse NDJSON message objects will encounter a line without `message_id` and should skip it (the skill already filters by `message_id` presence in Step 2a). No schema changes. No new storage keys. No CLI flag changes.

**Backward compatibility:** The ready line is a new stdout emission. Any consumer that assumes every stdout line is a mailbox message must be updated to check for the `type` field. The only known consumer is the `st-watch` skill, which is updated in the same release.

## 8. Testing

### 8.1 Ready line emission

```bash
# Start watcher, capture first line
steop mailbox watch --type TASK:REQUEST --interval 10 | head -1
# Expected: {"type":"ready","last_message_id":<N>,"interval":10}
```

Verify `last_message_id` matches the value in `steop storage get watcher:last_message_id` (or 0 if no prior state).

### 8.2 Startup timing

```bash
# Time from process start to ready line
time (steop mailbox watch --type TASK:REQUEST --interval 10 2>/dev/null | head -1)
# Expected: < 2 seconds (typical), < 11 seconds (worst case, server slow)
```

Compare against baseline (current serial init): expect 3-10x improvement in typical case.

### 8.3 Lifecycle writes self-heal

```bash
# Start watcher with server temporarily unreachable
# (e.g., stop stele-server, start watcher, restart stele-server within interval)
steop storage get watcher:state
# Expected: {"status":"watching",...} — written by first tick after server recovery
```

### 8.4 Ready line does not break task processing

```bash
# Send a task while watcher is running
steop mailbox send --to <id> --type TASK:REQUEST --subject "test" --meta '{"task_id":"t1","description":"echo hello"}'
# Verify watcher emits the task as a second NDJSON line (after the ready line)
# Verify skill Step 2 processes it normally
```

### 8.5 SKILL.md token reduction

Count tokens (approximate via line count) before and after:
- Before: ~99 lines
- After: ~60 lines (target: 40% reduction)

Verify Step 1 is the first actionable content after frontmatter.
