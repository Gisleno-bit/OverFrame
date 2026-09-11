//! Measured contact geometry: where the striking piece of a fighter's model
//! actually is, in world space, relative to the hitbox the simulation tests
//! on that tick. This is what the art review reads instead of eyeballing an
//! overlay (`docs/art/reviews/*.md`, "export world-space contact-piece
//! geometry and signed separation").
//!
//! Pure CPU. A measurement is only ever taken on a pose and root that
//! came out of [`super::render_eval::evaluate`] — the exact transform the
//! renderer draws, eased facing and hitlag rattle included — so a
//! diagnostic never describes a mesh other than the one on screen.
//! Nothing here feeds back into the simulation.

use super::anim::{self, Limb};
use super::characters::CharacterModel;
use super::math3::{v3, Xf, V3};
use super::rig::Pose;
use crate::sim::attacks::MoveId;
use crate::sim::fighter::Fighter;
use serde::Serialize;

/// World-space geometry of the contact piece on one tick.
#[derive(Debug, Clone, Serialize)]
pub struct ContactMeasure {
    /// Which move the measurement is for (its strike limb picks the piece).
    pub action_id: &'static str,
    /// Bone that carries the contact piece (`hand_r`, `foot_l`, `chest`…).
    pub bone: String,
    /// Piece ids on that bone (spec-built models) or the bone name.
    pub piece_ids: Vec<String>,
    /// Fighter root and sim facing the measurement was taken with, and the
    /// eased (drawn) facing actually used for the root transform.
    pub root: [f32; 2],
    pub facing: f32,
    pub eased_facing: f32,
    /// World position of the bone's joint.
    pub joint: [f32; 3],
    /// Centroid and axis-aligned bounds of the piece's vertices.
    pub centroid: [f32; 3],
    pub aabb_min: [f32; 3],
    pub aabb_max: [f32; 3],
    /// Runtime hitbox on this tick (centre x, y; radius), if any.
    pub hitbox: Option<HitboxRef>,
    /// **Method.** The piece's triangles are projected onto the XY fighting
    /// plane (the plane the simulation tests in). `mesh_distance` is the
    /// minimum Euclidean distance from the hitbox centre to that projected
    /// surface — 0 when the centre lies inside any triangle, otherwise the
    /// distance to the nearest triangle edge — so an edge or face crossing
    /// the circle counts even when every vertex is outside it.
    /// `signed_separation = mesh_distance − radius`: negative = the piece
    /// penetrates the hitbox circle, positive = the gap to its near edge.
    /// `nearest_point` is the point of the projected surface that realises
    /// the distance (z taken from the piece's front, for drawing).
    /// Exceptions: throws have no fighter hitbox (all three are `None`);
    /// body-centred hitboxes (chest piece) report 0 / −radius when the
    /// centre sits inside the chest silhouette, which says nothing about
    /// the pose's readability. `special_n` is *not* an exception here: its
    /// own fighter hitbox (a real melee hitbox, separate from the
    /// projectile it emits) is measured against the contact piece exactly
    /// like every other action — the caller may additionally measure the
    /// same piece against the projectile object itself (a second, later
    /// runtime event), but that is an addition, never a substitute.
    pub mesh_distance: Option<f32>,
    pub nearest_point: Option<[f32; 3]>,
    pub signed_separation: Option<f32>,
    /// Diagnostic only: the vertex nearest to the hitbox centre and its
    /// distance (always ≥ `mesh_distance`).
    pub nearest_vertex: Option<[f32; 3]>,
    pub vertex_distance: Option<f32>,
    /// Triangles of the piece that were measured.
    pub triangle_count: usize,
    /// The vertex farthest along the fighter's facing (the "tip"), and its
    /// separation from the hitbox centre along X, for reach comparisons.
    pub tip: [f32; 3],
    pub tip_reach_x: f32,
}

#[derive(Debug, Clone, Serialize)]
pub struct HitboxRef {
    pub id: String,
    pub center: [f32; 2],
    pub radius: f32,
}

/// Distance from `c` to the segment `a`–`b` in 2D, and the closest point.
pub fn point_segment_xy(c: [f32; 2], a: [f32; 2], b: [f32; 2]) -> (f32, [f32; 2]) {
    let (abx, aby) = (b[0] - a[0], b[1] - a[1]);
    let len2 = abx * abx + aby * aby;
    let t = if len2 <= 1e-12 {
        0.0
    } else {
        (((c[0] - a[0]) * abx + (c[1] - a[1]) * aby) / len2).clamp(0.0, 1.0)
    };
    let p = [a[0] + abx * t, a[1] + aby * t];
    ((c[0] - p[0]).hypot(c[1] - p[1]), p)
}

/// Twice the signed area of the 2D triangle `t`.
pub fn triangle_area2_xy(t: &[[f32; 2]; 3]) -> f32 {
    (t[1][0] - t[0][0]) * (t[2][1] - t[0][1]) - (t[2][0] - t[0][0]) * (t[1][1] - t[0][1])
}

/// A projected triangle with (numerically) no area — an edge-on face or
/// coincident points. It has no interior, so it is measured by its edges.
pub fn triangle_degenerate_xy(t: &[[f32; 2]; 3]) -> bool {
    let len2 = |a: [f32; 2], b: [f32; 2]| (b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2);
    let scale = len2(t[0], t[1]).max(len2(t[1], t[2])).max(len2(t[2], t[0]));
    triangle_area2_xy(t).abs() <= 1e-7 * (1.0 + scale)
}

/// Is `c` inside (or on the boundary of) the 2D triangle `t`? A degenerate
/// triangle has no interior and always answers `false` (the sign test alone
/// would accept every point on its line, or every point at all when the
/// three vertices coincide).
pub fn point_in_triangle_xy(c: [f32; 2], t: &[[f32; 2]; 3]) -> bool {
    if triangle_degenerate_xy(t) {
        return false;
    }
    let sign = |p: [f32; 2], a: [f32; 2], b: [f32; 2]| {
        (p[0] - b[0]) * (a[1] - b[1]) - (a[0] - b[0]) * (p[1] - b[1])
    };
    let d1 = sign(c, t[0], t[1]);
    let d2 = sign(c, t[1], t[2]);
    let d3 = sign(c, t[2], t[0]);
    let has_neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
    let has_pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
    !(has_neg && has_pos)
}

/// Minimum distance from `c` to a set of 2D triangles (0 if `c` is inside
/// one), and the point of the surface that realises it.
pub fn mesh_distance_xy(tris: &[[[f32; 2]; 3]], c: [f32; 2]) -> Option<(f32, [f32; 2])> {
    let mut best: Option<(f32, [f32; 2])> = None;
    for t in tris {
        if point_in_triangle_xy(c, t) {
            return Some((0.0, c));
        }
        for i in 0..3 {
            let (d, p) = point_segment_xy(c, t[i], t[(i + 1) % 3]);
            if best.map_or(true, |(bd, _)| d < bd) {
                best = Some((d, p));
            }
        }
    }
    best
}

/// Tip and base bone for a strike limb (mirrors `Scene3D`'s trail logic).
pub fn limb_bones(limb: Limb) -> (&'static str, &'static str) {
    match limb {
        Limb::ArmL => ("hand_l", "forearm_l"),
        Limb::LegR | Limb::BothLegs => ("foot_r", "shin_r"),
        Limb::LegL => ("foot_l", "shin_l"),
        Limb::Body => ("chest", "hips"),
        Limb::ArmR | Limb::BothArms => ("hand_r", "forearm_r"),
    }
}

/// Measure the contact piece of `f` for `action` on an already evaluated
/// `pose` / `root` (from [`super::render_eval::evaluate`]), against
/// `hitbox` (id, centre, radius) if the tick has one.
#[allow(clippy::too_many_arguments)]
pub fn measure_posed(
    model: &CharacterModel,
    f: &Fighter,
    pose: &Pose,
    root: &Xf,
    eased_facing: f32,
    action: MoveId,
    hitbox: Option<(&str, [f32; 2], f32)>,
) -> Option<ContactMeasure> {
    let rig = &model.rig;
    let world = rig.world(pose, root);

    // The designated contact piece of the authored direction — exactly that
    // piece, never every piece on its bone and never an accessory
    // (`docs/art/procedural/anim/FORMAT.md`). Fighters without a direction
    // file fall back to the generic strike limb and its whole bone.
    let directed = model.directions.as_ref().and_then(|d| d.get(action));
    let bi = match directed {
        Some(d) => d.bone,
        None => rig.bone(limb_bones(anim::strike_spec(action).limb).0)?,
    };
    let bone = &rig.bones[bi];
    if bone.mesh.pos.is_empty() {
        return None;
    }
    // Which vertices belong to which piece (spec-built models keep ranges).
    let ranges: Vec<(String, u32, u32)> = match directed {
        Some(d) => {
            let p = &model.pieces[d.piece];
            vec![(p.id.clone(), p.vertices.0, p.vertices.1)]
        }
        None => model
            .pieces
            .iter()
            .filter(|p| p.bone == bi)
            .map(|p| (p.id.clone(), p.vertices.0, p.vertices.1))
            .collect(),
    };
    let piece_ids: Vec<String> = if ranges.is_empty() {
        vec![bone.name.clone()]
    } else {
        ranges.iter().map(|r| r.0.clone()).collect()
    };
    // World positions of the whole bone (triangles index into these) and,
    // separately, only the designated piece's own vertices — the ones that
    // describe the contact.
    let all: Vec<V3> = bone.mesh.pos.iter().map(|p| world[bi].point(*p)).collect();
    let in_piece = |vi: u32| ranges.is_empty() || ranges.iter().any(|r| vi >= r.1 && vi < r.2);
    let verts: Vec<V3> = all
        .iter()
        .enumerate()
        .filter(|(i, _)| in_piece(*i as u32))
        .map(|(_, p)| *p)
        .collect();
    if verts.is_empty() {
        return None;
    }
    // Triangles that belong to the designated piece: every index inside one
    // of the piece ranges (pieces are appended contiguously, so a triangle
    // never straddles two pieces). Legacy models: the whole bone.
    let tris: Vec<[[f32; 2]; 3]> = bone
        .mesh
        .idx
        .chunks(3)
        .filter(|t| t.len() == 3 && t.iter().all(|i| in_piece(*i)))
        .map(|t| {
            let p = |i: u32| {
                let v = all[i as usize];
                [v.x, v.y]
            };
            [p(t[0]), p(t[1]), p(t[2])]
        })
        .collect();

    let n = verts.len() as f32;
    let centroid = verts.iter().fold(V3::ZERO, |a, p| a + *p) * (1.0 / n);
    let mut lo = v3(f32::MAX, f32::MAX, f32::MAX);
    let mut hi = v3(f32::MIN, f32::MIN, f32::MIN);
    for p in &verts {
        lo = v3(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z));
        hi = v3(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z));
    }
    // "Tip" = the vertex farthest along the *strike* direction, which is the
    // fighter's facing for a forward move and the opposite for a rearward
    // one (back air): the hitbox's own local X sign decides, so a rearward
    // reach is never reported as a negative forward reach.
    let axis = match hitbox {
        Some((_, c, _)) if ((c[0] - f.pos.x) * f.facing).abs() > 1e-3 => {
            ((c[0] - f.pos.x) * f.facing).signum() * f.facing
        }
        _ => f.facing,
    };
    let tip = *verts
        .iter()
        .max_by(|a, b| (a.x * axis).total_cmp(&(b.x * axis)))
        .unwrap();

    let mut out = ContactMeasure {
        action_id: crate::export::action_id(action),
        bone: bone.name.clone(),
        piece_ids,
        root: [f.pos.x, f.pos.y],
        facing: f.facing,
        eased_facing,
        joint: [world[bi].t.x, world[bi].t.y, world[bi].t.z],
        centroid: [centroid.x, centroid.y, centroid.z],
        aabb_min: [lo.x, lo.y, lo.z],
        aabb_max: [hi.x, hi.y, hi.z],
        hitbox: None,
        mesh_distance: None,
        nearest_point: None,
        signed_separation: None,
        nearest_vertex: None,
        vertex_distance: None,
        triangle_count: tris.len(),
        tip: [tip.x, tip.y, tip.z],
        tip_reach_x: (tip.x - f.pos.x) * axis,
    };
    if let Some((id, c, r)) = hitbox {
        out.hitbox = Some(HitboxRef {
            id: id.to_string(),
            center: c,
            radius: r,
        });
        out.tip_reach_x = (tip.x - c[0]) * axis;
        let nearest = *verts
            .iter()
            .min_by(|a, b| {
                let da = (a.x - c[0]).hypot(a.y - c[1]);
                let db = (b.x - c[0]).hypot(b.y - c[1]);
                da.total_cmp(&db)
            })
            .unwrap();
        out.nearest_vertex = Some([nearest.x, nearest.y, nearest.z]);
        out.vertex_distance = Some((nearest.x - c[0]).hypot(nearest.y - c[1]));
        if let Some((d, p)) = mesh_distance_xy(&tris, c) {
            out.mesh_distance = Some(d);
            out.nearest_point = Some([p[0], p[1], hi.z]);
            out.signed_separation = Some(d - r);
        }
    }
    Some(out)
}

/// Where a foot piece really is against the plane the fighter stands on.
///
/// The review asked for this by name: a pose can look right and still put a
/// foot through the floor, and only the mesh bounds say so. Measured on the
/// drawn pose, like every other diagnostic here.
#[derive(Debug, Clone, Serialize)]
pub struct FootSupport {
    pub piece_id: String,
    pub bone: String,
    pub aabb_min: [f32; 3],
    pub aabb_max: [f32; 3],
    /// Lowest vertex of the piece minus the support plane: negative means
    /// the drawn foot is below the floor.
    pub support_distance: f32,
}

/// Foot mesh bounds and support-plane distances for `pose` under `root`.
/// `plane_y` is the surface the fighter stands on (its own root while
/// grounded); pass `0.0` with an identity root for a root-local answer.
pub fn foot_support(
    model: &CharacterModel,
    pose: &Pose,
    root: &Xf,
    plane_y: f32,
) -> Vec<FootSupport> {
    let rig = &model.rig;
    let world = rig.world(pose, root);
    let mut out = Vec::new();
    for id in ["k_foot_r", "k_foot_l"] {
        let Some(pr) = model.pieces.iter().find(|p| p.id == id) else {
            continue;
        };
        let mesh = &rig.bones[pr.bone].mesh;
        let mut lo = v3(f32::MAX, f32::MAX, f32::MAX);
        let mut hi = v3(f32::MIN, f32::MIN, f32::MIN);
        for p in &mesh.pos[pr.vertices.0 as usize..pr.vertices.1 as usize] {
            let q = world[pr.bone].point(*p);
            lo = v3(lo.x.min(q.x), lo.y.min(q.y), lo.z.min(q.z));
            hi = v3(hi.x.max(q.x), hi.y.max(q.y), hi.z.max(q.z));
        }
        out.push(FootSupport {
            piece_id: id.to_string(),
            bone: rig.bones[pr.bone].name.clone(),
            aabb_min: [lo.x, lo.y, lo.z],
            aabb_max: [hi.x, hi.y, hi.z],
            support_distance: lo.y - plane_y,
        });
    }
    out
}

/// The lowest support distance of `pose`, if the model has foot pieces.
pub fn lowest_support(model: &CharacterModel, pose: &Pose, root: &Xf, plane_y: f32) -> Option<f32> {
    foot_support(model, pose, root, plane_y)
        .iter()
        .map(|s| s.support_distance)
        .fold(None, |a: Option<f32>, x| Some(a.map_or(x, |v| v.min(x))))
}

/// World-space hurt capsule of a fighter (segment ends + radius), as the
/// simulation tests it this tick.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct CapsuleRef {
    pub a: [f32; 2],
    pub b: [f32; 2],
    pub radius: f32,
}

pub fn capsule(f: &Fighter) -> CapsuleRef {
    let (a, b) = f.hurt_segment();
    CapsuleRef {
        a: [a.x, a.y],
        b: [b.x, b.y],
        radius: f.hurt_radius(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::export;
    use crate::model::characters::build;
    use crate::model::render_eval::{evaluate, evaluate_and_measure, PortState};
    use crate::sim::roster::CharacterId;

    /// Replay a case's ticks through a fresh render history and measure the
    /// tick at `idx` (what the capture tool and the JSON export do).
    fn measure_tick(
        model: &CharacterModel,
        ticks: &[export::CaseTick],
        idx: usize,
        action: MoveId,
    ) -> ContactMeasure {
        let mut port = PortState::default();
        for t in &ticks[..idx] {
            evaluate(model, &mut port, &t.state.fighters[0], t.state.frame, true);
        }
        let t = &ticks[idx];
        let hb = t.row.center.map(|c| {
            (
                t.row.hitbox_id.as_str(),
                [c.0, c.1],
                t.row.radius.unwrap_or(0.0),
            )
        });
        let (_, m) = evaluate_and_measure(
            model,
            &mut port,
            &t.state.fighters[0],
            t.state.frame,
            true,
            action,
            hb,
        );
        m.unwrap()
    }

    #[test]
    fn kestrel_jab_contact_piece_is_the_right_hand_and_is_measured() {
        let model = build(CharacterId::Kestrel);
        let ticks = export::run_case(CharacterId::Kestrel, MoveId::Jab, "ground").unwrap();
        let idx = ticks.iter().position(|t| t.row.hitbox_active).unwrap();
        let first = &ticks[idx];
        let f = &first.state.fighters[0];
        let m = measure_tick(&model, &ticks, idx, MoveId::Jab);
        assert_eq!(m.bone, "hand_r");
        assert_eq!(m.piece_ids, vec!["k_hand_r".to_string()]);
        assert_eq!(m.facing, 1.0);
        assert_eq!(m.eased_facing, 1.0);
        assert!(m.signed_separation.unwrap().is_finite());
        assert!(m.triangle_count > 0);
        // Surface distance never exceeds the nearest-vertex distance.
        assert!(m.mesh_distance.unwrap() <= m.vertex_distance.unwrap() + 1e-4);
        assert_eq!(
            m.signed_separation.unwrap(),
            m.mesh_distance.unwrap() - first.row.radius.unwrap()
        );
        // The hand is in front of the root and its bounds are sane.
        assert!(
            m.centroid[0] > f.pos.x,
            "hand ahead of the root: {:?}",
            m.centroid
        );
        assert!(m.aabb_max[0] >= m.aabb_min[0] && m.aabb_max[1] >= m.aabb_min[1]);
        assert!(m.tip_reach_x.is_finite());
    }

    #[test]
    fn facing_left_mirrors_the_measurement() {
        let model = build(CharacterId::Kestrel);
        let right =
            export::run_case_facing(CharacterId::Kestrel, MoveId::Ftilt, "ground", 1.0).unwrap();
        let left =
            export::run_case_facing(CharacterId::Kestrel, MoveId::Ftilt, "ground", -1.0).unwrap();
        let ir = right.iter().position(|t| t.row.hitbox_active).unwrap();
        let il = left.iter().position(|t| t.row.hitbox_active).unwrap();
        let (fr, fl) = (&right[ir], &left[il]);
        assert_eq!(fr.row.state_frame, fl.row.state_frame);
        let cr = fr.row.center.unwrap();
        let cl = fl.row.center.unwrap();
        assert!(
            cr.0 > 0.0 && cl.0 < 0.0,
            "hitbox flips with facing: {cr:?} {cl:?}"
        );
        let mr = measure_tick(&model, &right, ir, MoveId::Ftilt);
        let ml = measure_tick(&model, &left, il, MoveId::Ftilt);
        assert_eq!(ml.facing, -1.0);
        assert_eq!(ml.eased_facing, -1.0, "constant facing: drawn == sim");
        let dr = mr.signed_separation.unwrap();
        let dl = ml.signed_separation.unwrap();
        assert!(
            (dr - dl).abs() < 1e-3,
            "separation is facing-symmetric: {dr} vs {dl}"
        );
        assert!((mr.mesh_distance.unwrap() - ml.mesh_distance.unwrap()).abs() < 1e-3);
        assert_eq!(mr.triangle_count, ml.triangle_count);
        assert!((mr.tip_reach_x - ml.tip_reach_x).abs() < 1e-3);
    }

    #[test]
    fn edge_crossing_the_circle_counts_even_with_every_vertex_outside() {
        // Triangle whose bottom edge passes 1 unit above the centre while
        // all three vertices are ≥ 10 units away.
        let tri = [[[-10.0, 1.0], [10.0, 1.0], [0.0, 20.0]]];
        let (d, p) = mesh_distance_xy(&tri, [0.0, 0.0]).unwrap();
        assert!((d - 1.0).abs() < 1e-5, "surface distance {d}");
        assert!((p[0]).abs() < 1e-5 && (p[1] - 1.0).abs() < 1e-5);
        // With radius 2 the piece penetrates by 1 (signed −1) although the
        // nearest vertex is ~10 away: a vertex-only test would miss it.
        let signed = d - 2.0;
        assert!((signed + 1.0).abs() < 1e-5);
        let vd = (10.0f32).hypot(1.0);
        assert!(vd > 2.0 && d < 2.0);
    }

    #[test]
    fn centre_inside_a_triangle_is_zero_distance() {
        let tri = [[[-5.0, -5.0], [5.0, -5.0], [0.0, 5.0]]];
        let (d, p) = mesh_distance_xy(&tri, [0.0, 0.0]).unwrap();
        assert_eq!(d, 0.0);
        assert_eq!(p, [0.0, 0.0]);
        // On the boundary also counts as inside.
        let (d, _) = mesh_distance_xy(&tri, [0.0, -5.0]).unwrap();
        assert_eq!(d, 0.0);
    }

    #[test]
    fn degenerate_triangles_are_measured_by_their_edges() {
        // Collinear: the sign test alone would call [100, 0] "inside".
        let line = [[[0.0, 0.0], [1.0, 0.0], [2.0, 0.0]]];
        assert!(triangle_degenerate_xy(&line[0]));
        assert!(!point_in_triangle_xy([100.0, 0.0], &line[0]));
        let (d, p) = mesh_distance_xy(&line, [100.0, 0.0]).unwrap();
        assert!((d - 98.0).abs() < 1e-4, "collinear outside: {d}");
        assert_eq!(p, [2.0, 0.0]);
        // A point on the segment itself is at distance 0 — via the edge,
        // not via a fake interior.
        let (d, _) = mesh_distance_xy(&line, [1.5, 0.0]).unwrap();
        assert!(d.abs() < 1e-6, "collinear inside the segment: {d}");
        // Three coincident points: a single point.
        let dot = [[[0.0, 0.0], [0.0, 0.0], [0.0, 0.0]]];
        assert!(triangle_degenerate_xy(&dot[0]));
        let (d, p) = mesh_distance_xy(&dot, [100.0, 100.0]).unwrap();
        assert!((d - 20000.0f32.sqrt()).abs() < 1e-2, "coincident: {d}");
        assert_eq!(p, [0.0, 0.0]);
        // A thin but valid triangle keeps its interior.
        let thin = [[[0.0, 0.0], [10.0, 0.0], [5.0, 0.01]]];
        assert!(!triangle_degenerate_xy(&thin[0]));
        assert!(point_in_triangle_xy([5.0, 0.005], &thin[0]));
        let (d, _) = mesh_distance_xy(&thin, [5.0, 0.005]).unwrap();
        assert_eq!(d, 0.0);
        let (d, _) = mesh_distance_xy(&thin, [5.0, 0.5]).unwrap();
        assert!(d > 0.48 && d < 0.5, "outside the thin triangle: {d}");
        // Mixed set: the degenerate face never hides the real answer.
        let mixed = [line[0], [[50.0, 0.0], [52.0, 0.0], [51.0, 2.0]]];
        let (d, _) = mesh_distance_xy(&mixed, [100.0, 0.0]).unwrap();
        assert!((d - 48.0).abs() < 1e-4, "mixed: {d}");
    }

    #[test]
    fn far_triangle_reports_positive_separation() {
        let tri = [[[10.0, 0.0], [12.0, 0.0], [11.0, 2.0]]];
        let (d, p) = mesh_distance_xy(&tri, [0.0, 0.0]).unwrap();
        assert!((d - 10.0).abs() < 1e-5);
        assert_eq!(p, [10.0, 0.0]);
        // Degenerate (edge-on) triangle still measures as its segment.
        let flat = [[[3.0, 0.0], [3.0, 4.0], [3.0, 8.0]]];
        let (d, _) = mesh_distance_xy(&flat, [0.0, 0.0]).unwrap();
        assert!((d - 3.0).abs() < 1e-5);
        assert!(mesh_distance_xy(&[], [0.0, 0.0]).is_none());
    }

    #[test]
    fn capture_order_does_not_change_a_measurement() {
        // Each case is measured from a fresh render history, so measuring
        // another case first (or the same one twice) changes nothing.
        let model = build(CharacterId::Kestrel);
        let ftilt = export::run_case(CharacterId::Kestrel, MoveId::Ftilt, "ground").unwrap();
        let nair = export::run_case(CharacterId::Kestrel, MoveId::Nair, "fullhop").unwrap();
        let i = ftilt.iter().position(|t| t.row.hitbox_active).unwrap();
        let j = nair.iter().position(|t| t.row.hitbox_active).unwrap();
        let a = measure_tick(&model, &ftilt, i, MoveId::Ftilt);
        let _ = measure_tick(&model, &nair, j, MoveId::Nair);
        let b = measure_tick(&model, &ftilt, i, MoveId::Ftilt);
        assert_eq!(a.signed_separation, b.signed_separation);
        assert_eq!(a.joint, b.joint);
        assert_eq!(a.tip, b.tip);
    }

    #[test]
    fn capsule_shrinks_in_crouch_and_matches_the_sim() {
        let mut f = Fighter::new(CharacterId::Kestrel.data(), 0, crate::sim::Vec2::ZERO);
        let idle = capsule(&f);
        f.set_state_pub(crate::sim::fighter::State::Crouch);
        let crouch = capsule(&f);
        assert_eq!(idle.radius, 10.5);
        assert!(crouch.b[1] < idle.b[1]);
        assert_eq!(idle.a[1], 10.5);
    }
}
