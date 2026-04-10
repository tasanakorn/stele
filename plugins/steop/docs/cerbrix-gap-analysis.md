> **Status: v1 foundation shipped — see [DESIGN.md](./DESIGN.md) for the implementation blueprint.**
>
> This document remains the *source of truth for the cerbrix feature catalog* and the planning ledger for what's still ahead. Items from the original "Bucket C — deliberately not recommended" list have been partially reversed: the Go runtime and session state persistence ARE implemented in v1, routed through stele-server instead of a `.cerbrix/` directory.
>
> The [Capability Matrix](#2-capability-matrix) and [Prioritized Recommendations](#4-prioritized-recommendations) tables now carry a **Status** column tracking v1/v2/deferred decisions. The [Open Design Questions](#5-open-design-questions-resolved) are resolved and preserved for the record.

---

# Steop vs Cerbrix: Gap Analysis

> Comparative analysis of features and techniques in `cerbrix` (a POC Claude Code plugin) against the current `steop` plugin. Originally produced via the `/steop:st-flow` pipeline; kept up to date as implementation progresses.

## 1. Executive Summary

Steop began as lean prompt-orchestration — 6 skills, 5 agents, zero infrastructure. Cerbrix was the infrastructure-heavy reference — 12 skills, 5 hooks, a Go CLI companion, a project-local state directory, statusline, profiles, tmux team coordination.

**v1 has shipped** (see [§7 v1 Implementation Summary](#7-v1-implementation-summary)): steop now has a Go runtime, Claude Code hooks, a `/api/v1/steop/*` REST API on stele-server, and session state backed by stele — **without** a project-local state directory. PreToolUse safety gates and PostToolUse tool-call counters are live. Ergonomics features (keyword router, session recap, Stop snapshots, HUD) are scoped for v2.

The central design principle that drove this revision holds: cerbrix uses local JSON files as its database, while steop sits inside a repo whose entire reason to exist is the stele memory server — so persistence gaps are closed through stele, not by replicating `.cerbrix/`.

## 2. Capability Matrix

Status legend: **✓ v1** shipped · **v2** planned next · **v3+** later · **—** deferred / out of scope · **n/a** already at parity

| Capability                             | Cerbrix                                                             | Steop (post-v1)                                                      | Bucket | Status | Note                                                      |
| -------------------------------------- | ------------------------------------------------------------------- | -------------------------------------------------------------------- | ------ | ------ | --------------------------------------------------------- |
| Skill count / taxonomy                 | 12 skills, levels 1/2/3, rich frontmatter                           | 6 skills, bare frontmatter                                           | D      | v2     | Frontmatter enrichment slated with keyword router         |
| Agent roster                           | planner / architect / executor / verifier / explorer               | consultant / researcher / architect / executor / reviewer            | D      | n/a    | 5-agent roster covering same phases                      |
| Pipeline orchestration model           | Sequential phases via subagents + `cerbrix autopilot` state         | Sequential phases via subagents + stele-backed session state         | D      | ✓ v1   | State now lives in stele, not disk                        |
| Phase retry / loop mechanics           | Explicit counters: stepRetry max 5, loopCount max 3                 | Atomic counters available via `/state/:id/incr` API                  | A      | v2     | API shipped in v1; skill integration comes in v2          |
| Persistent plan artifacts              | `.cerbrix/plans/plan-{slug}.md`, `[DONE]` markers                   | `/api/v1/steop/storage?scope=steop/plans&key=...` available          | C      | v2     | Storage endpoint shipped; skill integration pending       |
| Session resume / recap                 | `cb-recap` reads state + git + plans                                | Skeleton: state API ready; `st-recap` skill + SessionStart hook TBD  | A      | v2     | Stele-backed, not disk-backed                             |
| Cancel / teardown                      | `cb-cancel` clears active-mode.json                                 | `DELETE /api/v1/steop/state/:id` available                           | B      | v2     | API shipped; skill wrapper pending                        |
| Keyword-triggered skill injection      | UserPromptSubmit regex router → injects skill body                  | Hook infra ready; handler is a stub                                  | A      | v2     | Highest-priority v2 item                                  |
| PreToolUse safety gates                | Regex blocks `git push -f`, `rm -rf /`, `rm -rf ~/`, push-to-main   | **Six regex blocks live** in `plugins/steop/bin/steop hook PreToolUse` | A    | ✓ v1   | Pure-local check; no network dependency                   |
| PostToolUse error-context injection    | Non-zero exit → error context in next turn                          | Hook wiring ready; handler is a no-op stub                           | A      | v2     | Requires parsing `tool_response` shape                    |
| PostToolUse tool-call counter          | `toolCalls` counter fed into HUD                                    | **Live** — increments via `/api/v1/steop/state/:id/incr`             | B      | ✓ v1   | Tolerates server-unreachable                              |
| Stop / SessionEnd snapshots            | Snapshots to `state/last-stop.json`, `inbox/stop-{ts}.json`         | Stele storage API ready; Stop notify hook live (0.1.1)               | C      | partial ✓ v1 (notify) / v2 (snapshot) | Target scopes: `steop/sessions/<id>/{last-stop, inbox}`   |
| Companion CLI binary                   | Go binary dispatching hooks, state, HUD, team                       | **`apps/steop/` Go module** → `plugins/steop/bin/steop`              | C      | ✓ v1   | Reversed original "not recommended" decision              |
| Project-local state directory          | `.cerbrix/` with config, plans, inbox, logs, state                  | **None and never will** — stele owns all steop state                | C      | n/a    | Core design invariant                                     |
| XDG user config paths                  | `~/.config/cerbrix/`, `~/.local/share/...` (reserved)               | Reuses `~/.config/stele/config.toml` profile                         | B      | ✓ v1   | Zero new config files                                     |
| Statusline / HUD                       | `statusline.sh` + `cerbrix hud render` shows mode/phase/N/M/L/R    | `steop statusline` two-line renderer (line 1: session JSON, line 2: pipeline state) + `/steop:statusline-setup` patches `~/.claude/settings.json` | B      | ✓ v1   | No shell script, no jq; `steop` binary renders both lines |
| Skill profiles (core/extended/lab)     | Filter for `cerbrix install --profile`                              | None                                                                 | B      | —      | Premature below ~10 skills                                |
| Feature flags                          | `config.json` flags: team/hud/debug                                 | None                                                                 | B      | —      | Overkill for 6 skills                                     |
| Install / init utilities               | `cb-install` builds Go binary; `cb-init` scaffolds state            | `apps/steop/scripts/build.sh` builds local binary                    | D / C  | ✓ v1   | No init — stele-server is the single source of truth     |
| Doctor / health checks                 | `cb-doctor` + `scripts/doctor.sh` + auto-repair                     | None                                                                 | B      | —      | Low surface area                                          |
| Config management skill                | `cb-config` toggles features via `jq`                               | None                                                                 | B      | —      | Dependent on feature flags                                |
| Team / parallel tmux coordination      | `cb-team` splits panes, aggregates results                          | Prose-level "launch parallel agents"                                 | B      | —      | In-prompt parallelism covers 90%                          |
| Memory / knowledge persistence         | `.cerbrix/memory/` (reserved, unused)                               | Stele server (mature)                                                | C      | n/a    | Steop wins here, and now `steop_*` tables coexist         |
| Plugin manifest richness               | Rich (skills/hooks/mcp keys)                                        | **`hooks/hooks.json` present**; skills/agents discovered by convention | A    | ✓ v1   | Hooks registered; skill frontmatter richer in v2          |
| Design docs                            | `DESIGN.md` + `MVP-SPEC.md` + per-command docs                      | **`plugins/steop/docs/DESIGN.md` shipped** (9 sections)              | A      | ✓ v1   | This gap doc kept as planning ledger                      |
| Agent `disallowedTools` deny-list      | Explicit deny for planner/architect/explorer                        | Allowlist only                                                       | A      | v2     | Defense in depth                                          |
| Stable REST API contract               | No API — filesystem only                                            | **`/api/v1/steop/*` frozen** (storage/state/status)                  | —      | ✓ v1   | New capability, no cerbrix equivalent                     |
| Atomic counter primitives              | Read-modify-write on JSON file                                      | **`/state/:id/incr` + `/reset`** — atomic via SQL `ON CONFLICT`      | —      | ✓ v1   | Correctness win over cerbrix                              |

## 3. Gap Analysis by Category

### 3.1 Originally Bucket A — Missing and Recommended

These were the real gaps. Status after v1:

- **PreToolUse safety gates** — ✓ **v1**. Six regex blocks (`git push --force`, `git push -f`, `git push origin main/master`, `rm -rf /`, `rm -rf ~`, `rm -rf $HOME`) emit `permissionDecision: "deny"` via `${CLAUDE_PLUGIN_ROOT}/bin/steop hook PreToolUse`. Command-chaining (`&&`, `;`) is handled.
- **Plugin manifest enrichment** — ✓ **v1**. `plugins/steop/hooks/hooks.json` registers the hooks. Skills/agents still discovered by convention (keys are optional).
- **DESIGN.md / philosophy doc** — ✓ **v1**. `plugins/steop/docs/DESIGN.md` covers purpose, persistence model, hook taxonomy, versioning, phase roadmap, smoke tests.
- **UserPromptSubmit keyword router** — **v2**. Hook infrastructure is ready (`internal/hooks/output.go InjectUserPromptContext`). Handler is a stub. Triggers TBD.
- **Session resume (`st-recap`)** — **v2**. Storage + state APIs are ready. Needs a SessionStart hook that reads the last snapshot + an `st-recap` skill that calls the CLI.
- **Explicit retry counters** — **v2**. `/api/v1/steop/state/:id/incr` exists with atomic SQL semantics. Skills must now refer to the counters (e.g. `steop state incr $SESSION_ID loop_count`) instead of prose `max 3`.
- **PostToolUse error-context injection** — **v2**. Hook wiring is present; handler currently only bumps the counter. Parsing `tool_response` for error strings and returning them as `additionalContext` is the next increment.
- **Skill frontmatter richness** (`level`, `triggers`, `pipeline`) — **v2**. Metadata-only; gated on the keyword router actually using it.
- **Agent `disallowedTools`** — **v2**. Pure config edit across 5 agent files; bundled with the v2 ergonomics pass.

### 3.2 Originally Bucket B — Optional / depends on growth

- **PostToolUse tool-call counter** — ✓ **v1**. Ships as part of the foundation because the state API proves out atomicity.
- **XDG user config paths** — ✓ **v1** (via reuse of `~/.config/stele/config.toml`). No new config surface.
- **Statusline / HUD** — ✓ **v1**. `/api/v1/steop/status/:id` projects the statusline shape and never 404s. Both lines are now rendered entirely by `steop statusline`: line 1 is built from Claude Code's stdin JSON (model / project / git branch / context bar / rate limits or cost), line 2 is the steop pipeline state fetched from stele-server. `/steop:statusline-setup` patches `~/.claude/settings.json` `statusLine` to point directly at `steop statusline` — no shell script, no `jq` dependency. There is no plugin-level `statusLine` field in `plugin.json` — Claude Code only exposes `statusLine` via user settings, not plugin manifests.
- **cb-cancel** — **v2** (trivial wrapper over `DELETE /api/v1/steop/state/:id`).
- **Feature flags**, **skill profiles**, **cb-doctor**, **cb-config**, **tmux team mode** — **deferred**. Still premature.

### 3.3 Originally Bucket C — Reversed in v1

These were "deliberately not recommended" in the original analysis because cerbrix implemented them via filesystem-as-database. The v1 decision reversed that call by implementing them over stele-server instead of disk:

- **Companion CLI binary** — ✓ **v1**. Go module at `apps/steop/`, not Rust. Small, stdlib-only, fast cold start. Single binary shipped in `plugins/steop/bin/steop`.
- **Persistent plan artifacts** — API ✓ **v1**, skill wiring **v2**. `PUT /api/v1/steop/storage?scope=steop/plans&key=<slug>` with opaque JSON content.
- **Stop / SessionEnd snapshots** — API ✓ **v1**, Stop notify hook ✓ **v1 (0.1.1)**. The Stop hook fires on session end and posts a native OS notification via `POST /api/v1/steop/notify` (non-blocking; errors are swallowed). Full snapshot-to-storage (writing `steop/sessions/<id>/last-stop` and `steop/sessions/<id>/inbox/<ts>`) remains **v2**.
- **Memory directory** — reused stele's existing `memories` + `entities` tables. No new table.
- **Project-local `.steop/` state** — **never**. Core design invariant: all steop state lives in the stele SQLite file.
- **cb-init scaffolding** — not needed. Stele-server is the single source of truth; no per-project init step exists.

### 3.4 Bucket D — Parity (unchanged)

- **Pipeline phase structure** — clarify → [research] → plan → execute → verify with agent handoffs.
- **Agent roster** — consultant / researcher / architect / executor / reviewer.
- **Complexity gating** — consultant's complexity assessment.
- **Skill installation** — Claude Code plugin marketplace.

## 4. Prioritized Recommendations

| #   | Recommendation                                                                | Effort | Impact | Status  | Notes                                                                    |
| --- | ----------------------------------------------------------------------------- | ------ | ------ | ------- | ------------------------------------------------------------------------ |
| 1   | Add PreToolUse hook with git-force-push + rm-rf regex blocks                  | Low    | High   | ✓ v1    | Six patterns live; tested; chain-aware (`&&`, `;`).                      |
| 2   | Add UserPromptSubmit keyword router (autopilot/plan/cancel triggers)          | Low    | High   | v2      | Hook infra + output helper ready; handler is a stub.                     |
| 3   | Add `disallowedTools` to consultant / architect / reviewer agents             | Low    | Medium | v2      | Pure config edit.                                                        |
| 4   | Introduce `level` / `triggers` / `pipeline` frontmatter on all 6 skills       | Low    | Medium | v2      | Metadata bundled with #2.                                                |
| 5   | Add `st-recap` skill that pulls last session state from stele                 | Medium | High   | v2      | Needs SessionStart hook + storage GET.                                   |
| 6   | Promote execute/validate retry loop to explicit counter (stele-backed)        | Medium | Medium | v2      | Atomic `/state/:id/incr` shipped; skill integration pending.             |
| 7   | Write `plugins/steop/docs/DESIGN.md` mirroring cerbrix's design doc           | Medium | Medium | ✓ v1    | 9 sections; kept current with implementation.                            |
| 8   | Enrich `plugins/steop/.claude-plugin/plugin.json` with skills/agents/hooks    | Low    | Low    | ✓ v1    | `hooks/hooks.json` registered; keywords added.                           |
| 9   | Stand up `/api/v1/steop/*` storage/state/status REST API on stele-server      | High   | High   | ✓ v1    | New capability added during v1 planning; foundation for everything.      |
| 10  | Ship a Go runtime (`apps/steop/`) building to `plugins/steop/bin/steop`       | High   | High   | ✓ v1    | Originally "not recommended"; reversed because stele replaces `.cerbrix/`. |
| 11  | Stop / SessionEnd snapshot handler writing to `steop/sessions/<id>` storage   | Medium | Medium | v2      | API ready; only the hook handler and target scope convention are left.   |
| 12  | `steop statusline` two-line renderer + settings.json setup skill              | Medium | Low    | ✓ v1    | Renders both lines from the Go binary; wired via `/steop:statusline-setup` (patches `~/.claude/settings.json` directly — no shell script). |
| 13  | Release-workflow binary distribution (darwin arm64 + amd64)                   | Medium | Low    | v3      | v1 uses local build; `/steop:install` skill triggers `build.sh`.         |
| 14  | Optional MCP-tool mirrors of `/api/v1/steop/*` endpoints                      | Medium | Low    | v3      | Lets Claude read its own status without going through the Go binary.     |

**Still deferred**: feature flags, skill profiles, cb-doctor, cb-config, tmux team mode. All remain premature for steop's current surface area.

## 5. Open Design Questions (Resolved)

These were the open questions in the original analysis. Resolutions captured during the v1 planning/execution phase:

1. **Persistence backend.** → **Stele.** New `/api/v1/steop/*` REST API with three tables (`steop_storage`, `steop_state`, `steop_counters`). No `.steop/` directory, ever. This is a core design invariant.
2. **Hook scope.** → **Steop-bundled**, via `plugins/steop/hooks/hooks.json`. Safety hooks ship with the workflow plugin; a separate safety plugin is not justified at current scope.
3. **Keyword triggers.** → **Deferred to v2**. Exact patterns will be finalized alongside the UserPromptSubmit handler. Working list: `^autopilot[:\s]`, `build me`, `^plan[:\s]`, `cancel`.
4. **Retry hardening scope.** → **Counter-based.** `/api/v1/steop/state/:id/incr` provides atomic semantics via SQL `ON CONFLICT ... DO UPDATE`. Skills in v2 will call `steop state incr` in the execute/validate loop instead of relying on prose `max 3`.

Questions introduced and resolved during v1:

5. **Rust crates vs. Go module for the companion binary.** → **Go**, per the user's stated preference (fast startup, self-contained, lightweight dev loop). Rust stays on the server side; Go is additive.
6. **Where does the Go module live?** → `apps/steop/` as a peer to `apps/stele/`. Not a Cargo workspace member.
7. **Shared DB pool or a second SQLite file?** → Shared. The global tokio mutex already serializes all access; splitting files adds operational complexity without removing contention.
8. **Generic KV or typed endpoints for the API surface?** → Hybrid. **Storage** is a generic blob KV (plans/inboxes/snapshots are opaque); **state** and **status** are typed because atomic counter operations can't be expressed as generic PUTs safely.
9. **Ship binaries in git or via release workflow?** → Neither in v1. The binary is built locally by `apps/steop/scripts/build.sh` into `plugins/steop/bin/steop` (gitignored). Release-workflow distribution is a v3 concern.

## 6. Closing Note

Steop's minimalism was the starting point, not the finish line. v1 adds infrastructure deliberately — hooks, a Go runtime, a REST API contract — because each piece was paying for itself in safety or ergonomics. What stayed out is still out: no project-local state directory, no feature flag DSL, no tmux team coordination, no rewrite of stele in Go. The central principle holds: cerbrix's persistence features route through stele scopes, not a new disk directory.

## 7. v1 Implementation Summary

Shipped in the v1 foundation slice:

**Stele server (Rust, `apps/stele/crates/stele-server/`)**
- New tables in `db.rs`: `steop_storage` (generic blob KV), `steop_state` (per-session JSON data), `steop_counters` (atomic counters with FK cascade).
- New module `steop_api.rs` mounted at `/api/v1/steop/*` — handlers for storage PUT/GET/DELETE/list, state GET/PUT/DELETE, counter incr/reset, HUD status projection (never 404s).
- Delete handlers correctly propagate the row-exists boolean (`deleted: true` only when a row was removed).

**Go runtime (`apps/steop/`, new module)**
- Single binary with subcommands `hook`, `state`, `storage`, `hud` (stub), `version`.
- Config loader reads `~/.config/stele/config.toml` (macOS App Support fallback) with env-var overrides and graceful missing-config defaults.
- HTTP client with `X-Stele-Key` header, 10s timeout, `ErrNotFound` on 404.
- PreToolUse handler: six dangerous-pattern regexes, chain-aware, pure-local (no network).
- PostToolUse handler: atomic `tool_calls` increment, silent on server unreachable.
- Top-level `recover()` — no panic can kill a Claude Code hook.
- Stdlib + `github.com/BurntSushi/toml`; no other dependencies.

**Plugin wiring (`plugins/steop/`)**
- `hooks/hooks.json` registers PreToolUse (`Bash` matcher) and PostToolUse (`*` matcher) invoking `${CLAUDE_PLUGIN_ROOT}/bin/steop hook <Event>` with 5s timeout.
- Enriched `.claude-plugin/plugin.json` (keywords added, description refined).
- `docs/DESIGN.md` (new, 9 sections): purpose, architecture, persistence model with endpoint table, hook taxonomy, phase roadmap, versioning, smoke tests, known limitations.
- Build script `apps/steop/scripts/build.sh` produces `plugins/steop/bin/steop` (~6.2 MB, arm64, gitignored).

**CI (`.github/workflows/ci.yml`)**
- New `check-go` job: `go vet`, `go build`, `go test` on Go 1.22.
- Extended `validate-steop-plugin`: required-files check, `hooks.json` JSON validation, `version.go` ↔ `plugin.json` version sync.

**API contract freeze.** `/api/v1/steop/*` is stable. Additive changes allowed; breaking changes require a `/api/v2/*` prefix.

Next up (v2): UserPromptSubmit keyword router, `st-recap` skill + SessionStart handler, Stop/SessionEnd snapshot handler, agent `disallowedTools` deny-list, skill frontmatter enrichment, retry-counter integration in execute/validate, HUD renderer.
