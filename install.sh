#!/bin/sh
set -eu

release="${LAMEMAIL_RELEASE:-https://github.com/robcpc464/lamemail/releases/latest/download}"
source="https://github.com/robcpc464/lamemail#install"
bin="${XDG_BIN_HOME:-$HOME/.local/bin}"
data="${XDG_DATA_HOME:-$HOME/.local/share}"
desktop="$data/applications/lamemail-ui.desktop"
icon="$data/icons/hicolor/scalable/apps/lamemail.svg"
licenses="$data/licenses/lamemail"

die() {
  printf 'error: %s\n' "$*" >&2
  exit 1
}

uninstall() {
  rm -f "$bin/lamemail" "$bin/lamemail-ui" "$desktop" "$icon"
  rm -rf "$licenses"
  echo "removed lamemail and lamemail-ui from $bin, the menu entry, the icon and the licences from $data"
  echo "kept: the sync folder (~/.lamemail), the keyring passwords and ~/.config/lamemail"
}

install_release() {
  [ "$(uname -s)" = Linux ] || die "lamemail runs on Linux only"
  arch=$(uname -m)
  [ "$arch" = x86_64 ] || die "no binaries for $arch yet, build from source: $source"
  for tool in curl tar sha256sum; do
    command -v "$tool" >/dev/null || die "$tool is needed"
  done

  name="lamemail-$arch-linux"
  tmp=$(mktemp -d)
  trap 'rm -rf "$tmp"' EXIT
  curl -fsSL -o "$tmp/$name.tar.gz" "$release/$name.tar.gz"
  curl -fsSL -o "$tmp/$name.tar.gz.sha256" "$release/$name.tar.gz.sha256"
  (cd "$tmp" && sha256sum -c "$name.tar.gz.sha256" >/dev/null 2>&1) || die "the download does not match its checksum"
  tar -xzf "$tmp/$name.tar.gz" -C "$tmp"

  rm -rf "$licenses"
  mkdir -p "$bin" "$data"
  cp -R "$tmp/$name/bin/." "$bin/"
  cp -R "$tmp/$name/share/." "$data/"
  echo "installed $("$bin/lamemail" --version) into $bin, the menu entry, the icon and the licences into $data"

  missing=$(ldd "$bin/lamemail-ui" 2>&1 | grep 'not found' || true)
  if [ -n "$missing" ]; then
    echo "lamemail-ui needs a newer Qt 6 than this system has; the command line works without it:"
    echo "$missing"
  fi
  command -v secret-tool >/dev/null || echo "passwords need secret-tool (package libsecret) and a Secret Service keyring"
  case ":$PATH:" in
    *":$bin:"*) ;;
    *) echo "$bin is not in your PATH" ;;
  esac
  echo "next: lamemail init, then lamemail-ui"
}

main() {
  case "${1:-}" in
    "") install_release ;;
    --uninstall) uninstall ;;
    *) die "usage: install.sh [--uninstall]" ;;
  esac
}

main "$@"
