//! Tuning constants for the engine and the per-character attribute struct.
//!
//! Values are in *world units per frame* at a fixed 60 Hz. The universal
//! mechanics (hitlag, hitstun, shieldstun, SDI, knockback decay, landing lag,
//! dodge timings) are calibrated against the publicly documented *behaviour*
//! of the reference platform fighter — frame counts and formulas from
//! community frame-data documentation (see `docs/GAME_FEEL.md`). Formulas and
//! timings are mechanics, not assets; per-move numbers stay original.
//!
//! Scale: one reference unit is [`REF_UNIT`] world units (fighters are ~30
//! tall here versus ~13.6 there), so any reference speed converts as
//! `speed × REF_UNIT`.

/// Simulation tick rate. Everything is expressed per-tick.
pub const FPS: u32 = 60;

/// World units per reference unit (see module docs).
pub const REF_UNIT: f32 = 2.2;

/// Per-character attributes. Fase 3 adds more of these; the engine reads a
/// character purely through this struct, so new fighters are pure data.
#[derive(Clone, Copy, Debug)]
pub struct Character {
    pub id: super::roster::CharacterId,
    pub name: &'static str,
    /// Short archetype label for the select screen ("FAST-FALLER", ...).
    pub archetype: &'static str,
    /// The character's signature mechanic, for the select screen.
    pub trait_name: &'static str,
    pub trait_desc: &'static str,

    // --- ground movement ---
    pub walk_max: f32,
    pub dash_max: f32,
    pub run_max: f32,
    pub ground_accel: f32,
    pub ground_friction: f32,
    /// Traction used while sliding out of a wavedash / waveland.
    pub traction: f32,

    // --- air movement ---
    pub air_max: f32,
    pub air_accel: f32,
    pub air_friction: f32,

    // --- vertical ---
    pub gravity: f32,
    pub max_fall: f32,
    pub fastfall: f32,
    pub fullhop_v: f32,
    pub shorthop_v: f32,
    pub doublejump_v: f32,
    pub air_jumps: u8,
    pub jumpsquat: u8,
    /// Frames of the initial dash during which the direction can be reversed
    /// (the dash-dance window). Fast characters: ~11; short: 7; long: 15.
    pub dash_frames: u32,

    // --- defensive ---
    pub airdodge_speed: f32,

    // --- physical size (a capsule) & KO resistance ---
    pub weight: f32,
    pub half_width: f32,
    pub height: f32,

    /// Super armour: knockback below this value is absorbed (damage still
    /// taken) during the startup of smash attacks. `0.0` disables it.
    pub smash_armor: f32,
}

/// The Fase 1 fighter, now defined with the rest of the roster.
pub use super::roster::KESTREL;

// ----- engine-wide constants -----

/// Frames after leaving a ledge/platform during which you may still jump
/// (coyote time), matching the forgiving feel of the reference game.
pub const COYOTE_FRAMES: u32 = 3;

/// Default dash-dance window (per-character `dash_frames` overrides it).
pub const DASH_DANCE_WINDOW: u32 = 11;

/// L-cancel: pressing shield within this many frames before landing during an
/// aerial halves that aerial's landing lag.
pub const LCANCEL_WINDOW: u32 = 7;

/// Air-dodge total duration and its intangibility window (inclusive frames).
/// Long and committal: after it the fighter is helpless until landing, which
/// is what makes wavedashing a *technique* rather than a free escape.
pub const AIRDODGE_DURATION: u32 = 49;
pub const AIRDODGE_INTANGIBLE: (u32, u32) = (4, 29);

/// Landing lag after a wavedash/waveland, during which you slide with traction.
pub const WAVEDASH_LANDLAG: u32 = 10;

/// Roll: total frames and intangibility window.
pub const ROLL_DURATION: u32 = 31;
pub const ROLL_INTANGIBLE: (u32, u32) = (4, 19);
pub const ROLL_DISTANCE: f32 = 60.0;

/// Spot-dodge: total frames and intangibility window.
pub const SPOTDODGE_DURATION: u32 = 22;
pub const SPOTDODGE_INTANGIBLE: (u32, u32) = (2, 15);

/// Normal (non-attack) landing lag, and the L-cancel rule: an L-cancelled
/// aerial lands with `floor(landing_lag / 2)`.
pub const LANDING_LAG_NORMAL: u32 = 4;

/// Frames to lower the shield before acting again (jump, grab and up-smash
/// out of shield skip this, as does a roll/spot-dodge).
pub const SHIELD_DROP: u32 = 15;
/// A shield raised this many frames ago or fewer *powershields*: no shield
/// damage, no shieldstun, no pushback.
pub const POWERSHIELD_WINDOW: u32 = 2;

/// Tech: pressing shield within this window as you collide while tumbling
/// techs (no getup lag); the lockout prevents mashing.
pub const TECH_WINDOW: u32 = 20;
pub const TECH_LOCKOUT: u32 = 40;
pub const TECH_INTANGIBLE: u32 = 20;

/// Ledge: intangibility granted on grabbing a ledge, and regrab cooldown.
pub const LEDGE_INTANGIBLE: u32 = 30;
pub const LEDGE_HOG_BOX: f32 = 12.0;

/// Shield: 60 HP, decays while held, regenerates while down; attacks deal
/// 0.7× to it. Shieldstun and pushback are formulas in `knockback.rs`.
pub const SHIELD_MAX: f32 = 60.0;
pub const SHIELD_REGEN: f32 = 0.07;
pub const SHIELD_DECAY: f32 = 0.28;
pub const SHIELD_DAMAGE_MULT: f32 = 0.70;

/// Knockback → launch speed (world units per frame): 0.03 reference units per
/// knockback unit, scaled.
pub const LAUNCH_SPEED_SCALE: f32 = 0.03 * REF_UNIT;
/// Hitstun in frames per knockback unit.
pub const HITSTUN_SCALE: f32 = 0.40;
/// Knockback velocity loses this much magnitude every frame (direction
/// preserved); gravity is a separate, capped fall velocity on top.
pub const KB_DECAY: f32 = 0.051 * REF_UNIT;
/// Knockback above this many units puts the victim into tumble (techable).
pub const TUMBLE_THRESHOLD: f32 = 80.0;
/// Maximum directional-influence angle shift (radians) — about 18°.
pub const DI_MAX_RAD: f32 = 0.314159;

/// Smash DI: each stick pulse during hitlag moves the victim this far; ASDI
/// (the held stick on the last hitlag frame) moves half as far.
pub const SDI_STEP: f32 = 6.0 * REF_UNIT;
pub const ASDI_STEP: f32 = 3.0 * REF_UNIT;

/// Hitlag (freeze frames) formula: `floor(damage / 3 + 3)`, ×1.5 if the move
/// is electric, ×2/3 for a crouch-cancelling victim, capped.
pub const HITLAG_CAP: u32 = 20;
pub const HITLAG_ELECTRIC: f32 = 1.5;
pub const CROUCH_CANCEL: f32 = 2.0 / 3.0;

/// "Sakurai angle" (a move angle of 361 in the tables): grounded victims
/// below this knockback are sent along the ground (0°), everyone else at
/// [`SAKURAI_ANGLE_DEG`].
pub const SAKURAI_GROUND_KB: f32 = 32.1;
pub const SAKURAI_ANGLE_DEG: f32 = 44.0;
