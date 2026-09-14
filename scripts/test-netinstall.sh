#!/usr/bin/env bash
set -euo pipefail

# Local test harness for the netinstaller's integrity + authenticity checks.
#
# Builds a fake release (tarball + checksums.txt + checksums.txt.sig) in a
# local directory and runs scripts/netinstall.sh against it using local-mirror
# mode (PISWEEP_RELEASE_BASE_URL pointing at a filesystem path). Verifies that:
#   1. a clean, correctly-signed release installs successfully;
#   2. a tampered tarball is refused (checksum mismatch);
#   3. a tampered checksums.txt is refused (signature verification fails);
#   4. a missing signature is refused (fail closed).
#
# No network access is required. Requires: bash, openssl, sha256sum, tar, and
# a temporary copy of package-linux.sh output (built here from a fake binary,
# so the real binary does not need to exist).

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
NETINSTALL="$ROOT/scripts/netinstall.sh"
SIGN_SH="$ROOT/scripts/sign-release.sh"

VERSION="0.0.0"
VERSION_TAG="v$VERSION"

arch="$(uname -m)"
case "$arch" in
    x86_64|amd64) arch="x86_64" ;;
    aarch64|arm64) arch="aarch64" ;;
    *) echo "unsupported arch: $arch" >&2; exit 1 ;;
esac
TRIPLE="${arch}-unknown-linux-gnu"
TARBALL="pisweep-${VERSION}-${TRIPLE}.tar.gz"

WORK="$(mktemp -d)"
RELEASE_DIR="$WORK/release/$VERSION_TAG"
INSTALL_DIR="$WORK/install"
KEYS="$WORK/keys"

cleanup() {
    rm -rf "$WORK"
}
trap cleanup EXIT

fail() {
    echo "FAIL: $1" >&2
    exit 1
}

pass() {
    echo "PASS: $1"
}

# --- helpers ---------------------------------------------------------------

# Build the fake release tarball: a package dir whose install.sh is a stub
# that records its PREFIX, mirroring scripts/package-linux.sh's layout.
make_fake_tarball() {
    local stage="$WORK/stage/pisweep-${VERSION}-${TRIPLE}"
    rm -rf "$WORK/stage"
    mkdir -p "$stage"
    cat > "$stage/install.sh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
PREFIX="${PREFIX:-$HOME/.local}"
mkdir -p "$PREFIX/bin"
printf '#!/usr/bin/env bash\necho "pisweep %s (fake)"\n' "$(basename "$0")" > "$PREFIX/bin/pisweep"
chmod +x "$PREFIX/bin/pisweep"
EOF
    chmod +x "$stage/install.sh"
    tar -czf "$RELEASE_DIR/$TARBALL" -C "$WORK/stage" "pisweep-${VERSION}-${TRIPLE}"
}

write_checksums() {
    # checksums.txt uses two spaces between hash and name (matches netinstall.sh).
    local hash
    hash="$(sha256sum "$RELEASE_DIR/$TARBALL" | awk '{print $1}')"
    printf '%s  %s\n' "$hash" "$TARBALL" > "$RELEASE_DIR/checksums.txt"
}

run_install() {
    # Run the netinstaller against the local release directory.
    PISWEEP_RELEASE_BASE_URL="$WORK/release" \
        bash "$NETINSTALL" --prefix "$INSTALL_DIR" --force "$VERSION" \
        >"$WORK/out.log" 2>&1
}

reset_state() {
    rm -rf "$RELEASE_DIR" "$INSTALL_DIR"
    mkdir -p "$RELEASE_DIR" "$INSTALL_DIR"
}

# --- setup: generate a keypair and patch netinstall.sh's pinned key --------

mkdir -p "$RELEASE_DIR" "$INSTALL_DIR" "$KEYS"

# Generate a fresh keypair. The harness patches a temp copy of netinstall.sh
# to pin this generated public key, so the test is self-contained (it does not
# depend on the committed public key matching a secret we can produce here).
openssl genpkey -algorithm ED25519 -out "$KEYS/secret.pem" 2>/dev/null
openssl pkey -in "$KEYS/secret.pem" -pubout -out "$KEYS/public.pem" 2>/dev/null

NETINSTALL_PATCHED="$WORK/netinstall.sh"
python3 - "$NETINSTALL" "$KEYS/public.pem" "$NETINSTALL_PATCHED" <<'PY'
import sys, re
src, pub, dst = sys.argv[1], sys.argv[2], sys.argv[3]
text = open(src).read()
pubkey = open(pub).read().strip()
patched = re.sub(
    r"-----BEGIN PUBLIC KEY-----\n.*?\n-----END PUBLIC KEY-----",
    pubkey,
    text,
    count=1,
    flags=re.S,
)
open(dst, "w").write(patched)
PY
NETINSTALL="$NETINSTALL_PATCHED"

# --- case 1: clean install succeeds ---------------------------------------

make_fake_tarball
write_checksums
bash "$SIGN_SH" "$RELEASE_DIR/checksums.txt" "$KEYS/secret.pem" >/dev/null

rc=0
run_install || rc=$?
if [[ $rc -ne 0 ]]; then
    cat "$WORK/out.log" >&2
    fail "clean install should succeed"
fi
[[ -x "$INSTALL_DIR/bin/pisweep" ]] || fail "pisweep binary not installed"
grep -q "Signature OK" "$WORK/out.log" || fail "expected 'Signature OK' in output"
grep -q "Checksum OK" "$WORK/out.log" || fail "expected 'Checksum OK' in output"
pass "case 1: clean signed release installs"

# --- case 2: tampered tarball is refused -----------------------------------

reset_state
make_fake_tarball
write_checksums
bash "$SIGN_SH" "$RELEASE_DIR/checksums.txt" "$KEYS/secret.pem" >/dev/null
# Tamper with the tarball AFTER checksums were computed.
printf 'evil payload\n' >> "$RELEASE_DIR/$TARBALL"

rc=0
run_install || rc=$?
if [[ $rc -eq 0 ]]; then
    fail "tampered tarball should be refused"
fi
grep -q "checksum mismatch" "$WORK/out.log" || fail "expected checksum mismatch message"
[[ ! -e "$INSTALL_DIR/bin/pisweep" ]] || fail "tampered tarball must not be installed"
pass "case 2: tampered tarball refused"

# --- case 3: tampered checksums.txt is refused -----------------------------

reset_state
make_fake_tarball
write_checksums
bash "$SIGN_SH" "$RELEASE_DIR/checksums.txt" "$KEYS/secret.pem" >/dev/null
# Tamper with checksums.txt AFTER signing (signature will no longer verify).
printf 'deadbeef  %s\n' "$TARBALL" >> "$RELEASE_DIR/checksums.txt"

rc=0
run_install || rc=$?
if [[ $rc -eq 0 ]]; then
    fail "tampered checksums.txt should be refused"
fi
grep -q "signature verification FAILED" "$WORK/out.log" || fail "expected signature failure message"
[[ ! -e "$INSTALL_DIR/bin/pisweep" ]] || fail "tarball must not be installed on bad signature"
pass "case 3: tampered checksums.txt refused"

# --- case 4: missing signature is refused (fail closed) --------------------

reset_state
make_fake_tarball
write_checksums
# Intentionally do NOT sign — no checksums.txt.sig present.

rc=0
run_install || rc=$?
if [[ $rc -eq 0 ]]; then
    fail "missing signature should be refused"
fi
grep -q "without a release signature" "$WORK/out.log" || fail "expected missing-signature message"
[[ ! -e "$INSTALL_DIR/bin/pisweep" ]] || fail "tarball must not be installed without a signature"
pass "case 4: missing signature refused (fail closed)"

echo ""
echo "All netinstaller integrity/authenticity tests passed."