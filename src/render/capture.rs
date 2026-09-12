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

use super::scene3d::{ContactQuery, Diagnostics, FixedCam, ModelLook};
use super::{App, MqPainter, Screen};
use crate::capture_suite::{self, CamRule, Suite};
use crate::export::{self, Fixture};
use crate::model::math3::{v3, Xf, M3, V3};
use crate::sim::fighter::{Fighter, State};
use crate::sim::roster::CharacterId;
use crate::sim::Vec2;
use crate::viz::{font, Color as VColor, SceneOpts};
use image::{ImageBuffer, Rgba, RgbaImage};
use macroquad::prelude::*;
use serde::Serialize;
use std::path::{Path, PathBuf};

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
    /// Characters the suite captures, and every contact sheet each one is
    /// expected to have (so the manifest can check coverage without
    /// re-deriving the naming rule).
    pub characters: Vec<String>,
    pub contact_expected: Vec<ContactExpected>,
    /// Result of re-rendering the first contact case in isolation at the
    /// end of the run and comparing pixels with the sheet produced mid-run.
    pub isolation_check: Option<serde_json::Value>,
}

#[derive(Serialize, Debug, Clone)]
pub struct ContactExpected {
    pub character_id: String,
    pub action_id: String,
    pub variant_id: String,
    pub primary: String,
    pub wide: String,
    /// `hitbox`, `projectile` or `throw_no_hitbox` — what the cells show.
    pub evidence_kind: &'static str,
    /// Does this action define a fighter hitbox of its own at all? False
    /// for throws and for actions declared `no_melee` in the move tables
    /// (Kestrel's `special_n`, whose only causal event is the shot it
    /// releases). The evidence validator reads this instead of assuming
    /// every projectile action also swings, so a supplement is demanded
    /// only where the simulation really has something to show.
    pub fighter_hitbox: bool,
}

// ----------------------------------------------------------------- helpers

fn cam(eye: [f32; 3], target: [f32; 3], ortho_height: f32) -> FixedCam {
    FixedCam {
        eye: v3(eye[0], eye[1], eye[2]),
        target: v3(target[0], target[1], target[2]),
        ortho_height,
        perspective: false,
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

/// Render targets by size, created once and kept alive for the whole run
/// so no GPU pass or texture is deleted (and its id possibly reused) while
/// draw calls may still reference it.
#[derive(Default)]
struct Targets {
    map: std::collections::HashMap<(u32, u32), RenderTarget>,
}

impl Targets {
    fn get(&mut self, w: u32, h: u32) -> RenderTarget {
        self.map
            .entry((w, h))
            .or_insert_with(|| {
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
            })
            .clone()
    }
}

/// Submit every queued draw call now (before a camera switch or a
/// readback), so nothing can be reordered across render passes.
fn flush_gl() {
    // SAFETY: main thread, between draw calls — the documented way to reach
    // the miniquad context.
    unsafe {
        get_internal_gl().flush();
    }
}

/// Read a render target back as a top-down RGBA image.
fn read_rt(t: &RenderTarget) -> RgbaImage {
    flush_gl();
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

/// Write a PNG with the slowest/smallest deflate setting (the default
/// encoder settings produce files 3–4× larger, which matters against the
/// per-run size budget of EXCHANGE.md).
fn save(dir: &Path, rel: &str, img: &RgbaImage) -> Result<(), String> {
    use image::codecs::png::{CompressionType, FilterType, PngEncoder};
    use image::ImageEncoder;
    let p = dir.join(rel);
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let file = std::fs::File::create(&p).map_err(|e| format!("{}: {e}", p.display()))?;
    let w = std::io::BufWriter::new(file);
    let enc = PngEncoder::new_with_quality(w, CompressionType::Best, FilterType::Adaptive);
    enc.write_image(
        img.as_raw(),
        img.width(),
        img.height(),
        image::ExtendedColorType::Rgba8,
    )
    .map_err(|e| format!("{}: {e}", p.display()))
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

/// Which real runtime event a contact sheet is about.
///
/// An action normally has one, and `Declared` is the suite's own choice of
/// cells for it. The projectile action has two — the emission and the
/// move's own fighter hitbox — so it also gets a `FighterHitbox` pair, on
/// the same declared cameras, rather than letting the emission exception
/// hide a real hit region.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SheetEvent {
    Declared,
    FighterHitbox,
}

/// Which sampled ticks make the wide contact sheet, by index into the
/// case's ticks — four cells: context, anchor (first active / spawn /
/// throw start), a later real event (last active / in flight / release)
/// and the tick after. Labels come from the suite (its `cells` or the
/// declared exceptions), so the caption words are the reviewer's.
fn wide_picks(
    ticks: &[export::CaseTick],
    mid: crate::sim::attacks::MoveId,
    suite: &Suite,
    event: SheetEvent,
) -> Result<Vec<(usize, String)>, String> {
    let last = ticks.len().saturating_sub(1);
    let x = &suite.contact_exceptions;
    if export::action_spawns_projectile(mid) && event == SheetEvent::Declared {
        let fp = ticks
            .iter()
            .position(|t| t.projectile.is_some())
            .ok_or("no projectile appeared in export")?;
        let lp = ticks
            .iter()
            .rposition(|t| t.projectile.is_some())
            .unwrap_or(fp);
        let idx = [fp.saturating_sub(1), fp, (fp + 5).min(lp), last];
        return Ok(idx
            .iter()
            .zip(&x.projectile_wide)
            .map(|(i, l)| (*i, l.clone()))
            .collect());
    }
    if !export::action_has_hitbox(mid) {
        let ts = ticks
            .iter()
            .position(|t| t.label == "throw" || t.label == "release")
            .ok_or("throw never started in export")?;
        let rel = ticks
            .iter()
            .position(|t| t.label == "release")
            .ok_or("no release tick in export (no fighter hitbox by design)")?;
        let idx = [ts.saturating_sub(1), ts, rel, last];
        return Ok(idx
            .iter()
            .zip(&x.throw_wide)
            .map(|(i, l)| (*i, l.clone()))
            .collect());
    }
    let fa = ticks
        .iter()
        .position(|t| t.row.hitbox_active)
        .ok_or("no active hitbox tick in export")?;
    // Last active includes the late window: `hitbox_active` is true for
    // clean and late ticks alike.
    let la = ticks
        .iter()
        .rposition(|t| t.row.hitbox_active)
        .unwrap_or(fa);
    let idx = [fa.saturating_sub(1), fa, la, (la + 1).min(last)];
    Ok(idx
        .iter()
        .zip(&suite.contact_wide_camera.cells)
        .map(|(i, l)| (*i, l.clone()))
        .collect())
}

/// The three primary cells EXCHANGE.md fixes (1536×512): context, first
/// active, first tick after the *last* active — i.e. the wide picks minus
/// the "later" column, relabelled from the suite's primary lists.
fn primary_picks(
    wide: &[(usize, String)],
    mid: crate::sim::attacks::MoveId,
    suite: &Suite,
    event: SheetEvent,
) -> Vec<(usize, String)> {
    let labels: &Vec<String> =
        if export::action_spawns_projectile(mid) && event == SheetEvent::Declared {
            &suite.contact_exceptions.projectile_primary
        } else if !export::action_has_hitbox(mid) {
            &suite.contact_exceptions.throw_primary
        } else {
            &suite.contact_camera.cells
        };
    [0usize, 1, 3]
        .iter()
        .zip(labels)
        .map(|(k, l)| (wide[*k].0, l.clone()))
        .collect()
}

/// Overlay inputs for one contact cell (what to draw and what to measure).
fn cell_query(snap: &export::CaseTick, mid: crate::sim::attacks::MoveId) -> Diagnostics {
    let r = &snap.row;
    Diagnostics {
        capsules: true,
        hitbox: r.center.map(|c| (c.0, c.1, r.radius.unwrap_or(0.0))),
        projectile: snap
            .projectile
            .as_ref()
            .and_then(|p| p.center.map(|c| (c.0, c.1, p.radius.unwrap_or(0.0)))),
        contact: Some(ContactQuery {
            port: 0,
            action: mid,
            hitbox: r
                .center
                .map(|c| (r.hitbox_id.clone(), [c.0, c.1], r.radius.unwrap_or(0.0))),
        }),
        // The caption's "support" line and the JSON's `foot_support` are
        // always the measured fighter's own feet (port 0), never whichever
        // fighter's DiagOut happened to come back non-empty last -- with the
        // opposing Boulder fixture alive for every contact-case run, "last"
        // used to mean port 1 (see `select_diag`'s doc comment).
        support_port: Some(0),
    }
}

/// Caption lines and index metadata for one drawn contact cell, from the
/// measurement taken on the pose that was actually drawn.
fn cell_report(
    snap: &export::CaseTick,
    what: &str,
    measure: Option<&crate::model::contact::ContactMeasure>,
    support: &[crate::model::contact::FootSupport],
    replayed: usize,
) -> (Vec<String>, serde_json::Value) {
    let f = &snap.state.fighters[0];
    let r = &snap.row;
    let proj = snap
        .projectile
        .as_ref()
        .and_then(|p| p.center.map(|c| (c.0, c.1, p.radius.unwrap_or(0.0))));
    // Compact rows, each measured against the cell width when it is drawn
    // (`caption_block`): the event on one row, the runtime phase/tick/state
    // on another, the measurement on its own. The full metadata stays in
    // the JSON below, which is where the numbers are read from anyway.
    let mut caption = vec![
        what.to_string(),
        format!(
            "{} t{} sf{} [{}]",
            r.sample_phase, r.tick_index, r.state_frame, snap.label
        ),
    ];
    caption.push(match (r.center, r.radius) {
        (Some(c), Some(rad)) => format!("hb {} r{rad:.2} ({:.1},{:.1})", r.hitbox_id, c.0, c.1),
        _ => match proj {
            Some((x, y, rad)) => format!("proj r{rad:.2} ({x:.1},{y:.1})"),
            None => "no runtime hitbox this tick".into(),
        },
    });
    if let Some(m) = measure {
        match m.signed_separation {
            Some(sep) => {
                caption.push(format!(
                    "{} sep {sep:+.2} mesh {:.2}",
                    m.piece_ids.join("+"),
                    m.mesh_distance.unwrap_or(f32::NAN)
                ));
                caption.push(format!(
                    "vtx {:.2} reach {:+.2}",
                    m.vertex_distance.unwrap_or(f32::NAN),
                    m.tip_reach_x
                ));
            }
            None => caption.push(format!(
                "{} reach {:+.2}",
                m.piece_ids.join("+"),
                m.tip_reach_x
            )),
        }
        // A projectile action has no fighter hitbox to separate from, so
        // the row above says only "reach". The question that matters for it
        // is whether the piece's real surface meets the **emission region**
        // -- measured with the same projected triangles -- and the answer
        // goes on the image rather than only in the JSON, so a sheet can
        // never read as a contact it does not have.
        if let Some(e) = &m.emission {
            caption.push(format!(
                "emission r{:.2} ({:.1},{:.1}) sep {:+.3} mesh {:.3}",
                e.radius, e.center[0], e.center[1], e.signed_separation, e.mesh_distance
            ));
            caption.push(format!(
                "CONTACT {} (centroid {:.3}, not the test)",
                if e.signed_separation <= 0.0 {
                    "MET"
                } else {
                    "NOT MET"
                },
                e.centroid_distance
            ));
        }
    }
    let eased = measure.map(|m| m.eased_facing).unwrap_or(f.facing);
    caption.push(format!(
        "root({:.1},{:.1}) f{:+} d{eased:+.2} hl{} c{}",
        f.pos.x, f.pos.y, f.facing as i32, r.hitlag_remaining, r.contact_marker as u8
    ));
    // Support: how far the lowest foot vertex sits above the plane the
    // fighter stands on (negative = through the floor).
    let low = support
        .iter()
        .map(|s| s.support_distance)
        .fold(None, |a: Option<f32>, x| {
            Some(a.map_or(x, |v: f32| v.min(x)))
        });
    if let (true, Some(low)) = (f.grounded, low) {
        caption.push(format!("support {low:+.2}"));
    }
    let meta = serde_json::json!({
        "which": what,
        "label": snap.label,
        "sample_phase": r.sample_phase,
        "tick_index": r.tick_index,
        "state_frame": r.state_frame,
        "state": format!("{:?}", f.state),
        "root": [f.pos.x, f.pos.y],
        "facing": f.facing,
        "eased_facing_drawn": eased,
        "render_history": {"reset": true, "replayed_ticks": replayed},
        "hitbox_id": r.hitbox_id,
        "hitbox_active": r.hitbox_active,
        "center": r.center.map(|c| [c.0, c.1]),
        "radius": r.radius,
        "projectile": proj.map(|p| serde_json::json!({"center": [p.0, p.1], "radius": p.2})),
        "hitlag_remaining": r.hitlag_remaining,
        "contact_marker": r.contact_marker,
        "capsule": crate::model::contact::capsule(f),
        "contact_piece": measure,
        "grounded": f.grounded,
        "foot_support": support,
    });
    (caption, meta)
}

/// Reconstruct the renderer's history for `ticks[idx]`: forget everything,
/// then evaluate every earlier tick of the case in order (no drawing).
/// Deterministic, and independent of what was captured before.
fn reconstruct_history(app: &mut App, ticks: &[export::CaseTick], idx: usize) -> usize {
    app.scene.reset();
    app.scene.reset_render_history();
    for t in &ticks[..idx] {
        for f in &t.state.fighters {
            app.scene.evaluate_fighter(f, t.state.frame);
        }
    }
    idx
}

fn fixed_cam(rule: &CamRule, x: f32, y: f32, facing: f32) -> FixedCam {
    let (eye, target) = rule.resolve(x, y, facing);
    FixedCam {
        eye: v3(eye[0], eye[1], eye[2]),
        target: v3(target[0], target[1], target[2]),
        ortho_height: rule.orthographic_height,
        perspective: false,
    }
}

/// One facing row of a contact case: facing, its ticks, and the wide picks.
type FacingRow = (f32, Vec<export::CaseTick>, Vec<(usize, String)>);

/// The two sheets of one contact case, rendered but not yet saved.
#[derive(Default)]
struct ContactSheets {
    primary: Option<(RgbaImage, FileEntry)>,
    wide: Option<(RgbaImage, FileEntry)>,
}

/// Render the primary and/or wide contact sheet of one case. Self-contained:
/// every cell resets and replays the render history, so the result does not
/// depend on what was captured before (the isolation self-check at the end
/// of a run re-renders a case and compares pixels).
#[allow(clippy::too_many_arguments)]
async fn render_contact_case(
    app: &mut App,
    rts: &mut Targets,
    suite: &Suite,
    primary_rule: &CamRule,
    wide_rule: &CamRule,
    id: CharacterId,
    mid: crate::sim::attacks::MoveId,
    variant: &str,
    rel: &str,
    rel_wide: &str,
    want_primary: bool,
    want_wide: bool,
    event: SheetEvent,
) -> Result<ContactSheets, String> {
    let aid = export::action_id(mid);
    let pcell = suite.contact_camera.cell;
    let wcell = suite.contact_wide_camera.cell;
    let primary_rt = rts.get(pcell[0], pcell[1]);
    let wide_rt = rts.get(wcell[0], wcell[1]);
    // One run per facing row of the wide sheet; the primary sheet
    // uses the +1 run.
    let mut rows: Vec<FacingRow> = Vec::new();
    let mut failed: Option<String> = None;
    for facing in &suite.contact_wide_camera.rows_facing {
        match export::run_case_facing(id, mid, variant, *facing) {
            Ok(t) => match wide_picks(&t, mid, suite, event) {
                Ok(p) => rows.push((*facing, t, p)),
                Err(e) => failed = Some(e),
            },
            Err(e) => failed = Some(e),
        }
    }
    if let Some(e) = failed {
        return Err(e);
    }
    let Some(right) = rows.iter().find(|r| r.0 == 1.0) else {
        return Err("no facing +1 row declared".into());
    };
    let primary = primary_picks(&right.2, mid, suite, event);
    let right_ticks = right.1.clone();
    // A supplemental sheet says so in its own metadata; the cameras and
    // cell rules stay the suite's.
    let supplemental = event == SheetEvent::FighterHitbox;

    let mut out_sheets = ContactSheets::default();
    // --- primary sheet
    if want_primary {
        let (cw, ch) = (pcell[0], pcell[1]);
        let mut sheet: RgbaImage = ImageBuffer::new(cw * primary.len() as u32, ch);
        let mut cells = Vec::new();
        for (k, (ti, what)) in primary.iter().enumerate() {
            let snap = &right_ticks[*ti];
            let f = &snap.state.fighters[0];
            let c = fixed_cam(primary_rule, f.pos.x, f.pos.y, f.facing);
            let replayed = reconstruct_history(app, &right_ticks, *ti);
            let measured = app.scene.draw_fixed_diag(
                &snap.state,
                &c,
                &primary_rt,
                SceneOpts {
                    hud: false,
                    ..SceneOpts::default()
                },
                Some(&cell_query(snap, mid)),
            );
            let (caption, mut meta) = cell_report(
                snap,
                what,
                measured.contact.as_ref(),
                &measured.support,
                replayed,
            );
            flush_gl();
            set_camera(&super::scene3d::rt_camera_2d(&primary_rt));
            caption_block(&caption, cw as f32);
            flush_gl();
            set_default_camera();
            next_frame().await;
            let img = read_rt(&primary_rt);
            blit(&mut sheet, &img, k as u32 * cw, 0);
            meta["camera"] = cam_json(&c, cw, ch);
            cells.push(meta);
        }
        let entry = FileEntry {
            path: rel.to_string(),
            kind: "game3d",
            width: cw * primary.len() as u32,
            height: ch,
            tick: None,
            fixture: None,
            camera: serde_json::json!({
                "action_id": aid, "variant_id": variant, "facing": 1,
                "declared": "capture-suite.json#contact_camera",
                "event": if supplemental { "fighter_hitbox" } else { "declared" },
                "supplemental": supplemental,
                "cells_rule": suite.contact_camera.cells,
                "camera_rule": {"eye": suite.contact_camera.eye, "target": suite.contact_camera.target, "orthographic_height": suite.contact_camera.orthographic_height},
                "cells": cells,
            }),
            note: Some(if supplemental {
                "supplemental primary sheet for the move's own fighter hitbox (the projectile action has two real runtime events: the emission on the move's first frame and this hit region on its startup frame). Same declared contact_camera and cell rule; same overlays and measurement path".into()
            } else {
                "primary contact sheet; overlays: red = runtime hitbox, green = hurt capsules, orange = projectile, yellow cross = nearest point of the piece's XY-projected surface + line to the hitbox centre (signed_separation = that distance minus the radius), magenta = tip, white = joint, cyan = piece bounds; measured on the drawn pose after a history reset + replay".to_string()
            }),
        };
        out_sheets.primary = Some((sheet, entry));
    }

    // --- wide diagnostic sheet: one fixed camera per facing row
    if want_wide {
        let (cw, ch) = (wcell[0], wcell[1]);
        let ncols = suite.contact_wide_camera.grid[0];
        let nrows = suite.contact_wide_camera.grid[1];
        let mut sheet: RgbaImage = ImageBuffer::new(cw * ncols, ch * nrows);
        let mut row_meta = Vec::new();
        for (ri, (facing, ticks, picks)) in rows.iter().enumerate() {
            let anchor = &ticks[picks[1].0];
            let af = &anchor.state.fighters[0];
            let c = fixed_cam(wide_rule, af.pos.x, af.pos.y, *facing);
            let mut cells = Vec::new();
            for (k, (ti, what)) in picks.iter().enumerate() {
                let snap = &ticks[*ti];
                let replayed = reconstruct_history(app, ticks, *ti);
                let measured = app.scene.draw_fixed_diag(
                    &snap.state,
                    &c,
                    &wide_rt,
                    SceneOpts {
                        hud: false,
                        ..SceneOpts::default()
                    },
                    Some(&cell_query(snap, mid)),
                );
                let (caption, meta) = cell_report(
                    snap,
                    what,
                    measured.contact.as_ref(),
                    &measured.support,
                    replayed,
                );
                flush_gl();
                set_camera(&super::scene3d::rt_camera_2d(&wide_rt));
                caption_block(&caption, cw as f32);
                flush_gl();
                set_default_camera();
                next_frame().await;
                let img = read_rt(&wide_rt);
                blit(&mut sheet, &img, k as u32 * cw, ri as u32 * ch);
                cells.push(meta);
            }
            row_meta.push(serde_json::json!({
                "row": ri, "facing": facing,
                "camera": cam_json(&c, cw, ch),
                "anchor": {"which": picks[1].1, "tick_index": anchor.tick_index, "root": [af.pos.x, af.pos.y]},
                "cells": cells,
            }));
        }
        let entry = FileEntry {
            path: rel_wide.to_string(),
            kind: "game3d",
            width: cw * ncols,
            height: ch * nrows,
            tick: None,
            fixture: None,
            camera: serde_json::json!({
                "action_id": aid, "variant_id": variant,
                "declared": "capture-suite.json#contact_wide_camera",
                "event": if supplemental { "fighter_hitbox" } else { "declared" },
                "supplemental": supplemental,
                "camera_rule": {"eye": suite.contact_wide_camera.eye, "target": suite.contact_wide_camera.target, "orthographic_height": suite.contact_wide_camera.orthographic_height, "anchor": suite.contact_wide_camera.anchor, "rule": suite.contact_wide_camera.camera_rule},
                "cells_rule": suite.contact_wide_camera.cells,
                "rows": row_meta,
            }),
            note: Some(if supplemental {
                "supplemental wide sheet for the move's own fighter hitbox, both facings, on the declared contact_wide_camera: the projectile emission exception never hides this hit region, and the hand is measured against it here".to_string()
            } else {
                "wide diagnostic sheet (auxiliary): one fixed camera per facing row, 4 columns incl. the last active tick (late frames included); same overlays and measurement path as the primary sheet".to_string()
            }),
        };
        out_sheets.wide = Some((sheet, entry));
    }
    Ok(out_sheets)
}

/// The GIF format's native delay unit is centiseconds. The `image` crate
/// (0.25.10, `codecs/gif.rs::convert_frame`) gets there in two lossy
/// steps: `img_frame.delay().into_ratio().to_integer()` first truncates
/// the requested delay down to a whole **millisecond** (line 587), then
/// `frame_delay / 10` floors that millisecond count down to a whole
/// **centisecond** (line 598) -- so a requested playback fps only comes
/// back out intact when `1000 / fps` happens to already be an exact
/// multiple of 10 (20, 25, 50, 100 fps all round-trip cleanly; the
/// existing `gif` recipe's 20 fps is why this was never noticed before).
/// 60 fps is not one of those: `1000/60` truncates to 16ms, which floors
/// to 1 centisecond, so the encoded GIF actually plays at 100 fps, not
/// 60 -- roughly 1.67x too fast, not a rounding error. This computes the
/// *actual* achieved fps for a requested one, so captions and metadata
/// can report the real number instead of repeating the request as if the
/// encoder had honoured it. (A pure function of the request, so it is
/// tested directly without touching the encoder or a GPU context.)
fn gif_actual_fps(requested_fps: u32) -> f64 {
    let fps = requested_fps.max(1);
    let truncated_ms = 1000u32 / fps;
    let centis = truncated_ms / 10;
    if centis == 0 {
        // The encoder saturates a zero delay rather than erroring; most
        // viewers read a zero delay as "as fast as it can", which is not
        // a meaningful fps number to report as achieved.
        f64::INFINITY
    } else {
        100.0 / centis as f64
    }
}

/// Optional, opt-in per-action motion-cycle capture (`capture-suite.json`
/// `#action_motion`): every simulation tick of one case's own run --
/// [`export::run_case_facing`]'s "one tick before the move starts to the
/// tick it ends" -- drawn in order, none skipped, at the sim's own 60Hz,
/// for both facing rows (matching `contact_wide_camera.rows_facing`). A
/// review aid for the full cycle (does anticipation read clearly? does
/// recovery? is the whole arc coherent?), never a measurement: nothing
/// here is captioned or compared pixel for pixel, and it never runs on a
/// plain full sweep (only when the caller explicitly asks). One fixed
/// camera for the whole clip -- the same `contact_wide_camera` anchor rule
/// the wide sheet uses, anchored on the case's first `"active"` tick (or
/// its midpoint if the case never reports one, e.g. a throw).
///
/// Produces two files per facing: the GIF itself, labelled with its real
/// achieved playback speed rather than a false "60fps" (see
/// [`gif_actual_fps`]) -- the `image` crate's GIF encoder cannot actually
/// hit 60fps -- and a lossless per-tick PNG sequence plus a JSON sidecar
/// (`sha`/`case`/`facing`/`ticks`/`camera`/`fps`) next to it, for
/// re-encoding to APNG/MP4 at the sim's true rate. The GIF stays as a
/// quick, honestly-labelled preview; the sidecar is the reliable source.
#[allow(clippy::too_many_arguments)]
async fn render_action_motion_gif(
    app: &mut App,
    rts: &mut Targets,
    suite: &Suite,
    wide_rule: &CamRule,
    out: &Path,
    id: CharacterId,
    mid: crate::sim::attacks::MoveId,
    variant: &str,
    rel: &str,
    facing: f32,
    source_sha: &str,
) -> Result<FileEntry, String> {
    let facing = if facing < 0.0 { -1.0 } else { 1.0 };
    let ticks = export::run_case_facing(id, mid, variant, facing)?;
    if ticks.is_empty() {
        return Err("empty case run".into());
    }
    // Anchor the fixed camera on the tick the action's real event happens:
    // its first active hitbox, or -- for an action whose only event is a
    // release -- the tick the shot actually leaves. Falling through to the
    // middle of the sequence would quietly reframe a move that no longer
    // has an `active` tick at all.
    let anchor_idx = ticks
        .iter()
        .position(|t| t.label == "active")
        .or_else(|| {
            ticks
                .iter()
                .position(|t| t.label == "emission" || t.label == "release")
        })
        .unwrap_or(ticks.len() / 2);
    let af = &ticks[anchor_idx].state.fighters[0];
    let cam = fixed_cam(wide_rule, af.pos.x, af.pos.y, facing);
    let spec = &suite.action_motion;
    let (gw, gh) = (spec.output[0], spec.output[1]);
    let rt = rts.get(gw, gh);
    let stem = rel.strip_suffix(".gif").unwrap_or(rel);
    let frames_rel = format!("{stem}-frames");
    let sidecar_rel = format!("{stem}.json");
    let path = out.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    let file = std::fs::File::create(&path).map_err(|e| e.to_string())?;
    let mut enc = image::codecs::gif::GifEncoder::new_with_speed(file, 10);
    enc.set_repeat(image::codecs::gif::Repeat::Infinite)
        .map_err(|e| e.to_string())?;
    let delay = image::Delay::from_numer_denom_ms(1000, spec.playback_fps.max(1));
    app.scene.reset();
    app.scene.reset_render_history();
    let mut frames = 0u32;
    for t in &ticks {
        app.scene.draw_fixed(
            &t.state,
            &cam,
            &rt,
            SceneOpts {
                hud: false,
                ..SceneOpts::default()
            },
        );
        next_frame().await;
        let img = read_rt(&rt);
        // One render, two outputs: the same frame goes into the (lossy
        // playback-speed) GIF and is also saved losslessly for the PNG
        // sequence, so the two can never drift apart in content.
        save(out, &format!("{frames_rel}/tick_{frames:04}.png"), &img)?;
        let frame = image::Frame::from_parts(img, 0, 0, delay);
        enc.encode_frame(frame).map_err(|e| e.to_string())?;
        frames += 1;
    }
    drop(enc);
    let actual_fps = gif_actual_fps(spec.playback_fps);
    let cam_meta = cam_json(&cam, gw, gh);
    let sidecar = serde_json::json!({
        "sha": source_sha,
        "case": {"action_id": export::action_id(mid), "variant_id": variant},
        "facing": facing,
        "ticks": frames,
        "camera": cam_meta.clone(),
        "fps": 60,
        "frames_dir": frames_rel.clone(),
        "frame_naming": "tick_%04d.png, one per simulation tick, none skipped",
        "note": "lossless per-tick PNG sequence at the simulation's real 60Hz; encode this to APNG/MP4 for reliable temporal review instead of the sibling .gif, whose encoder cannot actually reach 60fps (see that file's own note).",
    });
    std::fs::write(
        out.join(&sidecar_rel),
        serde_json::to_vec_pretty(&sidecar).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Ok(FileEntry {
        path: rel.to_string(),
        kind: "game3d",
        width: gw,
        height: gh,
        tick: None,
        fixture: None,
        camera: cam_meta.as_object().map_or_else(
            || serde_json::json!({}),
            |m| {
                let mut m = m.clone();
                m.insert("action_id".into(), export::action_id(mid).into());
                m.insert("variant_id".into(), variant.into());
                m.insert("facing".into(), facing.into());
                m.insert(
                    "declared".into(),
                    "capture-suite.json#action_motion, anchor/rule from #contact_wide_camera".into(),
                );
                m.insert(
                    "png_sequence".into(),
                    serde_json::json!({"frames_dir": frames_rel.clone(), "sidecar": sidecar_rel.clone(), "frame_count": frames, "fps": 60}),
                );
                serde_json::Value::Object(m)
            },
        ),
        note: Some(format!(
            "optional review capture, not a measurement: {frames} frames, one per simulation tick from one tick before the move starts through its last recovery tick, sim runs at 60Hz and every tick is drawn, none skipped. Requested {} fps playback, but the `image` crate's GIF encoder actually achieves ~{actual_fps:.1} fps here (its centisecond rounding does not preserve 60fps -- see `{sidecar_rel}` for a lossless per-tick PNG sequence to re-encode locally instead). Not part of contact_expected.",
            spec.playback_fps
        )),
    })
}

/// The same case again, seen through **the game's own match camera**.
///
/// The declared art cameras are fixed and orthographic, which is what makes
/// their measurements comparable; this one is the real
/// [`MatchCamera`](crate::model::MatchCamera), updated every tick from the
/// case's own state exactly as the game updates it, so an action can also be
/// judged at the framing and scale a player actually sees. It is a review
/// capture and says so: nothing is measured here.
///
/// The camera is settled on the first tick's state before recording, so the
/// clip opens already framed instead of easing in from its default pose.
///
/// What it reproduces and what it does not is recorded in the sidecar and
/// the index note rather than left to be assumed: the 3D pass is the
/// interactive one — same camera, same shake, same billboard axes, same
/// hitstop wash — and the HUD is deliberately off. It is still a review
/// capture, not a measurement.
#[allow(clippy::too_many_arguments)]
async fn render_action_gamecam_gif(
    app: &mut App,
    rts: &mut Targets,
    suite: &Suite,
    out: &Path,
    id: CharacterId,
    mid: crate::sim::attacks::MoveId,
    variant: &str,
    rel: &str,
    facing: f32,
    source_sha: &str,
) -> Result<FileEntry, String> {
    let facing = if facing < 0.0 { -1.0 } else { 1.0 };
    let ticks = export::run_case_facing(id, mid, variant, facing)?;
    if ticks.is_empty() {
        return Err("empty case run".into());
    }
    let spec = &suite.action_motion;
    let (gw, gh) = (spec.output[0], spec.output[1]);
    let rt = rts.get(gw, gh);
    let aspect = gw as f32 / gh.max(1) as f32;
    let stem = rel.strip_suffix(".gif").unwrap_or(rel);
    let frames_rel = format!("{stem}-frames");
    let sidecar_rel = format!("{stem}.json");
    let path = out.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    let file = std::fs::File::create(&path).map_err(|e| e.to_string())?;
    let mut enc = image::codecs::gif::GifEncoder::new_with_speed(file, 10);
    enc.set_repeat(image::codecs::gif::Repeat::Infinite)
        .map_err(|e| e.to_string())?;
    let delay = image::Delay::from_numer_denom_ms(1000, spec.playback_fps.max(1));

    let mut mc = crate::model::MatchCamera::new();
    for _ in 0..90 {
        mc.update(&ticks[0].state, aspect);
    }
    let opened = crate::render::scene3d::FixedCam::from_match(&mc);

    app.scene.reset();
    app.scene.reset_render_history();
    // `Scene::billboard` takes its axes from the scene's *own* camera, and
    // `Scene::reset` only clears that camera's smoothing, not its pose. So
    // the camera this capture projects with is also assigned to the scene,
    // shake included: otherwise the soft halo and the release flash face a
    // camera that is not the one the frame was rendered from. The previous
    // camera is put back afterwards so the orthographic captures that run
    // next are bit-for-bit what they were.
    let saved_cam = app.scene.camera;
    let mut frames = 0u32;
    let mut max_shake = 0.0f32;
    let mut max_flash = 0.0f32;
    for t in &ticks {
        mc.update(&t.state, aspect);
        // The same shake `Scene::draw` applies, on the same camera.
        let shake = mc.shake(&t.state);
        let mut shaken = mc;
        shaken.pos = mc.pos + shake;
        shaken.target = mc.target + shake;
        app.scene.camera = shaken;
        max_shake = max_shake.max(t.state.camera_shake);
        max_flash = max_flash.max(t.state.hitstop_flash);
        let cam = crate::render::scene3d::FixedCam::from_match(&shaken);
        app.scene.draw_fixed(
            &t.state,
            &cam,
            &rt,
            SceneOpts {
                hud: false,
                ..SceneOpts::default()
            },
        );
        // …and the 2D hitstop wash the interactive render lays on top.
        app.scene.draw_hitstop_flash(&t.state, &rt);
        next_frame().await;
        let img = read_rt(&rt);
        save(out, &format!("{frames_rel}/tick_{frames:04}.png"), &img)?;
        let frame = image::Frame::from_parts(img, 0, 0, delay);
        enc.encode_frame(frame).map_err(|e| e.to_string())?;
        frames += 1;
    }
    drop(enc);
    app.scene.camera = saved_cam;
    let actual_fps = gif_actual_fps(spec.playback_fps);
    let closed = crate::render::scene3d::FixedCam::from_match(&mc);
    let cam_meta = serde_json::json!({
        "rule": "the game's own MatchCamera, updated once per simulation tick from this case's state",
        "projection": "perspective",
        "fovy_degrees": mc.fovy.to_degrees(),
        "first_tick": {"eye": [opened.eye.x, opened.eye.y, opened.eye.z],
                       "target": [opened.target.x, opened.target.y, opened.target.z]},
        "last_tick": {"eye": [closed.eye.x, closed.eye.y, closed.eye.z],
                      "target": [closed.target.x, closed.target.y, closed.target.z]},
        "width": gw,
        "height": gh,
        "hud": false,
        "reproduces": "the 3D pass of the interactive render: the same MatchCamera including its shake, the same perspective projection, the same billboard axes, and the same 2D hitstop wash",
        "omits": "the HUD (port tags, damage, stocks, timer) -- deliberately, so the action is not covered by it. Nothing else from `Scene::draw` is left out.",
        "max_camera_shake_in_case": max_shake,
        "max_hitstop_flash_in_case": max_flash,
        "fighters_in_case": ticks[0].state.fighters.len(),
    });
    let sidecar = serde_json::json!({
        "sha": source_sha,
        "case": {"action_id": export::action_id(mid), "variant_id": variant},
        "facing": facing,
        "ticks": frames,
        "camera": cam_meta.clone(),
        "fps": 60,
        "frames_dir": frames_rel.clone(),
        "frame_naming": "tick_%04d.png, one per simulation tick, none skipped",
        "note": "review capture through the real match camera, not a measurement: the declared orthographic art cameras are the ones anything is measured against. Lossless per-tick PNG sequence at the simulation's real 60Hz; encode this rather than the sibling .gif.",
    });
    std::fs::write(
        out.join(&sidecar_rel),
        serde_json::to_vec_pretty(&sidecar).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Ok(FileEntry {
        path: rel.to_string(),
        kind: "game3d",
        width: gw,
        height: gh,
        tick: None,
        fixture: None,
        camera: cam_meta,
        note: Some(format!(
            "optional review capture through the game's own match camera, not a measurement: {frames} frames, one per simulation tick, none skipped. Reproduces the interactive 3D pass -- same MatchCamera and shake (max {max_shake:.3} in this case), same perspective projection, same billboard axes, same 2D hitstop wash (max {max_flash:.3}) -- and deliberately omits only the HUD. Requested {} fps playback; the GIF encoder actually achieves ~{actual_fps:.1} fps (see `{sidecar_rel}` for the lossless PNG sequence). Not part of contact_expected.",
            spec.playback_fps
        )),
    })
}

/// Caption lines on a dark band at the top of a cell (readable over any
/// scene content).
///
/// Every row is measured against the cell before it is drawn
/// ([`font::fit_line`]): a row that would not fit shrinks to the largest
/// scale that does, so no caption can leave its cell.
fn caption_block(lines: &[String], cell_w: f32) {
    const PAD: f32 = 8.0;
    const LINE: f32 = 18.0;
    const BASE: f32 = 1.3;
    const MIN: f32 = 0.9;
    let h = 6.0 + LINE * lines.len() as f32 + 4.0;
    draw_rectangle(0.0, 0.0, cell_w, h, Color::from_rgba(8, 10, 18, 190));
    for (li, line) in lines.iter().enumerate() {
        let (text, scale) = font::fit_line(&ascii_fold(line), cell_w - PAD * 2.0, BASE, MIN);
        label_text(&text, PAD, 6.0 + LINE * li as f32, scale);
    }
}

/// Number of pixels that differ between two images of the same size
/// (`u32::MAX` if the sizes differ).
fn pixel_diff(a: &RgbaImage, b: &RgbaImage) -> u32 {
    if a.dimensions() != b.dimensions() {
        return u32::MAX;
    }
    a.pixels().zip(b.pixels()).filter(|(x, y)| x != y).count() as u32
}

/// Rec.709 luma copy of an image (for the greyscale mirror row).
fn greyscale(img: &RgbaImage) -> RgbaImage {
    let mut out = img.clone();
    for p in out.pixels_mut() {
        let l = (0.2126 * p[0] as f32 + 0.7152 * p[1] as f32 + 0.0722 * p[2] as f32)
            .round()
            .clamp(0.0, 255.0) as u8;
        *p = Rgba([l, l, l, 255]);
    }
    out
}

// ----------------------------------------------------------------- runner

/// Produce the whole suite under `opts.out`. Returns the index it wrote.
pub(super) async fn run(app: &mut App, opts: &CaptureOpts) -> Result<CaptureIndex, String> {
    let suite = capture_suite::load()?;
    let primary_rule = suite.contact_camera.rule()?;
    let wide_rule = suite.contact_wide_camera.rule()?;
    let out = opts.out.clone();
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let mut rts = Targets::default();
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
        characters: suite.characters.clone(),
        ..Default::default()
    };

    // The first contact case rendered in this run, for the isolation
    // self-check at the end: (action, variant, primary path, wide path,
    // primary image, wide image).
    let mut first_contact: Option<(
        crate::sim::attacks::MoveId,
        String,
        String,
        String,
        RgbaImage,
        Option<RgbaImage>,
    )> = None;

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
        let cell = rts.get(cell_w, cell_h);

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
            let ncell = suite.combat_size.cells.len() as u32;
            let (cw, ch) = (
                suite.combat_size.output[0] / ncell,
                suite.combat_size.output[1],
            );
            let sheet_w = cw * ncell;
            let mut sheet: RgbaImage = ImageBuffer::new(sheet_w, ch);
            let cellrt = rts.get(cw, ch);
            let mut cells = Vec::new();
            let heights: Vec<f32> = suite
                .combat_size
                .cells
                .iter()
                .map(|c| c.projected_height_px)
                .collect();
            for (k, px) in heights.iter().enumerate() {
                let oh = height * ch as f32 / px;
                let c = FixedCam {
                    eye: v3(f.pos.x, f.pos.y + oh * 0.42, 120.0),
                    target: v3(f.pos.x, f.pos.y + oh * 0.42, 0.0),
                    ortho_height: oh,
                    perspective: false,
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

        // Contact sheets: every exported case of the character (all 22
        // actions and their real variants). Primary sheet = the suite's
        // contact_camera (per-cell camera, 3 cells, facing +1); wide sheet =
        // contact_wide_camera (one fixed camera per facing row, 4 cells).
        // Every cell reconstructs the render history from scratch and the
        // measurement is taken on the pose that is drawn.
        let cases = export::cases_for(id);
        let mut first_variant_seen: Vec<&'static str> = Vec::new();
        for (_, mid, variant) in &cases {
            let aid = export::action_id(*mid);
            let primary_variant = !first_variant_seen.contains(&aid);
            first_variant_seen.push(aid);
            let stem = if primary_variant {
                format!("characters/{name}/contact-{aid}")
            } else {
                format!("characters/{name}/contact-{aid}-{variant}")
            };
            let rel = format!("{stem}.png");
            let rel_wide = format!("{stem}-wide.png");
            index.contact_expected.push(ContactExpected {
                character_id: name.clone(),
                action_id: aid.into(),
                variant_id: variant.to_string(),
                primary: rel.clone(),
                wide: rel_wide.clone(),
                evidence_kind: if export::action_spawns_projectile(*mid) {
                    "projectile"
                } else if export::action_has_hitbox(*mid) {
                    "hitbox"
                } else {
                    "throw_no_hitbox"
                },
                fighter_hitbox: export::action_has_hitbox(*mid)
                    && !crate::sim::attacks::data(id, *mid).no_melee,
            });
            // The optional motion gif (below) has its own name and its own
            // gate (only ever produced when `--only` is explicitly given),
            // so it must also be checked here: otherwise a run asking only
            // for `--only <action>-motion` would never reach it, since
            // neither the primary nor the wide sheet name matches that
            // filter and the loop would skip the case entirely.
            let motion_rel = format!("{stem}-motion.gif");
            let want_motion = opts.only.is_some() && want(&motion_rel);
            if !want(&rel) && !want(&rel_wide) && !want_motion {
                continue;
            }
            let sheets = match render_contact_case(
                app,
                &mut rts,
                &suite,
                &primary_rule,
                &wide_rule,
                id,
                *mid,
                variant,
                &rel,
                &rel_wide,
                want(&rel),
                want(&rel_wide),
                SheetEvent::Declared,
            )
            .await
            {
                Ok(s) => s,
                Err(e) => {
                    index.skipped.push((rel.clone(), e.clone()));
                    index.skipped.push((rel_wide.clone(), e));
                    continue;
                }
            };
            if let Some((img, entry)) = sheets.primary {
                save(&out, &rel, &img)?;
                if first_contact.is_none() {
                    first_contact = Some((
                        *mid,
                        variant.to_string(),
                        rel.clone(),
                        rel_wide.clone(),
                        img.clone(),
                        None,
                    ));
                }
                index.files.push(entry);
            }
            if let Some((img, entry)) = sheets.wide {
                save(&out, &rel_wide, &img)?;
                if let Some(fc) = first_contact.as_mut() {
                    if fc.2 == rel && fc.5.is_none() {
                        fc.5 = Some(img.clone());
                    }
                }
                index.files.push(entry);
            }

            // A projectile action can *also* carry an ordinary fighter
            // hitbox some frames after the release (Boulder's and Viper's
            // special_n still do, on `md.startup`) — per art commit
            // 343def9c8c45ec69dba36b2b7ce863255ee9b0e4 and review
            // 91f9c5a2834353bbb7207dcc4d866a4e513fd48f. The emission
            // exception in wide_picks/primary_picks must never hide such a
            // hitbox, so it gets its own clearly labelled supplemental
            // sheet pair, both facings, on the same declared cameras. Not
            // part of contact_expected (that stays one declared case per
            // action/variant) — these are additional files, not a second
            // required coverage row.
            //
            // Kestrel's special_n no longer has one at all (`no_melee`, see
            // `sim::attacks`), so the supplement is not produced for it and
            // is not demanded of it: the sheet would have nothing to show,
            // and demanding it would keep asking for a contact the
            // simulation no longer makes. This is gated on the real move
            // data, not on the character, so it follows whatever the tables
            // actually say.
            let has_fighter_hitbox = !crate::sim::attacks::data(id, *mid).no_melee;
            if *mid == crate::sim::attacks::MoveId::SpecialN && has_fighter_hitbox {
                let hb_stem = format!("{stem}-hitbox");
                let hb_rel = format!("{hb_stem}.png");
                let hb_rel_wide = format!("{hb_stem}-wide.png");
                if want(&hb_rel) || want(&hb_rel_wide) {
                    match render_contact_case(
                        app,
                        &mut rts,
                        &suite,
                        &primary_rule,
                        &wide_rule,
                        id,
                        *mid,
                        variant,
                        &hb_rel,
                        &hb_rel_wide,
                        want(&hb_rel),
                        want(&hb_rel_wide),
                        SheetEvent::FighterHitbox,
                    )
                    .await
                    {
                        Ok(hb_sheets) => {
                            if let Some((img, entry)) = hb_sheets.primary {
                                save(&out, &hb_rel, &img)?;
                                index.files.push(entry);
                            }
                            if let Some((img, entry)) = hb_sheets.wide {
                                save(&out, &hb_rel_wide, &img)?;
                                index.files.push(entry);
                            }
                        }
                        Err(e) => {
                            index.skipped.push((hb_rel.clone(), e.clone()));
                            index.skipped.push((hb_rel_wide.clone(), e));
                        }
                    }
                }
            }

            // Optional, opt-in per-action motion-cycle GIF (capture-suite.json
            // #action_motion): every simulation tick of this case's own run,
            // none skipped, so a reviewer can watch the full cycle rather
            // than only the primary/wide contact cells. This never runs on
            // a plain full sweep (`--only` unset) -- only when explicitly
            // asked for (e.g. `--only motion`), so it never competes with
            // the run size budget of an ordinary evidence capture. Not part
            // of `contact_expected`: supplementary, never required for
            // coverage, and primary/wide sheets and their cameras are
            // unchanged. Both facing rows are produced together (matching
            // `contact_wide_camera.rows_facing`) whenever the trigger name
            // matches -- `--only <action>-motion` is one request for the
            // whole cycle, not one per facing.
            if want_motion {
                let motion_rel_b = format!("{stem}-motion2.gif");
                for (rel, facing) in [(motion_rel.clone(), 1.0f32), (motion_rel_b, -1.0f32)] {
                    match render_action_motion_gif(
                        app,
                        &mut rts,
                        &suite,
                        &wide_rule,
                        &out,
                        id,
                        *mid,
                        variant,
                        &rel,
                        facing,
                        &opts.source_sha,
                    )
                    .await
                    {
                        Ok(entry) => index.files.push(entry),
                        Err(e) => index.skipped.push((rel, e)),
                    }
                }
                // The same cycle again through the game's own match
                // camera, so the action is reviewed at the framing a
                // player actually sees as well as at the fixed
                // measurement camera. Same trigger, same two facings.
                for (rel, facing) in [
                    (format!("{stem}-gamecam.gif"), 1.0f32),
                    (format!("{stem}-gamecam2.gif"), -1.0f32),
                ] {
                    match render_action_gamecam_gif(
                        app,
                        &mut rts,
                        &suite,
                        &out,
                        id,
                        *mid,
                        variant,
                        &rel,
                        facing,
                        &opts.source_sha,
                    )
                    .await
                    {
                        Ok(entry) => index.files.push(entry),
                        Err(e) => index.skipped.push((rel, e)),
                    }
                }
            }
        }

        // Hurt capsules in idle / crouch / hitstun, primary camera.
        let rel = format!("characters/{name}/capsules.png");
        if want(&rel) {
            let (cw, ch) = (512u32, 512u32);
            let cellrt = rts.get(cw, ch);
            let mut sheet: RgbaImage = ImageBuffer::new(cw * 3, ch);
            let mut cells = Vec::new();
            let base = idle.state_at(suite.combat.fixture_tick);
            for (k, (what, st)) in [
                ("idle", State::Stand),
                ("crouch", State::Crouch),
                ("hitstun", State::Hitstun { tumble: false }),
            ]
            .iter()
            .enumerate()
            {
                let mut gs = base.clone();
                gs.fighters[0].set_state_pub(*st);
                gs.fighters[0].state_frame = 6;
                gs.fighters[1].set_state_pub(State::Dead);
                let f = &gs.fighters[0];
                let c = FixedCam {
                    eye: v3(f.pos.x, f.pos.y + 22.0, 120.0),
                    target: v3(f.pos.x, f.pos.y + 22.0, 0.0),
                    ortho_height: 44.0,
                    perspective: false,
                };
                let diag = Diagnostics {
                    capsules: true,
                    ..Default::default()
                };
                app.scene.reset_render_history();
                let _ = app.scene.draw_fixed_diag(
                    &gs,
                    &c,
                    &cellrt,
                    SceneOpts {
                        hud: false,
                        ..SceneOpts::default()
                    },
                    Some(&diag),
                );
                let cap = crate::model::contact::capsule(f);
                set_camera(&super::scene3d::rt_camera_2d(&cellrt));
                label_text(&format!("{what}  state {:?}", f.state), 8.0, 8.0, 1.4);
                label_text(
                    &format!(
                        "capsule a=({:.2},{:.2}) b=({:.2},{:.2}) r={:.2}",
                        cap.a[0], cap.a[1], cap.b[0], cap.b[1], cap.radius
                    ),
                    8.0,
                    28.0,
                    1.4,
                );
                set_default_camera();
                next_frame().await;
                let img = read_rt(&cellrt);
                blit(&mut sheet, &img, k as u32 * cw, 0);
                cells.push(serde_json::json!({
                    "which": what, "state": format!("{:?}", f.state),
                    "capsule": cap, "root": [f.pos.x, f.pos.y], "facing": f.facing,
                    "camera": cam_json(&c, cw, ch),
                }));
            }
            save(&out, &rel, &sheet)?;
            index.files.push(FileEntry {
                path: rel,
                kind: "game3d",
                width: cw * 3,
                height: ch,
                tick: Some(suite.combat.fixture_tick as i64),
                fixture: Some(idle.id.clone()),
                camera: serde_json::json!({ "cells": cells, "note": "state set on the idle fixture's player 0 at state_frame 6; the capsule is Fighter::hurt_segment/hurt_radius exactly as the sim tests it" }),
                note: Some("green wireframe = hurt capsule the simulation tests".into()),
            });
        }

        // Supplemental support view: the same states plus a landing, with a
        // camera that leaves room *below* the support plane, so both feet
        // are fully visible and the measured support distances can be read
        // against the drawn floor line. The declared capsule camera above is
        // unchanged; this is the extra fixed diagnostic view the review
        // allows, with its camera recorded here.
        let rel = format!("characters/{name}/support.png");
        if want(&rel) {
            let (cw, ch) = (512u32, 512u32);
            let cellrt = rts.get(cw, ch);
            let states = [
                ("idle", State::Stand, 6u32),
                ("crouch", State::Crouch, 6),
                ("hitstun", State::Hitstun { tumble: false }, 6),
                ("land_lag", State::LandLag { total: 12 }, 1),
            ];
            let mut sheet: RgbaImage = ImageBuffer::new(cw * states.len() as u32, ch);
            let mut cells = Vec::new();
            let base = idle.state_at(suite.combat.fixture_tick);
            // Room for the whole fighter plus 8 units under the floor.
            const OH: f32 = 52.0;
            const BELOW: f32 = 8.0;
            for (k, (what, st, sf)) in states.iter().enumerate() {
                let mut gs = base.clone();
                gs.fighters[0].set_state_pub(*st);
                gs.fighters[0].state_frame = *sf;
                gs.fighters[1].set_state_pub(State::Dead);
                let f = &gs.fighters[0];
                let cy = f.pos.y - BELOW + OH * 0.5;
                let c = FixedCam {
                    eye: v3(f.pos.x, cy, 120.0),
                    target: v3(f.pos.x, cy, 0.0),
                    ortho_height: OH,
                    perspective: false,
                };
                // Ask for port 0's feet explicitly -- this view is evidence
                // about the fighter being posed, not whichever fighter's
                // DiagOut a same-came-back-non-empty heuristic used to pick.
                // No attack is in progress here, so there is no `contact`
                // query to piggyback on; `support_port` is the only way to
                // name the fighter without inventing one.
                let diag = Diagnostics {
                    capsules: true,
                    support_port: Some(0),
                    ..Default::default()
                };
                app.scene.reset_render_history();
                let measured = app.scene.draw_fixed_diag(
                    &gs,
                    &c,
                    &cellrt,
                    SceneOpts {
                        hud: false,
                        ..SceneOpts::default()
                    },
                    Some(&diag),
                );
                let feet = &measured.support;
                // Two real, finite foot measurements or this cell is not
                // evidence: an empty/short list folding to f32::MAX and
                // getting drawn as "lowest foot" is exactly the defect this
                // guards against -- fail loudly here instead of saving a
                // sentinel as if it were a measurement.
                if feet.len() != 2 || feet.iter().any(|x| !x.support_distance.is_finite()) {
                    return Err(format!(
                        "support.png/{what}: expected 2 finite foot measurements, got {:?}",
                        feet.iter()
                            .map(|x| (x.piece_id.clone(), x.support_distance))
                            .collect::<Vec<_>>()
                    ));
                }
                let lowest = feet
                    .iter()
                    .map(|x| x.support_distance)
                    .fold(f32::MAX, f32::min);
                flush_gl();
                set_camera(&super::scene3d::rt_camera_2d(&cellrt));
                // The support plane, exactly where the camera puts it.
                let plane_y = (cy + OH * 0.5 - f.pos.y) * ch as f32 / OH;
                draw_rectangle(
                    0.0,
                    plane_y,
                    cw as f32,
                    1.0,
                    Color::from_rgba(90, 220, 220, 190),
                );
                let mut rows = vec![
                    format!("{what}  sf{sf}"),
                    format!("support plane y={:.2} (cyan)", f.pos.y),
                    format!("lowest foot {lowest:+.3}"),
                ];
                for x in feet {
                    rows.push(format!(
                        "{} y[{:.2},{:.2}] d{:+.3}",
                        x.piece_id, x.aabb_min[1], x.aabb_max[1], x.support_distance
                    ));
                }
                caption_block(&rows, cw as f32);
                flush_gl();
                set_default_camera();
                next_frame().await;
                let img = read_rt(&cellrt);
                blit(&mut sheet, &img, k as u32 * cw, 0);
                cells.push(serde_json::json!({
                    "which": what, "state": format!("{:?}", f.state), "state_frame": sf,
                    "root": [f.pos.x, f.pos.y], "facing": f.facing,
                    "support_plane_y": f.pos.y,
                    "lowest_support_distance": lowest,
                    "feet": feet,
                    "camera": cam_json(&c, cw, ch),
                }));
            }
            save(&out, &rel, &sheet)?;
            index.files.push(FileEntry {
                path: rel,
                kind: "game3d",
                width: cw * states.len() as u32,
                height: ch,
                tick: Some(suite.combat.fixture_tick as i64),
                fixture: Some(idle.id.clone()),
                camera: serde_json::json!({
                    "cells": cells,
                    "declared_here": "supplemental fixed diagnostic view (docs/art/reviews: \"add a fixed diagnostic capsule view with declared camera if needed\"); the suite's own contact and capsule cameras are unchanged",
                    "camera_rule": {"eye": ["player.x", "player.y - 8 + 26", 120], "orthographic_height": OH, "note": "8 units of the frame are below the support plane so the feet cannot be cut off"},
                }),
                note: Some("cyan line = the support plane the fighter stands on; captions carry each foot piece's measured Y bounds and its signed distance to that plane (negative = through the floor)".into()),
            });
        }

        // Mirror 0/3: same character, palettes 0 and 3, idle and attack,
        // colour and greyscale, at the 120 px reference height.
        let rel = format!("characters/{name}/mirror-0-3.png");
        if want(&rel) {
            let (cw, ch) = (480u32, 270u32);
            let cellrt = rts.get(cw, ch);
            let height = id.data().height;
            let oh = height * ch as f32 / 120.0;
            let c = FixedCam {
                eye: v3(0.0, oh * 0.45, 120.0),
                target: v3(0.0, oh * 0.45, 0.0),
                ortho_height: oh,
                perspective: false,
            };
            let mut cfg = crate::sim::MatchConfig {
                stage: crate::sim::stage::StageId::Lattice,
                ..crate::sim::MatchConfig::default()
            };
            cfg.chars[0] = id;
            cfg.chars[1] = id;
            cfg.palettes[0] = 0;
            cfg.palettes[1] = 3;
            let mut gs = crate::sim::GameState::new(2, cfg);
            export::place_on_main(
                &mut gs,
                &[
                    export::FixturePlayer {
                        character: name.clone(),
                        palette: 0,
                        x: -14.0,
                        facing: 1.0,
                    },
                    export::FixturePlayer {
                        character: name.clone(),
                        palette: 3,
                        x: 14.0,
                        facing: -1.0,
                    },
                ],
            );
            gs.fighters[0].palette = 0;
            gs.fighters[1].palette = 3;
            for _ in 0..8 {
                gs.step(&[crate::sim::PlayerInput::default(); 2]);
            }
            let idle_state = gs.clone();
            // Attack: player 0 forward-tilts into player 1 (both attacking
            // would clank into Rebound before an active after_step tick);
            // sample player 0's first active tick.
            let mut attack_state = None;
            for t in 0..40u32 {
                let inp = if t == 0 {
                    [
                        crate::sim::PlayerInput {
                            stick: Vec2::new(0.55, 0.0),
                            buttons: crate::sim::buttons::ATTACK,
                            ..Default::default()
                        },
                        crate::sim::PlayerInput::default(),
                    ]
                } else {
                    [crate::sim::PlayerInput::default(); 2]
                };
                gs.step(&inp);
                if gs.fighters[0].hitbox_geometry().is_some() {
                    attack_state = Some(gs.clone());
                    break;
                }
            }
            let mut sheet: RgbaImage = ImageBuffer::new(cw * 2, ch * 2);
            let mut cells = Vec::new();
            let shots: Vec<(&str, Option<&crate::sim::GameState>)> = vec![
                ("idle", Some(&idle_state)),
                ("ftilt_first_active", attack_state.as_ref()),
            ];
            for (k, (what, st)) in shots.iter().enumerate() {
                let Some(st) = st else {
                    index
                        .skipped
                        .push((format!("{rel}#{what}"), "attack sample not reached".into()));
                    continue;
                };
                app.scene.draw_fixed(
                    st,
                    &c,
                    &cellrt,
                    SceneOpts {
                        hud: false,
                        ..SceneOpts::default()
                    },
                );
                set_camera(&super::scene3d::rt_camera_2d(&cellrt));
                label_text(
                    &format!("{what}  palettes 0 / 3  120px (P1 attacks, P2 idle)"),
                    6.0,
                    6.0,
                    1.2,
                );
                set_default_camera();
                next_frame().await;
                let img = read_rt(&cellrt);
                blit(&mut sheet, &img, k as u32 * cw, 0);
                let grey = greyscale(&img);
                blit(&mut sheet, &grey, k as u32 * cw, ch);
                cells.push(serde_json::json!({
                    "which": what, "row0": "colour", "row1": "greyscale (Rec.709 luma)",
                    "p1": {"palette": 0, "x": -14, "facing": 1}, "p2": {"palette": 3, "x": 14, "facing": -1},
                    "camera": cam_json(&c, cw, ch),
                }));
            }
            save(&out, &rel, &sheet)?;
            index.files.push(FileEntry {
                path: rel,
                kind: "game3d",
                width: cw * 2,
                height: ch * 2,
                tick: None,
                fixture: None,
                camera: serde_json::json!({ "cells": cells, "scale_px_per_unit": 120.0 / height }),
                note: Some("mirror match 0/3 on the real stage; bottom row is the same render converted to greyscale on the CPU".into()),
            });
        }
    }

    // ---------------------------------------------------------- scenes
    let cs = &suite.combat;
    let (sw, sh) = (cs.output[0], cs.output[1]);
    let scene_rt = rts.get(sw, sh);
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
        let gif_rt = rts.get(gw, gh);
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
        let sel = &suite.ui_selection;
        app.rules.p1 = export::parse_character(&sel.p1.character).unwrap_or(CharacterId::Kestrel);
        app.rules.p1_pal = sel.p1.palette;
        app.rules.p2 = export::parse_character(&sel.p2.character).unwrap_or(CharacterId::Kestrel);
        app.rules.p2_pal = sel.p2.palette;
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
                "screen": format!("{} (real menu)", sel.screen),
                "p1": {"character": sel.p1.character, "palette": sel.p1.palette},
                "p2": {"character": sel.p2.character, "palette": sel.p2.palette},
            }),
            note: Some(if [w, h] == sel.window {
                format!("window at {w}x{h}")
            } else {
                format!(
                    "window came up at {w}x{h}, not {}x{}",
                    sel.window[0], sel.window[1]
                )
            }),
        });
    }

    // ---------------------------------------------------------- isolation
    // Re-render the first contact case after everything else and compare
    // it pixel for pixel with the copy produced mid-run: an unequal result
    // means renderer state leaked between captures (GPU pass, batching,
    // history) and the run must not be trusted.
    if let Some((mid, variant, rel, rel_wide, primary_img, wide_img)) = first_contact.take() {
        let id = export::parse_character(&suite.characters[0]).unwrap_or(CharacterId::Kestrel);
        match render_contact_case(
            app,
            &mut rts,
            &suite,
            &primary_rule,
            &wide_rule,
            id,
            mid,
            &variant,
            &rel,
            &rel_wide,
            true,
            wide_img.is_some(),
            SheetEvent::Declared,
        )
        .await
        {
            Ok(again) => {
                let mut report = serde_json::json!({
                    "case": {"action_id": export::action_id(mid), "variant_id": variant},
                    "method": "same case rendered again after the whole run, from a reset render history, and compared byte for byte with the mid-run sheet",
                });
                let mut ok = true;
                if let Some((img2, _)) = again.primary {
                    let diff = pixel_diff(&primary_img, &img2);
                    if diff > 0 {
                        ok = false;
                        let p = rel.replace(".png", "-isolated.png");
                        save(&out, &p, &img2)?;
                        report["primary_isolated_copy"] = serde_json::json!(p);
                    }
                    report["primary_differing_pixels"] = serde_json::json!(diff);
                }
                if let (Some(w1), Some((w2, _))) = (wide_img, again.wide) {
                    let diff = pixel_diff(&w1, &w2);
                    if diff > 0 {
                        ok = false;
                        let p = rel_wide.replace(".png", "-isolated.png");
                        save(&out, &p, &w2)?;
                        report["wide_isolated_copy"] = serde_json::json!(p);
                    }
                    report["wide_differing_pixels"] = serde_json::json!(diff);
                }
                report["identical"] = serde_json::json!(ok);
                index.isolation_check = Some(report);
            }
            Err(e) => {
                index.isolation_check = Some(serde_json::json!({"identical": false, "error": e}));
            }
        }
    }

    // ---------------------------------------------------------- runtime
    let contact_chars: Vec<CharacterId> = suite
        .characters
        .iter()
        .filter_map(|n| export::parse_character(n))
        .collect();
    match export::write_runtime_for(&out, &opts.source_sha, &contact_chars) {
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The exact regression Codex's Windows review caught: a requested
    /// 60fps GIF does not actually play at 60fps, because the `image`
    /// crate's encoder truncates to a whole millisecond before flooring to
    /// a whole centisecond. 1000/60 = 16.67ms truncates to 16ms, and
    /// 16ms/10 floors to 1 centisecond -- an actual playback rate of
    /// 100fps, not 60. This is the number `render_action_motion_gif` must
    /// report instead of repeating the false "60fps" claim.
    #[test]
    fn gif_actual_fps_reports_the_real_rate_not_the_request() {
        assert_eq!(gif_actual_fps(60), 100.0);
    }

    /// The pre-existing `gif` recipe's 20fps happens to round-trip
    /// perfectly (1000/20 = 50ms, an exact multiple of 10) -- which is
    /// exactly why this bug went unnoticed until a 60fps request exposed
    /// it. This case guards against "fixing" the function into reporting
    /// a wrong number for a request that was never actually broken.
    #[test]
    fn gif_actual_fps_matches_the_request_when_it_divides_evenly() {
        assert_eq!(gif_actual_fps(20), 20.0);
        assert_eq!(gif_actual_fps(25), 25.0);
        assert_eq!(gif_actual_fps(50), 50.0);
        assert_eq!(gif_actual_fps(100), 100.0);
    }

    /// A request slow enough that even a whole extra millisecond of
    /// truncation error cannot change which centisecond it floors to
    /// still comes back accurate to within that one encoder unit.
    #[test]
    fn gif_actual_fps_is_never_wildly_off_for_a_slow_request() {
        let got = gif_actual_fps(10);
        assert!((got - 10.0).abs() < 0.5, "got {got}");
    }
}
