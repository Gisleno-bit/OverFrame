//! CPU lighting and palettes.
//!
//! Vertex colours are computed on the CPU each frame (a few thousand vertices
//! per fighter — trivial) so the GPU side needs no custom shader: macroquad's
//! default material multiplies vertex colour by a white texture. Model:
//! hemisphere ambient (sky/ground) + one directional key light + a soft rim,
//! quantised into three bands for a clean, readable toon look.

use super::math3::{v3, V3};

/// Material slots a mesh vertex can reference. A [`Palette`] maps them to
/// colours.
pub mod slot {
    pub const PRIMARY: u8 = 0;
    pub const SECONDARY: u8 = 1;
    pub const ACCENT: u8 = 2;
    pub const SKIN: u8 = 3;
    pub const DARK: u8 = 4;
    /// Unlit / emissive (eyes, cores, glow strips).
    pub const GLOW: u8 = 5;
    pub const LIGHT: u8 = 6;
    pub const EXTRA: u8 = 7;
    pub const COUNT: usize = 8;
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    pub colors: [[u8; 3]; slot::COUNT],
    pub name: &'static str,
}

impl Palette {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        name: &'static str,
        primary: [u8; 3],
        secondary: [u8; 3],
        accent: [u8; 3],
        skin: [u8; 3],
        dark: [u8; 3],
        glow: [u8; 3],
        light: [u8; 3],
        extra: [u8; 3],
    ) -> Palette {
        Palette {
            colors: [primary, secondary, accent, skin, dark, glow, light, extra],
            name,
        }
    }

    #[inline]
    pub fn color(&self, slot: u8) -> [u8; 3] {
        self.colors[(slot as usize) % slot::COUNT]
    }

    /// The colour used for HUD accents / menus (the primary).
    pub fn primary(&self) -> [u8; 3] {
        self.colors[0]
    }
}

/// Scene lighting.
#[derive(Clone, Copy, Debug)]
pub struct Light {
    /// Unit vector *toward* the key light.
    pub dir: V3,
    pub key: [f32; 3],
    pub sky: [f32; 3],
    pub ground: [f32; 3],
    /// Rim light strength (0..1) — a pale edge on surfaces facing away from
    /// the camera's view axis, separating fighters from the background.
    pub rim: f32,
    /// Fill light from the camera (+Z), unbanded, so front faces never go
    /// muddy whatever the key direction.
    pub fill: f32,
    /// Toon banding: number of shade bands on the key light (0 = smooth).
    pub bands: u32,
}

impl Light {
    pub fn default_for(sky: (u8, u8, u8), ground: (u8, u8, u8)) -> Light {
        let f = |c: (u8, u8, u8), k: f32| {
            [
                c.0 as f32 / 255.0 * k,
                c.1 as f32 / 255.0 * k,
                c.2 as f32 / 255.0 * k,
            ]
        };
        Light {
            dir: v3(-0.35, 0.8, 0.6).norm(),
            key: [1.1, 1.05, 0.98],
            sky: f(sky, 0.35).map(|x| x + 0.42),
            ground: f(ground, 0.3).map(|x| x + 0.2),
            rim: 0.3,
            fill: 0.3,
            bands: 4,
        }
    }
}

/// Whole-model colour modulation from the fighter's state.
#[derive(Clone, Copy, Debug)]
pub struct Tint {
    /// Blend toward white (hit flash), 0..1.
    pub flash: f32,
    /// Blend toward grey (shield stun / knockdown), 0..1.
    pub grey: f32,
    pub alpha: u8,
}

impl Default for Tint {
    fn default() -> Self {
        Tint {
            flash: 0.0,
            grey: 0.0,
            alpha: 255,
        }
    }
}

/// Shade one vertex. `pos` is used for the rim term (view assumed along +Z).
#[inline]
pub fn shade(albedo: [u8; 3], n: V3, _pos: V3, light: &Light, tint: &Tint, slot: u8) -> [u8; 4] {
    let a = [
        albedo[0] as f32 / 255.0,
        albedo[1] as f32 / 255.0,
        albedo[2] as f32 / 255.0,
    ];
    let out = if slot == slot::GLOW {
        // Emissive: full albedo, slightly lifted.
        [
            (a[0] * 1.15).min(1.0),
            (a[1] * 1.15).min(1.0),
            (a[2] * 1.15).min(1.0),
        ]
    } else {
        // Hemisphere ambient.
        let up = (n.y * 0.5 + 0.5).clamp(0.0, 1.0);
        let mut diff = n.dot(light.dir).max(0.0);
        if light.bands > 0 {
            // Quantise, but keep a soft transition so facets don't flicker.
            let b = light.bands as f32;
            diff = ((diff * b).floor() + smooth_frac(diff * b)) / b;
        }
        // Rim: surfaces whose normal points away from +Z get a pale edge.
        let rim = light.rim * (1.0 - n.z.max(0.0)).powi(3) * (0.4 + 0.6 * up);
        let fill = light.fill * n.z.max(0.0);
        let mut c = [0.0f32; 3];
        for i in 0..3 {
            let amb = light.sky[i] * up + light.ground[i] * (1.0 - up);
            c[i] = a[i] * (amb + light.key[i] * diff * 0.8 + fill) + rim * 0.9;
        }
        c
    };
    let mut c = out;
    for x in &mut c {
        *x = *x * (1.0 - tint.flash) + tint.flash;
        let g = 0.299 * out[0] + 0.587 * out[1] + 0.114 * out[2];
        *x = *x * (1.0 - tint.grey) + g * tint.grey;
    }
    [
        (c[0].clamp(0.0, 1.0) * 255.0) as u8,
        (c[1].clamp(0.0, 1.0) * 255.0) as u8,
        (c[2].clamp(0.0, 1.0) * 255.0) as u8,
        tint.alpha,
    ]
}

#[inline]
fn smooth_frac(x: f32) -> f32 {
    // Fractional part pushed toward 0/1 (a 25 % wide soft step per band).
    let f = x.fract();
    ((f - 0.75) / 0.25).clamp(0.0, 1.0)
}

/// Colour of a static (unanimated) vertex, e.g. stage geometry.
#[inline]
pub fn shade_static(albedo: [u8; 3], n: V3, pos: V3, light: &Light, slot: u8) -> [u8; 4] {
    shade(albedo, n, pos, light, &Tint::default(), slot)
}
