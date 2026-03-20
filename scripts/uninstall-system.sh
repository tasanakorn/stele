#!/usr/bin/env bash
set -euo pipefail

# Must be root
if [ "$(id -u)" -ne 0 ]; then
    echo "Error: this script must be run as root (sudo $0)"
    exit 1
fi

PURGE=false
if [ "${1:-}" = "--purge" ]; then
    PURGE=true
fi

echo "=== Uninstalling Stele system service ==="

# Stop and disable service
if systemctl is-active --quiet stele 2>/dev/null; then
    echo "Stopping stele service..."
    systemctl stop stele
fi

if systemctl is-enabled --quiet stele 2>/dev/null; then
    echo "Disabling stele service..."
    systemctl disable stele
fi

# Remove unit file
if [ -f /etc/systemd/system/stele.service ]; then
    echo "Removing systemd unit file..."
    rm /etc/systemd/system/stele.service
fi

# Remove binary
if [ -f /usr/local/bin/stele ]; then
    echo "Removing binary..."
    rm /usr/local/bin/stele
fi

# Remove environment file
if [ -f /etc/default/stele ]; then
    echo "Removing environment config..."
    rm /etc/default/stele
fi

# Remove data directory
if [ -d /var/lib/stele ]; then
    if [ "$PURGE" = true ]; then
        echo "Removing data directory /var/lib/stele..."
        rm -rf /var/lib/stele
    else
        echo "Keeping data directory /var/lib/stele (use --purge to remove)"
    fi
fi

# Remove user and group
if getent passwd stele >/dev/null 2>&1; then
    echo "Removing stele user..."
    userdel stele
fi

if getent group stele >/dev/null 2>&1; then
    echo "Removing stele group..."
    groupdel stele
fi

# Reload systemd
systemctl daemon-reload

echo ""
echo "=== Stele uninstalled ==="
if [ "$PURGE" = false ] && [ -d /var/lib/stele ]; then
    echo "Note: /var/lib/stele was preserved. Run with --purge to remove data."
fi
