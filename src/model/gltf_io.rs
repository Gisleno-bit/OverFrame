//! glTF 2.0 binary (`.glb`) import and export for rigs and stage dressing.
//!
//! The art pipeline is *Blender → .glb → OVERFRAME* with no custom tooling:
//!
//! * A fighter is a hierarchy of **parented objects** (no armature, no
//!   skinning): one object per bone, named with the standard bone names
//!   (`root`, `hips`, `spine`, `chest`, `neck`, `head`, `upper_arm_r`,
//!   `forearm_r`, `hand_r`, `thigh_r`, `shin_r`, `foot_r`, same with `_l`,
//!   plus any extras). Each object's mesh becomes that bone's rigid part.
//! * **Materials name the palette slot**: `primary`, `secondary`, `accent`,
//!   `skin`, `dark`, `glow`, `light`, `extra` (a prefix like `mat_` or
//!   `M_` is ignored). That is how one model gets every colour scheme.
//! * Units: 1 Blender unit = 1 game unit (fighters are ~30 tall); the model
//!   faces **-Y in Blender** (= +Z in glTF), which the importer turns into the
//!   engine's +X. [`load_rig`] can also rescale a model to a target height.
//! * Rest rotation/scale on objects are baked into their meshes so bone axes
//!   stay world-aligned (what the animation keys assume). Keep object
//!   rotations at zero in the rest pose for the cleanest result.
//!
//! [`export_rig`] writes any in-engine rig (including the procedural ones) as
//! a `.glb`, so an artist starts from the exact skeleton, proportions and
//! placeholder parts the game uses. Import(export(rig)) round-trips, which the
//! tests check.
//!
//! See `docs/ART_PIPELINE.md` for the artist-facing version of this.

use super::lighting::{slot, Palette};
use super::math3::{v3, Xf, M3, V3};
use super::mesh::MeshData;
use super::rig::Rig;

/// Import error.
#[derive(Debug)]
pub struct GltfError(pub String);

impl std::fmt::Display for GltfError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for GltfError {}

fn err<T>(msg: impl Into<String>) -> Result<T, GltfError> {
    Err(GltfError(msg.into()))
}

/// glTF's +Z-forward to the engine's +X-forward.
fn gltf_to_engine() -> M3 {
    M3::rot_y(90.0)
}
fn engine_to_gltf() -> M3 {
    M3::rot_y(-90.0)
}

/// Palette slot from a material name (`primary`, `mat_accent`, `M_Glow`…).
pub fn slot_from_name(name: &str) -> Option<u8> {
    let mut n = name.to_ascii_lowercase();
    // Blender's ".001" duplicate suffix.
    if let Some(dot) = n.rfind('.') {
        if n[dot + 1..].chars().all(|c| c.is_ascii_digit()) {
            n.truncate(dot);
        }
    }
    let n = n.rsplit(['_', ' ']).next().unwrap_or(&n).to_string();
    let n = n.trim_end_matches(|c: char| c.is_ascii_digit());
    Some(match n {
        "primary" | "main" | "body" => slot::PRIMARY,
        "secondary" => slot::SECONDARY,
        "accent" => slot::ACCENT,
        "skin" => slot::SKIN,
        "dark" => slot::DARK,
        "glow" | "emissive" => slot::GLOW,
        "light" => slot::LIGHT,
        "extra" => slot::EXTRA,
        _ => return None,
    })
}

fn slot_name(s: u8) -> &'static str {
    match s {
        slot::PRIMARY => "primary",
        slot::SECONDARY => "secondary",
        slot::ACCENT => "accent",
        slot::SKIN => "skin",
        slot::DARK => "dark",
        slot::GLOW => "glow",
        slot::LIGHT => "light",
        _ => "extra",
    }
}

fn quat_to_m3(q: [f32; 4]) -> M3 {
    let [x, y, z, w] = q;
    M3 {
        m: [
            [
                1.0 - 2.0 * (y * y + z * z),
                2.0 * (x * y - z * w),
                2.0 * (x * z + y * w),
            ],
            [
                2.0 * (x * y + z * w),
                1.0 - 2.0 * (x * x + z * z),
                2.0 * (y * z - x * w),
            ],
            [
                2.0 * (x * z - y * w),
                2.0 * (y * z + x * w),
                1.0 - 2.0 * (x * x + y * y),
            ],
        ],
    }
}

// ----------------------------------------------------------------- import

struct Doc {
    gltf: gltf::Gltf,
}

impl Doc {
    fn parse(bytes: &[u8]) -> Result<Doc, GltfError> {
        let gltf =
            gltf::Gltf::from_slice(bytes).map_err(|e| GltfError(format!("glTF parse: {e}")))?;
        if gltf.blob.is_none() {
            return err("only binary .glb files with an embedded BIN chunk are supported (export as .glb from Blender)");
        }
        Ok(Doc { gltf })
    }

    fn mesh_data(&self, mesh: gltf::Mesh<'_>, xf: &Xf) -> Result<MeshData, GltfError> {
        let blob = self.gltf.blob.as_deref();
        let mut out = MeshData::new();
        for prim in mesh.primitives() {
            if prim.mode() != gltf::mesh::Mode::Triangles {
                continue;
            }
            let reader = prim.reader(|buffer| match buffer.source() {
                gltf::buffer::Source::Bin => blob,
                gltf::buffer::Source::Uri(_) => None,
            });
            let Some(positions) = reader.read_positions() else {
                return err("primitive without POSITION");
            };
            let pos: Vec<V3> = positions.map(|p| v3(p[0], p[1], p[2])).collect();
            let nrm: Option<Vec<V3>> = reader
                .read_normals()
                .map(|n| n.map(|n| v3(n[0], n[1], n[2])).collect());
            let uv: Option<Vec<[f32; 2]>> =
                reader.read_tex_coords(0).map(|t| t.into_f32().collect());
            let idx: Vec<u32> = match reader.read_indices() {
                Some(i) => i.into_u32().collect(),
                None => (0..pos.len() as u32).collect(),
            };
            let s = prim
                .material()
                .name()
                .and_then(slot_from_name)
                .unwrap_or(slot::PRIMARY);
            let mut m = MeshData {
                nrm: nrm.unwrap_or_else(|| vec![V3::Y; pos.len()]),
                uv: uv.unwrap_or_else(|| vec![[0.0, 0.0]; pos.len()]),
                slot: vec![s; pos.len()],
                pos,
                idx,
            };
            if m.idx.iter().any(|&i| i as usize >= m.pos.len()) {
                return err("index out of range");
            }
            if reader.read_normals().is_none() {
                m.recompute_normals();
            }
            m.transform_in_place(xf);
            out.append(&m);
        }
        Ok(out)
    }
}

/// Load a segmented rig from a `.glb`. Node names become bone names; each
/// node's mesh becomes the bone's part; rest rotation/scale are baked.
pub fn load_rig(bytes: &[u8]) -> Result<Rig, GltfError> {
    let doc = Doc::parse(bytes)?;
    let scene = doc
        .gltf
        .default_scene()
        .or_else(|| doc.gltf.scenes().next())
        .ok_or_else(|| GltfError("no scene".into()))?;
    let mut rig = Rig::new();
    let conv = gltf_to_engine();
    let roots: Vec<gltf::Node<'_>> = scene.nodes().collect();
    if roots.is_empty() {
        return err("scene has no nodes");
    }
    // If there is more than one root, synthesise a `root` bone.
    let single_root = roots.len() == 1;
    let mut parent_name = String::new();
    if !single_root {
        rig.add("root", "", V3::ZERO);
        parent_name = "root".into();
    }
    for n in roots {
        visit(&doc, &mut rig, n, &parent_name, &conv, M3::IDENTITY)?;
    }
    if rig.bone("root").is_none() {
        return err("no bone named 'root' (the top-level object must be called root)");
    }
    Ok(rig)
}

/// `parent_bake` is the accumulated rotation/scale of the ancestors, applied
/// to this node's translation so offsets stay in world-aligned bone frames.
fn visit(
    doc: &Doc,
    rig: &mut Rig,
    node: gltf::Node<'_>,
    parent: &str,
    conv: &M3,
    parent_bake: M3,
) -> Result<(), GltfError> {
    let name = node
        .name()
        .map(|n| n.to_ascii_lowercase())
        .unwrap_or_else(|| format!("node{}", node.index()));
    let (t, r, s) = node.transform().decomposed();
    let local_rs = quat_to_m3(r) * M3::scale(v3(s[0], s[1], s[2]));
    let bake = parent_bake * local_rs;
    // Offset in engine space: convert glTF axes, then apply ancestor bake.
    let offset = conv.apply(parent_bake.apply(v3(t[0], t[1], t[2])));
    let bone_name = if rig.bone(&name).is_some() {
        format!("{name}.{}", node.index())
    } else {
        name
    };
    rig.add(&bone_name, parent, offset);
    if let Some(mesh) = node.mesh() {
        let xf = Xf::new(*conv * bake, V3::ZERO);
        let m = doc.mesh_data(mesh, &xf)?;
        rig.attach(&bone_name, m);
    }
    for child in node.children() {
        visit(doc, rig, child, &bone_name, conv, bake)?;
    }
    Ok(())
}

/// Load stage dressing: every mesh in the file, flattened into one
/// [`MeshData`] in engine space. Returns `(mesh, has_slab)` where `has_slab`
/// is true if a node named `slab` exists (the procedural main platform is
/// then omitted by the renderer).
pub fn load_static(bytes: &[u8]) -> Result<(MeshData, bool), GltfError> {
    let doc = Doc::parse(bytes)?;
    let scene = doc
        .gltf
        .default_scene()
        .or_else(|| doc.gltf.scenes().next())
        .ok_or_else(|| GltfError("no scene".into()))?;
    let conv = gltf_to_engine();
    let mut out = MeshData::new();
    let mut has_slab = false;
    fn walk(
        doc: &Doc,
        node: gltf::Node<'_>,
        parent: Xf,
        conv: &M3,
        out: &mut MeshData,
        has_slab: &mut bool,
    ) -> Result<(), GltfError> {
        let (t, r, s) = node.transform().decomposed();
        let local = Xf::new(
            quat_to_m3(r) * M3::scale(v3(s[0], s[1], s[2])),
            v3(t[0], t[1], t[2]),
        );
        let world = parent * local;
        if node.name().is_some_and(|n| n.eq_ignore_ascii_case("slab")) {
            *has_slab = true;
        }
        if let Some(mesh) = node.mesh() {
            let xf = Xf::new(*conv, V3::ZERO) * world;
            out.append(&doc.mesh_data(mesh, &xf)?);
        }
        for c in node.children() {
            walk(doc, c, world, conv, out, has_slab)?;
        }
        Ok(())
    }
    for n in scene.nodes() {
        walk(&doc, n, Xf::IDENTITY, &conv, &mut out, &mut has_slab)?;
    }
    Ok((out, has_slab))
}

/// Uniformly scale a rig (offsets and meshes) so its rest-pose height equals
/// `target`.
pub fn fit_height(rig: &mut Rig, target: f32) {
    let w = rig.world(&rig.rest_pose(), &Xf::IDENTITY);
    let mut top = f32::MIN;
    let mut bottom = f32::MAX;
    for (bi, b) in rig.bones.iter().enumerate() {
        for p in &b.mesh.pos {
            let y = w[bi].point(*p).y;
            top = top.max(y);
            bottom = bottom.min(y);
        }
    }
    let h = top - bottom;
    if h <= 1e-3 || target <= 0.0 {
        return;
    }
    let k = target / h;
    let xf = Xf::new(M3::scale(v3(k, k, k)), V3::ZERO);
    for b in &mut rig.bones {
        b.offset = b.offset * k;
        b.mesh.transform_in_place(&xf);
    }
    // Drop the model so its lowest point sits on the floor.
    let w = rig.world(&rig.rest_pose(), &Xf::IDENTITY);
    let mut bottom = f32::MAX;
    for (bi, b) in rig.bones.iter().enumerate() {
        for p in &b.mesh.pos {
            bottom = bottom.min(w[bi].point(*p).y);
        }
    }
    if let Some(r) = rig.bone("root") {
        rig.bones[r].offset.y -= bottom;
    }
}

// ----------------------------------------------------------------- export

/// Write a rig as a `.glb` (one node per bone, one primitive per slot, a
/// material per slot coloured from `palette` so it previews sensibly).
pub fn export_rig(rig: &Rig, palette: &Palette) -> Vec<u8> {
    let conv = engine_to_gltf();
    let mut bin: Vec<u8> = Vec::new();
    let mut buffer_views = Vec::new();
    let mut accessors = Vec::new();
    let mut meshes = Vec::new();
    let mut nodes = Vec::new();

    let mut push_view = |bin: &mut Vec<u8>, bytes: &[u8], target: u32| -> usize {
        while bin.len() % 4 != 0 {
            bin.push(0);
        }
        let off = bin.len();
        bin.extend_from_slice(bytes);
        buffer_views.push(format!(
            r#"{{"buffer":0,"byteOffset":{off},"byteLength":{},"target":{target}}}"#,
            bytes.len()
        ));
        buffer_views.len() - 1
    };

    for (bi, b) in rig.bones.iter().enumerate() {
        let mut mesh_index = None;
        if !b.mesh.idx.is_empty() {
            // Group triangles by slot; each group becomes a primitive with
            // its own (compact) vertex set.
            let mut groups: std::collections::BTreeMap<u8, Vec<u32>> = Default::default();
            for t in b.mesh.idx.chunks(3) {
                groups
                    .entry(b.mesh.slot[t[0] as usize])
                    .or_default()
                    .extend_from_slice(t);
            }
            let mut prims = Vec::new();
            for (s, idx) in groups {
                let mut remap = vec![u32::MAX; b.mesh.pos.len()];
                let mut local: Vec<u32> = Vec::with_capacity(idx.len());
                let mut used: Vec<usize> = Vec::new();
                for &i in &idx {
                    let i = i as usize;
                    if remap[i] == u32::MAX {
                        remap[i] = used.len() as u32;
                        used.push(i);
                    }
                    local.push(remap[i]);
                }
                let n = used.len();
                let mut pos_bytes = Vec::with_capacity(n * 12);
                let mut nrm_bytes = Vec::with_capacity(n * 12);
                let mut uv_bytes = Vec::with_capacity(n * 8);
                let mut lo = [f32::MAX; 3];
                let mut hi = [f32::MIN; 3];
                for &i in &used {
                    let p = conv.apply(b.mesh.pos[i]);
                    let nn = conv.apply(b.mesh.nrm[i]);
                    for (k, v) in [p.x, p.y, p.z].iter().enumerate() {
                        lo[k] = lo[k].min(*v);
                        hi[k] = hi[k].max(*v);
                        pos_bytes.extend_from_slice(&v.to_le_bytes());
                    }
                    for v in [nn.x, nn.y, nn.z] {
                        nrm_bytes.extend_from_slice(&v.to_le_bytes());
                    }
                    for v in b.mesh.uv[i] {
                        uv_bytes.extend_from_slice(&v.to_le_bytes());
                    }
                }
                let pv = push_view(&mut bin, &pos_bytes, 34962);
                let nv = push_view(&mut bin, &nrm_bytes, 34962);
                let uvv = push_view(&mut bin, &uv_bytes, 34962);
                let pa = accessors.len();
                accessors.push(format!(
                    r#"{{"bufferView":{pv},"componentType":5126,"count":{n},"type":"VEC3","min":[{},{},{}],"max":[{},{},{}]}}"#,
                    lo[0], lo[1], lo[2], hi[0], hi[1], hi[2]
                ));
                let na = accessors.len();
                accessors.push(format!(
                    r#"{{"bufferView":{nv},"componentType":5126,"count":{n},"type":"VEC3"}}"#
                ));
                let ua = accessors.len();
                accessors.push(format!(
                    r#"{{"bufferView":{uvv},"componentType":5126,"count":{n},"type":"VEC2"}}"#
                ));
                let mut ib = Vec::with_capacity(local.len() * 4);
                for i in &local {
                    ib.extend_from_slice(&i.to_le_bytes());
                }
                let iv = push_view(&mut bin, &ib, 34963);
                let ia = accessors.len();
                accessors.push(format!(
                    r#"{{"bufferView":{iv},"componentType":5125,"count":{},"type":"SCALAR"}}"#,
                    local.len()
                ));
                prims.push(format!(
                    r#"{{"attributes":{{"POSITION":{pa},"NORMAL":{na},"TEXCOORD_0":{ua}}},"indices":{ia},"material":{}}}"#,
                    s as usize % slot::COUNT
                ));
            }
            meshes.push(format!(
                r#"{{"name":"{}_mesh","primitives":[{}]}}"#,
                json_escape(&b.name),
                prims.join(",")
            ));
            mesh_index = Some(meshes.len() - 1);
        }
        let children: Vec<String> = rig
            .bones
            .iter()
            .enumerate()
            .filter(|(_, c)| c.parent == Some(bi))
            .map(|(i, _)| i.to_string())
            .collect();
        let t = conv.apply(b.offset);
        let mut node = format!(
            r#"{{"name":"{}","translation":[{},{},{}]"#,
            json_escape(&b.name),
            t.x,
            t.y,
            t.z
        );
        if let Some(m) = mesh_index {
            node.push_str(&format!(r#","mesh":{m}"#));
        }
        if !children.is_empty() {
            node.push_str(&format!(r#","children":[{}]"#, children.join(",")));
        }
        node.push('}');
        nodes.push(node);
    }

    let materials: Vec<String> = (0..slot::COUNT)
        .map(|s| {
            let c = palette.color(s as u8);
            let f = |v: u8| (v as f32 / 255.0).powf(2.2);
            format!(
                r#"{{"name":"{}","pbrMetallicRoughness":{{"baseColorFactor":[{},{},{},1.0],"metallicFactor":0.0,"roughnessFactor":0.8}}}}"#,
                slot_name(s as u8),
                f(c[0]),
                f(c[1]),
                f(c[2])
            )
        })
        .collect();
    let roots: Vec<String> = rig
        .bones
        .iter()
        .enumerate()
        .filter(|(_, b)| b.parent.is_none())
        .map(|(i, _)| i.to_string())
        .collect();
    let json = format!(
        r#"{{"asset":{{"version":"2.0","generator":"OVERFRAME rig exporter"}},"scene":0,"scenes":[{{"name":"rig","nodes":[{}]}}],"nodes":[{}],"meshes":[{}],"materials":[{}],"accessors":[{}],"bufferViews":[{}],"buffers":[{{"byteLength":{}}}]}}"#,
        roots.join(","),
        nodes.join(","),
        meshes.join(","),
        materials.join(","),
        accessors.join(","),
        buffer_views.join(","),
        bin.len()
    );
    pack_glb(json.as_bytes(), &bin)
}

fn json_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Assemble a GLB container from a JSON chunk and a BIN chunk.
pub fn pack_glb(json: &[u8], bin: &[u8]) -> Vec<u8> {
    let mut j = json.to_vec();
    while j.len() % 4 != 0 {
        j.push(b' ');
    }
    let mut b = bin.to_vec();
    while b.len() % 4 != 0 {
        b.push(0);
    }
    let total = 12 + 8 + j.len() + 8 + b.len();
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&(j.len() as u32).to_le_bytes());
    out.extend_from_slice(&0x4E4F_534Au32.to_le_bytes());
    out.extend_from_slice(&j);
    out.extend_from_slice(&(b.len() as u32).to_le_bytes());
    out.extend_from_slice(&0x004E_4942u32.to_le_bytes());
    out.extend_from_slice(&b);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::characters;
    use crate::sim::roster::CharacterId;

    #[test]
    fn export_import_round_trips_every_fighter() {
        for id in CharacterId::ALL {
            let model = characters::build(id);
            let glb = export_rig(&model.rig, model.palette(0));
            assert_eq!(&glb[0..4], b"glTF");
            let back = load_rig(&glb).unwrap_or_else(|e| panic!("{:?}: {e}", id));
            assert_eq!(back.len(), model.rig.len(), "{:?} bone count", id);
            // Bone order may differ (depth-first on import); match by name.
            for a in &model.rig.bones {
                let bi = back
                    .bone(&a.name)
                    .unwrap_or_else(|| panic!("{} missing", a.name));
                let b = &back.bones[bi];
                let pa = a.parent.map(|p| model.rig.bones[p].name.clone());
                let pb = b.parent.map(|p| back.bones[p].name.clone());
                assert_eq!(pa, pb, "{} parent", a.name);
                assert!((a.offset - b.offset).len() < 1e-3, "{} offset", a.name);
                assert_eq!(
                    a.mesh.triangle_count(),
                    b.mesh.triangle_count(),
                    "{} tris",
                    a.name
                );
                assert_eq!(
                    a.mesh.vertex_count(),
                    b.mesh.vertex_count(),
                    "{} verts",
                    a.name
                );
                // Slots survive via material names; positions survive the
                // axis conversion. Vertex order may change (primitives are
                // grouped by slot), so compare as sorted triangle keys.
                let keys = |m: &MeshData| {
                    let mut v: Vec<(u8, i64, i64, i64)> = m
                        .idx
                        .chunks(3)
                        .map(|t| {
                            let c = (m.pos[t[0] as usize]
                                + m.pos[t[1] as usize]
                                + m.pos[t[2] as usize])
                                * (1.0 / 3.0);
                            (
                                m.slot[t[0] as usize],
                                (c.x * 100.0).round() as i64,
                                (c.y * 100.0).round() as i64,
                                (c.z * 100.0).round() as i64,
                            )
                        })
                        .collect();
                    v.sort();
                    v
                };
                assert_eq!(keys(&a.mesh), keys(&b.mesh), "{} triangles", a.name);
            }
            // And it poses exactly like the original.
            let mut pa = model.rig.rest_pose();
            pa.rot(&model.rig, "upper_arm_r", 10.0, 0.0, 70.0);
            let mut pb = back.rest_pose();
            pb.rot(&back, "upper_arm_r", 10.0, 0.0, 70.0);
            let wa = model.rig.world(&pa, &Xf::IDENTITY);
            let wb = back.world(&pb, &Xf::IDENTITY);
            for a in &model.rig.bones {
                let ja = model.rig.joint(&wa, &a.name).unwrap();
                let jb = back.joint(&wb, &a.name).unwrap();
                assert!((ja - jb).len() < 1e-3, "{} joint", a.name);
            }
        }
    }

    #[test]
    fn rest_rotation_is_baked_and_height_fits() {
        // A two-node file: root with a child rotated 90° about Z carrying a
        // unit cube. After import the child's cube must be rotated (baked)
        // while the bone frame stays axis-aligned.
        let mut rig = Rig::new();
        rig.add("root", "", V3::ZERO);
        rig.add("arm", "root", v3(0.0, 10.0, 0.0));
        rig.attach("arm", MeshData::cuboid(4.0, 1.0, 1.0, slot::ACCENT));
        let pal = crate::model::palettes::get(CharacterId::Kestrel, 0);
        let glb = export_rig(&rig, pal);
        // Patch the JSON to add a rotation to node 1 (quaternion for 90°
        // about X: the box, which lies along glTF +Z, ends up along Y).
        let json_len = u32::from_le_bytes([glb[12], glb[13], glb[14], glb[15]]) as usize;
        let json = std::str::from_utf8(&glb[20..20 + json_len])
            .unwrap()
            .to_string();
        let bin = &glb[20 + json_len + 8..];
        let patched = json.replacen(
            r#""name":"arm","translation":[0,10,0]"#,
            r#""name":"arm","translation":[0,10,0],"rotation":[0.7071068,0,0,0.7071068]"#,
            1,
        );
        assert_ne!(patched, json, "test patch must apply");
        let glb2 = pack_glb(patched.as_bytes(), bin);
        let back = load_rig(&glb2).unwrap();
        let arm = back.bone("arm").unwrap();
        let (lo, hi) = back.bones[arm].mesh.bounds();
        // The 4-long box now extends along Y (rotated by the baked 90°).
        assert!(hi.y - lo.y > 3.9 && hi.x - lo.x < 1.1, "{lo:?} {hi:?}");
        assert!((back.bones[arm].offset - v3(0.0, 10.0, 0.0)).len() < 1e-3);

        let mut fitted = back.clone();
        fit_height(&mut fitted, 30.0);
        let w = fitted.world(&fitted.rest_pose(), &Xf::IDENTITY);
        let mut top = f32::MIN;
        let mut bottom = f32::MAX;
        for (bi, b) in fitted.bones.iter().enumerate() {
            for p in &b.mesh.pos {
                let y = w[bi].point(*p).y;
                top = top.max(y);
                bottom = bottom.min(y);
            }
        }
        assert!((top - bottom - 30.0).abs() < 1e-2);
        assert!(bottom.abs() < 1e-2);
    }

    #[test]
    fn material_names_map_to_slots() {
        assert_eq!(slot_from_name("primary"), Some(slot::PRIMARY));
        assert_eq!(slot_from_name("mat_Accent"), Some(slot::ACCENT));
        assert_eq!(slot_from_name("M_glow.001"), Some(slot::GLOW));
        assert_eq!(slot_from_name("Skin2"), Some(slot::SKIN));
        assert_eq!(slot_from_name("wood"), None);
    }

    #[test]
    fn static_import_flattens_and_detects_slab() {
        let mut rig = Rig::new();
        rig.add("slab", "", v3(0.0, 0.0, 0.0));
        rig.add("rock", "slab", v3(50.0, 0.0, -20.0));
        rig.attach("slab", MeshData::cuboid(10.0, 2.0, 4.0, slot::PRIMARY));
        rig.attach("rock", MeshData::sphere(3.0, 4, 6, slot::EXTRA));
        let glb = export_rig(
            &rig,
            &crate::model::stage3d::look(crate::sim::stage::StageId::Lattice).palette,
        );
        let (m, has_slab) = load_static(&glb).unwrap();
        assert!(has_slab);
        assert_eq!(m.triangle_count(), 12 + 4 * 6 * 2);
        let (lo, hi) = m.bounds();
        assert!(hi.x > 50.0 && lo.x < -4.0, "{lo:?} {hi:?}");
    }
}
