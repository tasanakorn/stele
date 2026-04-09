# Stele

**Give your Claude Code agents persistent memory and structured workflows that work across sessions and machines.**

[![CI](https://github.com/tasanakorn/stele/actions/workflows/ci.yml/badge.svg)](https://github.com/tasanakorn/stele/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

Stele is a suite of [Claude Code](https://claude.ai/code) plugins. **steop** gives your agents a structured workflow pipeline with specialized sub-agents. **stele** gives them shared, persistent memory backed by a lightweight Rust server. Install from the marketplace in seconds — no config files to edit.

## Quick Start

```bash
# In Claude Code — add the marketplace and install a plugin
/plugin → Discover → Marketplaces → Add marketplace → tasanakorn/stele
/plugin → Discover → select a plugin → Install → /reload-plugins
```

Then try it:

```
/steop:st-flow Refactor the auth module to use JWT tokens
```

## Plugins

| Plugin | What it does | Prerequisites | Docs |
| --- | --- | --- | --- |
| **steop** | Structured workflow pipeline with specialized agents | None — works out of the box | [README](plugins/steop/README.md) |
| **stele** | Persistent shared memory — flat memories + knowledge graph via MCP | Git, [Rust toolchain](https://rustup.rs/) — `/stele:install` builds and installs everything | [README](plugins/stele/README.md) |
| **stelite** | Lightweight inline variant of steop (unmaintained snapshot) | None — works out of the box | [README](plugins/stelite/README.md) |

---

## Steop — Agentic Workflow Pipeline

steop turns a task description into a structured pipeline with specialized agents handling each phase — Opus for architecture, Sonnet for review. Works completely standalone, no server required.

```
/steop:st-flow <your task description>
```

**Pipeline phases:**

| Phase | Command | What happens |
| --- | --- | --- |
| Clarify | `/steop:st-clarify` | Analyze request, resolve ambiguities, produce task brief |
| Research | `/steop:st-research` | Deep codebase investigation and context gathering |
| Plan | `/steop:st-plan` | Design implementation strategy and blueprint |
| Execute | `/steop:st-execute` | Implement code changes according to plan |
| Validate | `/steop:st-validate` | Review changes for correctness and completeness |

Simple tasks skip Research and go straight from Clarify to Plan. Use `/steop:st-flow` to run the full pipeline automatically, or invoke individual phases when you need fine-grained control.

See the [steop README](plugins/steop/README.md) for agent details and configuration.

---

## Stele — Shared Team Memory

The stele plugin gives Claude Code persistent, shared memory across sessions and machines. It connects to a running Stele server via MCP.

**The problem:** Claude Code starts every session with a blank slate. Decisions made yesterday, conventions agreed on last week, bugs debugged an hour ago — all gone. Stele fixes this by giving your agents a shared memory layer that persists across sessions and machines.

**Typical workflow:**

1. `/stele:install` — first time only, configure connection profile (MCP is auto-registered by the plugin)
2. `/stele:bootstrap` — once per project, set up scope and conventions
3. `/stele:sync` — start of each session, pull latest team knowledge
4. `/stele:checkpoint` — end of session, save decisions and discoveries

| Skill | Command | Description |
| --- | --- | --- |
| Install | `/stele:install` | Configure connection profile for your server      |
| Bootstrap | `/stele:bootstrap` | Initialize a project — create scope, seed entities, generate CLAUDE.md |
| Sync | `/stele:sync` | Pull latest shared team context into the current session |
| Checkpoint | `/stele:checkpoint` | Save session findings back to Stele |

The **stele-librarian** is a read-only subagent (Sonnet) for searching memories and graph nodes, automatically available when the plugin is installed.

See the [stele plugin README](plugins/stele/README.md) for full documentation.

### Manual MCP Setup (Without Plugin)

If you prefer not to use the plugin, connect Claude Code via the CLI stdio proxy:

```bash
# User scope (available in all projects)
claude mcp add --scope user stele -- stele mcp

# Project scope (shared via .mcp.json)
claude mcp add stele -- stele mcp
```

Or add to `~/.claude/settings.json` (user) or `.mcp.json` (project):

```json
{
  "mcpServers": {
    "stele": {
      "command": "stele",
      "args": ["mcp"]
    }
  }
}
```

For remote servers, pass the URL as a flag:

```json
{
  "mcpServers": {
    "stele": {
      "command": "stele",
      "args": ["--server-url", "http://remote:3100", "mcp"]
    }
  }
}
```

**Alternative (direct HTTP, no CLI needed):**

```json
{
  "mcpServers": {
    "stele": {
      "type": "http",
      "url": "http://localhost:3100/mcp"
    }
  }
}
```

Without the plugin, use the `bootstrap_project` MCP tool to generate the CLAUDE.md protocol section:

```
Bootstrap this project with stele, scope = "acme", this is a web app
```

**Supported project types:** `web-app`, `frontend`, `api`, `backend`, `library`, `sdk`, `monorepo`, `data-pipeline`, `ml`, or `general` (default).

---

## Stele Server

The Stele server is the backend for the stele plugin. Single Rust binary, SQLite storage, no external dependencies. It serves [MCP](https://modelcontextprotocol.io/) over Streamable HTTP.

### Install

**Docker** (quickest):

```bash
docker run -d -p 3100:3100 -v stele-data:/data ghcr.io/tasanakorn/stele
```

**macOS** (menu bar app):

```bash
# Download from GitHub Releases, or build from source:
apps/stele/scripts/build-macos.sh
open apps/stele/target/release/Stele.app
```

**Linux** (systemd):

```bash
sudo apps/stele/scripts/install-system.sh
sudo systemctl start stele
```

**From source** (Rust 1.75+, SQLite bundled):

```bash
cd apps/stele
cargo build --release                                        # desktop (macOS)
cargo build --release --features headless --no-default-features  # headless
```

### Configuration

| Flag | Env Var | Default | Description |
| --- | --- | --- | --- |
| `--bind` | `STELE_BIND` | `127.0.0.1:3100` | Address to listen on |
| `--db` | `STELE_DB` | `~/Library/Application Support/Stele/stele.db` (desktop) / `./stele.db` (headless) | SQLite database path |
| `--mcp-path` | `STELE_MCP_PATH` | `/mcp` | HTTP path for MCP endpoint |

Set log level with `RUST_LOG` (e.g. `RUST_LOG=debug`).

### Memory Model

Stele provides two complementary memory systems:

**Flat Memories** — prose entries for decisions, conventions, troubleshooting notes, and references. Organized by **scope** (hierarchical, prefix-matched) and **tags** (flat labels). Full-text search on title and content.

**Knowledge Graph** — structured relationships between entities (services, components, dependencies). Three primitives: entities (nodes), observations (facts attached to entities), and relations (directed edges). Full-text search across entity names and observation content.

Memory types: `knowledge`, `decision`, `convention`, `troubleshooting`, `reference`, `other`.

### API Reference

<details>
<summary>MCP Tools (16 tools)</summary>

#### Flat Memory (7 tools)

| Tool | Description |
| --- | --- |
| `store_memory` | Create a new shared memory |
| `recall_memories` | Search by keywords, scope, and/or tags |
| `get_memory` | Retrieve a memory by ID |
| `update_memory` | Update title, content, scope, tags, or type |
| `forget_memory` | Delete a memory |
| `list_scopes` | List scopes with memory counts |
| `list_tags` | List tags with memory counts |

#### Knowledge Graph (9 tools)

| Tool | Description |
| --- | --- |
| `create_entities` | Create nodes (idempotent — existing entities get observations appended) |
| `create_relations` | Create directed edges (idempotent) |
| `add_observations` | Append atomic facts to an entity |
| `delete_entities` | Delete nodes (cascades observations + relations) |
| `delete_observations` | Remove specific facts by exact content match |
| `delete_relations` | Remove specific edges |
| `read_graph` | Full graph dump for a scope |
| `search_nodes` | FTS across entity names + observations |
| `open_nodes` | Fetch entities + their direct neighbor relations |

</details>

<details>
<summary>REST API</summary>

JSON API mounted at `/api/v1` alongside the MCP endpoint. CORS enabled for browser access.

#### Flat Memory

| Method | Path | Description |
| --- | --- | --- |
| GET | /api/v1/memories | Search/list memories |
| POST | /api/v1/memories | Create a memory |
| GET | /api/v1/memories/:id | Get single memory |
| PUT | /api/v1/memories/:id | Update a memory |
| DELETE | /api/v1/memories/:id | Delete a memory |
| GET | /api/v1/scopes | List scopes with counts |
| GET | /api/v1/tags | List tags with counts |
| GET | /api/v1/stats | Dashboard summary stats |

#### Knowledge Graph

| Method | Path | Description |
| --- | --- | --- |
| GET | /api/v1/graph?scope= | Read full graph |
| POST | /api/v1/graph/entities | Create entities |
| GET | /api/v1/graph/entities?q=&scope= | Search entities |
| GET | /api/v1/graph/entities/:name?scope= | Get entity by name |
| DELETE | /api/v1/graph/entities/:name?scope= | Delete entity |
| POST | /api/v1/graph/entities/:name/observations | Add observations |
| DELETE | /api/v1/graph/entities/:name/observations | Delete observations |
| POST | /api/v1/graph/relations | Create relations |
| DELETE | /api/v1/graph/relations | Delete relations |
| GET | /api/v1/graph/open?names=a,b&scope= | Open specific nodes |

</details>

---

## Contributing

Contributions are welcome! The project uses standard Rust tooling:

```bash
cd apps/stele
cargo fmt --check    # formatting
cargo clippy         # lints
```

There are no tests yet — adding test coverage is a great way to contribute.

## License

MIT
