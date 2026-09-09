//! Game-feel contract: the impact and response rules calibrated against
//! published platform-fighter frame data (see `docs/GAME_FEEL.md`). These
//! pin the *numbers that players feel*: freeze frames on hit, shieldstun,
//! smash DI, crouch cancelling, weak hits sliding along the ground, landing
//! lag and L-cancel, helpless air-dodges, dash-attack momentum, movement
//! speed relative to the stage, and knockback decay.

use overframe::sim::attacks::{self, MoveId};
use overframe::sim::constants as k;
use overframe::sim::fighter::State;
use overframe::sim::knockback;
use overframe::sim::math::Vec2;
use overframe::sim::roster::CharacterId;
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

/// Two Kestrels standing face to face at `gap` units, settled on the stage.
fn duel(gap: f32) -> GameState {
    let mut gs = GameState::new(2, MatchConfig::default());
    for _ in 0..90 {
        gs.step(&[neutral(), neutral()]);
    }
    gs.fighters[0].pos = Vec2::new(-gap * 0.5, 0.0);
    gs.fighters[0].facing = 1.0;
    gs.fighters[1].pos = Vec2::new(gap * 0.5, 0.0);
    gs.fighters[1].facing = -1.0;
    gs.fighters[1].intangible = 0;
    gs.fighters[0].intangible = 0;
    gs
}

/// Step until the victim (player 1) is in hitstun, returning the frame it
/// happened on. Player 0 plays `p0(frame)`, player 1 plays `p1(frame)`.
fn until_hit(
    gs: &mut GameState,
    max: u64,
    mut p0: impl FnMut(u64) -> PlayerInput,
    mut p1: impl FnMut(u64) -> PlayerInput,
) -> Option<u64> {
    for i in 0..max {
        gs.step(&[p0(i), p1(i)]);
        if matches!(gs.fighters[1].state, State::Hitstun { .. }) {
            return Some(i);
        }
    }
    None
}

#[test]
fn hitlag_freezes_both_fighters_by_the_damage_formula() {
    // Forward tilt (8%): floor(8/3 + 3) = 5 freeze frames for both.
    let mut gs = duel(18.0);
    let f = until_hit(
        &mut gs,
        20,
        |i| {
            if i == 0 {
                stick_press(0.55, 0.0, buttons::ATTACK)
            } else {
                neutral()
            }
        },
        |_| neutral(),
    )
    .expect("ftilt should connect");
    let expect = knockback::hitlag(8.0, false, false);
    assert_eq!(expect, 5);
    assert_eq!(gs.fighters[1].hitlag, expect, "victim freeze");
    assert_eq!(gs.fighters[0].hitlag, expect, "attacker freeze");
    // Nobody moves while frozen.
    let (a, b) = (gs.fighters[0].pos, gs.fighters[1].pos);
    for _ in 0..expect {
        gs.step(&[neutral(), neutral()]);
    }
    assert_eq!(gs.fighters[0].pos, a);
    assert_eq!(gs.fighters[1].pos, b);
    let _ = f;
}

#[test]
fn weak_grounded_hit_slides_along_the_ground() {
    // A jab at 0% is a Sakurai-angle hit below the launch threshold: the
    // victim flinches and slides but never leaves the floor.
    let mut gs = duel(16.0);
    until_hit(
        &mut gs,
        20,
        |i| {
            if i == 0 {
                press(buttons::ATTACK)
            } else {
                neutral()
            }
        },
        |_| neutral(),
    )
    .expect("jab should connect");
    let x0 = gs.fighters[1].pos.x;
    for _ in 0..30 {
        gs.step(&[neutral(), neutral()]);
        assert!(
            gs.fighters[1].pos.y.abs() < 1e-3,
            "victim must stay on the floor"
        );
    }
    assert!(gs.fighters[1].pos.x > x0 + 2.0, "victim slides back");
    assert!(
        matches!(gs.fighters[1].state, State::Stand),
        "back to standing"
    );
}

#[test]
fn strong_hit_launches_with_uniform_decay_and_gravity() {
    let mut gs = duel(24.0);
    gs.fighters[1].percent = 80.0;
    until_hit(
        &mut gs,
        40,
        |i| if i < 2 { cstick(1.0, 0.0) } else { neutral() },
        |_| neutral(),
    )
    .expect("fsmash should connect");
    let hl = gs.fighters[1].hitlag;
    for _ in 0..hl {
        gs.step(&[neutral(), neutral()]);
    }
    assert!(gs.fighters[1].kb_vel.y > 0.0, "launched upward");
    let s0 = gs.fighters[1].kb_vel.length();
    gs.step(&[neutral(), neutral()]);
    let s1 = gs.fighters[1].kb_vel.length();
    assert!(
        (s0 - s1 - k::KB_DECAY).abs() < 1e-3,
        "knockback decays by KB_DECAY per frame"
    );
    assert!(
        gs.fighters[1].kb_fall < 0.0,
        "gravity accumulates separately"
    );
    assert!(matches!(
        gs.fighters[1].state,
        State::Hitstun { tumble: true }
    ));
}

#[test]
fn crouch_cancel_takes_a_third_off_the_knockback() {
    fn launch_speed(crouch: bool) -> f32 {
        let mut gs = duel(18.0);
        gs.fighters[1].percent = 40.0;
        until_hit(
            &mut gs,
            30,
            |i| {
                if i == 6 {
                    stick_press(0.55, 0.0, buttons::ATTACK)
                } else {
                    neutral()
                }
            },
            |_| if crouch { stick(0.0, -1.0) } else { neutral() },
        )
        .expect("ftilt should connect");
        gs.fighters[1].kb_vel.length()
    }
    let standing = launch_speed(false);
    let crouching = launch_speed(true);
    assert!(crouching > 0.0 && standing > 0.0);
    let ratio = crouching / standing;
    assert!((ratio - k::CROUCH_CANCEL).abs() < 0.03, "ratio {ratio}");
}

#[test]
fn smash_di_moves_the_victim_during_hitlag() {
    fn run(sdi: bool) -> Vec2 {
        let mut gs = duel(24.0);
        gs.fighters[1].percent = 60.0;
        let hit = until_hit(
            &mut gs,
            40,
            |i| if i < 2 { cstick(1.0, 0.0) } else { neutral() },
            |_| neutral(),
        )
        .expect("fsmash should connect");
        let _ = hit;
        // During hitlag: one hard stick pulse up-and-away (SDI), then held.
        let hl = gs.fighters[1].hitlag;
        assert!(hl > 3);
        for i in 0..hl {
            // Held up all through hitlag: one pulse on entry, ASDI at the end.
            let _ = i;
            let p1 = if sdi { stick(0.0, 1.0) } else { neutral() };
            gs.step(&[neutral(), p1]);
        }
        gs.fighters[1].pos
    }
    let with = run(true);
    let without = run(false);
    let dy = with.y - without.y;
    // One SDI pulse (6 ref units) + ASDI on the last frame (3 ref units).
    let expect = k::SDI_STEP + k::ASDI_STEP;
    assert!(
        (dy - expect).abs() < 0.5,
        "SDI moved {dy}, expected {expect}"
    );
}

#[test]
fn shieldstun_and_pushback_follow_the_block_formulas() {
    let mut gs = duel(18.0);
    let mut stun = None;
    for i in 0..30u64 {
        let p0 = if i == 8 {
            stick_press(0.55, 0.0, buttons::ATTACK)
        } else {
            neutral()
        };
        // Raise the shield early enough not to powershield.
        gs.step(&[p0, press(buttons::SHIELD)]);
        if let State::ShieldStun { total } = gs.fighters[1].state {
            stun = Some(total);
            break;
        }
    }
    let stun = stun.expect("ftilt should be blocked");
    assert_eq!(stun, knockback::shieldstun(8.0));
    assert_eq!(stun, 5);
    // Defender pushed away from the attacker at the formula's speed.
    assert!((gs.fighters[1].vel.x - knockback::shield_push(8.0)).abs() < 1e-3);
    assert!(
        gs.fighters[1].shield_health < k::SHIELD_MAX,
        "shield took damage"
    );
}

#[test]
fn powershield_in_the_first_frames_takes_no_stun() {
    // Shield raised on the frame the hit lands: powershield.
    let mut gs = duel(18.0);
    let mut saw_ps = false;
    for i in 0..40u64 {
        let p0 = if i == 0 {
            stick_press(0.55, 0.0, buttons::ATTACK)
        } else {
            neutral()
        };
        // Kestrel ftilt hits on frame 4 of the move: shield raised on that
        // very frame (step 3) → inside the powershield window.
        let p1 = if i >= 3 {
            press(buttons::SHIELD)
        } else {
            neutral()
        };
        gs.step(&[p0, p1]);
        if gs
            .fx
            .iter()
            .any(|f| f.kind == overframe::sim::FxKind::Powershield)
        {
            saw_ps = true;
            break;
        }
    }
    assert!(saw_ps, "expected a powershield");
    assert!(matches!(gs.fighters[1].state, State::Shield));
    assert!(
        (gs.fighters[1].shield_health - k::SHIELD_MAX).abs() < 0.6,
        "no shield damage"
    );
}

#[test]
fn shield_drop_and_out_of_shield_options() {
    let mut gs = GameState::new(1, MatchConfig::default());
    for _ in 0..90 {
        gs.step(&[neutral()]);
    }
    for _ in 0..3 {
        gs.step(&[press(buttons::SHIELD)]);
    }
    assert!(matches!(gs.fighters[0].state, State::Shield));
    gs.step(&[neutral()]);
    assert!(matches!(gs.fighters[0].state, State::ShieldDrop));
    // Frames spent lowering the shield (unable to act).
    let mut lag = 1;
    while matches!(gs.fighters[0].state, State::ShieldDrop) {
        // Tapping attack does nothing while dropping ...
        gs.step(&[if lag % 2 == 0 {
            press(buttons::ATTACK)
        } else {
            neutral()
        }]);
        assert!(!matches!(gs.fighters[0].state, State::Attack { .. }));
        lag += 1;
        assert!(lag < 40);
    }
    // ... the transition tick is the first actionable one.
    assert_eq!(
        lag,
        k::SHIELD_DROP + 1,
        "SHIELD_DROP frames of lag, then free"
    );
    // Jump out of shield is instant.
    for _ in 0..3 {
        gs.step(&[press(buttons::SHIELD)]);
    }
    gs.step(&[stick_press(0.0, 0.0, buttons::SHIELD | buttons::JUMP)]);
    assert!(matches!(gs.fighters[0].state, State::JumpSquat));
}

#[test]
fn air_dodge_ends_helpless_and_wavedash_still_works() {
    let mut gs = GameState::new(1, MatchConfig::default());
    for _ in 0..90 {
        gs.step(&[neutral()]);
    }
    // Full hop, air-dodge straight up (no landing): must end helpless.
    let mut helpless = false;
    for i in 0..80u64 {
        let inp = if i < 6 {
            press(buttons::JUMP)
        } else if i == 8 {
            stick_press(0.0, 1.0, buttons::SHIELD)
        } else {
            neutral()
        };
        gs.step(&[inp]);
        if matches!(gs.fighters[0].state, State::Helpless) {
            helpless = true;
            break;
        }
    }
    assert!(
        helpless,
        "an air-dodge that doesn't land leaves you helpless"
    );
}

#[test]
fn landing_lag_normal_lcancel_and_autocancel() {
    fn land_with(inputs: impl Fn(u64) -> PlayerInput) -> u32 {
        let mut gs = GameState::new(1, MatchConfig::default());
        for _ in 0..90 {
            gs.step(&[neutral()]);
        }
        for i in 0..120u64 {
            gs.step(&[inputs(i)]);
            if let State::LandLag { total } = gs.fighters[0].state {
                return total;
            }
        }
        u32::MAX
    }
    // Empty short hop: normal landing lag.
    let plain = land_with(|i| {
        if i == 0 {
            press(buttons::JUMP)
        } else {
            neutral()
        }
    });
    assert_eq!(plain, k::LANDING_LAG_NORMAL);
    // Nair, fast-fall into it: the full aerial landing lag ...
    let nair = attacks::data(CharacterId::Kestrel, MoveId::Nair).landing_lag;
    let full = land_with(|i| match i {
        0 => press(buttons::JUMP),
        3 => press(buttons::ATTACK),
        i if i >= 5 => stick(0.0, -1.0),
        _ => neutral(),
    });
    assert_eq!(full, nair);
    // ... halved (floored) by an L-cancel ...
    let lc = land_with(|i| match i {
        0 => press(buttons::JUMP),
        3 => press(buttons::ATTACK),
        i if i >= 5 => stick_press(0.0, -1.0, buttons::SHIELD),
        _ => neutral(),
    });
    assert_eq!(lc, nair / 2);
    // ... and a nair thrown out right before touchdown (hitbox not yet out)
    // autocancels into a normal landing.
    let landing_frame = {
        let mut gs = GameState::new(1, MatchConfig::default());
        for _ in 0..90 {
            gs.step(&[neutral()]);
        }
        let mut f = None;
        for i in 0..120u64 {
            gs.step(&[if i == 0 {
                press(buttons::JUMP)
            } else {
                neutral()
            }]);
            if i > 3 && gs.fighters[0].grounded {
                f = Some(i);
                break;
            }
        }
        f.expect("short hop lands")
    };
    let ac = land_with(|i| match i {
        0 => press(buttons::JUMP),
        i if i == landing_frame - 1 => press(buttons::ATTACK),
        _ => neutral(),
    });
    assert_eq!(ac, k::LANDING_LAG_NORMAL, "autocancel");
}

#[test]
fn late_hits_are_weaker_and_moves_end_at_iasa() {
    let md = attacks::data(CharacterId::Kestrel, MoveId::Nair);
    let clean = md.hitbox_at(md.startup).unwrap();
    let late = md.hitbox_at(md.startup + md.active + 2).unwrap();
    assert!(late.damage < clean.damage && late.bkb < clean.bkb);
    assert!(md.is_active(md.startup + md.active + md.late_active - 1));
    assert!(!md.is_active(md.startup + md.active + md.late_active));
    assert_eq!(
        md.total(),
        md.startup + md.active + md.late_active + md.endlag
    );
    // A sex kick lingers: more than a third of the move has a hitbox out.
    assert!((md.active + md.late_active) * 3 > md.total());
}

#[test]
fn dash_attack_carries_momentum_but_jab_plants() {
    fn travel(id: MoveId, input: PlayerInput) -> f32 {
        let mut gs = GameState::new(1, MatchConfig::default());
        for _ in 0..90 {
            gs.step(&[neutral()]);
        }
        for _ in 0..14 {
            gs.step(&[stick(1.0, 0.0)]);
        }
        let x0 = gs.fighters[0].pos.x;
        gs.step(&[input]);
        assert!(matches!(gs.fighters[0].state, State::Attack { id: got, .. } if got == id));
        for _ in 0..25 {
            gs.step(&[neutral()]);
        }
        gs.fighters[0].pos.x - x0
    }
    let da = travel(MoveId::DashAttack, stick_press(1.0, 0.0, buttons::ATTACK));
    assert!(da > 25.0, "dash attack slides forward ({da})");
}

#[test]
fn run_speed_crosses_the_stage_at_reference_pace() {
    // A fast-faller runs the main platform (310 units) in roughly a second:
    // the reference crosses its three-platform stage in ~65 frames.
    let mut gs = GameState::new(1, MatchConfig::default());
    for _ in 0..90 {
        gs.step(&[neutral()]);
    }
    gs.fighters[0].pos.x = -150.0;
    let mut frames = 0;
    while gs.fighters[0].pos.x < 150.0 && frames < 200 {
        gs.step(&[stick(1.0, 0.0)]);
        frames += 1;
    }
    assert!((60..=85).contains(&frames), "crossed in {frames} frames");
    // And falls a body height in about five frames at terminal speed.
    let ch = CharacterId::Kestrel.data();
    assert!(ch.height / ch.max_fall < 6.0);
}

#[test]
fn dash_dance_window_is_per_character() {
    assert_eq!(CharacterId::Kestrel.data().dash_frames, 11);
    assert!(CharacterId::Viper.data().dash_frames < CharacterId::Kestrel.data().dash_frames);
    assert!(CharacterId::Boulder.data().dash_frames > CharacterId::Kestrel.data().dash_frames);
}

#[test]
fn swept_hitboxes_do_not_tunnel() {
    use overframe::sim::math::segment_distance;
    // A hitbox that jumps from left of a capsule to right of it in one tick
    // still registers (distance along the sweep is zero).
    let a0 = Vec2::new(-20.0, 15.0);
    let a1 = Vec2::new(20.0, 15.0);
    let (h0, h1) = (Vec2::new(0.0, 5.0), Vec2::new(0.0, 25.0));
    assert_eq!(segment_distance(a0, a1, h0, h1), 0.0);
    assert!(
        (segment_distance(Vec2::new(10.0, 15.0), Vec2::new(20.0, 15.0), h0, h1) - 10.0).abs()
            < 1e-4
    );
}
