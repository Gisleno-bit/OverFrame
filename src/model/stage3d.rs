//! 3D stage models built from the simulation's stage data plus per-stage
//! decoration. Gameplay geometry (platform tops, ledges, blast zones) comes
//! straight from `sim::stage`, so what you stand on is exactly what you see;
//! everything else here is dressing.
//!
//! Slots (see [`palette`]): 0 slab, 1 slab side, 2 lip, 3 soft platform,
//! 4 accent, 5 glow (unlit), 6 decor light, 7 decor dark.

use super::lighting::{slot, Light, Palette};
use super::math3::{v3, Xf, M3, V3};
use super::mesh::MeshData;
use crate::sim::stage::{Stage, StageId};

/// Depth (Z extent) of the main stage slab and of soft platforms.
pub const SLAB_DEPTH: f32 = 84.0;
pub const SOFT_DEPTH: f32 = 34.0;
pub const SLAB_THICK: f32 = 26.0;

pub struct StageModel {
    /// Lit geometry near the play plane.
    pub near: MeshData,
    /// Distant silhouettes (rendered with fog toward the sky colour).
    pub far: MeshData,
    pub palette: Palette,
    pub light: Light,
    /// Vertical position of the "floor" plane (sea, ground, void) if any.
    pub floor_y: Option<f32>,
    pub look: Look,
}

/// The 3D art direction of a stage: sky gradient, material palette and the
/// texture style. Independent of the 2D `Theme` used by the classic view.
#[derive(Clone, Copy, Debug)]
pub struct Look {
    pub sky_top: [u8; 3],
    pub sky_bottom: [u8; 3],
    /// Horizon haze colour (fog target for distant geometry).
    pub haze: [u8; 3],
    pub palette: Palette,
    /// Key light direction (toward the light).
    pub light_dir: V3,
    pub key: [f32; 3],
    /// Surface texture style for the slab (see `TexStyle`).
    pub tex: TexStyle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TexStyle {
    /// Machined panels with seams.
    Panels,
    /// Sandstone: soft noise and strata lines.
    Stone,
    /// Basalt: coarse noise and cracks.
    Basalt,
}

/// The look of each stage.
pub fn look(id: StageId) -> Look {
    match id {
        StageId::Lattice => Look {
            sky_top: [10, 10, 26],
            sky_bottom: [66, 44, 104],
            haze: [70, 54, 110],
            palette: Palette::new(
                "The Lattice",
                [126, 134, 178],
                [70, 74, 112],
                [170, 226, 255],
                [116, 138, 214],
                [46, 48, 74],
                [150, 220, 255],
                [92, 96, 140],
                [52, 54, 88],
            ),
            light_dir: v3(-0.35, 0.8, 0.6),
            key: [1.1, 1.08, 1.05],
            tex: TexStyle::Panels,
        },
        StageId::Meridian => Look {
            sky_top: [24, 34, 74],
            sky_bottom: [236, 150, 96],
            haze: [220, 150, 110],
            palette: Palette::new(
                "Meridian",
                [206, 170, 118],
                [150, 110, 72],
                [255, 214, 140],
                [172, 142, 104],
                [96, 68, 48],
                [255, 226, 160],
                [190, 134, 108],
                [130, 84, 84],
            ),
            light_dir: v3(0.45, 0.78, 0.5),
            key: [1.2, 1.0, 0.85],
            tex: TexStyle::Stone,
        },
        StageId::Tidegate => Look {
            sky_top: [12, 10, 34],
            sky_bottom: [78, 46, 96],
            haze: [80, 60, 110],
            palette: Palette::new(
                "Tidegate",
                [84, 80, 98],
                [52, 48, 64],
                [150, 240, 200],
                [110, 82, 122],
                [34, 30, 44],
                [180, 255, 225],
                [76, 66, 104],
                [40, 34, 60],
            ),
            light_dir: v3(-0.5, 0.7, 0.5),
            key: [0.95, 1.0, 1.1],
            tex: TexStyle::Basalt,
        },
    }
}

fn c(t: (u8, u8, u8)) -> [u8; 3] {
    [t.0, t.1, t.2]
}

fn mix(a: [u8; 3], b: [u8; 3], t: f32) -> [u8; 3] {
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t) as u8;
    [m(a[0], b[0]), m(a[1], b[1]), m(a[2], b[2])]
}

/// A material palette derived from the 2D theme (fallback for stages that
/// have no dedicated [`Look`]; the shipped stages all do).
#[allow(dead_code)]
pub fn palette(stage: &Stage) -> Palette {
    let th = &stage.theme;
    let slab = c(th.slab);
    let side = mix(slab, [0, 0, 0], 0.35);
    let lip = c(th.lip);
    let soft = c(th.soft);
    let glow = c(th.soft_glow);
    let bg = c(th.bg_bottom);
    Palette::new(
        stage.name,
        slab,
        side,
        lip,
        soft,
        mix(lip, slab, 0.5),
        glow,
        mix(bg, [255, 255, 255], 0.25),
        mix(bg, [0, 0, 0], 0.35),
    )
}

pub fn build(stage: &Stage) -> StageModel {
    let mut near = MeshData::new();
    let mut far = MeshData::new();

    // ---- gameplay geometry
    for p in &stage.platforms {
        let w = p.right - p.left;
        let cx = (p.left + p.right) * 0.5;
        if p.solid {
            // Slab with a bevelled edge, a lip strip along the front, and a
            // tapered keel below so it reads as a floating mass.
            near.append(
                &MeshData::bevel_box(w, SLAB_THICK, SLAB_DEPTH, 2.5, slot::PRIMARY).translate(v3(
                    cx,
                    p.y - SLAB_THICK * 0.5,
                    0.0,
                )),
            );
            near.append(
                &MeshData::cuboid(w - 4.0, 1.2, 2.2, slot::ACCENT).translate(v3(
                    cx,
                    p.y - 0.6,
                    SLAB_DEPTH * 0.5 + 0.6,
                )),
            );
            let keel_h = 70.0;
            near.append(
                &MeshData::cylinder(w * 0.12, w * 0.46, keel_h, 6, slot::SECONDARY)
                    .flat()
                    .translate(v3(cx, p.y - SLAB_THICK - keel_h * 0.5 + 1.0, 0.0)),
            );
            // Underside glow ring.
            near.append(
                &MeshData::torus(w * 0.2, 1.6, 16, 6, slot::GLOW).translate(v3(
                    cx,
                    p.y - SLAB_THICK - keel_h + 6.0,
                    0.0,
                )),
            );
        } else {
            near.append(
                &MeshData::bevel_box(w, 3.2, SOFT_DEPTH, 0.9, slot::SKIN).translate(v3(
                    cx,
                    p.y - 1.6,
                    0.0,
                )),
            );
            near.append(
                &MeshData::cuboid(w - 2.0, 0.8, 1.4, slot::GLOW).translate(v3(
                    cx,
                    p.y - 0.4,
                    SOFT_DEPTH * 0.5 + 0.4,
                )),
            );
            // Small emitter posts under each end.
            for x in [p.left + 3.0, p.right - 3.0] {
                near.append(
                    &MeshData::cylinder(0.9, 0.5, 6.0, 6, slot::DARK)
                        .flat()
                        .translate(v3(x, p.y - 6.0, 0.0)),
                );
            }
        }
    }
    // Ledge markers: a small glow nub at each grab point.
    for l in &stage.ledges {
        near.append(&MeshData::sphere(1.2, 3, 6, slot::GLOW).translate(v3(
            l.pos.x,
            l.pos.y - 1.5,
            SLAB_DEPTH * 0.5,
        )));
    }

    let floor_y;
    match stage.id {
        StageId::Lattice => {
            // The Lattice: a lattice of hexagonal pylons and rings suspended
            // in a violet void; a glowing grid far below.
            for (i, (x, z, h)) in [
                (-170.0f32, -140.0f32, 260.0f32),
                (150.0, -170.0, 300.0),
                (-40.0, -230.0, 340.0),
                (230.0, -110.0, 200.0),
                (-250.0, -90.0, 180.0),
                (60.0, -300.0, 420.0),
            ]
            .iter()
            .enumerate()
            {
                let s = if i % 2 == 0 { slot::LIGHT } else { slot::EXTRA };
                far.append(&MeshData::cylinder(9.0, 7.0, *h, 6, s).flat().translate(v3(
                    *x,
                    -60.0 + h * 0.5 - 90.0,
                    *z,
                )));
                far.append(&MeshData::torus(14.0, 1.4, 12, 5, slot::GLOW).translate(v3(
                    *x,
                    40.0 + i as f32 * 25.0,
                    *z,
                )));
            }
            for (x, y, z, r) in [
                (-120.0f32, 150.0f32, -220.0f32, 40.0f32),
                (140.0, 190.0, -260.0, 55.0),
                (20.0, 240.0, -320.0, 70.0),
            ] {
                far.append(
                    &MeshData::torus(r, 2.5, 24, 6, slot::EXTRA)
                        .transform(&Xf::new(M3::rot_x(70.0) * M3::rot_z(20.0), v3(x, y, z))),
                );
            }
            floor_y = Some(stage.blast_bottom + 30.0);
        }
        StageId::Meridian => {
            // Meridian: a flat arena at dawn over a desert plain; long low
            // dunes and a ring of distant monoliths.
            for i in 0..9 {
                let x = -420.0 + i as f32 * 105.0;
                let h = 60.0 + ((i * 37) % 7) as f32 * 22.0;
                let s = if i % 3 == 0 { slot::LIGHT } else { slot::EXTRA };
                far.append(&MeshData::bevel_box(22.0, h, 18.0, 3.0, s).translate(v3(
                    x,
                    stage.blast_bottom + 60.0 + h * 0.5,
                    -300.0 - (i % 2) as f32 * 80.0,
                )));
            }
            for i in 0..5 {
                let x = -380.0 + i as f32 * 190.0;
                far.append(
                    &MeshData::ellipsoid(140.0, 26.0, 60.0, 4, 10, slot::EXTRA)
                        .flat()
                        .translate(v3(
                            x,
                            stage.blast_bottom + 40.0,
                            -200.0 - (i % 2) as f32 * 60.0,
                        )),
                );
            }
            floor_y = Some(stage.blast_bottom + 40.0);
        }
        StageId::Tidegate => {
            // Tidegate: a sea gate — jagged basalt stacks and a broken arch
            // over dark water.
            for (x, z, h, r) in [
                (-230.0f32, -120.0f32, 180.0f32, 26.0f32),
                (-290.0, -200.0, 260.0, 34.0),
                (250.0, -140.0, 150.0, 22.0),
                (320.0, -240.0, 300.0, 40.0),
                (40.0, -330.0, 220.0, 30.0),
            ] {
                far.append(
                    &MeshData::cylinder(r * 0.6, r, h, 7, slot::EXTRA)
                        .flat()
                        .translate(v3(x, stage.blast_bottom + 30.0 + h * 0.5, z)),
                );
            }
            // Broken arch behind the stage.
            far.append(
                &MeshData::torus(150.0, 12.0, 20, 6, slot::LIGHT).transform(&Xf::new(
                    M3::rot_x(90.0),
                    v3(-40.0, stage.blast_bottom + 60.0, -180.0),
                )),
            );
            floor_y = Some(stage.blast_bottom + 30.0);
        }
    }

    // Floor plane (sea / ground / void grid).
    if let Some(y) = floor_y {
        far.append(
            &MeshData::ground_quad(2400.0, 1600.0, 24.0, slot::EXTRA).translate(v3(0.0, y, -300.0)),
        );
    }

    let lk = look(stage.id);
    let mut light = Light::default_for(
        (lk.sky_top[0], lk.sky_top[1], lk.sky_top[2]),
        (lk.sky_bottom[0], lk.sky_bottom[1], lk.sky_bottom[2]),
    );
    light.dir = lk.light_dir.norm();
    light.key = lk.key;
    // Stage surfaces: smoother shading than the fighters.
    light.bands = 0;
    light.rim = 0.12;
    let near = near.box_uv(1.0 / 24.0);
    StageModel {
        near,
        far,
        palette: lk.palette,
        light,
        floor_y,
        look: lk,
    }
}

/// A tileable RGBA texture for the stage surfaces (multiplied over the vertex
/// colour, so it only needs to carry brightness variation).
pub fn texture(style: TexStyle, size: usize) -> Vec<u8> {
    let mut out = vec![255u8; size * size * 4];
    let n = size as f32;
    // Tileable value noise from a few sine octaves (cheap, deterministic).
    let noise = |x: f32, y: f32| -> f32 {
        let mut v = 0.0;
        let mut amp = 0.5;
        let mut f = 1.0;
        for _ in 0..4 {
            v += amp
                * ((x * f * std::f32::consts::TAU + (y * f * 1.7).sin() * 2.0).sin()
                    * (y * f * std::f32::consts::TAU * 0.7 + (x * f * 2.3).cos() * 1.5).cos());
            amp *= 0.5;
            f *= 2.0;
        }
        v * 0.5 + 0.5
    };
    for y in 0..size {
        for x in 0..size {
            let u = x as f32 / n;
            let v = y as f32 / n;
            let base = noise(u, v);
            let b = match style {
                TexStyle::Panels => {
                    // 2×2 panels per tile with dark seams and a faint grain.
                    let seam_u = ((u * 2.0).fract() - 0.5).abs() > 0.47;
                    let seam_v = ((v * 2.0).fract() - 0.5).abs() > 0.47;
                    let seam = if seam_u || seam_v { 0.72 } else { 1.0 };
                    seam * (0.92 + 0.10 * base)
                }
                TexStyle::Stone => {
                    // Horizontal strata + soft mottling.
                    let strata =
                        0.94 + 0.06 * ((v * 6.0 + base * 0.8) * std::f32::consts::TAU).sin();
                    strata * (0.9 + 0.12 * base)
                }
                TexStyle::Basalt => {
                    let crack = if base > 0.78 { 0.7 } else { 1.0 };
                    crack * (0.84 + 0.2 * base)
                }
            };
            let i = (y * size + x) * 4;
            let c = (b.clamp(0.0, 1.0) * 255.0) as u8;
            out[i] = c;
            out[i + 1] = c;
            out[i + 2] = c;
            out[i + 3] = 255;
        }
    }
    out
}

/// Fog factor for a point (0 = none, 1 = fully the sky colour), by depth.
pub fn fog(p: V3) -> f32 {
    let d = (-p.z - 60.0).max(0.0);
    (d / 420.0).clamp(0.0, 0.85)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stages_build_with_bounded_geometry() {
        for id in StageId::ALL {
            let st = Stage::by_id(id);
            let m = build(&st);
            assert!(m.near.triangle_count() > 50);
            assert!(
                m.near.triangle_count() + m.far.triangle_count() < 60_000,
                "{:?}",
                id
            );
            // The main platform top must be at the sim's platform height.
            let (lo, hi) = m.near.bounds();
            let main = st.main();
            // Highest vertex over the main slab's footprint that is not a
            // soft platform (those sit above): must be exactly the slab top.
            let top = m
                .near
                .pos
                .iter()
                .filter(|p| p.x >= main.left && p.x <= main.right && p.y <= main.y + 0.5)
                .map(|p| p.y)
                .fold(f32::MIN, f32::max);
            assert!(
                (top - main.y).abs() < 0.01,
                "{:?} top {} vs {}",
                id,
                top,
                main.y
            );
            assert!(lo.x <= main.left + 0.5 && hi.x >= main.right - 0.5);
            assert!(lo.y < main.y - SLAB_THICK);
        }
    }
}
