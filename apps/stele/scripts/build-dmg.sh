#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

# Extract version from Cargo.toml
VERSION=$(grep '^version' "$ROOT_DIR/Cargo.toml" | head -1 | sed 's/.*"\(.*\)".*/\1/')

APP_DIR="$ROOT_DIR/target/release/Stele.app"
DMG_NAME="Stele-${VERSION}-macos.dmg"
DMG_PATH="$ROOT_DIR/target/release/$DMG_NAME"

if [ ! -d "$APP_DIR" ]; then
    echo "Error: Stele.app not found at $APP_DIR"
    echo "Run ./scripts/build-macos.sh first."
    exit 1
fi

echo "Creating $DMG_NAME..."

# Create temporary directory for DMG contents
STAGING_DIR=$(mktemp -d)
trap 'rm -rf "$STAGING_DIR"' EXIT

cp -R "$APP_DIR" "$STAGING_DIR/Stele.app"
ln -s /Applications "$STAGING_DIR/Applications"

# Remove existing DMG if present
rm -f "$DMG_PATH"

# Create compressed DMG
hdiutil create \
    -volname "Stele" \
    -srcfolder "$STAGING_DIR" \
    -ov \
    -format UDZO \
    "$DMG_PATH" >/dev/null 2>&1

echo ""
echo "✓ Created: $DMG_PATH"
echo "  Size: $(du -h "$DMG_PATH" | cut -f1)"
