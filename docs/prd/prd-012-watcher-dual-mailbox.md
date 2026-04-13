# PRD-012: Watcher dual-mailbox polling (2-segment + 3-segment)

- **Status:** Implemented (v0.12.4)
- **Version target:** v0.12.4
- **Author:** Tasanakorn (design) + Claude Code (PRD authoring)
- **Scope:** `apps/steop/cmd_mailbox_watch.go`, `apps/steop/internal/client/`

## Goals

- Watcher polls both the project-level (2-segment) and session-level (3-segment) mailboxes on every tick.
- Works whether the sender addressed using a short name (resolves to 3-segment session ID) or explicitly used a 2-segment project ID.
- Deduplicates messages that appear in both inboxes within a single poll cycle.

## Non-goals

- Server-side prefix matching or fallback routing. `mailbox.list` stays exact-match on `to_id`.
- Changing `ResolveTarget` behavior in `apps/steop/internal/client/resolve.go`.
- Exposing session UUID via an env var from Claude Code so `mailboxClientAndID()` can always return the 3-segment form.
- Refreshing the secondary poll target after startup (see Edge Cases for the rationale).
- Eviction policy for the per-tick `seen` map (already bounded; cleared every tick).

## Background & Motivation

PRD-007 made `st-send <short-name>` resolve to an active 3-segment session ID (`host:project_dir:UUID`) when any active UUID session exists, falling back to 2-segment (`host:project_dir`) only when none does. PRD-008 and PRD-009 standardized the watcher on whatever ID `mailboxClientAndID()` returns — which is 3-segment only when `--x-session-id` is provided by the PreToolUse hook, and 2-segment otherwise.

These two decisions interact badly in the default skill flow:

1. Claude Code launches `steop mailbox watch` via the `/steop:st-watch` skill without `--x-session-id`. Watcher polls mailbox `host:project_dir` (2-segment).
2. A peer runs `steop send stele-monitor "task"`. `ResolveTarget` finds the watcher's active session and composes `host:/path/stele-monitor:<UUID>` (3-segment).
3. `mailbox.list` on the server filters `WHERE to_id = ?1` — exact match. The 3-segment message is never returned to the 2-segment watcher. The task silently disappears.

The inverse is also broken: when the watcher is started *with* `--x-session-id` (PreToolUse hook injection), a sender who explicitly addresses the 2-segment project ID (e.g. because no active session existed at send time, or the caller deliberately chose project-level) won't be picked up either.

The fix lives entirely on the watcher side. Senders should continue to address whichever form `ResolveTarget` or their explicit input produces. The server contract (exact `to_id` match, no prefix semantics, no fallback) stays unchanged — this preserves the load-bearing invariant from DESIGN.md that the 3rd segment is a closed set (`UUID` or literal `USER`) and that storage dispatches on arity without ambiguity. The watcher simply polls both mailboxes.

### Current state

`apps/steop/cmd_mailbox_watch.go` builds a single poll target:

- **Line 41** — `c, id := mailboxClientAndID()`. `id` is 2-segment if `globalSessionID == ""`, 3-segment otherwise.
- **Line 86 (post-PRD-011)** — the poll loop runs one `c.MailboxList(id, ...)` call per tick with exactly that single ID.
- **Lines 63-66** — lifecycle writes (`watcher:state`, `watcher:heartbeat`) use the same `id`.

`mailboxClientAndID()` (in `cmd_mailbox.go`) produces the 2-seg vs. 3-seg form by inspecting the package-level `globalSessionID` set from `--x-session-id`. It does not consult storage or session listings — it is purely a function of CLI flags.

`client.SessionList(host, projectDir, state, limit)` already exists (`internal/client/sessions.go`); it returns sessions filtered by host + project_dir + lifecycle state. Sessions have `ID` (composite form), `LastActiveAt` (RFC3339Nano), and the usual fields.

The server side is untouched: `steop_mailbox_list` SQL is `WHERE to_id = ?1` (`docs/steop/DESIGN.md`). No prefix matching, no arity-based fallback — by design, per the PRD-001 RPC identity constraint.

## Design

**Dual-ID polling at the client with startup-time secondary discovery.**

1. After `c, id := mailboxClientAndID()`, build a `pollIDs []string` slice with the primary `id` first and the complementary form appended:
   - If `globalSessionID != ""` (primary is 3-segment): append `c.ProjectID()` (the 2-segment form).
   - Else (primary is 2-segment): call `latestSessionID(c)` to discover the most recently active UUID session for this `(host, project_dir)` and append its composite ID if one exists.

2. Introduce a `latestSessionID(c *client.Client) string` helper:
   - **Project-dir guard:** if `c.ProjectDir() == ""`, return `""` immediately. Calling `SessionList` without a `project_dir` filter would match sessions from *all* projects on the host, which is wrong.
   - Call `c.SessionList(c.Host(), c.ProjectDir(), "active", 0)`.
   - Skip entries whose composite ID is not 3-segment or whose 3rd segment is the literal `USER`.
   - Return the `ID` of the UUID session with the greatest `LastActiveAt` (RFC3339Nano), or `""` if none qualifies.
   - Called exactly once at startup. Not refreshed on subsequent ticks.

3. Rewrite the `poll` closure to iterate `pollIDs`:
   - Clear `seen map[int64]bool` at the start of every tick.
   - For each `pid` in `pollIDs`, call `c.MailboxList(pid, ...)` with the same `status=["NEW"]`, `msgType`, `Limit=50` filter.
   - Dedup emits using `seen[m.MessageID]`.
   - On `MailboxList` error: `continue` (same as today — one bad ID does not stop the other).

4. Lifecycle writes (`watcher:state`, `watcher:heartbeat`) continue to use the primary `id` returned by `mailboxClientAndID()`. The watcher's identity for liveness purposes is unchanged. The secondary poll target is purely a read-side mirror.

5. Signal handler continues to `StorageDelete` lifecycle keys under the primary `id` only.

**Why startup-time discovery, not per-tick.** Calling `SessionList` every tick would add a network round-trip on every poll interval for no practical gain. The 2-segment project-level mailbox already catches messages addressed to any new session that starts after the watcher begins, *provided the sender chooses 2-segment*. PRD-007 makes senders prefer 3-segment to the most recently active session — so if a newer UUID session appears mid-watch, peer sends to it will land in that new session's inbox and the watcher will miss them. This is an explicit limitation, captured under Edge Cases; the right fix is either (a) the watcher runs under `--x-session-id` in the first place (PRD-003 path), or (b) a later PRD refreshes `latestSessionID` on a slow cadence. Out of scope here.

**Why not server-side prefix match / arity fallback.** The RPC identity contract (DESIGN.md, PRD-001) explicitly closes the 3rd segment to `UUID | USER` and dispatches storage on arity. Adding "2-seg matches 3-seg on same prefix" breaks that contract and bleeds project-level messages into arbitrary session inboxes. The watcher is the correct place to union the two views because only the client knows whether it wants project-level, session-level, or both.

## Changes by Component

| Component                                             | Change                                                                                                                                                                                                                |
| ----------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `apps/steop/cmd_mailbox_watch.go`                     | Build `pollIDs` slice after `mailboxClientAndID()`. Primary first, secondary (2-seg or 3-seg complement) second. Rewrite `poll` to iterate `pollIDs`, keeping `seen` as per-tick dedup. Lifecycle writes use primary. |
| `apps/steop/cmd_mailbox_watch.go` (new helper)        | Add `latestSessionID(c *client.Client) string` with project-dir guard and RFC3339Nano ordering over the UUID-only subset of `SessionList(…, "active", 0)`.                                                            |
| `apps/steop/internal/client/`                         | No new public API. Existing `Client.Host()`, `ProjectDir()`, `ProjectID()`, `SessionList(...)` suffice.                                                                                                               |
| `plugins/steop/skills/st-watch/SKILL.md`              | No change. NDJSON shape and ready line are unchanged.                                                                                                                                                                 |
| `plugins/steop/.claude-plugin/plugin.json` / `apps/stele/Cargo.toml` | Patch bump to v0.12.4 via `scripts/bump-version.py`.                                                                                                                                                                  |
| `docs/README.md`                                      | Add PRD-012 row to the PRD table.                                                                                                                                                                                     |
| Server (`apps/stele/`)                                | No changes. Exact-match `to_id` filter is preserved deliberately.                                                                                                                                                     |
| Storage layer                                         | No migration. No new keys.                                                                                                                                                                                            |

## Edge Cases

- **No active UUID session at watcher startup.** `latestSessionID` returns `""`; `pollIDs` has length 1 (just the 2-segment ID). Watcher behaves exactly as today for a pure project-level flow. Once a UUID session starts and a sender addresses it 3-segment, see next bullet.
- **New UUID session starts after watcher.** Secondary poll target is fixed at startup and does not refresh. A sender that `ResolveTarget`s to that new session's 3-segment ID will miss this watcher's view. Workaround: start the watcher with `--x-session-id=<new-UUID>` (PRD-003 hook path), or restart the watcher. A future PRD may add slow-cadence refresh; out of scope here.
- **Watcher started with `--x-session-id`.** Primary is 3-segment, secondary is the 2-segment project ID. Always covers both forms regardless of whether other sessions exist — no `SessionList` call needed on this branch.
- **Duplicate message appears in both mailboxes.** Cannot happen under the current RPC: a single `mailbox.send` writes one row with one `to_id`. The `seen` map is defensive — it also handles the (hypothetical) case of the 2-seg and 3-seg discovery collapsing into the same ID in a future refactor.
- **Across ticks, same `NEW` message re-emits.** This is PRD-011 at-least-once delivery. Per-tick `seen` reset is intentional; cross-tick dedup lives server-side via `status` transition on claim.
- **`c.ProjectDir() == ""`.** `latestSessionID` returns `""` and the watcher degrades to single-ID polling on the primary. Prevents accidental cross-project session discovery on the host.
- **`SessionList` error.** `latestSessionID` returns `""`; watcher starts with single-ID `pollIDs`. Logged as a `continue`-equivalent (no startup abort) — startup latency budget per PRD-010 stays intact.
- **Heartbeat and state keys.** Written under primary `id` only. Monitors and liveness probes (PRD-008) continue to see a single canonical watcher identity. The secondary poll target is intentionally invisible to lifecycle observers.
- **Two-watcher race on the same project.** Each watcher picks its own `latestSessionID` independently; both may end up polling the same session's mailbox. 409-on-claim (PRD-009) resolves the duplicate-claim race exactly as before.
- **Backlog > 50 NEW messages per mailbox.** `Limit=50` oldest-first, per mailbox. Unchanged limit; now applies independently to each of the up-to-two polled mailboxes, which effectively doubles the visible ceiling in the dual-poll case. Not a contract change.

## Migration

No migration required.

- No schema change, no RPC change, no new storage keys, no SKILL.md change.
- Stale `watcher:*` lifecycle keys from prior versions continue to be overwritten/deleted on the primary `id` as before.
- Patch bump (`v0.12.3 → v0.12.4`) — the external NDJSON shape, ready line, RPC surface, and config keys are all unchanged. The only observable difference is that messages previously dropped on the floor now arrive at the watcher.

## Testing

Manual smoke tests. Run from `apps/steop/`:

1. **Build and install.** `go build -o target/steop . && rm -f ~/.local/bin/steop && cp target/steop ~/.local/bin/steop`. (macOS Tahoe SIGKILL workaround: always `rm` before `cp`.)
2. **Dual-mailbox: the bug this PRD fixes.**
   - Start Claude Code in a project with an active UUID session (any `claude` invocation registers one). Note the UUID via `steop session list`.
   - In a separate terminal: `steop mailbox watch --interval=2`. Confirm `ready` line. No `--x-session-id` → primary is 2-segment.
   - From a peer terminal: `steop send <this-project-short-name> "hello"`. `ResolveTarget` should compose a 3-segment session ID.
   - Confirm the watcher emits the message as NDJSON within one poll interval (secondary poll target picked up the 3-segment message).
   - Claim via `steop mailbox read`. Confirm no re-emit.
3. **Inverse: watcher under `--x-session-id`, sender addresses 2-seg project.**
   - Start `steop mailbox watch --x-session-id=<UUID> --interval=2`. Primary 3-seg, secondary 2-seg project.
   - Peer sends `steop mailbox send --to-id='<host>:<project_dir>' ...` (raw 2-segment).
   - Confirm watcher emits it.
4. **No active UUID session.** Ensure no active sessions exist for the project. Start watcher without `--x-session-id`. Confirm `pollIDs` length 1 (only 2-seg polled — add a one-line debug print if needed during development, remove before merge). Peer sends to 2-seg project ID; confirm delivery. Peer sends to a 3-seg ID with `USER` literal; confirm watcher does *not* pick it up (USER is filtered out of `latestSessionID`).
5. **Per-tick dedup.** Temporarily configure `pollIDs` to hold the same ID twice (debug-only). Send one message. Confirm it is emitted once per tick, not twice.
6. **Lifecycle keys remain on primary.** After watcher startup, `steop storage get <primary-id> watcher:state` returns `watching`; `steop storage get <secondary-id> watcher:state` returns empty. Signal the watcher (Ctrl-C); confirm primary's keys are deleted and secondary never had any.
7. **`SessionList` failure path.** Point the CLI at an offline server (or break `STELE_BIND` temporarily). Watcher start still proceeds with single-ID polling; no startup hang. Restore the server.
8. **SKILL-level end-to-end.** `/steop:st-watch` from inside Claude Code. From a peer Claude Code session, `/steop:st-send <this-project-short-name> "task"`. Confirm the task is received and processed regardless of whether `ResolveTarget` produced 2-seg or 3-seg. Repeat with the peer session forced to 2-seg (no active UUID sessions) and 3-seg (active session present).
