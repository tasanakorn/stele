# steop

Go runtime for the **steop** Claude Code workflow plugin. A single binary that
dispatches all 11 Claude Code hooks (PreToolUse safety regexes, PostToolUse
counter + state, UserPromptSubmit keyword injection, SubagentStart/Stop
lifecycle, SessionStart/End, PreCompact, Stop/Inbox, PermissionRequest observe)
and provides CLI helpers for session state, scoped blob storage, logs, and
the cross-host inbox. It talks HTTP to `stele-server`'s `/api/v1/steop/*`
endpoints, tagging every request with composite identity headers
(`X-Steop-Host`, `X-Steop-Project-Dir`).

## Build

```bash
./scripts/build.sh
```

The build script produces a statically-linked binary at `~/.local/bin/steop`
by default. Override the output directory with the `OUT_DIR` env var:

```bash
OUT_DIR=/custom/path ./scripts/build.sh
```

Manual build:

```bash
go build -o steop .
go test ./...
go vet ./...
```

## Subcommands

- `steop hook <event>` — dispatch a Claude Code hook (reads JSON from stdin). Supported events:
  - `UserPromptSubmit` — writes session sentinel + injects SKILL.md on `st-<phase>:` / `/steop:st-<phase>` triggers
  - `PreToolUse` — Bash deny regex (force-push, `rm -rf /`, etc.)
  - `PostToolUse` — increments `tool_calls` counter, merges `last_tool` state, fires log event
  - `Stop` — desktop notify + posts session summary to inbox + clears phase
  - `SessionStart`, `SessionEnd`, `SubagentStart`, `SubagentStop`, `PreCompact`, `PostToolUseFailure`, `PermissionRequest` — structured logging to `/api/v1/steop/log`
- `steop state get|set|incr|reset|delete <session> ...` — session state + counters.
- `steop storage put|get|delete|list <scope> [key] [content]` — scoped blobs.
- `steop statusline [--session=<id>] [--json] [--no-color] [--line2-only]` — two-line renderer for the Claude Code status bar. Reads Claude Code's session JSON from stdin and prints:
  - **Line 1**: `model | project | git branch | context bar | cost-or-rate-limits`
  - **Line 2**: `steop: [<mode>] <phase> <step>  loop=N tools=N retries=N`

  When stdin has no session JSON (or `--line2-only` is passed), only line 2 is printed. Cross-platform (macOS, Linux, Windows), no shell/`jq` dependencies. Always exits 0.
- `steop monitor [--json] [--limit=<n>]` — list recent steop sessions on stele-server.
- `steop version` — print the version constant.

## Environment variables

| Variable              | Purpose                                                                           |
| --------------------- | --------------------------------------------------------------------------------- |
| `STELE_URL`           | Override server base URL (default `127.0.0.1:3100`).                              |
| `STELE_AUTH_KEY`      | Override auth key (sent as `X-Stele-Key` header).                                 |
| `STEOP_DEBUG`         | Set to `1` to enable debug logging to stderr.                                     |
| `CLAUDE_PLUGIN_ROOT`  | Set by Claude Code; points at the plugin install dir. Used by the keyword-injection path in `UserPromptSubmit` to load `$root/skills/<name>/SKILL.md`. |

Hook-provided fields (not env vars, but worth knowing): every hook handler reads `in.Cwd` from stdin JSON and forwards it as `X-Steop-Project-Dir` on every stele-server request, so Claude Code's `cwd` flows end-to-end as part of the composite session identity.

## Config

Reads `~/.config/stele/config.toml` (primary) or
`~/Library/Application Support/stele/config.toml` on macOS (fallback). If no
config is present, a local default profile is used — the binary never panics
on missing config.

### Profile fields (v0.5.0+)

```toml
default_profile = "default"

[profiles.default]
server_url = "http://127.0.0.1:3100"
auth_key   = ""        # optional — sent as X-Stele-Key
host       = "laptop"  # optional — sent as X-Steop-Host; auto-populated on first load
```

The `host` field is added automatically on first load via `os.Hostname()` and persisted back to the config file. It's used to disambiguate sessions across machines when multiple hosts share a single stele-server.

## Hook output shapes

All handlers write JSON to stdout and exit 0. Three canonical shapes:

- **Allow silently**: `{}`
- **Deny (PreToolUse)**: `{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"..."}}`
- **Inject context (UserPromptSubmit)**: `{"hookSpecificOutput":{"hookEventName":"UserPromptSubmit","additionalContext":"<skill body>"}}`

The binary always exits 0 — a broken hook handler must never stall Claude Code. Errors are logged via `STEOP_DEBUG=1` and swallowed.

## HTTP timeouts

- Normal client operations: 10s.
- Log + Inbox POSTs (fire-and-forget): 500ms via `fastClone()`. A dead stele-server cannot stall a hook beyond this cap regardless of the hook-level Claude Code timeout.
