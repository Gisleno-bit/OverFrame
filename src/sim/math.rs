//! Small, dependency-free math used throughout the simulation.
//!
//! The simulation uses `f32`. On a single build/architecture (the normal case
//! for a match between two Windows x86-64 players running the same binary) the
//! IEEE-754 operations here are bit-for-bit reproducible, which is why the GGRS
//! `SyncTest` in `tests/` passes. Cross-architecture bit-determinism is a
//! Fase-2 hardening task (documented in the README) — the whole point of keeping
//! this module tiny and the simulation pure is that swapping in a fixed-point
//! number type later touches only this file and the constants.

/// 2D vector with the handful of operations the simulation needs.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub const ZERO: Vec2 = Vec2 { x: 0.0, y: 0.0 };

    #[inline]
    pub const fn new(x: f32, y: f32) -> Self {
        Vec2 { x, y }
    }

    #[inline]
    pub fn length(self) -> f32 {
        (self.x * self.x + self.y * self.y).sqrt()
    }

    #[inline]
    pub fn length_sq(self) -> f32 {
        self.x * self.x + self.y * self.y
    }

    #[inline]
    pub fn normalized_or_zero(self) -> Vec2 {
        let len = self.length();
        if len > 1e-6 {
            Vec2::new(self.x / len, self.y / len)
        } else {
            Vec2::ZERO
        }
    }
}

impl std::ops::Add for Vec2 {
    type Output = Vec2;
    #[inline]
    fn add(self, o: Vec2) -> Vec2 {
        Vec2::new(self.x + o.x, self.y + o.y)
    }
}
impl std::ops::Sub for Vec2 {
    type Output = Vec2;
    #[inline]
    fn sub(self, o: Vec2) -> Vec2 {
        Vec2::new(self.x - o.x, self.y - o.y)
    }
}
impl std::ops::Mul<f32> for Vec2 {
    type Output = Vec2;
    #[inline]
    fn mul(self, s: f32) -> Vec2 {
        Vec2::new(self.x * s, self.y * s)
    }
}
impl std::ops::AddAssign for Vec2 {
    #[inline]
    fn add_assign(&mut self, o: Vec2) {
        self.x += o.x;
        self.y += o.y;
    }
}

/// Clamp helper (stable across older Rust than `f32::clamp` guarantees around edge cases).
#[inline]
pub fn clampf(v: f32, lo: f32, hi: f32) -> f32 {
    if v < lo {
        lo
    } else if v > hi {
        hi
    } else {
        v
    }
}

/// Move `v` toward `target` by at most `max_delta` (used for friction / accel).
#[inline]
pub fn approach(v: f32, target: f32, max_delta: f32) -> f32 {
    if v < target {
        (v + max_delta).min(target)
    } else {
        (v - max_delta).max(target)
    }
}

/// Deterministic little RNG (xorshift32). Seeded per match; part of the saved
/// game state so rollbacks reproduce every "random" outcome exactly.
#[derive(Clone, Copy, Debug)]
pub struct Rng {
    state: u32,
}

impl Rng {
    pub fn new(seed: u32) -> Self {
        // Avoid a zero state, which xorshift cannot escape.
        Rng {
            state: seed | 0x9E37_79B9,
        }
    }

    #[inline]
    pub fn next_u32(&mut self) -> u32 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.state = x;
        x
    }

    /// Uniform-ish float in `[-1.0, 1.0]`.
    #[inline]
    pub fn next_signed(&mut self) -> f32 {
        (self.next_u32() as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

/// Shortest distance between two line segments `a0→a1` and `b0→b1` (2D).
pub fn segment_distance(a0: Vec2, a1: Vec2, b0: Vec2, b1: Vec2) -> f32 {
    // If the segments intersect the distance is zero; otherwise it is the
    // minimum of the four endpoint-to-segment distances.
    if segments_intersect(a0, a1, b0, b1) {
        return 0.0;
    }
    point_segment_distance(a0, b0, b1)
        .min(point_segment_distance(a1, b0, b1))
        .min(point_segment_distance(b0, a0, a1))
        .min(point_segment_distance(b1, a0, a1))
}

/// Distance from point `p` to segment `a→b`.
pub fn point_segment_distance(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let len2 = ab.x * ab.x + ab.y * ab.y;
    if len2 <= 1e-8 {
        return (p - a).length();
    }
    let t = clampf(((p.x - a.x) * ab.x + (p.y - a.y) * ab.y) / len2, 0.0, 1.0);
    let q = Vec2::new(a.x + ab.x * t, a.y + ab.y * t);
    (p - q).length()
}

fn orient(a: Vec2, b: Vec2, c: Vec2) -> f32 {
    (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
}

fn segments_intersect(a0: Vec2, a1: Vec2, b0: Vec2, b1: Vec2) -> bool {
    let d1 = orient(b0, b1, a0);
    let d2 = orient(b0, b1, a1);
    let d3 = orient(a0, a1, b0);
    let d4 = orient(a0, a1, b1);
    ((d1 > 0.0 && d2 < 0.0) || (d1 < 0.0 && d2 > 0.0))
        && ((d3 > 0.0 && d4 < 0.0) || (d3 < 0.0 && d4 > 0.0))
}
