# Local patch of ksni 0.3.6

Source: crates.io `ksni` 0.3.6 (Unlicense), used through `[patch.crates-io]` in the workspace `Cargo.toml`.

Added: the Ayatana label, the text shown next to the tray icon (GNOME AppIndicator extension, Ubuntu).
- `Tray::label()` and `Tray::label_guide()` in `src/lib.rs`.
- D-Bus properties `XAyatanaLabel`, `XAyatanaLabelGuide` and signal `XAyatanaNewLabel` in `src/dbus_interface.rs`.
- Change tracking and emission in `src/service.rs` (`update_properties`).

Drop this folder and the patch once upstream ksni supports the label.
