# Stele

Shared memory layer for [Claude Code](https://claude.ai/code). A single Rust binary that serves an [MCP](https://modelcontextprotocol.io/) interface over Streamable HTTP, backed by SQLite. Any Claude Code instance on the network can connect and share knowledge with the team.

## Quick Start

```bash
cargo build --release
./target/release/stele
```

Stele is now listening on `127.0.0.1:3100`. Connect Claude Code by adding to your `.mcp.json`:

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

| Flag         | Env Var          | Default           | Description                   |
| ------------ | ---------------- | ----------------- | ----------------------------- |
| `--bind`     | `STELE_BIND`     | `127.0.0.1:3100`  | Address to listen on          |
| `--db`       | `STELE_DB`       | `./stele.db`      | Path to SQLite database file  |
| `--mcp-path` | `STELE_MCP_PATH` | `/mcp`            | HTTP path for MCP endpoint    |

Set log level with `RUST_LOG` (e.g. `RUST_LOG=debug`).

## How It Works

Memories are organized along two dimensions:

**Scopes** — hierarchical, one per memory. Scopes work like paths and are prefix-matched when querying:

```
scope: "team-a/frontend"

# Found by querying:
scope: "team-a"            # prefix match
scope: "team-a/frontend"   # exact match
```

**Tags** — flat labels, many per memory. Filter by any matching tag (default) or require all tags with `match_all_tags`:

```
tags: ["vue", "auth", "component-patterns"]

# Found by querying:
tags: ["vue"]              # any match
tags: ["auth", "vue"]      # union (any) or intersection (all)
```

Both dimensions combine with full-text search on title and content.

## MCP Tools

| Tool              | Description                                   |
| ----------------- | --------------------------------------------- |
| `store_memory`    | Create a new shared memory                    |
| `recall_memories` | Search by keywords, scope, and/or tags        |
| `get_memory`      | Retrieve a memory by ID                       |
| `update_memory`   | Update title, content, scope, tags, or type   |
| `forget_memory`   | Delete a memory                               |
| `list_scopes`     | List scopes with memory counts                |
| `list_tags`       | List tags with memory counts                  |

### Memory Types

`knowledge`, `decision`, `convention`, `troubleshooting`, `reference`, `other`

## Docker

```bash
docker build -t stele .
docker run -d -p 3100:3100 -v stele-data:/data stele
```

The container stores the database at `/data/stele.db` and listens on `0.0.0.0:3100`.

## Building from Source

Requires Rust 1.75+. SQLite is bundled (no system SQLite needed).

```bash
cargo build --release
```

## License

MIT
