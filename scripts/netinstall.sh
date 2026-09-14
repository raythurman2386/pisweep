#!/usr/bin/env bash
set -euo pipefail

# Install pisweep from a prebuilt GitHub Release tarball.
#
# Downloads the release tarball plus its checksums.txt and checksums.txt.sig,
# verifies the checksums signature against the pinned Ed25519 public key
# (fail closed: no signature or a bad one refuses the install), verifies the
# tarball's SHA-256 against its checksums.txt entry, then extracts and runs
# the bundled install.sh (desktop entry, icons, licenses into ~/.local).

REPO="raythurman2386/pisweep"
# Base URL for release artifacts. Overridable so the installer can be tested
# against a local mirror without hitting GitHub.
DEFAULT_RELEASE_BASE_URL="https://github.com/$REPO/releases/download"
RELEASE_BASE_URL="${PISWEEP_RELEASE_BASE_URL:-$DEFAULT_RELEASE_BASE_URL}"
VERSION=""
PREFIX="${PREFIX:-$HOME/.local}"
FORCE=false

# Pinned Ed25519 public key (PEM) used to verify the release signature. This is
# the root of trust: it must match the key used by scripts/sign-release.sh.
# A release whose checksums.txt.sig does not verify against this key is refused.
SIGNING_PUBLIC_KEY='-----BEGIN PUBLIC KEY-----
MCowBQYDK2VwAyEAfdt6Sovk53nqRVnGZ9sgS6Q5oFqYnvH1c2pllFLtiOI=
-----END PUBLIC KEY-----'

print_usage() {
    cat <<EOF
Usage: $0 [OPTIONS] [VERSION]

Install pisweep from a prebuilt GitHub Release tarball.

Options:
  --prefix DIR  Install into DIR (default: \$HOME/.local, or \$PREFIX)
  --force       Overwrite an existing install without prompting
  -h, --help    Show this help message

Environment:
  PISWEEP_RELEASE_BASE_URL  Override the release artifact base URL

Examples:
  $0                # latest release into ~/.local
  $0 0.1.2          # a specific release
  $0 --prefix /opt/pisweep
EOF
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --prefix)
            PREFIX="$2"
            shift 2
            ;;
        --prefix=*)
            PREFIX="${1#--prefix=}"
            shift
            ;;
        --force)
            FORCE=true
            shift
            ;;
        -h|--help)
            print_usage
            exit 0
            ;;
        -*)
            echo "Unknown option: $1" >&2
            print_usage >&2
            exit 2
            ;;
        *)
            if [[ -n "$VERSION" ]]; then
                echo "Unexpected argument: $1" >&2
                print_usage >&2
                exit 2
            fi
            VERSION="$1"
            shift
            ;;
    esac
done

detect_triple() {
    local arch
    arch="$(uname -m)"
    case "$arch" in
        x86_64|amd64)  arch="x86_64" ;;
        aarch64|arm64) arch="aarch64" ;;
        *)
            echo "Error: unsupported architecture: $arch" >&2
            exit 1
            ;;
    esac
    echo "${arch}-unknown-linux-gnu"
}

get_latest_version() {
    local tag
    # Follow the /releases/latest redirect instead of the API. The API endpoint
    # is rate-limited to 60 req/hr per IP for unauthenticated clients, which
    # breaks installs on shared/NAT'd networks; the redirect is not limited.
    tag="$(curl -fsSL -o /dev/null -w '%{url_effective}' \
        "https://github.com/$REPO/releases/latest" 2>/dev/null \
        | sed -E 's#.*/tag/##')"
    if [[ -z "$tag" ]]; then
        echo "Error: could not determine latest version from GitHub" >&2
        exit 1
    fi
    echo "$tag"
}

# fetch SRC DEST
#
# Download SRC to DEST. When SRC is a local path (absolute, relative, or a
# file:// URL), copy it directly instead of using curl. This lets the installer
# run against a local mirror / offline directory, and makes it testable without
# network access.
fetch() {
    local src="$1" dst="$2"
    if [[ "$src" == file://* ]]; then
        cp "${src#file://}" "$dst"
    elif [[ "$src" == /* || "$src" == ./* || "$src" == ../* ]]; then
        cp "$src" "$dst"
    else
        curl -fsSL --retry 3 --retry-delay 2 -o "$dst" "$src"
    fi
}

main() {
    local triple version_tag
    triple="$(detect_triple)"

    if [[ -z "$VERSION" ]]; then
        version_tag="$(get_latest_version)"
    else
        version_tag="$VERSION"
        if [[ "$version_tag" != v* ]]; then
            version_tag="v$version_tag"
        fi
    fi

    local version_no_v="${version_tag#v}"
    local tarball="pisweep-${version_no_v}-${triple}.tar.gz"

    echo "==> Platform: $triple"
    echo "==> Version:  $version_tag"
    echo "==> Prefix:   $PREFIX"

    if [[ -x "$PREFIX/bin/pisweep" ]] && [[ "$FORCE" != true ]]; then
        echo "==> pisweep is already installed at $PREFIX/bin/pisweep"
        echo "    Use --force to overwrite."
        exit 0
    fi

    if ! command -v tar >/dev/null 2>&1; then
        echo "Error: tar is required to unpack the release tarball" >&2
        exit 1
    fi

    local tmp_dir
    tmp_dir="$(mktemp -d)"
    # tmp_dir is local to main(); the EXIT trap runs after main returns, so
    # reference it with ${tmp_dir:-} to avoid an "unbound variable" error
    # under `set -u` when the trap fires post-return.
    trap 'rm -rf "${tmp_dir:-}"' EXIT

    # Fail closed on authenticity: the checksums.txt signature is required and
    # must verify against the pinned Ed25519 public key. This proves the
    # checksums (and therefore the tarball) were produced by the pisweep
    # maintainers, not tampered with in transit or on the release host.
    echo "==> Fetching checksums and signature ..."
    if ! fetch "$RELEASE_BASE_URL/$version_tag/checksums.txt" "$tmp_dir/checksums.txt" 2>/dev/null; then
        echo "Error: failed to download checksums.txt for $version_tag" >&2
        echo "Refusing to install without a checksum. Verify the release is complete." >&2
        exit 1
    fi
    if ! fetch "$RELEASE_BASE_URL/$version_tag/checksums.txt.sig" "$tmp_dir/checksums.txt.sig" 2>/dev/null; then
        echo "Error: failed to download checksums.txt.sig for $version_tag" >&2
        echo "Refusing to install without a release signature." >&2
        exit 1
    fi
    if ! command -v openssl &>/dev/null; then
        echo "Error: openssl is required to verify the release signature" >&2
        exit 1
    fi
    local pubkey_file
    pubkey_file="$tmp_dir/pisweep-signing-key.pub"
    printf '%s\n' "$SIGNING_PUBLIC_KEY" > "$pubkey_file"
    if ! openssl pkeyutl -verify -rawin -in "$tmp_dir/checksums.txt" \
        -sigfile "$tmp_dir/checksums.txt.sig" \
        -pubin -inkey "$pubkey_file" >/dev/null 2>&1; then
        echo "Error: release signature verification FAILED for checksums.txt" >&2
        echo "Refusing to install: the release could not be authenticated." >&2
        exit 1
    fi
    echo "==> Signature OK"

    # Fail closed on integrity: the tarball entry must exist in the signed
    # checksums and the downloaded tarball must match it. Anchored to the
    # exact artifact name (end of line) so archive entries can't shadow it.
    echo "==> Downloading $tarball ..."
    if ! fetch "$RELEASE_BASE_URL/$version_tag/$tarball" "$tmp_dir/$tarball" 2>/dev/null; then
        echo "Error: failed to download $tarball" >&2
        echo "Check that the release exists and covers your architecture." >&2
        exit 1
    fi

    local expected
    expected="$(grep -E "^[0-9a-f]{64}  ${tarball}$" "$tmp_dir/checksums.txt" | awk '{print $1}')"
    if [[ -z "$expected" ]]; then
        echo "Error: no checksum entry found for $tarball in checksums.txt" >&2
        echo "Refusing to install an unverified tarball." >&2
        exit 1
    fi
    local actual
    if command -v sha256sum &>/dev/null; then
        actual="$(sha256sum "$tmp_dir/$tarball" | awk '{print $1}')"
    elif command -v shasum &>/dev/null; then
        actual="$(shasum -a 256 "$tmp_dir/$tarball" | awk '{print $1}')"
    else
        echo "Error: neither sha256sum nor shasum is available to verify the download" >&2
        exit 1
    fi
    if [[ "$actual" != "$expected" ]]; then
        echo "Error: checksum mismatch!" >&2
        echo "  expected: $expected" >&2
        echo "  actual:   $actual" >&2
        exit 1
    fi
    echo "==> Checksum OK"

    echo "==> Unpacking ..."
    local extract_dir
    extract_dir="$tmp_dir/extract"
    mkdir -p "$extract_dir"
    tar -xzf "$tmp_dir/$tarball" -C "$extract_dir"

    local package_dir
    package_dir="$(find "$extract_dir" -maxdepth 1 -type d -name 'pisweep-'"$version_no_v"'-*' | head -1)"
    if [[ -z "$package_dir" ]] || [[ ! -x "$package_dir/install.sh" ]]; then
        echo "Error: expected package dir with install.sh not found in the tarball" >&2
        exit 1
    fi

    echo "==> Running the bundled install.sh ..."
    PREFIX="$PREFIX" FORCE="$FORCE" bash "$package_dir/install.sh"

    echo "==> Installed pisweep $version_tag into $PREFIX"
}

main