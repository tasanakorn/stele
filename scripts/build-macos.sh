#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

# Extract version from Cargo.toml
VERSION=$(grep '^version' "$ROOT_DIR/Cargo.toml" | head -1 | sed 's/.*"\(.*\)".*/\1/')
echo "Building Stele v${VERSION} .app bundle..."

# 1. Build release binary
echo "→ Compiling release binary..."
cargo build --release --manifest-path "$ROOT_DIR/Cargo.toml"

BINARY="$ROOT_DIR/target/release/stele"
if [ ! -f "$BINARY" ]; then
    echo "Error: Release binary not found at $BINARY"
    exit 1
fi

# 2. Generate .icns from AppIcon.png
ICON_SRC="$ROOT_DIR/assets/AppIcon.png"
if [ ! -f "$ICON_SRC" ]; then
    echo "Error: App icon not found at $ICON_SRC"
    exit 1
fi

ICONSET_DIR="$ROOT_DIR/target/release/AppIcon.iconset"
ICNS_FILE="$ROOT_DIR/target/release/AppIcon.icns"

echo "→ Generating .icns from AppIcon.png..."
rm -rf "$ICONSET_DIR"
mkdir -p "$ICONSET_DIR"

# Generate all required icon sizes
for SIZE in 16 32 64 128 256 512; do
    sips -z $SIZE $SIZE "$ICON_SRC" --out "$ICONSET_DIR/icon_${SIZE}x${SIZE}.png" >/dev/null 2>&1
done
for SIZE in 32 64 128 256 512 1024; do
    HALF=$((SIZE / 2))
    sips -z $SIZE $SIZE "$ICON_SRC" --out "$ICONSET_DIR/icon_${HALF}x${HALF}@2x.png" >/dev/null 2>&1
done

iconutil -c icns "$ICONSET_DIR" -o "$ICNS_FILE"
rm -rf "$ICONSET_DIR"

# 3. Assemble .app bundle
APP_DIR="$ROOT_DIR/target/release/Stele.app"
echo "→ Assembling Stele.app..."
rm -rf "$APP_DIR"
mkdir -p "$APP_DIR/Contents/MacOS"
mkdir -p "$APP_DIR/Contents/Resources"

# Copy binary
cp "$BINARY" "$APP_DIR/Contents/MacOS/stele"

# Copy and process Info.plist (substitute version)
sed "s/__VERSION__/${VERSION}/g" "$ROOT_DIR/macos/Info.plist" > "$APP_DIR/Contents/Info.plist"

# Copy icon
cp "$ICNS_FILE" "$APP_DIR/Contents/Resources/AppIcon.icns"

echo ""
echo "✓ Built: $APP_DIR"
echo ""
echo "To install, drag Stele.app to /Applications, or run:"
echo "  open $APP_DIR"
