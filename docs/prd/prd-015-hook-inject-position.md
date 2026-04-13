# PRD-015: PreToolUse hook — inject identity flags right after the `steop` token

- **Status:** Implemented (v0.12.6)
- **Target version:** v0.12.6
- **Author:** Tasanakorn (design) + Claude Code (PRD authoring)

## Goals

- Inject `--x-session-id` and `--x-project-dir` immediately after the `steop` token in every rewritten shell segment, so the rewritten command reads `steop --x-session-id=<id> --x-project-dir=<quoted-dir> <rest-of-segment>` rather than appending the flags to the end of the segment.
- Fix the Monitor-launched `steop mailbox watch` failure mode where stdout notifications are not delivered to the model because identity flags land after `2>&1` (and/or after subcommand arguments) in the rewritten command.
- Preserve the existing Bash/Monitor matcher, the env-var-prefix tolerance of `steopLeadRe`, and the "skip injection when x-flags are already present" guard.

## Non-goals

- No change to CLI flag parsing. `parseGlobalFlags` in `apps/steop/cmd/steop/main.go` already accepts `--x-session-id` / `--x-project-dir` at any position in `os.Args` and is position-independent; this PRD does not touch it.
- No change to the hook matcher. `plugins/steop/hooks/hooks.json` continues to match `Bash|Monitor`.
- No change to the overall identity-injection design established by PRD-003 (source of `sessionID`/`projectDir`, suffix shape, per-segment rewriting, dangerous-pattern deny-first order).
- No schema, config, or migration work.

## Background & Motivation

PRD-003 (Implemented v0.9.0) introduced the PreToolUse hook at `apps/steop/internal/hooks/pre_tool_use.go`. For every `Bash` (and later `Monitor`) tool invocation whose first command token is `steop`, the hook rewrites the command to carry identity flags sourced from hook-stdin `session_id` and env `CLAUDE_PROJECT_DIR`. The rewrite rules (PRD-003 §4.1) are:

1. Match `steop` as the leading token (with optional env-var prefix) via a word-boundary-like regex.
2. Skip rewrite if either `--x-session-id` or `--x-project-dir` is already present.
3. Omit flags whose value is empty.
4. Apply per segment across `&& || ; |` chains.

PRD-015 is a **placement correction**, not a replacement of PRD-003. The injection still happens, still uses the same sources, still respects the same guard; only the position within each matched segment changes.

### Current state

In `injectIdentity` at `apps/steop/internal/hooks/pre_tool_use.go:94`, the suffix is appended to the **end** of the matched segment:

```go
if isSteopSegment(part) {
    part = strings.TrimRight(part, " \t") + suffix
    injected = true
}
```

With `suffix` produced by `buildIdentitySuffix` (lines 115–126) as `" --x-session-id=<id> --x-project-dir='<dir>'"`, an input command like:

```
steop mailbox watch --type=task_request 2>&1
```

is rewritten to:

```
steop mailbox watch --type=task_request 2>&1 --x-session-id=<id> --x-project-dir='<dir>'
```

Empirically (validated 2026-04-13 via an end-to-end Monitor run), this rewritten form delivers stdout lines to Monitor's pane but does **not** surface them to the model as notifications. Relocating the flags to directly after the `steop` token restores notification delivery. The exact mechanism is not fully understood, but the workaround is deterministic and load-bearing for PRD-013's WATCHER:READY handshake and for any future Monitor-driven st-watch flow.

## Design

**Insertion point.** The existing regex `steopLeadRe = ^(?:\w+=\S*\s+)*steop(?:\s|$)` (pre_tool_use.go:25) already recognizes the location just past the `steop` token (with any leading env-var assignments consumed). Use `steopLeadRe.FindStringIndex` on the **trimmed** segment to compute a byte offset for insertion. The insertion point is the end of the match, minus one byte if that last byte is a trailing space or tab (so the flags sit between `steop` and the existing whitespace), or unchanged if the match ended at end-of-string.

Concretely, the rewritten shell shape is:

```
[env-prefix ]steop --x-session-id=<id> --x-project-dir=<quoted-dir> <rest-of-segment>
```

Any env-var assignments that prefix the `steop` token stay in place; the flags land *between* the `steop` token and whatever follows (subcommand, args, redirections, or nothing).

**Suffix shape.** `buildIdentitySuffix` continues to emit each flag with a single leading space (`" --x-session-id=<id> --x-project-dir='<dir>'"`). This is already the correct shape for mid-segment insertion: the leading space separates the flags from the `steop` token, and a single space already exists between the match end and the rest of the segment. No trailing space is added; we rely on the existing whitespace in the original segment. The function is not renamed — the name continues to describe "a string of identity flags" regardless of insertion position.

**Rewrite loop.** The per-segment body in `injectIdentity` changes from:

```go
part = strings.TrimRight(part, " \t") + suffix
```

to something equivalent to:

```go
trimmed := strings.TrimLeft(part, " \t")
leadingWS := part[:len(part)-len(trimmed)]
loc := steopLeadRe.FindStringIndex(trimmed)
// loc is guaranteed non-nil because isSteopSegment returned true.
insertAt := loc[1]
// If the match consumed a trailing space (steop + whitespace), step back so
// the inserted suffix falls between `steop` and that whitespace.
if insertAt > 0 && (trimmed[insertAt-1] == ' ' || trimmed[insertAt-1] == '\t') {
    insertAt--
}
part = leadingWS + trimmed[:insertAt] + suffix + trimmed[insertAt:]
```

The surrounding split-on-shell-operators logic (`shellSplitRe`), the per-segment reassembly with `" && "` / `" || "` spacing, and the "was any segment injected" short-circuit all remain exactly as they are today.

**Skip guard.** The pre-injection check `strings.Contains(cmd, "--x-session-id") || strings.Contains(cmd, "--x-project-dir")` at pre_tool_use.go:80 is untouched. Users who pre-specify either flag anywhere in the command continue to bypass injection entirely.

**Dangerous-pattern deny.** The `dangerousPatterns` deny-list runs before injection and is untouched.

## Changes by Component

| Component                                                         | Change                                                                                                                                         |
| ----------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------- |
| `apps/steop/internal/hooks/pre_tool_use.go`                       | Rewrite the per-segment body of `injectIdentity` to insert `suffix` at the end of the `steopLeadRe` match (adjusted for trailing whitespace). |
| `apps/steop/internal/hooks/pre_tool_use.go` (`buildIdentitySuffix`) | No behavioral change; leading-space-per-flag shape is retained and now serves as the mid-segment separator.                                  |
| `apps/steop/internal/hooks/pre_tool_use_test.go`                  | Update four existing tests to assert the new position; add two new tests (Monitor redirection case, env-var-prefix case).                      |
| `apps/steop/cmd/steop/main.go` (`parseGlobalFlags`)                | No change. Already position-independent.                                                                                                       |
| `plugins/steop/hooks/hooks.json`                                   | No change. Matcher remains `Bash|Monitor`.                                                                                                     |
| `apps/stele/Cargo.toml`, `plugins/steop/.claude-plugin/plugin.json`, `apps/steop/...` | Version bump to `v0.12.6` via `scripts/bump-version.py`.                                                                    |
| `docs/README.md`                                                   | Add prd-015 row to the PRD table.                                                                                                             |

## Edge Cases

1. **Env-var prefix.** Input `FOO=bar steop mailbox watch` — `steopLeadRe` consumes `FOO=bar steop `; the insertion point falls between `steop` and the following space. Output: `FOO=bar steop --x-session-id=<id> --x-project-dir=<quoted-dir> mailbox watch`. Env-var prefix is preserved.
2. **Chained commands.** Input `steop a && steop b`. After `shellSplitRe.Split`, each segment is independently matched and rewritten. Output: `steop --x-session-id=<id> --x-project-dir=<quoted-dir> a && steop --x-session-id=<id> --x-project-dir=<quoted-dir> b`. Both segments carry the flags immediately after their own `steop` token.
3. **`steop` with no subcommand.** Input `steop` (regex tail matches `$`). `loc[1]` equals `len(trimmed)`; no trailing-whitespace byte to step back past. The suffix is appended (with its leading space) to the end of the segment, producing `steop --x-session-id=<id> --x-project-dir=<quoted-dir>`. Behaviorally identical to the old end-append for this degenerate case.
4. **Pre-existing flags.** Input already contains `--x-session-id=<X>` or `--x-project-dir=<Y>`. The existing `strings.Contains` guard at pre_tool_use.go:80 returns `cmd` unchanged. No position change to reason about.
5. **Redirections anywhere.** Input `steop mailbox watch --type=task_request 2>&1` (or `> file`, `2> err.log`, etc.). Redirections now sit strictly after the identity flags and the subcommand arguments; the hook never emits flags after a redirection operator. This is the primary bug fix motivating the PRD.
6. **`echo steop ...`** (non-goal reaffirmation). The `^`-anchored `steopLeadRe` never matches when `steop` is an argument to another command; `isSteopSegment` returns false; segment is not touched. Unchanged from today.

## Migration

None. This is a patch-level behavior fix to an internal hook binary.

- Version bump: run `scripts/bump-version.py 0.12.6` from the repo root to move `apps/stele/Cargo.toml`, `plugins/stele/.claude-plugin/plugin.json`, `plugins/steop/.claude-plugin/plugin.json`, and `apps/steop` in lock-step.
- Users must rebuild/reinstall the `steop` binary (`/steop:install` or `cd apps/steop && go build -o target/steop . && cp target/steop ~/.local/bin/steop`, with `rm` before `cp` on macOS Tahoe per repo wisdom).
- No config, DB, or MCP contract changes. Clients that pre-specify `--x-session-id` / `--x-project-dir` continue to see no rewrite at all.

## Testing

All tests live in `apps/steop/internal/hooks/pre_tool_use_test.go`.

**Existing tests to update (assertion intent changes from "suffix at end" to "suffix immediately after `steop` token"):**

- `TestPreToolUseInjectsIdentity` (line 104) — change the expected command to `steop --x-session-id=<id> --x-project-dir='<dir>' <rest>`. Assert that the substring `steop --x-session-id=` appears before the subcommand.
- `TestPreToolUseInjectsSessionIDOnly` (line 121) — same position assertion, but with only `--x-session-id` in the suffix (no `--x-project-dir`).
- `TestPreToolUseChainedCommands` (line 156) — for a command like `steop a && steop b`, assert that both halves have the flags between `steop` and their respective subcommand tokens. Split on ` && ` and apply the position check to each part.
- `TestPreToolUseProjectDirWithSpaces` (line 198) — keep the existing quoting assertion; additionally assert that the quoted `--x-project-dir='<dir with spaces>'` sits before the subcommand.

**New tests to add:**

- `TestPreToolUseInjectsBeforeRedirection` — input `steop mailbox watch --type=task_request 2>&1`. Expected output `steop --x-session-id=<id> --x-project-dir='<dir>' mailbox watch --type=task_request 2>&1`. Assert explicitly that the substring index of `--x-session-id=` is less than the substring index of `2>&1`. This is the load-bearing Monitor case.
- `TestPreToolUseInjectsAfterEnvVarPrefix` — input `FOO=bar steop status`. Expected output `FOO=bar steop --x-session-id=<id> --x-project-dir='<dir>' status`. Assert the output begins with `FOO=bar steop ` and that the flags precede `status`.

**Manual verification:**

1. Build `steop` with the change, install to `~/.local/bin/steop`.
2. In a Claude Code session with a valid `session_id` and `CLAUDE_PROJECT_DIR`, invoke `Monitor` with `steop mailbox watch --type=task_request 2>&1`. Confirm (a) the rewritten command visible to the hook shows flags before `2>&1`, and (b) Monitor-emitted stdout lines are surfaced to the model as notifications (WATCHER:READY handshake from PRD-013 reaches the model).
3. Invoke `Bash` with `steop storage get foo` and confirm no regression in the simple no-redirection case.
