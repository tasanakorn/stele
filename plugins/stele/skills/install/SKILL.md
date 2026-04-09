---
name: install
description: Configure Stele MCP connection at user or project level
user-invocable: true
---

# Install Stele MCP Connection

Configure the Stele MCP server connection so Claude Code can access shared team memory.

## Procedure

### Step 1: Check Existing Configuration

Check if Stele MCP is already configured:

1. **Project-level:** Look for a `stele` entry in `.mcp.json` in the project root
2. **User-level:** Look for a `stele` entry in `~/.claude/settings.json` under `mcpServers`

If already configured at either level, report the current URL and scope. Ask the user if they want to reconfigure or keep the current setup. If keeping, skip to Step 4.

### Step 2: Ask for Server URL

Ask the user for the Stele server URL (the base URL, not the `/mcp` path).

- Default: `http://127.0.0.1:3100`
- For remote servers: `http://<host>:<port>`

The `stele mcp` CLI proxy will append `/mcp` automatically.

### Step 3: Ask for Scope and Write Config

Ask the user where to install the MCP connection:

**User scope** — available in all projects, good for personal machines:

```bash
# Local server (default URL)
claude mcp add --scope user stele -- stele mcp

# Remote server
claude mcp add --scope user stele -- stele --server-url <url> mcp
```

**Project scope** — shared via version control, good for team projects. Write `.mcp.json` in the project root:

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

For a remote server:

```json
{
  "mcpServers": {
    "stele": {
      "command": "stele",
      "args": ["--server-url", "<url>", "mcp"]
    }
  }
}
```

If `.mcp.json` already exists with other servers, merge the `stele` entry — do not overwrite other entries.

**Alternative (direct HTTP, no CLI needed):** If the `stele` CLI is not available, use Streamable HTTP:

```json
{
  "mcpServers": {
    "stele": {
      "type": "http",
      "url": "<url>/mcp"
    }
  }
}
```

### Step 4: Inform User

Tell the user:
- Where the config was written (user-level or project-level)
- They need to **restart Claude Code** for the MCP connection to take effect
- After restart, all 17 Stele MCP tools will be available
- Suggest running `/stele:bootstrap` to set up the current project if it doesn't have a Stele protocol section in CLAUDE.md yet
