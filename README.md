# Pisweep

Minesweeper for the [pi suite](https://github.com/raythurman2386), built with
[GPUI Kit](https://github.com/longbridge/gpui-kit) — a small, native,
theme-following desktop game written for Raspberry Pi 5-class hardware (and
happy on any Linux desktop). Rules and the hint AI are a Rust port of
[omamine](https://github.com/raythurman2386/omamine).

## Features

- **Three classic boards** — Beginner (9×9, 10 mines), Intermediate
  (16×16, 40), Expert (30×16, 99) — with first-click safety, flood-fill
  reveals, and chord-clicking.
- **Certain-solve hints**: the hint AI is a pure deduction solver (sentence
  constraints, subset differences, remaining-mine counts). It never guesses;
  `a` plays one provably safe reveal or flag, or tells you there is none.
- **Game tools**: flagging, remaining-mine counter, timer, best times per
  difficulty, and a face button to start over.
- **Aesthetic**: keyboard-first, tooltips on every control (the face, hint,
  and difficulty buttons also show their keybindings), and live desktop
  theming (pimarchy/Omarchy palette + text scale).

The board engine and the solver are pure Rust with no UI imports, so
first-click safety, flood-fill, chords, wins/losses, and hint certainty are
covered by unit tests (23 across the suite).

## Install

User-local install from a tagged release (no root, Ed25519-verified,
fail-closed):

```sh
curl -fsSL https://raw.githubusercontent.com/raythurman2386/pisweep/main/scripts/netinstall.sh | bash
```

Or build and install from source:

```sh
cargo build --release
./scripts/install.sh
```

Uninstall with `./scripts/uninstall.sh`. The netinstaller accepts a `--prefix`
directory, an optional version argument, and `--force`; the source install
honors `PREFIX=DIR`.

Tagged `v*` releases also build x86_64 + aarch64 tarballs on GitHub Actions
(glibc 2.39+ — e.g. Raspberry Pi OS / Debian 13). Unpack the one for your
architecture and run `./install.sh` inside.

Releases are authenticated with Ed25519 signatures over `checksums.txt`; the
public key is committed as `pisweep-signing-key.pub` and pinned in the
installer, which refuses anything it cannot verify.

## Keyboard

| Keys | Action |
|---|---|
| Click / `space` / `enter` | Open a tile |
| Right click / `f` / `x` | Flag |
| Middle click / `c` | Chord |
| Arrows / `hjkl` | Move the cursor |
| `n` / the face | New game |
| `a` | One certain solver move (hint) |
| `1` / `2` / `3` | Beginner · Intermediate · Expert |
| `?` | Help overlay · `F11`/`Super+F` fullscreen · `Ctrl+Q` quit |

## State and theming

- Best times live in `~/.local/share/pisweep/stats.json`.
- Colors follow the desktop theme —
  `~/.local/state/pimarchy/current/theme/colors.toml` first, then Omarchy —
  re-tinting live on theme switches; text follows the desktop text scale.
  `PISWEEP_THEME_DIR` overrides the search for tests.

## Fonts

The iA Writer Mono font is bundled under the SIL Open Font License 1.1; see
`fonts/OFL.txt`. The font is copyright Information Architects Inc. and based
on IBM Plex, copyright IBM Corp.

## Development

```sh
cargo fmt --check          # formatting
cargo clippy --all-targets -- -D warnings
cargo test                 # 23 tests
cargo run --release        # play
```

CI runs fmt, clippy, and tests on every push; tagged `v*` releases build
x86_64 + aarch64 tarballs (glibc 2.39+) with an install smoke test, and the
netinstall integrity harness can be run locally with
`bash scripts/test-netinstall.sh`.

## License

MIT — see [LICENSE](LICENSE). Bundled fonts: SIL OFL 1.1 (see above).