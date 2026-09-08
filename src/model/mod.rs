//! 3D content layer: meshes, segmented rigs, procedural animation, the three
//! fighter models, stage models, the match camera and (optionally) glTF
//! import. Pure CPU code with no GPU dependency, so it is unit-tested in CI and
//! shared by any backend; `render::scene3d` is the macroquad side that uploads
//! what this module produces.

pub mod anim;
pub mod camera;
pub mod characters;
pub mod lighting;
pub mod math3;
pub mod mesh;
pub mod palettes;
pub mod rig;
pub mod stage3d;

#[cfg(feature = "gltf")]
pub mod gltf_import;

pub use camera::MatchCamera;
pub use characters::CharacterModel;
pub use lighting::{Light, Palette, Tint};
pub use mesh::MeshData;
pub use rig::{Pose, Rig, Vert};
