#!/usr/bin/env bash
# Remove a Pisweep user install from ~/.local.
set -euo pipefail

PREFIX="${PREFIX:-$HOME/.local}"

rm -f "$PREFIX/bin/pisweep"
rm -f "$PREFIX/share/applications/pisweep.desktop"
rm -f "$PREFIX/share/icons/hicolor/scalable/apps/pisweep.svg"
rm -f "$PREFIX/share/icons/hicolor/128x128/apps/pisweep.png"
rm -rf "$PREFIX/share/licenses/pisweep"

if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database "$PREFIX/share/applications" >/dev/null 2>&1 || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
  gtk-update-icon-cache -f -t "$PREFIX/share/icons/hicolor" >/dev/null 2>&1 || true
fi

echo "Removed pisweep from $PREFIX"