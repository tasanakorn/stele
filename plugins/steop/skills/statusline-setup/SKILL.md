---
name: statusline-setup
description: Configure Claude Code's statusLine to call `steop statusline`
user-invocable: true
---

# Set up steop statusline

Optional companion to `/steop:install`. Wires the two-line statusline into Claude Code's native status bar by patching `~/.claude/settings.json` to invoke `steop statusline` directly — no shell script, no `jq` prerequisite.

**What gets rendered:**

- Line 1: model | project | git branch | context bar | rate limits or cost (parsed from Claude Code's stdin JSON by the `steop` binary itself)
- Line 2: steop pipeline state (`steop: [mode] phase step  loop=N tools=N retries=N`), or `idle`/`offline` when unavailable

Requires `/steop:install` to have been run first so `steop` is on `PATH`.

## Procedure

### Step 1: Verify steop is on PATH

```bash
which steop && steop version
```

If `steop` is not found, run `/steop:install` first and ensure `~/.local/bin` is on your `PATH`.

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

desired = {
    "type": "command",
    "command": "steop statusline",
}
prev = data.get("statusLine")

if prev == desired:
    print(f"{p} already configured — nothing to do")
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

## Uninstall

```bash
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

- `steop` binary on `PATH` (run `/steop:install` first).
- Python 3 for the settings patcher.
- `git` is optional and only used for the branch segment at runtime.
