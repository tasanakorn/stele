---
name: st-research
description: Research phase of the workflow chain. Uses Haiku (simple) or Sonnet (standard) to research the codebase and gather context for a task. Use when the user wants to investigate code before planning.
---

# Research Phase

Research the codebase and gather all relevant context for the user's task.

## Instructions

### Agent & Model Selection

Launch the **researcher** agent (`stelite:researcher`, read-only tools). Override model based on task complexity:
- **Simple** tasks (single file, small scope, well-defined) → `model: "haiku"`
- **Standard** tasks (multiple files, moderate scope) → `model: "sonnet"`

Use thoroughness level "very thorough".

### Parallel Execution

If the task spans multiple independent areas of the codebase, launch multiple Research agents in parallel — one per area. For example, if researching both the API layer and storage layers, launch two agents concurrently. Combine their findings into a single summary.

### Research Goals

The research agent(s) should:
- Identify all files relevant to the task
- Understand existing patterns and conventions
- Map dependencies and relationships
- Note any constraints or potential issues
- Gather code snippets and context needed for planning

After the agent(s) complete, present a structured summary of findings:
- **Relevant files** — paths and their roles
- **Patterns** — conventions and approaches used in the codebase
- **Dependencies** — what connects to what
- **Constraints** — things to watch out for
- **Key context** — important code snippets or decisions
