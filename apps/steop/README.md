# steop

Go runtime for the **steop** Claude Code workflow plugin. A single binary that
dispatches Claude Code hooks (PreToolUse safety blocks, PostToolUse counter
bumps) and provides CLI helpers for session state and scoped blob storage.
It talks HTTP to `stele-server`'s `/api/v1/steop/*` endpoints.

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

- `steop hook <event>` — dispatch a Claude Code hook (reads JSON from stdin).
- `steop state get|set|incr|reset|delete <session> ...` — session state + counters.
- `steop storage put|get|delete|list <scope> [key] [content]` — scoped blobs.
- `steop statusline [--session=<id>] [--json] [--no-color] [--line2-only]` — two-line renderer for the Claude Code status bar. Reads Claude Code's session JSON from stdin and prints:
  - **Line 1**: `model | project | git branch | context bar | cost-or-rate-limits`
  - **Line 2**: `steop: [<mode>] <phase> <step>  loop=N tools=N retries=N`

  When stdin has no session JSON (or `--line2-only` is passed), only line 2 is printed. Cross-platform (macOS, Linux, Windows), no shell/`jq` dependencies. Always exits 0.
- `steop monitor [--json] [--limit=<n>]` — list recent steop sessions on stele-server.
- `steop version` — print the version constant.

## Environment variables

| Variable         | Purpose                                               |
| ---------------- | ----------------------------------------------------- |
| `STELE_URL`      | Override server base URL (default `127.0.0.1:3100`).  |
| `STELE_AUTH_KEY` | Override auth key (sent as `X-Stele-Key` header).     |
| `STEOP_DEBUG`    | Set to `1` to enable debug logging to stderr.         |

## Config

Reads `~/.config/stele/config.toml` (primary) or
`~/Library/Application Support/stele/config.toml` on macOS (fallback). If no
config is present, a local default profile is used — the binary never panics
on missing config.
