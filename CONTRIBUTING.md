# Contributing to OVERFRAME

Thanks for helping build a free, open platform fighter for the community!

## The one rule that matters most: clean-room originality

OVERFRAME stays legally safe only because it contains **no** assets, data or code
from any other game. Every contributor must follow this:

- **Never** copy, paste, port or transcribe assets, data tables, or code from
  another game or from a decompilation/disassembly of one.
- **Never** add values "ripped" from another game's files (stats, exact frame
  data, real stage coordinates, etc.). Author tuning values here and justify them
  by feel and tests.
- Reproducing a **mechanic's behaviour** from public, community-written
  documentation is fine; reproducing an **asset or dataset** is not.
- When in doubt, open an issue first. See [`docs/LEGAL.md`](docs/LEGAL.md).

PRs that violate this cannot be merged, no matter how good the code is.

## Getting set up

```bash
# Install Rust: https://rustup.rs
# Linux GUI deps (once):
sudo apt-get install -y libx11-dev libxi-dev libgl1-mesa-dev libasound2-dev

cargo run --release            # play the game
cargo test --no-default-features --features netcode   # fast tests + rollback check
```

## Before you open a PR

- `cargo fmt --all`
- `cargo clippy --no-default-features --features netcode -- -D warnings`
- `cargo test --no-default-features --features netcode` (and `cargo test` if you
  touched the GUI)
- Keep the **simulation pure**: no rendering, window, audio, threading or
  wall-clock in `src/sim`. If it can't be saved/loaded and replayed identically,
  it doesn't belong in the sim. Determinism is not optional — rollback depends on
  it, and CI's GGRS `SyncTest` will catch regressions.
- New mechanics should come with a property test in `tests/mechanics.rs`
  asserting the *relationship* they create (not a magic number).

## Good first contributions

- Gamepad input mapping in `src/render`.
- A second stage (add a `Stage` in `src/sim/stage.rs`).
- Tuning passes on Kestrel's frame data (`src/sim/attacks.rs`) — with tests.
- Fixed-point math in `src/sim/math.rs` for cross-architecture determinism.
- Wiring GGRS P2P in `src/netcode.rs` behind a connect/lobby UI.

## Architecture at a glance

See [`docs/DESIGN.md`](docs/DESIGN.md). Short version: `sim` is the pure game,
`viz` draws it through a `Painter` trait, `render` is the macroquad window,
`headless` is the GIF/CI renderer, and `netcode` is the GGRS glue.

## Code of conduct

Be kind and constructive. This is a community project; treat contributors the way
you'd want to be treated at a friendly local tournament.
