//! Background worker that refreshes remote sets while the window is hidden.
//!
//! It runs on its own thread and re-reads settings and the registry from disk on every
//! pass, so it does not depend on the window being visible or on the UI's state.

use std::collections::HashMap;
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

use hostmask_core::remote;
use hostmask_core::Manager;

/// How often the worker checks whether any remote set is due.
const CHECK_EVERY: Duration = Duration::from_secs(30);
/// After a failed download, wait this long before retrying the same set.
const RETRY_AFTER: Duration = Duration::from_secs(10 * 60);

pub fn spawn_remote_updater(root: PathBuf) {
    thread::spawn(move || {
        let mut failed: HashMap<String, Instant> = HashMap::new();
        loop {
            refresh_due_sets(&root, &mut failed);
            thread::sleep(CHECK_EVERY);
        }
    });
}

fn refresh_due_sets(root: &PathBuf, failed: &mut HashMap<String, Instant>) {
    let Ok(mut manager) = Manager::open(root) else { return };
    let now = Instant::now();
    for name in manager.remote_sets_due(remote::now_unix()) {
        if failed.get(&name).is_some_and(|t| now.duration_since(*t) < RETRY_AFTER) {
            continue;
        }
        let Some(url) = manager.remote_url(&name) else { continue };
        match remote::fetch(&url) {
            Ok(text) => {
                failed.remove(&name);
                let _ = manager.apply_remote(&name, &text);
            }
            Err(_) => {
                failed.insert(name, now);
            }
        }
    }
}
