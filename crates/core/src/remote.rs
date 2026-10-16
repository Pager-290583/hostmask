//! Remote hosts sets: a URL registry and the downloader.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

const REGISTRY_FILE: &str = "remote.json";
const USER_AGENT: &str = concat!("hostmask/", env!("CARGO_PKG_VERSION"));
/// Hosts lists are small text files; anything bigger is almost certainly wrong.
const MAX_SIZE: u64 = 20 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RemoteEntry {
    pub url: String,
    /// Unix time of the last successful download.
    #[serde(default)]
    pub last_updated: Option<u64>,
}

/// Maps remote set names to their source URLs. Persisted as `remote.json`.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct RemoteRegistry {
    pub entries: BTreeMap<String, RemoteEntry>,
}

impl RemoteRegistry {
    pub fn load(root: &Path) -> Self {
        fs::read_to_string(root.join(REGISTRY_FILE))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, root: &Path) -> io::Result<()> {
        let json = serde_json::to_string_pretty(self).map_err(io::Error::other)?;
        fs::write(root.join(REGISTRY_FILE), json)
    }
}

#[derive(Debug)]
pub enum FetchError {
    /// The URL is not http(s).
    BadUrl(String),
    Http(String),
    /// The server returned something that is not a hosts file (HTML page, image, binary).
    NotHosts(String),
    TooLarge,
}

impl std::fmt::Display for FetchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FetchError::BadUrl(u) => write!(f, "unsupported URL (use http or https): {u}"),
            FetchError::Http(e) => write!(f, "download failed: {e}"),
            FetchError::NotHosts(kind) => write!(f, "the server returned {kind}, not a hosts file"),
            FetchError::TooLarge => write!(f, "the hosts file is larger than 20 MB"),
        }
    }
}

impl std::error::Error for FetchError {}

/// Downloads a hosts list and returns its text.
pub fn fetch(url: &str) -> Result<String, FetchError> {
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err(FetchError::BadUrl(url.into()));
    }
    let mut resp = ureq::get(url)
        .header("User-Agent", USER_AGENT)
        .header("Accept", "text/plain, */*")
        .call()
        .map_err(|e| FetchError::Http(e.to_string()))?;

    let content_type = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_ascii_lowercase();
    if content_type.starts_with("text/html") {
        return Err(FetchError::NotHosts("an HTML page".into()));
    }
    if content_type.starts_with("image/") {
        return Err(FetchError::NotHosts("an image".into()));
    }

    let text = resp
        .body_mut()
        .with_config()
        .limit(MAX_SIZE)
        .read_to_string()
        .map_err(|e| match e {
            ureq::Error::BodyExceedsLimit(_) => FetchError::TooLarge,
            other => FetchError::Http(other.to_string()),
        })?;
    Ok(text)
}

pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
