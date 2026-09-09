//! Procedural animation for the standard humanoid skeleton.
//!
//! Every fighter state maps to a pose, blended smoothly from key poses that are
//! driven by the *simulation's own frame data*: an attack winds up during its
//! startup frames, snaps to the strike pose on its first active frame and
//! recovers during endlag, so what you see always matches what hits. The
//! striking limb is aimed at the move's hitbox offset, so a low sweep is low
//! and an up-tilt is up, per character, without hand-keying each move.
//!
//! Poses are keyed in Euler degrees (see `math3::M3::euler`): for a limb
//! hanging down, `+z` swings its tip forward (+X); for the spine, `-z` leans
//! forward. `±x` moves a limb out sideways (sign depends on the side).

use super::math3::{ease, ease_in, ease_out, v3, V3};
use super::rig::{Pose, Rig};
use crate::sim::attacks::{self, MoveId};
use crate::sim::fighter::{Fighter, GetupKind, LedgeKind, State};
use std::f32::consts::TAU;

/// Body proportions for [`humanoid_skeleton`] (sim units; fighters are
/// ~30 tall).
#[derive(Clone, Copy, Debug)]
pub struct Proportions {
    pub thigh: f32,
    pub shin: f32,
    pub ankle: f32,
    pub torso: f32,
    pub neck: f32,
    pub shoulder_half: f32,
    pub hip_half: f32,
    pub upper_arm: f32,
    pub forearm: f32,
}

impl Proportions {
    pub fn hip_height(&self) -> f32 {
        self.ankle + self.shin + self.thigh
    }
}

/// Build the bone hierarchy (no meshes) every humanoid fighter shares.
pub fn humanoid_skeleton(p: &Proportions) -> Rig {
    let mut r = Rig::new();
    r.add("root", "", V3::ZERO);
    r.add("hips", "root", v3(0.0, p.hip_height(), 0.0));
    r.add("spine", "hips", v3(0.0, p.torso * 0.3, 0.0));
    r.add("chest", "spine", v3(0.0, p.torso * 0.35, 0.0));
    r.add("neck", "chest", v3(0.0, p.torso * 0.35, 0.0));
    r.add("head", "neck", v3(0.0, p.neck, 0.0));
    for (side, s) in [("r", 1.0f32), ("l", -1.0f32)] {
        r.add(
            &format!("upper_arm_{side}"),
            "chest",
            v3(0.0, p.torso * 0.28, s * p.shoulder_half),
        );
        r.add(
            &format!("forearm_{side}"),
            &format!("upper_arm_{side}"),
            v3(0.0, -p.upper_arm, 0.0),
        );
        r.add(
            &format!("hand_{side}"),
            &format!("forearm_{side}"),
            v3(0.0, -p.forearm, 0.0),
        );
        r.add(
            &format!("thigh_{side}"),
            "hips",
            v3(0.0, 0.0, s * p.hip_half),
        );
        r.add(
            &format!("shin_{side}"),
            &format!("thigh_{side}"),
            v3(0.0, -p.thigh, 0.0),
        );
        r.add(
            &format!("foot_{side}"),
            &format!("shin_{side}"),
            v3(0.0, -p.shin, 0.0),
        );
    }
    r
}

/// Per-character motion personality.
#[derive(Clone, Copy, Debug)]
pub struct AnimStyle {
    /// Vertical bob amplitude in idle/run.
    pub bob: f32,
    /// Forward lean while running (degrees).
    pub run_lean: f32,
    /// Limb swing multiplier.
    pub swing: f32,
    /// How far the hips drop in crouch (sim units).
    pub crouch_depth: f32,
    /// Frames per walk / run cycle.
    pub walk_cycle: f32,
    pub run_cycle: f32,
    /// 0 = nimble, 1 = ponderous (softens easing and adds follow-through).
    pub heavy: f32,
}

impl Default for AnimStyle {
    fn default() -> Self {
        AnimStyle {
            bob: 0.6,
            run_lean: 16.0,
            swing: 1.0,
            crouch_depth: 7.0,
            walk_cycle: 30.0,
            run_cycle: 16.0,
            heavy: 0.0,
        }
    }
}

// ----------------------------------------------------------------- helpers

fn lean(p: &mut Pose, rig: &Rig, deg: f32) {
    p.add(rig, "spine", v3(0.0, 0.0, -deg * 0.55));
    p.add(rig, "chest", v3(0.0, 0.0, -deg * 0.45));
}

/// Leg pose: `thigh`/`shin` forward-swing degrees; the foot compensates to
/// stay level unless `foot` is given.
fn leg(p: &mut Pose, rig: &Rig, side: &str, thigh: f32, shin: f32, foot: Option<f32>) {
    p.rot(rig, &format!("thigh_{side}"), 0.0, 0.0, thigh);
    p.rot(rig, &format!("shin_{side}"), 0.0, 0.0, shin);
    let f = foot.unwrap_or(-(thigh + shin) * 0.8);
    p.rot(rig, &format!("foot_{side}"), 0.0, 0.0, f);
}

/// Arm pose: `upper` forward-swing, `fore` elbow bend, `out` sideways lift.
fn arm(p: &mut Pose, rig: &Rig, side: &str, upper: f32, fore: f32, out: f32) {
    let s = if side == "r" { -1.0 } else { 1.0 };
    p.rot(rig, &format!("upper_arm_{side}"), s * out, 0.0, upper);
    p.rot(rig, &format!("forearm_{side}"), 0.0, 0.0, fore);
    p.rot(rig, &format!("hand_{side}"), 0.0, 0.0, fore * 0.15);
}

fn both_legs(p: &mut Pose, rig: &Rig, thigh: f32, shin: f32) {
    leg(p, rig, "r", thigh, shin, None);
    leg(p, rig, "l", thigh, shin, None);
}

fn both_arms(p: &mut Pose, rig: &Rig, upper: f32, fore: f32, out: f32) {
    arm(p, rig, "r", upper, fore, out);
    arm(p, rig, "l", upper, fore, out);
}

/// Rotate the whole body about a point `centre_h` above the feet (for rolls,
/// tumbles, knockdowns) so the visual centre stays put.
fn spin_body(p: &mut Pose, rig: &Rig, deg: f32, centre_h: f32) {
    if let Some(i) = rig.bone("root") {
        let (s, c) = deg.to_radians().sin_cos();
        // Rotating about the origin moves the centre (0,h) to (-h·s, h·c);
        // shift back so it stays at (0,h).
        p.rot[i] = v3(0.0, 0.0, deg);
        p.off[i] = p.off[i] + v3(centre_h * s, centre_h - centre_h * c, 0.0);
    }
}

fn root_scale(p: &mut Pose, rig: &Rig, sx: f32, sy: f32) {
    p.scale(rig, "root", v3(sx, sy, sx));
}

// ----------------------------------------------------------------- key poses

/// Ready stance: knees soft, front foot forward, guard up.
pub fn stance(rig: &Rig, st: &AnimStyle, t: f32) -> Pose {
    let mut p = rig.rest_pose();
    let breathe = (t * 0.10).sin();
    let sway = (t * 0.07).sin();
    leg(&mut p, rig, "r", 14.0, -16.0, None);
    leg(&mut p, rig, "l", -10.0, -12.0, None);
    lean(&mut p, rig, 7.0 + breathe * 0.8);
    p.rot(rig, "head", 0.0, 0.0, 4.0 - breathe);
    arm(&mut p, rig, "r", 30.0 + sway * 3.0, 95.0, 8.0);
    arm(&mut p, rig, "l", 22.0 - sway * 3.0, 105.0, 14.0);
    p.offset(rig, "root", v3(0.0, breathe * st.bob * 0.3, 0.0));
    p.scale(
        rig,
        "chest",
        v3(
            1.0 + breathe * 0.015,
            1.0 + breathe * 0.02,
            1.0 + breathe * 0.015,
        ),
    );
    p
}

/// Walk/run cycle at `phase` (0..1). `run` widens everything.
pub fn gait(rig: &Rig, st: &AnimStyle, phase: f32, run: bool) -> Pose {
    let mut p = rig.rest_pose();
    let (amp, arm_amp, bend, lean_deg) = if run {
        (48.0 * st.swing, 40.0 * st.swing, 85.0, st.run_lean)
    } else {
        (26.0 * st.swing, 18.0 * st.swing, 40.0, 4.0)
    };
    let a = phase * TAU;
    let s = a.sin();
    let c = a.cos();
    // Right leg leads on sin, left is half a cycle behind. Knee bends while
    // the thigh moves forward (cos > 0 for right, < 0 for left).
    leg(&mut p, rig, "r", amp * s, -bend * c.max(0.0), None);
    leg(&mut p, rig, "l", -amp * s, -bend * (-c).max(0.0), None);
    // Arms opposite to legs.
    let fore = if run { 80.0 } else { 25.0 };
    arm(&mut p, rig, "r", -arm_amp * s, fore, 6.0);
    arm(&mut p, rig, "l", arm_amp * s, fore, 6.0);
    lean(&mut p, rig, lean_deg);
    p.rot(rig, "head", 0.0, 0.0, lean_deg * 0.5);
    p.rot(rig, "hips", 0.0, 0.0, if run { 2.0 * s } else { 0.0 });
    // Two bobs per cycle.
    let bob = st.bob * if run { 1.4 } else { 0.7 };
    p.offset(
        rig,
        "root",
        v3(0.0, -bob * (2.0 * a).cos() * 0.5 + bob * 0.5, 0.0),
    );
    p
}

pub fn crouch(rig: &Rig, st: &AnimStyle) -> Pose {
    let mut p = rig.rest_pose();
    let d = st.crouch_depth;
    leg(&mut p, rig, "r", 62.0, -104.0, Some(42.0));
    leg(&mut p, rig, "l", 48.0, -100.0, Some(52.0));
    lean(&mut p, rig, 24.0);
    p.rot(rig, "head", 0.0, 0.0, -10.0);
    arm(&mut p, rig, "r", 40.0, 70.0, 10.0);
    arm(&mut p, rig, "l", 30.0, 80.0, 16.0);
    p.offset(rig, "root", v3(0.0, -d, 0.0));
    p
}

/// Airborne poses.
pub fn air(rig: &Rig, rising: bool, fastfall: bool) -> Pose {
    let mut p = rig.rest_pose();
    if rising {
        leg(&mut p, rig, "r", 55.0, -80.0, Some(20.0));
        leg(&mut p, rig, "l", 20.0, -50.0, Some(20.0));
        arm(&mut p, rig, "r", -60.0, 40.0, 30.0);
        arm(&mut p, rig, "l", 80.0, 60.0, 30.0);
        lean(&mut p, rig, -6.0);
    } else if fastfall {
        leg(&mut p, rig, "r", -8.0, -6.0, Some(-30.0));
        leg(&mut p, rig, "l", 6.0, -6.0, Some(-30.0));
        arm(&mut p, rig, "r", -30.0, 10.0, 25.0);
        arm(&mut p, rig, "l", -30.0, 10.0, 25.0);
        lean(&mut p, rig, 10.0);
    } else {
        leg(&mut p, rig, "r", 25.0, -35.0, Some(-10.0));
        leg(&mut p, rig, "l", -15.0, -30.0, Some(-10.0));
        arm(&mut p, rig, "r", -20.0, 30.0, 55.0);
        arm(&mut p, rig, "l", -20.0, 30.0, 55.0);
        lean(&mut p, rig, -4.0);
    }
    p
}

pub fn shield(rig: &Rig, st: &AnimStyle) -> Pose {
    let mut p = rig.rest_pose();
    leg(&mut p, rig, "r", 30.0, -50.0, None);
    leg(&mut p, rig, "l", 10.0, -45.0, None);
    lean(&mut p, rig, 14.0);
    p.rot(rig, "head", 0.0, 0.0, -12.0);
    arm(&mut p, rig, "r", 55.0, 120.0, -25.0);
    arm(&mut p, rig, "l", 65.0, 115.0, -20.0);
    p.offset(rig, "root", v3(0.0, -st.crouch_depth * 0.35, 0.0));
    p
}

pub fn flinch(rig: &Rig, strength: f32) -> Pose {
    let mut p = rig.rest_pose();
    let k = strength.clamp(0.3, 1.0);
    leg(&mut p, rig, "r", 25.0 * k, -40.0 * k, None);
    leg(&mut p, rig, "l", -20.0 * k, -30.0 * k, None);
    lean(&mut p, rig, -22.0 * k);
    p.rot(rig, "head", 0.0, 0.0, 25.0 * k);
    arm(&mut p, rig, "r", 70.0 * k, 40.0, 40.0 * k);
    arm(&mut p, rig, "l", 50.0 * k, 50.0, 40.0 * k);
    p
}

pub fn tumble_limbs(rig: &Rig) -> Pose {
    let mut p = rig.rest_pose();
    leg(&mut p, rig, "r", 45.0, -30.0, Some(-20.0));
    leg(&mut p, rig, "l", -35.0, -20.0, Some(-20.0));
    arm(&mut p, rig, "r", -120.0, 30.0, 40.0);
    arm(&mut p, rig, "l", 100.0, 40.0, 40.0);
    lean(&mut p, rig, -10.0);
    p.rot(rig, "head", 0.0, 0.0, 18.0);
    p
}

pub fn lying(rig: &Rig, thickness: f32) -> Pose {
    let mut p = rig.rest_pose();
    both_legs(&mut p, rig, 8.0, -6.0);
    arm(&mut p, rig, "r", 40.0, 30.0, 70.0);
    arm(&mut p, rig, "l", 30.0, 20.0, 70.0);
    p.rot(rig, "head", 0.0, 0.0, 15.0);
    // Lie on the back, head behind.
    if let Some(i) = rig.bone("root") {
        p.rot[i] = v3(0.0, 0.0, 90.0);
        p.off[i] = v3(0.0, thickness, 0.0);
    }
    p
}

pub fn ledge_hang(rig: &Rig) -> Pose {
    let mut p = rig.rest_pose();
    both_arms(&mut p, rig, 165.0, -10.0, 4.0);
    leg(&mut p, rig, "r", 10.0, -25.0, Some(-20.0));
    leg(&mut p, rig, "l", -5.0, -15.0, Some(-20.0));
    lean(&mut p, rig, 8.0);
    p.rot(rig, "head", 0.0, 0.0, -20.0);
    p
}

pub fn grab_reach(rig: &Rig) -> Pose {
    let mut p = rig.rest_pose();
    leg(&mut p, rig, "r", 30.0, -30.0, None);
    leg(&mut p, rig, "l", -20.0, -20.0, None);
    lean(&mut p, rig, 14.0);
    both_arms(&mut p, rig, 95.0, 5.0, 12.0);
    p
}

pub fn held(rig: &Rig) -> Pose {
    let mut p = rig.rest_pose();
    both_legs(&mut p, rig, 10.0, -35.0);
    arm(&mut p, rig, "r", 20.0, 60.0, 50.0);
    arm(&mut p, rig, "l", 15.0, 70.0, 50.0);
    p.rot(rig, "head", 0.0, 0.0, 12.0);
    p.offset(rig, "root", v3(0.0, 3.0, 0.0));
    p
}

// ----------------------------------------------------------------- attacks

/// Which limb delivers a move (drives the aimed strike pose).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Limb {
    ArmR,
    ArmL,
    BothArms,
    LegR,
    LegL,
    BothLegs,
    /// Whole-body lunge (shoulder / hip).
    Body,
}

/// Per-move shape hints. Frame timing comes from the move data itself.
#[derive(Clone, Copy, Debug)]
pub struct Strike {
    pub limb: Limb,
    /// Wind-up pull-back (degrees away from the strike direction).
    pub windup: f32,
    /// Body lean into the strike (degrees, + forward).
    pub lean: f32,
    /// Hip drop (sim units) during the strike.
    pub crouch: f32,
    /// Torso twist about Y (degrees).
    pub twist: f32,
}

pub fn strike_spec(id: MoveId) -> Strike {
    use Limb::*;
    let s = |limb, windup, lean, crouch, twist| Strike {
        limb,
        windup,
        lean,
        crouch,
        twist,
    };
    match id {
        MoveId::Jab => s(ArmR, 25.0, 6.0, 0.0, 20.0),
        MoveId::Ftilt => s(LegR, 40.0, 4.0, 1.0, 10.0),
        MoveId::Utilt => s(LegR, 50.0, -10.0, 1.5, 0.0),
        MoveId::Dtilt => s(LegR, 30.0, 18.0, 6.0, 0.0),
        MoveId::Fsmash => s(BothArms, 70.0, 16.0, 2.0, 45.0),
        MoveId::Usmash => s(ArmR, 80.0, -14.0, 0.0, 10.0),
        MoveId::Dsmash => s(BothLegs, 40.0, 10.0, 7.0, 0.0),
        MoveId::DashAttack => s(Body, 30.0, 30.0, 4.0, 20.0),
        MoveId::Nair => s(BothLegs, 30.0, 0.0, 0.0, 40.0),
        MoveId::Fair => s(LegR, 45.0, 8.0, 0.0, 15.0),
        MoveId::Bair => s(LegL, 45.0, -10.0, 0.0, -30.0),
        MoveId::Uair => s(LegR, 60.0, -25.0, 0.0, 0.0),
        MoveId::Dair => s(BothLegs, 40.0, 12.0, 0.0, 0.0),
        MoveId::SpecialN => s(BothArms, 40.0, 8.0, 0.0, 0.0),
        MoveId::SpecialUp => s(BothArms, 60.0, -20.0, 0.0, 60.0),
        MoveId::SpecialSide => s(Body, 50.0, 26.0, 2.0, 30.0),
        MoveId::SpecialDown => s(BothArms, 60.0, 20.0, 8.0, 0.0),
        MoveId::ThrowF | MoveId::ThrowB => s(BothArms, 40.0, 10.0, 0.0, 40.0),
        MoveId::ThrowU => s(BothArms, 40.0, -20.0, 0.0, 0.0),
        MoveId::ThrowD => s(BothArms, 40.0, 30.0, 6.0, 0.0),
    }
}

/// The strike pose for a limb aimed at `aim_deg` (0 = forward, 90 = up), on
/// top of `base`. `extend` (0..1) pushes the limb from wind-up to full reach.
fn aimed(rig: &Rig, base: &Pose, spec: &Strike, aim_deg: f32, extend: f32, airborne: bool) -> Pose {
    let mut p = base.clone();
    // Direction the limb tip should point: a hanging limb points down (-90°);
    // rotating +z by (aim + 90) points it at `aim`. Wind-up pulls the other
    // way; `extend` blends between them.
    let target = aim_deg + 90.0;
    let pull = target - spec.windup - 30.0;
    let z = pull + (target - pull) * extend;
    let fore_windup = 80.0;
    let fore = fore_windup * (1.0 - extend);
    let plant = |p: &mut Pose, side: &str| {
        // Supporting leg braced.
        if !airborne {
            leg(p, rig, side, -8.0, -28.0, None);
        }
    };
    match spec.limb {
        Limb::ArmR => {
            arm(&mut p, rig, "r", z, fore, 6.0);
            arm(&mut p, rig, "l", 25.0 - 20.0 * extend, 100.0, 12.0);
        }
        Limb::ArmL => {
            arm(&mut p, rig, "l", z, fore, 6.0);
            arm(&mut p, rig, "r", 25.0 - 20.0 * extend, 100.0, 12.0);
        }
        Limb::BothArms => {
            arm(&mut p, rig, "r", z, fore, 4.0);
            arm(&mut p, rig, "l", z - 8.0, fore, 12.0);
        }
        Limb::LegR => {
            leg(&mut p, rig, "r", z, -fore, Some(-15.0 * extend));
            plant(&mut p, "l");
            arm(&mut p, rig, "r", -30.0 * extend, 60.0, 20.0);
            arm(&mut p, rig, "l", 40.0 * extend, 70.0, 20.0);
        }
        Limb::LegL => {
            leg(&mut p, rig, "l", z, -fore, Some(-15.0 * extend));
            plant(&mut p, "r");
            arm(&mut p, rig, "l", -30.0 * extend, 60.0, 20.0);
            arm(&mut p, rig, "r", 40.0 * extend, 70.0, 20.0);
        }
        Limb::BothLegs => {
            // Split: one leg at the aim, the other mirrored behind.
            leg(&mut p, rig, "r", z, -fore * 0.5, Some(-10.0));
            let back = -(aim_deg + 90.0) - 20.0;
            let zb = pull * 0.3 + (back - pull * 0.3) * extend;
            leg(&mut p, rig, "l", zb, -fore * 0.5, Some(-10.0));
            both_arms(&mut p, rig, -40.0 * extend, 40.0, 45.0 * extend);
        }
        Limb::Body => {
            // Shoulder / hip lunge: everything leans in, arms trail.
            leg(&mut p, rig, "r", 40.0 * extend, -30.0, None);
            leg(&mut p, rig, "l", -35.0 * extend, -15.0, None);
            arm(&mut p, rig, "r", 60.0 * extend, 90.0, 10.0);
            arm(&mut p, rig, "l", -50.0 * extend, 40.0, 20.0);
        }
    }
    // Body follows the strike.
    let l = -spec.lean * (1.0 - extend) * 0.4 + spec.lean * extend;
    lean(&mut p, rig, l);
    p.add(
        rig,
        "chest",
        v3(0.0, spec.twist * (extend * 2.0 - 1.0), 0.0),
    );
    if let Some(i) = rig.bone("root") {
        p.off[i] = p.off[i] + v3(0.0, -spec.crouch * extend, 0.0);
    }
    p
}

/// The whole attack timeline: `sf` is the frame within the move.
pub fn attack(rig: &Rig, f: &Fighter, id: MoveId, sf: u32, base: &Pose, heavy: f32) -> Pose {
    let md = attacks::data(f.character.id, id);
    let spec = strike_spec(id);
    let hb = md.hitbox.offset;
    let aim = hb.y.atan2(hb.x.max(0.01)).to_degrees();
    let airborne = md.is_aerial || !f.grounded;
    let windup = aimed(rig, base, &spec, aim, 0.0, airborne);
    let strike = aimed(rig, base, &spec, aim, 1.0, airborne);
    let overshoot = aimed(rig, base, &spec, aim, 1.12, airborne);
    let st = md.startup.max(1) as f32;
    if sf < md.startup {
        let t = sf as f32 / st;
        // First 45 % pull back, then accelerate into the strike.
        if t < 0.45 {
            base.blend(&windup, ease(t / 0.45))
        } else {
            let k = ease_in((t - 0.45) / 0.55);
            let k = k * (1.0 - heavy * 0.25) + heavy * 0.25 * k * k;
            windup.blend(&strike, k)
        }
    } else if sf < md.startup + md.active {
        let t = (sf - md.startup) as f32 / md.active.max(1) as f32;
        strike.blend(&overshoot, ease_out(t))
    } else {
        let t = (sf - md.startup - md.active) as f32 / md.endlag.max(1) as f32;
        overshoot.blend(base, ease(t * (1.0 - heavy * 0.3) + heavy * 0.3 * t * t))
    }
}

// ----------------------------------------------------------------- driver

/// Compute the pose for a fighter's current state. `frame` is the global
/// match frame (for idle timing).
pub fn fighter_pose(rig: &Rig, f: &Fighter, st: &AnimStyle, frame: u64) -> Pose {
    let t = frame as f32;
    let sf = f.state_frame;
    let hip_h = rig
        .bone("hips")
        .map(|i| rig.bones[i].offset.y)
        .unwrap_or(15.0);
    let centre = f.character.height * 0.5;
    let base_ground = stance(rig, st, t);
    let base_air = air(rig, f.vel.y > 0.5, f.fastfalling);

    match f.state {
        State::Stand => base_ground,
        State::Walk => {
            let ph = (t / st.walk_cycle).fract();
            gait(rig, st, ph, false)
        }
        State::Dash => {
            // Burst: deep lean settling into the run cycle.
            let ph = (sf as f32 / st.run_cycle).fract();
            let mut p = gait(rig, st, ph, true);
            let k = 1.0 - (sf as f32 / 8.0).min(1.0);
            lean(&mut p, rig, 12.0 * k);
            p.offset(rig, "root", v3(0.0, -2.0 * k, 0.0));
            p
        }
        State::Run => {
            let ph = (t / st.run_cycle).fract();
            gait(rig, st, ph, true)
        }
        State::Crouch => {
            let c = crouch(rig, st);
            base_ground.blend(&c, ease(sf as f32 / 4.0))
        }
        State::JumpSquat => {
            let c = crouch(rig, st);
            let k = ease((sf as f32 + 1.0) / f.character.jumpsquat.max(1) as f32);
            let mut p = base_ground.blend(&c, k * 0.8);
            root_scale(&mut p, rig, 1.0 + 0.08 * k, 1.0 - 0.10 * k);
            p
        }
        State::Air => {
            let mut p = base_air;
            // Stretch on take-off, settle after a few frames.
            if sf < 6 && f.vel.y > 0.5 {
                let k = 1.0 - sf as f32 / 6.0;
                root_scale(&mut p, rig, 1.0 - 0.06 * k, 1.0 + 0.10 * k);
            }
            p
        }
        State::LandLag { total } => {
            let c = crouch(rig, st);
            let k = 1.0 - (sf as f32 / total.max(1) as f32);
            let mut p = base_ground.blend(&c, ease(k) * 0.7);
            let sq = ease(k) * 0.12;
            root_scale(&mut p, rig, 1.0 + sq, 1.0 - sq);
            p
        }
        State::Waveland => {
            let c = crouch(rig, st);
            let mut p = base_ground.blend(&c, 0.6);
            lean(&mut p, rig, 10.0);
            p
        }
        State::Attack { id, .. } => {
            let base = if f.grounded { &base_ground } else { &base_air };
            let mut p = attack(rig, f, id, sf, base, st.heavy);
            if f.charge > 0 && f.charge_armed {
                // Charging a smash: the wind-up trembles harder as it fills
                // and the body sinks a touch.
                let c = f.charge as f32;
                let amp = 0.6 + c / 60.0 * 1.4;
                let j = (c * 2.7).sin() * amp;
                p.offset(
                    rig,
                    "root",
                    v3(
                        j * 0.5,
                        -st.crouch_depth * 0.15 * (c / 60.0) - j.abs() * 0.3,
                        0.0,
                    ),
                );
            }
            p
        }
        State::Shield => {
            let s = shield(rig, st);
            base_ground.blend(&s, ease(sf as f32 / 3.0))
        }
        State::ShieldStun { .. } => {
            let mut p = shield(rig, st);
            let j = ((sf as f32) * 2.1).sin() * 1.2;
            p.offset(
                rig,
                "root",
                v3(-f.facing * j.abs() * 0.5, -st.crouch_depth * 0.35, 0.0),
            );
            p
        }
        State::Roll { dir } => {
            // Tuck and tumble a full turn in the roll direction.
            let dur = crate::sim::constants::ROLL_DURATION.max(1) as f32;
            let k = (sf as f32 / dur).min(1.0);
            let c = crouch(rig, st);
            let mut p = base_ground.blend(&c, 0.9);
            spin_body(&mut p, rig, -dir * f.facing * 360.0 * ease(k), centre);
            p
        }
        State::Spotdodge => {
            let mut p = shield(rig, st);
            let k = (sf as f32 / 6.0).min(1.0);
            p.add(rig, "chest", v3(0.0, 70.0 * k, 0.0));
            p.add(rig, "hips", v3(0.0, 40.0 * k, 0.0));
            lean(&mut p, rig, -10.0 * k);
            p
        }
        State::Airdodge => {
            let mut p = tumble_limbs(rig);
            both_legs(&mut p, rig, 50.0, -80.0);
            both_arms(&mut p, rig, 60.0, 90.0, 30.0);
            let dur = crate::sim::constants::AIRDODGE_INTANGIBLE.1 as f32;
            let k = (sf as f32 / dur).min(1.0);
            spin_body(&mut p, rig, -f.facing * 360.0 * ease(k), centre);
            p
        }
        State::Helpless => {
            // Limp fall: arms up, legs dangling, slight backward lean.
            let mut p = rig.rest_pose();
            leg(&mut p, rig, "r", 20.0, -30.0, Some(-20.0));
            leg(&mut p, rig, "l", -10.0, -20.0, Some(-20.0));
            arm(&mut p, rig, "r", -140.0, 20.0, 40.0);
            arm(&mut p, rig, "l", -150.0, 25.0, 40.0);
            lean(&mut p, rig, -14.0);
            p.rot(rig, "head", 0.0, 0.0, 12.0);
            p
        }
        State::ShieldDrop => {
            let s = shield(rig, st);
            s.blend(
                &base_ground,
                ease(sf as f32 / crate::sim::constants::SHIELD_DROP as f32),
            )
        }
        State::Grab => {
            let g = grab_reach(rig);
            base_ground.blend(&g, ease((sf as f32 + 1.0) / 4.0))
        }
        State::Hold => grab_reach(rig),
        State::Grabbed => held(rig),
        State::Throw { id } => attack(rig, f, id, sf, &grab_reach(rig), st.heavy),
        State::LedgeGrab => {
            let mut p = ledge_hang(rig);
            let b = (t * 0.12).sin();
            p.add(rig, "shin_r", v3(0.0, 0.0, b * 4.0));
            p.add(rig, "shin_l", v3(0.0, 0.0, -b * 4.0));
            p
        }
        State::LedgeAction { kind } => {
            // Each option runs on the sim's own (fresh / tired) timeline:
            // hang → pull up over the first two fifths, then the option.
            let (total, _, hit) = f.ledge_option(kind);
            let climb = (total * 2 / 5).max(1) as f32;
            match kind {
                LedgeKind::Getup => {
                    if (sf as f32) < climb {
                        ledge_hang(rig).blend(&crouch(rig, st), ease(sf as f32 / climb))
                    } else {
                        let rest = (total as f32 - climb).max(1.0);
                        let k = ease(((sf as f32 - climb) / rest).min(1.0));
                        crouch(rig, st).blend(&base_ground, k)
                    }
                }
                LedgeKind::Jump => {
                    let up = crate::sim::constants::LEDGE_JUMP.0.max(1) as f32;
                    ledge_hang(rig).blend(&air(rig, true, false), ease((sf as f32 / up).min(1.0)))
                }
                LedgeKind::Roll => {
                    if (sf as f32) < climb {
                        ledge_hang(rig).blend(&crouch(rig, st), ease(sf as f32 / climb))
                    } else {
                        let rest = (total as f32 - climb).max(1.0);
                        let k = ((sf as f32 - climb) / rest).min(1.0);
                        let mut p = crouch(rig, st);
                        spin_body(&mut p, rig, -f.facing * 360.0 * ease(k), centre);
                        p
                    }
                }
                LedgeKind::Attack => {
                    // Strike on the ledge attack's hit frame: shift the tilt's
                    // timeline so its startup ends exactly there.
                    let md = attacks::data(f.character.id, MoveId::Ftilt);
                    let shift = hit.saturating_sub(md.startup);
                    if sf < shift.saturating_sub(6) {
                        ledge_hang(rig)
                    } else {
                        let pre = shift.saturating_sub(6);
                        let k = ease(((sf - pre) as f32 / 6.0).min(1.0));
                        let base = ledge_hang(rig).blend(&base_ground, k);
                        attack(
                            rig,
                            f,
                            MoveId::Ftilt,
                            sf.saturating_sub(shift),
                            &base,
                            st.heavy,
                        )
                    }
                }
            }
        }
        State::Hitstun { tumble } => {
            if tumble {
                let mut p = tumble_limbs(rig);
                let spin = -f.facing * (sf as f32 * 11.0);
                spin_body(&mut p, rig, spin, centre);
                p
            } else {
                let k = 1.0 - (sf as f32 / 14.0).min(1.0);
                flinch(rig, 0.4 + 0.6 * k)
            }
        }
        State::Knockdown => {
            let l = lying(rig, f.character.half_width * 0.9);
            if sf < 6 {
                tumble_limbs(rig).blend(&l, ease(sf as f32 / 6.0))
            } else {
                l
            }
        }
        State::RunTurn => {
            // Brake: plant the feet, lean back against the momentum, then
            // settle into the stance facing the new way.
            let dur = crate::sim::constants::RUN_TURN.max(1) as f32;
            let k = (sf as f32 / dur).min(1.0);
            let ph = (t / st.run_cycle).fract();
            let mut p = gait(rig, st, ph, true).blend(&base_ground, ease(k));
            let brake = (k * std::f32::consts::PI).sin();
            lean(&mut p, rig, -18.0 * brake);
            p.offset(rig, "root", v3(0.0, -st.crouch_depth * 0.25 * brake, 0.0));
            p
        }
        State::Tech { dir } => {
            let c = crouch(rig, st);
            if dir == 0.0 {
                // Tech in place: a quick crouch that springs back up.
                let dur = crate::sim::constants::TECH_IN_PLACE.0.max(1) as f32;
                let k = (sf as f32 / dur).min(1.0);
                c.blend(&base_ground, ease(k))
            } else {
                let dur = crate::sim::constants::TECH_ROLL.0.max(1) as f32;
                let k = (sf as f32 / dur).min(1.0);
                let mut p = base_ground.blend(&c, 0.9);
                spin_body(&mut p, rig, -dir * f.facing * 360.0 * ease(k), centre);
                p
            }
        }
        State::Getup { kind } => {
            let l = lying(rig, f.character.half_width * 0.9);
            match kind {
                GetupKind::Stand => {
                    let dur = crate::sim::constants::GETUP_STAND.0.max(1) as f32;
                    let k = (sf as f32 / dur).min(1.0);
                    l.blend(&crouch(rig, st), ease(k.min(0.6) / 0.6))
                        .blend(&base_ground, ease(((k - 0.6) / 0.4).clamp(0.0, 1.0)))
                }
                GetupKind::Roll { dir } => {
                    let dur = crate::sim::constants::GETUP_ROLL.0.max(1) as f32;
                    let k = (sf as f32 / dur).min(1.0);
                    let mut p = base_ground.blend(&crouch(rig, st), 0.9);
                    spin_body(&mut p, rig, -dir * f.facing * 360.0 * ease(k), centre);
                    l.blend(&p, ease((k * 4.0).min(1.0)))
                }
                GetupKind::Attack => {
                    // Sweep: kick forward, then behind, from a low crouch.
                    let (front, back) = crate::sim::constants::GETUP_ATTACK_HITS;
                    let base = crouch(rig, st);
                    let mut p = l.blend(&base, ease((sf as f32 / 8.0).min(1.0)));
                    let swing = |p: &mut Pose, side: f32, k: f32| {
                        let leg = if side > 0.0 { "thigh_r" } else { "thigh_l" };
                        p.add(rig, leg, v3(0.0, 0.0, -side * 95.0 * k));
                        lean(p, rig, -side * 18.0 * k);
                    };
                    if sf < back - 2 {
                        let k = ((sf as f32 - (front as f32 - 5.0)) / 5.0).clamp(0.0, 1.0);
                        swing(&mut p, 1.0, ease(k));
                    } else {
                        let k = ((sf as f32 - (back as f32 - 3.0)) / 4.0).clamp(0.0, 1.0);
                        swing(&mut p, -1.0, ease(k));
                    }
                    let dur = crate::sim::constants::GETUP_ATTACK.0 as f32;
                    let settle = ((sf as f32 - (back as f32 + 6.0)) / (dur - back as f32 - 6.0))
                        .clamp(0.0, 1.0);
                    p.blend(&base_ground, ease(settle))
                }
            }
        }
        State::Rebound { total } => {
            // Recoil: snap back from the clash, then recover.
            let k = (sf as f32 / total.max(1) as f32).min(1.0);
            let mut p = base_ground.clone();
            let kick = (1.0 - k) * (1.0 - k);
            lean(&mut p, rig, -22.0 * kick);
            p.offset(rig, "root", v3(-f.facing * 2.0 * kick, 0.0, 0.0));
            both_arms(&mut p, rig, 40.0 * kick, 60.0 * kick, 20.0 * kick);
            p
        }
        State::Dead => rig.rest_pose(),
    }
    .with_hip_check(hip_h)
}

trait PoseExt {
    fn with_hip_check(self, hip_h: f32) -> Self;
}
impl PoseExt for Pose {
    /// No-op hook kept so future IK ground clamping can plug in here.
    fn with_hip_check(self, _hip_h: f32) -> Self {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::math3::Xf;

    fn props() -> Proportions {
        Proportions {
            thigh: 7.0,
            shin: 6.5,
            ankle: 1.0,
            torso: 10.0,
            neck: 1.5,
            shoulder_half: 4.0,
            hip_half: 2.2,
            upper_arm: 5.5,
            forearm: 5.0,
        }
    }

    #[test]
    fn skeleton_has_standard_bones() {
        let r = humanoid_skeleton(&props());
        for n in [
            "root",
            "hips",
            "spine",
            "chest",
            "neck",
            "head",
            "upper_arm_r",
            "forearm_l",
            "hand_r",
            "thigh_l",
            "shin_r",
            "foot_l",
        ] {
            assert!(r.bone(n).is_some(), "missing {n}");
        }
        let w = r.world(&r.rest_pose(), &Xf::IDENTITY);
        let head = r.joint(&w, "head").unwrap();
        assert!(head.y > 20.0 && head.y < 40.0, "{head:?}");
        let foot = r.joint(&w, "foot_r").unwrap();
        assert!((foot.y - 1.0).abs() < 1e-4);
    }

    #[test]
    fn strike_pose_aims_at_hitbox() {
        // An up-tilt (aim ≈ up) must put the striking foot above the hips; a
        // down-tilt must put it below.
        let r = humanoid_skeleton(&props());
        let st = AnimStyle::default();
        let base = stance(&r, &st, 0.0);
        let up = aimed(&r, &base, &strike_spec(MoveId::Utilt), 80.0, 1.0, false);
        let down = aimed(&r, &base, &strike_spec(MoveId::Dtilt), -20.0, 1.0, false);
        let wu = r.world(&up, &Xf::IDENTITY);
        let wd = r.world(&down, &Xf::IDENTITY);
        let hips = r.joint(&wu, "hips").unwrap();
        assert!(r.joint(&wu, "foot_r").unwrap().y > hips.y);
        assert!(r.joint(&wd, "foot_r").unwrap().y < hips.y - 3.0);
        assert!(
            r.joint(&wd, "foot_r").unwrap().x > 4.0,
            "sweep reaches forward"
        );
    }

    #[test]
    fn gait_is_periodic_and_alternates() {
        let r = humanoid_skeleton(&props());
        let st = AnimStyle::default();
        let a = gait(&r, &st, 0.25, true);
        let b = gait(&r, &st, 0.75, true);
        let tr = r.bone("thigh_r").unwrap();
        let tl = r.bone("thigh_l").unwrap();
        assert!(
            (a.rot[tr].z - b.rot[tl].z).abs() < 1e-4,
            "half cycle mirrors sides"
        );
        assert!(a.rot[tr].z > 30.0);
        let c = gait(&r, &st, 1.25, true);
        assert!((a.rot[tr].z - c.rot[tr].z).abs() < 1e-3, "periodic");
    }
}
