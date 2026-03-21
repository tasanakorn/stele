# Changelog

Maintained with the help of AI tooling. Each entry references the git hash it covers through.

## 0.2.0 — 2026-03-21 `428b106`

### Added
- Claude Code plugin at `plugin/` for marketplace distribution with four skills (`/stele:install`, `/stele:bootstrap`, `/stele:sync`, `/stele:checkpoint`) and a read-only `stele-librarian` subagent (`4bed4f2`)
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
