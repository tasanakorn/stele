# Claude Code Integration Guide

How Claude Code delivers identity, context, and session state to external processes. This document covers three integration surfaces: **Hooks**, **Bash Tool**, and **statusLine Commands**.

> Sources: official Claude Code documentation + runtime inspection on **v2.1.104** (2026-04-12). Each field is marked **(doc)** when sourced from official docs or **(inspected)** when verified by runtime observation. Behaviour may change across versions.

---

## Quick Reference: Identity by Surface

How `CLAUDE_PROJECT_DIR` and `session_id` are delivered across each integration surface:

| Surface    | `project_dir`                              | `session_id`                               | Source      |
| ---------- | ------------------------------------------ | ------------------------------------------ | ----------- |
| Hooks      | env `CLAUDE_PROJECT_DIR`                   | stdin JSON `session_id`                    | (doc)       |
| Bash tool  | **none**                                   | **none**                                   | (inspected) |
| statusLine | stdin JSON `workspace.project_dir`         | stdin JSON `session_id`                    | (doc)       |

---

## 1. Hooks

Hook commands are spawned as subprocesses when Claude Code lifecycle events fire. They receive structured JSON on stdin and write a JSON response to stdout.

### Environment Variables

Hook commands run in the current directory with Claude Code's environment. Per official docs, hooks receive these variables:

| Variable               | Value                            | Source      | Notes                                                     |
| ---------------------- | -------------------------------- | ----------- | --------------------------------------------------------- |
| `CLAUDE_PROJECT_DIR`   | `/path/to/project`               | **(doc)**   | Project root directory                                    |
| `CLAUDE_PLUGIN_ROOT`   | `<plugin-install-dir>`           | **(doc)**   | Plugin's installation directory (plugin hooks only)        |
| `CLAUDE_PLUGIN_DATA`   | `<plugin-data-dir>`              | **(doc)**   | Plugin's persistent data directory (plugin hooks only)     |
| `CLAUDE_ENV_FILE`      | `<path>`                         | **(doc)**   | Only in SessionStart, CwdChanged, FileChanged             |
| `CLAUDE_CODE_REMOTE`   | `true`                           | **(doc)**   | Only in remote/web environments; not set in local CLI      |
| `CLAUDECODE`           | `1`                              | **(doc)**   | Canonical "am I inside Claude Code?" flag                  |

**Not set:**

| Variable           | Notes                                             |
| ------------------ | ------------------------------------------------- |
| `CLAUDE_SESSION_ID`| Session ID is delivered via stdin JSON, not env    |

### stdin JSON Payload

Every hook receives a JSON object on stdin with **common fields** (always present) plus **event-specific fields**.

#### Common Fields

```jsonc
{
  "session_id":      "d290f1ee-6c54-4b01-90e6-d701748f0851",  // always present
  "transcript_path": "<home>/.claude/projects/.../transcript.jsonl",
  "cwd":             "/path/to/project",
  "hook_event_name": "PostToolUse",                            // event type
  "permission_mode": "default"                                 // may be absent for some events
}
```

| Field              | Type   | Always present | Notes                                                     |
| ------------------ | ------ | -------------- | --------------------------------------------------------- |
| `session_id`       | string | yes            | UUID for the current Claude Code session                  |
| `transcript_path`  | string | yes            | Path to the session's JSONL transcript file               |
| `cwd`              | string | yes            | Working directory when the event fired                    |
| `hook_event_name`  | string | yes            | Event name (matches the hook key in `hooks.json`)         |
| `permission_mode`  | string | conditional    | Present for tool events; absent for `UserPromptSubmit`, `Stop` |

#### Event: SessionStart

```jsonc
{
  "session_id":      "d290f1ee-6c54-4b01-90e6-d701748f0851",
  "transcript_path": "<home>/.claude/projects/.../transcript.jsonl",
  "cwd":             "/path/to/project",
  "hook_event_name": "SessionStart",
  "source":          "startup",          // "startup" | "resume" | "clear" | "compact"
  "model":           "claude-opus-4-6"
}
```

#### Event: SessionEnd

```jsonc
{
  "session_id":      "d290f1ee-6c54-4b01-90e6-d701748f0851",
  "transcript_path": "<home>/.claude/projects/.../transcript.jsonl",
  "cwd":             "/path/to/project",
  "hook_event_name": "SessionEnd"
}
```

#### Event: UserPromptSubmit

```jsonc
{
  "session_id":      "d290f1ee-6c54-4b01-90e6-d701748f0851",
  "transcript_path": "<home>/.claude/projects/.../transcript.jsonl",
  "cwd":             "/path/to/project",
  "hook_event_name": "UserPromptSubmit",
  "prompt":          "Please implement feature X"
}
```

#### Event: PreToolUse

```jsonc
{
  "session_id":      "d290f1ee-6c54-4b01-90e6-d701748f0851",
  "transcript_path": "<home>/.claude/projects/.../transcript.jsonl",
  "cwd":             "/path/to/project",
  "hook_event_name": "PreToolUse",
  "permission_mode": "default",
  "tool_name":       "Bash",
  "tool_use_id":     "tool_abc123",
  "tool_input": {
    "command":     "npm test",
    "description": "Run test suite",
    "timeout":     120000
  }
}
```

#### Event: PostToolUse

```jsonc
{
  "session_id":      "d290f1ee-6c54-4b01-90e6-d701748f0851",
  "transcript_path": "<home>/.claude/projects/.../transcript.jsonl",
  "cwd":             "/path/to/project",
  "hook_event_name": "PostToolUse",
  "permission_mode": "default",
  "tool_name":       "Bash",
  "tool_use_id":     "tool_abc123",
  "tool_input": {
    "command":     "npm test",
    "description": "Run test suite",
    "timeout":     120000
  },
  "tool_response":   "... tool output ..."
}
```

#### Event: PostToolUseFailure

```jsonc
{
  "session_id":      "d290f1ee-6c54-4b01-90e6-d701748f0851",
  "transcript_path": "<home>/.claude/projects/.../transcript.jsonl",
  "cwd":             "/path/to/project",
  "hook_event_name": "PostToolUseFailure",
  "permission_mode": "default",
  "tool_name":       "Bash",
  "tool_use_id":     "tool_abc123",
  "tool_input": {
    "command":     "npm test",
    "description": "Run test suite",
    "timeout":     120000
  },
  "error":           "command timed out"
}
```

#### Event: Stop

```jsonc
{
  "session_id":             "d290f1ee-6c54-4b01-90e6-d701748f0851",
  "transcript_path":        "<home>/.claude/projects/.../transcript.jsonl",
  "cwd":                    "/path/to/project",
  "hook_event_name":        "Stop",
  "stop_hook_active":       true,
  "last_assistant_message": "Done. The tests pass now.",
  "trigger":                "end_turn",
  "reason":                 "end_turn",
  "is_interrupt":           false
}
```

#### Event: SubagentStart

```jsonc
{
  "session_id":      "d290f1ee-6c54-4b01-90e6-d701748f0851",
  "transcript_path": "<home>/.claude/projects/.../transcript.jsonl",
  "cwd":             "/path/to/project",
  "hook_event_name": "SubagentStart",
  "agent_id":        "a0e3d459a64394831",
  "agent_type":      "researcher"
}
```

#### Event: SubagentStop

```jsonc
{
  "session_id":      "d290f1ee-6c54-4b01-90e6-d701748f0851",
  "transcript_path": "<home>/.claude/projects/.../transcript.jsonl",
  "cwd":             "/path/to/project",
  "hook_event_name": "SubagentStop",
  "agent_id":        "a0e3d459a64394831",
  "agent_type":      "researcher",
  "success":         true,
  "output":          "... agent result ..."
}
```

### All Fields

| Field                    | Type   | Present in                                    |
| ------------------------ | ------ | --------------------------------------------- |
| `session_id`             | string | all events                                    |
| `transcript_path`        | string | all events                                    |
| `cwd`                    | string | all events                                    |
| `hook_event_name`        | string | all events                                    |
| `permission_mode`        | string | tool events, PermissionRequest                |
| `tool_name`              | string | PreToolUse, PostToolUse, PostToolUseFailure   |
| `tool_input`             | object | PreToolUse, PostToolUse, PostToolUseFailure   |
| `tool_response`          | any    | PostToolUse                                   |
| `tool_use_id`            | string | PreToolUse, PostToolUse, PostToolUseFailure   |
| `prompt`                 | string | UserPromptSubmit                              |
| `stop_hook_active`       | bool   | Stop                                          |
| `last_assistant_message` | string | Stop                                          |
| `agent_id`               | string | SubagentStart, SubagentStop                   |
| `agent_type`             | string | SubagentStart, SubagentStop                   |
| `model`                  | string | SessionStart                                  |
| `source`                 | string | SessionStart                                  |
| `output`                 | string | SubagentStop                                  |
| `success`                | bool   | SubagentStop                                  |
| `trigger`                | string | Stop                                          |
| `error`                  | string | PostToolUseFailure                            |
| `is_interrupt`           | bool   | Stop                                          |
| `reason`                 | string | Stop                                          |

### stdout Response

Hooks write JSON to stdout. The response controls whether Claude Code proceeds:

```jsonc
// Allow (default — proceed normally)
{}

// Block with a message shown to the assistant
{ "decision": "block", "reason": "Cannot run rm -rf in production directory" }
```

### Steop Hook Wiring

From `plugins/steop/hooks/hooks.json`:

| Event               | Matcher | Timeout | Purpose                                    |
| -------------------- | ------- | ------- | ------------------------------------------ |
| `SessionStart`       | `*`     | 5s      | Create/reactivate session, write sentinel  |
| `UserPromptSubmit`   | `*`     | 5s      | Skill dispatch (slash commands)            |
| `PreToolUse`         | `Bash`  | 5s      | Safety guards on Bash commands             |
| `PermissionRequest`  | `*`     | 5s      | Permission policy enforcement              |
| `PostToolUse`        | `*`     | 3s      | Counter tracking (tool_calls, loop_count)  |
| `PostToolUseFailure` | `*`     | 5s      | Retry counter tracking                     |
| `SubagentStart`      | `*`     | 5s      | Track subagent lifecycle                   |
| `SubagentStop`       | `*`     | 5s      | Track subagent lifecycle                   |
| `PreCompact`         | `*`     | 5s      | Snapshot state before context compaction   |
| `Stop`               | `*`     | 10s     | End-of-turn housekeeping                   |
| `SessionEnd`         | `*`     | 30s     | Mark session stopped, cleanup              |

---

## 2. Bash Tool

When Claude Code invokes a command via the Bash tool, it spawns a shell subprocess. The LLM composes the command; the user's shell profile is sourced.

### Environment Variables

The spawned shell inherits the user's profile (`~/.zshrc` / `~/.bashrc`) plus Claude-injected variables:

| Variable                               | Value                                          | Source          | Notes                                                    |
| -------------------------------------- | ---------------------------------------------- | --------------- | -------------------------------------------------------- |
| `CLAUDECODE`                           | `1`                                            | **(inspected)** | Canonical "am I inside Claude Code?" flag                |
| `CLAUDE_CODE_ENTRYPOINT`               | `cli`                                          | **(inspected)** | Launch mode (`cli`, `vscode`, `jetbrains`)               |
| `CLAUDE_CODE_EXECPATH`                 | `<home>/.local/share/claude/versions/<ver>`    | **(inspected)** | Installation directory of the running binary              |

**Not available** (verified by runtime inspection on v2.1.104):

| Variable             | Status        | Source          | Notes                                                    |
| -------------------- | ------------- | --------------- | -------------------------------------------------------- |
| `CLAUDE_PROJECT_DIR` | **not set**   | **(inspected)** | Not injected into Bash tool env (unlike hooks)           |
| `CLAUDE_SESSION_ID`  | **not set**   | **(inspected)** | Session ID is never exposed as an env var                |

### Note on Project Directory

`CLAUDE_PROJECT_DIR` is not set in the Bash tool env, but `PWD` is the project root (Claude Code runs Bash commands from the project directory). The LLM also receives the project path in its system instructions (`Primary working directory: ...`) and can pass it as a CLI argument when needed.

`session_id` is not available to the LLM via the Bash tool at all. It is only delivered to hooks (stdin JSON) and statusLine commands (stdin JSON).

### stdin / stdout

- **stdin:** Not connected (no JSON payload).
- **stdout:** Captured and returned to the LLM as the tool result.
- **stderr:** Captured and returned alongside stdout.


---

## 3. statusLine Commands

When `statusLine` is configured as a command in settings, Claude Code pipes session metadata to the command's stdin for rendering a status bar.

### Configuration

In `<home>/.claude/settings.json`:

```json
{ "statusLine": { "type": "command", "command": "steop statusline" } }
```

### Environment Variables

statusLine commands run as external subprocesses spawned by the Claude Code UI renderer. They do **not** receive any Claude-specific env vars:

| Variable               | Status  |
| ---------------------- | ------- |
| `CLAUDECODE`           | not set |
| `CLAUDE_CODE_ENTRYPOINT` | not set |
| `CLAUDE_CODE_EXECPATH` | not set |
| `CLAUDE_PROJECT_DIR`   | not set |
| `CLAUDE_SESSION_ID`    | not set |

### stdin JSON Payload

Claude Code pipes a JSON object to stdin after each assistant message (debounced ~300ms).

```jsonc
{
  // ── Identity ──
  "session_id":   "d290f1ee-6c54-4b01-90e6-d701748f0851",
  "session_name": "my-session",        // only if set via --name or /rename

  // ── Paths ──
  "cwd":             "/path/to/project",
  "transcript_path": "<home>/.claude/projects/.../transcript.jsonl",

  // ── Model ──
  "model": {
    "id":           "claude-opus-4-6",
    "display_name": "Opus"
  },

  // ── Workspace ──
  "workspace": {
    "current_dir":  "/path/to/project",
    "project_dir":  "/path/to/project",
    "added_dirs":   [],
    "git_worktree": "feature-xyz"       // only in linked worktrees
  },

  // ── Context Window ──
  "context_window": {
    "total_input_tokens":  15234,
    "total_output_tokens": 4521,
    "context_window_size": 200000,
    "used_percentage":     8.0,
    "remaining_percentage": 92.0,
    "current_usage": {                  // null before first API call
      "input_tokens":                8500,
      "output_tokens":               1200,
      "cache_creation_input_tokens": 5000,
      "cache_read_input_tokens":     2000
    }
  },

  // ── Cost (API-billed plans) ──
  "cost": {
    "total_cost_usd":        0.01234,
    "total_duration_ms":     45000,
    "total_api_duration_ms": 2300,
    "total_lines_added":     156,
    "total_lines_removed":   23
  },

  // ── Rate Limits (Pro/Max subscriber plans) ──
  "rate_limits": {
    "five_hour": {
      "used_percentage": 23.5,
      "resets_at":       1738425600      // Unix epoch
    },
    "seven_day": {
      "used_percentage": 41.2,
      "resets_at":       1738857600
    }
  },

  // ── Version ──
  "version": "2.1.104",

  // ── Optional sections ──
  "exceeds_200k_tokens": false,
  "output_style": { "name": "default" },
  "vim":   { "mode": "NORMAL" },                   // only if vim mode enabled
  "agent": { "name": "security-reviewer" },         // only if --agent flag
  "worktree": {                                      // only in worktree sessions
    "name":            "my-feature",
    "path":            "<home>/.claude/worktrees/my-feature",
    "branch":          "worktree-my-feature",
    "original_cwd":    "/path/to/project",
    "original_branch": "main"
  }
}
```

### Field Availability

| Field                     | Always present | Condition                               |
| ------------------------- | -------------- | --------------------------------------- |
| `session_id`              | yes            |                                         |
| `cwd`                     | yes            |                                         |
| `transcript_path`         | yes            |                                         |
| `model`                   | yes            |                                         |
| `workspace`               | yes            |                                         |
| `workspace.project_dir`   | yes            |                                         |
| `context_window`          | yes            | `used_percentage` may be null early     |
| `version`                 | yes            |                                         |
| `session_name`            | no             | Only when set via `--name` or `/rename` |
| `cost`                    | conditional    | API-billed plans only                   |
| `rate_limits`             | conditional    | Pro/Max plans, after first API response |
| `vim`                     | no             | Only when vim mode is enabled           |
| `agent`                   | no             | Only when using `--agent` flag          |
| `worktree`                | no             | Only during `--worktree` sessions       |
| `workspace.git_worktree`  | no             | Only in linked worktrees                |

### stdout

The command prints one or two lines of text (optionally ANSI-colored) to stdout. Claude Code renders this in the status bar area.

### Trigger Frequency

The statusline command runs:
- After each assistant message
- When permission mode changes
- When vim mode toggles
- Debounced at ~300ms

---

## 4. Cross-Surface Comparison

### Environment Variables

| Variable                   | Hooks              | Bash tool            | statusLine           |
| -------------------------- | ------------------ | -------------------- | -------------------- |
| `CLAUDECODE`               | set **(doc)**      | `"1"` **(inspected)**| not set              |
| `CLAUDE_CODE_ENTRYPOINT`   | —                  | set **(inspected)**  | not set              |
| `CLAUDE_CODE_EXECPATH`     | —                  | set **(inspected)**  | not set              |
| `CLAUDE_PROJECT_DIR`       | set **(doc)**      | not set **(inspected)** | not set           |
| `CLAUDE_SESSION_ID`        | not set            | not set **(inspected)** | not set           |

### Identity Availability

| Identity piece  | Hooks                              | Bash tool                         | statusLine                                    |
| --------------- | ---------------------------------- | --------------------------------- | --------------------------------------------- |
| `session_id`    | stdin JSON `session_id`            | not available (LLM must pass)     | stdin JSON `session_id`                       |
| `project_dir`   | env `CLAUDE_PROJECT_DIR` **(doc)** | not available **(inspected)**     | stdin JSON `workspace.project_dir`            |
| `host`          | `STELE_HOST` / config / hostname   | `STELE_HOST` / config / hostname  | `STELE_HOST` / config / hostname              |

### Input / Output

| Aspect         | Hooks                          | Bash tool              | statusLine                     |
| -------------- | ------------------------------ | ---------------------- | ------------------------------ |
| **stdin**      | JSON payload                   | not connected          | JSON payload                   |
| **stdout**     | JSON response (`{}` or block)  | captured as tool result| rendered in status bar         |
| **stderr**     | ignored (debug logging)        | captured with stdout   | ignored                        |
| **Exit code**  | non-zero = hook failure        | returned to LLM        | ignored (must not stall)       |

---

## 5. Known Gaps (as of v0.8.3) {#known-gaps}

1. **`CLAUDE_PROJECT_DIR` is not set in Bash tool env** — Official docs confirm it is set for hooks, but runtime inspection shows it is **not set** in the Bash tool environment. Commands like `steop state set <session> <json>` called from the Bash tool build composite IDs with an empty project_dir segment, producing IDs like `hostname::session-uuid` instead of `hostname:/path/to/project:session-uuid`. The LLM caller has no fallback.

2. **statusLine ignores `session_id` and `workspace.project_dir` from stdin** — The `Session` struct in `cmd_statusline_line1.go` does not extract `session_id`. With `--line2-only`, stdin is skipped entirely. The statusline falls back to `SessionList("","","",1)` which returns the globally most-recent session — wrong when multiple projects are active.

3. **Sentinel file is global** — `set-phase` and `clear-phase` use a single file `~/.config/stele/steop-current-session`. Concurrent Claude Code sessions race on this file. Planned for deprecation in [prd-003](prd/prd-003-identity-injection.md) via PreToolUse `--x-session-id` injection.
