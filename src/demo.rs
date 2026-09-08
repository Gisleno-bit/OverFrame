//! A scripted demo match — a short "combo video" used by the headless GIF tool
//! and the game's attract screen. It shows the movement tech (dash-dance,
//! wavedash, walk-in spacing, tilts) and finishes with a high-percent KO.
//!
//! The performer (P1) is a small closed-loop controller: it reads the live
//! [`GameState`] and reacts to spacing, so the choreography lands reliably while
//! still being produced entirely through the same inputs and
//! [`GameState::step`] the real game uses — an honest demonstration, not a canned
//! animation. It is deterministic given the state, so it replays identically.

use crate::sim::input::buttons;
use crate::sim::math::Vec2;
use crate::sim::{GameState, MatchConfig, PlayerInput};

/// Length of the demo loop, in ticks.
pub const DEMO_LEN: u64 = 300;

/// Build the demo's starting position: two fighters on the ground, the target
/// pre-damaged so a clean hit shows a satisfying high-percent KO.
pub fn match_state() -> GameState {
    match_state_with(MatchConfig {
        stocks: 4,
        seed: 0xC0FFEE,
        ..MatchConfig::default()
    })
}

/// The demo starting position with custom rules (characters, stage, palettes).
/// The choreography is tuned for the default cast but plays on any.
pub fn match_state_with(config: MatchConfig) -> GameState {
    let mut gs = GameState::new(2, config);
    let floor = gs.stage.main().y;
    gs.fighters[0].pos = Vec2::new(-34.0, floor);
    gs.fighters[0].facing = 1.0;
    gs.fighters[0].grounded = true;
    gs.fighters[1].pos = Vec2::new(52.0, floor);
    gs.fighters[1].facing = -1.0;
    gs.fighters[1].grounded = true;
    gs.fighters[1].percent = 74.0;
    gs
}

fn stick(x: f32, y: f32) -> PlayerInput {
    PlayerInput {
        stick: Vec2::new(x, y),
        ..Default::default()
    }
}
fn stick_btn(x: f32, y: f32, b: u16) -> PlayerInput {
    PlayerInput {
        stick: Vec2::new(x, y),
        buttons: b,
        ..Default::default()
    }
}
fn press(b: u16) -> PlayerInput {
    PlayerInput {
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

/// Inputs for both players, given the live state and the current tick.
pub fn inputs(gs: &GameState, frame: u64) -> [PlayerInput; 2] {
    let f = frame % DEMO_LEN;
    let a = &gs.fighters[0];
    let b = &gs.fighters[1];
    let dx = b.pos.x - a.pos.x;
    let dir = if dx >= 0.0 { 1.0 } else { -1.0 };
    let adx = dx.abs();

    let p1 = if f < 38 {
        // 1) Dash-dance flourish.
        if (f / 7) % 2 == 0 {
            stick(1.0, 0.0)
        } else {
            stick(-1.0, 0.0)
        }
    } else if f == 42 {
        // 2) Wavedash toward the target (short hop...).
        press(buttons::JUMP)
    } else if f == 46 {
        // (...down-forward air-dodge into the ground; a half tilt so the
        // slide stops at poking range instead of crossing the target).
        stick_btn(0.7 * dir, -0.5, buttons::SHIELD)
    } else if (58..150).contains(&f) {
        // 3) Walk in to spacing, then poke with a tilt.
        if !a.grounded {
            PlayerInput::default()
        } else if adx > 26.0 {
            stick(dir * 0.6, 0.0)
        } else if f == 96 {
            stick_btn(dir * 0.9, 0.0, buttons::ATTACK) // ftilt
        } else {
            PlayerInput::default()
        }
    } else if f == 168 || f == 169 {
        // 4) The finisher: forward smash → KO.
        cstick(dir, 0.0)
    } else if (58..250).contains(&f) && a.grounded && adx > 26.0 {
        // keep spacing if the target drifted (e.g. after a hit)
        stick(dir * 0.5, 0.0)
    } else {
        PlayerInput::default()
    };

    // Target: DI up-and-away while being launched, otherwise a passive dummy.
    let p2 = if matches!(b.state, crate::sim::fighter::State::Hitstun { .. }) {
        stick(0.6 * dir, 0.5)
    } else {
        PlayerInput::default()
    };

    [p1, p2]
}
