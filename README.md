# Pisweep

Classic Minesweeper for the pi suite, built with [GPUI Kit](https://github.com/longbridge/gpui-kit).
Rules and the hint AI are a Rust port of [omamine](https://github.com/raythurman2386/omamine).

The board engine and the knowledge solver are pure Rust with no UI imports, so first-click safety, flood-fill, chords, wins/losses, and certain hints are covered by unit tests.

## Install

User-local install (binary, icon, launcher). No root:

```sh
./scripts/install.sh
```

That puts `pisweep` on `~/.local/bin` and a desktop entry in the app launcher. Uninstall with `./scripts/uninstall.sh`.

Or install straight from a tagged release without cloning:

```sh
curl -fsSL https://raw.githubusercontent.com/raythurman2386/pisweep/main/scripts/netinstall.sh | bash
```

The netinstaller resolves the latest `v*` release, verifies its `checksums.txt` against a pinned Ed25519 public key (fail closed — no signature or a bad one refuses the install), checks the tarball's SHA-256, then installs into `~/.local` (override with `--prefix DIR`, or a version argument: `... | bash -s -- 0.1.0`).

Tagged releases (`v*`) build Linux tarballs on GitHub Actions for x86_64 and aarch64 (Raspberry Pi 5 and other 64-bit ARM boards), each requiring glibc 2.39+ (Debian 13, Ubuntu 24.04, current Raspberry Pi OS). Unpack the one for your machine and run `./install.sh` inside.

## Release signing

Every release's `checksums.txt` is signed with an Ed25519 key, so an installer can prove the checksums (and therefore the tarball) came from this repo:

- `bash scripts/gen-signing-key.sh` generates the keypair into `~/.pisweep/signing` — the secret key stays offline forever and is never committed, used in CI, or uploaded. Only the public key is committed (`pisweep-signing-key.pub`) and pinned in the installers.
- `bash scripts/sign-release.sh CHECKSUMS_FILE SECRET_KEY` signs one file; `scripts/sign-releases.sh VERSION...` batch-signs published releases offline into `~/.pisweep/signing/releases/<version>/`; `scripts/upload-release-sigs.sh VERSION...` attaches each `checksums.txt.sig` back to its release with `gh release upload --clobber`.

Verification on the installer side is fail-closed: a release without a signature, or whose signature does not verify against the pinned public key, is refused.

## Run from source

```sh
cargo run --release
```

## Play

| Input | Action |
| --- | --- |
| Left click / space / enter | Open |
| Right click / `f` / `x` | Flag |
| Middle click / `c` | Chord |
| Arrows or `hjkl` | Cursor |
| `n` or the face | New game |
| Hint / `a` | One certain solver move |
| `1` `2` `3` | Beginner, Intermediate, Expert |
| `?` | Keys |
| `Ctrl+Q` | Quit |

The first click is always safe. Best times are stored in `~/.local/share/pisweep/stats.json`. Hover any control or tile for a tooltip; the face, hint, and difficulty buttons also show their keybindings.

## Fonts

The iA Writer Mono font is bundled under the SIL Open Font License 1.1; see `fonts/OFL.txt`. The font is copyright Information Architects Inc. and based on IBM Plex, copyright IBM Corp.
