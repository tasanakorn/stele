# PRD — Identity Injection & Multi-Session Statusline

**Status:** Implemented (v0.9.0)
**Target version:** v0.9.0
**Scope:** steop PreToolUse hook, steop CLI flag handling, statusline line 2
**Author:** Tasanakorn (design) + Claude Code (PRD authoring)

---

## 1. Goals

1. **Close the Bash tool identity gap.** The PreToolUse hook intercepts Bash commands that invoke `steop` and appends `--x-session-id` and `--x-project-dir` flags sourced from the hook's stdin JSON and env. The steop CLI treats these flags as first-priority overrides, so commands run via the Bash tool carry full identity without relying on env vars that Claude Code does not inject.
2. **Session-accurate statusline.** Line 2 of `steop statusline` shows the state of **its own session**, not the single most-recent global session. Each Claude Code instance sees its own steop state.

## 2. Non-goals

- Injecting identity into non-steop Bash commands.
- Modifying how hooks receive identity (that's Claude Code's responsibility).
- Changing the composite ID format or server-side ID parsing.
- Line 1 of the statusline (session metadata from Claude Code's stdin JSON).

## 3. Background & Motivation

### 3.1 Current state

Per [claude-code-integration.md](../claude-code-integration.md):

| Surface    | `project_dir`            | `session_id`           |
| ---------- | ------------------------ | ---------------------- |
| Hooks      | env `CLAUDE_PROJECT_DIR` | stdin JSON             |
| Bash tool  | **none**                 | **none**               |
| statusLine | stdin JSON               | stdin JSON             |

When the LLM runs `steop state set <session> '{"phase":"execute"}'` via the Bash tool, the steop CLI calls `detectProjectDir()` which reads `CLAUDE_PROJECT_DIR` — but that env var is **not set** in the Bash tool environment (verified on v2.1.104). The composite ID is built with an empty `project_dir` segment, causing mismatches with sessions created by hooks (which do have the project dir).

The statusline has a parallel problem: it ignores `session_id` and `workspace.project_dir` from its stdin JSON and falls back to `SessionList("","","",1)` — returning the globally most-recent session regardless of project.

### 3.2 Why PreToolUse hook injection

The PreToolUse hook has everything needed:

- `session_id` from stdin JSON (always present)
- `CLAUDE_PROJECT_DIR` from env (set for hooks per official docs)
- Ability to **modify tool input** via `updatedInput` in the hook response

By rewriting the Bash command to append identity flags, the steop CLI receives correct identity without any change to Claude Code itself.

## 4. Design

### 4.1 PreToolUse Hook: Identity Injection

**Trigger:** PreToolUse hook detects a Bash command where the first token (after env assignments, `&&`, `;`, `|`) is `steop`.

**Action:** Append two flags to the command:

```
--x-session-id=<session_id>  --x-project-dir=<project_dir>
```

**Response:** Return `updatedInput` with the rewritten command:

```json
{
  "hookSpecificOutput": {
    "permissionDecision": "allow",
    "updatedInput": {
      "command": "steop state set abc123 '{\"phase\":\"execute\"}' --x-session-id=d290f1ee-... --x-project-dir=/path/to/project"
    }
  }
}
```

**Rules:**

- Only inject when the command invokes `steop` (exact token match, not substring).
- Do not inject if `--x-session-id` or `--x-project-dir` is already present (respect explicit overrides).
- If `CLAUDE_PROJECT_DIR` is empty, omit `--x-project-dir` (do not inject empty values).
- If `session_id` is empty in stdin JSON, omit `--x-session-id`.
- For piped/chained commands (`cmd1 && steop ...`), only append to the `steop` segment.
- Preserve existing safety checks (dangerous pattern blocking runs first; injection only applies to allowed commands).

### 4.2 CLI: `--x-session-id` and `--x-project-dir` Flags

Add two global flags to the steop CLI, parsed before subcommand dispatch:

| Flag               | Type   | Purpose                                      |
| ------------------ | ------ | -------------------------------------------- |
| `--x-session-id`   | string | Override session ID for composite ID building |
| `--x-project-dir`  | string | Override project directory                    |

**Priority order** (highest first):

1. `--x-session-id` / `--x-project-dir` (hook-injected flags)
2. CLI positional arguments (e.g. `steop state get <session>`)
3. `CLAUDE_PROJECT_DIR` env var
4. Server-side `ResolveProjectDir` fallback

**Sentinel file deprecation:** The file `~/.config/stele/steop-current-session` is written by `UserPromptSubmit` hook and read only by `set-phase` and `clear-phase` (to resolve session ID without a CLI argument). With `--x-session-id` injection, both commands receive identity via the flag, making the sentinel redundant. This PRD deprecates:

- `hooks.WriteSentinel()` call in `user_prompt_submit.go`
- `hooks.ReadSentinel()` calls in `cmd_state.go` (`set-phase`, `clear-phase`)
- `internal/hooks/session_sentinel.go` (entire file)

The `x-` prefix signals these are injected by the system, not typed by the user.

### 4.3 Statusline: Own-Session View

**Current:** `resolveStatuslineSession` ignores stdin JSON and picks the single most-recent session globally via `SessionList("","","",1)`.

**New:** Always parse stdin JSON (even with `--line2-only`) to extract `session_id` and `workspace.project_dir`. Use these to query the exact session:

1. Extract `session_id` and `workspace.project_dir` from stdin JSON.
2. Build a composite ID from `host`, `project_dir`, and `session_id`.
3. Call `StatusGet(compositeID)` to fetch the state for **this session only**.

This replaces the global guess with an exact lookup. Each Claude Code instance shows its own session's steop state.

**Fallback:** If stdin JSON is empty or unparseable (e.g. manual `steop statusline` from terminal), fall back to current behaviour (global most-recent).

### 4.4 Session Struct Update

Add `session_id` and `project_dir` to the statusline's `Session` struct (currently only parses `model`, `workspace`, `context_window`, `rate_limits`, `cost`):

```go
type Session struct {
    SessionID string              `json:"session_id,omitempty"`
    Model     *SessionModel       `json:"model,omitempty"`
    Workspace *SessionWorkspace   `json:"workspace,omitempty"`
    // ... existing fields
}
```

`SessionWorkspace` already has `ProjectDir string`.

## 5. Changes by Component

| Component                          | Change                                                        |
| ---------------------------------- | ------------------------------------------------------------- |
| `apps/steop/internal/hooks/pre_tool_use.go` | Detect `steop` in Bash commands, return `updatedInput` with injected flags |
| `apps/steop/main.go`              | Parse `--x-session-id` and `--x-project-dir` global flags     |
| `apps/steop/internal/client/client.go` | `WithOverrides(sessionID, projectDir)` method, priority chain |
| `apps/steop/cmd_state.go`         | Use overridden identity when building composite IDs            |
| `apps/steop/cmd_hook.go`          | Pass hook input fields to injection logic                      |
| `apps/steop/internal/hooks/user_prompt_submit.go` | Remove `WriteSentinel()` call                        |
| `apps/steop/internal/hooks/session_sentinel.go` | Delete (deprecated by `--x-session-id` injection)      |
| `apps/steop/cmd_statusline.go`    | Always parse stdin; use `session_id` + `project_dir` for exact lookup |
| `apps/steop/cmd_statusline_line1.go` | Add `SessionID` field to `Session` struct                   |
| `plugins/steop/hooks/hooks.json`  | No change (PreToolUse on Bash already registered)              |

## 6. Hook Response Format

The existing steop PreToolUse hook returns `Allow()` (`{}`) or `DenyPreToolUse(reason)` (`{"decision":"block","reason":"..."}`).

For identity injection, the new response format uses the `hookSpecificOutput` structure:

```json
{
  "hookSpecificOutput": {
    "hookEventName": "PreToolUse",
    "permissionDecision": "allow",
    "updatedInput": {
      "command": "<original command> --x-session-id=<id> --x-project-dir=<dir>"
    }
  }
}
```

When dangerous patterns are detected, the deny response takes precedence (deny > allow). Safety checks run before injection.

## 7. Edge Cases

1. **Multiple `steop` invocations in one command** (`steop state get X && steop state set X '{}'`) — inject into each `steop` segment independently.
2. **`steop` as substring** (`my-steop-wrapper`) — do not inject. Match on word boundary.
3. **Quoted arguments** (`steop state set X '{"key":"val"}'`) — append flags after all existing arguments; do not break quoting.
4. **Subagents** — subagents fire their own hooks with the same `session_id`, so injection works identically.
5. **statusLine with no stdin** (e.g. manual `steop statusline --line2-only` from terminal) — no `session_id` available, fall back to global most-recent.
6. **statusLine with `--session=<id>`** — explicit flag takes precedence over stdin `session_id`.

## 8. Migration

No database changes. No wire format changes. The new CLI flags are additive. Existing commands without `--x-session-id` / `--x-project-dir` work exactly as before (fall through to lower-priority sources).

The statusline change is purely client-side rendering; the `SessionList` RPC already supports filtering by `project_dir`.

## 9. Testing

- **Unit:** `pre_tool_use_test.go` — verify injection for simple commands, piped commands, commands with existing flags, non-steop commands.
- **Unit:** CLI flag parsing — verify priority chain (flag > positional > sentinel > env > server).
- **Integration:** Run `steop state set` via Bash tool, verify composite ID has correct project_dir segment.
- **Statusline:** Verify own-session lookup from stdin JSON; verify fallback when stdin is empty; verify `--session=` override.
