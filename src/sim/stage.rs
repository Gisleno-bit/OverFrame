//! Stage geometry. Original layout ("The Lattice") — a solid central platform
//! plus three drop-through platforms, with blast zones around it. The
//! *proportions* echo the competitive "battlefield" family (that arrangement is
//! a game-design idea, not a protectable asset); the numbers, name and art are
//! original.

use super::math::Vec2;

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

/// The full stage: platforms, ledges and blast zones.
#[derive(Clone, Debug)]
pub struct Stage {
    pub name: &'static str,
    pub platforms: Vec<Platform>,
    pub ledges: Vec<Ledge>,
    pub blast_left: f32,
    pub blast_right: f32,
    pub blast_top: f32,
    pub blast_bottom: f32,
    /// Where players spawn.
    pub spawns: Vec<Vec2>,
}

impl Stage {
    /// The prototype's default stage.
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

        let ledges = vec![
            Ledge {
                pos: Vec2::new(main.left, main.y),
                side: -1.0,
            },
            Ledge {
                pos: Vec2::new(main.right, main.y),
                side: 1.0,
            },
        ];

        Stage {
            name: "The Lattice",
            platforms: vec![main, soft_l, soft_r, soft_top],
            ledges,
            blast_left: -285.0,
            blast_right: 285.0,
            blast_top: 235.0,
            blast_bottom: -190.0,
            spawns: vec![Vec2::new(-70.0, 1.0), Vec2::new(70.0, 1.0)],
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
