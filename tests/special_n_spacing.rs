//! What Kestrel's discharge actually does at three spacings against all
//! three Alpha fighters: hit, block and whiff, measured.
//!
//! This is a **measurement**, not a balance change. Nothing here proposes a
//! number; it reports what the current simulation does so a range or
//! recovery decision can be made against evidence instead of impressions.
//! The presentation batch that ships with it changed none of these values,
//! and the assertions below exist only to catch it if a later batch does.
//!
//! Run the table with:
//! `cargo test --test special_n_spacing -- --ignored --nocapture`

use overframe::sim::attacks::{self, MoveId};
use overframe::sim::constants as k;
use overframe::sim::fighter::State;
use overframe::sim::math::Vec2;
use overframe::sim::roster::CharacterId;
use overframe::sim::{buttons, GameState, MatchConfig, PlayerInput};

/// Gaps between the two fighters, in world units. Close is inside Kestrel's
/// own fsmash reach, far is most of a platform apart.
const SPACINGS: [(&str, f32); 3] = [("close", 30.0), ("medium", 70.0), ("far", 120.0)];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Stance {
    /// Standing, no shield: the shot reaches the body.
    Open,
    /// Holding a settled shield towards the shot.
    Shielding,
    /// Nobody in the way: the shot is fired at an empty lane.
    Away,
}

fn neutral() -> PlayerInput {
    PlayerInput::default()
}
fn press(b: u16) -> PlayerInput {
    PlayerInput {
        buttons: b,
        ..Default::default()
    }
}

#[derive(Debug, Clone)]
struct Outcome {
    /// Tick the shot resolved on, from the press.
    tick: Option<u32>,
    /// Integrated sweep of the shot from its spawn point to where it
    /// resolved, **including the emission tick's own step**.
    ///
    /// `update_projectiles` advances `pos += vel` *before* it collides, so
    /// a shot has already swept one full `PROJECTILE_SPEED` by the time it
    /// can hit anything: there is no such thing as a 0-unit hit at the
    /// muzzle. Reporting `(tick - fire) * SPEED` would have claimed exactly
    /// that for a collision on the spawn tick, which is why this counts the
    /// step the emission tick itself takes.
    swept: Option<f32>,
    /// The swept segment that actually resolved, `(from, to)` in world x —
    /// the same `prev -> pos` segment the collision test uses. Read off the
    /// live projectile when one survives the tick; `None` when it was spent
    /// on the very tick it spawned and never appeared in the list.
    segment: Option<(f32, f32)>,
    /// True when the shot was consumed on the same tick it was emitted.
    same_tick_as_spawn: bool,
    percent_delta: f32,
    shield_delta: f32,
    defender_state: String,
    defender_hitlag: u32,
    defender_vel: Vec2,
    /// Frames of Kestrel's own commitment still left when it resolved.
    lockout_left: Option<u32>,
    /// Ticks until Kestrel could act again at all.
    total_lockout: u32,
    /// Most shots alive at once during the whole run: one shot, never two.
    max_live: usize,
    /// Shots still alive on the tick it resolved: a spent shot leaves none.
    live_at_resolution: usize,
    projectile_expired: bool,
}

/// Hold shield for this many ticks before the shot is fired, so a blocking
/// defender is settled well outside the powershield window.
const SETTLE: u32 = 10;

fn run(defender: CharacterId, gap: f32, stance: Stance, facing: f32) -> Outcome {
    let cfg = MatchConfig {
        chars: [
            CharacterId::Kestrel,
            defender,
            CharacterId::Kestrel,
            CharacterId::Kestrel,
        ],
        ..Default::default()
    };
    let mut gs = GameState::new(2, cfg);
    for _ in 0..90 {
        gs.step(&[neutral(), neutral()]);
    }
    // Kestrel on one side, the defender `gap` away on the side it faces.
    gs.fighters[0].pos = Vec2::new(-gap * 0.5 * facing, 0.0);
    gs.fighters[0].facing = facing;
    let d_side = if stance == Stance::Away { -1.0 } else { 1.0 };
    gs.fighters[1].pos = Vec2::new(gs.fighters[0].pos.x + gap * facing * d_side, 0.0);
    gs.fighters[1].facing = -facing * d_side;
    for f in gs.fighters.iter_mut() {
        f.intangible = 0;
    }

    let percent_before = gs.fighters[1].percent;
    let mut shield_before = gs.fighters[1].shield_health;
    let mut out = Outcome {
        tick: None,
        swept: None,
        segment: None,
        same_tick_as_spawn: false,
        percent_delta: 0.0,
        shield_delta: 0.0,
        defender_state: String::new(),
        defender_hitlag: 0,
        defender_vel: Vec2::ZERO,
        lockout_left: None,
        total_lockout: 0,
        max_live: 0,
        live_at_resolution: 0,
        projectile_expired: false,
    };
    let md = attacks::data(CharacterId::Kestrel, MoveId::SpecialN);
    let fire_at = if stance == Stance::Shielding {
        SETTLE
    } else {
        0
    };
    let mut spawn_x = None;
    for tick in 0..(fire_at + 200) {
        let p0 = if tick == fire_at {
            press(buttons::SPECIAL)
        } else {
            neutral()
        };
        let p1 = if stance == Stance::Shielding {
            press(buttons::SHIELD)
        } else {
            neutral()
        };
        if stance == Stance::Shielding {
            // Keep the shield from decaying into a break over a long whiff
            // run: this measures the block, not the drain.
            gs.fighters[1].shield_health = gs.fighters[1].shield_health.max(40.0);
            shield_before = gs.fighters[1].shield_health;
        }
        let before_pct = gs.fighters[1].percent;
        let before_shield = gs.fighters[1].shield_health;
        let alive_before = gs.projectiles.len();
        // The segment the collision test will use, captured before the
        // step: `pos` is where the shot starts this tick and `pos + vel` is
        // where it ends, which is exactly the swept segment
        // `update_projectiles` tests (it advances first, then collides).
        let before_list: Vec<(usize, u32, f32, f32)> = gs
            .projectiles
            .iter()
            .filter(|p| p.active)
            .map(|p| (p.owner, p.life, p.pos.x, p.vel.x))
            .collect();
        gs.step(&[p0, p1]);
        out.max_live = out.max_live.max(gs.projectiles.len());
        if spawn_x.is_none() {
            if let Some(p) = gs.projectiles.first() {
                spawn_x = Some(p.pos.x - p.vel.x);
            }
        }
        // Still committed?
        if matches!(
            gs.fighters[0].state,
            State::Attack {
                id: MoveId::SpecialN,
                ..
            }
        ) {
            out.total_lockout = tick - fire_at + 1;
        }
        let hit = gs.fighters[1].percent > before_pct;
        let blocked = before_shield - gs.fighters[1].shield_health > k::SHIELD_DECAY + 1e-4;
        if (hit || blocked) && out.tick.is_none() {
            out.tick = Some(tick - fire_at);
            out.percent_delta = gs.fighters[1].percent - percent_before;
            out.shield_delta = shield_before - gs.fighters[1].shield_health;
            out.defender_state = format!("{:?}", gs.fighters[1].state);
            out.defender_hitlag = gs.fighters[1].hitlag;
            out.defender_vel = gs.fighters[1].vel;
            out.live_at_resolution = gs.projectiles.len();
            out.lockout_left = match gs.fighters[0].state {
                State::Attack {
                    id: MoveId::SpecialN,
                    ..
                } => Some(md.total().saturating_sub(gs.fighters[0].state_frame)),
                _ => Some(0),
            };
            // The sweep, counted from the spawn point and including the
            // emission tick's own step (see `swept`). When a live
            // projectile is still in the list its own `prev -> pos` segment
            // is recorded too; when the shot was spent on the tick it
            // spawned there is no such object left to read, and the row
            // says so rather than inventing a position.
            out.same_tick_as_spawn = tick == fire_at;
            out.swept = Some(((tick - fire_at + 1) as f32) * attacks::PROJECTILE_SPEED);
            // The shot that resolved is the one that is gone from the list:
            // matched by owner and by the one tick of life it loses, the
            // same way the boundary suite matches them. Its segment is the
            // one captured above. A shot spent on the tick it spawned was
            // never in that list, so there is no segment to report and the
            // row says so.
            let survivors: Vec<(usize, u32)> =
                gs.projectiles.iter().map(|p| (p.owner, p.life)).collect();
            out.segment = before_list
                .iter()
                .find(|(o, l, _, _)| !survivors.iter().any(|(so, sl)| so == o && sl + 1 == *l))
                .map(|(_, _, x, vx)| (*x, *x + *vx));
        }
        if out.tick.is_none() && alive_before > 0 && gs.projectiles.is_empty() {
            out.projectile_expired = true;
            out.swept = Some(attacks::PROJECTILE_SPEED * attacks::PROJECTILE_LIFE as f32);
            break;
        }
        if out.tick.is_some() && !matches!(gs.fighters[0].state, State::Attack { .. }) {
            break;
        }
    }
    out
}

/// The table the range/recovery decision should be made against.
#[test]
#[ignore]
fn diag_hit_block_and_whiff_at_three_spacings() {
    let md = attacks::data(CharacterId::Kestrel, MoveId::SpecialN);
    println!(
        "kestrel special_n: commitment {} ticks (startup {} active {} endlag {}), projectile speed {} damage {} radius {} life {}",
        md.total(),
        md.startup,
        md.active,
        md.endlag,
        attacks::PROJECTILE_SPEED,
        attacks::PROJECTILE_DAMAGE,
        attacks::PROJECTILE_RADIUS,
        attacks::PROJECTILE_LIFE
    );
    println!(
        "swept = integrated travel from the spawn point, counting the emission tick's own step: `update_projectiles` advances pos += vel BEFORE it collides, so the first tick already sweeps {:.0} units and there is no 0-unit hit at the muzzle. @spwn = the shot was consumed on the very tick it was emitted, so no live projectile was left to read a segment from.",
        attacks::PROJECTILE_SPEED
    );
    println!(
        "{:<8} {:<8} {:<10} {:>5} {:>8} {:>5} {:>8} {:>8} {:>6} {:>7} {:>22} {:>5}",
        "defender",
        "spacing",
        "stance",
        "tick",
        "swept",
        "@spwn",
        "d%",
        "dshield",
        "hitlag",
        "left",
        "state",
        "live"
    );
    for ch in CharacterId::ALL {
        for (name, gap) in SPACINGS {
            for stance in [Stance::Open, Stance::Shielding, Stance::Away] {
                // Both facings produce identical numbers; the mirror is
                // asserted separately below, so the table stays readable.
                let o = run(ch, gap, stance, 1.0);
                println!(
                    "{:<8} {:<8} {:<10} {:>5} {:>8} {:>5} {:>8} {:>8} {:>6} {:>7} {:>22} {:>5}",
                    format!("{ch:?}"),
                    name,
                    format!("{stance:?}"),
                    o.tick.map(|t| t.to_string()).unwrap_or_else(|| "-".into()),
                    o.swept
                        .map(|f| format!("{f:.1}"))
                        .unwrap_or_else(|| "-".into()),
                    if o.same_tick_as_spawn { "yes" } else { "no" },
                    format!("{:.0}", o.percent_delta),
                    format!("{:.2}", o.shield_delta),
                    o.defender_hitlag,
                    o.lockout_left
                        .map(|l| l.to_string())
                        .unwrap_or_else(|| "-".into()),
                    if o.defender_state.is_empty() {
                        "-".to_string()
                    } else {
                        o.defender_state.clone()
                    },
                    o.max_live
                );
            }
        }
    }
}

/// The measurement above must keep meaning the same thing: the shot does
/// the damage the table says, to every fighter, at every spacing, and the
/// commitment is the one the move table declares. No balance was changed by
/// the presentation batch, and this fails loudly if a later one changes it
/// without saying so.
#[test]
fn the_discharge_does_the_same_thing_at_every_spacing_and_to_everyone() {
    let md = attacks::data(CharacterId::Kestrel, MoveId::SpecialN);
    for ch in CharacterId::ALL {
        for (name, gap) in SPACINGS {
            for facing in [1.0f32, -1.0] {
                let hit = run(ch, gap, Stance::Open, facing);
                assert_eq!(
                    hit.percent_delta,
                    attacks::PROJECTILE_DAMAGE,
                    "{ch:?} {name} facing {facing:+}: the shot does its table damage, got {hit:?}"
                );
                assert!(
                    hit.max_live <= 1 && hit.live_at_resolution == 0,
                    "{ch:?} {name} facing {facing:+}: one shot, spent on the target, got {hit:?}"
                );
                assert!(
                    hit.defender_state.starts_with("Hitstun"),
                    "{ch:?} {name} facing {facing:+}: an open defender is hit, got {}",
                    hit.defender_state
                );

                let block = run(ch, gap, Stance::Shielding, facing);
                let want_cost = attacks::PROJECTILE_DAMAGE * k::SHIELD_DAMAGE_MULT
                    + k::SHIELD_DECAY
                    - k::SHIELD_REGEN;
                assert!(
                    (block.shield_delta - want_cost).abs() < 1e-3,
                    "{ch:?} {name} facing {facing:+}: a block costs exactly {want_cost:.3}, got {:.3}",
                    block.shield_delta
                );
                assert_eq!(
                    block.percent_delta, 0.0,
                    "{ch:?} {name} facing {facing:+}: a block takes no damage"
                );
                assert!(
                    block
                        .defender_state
                        .starts_with(&format!("ShieldStun {{ total: {}", knock_stun())),
                    "{ch:?} {name} facing {facing:+}: the block's stun is the table's, got {}",
                    block.defender_state
                );

                let whiff = run(ch, gap, Stance::Away, facing);
                assert_eq!(
                    whiff.percent_delta, 0.0,
                    "{ch:?} {name} facing {facing:+}: a shot into an empty lane touches nobody"
                );
                assert!(
                    whiff.projectile_expired,
                    "{ch:?} {name} facing {facing:+}: the shot lives out its own life, got {whiff:?}"
                );
                // The sweep is never reported as zero: the emission tick
                // itself advances the shot a full step before it can
                // collide with anything.
                assert!(
                    hit.swept.unwrap_or(0.0) >= attacks::PROJECTILE_SPEED - 1e-3,
                    "{ch:?} {name} facing {facing:+}: the first tick already sweeps a step, got {:?}",
                    hit.swept
                );
                // A live projectile's own segment is the one the collision
                // test uses; a shot spent on its spawn tick has none, and
                // says so instead of pretending to a position.
                if let Some((from, to)) = hit.segment {
                    assert!(
                        ((to - from).abs() - attacks::PROJECTILE_SPEED).abs() < 1e-3,
                        "{ch:?} {name} facing {facing:+}: the recorded segment is one tick of travel, got {from} -> {to}"
                    );
                }
                assert_eq!(
                    hit.same_tick_as_spawn,
                    hit.segment.is_none(),
                    "{ch:?} {name} facing {facing:+}: only a shot spent on its spawn tick has no segment to record"
                );
                assert_eq!(
                    whiff.total_lockout,
                    md.total(),
                    "{ch:?} {name} facing {facing:+}: Kestrel is committed for the table's {} ticks whatever happens downrange",
                    md.total()
                );
            }
        }
    }
}

fn knock_stun() -> u32 {
    overframe::sim::knockback::shieldstun(attacks::PROJECTILE_DAMAGE)
}
