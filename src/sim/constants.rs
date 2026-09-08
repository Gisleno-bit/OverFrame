//! Tuning constants for the engine and for the (currently single) character.
//!
//! Values are in *world units per frame* at a fixed 60 Hz. They are inspired by
//! the publicly documented movement/physics *behaviour* of fast-faller
//! archetypes in platform fighters — the numbers themselves are original and
//! hand-tuned for feel (see `tests/mechanics.rs` for the properties they must
//! satisfy). No data, code or assets from any other game are used.

/// Simulation tick rate. Everything is expressed per-tick.
pub const FPS: u32 = 60;

/// Per-character attributes. Fase 3 adds more of these; the engine reads a
/// character purely through this struct, so new fighters are pure data.
#[derive(Clone, Copy, Debug)]
pub struct Character {
    pub name: &'static str,

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

    // --- defensive ---
    pub airdodge_speed: f32,

    // --- physical size (a capsule) & KO resistance ---
    pub weight: f32,
    pub half_width: f32,
    pub height: f32,
}

/// The prototype fighter. An agile, fast-falling bruiser built to show off the
/// movement tech (dash-dance, wavedash, fast-fall, short-hop, L-cancel).
pub const KESTREL: Character = Character {
    name: "Kestrel",

    walk_max: 1.55,
    dash_max: 2.30,
    run_max: 2.05,
    ground_accel: 0.22,
    ground_friction: 0.090,
    traction: 0.050,

    air_max: 1.85,
    air_accel: 0.115,
    air_friction: 0.020,

    gravity: 0.290,
    max_fall: 3.20,
    fastfall: 4.85,
    fullhop_v: 5.60,
    shorthop_v: 3.70,
    doublejump_v: 4.80,
    air_jumps: 1,
    jumpsquat: 4,

    airdodge_speed: 3.10,

    weight: 90.0,
    half_width: 9.0,
    height: 30.0,
};

// ----- engine-wide constants -----

/// Frames after leaving a ledge/platform during which you may still jump
/// (coyote time), matching the forgiving feel of the reference game.
pub const COYOTE_FRAMES: u32 = 3;

/// Number of frames at the start of a dash during which pressing the opposite
/// direction produces a dash-back instead of a run — this is what enables
/// dash-dancing.
pub const DASH_DANCE_WINDOW: u32 = 11;

/// L-cancel: pressing shield within this many frames before landing during an
/// aerial halves that aerial's landing lag.
pub const LCANCEL_WINDOW: u32 = 7;

/// Air-dodge active duration and its intangibility window (inclusive frames).
pub const AIRDODGE_DURATION: u32 = 28;
pub const AIRDODGE_INTANGIBLE: (u32, u32) = (2, 19);

/// Landing lag after a wavedash/waveland, during which you slide with traction.
pub const WAVEDASH_LANDLAG: u32 = 10;

/// Roll: total frames and intangibility window.
pub const ROLL_DURATION: u32 = 32;
pub const ROLL_INTANGIBLE: (u32, u32) = (4, 18);
pub const ROLL_DISTANCE: f32 = 46.0;

/// Spot-dodge: total frames and intangibility window.
pub const SPOTDODGE_DURATION: u32 = 22;
pub const SPOTDODGE_INTANGIBLE: (u32, u32) = (2, 14);

/// Tech: pressing shield within this window as you collide while tumbling
/// techs (no getup lag); the lockout prevents mashing.
pub const TECH_WINDOW: u32 = 20;
pub const TECH_LOCKOUT: u32 = 40;
pub const TECH_INTANGIBLE: u32 = 20;

/// Ledge: intangibility granted on grabbing a ledge, and regrab cooldown.
pub const LEDGE_INTANGIBLE: u32 = 30;
pub const LEDGE_HOG_BOX: f32 = 12.0;

/// Shield.
pub const SHIELD_MAX: f32 = 60.0;
pub const SHIELD_REGEN: f32 = 0.08;
pub const SHIELD_DECAY: f32 = 0.24;
pub const SHIELD_DAMAGE_MULT: f32 = 0.70;
pub const SHIELDSTUN_MULT: f32 = 0.35;

/// Knockback → launch speed (world units per frame) and hitstun (frames).
pub const LAUNCH_SPEED_SCALE: f32 = 0.043;
pub const HITSTUN_SCALE: f32 = 0.40;
/// Horizontal deceleration applied to knockback velocity each frame.
pub const KB_FRICTION: f32 = 0.038;
/// Knockback above this many units puts the victim into tumble (techable).
pub const TUMBLE_THRESHOLD: f32 = 80.0;
/// Maximum directional-influence angle shift (radians) — about 18°.
pub const DI_MAX_RAD: f32 = 0.314159;

/// Global hitlag (freeze) frames scale with damage.
pub const HITLAG_BASE: f32 = 3.0;
pub const HITLAG_PER_DAMAGE: f32 = 0.30;
