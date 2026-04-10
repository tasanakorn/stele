---
name: statusline-setup
description: Install the shipped two-line statusline template to ~/.claude/statusline.sh and point Claude Code at it
user-invocable: true
---

# Set up steop statusline

Optional companion to `/steop:install`. Wires a two-line statusline into Claude Code's native status bar by installing a complete template and patching `~/.claude/settings.json`.

**The template** is cerbrix's line-1 renderer (`model | project | git branch | context bar | rate limits or cost`, parsed from Claude Code's stdin JSON via `jq`) plus a steop line 2 (`steop: [<mode>] <phase> <step>  loop=N tools=N retries=N`, rendered by the `steop statusline` subcommand — prints nothing when steop is unavailable or no session exists, so the statusline degrades gracefully to one line).

The installed file is yours — edit freely. Re-running this skill overwrites it (with a `.bak` first).

## Procedure

### Step 1: Install the template

```bash
mkdir -p "$HOME/.claude"

TEMPLATE="${CLAUDE_PLUGIN_ROOT:-}/scripts/statusline.sh"
if [ ! -f "$TEMPLATE" ]; then
    TEMPLATE="$(ls -t "$HOME/.claude/plugins/cache"/*/steop/*/scripts/statusline.sh 2>/dev/null | head -1 || true)"
fi
if [ -z "$TEMPLATE" ] || [ ! -f "$TEMPLATE" ]; then
    echo "error: could not locate the statusline.sh template shipped with this plugin" >&2
    echo "       (expected at \${CLAUDE_PLUGIN_ROOT}/scripts/statusline.sh)" >&2
    exit 1
fi

if [ -f "$HOME/.claude/statusline.sh" ]; then
    cp "$HOME/.claude/statusline.sh" "$HOME/.claude/statusline.sh.bak"
    echo "backed up existing → ~/.claude/statusline.sh.bak"
fi

cp "$TEMPLATE" "$HOME/.claude/statusline.sh"
chmod +x "$HOME/.claude/statusline.sh"
echo "installed ~/.claude/statusline.sh"
```

### Step 2: Patch `~/.claude/settings.json`

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

script = pathlib.Path.home() / ".claude" / "statusline.sh"
desired = {
    "type": "command",
    "command": f"bash {script}",
    "refreshInterval": 2,
}
prev = data.get("statusLine")

if prev == desired:
    print(f"{p} already points at {script} — nothing to do")
    sys.exit(0)

if prev:
    if p.exists():
        shutil.copy(p, p.with_suffix(".json.bak"))
        print(f"backed up existing settings to {p.with_suffix('.json.bak')}")
    print(f"warning: replacing existing statusLine: {prev}", file=sys.stderr)

data["statusLine"] = desired
p.write_text(json.dumps(data, indent=2) + "\n")
print(f"wrote statusLine to {p}")
PY
```

If a different `statusLine` was already configured, the script prints a warning before overwriting and leaves `settings.json.bak` behind. Stop and ask the user whether to proceed if that happens.

### Step 3: Restart Claude Code

Tell the user:

> Restart Claude Code (quit and relaunch). The two-line statusline will appear at the bottom of your session on next launch. Run `/steop:st-flow <task>` to see line 2 cycle through `clarify → research → plan → execute → validate` with the phase token colour-coded per the st-flow agent palette.

## Customizing

`~/.claude/statusline.sh` is yours after installation. Common edits:

- **Remove segments** — comment out any `--- Segment: … ---` block in the template.
- **Change the separator** — edit the `join()` function at the bottom of line 1.
- **Resize the context bar** — change `width=8` in the context-window segment.
- **Add your own segment** — append a new `line1+=(...)` block. The `jval` helper parses fields from Claude Code's stdin JSON via `jq`.
- **Also show cerbrix on a third line** — add `command -v cerbrix &>/dev/null && echo "cerbrix: $(cerbrix hud render 2>/dev/null || echo idle)"` below the steop block.

Re-running `/steop:statusline-setup` overwrites the file (with a `.bak`), so move customisations to a different filename if you want them preserved across re-runs.

## Uninstall

```bash
cp "$HOME/.claude/statusline.sh.bak" "$HOME/.claude/statusline.sh" 2>/dev/null \
  || rm -f "$HOME/.claude/statusline.sh"

cp "$HOME/.claude/settings.json.bak" "$HOME/.claude/settings.json" 2>/dev/null || \
python3 - <<'PY'
import json, pathlib
p = pathlib.Path.home() / ".claude" / "settings.json"
if not p.exists():
    raise SystemExit(0)
data = json.loads(p.read_text())
data.pop("statusLine", None)
p.write_text(json.dumps(data, indent=2) + "\n")
print("removed statusLine from settings.json")
PY
```

Then restart Claude Code.

## Prerequisites

- `jq` for the line-1 template (`brew install jq` / `apt install jq`).
- Python 3 for the settings patcher.
- `git` is optional and only used for the branch segment.
