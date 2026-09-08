//! The roster: every playable character as pure data.
//!
//! A character is a [`Character`] attribute block (movement, weight, size, a
//! signature *trait*) plus a move table in [`super::attacks`]. Adding a fighter
//! is adding a `CharacterId`, one attribute block and one table — the engine
//! never special-cases names. All designs, names and numbers are original.
//!
//! Archetypes covered so far (the classic platform-fighter spread):
//!
//! | Fighter  | Archetype     | Trait                                              |
//! |----------|---------------|----------------------------------------------------|
//! | Kestrel  | Fast-faller   | *Momentum*: fastest fall speed, longest wavedash   |
//! | Boulder  | Heavyweight   | *Bulwark*: super armour during smash-attack startup |
//! | Viper    | Lightweight   | *Skyline*: two air jumps, best air control          |

use super::constants::Character;

/// Identifies a playable character. The `u8` repr is what travels in the
/// lobby protocol and the match config.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum CharacterId {
    Kestrel = 0,
    Boulder = 1,
    Viper = 2,
}

impl CharacterId {
    pub const ALL: [CharacterId; 3] = [
        CharacterId::Kestrel,
        CharacterId::Boulder,
        CharacterId::Viper,
    ];

    pub fn from_u8(v: u8) -> Option<CharacterId> {
        CharacterId::ALL.get(v as usize).copied()
    }

    #[inline]
    pub fn index(self) -> usize {
        self as usize
    }

    /// The attribute block for this character.
    pub fn data(self) -> &'static Character {
        match self {
            CharacterId::Kestrel => &KESTREL,
            CharacterId::Boulder => &BOULDER,
            CharacterId::Viper => &VIPER,
        }
    }

    pub fn name(self) -> &'static str {
        self.data().name
    }
}

/// Number of colour palettes each character offers.
pub const PALETTES: u8 = 6;

/// Kestrel — the Fase 1 fighter. Agile fast-faller: the movement-tech character.
pub const KESTREL: Character = Character {
    id: CharacterId::Kestrel,
    name: "Kestrel",
    archetype: "FAST-FALLER",
    trait_name: "MOMENTUM",
    trait_desc: "FASTEST FALL SPEED AND LONGEST WAVEDASH",

    // Fast-faller class (reference units in comments; ×2.2 world units).
    walk_max: 3.45,         // 1.57
    dash_max: 4.10,         // 1.86 initial dash
    run_max: 4.70,          // 2.14
    ground_accel: 0.44,     // dash accel ≈ 0.2
    ground_friction: 0.180, // traction 0.08
    traction: 0.150,

    air_max: 1.85,       // 0.84
    air_accel: 0.22,     // 0.10
    air_friction: 0.045, // 0.02

    gravity: 0.50,    // 0.23
    max_fall: 6.10,   // 2.8
    fastfall: 7.40,   // 3.4
    fullhop_v: 8.10,  // full hop ≈ 30 ref units
    shorthop_v: 4.85, // short hop ≈ 10.7 ref units
    doublejump_v: 7.90,
    air_jumps: 1,
    jumpsquat: 3,
    dash_frames: 11,

    airdodge_speed: 6.60, // 3.0

    weight: 90.0,
    half_width: 9.0,
    height: 30.0,

    smash_armor: 0.0,
};

/// Boulder — a slow, heavy bruiser. Hits like a truck, dies late, and can
/// shrug off pokes while winding up a smash (super armour).
pub const BOULDER: Character = Character {
    id: CharacterId::Boulder,
    name: "Boulder",
    archetype: "HEAVYWEIGHT",
    trait_name: "BULWARK",
    trait_desc: "SUPER ARMOUR WHILE CHARGING A SMASH ATTACK",

    // Heavyweight class: slow on the ground, average fall, big jumps are not
    // its thing.
    walk_max: 1.65, // 0.75
    dash_max: 3.20, // 1.45
    run_max: 3.50,  // 1.6
    ground_accel: 0.28,
    ground_friction: 0.200,
    traction: 0.170,

    air_max: 1.40, // 0.64
    air_accel: 0.12,
    air_friction: 0.030,

    gravity: 0.26,    // 0.12
    max_fall: 4.60,   // 2.1
    fastfall: 6.40,   // 2.9
    fullhop_v: 5.35,  // full hop ≈ 25 ref units
    shorthop_v: 3.25, // ≈ 9
    doublejump_v: 5.20,
    air_jumps: 1,
    jumpsquat: 6,
    dash_frames: 13,

    airdodge_speed: 5.90,

    weight: 118.0,
    half_width: 12.0,
    height: 36.0,

    // Incoming knockback below this is absorbed during smash startup.
    smash_armor: 60.0,
};

/// Viper — a light, quick zoner/acrobat. Two air jumps and top air control,
/// but weak hits and an early death.
pub const VIPER: Character = Character {
    id: CharacterId::Viper,
    name: "Viper",
    archetype: "LIGHTWEIGHT",
    trait_name: "SKYLINE",
    trait_desc: "TWO AIR JUMPS AND THE BEST AIR CONTROL",

    // Lightweight / acrobat class: quick dash with a short dash-dance, floaty
    // but with a real fast-fall, the best air control in the cast.
    walk_max: 2.90, // 1.32
    dash_max: 3.90, // 1.77
    run_max: 4.20,  // 1.9
    ground_accel: 0.48,
    ground_friction: 0.150,
    traction: 0.120,

    air_max: 2.15, // 0.98
    air_accel: 0.28,
    air_friction: 0.040,

    gravity: 0.30,    // 0.136
    max_fall: 4.80,   // 2.2
    fastfall: 6.60,   // 3.0
    fullhop_v: 6.70,  // full hop ≈ 34 ref units
    shorthop_v: 4.30, // ≈ 14
    doublejump_v: 6.30,
    air_jumps: 2,
    jumpsquat: 3,
    dash_frames: 7,

    airdodge_speed: 6.80,

    weight: 68.0,
    half_width: 7.5,
    height: 27.0,

    smash_armor: 0.0,
};
