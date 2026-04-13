# PRD-016: Hook injection scope + public identity flags

- **Status:** Implemented (v0.13.1)
- **Target version:** v0.13.1
- **Author:** Tasanakorn (design) + Claude Code (PRD authoring)

## Goals

- Stop rewriting `Monitor` tool invocations in the PreToolUse hook so per-line stdout notifications reach Claude again.
- Expose public `--session-id` / `--project-dir` flags on every `steop` subcommand as equivalent aliases of the internal `--x-*` forms.
- Add a new `steop identity` subcommand that prints resolved identity as JSON, giving skills a read-back mechanism for what the hook injected.
- Extend `steop mailbox watch`'s `WATCHER:READY` line with `session_id` and `project_dir` for debug visibility.
- Update `/steop:st-flow` docs to pass identity explicitly via public flags when subprocesses run outside the Bash injection path.

## Non-goals

- No change to Bash-hook injection behavior beyond extending the skip guard to also recognize the new public flag names.
- No rename or removal of `--x-session-id` / `--x-project-dir`; internal callers and existing scripts keep working unchanged.
- No change to session resolution, storage, mailbox, or `steop send` smart-addressing logic.
- No schema, config, or migration work.
- No retroactive edits to PRD-003; PRD-016 amends its scope additively.

## Background & Motivation

PRD-003 (v0.9.0) introduced the PreToolUse hook that injects `--x-session-id` / `--x-project-dir` into every `steop ...` command. PRD-015 (v0.12.6) fixed a Monitor regression by moving the injected flags to sit directly after the `steop` token instead of being appended past redirections. Both PRDs assumed the hook must cover both `Bash` and `Monitor`.

Empirical investigation on 2026-04-13 (during PRD-014 follow-up) uncovered a deeper constraint: **any `AllowWithUpdatedInput` rewrite on a `Monitor` tool call suppresses per-line stdout notifications to the model, regardless of the rewritten command's correctness**. Bytes still reach Monitor's output pane, but the per-line notification channel — the mechanism `st-watch` relies on for WATCHER:READY and for each streaming mailbox event — does not fire. A Python probe with identical output behavior emits notifications when the invocation has no `steop` token (hook returns plain `Allow()`) and stops emitting them when `steop` is the leading token (hook returns `AllowWithUpdatedInput`).

That leaves only one safe option for Monitor: the hook must not rewrite Monitor commands at all. Since injection is the only way a Monitor-launched `steop` subprocess currently learns its identity, the hook's disappearance on that path must be compensated by public flags that skills can pass explicitly.

### Current state

**Flag parsing** lives in `apps/steop/main.go:56-69` (`parseGlobalFlags`). It scans `os.Args` for `--x-session-id=<val>` / `--x-project-dir=<val>` in equals-form only, strips them, and stores into package globals `globalSessionID` / `globalProjectDir` (main.go:11-14). Position-independent, confirmed by PRD-015. The subcommands `cmd_state.go`, `cmd_storage.go`, `cmd_mailbox.go`, `cmd_mailbox_watch.go`, and `cmd_send.go` consult the globals; `cmd_statusline.go`, `cmd_monitor.go`, `cmd_hook.go`, and `cmd_version.go` do not.

**Hook rewrite** lives in `apps/steop/internal/hooks/pre_tool_use.go`:

- `HandlePreToolUse` (`:38-65`) guards on `ToolName != "Bash" && ToolName != "Monitor"` (`:42`), runs the dangerous-pattern deny list, and then calls `injectIdentity` followed by `AllowWithUpdatedInput`.
- `injectIdentity` (`:75-121`) bails when both identity values are empty, and at `:80` bails when the command already contains `--x-session-id` or `--x-project-dir`.
- `buildIdentitySuffix` (`:128-139`) emits `" --x-session-id=<id> --x-project-dir='<quoted>'"`.

**Matcher** in `plugins/steop/hooks/hooks.json` is `"Bash|Monitor"` on PreToolUse.

**WATCHER:READY emission** sits in `apps/steop/cmd_mailbox_watch.go:192-201` as an anonymous struct with fields `MessageType` and `Interval` only.

## Design

### 1. Hook matcher restricted to Bash

`plugins/steop/hooks/hooks.json` changes its PreToolUse matcher from `"Bash|Monitor"` to `"Bash"`. Monitor invocations no longer reach the hook, so they always return a plain `Allow()` from the Claude Code runtime and notifications flow.

`apps/steop/internal/hooks/pre_tool_use.go:42` drops `Monitor` from the toolname guard so that even if a future matcher change re-routes Monitor here, the handler explicitly refuses to rewrite it. A Monitor invocation that somehow arrives returns a plain `Allow()` with no `updatedInput`.

### 2. Public flag aliases

`parseGlobalFlags` in `apps/steop/main.go` grows recognition for `--session-id=<val>` and `--project-dir=<val>` as equivalent aliases of `--x-session-id` / `--x-project-dir`. Both forms write to the same `globalSessionID` / `globalProjectDir` package globals; last-one-wins if both are present in a single invocation. All subcommands that already consult the globals (`cmd_state.go`, `cmd_storage.go`, `cmd_mailbox.go`, `cmd_mailbox_watch.go`, `cmd_send.go`) inherit the new flags for free.

Precedence below the flags is unchanged: public/`--x-*` flags first, then positional args, then `CLAUDE_PROJECT_DIR` env, then `STELE_HOST`/`os.Hostname()` for host, then the server-side `ResolveProjectDir` fallback.

### 3. Skip-guard extension

`pre_tool_use.go:80` currently does `strings.Contains(cmd, "--x-session-id") || strings.Contains(cmd, "--x-project-dir")`. It gains two additional substrings, `--session-id` and `--project-dir`, so a user-typed (or skill-injected) public flag also bypasses injection. Note that a command containing `--x-session-id` naturally also matches `--session-id` as a substring — the additional checks are strictly additive and do not change existing Bash-path behavior.

### 4. `steop identity` subcommand

New file `apps/steop/cmd_identity.go` implements `runIdentity`. Dispatch is added to the switch in `apps/steop/main.go`. The command takes no positional args, honors the global identity flags plus the existing env fallbacks, and prints a single JSON object to stdout:

```
{
  "session_id":            "<uuid or empty>",
  "project_dir":           "<absolute path or empty>",
  "host":                  "<resolved host>",
  "session_composite_id":  "host:project_dir:UUID or empty",
  "project_composite_id":  "host:project_dir"
}
```

Empty string fields signal unresolved identity rather than erroring, so skills can diagnose injection failures without a non-zero exit. The output shape mirrors how `cmd_statusline.go` and `internal/client/client.go` already compose composite ids.

### 5. WATCHER:READY additive fields

`apps/steop/cmd_mailbox_watch.go:192-201` extends the anonymous payload struct with two new `omitempty`-tagged fields:

```
{"message_type":"WATCHER:READY","interval":10,"session_id":"<id>","project_dir":"<path>"}
```

Values come from the resolved client (`c.ProjectDir()` or `globalProjectDir`) and the selected session / `globalSessionID`. Older consumers that key only on `message_type` / `interval` continue to parse the line unchanged.

### 6. `/steop:st-flow` docs

`plugins/steop/skills/st-flow/SKILL.md` grows a short note explaining that nested subprocesses launched outside the Bash injection path (for example, Monitor or any tool that forks a `steop` invocation without going through a Bash command) must receive `--session-id` / `--project-dir` explicitly, and that `steop identity` is the read-back mechanism to confirm what the hook injected.

## Changes by Component

| Component                                                                                                                         | Change                                                                                                        |
| --------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------- |
| `plugins/steop/hooks/hooks.json`                                                                                                  | PreToolUse matcher `"Bash\|Monitor"` -> `"Bash"`.                                                             |
| `apps/steop/internal/hooks/pre_tool_use.go`                                                                                       | Drop `Monitor` from the toolname guard at `:42`; extend skip guard at `:80` with `--session-id`/`--project-dir`. |
| `apps/steop/internal/hooks/pre_tool_use_test.go`                                                                                  | Drop/repurpose Monitor injection tests into a Monitor-skip test; add public-flag skip-guard + parsing cases.  |
| `apps/steop/main.go` (`parseGlobalFlags`, dispatch switch)                                                                        | Accept `--session-id=` / `--project-dir=` aliases; add `identity` dispatch case.                              |
| `apps/steop/cmd_identity.go` (new)                                                                                                | Implement `runIdentity`; emit canonical JSON identity object.                                                 |
| `apps/steop/cmd_mailbox_watch.go`                                                                                                 | Extend WATCHER:READY payload with `session_id` + `project_dir` (`omitempty`).                                 |
| `plugins/steop/skills/st-flow/SKILL.md`                                                                                           | Note public-flag requirement for non-Bash subprocesses; reference `steop identity`.                           |
| `apps/stele/Cargo.toml`, `apps/steop/version.go`, `plugins/steop/.claude-plugin/plugin.json`, `plugins/stele/.claude-plugin/plugin.json` | Lock-step bump to `v0.13.1` via `python scripts/bump-version.py patch`.                                 |
| `docs/README.md`                                                                                                                  | Add prd-016 row to the PRD table.                                                                             |

No changes needed in `cmd_state.go`, `cmd_storage.go`, `cmd_send.go`, or `cmd_mailbox.go` — they inherit public-flag support via the aliased globals.

## Edge Cases

1. **Existing `--x-*` callers.** Untouched. The hook still injects them on Bash, storage still reads the same globals, and the skip guard still recognizes them.
2. **Mixed flag forms.** `steop ... --session-id=A --x-session-id=B` is last-one-wins (`B`). Skills should not mix forms; documenting one public form is enough.
3. **Nested shell chains via Monitor.** After the matcher change, commands like `steop mailbox watch ... | tee log` launched under Monitor receive no injection. Skills must pass identity explicitly via public flags. This is the motivation for Goals 2 and 3.
4. **`steop state set-phase` / `clear-phase` under Monitor.** These silently no-op when `globalSessionID == ""`. Any Monitor-only caller that does not pass `--session-id` falls back to this existing best-effort, non-blocking contract. Acceptable.
5. **`steop send` smart addressing.** Independent of flag form; continues to work through the aliased globals.
6. **Env-var parsing in `parseGlobalFlags`.** Aliasing is purely additive within the same scan loop; no new parsing risk.
7. **Docs drift with PRD-003.** PRD-003 §4.2 hardcodes the `--x-*` names. PRD-016 does not retroactively edit it; this PRD is the canonical record that public aliases exist from v0.13.1 onward.
8. **Smoke script.** `apps/steop/scripts/smoke-mailbox.py:19` uses `--x-*` explicitly; still valid post-change. An optional sibling check for `steop identity` output shape can be added opportunistically.

## Migration

None required.

- Version bump: `python scripts/bump-version.py patch` at repo root moves `apps/stele/Cargo.toml`, both plugin `plugin.json` files, and `apps/steop/version.go` to `v0.13.1` in lock-step.
- Users rebuild/reinstall the `steop` binary (`/steop:install` or `cd apps/steop && go build -o target/steop . && cp target/steop ~/.local/bin/steop`, with `rm` before `cp` on macOS Tahoe per the repo wisdom note).
- Reinstall the Claude Code plugin or otherwise refresh `hooks.json` so the new `"Bash"`-only matcher takes effect.
- No config, DB, or MCP contract changes. Internal `--x-*` callers are untouched.

## Testing

Unit tests live in `apps/steop/internal/hooks/pre_tool_use_test.go` and `apps/steop/main_test.go` (or a sibling file for identity).

**Existing tests to update:**

- `TestPreToolUseFlagsBeforeRedirection` (`pre_tool_use_test.go:228-245`) — rename to `TestPreToolUseMonitorNotRewritten`; input is a Monitor tool invocation with `steop mailbox watch ... 2>&1`; assert plain `Allow()` with no `updatedInput`.
- Add a Bash mirror of the original assertion under a new name so PRD-015's redirection regression coverage remains intact for the Bash path.
- `TestPreToolUseRespectsExistingFlags` (`:157-162`) — grow cases for `--session-id=existing` and `--project-dir=/foo`; both must skip injection.

**New tests:**

- Monitor + any `steop ...` command returns plain `Allow()` regardless of `--x-*`/public-flag presence.
- `parseGlobalFlags` accepts `--session-id=` / `--project-dir=` and populates the same globals as `--x-*`.
- Mixed-form precedence is last-one-wins within a single `os.Args` scan.
- `steop identity` emits the canonical JSON shape from flag sources, from env fallback, and with empty strings when nothing is resolved.
- `WATCHER:READY` JSON includes the two new fields when identity is present and omits them (via `omitempty`) when both are empty.

**Manual verification:**

1. Build and install `steop` v0.13.1.
2. Reinstall the plugin (or manually edit `hooks.json`) so Monitor no longer triggers PreToolUse.
3. In a Claude Code session with a valid `session_id` and `CLAUDE_PROJECT_DIR`, run `Monitor` with `steop mailbox watch --type=task_request 2>&1`. Confirm per-line stdout notifications reach the model (WATCHER:READY surfaces to the model, not only to the Monitor pane).
4. Run `Bash` with `steop identity` and verify the JSON shows the injected `session_id` / `project_dir`.
5. Run `Bash` with `steop storage get foo --session-id=<id>` and confirm the public flag is honored identically to `--x-session-id`.
6. Run `Bash` with `steop storage get foo` (no flags) and confirm the hook injects `--x-*` as before and the skip guard does not fire.
