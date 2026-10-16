# GasMask 0.8.6 — reverse engineering notes

Target: `/Applications/Gas Mask.app` (bundle id `ee.clockwise.gmask`, macOS, x86_64, Objective-C/Cocoa).
Method: static inspection of the bundle (Info.plist, resources, `strings`, `nm`, `otool`) and the
app's own log file. No code was copied; the Rust implementation is written from these observations.

## What the app is

A hosts-file manager. Its job:

1. Keep several named hosts files ("sets").
2. Activate one of them by copying it over `/etc/hosts`.
3. Download remote hosts lists and refresh them periodically.
4. Build a "combined" file out of several sets.
5. Watch `/etc/hosts` and restore the active set if another program changes it.

## Bundle structure

| Path | Purpose |
|---|---|
| `Contents/MacOS/Gas Mask` | main Cocoa executable (`LSUIElement`, runs from the menu bar) |
| `Contents/Resources/Launcher.app` | tiny helper for "start at login" (`/usr/bin/open <app> --args openatlogin`) |
| `Contents/Resources/default.hst` | template for a new set (`127.0.0.1 localhost` etc.) |
| `Contents/Resources/UserDefaults.plist` | default preferences |
| `Contents/Frameworks/Sparkle.framework` | auto-update (`SUFeedURL` = `http://gmask.clockwise.ee/check_update/`, DSA-signed) |
| `Contents/Frameworks/ShortcutRecorder`, `BGHUDAppKit`, `Growl` | UI widgets / notifications |
| `Info.plist` → `CFBundleDocumentTypes`, `UTExportedTypeDeclarations` | `.hst` extension is registered as a document type `ee.clockwise.gasmask.hst` |

## Data layout (on disk, per user)

```
~/Library/Gas Mask/
    Local/<name>.hst     user-written sets          (e.g. MT2.hst, PR.hst, MT4.hst)
    Remote/<name>.hst    downloaded sets (cache)
    Combined/<name>.hst  generated combinations
```

Active set and settings live in `~/Library/Preferences/ee.clockwise.gmask.plist`:
`activeHostsFile` (full path of the active set), `remoteHostsUpdateInterval` (minutes, default 60),
`overrideExternalModifications`, `syntaxHighlighting`, `showNameInStatusBar`, `startAtLogin`,
two global hotkeys (`activateNextHotkey`, `activatePreviousHotkey`, `updateAndSynchronizeHotkey`).

Files are plain UTF-8 text (`com.apple.TextEncoding: utf-8`). No header, no metadata: the set file is
the exact content that gets written to `/etc/hosts`.

System-wide strings: `/etc/hosts`, `/etc/gasmask/` (only referenced by the 0.4 → 0.5 migration code,
`StructureConverter` — the directory does not exist on a current install).

## Activation flow (`-[AbstractHostsController activateHostsFile:]`, `Util flushDirectoryServiceCache`)

From the log (`~/Library/Logs/Gas Mask.log`):

```
[DEBUG] HostsMainController  - Activating: "MT2"
[DEBUG] AbstractHostsController - Saving hosts file "MT2" to "/etc/hosts"
[DEBUG] Util - Flushing Directory Service Cache
```

Strings and imports show the mechanism:

* `_AuthorizationCreate`, `_AuthorizationCopyRights`, `_AuthorizationExecuteWithPrivileges`
  with right `system.privilege.admin` — the privileged helper (`+[PrivilegedActions authorizeWithPrompt:]`,
  `makeWritableForCurrentUser:prompt:`).
* `/bin/cp`, `/bin/chmod`, `/bin/rm` — the privileged work is done with these commands.
* `/usr/bin/dscacheutil` with the `-flushcache` argument — DNS cache flush (`+[Util flushDirectoryServiceCache]`).
* `ensureHostsFileIsWritable:withReason:` — if `/etc/hosts` is not writable for the current user,
  the app asks for admin rights and makes it writable (`Failed to make "%@" writable`).

## Watching `/etc/hosts`

`VDKQueue` (kqueue-based file watcher) on `/etc/hosts`. On change, with `overrideExternalModifications` on:
`External application has changed the hosts file, restoring file`.

## Remote sets

* `RemoteHostsManager`: timer, interval from `remoteHostsUpdateInterval`; also refreshes on network-up
  (`Network came up` → `Starting timer ... Interval: 60 minutes`).
* `HostsDownloader`: raw HTTP/1.0 `GET <path>` over a socket with headers `Host`, `User-Agent: Gas Mask/<version>`,
  `Accept: text/html,text/plain`, optional `Last-Modified`.
* Rejects responses with `Content-Type` of HTML or images ("Can't download the hosts file. It contains HTML page.").

## Combined sets

`CombinedHosts initWithPath:allHostsFiles:` — concatenates the member sets. Each block starts with
`# Hosts File: <name>`. Changes in a member (`hostsFileSaved:`, `hostsFileRenamed:`, `hostsFileRemoved:`) are
propagated into the combined file.

## Other behaviour

* Switching next/previous set by hotkey (`activateNext`, `activatePrevious`), wrapping around.
* Creating a set from a dragged file or a URL, and from the Services menu (`createNewHostsFile`).
* "Move to trash" when deleting a set (`+[FileUtil moveToTrash:]`).
* Crash reporting (`CrashReportSender`) and Sparkle updates — intentionally not reproduced.

## Known quirks of the original (observed)

* Log spam every few hours: `TranformAppToState: ... NSOSStatusErrorDomain Code=-50` — a transform-process-type
  call that fails on this macOS version. Harmless for hosts management.
* Settings `activeHostsFile` is an absolute path, so moving the data folder breaks it. The Rust version
  stores only the set name.

## What the Rust clone reproduces

| GasMask feature | Rust clone |
|---|---|
| `Local/`, `Remote/`, `Combined/` layout, `.hst` files | yes, same layout; reads existing data directly |
| Activate set → `/etc/hosts` with admin rights | yes (osascript on macOS, pkexec on Linux, elevated copy on Windows) |
| DNS cache flush | yes (`dscacheutil` / `resolvectl` / `ipconfig /flushdns`) |
| Combined sets with `# Hosts File:` markers | yes |
| Next / previous set | yes (CLI; hotkeys not yet) |
| Remote set download and periodic refresh | not yet |
| Watching `/etc/hosts` and restoring | not yet |
| Menu-bar / GUI | not yet (CLI only) |
| Auto-update, crash reporter | not planned |
