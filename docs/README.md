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
| [claude-code-integration.md](claude-code-integration.md)                            | Claude Code env vars, hook JSON, statusLine stdin payloads            |

## Product Requirements (`prd/`)

Forward-looking design docs. Filename convention: `prd-NNN-<slug>.md`. Numbers are allocated sequentially and are permanent once assigned (even after the PRD is implemented or superseded).

| PRD                                                                             | Status              | Description                                                                                       |
| ------------------------------------------------------------------------------- | ------------------- | ------------------------------------------------------------------------------------------------- |
| [prd-001-mailbox-v2](prd/prd-001-mailbox-v2.md)                                 | Implemented v0.8.0  | `steop_mailbox` table + `steop.mailbox.*` RPC                                                     |
| [prd-003-identity-injection](prd/prd-003-identity-injection.md)                 | Implemented v0.9.0  | PreToolUse identity injection + multi-session statusline                                          |
| [prd-004-st-prd-skill](prd/prd-004-st-prd-skill.md)                             | Implemented v0.9.1  | `/steop:st-prd` skill for convention-enforced PRD authoring                                       |
| [prd-005-storage-session-fallback](prd/prd-005-storage-session-fallback.md)     | Implemented v0.9.3  | `steop storage` session fallback + st-watch cleanup                                               |
| [prd-006-st-send-smart-addressing](prd/prd-006-st-send-smart-addressing.md)     | Implemented v0.10.0 | `/steop:st-send` skill with short-name resolution + mode-aware task routing                       |
| [prd-007-st-send-session-resolve](prd/prd-007-st-send-session-resolve.md)       | Implemented v0.10.1 | Fix `st-send` to resolve to active session UUID instead of hardcoded `USER`                       |
| [prd-008-watcher-lifecycle](prd/prd-008-watcher-lifecycle.md)                   | Implemented v0.11.0 | Watcher lifecycle state + heartbeat for cross-session liveness detection                          |
| [prd-009-st-watch-streamline](prd/prd-009-st-watch-streamline.md)               | Implemented v0.12.0 | Streamline st-watch startup: CLI auto-resume + single-turn monitoring entry                       |
| [prd-010-st-watch-fast-startup](prd/prd-010-st-watch-fast-startup.md)           | Implemented v0.12.1 | Fast startup: ready line, parallel RPCs, SKILL.md trimming for <1 min monitoring                  |
| [prd-011-watcher-claim-based-cursor](prd/prd-011-watcher-claim-based-cursor.md) | Implemented v0.12.3 | Drop persistent `watcher:last_message_id` cursor; rely on server-side status=NEW + in-memory seen |
| [prd-012-watcher-dual-mailbox](prd/prd-012-watcher-dual-mailbox.md)             | Implemented v0.12.4 | Watcher polls both 2-segment project and 3-segment session mailboxes; per-tick seen dedup         |
| [prd-013-watcher-meta-event-format](prd/prd-013-watcher-meta-event-format.md)   | Proposed            | Make st-watch work end-to-end: explicit per-message_type conditions, restore WATCHER:READY line    |
| [prd-014-mailbox-watch-flag-parsing](prd/prd-014-mailbox-watch-flag-parsing.md) | Implemented v0.13.0 | mailbox watch parsing + emission throttle: `mailbox.update_meta` RPC + one-in-flight gate via `meta.task_status=DONE` ack |
| [prd-015-hook-inject-position](prd/prd-015-hook-inject-position.md)             | Implemented v0.12.6 | Move PreToolUse identity flags to directly after the `steop` token so redirections don't hide them          |
| [prd-016-hook-scope-public-flags](prd/prd-016-hook-scope-public-flags.md)       | Implemented v0.13.1 | Restrict PreToolUse hook to Bash; add public --session-id / --project-dir flags + steop identity command.                 |
| [prd-017-st-watch-monitor-identity-wiring](prd/prd-017-st-watch-monitor-identity-wiring.md) | Implemented v0.13.3 | Wire /steop:st-watch to resolve identity via `steop identity` and embed public flags in Monitor command (PRD-016 consumer). |
| [prd-018-st-xp-skill](prd/prd-018-st-xp-skill.md)                               | Implemented v0.14.0 | XP-style fast-feedback workflow skill (renamed to /steop:st-lite in v0.19.0 — see PRD-024).        |
| [prd-019-stylos-foundation](prd/prd-019-stylos-foundation.md)                   | Proposed            | zenoh-based interconnect foundation — apps/stylos/ skeleton, ports 31746/31747, Rust+Go POC       |
| [prd-020-steop-local-backend](prd/prd-020-steop-local-backend.md)               | Implemented v0.16.0 | Move steop session/project/phase/storage/logs to local SQLite; keep mailbox+notify on stele       |
| [prd-021-trim-hook-mailbox-posts](prd/prd-021-trim-hook-mailbox-posts.md)       | Implemented v0.16.1 | Stop HandleStop/HandleSessionEnd from posting HOOK:* mailbox rows; confine writes to task pipeline |
| [prd-022-stylos-in-stele-server](prd/prd-022-stylos-in-stele-server.md)         | Implemented v0.17.0 | Embed stylos zenoh router inside stele-server with default-on feature, health endpoint, heartbeat  |
| [prd-023-stylos-default-udp](prd/prd-023-stylos-default-udp.md)                  | Implemented v0.18.0 | Drop QUIC from stylos; default data plane to UDP+TCP on 31747; remove no-quic flag + TLS config     |
| [prd-024-rename-st-xp-to-st-lite](prd/prd-024-rename-st-xp-to-st-lite.md)        | Proposed            | Rename /steop:st-xp skill to /steop:st-lite to avoid Agile XP methodology confusion.                |

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
| [DESIGN.md](steop/DESIGN.md)                                    | Current design blueprint (v2, 0.16.0+)                     |
| [smoke-tests.md](steop/smoke-tests.md)                          | Curl sequences for stele-backed surface (mailbox, notify)  |
| [gap-analysis.md](steop/gap-analysis.md)                        | steop vs cerbrix vs omc — feature comparison snapshot      |
| [hook-gap.md](steop/hook-gap.md)                                | Deep dive on hook-event coverage (companion to gap doc)    |
| [idle-detection.md](steop/idle-detection.md)                    | Detecting true Claude Code idle (signals + composite recipe) |
| [cerbrix-gap-analysis.md](steop/cerbrix-gap-analysis.md)        | cerbrix feature catalog + planning ledger                  |
| [local-storage.md](steop/local-storage.md)                      | Local SQLite backend for session/project/phase/storage/logs |

## Stylos foundation (`stylos/`)

zenoh-based interconnect foundation. Spec and docs are still forthcoming; see [PRD-019](prd/prd-019-stylos-foundation.md) for the design intent and scope.

| Doc                                         | Description                                                |
| ------------------------------------------- | ---------------------------------------------------------- |
| [README.md](stylos/README.md)               | Entry point and orientation for the Stylos subtree         |
| [architecture.md](stylos/architecture.md)   | Peer topology, UDP+TCP transport, crate/module layout      |
| [addressing.md](stylos/addressing.md)       | `stylos/<realm>/<role>/<instance>` key-expr grammar        |
| [discovery.md](stylos/discovery.md)         | Multicast scouting, gossip, non-multicast network caveats  |
| [poc.md](stylos/poc.md)                     | Rust ↔ Go pub/sub/get/queryable POC spec and acceptance    |
| [cross-lang.md](stylos/cross-lang.md)       | Rust/Go/TS/Python binding status and caveats               |
