---
name: st-flow
description: Flow workflow chain that runs clarify → [research] → plan → execute → validate end-to-end without stopping. Pauses only on genuine ambiguity or circuit-breaker conditions.
---

# Flow Workflow Chain

Run the full pipeline end-to-end. Do NOT pause between phases unless a stop condition is hit.

| Complexity | Pipeline                                           |
| ---------- | -------------------------------------------------- |
| Simple     | Clarify → Plan → Execute → Validate                |
| Standard   | Clarify → Research → Plan → Execute → Validate     |
| Complex    | Clarify → Research → Plan → Execute → Validate     |

## Flow Rules

1. **Zero-pause default.** Phases flow into each other automatically. Do NOT present output and wait for confirmation between phases. Emit a one-line status update when entering each phase (e.g., "[Clarify] Analyzing request..." / "[Plan] Designing implementation...").

2. **Single ambiguity gate (Clarify phase only).** Pause ONLY if the user's request has genuine ambiguity — no identifiable action, contradictory requirements, or multiple plausible interpretations with no way to pick one. If the request has concrete anchors (specific files, a clear action verb, identifiable scope), the consultant MUST produce the brief and return immediately without asking questions or waiting.

3. **Stop conditions.** Halt the pipeline and report to the user if ANY of these occur:
   - The same error appears 3 times during Execute
   - Validate reports **Fail** status 3 rounds in a row (execute-validate retry loop)
   - User explicitly says "stop", "cancel", or "pause"

4. **Execute-Validate retry loop.** If Validate finds issues with severity "high" or "critical", automatically re-enter Execute to fix them, then re-Validate. Loop up to 3 times before halting.

5. **Phase skills have their own pause instructions — ignore them.** When running inside st-flow, override any "wait for user" / "ask for approval" instructions in individual phase skills. Those pauses exist for standalone use only.

## Statusline State Updates

At the start of each phase, run `steop state set-phase <phase> --mode flow` so the Claude Code statusline reflects live progress. This is best-effort and non-blocking: if the local steop DB is unreachable or no session identity has been resolved yet, the command silently exits 0 and the pipeline proceeds normally. After the Finalize step, run `steop state clear-phase` to reset the statusline to idle.

### Identity in nested subprocesses

The PreToolUse hook injects `--x-session-id` / `--x-project-dir` into `steop` invocations launched from `Bash`. Subprocesses launched outside Bash — Monitor, or any tool that forks `steop` without going through a Bash command — receive no injection. Pass `--session-id=<id>` and `--project-dir=<absolute path>` explicitly when calling `steop` in those contexts. Use `steop identity` to read back the resolved values and confirm what the hook provided.

## Agents

| Phase    | Agent               | Model   | Tools                  | Color   |
| -------- | ------------------- | ------- | ---------------------- | ------- |
| Clarify  | `steop:consultant`  | opus    | Glob, Grep, Read, Bash | cyan    |
| Research | `steop:researcher`  | inherit | Glob, Grep, Read, Bash | blue    |
| Plan     | `steop:architect`   | opus    | Glob, Grep, Read, Bash | green   |
| Execute  | `steop:executor`    | inherit | All tools              | yellow  |
| Validate | `steop:reviewer`    | sonnet  | Glob, Grep, Read, Bash | magenta |

Agents with `inherit` model have their model overridden based on complexity.

## Phase 1: Clarify

```bash
steop state set-phase clarify --mode flow
```

Launch the **consultant** agent. Pass the following override instruction:

> **FLOW MODE:** Do NOT ask clarifying questions or wait for user confirmation unless the request is genuinely ambiguous (no identifiable action, contradictory, or multiple incompatible interpretations). If the intent is clear enough to act on, produce the Task Brief per the shape below and return immediately. **State assumptions explicitly in the `Assumptions:` field of the brief; do NOT investigate to remove them.** If two or more concrete framings plausibly satisfy the request, list them under `Open questions:` rather than picking silently.

Task Brief output shape:

```
Objective:         <one line>
Assumptions:       <0–3 bullets — list explicitly; do NOT investigate to remove>
Complexity:        simple | standard | complex
Success criteria:  <1–3 bullets — verifiable; each is independently checkable>
Open questions:    <0–N bullets — alternate framings the consultant surfaced>
Groups:            [G1: files/area, G2: ..., G3: ...]   # only if independent
```

`Open questions:` in FLOW MODE is informational-only — it lists alternate framings the consultant chose not to investigate. It does NOT pause the pipeline; Rules 1–2 (zero-pause + single ambiguity gate) are unchanged. The Finalize step surfaces any non-empty `Open questions:` so the user sees what was assumed-away.

The consultant will:
- Do a lightweight codebase scan (3-5 tool calls)
- Parse the core intent
- Define scope and determine **complexity**: simple / standard / complex
- Produce a Task Brief

Emit status: `[Clarify] <objective from brief> | Complexity: <level>`

Proceed immediately to the next phase.

## Phase 2: Research — skip if Simple

```bash
steop state set-phase research --mode flow
```

**Skip for Simple tasks.**

For Standard / Complex tasks, launch the **researcher** agent with model override:
- **Standard** → `model: "sonnet"`
- **Complex** → `model: "sonnet"`

**Parallel execution**: If the task spans multiple independent areas, launch multiple researcher agents in parallel (one per area). Combine their findings before proceeding.

Pass the following override instruction:

> **FLOW MODE:** List at the end of your Research Summary the assumptions you made about *which files to read* and *which areas to skip* — don't investigate away every uncertainty; state them. If a relevant-looking file was deprioritized due to time, note it under `Assumptions` so the architect can reconsider.

Emit status: `[Research] Investigated <N> areas, <summary>`

Proceed immediately to Plan.

## Phase 3: Plan

```bash
steop state set-phase plan --mode flow
```

Launch the **architect** agent. Pass all available context (Task Brief + Research findings if applicable).

Pass the following override instruction:

> **FLOW MODE:** Produce the implementation blueprint and return it. Do NOT pause for approval — the executor will follow it directly. For each major design choice, include an inline **Alternatives considered:** line (simpler option, one-line rationale for rejection). Prefer the simpler of two equivalent designs; if YAGNI points one way and the plan goes the other, state the reason.

Emit status: `[Plan] <N> steps across <N> files`

Proceed immediately to Execute.

## Phase 4: Execute

```bash
steop state set-phase execute --mode flow
```

Launch the **executor** agent with model override based on complexity:
- **Simple** → `model: "haiku"`
- **Standard** → `model: "sonnet"`
- **Complex** → `model: "opus"`

**Parallel execution**: If the plan contains independent steps, launch multiple executor agents in parallel — one per independent group.

Pass the following override instruction:

> **FLOW MODE:** Implement the plan. Prefer YAGNI — skip defensive code, edge cases, and polish unless they block the happy path or are named in the plan. Leave TODOs where assumptions are load-bearing. Do NOT refactor neighboring code. Only remove imports, variables, or functions that your changes made unused. Return as soon as the planned steps are complete and the main path works.

Emit status: `[Execute] Modified <N> files`

Proceed immediately to Validate.

## Phase 5: Validate

```bash
steop state set-phase validate --mode flow
```

Launch the **reviewer** agent. It will review all changes, run tests/linting if available, and produce a verification report.

Pass the following override instruction:

> **FLOW MODE:** Check each bullet in the `Success criteria:` section of the Task Brief. A criterion is satisfied when you can *observe* it — run the command, read the file, open the page. Also: does the implementation match the Plan's steps? Are there obvious regressions in touched files? Report `Pass` or `Fail`. On `Fail`, name the first unsatisfied criterion (or regression) — the Execute-Validate retry loop (Rule 4) needs a concrete target to fix.

- If **Pass** or only low-severity issues: emit status `[Validate] Pass` and finalize.
- If **Fail** or high/critical issues: emit status `[Validate] Issues found, retrying...` and loop back to Execute (up to 3 times per stop condition #4).

## Finalize

After all phases complete (or a stop condition halts the pipeline), present a single summary:

- **Objective** (from Clarify)
- **Changes made** (from Execute)
- **Validation status** (from Validate)
- **Issues** (if any remain)

### PRD Status Update

If the flow was invoked with a PRD reference (e.g. `st-flow implement per docs/prd/prd-NNN-...`) and validation passed:

1. Read the PRD file and find the `**Status:**` line
2. Update it to `**Status:** Implemented (<version>)` where `<version>` is the target version from the PRD (or current workspace version if not specified)
3. Include the PRD file in the changes summary

Do NOT update the PRD status if:
- Validation failed and the pipeline halted
- The flow was not associated with a PRD
- The PRD status is already marked as implemented

```bash
steop state clear-phase
```
