# PRD-014: mailbox watch parsing + emission throttle

- **Status:** Implemented (v0.13.0)
- **Version target:** v0.13.0
- **Author:** Tasanakorn (design) + Claude Code (PRD authoring)
- **Scope:** `apps/steop/cmd_mailbox_watch.go`, `plugins/steop/skills/st-watch/SKILL.md`, `apps/stele/crates/stele-server/src/steop_api.rs`, `apps/stele/crates/stele-server/src/db.rs`, `apps/steop/internal/client/mailbox.go`, `apps/steop/cmd_mailbox.go`

## Goals

- Accept both `--flag=value` and `--flag value` forms for every flag `steop mailbox watch` advertises (`--type`, `--interval`, `--since`), matching the convention users reasonably expect from a POSIX-style CLI. _(shipped in v0.12.6 — section kept for historical completeness)_
- Introduce a new `POST /api/v1/steop/mailbox.update_meta` RPC that performs a shallow JSON merge of a caller-supplied patch into the target row's `meta` column, without touching the lifecycle `status` column. This is the narrow server-side hook agents and the watcher need to exchange per-message state (starting with `task_status`).
- Add an in-process emission throttle to `steop mailbox watch`: after emitting a `TASK:REQUEST` NDJSON line, suspend further stdout emissions until either (a) the row's `meta.task_status` becomes `"DONE"`, or (b) a 5-minute deadline elapses — whichever comes first. Polling continues unchanged during suspend; only stdout is gated.
- Update `plugins/steop/skills/st-watch/SKILL.md` to add an ack contract: the agent MUST call `steop mailbox update-meta <message_id> '{"task_status":"DONE"}'` immediately before archiving each handled task, giving the watcher the signal it needs to lift the throttle and accept the next task.

## Non-goals

- No server-side changes _beyond_ the new `mailbox.update_meta` RPC. The existing mailbox RPC surface, schema, and lifecycle `status` semantics are untouched.
- No new filter knobs, no multi-value `--type`, no regex matching. The flag-parser fix is scoped to `--flag value` acceptance.
- No change to watcher polling loop, dual-mailbox behavior (PRD-012), ready-line emission (PRD-013), or lifecycle writes (PRD-008). Polling continues at the configured interval during throttle suspend.
- No change to the `--since` deprecation path beyond accepting its value in either form.
- No other reserved `meta` keys introduced in this PRD. `task_status` is the only key the watcher reads; callers can stuff whatever else they like in `meta_patch` but the watcher ignores it.
- No configurable throttle timeout. The 5-minute deadline is a hard-coded constant; making it configurable can come later when there is evidence of tasks legitimately exceeding it.
- No retry, redelivery, or dead-letter semantics for throttled-but-never-acked messages. Rows sit at `status=NEW` on the server; on resume (via DONE or timeout) they re-appear in the next poll and are re-emitted.
- No cross-process coordination. The throttle lives entirely in the watcher process — two concurrently running watchers against the same mailbox would each throttle independently. This is acceptable because `/steop:st-watch` is a single-instance skill.
- No MCP tool surface for `update_meta`. HTTP-only, consistent with every other mailbox RPC. Agents invoke it via the `steop mailbox update-meta` CLI, not via a tool call.

## Background & Motivation

`/steop:st-watch` invokes the companion binary as:

```
steop mailbox watch --type TASK:REQUEST --interval 10
```

That is the canonical POSIX space-separated form and it is exactly what the current SKILL.md writes. But `runMailboxWatch` in `apps/steop/cmd_mailbox_watch.go` parses flags only in the `--flag=value` form — it iterates `range args` and splits each token on `=`. A bare `--type` with the value in the next token is silently ignored: `msgType` stays `""`, and by the time `runMailboxWatch` hands off to the poll loop, the per-poll `MailboxListOptions{MessageType: msgType}` is an unfiltered request.

The consequence: every `NEW` mailbox message is forwarded to stdout regardless of type — `HOOK:Stop`, `HOOK:SessionEnd`, `HOOK:PreToolUse`, everything. PRD-013's Step 2 filter rule in the skill saves the agent from acting on non-task lines, but the watcher is still doing far more work than intended, the stream is noisy for any human observer, and any hook-event mailbox traffic from a busy session floods the monitor.

PRD-012's dual-mailbox polling compounded the blast radius: `msgType` is forwarded to `MailboxListOptions.MessageType` for both `pollIDs`, so the bug silently zeros the filter for **every** poll on **every** mailbox. PRD-013 touched the same file to restore `WATCHER:READY` and the surrounding skill, but kept the space-separated flag form in SKILL.md — the parser bug was never examined.

### Current state

**`apps/steop/cmd_mailbox_watch.go`** (at `runMailboxWatch`, post-PRD-013):

- Local `msgType`, `interval`, and `sinceArg` strings are declared and defaulted.
- `for _, arg := range args { ... }` walks the flag list token by token. Each branch checks `strings.HasPrefix(arg, "--type=")`, `"--interval="`, `"--since="` and slices after the `=`. There is no lookahead into `args[i+1]`. A bare `--type` falls through and is silently discarded.
- `msgType` is then forwarded to `MailboxListOptions{MessageType: msgType}` inside the `poll` closure for both `pollIDs` entries (PRD-012).
- `interval` is parsed via `strconv.Atoi` and clamped `[5, 300]`; the default (10) happens to match what the skill passes, so the space-form bug is invisible for `--interval` today.
- `--since` is a deprecated no-op that just prints a warning; its value is never read, so the space-form bug there is cosmetic at worst.

**`plugins/steop/skills/st-watch/SKILL.md`** (post-PRD-013):

- Step 1 tells the agent to run `steop mailbox watch --type TASK:REQUEST --interval 10` under the `Monitor` tool. This is the exact invocation the parser cannot handle.
- Step 2's explicit `message_type == "TASK:REQUEST"` filter (PRD-013) masks the bug at the agent layer but does not fix the server-side filter or the wasted poll bandwidth.

**Observed symptom.** On any session that has been emitting `HOOK:*` mailbox messages, starting `/steop:st-watch` floods the NDJSON stream with hook events. The agent ignores them (thanks to PRD-013's filter) but the Monitor window still fills, the watcher pays the RPC cost, and any debugging tail (e.g. `jq -c .`) becomes useless.

**Prior PRDs.** No overlap:

- PRD-012 (dual-mailbox polling) forwards `msgType` to each `pollID`; this PRD fixes what `msgType` is.
- PRD-013 (WATCHER:READY + skill filter) is orthogonal: consumer-side filter rule stays as-is; server-side filter comes back.

### Background — emission throttle

Even with Goal 1 landed (server-side `--type` filter restored in v0.12.6), a second, deeper problem surfaces when the peer side is under load. A single `/steop:st-watch` session handling TASK:REQUESTs is LLM-slow: the agent reads the emitted NDJSON line, fans out to a sub-agent pipeline (clarify → research → plan → execute → validate), and only then runs `steop mailbox archive`. In the interval between emission and archive — easily several minutes — the row sits at `status=NEW` on the server. Every subsequent poll re-lists it, re-emits the NDJSON line, and the agent sees a fresh "new task" arrive repeatedly. If the peer has queued multiple TASK:REQUEST rows, they all emit on the first poll, and the agent spawns parallel pipelines it can neither coordinate nor undo.

The underlying cause is that mailbox `status` only has two load-bearing server-side values (`NEW` and `ARCHIVE`), and the watcher cannot flip `NEW` itself without racing the archive lifecycle. We need a lighter per-message signal that the watcher can read without writing, orthogonal to `status` — something the agent sets when it has accepted ownership of the task, and that the watcher respects as a "don't re-emit, don't move on" gate.

The chosen mechanism: `meta.task_status` (a reserved key inside the existing `meta` JSON column) set to `"DONE"` by the agent right before archive. An in-process gate in the watcher suspends stdout emission after sending one `TASK:REQUEST` NDJSON line, and lifts when the poll observes `meta.task_status == "DONE"` on the held row — or when a 5-minute fallback deadline elapses, whichever is first. Polling continues normally during suspend so the gate can observe the state transition; only stdout is gated. This keeps the server as the single source of truth (the status=NEW row stays queryable, acting as the queue), avoids introducing a new watcher-side queue, and survives watcher restarts — a fresh watcher will see the un-archived row on first poll, re-emit it once, and throttle again.

## Design

**Rewrite `runMailboxWatch`'s flag loop to index-based iteration with a shared `flagVal` closure that accepts both `--flag=value` and `--flag value` forms, update SKILL.md to the defensive equals form, add a shallow-merge `mailbox.update_meta` RPC and CLI subcommand, and gate stdout emission in the watcher behind a `meta.task_status == "DONE"` ack or a 5-minute deadline.**

1. **Parser** (`apps/steop/cmd_mailbox_watch.go`, `runMailboxWatch`) — _shipped in v0.12.6_:

   - Replace `for _, arg := range args` with index-based `for i := 0; i < len(args); i++`.
   - Introduce a single closure `flagVal(name string) (string, bool)` that inspects `args[i]`:
     - If `args[i] == "--"+name+"=..."`, return the slice after `=`.
     - Else if `args[i] == "--"+name`, advance `i` and return `args[i]` (when in range); otherwise report error.
   - Use `flagVal` for `--type`, `--interval`, and `--since` branches. An unrecognized token falls through to the existing error/help path unchanged.
   - The closure advances the loop index by reference (via pointer or returned delta). Simplest: return `(value, consumedExtra bool)`; caller bumps `i` accordingly.
   - Missing value (`--type` as the last token with no follower) returns a clear `fmt.Errorf("flag --%s requires a value", name)` — same error shape the rest of the CLI uses.

2. **Skill — parser defensive form** (`plugins/steop/skills/st-watch/SKILL.md`) — _shipped in v0.12.6 for the equals-form change; ack contract is new in v0.13.0 (see item 6)_:

   - Change the Step 1 invocation from `steop mailbox watch --type TASK:REQUEST --interval 10` to `steop mailbox watch --type=TASK:REQUEST --interval=10`.
   - Rationale: defensive. Users may still have an older `steop` binary (v0.12.5 or earlier) on `PATH` after pulling the plugin. The equals form worked before and still works after this fix, so the skill is forward- and backward-compatible.

3. **Version bump.** Minor (`0.12.6 → 0.13.0`): new additive RPC + new watcher behavior. Bump per `docs/versioning.md`:
   - `apps/stele/Cargo.toml` (workspace — server + CLI in lockstep)
   - `apps/steop/version.go`
   - `plugins/steop/.claude-plugin/plugin.json`
   - `plugins/stele/.claude-plugin/plugin.json` (CI enforces match with `apps/stele/Cargo.toml`)

   Use `scripts/bump-version.py 0.13.0`.

4. **`mailbox.update_meta` RPC** (`apps/stele/crates/stele-server/src/steop_api.rs` + `apps/stele/crates/stele-server/src/db.rs`):

   - New route `POST /api/v1/steop/mailbox.update_meta`, registered alongside the existing `mailbox.*` routes.
   - Request body: `{ "id": "<composite-id>", "message_id": <i64>, "meta_patch": <json object> }`. Define a private `MailboxUpdateMetaReq { id, message_id, meta_patch }` struct in `steop_api.rs` next to `MailboxRowReq` (currently at line 266). Response body: the existing `SteopMailboxRow` (db.rs:1307-1320), so the caller sees the merged `meta` and can sanity-check their write. No new response type.
   - **Rationale for keeping types server-private, not promoting to `stele-common`:** existing mailbox RPC types (`MailboxRowReq`, `SteopMailboxRow`, `MailboxListOptions`) are all private to the `stele-server` crate today — the Go `steop` client speaks the wire format directly via its own `internal/client` structs. Mirroring that convention avoids a cross-crate surface change for a single new endpoint and keeps the server free to iterate on shapes without bumping `stele-common`.
   - **SQL technique:** read-modify-write inside a single transaction, reusing the same JSON helpers `steop_state_put` uses today (db.rs:1565-1602). Concretely: `SELECT meta FROM steop_mailbox WHERE id = ?1 AND message_id = ?2`, run it through `steop_parse_json` (db.rs:1326), shallow-merge `meta_patch` via `steop_json_merge` (db.rs:1330), then `UPDATE steop_mailbox SET meta = ?1 WHERE id = ?2 AND message_id = ?3`. Do NOT touch `status`, `created_at`, or any other column. Return the re-SELECTed row as `SteopMailboxRow`.
   - **Errors:**
     - `err400` — id grammar invalid (same helper the other mailbox handlers call).
     - `not_found()` — no row matches `(id, message_id)`.
     - `err500` — any SQL failure surfaced from the transaction.
     - No status-transition guard. The brief explicitly leaves `status` alone; updating meta on an `ARCHIVE` row is allowed and useful (lets callers annotate post-mortem metadata).
   - Shallow merge semantics match `steop_json_merge`: top-level keys in the patch overwrite corresponding keys in the existing `meta`; keys present only in the existing `meta` are preserved; nested objects are replaced wholesale, not deep-merged.

5. **Watcher emission throttle** (`apps/steop/cmd_mailbox_watch.go`, `runMailboxWatch`):

   - Three new locals inside `runMailboxWatch`, scoped to the poll loop: `throttleActive bool`, `throttleMsgID int64`, `throttleDeadline time.Time`.
   - New constant at the top of the file: `const throttleTimeout = 5 * time.Minute`.
   - Gate placement: inside the existing `for _, m := range msgs` emission loop, before the `fmt.Fprintln(os.Stdout, ...)` call for any row whose `message_type == "TASK:REQUEST"`.
     - If `throttleActive` is true, check `time.Now().After(throttleDeadline)` → if yes, clear the throttle (log a one-line stderr note: `watcher: throttle timeout for message_id=<N>, resuming`) and fall through to emit.
     - Else, inspect the current batch for the held `throttleMsgID` — if present, read `meta.task_status`; if it equals `"DONE"`, clear the throttle (log `watcher: DONE ack received for message_id=<N>, resuming`) and fall through. If the held row is no longer in the batch at all (e.g. archived without the ack), clear the throttle as well — naked archive still means "move on."
     - If throttle is still active after those checks, skip the stdout emission for this row (and any subsequent rows in the batch).
   - On emit, set `throttleActive = true`, `throttleMsgID = m.MessageID`, `throttleDeadline = time.Now().Add(throttleTimeout)`. Only one message flips the throttle per poll batch; subsequent TASK:REQUEST rows in the same batch are deferred (they remain `status=NEW` server-side and will re-appear next tick).
   - **Meta inspection helper:** `MailboxMessage.Meta` is typed as `interface{}` in `apps/steop/internal/client/mailbox.go:12`. Add a private helper in `cmd_mailbox_watch.go`:
     ```go
     func metaTaskStatus(m MailboxMessage) string {
         obj, ok := m.Meta.(map[string]interface{})
         if !ok { return "" }
         s, _ := obj["task_status"].(string)
         return s
     }
     ```
     Unknown shapes return `""`, which is treated as "not DONE" by the gate.
   - **No `time.AfterFunc` / `time.After`.** The codebase uses deadline-style `time.Time` comparisons everywhere (grep confirms no existing uses of either). Keep that convention; avoids leaking timers on loop exit.
   - **Suppressed messages are not queued in memory.** They stay `status=NEW` on the server and re-appear on every poll as long as the status holds. The server is the queue.
   - **Polling continues at the configured interval** — the per-poll RPC pair is not gated. Only the stdout `fmt.Fprintln` is gated. This is required so the resume condition (the `meta.task_status=="DONE"` on the held row, or the row disappearing) can actually be observed.
   - **Interaction with PRD-012 dual-mailbox polling:** the `seen` map already collapses duplicate `message_id`s across the two `pollIDs`. The throttle reads from the merged set, so a TASK:REQUEST that lands in both mailboxes still only flips the throttle once.
   - **Interaction with PRD-013 WATCHER:READY:** the ready line is emitted once before the first poll and has no `message_type == "TASK:REQUEST"` check around it. The throttle gate does not apply. No change.

6. **CLI subcommand** (`apps/steop/cmd_mailbox.go` + `apps/steop/internal/client/mailbox.go`):

   - Add dispatch entry in `cmd_mailbox.go` (alongside `read`, `archive`, `list`, etc. at lines 14-34): a `"update-meta"` case that calls `runMailboxUpdateMeta(args[1:])`.
   - Implement `runMailboxUpdateMeta(args []string) error` modeled on the existing `runMailboxRead`: positional args `<message_id> <meta-json>`, with `--id` / `-x-session-id` flag handling consistent with the rest of the subcommands. Parse `message_id` as `int64`, `json.Unmarshal` the JSON argument into `interface{}`, call the new client method, print the response row as JSON to stdout.
   - Add client method in `apps/steop/internal/client/mailbox.go`:
     ```go
     func (c *Client) MailboxUpdateMeta(id string, messageID int64, metaPatch interface{}) (*MailboxMessage, error)
     ```
     Mirrors the call shape of the existing `MailboxRead`: POST to `/api/v1/steop/mailbox.update_meta`, body `{id, message_id, meta_patch}`, decode response into `MailboxMessage`.

7. **Skill ack contract** (`plugins/steop/skills/st-watch/SKILL.md`):

   - Insert a new numbered bullet at Step 2e, immediately **before** the existing `steop mailbox archive <message_id>` line:
     ```
     e. steop mailbox update-meta <message_id> '{"task_status":"DONE"}'
     ```
     Renumber the subsequent `archive` step accordingly.
   - Add a one-paragraph rationale under the step list explaining that the ack lifts the watcher's emission throttle so the next TASK:REQUEST can surface, and that archive is still the end-of-life signal — both calls happen, in that order.
   - No other text changes. The PRD-013 filter rule and Step 1 invocation stay put.

**Why index-based loop + closure, not `flag.FlagSet`.** The rest of `steop` hand-rolls flag parsing for consistency across subcommands (`cmd_mailbox_watch.go` is one of several). Introducing `flag.FlagSet` here would diverge from the convention and invite partial rewrites of every other `cmd_*.go`. Out of scope. _(This rationale applied to Goal 1, which is already shipped in v0.12.6.)_

**Why fix the skill too, not just the parser.** Users install the plugin and the binary separately (`/steop:install` is a manual step). Until every user rebuilds, some `steop` binaries in the wild will still mis-parse. The equals form is a single-character change in SKILL.md that works on both old and new binaries.

**Why not add a regression test.** `apps/steop/` has no Go test suite today (per repo conventions, there are no tests yet). Adding one here would be the first test file and belongs in its own PRD. The manual smoke tests below are sufficient to verify the fix.

## Changes by Component

| Component                                          | Change                                                                                                                                                                                                                                                                                                             |
| -------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `apps/steop/cmd_mailbox_watch.go`                  | (Already shipped v0.12.6) Index-based flag loop with `flagVal` closure accepting both forms. **New:** add `throttleActive`/`throttleMsgID`/`throttleDeadline` locals, `throttleTimeout` constant, `metaTaskStatus` helper, and gate the `TASK:REQUEST` stdout emission behind the throttle state machine described in Design item 5. |
| `apps/stele/crates/stele-server/src/steop_api.rs`  | Register new `POST /mailbox.update_meta` route. Add private `MailboxUpdateMetaReq { id, message_id, meta_patch }` struct next to `MailboxRowReq` (line 266). Handler validates id grammar (`err400`), dispatches to `db::steop_mailbox_update_meta`, maps `None` to `not_found()`, any `Err` to `err500`. Returns `SteopMailboxRow`. |
| `apps/stele/crates/stele-server/src/db.rs`         | Add `steop_mailbox_update_meta(id, message_id, meta_patch)` fn. Inside a single transaction: SELECT current `meta` by `(id, message_id)`, run through `steop_parse_json` + `steop_json_merge`, UPDATE `meta` column only, re-SELECT and return as `SteopMailboxRow`. Returns `Option<SteopMailboxRow>` (None on no match). Never touches `status`. |
| `apps/steop/internal/client/mailbox.go`            | Add `(*Client).MailboxUpdateMeta(id string, messageID int64, metaPatch interface{}) (*MailboxMessage, error)` mirroring `MailboxRead`'s call shape. POSTs to `/api/v1/steop/mailbox.update_meta`, decodes `SteopMailboxRow` wire format into `MailboxMessage`.                                                    |
| `apps/steop/cmd_mailbox.go`                        | Add `"update-meta"` dispatch case in the subcommand switch (around lines 14-34). Implement `runMailboxUpdateMeta` modeled on `runMailboxRead`: parse `message_id` int64, unmarshal JSON patch, call `MailboxUpdateMeta`, print result.                                                                            |
| `plugins/steop/skills/st-watch/SKILL.md`           | (Already shipped v0.12.6) Step 1 invocation uses equals form. **New:** insert Step 2e `steop mailbox update-meta <message_id> '{"task_status":"DONE"}'` immediately before the archive step, with a short rationale paragraph explaining the throttle contract.                                                                                                  |
| `apps/steop/version.go`                            | `0.12.6` → `0.13.0`.                                                                                                                                                                                                                                                                                              |
| `plugins/steop/.claude-plugin/plugin.json`         | `0.12.6` → `0.13.0`.                                                                                                                                                                                                                                                                                              |
| `apps/stele/Cargo.toml` (workspace)                | `0.12.6` → `0.13.0`. Propagates to both `stele-server` and `stele-cli` crate manifests via the workspace `version` inheritance.                                                                                                                                                                                   |
| `plugins/stele/.claude-plugin/plugin.json`         | `0.12.6` → `0.13.0`. CI enforces this matches the Cargo workspace version.                                                                                                                                                                                                                                        |
| `docs/README.md`                                   | Update PRD-014 row title + description to reflect expanded scope (parser fix + emission throttle).                                                                                                                                                                                                                 |

## Edge Cases

- **Equals form (`--type=TASK:REQUEST`) already works.** Still works after the Goal 1 rewrite — `flagVal` checks the `--flag=` prefix first. No regression.
- **Space form for `--interval`.** Was silently using the default (10) before when users wrote `--interval 10`. The default happens to be `10`, so the fix is functionally invisible in that one case. For any other value (e.g. `--interval 30`), the fix is the difference between "what the user asked for" and "what the parser silently substituted."
- **`--since` is deprecated.** The branch just prints a warning and ignores the value. Accepting the space form is trivial and keeps it consistent with `--type`/`--interval`.
- **`--type` is the final token with no follower.** New parser returns a clear error ("flag --type requires a value") instead of silently treating it as empty. Old parser would have silently left `msgType = ""`.
- **Unknown flag.** Falls through to the existing error/help path unchanged. Not in scope for this PRD.
- **Flag value that looks like another flag (e.g. `--type --interval`).** New parser treats `--interval` as the value of `--type` and fails downstream validation (server rejects `--interval` as a `message_type`). Acceptable — users writing `--type --interval` are making a nonsense request; the equals form avoids the ambiguity.
- **SKILL.md change on older binaries.** Equals form has been supported since the command was introduced. No binary older than v0.12.6 breaks on `--type=TASK:REQUEST`. Forward/backward compatible.
- **PRD-012 dual-mailbox polling.** `msgType` is now the user-supplied value for both `pollIDs` entries. `MailboxListOptions{MessageType: "TASK:REQUEST"}` filters each server-side. No change to the dual-polling semantics.
- **PRD-013 WATCHER:READY.** The ready line is not a mailbox message; it is emitted directly by the watcher before the first poll. Server-side `--type` filter does not affect it. Skill-side `message_type == "TASK:REQUEST"` filter (PRD-013) still ignores it. No interaction with the throttle either — the gate only guards `TASK:REQUEST` rows, and the ready line is emitted outside the poll loop.
- **`update_meta` on an ARCHIVE row.** Allowed by design. The handler does not inspect `status`; shallow-merging `meta` on an archived row is a useful post-mortem annotation path (e.g. marking outcome) and has no side effect on the throttle (archived rows are not re-emitted regardless of meta).
- **Throttle clear via naked archive vs meta.task_status=DONE.** Both unblock the watcher. If the held row's `message_id` drops out of the poll batch entirely (row archived without the ack), the gate treats that as implicit completion and resumes. The 5-minute deadline is the final fallback if neither happens (e.g. agent crash mid-task).
- **Duplicate TASK:REQUEST on two pollIDs.** The existing `seen map[int64]struct{}` (from PRD-012) collapses the merged batch before the throttle gate inspects it. A single message_id flips the throttle exactly once even if both the 2-seg and 3-seg mailboxes return it.
- **Meta visibility requires Go-side inspection.** The server does not filter or project `meta` in list responses; it returns the full JSON column. The Go watcher type-asserts and reads `task_status` on the client. No server-side change needed to expose the field.
- **Shallow merge overwrites nested objects wholesale.** Documented in Design item 4. If a caller wants to preserve a nested `meta.diagnostics.timings` sub-object while updating `meta.diagnostics.phase`, they must send the full `diagnostics` object in the patch. This matches the semantics of `steop_state_put`.
- **Watcher restart mid-throttle.** In-process state is lost; the next `runMailboxWatch` process sees the held row on its first poll (still `status=NEW`, regardless of `task_status` setting) and re-emits it, then throttles again. The agent may receive a duplicate TASK:REQUEST NDJSON line across restarts — this is inherent to not persisting throttle state and is acceptable given the `/steop:st-watch` single-instance contract.

## Migration

No schema migration required.

- No schema change: `meta` column already exists on `steop_mailbox` (JSON TEXT, nullable). `steop_json_merge` and `steop_parse_json` helpers are already registered in `db.rs`.
- Minor bump (`v0.12.6 → v0.13.0`): new additive RPC and new watcher behavior. No breaking changes to existing contracts.
- **Deploy order matters loosely.** `stele-server` must be rebuilt and redeployed before `steop` clients rely on `mailbox.update_meta`. The compatibility matrix:
  - Old `steop` binary against new `stele-server`: works unchanged. The old binary never calls `update_meta` so the new route is inert.
  - New `steop` binary against old `stele-server`: the `steop mailbox update-meta` CLI returns HTTP 404 from the server; the watcher's throttle gate cannot observe a DONE ack and will resume via the 5-minute timeout. End-to-end functionality is degraded (each task takes a 5-minute cool-down) but not broken.
  - New on new: full throttle semantics, sub-second resume on ack.
- Users must rebuild both:
  - `stele-server` / `stele` CLI via `cd apps/stele && cargo build --release` and redeploy.
  - `steop` companion binary via `/steop:install` (or `cd apps/steop && go build -o target/steop . && rm -f ~/.local/bin/steop && cp target/steop ~/.local/bin/steop`). macOS Tahoe SIGKILL workaround applies: always `rm` before `cp`.
- **Docs update (follow-up, not blocking):** `docs/stele/http-api.md:422-431` documents the mailbox RPC block with the correct `{id, ...}` shape. Adding the new `mailbox.update_meta` row to that doc slots in cleanly and will happen as part of implementation, not as a PRD deliverable.

## Testing

Manual smoke tests. Run from `apps/steop/` (for steop-side) or `apps/stele/` (for server-side). Tests 1-11 are Goal-1 regression coverage carried over from the v0.12.6 ship and still apply; tests 12-18 cover the new RPC, throttle, and skill ack contract.

1. **Build and install.** `go build -o target/steop . && rm -f ~/.local/bin/steop && cp target/steop ~/.local/bin/steop`.
2. **Space form accepted.** `steop mailbox watch --type TASK:REQUEST --interval 10`. First stdout line must be `{"message_type":"WATCHER:READY","interval":10}` (confirms `--interval 10` parsed as `10`, not the default). Send a non-task message (e.g. a `HOOK:Stop` row written directly via `steop storage put` or a peer's hook) — it must NOT appear on the stream.
3. **Equals form still works.** `steop mailbox watch --type=TASK:REQUEST --interval=10`. Identical behavior to test 2.
4. **Mixed forms.** `steop mailbox watch --type=TASK:REQUEST --interval 30`. Ready line must report `"interval":30`.
5. **Server-side filter engaged.** Start a peer that emits both `TASK:REQUEST` and `HOOK:Stop` rows into the same mailbox. The watcher stream must contain only `TASK:REQUEST` lines (plus the leading `WATCHER:READY`). Confirm by piping to `jq -c 'select(.message_type != "TASK:REQUEST" and .message_type != "WATCHER:READY")'` — output must be empty.
6. **Missing value error.** `steop mailbox watch --type`. Exits with a clear error message naming `--type`; no stdout emitted.
7. **Interval clamp preserved.** `steop mailbox watch --type=TASK:REQUEST --interval 500` — ready line reports `"interval":300` (PRD-010 clamp). `steop mailbox watch --type=TASK:REQUEST --interval 1` exits with status 2 before any stdout.
8. **`--since` space form.** `steop mailbox watch --type=TASK:REQUEST --since 2026-04-01T00:00:00Z`. Prints the deprecation warning, proceeds normally — confirms the space form is tolerated for the deprecated flag too.
9. **Skill end-to-end.** Run `/steop:st-watch` from Claude Code. Verify it invokes `steop mailbox watch --type=TASK:REQUEST --interval=10` (new equals form in SKILL.md). Send a `TASK:REQUEST` from a peer via `/steop:st-send`; verify Step 2 flow fires. Confirm no `HOOK:*` lines surface even under an active session emitting hook events.
10. **Regression on PRD-012.** With an active UUID session, start the watcher without `--x-session-id`. Send a task to the 2-segment project ID and another to the 3-segment session ID; both arrive as separate `TASK:REQUEST` lines. No `HOOK:*` bleed-through from either mailbox.
11. **Regression on PRD-013.** Ready line emits exactly once, ahead of any task line, with correct `interval` reflecting the parsed value.
12. **`update_meta` happy path via curl.**
    ```
    curl -sS -X POST http://localhost:3100/api/v1/steop/mailbox.update_meta \
      -H 'Content-Type: application/json' \
      -d '{"id":"host:proj","message_id":42,"meta_patch":{"task_status":"DONE","note":"ok"}}'
    ```
    Response is 200 with the full `SteopMailboxRow`. `meta.task_status == "DONE"`, `meta.note == "ok"`, any pre-existing `meta` keys are preserved, `status` column is unchanged. Re-run with a second patch `{"meta_patch":{"note":"updated"}}` — confirms shallow merge (note overwritten, task_status preserved).
13. **`update_meta` 404 on unknown message_id.** Same shape, `message_id: 999999`. Server returns 404 (not 500, not 200 with nulls).
14. **`update_meta` on ARCHIVE row.** Archive a row, then call `update_meta` on it with `{"outcome":"success"}`. Returns 200, merged meta reflects the new key, row's `status` remains `ARCHIVE`.
15. **Watcher emits one TASK:REQUEST then suspends.** Peer sends three TASK:REQUEST rows in quick succession. Watcher stdout contains exactly one TASK:REQUEST line (the first one, after WATCHER:READY). Confirm via `jq -c 'select(.message_type == "TASK:REQUEST")' | wc -l` → 1. The other two remain `status=NEW` server-side (verify with `steop mailbox list`).
16. **Watcher resumes after DONE ack.** Continuing from test 15: call `steop mailbox update-meta <first_message_id> '{"task_status":"DONE"}'`. Within one poll interval, stderr prints `watcher: DONE ack received for message_id=<N>, resuming`. Next TASK:REQUEST line appears on stdout. Archive the first row, repeat for the second — each task produces exactly one in-flight emission.
17. **Watcher resumes via 5-min timeout.** Start fresh watcher. Peer sends one TASK:REQUEST. Watcher emits it and suspends. Do nothing (no ack, no archive). After ~5 minutes, stderr prints `watcher: throttle timeout for message_id=<N>, resuming`. Next peer TASK:REQUEST emits. (Use `throttleTimeout = 30 * time.Second` under a debug build flag if you want a faster test cycle; revert before commit.)
18. **SKILL.md end-to-end (Monitor + st-send exactly-one in-flight).** From Claude Code session A, run `/steop:st-watch`. From session B, fire `/steop:st-send` three times in succession with three distinct task descriptions. Session A's Monitor window shows task 1 fire its pipeline (clarify → … → validate → update-meta DONE → archive). Only after archive completes does task 2 appear on the Monitor stream. Confirm no overlapping pipelines ever run in session A. Total time ≈ 3 × per-task latency, not 1× with parallelism.
