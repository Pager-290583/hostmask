//! Platform-specific pieces: where the system hosts file lives, how to write it
//! with administrator rights, how to flush the DNS cache, and where data is stored.
//!
//! GasMask on macOS (verified by reverse engineering) does the same thing:
//! it saves the set to `/etc/hosts` through an admin-authorized copy and then runs
//! `dscacheutil -flushcache`. This module reproduces that behaviour on each OS.

use std::fs;
use std::io;
use std::path::PathBuf;
#[cfg(any(target_os = "macos", windows))]
use std::path::Path;
use std::process::Command;

#[derive(Debug)]
pub enum PlatformError {
    Io(io::Error),
    /// The privileged helper ran but returned a non-zero status.
    Command { cmd: String, stderr: String },
    /// The user cancelled the password / UAC prompt.
    Cancelled,
}

impl std::fmt::Display for PlatformError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlatformError::Io(e) => write!(f, "I/O error: {e}"),
            PlatformError::Command { cmd, stderr } => write!(f, "command failed ({cmd}): {stderr}"),
            PlatformError::Cancelled => write!(f, "authorization was cancelled"),
        }
    }
}

impl std::error::Error for PlatformError {}

impl From<io::Error> for PlatformError {
    fn from(e: io::Error) -> Self {
        PlatformError::Io(e)
    }
}

/// Path of the system hosts file on this OS.
pub fn system_hosts_path() -> PathBuf {
    #[cfg(windows)]
    {
        let root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into());
        PathBuf::from(root).join(r"System32\drivers\etc\hosts")
    }
    #[cfg(not(windows))]
    {
        PathBuf::from("/etc/hosts")
    }
}

/// Default data directory. On macOS this is GasMask's own directory, so existing
/// sets are picked up without copying.
pub fn default_data_dir() -> PathBuf {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    #[cfg(target_os = "macos")]
    {
        home.unwrap_or_default().join("Library").join("Gas Mask")
    }
    #[cfg(windows)]
    {
        std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_default()
            .join("hostmask")
    }
    #[cfg(all(not(target_os = "macos"), not(windows)))]
    {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.unwrap_or_default().join(".local/share"))
            .join("hostmask")
    }
}

/// Opens Chrome's network diagnostics page, where the host cache and socket pools can be
/// cleared. Browsers keep their own DNS cache, so changing the system hosts file alone is
/// not always enough. Windows is not supported yet and returns an error.
#[cfg(target_os = "macos")]
pub fn open_chrome_net_internals() -> io::Result<()> {
    let status = Command::new("/usr/bin/open")
        .args(["-a", "Google Chrome", "chrome://net-internals/#dns"])
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::new(io::ErrorKind::NotFound, "Google Chrome не найден"))
    }
}

/// Linux: `xdg-open` would hand a `chrome://` URL to the default browser, which may not be
/// Chrome, so the browser is started directly. Tries native Chrome/Chromium first, then their
/// Flatpak builds. Inside Flatpak the search runs on the host through `flatpak-spawn`.
#[cfg(all(unix, not(target_os = "macos")))]
pub fn open_chrome_net_internals() -> io::Result<()> {
    // Exit 0 once a browser has been started in the background, 127 if none was found.
    const FIND_AND_OPEN: &str = r#"
for b in google-chrome-stable google-chrome chromium-browser chromium; do
  if command -v "$b" >/dev/null 2>&1; then setsid "$b" "$1" >/dev/null 2>&1 & exit 0; fi
done
for a in com.google.Chrome org.chromium.Chromium; do
  if flatpak info "$a" >/dev/null 2>&1; then setsid flatpak run "$a" "$1" >/dev/null 2>&1 & exit 0; fi
done
exit 127
"#;
    let mut cmd = if std::env::var_os("FLATPAK_ID").is_some() {
        let mut c = Command::new("flatpak-spawn");
        c.args(["--host", "sh", "-c"]);
        c
    } else {
        let mut c = Command::new("sh");
        c.arg("-c");
        c
    };
    let status = cmd
        .args([FIND_AND_OPEN, "sh", "chrome://net-internals/#dns"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()?;
    match status.code() {
        Some(0) => Ok(()),
        Some(127) => Err(io::Error::new(io::ErrorKind::NotFound, "Google Chrome или Chromium не найден")),
        _ => Err(io::Error::other(format!("запуск браузера завершился с ошибкой ({status})"))),
    }
}

#[cfg(windows)]
pub fn open_chrome_net_internals() -> io::Result<()> {
    Err(io::Error::new(io::ErrorKind::Unsupported, "пока не поддерживается в Windows"))
}

pub fn read_system_hosts() -> io::Result<String> {
    fs::read_to_string(system_hosts_path())
}

/// Replaces the system hosts file with `contents` and flushes DNS caches.
///
/// `contents` is first written to a temp file (unprivileged), then a single
/// privileged command copies it into place and flushes the cache.
pub fn install_system_hosts(contents: &str) -> Result<(), PlatformError> {
    // Linux passes the contents through stdin. A staging file would not work inside a
    // Flatpak sandbox: its /tmp is private, so the privileged host command cannot see it.
    #[cfg(all(unix, not(target_os = "macos")))]
    return install_linux(contents);

    #[cfg(not(all(unix, not(target_os = "macos"))))]
    {
        let tmp = staging_file()?;
        fs::write(&tmp, contents)?;
        let result = install_from(&tmp);
        let _ = fs::remove_file(&tmp);
        result
    }
}

#[cfg(not(all(unix, not(target_os = "macos"))))]
fn staging_file() -> io::Result<PathBuf> {
    let name = format!("hostmask-{}-{}.tmp", std::process::id(), nanos());
    Ok(std::env::temp_dir().join(name))
}

#[cfg(not(all(unix, not(target_os = "macos"))))]
fn nanos() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0)
}

#[cfg(target_os = "macos")]
fn install_from(src: &Path) -> Result<(), PlatformError> {
    // Preferred: sudo, which can use Touch ID when pam_tid is enabled in /etc/pam.d/sudo_local.
    // If sudo cannot authenticate here, fall back to the standard password dialog.
    match install_with_sudo(src) {
        Some(result) => result,
        None => install_with_osascript(src),
    }
}

/// The shell command that installs the hosts file and flushes the DNS cache.
#[cfg(target_os = "macos")]
fn macos_install_command(src: &Path) -> String {
    format!(
        "/bin/cp -f {src} /etc/hosts && /bin/chmod 644 /etc/hosts && /usr/bin/dscacheutil -flushcache; /usr/bin/killall -HUP mDNSResponder 2>/dev/null; true",
        src = shell_quote(&src.to_string_lossy()),
    )
}

/// Runs the install through sudo. Returns `None` when sudo could not ask for credentials
/// here (no terminal and no Touch ID), so the caller can use the password dialog instead.
#[cfg(target_os = "macos")]
fn install_with_sudo(src: &Path) -> Option<Result<(), PlatformError>> {
    let out = Command::new("/usr/bin/sudo")
        .args(["-p", "", "--", "/bin/sh", "-c", &macos_install_command(src)])
        .stdin(std::process::Stdio::null())
        .output()
        .ok()?;
    if out.status.success() {
        return Some(Ok(()));
    }
    let stderr = String::from_utf8_lossy(&out.stderr).trim().to_owned();
    if stderr.contains("a terminal is required") || stderr.contains("password is required") {
        return None;
    }
    Some(Err(PlatformError::Command { cmd: "sudo".into(), stderr }))
}

#[cfg(target_os = "macos")]
fn install_with_osascript(src: &Path) -> Result<(), PlatformError> {
    // Same mechanism as GasMask: run a shell command as administrator via osascript,
    // which shows the standard system password dialog.
    let shell = macos_install_command(src);
    let script = format!(
        "do shell script \"{}\" with administrator privileges",
        applescript_escape(&shell)
    );
    let out = Command::new("/usr/bin/osascript").arg("-e").arg(&script).output()?;
    if out.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&out.stderr).trim().to_owned();
    // osascript reports user cancellation as error -128.
    if stderr.contains("-128") {
        Err(PlatformError::Cancelled)
    } else {
        Err(PlatformError::Command { cmd: "osascript".into(), stderr })
    }
}

/// Writes `contents` to /etc/hosts through pkexec (polkit asks for the password) and
/// flushes the resolver caches. Inside Flatpak the command runs on the host, which needs
/// `--talk-name=org.freedesktop.Flatpak`.
#[cfg(all(unix, not(target_os = "macos")))]
fn install_linux(contents: &str) -> Result<(), PlatformError> {
    let flush = "(resolvectl flush-caches || systemd-resolve --flush-caches || nscd -i hosts) >/dev/null 2>&1; true";
    // `cat >` truncates the existing file, so its owner and mode stay as they are.
    let script = format!("cat > /etc/hosts && chmod 644 /etc/hosts && {flush}");
    let mut cmd = if std::env::var_os("FLATPAK_ID").is_some() {
        let mut c = Command::new("flatpak-spawn");
        c.args(["--host", "pkexec", "sh", "-c"]);
        c
    } else {
        let mut c = Command::new("pkexec");
        c.args(["sh", "-c"]);
        c
    };
    let mut child = cmd
        .arg(&script)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()?;
    if let Some(mut stdin) = child.stdin.take() {
        // Dropping stdin at the end of the block sends EOF. A write error shows up in the
        // exit status below, so it is ignored here.
        let _ = std::io::Write::write_all(&mut stdin, contents.as_bytes());
    }
    let out = child.wait_with_output()?;
    if out.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&out.stderr).trim().to_owned();
    // pkexec exits with 126 when the user dismisses the authentication dialog.
    if out.status.code() == Some(126) {
        Err(PlatformError::Cancelled)
    } else {
        Err(PlatformError::Command { cmd: "pkexec".into(), stderr })
    }
}

#[cfg(windows)]
fn install_from(src: &Path) -> Result<(), PlatformError> {
    // The process must already be elevated (manifest requireAdministrator or UAC relaunch).
    // Not yet tested on Windows.
    fs::copy(src, system_hosts_path())?;
    let _ = Command::new("ipconfig").arg("/flushdns").output();
    Ok(())
}

/// Quotes a string for POSIX sh using single quotes.
#[cfg(any(target_os = "macos", test))]
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// Escapes a string to be embedded in a double-quoted AppleScript string literal.
#[cfg(any(target_os = "macos", test))]
fn applescript_escape(s: &str) -> String {
    s.replace('\\', r"\\").replace('"', r#"\""#)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_single_quotes_for_sh() {
        assert_eq!(shell_quote("a'b"), r"'a'\''b'");
    }

    #[test]
    fn escapes_for_applescript() {
        assert_eq!(applescript_escape(r#"say "hi" \ x"#), r#"say \"hi\" \\ x"#);
    }
}
