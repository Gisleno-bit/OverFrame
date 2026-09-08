//! Tournament-style match camera: frames every live fighter, zooms with the
//! spread, sits a little above the action and smooths every move. Purely a
//! renderer state — it reads the simulation and never writes to it, so it is
//! rollback-safe by construction.

use super::math3::{v3, V3};
use crate::sim::fighter::State;
use crate::sim::GameState;

#[derive(Clone, Copy, Debug)]
pub struct MatchCamera {
    pub pos: V3,
    pub target: V3,
    /// Vertical field of view, radians.
    pub fovy: f32,
    initialised: bool,
}

impl Default for MatchCamera {
    fn default() -> Self {
        Self::new()
    }
}

impl MatchCamera {
    pub const MIN_DIST: f32 = 170.0;
    pub const MAX_DIST: f32 = 840.0;

    pub fn new() -> Self {
        MatchCamera {
            pos: v3(0.0, 90.0, 380.0),
            target: v3(0.0, 40.0, 0.0),
            fovy: 38.0f32.to_radians(),
            initialised: false,
        }
    }

    /// Where the camera wants to be for this state: (target, distance).
    pub fn desired(&self, gs: &GameState, aspect: f32) -> (V3, f32) {
        let st = &gs.stage;
        let mut lo = v3(f32::MAX, f32::MAX, 0.0);
        let mut hi = v3(f32::MIN, f32::MIN, 0.0);
        let mut n = 0;
        for f in &gs.fighters {
            if matches!(f.state, State::Dead) {
                continue;
            }
            let c = f.body_center();
            let h = f.character.height;
            lo.x = lo.x.min(c.x - 20.0);
            hi.x = hi.x.max(c.x + 20.0);
            lo.y = lo.y.min(c.y - h * 0.5 - 12.0);
            hi.y = hi.y.max(c.y + h * 0.5 + 16.0);
            n += 1;
        }
        if n == 0 {
            let m = st.main();
            lo = v3(m.left, m.y - 20.0, 0.0);
            hi = v3(m.right, m.y + 120.0, 0.0);
        }
        // Never frame tighter than the main platform's middle third, and keep
        // the stage top in view when fighters hug the floor.
        let m = st.main();
        let span = m.right - m.left;
        lo.x = lo.x.min(-span * 0.25);
        hi.x = hi.x.max(span * 0.25);
        lo.y = lo.y.min(m.y - 10.0);
        hi.y = hi.y.max(m.y + 70.0);

        let centre = v3((lo.x + hi.x) * 0.5, (lo.y + hi.y) * 0.5 + 6.0, 0.0);
        let half_w = (hi.x - lo.x) * 0.5 + 28.0;
        let half_h = (hi.y - lo.y) * 0.5 + 22.0;
        let t = (self.fovy * 0.5).tan();
        let dist = (half_h / t).max(half_w / (t * aspect.max(0.5)));
        let dist = dist.clamp(Self::MIN_DIST, Self::MAX_DIST);
        // Keep the target inside the blast zones so the view never drifts
        // into empty space.
        let tx = centre.x.clamp(st.blast_left * 0.6, st.blast_right * 0.6);
        let ty = centre.y.clamp(st.blast_bottom * 0.45, st.blast_top * 0.75);
        (v3(tx, ty, 0.0), dist)
    }

    /// Advance the camera one render frame toward its desired framing.
    pub fn update(&mut self, gs: &GameState, aspect: f32) {
        let (target, dist) = self.desired(gs, aspect);
        // Sit above the target looking slightly down, so platform tops read.
        let want = v3(target.x, target.y + dist * 0.22, dist);
        if !self.initialised {
            self.pos = want;
            self.target = target;
            self.initialised = true;
        } else {
            self.pos = self.pos.lerp(want, 0.11);
            self.target = self.target.lerp(target, 0.14);
        }
    }

    /// Deterministic shake offset from the sim's shake magnitude.
    pub fn shake(&self, gs: &GameState) -> V3 {
        let s = gs.camera_shake.min(14.0) * 0.6;
        let a = gs.frame as f32;
        v3((a * 1.7).sin() * s, (a * 2.3).cos() * s, 0.0)
    }

    /// Reset the smoothing (new match).
    pub fn reset(&mut self) {
        self.initialised = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::MatchConfig;

    #[test]
    fn camera_frames_both_fighters_and_settles() {
        let gs = GameState::new(2, MatchConfig::default());
        let mut cam = MatchCamera::new();
        cam.update(&gs, 16.0 / 9.0);
        let first = cam.pos;
        for _ in 0..120 {
            cam.update(&gs, 16.0 / 9.0);
        }
        // Settled at the desired framing.
        let (t, d) = cam.desired(&gs, 16.0 / 9.0);
        assert!((cam.target - t).len() < 0.5);
        assert!((cam.pos.z - d).abs() < 0.5);
        assert!(
            (first - cam.pos).len() < 1.0,
            "first update snaps, no drift"
        );
        // Both fighters inside the vertical frustum.
        let half = (cam.pos.z) * (cam.fovy * 0.5).tan();
        for f in &gs.fighters {
            let dy = (f.body_center().y - cam.target.y).abs();
            assert!(dy < half, "fighter out of frame");
        }
    }
}
