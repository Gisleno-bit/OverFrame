# Changelog

All notable changes to OVERFRAME. Format follows *Keep a Changelog*; versions
follow SemVer once `v1.0.0` ships.

## [0.5.0] — Game feel: reference-calibrated impact and response

The combat engine was re-tuned against public platform-fighter frame data
(mechanics and formulas only; every per-move number stays original). The
full record — reference summary, diagnosis, before/after tables, plan,
pseudocode and measurements — is `docs/GAME_FEEL.md`.

### Added
- **Hitlag formula** `⌊d/3+3⌋` (×1.5 electric, ×2/3 crouch-cancel, cap 20)
  for both fighters; **electric** moves; **crouch cancelling**.
- **SDI / ASDI** during the freeze, and **DI is read on the last hitlag
  frame** (`pending_launch`), so players react during hitstop like the
  reference.
- **Grounded flinch**: hits below the tumble threshold on a grounded victim
  slide them along the floor instead of lifting them (`ground_stun`).
- **Sakurai angle** (`361` in move tables): 0° shove at low knockback, 44°
  launch otherwise.
- **Shield rules**: additive shieldstun `⌊(0.45d+2)·200/201⌋`, pushback
  `min(2, 0.09d+0.4)`, **powershield** (first 2 frames), **shield drop**
  (15 frames; jump / grab / up-smash cancel it), `State::ShieldDrop`.
- **Helpless** state after air-dodges and aerial up-specials.
- **Autocancel** windows on aerials; L-cancel is `⌊lag/2⌋`.
- **Analog walk** (speed follows the stick tilt), **air-dodge scaled by the
  stick tilt** (a half tilt is a half wavedash), and **doubled traction on
  release** so walk / run stops are precise while wavedash slides keep their
  length.
- **Late hits** (`MoveData::late(frames, scale)`) and per-frame hitbox lookup;
  dash attack carries 85 % of run speed; per-character **dash-dance window**
  (`Character::dash_frames`).
- **Swept hitboxes vs hurt capsules** (`math::segment_distance`) — no
  tunnelling; crouch / knockdown / shield shrink the capsule. Training view
  draws the capsule.
- **Procedural sound** (`render/audio.rs`): tap / thud / crack hits by
  damage, shield, powershield, KO, landing, jump, swing, tech, dash — all
  synthesised at start-up, no audio files. Options → SFX VOLUME.
- **Gamepad rumble** on hits (victim strong, attacker weak) via gilrs force
  feedback. Options → RUMBLE.
- **Impact presentation**: hit-frame flash, victim rattle during hitlag,
  impact-line fan along the launch, powershield ring, dust on dash / land /
  wavedash, camera shake that scales with knockback (jabs no longer shake).
- Simulation events (`Fx { born, who, dir }`; land / jump / swing / dash /
  tech / wavedash) so audio, rumble and VFX are pure functions of the sim
  state and rollback-safe (de-duplicated by `(born, kind)`).
- `tests/feel.rs` (15 tests) pinning every rule above; 68 tests total.
- Animation-viewer clips for HELPLESS and SHIELD DROP; poses in `anim.rs`.

### Changed
- **Scale**: `REF_UNIT = 2.2` world units per reference unit. Every ground,
  air, gravity, fall and jump speed in `roster.rs` was rescaled (they were
  ~2.3× too slow for the world); knockback launch `0.03 × 2.2` per unit,
  uniform decay `0.051 × 2.2` with a separate capped gravity.
- Air-dodge 49 frames (intangible 4–29), roll 31 (4–19, 60 u), spot-dodge
  22 (2–15); shield decay 0.28 / regen 0.07; Boulder weight 118; Kestrel
  jump-squat 3.
- Every move table rebuilt as startup / active / late / endlag with
  class-shaped timings (e.g. Kestrel jab frame 1–2 / 17, nair 3–6 late to
  30 / 41, landing 15) and reference-class landing lag (15–20).
- Camera: max distance 840; stage blast zones widened to ~3.1 platform
  half-widths (Lattice ±486 / 434 / −236, Meridian ±534, Tidegate −440 /
  470).
- `[profile.release] panic = "unwind"` so an audio backend that fails to open
  leaves the game running silent instead of aborting; `macroquad` `audio`
  feature enabled.

### Fixed
- Fast hitboxes could pass through a hurtbox between frames.
- The scripted demo (attract mode / GIF tool) re-timed for the new speeds
  (shorter wavedash into poking range; the forward smash still KOs off the
  side).
- L-cancel rounding (`floor`), autocancel landings.

## [0.4.0] — Fase 3 (in progress): 3D models, stages and art pipeline

### Added
- **3D renderer** (`render/scene3d.rs`): the match is drawn in 3D with a
  tournament-style camera (frames both fighters, zooms with the spread, smooth,
  shake on hits), CPU toon lighting, inverted-hull outlines, soft blob shadows,
  billboard hit / shield / blast / dust effects, strike trails, per-stage sky.
  The classic 2D view stays available (Options → RENDERER, `render_3d` in the
  settings file) and remains what the headless GIF tool uses.
- **Original 3D fighters** (`model/characters.rs`): Kestrel, Boulder and Viper
  as segmented low-poly models on a shared humanoid skeleton with signature
  extras (crest, scarf and tail feathers; orbiting stones and a glowing core;
  hood and a five-segment tail). **Six palettes each** (`model/palettes.rs`),
  shared with the HUD and menus.
- **Procedural animation** (`model/anim.rs`) driven by the simulation's own
  frame data: stance/breathing, walk and run cycles, crouch, jump-squat squash
  and take-off stretch, rising/falling/fast-fall, shield, rolls, spot-dodge,
  air-dodge, hitstun flinch and tumble, knockdown, ledge hang, grabs/throws;
  every attack is a wind-up → strike → recover timeline with the striking limb
  aimed at the move's hitbox. Per-character secondary motion for the extras.
  Turns are eased over a few frames instead of mirroring.
- **3D stages** (`model/stage3d.rs`) built from the sim's platform data plus
  per-stage art direction (`Look`): **The Lattice** redesigned as a violet
  void of hexagonal pylons and orbital rings, **Meridian** a desert arena at
  dawn with monoliths and a low sun, **Tidegate** a basalt sea gate with a
  broken arch under the moon. Procedural tiled surface textures (panels,
  sandstone strata, cracked basalt), fog on distant geometry, glow strips.
- **Select screen** with large rotating 3D model previews on pedestals, stat
  bars, palette names, and an orbiting 3D stage preview; 3D previews in the
  online lobby too.
- **Blender pipeline** (`model/gltf_io.rs`, `model/assets.rs`,
  `docs/ART_PIPELINE.md`): `.glb` import of segmented rigs (node names = bones,
  material names = palette slots, rest rotation/scale baked, auto height fit)
  and stage dressing; `.glb` export of any in-engine rig
  (`overframe --export-glb kestrel k.glb`) as the artist's starting point.
  Drop `assets/characters/<name>.glb` or `assets/stages/<name>.glb` next to
  the game (or `OVERFRAME_ASSETS`) and it is used with the same animation.
  Export → import round-trips in tests.
- **Animation viewer** (`overframe --anim viper[:clip[:frame]]`): every state
  and move on a pedestal with hitboxes, palette/fighter cycling, pause, orbit.
- **Attract mode**: the main menu plays the demo match in 3D behind the items;
  WATCH DEMO shows it full screen.
- CLI: `--versus p1,p2,stage[,pal1,pal2]`, `--demo [spec]`,
  `--screenshot file.png --at N`, `--record dir from count`.
- `model` unit tests (18) covering primitives, rig maths, animation aims,
  character/stage models, camera and the glTF round trip.

### Changed
- `PALETTES` is 6 (was 4); `viz::fighter_color` reads the shared palettes.
- `SceneOpts` gained `hud` and `hitboxes`.
- `gui` feature now includes `gltf`; new `gltf` feature (crate `gltf`, no
  image decoding).

### Known limitations
- Models are procedural placeholders in an original style; the pipeline is
  ready for artist-made replacements. No skinning by design (see
  `docs/ART_PIPELINE.md`).
- Stage dressing from `.glb` is drawn with the stage's palette slots only.

## [0.3.0] — Fase 3 (in progress): roster, stages, online lobbies

### Added
- **Roster of 3 characters** across the classic archetypes, all original:
  **Kestrel** (fast-faller, *Momentum*), **Boulder** (heavyweight, *Bulwark*:
  super armour during smash startup), **Viper** (lightweight, *Skyline*: two air
  jumps, best air control). Characters are pure data (`sim/roster.rs` +
  per-character move tables in `sim/attacks.rs`).
- **3 stages** with distinct layouts and colour themes: **The Lattice**
  (floating platforms), **Meridian** (flat), **Tidegate** (asymmetric).
- **Character/palette select** and **stage select** for local Versus and
  Training, with on-screen character cards (archetype, trait, palette swatches)
  and stage thumbnails.
- **Match rules**: stock count and an optional **time limit** (timed matches end
  on the clock with a most-stocks / lowest-percent tiebreak).
- **Online lobbies**: after connecting, both players sit in a lobby to pick
  character + palette, **chat**, and **ready up**; the host sets the stage and
  rules and starts. Selections travel in a new versioned lobby protocol and
  build an identical `GameState` on both sides.
- **LAN room browser**: hosts broadcast an announcement; the Join screen lists
  discovered rooms automatically (plus manual code / `ip:port` entry).
- **Room passwords** (FNV-hashed in the handshake; wrong password rejected).
- **Typed identity** (`identity::Identity`): `Local` today, `Steam(SteamID64)`
  ready for the Steam build; the ban list now keys on it (`local:` / `steam:`).
- **Platform abstraction** (`netcode/platform.rs`): the seam Steam plugs into,
  with the LAN backend shipped and the Steam backend specified.
- Renderer: per-character palettes and per-stage themes; HUD shows character
  names; a match clock for timed games.
- Docs: `docs/STEAM.md` (Steamworks integration checklist), `docs/TEST_PLAN.md`
  (automated + manual), and DESIGN sections on the content model, the 3D asset
  pipeline plan and the Steam integration.
- Tests: `tests/content.rs` (roster completeness, distinct archetypes, stage
  validity, timed-match end). `tests/netcode_flow.rs` now drives the full lobby
  (pick characters, agree stage, ready, start) before the rollback match, and
  adds a wrong-password rejection test.

### Changed
- `MatchConfig` carries stage, per-slot character + palette, stocks and time.
- `netcode::NetMatch` gained a lobby phase (Handshaking → Lobby → Syncing →
  Running); `host`/`join` now take the rules and a password.
- `Identity` moved to a top-level module so non-netcode builds compile.
- Menu restructured: Versus/Training go through a setup screen; "Watch Demo"
  removed from the menu (the demo remains for the GIF tool and tests).

### Known limitations
- Roster is 3 of the planned 8-10; art is 2D placeholder pending original 3D
  models (pipeline documented).
- Steam integration is specified and seam-ready but not implemented; online is
  direct-UDP + LAN discovery today (internet still needs a forwarded port).

## [0.2.0] — Fase 2: online play + gamepads

### Added
- **Online play (P2P rollback, GGRS).** *Online → Host / Join* in the menu.
  The host shows a **room code** (`XXXXX-XXXXX`, an encoded `ip:port`); the
  guest types it (or a raw `ip:port`). Works on LAN out of the box and over
  the internet with a forwarded UDP port (default 7777, configurable).
- **Handshake protocol** (`netcode::handshake`) that runs on the *same* UDP
  socket GGRS then uses: version check, player ids, match seed, ban check.
  Mismatched builds are refused instead of desyncing.
- **Custom GGRS socket** (`netcode::socket::OfSocket`) wire-compatible with
  GGRS's own, plus handshake framing — one port, one NAT mapping.
- **Match state machine** (`netcode::session::NetMatch`): handshaking → syncing
  → running → ended; GGRS frame pacing (`WaitRecommendation` + `frames_ahead`
  guard), 8-frame prediction window, desync detection on (30-frame interval).
- **In-match network HUD:** ping, rollback frames on the last tick, frames
  ahead, input delay, peer name, room code (host), "WAITING FOR PEER", and a
  red **DESYNC DETECTED** banner if checksums ever disagree.
- **Gamepad support** via `gilrs` (XInput on Windows, evdev on Linux, IOKit on
  macOS). Classic layout by default (A attack, B special, X/Y jump, triggers/LB
  shield, RB grab; left stick / right stick = stick / C-stick). First pad is
  P1, second is P2; keyboard and pad are merged per player so either works.
- **Options screen:** input delay, host port, per-pad rebinding (select →
  Enter → press button), reset to defaults. Persisted to a settings file.
- **Settings file** (`config.rs`): `player_id`, `host_port`, `input_delay`,
  `name`, pad bindings. Human-editable `key = value`; path is per-platform
  (`%APPDATA%\overframe`, `~/Library/Application Support/overframe`,
  `~/.config/overframe`) and overridable with `OVERFRAME_CONFIG_DIR`.
- **Player identity:** random 64-bit id generated on first run, exchanged in
  the handshake — the anchor for the ban system.
- **Ban list data format** (`netcode::banlist`): `bans.txt` with `issuer`,
  `ban <id> [until <unix>] [reason …]`, `sig`. Hosts refuse listed ids with
  the reason. Signature verification is a documented Fase 3 stub; the full
  server-less design is in `docs/DESIGN.md`.
- **CLI flags:** `--host [port]`, `--join <code|ip:port>`, `--help`.
- **Tests:** `tests/netcode_flow.rs` (room codes, handshake, ban list,
  settings, a real loopback host/guest match with induced rollbacks ending on
  identical states, ban rejection) and `tests/gamepad_map.rs` (mapping,
  deadzone, triggers, rebinding, persistence, keyboard/pad merge).
- `viz`: HUD marks the locally controlled fighter with **YOU** online; more
  bitmap-font glyphs (`= ' , _ ? * < >`).

### Changed
- `netcode.rs` became the `netcode/` module (`mod.rs`, `session.rs`,
  `socket.rs`, `handshake.rs`, `roomcode.rs`, `banlist.rs`).
- `render/` split into `mod.rs` (screens, loop) and `input.rs` (keyboard +
  gilrs hub).
- Feature flags: `gui` now implies `netcode` and pulls `gilrs`; `netcode`
  pulls `ggrs`, `bincode`, `serde`, `bytemuck`.
- Linux build dependency added: `libudev-dev` (for gilrs). CI updated.

### Known limitations
- No NAT traversal: internet play needs the host to forward the UDP port
  (UPnP is on the roadmap).
- Two players only online (the engine and GGRS support more; handshake/UI
  assume two).
- `f32` simulation: bit-exact for same-architecture peers; cross-architecture
  fixed-point hardening pending.
- Gamepads were validated through the pure mapping layer and gilrs
  initialisation; a physical Xbox / Switch Pro pass on Windows is the first
  thing to do on real hardware.

## [0.1.0] — Fase 1: prototype

- Deterministic 60 Hz simulation: full movement tech (dash-dance, wavedash,
  fast-fall, short/full hop, L-cancel), shields, rolls, dodges, ledges, tech,
  knockback + hitstun + DI, grabs/throws, projectiles, stocks, blast zones.
- macroquad GUI: local 2P versus, training mode with hit/hurtbox display,
  scripted demo. Headless GIF renderer. GGRS `SyncTest` determinism test.
- Original fighter **Kestrel** and stage **The Lattice**. MIT license, CI,
  release workflow producing native Windows/macOS/Linux builds.
