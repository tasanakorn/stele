---
name: install
description: Build and install the steop companion binary to ~/.local/bin
user-invocable: true
---

# Install steop Binary

End-to-end setup for the `steop` companion binary. Uses `go install` to pull the source module from the Go proxy and drop a statically-linked binary into `~/.local/bin`.

The steop plugin ships skills, agents, and Claude Code hooks. The hooks call a `steop` binary which must be on your `PATH`. This skill installs that binary.

## Procedure

### Step 1: Detect Platform

```bash
uname -s
```

Store the result — macOS (`Darwin`) or Linux (`Linux`). Both are supported; Windows is not.

### Step 2: Check Prerequisites

Verify the Go toolchain is available:

```bash
go version
```

If `go` is not found, tell the user:

> steop's companion binary is built from Go source and requires the Go toolchain (1.22 or newer). Install it from https://go.dev/dl/ then re-run `/steop:install`.

Stop here if Go is missing. Git is **not** required — `go install` fetches the source via the Go module proxy.

### Step 3: Ensure Install Directory Exists

The binary installs to `~/.local/bin`. Create it if missing:

```bash
mkdir -p "$HOME/.local/bin"
```

### Step 4: Install

Tell the user:

> Installing `steop` via `go install` from the `main` branch. The first run downloads the module to your Go module cache; subsequent reinstalls reuse the cache and finish in seconds.

```bash
GOBIN="$HOME/.local/bin" go install -trimpath -ldflags="-s -w" \
    github.com/tasanakorn/stele/apps/steop@main
```

**Why `@main` and not `@latest`?** `stele` is a monorepo — `apps/steop/` is a subdirectory module, so Go's proxy only recognises release tags prefixed with `apps/steop/vX.Y.Z`. Rather than maintaining a parallel tag stream just for steop, we install from the `main` branch HEAD. The resulting binary records a Go pseudo-version (e.g. `v0.0.0-20260410133000-abc123def456`) in its build metadata, but `steop version` continues to report the human-facing version from `apps/steop/version.go` — currently `0.4.1`.

**To pin a different point**, replace `@main`:
- `@<commit-sha>` — exact commit (the most reliable pin for reproducibility)
- `@<branch-name>` — tip of a feature branch

To see which commit a currently-installed binary was built from:

```bash
go version -m "$(which steop)" | grep vcs
```

### Step 5: Verify Installation

Check that `steop` is reachable on `PATH`:

```bash
which steop && steop version
```

**If `steop` is not found**, `~/.local/bin` is probably not on `PATH`. Offer the snippet for their shell rc file (`~/.zshrc` or `~/.bashrc`):

```bash
export PATH="$HOME/.local/bin:$PATH"
```

Then re-open their shell and retry `steop version`. If it still fails, stop and ask the user to troubleshoot their `PATH`.

### Step 6: Next Steps

Tell the user:

- The steop plugin hooks invoke `steop` as a bare command, so `~/.local/bin` **must** remain on your `PATH` whenever Claude Code runs. If it is not, the hooks will fail silently (Claude Code tolerates hook failures as advisory).
- **Restart Claude Code** so newly-registered hooks pick up the freshly installed binary.
- steop's runtime state is stored in the stele server. If you have not installed stele yet, run `/stele:install` — steop reuses stele's `~/.config/stele/config.toml` profile.
- To see a live pipeline status in your Claude Code status bar, run `/steop:statusline-install` next.
- To rebuild from a local checkout during development, run `apps/steop/scripts/build.sh` — it builds from your working tree instead of the Go proxy and installs to `~/.local/bin`.
