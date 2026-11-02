//! Status-bar icon with the list of sets.
//!
//! macOS uses the menu bar through `tray-icon`. Linux uses StatusNotifierItem over D-Bus
//! through `ksni`, which does not need GTK. Both expose the same `Tray` API to the app.

/// What the user asked for from the tray menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrayCmd {
    Show,
    Next,
    Prev,
    Activate(String),
    BrowserCache,
    Quit,
}

/// Monochrome mask for the macOS template icon. The system supplies its tint.
#[cfg(not(target_os = "linux"))]
fn mask_alpha() -> Vec<u8> {
    const SIZE: u32 = 36;
    let mut alpha = Vec::with_capacity((SIZE * SIZE) as usize);
    for y in 0..SIZE {
        for x in 0..SIZE {
            // Supersample the silhouette so it remains smooth on Retina displays.
            let mut coverage = 0_u16;
            for sy in 0..4 {
                for sx in 0..4 {
                    let dx = x as f32 + (sx as f32 + 0.5) / 4.0 - 18.0;
                    let py = y as f32 + (sy as f32 + 0.5) / 4.0;
                    let u = dx.abs() / 15.0;
                    let top = 8.0 + 5.0 * (1.0 - u).powi(4);
                    let bottom = 27.0 - 7.0 * u.powi(3) - 5.0 * (1.0 - u).powi(12);
                    let eye = ((dx.abs() - 7.5) / 5.0).powi(2)
                        + ((py - 17.5) / 3.0).powi(2) < 1.0;
                    if u <= 1.0 && py >= top && py <= bottom && !eye {
                        coverage += 1;
                    }
                }
            }
            alpha.push((coverage * 255 / 16) as u8);
        }
    }
    alpha
}

#[cfg(not(target_os = "linux"))]
pub use native::Tray;

#[cfg(target_os = "linux")]
pub use linux::Tray;

#[cfg(not(target_os = "linux"))]
mod native {
    use std::sync::mpsc::{self, Receiver};

    use eframe::egui;
    use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
    use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

    use super::{mask_alpha, TrayCmd};

    const ID_SHOW: &str = "show";
    const ID_NEXT: &str = "next";
    const ID_PREV: &str = "prev";
    const ID_BROWSER: &str = "browser";
    const ID_QUIT: &str = "quit";
    const ID_SET_PREFIX: &str = "set:";

    pub struct Tray {
        icon: TrayIcon,
        rx: Receiver<TrayCmd>,
        /// Last state the menu was built from, to avoid rebuilding it every frame.
        state: Option<(Vec<String>, Option<String>)>,
    }

    impl Tray {
        /// Creates the status item. Menu clicks are forwarded to `rx` and wake the UI.
        pub fn new(ctx: egui::Context) -> Result<Self, String> {
            let (tx, rx) = mpsc::channel();
            MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
                if let Some(cmd) = parse_id(event.id.as_ref()) {
                    let _ = tx.send(cmd);
                    ctx.request_repaint();
                }
            }));

            let icon = TrayIconBuilder::new()
                .with_tooltip("Hostmask")
                .with_icon(template_icon())
                .with_icon_as_template(true)
                .build()
                .map_err(|e| e.to_string())?;
            Ok(Self { icon, rx, state: None })
        }

        /// Returns the next pending command, if any.
        pub fn try_recv(&self) -> Option<TrayCmd> {
            self.rx.try_recv().ok()
        }

        /// Forces the next `sync` to rebuild the menu. Needed because macOS toggles the
        /// checkmark of a clicked `CheckMenuItem` itself, even when the switch failed.
        pub fn invalidate(&mut self) {
            self.state = None;
        }

        /// Rebuilds the menu if the set list or the active set changed.
        pub fn sync(&mut self, names: &[String], active: Option<&str>) {
            let state = (names.to_vec(), active.map(str::to_owned));
            if self.state.as_ref() == Some(&state) {
                return;
            }
            self.icon.set_menu(Some(Box::new(build_menu(names, active))));
            // macOS shows this text next to the icon in the menu bar.
            self.icon.set_title(active);
            self.state = Some(state);
        }
    }

    fn build_menu(names: &[String], active: Option<&str>) -> Menu {
        let menu = Menu::new();
        let _ = menu.append(&MenuItem::with_id(ID_SHOW, "Открыть Hostmask", true, None));
        let _ = menu.append(&PredefinedMenuItem::separator());

        if names.is_empty() {
            let _ = menu.append(&MenuItem::new("Наборов нет", false, None));
        }
        for name in names {
            let checked = active == Some(name.as_str());
            let item = CheckMenuItem::with_id(format!("{ID_SET_PREFIX}{name}"), name, true, checked, None);
            let _ = menu.append(&item);
        }

        if !names.is_empty() {
            let _ = menu.append(&PredefinedMenuItem::separator());
            let _ = menu.append(&MenuItem::with_id(ID_NEXT, "Следующий набор", true, None));
            let _ = menu.append(&MenuItem::with_id(ID_PREV, "Предыдущий набор", true, None));
        }
        let _ = menu.append(&PredefinedMenuItem::separator());
        let _ = menu.append(&MenuItem::with_id(ID_BROWSER, "Сбросить кэш браузера…", true, None));
        let _ = menu.append(&PredefinedMenuItem::separator());
        let _ = menu.append(&MenuItem::with_id(ID_QUIT, "Выйти из Hostmask", true, None));
        menu
    }

    fn parse_id(id: &str) -> Option<TrayCmd> {
        match id {
            ID_SHOW => Some(TrayCmd::Show),
            ID_NEXT => Some(TrayCmd::Next),
            ID_PREV => Some(TrayCmd::Prev),
            ID_BROWSER => Some(TrayCmd::BrowserCache),
            ID_QUIT => Some(TrayCmd::Quit),
            other => other.strip_prefix(ID_SET_PREFIX).map(|n| TrayCmd::Activate(n.to_owned())),
        }
    }

    /// Black-on-transparent mask. Black with alpha is a macOS "template" image,
    /// so the system tints it for light and dark menu bars.
    fn template_icon() -> Icon {
        const SIZE: u32 = 36;
        let mut rgba = Vec::with_capacity((SIZE * SIZE * 4) as usize);
        for a in mask_alpha() {
            rgba.extend_from_slice(&[0, 0, 0, a]);
        }
        Icon::from_rgba(rgba, SIZE, SIZE).expect("icon dimensions match the buffer")
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn parses_menu_ids() {
            assert_eq!(parse_id("show"), Some(TrayCmd::Show));
            assert_eq!(parse_id("browser"), Some(TrayCmd::BrowserCache));
            assert_eq!(parse_id("quit"), Some(TrayCmd::Quit));
            assert_eq!(parse_id("set:Работа"), Some(TrayCmd::Activate("Работа".into())));
            assert_eq!(parse_id("unknown"), None);
        }
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use std::sync::mpsc::{self, Receiver};

    use eframe::egui;
    use ksni::blocking::{Handle, TrayMethods};
    use ksni::menu::{CheckmarkItem, MenuItem, StandardItem};
    use ksni::Icon;

    use super::TrayCmd;

    const SIZE: i32 = 36;

    /// The tray service's state. It lives on ksni's thread and is updated through `Handle`.
    struct HostTray {
        tx: mpsc::Sender<TrayCmd>,
        ctx: egui::Context,
        names: Vec<String>,
        active: Option<String>,
    }

    impl HostTray {
        /// Forwards a menu click to the UI and wakes it up, even while the window is hidden.
        fn send(&self, cmd: TrayCmd) {
            let _ = self.tx.send(cmd);
            self.ctx.request_repaint();
        }

        fn item(label: &str, cmd: TrayCmd) -> MenuItem<Self> {
            StandardItem {
                label: label.into(),
                activate: Box::new(move |t: &mut Self| t.send(cmd.clone())),
                ..Default::default()
            }
            .into()
        }
    }

    impl ksni::Tray for HostTray {
        fn id(&self) -> String {
            "io.github.hostmask.Hostmask".into()
        }

        fn title(&self) -> String {
            match &self.active {
                Some(name) => format!("Hostmask: {name}"),
                None => "Hostmask".into(),
            }
        }

        // Shown next to the icon, like the macOS menu bar title. Needs the patched ksni
        // in `vendor/ksni`; KDE ignores the label and shows the icon only.
        fn label(&self) -> String {
            self.active.clone().unwrap_or_default()
        }

        fn label_guide(&self) -> String {
            self.names.iter().max_by_key(|n| n.chars().count()).cloned().unwrap_or_default()
        }

        fn icon_pixmap(&self) -> Vec<Icon> {
            // SNI expects ARGB32 in network byte order, rather than PNG/RGBA.
            let icon = eframe::icon_data::from_png_bytes(include_bytes!(
                "../../../assets/icons/hostmask-36.png"
            )).expect("bundled tray icon is a valid PNG");
            let data = icon.rgba.chunks_exact(4)
                .flat_map(|p| [p[3], p[0], p[1], p[2]]).collect();
            vec![Icon { width: SIZE, height: SIZE, data }]
        }

        fn activate(&mut self, _x: i32, _y: i32) {
            self.send(TrayCmd::Show);
        }

        fn menu(&self) -> Vec<MenuItem<Self>> {
            let mut items = vec![Self::item("Открыть Hostmask", TrayCmd::Show), MenuItem::Separator];

            if self.names.is_empty() {
                items.push(MenuItem::Standard(StandardItem {
                    label: "Наборов нет".into(),
                    enabled: false,
                    ..Default::default()
                }));
            }
            for name in &self.names {
                let n = name.clone();
                items.push(
                    CheckmarkItem {
                        label: name.clone(),
                        checked: self.active.as_deref() == Some(name.as_str()),
                        activate: Box::new(move |t: &mut Self| t.send(TrayCmd::Activate(n.clone()))),
                        ..Default::default()
                    }
                    .into(),
                );
            }

            if !self.names.is_empty() {
                items.push(MenuItem::Separator);
                items.push(Self::item("Следующий набор", TrayCmd::Next));
                items.push(Self::item("Предыдущий набор", TrayCmd::Prev));
            }
            items.push(MenuItem::Separator);
            items.push(Self::item("Сбросить кэш браузера…", TrayCmd::BrowserCache));
            items.push(MenuItem::Separator);
            items.push(Self::item("Выйти из Hostmask", TrayCmd::Quit));
            items
        }
    }

    pub struct Tray {
        handle: Handle<HostTray>,
        rx: Receiver<TrayCmd>,
        /// Last state pushed to the tray service, to skip redundant updates.
        state: Option<(Vec<String>, Option<String>)>,
    }

    impl Tray {
        /// Starts the StatusNotifierItem service. Clicks are forwarded to `rx` and wake the UI.
        /// Fails when no D-Bus session or StatusNotifier host is available.
        pub fn new(ctx: egui::Context) -> Result<Self, String> {
            let (tx, rx) = mpsc::channel();
            let tray = HostTray { tx, ctx, names: Vec::new(), active: None };
            // Flatpak has no access to the bus name, so do not claim one.
            let handle = tray
                .disable_dbus_name(true)
                .spawn()
                .map_err(|e| e.to_string())?;
            Ok(Self { handle, rx, state: None })
        }

        /// Returns the next pending command, if any.
        pub fn try_recv(&self) -> Option<TrayCmd> {
            self.rx.try_recv().ok()
        }

        /// Forces the next `sync` to push the state again.
        pub fn invalidate(&mut self) {
            self.state = None;
        }

        /// Pushes the set list and the active set to the tray menu if they changed.
        pub fn sync(&mut self, names: &[String], active: Option<&str>) {
            let state = (names.to_vec(), active.map(str::to_owned));
            if self.state.as_ref() == Some(&state) {
                return;
            }
            self.handle.update(|t| {
                t.names = state.0.clone();
                t.active = state.1.clone();
            });
            self.state = Some(state);
        }
    }
}
