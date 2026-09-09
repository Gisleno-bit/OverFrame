//! Grab and ledge contract (see `docs/GAME_FEEL.md` §1.2 / §3.5): standing
//! and dash grab timings, the `76 + 1.6 × percent` hold with mash-out,
//! pummels, throws by stick direction, grab release; the 7-frame ledge
//! catch with 37 frames of intangibility that survives a ledge drop, fresh
//! and tired getup options with their durations and intangibility, the
//! committed ledge jump, the ledge attack's hit frame, and the hang limit.

use overframe::sim::constants as k;
use overframe::sim::fighter::{LedgeKind, State};
use overframe::sim::math::Vec2;
use overframe::sim::{buttons, GameState, MatchConfig, PlayerInput};

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

/// Two Kestrels face to face, `gap` apart, settled on the main platform.
fn duel(gap: f32) -> GameState {
    let mut gs = GameState::new(2, MatchConfig::default());
    for _ in 0..90 {
        gs.step(&[neutral(), neutral()]);
    }
    gs.fighters[0].pos = Vec2::new(-gap * 0.5, 0.0);
    gs.fighters[0].facing = 1.0;
    gs.fighters[1].pos = Vec2::new(gap * 0.5, 0.0);
    gs.fighters[1].facing = -1.0;
    gs.fighters[0].intangible = 0;
    gs.fighters[1].intangible = 0;
    gs
}

/// Step with P0 = `p0(frame)`, P1 neutral, until `pred` holds; returns the
/// 1-based frame count.
fn until(
    gs: &mut GameState,
    max: u32,
    mut p0: impl FnMut(u32) -> PlayerInput,
    mut p1: impl FnMut(u32) -> PlayerInput,
    pred: impl Fn(&GameState) -> bool,
) -> Option<u32> {
    for i in 0..max {
        gs.step(&[p0(i), p1(i)]);
        if pred(gs) {
            return Some(i + 1);
        }
    }
    None
}

fn held(gs: &GameState) -> bool {
    matches!(gs.fighters[1].state, State::Grabbed)
}

// ------------------------------------------------------------ grabs

#[test]
fn standing_grab_catches_on_frame_7_and_lasts_30() {
    let mut gs = duel(14.0);
    let f = until(
        &mut gs,
        20,
        |i| {
            if i == 0 {
                press(buttons::GRAB)
            } else {
                neutral()
            }
        },
        |_| neutral(),
        held,
    )
    .expect("grab connects");
    assert_eq!(f, k::GRAB_STAND.0, "standing grab hits on frame 7");

    // A whiffed standing grab: 30 frames before acting again.
    let mut gs = duel(200.0);
    let f = until(
        &mut gs,
        60,
        |i| {
            if i == 0 {
                press(buttons::GRAB)
            } else {
                neutral()
            }
        },
        |_| neutral(),
        |g| matches!(g.fighters[0].state, State::Stand),
    )
    .unwrap();
    assert_eq!(f, k::GRAB_STAND.2 + 1, "standing grab is 30 frames long");
}

#[test]
fn dash_grab_is_slower_longer_and_slides() {
    // Dash for 6 frames, then grab: hits on frame 12 of the grab and takes
    // 40 frames; the fighter keeps sliding forward while it comes out.
    let mut gs = duel(60.0);
    let f = until(
        &mut gs,
        40,
        |i| {
            if i < 6 {
                stick(1.0, 0.0)
            } else if i == 6 {
                press(buttons::GRAB)
            } else {
                neutral()
            }
        },
        |_| neutral(),
        held,
    )
    .expect("dash grab connects");
    assert_eq!(f - 6, k::GRAB_DASH.0, "dash grab hits on its frame 12");
    assert!(gs.fighters[0].grab_dash);

    let mut gs = duel(300.0);
    let x0 = gs.fighters[0].pos.x;
    let f = until(
        &mut gs,
        80,
        |i| {
            if i < 6 {
                stick(1.0, 0.0)
            } else if i == 6 {
                press(buttons::GRAB)
            } else {
                neutral()
            }
        },
        |_| neutral(),
        |g| g.fighters[0].state_frame == 8 && matches!(g.fighters[0].state, State::Grab),
    )
    .unwrap();
    let _ = f;
    let during = gs.fighters[0].pos.x;
    assert!(during > x0 + 20.0, "a dash grab slides forward");
    let f = until(
        &mut gs,
        80,
        |_| neutral(),
        |_| neutral(),
        |g| matches!(g.fighters[0].state, State::Stand),
    )
    .unwrap();
    assert_eq!(f + 8, k::GRAB_DASH.2 + 1, "dash grab is 40 frames long");
}

#[test]
fn hold_lasts_76_plus_1_6_per_percent_and_mashing_shortens_it() {
    // At 50 % the hold is floor(76 + 80) = 156 frames without mashing.
    let mut gs = duel(14.0);
    gs.fighters[1].percent = 50.0;
    until(
        &mut gs,
        20,
        |i| {
            if i == 0 {
                press(buttons::GRAB)
            } else {
                neutral()
            }
        },
        |_| neutral(),
        held,
    )
    .unwrap();
    assert_eq!(gs.fighters[0].grab_timer, 156);
    let f = until(&mut gs, 400, |_| neutral(), |_| neutral(), |g| !held(g)).unwrap();
    assert_eq!(f, 156, "the hold runs out after 76 + 1.6 × 50 frames");
    // Release: the victim is shoved away with release lag, so is the holder.
    assert!(matches!(
        gs.fighters[1].state,
        State::LandLag {
            total: k::GRAB_RELEASE_LAG
        }
    ));
    assert!(matches!(
        gs.fighters[0].state,
        State::LandLag {
            total: k::GRAB_RELEASE_LAG
        }
    ));
    assert!(
        gs.fighters[1].vel.x > 0.0,
        "victim pushed away from the holder"
    );

    // Mashing: alternate buttons every frame → 6 frames off per input.
    let mut gs = duel(14.0);
    gs.fighters[1].percent = 50.0;
    until(
        &mut gs,
        20,
        |i| {
            if i == 0 {
                press(buttons::GRAB)
            } else {
                neutral()
            }
        },
        |_| neutral(),
        held,
    )
    .unwrap();
    let f = until(
        &mut gs,
        400,
        |_| neutral(),
        |i| {
            if i % 2 == 0 {
                press(buttons::JUMP)
            } else {
                press(buttons::ATTACK)
            }
        },
        |g| !held(g),
    )
    .unwrap();
    // Each frame is one fresh input (−6) plus the tick (−1): ≈ 156 / 7.
    assert!(
        (20..=24).contains(&f),
        "mashing every frame breaks out in ~22 frames, got {f}"
    );
}

#[test]
fn pummel_damages_on_a_cooldown_and_the_stick_throws() {
    let mut gs = duel(14.0);
    until(
        &mut gs,
        20,
        |i| {
            if i == 0 {
                press(buttons::GRAB)
            } else {
                neutral()
            }
        },
        |_| neutral(),
        held,
    )
    .unwrap();
    // Attack while holding pummels (never throws); mashing it respects the
    // cooldown: 3 presses within 20 frames = one pummel.
    for i in 0..20u32 {
        let p0 = if i % 6 == 0 {
            press(buttons::ATTACK)
        } else {
            neutral()
        };
        gs.step(&[p0, neutral()]);
    }
    assert!(held(&gs), "attack pummels instead of throwing");
    assert_eq!(gs.fighters[1].percent, k::PUMMEL_DAMAGE);
    // After the cooldown a second pummel lands.
    for i in 0..25u32 {
        let p0 = if i == 2 {
            press(buttons::ATTACK)
        } else {
            neutral()
        };
        gs.step(&[p0, neutral()]);
    }
    assert_eq!(gs.fighters[1].percent, 2.0 * k::PUMMEL_DAMAGE);

    // Holding the stick up throws up.
    gs.step(&[stick(0.0, 1.0), neutral()]);
    assert!(matches!(
        gs.fighters[0].state,
        State::Throw {
            id: overframe::sim::attacks::MoveId::ThrowU
        }
    ));
}

// ------------------------------------------------------------ ledges

/// One Kestrel falling just outside the right ledge, about to catch it.
fn at_ledge(percent: f32) -> GameState {
    let mut gs = GameState::new(1, MatchConfig::default());
    for _ in 0..90 {
        gs.step(&[neutral()]);
    }
    let m = gs.stage.main();
    let f = &mut gs.fighters[0];
    f.percent = percent;
    f.intangible = 0;
    f.grounded = false;
    f.state = State::Air;
    f.state_frame = 0;
    f.pos = Vec2::new(m.right + f.character.half_width + 4.0, m.y - 4.0);
    f.vel = Vec2::new(0.0, -1.0);
    gs.step(&[neutral()]);
    assert!(
        matches!(gs.fighters[0].state, State::LedgeGrab),
        "caught the ledge"
    );
    gs
}

#[test]
fn ledge_catch_is_7_uncontrollable_frames_with_37_intangible() {
    let mut gs = at_ledge(0.0);
    assert_eq!(gs.fighters[0].intangible, k::LEDGE_INTANGIBLE);
    // Holding shield during the catch does nothing until frame 8.
    let mut f = 0;
    for i in 0..30u32 {
        gs.step(&[if i % 2 == 0 {
            press(buttons::SHIELD)
        } else {
            neutral()
        }]);
        if !matches!(gs.fighters[0].state, State::LedgeGrab) {
            f = i + 1;
            break;
        }
    }
    assert!(
        f == k::LEDGE_CATCH || f == k::LEDGE_CATCH + 1,
        "first actionable frame after the catch (got {f})"
    );
    assert!(matches!(
        gs.fighters[0].state,
        State::LedgeAction {
            kind: LedgeKind::Roll
        }
    ));
    assert_eq!(gs.fighters[0].facing, -1.0, "hangs facing the stage");
}

#[test]
fn ledge_drop_keeps_the_intangibility() {
    let mut gs = at_ledge(0.0);
    for _ in 0..k::LEDGE_CATCH {
        gs.step(&[neutral()]);
    }
    let before = gs.fighters[0].intangible;
    gs.step(&[stick(0.0, -1.0)]);
    assert!(matches!(gs.fighters[0].state, State::Air));
    assert_eq!(gs.fighters[0].intangible, before - 1, "ledgedash-able");
}

#[test]
fn getup_is_fast_and_safe_below_100_and_slow_above() {
    for (tired, pct) in [(0usize, 0.0f32), (1, 100.0)] {
        let mut gs = at_ledge(pct);
        for _ in 0..k::LEDGE_CATCH {
            gs.step(&[neutral()]);
        }
        gs.step(&[stick(-1.0, 0.0)]); // toward the stage (facing is -1)
        assert!(matches!(
            gs.fighters[0].state,
            State::LedgeAction {
                kind: LedgeKind::Getup
            }
        ));
        let (total, inv) = k::LEDGE_GETUP[tired];
        assert_eq!(
            gs.fighters[0].intangible,
            inv.max(k::LEDGE_INTANGIBLE - k::LEDGE_CATCH - 1)
        );
        let mut n = 1;
        while matches!(gs.fighters[0].state, State::LedgeAction { .. }) {
            gs.step(&[neutral()]);
            n += 1;
            assert!(n < 200);
        }
        assert_eq!(n, total + 1, "getup takes {total} frames (tired={tired})");
        assert!(gs.fighters[0].grounded && matches!(gs.fighters[0].state, State::Stand));
        let m = gs.stage.main();
        assert!(gs.fighters[0].pos.x < m.right, "ended on the stage");
    }
}

#[test]
fn ledge_roll_and_attack_follow_their_timelines() {
    // Roll: fresh 49 frames, 30 intangible; ends well onto the stage.
    let mut gs = at_ledge(0.0);
    for _ in 0..k::LEDGE_CATCH {
        gs.step(&[neutral()]);
    }
    gs.step(&[press(buttons::SHIELD)]); // shield = ledge roll
    assert!(matches!(
        gs.fighters[0].state,
        State::LedgeAction {
            kind: LedgeKind::Roll
        }
    ));
    let mut n = 1;
    while matches!(gs.fighters[0].state, State::LedgeAction { .. }) {
        gs.step(&[neutral()]);
        n += 1;
    }
    assert_eq!(n, k::LEDGE_ROLL[0].0 + 1);
    let m = gs.stage.main();
    assert!(gs.fighters[0].pos.x < m.right - 30.0, "rolled inward");

    // Attack: the tilt's hitbox is out on frames 24–26 (fresh) / 42–44
    // (tired), and never before.
    for (tired, pct) in [(0usize, 0.0f32), (1, 100.0)] {
        let mut gs = at_ledge(pct);
        for _ in 0..k::LEDGE_CATCH {
            gs.step(&[neutral()]);
        }
        gs.step(&[press(buttons::ATTACK)]);
        let (total, _, hit) = k::LEDGE_ATTACK[tired];
        let mut active = vec![];
        let mut n = 1;
        while matches!(gs.fighters[0].state, State::LedgeAction { .. }) {
            if let Some((hb, _)) = gs.fighters[0].active_hitbox() {
                active.push(gs.fighters[0].state_frame);
                assert_eq!(hb.damage, k::LEDGE_ATTACK_DAMAGE[tired], "fixed power");
            }
            gs.step(&[neutral()]);
            n += 1;
        }
        assert_eq!(n, total + 1, "ledge attack length (tired={tired})");
        assert_eq!(
            active,
            (hit..hit + k::LEDGE_ATTACK_ACTIVE).collect::<Vec<_>>(),
            "hit frames (tired={tired})"
        );
    }
}

#[test]
fn ledge_jump_is_committed() {
    let mut gs = at_ledge(0.0);
    for _ in 0..k::LEDGE_CATCH {
        gs.step(&[neutral()]);
    }
    gs.step(&[press(buttons::JUMP)]);
    assert!(matches!(
        gs.fighters[0].state,
        State::LedgeAction {
            kind: LedgeKind::Jump
        }
    ));
    let y0 = gs.fighters[0].pos.y;
    // Still on the ledge for the wind-up...
    for _ in 0..k::LEDGE_JUMP.0 - 1 {
        gs.step(&[neutral()]);
    }
    assert!(gs.fighters[0].ledge.is_some());
    assert_eq!(gs.fighters[0].pos.y, y0);
    // ...then airborne and rising, but attacks are ignored until it ends.
    for _ in 0..=k::LEDGE_JUMP.1 {
        gs.step(&[stick_press(0.0, 0.0, buttons::ATTACK)]);
        assert!(!matches!(gs.fighters[0].state, State::Attack { .. }));
    }
    assert!(gs.fighters[0].pos.y > y0 + 20.0, "left the ledge upward");
    assert!(matches!(gs.fighters[0].state, State::Air));
}

#[test]
fn hang_time_is_11s_fresh_and_8s_tired() {
    for (tired, pct) in [(0usize, 0.0f32), (1, 100.0)] {
        let mut gs = at_ledge(pct);
        let mut n = 0;
        while matches!(gs.fighters[0].state, State::LedgeGrab) {
            gs.step(&[neutral()]);
            n += 1;
            assert!(n < 1000);
        }
        assert_eq!(n, k::LEDGE_HANG[tired], "hang limit (tired={tired})");
        assert!(matches!(gs.fighters[0].state, State::Air));
    }
}

#[test]
fn helpless_fighters_can_still_catch_the_ledge() {
    let mut gs = GameState::new(1, MatchConfig::default());
    for _ in 0..90 {
        gs.step(&[neutral()]);
    }
    let m = gs.stage.main();
    let f = &mut gs.fighters[0];
    f.grounded = false;
    f.state = State::Helpless;
    f.state_frame = 0;
    f.pos = Vec2::new(m.right + f.character.half_width + 4.0, m.y - 4.0);
    f.vel = Vec2::new(0.0, -1.0);
    gs.step(&[neutral()]);
    assert!(matches!(gs.fighters[0].state, State::LedgeGrab));
}
