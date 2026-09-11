//! Executable, non-behaviour-locking diagnostics for four gameplay
//! questions raised in review (2026-09 checkpoint), ahead of any
//! simulation change. Run with:
//!
//!   cargo test --test gameplay_diagnostics -- --nocapture
//!
//! Every test here MEASURES the engine's current behaviour precisely and
//! prints it (`[DIAG] ...` lines). Assertions in this file are fixture
//! sanity checks only -- "the staged scenario actually happened the way
//! it was meant to" (shield really was held N frames, the projectile
//! really existed, the hitbox really went active) -- never a verdict on
//! whether the measured behaviour is correct. That decision belongs to
//! the reviewer; a diagnostic must not bake today's behaviour, bug or
//! not, into a permanent requirement. No simulation code is touched by
//! this file.
//!
//! Four questions, matching the review's numbering:
//!  1. SpecialN's projectile against an established shield and a
//!     powershield, both facings: percent / state / shield_health /
//!     attacker+victim hitlag / shield "consumption".
//!  2. SpecialN from idle vs. as an out-of-shield option (a real input
//!     edge): does a projectile actually get spawned in both cases?
//!  3. SpecialN's own (0-damage) fighter hitbox, isolated from the
//!     projectile: its real effect on a fresh victim, on a clank, and on
//!     the staleness queue.
//!  4. ThrowB vs ThrowF vs Bair: does back-throw's launch velocity sign
//!     actually oppose forward-throw's, for both facings and all three
//!     characters?

use overframe::sim::attacks::{self, MoveId};
use overframe::sim::fighter::State;
use overframe::sim::knockback;
use overframe::sim::math::Vec2;
use overframe::sim::roster::CharacterId;
use overframe::sim::{buttons, FxKind, GameState, MatchConfig, PlayerInput};

// --------------------------------------------------------------- helpers

fn neutral() -> PlayerInput {
    PlayerInput::default()
}
fn press(b: u16) -> PlayerInput {
    PlayerInput {
        buttons: b,
        ..Default::default()
    }
}
fn stick(x: f32, y: f32) -> PlayerInput {
    PlayerInput {
        stick: Vec2::new(x, y),
        ..Default::default()
    }
}
fn stick_press(x: f32, y: f32, b: u16) -> PlayerInput {
    PlayerInput {
        stick: Vec2::new(x, y),
        buttons: b,
        ..Default::default()
    }
}

fn report(line: impl std::fmt::Display) {
    println!("[DIAG] {line}");
}

/// Two fighters `gap` apart, settled on the stage, attacker (port 0) with
/// the given facing, victim (port 1) mirrored -- same pattern as
/// `tests/feel.rs`'s and `tests/grab_ledge.rs`'s `duel()`, generalised to
/// pick characters and the attacker's facing explicitly.
fn place(chars: [CharacterId; 2], gap: f32, atk_facing: f32) -> GameState {
    let mut cfg = MatchConfig::default();
    cfg.chars[0] = chars[0];
    cfg.chars[1] = chars[1];
    let mut gs = GameState::new(2, cfg);
    for _ in 0..90 {
        gs.step(&[neutral(), neutral()]);
    }
    let (ax, bx) = if atk_facing > 0.0 {
        (-gap * 0.5, gap * 0.5)
    } else {
        (gap * 0.5, -gap * 0.5)
    };
    gs.fighters[0].pos = Vec2::new(ax, 0.0);
    gs.fighters[0].facing = atk_facing;
    gs.fighters[1].pos = Vec2::new(bx, 0.0);
    gs.fighters[1].facing = -atk_facing;
    gs.fighters[0].intangible = 0;
    gs.fighters[1].intangible = 0;
    gs
}

fn special_n_startup(ch: CharacterId) -> u32 {
    attacks::data(ch, MoveId::SpecialN).startup
}

// ============================================================ Group 1 =
// SpecialN's projectile against a genuinely established shield, a
// powershield, and the two controls (back side, no shield), both facings.
//
// The first version of this group called a shield "established" that was
// raised on tick 0 with the shot fired on tick 0 and landing on tick 1:
// age 1, still inside `POWERSHIELD_WINDOW`. The reviewer caught it. The
// shield is now warmed for `PREWARM` ticks *before* the shot is fired and
// its real age at the impact tick is measured and asserted, so the label
// and the fixture agree.

/// Ticks the shield is held before the shot is even fired, so that at
/// impact its age is far outside `POWERSHIELD_WINDOW` (2).
const PREWARM: u32 = 10;

/// Step an undefended victim until SpecialN's projectile visibly resolves
/// against them (percent changes or they leave `Stand`) -- the natural
/// impact tick, counted from the tick SPECIAL is pressed.
fn find_projectile_resolution_tick(gap: f32, atk_facing: f32) -> u32 {
    let mut gs = place(
        [CharacterId::Kestrel, CharacterId::Kestrel],
        gap,
        atk_facing,
    );
    let before = gs.fighters[1].percent;
    for i in 0..60u32 {
        let p0 = if i == 0 {
            press(buttons::SPECIAL)
        } else {
            neutral()
        };
        gs.step(&[p0, neutral()]);
        if gs.fighters[1].percent != before || !matches!(gs.fighters[1].state, State::Stand) {
            return i;
        }
    }
    panic!("SpecialN's projectile never resolved against an undefended victim within 60 ticks");
}

/// How the defender is set up for a case.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Guard {
    /// SHIELD held from tick 0; the shot is fired `PREWARM` ticks later.
    Established,
    /// SHIELD raised on the exact tick the shot lands (age 0 at impact).
    Powershield,
    /// SHIELD held, but the defender faces away from the approach.
    BackTurned,
    /// No shield at all.
    None,
}

struct ShieldDiag {
    facing: f32,
    guard: Guard,
    shield_age_at_impact: Option<u32>,
    state_at_impact: String,
    percent_before: f32,
    percent_after: f32,
    state_after: String,
    shield_health_before: f32,
    shield_health_after: f32,
    holding_drain: f32,
    attacker_hitlag: u32,
    victim_hitlag: u32,
    saw_powershield_fx: bool,
    saw_shield_fx: bool,
    saw_hit_fx: bool,
    projectiles_remaining: usize,
}

impl std::fmt::Display for ShieldDiag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "g1 facing={:+} guard={:?} | shield_age_at_impact={:?} state_at_impact={} | percent {}->{} | state_after={} | shield_health {:.2}->{:.2} (delta {:+.2}, of which holding drain {:.2}) | attacker_hitlag={} victim_hitlag={} | fx: powershield={} shield={} hit={} | projectiles_remaining={}",
            self.facing,
            self.guard,
            self.shield_age_at_impact,
            self.state_at_impact,
            self.percent_before,
            self.percent_after,
            self.state_after,
            self.shield_health_before,
            self.shield_health_after,
            self.shield_health_after - self.shield_health_before,
            self.holding_drain,
            self.attacker_hitlag,
            self.victim_hitlag,
            self.saw_powershield_fx,
            self.saw_shield_fx,
            self.saw_hit_fx,
            self.projectiles_remaining,
        )
    }
}

/// Run one shield case end to end and report what the simulation did.
/// Returns the diagnostic; the assertions here are fixture sanity only
/// (the guard really was in the state the case name claims, at the tick
/// the shot really landed).
fn run_shield_diag(atk_facing: f32, guard: Guard) -> ShieldDiag {
    let gap = 40.0;
    let flight = find_projectile_resolution_tick(gap, atk_facing);
    let fire_at = match guard {
        Guard::Established | Guard::BackTurned => PREWARM,
        _ => 0,
    };
    let impact_at = fire_at + flight;
    let mut gs = place(
        [CharacterId::Kestrel, CharacterId::Kestrel],
        gap,
        atk_facing,
    );
    if guard == Guard::BackTurned {
        // Same shield, wrong way round: the defender's back is to the shot.
        gs.fighters[1].facing = atk_facing;
    }
    let percent_before = gs.fighters[1].percent;
    let shield_health_before = gs.fighters[1].shield_health;

    let mut saw_powershield_fx = false;
    let mut saw_shield_fx = false;
    let mut saw_hit_fx = false;
    let mut attacker_hitlag = 0u32;
    let mut victim_hitlag = 0u32;
    let mut captured = false;
    let mut shield_age_at_impact = None;
    let mut state_at_impact = String::from("(never reached)");
    let mut holding_drain = 0.0f32;

    for i in 0..(impact_at + 8) {
        let p0 = if i == fire_at {
            press(buttons::SPECIAL)
        } else {
            neutral()
        };
        let shielding = match guard {
            Guard::Established | Guard::BackTurned => true,
            Guard::Powershield => i >= impact_at,
            Guard::None => false,
        };
        let p1 = if shielding {
            press(buttons::SHIELD)
        } else {
            neutral()
        };
        // The shield's real age at the moment the shot resolves. The block
        // happens inside this tick's step, after the tick's own shield
        // input has been processed: a shield already up going into the tick
        // is exactly `state_frame` ticks old, and a shield raised on this
        // very tick is zero ticks old -- a powershield by definition.
        let held_before = matches!(gs.fighters[1].state, State::Shield);
        if i == impact_at {
            state_at_impact = format!("{:?}", gs.fighters[1].state);
            holding_drain = shield_health_before - gs.fighters[1].shield_health;
            shield_age_at_impact = if held_before {
                Some(gs.fighters[1].state_frame)
            } else if shielding {
                Some(0)
            } else {
                None
            };
        }
        gs.step(&[p0, p1]);
        if i == impact_at && !held_before && shielding {
            // Raised this tick: confirm the raise actually took effect.
            state_at_impact = format!("{} -> {:?}", state_at_impact, gs.fighters[1].state);
        }
        for fx in &gs.fx {
            match fx.kind {
                FxKind::Powershield => saw_powershield_fx = true,
                FxKind::Shield => saw_shield_fx = true,
                FxKind::Hit => saw_hit_fx = true,
                _ => {}
            }
        }
        if !captured && (gs.fighters[1].percent != percent_before || gs.fighters[0].hitlag > 0) {
            captured = true;
            attacker_hitlag = gs.fighters[0].hitlag;
            victim_hitlag = gs.fighters[1].hitlag;
        }
    }

    // Fixture sanity: the guard really was what the case claims, at the
    // tick the shot really landed.
    match guard {
        Guard::Established | Guard::BackTurned => {
            let age = shield_age_at_impact.expect(
                "fixture broken: the defender was not in Shield on the tick the shot landed",
            );
            assert!(
                age >= PREWARM,
                "fixture broken: {guard:?} shield was only {age} ticks old at impact, still inside POWERSHIELD_WINDOW"
            );
        }
        Guard::Powershield => {
            let age = shield_age_at_impact.expect(
                "fixture broken: the defender was not in Shield on the tick the shot landed",
            );
            assert!(
                age < 2,
                "fixture broken: the powershield case was {age} ticks old at impact, outside the window"
            );
        }
        Guard::None => {}
    }

    ShieldDiag {
        facing: atk_facing,
        guard,
        shield_age_at_impact,
        state_at_impact,
        percent_before,
        percent_after: gs.fighters[1].percent,
        state_after: format!("{:?}", gs.fighters[1].state),
        shield_health_before,
        shield_health_after: gs.fighters[1].shield_health,
        holding_drain,
        attacker_hitlag,
        victim_hitlag,
        saw_powershield_fx,
        saw_shield_fx,
        saw_hit_fx,
        projectiles_remaining: gs.projectiles.len(),
    }
}

#[test]
fn diag_1_special_n_projectile_vs_shield_both_facings_and_kinds() {
    for &facing in &[1.0f32, -1.0] {
        for guard in [
            Guard::Established,
            Guard::Powershield,
            Guard::BackTurned,
            Guard::None,
        ] {
            let d = run_shield_diag(facing, guard);
            report(&d);
        }
    }
}

// ============================================================ Group 2 =
// SpecialN from idle vs. as a real-edge out-of-shield option: does the
// projectile actually spawn in both cases?

struct SpawnDiag {
    label: &'static str,
    entered_special_n: bool,
    peak_projectiles: usize,
    final_state: String,
}

impl std::fmt::Display for SpawnDiag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "g2 {} | entered_special_n={} | peak_projectiles={} | final_state={}",
            self.label, self.entered_special_n, self.peak_projectiles, self.final_state
        )
    }
}

#[test]
fn diag_2a_special_n_from_idle_spawns_a_projectile() {
    // Baseline: a genuine press edge from a plain idle Stand.
    let mut gs = place([CharacterId::Kestrel, CharacterId::Kestrel], 200.0, 1.0);
    let mut peak = 0usize;
    let mut entered = false;
    for i in 0..30u32 {
        let p0 = if i == 0 {
            press(buttons::SPECIAL)
        } else {
            neutral()
        };
        gs.step(&[p0, neutral()]);
        peak = peak.max(gs.projectiles.len());
        if matches!(
            gs.fighters[0].state,
            State::Attack {
                id: MoveId::SpecialN,
                ..
            }
        ) {
            entered = true;
        }
    }
    let d = SpawnDiag {
        label: "from_idle",
        entered_special_n: entered,
        peak_projectiles: peak,
        final_state: format!("{:?}", gs.fighters[0].state),
    };
    report(&d);
    assert!(entered, "fixture broken: SpecialN never actually started");
}

#[test]
fn diag_2b_special_n_pressed_as_an_out_of_shield_option_while_still_shielding() {
    // Real edge: SHIELD held for 10 frames (an established shield, not a
    // fresh press), then on frame 10 SPECIAL is pressed *without* also
    // holding SHIELD that same tick -- the standard "out of shield"
    // input. The fighter is still in `State::Shield` going into that
    // tick's processing.
    let mut gs = place([CharacterId::Kestrel, CharacterId::Kestrel], 200.0, 1.0);
    let mut peak = 0usize;
    let mut entered = false;
    let mut was_shielding_the_tick_before_the_press = false;
    for i in 0..40u32 {
        let p0 = match i {
            0..=9 => press(buttons::SHIELD),
            10 => press(buttons::SPECIAL),
            _ => neutral(),
        };
        if i == 10 {
            was_shielding_the_tick_before_the_press = matches!(gs.fighters[0].state, State::Shield);
        }
        gs.step(&[p0, neutral()]);
        peak = peak.max(gs.projectiles.len());
        if matches!(
            gs.fighters[0].state,
            State::Attack {
                id: MoveId::SpecialN,
                ..
            }
        ) {
            entered = true;
        }
    }
    assert!(
        was_shielding_the_tick_before_the_press,
        "fixture broken: fighter was not actually in State::Shield when SPECIAL was pressed"
    );
    let d = SpawnDiag {
        label: "from_shield_edge",
        entered_special_n: entered,
        peak_projectiles: peak,
        final_state: format!("{:?}", gs.fighters[0].state),
    };
    report(&d);
    assert!(
        entered,
        "fixture broken: the out-of-shield SpecialN option never actually started the move"
    );
}

#[test]
fn diag_2c_special_n_after_a_full_shield_drop_back_to_stand() {
    // Comparison path: release SHIELD, wait until genuinely back in
    // `Stand` (not just "not Shield" -- ShieldDrop has its own recovery),
    // then press SPECIAL as a fresh edge from an actionable state.
    let mut gs = place([CharacterId::Kestrel, CharacterId::Kestrel], 200.0, 1.0);
    for i in 0..10u32 {
        let _ = i;
        gs.step(&[press(buttons::SHIELD), neutral()]);
    }
    let mut back_to_stand_tick = None;
    for i in 0..40u32 {
        gs.step(&[neutral(), neutral()]);
        if matches!(gs.fighters[0].state, State::Stand) {
            back_to_stand_tick = Some(i);
            break;
        }
    }
    let recovered = back_to_stand_tick.is_some();
    let mut peak = 0usize;
    let mut entered = false;
    if recovered {
        for i in 0..30u32 {
            let p0 = if i == 0 {
                press(buttons::SPECIAL)
            } else {
                neutral()
            };
            gs.step(&[p0, neutral()]);
            peak = peak.max(gs.projectiles.len());
            if matches!(
                gs.fighters[0].state,
                State::Attack {
                    id: MoveId::SpecialN,
                    ..
                }
            ) {
                entered = true;
            }
        }
    }
    let d = SpawnDiag {
        label: "from_shield_after_full_drop",
        entered_special_n: entered,
        peak_projectiles: peak,
        final_state: format!("{:?}", gs.fighters[0].state),
    };
    report(&d);
    assert!(
        recovered,
        "fixture broken: never made it back to a plain Stand after releasing shield"
    );
    assert!(entered, "fixture broken: SpecialN never actually started");
}

// ============================================================ Group 3 =
// SpecialN's former 0-damage fighter hitbox, isolated from the
// projectile. Kestrel's projectile action no longer has one at all
// (`MoveData::no_melee`, see docs/art/procedural/anim/kestrel-gameplay-fixes.md),
// so what these measure now is the *absence*: run exactly the scenarios
// that used to produce a hit at tick 8 and report what actually happens.

struct IsolatedDiag {
    facing: f32,
    old_active_tick: u32,
    percent_before: f32,
    percent_after: f32,
    victim_left_stand_at: Option<u32>,
    peak_attacker_hitlag: u32,
    peak_victim_hitlag: u32,
    peak_hitstun_timer: u32,
    peak_launch_speed: f32,
    saw_hit_fx: bool,
    final_state: String,
}

impl std::fmt::Display for IsolatedDiag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "g3a facing={:+} old_active_tick={} | percent {}->{} (delta {:+.2}) | victim_left_stand_at={:?} | peak attacker_hitlag={} victim_hitlag={} hitstun_timer={} launch_speed={:.4} | hit_fx={} | final_state={}",
            self.facing,
            self.old_active_tick,
            self.percent_before,
            self.percent_after,
            self.percent_after - self.percent_before,
            self.victim_left_stand_at,
            self.peak_attacker_hitlag,
            self.peak_victim_hitlag,
            self.peak_hitstun_timer,
            self.peak_launch_speed,
            self.saw_hit_fx,
            self.final_state,
        )
    }
}

/// The old isolation fixture, unchanged: the victim stands inside what
/// used to be SpecialN's own melee reach and is made briefly intangible so
/// the projectile (spawned the instant SPECIAL is pressed) passes through
/// with no effect and is far downrange again before intangibility expires
/// -- well before the old melee active tick. `can_be_hit()` gates the
/// melee and projectile paths identically (`src/sim/state.rs`), so the
/// isolation is real, not simulated around.
fn run_isolated_old_melee_window(facing: f32) -> IsolatedDiag {
    let gap = 18.0; // inside the old hitbox's reach (offset 16, reach 20)
    let mut gs = place([CharacterId::Kestrel, CharacterId::Kestrel], gap, facing);
    gs.fighters[1].intangible = 7;
    let percent_before = gs.fighters[1].percent;
    let su = special_n_startup(CharacterId::Kestrel);
    let old_active_tick = su - 1; // press at T=0, old melee went active at T+startup-1

    let mut victim_left_stand_at = None;
    let mut peak_attacker_hitlag = 0u32;
    let mut peak_victim_hitlag = 0u32;
    let mut peak_hitstun_timer = 0u32;
    let mut peak_launch_speed = 0.0f32;
    let mut saw_hit_fx = false;
    for i in 0..(su + 6) {
        let p0 = if i == 0 {
            press(buttons::SPECIAL)
        } else {
            neutral()
        };
        gs.step(&[p0, neutral()]);
        // Only count from the tick intangibility has expired: before that
        // the victim is untouchable for reasons the fixture created.
        if i >= 7 {
            if victim_left_stand_at.is_none() && !matches!(gs.fighters[1].state, State::Stand) {
                victim_left_stand_at = Some(i);
            }
            peak_attacker_hitlag = peak_attacker_hitlag.max(gs.fighters[0].hitlag);
            peak_victim_hitlag = peak_victim_hitlag.max(gs.fighters[1].hitlag);
            peak_hitstun_timer = peak_hitstun_timer.max(gs.fighters[1].hitstun_timer);
            peak_launch_speed =
                peak_launch_speed.max(gs.fighters[1].vel.x.hypot(gs.fighters[1].vel.y));
            for fx in &gs.fx {
                if matches!(fx.kind, FxKind::Hit) {
                    saw_hit_fx = true;
                }
            }
        }
    }
    IsolatedDiag {
        facing,
        old_active_tick,
        percent_before,
        percent_after: gs.fighters[1].percent,
        victim_left_stand_at,
        peak_attacker_hitlag,
        peak_victim_hitlag,
        peak_hitstun_timer,
        peak_launch_speed,
        saw_hit_fx,
        final_state: format!("{:?}", gs.fighters[1].state),
    }
}

#[test]
fn diag_3a_the_old_melee_window_isolated_from_the_projectile_both_facings() {
    for &facing in &[1.0f32, -1.0] {
        let d = run_isolated_old_melee_window(facing);
        report(&d);
    }
}

#[test]
fn diag_3b_the_old_melee_window_against_a_lighter_attack() {
    // Dtilt (7% for Kestrel): |0 - 7| = 7 < CLANK_DIFF(9). This used to
    // rebound both fighters at tick 8. Measure what the same timing does
    // now that no melee hitbox exists to meet Dtilt at all.
    let su_sn = special_n_startup(CharacterId::Kestrel); // 9
    let su_dtilt = attacks::data(CharacterId::Kestrel, MoveId::Dtilt).startup; // 6
    let sn_active = su_sn - 1;
    let dtilt_press_at = sn_active - (su_dtilt - 1);

    let mut gs = place([CharacterId::Kestrel, CharacterId::Kestrel], 32.0, 1.0);
    gs.fighters[1].intangible = 7;
    let mut clanked = None;
    for i in 0..30u32 {
        let p0 = if i == 0 {
            press(buttons::SPECIAL)
        } else {
            neutral()
        };
        let p1 = if i == dtilt_press_at {
            // Dtilt is selected by a held stick.y past -TILT_Y (see
            // `Fighter::pick_ground_attack` in src/sim/fighter.rs) --
            // facing-independent, unlike Ftilt/Fsmash which read stick.x.
            stick_press(0.0, -0.55, buttons::ATTACK)
        } else {
            neutral()
        };
        gs.step(&[p0, p1]);
        if clanked.is_none()
            && (matches!(gs.fighters[0].state, State::Rebound { .. })
                || matches!(gs.fighters[1].state, State::Rebound { .. }))
        {
            clanked = Some(i);
        }
    }
    report(format!(
        "g3b vs Dtilt 7dmg at the old clank tick | clank_tick={:?} | attacker_state={:?} victim_state={:?} | attacker_percent={} victim_percent={}",
        clanked,
        gs.fighters[0].state,
        gs.fighters[1].state,
        gs.fighters[0].percent,
        gs.fighters[1].percent
    ));
}

#[test]
fn diag_3c_the_old_melee_window_against_a_much_heavier_attack() {
    // Fsmash (15% for Kestrel): |0 - 15| >= CLANK_DIFF(9).
    let su_sn = special_n_startup(CharacterId::Kestrel); // 9
    let su_fsmash = attacks::data(CharacterId::Kestrel, MoveId::Fsmash).startup; // 11
    let fsmash_active = su_fsmash - 1;
    let sn_press_at = fsmash_active - (su_sn - 1);

    let mut gs = place([CharacterId::Kestrel, CharacterId::Kestrel], 38.0, 1.0);
    gs.fighters[1].intangible = 9;
    let sn_attacker_percent_before = gs.fighters[0].percent;
    let fsmash_owner_percent_before = gs.fighters[1].percent;
    for i in 0..35u32 {
        let p0 = if i == sn_press_at {
            press(buttons::SPECIAL)
        } else {
            neutral()
        };
        let p1 = if i == 0 {
            PlayerInput {
                cstick: Vec2::new(-1.0, 0.0),
                ..Default::default()
            }
        } else {
            neutral()
        };
        gs.step(&[p0, p1]);
    }
    let sn_attacker_took_damage = gs.fighters[0].percent != sn_attacker_percent_before;
    report(format!(
        "g3c vs Fsmash 15dmg at the old clank tick | SpecialN_attacker(port0)_state={:?} percent {}->{} | Fsmash_owner(port1)_state={:?} percent {}->{} | SpecialN_attacker_took_fsmash_damage={}",
        gs.fighters[0].state,
        sn_attacker_percent_before,
        gs.fighters[0].percent,
        gs.fighters[1].state,
        fsmash_owner_percent_before,
        gs.fighters[1].percent,
        sn_attacker_took_damage,
    ));
}

#[test]
fn diag_3d_the_old_melee_window_and_the_stale_queue() {
    // Prepare a synthetic baseline queue (as if 8 Jabs had already landed
    // most-recent-first) directly on the public `stale` field -- a
    // controlled fixture, not a claim about how the queue gets there in
    // real play -- then run the old isolated SpecialN window and read the
    // *actual* `stale_multiplier` change via the crate's own public method.
    let gap = 18.0;
    let mut gs = place([CharacterId::Kestrel, CharacterId::Kestrel], gap, 1.0);
    gs.fighters[1].intangible = 7;
    for slot in 0..8 {
        gs.fighters[0].stale[slot] = Some(MoveId::Jab);
    }
    let mult_before = gs.fighters[0].stale_multiplier(MoveId::Jab);
    let stale_before = gs.fighters[0].stale;

    let su = special_n_startup(CharacterId::Kestrel);
    for i in 0..(su + 4) {
        let p0 = if i == 0 {
            press(buttons::SPECIAL)
        } else {
            neutral()
        };
        gs.step(&[p0, neutral()]);
    }
    let mult_after = gs.fighters[0].stale_multiplier(MoveId::Jab);
    report(format!(
        "g3d stale | queue_before={:?} queue_after={:?} | Jab stale_multiplier {:.4} -> {:.4} (delta {:+.4}) across the old melee window",
        stale_before,
        gs.fighters[0].stale,
        mult_before,
        mult_after,
        mult_after - mult_before
    ));
}

// ============================================================ Group 4 =
// ThrowB vs ThrowF vs Bair: launch-velocity sign vs. attacker facing, all
// three characters, both facings. Computed two ways: (a) analytically,
// calling the engine's own public `knockback` functions directly with
// each move's authored hitbox data (the exact formula chain
// `apply_hit_with` uses internally, see `src/sim/state.rs:683-729`), for
// full matrix coverage; (b) once, end-to-end, via a real grab -> throw
// input sequence for Kestrel, both facings, to confirm the analytical
// numbers match what actually happens in a real match.

struct ThrowCalc {
    character: CharacterId,
    mv_name: &'static str,
    facing: f32,
    angle_deg_authored: f32,
    resolved_angle_deg: f32,
    vel: Vec2,
}

impl std::fmt::Display for ThrowCalc {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let sign_matches_facing = self.vel.x.signum() == self.facing.signum();
        write!(
            f,
            "g4 {:?} {} facing={:+} | angle_deg authored={:.1} resolved={:.1} | vel=({:+.3},{:+.3}) | vel.x sign matches attacker facing: {}",
            self.character, self.mv_name, self.facing,
            self.angle_deg_authored, self.resolved_angle_deg,
            self.vel.x, self.vel.y, sign_matches_facing,
        )
    }
}

fn analytical_launch(
    ch: CharacterId,
    id: MoveId,
    mv_name: &'static str,
    attacker_facing: f32,
    victim_grounded: bool,
) -> ThrowCalc {
    let md = attacks::data(ch, id);
    let hb = md.hitbox;
    let victim_weight = ch.data().weight;
    // A fresh hit from 0%, matching how a duel would normally start.
    let kb = knockback::knockback(hb.damage, hb.damage, victim_weight, hb.kbg, hb.bkb);
    let angle_deg = knockback::resolve_angle(hb.angle_deg, kb, victim_grounded);
    let mut angle = angle_deg.to_radians();
    if attacker_facing < 0.0 {
        angle = std::f32::consts::PI - angle;
    }
    let vel = knockback::launch_velocity(kb, angle);
    ThrowCalc {
        character: ch,
        mv_name,
        facing: attacker_facing,
        angle_deg_authored: hb.angle_deg,
        resolved_angle_deg: angle_deg,
        vel,
    }
}

#[test]
fn diag_4a_throw_and_bair_direction_matrix_all_characters_both_facings() {
    let chars = [
        CharacterId::Kestrel,
        CharacterId::Boulder,
        CharacterId::Viper,
    ];
    let mut throwf_throwb_agree = 0;
    let mut total = 0;
    for &ch in &chars {
        for &facing in &[1.0f32, -1.0] {
            let tf = analytical_launch(ch, MoveId::ThrowF, "ThrowF", facing, true);
            let tb = analytical_launch(ch, MoveId::ThrowB, "ThrowB", facing, true);
            let ba = analytical_launch(ch, MoveId::Bair, "Bair  ", facing, false);
            report(&tf);
            report(&tb);
            report(&ba);
            total += 1;
            if tf.vel.x.signum() == tb.vel.x.signum() {
                throwf_throwb_agree += 1;
            }
        }
    }
    report(format!(
        "g4 summary: ThrowF and ThrowB launch the victim in the SAME horizontal direction in {throwf_throwb_agree}/{total} character x facing combinations (0 would mean ThrowB always mirrors ThrowF into the opposite direction)"
    ));
}

#[test]
fn diag_4b_live_grab_and_throw_direction_both_facings_kestrel() {
    for &atk_facing in &[1.0f32, -1.0] {
        for (throw_stick_x, label) in [(1.0f32, "ThrowF"), (-1.0f32, "ThrowB")] {
            let mut gs = place(
                [CharacterId::Kestrel, CharacterId::Kestrel],
                14.0,
                atk_facing,
            );
            let mut held_tick = None;
            for i in 0..20u32 {
                let p0 = if i == 0 {
                    press(buttons::GRAB)
                } else {
                    neutral()
                };
                gs.step(&[p0, neutral()]);
                if matches!(gs.fighters[1].state, State::Grabbed) {
                    held_tick = Some(i);
                    break;
                }
            }
            assert!(
                held_tick.is_some(),
                "fixture broken: standing grab never connected (facing={atk_facing})"
            );
            // Real stick-direction edge toward (ThrowF) or away from
            // (ThrowB) the attacker's own facing.
            gs.step(&[stick(throw_stick_x * atk_facing, 0.0), neutral()]);
            let chose_expected = match label {
                "ThrowF" => matches!(gs.fighters[0].state, State::Throw { id: MoveId::ThrowF }),
                _ => matches!(gs.fighters[0].state, State::Throw { id: MoveId::ThrowB }),
            };
            assert!(
                chose_expected,
                "fixture broken: stick direction did not select {label} (facing={atk_facing}, got {:?})",
                gs.fighters[0].state
            );
            let mut vel_at_launch = None;
            for _ in 0..30u32 {
                gs.step(&[neutral(), neutral()]);
                if matches!(gs.fighters[1].state, State::Hitstun { .. }) {
                    vel_at_launch = Some(gs.fighters[1].vel);
                    break;
                }
            }
            let vel = vel_at_launch.expect("fixture broken: the throw never actually released");
            report(format!(
                "g4b live {label} facing={:+} | victim launch vel=({:+.3},{:+.3}) | vel.x sign matches attacker facing: {}",
                atk_facing, vel.x, vel.y, vel.x.signum() == atk_facing.signum()
            ));
        }
    }
}
