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
use crate::model::render_eval::{self, PortState};
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
    /// Inverted-hull outline material (None if the shader failed to build).
    outline: Option<Material>,
    /// Render-only per-port state: smoothed facing and strike trails.
    ports: Vec<PortFx>,
    /// Capture overrides (see `render::capture`): drawing surface size when
    /// not the window, a fixed outline width in world units, silhouette
    /// mode (unlit black), and whether the extras lag layer runs.
    pub(crate) viewport: Option<(f32, f32)>,
    pub(crate) outline_width: Option<f32>,
    pub(crate) silhouette: bool,
    pub(crate) lag_enabled: bool,
}

/// Per-fighter render state that is *not* part of the simulation (so it is
/// never saved/rolled back; it just eases visuals between sim frames).
#[derive(Clone, Debug, Default)]
struct PortFx {
    /// Eased facing, frame continuity and extras lag: the shared render
    /// evaluation state (`model::render_eval`), also used by measurements.
    state: PortState,
    /// Recent world positions of the striking limb's tip while a hitbox is
    /// active, newest last. Drawn as a fading ribbon.
    trail: Vec<(V3, V3)>,
}

const OUTLINE_VERTEX: &str = r#"#version 100
attribute vec3 position;
attribute vec2 texcoord;
attribute vec4 color0;
attribute vec4 normal;
varying lowp vec4 color;
uniform mat4 Model;
uniform mat4 Projection;
uniform float Width;
void main() {
    vec3 n = normal.xyz;
    float l = length(n);
    if (l > 0.0001) { n = n / l; }
    gl_Position = Projection * Model * vec4(position + n * Width, 1);
    color = color0 / 255.0;
}"#;

const OUTLINE_FRAGMENT: &str = r#"#version 100
varying lowp vec4 color;
void main() {
    gl_FragColor = color;
}"#;

fn make_outline_material() -> Option<Material> {
    load_material(
        ShaderSource::Glsl {
            vertex: OUTLINE_VERTEX,
            fragment: OUTLINE_FRAGMENT,
        },
        MaterialParams {
            pipeline_params: PipelineParams {
                cull_face: miniquad::CullFace::Front,
                depth_test: miniquad::Comparison::LessOrEqual,
                depth_write: true,
                color_blend: Some(miniquad::BlendState::new(
                    miniquad::Equation::Add,
                    miniquad::BlendFactor::Value(miniquad::BlendValue::SourceAlpha),
                    miniquad::BlendFactor::OneMinusValue(miniquad::BlendValue::SourceAlpha),
                )),
                ..Default::default()
            },
            uniforms: vec![UniformDesc::new("Width", UniformType::Float1)],
            textures: vec![],
        },
    )
    .map_err(|e| eprintln!("outline shader unavailable: {e:?}"))
    .ok()
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
            outline: make_outline_material(),
            ports: Vec::new(),
            viewport: None,
            outline_width: None,
            silhouette: false,
            lag_enabled: true,
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

    /// Root transform: the model faces +X and turns about Y to face left
    /// (a real turn, not a mirror — a few frames long, purely visual).
    /// Evaluate what this fighter looks like on `frame` (advancing its
    /// render history) without drawing it — the capture tool replays a
    /// case's earlier ticks this way to reconstruct the render state of the
    /// tick it draws.
    pub(crate) fn evaluate_fighter(&mut self, f: &Fighter, frame: u64) -> render_eval::Evaluated {
        let lag = self.lag_enabled;
        let i = f.port.min(7);
        if self.ports.len() <= i {
            self.ports.resize(i + 1, PortFx::default());
        }
        // Disjoint field borrows: the model table and the port state.
        let model = &self.models[f.character.id.index()];
        let port = &mut self.ports[i];
        let ev = render_eval::evaluate(model, &mut port.state, f, frame, lag);
        if ev.fresh {
            port.trail.clear();
        }
        ev
    }

    /// Forget every port's render history (eased facing, trails, extras
    /// lag). The capture tool calls this before reconstructing a tick.
    pub(crate) fn reset_render_history(&mut self) {
        self.ports.clear();
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
        let ev = self.evaluate_fighter(f, frame);
        self.draw_posed(f, frame, &ev.pose, &ev.root);
    }

    /// Draw a fighter and, for the diagnostics' player, measure the contact
    /// piece on the *same* evaluated pose and root that were drawn.
    fn draw_fighter_measured(
        &mut self,
        f: &Fighter,
        frame: u64,
        query: Option<&ContactQuery>,
    ) -> DiagOut {
        if matches!(f.state, State::Dead) {
            return DiagOut::default();
        }
        let ev = self.evaluate_fighter(f, frame);
        let mut out = DiagOut::default();
        let model = &self.models[f.character.id.index()];
        if query.map(|q| q.port == f.port).unwrap_or(false) {
            let q = query.unwrap();
            let hb = q.hitbox.as_ref().map(|(id, c, r)| (id.as_str(), *c, *r));
            out.contact = crate::model::contact::measure_posed(
                model,
                f,
                &ev.pose,
                &ev.root,
                ev.eased_facing,
                q.action,
                hb,
            );
        }
        // The feet on the drawn pose, against the plane the fighter stands
        // on -- independent of whether a contact query was requested for
        // this port. A fixture with no attack in progress (idle, crouch,
        // hitstun, land lag...) still needs real foot-support evidence:
        // gating this behind `query` left it an empty Vec there, which a
        // fold-to-minimum downstream turned into f32::MAX and drew as if
        // it were a measurement instead of "no data".
        out.support = crate::model::contact::foot_support(model, &ev.pose, &ev.root, f.pos.y);
        self.draw_posed(f, frame, &ev.pose, &ev.root);
        out
    }

    /// Draw a fighter in an explicit pose under an explicit root transform
    /// (the capture tool uses this for rest poses and fixed yaws).
    pub(crate) fn draw_posed(
        &mut self,
        f: &Fighter,
        _frame: u64,
        pose: &crate::model::rig::Pose,
        root: &Xf,
    ) {
        let i = f.port.min(7);
        if self.ports.len() <= i {
            self.ports.resize(i + 1, PortFx::default());
        }
        let model = &self.models[f.character.id.index()];
        let world = model.rig.world(pose, root);
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
        if self.silhouette {
            for v in &mut self.verts {
                v.rgba = [0, 0, 0, 255];
            }
        }
        // Strike trail: remember the striking limb's tip while active.
        let limb_tip = f.active_hitbox().map(|_| {
            let spec = anim::strike_spec(match f.state {
                State::Attack { id, .. } | State::Throw { id } => id,
                _ => crate::sim::attacks::MoveId::Jab,
            });
            let (tip, base) = match spec.limb {
                anim::Limb::ArmL => ("hand_l", "forearm_l"),
                anim::Limb::LegR | anim::Limb::BothLegs => ("foot_r", "shin_r"),
                anim::Limb::LegL => ("foot_l", "shin_l"),
                anim::Limb::Body => ("chest", "hips"),
                _ => ("hand_r", "forearm_r"),
            };
            (
                model
                    .rig
                    .joint(&world, tip)
                    .unwrap_or(v3(f.pos.x, f.pos.y, 0.0)),
                model
                    .rig
                    .joint(&world, base)
                    .unwrap_or(v3(f.pos.x, f.pos.y, 0.0)),
            )
        });
        let accent = model.palette(f.palette).color(slot::ACCENT);
        let dark = model.palette(f.palette).color(slot::DARK);
        {
            let i = f.port.min(7);
            let p = &mut self.ports[i];
            match limb_tip {
                Some(t) => {
                    if p.trail.last().map_or(true, |l| (l.0 - t.0).len() > 0.01) {
                        p.trail.push(t);
                    }
                    if p.trail.len() > 10 {
                        p.trail.remove(0);
                    }
                }
                None => {
                    if !p.trail.is_empty() {
                        p.trail.remove(0);
                    }
                }
            }
        }
        self.flush_tris(None);
        if self.silhouette {
            return;
        }
        // Toon outline: inverted hull, pushed out along normals in the shader.
        if let Some(mat) = self.outline.clone() {
            let dist = (self.camera.pos - self.camera.target).len();
            let width = self
                .outline_width
                .unwrap_or_else(|| (dist * 0.0011).clamp(0.18, 0.7));
            mat.set_uniform("Width", width);
            let oc = [
                (dark[0] as f32 * 0.45) as u8,
                (dark[1] as f32 * 0.45) as u8,
                (dark[2] as f32 * 0.5) as u8,
                tint.alpha,
            ];
            for v in &mut self.verts {
                v.rgba = oc;
            }
            gl_use_material(&mat);
            self.flush_tris(None);
            gl_use_default_material();
        }
        // Trail ribbon.
        let trail = self.ports[f.port.min(7)].trail.clone();
        if trail.len() >= 2 {
            let n = trail.len();
            let mut verts = Vec::with_capacity(n * 2);
            let mut idx = Vec::with_capacity((n - 1) * 6);
            for (k, (tip, base)) in trail.iter().enumerate() {
                let a = (k as f32 / (n - 1) as f32).powi(2) * 0.55;
                let c = [accent[0], accent[1], accent[2], (a * 255.0) as u8];
                let mid = tip.lerp(*base, 0.45);
                verts.push(Vertex {
                    position: mq(*tip),
                    uv: vec2(0.0, 0.0),
                    color: c,
                    normal: vec4(0.0, 0.0, 1.0, 0.0),
                });
                verts.push(Vertex {
                    position: mq(mid),
                    uv: vec2(0.0, 1.0),
                    color: c,
                    normal: vec4(0.0, 0.0, 1.0, 0.0),
                });
            }
            for k in 0..(n as u16 - 1) {
                let a = k * 2;
                idx.extend_from_slice(&[a, a + 1, a + 3, a, a + 3, a + 2]);
                // both windings so it is visible from either side
                idx.extend_from_slice(&[a, a + 3, a + 1, a, a + 2, a + 3]);
            }
            draw_mesh(&Mesh {
                vertices: verts,
                indices: idx,
                texture: None,
            });
        }
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
                    // Impact frame: a hard white flash for the first two
                    // frames, then a warm spark + ring that fades, plus
                    // impact lines streaking along the launch direction.
                    let strong = fx.magnitude.min(40.0);
                    let r = (5.0 + strong * 0.45) * (0.7 + t * 0.8);
                    let a = (1.0 - t).clamp(0.0, 1.0);
                    if fx.life + 2 >= fx.max_life {
                        self.billboard(
                            p,
                            r * 3.0,
                            Color::new(1.0, 1.0, 1.0, 0.95),
                            &self.tex_spark,
                        );
                    }
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
                    // Impact lines: a fan around the launch direction, growing
                    // and thinning as they fade.
                    let dir = v3(fx.dir.x, fx.dir.y, 0.0);
                    let n = if strong > 18.0 { 7 } else { 5 };
                    for k in 0..n {
                        let spread = (k as f32 / (n - 1) as f32 - 0.5) * 1.6;
                        let (sn, cs) = spread.sin_cos();
                        let d = v3(dir.x * cs - dir.y * sn, dir.x * sn + dir.y * cs, 0.0);
                        let len = (r * (1.6 + t * 2.6)) * (0.7 + 0.3 * (k % 2) as f32);
                        let start = p + d * (r * 0.8 + t * r * 1.5);
                        let width = (r * 0.22 * (1.0 - t)).max(0.4);
                        self.streak(
                            start,
                            start + d * len,
                            width,
                            Color::new(1.0, 0.95, 0.8, 0.8 * a),
                        );
                    }
                }
                FxKind::Shield => {
                    let r = 14.0 * (0.5 + t);
                    self.billboard(
                        p,
                        r * 2.0,
                        Color::new(0.5, 0.8, 1.0, 0.7 * (1.0 - t)),
                        &self.tex_ring,
                    );
                    // Short streaks skidding back from the shield.
                    let d = v3(fx.dir.x, 0.2, 0.0);
                    self.streak(
                        p,
                        p + d * (10.0 + t * 14.0),
                        1.2 * (1.0 - t),
                        Color::new(0.7, 0.9, 1.0, 0.6 * (1.0 - t)),
                    );
                }
                FxKind::Powershield => {
                    let r = 18.0 * (0.6 + t * 1.2);
                    let a = 1.0 - t;
                    self.billboard(
                        p,
                        r * 2.4,
                        Color::new(1.0, 1.0, 1.0, 0.9 * a),
                        &self.tex_ring,
                    );
                    self.billboard(
                        p,
                        r * 1.4,
                        Color::new(0.8, 0.95, 1.0, 0.7 * a),
                        &self.tex_spark,
                    );
                }
                FxKind::Clank => {
                    // Clash spark: a bright cross of four short streaks and
                    // a small flash, between the two blades.
                    let a = 1.0 - t;
                    let r = (8.0 + fx.magnitude * 0.4) * (0.6 + t);
                    self.billboard(
                        p,
                        r * 1.6,
                        Color::new(1.0, 0.98, 0.85, 0.9 * a),
                        &self.tex_spark,
                    );
                    for k in 0..4 {
                        let ang =
                            std::f32::consts::FRAC_PI_4 + k as f32 * std::f32::consts::FRAC_PI_2;
                        let d = v3(ang.cos(), ang.sin(), 0.0);
                        self.streak(
                            p + d * (r * 0.3),
                            p + d * (r * (1.2 + t)),
                            (1.4 * a).max(0.3),
                            Color::new(1.0, 0.95, 0.75, 0.85 * a),
                        );
                    }
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
                    // Puffs drift along the effect direction (up for landings,
                    // backwards for dashes and wavedashes).
                    let r = (4.0 + fx.magnitude.min(20.0) * 0.25) * (0.6 + t * 1.3);
                    let drift = v3(fx.dir.x, fx.dir.y, 0.0) * (t * 10.0);
                    for k in 0..3 {
                        let off = v3(
                            ((k as f32) - 1.0) * r * 0.9,
                            r * 0.4 + k as f32 * 0.6,
                            8.0 + k as f32,
                        );
                        self.billboard(
                            v3(fx.pos.x, fx.pos.y, 0.0) + off + drift,
                            r * 1.8,
                            Color::new(0.85, 0.85, 0.9, 0.35 * (1.0 - t)),
                            &self.tex_soft,
                        );
                    }
                }
                FxKind::Land | FxKind::Jump | FxKind::Swing | FxKind::Tech => {}
            }
        }
    }

    /// A thin quad from `a` to `b` in the play plane (impact lines, skids).
    fn streak(&self, a: V3, b: V3, width: f32, color: Color) {
        let d = b - a;
        let l = d.len();
        if l < 1e-3 {
            return;
        }
        let n = v3(-d.y / l, d.x / l, 0.0) * (width * 0.5);
        let c: [u8; 4] = color.into();
        let q = |p: V3| Vertex {
            position: mq(p),
            uv: vec2(0.5, 0.5),
            color: c,
            normal: vec4(0.0, 0.0, 1.0, 0.0),
        };
        let m = Mesh {
            vertices: vec![q(a - n), q(b - n * 0.3), q(b + n * 0.3), q(a + n)],
            indices: vec![0, 1, 2, 0, 2, 3, 0, 2, 1, 0, 3, 2],
            texture: Some(self.tex_soft.clone()),
        };
        draw_mesh(&m);
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
        let (w, h) = self
            .viewport
            .unwrap_or_else(|| (screen_width(), screen_height()));
        Some(((ndc.x * 0.5 + 0.5) * w, (1.0 - (ndc.y * 0.5 + 0.5)) * h))
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

// ---------------------------------------------------------------- captures

/// A fixed orthographic camera in world units (docs/art/capture-suite.json).
#[derive(Clone, Copy, Debug)]
pub(crate) struct FixedCam {
    pub eye: V3,
    pub target: V3,
    /// Vertical extent of the view in world units.
    pub ortho_height: f32,
}

impl FixedCam {
    fn camera(&self, rt: &RenderTarget) -> Camera3D {
        Camera3D {
            position: mq(self.eye),
            target: mq(self.target),
            up: vec3(0.0, 1.0, 0.0),
            fovy: self.ortho_height,
            aspect: Some(rt.texture.width() / rt.texture.height().max(1.0)),
            projection: Projection::Orthographics,
            render_target: Some(rt.clone()),
            ..Default::default()
        }
    }

    /// Outline width for this camera: 1.5 px at 1080p-equivalent density,
    /// clamped to FORMAT.md's 0.08–0.28 world units.
    pub fn outline_width(&self, rt_height: f32) -> f32 {
        let px = self.ortho_height / rt_height.max(1.0);
        (1.5 * px * (rt_height / 1080.0)).clamp(0.08, 0.28)
    }
}

/// 2D camera that maps `(0,0)-(w,h)` (y down) onto a render target in the
/// same orientation as the 3D pass, so `get_texture_data` reads one
/// consistent, bottom-up image.
pub(crate) fn rt_camera_2d(rt: &RenderTarget) -> Camera2D {
    let (w, h) = (rt.texture.width(), rt.texture.height());
    Camera2D {
        target: vec2(w * 0.5, h * 0.5),
        zoom: vec2(2.0 / w, -2.0 / h),
        render_target: Some(rt.clone()),
        ..Default::default()
    }
}

/// How a model capture is dressed.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ModelLook {
    /// Black object on white, unlit, no outline, no backdrop.
    pub silhouette: bool,
    /// Studio gradient behind the model (else the caller's clear colour).
    pub backdrop: bool,
    /// Draw the fighter's overlays (hitboxes / hurt capsule).
    pub hitboxes: bool,
}

/// Measured overlays for a diagnostic capture: what the simulation tests
/// and where the model's contact piece is, drawn as wireframes over the
/// scene (never in normal play).
#[derive(Clone, Debug, Default)]
pub(crate) struct Diagnostics {
    /// Hurt capsules of every fighter.
    pub capsules: bool,
    /// Runtime hitbox of player 0 this tick (x, y, radius).
    pub hitbox: Option<(f32, f32, f32)>,
    /// Projectile hitbox (x, y, radius).
    pub projectile: Option<(f32, f32, f32)>,
    /// Measure the contact piece of one fighter on the pose actually drawn.
    pub contact: Option<ContactQuery>,
    /// Which fighter's foot-support measurement to report, independent of
    /// `contact`'s port: `draw_fighter_measured` computes support for every
    /// drawn fighter (a fixture with no attack in progress still needs real
    /// foot-support evidence), so this says which one the caller wants
    /// back. Never inferred from "whichever came back non-empty" -- with
    /// two live fighters that is now *every* fighter, and picking by
    /// arrival order silently returns the wrong one (see `select_diag`'s
    /// doc comment).
    pub support_port: Option<usize>,
}

/// What to measure: which port, for which action, against which hitbox.
#[derive(Clone, Debug)]
pub(crate) struct ContactQuery {
    pub port: usize,
    pub action: crate::sim::attacks::MoveId,
    pub hitbox: Option<(String, [f32; 2], f32)>,
}

/// What a diagnostic draw measured on the pose it actually drew.
#[derive(Default, Clone, Debug)]
pub(crate) struct DiagOut {
    pub contact: Option<crate::model::contact::ContactMeasure>,
    /// Foot bounds and support-plane distances of the queried fighter.
    pub support: Vec<crate::model::contact::FootSupport>,
}

/// Pick the final `DiagOut` from one `(port, DiagOut)` per drawn fighter.
///
/// `contact` and `support` are selected **independently**, each by an
/// explicit port match -- never by "whichever fighter's output happened to
/// look non-trivial last". That heuristic used to work only by accident:
/// `draw_fighter_measured` once left `support` empty for every fighter but
/// the queried one, so a second, live fighter never had anything to
/// overwrite the real measurement with. Once support became unconditional
/// (every drawn fighter needs real foot-support evidence, attack or not),
/// a live second fighter's `{contact: None, support: <its own feet>}` was
/// picked last and silently wiped out the first fighter's real contact
/// measurement on every capture with two living fighters -- which is
/// effectively all of them. This function is the fix and its own
/// regression guard: pure data selection, no drawing, so it is tested
/// directly against a synthetic two-fighter scene without a GPU context.
fn select_diag(
    per_port: &[(usize, DiagOut)],
    query: Option<&ContactQuery>,
    support_port: Option<usize>,
) -> DiagOut {
    let mut out = DiagOut::default();
    if let Some(q) = query {
        if let Some((_, m)) = per_port.iter().find(|(p, _)| *p == q.port) {
            out.contact = m.contact.clone();
        }
    }
    if let Some(sp) = support_port {
        if let Some((_, m)) = per_port.iter().find(|(p, _)| *p == sp) {
            out.support = m.support.clone();
        }
    }
    out
}

impl Scene3D {
    /// Draw a match frame with a fixed orthographic camera into `rt`.
    pub(crate) fn draw_fixed(
        &mut self,
        gs: &GameState,
        cam: &FixedCam,
        rt: &RenderTarget,
        opts: SceneOpts,
    ) {
        let _ = self.draw_fixed_diag(gs, cam, rt, opts, None);
    }

    /// [`draw_fixed`] plus measured overlays. Returns the contact
    /// measurement taken on the drawn pose when `diag.contact` asks for one.
    pub(crate) fn draw_fixed_diag(
        &mut self,
        gs: &GameState,
        cam: &FixedCam,
        rt: &RenderTarget,
        opts: SceneOpts,
        diag: Option<&Diagnostics>,
    ) -> DiagOut {
        self.ensure_stage(gs);
        let (w, h) = (rt.texture.width(), rt.texture.height());
        self.viewport = Some((w, h));
        super::set_painter_dims(Some((w, h)));
        self.outline_width = Some(cam.outline_width(h));

        set_camera(&rt_camera_2d(rt));
        clear_background(BLACK);
        self.draw_sky(w, h, gs);

        let c3 = cam.camera(rt);
        self.view_proj = c3.matrix();
        set_camera(&c3);
        for m in &self.stage_far {
            draw_mesh(m);
        }
        for m in &self.stage_near {
            draw_mesh(m);
        }
        for f in &gs.fighters {
            self.draw_shadow(f, gs);
        }
        let query = diag.and_then(|d| d.contact.as_ref());
        let support_port = diag.and_then(|d| d.support_port);
        let mut per_port: Vec<(usize, DiagOut)> = Vec::new();
        for f in &gs.fighters {
            let m = self.draw_fighter_measured(f, gs.frame, query);
            per_port.push((f.port, m));
        }
        let out = select_diag(&per_port, query, support_port);
        let measured = out.contact.clone();
        self.draw_projectiles(gs);
        for f in &gs.fighters {
            self.draw_overlays(f, opts);
        }
        self.draw_fx(gs);
        if let Some(d) = diag {
            self.draw_diagnostics(gs, d, measured.as_ref());
        }

        // Submit the 3D pass before the 2D one, and the 2D one before the
        // caller switches passes again: draw calls never straddle a pass.
        flush_gl();
        set_camera(&rt_camera_2d(rt));
        if opts.hud {
            let mut p = MqPainter;
            self.draw_port_tags(&mut p, gs, opts);
            viz::draw_hud(&mut p, w, h, gs, opts.local_player);
        }
        flush_gl();
        set_default_camera();
        self.viewport = None;
        self.outline_width = None;
        super::set_painter_dims(None);
        out
    }

    /// Measured overlays drawn as flat strokes in the fighting plane (in
    /// front of the models, z = [`DIAG_Z`]): hurt capsules (green), the
    /// runtime hitbox (red circle + centre cross), a projectile (orange),
    /// and the contact-piece markers — cyan box = piece bounds, white ring =
    /// joint, magenta ring = tip, yellow cross = point of the piece's
    /// projected surface nearest the hitbox centre, with a line to it. Strokes are built as thin quads
    /// (not GL lines) so their width is exact in world units.
    fn draw_diagnostics(
        &self,
        gs: &GameState,
        d: &Diagnostics,
        measured: Option<&crate::model::contact::ContactMeasure>,
    ) {
        let w = DIAG_STROKE;
        if d.capsules {
            for f in &gs.fighters {
                if matches!(f.state, State::Dead) {
                    continue;
                }
                let c = crate::model::contact::capsule(f);
                let col = [110, 230, 120, 220];
                stroke_circle(c.a[0], c.a[1], c.radius, w, col);
                stroke_circle(c.b[0], c.b[1], c.radius, w, col);
                stroke_polyline(
                    &[(c.a[0] - c.radius, c.a[1]), (c.b[0] - c.radius, c.b[1])],
                    w,
                    col,
                );
                stroke_polyline(
                    &[(c.a[0] + c.radius, c.a[1]), (c.b[0] + c.radius, c.b[1])],
                    w,
                    col,
                );
            }
        }
        if let Some((x, y, r)) = d.hitbox {
            let col = [255, 70, 70, 240];
            stroke_circle(x, y, r, w, col);
            let k = (r * 0.2).max(0.6);
            stroke_polyline(&[(x - k, y), (x + k, y)], w, col);
            stroke_polyline(&[(x, y - k), (x, y + k)], w, col);
        }
        if let Some((x, y, r)) = d.projectile {
            stroke_circle(x, y, r, w, [255, 170, 60, 240]);
        }
        if let Some(m) = measured {
            let cyan = [90, 220, 255, 220];
            let (lo, hi) = (m.aabb_min, m.aabb_max);
            stroke_polyline(
                &[
                    (lo[0], lo[1]),
                    (hi[0], lo[1]),
                    (hi[0], hi[1]),
                    (lo[0], hi[1]),
                    (lo[0], lo[1]),
                ],
                w,
                cyan,
            );
            stroke_circle(m.joint[0], m.joint[1], 0.6, w, [255, 255, 255, 255]);
            stroke_circle(m.tip[0], m.tip[1], 0.6, w, [255, 90, 255, 255]);
            if let (Some(nv), Some(hb)) = (m.nearest_point, &m.hitbox) {
                let yellow = [255, 230, 80, 255];
                let k = 1.2;
                stroke_polyline(&[(nv[0] - k, nv[1] - k), (nv[0] + k, nv[1] + k)], w, yellow);
                stroke_polyline(&[(nv[0] - k, nv[1] + k), (nv[0] + k, nv[1] - k)], w, yellow);
                stroke_polyline(
                    &[(nv[0], nv[1]), (hb.center[0], hb.center[1])],
                    w * 0.7,
                    [255, 230, 80, 200],
                );
            }
        }
    }

    /// Draw one fighter model (explicit pose and root) with a fixed camera
    /// into `rt`. Lighting is the current stage's fighter light.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn draw_model_fixed(
        &mut self,
        f: &Fighter,
        frame: u64,
        pose: &crate::model::rig::Pose,
        root: &Xf,
        cam: &FixedCam,
        rt: &RenderTarget,
        look: ModelLook,
    ) {
        let (w, h) = (rt.texture.width(), rt.texture.height());
        self.viewport = Some((w, h));
        super::set_painter_dims(Some((w, h)));
        self.outline_width = Some(cam.outline_width(h));
        self.silhouette = look.silhouette;

        set_camera(&rt_camera_2d(rt));
        if look.silhouette {
            clear_background(WHITE);
        } else if look.backdrop {
            clear_background(BLACK);
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
        }

        let c3 = cam.camera(rt);
        self.view_proj = c3.matrix();
        set_camera(&c3);
        self.draw_posed(f, frame, pose, root);
        if look.hitboxes && !look.silhouette {
            self.draw_overlays(
                f,
                SceneOpts {
                    hitboxes: true,
                    ..SceneOpts::default()
                },
            );
        }
        flush_gl();
        set_default_camera();
        self.silhouette = false;
        self.viewport = None;
        self.outline_width = None;
        super::set_painter_dims(None);
    }

    /// A character's rig (for rest poses in captures).
    pub(crate) fn model_rig(&self, id: CharacterId) -> &crate::model::rig::Rig {
        &self.models[id.index()].rig
    }

    /// Make sure a stage's geometry and lights are loaded (for captures that
    /// draw only a model but want the stage's fighter light).
    pub(crate) fn prepare_stage(&mut self, gs: &GameState) {
        self.ensure_stage(gs);
    }

    /// Triangle counts: (per fighter model, stage near + far).
    pub(crate) fn triangle_budget(&self) -> (Vec<(String, usize)>, usize) {
        let models = self
            .models
            .iter()
            .enumerate()
            .map(|(i, m)| {
                (
                    crate::export::character_id(CharacterId::ALL[i]).to_string(),
                    m.rig.triangle_count(),
                )
            })
            .collect();
        let stage = self
            .stage_near
            .iter()
            .chain(self.stage_far.iter())
            .map(|m| m.indices.len() / 3)
            .sum();
        (models, stage)
    }
}

/// Submit every queued draw call now.
pub(crate) fn flush_gl() {
    // SAFETY: main thread, between draw calls — the documented way to reach
    // the miniquad context.
    unsafe {
        get_internal_gl().flush();
    }
}

/// Depth at which diagnostic strokes are drawn (in front of every model,
/// whose depth extent is within ±9 units).
const DIAG_Z: f32 = 10.0;
/// Stroke width of diagnostic overlays in world units (≈2 px at the 44-unit
/// contact camera on 512 px).
const DIAG_STROKE: f32 = 0.18;

/// A polyline in the XY fighting plane as a strip of thin quads.
fn stroke_polyline(pts: &[(f32, f32)], width: f32, rgba: [u8; 4]) {
    if pts.len() < 2 {
        return;
    }
    let mut vertices = Vec::with_capacity(pts.len() * 2);
    let mut indices: Vec<u16> = Vec::with_capacity((pts.len() - 1) * 6);
    let hw = width * 0.5;
    for i in 0..pts.len() {
        // Direction of the adjacent segment(s), averaged at interior points.
        let prev = if i > 0 { pts[i - 1] } else { pts[i] };
        let next = if i + 1 < pts.len() {
            pts[i + 1]
        } else {
            pts[i]
        };
        let (dx, dy) = (next.0 - prev.0, next.1 - prev.1);
        let len = (dx * dx + dy * dy).sqrt().max(1e-5);
        let (nx, ny) = (-dy / len * hw, dx / len * hw);
        let (x, y) = pts[i];
        for (ox, oy) in [(nx, ny), (-nx, -ny)] {
            vertices.push(Vertex {
                position: vec3(x + ox, y + oy, DIAG_Z),
                uv: vec2(0.0, 0.0),
                color: rgba,
                normal: vec4(0.0, 0.0, 1.0, 0.0),
            });
        }
        if i + 1 < pts.len() {
            let b = (i * 2) as u16;
            indices.extend_from_slice(&[b, b + 1, b + 3, b, b + 3, b + 2]);
        }
    }
    draw_mesh(&Mesh {
        vertices,
        indices,
        texture: None,
    });
}

/// A circle outline in the XY fighting plane.
fn stroke_circle(cx: f32, cy: f32, r: f32, width: f32, rgba: [u8; 4]) {
    let n = 48;
    let pts: Vec<(f32, f32)> = (0..=n)
        .map(|i| {
            let a = i as f32 / n as f32 * std::f32::consts::TAU;
            (cx + a.cos() * r, cy + a.sin() * r)
        })
        .collect();
    stroke_polyline(&pts, width, rgba);
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
        remap.fill(u32::MAX);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::contact::{ContactMeasure, FootSupport};

    fn fake_contact(action_id: &'static str) -> ContactMeasure {
        ContactMeasure {
            action_id,
            bone: "hand_r".into(),
            piece_ids: vec!["hand_r".into()],
            root: [0.0, 0.0],
            facing: 1.0,
            eased_facing: 1.0,
            joint: [0.0, 0.0, 0.0],
            centroid: [0.0, 0.0, 0.0],
            aabb_min: [0.0, 0.0, 0.0],
            aabb_max: [0.0, 0.0, 0.0],
            hitbox: None,
            mesh_distance: Some(0.0),
            nearest_point: None,
            signed_separation: Some(-1.0),
            nearest_vertex: None,
            vertex_distance: None,
            triangle_count: 0,
            tip: [0.0, 0.0, 0.0],
            tip_reach_x: 0.0,
        }
    }

    fn fake_support(piece_id: &str) -> FootSupport {
        FootSupport {
            piece_id: piece_id.into(),
            bone: "foot_r".into(),
            aabb_min: [0.0, 0.0, 0.0],
            aabb_max: [1.0, 1.0, 1.0],
            support_distance: 0.0,
        }
    }

    /// Regression test for the P1/P2 overwrite bug Codex's Windows review
    /// caught: with two live fighters, `draw_fighter_measured` now always
    /// returns non-empty `support` for both, so the old "whichever came
    /// back non-trivial last" heuristic in `draw_fixed_diag` would pick
    /// port 1's `{contact: None, support: <its own feet>}` and silently
    /// wipe out port 0's real contact measurement. `select_diag` must
    /// return port 0's contact untouched and port 0's support (the
    /// queried/support_port fighter), never falling back to port 1's data
    /// just because it was measured after port 0's.
    #[test]
    fn select_diag_does_not_let_a_second_live_fighter_overwrite_the_first() {
        let p0 = DiagOut {
            contact: Some(fake_contact("jab2")),
            support: vec![fake_support("foot_r"), fake_support("foot_l")],
        };
        let p1 = DiagOut {
            contact: None,
            support: vec![fake_support("foot_r"), fake_support("foot_l")],
        };
        let per_port = vec![(0usize, p0), (1usize, p1)];
        let query = ContactQuery {
            port: 0,
            action: crate::sim::attacks::MoveId::Jab2,
            hitbox: None,
        };
        let out = select_diag(&per_port, Some(&query), Some(0));
        assert!(
            out.contact.is_some(),
            "port 0's real contact measurement must survive a live port 1"
        );
        assert_eq!(out.contact.unwrap().action_id, "jab2");
        assert_eq!(
            out.support.len(),
            2,
            "support_port=0 must report port 0's feet"
        );
    }

    /// `support_port` and `contact`'s port are independent: a caller can ask
    /// for port 0's contact while wanting port 1's foot-support evidence (or
    /// vice-versa) -- neither selection should leak into the other.
    #[test]
    fn select_diag_picks_contact_and_support_from_different_ports_independently() {
        let p0 = DiagOut {
            contact: Some(fake_contact("dash_attack")),
            support: vec![fake_support("p0_foot")],
        };
        let p1 = DiagOut {
            contact: None,
            support: vec![fake_support("p1_foot")],
        };
        let per_port = vec![(0usize, p0), (1usize, p1)];
        let query = ContactQuery {
            port: 0,
            action: crate::sim::attacks::MoveId::DashAttack,
            hitbox: None,
        };
        let out = select_diag(&per_port, Some(&query), Some(1));
        assert_eq!(out.contact.unwrap().action_id, "dash_attack");
        assert_eq!(out.support[0].piece_id, "p1_foot");
    }

    /// No `support_port` set (e.g. the capsules.png-style call, which only
    /// wants hurt-capsule overlays): support stays empty rather than
    /// guessing a fighter to report.
    #[test]
    fn select_diag_leaves_support_empty_when_no_support_port_is_asked_for() {
        let p0 = DiagOut {
            contact: Some(fake_contact("jab")),
            support: vec![fake_support("foot_r")],
        };
        let per_port = vec![(0usize, p0)];
        let query = ContactQuery {
            port: 0,
            action: crate::sim::attacks::MoveId::Jab,
            hitbox: None,
        };
        let out = select_diag(&per_port, Some(&query), None);
        assert!(out.contact.is_some());
        assert!(out.support.is_empty());
    }

    /// No live query at all (e.g. a plain fixture draw with no diagnostics)
    /// still lets `support_port` alone pull that fighter's feet, since
    /// `support.png` needs foot evidence without faking an attack query.
    #[test]
    fn select_diag_reports_support_with_no_contact_query() {
        let p0 = DiagOut {
            contact: None,
            support: vec![fake_support("foot_r"), fake_support("foot_l")],
        };
        let per_port = vec![(0usize, p0)];
        let out = select_diag(&per_port, None, Some(0));
        assert!(out.contact.is_none());
        assert_eq!(out.support.len(), 2);
    }
}
