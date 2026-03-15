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
- **`models.rs`** — Domain types: `Memory`, `SearchResult`, `MemoryType` enum, `ScopeInfo`, `TagInfo`, `Stats`.
- **`query.rs`** — `SearchParams` struct used to pass search criteria from server to db layer.
- **`config.rs`** — Clap derive struct with env var fallbacks. Desktop feature adds `with_desktop_defaults()` to relocate DB to `~/Library/Application Support/Stele/`.

## Data Model

Two-dimensional organization:

1. **Scope** (one per memory, hierarchical) — queried via prefix match: `scope = ?1 OR scope LIKE ?1||'/%'`. Example: querying `team-a` matches `team-a`, `team-a/frontend`, `team-a/backend`.
2. **Tags** (many per memory, flat labels) — stored in `memory_tags` join table. Filtered as union (any tag matches) by default, or intersection (all tags must match) with `match_all_tags`.

Full-text search uses SQLite FTS5 on title + content, kept in sync via INSERT/UPDATE/DELETE triggers. The FTS table uses content-sync mode (`content='memories'`).

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

## rmcp Conventions

- Tool methods go inside `#[tool_router] impl SteleServer { ... }` — this generates `Self::tool_router()`.
- `#[tool_handler] impl ServerHandler for SteleServer` auto-implements `call_tool`, `list_tools`, `get_tool` by delegating to the router stored in `self.tool_router`.
- Tool parameters use `Parameters<T>` extractor where `T: Deserialize + JsonSchema`.
- Tool return type is `String` (auto-converted to `Content::text` by rmcp).

## Docker

```bash
docker build -t stele .                              # uses headless feature
docker run -v stele-data:/data -p 3100:3100 stele
```
