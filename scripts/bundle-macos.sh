#!/bin/sh
# Builds a release Hostmask.app for macOS into ./dist.
# Usage: scripts/bundle-macos.sh [aarch64-apple-darwin|x86_64-apple-darwin]
set -eu

ROOT=$(cd "$(dirname "$0")/.." && pwd)
TARGET_DIR=${CARGO_TARGET_DIR:-$ROOT/target}
case "$TARGET_DIR" in
    /*) ;;
    *) TARGET_DIR="$PWD/$TARGET_DIR" ;;
esac
TARGET=${1:-$(rustc -vV | sed -n 's/^host: //p')}
case "$TARGET" in
    aarch64-apple-darwin|x86_64-apple-darwin) ;;
    *) echo "Unsupported macOS target: $TARGET" >&2; exit 1 ;;
esac
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT/Cargo.toml" | head -1)
APP="$ROOT/dist/Hostmask.app"

export CARGO_TARGET_DIR="$TARGET_DIR"
export MACOSX_DEPLOYMENT_TARGET=${MACOSX_DEPLOYMENT_TARGET:-11.0}
cargo build --locked --release --target "$TARGET" -p hostmask-gui -p hostmask --manifest-path "$ROOT/Cargo.toml"

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$TARGET_DIR/$TARGET/release/hostmask-gui" "$APP/Contents/MacOS/Hostmask"
cp "$TARGET_DIR/$TARGET/release/hostmask" "$APP/Contents/Resources/hostmask-cli"

# Generate all Retina and non-Retina sizes from the approved artwork.
ICONSET="$ROOT/dist/Hostmask.iconset"
mkdir -p "$ICONSET"
for SIZE in 16 32 128 256 512; do
    sips -z "$SIZE" "$SIZE" "$ROOT/assets/icons/hostmask-1024.png" --out "$ICONSET/icon_${SIZE}x${SIZE}.png" >/dev/null
    DOUBLE=$((SIZE * 2))
    sips -z "$DOUBLE" "$DOUBLE" "$ROOT/assets/icons/hostmask-1024.png" --out "$ICONSET/icon_${SIZE}x${SIZE}@2x.png" >/dev/null
done
iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/Hostmask.icns"

cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key><string>Hostmask</string>
    <key>CFBundleDisplayName</key><string>Hostmask</string>
    <key>CFBundleIdentifier</key><string>io.github.hostmask</string>
    <key>CFBundleExecutable</key><string>Hostmask</string>
    <key>CFBundleIconFile</key><string>Hostmask</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleShortVersionString</key><string>$VERSION</string>
    <key>CFBundleVersion</key><string>$VERSION</string>
    <key>LSMinimumSystemVersion</key><string>$MACOSX_DEPLOYMENT_TARGET</string>
    <key>LSUIElement</key><true/>
    <key>NSHighResolutionCapable</key><true/>
    <key>NSHumanReadableCopyright</key><string>MIT License</string>
</dict>
</plist>
PLIST

# Ad-hoc signature so the bundle runs locally. It is not notarized.
codesign --force --deep --sign - "$APP"
codesign --verify --deep --strict "$APP"
echo "Built $APP"
