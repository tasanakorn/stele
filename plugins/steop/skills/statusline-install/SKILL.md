---
name: statusline-install
description: Install the steop statusline into Claude Code's native status bar by configuring ~/.claude/settings.json
user-invocable: true
---

# Install steop statusline

One-time setup to wire `steop statusline` into Claude Code's native status bar. The statusline is rendered by Claude Code itself on each refresh, so the current phase, step, and counters of a running `/steop:st-flow` pipeline appear inline in your session — no second terminal, no tmux pane.

This is the cerbrix-style integration: a one-shot render command driven by Claude Code's `statusLine` setting in `~/.claude/settings.json`.

## Procedure

### Step 1: Verify `steop` is on PATH

```bash
which steop && steop version
```

If `steop` is not found, run `/steop:install` first and restart your shell. The statusline command lives in the same binary as the hook dispatcher.

### Step 2: Verify the stele server is reachable

The statusline polls stele-server's `/api/v1/steop/status/:id` endpoint. Confirm the server is up:

```bash
curl -sf http://127.0.0.1:3100/api/v1/stats >/dev/null && echo "stele-server OK"
```

If the curl fails:
- Start the server (`cargo run -p stele-server` from `apps/stele/` or launch the menu bar app).
- Or update `~/.config/stele/config.toml` if the server is on a different host/port.

The statusline tolerates an unreachable server — it will print `steop offline` instead of failing — but installation is smoother if the server is up for the smoke test.

### Step 3: Smoke-test the statusline command

Claude Code writes a JSON payload to the command's stdin. Simulate that manually:

```bash
echo '{}' | steop statusline --no-color
```

Expected outputs:
- **`steop idle`** — normal on a fresh install (no sessions exist yet).
- **`[flow] execute 3/7 loop=0 tools=12 retries=0`** — success, a session exists.
- **`steop offline`** — the Go binary could not reach the server. Recheck Step 2.

The command always exits 0; a broken statusline must never stall a Claude Code session.

### Step 4: Patch `~/.claude/settings.json`

Add or update the `statusLine` key to invoke `steop statusline`. This script creates a `.bak` first, warns if a different statusline is already configured, and preserves any other settings:

```bash
python3 - <<'PY'
import json, pathlib, shutil, sys

p = pathlib.Path.home() / ".claude" / "settings.json"
p.parent.mkdir(parents=True, exist_ok=True)

data = {}
if p.exists() and p.read_text().strip():
    try:
        data = json.loads(p.read_text())
    except json.JSONDecodeError as e:
        print(f"error: existing {p} is not valid JSON: {e}", file=sys.stderr)
        sys.exit(1)
    shutil.copy(p, p.with_suffix(".json.bak"))
    print(f"backed up existing settings to {p.with_suffix('.json.bak')}")

desired = {
    "type": "command",
    "command": "steop statusline",
    "refreshInterval": 2,
}
prev = data.get("statusLine")
if prev and prev != desired:
    print(f"warning: replacing existing statusLine: {prev}", file=sys.stderr)

data["statusLine"] = desired
p.write_text(json.dumps(data, indent=2) + "\n")
print(f"wrote statusLine to {p}")
PY
```

**If a different `statusLine` is already configured**, the script prints a warning but still overwrites it. Stop and ask the user whether to proceed — if they want to keep their existing statusline, revert from `settings.json.bak` and skip Step 5.

### Step 5: Restart Claude Code

Claude Code reads `~/.claude/settings.json` on startup. Tell the user:

> Restart Claude Code (quit and relaunch, or run `/reload-plugins` is not enough — a full restart is required). The steop statusline will appear at the bottom of your session on next launch.

### Step 6: Verify

After restart, run `/steop:st-flow <any small task>` in a new Claude Code session. The statusline at the bottom should update through `[flow] clarify`, `[flow] research`, `[flow] plan`, etc. as the pipeline progresses.

## Uninstall

To revert, restore the backup:

```bash
cp "$HOME/.claude/settings.json.bak" "$HOME/.claude/settings.json"
```

Or remove just the `statusLine` key:

```bash
python3 - <<'PY'
import json, pathlib
p = pathlib.Path.home() / ".claude" / "settings.json"
data = json.loads(p.read_text())
data.pop("statusLine", None)
p.write_text(json.dumps(data, indent=2) + "\n")
PY
```

Then restart Claude Code.

## Prerequisites

- `steop` must be on `PATH`. Run `/steop:install` if not.
- Python 3 must be available for the settings-patching script.
- The stele server should be reachable at the URL in `~/.config/stele/config.toml` (otherwise the statusline will show `steop offline`).
