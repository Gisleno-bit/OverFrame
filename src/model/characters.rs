//! The three original fighters as segmented 3D models.
//!
//! Kestrel is built from the art specification
//! `docs/art/procedural/kestrel.json` (see `model::procedural` and
//! `docs/art/procedural/FORMAT.md`): 31 rigid pieces on 21 bones, with the
//! crest and scarf driven by the closed lag algorithm of that contract.
//! Boulder and Viper still use the hand-built models below until their round
//! of the art exchange; their extras are animated from the fighter's velocity
//! and the frame counter (`secondary`).
//!
//! Everything here is original design work: no proportions, colours or
//! features are taken from any existing game (see `docs/LEGAL.md`).

use super::anim::{humanoid_skeleton, AnimStyle, Proportions};
use super::lighting::{slot, Palette};
use super::math3::{v3, Xf, M3, V3};
use super::mesh::MeshData;
use super::procedural::{self, ExtraLag, PieceRange};
use super::rig::{Pose, Rig};
use crate::sim::fighter::{Fighter, State};
use crate::sim::roster::CharacterId;

/// A fighter's visual: rig + motion style + colour schemes.
pub struct CharacterModel {
    pub rig: Rig,
    pub style: AnimStyle,
    pub palettes: Vec<Palette>,
    /// Procedural animation of the extra bones (called after the base pose).
    pub secondary: fn(&Rig, &mut Pose, &Fighter, u64),
    /// Lagged extras (spec-built models only; empty otherwise). Applied by
    /// the renderer after `secondary`, with its own per-port history.
    pub extras: Vec<ExtraLag>,
    /// Piece id → vertex range, for spec-built models (diagnostics).
    pub pieces: Vec<PieceRange>,
    /// Which art specification built this model, if any.
    pub spec_id: Option<&'static str>,
}

impl CharacterModel {
    pub fn palette(&self, i: u8) -> &Palette {
        &self.palettes[(i as usize) % self.palettes.len()]
    }
}

pub fn build(id: CharacterId) -> CharacterModel {
    match id {
        CharacterId::Kestrel => kestrel(),
        CharacterId::Boulder => boulder(),
        CharacterId::Viper => viper(),
    }
}

/// The procedural model, unless an artist-made `.glb` is installed under
/// `assets/characters/` (see `model::assets`), in which case that rig is used
/// with the same motion style, palettes and secondary animation.
#[cfg(feature = "gltf")]
pub fn build_with_assets(id: CharacterId) -> CharacterModel {
    let mut m = build(id);
    if let Some(rig) = super::assets::character_rig(id) {
        m.rig = rig;
    }
    m
}

// ----------------------------------------------------------------- helpers

/// A limb segment hanging from its joint: a joint ball plus a tapered shaft
/// from the origin down to `-len`.
fn limb(r_top: f32, r_bot: f32, len: f32, slot_joint: u8, slot_shaft: u8) -> MeshData {
    let shaft = MeshData::cylinder(r_bot, r_top, len, 8, slot_shaft)
        .translate(v3(0.0, -len * 0.5, 0.0))
        .flat();
    let ball = MeshData::sphere(r_top * 1.05, 4, 8, slot_joint).flat();
    shaft.with(ball)
}

fn rot(x: f32, y: f32, z: f32) -> Xf {
    Xf::new(M3::euler(v3(x, y, z)), V3::ZERO)
}

fn at(x: f32, y: f32, z: f32) -> Xf {
    Xf::translation(v3(x, y, z))
}

/// Wedge-shaped foot pointing forward (+X) from the ankle.
fn foot(len: f32, w: f32, h: f32, s: u8) -> MeshData {
    MeshData::plate(
        &[
            (-len * 0.3, 0.0),
            (len * 0.7, 0.0),
            (len * 0.55, h),
            (-len * 0.25, h),
        ],
        w,
        s,
    )
    .translate(v3(0.0, -h, 0.0))
}

// ----------------------------------------------------------------- Kestrel

fn kestrel() -> CharacterModel {
    fn secondary(_rig: &Rig, _pose: &mut Pose, _f: &Fighter, _frame: u64) {}
    from_spec(
        "kestrel",
        procedural::KESTREL_JSON,
        AnimStyle {
            bob: 0.7,
            run_lean: 20.0,
            swing: 1.05,
            crouch_depth: 7.5,
            walk_cycle: 28.0,
            run_cycle: 14.0,
            heavy: 0.0,
        },
        secondary,
    )
}

/// Build a model from an embedded art specification. The specs are checked
/// by unit tests, so a failure here is a build-time authoring error.
fn from_spec(
    id: &'static str,
    json: &str,
    style: AnimStyle,
    secondary: fn(&Rig, &mut Pose, &Fighter, u64),
) -> CharacterModel {
    let spec = procedural::parse(json).unwrap_or_else(|e| panic!("{id}.json: {e}"));
    let built = procedural::build(&spec).unwrap_or_else(|e| panic!("{id}.json: {e}"));
    let palettes = super::palettes::of(match id {
        "kestrel" => CharacterId::Kestrel,
        "boulder" => CharacterId::Boulder,
        _ => CharacterId::Viper,
    })
    .to_vec();
    CharacterModel {
        rig: built.rig,
        style,
        palettes,
        secondary,
        extras: built.extras,
        pieces: built.pieces,
        spec_id: Some(id),
    }
}

// ----------------------------------------------------------------- Boulder

fn boulder() -> CharacterModel {
    let p = Proportions {
        thigh: 6.4,
        shin: 5.8,
        ankle: 1.8,
        torso: 13.0,
        neck: 0.4,
        shoulder_half: 7.6,
        hip_half: 3.4,
        upper_arm: 7.2,
        forearm: 7.0,
    };
    let mut r = humanoid_skeleton(&p);
    for i in 0..3 {
        r.add(&format!("orb{i}"), "chest", v3(0.0, 4.0, 0.0));
    }

    // Massive chest block with a glowing core and crack plates.
    r.attach(
        "chest",
        MeshData::bevel_box(8.6, 9.4, 10.5, 1.6, slot::PRIMARY).translate(v3(0.0, 2.2, 0.0)),
    );
    r.attach(
        "spine",
        MeshData::bevel_box(7.0, 5.0, 8.4, 1.2, slot::SECONDARY).translate(v3(0.0, 1.4, 0.0)),
    );
    r.attach(
        "hips",
        MeshData::bevel_box(7.8, 3.2, 9.0, 1.0, slot::DARK).translate(v3(0.0, 0.6, 0.0)),
    );
    r.attach(
        "chest",
        MeshData::sphere(1.7, 5, 10, slot::GLOW).translate(v3(3.9, 2.6, 0.0)),
    );
    for (y, z, a) in [
        (4.8f32, -2.6f32, 25.0f32),
        (0.4, 3.0, -35.0),
        (-1.2, -3.4, 60.0),
    ] {
        r.attach(
            "chest",
            MeshData::plate(
                &[(-2.2, -0.25), (2.2, -0.25), (2.2, 0.25), (-2.2, 0.25)],
                0.5,
                slot::ACCENT,
            )
            .transform(&(at(4.3, y, z) * rot(0.0, 90.0, a))),
        );
    }
    // Head: a small block sunk between the shoulders with a visor slit.
    r.attach(
        "head",
        MeshData::bevel_box(4.6, 3.8, 4.8, 0.9, slot::SECONDARY).translate(v3(0.3, 1.6, 0.0)),
    );
    r.attach(
        "head",
        MeshData::plate(
            &[(-1.9, -0.35), (1.9, -0.35), (1.9, 0.35), (-1.9, 0.35)],
            0.6,
            slot::GLOW,
        )
        .transform(&(at(2.7, 2.0, 0.0) * rot(90.0, 0.0, 0.0))),
    );
    for s in ["r", "l"] {
        // Pauldron + thick arms + block fists.
        r.attach(
            &format!("upper_arm_{s}"),
            MeshData::bevel_box(5.6, 4.4, 5.6, 1.3, slot::SECONDARY).translate(v3(0.0, 0.6, 0.0)),
        );
        r.attach(
            &format!("upper_arm_{s}"),
            limb(2.3, 2.0, p.upper_arm, slot::PRIMARY, slot::PRIMARY),
        );
        r.attach(
            &format!("forearm_{s}"),
            limb(2.2, 2.6, p.forearm, slot::DARK, slot::PRIMARY),
        );
        r.attach(
            &format!("hand_{s}"),
            MeshData::bevel_box(3.8, 3.6, 3.8, 0.9, slot::DARK).translate(v3(0.3, -1.4, 0.0)),
        );
        // Short pillar legs, slab feet.
        r.attach(
            &format!("thigh_{s}"),
            limb(2.5, 2.2, p.thigh, slot::PRIMARY, slot::PRIMARY),
        );
        r.attach(
            &format!("shin_{s}"),
            limb(2.2, 2.4, p.shin, slot::DARK, slot::SECONDARY),
        );
        r.attach(
            &format!("foot_{s}"),
            MeshData::bevel_box(5.6, 1.9, 4.2, 0.6, slot::DARK).translate(v3(0.9, -0.9, 0.0)),
        );
    }
    // Orbiting stones.
    for i in 0..3 {
        r.attach(
            &format!("orb{i}"),
            MeshData::bevel_box(1.6, 1.3, 1.5, 0.4, slot::ACCENT).translate(v3(7.5, 0.0, 0.0)),
        );
    }

    let palettes = super::palettes::BOULDER.to_vec();

    fn secondary(rig: &Rig, pose: &mut Pose, f: &Fighter, frame: u64) {
        let t = frame as f32;
        // Stones orbit the shoulders; they pull in tight while shielding and
        // flare out on smash startup (the super-armour tell).
        let (radius, speed) = match f.state {
            State::Shield | State::ShieldStun { .. } => (0.55, 0.16),
            State::Attack { id, .. } if crate::sim::attacks::is_smash(id) => (1.35, 0.30),
            _ => (1.0, 0.07),
        };
        for i in 0..3 {
            let a = t * speed * 57.3 + i as f32 * 120.0;
            pose.rot(
                rig,
                &format!("orb{i}"),
                0.0,
                a,
                12.0 * (t * 0.05 + i as f32).sin(),
            );
            pose.scale(rig, &format!("orb{i}"), v3(radius, 1.0, radius));
        }
    }

    CharacterModel {
        rig: r,
        style: AnimStyle {
            bob: 0.9,
            run_lean: 10.0,
            swing: 0.8,
            crouch_depth: 5.5,
            walk_cycle: 36.0,
            run_cycle: 20.0,
            heavy: 1.0,
        },
        palettes,
        secondary,
        extras: Vec::new(),
        pieces: Vec::new(),
        spec_id: None,
    }
}

// ----------------------------------------------------------------- Viper

fn viper() -> CharacterModel {
    let p = Proportions {
        thigh: 7.0,
        shin: 6.4,
        ankle: 1.2,
        torso: 9.6,
        neck: 1.8,
        shoulder_half: 3.6,
        hip_half: 1.9,
        upper_arm: 5.6,
        forearm: 5.4,
    };
    let mut r = humanoid_skeleton(&p);
    r.add("hood", "head", v3(-0.6, 1.2, 0.0));
    r.add("tail1", "hips", v3(-1.4, -0.4, 0.0));
    r.add("tail2", "tail1", v3(-4.0, 0.0, 0.0));
    r.add("tail3", "tail2", v3(-3.6, 0.0, 0.0));
    r.add("tail4", "tail3", v3(-3.2, 0.0, 0.0));
    r.add("tail5", "tail4", v3(-2.8, 0.0, 0.0));

    // Slim torso with banded scales (alternating slots).
    for (i, y) in [0.0f32, 1.5, 3.0].iter().enumerate() {
        let s = if i % 2 == 0 {
            slot::PRIMARY
        } else {
            slot::SECONDARY
        };
        r.attach(
            "spine",
            MeshData::cylinder(2.3, 2.5, 1.5, 8, s)
                .flat()
                .translate(v3(0.0, *y + 0.4, 0.0)),
        );
    }
    r.attach(
        "chest",
        MeshData::ellipsoid(2.6, 3.4, 3.4, 5, 8, slot::PRIMARY)
            .flat()
            .translate(v3(0.0, 1.8, 0.0)),
    );
    r.attach(
        "chest",
        MeshData::plate(
            &[(-1.4, -1.0), (1.4, -1.0), (1.0, 2.6), (-1.0, 2.6)],
            3.2,
            slot::SECONDARY,
        )
        .transform(&(at(1.9, 1.6, 0.0) * rot(0.0, 90.0, 0.0))),
    );
    r.attach(
        "hips",
        MeshData::cylinder(2.2, 2.0, 1.4, 8, slot::DARK)
            .flat()
            .translate(v3(0.0, 0.3, 0.0)),
    );
    // Head: long snout, hood fan behind.
    r.attach(
        "head",
        MeshData::ellipsoid(3.8, 2.3, 2.3, 5, 8, slot::PRIMARY)
            .flat()
            .translate(v3(1.0, 1.8, 0.0)),
    );
    r.attach(
        "head",
        MeshData::ellipsoid(1.4, 0.9, 1.5, 3, 6, slot::SECONDARY)
            .flat()
            .translate(v3(4.4, 1.3, 0.0)),
    );
    for z in [-1.0f32, 1.0] {
        r.attach(
            "head",
            MeshData::sphere(0.5, 3, 6, slot::GLOW).translate(v3(3.4, 2.4, z)),
        );
    }
    r.attach(
        "hood",
        MeshData::plate(
            &[
                (0.0, -2.6),
                (-1.6, 3.6),
                (0.6, 5.2),
                (2.8, 3.6),
                (2.0, -2.0),
            ],
            0.5,
            slot::SECONDARY,
        )
        .transform(&(at(0.0, 0.0, 0.0) * rot(0.0, 90.0, 0.0)))
        .transform(&Xf::new(M3::scale(v3(1.0, 1.0, 1.9)), V3::ZERO)),
    );
    for s in ["r", "l"] {
        r.attach(
            &format!("upper_arm_{s}"),
            limb(1.25, 1.0, p.upper_arm, slot::PRIMARY, slot::PRIMARY),
        );
        r.attach(
            &format!("forearm_{s}"),
            limb(1.1, 0.9, p.forearm, slot::DARK, slot::SECONDARY),
        );
        // Forearm fin + claw hand.
        r.attach(
            &format!("forearm_{s}"),
            MeshData::plate(&[(0.0, 0.0), (-3.0, -2.0), (0.0, -4.4)], 0.4, slot::ACCENT)
                .translate(v3(-0.9, -0.4, 0.0)),
        );
        r.attach(
            &format!("hand_{s}"),
            MeshData::cylinder(0.9, 0.1, 2.6, 6, slot::DARK)
                .flat()
                .translate(v3(0.0, -1.3, 0.0)),
        );
        r.attach(
            &format!("thigh_{s}"),
            limb(1.5, 1.15, p.thigh, slot::PRIMARY, slot::PRIMARY),
        );
        r.attach(
            &format!("shin_{s}"),
            limb(1.2, 0.95, p.shin, slot::DARK, slot::SECONDARY),
        );
        r.attach(&format!("foot_{s}"), foot(4.2, 1.9, 1.3, slot::DARK));
    }
    // Tail: tapered segments with accent spines.
    let radii = [1.7f32, 1.45, 1.2, 0.95, 0.65];
    let lens = [4.0f32, 3.6, 3.2, 2.8, 3.2];
    for i in 0..5 {
        let seg = MeshData::cylinder(
            radii[i],
            radii.get(i + 1).copied().unwrap_or(0.15),
            lens[i],
            8,
            if i % 2 == 0 {
                slot::PRIMARY
            } else {
                slot::SECONDARY
            },
        )
        .flat()
        .transform(&(at(-lens[i] * 0.5, 0.0, 0.0) * rot(0.0, 0.0, 90.0)));
        let spine = MeshData::plate(
            &[
                (0.0, 0.0),
                (-lens[i] * 0.6, radii[i] * 1.6),
                (-lens[i], 0.0),
            ],
            0.35,
            slot::ACCENT,
        )
        .translate(v3(0.0, radii[i] * 0.7, 0.0));
        r.attach(&format!("tail{}", i + 1), seg.with(spine));
    }

    let palettes = super::palettes::VIPER.to_vec();

    fn secondary(rig: &Rig, pose: &mut Pose, f: &Fighter, frame: u64) {
        let t = frame as f32;
        // Tail: travelling wave, dragged opposite to motion, whipping in the
        // air. Each segment lags the previous one.
        let speed = (f.vel.x * f.facing).clamp(-3.0, 3.0);
        let vy = f.vel.y.clamp(-5.0, 5.0);
        let airborne = !f.grounded;
        let amp = if airborne { 14.0 } else { 7.0 } + speed.abs() * 3.0;
        let base = if airborne { 18.0 } else { 6.0 };
        for i in 0..5 {
            let k = i as f32;
            let wave = (t * 0.22 - k * 0.9).sin() * amp * (0.5 + 0.2 * k);
            let drag = speed * 6.0 - vy * 3.0;
            let z = base + wave + drag;
            pose.rot(
                rig,
                &format!("tail{}", i + 1),
                (t * 0.11 - k * 0.7).sin() * 4.0,
                0.0,
                if i == 0 { z * 0.6 } else { z * 0.5 },
            );
        }
        // Hood flares when attacking or shielding.
        let flare = match f.state {
            State::Attack { .. } => 1.0,
            State::Shield | State::ShieldStun { .. } => 0.7,
            _ => 0.25,
        };
        pose.scale(rig, "hood", v3(1.0, 0.8 + 0.35 * flare, 0.6 + 0.6 * flare));
        pose.rot(rig, "hood", 0.0, 0.0, -10.0 * flare);
    }

    CharacterModel {
        rig: r,
        style: AnimStyle {
            bob: 0.5,
            run_lean: 24.0,
            swing: 1.15,
            crouch_depth: 8.0,
            walk_cycle: 26.0,
            run_cycle: 12.0,
            heavy: 0.0,
        },
        palettes,
        secondary,
        extras: Vec::new(),
        pieces: Vec::new(),
        spec_id: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::roster::PALETTES;

    #[test]
    fn every_character_builds_with_enough_palettes() {
        for id in CharacterId::ALL {
            let m = build(id);
            assert!(m.palettes.len() >= PALETTES as usize, "{:?} palettes", id);
            assert!(m.rig.triangle_count() > 300, "{:?} too simple", id);
            assert!(m.rig.triangle_count() < 20_000, "{:?} too heavy", id);
            // Standard bones present so shared animation applies.
            for b in ["head", "hand_r", "foot_l", "chest"] {
                assert!(m.rig.bone(b).is_some());
            }
        }
    }

    #[test]
    fn models_fit_their_collision_size() {
        // The rest-pose model should roughly match the sim's hurt capsule
        // (height / half-width) so hits look right.
        for id in CharacterId::ALL {
            let m = build(id);
            let ch = id.data();
            let w = m.rig.world(&m.rig.rest_pose(), &Xf::IDENTITY);
            let mut top = f32::MIN;
            let mut bottom = f32::MAX;
            for (bi, b) in m.rig.bones.iter().enumerate() {
                for p in &b.mesh.pos {
                    let y = w[bi].point(*p).y;
                    top = top.max(y);
                    bottom = bottom.min(y);
                }
            }
            let ratio = top / ch.height;
            assert!(
                ratio > 0.85 && ratio < 1.25,
                "{:?} height ratio {ratio}",
                id
            );
            assert!(bottom > -1.0, "{:?} sinks below the floor: {bottom}", id);
        }
    }
}
