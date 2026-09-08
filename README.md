# OVERFRAME

**An open, tournament-ready platform fighter.** Fast, precise, rollback-ready — and 100% original, free, and MIT-licensed.

![OVERFRAME demo](docs/media/overframe-demo.gif)

*The clip above is rendered by the actual game engine (not a pre-baked animation): dash-dance → wavedash → walk-in → tilt → forward smash → KO.*

[![CI](https://github.com/Gisleno-bit/overframe/actions/workflows/ci.yml/badge.svg)](https://github.com/Gisleno-bit/overframe/actions/workflows/ci.yml)
&nbsp;License: MIT &nbsp;•&nbsp; Language: Rust &nbsp;•&nbsp; Status: **Fase 1 prototype**

---

## What this is

OVERFRAME is a platform fighter that reproduces the *feel* of classic competitive platform fighting — the movement, the spacing game, the tech skill — as a game the community can own and run at tournaments **without any legal risk**. To make that possible, everything shippable is original and built from scratch:

- **Original everything** — characters, stage, names, art, sound, and code are all our own. No assets, ROMs, ISOs or decompiled code from any other game are used, referenced, or required.
- **Gameplay mechanics only.** What we borrow is *ideas* — a percent/knockback damage model, recovery mechanics, L-cancelling, wavedashing, teching. Game mechanics and rules are not copyrightable; the assets that express them are. We copy the former and invent the latter. See [`docs/LEGAL.md`](docs/LEGAL.md).
- **Free & open source (MIT).** No price, no lock-in. Fork it, host it, mod it.
- **Rollback-ready.** The simulation is deterministic and save/load-able, and it's already wired to [GGRS](https://github.com/gschup/ggrs). A GGRS `SyncTest` runs in CI and proves the engine is rollback-safe.

The prototype ships one fighter — **Kestrel**, an agile fast-faller — on one stage, **The Lattice**.

## Is this legal? (short version)

Yes. Copyright protects a game's *expression* (its art, music, characters, story, code), not its *ideas, rules, systems or mechanics*. Games like *Rivals of Aether*, *Brawlhalla*, *Fraymakers* and *Slap City* all recreate this genre's mechanics and have never had a problem, because they built their own assets. OVERFRAME follows the same, well-trodden path — and goes further by being open-source and free. The full reasoning, and the "clean-room" rules every contributor follows, is in [`docs/LEGAL.md`](docs/LEGAL.md). (This is not legal advice.)

## Play it

### Option A — download a build (no tools needed)

Grab the latest build for your OS from the [**Releases**](https://github.com/Gisleno-bit/overframe/releases) page:

- **Windows** → `overframe-windows-x86_64.zip` → unzip → run **`overframe.exe`**.
- **macOS** → `overframe-macos-arm64.tar.gz`.
- **Linux** → `overframe-linux-x86_64.tar.gz`.

These binaries are built automatically on a real Windows/macOS/Linux machine by [GitHub Actions](.github/workflows/release.yml) whenever a `vX.Y.Z` tag is pushed — so the Windows `.exe` is a genuine native Windows build.

### Option B — build it yourself (one command)

You need the [Rust toolchain](https://rustup.rs) (`rustup`). Then:

```bash
cargo run --release
```

On **Windows** that produces and runs `target\release\overframe.exe` — a standalone GUI executable, exactly what you want.
On **Linux** you also need the usual game dev libraries once:

```bash
sudo apt-get install -y libx11-dev libxi-dev libgl1-mesa-dev libasound2-dev
```

> **Why isn't a prebuilt `.exe` committed to the repo?** Shipping binaries in Git is bad practice (they bloat history and can't be audited). Instead, the exact same `.exe` is produced reproducibly by CI and attached to each Release — that's the standard, trustworthy way open-source games distribute executables.

## Controls

Two players share one keyboard (gamepad support is on the roadmap).

| Action    | Player 1        | Player 2          |
|-----------|-----------------|-------------------|
| Move      | `W A S D`       | Arrow keys        |
| C-stick   | `Q E R F`       | `I J K L`         |
| Jump      | `Space`         | `Right Shift`     |
| Attack    | `C`             | `.`               |
| Special   | `V`             | `/`               |
| Shield    | `Left Shift`    | `Right Ctrl`      |
| Grab      | `X`             | `,`               |

**Tech reference**

- **Tilt / smash** — Attack + a *held* stick direction is a tilt; a **C-stick** flick is a smash.
- **Short hop** — tap jump; **full hop** — hold it.
- **Fast fall** — flick down while descending.
- **Wavedash** — jump, then air-dodge (Shield) diagonally down-into the ground; you slide.
- **L-cancel** — press Shield in the last few frames before landing an aerial to halve its landing lag.
- **Ledge, roll, spot-dodge, air-dodge, tech** — all present. See the in-game **Controls** screen and [`docs/DESIGN.md`](docs/DESIGN.md).

Menu: **Versus (2P local)**, **Training** (with hit/hurtbox display — `Tab` to toggle, `Backspace` to reset), **Watch Demo**, **Controls**.

## How it's built

Three cleanly separated layers, which is what makes the game testable, replayable and (soon) networked:

```
src/
  sim/        Deterministic, engine-agnostic simulation. No rendering, window,
              audio or wall-clock. GameState::step(inputs) -> next tick.
  viz/        Backend-agnostic drawing: a Painter trait + one draw_scene().
  render/     macroquad front-end (window, menus, input, game loop) — impl Painter.
  headless.rs CPU rasteriser that renders the same scene to a GIF (no GPU/display).
  netcode.rs  GGRS rollback glue (Config, SyncTest, request handling).
  demo.rs     The scripted showcase match used by the GIF tool and attract mode.
```

Because the simulation is pure and input-driven, the **exact same `draw_scene`** powers both the game window and the headless GIF, and the **exact same `step`** runs locally and (in Fase 2) over the network.

### Determinism & rollback

Rollback netcode requires the simulation to be deterministic and perfectly save/load-able. OVERFRAME's `GameState` is a handful of `Vec`s with no external handles; `step` reads only its own fields plus inputs and never touches the clock or a global RNG (the RNG is seeded per match and saved). `tests/determinism.rs` drives the whole thing through a GGRS `SyncTestSession`, which re-simulates every frame from a saved state and compares checksums — if anything drifted, CI would fail.

> The simulation currently uses `f32`, which is bit-reproducible for two players on the same architecture running the same binary (the normal case). Cross-architecture bit-determinism (fixed-point math) is a documented Fase-2 hardening task; the math is isolated in `src/sim/math.rs` so the change is contained.

## Testing

```bash
# Fast, no system deps: physics/property tests + rollback determinism
cargo test --no-default-features --features netcode

# Everything (also links the GUI)
cargo test
```

`tests/mechanics.rs` asserts the *relationships* that make it feel right — a full hop clears a short hop, fast-falling lands sooner, dashing outruns walking, an angled air-dodge wavedashes, L-cancel cuts landing lag, knockback grows with percent, and the sim is deterministic. `tests/determinism.rs` is the GGRS rollback check.

## Roadmap

- **Fase 1 — prototype (this repo).** One fighter, one stage, full movement tech, knockback/percent/stocks, shields/rolls/ledges, local 2-player, training mode with hitbox display, deterministic rollback-ready core. ✅
- **Fase 2 — online.** Wire GGRS P2P (the glue is in `netcode.rs`), a Slippi-style matchmaking/direct-connect UI, and fixed-point determinism hardening. Gamepad support.
- **Fase 3 — content.** Expand the roster and stages, all-original art and audio, a real animation system.
- **Fase 4 — launch.** Ship as free software; community tournament support.

## Contributing

PRs welcome — see [`CONTRIBUTING.md`](CONTRIBUTING.md), especially the **clean-room rule**: never copy assets, data or code from another game, and don't add "reference" values pulled from another game's files. Mechanics from public community documentation (how a mechanic behaves) are fine; assets and data are not.

## License

[MIT](LICENSE). Original work by the OVERFRAME contributors. OVERFRAME is a homage built for the community; it is not affiliated with, endorsed by, or derived from any other game or company.
