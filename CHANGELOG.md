# Changelog

Maintained with the help of AI tooling. Each entry references the git hash it covers through.

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
- Architecture docs: `docs/architecture.md`, `docs/cli.md`, `docs/data-model.md`, `docs/deployment.md`, `docs/mcp-tools.md`, `docs/rest-api.md`, `docs/server.md` (`599adb2`)

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
