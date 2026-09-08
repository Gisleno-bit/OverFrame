//! Procedural mesh data and primitive builders.
//!
//! A [`MeshData`] is plain indexed geometry with a *material slot* per vertex
//! instead of a colour: palettes resolve slots to colours at draw time, which is
//! how one model gets N colour schemes for free. Builders return meshes centred
//! on the origin (or noted otherwise); compose with [`MeshData::transform`] and
//! [`MeshData::append`].
//!
//! Conventions: +Y up, +X is the direction a fighter faces, +Z toward the camera.

use super::math3::{v3, Xf, V3};
use std::f32::consts::PI;

#[derive(Clone, Debug, Default)]
pub struct MeshData {
    pub pos: Vec<V3>,
    pub nrm: Vec<V3>,
    pub uv: Vec<[f32; 2]>,
    pub slot: Vec<u8>,
    pub idx: Vec<u32>,
}

impl MeshData {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn vertex_count(&self) -> usize {
        self.pos.len()
    }
    pub fn triangle_count(&self) -> usize {
        self.idx.len() / 3
    }

    fn push(&mut self, p: V3, n: V3, uv: [f32; 2], slot: u8) -> u32 {
        self.pos.push(p);
        self.nrm.push(n);
        self.uv.push(uv);
        self.slot.push(slot);
        (self.pos.len() - 1) as u32
    }

    fn tri(&mut self, a: u32, b: u32, c: u32) {
        self.idx.push(a);
        self.idx.push(b);
        self.idx.push(c);
    }

    fn quad(&mut self, a: u32, b: u32, c: u32, d: u32) {
        self.tri(a, b, c);
        self.tri(a, c, d);
    }

    /// Assign every vertex to `slot`.
    pub fn slot(mut self, slot: u8) -> Self {
        for s in &mut self.slot {
            *s = slot;
        }
        self
    }

    /// Apply an affine transform (positions and normals; winding fixed if
    /// the transform mirrors).
    pub fn transform(mut self, xf: &Xf) -> Self {
        self.transform_in_place(xf);
        self
    }

    pub fn transform_in_place(&mut self, xf: &Xf) {
        for p in &mut self.pos {
            *p = xf.point(*p);
        }
        for n in &mut self.nrm {
            *n = xf.normal(*n);
        }
        if xf.mirrored() {
            for t in self.idx.chunks_mut(3) {
                t.swap(1, 2);
            }
        }
    }

    pub fn translate(self, t: V3) -> Self {
        self.transform(&Xf::translation(t))
    }

    /// Append another mesh (indices re-based).
    pub fn append(&mut self, o: &MeshData) {
        let base = self.pos.len() as u32;
        self.pos.extend_from_slice(&o.pos);
        self.nrm.extend_from_slice(&o.nrm);
        self.uv.extend_from_slice(&o.uv);
        self.slot.extend_from_slice(&o.slot);
        self.idx.extend(o.idx.iter().map(|i| i + base));
    }

    pub fn with(mut self, o: MeshData) -> Self {
        self.append(&o);
        self
    }

    /// Un-index into flat-shaded triangles (each face gets its own vertices
    /// and a face normal). Gives the faceted, low-poly look.
    pub fn flat(&self) -> MeshData {
        let mut out = MeshData::new();
        for t in self.idx.chunks(3) {
            let (a, b, c) = (t[0] as usize, t[1] as usize, t[2] as usize);
            let n = (self.pos[b] - self.pos[a])
                .cross(self.pos[c] - self.pos[a])
                .norm();
            let i0 = out.push(self.pos[a], n, self.uv[a], self.slot[a]);
            let i1 = out.push(self.pos[b], n, self.uv[b], self.slot[b]);
            let i2 = out.push(self.pos[c], n, self.uv[c], self.slot[c]);
            out.tri(i0, i1, i2);
        }
        out
    }

    /// Recompute smooth normals from the geometry (area-weighted).
    pub fn recompute_normals(&mut self) {
        let mut acc = vec![V3::ZERO; self.pos.len()];
        for t in self.idx.chunks(3) {
            let (a, b, c) = (t[0] as usize, t[1] as usize, t[2] as usize);
            let n = (self.pos[b] - self.pos[a]).cross(self.pos[c] - self.pos[a]);
            acc[a] = acc[a] + n;
            acc[b] = acc[b] + n;
            acc[c] = acc[c] + n;
        }
        self.nrm = acc.into_iter().map(|n| n.norm()).collect();
    }

    /// Planar-per-axis ("box") texture coordinates from world position: each
    /// vertex is projected along its normal's dominant axis, scaled by `scale`
    /// texture repeats per unit. Good enough for tiled stage materials.
    pub fn box_uv(mut self, scale: f32) -> Self {
        for (i, p) in self.pos.iter().enumerate() {
            let n = self.nrm[i];
            let (ax, ay, az) = (n.x.abs(), n.y.abs(), n.z.abs());
            self.uv[i] = if ay >= ax && ay >= az {
                [p.x * scale, p.z * scale]
            } else if ax >= az {
                [p.z * scale, p.y * scale]
            } else {
                [p.x * scale, p.y * scale]
            };
        }
        self
    }

    /// Axis-aligned bounds (min, max).
    pub fn bounds(&self) -> (V3, V3) {
        let mut lo = v3(f32::MAX, f32::MAX, f32::MAX);
        let mut hi = v3(f32::MIN, f32::MIN, f32::MIN);
        for p in &self.pos {
            lo = v3(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z));
            hi = v3(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z));
        }
        (lo, hi)
    }

    // ------------------------------------------------------------ builders

    /// Box of size `(sx, sy, sz)` centred on the origin, flat-shaded.
    pub fn cuboid(sx: f32, sy: f32, sz: f32, slot: u8) -> MeshData {
        let (hx, hy, hz) = (sx * 0.5, sy * 0.5, sz * 0.5);
        let mut m = MeshData::new();
        // (normal, four corners CCW seen from outside)
        let faces: [(V3, [V3; 4]); 6] = [
            (
                V3::Z,
                [
                    v3(-hx, -hy, hz),
                    v3(hx, -hy, hz),
                    v3(hx, hy, hz),
                    v3(-hx, hy, hz),
                ],
            ),
            (
                -V3::Z,
                [
                    v3(hx, -hy, -hz),
                    v3(-hx, -hy, -hz),
                    v3(-hx, hy, -hz),
                    v3(hx, hy, -hz),
                ],
            ),
            (
                V3::X,
                [
                    v3(hx, -hy, hz),
                    v3(hx, -hy, -hz),
                    v3(hx, hy, -hz),
                    v3(hx, hy, hz),
                ],
            ),
            (
                -V3::X,
                [
                    v3(-hx, -hy, -hz),
                    v3(-hx, -hy, hz),
                    v3(-hx, hy, hz),
                    v3(-hx, hy, -hz),
                ],
            ),
            (
                V3::Y,
                [
                    v3(-hx, hy, hz),
                    v3(hx, hy, hz),
                    v3(hx, hy, -hz),
                    v3(-hx, hy, -hz),
                ],
            ),
            (
                -V3::Y,
                [
                    v3(-hx, -hy, -hz),
                    v3(hx, -hy, -hz),
                    v3(hx, -hy, hz),
                    v3(-hx, -hy, hz),
                ],
            ),
        ];
        let uvs = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
        for (n, c) in faces.iter() {
            let i = [
                m.push(c[0], *n, uvs[0], slot),
                m.push(c[1], *n, uvs[1], slot),
                m.push(c[2], *n, uvs[2], slot),
                m.push(c[3], *n, uvs[3], slot),
            ];
            m.quad(i[0], i[1], i[2], i[3]);
        }
        m
    }

    /// Box with chamfered (bevelled) edges: a cuboid whose faces are inset by
    /// `bevel` and joined by 45° strips. Reads as a soft block under lighting.
    pub fn bevel_box(sx: f32, sy: f32, sz: f32, bevel: f32, slot: u8) -> MeshData {
        let b = bevel.min(sx * 0.45).min(sy * 0.45).min(sz * 0.45);
        // Build as the convex hull of a "superellipse"-ish frame: 8 corner
        // clusters × 3 points each, triangulated by hand is messy — instead,
        // compose: inner faces (6 quads) + 12 edge strips + 8 corner triangles.
        let (hx, hy, hz) = (sx * 0.5, sy * 0.5, sz * 0.5);
        let mut m = MeshData::new();
        // Corner offsets for the three inset points of corner (sx,sy,sz sign).
        let corner = |sxn: f32, syn: f32, szn: f32| -> [V3; 3] {
            // point on the X face, Y face, Z face for this corner.
            [
                v3(sxn * hx, syn * (hy - b), szn * (hz - b)),
                v3(sxn * (hx - b), syn * hy, szn * (hz - b)),
                v3(sxn * (hx - b), syn * (hy - b), szn * hz),
            ]
        };
        let signs = [-1.0f32, 1.0f32];
        // Map corner (i,j,k) → index of first of its 3 vertices.
        let mut ids = [[[0u32; 2]; 2]; 2];
        for (i, sxn) in signs.iter().enumerate() {
            for (j, syn) in signs.iter().enumerate() {
                for (k, szn) in signs.iter().enumerate() {
                    let c = corner(*sxn, *syn, *szn);
                    let n = v3(*sxn, *syn, *szn).norm();
                    let id = m.push(c[0], n, [0.0, 0.0], slot);
                    m.push(c[1], n, [0.0, 0.0], slot);
                    m.push(c[2], n, [0.0, 0.0], slot);
                    ids[i][j][k] = id;
                    // corner triangle
                    m.tri(id, id + 1, id + 2);
                }
            }
        }
        // Faces: for each axis face, 4 corner points (the point lying on that face).
        let face = |m: &mut MeshData, pts: [u32; 4]| m.quad(pts[0], pts[1], pts[2], pts[3]);
        // X faces use vertex +0, Y faces +1, Z faces +2.
        face(
            &mut m,
            [ids[1][0][1], ids[1][0][0], ids[1][1][0], ids[1][1][1]],
        );
        face(
            &mut m,
            [ids[0][0][0], ids[0][0][1], ids[0][1][1], ids[0][1][0]],
        );
        face(
            &mut m,
            [
                ids[0][1][1] + 1,
                ids[1][1][1] + 1,
                ids[1][1][0] + 1,
                ids[0][1][0] + 1,
            ],
        );
        face(
            &mut m,
            [
                ids[0][0][0] + 1,
                ids[1][0][0] + 1,
                ids[1][0][1] + 1,
                ids[0][0][1] + 1,
            ],
        );
        face(
            &mut m,
            [
                ids[0][0][1] + 2,
                ids[1][0][1] + 2,
                ids[1][1][1] + 2,
                ids[0][1][1] + 2,
            ],
        );
        face(
            &mut m,
            [
                ids[1][0][0] + 2,
                ids[0][0][0] + 2,
                ids[0][1][0] + 2,
                ids[1][1][0] + 2,
            ],
        );
        // Edge strips (12): each joins two corner clusters along one axis.
        // Along X (varying i): strip between Y-point and Z-point of both corners.
        for j in 0..2 {
            for k in 0..2 {
                let a = ids[0][j][k];
                let b2 = ids[1][j][k];
                m.quad(a + 1, b2 + 1, b2 + 2, a + 2);
            }
        }
        for i in 0..2 {
            for k in 0..2 {
                let a = ids[i][0][k];
                let b2 = ids[i][1][k];
                m.quad(a, b2, b2 + 2, a + 2);
            }
        }
        for i in 0..2 {
            for j in 0..2 {
                let a = ids[i][j][0];
                let b2 = ids[i][j][1];
                m.quad(a, b2, b2 + 1, a + 1);
            }
        }
        // Fix winding so everything faces outward, then use face normals.
        m.orient_outward();
        m.flat()
    }

    /// UV sphere (smooth normals).
    pub fn sphere(r: f32, lat: u32, lon: u32, slot: u8) -> MeshData {
        MeshData::ellipsoid(r, r, r, lat, lon, slot)
    }

    /// Ellipsoid with radii `(rx, ry, rz)`, smooth normals.
    pub fn ellipsoid(rx: f32, ry: f32, rz: f32, lat: u32, lon: u32, slot: u8) -> MeshData {
        let lat = lat.max(2);
        let lon = lon.max(3);
        let mut m = MeshData::new();
        for i in 0..=lat {
            let v = i as f32 / lat as f32;
            let phi = v * PI; // 0 top → PI bottom
            let (sp, cp) = phi.sin_cos();
            for j in 0..=lon {
                let u = j as f32 / lon as f32;
                let th = u * 2.0 * PI;
                let (st, ct) = th.sin_cos();
                let dir = v3(sp * ct, cp, sp * st);
                let p = v3(dir.x * rx, dir.y * ry, dir.z * rz);
                let n = v3(dir.x / rx, dir.y / ry, dir.z / rz).norm();
                m.push(p, n, [u, v], slot);
            }
        }
        let stride = lon + 1;
        for i in 0..lat {
            for j in 0..lon {
                let a = i * stride + j;
                let b = a + stride;
                m.quad(a, a + 1, b + 1, b);
            }
        }
        m
    }

    /// Cylinder/cone along +Y from `-h/2` (radius `r0`) to `+h/2` (radius
    /// `r1`), capped. Smooth sides.
    pub fn cylinder(r0: f32, r1: f32, h: f32, seg: u32, slot: u8) -> MeshData {
        let seg = seg.max(3);
        let mut m = MeshData::new();
        let (y0, y1) = (-h * 0.5, h * 0.5);
        let slope = (r0 - r1) / h.max(1e-4);
        for j in 0..=seg {
            let u = j as f32 / seg as f32;
            let th = u * 2.0 * PI;
            let (st, ct) = th.sin_cos();
            let n = v3(ct, slope, st).norm();
            m.push(v3(ct * r0, y0, st * r0), n, [u, 0.0], slot);
            m.push(v3(ct * r1, y1, st * r1), n, [u, 1.0], slot);
        }
        for j in 0..seg {
            let a = j * 2;
            m.quad(a, a + 1, a + 3, a + 2);
        }
        // caps
        let ctop = m.push(v3(0.0, y1, 0.0), V3::Y, [0.5, 0.5], slot);
        let cbot = m.push(v3(0.0, y0, 0.0), -V3::Y, [0.5, 0.5], slot);
        let base = m.pos.len() as u32;
        for j in 0..=seg {
            let th = j as f32 / seg as f32 * 2.0 * PI;
            let (st, ct) = th.sin_cos();
            m.push(
                v3(ct * r1, y1, st * r1),
                V3::Y,
                [0.5 + ct * 0.5, 0.5 + st * 0.5],
                slot,
            );
            m.push(
                v3(ct * r0, y0, st * r0),
                -V3::Y,
                [0.5 + ct * 0.5, 0.5 + st * 0.5],
                slot,
            );
        }
        for j in 0..seg {
            let a = base + j * 2;
            m.tri(ctop, a + 2, a);
            m.tri(cbot, a + 1, a + 3);
        }
        m
    }

    /// Capsule along +Y: total length `len` (including caps), radius `r`.
    pub fn capsule(r: f32, len: f32, seg: u32, slot: u8) -> MeshData {
        let body = (len - 2.0 * r).max(0.0);
        let rings = 3u32;
        let seg = seg.max(4);
        let mut m = MeshData::new();
        // Rows: top hemisphere (rings+1 rows), bottom hemisphere (rings+1 rows).
        let mut rows: Vec<(f32, f32)> = Vec::new(); // (y, radius)
        let mut row_n: Vec<f32> = Vec::new(); // normal y-component factor
        for i in 0..=rings {
            let phi = i as f32 / rings as f32 * (PI * 0.5);
            rows.push((body * 0.5 + r * phi.cos(), r * phi.sin()));
            row_n.push(phi.cos());
        }
        for i in 0..=rings {
            let phi = PI * 0.5 + i as f32 / rings as f32 * (PI * 0.5);
            rows.push((-body * 0.5 + r * phi.cos(), r * phi.sin()));
            row_n.push(phi.cos());
        }
        for (ri, (y, rad)) in rows.iter().enumerate() {
            let ny = row_n[ri];
            let nr = (1.0 - ny * ny).max(0.0).sqrt();
            for j in 0..=seg {
                let u = j as f32 / seg as f32;
                let th = u * 2.0 * PI;
                let (st, ct) = th.sin_cos();
                m.push(
                    v3(ct * rad, *y, st * rad),
                    v3(ct * nr, ny, st * nr).norm(),
                    [u, ri as f32 / (rows.len() - 1) as f32],
                    slot,
                );
            }
        }
        let stride = seg + 1;
        for i in 0..(rows.len() as u32 - 1) {
            for j in 0..seg {
                let a = i * stride + j;
                let b = a + stride;
                m.quad(a, a + 1, b + 1, b);
            }
        }
        m
    }

    /// A flat plate: polygon in the XY plane (CCW), extruded `thick` along Z,
    /// centred on z=0. Good for fins, crests, feathers, blades.
    pub fn plate(points: &[(f32, f32)], thick: f32, slot: u8) -> MeshData {
        let mut m = MeshData::new();
        let hz = thick * 0.5;
        let n = points.len() as u32;
        // front and back fans
        let front = m.pos.len() as u32;
        for (x, y) in points {
            m.push(v3(*x, *y, hz), V3::Z, [0.0, 0.0], slot);
        }
        let back = m.pos.len() as u32;
        for (x, y) in points {
            m.push(v3(*x, *y, -hz), -V3::Z, [0.0, 0.0], slot);
        }
        for i in 1..n - 1 {
            m.tri(front, front + i, front + i + 1);
            m.tri(back, back + i + 1, back + i);
        }
        // sides
        for i in 0..n {
            let j = (i + 1) % n;
            let (ax, ay) = points[i as usize];
            let (bx, by) = points[j as usize];
            let e = v3(bx - ax, by - ay, 0.0);
            let nn = v3(e.y, -e.x, 0.0).norm();
            let a0 = m.push(v3(bx, by, hz), nn, [0.0, 0.0], slot);
            let a1 = m.push(v3(ax, ay, hz), nn, [0.0, 0.0], slot);
            let a2 = m.push(v3(ax, ay, -hz), nn, [0.0, 0.0], slot);
            let a3 = m.push(v3(bx, by, -hz), nn, [0.0, 0.0], slot);
            m.quad(a0, a1, a2, a3);
        }
        m
    }

    /// Torus in the XZ plane (axis Y).
    pub fn torus(big: f32, small: f32, seg_big: u32, seg_small: u32, slot: u8) -> MeshData {
        let (sb, ss) = (seg_big.max(3), seg_small.max(3));
        let mut m = MeshData::new();
        for i in 0..=sb {
            let th = i as f32 / sb as f32 * 2.0 * PI;
            let (st, ct) = th.sin_cos();
            for j in 0..=ss {
                let ph = j as f32 / ss as f32 * 2.0 * PI;
                let (sp, cp) = ph.sin_cos();
                let n = v3(ct * cp, sp, st * cp);
                let p = v3(ct * (big + small * cp), small * sp, st * (big + small * cp));
                m.push(p, n, [i as f32 / sb as f32, j as f32 / ss as f32], slot);
            }
        }
        let stride = ss + 1;
        for i in 0..sb {
            for j in 0..ss {
                let a = i * stride + j;
                let b = a + stride;
                m.quad(a, a + 1, b + 1, b);
            }
        }
        m
    }

    /// Flat quad in the XZ plane (facing +Y), size `(sx, sz)`, with `uv_scale`
    /// texture repeats.
    pub fn ground_quad(sx: f32, sz: f32, uv_scale: f32, slot: u8) -> MeshData {
        let (hx, hz) = (sx * 0.5, sz * 0.5);
        let mut m = MeshData::new();
        let a = m.push(v3(-hx, 0.0, hz), V3::Y, [0.0, 0.0], slot);
        let b = m.push(v3(hx, 0.0, hz), V3::Y, [uv_scale, 0.0], slot);
        let c = m.push(v3(hx, 0.0, -hz), V3::Y, [uv_scale, uv_scale], slot);
        let d = m.push(v3(-hx, 0.0, -hz), V3::Y, [0.0, uv_scale], slot);
        m.quad(a, b, c, d);
        m
    }

    /// Flat quad in the XY plane facing +Z, centred, size `(sx, sy)`.
    pub fn billboard_quad(sx: f32, sy: f32, slot: u8) -> MeshData {
        let (hx, hy) = (sx * 0.5, sy * 0.5);
        let mut m = MeshData::new();
        let a = m.push(v3(-hx, -hy, 0.0), V3::Z, [0.0, 1.0], slot);
        let b = m.push(v3(hx, -hy, 0.0), V3::Z, [1.0, 1.0], slot);
        let c = m.push(v3(hx, hy, 0.0), V3::Z, [1.0, 0.0], slot);
        let d = m.push(v3(-hx, hy, 0.0), V3::Z, [0.0, 0.0], slot);
        m.quad(a, b, c, d);
        m
    }

    /// Make every triangle wind outward from the mesh centroid (for convex
    /// shapes built by hand).
    pub fn orient_outward(&mut self) {
        let n = self.pos.len().max(1) as f32;
        let c = self.pos.iter().fold(V3::ZERO, |a, p| a + *p) * (1.0 / n);
        for t in self.idx.chunks_mut(3) {
            let (a, b, cc) = (
                self.pos[t[0] as usize],
                self.pos[t[1] as usize],
                self.pos[t[2] as usize],
            );
            let fnrm = (b - a).cross(cc - a);
            let centre = (a + b + cc) * (1.0 / 3.0);
            if fnrm.dot(centre - c) < 0.0 {
                t.swap(1, 2);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn closed(m: &MeshData) -> bool {
        // Every directed edge must have a matching opposite edge.
        use std::collections::HashMap;
        let key = |p: V3| {
            (
                (p.x * 1000.0) as i64,
                (p.y * 1000.0) as i64,
                (p.z * 1000.0) as i64,
            )
        };
        let mut edges: HashMap<(_, _), i32> = HashMap::new();
        for t in m.idx.chunks(3) {
            for e in 0..3 {
                let a = key(m.pos[t[e] as usize]);
                let b = key(m.pos[t[(e + 1) % 3] as usize]);
                *edges.entry((a, b)).or_default() += 1;
                *edges.entry((b, a)).or_default() -= 1;
            }
        }
        edges.values().all(|v| *v == 0)
    }

    #[test]
    fn primitives_are_closed_and_outward() {
        for (name, m) in [
            ("cuboid", MeshData::cuboid(2.0, 3.0, 4.0, 0)),
            ("bevel_box", MeshData::bevel_box(4.0, 6.0, 3.0, 0.5, 0)),
            ("sphere", MeshData::sphere(1.0, 6, 8, 0)),
            ("cylinder", MeshData::cylinder(1.0, 0.5, 2.0, 8, 0)),
            ("capsule", MeshData::capsule(1.0, 4.0, 8, 0)),
            (
                "plate",
                MeshData::plate(&[(0.0, 0.0), (2.0, 0.0), (1.0, 3.0)], 0.2, 0),
            ),
        ] {
            assert!(closed(&m), "{name} not closed");
            // Outward: face normal · (centroid - mesh centre) > 0 for most faces.
            let n = m.pos.len() as f32;
            let c = m.pos.iter().fold(V3::ZERO, |a, p| a + *p) * (1.0 / n);
            let mut bad = 0;
            for t in m.idx.chunks(3) {
                let (a, b, cc) = (
                    m.pos[t[0] as usize],
                    m.pos[t[1] as usize],
                    m.pos[t[2] as usize],
                );
                let fnrm = (b - a).cross(cc - a);
                let centre = (a + b + cc) * (1.0 / 3.0);
                if fnrm.dot(centre - c) < -1e-4 {
                    bad += 1;
                }
            }
            assert!(
                bad * 10 < m.triangle_count().max(1),
                "{name}: {bad} inward faces"
            );
        }
        // The torus is not convex: compare against its analytic normal.
        let t = MeshData::torus(3.0, 0.5, 8, 6, 0);
        assert!(closed(&t), "torus not closed");
        for tri in t.idx.chunks(3) {
            let (a, b, c) = (
                t.pos[tri[0] as usize],
                t.pos[tri[1] as usize],
                t.pos[tri[2] as usize],
            );
            let fnrm = (b - a).cross(c - a);
            let p = (a + b + c) * (1.0 / 3.0);
            let ring = v3(p.x, 0.0, p.z).norm() * 3.0;
            assert!(fnrm.dot(p - ring) > 0.0, "torus face inward");
        }
    }

    #[test]
    fn mirror_transform_keeps_outward_winding() {
        let m = MeshData::cuboid(2.0, 2.0, 2.0, 0);
        let mirrored = m.clone().transform(&Xf::new(
            super::super::math3::M3::scale(v3(-1.0, 1.0, 1.0)),
            V3::ZERO,
        ));
        assert!(closed(&mirrored));
        // First face (+Z) still faces +Z after mirroring in X.
        let t = &mirrored.idx[0..3];
        let (a, b, c) = (
            mirrored.pos[t[0] as usize],
            mirrored.pos[t[1] as usize],
            mirrored.pos[t[2] as usize],
        );
        assert!((b - a).cross(c - a).z > 0.0);
    }
}
