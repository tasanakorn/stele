# CLI Reference

## Overview

`stele` is a CLI client for the Stele shared memory server. The binary is produced by the `stele-cli` crate and communicates with the server via the REST API using `ureq` (synchronous HTTP).

## Installation

```bash
cd apps/stele
cargo build -p stele-cli
# Binary at target/release/stele (or target/debug/stele)
```

## Global Flags

| Flag           | Env Var          | Description                     |
| -------------- | ---------------- | ------------------------------- |
| `--profile`    | `STELE_PROFILE`  | Named connection profile        |
| `--server-url` | `STELE_URL`      | Override server URL             |
| `--auth-key`   | `STELE_AUTH_KEY` | Override auth key               |
| `--json`       |                  | Output raw JSON instead of text |

## Configuration

Config file location: `~/.config/stele/config.toml` (resolved via `dirs::config_dir()`).

```toml
default_profile = "local"

[profiles.local]
server_url = "http://127.0.0.1:3100"

[profiles.team]
server_url = "http://stele.internal:3100"
auth_key = "team-shared-key"
```

**Connection resolution priority:** CLI flags > environment variables > named profile > default profile > fallback (`http://localhost:3100`).

## Memory Commands

### stele store

Create a memory.

```
stele store --title "..." --content "..." --scope myproject [--tags tag1,tag2] [--type decision] [--author "name"]
```

### stele recall

Search memories.

```
stele recall [query] [--scope myproject] [--tags tag1,tag2] [--match-all-tags] [--limit 10]
```

### stele get

Get memory by ID.

```
stele get <id>
```

### stele update

Update a memory.

```
stele update <id> [--title "..."] [--content "..."] [--scope newscope] [--tags a,b] [--type convention]
```

### stele forget

Delete a memory.

```
stele forget <id>
```

## Info Commands

### stele scopes

List scopes with counts.

```
stele scopes [--prefix team-a]
```

### stele tags

List tags with counts.

```
stele tags [--scope myproject]
```

### stele stats

Server statistics.

```
stele stats
```

### stele status

Health check.

```
stele status
```

## Graph Commands

### stele graph read

Read full graph.

```
stele graph read --scope myproject
```

### stele graph search

Search nodes.

```
stele graph search "query" [--scope myproject] [--limit 20]
```

### stele graph open

Open nodes with neighbors.

```
stele graph open "EntityA,EntityB" [--scope myproject]
```

### stele graph entities create

Create an entity.

```
stele graph entities create --name ServiceA --type service --scope myproject [--observations "fact1,fact2"]
```

### stele graph entities get

Get entity.

```
stele graph entities get ServiceA --scope myproject
```

### stele graph entities delete

Delete entity (cascades observations and relations).

```
stele graph entities delete ServiceA --scope myproject
```

### stele graph observations add

Add facts to an entity.

```
stele graph observations add ServiceA --scope myproject --observations "fact1,fact2"
```

### stele graph observations delete

Delete facts by content.

```
stele graph observations delete ServiceA --scope myproject --observations "fact1"
```

### stele graph relations create

Create a directed relation.

```
stele graph relations create --from ServiceA --to ServiceB --type depends_on --scope myproject
```

### stele graph relations delete

Delete a relation.

```
stele graph relations delete --from ServiceA --to ServiceB --type depends_on --scope myproject
```

## Config Commands

### stele config init

Create a default config file at `~/.config/stele/config.toml`.

### stele config show

Display the current resolved configuration.

### stele config path

Print the config file path.

## JSON Output

All commands support `--json` for machine-readable output. Useful for piping to other tools:

```bash
stele --json recall "auth" --scope myproject | jq '.[].title'
```

## MCP Proxy

```
stele mcp
stele --profile team mcp
stele --server-url http://remote:3100 mcp
```

Acts as a stdio-to-Streamable-HTTP bridge. Claude Code launches `stele mcp` as a child process, sends JSON-RPC messages over stdin (one per line), and receives responses on stdout. The proxy POSTs each message to the server's `/mcp` endpoint, parses SSE responses, and writes JSON-RPC back.

Session tracking is automatic -- the proxy captures the `mcp-session-id` header from the server's response and includes it on all subsequent requests. On stdin EOF (Claude Code disconnect), the proxy sends a DELETE to terminate the MCP session cleanly.

Configure Claude Code to use the proxy:

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

For a remote server or named profile:

```json
{
  "mcpServers": {
    "stele-team": {
      "command": "stele",
      "args": ["--profile", "team", "mcp"]
    }
  }
}
```
