# OVERFRAME test plan

Two layers: **automated** (run in CI on every push) and **manual** (a human
checklist before tagging a release, because input devices, real networks and
"feel" can't be unit-tested).

## Automated (`cargo test`)

Run the fast, dependency-free suite:

```bash
cargo test --no-default-features --features netcode
```

| Suite | What it proves |
|---|---|
| `tests/mechanics.rs` (9) | Movement *feel*: full hop > short hop, fast-fall lands sooner, dash > walk, angled air-dodge wavedashes, L-cancel cuts landing lag, knockback grows with percent, sim is deterministic. |
| `tests/determinism.rs` (1) | GGRS `SyncTest`: save→load→re-simulate reproduces identical checksums over 400 frames of busy input — the engine is rollback-safe. |
| `tests/content.rs` (7) | Every character has a complete, sane moveset; archetypes are actually distinct (weight/air/armour/jumps ordering; Boulder fsmash > Kestrel, Viper jab faster than Boulder); stages well-formed; timed match ends on the clock. |
| `tests/netcode_flow.rs` (8) | Room codes + handshake + ban list round-trip; **a real host/guest pair over loopback UDP connects, runs the lobby, picks different characters, agrees a stage, starts, plays 300 frames with induced rollbacks, and ends byte-identical**; banned id and wrong password are rejected. |
| `tests/gamepad_map.rs` (7) | Gamepad mapping, deadzone, analog triggers, rebinding, persistence, keyboard/pad merge — no hardware needed. |
| `src/model/*` unit tests (18) | 3D layer without a GPU: primitives are closed and outward-facing; bone maths composes and mirrors correctly; strike poses aim at the hitbox (up-tilt foot above hips, down-tilt below and forward); gait is periodic and alternates sides; every fighter model builds, has ≥ `PALETTES` palettes and fits its collision height; stage models sit exactly on the sim's platforms; the camera settles and frames both fighters; `.glb` export → import round-trips every fighter (bones, parts, slots, posed joints), rest rotations are baked, height fitting works. |

CI also runs `cargo fmt --all -- --check` and
`cargo clippy … -- -D warnings` on the sim, GUI and headless feature sets, and
builds the GUI on Windows/macOS/Linux.

## Manual — visuals (3D)

- [ ] `overframe --anim kestrel` (then boulder, viper): step every clip with
      ←/→; no part detaches, limbs point where the red hitbox sphere is on
      active frames, palettes (P) all read well, outlines are clean.
- [ ] Each stage in `--versus`: platforms you stand on match the drawn tops
      (no floating / sinking), ledges glow at the grab points, camera keeps
      both fighters in view at max spread and never clips into the slab.
- [ ] Turning is a short turn (not a pop); hit flash, shield bubble, KO blast
      and dust read at a glance; 60 FPS on the target machine (Options →
      RENDERER 2D CLASSIC as the fallback).
- [ ] Drop a `.glb` into `assets/characters/` → start prints `assets: loaded…`
      and the model animates like the placeholder.

## Manual — matches & feel

- [ ] Each of the 3 fighters: land every normal, tilt, smash, aerial, special,
      grab and all four throws; confirm none softlock and all deal damage.
- [ ] Boulder super armour: get hit by a jab mid-smash-startup → armour absorbs
      it (no launch); get hit by a smash → it breaks through.
- [ ] Viper double jump and Kestrel wavedash length are noticeably different.
- [ ] Ledge, tech, roll, spot-dodge, air-dodge all work on each stage.
- [ ] KO on every blast zone of every stage; timed match ends and shows a winner
      / draw correctly.

## Manual — input

- [ ] Keyboard P1 and P2 simultaneously (2 players, one keyboard).
- [ ] Xbox pad on Windows (XInput): all default bindings; rebind one and confirm
      it persists after restart.
- [ ] Switch Pro pad (via Steam or vendor driver); note any inverted axis.
- [ ] Plug a pad in *after* launch → picked up live; unplug mid-menu → no crash.
- [ ] Keyboard + pad on the same player both drive the fighter (merge).

## Manual — online (LAN)

- [ ] Host on PC A, PC B sees the room in the browser automatically; join.
- [ ] Lobby: both change character/palette and see the other's choice update;
      chat both ways; both ready → host starts; correct characters/stage/stocks
      load on both PCs.
- [ ] Play a full set; watch the HUD: ping is the real RTT, rollback frames stay
      low (single digits on LAN), no DESYNC banner.
- [ ] Room password: wrong password is rejected with a clear message.
- [ ] Host bans the guest's id (add to `bans.txt`) → guest can't rejoin.
- [ ] One side quits mid-match → the other returns to menu with "PEER
      DISCONNECTED", no crash.

## Manual — online (internet)

- [ ] Host forwards the UDP port, shares public `ip:port`; remote guest joins.
- [ ] At ~40–80 ms RTT the game stays at 60 FPS; raise input delay to 3–4 and
      confirm rollbacks shrink.
- [ ] Deliberately throttle one side (e.g. `clumsy`/`tc netem`, 5% loss, 100 ms):
      the match stays in sync (no desync), just rollbacks more.

## Manual — multi-instance smoke (no second PC)

```bash
OVERFRAME_CONFIG_DIR=/tmp/a overframe --host 7801
OVERFRAME_CONFIG_DIR=/tmp/b overframe --join 127.0.0.1:7801
```

Both reach the lobby, pick, ready, start, and run in sync. (This is exactly what
`tests/netcode_flow.rs` automates, but the manual run also exercises the GUI.)

## Regression gates before a release tag

1. `cargo test` green on the CI matrix.
2. `cargo clippy -- -D warnings` and `cargo fmt --check` clean.
3. The LAN online checklist passes on two physical machines.
4. At least one internet match at >40 ms RTT with no desync.
