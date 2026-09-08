//! Headless CPU rasteriser + animated-GIF export.
//!
//! Implements [`crate::viz::Painter`] over a plain RGBA byte buffer (software
//! alpha blending), so the exact same [`crate::viz::draw_scene`] the game window
//! uses can render frames with no GPU, no display and no window — which is how
//! this runs in CI and on servers. `overframe-replay` calls [`render_demo_gif`].

use std::fs::File;
use std::path::Path;

use image::codecs::gif::GifEncoder;
use image::{Delay, Frame, RgbaImage};

use crate::viz::{self, Color, Painter, SceneOpts};

/// A software RGBA canvas.
pub struct Canvas {
    pub w: u32,
    pub h: u32,
    pub buf: Vec<u8>, // RGBA8
}

impl Canvas {
    pub fn new(w: u32, h: u32) -> Self {
        Canvas {
            w,
            h,
            buf: vec![0; (w * h * 4) as usize],
        }
    }

    #[inline]
    fn blend(&mut self, x: i32, y: i32, c: Color) {
        if x < 0 || y < 0 || x >= self.w as i32 || y >= self.h as i32 || c.a == 0 {
            return;
        }
        let idx = ((y as u32 * self.w + x as u32) * 4) as usize;
        let a = c.a as f32 / 255.0;
        let inv = 1.0 - a;
        let b = &mut self.buf[idx..idx + 4];
        b[0] = (c.r as f32 * a + b[0] as f32 * inv) as u8;
        b[1] = (c.g as f32 * a + b[1] as f32 * inv) as u8;
        b[2] = (c.b as f32 * a + b[2] as f32 * inv) as u8;
        b[3] = 255;
    }

    pub fn to_image(&self) -> RgbaImage {
        RgbaImage::from_raw(self.w, self.h, self.buf.clone()).expect("buffer size matches")
    }

    pub fn clear(&mut self, c: Color) {
        for px in self.buf.chunks_exact_mut(4) {
            px[0] = c.r;
            px[1] = c.g;
            px[2] = c.b;
            px[3] = 255;
        }
    }
}

impl Painter for Canvas {
    fn fill_rect(&mut self, x: f32, y: f32, w: f32, h: f32, c: Color) {
        let x0 = x.floor() as i32;
        let y0 = y.floor() as i32;
        let x1 = (x + w).ceil() as i32;
        let y1 = (y + h).ceil() as i32;
        for py in y0..y1 {
            for px in x0..x1 {
                self.blend(px, py, c);
            }
        }
    }

    fn fill_circle(&mut self, cx: f32, cy: f32, r: f32, c: Color) {
        if r <= 0.0 {
            return;
        }
        let r2 = r * r;
        let x0 = (cx - r).floor() as i32;
        let x1 = (cx + r).ceil() as i32;
        let y0 = (cy - r).floor() as i32;
        let y1 = (cy + r).ceil() as i32;
        for py in y0..y1 {
            for px in x0..x1 {
                let dx = px as f32 + 0.5 - cx;
                let dy = py as f32 + 0.5 - cy;
                let d2 = dx * dx + dy * dy;
                if d2 <= r2 {
                    // Light edge anti-aliasing.
                    let edge = r - d2.sqrt();
                    let aa = edge.clamp(0.0, 1.0);
                    self.blend(px, py, c.with_a((c.a as f32 * aa) as u8));
                }
            }
        }
    }

    fn line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, thick: f32, c: Color) {
        let dx = x1 - x0;
        let dy = y1 - y0;
        let len = (dx * dx + dy * dy).sqrt().max(1.0);
        let steps = len.ceil() as i32;
        let r = (thick * 0.5).max(0.5);
        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            let x = x0 + dx * t;
            let y = y0 + dy * t;
            self.fill_circle(x, y, r, c);
        }
    }

    fn dims(&self) -> (f32, f32) {
        (self.w as f32, self.h as f32)
    }
}

/// Render the scripted demo to an animated GIF at `path`.
///
/// `width`/`height` set the resolution; the demo runs at 60 Hz and is sampled
/// every other tick to a 30 fps GIF.
pub fn render_demo_gif(
    path: impl AsRef<Path>,
    width: u32,
    height: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut gs = crate::demo::match_state();
    let mut canvas = Canvas::new(width, height);

    let file = File::create(path)?;
    // speed 1..=30; higher is faster encoding, slightly lower quality.
    let mut encoder = GifEncoder::new_with_speed(file, 20);
    encoder.set_repeat(image::codecs::gif::Repeat::Infinite)?;

    let opts = SceneOpts {
        training: false,
        watermark: true,
    };

    let total = crate::demo::DEMO_LEN;
    for frame in 0..total {
        let inputs = crate::demo::inputs(&gs, frame);
        gs.step(&inputs);

        // Sample every other frame → 30 fps.
        if frame % 2 == 0 {
            canvas.clear(Color::rgb(12, 12, 20));
            viz::draw_scene(&mut canvas, &gs, opts);
            let img = canvas.to_image();
            let fr = Frame::from_parts(img, 0, 0, Delay::from_numer_denom_ms(33, 1));
            encoder.encode_frame(fr)?;
        }
    }
    Ok(())
}

/// Render a single frame of the demo (at tick `at`) to a PNG — handy for docs.
pub fn render_demo_png(
    path: impl AsRef<Path>,
    width: u32,
    height: u32,
    at: u64,
    training: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut gs = crate::demo::match_state();
    for frame in 0..at {
        let inp = crate::demo::inputs(&gs, frame);
        gs.step(&inp);
    }
    let mut canvas = Canvas::new(width, height);
    canvas.clear(Color::rgb(12, 12, 20));
    viz::draw_scene(
        &mut canvas,
        &gs,
        SceneOpts {
            training,
            watermark: true,
        },
    );
    canvas.to_image().save(path)?;
    Ok(())
}
