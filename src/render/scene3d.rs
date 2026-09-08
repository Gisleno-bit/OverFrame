//! macroquad 3D match renderer.
//!
//! Uploads the stage once per stage, skins every fighter each frame from the
//! `model` layer (CPU lighting, no custom shaders), draws soft shadows, shield
//! bubbles, billboard hit effects and the strike glow, then hands off to the
//! 2D HUD in `viz`. Camera framing/smoothing lives here too — it is render
//! state only and never touches the simulation.

use macroquad::prelude::*;

use crate::model::anim;
use crate::model::characters::CharacterModel;
use crate::model::lighting::{shade_static, slot, Light, Tint};
use crate::model::math3::{v3, Xf, M3, V3};
use crate::model::mesh::MeshData;
use crate::model::rig::Vert;
use crate::model::stage3d::{self, StageModel, SLAB_DEPTH, SOFT_DEPTH};
use crate::model::MatchCamera;
use crate::sim::attacks;
use crate::sim::fighter::{Fighter, State};
use crate::sim::roster::CharacterId;
use crate::sim::stage::StageId;
use crate::sim::{FxKind, GameState};
use crate::viz::{self, font, Color as VColor, Painter, SceneOpts};

use super::{col, MqPainter};

/// Largest index batch we hand to macroquad in one call (its default draw
/// call holds 5000 indices / 10000 vertices).
const MAX_BATCH_INDICES: usize = 4500;

/// Uploaded stage geometry for the menu's orbiting stage preview.
struct PreviewStage {
    id: StageId,
    near: Vec<Mesh>,
    far: Vec<Mesh>,
    sky_top: [u8; 3],
    sky_bottom: [u8; 3],
}

pub struct Scene3D {
    models: Vec<CharacterModel>,
    stage_id: Option<StageId>,
    stage_near: Vec<Mesh>,
    stage_far: Vec<Mesh>,
    light: Light,
    sky_top: VColor,
    sky_bottom: VColor,
    pub camera: MatchCamera,
    tex_soft: Texture2D,
    tex_spark: Texture2D,
    tex_ring: Texture2D,
    stage_tex: Option<Texture2D>,
    /// Cached stage geometry for the menu preview.
    preview_stage: Option<PreviewStage>,
    // Scratch buffers reused every frame.
    verts: Vec<Vert>,
    idx: Vec<u32>,
    remap: Vec<u32>,
    stamp: Vec<u32>,
    generation: u32,
    /// Last view-projection matrix, for projecting labels onto the screen.
    view_proj: Mat4,
    preview_angle: f32,
}

impl Scene3D {
    pub fn new() -> Self {
        let models = CharacterId::ALL
            .iter()
            .map(|id| crate::model::characters::build_with_assets(*id))
            .collect();
        Scene3D {
            models,
            stage_id: None,
            stage_near: Vec::new(),
            stage_far: Vec::new(),
            light: Light::default_for((40, 40, 60), (20, 20, 30)),
            sky_top: VColor::rgb(20, 20, 40),
            sky_bottom: VColor::rgb(10, 10, 20),
            camera: MatchCamera::new(),
            tex_soft: radial_texture(64, |d| (1.0 - d).clamp(0.0, 1.0).powf(1.6), false),
            tex_spark: radial_texture(64, |d| (1.0 - d * 1.15).clamp(0.0, 1.0).powf(0.7), true),
            tex_ring: radial_texture(
                64,
                |d| (1.0 - ((d - 0.72).abs() * 5.0)).clamp(0.0, 1.0),
                false,
            ),
            stage_tex: None,
            preview_stage: None,
            verts: Vec::new(),
            idx: Vec::new(),
            remap: Vec::new(),
            stamp: Vec::new(),
            generation: 1,
            view_proj: Mat4::IDENTITY,
            preview_angle: 0.0,
        }
    }

    /// Forget camera smoothing (call when a new match starts).
    pub fn reset(&mut self) {
        self.camera.reset();
    }

    // ------------------------------------------------------------ stage

    fn ensure_stage(&mut self, gs: &GameState) {
        if self.stage_id == Some(gs.stage.id) {
            return;
        }
        let dressing = crate::model::assets::stage_dressing(gs.stage.id);
        let model: StageModel = stage3d::build_with_dressing(&gs.stage, dressing);
        let lk = model.look;
        // Fighters are lit by a neutral, bright version of the stage light so
        // they read on every stage; the stage itself uses its own.
        let mut fl = Light::default_for(
            (lk.sky_top[0], lk.sky_top[1], lk.sky_top[2]),
            (lk.sky_bottom[0], lk.sky_bottom[1], lk.sky_bottom[2]),
        );
        fl.dir = model.light.dir;
        fl.key = model.light.key;
        self.light = fl;
        self.sky_top = VColor::rgb(lk.sky_top[0], lk.sky_top[1], lk.sky_top[2]);
        self.sky_bottom = VColor::rgb(lk.sky_bottom[0], lk.sky_bottom[1], lk.sky_bottom[2]);
        let tex_bytes = stage3d::texture(lk.tex, 128);
        let tex = Texture2D::from_rgba8(128, 128, &tex_bytes);
        tex.set_filter(FilterMode::Linear);
        // Tile it: macroquad textures clamp by default.
        // SAFETY: called from the main thread between frames, which is the
        // documented way to reach the miniquad context.
        unsafe {
            let gl = get_internal_gl();
            gl.quad_context.texture_set_wrap(
                tex.raw_miniquad_id(),
                miniquad::TextureWrap::Repeat,
                miniquad::TextureWrap::Repeat,
            );
        }
        self.stage_tex = Some(tex.clone());
        self.stage_near = upload_static(
            &model.near,
            &model.palette,
            &model.light,
            lk.haze,
            false,
            Some(&tex),
        );
        self.stage_far = upload_static(
            &model.far,
            &model.palette,
            &model.light,
            lk.haze,
            true,
            None,
        );
        self.stage_id = Some(gs.stage.id);
        self.camera.reset();
    }

    // ------------------------------------------------------------ frame

    /// Draw a whole match frame (3D scene + 2D HUD).
    pub fn draw(&mut self, gs: &GameState, opts: SceneOpts) {
        self.ensure_stage(gs);
        let (w, h) = (screen_width(), screen_height());
        let aspect = w / h.max(1.0);
        self.camera.update(gs, aspect);

        // Sky (2D, behind everything).
        self.draw_sky(w, h, gs);

        // 3D pass.
        let shake = self.camera.shake(gs);
        let cam = Camera3D {
            position: mq(self.camera.pos + shake),
            target: mq(self.camera.target + shake),
            up: vec3(0.0, 1.0, 0.0),
            fovy: self.camera.fovy,
            aspect: Some(aspect),
            projection: Projection::Perspective,
            ..Default::default()
        };
        self.view_proj = cam.matrix();
        set_camera(&cam);

        for m in &self.stage_far {
            draw_mesh(m);
        }
        for m in &self.stage_near {
            draw_mesh(m);
        }
        for f in &gs.fighters {
            self.draw_shadow(f, gs);
        }
        for f in &gs.fighters {
            self.draw_fighter(f, gs.frame);
        }
        self.draw_projectiles(gs);
        for f in &gs.fighters {
            self.draw_overlays(f, opts);
        }
        self.draw_fx(gs);

        // 2D pass: HUD.
        set_default_camera();
        let mut p = MqPainter;
        if gs.hitstop_flash > 0.02 {
            let a = ((gs.hitstop_flash * 40.0) as u8).min(60);
            p.fill_rect(0.0, 0.0, w, h, VColor::rgba(255, 255, 255, a));
        }
        if !opts.hud {
            return;
        }
        self.draw_port_tags(&mut p, gs, opts);
        viz::draw_hud(&mut p, w, h, gs, opts.local_player);
        if opts.training {
            font::draw_text(
                &mut p,
                "TRAINING",
                12.0,
                12.0,
                2.0,
                VColor::rgba(200, 220, 255, 180),
            );
            self.draw_state_labels(&mut p, gs);
        }
        if opts.watermark {
            let t = "OVERFRAME";
            let sc = 2.0;
            font::draw_text(
                &mut p,
                t,
                w - font::text_width(t, sc) - 12.0,
                12.0,
                sc,
                VColor::rgba(255, 255, 255, 70),
            );
        }
        if let Some(winner) = gs.match_over {
            viz::draw_match_over(&mut p, w, h, winner);
        }
    }

    fn draw_sky(&self, w: f32, h: f32, gs: &GameState) {
        let bands = 40;
        for i in 0..bands {
            let t = i as f32 / bands as f32;
            let c = self.sky_top.lerp(self.sky_bottom, t.powf(1.3));
            draw_rectangle(0.0, h * t, w, h / bands as f32 + 1.0, col(c));
        }
        // Stage-specific sky dressing.
        match gs.stage.id {
            StageId::Lattice => {
                // A scatter of faint stars, fixed per stage.
                for i in 0..90u32 {
                    let hsh = i.wrapping_mul(2654435761);
                    let x = (hsh % 1000) as f32 / 1000.0 * w;
                    let y = ((hsh >> 10) % 1000) as f32 / 1000.0 * h * 0.7;
                    let tw = ((gs.frame as f32 * 0.05 + i as f32).sin() * 0.5 + 0.5) * 0.6 + 0.4;
                    let s = 1.0 + ((hsh >> 20) % 2) as f32;
                    draw_rectangle(x, y, s, s, Color::new(0.85, 0.9, 1.0, 0.55 * tw));
                }
            }
            StageId::Meridian => {
                // Low sun.
                let sun = Color::new(1.0, 0.86, 0.6, 0.9);
                draw_texture_ex(
                    &self.tex_soft,
                    w * 0.68 - 160.0,
                    h * 0.30 - 160.0,
                    Color::new(1.0, 0.75, 0.45, 0.55),
                    DrawTextureParams {
                        dest_size: Some(vec2(320.0, 320.0)),
                        ..Default::default()
                    },
                );
                draw_circle(w * 0.68, h * 0.30, 26.0, sun);
            }
            StageId::Tidegate => {
                // Moon and a haze band at the horizon.
                draw_circle(w * 0.22, h * 0.20, 22.0, Color::new(0.9, 0.95, 1.0, 0.85));
                draw_texture_ex(
                    &self.tex_soft,
                    w * 0.22 - 90.0,
                    h * 0.20 - 90.0,
                    Color::new(0.7, 0.85, 1.0, 0.35),
                    DrawTextureParams {
                        dest_size: Some(vec2(180.0, 180.0)),
                        ..Default::default()
                    },
                );
            }
        }
    }

    // ------------------------------------------------------------ fighters

    fn fighter_root(f: &Fighter) -> Xf {
        Xf::new(M3::scale(v3(f.facing, 1.0, 1.0)), v3(f.pos.x, f.pos.y, 0.0))
    }

    fn tint_for(f: &Fighter) -> Tint {
        let mut t = Tint::default();
        if f.anim_flash > 0 || matches!(f.state, State::Hitstun { .. }) {
            t.flash = if f.anim_flash > 0 { 0.65 } else { 0.25 };
        }
        if matches!(f.state, State::ShieldStun { .. } | State::Knockdown) {
            t.grey = 0.4;
        }
        if f.is_intangible() && (f.state_frame / 3) % 2 == 0 && !matches!(f.state, State::LedgeGrab)
        {
            t.alpha = 150;
        }
        t
    }

    fn draw_fighter(&mut self, f: &Fighter, frame: u64) {
        if matches!(f.state, State::Dead) {
            return;
        }
        let model = &self.models[f.character.id.index()];
        let mut pose = anim::fighter_pose(&model.rig, f, &model.style, frame);
        (model.secondary)(&model.rig, &mut pose, f, frame);
        let world = model.rig.world(&pose, &Self::fighter_root(f));
        let tint = Self::tint_for(f);
        self.verts.clear();
        self.idx.clear();
        model.rig.skin(
            &world,
            model.palette(f.palette),
            &self.light,
            &tint,
            &mut self.verts,
            &mut self.idx,
        );
        self.flush_tris(None);
    }

    /// Soft shadow on the platform below the fighter.
    fn draw_shadow(&self, f: &Fighter, gs: &GameState) {
        if matches!(f.state, State::Dead) {
            return;
        }
        let hw = f.character.half_width;
        let mut best: Option<(f32, f32)> = None; // (y, depth)
        for p in &gs.stage.platforms {
            if f.pos.x + hw < p.left || f.pos.x - hw > p.right || p.y > f.pos.y + 2.0 {
                continue;
            }
            let depth = if p.solid { SLAB_DEPTH } else { SOFT_DEPTH };
            if best.map_or(true, |(y, _)| p.y > y) {
                best = Some((p.y, depth));
            }
        }
        let Some((y, depth)) = best else { return };
        let height = (f.pos.y - y).max(0.0);
        let a = (0.55 * (1.0 - height / 140.0)).clamp(0.0, 0.55);
        if a <= 0.01 {
            return;
        }
        let sx = (hw * 4.0 + height * 0.05).min(depth);
        let sz = (hw * 2.6).min(depth * 0.8);
        let c = [0u8, 0, 10, (a * 255.0) as u8];
        let q = |x: f32, z: f32, u: f32, v: f32| Vertex {
            position: vec3(f.pos.x + x, y + 0.25, z),
            uv: vec2(u, v),
            color: c,
            normal: vec4(0.0, 1.0, 0.0, 0.0),
        };
        let m = Mesh {
            vertices: vec![
                q(-sx, sz, 0.0, 0.0),
                q(sx, sz, 1.0, 0.0),
                q(sx, -sz, 1.0, 1.0),
                q(-sx, -sz, 0.0, 1.0),
            ],
            indices: vec![0, 1, 2, 0, 2, 3],
            texture: Some(self.tex_soft.clone()),
        };
        draw_mesh(&m);
    }

    /// Shield bubble, strike glow, training boxes.
    fn draw_overlays(&self, f: &Fighter, opts: SceneOpts) {
        if matches!(f.state, State::Dead) {
            return;
        }
        let bc = f.body_center();
        if matches!(f.state, State::Shield) {
            let frac = (f.shield_health / crate::sim::constants::SHIELD_MAX).clamp(0.1, 1.0);
            let r = f.character.half_width + f.character.height * 0.5 * frac;
            let c = viz::fighter_color(f.character.id.index(), f.palette);
            draw_sphere_ex(
                vec3(bc.x, bc.y, 0.0),
                r,
                None,
                Color::from_rgba(c.r / 2 + 90, c.g / 2 + 110, 255, 80),
                DrawSphereParams {
                    rings: 10,
                    slices: 14,
                    draw_mode: DrawMode::Triangles,
                },
            );
            draw_sphere_wires(
                vec3(bc.x, bc.y, 0.0),
                r * 1.01,
                None,
                Color::from_rgba(200, 230, 255, 70),
            );
        }
        if let Some((hb, world)) = f.active_hitbox() {
            // Strike glow: a bright translucent sphere at the hitbox.
            let c = Color::from_rgba(255, 235, 170, 120);
            draw_sphere_ex(
                vec3(world.x, world.y, 0.0),
                hb.radius * 0.55,
                None,
                c,
                DrawSphereParams {
                    rings: 6,
                    slices: 8,
                    draw_mode: DrawMode::Triangles,
                },
            );
            if opts.training {
                draw_sphere_wires(
                    vec3(world.x, world.y, 0.0),
                    hb.radius,
                    None,
                    Color::from_rgba(255, 70, 70, 200),
                );
            }
        }
        if opts.training {
            draw_sphere_wires(
                vec3(bc.x, bc.y, 0.0),
                f.hurt_radius(),
                None,
                Color::from_rgba(120, 220, 120, 140),
            );
        }
    }

    fn draw_projectiles(&self, gs: &GameState) {
        for pr in &gs.projectiles {
            if !pr.active {
                continue;
            }
            let r = attacks::PROJECTILE_RADIUS;
            draw_sphere_ex(
                vec3(pr.pos.x, pr.pos.y, 0.0),
                r,
                None,
                Color::from_rgba(255, 245, 200, 255),
                DrawSphereParams {
                    rings: 6,
                    slices: 8,
                    draw_mode: DrawMode::Triangles,
                },
            );
            let dir = v3(pr.vel.x, pr.vel.y, 0.0);
            let tail = v3(pr.pos.x, pr.pos.y, 0.0) - dir * 3.0;
            self.billboard(
                tail,
                r * 5.0,
                Color::from_rgba(255, 220, 120, 90),
                &self.tex_soft,
            );
            self.billboard(
                v3(pr.pos.x, pr.pos.y, 0.0),
                r * 4.0,
                Color::from_rgba(255, 240, 180, 150),
                &self.tex_soft,
            );
        }
    }

    fn draw_fx(&self, gs: &GameState) {
        for fx in &gs.fx {
            let t = 1.0 - fx.life as f32 / fx.max_life as f32;
            let p = v3(fx.pos.x, fx.pos.y, 6.0);
            match fx.kind {
                FxKind::Hit => {
                    let r = (6.0 + fx.magnitude.min(30.0) * 0.4) * (0.6 + t * 0.9);
                    let a = (1.0 - t).clamp(0.0, 1.0);
                    self.billboard(
                        p,
                        r * 2.2,
                        Color::new(1.0, 0.85, 0.5, 0.9 * a),
                        &self.tex_spark,
                    );
                    self.billboard(p, r * 1.1, Color::new(1.0, 1.0, 1.0, a), &self.tex_spark);
                    self.billboard(
                        p,
                        r * 2.4 * (0.6 + t),
                        Color::new(1.0, 0.9, 0.7, 0.55 * a),
                        &self.tex_ring,
                    );
                }
                FxKind::Shield => {
                    let r = 14.0 * (0.5 + t);
                    self.billboard(
                        p,
                        r * 2.0,
                        Color::new(0.5, 0.8, 1.0, 0.7 * (1.0 - t)),
                        &self.tex_ring,
                    );
                }
                FxKind::Blast => {
                    let r = (14.0 + fx.magnitude.min(40.0) * 0.5) * (0.3 + t * 1.4);
                    let a = 1.0 - t;
                    self.billboard(
                        p,
                        r * 2.0,
                        Color::new(1.0, 0.75, 0.45, 0.9 * a),
                        &self.tex_spark,
                    );
                    self.billboard(
                        p,
                        r * 3.0,
                        Color::new(1.0, 0.9, 0.7, 0.7 * a),
                        &self.tex_ring,
                    );
                }
                FxKind::Dust => {
                    let r = 6.0 * (0.6 + t * 1.2);
                    self.billboard(
                        v3(fx.pos.x, fx.pos.y + r * 0.4, 8.0),
                        r * 2.0,
                        Color::new(0.85, 0.85, 0.9, 0.45 * (1.0 - t)),
                        &self.tex_soft,
                    );
                }
            }
        }
    }

    /// A camera-facing textured quad.
    fn billboard(&self, centre: V3, size: f32, color: Color, tex: &Texture2D) {
        let fwd = (self.camera.target - self.camera.pos).norm();
        let right = fwd.cross(V3::Y).norm() * (size * 0.5);
        let up = right.cross(fwd).norm() * (size * 0.5);
        let c: [u8; 4] = color.into();
        let q = |p: V3, u: f32, v: f32| Vertex {
            position: mq(p),
            uv: vec2(u, v),
            color: c,
            normal: vec4(0.0, 0.0, 1.0, 0.0),
        };
        let m = Mesh {
            vertices: vec![
                q(centre - right - up, 0.0, 1.0),
                q(centre + right - up, 1.0, 1.0),
                q(centre + right + up, 1.0, 0.0),
                q(centre - right + up, 0.0, 0.0),
            ],
            indices: vec![0, 1, 2, 0, 2, 3],
            texture: Some(tex.clone()),
        };
        draw_mesh(&m);
    }

    // ------------------------------------------------------------ 2D labels

    /// Project a world point to screen pixels (None if behind the camera).
    pub fn project(&self, p: V3) -> Option<(f32, f32)> {
        let clip = self.view_proj * vec4(p.x, p.y, p.z, 1.0);
        if clip.w <= 0.0 {
            return None;
        }
        let ndc = vec2(clip.x / clip.w, clip.y / clip.w);
        Some((
            (ndc.x * 0.5 + 0.5) * screen_width(),
            (1.0 - (ndc.y * 0.5 + 0.5)) * screen_height(),
        ))
    }

    /// "P1"/"P2" markers above each fighter's head.
    fn draw_port_tags(&self, p: &mut MqPainter, gs: &GameState, opts: SceneOpts) {
        for (i, f) in gs.fighters.iter().enumerate() {
            if matches!(f.state, State::Dead) {
                continue;
            }
            let top = v3(f.pos.x, f.pos.y + f.character.height + 6.0, 0.0);
            let Some((sx, sy)) = self.project(top) else {
                continue;
            };
            let c = viz::PORT_COLORS[i % 4];
            let label = if opts.local_player == Some(i) {
                format!("P{} YOU", i + 1)
            } else {
                format!("P{}", i + 1)
            };
            let tw = font::text_width(&label, 1.5);
            font::draw_text(p, &label, sx - tw * 0.5, sy - 20.0, 1.5, c);
            // Small pointer.
            p.line(sx - 5.0, sy - 8.0, sx, sy - 2.0, 2.0, c);
            p.line(sx + 5.0, sy - 8.0, sx, sy - 2.0, 2.0, c);
        }
    }

    fn draw_state_labels(&self, p: &mut MqPainter, gs: &GameState) {
        for f in &gs.fighters {
            if matches!(f.state, State::Dead) {
                continue;
            }
            let label = viz::state_name(&f.state);
            let Some((sx, sy)) = self.project(v3(f.pos.x, f.pos.y - 6.0, 0.0)) else {
                continue;
            };
            font::draw_text(
                p,
                label,
                sx - font::text_width(label, 1.0) * 0.5,
                sy,
                1.0,
                VColor::rgba(220, 240, 220, 200),
            );
        }
    }

    // ------------------------------------------------------------ viewer

    /// Animation viewer: one fighter on a pedestal in a neutral studio,
    /// orbiting slowly. `f` is a synthetic fighter whose state the caller
    /// drives; `label` is drawn under it.
    pub fn draw_viewer(&mut self, f: &Fighter, frame: u64, angle_deg: f32, label: &str, sub: &str) {
        let (w, h) = (screen_width(), screen_height());
        // Studio backdrop.
        let top = VColor::rgb(34, 36, 56);
        let bottom = VColor::rgb(12, 12, 20);
        let bands = 24;
        for i in 0..bands {
            let t = i as f32 / bands as f32;
            draw_rectangle(
                0.0,
                h * t,
                w,
                h / bands as f32 + 1.0,
                col(top.lerp(bottom, t)),
            );
        }
        let height = f.character.height;
        let a = angle_deg.to_radians();
        let dist = height * 3.4;
        let cam = Camera3D {
            position: vec3(a.sin() * dist, height * 0.75, a.cos() * dist),
            target: vec3(0.0, height * 0.5, 0.0),
            up: vec3(0.0, 1.0, 0.0),
            fovy: 30.0f32.to_radians(),
            aspect: Some(w / h.max(1.0)),
            projection: Projection::Perspective,
            ..Default::default()
        };
        self.view_proj = cam.matrix();
        set_camera(&cam);
        self.light = Light::default_for((120, 130, 170), (40, 40, 60));
        // Pedestal + floor grid.
        let disc = MeshData::cylinder(height * 1.3, height * 1.35, 1.6, 28, slot::SECONDARY)
            .flat()
            .translate(v3(0.0, -0.8, 0.0));
        let pal = crate::model::lighting::Palette::new(
            "studio",
            [60, 64, 90],
            [52, 56, 82],
            [0, 0, 0],
            [0, 0, 0],
            [0, 0, 0],
            [0, 0, 0],
            [0, 0, 0],
            [0, 0, 0],
        );
        for m in upload_static(&disc, &pal, &self.light, [30, 30, 40], false, None) {
            draw_mesh(&m);
        }
        for i in -6..=6 {
            let x = i as f32 * height * 0.2;
            let c = Color::from_rgba(120, 130, 170, 40);
            draw_line_3d(vec3(x, 0.05, -height * 1.2), vec3(x, 0.05, height * 1.2), c);
            draw_line_3d(vec3(-height * 1.2, 0.05, x), vec3(height * 1.2, 0.05, x), c);
        }
        // Shadow on the pedestal.
        let hw = f.character.half_width;
        let c = [0u8, 0, 10, 110];
        let q = |x: f32, z: f32, u: f32, v: f32| Vertex {
            position: vec3(f.pos.x + x, 0.15, z),
            uv: vec2(u, v),
            color: c,
            normal: vec4(0.0, 1.0, 0.0, 0.0),
        };
        let (sx, sz) = (hw * 3.6, hw * 2.6);
        draw_mesh(&Mesh {
            vertices: vec![
                q(-sx, sz, 0.0, 0.0),
                q(sx, sz, 1.0, 0.0),
                q(sx, -sz, 1.0, 1.0),
                q(-sx, -sz, 0.0, 1.0),
            ],
            indices: vec![0, 1, 2, 0, 2, 3],
            texture: Some(self.tex_soft.clone()),
        });
        self.draw_fighter(f, frame);
        self.draw_overlays(
            f,
            SceneOpts {
                hitboxes: true,
                ..SceneOpts::default()
            },
        );
        set_default_camera();
        let mut p = MqPainter;
        let sc = 3.0;
        font::draw_text(
            &mut p,
            label,
            w * 0.5 - font::text_width(label, sc) * 0.5,
            h - 96.0,
            sc,
            VColor::rgb(255, 220, 120),
        );
        font::draw_text(
            &mut p,
            sub,
            w * 0.5 - font::text_width(sub, 1.6) * 0.5,
            h - 62.0,
            1.6,
            VColor::rgba(200, 210, 230, 200),
        );
    }

    // ------------------------------------------------------------ preview

    /// Draw a slowly turning idle model into a screen rectangle (menus).
    pub fn draw_preview(&mut self, id: CharacterId, palette: u8, x: f32, y: f32, w: f32, h: f32) {
        self.preview_angle += 0.6;
        let angle = self.preview_angle;
        let model = &self.models[id.index()];
        let mut pose = anim::stance(&model.rig, &model.style, angle * 2.0);
        let dummy = Fighter::new(id.data(), 0, crate::sim::math::Vec2::ZERO);
        (model.secondary)(&model.rig, &mut pose, &dummy, angle as u64);
        let root = Xf::new(M3::rot_y(angle), V3::ZERO);
        let world = model.rig.world(&pose, &root);
        let light = Light::default_for((90, 100, 140), (30, 30, 40));
        self.verts.clear();
        self.idx.clear();
        model.rig.skin(
            &world,
            model.palette(palette),
            &light,
            &Tint::default(),
            &mut self.verts,
            &mut self.idx,
        );
        let dpi = miniquad::window::dpi_scale();
        let sh = screen_height();
        let height = id.data().height;
        let cam = Camera3D {
            position: vec3(0.0, height * 0.55, height * 2.6),
            target: vec3(0.0, height * 0.48, 0.0),
            up: vec3(0.0, 1.0, 0.0),
            fovy: 32.0f32.to_radians(),
            aspect: Some(w / h.max(1.0)),
            projection: Projection::Perspective,
            viewport: Some((
                (x * dpi) as i32,
                ((sh - y - h) * dpi) as i32,
                (w * dpi) as i32,
                (h * dpi) as i32,
            )),
            ..Default::default()
        };
        set_camera(&cam);
        // Pedestal.
        let disc = MeshData::cylinder(height * 0.42, height * 0.45, 1.2, 18, slot::SECONDARY)
            .flat()
            .translate(v3(0.0, -0.6, 0.0));
        let pal = crate::model::lighting::Palette::new(
            "pedestal",
            [60, 64, 90],
            [46, 50, 72],
            [0, 0, 0],
            [0, 0, 0],
            [0, 0, 0],
            [0, 0, 0],
            [0, 0, 0],
            [0, 0, 0],
        );
        for m in upload_static(&disc, &pal, &light, [30, 30, 40], false, None) {
            draw_mesh(&m);
        }
        self.flush_tris(None);
        set_default_camera();
    }

    /// Draw a whole stage, slowly orbiting, into a screen rectangle.
    pub fn draw_stage_preview(&mut self, id: StageId, x: f32, y: f32, w: f32, h: f32) {
        if self.preview_stage.as_ref().map(|c| c.id) != Some(id) {
            let stage = crate::sim::stage::Stage::by_id(id);
            let model = stage3d::build(&stage);
            let lk = model.look;
            let tex_bytes = stage3d::texture(lk.tex, 128);
            let tex = Texture2D::from_rgba8(128, 128, &tex_bytes);
            tex.set_filter(FilterMode::Linear);
            // SAFETY: main thread, between draw calls (see ensure_stage).
            unsafe {
                let gl = get_internal_gl();
                gl.quad_context.texture_set_wrap(
                    tex.raw_miniquad_id(),
                    miniquad::TextureWrap::Repeat,
                    miniquad::TextureWrap::Repeat,
                );
            }
            let near = upload_static(
                &model.near,
                &model.palette,
                &model.light,
                lk.haze,
                false,
                Some(&tex),
            );
            let far = upload_static(
                &model.far,
                &model.palette,
                &model.light,
                lk.haze,
                true,
                None,
            );
            self.preview_stage = Some(PreviewStage {
                id,
                near,
                far,
                sky_top: lk.sky_top,
                sky_bottom: lk.sky_bottom,
            });
        }
        let Some(ps) = self.preview_stage.as_ref() else {
            return;
        };
        let (near, far, top, bottom) = (&ps.near, &ps.far, ps.sky_top, ps.sky_bottom);
        // Sky inside the frame (2D), then the 3D orbit view.
        let bands = 12;
        for i in 0..bands {
            let t = i as f32 / bands as f32;
            let c = VColor::rgb(top[0], top[1], top[2])
                .lerp(VColor::rgb(bottom[0], bottom[1], bottom[2]), t);
            draw_rectangle(x, y + h * t, w, h / bands as f32 + 1.0, col(c));
        }
        self.preview_angle += 0.6;
        let a = (self.preview_angle * 0.35).to_radians();
        let dist = 520.0;
        let dpi = miniquad::window::dpi_scale();
        let sh = screen_height();
        let cam = Camera3D {
            position: vec3(a.sin() * dist * 0.55, 150.0, a.cos() * dist),
            target: vec3(0.0, 30.0, 0.0),
            up: vec3(0.0, 1.0, 0.0),
            fovy: 36.0f32.to_radians(),
            aspect: Some(w / h.max(1.0)),
            projection: Projection::Perspective,
            viewport: Some((
                (x * dpi) as i32,
                ((sh - y - h) * dpi) as i32,
                (w * dpi) as i32,
                (h * dpi) as i32,
            )),
            ..Default::default()
        };
        set_camera(&cam);
        for m in far {
            draw_mesh(m);
        }
        for m in near {
            draw_mesh(m);
        }
        set_default_camera();
    }

    // ------------------------------------------------------------ batching

    /// Draw the scratch `verts`/`idx` buffers in batches small enough for
    /// macroquad's draw-call limits.
    fn flush_tris(&mut self, texture: Option<&Texture2D>) {
        let n = self.verts.len();
        if self.remap.len() < n {
            self.remap.resize(n, 0);
            self.stamp.resize(n, 0);
        }
        let mut start = 0;
        while start < self.idx.len() {
            let end = (start + MAX_BATCH_INDICES).min(self.idx.len());
            self.generation = self.generation.wrapping_add(1);
            let gen = self.generation;
            let mut vertices: Vec<Vertex> = Vec::with_capacity(end - start);
            let mut indices: Vec<u16> = Vec::with_capacity(end - start);
            for &i in &self.idx[start..end] {
                let i = i as usize;
                if self.stamp[i] != gen {
                    self.stamp[i] = gen;
                    self.remap[i] = vertices.len() as u32;
                    let v = &self.verts[i];
                    vertices.push(Vertex {
                        position: mq(v.pos),
                        uv: vec2(v.uv[0], v.uv[1]),
                        color: v.rgba,
                        normal: vec4(v.nrm.x, v.nrm.y, v.nrm.z, 0.0),
                    });
                }
                indices.push(self.remap[i] as u16);
            }
            let m = Mesh {
                vertices,
                indices,
                texture: texture.cloned(),
            };
            draw_mesh(&m);
            start = end;
        }
    }
}

impl Default for Scene3D {
    fn default() -> Self {
        Self::new()
    }
}

#[inline]
fn mq(v: V3) -> Vec3 {
    vec3(v.x, v.y, v.z)
}

/// Bake lighting into a static mesh and split it into GPU-sized batches.
fn upload_static(
    data: &MeshData,
    palette: &crate::model::lighting::Palette,
    light: &Light,
    sky: [u8; 3],
    fogged: bool,
    texture: Option<&Texture2D>,
) -> Vec<Mesh> {
    let mut out = Vec::new();
    let n = data.pos.len();
    let mut remap = vec![u32::MAX; n];
    let mut start = 0;
    while start < data.idx.len() {
        let end = (start + MAX_BATCH_INDICES).min(data.idx.len());
        for r in remap.iter_mut() {
            *r = u32::MAX;
        }
        let mut vertices: Vec<Vertex> = Vec::new();
        let mut indices: Vec<u16> = Vec::with_capacity(end - start);
        for &i in &data.idx[start..end] {
            let i = i as usize;
            if remap[i] == u32::MAX {
                remap[i] = vertices.len() as u32;
                let p = data.pos[i];
                let mut c = shade_static(
                    palette.color(data.slot[i]),
                    data.nrm[i],
                    p,
                    light,
                    data.slot[i],
                );
                if fogged {
                    let f = stage3d::fog(p);
                    for k in 0..3 {
                        c[k] = (c[k] as f32 * (1.0 - f) + sky[k] as f32 * f) as u8;
                    }
                }
                vertices.push(Vertex {
                    position: mq(p),
                    uv: vec2(data.uv[i][0], data.uv[i][1]),
                    color: c,
                    normal: vec4(data.nrm[i].x, data.nrm[i].y, data.nrm[i].z, 0.0),
                });
            }
            indices.push(remap[i] as u16);
        }
        out.push(Mesh {
            vertices,
            indices,
            texture: texture.cloned(),
        });
        start = end;
    }
    out
}

/// A square RGBA texture whose alpha follows `f(distance from centre 0..1)`;
/// `additive_core` whitens the centre for sparks.
fn radial_texture(size: u16, f: impl Fn(f32) -> f32, additive_core: bool) -> Texture2D {
    let mut bytes = vec![0u8; size as usize * size as usize * 4];
    for y in 0..size {
        for x in 0..size {
            let dx = (x as f32 + 0.5) / size as f32 * 2.0 - 1.0;
            let dy = (y as f32 + 0.5) / size as f32 * 2.0 - 1.0;
            let d = (dx * dx + dy * dy).sqrt();
            let a = f(d).clamp(0.0, 1.0);
            let i = (y as usize * size as usize + x as usize) * 4;
            let core = if additive_core {
                (1.0 - d * 2.0).clamp(0.0, 1.0)
            } else {
                0.0
            };
            bytes[i] = 255;
            bytes[i + 1] = 255;
            bytes[i + 2] = (255.0 * (0.85 + 0.15 * core)) as u8;
            bytes[i + 3] = (a * 255.0) as u8;
        }
    }
    let t = Texture2D::from_rgba8(size, size, &bytes);
    t.set_filter(FilterMode::Linear);
    t
}
