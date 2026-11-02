# Hostmask icon

Approved artwork: a silver mask and two connected turquoise network nodes on a graphite tile.
Generated with the built-in ImageGen tool; original prompt: a macOS Hostmask icon combining a friendly mask and network routing, legible at small sizes, no text.

- `hostmask-source.png`: original approved artwork, with transparency.
- `hostmask-1024.png`: application / Flatpak icon and source for macOS ICNS sizes.
- `hostmask-256.png`: embedded window icon and Flatpak launcher icon.
- `hostmask-36.png`: Linux tray icon.

The macOS menu bar uses a separate monochrome mask in `crates/gui/src/tray.rs`, so macOS can tint it for light and dark backgrounds.
`scripts/bundle-macos.sh` creates the `.icns` with the native macOS tools `sips` and `iconutil`.
