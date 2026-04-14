# Hook Event Gap: steop vs cerbrix vs omc

Companion to [`gap-analysis.md`](gap-analysis.md) — a deep dive on Axis 3 (hooks). Snapshot date: **2026-04-11** (initial) / **2026-04-11** (v0.5.0 closure).

**Update (v0.5.0):** steop now intercepts **all 11** Claude Code hook events, matching omc's surface. As of v0.16.0, most events are log-only and persist to the local SQLite DB at `~/.local/share/steop/steop.db` (see [local-storage.md](local-storage.md)); Stop and SessionEnd additionally post session summaries to the stele mailbox via `steop.mailbox.send`. The matrix below reflects the new state.

## Matrix

| Event                | steop (v0.5.0)                                | cerbrix                      | omc                                       |
| -------------------- | --------------------------------------------- | ---------------------------- | ----------------------------------------- |
| `SessionStart`       | log                                           | —                            | load project memory + wiki                |
| `UserPromptSubmit`   | session sentinel + keyword skill inject       | keyword inject               | keyword detect + skill inject             |
| `PreToolUse`         | Bash deny regexes                             | Bash deny regexes            | all-matcher tool enforcer                 |
| `PermissionRequest`  | allow-through stub                            | —                            | Bash permission interceptor               |
| `PostToolUse`        | counter + state update + log                  | state update                 | verify + auto-save to project memory      |
| `PostToolUseFailure` | log                                           | —                            | failure logging                           |
| `SubagentStart`      | log (agent_id, type, model)                   | —                            | lifecycle tracker                         |
| `SubagentStop`       | log (agent_id, output truncated, success)     | —                            | track stop + verify deliverables          |
| `PreCompact`         | log                                           | —                            | flush memory before compaction            |
| `Stop`               | desktop notify + inbox summary + state clear  | save to inbox                | context-guard + persistent-mode           |
| `SessionEnd`         | log + inbox summary                           | archive session              | teardown + wiki sync                      |

See `plugins/steop/hooks/hooks.json` for steop's current wiring. Log events are queryable locally against the `steop.db` SQLite file (see [local-storage.md](local-storage.md) for schema and CLI access); mailbox envelopes via `POST /api/v1/steop/steop.mailbox.list`. Every record carries `host` + `project_dir` for cross-machine identity.

## Events steop is missing — fire conditions and use cases

### `SessionStart` — session opens
Fires when Claude Code opens a new session (new terminal, new project, fresh chat).

- **omc**: loads `project-memory.json` and wiki snapshot into context. Has matchers `init` (first-time setup, 30s timeout) and `maintenance` (scheduled runs, 60s).
- **Potential steop use**: auto-run `/stele:sync` — pull latest shared memories + knowledge graph into the session without the user typing the command. Removes a manual step.

### `PermissionRequest` (Bash matcher) — user-approval prompt
Fires when Claude asks the user to approve a Bash command (tool-use confirmation flow).

- **omc**: intercepts and auto-responds via policy before the UI prompt appears.
- **Potential steop use**: auto-approve an allow-list of read-only commands (`git status`, `ls`, `cat` inside project dir) so long pipelines aren't blocked on confirmation modals.
- **Caveat**: conflicts with safety-first stance; out of scope unless an explicit allow-list config is added.

### `PostToolUseFailure` — tool call failed
Fires when any tool returns an error (file not found, command exits non-zero, permission denied, subprocess timeout).

- **omc**: structured failure logging, feeds back into reviewer.
- **Potential steop use**: feed failure signals into the execute-validate retry counter immediately, instead of relying on the reviewer reading stderr after the fact. Today silent tool failures can go unnoticed until Validate.

### `SubagentStart` — subagent spawned
Fires when the parent launches a subagent via the Agent tool.

- **omc**: lifecycle tracking — start timer, record which phase spawned which agent, feed into `trace_timeline` MCP tool.
- **Potential steop use**: record phase→agent mapping in session state so `steop inspect` can show "currently running: researcher × 2 (parallel)".

### `SubagentStop` — subagent returned  ← highest-value gap
Fires when a subagent completes (success or failure).

- **omc**: `verify-deliverables.mjs` inspects the return message and flags empty or malformed output.
- **Potential steop use**: silent agent failures are the #1 pipeline failure mode today. A researcher that returns empty, an executor that claims success without editing files, an architect that produces no plan — none are caught until Validate. A `SubagentStop` hook could detect "agent X returned 0 files touched in Execute phase" and trigger the retry loop immediately instead of wasting another phase.

### `PreCompact` — context window about to compact
Fires right before Claude Code auto-compacts the conversation context.

- **omc**: flushes current mode state + project memory + wiki to disk so the compacted summary can reference it later.
- **Potential steop use**: dump current phase, step, retry counters, Task Brief, and last plan to stele as a memory tagged `#compact-rescue`. Today long pipelines that hit compaction lose pipeline state because the Task Brief and the plan were only in conversation text.

### `SessionEnd` — session closes
Fires when the user exits Claude Code, closes the tab, or the session times out.

- **cerbrix**: archives the active mode to `state/sessions/session-<timestamp>.json`, deletes the active-mode file, aggregates team task results.
- **omc**: session teardown + wiki sync.
- **Potential steop use**: persist a session summary memory to stele — `phase=<final>, mode=<final>, tool_calls=N, retries=N, loop_count=N, completed=<bool>`. `/stele:sync` in a future session can then surface "last session in this project ran flow mode, ended at validate, 2 retries." Today that data exists in `steop state` but gets orphaned when the session ID is forgotten.

## Priority for steop (v0.5.0 status)

| Rank | Event                | Status  | Notes                                                                                             |
| :--: | -------------------- | ------- | ------------------------------------------------------------------------------------------------- |
|  1   | `SubagentStop`       | wired   | Log-only in v1. Deliverable verification (per-phase heuristics) is a follow-up.                  |
|  2   | `SessionEnd`         | wired   | Logs + posts session summary to inbox. Ready for `/stele:sync` consumption.                      |
|  3   | `PreCompact`         | wired   | Log-only in v1. `#compact-rescue` memory tagging is a follow-up.                                 |
|  4   | `SessionStart`       | wired   | Log-only in v1. Auto-`/stele:sync` on startup is a follow-up.                                    |
|  5   | `PostToolUseFailure` | wired   | Log-only in v1. Retry-loop integration is a follow-up.                                           |
|  6   | `SubagentStart`      | wired   | Logs `agent_id`, `agent_type`, `model`, truncated prompt.                                        |
|  7   | `PermissionRequest`  | stub    | Returns `Allow()` without injecting a decision envelope — observes but does not auto-approve.    |

All 7 events the doc originally flagged as gaps are now wired in `plugins/steop/hooks/hooks.json`. The v1 scope was "wire the events and persist structured logs/inboxes"; the deeper behavioral follow-ups (deliverable verification, compact-rescue memories, retry integration) are tracked separately.

## Implementation notes (for future work)

- All new hooks dispatch to the existing `steop hook <event>` Go binary (`apps/steop/cmd_hook.go`). Adding an event is one `case` branch in the dispatcher plus a handler in `apps/steop/internal/hooks/`.
- Hook manifest lives at `plugins/steop/hooks/hooks.json`. Events are declared per-matcher with a timeout (5s default).
- Session state is already a merge-mode `PUT` against stele-server — summary writes on `SessionEnd` reuse the existing `state` client with no new REST endpoint.
- `PreCompact` rescue memories should tag `#compact-rescue` + the session ID so they're cheap to find in a follow-up session.
- `SubagentStop` verification heuristics should be per-phase: executor = "at least one file modified", researcher = "non-empty structured report", architect = "plan has at least one numbered step". Hard-coded in the handler to start; promote to config later if it proves fragile.
