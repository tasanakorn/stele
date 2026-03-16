# Stele

Shared memory layer for [Claude Code](https://claude.ai/code). A single Rust binary that serves an [MCP](https://modelcontextprotocol.io/) interface over Streamable HTTP, backed by SQLite. Any Claude Code instance on the network can connect and share knowledge with the team.

## Quick Start

### Desktop (macOS — default)

Runs as a menu bar app with a tray icon.

```bash
cargo build --release
./target/release/stele
```

The database is stored at `~/Library/Application Support/Stele/stele.db`.

### Headless (Linux / Docker / CI)

```bash
cargo build --release --features headless --no-default-features
./target/release/stele
```

Stele is now listening on `127.0.0.1:3100`.

### Connect Claude Code

**Option A — CLI** (recommended):

```bash
# User scope (available in all projects)
claude mcp add --scope user stele --transport http http://localhost:3100/mcp

# Project scope (current project only)
claude mcp add stele --transport http http://localhost:3100/mcp
```

Verify with `claude mcp list`.

**Option B — Settings file:**

Add to `~/.claude/settings.json` (user scope) or `.mcp.json` (project scope):

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

## Configuration

| Flag         | Env Var          | Default                                            | Description                  |
| ------------ | ---------------- | -------------------------------------------------- | ---------------------------- |
| `--bind`     | `STELE_BIND`     | `127.0.0.1:3100`                                   | Address to listen on         |
| `--db`       | `STELE_DB`       | `~/Library/Application Support/Stele/stele.db` (desktop) / `./stele.db` (headless) | Path to SQLite database file |
| `--mcp-path` | `STELE_MCP_PATH` | `/mcp`                                             | HTTP path for MCP endpoint   |

Set log level with `RUST_LOG` (e.g. `RUST_LOG=debug`).

## How It Works

Stele provides two complementary memory systems:

### Flat Memories

Prose entries for decisions, conventions, troubleshooting notes, and references. Organized by **scope** and **tags**.

**Scopes** are hierarchical and prefix-matched when querying:

```
scope: "team-a/frontend"

# Found by querying:
scope: "team-a"            # prefix match
scope: "team-a/frontend"   # exact match
```

**Tags** are flat labels, many per memory. Filter by any matching tag (default) or require all tags with `match_all_tags`:

```
tags: ["vue", "auth", "component-patterns"]

# Found by querying:
tags: ["vue"]              # any match
tags: ["auth", "vue"]      # union (any) or intersection (all)
```

Both dimensions combine with full-text search on title and content.

### Knowledge Graph

Structured relationships between entities — services, components, people, dependencies. Three primitives:

- **Entities** — nodes with a name, type, and scope (unique within scope)
- **Observations** — atomic facts attached to entities
- **Relations** — directed edges between entities (e.g. `OrderService --depends_on--> PaymentService`)

Full-text search across entity names and observation content.

## MCP Tools

### Flat Memory (7 tools)

| Tool              | Description                                 |
| ----------------- | ------------------------------------------- |
| `store_memory`    | Create a new shared memory                  |
| `recall_memories` | Search by keywords, scope, and/or tags      |
| `get_memory`      | Retrieve a memory by ID                     |
| `update_memory`   | Update title, content, scope, tags, or type |
| `forget_memory`   | Delete a memory                             |
| `list_scopes`     | List scopes with memory counts              |
| `list_tags`       | List tags with memory counts                |

### Knowledge Graph (9 tools)

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

### Bootstrap (1 tool)

| Tool                | Description                                                                          |
| ------------------- | ------------------------------------------------------------------------------------ |
| `bootstrap_project` | Generate a CLAUDE.md operational protocol for using Stele in a new project           |

The `bootstrap_project` tool produces a comprehensive protocol covering hybrid storage strategy, knowledge synchronization, update-on-change rules, tagging conventions, and scope guidance — tailored to the project type (web-app, api, library, monorepo, data-pipeline, etc.).

### Memory Types

`knowledge`, `decision`, `convention`, `troubleshooting`, `reference`, `other`

## REST API

JSON API mounted at `/api/v1` alongside the MCP endpoint. CORS enabled for browser access.

### Flat Memory

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

### Knowledge Graph

| Method | Path                                      | Description        |
| ------ | ----------------------------------------- | ------------------ |
| GET    | /api/v1/graph?scope=                      | Read full graph    |
| POST   | /api/v1/graph/entities                    | Create entities    |
| GET    | /api/v1/graph/entities?q=&scope=          | Search entities    |
| GET    | /api/v1/graph/entities/:name?scope=       | Get entity by name |
| DELETE | /api/v1/graph/entities/:name?scope=       | Delete entity      |
| POST   | /api/v1/graph/entities/:name/observations | Add observations   |
| DELETE | /api/v1/graph/entities/:name/observations | Delete observations|
| POST   | /api/v1/graph/relations                   | Create relations   |
| DELETE | /api/v1/graph/relations                   | Delete relations   |
| GET    | /api/v1/graph/open?names=a,b&scope=       | Open specific nodes|

## Docker

```bash
docker build -t stele .
docker run -d -p 3100:3100 -v stele-data:/data stele
```

The container uses the headless build. Database is stored at `/data/stele.db`.

## Building from Source

Requires Rust 1.75+. SQLite is bundled (no system SQLite needed).

```bash
# Desktop (default on macOS)
cargo build --release

# Headless
cargo build --release --features headless --no-default-features
```

## License

MIT
