# Stele Plugin for Claude Code

Shared team memory for Claude Code. This plugin provides skills and a subagent for working with a [Stele](https://github.com/tasanakorn/stele) server.

## Prerequisites

A Stele server must be running and accessible. See the [main README](../README.md) for installation options.

## Installation

Add the marketplace in Claude Code, then install the stele plugin:

```
/plugin → Discover → Marketplaces → Add marketplace → tasanakorn/stele
/plugin → Discover → stele → Install
/reload-plugins
```

## Getting Started

The MCP connection is automatically registered when the plugin is installed — no manual MCP configuration needed.

If connecting to a remote server, run `/stele:install` to configure your connection profile.

Then bootstrap your project:

```
/stele:bootstrap
```

## Skills

| Skill      | Command            | Description                                                          |
| ---------- | ------------------ | -------------------------------------------------------------------- |
| Install    | `/stele:install`   | Configure Stele connection profile for your server                   |
| Bootstrap  | `/stele:bootstrap` | Initialize a project — create scope, seed entities, generate CLAUDE.md |
| Sync       | `/stele:sync`      | Pull latest shared team context into the current session             |
| Checkpoint | `/stele:checkpoint`| Save session findings, decisions, and discoveries back to Stele      |

## Agent

| Agent           | Description                                                                                |
| --------------- | ------------------------------------------------------------------------------------------ |
| stele-librarian | Read-only retrieval subagent for searching memories and graph nodes. Uses Sonnet for fast lookups. |

## License

MIT
