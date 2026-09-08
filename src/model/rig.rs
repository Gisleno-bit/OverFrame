//! Segmented rigs: a bone hierarchy where every bone owns a rigid mesh.
//!
//! No skinning — each body part is a separate piece of geometry parented to a
//! bone, the way early-3D fighters were built. It is cheap (no per-vertex
//! weights), rollback-friendly (the renderer only reads the sim state) and it
//! maps 1:1 onto a Blender scene of parented objects, which is what the glTF
//! importer expects (see `docs/ART_PIPELINE.md`).
//!
//! A [`Pose`] is a per-bone rotation (Euler, degrees), translation offset and
//! scale; [`Rig::world`] composes it into world transforms and
//! [`Rig::skin`] emits lit, coloured vertices ready for the GPU.

use super::lighting::{shade, Light, Palette, Tint};
use super::math3::{v3, Xf, V3};
use super::mesh::MeshData;

#[derive(Clone, Debug)]
pub struct Bone {
    pub name: String,
    pub parent: Option<usize>,
    /// Rest position relative to the parent bone.
    pub offset: V3,
    /// Geometry in bone-local space (origin at the joint).
    pub mesh: MeshData,
}

#[derive(Clone, Debug, Default)]
pub struct Rig {
    pub bones: Vec<Bone>,
}

/// One vertex ready for upload: world position, final colour, uv.
#[derive(Clone, Copy, Debug)]
pub struct Vert {
    pub pos: V3,
    pub nrm: V3,
    pub rgba: [u8; 4],
    pub uv: [f32; 2],
}

impl Rig {
    pub fn new() -> Self {
        Rig { bones: Vec::new() }
    }

    /// Add a bone; returns its index. `parent` is a bone name ("" for root).
    pub fn add(&mut self, name: &str, parent: &str, offset: V3) -> usize {
        let parent = if parent.is_empty() {
            None
        } else {
            Some(
                self.bone(parent)
                    .unwrap_or_else(|| panic!("unknown parent bone {parent}")),
            )
        };
        self.bones.push(Bone {
            name: name.to_string(),
            parent,
            offset,
            mesh: MeshData::new(),
        });
        self.bones.len() - 1
    }

    pub fn bone(&self, name: &str) -> Option<usize> {
        self.bones.iter().position(|b| b.name == name)
    }

    /// Attach (append) geometry to a named bone.
    pub fn attach(&mut self, name: &str, mesh: MeshData) {
        let i = self
            .bone(name)
            .unwrap_or_else(|| panic!("unknown bone {name}"));
        self.bones[i].mesh.append(&mesh);
    }

    pub fn len(&self) -> usize {
        self.bones.len()
    }
    pub fn is_empty(&self) -> bool {
        self.bones.is_empty()
    }

    pub fn triangle_count(&self) -> usize {
        self.bones.iter().map(|b| b.mesh.triangle_count()).sum()
    }

    /// Rest pose (identity for every bone).
    pub fn rest_pose(&self) -> Pose {
        Pose::rest(self.bones.len())
    }

    /// World transform of every bone for `pose`, under `root`.
    pub fn world(&self, pose: &Pose, root: &Xf) -> Vec<Xf> {
        let mut out: Vec<Xf> = Vec::with_capacity(self.bones.len());
        for (i, b) in self.bones.iter().enumerate() {
            let local = Xf::trs(b.offset + pose.off[i], pose.rot[i], pose.scl[i]);
            let parent = match b.parent {
                Some(p) => out[p],
                None => *root,
            };
            out.push(parent * local);
        }
        out
    }

    /// Emit lit vertices + indices for the whole rig in `pose`.
    pub fn skin(
        &self,
        world: &[Xf],
        palette: &Palette,
        light: &Light,
        tint: &Tint,
        out_v: &mut Vec<Vert>,
        out_i: &mut Vec<u32>,
    ) {
        for (bi, b) in self.bones.iter().enumerate() {
            let xf = &world[bi];
            let base = out_v.len() as u32;
            let mirrored = xf.mirrored();
            for (vi, p) in b.mesh.pos.iter().enumerate() {
                let wp = xf.point(*p);
                let wn = xf.normal(b.mesh.nrm[vi]);
                let albedo = palette.color(b.mesh.slot[vi]);
                let rgba = shade(albedo, wn, wp, light, tint, b.mesh.slot[vi]);
                out_v.push(Vert {
                    pos: wp,
                    nrm: wn,
                    rgba,
                    uv: b.mesh.uv[vi],
                });
            }
            if mirrored {
                for t in b.mesh.idx.chunks(3) {
                    out_i.push(base + t[0]);
                    out_i.push(base + t[2]);
                    out_i.push(base + t[1]);
                }
            } else {
                out_i.extend(b.mesh.idx.iter().map(|i| base + i));
            }
        }
    }

    /// World-space position of a bone's joint.
    pub fn joint(&self, world: &[Xf], name: &str) -> Option<V3> {
        self.bone(name).map(|i| world[i].t)
    }
}

/// Per-bone animation values. Rotations are Euler degrees applied Z→Y→X in
/// the bone's local frame (see [`super::math3::M3::euler`]).
#[derive(Clone, Debug, PartialEq)]
pub struct Pose {
    pub rot: Vec<V3>,
    pub off: Vec<V3>,
    pub scl: Vec<V3>,
}

impl Pose {
    pub fn rest(n: usize) -> Pose {
        Pose {
            rot: vec![V3::ZERO; n],
            off: vec![V3::ZERO; n],
            scl: vec![V3::ONE; n],
        }
    }

    pub fn len(&self) -> usize {
        self.rot.len()
    }
    pub fn is_empty(&self) -> bool {
        self.rot.is_empty()
    }

    /// Set a bone's rotation by name (ignored if the rig lacks the bone, so
    /// animation code written for the standard skeleton works on partial rigs).
    pub fn set(&mut self, rig: &Rig, name: &str, rot: V3) {
        if let Some(i) = rig.bone(name) {
            self.rot[i] = rot;
        }
    }
    /// Set a bone's rotation from (x, y, z) degrees.
    pub fn rot(&mut self, rig: &Rig, name: &str, x: f32, y: f32, z: f32) {
        self.set(rig, name, v3(x, y, z));
    }
    /// Add to a bone's rotation.
    pub fn add(&mut self, rig: &Rig, name: &str, rot: V3) {
        if let Some(i) = rig.bone(name) {
            self.rot[i] = self.rot[i] + rot;
        }
    }
    pub fn offset(&mut self, rig: &Rig, name: &str, off: V3) {
        if let Some(i) = rig.bone(name) {
            self.off[i] = off;
        }
    }
    pub fn scale(&mut self, rig: &Rig, name: &str, s: V3) {
        if let Some(i) = rig.bone(name) {
            self.scl[i] = s;
        }
    }

    /// Mirror a pose's forward/back swing between left and right limbs
    /// (used to build the second half of a walk cycle from the first).
    pub fn swap_sides(&mut self, rig: &Rig) {
        let names = [
            "upper_arm",
            "forearm",
            "hand",
            "thigh",
            "shin",
            "foot",
            "shoulder",
        ];
        for n in names {
            let l = rig.bone(&format!("{n}_l"));
            let r = rig.bone(&format!("{n}_r"));
            if let (Some(l), Some(r)) = (l, r) {
                self.rot.swap(l, r);
                self.off.swap(l, r);
                self.scl.swap(l, r);
            }
        }
    }

    /// Linear blend `self → o` by `t` (0..1). Rotations are blended as Euler
    /// angles, which is what the hand-keyed poses here expect.
    pub fn blend(&self, o: &Pose, t: f32) -> Pose {
        let t = t.clamp(0.0, 1.0);
        Pose {
            rot: self
                .rot
                .iter()
                .zip(&o.rot)
                .map(|(a, b)| a.lerp(*b, t))
                .collect(),
            off: self
                .off
                .iter()
                .zip(&o.off)
                .map(|(a, b)| a.lerp(*b, t))
                .collect(),
            scl: self
                .scl
                .iter()
                .zip(&o.scl)
                .map(|(a, b)| a.lerp(*b, t))
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::math3::M3;

    fn arm_rig() -> Rig {
        let mut r = Rig::new();
        r.add("root", "", V3::ZERO);
        r.add("upper", "root", v3(0.0, 10.0, 0.0));
        r.add("lower", "upper", v3(0.0, -5.0, 0.0));
        r
    }

    #[test]
    fn hierarchy_composes() {
        let rig = arm_rig();
        let mut pose = rig.rest_pose();
        let w = rig.world(&pose, &Xf::IDENTITY);
        assert_eq!(rig.joint(&w, "lower"), Some(v3(0.0, 5.0, 0.0)));
        // Rotate the upper bone 90° about Z: the lower joint (5 below) swings
        // to +X (see math3 rotation convention).
        pose.rot(&rig, "upper", 0.0, 0.0, 90.0);
        let w = rig.world(&pose, &Xf::IDENTITY);
        let j = rig.joint(&w, "lower").unwrap();
        assert!((j - v3(5.0, 10.0, 0.0)).len() < 1e-4, "{j:?}");
    }

    #[test]
    fn blend_and_mirror() {
        let rig = arm_rig();
        let a = rig.rest_pose();
        let mut b = rig.rest_pose();
        b.rot(&rig, "upper", 0.0, 0.0, 40.0);
        let m = a.blend(&b, 0.5);
        assert!((m.rot[1].z - 20.0).abs() < 1e-5);
        let root = Xf::new(M3::scale(v3(-1.0, 1.0, 1.0)), V3::ZERO);
        let w = rig.world(&b, &root);
        let j = rig.joint(&w, "lower").unwrap();
        assert!(j.x < 0.0, "mirrored root flips facing: {j:?}");
    }
}
