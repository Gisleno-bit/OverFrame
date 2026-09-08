//! Input hub for the window: keyboard maps + physical gamepads via `gilrs`,
//! merged per player slot so either device drives the fighter.
//!
//! Slot 1 = keyboard set 1 ⊕ first connected pad; slot 2 = keyboard set 2 ⊕
//! second pad. Pads are ordered by connection order, which is what players
//! expect ("the first controller I plugged in is P1").

use macroquad::prelude::{is_key_down, KeyCode};

use crate::gamepad::{self, PadBindings, PadButton, PadSnapshot};
use crate::sim::input::{buttons, PlayerInput};
use crate::sim::math::Vec2;

/// A player's keyboard binding.
pub struct KeyMap {
    pub up: KeyCode,
    pub down: KeyCode,
    pub left: KeyCode,
    pub right: KeyCode,
    pub cup: KeyCode,
    pub cdown: KeyCode,
    pub cleft: KeyCode,
    pub cright: KeyCode,
    pub jump: KeyCode,
    pub attack: KeyCode,
    pub special: KeyCode,
    pub shield: KeyCode,
    pub grab: KeyCode,
}

pub fn player_one_keys() -> KeyMap {
    KeyMap {
        up: KeyCode::W,
        down: KeyCode::S,
        left: KeyCode::A,
        right: KeyCode::D,
        cup: KeyCode::R,
        cdown: KeyCode::F,
        cleft: KeyCode::Q,
        cright: KeyCode::E,
        jump: KeyCode::Space,
        attack: KeyCode::C,
        special: KeyCode::V,
        shield: KeyCode::LeftShift,
        grab: KeyCode::X,
    }
}

pub fn player_two_keys() -> KeyMap {
    KeyMap {
        up: KeyCode::Up,
        down: KeyCode::Down,
        left: KeyCode::Left,
        right: KeyCode::Right,
        cup: KeyCode::I,
        cdown: KeyCode::K,
        cleft: KeyCode::J,
        cright: KeyCode::L,
        jump: KeyCode::RightShift,
        attack: KeyCode::Period,
        special: KeyCode::Slash,
        shield: KeyCode::RightControl,
        grab: KeyCode::Comma,
    }
}

pub fn read_keyboard(k: &KeyMap) -> PlayerInput {
    let axis = |neg: KeyCode, pos: KeyCode| -> f32 {
        (is_key_down(pos) as i32 as f32) - (is_key_down(neg) as i32 as f32)
    };
    let stick = Vec2::new(axis(k.left, k.right), axis(k.down, k.up));
    let cstick = Vec2::new(axis(k.cleft, k.cright), axis(k.cdown, k.cup));
    let mut b = 0u16;
    if is_key_down(k.jump) {
        b |= buttons::JUMP;
    }
    if is_key_down(k.attack) {
        b |= buttons::ATTACK;
    }
    if is_key_down(k.special) {
        b |= buttons::SPECIAL;
    }
    if is_key_down(k.shield) {
        b |= buttons::SHIELD;
    }
    if is_key_down(k.grab) {
        b |= buttons::GRAB;
    }
    PlayerInput {
        stick,
        cstick,
        buttons: b,
    }
}

/// Translate a gilrs button to our device-agnostic enum.
fn pad_button(b: gilrs::Button) -> Option<PadButton> {
    use gilrs::Button as B;
    Some(match b {
        B::South => PadButton::South,
        B::East => PadButton::East,
        B::North => PadButton::North,
        B::West => PadButton::West,
        // gilrs naming: LeftTrigger = bumper (LB), LeftTrigger2 = analog trigger (LT).
        B::LeftTrigger => PadButton::LeftBumper,
        B::RightTrigger => PadButton::RightBumper,
        B::LeftTrigger2 => PadButton::LeftTrigger,
        B::RightTrigger2 => PadButton::RightTrigger,
        B::Select => PadButton::Select,
        B::Start => PadButton::Start,
        B::LeftThumb => PadButton::LeftThumb,
        B::RightThumb => PadButton::RightThumb,
        B::DPadUp => PadButton::DPadUp,
        B::DPadDown => PadButton::DPadDown,
        B::DPadLeft => PadButton::DPadLeft,
        B::DPadRight => PadButton::DPadRight,
        _ => return None,
    })
}

/// Everything the window reads input from.
pub struct InputHub {
    keys: [KeyMap; 2],
    gilrs: Option<gilrs::Gilrs>,
    /// Connected pads in connection order.
    order: Vec<gilrs::GamepadId>,
    /// Buttons pressed since the last `poll`, per pad slot (for rebinding).
    just_pressed: [Option<PadButton>; 2],
    /// Any button pressed on any pad this frame (menu navigation).
    any_pressed: Option<PadButton>,
    /// Last-known gamepad names for the options screen.
    pub names: Vec<String>,
    pub init_error: Option<String>,
    /// Force-feedback effect per pad slot (built lazily; None if unsupported).
    rumble_fx: [Option<gilrs::ff::Effect>; 2],
    rumble_tried: [bool; 2],
    pub rumble_enabled: bool,
}

impl InputHub {
    pub fn new() -> Self {
        let (gilrs, init_error) = match gilrs::Gilrs::new() {
            Ok(g) => (Some(g), None),
            Err(e) => (None, Some(format!("{e}"))),
        };
        let mut hub = InputHub {
            keys: [player_one_keys(), player_two_keys()],
            gilrs,
            order: Vec::new(),
            just_pressed: [None, None],
            any_pressed: None,
            names: Vec::new(),
            init_error,
            rumble_fx: [None, None],
            rumble_tried: [false, false],
            rumble_enabled: true,
        };
        hub.refresh_order();
        hub
    }

    /// Short controller vibration on the pad driving `slot` (0..2);
    /// `strength` 0..1. Silently does nothing without a pad or force feedback.
    pub fn rumble(&mut self, slot: usize, strength: f32) {
        if !self.rumble_enabled || slot > 1 {
            return;
        }
        let Some(id) = self.order.get(slot).copied() else {
            return;
        };
        if self.rumble_fx[slot].is_none() && !self.rumble_tried[slot] {
            self.rumble_tried[slot] = true;
            if let Some(g) = self.gilrs.as_mut() {
                if g.gamepad(id).is_ff_supported() {
                    use gilrs::ff::{
                        BaseEffect, BaseEffectType, EffectBuilder, Repeat, Replay, Ticks,
                    };
                    let built = EffectBuilder::new()
                        .add_effect(BaseEffect {
                            kind: BaseEffectType::Strong { magnitude: 0xE000 },
                            scheduling: Replay {
                                after: Ticks::from_ms(0),
                                play_for: Ticks::from_ms(110),
                                with_delay: Ticks::from_ms(0),
                            },
                            envelope: Default::default(),
                        })
                        .add_effect(BaseEffect {
                            kind: BaseEffectType::Weak { magnitude: 0x9000 },
                            scheduling: Replay {
                                after: Ticks::from_ms(0),
                                play_for: Ticks::from_ms(70),
                                with_delay: Ticks::from_ms(0),
                            },
                            envelope: Default::default(),
                        })
                        .repeat(Repeat::For(Ticks::from_ms(110)))
                        .gamepads(&[id])
                        .finish(g);
                    self.rumble_fx[slot] = built.ok();
                }
            }
        }
        if let Some(fx) = &self.rumble_fx[slot] {
            let _ = fx.set_gain(strength.clamp(0.1, 1.0));
            let _ = fx.play();
        }
    }

    /// Forget built rumble effects (pads changed).
    fn reset_rumble(&mut self) {
        self.rumble_fx = [None, None];
        self.rumble_tried = [false, false];
    }

    fn refresh_order(&mut self) {
        let Some(g) = &self.gilrs else {
            return;
        };
        // Keep known pads that are still connected, append newly seen ones.
        let connected: Vec<gilrs::GamepadId> = g.gamepads().map(|(id, _)| id).collect();
        self.order.retain(|id| connected.contains(id));
        for id in connected {
            if !self.order.contains(&id) {
                self.order.push(id);
            }
        }
        self.names = self
            .order
            .iter()
            .map(|id| g.gamepad(*id).name().to_owned())
            .collect();
    }

    /// Drain gamepad events. Call once per rendered frame, before reading.
    pub fn poll(&mut self) {
        self.just_pressed = [None, None];
        self.any_pressed = None;
        let mut hot: Vec<(gilrs::GamepadId, PadButton)> = Vec::new();
        let mut topology_changed = false;
        if let Some(g) = self.gilrs.as_mut() {
            while let Some(ev) = g.next_event() {
                match ev.event {
                    gilrs::EventType::ButtonPressed(b, _) => {
                        if let Some(pb) = pad_button(b) {
                            hot.push((ev.id, pb));
                        }
                    }
                    gilrs::EventType::Connected | gilrs::EventType::Disconnected => {
                        topology_changed = true;
                    }
                    _ => {}
                }
            }
        }
        if topology_changed || self.order.is_empty() {
            self.refresh_order();
            if topology_changed {
                self.reset_rumble();
            }
        }
        for (id, pb) in hot {
            self.any_pressed = Some(pb);
            if let Some(slot) = self.order.iter().position(|x| *x == id) {
                if slot < 2 {
                    self.just_pressed[slot] = Some(pb);
                }
            }
        }
    }

    /// Number of connected gamepads.
    pub fn pad_count(&self) -> usize {
        self.order.len()
    }

    /// Snapshot of the pad in `slot` (0 or 1), if connected.
    pub fn snapshot(&self, slot: usize) -> Option<PadSnapshot> {
        let g = self.gilrs.as_ref()?;
        let id = *self.order.get(slot)?;
        let gp = g.connected_gamepad(id)?;
        use gilrs::{Axis, Button as B};
        let mut mask = 0u32;
        for b in [
            B::South,
            B::East,
            B::North,
            B::West,
            B::LeftTrigger,
            B::RightTrigger,
            B::LeftTrigger2,
            B::RightTrigger2,
            B::Select,
            B::Start,
            B::LeftThumb,
            B::RightThumb,
            B::DPadUp,
            B::DPadDown,
            B::DPadLeft,
            B::DPadRight,
        ] {
            if gp.is_pressed(b) {
                if let Some(pb) = pad_button(b) {
                    mask |= pb.mask();
                }
            }
        }
        let trig = |b: B| gp.button_data(b).map(|d| d.value()).unwrap_or(0.0);
        Some(PadSnapshot {
            left: Vec2::new(gp.value(Axis::LeftStickX), gp.value(Axis::LeftStickY)),
            right: Vec2::new(gp.value(Axis::RightStickX), gp.value(Axis::RightStickY)),
            left_trigger: trig(B::LeftTrigger2),
            right_trigger: trig(B::RightTrigger2),
            buttons: mask,
        })
    }

    /// Merged input for player `slot` (0 = P1, 1 = P2).
    pub fn player_input(&self, slot: usize, bindings: &PadBindings) -> PlayerInput {
        let kb = read_keyboard(&self.keys[slot.min(1)]);
        match self.snapshot(slot) {
            Some(snap) => gamepad::merge(kb, gamepad::map(&snap, bindings)),
            None => kb,
        }
    }

    /// A button freshly pressed on pad `slot` this frame (for rebinding).
    pub fn just_pressed(&self, slot: usize) -> Option<PadButton> {
        self.just_pressed.get(slot).copied().flatten()
    }

    /// A button freshly pressed on any pad this frame (menus).
    pub fn any_just_pressed(&self) -> Option<PadButton> {
        self.any_pressed
    }

    /// Left-stick direction of any pad as a menu nudge (-1, 0, +1 per axis),
    /// edge-detected by the caller.
    pub fn any_stick(&self) -> Vec2 {
        for slot in 0..2 {
            if let Some(s) = self.snapshot(slot) {
                if s.left.length() > 0.6 {
                    return s.left;
                }
            }
        }
        Vec2::ZERO
    }
}
