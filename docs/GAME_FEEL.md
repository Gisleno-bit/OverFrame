# Game feel: how OVERFRAME hits, and why

This document is the combat-design record behind the 0.5.0 "game feel"
release. It explains what makes the classic competitive platform fighter feel
*crisp*, diagnoses what was wrong with OVERFRAME 0.4.0, lists every number we
changed with its reference, and says how to measure that the change worked.

**Clean-room line.** Everything here comes from public community frame-data
documentation (SSBWiki mechanics pages; FightCore publishes the same community
data — it was unreachable from the build environment, so SSBWiki is cited).
We reproduce *how mechanics behave* — timings and formulas, which are rules,
not assets — and keep every per-move number, name, model, sound and animation
original. Nothing is read from another game's files. See `docs/LEGAL.md` and
`CONTRIBUTING.md`.

Unit convention: OVERFRAME fighters are ~30 world units tall; the reference
cast is ~13.6 units tall. So **1 reference unit = `REF_UNIT` = 2.2 world
units**, and any reference *speed* converts as `× 2.2`. Frame counts convert
1:1 (both simulations run at 60 Hz).

---

## 1. What the reference actually does (summary of the frame data)

The "feel" people describe — weighty hits, snappy movement, "you can always
do something" — is a handful of universal rules plus a movement model. None of
it is in the art.

### 1.1 Impact: the freeze, the launch, the stun

| Rule | Reference behaviour |
|---|---|
| **Hitlag** (freeze frames, both fighters) | `⌊⌊⌊damage/3 + 3⌋ × e⌋ × c⌋`, capped at 20. `e = 1.5` for electric moves, `c = 2/3` when the victim crouch-cancels. A 3% jab freezes 4 frames; a 15% smash 8; a 21% hit 10. |
| **Knockback** | `((p/10 + p·d/20) × 200/(w+100) × 1.4 + 18) × kbg/100 + bkb`, where `p` = victim percent *after* the hit, `d` = damage, `w` = weight, `kbg`/`bkb` per hit. |
| **Launch speed** | `0.03 × knockback` units/frame. |
| **Knockback decay** | The launch vector loses `0.051` units/frame of *magnitude* (direction preserved); gravity is applied separately and its fall speed is capped by the character's `max_fall`. |
| **Hitstun** | `⌊0.4 × knockback⌋` frames. Above **80** knockback the victim *tumbles* (can tech, can be footstooled…); below it, a grounded victim *flinches and slides along the ground* instead of leaving it. |
| **Crouch cancel** | Crouching when hit: knockback ×2/3 and hitlag ×2/3 — punishes weak pokes. |
| **Sakurai angle (361)** | Below 32.1 knockback on a grounded victim → 0° (a shove along the floor); otherwise 44°. Most jabs, tilts and nairs use it, which is why weak hits *push* and strong ones *launch*. |
| **DI** | Stick perpendicular to the launch rotates the angle up to ≈18°, read on the **last hitlag frame** — players react *during* the freeze. |
| **SDI / ASDI** | Each stick *pulse* during hitlag moves the victim 6 units; the held stick on the last freeze frame moves 3 (ASDI), which can put a low-launched victim back on the floor and end the combo. |

### 1.2 Defence

| Rule | Reference behaviour |
|---|---|
| **Shieldstun** | `⌊(0.45·d + 2) × 200/201⌋` frames. Pushback on the shielder `min(2, 0.09·d + 0.4)` units/frame, decaying. |
| **Shield** | 60 HP, drains 0.28/frame held, regenerates 0.07/frame down, takes 0.7× damage. Dropping it costs **15 frames** (jump, grab and up-smash cancel the drop; rolls and spot-dodges skip it). |
| **Powershield** | Blocking within the first **2** frames of raising the shield: no damage, no stun, no push. |
| **Air-dodge** | **49** frames total, intangible **4–29**, then **helpless** until landing — which is what makes wavedashing a *technique* (you commit) rather than a free escape. |
| **Roll** | 31 frames, intangible 4–19. **Spot-dodge** 22, intangible 2–15. |

### 1.3 Movement (the part people call "the feel")

| Rule | Reference behaviour |
|---|---|
| **Landing lag** | 4 frames for an empty landing. Every aerial has its own landing lag; **L-cancel** (shield within 7 frames before landing) halves it (`⌊lag/2⌋`); landing *outside* the aerial's active window (**autocancel**) costs the normal 4. |
| **Dash-dance** | You can reverse direction for the whole *initial dash* (per character: 7–15 frames); after that you are running and a reversal is a slow turn. |
| **IASA** | Moves can be interrupted before their animation ends; effectively every move is `startup + active (+ late) + endlag` and the endlag is the whole recovery. |
| **Jump-squat** | 3–6 frames of squat before leaving the ground; short hop if the button is released before take-off. |
| **Fast-fall** | Tapping down past the apex jumps straight to a much higher fall speed (≈1.2–1.4× max fall). |
| **Movement classes** (reference units / frame) | Fast-faller: walk 1.6, dash 1.9, run 2.2, air 0.83, gravity 0.23, fall 2.8 / fast-fall 3.4, squat 3, weight 75, dash-dance 11. Heavyweight: walk ≈0.7, run 1.5–1.6, gravity ≈0.12–0.13, fall ≈2.1/2.9–3.0, squat 5–8, weight 110–120, dash-dance ≈13. Lightweight/acrobat: walk 1.3, run 1.8–1.9, air 0.9–1.0, gravity 0.13, fall 2.1–2.2/3.0, squat 3, weight 85–90, dash-dance 7. |
| **Stage proportions** | Standard three-platform stage: main platform half-width ≈71, side blast ±224, top 200, bottom −109, i.e. about **3.1 platform half-widths to the side blast zone**. |

Everything in that list is reproduced in OVERFRAME. Nothing else was needed
to close the gap.

---

## 2. Diagnosis: why 0.4.0 felt bad

Measured with the same harness as §6 against the pre-0.5.0 build.

1. **Everything moved 2.3× too slowly for the size of the world.** Fighters
   were 30 units tall on a 310-unit stage — the same *proportions* as the
   reference — but run speed was 2.05 u/f (reference-equivalent 0.93, versus
   2.2). Crossing the stage took ~150 frames instead of ~70. Falls, jumps and
   air-dodges were scaled the same way, so short hops floated and fast-falls
   barely mattered. This one error made the game feel underwater even though
   every mechanic was present.
2. **Knockback was launch-only and decayed like friction.** Launch scale
   0.043 (should be 0.066 at our scale) with a weak 0.038/frame decay: strong
   hits went too slow and *kept going* for too long, weak hits floated instead
   of shoving. There was no grounded flinch: a 3% jab at 0% lifted the victim
   off the floor.
3. **Hitlag was flat** (`3 + 0.30·damage`, no floor, no cap, no electric,
   no crouch cancel) so a smash and a jab froze almost the same, and nothing
   happened *during* the freeze — DI was read on the hit frame, before the
   player could react, and there was no SDI/ASDI at all.
4. **Shieldstun was a multiplier** (`0.35·damage`) instead of the additive
   formula, so jabs on shield gave the attacker a frame *disadvantage* that
   never happens in the reference; there was no powershield and no shield-drop
   lag, so shield was a free option-select.
5. **Air-dodge was 28 frames and not helpless**, so wavedashing was free and
   air-dodging out of every combo was the correct answer.
6. **Aerial landing lag was too short (8–12 frames on most aerials, versus
   15–22 in the reference class) and there was no autocancel**, so
   L-cancelling saved 4–6 frames instead of 8–11 and every aerial was safe on
   landing — the risk/reward that makes aerials *decisions* was missing.
7. **Move timings were "safe" rather than reference-shaped**: jab hit on
   frame 3 (reference 2), forward-tilt frame 6 (5), dash attack frame 8 (4),
   up-smash frame 10 (7), nair frame 4 with only 6 active frames and no late
   hit, no IASA-style early autocancel windows. Dash attack planted instead
   of carrying momentum.
8. **Hits were silent and weightless in presentation**: no sound at all, no
   rumble, a camera shake that did not scale with the hit, no hit-frame
   flash, and hitboxes were tested as a *point sample* each frame (fast
   moves could tunnel through a hurtbox — a hit you *saw* but didn't get).

---

## 3. Comparison table: reference → 0.4.0 → 0.5.0

Reference values in reference units; OVERFRAME values in world units
(`= ref × 2.2`) unless the row is a frame count or formula.

### 3.1 Universal mechanics

| Mechanic | Reference | 0.4.0 | 0.5.0 | Where |
|---|---|---|---|---|
| Hitlag | `⌊d/3+3⌋`, ×1.5 electric, ×2/3 CC, cap 20 | `3 + 0.3d` | **reference formula** | `knockback::hitlag` |
| Hitstun | `0.4 × kb` | `0.4 × kb` | same | `knockback::hitstun` |
| Launch speed / kb unit | 0.03 | 0.043 (= 0.0195 ref) | **0.066 (= 0.03 ref)** | `LAUNCH_SPEED_SCALE` |
| KB decay | 0.051/frame, direction preserved | friction-like 0.038 | **0.1122 (= 0.051 ref)**, magnitude decay + separate capped gravity | `KB_DECAY`, `Fighter::tick_hitstun` |
| Tumble threshold | 80 kb | 80 | 80, and **below it grounded victims slide** | `TUMBLE_THRESHOLD`, `ground_stun` |
| Sakurai angle | 0° if grounded & kb ≤ 32.1, else 44° | not implemented | **implemented** (`361` in tables) | `knockback::resolve_angle` |
| Crouch cancel | kb ×2/3, hitlag ×2/3 | none | **implemented** | `GameState::apply_hit` |
| DI timing | last hitlag frame | hit frame | **last hitlag frame** | `pending_launch` |
| SDI / ASDI | 6 / 3 units per pulse | none | **13.2 / 6.6 world units** | `SDI_STEP`, `ASDI_STEP` |
| Shieldstun | `⌊(0.45d+2)·200/201⌋` | `0.35d` | **reference formula** | `knockback::shieldstun` |
| Shield push | `min(2, 0.09d+0.4)` | fixed nudge | **formula × 2.2** | `knockback::shield_push` |
| Shield HP / decay / regen / dmg | 60 / 0.28 / 0.07 / 0.7× | 60 / 0.24 / 0.08 / 0.7× | **60 / 0.28 / 0.07 / 0.7×** | `constants.rs` |
| Powershield | first 2 frames | none | **2 frames** | `POWERSHIELD_WINDOW` |
| Shield drop | 15 frames | 0 | **15** (jump/grab/usmash cancel) | `SHIELD_DROP`, `State::ShieldDrop` |
| Air-dodge | 49 total, 4–29 intangible, helpless | 28, 2–19, actionable | **49, 4–29, helpless** | `AIRDODGE_*`, `State::Helpless` |
| Roll | 31, 4–19 | 32, 4–18, 46 u | **31, 4–19, 60 u** | `ROLL_*` |
| Spot-dodge | 22, 2–15 | 22, 2–14 | **22, 2–15** | `SPOTDODGE_*` |
| Empty landing lag | 4 | 4 | 4 | `LANDING_LAG_NORMAL` |
| L-cancel | `⌊lag/2⌋`, 7-frame window | `lag/2` | **`⌊lag/2⌋`**, window 7 | `on_land` |
| Autocancel | land outside the active window → 4 | none | **implemented** (early: before startup; late: after 75 % of the endlag) | `MoveData::autocancels` |
| Dash-dance window | per character 7–15 | 11 for everyone | **per character** (11 / 13 / 7) | `Character::dash_frames` |
| Air-dodge vector | `stick × speed` (tilt sets length) | normalised (full length always) | **`stick × speed`** — a half tilt is a half wavedash | `Fighter::tick` (shield press in air) |
| Walk speed | scales with stick tilt | `walk_max` at any tilt | **analog**: 30 % at the deadzone → 100 % just under the dash threshold | `ground_control` |
| Stopping | traction, doubled above walk speed | traction | **doubled traction on release** (walk stop 12 u / 9 f, run stop 28 u / 14 f for Kestrel); wavedash slides use plain traction until they drop below walk speed | `ground_control` |
| Dash attack momentum | carries run speed | plants | **`vel.x = 0.85 × run_max`** on start | `Fighter::start_attack` |
| Hitbox collision | swept, per frame | point sample vs circle | **swept segment vs hurt capsule** | `math::segment_distance`, `state.rs` |
| Hurtbox | capsule, shrinks when crouching | circle | **capsule** (0.62 h crouched, 0.4 h knocked down, 0.85 h in shield) | `Fighter::hurt_segment` |

### 3.2 Movement per class (world units / frame)

| Attribute | Fast-faller ref | Kestrel 0.4.0 → **0.5.0** | Heavy ref | Boulder 0.4.0 → **0.5.0** | Light ref | Viper 0.4.0 → **0.5.0** |
|---|---|---|---|---|---|---|
| Walk | 1.6 (3.52) | 1.55 → **3.45** | 0.75 (1.65) | 1.05 → **1.65** | 1.32 (2.9) | 1.75 → **2.90** |
| Initial dash | 1.9 (4.18) | 2.30 → **4.10** | 1.45 (3.2) | 1.75 → **3.20** | 1.77 (3.9) | 2.45 → **3.90** |
| Run | 2.2 (4.84) | 2.05 → **4.70** | 1.6 (3.5) | 1.60 → **3.50** | 1.9 (4.2) | 2.25 → **4.20** |
| Ground accel | 0.2 (0.44) | 0.22 → **0.44** | — | 0.16 → **0.28** | — | 0.26 → **0.48** |
| Traction (friction) | 0.08 (0.18) | 0.09 → **0.18** | — | 0.11 → **0.20** | — | 0.08 → **0.15** |
| Waveland traction | — | 0.05 → **0.115** | — | 0.07 → **0.17** | — | 0.055 → **0.17** |
| Air speed | 0.83 (1.83) | 1.85 → **1.85** | 0.64 (1.4) | 1.30 → **1.40** | 0.98 (2.15) | 2.15 → **2.15** |
| Air accel | 0.10 (0.22) | 0.115 → **0.22** | — | 0.07 → **0.12** | — | 0.15 → **0.28** |
| Gravity | 0.23 (0.51) | 0.29 → **0.50** | 0.12 (0.26) | 0.24 → **0.26** | 0.136 (0.30) | 0.215 → **0.30** |
| Max fall / fast-fall | 2.8 / 3.4 (6.2 / 7.5) | 3.2 / 4.85 → **6.1 / 7.4** | 2.1 / 2.9 (4.6 / 6.4) | 2.7 / 3.9 → **4.6 / 6.4** | 2.2 / 3.0 (4.8 / 6.6) | 2.6 / 3.8 → **4.8 / 6.6** |
| Jump-squat | 3 | 4 → **3** | 5–6 | 6 → **6** | 3 | 3 → **3** |
| Dash-dance window | 11 | 11 → **11** | 13 | 11 → **13** | 7 | 11 → **7** |
| Air-dodge speed | 3.0 (6.6) | 3.1 → **6.6** | — | 2.6 → **5.9** | — | 3.3 → **6.8** |
| Weight | 75 | 82 → 82 | 110–120 | 122 → **118** | 87–90 | 68 → 68 |

### 3.3 Move timings (Kestrel, the fast-faller; 1-based hit frame)

Per-move numbers are original but shaped like the class. "Ref shape" is the
fast-faller *pattern* from the public data (first hit frame / total).

| Move | Ref shape | 0.4.0 | **0.5.0** | Notes |
|---|---|---|---|---|
| Jab | 2 / 17 | 3 / 13 | **1–2 / 17**, 3 %, 361° | shoves at low % |
| F-tilt | 5 / ~25 | 6 / 23 | **4–6 / 25**, 8 %, 361° | |
| U-tilt | 5 / 23 | 5 / 24 | **4–7 / 23** | |
| D-tilt | 7 / 28 | 5 / 20 | **6–8 / 27** | |
| F-smash | 12 / 40 | 12 / 41 | **11–13 / 40**, 15 % | KO ~110 % from centre, no DI |
| U-smash | 7 / 49, late 10–17 | 10 / 38 | **7–9 / 40**, late +8 @ 0.7 | |
| D-smash | 6 / 46 | 9 / 36 | **6–8 / 39** | |
| Dash attack | 4 / 36 | 8 / 32 | **4–7 / 36**, late +6, carries momentum | |
| Nair | 4 / 42, late 8–31, land 15 | 4 / 24, land 8 | **3–6 / 41**, late +24 @ 0.75, land 15 (L-cancel 7), autocancel 1–2 & 37+ | the combo tool |
| Fair | 9 / 53, land 22 | 7 / 29, land 12 | **7–10 / 38**, land 20 | |
| Bair | 4 / 38, land 20 | 6 / 26, land 10 | **4–7 / 37**, late +12 @ 0.6, land 20 | |
| Uair | 8 / 36, land 18 | 5 / 23, land 9 | **7–10 / 36**, land 18 | |
| Dair | 5 / 41, land 18 | 9 / 36, land 18 | **7–11 / 41**, land 20, 270° spike | |
| Down-special | electric | plain | **electric** (hitlag ×1.5) | |

Boulder and Viper tables were reshaped the same way (`src/sim/attacks.rs`).

### 3.4 Presentation

| Element | 0.4.0 | **0.5.0** |
|---|---|---|
| Hit sound | none | procedural **tap / thud / crack** by damage, shield *tock*, powershield *ping*, KO *boom*, land / jump / swing / tech / dash scuffs (`render/audio.rs`, synthesised at start-up — no audio files) |
| Rumble | none | gilrs force feedback: victim strong 110 ms, attacker weak 70 ms (`InputHub::rumble`), Options → RUMBLE |
| Camera shake | `damage × 0.35 + kb × 0.02` (jabs shook) | `damage × 0.18 + max(0, kb − 70) × 0.06`, capped 12 — smashes shake, jabs don't |
| Hit-frame flash | none | white flash on the hit frame (1.0 above 90 kb, else 0.55) + victim **rattle** during hitlag |
| Impact VFX | one sprite | impact burst + fan of impact lines along the launch direction, powershield ring, dust on dash / land / wavedash |
| Training view | hitboxes | hitboxes **and the hurt capsule** |

---

## 4. Implementation plan (what was done, in order)

Each step is one commit-sized change; all of them are in `100fc76` and
`9afc3a5`.

1. **Establish the scale.** Compare fighter height and main-platform width to
   the reference; fix `REF_UNIT = 2.2`. Every speed in `roster.rs` is now
   written as `reference × 2.2` with the reference in a comment.
2. **Impact formulas** (`sim/knockback.rs`): replace hitlag with the floored,
   capped formula plus electric / crouch-cancel multipliers; add `shieldstun`,
   `shield_push`, `resolve_angle` (Sakurai). Knockback → launch speed at
   `0.03 × 2.2`.
3. **Knockback integration** (`Fighter::tick_hitstun`): decay the launch
   vector's *magnitude* by `KB_DECAY` each frame; accumulate gravity into a
   separate `kb_fall` capped at `max_fall`; below tumble and grounded → slide
   along the floor with traction (`ground_stun`).
4. **The freeze does work** (`Fighter::tick`, hitlag branch): store the launch
   as `pending_launch`; during hitlag read stick *pulses* (SDI) and on the last
   frame the held stick (ASDI) and DI, then launch. ASDI that lands cancels
   the hitstun.
5. **Crouch cancel and grounded flinch** (`GameState::apply_hit`).
6. **Defence timings**: air-dodge 49 / 4–29 → `Helpless`; roll 31 / 4–19;
   spot-dodge 22 / 2–15; shield drop 15 with cancels; powershield window 2;
   shield HP constants.
7. **Landing**: `on_land` applies autocancel (4), L-cancel (`⌊lag/2⌋`) or the
   aerial's landing lag; `MoveData::autocancels(frame)`.
8. **Move tables**: rebuild every move as `startup / active / late(frames,
   scale) / endlag`, 361° on pokes, electric flag, landing lag per aerial,
   dash attack momentum, per-character `dash_frames`.
9. **Ground control**: analog walk speed, air-dodge vector scaled by stick
   tilt (short wavedashes on purpose), doubled traction when the stick is
   released so stops are precise while slides keep their length.
10. **Collision**: hurt *capsules* (`hurt_segment`) and *swept* hitboxes
   (`active_hitbox_prev` → `segment_distance`) so fast moves cannot tunnel.
11. **Feedback as a pure function of the sim**: every event (`Fx { born, who,
    dir }`) is created by `step`; the renderer voices sounds, rumble, flash and
    shake from the fx list, de-duplicated by `(born, kind)` so rollbacks never
    double-fire. No render state feeds back into the sim.
12. **Tests** (`tests/feel.rs`, 15) pin every rule above; the old suites
    (68 tests total) still pass.

---

## 5. Pseudocode

### 5.1 Hit resolution

```text
on_hit(attacker A, victim V, hitbox H):
    if V.state == Crouch and V.grounded: cc = true
    d      = H.damage × (late ? H.late_scale : 1)
    V.percent += d
    kb     = ((V.percent/10 + V.percent·d/20) × 200/(V.weight+100) × 1.4 + 18) × H.kbg/100 + H.bkb
    if cc: kb ×= 2/3
    angle  = H.angle == 361 ? (V.grounded and kb ≤ 32.1 ? 0° : 44°) : H.angle
    lag    = ⌊⌊⌊d/3 + 3⌋ × (H.electric ? 1.5 : 1)⌋ × (cc ? 2/3 : 1)⌋, cap 20
    A.hitlag = lag ;  V.hitlag = lag
    V.hitstun = ⌊0.4 × kb⌋ ;  V.tumble = kb ≥ 80
    if V.grounded and not V.tumble and launch_y(angle, kb) ≤ 0:
        V.ground_stun = true                       # flinch + slide, stays on the floor
    V.pending_launch = (angle, kb)                  # applied at the END of hitlag
    fx.push(Hit{ born: frame, who: V, dir: angle, magnitude: d })
    camera_shake = min(12, d × 0.18 + max(0, kb − 70) × 0.06)
```

### 5.2 During the freeze (SDI / ASDI / DI)

```text
tick_hitlag(V, stick):
    if pulse(stick, V.prev_stick):                  # new direction past the deadzone
        V.pos += stick.normalized × SDI_STEP        # 6 ref units
    if V.hitlag == 1:                               # last frozen frame
        V.pos += stick × ASDI_STEP                  # 3 ref units, may touch the floor
        if V.ground_stun or landed_by_asdi: keep grounded, end tumble
        (angle, kb) = V.pending_launch
        angle += clamp(perp(stick, angle), −1, 1) × 18°      # DI
        V.kb_vel = (cos angle, sin angle) × kb × 0.03 × REF_UNIT
    V.hitlag −= 1
```

### 5.3 Knockback integration

```text
tick_hitstun(V):
    speed = |V.kb_vel|
    if speed > 0: V.kb_vel ×= max(0, speed − KB_DECAY) / speed     # magnitude decay
    if not V.grounded:
        V.kb_fall = min(V.kb_fall + gravity, max_fall)
    V.pos += V.kb_vel + (0, −V.kb_fall)
    if V.ground_stun: V.pos.x += V.kb_vel.x ; V.kb_vel.x → 0 by traction
    if V.hitstun_timer == 0: exit hitstun (tumble → Air/tech, else Stand)
```

### 5.4 Landing

```text
on_land(F):
    if F.state == Attack(aerial m, frame f):
        if m.autocancels(f):    lag = 4
        elif F.lcancel_armed:   lag = ⌊m.landing_lag / 2⌋
        else:                   lag = m.landing_lag
    elif F.state in {Airdodge, Helpless} and sliding: lag = 10 (waveland, traction slide)
    else:                       lag = 4
    F.state = LandLag(lag)

autocancels(m, f): f + 1 < m.startup  or  f ≥ m.startup + m.active + m.late + ⌊m.endlag × 3/4⌋
```

### 5.5 Shield

```text
shield_hit(V, d):
    if V.state == Shield and V.state_frame < 2: powershield → fx Powershield, return
    V.shield_hp −= d × 0.7            (break if ≤ 0)
    V.shieldstun = ⌊(0.45·d + 2) × 200/201⌋
    V.vel.x = away × min(2, 0.09·d + 0.4) × REF_UNIT
    A.vel.x = towards × small nudge
release_shield(V): state = ShieldDrop(15)   # jump / grab / up-smash cancel it
```

### 5.6 Feedback (renderer)

```text
each drawn frame with displayed state S:
    for fx in S.fx where (fx.born, fx.kind) not yet voiced and fx.born + 6 ≥ S.frame:
        Hit:  play hit[d < 9 ? tap : d < 18 ? thud : crack]; rumble(victim, strong); rumble(attacker, weak)
        Shield/Powershield/Blast/Land/Jump/Swing/Tech/Dust: play the matching clip
    victim in hitlag: draw root offset by a 1-unit alternating rattle
    hit frame: white flash; camera offset = shake(S.camera_shake)
```

---

## 6. Test results and how to measure

### 6.1 Automated (`cargo test --no-default-features --features netcode`)

`tests/feel.rs` (15 tests) — all green:

| Test | Asserts |
|---|---|
| `hitlag_freezes_both_fighters_by_the_damage_formula` | 8 % → 5 freeze frames on both; nobody moves while frozen |
| `weak_grounded_hit_slides_along_the_ground` | jab at 0 % keeps the victim on the floor and slides it |
| `strong_hit_launches_with_uniform_decay_and_gravity` | smash launch speed = `kb × 0.066`, decays 0.1122/frame, fall capped |
| `crouch_cancel_takes_a_third_off_the_knockback` | kb ×2/3, hitlag ×2/3 when crouching |
| `smash_di_moves_the_victim_during_hitlag` | stick pulses displace 13.2 u each during the freeze |
| `shieldstun_and_pushback_follow_the_block_formulas` | 8 % on shield → 5 frames stun, push `2.46` u/f |
| `powershield_in_the_first_frames_takes_no_stun` | shield raised ≤ 2 frames before the hit: no damage, no stun |
| `shield_drop_and_out_of_shield_options` | 15-frame drop; jump / grab / up-smash cancel it |
| `air_dodge_ends_helpless_and_wavedash_still_works` | non-landing air-dodge → `Helpless`; angled one still wavedashes |
| `landing_lag_normal_lcancel_and_autocancel` | empty 4; aerial full; L-cancel ⌊/2⌋; autocancel 4 |
| `late_hits_are_weaker_and_moves_end_at_iasa` | late frames deal `late_scale`; move ends at `total()` |
| `dash_attack_carries_momentum_but_jab_plants` | dash attack keeps 85 % of run speed; jab stops |
| `run_speed_crosses_the_stage_at_reference_pace` | Kestrel covers 240 u in ≤ 60 frames |
| `dash_dance_window_is_per_character` | 11 / 13 / 7 |
| `swept_hitboxes_do_not_tunnel` | a hitbox that jumps past a capsule in one frame still hits |

Plus the unchanged suites: `mechanics` (9), `determinism` (1, GGRS
SyncTest), `content` (7), `netcode_flow` (8), `gamepad_map` (7) and the
`src/` unit tests (21, of which 18 are the `model` layer) = **68 tests**
with `cargo test` (64 without the GUI feature). Determinism across rollbacks is what makes it safe to
drive sound and rumble from the sim.

### 6.2 Measured numbers (this build)

Formulas:

| Damage | Hitlag | Electric | Crouch-cancelled | Shieldstun | Shield push (u/f) |
|---|---|---|---|---|---|
| 3 | 4 | 6 | 2 | 3 | 1.47 |
| 8 | 5 | 7 | 3 | 5 | 2.46 |
| 11 | 6 | 9 | 4 | 6 | 3.06 |
| 15 | 8 | 12 | 5 | 8 | 3.85 |
| 21 | 10 | 15 | 6 | 11 | 4.40 |

Movement (from a standing start, flat ground):

| Fighter | Squat | Dash-dance | Short hop apex / airtime | Full hop apex / airtime | 240 u run | Wavedash |
|---|---|---|---|---|---|---|
| Kestrel | 3 | 11 | 21 u / 18 f | 62 u / 31 f | 55 f (4.36 u/f) | 77 u full · 50 u at 0.7 tilt · 34 u at 0.45 |
| Boulder | 6 | 13 | 19 u / 24 f | 52 u / 40 f | 72 f (3.33 u/f) | 50 · 31 · 19 u |
| Viper | 3 | 7 | 29 u / 27 f | 72 u / 44 f | 60 f (4.00 u/f) | 69 · 43 · 30 u |

(Reference fast-faller: short hop ≈ 10.7 ref = 23.5 u, full hop ≈ 30 ref =
66 u; run 2.2 ref = 4.84 u/f. The run figure includes the initial dash and
acceleration. A full Kestrel wavedash is ≈ 35 reference units — about 2.5
fighter heights, the longest in the cast as its *Momentum* trait says — and
the tilt of the stick picks the length, as in the reference.) Letting go of the stick stops a Kestrel walk in 9 frames / 12 u
and a run in 14 frames / 28 u.

Kestrel forward-smash (15 %, 38°, bkb 25, kbg 95) on Kestrel:

| Victim % | Knockback | Hitlag | Hitstun | Launch u/f | Tumble |
|---|---|---|---|---|---|
| 0 | 60 | 8 | 23 | 3.96 | no |
| 40 | 108 | 8 | 43 | 7.10 | yes |
| 80 | 155 | 8 | 62 | 10.24 | yes |
| 120 | 203 | 8 | 81 | 13.38 | yes |

KO from centre stage (The Lattice), no DI, during hitstun: **Kestrel 110 %,
Boulder 130 %, Viper 95 %**. With DI up-and-in from the edge it moves to
~130 % for Kestrel; up-smash from centre KOs off the top at ~130 %.

### 6.3 How to measure it yourself

- **Frame counts.** Training mode (`overframe --versus kestrel,boulder`,
  hitboxes on) shows each fighter's state name, frame counter and hurt
  capsule; `overframe --anim kestrel` (←/→ steps a frame, ↑/↓ changes clip;
  `--anim kestrel:CLIP:FRAME` opens paused on a clip index) shows the hitbox
  sphere on active frames — they must match the table.
- **A/B the feel.** `overframe --demo --record frames 60 300` writes the same
  scripted match as PNG frames; compare against the 0.4.0 recording (or
  `git checkout v0.4.0`) to see stage-crossing time, hitstop length and
  launch arcs side by side.
- **Numbers, not opinions.** Copy the pattern of `tests/feel.rs`: build a
  `GameState`, drive it with `PlayerInput`s, and read `hitlag`,
  `hitstun_timer`, `kb_vel`, `pos`. Any feel claim in this document is
  reproducible that way; a PR that changes a constant should update the
  matching test.
- **Playtest checklist** (see `docs/TEST_PLAN.md`, "Manual — matches &
  feel"): jab → jab → grab is a true string at low %; nair → up-tilt links on
  a fast-faller at 20–60 %; a crouching heavy laughs off a dash attack;
  powershield → up-smash punishes a laggy aerial; every KO has a crack, a
  flash, a shake and a rumble that read as *the* big hit.

---

## 7. Tuning knobs

Every value above is a constant; the ones designers will actually touch:

| Feel | Knob | Direction |
|---|---|---|
| "Hits feel light" | `HITLAG_CAP`, the hitlag formula floor (3) | raise → heavier, slower |
| "Combos drop too early" | `HITSTUN_SCALE` (0.4) | raise → longer stun |
| "Too easy to KO" | `LAUNCH_SPEED_SCALE`, per-move `kbg` | lower → survives longer |
| "Wavedash too long / short" | per-character `traction`, `airdodge_speed` (players also pick the length with the stick tilt) | more traction → shorter |
| "Stops feel slippery / sticky" | the release multiplier on `ground_friction` (×2) | raise → snappier |
| "Shield too safe" | `shieldstun`, `SHIELD_DROP` | more stun / longer drop → riskier |
| "Ground game too slow" | `dash_max`, `run_max`, `ground_accel` | keep the *ratio* to jump/fall speeds |
| "Feedback too loud" | `sfx_volume`, `rumble` (Options) | per player |

Change one thing at a time and re-run `tests/feel.rs` — the tests are
the spec.

---

## Sources (public mechanics documentation)

- SSBWiki: *Hitlag*, *Knockback*, *Hitstun*, *Shieldstun*, *Shield*, *Smash
  directional influence*, *Directional influence*, *Sakurai angle*, *Air
  dodge*, *Roll*, *Dash-dancing*, *L-cancel*, *Auto-cancel*, and the
  per-character attribute tables for the fast-faller, heavy and lightweight
  archetypes (frame counts and speeds only).
- FightCore (fightcore.gg) publishes the same community frame data; use it
  to cross-check the shapes in §3.3.
- libmelee's stage constants for the platform / blast-zone *proportions*.

No game files, data tables extracted from a ROM/ISO, decompiled code, models
or sounds were used. Per-move values in OVERFRAME are original.
