# Hostmask

<p align="center">
  <img src="assets/icons/hostmask-256.png" alt="Hostmask — hosts-file set manager for macOS, Linux and Windows" width="160" height="160">
</p>

A hosts-file set manager for macOS, Linux (Flatpak) and Windows. A Rust reimplementation of GasMask 0.8.6, see `docs/REVERSE_ENGINEERING.md`.

## Features

- **GUI** (`crates/gui`, egui): sets grouped into Local / Remote / Combined, monospace editor with unsaved-changes tracking, `/etc/hosts` view.
- Activate a set: writes it to `/etc/hosts` with administrator rights (macOS password dialog) and flushes the DNS cache.
- Remote sets: add by http(s) URL, refreshed every N minutes (setting). Rejects HTML/image responses.
- Combined sets from several sets, marked with `# Hosts File: <name>`.
- Warning banner when `/etc/hosts` no longer matches the active set, with a one-click re-apply.
- Shortcuts: Cmd+S save, Cmd+Option+→ / ← next / previous set.
- Status-bar (menu bar) icon: closing the window hides it and the app keeps running. The menu lists all sets (checkmark on the active one), Next/Previous, Show and Quit. The active set's name is shown next to the icon (on Linux: GNOME AppIndicator extension / Ubuntu; KDE shows the icon only). Turn this off in Settings to make closing the window quit the app.
- Remote sets refresh in a background thread, so they update while the window is hidden.
- CLI (`crates/cli`): `hostmask list | show | new | save | rm | rename | activate | next | prev | combine | system`.
- Data stays in the GasMask folder `~/Library/Gas Mask/{Local,Remote,Combined}` (override with `HOSTMASK_HOME`).

- First start (no sets yet): the machine's current hosts file is saved as the set "Исходный" and marked active.

## Not yet

- Global hotkeys, start at login.
- Automatic restore of `/etc/hosts` when another program changes it (the app warns instead, so you are not asked for a password on every change).
- Windows browser DNS/cache shortcut.

## Downloads and installation

Download the files from [Releases](https://github.com/Pager-290583/hostmask/releases/tag/latest).

| Platform | File | Installation |
| --- | --- | --- |
| macOS, Apple Silicon (M1 and newer) | `Hostmask-macos-arm64.dmg` | Open the disk image and drag Hostmask into Applications. |
| macOS, Intel | `Hostmask-macos-x86_64.dmg` | Open the disk image and drag Hostmask into Applications. |
| Linux, x86_64 | `hostmask.flatpak` | `flatpak install --user hostmask.flatpak` |
| Windows 10/11, x64 | `Hostmask-windows-x86_64.zip` | Extract the ZIP and run `Hostmask.exe`. |

macOS builds are ad-hoc signed, without Apple notarization. If Gatekeeper blocks the first launch, use **System Settings → Privacy & Security → Open Anyway** for Hostmask. The app lives in the menu bar and does not show a Dock icon.
Linux needs a Flatpak runtime from Flathub; GNOME additionally needs the AppIndicator extension for the tray. See [Fedora instructions](flatpak/BUILD-FEDORA.md).
Windows is a portable build with the icon embedded in `Hostmask.exe`; no installer is required. It requests administrator permission (UAC) at launch to write the system hosts file. Closing the window keeps it in the system tray. Sets live in `%APPDATA%\hostmask`. The executable has no Windows publisher certificate. The Chrome DNS/cache shortcut is currently unavailable on Windows. See [Windows notes](windows/README.txt).
Release file checksums are published in `SHA256SUMS`.

## Build

Use current stable Rust (eframe 0.37 requires Rust 1.95 or newer) and Xcode Command Line Tools on macOS.


    cargo test --workspace
    scripts/bundle-macos.sh        # -> dist/Hostmask.app (ad-hoc signed, not notarized)

    scripts/package-macos.sh       # -> dist/Hostmask-macos-arm64.dmg or x86_64.dmg

Keep Cargo build output outside synced folders, for example:

```sh
export CARGO_TARGET_DIR="$HOME/Library/Caches/hostmask/target"
scripts/package-macos.sh
```

The bundle script accepts an optional Rust target (`aarch64-apple-darwin` or `x86_64-apple-darwin`); install that target with rustup before cross-compiling.
Artwork lives in `assets/icons/`. The bundle script generates Retina/non-Retina ICNS sizes automatically. Linux uses the same artwork in the launcher, window and tray; macOS uses a monochrome mask in the menu bar.

On Windows, use current stable Rust with the MSVC toolchain and Visual Studio C++ Build Tools / Windows SDK:

```powershell
cargo test --locked --workspace
./scripts/package-windows.ps1
```

This creates `dist/Hostmask-windows-x86_64.zip` with `Hostmask.exe`, `hostmask-cli.exe` and a short readme. Windows resources and the UAC manifest are embedded during the native Windows build.

## Automated releases

[Build and release](.github/workflows/release.yml) builds Linux Flatpak, native macOS images for Apple Silicon and Intel, and a portable Windows x64 ZIP. Pull requests build without publishing. Pushes to `main` update the rolling `latest` pre-release; `v*` tags publish permanent releases. A manual run on `main` rebuilds and republishes `latest`.

Publication waits for every platform build and its workspace tests. The Windows job also checks CLI startup and creation of the GUI window, using a temporary data directory and verifying that the system hosts file stays unchanged. One release job uploads all platforms together, preserving the existing rolling release and its download URLs. No Apple signing certificate or notarization credentials are configured.


## Caution

Hostmask and GasMask both write `/etc/hosts`. Use one of them at a time: the last one to activate wins, and the other will then show a mismatch.
