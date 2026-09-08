//! Gamepad mapping, independent of any input library.
//!
//! The runtime (`render`) reads physical controllers through `gilrs` and turns
//! each into a [`PadSnapshot`]; this module turns a snapshot plus the player's
//! [`PadBindings`] into the simulation's [`PlayerInput`]. Keeping the mapping
//! pure means it is unit-tested without hardware (`tests/gamepad_map.rs`), and
//! the default layout mirrors the classic platform-fighter controller:
//!
//! | Action  | Default buttons                          |
//! |---------|------------------------------------------|
//! | Attack  | South (A)                                |
//! | Special | East (B)                                 |
//! | Jump    | North (Y) or West (X)                    |
//! | Shield  | Left/Right analog trigger, Left bumper   |
//! | Grab    | Right bumper (the "Z" position)          |
//!
//! Left stick = control stick, right stick = C-stick.

use crate::sim::input::{buttons, PlayerInput};
use crate::sim::math::Vec2;

/// Physical buttons we recognise. Values are bit positions in a `u32` mask so a
/// binding can hold several buttons for one action.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum PadButton {
    South = 0,
    East = 1,
    North = 2,
    West = 3,
    LeftBumper = 4,
    RightBumper = 5,
    LeftTrigger = 6,
    RightTrigger = 7,
    Select = 8,
    Start = 9,
    LeftThumb = 10,
    RightThumb = 11,
    DPadUp = 12,
    DPadDown = 13,
    DPadLeft = 14,
    DPadRight = 15,
}

impl PadButton {
    pub const ALL: [PadButton; 16] = [
        PadButton::South,
        PadButton::East,
        PadButton::North,
        PadButton::West,
        PadButton::LeftBumper,
        PadButton::RightBumper,
        PadButton::LeftTrigger,
        PadButton::RightTrigger,
        PadButton::Select,
        PadButton::Start,
        PadButton::LeftThumb,
        PadButton::RightThumb,
        PadButton::DPadUp,
        PadButton::DPadDown,
        PadButton::DPadLeft,
        PadButton::DPadRight,
    ];

    #[inline]
    pub const fn mask(self) -> u32 {
        1 << (self as u32)
    }

    pub fn from_index(i: u32) -> Option<PadButton> {
        PadButton::ALL.get(i as usize).copied()
    }

    /// Short label for the options screen.
    pub fn label(self) -> &'static str {
        match self {
            PadButton::South => "A / SOUTH",
            PadButton::East => "B / EAST",
            PadButton::North => "Y / NORTH",
            PadButton::West => "X / WEST",
            PadButton::LeftBumper => "LB",
            PadButton::RightBumper => "RB",
            PadButton::LeftTrigger => "LT",
            PadButton::RightTrigger => "RT",
            PadButton::Select => "SELECT",
            PadButton::Start => "START",
            PadButton::LeftThumb => "L3",
            PadButton::RightThumb => "R3",
            PadButton::DPadUp => "DPAD UP",
            PadButton::DPadDown => "DPAD DOWN",
            PadButton::DPadLeft => "DPAD LEFT",
            PadButton::DPadRight => "DPAD RIGHT",
        }
    }
}

/// Human-readable list of the buttons in a mask, e.g. `"Y / NORTH + X / WEST"`.
pub fn mask_label(mask: u32) -> String {
    let parts: Vec<&str> = PadButton::ALL
        .iter()
        .filter(|b| mask & b.mask() != 0)
        .map(|b| b.label())
        .collect();
    if parts.is_empty() {
        "(NONE)".to_owned()
    } else {
        parts.join(" + ")
    }
}

/// A controller's state at one instant. Axes are in `[-1, 1]` (`+y` up),
/// triggers in `[0, 1]`, `buttons` is a [`PadButton`] mask.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PadSnapshot {
    pub left: Vec2,
    pub right: Vec2,
    pub left_trigger: f32,
    pub right_trigger: f32,
    pub buttons: u32,
}

impl PadSnapshot {
    #[inline]
    pub fn pressed(&self, b: PadButton) -> bool {
        self.buttons & b.mask() != 0
    }
}

/// The actions a gamepad can be bound to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PadAction {
    Jump,
    Attack,
    Special,
    Shield,
    Grab,
}

impl PadAction {
    pub const ALL: [PadAction; 5] = [
        PadAction::Jump,
        PadAction::Attack,
        PadAction::Special,
        PadAction::Shield,
        PadAction::Grab,
    ];
    pub fn label(self) -> &'static str {
        match self {
            PadAction::Jump => "JUMP",
            PadAction::Attack => "ATTACK",
            PadAction::Special => "SPECIAL",
            PadAction::Shield => "SHIELD",
            PadAction::Grab => "GRAB",
        }
    }
}

/// Which physical buttons trigger each action (masks), plus the stick deadzone.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PadBindings {
    pub jump: u32,
    pub attack: u32,
    pub special: u32,
    pub shield: u32,
    pub grab: u32,
    /// Radial deadzone for both sticks.
    pub deadzone: f32,
}

impl Default for PadBindings {
    fn default() -> Self {
        PadBindings {
            jump: PadButton::North.mask() | PadButton::West.mask(),
            attack: PadButton::South.mask(),
            special: PadButton::East.mask(),
            shield: PadButton::LeftTrigger.mask()
                | PadButton::RightTrigger.mask()
                | PadButton::LeftBumper.mask(),
            grab: PadButton::RightBumper.mask(),
            deadzone: 0.20,
        }
    }
}

impl PadBindings {
    pub fn get(&self, a: PadAction) -> u32 {
        match a {
            PadAction::Jump => self.jump,
            PadAction::Attack => self.attack,
            PadAction::Special => self.special,
            PadAction::Shield => self.shield,
            PadAction::Grab => self.grab,
        }
    }

    /// Bind `action` to exactly `button` (replacing previous buttons) and remove
    /// that button from every other action so one press means one thing.
    pub fn rebind(&mut self, action: PadAction, button: PadButton) {
        let m = button.mask();
        for a in PadAction::ALL {
            let cur = self.get(a) & !m;
            self.set(a, cur);
        }
        self.set(action, m);
    }

    fn set(&mut self, a: PadAction, mask: u32) {
        match a {
            PadAction::Jump => self.jump = mask,
            PadAction::Attack => self.attack = mask,
            PadAction::Special => self.special = mask,
            PadAction::Shield => self.shield = mask,
            PadAction::Grab => self.grab = mask,
        }
    }
}

/// Apply a radial deadzone and rescale so the edge of the deadzone maps to 0
/// and full deflection stays 1.
pub fn apply_deadzone(v: Vec2, deadzone: f32) -> Vec2 {
    let len = v.length();
    if len <= deadzone {
        return Vec2::ZERO;
    }
    let scaled = ((len - deadzone) / (1.0 - deadzone)).min(1.0);
    v.normalized_or_zero() * scaled
}

/// Trigger pull that counts as a press.
pub const TRIGGER_THRESHOLD: f32 = 0.45;

/// Turn a controller snapshot into simulation input.
pub fn map(snap: &PadSnapshot, b: &PadBindings) -> PlayerInput {
    let stick = apply_deadzone(snap.left, b.deadzone);
    let cstick = apply_deadzone(snap.right, b.deadzone);

    // Analog triggers act as their digital buttons past a threshold.
    let mut held = snap.buttons;
    if snap.left_trigger > TRIGGER_THRESHOLD {
        held |= PadButton::LeftTrigger.mask();
    }
    if snap.right_trigger > TRIGGER_THRESHOLD {
        held |= PadButton::RightTrigger.mask();
    }

    let mut out = 0u16;
    if held & b.jump != 0 {
        out |= buttons::JUMP;
    }
    if held & b.attack != 0 {
        out |= buttons::ATTACK;
    }
    if held & b.special != 0 {
        out |= buttons::SPECIAL;
    }
    if held & b.shield != 0 {
        out |= buttons::SHIELD;
    }
    if held & b.grab != 0 {
        out |= buttons::GRAB;
    }
    PlayerInput {
        stick,
        cstick,
        buttons: out,
    }
}

/// Merge two input sources for the same player (e.g. keyboard + gamepad) so
/// either device works: buttons OR together, the stronger stick wins.
pub fn merge(a: PlayerInput, b: PlayerInput) -> PlayerInput {
    let stick = if b.stick.length_sq() > a.stick.length_sq() {
        b.stick
    } else {
        a.stick
    };
    let cstick = if b.cstick.length_sq() > a.cstick.length_sq() {
        b.cstick
    } else {
        a.cstick
    };
    PlayerInput {
        stick,
        cstick,
        buttons: a.buttons | b.buttons,
    }
}
