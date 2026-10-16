//! The Hostmask window: sidebar with sets, editor, dialogs and background jobs.

use std::collections::HashSet;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

use eframe::egui::{self, Align2, Button, Key, KeyboardShortcut, Modifiers, RichText, TextEdit, TextStyle};
use hostmask_core::platform::{self, PlatformError};
use hostmask_core::remote::{self, FetchError};
use hostmask_core::{HostsSet, Manager, SetKind, Settings};

use crate::background;
use crate::tray::{Tray, TrayCmd};

/// How often we re-read `/etc/hosts` to see whether it still matches the active set.
const SYSTEM_CHECK_EVERY: Duration = Duration::from_secs(3);

const SAVE_SHORTCUT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::S);
const NEXT_SHORTCUT: KeyboardShortcut =
    KeyboardShortcut::new(Modifiers { alt: true, command: true, ..Modifiers::NONE }, Key::ArrowRight);
const PREV_SHORTCUT: KeyboardShortcut =
    KeyboardShortcut::new(Modifiers { alt: true, command: true, ..Modifiers::NONE }, Key::ArrowLeft);

#[derive(Debug, Clone, PartialEq, Eq)]
enum Selection {
    None,
    /// The live `/etc/hosts`, read-only.
    System,
    Set(String),
}

/// Modal dialogs. Only one is open at a time.
enum Dialog {
    NewSet { name: String, contents: String, from_file: bool },
    Rename { from: String, to: String },
    Delete { name: String },
    AddRemote { name: String, url: String },
    Combine { out: String, picks: Vec<(String, bool)> },
    Settings { minutes: u64, watch: bool, keep_running: bool },
    /// The editor has unsaved changes and the user asked to switch sets.
    Unsaved { target: Selection },
    Message { title: String, text: String },
    /// Shown after a set is applied: the browser may still use its own DNS cache.
    BrowserHint { set: Option<String> },
}

/// Results of work done on background threads.
enum JobResult {
    Activated { name: String, result: Result<(), PlatformError> },
    Fetched { name: String, url: String, is_new: bool, result: Result<String, FetchError> },
}

pub struct HostmaskApp {
    manager: Manager,
    ctx: egui::Context,
    sets: Vec<HostsSet>,
    active: Option<String>,

    selection: Selection,
    /// Text in the editor (may differ from the saved set when dirty).
    editor: String,
    /// Last saved text of the selected set.
    saved: String,
    /// Live `/etc/hosts`, refreshed while it is selected.
    system_text: String,
    /// `/etc/hosts` differs from the active set.
    mismatch: bool,

    /// Set while an activation (which needs admin rights) is running.
    busy: Option<String>,
    /// Remote sets currently being downloaded.
    in_flight: HashSet<String>,

    dialog: Option<Dialog>,
    /// Error shown inside the open dialog (e.g. "name already taken").
    dialog_error: Option<String>,
    /// Last status message; `true` means error.
    status: Option<(String, bool)>,

    tx: Sender<JobResult>,
    rx: Receiver<JobResult>,
    last_system_check: Option<Instant>,

    /// Status-bar icon; `None` if the platform refused to create one.
    tray: Option<Tray>,
    /// Set when the user chose Quit, so closing the window really exits.
    quitting: bool,
}

impl HostmaskApp {
    pub fn new(cc: &eframe::CreationContext<'_>, manager: Manager) -> Self {
        let (tx, rx) = mpsc::channel();
        background::spawn_remote_updater(manager.root().to_path_buf());
        let tray = match Tray::new(cc.egui_ctx.clone()) {
            Ok(t) => Some(t),
            Err(e) => {
                eprintln!("hostmask: status bar icon unavailable: {e}");
                None
            }
        };
        let mut app = Self {
            manager,
            ctx: cc.egui_ctx.clone(),
            sets: Vec::new(),
            active: None,
            selection: Selection::None,
            editor: String::new(),
            saved: String::new(),
            system_text: String::new(),
            mismatch: false,
            busy: None,
            in_flight: HashSet::new(),
            dialog: None,
            dialog_error: None,
            status: None,
            tx,
            rx,
            last_system_check: None,
            tray,
            quitting: false,
        };
        app.reload();
        if let Some(active) = app.active.clone() {
            app.load_selection(Selection::Set(active));
        }
        app
    }

    // ----- state helpers -------------------------------------------------

    fn reload(&mut self) {
        self.sets = self.manager.list().unwrap_or_default();
        self.active = self.manager.active().ok().flatten();
        if let Selection::Set(name) = &self.selection {
            if !self.sets.iter().any(|s| &s.name == name) {
                self.selection = Selection::None;
                self.editor.clear();
                self.saved.clear();
            }
        }
        self.sync_tray();
    }

    /// Rebuilds the status-bar menu even if nothing in the state changed.
    fn refresh_tray(&mut self) {
        if let Some(tray) = &mut self.tray {
            tray.invalidate();
        }
        self.sync_tray();
    }

    fn sync_tray(&mut self) {
        let names: Vec<String> = self
            .sets
            .iter()
            .filter(|s| s.kind != SetKind::Combined)
            .map(|s| s.name.clone())
            .collect();
        let active = self.active.clone();
        if let Some(tray) = &mut self.tray {
            tray.sync(&names, active.as_deref());
        }
    }

    fn selected_name(&self) -> Option<&str> {
        match &self.selection {
            Selection::Set(n) => Some(n),
            _ => None,
        }
    }

    fn dirty(&self) -> bool {
        matches!(self.selection, Selection::Set(_)) && self.editor != self.saved
    }

    fn set_kind(&self, name: &str) -> Option<SetKind> {
        self.sets.iter().find(|s| s.name == name).map(|s| s.kind)
    }

    fn info(&mut self, text: impl Into<String>) {
        self.status = Some((text.into(), false));
    }

    fn error(&mut self, text: impl Into<String>) {
        self.status = Some((text.into(), true));
    }

    /// Switches the editor to `target`, asking first if there are unsaved edits.
    fn request_selection(&mut self, target: Selection) {
        if target == self.selection {
            return;
        }
        if self.dirty() {
            self.open_dialog(Dialog::Unsaved { target });
        } else {
            self.load_selection(target);
        }
    }

    fn load_selection(&mut self, target: Selection) {
        match &target {
            Selection::Set(name) => match self.manager.read(name) {
                Ok(text) => {
                    self.editor = text.clone();
                    self.saved = text;
                }
                Err(e) => {
                    self.error(e.to_string());
                    return;
                }
            },
            Selection::System => self.refresh_system_text(),
            Selection::None => {
                self.editor.clear();
                self.saved.clear();
            }
        }
        self.selection = target;
    }

    fn refresh_system_text(&mut self) {
        self.system_text = platform::read_system_hosts().unwrap_or_else(|e| format!("Не удалось прочитать /etc/hosts: {e}"));
    }

    fn open_dialog(&mut self, dialog: Dialog) {
        self.dialog = Some(dialog);
        self.dialog_error = None;
    }

    // ----- actions -------------------------------------------------------

    /// Writes the editor text to the selected set.
    fn save_selected(&mut self) -> bool {
        let Some(name) = self.selected_name().map(str::to_owned) else { return false };
        match self.manager.save(&name, &self.editor) {
            Ok(()) => {
                self.saved = self.editor.clone();
                self.info(format!("Сохранено: «{name}»"));
                true
            }
            Err(e) => {
                self.error(e.to_string());
                false
            }
        }
    }

    /// Makes `name` the active set: writes it to `/etc/hosts` on a background thread.
    fn activate(&mut self, name: &str) {
        if self.busy.is_some() {
            return;
        }
        // Activation installs what is on disk, so write pending edits first.
        if self.selected_name() == Some(name) && self.dirty() && !self.save_selected() {
            return;
        }
        let contents = match self.manager.read(name) {
            Ok(c) => c,
            Err(e) => return self.error(e.to_string()),
        };
        self.busy = Some(format!("Применяю «{name}». Может появиться запрос пароля администратора…"));
        let tx = self.tx.clone();
        let ctx = self.ctx.clone();
        let name = name.to_owned();
        thread::spawn(move || {
            let result = platform::install_system_hosts(&contents);
            let _ = tx.send(JobResult::Activated { name, result });
            ctx.request_repaint();
        });
    }

    fn switch_relative(&mut self, step: i32) {
        match self.manager.neighbour(step) {
            Ok(Some(name)) => self.activate(&name),
            Ok(None) => self.info("Нет наборов для переключения"),
            Err(e) => self.error(e.to_string()),
        }
    }

    /// Downloads a remote set in the background. `is_new` means create it, otherwise update it.
    fn fetch_remote(&mut self, name: &str, url: &str, is_new: bool) {
        if !self.in_flight.insert(name.to_owned()) {
            return;
        }
        let tx = self.tx.clone();
        let ctx = self.ctx.clone();
        let (name, url) = (name.to_owned(), url.to_owned());
        thread::spawn(move || {
            let result = remote::fetch(&url);
            let _ = tx.send(JobResult::Fetched { name, url, is_new, result });
            ctx.request_repaint();
        });
    }

    fn delete_set(&mut self, name: &str) {
        match self.manager.delete(name) {
            Ok(()) => {
                if self.selected_name() == Some(name) {
                    self.selection = Selection::None;
                    self.editor.clear();
                    self.saved.clear();
                }
                self.reload();
                self.info(format!("Удалён набор «{name}»"));
            }
            Err(e) => self.error(e.to_string()),
        }
    }

    // ----- background plumbing -------------------------------------------

    fn poll_jobs(&mut self) {
        while let Ok(job) = self.rx.try_recv() {
            match job {
                JobResult::Activated { name, result } => {
                    self.busy = None;
                    self.refresh_tray();
                    match result {
                        Ok(()) => {
                            if let Err(e) = self.manager.set_active(Some(&name)) {
                                self.error(e.to_string());
                            } else {
                                self.info(format!("Активен набор «{name}»"));
                            }
                            self.reload();
                            self.mismatch = false;
                            self.show_browser_hint(Some(name.clone()));
                        }
                        Err(PlatformError::Cancelled) => {
                            self.error("Отменено. /etc/hosts не изменён.");
                        }
                        Err(e) => {
                            self.open_dialog(Dialog::Message {
                                title: "Не удалось применить набор".into(),
                                text: e.to_string(),
                            });
                        }
                    }
                }
                JobResult::Fetched { name, url, is_new, result } => {
                    self.in_flight.remove(&name);
                    match result {
                        Ok(text) => {
                            let outcome = if is_new {
                                self.manager.insert_remote(&name, &url, &text)
                            } else {
                                self.manager.apply_remote(&name, &text)
                            };
                            match outcome {
                                Ok(()) => {
                                    self.reload();
                                    if self.selected_name() == Some(name.as_str()) && !self.dirty() {
                                        self.load_selection(Selection::Set(name.clone()));
                                    }
                                    self.info(format!("Обновлён удалённый набор «{name}»"));
                                }
                                Err(e) => self.error(e.to_string()),
                            }
                        }
                        Err(e) => {
                            if is_new {
                                self.open_dialog(Dialog::Message {
                                    title: "Не удалось загрузить набор".into(),
                                    text: e.to_string(),
                                });
                            } else {
                                self.error(format!("Обновление «{name}» не удалось: {e}"));
                            }
                        }
                    }
                }
            }
        }
    }

    /// Work done on every UI tick while the window is visible.
    /// Remote updates happen in `background`, so they also run while the window is hidden.
    fn periodic(&mut self) {
        let now = Instant::now();
        if self.last_system_check.map_or(true, |t| now.duration_since(t) >= SYSTEM_CHECK_EVERY) {
            self.last_system_check = Some(now);
            if self.manager.settings().watch_system_hosts {
                self.check_system();
            }
            self.reload_remote_text();
        }
    }

    /// Picks up contents that the background updater wrote to a remote set we are showing.
    fn reload_remote_text(&mut self) {
        let Selection::Set(name) = self.selection.clone() else { return };
        if self.dirty() || self.manager.remote_url(&name).is_none() {
            return;
        }
        if let Ok(text) = self.manager.read(&name) {
            if text != self.saved {
                self.editor = text.clone();
                self.saved = text;
            }
        }
    }

    /// Handles tray menu commands. Runs on every tick, including while the window is hidden.
    fn poll_tray(&mut self, ctx: &egui::Context) {
        while let Some(cmd) = self.tray.as_ref().and_then(Tray::try_recv) {
            match cmd {
                TrayCmd::Show => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
                TrayCmd::Next => self.switch_relative(1),
                TrayCmd::Prev => self.switch_relative(-1),
                TrayCmd::Activate(name) => self.activate(&name),
                TrayCmd::BrowserCache => {
                    let set = self.active.clone();
                    self.show_browser_hint(set);
                }
                TrayCmd::Quit => {
                    self.quitting = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }
    }

    /// Shows the window with the advice to clear the browser's DNS cache and socket pools.
    fn show_browser_hint(&mut self, set: Option<String>) {
        self.ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
        self.ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        self.open_dialog(Dialog::BrowserHint { set });
    }

    /// Closing the window hides it and keeps the app in the status bar,
    /// unless the setting is off, there is no status item, or the user chose Quit.
    fn handle_close(&mut self, ctx: &egui::Context) {
        let close_requested = ctx.input(|i| i.viewport().close_requested());
        if !close_requested || self.quitting {
            return;
        }
        let keep = self.tray.is_some() && self.manager.settings().keep_running_in_background;
        if keep {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        }
    }

    fn check_system(&mut self) {
        if self.selection == Selection::System {
            self.refresh_system_text();
        }
        // While an activation is in progress /etc/hosts is about to change; don't flag it.
        if self.busy.is_some() {
            return;
        }
        let Some(active) = self.active.clone() else {
            self.mismatch = false;
            return;
        };
        let system = platform::read_system_hosts().unwrap_or_default();
        let wanted = self.manager.read(&active).unwrap_or_default();
        self.mismatch = normalize(&system) != normalize(&wanted);
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.input_mut(|i| i.consume_shortcut(&SAVE_SHORTCUT)) && self.dirty() {
            self.save_selected();
        }
        if ctx.input_mut(|i| i.consume_shortcut(&NEXT_SHORTCUT)) {
            self.switch_relative(1);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&PREV_SHORTCUT)) {
            self.switch_relative(-1);
        }
    }

    // ----- layout --------------------------------------------------------

    fn sidebar(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        ui.heading("Наборы");
        ui.separator();

        let mut clicked: Option<Selection> = None;
        egui::ScrollArea::vertical().auto_shrink([false, true]).show(ui, |ui| {
            if ui
                .selectable_label(self.selection == Selection::System, "/etc/hosts (системный)")
                .clicked()
            {
                clicked = Some(Selection::System);
            }
            ui.add_space(6.0);

            let groups = [
                (SetKind::Local, "Локальные"),
                (SetKind::Remote, "Удалённые"),
                (SetKind::Combined, "Комбинированные"),
            ];
            for (kind, title) in groups {
                let names: Vec<&str> = self
                    .sets
                    .iter()
                    .filter(|s| s.kind == kind)
                    .map(|s| s.name.as_str())
                    .collect();
                if names.is_empty() {
                    continue;
                }
                ui.label(RichText::new(title).small().weak());
                for name in names {
                    let is_active = self.active.as_deref() == Some(name);
                    let text = if is_active { format!("● {name}") } else { format!("   {name}") };
                    let selected = self.selection == Selection::Set(name.to_owned());
                    if ui.selectable_label(selected, text).clicked() {
                        clicked = Some(Selection::Set(name.to_owned()));
                    }
                }
                ui.add_space(6.0);
            }
        });
        if let Some(target) = clicked {
            self.request_selection(target);
        }

        ui.separator();
        if ui.button("Новый набор").clicked() {
            self.open_dialog(Dialog::NewSet { name: String::new(), contents: String::new(), from_file: false });
        }
        if ui.button("Из файла…").clicked() {
            if let Some(path) = rfd::FileDialog::new().add_filter("Hosts", &["hst", "txt", "hosts"]).pick_file() {
                match std::fs::read_to_string(&path) {
                    Ok(contents) => {
                        let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_owned();
                        self.open_dialog(Dialog::NewSet { name, contents, from_file: true });
                    }
                    Err(e) => self.error(format!("Не удалось прочитать файл: {e}")),
                }
            }
        }
        if ui.button("Удалённый набор").clicked() {
            self.open_dialog(Dialog::AddRemote { name: String::new(), url: String::new() });
        }
        if ui.button("Объединить…").clicked() {
            let picks = self
                .sets
                .iter()
                .filter(|s| s.kind != SetKind::Combined)
                .map(|s| (s.name.clone(), false))
                .collect();
            self.open_dialog(Dialog::Combine { out: String::new(), picks });
        }
        if ui.button("Настройки").clicked() {
            let s = self.manager.settings();
            self.open_dialog(Dialog::Settings {
                minutes: s.remote_update_minutes,
                watch: s.watch_system_hosts,
                keep_running: s.keep_running_in_background,
            });
        }
    }

    fn main_panel(&mut self, ui: &mut egui::Ui) {
        match self.selection.clone() {
            Selection::None => {
                ui.vertical_centered(|ui| {
                    ui.add_space(ui.available_height() / 3.0);
                    ui.label("Выберите набор слева или создайте новый.");
                });
            }
            Selection::System => {
                ui.heading("/etc/hosts");
                ui.label(RichText::new("Только просмотр. Чтобы изменить системный файл, активируйте набор.").weak());
                ui.separator();
                let mut text = self.system_text.clone();
                egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
                    ui.add(
                        TextEdit::multiline(&mut text)
                            .font(TextStyle::Monospace)
                            .interactive(false)
                            .desired_width(f32::INFINITY),
                    );
                });
            }
            Selection::Set(name) => self.set_view(ui, &name),
        }
    }

    fn set_view(&mut self, ui: &mut egui::Ui, name: &str) {
        let kind = self.set_kind(name);
        let is_active = self.active.as_deref() == Some(name);
        let dirty = self.dirty();

        ui.horizontal(|ui| {
            ui.heading(name);
            if is_active {
                ui.label(RichText::new("активен").color(ui.visuals().hyperlink_color));
            }
            if dirty {
                ui.label(RichText::new("● не сохранено").color(ui.visuals().warn_fg_color));
            }
        });
        if let Some(url) = self.manager.remote_url(name) {
            ui.label(RichText::new(url).small().weak());
        } else if let Some(kind) = kind {
            ui.label(RichText::new(match kind {
                SetKind::Local => "Локальный набор",
                SetKind::Remote => "Удалённый набор",
                SetKind::Combined => "Комбинированный набор",
            }).small().weak());
        }

        ui.add_space(4.0);
        let busy = self.busy.is_some();
        ui.horizontal_wrapped(|ui| {
            if ui.add_enabled(!busy && !is_active, Button::new("Сделать активным")).clicked() {
                self.activate(name);
            }
            if ui.add_enabled(dirty, Button::new("Сохранить")).clicked() {
                self.save_selected();
            }
            if ui.add_enabled(dirty, Button::new("Отменить правки")).clicked() {
                self.editor = self.saved.clone();
            }
            if ui.button("Переименовать…").clicked() {
                self.open_dialog(Dialog::Rename { from: name.to_owned(), to: name.to_owned() });
            }
            if self.manager.remote_url(name).is_some() && ui.button("Обновить").clicked() {
                if let Some(url) = self.manager.remote_url(name) {
                    self.fetch_remote(name, &url, false);
                }
            }
            if ui.button("Удалить…").clicked() {
                self.open_dialog(Dialog::Delete { name: name.to_owned() });
            }
        });

        if self.mismatch && self.active.is_some() {
            ui.add_space(6.0);
            let active = self.active.clone().unwrap_or_default();
            ui.horizontal_wrapped(|ui| {
                ui.colored_label(
                    ui.visuals().warn_fg_color,
                    format!("⚠ /etc/hosts отличается от активного набора «{active}» (его мог изменить другой программой)."),
                );
                if ui.add_enabled(!busy, Button::new(format!("Применить «{active}»"))).clicked() {
                    self.activate(&active);
                }
            });
        }

        ui.separator();
        let edit_name = name.to_owned();
        egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
            let response = ui.add_sized(
                ui.available_size(),
                TextEdit::multiline(&mut self.editor)
                    .font(TextStyle::Monospace)
                    .code_editor()
                    .desired_width(f32::INFINITY)
                    .hint_text(format!("Содержимое набора «{edit_name}»")),
            );
            let _ = response;
        });
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if let Some(busy) = &self.busy {
                ui.spinner();
                ui.label(busy.as_str());
            } else if let Some((text, is_error)) = &self.status {
                let color = if *is_error { ui.visuals().error_fg_color } else { ui.visuals().text_color() };
                ui.colored_label(color, text.as_str());
            }
            if let Some(name) = &self.active {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new(format!("Активен: {name}")).small().weak());
                });
            }
        });
    }

    // ----- dialogs -------------------------------------------------------

    fn show_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut dialog) = self.dialog.take() else { return };
        let mut close = false;
        let error = self.dialog_error.clone();

        match &mut dialog {
            Dialog::NewSet { name, contents, from_file } => {
                let title = if *from_file { "Новый набор из файла" } else { "Новый набор" };
                let lines = contents.lines().count();
                window(ctx, title).show(ctx, |ui| {
                    ui.label("Название:");
                    ui.text_edit_singleline(name);
                    if *from_file {
                        ui.label(RichText::new(format!("Строк в файле: {lines}")).weak());
                    }
                    footer(ui, &error, |ui| {
                        let valid = !name.trim().is_empty();
                        if ui.add_enabled(valid, Button::new("Создать")).clicked() {
                            match self.manager.create(name.trim(), contents) {
                                Ok(()) => {
                                    self.reload();
                                    self.info(format!("Создан набор «{}»", name.trim()));
                                    self.load_selection(Selection::Set(name.trim().to_owned()));
                                    close = true;
                                }
                                Err(e) => self.dialog_error = Some(e.to_string()),
                            }
                        }
                        if ui.button("Отмена").clicked() {
                            close = true;
                        }
                    });
                });
            }

            Dialog::Rename { from, to } => {
                window(ctx, "Переименовать набор").show(ctx, |ui| {
                    ui.label(format!("«{from}» →"));
                    ui.text_edit_singleline(to);
                    footer(ui, &error, |ui| {
                        if ui.add_enabled(!to.trim().is_empty(), Button::new("Переименовать")).clicked() {
                            match self.manager.rename(from, to.trim()) {
                                Ok(()) => {
                                    if self.selection == Selection::Set(from.clone()) {
                                        self.selection = Selection::Set(to.trim().to_owned());
                                    }
                                    self.reload();
                                    close = true;
                                }
                                Err(e) => self.dialog_error = Some(e.to_string()),
                            }
                        }
                        if ui.button("Отмена").clicked() {
                            close = true;
                        }
                    });
                });
            }

            Dialog::Delete { name } => {
                window(ctx, "Удалить набор?").show(ctx, |ui| {
                    ui.label(format!("Набор «{name}» будет удалён вместе с файлом."));
                    footer(ui, &error, |ui| {
                        if ui.add(Button::new(RichText::new("Удалить").color(ui.visuals().error_fg_color))).clicked() {
                            let name = name.clone();
                            self.delete_set(&name);
                            close = true;
                        }
                        if ui.button("Отмена").clicked() {
                            close = true;
                        }
                    });
                });
            }

            Dialog::AddRemote { name, url } => {
                window(ctx, "Удалённый набор").show(ctx, |ui| {
                    ui.label("Название:");
                    ui.text_edit_singleline(name);
                    ui.label("Адрес (http/https):");
                    ui.text_edit_singleline(url);
                    ui.label(RichText::new("Набор будет обновляться автоматически по интервалу из настроек.").small().weak());
                    footer(ui, &error, |ui| {
                        let valid = !name.trim().is_empty() && !url.trim().is_empty();
                        if ui.add_enabled(valid, Button::new("Загрузить")).clicked() {
                            if self.manager.list().unwrap_or_default().iter().any(|s| s.name == name.trim()) {
                                self.dialog_error = Some(format!("Набор «{}» уже существует", name.trim()));
                            } else {
                                self.fetch_remote(name.trim(), url.trim(), true);
                                self.info(format!("Загружаю «{}»…", name.trim()));
                                close = true;
                            }
                        }
                        if ui.button("Отмена").clicked() {
                            close = true;
                        }
                    });
                });
            }

            Dialog::Combine { out, picks } => {
                window(ctx, "Объединить наборы").show(ctx, |ui| {
                    ui.label("Наборы, которые войдут в комбинированный файл:");
                    egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                        for (name, picked) in picks.iter_mut() {
                            ui.checkbox(picked, name.as_str());
                        }
                    });
                    ui.separator();
                    ui.label("Название результата:");
                    ui.text_edit_singleline(out);
                    let chosen: Vec<String> = picks.iter().filter(|(_, p)| *p).map(|(n, _)| n.clone()).collect();
                    footer(ui, &error, |ui| {
                        let valid = !out.trim().is_empty() && !chosen.is_empty();
                        if ui.add_enabled(valid, Button::new("Создать")).clicked() {
                            match self.manager.combine(out.trim(), &chosen) {
                                Ok(()) => {
                                    self.reload();
                                    self.load_selection(Selection::Set(out.trim().to_owned()));
                                    close = true;
                                }
                                Err(e) => self.dialog_error = Some(e.to_string()),
                            }
                        }
                        if ui.button("Отмена").clicked() {
                            close = true;
                        }
                    });
                });
            }

            Dialog::Settings { minutes, watch, keep_running } => {
                window(ctx, "Настройки").show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Обновлять удалённые наборы каждые");
                        ui.add(egui::DragValue::new(minutes).range(0..=10_080).suffix(" мин"));
                    });
                    ui.label(RichText::new("0 — не обновлять автоматически.").small().weak());
                    ui.add_space(6.0);
                    ui.checkbox(watch, "Предупреждать, если /etc/hosts изменён другой программой");
                    ui.checkbox(keep_running, "При закрытии окна оставлять Hostmask в строке меню");
                    footer(ui, &error, |ui| {
                        if ui.button("Сохранить").clicked() {
                            let settings = Settings {
                                remote_update_minutes: *minutes,
                                watch_system_hosts: *watch,
                                keep_running_in_background: *keep_running,
                            };
                            match self.manager.set_settings(settings) {
                                Ok(()) => {
                                    self.info("Настройки сохранены");
                                    close = true;
                                }
                                Err(e) => self.dialog_error = Some(e.to_string()),
                            }
                        }
                        if ui.button("Отмена").clicked() {
                            close = true;
                        }
                    });
                });
            }

            Dialog::Unsaved { target } => {
                window(ctx, "Несохранённые изменения").show(ctx, |ui| {
                    ui.label("В наборе есть правки, которые не сохранены.");
                    footer(ui, &error, |ui| {
                        if ui.button("Сохранить").clicked() {
                            if self.save_selected() {
                                self.load_selection(target.clone());
                                close = true;
                            }
                        }
                        if ui.button("Не сохранять").clicked() {
                            self.load_selection(target.clone());
                            close = true;
                        }
                        if ui.button("Отмена").clicked() {
                            close = true;
                        }
                    });
                });
            }

            Dialog::Message { title, text } => {
                window(ctx, title.as_str()).show(ctx, |ui| {
                    ui.label(text.as_str());
                    footer(ui, &None, |ui| {
                        if ui.button("OK").clicked() {
                            close = true;
                        }
                    });
                });
            }
            Dialog::BrowserHint { set } => {
                let what = match set {
                    Some(name) => format!("Набор «{name}» применён в системе."),
                    None => "Набор применён в системе.".to_owned(),
                };
                let mut open_error = None;
                window(ctx, "Сбросьте кэш браузера").show(ctx, |ui| {
                    ui.label(what);
                    ui.label("Chrome хранит собственный DNS-кэш и открытые соединения, поэтому может продолжать ходить по старым адресам.");
                    ui.add_space(4.0);
                    ui.label("1. chrome://net-internals/#dns → Clear host cache");
                    ui.label("2. chrome://net-internals/#sockets → Close idle sockets");
                    ui.label("Если не помогло: Flush socket pools (закроет и активные соединения, сохраните работу).");
                    footer(ui, &error, |ui| {
                        if ui.button("Открыть net-internals").clicked() {
                            if let Err(e) = platform::open_chrome_net_internals() {
                                open_error = Some(format!("Не удалось открыть Chrome: {e}"));
                            }
                        }
                        if ui.button("OK").clicked() {
                            close = true;
                        }
                    });
                });
                if let Some(e) = open_error {
                    self.dialog_error = Some(e);
                }
            }
        }

        if close {
            self.dialog_error = None;
        } else {
            self.dialog = Some(dialog);
        }
    }
}

impl eframe::App for HostmaskApp {
    // `logic` runs even while the window is hidden; `ui` does not. Tray commands must be
    // handled here, otherwise "Открыть Hostmask" can never bring the hidden window back.
    // Activation results come from background threads and must be handled while hidden too,
    // otherwise `busy` stays set and later tray switches are silently ignored.
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_tray(ctx);
        self.poll_jobs();
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.handle_close(&ctx);
        self.handle_shortcuts(&ctx);
        self.periodic();

        egui::Panel::bottom("status").show(ui, |ui| self.status_bar(ui));
        egui::Panel::left("sidebar")
            .resizable(true)
            .default_size(250.0)
            .show(ui, |ui| self.sidebar(ui));
        egui::CentralPanel::default().show(ui, |ui| self.main_panel(ui));

        self.show_dialog(&ctx);

        // Keep the periodic checks running even when the window is idle.
        ctx.request_repaint_after(Duration::from_secs(1));
    }
}

/// A dialog window centred on the screen.
fn window<'a>(_ctx: &egui::Context, title: &'a str) -> egui::Window<'a> {
    egui::Window::new(title)
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        .default_width(360.0)
}

/// Standard dialog footer: an optional error line and the buttons.
fn footer(ui: &mut egui::Ui, error: &Option<String>, buttons: impl FnOnce(&mut egui::Ui)) {
    ui.add_space(8.0);
    if let Some(err) = error {
        ui.colored_label(ui.visuals().error_fg_color, err.as_str());
    }
    ui.horizontal(|ui| buttons(ui));
}

/// Normalises a hosts file for comparison: line endings and trailing blank lines don't count.
fn normalize(text: &str) -> String {
    text.replace("\r\n", "\n").trim_end().to_owned()
}

#[cfg(test)]
mod tests {
    use super::normalize;

    #[test]
    fn ignores_trailing_whitespace_and_crlf() {
        assert_eq!(normalize("a\r\nb\n\n"), normalize("a\nb"));
        assert_ne!(normalize("a\nb"), normalize("a\nc"));
    }
}
