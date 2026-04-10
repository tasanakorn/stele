---
name: st-clarify
description: Clarify phase of the workflow chain. Uses Opus to analyze the user's request, resolve ambiguities, define scope, and produce a clear task brief before research begins.
---

# Clarify Phase

Analyze and clarify the user's request before any codebase research.

## Instructions

```bash
steop state set-phase clarify --mode clarify
```

Launch the **consultant** agent (`steop:consultant`, Opus, read-only tools).

> **Note:** When invoked from `st-flow` with FLOW MODE, skip the wait-for-confirmation step and return the Task Brief immediately. The instructions below describe standalone behavior.

### Step 1: Lightweight Codebase Scan

Before asking questions, do a quick orientation scan to ground the clarification in reality:
- `Glob` for project structure (top-level dirs, key config files)
- `Grep` for patterns directly related to the user's request (function names, components, modules mentioned)
- Read CLAUDE.md or README if present for project context

This is NOT a full research pass — spend no more than 3-5 tool calls. Just enough to understand the landscape.

### Step 2: Clarify

With codebase context in hand, the agent should:
- Parse the user's request and identify the core intent
- Spot ambiguities, missing details, or implicit assumptions
- Define the scope — what's in and what's out
- Ask the user clarifying questions if anything is unclear or under-specified (questions should be informed by what was found in the scan)
- Produce a **Task Brief** once clarity is reached

### Task Brief Output

After clarification, present a structured brief:
- **Objective** — one-sentence statement of what will be done
- **Scope** — explicit boundaries (what's included, what's excluded)
- **Complexity** — simple / standard / complex (this guides model selection in later phases)
- **Assumptions** — anything assumed that wasn't explicitly stated
- **Open questions** — any remaining questions for the user (if none, state "None")

Wait for the user to confirm or adjust the brief before proceeding to the next phase.
