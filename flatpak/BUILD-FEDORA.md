# Сборка и проверка Flatpak на Fedora

Все команды выполняются на Fedora, из папки репозитория `hostmask/flatpak`.

## 1. Установка инструментов

```sh
sudo dnf install -y flatpak flatpak-builder
flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
flatpak install --user -y flathub \
  org.freedesktop.Platform//25.08 \
  org.freedesktop.Sdk//25.08 \
  org.freedesktop.Sdk.Extension.rust-stable//25.08
```

## 2. Сборка и установка

```sh
cd hostmask/flatpak
flatpak run org.flatpak.Builder --user --install --force-clean --state-dir ~/.cache/hostmask-flatpak-state ~/.cache/hostmask-flatpak-build io.github.hostmask.Hostmask.yml
```

Первая сборка долгая: скачиваются crates и компилируется Rust. Сборка идёт с `--share=network`,
поэтому нужен доступ к интернету.

Если падает, сохраните вывод целиком:

```sh
flatpak run org.flatpak.Builder --user --install --force-clean --state-dir ~/.cache/hostmask-flatpak-state ~/.cache/hostmask-flatpak-build io.github.hostmask.Hostmask.yml 2>&1 | tee ~/hostmask-build.log
```

## Пакет для передачи другим (один файл)

После сборки из раздела 2 (с `--state-dir ~/.cache/hostmask-flatpak-state`):

```sh
cd hostmask
flatpak build-bundle --runtime-repo=https://dl.flathub.org/repo/flathub.flatpakrepo \
  ~/.cache/hostmask-flatpak-state/cache dist/linux/hostmask.flatpak io.github.hostmask.Hostmask master
```

Получатель ставит его по инструкции `dist/linux/README.md`. Наборы hosts в пакет не попадают.

## 3. Запуск

```sh
flatpak run io.github.hostmask.Hostmask 2>&1 | tee run.log
```

Запускать из терминала нужно, чтобы видеть ошибки. Окно должно открыться.

## 4. Что проверить

1. **Окно и список наборов.** Наборы из `~/.var/app/io.github.hostmask.Hostmask/data/hostmask/`
   (первый запуск создаёт пустую папку, наборы можно добавить через окно).
2. **Смена набора.** Выберите набор и нажмите «Сделать активным». Должен появиться запрос
   пароля (polkit). После этого проверьте в обычном терминале:
   ```sh
   cat /etc/hosts
   ```
   В файле должен быть содержимое выбранного набора.
3. **Трей.** Иконка глобуса в панели (см. раздел про GNOME ниже). Меню: переключение наборов,
   «Сбросить кэш браузера…», «Выйти из Hostmask».
4. **Закрытие окна.** Закрытие окна прячет его, приложение остаётся в трее. Пункт «Открыть
   Hostmask» должен вернуть окно.

## 5. GNOME: иконка в трее

GNOME не показывает иконки StatusNotifierItem без расширения. Установите и включите:

```sh
sudo dnf install -y gnome-shell-extension-appindicator
gnome-extensions enable appindicatorsupport@rgcjonas.gmail.com
```

Затем выйдите из сессии и войдите снова (на Wayland перезапуска GNOME Shell нет).
На KDE Plasma это не нужно.

## 6. Частые проблемы

- **`flatpak-builder: command not found`**: не установлен пакет `flatpak-builder`.
- **Не скачивается runtime**: проверьте, что добавлен remote `flathub` с опцией `--user`.
- **Окно не открывается, в `run.log` есть ошибки про Vulkan или GL**: пришлите `run.log`.
- **Нет запроса пароля при смене набора**: пришлите вывод `flatpak run` из `run.log` и
  вывод `pkexec --version` на хосте.
- **Трея нет, а окно работает**: это обычно про GNOME-расширение из раздела 5.

## 7. Что прислать, если что-то не работает

- `build.log` (если сборка упала) или `run.log` (если запуск упал или что-то не работает);
- какой пункт из раздела 4 не сработал;
- версия Fedora: `cat /etc/fedora-release`;
- рабочий стол: GNOME, KDE или другой (`echo $XDG_CURRENT_DESKTOP`).
