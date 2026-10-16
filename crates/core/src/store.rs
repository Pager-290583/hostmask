//! On-disk storage of hosts sets.
//!
//! Layout (same as GasMask):
//! ```text
//! <root>/Local/<name>.hst     user-written sets
//! <root>/Remote/<name>.hst    sets downloaded from a URL (cache)
//! <root>/Combined/<name>.hst  generated combinations
//! <root>/active               name of the currently active set (plain text)
//! ```

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const EXT: &str = "hst";
const ACTIVE_FILE: &str = "active";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetKind {
    Local,
    Remote,
    Combined,
}

impl SetKind {
    /// Subdirectory name used on disk.
    pub fn dir_name(self) -> &'static str {
        match self {
            SetKind::Local => "Local",
            SetKind::Remote => "Remote",
            SetKind::Combined => "Combined",
        }
    }

    const ALL: [SetKind; 3] = [SetKind::Local, SetKind::Remote, SetKind::Combined];
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostsSet {
    pub name: String,
    pub kind: SetKind,
}

#[derive(Debug)]
pub enum StoreError {
    InvalidName(String),
    NotFound(String),
    AlreadyExists(String),
    Io(io::Error),
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StoreError::InvalidName(n) => write!(f, "invalid set name: {n:?}"),
            StoreError::NotFound(n) => write!(f, "no hosts set named {n:?}"),
            StoreError::AlreadyExists(n) => write!(f, "hosts set {n:?} already exists"),
            StoreError::Io(e) => write!(f, "I/O error: {e}"),
        }
    }
}

impl std::error::Error for StoreError {}

impl From<io::Error> for StoreError {
    fn from(e: io::Error) -> Self {
        StoreError::Io(e)
    }
}

pub struct Store {
    root: PathBuf,
}

impl Store {
    /// Opens (and creates if needed) a store rooted at `root`.
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, StoreError> {
        let root = root.into();
        for kind in SetKind::ALL {
            fs::create_dir_all(root.join(kind.dir_name()))?;
        }
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn path_of(&self, kind: SetKind, name: &str) -> PathBuf {
        self.root.join(kind.dir_name()).join(format!("{name}.{EXT}"))
    }

    /// Finds a set by name, preferring Local over Remote over Combined.
    pub fn find(&self, name: &str) -> Option<HostsSet> {
        SetKind::ALL
            .into_iter()
            .find(|&kind| self.path_of(kind, name).is_file())
            .map(|kind| HostsSet { name: name.to_owned(), kind })
    }

    /// Lists sets of all kinds, sorted by name (case-insensitive).
    pub fn list(&self) -> Result<Vec<HostsSet>, StoreError> {
        let mut out = Vec::new();
        for kind in SetKind::ALL {
            for entry in fs::read_dir(self.root.join(kind.dir_name()))? {
                let path = entry?.path();
                if path.extension().and_then(|e| e.to_str()) != Some(EXT) {
                    continue;
                }
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    out.push(HostsSet { name: stem.to_owned(), kind });
                }
            }
        }
        out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        Ok(out)
    }

    pub fn read(&self, name: &str) -> Result<String, StoreError> {
        let set = self.find(name).ok_or_else(|| StoreError::NotFound(name.into()))?;
        Ok(fs::read_to_string(self.path_of(set.kind, name))?)
    }

    /// Creates a new Local set. Fails if the name is taken.
    pub fn create(&self, name: &str, contents: &str) -> Result<(), StoreError> {
        self.create_kind(SetKind::Local, name, contents)
    }

    /// Creates a new set of the given kind. Fails if the name is taken.
    pub fn create_kind(&self, kind: SetKind, name: &str, contents: &str) -> Result<(), StoreError> {
        validate_name(name)?;
        if self.find(name).is_some() {
            return Err(StoreError::AlreadyExists(name.into()));
        }
        fs::write(self.path_of(kind, name), contents)?;
        Ok(())
    }

    /// Overwrites a set's contents in place. Remote sets are kept as cache,
    /// so saving one is allowed but will be replaced on the next update.
    pub fn save(&self, name: &str, contents: &str) -> Result<(), StoreError> {
        let set = self.find(name).ok_or_else(|| StoreError::NotFound(name.into()))?;
        fs::write(self.path_of(set.kind, name), contents)?;
        Ok(())
    }

    pub fn delete(&self, name: &str) -> Result<(), StoreError> {
        let set = self.find(name).ok_or_else(|| StoreError::NotFound(name.into()))?;
        fs::remove_file(self.path_of(set.kind, name))?;
        if self.active()?.as_deref() == Some(name) {
            self.set_active(None)?;
        }
        Ok(())
    }

    pub fn rename(&self, from: &str, to: &str) -> Result<(), StoreError> {
        validate_name(to)?;
        let set = self.find(from).ok_or_else(|| StoreError::NotFound(from.into()))?;
        if self.find(to).is_some() {
            return Err(StoreError::AlreadyExists(to.into()));
        }
        fs::rename(self.path_of(set.kind, from), self.path_of(set.kind, to))?;
        if self.active()?.as_deref() == Some(from) {
            self.set_active(Some(to))?;
        }
        Ok(())
    }

    /// Name of the active set, if any.
    pub fn active(&self) -> Result<Option<String>, StoreError> {
        match fs::read_to_string(self.root.join(ACTIVE_FILE)) {
            Ok(s) => {
                let s = s.trim();
                Ok((!s.is_empty()).then(|| s.to_owned()))
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn set_active(&self, name: Option<&str>) -> Result<(), StoreError> {
        match name {
            Some(n) => fs::write(self.root.join(ACTIVE_FILE), n)?,
            None => match fs::remove_file(self.root.join(ACTIVE_FILE)) {
                Ok(()) => {}
                Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                Err(e) => return Err(e.into()),
            },
        }
        Ok(())
    }

    /// Returns the name after (`step = 1`) or before (`step = -1`) the active set,
    /// wrapping around. With no active set, returns the first set.
    pub fn neighbour(&self, step: i32) -> Result<Option<String>, StoreError> {
        let names: Vec<String> = self
            .list()?
            .into_iter()
            .filter(|s| s.kind != SetKind::Combined)
            .map(|s| s.name)
            .collect();
        if names.is_empty() {
            return Ok(None);
        }
        let len = names.len() as i32;
        let idx = match self.active()? {
            Some(active) => match names.iter().position(|n| *n == active) {
                Some(i) => (i as i32 + step).rem_euclid(len),
                None => 0,
            },
            None => 0,
        };
        Ok(Some(names[idx as usize].clone()))
    }
}

/// Rejects names that would escape the directory or be hard to use as a file name.
pub fn validate_name(name: &str) -> Result<(), StoreError> {
    let bad = name.is_empty()
        || name.starts_with('.')
        || name.trim() != name
        || name.chars().any(|c| matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || c.is_control());
    if bad {
        Err(StoreError::InvalidName(name.into()))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store(tag: &str) -> Store {
        let dir = std::env::temp_dir().join(format!("hostmask-test-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        Store::open(dir).unwrap()
    }

    #[test]
    fn create_read_save_delete() {
        let s = temp_store("crud");
        s.create("work", "1.1.1.1 a\n").unwrap();
        assert_eq!(s.read("work").unwrap(), "1.1.1.1 a\n");
        s.save("work", "2.2.2.2 b\n").unwrap();
        assert_eq!(s.read("work").unwrap(), "2.2.2.2 b\n");
        assert!(matches!(s.create("work", ""), Err(StoreError::AlreadyExists(_))));
        s.delete("work").unwrap();
        assert!(matches!(s.read("work"), Err(StoreError::NotFound(_))));
    }

    #[test]
    fn neighbour_wraps_around() {
        let s = temp_store("nav");
        for n in ["a", "b", "c"] {
            s.create(n, "").unwrap();
        }
        s.set_active(Some("c")).unwrap();
        assert_eq!(s.neighbour(1).unwrap().as_deref(), Some("a"));
        assert_eq!(s.neighbour(-1).unwrap().as_deref(), Some("b"));
    }

    #[test]
    fn rename_moves_active_marker() {
        let s = temp_store("rename");
        s.create("old", "x").unwrap();
        s.set_active(Some("old")).unwrap();
        s.rename("old", "new").unwrap();
        assert_eq!(s.active().unwrap().as_deref(), Some("new"));
    }

    #[test]
    fn rejects_bad_names() {
        assert!(validate_name("../etc").is_err());
        assert!(validate_name(".hidden").is_err());
        assert!(validate_name("a/b").is_err());
        assert!(validate_name("").is_err());
        assert!(validate_name("ok name").is_ok());
    }
}
