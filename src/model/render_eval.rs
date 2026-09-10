//! The one place a fighter's *drawn* pose and root are computed.
//!
//! `Scene3D` draws with it and the contact measurement measures with it, so
//! a diagnostic can never describe a different mesh than the one on screen:
//! both go through the eased facing (turns take a few frames), the extras
//! lag history and the hitlag rattle. The state lives in a [`PortState`]
//! per fighter port; it is render-only, never saved or rolled back, and a
//! capture reconstructs it deterministically by replaying the sampled
//! ticks in order from a fresh state (`docs/art/capture-suite.json`,
//! `contact_render_contract`).

use super::characters::CharacterModel;
use super::math3::{v3, Xf, M3};
use super::procedural::ExtrasState;
use super::rig::Pose;
use super::{anim, contact};
use crate::sim::attacks::MoveId;
use crate::sim::fighter::{Fighter, State};

/// Frames a turn takes to settle (per sim frame; see [`PortState::advance`]).
pub const TURN_EASE: f32 = 0.42;
/// A gap larger than this between drawn frames resets the eased facing
/// (a new match, a seek, a long stall).
pub const FRESH_GAP: u64 = 30;

/// Render-only state of one fighter port.
#[derive(Clone, Debug)]
pub struct PortState {
    /// Eased facing in −1..1 (the sim's is ±1).
    pub facing: f32,
    pub last_frame: u64,
    /// Lag history of spec-built extras (scarf, crest…).
    pub lag: ExtrasState,
}

impl Default for PortState {
    fn default() -> Self {
        PortState {
            facing: 1.0,
            last_frame: u64::MAX,
            lag: ExtrasState::default(),
        }
    }
}

impl PortState {
    /// Advance to sim `frame` and return the eased facing. Returns `true`
    /// as the second value when the history was (re)started this frame.
    pub fn advance(&mut self, f: &Fighter, frame: u64) -> (f32, bool) {
        let fresh = self.last_frame == u64::MAX
            || frame < self.last_frame
            || frame > self.last_frame + FRESH_GAP;
        if fresh {
            self.facing = f.facing;
        } else if frame != self.last_frame {
            // Ease toward the sim facing over ~5 frames (per sim frame, so
            // rollback re-simulation does not speed it up).
            let steps = (frame - self.last_frame).min(6);
            for _ in 0..steps {
                self.facing += (f.facing - self.facing) * TURN_EASE;
            }
            if (self.facing - f.facing).abs() < 0.02 {
                self.facing = f.facing;
            }
        }
        self.last_frame = frame;
        (self.facing, fresh)
    }
}

/// Root transform for an eased facing: +1 → 0°, −1 → 180°, in between the
/// fighter faces the camera.
pub fn root_for(f: &Fighter, eased_facing: f32) -> Xf {
    let deg = (1.0 - eased_facing) * 90.0;
    Xf::new(M3::rot_y(deg), v3(f.pos.x, f.pos.y, 0.0))
}

/// What the renderer draws for one fighter on one frame.
#[derive(Clone, Debug)]
pub struct Evaluated {
    pub pose: Pose,
    pub root: Xf,
    pub eased_facing: f32,
    /// The port history was (re)started on this frame.
    pub fresh: bool,
}

/// Evaluate the drawn pose and root of `f` on sim `frame`, advancing the
/// port's render history. `lag_enabled` = run the extras lag layer.
pub fn evaluate(
    model: &CharacterModel,
    port: &mut PortState,
    f: &Fighter,
    frame: u64,
    lag_enabled: bool,
) -> Evaluated {
    let (eased, fresh) = port.advance(f, frame);
    let rig = &model.rig;
    let mut pose = anim::fighter_pose(rig, f, &model.style, frame);
    (model.secondary)(rig, &mut pose, f, frame);
    let mut root = root_for(f, eased);
    if !model.extras.is_empty() && lag_enabled {
        port.lag.apply(
            &model.extras,
            rig,
            &mut pose,
            &root,
            frame,
            f.hitlag > 0,
            f.facing,
        );
    }
    // Hitlag shake: the victim vibrates in place while frozen (the classic
    // freeze-frame "rattle"); harder hits rattle wider.
    if f.hitlag > 0 && matches!(f.state, State::Hitstun { .. }) {
        let amp = 0.9 + (f.hitlag as f32).min(12.0) * 0.12;
        let s = if frame % 2 == 0 { 1.0 } else { -1.0 };
        root.t = root.t + v3(s * amp, (frame % 4 == 0) as i32 as f32 * amp * 0.4, 0.0);
    }
    Evaluated {
        pose,
        root,
        eased_facing: eased,
        fresh,
    }
}

/// Evaluate and measure in one go — the measurement is taken on exactly
/// the pose and root [`evaluate`] returns.
pub fn evaluate_and_measure(
    model: &CharacterModel,
    port: &mut PortState,
    f: &Fighter,
    frame: u64,
    lag_enabled: bool,
    action: MoveId,
    hitbox: Option<(&str, [f32; 2], f32)>,
) -> (Evaluated, Option<contact::ContactMeasure>) {
    let ev = evaluate(model, port, f, frame, lag_enabled);
    let m = contact::measure_posed(
        model,
        f,
        &ev.pose,
        &ev.root,
        ev.eased_facing,
        action,
        hitbox,
    );
    (ev, m)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::characters::build;
    use crate::sim::roster::CharacterId;
    use crate::sim::Vec2;

    fn fighter(facing: f32) -> Fighter {
        let mut f = Fighter::new(CharacterId::Kestrel.data(), 0, Vec2::ZERO);
        f.facing = facing;
        f.grounded = true;
        f.set_state_pub(State::Stand);
        f
    }

    #[test]
    fn a_turn_is_eased_and_the_measurement_follows_the_drawn_root() {
        let model = build(CharacterId::Kestrel);
        let mut port = PortState::default();
        let right = fighter(1.0);
        for frame in 0..4u64 {
            let ev = evaluate(&model, &mut port, &right, frame, true);
            assert_eq!(ev.eased_facing, 1.0);
        }
        // The sim flips the facing on frame 4; the drawn facing takes
        // several frames to get there and the measured root is the drawn
        // one, not the sim's.
        let left = fighter(-1.0);
        let mut eased = Vec::new();
        for frame in 4..20u64 {
            let (ev, m) = evaluate_and_measure(
                &model,
                &mut port,
                &left,
                frame,
                true,
                MoveId::Jab,
                Some(("clean", [13.0, 21.0], 7.5)),
            );
            eased.push(ev.eased_facing);
            let m = m.unwrap();
            assert_eq!(m.eased_facing, ev.eased_facing);
            assert_eq!(m.facing, -1.0);
            // The measured joint is where the drawn root puts it.
            let rig = &model.rig;
            let w = rig.world(&ev.pose, &ev.root);
            let j = rig.joint(&w, "hand_r").unwrap();
            assert!((m.joint[0] - j.x).abs() < 1e-5 && (m.joint[1] - j.y).abs() < 1e-5);
        }
        assert!(
            eased[0] > -1.0 && eased[0] < 1.0,
            "first turn frame is mid-way: {eased:?}"
        );
        assert!(
            eased.windows(2).all(|w| w[1] <= w[0]),
            "monotone turn: {eased:?}"
        );
        assert_eq!(*eased.last().unwrap(), -1.0, "settled: {eased:?}");
        // A sim-facing root on a mid-turn frame would differ from the drawn
        // one — which is why measurements never use it.
        let mid = root_for(&left, eased[0]);
        let sim = root_for(&left, -1.0);
        assert!((mid.m.m[0][0] - sim.m.m[0][0]).abs() > 0.1);
    }

    #[test]
    fn fresh_history_snaps_and_a_gap_resets() {
        let model = build(CharacterId::Kestrel);
        let mut port = PortState::default();
        let left = fighter(-1.0);
        let ev = evaluate(&model, &mut port, &left, 100, true);
        assert!(ev.fresh);
        assert_eq!(ev.eased_facing, -1.0);
        let right = fighter(1.0);
        let ev = evaluate(&model, &mut port, &right, 101, true);
        assert!(!ev.fresh);
        assert!(ev.eased_facing < 1.0);
        // A long gap (seek / new match) restarts the history.
        let ev = evaluate(&model, &mut port, &right, 101 + FRESH_GAP + 1, true);
        assert!(ev.fresh);
        assert_eq!(ev.eased_facing, 1.0);
        // Going backwards (rollback) also restarts it.
        let ev = evaluate(&model, &mut port, &left, 50, true);
        assert!(ev.fresh);
        assert_eq!(ev.eased_facing, -1.0);
    }

    #[test]
    fn replaying_a_fresh_history_reproduces_the_sequential_state() {
        let model = build(CharacterId::Kestrel);
        let seq: Vec<Fighter> = (0..10)
            .map(|i| fighter(if i < 5 { 1.0 } else { -1.0 }))
            .collect();
        let mut a = PortState::default();
        let mut last = None;
        for (i, f) in seq.iter().enumerate() {
            last = Some(evaluate(&model, &mut a, f, i as u64, true));
        }
        let mut b = PortState::default();
        for (i, f) in seq.iter().enumerate().take(9) {
            evaluate(&model, &mut b, f, i as u64, true);
        }
        let again = evaluate(&model, &mut b, &seq[9], 9, true);
        let last = last.unwrap();
        assert_eq!(again.eased_facing, last.eased_facing);
        assert_eq!(again.pose, last.pose);
        assert_eq!(again.root.t, last.root.t);
    }
}
