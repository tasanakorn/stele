---
name: hud
description: Open the live steop HUD — a compact terminal panel that shows the current phase, step, and counters for the most recent session
---

# steop HUD

Watch a running st-flow pipeline live. The HUD polls the steop server and redraws a compact panel in place each second.

## Usage

The HUD is a terminal program. Run it in a separate pane (tmux split, second terminal, etc.) — not inside the Claude Code session itself.

```bash
steop hud
```

This watches the most recently updated session. To watch a specific session:

```bash
steop hud --session=<session-id>
```

Find session ids with `steop monitor`.

## Flags

| Flag                   | Effect                                                                 |
| ---------------------- | ---------------------------------------------------------------------- |
| `--session=<id>`       | Watch a specific session instead of the most recent                    |
| `--once`               | Print a one-line summary and exit (use for tmux status-right)          |
| `--json`               | Emit newline-delimited JSON per poll instead of a panel                |
| `--interval=<seconds>` | Poll interval; accepts fractions (default 1; e.g. `--interval=0.5`)    |
| `--no-color`           | Disable ANSI colors (also honored via `NO_COLOR` env var)              |

## What the panel shows

- **phase** — current pipeline phase (clarify, research, plan, execute, validate). Color-coded to match st-flow's agent colors.
- **mode** — `flow` for full pipeline, or a single phase name when run standalone.
- **step** — current step within the phase, formatted as `N/total`.
- **counters** — `loop` (execute-validate retry loop count), `tools` (tool calls so far), `retries` (step-level retries).
- **updated** — timestamp of the last state mutation.

## Interactive mode

Press **Ctrl+C** to quit. The HUD restores the cursor and exits cleanly.

If no session exists yet (nothing has run), the panel shows `no sessions found (run a st-flow task first)` and keeps polling — it picks up the first session automatically once `/steop:st-flow` creates one.

## Tmux integration

Put the HUD in a second pane:

```bash
tmux split-window -h "steop hud"
```

Or put a one-line summary in the status bar (see `/steop:hud-install`).

## Prerequisites

- `steop` must be on `PATH`. If not, run `/steop:install`.
- The stele server must be reachable at the URL in `~/.config/stele/config.toml`.
- At least one st-flow run must have happened (or a `steop state set` call) for a session to exist.
