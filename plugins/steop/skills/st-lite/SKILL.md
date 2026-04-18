---
name: st-lite
description: Compressed 3-phase fast-feedback workflow chain (clarify → execute → validate) with cap-3 parallel executors, no retry loop, fail-fast. For prototypes, spikes, and quick refactors.
---

# Lite Workflow Chain

Run a compressed, fast-feedback pipeline. Mirrors `/steop:st-flow`'s phase names (`Clarify → Execute → Validate`) so you carry one mental model across skills; the `--mode lite` tag signals compressed semantics: Research and Plan are skipped, retry loop is off, executors fan out eagerly (cap 3), assumptions are stated explicitly rather than investigated away, and scope stays YAGNI.

Use `st-lite` for: prototypes, spikes, mechanical refactors, quick experiments. Use `/steop:st-flow` instead for: production changes, breaking-change work, anything where correctness matters more than wall-clock time.

## Flow Rules

1. **Zero-pause default.** Phases flow into each other automatically. Do NOT present output and wait for confirmation between phases. Emit a one-line status update when entering each phase (e.g., "[lite] Clarify: <objective>").

2. **Single ambiguity gate (Clarify phase only).** Pause ONLY if the user's request has genuine ambiguity — no identifiable action, contradictory requirements, or multiple plausible interpretations with no way to pick one. Otherwise produce the brief and return immediately.

3. **Stop conditions.** Halt the pipeline and report if ANY of these occur:
   - The same error appears 3 times during Execute
   - Validate reports `fail` (fail-fast — no retry)
   - User explicitly says "stop", "cancel", or "pause"

4. **No retry loop.** If Validate reports `fail`, halt and report. The user decides whether to iterate manually, rerun `st-lite` with adjusted scope, or escalate to `/steop:st-flow`.

5. **Phase skills have their own pause instructions — ignore them.** When running inside st-lite, override any "wait for user" / "ask for approval" instructions in individual phase skills. Those pauses exist for standalone use only.

## Statusline State Updates

At the start of each phase, run `steop state set-phase <phase> --mode lite` so the Claude Code statusline reflects live progress as `[lite] <phase>: <detail>`. This is best-effort and non-blocking: if the local steop DB is unreachable or no session identity has been resolved yet, the command silently exits 0 and the pipeline proceeds normally. After Finalize, run `steop state clear-phase` to reset the statusline to idle.

### Identity in nested subprocesses

The PreToolUse hook injects `--x-session-id` / `--x-project-dir` into `steop` invocations launched from `Bash`. Subprocesses launched outside Bash — Monitor, or any tool that forks `steop` without going through a Bash command — receive no injection. Pass `--session-id=<id>` and `--project-dir=<absolute path>` explicitly when calling `steop` in those contexts. Use `steop identity` to read back the resolved values and confirm what the hook provided.

## Agents

| Phase    | Agent              | Model  | Tools                  | Color   |
| -------- | ------------------ | ------ | ---------------------- | ------- |
| Clarify  | `steop:consultant` | opus   | Glob, Grep, Read, Bash | cyan    |
| Execute  | `steop:executor`   | sonnet | All tools              | yellow  |
| Validate | `steop:reviewer`   | sonnet | Glob, Grep, Read, Bash | magenta |

Execute downgrades to `haiku` only when Clarify emits `complexity=trivial` (pure renames, single-flag toggles, mechanical edits). Clarify stays on opus — Lite saves cost by skipping Research/Plan, not by underpowering reasoning.

## Phase 1: Clarify

```bash
steop state set-phase clarify --mode lite
```

Launch the **consultant** agent. Pass the following override instruction:

> **LITE MODE:** Produce a minimal brief in 1–3 tool calls max. Do NOT ask questions unless the request is genuinely ambiguous. Emit the full brief shape: 1-line objective, explicit `Assumptions:` list (zero or more bullets — if none, write `(none)`), approach confidence (`high` or `ambiguous`), complexity guess (`trivial`/`moderate`/`complex`), `Success criteria:` (1–3 verifiable bullets), and optional `Groups:` list when the work splits into disjoint file sets. If complexity=`complex`, append a one-line suggestion to consider `/steop:st-flow` instead — but still proceed. **State assumptions explicitly in the `Assumptions:` field; do NOT investigate to remove them.**

Brief output shape:

```
Objective:         <one line>
Assumptions:       <0–3 bullets — list explicitly; do NOT investigate to remove>
Approach:          high | ambiguous
Complexity:        trivial | moderate | complex
Success criteria:  <1–3 bullets — verifiable; each is independently checkable>
Groups:            [G1: files/area, G2: ..., G3: ...]   # only if independent
```

Emit status: `[lite] Clarify: <objective>`

Proceed immediately to Execute.

## Phase 2: Execute

```bash
steop state set-phase execute --mode lite
```

Launch **executor** agent(s). Model: **sonnet** default; downgrade to **haiku** only if Clarify emits `complexity=trivial`. Pass the following override:

> **LITE MODE:** Implement the smallest working slice. Prefer YAGNI — skip defensive code, edge cases, and polish unless they block the happy path. Leave TODOs where assumptions are load-bearing. Do NOT refactor neighboring code. Return as soon as the main path works.

### Parallel fan-out (cap 3)

- **Independent groups (default):** If Clarify emitted disjoint `Groups:`, launch one executor per group, up to 3 in parallel. Collapse the tail if more than 3 groups exist (merge into the last, or defer with a TODO in the Finalize block).
- **Parallel exploration (opt-in):** If Clarify emitted `approach: ambiguous` OR the user passed `--explore` in the invocation, launch up to 3 executors on the **same** problem with different approaches. The Finalize block surfaces all 3; the user picks one.
- **Otherwise:** single executor.

Emit status: `[lite] Execute: <N> parallel` (or `[lite] Execute: 1` for single).

Proceed immediately to Validate.

## Phase 3: Validate

```bash
steop state set-phase validate --mode lite
```

Launch the **reviewer** agent (sonnet). Single serial pass — no fan-out. Pass the following override:

> **LITE MODE:** Check each bullet in the `Success criteria:` section of the Clarify Brief. A criterion is satisfied when you can *observe* it — run the command, read the file, open the page. Also smoke-check the generics: does it build? does the main path run? are there obvious regressions in touched files? Do NOT audit exhaustively, do NOT run full test suites unless the project has a fast `make check` or equivalent. Report `pass` or `fail` with a one-line reason that references the criterion it passed or the first one it failed.

- If `pass` (or only low-severity notes): emit status `[lite] Validate: pass` and finalize.
- If `fail`: emit status `[lite] Validate: fail`, halt, and present the failure summary. **Do NOT retry.**

## Finalize

Present a single summary:

- **Objective** (from Clarify)
- **Changes made** (from Execute — include all parallel branches if exploration mode)
- **Validate status** (pass/fail + one-line reason)
- **Known-unknowns** (what a full `st-flow` would have checked but `st-lite` skipped — e.g. edge cases, cross-module effects, exhaustive tests)

**No PRD status auto-update.** `st-lite` is exploratory; even when the run corresponds to a PRD, the user closes out the PRD manually (unlike `/steop:st-flow`, which auto-updates PRD status on validation pass).

```bash
steop state clear-phase
```
