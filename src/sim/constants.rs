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

/// Tech: a shield press while tumbling opens a [`TECH_WINDOW`]-frame window;
/// touching the ground inside it techs. Any press also starts a lockout
/// ([`TECH_WINDOW`] + [`TECH_LOCKOUT`] frames) during which new presses do
/// nothing, so mashing shield does not tech. Tech in place / tech roll as
/// `(total, intangible)`; the roll covers [`TECH_ROLL_DISTANCE`].
pub const TECH_WINDOW: u32 = 20;
pub const TECH_LOCKOUT: u32 = 40;
pub const TECH_INTANGIBLE: u32 = 20;
pub const TECH_IN_PLACE: (u32, u32) = (26, 20);
pub const TECH_ROLL: (u32, u32) = (40, 20);
pub const TECH_ROLL_DISTANCE: f32 = 70.0;

/// Missed tech: [`KNOCKDOWN_BOUNCE`] frames on the floor before you may act,
/// then stand / roll / getup attack as `(total, intangible)`; the getup
/// attack strikes in front on [`GETUP_ATTACK_HITS`].0 and behind on `.1`
/// (each for [`GETUP_ATTACK_ACTIVE`] frames) with a fixed weak hitbox.
pub const KNOCKDOWN_BOUNCE: u32 = 18;
pub const GETUP_STAND: (u32, u32) = (30, 22);
pub const GETUP_ROLL: (u32, u32) = (35, 25);
pub const GETUP_ROLL_DISTANCE: f32 = 66.0;
pub const GETUP_ATTACK: (u32, u32) = (49, 14);
pub const GETUP_ATTACK_HITS: (u32, u32) = (15, 21);
pub const GETUP_ATTACK_ACTIVE: u32 = 3;
pub const GETUP_ATTACK_DAMAGE: f32 = 6.0;

/// Smash attacks: attack pressed within [`SMASH_FLICK_FRAMES`] frames of the
/// stick crossing the hard threshold is a smash (a held stick is a tilt);
/// holding attack keeps the smash charging at its charge frame for up to
/// [`SMASH_CHARGE_MAX`] frames, scaling damage up to ×(1 + SMASH_CHARGE_BONUS)
/// — knockback follows the charged damage. C-stick smashes do not charge.
pub const SMASH_FLICK_FRAMES: u32 = 3;
pub const SMASH_CHARGE_MAX: u32 = 60;
pub const SMASH_CHARGE_BONUS: f32 = 0.367;

/// Reversing a *run* (past the dash-dance window) is a slow turn: this many
/// frames of braking during which only a jump comes out.
pub const RUN_TURN: u32 = 20;

/// Clank: two grounded attacks whose hitboxes touch cancel each other when
/// their damages differ by less than [`CLANK_DIFF`], sending both fighters
/// into a rebound of `⌊max damage / 3⌋ + REBOUND_BASE` frames; otherwise the
/// weaker hit is simply cancelled and the stronger one lands.
pub const CLANK_DIFF: f32 = 9.0;
pub const REBOUND_BASE: u32 = 8;

/// Stale-move negation: the last [`STALE_QUEUE`] hits that connected;
/// each earlier use of the same move takes its slot's share off the damage
/// (0.09 for the most recent … 0.01 for the oldest). Knockback is computed
/// from the fresh damage (it mostly ignores staleness). Reset on a KO.
pub const STALE_QUEUE: usize = 9;
pub const STALE_STEPS: [f32; STALE_QUEUE] = [0.09, 0.08, 0.07, 0.06, 0.05, 0.04, 0.03, 0.02, 0.01];

/// Meteor cancel: a spike (launch angle within [`METEOR_ANGLES`], degrees
/// below the horizon) can be cancelled with a jump or an up-special once
/// this many frames of hitstun have passed.
pub const METEOR_CANCEL_FRAMES: u32 = 8;
pub const METEOR_ANGLES: (f32, f32) = (250.0, 290.0);

/// Ledge. Catching one takes [`LEDGE_CATCH`] uncontrollable frames; the
/// catch grants [`LEDGE_INTANGIBLE`] frames of intangibility in total (kept
/// if you let go — that is the ledgedash), and you may hang for
/// [`LEDGE_HANG`] frames (fresh / tired) before dropping. At
/// [`LEDGE_TIRED_PERCENT`] or more every getup option is the slow variant.
pub const LEDGE_CATCH: u32 = 7;
pub const LEDGE_INTANGIBLE: u32 = 37;
pub const LEDGE_HANG: [u32; 2] = [660, 480];
pub const LEDGE_TIRED_PERCENT: f32 = 100.0;
pub const LEDGE_HOG_BOX: f32 = 12.0;
pub const LEDGE_REGRAB_CD: u32 = 22;
/// Getup options as `(total frames, intangible frames)`, `[fresh, tired]`.
/// Original class-shaped values: fast and safe below 100 %, long and
/// punishable above it.
pub const LEDGE_GETUP: [(u32, u32); 2] = [(33, 29), (59, 50)];
pub const LEDGE_ROLL: [(u32, u32); 2] = [(49, 30), (79, 50)];
/// Ledge attack: `(total, intangible, first hit frame)`; the hit lasts
/// [`LEDGE_ATTACK_ACTIVE`] frames and uses the character's forward tilt.
pub const LEDGE_ATTACK: [(u32, u32, u32); 2] = [(55, 20, 24), (69, 34, 42)];
pub const LEDGE_ATTACK_ACTIVE: u32 = 3;
/// Ledge jump: frames on the ledge before leaving, then frames airborne with
/// no actions (drift only) — a committed option, as in the reference.
pub const LEDGE_JUMP: (u32, u32) = (10, 20);
/// How far onto the stage a getup / roll puts you (beyond half-width).
pub const LEDGE_GETUP_STEP: f32 = 4.0;
pub const LEDGE_ROLL_STEP: f32 = 44.0;

/// Grabs. Standing grab: hit frames 7–8 of 30; dash grab (from a dash or
/// run): 12–13 of 40 with a slide. A hold lasts `⌊76 + 1.6 × percent⌋`
/// frames, and every fresh input from the victim (button press or stick
/// direction) takes [`GRAB_MASH_FRAMES`] off it. Pummels deal
/// [`PUMMEL_DAMAGE`] every [`PUMMEL_COOLDOWN`] frames. When the hold runs
/// out both fighters get [`GRAB_RELEASE_LAG`] frames.
pub const GRAB_STAND: (u32, u32, u32) = (7, 8, 30);
pub const GRAB_DASH: (u32, u32, u32) = (12, 13, 40);
pub const GRAB_REACH: f32 = 16.0;
pub const GRAB_HOLD_BASE: f32 = 76.0;
pub const GRAB_HOLD_PER_PERCENT: f32 = 1.6;
pub const GRAB_MASH_FRAMES: u32 = 6;
pub const PUMMEL_DAMAGE: f32 = 2.0;
pub const PUMMEL_COOLDOWN: u32 = 20;
pub const PUMMEL_HITLAG: u32 = 3;
pub const GRAB_RELEASE_LAG: u32 = 30;
pub const GRAB_RELEASE_PUSH: f32 = 1.4 * REF_UNIT;

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
