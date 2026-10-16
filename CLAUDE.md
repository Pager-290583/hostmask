# Hostmask: guide for Claude Code

Hosts-file set manager. Rust reimplementation of GasMask 0.8.6 (macOS). Cargo workspace:
- `crates/core` (`hostmask_core`): sets, storage, `platform.rs` (writes the system hosts file, flushes DNS).
- `crates/cli` (`hostmask`): command-line tool.
- `crates/gui` (`hostmask-gui`): eframe/egui window, tray (`tray-icon` on macOS, `ksni` on Linux).

Full project memory (decisions, verified facts, pending work) is in `docs/memory/`. Start with `docs/memory/MEMORY.md`, then `hostmask-status.md`. Those files were exported from a macOS session. Their `[[links]]` point at files in the same folder.

## Build and test

- `cargo test --workspace`
- macOS app bundle: `scripts/bundle-macos.sh` (writes `dist/Hostmask.app`).
- Linux/Flatpak: see `flatpak/BUILD-FEDORA.md`.
- Fedora has no system Rust. `scripts/cargo-sdk.sh <args>` runs cargo from the Flatpak SDK (`org.freedesktop.Sdk//25.08` + rust-stable), with output in `~/.cache/hostmask-target`.

Keep build output outside the synced project folder. On macOS the Cargo target dir was moved to `~/Library/Caches/hostmask/target` because the project lives in Yandex Disk. Set `CARGO_TARGET_DIR` in the shell, or the build will recreate `target/` inside the project.

## Rules that came out of real bugs

- In eframe 0.37, `App::ui` is not called while the window is hidden. Tray commands and background job results must be handled in `logic()`.
- Writing `/etc/hosts` on Linux must pass the contents through stdin. Flatpak's `/tmp` is private, so a temp file is not visible to the host.
- The Linux tray uses `ksni`, not `tray-icon`. `tray-icon` needs GTK and libappindicator, which the Flatpak runtime does not have.
- Wayland cannot hide a window (`ViewportCommand::Visible(false)` is ignored), so close-to-tray needs X11. The Linux GUI forces XWayland; keep `--socket=x11` in the Flatpak manifest.
- `ksni` is patched in `vendor/ksni` (see its `HOSTMASK.md`) to add the Ayatana label, the set name next to the tray icon. Upstream 0.3.6 has no label.
- Browser DNS cache (Chrome) is separate from the OS cache. Changing hosts is not enough; see `docs/memory/hostmask-browser-dns-cache.md`.
- Do not run privileged commands on the user's real `/etc/hosts` as a test. Activation changes the system file. Ask first.

## Working with the user

The user writes in Russian and expects Russian replies. They test the app themselves and report results. Give commands they must run (anything needing sudo or a password) as copy-paste blocks. Do not change system configuration without asking.
