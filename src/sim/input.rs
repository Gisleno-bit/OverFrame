//! Player input: the only thing that drives the simulation forward.
//!
//! Two representations exist:
//!
//! * [`NetInput`] — a tiny, `Pod` value (8 bytes) that is what GGRS transmits
//!   over the wire and stores in its input queue.
//! * [`PlayerInput`] — the decoded, ergonomic form the simulation reads
//!   (analog sticks as `f32`, buttons as bits).
//!
//! Edge detection ("was this button *just* pressed?") is not stored here; each
//! fighter keeps the previous frame's buttons as part of its saved state, so
//! rollback reproduces edges exactly.

/// Button bitmask values. A control stick is analog and handled separately.
pub mod buttons {
    pub const ATTACK: u16 = 1 << 0; // A — tilts / smashes / aerials / jab
    pub const SPECIAL: u16 = 1 << 1; // B — special moves
    pub const JUMP: u16 = 1 << 2; // X / Y — jump, also used for platform drop
    pub const SHIELD: u16 = 1 << 3; // L / R — shield, air dodge, L-cancel, roll, spot dodge
    pub const GRAB: u16 = 1 << 4; // Z — grab
}

/// Decoded input read by the simulation.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PlayerInput {
    /// Control stick, each axis in `[-1.0, 1.0]`. `+y` is up.
    pub stick: crate::sim::math::Vec2,
    /// C-stick, each axis in `[-1.0, 1.0]`. Used for smashes / aerials.
    pub cstick: crate::sim::math::Vec2,
    /// Pressed-button bitmask (see [`buttons`]).
    pub buttons: u16,
}

impl PlayerInput {
    #[inline]
    pub fn held(&self, mask: u16) -> bool {
        self.buttons & mask != 0
    }
}

/// Compact, network-transmittable input. `#[repr(C)]` + all-`Pod` fields so
/// GGRS can treat it as bytes. 8 bytes/frame/player.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct NetInput {
    pub buttons: u16,
    pub stick_x: i8,
    pub stick_y: i8,
    pub cstick_x: i8,
    pub cstick_y: i8,
    pub _pad: u16,
}

impl NetInput {
    /// Encode a decoded input into wire form.
    pub fn encode(i: &PlayerInput) -> Self {
        let q = |v: f32| -> i8 { (crate::sim::math::clampf(v, -1.0, 1.0) * 127.0) as i8 };
        NetInput {
            buttons: i.buttons,
            stick_x: q(i.stick.x),
            stick_y: q(i.stick.y),
            cstick_x: q(i.cstick.x),
            cstick_y: q(i.cstick.y),
            _pad: 0,
        }
    }

    /// Decode a wire input back into the form the simulation reads.
    pub fn decode(&self) -> PlayerInput {
        let d = |v: i8| -> f32 { v as f32 / 127.0 };
        PlayerInput {
            stick: crate::sim::math::Vec2::new(d(self.stick_x), d(self.stick_y)),
            cstick: crate::sim::math::Vec2::new(d(self.cstick_x), d(self.cstick_y)),
            buttons: self.buttons,
        }
    }
}

// Safe, manual byte-cast support so the `netcode` feature can implement
// `bytemuck::Pod` without forcing the dependency on the pure simulation.
unsafe impl bytemuck_shim::AnyBitPattern for NetInput {}
unsafe impl bytemuck_shim::NoUninit for NetInput {}

/// A minimal local shim describing the two marker traits we need, so the core
/// simulation stays dependency-free. When the `netcode` feature is on we also
/// assert `NetInput: bytemuck::Pod` in that module.
pub mod bytemuck_shim {
    /// Marker: every bit pattern of the correct size is a valid value.
    ///
    /// # Safety
    /// Implementor must have no padding with uninitialised bytes that could be read.
    pub unsafe trait AnyBitPattern: Copy {}
    /// Marker: the type has no uninitialised (padding) bytes.
    ///
    /// # Safety
    /// Implementor must be `#[repr(C)]`/packed with all fields initialised.
    pub unsafe trait NoUninit: Copy {}
}
