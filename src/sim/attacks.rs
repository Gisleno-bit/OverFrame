//! Move definitions: frame data and hitboxes, one table per character.
//!
//! All numbers here are original tuning values. Frame data is expressed the way
//! platform-fighter players think about it — startup, active, endlag — plus a
//! hitbox with damage, angle, base knockback and knockback growth. Look a move
//! up with [`data`]`(character, move)`; the tables are plain functions so adding
//! a fighter is adding one function and one match arm.

use super::math::Vec2;
use super::roster::CharacterId;

/// Every action that can produce a hitbox.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoveId {
    Jab,
    /// Second jab: attack again after the first jab's hit (jab 1 → jab 2).
    Jab2,
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
///
/// `startup` is the move's first active frame counted the way players count
/// ("hits on frame 4" ⇒ `startup: 4`; the input frame is frame 1). The clean
/// hit lasts `active` frames; `late_active` extra frames follow with a weaker
/// "late" hitbox (damage and base knockback × `late_scale`) — the lingering
/// second half of a sex kick. `endlag` frames complete the animation; the move
/// is interruptible (IASA) at `total()`.
#[derive(Clone, Copy, Debug)]
pub struct MoveData {
    pub startup: u32,
    pub active: u32,
    pub endlag: u32,
    pub hitbox: Hitbox,
    pub late_active: u32,
    pub late_scale: f32,
    /// Electric hits freeze both fighters 1.5× longer.
    pub electric: bool,
    /// Landing lag for aerials (before L-cancel). 0 for grounded moves.
    pub landing_lag: u32,
    pub is_aerial: bool,
    /// A short reach value the renderer uses to draw the strike pose.
    pub reach: f32,
}

impl MoveData {
    /// Interruptible-as-soon-as frame (the move's length).
    #[inline]
    pub fn total(&self) -> u32 {
        self.startup + self.active + self.late_active + self.endlag
    }
    /// Is `frame` (0-based within the move) an active hitbox frame (clean or
    /// late)?
    #[inline]
    pub fn is_active(&self, frame: u32) -> bool {
        frame >= self.startup && frame < self.startup + self.active + self.late_active
    }
    /// Is `frame` inside the clean (strong) window?
    #[inline]
    pub fn is_clean(&self, frame: u32) -> bool {
        frame >= self.startup && frame < self.startup + self.active
    }
    /// The hitbox in effect on `frame`, if any (late hits are scaled down).
    pub fn hitbox_at(&self, frame: u32) -> Option<Hitbox> {
        if self.is_clean(frame) {
            Some(self.hitbox)
        } else if self.is_active(frame) {
            Some(Hitbox {
                damage: (self.hitbox.damage * self.late_scale).round(),
                bkb: self.hitbox.bkb * self.late_scale,
                ..self.hitbox
            })
        } else {
            None
        }
    }
    /// Landing inside the first frames before the hitbox comes out, or once
    /// the swing is spent (last quarter of endlag), *autocancels*: normal
    /// landing lag instead of the aerial's.
    pub fn autocancels(&self, frame: u32) -> bool {
        let early = frame + 1 < self.startup;
        let late_start = self.startup + self.active + self.late_active + (self.endlag * 3) / 4;
        early || frame >= late_start
    }
    /// Add a late (weaker, lingering) hit window.
    pub const fn late(mut self, frames: u32, scale: f32) -> Self {
        self.late_active = frames;
        self.late_scale = scale;
        self
    }
    /// Mark the move electric (longer hitlag).
    pub const fn electric(mut self) -> Self {
        self.electric = true;
        self
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
        late_active: 0,
        late_scale: 1.0,
        electric: false,
        landing_lag,
        is_aerial,
        reach,
    }
}

/// Look up a move's data for a character. The single source of truth for
/// frame data.
pub fn data(ch: CharacterId, id: MoveId) -> MoveData {
    match ch {
        CharacterId::Kestrel => kestrel(id),
        CharacterId::Boulder => boulder(id),
        CharacterId::Viper => viper(id),
    }
}

/// Is this move a smash attack (for super armour and charge rules)?
#[inline]
pub fn is_smash(id: MoveId) -> bool {
    matches!(id, MoveId::Fsmash | MoveId::Usmash | MoveId::Dsmash)
}

/// Every move id, for validation tests and tooling.
pub const ALL_MOVES: [MoveId; 22] = [
    MoveId::Jab,
    MoveId::Jab2,
    MoveId::Ftilt,
    MoveId::Utilt,
    MoveId::Dtilt,
    MoveId::Fsmash,
    MoveId::Usmash,
    MoveId::Dsmash,
    MoveId::DashAttack,
    MoveId::Nair,
    MoveId::Fair,
    MoveId::Bair,
    MoveId::Uair,
    MoveId::Dair,
    MoveId::SpecialN,
    MoveId::SpecialUp,
    MoveId::SpecialSide,
    MoveId::SpecialDown,
    MoveId::ThrowF,
    MoveId::ThrowB,
    MoveId::ThrowU,
    MoveId::ThrowD,
];

/// Kestrel — balanced fast-faller: quick, medium damage, fair reach. Angle
/// 361 is the Sakurai angle (see `knockback::resolve_angle`).
#[rustfmt::skip]
fn kestrel(id: MoveId) -> MoveData {
    use MoveId::*;
    match id {
        //         su ac el  offset        rad  dmg  ang   kbg   bkb  land aer reach
        Jab => mv(1, 2, 14, (13.0, 6.0), 7.5, 3.0, 361.0, 20.0, 12.0, 0, false, 16.0),
        Jab2 => mv(2, 2, 18, (14.0, 7.0), 7.5, 4.0, 60.0, 30.0, 22.0, 0, false, 16.0),
        Ftilt => mv(4, 3, 18, (18.0, 4.0), 8.5, 8.0, 361.0, 70.0, 15.0, 0, false, 22.0),
        Utilt => mv(4, 4, 15, (4.0, 18.0), 9.5, 7.0, 95.0, 90.0, 18.0, 0, false, 22.0),
        Dtilt => mv(6, 3, 18, (16.0, -4.0), 8.0, 7.0, 25.0, 45.0, 20.0, 0, false, 20.0),
        Fsmash => mv(11, 3, 26, (22.0, 4.0), 10.5, 15.0, 38.0, 95.0, 25.0, 0, false, 28.0),
        Usmash => mv(7, 3, 22, (2.0, 22.0), 11.5, 15.0, 85.0, 105.0, 30.0, 0, false, 28.0).late(8, 0.7),
        Dsmash => mv(6, 3, 30, (20.0, -2.0), 10.0, 14.0, 28.0, 90.0, 30.0, 0, false, 26.0),
        DashAttack => mv(4, 4, 22, (16.0, 6.0), 10.0, 8.0, 70.0, 60.0, 35.0, 0, false, 22.0).late(6, 0.7),

        Nair => mv(3, 4, 10, (12.0, 4.0), 10.5, 11.0, 361.0, 100.0, 10.0, 15, true, 20.0).late(24, 0.75),
        Fair => mv(7, 4, 27, (18.0, 6.0), 9.0, 11.0, 40.0, 75.0, 15.0, 20, true, 24.0),
        Bair => mv(4, 4, 17, (-18.0, 4.0), 9.0, 13.0, 361.0, 80.0, 18.0, 20, true, 24.0).late(12, 0.6),
        Uair => mv(7, 4, 25, (2.0, 18.0), 10.0, 12.0, 85.0, 70.0, 20.0, 18, true, 22.0),
        Dair => mv(7, 5, 29, (8.0, -16.0), 9.0, 12.0, 270.0, 45.0, 40.0, 20, true, 22.0),

        SpecialN => mv(9, 1, 22, (16.0, 6.0), 2.0, 0.0, 0.0, 0.0, 0.0, 0, false, 20.0),
        SpecialUp => mv(6, 8, 26, (4.0, 14.0), 12.0, 6.0, 80.0, 60.0, 30.0, 0, false, 22.0),
        SpecialSide => mv(10, 6, 24, (18.0, 4.0), 10.0, 9.0, 30.0, 55.0, 45.0, 0, false, 24.0),
        SpecialDown => mv(0, 3, 28, (0.0, 0.0), 22.0, 5.0, 70.0, 45.0, 35.0, 0, false, 24.0).electric(),

        // Throws: the "hitbox" carries the throw's launch parameters. Startup is
        // the release frame; active/endlag frame the release + throw endlag.
        ThrowF => mv(8, 1, 20, (16.0, 8.0), 6.0, 8.0, 42.0, 60.0, 45.0, 0, false, 0.0),
        ThrowB => mv(10, 1, 22, (-16.0, 8.0), 6.0, 9.0, 45.0, 62.0, 50.0, 0, false, 0.0),
        ThrowU => mv(8, 1, 20, (0.0, 18.0), 6.0, 7.0, 88.0, 65.0, 55.0, 0, false, 0.0),
        ThrowD => mv(9, 1, 22, (6.0, 2.0), 6.0, 6.0, 80.0, 50.0, 45.0, 0, false, 0.0),
    }
}

/// Boulder — slow startup, big hitboxes, huge knockback. Every smash is a kill
/// move; every aerial lands with real lag. Grabs are the strongest in the cast.
#[rustfmt::skip]
fn boulder(id: MoveId) -> MoveData {
    use MoveId::*;
    match id {
        //         su  ac el  offset        rad   dmg  ang   kbg   bkb  land aer  reach
        Jab => mv(4, 2, 16, (15.0, 7.0), 9.5, 5.0, 361.0, 25.0, 18.0, 0, false, 18.0),
        Jab2 => mv(4, 3, 22, (17.0, 8.0), 10.0, 7.0, 55.0, 40.0, 30.0, 0, false, 20.0),
        Ftilt => mv(8, 4, 24, (21.0, 5.0), 11.5, 12.0, 361.0, 75.0, 22.0, 0, false, 26.0),
        Utilt => mv(8, 5, 22, (4.0, 22.0), 12.5, 11.0, 92.0, 95.0, 24.0, 0, false, 26.0),
        Dtilt => mv(8, 4, 22, (19.0, -5.0), 10.0, 10.0, 24.0, 45.0, 26.0, 0, false, 24.0),
        Fsmash => mv(17, 4, 34, (26.0, 5.0), 14.5, 21.0, 40.0, 100.0, 35.0, 0, false, 34.0),
        Usmash => mv(13, 4, 30, (2.0, 27.0), 14.5, 19.0, 88.0, 105.0, 36.0, 0, false, 34.0).late(8, 0.7),
        Dsmash => mv(11, 4, 34, (24.0, -3.0), 12.5, 18.0, 25.0, 95.0, 40.0, 0, false, 30.0),
        DashAttack => mv(9, 5, 28, (19.0, 7.0), 12.5, 13.0, 65.0, 65.0, 45.0, 0, false, 26.0).late(6, 0.7),

        Nair => mv(5, 6, 14, (14.0, 5.0), 13.5, 14.0, 361.0, 100.0, 12.0, 20, true, 24.0).late(22, 0.7),
        Fair => mv(11, 5, 30, (22.0, 6.0), 12.0, 16.0, 42.0, 85.0, 22.0, 24, true, 28.0),
        Bair => mv(8, 5, 25, (-22.0, 5.0), 12.0, 16.0, 361.0, 88.0, 24.0, 22, true, 28.0).late(10, 0.6),
        Uair => mv(9, 5, 26, (2.0, 22.0), 12.5, 14.0, 85.0, 78.0, 26.0, 20, true, 26.0),
        Dair => mv(13, 6, 30, (9.0, -19.0), 12.0, 17.0, 270.0, 55.0, 45.0, 26, true, 26.0),

        SpecialN => mv(14, 1, 28, (18.0, 7.0), 2.0, 0.0, 0.0, 0.0, 0.0, 0, false, 22.0),
        SpecialUp => mv(9, 10, 32, (5.0, 16.0), 15.0, 9.0, 82.0, 62.0, 40.0, 0, false, 26.0),
        SpecialSide => mv(14, 8, 30, (22.0, 5.0), 13.0, 13.0, 32.0, 60.0, 55.0, 0, false, 28.0),
        SpecialDown => mv(12, 4, 34, (0.0, 0.0), 30.0, 9.0, 75.0, 50.0, 48.0, 0, false, 30.0),

        ThrowF => mv(10, 1, 24, (18.0, 9.0), 6.0, 11.0, 42.0, 68.0, 55.0, 0, false, 0.0),
        ThrowB => mv(12, 1, 26, (-18.0, 9.0), 6.0, 12.0, 45.0, 70.0, 60.0, 0, false, 0.0),
        ThrowU => mv(10, 1, 24, (0.0, 20.0), 6.0, 10.0, 88.0, 75.0, 62.0, 0, false, 0.0),
        ThrowD => mv(11, 1, 26, (7.0, 2.0), 6.0, 9.0, 78.0, 55.0, 52.0, 0, false, 0.0),
    }
}

/// Viper — everything comes out fast and recovers fast, but hits are light and
/// knockback growth is low: a combo/zoning character that needs edgeguards to
/// close stocks.
#[rustfmt::skip]
fn viper(id: MoveId) -> MoveData {
    use MoveId::*;
    match id {
        //         su ac el  offset        rad  dmg  ang   kbg   bkb  land aer reach
        Jab => mv(1, 2, 11, (12.0, 5.0), 7.0, 2.0, 361.0, 15.0, 10.0, 0, false, 14.0),
        Jab2 => mv(2, 2, 15, (13.0, 6.0), 7.0, 3.0, 65.0, 25.0, 20.0, 0, false, 14.0),
        Ftilt => mv(4, 3, 14, (16.0, 4.0), 8.0, 6.0, 361.0, 60.0, 12.0, 0, false, 20.0),
        Utilt => mv(3, 4, 13, (3.0, 16.0), 9.0, 5.0, 96.0, 80.0, 15.0, 0, false, 20.0),
        Dtilt => mv(4, 3, 14, (15.0, -4.0), 7.0, 6.0, 20.0, 40.0, 18.0, 0, false, 18.0),
        Fsmash => mv(8, 3, 26, (20.0, 4.0), 9.5, 12.0, 36.0, 88.0, 22.0, 0, false, 26.0),
        Usmash => mv(6, 4, 22, (2.0, 20.0), 10.5, 12.0, 90.0, 92.0, 25.0, 0, false, 26.0).late(6, 0.7),
        Dsmash => mv(5, 3, 26, (18.0, -2.0), 9.0, 10.0, 26.0, 84.0, 26.0, 0, false, 24.0),
        DashAttack => mv(4, 4, 20, (15.0, 6.0), 9.0, 7.0, 70.0, 55.0, 30.0, 0, false, 20.0).late(5, 0.7),

        Nair => mv(2, 4, 10, (11.0, 4.0), 9.5, 9.0, 361.0, 100.0, 8.0, 12, true, 18.0).late(18, 0.7),
        Fair => mv(5, 4, 22, (17.0, 6.0), 8.0, 9.0, 38.0, 68.0, 12.0, 15, true, 22.0),
        Bair => mv(4, 4, 16, (-17.0, 4.0), 8.0, 11.0, 361.0, 74.0, 15.0, 16, true, 22.0).late(10, 0.6),
        Uair => mv(4, 4, 20, (2.0, 17.0), 9.0, 8.0, 85.0, 64.0, 16.0, 14, true, 20.0),
        Dair => mv(6, 5, 24, (7.0, -15.0), 8.0, 10.0, 270.0, 40.0, 32.0, 18, true, 20.0),

        SpecialN => mv(8, 1, 18, (15.0, 6.0), 2.0, 0.0, 0.0, 0.0, 0.0, 0, false, 18.0),
        SpecialUp => mv(5, 7, 22, (4.0, 13.0), 11.0, 5.0, 80.0, 58.0, 26.0, 0, false, 20.0),
        SpecialSide => mv(8, 6, 20, (17.0, 4.0), 9.0, 7.0, 30.0, 50.0, 38.0, 0, false, 22.0),
        SpecialDown => mv(0, 3, 24, (0.0, 0.0), 20.0, 5.0, 70.0, 42.0, 30.0, 0, false, 22.0).electric(),

        ThrowF => mv(7, 1, 18, (15.0, 8.0), 6.0, 6.0, 42.0, 55.0, 40.0, 0, false, 0.0),
        ThrowB => mv(9, 1, 20, (-15.0, 8.0), 6.0, 7.0, 45.0, 58.0, 44.0, 0, false, 0.0),
        ThrowU => mv(7, 1, 18, (0.0, 17.0), 6.0, 6.0, 88.0, 62.0, 48.0, 0, false, 0.0),
        ThrowD => mv(8, 1, 20, (6.0, 2.0), 6.0, 5.0, 80.0, 48.0, 40.0, 0, false, 0.0),
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

pub const PROJECTILE_SPEED: f32 = 7.0;
pub const PROJECTILE_LIFE: u32 = 60;
pub const PROJECTILE_RADIUS: f32 = 4.0;
pub const PROJECTILE_DAMAGE: f32 = 5.0;
pub const PROJECTILE_KBG: f32 = 30.0;
pub const PROJECTILE_BKB: f32 = 30.0;
pub const PROJECTILE_ANGLE: f32 = 25.0;
