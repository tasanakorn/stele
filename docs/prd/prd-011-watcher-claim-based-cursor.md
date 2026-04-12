# PRD-011: Watcher cursor advances only on claim (drop persistent cursor)

- **Status:** Implemented (v0.12.3)
- **Version target:** v0.12.3
- **Author:** Tasanakorn (design) + Claude Code (PRD authoring)
- **Scope:** `apps/steop/cmd_mailbox_watch.go`

## Goals

- Drop the persistent `watcher:last_message_id` cursor entirely.
- Rely on the server-side `status=NEW` filter combined with an in-memory `seen` map for within-session deduplication.
- Fix the orphaned-message problem: if the LLM is idle (or busy on another task) when Monitor receives an event, the message stays `NEW` and re-emits on the next poll instead of being silently skipped because the cursor already advanced past it.

## Non-goals

- Server-side push (webhook/SSE). Polling stays. Interval defaults remain unchanged.
- Changing claim semantics or the 409-on-dup-claim contract established in PRD-009.
- Changing PRD-010 startup optimizations beyond removing the now-dead cursor-read goroutine.
- Introducing eviction policy for the in-memory `seen` map.

## Background & Motivation

PRD-010 made `st-watch` startup fast by running the cursor-read (`StorageGet watcher:last_message_id`) in parallel with lifecycle writes, and by emitting a `ready` line with `last_message_id` for Monitor to latch onto. That design kept a **persistent cursor** that advances on every emit: once the watcher writes a message to stdout, `last_message_id` is bumped and persisted via `StoragePut`, so future polls (in this process or any successor process) skip anything with `id <= lastID`.

This "advance on emit" rule is the root cause of an orphaned-message bug:

1. Watcher emits message `42` to stdout.
2. Monitor receives the event, but the LLM is idle / mid-turn / not in listening state, so nothing calls `mailbox.read`.
3. Watcher persists `lastID = 42`.
4. On the next poll, message `42` is still `NEW` (never claimed), but `m.MessageID <= lastID` filters it out.
5. Message `42` is effectively dropped until the next watcher restart picks it up from storage — and even then, only because the cursor was persisted. In the worst case, the cursor was persisted *and* the message was never claimed, so it sits `NEW` forever while the watcher keeps skipping it.

The correct advance point is **claim, not emit**. The server already implements this: `mailbox.read` transitions `NEW → READ` atomically and returns 409 on dup. Once a message is claimed (by this LLM or anyone else), the `status=NEW` filter excludes it from future `mailbox.list` results. There is no reason to maintain a second, client-side cursor — it only introduces a failure mode where emit-without-claim silently drops work.

The in-memory `seen` map is still needed: without it, a still-`NEW` message the LLM hasn't claimed yet would be re-emitted every `interval` seconds until it is finally claimed (or archived). With `seen`, the watcher emits each message exactly once per process lifetime; if the LLM never claims it, the next watcher process will emit it again, which is the intended recovery behavior documented in PRD-009.

### Current state

`apps/steop/cmd_mailbox_watch.go` reads, writes, and emits the cursor in four places:

- **Line 36** — deprecation warning for `--since`: `"mailbox watch: --since is deprecated; resume is automatic via watcher:last_message_id"`.
- **Lines 48-59** — parallel init goroutine: `StorageGet(id, "watcher:last_message_id")`, `ParseInt` into `lastID`, joined via `wg.Wait()` before first poll.
- **Line 72** — ready-line field: `{"type":"ready","last_message_id":<lastID>,"interval":<interval>}`.
- **Line 89** — poll filter: `if m.MessageID <= lastID { continue }`.
- **Lines 103-104** — checkpoint after emit: `lastID = m.MessageID; c.StoragePut(id, "watcher:last_message_id", strconv.FormatInt(m.MessageID, 10))`.

No other Go file, skill, or server component reads `watcher:last_message_id`. The SKILL.md uses the `ready` line purely as a liveness signal (Monitor latches the ready marker and then relays subsequent NDJSON events); the `last_message_id` field value is not consumed downstream.

The mailbox state machine is `(none) → NEW → READ → ARCHIVE` (also `NEW → ARCHIVE` direct). `mailbox.read` performs the `NEW → READ` transition and returns 409 if the message is not `NEW`. `mailbox.list status=["NEW"]` is side-effect free and returns oldest-first (`ORDER BY created_at ASC LIMIT ?`, CLI passes `Limit=50`).

## Design

**Option (a) — drop the persistent cursor entirely.**

1. Remove the `StorageGet("watcher:last_message_id")` goroutine and the surrounding `sync.WaitGroup` in the parallel-init block. The init phase collapses to the two fire-and-forget `StoragePut` calls for `watcher:state` and `watcher:heartbeat`.
2. Remove the `lastID` variable and the `m.MessageID <= lastID` filter in `poll()`.
3. Remove the `StoragePut("watcher:last_message_id", ...)` call after each emit.
4. Change the deprecation warning for `--since` to stop referencing `watcher:last_message_id`:
   `"mailbox watch: --since is deprecated; resume is automatic via server-side status=NEW filter"`.
5. Ready line emits `"last_message_id": null` — preserves the field for consumer compatibility, conveys "no client-side cursor" semantics. (`null` is a valid JSON value; Monitor does not read this field.)
6. The in-memory `seen map[int64]bool` remains unbounded. Watcher sessions are bounded by the Claude Code session lifetime; volume is O(tens). If growth ever becomes a concern, a follow-up can evict entries whose messages have transitioned out of `NEW`, but that is explicitly out of scope here.

**Why Option (a) over any hybrid.** Any scheme that keeps a cursor at all has to answer "when do we advance it?" The only safe answer is "when the message is claimed," and the server already tracks exactly that via `status`. Duplicating this state client-side adds no information and re-introduces the orphan-on-emit bug the moment emit and claim diverge. Dropping it outright is simpler, correct, and makes the startup path faster as a free side effect.

## Changes by Component

| Component                                                          | Change                                                                                                                                                |
| ------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| `apps/steop/cmd_mailbox_watch.go`                                  | Drop cursor read goroutine + `wg.Wait`; drop `lastID` variable, `<= lastID` filter, and post-emit `StoragePut`; update `--since` deprecation message. |
| `apps/steop/cmd_mailbox_watch.go` (ready line)                     | Emit `"last_message_id": null` instead of `<int>`.                                                                                                    |
| `apps/steop/` (package imports)                                    | Remove `sync` import if no longer used after `WaitGroup` removal. `strconv` still needed for `--interval` parsing.                                    |
| `plugins/steop/skills/st-watch/SKILL.md`                           | No change required — ready line is used as liveness only, field value is ignored. Optionally note in a sentence that the cursor is now server-side.   |
| `plugins/steop/.claude-plugin/plugin.json` / `apps/stele/Cargo.toml` | Version bump to v0.12.3 via `scripts/bump-version.py` (patch release).                                                                                |
| `docs/README.md`                                                   | Add PRD-011 row to the PRD table.                                                                                                                     |
| Server (`apps/stele/`)                                             | No changes. `mailbox.list status=NEW` and `mailbox.read` 409-dedup already provide the exact contract this PRD relies on.                             |
| Storage layer                                                      | No migration. Stale `watcher:last_message_id` keys from prior versions are harmless — no code reads them after this change.                           |

## Edge Cases

- **Backlog > 50 NEW messages.** `mailbox.list` caps at `Limit=50` oldest-first. If more than 50 messages accumulate, older ones block visibility of newer ones until claimed or archived. This is a pre-existing limit, not introduced here; acknowledged so operators know the ceiling. No change in this PRD.
- **Upgrade with orphaned NEW messages from v0.12.0 / v0.12.1.** On upgrade, any `NEW` message that a previous watcher had emitted and advanced past will now re-emit on the first poll of the new watcher. This is the intended recovery path — operators may see a short burst of previously-dropped messages, all of which claim cleanly (or 409 if something else already claimed them).
- **Crash between emit and claim.** Same behavior as PRD-009: on next watcher start (or next poll in this process, if the claim never happened), the message re-emits because it is still `NEW`; the claim attempt either succeeds or returns 409.
- **Stale `watcher:last_message_id` keys in storage.** Left in place after upgrade; nothing reads them. Operators who want to tidy up can `steop storage delete watcher:last_message_id` for the relevant `id`, but it is not required.
- **Multiple watcher processes on the same session.** Each has its own `seen` map; both may emit the same message. The 409-on-claim contract resolves the race — exactly one wins the `NEW → READ` transition. This matches PRD-008/PRD-009 behavior.
- **LLM claims `11` while `10` and `12` are also `NEW`.** Message `10` keeps appearing in `mailbox.list status=NEW` results and therefore keeps hitting the `seen` check on every poll in the current process (no re-emit), and re-emits cleanly on any new watcher process. 409 handles any dup-claim race. This is the intended behavior — no implicit ordering constraint on which `NEW` message the LLM chooses to claim.
- **`seen` map growth.** Unbounded per process. Volume is O(tens) per session; memory impact is negligible. Out of scope to evict here.
- **`to_id` matching.** Session-level `id` only; pre-existing constraint, not affected.

## Migration

No migration required.

- Existing `watcher:last_message_id` keys in storage become stale immediately after upgrade. Nothing reads them. They can be left as-is or manually deleted with `steop storage delete watcher:last_message_id` per `id`.
- No schema change, no RPC change, no SKILL.md behavioral change.
- Version bump is patch (`v0.12.2 → v0.12.3`) because the external contract (stdout NDJSON shape, RPC surface, config keys) is unchanged aside from `ready.last_message_id` becoming `null`, which consumers do not read.

## Testing

Manual smoke tests — no new automated harness needed. Run from `apps/steop/`:

1. **Build and install.** `go build -o target/steop . && rm -f ~/.local/bin/steop && cp target/steop ~/.local/bin/steop`. (macOS Tahoe SIGKILL workaround: always `rm` before `cp`.)
2. **Orphan recovery (the bug this PRD fixes).**
   - Start `steop mailbox watch --interval=2` in one terminal. Confirm the `ready` line is emitted with `"last_message_id": null`.
   - From another terminal, `steop mailbox send` a message to this session's `id`.
   - Watcher emits the message as NDJSON. Do NOT claim it.
   - Wait 2-3 poll intervals. Confirm the message is NOT re-emitted (in-memory `seen` guard works).
   - Kill the watcher. Restart it. Confirm the message re-emits on the first poll of the new process (server-side `status=NEW` still includes it).
   - Claim via `steop mailbox read`. Confirm subsequent watcher polls no longer see the message.
3. **409-on-dup-claim still works.** Start two watchers on the same `id`. Send one message. Confirm both emit it. Call `mailbox.read` twice; confirm the second returns 409.
4. **Startup time.** `time steop mailbox watch --interval=2` until `ready` line (or wrap with a small script). Should be at least as fast as v0.12.1, since the `StorageGet` + `wg.Wait` are gone.
5. **Stale cursor key is harmless.** Manually `steop storage put <id> watcher:last_message_id 999999`. Start watcher, send a message. Confirm it still emits (no cursor is read).
6. **`--since` deprecation warning.** `steop mailbox watch --since=123`. Confirm the warning references `status=NEW`, not `watcher:last_message_id`.
7. **SKILL-level end-to-end.** `/steop:st-watch` from inside Claude Code; send a task from another session via `/steop:st-send`; confirm the LLM receives and processes it; leave another task unclaimed briefly, confirm it is re-surfaced on watcher restart.
