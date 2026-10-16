# Hostmask

A hosts-file set manager for macOS (Windows and Linux/Flatpak planned). A Rust reimplementation of GasMask 0.8.6, see `docs/REVERSE_ENGINEERING.md`.

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
- Windows and Linux/Flatpak builds: code exists but is untested.

## Build

    cargo test --workspace
    scripts/bundle-macos.sh        # -> dist/Hostmask.app (ad-hoc signed, not notarized)

Run `Hostmask.app` with right-click → Open the first time, since it is not notarized.

## Caution

Hostmask and GasMask both write `/etc/hosts`. Use one of them at a time: the last one to activate wins, and the other will then show a mismatch.
