//! Fighter models built from the art specifications in
//! `docs/art/procedural/*.json` (contract: `docs/art/procedural/FORMAT.md`).
//!
//! The JSON files are embedded into the executable and parsed when a model is
//! built, so the numbers the art reviewer edits are *exactly* the numbers the
//! game draws — no hand-copied tables that can drift. Unknown fields, unknown
//! bones, bad slots, non-convex plates and non-finite numbers are authoring
//! errors and fail loudly (a unit test parses every shipped spec).
//!
//! Conventions (from FORMAT.md): sim units; +X is the fighter's front, +Y up,
//! +Z toward the side camera; `_r` parts sit at +Z and `_l` at −Z. A piece is
//! placed by `T(position) · Rx · Ry · Rz` in its bone's frame, which is
//! exactly [`Xf::trs`] with unit scale. Root is at the feet.
//!
//! The *extras lag* (scarf, crest, tail…) is the closed algorithm of
//! FORMAT.md §"Retardo de extras": each extra bone rotates about its local Z
//! by `clamp(gain · twist(parent_now⁻¹ · parent_{now−delay}), ±limit)`. It is
//! purely visual — nothing in `sim` reads it — and the renderer keeps the
//! history per port ([`ExtrasState`]).

use super::lighting::{slot, Palette};
use super::math3::{v3, Xf, M3, V3};
use super::mesh::MeshData;
use super::rig::{Pose, Rig};
use serde::Deserialize;
use std::collections::BTreeMap;

pub const KESTREL_JSON: &str = include_str!("../../docs/art/procedural/kestrel.json");
pub const BOULDER_JSON: &str = include_str!("../../docs/art/procedural/boulder.json");
pub const VIPER_JSON: &str = include_str!("../../docs/art/procedural/viper.json");
pub const TRAMA_JSON: &str = include_str!("../../docs/art/procedural/trama.json");
pub const PALETTES_JSON: &str = include_str!("../../docs/art/procedural/palettes.json");
pub const LATTICE_JSON: &str = include_str!("../../docs/art/procedural/lattice.json");

/// Every shipped character spec, by id.
pub const ALL_SPECS: [(&str, &str); 4] = [
    ("kestrel", KESTREL_JSON),
    ("boulder", BOULDER_JSON),
    ("viper", VIPER_JSON),
    ("trama", TRAMA_JSON),
];

// ----------------------------------------------------------------- schema

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct Spec {
    pub schema_version: u32,
    pub id: String,
    pub units: String,
    pub reference: Reference,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub proportions: BTreeMap<String, f32>,
    pub bones: Vec<BoneSpec>,
    pub pieces: Vec<PieceSpec>,
    #[serde(default)]
    pub extras: Vec<ExtraSpec>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct Reference {
    pub height: f32,
    pub half_width: f32,
    pub hurt_radius: f32,
    #[serde(default)]
    pub status: String,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct BoneSpec {
    pub name: String,
    pub parent: Option<String>,
    pub offset: [f32; 3],
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct PieceSpec {
    pub id: String,
    pub bone: String,
    pub primitive: String,
    pub parameters: serde_json::Value,
    pub position: [f32; 3],
    pub rotation_deg: [f32; 3],
    pub slot: String,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct ExtraSpec {
    pub bone: String,
    pub parent: String,
    pub delay_ticks: u32,
    pub gain: f32,
    pub max_angle_deg: f32,
    pub axis: String,
    #[serde(default)]
    pub translation_lag: f32,
    #[serde(default)]
    pub oscillation_amplitude: f32,
}

/// Primitive parameters, each with its own strict field set.
#[derive(Debug, Clone, PartialEq)]
pub enum Primitive {
    Cuboid {
        size: [f32; 3],
    },
    BevelBox {
        size: [f32; 3],
        bevel: f32,
    },
    Ellipsoid {
        radii: [f32; 3],
        lat: u32,
        lon: u32,
    },
    Cylinder {
        r0: f32,
        r1: f32,
        height: f32,
        segments: u32,
    },
    Plate {
        points_xy: Vec<[f32; 2]>,
        thickness: f32,
    },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CuboidP {
    size: [f32; 3],
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BevelBoxP {
    size: [f32; 3],
    bevel: f32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EllipsoidP {
    radii: [f32; 3],
    lat: u32,
    lon: u32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CylinderP {
    r0: f32,
    r1: f32,
    height: f32,
    segments: u32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PlateP {
    points_xy: Vec<[f32; 2]>,
    thickness: f32,
}

impl PieceSpec {
    pub fn primitive(&self) -> Result<Primitive, String> {
        let p = self.parameters.clone();
        let e = |err: serde_json::Error| format!("piece {}: {err}", self.id);
        Ok(match self.primitive.as_str() {
            "cuboid" => {
                let c: CuboidP = serde_json::from_value(p).map_err(e)?;
                Primitive::Cuboid { size: c.size }
            }
            "bevel_box" => {
                let c: BevelBoxP = serde_json::from_value(p).map_err(e)?;
                Primitive::BevelBox {
                    size: c.size,
                    bevel: c.bevel,
                }
            }
            "ellipsoid" => {
                let c: EllipsoidP = serde_json::from_value(p).map_err(e)?;
                Primitive::Ellipsoid {
                    radii: c.radii,
                    lat: c.lat,
                    lon: c.lon,
                }
            }
            "cylinder" => {
                let c: CylinderP = serde_json::from_value(p).map_err(e)?;
                Primitive::Cylinder {
                    r0: c.r0,
                    r1: c.r1,
                    height: c.height,
                    segments: c.segments,
                }
            }
            "plate" => {
                let c: PlateP = serde_json::from_value(p).map_err(e)?;
                Primitive::Plate {
                    points_xy: c.points_xy,
                    thickness: c.thickness,
                }
            }
            other => return Err(format!("piece {}: unknown primitive `{other}`", self.id)),
        })
    }
}

/// Slot names in the order FORMAT.md fixes them.
pub const SLOT_NAMES: [&str; slot::COUNT] = [
    "primary",
    "secondary",
    "accent",
    "skin",
    "dark",
    "glow",
    "light",
    "extra",
];

pub fn slot_index(name: &str) -> Option<u8> {
    SLOT_NAMES.iter().position(|s| *s == name).map(|i| i as u8)
}

// ----------------------------------------------------------------- parsing

pub fn parse(json: &str) -> Result<Spec, String> {
    let spec: Spec = serde_json::from_str(json).map_err(|e| e.to_string())?;
    validate(&spec)?;
    Ok(spec)
}

fn finite(xs: &[f32], what: &str) -> Result<(), String> {
    if xs.iter().all(|x| x.is_finite()) {
        Ok(())
    } else {
        Err(format!("{what}: non-finite number"))
    }
}

/// Authoring rules from FORMAT.md that a parse alone cannot enforce.
pub fn validate(spec: &Spec) -> Result<(), String> {
    if spec.schema_version != 1 {
        return Err(format!(
            "unsupported schema_version {}",
            spec.schema_version
        ));
    }
    if spec.units != "game_units" {
        return Err(format!("units must be game_units, got `{}`", spec.units));
    }
    finite(
        &[
            spec.reference.height,
            spec.reference.half_width,
            spec.reference.hurt_radius,
        ],
        "reference",
    )?;
    // Bones: unique names, parent declared earlier, exactly one root.
    let mut names: Vec<&str> = Vec::new();
    for b in &spec.bones {
        if names.contains(&b.name.as_str()) {
            return Err(format!("duplicate bone `{}`", b.name));
        }
        match &b.parent {
            None => {
                if !names.is_empty() {
                    return Err(format!(
                        "bone `{}`: only the first bone may be root",
                        b.name
                    ));
                }
            }
            Some(p) => {
                if !names.contains(&p.as_str()) {
                    return Err(format!(
                        "bone `{}`: parent `{p}` must be declared before it",
                        b.name
                    ));
                }
            }
        }
        finite(&b.offset, &format!("bone `{}` offset", b.name))?;
        names.push(&b.name);
    }
    if spec.bones.is_empty() {
        return Err("no bones".into());
    }
    // Pieces.
    let mut ids: Vec<&str> = Vec::new();
    for p in &spec.pieces {
        if ids.contains(&p.id.as_str()) {
            return Err(format!("duplicate piece id `{}`", p.id));
        }
        ids.push(&p.id);
        if !names.contains(&p.bone.as_str()) {
            return Err(format!("piece `{}`: unknown bone `{}`", p.id, p.bone));
        }
        if slot_index(&p.slot).is_none() {
            return Err(format!("piece `{}`: unknown slot `{}`", p.id, p.slot));
        }
        finite(&p.position, &format!("piece `{}` position", p.id))?;
        finite(&p.rotation_deg, &format!("piece `{}` rotation", p.id))?;
        let prim = p.primitive()?;
        match &prim {
            Primitive::Cuboid { size } => {
                finite(size, &p.id)?;
                if size.iter().any(|s| *s <= 0.0) {
                    return Err(format!("piece `{}`: size must be positive", p.id));
                }
            }
            Primitive::BevelBox { size, bevel } => {
                finite(size, &p.id)?;
                finite(&[*bevel], &p.id)?;
                if size.iter().any(|s| *s <= 0.0) || *bevel < 0.0 {
                    return Err(format!("piece `{}`: bad bevel_box", p.id));
                }
            }
            Primitive::Ellipsoid { radii, lat, lon } => {
                finite(radii, &p.id)?;
                if radii.iter().any(|s| *s <= 0.0) || *lat < 2 || *lon < 3 {
                    return Err(format!("piece `{}`: bad ellipsoid", p.id));
                }
            }
            Primitive::Cylinder {
                r0,
                r1,
                height,
                segments,
            } => {
                finite(&[*r0, *r1, *height], &p.id)?;
                if *r0 < 0.0 || *r1 < 0.0 || *height <= 0.0 || *segments < 3 {
                    return Err(format!("piece `{}`: bad cylinder", p.id));
                }
            }
            Primitive::Plate {
                points_xy,
                thickness,
            } => {
                finite(&[*thickness], &p.id)?;
                for pt in points_xy {
                    finite(pt, &p.id)?;
                }
                if *thickness <= 0.0 || points_xy.len() < 3 {
                    return Err(format!("piece `{}`: bad plate", p.id));
                }
                if !convex_ccw(points_xy) {
                    return Err(format!(
                        "piece `{}`: plate outline must be convex and counter-clockwise",
                        p.id
                    ));
                }
            }
        }
    }
    // Extras: bone and parent exist, parent is the bone's declared parent,
    // only the Z axis is defined, and no dt-dependent features.
    for e in &spec.extras {
        let Some(b) = spec.bones.iter().find(|b| b.name == e.bone) else {
            return Err(format!("extra `{}`: unknown bone", e.bone));
        };
        if b.parent.as_deref() != Some(e.parent.as_str()) {
            return Err(format!(
                "extra `{}`: parent `{}` does not match the bone's parent {:?}",
                e.bone, e.parent, b.parent
            ));
        }
        if e.axis != "z" {
            return Err(format!("extra `{}`: only axis z is supported", e.bone));
        }
        finite(&[e.gain, e.max_angle_deg], &format!("extra `{}`", e.bone))?;
        if e.gain < 0.0 || e.max_angle_deg < 0.0 || e.delay_ticks > 31 {
            return Err(format!("extra `{}`: out-of-range lag values", e.bone));
        }
        if e.translation_lag != 0.0 || e.oscillation_amplitude != 0.0 {
            return Err(format!(
                "extra `{}`: translation lag / oscillation are not part of v1",
                e.bone
            ));
        }
    }
    Ok(())
}

/// Strictly convex, counter-clockwise, positive area.
pub fn convex_ccw(pts: &[[f32; 2]]) -> bool {
    let n = pts.len();
    if n < 3 {
        return false;
    }
    let mut area = 0.0f32;
    for i in 0..n {
        let a = pts[i];
        let b = pts[(i + 1) % n];
        area += a[0] * b[1] - b[0] * a[1];
    }
    if area <= 1e-6 {
        return false;
    }
    for i in 0..n {
        let a = pts[i];
        let b = pts[(i + 1) % n];
        let c = pts[(i + 2) % n];
        let cross = (b[0] - a[0]) * (c[1] - b[1]) - (b[1] - a[1]) * (c[0] - b[0]);
        if cross < -1e-6 {
            return false;
        }
    }
    true
}

// ----------------------------------------------------------------- building

/// Where a piece's vertices ended up (diagnostics / overlays).
#[derive(Debug, Clone)]
pub struct PieceRange {
    pub id: String,
    pub bone: usize,
    /// Vertex range inside the bone's mesh.
    pub vertices: (u32, u32),
    pub triangles: usize,
}

/// One extra's lag parameters, resolved to bone indices.
#[derive(Debug, Clone, Copy)]
pub struct ExtraLag {
    pub bone: usize,
    pub parent: usize,
    pub delay: usize,
    pub gain: f32,
    pub limit_deg: f32,
}

/// A rig built from a spec plus everything a reviewer may want to trace back.
#[derive(Debug, Clone)]
pub struct Built {
    pub id: String,
    pub rig: Rig,
    pub pieces: Vec<PieceRange>,
    pub extras: Vec<ExtraLag>,
    pub height: f32,
    pub half_width: f32,
}

/// Mesh for one primitive in its own frame (centred, flat-shaded, no
/// degenerate triangles).
pub fn primitive_mesh(prim: &Primitive, slot: u8) -> MeshData {
    let m = match prim {
        Primitive::Cuboid { size } => MeshData::cuboid(size[0], size[1], size[2], slot),
        Primitive::BevelBox { size, bevel } => {
            MeshData::bevel_box(size[0], size[1], size[2], *bevel, slot)
        }
        Primitive::Ellipsoid { radii, lat, lon } => {
            MeshData::ellipsoid(radii[0], radii[1], radii[2], *lat, *lon, slot)
        }
        Primitive::Cylinder {
            r0,
            r1,
            height,
            segments,
        } => MeshData::cylinder(*r0, *r1, *height, *segments, slot),
        Primitive::Plate {
            points_xy,
            thickness,
        } => {
            let pts: Vec<(f32, f32)> = points_xy.iter().map(|p| (p[0], p[1])).collect();
            MeshData::plate(&pts, *thickness, slot)
        }
    };
    m.drop_degenerate().flat()
}

pub fn build(spec: &Spec) -> Result<Built, String> {
    validate(spec)?;
    let mut rig = Rig::new();
    for b in &spec.bones {
        rig.add(
            &b.name,
            b.parent.as_deref().unwrap_or(""),
            v3(b.offset[0], b.offset[1], b.offset[2]),
        );
    }
    let mut pieces = Vec::with_capacity(spec.pieces.len());
    for p in &spec.pieces {
        let prim = p.primitive()?;
        let s = slot_index(&p.slot).ok_or_else(|| format!("piece `{}`: bad slot", p.id))?;
        let xf = Xf::trs(
            v3(p.position[0], p.position[1], p.position[2]),
            v3(p.rotation_deg[0], p.rotation_deg[1], p.rotation_deg[2]),
            V3::ONE,
        );
        let mesh = primitive_mesh(&prim, s).transform(&xf);
        let bi = rig
            .bone(&p.bone)
            .ok_or_else(|| format!("piece `{}`: bad bone", p.id))?;
        let start = rig.bones[bi].mesh.vertex_count() as u32;
        let tris = mesh.triangle_count();
        rig.attach(&p.bone, mesh);
        let end = rig.bones[bi].mesh.vertex_count() as u32;
        pieces.push(PieceRange {
            id: p.id.clone(),
            bone: bi,
            vertices: (start, end),
            triangles: tris,
        });
    }
    let mut extras = Vec::with_capacity(spec.extras.len());
    for e in &spec.extras {
        let bone = rig.bone(&e.bone).ok_or("extra bone")?;
        let parent = rig.bone(&e.parent).ok_or("extra parent")?;
        extras.push(ExtraLag {
            bone,
            parent,
            delay: e.delay_ticks as usize,
            gain: e.gain,
            limit_deg: e.max_angle_deg,
        });
    }
    Ok(Built {
        id: spec.id.clone(),
        rig,
        pieces,
        extras,
        height: spec.reference.height,
        half_width: spec.reference.half_width,
    })
}

/// Rest-pose bounds of the whole model in root space.
pub fn rest_bounds(rig: &Rig) -> (V3, V3) {
    let w = rig.world(&rig.rest_pose(), &Xf::IDENTITY);
    let mut lo = v3(f32::MAX, f32::MAX, f32::MAX);
    let mut hi = v3(f32::MIN, f32::MIN, f32::MIN);
    for (bi, b) in rig.bones.iter().enumerate() {
        for p in &b.mesh.pos {
            let q = w[bi].point(*p);
            lo = v3(lo.x.min(q.x), lo.y.min(q.y), lo.z.min(q.z));
            hi = v3(hi.x.max(q.x), hi.y.max(q.y), hi.z.max(q.z));
        }
    }
    (lo, hi)
}

// ----------------------------------------------------------------- palettes

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PalettesFile {
    schema_version: u32,
    #[allow(dead_code)]
    encoding: String,
    slot_order: Vec<String>,
    palettes_per_character: usize,
    characters: BTreeMap<String, Vec<PaletteEntry>>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PaletteEntry {
    index: usize,
    name: String,
    slots: BTreeMap<String, SlotColor>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SlotColor {
    #[allow(dead_code)]
    hex: String,
    rgb: [u8; 3],
}

/// The six palettes of `character` from `palettes.json`, in index order.
/// Palette names are leaked once (they live as long as the program).
pub fn palettes(character: &str) -> Result<Vec<Palette>, String> {
    let f: PalettesFile = serde_json::from_str(PALETTES_JSON).map_err(|e| e.to_string())?;
    if f.schema_version != 1 {
        return Err("palettes: unsupported schema".into());
    }
    if f.slot_order != SLOT_NAMES {
        return Err("palettes: slot order differs from FORMAT.md".into());
    }
    let list = f
        .characters
        .get(character)
        .ok_or_else(|| format!("palettes: no character `{character}`"))?;
    if list.len() != f.palettes_per_character {
        return Err(format!(
            "palettes: `{character}` has {} palettes, expected {}",
            list.len(),
            f.palettes_per_character
        ));
    }
    let mut out = Vec::with_capacity(list.len());
    for (i, e) in list.iter().enumerate() {
        if e.index != i {
            return Err(format!(
                "palettes: `{character}` index {} out of order",
                e.index
            ));
        }
        let mut colors = [[0u8; 3]; slot::COUNT];
        for (si, name) in SLOT_NAMES.iter().enumerate() {
            let c = e
                .slots
                .get(*name)
                .ok_or_else(|| format!("palettes: `{character}` #{i} lacks slot {name}"))?;
            colors[si] = c.rgb;
        }
        let name: &'static str = Box::leak(e.name.clone().into_boxed_str());
        out.push(Palette { colors, name });
    }
    Ok(out)
}

/// Which characters `palettes.json` covers (used by tests / exports).
pub fn palette_characters() -> Vec<String> {
    serde_json::from_str::<PalettesFile>(PALETTES_JSON)
        .map(|f| f.characters.keys().cloned().collect())
        .unwrap_or_default()
}

// ----------------------------------------------------------------- extras lag

/// Unit quaternion `(w, x, y, z)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quat {
    pub w: f32,
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Quat {
    pub const IDENTITY: Quat = Quat {
        w: 1.0,
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };

    /// From a rotation matrix (columns orthonormalised first, so bone scale
    /// or numeric drift cannot leak in).
    pub fn from_m3(m: &M3) -> Quat {
        let c0 = v3(m.m[0][0], m.m[1][0], m.m[2][0]).norm();
        let c1 = v3(m.m[0][1], m.m[1][1], m.m[2][1]);
        let c2 = c0.cross(c1).norm();
        let c1 = c2.cross(c0).norm();
        let r = [[c0.x, c1.x, c2.x], [c0.y, c1.y, c2.y], [c0.z, c1.z, c2.z]];
        let tr = r[0][0] + r[1][1] + r[2][2];
        let q = if tr > 0.0 {
            let s = (tr + 1.0).sqrt() * 2.0;
            Quat {
                w: 0.25 * s,
                x: (r[2][1] - r[1][2]) / s,
                y: (r[0][2] - r[2][0]) / s,
                z: (r[1][0] - r[0][1]) / s,
            }
        } else if r[0][0] > r[1][1] && r[0][0] > r[2][2] {
            let s = (1.0 + r[0][0] - r[1][1] - r[2][2]).sqrt() * 2.0;
            Quat {
                w: (r[2][1] - r[1][2]) / s,
                x: 0.25 * s,
                y: (r[0][1] + r[1][0]) / s,
                z: (r[0][2] + r[2][0]) / s,
            }
        } else if r[1][1] > r[2][2] {
            let s = (1.0 + r[1][1] - r[0][0] - r[2][2]).sqrt() * 2.0;
            Quat {
                w: (r[0][2] - r[2][0]) / s,
                x: (r[0][1] + r[1][0]) / s,
                y: 0.25 * s,
                z: (r[1][2] + r[2][1]) / s,
            }
        } else {
            let s = (1.0 + r[2][2] - r[0][0] - r[1][1]).sqrt() * 2.0;
            Quat {
                w: (r[1][0] - r[0][1]) / s,
                x: (r[0][2] + r[2][0]) / s,
                y: (r[1][2] + r[2][1]) / s,
                z: 0.25 * s,
            }
        };
        q.normalized()
    }

    pub fn normalized(self) -> Quat {
        let l = (self.w * self.w + self.x * self.x + self.y * self.y + self.z * self.z).sqrt();
        if l < 1e-8 {
            return Quat::IDENTITY;
        }
        Quat {
            w: self.w / l,
            x: self.x / l,
            y: self.y / l,
            z: self.z / l,
        }
    }

    pub fn inverse(self) -> Quat {
        Quat {
            w: self.w,
            x: -self.x,
            y: -self.y,
            z: -self.z,
        }
    }

    /// Hamilton product `self · o` (apply `o` first, then `self`).
    pub fn then(self, o: Quat) -> Quat {
        Quat {
            w: self.w * o.w - self.x * o.x - self.y * o.y - self.z * o.z,
            x: self.w * o.x + self.x * o.w + self.y * o.z - self.z * o.y,
            y: self.w * o.y - self.x * o.z + self.y * o.w + self.z * o.x,
            z: self.w * o.z + self.x * o.y - self.y * o.x + self.z * o.w,
        }
    }

    /// Signed twist about the local Z axis, degrees in `[-180, 180]`
    /// (swing–twist decomposition).
    pub fn twist_z_deg(self) -> f32 {
        let mut a = 2.0 * self.z.atan2(self.w).to_degrees();
        while a > 180.0 {
            a -= 360.0;
        }
        while a < -180.0 {
            a += 360.0;
        }
        a
    }
}

/// Maximum history kept per extra (FORMAT.md: 32 samples).
pub const LAG_HISTORY: usize = 32;

/// Renderer-side lag history for one fighter's extras. Purely visual: it
/// is rebuilt from scratch whenever the frame sequence breaks (rollback,
/// seek, replay load, character change) or the fighter turns around.
#[derive(Debug, Clone, Default)]
pub struct ExtrasState {
    last_frame: Option<u64>,
    facing: f32,
    /// Per extra: parent orientations, newest last.
    hist: Vec<Vec<Quat>>,
    /// Per extra: the angle currently applied (frozen during hitlag).
    pub theta: Vec<f32>,
}

impl ExtrasState {
    fn reset(&mut self, n: usize) {
        self.hist = vec![Vec::with_capacity(LAG_HISTORY); n];
        self.theta = vec![0.0; n];
    }

    /// Apply the lag layer for sim tick `frame`. `pose` holds the combat
    /// pose (extras still at rest); on return the extra bones carry their
    /// `Rz(theta)`. `frozen` = the fighter is in hitlag (clock and angles
    /// hold). Returns nothing the simulation could read.
    #[allow(clippy::too_many_arguments)]
    pub fn apply(
        &mut self,
        extras: &[ExtraLag],
        rig: &Rig,
        pose: &mut Pose,
        root: &Xf,
        frame: u64,
        frozen: bool,
        facing: f32,
    ) {
        if extras.is_empty() {
            return;
        }
        let broken = match self.last_frame {
            None => true,
            Some(last) => frame < last || frame > last + LAG_HISTORY as u64,
        };
        if broken || self.hist.len() != extras.len() || facing != self.facing {
            self.reset(extras.len());
            self.facing = facing;
            self.last_frame = None;
        }
        let advance = match self.last_frame {
            None => true,
            Some(last) => frame > last && !frozen,
        };
        for (ei, e) in extras.iter().enumerate() {
            // Parent orientation with every *earlier* extra's lag applied but
            // not this one's (chains evaluate parents before children).
            let world = rig.world(pose, root);
            let q_now = Quat::from_m3(&world[e.parent].m);
            let h = &mut self.hist[ei];
            if h.is_empty() {
                // Fresh history: fill with the current orientation → theta 0.
                for _ in 0..=e.delay.min(LAG_HISTORY - 1) {
                    h.push(q_now);
                }
            } else if advance {
                h.push(q_now);
                if h.len() > LAG_HISTORY {
                    h.remove(0);
                }
            }
            if advance {
                let n = h.len();
                let old = h[n.saturating_sub(1 + e.delay)];
                let delta = q_now.inverse().then(old);
                let twist = delta.twist_z_deg();
                self.theta[ei] = (e.gain * twist).clamp(-e.limit_deg, e.limit_deg);
            }
            let cur = pose.rot[e.bone];
            pose.rot[e.bone] = v3(cur.x, cur.y, self.theta[ei]);
        }
        if advance || self.last_frame.is_none() {
            self.last_frame = Some(frame);
        }
    }
}

// ----------------------------------------------------------------- stage spec

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct LatticeSpec {
    pub schema_version: u32,
    pub id: String,
    pub units: String,
    #[serde(default)]
    pub coordinates: String,
    pub background_srgb: String,
    pub colliders_reference: Vec<ColliderRef>,
    pub pieces: Vec<StagePiece>,
    #[serde(default)]
    pub animation: String,
    #[serde(default)]
    pub textures: String,
    #[serde(default)]
    pub particles: String,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct ColliderRef {
    pub id: String,
    pub x_min: f32,
    pub x_max: f32,
    pub y: f32,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct StagePiece {
    pub id: String,
    pub primitive: String,
    pub size: [f32; 3],
    #[serde(default)]
    pub bevel: f32,
    pub position: [f32; 3],
    pub rotation_deg: [f32; 3],
    pub color: String,
    pub layer: String,
    #[serde(default)]
    pub collision: bool,
}

pub fn parse_lattice(json: &str) -> Result<LatticeSpec, String> {
    let s: LatticeSpec = serde_json::from_str(json).map_err(|e| e.to_string())?;
    for p in &s.pieces {
        if p.collision {
            return Err(format!("stage piece `{}` must not collide", p.id));
        }
        if !matches!(p.primitive.as_str(), "cuboid" | "bevel_box") {
            return Err(format!("stage piece `{}`: unknown primitive", p.id));
        }
        hex_rgb(&p.color).ok_or_else(|| format!("stage piece `{}`: bad colour", p.id))?;
    }
    Ok(s)
}

/// `#RRGGBB` → RGB8.
pub fn hex_rgb(s: &str) -> Option<[u8; 3]> {
    let h = s.strip_prefix('#')?;
    if h.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(h, 16).ok()?;
    Some([(v >> 16) as u8, (v >> 8) as u8, v as u8])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn built(json: &str) -> Built {
        build(&parse(json).expect("parse")).expect("build")
    }

    #[test]
    fn every_shipped_spec_parses_builds_and_fits_its_capsule() {
        for (id, json) in ALL_SPECS {
            let spec = parse(json).unwrap_or_else(|e| panic!("{id}: {e}"));
            assert_eq!(spec.id, id);
            let b = build(&spec).unwrap_or_else(|e| panic!("{id}: {e}"));
            assert_eq!(b.pieces.len(), spec.pieces.len());
            assert_eq!(b.rig.len(), spec.bones.len());
            let (lo, hi) = rest_bounds(&b.rig);
            // FORMAT.md: nothing below the floor, nothing above `height`,
            // nothing outside ±half_width in X, feet just above y=0.
            assert!(lo.y >= 0.0, "{id}: below floor {lo:?}");
            assert!(lo.y <= 0.3, "{id}: feet float {lo:?}");
            assert!(
                hi.y <= spec.reference.height + 1e-3,
                "{id}: too tall {hi:?}"
            );
            assert!(
                hi.y >= spec.reference.height * 0.9,
                "{id}: too short {hi:?}"
            );
            assert!(lo.x >= -spec.reference.half_width - 1e-3, "{id}: {lo:?}");
            assert!(hi.x <= spec.reference.half_width + 1e-3, "{id}: {hi:?}");
            // Standard bones so the shared animation applies.
            for n in [
                "hips",
                "spine",
                "chest",
                "neck",
                "head",
                "upper_arm_r",
                "forearm_r",
                "hand_r",
                "thigh_r",
                "shin_r",
                "foot_r",
                "upper_arm_l",
                "forearm_l",
                "hand_l",
                "thigh_l",
                "shin_l",
                "foot_l",
            ] {
                assert!(b.rig.bone(n).is_some(), "{id}: missing bone {n}");
            }
            // Normals finite and unit; no degenerate triangles.
            for bone in &b.rig.bones {
                for n in &bone.mesh.nrm {
                    assert!(n.x.is_finite() && n.y.is_finite() && n.z.is_finite());
                    assert!((n.len() - 1.0).abs() < 1e-3, "{id}: normal {n:?}");
                }
                for t in bone.mesh.idx.chunks(3) {
                    let (a, bb, c) = (
                        bone.mesh.pos[t[0] as usize],
                        bone.mesh.pos[t[1] as usize],
                        bone.mesh.pos[t[2] as usize],
                    );
                    assert!((bb - a).cross(c - a).len() > 1e-6, "{id}: degenerate tri");
                }
            }
            let tris = b.rig.triangle_count();
            assert!(tris < 3000, "{id}: {tris} triangles over budget");
            assert!(tris > 200, "{id}: {tris} triangles, suspiciously few");
        }
    }

    #[test]
    fn spec_matches_the_readable_table_counts() {
        // 31 / 31 / 31 / 38 pieces, 21 / 21 / 22 / 23 bones per the package.
        let counts: Vec<(usize, usize)> = ALL_SPECS
            .iter()
            .map(|(_, j)| {
                let s = parse(j).unwrap();
                (s.pieces.len(), s.bones.len())
            })
            .collect();
        assert_eq!(counts, vec![(31, 21), (31, 21), (31, 22), (38, 23)]);
    }

    #[test]
    fn kestrel_pieces_and_extras_are_traceable() {
        let b = built(KESTREL_JSON);
        let crest = b.pieces.iter().find(|p| p.id == "k_crest_2").unwrap();
        assert_eq!(b.rig.bones[crest.bone].name, "crest");
        assert!(crest.vertices.1 > crest.vertices.0);
        assert_eq!(b.extras.len(), 3);
        assert_eq!(b.rig.bones[b.extras[2].bone].name, "scarf_b");
        assert_eq!(b.rig.bones[b.extras[2].parent].name, "scarf_a");
        assert_eq!(b.extras[2].delay, 5);
        // Hips at 13.7 (bone table) and the scarf trailing toward -X.
        let w = b.rig.world(&b.rig.rest_pose(), &Xf::IDENTITY);
        let hips = b.rig.joint(&w, "hips").unwrap();
        assert!((hips.y - 13.7).abs() < 1e-4);
        let sb = b.rig.joint(&w, "scarf_b").unwrap();
        assert!(sb.x < -4.0, "scarf_b at {sb:?}");
    }

    #[test]
    fn rejects_authoring_errors() {
        let ok = parse(KESTREL_JSON).unwrap();
        // Unknown field.
        let bad = KESTREL_JSON.replacen("\"slot\"", "\"colour\"", 1);
        assert!(parse(&bad).is_err());
        // Unknown slot.
        let mut s = ok.clone();
        s.pieces[0].slot = "seventh".into();
        assert!(validate(&s).is_err());
        // Child before parent.
        let mut s = ok.clone();
        s.bones.swap(1, 2);
        assert!(validate(&s).is_err());
        // Concave plate.
        let mut s = ok.clone();
        let idx = s
            .pieces
            .iter()
            .position(|p| p.primitive == "plate")
            .unwrap();
        s.pieces[idx].parameters = serde_json::json!({
            "points_xy": [[0,0],[2,0],[1,0.2],[2,2],[0,2]],
            "thickness": 1.0
        });
        assert!(validate(&s).is_err());
        // Clockwise plate.
        let mut s = ok.clone();
        s.pieces[idx].parameters = serde_json::json!({
            "points_xy": [[0,0],[0,2],[2,2],[2,0]],
            "thickness": 1.0
        });
        assert!(validate(&s).is_err());
        // Extra on a non-Z axis.
        let mut s = ok;
        s.extras[0].axis = "x".into();
        assert!(validate(&s).is_err());
    }

    #[test]
    fn palettes_json_has_six_per_character_in_slot_order() {
        for id in ["kestrel", "boulder", "viper", "trama"] {
            let p = palettes(id).unwrap_or_else(|e| panic!("{id}: {e}"));
            assert_eq!(p.len(), crate::sim::roster::PALETTES as usize);
            // PALETAS.md, first row: Kestrel 0 "Coral" primary #F16F54.
            if id == "kestrel" {
                assert_eq!(p[0].name, "Coral");
                assert_eq!(p[0].color(slot::PRIMARY), [241, 111, 84]);
                assert_eq!(p[3].name, "Abisal");
            }
        }
        assert!(palettes("nobody").is_err());
    }

    #[test]
    fn lattice_spec_is_decorative_only_and_matches_colliders() {
        let l = parse_lattice(LATTICE_JSON).unwrap();
        assert_eq!(l.pieces.len(), 41);
        assert!(l.pieces.iter().all(|p| !p.collision));
        let stage = crate::sim::Stage::by_id(crate::sim::stage::StageId::Lattice);
        // Every referenced collider is a real platform of the sim stage.
        for c in &l.colliders_reference {
            assert!(
                stage
                    .platforms
                    .iter()
                    .any(|p| (p.left - c.x_min).abs() < 1e-3
                        && (p.right - c.x_max).abs() < 1e-3
                        && (p.y - c.y).abs() < 1e-3),
                "collider {} not in the sim stage",
                c.id
            );
        }
    }

    #[test]
    fn extras_lag_follows_the_closed_algorithm() {
        let b = built(KESTREL_JSON);
        let mut st = ExtrasState::default();
        let root = Xf::IDENTITY;
        let neck = b.rig.bone("neck").unwrap();
        let scarf_a = b.extras[1];
        assert_eq!(b.rig.bones[scarf_a.bone].name, "scarf_a");
        // Frame 0..10 at rest: theta stays 0.
        for f in 0..10u64 {
            let mut pose = b.rig.rest_pose();
            st.apply(&b.extras, &b.rig, &mut pose, &root, f, false, 1.0);
            assert_eq!(pose.rot[scarf_a.bone].z, 0.0);
        }
        // Snap the neck 30° about Z: after the delay the scarf lags by
        // clamp(gain·(old − now)) = clamp(0.55·(−30)) → −9 (limit).
        let mut last = 0.0;
        for f in 10..13u64 {
            let mut pose = b.rig.rest_pose();
            pose.rot[neck] = v3(0.0, 0.0, 30.0);
            st.apply(&b.extras, &b.rig, &mut pose, &root, f, false, 1.0);
            last = pose.rot[scarf_a.bone].z;
            assert!(last <= 0.0, "scarf should trail: {last}");
        }
        assert!((last + 9.0).abs() < 1e-3, "clamped to the limit: {last}");
        // Hold the neck: delay 3 later the history catches up → 0 again.
        for f in 13..30u64 {
            let mut pose = b.rig.rest_pose();
            pose.rot[neck] = v3(0.0, 0.0, 30.0);
            st.apply(&b.extras, &b.rig, &mut pose, &root, f, false, 1.0);
            last = pose.rot[scarf_a.bone].z;
        }
        assert!(last.abs() < 1e-3, "settled: {last}");
        // Hitlag freezes both clock and angle; turning around resets.
        let mut pose = b.rig.rest_pose();
        pose.rot[neck] = v3(0.0, 0.0, -40.0);
        st.apply(&b.extras, &b.rig, &mut pose, &root, 30, true, 1.0);
        assert!(pose.rot[scarf_a.bone].z.abs() < 1e-3, "frozen");
        let mut pose = b.rig.rest_pose();
        pose.rot[neck] = v3(0.0, 0.0, -40.0);
        st.apply(&b.extras, &b.rig, &mut pose, &root, 31, false, -1.0);
        assert_eq!(pose.rot[scarf_a.bone].z, 0.0, "facing change resets");
        // Rollback (frame goes backwards) resets rather than inventing a past.
        let mut pose = b.rig.rest_pose();
        st.apply(&b.extras, &b.rig, &mut pose, &root, 5, false, -1.0);
        assert_eq!(pose.rot[scarf_a.bone].z, 0.0);
    }

    #[test]
    fn quaternion_twist_is_signed_and_wrapped() {
        let q = Quat::from_m3(&M3::rot_z(30.0));
        assert!((q.twist_z_deg() - 30.0).abs() < 1e-3);
        let q = Quat::from_m3(&M3::rot_z(-170.0));
        assert!((q.twist_z_deg() + 170.0).abs() < 1e-3);
        // A pure X rotation has no Z twist.
        let q = Quat::from_m3(&M3::rot_x(80.0));
        assert!(q.twist_z_deg().abs() < 1e-3);
        // Round trip through inverse·mul.
        let a = Quat::from_m3(&M3::rot_z(10.0));
        let b = Quat::from_m3(&M3::rot_z(25.0));
        assert!((a.inverse().then(b).twist_z_deg() - 15.0).abs() < 1e-3);
    }
}
