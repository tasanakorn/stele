# Steop Design (v1)

## 1. Purpose

Steop is an agentic workflow pipeline for Claude Code. Markdown skills and subagents drive a clarify -> research -> plan -> execute -> validate loop, while a small Go runtime (`steop`) handles hook dispatch and reads/writes session state through the stele-server REST API. Safety hooks block dangerous shell commands, PostToolUse hooks bump a per-session tool-call counter, and all persistence is delegated to stele — there is no project-local state directory.

## 2. Non-goals

- No `.steop/` or `.cerbrix/` directory. Session state lives in stele-server's SQLite.
- No web HUD and no standalone TUI panel. Live progress surfaces through Claude Code's native `statusLine` setting as a two-line display: both lines are rendered by the `steop statusline` subcommand, which reads Claude Code's stdin JSON directly. Line 1 shows model / project / git branch / context bar / rate limits or cost; line 2 shows the steop pipeline state. `/steop:statusline-setup` patches `~/.claude/settings.json` to point `statusLine` directly at `steop statusline` — no shell script, no `jq` prerequisite.
- No tmux team coordination.
- No feature-flag DSL or config schema language.
- No rewrite of stele itself in Go. Stele stays Rust; steop talks to it over HTTP.

## 3. Architecture

Three layers:

1. **Plugin content** (`plugins/steop/`) — markdown skills (`st-flow`, `st-clarify`, `st-research`, `st-plan`, `st-execute`, `st-validate`) and subagents (`consultant`, `researcher`, `architect`, `executor`, `reviewer`). This is the part Claude Code loads and reads; it drives the pipeline.
2. **Go runtime** (`apps/steop/`) — a single `steop` binary with subcommands `hook`, `state`, `storage`, `statusline`, `monitor`, `version`. Installed to `~/.local/bin/steop` by `/steop:install` (or by `apps/steop/scripts/build.sh` for developers). The binary must be on the user's `PATH`; hooks invoke it as a bare `steop` command, and Claude Code invokes `steop statusline` every couple of seconds if `/steop:statusline-setup` has been run. It is the target of every hook invocation and of any CLI call the skills make.
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
- **Status** — statusline read projection keyed by `session_id`. Never 404s: when absent it returns a defaulted row so the status readers can render without special-casing.

Four resource groups in v0.5.0:

- **Storage** — generic blob KV keyed by `(scope, key)`. Plans, snapshots, arbitrary workflow blobs.
- **State** — per-session state keyed by `session_id`. One row per session with JSON `data` + atomic integer `counters`.
- **Status** — statusline read projection keyed by `session_id`. Never 404s.
- **Log / Inbox (v0.5.0+)** — append-only structured event log (`steop_logs`) and session-summary FIFO queue (`steop_inbox`). Every row carries `host`, `session_id`, `project_dir`, timestamp.

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
| GET    | /api/v1/steop/status/:session_id           | —                                                       | Read statusline projection (never 404s)         |
| POST   | /api/v1/steop/log                          | body `{session_id, event, data, host?, project_dir?}`   | Append structured log event (v0.5.0+)            |
| GET    | /api/v1/steop/log                          | query `?session_id=&host=&project_dir=&limit=`          | Query logs (DESC by created_at, default 200)     |
| POST   | /api/v1/steop/inbox                        | body `{session_id, payload, host?, project_dir?}`       | Append session summary envelope (v0.5.0+)        |
| GET    | /api/v1/steop/inbox                        | query `?session_id=&host=&project_dir=&limit=`          | Read inbox FIFO (ASC by created_at)              |

- **Notify** (`POST /api/v1/steop/notify`) — fire-and-forget native OS notification. Desktop builds render via `notify-rust`; headless builds return 501. The Stop hook calls this with `title` = `"Claude Code · <cwd basename>"` and `body` = truncated `last_assistant_message`. The call is non-blocking: the Go handler swallows errors so Claude Code always stops cleanly. Note: macOS may prompt for notification permission on first fire — this is a runtime UX quirk, not solved in v1.

### Log + Inbox semantics (v0.5.0+)

Both facilities are **append-only**. v0.5.0 has no DELETE or TTL — rows accumulate until the user prunes manually. This matches the intended use cases:

- **Logs** are operational breadcrumbs — hook events, phase transitions, subagent lifecycle. Query by `session_id` (most common), filtered by `host` / `project_dir` when debugging cross-machine. Keep the last ~200 per session is the default limit. Body precedence: request body fields win over `X-Steop-Host` / `X-Steop-Project-Dir` header fallbacks.
- **Inbox** is a session-summary queue. On `Stop` and `SessionEnd`, the `steop` binary posts a payload containing final `data` + `counters` + any workflow metadata. A future reader (not yet implemented) would consume these to populate `/stele:sync` with "what did my last session do". FIFO ordering is enforced at query time via `ORDER BY created_at ASC`.

### Composite session identity (v0.5.0+)

Problem: `session_id` alone is not unique across machines. Two Claude Code instances on different hosts can generate the same session ID (UUIDs are unique in practice but the guarantee doesn't bind stele). Worse, the same session ID can appear for different projects on the same machine if the user switches workspaces.

Solution: **every** outbound HTTP request from **every** stele client carries two additional headers. No matter the transport (REST API, MCP proxy) and no matter the route (`/api/v1/memories`, `/api/v1/graph/*`, `/api/v1/steop/*`, `/mcp`), the identifying headers are present:

- `X-Steop-Host` — the originating machine.
- `X-Steop-Project-Dir` — the workspace directory.

#### Client injection

Both headers are populated at client construction time, not per-request, so zero code paths can bypass them:

| Client                         | Host source                                                                                      | Project dir source                                                          | Applied at                                            |
| ------------------------------ | ------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------- | ----------------------------------------------------- |
| Go steop (`apps/steop/`)       | `Profile.host` from `~/.config/stele/config.toml`; fallback `STELE_HOST` env; fallback `os.Hostname()`. Auto-backfilled into config on first load. | `CLAUDE_PROJECT_DIR` env; fallback `PWD`; fallback `os.Getwd()`.            | `client.New()` → `do()` sets headers unconditionally. |
| Rust stele CLI (`stele-cli/`)  | `Profile.host` from config; fallback `STELE_HOST` env; fallback `gethostname::gethostname()`. Auto-backfilled into config on first load. | `CLAUDE_PROJECT_DIR` env; fallback `PWD`.                                   | `SteleClient::apply_headers()` on every get/post/put/delete. |
| MCP proxy (`mcp_proxy.rs`)     | Same as Rust CLI.                                                                                 | Same as Rust CLI.                                                           | `forward_request`, `forward_request_capture`, shutdown DELETE. |

Hook handlers in the Go steop binary additionally override `project_dir` per-call via `client.WithRequestContext("", in.Cwd)` using the hook stdin payload — the hook-reported `cwd` is more authoritative than the binary's own working directory when running as a hook subprocess.

Header values are ASCII-graphic sanitized (`/` kept for paths, control chars and spaces stripped) so they are always valid HTTP header values. The `sanitizeHeader()` helper in Go matches the Rust `SteleClient::sanitize()` behavior byte-for-byte.

#### Server persistence

The server honors the headers on every steop write endpoint, not just log/inbox:

| Endpoint                                    | Provenance persistence                                            |
| ------------------------------------------- | ----------------------------------------------------------------- |
| `PUT /api/v1/steop/state/:id`               | Writes `host`/`project_dir` into the row. Blank headers don't clobber pre-existing values. |
| `POST /api/v1/steop/state/:id/incr`         | Calls `ensure_state_row` which backfills blank columns on the first counter bump. |
| `POST /api/v1/steop/state/:id/reset`        | Same as `incr`.                                                   |
| `POST /api/v1/steop/log`                    | Persists into `steop_logs` rows; body fields override headers.    |
| `POST /api/v1/steop/inbox`                  | Persists into `steop_inbox` rows; body fields override headers.   |
| `GET`/search routes on memories/graph       | Not persisted (no composite columns on memory tables in v0.5.0). Headers are still sent and available to future analytics/audit layers. |

Schema change: the `steop_state` table gained `host TEXT NOT NULL DEFAULT ''` and `project_dir TEXT NOT NULL DEFAULT ''` columns via an idempotent `ALTER TABLE` guarded by `pragma_table_info('steop_state')` in `ensure_steop_schema()`. New rows populate from incoming headers. Pre-existing rows keep their defaults (`''`) until their next PUT or counter mutation, at which point `ensure_state_row` fills the blank columns without disturbing any non-blank values already there.

The primary key remains `session_id` alone in v0.5.0 — true composite-PK uniqueness is a v2 migration. The columns are provenance metadata today, not part of the identity.

Headers are optional: the server tolerates their absence and defaults to empty strings. Clients that don't set them behave identically to v0.4.x, and the server does not clobber previously stored provenance with blank values from such clients.

## 5. Hook taxonomy

All 11 Claude Code hook events are wired in v0.5.0. Dispatcher at `apps/steop/cmd_hook.go` routes each event to a handler under `apps/steop/internal/hooks/`. Handlers read `HookInput` from stdin, emit one of `Allow()`, `DenyPreToolUse(reason)`, or `InjectUserPromptContext(text)` to stdout, and always exit 0.

| Event                 | Matcher | Handler                           | v0.5.0 behavior                                                                                      |
| --------------------- | ------- | --------------------------------- | ---------------------------------------------------------------------------------------------------- |
| `SessionStart`        | `*`     | `session_start.go`                | Log `{cwd, permission_mode}` to `/api/v1/steop/log`                                                  |
| `UserPromptSubmit`    | `*`     | `user_prompt_submit.go`           | Write session sentinel; on `st-<phase>:` / `/steop:st-<phase>` triggers, inject matching SKILL.md    |
| `PreToolUse`          | `Bash`  | `pre_tool_use.go`                 | Deny force-push, `rm -rf /`, `rm -rf ~/`, `rm -rf $HOME` via regex                                   |
| `PermissionRequest`   | `*`     | `permission_request.go`           | Observe-only stub (returns `Allow()`)                                                                |
| `PostToolUse`         | `*`     | `post_tool_use.go`                | Increment `tool_calls` counter + merge `{last_tool, last_tool_at}` into state + log event            |
| `PostToolUseFailure`  | `*`     | `post_tool_use_failure.go`        | Log `{tool_name, error, is_interrupt}`                                                               |
| `SubagentStart`       | `*`     | `subagent_start.go`               | Log `{agent_id, agent_type, model, prompt[:500]}`                                                    |
| `SubagentStop`        | `*`     | `subagent_stop.go`                | Log `{agent_id, agent_type, output[:500], success}`                                                  |
| `PreCompact`          | `*`     | `pre_compact.go`                  | Log `{cwd, trigger}`                                                                                 |
| `Stop`                | `*`     | `stop.go`                         | Desktop notify + fetch state + post inbox summary + log persistent_mode flag + clear phase/mode      |
| `SessionEnd`          | `*`     | `session_end.go`                  | Log `{cwd, reason, transcript_path}` + post inbox summary with final `data` + `counters`             |

Hook manifest lives at `plugins/steop/hooks/hooks.json`. Per-event timeouts: 3s for `PostToolUse` (hot path), 10s for `Stop`, 30s for `SessionEnd`, 5s for the rest. All HTTP calls from the handlers use a 500ms `fastClone()` timeout so a dead stele-server never stalls Claude Code beyond 500ms per event.

### Keyword injection in UserPromptSubmit

Opt-in explicit triggers only. No implicit auto-routing — the gap-analysis non-goal applies.

| Trigger regex                              | Skill injected    |
| ------------------------------------------ | ----------------- |
| `^/?(steop:)?st-flow\b` or `^flow:`        | `st-flow`         |
| `^/?(steop:)?st-clarify\b` or `^clarify:`  | `st-clarify`      |
| `^/?(steop:)?st-research\b` or `^research:`| `st-research`     |
| `^/?(steop:)?st-plan\b` or `^plan:`        | `st-plan`         |
| `^/?(steop:)?st-execute\b` or `^execute:`  | `st-execute`      |
| `^/?(steop:)?st-validate\b` or `^validate:`| `st-validate`     |

Skill body loaded from `$CLAUDE_PLUGIN_ROOT/skills/<name>/SKILL.md`. Missing env var or file → fall through to `Allow()`. Output shape: `{"hookSpecificOutput":{"hookEventName":"UserPromptSubmit","additionalContext":"<full SKILL.md contents>"}}`.

## 6. Phase roadmap

- **v0.4 (done)** — foundation. Go runtime, 4 hook events, REST endpoints for storage/state/status/notify, PreToolUse deny list, PostToolUse counter, manifest + docs + CI. Statusline rendered in-process (v0.4.12).
- **v0.5 (current)** — full hook surface + composite identity. All 11 Claude Code events wired, `log` + `inbox` REST endpoints, `host` + `project_dir` composite session identity with `X-Steop-*` headers, keyword injection on UserPromptSubmit, hostname auto-detected on first config load. Persistent-mode flag read-only.
- **v0.6** — consumers. `/stele:sync` reads inbox to surface past-session summaries. `steop recap` skill. Deliverable verification heuristics on `SubagentStop`. Compact-rescue memory tagging on `PreCompact`.
- **v0.7** — persistent-mode honored. Stop hook returns `{"decision":"block","reason":"..."}` when `persistent_mode` flag is set, with safety guards against infinite loops.
- **v1.0** — release surface. Prebuilt binaries, optional MCP tool wrappers around the REST endpoints, FTS over log payloads.

## 7. Versioning

The plugin version in `plugins/steop/.claude-plugin/plugin.json` and the Go `const Version` in `apps/steop/version.go` must match. CI enforces this. Together they are the **single source of truth** for the human-facing steop version — what `steop version` prints, what the plugin marketplace displays.

The REST contract under `/api/v1/steop/*` is frozen at v1. Additive changes (new fields, new endpoints) are allowed. Any breaking change requires a new `/api/v2/steop/*` prefix; v1 must keep working.

**No separate Go module tags.** `apps/steop/` is a subdirectory Go module inside this monorepo. Go's proxy expects such modules to be tagged with a subdirectory prefix (`apps/steop/vX.Y.Z`), which would fork the tag namespace and create churn every time steop moves independently of stele-server. We deliberately do **not** maintain that parallel tag stream.

Instead, `/steop:install` installs from the `main` branch tip:

```bash
go install github.com/tasanakorn/stele/apps/steop@main
```

This records a Go pseudo-version (e.g. `v0.0.0-20260410133000-abc123def456`) in the binary's build metadata — visible via `go version -m $(which steop)` — while `steop version` still prints the human-facing version from `version.go`. Pseudo-versions are the commit the user actually got; the const is the release they think they got. In practice the two should agree because `main` is the only shipping branch.

If we ever need independent release tagging for steop (for reproducible pins, a plugin marketplace that wants semver, or a detached release cadence from stele-server), options in order of preference:

1. **Publish prebuilt binaries via GitHub Releases** and rewrite `/steop:install` to download them. No Go toolchain required on the user's machine, no tag pollution, reproducible artefacts.
2. **Adopt `apps/steop/vX.Y.Z` tags** as Go's native multi-module convention. Cheap per-release but doubles the tag namespace.
3. **Split `apps/steop/` into its own repository.** Clean but loses the monorepo coupling with `stele-server`.

Until any of those become necessary, `@main` + the version const is deliberately enough.

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

# --- log (v0.5.0+) ---
curl -sS -X POST http://127.0.0.1:3100/api/v1/steop/log \
  -H "X-Stele-Key: $KEY" -H 'Content-Type: application/json' \
  -H 'X-Steop-Host: laptop-a' -H 'X-Steop-Project-Dir: /tmp/proj' \
  -d '{"session_id":"sess-1","event":"post_tool_use","data":{"tool_name":"Bash"}}'

curl -sS "http://127.0.0.1:3100/api/v1/steop/log?session_id=sess-1&limit=20" \
  -H "X-Stele-Key: $KEY"

# --- inbox (v0.5.0+) ---
curl -sS -X POST http://127.0.0.1:3100/api/v1/steop/inbox \
  -H "X-Stele-Key: $KEY" -H 'Content-Type: application/json' \
  -d '{"session_id":"sess-1","host":"laptop-a","project_dir":"/tmp/proj","payload":{"phase":"validate","tool_calls":42}}'

curl -sS "http://127.0.0.1:3100/api/v1/steop/inbox?session_id=sess-1" \
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

# Keyword injection (v0.5.0+)
echo '{"session_id":"sess-1","cwd":"/tmp","hook_event_name":"UserPromptSubmit","prompt":"/steop:st-flow build the thing"}' \
  | CLAUDE_PLUGIN_ROOT=$(pwd)/plugins/steop steop hook UserPromptSubmit
# => {"hookSpecificOutput":{"hookEventName":"UserPromptSubmit","additionalContext":"<st-flow SKILL.md body>"}}

# Subagent lifecycle log (v0.5.0+)
echo '{"session_id":"sess-1","cwd":"/tmp","hook_event_name":"SubagentStop","agent_id":"a-1","agent_type":"researcher","output":"done","success":true}' \
  | steop hook SubagentStop
# => {}  (log event appended to steop_logs)
```

## 9. Known limitations

- No server-side auth enforcement yet on steop endpoints beyond whatever the existing stele auth middleware provides.
- No migrations subsystem. Schema changes are additive-only: `CREATE TABLE IF NOT EXISTS` for new tables, and the `ensure_steop_schema()` helper uses `pragma_table_info` guards before `ALTER TABLE ADD COLUMN`.
- The stele-server uses a shared tokio mutex around its SQLite connection, so all DB access (including steop) is serialized. Fine for workflow-scale traffic but would need revisiting for high concurrency.
- The `steop` binary must be rebuilt with `apps/steop/scripts/build.sh` after every Go source change. No auto-rebuild on install.
- Status projection has no background materializer yet; it computes on read.
- **v0.5.0**: `log` and `inbox` tables are append-only with no TTL or DELETE path. Rows accumulate until manually pruned.
- **v0.5.0**: `steop_state` primary key is still `session_id` alone — the `host` + `project_dir` columns are metadata, not composite key. Two machines posting the same `session_id` will still collide on `/state/:id` writes. Composite PK is a v2 migration.
- **v0.5.0**: `persistent_mode` flag is stored but not honored — Stop always returns `Allow()`. Full block-and-resume loop is v0.7.
- **v0.5.0**: `PermissionRequest` handler is observe-only. It does not inject an allow/deny envelope, so user confirmation prompts are unaffected by steop today.
