//! Knockback, hitstun and directional influence (DI).
//!
//! The knockback function reproduces the well-known, publicly documented
//! *formula* used by the reference platform fighter. A mathematical formula is
//! a mechanic, not a copyrightable asset — this is exactly the sort of thing the
//! project's legal strategy says is fine to reproduce. No game data is used; the
//! per-move numbers live in `attacks.rs` and are original tuning values.

use super::constants as k;
use super::math::Vec2;

/// Melee-style knockback.
///
/// * `percent_after` — the victim's damage percent **after** this hit lands.
/// * `damage` — the move's damage.
/// * `weight` — the victim's weight.
/// * `kbg` — knockback growth (per-move, ~100 = average).
/// * `bkb` — base knockback (per-move).
///
/// Returns knockback in the engine's internal units.
pub fn knockback(percent_after: f32, damage: f32, weight: f32, kbg: f32, bkb: f32) -> f32 {
    let p = percent_after;
    let d = damage;
    let w = weight;
    let base = (((p / 10.0) + (p * d / 20.0)) * (200.0 / (w + 100.0)) * 1.4) + 18.0;
    (base * (kbg / 100.0)) + bkb
}

/// Hitstun in frames for a given knockback value.
#[inline]
pub fn hitstun(kb: f32) -> u32 {
    (kb * k::HITSTUN_SCALE) as u32
}

/// Frozen "hitlag" frames (both attacker and victim) for a given move damage.
#[inline]
pub fn hitlag(damage: f32) -> u32 {
    (k::HITLAG_BASE + damage * k::HITLAG_PER_DAMAGE) as u32
}

/// Apply directional influence to a launch angle.
///
/// `base_angle` is in radians (0 = +x/right, +y up). The component of the DI
/// stick perpendicular to the launch direction rotates the angle by up to
/// [`k::DI_MAX_RAD`].
pub fn apply_di(base_angle: f32, stick: Vec2) -> f32 {
    let kb_dir = Vec2::new(base_angle.cos(), base_angle.sin());
    // z-component of cross(kb_dir, stick): positive => stick is CCW of launch dir.
    let perp = kb_dir.x * stick.y - kb_dir.y * stick.x;
    let perp = super::math::clampf(perp, -1.0, 1.0);
    base_angle + k::DI_MAX_RAD * perp
}

/// Convert a knockback magnitude + angle into an initial launch velocity.
#[inline]
pub fn launch_velocity(kb: f32, angle: f32) -> Vec2 {
    let speed = kb * k::LAUNCH_SPEED_SCALE;
    Vec2::new(angle.cos() * speed, angle.sin() * speed)
}

/// Does this knockback send the victim into (techable) tumble?
#[inline]
pub fn causes_tumble(kb: f32) -> bool {
    kb >= k::TUMBLE_THRESHOLD
}
