# Design: Multi-Binary Architecture — Stele Server + CLI

## Summary

Split Stele from a single binary into a Cargo workspace with two binaries:
- **`stele-server`** — the full server (MCP + REST API + optional desktop tray)
- **`stele`** — lightweight CLI client + MCP stdio proxy

## Motivation

Currently Stele is a monolithic binary. Users need a lightweight way to:
1. Interact with a running Stele server from the command line (store, recall, search memories)
2. Use Stele as an MCP server in Claude Code config without direct HTTP connectivity (stdio proxy)
3. Connect to multiple server instances (local dev, team, production)
4. Optionally secure the server with a pre-shared key

## Proposed Changes

### 1. Cargo Workspace Restructure

Move from a single crate to a workspace with parallel subdirectories:

```
apps/stele/
├── Cargo.toml              ← [workspace] only
├── server/                 ← stele-server (moved from src/)
│   ├── Cargo.toml
│   └── src/ (existing source files + auth.rs)
├── cli/                    ← stele-cli (new, bin name "stele")
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs
│       ├── commands.rs     ← clap subcommands
│       ├── client.rs       ← HTTP client (REST API calls)
│       ├── config.rs       ← multi-profile config file
│       ├── mcp_proxy.rs    ← stdio-to-HTTP MCP proxy
│       └── auto_start.rs   ← local server management
```

### 2. Authentication (Server-Side)

Add optional pre-shared key authentication:
- **Auth modes:** `none` (default) or `preshared`
- Server checks `X-Stele-Key` header via axum middleware
- Configured via `--auth-key` / `STELE_AUTH_KEY` or `config.toml`
- Applies to both MCP endpoint and REST API
- Constant-time key comparison to prevent timing attacks

### 3. CLI with Multi-Profile Config

Config file at `~/.config/stele/config.toml` with named connection profiles:

```toml
default_profile = "local"

[profiles.local]
url = "http://127.0.0.1:3100"
mode = "local"
auth = "none"

[profiles.local.auto_start]
enabled = true
binary = "stele-server"

[profiles.team]
url = "http://stele.internal:3100"
mode = "remote"
auth = "preshared"
key = "team-shared-key"

[profiles.prod]
url = "https://stele.example.com"
mode = "remote"
auth = "preshared"
key = "prod-secret-key"
```

Priority order: CLI flags > env vars > profile from config > defaults.

### 4. CLI Commands

All commands map 1:1 to existing REST API endpoints via sync HTTP (ureq):

```bash
# Memory CRUD
stele store --title "..." --content "..." --scope myproject --tags tag1,tag2
stele recall --query "..." --scope myproject
stele get <id>
stele update <id> --title "..."
stele forget <id>

# Info
stele stats / scopes / tags

# Knowledge graph
stele graph read/search/open
stele graph entity create/get/delete
stele graph relation create/delete
stele graph observation add/delete

# MCP stdio proxy
stele mcp
stele --profile team mcp

# Config management
stele config init / show / path
stele config add-profile / remove-profile / use / list-profiles

# Output
stele --json recall ...   # raw JSON for piping
```

### 5. MCP Stdio Proxy (`stele mcp`)

Raw JSON-RPC transport bridge — reads from stdin, POSTs to server's `/mcp`, writes response to stdout. Handles `Mcp-Session-Id` for session continuity. No rmcp dependency needed — fully synchronous.

Claude Code config:
```json
{
  "mcpServers": {
    "stele": { "command": "stele", "args": ["mcp"] },
    "stele-team": { "command": "stele", "args": ["--profile", "team", "mcp"] }
  }
}
```

### 6. Auto-Start (Local Mode)

When `mode = "local"` and `auto_start.enabled = true`:
1. Health check the server URL
2. If unreachable, spawn `stele-server` as a detached background process
3. Poll for readiness (up to 5 seconds)
4. Proceed with the command

### CLI Dependencies

Minimal: `clap`, `ureq`, `serde`, `serde_json`, `toml`, `dirs`. No tokio, no rmcp, no SQLite.

## Non-Goals (for this issue)

- TLS/mTLS between CLI and server
- User/role-based access control (only pre-shared key)
- CLI as a standalone MCP server with its own storage

## Open Questions

1. Should `stele mcp` also support SSE (server-sent events) for notifications, or is request/response sufficient for the initial implementation?
2. Should the CLI config support key rotation (multiple valid keys)?
3. Should auto-start capture server logs somewhere (e.g., `~/.local/share/stele/server.log`)?
