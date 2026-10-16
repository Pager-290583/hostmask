mod app;
mod background;
mod tray;

use std::path::PathBuf;

use hostmask_core::platform;
use hostmask_core::Manager;

fn main() -> eframe::Result {
    let root = std::env::var_os("HOSTMASK_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(platform::default_data_dir);

    let manager = match Manager::open(&root) {
        Ok(m) => m,
        Err(e) => {
            rfd::MessageDialog::new()
                .set_title("Hostmask")
                .set_description(format!("Не удалось открыть папку с наборами {}:\n{e}", root.display()))
                .set_level(rfd::MessageLevel::Error)
                .show();
            std::process::exit(1);
        }
    };
    // First start on this machine: begin with its own hosts file, not an empty list.
    if let Ok(system) = platform::read_system_hosts() {
        if let Err(e) = manager.seed_from_system(&system) {
            eprintln!("hostmask: could not save the system hosts file as a set: {e}");
        }
    }

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Hostmask")
            .with_inner_size([980.0, 640.0])
            .with_min_inner_size([680.0, 420.0]),
        // Menu-bar only: no Dock icon and no entry in the app switcher. The bundle's
        // Info.plist sets LSUIElement too, which covers the Finder launch.
        #[cfg(target_os = "macos")]
        event_loop_builder: Some(Box::new(|builder| {
            use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
            builder.with_activation_policy(ActivationPolicy::Accessory);
        })),
        // Wayland cannot hide a window (winit ignores `set_visible` there), so closing to the
        // tray would do nothing. Use X11 (XWayland) when it is available.
        #[cfg(target_os = "linux")]
        event_loop_builder: Some(Box::new(|builder| {
            if std::env::var_os("DISPLAY").is_some() {
                use winit::platform::x11::EventLoopBuilderExtX11;
                builder.with_x11();
            }
        })),
        ..Default::default()
    };

    eframe::run_native(
        "Hostmask",
        options,
        Box::new(move |cc| Ok(Box::new(app::HostmaskApp::new(cc, manager)))),
    )
}
