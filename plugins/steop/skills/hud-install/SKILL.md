---
name: hud-install
description: Verify the steop HUD works and optionally wire it into tmux status-right or a shell alias
user-invocable: true
---

# Install steop HUD

One-time setup to verify the HUD works and optionally integrate it with tmux and your shell.

## Procedure

### Step 1: Verify `steop` is on PATH

```bash
which steop && steop version
```

If `steop` is not found, run `/steop:install` first and restart your shell.

### Step 2: Verify the server is reachable

The HUD polls `stele-server`'s steop endpoints. Confirm the server is up:

```bash
curl -sf http://127.0.0.1:3100/api/v1/stats >/dev/null && echo "stele-server OK"
```

If the curl fails:
- Start the server (`cargo run -p stele-server` from `apps/stele/` or launch the menu bar app).
- Or update `~/.config/stele/config.toml` if the server is on a different host/port.

### Step 3: Smoke-test the HUD

Run it once. This exits immediately and prints a single line (or an error if no sessions exist yet):

```bash
steop hud --once --no-color
```

Expected outputs:
- **`no sessions found ...`** — normal on a fresh install. Run `/steop:st-flow <task>` once, then retry.
- **`[flow] execute step=3/7 loop=0 tools=12`** — success, a session exists.

### Step 4 (optional): Tmux status-right snippet

Add a one-line HUD summary to your tmux status bar. Append to `~/.tmux.conf`:

```bash
cat >> "$HOME/.tmux.conf" <<'EOF'

# steop HUD in status-right (refreshes every 2s)
set -g status-interval 2
set -ag status-right ' #(steop hud --once --no-color 2>/dev/null)'
EOF
tmux source-file "$HOME/.tmux.conf" 2>/dev/null || true
```

### Step 5 (optional): Shell alias

If you frequently open the HUD, add a short alias:

```bash
printf '\nalias hud="steop hud"\n' >> "$HOME/.zshrc"
```

Use `~/.bashrc` instead if you are on bash. Re-source your shell rc or open a new terminal.

### Step 6: Run the HUD

Tell the user:

> Setup complete. In a second terminal pane, run `/steop:hud` (or `steop hud` directly) to open the live panel. Press Ctrl+C to quit.

Recommended layout: split your terminal so Claude Code runs on the left and `steop hud` runs on the right.
