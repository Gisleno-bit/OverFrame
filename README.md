# OVERFRAME

**An open, tournament-ready platform fighter.** Fast, precise, rollback-ready — and 100% original, free, and MIT-licensed.

![OVERFRAME demo](docs/media/overframe-demo.gif)

![Roster](docs/media/roster.png)

*The clip above is rendered by the actual game engine (not a pre-baked animation): dash-dance → wavedash → walk-in → tilt → forward smash → KO.*

[![CI](https://github.com/Gisleno-bit/overframe/actions/workflows/ci.yml/badge.svg)](https://github.com/Gisleno-bit/overframe/actions/workflows/ci.yml)
&nbsp;License: MIT &nbsp;•&nbsp; Language: Rust &nbsp;•&nbsp; Status: **Fase 3 (in progress) — roster, stages, online lobbies**

---

## What this is

OVERFRAME is a platform fighter that reproduces the *feel* of classic competitive platform fighting — the movement, the spacing game, the tech skill — as a game the community can own and run at tournaments **without any legal risk**. To make that possible, everything shippable is original and built from scratch:

- **Original everything** — characters, stage, names, art, sound, and code are all our own. No assets, ROMs, ISOs or decompiled code from any other game are used, referenced, or required.
- **Gameplay mechanics only.** What we borrow is *ideas* — a percent/knockback damage model, recovery mechanics, L-cancelling, wavedashing, teching. Game mechanics and rules are not copyrightable; the assets that express them are. We copy the former and invent the latter. See [`docs/LEGAL.md`](docs/LEGAL.md).
- **Free & open source (MIT).** No price, no lock-in. Fork it, host it, mod it.
- **Rollback-ready.** The simulation is deterministic and save/load-able, and it's already wired to [GGRS](https://github.com/gschup/ggrs). A GGRS `SyncTest` runs in CI and proves the engine is rollback-safe.

The build ships a **3-fighter roster** across the classic archetypes and **3
stages**:

| Fighter | Archetype | Signature trait |
|---|---|---|
| **Kestrel** | Fast-faller | *Momentum* — fastest fall speed, longest wavedash |
| **Boulder** | Heavyweight | *Bulwark* — super armour while charging a smash |
| **Viper** | Lightweight | *Skyline* — two air jumps, best air control |

Stages: **The Lattice** (floating-platform standard), **Meridian** (flat
neutral), **Tidegate** (asymmetric). Characters, stages, names, colours and
frame data are all original; the fighters are 2D placeholder art (an original,
labelled art style, not lookalikes) with a documented path to original 3D models
(`docs/DESIGN.md`).

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
sudo apt-get install -y libx11-dev libxi-dev libgl1-mesa-dev libasound2-dev libudev-dev
```

> **Why isn't a prebuilt `.exe` committed to the repo?** Shipping binaries in Git is bad practice (they bloat history and can't be audited). Instead, the exact same `.exe` is produced reproducibly by CI and attached to each Release — that's the standard, trustworthy way open-source games distribute executables.

## Controls

Two players share one keyboard, and any connected **gamepad** works too (first pad = P1, second = P2; keyboard and pad are merged, so either drives the fighter).

| Action    | Player 1        | Player 2          |
|-----------|-----------------|-------------------|
| Move      | `W A S D`       | Arrow keys        |
| C-stick   | `Q E R F`       | `I J K L`         |
| Jump      | `Space`         | `Right Shift`     |
| Attack    | `C`             | `.`               |
| Special   | `V`             | `/`               |
| Shield    | `Left Shift`    | `Right Ctrl`      |
| Grab      | `X`             | `,`               |

**Gamepad (default layout, rebindable in Options)**

| Action  | Buttons                         |
|---------|---------------------------------|
| Stick / C-stick | Left stick / Right stick |
| Attack  | A (South)                       |
| Special | B (East)                        |
| Jump    | Y (North) or X (West)           |
| Shield  | LT, RT (analog triggers) or LB  |
| Grab    | RB                              |

Gamepads are read with [`gilrs`](https://gitlab.com/gilrs-project/gilrs): XInput on Windows (Xbox pads and anything Steam/other drivers expose as XInput, e.g. a Switch Pro controller through Steam), evdev on Linux, IOKit on macOS. Rebind any action in **Options** (select the row, press Enter, press the button); bindings, input delay and the host port persist in the settings file (path shown at the bottom of the Options screen).

**Tech reference**

- **Tilt / smash** — Attack + a *held* stick direction is a tilt; a **C-stick** flick is a smash.
- **Short hop** — tap jump; **full hop** — hold it.
- **Fast fall** — flick down while descending.
- **Wavedash** — jump, then air-dodge (Shield) diagonally down-into the ground; you slide.
- **L-cancel** — press Shield in the last few frames before landing an aerial to halve its landing lag.
- **Ledge, roll, spot-dodge, air-dodge, tech** — all present. See the in-game **Controls** screen and [`docs/DESIGN.md`](docs/DESIGN.md).

Menu: **Versus** (2P local, with character/stage select and match rules), **Online** (host / join with a LAN room browser and lobby), **Training** (character + stage, hit/hurtbox display — `Tab` toggle, `Backspace` reset), **Options** (input delay, port, gamepad rebinding), **Controls**.

## Play online

![Online match](docs/media/online.png)

Online play is **peer-to-peer rollback** (GGRS) over a single UDP port — no account, no server, no matchmaking service.

1. **Host** → *Online → Host game*. You land in a **lobby** with a **room code**
   (e.g. `R0004-0GYF8`). The host sets the stage, stocks and time.
2. **Join** → *Online → Join game*. **LAN rooms appear automatically** in the
   browser; pick one, or type a room code / `ip:port`. Optional room password.
3. In the lobby both players pick **character + palette**, **chat**, and press
   **Ready**. When both are ready the host starts; GGRS synchronises and the
   match begins. The top-left HUD shows **ping**, **rollback frames**, how many
   frames you are **ahead**, and the **input delay**.

| Situation | What to do |
|---|---|
| **Same LAN** | Share the room code as shown. Done. |
| **Internet** | The host forwards **UDP port 7777** (or the port set in Options) on their router to their PC, then shares their **public IP** + port (`203.0.113.9:7777`) — or a room code built from it. UPnP auto-forwarding is on the roadmap; for now it's a manual router rule. |
| **Two instances on one PC (testing)** | `overframe --host 7801` and `overframe --join 127.0.0.1:7801`. Use a different settings dir per instance so they get different ids: `OVERFRAME_CONFIG_DIR=/tmp/a` / `OVERFRAME_CONFIG_DIR=/tmp/b` (on Windows PowerShell: `$env:OVERFRAME_CONFIG_DIR="C:\tmp\a"`). |

Command-line shortcuts: `overframe --host [port]`, `overframe --join <code|ip:port>`.

**Input delay.** Default 2 frames (≈33 ms of local delay traded for shorter rollbacks). Lower it to 1 for LAN, raise to 3–4 on bad connections (*Options → Input delay*). Both sides use the host's setting.

**How it stays in sync.** The simulation is deterministic and save/load-able, so GGRS predicts the remote input, and when the real input arrives a few frames later it rolls back and re-simulates. Desync detection is on (checksums compared every 30 frames); if the engine ever disagreed you'd see a red **DESYNC DETECTED** banner — that is a bug report, not something a player can cause. Two players on the same architecture with the same build never desync; cross-architecture bit-exactness (fixed-point math) is the remaining hardening item.

**Identity.** On first run the game generates a random 64-bit `player_id` (stored in the settings file; no hardware or personal data). It is exchanged in the handshake so tournament organisers can refer to a stable id — the groundwork for the ban list described in [`docs/DESIGN.md`](docs/DESIGN.md#ban-system). A host with a `bans.txt` next to its settings file refuses listed ids.

## How it's built

Three cleanly separated layers, which is what makes the game testable, replayable and (soon) networked:

```
src/
  sim/        Deterministic, engine-agnostic simulation. No rendering, window,
              audio or wall-clock. GameState::step(inputs) -> next tick.
  viz/        Backend-agnostic drawing: a Painter trait + one draw_scene().
  render/     macroquad front-end: window, menus, options, online screens, the
              fixed-timestep loop; input.rs merges keyboard + gilrs gamepads.
  netcode/    GGRS rollback: mod.rs (Config, SyncTest, request handling),
              session.rs (host/join state machine on P2PSession), socket.rs
              (one UDP socket for handshake + GGRS), handshake.rs, roomcode.rs,
              banlist.rs.
  gamepad.rs  Pure gamepad → PlayerInput mapping and bindings (unit-tested).
  config.rs   Settings file: player_id, port, input delay, pad bindings.
  headless.rs CPU rasteriser that renders the same scene to a GIF (no GPU/display).
  demo.rs     The scripted showcase match used by the GIF tool and attract mode.
```

Because the simulation is pure and input-driven, the **exact same `draw_scene`** powers both the game window and the headless GIF, and the **exact same `step`** runs locally and over the network.

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

`tests/mechanics.rs` asserts the *relationships* that make it feel right — a full hop clears a short hop, fast-falling lands sooner, dashing outruns walking, an angled air-dodge wavedashes, L-cancel cuts landing lag, knockback grows with percent, and the sim is deterministic. `tests/determinism.rs` is the GGRS `SyncTest` rollback check.

`tests/content.rs` checks every character has a complete, sane moveset and a genuinely distinct archetype, and that stages are well-formed. `tests/netcode_flow.rs` covers the online layer end to end: room-code and handshake round-trips, the ban-list format, and — the important one — **a real host and guest connecting over loopback UDP, synchronising through GGRS, playing 300 frames with deliberately uneven pacing so rollbacks actually happen, and finishing on byte-identical states**. It also checks that a banned id is refused. `tests/gamepad_map.rs` covers the gamepad mapping, deadzones, rebinding and persistence without hardware.

## Roadmap

- **Fase 1 — prototype (this repo).** One fighter, one stage, full movement tech, knockback/percent/stocks, shields/rolls/ledges, local 2-player, training mode with hitbox display, deterministic rollback-ready core. ✅
- **Fase 2 — online (this release).** GGRS P2P rollback netcode over direct UDP with room codes, host/join UI, live ping/rollback HUD, gamepad support with in-game rebinding, persistent player identity and the ban-list data format. ✅ Still open from Fase 2: fixed-point determinism hardening (see below).
- **Fase 3 — content + Steam (in progress).** 3 fighters across archetypes, 3
  stages, character/stage select, match rules (stocks/time), LAN lobbies with a
  room browser + chat, Steam-ID-ready ban list. ✅ Remaining: more fighters/stages
  toward 8-10, original 3D models + animation, and the Steam integration
  (matchmaking, invites, SDR transport) planned in `docs/STEAM.md`.
- **Fase 4 — launch.** Ship free on Steam (Early Access); community tournament
  support.

## Contributing

PRs welcome — see [`CONTRIBUTING.md`](CONTRIBUTING.md), especially the **clean-room rule**: never copy assets, data or code from another game, and don't add "reference" values pulled from another game's files. Mechanics from public community documentation (how a mechanic behaves) are fine; assets and data are not.

## License

[MIT](LICENSE). Original work by the OVERFRAME contributors. OVERFRAME is a homage built for the community; it is not affiliated with, endorsed by, or derived from any other game or company.
