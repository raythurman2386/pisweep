#!/usr/bin/env bash
# Build a Linux x86_64 release tarball: binary, desktop entry, icon, install script.
set -euo pipefail

ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
VERSION="${VERSION:-$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT/Cargo.toml" | head -1)}"
ARCH="${ARCH:-x86_64}"
TARGET="${TARGET:-${ARCH}-unknown-linux-gnu}"
BIN="${PREBUILT_BIN:-$ROOT/target/release/pisweep}"
OUT_DIR="${OUT_DIR:-$ROOT/target/package}"
NAME="pisweep-${VERSION}-${TARGET}"
STAGE="$OUT_DIR/$NAME"

if [[ ! -x "$BIN" ]]; then
  echo "package-linux: missing binary at $BIN (build --release first)" >&2
  exit 1
fi

rm -rf "$STAGE"
mkdir -p "$STAGE"

install -m 755 "$BIN" "$STAGE/pisweep"
install -m 644 "$ROOT/dist/pisweep.desktop" "$STAGE/pisweep.desktop"
install -m 644 "$ROOT/dist/pisweep.svg" "$STAGE/pisweep.svg"
install -m 644 "$ROOT/LICENSE" "$STAGE/LICENSE"
install -m 644 "$ROOT/fonts/OFL.txt" "$STAGE/OFL.txt"

cat >"$STAGE/install.sh" <<'INSTALL'
#!/usr/bin/env bash
# Install a prebuilt Pisweep release into ~/.local. No root, no compiler.
set -euo pipefail
HERE="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
PREFIX="${PREFIX:-$HOME/.local}"

install -Dm755 "$HERE/pisweep" "$PREFIX/bin/pisweep"
install -Dm644 "$HERE/pisweep.desktop" "$PREFIX/share/applications/pisweep.desktop"
install -Dm644 "$HERE/pisweep.svg" "$PREFIX/share/icons/hicolor/scalable/apps/pisweep.svg"
install -Dm644 "$HERE/LICENSE" "$PREFIX/share/licenses/pisweep/LICENSE"
install -Dm644 "$HERE/OFL.txt" "$PREFIX/share/licenses/pisweep/OFL.txt"

if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database "$PREFIX/share/applications" >/dev/null 2>&1 || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
  gtk-update-icon-cache -f -t "$PREFIX/share/icons/hicolor" >/dev/null 2>&1 || true
fi

echo "Installed pisweep to $PREFIX/bin/pisweep"
INSTALL
chmod 755 "$STAGE/install.sh"

mkdir -p "$OUT_DIR"
TARBALL="$OUT_DIR/${NAME}.tar.gz"
tar -czf "$TARBALL" -C "$OUT_DIR" "$NAME"
echo "packaged: $TARBALL"