#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

VERSION=$(grep '^version' "$ROOT_DIR/Cargo.toml" | head -1 | sed 's/.*"\(.*\)"/\1/')

# Must be root
if [ "$(id -u)" -ne 0 ]; then
    echo "Error: this script must be run as root (sudo $0)"
    exit 1
fi

echo "=== Installing Stele v${VERSION} (system service) ==="

# Build release binary in headless mode
echo "Building release binary (headless)..."
cd "$ROOT_DIR"
BUILD_USER="${SUDO_USER:-$(logname)}"
BUILD_USER_HOME=$(getent passwd "$BUILD_USER" | cut -d: -f6)
sudo -u "$BUILD_USER" env PATH="$BUILD_USER_HOME/.cargo/bin:$PATH" \
    cargo build --release --features headless --no-default-features

# Stop service before replacing binary (upgrade case)
if systemctl is-active --quiet stele 2>/dev/null; then
    echo "Stopping running stele service..."
    systemctl stop stele
fi

# Install binary
echo "Installing binary to /usr/local/bin/stele..."
install -m 755 "$ROOT_DIR/target/release/stele" /usr/local/bin/stele

# Create system user/group
if ! getent group stele >/dev/null 2>&1; then
    echo "Creating stele group..."
    groupadd --system stele
fi

if ! getent passwd stele >/dev/null 2>&1; then
    echo "Creating stele user..."
    useradd --system --gid stele --no-create-home --shell /usr/sbin/nologin stele
fi

# Create data directory
echo "Creating /var/lib/stele..."
install -d -o stele -g stele -m 750 /var/lib/stele

# Install systemd unit file
echo "Installing systemd service..."
install -m 644 "$ROOT_DIR/systemd/stele.service" /etc/systemd/system/stele.service

# Install environment config (don't overwrite existing)
if [ ! -f /etc/default/stele ]; then
    echo "Installing default environment to /etc/default/stele..."
    install -m 644 "$ROOT_DIR/systemd/stele.env" /etc/default/stele
else
    echo "/etc/default/stele already exists, skipping (check systemd/stele.env for new options)"
fi

# Reload systemd
systemctl daemon-reload

echo ""
echo "=== Stele v${VERSION} installed ==="
echo ""
echo "To start the service:"
echo "  sudo systemctl enable stele    # start on boot"
echo "  sudo systemctl start stele     # start now"
echo ""
echo "Configuration: /etc/default/stele"
echo "Database:      /var/lib/stele/stele.db"
echo "Logs:          journalctl -u stele"
