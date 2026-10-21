# lamemail-ui

The UI of lamemail: Qt Quick (QML) for the surface, C++ with Qt 6 for the
rest, nothing else. It holds no mail logic: everything that concerns mail
runs through the `lamemail` binary of [`../lamemail-cli`](../lamemail-cli),
the one next to `lamemail-ui`, else the one in `PATH`. The
[main README](../README.md) shows the UI and its keys.

## Building

Qt 6.9 or newer with QtQuick, Controls, Shapes, Labs Platform, Network,
D-Bus, the Wayland plugin and the SVG plugins, qmake6, make and a C++20
compiler (Arch: `qt6-base`, `qt6-declarative`, `qt6-wayland`, `qt6-svg`,
`base-devel`). Omarchy has all of it.

```sh
mkdir -p build && cd build
qmake6 -r ../lamemail-ui.pro && make -j"$(nproc)"
./app/lamemail-ui --dir <sync folder>
```

## Usage

```sh
lamemail-ui [--dir <path>] [--hidden] [--open <path>]
```

`--dir` is the sync folder, as for `lamemail`. `--hidden` starts in the
tray without a window while "Close to tray" is on; the start at login uses
it. `--open` shows a mail, its path as `lamemail list --json` prints it,
and the window even with `--hidden`. A second start on the same sync
folder brings up the running window, with the mail of `--open`.

## Files

- `~/.config/lamemail/lamemail-ui.conf`: what only concerns the UI, such
  as the theme, the tray, notifications, the download folder and the
  layout of the detail view; everything about mail goes through `lamemail`
- `~/.config/autostart/lamemail-ui.desktop` while "Autostart" is on
- `$XDG_RUNTIME_DIR/lamemail/attachments/`: attachments opened from a
  mail, read-only and gone after logging out

## On the desktop

- Colours follow the Omarchy theme live, unless Light or Dark is chosen;
  without Omarchy the dark palette is used. The font is the system's
  monospace one.
- The tray needs a StatusNotifierItem host (the Omarchy shell, KDE,
  Waybar); without one, closing the window quits.
- Notifications go to the desktop's notification service over D-Bus.
- Attachments open through the XDG desktop portal, without one through
  `xdg-open`.
- Shadows need the Qt module `QtQuick.Effects` (`qt6-declarative` on
  Arch, `qml6-module-qtquick-effects` on Debian and Ubuntu); without it
  everything else works.
- Icons are [Lucide](https://lucide.dev) paths drawn by Qt Quick Shapes
  (ISC licence in `LICENSE-lucide`), so no icon font is needed.
- Texts are English, or German when the locale starts with `de`.
