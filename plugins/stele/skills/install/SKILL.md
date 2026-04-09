---
name: install
description: Configure Stele connection profile for your server
user-invocable: true
---

# Configure Stele Connection

Set up the connection profile so the Stele CLI and MCP proxy can reach your server.

The MCP server is **automatically registered** when this plugin is installed — no manual MCP configuration is needed.

## Prerequisites

The `stele` CLI binary must be installed and available on your PATH. Verify with:

```bash
stele --help
```

## Procedure

### Step 1: Check Current Configuration

Run the following to see if a config file already exists:

```bash
stele config show
```

If this shows a valid config with the correct server URL, skip to Step 3.

If no config file exists, the default profile (`local` at `http://127.0.0.1:3100`) is used automatically. This is correct for a local Stele server with default settings.

### Step 2: Create or Edit Profile

**For a local server with default settings** — no config file is needed. The defaults work out of the box.

**For a remote server or custom port**, initialize the config and edit it:

```bash
stele config init
```

This creates `~/.config/stele/config.toml`. Edit it with the server URL and (optionally) auth key:

```toml
default_profile = "local"

[profiles.local]
server_url = "http://127.0.0.1:3100"

[profiles.remote]
server_url = "https://stele.example.com:3100"
auth_key = "your-auth-key-here"
```

To switch the active profile, set `default_profile` to the desired profile name, or use the `--profile` flag / `STELE_PROFILE` env var per-invocation.

### Step 3: Verify Connection

```bash
stele status
```

This should report the server as reachable. If it fails:

- Confirm the Stele server is running
- Check the server URL in `stele config show`
- Check firewall / network access for remote servers

### Step 4: Next Steps

Tell the user:

- The MCP connection is provided by the plugin automatically — no restart needed for MCP setup
- If they changed the connection profile, they should **restart Claude Code** so the MCP proxy picks up the new settings
- Suggest running `/stele:bootstrap` to initialize the current project with Stele if it does not have a protocol section in CLAUDE.md yet
