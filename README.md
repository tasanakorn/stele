# Stele

A suite of [Claude Code](https://claude.ai/code) plugins for shared memory and agentic workflows, backed by a lightweight Rust server.

## Stele Marketplace

All plugins are distributed through the Stele marketplace. To add it:

In Claude Code: `/plugin` → **Discover** → **Marketplaces** → **Add marketplace** → enter `tasanakorn/stele`

Then install any plugin: `/plugin` → **Discover** → select the plugin → **Install** → `/reload-plugins`

| Plugin    | Description                                                                | Docs                              |
| --------- | -------------------------------------------------------------------------- | --------------------------------- |
| **steop** | Agentic workflow pipeline with specialized agents                          | [README](plugins/steop/README.md) |
| **stele** | Shared team memory — flat memories + knowledge graph via MCP               | [README](plugins/stele/README.md) |

## Steop — Agentic Workflow Pipeline

steop turns a task description into a structured pipeline — **clarify → research → plan → execute → validate** — with specialized agents handling each phase (Opus for architecture, Sonnet for review). Works completely standalone, no server required.

```
/steop:st-flow <your task description>
```

| Skill    | Command              | Description                                              |
| -------- | -------------------- | -------------------------------------------------------- |
| Flow     | `/steop:st-flow`     | Full pipeline end-to-end                                 |
| Clarify  | `/steop:st-clarify`  | Analyze request, resolve ambiguities, produce task brief |
| Research | `/steop:st-research` | Deep codebase investigation and context gathering        |
| Plan     | `/steop:st-plan`     | Design implementation strategy and blueprint             |
| Execute  | `/steop:st-execute`  | Implement code changes according to plan                 |
| Validate | `/steop:st-validate` | Review changes for correctness and completeness          |

| Complexity | Pipeline                                            |
| ---------- | --------------------------------------------------- |
| Simple     | Clarify → Plan → Execute → Validate                 |
| Standard   | Clarify → Research → Plan → Execute → Validate      |
| Complex    | Clarify → Research → Plan → Execute → Validate      |

See the [steop README](plugins/steop/README.md) for agent details and configuration.

## Stele — Shared Team Memory

The stele plugin gives Claude Code persistent, shared memory across sessions and machines. It connects to a running Stele server via MCP.

| Skill      | Command             | Description                                                            |
| ---------- | ------------------- | ---------------------------------------------------------------------- |
| Install    | `/stele:install`    | Check Stele MCP connection and help configure it                       |
| Bootstrap  | `/stele:bootstrap`  | Initialize a project — create scope, seed entities, generate CLAUDE.md |
| Sync       | `/stele:sync`       | Pull latest shared team context into the current session               |
| Checkpoint | `/stele:checkpoint` | Save session findings back to Stele                                    |

**Typical workflow:**

1. `/stele:install` — first time only, verify server is reachable and MCP is configured
2. `/stele:bootstrap` — once per project, set up scope and conventions
3. `/stele:sync` — start of each session, pull latest team knowledge
4. `/stele:checkpoint` — end of session, save decisions and discoveries

The **stele-librarian** is a read-only subagent (Sonnet) for searching memories and graph nodes, automatically available when the plugin is installed.

See the [stele plugin README](plugins/stele/README.md) for full documentation.

### Manual MCP Setup (Without Plugin)

If you prefer not to use the plugin, connect Claude Code directly:

```bash
# User scope (available in all projects)
claude mcp add --scope user stele --transport http http://localhost:3100/mcp

# Project scope (shared via .mcp.json)
claude mcp add stele --transport http http://localhost:3100/mcp
```

Or add to `~/.claude/settings.json` (user) or `.mcp.json` (project):

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

## Stele App

The Stele server is a companion to the stele plugin. Single Rust binary, SQLite storage, no external dependencies. It serves [MCP](https://modelcontextprotocol.io/) over Streamable HTTP.

### Quick Start

**macOS (menu bar app):**

```bash
# Download Stele-x.x.x-macos.dmg from GitHub Releases, open and drag to /Applications
# Or build from source:
apps/stele/scripts/build-macos.sh
open apps/stele/target/release/Stele.app
```

**Docker:**

```bash
docker run -d -p 3100:3100 -v stele-data:/data ghcr.io/tasanakorn/stele
```

**Build from source:**

```bash
cd apps/stele && cargo build --release && ./target/release/stele
```

### How It Works

Stele provides two complementary memory systems:

#### Flat Memories

Prose entries for decisions, conventions, troubleshooting notes, and references. Organized by **scope** and **tags**.

**Scopes** are hierarchical and prefix-matched when querying:

```
scope: "acme/frontend"

# Found by querying:
scope: "acme"              # prefix match — matches all acme/* scopes
scope: "acme/frontend"     # exact match
```

**Tags** are flat labels, many per memory. Filter by any matching tag (default) or require all tags with `match_all_tags`:

```
tags: ["vue", "auth", "component-patterns"]

# Found by querying:
tags: ["vue"]              # any match
tags: ["auth", "vue"]      # union (any) or intersection (all)
```

Both dimensions combine with full-text search on title and content.

#### Knowledge Graph

Structured relationships between entities — services, components, people, dependencies. Three primitives:

- **Entities** — nodes with a name, type, and scope (unique within scope)
- **Observations** — atomic facts attached to entities
- **Relations** — directed edges between entities (e.g. `OrderService --depends_on--> PaymentService`)

Full-text search across entity names and observation content.

#### Memory Types

`knowledge`, `decision`, `convention`, `troubleshooting`, `reference`, `other`

### Configuration

| Flag         | Env Var          | Default                                                                                                          | Description                |
| ------------ | ---------------- | ---------------------------------------------------------------------------------------------------------------- | -------------------------- |
| `--bind`     | `STELE_BIND`     | `127.0.0.1:3100`                                                                                                 | Address to listen on       |
| `--db`       | `STELE_DB`       | `~/Library/Application Support/Stele/stele.db` (desktop) / `./stele.db` (headless)                               | SQLite database path       |
| `--mcp-path` | `STELE_MCP_PATH` | `/mcp`                                                                                                           | HTTP path for MCP endpoint |

Set log level with `RUST_LOG` (e.g. `RUST_LOG=debug`).

### MCP Tools

#### Flat Memory (7 tools)

| Tool              | Description                                 |
| ----------------- | ------------------------------------------- |
| `store_memory`    | Create a new shared memory                  |
| `recall_memories` | Search by keywords, scope, and/or tags      |
| `get_memory`      | Retrieve a memory by ID                     |
| `update_memory`   | Update title, content, scope, tags, or type |
| `forget_memory`   | Delete a memory                             |
| `list_scopes`     | List scopes with memory counts              |
| `list_tags`       | List tags with memory counts                |

#### Knowledge Graph (9 tools)

| Tool                  | Description                                                             |
| --------------------- | ----------------------------------------------------------------------- |
| `create_entities`     | Create nodes (idempotent — existing entities get observations appended) |
| `create_relations`    | Create directed edges (idempotent)                                      |
| `add_observations`    | Append atomic facts to an entity                                        |
| `delete_entities`     | Delete nodes (cascades observations + relations)                        |
| `delete_observations` | Remove specific facts by exact content match                            |
| `delete_relations`    | Remove specific edges                                                   |
| `read_graph`          | Full graph dump for a scope                                             |
| `search_nodes`        | FTS across entity names + observations                                  |
| `open_nodes`          | Fetch entities + their direct neighbor relations                        |

### REST API

JSON API mounted at `/api/v1` alongside the MCP endpoint. CORS enabled for browser access.

#### Flat Memory

| Method | Path                 | Description             |
| ------ | -------------------- | ----------------------- |
| GET    | /api/v1/memories     | Search/list memories    |
| POST   | /api/v1/memories     | Create a memory         |
| GET    | /api/v1/memories/:id | Get single memory       |
| PUT    | /api/v1/memories/:id | Update a memory         |
| DELETE | /api/v1/memories/:id | Delete a memory         |
| GET    | /api/v1/scopes       | List scopes with counts |
| GET    | /api/v1/tags         | List tags with counts   |
| GET    | /api/v1/stats        | Dashboard summary stats |

#### Knowledge Graph

| Method | Path                                      | Description         |
| ------ | ----------------------------------------- | ------------------- |
| GET    | /api/v1/graph?scope=                      | Read full graph     |
| POST   | /api/v1/graph/entities                    | Create entities     |
| GET    | /api/v1/graph/entities?q=&scope=          | Search entities     |
| GET    | /api/v1/graph/entities/:name?scope=       | Get entity by name  |
| DELETE | /api/v1/graph/entities/:name?scope=       | Delete entity       |
| POST   | /api/v1/graph/entities/:name/observations | Add observations    |
| DELETE | /api/v1/graph/entities/:name/observations | Delete observations |
| POST   | /api/v1/graph/relations                   | Create relations    |
| DELETE | /api/v1/graph/relations                   | Delete relations    |
| GET    | /api/v1/graph/open?names=a,b&scope=       | Open specific nodes |

### Installation Options

#### macOS .app Bundle

Download from [GitHub Releases](https://github.com/tasanakorn/stele/releases) or build locally:

```bash
apps/stele/scripts/build-macos.sh    # builds apps/stele/target/release/Stele.app
apps/stele/scripts/build-dmg.sh      # creates apps/stele/target/release/Stele-x.x.x-macos.dmg
```

- **Stele.app** — double-click to launch, or drag to `/Applications`
- **DMG** — compressed disk image with `/Applications` symlink for easy install

The app runs as a menu-bar-only utility (`LSUIElement=true`) — no Dock icon, just a tray icon. The database is stored at `~/Library/Application Support/Stele/stele.db`.

#### Docker

```bash
docker build -t stele apps/stele/
docker run -d -p 3100:3100 -v stele-data:/data stele
```

The container uses the headless build. Database is stored at `/data/stele.db`.

#### Linux systemd Service

```bash
sudo apps/stele/scripts/install-system.sh    # builds, creates stele user, installs service
sudo systemctl start stele
```

#### Building from Source

Requires Rust 1.75+. SQLite is bundled (no system SQLite needed).

```bash
cd apps/stele

# Desktop (default on macOS)
cargo build --release

# Headless
cargo build --release --features headless --no-default-features
```

## License

MIT
