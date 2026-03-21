# Stele

Shared memory layer for [Claude Code](https://claude.ai/code). A single Rust binary that serves an [MCP](https://modelcontextprotocol.io/) interface over Streamable HTTP, backed by SQLite. Any Claude Code instance on the network can connect and share knowledge with the team.

## Quick Start

### 1. Start the Server

**macOS (desktop)** — download from [GitHub Releases](https://github.com/tasanakorn/stele/releases) or build from source:

```bash
# Option A: Download the .app bundle
# Download Stele-x.x.x-macos.dmg from GitHub Releases
# Open the DMG and drag Stele.app to /Applications
# Launch from Applications — it appears as a menu bar icon (no Dock icon)

# Option B: Build and run from source
./scripts/build-macos.sh           # builds target/release/Stele.app
open target/release/Stele.app      # launch the menu bar app

# Option C: Run the binary directly (no .app bundle)
cargo build --release
./target/release/stele
```

The database is stored at `~/Library/Application Support/Stele/stele.db`.

**Linux / Docker (headless):**

```bash
cargo build --release --features headless --no-default-features
./target/release/stele
```

**Docker:**

```bash
docker run -d -p 3100:3100 -v stele-data:/data ghcr.io/tasanakorn/stele
```

Stele is now listening on `127.0.0.1:3100`.

### 2. Install the Plugin

Add the Stele marketplace and install the plugin:

```bash
# Add the marketplace (one-time)
claude plugin add tasanakorn/stele

# In Claude Code, install the plugin from the marketplace
/plugin
# → Select "Discover" → find "stele" → Install
# → Run /reload-plugins to activate
```

Or install directly within Claude Code:

```
/plugin
```

Navigate to **Discover** → **Marketplaces** → **Add marketplace** → enter `tasanakorn/stele` → then install the `stele` plugin.

The plugin auto-configures the MCP connection and provides skills + a subagent.

### 3. Bootstrap Your Project

```
/stele:bootstrap
```

The bootstrap skill asks for your project name, scope, and type, then:
- Creates a project entity in the knowledge graph
- Stores conventions as shared memory
- Writes a protocol section into your CLAUDE.md

That's it. All Claude Code sessions in this project now share knowledge through Stele.

## Usage

### Skills

The plugin provides four skills:

| Skill      | Command            | Description                                                        |
| ---------- | ------------------ | ------------------------------------------------------------------ |
| Install    | `/stele:install`   | Check Stele MCP connection and help configure it                   |
| Bootstrap  | `/stele:bootstrap` | Initialize a project — create scope, seed entities, generate CLAUDE.md |
| Sync       | `/stele:sync`      | Pull latest shared team context into the current session           |
| Checkpoint | `/stele:checkpoint`| Save session findings back to Stele                                |

**Typical workflow:**

1. `/stele:install` — first time only, verify server is reachable and MCP is configured
2. `/stele:bootstrap` — once per project, set up scope and conventions
3. `/stele:sync` — start of each session, pull latest team knowledge
4. `/stele:checkpoint` — end of session, save decisions and discoveries

### Agent

The **stele-librarian** is a read-only subagent (Sonnet) for searching memories and graph nodes. It's automatically available when the plugin is installed. Claude Code will use it when it needs to look up shared knowledge without writing anything.

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

## How It Works

Stele provides two complementary memory systems:

### Flat Memories

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

### Knowledge Graph

Structured relationships between entities — services, components, people, dependencies. Three primitives:

- **Entities** — nodes with a name, type, and scope (unique within scope)
- **Observations** — atomic facts attached to entities
- **Relations** — directed edges between entities (e.g. `OrderService --depends_on--> PaymentService`)

Full-text search across entity names and observation content.

### Memory Types

`knowledge`, `decision`, `convention`, `troubleshooting`, `reference`, `other`

## Configuration

| Flag         | Env Var          | Default                                                              | Description                |
| ------------ | ---------------- | -------------------------------------------------------------------- | -------------------------- |
| `--bind`     | `STELE_BIND`     | `127.0.0.1:3100`                                                     | Address to listen on       |
| `--db`       | `STELE_DB`       | `~/Library/Application Support/Stele/stele.db` (desktop) / `./stele.db` (headless) | SQLite database path |
| `--mcp-path` | `STELE_MCP_PATH` | `/mcp`                                                               | HTTP path for MCP endpoint |

Set log level with `RUST_LOG` (e.g. `RUST_LOG=debug`).

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

## Installation Options

### macOS .app Bundle

Download from [GitHub Releases](https://github.com/tasanakorn/stele/releases) or build locally:

```bash
./scripts/build-macos.sh    # builds target/release/Stele.app
./scripts/build-dmg.sh      # creates Stele-x.x.x-macos.dmg
```

- **Stele.app** — double-click to launch, or drag to `/Applications`
- **DMG** — compressed disk image with `/Applications` symlink for easy install

The app runs as a menu-bar-only utility (`LSUIElement=true`) — no Dock icon, just a tray icon. The database is stored at `~/Library/Application Support/Stele/stele.db`.

### Docker

```bash
docker build -t stele .
docker run -d -p 3100:3100 -v stele-data:/data stele
```

The container uses the headless build. Database is stored at `/data/stele.db`.

### Linux systemd Service

```bash
sudo ./scripts/install-system.sh    # builds, creates stele user, installs service
sudo systemctl start stele
```

### Building from Source

Requires Rust 1.75+. SQLite is bundled (no system SQLite needed).

```bash
# Desktop (default on macOS)
cargo build --release

# Headless
cargo build --release --features headless --no-default-features
```

## License

MIT
