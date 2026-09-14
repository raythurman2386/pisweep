#!/usr/bin/env bash
# Install Pisweep into ~/.local (binary, desktop entry, icon). No root.
set -euo pipefail

ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
PREFIX="${PREFIX:-$HOME/.local}"

cd "$ROOT"
cargo build --release --locked

install -Dm755 "$ROOT/target/release/pisweep" "$PREFIX/bin/pisweep"
install -Dm644 "$ROOT/dist/pisweep.desktop" "$PREFIX/share/applications/pisweep.desktop"
install -Dm644 "$ROOT/dist/pisweep.svg" "$PREFIX/share/icons/hicolor/scalable/apps/pisweep.svg"
install -Dm644 "$ROOT/LICENSE" "$PREFIX/share/licenses/pisweep/LICENSE"
install -Dm644 "$ROOT/fonts/OFL.txt" "$PREFIX/share/licenses/pisweep/OFL.txt"

if command -v rsvg-convert >/dev/null 2>&1; then
  tmp="$(mktemp --suffix=.png)"
  rsvg-convert -w 128 -h 128 "$ROOT/dist/pisweep.svg" -o "$tmp"
  install -Dm644 "$tmp" "$PREFIX/share/icons/hicolor/128x128/apps/pisweep.png"
  rm -f "$tmp"
elif command -v magick >/dev/null 2>&1; then
  tmp="$(mktemp --suffix=.png)"
  magick -background none "$ROOT/dist/pisweep.svg" -resize 128x128 "$tmp"
  install -Dm644 "$tmp" "$PREFIX/share/icons/hicolor/128x128/apps/pisweep.png"
  rm -f "$tmp"
fi

if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database "$PREFIX/share/applications" >/dev/null 2>&1 || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
  gtk-update-icon-cache -f -t "$PREFIX/share/icons/hicolor" >/dev/null 2>&1 || true
fi

echo "Installed pisweep to $PREFIX/bin/pisweep"
echo "Launcher: $PREFIX/share/applications/pisweep.desktop"
if [[ ":$PATH:" != *":$PREFIX/bin:"* ]]; then
  echo "Add $PREFIX/bin to PATH if the launcher cannot find pisweep."
fi