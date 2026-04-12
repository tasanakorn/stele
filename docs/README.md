# Stele Documentation

Single source of truth for all Stele specs. The layout follows a simple rule:

- **`docs/*.md`** — cross-module specs that apply to the whole workspace.
- **`docs/prd/`** — product requirements docs (forward-looking, numbered).
- **`docs/<module>/`** — module-specific deep detail (SQL, structs, hooks, etc.).

If a doc mentions specific SQL tables, struct field names, or hook handlers, it belongs in a module subfolder. If it describes the workspace as a whole or a forward-looking design, it lives at the `docs/` root (or `docs/prd/`).

## Workspace-wide

| Doc                                                                                 | Description                                                           |
| ----------------------------------------------------------------------------------- | --------------------------------------------------------------------- |
| [architecture.md](architecture.md)                                                  | Top-level map — components, how they talk, key design choices         |
| [versioning.md](versioning.md)                                                      | SemVer policy and the `scripts/bump-version.py` workflow              |
| [plugin-marketplace-troubleshooting.md](plugin-marketplace-troubleshooting.md)      | Recovering from stale Claude Code marketplace state when re-adding    |

## Product Requirements (`prd/`)

Forward-looking design docs. Filename convention: `prd-NNN-<slug>.md`. Numbers are allocated sequentially and are permanent once assigned (even after the PRD is implemented or superseded).

| PRD                                                        | Status              | Description                                  |
| ---------------------------------------------------------- | ------------------- | -------------------------------------------- |
| [prd-001-mailbox-v2](prd/prd-001-mailbox-v2.md)            | Implemented v0.8.0  | `steop_mailbox` table + `steop.mailbox.*` RPC |

## Stele server (`stele/`)

Shared-memory server, REST API, MCP tool surface, and CLI.

| Doc                                          | Description                                          |
| -------------------------------------------- | ---------------------------------------------------- |
| [server.md](stele/server.md)                 | Server internals — entry points, crates, rmcp wiring |
| [data-model.md](stele/data-model.md)         | SQLite schema, FTS5, scopes, tags, graph tables      |
| [mcp-tools.md](stele/mcp-tools.md)           | MCP tool reference (flat memory + knowledge graph)   |
| [http-api.md](stele/http-api.md)             | REST API reference under `/api/v1`                   |
| [cli.md](stele/cli.md)                       | `stele` CLI subcommands and config profiles          |
| [deployment.md](stele/deployment.md)         | Build profiles, Docker, macOS `.app` bundle          |
| [testing.md](stele/testing.md)               | End-to-end integration test strategy                 |

## Steop workflow (`steop/`)

Agentic workflow pipeline plugin plus its `steop` companion binary.

| Doc                                                             | Description                                                |
| --------------------------------------------------------------- | ---------------------------------------------------------- |
| [DESIGN.md](steop/DESIGN.md)                                    | Current design blueprint (v2, 0.7.0+)                      |
| [smoke-tests.md](steop/smoke-tests.md)                          | Copy-paste curl sequence exercising every `steop.*` RPC    |
| [gap-analysis.md](steop/gap-analysis.md)                        | steop vs cerbrix vs omc — feature comparison snapshot      |
| [hook-gap.md](steop/hook-gap.md)                                | Deep dive on hook-event coverage (companion to gap doc)    |
| [cerbrix-gap-analysis.md](steop/cerbrix-gap-analysis.md)        | cerbrix feature catalog + planning ledger                  |
