# stelite

Agentic workflow pipeline for Claude Code.

stelite provides a structured multi-phase workflow using specialized agents for clarification, research, planning, execution, and validation.

## Install

Add the marketplace in Claude Code, then install the stelite plugin:

```
/plugin → Discover → Marketplaces → Add marketplace → tasanakorn/stele
/plugin → Discover → stelite → Install
/reload-plugins
```

## Skills

| Skill    | Command             | Description                                                              |
| -------- | ------------------- | ------------------------------------------------------------------------ |
| Flow     | `/stelite:st-flow`     | Full pipeline: clarify -> [research] -> plan -> execute -> validate     |
| Clarify  | `/stelite:st-clarify`  | Analyze request, resolve ambiguities, produce task brief               |
| Research | `/stelite:st-research` | Deep codebase investigation and context gathering                      |
| Plan     | `/stelite:st-plan`   | Design implementation strategy and blueprint                             |
| Execute  | `/stelite:st-execute` | Implement code changes according to plan                                |
| Validate | `/stelite:st-validate` | Review changes for correctness and completeness                        |

## Agents

| Agent      | Model   | Role                                    |
| ---------- | ------- | --------------------------------------- |
| consultant | opus    | Requirements analysis and scoping       |
| researcher | inherit | Codebase investigation and mapping      |
| architect  | opus    | Implementation design and planning      |
| executor   | inherit | Code implementation                     |
| reviewer   | sonnet  | Change validation and quality checks    |

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

| Complexity | Pipeline                                        |
| ---------- | ----------------------------------------------- |
| Simple     | Clarify -> Plan -> Execute -> Validate          |
| Standard   | Clarify -> Research -> Plan -> Execute -> Validate |
| Complex    | Clarify -> Research -> Plan -> Execute -> Validate |

The Clarify phase determines complexity, which controls pipeline shape and model selection for all subsequent phases.
