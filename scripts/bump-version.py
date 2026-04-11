"""Bump Stele workspace / plugin versions.

Four components can be bumped, individually or together. Each component
owns one or more version-bearing files that are always moved together:

  workspace  apps/stele/Cargo.toml                             ([workspace.package] version)
             - all Rust crates (stele-cli, stele-server, stele-common) inherit via
               `version.workspace = true`; Cargo.lock is refreshed by `cargo update`.
  stele      plugins/stele/.claude-plugin/plugin.json
  steop      plugins/steop/.claude-plugin/plugin.json
             apps/steop/version.go                             (`const Version = "..."`)
  stelite    plugins/stelite/.claude-plugin/plugin.json

Default selection when no components are listed is `workspace stele steop`
(the three that historically move in lock-step). `stelite` has its own
cadence and is only touched when explicitly named.

For bump keywords (`major`/`minor`/`patch`) each selected component is
computed from ITS OWN current version, so a drifted plugin doesn't get
yanked to match the workspace. If a component's own files have drifted
among themselves, the MAX semver is treated as the current and all files
are healed to the new target on write. For an explicit version
(e.g. `0.6.0`) every selected component is set to that literal version.

`cargo update --workspace` runs automatically iff `workspace` is in the
selected set (since Cargo.lock only needs refreshing when Cargo.toml
changed). `--no-cargo-update` skips it either way.

Usage:

    python scripts/bump-version.py patch
    python scripts/bump-version.py patch stele              # only stele plugin
    python scripts/bump-version.py patch steop stelite      # two components
    python scripts/bump-version.py 0.6.0                    # defaults -> 0.6.0
    python scripts/bump-version.py 0.6.0 workspace stele    # explicit pair
    python scripts/bump-version.py patch --all              # all four components
    python scripts/bump-version.py --list                   # show current versions
    python scripts/bump-version.py patch --dry-run
    python scripts/bump-version.py patch --no-cargo-update

Stdlib only — no external packages.
"""

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parent.parent
CARGO_TOML = REPO_ROOT / "apps" / "stele" / "Cargo.toml"

# Component registry. Each component has an ordered list of target files;
# every file is rewritten to the same new version. Order is meaningful for
# printed listings.
#
# File kinds:
#   cargo     - [workspace.package] version = "..."  (TOML, regex rewrite)
#   json      - top-level "version" key              (JSON, parsed)
#   go-const  - `const Version = "..."`              (Go, regex rewrite)
COMPONENTS: dict[str, list[dict]] = {
    "workspace": [
        {"kind": "cargo", "path": CARGO_TOML},
    ],
    "stele": [
        {
            "kind": "json",
            "path": REPO_ROOT / "plugins" / "stele" / ".claude-plugin" / "plugin.json",
        },
    ],
    "steop": [
        {
            "kind": "json",
            "path": REPO_ROOT / "plugins" / "steop" / ".claude-plugin" / "plugin.json",
        },
        {
            "kind": "go-const",
            "path": REPO_ROOT / "apps" / "steop" / "version.go",
        },
    ],
    "stelite": [
        {
            "kind": "json",
            "path": REPO_ROOT / "plugins" / "stelite" / ".claude-plugin" / "plugin.json",
        },
    ],
}
ALL_COMPONENTS = list(COMPONENTS.keys())
DEFAULT_COMPONENTS = ["workspace", "stele", "steop"]

SEMVER_RE = re.compile(r"^(\d+)\.(\d+)\.(\d+)$")
BUMP_KINDS = ("major", "minor", "patch")

# Matches the first `version = "x.y.z"` that appears inside the
# [workspace.package] section, stopping at the next [section] header.
WORKSPACE_PKG_VERSION_RE = re.compile(
    r"(\[workspace\.package\][^\[]*?version\s*=\s*\")([^\"]+)(\")",
    re.DOTALL,
)

# Matches `const Version = "x.y.z"` in a Go source file.
GO_VERSION_CONST_RE = re.compile(
    r"(const\s+Version\s*=\s*\")([^\"]+)(\")",
)


def parse_semver(version: str) -> tuple[int, int, int]:
    m = SEMVER_RE.match(version)
    if not m:
        raise ValueError(f"not a valid semver: {version!r}")
    return int(m.group(1)), int(m.group(2)), int(m.group(3))


def semver_key(version: str) -> tuple[int, int, int]:
    try:
        return parse_semver(version)
    except ValueError:
        return (-1, -1, -1)


def bump(current: str, kind: str) -> str:
    major, minor, patch = parse_semver(current)
    if kind == "major":
        return f"{major + 1}.0.0"
    if kind == "minor":
        return f"{major}.{minor + 1}.0"
    if kind == "patch":
        return f"{major}.{minor}.{patch + 1}"
    raise ValueError(f"unknown bump kind: {kind}")


# ---- per-file readers / writers ----

def read_cargo_version(path: Path) -> str:
    text = path.read_text(encoding="utf-8")
    m = WORKSPACE_PKG_VERSION_RE.search(text)
    if not m:
        raise RuntimeError(f"no [workspace.package] version in {path}")
    return m.group(2)


def write_cargo_version(path: Path, new_version: str) -> None:
    text = path.read_text(encoding="utf-8")
    new_text, count = WORKSPACE_PKG_VERSION_RE.subn(
        lambda m: f"{m.group(1)}{new_version}{m.group(3)}",
        text,
        count=1,
    )
    if count != 1:
        raise RuntimeError(f"failed to rewrite version in {path}")
    path.write_text(new_text, encoding="utf-8")


def read_json_version(path: Path) -> str:
    data = json.loads(path.read_text(encoding="utf-8"))
    return data["version"]


def write_json_version(path: Path, new_version: str) -> None:
    # Rewrite only the top-level "version" value in place. Avoid round-
    # tripping through json.dumps because that would reformat compact
    # inline arrays into multi-line form and escape non-ASCII characters
    # (e.g. em-dash) that the source file intentionally contains.
    text = path.read_text(encoding="utf-8")
    pattern = re.compile(r'("version"\s*:\s*")([^"]+)(")')
    new_text, count = pattern.subn(
        lambda m: f"{m.group(1)}{new_version}{m.group(3)}",
        text,
        count=1,
    )
    if count != 1:
        raise RuntimeError(f'no top-level "version" key in {path}')
    path.write_text(new_text, encoding="utf-8")


def read_go_const_version(path: Path) -> str:
    text = path.read_text(encoding="utf-8")
    m = GO_VERSION_CONST_RE.search(text)
    if not m:
        raise RuntimeError(f"no `const Version = \"...\"` in {path}")
    return m.group(2)


def write_go_const_version(path: Path, new_version: str) -> None:
    text = path.read_text(encoding="utf-8")
    new_text, count = GO_VERSION_CONST_RE.subn(
        lambda m: f"{m.group(1)}{new_version}{m.group(3)}",
        text,
        count=1,
    )
    if count != 1:
        raise RuntimeError(f"failed to rewrite const Version in {path}")
    path.write_text(new_text, encoding="utf-8")


READERS = {
    "cargo": read_cargo_version,
    "json": read_json_version,
    "go-const": read_go_const_version,
}
WRITERS = {
    "cargo": write_cargo_version,
    "json": write_json_version,
    "go-const": write_go_const_version,
}


# ---- component-level operations ----

def read_file_versions(component: str) -> list[tuple[dict, str]]:
    out = []
    for spec in COMPONENTS[component]:
        path: Path = spec["path"]
        if not path.is_file():
            raise RuntimeError(f"{component}: file not found at {path}")
        ver = READERS[spec["kind"]](path)
        out.append((spec, ver))
    return out


def component_current(file_versions: list[tuple[dict, str]]) -> str:
    """Canonical current version for a component: max semver across its files.

    If the files drift (e.g. version.go lags plugin.json), the next bump
    heals them by rewriting all files to the same new value.
    """
    return max((v for _, v in file_versions), key=semver_key)


def write_component_version(
    file_versions: list[tuple[dict, str]], new_version: str
) -> None:
    # Only rewrite files that are not already at the target. This keeps the
    # diff minimal and avoids round-tripping files through json.dumps (which
    # would reformat compact inline arrays into multi-line form).
    for spec, current in file_versions:
        if current != new_version:
            WRITERS[spec["kind"]](spec["path"], new_version)


def resolve_target(arg: str, current: str) -> str:
    if arg in BUMP_KINDS:
        return bump(current, arg)
    parse_semver(arg)  # validate
    return arg


def run_cargo_update() -> int:
    cwd = REPO_ROOT / "apps" / "stele"
    print(f"Running `cargo update --workspace` in {cwd}")
    proc = subprocess.run(
        ["cargo", "update", "--workspace"],
        cwd=cwd,
    )
    return proc.returncode


# ---- commands ----

def cmd_list() -> int:
    print("Current versions:")
    # Capture workspace version as the reference point for drift markers.
    try:
        workspace_files = read_file_versions("workspace")
        workspace_version = component_current(workspace_files)
    except Exception:
        workspace_version = None

    for name in ALL_COMPONENTS:
        try:
            files = read_file_versions(name)
        except Exception as e:
            print(f"  {name:<10} (error: {e})")
            continue
        current = component_current(files)

        markers = []
        if (
            name in ("stele", "steop")
            and workspace_version is not None
            and current != workspace_version
        ):
            markers.append("drifted from workspace")
        distinct = {v for _, v in files}
        if len(distinct) > 1:
            markers.append("internal drift")
        marker_str = ("  [" + "; ".join(markers) + "]") if markers else ""

        print(f"  {name:<10} {current:<10}{marker_str}")
        for spec, v in files:
            rel = spec["path"].relative_to(REPO_ROOT)
            tail = "" if v == current else f"  (drift: {v})"
            print(f"             - {rel}{tail}")
    return 0


def validate_components(names: list[str]) -> list[str]:
    unknown = [n for n in names if n not in COMPONENTS]
    if unknown:
        raise ValueError(
            f"unknown component(s): {', '.join(unknown)}. "
            f"known: {', '.join(ALL_COMPONENTS)}"
        )
    seen = set()
    result = []
    for n in names:
        if n not in seen:
            seen.add(n)
            result.append(n)
    return result


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Bump Stele workspace / plugin versions.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=(
            "Components: " + ", ".join(ALL_COMPONENTS) + "\n"
            "Default:    " + " ".join(DEFAULT_COMPONENTS)
        ),
    )
    parser.add_argument(
        "version",
        nargs="?",
        help="New version (e.g. 0.6.0) or bump kind (major/minor/patch). "
        "Omit together with --list to just inspect current state.",
    )
    parser.add_argument(
        "components",
        nargs="*",
        help="Components to bump. Defaults to: " + " ".join(DEFAULT_COMPONENTS),
    )
    parser.add_argument(
        "--all",
        action="store_true",
        help="Bump every known component (overrides positional list).",
    )
    parser.add_argument(
        "--list",
        action="store_true",
        help="List current versions of all components and exit.",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Show what would change without writing any files.",
    )
    parser.add_argument(
        "--no-cargo-update",
        action="store_true",
        help="Skip `cargo update --workspace` even if workspace is selected.",
    )
    args = parser.parse_args()

    if args.list:
        return cmd_list()

    if args.version is None:
        parser.error("version is required unless --list is given")

    # Resolve component selection.
    if args.all:
        selected = ALL_COMPONENTS[:]
    elif args.components:
        try:
            selected = validate_components(args.components)
        except ValueError as e:
            print(f"error: {e}", file=sys.stderr)
            return 2
    else:
        selected = DEFAULT_COMPONENTS[:]

    # Read current file versions per component.
    try:
        files_by_comp: dict[str, list[tuple[dict, str]]] = {
            n: read_file_versions(n) for n in selected
        }
    except RuntimeError as e:
        print(f"error: {e}", file=sys.stderr)
        return 1

    # Compute current + target per component.
    try:
        current: dict[str, str] = {
            n: component_current(files_by_comp[n]) for n in selected
        }
        targets: dict[str, str] = {
            n: resolve_target(args.version, current[n]) for n in selected
        }
    except ValueError as e:
        print(f"error: {e}", file=sys.stderr)
        return 2

    # A component "needs write" if ANY of its files disagrees with the target.
    # This matters when the component-level current (= max of files) already
    # equals the target but a file is drifted below it and still needs healing.
    def component_needs_write(name: str) -> bool:
        target = targets[name]
        return any(fver != target for _, fver in files_by_comp[name])

    # Print the plan.
    is_bump = args.version in BUMP_KINDS
    header = f"Bump ({args.version})" if is_bump else f"Set version -> {args.version}"
    print(f"{header} for: {', '.join(selected)}")
    any_changes = False
    for name in selected:
        cur = current[name]
        new = targets[name]
        needs = component_needs_write(name)
        if needs:
            any_changes = True
            suffix = ""
        else:
            suffix = "  (no change)"
        print(f"  {name:<10} {cur} -> {new}{suffix}")
        for spec, fver in files_by_comp[name]:
            rel = spec["path"].relative_to(REPO_ROOT)
            if fver == new:
                file_note = "  (unchanged)"
            else:
                file_note = f"  (was {fver}, healing)" if fver != cur else ""
            print(f"             - {rel}{file_note}")

    if args.dry_run:
        print("Dry run — no files written.")
        return 0

    if not any_changes:
        print("Nothing to do: all selected components already at target version.")
        return 0

    # Apply.
    for name in selected:
        if component_needs_write(name):
            write_component_version(files_by_comp[name], targets[name])
    print("Files updated.")

    # Only run cargo update if the workspace file actually changed.
    workspace_changed = "workspace" in selected and component_needs_write("workspace")
    if not workspace_changed:
        return 0
    if args.no_cargo_update:
        print("Skipped `cargo update --workspace` (--no-cargo-update).")
        return 0

    rc = run_cargo_update()
    if rc != 0:
        print(
            "warning: `cargo update --workspace` exited non-zero; "
            "Cargo.lock may be out of sync.",
            file=sys.stderr,
        )
        return rc
    return 0


if __name__ == "__main__":
    sys.exit(main())
