# Versioning

SemVer. Major = breaking MCP/API/DB changes, minor = new features, patch = fixes and docs.

## Bump script

Use `scripts/bump-version.py` (stdlib Python, no deps) to move versions in lock-step and automatically refresh `Cargo.lock`.

## Components

The script knows about four components. Each owns one or more version-bearing files that are always rewritten together:

| Component   | Files                                                                              |
| ----------- | ---------------------------------------------------------------------------------- |
| `workspace` | `apps/stele/Cargo.toml` (all Rust crates inherit via `version.workspace = true`)   |
| `stele`     | `plugins/stele/.claude-plugin/plugin.json`                                         |
| `steop`     | `plugins/steop/.claude-plugin/plugin.json` + `apps/steop/version.go`               |
| `stelite`   | `plugins/stelite/.claude-plugin/plugin.json`                                       |

Default selection is `workspace stele steop` (the three that historically move together). `stelite` has its own cadence and is only touched when explicitly named or via `--all`.

## Bump semantics

- **Bump keywords** (`major`/`minor`/`patch`) — each selected component is computed from *its own* current version, so a drifted component is not force-aligned to the workspace.
- **Explicit version** (e.g. `0.6.0`) — every selected component is set to that literal value.
- **Intra-component drift healing** — if files within a single component disagree (e.g. `apps/steop/version.go` lagging `plugins/steop/.../plugin.json`), the max semver is treated as the current version and all files in the component are rewritten on the next bump.
- **`cargo update --workspace`** — runs automatically iff `workspace` is in the change set. Skip with `--no-cargo-update`.

## Usage

```bash
python scripts/bump-version.py --list                    # inspect current versions + drift
python scripts/bump-version.py patch                     # workspace + stele + steop, patch bump
python scripts/bump-version.py minor                     # same, minor bump
python scripts/bump-version.py 0.6.0                     # explicit version for defaults
python scripts/bump-version.py patch steop               # only the steop component
python scripts/bump-version.py 0.6.0 workspace stele     # explicit component list
python scripts/bump-version.py patch --all               # include stelite too
python scripts/bump-version.py patch --dry-run           # preview with no writes
python scripts/bump-version.py patch --no-cargo-update   # skip `cargo update --workspace`
```

## CI coupling

CI validates that `plugins/stele/.claude-plugin/plugin.json` matches the workspace version in `apps/stele/Cargo.toml`. The default component set (`workspace stele steop`) keeps this invariant satisfied automatically.
