//! The fighter: its per-frame state machine, movement and defensive options.
//!
//! Combat resolution (who hits whom) lives in `state.rs`, which owns all the
//! fighters. This module handles everything a fighter does on its own given its
//! input and the stage: walking, dashing, dash-dancing, jumping (short/full/
//! double), fast-falling, air-dodging (and therefore wavedashing), shielding,
//! rolling, spot-dodging, grabbing ledges, and moving through attack/landing lag.
//!
//! Integration model: each state handler only *sets* `self.vel` (or, for the
//! few fixed-arc moves like rolls, moves `self.pos` directly and sets `vel = 0`).
//! Exactly one `self.pos += self.vel` happens per tick, right before stage
//! collision. This keeps the physics easy to reason about and deterministic.

use super::attacks::{self, MoveId};
use super::constants as k;
use super::constants::Character;
use super::input::{buttons, PlayerInput};
use super::math::{approach, clampf, Vec2};
use super::stage::Stage;

/// Ledge option chosen when hanging.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LedgeKind {
    Getup,
    Jump,
    Roll,
    Attack,
}

/// The fighter's current action.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum State {
    Stand,
    Walk,
    Crouch,
    Dash,
    Run,
    JumpSquat,
    Air,
    LandLag { total: u32 },
    Waveland,
    Attack { id: MoveId, aerial: bool },
    Shield,
    ShieldStun { total: u32 },
    Roll { dir: f32 },
    Spotdodge,
    Airdodge,
    Grab,
    Hold,
    Grabbed,
    Throw { id: MoveId },
    LedgeGrab,
    LedgeAction { kind: LedgeKind },
    Hitstun { tumble: bool },
    Knockdown,
    Dead,
}

/// One fighter's full mutable state. Part of the saved game state, so `Clone`
/// with no external handles.
#[derive(Clone, Debug)]
pub struct Fighter {
    pub character: &'static Character,
    pub port: usize,

    pub pos: Vec2,
    pub vel: Vec2,
    pub facing: f32, // +1 right, -1 left

    pub grounded: bool,
    pub support: Option<usize>,

    pub state: State,
    pub state_frame: u32,

    pub jumps_left: u8,
    pub fastfalling: bool,
    pub jump_held_at_squat_start: bool,

    pub percent: f32,
    pub stocks: i32,

    pub prev_buttons: u16,
    pub prev_cstick_len: f32,

    pub shield_health: f32,
    pub intangible: u32,
    pub hitlag: u32,
    pub hitstun_timer: u32,
    pub tech_lockout: u32,

    pub airdodge_dir: Vec2,
    pub lcancel_armed: bool,
    pub coyote: u32,

    pub ledge: Option<usize>,
    pub ledge_regrab_cd: u32,

    pub grabbing: Option<usize>,
    pub grabbed_by: Option<usize>,
    pub grab_timer: u32,

    pub already_hit: bool,
    pub respawn_timer: u32,

    pub kb_vel: Vec2,
    pub anim_flash: u32,
}

/// Result of a fighter's tick that the owner must act on.
#[derive(Clone, Copy, Debug, Default)]
pub struct TickOut {
    pub spawn_projectile: bool,
}

const DEADZONE: f32 = 0.30;
const HARD: f32 = 0.65;

impl Fighter {
    pub fn new(character: &'static Character, port: usize, spawn: Vec2) -> Self {
        Fighter {
            character,
            port,
            pos: spawn,
            vel: Vec2::ZERO,
            facing: if spawn.x <= 0.0 { 1.0 } else { -1.0 },
            grounded: false,
            support: None,
            state: State::Air,
            state_frame: 0,
            jumps_left: character.air_jumps,
            fastfalling: false,
            jump_held_at_squat_start: false,
            percent: 0.0,
            stocks: 4,
            prev_buttons: 0,
            prev_cstick_len: 0.0,
            shield_health: k::SHIELD_MAX,
            intangible: 0,
            hitlag: 0,
            hitstun_timer: 0,
            tech_lockout: 0,
            airdodge_dir: Vec2::ZERO,
            lcancel_armed: false,
            coyote: 0,
            ledge: None,
            ledge_regrab_cd: 0,
            grabbing: None,
            grabbed_by: None,
            grab_timer: 0,
            already_hit: false,
            respawn_timer: 0,
            kb_vel: Vec2::ZERO,
            anim_flash: 0,
        }
    }

    #[inline]
    fn set_state(&mut self, s: State) {
        self.state = s;
        self.state_frame = 0;
        self.already_hit = false;
    }

    #[inline]
    pub fn body_center(&self) -> Vec2 {
        Vec2::new(self.pos.x, self.pos.y + self.character.height * 0.5)
    }
    #[inline]
    pub fn hurt_radius(&self) -> f32 {
        self.character.height * 0.5
    }
    #[inline]
    pub fn is_intangible(&self) -> bool {
        self.intangible > 0 || matches!(self.state, State::Dead)
    }
    #[inline]
    pub fn can_be_hit(&self) -> bool {
        !self.is_intangible() && !matches!(self.state, State::Dead | State::Grabbed)
    }
    #[inline]
    pub fn is_shielding(&self) -> bool {
        matches!(self.state, State::Shield)
    }

    /// Current active hitbox in world space, if any.
    pub fn active_hitbox(&self) -> Option<(attacks::Hitbox, Vec2)> {
        let id = match self.state {
            State::Attack { id, .. } => id,
            _ => return None,
        };
        if self.already_hit {
            return None;
        }
        let md = attacks::data(id);
        if !md.is_active(self.state_frame) {
            return None;
        }
        let hb = md.hitbox;
        let world = Vec2::new(
            self.pos.x + self.facing * hb.offset.x,
            self.pos.y + hb.offset.y + self.character.height * 0.5,
        );
        Some((hb, world))
    }

    #[inline]
    fn pressed(&self, cur: u16, mask: u16) -> bool {
        (cur & mask != 0) && (self.prev_buttons & mask == 0)
    }
    #[inline]
    fn cstick_flick(&self, cst_len: f32) -> bool {
        cst_len > HARD && self.prev_cstick_len <= HARD
    }

    // ============================ main tick ============================
    pub fn tick(&mut self, input: &PlayerInput, stage: &Stage) -> TickOut {
        let mut out = TickOut::default();

        self.intangible = self.intangible.saturating_sub(1);
        self.tech_lockout = self.tech_lockout.saturating_sub(1);
        self.ledge_regrab_cd = self.ledge_regrab_cd.saturating_sub(1);
        self.coyote = self.coyote.saturating_sub(1);
        if self.anim_flash > 0 {
            self.anim_flash -= 1;
        }

        if self.hitlag > 0 {
            self.hitlag -= 1;
            self.remember_inputs(input);
            return out;
        }

        if matches!(self.state, State::Dead) {
            if self.respawn_timer > 0 {
                self.respawn_timer -= 1;
                if self.respawn_timer == 0 {
                    self.respawn(stage);
                }
            }
            self.remember_inputs(input);
            return out;
        }

        // ---- L-cancel: pressing shield during an airborne aerial arms it, so
        //      that the aerial's landing lag is halved on touchdown. ----
        if self.pressed(input.buttons, buttons::SHIELD)
            && matches!(self.state, State::Attack { aerial: true, .. })
            && !self.grounded
        {
            self.lcancel_armed = true;
        }

        // ---- state intent + timers (no integration yet) ----
        match self.state {
            State::LedgeGrab => self.tick_ledge(input),
            State::LedgeAction { kind } => self.tick_ledge_action(kind),
            State::Hitstun { tumble } => {
                self.tick_hitstun(input, stage, tumble);
                self.remember_inputs(input);
                self.state_frame = self.state_frame.saturating_add(1);
                return out; // hitstun integrates itself
            }
            State::Knockdown => self.tick_knockdown(input),
            State::Grab => self.tick_grab(),
            State::Hold => self.tick_hold(input),
            State::Grabbed => {}
            State::Throw { id } => self.tick_throw(id),
            State::Shield => self.tick_shield(input),
            State::ShieldStun { total } if self.state_frame >= total => {
                self.set_state(State::Shield);
            }
            State::Roll { dir } => self.tick_roll(dir),
            State::Spotdodge => self.tick_spotdodge(),
            State::Airdodge => self.tick_airdodge(),
            State::LandLag { total } if self.state_frame >= total => {
                self.set_state(if self.grounded {
                    State::Stand
                } else {
                    State::Air
                });
            }
            State::Waveland if self.state_frame >= k::WAVEDASH_LANDLAG => {
                self.set_state(State::Stand);
            }
            State::JumpSquat => self.tick_jumpsquat(input),
            State::Attack { id, .. } => {
                let md = attacks::data(id);
                if self.state_frame >= md.total() {
                    self.set_state(if self.grounded {
                        State::Stand
                    } else {
                        State::Air
                    });
                }
            }
            _ => {}
        }

        // ---- free-state intent ----
        if self.is_actionable() && self.handle_free_intent(input) {
            out.spawn_projectile = true;
        }

        // ---- velocity from physics ----
        if self.grounded {
            self.ground_control(input);
        } else {
            self.air_control(input);
        }

        // ---- single integration ----
        if !matches!(
            self.state,
            State::Roll { .. } | State::LedgeGrab | State::Grabbed
        ) {
            self.pos += self.vel;
        }

        // ---- stage collision ----
        self.collide_stage(stage, input);

        self.remember_inputs(input);
        self.state_frame = self.state_frame.saturating_add(1);
        out
    }

    #[inline]
    fn remember_inputs(&mut self, input: &PlayerInput) {
        self.prev_buttons = input.buttons;
        self.prev_cstick_len = input.cstick.length();
    }

    fn respawn(&mut self, stage: &Stage) {
        let s = stage.spawns[self.port % stage.spawns.len()];
        self.pos = Vec2::new(s.x, s.y + 60.0);
        self.vel = Vec2::ZERO;
        self.percent = 0.0;
        self.grounded = false;
        self.jumps_left = self.character.air_jumps;
        self.intangible = 120;
        self.fastfalling = false;
        self.kb_vel = Vec2::ZERO;
        self.facing = if s.x <= 0.0 { 1.0 } else { -1.0 };
        self.set_state(State::Air);
    }

    fn is_actionable(&self) -> bool {
        matches!(
            self.state,
            State::Stand | State::Walk | State::Crouch | State::Dash | State::Run | State::Air
        )
    }

    // ---------------------- free intent ----------------------
    /// Returns true if a projectile should be spawned this tick.
    fn handle_free_intent(&mut self, input: &PlayerInput) -> bool {
        let cur = input.buttons;
        let sx = input.stick.x;
        let sy = input.stick.y;
        let ch = self.character;

        // Jump.
        if self.pressed(cur, buttons::JUMP) {
            if self.grounded || self.coyote > 0 {
                self.jump_held_at_squat_start = true;
                self.grounded_for_squat();
                self.set_state(State::JumpSquat);
                return false;
            } else if self.jumps_left > 0 {
                self.jumps_left -= 1;
                self.vel.y = ch.doublejump_v;
                if sx.abs() > DEADZONE {
                    self.vel.x = clampf(sx, -1.0, 1.0) * ch.air_max;
                }
                self.fastfalling = false;
                self.set_state(State::Air);
                return false;
            }
        }

        // Shield / air-dodge / roll / spot-dodge.
        if cur & buttons::SHIELD != 0 {
            if self.grounded {
                if self.pressed(cur, buttons::SHIELD) && sx.abs() > HARD {
                    self.set_state(State::Roll { dir: sx.signum() });
                    return false;
                } else if self.pressed(cur, buttons::SHIELD) && sy < -HARD {
                    self.set_state(State::Spotdodge);
                    return false;
                } else {
                    self.set_state(State::Shield);
                    return false;
                }
            } else if self.pressed(cur, buttons::SHIELD) {
                let dir = if input.stick.length() > DEADZONE {
                    input.stick.normalized_or_zero()
                } else {
                    Vec2::ZERO
                };
                self.airdodge_dir = dir;
                self.vel = dir * ch.airdodge_speed;
                self.set_state(State::Airdodge);
                return false;
            }
        }

        // Grab (grounded).
        if self.pressed(cur, buttons::GRAB) && self.grounded {
            self.set_state(State::Grab);
            return false;
        }

        // Specials.
        if self.pressed(cur, buttons::SPECIAL) {
            let id = if sy > HARD {
                MoveId::SpecialUp
            } else if sy < -HARD {
                MoveId::SpecialDown
            } else if sx.abs() > HARD {
                self.facing = sx.signum();
                MoveId::SpecialSide
            } else {
                MoveId::SpecialN
            };
            match id {
                MoveId::SpecialUp => {
                    self.vel.y = ch.fullhop_v * 1.15;
                    self.vel.x += self.facing * 0.8;
                    self.fastfalling = false;
                    self.grounded = false;
                    self.support = None;
                }
                MoveId::SpecialSide => self.vel.x = self.facing * ch.dash_max * 1.4,
                _ => {}
            }
            let spawn = id == MoveId::SpecialN;
            self.set_state(State::Attack {
                id,
                aerial: !self.grounded,
            });
            return spawn;
        }

        // Attacks.
        let cst_len = input.cstick.length();
        if self.pressed(cur, buttons::ATTACK) || self.cstick_flick(cst_len) {
            let smash = self.cstick_flick(cst_len);
            if self.grounded {
                let id = self.pick_ground_attack(input, smash);
                self.vel.x = 0.0;
                self.set_state(State::Attack { id, aerial: false });
            } else {
                let id = self.pick_aerial(input, smash);
                self.set_state(State::Attack { id, aerial: true });
            }
            return false;
        }

        false
    }

    fn grounded_for_squat(&mut self) {
        // Ensure jumpsquat is treated as grounded even during coyote time.
        self.grounded = true;
    }

    fn pick_ground_attack(&mut self, input: &PlayerInput, smash: bool) -> MoveId {
        let sx = input.stick.x;
        let sy = input.stick.y;
        let dashing = matches!(self.state, State::Dash | State::Run) && sx.abs() > DEADZONE;
        if dashing {
            return MoveId::DashAttack;
        }
        if smash {
            let c = input.cstick;
            if c.y.abs() > c.x.abs() {
                return if c.y > 0.0 {
                    MoveId::Usmash
                } else {
                    MoveId::Dsmash
                };
            }
            self.facing = c.x.signum();
            return MoveId::Fsmash;
        }
        if sy > HARD {
            MoveId::Utilt
        } else if sy < -HARD {
            MoveId::Dtilt
        } else if sx.abs() > HARD {
            self.facing = sx.signum();
            MoveId::Ftilt
        } else {
            MoveId::Jab
        }
    }

    fn pick_aerial(&self, input: &PlayerInput, smash: bool) -> MoveId {
        let (ax, ay) = if smash {
            (input.cstick.x, input.cstick.y)
        } else {
            (input.stick.x, input.stick.y)
        };
        if ay > HARD && ay.abs() > ax.abs() {
            MoveId::Uair
        } else if ay < -HARD && ay.abs() > ax.abs() {
            MoveId::Dair
        } else if ax.abs() > DEADZONE {
            if ax.signum() == self.facing {
                MoveId::Fair
            } else {
                MoveId::Bair
            }
        } else {
            MoveId::Nair
        }
    }

    // ---------------------- physics ----------------------
    fn ground_control(&mut self, input: &PlayerInput) {
        let ch = self.character;
        let sx = input.stick.x;
        self.vel.y = 0.0;

        let controllable = matches!(
            self.state,
            State::Stand | State::Walk | State::Dash | State::Run | State::Crouch
        );

        if !controllable {
            if matches!(self.state, State::Waveland) {
                self.vel.x = approach(self.vel.x, 0.0, ch.traction);
            } else {
                self.vel.x = approach(self.vel.x, 0.0, ch.ground_friction * 0.6);
            }
            return;
        }

        // Crouch.
        if input.stick.y < -HARD {
            self.set_state(State::Crouch);
            self.vel.x = approach(self.vel.x, 0.0, ch.ground_friction);
            return;
        }
        if matches!(self.state, State::Crouch) && input.stick.y >= -DEADZONE {
            self.set_state(State::Stand);
        }

        if sx.abs() > DEADZONE {
            let want = sx.signum();
            match self.state {
                State::Stand | State::Walk | State::Crouch => {
                    self.facing = want;
                    if sx.abs() > HARD {
                        self.set_state(State::Dash);
                        self.vel.x =
                            approach(self.vel.x, want * ch.dash_max, ch.ground_accel * 2.0);
                    } else {
                        self.set_state(State::Walk);
                        self.vel.x = approach(self.vel.x, want * ch.walk_max, ch.ground_accel);
                    }
                }
                State::Dash => {
                    if want != self.facing && self.state_frame < k::DASH_DANCE_WINDOW {
                        self.facing = want;
                        self.set_state(State::Dash);
                    } else if self.state_frame >= k::DASH_DANCE_WINDOW {
                        self.set_state(State::Run);
                    }
                    self.vel.x =
                        approach(self.vel.x, self.facing * ch.dash_max, ch.ground_accel * 2.0);
                }
                State::Run => {
                    if want != self.facing {
                        self.facing = want;
                        self.set_state(State::Dash);
                    }
                    self.vel.x = approach(self.vel.x, self.facing * ch.run_max, ch.ground_accel);
                }
                _ => {}
            }
        } else {
            self.vel.x = approach(self.vel.x, 0.0, ch.ground_friction);
            if matches!(self.state, State::Walk | State::Dash | State::Run) {
                self.set_state(State::Stand);
            }
        }
    }

    fn air_control(&mut self, input: &PlayerInput) {
        let ch = self.character;
        let sx = input.stick.x;
        let sy = input.stick.y;

        if matches!(self.state, State::Airdodge) {
            return; // airdodge sets its own velocity
        }

        if sx.abs() > DEADZONE {
            self.vel.x = approach(self.vel.x, sx.signum() * ch.air_max, ch.air_accel);
        } else {
            self.vel.x = approach(self.vel.x, 0.0, ch.air_friction);
        }

        if !self.fastfalling && self.vel.y < 0.05 && sy < -HARD {
            self.fastfalling = true;
            self.vel.y = -ch.fastfall;
        }

        let terminal = if self.fastfalling {
            ch.fastfall
        } else {
            ch.max_fall
        };
        self.vel.y -= ch.gravity;
        if self.vel.y < -terminal {
            self.vel.y = -terminal;
        }
    }

    fn tick_jumpsquat(&mut self, input: &PlayerInput) {
        let ch = self.character;
        if self.state_frame + 1 >= ch.jumpsquat as u32 {
            let short = input.buttons & buttons::JUMP == 0;
            let jv = if short { ch.shorthop_v } else { ch.fullhop_v };
            self.grounded = false;
            self.support = None;
            self.vel.y = jv;
            if input.stick.x.abs() > DEADZONE {
                self.vel.x += clampf(input.stick.x, -1.0, 1.0) * ch.air_accel * 3.0;
            }
            self.fastfalling = false;
            self.set_state(State::Air);
        }
    }

    fn tick_airdodge(&mut self) {
        if self.state_frame == k::AIRDODGE_INTANGIBLE.0 {
            self.intangible = k::AIRDODGE_INTANGIBLE.1 - k::AIRDODGE_INTANGIBLE.0 + 1;
        }
        let t = self.state_frame as f32 / k::AIRDODGE_DURATION as f32;
        let damp = clampf(1.0 - t, 0.0, 1.0);
        self.vel = self.airdodge_dir * self.character.airdodge_speed * damp;
        self.vel.y -= self.character.gravity * 0.6 * t;
        if self.state_frame >= k::AIRDODGE_DURATION {
            self.set_state(State::Air);
        }
    }

    fn tick_spotdodge(&mut self) {
        if self.state_frame == k::SPOTDODGE_INTANGIBLE.0 {
            self.intangible = k::SPOTDODGE_INTANGIBLE.1 - k::SPOTDODGE_INTANGIBLE.0 + 1;
        }
        self.vel = Vec2::ZERO;
        if self.state_frame >= k::SPOTDODGE_DURATION {
            self.set_state(State::Crouch);
        }
    }

    fn tick_roll(&mut self, dir: f32) {
        if self.state_frame == k::ROLL_INTANGIBLE.0 {
            self.intangible = k::ROLL_INTANGIBLE.1 - k::ROLL_INTANGIBLE.0 + 1;
        }
        let step = k::ROLL_DISTANCE / k::ROLL_DURATION as f32;
        self.pos.x += dir * step;
        self.vel = Vec2::ZERO;
        if self.state_frame >= k::ROLL_DURATION {
            self.set_state(State::Stand);
        }
    }

    fn tick_shield(&mut self, input: &PlayerInput) {
        let cur = input.buttons;
        let sx = input.stick.x;
        let sy = input.stick.y;

        // Shield shrinks while held, and breaks if depleted.
        self.shield_health -= k::SHIELD_DECAY;
        if self.shield_health <= 0.0 {
            self.shield_health = 0.0;
            // shield break -> long stun
            self.set_state(State::ShieldStun { total: 120 });
            return;
        }

        // Out-of-shield options.
        if self.pressed(cur, buttons::JUMP) {
            self.set_state(State::JumpSquat);
            self.jump_held_at_squat_start = true;
            return;
        }
        if self.pressed(cur, buttons::GRAB) || self.pressed(cur, buttons::ATTACK) {
            self.set_state(State::Grab);
            return;
        }
        if self.pressed(cur, buttons::SPECIAL) {
            self.set_state(State::Attack {
                id: MoveId::SpecialN,
                aerial: false,
            });
            return;
        }
        if sx.abs() > HARD {
            self.set_state(State::Roll { dir: sx.signum() });
            return;
        }
        if sy < -HARD {
            self.set_state(State::Spotdodge);
            return;
        }
        if cur & buttons::SHIELD == 0 {
            self.set_state(State::Stand);
        }
    }

    // ---------------------- stage collision ----------------------
    fn collide_stage(&mut self, stage: &Stage, input: &PlayerInput) {
        let ch = self.character;
        let was_grounded = self.grounded;

        // Ledge grab.
        if !self.grounded
            && self.vel.y < 0.1
            && self.ledge_regrab_cd == 0
            && matches!(
                self.state,
                State::Air | State::Attack { aerial: true, .. } | State::Airdodge
            )
        {
            for (i, l) in stage.ledges.iter().enumerate() {
                let dx = self.pos.x - l.pos.x;
                let outer = dx * l.side > 0.0 && dx.abs() < k::LEDGE_HOG_BOX + ch.half_width;
                let dy = (self.pos.y - l.pos.y).abs();
                if outer && dy < k::LEDGE_HOG_BOX {
                    self.grab_ledge(i, *l);
                    return;
                }
            }
        }

        // Landing on platforms.
        self.grounded = false;
        self.support = None;
        let prev_feet = self.pos.y - self.vel.y;
        for (i, p) in stage.platforms.iter().enumerate() {
            let within =
                self.pos.x >= p.left - ch.half_width && self.pos.x <= p.right + ch.half_width;
            if !within {
                continue;
            }
            let feet = self.pos.y;
            let crossed = feet <= p.y && prev_feet >= p.y - 0.01;
            let falling = self.vel.y <= 0.0;

            let drop_through = !p.solid
                && input.stick.y < -HARD
                && (self.prev_buttons & buttons::JUMP == 0)
                && (input.buttons & buttons::JUMP != 0);

            if falling && crossed && !drop_through {
                self.pos.y = p.y;
                self.vel.y = 0.0;
                self.grounded = true;
                self.support = Some(i);
                break;
            }
        }

        // Keep the fighter on top of a solid platform horizontally (walls at edges
        // are intentionally soft so recovery works like the reference game).
        if self.grounded {
            if let Some(idx) = self.support {
                let p = stage.platforms[idx];
                if p.solid {
                    // fall off if walking past the edge
                    if self.pos.x < p.left - ch.half_width || self.pos.x > p.right + ch.half_width {
                        self.grounded = false;
                        self.support = None;
                    }
                }
            }
        }

        if self.grounded && !was_grounded {
            self.on_land(input);
        } else if !self.grounded && was_grounded {
            self.coyote = k::COYOTE_FRAMES;
            if matches!(
                self.state,
                State::Stand | State::Walk | State::Dash | State::Run | State::Crouch
            ) {
                self.set_state(State::Air);
            }
        }
    }

    fn on_land(&mut self, _input: &PlayerInput) {
        self.fastfalling = false;
        self.jumps_left = self.character.air_jumps;

        match self.state {
            State::Attack { id, aerial: true } => {
                let md = attacks::data(id);
                let mut lag = md.landing_lag;
                if self.lcancel_armed {
                    lag = lag.div_ceil(2);
                }
                self.lcancel_armed = false;
                self.vel.x *= 0.5;
                self.set_state(State::LandLag { total: lag.max(1) });
            }
            State::Airdodge => {
                let hspeed = self.vel.x;
                self.vel = Vec2::new(hspeed, 0.0);
                self.set_state(State::Waveland);
            }
            State::Air => {
                self.vel.x *= 0.85;
                self.set_state(State::LandLag { total: 2 });
            }
            _ => {}
        }
    }

    fn grab_ledge(&mut self, idx: usize, l: super::stage::Ledge) {
        self.ledge = Some(idx);
        self.pos = Vec2::new(
            l.pos.x - l.side * self.character.half_width,
            l.pos.y - self.character.height,
        );
        self.vel = Vec2::ZERO;
        self.facing = l.side;
        self.grounded = false;
        self.jumps_left = self.character.air_jumps;
        self.fastfalling = false;
        self.intangible = k::LEDGE_INTANGIBLE;
        self.set_state(State::LedgeGrab);
    }

    fn tick_ledge(&mut self, input: &PlayerInput) {
        let cur = input.buttons;
        let sx = input.stick.x;
        let sy = input.stick.y;
        let out_dir = -self.facing;
        if self.state_frame >= 2 {
            if self.pressed(cur, buttons::JUMP) || sy > HARD {
                self.set_state(State::LedgeAction {
                    kind: LedgeKind::Jump,
                });
            } else if self.pressed(cur, buttons::ATTACK) {
                self.set_state(State::LedgeAction {
                    kind: LedgeKind::Attack,
                });
            } else if sx.signum() == out_dir && sx.abs() > HARD {
                self.set_state(State::LedgeAction {
                    kind: LedgeKind::Roll,
                });
            } else if sx.signum() == self.facing && sx.abs() > HARD {
                self.set_state(State::LedgeAction {
                    kind: LedgeKind::Getup,
                });
            } else if sy < -HARD {
                self.ledge = None;
                self.ledge_regrab_cd = 22;
                self.intangible = 0;
                self.set_state(State::Air);
            }
        }
    }

    fn tick_ledge_action(&mut self, kind: LedgeKind) {
        let ch = self.character;
        match kind {
            LedgeKind::Jump => {
                self.ledge = None;
                self.grounded = false;
                self.vel.y = ch.fullhop_v;
                self.vel.x = self.facing * 0.8;
                self.intangible = 10;
                self.set_state(State::Air);
            }
            LedgeKind::Getup => {
                self.ledge = None;
                self.pos.y += ch.height + 1.0;
                self.pos.x += self.facing * (ch.half_width + 4.0);
                self.vel = Vec2::ZERO;
                self.grounded = true;
                self.intangible = 6;
                self.set_state(State::LandLag { total: 6 });
            }
            LedgeKind::Roll => {
                self.ledge = None;
                self.pos.y += ch.height + 1.0;
                self.pos.x += self.facing * (ch.half_width + 20.0);
                self.grounded = true;
                self.intangible = k::ROLL_INTANGIBLE.1;
                self.set_state(State::LandLag { total: 10 });
            }
            LedgeKind::Attack => {
                self.ledge = None;
                self.pos.y += ch.height + 1.0;
                self.pos.x += self.facing * (ch.half_width + 4.0);
                self.grounded = true;
                self.set_state(State::Attack {
                    id: MoveId::Ftilt,
                    aerial: false,
                });
            }
        }
    }

    fn tick_hitstun(&mut self, input: &PlayerInput, stage: &Stage, tumble: bool) {
        let ch = self.character;
        self.kb_vel.x = approach(self.kb_vel.x, 0.0, k::KB_FRICTION);
        self.kb_vel.y -= ch.gravity;
        if self.kb_vel.y < -ch.max_fall {
            self.kb_vel.y = -ch.max_fall;
        }
        if self.state_frame > 2 && input.stick.x.abs() > DEADZONE {
            self.kb_vel.x += input.stick.x.signum() * ch.air_accel * 0.25;
        }
        self.vel = self.kb_vel;
        self.pos += self.vel;

        // Ground collision → tech or knockdown.
        let main = stage.main();
        let over = self.pos.x >= main.left && self.pos.x <= main.right;
        if self.vel.y < 0.0 && self.pos.y <= main.y && over {
            self.pos.y = main.y;
            let teching = tumble
                && self.tech_lockout == 0
                && (input.buttons & buttons::SHIELD != 0)
                && (self.prev_buttons & buttons::SHIELD == 0);
            self.grounded = true;
            self.vel = Vec2::ZERO;
            self.kb_vel = Vec2::ZERO;
            if teching {
                self.intangible = k::TECH_INTANGIBLE;
                self.set_state(State::LandLag { total: 8 });
            } else if tumble {
                self.tech_lockout = k::TECH_LOCKOUT;
                self.set_state(State::Knockdown);
            } else {
                self.set_state(State::LandLag { total: 4 });
            }
            return;
        }

        if self.hitstun_timer > 0 {
            self.hitstun_timer -= 1;
            if self.hitstun_timer == 0 {
                self.set_state(State::Air);
                self.vel = self.kb_vel;
            }
        } else {
            self.set_state(State::Air);
            self.vel = self.kb_vel;
        }
    }

    fn tick_knockdown(&mut self, input: &PlayerInput) {
        self.vel = Vec2::ZERO;
        if self.state_frame >= 18 {
            let sx = input.stick.x;
            if sx.abs() > HARD {
                self.pos.x += sx.signum() * k::ROLL_DISTANCE * 0.6;
            }
            self.intangible = 8;
            self.set_state(State::LandLag { total: 4 });
        }
    }

    fn tick_grab(&mut self) {
        if self.state_frame >= 30 {
            self.set_state(State::Stand);
        }
    }

    fn tick_hold(&mut self, input: &PlayerInput) {
        self.grab_timer = self.grab_timer.saturating_sub(1);
        let sx = input.stick.x;
        let sy = input.stick.y;
        let cur = input.buttons;
        let want_throw = self.pressed(cur, buttons::ATTACK) || input.cstick.length() > 0.5;
        if want_throw {
            let id = if sy > HARD {
                MoveId::ThrowU
            } else if sy < -HARD {
                MoveId::ThrowD
            } else if sx.signum() == self.facing && sx.abs() > DEADZONE {
                MoveId::ThrowF
            } else if sx.abs() > DEADZONE {
                MoveId::ThrowB
            } else {
                MoveId::ThrowF
            };
            self.set_state(State::Throw { id });
        } else if self.grab_timer == 0 {
            self.grabbing = None;
            self.set_state(State::Stand);
        }
    }

    fn tick_throw(&mut self, id: MoveId) {
        let md = attacks::data(id);
        if self.state_frame >= md.total() {
            self.grabbing = None;
            self.set_state(State::Stand);
        }
    }

    /// Enter hitstun with a launch velocity and duration.
    pub fn apply_launch(&mut self, kb_vel: Vec2, hitstun: u32, tumble: bool, hitlag: u32) {
        self.kb_vel = kb_vel;
        self.vel = kb_vel;
        self.grounded = false;
        self.support = None;
        self.fastfalling = false;
        self.hitstun_timer = hitstun;
        self.hitlag = hitlag;
        self.intangible = 0;
        self.ledge = None;
        self.grabbing = None;
        self.grabbed_by = None;
        self.anim_flash = 6;
        self.set_state(State::Hitstun { tumble });
    }

    /// Begin holding an opponent (successful grab).
    pub fn catch(&mut self, victim: usize) {
        self.grabbing = Some(victim);
        self.grab_timer = 90;
        self.set_state(State::Hold);
    }

    /// Become grabbed by a holder.
    pub fn set_grabbed(&mut self, holder: usize) {
        self.grabbed_by = Some(holder);
        self.vel = Vec2::ZERO;
        self.set_state(State::Grabbed);
    }

    /// Take a hit while shielding: reduce shield, apply shield stun/pushback.
    pub fn shield_hit(&mut self, damage: f32, push: f32) {
        self.shield_health -= damage * k::SHIELD_DAMAGE_MULT;
        let stun = (damage * k::SHIELDSTUN_MULT) as u32 + 1;
        self.vel.x = -self.facing * push;
        if self.shield_health <= 0.0 {
            self.shield_health = 0.0;
            self.set_state(State::ShieldStun { total: 120 });
        } else {
            self.set_state(State::ShieldStun { total: stun });
        }
    }
}
