//! Move definitions: frame data and hitboxes for Kestrel's moveset.
//!
//! All numbers here are original tuning values. Frame data is expressed the way
//! platform-fighter players think about it — startup, active, endlag — plus a
//! hitbox with damage, angle, base knockback and knockback growth.

use super::math::Vec2;

/// Every action that can produce a hitbox.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoveId {
    Jab,
    Ftilt,
    Utilt,
    Dtilt,
    Fsmash,
    Usmash,
    Dsmash,
    DashAttack,
    Nair,
    Fair,
    Bair,
    Uair,
    Dair,
    SpecialN,    // projectile spawner
    SpecialUp,   // recovery hit
    SpecialSide, // lunge
    SpecialDown, // quick shockwave
    ThrowF,
    ThrowB,
    ThrowU,
    ThrowD,
}

/// A single hitbox. `offset.x` is *forward* (in the direction the fighter faces);
/// the state code multiplies by facing when placing it in the world.
#[derive(Clone, Copy, Debug)]
pub struct Hitbox {
    pub offset: Vec2,
    pub radius: f32,
    pub damage: f32,
    /// Launch angle in degrees, 0 = forward/horizontal, 90 = up, 270 = down.
    pub angle_deg: f32,
    pub kbg: f32,
    pub bkb: f32,
}

/// Frame data + hitbox for one move.
#[derive(Clone, Copy, Debug)]
pub struct MoveData {
    pub startup: u32,
    pub active: u32,
    pub endlag: u32,
    pub hitbox: Hitbox,
    /// Landing lag for aerials (before L-cancel). 0 for grounded moves.
    pub landing_lag: u32,
    pub is_aerial: bool,
    /// A short reach value the renderer uses to draw the strike pose.
    pub reach: f32,
}

impl MoveData {
    #[inline]
    pub fn total(&self) -> u32 {
        self.startup + self.active + self.endlag
    }
    /// Is `frame` (0-based within the move) an active hitbox frame?
    #[inline]
    pub fn is_active(&self, frame: u32) -> bool {
        frame >= self.startup && frame < self.startup + self.active
    }
}

// A compact constructor for the frame-data table below; the many parameters are
// intentional — it's a data row, not general-purpose API.
#[allow(clippy::too_many_arguments)]
const fn mv(
    startup: u32,
    active: u32,
    endlag: u32,
    off: (f32, f32),
    radius: f32,
    damage: f32,
    angle_deg: f32,
    kbg: f32,
    bkb: f32,
    landing_lag: u32,
    is_aerial: bool,
    reach: f32,
) -> MoveData {
    MoveData {
        startup,
        active,
        endlag,
        hitbox: Hitbox {
            offset: Vec2::new(off.0, off.1),
            radius,
            damage,
            angle_deg,
            kbg,
            bkb,
        },
        landing_lag,
        is_aerial,
        reach,
    }
}

/// Look up a move's data. This is the single source of truth for frame data.
pub fn data(id: MoveId) -> MoveData {
    use MoveId::*;
    match id {
        //         su ac el  offset        rad  dmg  ang   kbg   bkb  land aer reach
        Jab => mv(
            3,
            2,
            8,
            (13.0, 6.0),
            8.0,
            3.0,
            25.0,
            20.0,
            12.0,
            0,
            false,
            16.0,
        ),
        Ftilt => mv(
            6,
            3,
            14,
            (18.0, 4.0),
            9.0,
            8.0,
            32.0,
            70.0,
            15.0,
            0,
            false,
            22.0,
        ),
        Utilt => mv(
            5,
            4,
            15,
            (4.0, 18.0),
            10.0,
            7.0,
            95.0,
            90.0,
            18.0,
            0,
            false,
            22.0,
        ),
        Dtilt => mv(
            5,
            3,
            12,
            (16.0, -4.0),
            8.0,
            6.0,
            20.0,
            40.0,
            20.0,
            0,
            false,
            20.0,
        ),
        Fsmash => mv(
            12,
            3,
            26,
            (22.0, 4.0),
            11.0,
            15.0,
            38.0,
            95.0,
            25.0,
            0,
            false,
            28.0,
        ),
        Usmash => mv(
            10,
            4,
            24,
            (2.0, 22.0),
            12.0,
            14.0,
            90.0,
            100.0,
            28.0,
            0,
            false,
            28.0,
        ),
        Dsmash => mv(
            9,
            3,
            24,
            (20.0, -2.0),
            10.0,
            13.0,
            28.0,
            90.0,
            30.0,
            0,
            false,
            26.0,
        ),
        DashAttack => mv(
            8,
            4,
            20,
            (16.0, 6.0),
            10.0,
            9.0,
            55.0,
            60.0,
            35.0,
            0,
            false,
            22.0,
        ),

        Nair => mv(
            4,
            6,
            14,
            (12.0, 4.0),
            11.0,
            8.0,
            45.0,
            55.0,
            15.0,
            8,
            true,
            20.0,
        ),
        Fair => mv(
            7,
            4,
            18,
            (18.0, 6.0),
            9.0,
            10.0,
            40.0,
            75.0,
            15.0,
            12,
            true,
            24.0,
        ),
        Bair => mv(
            6,
            4,
            16,
            (-18.0, 4.0),
            9.0,
            11.0,
            40.0,
            80.0,
            18.0,
            10,
            true,
            24.0,
        ),
        Uair => mv(
            5,
            4,
            14,
            (2.0, 18.0),
            10.0,
            9.0,
            85.0,
            70.0,
            20.0,
            9,
            true,
            22.0,
        ),
        Dair => mv(
            9,
            5,
            22,
            (8.0, -16.0),
            9.0,
            12.0,
            270.0,
            45.0,
            40.0,
            18,
            true,
            22.0,
        ),

        SpecialN => mv(
            10,
            1,
            22,
            (16.0, 6.0),
            2.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0,
            false,
            20.0,
        ),
        SpecialUp => mv(
            6,
            8,
            26,
            (4.0, 14.0),
            12.0,
            6.0,
            80.0,
            60.0,
            30.0,
            0,
            false,
            22.0,
        ),
        SpecialSide => mv(
            10,
            6,
            24,
            (18.0, 4.0),
            10.0,
            9.0,
            30.0,
            55.0,
            45.0,
            0,
            false,
            24.0,
        ),
        SpecialDown => mv(
            8,
            3,
            26,
            (0.0, 0.0),
            22.0,
            5.0,
            70.0,
            45.0,
            35.0,
            0,
            false,
            24.0,
        ),

        // Throws: the "hitbox" carries the throw's launch parameters. Startup is
        // the release frame; active/endlag frame the release + throw endlag.
        ThrowF => mv(
            8,
            1,
            20,
            (16.0, 8.0),
            6.0,
            8.0,
            42.0,
            60.0,
            45.0,
            0,
            false,
            0.0,
        ),
        ThrowB => mv(
            10,
            1,
            22,
            (-16.0, 8.0),
            6.0,
            9.0,
            45.0,
            62.0,
            50.0,
            0,
            false,
            0.0,
        ),
        ThrowU => mv(
            8,
            1,
            20,
            (0.0, 18.0),
            6.0,
            7.0,
            88.0,
            65.0,
            55.0,
            0,
            false,
            0.0,
        ),
        ThrowD => mv(
            9,
            1,
            22,
            (6.0, 2.0),
            6.0,
            6.0,
            80.0,
            50.0,
            45.0,
            0,
            false,
            0.0,
        ),
    }
}

/// Projectile fired by SpecialN.
#[derive(Clone, Copy, Debug)]
pub struct Projectile {
    pub pos: Vec2,
    pub vel: Vec2,
    pub facing: f32,
    pub life: u32,
    pub owner: usize,
    pub active: bool,
}

pub const PROJECTILE_SPEED: f32 = 3.4;
pub const PROJECTILE_LIFE: u32 = 60;
pub const PROJECTILE_RADIUS: f32 = 4.0;
pub const PROJECTILE_DAMAGE: f32 = 5.0;
pub const PROJECTILE_KBG: f32 = 30.0;
pub const PROJECTILE_BKB: f32 = 30.0;
pub const PROJECTILE_ANGLE: f32 = 25.0;
