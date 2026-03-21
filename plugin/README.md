# Stele Plugin for Claude Code

Shared team memory for Claude Code. This plugin connects Claude Code to a running [Stele](https://github.com/tasanakorn/stele) server, providing skills for bootstrapping projects, syncing team context, and checkpointing session findings.

## Prerequisites

A Stele server must be running and accessible. See the [main README](../README.md) for installation options.

Default server address: `http://127.0.0.1:3100/mcp`

## Installation

### From Marketplace

```bash
claude plugin add stele
```

### Manual

Clone the repository and install the plugin from the `plugin/` directory:

```bash
git clone https://github.com/tasanakorn/stele.git
claude plugin install ./stele/plugin
```

## What You Get

### MCP Connection

The plugin auto-configures a connection to the Stele MCP server at `localhost:3100`. All 17 Stele MCP tools become available in your Claude Code sessions.

### Skills

| Skill         | Command              | Description                                                                            |
| ------------- | -------------------- | -------------------------------------------------------------------------------------- |
| Install       | `/stele:install`     | Check Stele MCP connection and help configure it at user or project level              |
| Bootstrap     | `/stele:bootstrap`   | Initialize a project with Stele — creates scope, seeds entities, generates CLAUDE.md   |
| Sync          | `/stele:sync`        | Pull latest shared team context (flat memories + knowledge graph) into current session |
| Checkpoint    | `/stele:checkpoint`  | Save session findings, decisions, and discoveries back to Stele                        |

### Agent

| Agent            | Description                                                     |
| ---------------- | --------------------------------------------------------------- |
| stele-librarian  | Read-only retrieval subagent for searching memories and graph nodes. Uses Sonnet for fast, cost-effective lookups. |

## Configuration

The plugin connects to Stele at `http://127.0.0.1:3100/mcp` by default. To use a different address, edit the `.mcp.json` file in the plugin directory or override it in your project's `.mcp.json`.

## License

MIT
