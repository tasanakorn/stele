# PRD-021 — Stop hooks from posting HOOK:* mailbox rows

**Status:** Implemented (v0.16.1)
**Target version:** v0.16.1
**Scope:** `apps/steop/internal/hooks/stop.go`, `apps/steop/internal/hooks/session_end.go`, `docs/steop/DESIGN.md`
**Author:** Tasanakorn (design) + Claude Code (PRD authoring)

---

## 1. Goals

1. Drop the `HOOK:Stop` mailbox post from `HandleStop`. Keep `steop.notify`.
2. Drop the `HOOK:SessionEnd` mailbox post from `HandleSessionEnd`.
3. Confine steop → stele-server mailbox traffic to the task pipeline: `/steop:st-send` (`TASK:REQUEST`), `/steop:st-watch` (`MailboxList` poll), `watcher_cleanup` (`TASK:FAILED`), plus the manual `steop mailbox *` CLI.
4. Update `docs/steop/DESIGN.md` §7 hook taxonomy and §5 / §5.1 namespace wording to match the new reality.

## 2. Non-goals

- Server endpoints. `steop.mailbox.*` and `steop.notify` are unchanged.
- Local SQLite (`~/.local/share/steop/steop.db`) schema and behavior. Session logs, state, storage, and the `session_end` local log row are all untouched.
- Identity composition (`host:project_dir[:UUID|:USER]`) and the closed 3rd-segment set.
- `TASK:*` contract on the mailbox surface.
- `apps/steop/cmd_mailbox.go` CLI — operators may still post `HOOK:*` by hand.
- Watcher cleanup `TASK:FAILED` sends from `cleanupWatcherTasks` — these are task-pipeline traffic, not hook-lifecycle.
- Other hook handlers (`HandlePreCompact`, `HandlePostToolUse`, etc.) — they already do not touch the mailbox.

## 3. Background & Motivation

[PRD-020](prd-020-steop-local-backend.md) moved session lifecycle, state, storage, and event logs to a local SQLite database at `~/.local/share/steop/steop.db`. The remaining stele-server surface narrowed to mailbox + notify only (`steop_mailbox` is the sole surviving `steop_*` table). In that context the two leftover `HOOK:*` mailbox posts — `HOOK:Stop` and `HOOK:SessionEnd` — are the only lifecycle signals still written to stele-server.

### Current state

Two call sites emit `HOOK:*` rows today:

- `apps/steop/internal/hooks/stop.go:62-68` — inside the `state != nil` branch, after `c.Notify(req)` and before `cleanupWatcherTasks`, `HandleStop` posts a `HOOK:Stop` row carrying `{cwd, data, counters, ended_at}`.
- `apps/steop/internal/hooks/session_end.go:62-68` — after the local `session_end` log append, `HandleSessionEnd` posts a `HOOK:SessionEnd` row carrying `{cwd, reason, transcript_path, [resolved_project_dir], [data, counters]}`.

Neither row has a consumer. Grep of `HOOK:Stop` and `HOOK:SessionEnd` across the repo returns only the two producer sites plus doc mentions. `st-watch` filters mailbox poll results to `TASK:REQUEST` and explicitly treats `HOOK:*` as noise (see prd-001 / prd-014 noise-filter fixtures). Removing the posts is observationally invisible to every existing consumer.

These posts are also the only remaining steop → stele write traffic that is not driven by the explicit task pipeline. Trimming them keeps the production-path network surface aligned with the PRD-020 principle: session-lifecycle signals are local, cross-agent task messages go through the mailbox.

## 4. Design

Two-step edit, no new code paths, no schema change:

1. Delete the `c.MailboxSend(..., MessageType: "HOOK:Stop", ...)` block in `HandleStop` and strip the now-dead `state != nil` branch that only exists to build the mailbox payload. `c.Notify(req)`, `sessionIdent`, `cleanupWatcherTasks`, `StorageDelete("watcher:state"|"watcher:heartbeat")`, and the phase/mode `StatePut` clear remain.
2. Delete the `c.MailboxSend(..., MessageType: "HOOK:SessionEnd", ...)` block in `HandleSessionEnd` and strip the now-dead `StateGet` + payload-assembly block. `db.LogAppend(ctx, id, "session_end", ...)`, `cleanupWatcherTasks`, and `db.SessionStop` remain.

After the edits both handlers are local-only plus — for `HandleStop` — the `steop.notify` desktop notification. No mailbox traffic originates from session lifecycle.

DESIGN.md §7 is updated so the `Stop` row lists `Local + Stele (notify only)` and the `SessionEnd` row becomes `Local`. §5 / §5.1 `HOOK:*` namespace is downgraded to "reserved but unused as of v0.16.1" — no hook emits it (`HandlePreCompact` is also Local-only).

## 5. Changes by Component

| Component                                       | Change                                                                                                                                                                                                                                                                                                                 |
| ----------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `apps/steop/internal/hooks/stop.go`             | Remove the `c.MailboxSend(...)` posting `HOOK:Stop` (lines 62-68). Remove the dead `state != nil` block that only existed to assemble the mailbox payload (lines 41-72): `StateGet`, `data`/`counters` unmarshal, `payload` map, `subject`, and the `persistent_mode` debug log. Keep `sessionIdent`, `cleanupWatcherTasks`, both `StorageDelete` calls, and the final phase/mode `StatePut`. `c.Notify(req)` stays. |
| `apps/steop/internal/hooks/session_end.go`      | Remove the `c.MailboxSend(...)` posting `HOOK:SessionEnd` (lines 62-68). Remove the dead `StateGet` + payload-assembly block (lines 34-61): `state`, `payload`, `ProjectDirResolved`, `subject`. Keep `db.LogAppend("session_end", ...)`, `cleanupWatcherTasks`, and `db.SessionStop`.                                |
| `docs/steop/DESIGN.md` §7 (hook taxonomy table) | `Stop` row: Backends becomes `Local + Stele (notify only)`; Behavior drops `steop.mailbox.send`, keeps `steop.notify`. `SessionEnd` row: Backends becomes `Local`; Behavior drops the `steop.mailbox.send` clause.                                                                                                      |
| `docs/steop/DESIGN.md` §5.1 `message_type` vocabulary | `HOOK:*` bullet changed to "reserved but unused as of v0.16.1 (no hook emits it — `HandleStop` notifies only, `HandleSessionEnd` logs locally only, `HandlePreCompact` logs locally only)".                                                                                                                            |
| `docs/README.md` PRD table                      | Add a row for PRD-021 (Proposed, one-line description).                                                                                                                                                                                                                                                                |
| Workspace version                               | `python scripts/bump-version.py patch` — moves `apps/stele/Cargo.toml`, `plugins/stele/.claude-plugin/plugin.json`, `plugins/steop/.claude-plugin/plugin.json`, `apps/steop/version.go`, `Cargo.lock` from `0.16.0` → `0.16.1` in lockstep. CI validates plugin ↔ workspace match.                                      |

## 6. Edge Cases

- **Empty `HOOK:*` namespace.** After this PRD no handler emits a `HOOK:*` row. DESIGN.md §5.1 marks the namespace "reserved but unused as of v0.16.1" rather than retiring it: the grammar is still server-legal, the manual `steop mailbox send` CLI can still emit any `message_type`, and operators may want the prefix for future lifecycle signals. Reserving leaves that door open at zero cost.
- **Dead `state != nil` branches.** The `StateGet` + payload-assembly blocks in both handlers only exist to feed the removed `MailboxSend`. The executor MUST delete them together with the send so `go vet` / unused-variable checks stay clean. In `stop.go` that includes the `persistent_mode set but not honored in v1` debug log at lines 69-71 (which references `data` unmarshalled solely for the mailbox payload) — it disappears naturally and is not re-added elsewhere.
- **Pre-existing DESIGN.md §7 drift.** The current `Stop` row claims the handler "appends stop log", but `stop.go` has no `LogAppend` call. This is an independent doc bug, out of scope for PRD-021. If DESIGN.md §7 is being rewritten anyway during the Stop-row edit, the executor MAY trivially drop the "append stop log" phrase in the same pass; otherwise leave it for a dedicated docs fix.
- **Historical PRD references.** `docs/prd/prd-002-mailbox-watcher.md` (~lines 303-313, 435-437) describes `HOOK:Stop` / `HOOK:SessionEnd` as producers. Per convention, implemented PRDs stay as shipped — no edit. `prd-001-mailbox-v2.md` and `prd-014-mailbox-watch-flag-parsing.md` cite `HOOK:*` as noise-filter examples, still factually defensible post-PRD, no edit.
- **Operators with stale `HOOK:*` rows.** Rows written by older `steop` builds stay in the mailbox until archived. `st-watch` already filters them out. See §7.

## 7. Migration

No migration. The removed call sites were write-only with no readers — every `HOOK:Stop` / `HOOK:SessionEnd` row ever posted has been consumed by nothing. Operators may see older rows in `/api/v1/steop/mailbox.list` history until archived naturally; no action is required. No server endpoint changes, no schema changes, no client-wire-format change.

Operators running mixed versions (one host on v0.16.0 hook binary, another on v0.16.1) will see a clean, monotonic reduction in `HOOK:*` traffic as hosts upgrade. No flag day.

## 8. Testing

No existing test assertion breaks. `apps/steop/internal/hooks/stop_test.go` only exercises `buildBody` and `defaultTitle`; both remain in place. No test references `HOOK:Stop` or `HOOK:SessionEnd`.

Suggested smoke check after the executor lands the change:

1. Build and install `steop` via `/steop:install`.
2. Start a Claude Code session in a project bootstrapped with steop, so `SessionStart` → `Stop` fires at least once.
3. End the session (trigger `Stop` and `SessionEnd`).
4. Query the server:
   ```bash
   curl -s http://localhost:3100/api/v1/steop/mailbox.list \
     -H 'Content-Type: application/json' \
     -d '{"id":"<host>:<project_dir>","status":["NEW","READ","ARCHIVE"],"limit":50}' \
     | jq '.messages[] | select(.message_type | startswith("HOOK:")) | .message_type'
   ```
5. Expect no `HOOK:Stop` or `HOOK:SessionEnd` rows created after the upgrade. Pre-existing rows from older binaries are acceptable and can be archived manually.

Also confirm the desktop notification still fires on `Stop` (exercises the retained `c.Notify(req)` path) — a macOS user should see the "Claude Code · <project>" banner.
