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
