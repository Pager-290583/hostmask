//! High-level operations shared by the GUI and CLI.
//!
//! `Manager` keeps the set files, the remote-URL registry and the settings in sync.

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use crate::combine::{combine, CombinedSource};
use crate::remote::{self, FetchError, RemoteEntry, RemoteRegistry};
use crate::settings::Settings;
use crate::store::{HostsSet, SetKind, Store, StoreError};

/// Name of the set made from the machine's own hosts file on first start.
pub const INITIAL_SET: &str = "Исходный";

#[derive(Debug)]
pub enum ManagerError {
    Store(StoreError),
    Fetch(FetchError),
    Io(io::Error),
    /// The set exists but is not a remote set (or has no URL recorded).
    NotRemote(String),
}

impl fmt::Display for ManagerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ManagerError::Store(e) => write!(f, "{e}"),
            ManagerError::Fetch(e) => write!(f, "{e}"),
            ManagerError::Io(e) => write!(f, "I/O error: {e}"),
            ManagerError::NotRemote(n) => write!(f, "{n:?} is not a remote set"),
        }
    }
}

impl std::error::Error for ManagerError {}

impl From<StoreError> for ManagerError {
    fn from(e: StoreError) -> Self {
        ManagerError::Store(e)
    }
}

impl From<FetchError> for ManagerError {
    fn from(e: FetchError) -> Self {
        ManagerError::Fetch(e)
    }
}

impl From<io::Error> for ManagerError {
    fn from(e: io::Error) -> Self {
        ManagerError::Io(e)
    }
}

pub struct Manager {
    store: Store,
    remote: RemoteRegistry,
    settings: Settings,
}

impl Manager {
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, ManagerError> {
        let store = Store::open(root)?;
        let remote = RemoteRegistry::load(store.root());
        let settings = Settings::load(store.root());
        Ok(Self { store, remote, settings })
    }

    pub fn root(&self) -> &Path {
        self.store.root()
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    pub fn set_settings(&mut self, settings: Settings) -> Result<(), ManagerError> {
        settings.save(self.store.root())?;
        self.settings = settings;
        Ok(())
    }

    pub fn list(&self) -> Result<Vec<HostsSet>, ManagerError> {
        Ok(self.store.list()?)
    }

    pub fn read(&self, name: &str) -> Result<String, ManagerError> {
        Ok(self.store.read(name)?)
    }

    pub fn active(&self) -> Result<Option<String>, ManagerError> {
        Ok(self.store.active()?)
    }

    pub fn set_active(&self, name: Option<&str>) -> Result<(), ManagerError> {
        Ok(self.store.set_active(name)?)
    }

    pub fn neighbour(&self, step: i32) -> Result<Option<String>, ManagerError> {
        Ok(self.store.neighbour(step)?)
    }

    /// On first start (no sets and no active set), saves this machine's current hosts file as
    /// the set [`INITIAL_SET`] and marks it active. The user starts from their own hosts and can
    /// always switch back to it. Returns the created set's name, or `None` if there were sets.
    pub fn seed_from_system(&self, system_hosts: &str) -> Result<Option<String>, ManagerError> {
        if !self.store.list()?.is_empty() || self.store.active()?.is_some() {
            return Ok(None);
        }
        self.store.create(INITIAL_SET, system_hosts)?;
        self.store.set_active(Some(INITIAL_SET))?;
        Ok(Some(INITIAL_SET.to_owned()))
    }

    pub fn create(&self, name: &str, contents: &str) -> Result<(), ManagerError> {
        Ok(self.store.create(name, contents)?)
    }

    pub fn save(&self, name: &str, contents: &str) -> Result<(), ManagerError> {
        Ok(self.store.save(name, contents)?)
    }

    pub fn delete(&mut self, name: &str) -> Result<(), ManagerError> {
        self.store.delete(name)?;
        if self.remote.entries.remove(name).is_some() {
            self.remote.save(self.store.root())?;
        }
        Ok(())
    }

    pub fn rename(&mut self, from: &str, to: &str) -> Result<(), ManagerError> {
        self.store.rename(from, to)?;
        if let Some(entry) = self.remote.entries.remove(from) {
            self.remote.entries.insert(to.to_owned(), entry);
            self.remote.save(self.store.root())?;
        }
        Ok(())
    }

    /// The URL a remote set is downloaded from, if the set is remote.
    pub fn remote_entry(&self, name: &str) -> Option<&RemoteEntry> {
        self.remote.entries.get(name)
    }

    /// URL of a remote set, if `name` is one.
    pub fn remote_url(&self, name: &str) -> Option<String> {
        self.remote.entries.get(name).map(|e| e.url.clone())
    }

    /// Creates a remote set from text that was already downloaded from `url`.
    pub fn insert_remote(&mut self, name: &str, url: &str, contents: &str) -> Result<(), ManagerError> {
        self.store.create_kind(SetKind::Remote, name, contents)?;
        self.remote.entries.insert(
            name.to_owned(),
            RemoteEntry { url: url.to_owned(), last_updated: Some(remote::now_unix()) },
        );
        self.remote.save(self.store.root())?;
        Ok(())
    }

    /// Replaces a remote set's cached contents with freshly downloaded text.
    pub fn apply_remote(&mut self, name: &str, contents: &str) -> Result<(), ManagerError> {
        let entry = self
            .remote
            .entries
            .get_mut(name)
            .ok_or_else(|| ManagerError::NotRemote(name.into()))?;
        entry.last_updated = Some(remote::now_unix());
        self.store.save(name, contents)?;
        self.remote.save(self.store.root())?;
        Ok(())
    }

    /// Remote sets whose last download is older than the configured interval.
    pub fn remote_sets_due(&self, now: u64) -> Vec<String> {
        let minutes = self.settings.remote_update_minutes;
        if minutes == 0 {
            return Vec::new();
        }
        let max_age = minutes * 60;
        self.remote
            .entries
            .iter()
            .filter(|(_, e)| e.last_updated.map_or(true, |t| now.saturating_sub(t) >= max_age))
            .map(|(name, _)| name.clone())
            .collect()
    }

    /// Builds a Combined set from the given sets.
    pub fn combine(&self, out: &str, names: &[String]) -> Result<(), ManagerError> {
        let contents: Vec<(String, String)> = names
            .iter()
            .map(|n| self.store.read(n).map(|c| (n.clone(), c)))
            .collect::<Result<_, _>>()?;
        let sources: Vec<CombinedSource<'_>> = contents
            .iter()
            .map(|(name, c)| CombinedSource { name, contents: c })
            .collect();
        self.store.create_kind(SetKind::Combined, out, &combine(&sources))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manager() -> (tempfile::TempDir, Manager) {
        let dir = tempfile::tempdir().unwrap();
        let m = Manager::open(dir.path()).unwrap();
        (dir, m)
    }

    fn with_remote(m: &mut Manager, name: &str, last_updated: Option<u64>) {
        m.store.create_kind(SetKind::Remote, name, "0.0.0.0 x").unwrap();
        m.remote.entries.insert(
            name.to_owned(),
            RemoteEntry { url: format!("https://example.test/{name}"), last_updated },
        );
    }

    #[test]
    fn seeds_from_system_hosts_only_on_first_start() {
        let (_dir, m) = manager();
        let seeded = m.seed_from_system("127.0.0.1 localhost\n").unwrap();
        assert_eq!(seeded.as_deref(), Some(INITIAL_SET));
        assert_eq!(m.read(INITIAL_SET).unwrap(), "127.0.0.1 localhost\n");
        assert_eq!(m.active().unwrap().as_deref(), Some(INITIAL_SET));
        // Sets exist now, so a second start changes nothing.
        assert_eq!(m.seed_from_system("other").unwrap(), None);
        assert_eq!(m.read(INITIAL_SET).unwrap(), "127.0.0.1 localhost\n");
    }

    #[test]
    fn does_not_seed_when_user_has_sets() {
        let (_dir, m) = manager();
        m.create("work", "10.0.0.1 a").unwrap();
        assert_eq!(m.seed_from_system("127.0.0.1 localhost").unwrap(), None);
        assert!(m.read(INITIAL_SET).is_err());
    }

    #[test]
    fn rename_and_delete_keep_registry_in_sync() {
        let (_dir, mut m) = manager();
        with_remote(&mut m, "ads", Some(1));
        m.rename("ads", "ads2").unwrap();
        assert!(m.remote_entry("ads").is_none());
        assert!(m.remote_entry("ads2").is_some());
        m.delete("ads2").unwrap();
        assert!(m.remote_entry("ads2").is_none());
    }

    #[test]
    fn due_sets_follow_interval() {
        let (_dir, mut m) = manager();
        with_remote(&mut m, "fresh", Some(10_000));
        with_remote(&mut m, "stale", Some(0));
        with_remote(&mut m, "never", None);
        let now = 10_000 + 60; // 1 minute after "fresh" was updated
        let mut due = m.remote_sets_due(now);
        due.sort();
        assert_eq!(due, vec!["never".to_string(), "stale".to_string()]);

        m.set_settings(Settings { remote_update_minutes: 0, ..Settings::default() }).unwrap();
        assert!(m.remote_sets_due(now).is_empty());
    }

    #[test]
    fn combine_creates_combined_set() {
        let (_dir, m) = manager();
        m.create("a", "1.1.1.1 a").unwrap();
        m.create("b", "2.2.2.2 b").unwrap();
        m.combine("both", &["a".to_string(), "b".to_string()]).unwrap();
        let set = m.list().unwrap().into_iter().find(|s| s.name == "both").unwrap();
        assert_eq!(set.kind, SetKind::Combined);
        assert!(m.read("both").unwrap().contains("# Hosts File: b"));
    }
}
