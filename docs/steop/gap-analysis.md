# Gap Analysis: steop vs cerbrix vs omc

> **Update (2026-04, v0.16.0): session KV / state / storage / status / event log / session lifecycle moved to local SQLite (`~/.local/share/steop/steop.db`) per PRD-020. References to `/api/v1/steop/state`, `/api/v1/steop/storage`, `/api/v1/steop/session`, `/api/v1/steop/status`, `/api/v1/steop/log` and matching server tables below are historical. Only `steop.mailbox.*` and `steop.notify` remain on stele-server HTTP.**

This document compares three Claude Code plugin ecosystems as of **2026-04-11**: **steop** (this repo, `plugins/steop/` + `apps/steop/`), **cerbrix** (`../cerbrix/`, a project-local agentic workflow system), and **oh-my-claudecode** / **omc** (`../oh-my-claudecode/`, published as `oh-my-claude-sisyphus` v4.11.2). The analysis covers ten axes: agents, skills, hooks, pipeline, runtime, safety, observability, state, notifications, and install. Source fact-gathering was performed by three parallel research agents; raw counts are treated as authoritative. External projects will drift; this is a snapshot.

## TL;DR
- **omc is maximalist** (19 agents, 37 skills, 10 hook events, Node+Python+Go, built-in MCP server, SWE-bench harness). It bets on surface area and benchmarked optimization.
- **cerbrix is the structural ancestor** of steop (same 6 Bash deny regexes, same hook skeleton). It bets on project-local isolation, keyword-injected skills, and tmux team coordination.
- **steop is the disciplined minimalist** (5 agents, 8 skills, 4 hooks, single Go binary, stele-backed state). It bets on workflow discipline (linear clarify→research→plan→execute→validate) plus persistent cross-session memory via stele.
- **steop's biggest real gaps**: no keyword-injection UX, no `recap`/session-orientation skill, no HUD beyond statusline, no PRD-driven persistence loop, no benchmark harness, no `SubagentStart/Stop` lifecycle tracking.
- **steop's biggest wins**: stele-backed cross-project memory (neither competitor has this), dedicated Clarify phase with ambiguity gate, targeted Opus routing, zero-dependency Go binary, statusline rendered in-process.

## Quick Comparison Matrix
| Axis          | steop                                  | cerbrix                               | omc                                               |
| ------------- | -------------------------------------- | ------------------------------------- | ------------------------------------------------- |
| Agents        | 5 (consul, resear, arch, exec, review) | 5 (arch, exec, explore, plan, verify) | 19 (incl scientist, critic, designer, qa)         |
| Skills        | 8                                      | 12                                    | 37 + 4 dev missions                               |
| Hooks         | 4 events                               | 5 events                              | 10 events                                         |
| Pipeline      | Linear 5-phase w/ Clarify gate         | 3-phase autopilot (plan/exec/verify)  | 4 overlapping modes (team/autopilot/ralph/ralplan)|
| Runtime       | Go 1.22+, single binary                | Go 1.22+ (Cobra), single binary       | Node.js/TS + Python REPL, no Go                   |
| Safety        | 6 Bash regex denies                    | 6 Bash regex denies (shared lineage)  | PreTool + PermissionRequest + 8 read-only agents  |
| Observability | 2-line statusline (in-proc Go)         | 2-line statusline (shell) + HUD       | HUD (3 presets) + trace MCP + SWE-bench           |
| State         | stele-server remote (per-session merge)| Project-local `.cerbrix/` JSON        | `.omc/` + SQLite + wiki + project-memory          |
| Notifications | Desktop only (notify-rust)             | None                                  | Telegram + Discord + Slack                        |
| Install       | Plugin + `go install`                  | Plugin + taskfile to `/usr/local/bin` | Plugin + npm global + MCP auto-register           |

## Axis 1: Agents
- steop 5 (Opus: consultant+architect; Sonnet: reviewer; inherit: researcher+executor); omc 19 (Opus across 8); cerbrix 5 (Opus: architect+planner). steop Opus-frugal; omc Opus-heavy. Source: `plugins/steop/agents/`.

## Axis 2: Skills
- steop 8 `/steop:*`; cerbrix 12 `/cerbrix:cb-*` with `agent:` + `level: 1-3` frontmatter; omc 37 + 4 missions, no prefix. cerbrix's per-skill metadata is the useful gap.

## Axis 3: Hooks
- steop 4 (UserPromptSubmit, PreToolUse Bash, PostToolUse, Stop); cerbrix 5 (+SessionEnd, keyword injection); omc 10 (+SessionStart, PermissionRequest, PostToolUseFailure, SubagentStart/Stop, PreCompact, SessionEnd). Missing `SessionEnd`/`PreCompact`/`SubagentStart/Stop` is the biggest gap. See `plugins/steop/hooks/hooks.json`.

## Axis 4: Pipeline & Orchestration
- steop linear 5 (clarify→research→plan→exec→validate), retry loop max 3; cerbrix 3-phase cb-auto, max 5 step retries / 3 verify loops, no research; omc 4 modes (team/autopilot/ralph/ralplan) + deep-interview Ouroboros. omc's 4 modes create choice paralysis.

## Axis 5: Runtime & Build
- steop Go 1.22+, CGO off, `go install ...@main` → `~/.local/bin/steop`; cerbrix Go + Taskfile → `/usr/local/bin/cerbrix`; omc Node.js/TS + Python bridge + better-sqlite3 + `@ast-grep/napi`. steop is zero-dep; omc is heaviest.

## Axis 6: Safety
- steop 6 Bash deny regexes (force-push variants, `rm -rf /`, `rm -rf ~/`, `rm -rf $HOME`); cerbrix 6 regexes of shared lineage with slightly different coverage (no `$HOME` variant) + agent `disallowedTools`; omc PreTool enforcer + PermissionRequest + 8/19 read-only agents + MCP tool annotations + kill switches (`DISABLE_OMC`, `OMC_SKIP_HOOKS`). omc's agent-level read-only and MCP annotations worth cherry-picking. Source: `apps/steop/internal/hooks/pre_tool_use.go`.

## Axis 7: Observability
- steop 2-line statusline rendered in-proc + `steop monitor`/`inspect`, no HUD; cerbrix statusline shells to `cerbrix hud render`; omc HUD 3 presets + `trace_timeline`/`trace_summary` MCP tools + SWE-bench Docker harness. steop's statusline is fastest; omc's depth is widest.

## Axis 8: State & Memory
- steop per-session state via local SQLite at `~/.local/share/steop/steop.db` (post-v0.16.0; previously stele `PUT /api/v1/steop/state/{id}` merge mode), cross-machine memory via stele MCP; cerbrix `.cerbrix/` flat JSON, project-local only; omc `.omc/state/` + SQLite + `project-memory.json` + wiki sync + `<remember>` tags. steop is the only project with cross-machine shared memory.

## Axis 9: Notifications
- steop desktop-only via `notify-rust` (Stop hook); cerbrix none (tmux only); omc Telegram + Discord + Slack (session end, need-input, bg done). Desktop covers primary case; multi-channel is nice-to-have.

## Axis 10: Install & Distribution
- steop local marketplace + `/steop:install` runs `go install`, prereqs Go 1.22+ and stele-server on :3100; cerbrix plugin + Taskfile build, prereqs Go + Task + tmux (opt); omc plugin + `npm i -g oh-my-claude-sisyphus` + local checkout, prereqs Node + Python. steop is simplest to install; omc has widest distribution. See `plugins/steop/.claude-plugin/plugin.json`.

## Gaps steop Could Close (Prioritized)

### High
1. **Session recap skill** — add `/steop:recap` that loads current session state, recent plans, last phase, dangling retries. Why: users context-switch between terminals and lose track of where the pipeline is. Effort: 1 skill + `steop state` read. Source: cerbrix `cb-recap`.
2. **Keyword injection in `UserPromptSubmit`** — detect trigger phrases (`flow:`, `clarify:`, `plan:`) and inject the corresponding `SKILL.md`. Why: reduces friction vs typing `/steop:st-flow`. Effort: extend `steop hook user_prompt_submit`. Source: cerbrix, omc.
3. **`SubagentStop` hook for deliverable verification** — detect when researcher/architect returns empty or malformed output. Why: silent agent failures are the single biggest pipeline failure mode. Effort: new hook event + reviewer nudge. Source: omc.
4. **`SessionEnd` hook for archival** — persist final phase, tool_calls, retries, loop_count to stele as a session summary memory. Why: closes the observability loop — users can `/stele:sync` and see what past sessions did. Effort: new hook handler + stele `store_memory` call. Source: cerbrix `SessionEnd`, omc.

### Medium
5. **Full `steop hud` TUI** — full-screen view showing phase timeline, retry history, step list from the active plan. Why: statusline is too cramped for multi-retry debugging. Effort: ~1-2 days with `bubbletea`. Source: cerbrix `hud render`, omc HUD presets.
6. **Plan file persistence convention** — write `plans/plan-<session>.md` on Plan phase output, reference it in statusline and `steop inspect`. Why: currently plans live only in conversation history. Effort: architect agent writes file + state tracks path. Source: cerbrix `.cerbrix/plans/plan-*.md`.
7. **Agent-per-skill explicit binding** — add `agent:` field to SKILL.md frontmatter so skills declaratively route to their agent. Why: eliminates implicit orchestration logic in SKILL.md prose. Effort: schema extension + loader change. Source: cerbrix.
8. **`level:` complexity field on skills** — formalize the simple/standard/complex tiers that drive model overrides. Why: complexity is currently consultant-internal and invisible. Effort: frontmatter convention + consultant reads it. Source: cerbrix, omc.
9. **`PreCompact` hook** — before Claude Code compacts context, dump current phase/step/retries into a stele memory tagged `#compact-rescue`. Why: long pipelines currently lose state on compaction. Effort: new hook event. Source: omc.

### Low
10. **Multi-channel notifications** — add Telegram/Discord webhooks to the existing desktop notifier. Why: remote monitoring for long pipelines. Effort: config + 2 clients in stele-server. Source: omc.
11. **PRD-driven persistence loop** — `/steop:ralph`-style mode that iterates Execute+Validate until all acceptance criteria pass. Why: useful for greenfield feature work. Effort: new orchestration skill + state field. Source: omc `ralph`.
12. **Benchmark harness** — track pipeline success rate vs vanilla Claude Code on a small internal task set. Why: empirical justification for architectural choices. Effort: substantial. Source: omc SWE-bench.

## Things steop Does Better

- **Cross-project shared memory via stele** — neither cerbrix (project-local `.cerbrix/`) nor omc (project-local `.omc/` + wiki) has a multi-machine shared memory server. This is steop's structural advantage.
- **Dedicated Clarify phase with single ambiguity gate** — cerbrix folds clarification into `cb-auto`; omc has `deep-interview` Ouroboros (heavyweight). steop's single gate is the right granularity.
- **Targeted Opus routing** — only consultant and architect use Opus; researcher/executor inherit; reviewer is Sonnet. omc sprays Opus across 8 agents; cerbrix uses Opus for 2. steop is more cost-aware.
- **Single Go binary, zero runtime deps** — omc requires Node + Python + native SQLite build + optional Go. cerbrix requires Go + taskfile. steop is `go install`-and-done.
- **Statusline rendered in-process** — steop's statusline is a Go subcommand. cerbrix shells out from a shell script. Perf advantage on every prompt render (see `apps/steop/cmd_statusline.go` and `apps/steop/cmd_statusline_line1.go`).
- **Linear phase discipline** — omc has 4 overlapping orchestration modes which creates choice paralysis. steop's single linear flow is legible.

## Non-Goals

- **37 skills** — omc's skill surface creates discoverability collapse. Cap steop at ~12.
- **Multi-language runtime** — omc's Node + Python REPL + Go-free is an install/debug nightmare. Stay pure Go.
- **Built-in MCP server in the plugin binary** — omc bundles notepad/project-memory/ast-grep tools. This is stele's job. Do not duplicate.
- **Project-local state silos** — cerbrix's `.cerbrix/` per repo prevents cross-project learning. Do not regress.
- **tmux team coordination** — cerbrix `cb-team` and omc `team-server` spawn tmux workers. High complexity, narrow audience.
- **SWE-bench harness** — premature. Revisit only after steop has a stable user base.
- **19 specialized agents** — each added agent is a maintenance and routing burden. Add only on demonstrated demand.
- **Keyword auto-injection beyond explicit triggers** — cerbrix matches `build me`, `autopilot:`, etc. Implicit control flow degrades predictability. Limit any injection to explicit `steop:` / `st-` triggers.

## Philosophical Differences

- **steop — workflow discipline + shared memory**: bet is that a small, opinionated linear pipeline combined with persistent cross-session memory outperforms larger surface area. Tradeoff: less flexibility for exotic workflows.
- **cerbrix — project-local isolation + keyword injection**: bet is that each project should own its agentic state, and that users want zero-friction keyword triggers over explicit slash commands. Tradeoff: no cross-project learning, implicit control flow.
- **omc — maximalism + benchmarked optimization**: bet is that breadth (19 agents, 37 skills, 10 hooks, multi-channel notifications, SWE-bench) wins, and that benchmarks justify complexity. Tradeoff: install fragility, discoverability collapse, multi-language runtime.

## Appendix: Source References

- `plugins/steop/.claude-plugin/plugin.json`, `plugins/steop/agents/`, `plugins/steop/hooks/hooks.json`, `plugins/steop/skills/`
- `apps/steop/cmd_statusline.go`, `apps/steop/cmd_statusline_line1.go`, `apps/steop/internal/hooks/pre_tool_use.go`
- `../cerbrix/` and `../oh-my-claudecode/` (external, snapshot 2026-04-11)
