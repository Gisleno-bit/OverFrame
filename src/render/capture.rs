//! `overframe --capture <dir>`: the fixed-camera evidence suite of
//! `docs/art/EXCHANGE.md`, rendered by the *production* renderer
//! (`Scene3D`) into off-screen textures and written as PNG/GIF, plus the
//! runtime exports. Every image is labelled `game3d`; nothing here uses the
//! 2D diagnostic rasteriser.
//!
//! Cameras, resolutions, poses and fixtures come from
//! `docs/art/capture-suite.json` and `docs/art/fixtures/*.json`, so a
//! reviewer can compare two runs pixel for pixel. The tool writes
//! `capture-index.json` describing every file (size, tick, camera, label);
//! CI adds SHA-256s and check results to make the manifest.

use super::scene3d::{FixedCam, ModelLook};
use super::{App, MqPainter, Screen};
use crate::export::{self, Fixture};
use crate::model::math3::{v3, Xf, M3, V3};
use crate::sim::fighter::{Fighter, State};
use crate::sim::roster::CharacterId;
use crate::sim::Vec2;
use crate::viz::{font, Color as VColor, SceneOpts};
use image::{ImageBuffer, Rgba, RgbaImage};
use macroquad::prelude::*;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const SUITE_JSON: &str = include_str!("../../docs/art/capture-suite.json");

/// How `root_yaw_deg` is applied (same rotation the game uses for facing).
pub const YAW_CONVENTION: &str = "root rotated about +Y by yaw, right-handed (model::math3::M3::rot_y); yaw 0 = fighter faces +X (screen right), 90 = back to camera, 180 = faces -X, 270 = face to camera";

/// Command-line options for a capture run.
#[derive(Clone, Debug, Default)]
pub struct CaptureOpts {
    pub out: PathBuf,
    /// Full source SHA to stamp into the exports (`unknown` if not given).
    pub source_sha: String,
    /// Only produce entries whose path contains this text (debugging).
    pub only: Option<String>,
}

// ----------------------------------------------------------------- suite

#[derive(Deserialize, Debug, Clone)]
pub struct Suite {
    pub schema_version: u32,
    pub id: String,
    pub units: String,
    pub kind_required: String,
    pub warmup_ticks: u32,
    pub turnaround: TurnaroundSpec,
    pub combat: CombatSpec,
    pub gif: GifSpec,
    pub runtime_frame_data: String,
    #[serde(default = "default_characters")]
    pub characters: Vec<String>,
    #[serde(default = "default_contact_actions")]
    pub contact_actions: Vec<ContactAction>,
    #[serde(default)]
    pub fixtures: Fixtures,
}

fn default_characters() -> Vec<String> {
    vec!["kestrel".into()]
}
fn default_contact_actions() -> Vec<ContactAction> {
    ["jab", "ftilt", "fsmash", "nair", "fair", "dair"]
        .iter()
        .map(|a| ContactAction {
            action_id: a.to_string(),
            variant_id: if *a == "fsmash" {
                "cstick"
            } else if a.ends_with("air") {
                "fullhop"
            } else {
                "ground"
            }
            .to_string(),
        })
        .collect()
}

#[derive(Deserialize, Debug, Clone)]
pub struct ContactAction {
    pub action_id: String,
    pub variant_id: String,
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct Fixtures {
    #[serde(default = "idle_fixture")]
    pub idle: String,
    #[serde(default = "combat_fixture")]
    pub combat: String,
}
fn idle_fixture() -> String {
    "docs/art/fixtures/idle-v1.json".into()
}
fn combat_fixture() -> String {
    "docs/art/fixtures/combat-v1.json".into()
}

#[derive(Deserialize, Debug, Clone)]
pub struct TurnaroundSpec {
    pub output: [u32; 2],
    pub grid: [u32; 2],
    pub eye: [f32; 3],
    pub target: [f32; 3],
    pub up: [f32; 3],
    pub orthographic_height: f32,
    pub root_yaw_deg: Vec<f32>,
    pub pose: String,
    pub lag_enabled: bool,
}

#[derive(Deserialize, Debug, Clone)]
pub struct CombatSpec {
    pub output: [u32; 2],
    pub eye: [f32; 3],
    pub target: [f32; 3],
    pub up: [f32; 3],
    pub orthographic_width: f32,
    pub players: Vec<CombatPlayer>,
    pub fixture_tick: u32,
    #[serde(default = "depth_eye")]
    pub depth_eye: [f32; 3],
}
fn depth_eye() -> [f32; 3] {
    [0.0, 156.5, 489.0]
}

#[derive(Deserialize, Debug, Clone)]
pub struct CombatPlayer {
    pub root: [f32; 3],
    pub facing: f32,
}

#[derive(Deserialize, Debug, Clone)]
pub struct GifSpec {
    pub output: [u32; 2],
    pub simulation_hz: u32,
    pub duration_ticks: u32,
    pub sample_stride: u32,
    pub playback_fps: u32,
}

pub fn suite() -> Result<Suite, String> {
    serde_json::from_str(SUITE_JSON).map_err(|e| format!("capture-suite.json: {e}"))
}

// ----------------------------------------------------------------- index

#[derive(Serialize, Debug, Clone)]
pub struct FileEntry {
    pub path: String,
    pub kind: &'static str,
    pub width: u32,
    pub height: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tick: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fixture: Option<String>,
    pub camera: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Serialize, Debug, Clone, Default)]
pub struct CaptureIndex {
    pub schema_version: u32,
    pub tool: String,
    pub source_sha: String,
    pub suite_id: String,
    pub renderer: String,
    pub window: [u32; 2],
    pub outline_widths: Vec<(String, f32)>,
    pub triangles: serde_json::Value,
    pub files: Vec<FileEntry>,
    pub skipped: Vec<(String, String)>,
    pub frame_data_cases: Vec<export::CaseReport>,
}

// ----------------------------------------------------------------- helpers

fn cam(eye: [f32; 3], target: [f32; 3], ortho_height: f32) -> FixedCam {
    FixedCam {
        eye: v3(eye[0], eye[1], eye[2]),
        target: v3(target[0], target[1], target[2]),
        ortho_height,
    }
}

fn cam_json(c: &FixedCam, w: u32, h: u32) -> serde_json::Value {
    serde_json::json!({
        "projection": "orthographic",
        "eye": [c.eye.x, c.eye.y, c.eye.z],
        "target": [c.target.x, c.target.y, c.target.z],
        "up": [0, 1, 0],
        "orthographic_height": c.ortho_height,
        "orthographic_width": c.ortho_height * w as f32 / h as f32,
        "pixels_per_unit": h as f32 / c.ortho_height,
    })
}

fn rt(w: u32, h: u32) -> RenderTarget {
    let t = render_target_ex(
        w,
        h,
        RenderTargetParams {
            sample_count: 1,
            depth: true,
        },
    );
    t.texture.set_filter(FilterMode::Nearest);
    t
}

/// Read a render target back as a top-down RGBA image.
fn read_rt(t: &RenderTarget) -> RgbaImage {
    // SAFETY: main thread, between draw calls — the documented way to reach
    // the miniquad context.
    unsafe {
        get_internal_gl().flush();
    }
    let img = t.texture.get_texture_data();
    let (w, h) = (img.width as u32, img.height as u32);
    let mut out: RgbaImage = ImageBuffer::new(w, h);
    for y in 0..h {
        let src = (h - 1 - y) as usize * w as usize * 4;
        for x in 0..w as usize {
            let i = src + x * 4;
            out.put_pixel(
                x as u32,
                y,
                Rgba([img.bytes[i], img.bytes[i + 1], img.bytes[i + 2], 255]),
            );
        }
    }
    out
}

fn blit(dst: &mut RgbaImage, src: &RgbaImage, x0: u32, y0: u32) {
    for (x, y, p) in src.enumerate_pixels() {
        if x0 + x < dst.width() && y0 + y < dst.height() {
            dst.put_pixel(x0 + x, y0 + y, *p);
        }
    }
}

fn save(dir: &Path, rel: &str, img: &RgbaImage) -> Result<(), String> {
    let p = dir.join(rel);
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    img.save(&p).map_err(|e| format!("{}: {e}", p.display()))
}

/// A synthetic fighter at the origin, standing, for model captures.
fn model_fighter(id: CharacterId, palette: u8) -> Fighter {
    let mut f = Fighter::new(id.data(), 0, Vec2::ZERO);
    f.palette = palette;
    f.facing = 1.0;
    f.grounded = true;
    f.set_state_pub(State::Stand);
    f
}

fn renderer_string() -> String {
    // SAFETY: as above.
    let info = unsafe { get_internal_gl().quad_context.info() };
    format!(
        "macroquad 0.4 / miniquad {:?}; GL version string: {}",
        info.backend, info.gl_version_string
    )
}

/// The pixel font is ASCII-only: fold accents for labels.
fn ascii_fold(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            'á' | 'à' | 'ä' | 'â' => 'a',
            'é' | 'è' | 'ë' | 'ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' => 'o',
            'ú' | 'ù' | 'ü' | 'û' => 'u',
            'ñ' => 'n',
            'Á' | 'À' | 'Ä' | 'Â' => 'A',
            'É' | 'È' | 'Ë' | 'Ê' => 'E',
            'Í' | 'Ì' | 'Ï' | 'Î' => 'I',
            'Ó' | 'Ò' | 'Ö' | 'Ô' => 'O',
            'Ú' | 'Ù' | 'Ü' | 'Û' => 'U',
            'Ñ' => 'N',
            c if c.is_ascii() => c,
            _ => '?',
        })
        .collect()
}

fn label_text(text: &str, x: f32, y: f32, scale: f32) {
    let mut p = MqPainter;
    font::draw_text(
        &mut p,
        &ascii_fold(text),
        x,
        y,
        scale,
        VColor::rgba(240, 236, 220, 235),
    );
}

// ----------------------------------------------------------------- runner

/// Produce the whole suite under `opts.out`. Returns the index it wrote.
pub(super) async fn run(app: &mut App, opts: &CaptureOpts) -> Result<CaptureIndex, String> {
    let suite = suite()?;
    if suite.kind_required != "game3d" {
        return Err(format!(
            "suite requires `{}` captures; this tool produces game3d",
            suite.kind_required
        ));
    }
    let out = opts.out.clone();
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let idle = Fixture::parse(export::FIXTURE_IDLE)?;
    let combat = Fixture::parse(export::FIXTURE_COMBAT)?;
    let want = |name: &str| opts.only.as_deref().map_or(true, |o| name.contains(o));

    let mut index = CaptureIndex {
        schema_version: 1,
        tool: format!("overframe {} --capture", env!("CARGO_PKG_VERSION")),
        source_sha: opts.source_sha.clone(),
        suite_id: suite.id.clone(),
        renderer: renderer_string(),
        window: [screen_width() as u32, screen_height() as u32],
        ..Default::default()
    };

    // Stage light + geometry for every capture.
    app.scene.reset();
    app.scene.prepare_stage(&idle.initial_state());
    next_frame().await;

    // ---------------------------------------------------------- characters
    let ta = &suite.turnaround;
    let cell_w = ta.output[0] / ta.grid[0];
    let cell_h = ta.output[1] / ta.grid[1];
    let ta_cam = cam(ta.eye, ta.target, ta.orthographic_height);
    index
        .outline_widths
        .push(("turnaround".into(), ta_cam.outline_width(cell_h as f32)));

    for name in &suite.characters {
        let Some(id) = export::parse_character(name) else {
            index
                .skipped
                .push((name.clone(), "unknown character".into()));
            continue;
        };
        let rest_pose = |app: &App| app.scene.model_rig(id).rest_pose();
        let cell = rt(cell_w, cell_h);

        // Turnaround + silhouette: rest pose, 4 yaws, lag off.
        for (file, look) in [
            (
                "turnaround",
                ModelLook {
                    silhouette: false,
                    backdrop: true,
                    hitboxes: false,
                },
            ),
            (
                "silhouette",
                ModelLook {
                    silhouette: true,
                    backdrop: false,
                    hitboxes: false,
                },
            ),
        ] {
            let rel = format!("characters/{name}/{file}.png");
            if !want(&rel) {
                continue;
            }
            let mut sheet: RgbaImage = ImageBuffer::from_pixel(
                ta.output[0],
                ta.output[1],
                if look.silhouette {
                    Rgba([255, 255, 255, 255])
                } else {
                    Rgba([12, 12, 20, 255])
                },
            );
            let f = model_fighter(id, 0);
            app.scene.lag_enabled = ta.lag_enabled;
            for (i, yaw) in ta.root_yaw_deg.iter().enumerate() {
                let pose = rest_pose(app);
                let root = Xf::new(M3::rot_y(*yaw), V3::ZERO);
                app.scene
                    .draw_model_fixed(&f, 0, &pose, &root, &ta_cam, &cell, look);
                next_frame().await;
                let img = read_rt(&cell);
                let (gx, gy) = (i as u32 % ta.grid[0], i as u32 / ta.grid[0]);
                blit(&mut sheet, &img, gx * cell_w, gy * cell_h);
            }
            app.scene.lag_enabled = true;
            save(&out, &rel, &sheet)?;
            index.files.push(FileEntry {
                path: rel,
                kind: "game3d",
                width: ta.output[0],
                height: ta.output[1],
                tick: None,
                fixture: None,
                camera: serde_json::json!({
                    "per_cell": cam_json(&ta_cam, cell_w, cell_h),
                    "cells": ta.root_yaw_deg.iter().map(|y| serde_json::json!({"root_yaw_deg": y})).collect::<Vec<_>>(),
                    "pose": "rest", "lag_enabled": ta.lag_enabled,
                    "yaw_convention": YAW_CONVENTION,
                }),
                note: Some(if look.silhouette {
                    "black object, white ground, unlit, no outline".into()
                } else {
                    "studio backdrop, stage fighter light".into()
                }),
            });
        }

        // Palettes: 3×2 cells, same camera and rest pose, yaw 0.
        let rel = format!("characters/{name}/palettes.png");
        if want(&rel) {
            let n = crate::sim::roster::PALETTES as u32;
            let (cols, rows) = (3u32, 2u32);
            let mut sheet: RgbaImage =
                ImageBuffer::from_pixel(cols * cell_w, rows * cell_h, Rgba([12, 12, 20, 255]));
            app.scene.lag_enabled = false;
            let mut names = Vec::new();
            for pi in 0..n.min(cols * rows) {
                let f = model_fighter(id, pi as u8);
                let pose = rest_pose(app);
                let root = Xf::IDENTITY;
                app.scene.draw_model_fixed(
                    &f,
                    0,
                    &pose,
                    &root,
                    &ta_cam,
                    &cell,
                    ModelLook {
                        silhouette: false,
                        backdrop: true,
                        hitboxes: false,
                    },
                );
                set_camera(&super::scene3d::rt_camera_2d(&cell));
                label_text(
                    &format!("{pi} {}", crate::model::palettes::get(id, pi as u8).name),
                    12.0,
                    12.0,
                    2.0,
                );
                set_default_camera();
                next_frame().await;
                let img = read_rt(&cell);
                blit(&mut sheet, &img, (pi % cols) * cell_w, (pi / cols) * cell_h);
                names.push(crate::model::palettes::get(id, pi as u8).name);
            }
            app.scene.lag_enabled = true;
            save(&out, &rel, &sheet)?;
            index.files.push(FileEntry {
                path: rel,
                kind: "game3d",
                width: cols * cell_w,
                height: rows * cell_h,
                tick: None,
                fixture: None,
                camera: serde_json::json!({
                    "per_cell": cam_json(&ta_cam, cell_w, cell_h),
                    "palette_indices": (0..n).collect::<Vec<_>>(),
                    "palette_names": names,
                    "pose": "rest", "root_yaw_deg": 0,
                }),
                note: None,
            });
        }

        // Combat size: the idle fixture at its capture tick, two cells at 72
        // and 120 px projected height, real stage behind.
        let rel = format!("characters/{name}/combat-size.png");
        if want(&rel) {
            let gs = idle.state_at(suite.combat.fixture_tick);
            let f = &gs.fighters[0];
            let height = f.character.height;
            let (cw, ch) = (320u32, 180u32);
            let sheet_w = cw * 2;
            let mut sheet: RgbaImage = ImageBuffer::new(sheet_w, ch);
            let cellrt = rt(cw, ch);
            let mut cells = Vec::new();
            for (k, px) in [72.0f32, 120.0].iter().enumerate() {
                let oh = height * ch as f32 / px;
                let c = FixedCam {
                    eye: v3(f.pos.x, f.pos.y + oh * 0.42, 120.0),
                    target: v3(f.pos.x, f.pos.y + oh * 0.42, 0.0),
                    ortho_height: oh,
                };
                app.scene.draw_fixed(
                    &gs,
                    &c,
                    &cellrt,
                    SceneOpts {
                        hud: false,
                        ..SceneOpts::default()
                    },
                );
                set_camera(&super::scene3d::rt_camera_2d(&cellrt));
                label_text(&format!("{px}px = {height}u"), 8.0, 8.0, 1.5);
                set_default_camera();
                next_frame().await;
                let img = read_rt(&cellrt);
                blit(&mut sheet, &img, k as u32 * cw, 0);
                cells.push(serde_json::json!({
                    "projected_height_px": px,
                    "camera": cam_json(&c, cw, ch),
                    "scale_px_per_unit": px / height,
                }));
            }
            save(&out, &rel, &sheet)?;
            index.files.push(FileEntry {
                path: rel,
                kind: "game3d",
                width: sheet_w,
                height: ch,
                tick: Some(suite.combat.fixture_tick as i64),
                fixture: Some(idle.id.clone()),
                camera: serde_json::json!({ "cells": cells, "player": 0 }),
                note: Some("idle fixture, real stage, no HUD".into()),
            });
        }

        // Contact sheets: three ticks of each listed action, hitboxes on.
        for ca in &suite.contact_actions {
            let rel = format!("characters/{name}/contact-{}.png", ca.action_id);
            if !want(&rel) {
                continue;
            }
            let Some(mid) = crate::sim::attacks::ALL_MOVES
                .iter()
                .copied()
                .find(|m| export::action_id(*m) == ca.action_id)
            else {
                index.skipped.push((rel, "unknown action".into()));
                continue;
            };
            let ticks = match export::run_case(id, mid, &ca.variant_id) {
                Ok(t) => t,
                Err(e) => {
                    index.skipped.push((rel, e));
                    continue;
                }
            };
            let first_active = ticks.iter().position(|t| t.row.hitbox_active);
            let last_active = ticks.iter().rposition(|t| t.row.hitbox_active);
            let (Some(fa), Some(la)) = (first_active, last_active) else {
                index
                    .skipped
                    .push((rel, "no active hitbox tick in export".into()));
                continue;
            };
            let picks = [
                (fa.saturating_sub(1), "last inactive"),
                (fa, "first active"),
                ((la + 1).min(ticks.len() - 1), "first after last active"),
            ];
            let (cw, ch) = (512u32, 512u32);
            let cellrt = rt(cw, ch);
            let mut sheet: RgbaImage = ImageBuffer::new(cw * 3, ch);
            let mut cells = Vec::new();
            for (k, (ti, what)) in picks.iter().enumerate() {
                let snap = &ticks[*ti];
                let f = &snap.state.fighters[0];
                // Centred a little ahead of the fighter so the front hitbox
                // is inside the cell (declared in capture-suite.json).
                let cx = f.pos.x + f.facing * 10.0;
                let c = FixedCam {
                    eye: v3(cx, f.pos.y + 22.0, 120.0),
                    target: v3(cx, f.pos.y + 22.0, 0.0),
                    ortho_height: 44.0,
                };
                app.scene.draw_fixed(
                    &snap.state,
                    &c,
                    &cellrt,
                    SceneOpts {
                        hud: false,
                        hitboxes: true,
                        ..SceneOpts::default()
                    },
                );
                set_camera(&super::scene3d::rt_camera_2d(&cellrt));
                let r = &snap.row;
                label_text(
                    &format!("{what}  tick {}  sf {}", r.tick_index, r.state_frame),
                    8.0,
                    8.0,
                    1.5,
                );
                let geo = match (r.center, r.radius) {
                    (Some(c), Some(rad)) => {
                        format!("{} r={rad:.2} @({:.2},{:.2})", r.hitbox_id, c.0, c.1)
                    }
                    _ => "no hitbox".into(),
                };
                label_text(&geo, 8.0, 30.0, 1.5);
                label_text(
                    &format!(
                        "hitlag {}  contact {}",
                        r.hitlag_remaining, r.contact_marker as u8
                    ),
                    8.0,
                    52.0,
                    1.5,
                );
                set_default_camera();
                next_frame().await;
                let img = read_rt(&cellrt);
                blit(&mut sheet, &img, k as u32 * cw, 0);
                cells.push(serde_json::json!({
                    "which": what,
                    "tick_index": r.tick_index,
                    "state_frame": r.state_frame,
                    "hitbox_id": r.hitbox_id,
                    "hitbox_active": r.hitbox_active,
                    "center": r.center.map(|c| [c.0, c.1]),
                    "radius": r.radius,
                    "hitlag_remaining": r.hitlag_remaining,
                    "camera": cam_json(&c, cw, ch),
                }));
            }
            save(&out, &rel, &sheet)?;
            index.files.push(FileEntry {
                path: rel,
                kind: "game3d",
                width: cw * 3,
                height: ch,
                tick: None,
                fixture: None,
                camera: serde_json::json!({
                    "action_id": ca.action_id, "variant_id": ca.variant_id,
                    "sample_phase": "after_step", "cells": cells,
                }),
                note: Some("values from the runtime exporter (frame-data.csv)".into()),
            });
        }
    }

    // ---------------------------------------------------------- scenes
    let cs = &suite.combat;
    let (sw, sh) = (cs.output[0], cs.output[1]);
    let scene_rt = rt(sw, sh);
    let ortho_h = cs.orthographic_width * sh as f32 / sw as f32;
    let fixed = cam(cs.eye, cs.target, ortho_h);
    let depth = cam(cs.depth_eye, cs.target, ortho_h);
    index
        .outline_widths
        .push(("scenes".into(), fixed.outline_width(sh as f32)));

    let rel = "scenes/lattice-fixed.png";
    if want(rel) {
        let mut gs = idle.initial_state();
        for f in &mut gs.fighters {
            f.set_state_pub(State::Dead);
        }
        app.scene.draw_fixed(
            &gs,
            &fixed,
            &scene_rt,
            SceneOpts {
                hud: false,
                ..SceneOpts::default()
            },
        );
        next_frame().await;
        save(&out, rel, &read_rt(&scene_rt))?;
        index.files.push(FileEntry {
            path: rel.into(),
            kind: "game3d",
            width: sw,
            height: sh,
            tick: None,
            fixture: None,
            camera: cam_json(&fixed, sw, sh),
            note: Some("no players".into()),
        });
    }

    for (rel, c, note) in [
        ("scenes/combat-fixed.png", &fixed, "idle fixture, HUD on"),
        (
            "scenes/combat-depth.png",
            &depth,
            "idle fixture, second angle",
        ),
    ] {
        if !want(rel) {
            continue;
        }
        let gs = idle.state_at(cs.fixture_tick);
        app.scene.draw_fixed(
            &gs,
            c,
            &scene_rt,
            SceneOpts {
                hud: true,
                ..SceneOpts::default()
            },
        );
        next_frame().await;
        save(&out, rel, &read_rt(&scene_rt))?;
        index.files.push(FileEntry {
            path: rel.into(),
            kind: "game3d",
            width: sw,
            height: sh,
            tick: Some(cs.fixture_tick as i64),
            fixture: Some(idle.id.clone()),
            camera: cam_json(c, sw, sh),
            note: Some(note.into()),
        });
    }

    // Combat GIF: warm the visuals for `warmup_ticks`, then sample.
    let rel = "scenes/combat.gif";
    if want(rel) {
        let g = &suite.gif;
        let (gw, gh) = (g.output[0], g.output[1]);
        let gif_rt = rt(gw, gh);
        let gif_cam = cam(
            cs.eye,
            cs.target,
            cs.orthographic_width * gh as f32 / gw as f32,
        );
        let mut gs = combat.initial_state();
        app.scene.reset();
        let path = out.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
        let file = std::fs::File::create(&path).map_err(|e| e.to_string())?;
        let mut enc = image::codecs::gif::GifEncoder::new_with_speed(file, 10);
        enc.set_repeat(image::codecs::gif::Repeat::Infinite)
            .map_err(|e| e.to_string())?;
        let delay = image::Delay::from_numer_denom_ms(1000, g.playback_fps.max(1));
        let mut frames = 0u32;
        let start = -(combat.warmup_ticks.max(suite.warmup_ticks) as i64);
        for t in start..(g.duration_ticks as i64) {
            gs.step(&combat.inputs_at(t));
            let draw_it = t < 0 || (t as u32) % g.sample_stride == 0;
            if !draw_it {
                continue;
            }
            app.scene.draw_fixed(
                &gs,
                &gif_cam,
                &gif_rt,
                SceneOpts {
                    hud: true,
                    ..SceneOpts::default()
                },
            );
            next_frame().await;
            if t >= 0 {
                let img = read_rt(&gif_rt);
                let frame = image::Frame::from_parts(img, 0, 0, delay);
                enc.encode_frame(frame).map_err(|e| e.to_string())?;
                frames += 1;
            }
        }
        drop(enc);
        index.files.push(FileEntry {
            path: rel.into(),
            kind: "game3d",
            width: gw,
            height: gh,
            tick: Some(0),
            fixture: Some(combat.id.clone()),
            camera: cam_json(&gif_cam, gw, gh),
            note: Some(format!(
                "{frames} frames: ticks 0..{} every {} at {} fps ({} Hz sim); warmup {} ticks drawn, not saved",
                g.duration_ticks, g.sample_stride, g.playback_fps, g.simulation_hz,
                combat.warmup_ticks.max(suite.warmup_ticks)
            )),
        });
    }

    // ---------------------------------------------------------- UI
    let rel = "ui/selection.png";
    if want(rel) {
        app.rules.p1 = CharacterId::Kestrel;
        app.rules.p1_pal = 0;
        app.rules.p2 = CharacterId::Kestrel;
        app.rules.p2_pal = 3;
        app.screen = Screen::LocalSetup;
        app.setup_idx = 0;
        // Two frames so the live previews have settled.
        for _ in 0..2 {
            app.draw();
            next_frame().await;
        }
        app.draw();
        // SAFETY: as above.
        unsafe {
            get_internal_gl().flush();
        }
        let img = get_screen_data();
        let (w, h) = (img.width as u32, img.height as u32);
        let mut out_img: RgbaImage = ImageBuffer::new(w, h);
        for y in 0..h {
            let src = (h - 1 - y) as usize * w as usize * 4;
            for x in 0..w as usize {
                let i = src + x * 4;
                out_img.put_pixel(
                    x as u32,
                    y,
                    Rgba([img.bytes[i], img.bytes[i + 1], img.bytes[i + 2], 255]),
                );
            }
        }
        save(&out, rel, &out_img)?;
        next_frame().await;
        index.files.push(FileEntry {
            path: rel.into(),
            kind: "game3d",
            width: w,
            height: h,
            tick: None,
            fixture: None,
            camera: serde_json::json!({
                "screen": "local versus setup (real menu)",
                "p1": {"character": "kestrel", "palette": 0},
                "p2": {"character": "kestrel", "palette": 3},
            }),
            note: Some(if (w, h) == (1920, 1080) {
                "window at 1920x1080".into()
            } else {
                format!("window came up at {w}x{h}, not 1920x1080")
            }),
        });
    }

    // ---------------------------------------------------------- runtime
    match export::write_runtime(&out, &opts.source_sha) {
        Ok(reports) => index.frame_data_cases = reports,
        Err(e) => index.skipped.push(("runtime".into(), e.to_string())),
    }
    let (models, stage_tris) = app.scene.triangle_budget();
    index.triangles = serde_json::json!({
        "characters": models.iter().map(|(n, t)| serde_json::json!({"character_id": n, "triangles": t})).collect::<Vec<_>>(),
        "stage_visible": stage_tris,
        "budget": {"character": 3000, "stage": 6000},
    });
    let text = serde_json::to_string_pretty(&index).map_err(|e| e.to_string())?;
    std::fs::write(out.join("capture-index.json"), text).map_err(|e| e.to_string())?;
    Ok(index)
}
