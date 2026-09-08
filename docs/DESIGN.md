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
  code path powers the local `SyncTest` (determinism proof) and the online
  `P2PSession`. See "Online play" below.

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
determinism, a later pass replaces `f32` in `src/sim/math.rs` with a
fixed-point type — the module boundary is drawn so that change is localized.

## Online play (Fase 2)

### Topology: direct P2P, no server

Two peers, one UDP socket each, GGRS rollback between them. There is no
matchmaking/signalling server: the guest needs the host's address, which the
host shares as a **room code** (`XXXXX-XXXXX` = 48 bits of IPv4 + port in
Crockford base-32) or as a plain `ip:port`. This is the simplest thing that
works for LAN and for internet play with a forwarded port, and it keeps the
project dependency-free on any hosted service — a deliberate Fase 2 choice.

Trade-off: no NAT traversal. Two players behind home routers need the host to
forward the UDP port. Options for later, in increasing cost: UPnP-IGD from the
host (automatic forwarding on most home routers), a tiny community-run STUN/relay
(hole punching; ~100 lines on the client, one small public server), or a lobby
service (Slippi-style, needs an account model).

### Connection lifecycle

```
guest                                   host
  │  Hello{player_id, version, name}  ─►  │  check version, check ban list
  │  ◄─  Welcome{player_id, seed, delay}  │  (or Reject{reason})
  │  build P2PSession(local=1, remote=0)  │  build P2PSession(local=0, remote=1)
  │  ◄────── GGRS sync packets ────────►  │
  │  Running: add_local_input / advance_frame, 60 Hz, both sides
```

* **Handshake before GGRS.** GGRS needs the remote address at session-build
  time, but the host doesn't know who will connect. `netcode::socket::OfSocket`
  therefore speaks both protocols on **one** socket: datagrams that start with
  the `OVERFRHS` magic are handshake messages; anything else is a
  bincode-serialised `ggrs::Message` (the same encoding as GGRS's own UDP
  socket, so it is wire-compatible). One port means one NAT mapping.
* **Seed in `Welcome`.** Both peers construct `GameState::new(2, {seed})`
  from the host's seed, so the deterministic RNG matches from frame 0.
* **Protocol version** in `Hello`/`Welcome`. Any change to gameplay-affecting
  rules bumps `PROTOCOL_VERSION`; mismatched builds are refused instead of
  desyncing ten seconds in.
* **Identity.** `player_id` is a random 64-bit value generated once and stored
  in the settings file. It is exchanged in the handshake and checked against
  the local ban list (below).

### Frame pacing with a fixed timestep

`render` keeps a 60 Hz accumulator. Each tick it calls `NetMatch::advance`,
which: honours GGRS `WaitRecommendation` events (skip N ticks so the slower
peer catches up), skips a tick if `frames_ahead() >= 3`, then
`add_local_input` + `advance_frame`. A `PredictionThreshold` error (we are
more than `MAX_PREDICTION = 8` frames ahead of confirmed remote input) simply
stalls that tick; the HUD shows "WAITING FOR PEER" if that persists.
`NetMatch::poll` runs every *rendered* frame regardless, so sockets are drained
and the handshake progresses even when no tick is due.

### Why rollback is cheap here

A rollback re-simulates up to 8 frames. `GameState::step` for two fighters is
a few thousand floating-point operations and `GameState` is a handful of small
`Vec`s, so a save is a clone of a few hundred bytes and a worst-case rollback
is well under 0.2 ms. Rendering is untouched: the window draws whatever state
the last `advance` left, so 60 FPS is not at risk from the netcode itself. The
real 60 FPS hazards are the usual macroquad ones — vsync hiccups and the OS
scheduler — which the accumulator absorbs (capped at 5 ticks per frame to
avoid a spiral of death).

### Determinism status

`f32` simulation; bit-exact on one architecture with one binary (two Windows
x86-64 players on the same release: the normal case). GGRS desync detection is
**on** (checksums every 30 frames) and surfaces as a red banner, so a genuine
divergence is visible immediately rather than silently corrupting a set.
Cross-architecture bit-exactness (e.g. x86-64 vs. Apple Silicon) is not
guaranteed until the fixed-point math pass; the numeric code is isolated in
`sim/math.rs` and `sim/constants.rs` for exactly that migration.

## Ban system

Goal: tournament organisers can exclude a player from their events' online
brackets **without a central server** and without the game phoning home.

### Data model (implemented now)

* Every install has a stable random `player_id` (u64). It is not derived from
  hardware — a hardware hash would be both a privacy problem and trivially
  spoofable in an open-source client — so the id is an *identity*, not a
  proof of identity. That is enough for the actual use case (organisers
  refusing known-bad actors from their own events) and honest about what it
  can't do (stop a determined person from generating a new id).
* `netcode::banlist::BanList`: a plain-text file, `bans.txt` in the settings
  directory:

  ```text
  # OVERFRAME ban list
  issuer Madrid Weekly TO
  ban 3fa9c1d2e4b5a678 until 1767225600 reason repeated no-shows
  ban 00ddeeff00112233 reason cheating
  sig <signature, Fase 3>
  ```
  Entries can expire (`until`, unix seconds). A host loads the file at match
  creation and answers `Hello` from a listed id with `Reject{BANNED: reason}`;
  the guest shows the reason. This is live today and covered by
  `tests/netcode_flow.rs::host_rejects_banned_guest`.

### Distribution & trust (Fase 3 design)

1. **Signed lists, not trusted servers.** Each organiser (or a regional
   community) holds an ed25519 keypair. They publish their list at any URL
   (a GitHub repo, a pastebin, their Discord) with `sig` = signature over the
   canonical text above the `sig` line. Players who want to *enforce* a list
   when hosting import it; the client verifies the signature against the
   issuer's public key, which it learns once (QR/text in the community's
   rules post) and pins. `BanList::verification()` is the stub where this
   lands; `Unverified` becomes `Valid(issuer)` / `Invalid`.
2. **Multiple issuers, local choice.** A player can subscribe to several
   lists. Enforcement is local and opt-in: a list only affects matches *you
   host*. Nobody can ban anyone globally — there is no global.
3. **Tournament mode.** A TO runs the bracket's hosts (or players host with
   the TO's list enabled); a `Hello` from a listed id is refused with the
   reason, and the guest sees it. Because the id is in every handshake, a TO
   can also *log* ids of players in their bracket to build the list in the
   first place, with consent stated in the event rules.
4. **Revocation and expiry.** Lists are re-fetched on a schedule the player
   chooses (or manually); `until` handles temporary bans without republishing.
5. **What it does not do.** It does not stop id regeneration (delete the
   settings file → new id). Mitigations if a community needs them: tie ids to
   a bracket registration (start.gg id in the `Hello` name field, verified by
   the TO out of band), or a proof-of-work cost on new ids. Both are additive
   and keep the no-server property.

Implementation plan for Fase 3: add `ed25519-dalek` (pure Rust, small), a
canonicalisation function for the list text, `Settings.trusted_issuers`,
signature verification in `BanList::verification`, and a "Ban lists" options
page (import from URL/file, show issuer + entry count + validity).

## Content model (Fase 3)

Characters and stages are **pure data**, so the roster and stage list grow
without touching engine code.

* A character is a `roster::Character` attribute block (movement, weight, size,
  a signature *trait*) plus a move table in `attacks.rs`. `CharacterId` is a
  `u8`-repr enum that also travels in the lobby protocol. Adding a fighter is:
  add an enum variant, one `Character` const, one move-table function, one match
  arm — and `tests/content.rs` immediately checks it has a complete, sane
  moveset and a distinct archetype.
* A stage is a `stage::Stage` (platforms, ledges, blast zones, spawns, colour
  `Theme`). `StageId` is likewise a `u8` enum. The renderer reads the theme, so
  a new stage brings its own palette for free.

Current roster (archetype spread): **Kestrel** fast-faller (Momentum: fastest
fall, longest wavedash), **Boulder** heavyweight (Bulwark: super armour during
smash startup), **Viper** lightweight (Skyline: two air jumps, best air
control). Stages: **The Lattice** (battlefield-like), **Meridian** (flat),
**Tidegate** (asymmetric). Signature traits live in the simulation
(`smash_armor`, `air_jumps`) and are covered by tests.

Match rules (`MatchConfig`) carry stage, per-slot character + palette, stock
count and an optional time limit; a timed match ends on the clock with a
most-stocks-then-lowest-percent tiebreak. All of it is part of the saved state,
so it is rollback- and lobby-safe.

### 3D asset pipeline (planned)

The 2D capsules are placeholders. The planned path to models keeps the data
model intact: author original rigged models in Blender → export glTF 2.0 →
load with a Rust glTF loader → drive with the existing per-move frame data
(startup/active/endlag windows become animation events; the hitboxes stay in
`attacks.rs`). Because gameplay reads a character purely through `Character` +
the move table, the renderer can move from capsules to skinned meshes without
changing the simulation or the netcode. If no artist is available, models must
be CC0/CC-BY (credited) and visibly unlike any existing franchise character —
see `docs/LEGAL.md`.

## Steam integration (Fase 3 plan)

The whole online stack — handshake, lobby, GGRS session, ban logic — is written
against `Identity` and an `impl NonBlockingSocket`, never against "UDP" or "an
IP". Steam therefore plugs in behind `netcode::platform::Platform` without
touching gameplay:

| Concern | LAN backend (shipped) | Steam backend (planned) |
|---|---|---|
| Identity | random `Identity::Local` | `Identity::Steam(SteamID64)` from `steamworks` |
| Auth | none | Steam session ticket |
| Room list | UDP broadcast (`lobby::Browser`) | `ISteamMatchmaking` lobby list + metadata (region, ping) |
| Invite / join | room code / `ip:port` | friend invite, `GameLobbyJoinRequested` |
| Transport | our `OfSocket` (UDP) | `ISteamNetworkingMessages` over **Steam Datagram Relay** |
| NAT / port forwarding | manual | none needed (SDR relays) |
| Ban keys | local id | Steam id (already supported by `banlist`) |

Concretely, Fase 3 adds a `steam` cargo feature that pulls the `steamworks`
crate and provides: a `SteamPlatform: Platform`, a `SteamSocket:
NonBlockingSocket<SteamId>` wrapping `ISteamNetworkingMessages`, and a
`NetMatch` constructor that builds its `P2PSession` on that socket. Lobby state
messages (`Pick`/`Rules`/`Chat`/`Start`) ride Steam lobby chat or SDR messages
instead of our datagrams; the *content* is unchanged. Steamworks is a C++ SDK
loaded as a dynamic library at runtime; there is no dependency conflict with
GGRS (different layers), and GGRS never learns which transport it is on.

The Steamworks setup checklist (App ID, depots, SDK placement, Early-Access
manifest) is in `docs/STEAM.md`.

## Extending it

- **New fighter:** add a `constants::Character` and (optionally) a moveset; the
  engine reads characters purely as data.
- **New stage:** add a `Stage` (platforms, ledges, blast zones, spawns).
- **New move:** add a `MoveId` + a row in `attacks::data`.
- **New effect / HUD:** it's all in `viz::draw_scene`, shared by both backends.
