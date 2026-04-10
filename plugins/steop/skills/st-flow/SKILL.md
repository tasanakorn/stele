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

At the start of each phase, write the current phase into session state so the Claude Code statusline (see `/steop:statusline-setup`) reflects live progress. The session id is discovered from the most recently updated session (hooks create one on the first tool call).

```bash
SID="$(steop monitor --json --limit=1 2>/dev/null | grep -o '"session_id"[[:space:]]*:[[:space:]]*"[^"]*"' | head -1 | sed -E 's/.*"([^"]+)"$/\1/')"
if [ -n "$SID" ]; then
  steop state set "$SID" '{"mode":"flow","phase":"<PHASE>"}'
fi
```

Replace `<PHASE>` with the phase name you are entering: `clarify`, `research`, `plan`, `execute`, or `validate`. Run this block once at the top of each `Phase N` section below, before launching the agent. If `SID` is empty (no session yet — first phase of a fresh run), silently skip; subsequent phases will succeed once a hook has created the session.

This is best-effort and non-blocking. If `steop` or the server is unavailable, the pipeline proceeds normally.

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

Update statusline state: write `{"mode":"flow","phase":"clarify"}` via the Statusline State Updates helper above.

Launch the **consultant** agent. Pass the following override instruction:

> **FLOW MODE:** Do NOT ask clarifying questions or wait for user confirmation unless the request is genuinely ambiguous (no identifiable action, contradictory, or multiple incompatible interpretations). If the intent is clear enough to act on, produce the Task Brief and return immediately. Prefer making reasonable assumptions over asking questions.

The consultant will:
- Do a lightweight codebase scan (3-5 tool calls)
- Parse the core intent
- Define scope and determine **complexity**: simple / standard / complex
- Produce a Task Brief

Emit status: `[Clarify] <objective from brief> | Complexity: <level>`

Proceed immediately to the next phase.

## Phase 2: Research — skip if Simple

Update statusline state: write `{"mode":"flow","phase":"research"}` via the Statusline State Updates helper above.

**Skip for Simple tasks.**

For Standard / Complex tasks, launch the **researcher** agent with model override:
- **Standard** → `model: "sonnet"`
- **Complex** → `model: "sonnet"`

**Parallel execution**: If the task spans multiple independent areas, launch multiple researcher agents in parallel (one per area). Combine their findings before proceeding.

Emit status: `[Research] Investigated <N> areas, <summary>`

Proceed immediately to Plan.

## Phase 3: Plan

Update statusline state: write `{"mode":"flow","phase":"plan"}` via the Statusline State Updates helper above.

Launch the **architect** agent. Pass all available context (Task Brief + Research findings if applicable).

Pass the following override instruction:

> **FLOW MODE:** Produce the implementation blueprint and return it. Do NOT present it for approval or ask for adjustments. The executor will follow it directly.

Emit status: `[Plan] <N> steps across <N> files`

Proceed immediately to Execute.

## Phase 4: Execute

Update statusline state: write `{"mode":"flow","phase":"execute"}` via the Statusline State Updates helper above.

Launch the **executor** agent with model override based on complexity:
- **Simple** → `model: "haiku"`
- **Standard** → `model: "sonnet"`
- **Complex** → `model: "opus"`

**Parallel execution**: If the plan contains independent steps, launch multiple executor agents in parallel — one per independent group.

Emit status: `[Execute] Modified <N> files`

Proceed immediately to Validate.

## Phase 5: Validate

Update statusline state: write `{"mode":"flow","phase":"validate"}` via the Statusline State Updates helper above.

Launch the **reviewer** agent. It will review all changes, run tests/linting if available, and produce a verification report.

- If **Pass** or only low-severity issues: emit status `[Validate] Pass` and finalize.
- If **Fail** or high/critical issues: emit status `[Validate] Issues found, retrying...` and loop back to Execute (up to 3 times per stop condition #4).

## Finalize

After all phases complete (or a stop condition halts the pipeline), present a single summary:

- **Objective** (from Clarify)
- **Changes made** (from Execute)
- **Validation status** (from Validate)
- **Issues** (if any remain)
