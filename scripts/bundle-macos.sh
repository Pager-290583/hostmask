#!/bin/sh
# Builds a release Hostmask.app for macOS into ./dist.
# Usage: scripts/bundle-macos.sh
set -eu

ROOT=$(cd "$(dirname "$0")/.." && pwd)
TARGET_DIR=${CARGO_TARGET_DIR:-$ROOT/target}
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT/Cargo.toml" | head -1)
APP="$ROOT/dist/Hostmask.app"

cargo build --release -p hostmask-gui -p hostmask --manifest-path "$ROOT/Cargo.toml"

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$TARGET_DIR/release/hostmask-gui" "$APP/Contents/MacOS/Hostmask"
cp "$TARGET_DIR/release/hostmask" "$APP/Contents/Resources/hostmask-cli"

cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key><string>Hostmask</string>
    <key>CFBundleDisplayName</key><string>Hostmask</string>
    <key>CFBundleIdentifier</key><string>io.github.hostmask</string>
    <key>CFBundleExecutable</key><string>Hostmask</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleShortVersionString</key><string>$VERSION</string>
    <key>CFBundleVersion</key><string>$VERSION</string>
    <key>LSMinimumSystemVersion</key><string>11.0</string>
    <key>LSUIElement</key><true/>
    <key>NSHighResolutionCapable</key><true/>
    <key>NSHumanReadableCopyright</key><string>MIT License</string>
</dict>
</plist>
PLIST

# Ad-hoc signature so the bundle runs locally. It is not notarized.
codesign --force --deep --sign - "$APP"
echo "Built $APP"
