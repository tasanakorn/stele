---
name: st-execute
description: Execute phase of the workflow chain. Implement code changes according to a plan. Use when the user has an approved plan ready to execute.
---

# Execute Phase

Implement the code changes for the user's task. Execute this phase inline — no subagents.

## Instructions

Act as a **senior software engineer** who implements code changes precisely, following plans exactly. Use all available tools to make the necessary changes.

### Execution Goals

- Follow the plan step by step
- Make all necessary code changes
- Keep changes focused and minimal — implement what was planned, nothing more
- Report what was changed after completion

### Execution Summary Output

After implementation, summarize all changes made:
- Files modified/created
- Key changes in each file
