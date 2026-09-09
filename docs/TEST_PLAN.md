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
| `tests/feel.rs` (15) | The **game-feel contract** (`docs/GAME_FEEL.md`): hitlag = `⌊d/3+3⌋` on both fighters and nobody moves while frozen; a weak grounded hit slides along the floor; strong hits launch at `0.03×kb` (scaled) with uniform decay and capped gravity; crouch cancel takes a third off; SDI pulses move the victim during hitlag; shieldstun and pushback follow the block formulas; powershield in the first 2 frames; 15-frame shield drop with jump/grab/up-smash cancels; air-dodge ends helpless but still wavedashes; landing lag 4 / full / `⌊lag/2⌋` / autocancel; late hits are weaker and moves end at IASA; dash attack carries momentum, jab plants; run crosses 240 u in ≤ 60 frames; dash-dance window is per character; swept hitboxes don't tunnel. |
| `tests/grab_ledge.rs` (11) | Standing grab hits on frame 7 of 30, dash grab on 12 of 40 and slides; hold = `⌊76 + 1.6p⌋`, mashing takes 6 per input, release lags both and shoves the victim; pummel on a cooldown, stick throws; ledge catch 7 frames / 37 intangible kept on drop; getup 33 / 59, roll 49 / 79, attack hitbox exactly on 24–26 / 42–44; committed ledge jump; hang limit 660 / 480; helpless fighters grab ledges. |
| `tests/neutral.rs` (11) | Dash-dance reversal is instant but a run reversal is a 20-frame `RunTurn` (jump cancels it); JC grab / up-smash from the squat; platform drop by a down flick only; 20-frame tech window with lockout, tech in place 26 / roll 40 (70 u); knockdown bounce 18 then stand 30 / roll 35 / getup attack 49 with hits exactly on 15–17 front and 21–23 behind; equal tilts clank into a rebound, jab loses to smash; staling 8 → 7.3 → 6.6 and a KO clears it; meteor cancel after 8 frames; flick = smash / held = tilt, full charge ×1.367, C-stick uncharged; an occupied ledge can't be grabbed; a waveland that slides off a platform edge-cancels. |
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

- [ ] **Hitstop reads**: a jab is a short tick, a smash a clear freeze (8
      frames at 15 %) with a flash, a rattle on the victim, impact lines along
      the launch, a camera shake and rumble on both pads (victim stronger).
      Jabs must *not* shake the camera.
- [ ] **Weak hits shove, strong hits launch**: jab at 0 % slides the victim
      along the floor; forward-smash at 40 %+ tumbles. Crouching into a dash
      attack visibly shortens the knockback and the freeze.
- [ ] **DI during the freeze**: hold up-and-in on a forward-smash at 100 %
      from the edge and survive; hold nothing and die. Tapping the stick in a
      multi-hit nudges you out.
- [ ] **Shield**: raise shield as a smash lands → powershield ping and no
      push; hold shield, release → 15 frames before you can attack, but a
      jump / grab / up-smash comes out immediately.
- [ ] **Movement**: dash-dance feels different on Kestrel (11), Boulder (13)
      and Viper (7); a wavedash slides and an air-dodge that doesn't land
      leaves you helpless; L-cancelled nair → up-tilt links at 20–60 %,
      un-cancelled it doesn't; landing outside the hit window (autocancel) is
      as fast as an empty landing.
- [ ] **Sound**: tap / thud / crack scale with damage; shield tock, KO boom,
      landing thump, jump and swing whooshes; Options → SFX VOLUME 0 silences
      everything and RUMBLE OFF stops the pads. No audio device → the game
      still runs (silent).
- [ ] **Tech chase**: forward-smash at 40 % → the victim techs in place,
      tech-rolls left/right, or misses and lies down; from the floor the
      getup attack visibly kicks front then back; mashing shield does *not*
      tech.
- [ ] **Inputs**: a slow forward tilt of the stick + A is a forward tilt,
      a flick + A is a forward smash; holding A makes the smash tremble and
      hit harder; C-stick smashes never wait. Two fighters at one ledge:
      the second falls past it.
- [ ] **Neutral texture**: dash-dancing is free, but a reversal from a full
      run brakes; two forward tilts meeting clank with a spark and a tink;
      the fourth jab in a row reads a lower number; the percent counter pops
      on every hit.
- [ ] **Grab game**: grab → pummel twice → up-throw reads as three
      distinct beats; a 0 % victim mashing out breaks free in about a
      second, at 100 % it can't; a dash grab visibly slides.
- [ ] **Ledge**: hang outside the edge facing in; nothing responds for the
      first frames; drop → immediate air-dodge onto the stage while still
      flashing (ledgedash); at 100 %+ every option is visibly slower; the
      ledge attack's hitbox appears where the kick is.
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
