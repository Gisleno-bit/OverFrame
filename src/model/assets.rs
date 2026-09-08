//! Optional artist-made assets: `.glb` files that replace the procedural
//! fighter rigs and add stage dressing, found under an `assets/` directory
//! (next to the executable, or the current directory, or `$OVERFRAME_ASSETS`).
//!
//! ```text
//! assets/
//!   characters/kestrel.glb      # replaces Kestrel's rig (see docs/ART_PIPELINE.md)
//!   stages/the-lattice.glb      # dressing for The Lattice (node "slab" replaces the platform)
//! ```

use super::gltf_io;
use super::mesh::MeshData;
use super::rig::Rig;
use crate::sim::roster::CharacterId;
use crate::sim::stage::StageId;
use std::path::PathBuf;

/// Candidate asset roots, most specific first.
pub fn asset_dirs() -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Ok(p) = std::env::var("OVERFRAME_ASSETS") {
        v.push(PathBuf::from(p));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            v.push(dir.join("assets"));
        }
    }
    v.push(PathBuf::from("assets"));
    v
}

fn slug(name: &str) -> String {
    name.to_ascii_lowercase().replace(' ', "-")
}

fn read_first(rel: &str) -> Option<(PathBuf, Vec<u8>)> {
    for dir in asset_dirs() {
        let p = dir.join(rel);
        if let Ok(bytes) = std::fs::read(&p) {
            return Some((p, bytes));
        }
    }
    None
}

/// Path (relative to an asset root) of a character's model.
pub fn character_rel(id: CharacterId) -> String {
    format!("characters/{}.glb", slug(id.name()))
}

/// Path (relative to an asset root) of a stage's dressing.
pub fn stage_rel(id: StageId) -> String {
    format!("stages/{}.glb", slug(id.name()))
}

/// Load a character's `.glb` rig if one is installed, scaled to the
/// character's collision height. Errors are reported on stderr and ignored
/// (the procedural model is used instead).
pub fn character_rig(id: CharacterId) -> Option<Rig> {
    let (path, bytes) = read_first(&character_rel(id))?;
    match gltf_io::load_rig(&bytes) {
        Ok(mut rig) => {
            gltf_io::fit_height(&mut rig, id.data().height);
            eprintln!(
                "assets: loaded {} ({} bones, {} tris)",
                path.display(),
                rig.len(),
                rig.triangle_count()
            );
            Some(rig)
        }
        Err(e) => {
            eprintln!("assets: {} ignored: {e}", path.display());
            None
        }
    }
}

/// Load a stage's dressing `.glb` if installed: `(mesh, replaces_slab)`.
pub fn stage_dressing(id: StageId) -> Option<(MeshData, bool)> {
    let (path, bytes) = read_first(&stage_rel(id))?;
    match gltf_io::load_static(&bytes) {
        Ok(v) => {
            eprintln!(
                "assets: loaded {} ({} tris)",
                path.display(),
                v.0.triangle_count()
            );
            Some(v)
        }
        Err(e) => {
            eprintln!("assets: {} ignored: {e}", path.display());
            None
        }
    }
}
