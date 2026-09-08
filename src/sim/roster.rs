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

    walk_max: 1.05,
    dash_max: 1.75,
    run_max: 1.60,
    ground_accel: 0.16,
    ground_friction: 0.110,
    traction: 0.070,

    air_max: 1.30,
    air_accel: 0.070,
    air_friction: 0.018,

    gravity: 0.240,
    max_fall: 2.70,
    fastfall: 3.90,
    fullhop_v: 5.10,
    shorthop_v: 3.30,
    doublejump_v: 4.40,
    air_jumps: 1,
    jumpsquat: 6,

    airdodge_speed: 2.60,

    weight: 122.0,
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

    walk_max: 1.75,
    dash_max: 2.45,
    run_max: 2.25,
    ground_accel: 0.26,
    ground_friction: 0.080,
    traction: 0.055,

    air_max: 2.15,
    air_accel: 0.150,
    air_friction: 0.022,

    gravity: 0.215,
    max_fall: 2.60,
    fastfall: 3.80,
    fullhop_v: 5.30,
    shorthop_v: 3.40,
    doublejump_v: 4.30,
    air_jumps: 2,
    jumpsquat: 3,

    airdodge_speed: 3.30,

    weight: 68.0,
    half_width: 7.5,
    height: 27.0,

    smash_armor: 0.0,
};
