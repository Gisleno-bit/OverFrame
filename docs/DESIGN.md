# Design & engineering notes

How OVERFRAME works under the hood, and the mechanics it models. All numbers
here are our own tuning values (see `src/sim/constants.rs` and
`src/sim/attacks.rs`); they were chosen for feel and are pinned by the property
tests in `tests/mechanics.rs`.

## Architecture

```
                 inputs (per player, per tick)
                          │
        ┌─────────────────▼───────────────────┐
        │  sim  — pure, deterministic          │   GameState::step(inputs)
        │  GameState { fighters, projectiles,  │   → advances exactly one
        │              stage, rng, frame, fx }  │     60 Hz tick
        └───────┬───────────────────┬──────────┘
                │                   │
     draw_scene │                   │ save / load / re-sim
                ▼                   ▼
   ┌────────────────────┐   ┌────────────────────┐
   │ viz::Painter        │   │ netcode (GGRS)      │
   │  ├ render (macroquad)│   │  SyncTest / P2P     │
   │  └ headless (GIF)    │   └────────────────────┘
   └────────────────────┘
```

- **`sim`** has zero rendering/window/audio/clock dependencies. It is the single
  source of truth for gameplay, and the only thing rollback saves and replays.
- **`viz`** is a three-primitive `Painter` trait (`fill_rect`, `fill_circle`,
  `line`) plus one `draw_scene`. The macroquad window and the headless GIF tool
  are just two `Painter` implementations, so they render pixel-identical scenes.
- **`netcode`** implements `ggrs::Config` and services GGRS requests. The same
  code path powers the local `SyncTest` (determinism proof) and, in Fase 2, P2P.

## The tick

Everything runs at a fixed **60 Hz**. `GameState::step`:

1. Advances each fighter's state machine (intent from input → movement/physics).
2. Spawns requested projectiles.
3. Resolves combat: active hitboxes vs. hurtboxes, shields, grabs, throws.
4. Keeps grabbed fighters attached; updates projectiles.
5. Regenerates shields; checks KOs (blast zones) and match end.

A fighter integrates position exactly once per tick (`pos += vel`), which keeps
the physics easy to reason about and deterministic.

## Movement model

Each character is pure data (`constants::Character`). Kestrel, the prototype
fighter, is an agile fast-faller. The model includes:

- **Ground:** walk / dash / run with distinct max speeds, ground friction, and a
  separate **traction** value used when sliding out of a wavedash.
- **Dash-dance:** flipping the stick within the dash window restarts the dash the
  other way instead of committing to a run.
- **Air:** horizontal air acceleration toward a max air speed, air friction, and
  gravity to a terminal fall speed.
- **Jumps:** a jump-squat, **short hop** (release jump during squat) vs **full
  hop** (hold it), and a double jump.
- **Fast fall:** flicking down while descending snaps to a higher fall speed.
- **Air-dodge → wavedash:** an air-dodge has a direction and intangibility; if it
  meets the ground at an angle, horizontal momentum is preserved and slides out
  with traction — i.e. a wavedash emerges from the air-dodge rules, it isn't a
  special case.
- **L-cancel:** pressing shield within `LCANCEL_WINDOW` frames before landing an
  aerial halves that aerial's landing lag.
- **Defense:** shield (shrinks with use and damage, can break), roll, spot-dodge,
  and out-of-shield jump/grab.
- **Ledges:** grab from off-stage with intangibility; get-up / jump / roll /
  attack; drop and regrab.
- **Hitstun & tech:** knockback puts you in hitstun (tumble above a threshold);
  hitting the ground while tumbling can be **teched** (with a lockout to prevent
  mashing) or becomes a knockdown.

## Damage, knockback, DI

Damage is percent-based. Knockback uses the genre's well-known public formula:

```
KB = (((( p/10 + p*d/20 ) · (200/(w+100)) · 1.4 ) + 18 ) · (KBG/100)) + BKB
```

where `p` = victim percent after the hit, `d` = move damage, `w` = victim
weight, `KBG` = knockback growth, `BKB` = base knockback. Hitstun is proportional
to knockback; **directional influence (DI)** rotates the launch angle by up to
~18° based on the component of the victim's stick perpendicular to the launch.
Both fighters freeze for a few **hitlag** frames scaled by damage.

> A formula is a mechanic, not an asset — reproducing it is exactly the kind of
> thing the legal strategy permits. The per-move numbers below are ours.

## Kestrel frame data (prototype)

Frames at 60 Hz. `su/ac/el` = startup / active / endlag. Aerials also list
landing lag (halved by L-cancel). These are tuning values, not taken from any
other game.

| Move        | su | ac | el | dmg | angle | KBG | BKB | land |
|-------------|---:|---:|---:|----:|------:|----:|----:|-----:|
| Jab         | 3  | 2  | 8  | 3   | 25    | 20  | 12  | –    |
| F-tilt      | 6  | 3  | 14 | 8   | 32    | 70  | 15  | –    |
| U-tilt      | 5  | 4  | 15 | 7   | 95    | 90  | 18  | –    |
| D-tilt      | 5  | 3  | 12 | 6   | 20    | 40  | 20  | –    |
| F-smash     | 12 | 3  | 26 | 15  | 38    | 95  | 25  | –    |
| U-smash     | 10 | 4  | 24 | 14  | 90    | 100 | 28  | –    |
| D-smash     | 9  | 3  | 24 | 13  | 28    | 90  | 30  | –    |
| Dash attack | 8  | 4  | 20 | 9   | 55    | 60  | 35  | –    |
| N-air       | 4  | 6  | 14 | 8   | 45    | 55  | 15  | 8    |
| F-air       | 7  | 4  | 18 | 10  | 40    | 75  | 15  | 12   |
| B-air       | 6  | 4  | 16 | 11  | 40    | 80  | 18  | 10   |
| U-air       | 5  | 4  | 14 | 9   | 85    | 70  | 20  | 9    |
| D-air       | 9  | 5  | 22 | 12  | 270   | 45  | 45  | 18   |
| Neutral-B   | 10 | 1  | 22 | (fires a projectile) | | | | – |
| Up-B        | 6  | 8  | 26 | 6   | 80    | 60  | 30  | –    |
| Side-B      | 10 | 6  | 24 | 9   | 30    | 55  | 45  | –    |
| Down-B      | 8  | 3  | 26 | 5   | 70    | 45  | 35  | –    |
| Throws (F/B/U/D) | — | — | — | 6–9 | varies | 50–65 | 45–55 | – |

## Determinism & rollback

Rollback netcode re-simulates past frames from a saved state, so the simulation
must be:

- **Deterministic** — driven only by inputs and a seeded, saved RNG; no wall
  clock, no global mutable state, one integration step per tick.
- **Save/load-able** — `GameState` is `Clone` with no external handles; the RNG
  and all timers are fields.

`tests/determinism.rs` feeds a busy 400-frame script through a GGRS
`SyncTestSession` with a rollback check distance of 2. GGRS saves state, advances,
rolls back, re-simulates, and compares a checksum (`GameState::checksum`) — any
non-determinism or lossy save is a test failure. This runs in CI.

The simulation uses `f32`. On one architecture with one binary (two Windows
x86-64 players, the common case) IEEE-754 ops are bit-reproducible, so SyncTest
passes and same-binary online play is safe. For guaranteed cross-architecture
determinism, Fase 2 replaces `f32` in `src/sim/math.rs` with a fixed-point type —
the module boundary is drawn so that change is localized.

## Extending it

- **New fighter:** add a `constants::Character` and (optionally) a moveset; the
  engine reads characters purely as data.
- **New stage:** add a `Stage` (platforms, ledges, blast zones, spawns).
- **New move:** add a `MoveId` + a row in `attacks::data`.
- **New effect / HUD:** it's all in `viz::draw_scene`, shared by both backends.
