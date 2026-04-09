---
name: install
description: Install Stele CLI and server, configure connection, and verify setup
user-invocable: true
---

# Install & Configure Stele

End-to-end setup: clone the source, install the CLI, optionally install the server, configure the connection profile, and verify everything works.

The MCP connection is **automatically registered** when this plugin is installed — no manual MCP configuration is needed. This skill handles everything else.

## Procedure

### Step 1: Detect Platform

```bash
uname -s
```

Store the result — macOS (`Darwin`) or Linux (`Linux`). This determines which server options are available later.

### Step 2: Check Prerequisites

Check that both Git and the Rust toolchain are available:

```bash
git --version && cargo --version
```

If `git` is not found, tell the user to install Git first and re-run `/stele:install`.

If `cargo` is not found, tell the user:

> Stele is built from source and requires the Rust toolchain. Install it from https://rustup.rs/ then re-run `/stele:install`.

Stop here if either tool is missing.

### Step 3: Clone Source Repository

Stele is built from a local source tree to ensure reproducible builds with pinned dependencies.

```bash
if [ -d /tmp/stele-build/.git ]; then
  git -C /tmp/stele-build pull --ff-only
else
  rm -rf /tmp/stele-build
  git clone https://github.com/tasanakorn/stele.git /tmp/stele-build
fi
```

If the clone fails (network issues, etc.), stop and ask the user to troubleshoot their network.

### Step 4: Install CLI

Check if the `stele` CLI is already on PATH:

```bash
which stele
```

**If found**, verify it works with `stele --help` and skip to Step 5.

**If not found**, install it:

> Installing the Stele CLI from source. This compiles from source and may take a few minutes.

```bash
cargo install --path /tmp/stele-build/apps/stele/crates/stele-cli
```

After install, verify:

```bash
stele --help
```

If `stele` is still not found, tell the user to ensure `~/.cargo/bin` is in their PATH:

```bash
source "$HOME/.cargo/env"
```

Then retry `stele --help`. If it still fails, stop and ask the user to troubleshoot their PATH.

### Step 5: Check for Existing Server

Before asking about server installation, check if a server is already reachable:

```bash
stele status
```

**If the server responds successfully**, tell the user a server is already running and skip to Step 7.

**If it fails**, continue to Step 6.

### Step 6: Install Server

Ask the user:

> Do you want to install the Stele server locally, or will you connect to a remote/existing server?
>
> 1. **Install locally** — I'll build and install the server on this machine
> 2. **Remote server** — I already have a server running elsewhere, just configure the connection

**If remote**: skip to Step 7.

**If local**, proceed based on platform:

#### macOS

Ask the user:

> Which server mode do you prefer?
>
> 1. **Desktop app** (recommended) — Menu bar app with system tray icon. Data stored in `~/Library/Application Support/Stele/`.
> 2. **Headless daemon** — Background process with no UI, suitable for automation or launchd.

**Desktop app (recommended):**

> Building the Stele desktop app. This compiles from source and may take several minutes.

```bash
bash /tmp/stele-build/apps/stele/scripts/build-macos.sh
```

After the build completes, install the app to `/Applications/`:

```bash
rm -rf /Applications/Stele.app
cp -r /tmp/stele-build/apps/stele/target/release/Stele.app /Applications/
```

If the copy fails with a permission error, retry with `sudo`:

```bash
sudo rm -rf /Applications/Stele.app
sudo cp -r /tmp/stele-build/apps/stele/target/release/Stele.app /Applications/
```

Launch the app:

```bash
open /Applications/Stele.app
```

Tell the user: Stele is now running in the menu bar. Data is stored at `~/Library/Application Support/Stele/stele.db`.

**Headless daemon:**

> Building the Stele server (headless mode). This compiles from source and may take several minutes.

```bash
cargo install --path /tmp/stele-build/apps/stele/crates/stele-server --no-default-features --features headless
```

After install, start it:

```bash
stele-server &
```

Tell the user: The server is listening on `http://127.0.0.1:3100` by default. Use `--bind` to change the address.

#### Linux

On Linux, only headless mode is available. Ask the user:

> How would you like to install the server?
>
> 1. **Build from source** — Headless server binary (Rust toolchain already verified)
> 2. **Docker** — Container using the project Dockerfile (requires Docker)

**Build from source:**

> Building the Stele server (headless). This compiles from source and may take several minutes.

```bash
cargo install --path /tmp/stele-build/apps/stele/crates/stele-server --no-default-features --features headless
```

After install, start it:

```bash
stele-server &
```

**Docker:**

```bash
docker build -t stele /tmp/stele-build/apps/stele/
docker run -d --name stele -v stele-data:/data -p 3100:3100 stele
```

Tell the user: The server is running in Docker, data persisted in the `stele-data` volume.

#### Verify Server Started

After any install method, wait a few seconds for the server to initialize, then verify:

```bash
stele status
```

If it fails, wait 5 seconds and retry once. If it still fails:

- Check if the process is running: `ps aux | grep stele-server`
- On macOS desktop: check if the app launched with `ps aux | grep 'Stele.app/Contents/MacOS/stele'`
- Ensure port 3100 is not in use by another process: `lsof -i :3100`
- Check logs for errors

### Step 7: Configure Connection Profile

Run:

```bash
stele config show
```

**If a local server with default settings** (port 3100, no auth) — no config file is needed. The built-in defaults work.

**If a remote server or custom port**, create the config:

```bash
stele config init
```

This creates `~/.config/stele/config.toml`. Show the user the format:

```toml
default_profile = "local"

[profiles.local]
server_url = "http://127.0.0.1:3100"

[profiles.remote]
server_url = "https://stele.example.com:3100"
auth_key = "your-auth-key-here"
```

Ask the user for their server URL and (optionally) auth key, then write the values into the config file.

To switch profiles: set `default_profile`, use `--profile <name>`, or set `STELE_PROFILE` env var.

### Step 8: Verify Connectivity

```bash
stele status
```

This should report the server as reachable. If it fails:

- Confirm the Stele server is running
- Check the server URL in `stele config show`
- Check firewall / network access for remote servers

### Step 9: Next Steps

Tell the user:

- The MCP connection is provided by the plugin automatically — no action needed
- If they changed the connection profile, they should **restart Claude Code** so the MCP proxy picks up the new settings
- The `/tmp/stele-build` directory can be removed to free disk space: `rm -rf /tmp/stele-build`
- Suggest running `/stele:bootstrap` to initialize the current project with Stele
