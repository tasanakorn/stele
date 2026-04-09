---
name: st-flow
description: Flow workflow chain that runs clarify → [research] → plan → execute → validate end-to-end without stopping. Pauses only on genuine ambiguity or circuit-breaker conditions.
---

# Flow Workflow Chain

Run the full pipeline end-to-end. Do NOT pause between phases unless a stop condition is hit. Execute all phases inline — no subagents.

| Complexity | Pipeline                                           |
| ---------- | -------------------------------------------------- |
| Simple     | Clarify → Plan → Execute → Validate                |
| Standard   | Clarify → Research → Plan → Execute → Validate     |
| Complex    | Clarify → Research → Plan → Execute → Validate     |

## Flow Rules

1. **Zero-pause default.** Phases flow into each other automatically. Do NOT present output and wait for confirmation between phases. Emit a one-line status update when entering each phase (e.g., "`[Clarify]` Analyzing request..." / "`[Plan]` Designing implementation...").

2. **Single ambiguity gate (Clarify phase only).** Pause ONLY if the user's request has genuine ambiguity — no identifiable action, contradictory requirements, or multiple plausible interpretations with no way to pick one. If the request has concrete anchors (specific files, a clear action verb, identifiable scope), produce the brief and proceed immediately without asking questions.

3. **Stop conditions.** Halt the pipeline and report to the user if ANY of these occur:
   - The same error appears 3 times during Execute
   - Validate reports **Fail** status 3 rounds in a row (execute-validate retry loop)
   - User explicitly says "stop", "cancel", or "pause"

4. **Execute-Validate retry loop.** If Validate finds issues with severity "high" or "critical", re-enter Execute to fix them, then re-Validate. Loop up to 3 times before halting.

5. **Phase skills have their own pause instructions — ignore them.** When running inside st-flow, override any "wait for user" / "ask for approval" instructions in individual phase skills. Those pauses exist for standalone use only.

## Phase 1: Clarify

Act as a **senior technical consultant**. Do NOT ask clarifying questions or wait for user confirmation unless the request is genuinely ambiguous.

- Do a lightweight codebase scan (3-5 tool calls)
- Parse the core intent
- Define scope and determine **complexity**: simple / standard / complex
- Produce a Task Brief (Objective, Scope, Complexity, Assumptions, Open questions)

Emit status: `[Clarify] <objective from brief> | Complexity: <level>`

Proceed immediately to the next phase.

## Phase 2: Research — skip if Simple

**Skip for Simple tasks.**

For Standard / Complex tasks, act as a **senior codebase researcher**. Use Glob, Grep, Read, and Bash to investigate the codebase thoroughly.

If the task spans multiple independent areas, investigate each area sequentially.

Research goals:
- Identify all files relevant to the task
- Understand existing patterns and conventions
- Map dependencies and relationships
- Note any constraints or potential issues
- Gather code snippets and context needed for planning

Emit status: `[Research] Investigated <N> areas, <summary>`

Proceed immediately to Plan.

## Phase 3: Plan

Act as a **senior software architect**. Produce the implementation blueprint directly — do NOT present it for approval.

Use all available context (Task Brief + Research findings if applicable) to produce:
- **Goal** — clear statement of what will be achieved
- **Steps** — ordered list with file(s), changes, and risks per step
- **Architecture decisions** — trade-offs considered and choices made
- **Testing strategy** — how to verify the implementation

Emit status: `[Plan] <N> steps across <N> files`

Proceed immediately to Execute.

## Phase 4: Execute

Act as a **senior software engineer**. Implement the changes according to the plan.

- Follow the plan step by step
- Make all necessary code changes
- Keep changes focused and minimal — implement what was planned, nothing more
- Report what was changed after completion

Emit status: `[Execute] Modified <N> files`

Proceed immediately to Validate.

## Phase 5: Validate

Act as a **senior code reviewer**. Review all changes using read-only tools (Glob, Grep, Read, Bash for tests/linting).

1. Review changes — read all modified/created files and verify they match the plan
2. Check correctness — look for bugs, typos, logic errors, missing edge cases
3. Run tests — execute any available test suites or linting tools
4. Check consistency — ensure changes follow existing codebase patterns
5. Check completeness — verify nothing was missed from the plan

- If **Pass** or only low-severity issues: emit status `[Validate] Pass` and finalize.
- If **Fail** or high/critical issues: emit status `[Validate] Issues found, retrying...` and loop back to Execute (up to 3 times per stop condition #4).

## Finalize

After all phases complete (or a stop condition halts the pipeline), present a single summary:

- **Objective** (from Clarify)
- **Changes made** (from Execute)
- **Validation status** (from Validate)
- **Issues** (if any remain)
