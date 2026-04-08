---
name: st-flow
description: Flow workflow chain that runs clarify → [explore] → plan → execute → validate. Explore is skipped for simple tasks. Use when the user wants a full end-to-end automated workflow.
---

# Flow Workflow Chain

Run the full workflow chain, adapting to complexity:

| Complexity | Pipeline                                           |
| ---------- | -------------------------------------------------- |
| Simple     | Clarify → Plan → Execute → Validate               |
| Standard   | Clarify → Explore → Plan → Execute → Validate     |
| Complex    | Clarify → Explore → Plan → Execute → Validate     |

## Instructions

Execute each phase in sequence, passing context forward. Each phase uses a dedicated agent with scoped tools and a specialized system prompt. The Clarify phase determines **complexity** (simple / standard / complex) which controls both pipeline shape and model selection.

### Agents

| Phase    | Agent                  | Model   | Tools                     | Color   |
| -------- | ---------------------- | ------- | ------------------------- | ------- |
| Clarify  | `steop:consultant`    | opus    | Glob, Grep, Read, Bash    | cyan    |
| Explore  | `steop:researcher`    | inherit | Glob, Grep, Read, Bash    | blue    |
| Plan     | `steop:architect`     | opus    | Glob, Grep, Read, Bash    | green   |
| Execute  | `steop:executor`      | inherit | All tools                 | yellow  |
| Validate | `steop:reviewer`      | sonnet  | Glob, Grep, Read, Bash    | magenta |

Agents with `inherit` model have their model overridden based on complexity.

### Phase 1: Clarify

Launch the **consultant** agent. It will:
- Do a lightweight codebase scan (3-5 tool calls)
- Parse the core intent and identify ambiguities
- Define scope and determine **complexity**: simple / standard / complex
- Produce a Task Brief

Present the Task Brief to the user and wait for confirmation. The confirmed complexity guides the rest of the pipeline.

### Phase 2: Explore — skip if Simple

**Skip this phase for Simple tasks** — the consultant's scan is sufficient, and the architect can read files as needed.

For Standard / Complex tasks, launch the **researcher** agent with model override:
- **Standard** → `model: "sonnet"`
- **Complex** → `model: "sonnet"`

**Parallel execution**: If the task spans multiple independent areas, launch multiple researcher agents in parallel (one per area). Combine their findings before proceeding.

### Phase 3: Plan

Launch the **architect** agent. Pass all available context (Task Brief + Explore findings if applicable). It will produce a step-by-step implementation blueprint.

Present the plan to the user and wait for approval before proceeding.

### Phase 4: Execute

Launch the **executor** agent with model override based on complexity:
- **Simple** → `model: "haiku"`
- **Standard** → `model: "sonnet"`
- **Complex** → `model: "opus"`

**Parallel execution**: If the plan contains independent steps, launch multiple executor agents in parallel — one per independent group. Each gets the full plan context but clear instructions on which steps to implement.

### Phase 5: Validate

Launch the **reviewer** agent. It will:
- Review all changes made in the execute phase
- Run relevant tests or linting if available
- Check for correctness, consistency, and completeness
- Report issues with severity ratings

After all phases complete, summarize the results to the user.
