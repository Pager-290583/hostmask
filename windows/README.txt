Hostmask for Windows 10/11 x64

Extract the ZIP to a folder and run Hostmask.exe. No installer is needed.
Windows asks for administrator permission (UAC) at launch because switching
sets writes %SystemRoot%\System32\drivers\etc\hosts and flushes the DNS cache.
The application is not code-signed with a Windows publisher certificate.

Closing the window keeps Hostmask in the system tray by default. Right-click
the tray icon to open the window, switch a set, or quit.
Sets and settings are stored in %APPDATA%\hostmask. HOSTMASK_HOME overrides it.
CLI: run hostmask-cli.exe from a terminal; activation needs an elevated terminal.

The browser-cache shortcut is not implemented on Windows. Clear the browser's
DNS/socket cache manually or restart the browser after switching hosts.

https://github.com/Pager-290583/hostmask
