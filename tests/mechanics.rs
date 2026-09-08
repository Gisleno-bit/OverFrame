//! Property tests for the movement/physics that make the game *feel* right.
//!
//! These don't check exact numbers (those are tuning); they check the
//! *relationships* a platform-fighter player relies on: a full hop is higher
//! than a short hop, fast-falling is faster, dashing outruns walking, a
//! diagonal air-dodge into the ground slides (wavedash), L-cancel cuts landing
//! lag, knockback grows with percent, and the whole thing is deterministic.

use overframe::sim::buttons;
use overframe::sim::fighter::State;
use overframe::sim::math::Vec2;
use overframe::sim::{GameState, MatchConfig, PlayerInput};

fn neutral() -> PlayerInput {
    PlayerInput::default()
}

fn press(buttons: u16) -> PlayerInput {
    PlayerInput {
        buttons,
        ..Default::default()
    }
}

fn stick(x: f32, y: f32) -> PlayerInput {
    PlayerInput {
        stick: Vec2::new(x, y),
        ..Default::default()
    }
}

fn stick_press(x: f32, y: f32, buttons: u16) -> PlayerInput {
    PlayerInput {
        stick: Vec2::new(x, y),
        buttons,
        ..Default::default()
    }
}

/// Run `frames` ticks feeding player 0 the input produced by `f(frame)`.
fn drive<F: FnMut(u64) -> PlayerInput>(gs: &mut GameState, frames: u64, mut f: F) {
    for i in 0..frames {
        let inp = f(i);
        gs.step(&[inp]);
    }
}

fn settle(gs: &mut GameState) {
    // Let the fighter fall and land on the main platform.
    drive(gs, 90, |_| neutral());
    assert!(gs.fighters[0].grounded, "fighter should have landed");
}

fn one_player() -> GameState {
    GameState::new(1, MatchConfig::default())
}

#[test]
fn falls_and_lands() {
    let mut gs = one_player();
    settle(&mut gs);
    assert!((gs.fighters[0].pos.y - 0.0).abs() < 2.0);
}

#[test]
fn full_hop_is_higher_than_short_hop() {
    // Short hop: tap jump for a single frame.
    let mut gs = one_player();
    settle(&mut gs);
    let base = gs.fighters[0].pos.y;
    let mut short_peak = base;
    drive(&mut gs, 60, |i| {
        if i == 0 {
            press(buttons::JUMP)
        } else {
            neutral()
        }
    });
    // record peak during flight
    let mut gs2 = one_player();
    settle(&mut gs2);
    let base2 = gs2.fighters[0].pos.y;
    let mut full_peak = base2;
    // Full hop: hold jump through the whole jumpsquat.
    for i in 0..60u64 {
        let inp = if i < 6 {
            press(buttons::JUMP)
        } else {
            neutral()
        };
        gs2.step(&[inp]);
        full_peak = full_peak.max(gs2.fighters[0].pos.y);
    }
    // redo short hop measuring peak
    let mut gs3 = one_player();
    settle(&mut gs3);
    for i in 0..60u64 {
        let inp = if i == 0 {
            press(buttons::JUMP)
        } else {
            neutral()
        };
        gs3.step(&[inp]);
        short_peak = short_peak.max(gs3.fighters[0].pos.y);
    }
    let _ = base;
    assert!(
        full_peak > short_peak + 5.0,
        "full hop {full_peak} should clear short hop {short_peak}"
    );
    assert!(
        short_peak > base2 + 5.0,
        "short hop should leave the ground"
    );
}

#[test]
fn fast_fall_is_faster() {
    // Full hop, then at apex either drift (control) or fast-fall.
    fn frames_to_land(fast: bool) -> u64 {
        let mut gs = one_player();
        settle(&mut gs);
        let mut apex_passed = false;
        for i in 0..300u64 {
            let inp = if i < 6 {
                press(buttons::JUMP)
            } else if apex_passed && fast {
                stick(0.0, -1.0)
            } else {
                neutral()
            };
            let before = gs.fighters[0].vel.y;
            gs.step(&[inp]);
            if !apex_passed && before > 0.0 && gs.fighters[0].vel.y <= 0.0 {
                apex_passed = true;
            }
            if apex_passed && gs.fighters[0].grounded && i > 10 {
                return i;
            }
        }
        u64::MAX
    }
    let slow = frames_to_land(false);
    let fast = frames_to_land(true);
    assert!(slow != u64::MAX && fast != u64::MAX, "both should land");
    assert!(
        fast < slow,
        "fast-fall ({fast}) should land before drift ({slow})"
    );
}

#[test]
fn dash_outruns_walk() {
    let mut walk = one_player();
    settle(&mut walk);
    drive(&mut walk, 30, |_| stick(0.45, 0.0)); // soft = walk
    let walk_speed = walk.fighters[0].vel.x.abs();

    let mut dash = one_player();
    settle(&mut dash);
    drive(&mut dash, 30, |_| stick(1.0, 0.0)); // hard = dash/run
    let dash_speed = dash.fighters[0].vel.x.abs();

    assert!(
        dash_speed > walk_speed + 0.2,
        "dash {dash_speed} should beat walk {walk_speed}"
    );
}

#[test]
fn wavedash_slides_on_landing() {
    let mut gs = one_player();
    settle(&mut gs);
    let start_x = gs.fighters[0].pos.x;
    let mut saw_waveland = false;
    let mut max_x = start_x;
    // Short hop, then air-dodge down-forward into the ground shortly after
    // takeoff — the canonical wavedash.
    for i in 0..60u64 {
        let inp = if i == 0 {
            press(buttons::JUMP)
        } else if i == 5 {
            stick_press(0.9, -0.5, buttons::SHIELD)
        } else {
            neutral()
        };
        gs.step(&[inp]);
        if matches!(gs.fighters[0].state, State::Waveland) {
            saw_waveland = true;
        }
        max_x = max_x.max(gs.fighters[0].pos.x);
    }
    assert!(
        saw_waveland,
        "an angled air-dodge into the ground should waveland"
    );
    assert!(
        max_x > start_x + 12.0,
        "wavedash should slide forward (moved {} units)",
        max_x - start_x
    );
}

#[test]
fn lcancel_reduces_landing_lag() {
    fn landing_lag(lcancel: bool) -> u32 {
        let mut gs = one_player();
        settle(&mut gs);
        // Short hop then immediately an aerial; optionally press shield (L) just
        // before landing to L-cancel.
        for i in 0..80u64 {
            let inp = if i == 0 {
                press(buttons::JUMP)
            } else if i == 3 {
                press(buttons::ATTACK) // nair on takeoff
            } else if i >= 5 {
                // Fast-fall so we land while the aerial is still in lag; press
                // shield within the window to L-cancel in that variant.
                let b = if lcancel { buttons::SHIELD } else { 0 };
                stick_press(0.0, -1.0, b)
            } else {
                neutral()
            };
            gs.step(&[inp]);
            if let State::LandLag { total } = gs.fighters[0].state {
                return total;
            }
        }
        u32::MAX
    }
    let normal = landing_lag(false);
    let cancelled = landing_lag(true);
    assert!(normal != u32::MAX, "should have produced landing lag");
    assert!(cancelled != u32::MAX, "should have produced landing lag");
    assert!(
        cancelled < normal,
        "L-cancel ({cancelled}) should be shorter than normal ({normal})"
    );
}

#[test]
fn knockback_grows_with_percent() {
    use overframe::sim::knockback::{hitstun, knockback};
    let low = knockback(20.0, 12.0, 90.0, 90.0, 30.0);
    let high = knockback(120.0, 12.0, 90.0, 90.0, 30.0);
    assert!(
        high > low * 1.5,
        "knockback should scale strongly with percent"
    );
    assert!(
        hitstun(high) > hitstun(low),
        "hitstun should grow with knockback"
    );
}

#[test]
fn a_clean_hit_deals_damage_and_launches() {
    // Two players next to each other; p0 jabs p1.
    let mut gs = GameState::new(2, MatchConfig::default());
    // settle both
    for _ in 0..90 {
        gs.step(&[neutral(), neutral()]);
    }
    // place them adjacent
    gs.fighters[0].pos = Vec2::new(-8.0, 0.0);
    gs.fighters[0].facing = 1.0;
    gs.fighters[1].pos = Vec2::new(8.0, 0.0);
    gs.fighters[1].facing = -1.0;
    gs.fighters[1].intangible = 0;

    let before = gs.fighters[1].percent;
    // p0 presses A (jab).
    for i in 0..20u64 {
        let p0 = if i == 0 {
            press(buttons::ATTACK)
        } else {
            neutral()
        };
        gs.step(&[p0, neutral()]);
    }
    assert!(
        gs.fighters[1].percent > before,
        "victim should have taken damage"
    );
}

#[test]
fn simulation_is_deterministic() {
    // Same seed + same scripted inputs => identical state.
    let script = |i: u64| -> [PlayerInput; 2] {
        let p0 = match i % 20 {
            0 => press(buttons::JUMP),
            3 => press(buttons::ATTACK),
            7 => stick(1.0, 0.0),
            11 => PlayerInput {
                stick: Vec2::new(0.8, -0.6),
                buttons: buttons::SHIELD,
                ..Default::default()
            },
            _ => neutral(),
        };
        let p1 = match i % 17 {
            0 => stick(-1.0, 0.0),
            5 => press(buttons::JUMP),
            9 => press(buttons::SPECIAL),
            _ => neutral(),
        };
        [p0, p1]
    };

    let run = || {
        let mut gs = GameState::new(2, MatchConfig::default());
        for i in 0..600u64 {
            let inp = script(i);
            gs.step(&inp);
        }
        gs
    };
    let a = run();
    let b = run();
    for i in 0..2 {
        assert_eq!(a.fighters[i].pos, b.fighters[i].pos, "positions must match");
        assert_eq!(a.fighters[i].percent, b.fighters[i].percent);
        assert_eq!(a.fighters[i].stocks, b.fighters[i].stocks);
    }
    assert_eq!(a.frame, b.frame);
}
