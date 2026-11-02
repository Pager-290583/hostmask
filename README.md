# Hostmask

<p align="center">
  <img src="assets/icons/hostmask-256.png" alt="Hostmask — hosts-file set manager for macOS, Linux and Windows" width="160" height="160">
</p>

A hosts-file set manager for macOS, Linux (Flatpak) and Windows. A Rust reimplementation of GasMask 0.8.6, see `docs/REVERSE_ENGINEERING.md`.

## Features

- **GUI** (`crates/gui`, egui): sets grouped into Local / Remote / Combined, monospace editor with unsaved-changes tracking, `/etc/hosts` view.
- Activate a set: writes it to `/etc/hosts` with administrator rights (macOS password dialog) and flushes the DNS cache.
- Remote sets: add by http(s) URL, refreshed every N minutes (setting). Rejects HTML/image responses.
- Combined sets from several sets, marked with `# Hosts File: <name>`.
- Warning banner when `/etc/hosts` no longer matches the active set, with a one-click re-apply.
- Shortcuts: Cmd+S save, Cmd+Option+→ / ← next / previous set.
- Status-bar (menu bar) icon: closing the window hides it and the app keeps running. The menu lists all sets (checkmark on the active one), Next/Previous, Show and Quit. The active set's name is shown next to the icon (on Linux: GNOME AppIndicator extension / Ubuntu; KDE shows the icon only). Turn this off in Settings to make closing the window quit the app.
- Remote sets refresh in a background thread, so they update while the window is hidden.
- CLI (`crates/cli`): `hostmask list | show | new | save | rm | rename | activate | next | prev | combine | system`.
- Data stays in the GasMask folder `~/Library/Gas Mask/{Local,Remote,Combined}` (override with `HOSTMASK_HOME`).

- First start (no sets yet): the machine's current hosts file is saved as the set "Исходный" and marked active.

## Not yet

- Global hotkeys, start at login.
- Automatic restore of `/etc/hosts` when another program changes it (the app warns instead, so you are not asked for a password on every change).
- Windows browser DNS/cache shortcut.

## Установка

Готовые сборки находятся в [GitHub Releases → latest](https://github.com/Pager-290583/hostmask/releases/tag/latest), в блоке **Assets**. Это автоматически обновляемый предварительный релиз. Скачивать исходники и устанавливать Rust для запуска приложения не нужно.

| Платформа | Файл |
| --- | --- |
| macOS, Apple Silicon (M1 и новее) | `Hostmask-macos-arm64.dmg` |
| macOS, Intel | `Hostmask-macos-x86_64.dmg` |
| Windows 10/11, x64 | `Hostmask-windows-x86_64.zip` |
| Linux, x86_64 | `hostmask.flatpak` |

### macOS

1. Откройте меню **Apple → Об этом Mac**. Если указан чип Apple M1/M2/M3/M4/M5 или новее — выбирайте `arm64`; если процессор Intel — `x86_64`.
2. Скачайте соответствующий `.dmg` из **Assets** релиза.
3. Откройте образ и перетащите **Hostmask.app** на значок **Applications / Программы** внутри образа.
4. Запустите **Hostmask** из папки **Программы** в Finder. Затем образ можно извлечь.
5. Если macOS заблокирует первый запуск, закройте уведомление и откройте **Системные настройки → Конфиденциальность и безопасность → Всё равно открыть** для Hostmask. Подтвердите запуск. Сборка имеет локальную ad-hoc подпись, без нотариализации Apple; [инструкция Apple](https://support.apple.com/ru-ru/102445).

**Где искать приложение:** `/Applications/Hostmask.app`. В Finder папка «Программы» открывается сочетанием **⌘⇧A**. Hostmask работает через значок маски в верхней строке меню и не показывает значок в Dock. Закрытие окна по умолчанию прячет его; вернуть окно можно через значок маски → **Открыть Hostmask**. Для полного завершения выберите **Выйти из Hostmask**.

**Переключение наборов:** при записи системного hosts macOS запросит права администратора. Touch ID используется, если он уже настроен в системе для sudo; иначе появится запрос пароля.

**Обновление:** завершите Hostmask через его меню, скачайте новый DMG и замените приложение в «Программах». Наборы и настройки хранятся отдельно, в `~/Library/Gas Mask/`, поэтому замена приложения их не удаляет.

### Windows

1. Для Windows 10/11 **x64** скачайте `Hostmask-windows-x86_64.zip` из **Assets** релиза.
2. Нажмите по ZIP правой кнопкой → **Извлечь всё**. Переместите распакованную папку в постоянное место, например в свою папку «Программы». Установщик не нужен.
3. Откройте распакованную папку и запустите **Hostmask.exe**. Не запускайте приложение непосредственно внутри ZIP.
4. Подтвердите запрос контроля учётных записей **UAC**. Приложение запрашивает права администратора при запуске, поскольку переключение наборов меняет системный файл `%SystemRoot%\System32\drivers\etc\hosts`.
5. Если SmartScreen показывает предупреждение о неизвестном издателе, учитывайте, что эта сборка не имеет сертификата подписи Windows. Для файла, скачанного из Releases этого репозитория, разрешение запуска доступно через **Подробнее → Выполнить в любом случае**, если политика компьютера это допускает.

**Где искать приложение:** в папке, куда вы распаковали ZIP. Можно создать ярлык для `Hostmask.exe` на рабочем столе. После закрытия окна Hostmask по умолчанию остаётся в системном трее рядом с часами; значок может находиться под стрелкой **˄**. Нажмите по маске правой кнопкой → **Открыть Hostmask** или **Выйти из Hostmask**.

**Данные:** наборы и настройки находятся в `%APPDATA%\hostmask` учётной записи, от которой запущено приложение. Консольная утилита `hostmask-cli.exe` лежит рядом с GUI; для активации набора из CLI нужен терминал с правами администратора. Пункт открытия Chrome для очистки кэша на Windows пока не реализован — при необходимости перезапустите браузер после смены hosts.

**Обновление:** завершите Hostmask через трей, распакуйте новый ZIP и замените файлы программы. Папку `%APPDATA%\hostmask` удалять не нужно.

### Linux — Flatpak

Пакет предназначен для **x86_64**. Flatpak устанавливается один раз; компилятор Rust и Flatpak SDK для готового пакета не нужны.

**1. Установите Flatpak**, если он ещё не установлен.

Fedora:

```sh
sudo dnf install flatpak
```

Ubuntu / Debian:

```sh
sudo apt update
sudo apt install flatpak
```

Для другого дистрибутива используйте [инструкции настройки Flathub](https://flathub.org/en/setup). После первой установки Flatpak выйдите из сеанса и войдите снова, чтобы приложения появились в меню рабочего стола.

**2. Подключите Flathub для текущего пользователя и установите runtime:**

```sh
flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
flatpak install --user flathub org.freedesktop.Platform//25.08
```

**3. Скачайте `hostmask.flatpak` из Assets релиза.** Откройте терминал в папке со скачанным файлом и выполните:

```sh
flatpak install --user ./hostmask.flatpak
```

Hostmask распространяется через GitHub Releases; в каталоге Flathub его искать не нужно. Flathub здесь используется для системного runtime.

**4. Запустите Hostmask** из меню приложений или командой:

```sh
flatpak run io.github.hostmask.Hostmask
```

**5. Настройте трей, если используете GNOME.** Для значка нужно расширение **AppIndicator and KStatusNotifierItem Support**. Пример для Fedora:

```sh
sudo dnf install gnome-shell-extension-appindicator
gnome-extensions enable appindicatorsupport@rgcjonas.gmail.com
```

После установки расширения выйдите из сеанса и войдите снова. В KDE Plasma дополнительное расширение для трея обычно не требуется. Hostmask использует X11/XWayland, чтобы закрытие окна могло прятать его в трей. Если значок трея недоступен, отключите в настройках Hostmask опцию «При закрытии окна оставлять Hostmask в строке меню» — тогда закрытие окна завершит приложение.

**Переключение наборов:** изменение `/etc/hosts` выполняется через `pkexec` и системный запрос прав администратора. На компьютере должны работать polkit и его агент авторизации. Hostmask сбрасывает системный DNS-кэш; браузер может потребовать отдельного сброса кэша или перезапуска.

**Данные:** `~/.var/app/io.github.hostmask.Hostmask/data/hostmask/`.

**Обновление:** завершите приложение через трей, скачайте свежий `hostmask.flatpak`, откройте терминал в папке с файлом и выполните:

```sh
flatpak install --user --reinstall ./hostmask.flatpak
```

Инструкции по сборке из исходников и диагностике: [Flatpak на Fedora](flatpak/BUILD-FEDORA.md). Справка по командам: [документация Flatpak](https://docs.flatpak.org/en/latest/using-flatpak.html).

### Первый запуск и проверка загрузки

На новой установке Hostmask сохраняет текущий системный hosts в набор **«Исходный»**. Сам первый запуск не меняет системный файл. Чтобы применить другой набор, выберите его и нажмите **Сделать активным** либо выберите набор в меню трея.

Для проверки скачанного файла возьмите `SHA256SUMS` из того же релиза и сравните указанную в нём сумму с результатом команды для своего файла:

macOS:

```sh
shasum -a 256 Hostmask-macos-arm64.dmg
```

Windows PowerShell:

```powershell
Get-FileHash .\Hostmask-windows-x86_64.zip -Algorithm SHA256
```

Linux:

```sh
sha256sum hostmask.flatpak
```

Для Intel Mac подставьте имя `Hostmask-macos-x86_64.dmg`. Переменная `HOSTMASK_HOME` позволяет использовать другую папку данных на любой платформе.

## Build

Use current stable Rust (eframe 0.37 requires Rust 1.95 or newer) and Xcode Command Line Tools on macOS.


    cargo test --workspace
    scripts/bundle-macos.sh        # -> dist/Hostmask.app (ad-hoc signed, not notarized)

    scripts/package-macos.sh       # -> dist/Hostmask-macos-arm64.dmg or x86_64.dmg

Keep Cargo build output outside synced folders, for example:

```sh
export CARGO_TARGET_DIR="$HOME/Library/Caches/hostmask/target"
scripts/package-macos.sh
```

The bundle script accepts an optional Rust target (`aarch64-apple-darwin` or `x86_64-apple-darwin`); install that target with rustup before cross-compiling.
Artwork lives in `assets/icons/`. The bundle script generates Retina/non-Retina ICNS sizes automatically. Linux uses the same artwork in the launcher, window and tray; macOS uses a monochrome mask in the menu bar.

On Windows, use current stable Rust with the MSVC toolchain and Visual Studio C++ Build Tools / Windows SDK:

```powershell
cargo test --locked --workspace
./scripts/package-windows.ps1
```

This creates `dist/Hostmask-windows-x86_64.zip` with `Hostmask.exe`, `hostmask-cli.exe` and a short readme. Windows resources and the UAC manifest are embedded during the native Windows build.

## Automated releases

[Build and release](.github/workflows/release.yml) builds Linux Flatpak, native macOS images for Apple Silicon and Intel, and a portable Windows x64 ZIP. Pull requests build without publishing. Pushes to `main` update the rolling `latest` pre-release; `v*` tags publish permanent releases. A manual run on `main` rebuilds and republishes `latest`.

Publication waits for every platform build and its workspace tests. The Windows job also checks CLI startup and creation of the GUI window, using a temporary data directory and verifying that the system hosts file stays unchanged. One release job uploads all platforms together, preserving the existing rolling release and its download URLs. No Apple signing certificate or notarization credentials are configured.


## Caution

Hostmask and GasMask both write `/etc/hosts`. Use one of them at a time: the last one to activate wins, and the other will then show a mismatch.
