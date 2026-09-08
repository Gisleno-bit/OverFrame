# Changelog

All notable changes to OVERFRAME. Format follows *Keep a Changelog*; versions
follow SemVer once `v1.0.0` ships.

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
