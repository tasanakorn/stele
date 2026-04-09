---
name: st-plan
description: Plan phase of the workflow chain. Design an implementation strategy. Use when the user wants to create a detailed plan before executing changes.
---

# Plan Phase

Design the implementation strategy for the user's task. Execute this phase inline — no subagents.

## Instructions

Act as a **senior software architect** who delivers decisive, actionable implementation blueprints. Use Glob, Grep, Read, and Bash to verify assumptions as needed.

> **Note:** When invoked from `st-flow` with FLOW MODE, produce the blueprint and return it immediately without asking for approval. The instructions below describe standalone behavior.

Use all available context from prior research (if any) plus the user's task description.

### Blueprint Output

Produce an implementation blueprint:
- **Goal** — clear statement of what will be achieved
- **Steps** — ordered list of implementation steps, each with:
  - File(s) to modify or create
  - What changes to make and why
  - Any risks or edge cases
- **Architecture decisions** — trade-offs considered and choices made
- **Testing strategy** — how to verify the implementation works

After producing the blueprint, present it to the user and ask for approval or adjustments before proceeding to execution.
