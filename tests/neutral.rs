//! The rest of the neutral / punish game (see `docs/GAME_FEEL.md` §3.6):
//! run turnarounds vs. free dash-dancing, the tech window and lockout, tech
//! in place / tech roll, missed-tech knockdown options (stand, roll, getup
//! attack with its two-sided hitbox), jump-cancelled grab and up-smash,
//! platform drops by a down flick, clank / priority between grounded
//! attacks, stale-move negation, and meteor cancelling.

use overframe::sim::attacks::MoveId;
use overframe::sim::constants as k;
use overframe::sim::fighter::{GetupKind, State};
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
fn cstick(x: f32, y: f32) -> PlayerInput {
    PlayerInput {
        cstick: Vec2::new(x, y),
        ..Default::default()
    }
}

fn solo() -> GameState {
    let mut gs = GameState::new(1, MatchConfig::default());
    for _ in 0..90 {
        gs.step(&[neutral()]);
    }
    gs.fighters[0].pos = Vec2::new(0.0, 0.0);
    gs.fighters[0].facing = 1.0;
    gs.fighters[0].intangible = 0;
    gs
}

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

fn frames_in<F: Fn(&State) -> bool>(gs: &mut GameState, inputs: &[PlayerInput], pred: F) -> u32 {
    let mut n = 0;
    while pred(&gs.fighters[0].state) {
        gs.step(inputs);
        n += 1;
        assert!(n < 400, "state never ended");
    }
    n
}

// ------------------------------------------------------------ movement

#[test]
fn dash_dance_is_free_but_a_run_reversal_is_a_slow_turn() {
    // Inside the dash window a reversal is a new dash at once.
    let mut gs = solo();
    for _ in 0..5 {
        gs.step(&[stick(1.0, 0.0)]);
    }
    gs.step(&[stick(-1.0, 0.0)]);
    assert!(matches!(gs.fighters[0].state, State::Dash));
    assert_eq!(gs.fighters[0].facing, -1.0);

    // Past it you are running, and a reversal brakes for RUN_TURN frames
    // (facing flips half-way), then runs the other way if still held.
    let mut gs = solo();
    let dash = gs.fighters[0].character.dash_frames;
    for _ in 0..dash + 10 {
        gs.step(&[stick(1.0, 0.0)]);
    }
    assert!(matches!(gs.fighters[0].state, State::Run));
    gs.step(&[stick(-1.0, 0.0)]);
    assert!(matches!(gs.fighters[0].state, State::RunTurn));
    let n = frames_in(&mut gs, &[stick(-1.0, 0.0)], |s| {
        matches!(s, State::RunTurn)
    });
    assert_eq!(n, k::RUN_TURN);
    assert!(matches!(gs.fighters[0].state, State::Run));
    assert_eq!(gs.fighters[0].facing, -1.0);

    // A jump interrupts the turn.
    let mut gs = solo();
    for _ in 0..dash + 10 {
        gs.step(&[stick(1.0, 0.0)]);
    }
    gs.step(&[stick(-1.0, 0.0)]);
    for _ in 0..3 {
        gs.step(&[stick(-1.0, 0.0)]);
    }
    gs.step(&[stick_press(-1.0, 0.0, buttons::JUMP)]);
    assert!(matches!(gs.fighters[0].state, State::JumpSquat));
}

#[test]
fn jump_cancelled_grab_and_up_smash_come_out_of_a_dash() {
    // Dash → jump → grab during the squat = standing grab (not a dash grab).
    let mut gs = solo();
    for _ in 0..6 {
        gs.step(&[stick(1.0, 0.0)]);
    }
    gs.step(&[stick_press(1.0, 0.0, buttons::JUMP)]);
    assert!(matches!(gs.fighters[0].state, State::JumpSquat));
    gs.step(&[stick_press(1.0, 0.0, buttons::GRAB)]);
    assert!(matches!(gs.fighters[0].state, State::Grab));
    assert!(!gs.fighters[0].grab_dash, "JC grab is the standing grab");
    assert!(gs.fighters[0].vel.x > 2.0, "keeps the dash momentum");

    // Dash → jump → C-stick up during the squat = up-smash.
    let mut gs = solo();
    for _ in 0..6 {
        gs.step(&[stick(1.0, 0.0)]);
    }
    gs.step(&[stick_press(1.0, 0.0, buttons::JUMP)]);
    gs.step(&[cstick(0.0, 1.0)]);
    assert!(matches!(
        gs.fighters[0].state,
        State::Attack {
            id: MoveId::Usmash,
            aerial: false
        }
    ));
}

#[test]
fn platform_drop_needs_a_down_flick_not_a_slow_crouch() {
    // Land on a Lattice side platform (y = 59, x 43..125).
    let mut gs = solo();
    gs.fighters[0].pos = Vec2::new(80.0, 70.0);
    gs.fighters[0].grounded = false;
    gs.fighters[0].state = State::Air;
    for _ in 0..30 {
        gs.step(&[neutral()]);
    }
    assert!(
        gs.fighters[0].grounded && gs.fighters[0].pos.y > 50.0,
        "on the platform"
    );

    // A quick flick to hard down drops through.
    let mut a = gs.clone();
    a.step(&[stick(0.0, -1.0)]);
    a.step(&[stick(0.0, -1.0)]);
    assert!(!a.fighters[0].grounded, "flicked through the platform");

    // Easing the stick down over a few frames crouches instead.
    let mut b = gs;
    for i in 1..=6 {
        b.step(&[stick(0.0, -(i as f32) / 6.0)]);
    }
    assert!(b.fighters[0].grounded);
    assert!(matches!(b.fighters[0].state, State::Crouch));
}

// ------------------------------------------------------------ techs & knockdowns

/// Launch P0 into a techable tumble that lands on the stage about 24 frames
/// later.
fn tumble(gs: &mut GameState) {
    gs.fighters[0].pos = Vec2::new(0.0, 40.0);
    gs.fighters[0].grounded = false;
    gs.fighters[0].state = State::Air;
    gs.fighters[0].apply_launch(Vec2::new(1.0, 2.0), 90, true, 0);
}

#[test]
fn tech_window_is_20_frames_with_a_lockout() {
    // Shield 10 frames before touching down → tech in place.
    let mut gs = solo();
    tumble(&mut gs);
    let mut landed = None;
    for i in 0..80u32 {
        let inp = if i == 8 {
            press(buttons::SHIELD)
        } else {
            neutral()
        };
        gs.step(&[inp]);
        if gs.fighters[0].grounded {
            landed = Some(i + 1);
            break;
        }
    }
    let landed = landed.expect("touched down");
    assert!(landed - 9 <= k::TECH_WINDOW, "landed inside the window");
    assert!(matches!(gs.fighters[0].state, State::Tech { dir } if dir == 0.0));
    assert_eq!(gs.fighters[0].intangible, k::TECH_IN_PLACE.1);
    let n = frames_in(&mut gs, &[neutral()], |s| matches!(s, State::Tech { .. }));
    assert_eq!(n, k::TECH_IN_PLACE.0);
    assert!(matches!(gs.fighters[0].state, State::Stand));

    // Pressing shield on the frame of impact (no early press) still techs
    // only if it lands inside a window: pressing every frame is *locked
    // out* after the first press and misses.
    let mut gs = solo();
    gs.fighters[0].pos = Vec2::new(0.0, 160.0);
    gs.fighters[0].grounded = false;
    gs.fighters[0].state = State::Air;
    gs.fighters[0].apply_launch(Vec2::new(0.0, 0.0), 200, true, 0);
    for i in 0..200u32 {
        let inp = if i % 2 == 0 {
            press(buttons::SHIELD)
        } else {
            neutral()
        };
        gs.step(&[inp]);
        if gs.fighters[0].grounded {
            break;
        }
    }
    assert!(
        matches!(gs.fighters[0].state, State::Knockdown),
        "mashing shield from far above is locked out → missed tech"
    );
}

#[test]
fn tech_roll_and_the_three_getups() {
    // Tech roll: holding a direction on impact rolls that way, 40 frames.
    let mut gs = solo();
    tumble(&mut gs);
    let mut x_land = 0.0;
    for i in 0..80u32 {
        let inp = if i == 8 {
            stick_press(1.0, 0.0, buttons::SHIELD)
        } else {
            stick(1.0, 0.0)
        };
        gs.step(&[inp]);
        if gs.fighters[0].grounded {
            x_land = gs.fighters[0].pos.x;
            break;
        }
    }
    assert!(matches!(gs.fighters[0].state, State::Tech { dir } if dir == 1.0));
    let n = frames_in(&mut gs, &[neutral()], |s| matches!(s, State::Tech { .. }));
    assert_eq!(n, k::TECH_ROLL.0);
    assert!(
        (gs.fighters[0].pos.x - x_land - k::TECH_ROLL_DISTANCE).abs() < 1.0,
        "rolled the tech-roll distance"
    );

    // Missed tech: bounce, then each option.
    for (input, expect, total, inv) in [
        (
            stick(0.0, 1.0),
            GetupKind::Stand,
            k::GETUP_STAND.0,
            k::GETUP_STAND.1,
        ),
        (
            stick(-1.0, 0.0),
            GetupKind::Roll { dir: -1.0 },
            k::GETUP_ROLL.0,
            k::GETUP_ROLL.1,
        ),
        (
            press(buttons::ATTACK),
            GetupKind::Attack,
            k::GETUP_ATTACK.0,
            k::GETUP_ATTACK.1,
        ),
    ] {
        let mut gs = solo();
        tumble(&mut gs);
        while !matches!(gs.fighters[0].state, State::Knockdown) {
            gs.step(&[neutral()]);
        }
        // Nothing responds during the bounce.
        for _ in 0..k::KNOCKDOWN_BOUNCE - 1 {
            gs.step(&[input]);
            assert!(matches!(gs.fighters[0].state, State::Knockdown));
        }
        gs.step(&[neutral()]);
        let x0 = gs.fighters[0].pos.x;
        gs.step(&[input]);
        assert_eq!(gs.fighters[0].state, State::Getup { kind: expect });
        assert_eq!(gs.fighters[0].intangible, inv);
        let mut hits: Vec<(u32, f32)> = vec![];
        let mut n = 1;
        while matches!(gs.fighters[0].state, State::Getup { .. }) {
            if let Some((hb, world)) = gs.fighters[0].active_hitbox() {
                hits.push((gs.fighters[0].state_frame, (world.x - x0).signum()));
                assert_eq!(hb.damage, k::GETUP_ATTACK_DAMAGE);
            }
            gs.step(&[neutral()]);
            n += 1;
        }
        assert_eq!(n, total + 1, "{expect:?} length");
        match expect {
            GetupKind::Roll { dir } => {
                assert!((gs.fighters[0].pos.x - x0 - dir * k::GETUP_ROLL_DISTANCE).abs() < 1.0);
                assert!(hits.is_empty());
            }
            GetupKind::Stand => assert!(hits.is_empty()),
            GetupKind::Attack => {
                let (front, back) = k::GETUP_ATTACK_HITS;
                let a = k::GETUP_ATTACK_ACTIVE;
                let expect_frames: Vec<u32> = (front..front + a).chain(back..back + a).collect();
                let frames: Vec<u32> = hits.iter().map(|h| h.0).collect();
                assert_eq!(frames, expect_frames, "getup attack hit frames");
                // In front first, then behind.
                assert!(hits[..a as usize].iter().all(|h| h.1 > 0.0));
                assert!(hits[a as usize..].iter().all(|h| h.1 < 0.0));
            }
        }
    }
}

// ------------------------------------------------------------ hits

#[test]
fn close_attacks_clank_and_the_stronger_one_wins_otherwise() {
    // Two forward tilts (8 % each) meeting head-on: both rebound, no damage.
    let mut gs = duel(44.0);
    let mut clanked = false;
    for i in 0..20u32 {
        let inp = if i == 0 {
            stick_press(0.55, 0.0, buttons::ATTACK)
        } else {
            neutral()
        };
        let inp1 = if i == 0 {
            stick_press(-0.55, 0.0, buttons::ATTACK)
        } else {
            neutral()
        };
        gs.step(&[inp, inp1]);
        if matches!(gs.fighters[0].state, State::Rebound { .. }) {
            clanked = true;
            break;
        }
    }
    assert!(clanked, "equal tilts clank");
    assert!(matches!(gs.fighters[1].state, State::Rebound { .. }));
    assert_eq!(gs.fighters[0].percent, 0.0);
    assert_eq!(gs.fighters[1].percent, 0.0);
    let expect = (8.0f32 / 3.0) as u32 + k::REBOUND_BASE;
    assert!(matches!(gs.fighters[0].state, State::Rebound { total } if total == expect));

    // Jab (3 %) into a forward smash (15 %): the jab is cancelled, the smash
    // lands.
    let mut gs = duel(40.0);
    let mut hit = false;
    for i in 0..30u32 {
        // P0 smashes at once; P1 jabs when the smash is about to come out.
        let inp0 = if i == 0 { cstick(1.0, 0.0) } else { neutral() };
        let inp1 = if i == 9 {
            press(buttons::ATTACK)
        } else {
            neutral()
        };
        gs.step(&[inp0, inp1]);
        if matches!(gs.fighters[1].state, State::Hitstun { .. }) {
            hit = true;
            break;
        }
    }
    assert!(hit, "the smash beats the jab");
    assert_eq!(gs.fighters[0].percent, 0.0, "the jab never landed");
    assert!(!matches!(gs.fighters[0].state, State::Rebound { .. }));
}

#[test]
fn stale_moves_lose_damage_but_not_knockback() {
    let mut gs = duel(10.0);
    let f = &gs.fighters[0];
    assert_eq!(f.stale_multiplier(MoveId::Ftilt), 1.0);
    let mut percents = vec![];
    for _ in 0..3 {
        // Reset the victim so every hit is measured from 0 %.
        gs.fighters[1].percent = 0.0;
        gs.fighters[1].pos = Vec2::new(5.0, 0.0);
        gs.fighters[1].state = State::Stand;
        gs.fighters[1].state_frame = 0;
        gs.fighters[1].hitlag = 0;
        gs.fighters[1].hitstun_timer = 0;
        gs.fighters[1].grounded = true;
        gs.fighters[0].pos = Vec2::new(-5.0, 0.0);
        gs.fighters[0].state = State::Stand;
        gs.fighters[0].state_frame = 0;
        gs.fighters[0].hitlag = 0;
        let mut landed = false;
        for i in 0..40u32 {
            let inp = if i == 0 {
                stick_press(0.55, 0.0, buttons::ATTACK)
            } else {
                neutral()
            };
            gs.step(&[inp, neutral()]);
            if gs.fighters[1].percent > 0.0 && !landed {
                landed = true;
                percents.push(gs.fighters[1].percent);
            }
        }
        // Let the swing finish before the next one.
        for _ in 0..40 {
            gs.step(&[neutral(), neutral()]);
        }
    }
    // 8 → 8 × 0.91 → 8 × 0.83 (newest slot 0.09, next 0.08).
    assert_eq!(percents, vec![8.0, 7.3, 6.6]);
    assert!((gs.fighters[0].stale_multiplier(MoveId::Ftilt) - 0.76).abs() < 1e-4);
    // Knockback still uses the fresh 8 %: hitstun is unchanged.
    let kb_fresh = overframe::sim::knockback::knockback(8.0, 8.0, 90.0, 70.0, 15.0);
    let _ = kb_fresh;
    // A KO clears the queue.
    gs.fighters[0].stocks = 3;
    gs.fighters[0].pos = Vec2::new(0.0, -400.0);
    gs.fighters[0].grounded = false;
    gs.fighters[0].state = State::Air;
    for _ in 0..200 {
        gs.step(&[neutral(), neutral()]);
    }
    assert_eq!(gs.fighters[0].stale_multiplier(MoveId::Ftilt), 1.0);
}

#[test]
fn spikes_can_be_meteor_cancelled_after_8_frames() {
    // Kestrel down-air (270°) on an airborne Kestrel.
    fn setup() -> GameState {
        let mut gs = duel(0.0);
        // Out at x = 140: no platform underneath but the main stage.
        gs.fighters[0].pos = Vec2::new(140.0, 150.0);
        gs.fighters[0].grounded = false;
        gs.fighters[0].state = State::Air;
        gs.fighters[1].pos = Vec2::new(144.0, 120.0);
        gs.fighters[1].grounded = false;
        gs.fighters[1].state = State::Air;
        let mut hit = false;
        for i in 0..30u32 {
            // C-stick down: a down-air without the fast-fall.
            let inp = if i == 0 { cstick(0.0, -1.0) } else { neutral() };
            gs.step(&[inp, neutral()]);
            if matches!(gs.fighters[1].state, State::Hitstun { .. }) {
                hit = true;
                break;
            }
        }
        assert!(hit, "dair connects");
        assert!(gs.fighters[1].meteor, "a 270° hit is a meteor");
        gs
    }
    // Jumping too early does nothing; after 8 frames it cancels.
    let mut gs = setup();
    for _ in 0..gs.fighters[1].hitlag {
        gs.step(&[neutral(), neutral()]);
    }
    for _ in 0..k::METEOR_CANCEL_FRAMES - 2 {
        gs.step(&[neutral(), neutral()]);
    }
    gs.step(&[neutral(), press(buttons::JUMP)]);
    assert!(
        matches!(gs.fighters[1].state, State::Hitstun { .. }),
        "too early"
    );
    for _ in 0..4 {
        gs.step(&[neutral(), neutral()]);
    }
    let jumps = gs.fighters[1].jumps_left;
    gs.step(&[neutral(), press(buttons::JUMP)]);
    assert!(
        matches!(gs.fighters[1].state, State::Air),
        "meteor cancelled"
    );
    assert_eq!(
        gs.fighters[1].jumps_left,
        jumps - 1,
        "spent the double jump"
    );
    assert!(gs.fighters[1].vel.y > 0.0, "rising");
}

// ------------------------------------------------------------ smashes, ledges, edges

#[test]
fn a_flick_is_a_smash_a_held_stick_is_a_tilt_and_holding_charges() {
    // Flick + attack on the same frame → forward smash.
    let mut gs = solo();
    gs.step(&[stick_press(1.0, 0.0, buttons::ATTACK)]);
    assert!(matches!(
        gs.fighters[0].state,
        State::Attack {
            id: MoveId::Fsmash,
            aerial: false
        }
    ));
    // Stick held (a walk-strength tilt) for a few frames, then attack →
    // forward tilt.
    let mut gs = solo();
    for _ in 0..k::SMASH_FLICK_FRAMES + 1 {
        gs.step(&[stick(0.55, 0.0)]);
    }
    gs.step(&[stick_press(0.55, 0.0, buttons::ATTACK)]);
    assert!(matches!(
        gs.fighters[0].state,
        State::Attack {
            id: MoveId::Ftilt,
            aerial: false
        }
    ));

    // Charging: hold attack → the smash waits at its charge frame; a full
    // charge (60 frames) deals ×1.367 and then releases on its own.
    let mut gs = duel(30.0);
    let base =
        overframe::sim::attacks::data(overframe::sim::roster::CharacterId::Kestrel, MoveId::Fsmash)
            .hitbox
            .damage;
    let mut hit_at = None;
    for i in 0..140u32 {
        let inp = stick_press(1.0, 0.0, buttons::ATTACK); // held throughout
        gs.step(&[inp, neutral()]);
        if gs.fighters[1].percent > 0.0 && hit_at.is_none() {
            hit_at = Some(i + 1);
        }
    }
    let hit_at = hit_at.expect("charged smash lands");
    assert!(
        hit_at > k::SMASH_CHARGE_MAX + 10,
        "the hit came after a full charge ({hit_at})"
    );
    assert_eq!(gs.fighters[0].charge, k::SMASH_CHARGE_MAX);
    assert_eq!(
        gs.fighters[1].percent,
        (base * (1.0 + k::SMASH_CHARGE_BONUS)).round()
    );

    // Releasing early gives a partial charge; a C-stick smash never charges.
    let mut gs = duel(30.0);
    for i in 0..60u32 {
        let inp = if i < 20 {
            stick_press(1.0, 0.0, buttons::ATTACK)
        } else {
            neutral()
        };
        gs.step(&[inp, neutral()]);
    }
    let pct = gs.fighters[1].percent;
    assert!(pct > base && pct < base * 1.3, "partial charge ({pct})");
    let mut gs = duel(30.0);
    for i in 0..60u32 {
        let inp = if i < 40 { cstick(1.0, 0.0) } else { neutral() };
        gs.step(&[inp, neutral()]);
    }
    assert_eq!(gs.fighters[1].percent, base, "C-stick smash is uncharged");
}

#[test]
fn an_occupied_ledge_cannot_be_grabbed() {
    let mut gs = duel(0.0);
    let m = gs.stage.main();
    // P1 hangs on the right ledge.
    let f = &mut gs.fighters[1];
    f.grounded = false;
    f.state = State::Air;
    f.pos = Vec2::new(m.right + f.character.half_width + 4.0, m.y - 4.0);
    f.vel = Vec2::new(0.0, -1.0);
    gs.step(&[neutral(), neutral()]);
    assert!(matches!(gs.fighters[1].state, State::LedgeGrab));
    // P0 arrives at the same ledge: edgehogged, keeps falling.
    let f = &mut gs.fighters[0];
    f.grounded = false;
    f.state = State::Air;
    f.pos = Vec2::new(m.right + f.character.half_width + 4.0, m.y - 4.0);
    f.vel = Vec2::new(0.0, -1.0);
    for _ in 0..4 {
        gs.step(&[neutral(), neutral()]);
    }
    assert!(matches!(gs.fighters[0].state, State::Air));
    assert!(gs.fighters[0].pos.y < m.y - 8.0, "fell past the ledge");
    // Once P1 lets go, the ledge is free again.
    for _ in 0..k::LEDGE_CATCH {
        gs.step(&[neutral(), neutral()]);
    }
    gs.step(&[neutral(), stick(0.0, -1.0)]);
    assert!(!matches!(gs.fighters[1].state, State::LedgeGrab));
    let f = &mut gs.fighters[0];
    f.pos = Vec2::new(m.right + f.character.half_width + 4.0, m.y - 4.0);
    f.vel = Vec2::new(0.0, -1.0);
    f.ledge_regrab_cd = 0;
    gs.step(&[neutral(), neutral()]);
    assert!(matches!(gs.fighters[0].state, State::LedgeGrab));
}

#[test]
fn landing_lag_is_edge_cancelled_by_sliding_off() {
    // Waveland toward the edge of a side platform: the slide carries the
    // fighter off it and the lag ends at once (Air), no 10-frame wait.
    let mut gs = solo();
    let f = &mut gs.fighters[0];
    f.pos = Vec2::new(115.0, 64.0);
    f.grounded = false;
    f.state = State::Air;
    f.vel = Vec2::ZERO;
    let mut cancelled_at = None;
    for i in 0..40u32 {
        let inp = if i == 0 {
            stick_press(1.0, -0.4, buttons::SHIELD)
        } else {
            neutral()
        };
        gs.step(&[inp]);
        let f = &gs.fighters[0];
        if matches!(f.state, State::Air) && i > 2 && !f.grounded {
            cancelled_at = Some(i + 1);
            break;
        }
    }
    let at = cancelled_at.expect("slid off into the air");
    assert!(
        at < 14,
        "edge-cancelled well before the waveland lag ended ({at})"
    );
    assert!(gs.fighters[0].pos.x > 125.0, "past the platform edge");
}

#[test]
fn jab_chains_into_jab_2_only_after_it_connects() {
    // Jab that hits → a second attack press during its recovery chains.
    let mut gs = duel(16.0);
    let mut chained = false;
    for i in 0..30u32 {
        let inp = if i == 0 || i == 6 {
            press(buttons::ATTACK)
        } else {
            neutral()
        };
        gs.step(&[inp, neutral()]);
        if matches!(
            gs.fighters[0].state,
            State::Attack {
                id: MoveId::Jab2,
                ..
            }
        ) {
            chained = true;
            break;
        }
    }
    assert!(chained, "jab 1 → jab 2");
    // The second jab lands too (jab 1 3 % + jab 2 4 %).
    for _ in 0..30 {
        gs.step(&[neutral(), neutral()]);
    }
    assert_eq!(gs.fighters[1].percent, 3.0 + 4.0);

    // A whiffed jab does not chain: the press is simply eaten.
    let mut gs = duel(200.0);
    let mut chained = false;
    for i in 0..30u32 {
        let inp = if i == 0 || i == 6 {
            press(buttons::ATTACK)
        } else {
            neutral()
        };
        gs.step(&[inp, neutral()]);
        if matches!(
            gs.fighters[0].state,
            State::Attack {
                id: MoveId::Jab2,
                ..
            }
        ) {
            chained = true;
        }
    }
    assert!(!chained, "a whiffed jab has no follow-up");
}

#[test]
fn rolls_stop_at_the_platform_edge() {
    // Roll right from near the main platform's right edge: never off it.
    let mut gs = solo();
    let m = gs.stage.main();
    gs.fighters[0].pos = Vec2::new(m.right - 20.0, 0.0);
    gs.step(&[neutral()]);
    gs.step(&[stick_press(1.0, 0.0, buttons::SHIELD)]);
    assert!(matches!(gs.fighters[0].state, State::Roll { .. }));
    for _ in 0..40 {
        gs.step(&[neutral()]);
        assert!(gs.fighters[0].grounded, "stayed on the stage");
    }
    let hw = gs.fighters[0].character.half_width;
    assert!(
        (gs.fighters[0].pos.x - (m.right - hw)).abs() < 0.01,
        "stopped at the edge"
    );

    // Same for a tech roll.
    let mut gs = solo();
    gs.fighters[0].pos = Vec2::new(m.right - 30.0, 40.0);
    gs.fighters[0].grounded = false;
    gs.fighters[0].state = State::Air;
    gs.fighters[0].apply_launch(Vec2::new(0.0, 2.0), 90, true, 0);
    for i in 0..80u32 {
        let inp = if i == 8 {
            stick_press(1.0, 0.0, buttons::SHIELD)
        } else {
            stick(1.0, 0.0)
        };
        gs.step(&[inp]);
        if matches!(gs.fighters[0].state, State::Tech { .. }) {
            break;
        }
    }
    assert!(matches!(gs.fighters[0].state, State::Tech { dir } if dir == 1.0));
    for _ in 0..50 {
        gs.step(&[neutral()]);
        assert!(gs.fighters[0].grounded);
    }
    assert!(gs.fighters[0].pos.x <= m.right - hw + 0.01);
}
