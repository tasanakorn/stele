# stelite

Lightweight inline workflow pipeline for Claude Code.

stelite provides a structured multi-phase workflow that executes all phases inline — the current model performs each phase directly without launching subagents. Same pipeline structure as steop, lighter overhead.

## Install

Add the marketplace in Claude Code, then install the stelite plugin:

```
/plugin -> Discover -> Marketplaces -> Add marketplace -> tasanakorn/stele
/plugin -> Discover -> stelite -> Install
/reload-plugins
```

## Skills

| Skill    | Command                | Description                                                          |
| -------- | ---------------------- | -------------------------------------------------------------------- |
| Flow     | `/stelite:st-flow`     | Full pipeline: clarify -> [research] -> plan -> execute -> validate  |
| Clarify  | `/stelite:st-clarify`  | Analyze request, resolve ambiguities, produce task brief             |
| Research | `/stelite:st-research` | Deep codebase investigation and context gathering                    |
| Plan     | `/stelite:st-plan`     | Design implementation strategy and blueprint                         |
| Execute  | `/stelite:st-execute`  | Implement code changes according to plan                             |
| Validate | `/stelite:st-validate` | Review changes for correctness and completeness                      |

## Usage

### Full pipeline

```
/stelite:st-flow <task description>
```

Runs the full workflow from clarify to validate, adapting the pipeline based on complexity.

### Individual phases

Run phases independently when you need granular control:

```
/stelite:st-clarify <task description>
/stelite:st-research <what to investigate>
/stelite:st-plan <task with prior context>
/stelite:st-execute implement the approved plan
/stelite:st-validate check the changes we just made
```

## Pipeline

| Complexity | Pipeline                                           |
| ---------- | -------------------------------------------------- |
| Simple     | Clarify -> Plan -> Execute -> Validate             |
| Standard   | Clarify -> Research -> Plan -> Execute -> Validate |
| Complex    | Clarify -> Research -> Plan -> Execute -> Validate |

The Clarify phase determines complexity, which controls whether Research is included.

## Comparison with steop

| Aspect        | steop                     | stelite                       |
| ------------- | ------------------------- | ----------------------------- |
| Execution     | Subagents (5 specialized) | Inline (current model)        |
| Parallelism   | Multi-agent parallel      | Sequential                    |
| Model control | Per-phase model overrides | Uses current model throughout |
| Overhead      | Higher (agent spawning)   | Lower (no context switching)  |
| Best for      | Complex multi-area tasks  | Focused single-area tasks     |
