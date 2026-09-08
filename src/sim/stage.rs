//! Stages. Original layouts and names; each is pure geometry plus a colour
//! theme the renderer uses. The *kinds* of layout (flat, floating platforms,
//! asymmetric) are genre conventions — the shapes, numbers, names and art are
//! ours.
//!
//! | Stage       | Layout                                              |
//! |-------------|-----------------------------------------------------|
//! | The Lattice | Main slab + three floating platforms (competitive standard) |
//! | Meridian    | One long flat slab, no platforms (pure neutral)     |
//! | Tidegate    | Shorter slab, one low-left and one high-right platform (asymmetric) |

use super::math::Vec2;

/// Identifies a stage. The `u8` repr travels in the lobby protocol.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum StageId {
    Lattice = 0,
    Meridian = 1,
    Tidegate = 2,
}

impl StageId {
    pub const ALL: [StageId; 3] = [StageId::Lattice, StageId::Meridian, StageId::Tidegate];

    pub fn from_u8(v: u8) -> Option<StageId> {
        StageId::ALL.get(v as usize).copied()
    }

    #[inline]
    pub fn index(self) -> usize {
        self as usize
    }

    pub fn build(self) -> Stage {
        match self {
            StageId::Lattice => Stage::lattice(),
            StageId::Meridian => Stage::meridian(),
            StageId::Tidegate => Stage::tidegate(),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            StageId::Lattice => "The Lattice",
            StageId::Meridian => "Meridian",
            StageId::Tidegate => "Tidegate",
        }
    }
}

/// A one-way (soft) or solid platform. `y` is the top surface (world units, y-up).
#[derive(Clone, Copy, Debug)]
pub struct Platform {
    pub left: f32,
    pub right: f32,
    pub y: f32,
    /// Solid platforms block from all sides and cannot be dropped through.
    pub solid: bool,
}

impl Platform {
    #[inline]
    pub fn contains_x(&self, x: f32) -> bool {
        x >= self.left && x <= self.right
    }
    #[inline]
    pub fn center_x(&self) -> f32 {
        (self.left + self.right) * 0.5
    }
}

/// A grabbable ledge at a platform corner. `side` is -1 for a left ledge, +1 for a right ledge.
#[derive(Clone, Copy, Debug)]
pub struct Ledge {
    pub pos: Vec2,
    pub side: f32,
}

/// Colour theme (plain RGB so the simulation stays renderer-agnostic).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Theme {
    pub bg_top: (u8, u8, u8),
    pub bg_bottom: (u8, u8, u8),
    pub slab: (u8, u8, u8),
    pub lip: (u8, u8, u8),
    pub soft: (u8, u8, u8),
    pub soft_glow: (u8, u8, u8),
}

/// The full stage: platforms, ledges, blast zones, spawns and theme.
#[derive(Clone, Debug)]
pub struct Stage {
    pub id: StageId,
    pub name: &'static str,
    pub platforms: Vec<Platform>,
    pub ledges: Vec<Ledge>,
    pub blast_left: f32,
    pub blast_right: f32,
    pub blast_top: f32,
    pub blast_bottom: f32,
    /// Where players spawn.
    pub spawns: Vec<Vec2>,
    pub theme: Theme,
}

fn ledges_for(main: &Platform) -> Vec<Ledge> {
    vec![
        Ledge {
            pos: Vec2::new(main.left, main.y),
            side: -1.0,
        },
        Ledge {
            pos: Vec2::new(main.right, main.y),
            side: 1.0,
        },
    ]
}

impl Stage {
    /// Build any stage by id.
    pub fn by_id(id: StageId) -> Stage {
        id.build()
    }

    /// The competitive standard: a slab and three floating platforms.
    pub fn lattice() -> Stage {
        let main = Platform {
            left: -155.0,
            right: 155.0,
            y: 0.0,
            solid: true,
        };
        let soft_l = Platform {
            left: -95.0,
            right: -35.0,
            y: 45.0,
            solid: false,
        };
        let soft_r = Platform {
            left: 35.0,
            right: 95.0,
            y: 45.0,
            solid: false,
        };
        let soft_top = Platform {
            left: -30.0,
            right: 30.0,
            y: 82.0,
            solid: false,
        };
        Stage {
            id: StageId::Lattice,
            name: "The Lattice",
            ledges: ledges_for(&main),
            platforms: vec![main, soft_l, soft_r, soft_top],
            blast_left: -285.0,
            blast_right: 285.0,
            blast_top: 235.0,
            blast_bottom: -190.0,
            spawns: vec![Vec2::new(-70.0, 1.0), Vec2::new(70.0, 1.0)],
            theme: Theme {
                bg_top: (18, 20, 34),
                bg_bottom: (30, 26, 52),
                slab: (44, 48, 74),
                lip: (120, 200, 230),
                soft: (90, 110, 170),
                soft_glow: (150, 210, 255),
            },
        }
    }

    /// A single long flat slab. Nothing to hide behind: pure neutral.
    pub fn meridian() -> Stage {
        let main = Platform {
            left: -190.0,
            right: 190.0,
            y: 0.0,
            solid: true,
        };
        Stage {
            id: StageId::Meridian,
            name: "Meridian",
            ledges: ledges_for(&main),
            platforms: vec![main],
            blast_left: -300.0,
            blast_right: 300.0,
            blast_top: 225.0,
            blast_bottom: -180.0,
            spawns: vec![Vec2::new(-80.0, 1.0), Vec2::new(80.0, 1.0)],
            theme: Theme {
                bg_top: (12, 26, 30),
                bg_bottom: (22, 44, 48),
                slab: (36, 60, 66),
                lip: (255, 196, 110),
                soft: (70, 120, 120),
                soft_glow: (255, 220, 150),
            },
        }
    }

    /// Asymmetric: a shorter slab, a low platform on the left and a high one on
    /// the right. Positions and stage control matter more than on Lattice.
    pub fn tidegate() -> Stage {
        let main = Platform {
            left: -135.0,
            right: 135.0,
            y: 0.0,
            solid: true,
        };
        let low_left = Platform {
            left: -120.0,
            right: -52.0,
            y: 34.0,
            solid: false,
        };
        let high_right = Platform {
            left: 30.0,
            right: 100.0,
            y: 70.0,
            solid: false,
        };
        Stage {
            id: StageId::Tidegate,
            name: "Tidegate",
            ledges: ledges_for(&main),
            platforms: vec![main, low_left, high_right],
            blast_left: -270.0,
            blast_right: 270.0,
            blast_top: 240.0,
            blast_bottom: -185.0,
            spawns: vec![Vec2::new(-60.0, 1.0), Vec2::new(60.0, 1.0)],
            theme: Theme {
                bg_top: (30, 16, 36),
                bg_bottom: (56, 26, 54),
                slab: (64, 42, 70),
                lip: (150, 240, 200),
                soft: (130, 80, 130),
                soft_glow: (200, 255, 230),
            },
        }
    }

    /// Is a point outside the blast zone (i.e. a KO)?
    #[inline]
    pub fn is_ko(&self, p: Vec2) -> bool {
        p.x < self.blast_left
            || p.x > self.blast_right
            || p.y > self.blast_top
            || p.y < self.blast_bottom
    }

    /// The main (solid) platform, by convention index 0.
    #[inline]
    pub fn main(&self) -> Platform {
        self.platforms[0]
    }
}
