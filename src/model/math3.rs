//! Tiny 3D maths for the model layer: a vector, a 3×3 matrix and an affine
//! transform. Kept dependency-free so the rig/animation code compiles (and is
//! tested) without a GPU crate.

use std::ops::{Add, Mul, Neg, Sub};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct V3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[inline]
pub const fn v3(x: f32, y: f32, z: f32) -> V3 {
    V3 { x, y, z }
}

impl V3 {
    pub const ZERO: V3 = v3(0.0, 0.0, 0.0);
    pub const ONE: V3 = v3(1.0, 1.0, 1.0);
    pub const X: V3 = v3(1.0, 0.0, 0.0);
    pub const Y: V3 = v3(0.0, 1.0, 0.0);
    pub const Z: V3 = v3(0.0, 0.0, 1.0);

    #[inline]
    pub fn dot(self, o: V3) -> f32 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }
    #[inline]
    pub fn cross(self, o: V3) -> V3 {
        v3(
            self.y * o.z - self.z * o.y,
            self.z * o.x - self.x * o.z,
            self.x * o.y - self.y * o.x,
        )
    }
    #[inline]
    pub fn len(self) -> f32 {
        self.dot(self).sqrt()
    }
    #[inline]
    pub fn norm(self) -> V3 {
        let l = self.len();
        if l > 1e-6 {
            self * (1.0 / l)
        } else {
            V3::Y
        }
    }
    #[inline]
    pub fn scale(self, s: f32) -> V3 {
        self * s
    }
    #[inline]
    pub fn mul_elem(self, o: V3) -> V3 {
        v3(self.x * o.x, self.y * o.y, self.z * o.z)
    }
    #[inline]
    pub fn lerp(self, o: V3, t: f32) -> V3 {
        self + (o - self) * t
    }
}

impl Add for V3 {
    type Output = V3;
    #[inline]
    fn add(self, o: V3) -> V3 {
        v3(self.x + o.x, self.y + o.y, self.z + o.z)
    }
}
impl Sub for V3 {
    type Output = V3;
    #[inline]
    fn sub(self, o: V3) -> V3 {
        v3(self.x - o.x, self.y - o.y, self.z - o.z)
    }
}
impl Mul<f32> for V3 {
    type Output = V3;
    #[inline]
    fn mul(self, s: f32) -> V3 {
        v3(self.x * s, self.y * s, self.z * s)
    }
}
impl Neg for V3 {
    type Output = V3;
    #[inline]
    fn neg(self) -> V3 {
        v3(-self.x, -self.y, -self.z)
    }
}

/// Row-major 3×3 matrix.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct M3 {
    pub m: [[f32; 3]; 3],
}

impl M3 {
    pub const IDENTITY: M3 = M3 {
        m: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
    };

    pub fn scale(s: V3) -> M3 {
        M3 {
            m: [[s.x, 0.0, 0.0], [0.0, s.y, 0.0], [0.0, 0.0, s.z]],
        }
    }

    /// Rotation about X (degrees).
    pub fn rot_x(deg: f32) -> M3 {
        let (s, c) = deg.to_radians().sin_cos();
        M3 {
            m: [[1.0, 0.0, 0.0], [0.0, c, -s], [0.0, s, c]],
        }
    }
    /// Rotation about Y (degrees).
    pub fn rot_y(deg: f32) -> M3 {
        let (s, c) = deg.to_radians().sin_cos();
        M3 {
            m: [[c, 0.0, s], [0.0, 1.0, 0.0], [-s, 0.0, c]],
        }
    }
    /// Rotation about Z (degrees).
    pub fn rot_z(deg: f32) -> M3 {
        let (s, c) = deg.to_radians().sin_cos();
        M3 {
            m: [[c, -s, 0.0], [s, c, 0.0], [0.0, 0.0, 1.0]],
        }
    }
    /// Euler rotation applied as Z, then Y, then X (in the bone's local frame).
    /// Degrees. This is the order animation keys are written in.
    pub fn euler(deg: V3) -> M3 {
        M3::rot_x(deg.x) * M3::rot_y(deg.y) * M3::rot_z(deg.z)
    }

    #[inline]
    pub fn apply(&self, v: V3) -> V3 {
        let m = &self.m;
        v3(
            m[0][0] * v.x + m[0][1] * v.y + m[0][2] * v.z,
            m[1][0] * v.x + m[1][1] * v.y + m[1][2] * v.z,
            m[2][0] * v.x + m[2][1] * v.y + m[2][2] * v.z,
        )
    }

    pub fn transpose(&self) -> M3 {
        let m = &self.m;
        M3 {
            m: [
                [m[0][0], m[1][0], m[2][0]],
                [m[0][1], m[1][1], m[2][1]],
                [m[0][2], m[1][2], m[2][2]],
            ],
        }
    }

    pub fn determinant(&self) -> f32 {
        let m = &self.m;
        m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
            - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
            + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
    }

    /// Inverse (None if singular).
    pub fn inverse(&self) -> Option<M3> {
        let d = self.determinant();
        if d.abs() < 1e-9 {
            return None;
        }
        let m = &self.m;
        let inv = 1.0 / d;
        Some(M3 {
            m: [
                [
                    (m[1][1] * m[2][2] - m[1][2] * m[2][1]) * inv,
                    (m[0][2] * m[2][1] - m[0][1] * m[2][2]) * inv,
                    (m[0][1] * m[1][2] - m[0][2] * m[1][1]) * inv,
                ],
                [
                    (m[1][2] * m[2][0] - m[1][0] * m[2][2]) * inv,
                    (m[0][0] * m[2][2] - m[0][2] * m[2][0]) * inv,
                    (m[0][2] * m[1][0] - m[0][0] * m[1][2]) * inv,
                ],
                [
                    (m[1][0] * m[2][1] - m[1][1] * m[2][0]) * inv,
                    (m[0][1] * m[2][0] - m[0][0] * m[2][1]) * inv,
                    (m[0][0] * m[1][1] - m[0][1] * m[1][0]) * inv,
                ],
            ],
        })
    }
}

impl Mul for M3 {
    type Output = M3;
    fn mul(self, o: M3) -> M3 {
        let mut r = [[0.0f32; 3]; 3];
        for (i, row) in r.iter_mut().enumerate() {
            for (j, cell) in row.iter_mut().enumerate() {
                *cell =
                    self.m[i][0] * o.m[0][j] + self.m[i][1] * o.m[1][j] + self.m[i][2] * o.m[2][j];
            }
        }
        M3 { m: r }
    }
}

/// Affine transform: `p' = m·p + t`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Xf {
    pub m: M3,
    pub t: V3,
}

impl Xf {
    pub const IDENTITY: Xf = Xf {
        m: M3::IDENTITY,
        t: V3::ZERO,
    };

    pub fn translation(t: V3) -> Xf {
        Xf { m: M3::IDENTITY, t }
    }
    pub fn new(m: M3, t: V3) -> Xf {
        Xf { m, t }
    }
    /// Translate, then rotate (Euler degrees), then scale — the usual bone
    /// composition: `parent * T(offset) * R(rot) * S(scale)`.
    pub fn trs(t: V3, rot_deg: V3, scale: V3) -> Xf {
        Xf {
            m: M3::euler(rot_deg) * M3::scale(scale),
            t,
        }
    }
    #[inline]
    pub fn point(&self, p: V3) -> V3 {
        self.m.apply(p) + self.t
    }
    #[inline]
    pub fn dir(&self, d: V3) -> V3 {
        self.m.apply(d)
    }
    /// Transform a normal (inverse-transpose), renormalised.
    pub fn normal(&self, n: V3) -> V3 {
        match self.m.inverse() {
            Some(inv) => inv.transpose().apply(n).norm(),
            None => n,
        }
    }
    /// Is this transform mirrored (negative determinant)? Mirrored meshes
    /// have flipped winding.
    pub fn mirrored(&self) -> bool {
        self.m.determinant() < 0.0
    }
}

impl Mul for Xf {
    type Output = Xf;
    /// `self * o`: apply `o` first, then `self`.
    fn mul(self, o: Xf) -> Xf {
        Xf {
            m: self.m * o.m,
            t: self.m.apply(o.t) + self.t,
        }
    }
}

/// Smoothstep-style easing for keyframe blends.
#[inline]
pub fn ease(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Ease-out (fast start, gentle end) — good for strikes snapping into place.
#[inline]
pub fn ease_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t) * (1.0 - t)
}

/// Ease-in (gentle start, fast end) — good for wind-ups.
#[inline]
pub fn ease_in(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotation_is_orthonormal() {
        let r = M3::euler(v3(30.0, -45.0, 120.0));
        let x = r.apply(V3::X);
        let y = r.apply(V3::Y);
        assert!((x.len() - 1.0).abs() < 1e-5);
        assert!(x.dot(y).abs() < 1e-5);
        let inv = r.inverse().unwrap();
        let back = inv.apply(x);
        assert!((back - V3::X).len() < 1e-5);
    }

    #[test]
    fn xf_composition_order() {
        let a = Xf::translation(v3(1.0, 0.0, 0.0));
        let b = Xf::new(M3::rot_z(90.0), V3::ZERO);
        // (a*b)(p) = a(b(p)): rotate then translate.
        let p = (a * b).point(V3::X);
        assert!((p - v3(1.0, 1.0, 0.0)).len() < 1e-5);
    }

    #[test]
    fn mirror_detected() {
        let m = Xf::new(M3::scale(v3(-1.0, 1.0, 1.0)), V3::ZERO);
        assert!(m.mirrored());
        assert!(!Xf::IDENTITY.mirrored());
    }
}
