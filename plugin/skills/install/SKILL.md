---
name: install
description: Check Stele MCP connection and help configure it at user or project level
user-invocable: true
---

# Install / Configure Stele MCP Connection

Check whether the Stele MCP server is configured and reachable, and help set it up if not.

## Procedure

### Step 1: Check Existing Configuration

Look for a `stele` MCP entry in:

1. **Project-level:** `.mcp.json` in the project root
2. **User-level:** `~/.claude/settings.json` under `mcpServers`

Report what was found. If Stele is already configured at either level, note the URL and move to Step 3 (connectivity check).

### Step 2: Configure MCP Connection

If Stele is not configured, ask the user:

- **Server address** — default `http://127.0.0.1:3100/mcp`, or a custom address
- **Scope** — where to install:
  - **User level** — available in all projects, good for personal machines. Run: `claude mcp add --scope user stele --transport http <url>`
  - **Project level** — shared via version control, good for team projects. Write to `.mcp.json` in the project root.

For project-level, create or update `.mcp.json`. If it already exists with other servers, merge the `stele` entry — do not overwrite other entries.

```json
{
  "mcpServers": {
    "stele": {
      "type": "http",
      "url": "http://127.0.0.1:3100/mcp"
    }
  }
}
```

### Step 3: Check Connectivity

Try calling `list_scopes(scope: "global")` to verify the Stele server is reachable.

- **If successful:** Report the server is running and connected. Show the number of scopes/memories found.
- **If it fails:** The server is not reachable. Tell the user how to start it:
  - **macOS (desktop):** Download from GitHub Releases or `cargo build --release && ./target/release/stele`
  - **Headless (Linux/Docker):** `cargo build --release --features headless --no-default-features && ./target/release/stele`
  - **Docker:** `docker run -d -p 3100:3100 -v stele-data:/data ghcr.io/tasanakorn/stele`

### Step 4: Summary

Report:
- MCP configuration status (user-level, project-level, or not configured)
- Server connectivity (reachable or not)
- Next steps if anything needs attention
