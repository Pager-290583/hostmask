#!/bin/sh
# Runs cargo from the Flatpak Rust SDK, with build output outside Yandex Disk.
# Fedora has no system Rust here; usage: scripts/cargo-sdk.sh build -p hostmask-gui
cd "$(dirname "$0")/.."
exec flatpak run --filesystem=home --share=network --command=sh org.freedesktop.Sdk//25.08 -c \
  'export PATH=/usr/lib/sdk/rust-stable/bin:$PATH CARGO_HOME=$HOME/.cache/hostmask-cargo CARGO_TARGET_DIR=$HOME/.cache/hostmask-target; cargo "$@"' sh "$@"
