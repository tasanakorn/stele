# PRD-013: st-watch autonomous monitoring with explicit event conditions

- **Status:** Implemented (v0.12.5)
- **Version target:** v0.12.5
- **Author:** Tasanakorn (design) + Claude Code (PRD authoring)
- **Scope:** `apps/steop/cmd_mailbox_watch.go`, `plugins/steop/skills/st-watch/SKILL.md`, `apps/steop/scripts/smoke-mailbox.py`

## Goals

- Make `/steop:st-watch` work reliably end-to-end: Claude Code starts the Monitor, receives watcher events, and autonomously processes tasks — without user interaction.
- Give the skill explicit per-`message_type` conditions so Claude Code knows exactly what to do with each line: process `TASK:REQUEST`, ignore `WATCHER:READY` and all other `WATCHER:*` lines, loop back after each task.
- Re-add the `WATCHER:READY` startup line (dropped since PRD-012) using the canonical `message_type` field and `NAMESPACE:NAME` value convention — consistent with real mailbox messages — so the skill's filter condition uses a single field across all line types.
- Update the mailbox smoke test to tolerate and skip past the ready line before asserting on the task message.

## Non-goals

- Changing watcher polling logic, dual-mailbox behavior, per-tick dedup, or lifecycle writes (all PRD-008..PRD-012 behavior preserved).
- Fixing `steop send` target resolution (`ResolveTarget`) or any sender-side addressing.
- Adding other `WATCHER:*` meta-events (e.g. `WATCHER:TICK`, `WATCHER:ERROR`). Only `WATCHER:READY` is (re-)introduced here.
- Changing the on-the-wire shape of persisted `steop_mailbox` rows. This PRD only touches the watcher's NDJSON stdout stream.
- Server-side changes. `stele-server` is untouched.

## Background & Motivation

The watcher's stdout stream is NDJSON. Consumers (the `/steop:st-watch` skill, the smoke test, and any future monitor UI) need a single rule to decide "is this line a task I must act on, or a side-band event I should ignore?"

Real mailbox messages (`models.MailboxMessage`) discriminate via a `message_type` field with namespaced values like `TASK:REQUEST`, `TASK:DONE`, `TASK:FAILED`, `TASK:CHECKIN`. The original ready line introduced by PRD-010 used a different field name (`type`) and a bare, un-namespaced value (`"ready"`). Two shapes, two parsing rules, two places a consumer can guess wrong.

Worse, the ready line is currently missing from the binary entirely — PRD-012's dual-mailbox rewrite dropped it. The skill and smoke test both implicitly assume the first line is a task message, which is fragile: the very first poll is also the slowest (cold RPC + `SessionList` lookup), and any future "watcher started" signal has nowhere to live.

This PRD re-introduces the ready line under the canonical `message_type` field with a namespaced `WATCHER:READY` value, and codifies the consumer-side filter rule that makes every future `WATCHER:*` event automatically safe to add.

### Current state

**`apps/steop/cmd_mailbox_watch.go`** (post-PRD-012, at `runMailboxWatch`):

- Line 41: `c, id := mailboxClientAndID()`.
- Lines 50-59: builds `pollIDs` (primary + optional complementary ID).
- Lines 62-66: fire-and-forget lifecycle writes (`watcher:state`, `watcher:heartbeat`) via `fc.StoragePut`.
- Line 75: `poll` closure; walks `pollIDs`, emits each `MailboxMessage` as a single JSON line on stdout.
- Line 107: immediate first `poll()`. **No ready line is emitted before or after this call.**

The struct emitted by `poll` is `models.MailboxMessage` (already carrying `message_type` — e.g. `"TASK:REQUEST"`).

**`plugins/steop/skills/st-watch/SKILL.md`** (current):

- Step 1 tells the agent to invoke the `Monitor` tool on `steop mailbox watch --type TASK:REQUEST --interval 10` with `persistent: true`, and says "Lines emitted are NDJSON task messages — proceed to Step 2 for each."
- There is **no explicit filter rule**. Because the current binary happens to emit only task messages (the ready line was dropped), the agent's implicit rule "every line is a task" is accidentally correct today and will silently break the moment any meta-event is emitted.

**`apps/steop/scripts/smoke-mailbox.py`** (current):

- Starts a watcher with `--type=TASK:REQUEST`, sleeps 1 s, sends a task, then in a loop reads lines until `json.loads` succeeds, and asserts `event["message_type"] == "TASK:REQUEST"` on the first parseable line.
- Today this works because the first parseable line *is* the task. Once the ready line returns, the first parseable line will be `WATCHER:READY` and the assertion will fail.

**Historical PRD references.** PRD-010 specified the ready line as `{"type":"ready","last_message_id":null,"interval":10}`; PRD-011 clarified `last_message_id` stays `null`; PRD-012 claimed the ready line is unchanged but the implementation dropped it. No production code currently emits `"type":"ready"`. Grep confirms: the only references to that string live in `docs/prd/prd-010-*.md` and `docs/prd/prd-011-*.md`.

**Discriminator value set.** Across production code, `message_type` values already in use are `TASK:REQUEST`, `TASK:DONE`, `TASK:FAILED`, `TASK:CHECKIN` (all uppercase, `NAMESPACE:NAME`). `WATCHER:READY` fits the convention.

## Design

**Re-emit the ready line, shaped as a meta-event in the same `message_type` namespace as real mailbox messages, and codify the consumer filter rule "process `TASK:REQUEST`, ignore everything else."**

1. **Watcher** (`apps/steop/cmd_mailbox_watch.go`): before the immediate first `poll()` call (line 107), emit a single NDJSON line on stdout:

   ```json
   {"message_type":"WATCHER:READY","interval":10}
   ```

   - Field order is intentional: `message_type` first so a line-oriented consumer can discriminate without fully parsing. Numeric fields follow.
   - `interval` is the resolved poll interval (post-clamp, in seconds) — unchanged from PRD-010's intent.
   - No `last_message_id` field. Per PRD-011, the persistent cursor was dropped; re-introducing it (even as `null`) is misleading.
   - Emit *before* the first `poll()` so consumers have a reliable "watcher is up and about to poll" signal even if the first poll takes the full cold-start budget. This also keeps the ready line strictly ahead of any task line.
   - Write via `json.Marshal` of an anonymous struct (not a raw string) so the encoder handles escaping consistently with the rest of the stream. Flush is not required — `os.Stdout` is line-buffered when attached to a terminal and pipe-buffered under `Monitor`; Go's default fd-3 behavior is sufficient.

2. **Skill** (`plugins/steop/skills/st-watch/SKILL.md`): add an explicit filter rule at the top of Step 2, replacing the "Lines emitted are NDJSON task messages" hand-wave. Concretely:

   - New first paragraph under **Step 2 — On Receiving a Task**: "Each line is a JSON object with a `message_type` field. **Process only lines where `message_type == "TASK:REQUEST"`.** Ignore all other lines (including `WATCHER:READY` and any other `WATCHER:*` meta-event) — do not parse them further, do not act on them, just wait for the next line."
   - Keep Step 1 unchanged except for a one-line clarification that the first line is typically `{"message_type":"WATCHER:READY",...}` and should be ignored per the Step 2 rule.
   - The `--type TASK:REQUEST` CLI flag is retained — it filters what the server returns into the task stream. The skill-side filter is belt-and-suspenders: it defends against `WATCHER:*` meta-events (which bypass `--type`), future task types, and any accidental stray output.

3. **Smoke test** (`apps/steop/scripts/smoke-mailbox.py`): in the parse loop, skip lines whose `message_type` is not `TASK:REQUEST` instead of breaking on the first parseable line. Concretely, after `event = json.loads(line)`:

   ```python
   if event.get("message_type") != "TASK:REQUEST":
       event = None
       continue
   break
   ```

   The existing 10-second deadline stays. The test now tolerates any number of leading `WATCHER:*` lines before the task arrives.

**Why a `message_type` discriminator, not a `kind` / `event` / wrapping `{"event":..., "data":...}`.** The watcher stream already carries `MailboxMessage` objects that have a `message_type` field. Reusing the same field for meta-events means consumers parse one shape, not two. A wrapping envelope would force every task line to change shape too; out of scope and user-hostile to existing scripts.

**Why `WATCHER:READY`, not `READY` or `watcher.ready`.** The `NAMESPACE:NAME` convention is already locked in by `TASK:*`. `WATCHER:*` parallels it and makes "ignore everything that isn't `TASK:REQUEST`" a simple string-equality check, not a prefix or glob check. Future meta-events (`WATCHER:STOPPING`, `WATCHER:ERROR`) slot in without further contract work.

**Why not a separate channel (stderr / sidecar file).** The watcher's output is consumed by the Claude Code `Monitor` tool, which reads stdout line-by-line. Adding a second channel would mean touching the `Monitor` integration and every downstream consumer. A single NDJSON stream with a typed discriminator is strictly simpler.

**Why not gate the ready line behind a flag.** It is small, ordered first, and ignored by the documented consumer rule. A flag would create two shapes for the same binary and invites drift.

## Changes by Component

| Component                                                            | Change                                                                                                                                                                                                 |
| -------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `apps/steop/cmd_mailbox_watch.go`                                    | Emit `{"message_type":"WATCHER:READY","interval":<interval>}` as a single NDJSON line on stdout immediately before the first `poll()` (between current lines 106 and 107). Use `json.Marshal`.         |
| `plugins/steop/skills/st-watch/SKILL.md`                             | Add explicit filter rule in Step 2: process only `message_type == "TASK:REQUEST"`; ignore all other lines incl. `WATCHER:*`. Add one-line note to Step 1 that the first line is typically `WATCHER:READY`. |
| `apps/steop/scripts/smoke-mailbox.py`                                | In the readline loop, skip events whose `message_type != "TASK:REQUEST"` instead of asserting on the first parseable line.                                                                             |
| `plugins/steop/.claude-plugin/plugin.json` / `apps/stele/Cargo.toml` | Patch bump to v0.12.5 via `scripts/bump-version.py`.                                                                                                                                                   |
| `docs/README.md`                                                     | Add PRD-013 row to the PRD table.                                                                                                                                                                      |
| Server (`apps/stele/`)                                               | No changes. Meta-events live purely on the watcher's stdout.                                                                                                                                           |
| Storage layer                                                        | No migration. No new keys.                                                                                                                                                                             |

## Edge Cases

- **Legacy consumers keyed on `type:"ready"`.** None exist in this repo (grep confirms). External consumers written against PRD-010 text will not match. Acceptable: the ready line has been missing since PRD-012 (v0.12.4), so any external code that depended on it is already broken. This PRD fixes that gap with a better shape.
- **Consumer that blindly acted on every line.** Becomes visible on the first `WATCHER:READY`: the old `/steop:st-watch` skill would have tried to extract `meta.task_id` from the ready line and failed. The explicit filter rule in the updated SKILL.md pre-empts this; the smoke test's updated skip guards it programmatically.
- **Cold-start slower than one poll interval.** Ready line is emitted before the first `poll()`, so the "watcher is up" signal arrives even if the first RPC stalls. A consumer watching for liveness can use the ready line instead of waiting for a real message.
- **Future `WATCHER:STOPPING` / `WATCHER:ERROR`.** Automatically dropped by the `message_type == "TASK:REQUEST"` consumer filter — no skill edit required. If a consumer wants to display them, it can match `startswith("WATCHER:")` without affecting the task-processing path.
- **Accidental collision with a real mailbox message whose `message_type` starts with `WATCHER:`.** The `--type TASK:REQUEST` CLI flag already filters `MailboxList` results server-side, and no legitimate mailbox message uses the `WATCHER:*` namespace today. The reserved namespace is called out in Step 2 text.
- **`interval` field type.** Go `int` marshals as JSON number; consumers must read it as an integer count of seconds (unchanged from PRD-010's semantics).
- **Buffering.** Under the Claude Code `Monitor` tool and the smoke test's `subprocess.PIPE`, Go's default stdout buffering flushes on newline for line-oriented writes. Not a new risk — every task line today relies on exactly the same behavior.
- **Dual-mailbox polling (PRD-012).** Ready line is emitted exactly once at startup, before any poll. It does not duplicate per `pollIDs` entry. Per-tick `seen` dedup is unaffected (only touches `MessageID`).
- **Lifecycle keys (PRD-008).** `watcher:state` / `watcher:heartbeat` semantics are unchanged. The ready line is a stdout signal, not a storage signal.

## Migration

No migration required.

- No schema change, no RPC change, no new storage keys.
- Skill consumers that follow the updated Step 2 rule are forward-compatible with any future `WATCHER:*` meta-event.
- Patch bump (`v0.12.4 → v0.12.5`) — the RPC surface, mailbox wire format, and config keys are unchanged. The only observable change is one additional NDJSON line at watcher startup, with a documented filter rule that causes compliant consumers to ignore it.

## Testing

Manual smoke tests. Run from `apps/steop/`:

1. **Build and install.** `go build -o target/steop . && rm -f ~/.local/bin/steop && cp target/steop ~/.local/bin/steop`. (macOS Tahoe SIGKILL workaround: always `rm` before `cp`.)
2. **Ready line shape.** `steop mailbox watch --type=TASK:REQUEST --interval=5`. The first stdout line must be exactly `{"message_type":"WATCHER:READY","interval":5}` (field order, value case, integer interval). Pipe to `jq -c .` to confirm valid JSON.
3. **Ready line precedes first poll.** With no backlog messages, the watcher should emit exactly the ready line and then nothing until either a task arrives or the next tick. Send a task from a peer and confirm the second line is the `TASK:REQUEST` NDJSON.
4. **Smoke test passes.** `python apps/steop/scripts/smoke-mailbox.py`. The test should skip the `WATCHER:READY` line and still assert on the `TASK:REQUEST` subject within its 10-second deadline.
5. **Skill filter rule.** From Claude Code, `/steop:st-watch`. Verify the watcher emits `WATCHER:READY`; the agent does *not* attempt to claim or process it (no `steop mailbox read` call fires). Send a `TASK:REQUEST` from a peer via `/steop:st-send`; verify normal Step 2 flow fires on that line only.
6. **Interval clamp.** `steop mailbox watch --interval=500 --type=TASK:REQUEST`. The clamp in the flag parser pins `interval` to 300; the ready line must report `"interval":300`, not `500`.
7. **Interval floor.** `steop mailbox watch --interval=1` — exits with status 2 before any stdout; no ready line emitted. Confirms the ready line is strictly post-arg-validation.
8. **Dual-mailbox interaction (regression check on PRD-012).** With an active UUID session, start the watcher without `--x-session-id`. Ready line emits once. Send to 2-segment project ID and to 3-segment session ID in succession; both arrive as separate `TASK:REQUEST` lines, and only one `WATCHER:READY` is ever emitted.
9. **Lifecycle keys (regression check on PRD-008).** `steop storage get <primary-id> watcher:state` still returns `watching`; Ctrl-C still deletes `watcher:state` and `watcher:heartbeat`. The ready line does not touch storage.
