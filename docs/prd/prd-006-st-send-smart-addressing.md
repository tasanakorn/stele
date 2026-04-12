# PRD — `/steop:st-send` Skill (Smart Addressing)

**Status:** Implemented (v0.10.0)
**Target version:** v0.10.0
**Scope:** steop plugin — new `st-send` skill + `steop send` CLI subcommand + st-watch mode routing
**Author:** Tasanakorn (design) + Claude Code (PRD authoring)

---

## 1. Goals

1. **Send tasks with short project names.** A new `/steop:st-send` skill and `steop send` CLI subcommand let users address recipients by a short suffix (e.g. `stele`) instead of the full composite ID (`macbook:/Users/tas/Documents/Projects/workspace-coder/stele:USER`). Resolution happens client-side by suffix-matching `project_dir` in `steop_sessions`.
2. **Mode-aware task routing.** Tasks carry a `mode` field in their `meta` JSON (`flow` or `normal`). `st-watch` inspects this field and routes accordingly: `flow` tasks invoke `/steop:st-flow`, `normal` tasks (the default) are handled as plain conversation without the full pipeline.
3. **Zero schema changes.** The mailbox protocol, `steop_mailbox` table, and `steop_sessions` table remain unchanged. Mode is carried in the existing free-form `meta` JSON field.

## 2. Non-goals

- Changing the mailbox protocol or `steop_mailbox` schema.
- Multi-recipient fan-out (sending to multiple projects in one command).
- Remote host discovery or cross-host routing.
- User-defined aliases or a persistent name registry. Short names are derived from session data at resolution time.
- Modifying the existing `steop mailbox send` subcommand. The new `steop send` is a separate top-level command.

## 3. Background & Motivation

### 3.1 Current state

Sending a task to another Claude Code session requires the caller to know the recipient's full composite ID — a string in the form `host:project_dir` or `host:project_dir:segment`. For example:

```
steop mailbox send --to="macbook:/Users/tas/Documents/Projects/workspace-coder/stele:USER" \
  --type=TASK:REQUEST --subject="Fix the bug" --meta='{"description":"..."}'
```

This is verbose, error-prone, and hostile to interactive use. Users already know their project by a short name (`stele`, `frontend`, `api`) but must manually look up the full path and compose the composite ID.

Meanwhile, the codebase already contains a precedent for suffix-based resolution: `ResolveProjectDir()` in `client.go` fetches all sessions and matches the `project_dir` suffix client-side. The same pattern can power a `steop send` command.

Additionally, `st-watch` currently routes all incoming `TASK:REQUEST` messages through `/steop:st-flow`. This is heavyweight for simple questions or quick tasks that do not need the full clarify-research-plan-execute-validate pipeline. A `mode` field in `meta` allows the sender to signal intent, and `st-watch` can route accordingly.

## 4. Design

### 4.1 Session resolution algorithm

The `steop send` command resolves a short project name to a full composite ID:

1. Call `steop.session.list` with `state=active` to fetch all active sessions.
2. Filter sessions where `project_dir` ends with `/<suffix>` or equals `<suffix>` exactly.
3. If zero matches: error with "no active session found for '<suffix>'".
4. If multiple matches across different `project_dir` values: error listing the ambiguous matches so the user can provide a more specific suffix.
5. If multiple matches for the same `project_dir` (different sessions): pick the one with the most recent `last_active_at`.
6. Compose the target ID as `host:project_dir:USER` (user-level addressing, since tasks are project-wide, not session-specific).

Host filtering: by default, resolution only considers sessions on the local host (matching the caller's hostname). This keeps behavior predictable and avoids cross-host ambiguity. Remote host support is a non-goal for this PRD.

### 4.2 `steop send` CLI subcommand

New top-level subcommand (not under `mailbox`):

```
steop send <target> <description> [--mode=normal|flow] [--subject=SUBJECT] [--meta=JSON]
```

| Argument      | Required | Default    | Description                                              |
| ------------- | -------- | ---------- | -------------------------------------------------------- |
| `<target>`    | yes      | —          | Short project name (suffix) or full composite ID         |
| `<description>` | yes    | —          | Task description (becomes `meta.description`)            |
| `--mode`      | no       | `normal`   | Routing mode: `normal` (plain conversation) or `flow` (st-flow pipeline) |
| `--subject`   | no       | first 80 chars of description | Message subject |
| `--meta`      | no       | `{}`       | Additional JSON merged into the `meta` field             |

Behavior:

1. If `<target>` contains a `:`, treat it as a full composite ID (no resolution needed).
2. Otherwise, run the resolution algorithm from 4.1.
3. Construct the mailbox message:
   - `type`: `TASK:REQUEST`
   - `to`: resolved composite ID
   - `from`: caller's identity (same derivation as `steop mailbox send`)
   - `subject`: from `--subject` or truncated description
   - `meta`: `{"description": "<description>", "mode": "<mode>"}` merged with any `--meta` JSON
   - `payload`: `null`
4. Call `steop.mailbox.send` RPC.
5. Print confirmation: `Sent TASK:REQUEST to <resolved-id> (mode: <mode>)`.

### 4.3 `/steop:st-send` skill

The skill provides a conversational interface for sending tasks. The SKILL.md instructs Claude Code to:

1. Ask the user for the target project and task description (if not provided in the initial prompt).
2. Ask for mode preference if the task sounds complex (suggest `flow`) or simple (suggest `normal`).
3. Run `steop send <target> "<description>" --mode=<mode>`.
4. Report the result.

The skill is lightweight — the heavy lifting is in the CLI binary.

### 4.4 `st-watch` mode routing

Currently, step 4e of `st-watch/SKILL.md` unconditionally invokes `/steop:st-flow` for all `TASK:REQUEST` messages. This changes to:

1. Parse `meta.mode` from the incoming message (default: `normal` if absent).
2. If `mode` is `flow`: invoke `/steop:st-flow` with the task description (existing behavior).
3. If `mode` is `normal`: process the task as a plain conversation — read the description, perform the requested work directly, and send a `TASK:RESPONSE` back to the sender. No clarify/research/plan/execute/validate pipeline.

This is a SKILL.md-only change. No Go code modification is needed for mode routing.

### 4.5 Full composite ID passthrough

When the user provides a full composite ID (contains `:`), `steop send` skips resolution entirely and sends directly. This preserves the ability to address any recipient precisely, and ensures backward compatibility with scripts or workflows that already compose full IDs.

## 5. Changes by Component

| Component                              | Change                                                                 |
| -------------------------------------- | ---------------------------------------------------------------------- |
| `apps/steop/main.go`                   | Register `send` subcommand dispatch                                    |
| `apps/steop/cmd_send.go` (new)         | `runSend()` — parse args, resolve target, compose message, send        |
| `apps/steop/internal/client/client.go` | Optional: extract resolution into a reusable `ResolveProject()` method |
| `plugins/steop/skills/st-send/SKILL.md` (new) | Skill instructions for `/steop:st-send`                         |
| `plugins/steop/skills/st-watch/SKILL.md`      | Update step 4e with mode-conditional routing                    |
| `docs/prd/prd-006-st-send-smart-addressing.md` (new) | This PRD                                                |
| `docs/README.md`                       | Add PRD-006 row to the PRD table                                       |

## 6. Edge Cases

| Scenario                                  | Behavior                                                                |
| ----------------------------------------- | ----------------------------------------------------------------------- |
| No active sessions match the suffix       | Error: "no active session found for '<suffix>'"                         |
| Multiple `project_dir` values match       | Error: list all matching project dirs, ask user to be more specific     |
| Multiple sessions for same project_dir    | Pick most recent by `last_active_at`                                    |
| Target already contains `:`               | Skip resolution, use as-is                                              |
| `--mode` value is neither `normal`/`flow` | CLI error: "invalid mode '<value>', must be 'normal' or 'flow'"         |
| `meta.mode` absent in incoming message    | `st-watch` defaults to `normal`                                         |
| Suffix matches a stopped session only     | No match (resolution only considers active sessions); error as "no active session" |
| Description is empty                      | CLI error: description is required                                      |

## 7. Migration

No migration is required. The `steop_mailbox` and `steop_sessions` tables are unchanged. The `mode` field is carried in the existing free-form `meta` JSON column.

Existing `steop mailbox send` commands continue to work unchanged. The new `steop send` is additive.

Existing `st-watch` instances will treat incoming messages without a `mode` field as `normal`, which matches the current behavior for simple tasks. Messages explicitly sent with `mode: flow` will route through `st-flow` as before.

## 8. Testing

### 8.1 CLI unit tests

- **Resolution with single match:** mock `session.list` returning one active session, verify correct composite ID output.
- **Resolution with ambiguous match:** mock multiple sessions with different `project_dir` values sharing the same suffix, verify error message lists all matches.
- **Resolution with multiple sessions same project:** mock multiple sessions for the same `project_dir`, verify the most recent `last_active_at` is selected.
- **No match:** mock empty session list, verify error.
- **Full ID passthrough:** provide a target containing `:`, verify no resolution call is made.
- **Mode validation:** verify `--mode=invalid` produces an error.

### 8.2 Manual smoke tests

```bash
# Resolve and send (normal mode, default)
steop send stele "Update the README with new API endpoints"

# Resolve and send (flow mode)
steop send stele "Refactor the auth middleware" --mode=flow

# Full ID passthrough
steop send "macbook:/Users/tas/Projects/stele:USER" "Quick fix" --mode=normal

# Ambiguous suffix (should error)
steop send app "Do something"
# Expected: error listing multiple matching project_dirs

# No match (should error)
steop send nonexistent "Do something"
# Expected: error "no active session found for 'nonexistent'"
```

### 8.3 st-watch mode routing

- Send a message with `mode: normal` and verify st-watch processes it as plain conversation (no st-flow invocation).
- Send a message with `mode: flow` and verify st-watch invokes `/steop:st-flow`.
- Send a message with no `mode` field and verify st-watch defaults to `normal`.
