# PRD — Storage Session Fallback & st-watch Cleanup

**Status:** Implemented (v0.9.3)
**Target version:** v0.9.3
**Scope:** steop CLI `cmd_storage.go` + steop plugin `st-watch` skill
**Author:** Tasanakorn (design) + Claude Code (PRD authoring)

---

## 1. Goals

1. **Eliminate manual `$SESSION_ID` placeholders.** The `steop storage` command should auto-resolve session scope from the hook-injected `--x-session-id` global flag, just like `steop state` and `steop mailbox` already do.
2. **Simplify st-watch.** Remove all `--session=$SESSION_ID` occurrences from the skill, since identity injection is now handled by the steop PreToolUse hook (PRD-003, v0.9.0). The skill should focus on mailbox polling and task dispatch.
3. **Consistent identity model.** After this change, every `steop` subcommand uses the same global-flag identity path — no subcommand requires the LLM to manually substitute session IDs.

## 2. Non-goals

- Changing the PreToolUse hook or its injection behavior (PRD-003 is stable).
- Removing the explicit `--session=` flag from `steop storage`. It stays available for manual/debugging use but becomes optional when `globalSessionID` is set.
- Modifying other skills (only st-watch uses `--session=$SESSION_ID`).
- Changing storage table schema or RPC surface.

## 3. Background & Motivation

### 3.1 Current state

PRD-003 (v0.9.0) introduced a PreToolUse hook that appends `--x-session-id=<id> --x-project-dir=<dir>` to every `steop` Bash command. The `parseGlobalFlags()` function in `main.go` strips these into `globalSessionID` and `globalProjectDir` globals.

Most subcommands already use `globalSessionID`:
- `steop state` — uses it via `cmd_state.go`
- `steop mailbox` — uses it via `cmd_mailbox.go`

**Exception:** `steop storage` (`cmd_storage.go`) parses its own `--session=` flag and ignores `globalSessionID` entirely. This means st-watch must include `--session=$SESSION_ID` as a placeholder the LLM is expected to substitute — but there is no step in the skill that obtains or defines the session ID. It relies on the LLM "just knowing" the value.

### 3.2 Pain points

| #   | Pain point                                                                                       | Remedy                                                       |
| --- | ------------------------------------------------------------------------------------------------ | ------------------------------------------------------------ |
| 1   | `cmd_storage.go` is the only subcommand that doesn't use `globalSessionID`                       | Add fallback to `globalSessionID` when `--session=` is absent |
| 2   | st-watch uses `$SESSION_ID` placeholder that the LLM must substitute without clear instructions  | Remove placeholder; rely on hook injection                   |
| 3   | Inconsistent identity model across subcommands                                                   | Uniform global-flag path                                     |

## 4. Design

### 4.1 `cmd_storage.go` change

In `runStorage()`, after parsing `--session=`, add a fallback:

```go
if sessionID == "" && globalSessionID != "" {
    sessionID = globalSessionID
}
```

This means:
- **`--session=X` given explicitly** → use `X` (no change)
- **`--session=` absent, `globalSessionID` set by hook** → use `globalSessionID` (new behavior)
- **Both absent** → project-scoped storage (no change)

The change is 3 lines of Go.

### 4.2 st-watch SKILL.md change

Replace all 4 occurrences of `--session=$SESSION_ID` with plain commands:

| Line | Before                                                              | After                                           |
| ---- | ------------------------------------------------------------------- | ----------------------------------------------- |
| 15   | `steop storage --session=$SESSION_ID get watcher:last_message_id`   | `steop storage get watcher:last_message_id`     |
| 55   | `steop storage --session=$SESSION_ID get watcher:active_tasks`      | `steop storage get watcher:active_tasks`         |
| 61   | `steop storage --session=$SESSION_ID put watcher:active_tasks [..]` | `steop storage put watcher:active_tasks [..]`   |
| 112  | `steop storage --session=$SESSION_ID put watcher:last_message_id ..`| `steop storage put watcher:last_message_id ..`  |

No other changes to the skill's logic or structure.

## 5. Changes by Component

| Component          | File                                      | Change                                                   |
| ------------------ | ----------------------------------------- | -------------------------------------------------------- |
| steop CLI          | `apps/steop/cmd_storage.go`               | Add `globalSessionID` fallback (3 lines)                 |
| steop plugin       | `plugins/steop/skills/st-watch/SKILL.md`  | Remove `--session=$SESSION_ID` from 4 storage commands   |

## 6. Edge Cases

1. **Direct CLI usage without hooks.** A human running `steop storage get <key>` from a terminal (no hook injection) gets project-scoped storage — same as today. No breaking change.
2. **Explicit `--session=` still wins.** If both `--session=X` and `globalSessionID=Y` are present, `X` takes precedence because the explicit flag is parsed first and the fallback only fires when `sessionID == ""`.
3. **Multiple watchers on different sessions.** Each session's hook injects its own `--x-session-id`, so watcher state remains isolated per session. No collision.

## 7. Migration

None. This is a backward-compatible patch — existing commands continue to work identically.

## 8. Testing

1. **Manual:** Run `steop storage get <key>` with and without `--x-session-id=<id>` to verify fallback behavior.
2. **Smoke test:** Start st-watch, send a `TASK:REQUEST`, verify watcher state is stored under session scope (check via `steop storage --session=<id> list`).
