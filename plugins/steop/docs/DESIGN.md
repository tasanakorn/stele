# Steop Design (v1)

## 1. Purpose

Steop is an agentic workflow pipeline for Claude Code. Markdown skills and subagents drive a clarify -> research -> plan -> execute -> validate loop, while a small Go runtime (`steop`) handles hook dispatch and reads/writes session state through the stele-server REST API. Safety hooks block dangerous shell commands, PostToolUse hooks bump a per-session tool-call counter, and all persistence is delegated to stele — there is no project-local state directory.

## 2. Non-goals

- No `.steop/` or `.cerbrix/` directory. Session state lives in stele-server's SQLite.
- No web HUD. Status is a REST read projection; rendering is a future CLI/TUI concern.
- No tmux team coordination.
- No feature-flag DSL or config schema language.
- No rewrite of stele itself in Go. Stele stays Rust; steop talks to it over HTTP.

## 3. Architecture

Three layers:

1. **Plugin content** (`plugins/steop/`) — markdown skills (`st-flow`, `st-clarify`, `st-research`, `st-plan`, `st-execute`, `st-validate`) and subagents (`consultant`, `researcher`, `architect`, `executor`, `reviewer`). This is the part Claude Code loads and reads; it drives the pipeline.
2. **Go runtime** (`apps/steop/`) — a single `steop` binary with subcommands `hook`, `state`, `storage`, `hud`, `version`. Installed to `~/.local/bin/steop` by `/steop:install` (or by `apps/steop/scripts/build.sh` for developers). The binary must be on the user's `PATH`; hooks invoke it as a bare `steop` command. It is the target of every hook invocation and of any CLI call the skills make.
3. **Stele server API** (`/api/v1/steop/*`) — REST endpoints on the existing stele-server process. New tables live alongside existing `memories`, `entities`, `relations`, etc. The server is the single source of truth.

```
 Claude Code
     |
     v
 hooks/hooks.json  (PreToolUse/PostToolUse)
     |
     v
 steop  (Go, on PATH — installed to ~/.local/bin)
     |   HTTP + X-Stele-Key
     v
 stele-server  (Rust / axum)
     |
     v
 SQLite  (steop_storage / steop_state / steop_status)
```

## 4. Persistence model

Three resource groups, all namespaced under `/api/v1/steop/`.

- **Storage** — generic blob KV keyed by `(scope, key)`. Used for plans, snapshots, inboxes, arbitrary workflow blobs. Value is an opaque JSON document.
- **State** — per-session state keyed by `session_id`. One row per session holding a JSON `data` object plus atomic integer `counters`. The counters map supports atomic increment and reset so hooks can bump them without racing the skill logic.
- **Status** — HUD read projection keyed by `session_id`. Never 404s: when absent it returns a defaulted row so the status readers can render without special-casing.

Endpoint table:

| Method | Path                                       | Body / Query                                            | Purpose                                          |
| ------ | ------------------------------------------ | ------------------------------------------------------- | ------------------------------------------------ |
| PUT    | /api/v1/steop/storage                      | query `?scope=&key=`, body `{"content":"..."}`          | Upsert a storage blob                            |
| GET    | /api/v1/steop/storage                      | query `?scope=&key=`                                    | Read a storage blob                              |
| DELETE | /api/v1/steop/storage                      | query `?scope=&key=`                                    | Delete a storage blob                            |
| GET    | /api/v1/steop/storage/list                 | query `?scope=`                                         | List storage keys in a scope                     |
| GET    | /api/v1/steop/state/:session_id            | —                                                       | Read session state + counters                    |
| PUT    | /api/v1/steop/state/:session_id            | body `{"data":{...},"merge":true}`                      | Upsert `data` (merge by default)                 |
| POST   | /api/v1/steop/state/:session_id/incr       | body `{"counter":"tool_calls","delta":1}`               | Atomic counter increment                         |
| POST   | /api/v1/steop/state/:session_id/reset      | body `{"counter":"loop_count","value":0}`               | Reset counter to value                           |
| DELETE | /api/v1/steop/state/:session_id            | —                                                       | Delete session state row (cascades counters)     |
| GET    | /api/v1/steop/status/:session_id           | —                                                       | Read HUD projection (never 404s)                 |

- **Notify** (`POST /api/v1/steop/notify`) — fire-and-forget native OS notification. Desktop builds render via `notify-rust`; headless builds return 501. The Stop hook calls this with `title` = `"Claude Code · <cwd basename>"` and `body` = truncated `last_assistant_message`. The call is non-blocking: the Go handler swallows errors so Claude Code always stops cleanly. Note: macOS may prompt for notification permission on first fire — this is a runtime UX quirk, not solved in v1.

## 5. Hook taxonomy

| Event            | Matcher | Purpose                                              | v1  |
| ---------------- | ------- | ---------------------------------------------------- | --- |
| PreToolUse       | Bash    | Safety regex blocks (git force push, rm -rf /, etc.) | yes |
| PostToolUse      | *       | Increment `tool_calls` counter                       | yes |
| UserPromptSubmit | -       | Keyword router (autopilot/build/plan/cancel)         | v2  |
| SessionStart     | -       | Inject recap from last snapshot                      | v2  |
| Stop/SessionEnd  | -       | Persist session snapshot to stele storage            | yes (Stop only — notify) |

v1 hook wiring lives in `plugins/steop/hooks/hooks.json`. The Go runtime reads the hook JSON event on stdin, dispatches on `hook_event_name`, and writes either `{}` (allow / no-op) or a `permissionDecision` payload to stdout.

## 6. Phase roadmap

- **v1 (now)** — foundation. Go runtime, hooks wiring, stele REST endpoints, PreToolUse deny list, PostToolUse counter, manifest + docs + CI.
- **v2** — ergonomics and resume. UserPromptSubmit routing, SessionStart recap, SessionEnd snapshot, status projection readers, skill integration for resume.
- **v3** — release surface. CLI releases, optional MCP tool wrappers around the REST endpoints, FTS over session snapshots.

## 7. Versioning

The plugin version in `plugins/steop/.claude-plugin/plugin.json` and the Go `const Version` in `apps/steop/version.go` must match. CI enforces this.

The REST contract under `/api/v1/steop/*` is frozen at v1. Additive changes (new fields, new endpoints) are allowed. Any breaking change requires a new `/api/v2/steop/*` prefix; v1 must keep working.

## 8. Verifying v1 (smoke tests)

Against a running stele-server on `http://127.0.0.1:3100` with auth key `$KEY`:

```bash
# --- storage ---
curl -sS -X PUT "http://127.0.0.1:3100/api/v1/steop/storage?scope=demo&key=plan" \
  -H "X-Stele-Key: $KEY" -H 'Content-Type: application/json' \
  -d '{"content":"{\"title\":\"first plan\",\"steps\":[1,2,3]}"}'

curl -sS "http://127.0.0.1:3100/api/v1/steop/storage?scope=demo&key=plan" \
  -H "X-Stele-Key: $KEY"

curl -sS "http://127.0.0.1:3100/api/v1/steop/storage/list?scope=demo" \
  -H "X-Stele-Key: $KEY"

curl -sS -X DELETE "http://127.0.0.1:3100/api/v1/steop/storage?scope=demo&key=plan" \
  -H "X-Stele-Key: $KEY"

# --- state ---
curl -sS -X PUT http://127.0.0.1:3100/api/v1/steop/state/sess-1 \
  -H "X-Stele-Key: $KEY" -H 'Content-Type: application/json' \
  -d '{"data":{"phase":"plan"},"merge":true}'

curl -sS http://127.0.0.1:3100/api/v1/steop/state/sess-1 \
  -H "X-Stele-Key: $KEY"

curl -sS -X POST http://127.0.0.1:3100/api/v1/steop/state/sess-1/incr \
  -H "X-Stele-Key: $KEY" -H 'Content-Type: application/json' \
  -d '{"counter":"tool_calls","delta":1}'

curl -sS -X POST http://127.0.0.1:3100/api/v1/steop/state/sess-1/reset \
  -H "X-Stele-Key: $KEY" -H 'Content-Type: application/json' \
  -d '{"counter":"tool_calls","value":0}'

curl -sS -X DELETE http://127.0.0.1:3100/api/v1/steop/state/sess-1 \
  -H "X-Stele-Key: $KEY"

# --- status ---
curl -sS http://127.0.0.1:3100/api/v1/steop/status/sess-1 \
  -H "X-Stele-Key: $KEY"
```

Hook smoke tests (local, no server required):

```bash
echo '{"hook_event_name":"PreToolUse","tool_name":"Bash","tool_input":{"command":"git push --force origin main"}}' \
  | steop hook PreToolUse
# => {"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny",...}}

echo '{"hook_event_name":"PreToolUse","tool_name":"Bash","tool_input":{"command":"ls -la"}}' \
  | steop hook PreToolUse
# => {}
```

## 9. Known limitations

- No server-side auth enforcement yet on steop endpoints beyond whatever the existing stele auth middleware provides.
- No migrations subsystem. Schema changes are additive-only and guarded by `CREATE TABLE IF NOT EXISTS`.
- The stele-server uses a shared tokio mutex around its SQLite connection, so all DB access (including steop) is serialized. This is fine for workflow-scale traffic but would need revisiting for high concurrency.
- The `steop` binary must be rebuilt with `apps/steop/scripts/build.sh` after every Go source change. There is no auto-rebuild on install.
- Status projection has no background materializer yet; it computes on read.
