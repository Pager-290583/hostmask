#!/bin/sh
# Build the app and a drag-to-Applications disk image for one Mac architecture.
set -eu
ROOT=$(cd "$(dirname "$0")/.." && pwd)
TARGET=${1:-$(rustc -vV | sed -n 's/^host: //p')}
case "$TARGET" in
    aarch64-apple-darwin) ARCH=arm64 ;;
    x86_64-apple-darwin) ARCH=x86_64 ;;
    *) echo "Unsupported macOS target: $TARGET" >&2; exit 1 ;;
esac
"$ROOT/scripts/bundle-macos.sh" "$TARGET"
STAGING=$(mktemp -d "$ROOT/dist/dmg.XXXXXX")
trap 'rm -rf "$STAGING"' EXIT
ditto "$ROOT/dist/Hostmask.app" "$STAGING/Hostmask.app"
ln -s /Applications "$STAGING/Applications"
hdiutil create -volname Hostmask -srcfolder "$STAGING" -ov -format UDZO \
    "$ROOT/dist/Hostmask-macos-$ARCH.dmg"
hdiutil verify "$ROOT/dist/Hostmask-macos-$ARCH.dmg"
