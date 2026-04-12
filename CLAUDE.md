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
├── docs/                          # See "Documentation Layout" below
├── .claude-plugin/                # Marketplace definition
├── CLAUDE.md
└── README.md
```

## Documentation Layout

`docs/` is the single source of truth for all specs. Start at [docs/README.md](docs/README.md) for the full index with one-line descriptions of every doc. The layout:

- **`docs/*.md`** — **common, cross-module** specs that apply to the whole workspace (e.g. `architecture.md`, `versioning.md`). Anything a contributor touching any crate or plugin should be able to find without guessing a subfolder.
- **`docs/prd/`** — **product requirements documents**. Cross-cutting forward-looking design docs that drive future implementation cycles. Not bound to one module's current implementation detail.
- **`docs/<module>/`** — **module-specific detail**. Deep design, gap analyses, smoke tests, hook semantics — anything that only makes sense in the context of one subsystem. Current subfolders: `docs/stele/` (shared-memory server + MCP), `docs/steop/` (agentic workflow pipeline).

Rule of thumb: if a doc mentions specific SQL tables, struct field names, or hook handlers, it belongs in a module subfolder. If it describes the workspace as a whole or a forward-looking design, it belongs at the `docs/` root (or `docs/prd/` for PRDs).

```
docs/
├── architecture.md                # Cross-module architecture overview
├── versioning.md                  # Workspace-wide SemVer rules & bump script
├── prd/                           # Product requirements docs (forward-looking)
│   └── prd-001-mailbox-v2.md
├── stele/                         # stele-server / MCP / REST API details
│   └── ...
└── steop/                         # steop workflow pipeline details
    ├── DESIGN.md
    ├── smoke-tests.md
    └── ...
```

### File naming conventions

- **Module docs (`docs/<module>/`)** — free-form kebab-case filename describing the topic. Uppercase is reserved for canonical per-module entry points (`DESIGN.md`, `README.md`).
- **Root docs (`docs/*.md`)** — kebab-case (`architecture.md`, `versioning.md`).

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

Server CLI flags have env var equivalents: `STELE_BIND`, `STELE_DB`, `STELE_MCP_PATH`, `STELE_AUTH_KEY`. If `--auth-key` / `STELE_AUTH_KEY` is set (or `auth_key` is persisted in `config.toml`), all HTTP/MCP routes require clients to send `X-Stele-Key`; the CLI already does this automatically via `stele config set --key <key>`.

Build profiles are configured in the workspace `Cargo.toml` to minimize disk usage (`incremental = false`, `codegen-units = 1`, `opt-level = "s"`).

There are no tests yet. No linter or formatter is configured beyond standard `cargo clippy` / `cargo fmt`.

**Build artifacts:** When building Go binaries, use `-o` to output into a `target/` directory under the module (e.g. `cd apps/steop && go build -o target/steop .`). `**/target` is gitignored.

## Architecture

Single async process: axum serves HTTP, rmcp handles MCP protocol framing, SQLite stores everything. The CLI is a sync ureq client that talks to the server's REST API and also acts as an MCP stdio↔HTTP proxy.

**Three workspace crates** under `apps/stele/crates/`: `stele-common` (shared types), `stele-server` (MCP + REST + SQLite + optional tray), `stele-cli` (HTTP client + MCP proxy, binary named `stele`).

For the full component map see [docs/architecture.md](docs/architecture.md). For server internals (entry points, startup flow, MCP/REST layers, rmcp conventions, tray, shutdown) see [docs/stele/server.md](docs/stele/server.md).

## Data Model

Two axes: **scope** (one per memory, hierarchical, prefix-matched — `team-a` matches `team-a/frontend` etc.) and **tags** (many per memory, flat, union-by-default or intersection with `match_all_tags`). Full-text search via SQLite FTS5, kept in sync by triggers.

Knowledge graph is a separate surface: entities (nodes scoped by name), observations (facts on entities), relations (directed edges). Cascades on entity delete. Two FTS5 tables power `search_nodes`.

Full schema, triggers, SQL snippets, and multi-scope query semantics: [docs/stele/data-model.md](docs/stele/data-model.md).

## MCP Tools

- **Flat memory (7):** `store_memory`, `recall_memories`, `get_memory`, `update_memory`, `forget_memory`, `list_scopes`, `list_tags`
- **Knowledge graph (9):** `create_entities`, `create_relations`, `add_observations`, `delete_entities`, `delete_observations`, `delete_relations`, `read_graph`, `search_nodes`, `open_nodes`
- **Deprecated (1):** `bootstrap_project` — use the `/stele:bootstrap` plugin skill instead.

Parameter shapes, multi-scope read conventions, lenient deserialization, and usage notes: [docs/stele/mcp-tools.md](docs/stele/mcp-tools.md).

## REST API

JSON API mounted at `/api/v1` alongside the MCP endpoint. CORS enabled. Covers flat memories, scopes/tags/stats, knowledge graph, and the Steop RPC surface. Full endpoint reference with request/response shapes: [docs/stele/http-api.md](docs/stele/http-api.md).

### Steop RPC identity (load-bearing constraint)

Steop methods live at `POST /api/v1/steop/<method>` with body-only input — no path params, no query params, no header identity. Every call carries a **composite SSH/SCP-style `id` string** that must be one of three forms:

- `host:project_dir` — 2-segment, project-level
- `host:project_dir:UUID` — 3-segment, session-level (canonical 8-4-4-4-12)
- `host:project_dir:USER` — 3-segment, user-level (literal `USER`, case-sensitive)

The 3rd segment is a **closed set** (v0.8+): UUID or literal `USER`. Anything else returns 400. `session.get`, `state.get`, and `status.get` require the 3-segment form. Storage dispatches on arity: 2-seg → project KV, 3-seg → session KV. The server only validates segment grammar; clients must compose ids correctly.

Full method list, request bodies, table schemas, and rationale: [docs/steop/DESIGN.md](docs/steop/DESIGN.md). Curl smoke tests: [docs/steop/smoke-tests.md](docs/steop/smoke-tests.md).

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
- **`/steop:st-send`** — Send a task to another Claude Code session by project name shorthand
- **`/steop:st-watch`** — Monitor mailbox for task requests and process them autonomously
- **`/steop:st-prd`** — PRD authoring: interactive clarify -> docs-first research -> convention-correct PRD file

### Agents (5)

consultant (Opus), researcher (inherit), architect (Opus), executor (inherit), reviewer (Sonnet)

### Plugin Structure

```
plugins/steop/
├── .claude-plugin/plugin.json
├── hooks/hooks.json
├── skills/{install,st-flow,st-clarify,st-research,st-plan,st-execute,st-validate,st-watch,st-send,st-prd}/SKILL.md
├── agents/{consultant,researcher,architect,executor,reviewer}.md
└── README.md
```

The steop plugin hooks invoke a bare `steop` command, built from `apps/steop/` and installed to `~/.local/bin/steop` by `/steop:install`. The binary is not shipped with the plugin.

## Packaging & Deployment

macOS `.app` bundle, DMG, Linux headless/systemd, and Docker are all covered in [docs/stele/deployment.md](docs/stele/deployment.md). Packaging scripts live under `apps/stele/scripts/` (`build-macos.sh`, `build-dmg.sh`).

## Plugin Marketplace Troubleshooting

Stale state after re-registering this repo as a local Claude Code marketplace — "Marketplace not found", orphaned cache, conflicting `extraKnownMarketplaces` — see [docs/plugin-marketplace-troubleshooting.md](docs/plugin-marketplace-troubleshooting.md) for failure modes and the recovery recipe.
