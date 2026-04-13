# PRD-017: st-watch identity wiring for Monitor invocations

- **Status:** Implemented (v0.13.3)
- **Target version:** v0.13.2 – v0.13.3
- **Author:** Tasanakorn (design) + Claude Code (PRD authoring)

## Goals

- Update `/steop:st-watch` to consume PRD-016's surface: invoke `steop identity` via Bash to resolve the per-session `session_id` / `project_dir`, then embed those values into the Monitor-launched `mailbox watch` command literal so the watcher polls the correct mailbox even though the PreToolUse hook no longer rewrites Monitor invocations.
- Verify the resolved identity round-trips by comparing the new `WATCHER:READY` echo against the `steop identity` output.
- Land the flag-position correction (v0.13.3): public identity flags belong **after** the `mailbox watch` subcommand alongside `--type` and `--interval`, per the PRD-015 placement convention.

## Non-goals

- No code change in `apps/steop/` or `apps/stele/`. All runtime behavior — `steop identity`, the public `--session-id` / `--project-dir` flags, the extended `WATCHER:READY` payload, and the `Bash`-only PreToolUse matcher — landed in PRD-016 (v0.13.1).
- No edits to `docs/claude-code-integration.md`. That doc is unversioned and tracked separately.
- No new version bump beyond the two patches that already shipped (v0.13.2, v0.13.3).
- No deprecation of the implicit fallback path in `cmd_mailbox_watch.go` (env-derived `globalProjectDir` + most-recently-active session auto-pick). Explicit-flags is the contract for Monitor; the binary-level fallback remains intact.

## Background & Motivation

PRD-016 (v0.13.1) restricted the PreToolUse hook matcher from `Bash|Monitor` to `Bash` so that per-line stdout notifications from Monitor reach the model again. As a side effect, any `steop` subprocess launched via the Monitor tool no longer receives `--x-session-id` / `--x-project-dir` injection. To compensate, PRD-016 introduced public `--session-id` / `--project-dir` flags, a new `steop identity` subcommand for read-back, and an extended `WATCHER:READY` line that echoes `session_id` / `project_dir` for debug confirmation.

`/steop:st-watch` is the first (and currently only) consumer that launches `steop` under Monitor. Without explicit flags it would fall back to `cmd_mailbox_watch.go`'s defensive scan (env-derived `project_dir` + most-recently-active session auto-pick) and silently poll the wrong session whenever the watcher and the active session diverge. PRD-017 updates the skill to drive PRD-016's surface end-to-end.

### Current state (pre-v0.13.2)

`plugins/steop/skills/st-watch/SKILL.md` Step 1 ("Start Watcher") issued a single Monitor call with the literal:

```
steop mailbox watch --type=TASK:REQUEST --interval=10
```

No identity was resolved up-front, no public flags were passed, and the `WATCHER:READY` example documented only `message_type` and `interval`. The skill implicitly relied on hook injection, which had just been removed for Monitor.

## Design

### v0.13.2 — identity wiring

Step 1 was renamed from "Start Watcher" to "Resolve identity, then start watcher" and acquired three substeps:

1. **Bash-first identity probe.** A `steop identity` invocation runs via Bash before any Monitor call. Because Bash is still rewritten by the PreToolUse hook, `steop identity` reflects the same `session_id` / `project_dir` the hook would have injected, giving the LLM a deterministic source of truth.
2. **Documented JSON shape.** The skill embeds the canonical output shape (`session_id`, `project_dir`, `host`, `session_composite_id`, `project_composite_id`) so the LLM knows which fields to extract without re-deriving them from `cmd_identity.go`.
3. **READY verification.** A new sentence after the WATCHER:READY example instructs the LLM to compare the READY line's echoed `session_id` / `project_dir` against the `steop identity` output. Mismatch is framed as "the watcher will poll the wrong mailbox" — explicit failure mode, corrective action left to the LLM, consistent with other st-* skills.

The WATCHER:READY example in the skill was extended from `{"message_type":"WATCHER:READY","interval":<n>}` to include the two new `omitempty` fields, mirroring the actual emission at `apps/steop/cmd_mailbox_watch.go:203-216`.

### v0.13.3 — flag-position correction

The Monitor command literal moved its identity flags from before the `mailbox` token to after the `watch` subcommand:

- **Before (v0.13.2):** `steop --session-id=<uuid> --project-dir='<path>' mailbox watch --type=TASK:REQUEST --interval=10`
- **After (v0.13.3):** `steop mailbox watch --session-id=<uuid> --project-dir='<path>' --type=TASK:REQUEST --interval=10`

Both forms parse equivalently — `parseGlobalFlags` (`apps/steop/main.go:56-69`) is position-independent — but the post-subcommand placement matches the PRD-015 convention (flags grouped with peer flags like `--type` / `--interval`) and reads as a normal subcommand invocation rather than as a hook-rewrite artifact.

## Changes by Component

| Component                                                                       | Change                                                                                                                                                                                                                                                                                                                                          |
| ------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `plugins/steop/skills/st-watch/SKILL.md` (v0.13.2)                              | Renamed Step 1 to "Resolve identity, then start watcher". Added a Bash-first `steop identity` probe with documented JSON output shape. Extended the WATCHER:READY example with the new `session_id` / `project_dir` fields. Added a verification sentence comparing READY's identity to `steop identity` and naming the failure mode explicitly. |
| `plugins/steop/skills/st-watch/SKILL.md` (v0.13.3)                              | Moved `--session-id` / `--project-dir` from the position between `steop` and `mailbox` to the position after `watch`, sitting alongside `--type` and `--interval`.                                                                                                                                                                               |
| `apps/stele/Cargo.toml`, `apps/steop/version.go`, both plugin `plugin.json` files | Two lock-step patch bumps via `python scripts/bump-version.py patch` to v0.13.2 and then v0.13.3. No code touched.                                                                                                                                                                                                                                |
| `docs/README.md`                                                                | New PRD-017 row in the PRD table.                                                                                                                                                                                                                                                                                                               |

No changes outside `plugins/steop/skills/st-watch/SKILL.md` and the version-bump set — roughly twelve added lines and four modified lines across the two patches.

## Edge Cases

1. **Stale `~/.local/bin/steop` (pre-v0.13.1).** The `steop identity` probe in Step 1 fails loud with `unknown command: identity` plus a non-zero exit. The LLM sees stderr immediately rather than progressing into a Monitor call with no identity. Mitigation belongs to `/steop:install` cadence, not the skill.
2. **Multi-session race in the same repo.** Each Claude Code session's hook injects its own per-session UUID into Bash subshells (PRD-003 behavior), so two concurrent st-watch invocations in the same project each see their own `session_id`. The race only materializes if a user manually shares a Bash environment across sessions (e.g. `tmux send-keys`), which is out of scope.
3. **READY verification is advisory.** The skill instructs the LLM to verify the READY echo but leaves the corrective action (abort, restart, continue with caveat) to its judgement, matching the tone of other st-* skills.
4. **Implicit fallback still works.** If a future caller invokes `steop mailbox watch` under Monitor without flags, `cmd_mailbox_watch.go:194-201` still derives `project_dir` from `CLAUDE_PROJECT_DIR` and auto-picks the most recently active session. PRD-017 does not deprecate that path; it documents the explicit-flags contract as the correctness path for the watcher.
5. **Project paths with spaces.** The skill template wraps `--project-dir='<path>'` in single quotes inside the Monitor command literal, preserving the existing PRD-015 quoting convention. No regression.

## Migration

Already shipped; no migration required. Users on v0.13.1 or earlier should reinstall the steop binary and refresh the plugin to pick up v0.13.3 (skill text + matching binary behavior already on disk).

## Testing

Skill files are markdown — no unit tests. Verification is the WATCHER:READY echo comparison the skill itself performs at runtime. `steop identity`, the public flag aliases, and the extended WATCHER:READY emission are covered by the test set introduced in PRD-016.

**Manual smoke (already performed during v0.13.2 / v0.13.3 development):**

1. Reinstall the steop binary at v0.13.3 and refresh the Claude Code plugin so Monitor no longer triggers PreToolUse.
2. Invoke `/steop:st-watch` in a Claude Code session with a valid `session_id` and `CLAUDE_PROJECT_DIR`.
3. Confirm the Bash `steop identity` call returns a non-empty `session_id` and absolute `project_dir`.
4. Confirm the Monitor `WATCHER:READY` line surfaces to the model (per-line notifications, courtesy of the Bash-only matcher) and echoes the same `session_id` / `project_dir`.
5. Send a `TASK:REQUEST` to the watched mailbox via `/steop:st-send` and confirm the watcher claims it and responds with `TASK:DONE`.
