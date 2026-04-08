---
name: st-plan
description: Plan phase of the workflow chain. Uses Opus to design an implementation strategy. Use when the user wants to create a detailed plan before executing changes.
---

# Plan Phase

Design the implementation strategy for the user's task.

## Instructions

Launch the **architect** agent (`steop:architect`, Opus, read-only tools).

Provide the agent with all available context from prior exploration (if any) plus the user's task description.

The planning agent should produce:
- **Goal** — clear statement of what will be achieved
- **Steps** — ordered list of implementation steps, each with:
  - File(s) to modify or create
  - What changes to make and why
  - Any risks or edge cases
- **Architecture decisions** — trade-offs considered and choices made
- **Testing strategy** — how to verify the implementation works

After the agent completes, present the plan to the user and ask for approval or adjustments before proceeding to execution.
