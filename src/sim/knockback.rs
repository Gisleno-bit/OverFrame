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

/// Frozen "hitlag" frames for a hit of `damage`: `floor(d/3 + 3)`, ×1.5 if
/// `electric`, ×2/3 if the victim `crouch_cancels` (victim side only), capped
/// at [`k::HITLAG_CAP`]. Attacker and victim freeze for the same base amount.
#[inline]
pub fn hitlag(damage: f32, electric: bool, crouch_cancel: bool) -> u32 {
    let mut f = (damage / 3.0 + 3.0).floor();
    if electric {
        f = (f * k::HITLAG_ELECTRIC).floor();
    }
    if crouch_cancel {
        f = (f * k::CROUCH_CANCEL).floor();
    }
    (f as u32).min(k::HITLAG_CAP)
}

/// Shieldstun in frames for a blocked hit of `damage` (full shield):
/// `floor((d × 0.45 + 2) × 200/201)`.
#[inline]
pub fn shieldstun(damage: f32) -> u32 {
    ((damage * 0.45 + 2.0) * (200.0 / 201.0)) as u32
}

/// Shield pushback speed (world units/frame) given to the *defender* on
/// block: `min(2, d × 0.09 + 0.4)` reference units.
#[inline]
pub fn shield_push(damage: f32) -> f32 {
    (damage * 0.09 + 0.4).min(2.0) * k::REF_UNIT
}

/// Resolve a move's launch angle in degrees: 361 is the "Sakurai angle",
/// which sends grounded victims along the ground below a knockback threshold
/// and at 44° otherwise.
#[inline]
pub fn resolve_angle(angle_deg: f32, kb: f32, victim_grounded: bool) -> f32 {
    if angle_deg >= 360.0 {
        if victim_grounded && kb <= k::SAKURAI_GROUND_KB {
            0.0
        } else {
            k::SAKURAI_ANGLE_DEG
        }
    } else {
        angle_deg
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hitlag_matches_reference_formula() {
        // floor(d/3 + 3): 3% → 4, 9% → 6, 12% → 7, 20% → 9, 45% → 18, cap 20.
        assert_eq!(hitlag(3.0, false, false), 4);
        assert_eq!(hitlag(9.0, false, false), 6);
        assert_eq!(hitlag(12.0, false, false), 7);
        assert_eq!(hitlag(20.0, false, false), 9);
        assert_eq!(hitlag(60.0, false, false), 20, "capped");
        assert_eq!(hitlag(12.0, true, false), 10, "electric ×1.5");
        assert_eq!(hitlag(12.0, false, true), 4, "crouch cancel ×2/3");
    }

    #[test]
    fn shieldstun_and_push_match_reference() {
        // 13% → 7 frames on a full shield; 8% → 5.
        assert_eq!(shieldstun(13.0), 7);
        assert_eq!(shieldstun(8.0), 5);
        assert!(shield_push(12.0) > shield_push(4.0));
        assert!(
            (shield_push(30.0) - 2.0 * k::REF_UNIT).abs() < 1e-5,
            "capped at 2"
        );
    }

    #[test]
    fn sakurai_angle_keeps_weak_grounded_hits_on_the_ground() {
        assert_eq!(resolve_angle(361.0, 20.0, true), 0.0);
        assert_eq!(resolve_angle(361.0, 60.0, true), k::SAKURAI_ANGLE_DEG);
        assert_eq!(resolve_angle(361.0, 20.0, false), k::SAKURAI_ANGLE_DEG);
        assert_eq!(resolve_angle(80.0, 20.0, true), 80.0);
    }
}
