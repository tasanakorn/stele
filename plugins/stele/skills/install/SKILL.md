---
name: install
description: Install Stele CLI and server, configure connection, and verify setup
user-invocable: true
---

# Install & Configure Stele

End-to-end setup: install the CLI, optionally install the server, configure the connection profile, and verify everything works.

The MCP server is **automatically registered** when this plugin is installed — no manual MCP configuration is needed. This skill handles everything else.

## Procedure

### Step 1: Detect Platform

```bash
uname -s
```

Store the result — macOS (`Darwin`) or Linux (`Linux`). This determines which server options are available later.

### Step 2: Check Prerequisites

Check if the Rust toolchain is available:

```bash
cargo --version
```

If `cargo` is not found, tell the user:

> Stele is installed from source and requires the Rust toolchain. Install it from https://rustup.rs/ then re-run `/stele:install`.

Stop here if Rust is not available.

### Step 3: Install CLI

Check if the `stele` CLI is already on PATH:

```bash
which stele
```

**If found**, verify it works with `stele --help` and skip to Step 4.

**If not found**, install it:

> Installing the Stele CLI from source. This compiles from source and may take a few minutes.

```bash
cargo install --git https://github.com/tasanakorn/stele stele-cli
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

### Step 4: Check for Existing Server

Before asking about server installation, check if a server is already reachable:

```bash
stele status
```

**If the server responds successfully**, tell the user a server is already running and skip to Step 6.

**If it fails**, continue to Step 5.

### Step 5: Install Server

Ask the user:

> Do you want to install the Stele server locally, or will you connect to a remote/existing server?
>
> 1. **Install locally** — I'll build and install the server on this machine
> 2. **Remote server** — I already have a server running elsewhere, just configure the connection

**If remote**: skip to Step 6.

**If local**, determine the server mode based on platform:

#### macOS — Choose Server Mode

Ask the user:

> Which server mode do you prefer?
>
> 1. **Desktop app** (recommended) — Runs as a menu bar app with a system tray icon. Database stored in `~/Library/Application Support/Stele/`.
> 2. **Headless daemon** — Runs as a background process with no UI. Suitable for automation or running via launchd.

**Desktop app:**

> Building the Stele server (desktop mode). This compiles from source and may take several minutes.

```bash
cargo install --git https://github.com/tasanakorn/stele stele-server
```

After install, start it. The desktop binary requires the macOS window server and cannot be backgrounded with `&` — launch it normally:

```bash
open -a stele-server 2>/dev/null || nohup stele-server &>/dev/null &
```

If `open -a` does not work (binary not in an `.app` bundle), tell the user:

> The desktop server needs to run in the foreground of its own terminal. Open a separate terminal and run `stele-server`, or build the `.app` bundle for a proper menu bar experience:
>
> ```bash
> git clone https://github.com/tasanakorn/stele.git /tmp/stele-build
> cd /tmp/stele-build/apps/stele && bash scripts/build-macos.sh
> cp -r target/release/Stele.app /Applications/
> open /Applications/Stele.app
> ```

Tell the user: The Stele server is now running in the menu bar. It will store data in `~/Library/Application Support/Stele/stele.db`.

**Headless:**

> Building the Stele server (headless mode). This compiles from source and may take several minutes.

```bash
cargo install --git https://github.com/tasanakorn/stele stele-server --no-default-features --features headless
```

After install, start it:

```bash
stele-server &
```

Tell the user: The server is listening on `http://127.0.0.1:3100` by default. Use `--bind` to change the address.

#### Linux — Headless Server

On Linux, only headless mode is available. Ask the user:

> How would you like to install the server?
>
> 1. **Build from source** — Uses `cargo install` (requires Rust toolchain, already verified)
> 2. **Docker** — Uses the project's Dockerfile (requires Docker)

**Build from source:**

> Building the Stele server (headless). This compiles from source and may take several minutes.

```bash
cargo install --git https://github.com/tasanakorn/stele stele-server --no-default-features --features headless
```

After install, start it:

```bash
stele-server &
```

**Docker:**

> The Docker build requires a clone of the Stele repository.

```bash
git clone https://github.com/tasanakorn/stele.git /tmp/stele-build
docker build -t stele /tmp/stele-build/apps/stele/
docker run -d --name stele -v stele-data:/data -p 3100:3100 stele
```

Tell the user: The server is running in Docker, data persisted in the `stele-data` volume.

#### Verify Server Started

After any install method, verify:

```bash
stele status
```

If it fails, wait a few seconds and retry (the server may need time to start). If it still fails:

- Check if the process is running (`ps aux | grep stele-server`)
- Check logs for errors
- Ensure port 3100 is not in use by another process

### Step 6: Configure Connection Profile

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

### Step 7: Verify Connectivity

```bash
stele status
```

This should report the server as reachable. If it fails:

- Confirm the Stele server is running
- Check the server URL in `stele config show`
- Check firewall / network access for remote servers

### Step 8: Next Steps

Tell the user:

- The MCP connection is provided by the plugin automatically — no action needed
- If they changed the connection profile, they should **restart Claude Code** so the MCP proxy picks up the new settings
- Suggest running `/stele:bootstrap` to initialize the current project with Stele
