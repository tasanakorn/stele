# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What is Stele

Stele is a shared memory server for Claude Code. It exposes an MCP (Model Context Protocol) interface so multiple Claude Code instances across different machines can store and retrieve shared knowledge. Two Rust binaries — `stele-server` (full server with Streamable HTTP) and `stele` (CLI client + MCP stdio-to-HTTP proxy) — SQLite storage, no external dependencies. Claude Code connects via `stele mcp` (stdio transport, recommended) or directly over Streamable HTTP.

## Repository Layout

Monorepo with apps and plugins at the top level:

```
stele/
├── apps/stele/                    # Cargo workspace root
│   ├── Cargo.toml                 # [workspace] manifest
│   ├── crates/
│   │   ├── stele-common/          # Shared types library
│   │   │   └── src/ (models.rs, query.rs)
│   │   ├── stele-server/          # Server binary (MCP + REST + tray)
│   │   │   └── src/ (main.rs, server.rs, api.rs, db.rs, ...)
│   │   └── stele-cli/             # CLI binary (client + MCP proxy)
│   │       └── src/ (main.rs, client.rs, config.rs, commands/)
│   ├── assets/
│   ├── macos/
│   ├── scripts/
│   ├── systemd/
│   └── Dockerfile
├── plugins/
│   ├── stele/                     # Claude Code plugin (shared memory)
│   └── steop/                     # Claude Code plugin (agentic workflow)
├── .claude-plugin/                # Marketplace definition
├── CLAUDE.md
└── README.md
```

## Build & Run

All build commands run from the `apps/stele/` directory:

```bash
cd apps/stele

# Server — Desktop / Menu Bar (default on macOS)
cargo build -p stele-server        # release build (default profile, see .cargo/config.toml)
cargo run -p stele-server          # menu bar app, DB at ~/Library/Application Support/Stele/

# Server — Headless daemon (Linux/Docker)
cargo build -p stele-server --features headless --no-default-features
cargo run -p stele-server --features headless --no-default-features

# CLI client
cargo build -p stele-cli           # builds 'stele' binary
cargo run -p stele-cli -- recall "search query" --scope myproject

# Server CLI flags
cargo run -p stele-server -- --bind 0.0.0.0:3100 --db /path/to/stele.db --mcp-path /mcp
```

Server CLI flags have env var equivalents: `STELE_BIND`, `STELE_DB`, `STELE_MCP_PATH`.

Build profiles are configured in the workspace `Cargo.toml` to minimize disk usage (`incremental = false`, `codegen-units = 1`, `opt-level = "s"`).

There are no tests yet. No linter or formatter is configured beyond standard `cargo clippy` / `cargo fmt`.

## Architecture

The server is a single async process: axum serves HTTP, rmcp handles MCP protocol framing, SQLite stores everything. The CLI is a sync binary using ureq to talk to the server's REST API.

### Workspace Crates

- **`stele-common`** — Shared types library. Contains `models.rs` (Memory, Entity, Graph, etc.) and `query.rs` (SearchParams).
- **`stele-server`** — Server binary. All MCP tools, REST API, SQLite, and optional desktop tray.
- **`stele-cli`** — CLI binary (named `stele`). HTTP client, multi-profile config, CLI commands, MCP stdio proxy.

All server source files are under `apps/stele/crates/stele-server/src/`.

- **`main.rs`** — Dual entry point. Desktop mode (`#[cfg(feature = "desktop")]`) runs the tray app on the main thread and the server on a background thread. Headless mode (`#[cfg(not(feature = "desktop"))]`) uses `#[tokio::main]`. Shared `run_server()` function handles axum/rmcp setup. Graceful shutdown via `CancellationToken`.
- **`tray.rs`** — macOS menu bar module (`#[cfg(feature = "desktop")]`). `TrayApp` creates a tray icon with status label, "Open Dashboard", and "Quit Stele" menu items. Uses `tray-icon` + `muda` crates.
- **`server.rs`** — `SteleServer` implements rmcp's `ServerHandler`. Tools are defined with rmcp's `#[tool_router]` / `#[tool_handler]` macros. Each tool method locks the DB mutex, calls into `db.rs`, and returns a JSON string. Tool parameter structs must derive `schemars::JsonSchema` (v1, not v0.8 — rmcp requires schemars v1).
- **`db.rs`** — SQLite schema init (tables + FTS5 + triggers), all CRUD functions. `DbPool` is `Arc<Mutex<Connection>>` (tokio mutex). SQL is built dynamically in `search_memories` using helper functions that append scope/tag filter clauses with positional parameter tracking (`?N` style).
- **`api.rs`** — REST API router mounted at `/api`. Axum handlers with JSON request/response, CORS via `tower-http`. Reuses `db.rs` functions directly.
- **`serde_helpers.rs`** — Lenient deserialization helpers. `string_or_vec`/`string_or_vec_opt` handle JSON-encoded arrays in strings. `string_or_string_vec`/`string_or_string_vec_opt` handle bare strings or arrays of strings (used for multi-scope parameters).
- **`config.rs`** — Clap derive struct with env var fallbacks. Desktop feature adds `with_desktop_defaults()` to relocate DB to `~/Library/Application Support/Stele/`.

Shared types in `apps/stele/crates/stele-common/src/`:

- **`models.rs`** — Domain types: `Memory`, `SearchResult`, `MemoryType` enum, `ScopeInfo`, `TagInfo`, `Stats`, plus knowledge graph types: `Entity`, `Observation`, `Relation`, `Graph`, `EntitySearchResult`.
- **`query.rs`** — `SearchParams` struct used to pass search criteria from server to db layer. `scope` is `Option<Vec<String>>` to support multi-scope queries.

CLI source in `apps/stele/crates/stele-cli/src/`:

- **`main.rs`** — Clap-based CLI with subcommands for memory CRUD, graph operations, and MCP proxy.
- **`config.rs`** — Multi-profile config file (`~/.config/stele/config.toml`). Named connection profiles with server URL and auth key.
- **`client.rs`** — `SteleClient` wrapping ureq HTTP agent. All methods map 1:1 to REST API endpoints. Auth via `X-Stele-Key` header.
- **`mcp_proxy.rs`** — MCP stdio-to-Streamable-HTTP proxy. Reads JSON-RPC from stdin, POSTs to server's `/mcp`, parses SSE responses, writes to stdout. Tracks `mcp-session-id` for session continuity.
- **`commands/`** — Command handlers split by domain: `memory.rs`, `info.rs`, `graph.rs`, `config_cmd.rs`.

## Data Model

Two-dimensional organization:

1. **Scope** (one per memory, hierarchical) — queried via prefix match: `scope = ?1 OR scope LIKE ?1||'/%'`. Example: querying `team-a` matches `team-a`, `team-a/frontend`, `team-a/backend`. Read/search tools accept multiple scopes (string or array) for cross-scope queries; write tools remain single-scope.
2. **Tags** (many per memory, flat labels) — stored in `memory_tags` join table. Filtered as union (any tag matches) by default, or intersection (all tags must match) with `match_all_tags`.

Full-text search uses SQLite FTS5 on title + content, kept in sync via INSERT/UPDATE/DELETE triggers. The FTS table uses content-sync mode (`content='memories'`).

### Knowledge Graph

Structured relationships stored in three tables:

1. **Entities** (`entities` table) — nodes with `name`, `entity_type`, `scope`. Names are unique within a scope (`UNIQUE(name, scope)`).
2. **Observations** (`observations` table) — atomic facts attached to entities. Stored in a join table with FK to entities (CASCADE delete).
3. **Relations** (`relations` table) — directed edges between entities with `relation_type`. Unique constraint on `(from_entity, to_entity, relation_type)`.

Two FTS5 tables enable `search_nodes` to match by entity name/type (`entities_fts`) OR observation content (`observations_fts`).

### MCP Tools

**Flat memory tools (7):** `store_memory`, `recall_memories`, `get_memory`, `update_memory`, `forget_memory`, `list_scopes`, `list_tags`

**Knowledge graph tools (9):**

| Tool                  | Description                                                              |
| --------------------- | ------------------------------------------------------------------------ |
| `create_entities`     | Create nodes (idempotent — existing entities get observations appended)  |
| `create_relations`    | Create directed edges (idempotent)                                       |
| `add_observations`    | Append atomic facts to an entity                                         |
| `delete_entities`     | Delete nodes (cascades observations + relations)                         |
| `delete_observations` | Remove specific facts by exact content match                             |
| `delete_relations`    | Remove specific edges                                                    |
| `read_graph`          | Full graph dump for one or more scopes (multi-scope)                     |
| `search_nodes`        | FTS across entity names + observations (multi-scope)                     |
| `open_nodes`          | Fetch entities + their direct neighbor relations (multi-scope)           |

**Bootstrap tool (1, deprecated):** `bootstrap_project` — generates a CLAUDE.md snippet teaching Claude Code how to use both flat memory and knowledge graph for a project. Deprecated in favor of the plugin's `/stele:bootstrap` skill.

## REST API

JSON API mounted at `/api/v1` alongside the MCP endpoint. CORS enabled for browser access.

| Method | Path                  | Description             |
| ------ | --------------------- | ----------------------- |
| GET    | /api/v1/memories      | Search/list memories    |
| POST   | /api/v1/memories      | Create a memory         |
| GET    | /api/v1/memories/:id  | Get single memory       |
| PUT    | /api/v1/memories/:id  | Update a memory         |
| DELETE | /api/v1/memories/:id  | Delete a memory         |
| GET    | /api/v1/scopes        | List scopes with counts |
| GET    | /api/v1/tags          | List tags with counts   |
| GET    | /api/v1/stats         | Dashboard summary stats |

### Knowledge Graph

| Method | Path                                      | Description            |
| ------ | ----------------------------------------- | ---------------------- |
| GET    | /api/v1/graph?scope=                      | Read full graph        |
| POST   | /api/v1/graph/entities                    | Create entities        |
| GET    | /api/v1/graph/entities?q=&scope=          | Search entities        |
| GET    | /api/v1/graph/entities/:name?scope=       | Get entity by name     |
| DELETE | /api/v1/graph/entities/:name?scope=       | Delete entity          |
| POST   | /api/v1/graph/entities/:name/observations | Add observations       |
| DELETE | /api/v1/graph/entities/:name/observations | Delete observations    |
| POST   | /api/v1/graph/relations                   | Create relations       |
| DELETE | /api/v1/graph/relations                   | Delete relations       |
| GET    | /api/v1/graph/open?names=a,b&scope=       | Open specific nodes    |

### Steop (workflow pipeline)

RPC-style surface mounted under `/api/v1/steop/*`. Every method is `POST /api/v1/steop/<method>` with a JSON body — no path params, no query params, no header identity. Served by `apps/stele/crates/stele-server/src/steop_api.rs`. Used by the `steop` Go binary and the stele CLI.

**Identity.** Every call carries composite SSH/SCP-style identifiers in the body: `host` + `project_dir` (project-level) or `host` + `project_dir` + `session_id` (session-level). Session IDs are globally unique Claude Code UUIDs; read methods accept `{session_id}` as a short form. Write methods require the full triple. The server does not validate identifiers — clients take care of completeness.

| Method                 | Body                                                                                                    | Description                                           |
| ---------------------- | ------------------------------------------------------------------------------------------------------- | ----------------------------------------------------- |
| `steop.session.start`  | `host, project_dir, session_id, data?`                                                                  | Create/reactivate session (idempotent)                |
| `steop.session.stop`   | `host, project_dir, session_id`                                                                         | Mark session stopped                                  |
| `steop.session.touch`  | `host, project_dir, session_id`                                                                         | Refresh `last_active_at`                              |
| `steop.session.get`    | `session_id` or full triple                                                                             | Get session row                                       |
| `steop.session.list`   | `host?, project_dir?, state?, limit?`                                                                   | List sessions                                         |
| `steop.project.list`   | `host?`                                                                                                 | List `host:project_dir` combos                        |
| `steop.state.get`      | `session_id` or full triple                                                                             | Get session state + counters                          |
| `steop.state.put`      | `host, project_dir, session_id, data, merge?=true`                                                      | Upsert `data` JSON                                    |
| `steop.state.incr`     | `host, project_dir, session_id, counter, delta?=1`                                                      | Atomic counter increment                              |
| `steop.state.reset`    | `host, project_dir, session_id, counter, value?=0`                                                      | Reset counter                                         |
| `steop.state.delete`   | `host, project_dir, session_id`                                                                         | Delete session row                                    |
| `steop.status.get`     | `session_id` or full triple                                                                             | Statusline projection (never 404s)                    |
| `steop.storage.put`    | `host, project_dir, key, content, session_id?`                                                          | KV put (session if `session_id`, project otherwise)   |
| `steop.storage.get`    | `host, project_dir, key, session_id?`                                                                   | KV get                                                |
| `steop.storage.delete` | `host, project_dir, key, session_id?`                                                                   | KV delete                                             |
| `steop.storage.list`   | `host, project_dir, session_id?`                                                                        | List keys                                             |
| `steop.log.append`     | `host, project_dir, session_id, event, data?`                                                           | Append log entry                                      |
| `steop.log.query`      | `host?, project_dir?, session_id?, limit?=200`                                                          | Query logs (DESC)                                     |
| `steop.mailbox.send`   | `from_host, from_project_dir, from_session_id, to_host, to_project_dir, to_session_id?, kind, subject, payload` | Send message (session or project recipient)           |
| `steop.mailbox.list`   | `to_host, to_project_dir, to_session_id?, limit?=200, include_acked?=false`                             | List messages for recipient (FIFO)                    |
| `steop.mailbox.ack`    | `id`                                                                                                    | Mark message acked                                    |
| `steop.notify`         | `title?, body?, subtitle?, sound?=false`                                                                | Fire desktop notification                             |

**Tables (v0.6.0):** `steop_sessions` (merges state + counters, PK `(host, project_dir, session_id)`, JSON `data` + `counters` columns), `steop_storage_session`, `steop_storage_project`, `steop_mailbox` (replaces `steop_inbox`), `steop_logs`. Sender on mailbox is **always** a session; recipient may be a project (`to_session_id=''`) or a session. See `docs/steop/DESIGN.md` for schema and semantics.

## rmcp Conventions

- Tool methods go inside `#[tool_router] impl SteleServer { ... }` — this generates `Self::tool_router()`.
- `#[tool_handler] impl ServerHandler for SteleServer` auto-implements `call_tool`, `list_tools`, `get_tool` by delegating to the router stored in `self.tool_router`.
- Tool parameters use `Parameters<T>` extractor where `T: Deserialize + JsonSchema`.
- Tool return type is `String` (auto-converted to `Content::text` by rmcp).

## Stele Shared Memory Protocol

**Scope:** `stele` | **Type:** library

This is the core Stele repository. Use scope `stele` for all memories and entities.

### Storage

- **Flat Memory** (`store_memory`/`recall_memories`) — facts, decisions, conventions, notes.
- **Knowledge Graph** (`create_entities`/`create_relations`/`search_nodes`/`open_nodes`) — things with relationships.

### Scope & Retrieval

Scopes use **prefix matching** — querying `stele` also matches `stele/core`, `stele/api`, `stele/desktop`.

| Scope           | Covers                         |
| --------------- | ------------------------------ |
| `stele`         | Workspace-wide standards       |
| `stele/core`    | Server, DB, MCP protocol layer |
| `stele/api`     | REST API endpoints             |
| `stele/desktop` | Tray app, menu bar (macOS)     |

**Multi-scope reads:** `scope: ["stele", "global"]` to include shared cross-project knowledge. Write tools remain single-scope.

### Workflow

- **Task start:** Run `/stele:sync` — pulls latest shared state. Do not assume you know the current state.
- **Before architectural changes:** Run `open_nodes` or `read_graph` to check dependencies.
- **End of session:** Run `/stele:checkpoint` — persists decisions, discoveries, and fixes back to Stele.
- **New sub-module:** Run `/stele:bootstrap` to create a sub-scope.

### Autonomous Updates (no permission needed)

You MUST update Stele immediately when any of these occur — do not defer:

- **Contract change** (API, env var, shared interface) → store + tag `#contract #breaking`
- **Lesson learned** (non-obvious bug fix) → store + tag `#wisdom`
- **Relationship discovered** (A depends on B) → `create_relations`
- **Convention established** (new agreed rule) → store + tag `#active`

Standard tags: `#active`, `#todo`, `#contract`, `#breaking`, `#wisdom`, `#conflict`. Project-specific: `#public-api`, `#semver`, `#docs`. Run `/stele:checkpoint` for full tagging convention.

## Claude Code Plugin

The `plugins/stele/` directory contains a Claude Code marketplace plugin that provides skills and a subagent for working with Stele.

### Skills

- **`/stele:install`** — Configure Stele connection profile for your server. The plugin ships `.mcp.json` so MCP is auto-registered on install; this skill handles CLI profile setup for remote servers or auth keys.
- **`/stele:bootstrap`** — Initialize a project with Stele: creates scope, seeds entities in the knowledge graph, generates CLAUDE.md protocol section. Replaces the deprecated `bootstrap_project` MCP tool.
- **`/stele:sync`** — Pull latest shared team context (flat memories + knowledge graph) into the current session.
- **`/stele:checkpoint`** — Save session findings (decisions, bugs, conventions) back to Stele.

### Agent

- **stele-librarian** — Read-only subagent (Sonnet) for searching memories and graph nodes.

### Plugin Structure

```
plugins/stele/
├── .claude-plugin/plugin.json
├── skills/{install,bootstrap,sync,checkpoint}/SKILL.md
├── agents/stele-librarian.md
└── README.md
```

The plugin version in `plugins/stele/.claude-plugin/plugin.json` must match `apps/stele/Cargo.toml` workspace version. CI validates this.

## Versioning

SemVer. Major = breaking MCP/API/DB changes, minor = new features, patch = fixes and docs. Use `scripts/bump-version.py` to move versions in lock-step — see [docs/versioning.md](docs/versioning.md) for components, bump semantics, and usage.

## Steop Plugin (Agentic Workflow)

The `plugins/steop/` directory contains an agentic workflow pipeline plugin for Claude Code.

### Skills

- **`/steop:install`** — Build and install the `steop` companion binary to `~/.local/bin`. Required after installing the plugin so hooks can find `steop` on `PATH`.
- **`/steop:st-flow`** — Full pipeline: clarify -> research -> plan -> execute -> validate (research skipped for simple tasks)
- **`/steop:st-clarify`** — Clarify phase: analyze request, scope, complexity assessment
- **`/steop:st-research`** — Research phase: deep codebase investigation
- **`/steop:st-plan`** — Plan phase: implementation blueprint
- **`/steop:st-execute`** — Execute phase: implement changes per plan
- **`/steop:st-validate`** — Validate phase: review correctness and completeness

### Agents (5)

consultant (Opus), researcher (inherit), architect (Opus), executor (inherit), reviewer (Sonnet)

### Plugin Structure

```
plugins/steop/
├── .claude-plugin/plugin.json
├── hooks/hooks.json
├── skills/{install,st-flow,st-clarify,st-research,st-plan,st-execute,st-validate}/SKILL.md
├── agents/{consultant,researcher,architect,executor,reviewer}.md
└── README.md
```

The steop plugin hooks invoke a bare `steop` command, built from `apps/steop/` and installed to `~/.local/bin/steop` by `/steop:install`. The binary is not shipped with the plugin.

## macOS .app Bundle

Shell-script-based packaging using only macOS built-ins (`sips`, `iconutil`, `hdiutil`). No `cargo-bundle` dependency.

```bash
apps/stele/scripts/build-macos.sh          # builds apps/stele/target/release/Stele.app
apps/stele/scripts/build-dmg.sh            # creates apps/stele/target/release/Stele-0.1.0-macos.dmg
```

- **`apps/stele/assets/AppIcon.png`** — 1024×1024 source icon (menu bar icon is separate: `assets/icon.png` at 22×22).
- **`apps/stele/macos/Info.plist`** — Bundle metadata template. `__VERSION__` is substituted from `Cargo.toml` at build time. `LSUIElement=true` hides from Dock.
- **`apps/stele/scripts/build-macos.sh`** — Runs `cargo build --release`, generates `.icns` via `sips`+`iconutil`, assembles `.app` directory layout.
- **`apps/stele/scripts/build-dmg.sh`** — Wraps `Stele.app` in a compressed DMG with `/Applications` symlink.

## Docker

```bash
docker build -t stele apps/stele/                          # uses headless feature
docker run -v stele-data:/data -p 3100:3100 stele-server
```

## Plugin Marketplace Troubleshooting

When registering this repo as a local marketplace (`/plugin marketplace add <path>`), stale state can cause "Marketplace not found" errors. Known failure modes:

1. **Stale `extraKnownMarketplaces` in `~/.claude/settings.json`** — If the marketplace was previously registered under a different name (e.g. `stele-plugins` → `stele-marketplace`), the old entry in `settings.json` persists and conflicts. Fix: remove the old entry from `extraKnownMarketplaces` before re-adding.
2. **Orphaned plugin cache** — `~/.claude/plugins/cache/<marketplace-name>/` may contain `.orphaned_at` marker files from a previous failed resolution. Fix: `rm -rf ~/.claude/plugins/cache/<marketplace-name>` then re-add.
3. **Resolution order** — Remove marketplace fully (`/plugin marketplace remove`), clear cache, then re-add. Running `/plugin` to install individual plugins only works after the marketplace resolves cleanly.
