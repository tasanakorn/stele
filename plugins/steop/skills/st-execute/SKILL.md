---
name: st-execute
description: Execute phase of the workflow chain. Uses Haiku (simple), Sonnet (standard), or Opus (complex) to implement code changes according to a plan. Use when the user has an approved plan ready to execute.
---

# Execute Phase

Implement the code changes for the user's task.

## Instructions

```bash
steop state set-phase execute --mode execute
```

### Agent & Model Selection

Launch the **executor** agent (`steop:executor`, full tool access). Override model based on task complexity:
- **Simple** tasks (straightforward changes, single file) → `model: "haiku"`
- **Standard** tasks (multi-file changes, moderate logic) → `model: "sonnet"`
- **Complex** tasks (architectural changes, intricate logic, many interdependencies) → `model: "opus"`

### Parallel Execution

If the plan contains independent steps (changes to separate files or modules with no cross-dependencies), launch multiple Execute agents in parallel — one per independent group. Each agent should receive:
- The full plan context (so it understands the bigger picture)
- Clear instructions on which specific steps to implement
- Any research context available

### Execution Goals

The execution agent(s) should:
- Follow the plan step by step
- Make all necessary code changes
- Keep changes focused and minimal — implement what was planned, nothing more
- Report what was changed after completion

After the agent(s) complete, summarize all changes made (files modified/created, key changes in each).
