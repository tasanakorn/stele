# Stele Plugin for Claude Code

Shared team memory for Claude Code. This plugin provides skills and a subagent for working with a [Stele](https://github.com/tasanakorn/stele) server.

## Prerequisites

A Stele server must be running and accessible. See the [main README](../README.md) for installation options.

## Installation

### From Marketplace

```bash
claude plugin add tasanakorn/stele
```

### Manual

```bash
git clone https://github.com/tasanakorn/stele.git
claude plugin install ./stele/plugin
```

## Getting Started

After installing the plugin, run `/stele:install` to configure the MCP connection:

```
/stele:install
```

This will ask for your Stele server URL and where to install the config (user-level or project-level). Restart Claude Code after setup to activate.

Then bootstrap your project:

```
/stele:bootstrap
```

## Skills

| Skill      | Command            | Description                                                          |
| ---------- | ------------------ | -------------------------------------------------------------------- |
| Install    | `/stele:install`   | Configure Stele MCP connection at user or project level              |
| Bootstrap  | `/stele:bootstrap` | Initialize a project — create scope, seed entities, generate CLAUDE.md |
| Sync       | `/stele:sync`      | Pull latest shared team context into the current session             |
| Checkpoint | `/stele:checkpoint`| Save session findings, decisions, and discoveries back to Stele      |

## Agent

| Agent           | Description                                                                                |
| --------------- | ------------------------------------------------------------------------------------------ |
| stele-librarian | Read-only retrieval subagent for searching memories and graph nodes. Uses Sonnet for fast lookups. |

## License

MIT
