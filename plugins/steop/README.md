# steop

Agentic workflow pipeline for Claude Code.

steop provides a structured multi-phase workflow using specialized agents for clarification, research, planning, execution, and validation.

## Install

Add the marketplace in Claude Code, then install the steop plugin:

```
/plugin → Discover → Marketplaces → Add marketplace → tasanakorn/stele
/plugin → Discover → steop → Install
/reload-plugins
```

### Companion binary

The steop plugin ships Claude Code hooks that invoke a `steop` companion binary which must be on your `PATH`. After installing the plugin, run:

`/steop:install`

This runs `go install github.com/tasanakorn/stele/apps/steop@main` under the hood, dropping the binary into `~/.local/bin/steop`. Requires Go 1.22+ (Git is **not** required — the source is fetched via the Go module proxy). Make sure `~/.local/bin` is on your `PATH`. The human-facing version reported by `steop version` comes from `apps/steop/version.go`; Go's build metadata records the pseudo-version derived from the `main` commit.

### Local storage

Since v0.16.0 (PRD-020) session state, storage, phase, session lifecycle, and the event log live in a local SQLite DB at `~/.local/share/steop/steop.db` (override with `STEOP_DB`). The `/steop:install` post-install step runs `steop db init` to create the DB; `steop db path` prints the resolved path. Only the cross-host mailbox + notify surfaces still go over HTTP to `stele-server`. See [docs/steop/local-storage.md](../../docs/steop/local-storage.md) for the schema and recovery guidance.

## Skills

| Skill              | Command                      | Description                                                                 |
| ------------------ | ---------------------------- | --------------------------------------------------------------------------- |
| Install            | `/steop:install`             | Build and install the steop companion binary to ~/.local/bin                |
| Flow               | `/steop:st-flow`             | Full pipeline: clarify -> [research] -> plan -> execute -> validate         |
| Clarify            | `/steop:st-clarify`          | Analyze request, resolve ambiguities, produce task brief                    |
| Research           | `/steop:st-research`         | Deep codebase investigation and context gathering                           |
| Plan               | `/steop:st-plan`             | Design implementation strategy and blueprint                                |
| Execute            | `/steop:st-execute`          | Implement code changes according to plan                                    |
| Validate           | `/steop:st-validate`         | Review changes for correctness and completeness                             |
| Send               | `/steop:st-send`             | Send a task to another session by short project name                        |
| Watch              | `/steop:st-watch`            | Monitor mailbox for incoming task requests and process them autonomously    |
| Lite               | `/steop:st-lite`             | Compressed 3-phase pipeline (clarify -> execute -> validate), fast feedback |
| PRD                | `/steop:st-prd`              | Author a new PRD: interactive clarify -> docs-first research -> author       |
| Statusline Setup   | `/steop:statusline-setup`    | Configure Claude Code's statusLine to call `steop statusline`                  |

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
/steop:st-flow <task description>
```

Runs the full workflow from clarify to validate, adapting the pipeline based on complexity.

### Individual phases

Run phases independently when you need granular control:

```
/steop:st-clarify <task description>
/steop:st-research <what to investigate>
/steop:st-plan <task with prior context>
/steop:st-execute implement the approved plan
/steop:st-validate check the changes we just made
```

## Pipeline

| Complexity | Pipeline                                        |
| ---------- | ----------------------------------------------- |
| Simple     | Clarify -> Plan -> Execute -> Validate          |
| Standard   | Clarify -> Research -> Plan -> Execute -> Validate |
| Complex    | Clarify -> Research -> Plan -> Execute -> Validate |

The Clarify phase determines complexity, which controls pipeline shape and model selection for all subsequent phases.
