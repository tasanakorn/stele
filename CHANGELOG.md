# Changelog

Maintained with the help of AI tooling. Each entry references the git hash it covers through.

## 0.5.1

### Added

- **Full hook surface for steop** — all 11 Claude Code hook events now wired: `SessionStart`, `UserPromptSubmit`, `PreToolUse`, `PermissionRequest`, `PostToolUse`, `PostToolUseFailure`, `SubagentStart`, `SubagentStop`, `PreCompact`, `Stop`, `SessionEnd`. Most are log-only in v1; a few carry meaningful state behavior.
- **Keyword injection in `UserPromptSubmit`** — prompts matching explicit `/steop:st-<phase>` or `<phase>:` triggers inject the corresponding SKILL.md body via `additionalContext`. Triggers: `st-flow`, `st-clarify`, `st-research`, `st-plan`, `st-execute`, `st-validate`. No implicit phrase matching.
- **stele-server log facility** — `POST /api/v1/steop/log` + `GET /api/v1/steop/log` append-only structured event store. New `steop_logs` SQLite table. Filters by `host`, `session_id`, `project_dir`. Default limit 200.
- **stele-server inbox facility** — `POST /api/v1/steop/inbox` + `GET /api/v1/steop/inbox` FIFO session-summary queue. New `steop_inbox` SQLite table. Stop and SessionEnd hooks persist session summaries here.
- **Composite session identity** — `host` + `project_dir` as metadata across all steop tables. steop Go client and stele CLI + MCP proxy both send `X-Steop-Host` and `X-Steop-Project-Dir` headers on every request. Hostname auto-detected via `gethostname` on first config load, persisted to `~/.config/stele/config.toml`.
- **`Profile.host` field** in both Go and Rust config schemas. Backfilled once on load if empty.
- **Persistent-mode flag** in Stop handler (read-only in v1 — flag is stored but does not trigger session resume; full block-and-resume loop is deferred).
- **`PostToolUse` state updates** — writes `last_tool` and `last_tool_at` to session state in addition to the existing counter increment.

### Changed

- **stele workspace version** bumped to `0.5.0` (additive REST contract expansion, idempotent schema migration for `steop_state` composite columns).
- **steop plugin + binary version** bumped to `0.5.0`.
- **steop dispatcher** (`cmd_hook.go`) refactored into a uniform `newClient()` closure that applies `WithRequestContext("", in.Cwd)` to every client-bearing handler.
- **Go client `do()`** now injects `X-Steop-Host` and `X-Steop-Project-Dir` headers uniformly. New `WithRequestContext(host, projectDir)` builder and `fastClone()` (500ms timeout) for fire-and-forget log/inbox posts.
- **Rust CLI `SteleClient`** refactored through a shared `apply_headers()` helper. Header sanitization strips non-ASCII-graphic characters.

### Migration

- `steop_state` table gains `host TEXT NOT NULL DEFAULT ''` and `project_dir TEXT NOT NULL DEFAULT ''` columns via idempotent `ALTER TABLE` on server startup (guarded by `pragma_table_info`). Existing rows are unaffected.

## 0.4.12

### Added

- Statusline is now rendered entirely by the `steop` Go binary. The shell script is gone.
- Cross-platform (macOS, Linux, Windows), no `jq`/`bash` deps, ~5× faster (p95 ~20 ms vs ~100 ms).
- `--line2-only` flag added to `steop statusline` for transitional back-compat.

### Removed

- `plugins/steop/scripts/statusline.sh` removed.

### Changed

- `/steop:statusline-setup` now points `~/.claude/settings.json` directly at `steop statusline`; no file copy.

## 2026-04-10 22:58:20 `8071d2a`

### Added
- **steop v0.4 Go runtime** — single `steop` binary with hook (`PreToolUse` safety regexes, `PostToolUse` counter, non-blocking `Stop` desktop notify), `state`, `storage`, `statusline`, `monitor`, `inspect`, and `version` subcommands (`105763d`, `d088195`, `a9dd8cf`)
- **steop REST surface** at `/api/v1/steop/*` — blob storage KV, per-session state with atomic counters, HUD/status projection, notify endpoint, sessions list/inspect, storage scopes (`105763d`, `d088195`)
- Desktop notifications on the `Stop` hook via `notify-rust`, pinned to the `com.tasanakorn.stele.app` bundle identifier so macOS 13+ skips the "Choose Application" modal (`105763d`)
- `/steop:install` skill — installs the `steop` binary to `~/.local/bin` (switched from `git clone + go build` to `go install github.com/tasanakorn/stele/apps/steop@main`, dropping the Git prerequisite) (`bc2b9b9`, `a9dd8cf`, `de0b0c5`)
- `/steop:statusline-setup` skill + `plugins/steop/scripts/statusline.sh` template — installs a native two-line Claude Code statusline (line 1: model / project / git branch / context bar / rate limits or cost; line 2: `steop statusline` pipeline state) (`a9dd8cf`, `de0b0c5`)
- Rate-limit segment in the statusline template showing `used%/elapsed%` per 5h and 7d window — turns yellow when quota burns faster than the clock, green otherwise; falls back to plain `used%` when `resets_at` is missing (`de0b0c5`)
- VCS build info in `steop version` (commit sha / time / dirty flag) via `runtime/debug.ReadBuildInfo()`, discoverable even with a `const Version` string (`a9dd8cf`, `de0b0c5`)
- Local MCP tool handling in `stele mcp` proxy: `list_profiles` tool answered without round-tripping to the server, optional per-call `profile` parameter on every tool for ad-hoc server routing, per-server session tracking with correct auth-key cleanup (`60d7a8b`)
- `stele config set` / `stele config remove` subcommands and `apps/stele/scripts/sync-profiles.py` bidirectional profile sync helper (`60d7a8b`)
- CI `check-go` job (vet / build / test on Go 1.22); `validate-steop-plugin` extended with required-files check, `hooks.json` JSON validation, and `version.go` ↔ `plugin.json` sync check (`105763d`)

### Changed
- Bumped Cargo workspace, stele plugin, and marketplace entries to **v0.4.0**; steop plugin + Go module bumped to **v0.4.8** over this range (`105763d` → `8071d2a`)
- Statusline context bar now uses absolute-token thresholds: yellow at ≥160K tokens (80% × 200K — the efficiency soft limit that applies in 1M-context mode too), red at ≥80% of `context_window_size`. Behavior in 200K sessions is unchanged (`8071d2a`)
- Statusline template palette tuned for dark-background legibility: bright (9x) colors only, no `dim`, exactly one bold per line (`de0b0c5`)
- Linux `/stele:install` flow provisions a systemd user service instead of forking a background process (`413d703`)
- macOS bundle identifier switched to `com.tasanakorn.stele.app` in `Info.plist` to match the notify-rust `set_application` pin (`105763d`)
- README plugin table now carries a "Prerequisites" column (`f192c3d`)
- `HudStatus` Go client type renamed to `Status`; the `/api/v1/steop/status/:id` REST contract is unchanged (`a9dd8cf`)

### Fixed
- `steop_api::status_get` returns 404 for unknown sessions instead of fabricating an idle stub (`d088195`)

### Removed
- Short-lived `steop hud` interactive TUI and its `/steop:hud` / `/steop:hud-install` skills — replaced by the one-shot `steop statusline` renderer wired into Claude Code's native `statusLine` setting (`a9dd8cf`)
- Dead `-X main.Version=...` ldflag from `apps/steop/scripts/build.sh` — was a no-op because `Version` is a const, not a var (`a9dd8cf`, `de0b0c5`)

## 2026-04-09 22:32:41 `a2dd409`

### Added
- Git commit hash in `--version` output for both CLI and server via `build.rs` (`a2dd409`)
- SemVer versioning guide in CLAUDE.md (`a2dd409`)

### Changed
- Bumped to v0.3.2 — Cargo workspace and plugin version now in sync (`a2dd409`)
- `/stele:install` skill rewritten for end-to-end setup: clone source, build from local tree, auto-cleanup `/tmp/stele-build` on completion (`122de31`, `9d92d82`, `a2dd409`)
- macOS install flow simplified — always installs Stele.app desktop mode, removed headless option (`a2dd409`)
- Stelite plugin converted to exact copy of steop with subagents (`a3b54a8`)
- `StreamableHttpServerConfig` init switched to `Default` + field assignment for rmcp forward compatibility (`a2dd409`)

## 2026-04-09 18:21:46 `ae04b95`

### Added
- Bundled `.mcp.json` in the stele plugin — MCP server is auto-registered on plugin install, no manual setup needed (`ae04b95`)

### Changed
- `/stele:install` skill repurposed from MCP setup to CLI connection profile configuration via `stele config` (`ae04b95`)
- Updated all docs (README, CLAUDE.md, deployment.md, plugin README, bootstrap skill) to reflect auto-registered MCP and profile-based install flow (`ae04b95`)

### Removed
- Direct HTTP transport fallback from install skill (`fd13fcf`)

## 2026-04-09 16:31:59 `fd13fcf`

### Added
- MCP stdio proxy (`stele mcp`) — stdio-to-Streamable-HTTP bridge as the primary transport for Claude Code, with session tracking via `mcp-session-id` (`58ce579`)
- CLI client (`stele-cli`) with multi-profile config, memory/graph commands, and MCP proxy (`599adb2`)
- Cargo workspace split: `stele-common` (shared types), `stele-server`, `stele-cli` as separate crates (`599adb2`)
- End-to-end integration test suite — 68 tests covering REST API and MCP transport (`797a687`)
- Architecture docs: `docs/stele/architecture.md`, `docs/stele/cli.md`, `docs/stele/data-model.md`, `docs/stele/deployment.md`, `docs/stele/mcp-tools.md`, `docs/stele/rest-api.md`, `docs/stele/server.md` (`599adb2`)

### Changed
- README restructured around marketplace installation and plugin-first developer experience (`a0905d0`)
- Install skill no longer offers direct HTTP transport fallback — stdio via `stele mcp` is the sole documented transport (`fd13fcf`)

### Fixed
- MCP proxy Accept header now includes both `application/json` and `text/event-stream` content types (`3c63389`)

## 2026-04-09 09:05:32 `3f63426`

### Added
- Stelite plugin at `plugins/stelite/` — lightweight, skills-only clone of steop that executes all phases inline without subagents (`4a63ede`)

### Changed
- README updated to position stelite as an unmaintained snapshot of steop, moved below steop and stele sections (`c16a3d0`, `3f63426`)

## 2026-04-08 18:04:17 `ddfe39d`

### Added
- Steop agentic workflow plugin at `plugins/steop/` with five-phase pipeline: clarify, research, plan, execute, validate — includes `/steop:st-flow` for end-to-end runs and individual phase skills (`05938e1`)
- Monorepo structure with `apps/` and `plugins/` top-level directories, separating the Rust server (`apps/stele/`) from Claude Code plugins (`plugins/stele/`, `plugins/steop/`) (`05938e1`)

### Changed
- Re-added `.mcp.json` to the stele plugin — MCP connection is now auto-registered on plugin install; `/stele:install` skill repurposed for CLI connection profile configuration (`1b2283c`)
- Bootstrap CLAUDE.md template slimmed down, deferring procedural details to plugin skills (`c4011c5`)
- Renamed steop Explore phase to Research (`st-explore` → `st-research`) and redesigned `st-flow` as an auto-continuing pipeline that only pauses on genuine ambiguity (`d095155`)
- README restructured around marketplace installation and plugin promotion (`c061e9a`)

### Fixed
- CI workflow updated to reference `st-research` instead of removed `st-explore` skill (`ddfe39d`)

## 0.2.0 — 2026-03-21 `428b106`

### Added
- Claude Code plugin at `plugins/stele/` for marketplace distribution with four skills (`/stele:install`, `/stele:bootstrap`, `/stele:sync`, `/stele:checkpoint`) and a read-only `stele-librarian` subagent (`4bed4f2`)
- `.claude-plugin/marketplace.json` for Claude Code marketplace discovery (`4bed4f2`)
- GitHub Actions CI workflow: rustfmt, clippy (headless), cargo check (desktop/macOS), plugin structure validation with version sync check (`4bed4f2`)
- GitHub Actions release workflow: macOS binaries (aarch64 + x86_64, desktop + headless), Docker image to ghcr.io, GitHub Release with sha256 checksums (`4bed4f2`)

### Changed
- README restructured with quick start flow (start server → install plugin → bootstrap), detailed macOS .app install options, and marketplace setup instructions (`428b106`)

### Deprecated
- `bootstrap_project` MCP tool — use the plugin's `/stele:bootstrap` skill instead. The tool remains functional for backward compatibility (`4bed4f2`)

## 2026-03-20 08:42:33 `3d94ac8`

### Added
- Linux systemd service with install/uninstall scripts, hardened unit file, dedicated `stele` user, and environment config at `/etc/default/stele` (`3d94ac8`)

## 2026-03-20 07:48:13 `7a809aa`

### Added
- Multi-scope retrieval for read/search tools — `recall_memories`, `search_nodes`, `read_graph`, `open_nodes`, and `list_tags` now accept scope as a string or array of strings for cross-scope queries (`17e3053`)
- Bind address settings dialog with live server rebind — new "Settings" menu item in tray opens an egui dialog to change the bind IP, persisted in `config.toml`, with the server rebinding without restart (`443d21d`)
- macOS `.app` bundle build scripts using only built-in tools (`sips`, `iconutil`, `hdiutil`) — `scripts/build-macos.sh` assembles `Stele.app`, `scripts/build-dmg.sh` creates a distributable DMG (`7a809aa`)

## 2026-03-16 23:00:51 `a033c96`

### Added
- Shared memory server for Claude Code with MCP interface over Streamable HTTP and SQLite storage (`9904b65`)
- Flat memory tools: `store_memory`, `recall_memories`, `get_memory`, `update_memory`, `forget_memory`, `list_scopes`, `list_tags` (`9904b65`)
- macOS/Windows/Linux menu bar app as default build mode with tray icon, "Open Dashboard", and "Quit" menu items (`ca28505`)
- REST API at `/api/v1` for memories, scopes, tags, and stats with CORS support (`ca28505`)
- Desktop-friendly defaults: DB relocated to `~/Library/Application Support/Stele/` on macOS (`ca28505`)
- Knowledge graph with entities, observations, and relations stored in SQLite with FTS5 search (`84863b4`)
- Knowledge graph MCP tools: `create_entities`, `create_relations`, `add_observations`, `delete_entities`, `delete_observations`, `delete_relations`, `read_graph`, `search_nodes`, `open_nodes` (`84863b4`)
- Knowledge graph REST API endpoints under `/api/v1/graph` (`84863b4`)
- `bootstrap_project` MCP tool for generating operational CLAUDE.md snippets (`84863b4`)

### Fixed
- Lenient deserialization for array parameters that arrive as JSON-encoded strings instead of actual arrays (`a033c96`)
