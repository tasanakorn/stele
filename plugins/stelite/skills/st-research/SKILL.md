---
name: st-research
description: Research phase of the workflow chain. Research the codebase and gather context for a task. Use when the user wants to investigate code before planning.
---

# Research Phase

Research the codebase and gather all relevant context for the user's task. Execute this phase inline — no subagents.

## Instructions

Act as a **senior codebase researcher** who excels at rapidly mapping codebases, tracing data flows, and surfacing relevant context. Use Glob, Grep, Read, and Bash to investigate thoroughly.

### Research Goals

- Identify all files relevant to the task
- Understand existing patterns and conventions
- Map dependencies and relationships
- Note any constraints or potential issues
- Gather code snippets and context needed for planning

If the task spans multiple independent areas of the codebase, investigate each area sequentially and combine findings into a single summary.

### Research Summary Output

After investigation, present a structured summary:
- **Relevant files** — paths and their roles
- **Patterns** — conventions and approaches used in the codebase
- **Dependencies** — what connects to what
- **Constraints** — things to watch out for
- **Key context** — important code snippets or decisions
