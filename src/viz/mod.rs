//! Backend-agnostic rendering.
//!
//! [`draw_scene`] turns a [`crate::sim::GameState`] into calls on a [`Painter`]
//! — three primitives (rect, circle, line) plus a bitmap font built on top of
//! them. The macroquad backend (`render`) and the headless CPU rasteriser
//! (`headless`) each implement [`Painter`], so the game window and the GIF tool
//! draw pixel-identical scenes from one piece of code.

pub mod font;

use crate::sim::attacks;
use crate::sim::fighter::{Fighter, State};
use crate::sim::math::Vec2;
use crate::sim::{FxKind, GameState};

/// Straight 8-bit RGBA colour.
#[derive(Clone, Copy, Debug)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Color { r, g, b, a: 255 }
    }
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Color { r, g, b, a }
    }
    pub fn with_a(self, a: u8) -> Self {
        Color { a, ..self }
    }
    pub fn lerp(self, o: Color, t: f32) -> Color {
        let t = t.clamp(0.0, 1.0);
        let m = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t) as u8;
        Color {
            r: m(self.r, o.r),
            g: m(self.g, o.g),
            b: m(self.b, o.b),
            a: m(self.a, o.a),
        }
    }
}

/// The three drawing primitives every backend must provide. Coordinates are in
/// screen pixels, origin top-left, y-down.
pub trait Painter {
    fn fill_rect(&mut self, x: f32, y: f32, w: f32, h: f32, c: Color);
    fn fill_circle(&mut self, cx: f32, cy: f32, r: f32, c: Color);
    fn line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, thick: f32, c: Color);
    fn dims(&self) -> (f32, f32);
}

/// World→screen camera. Fixed framing (stable for a small stage) plus shake.
#[derive(Clone, Copy, Debug)]
pub struct View {
    pub w: f32,
    pub h: f32,
    pub scale: f32,
    pub cx: f32,
    pub cy: f32,
    pub shake_x: f32,
    pub shake_y: f32,
}

impl View {
    pub fn new(w: f32, h: f32, shake: f32, frame: u64) -> Self {
        // Frame the play area (a bit wider than the platforms) with margin.
        let half_x = 235.0;
        let half_y = 165.0;
        let scale = (w / (2.0 * half_x)).min(h / (2.0 * half_y));
        // Deterministic shake from the frame counter.
        let s = shake.min(14.0);
        let a = frame as f32;
        View {
            w,
            h,
            scale,
            cx: 0.0,
            cy: 35.0,
            shake_x: (a * 1.7).sin() * s,
            shake_y: (a * 2.3).cos() * s,
        }
    }

    #[inline]
    pub fn sx(&self, world_x: f32) -> f32 {
        self.w * 0.5 + (world_x - self.cx) * self.scale + self.shake_x
    }
    #[inline]
    pub fn sy(&self, world_y: f32) -> f32 {
        self.h * 0.5 - (world_y - self.cy) * self.scale + self.shake_y
    }
    #[inline]
    pub fn s(&self, len: f32) -> f32 {
        len * self.scale
    }
    #[inline]
    pub fn p(&self, w: Vec2) -> (f32, f32) {
        (self.sx(w.x), self.sy(w.y))
    }
}

/// Options controlling the extra information drawn.
#[derive(Clone, Copy, Debug, Default)]
pub struct SceneOpts {
    /// Draw hurtboxes, hitboxes and per-fighter state text (training mode).
    pub training: bool,
    /// Draw a subtle "OVERFRAME" watermark.
    pub watermark: bool,
    /// Online: which fighter is controlled locally (its HUD panel says "YOU").
    pub local_player: Option<usize>,
}

/// Player accent colours (P1..P4).
pub const PORT_COLORS: [Color; 4] = [
    Color::rgb(255, 92, 74),
    Color::rgb(70, 150, 255),
    Color::rgb(90, 210, 120),
    Color::rgb(240, 205, 80),
];

const BG_TOP: Color = Color::rgb(18, 20, 34);
const BG_BOT: Color = Color::rgb(30, 26, 52);

/// Draw a whole frame.
pub fn draw_scene<P: Painter>(p: &mut P, gs: &GameState, opts: SceneOpts) {
    let (w, h) = p.dims();
    let view = View::new(w, h, gs.camera_shake, gs.frame);

    draw_background(p, w, h, gs);
    draw_stage(p, &view, gs);

    // Fighters (dead ones skipped inside).
    for f in &gs.fighters {
        draw_fighter(p, &view, f, opts);
    }

    draw_projectiles(p, &view, gs);
    draw_fx(p, &view, gs);
    draw_hud(p, w, h, gs, opts.local_player);

    if opts.training {
        font::draw_text(
            p,
            "TRAINING",
            12.0,
            12.0,
            2.0,
            Color::rgba(200, 220, 255, 180),
        );
    }
    if opts.watermark {
        let t = "OVERFRAME";
        let sc = 2.0;
        font::draw_text(
            p,
            t,
            w - font::text_width(t, sc) - 12.0,
            12.0,
            sc,
            Color::rgba(255, 255, 255, 70),
        );
    }

    if let Some(winner) = gs.match_over {
        draw_match_over(p, w, h, winner);
    }
}

fn draw_background<P: Painter>(p: &mut P, w: f32, h: f32, gs: &GameState) {
    // Vertical gradient in bands.
    let bands = 48;
    for i in 0..bands {
        let t = i as f32 / bands as f32;
        let c = BG_TOP.lerp(BG_BOT, t);
        p.fill_rect(0.0, h * t, w, h / bands as f32 + 1.0, c);
    }
    // Hit-flash brighten.
    if gs.hitstop_flash > 0.02 {
        let a = (gs.hitstop_flash * 40.0) as u8;
        p.fill_rect(0.0, 0.0, w, h, Color::rgba(255, 255, 255, a.min(60)));
    }
}

fn draw_stage<P: Painter>(p: &mut P, v: &View, gs: &GameState) {
    let stage = &gs.stage;

    // Blast-zone frame (faint).
    let bl = v.sx(stage.blast_left);
    let br = v.sx(stage.blast_right);
    let bt = v.sy(stage.blast_top);
    let bb = v.sy(stage.blast_bottom);
    let edge = Color::rgba(120, 60, 90, 60);
    p.line(bl, bt, br, bt, 2.0, edge);
    p.line(bl, bb, br, bb, 2.0, edge);
    p.line(bl, bt, bl, bb, 2.0, edge);
    p.line(br, bt, br, bb, 2.0, edge);

    for (i, plat) in stage.platforms.iter().enumerate() {
        let x0 = v.sx(plat.left);
        let x1 = v.sx(plat.right);
        let y = v.sy(plat.y);
        let width = x1 - x0;
        if plat.solid {
            // Main stage: a solid slab with a bright lip.
            let depth = v.s(60.0);
            p.fill_rect(x0, y, width, depth, Color::rgb(44, 48, 74));
            p.fill_rect(x0, y, width, v.s(4.0).max(3.0), Color::rgb(120, 200, 230));
            // side bevels
            p.fill_rect(x0, y, v.s(3.0).max(2.0), depth, Color::rgba(0, 0, 0, 60));
            p.fill_rect(
                x1 - v.s(3.0).max(2.0),
                y,
                v.s(3.0).max(2.0),
                depth,
                Color::rgba(0, 0, 0, 60),
            );
        } else {
            // Soft platform: a thin translucent bar with a glowing top.
            let thick = v.s(6.0).max(4.0);
            p.fill_rect(x0, y, width, thick, Color::rgba(90, 110, 170, 180));
            p.fill_rect(x0, y, width, 2.0, Color::rgb(150, 210, 255));
        }
        let _ = i;
    }

    // Ledge markers.
    for l in &stage.ledges {
        let (lx, ly) = v.p(l.pos);
        p.fill_circle(lx, ly, v.s(3.0).max(2.0), Color::rgba(150, 230, 255, 160));
    }
}

fn draw_fighter<P: Painter>(p: &mut P, v: &View, f: &Fighter, opts: SceneOpts) {
    if matches!(f.state, State::Dead) {
        return;
    }
    // Intangible blink.
    if f.is_intangible() && (f.state_frame / 3) % 2 == 0 && !matches!(f.state, State::LedgeGrab) {
        // still draw, but faded
    }

    let base = PORT_COLORS[f.port % 4];
    // State-driven tint.
    let mut body = base;
    if f.anim_flash > 0 || matches!(f.state, State::Hitstun { .. }) {
        body = body.lerp(Color::rgb(255, 255, 255), 0.6);
    }
    if matches!(f.state, State::ShieldStun { .. } | State::Knockdown) {
        body = body.lerp(Color::rgb(120, 120, 140), 0.4);
    }
    let mut alpha = 255u8;
    if f.is_intangible() && (f.state_frame / 3) % 2 == 0 {
        alpha = 140;
    }

    // Body geometry (a capsule) — crouch squashes it.
    let ch = f.character;
    let squash = if matches!(f.state, State::Crouch) {
        0.6
    } else {
        1.0
    };
    let hw = v.s(ch.half_width);
    let bh = v.s(ch.height * squash);
    let feet_x = v.sx(f.pos.x);
    let feet_y = v.sy(f.pos.y);
    let top_y = feet_y - bh;
    let cx = feet_x;

    // capsule: rect + rounded ends
    p.fill_rect(
        cx - hw,
        top_y + hw,
        hw * 2.0,
        bh - hw * 2.0,
        body.with_a(alpha),
    );
    p.fill_circle(cx, top_y + hw, hw, body.with_a(alpha));
    p.fill_circle(cx, feet_y - hw, hw, body.with_a(alpha));
    // head
    let head_r = hw * 0.9;
    let head_y = top_y - head_r * 0.2;
    p.fill_circle(cx, head_y, head_r, body.with_a(alpha));
    // facing pointer (a beak)
    let fx = cx + f.facing * head_r;
    p.fill_circle(
        fx,
        head_y,
        head_r * 0.45,
        base.lerp(Color::rgb(255, 255, 255), 0.5).with_a(alpha),
    );

    // Shield bubble.
    if matches!(f.state, State::Shield) {
        let frac = (f.shield_health / crate::sim::constants::SHIELD_MAX).clamp(0.1, 1.0);
        let r = hw + bh * 0.5 * frac;
        let sc = Color::rgba(120, 200, 255, 90);
        p.fill_circle(cx, feet_y - bh * 0.5, r, sc);
    }

    // Ledge-hang indicator.
    if matches!(f.state, State::LedgeGrab) {
        p.fill_circle(
            cx - f.facing * hw,
            top_y,
            hw * 0.5,
            Color::rgba(255, 255, 255, 160),
        );
    }

    // Active attack: draw a strike limb + glow, and (training) the hitbox.
    if let Some((hb, world)) = f.active_hitbox() {
        let bc = f.body_center();
        let (bx, by) = v.p(bc);
        let (hx, hy) = v.p(world);
        let strike = Color::rgb(255, 240, 180);
        p.line(bx, by, hx, hy, v.s(4.0).max(2.0), strike);
        p.fill_circle(
            hx,
            hy,
            v.s(hb.radius * 0.6).max(3.0),
            Color::rgba(255, 230, 150, 200),
        );
        if opts.training {
            p.fill_circle(hx, hy, v.s(hb.radius), Color::rgba(255, 70, 70, 90));
        }
    }

    // Training: hurtbox outline + state label.
    if opts.training {
        let bc = f.body_center();
        let (bx, by) = v.p(bc);
        p.fill_circle(bx, by, v.s(f.hurt_radius()), Color::rgba(120, 220, 120, 45));
        let label = state_name(&f.state);
        font::draw_text(
            p,
            label,
            cx - font::text_width(label, 1.0) * 0.5,
            head_y - head_r - 12.0,
            1.0,
            Color::rgba(220, 240, 220, 200),
        );
    }
}

fn draw_projectiles<P: Painter>(p: &mut P, v: &View, gs: &GameState) {
    for pr in &gs.projectiles {
        if !pr.active {
            continue;
        }
        let (x, y) = v.p(pr.pos);
        p.fill_circle(
            x,
            y,
            v.s(attacks::PROJECTILE_RADIUS + 1.0).max(3.0),
            Color::rgba(255, 220, 120, 120),
        );
        p.fill_circle(
            x,
            y,
            v.s(attacks::PROJECTILE_RADIUS).max(2.0),
            Color::rgb(255, 245, 200),
        );
    }
}

fn draw_fx<P: Painter>(p: &mut P, v: &View, gs: &GameState) {
    for fx in &gs.fx {
        let t = 1.0 - fx.life as f32 / fx.max_life as f32;
        let (x, y) = v.p(fx.pos);
        match fx.kind {
            FxKind::Hit => {
                let r = v.s(6.0 + fx.magnitude * 0.6) * (0.4 + t);
                let a = (200.0 * (1.0 - t)) as u8;
                p.fill_circle(x, y, r, Color::rgba(255, 240, 190, a));
                p.fill_circle(x, y, r * 0.55, Color::rgba(255, 255, 255, a));
                // star spikes
                let sp = r * 1.7;
                let col = Color::rgba(255, 220, 140, a);
                p.line(x - sp, y, x + sp, y, 2.0, col);
                p.line(x, y - sp, x, y + sp, 2.0, col);
            }
            FxKind::Shield => {
                let r = v.s(10.0) * (0.5 + t);
                p.fill_circle(
                    x,
                    y,
                    r,
                    Color::rgba(120, 200, 255, (150.0 * (1.0 - t)) as u8),
                );
            }
            FxKind::Blast => {
                let r = v.s(14.0 + fx.magnitude) * (0.3 + t * 1.5);
                let a = (220.0 * (1.0 - t)) as u8;
                p.fill_circle(x, y, r, Color::rgba(255, 200, 120, a));
                p.fill_circle(x, y, r * 0.5, Color::rgba(255, 255, 255, a));
            }
            FxKind::Dust => {
                let r = v.s(4.0) * (0.5 + t);
                p.fill_circle(
                    x,
                    y,
                    r,
                    Color::rgba(200, 200, 210, (120.0 * (1.0 - t)) as u8),
                );
            }
        }
    }
}

fn draw_hud<P: Painter>(p: &mut P, w: f32, h: f32, gs: &GameState, local: Option<usize>) {
    let n = gs.fighters.len().max(1);
    let panel_w = 150.0f32.min(w / n as f32 - 20.0);
    let gap = (w - panel_w * n as f32) / (n as f32 + 1.0);
    let base_y = h - 66.0;

    for (i, f) in gs.fighters.iter().enumerate() {
        let x = gap + (panel_w + gap) * i as f32;
        let accent = PORT_COLORS[f.port % 4];

        // Panel background.
        p.fill_rect(x, base_y, panel_w, 56.0, Color::rgba(10, 12, 22, 170));
        p.fill_rect(x, base_y, panel_w, 3.0, accent);

        // Label + stocks.
        font::draw_text(
            p,
            &if local == Some(i) {
                format!("P{} YOU", i + 1)
            } else {
                format!("P{}", i + 1)
            },
            x + 8.0,
            base_y + 8.0,
            2.0,
            accent,
        );
        // Stock icons, capped so large training-mode stock counts stay tidy.
        let shown = f.stocks.clamp(0, 5);
        for s in 0..shown {
            let sxp = x + panel_w - 14.0 - s as f32 * 12.0;
            p.fill_circle(sxp, base_y + 15.0, 4.0, accent);
        }
        if f.stocks > 5 {
            font::draw_text(
                p,
                &format!("x{}", f.stocks),
                x + panel_w - 84.0,
                base_y + 10.0,
                1.5,
                accent,
            );
        }

        // Percent, coloured by damage.
        let pc = percent_color(f.percent);
        let txt = format!("{}%", f.percent as i32);
        let sc = 3.0;
        let tw = font::text_width(&txt, sc);
        font::draw_text(p, &txt, x + panel_w * 0.5 - tw * 0.5, base_y + 26.0, sc, pc);
    }
}

fn percent_color(pct: f32) -> Color {
    let white = Color::rgb(240, 240, 240);
    let yellow = Color::rgb(255, 225, 110);
    let orange = Color::rgb(255, 140, 60);
    let red = Color::rgb(255, 60, 60);
    if pct < 50.0 {
        white.lerp(yellow, pct / 50.0)
    } else if pct < 100.0 {
        yellow.lerp(orange, (pct - 50.0) / 50.0)
    } else {
        orange.lerp(red, ((pct - 100.0) / 80.0).min(1.0))
    }
}

fn draw_match_over<P: Painter>(p: &mut P, w: f32, h: f32, winner: usize) {
    p.fill_rect(0.0, 0.0, w, h, Color::rgba(0, 0, 0, 120));
    let (title, col) = if winner == usize::MAX {
        ("DRAW".to_string(), Color::rgb(230, 230, 230))
    } else {
        (format!("P{} WINS", winner + 1), PORT_COLORS[winner % 4])
    };
    let sc = 6.0;
    let tw = font::text_width(&title, sc);
    font::draw_text(p, &title, w * 0.5 - tw * 0.5, h * 0.4, sc, col);
}

fn state_name(s: &State) -> &'static str {
    match s {
        State::Stand => "STAND",
        State::Walk => "WALK",
        State::Crouch => "CROUCH",
        State::Dash => "DASH",
        State::Run => "RUN",
        State::JumpSquat => "SQUAT",
        State::Air => "AIR",
        State::LandLag { .. } => "LAND",
        State::Waveland => "WAVELAND",
        State::Attack { .. } => "ATTACK",
        State::Shield => "SHIELD",
        State::ShieldStun { .. } => "SHIELDSTUN",
        State::Roll { .. } => "ROLL",
        State::Spotdodge => "SPOTDODGE",
        State::Airdodge => "AIRDODGE",
        State::Grab => "GRAB",
        State::Hold => "HOLD",
        State::Grabbed => "GRABBED",
        State::Throw { .. } => "THROW",
        State::LedgeGrab => "LEDGE",
        State::LedgeAction { .. } => "LEDGEACT",
        State::Hitstun { .. } => "HITSTUN",
        State::Knockdown => "DOWN",
        State::Dead => "DEAD",
    }
}
