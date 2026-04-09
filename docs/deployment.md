# Deployment Guide

## Build Profiles

Build profiles are defined in the workspace `Cargo.toml`:

- **dev**: `debug = 0`, `incremental = false`
- **release**: `debug = 0`, `strip = true`, `lto = true`, `codegen-units = 1`, `opt-level = "s"`

`.cargo/config.toml` sets `build.default-profile = "release"`, so plain `cargo build` uses the release profile. This may produce an "unused config key" warning on some Rust versions.

## macOS Desktop App

Build the server with the default desktop feature:

```bash
cd apps/stele
cargo build -p stele-server
```

Default DB location: `~/Library/Application Support/Stele/stele.db`

### .app Bundle

```bash
scripts/build-macos.sh    # Builds Stele.app in target/release/
```

- Runs `cargo build --release -p stele-server`
- Generates `.icns` via `sips` + `iconutil` from `assets/AppIcon.png` (1024x1024)
- Assembles the `.app` directory layout with `Info.plist` (`LSUIElement=true` hides the app from the Dock)

### DMG

```bash
scripts/build-dmg.sh      # Creates Stele-0.2.0-macos.dmg
```

Creates a compressed DMG with a `/Applications` symlink for drag-to-install.

### Settings

The bind address is configurable via tray menu > Settings. Settings are persisted in `config.toml` next to the DB file.

## Linux Headless

Build the headless server (no desktop/tray dependencies):

```bash
cd apps/stele
cargo build -p stele-server --features headless --no-default-features
```

### systemd

A unit file is provided at `apps/stele/systemd/stele.service`.

```bash
# Install
scripts/install-system.sh

# Uninstall
scripts/uninstall-system.sh
```

Environment is configured via `/etc/default/stele`:

```
STELE_BIND=0.0.0.0:3100
STELE_DB=/var/lib/stele/stele.db
```

## Docker

```bash
docker build -t stele apps/stele/
docker run -v stele-data:/data -p 3100:3100 stele
```

- Multi-stage build: `rust:slim` builder, `debian:bookworm-slim` runtime
- Builds the headless server only
- Volume `/data` for persistent SQLite DB
- Default environment: `STELE_BIND=0.0.0.0:3100`, `STELE_DB=/data/stele.db`
- Exposes port 3100

## CLI Installation

```bash
cd apps/stele
cargo build -p stele-cli
# Copy target/release/stele to a directory on PATH
```

Initialize the config after installation:

```bash
stele config init
```

This creates `~/.config/stele/config.toml` with a `local` profile pointing to `http://127.0.0.1:3100`.

## Environment Variables

| Variable          | Used By | Default              | Description               |
| ----------------- | ------- | -------------------- | ------------------------- |
| `STELE_BIND`      | Server  | `127.0.0.1:3100`     | Bind address              |
| `STELE_DB`        | Server  | `./stele.db`         | SQLite database path      |
| `STELE_MCP_PATH`  | Server  | `/mcp`               | MCP endpoint path         |
| `STELE_URL`       | CLI     | (from config)        | Server URL override       |
| `STELE_AUTH_KEY`  | CLI     | (from config)        | Auth key override         |
| `STELE_PROFILE`   | CLI     | (from config)        | Profile name override     |

## Claude Code Integration

### Via CLI stdio proxy (recommended)

The `stele mcp` command bridges stdio to Streamable HTTP. Claude Code launches it as a child process:

```json
{
  "mcpServers": {
    "stele": { "command": "stele", "args": ["mcp"] },
    "stele-team": { "command": "stele", "args": ["--profile", "team", "mcp"] }
  }
}
```

Or via the CLI:

```bash
claude mcp add --scope user stele -- stele mcp
```

### Direct HTTP (Streamable HTTP)

If the `stele` CLI is not installed, Claude Code can connect directly (requires Streamable HTTP transport support):

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

### Via Plugin

Install the Stele plugin from the Claude Code marketplace, then run `/stele:install` to configure the MCP connection at user or project level.

## Plugins

- Claude Code plugins live in `plugins/stele/` and `plugins/steop/`
- Installed via the Claude Code marketplace
- The version in `plugins/stele/.claude-plugin/plugin.json` must match the workspace version in `apps/stele/Cargo.toml`
- CI validates version sync between the plugin manifest and the crate
