//! User settings, stored as JSON in the data directory.

use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::Path;

const FILE: &str = "settings.json";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// How often remote sets are re-downloaded, in minutes. `0` disables it.
    pub remote_update_minutes: u64,
    /// Show a warning when `/etc/hosts` differs from the active set.
    pub watch_system_hosts: bool,
    /// Closing the window hides it and keeps the app in the menu bar.
    pub keep_running_in_background: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self { remote_update_minutes: 60, watch_system_hosts: true, keep_running_in_background: true }
    }
}

impl Settings {
    /// Loads settings from `root`, falling back to defaults if the file is missing or broken.
    pub fn load(root: &Path) -> Self {
        fs::read_to_string(root.join(FILE))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, root: &Path) -> io::Result<()> {
        let json = serde_json::to_string_pretty(self).map_err(io::Error::other)?;
        fs::write(root.join(FILE), json)
    }
}
