# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What is Stele

Stele is a shared memory server for Claude Code. It exposes an MCP (Model Context Protocol) interface over Streamable HTTP so multiple Claude Code instances across different machines can store and retrieve shared knowledge. Single Rust binary, SQLite storage, no external dependencies.

## Build & Run

Two build modes controlled by feature flags:

```bash
# Desktop / Menu Bar (default on macOS)
cargo build                # dev build — tray icon + menu bar
cargo build --release      # release build
cargo run                  # menu bar app, DB at ~/Library/Application Support/Stele/

# Headless daemon (Linux/Docker)
cargo build --features headless --no-default-features
cargo run --features headless --no-default-features

# CLI flags (both modes)
cargo run -- --bind 0.0.0.0:3100 --db /path/to/stele.db --mcp-path /mcp
```

All CLI flags have env var equivalents: `STELE_BIND`, `STELE_DB`, `STELE_MCP_PATH`.

There are no tests yet. No linter or formatter is configured beyond standard `cargo clippy` / `cargo fmt`.

## Architecture

The server is a single async process: axum serves HTTP, rmcp handles MCP protocol framing, SQLite stores everything.

- **`main.rs`** — Dual entry point. Desktop mode (`#[cfg(feature = "desktop")]`) runs the tray app on the main thread and the server on a background thread. Headless mode (`#[cfg(not(feature = "desktop"))]`) uses `#[tokio::main]`. Shared `run_server()` function handles axum/rmcp setup. Graceful shutdown via `CancellationToken`.
- **`tray.rs`** — macOS menu bar module (`#[cfg(feature = "desktop")]`). `TrayApp` creates a tray icon with status label, "Open Dashboard", and "Quit Stele" menu items. Uses `tray-icon` + `muda` crates.
- **`server.rs`** — `SteleServer` implements rmcp's `ServerHandler`. Tools are defined with rmcp's `#[tool_router]` / `#[tool_handler]` macros. Each tool method locks the DB mutex, calls into `db.rs`, and returns a JSON string. Tool parameter structs must derive `schemars::JsonSchema` (v1, not v0.8 — rmcp requires schemars v1).
- **`db.rs`** — SQLite schema init (tables + FTS5 + triggers), all CRUD functions. `DbPool` is `Arc<Mutex<Connection>>` (tokio mutex). SQL is built dynamically in `search_memories` using helper functions that append scope/tag filter clauses with positional parameter tracking (`?N` style).
- **`api.rs`** — REST API router mounted at `/api`. Axum handlers with JSON request/response, CORS via `tower-http`. Reuses `db.rs` functions directly.
- **`models.rs`** — Domain types: `Memory`, `SearchResult`, `MemoryType` enum, `ScopeInfo`, `TagInfo`, `Stats`, plus knowledge graph types: `Entity`, `Observation`, `Relation`, `Graph`, `EntitySearchResult`.
- **`serde_helpers.rs`** — Lenient deserialization helpers. `string_or_vec`/`string_or_vec_opt` handle JSON-encoded arrays in strings. `string_or_string_vec`/`string_or_string_vec_opt` handle bare strings or arrays of strings (used for multi-scope parameters).
- **`query.rs`** — `SearchParams` struct used to pass search criteria from server to db layer. `scope` is `Option<Vec<String>>` to support multi-scope queries.
- **`config.rs`** — Clap derive struct with env var fallbacks. Desktop feature adds `with_desktop_defaults()` to relocate DB to `~/Library/Application Support/Stele/`.

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

**Bootstrap tool (1):** `bootstrap_project` — generates a CLAUDE.md snippet teaching Claude Code how to use both flat memory and knowledge graph for a project.

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

## rmcp Conventions

- Tool methods go inside `#[tool_router] impl SteleServer { ... }` — this generates `Self::tool_router()`.
- `#[tool_handler] impl ServerHandler for SteleServer` auto-implements `call_tool`, `list_tools`, `get_tool` by delegating to the router stored in `self.tool_router`.
- Tool parameters use `Parameters<T>` extractor where `T: Deserialize + JsonSchema`.
- Tool return type is `String` (auto-converted to `Content::text` by rmcp).

## Stele Shared Memory Protocol

**Scope:** `stele` | **Type:** library

This is the core Stele repository. Use scope `stele` for all memories and entities. Sub-scopes: `stele/core` (server, DB, MCP), `stele/api` (REST endpoints), `stele/desktop` (tray/menu bar).

### Hybrid Storage Strategy

- **Flat Memory** — decisions, conventions, troubleshooting, references. Use `store_memory` / `recall_memories`.
- **Knowledge Graph** — architecture, components, dependencies. Use `create_entities` / `create_relations` / `search_nodes` / `open_nodes` / `read_graph`.
- Rule of thumb: **fact or note** → flat memory. **Thing with relationships** → knowledge graph.

### Knowledge Synchronization

- **On Boot:** At the start of every task, run `recall_memories(scope: ["stele", "global"])` and `search_nodes(query: "*", scope: ["stele", "global"])`. Do not assume you know the current state.
- **Dependency Awareness:** Before architectural changes, run `open_nodes` or `read_graph` to check what depends on the module you're changing.
- **Sub-projects:** When creating a new sub-module, call `bootstrap_project(project_name: "module-name", parent_scope: "stele")`.

### Update-on-Change Protocol (Autonomous)

You MUST update remote memory immediately when any of the following change — no permission needed:
- **Contract changes:** API signature, env var, or shared interface → `store_memory` with `#contract` tag + `add_observations` on the relevant entity. Tag `#breaking` if it affects other services.
- **Lessons learned:** Non-obvious bug fix → `add_observations` on the entity + `store_memory` with `#wisdom` tag.
- **Relationship discovery:** If you find module A depends on module B → `create_relations`.

### Tagging Convention

| Tag         | Meaning                                              |
| ----------- | ---------------------------------------------------- |
| `#active`   | Currently implemented and enforced rules             |
| `#todo`     | Technical debt or pending migrations                 |
| `#contract` | Inter-service API definitions and shared interfaces  |
| `#breaking` | Changes that require other agents/services to update |
| `#wisdom`   | Non-obvious technical discoveries and gotchas        |
| `#conflict` | Local rule that conflicts with a workspace-level rule|

Project-specific: `#public-api`, `#semver`, `#docs`

### Scope Guide

Queries use prefix matching: `recall_memories(scope: "stele")` matches `stele`, `stele/core`, `stele/api`, etc.

| Scope           | What it covers                     |
| --------------- | ---------------------------------- |
| `stele`         | Workspace-wide standards           |
| `stele/core`    | Server, DB, MCP protocol layer     |
| `stele/api`     | REST API endpoints                 |
| `stele/desktop` | Tray app, menu bar (macOS)         |

#### Multi-Scope Retrieval

Read tools (`recall_memories`, `search_nodes`, `read_graph`, `open_nodes`, `list_tags`) accept `scope` as a string or array of strings. Use array form to include the `global` scope for shared cross-project knowledge. Write tools remain single-scope.

- `recall_memories(scope: ["stele", "global"])` — project + shared global knowledge
- `recall_memories(scope: "stele")` — project only (children included via prefix match)
- REST API: comma-separated — `GET /api/v1/memories?scope=stele,global`

## Docker

```bash
docker build -t stele .                              # uses headless feature
docker run -v stele-data:/data -p 3100:3100 stele
```
