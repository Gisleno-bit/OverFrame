//! The authored animation directions, implemented
//! (`docs/art/procedural/anim/kestrel.json`, contract `anim/FORMAT.md`).
//!
//! Every action of a fighter that ships a direction file gets its pose from
//! here instead of the generic aimed strike in [`super::anim`]. The three
//! rules the contract insists on are all structural, not cosmetic:
//!
//! * **The contact piece is solved, not guessed.** The strike pose places the
//!   *exact* `contact_piece_id` (its centroid) on the runtime hit region,
//!   by solving the real hip/knee/ankle or shoulder/elbow/wrist chain in the
//!   fighter's own root-local frame. The hitbox offset is used with its sign,
//!   so a rearward move (back air) aims backward; nothing is clamped positive.
//!   When the limb cannot reach, it extends fully along the line to the target
//!   and the shortfall is reported ([`feasibility`]) rather than hidden.
//! * **The runtime owns the timeline.** Anticipation peaks at the authored
//!   fraction of the *available inactive startup* (never adding a frame),
//!   contact holds through the true active interval — `startup + active +
//!   late_active`, the late window included — and recovery starts only after
//!   the real last active tick. A held smash freezes on the anticipation pose
//!   because the simulation freezes `state_frame` there.
//! * **Special cases use their real events.** The projectile action aims at
//!   the runtime spawn point (and never chases the projectile afterwards),
//!   the radial action centres the body in its own effect instead of pushing
//!   a hand to the boundary, and throws work from the hold position and the
//!   runtime release parameters. None of them grow a fictitious melee hitbox.
//!
//! Nothing here reads or writes simulation state: no damage, frame data,
//! hitbox, capsule, model scale or bone length is touched. Poses are built in
//! the root-local frame (the drawn facing is a rotation of the whole root, see
//! [`super::render_eval`]), so the same pose serves both facings and the
//! measurements come out facing-symmetric.

use super::anim;
use super::anim_dir::{Directions, PoseFamily, Resolved};
use super::characters::CharacterModel;
use super::math3::{ease, ease_in, ease_out, v3, Xf, M3, V3};
use super::rig::{Pose, Rig};
use crate::sim::attacks::{self, MoveData, MoveId};
use crate::sim::fighter::{Fighter, State};
use crate::sim::roster::CharacterId;

/// Where the simulation holds a grabbed fighter, relative to the holder's
/// root and facing (`sim::state::GameState::update_grabs`). Read-only
/// knowledge of the sim: the throw poses aim the hands at the real hold
/// position instead of inventing one.
pub const HOLD_LOCAL_X: f32 = 16.0;
/// Where `SpecialN` spawns its projectile relative to the root and facing
/// (`sim::state::GameState::step`, spawn requests).
pub const PROJECTILE_SPAWN_LOCAL_X: f32 = 14.0;

/// When the recoil of a release-only action reaches its peak, and when the
/// body has settled back into guard — both as a fraction of the move's real
/// total length, so they scale with whatever the move table says.
///
/// These two are **authored presentation beats**, not runtime data: nothing
/// in the simulation changes shape, and the action's lockout is exactly the
/// move table's. They only decide when the drawn hand stops moving, which
/// the direction explicitly allows to finish before the lockout does.
const RECOIL_PEAK: f32 = 0.16;
const SETTLED_BY: f32 = 0.62;

/// Does this action's whole presentation consist of one release? True when
/// the simulation spawns a projectile for it **and** the move table gives
/// it no fighter hitbox on any frame — Kestrel's `special_n` after the
/// gameplay correction. Boulder's and Viper's keep their real hitbox, so
/// they keep the ordinary contact/follow-through timeline and labels.
pub fn is_release_only(ch: CharacterId, id: MoveId) -> bool {
    crate::export::action_spawns_projectile(id) && attacks::data(ch, id).no_melee
}

/// Which beat of a release-only action a state frame is in.
///
/// These are the *same* numbers the pose is built from, so a caption can
/// never claim a beat the animation is not in. They are presentation, not
/// simulation: the action's commitment is the move table's, untouched.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Beat {
    /// The shot leaves the palm.
    Emission,
    /// Palm and elbow withdrawing towards the ribs.
    Recoil,
    /// Settling back into guard, and holding it out for the lockout.
    Recovery,
}

/// The beat of `state_frame`, given the move it belongs to.
pub fn release_beat(md: &MoveData, state_frame: u32) -> Beat {
    let total = md.total().max(1) as f32;
    let peak = (total * RECOIL_PEAK).max(1.0);
    let t = state_frame.saturating_sub(1) as f32;
    if state_frame <= 1 {
        Beat::Emission
    } else if t <= peak {
        Beat::Recoil
    } else {
        Beat::Recovery
    }
}

// ----------------------------------------------------------------- 2D helpers

/// Unit direction of a bone at absolute Z angle `deg`: a bone hangs down at
/// 0° and swings its tip toward +X at 90° (the convention `anim` keys in).
#[inline]
fn dir(deg: f32) -> [f32; 2] {
    let (s, c) = deg.to_radians().sin_cos();
    [s, -c]
}

/// Inverse of [`dir`]: the absolute Z angle of the vector `(x, y)`.
#[inline]
fn ang(x: f32, y: f32) -> f32 {
    x.atan2(-y).to_degrees()
}

#[inline]
fn wrap180(mut a: f32) -> f32 {
    while a > 180.0 {
        a -= 360.0;
    }
    while a < -180.0 {
        a += 360.0;
    }
    a
}

#[inline]
fn rot_z_xy(deg: f32, p: V3) -> [f32; 2] {
    let (s, c) = deg.to_radians().sin_cos();
    [p.x * c - p.y * s, p.x * s + p.y * c]
}

/// Cumulative Z rotation of a transform whose rotation is Z-only (the
/// directed poses keep every ancestor of a solved chain planar, so the
/// projected chain is a true 2D articulated chain).
#[inline]
fn z_of(m: &M3) -> f32 {
    m.m[1][0].atan2(m.m[0][0]).to_degrees()
}

// ----------------------------------------------------------------- the chain

/// A solved limb: the two long links plus the short rigid step from the tip
/// joint to the contact piece.
#[derive(Clone, Copy, Debug)]
struct Chain {
    upper: usize,
    fore: usize,
    tip: usize,
    parent: Option<usize>,
    l1: f32,
    l2: f32,
}

fn chain(rig: &Rig, tip_bone: usize) -> Option<Chain> {
    let fore = rig.bones[tip_bone].parent?;
    let upper = rig.bones[fore].parent?;
    Some(Chain {
        upper,
        fore,
        tip: tip_bone,
        parent: rig.bones[upper].parent,
        l1: rig.bones[fore].offset.len(),
        l2: rig.bones[tip_bone].offset.len(),
    })
}

/// What a solve achieved, in world units, for the feasibility report.
#[derive(Clone, Copy, Debug, Default)]
pub struct Reach {
    /// Distance from the limb's root joint to the aim point.
    pub required: f32,
    /// Longest distance that chain can put the piece centroid at.
    pub max_reach: f32,
    /// Distance from the achieved piece centroid to the aim point.
    pub achieved: f32,
    /// The chain could place the centroid exactly on the aim point.
    pub reached: bool,
}

/// Elbows bend forward (positive local Z), knees bend backward (negative).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Bend {
    Elbow,
    Knee,
}

impl Bend {
    fn sign(self) -> f32 {
        match self {
            Bend::Elbow => 1.0,
            Bend::Knee => -1.0,
        }
    }
}

/// Put `effector` (a point in the tip bone's frame) on `target`
/// (root-local XY) by rotating the chain about Z only.
///
/// Exact two-link inverse kinematics in the XY fighting plane — the plane the
/// simulation tests and the plane `model::contact` projects onto. When the
/// target is out of reach the limb extends fully along the line to it (never
/// past it, never toward a mirrored target), and `reached` says so.
#[allow(clippy::too_many_arguments)]
fn solve(
    rig: &Rig,
    pose: &mut Pose,
    ch: &Chain,
    effector: V3,
    target: [f32; 2],
    bend: Bend,
    tip_abs: Option<f32>,
    tip_local: f32,
) -> Reach {
    let mut c = tip_local;
    let mut out = Reach::default();
    // Two passes: the short tip link changes the effective second link, and
    // an absolute tip orientation depends on the solved angles.
    for _ in 0..3 {
        pose.rot[ch.tip] = v3(0.0, 0.0, c);
        let world = rig.world(pose, &Xf::IDENTITY);
        let s = world[ch.upper].t;
        let phi = ch.parent.map(|p| z_of(&world[p].m)).unwrap_or(0.0);
        // Vector from the fore joint to the effector, in the fore bone's own
        // frame: down the second link, then the rotated tip offset.
        let e = rot_z_xy(c, effector);
        let q = [e[0], -ch.l2 + e[1]];
        let l2eff = (q[0] * q[0] + q[1] * q[1]).sqrt();
        let gamma = ang(q[0], q[1]);
        let d = [target[0] - s.x, target[1] - s.y];
        let required = (d[0] * d[0] + d[1] * d[1]).sqrt();
        let rmax = ch.l1 + l2eff - 1e-3;
        let rmin = (ch.l1 - l2eff).abs() + 1e-3;
        let r = required.clamp(rmin, rmax);
        let beta = ang(d[0], d[1]);
        // Aim point actually solved for: the target itself when in reach,
        // otherwise the farthest point of the same line.
        let tgt = [s.x + r * dir(beta)[0], s.y + r * dir(beta)[1]];
        let cos_a = ((r * r + ch.l1 * ch.l1 - l2eff * l2eff) / (2.0 * r * ch.l1)).clamp(-1.0, 1.0);
        let alpha = cos_a.acos().to_degrees();
        // Pick the side whose joint bend is anatomically legal; if both or
        // neither are, take the straighter one.
        let mut best: Option<(f32, f32, bool)> = None;
        for sign in [1.0f32, -1.0] {
            let a_abs = beta + sign * alpha;
            let du = dir(a_abs);
            let elb = [s.x + ch.l1 * du[0], s.y + ch.l1 * du[1]];
            let psi = ang(tgt[0] - elb[0], tgt[1] - elb[1]);
            let b = wrap180(psi - gamma - a_abs);
            let legal = b * bend.sign() >= -1e-3;
            let better = match best {
                None => true,
                Some((_, bb, blegal)) => match (legal, blegal) {
                    (true, false) => true,
                    (false, true) => false,
                    _ => b.abs() < bb.abs(),
                },
            };
            if better {
                best = Some((a_abs, b, legal));
            }
        }
        let (a_abs, b, _) = best.unwrap();
        pose.rot[ch.upper] = v3(0.0, 0.0, wrap180(a_abs - phi));
        pose.rot[ch.fore] = v3(0.0, 0.0, b);
        if let Some(t) = tip_abs {
            c = wrap180(t - (a_abs + b));
        }
        pose.rot[ch.tip] = v3(0.0, 0.0, c);
        out.required = required;
        out.max_reach = ch.l1 + l2eff;
        out.reached = required <= rmax && required >= rmin;
    }
    // What the piece centroid really ended up at.
    let world = rig.world(pose, &Xf::IDENTITY);
    let p = world[ch.tip].point(effector);
    out.achieved = ((p.x - target[0]).powi(2) + (p.y - target[1]).powi(2)).sqrt();
    out
}

// ----------------------------------------------------------------- shapes

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stage {
    /// The anticipation peak (also the pose a held smash freezes on).
    Wind,
    /// First contact.
    Contact,
    /// Follow-through held through the clean window.
    Ext,
    /// Joint folded first, before the body settles.
    Fold,
    /// The withdrawal right after a release: palm and elbow pulled back
    /// towards the ribs, with a small opposing shoulder motion. Only a
    /// release-only action reaches this stage -- an action with a real
    /// fighter hitbox still goes Contact -> Ext -> Fold, because its
    /// follow-through is part of the hit.
    Recoil,
}

/// Per-family shape: everything the prose fixes that is not the contact
/// solve itself (torso, hips, tip orientation, coil, head).
#[derive(Clone, Copy, Debug)]
struct Shape {
    lean: f32,
    lean_wind: f32,
    /// Extra forward lean the builder may add — in measured steps, only when
    /// the authored lean leaves the piece outside the hit region.
    lean_assist: f32,
    crouch: f32,
    /// Absolute orientation of the contact bone at contact (`None` keeps the
    /// local angle): feet lead with the sole, fists follow the forearm.
    tip_abs: Option<f32>,
    tip_local: f32,
    /// Where the piece coils at the anticipation peak, relative to the
    /// limb's root joint.
    coil: [f32; 2],
    /// Head yaw (about Y): "look back", "keep the face toward the strike".
    head_yaw: f32,
    follow_lean: f32,
    follow_tip: f32,
    /// Fraction (0..1) by which the Contact/Ext solve target is pulled from
    /// the true aim point back toward the limb root before solving. Zero
    /// for every family but one: when the aim point sits beyond the limb's
    /// maximum reach, the two-link solve degenerates to a fully extended,
    /// unbent line to it (see `solve`'s `rmax` clamp) -- an authored
    /// "bent forearm" direction cannot show up as anything but a straight
    /// arm in that case. Pulling the *solve* target closer (along the same
    /// line, so the limb still reaches toward the real region) lets the
    /// solver find a genuine bend; `staged` re-measures the achieved piece
    /// against the real, unpulled aim point afterwards, so intersection is
    /// still judged against the true hit region, never the decoy target.
    aim_pull: f32,
    /// `Stage::Contact`/`Ext` only: root-offset override, replacing `crouch`
    /// for those two stages (Wind still uses `crouch * 0.5`, Fold `crouch *
    /// 0.4`). `None` keeps the old single-curve behaviour (every stage
    /// scales the same `crouch`). Some families need compression already
    /// visible at Wind and a real, distinct extension by first active --
    /// two different depths, not one depth on a shared curve.
    crouch_active: Option<f32>,
    /// Root-local XY offset added to the true hitbox centre (`Aim::contact`)
    /// to get the Contact/Ext solve target — an authored point *inside* the
    /// hit region, not necessarily its centre. `[0.0, 0.0]` for every family
    /// but one. `achieved`/`reached` are re-measured against the real,
    /// unshifted `aim.contact` afterward (same safety net `aim_pull` uses),
    /// so "intersects" always means the true hit region, never the decoy
    /// point actually solved for.
    contact_offset: [f32; 2],
    /// Treat this family's lean values as the **final** chest line rather
    /// than an assist added on top of whatever the base pose already has.
    ///
    /// `anim::lean` adds to `spine` and `chest`, and the bases do not
    /// agree: the standing stance breathes around +7 degrees, the rising
    /// air pose sits at -6, an ordinary fall at -4 and a fastfall at +10.
    /// Adding the same number to all of them gives four different torsos
    /// for one action, which is the opposite of "ground and air share the
    /// upper-body identity". Reading back what the base already put in and
    /// subtracting it fixes that inside this pose only -- `anim::air`, the
    /// root, the bones, the legs and every other action are untouched.
    lean_absolute: bool,
    /// Solve the contact at a **fixed distance** along the shoulder -> aim
    /// line instead of at the aim itself (`Stage::Contact`/`Ext`).
    ///
    /// `aim_pull` takes a fraction of that line, so the same fraction
    /// leaves a different elbow bend wherever the base pose puts the
    /// shoulder -- Kestrel's airborne base sits further back than its
    /// stance, which straightened the arm again in the air. A fixed
    /// distance keeps the bend identical on the ground and in the air,
    /// which is what "ground and air share the upper-body identity" means
    /// for an arm whose target it cannot reach either way. `achieved` is
    /// still re-measured against the true aim, exactly as with `aim_pull`.
    aim_reach: Option<f32>,
    /// Where the piece withdraws to after a release, relative to the limb's
    /// root joint (`Stage::Recoil`). `None` reuses the anticipation coil —
    /// which points *behind* the shoulder, the right place to wind up from
    /// and the wrong place to recoil to. Only a release-only action ever
    /// reaches that stage.
    recoil: Option<[f32; 2]>,
}

#[allow(clippy::too_many_arguments)]
const fn sh(
    lean: f32,
    lean_wind: f32,
    lean_assist: f32,
    crouch: f32,
    tip_abs: Option<f32>,
    tip_local: f32,
    coil: [f32; 2],
    head_yaw: f32,
) -> Shape {
    Shape {
        lean,
        lean_wind,
        lean_assist,
        crouch,
        tip_abs,
        tip_local,
        coil,
        head_yaw,
        follow_lean: 3.0,
        follow_tip: 5.0,
        aim_pull: 0.0,
        crouch_active: None,
        contact_offset: [0.0, 0.0],
        lean_absolute: false,
        aim_reach: None,
        recoil: None,
    }
}

fn shape(f: PoseFamily) -> Shape {
    use PoseFamily::*;
    match f {
        // "From the compact forward guard straight out and back along the
        // same short line"; the chest leans only as far as the fist needs.
        Punch => sh(7.0, -4.0, 10.0, 0.0, None, 8.0, [-3.5, -2.0], 0.0),
        // "Lift the right knee, extend the sole forward … then fold back."
        Kick => sh(6.0, -6.0, 10.0, 1.0, Some(0.0), 0.0, [2.0, -7.0], 0.0),
        // "Coil the right fist close to the waist, then cut upward."
        Uppercut => sh(-8.0, 12.0, 8.0, 0.0, None, -20.0, [1.5, -6.5], 0.0),
        // "Fold the support knee and skim the right sole forward."
        LowKick => sh(16.0, 2.0, 10.0, 5.0, Some(-8.0), 0.0, [1.0, -7.0], 0.0),
        // "Draw the right palm near the rear shoulder, then drive it."
        PalmDrive => sh(20.0, -12.0, 16.0, 2.0, None, 0.0, [-5.0, -1.0], 0.0),
        // "Compress the right arm at the chest then thrust upward" --
        // 2026-09 review: usmash was compressing MORE through the active
        // window and never read as an upward release. Flip the curve: real,
        // deep compression is already visible at Wind (startup/charge,
        // `crouch * 0.5` off a bigger `crouch`), and `crouch_active` gives
        // Contact/Ext their own, separate value -- full neutral standing
        // height, a genuine extension relative to the Wind compression,
        // never a pop above it (which would float the feet; `support_lift`
        // only ever corrects a sunk foot, never a floating one).
        OverheadDrive => Shape {
            crouch_active: Some(0.0),
            ..sh(-10.0, 10.0, 8.0, 16.0, None, -15.0, [0.5, -4.0], 0.0)
        },
        // "Extend a low right heel from a tightly folded knee": a real low
        // heel sweep, not dtilt's poke. Deeper seated crouch than dtilt
        // (LowKick), a tighter Wind-stage fold, and a counter-lean at Wind
        // that reverses into the strike at Contact (a genuine counterweight
        // arc, not dtilt's barely-there `lean_wind`). `contact_offset`
        // solves toward an authored point inside dsmash's hitbox (centre
        // (20,13) r10 in root-local XY) rather than dead-centre -- lower and
        // closer to the body, so the shin actually reads diagonal toward
        // the floor instead of levelling out flat at the circle's centre;
        // `achieved` still gets re-measured against the true centre before
        // any pass/fail call is made (see `contact_offset`'s own doc).
        //
        // The chamber height (`coil[1]`) is a floor constraint, not taste:
        // `support_lift` skips the directed action's own contact foot in
        // every stage, so nothing catches that foot if the chamber puts it
        // under the plane. A first pass at -10.0 (heel tucked *down* beside
        // the seated hip) sank it through the stage across the whole
        // anticipation window and the wind -> contact blend (measured, both
        // facings: sf2 -0.281, sf3 -0.247, sf4 -0.239, sf5 -1.557). -5.0
        // folds the heel *up* under the seated hip instead -- the tighter
        // fold the direction actually asks for -- and clears the plane on
        // every tick of the cycle. Contact/Ext geometry is untouched by
        // this: `coil` only feeds the Wind target and the Fold "home"
        // point, so the active low diagonal and its measured separation are
        // exactly as captured. Guarded per tick, both feet, both facings, by
        // `neither_foot_penetrates_the_floor_on_any_tick_of_the_redirected_smashes`.
        LowSweep => Shape {
            contact_offset: [-4.0, -6.0],
            ..sh(8.0, -4.0, 12.0, 9.0, Some(-18.0), 0.0, [-1.0, -5.0], -6.0)
        },
        // "Drive the bent right forearm ahead of the chest": the runtime
        // hitbox centre sits beyond the arm's maximum reach (need 13.36u,
        // chain reaches 10.01u), so solving straight at it always
        // degenerates to a fully extended arm -- indistinguishable from
        // special_side/fsmash's straight punches, exactly what the review
        // flagged. `aim_pull` pulls the *solve* target back toward the
        // shoulder along the same line so a real elbow bend is possible;
        // the achieved piece is still measured, and required to intersect,
        // against the real (unpulled) hitbox region -- see `aim_pull`'s
        // own doc comment.
        RunningForearm => Shape {
            aim_pull: 0.42,
            ..sh(26.0, 6.0, 12.0, 3.5, None, 25.0, [-2.0, -3.0], 0.0)
        },
        // "Snap the right foot forward while the left knee folds back."
        SplitKick => sh(0.0, -8.0, 8.0, 0.0, Some(5.0), 0.0, [3.0, -5.0], 0.0),
        // "Fold the right knee beneath the chest, then push the sole
        // forward and slightly up"; the chest counterleans.
        RisingKick => sh(-6.0, 10.0, 10.0, 0.0, Some(10.0), 0.0, [2.0, -6.0], 0.0),
        // "Look back, coil the left knee, then extend the heel behind the
        // hips": the sole leads backward, so the piece points -X.
        BackKick => sh(14.0, 2.0, 0.0, 0.0, Some(180.0), 0.0, [-2.0, -6.0], -35.0),
        // "Tuck the right knee close then extend the foot upward."
        OverheadKick => sh(-18.0, 4.0, 6.0, 0.0, Some(60.0), 0.0, [2.0, -5.0], 0.0),
        // "Tuck the right heel beneath the hips, then drive it down."
        HeelDrop => sh(8.0, -6.0, 8.0, 0.0, Some(-35.0), 0.0, [1.5, -5.0], 0.0),
        // "Snap from the existing guard to a bent-elbow palm release at the
        // actual origin", compact torso, small recoil.
        //
        // MEASURED, not assumed. Kestrel's arm chain, read off the shipped
        // skeleton in its rest pose, is upper_arm 5.0 + forearm 4.5 = 9.5,
        // and the designated contact point on `k_hand_r` reaches 10.17u
        // from the shoulder with the elbow straight. The emission origin
        // (14, 15) is 14.29u from the shoulder here, so the piece's
        // *centroid* cannot sit on that point and does not pretend to:
        // it ends 4.89u away.
        //
        // That is not the contact test, though. The contract measures the
        // piece's projected triangles against the region, and the region
        // has the projectile's own radius of 4. Measured that way the real
        // surface of the hand **does** reach into it: signed_separation
        // -0.1355 on the ground, -0.1406 out of shield and -0.1152 in the
        // air, both facings (`tests/special_n_presentation.rs`).
        //
        // Two things make that true without touching bones, scale, the
        // emission point or any table. The palm is solved at a fixed
        // distance along the shoulder -> origin line (`aim_reach`) rather
        // than at a fraction of it, so the elbow keeps one bend wherever
        // the base pose puts the shoulder; and the chest line is stated
        // absolutely (`lean_absolute`) rather than added on top of four
        // base poses that disagree by up to 16 degrees. Before that, the
        // same authored assist produced 21 degrees of chest on the ground
        // and 10 in the air, and the aerial release fell 1.33u short of the
        // region no matter how straight the arm was.
        //
        // Chest 26 and elbow 45 is the corner of the reviewed range that
        // keeps the most elbow bend while the surface still reaches, in
        // every condition. Measured alternatives, all conditions identical:
        //
        //   chest    elbow 45.0   elbow 35.1   elbow 21.2
        //    21        +0.2949      +0.0782      -0.1518
        //    23        +0.1315      -0.0851      -0.3174
        //    25        -0.0328      -0.2494      -0.4841
        //    26        -0.1152      -0.3319      -0.5677
        //
        ProjectileRelease => Shape {
            // "Withdraw the palm and elbow briefly toward the ribs": in
            // front of the chest, not behind the shoulder.
            recoil: Some([2.0, -5.5]),
            // The chest line is stated outright, not added to whatever the
            // stance or the air pose happens to carry.
            lean_absolute: true,
            // 9.4 of the 10.17u the palm can reach: a real elbow bend,
            // identical on the ground and in the air.
            aim_reach: Some(9.4),
            ..sh(26.0, -6.0, 0.0, 0.0, None, -5.0, [-1.5, -4.0], 0.0)
        },
        // "Gather the right arm at the chest and lead the ascent."
        RisingDrive => sh(-14.0, 8.0, 8.0, 0.0, None, -20.0, [0.5, -4.0], 0.0),
        // "Coil the right elbow back then extend the palm."
        LateralDrive => sh(24.0, -8.0, 12.0, 2.0, None, 0.0, [-4.0, -2.0], 0.0),
        // "Open the elbows from a compact guard into a short symmetric
        // pulse around the chest" — no limb defines the radius.
        RadialPulse => sh(6.0, 0.0, 0.0, 3.0, None, 0.0, [0.0, 0.0], 0.0),
        ThrowForward => sh(12.0, -2.0, 0.0, 0.0, None, -10.0, [0.0, 0.0], 0.0),
        ThrowBackward => sh(-4.0, 6.0, 0.0, 0.0, None, -10.0, [0.0, 0.0], -30.0),
        ThrowUpward => sh(-16.0, 8.0, 0.0, 0.0, None, -25.0, [0.0, 0.0], 0.0),
        ThrowDownward => sh(26.0, -4.0, 0.0, 5.0, None, 10.0, [0.0, 0.0], 0.0),
    }
}

/// What the strike aims at, in root-local XY, and whether the runtime gives
/// it a hit region the contact piece is required to intersect.
#[derive(Clone, Copy, Debug)]
pub struct Aim {
    pub contact: [f32; 2],
    /// Absolute coil point (throws hold a real victim); `None` = the
    /// family's coil offset from the limb root.
    pub coil_abs: Option<[f32; 2]>,
    pub radius: f32,
    /// The contact piece must intersect this region for the action to pass.
    /// False for the throws (no fighter hitbox) and for the projectile
    /// action, whose real hit region is the spawned projectile.
    pub required: bool,
    pub kind: &'static str,
}

/// Which real runtime event a pose is being built for.
///
/// Most actions have exactly one. The projectile action has **two**, and
/// they are eight frames apart: the simulation releases the shot on the tick
/// the move starts (`sim::fighter::handle_free_intent`), and the move table
/// also gives it an ordinary fighter hitbox on its own `startup` frame. The
/// emission is not an excuse to ignore that hitbox, so the pose leads the
/// emission line first and then drives the same hand into the hitbox, and
/// both events are measured.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    /// The move's fighter hitbox (or the throw release).
    Hitbox,
    /// The projectile leaving the hand.
    Release,
}

/// Frames of the move on which each of its real events happens.
pub fn events_of(d: &Resolved, md: &MoveData) -> Vec<(Event, u32)> {
    if crate::export::action_spawns_projectile(d.id) {
        // Frame 0 is the release: the shot is requested on the tick the
        // move starts, so the authored anticipation fraction has no
        // inactive sample to peak in (timing_contract: "if no inactive
        // sample exists, apply contact immediately").
        let mut v = vec![(Event::Release, 0)];
        if md.hitbox_at(md.startup).is_some() {
            v.push((Event::Hitbox, md.startup));
        }
        return v;
    }
    vec![(Event::Hitbox, md.startup)]
}

/// The frame the pose must be *at contact* on: the move's hitbox frame, or
/// frame 0 for an action whose only early event is a release.
pub fn contact_frame(d: &Resolved, md: &MoveData) -> u32 {
    events_of(d, md)
        .iter()
        .find(|(e, _)| *e == Event::Hitbox)
        .map(|(_, f)| *f)
        .unwrap_or(0)
}

/// The aim of one action's `Hitbox` event, from the runtime data only.
pub fn aim_for(f: &Fighter, d: &Resolved, md: &MoveData) -> Option<Aim> {
    aim_for_event(f, d, md, Event::Hitbox)
}

/// The aim of one action for a specific real event.
pub fn aim_for_event(f: &Fighter, d: &Resolved, md: &MoveData, event: Event) -> Option<Aim> {
    let mid = f.character.height * 0.5;
    if d.family == PoseFamily::RadialPulse {
        return None;
    }
    if crate::export::action_spawns_projectile(d.id) {
        // The palm leads the emission line and stays there — it never
        // chases the projectile downrange.
        //
        // `md.no_melee` actions (Kestrel's special_n) have no fighter
        // hitbox on any frame, so the release is not merely the *first*
        // real event, it is the only one: the hitbox event resolves to the
        // same spawn origin instead of to a contact the simulation no
        // longer makes. Actions that still carry a fighter hitbox after
        // the release keep aiming at it, and it stays required.
        if event == Event::Release || md.no_melee {
            return Some(Aim {
                contact: [PROJECTILE_SPAWN_LOCAL_X, mid],
                coil_abs: None,
                radius: attacks::PROJECTILE_RADIUS,
                required: false,
                kind: "projectile_spawn",
            });
        }
        // …and the move's own fighter hitbox is a real hit region: the
        // designated hand has to be in it on its clean frames.
        return Some(Aim {
            contact: [md.hitbox.offset.x, md.hitbox.offset.y + mid],
            coil_abs: None,
            radius: md.hitbox.radius,
            required: true,
            kind: "hitbox",
        });
    }
    if !crate::export::action_has_hitbox(d.id) {
        // Throws: the hand opens toward the runtime release parameters,
        // from the position the simulation really holds the victim at.
        return Some(Aim {
            contact: [md.hitbox.offset.x, md.hitbox.offset.y + mid],
            coil_abs: Some([HOLD_LOCAL_X, mid]),
            radius: 0.0,
            required: false,
            kind: "throw_release",
        });
    }
    Some(Aim {
        contact: [md.hitbox.offset.x, md.hitbox.offset.y + mid],
        coil_abs: None,
        radius: md.hitbox.radius,
        required: true,
        kind: "hitbox",
    })
}

// ----------------------------------------------------------------- support

/// States whose grounded pose rests on the stage plane (rolls, techs,
/// getups and knockdowns deliberately leave it).
fn rests_on_the_floor(f: &Fighter) -> bool {
    f.grounded
        && matches!(
            f.state,
            State::Stand
                | State::Walk
                | State::Dash
                | State::Run
                | State::RunTurn
                | State::Crouch
                | State::JumpSquat
                | State::LandLag { .. }
                | State::Waveland
                | State::Shield
                | State::ShieldStun { .. }
                | State::ShieldDrop
                | State::Attack { .. }
                | State::Grab
                | State::Hold
                | State::Grabbed
                | State::Throw { .. }
                | State::Rebound { .. }
        )
}

/// Lift a grounded pose until the supporting foot rests on the plane.
///
/// Measured, never guessed: the shift is exactly the depth of the lowest
/// vertex of the support foot piece, and the pose is only ever raised. The
/// simulation root, the hurt capsule and every bone length are untouched —
/// this moves the drawn body, which is what sank through the floor.
fn support_lift(rig: &Rig, model: &CharacterModel, pose: &mut Pose, skip: Option<usize>) -> f32 {
    let low = super::contact::foot_support(model, pose, &Xf::IDENTITY, 0.0)
        .iter()
        .filter(|s| rig.bone(&s.bone) != skip)
        .map(|s| s.support_distance)
        .fold(None, |a: Option<f32>, x| Some(a.map_or(x, |v| v.min(x))));
    let lift = low.map(|y| (-y).max(0.0)).unwrap_or(0.0);
    if lift > 0.0 {
        if let Some(i) = rig.bone("root") {
            pose.off[i] = pose.off[i] + v3(0.0, lift, 0.0);
        }
    }
    lift
}

// ----------------------------------------------------------------- framing

fn side_of(rig: &Rig, bone: usize) -> &'static str {
    let n = &rig.bones[bone].name;
    if n.ends_with("_l") {
        "l"
    } else if n.ends_with("_r") {
        "r"
    } else {
        ""
    }
}

/// The limbs the strike does *not* use: the counterbalancing hand, the
/// support leg, the trailing arm — one line of the direction each.
#[allow(clippy::too_many_arguments)]
fn frame_body(
    p: &mut Pose,
    rig: &Rig,
    fam: PoseFamily,
    side: &str,
    airborne: bool,
    stage: Stage,
    crouch: f32,
    crouch_depth: f32,
) {
    use PoseFamily::*;
    let other = if side == "l" { "r" } else { "l" };
    let k = match stage {
        Stage::Wind => 0.45,
        Stage::Fold => 0.6,
        _ => 1.0,
    };
    // Legs of an arm strike: braced on the ground, trailing in the air.
    let stand_legs = |p: &mut Pose| {
        if airborne {
            anim::leg(p, rig, "r", 26.0, -40.0, Some(-12.0));
            anim::leg(p, rig, "l", -16.0, -28.0, Some(-10.0));
        } else {
            anim::leg(p, rig, "r", 16.0, -24.0, None);
            anim::leg(p, rig, "l", -12.0, -18.0, None);
        }
    };
    // Support leg of a kick (the leg that is not solved).
    let support = |p: &mut Pose| {
        if airborne {
            anim::leg(p, rig, other, -20.0, -34.0, Some(-10.0));
        } else {
            anim::leg(p, rig, other, -8.0, -26.0, None);
        }
    };
    // A standing leg bent toward the engine's own full-crouch reference
    // (`anim::crouch`'s thigh/shin bend, at this character's
    // `AnimStyle::crouch_depth` root drop), by however much of that
    // depth this stage's `crouch` asks for. A family's `crouch` root
    // offset alone is not a real lower stance: the support leg's own FK
    // angles do not shorten with it, so the foot goes exactly `crouch`
    // units below the floor and `support_lift` raises the root straight
    // back up by the same amount, cancelling most of the intended drop
    // (measured directly: `diag_crouch_cancel`). Bending the knee here
    // shortens the leg's own reach, so the floor-rest correction stays
    // small and the crouch actually shows.
    let crouch_leg = |p: &mut Pose, side: &str, thigh: f32, shin: f32| {
        let cf = if crouch_depth > 1e-3 {
            (crouch / crouch_depth).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let (deep_thigh, deep_shin) = if side == "r" {
            (62.0, -104.0)
        } else {
            (48.0, -100.0)
        };
        anim::leg(
            p,
            rig,
            side,
            thigh + (deep_thigh - thigh) * cf,
            shin + (deep_shin - shin) * cf,
            None,
        );
    };
    match fam {
        Punch => {
            // "Left fist stays beside the upper chest, below the face."
            anim::arm(p, rig, other, 24.0 - 8.0 * k, 104.0, 12.0);
            stand_legs(p);
        }
        Uppercut => {
            // "Left palm guards the chest rather than following."
            anim::arm(p, rig, other, 34.0, 96.0, 14.0);
            stand_legs(p);
        }
        PalmDrive => {
            // "Left hand pulls to the ribs and stays separated."
            anim::arm(p, rig, other, 30.0 + 14.0 * k, 112.0, 20.0);
            stand_legs(p);
        }
        OverheadDrive => {
            // "Left hand remains lower, framing the abdomen." "Rise from a
            // compact knee bend into an upward torso line" -- a real
            // compression on both legs (see `crouch_leg`), held through
            // the active window like the direction asks, not just a root
            // offset `support_lift` would otherwise erase.
            anim::arm(p, rig, other, 46.0, 86.0, 16.0);
            if airborne {
                stand_legs(p);
            } else {
                crouch_leg(p, "r", 16.0, -24.0);
                crouch_leg(p, "l", -12.0, -18.0);
            }
        }
        RunningForearm => {
            // "Left arm streams behind the torso, elbow separated."
            anim::arm(p, rig, other, -52.0 * k, 44.0, 24.0);
            if airborne {
                stand_legs(p);
            } else {
                anim::leg(p, rig, "r", 42.0 * k, -34.0, None);
                anim::leg(p, rig, "l", -36.0 * k, -18.0, None);
            }
        }
        ProjectileRelease => {
            // "Left hand braces the upper chest, below the emission line."
            anim::arm(p, rig, other, 44.0, 98.0, 16.0);
            stand_legs(p);
        }
        RisingDrive => {
            // "Left hand stays near the ribs, separated from the scarf";
            // "knees trail at different heights".
            anim::arm(p, rig, other, 28.0, 108.0, 18.0);
            anim::leg(p, rig, "r", 34.0 * k, -52.0, Some(-14.0));
            anim::leg(p, rig, "l", 12.0 * k, -30.0, Some(-10.0));
        }
        LateralDrive => {
            // "Left hand trails behind the hip."
            anim::arm(p, rig, other, -44.0 * k, 52.0, 22.0);
            if airborne {
                stand_legs(p);
            } else {
                anim::leg(p, rig, "r", 34.0 * k, -30.0, None);
                anim::leg(p, rig, "l", -30.0 * k, -16.0, None);
            }
        }
        RadialPulse => {
            // "Both hands frame the pulse at different depths; neither
            // pretends to define its whole radius."
            anim::arm(
                p,
                rig,
                "r",
                34.0 + 26.0 * k,
                74.0 - 30.0 * k,
                30.0 + 26.0 * k,
            );
            anim::arm(
                p,
                rig,
                "l",
                26.0 + 30.0 * k,
                84.0 - 34.0 * k,
                24.0 + 30.0 * k,
            );
            stand_legs(p);
        }
        Kick | LowKick => {
            // "Both fists frame the chest, with the rear elbow visibly
            // separated" / "near hand guards the chin, far hand back".
            anim::arm(p, rig, side, -26.0 * k, 62.0, 18.0);
            anim::arm(p, rig, other, 38.0 * k, 72.0, 22.0);
            // dtilt ("fold the support knee") needs a real bent support
            // knee under a real lower stance, not the plain kick's fixed
            // brace -- see `crouch_leg`. Plain `ftilt` (Kick) keeps its
            // original brace.
            if matches!(fam, LowKick) && !airborne {
                crouch_leg(p, other, -8.0, -26.0);
            } else {
                support(p);
            }
        }
        LowSweep => {
            // Distinct from dtilt's poke, per direction: "front hand guards
            // high", "the rear hand counterbalances behind" -- the near
            // hand tucks up tight (a high, folded guard) instead of framing
            // the chest, and the far hand opens back and low as a real
            // counterweight (compare `HeelDrop`'s "other hand opens
            // backward"), not a second guarding fist.
            anim::arm(p, rig, side, -14.0 * k, 104.0, 16.0);
            anim::arm(p, rig, other, -40.0 * k, 34.0, 30.0);
            // "Sit the hips over the support leg": a deeper fold than
            // dtilt's, not the shared brace.
            if !airborne {
                crouch_leg(p, other, -14.0, -34.0);
            } else {
                support(p);
            }
        }
        SplitKick => {
            // "Hands spread at different heights and stay behind the sole";
            // "the left knee folds back".
            anim::arm(p, rig, side, -34.0 * k, 54.0, 26.0);
            anim::arm(p, rig, other, 44.0 * k, 78.0, 34.0);
            anim::leg(p, rig, other, -46.0 * k, -76.0, Some(-16.0));
        }
        RisingKick => {
            // "Left hand guards near the face, right hand low and behind."
            anim::arm(p, rig, side, -30.0 * k, 58.0, 20.0);
            anim::arm(p, rig, other, 62.0 * k, 84.0, 24.0);
            anim::leg(p, rig, other, -24.0 * k, -60.0, Some(-14.0));
        }
        BackKick => {
            // "Near hand stays in front of the chest; far hand opens down."
            anim::arm(p, rig, other, 40.0 * k, 88.0, 16.0);
            anim::arm(p, rig, side, -18.0 * k, 26.0, 30.0);
            anim::leg(p, rig, other, 30.0 * k, -54.0, Some(-14.0));
        }
        OverheadKick => {
            // "Hands remain below the chest and at different depths";
            // "keep the other knee bent".
            anim::arm(p, rig, side, 22.0 * k, 96.0, 20.0);
            anim::arm(p, rig, other, 34.0 * k, 104.0, 32.0);
            anim::leg(p, rig, other, 6.0 * k, -84.0, Some(-18.0));
        }
        HeelDrop => {
            // "One hand guards the chest and the other opens backward."
            anim::arm(p, rig, side, 30.0 * k, 92.0, 18.0);
            anim::arm(p, rig, other, -46.0 * k, 40.0, 34.0);
            anim::leg(p, rig, other, -34.0 * k, -66.0, Some(-16.0));
        }
        ThrowForward | ThrowUpward | ThrowDownward => {
            // The guiding hand holds, then withdraws / opens away.
            anim::arm(p, rig, other, 88.0 - 26.0 * k, 22.0 + 30.0 * k, 14.0);
            stand_legs(p);
        }
        ThrowBackward => {
            // "Right hand guides the crossing target, then opens away."
            anim::arm(p, rig, other, 78.0 - 40.0 * k, 26.0 + 34.0 * k, 26.0);
            stand_legs(p);
        }
    }
}

// ----------------------------------------------------------------- poses

fn base_pose(model: &CharacterModel, f: &Fighter, md: &MoveData, frame: u64) -> Pose {
    let rig = &model.rig;
    if matches!(f.state, State::Throw { .. }) {
        anim::grab_reach(rig)
    } else if md.is_aerial || !f.grounded {
        anim::air(rig, f.vel.y > 0.5, f.fastfalling)
    } else {
        anim::stance(rig, &model.style, frame as f32)
    }
}

/// How much lean a base pose already carries, read back from the two bones
/// `anim::lean` writes into (`spine` at -0.55 deg per degree, `chest` at
/// -0.45). Averaged, so either bone alone cannot skew it.
fn base_lean_deg(p: &Pose, rig: &Rig) -> f32 {
    match (rig.bone("spine"), rig.bone("chest")) {
        (Some(si), Some(ci)) => (-p.rot[si].z / 0.55 + -p.rot[ci].z / 0.45) * 0.5,
        _ => 0.0,
    }
}

/// Build one key pose of a directed action.
#[allow(clippy::too_many_arguments)]
fn staged(
    model: &CharacterModel,
    d: &Resolved,
    f: &Fighter,
    md: &MoveData,
    frame: u64,
    stage: Stage,
    lean_extra: f32,
    event: Event,
) -> (Pose, Option<Reach>) {
    let rig = &model.rig;
    let s = shape(d.family);
    let mut p = base_pose(model, f, md, frame);
    let side = side_of(rig, d.bone);
    let airborne = md.is_aerial || !f.grounded;

    // Torso and hips: the authored line, plus any measured assist.
    let lean = match stage {
        Stage::Wind => s.lean_wind,
        Stage::Contact => s.lean + lean_extra,
        Stage::Ext => s.lean + lean_extra + s.follow_lean,
        Stage::Fold => (s.lean + lean_extra) * 0.4,
        // The opposing shoulder motion of a recoil: the chest gives back
        // the forward line it just spent, without becoming an anticipation.
        Stage::Recoil => s.lean_wind,
    };
    // `lean` is an assist on top of the base pose unless the family asks
    // for an absolute chest line (see `lean_absolute`).
    let applied = if s.lean_absolute {
        lean - base_lean_deg(&p, rig)
    } else {
        lean
    };
    anim::lean(&mut p, rig, applied);
    let crouch = match stage {
        Stage::Wind => s.crouch * 0.5,
        Stage::Fold => s.crouch * 0.4,
        Stage::Recoil => s.crouch * 0.4,
        _ => s.crouch_active.unwrap_or(s.crouch),
    };
    if crouch != 0.0 {
        if let Some(i) = rig.bone("root") {
            p.off[i] = p.off[i] + v3(0.0, -crouch, 0.0);
        }
    }
    let yaw = match stage {
        Stage::Wind => s.head_yaw * 0.6,
        Stage::Fold => s.head_yaw * 0.3,
        // The head keeps looking along the discharge through the recoil.
        Stage::Recoil => s.head_yaw,
        _ => s.head_yaw,
    };
    p.rot(rig, "head", 0.0, yaw, -lean * 0.25);

    frame_body(
        &mut p,
        rig,
        d.family,
        side,
        airborne,
        stage,
        crouch,
        model.style.crouch_depth,
    );

    // Rest the drawn body on the stage before solving, so the strike is
    // measured from a pose whose support foot is really on the floor.
    if rests_on_the_floor(f) {
        let skip = if side.is_empty() {
            None
        } else {
            rig.bone(&format!("foot_{side}"))
        };
        support_lift(rig, model, &mut p, skip);
    }

    // The contact solve.
    let Some(aim) = aim_for_event(f, d, md, event) else {
        return (p, None);
    };
    let Some(ch) = chain(rig, d.bone) else {
        return (p, None);
    };
    let bend = if rig.bones[d.bone].name.starts_with("foot") {
        Bend::Knee
    } else {
        Bend::Elbow
    };
    let world = rig.world(&p, &Xf::IDENTITY);
    let root = world[ch.upper].t;
    let target = match stage {
        Stage::Wind => aim
            .coil_abs
            .unwrap_or([root.x + s.coil[0], root.y + s.coil[1]]),
        // Palm and elbow withdrawn to the ribs: the limb's own coil point,
        // measured from the shoulder, so the forearm folds instead of
        // sweeping through a second line.
        Stage::Recoil => {
            let c = s.recoil.unwrap_or(s.coil);
            [root.x + c[0], root.y + c[1]]
        }
        Stage::Fold => {
            // Fold the joint first: pull the piece halfway home along the
            // limb, which bends knee/elbow instead of sweeping again.
            let home = [root.x + s.coil[0] * 0.6, root.y + s.coil[1] * 0.9];
            [
                (aim.contact[0] + home[0] * 2.0) / 3.0,
                (aim.contact[1] + home[1] * 2.0) / 3.0,
            ]
        }
        _ => {
            let base = if let Some(reach) = s.aim_reach {
                let (dx, dy) = (aim.contact[0] - root.x, aim.contact[1] - root.y);
                let d = (dx * dx + dy * dy).sqrt().max(1e-3);
                let k = (reach / d).min(1.0);
                [root.x + dx * k, root.y + dy * k]
            } else if s.aim_pull > 0.0 {
                [
                    root.x + (aim.contact[0] - root.x) * (1.0 - s.aim_pull),
                    root.y + (aim.contact[1] - root.y) * (1.0 - s.aim_pull),
                ]
            } else {
                aim.contact
            };
            [base[0] + s.contact_offset[0], base[1] + s.contact_offset[1]]
        }
    };
    let tip_abs = match (s.tip_abs, stage) {
        (Some(t), Stage::Ext) => Some(t + s.follow_tip),
        (t, _) => t,
    };
    let tip_local = match stage {
        Stage::Ext => s.tip_local + s.follow_tip,
        Stage::Wind => s.tip_local * 0.5,
        Stage::Recoil => s.tip_local * 0.5,
        _ => s.tip_local,
    };
    let mut reach = solve(
        rig, &mut p, &ch, d.effector, target, bend, tip_abs, tip_local,
    );
    // `aim_pull`/`contact_offset` (Contact/Ext only) solve toward a point
    // short of, or offset from, the real aim: re-measure `achieved` against
    // the *real* aim.contact so every consumer (feasibility's centroid_gap,
    // the lean_assist search's "good enough" check) judges intersection
    // against the true hit region, never the decoy point actually fed to
    // the solver.
    if (s.aim_pull > 0.0 || s.aim_reach.is_some() || s.contact_offset != [0.0, 0.0])
        && matches!(stage, Stage::Contact | Stage::Ext)
    {
        let world = rig.world(&p, &Xf::IDENTITY);
        let achieved_pt = world[ch.tip].point(d.effector);
        reach.achieved = ((achieved_pt.x - aim.contact[0]).powi(2)
            + (achieved_pt.y - aim.contact[1]).powi(2))
        .sqrt();
    }
    (p, Some(reach))
}

/// Build the contact pose, adding the family's measured lean assist only
/// while the contact piece stays outside the runtime hit region.
fn contact_pose(
    model: &CharacterModel,
    d: &Resolved,
    f: &Fighter,
    md: &MoveData,
    frame: u64,
    stage: Stage,
    event: Event,
) -> (Pose, Option<Reach>, f32) {
    let s = shape(d.family);
    let aim = aim_for_event(f, d, md, event);
    let want = aim.map(|a| a.radius * 0.85).unwrap_or(0.0);
    let mut best: Option<(Pose, Option<Reach>, f32)> = None;
    let steps = if s.lean_assist > 0.0 { 9 } else { 1 };
    for i in 0..steps {
        let extra = s.lean_assist * i as f32 / (steps - 1).max(1) as f32;
        let (p, r) = staged(model, d, f, md, frame, stage, extra, event);
        let good = r.map(|x| x.achieved <= want).unwrap_or(true);
        let better = match &best {
            None => true,
            Some((_, br, _)) => {
                r.map(|x| x.achieved).unwrap_or(0.0) < br.map(|x| x.achieved).unwrap_or(0.0)
            }
        };
        if good {
            return (p, r, extra);
        }
        if better {
            best = Some((p, r, extra));
        }
    }
    best.expect("at least one candidate")
}

/// The whole directed timeline of one action.
fn directed(model: &CharacterModel, d: &Resolved, f: &Fighter, frame: u64) -> Pose {
    let md = attacks::data(f.character.id, d.id);
    let sf = f.state_frame;
    let base = base_pose(model, f, &md, frame);
    let (contact, _, extra) = contact_pose(model, d, f, &md, frame, Stage::Contact, Event::Hitbox);
    let charging = f.charge > 0 && f.charge_armed && attacks::is_smash(d.id);
    let (wind, _) = staged(model, d, f, &md, frame, Stage::Wind, extra, Event::Hitbox);
    // A held smash freezes on the anticipation pose (the simulation freezes
    // `state_frame` there, so nothing else in the timeline moves either).
    if charging {
        return wind;
    }
    // A release-only action -- the shot leaves on frame 0 and the move
    // carries no fighter hitbox on any frame -- has one beat, not a strike
    // arc: emit, withdraw, settle once. It never reaches `Stage::Ext`, so
    // there is no second pose peak at the old state-frame-9 marker and no
    // long forward hold. An action that still has a real fighter hitbox
    // (Boulder's and Viper's special_n) keeps the contact -> follow-through
    // -> fold timeline below, because its follow-through is part of the hit.
    if crate::export::action_spawns_projectile(d.id) && md.no_melee {
        let (release, _, rel_extra) =
            contact_pose(model, d, f, &md, frame, Stage::Contact, Event::Release);
        let (recoil, _) = staged(
            model,
            d,
            f,
            &md,
            frame,
            Stage::Recoil,
            rel_extra,
            Event::Release,
        );
        let total = md.total().max(1) as f32;
        let peak = (total * RECOIL_PEAK).max(1.0);
        let settled = (total * SETTLED_BY).max(peak + 1.0);
        // The shot is requested on the tick the move starts, and the
        // fighter's own tick has already advanced `state_frame` to 1 by the
        // time anything is drawn. So state_frame 1 *is* the release frame,
        // and the withdrawal starts after it -- not one tick early, which
        // would draw the recoil on the very frame the shot leaves.
        let t = sf.saturating_sub(1) as f32;
        if t <= peak {
            return release.blend(&recoil, ease(t / peak));
        }
        if t <= settled {
            return recoil.blend(&base, ease((t - peak) / (settled - peak)));
        }
        return base;
    }
    let last_active = md.startup + md.active + md.late_active;
    if sf < md.startup {
        // An action with a second, earlier runtime event — the projectile
        // release on frame 0 — leads that event first and then carries the
        // same hand into the move's own hitbox. Neither event is dropped
        // and neither is delayed.
        if crate::export::action_spawns_projectile(d.id) {
            let (release, _, _) =
                contact_pose(model, d, f, &md, frame, Stage::Contact, Event::Release);
            if md.startup == 0 {
                return release;
            }
            let u = sf as f32 / md.startup as f32;
            return release.blend(&contact, ease(u));
        }
        // Anticipation inside the *available* inactive startup, never a
        // frame more: peak at the authored fraction, then drive to contact.
        let u = sf as f32 / md.startup.max(1) as f32;
        let frac = d.fraction.clamp(0.0, 1.0);
        if frac <= 0.0 {
            return base.blend(&contact, ease_in(u));
        }
        if u <= frac {
            return base.blend(&wind, ease(u / frac));
        }
        return wind.blend(&contact, ease_in((u - frac) / (1.0 - frac)));
    }
    if sf < last_active {
        // The whole real active interval — clean and late alike — keeps the
        // contact pose; the follow-through only deepens it.
        let (ext, _, _) = contact_pose(model, d, f, &md, frame, Stage::Ext, Event::Hitbox);
        let k = (sf - md.startup) as f32 / md.active.max(1) as f32;
        return contact.blend(&ext, ease_out(k.min(1.0)));
    }
    // Recovery: fold the joint first, then settle. Never a second strike.
    let (ext, _, _) = contact_pose(model, d, f, &md, frame, Stage::Ext, Event::Hitbox);
    let (fold, _) = staged(model, d, f, &md, frame, Stage::Fold, extra, Event::Hitbox);
    let v = (sf - last_active) as f32 / md.endlag.max(1) as f32;
    if v < 0.45 {
        ext.blend(&fold, ease(v / 0.45))
    } else {
        fold.blend(&base, ease(((v - 0.45) / 0.55).min(1.0)))
    }
}

/// The pose of a fighter on `frame`: the authored direction when the model
/// ships one for this action, the generic evaluator otherwise.
pub fn fighter_pose(model: &CharacterModel, f: &Fighter, frame: u64) -> Pose {
    let rig = &model.rig;
    let id = match f.state {
        State::Attack { id, .. } | State::Throw { id } => Some(id),
        _ => None,
    };
    let directed_pose = match (id, model.directions.as_ref()) {
        (Some(id), Some(dirs)) => dirs.get(id).map(|d| directed(model, d, f, frame)),
        _ => None,
    };
    let mut p = match directed_pose {
        Some(p) => {
            // The charge tremble is the sim's own tell, kept as it was.
            let mut p = p;
            if f.charge > 0 && f.charge_armed {
                let c = f.charge as f32;
                let amp = 0.6 + c / 60.0 * 1.4;
                let j = (c * 2.7).sin() * amp;
                if let Some(i) = rig.bone("root") {
                    p.off[i] = p.off[i]
                        + v3(
                            j * 0.5,
                            -model.style.crouch_depth * 0.15 * (c / 60.0) - j.abs() * 0.3,
                            0.0,
                        );
                }
            }
            p
        }
        None => anim::fighter_pose(rig, f, &model.style, frame),
    };
    // Every drawn grounded pose rests on the stage plane, measured from the
    // foot meshes themselves (idle, crouch, shield stun, landing lag…).
    if rests_on_the_floor(f) && model.directions.is_some() {
        let skip = match (id, model.directions.as_ref()) {
            (Some(id), Some(dirs)) => dirs
                .get(id)
                .filter(|d| rig.bones[d.bone].name.starts_with("foot"))
                .map(|d| d.bone),
            _ => None,
        };
        support_lift(rig, model, &mut p, skip);
    }
    p
}

// ----------------------------------------------------------------- evidence

/// What the implementation achieved for one action, in world units — the
/// report the direction contract asks for instead of a guessed correction.
///
/// **This is a synthetic probe, not the measured render output.** It is
/// built from [`contact_pose`] on a bare, one-shot [`Fighter`] at exactly
/// the event's frame: no simulation runs, no render history is replayed,
/// and none of [`fighter_pose`]'s extra effects apply — no eased-facing
/// interpolation, no hitlag rattle, no animation-history extras lag
/// (scarf/crest), no [`support_lift`]. It exists to catch geometry the IK
/// solve cannot reach *at all*, cheaply, for every action at once.
/// `guaranteed_intersection` here is not a claim that the real, rendered
/// per-frame pose intersects — that claim can only come from the actual
/// measurement pipeline ([`super::render_eval::evaluate`] +
/// [`super::contact::measure_posed`], the one `contact_json` and the
/// capture sheets use on real exported ticks, both facings, clean and
/// late frames alike). Treat this struct as a cheap pre-check, never as a
/// substitute for that real measurement.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Feasibility {
    pub action_id: &'static str,
    pub pose_family: &'static str,
    pub contact_piece_id: String,
    pub anticipation_fraction: f32,
    pub aim_kind: &'static str,
    pub target_local: [f32; 2],
    pub hit_radius: f32,
    pub intersection_required: bool,
    pub required_reach: f32,
    pub max_reach: f32,
    /// Distance from the achieved piece centroid to the aim point.
    pub centroid_gap: f32,
    /// A convex piece whose centroid is within the radius of the centre
    /// always has surface distance ≤ that gap, so the strike intersects.
    pub guaranteed_intersection: bool,
    pub lean_assist_deg: f32,
    pub note: Option<String>,
    /// Every real runtime event of this action, measured on its own frame.
    /// Most actions have one; the projectile action has two (the release on
    /// the move's first frame and its own fighter hitbox on `startup`), and
    /// the emission never excuses the hitbox.
    pub events: Vec<EventFeasibility>,
}

/// One real runtime event of an action, measured on the frame it happens.
#[derive(Debug, Clone, serde::Serialize)]
pub struct EventFeasibility {
    pub event: &'static str,
    pub state_frame: u32,
    pub aim_kind: &'static str,
    pub target_local: [f32; 2],
    pub hit_radius: f32,
    pub intersection_required: bool,
    pub required_reach: f32,
    pub max_reach: f32,
    pub centroid_gap: f32,
    pub guaranteed_intersection: bool,
    pub lean_assist_deg: f32,
    pub note: Option<String>,
}

/// Feasibility of every directed action of `model`, on a synthetic fighter
/// in the action's own state (no simulation is run). See the caveat on
/// [`Feasibility`] itself: this is a cheap reachability pre-check, not the
/// measured render output, and must never stand in for the real per-frame
/// contact measurement (`contact_json` / the capture sheets).
pub fn feasibility(model: &CharacterModel) -> Vec<Feasibility> {
    let Some(dirs) = model.directions.as_ref() else {
        return Vec::new();
    };
    dirs.actions
        .iter()
        .map(|d| feasibility_of(model, dirs, d))
        .collect()
}

/// Synthetic per-action probe behind [`feasibility`] — see the caveat on
/// [`Feasibility`]: built from a bare one-shot [`Fighter`], not the real
/// render/measurement path.
fn feasibility_of(model: &CharacterModel, _dirs: &Directions, d: &Resolved) -> Feasibility {
    let md = attacks::data(character_of(model), d.id);
    let mut f = Fighter::new(character_of(model).data(), 0, crate::sim::Vec2::ZERO);
    f.facing = 1.0;
    f.grounded = !md.is_aerial;
    if crate::export::action_has_hitbox(d.id) {
        f.set_state_pub(State::Attack {
            id: d.id,
            aerial: md.is_aerial,
        });
    } else {
        f.set_state_pub(State::Throw { id: d.id });
    }
    // Every real runtime event of this action, each measured on the frame
    // the simulation puts it on and on the pose drawn there.
    let mut events = Vec::new();
    for (event, at) in events_of(d, &md) {
        f.state_frame = at;
        let aim = aim_for_event(&f, d, &md, event);
        let (_, reach, extra) = contact_pose(model, d, &f, &md, 0, Stage::Contact, event);
        let r = reach.unwrap_or_default();
        let radius = aim.map(|a| a.radius).unwrap_or(0.0);
        let required = aim.map(|a| a.required).unwrap_or(false);
        let guaranteed = radius > 0.0 && r.achieved <= radius;
        let note = if aim.is_none() {
            Some(
                "radial effect: the body is centred in the runtime effect; no limb defines its radius"
                    .to_string(),
            )
        } else if event == Event::Release {
            Some(format!(
                "the simulation releases the projectile on the move's first frame, so the authored anticipation fraction ({:.2}) has no inactive sample to peak in and the palm leads the emission line from frame 0; the piece centroid ends {:.3}u from the spawn point (projectile radius {:.3}u). This event never replaces the move's own hitbox, measured separately below",
                d.fraction, r.achieved, radius
            ))
        } else if !required {
            Some(format!(
                "no fighter hitbox to intersect ({}); measured against the real runtime event",
                aim.map(|a| a.kind).unwrap_or("")
            ))
        } else if !r.reached && guaranteed {
            Some(format!(
                "limb cannot reach the hitbox centre (needs {:.3}u, chain reaches {:.3}u): it extends fully along the line and the piece still intersects, centroid {:.3}u from the centre (radius {:.3}u)",
                r.required, r.max_reach, r.achieved, radius
            ))
        } else if !guaranteed {
            Some(format!(
                "UNREACHABLE: centroid ends {:.3}u from the hitbox centre, radius {:.3}u — an art decision is needed (no simulation, hitbox, capsule, scale or bone length may be changed to close it)",
                r.achieved, radius
            ))
        } else {
            None
        };
        events.push(EventFeasibility {
            event: match event {
                Event::Hitbox => "hitbox",
                Event::Release => "projectile_release",
            },
            state_frame: at,
            aim_kind: aim.map(|a| a.kind).unwrap_or("radial_effect"),
            target_local: aim.map(|a| a.contact).unwrap_or([0.0, 0.0]),
            hit_radius: radius,
            intersection_required: required,
            required_reach: r.required,
            max_reach: r.max_reach,
            centroid_gap: r.achieved,
            guaranteed_intersection: guaranteed,
            lean_assist_deg: extra,
            note,
        });
    }
    // The flat fields describe the event that has to intersect (the
    // fighter hitbox), so a positive gap can never be hidden behind an
    // emission exception.
    let main = events
        .iter()
        .find(|e| e.intersection_required)
        .or_else(|| events.first())
        .cloned()
        .unwrap_or(EventFeasibility {
            event: "none",
            state_frame: 0,
            aim_kind: "radial_effect",
            target_local: [0.0, 0.0],
            hit_radius: 0.0,
            intersection_required: false,
            required_reach: 0.0,
            max_reach: 0.0,
            centroid_gap: 0.0,
            guaranteed_intersection: false,
            lean_assist_deg: 0.0,
            note: None,
        });
    Feasibility {
        action_id: d.action_id,
        pose_family: d.family.name(),
        contact_piece_id: d.piece_id.clone(),
        anticipation_fraction: d.fraction,
        aim_kind: main.aim_kind,
        target_local: main.target_local,
        hit_radius: main.hit_radius,
        intersection_required: main.intersection_required,
        required_reach: main.required_reach,
        max_reach: main.max_reach,
        centroid_gap: main.centroid_gap,
        guaranteed_intersection: main.guaranteed_intersection,
        lean_assist_deg: main.lean_assist_deg,
        note: main.note.clone(),
        events,
    }
}

fn character_of(model: &CharacterModel) -> crate::sim::roster::CharacterId {
    match model.spec_id {
        Some("boulder") => crate::sim::roster::CharacterId::Boulder,
        Some("viper") => crate::sim::roster::CharacterId::Viper,
        _ => crate::sim::roster::CharacterId::Kestrel,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::characters::build;
    use crate::sim::attacks::MoveId;
    use crate::sim::roster::CharacterId;
    use crate::sim::Vec2;

    fn fighter(id: MoveId, md: &MoveData, sf: u32) -> Fighter {
        let mut f = Fighter::new(CharacterId::Kestrel.data(), 0, Vec2::ZERO);
        f.facing = 1.0;
        f.grounded = !md.is_aerial;
        if crate::export::action_has_hitbox(id) {
            f.set_state_pub(State::Attack {
                id,
                aerial: md.is_aerial,
            });
        } else {
            f.set_state_pub(State::Throw { id });
        }
        f.state_frame = sf;
        f
    }

    /// The release geometry of an arm action, measured on a built pose:
    /// where the shoulder is, how far the palm can actually get from it,
    /// how bent the elbow is, how far the chest leans, and how far the palm
    /// sits off the shoulder->origin line.
    struct Geom {
        shoulder: [f32; 2],
        shoulder_to_origin: f32,
        palm_reach: f32,
        elbow: f32,
        lean: f32,
        off_line: f32,
    }

    fn geom(model: &CharacterModel, d: &Resolved, pose: &Pose) -> Geom {
        let rig = &model.rig;
        let w = rig.world(pose, &Xf::IDENTITY);
        let ch = chain(rig, d.bone).expect("the arm is a two-link chain");
        let sh = w[ch.upper].t;
        let el = w[ch.fore].t;
        let palm = w[ch.tip].point(d.effector);
        let origin = [PROJECTILE_SPAWN_LOCAL_X, 15.0];
        let v1 = [el.x - sh.x, el.y - sh.y];
        let v2 = [palm.x - el.x, palm.y - el.y];
        let n1 = (v1[0] * v1[0] + v1[1] * v1[1]).sqrt().max(1e-6);
        let n2 = (v2[0] * v2[0] + v2[1] * v2[1]).sqrt().max(1e-6);
        let elbow = ((v1[0] * v2[0] + v1[1] * v2[1]) / (n1 * n2))
            .clamp(-1.0, 1.0)
            .acos()
            .to_degrees();
        let chest = w[rig.bone("chest").expect("chest")].t;
        let head = w[rig.bone("head").expect("head")].t;
        let lean = (head.x - chest.x)
            .atan2((head.y - chest.y).max(1e-6))
            .to_degrees();
        let dx = [origin[0] - sh.x, origin[1] - sh.y];
        let dlen = (dx[0] * dx[0] + dx[1] * dx[1]).sqrt().max(1e-6);
        let pv = [palm.x - sh.x, palm.y - sh.y];
        let off_line = (pv[0] * dx[1] - pv[1] * dx[0]).abs() / dlen;
        Geom {
            shoulder: [sh.x, sh.y],
            shoulder_to_origin: dlen,
            palm_reach: (pv[0] * pv[0] + pv[1] * pv[1]).sqrt(),
            elbow,
            lean,
            off_line,
        }
    }

    /// World position of the contact piece's centroid for a fighter.
    fn centroid(model: &CharacterModel, d: &Resolved, pose: &Pose) -> [f32; 2] {
        let w = model.rig.world(pose, &Xf::IDENTITY);
        let p = w[d.bone].point(d.effector);
        [p.x, p.y]
    }

    #[test]
    fn two_link_solve_lands_on_a_reachable_target() {
        let m = build(CharacterId::Kestrel);
        let d = m
            .directions
            .as_ref()
            .unwrap()
            .get(MoveId::SpecialUp)
            .unwrap();
        let md = attacks::data(CharacterId::Kestrel, MoveId::SpecialUp);
        let f = fighter(MoveId::SpecialUp, &md, md.startup);
        let (p, r, _) = contact_pose(&m, d, &f, &md, 0, Stage::Contact, Event::Hitbox);
        let r = r.unwrap();
        assert!(r.reached, "up-special is inside the arm's reach: {r:?}");
        assert!(r.achieved < 0.05, "lands on the centre: {r:?}");
        let c = centroid(&m, d, &p);
        let aim = aim_for(&f, d, &md).unwrap();
        assert!((c[0] - aim.contact[0]).abs() < 0.05 && (c[1] - aim.contact[1]).abs() < 0.05);
    }

    #[test]
    fn dtilt_and_dsmash_really_crouch_not_just_offset_the_root() {
        // A family's `crouch` root offset alone is not a real lower stance:
        // the support leg's own FK angles do not shorten with it, so an
        // unbent leg sends the foot `crouch` units below the floor and
        // `support_lift` raises the root right back up, cancelling the
        // offset almost entirely (measured before this fix: dtilt and
        // dsmash both netted the same ~-0.99u drop regardless of their very
        // different authored crouch values). `crouch_leg` bends the knee
        // along with the offset so the drop actually shows. Guard the real
        // per-frame root height, not the authored constant, against ever
        // regressing back to that cancellation -- and keep the direction's
        // own ordering honest: dsmash ("much more seated", the low-sweep
        // family) must sit lower than dtilt ("moderately crouched").
        // (usmash used to be asserted here too, compressed at first active
        // -- the 2026-09 review flagged that exact shape as the bug
        // ["usmash comprime MAS durante activo y no transmite descarga
        // ascendente"], so that assertion doesn't belong in a "really
        // crouches" test any more. Its own compression -> extension arc is
        // covered by `usmash_compresses_at_wind_then_extends_by_first_active`
        // below.)
        let m = build(CharacterId::Kestrel);
        let root_i = m.rig.bone("root").unwrap();
        let root_y = |mid: MoveId| -> f32 {
            let md = attacks::data(CharacterId::Kestrel, mid);
            let f = fighter(mid, &md, md.startup);
            let p = fighter_pose(&m, &f, 0);
            m.rig.world(&p, &Xf::IDENTITY)[root_i].t.y
        };
        let stand_y = {
            let mut f = Fighter::new(CharacterId::Kestrel.data(), 0, Vec2::ZERO);
            f.facing = 1.0;
            f.grounded = true;
            let p = fighter_pose(&m, &f, 0);
            m.rig.world(&p, &Xf::IDENTITY)[root_i].t.y
        };
        let dtilt = root_y(MoveId::Dtilt);
        let dsmash = root_y(MoveId::Dsmash);
        assert!(
            dtilt < stand_y - 1.5,
            "dtilt should sit visibly lower than standing: {dtilt} vs stand {stand_y}"
        );
        assert!(
            dsmash < stand_y - 1.5,
            "dsmash should sit visibly lower than standing: {dsmash} vs stand {stand_y}"
        );
        assert!(
            dsmash < dtilt - 0.3,
            "dsmash (\"much more seated\") should sit lower than dtilt (\"moderately crouched\"): dsmash {dsmash} dtilt {dtilt}"
        );
    }

    #[test]
    fn usmash_compresses_at_wind_then_extends_by_first_active() {
        // 2026-09 review, Windows 882cac5: "usmash comprime MAS durante
        // activo y no transmite descarga ascendente" -- usmash was
        // compressing *more* through the active window and never read as
        // an upward release. The direction: real compression already
        // visible at startup/charge (Wind), knee/pelvis extension upward
        // by the first active frame -- a genuine compression -> extension
        // arc, not a held crouch (that would just be "a standing brace",
        // the old failure mode `crouch_active` replaced) and not a pop
        // *above* neutral standing (that would float the feet --
        // `support_lift` only ever corrects a sunk foot, never a floating
        // one). This replaces the old `dtilt_dsmash_and_usmash_really_
        // crouch_...` assertion that usmash must sit *below* standing at
        // first active, which directly contradicted this direction.
        let m = build(CharacterId::Kestrel);
        let root_i = m.rig.bone("root").unwrap();
        let md = attacks::data(CharacterId::Kestrel, MoveId::Usmash);
        let d = m.directions.as_ref().unwrap().get(MoveId::Usmash).unwrap();
        let stand_y = {
            let mut f = Fighter::new(CharacterId::Kestrel.data(), 0, Vec2::ZERO);
            f.facing = 1.0;
            f.grounded = true;
            let p = fighter_pose(&m, &f, 0);
            m.rig.world(&p, &Xf::IDENTITY)[root_i].t.y
        };
        // Wind: sampled at the authored anticipation peak (matching
        // `anticipation_peaks_at_the_authored_fraction...`'s own pattern)
        // -- the frame the base->wind ease curve actually finishes
        // rising on, not an arbitrary earlier midpoint still mostly
        // blended toward the base idle stance.
        let peak = (d.fraction * md.startup as f32).round() as u32;
        let wind_f = fighter(MoveId::Usmash, &md, peak);
        let wind_p = fighter_pose(&m, &wind_f, 0);
        let wind_y = m.rig.world(&wind_p, &Xf::IDENTITY)[root_i].t.y;
        // First active frame: real runtime event, same fixture the
        // simulation itself uses.
        let active_f = fighter(MoveId::Usmash, &md, md.startup);
        let active_p = fighter_pose(&m, &active_f, 0);
        let active_y = m.rig.world(&active_p, &Xf::IDENTITY)[root_i].t.y;
        assert!(
            wind_y < stand_y - 1.5,
            "usmash should show real compression at Wind (startup/charge), not a standing brace: {wind_y} vs stand {stand_y}"
        );
        assert!(
            active_y > wind_y + 1.0,
            "usmash should extend back up by its first active frame, not stay compressed: wind {wind_y} -> active {active_y}"
        );
        assert!(
            active_y <= stand_y + 0.5,
            "the extension is a rise off the Wind compression back toward standing, not an artificial pop above it (would float the feet): active {active_y} vs stand {stand_y}"
        );
        // Real contact check, not weakened: the arm still has to reach the
        // real runtime hitbox on the first active frame.
        let (_, reach, _) = contact_pose(&m, d, &active_f, &md, 0, Stage::Contact, Event::Hitbox);
        let reach = reach.unwrap();
        let aim = aim_for(&active_f, d, &md).unwrap();
        assert!(
            reach.achieved <= aim.radius,
            "usmash's contact piece must still intersect the real hitbox at first active: {reach:?} radius {}",
            aim.radius
        );
    }

    #[test]
    fn neither_foot_penetrates_the_floor_on_any_tick_of_the_redirected_smashes() {
        // Windows review of the visual patch: dsmash's *striking* foot sank
        // through the stage during the anticipation window and through the
        // wind -> contact blend (measured, both facings: sf2 -0.281, sf3
        // -0.247, sf4 -0.239, sf5 -1.557). `support_lift` cannot catch that
        // one: it deliberately skips the directed action's own contact foot
        // in every stage (otherwise the floor correction would drag the
        // strike off its aim), so only the chamber's own geometry keeps that
        // foot above the plane.
        //
        // Sample the REAL per-tick drawn pose -- every state_frame of the
        // whole cycle, not just the key stages, so the base -> wind and
        // wind -> contact blends and the whole recovery are covered -- for
        // both facings and both smashes this patch redirected. The active
        // ticks additionally re-assert the strict contact requirement in the
        // same test, so a future "fix" can never buy clean feet by pulling
        // the strike out of its hit region.
        let m = build(CharacterId::Kestrel);
        for mid in [MoveId::Dsmash, MoveId::Usmash] {
            let md = attacks::data(CharacterId::Kestrel, mid);
            let d = m.directions.as_ref().unwrap().get(mid).unwrap();
            let last_active = md.startup + md.active + md.late_active;
            for facing in [1.0f32, -1.0] {
                for sf in 0..md.total() {
                    let mut f = Fighter::new(CharacterId::Kestrel.data(), 0, Vec2::ZERO);
                    f.facing = facing;
                    f.grounded = true;
                    f.set_state_pub(State::Attack {
                        id: mid,
                        aerial: md.is_aerial,
                    });
                    f.state_frame = sf;
                    let p = fighter_pose(&m, &f, 0);
                    for s in super::super::contact::foot_support(&m, &p, &Xf::IDENTITY, 0.0) {
                        assert!(
                            s.support_distance > -1e-3,
                            "{mid:?} facing {facing:+} sf {sf}: {} sinks {:.4} through the floor",
                            s.piece_id,
                            s.support_distance
                        );
                    }
                    if sf >= md.startup && sf < last_active {
                        let c = centroid(&m, d, &p);
                        let aim = aim_for(&f, d, &md).unwrap();
                        let gap = ((c[0] - aim.contact[0]).powi(2)
                            + (c[1] - aim.contact[1]).powi(2))
                        .sqrt();
                        assert!(
                            gap <= aim.radius,
                            "{mid:?} facing {facing:+} sf {sf}: the contact piece left its real hit region ({gap:.4} > r{:.4}) -- clean feet must never be bought with a missed strike",
                            aim.radius
                        );
                    }
                }
            }
        }
    }

    /// `cargo test --lib diag_feet -- --ignored --nocapture` prints the
    /// per-tick support distance of BOTH feet, every state_frame of the
    /// whole cycle, both facings, for the smashes this patch redirected --
    /// the measurement behind
    /// `neither_foot_penetrates_the_floor_on_any_tick_of_the_redirected_smashes`.
    /// Report-only: it asserts nothing, so it can be run against a broken
    /// tree to see exactly which ticks sink and by how much.
    #[test]
    #[ignore]
    fn diag_feet_support_per_tick() {
        let m = build(CharacterId::Kestrel);
        for mid in [MoveId::Dsmash, MoveId::Usmash] {
            let md = attacks::data(CharacterId::Kestrel, mid);
            for facing in [1.0f32, -1.0] {
                println!(
                    "=== {mid:?} facing {facing:+} startup={} active={} late={} endlag={} ===",
                    md.startup, md.active, md.late_active, md.endlag
                );
                for sf in 0..md.total() {
                    let mut f = Fighter::new(CharacterId::Kestrel.data(), 0, Vec2::ZERO);
                    f.facing = facing;
                    f.grounded = true;
                    f.set_state_pub(State::Attack {
                        id: mid,
                        aerial: md.is_aerial,
                    });
                    f.state_frame = sf;
                    let p = fighter_pose(&m, &f, 0);
                    let phase = if sf < md.startup {
                        "windup"
                    } else if sf < md.startup + md.active + md.late_active {
                        "active"
                    } else {
                        "recovery"
                    };
                    let cells: Vec<String> =
                        super::super::contact::foot_support(&m, &p, &Xf::IDENTITY, 0.0)
                            .iter()
                            .map(|s| format!("{}={:+.4}", s.piece_id, s.support_distance))
                            .collect();
                    println!("sf={sf:<3} {phase:<8} {}", cells.join("  "));
                }
            }
        }
    }

    #[test]
    fn dash_attack_bends_the_elbow_instead_of_a_straight_punch() {
        // The runtime hitbox centre sits beyond the arm's maximum reach
        // (the review's complaint: "a fully extended punch ... despite the
        // authored running-forearm direction"), so solving straight at it
        // always degenerates to a fully extended, unbent line -- see
        // `solve`'s `rmax` clamp. `aim_pull` (RunningForearm's `Shape`)
        // pulls the solve target back within real reach so the elbow can
        // actually bend; guard that it does, and that the piece still
        // measures inside the real (unpulled) hit region afterwards.
        let m = build(CharacterId::Kestrel);
        let d = m
            .directions
            .as_ref()
            .unwrap()
            .get(MoveId::DashAttack)
            .unwrap();
        let md = attacks::data(CharacterId::Kestrel, MoveId::DashAttack);
        let f = fighter(MoveId::DashAttack, &md, md.startup);
        let (p, r, _) = contact_pose(&m, d, &f, &md, 0, Stage::Contact, Event::Hitbox);
        let fore = m.rig.bone("forearm_r").unwrap();
        let bend = p.rot[fore].z.abs();
        assert!(
            bend > 30.0,
            "forearm_r should show a real elbow bend, not a near-straight arm: {bend:.1} deg"
        );
        let r = r.unwrap();
        assert!(
            r.reached,
            "the pulled target should be within real reach: {r:?}"
        );
        assert!(
            r.achieved <= md.hitbox.radius,
            "the piece must still land inside the real hitbox after the pull: {r:?}"
        );
    }

    #[test]
    fn every_action_puts_its_own_piece_in_its_own_hit_region() {
        let m = build(CharacterId::Kestrel);
        let mut unreachable = Vec::new();
        for fz in feasibility(&m) {
            if !fz.intersection_required {
                continue;
            }
            if !fz.guaranteed_intersection {
                unreachable.push(format!(
                    "{}: gap {:.3} > radius {:.3}",
                    fz.action_id, fz.centroid_gap, fz.hit_radius
                ));
            }
        }
        assert!(unreachable.is_empty(), "{}", unreachable.join("; "));
    }

    #[test]
    fn a_rearward_move_aims_backward_and_is_never_clamped_forward() {
        let m = build(CharacterId::Kestrel);
        let d = m.directions.as_ref().unwrap().get(MoveId::Bair).unwrap();
        let md = attacks::data(CharacterId::Kestrel, MoveId::Bair);
        assert!(md.hitbox.offset.x < 0.0, "back air really is rearward");
        let f = fighter(MoveId::Bair, &md, md.startup);
        let aim = aim_for(&f, d, &md).unwrap();
        assert_eq!(aim.contact[0], md.hitbox.offset.x);
        let (p, r, _) = contact_pose(&m, d, &f, &md, 0, Stage::Contact, Event::Hitbox);
        let c = centroid(&m, d, &p);
        assert!(c[0] < -6.0, "the left foot ends behind the root: {c:?}");
        assert!(
            r.unwrap().achieved <= md.hitbox.radius,
            "and inside the rear hitbox: {r:?}"
        );
        // The designated piece is the left foot, not the right one.
        assert_eq!(m.rig.bones[d.bone].name, "foot_l");
        assert_eq!(d.piece_id, "k_foot_l");
    }

    #[test]
    fn contact_is_held_through_the_whole_active_interval_late_frames_included() {
        let m = build(CharacterId::Kestrel);
        let d = m.directions.as_ref().unwrap().get(MoveId::Nair).unwrap();
        let md = attacks::data(CharacterId::Kestrel, MoveId::Nair);
        assert!(md.late_active > 0, "nair has a late window");
        let last = md.startup + md.active + md.late_active;
        for sf in md.startup..last {
            let f = fighter(MoveId::Nair, &md, sf);
            let p = fighter_pose(&m, &f, 0);
            let c = centroid(&m, d, &p);
            let aim = aim_for(&f, d, &md).unwrap();
            let gap = ((c[0] - aim.contact[0]).powi(2) + (c[1] - aim.contact[1]).powi(2)).sqrt();
            assert!(
                gap <= md.hitbox.radius,
                "sf {sf} of {last}: gap {gap:.3} > radius {}",
                md.hitbox.radius
            );
        }
        // And the first recovery frame really does leave it.
        let f = fighter(MoveId::Nair, &md, last + md.endlag - 1);
        let p = fighter_pose(&m, &f, 0);
        let c = centroid(&m, d, &p);
        let aim = aim_for(&f, d, &md).unwrap();
        assert!(
            ((c[0] - aim.contact[0]).powi(2) + (c[1] - aim.contact[1]).powi(2)).sqrt()
                > md.hitbox.radius * 0.5,
            "the leg is gathered by the end of recovery"
        );
    }

    #[test]
    fn anticipation_peaks_at_the_authored_fraction_without_adding_a_frame() {
        let m = build(CharacterId::Kestrel);
        let dirs = m.directions.as_ref().unwrap();
        // fsmash: fraction 0.7 of an 11-frame startup.
        let d = dirs.get(MoveId::Fsmash).unwrap();
        let md = attacks::data(CharacterId::Kestrel, MoveId::Fsmash);
        assert_eq!(d.fraction, 0.7);
        let gap = |sf: u32| {
            let f = fighter(MoveId::Fsmash, &md, sf);
            let p = fighter_pose(&m, &f, 0);
            let c = centroid(&m, d, &p);
            let aim = aim_for(&f, d, &md).unwrap();
            ((c[0] - aim.contact[0]).powi(2) + (c[1] - aim.contact[1]).powi(2)).sqrt()
        };
        let peak = (0.7 * md.startup as f32).round() as u32;
        // The coil is farthest from the target at the authored peak.
        for sf in 0..md.startup {
            if sf != peak {
                assert!(
                    gap(sf) <= gap(peak) + 1e-3,
                    "sf {sf} coils further than the peak {peak}"
                );
            }
        }
        // Contact is reached exactly on the first active frame, not before.
        assert!(gap(md.startup) <= md.hitbox.radius);
        assert!(gap(md.startup - 1) > md.hitbox.radius * 0.5);
        // A zero-fraction action hits immediately: jab's startup is 1.
        let jmd = attacks::data(CharacterId::Kestrel, MoveId::Jab);
        let jd = dirs.get(MoveId::Jab).unwrap();
        assert_eq!(jd.fraction, 0.0);
        let f = fighter(MoveId::Jab, &jmd, jmd.startup);
        let p = fighter_pose(&m, &f, 0);
        let c = centroid(&m, jd, &p);
        let aim = aim_for(&f, jd, &jmd).unwrap();
        assert!(
            ((c[0] - aim.contact[0]).powi(2) + (c[1] - aim.contact[1]).powi(2)).sqrt()
                <= jmd.hitbox.radius
        );
    }

    #[test]
    fn a_held_smash_freezes_on_the_anticipation_pose() {
        let m = build(CharacterId::Kestrel);
        let md = attacks::data(CharacterId::Kestrel, MoveId::Fsmash);
        let mut f = fighter(MoveId::Fsmash, &md, (md.startup / 2).max(1) - 1);
        let free = fighter_pose(&m, &f, 0);
        f.charge = 20;
        f.charge_armed = true;
        let held = fighter_pose(&m, &f, 0);
        let d = m.directions.as_ref().unwrap().get(MoveId::Fsmash).unwrap();
        let aim = aim_for(&f, d, &md).unwrap();
        let gap = |p: &Pose| {
            let c = centroid(&m, d, p);
            ((c[0] - aim.contact[0]).powi(2) + (c[1] - aim.contact[1]).powi(2)).sqrt()
        };
        assert!(
            gap(&held) > gap(&free) - 1e-3,
            "charging holds the wind-up, never a half-extended strike"
        );
        // More charge only trembles: the arm stays coiled.
        f.charge = 55;
        let deep = fighter_pose(&m, &f, 0);
        assert!((gap(&deep) - gap(&held)).abs() < 2.0);
    }

    #[test]
    fn both_facings_give_the_same_pose_and_the_root_does_the_turning() {
        let m = build(CharacterId::Kestrel);
        let md = attacks::data(CharacterId::Kestrel, MoveId::Ftilt);
        let mut a = fighter(MoveId::Ftilt, &md, md.startup);
        let pa = fighter_pose(&m, &a, 0);
        a.facing = -1.0;
        let pb = fighter_pose(&m, &a, 0);
        assert_eq!(pa, pb, "poses are authored in the root-local frame");
    }

    #[test]
    fn grounded_poses_rest_on_the_support_plane() {
        let m = build(CharacterId::Kestrel);
        for st in [
            State::Stand,
            State::Crouch,
            State::Hitstun { tumble: false },
            State::LandLag { total: 12 },
            State::Shield,
            State::Attack {
                id: MoveId::Ftilt,
                aerial: false,
            },
        ] {
            let mut f = Fighter::new(CharacterId::Kestrel.data(), 0, Vec2::ZERO);
            f.grounded = true;
            f.facing = 1.0;
            f.set_state_pub(st);
            f.state_frame = 6;
            let p = fighter_pose(&m, &f, 30);
            let lowest = super::super::contact::foot_support(&m, &p, &Xf::IDENTITY, 0.0)
                .iter()
                .map(|x| x.support_distance)
                .fold(f32::MAX, f32::min);
            assert!(
                lowest > -1e-3,
                "{st:?}: the lowest foot vertex is {lowest} below the floor"
            );
            assert!(
                lowest < 2.5,
                "{st:?}: the feet float {lowest} above the floor"
            );
        }
    }

    #[test]
    fn nothing_directed_touches_the_simulation_tables() {
        // The directions read frame data; they never write it. Building
        // every pose of every action leaves the move table identical.
        let m = build(CharacterId::Kestrel);
        let before: Vec<(u32, u32, u32, f32, f32)> = attacks::ALL_MOVES
            .iter()
            .map(|id| {
                let d = attacks::data(CharacterId::Kestrel, *id);
                (
                    d.startup,
                    d.active,
                    d.late_active,
                    d.hitbox.offset.x,
                    d.hitbox.radius,
                )
            })
            .collect();
        for id in attacks::ALL_MOVES {
            let md = attacks::data(CharacterId::Kestrel, id);
            for sf in 0..md.total() {
                let f = fighter(id, &md, sf);
                let _ = fighter_pose(&m, &f, sf as u64);
            }
        }
        let after: Vec<(u32, u32, u32, f32, f32)> = attacks::ALL_MOVES
            .iter()
            .map(|id| {
                let d = attacks::data(CharacterId::Kestrel, *id);
                (
                    d.startup,
                    d.active,
                    d.late_active,
                    d.hitbox.offset.x,
                    d.hitbox.radius,
                )
            })
            .collect();
        assert_eq!(before, after);
    }

    #[test]
    fn the_projectile_action_serves_its_one_real_event_the_release() {
        // Kestrel's special_n used to carry a second "event" eight frames
        // after the release: a zero-damage fighter hitbox that the Windows
        // baseline showed interrupting, freezing and staling on contact.
        // That placeholder was removed from the simulation deliberately
        // (`MoveData::no_melee`), so the action's animation/evidence
        // contract now has exactly one real event to serve -- the release,
        // and its recoil. See docs/art/procedural/anim/kestrel-gameplay-fixes.md.
        let m = build(CharacterId::Kestrel);
        let d = m
            .directions
            .as_ref()
            .unwrap()
            .get(MoveId::SpecialN)
            .unwrap();
        let md = attacks::data(CharacterId::Kestrel, MoveId::SpecialN);
        assert!(
            md.no_melee,
            "Kestrel's special_n declares no fighter hitbox"
        );

        // One event, from the simulation: the shot leaves on frame 0.
        let evs = events_of(d, &md);
        assert_eq!(evs, vec![(Event::Release, 0)]);
        assert_eq!(contact_frame(d, &md), 0);

        // The palm leads the emission line on the release frame -- the
        // real spawn origin, not the projectile's later flight.
        //
        // The piece's *centroid* does not sit on that origin and cannot:
        // the arm chain (upper_arm 5.0 + forearm 4.5, palm point 10.17u
        // from the shoulder at full extension) is shorter than the 14.29u
        // to the origin. That is a fact about the centroid, not a contact
        // test. Whether the piece's real projected surface reaches the
        // emission *region* is measured with the contract's own triangle
        // method in `tests/special_n_presentation.rs`, where it does.
        //
        // What is asserted here is the pose: the palm sits on the line from
        // the shoulder to the origin, with a real elbow bend and a compact
        // chest, and it extends as far as the chain allows rather than
        // giving up early.
        let f0 = fighter(MoveId::SpecialN, &md, 1);
        let rel = aim_for_event(&f0, d, &md, Event::Release).unwrap();
        assert_eq!(rel.kind, "projectile_spawn");
        assert_eq!(rel.contact[0], PROJECTILE_SPAWN_LOCAL_X);
        let p0 = fighter_pose(&m, &f0, 0);
        let c0 = centroid(&m, d, &p0);
        let gap0 = ((c0[0] - rel.contact[0]).powi(2) + (c0[1] - rel.contact[1]).powi(2)).sqrt();
        let g = geom(&m, d, &p0);
        println!(
            "[SPECIAL_N] release: palm=({:+.3},{:+.3}) gap_to_origin={gap0:.3} shoulder=({:+.3},{:+.3}) shoulder_to_origin={:.3} palm_reach={:.3} elbow={:.1}deg lean={:.1}deg",
            c0[0], c0[1], g.shoulder[0], g.shoulder[1], g.shoulder_to_origin, g.palm_reach, g.elbow, g.lean
        );
        // On the line, not merely near the origin: the perpendicular
        // distance from the palm to the shoulder->origin segment.
        assert!(
            g.off_line < 0.5,
            "the release points at the origin: {:.4}u off the shoulder->origin line",
            g.off_line
        );
        // A real bend, not the straight-arm thrust the old assist forced.
        assert!(
            (30.0..=70.0).contains(&g.elbow),
            "a bent-elbow palm release, not a straight arm: elbow {:.1} deg",
            g.elbow
        );
        // A compact chest. The pose this replaces measured 38.2 degrees.
        assert!(
            g.lean <= 26.0 + 1e-3,
            "a compact torso, not the old forward lunge: chest {:.1} deg",
            g.lean
        );
        // And the residual gap is bounded, so it can never quietly grow:
        // this is a reported limitation, not a free tolerance.
        assert!(
            (gap0 - (g.shoulder_to_origin - g.palm_reach)).abs() < 0.25,
            "the centroid sits at the arm's own reach limit, not short of it: {gap0:.3}u vs {:.3}u",
            g.shoulder_to_origin - g.palm_reach
        );

        // There is no fighter-hitbox aim left to serve, on any frame: the
        // hitbox event resolves to the same spawn origin, and it is not a
        // required contact, because no contact exists to require.
        let hit = aim_for(&f0, d, &md).unwrap();
        assert_eq!(hit.kind, "projectile_spawn");
        assert!(!hit.required);
        for sf in 0..md.total() {
            assert!(
                md.hitbox_at(sf).is_none(),
                "sf {sf}: special_n must define no fighter hitbox at all"
            );
        }

        // The single event is reported, and the report no longer claims a
        // required intersection that does not exist.
        let fz = feasibility(&m)
            .into_iter()
            .find(|x| x.action_id == "special_n")
            .unwrap();
        assert_eq!(fz.events.len(), 1);
        assert_eq!(fz.aim_kind, "projectile_spawn");
        assert!(!fz.intersection_required);
        assert!(fz.events.iter().any(|e| e.event == "projectile_release"));

        // The hand still never chases the projectile downrange: by the end
        // of the move it is recovering, not further forward.
        let mut g = f0.clone();
        g.state_frame = md.total() - 1;
        let b = centroid(&m, d, &fighter_pose(&m, &g, 0));
        assert!(
            b[0] < rel.contact[0],
            "the palm does not follow the shot: {b:?}"
        );

        // Kestrel only. This is not a global "zero damage does not collide"
        // rule: the other projectile owners keep their own fighter hitbox
        // until they have their own evidence.
        for ch in [CharacterId::Boulder, CharacterId::Viper] {
            let other = attacks::data(ch, MoveId::SpecialN);
            assert!(!other.no_melee, "{ch:?} special_n keeps its own hitbox");
            assert!(other.hitbox_at(other.startup).is_some());
        }
    }

    #[test]
    fn throws_use_the_hold_position_and_the_release_parameters() {
        let m = build(CharacterId::Kestrel);
        for (id, back) in [
            (MoveId::ThrowF, false),
            (MoveId::ThrowB, true),
            (MoveId::ThrowU, false),
            (MoveId::ThrowD, false),
        ] {
            let d = m.directions.as_ref().unwrap().get(id).unwrap();
            let md = attacks::data(CharacterId::Kestrel, id);
            let f = fighter(id, &md, md.startup);
            let aim = aim_for(&f, d, &md).unwrap();
            assert_eq!(aim.kind, "throw_release");
            assert!(!aim.required, "a throw has no melee hitbox to intersect");
            assert_eq!(aim.coil_abs.unwrap()[0], HOLD_LOCAL_X);
            let p = fighter_pose(&m, &f, 0);
            let c = centroid(&m, d, &p);
            if back {
                assert!(c[0] < 0.0, "{id:?}: the hand opens backward: {c:?}");
                assert_eq!(d.piece_id, "k_hand_l");
            } else {
                assert!(c[0] > 0.0, "{id:?}: the hand opens forward: {c:?}");
            }
            // Before the release the hands are at the held target.
            let mut h = f.clone();
            h.state_frame = 0;
            let hc = centroid(&m, d, &fighter_pose(&m, &h, 0));
            assert!(hc[0] > 2.0, "{id:?}: the hold is in front: {hc:?}");
        }
    }

    #[test]
    fn the_radial_action_centres_the_body_instead_of_reaching() {
        let m = build(CharacterId::Kestrel);
        let d = m
            .directions
            .as_ref()
            .unwrap()
            .get(MoveId::SpecialDown)
            .unwrap();
        let md = attacks::data(CharacterId::Kestrel, MoveId::SpecialDown);
        assert_eq!(md.startup, 0, "zero startup: an immediate visual response");
        let f = fighter(MoveId::SpecialDown, &md, 0);
        assert!(aim_for(&f, d, &md).is_none(), "no limb solve");
        assert_eq!(d.piece_id, "k_chest");
        let p = fighter_pose(&m, &f, 0);
        let c = centroid(&m, d, &p);
        let mid = f.character.height * 0.5;
        let gap = (c[0].powi(2) + (c[1] - mid).powi(2)).sqrt();
        assert!(
            gap < md.hitbox.radius,
            "the chest sits inside the effect: {gap} vs {}",
            md.hitbox.radius
        );
    }
}
