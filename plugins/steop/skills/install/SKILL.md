---
name: install
description: Build and install the steop companion binary to ~/.local/bin
user-invocable: true
---

# Install steop Binary

End-to-end setup for the `steop` companion binary: clone the source, build from Go, install to `~/.local/bin`, and verify.

The steop plugin ships skills, agents, and Claude Code hooks. The hooks call a `steop` binary which must be on your `PATH`. This skill builds that binary from source.

## Procedure

### Step 1: Detect Platform

```bash
uname -s
```

Store the result — macOS (`Darwin`) or Linux (`Linux`). Both are supported; Windows is not.

### Step 2: Check Prerequisites

Check that both Git and the Go toolchain are available:

```bash
git --version && go version
```

If `git` is not found, tell the user to install Git first and re-run `/steop:install`.

If `go` is not found, tell the user:

> steop's companion binary is built from Go source and requires the Go toolchain (1.22 or newer). Install it from https://go.dev/dl/ then re-run `/steop:install`.

Stop here if either tool is missing.

### Step 3: Clone Source Repository

steop is built from a local source tree to ensure reproducible builds.

```bash
if [ -d /tmp/steop-build/.git ]; then
  git -C /tmp/steop-build pull --ff-only
else
  rm -rf /tmp/steop-build
  git clone https://github.com/tasanakorn/stele.git /tmp/steop-build
fi
```

If the clone fails (network issues, etc.), stop and ask the user to troubleshoot their network.

### Step 4: Ensure Install Directory Exists

The binary installs to `~/.local/bin`. Create it if missing:

```bash
mkdir -p "$HOME/.local/bin"
```

### Step 5: Build and Install

Read the version from the source and build directly into `~/.local/bin`:

> Building the steop binary from source. This should take only a few seconds.

```bash
cd /tmp/steop-build/apps/steop
VERSION="$(grep -E '^const Version' version.go | sed -E 's/.*"([^"]+)".*/\1/')"
CGO_ENABLED=0 go build \
    -trimpath \
    -ldflags="-s -w -X main.Version=${VERSION}" \
    -o "$HOME/.local/bin/steop" \
    .
```

### Step 6: Verify Installation

Check that `steop` is reachable on `PATH`:

```bash
which steop && steop version
```

**If `steop` is not found**, tell the user to add `~/.local/bin` to their `PATH`. Offer the snippet to add to their shell rc file (`~/.zshrc` or `~/.bashrc`):

```bash
export PATH="$HOME/.local/bin:$PATH"
```

Then re-open their shell and retry `steop version`. If it still fails, stop and ask the user to troubleshoot their `PATH`.

### Step 7: Cleanup

Remove the temporary build directory to free disk space:

```bash
rm -rf /tmp/steop-build
```

### Step 8: Next Steps

Tell the user:

- The steop plugin hooks invoke `steop` as a bare command, so `~/.local/bin` **must** remain on your `PATH` whenever Claude Code runs. If it is not, the hooks will fail silently (Claude Code tolerates hook failures as advisory).
- **Restart Claude Code** so newly-registered hooks pick up the freshly installed binary.
- steop's runtime state is stored in the stele server. If you have not installed stele yet, run `/stele:install` — steop reuses stele's `~/.config/stele/config.toml` profile.
- To rebuild in the future after pulling new changes, run `apps/steop/scripts/build.sh` from your clone — it installs directly to `~/.local/bin`.
