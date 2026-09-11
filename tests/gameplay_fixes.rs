//! Regressions for the deliberate gameplay corrections in
//! `docs/art/procedural/anim/kestrel-gameplay-fixes.md`.
//!
//! Unlike `tests/gameplay_diagnostics.rs`, which measures and reports,
//! every test here asserts the *decided* behaviour. Each one fails on the
//! pre-fix simulation and passes on the fixed one.
//!
//! Everything is driven through real `GameState::step` sequences or the
//! exporter's own staged cases (`export::run_case_facing`, the same
//! harness the evidence captures use). Where a formula appears it is only
//! ever a cross-check of a quantity the fix must leave alone (knockback
//! magnitude, the vertical component) -- never the thing under test.
//!
//!   1. The projectile respects a frontal shield and a powershield.
//!   2. The shield entry into the projectile action emits exactly once.
//!   3. A connecting projectile neither adds nor erases its owner's freeze.
//!   4. Kestrel's special_n has no fighter hitbox at all.
//!   5. ThrowB and Bair launch toward the attacker's back.

use overframe::export;
use overframe::sim::attacks::{self, MoveId};
use overframe::sim::constants as k;
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
fn stick_press(x: f32, y: f32, b: u16) -> PlayerInput {
    PlayerInput {
        stick: Vec2::new(x, y),
        buttons: b,
        ..Default::default()
    }
}

/// `n` settled fighters on the stage. Port 0 is the actor, facing
/// `atk_facing`; port 1 is placed `gap` away, mirrored.
fn place(chars: [CharacterId; 4], n: usize, gap: f32, atk_facing: f32) -> GameState {
    let cfg = MatchConfig {
        chars,
        ..Default::default()
    };
    let mut gs = GameState::new(n, cfg);
    for _ in 0..90 {
        gs.step(&vec![neutral(); n]);
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
    for f in gs.fighters.iter_mut() {
        f.intangible = 0;
    }
    gs
}

fn duel(gap: f32, atk_facing: f32) -> GameState {
    place([CharacterId::Kestrel; 4], 2, gap, atk_facing)
}

/// Ticks a shield is warmed before the shot is fired, so that at impact it
/// is far outside `POWERSHIELD_WINDOW`. That relationship is the whole
/// point of the constant, so it is checked at compile time rather than
/// restated in every assertion.
const PREWARM: u32 = 10;
const _: () = assert!(PREWARM > k::POWERSHIELD_WINDOW);

/// The tick, counted from the SPECIAL press, on which the shot resolves
/// against an undefended victim at this separation.
fn flight_ticks(gap: f32, atk_facing: f32) -> u32 {
    let mut gs = duel(gap, atk_facing);
    for i in 0..60u32 {
        let p0 = if i == 0 {
            press(buttons::SPECIAL)
        } else {
            neutral()
        };
        gs.step(&[p0, neutral()]);
        if gs.fighters[1].percent > 0.0 {
            return i;
        }
    }
    panic!("the shot never resolved against an undefended victim");
}

// =========================================================== 1. shield ==

/// Everything the defence did, sampled **on the tick the shot resolved** --
/// not eight ticks later, where shieldstun has already ended and the state
/// no longer says what happened.
struct ShotOutcome {
    /// Shield age going into the impact tick: `Some(n)` when the defender
    /// was already in `Shield` for `n` ticks, `Some(0)` when the shield was
    /// raised on this very tick (a powershield by definition), `None` when
    /// there was no shield at all.
    shield_age_at_impact: Option<u32>,
    state_before_impact: String,
    /// The defender's state immediately after the tick the shot resolved.
    state_at_impact: String,
    /// `ShieldStun { total }` taken on the impact tick, if any.
    shieldstun_total: Option<u32>,
    percent_delta_at_impact: f32,
    victim_hitlag_at_impact: u32,
    attacker_hitlag_at_impact: u32,
    /// The defender's horizontal speed right after the impact tick.
    victim_vel_x_at_impact: f32,
    victim_vel_y_at_impact: f32,
    /// A body launch queued for the end of hitlag, which a block must not
    /// produce at all.
    victim_pending_launch: bool,
    shield_lost_at_impact: f32,
    powershield_fx: bool,
    shield_fx: bool,
    hit_fx: bool,
    projectiles_after: usize,
    consumed_at_impact: bool,
    percent_delta_total: f32,
}

/// Fire one shot at a defender configured by `shield_from` (the tick the
/// SHIELD button goes down, or `None` for no shield) and report what the
/// simulation actually did, sampled at the impact tick. `back_turned`
/// points the defender away from the approach without changing anything
/// else.
fn fire_one_shot(atk_facing: f32, shield_from: Option<u32>, back_turned: bool) -> ShotOutcome {
    let gap = 40.0;
    let flight = flight_ticks(gap, atk_facing);
    let fire_at = if shield_from == Some(0) { PREWARM } else { 0 };
    let impact_at = fire_at + flight;
    let mut gs = duel(gap, atk_facing);
    if back_turned {
        gs.fighters[1].facing = atk_facing;
    }
    let percent_before = gs.fighters[1].percent;

    let mut o = ShotOutcome {
        shield_age_at_impact: None,
        state_before_impact: String::new(),
        state_at_impact: String::new(),
        shieldstun_total: None,
        percent_delta_at_impact: 0.0,
        victim_hitlag_at_impact: 0,
        attacker_hitlag_at_impact: 0,
        victim_vel_x_at_impact: 0.0,
        victim_vel_y_at_impact: 0.0,
        victim_pending_launch: false,
        shield_lost_at_impact: 0.0,
        powershield_fx: false,
        shield_fx: false,
        hit_fx: false,
        projectiles_after: 0,
        consumed_at_impact: false,
        percent_delta_total: 0.0,
    };

    for i in 0..(impact_at + 8) {
        let p0 = if i == fire_at {
            press(buttons::SPECIAL)
        } else {
            neutral()
        };
        let shielding = shield_from.is_some_and(|s| i >= s);
        let p1 = if shielding {
            press(buttons::SHIELD)
        } else {
            neutral()
        };
        let health_before = gs.fighters[1].shield_health;
        let percent_at_tick_start = gs.fighters[1].percent;
        let held_before = matches!(gs.fighters[1].state, State::Shield);
        if i == impact_at {
            // The shield's real age at the moment the shot resolves: the
            // block happens inside this step, after this tick's own shield
            // input was processed, so a shield already up is `state_frame`
            // ticks old and one raised on this tick is zero ticks old.
            o.state_before_impact = format!("{:?}", gs.fighters[1].state);
            o.shield_age_at_impact = if held_before {
                Some(gs.fighters[1].state_frame)
            } else if shielding {
                Some(0)
            } else {
                None
            };
            o.consumed_at_impact = gs.projectiles.len() == 1;
        }
        gs.step(&[p0, p1]);
        if i == impact_at {
            o.state_at_impact = format!("{:?}", gs.fighters[1].state);
            o.shieldstun_total = match gs.fighters[1].state {
                State::ShieldStun { total } => Some(total),
                _ => None,
            };
            o.shield_lost_at_impact = health_before - gs.fighters[1].shield_health;
            o.percent_delta_at_impact = gs.fighters[1].percent - percent_at_tick_start;
            o.victim_hitlag_at_impact = gs.fighters[1].hitlag;
            o.attacker_hitlag_at_impact = gs.fighters[0].hitlag;
            o.victim_vel_x_at_impact = gs.fighters[1].vel.x;
            o.victim_vel_y_at_impact = gs.fighters[1].vel.y;
            o.victim_pending_launch = gs.fighters[1].pending_launch.is_some();
            o.consumed_at_impact = o.consumed_at_impact && gs.projectiles.is_empty();
        }
        for fx in &gs.fx {
            match fx.kind {
                FxKind::Powershield => o.powershield_fx = true,
                FxKind::Shield => o.shield_fx = true,
                FxKind::Hit => o.hit_fx = true,
                _ => {}
            }
        }
    }
    o.projectiles_after = gs.projectiles.len();
    o.percent_delta_total = gs.fighters[1].percent - percent_before;
    o
}

#[test]
fn fix1_an_established_frontal_shield_blocks_the_shot_instead_of_taking_it_in_the_body() {
    let d = attacks::PROJECTILE_DAMAGE;
    for &facing in &[1.0f32, -1.0] {
        let o = fire_one_shot(facing, Some(0), false);
        let at = format!(
            "facing {facing:+} at the impact tick (shield age {:?}, {} -> {})",
            o.shield_age_at_impact, o.state_before_impact, o.state_at_impact
        );

        // The fixture really is what its name says: a settled shield, well
        // outside the powershield window, measured at the impact tick.
        let age = o
            .shield_age_at_impact
            .unwrap_or_else(|| panic!("{at}: the defender was not shielding when the shot landed"));
        assert!(
            age >= PREWARM,
            "{at}: an 'established' shield must be at least {PREWARM} ticks old at impact"
        );

        // No body hit: no damage, no body hitlag, no launch queued, and no
        // launch velocity -- only the shield's own pushback.
        assert_eq!(o.percent_delta_at_impact, 0.0, "{at}: no body damage");
        assert_eq!(o.percent_delta_total, 0.0, "{at}: and none later either");
        assert_eq!(o.victim_hitlag_at_impact, 0, "{at}: no body hitlag");
        assert!(!o.victim_pending_launch, "{at}: no body launch queued");
        assert_eq!(o.victim_vel_y_at_impact, 0.0, "{at}: no vertical launch");

        // The ordinary block response, with the engine's own numbers.
        let expected_stun = knockback::shieldstun(d);
        assert_eq!(
            o.shieldstun_total,
            Some(expected_stun),
            "{at}: the block takes the ordinary shieldstun"
        );
        let expected_push = -(-facing) * knockback::shield_push(d);
        assert!(
            (o.victim_vel_x_at_impact - expected_push).abs() < 1e-3,
            "{at}: the defender takes the ordinary shield pushback ({expected_push:+.4}), measured {:+.4}",
            o.victim_vel_x_at_impact
        );
        let expected_health = k::SHIELD_DECAY + d * k::SHIELD_DAMAGE_MULT - k::SHIELD_REGEN;
        assert!(
            (o.shield_lost_at_impact - expected_health).abs() < 1e-3,
            "{at}: the shield pays decay {:.3} + {:.1}x{:.2} - regen {:.3} = {expected_health:.3}, measured {:.3}",
            k::SHIELD_DECAY,
            d,
            k::SHIELD_DAMAGE_MULT,
            k::SHIELD_REGEN,
            o.shield_lost_at_impact
        );

        assert!(
            !o.hit_fx && o.shield_fx && !o.powershield_fx,
            "{at}: shield feedback, not a body hit (shield={} hit={} powershield={})",
            o.shield_fx,
            o.hit_fx,
            o.powershield_fx
        );
        assert!(
            o.consumed_at_impact,
            "{at}: the shot is spent on the shield"
        );
        assert_eq!(o.projectiles_after, 0, "{at}: exactly once");
        assert_eq!(
            o.attacker_hitlag_at_impact, 0,
            "{at}: a thrown object does not freeze its thrower"
        );
    }
}

#[test]
fn fix1_a_powershield_takes_no_shield_damage_or_stun_beyond_the_ordinary_holding_drain() {
    for &facing in &[1.0f32, -1.0] {
        let gap = 40.0;
        let impact = flight_ticks(gap, facing);
        // Raised on the exact tick the shot lands: age 0, inside the window.
        let o = fire_one_shot(facing, Some(impact), false);
        let at = format!(
            "facing {facing:+} at the impact tick (shield age {:?}, {} -> {})",
            o.shield_age_at_impact, o.state_before_impact, o.state_at_impact
        );
        assert_eq!(
            o.shield_age_at_impact,
            Some(0),
            "{at}: the fixture must be a real powershield, raised on the impact tick"
        );

        assert_eq!(o.percent_delta_at_impact, 0.0, "{at}: no body damage");
        assert_eq!(o.percent_delta_total, 0.0, "{at}: and none later either");
        assert_eq!(o.victim_hitlag_at_impact, 0, "{at}: no body hitlag");
        assert!(!o.victim_pending_launch, "{at}: no body launch queued");

        // Neither the extra shield damage nor the shieldstun a normal
        // block takes -- and no pushback either. The ordinary holding
        // drain is the only thing the shield pays, and it is discounted
        // explicitly rather than folded into a tolerance.
        assert_eq!(
            o.shieldstun_total, None,
            "{at}: a powershield takes no shieldstun, got {}",
            o.state_at_impact
        );
        assert!(
            o.state_at_impact.contains("Shield {")
                || o.state_at_impact == "Shield"
                || o.state_at_impact.starts_with("Shield"),
            "{at}: the defender is still shielding"
        );
        // No shield damage at all beyond the ordinary holding drain --
        // and on the raise tick not even that, because the drain only runs
        // for a shield that was already up. Bounded both ways so neither a
        // block's cost nor a silent regrowth can hide in a tolerance.
        let block_cost = attacks::PROJECTILE_DAMAGE * k::SHIELD_DAMAGE_MULT;
        assert!(
            o.shield_lost_at_impact <= k::SHIELD_DECAY + 1e-3,
            "{at}: a powershield may pay at most the ordinary holding drain ({:.3}), measured {:.3}",
            k::SHIELD_DECAY,
            o.shield_lost_at_impact
        );
        assert!(
            o.shield_lost_at_impact < block_cost,
            "{at}: and never a block's shield damage ({block_cost:.3}), measured {:.3}",
            o.shield_lost_at_impact
        );
        assert_eq!(
            o.shield_lost_at_impact, 0.0,
            "{at}: raised on this very tick, so not even the holding drain has run yet"
        );
        assert_eq!(
            o.victim_vel_x_at_impact, 0.0,
            "{at}: and takes no shield pushback"
        );
        assert!(
            o.powershield_fx && !o.hit_fx,
            "{at}: a powershield reports itself (powershield={} hit={})",
            o.powershield_fx,
            o.hit_fx
        );
        assert!(o.consumed_at_impact, "{at}: still consumed, exactly once");
        assert_eq!(o.projectiles_after, 0, "{at}");
    }
}

#[test]
fn fix1_controls_a_shot_into_the_back_or_into_no_shield_still_hits() {
    for &facing in &[1.0f32, -1.0] {
        let back = fire_one_shot(facing, Some(0), true);
        assert!(
            back.shield_age_at_impact.is_some(),
            "facing {facing:+}: the control really is shielding, just the wrong way round"
        );
        assert_eq!(
            back.percent_delta_at_impact,
            attacks::PROJECTILE_DAMAGE,
            "facing {facing:+}: a shield facing away blocks nothing"
        );
        assert!(back.victim_hitlag_at_impact > 0, "facing {facing:+}");
        assert!(back.hit_fx && !back.shield_fx, "facing {facing:+}");

        let bare = fire_one_shot(facing, None, false);
        assert_eq!(bare.shield_age_at_impact, None, "facing {facing:+}");
        assert_eq!(
            bare.percent_delta_at_impact,
            attacks::PROJECTILE_DAMAGE,
            "facing {facing:+}: an unshielded victim takes the shot"
        );
        assert!(bare.hit_fx, "facing {facing:+}");
    }
}

#[test]
fn fix1_the_block_follows_the_shot_not_where_its_owner_ended_up() {
    // Once the shot is away it is on its own. While it travels, the owner
    // is turned around and teleported to the far side of the defender --
    // so the *owner's* position and facing now say "this comes from
    // behind", while the shot itself is still approaching the defender's
    // front. The block must follow the shot.
    for &facing in &[1.0f32, -1.0] {
        let gap = 90.0;
        let mut gs = duel(gap, facing);
        let percent_before = gs.fighters[1].percent;
        let mut moved = false;
        let mut resolved = None;
        for i in 0..80u32 {
            let p0 = if i == 0 {
                press(buttons::SPECIAL)
            } else {
                neutral()
            };
            let had = gs.projectiles.len();
            gs.step(&[p0, press(buttons::SHIELD)]);
            // As soon as the shot exists, send its owner to the far side,
            // facing back the other way.
            if !moved && !gs.projectiles.is_empty() {
                gs.fighters[0].pos.x = gs.fighters[1].pos.x + facing * 40.0;
                gs.fighters[0].facing = -facing;
                moved = true;
            }
            if had == 1 && gs.projectiles.is_empty() {
                resolved = Some(gs.fighters[1].percent - percent_before);
                break;
            }
        }
        let dealt = resolved.expect("fixture: the shot never resolved");
        assert!(
            moved,
            "fixture: the owner was never relocated behind the defender"
        );
        assert_eq!(
            dealt, 0.0,
            "facing {facing:+}: the frontal shield must still block a shot whose owner has since turned and moved behind the defender (dealt {dealt})"
        );
    }
}

// ========================================================= 2. emission ==

/// Peak projectile count and the state reached, for one entry script.
fn emission_case(
    atk_facing: f32,
    script: impl Fn(u32) -> PlayerInput,
    ticks: u32,
) -> (usize, bool) {
    let mut gs = duel(200.0, atk_facing);
    let mut peak = 0usize;
    let mut entered = false;
    let mut total_spawned = 0usize;
    let mut last = 0usize;
    for i in 0..ticks {
        gs.step(&[script(i), neutral()]);
        let now = gs.projectiles.len();
        if now > last {
            total_spawned += now - last;
        }
        last = now;
        peak = peak.max(now);
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
    let _ = peak;
    (total_spawned, entered)
}

#[test]
fn fix2_the_shield_entry_emits_exactly_one_shot_on_a_real_press_edge() {
    for &facing in &[1.0f32, -1.0] {
        // SHIELD held for 10 ticks (a settled shield, not a fresh press),
        // then a real released-to-pressed SPECIAL edge out of it.
        let (spawned, entered) = emission_case(
            facing,
            |i| match i {
                0..=9 => press(buttons::SHIELD),
                10 => press(buttons::SPECIAL),
                _ => neutral(),
            },
            40,
        );
        assert!(entered, "facing {facing:+}: the action must still start");
        assert_eq!(
            spawned, 1,
            "facing {facing:+}: the shield entry must emit exactly one shot"
        );
    }
}

#[test]
fn fix2_holding_special_after_the_shield_entry_does_not_repeat_the_shot() {
    for &facing in &[1.0f32, -1.0] {
        let (spawned, entered) = emission_case(
            facing,
            |i| match i {
                0..=9 => press(buttons::SHIELD),
                // Pressed on tick 10 and never released afterwards.
                _ => press(buttons::SPECIAL),
            },
            80,
        );
        assert!(entered, "facing {facing:+}");
        assert_eq!(
            spawned, 1,
            "facing {facing:+}: holding SPECIAL must not auto-repeat the shot"
        );
    }
}

#[test]
fn fix2_controls_idle_and_full_shield_drop_entries_each_emit_once() {
    for &facing in &[1.0f32, -1.0] {
        let (idle, entered_idle) = emission_case(
            facing,
            |i| {
                if i == 0 {
                    press(buttons::SPECIAL)
                } else {
                    neutral()
                }
            },
            40,
        );
        assert!(entered_idle && idle == 1, "facing {facing:+}: idle entry");

        // Shield, release, wait out ShieldDrop, then press from Stand.
        let (dropped, entered_dropped) = emission_case(
            facing,
            |i| match i {
                0..=9 => press(buttons::SHIELD),
                10..=29 => neutral(),
                30 => press(buttons::SPECIAL),
                _ => neutral(),
            },
            70,
        );
        assert!(
            entered_dropped && dropped == 1,
            "facing {facing:+}: full shield-drop entry"
        );
    }
}

#[test]
fn fix2_the_shield_entry_opens_no_new_cancel() {
    // The action still runs its whole committed length out of shield: the
    // entry was fixed to emit, not to become a cancel.
    let md = attacks::data(CharacterId::Kestrel, MoveId::SpecialN);
    let mut gs = duel(200.0, 1.0);
    for i in 0..10u32 {
        let _ = i;
        gs.step(&[press(buttons::SHIELD), neutral()]);
    }
    gs.step(&[press(buttons::SPECIAL), neutral()]);
    let mut frames_in_action = 0u32;
    for _ in 0..(md.total() + 10) {
        if matches!(
            gs.fighters[0].state,
            State::Attack {
                id: MoveId::SpecialN,
                ..
            }
        ) {
            frames_in_action += 1;
        }
        gs.step(&[neutral(), neutral()]);
    }
    assert_eq!(
        frames_in_action,
        md.total(),
        "the out-of-shield entry keeps the action's full commitment"
    );
}

// ==================================================== 3. owner  hitlag ==

#[test]
fn fix3_a_connecting_shot_neither_adds_nor_erases_its_owners_existing_freeze() {
    // Three fighters, all real: port 0 fires at port 1 far downrange, and
    // port 2 hits port 0 while the shot is still travelling. When the shot
    // lands, port 0 is in someone else's hitlag -- a freeze the projectile
    // has no business touching in either direction.
    let mut gs = place(
        [
            CharacterId::Kestrel,
            CharacterId::Kestrel,
            CharacterId::Kestrel,
            CharacterId::Kestrel,
        ],
        3,
        150.0,
        1.0,
    );
    gs.fighters[2].pos = Vec2::new(gs.fighters[0].pos.x - 16.0, 0.0);
    gs.fighters[2].facing = 1.0;

    let mut owner_hitlag_at_impact = None;
    let mut hitlag_before_impact = 0;
    let mut owner_facing_before = gs.fighters[0].facing;
    let mut owner_state_before = format!("{:?}", gs.fighters[0].state);
    let victim_percent_before = gs.fighters[1].percent;

    for i in 0..80u32 {
        let p0 = if i == 0 {
            press(buttons::SPECIAL)
        } else {
            neutral()
        };
        // Port 2 swings a little later, so its freeze is running when the
        // shot lands downrange.
        let p2 = if i == 14 {
            press(buttons::ATTACK)
        } else {
            neutral()
        };
        let before = gs.fighters[0].hitlag;
        let facing_before = gs.fighters[0].facing;
        let state_before = format!("{:?}", gs.fighters[0].state);
        let projectiles_before = gs.projectiles.len();
        gs.step(&[p0, neutral(), p2]);
        let landed = projectiles_before == 1
            && gs.projectiles.is_empty()
            && gs.fighters[1].percent > victim_percent_before;
        if landed {
            hitlag_before_impact = before;
            owner_hitlag_at_impact = Some(gs.fighters[0].hitlag);
            owner_facing_before = facing_before;
            owner_state_before = state_before;
            break;
        }
    }

    let at_impact = owner_hitlag_at_impact.expect("fixture: the shot never landed on port 1");
    assert!(
        hitlag_before_impact > 1,
        "fixture: the owner must really be frozen by port 2 when the shot lands (had {hitlag_before_impact})"
    );
    // A tick of the owner's own freeze ran during this step; nothing else.
    assert_eq!(
        at_impact,
        hitlag_before_impact - 1,
        "the shot must neither erase the owner's existing freeze nor add one of its own"
    );
    assert_eq!(
        gs.fighters[0].facing, owner_facing_before,
        "the owner's facing survives its shot connecting"
    );
    assert_eq!(
        format!("{:?}", gs.fighters[0].state),
        owner_state_before,
        "the owner's state survives its shot connecting"
    );
}

#[test]
fn fix3_a_connecting_shot_still_does_not_freeze_an_unfrozen_owner() {
    let mut gs = duel(60.0, 1.0);
    let before = gs.fighters[1].percent;
    for i in 0..40u32 {
        let p0 = if i == 0 {
            press(buttons::SPECIAL)
        } else {
            neutral()
        };
        gs.step(&[p0, neutral()]);
        if gs.fighters[1].percent > before {
            assert_eq!(
                gs.fighters[0].hitlag, 0,
                "a projectile never freezes its owner"
            );
            return;
        }
    }
    panic!("fixture: the shot never landed");
}

// ================================================ 4. no melee placeholder ==

#[test]
fn fix4_kestrel_special_n_defines_no_fighter_hitbox_on_any_frame() {
    // Asserted through behaviour, not through the new field: this file
    // has to compile and run unchanged against the pre-fix simulation so
    // the BEFORE and AFTER runs are the same bytes. The declaration
    // itself (`MoveData::no_melee`) is checked by the crate's own unit
    // tests, which only exist after the fix.
    let md = attacks::data(CharacterId::Kestrel, MoveId::SpecialN);
    for sf in 0..md.total() {
        assert!(
            md.hitbox_at(sf).is_none(),
            "sf {sf}: special_n must define no fighter hitbox"
        );
    }
}

#[test]
fn fix4_the_old_tick_8_window_produces_no_hit_no_freeze_no_stale_and_no_fx() {
    // Exactly the baseline's isolation fixture: the victim stands inside
    // what used to be the melee reach and is briefly intangible so the
    // projectile passes through and flies on. What used to happen at tick
    // 8 -- 0 damage, 3 frames of hitlag on both, 4 of hitstun, a stale
    // entry -- must not happen at all now.
    let su = attacks::data(CharacterId::Kestrel, MoveId::SpecialN).startup;
    for &facing in &[1.0f32, -1.0] {
        let mut gs = duel(18.0, facing);
        gs.fighters[1].intangible = 7;
        for slot in 0..8 {
            gs.fighters[0].stale[slot] = Some(MoveId::Jab);
        }
        let stale_before = gs.fighters[0].stale;
        let mult_before = gs.fighters[0].stale_multiplier(MoveId::Jab);
        let percent_before = gs.fighters[1].percent;

        for i in 0..(su + 6) {
            let p0 = if i == 0 {
                press(buttons::SPECIAL)
            } else {
                neutral()
            };
            gs.step(&[p0, neutral()]);
            // From the tick intangibility expires, nothing may happen.
            if i >= 7 {
                assert_eq!(
                    gs.fighters[1].percent, percent_before,
                    "facing {facing:+} tick {i}: no damage"
                );
                assert!(
                    !matches!(gs.fighters[1].state, State::Hitstun { .. }),
                    "facing {facing:+} tick {i}: no hitstun"
                );
                assert_eq!(
                    gs.fighters[1].hitlag, 0,
                    "facing {facing:+} tick {i}: no victim hitlag"
                );
                assert_eq!(
                    gs.fighters[0].hitlag, 0,
                    "facing {facing:+} tick {i}: no attacker hitlag"
                );
                assert!(
                    !gs.fx.iter().any(|f| matches!(f.kind, FxKind::Hit)),
                    "facing {facing:+} tick {i}: no hit effect"
                );
            }
        }
        assert_eq!(
            gs.fighters[0].stale, stale_before,
            "facing {facing:+}: the removed placeholder must not take a stale slot"
        );
        assert_eq!(
            gs.fighters[0].stale_multiplier(MoveId::Jab),
            mult_before,
            "facing {facing:+}: and must not move another move's multiplier"
        );
    }
}

#[test]
fn fix4_the_removed_placeholder_no_longer_clanks() {
    // Dtilt timed onto the old tick-8 window used to rebound both
    // fighters. With no hitbox there is nothing for it to meet.
    let su_sn = attacks::data(CharacterId::Kestrel, MoveId::SpecialN).startup;
    let su_dtilt = attacks::data(CharacterId::Kestrel, MoveId::Dtilt).startup;
    let dtilt_press_at = (su_sn - 1) - (su_dtilt - 1);
    let mut gs = duel(32.0, 1.0);
    gs.fighters[1].intangible = 7;
    for i in 0..30u32 {
        let p0 = if i == 0 {
            press(buttons::SPECIAL)
        } else {
            neutral()
        };
        let p1 = if i == dtilt_press_at {
            stick_press(0.0, -0.55, buttons::ATTACK)
        } else {
            neutral()
        };
        gs.step(&[p0, p1]);
        assert!(
            !matches!(gs.fighters[0].state, State::Rebound { .. }),
            "tick {i}: the projectile owner must not rebound off anything"
        );
    }
}

#[test]
fn fix4_the_projectile_itself_and_the_actions_commitment_are_unchanged() {
    let md = attacks::data(CharacterId::Kestrel, MoveId::SpecialN);
    // The action's own timeline is the bug fix's explicit non-goal.
    assert_eq!((md.startup, md.active, md.endlag), (9, 1, 22));
    assert_eq!(md.total(), 32);
    // And the shot is the same shot.
    let mut gs = duel(60.0, 1.0);
    let mut spawned_at = None;
    let before = gs.fighters[1].percent;
    for i in 0..40u32 {
        let p0 = if i == 0 {
            press(buttons::SPECIAL)
        } else {
            neutral()
        };
        gs.step(&[p0, neutral()]);
        if spawned_at.is_none() && !gs.projectiles.is_empty() {
            spawned_at = Some(i);
        }
        if gs.fighters[1].percent > before {
            assert_eq!(
                gs.fighters[1].percent - before,
                attacks::PROJECTILE_DAMAGE,
                "the shot still deals its own damage"
            );
            assert_eq!(spawned_at, Some(0), "and is still released on entry");
            return;
        }
    }
    panic!("fixture: the shot never landed");
}

#[test]
fn fix4_is_kestrel_only_and_is_not_a_global_zero_damage_rule() {
    for ch in [CharacterId::Boulder, CharacterId::Viper] {
        let md = attacks::data(ch, MoveId::SpecialN);
        assert!(
            md.hitbox_at(md.startup).is_some(),
            "{ch:?} special_n still swings on its startup frame"
        );
        assert_eq!(
            md.hitbox.damage, 0.0,
            "{ch:?} special_n's hitbox is still zero damage -- zero damage alone never removes a hitbox"
        );
    }
}

// ================================================== 5. rearward launch ==

/// One measured launch from a real match, with the evidence that it was
/// the connection the test meant to produce.
#[derive(Debug, Clone, Copy)]
struct Launch {
    vel: Vec2,
    damage: f32,
    /// The attacker's state and frame on the tick the hit landed.
    attacker_frame: u32,
    attacker_aerial: bool,
    attacker_facing: f32,
}

impl Launch {
    fn magnitude(&self) -> f32 {
        self.vel.x.hypot(self.vel.y)
    }
}

/// The victim's launch from the exporter's own staged case for this
/// character/move/facing -- a real match on the same harness the evidence
/// captures use, not a formula.
fn staged_launch(ch: CharacterId, id: MoveId, variant: &str, facing: f32) -> Launch {
    let ticks = export::run_case_facing(ch, id, variant, facing)
        .unwrap_or_else(|e| panic!("{ch:?} {id:?} {variant} facing {facing:+}: {e}"));
    let mut before = 0.0;
    for t in &ticks {
        if t.state.fighters.len() < 2 {
            continue;
        }
        let v = &t.state.fighters[1];
        if v.percent > before {
            let a = &t.state.fighters[0];
            return Launch {
                vel: v.vel,
                damage: v.percent - before,
                attacker_frame: a.state_frame,
                attacker_aerial: !a.grounded,
                attacker_facing: a.facing,
            };
        }
        before = v.percent;
    }
    panic!("{ch:?} {id:?} {variant} facing {facing:+}: never connected");
}

/// Collects failures instead of aborting on the first, so one run reports
/// the whole matrix.
#[derive(Default)]
struct Failures(Vec<String>);

impl Failures {
    fn check(&mut self, ok: bool, msg: impl std::fmt::Display) {
        if !ok {
            self.0.push(msg.to_string());
        }
    }
    fn finish(self, what: &str) {
        if !self.0.is_empty() {
            panic!(
                "{what}: {} of the matrix failed:\n  {}",
                self.0.len(),
                self.0.join("\n  ")
            );
        }
    }
}

const CAST: [CharacterId; 3] = [
    CharacterId::Kestrel,
    CharacterId::Boulder,
    CharacterId::Viper,
];

#[test]
fn fix5_throw_b_launches_behind_the_thrower_in_real_matches_all_characters_both_facings() {
    let mut f = Failures::default();
    for ch in CAST {
        for &facing in &[1.0f32, -1.0] {
            let back = staged_launch(ch, MoveId::ThrowB, "from_grab", facing);
            let fwd = staged_launch(ch, MoveId::ThrowF, "from_grab", facing);
            // The attacker really threw: its own facing is unchanged by
            // the throw, and it is the thrower we measured against.
            f.check(
                back.attacker_facing == facing,
                format!(
                    "{ch:?} ThrowB facing {facing:+}: the thrower's own facing changed to {} -- a rearward launch must not turn the attacker",
                    back.attacker_facing
                ),
            );
            f.check(
                back.vel.x.signum() == -facing,
                format!(
                    "{ch:?} ThrowB facing {facing:+}: must send the victim behind the thrower, got vel=({:+.3},{:+.3}) |v|={:.3} dmg={}",
                    back.vel.x, back.vel.y, back.magnitude(), back.damage
                ),
            );
            f.check(
                fwd.vel.x.signum() == facing,
                format!(
                    "{ch:?} ThrowF facing {facing:+}: control must still send the victim forward, got vel=({:+.3},{:+.3})",
                    fwd.vel.x, fwd.vel.y
                ),
            );
            f.check(
                back.vel.y > 0.0,
                format!(
                    "{ch:?} ThrowB facing {facing:+}: the upward component must survive, got {:+.3}",
                    back.vel.y
                ),
            );
            println!(
                "[MATRIX] {ch:?} facing {facing:+} ThrowB vel=({:+.3},{:+.3}) |v|={:.3} dmg={} | ThrowF vel=({:+.3},{:+.3}) |v|={:.3}",
                back.vel.x,
                back.vel.y,
                back.magnitude(),
                back.damage,
                fwd.vel.x,
                fwd.vel.y,
                fwd.magnitude()
            );
        }
        // Mirroring the direction changes nothing else: the same throw at
        // both facings is one launch, reflected. Magnitude and vertical
        // component are therefore reported before/after by construction.
        let a = staged_launch(ch, MoveId::ThrowB, "from_grab", 1.0);
        let b = staged_launch(ch, MoveId::ThrowB, "from_grab", -1.0);
        f.check(
            (a.vel.x + b.vel.x).abs() < 1e-3 && (a.vel.y - b.vel.y).abs() < 1e-3,
            format!(
                "{ch:?} ThrowB: magnitude/vertical must be facing-symmetric, got {:?} vs {:?}",
                a.vel, b.vel
            ),
        );
        f.check(
            (a.magnitude() - b.magnitude()).abs() < 1e-3,
            format!(
                "{ch:?} ThrowB: |v| differs between facings ({:.4} vs {:.4})",
                a.magnitude(),
                b.magnitude()
            ),
        );
    }
    f.finish("ThrowB rearward matrix");
}

#[test]
fn fix5_the_other_throws_are_untouched() {
    let mut f = Failures::default();
    for ch in CAST {
        for &facing in &[1.0f32, -1.0] {
            let up = staged_launch(ch, MoveId::ThrowU, "from_grab", facing);
            f.check(
                up.vel.y > up.vel.x.abs(),
                format!(
                    "{ch:?} ThrowU facing {facing:+}: still goes up, got ({:+.3},{:+.3})",
                    up.vel.x, up.vel.y
                ),
            );
            let down = staged_launch(ch, MoveId::ThrowD, "from_grab", facing);
            f.check(
                down.vel.x.signum() == facing,
                format!(
                    "{ch:?} ThrowD facing {facing:+}: still sends the victim forward, got ({:+.3},{:+.3})",
                    down.vel.x, down.vel.y
                ),
            );
            println!(
                "[MATRIX] {ch:?} facing {facing:+} ThrowU vel=({:+.3},{:+.3}) | ThrowD vel=({:+.3},{:+.3})",
                up.vel.x, up.vel.y, down.vel.x, down.vel.y
            );
        }
    }
    f.finish("forward/up/down throw controls");
}

/// A pinned aerial script: where the victim stands, when the attacker
/// swings, and (for a late hit) the tick the victim steps into reach so
/// the clean window passes over empty air first. Nothing about the move is
/// changed to produce either hit -- only where the victim is standing.
#[derive(Debug, Clone, Copy)]
struct AerialScript {
    reach: f32,
    attack_tick: u32,
    arrive_at: Option<u32>,
}

/// Run one aerial script and return the connection it produced.
///
/// `pinned` says how a mismatch is treated. A pinned script is part of the
/// test's own claim, so a connection from another move, from the ground,
/// or with a changed facing is a defect and fails loudly. A search is
/// allowed to stumble into a grounded smash or a neighbouring move on its
/// way to a real aerial, so there a mismatch is simply "not this script".
fn live_aerial(
    ch: CharacterId,
    id: MoveId,
    facing: f32,
    script: AerialScript,
    pinned: bool,
) -> Option<Launch> {
    let side = if id == MoveId::Bair { -facing } else { facing };
    let spot = side * script.reach;
    let parked = side * 400.0;
    let mut gs = place([ch; 4], 2, 40.0, facing);
    gs.fighters[0].pos = Vec2::new(0.0, 0.0);
    gs.fighters[0].facing = facing;
    gs.fighters[1].pos = Vec2::new(
        if script.arrive_at.is_some() {
            parked
        } else {
            spot
        },
        0.0,
    );
    gs.fighters[1].facing = facing;
    let before = gs.fighters[1].percent;
    for i in 0..60u32 {
        if script.arrive_at == Some(i) {
            gs.fighters[1].pos = Vec2::new(spot, gs.fighters[0].pos.y);
        }
        let stick_x = if id == MoveId::Bair { -facing } else { facing };
        let (sx, sy) = match id {
            MoveId::Uair => (0.0, 1.0),
            MoveId::Dair => (0.0, -1.0),
            _ => (stick_x, 0.0),
        };
        let p0 = if i == 0 {
            press(buttons::JUMP)
        } else if i == script.attack_tick {
            stick_press(sx, sy, buttons::ATTACK)
        } else {
            neutral()
        };
        gs.step(&[p0, neutral()]);
        if gs.fighters[1].percent > before {
            let a = &gs.fighters[0];
            // The connection really came from the move under test, in the
            // air, with the attacker's facing unchanged.
            let (state_id, aerial) = match a.state {
                State::Attack { id, aerial } => (Some(id), aerial),
                _ => (None, false),
            };
            let right_move = state_id == Some(id) && aerial && !a.grounded;
            let right_facing = a.facing == facing;
            if !(right_move && right_facing) {
                assert!(
                    !pinned,
                    "{ch:?} {id:?} facing {facing:+} {script:?}: pinned script connected from {:?} (aerial {aerial}, grounded {}, facing {}) -- not the move under test",
                    a.state,
                    a.grounded,
                    a.facing
                );
                return None;
            }
            return Some(Launch {
                vel: gs.fighters[1].vel,
                damage: gs.fighters[1].percent - before,
                attacker_frame: a.state_frame,
                attacker_aerial: aerial,
                attacker_facing: a.facing,
            });
        }
    }
    None
}

/// The pinned scripts, found once by search and written down here so the
/// test runs the same two connections every time instead of re-searching.
/// `clean` lands inside `startup..startup+active`; `late` lands inside the
/// lingering window after it -- both asserted below against the real frame
/// data, so a wrong script fails loudly instead of silently measuring the
/// wrong hit.
fn bair_scripts(ch: CharacterId) -> (AerialScript, AerialScript) {
    // Found once by search over (reach, attack tick, arrival tick),
    // keeping only scripts that connect identically at both facings, and
    // written down here. The windows below are asserted against the move's
    // real frame data, so if a script ever stops landing where it claims
    // the test says so instead of quietly measuring the other hit.
    match ch {
        // clean: attacker frame 4 (window 4..8), 13 damage
        // late:  attacker frame 8 (window 8..20), 8 damage
        CharacterId::Kestrel => (
            AerialScript {
                reach: 12.0,
                attack_tick: 3,
                arrive_at: None,
            },
            AerialScript {
                reach: 12.0,
                attack_tick: 3,
                arrive_at: Some(10),
            },
        ),
        // clean: attacker frame 8 (window 8..13), 16 damage
        // late:  attacker frame 14 (window 13..23), 10 damage
        CharacterId::Boulder => (
            AerialScript {
                reach: 16.0,
                attack_tick: 5,
                arrive_at: None,
            },
            AerialScript {
                reach: 16.0,
                attack_tick: 5,
                arrive_at: Some(18),
            },
        ),
        // clean: attacker frame 4 (window 4..8), 11 damage
        // late:  attacker frame 8 (window 8..18), 7 damage
        CharacterId::Viper => (
            AerialScript {
                reach: 11.0,
                attack_tick: 3,
                arrive_at: None,
            },
            AerialScript {
                reach: 11.0,
                attack_tick: 3,
                arrive_at: Some(10),
            },
        ),
    }
}

#[test]
fn fix5_bair_launches_behind_the_attacker_clean_and_late_all_characters_both_facings() {
    let mut f = Failures::default();
    for ch in CAST {
        let md = attacks::data(ch, MoveId::Bair);
        let clean_damage = md.hitbox.damage;
        let late_damage = (md.hitbox.damage * md.late_scale).round();
        let clean_end = md.startup + md.active;
        let late_end = clean_end + md.late_active;
        assert!(
            md.late_active > 0 && (late_damage - clean_damage).abs() > 0.5,
            "{ch:?}: bair really has a distinguishable late window"
        );
        let (clean_script, late_script) = bair_scripts(ch);
        for &facing in &[1.0f32, -1.0] {
            for (label, script, want_damage, window) in [
                ("clean", clean_script, clean_damage, md.startup..clean_end),
                ("late", late_script, late_damage, clean_end..late_end),
            ] {
                let Some(l) = live_aerial(ch, MoveId::Bair, facing, script, true) else {
                    f.check(
                        false,
                        format!(
                            "{ch:?} {label} bair facing {facing:+} {script:?}: never connected"
                        ),
                    );
                    continue;
                };
                // The pinned script really produced the window it claims --
                // checked against the move's own frame data, not inferred
                // from the damage alone.
                f.check(
                    window.contains(&l.attacker_frame),
                    format!(
                        "{ch:?} {label} bair facing {facing:+} {script:?}: connected on attacker frame {}, outside {window:?}",
                        l.attacker_frame
                    ),
                );
                f.check(
                    (l.damage - want_damage).abs() < 0.5,
                    format!(
                        "{ch:?} {label} bair facing {facing:+} {script:?}: dealt {} damage, expected {want_damage}",
                        l.damage
                    ),
                );
                f.check(
                    l.vel.x.signum() == -facing,
                    format!(
                        "{ch:?} {label} bair facing {facing:+} {script:?}: must send the victim behind the attacker, got vel=({:+.3},{:+.3}) |v|={:.3} on frame {}",
                        l.vel.x, l.vel.y, l.magnitude(), l.attacker_frame
                    ),
                );
                println!(
                    "[MATRIX] {ch:?} facing {facing:+} bair {label} frame {} aerial {} vel=({:+.3},{:+.3}) |v|={:.3} dmg={}",
                    l.attacker_frame,
                    l.attacker_aerial,
                    l.vel.x,
                    l.vel.y,
                    l.magnitude(),
                    l.damage
                );
            }
            // Magnitude and vertical component survive the direction change:
            // the same connection mirrored.
            if let (Some(a), Some(b)) = (
                live_aerial(ch, MoveId::Bair, 1.0, clean_script, true),
                live_aerial(ch, MoveId::Bair, -1.0, clean_script, true),
            ) {
                f.check(
                    (a.vel.x + b.vel.x).abs() < 1e-3 && (a.vel.y - b.vel.y).abs() < 1e-3,
                    format!(
                        "{ch:?} clean bair: magnitude/vertical must be facing-symmetric, got {:?} vs {:?}",
                        a.vel, b.vel
                    ),
                );
            }
        }
    }
    f.finish("Bair rearward matrix");
}

#[test]
fn fix5_forward_up_and_down_aerials_are_untouched_controls() {
    // Real connections for fair, uair and dair on all three characters and
    // both facings: none of them is rearward, and none of them may drift
    // because ThrowB/Bair were changed.
    let mut f = Failures::default();
    for ch in CAST {
        for &facing in &[1.0f32, -1.0] {
            for id in [MoveId::Fair, MoveId::Uair, MoveId::Dair] {
                let md = attacks::data(ch, id);
                let mut landed = None;
                'find: for reach_step in 0..8u32 {
                    let reach = md.hitbox.offset.x.abs().max(6.0) + reach_step as f32 * 2.0;
                    for attack_tick in 3..24u32 {
                        let script = AerialScript {
                            reach,
                            attack_tick,
                            arrive_at: None,
                        };
                        if let Some(l) = live_aerial(ch, id, facing, script, false) {
                            landed = Some((l, script));
                            break 'find;
                        }
                    }
                }
                let Some((l, script)) = landed else {
                    f.check(
                        false,
                        format!("{ch:?} {id:?} facing {facing:+}: control never connected"),
                    );
                    continue;
                };
                let ok = match id {
                    // Forward air sends the victim forward.
                    MoveId::Fair => l.vel.x.signum() == facing,
                    // Up air sends them up, not sideways.
                    MoveId::Uair => l.vel.y > l.vel.x.abs(),
                    // Down air spikes: downward, and never rearward.
                    MoveId::Dair => l.vel.y < 0.0,
                    _ => unreachable!(),
                };
                f.check(
                    ok,
                    format!(
                        "{ch:?} {id:?} facing {facing:+} {script:?}: control changed direction, got vel=({:+.3},{:+.3})",
                        l.vel.x, l.vel.y
                    ),
                );
                println!(
                    "[MATRIX] {ch:?} facing {facing:+} {id:?} control frame {} vel=({:+.3},{:+.3}) dmg={}",
                    l.attacker_frame, l.vel.x, l.vel.y, l.damage
                );
            }
        }
    }
    f.finish("forward/up/down aerial controls");
}

#[test]
fn fix5_a_rearward_launch_still_takes_di() {
    // DI is applied by the victim on the last hitlag frame, after the
    // launch direction is decided, so it must still bend a back throw.
    let mut neutral_di = None;
    let mut steered = None;
    for (label, di) in [("neutral", 0.0f32), ("up", 1.0)] {
        let mut gs = duel(14.0, 1.0);
        let mut held = false;
        for i in 0..20u32 {
            let p0 = if i == 0 {
                press(buttons::GRAB)
            } else {
                neutral()
            };
            gs.step(&[p0, neutral()]);
            if matches!(gs.fighters[1].state, State::Grabbed) {
                held = true;
                break;
            }
        }
        assert!(held, "{label}: fixture never grabbed");
        // Stick back = ThrowB for a fighter facing +1.
        gs.step(&[stick_press(-1.0, 0.0, 0), neutral()]);
        assert!(
            matches!(gs.fighters[0].state, State::Throw { id: MoveId::ThrowB }),
            "{label}: fixture did not select ThrowB"
        );
        let mut launch = None;
        for _ in 0..40u32 {
            let victim_di = PlayerInput {
                stick: Vec2::new(0.0, di),
                ..Default::default()
            };
            gs.step(&[neutral(), victim_di]);
            if matches!(gs.fighters[1].state, State::Hitstun { .. }) && gs.fighters[1].hitlag == 0 {
                launch = Some(gs.fighters[1].vel);
                break;
            }
        }
        let v = launch.unwrap_or_else(|| panic!("{label}: the throw never released"));
        assert_eq!(
            v.x.signum(),
            -1.0,
            "{label}: DI must not undo the rearward direction ({v:?})"
        );
        if label == "neutral" {
            neutral_di = Some(v);
        } else {
            steered = Some(v);
        }
    }
    let n = neutral_di.unwrap();
    let s = steered.unwrap();
    assert!(
        (n.y - s.y).abs() > 1e-3 || (n.x - s.x).abs() > 1e-3,
        "DI must still change a rearward launch: neutral {n:?} vs steered {s:?}"
    );
}
