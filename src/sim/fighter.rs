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

/// How a knocked-down fighter gets back up.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GetupKind {
    Stand,
    Roll { dir: f32 },
    Attack,
}

impl State {
    /// A stable `(discriminant, payload)` pair for the rollback checksum.
    /// Two states that differ in any way must produce different codes, so a
    /// desync in a state *or its payload* is always detected.
    pub fn code(&self) -> (u32, u32) {
        match *self {
            State::Stand => (0, 0),
            State::Walk => (1, 0),
            State::Crouch => (2, 0),
            State::Dash => (3, 0),
            State::Run => (4, 0),
            State::RunTurn => (5, 0),
            State::JumpSquat => (6, 0),
            State::Air => (7, 0),
            State::LandLag { total } => (8, total),
            State::Waveland => (9, 0),
            State::Attack { id, aerial } => (10, id as u32 * 2 + u32::from(aerial)),
            State::Shield => (11, 0),
            State::ShieldStun { total } => (12, total),
            State::ShieldDrop => (13, 0),
            State::Roll { dir } => (14, dir.to_bits()),
            State::Spotdodge => (15, 0),
            State::Airdodge => (16, 0),
            State::Helpless => (17, 0),
            State::Grab => (18, 0),
            State::Hold => (19, 0),
            State::Grabbed => (20, 0),
            State::Throw { id } => (21, id as u32),
            State::Tech { dir } => (22, dir.to_bits()),
            State::Getup { kind } => (
                23,
                match kind {
                    GetupKind::Stand => 0,
                    GetupKind::Roll { dir } => dir.to_bits(),
                    GetupKind::Attack => 1,
                },
            ),
            State::Rebound { total } => (24, total),
            State::LedgeGrab => (25, 0),
            State::LedgeAction { kind } => (26, kind as u32),
            State::Hitstun { tumble } => (27, u32::from(tumble)),
            State::Knockdown => (28, 0),
            State::Dead => (29, 0),
        }
    }
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
    LandLag {
        total: u32,
    },
    Waveland,
    Attack {
        id: MoveId,
        aerial: bool,
    },
    Shield,
    ShieldStun {
        total: u32,
    },
    Roll {
        dir: f32,
    },
    Spotdodge,
    Airdodge,
    /// Falling with no actions until landing (after an air-dodge or an aerial
    /// up-special). Drift is allowed.
    Helpless,
    /// Lowering the shield: brief lag before acting again.
    ShieldDrop,
    Grab,
    Hold,
    Grabbed,
    Throw {
        id: MoveId,
    },
    /// Braking turn out of a full run (only a jump interrupts it).
    RunTurn,
    /// Teched a tumble landing: in place (`dir == 0`) or a roll.
    Tech {
        dir: f32,
    },
    /// Rising from a knockdown.
    Getup {
        kind: GetupKind,
    },
    /// Two grounded attacks clanked: recoil lag.
    Rebound {
        total: u32,
    },
    LedgeGrab,
    LedgeAction {
        kind: LedgeKind,
    },
    Hitstun {
        tumble: bool,
    },
    Knockdown,
    Dead,
}

/// One fighter's full mutable state. Part of the saved game state, so `Clone`
/// with no external handles.
#[derive(Clone, Debug)]
pub struct Fighter {
    pub character: &'static Character,
    pub port: usize,
    /// Colour palette index (0..PALETTES); visual only.
    pub palette: u8,

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
    /// The stick as it was on the previous tick (for flick detection after
    /// `prev_stick` has already been refreshed this tick).
    pub stick_last: Vec2,

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
    /// The current grab was started from a dash / run (slower, longer).
    pub grab_dash: bool,
    /// Frames until the next pummel is allowed.
    pub pummel_cd: u32,
    /// Frames left in the tech window opened by a shield press while
    /// tumbling (landing inside it techs).
    pub tech_armed: u32,
    /// The last hits that connected, newest first (stale-move negation).
    pub stale: [Option<MoveId>; super::constants::STALE_QUEUE],
    /// Match frame of the last hit taken (HUD percent pop).
    pub last_hit_frame: u64,
    /// The current hitstun came from a spike (meteor-cancellable).
    pub meteor: bool,
    /// Frames left in which an attack press still counts as a smash after
    /// the stick was flicked to hard.
    pub stick_flick: u32,
    /// Charge frames of the current smash (0 = uncharged).
    pub charge: u32,
    /// The current smash was started with the attack button (may charge).
    pub charge_armed: bool,
    /// Bit mask of ledges other fighters hold this tick (edgehog): cannot
    /// grab those.
    pub ledge_blocked: u8,

    pub already_hit: bool,
    pub respawn_timer: u32,

    pub kb_vel: Vec2,
    /// Separate fall velocity accumulated by gravity during knockback (the
    /// knockback vector itself decays uniformly; this is capped by fall speed).
    pub kb_fall: f32,
    /// Hitstun taken while staying on the ground (weak grounded hits): no
    /// gravity, slide with friction.
    pub ground_stun: bool,
    /// Stick as of the previous frame (for SDI pulse detection).
    pub prev_stick: Vec2,
    /// A launch waiting for the end of hitlag: (knockback, base angle in
    /// radians). Directional influence is read from the stick on the last
    /// freeze frame, so the victim can react during the freeze.
    pub pending_launch: Option<(f32, f32)>,
    /// Position at the start of this tick (hitboxes are swept between the two
    /// so fast movement can't tunnel through a hurtbox).
    pub prev_pos: Vec2,
    pub anim_flash: u32,
}

/// Result of a fighter's tick that the owner must act on.
#[derive(Clone, Copy, Debug, Default)]
pub struct TickOut {
    pub spawn_projectile: bool,
    /// Feedback events raised this tick (see [`ev`]), consumed by the match
    /// state to spawn effects / sounds or to act on the other fighter.
    pub events: u16,
}

/// Mark a swing on `out` and hand it back (early-return helper).
fn out_events_swing(out: &mut TickOut) -> TickOut {
    out.events |= ev::SWING;
    *out
}

/// Bit flags for [`TickOut::events`].
pub mod ev {
    pub const LAND: u16 = 1;
    pub const JUMP: u16 = 2;
    pub const SWING: u16 = 4;
    pub const DASH: u16 = 8;
    pub const TECH: u16 = 16;
    pub const WAVEDASH: u16 = 32;
    pub const DOUBLE_JUMP: u16 = 64;
    /// A grabbed victim made a fresh input (shortens the holder's grab).
    pub const MASH: u16 = 128;
    /// The holder pummelled (damage the victim).
    pub const PUMMEL: u16 = 256;
}

const DEADZONE: f32 = 0.30;
const HARD: f32 = 0.65;
/// Vertical tilt threshold for up / down tilts (between the deadzone and
/// the hard threshold, so a half-held stick still tilts).
const TILT_Y: f32 = 0.50;

impl Fighter {
    pub fn new(character: &'static Character, port: usize, spawn: Vec2) -> Self {
        Fighter {
            character,
            port,
            palette: 0,
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
            stick_last: Vec2::ZERO,
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
            grab_dash: false,
            pummel_cd: 0,
            tech_armed: 0,
            stale: [None; super::constants::STALE_QUEUE],
            last_hit_frame: 0,
            meteor: false,
            stick_flick: 0,
            charge: 0,
            charge_armed: false,
            ledge_blocked: 0,
            already_hit: false,
            respawn_timer: 0,
            kb_vel: Vec2::ZERO,
            kb_fall: 0.0,
            ground_stun: false,
            prev_stick: Vec2::ZERO,
            pending_launch: None,
            prev_pos: spawn,
            anim_flash: 0,
        }
    }

    #[inline]
    fn set_state(&mut self, s: State) {
        self.state = s;
        self.state_frame = 0;
        self.already_hit = false;
    }

    /// Force a state from the match (clank rebounds).
    pub fn set_state_pub(&mut self, s: State) {
        self.set_state(s);
        // A clank keeps the hitbox spent for the rest of the swing.
        if matches!(s, State::Rebound { .. }) {
            self.already_hit = true;
        }
    }

    #[inline]
    pub fn body_center(&self) -> Vec2 {
        Vec2::new(self.pos.x, self.pos.y + self.character.height * 0.5)
    }
    /// Hurtbox radius: the fighter is a vertical capsule of this radius (see
    /// [`Fighter::hurt_segment`]), a little wider than the drawn body.
    #[inline]
    pub fn hurt_radius(&self) -> f32 {
        self.character.half_width + 1.5
    }

    /// The hurt capsule's axis (feet → head, inset by the radius). Crouching
    /// and lying down shrink it.
    pub fn hurt_segment(&self) -> (Vec2, Vec2) {
        let r = self.hurt_radius();
        let h = match self.state {
            State::Crouch | State::Spotdodge | State::ShieldStun { .. } => {
                self.character.height * 0.62
            }
            State::Knockdown
            | State::Getup {
                kind: GetupKind::Stand,
            } => self.character.height * 0.4,
            State::Tech { .. } | State::Getup { .. } => self.character.height * 0.62,
            State::Shield | State::ShieldDrop => self.character.height * 0.85,
            _ => self.character.height,
        };
        let top = (h - r).max(r + 0.5);
        (
            Vec2::new(self.pos.x, self.pos.y + r),
            Vec2::new(self.pos.x, self.pos.y + top),
        )
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

    /// Current active hitbox in world space, if any — `None` once it has
    /// already connected this move (one hit per swing).
    pub fn active_hitbox(&self) -> Option<(attacks::Hitbox, Vec2)> {
        if self.already_hit {
            return None;
        }
        self.hitbox_geometry()
    }

    /// The hitbox the current state frame *defines*, whether or not it has
    /// already connected (tooling / overlays: the exporter reports geometry
    /// on the contact tick too).
    pub fn hitbox_geometry(&self) -> Option<(attacks::Hitbox, Vec2)> {
        let hb = match self.state {
            State::Attack { id, aerial } => {
                let md = attacks::data(self.character.id, id);
                let mut hb = md.hitbox_at(self.state_frame)?;
                if !aerial && attacks::is_smash(id) && self.charge > 0 {
                    hb.damage = (hb.damage * self.charge_multiplier()).round();
                }
                hb
            }
            State::Getup {
                kind: GetupKind::Attack,
            } => {
                // Weak two-sided sweep: in front first, then behind.
                let f = self.state_frame;
                let (front, back) = k::GETUP_ATTACK_HITS;
                let a = k::GETUP_ATTACK_ACTIVE;
                let side = if (front..front + a).contains(&f) {
                    1.0
                } else if (back..back + a).contains(&f) {
                    -1.0
                } else {
                    return None;
                };
                let mut hb = attacks::data(self.character.id, MoveId::Ftilt).hitbox;
                hb.offset = Vec2::new(hb.offset.x * side, -self.character.height * 0.25);
                hb.damage = k::GETUP_ATTACK_DAMAGE;
                hb.angle_deg = 361.0;
                hb.bkb = 30.0;
                hb.kbg = 50.0;
                hb
            }
            State::LedgeAction {
                kind: LedgeKind::Attack,
            } => {
                // The ledge attack strikes with the forward tilt's hitbox on
                // its own (ledge) timeline.
                let (_, _, hit) = self.ledge_option(LedgeKind::Attack);
                if !(hit..hit + k::LEDGE_ATTACK_ACTIVE).contains(&self.state_frame) {
                    return None;
                }
                let mut hb = attacks::data(self.character.id, MoveId::Ftilt).hitbox;
                hb.damage = k::LEDGE_ATTACK_DAMAGE[self.ledge_tired()];
                hb
            }
            _ => return None,
        };
        let world = Vec2::new(
            self.pos.x + self.facing * hb.offset.x,
            self.pos.y + hb.offset.y + self.character.height * 0.5,
        );
        Some((hb, world))
    }

    /// Where the active hitbox was at the start of this tick (for sweeping).
    pub fn active_hitbox_prev(&self) -> Option<Vec2> {
        let (hb, world) = self.active_hitbox()?;
        let _ = hb;
        Some(world + (self.prev_pos - self.pos))
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
        self.prev_pos = self.pos;

        self.intangible = self.intangible.saturating_sub(1);
        self.tech_lockout = self.tech_lockout.saturating_sub(1);
        self.ledge_regrab_cd = self.ledge_regrab_cd.saturating_sub(1);
        self.coyote = self.coyote.saturating_sub(1);
        self.pummel_cd = self.pummel_cd.saturating_sub(1);
        self.tech_armed = self.tech_armed.saturating_sub(1);
        // Smash flick window: the stick just crossed into "hard".
        if input.stick.length() > HARD && self.stick_last.length() <= HARD {
            self.stick_flick = k::SMASH_FLICK_FRAMES;
        } else {
            self.stick_flick = self.stick_flick.saturating_sub(1);
        }
        // A shield press while tumbling opens the tech window (and starts
        // the lockout so mashing does not work). Read even during hitlag.
        if matches!(self.state, State::Hitstun { tumble: true })
            && self.pressed(input.buttons, buttons::SHIELD)
            && self.tech_lockout == 0
        {
            self.tech_armed = k::TECH_WINDOW;
            self.tech_lockout = k::TECH_WINDOW + k::TECH_LOCKOUT;
        }
        if self.anim_flash > 0 {
            self.anim_flash -= 1;
        }

        if self.hitlag > 0 {
            self.hitlag -= 1;
            // Smash DI: a fresh hard stick input during hitlag nudges the
            // victim; ASDI (the stick held on the last freeze frame) nudges
            // half as far. Only while being hit, never into the ground.
            if matches!(self.state, State::Hitstun { .. }) {
                let st = input.stick;
                if self.hitlag == 0 {
                    if let Some((kb, base)) = self.pending_launch.take() {
                        // DI: rotate the launch by up to DI_MAX toward the
                        // stick's perpendicular component.
                        let angle = super::knockback::apply_di(base, st);
                        self.kb_vel = super::knockback::launch_velocity(kb, angle);
                        self.vel = self.kb_vel;
                    }
                }
                let hard = st.length() > HARD;
                let was_hard = self.prev_stick.length() > HARD;
                let turned = hard
                    && was_hard
                    && (st.normalized_or_zero() - self.prev_stick.normalized_or_zero()).length()
                        > 0.55;
                if hard && (!was_hard || turned) {
                    self.sdi_nudge(st.normalized_or_zero() * k::SDI_STEP, stage, false);
                } else if self.hitlag == 0 && hard {
                    self.sdi_nudge(st.normalized_or_zero() * k::ASDI_STEP, stage, true);
                }
            }
            self.prev_stick = input.stick;
            self.remember_inputs(input);
            return out;
        }
        self.prev_stick = input.stick;

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
            State::RunTurn => self.tick_run_turn(input),
            State::Tech { dir } => self.tick_tech(dir, stage),
            State::Getup { kind } => self.tick_getup(kind, stage),
            State::Rebound { total } if self.state_frame >= total => {
                self.set_state(State::Stand);
            }
            State::Grab => self.tick_grab(),
            State::Hold => out.events |= self.tick_hold(input),
            State::Grabbed => {
                // Mashing: every fresh button or hard stick direction
                // shortens the holder's grab (handled by the match state).
                let fresh_button = input.buttons & !self.prev_buttons != 0;
                let st = input.stick;
                let hard = st.length() > HARD;
                let was_hard = self.prev_stick.length() > HARD;
                let turned = hard
                    && was_hard
                    && (st.normalized_or_zero() - self.prev_stick.normalized_or_zero()).length()
                        > 0.55;
                if fresh_button || (hard && (!was_hard || turned)) {
                    out.events |= ev::MASH;
                }
            }
            State::Throw { id } => self.tick_throw(id),
            State::Shield => out.spawn_projectile = self.tick_shield(input),
            State::ShieldStun { total } if self.state_frame >= total => {
                self.set_state(State::Shield);
            }
            State::Roll { dir } => self.tick_roll(dir, stage),
            State::Spotdodge => self.tick_spotdodge(),
            State::Airdodge => self.tick_airdodge(),
            State::Helpless => {}
            State::ShieldDrop if self.state_frame >= k::SHIELD_DROP => {
                self.set_state(State::Stand);
            }
            // Edge-cancel: sliding off the platform during landing lag or a
            // waveland ends it at once.
            State::LandLag { .. } | State::Waveland if !self.grounded && self.state_frame > 0 => {
                self.set_state(State::Air);
            }
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
            State::Attack { id, aerial } => {
                let md = attacks::data(self.character.id, id);
                // Jab 1 → jab 2: a fresh attack press once the first jab has
                // come out (and it connected or was blocked) chains.
                if id == MoveId::Jab
                    && !aerial
                    && self.already_hit
                    && self.pressed(input.buttons, buttons::ATTACK)
                    && self.state_frame >= md.startup
                    && self.state_frame < md.startup + md.active + k::JAB_CHAIN_WINDOW
                {
                    self.set_state(State::Attack {
                        id: MoveId::Jab2,
                        aerial: false,
                    });
                    return out_events_swing(&mut out);
                }
                // Smash charge: hold the wind-up at its charge frame while
                // attack stays held (button-started smashes only).
                if !aerial && attacks::is_smash(id) && self.charge_armed {
                    let held = input.buttons & buttons::ATTACK != 0;
                    let at_charge_frame = self.state_frame + 1 == (md.startup / 2).max(1);
                    if held && at_charge_frame && self.charge < k::SMASH_CHARGE_MAX {
                        self.charge += 1;
                        // Cancel this tick's frame advance: stay put.
                        self.state_frame = self.state_frame.wrapping_sub(1);
                    } else if !held || self.charge >= k::SMASH_CHARGE_MAX {
                        self.charge_armed = false;
                    }
                }
                if self.state_frame >= md.total() {
                    self.set_state(if self.grounded {
                        State::Stand
                    } else if aerial && id == MoveId::SpecialUp {
                        // Recovery moves leave you helpless until you land.
                        State::Helpless
                    } else {
                        State::Air
                    });
                }
            }
            _ => {}
        }

        // ---- free-state intent ----
        let before = self.state;
        if self.is_actionable() && self.handle_free_intent(input) {
            out.spawn_projectile = true;
        }
        if before != self.state {
            match self.state {
                State::Attack { .. } => out.events |= ev::SWING,
                State::Dash if !matches!(before, State::Dash) => out.events |= ev::DASH,
                State::Air if !self.grounded && !matches!(before, State::Air) => {
                    out.events |= ev::DOUBLE_JUMP
                }
                _ => {}
            }
        }
        if matches!(before, State::JumpSquat) && matches!(self.state, State::Air) {
            out.events |= ev::JUMP;
        }

        // ---- velocity from physics ----
        if self.grounded {
            self.ground_control(input);
        } else {
            self.air_control(input);
        }

        // ---- single integration ----
        let hanging = matches!(self.state, State::LedgeAction { .. }) && self.ledge.is_some();
        if !matches!(
            self.state,
            State::Roll { .. } | State::LedgeGrab | State::Grabbed
        ) && !hanging
        {
            self.pos += self.vel;
        }

        // ---- stage collision ----
        let was_grounded = self.grounded;
        self.collide_stage(stage, input);
        if self.grounded && !was_grounded {
            out.events |= if matches!(self.state, State::Waveland) {
                ev::WAVEDASH
            } else {
                ev::LAND
            };
        }

        self.remember_inputs(input);
        self.state_frame = self.state_frame.saturating_add(1);
        out
    }

    /// Move the fighter by an SDI/ASDI nudge. SDI may not cross into the
    /// ground; ASDI may (which is what lets a grounded victim land at once).
    fn sdi_nudge(&mut self, delta: Vec2, stage: &Stage, allow_ground: bool) {
        let mut d = delta;
        let main = stage.main();
        let over = self.pos.x >= main.left && self.pos.x <= main.right;
        if !allow_ground && over && self.pos.y + d.y < main.y {
            d.y = (main.y - self.pos.y).max(0.0);
        }
        self.pos += d;
        if allow_ground && over && self.pos.y <= main.y && self.pos.y >= main.y - k::ASDI_STEP {
            // ASDI down onto the stage: land now (hitstun is cancelled by the
            // landing, as in the reference game).
            self.pos.y = main.y;
            self.grounded = true;
            self.kb_vel = Vec2::ZERO;
            self.kb_fall = 0.0;
            self.vel = Vec2::ZERO;
            self.hitstun_timer = 0;
            self.set_state(State::LandLag {
                total: k::LANDING_LAG_NORMAL,
            });
        }
    }

    #[inline]
    fn remember_inputs(&mut self, input: &PlayerInput) {
        self.prev_buttons = input.buttons;
        self.prev_cstick_len = input.cstick.length();
        self.stick_last = input.stick;
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
        self.kb_fall = 0.0;
        self.ground_stun = false;
        self.pending_launch = None;
        self.stale = [None; k::STALE_QUEUE];
        self.meteor = false;
        self.tech_armed = 0;
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
                // Air-dodge speed scales with how far the stick is tilted (a
                // full tilt is `airdodge_speed`), so a partial tilt gives a
                // short wavedash — length is a choice, as in the reference.
                let len = input.stick.length();
                let dir = if len > DEADZONE {
                    if len > 1.0 {
                        input.stick.normalized_or_zero()
                    } else {
                        input.stick
                    }
                } else {
                    Vec2::ZERO
                };
                self.airdodge_dir = dir;
                self.vel = dir * ch.airdodge_speed;
                self.set_state(State::Airdodge);
                return false;
            }
        }

        // Grab (grounded). From a dash or run it is the slower, sliding
        // dash grab.
        if self.pressed(cur, buttons::GRAB) && self.grounded {
            self.grab_dash = matches!(self.state, State::Dash | State::Run);
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

        // Attacks. A C-stick flick is always a smash; the attack button is a
        // smash when the stick was *flicked* to hard just now, a tilt when
        // it is merely held.
        let cst_len = input.cstick.length();
        if self.pressed(cur, buttons::ATTACK) || self.cstick_flick(cst_len) {
            let c_smash = self.cstick_flick(cst_len);
            let a_smash = !c_smash && self.stick_flick > 0 && input.stick.length() > HARD;
            if self.grounded {
                let smash_dir = if c_smash {
                    Some(input.cstick)
                } else if a_smash {
                    Some(input.stick)
                } else {
                    None
                };
                let id = self.pick_ground_attack(input, smash_dir);
                // Dash attack carries the run into the hit; everything else
                // plants the feet.
                if id == MoveId::DashAttack {
                    self.vel.x = self.facing * ch.run_max * 0.85;
                } else {
                    self.vel.x = 0.0;
                }
                self.charge = 0;
                self.charge_armed = a_smash && attacks::is_smash(id);
                self.set_state(State::Attack { id, aerial: false });
            } else {
                let smash = c_smash;
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

    /// Charged-smash damage multiplier (1.0 when uncharged).
    pub fn charge_multiplier(&self) -> f32 {
        1.0 + k::SMASH_CHARGE_BONUS
            * (self.charge.min(k::SMASH_CHARGE_MAX) as f32 / k::SMASH_CHARGE_MAX as f32)
    }

    fn pick_ground_attack(&mut self, input: &PlayerInput, smash: Option<Vec2>) -> MoveId {
        let sx = input.stick.x;
        let sy = input.stick.y;
        let dashing = matches!(self.state, State::Dash | State::Run) && sx.abs() > DEADZONE;
        if dashing {
            return MoveId::DashAttack;
        }
        if let Some(c) = smash {
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
        // Tilts come from a *held* stick at any tilt past the deadzone (a
        // flick to hard is a smash, handled above); jab is neutral.
        if sy > TILT_Y {
            MoveId::Utilt
        } else if sy < -TILT_Y {
            MoveId::Dtilt
        } else if sx.abs() > DEADZONE {
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
            let f = match self.state {
                // Sliding faster than a walk decelerates twice as hard (the
                // reference's doubled traction), which is what keeps wavedash
                // and run-stop slides short and controllable.
                State::Waveland if self.vel.x.abs() > ch.walk_max => ch.traction * 2.0,
                State::Waveland => ch.traction,
                State::Attack {
                    id: MoveId::DashAttack,
                    ..
                } => ch.ground_friction * 0.9,
                State::LandLag { .. } | State::Attack { .. } => ch.ground_friction * 1.2,
                State::RunTurn => ch.ground_friction * 1.5,
                State::Rebound { .. } => ch.ground_friction * 1.5,
                State::ShieldStun { .. } => ch.traction * 0.8,
                _ => ch.ground_friction * 0.6,
            };
            self.vel.x = approach(self.vel.x, 0.0, f);
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
                        // Analog walk: speed follows the tilt, from a creep at
                        // the deadzone to `walk_max` just under the dash
                        // threshold.
                        let t = ((sx.abs() - DEADZONE) / (HARD - DEADZONE)).clamp(0.0, 1.0);
                        let speed = ch.walk_max * (0.3 + 0.7 * t);
                        self.set_state(State::Walk);
                        self.vel.x = approach(self.vel.x, want * speed, ch.ground_accel);
                    }
                }
                State::Dash => {
                    if want != self.facing && self.state_frame < ch.dash_frames {
                        self.facing = want;
                        self.set_state(State::Dash);
                    } else if self.state_frame >= ch.dash_frames {
                        self.set_state(State::Run);
                    }
                    self.vel.x =
                        approach(self.vel.x, self.facing * ch.dash_max, ch.ground_accel * 2.0);
                }
                State::Run => {
                    if want != self.facing {
                        // Past the dash-dance window a reversal is a braking
                        // turn, not a fresh dash.
                        self.set_state(State::RunTurn);
                        return;
                    }
                    self.vel.x = approach(self.vel.x, self.facing * ch.run_max, ch.ground_accel);
                }
                _ => {}
            }
        } else {
            // Letting go of the stick stops you at doubled traction: a walk
            // stop takes ~9 frames, a run stop ~13, so spacing is precise
            // while wavedash slides (traction, above) keep their length.
            self.vel.x = approach(self.vel.x, 0.0, ch.ground_friction * 2.0);
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
        // Jump-cancels: a grab or an up-smash started during the squat
        // replaces the jump (this is what lets a dash turn into a standing
        // grab or an up-smash without the run's momentum being lost).
        let cur = input.buttons;
        if self.pressed(cur, buttons::GRAB) {
            self.grab_dash = false;
            self.set_state(State::Grab);
            return;
        }
        let c_up = input.cstick.y > HARD && self.cstick_flick(input.cstick.length());
        let a_up = self.pressed(cur, buttons::ATTACK) && input.stick.y > HARD;
        if c_up || a_up {
            self.charge = 0;
            self.charge_armed = a_up;
            self.set_state(State::Attack {
                id: MoveId::Usmash,
                aerial: false,
            });
            return;
        }
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
            self.set_state(State::Helpless);
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

    fn tick_roll(&mut self, dir: f32, stage: &Stage) {
        if self.state_frame == k::ROLL_INTANGIBLE.0 {
            self.intangible = k::ROLL_INTANGIBLE.1 - k::ROLL_INTANGIBLE.0 + 1;
        }
        let step = k::ROLL_DISTANCE / k::ROLL_DURATION as f32;
        self.slide_on_platform(dir * step, stage);
        self.vel = Vec2::ZERO;
        if self.state_frame >= k::ROLL_DURATION {
            self.set_state(State::Stand);
        }
    }

    /// Move along the platform you stand on, stopping at its edges (rolls,
    /// tech rolls and getup rolls never carry you off a ledge).
    fn slide_on_platform(&mut self, dx: f32, stage: &Stage) {
        self.pos.x += dx;
        if let Some(p) = self.support.and_then(|i| stage.platforms.get(i)) {
            let hw = self.character.half_width;
            self.pos.x = self.pos.x.clamp(p.left + hw, p.right - hw);
        }
    }

    /// Returns `true` when this tick's out-of-shield option is the
    /// projectile action and the shot must actually be requested — the
    /// shield entry emits exactly like the free-state entry does. The edge
    /// comes from `pressed`, so holding SPECIAL afterwards repeats nothing,
    /// and no new cancel option is opened here.
    #[must_use]
    fn tick_shield(&mut self, input: &PlayerInput) -> bool {
        let cur = input.buttons;
        let sx = input.stick.x;
        let sy = input.stick.y;

        // Shield shrinks while held, and breaks if depleted.
        self.shield_health -= k::SHIELD_DECAY;
        if self.shield_health <= 0.0 {
            self.shield_health = 0.0;
            // shield break -> long stun
            self.set_state(State::ShieldStun { total: 120 });
            return false;
        }

        // Out-of-shield options.
        if self.pressed(cur, buttons::JUMP) {
            self.set_state(State::JumpSquat);
            self.jump_held_at_squat_start = true;
            return false;
        }
        if self.pressed(cur, buttons::GRAB) || self.pressed(cur, buttons::ATTACK) {
            self.grab_dash = false;
            self.set_state(State::Grab);
            return false;
        }
        if self.pressed(cur, buttons::SPECIAL) {
            self.set_state(State::Attack {
                id: MoveId::SpecialN,
                aerial: false,
            });
            // This entry is already allowed; it just never asked for the
            // shot. Every supported entry into the projectile action emits
            // once, so request it here exactly as `handle_free_intent` does
            // from an actionable state.
            return true;
        }
        if sx.abs() > HARD {
            self.set_state(State::Roll { dir: sx.signum() });
            return false;
        }
        if sy < -HARD {
            self.set_state(State::Spotdodge);
            return false;
        }
        if cur & buttons::SHIELD == 0 {
            self.set_state(State::ShieldDrop);
        }
        false
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
                State::Air | State::Attack { aerial: true, .. } | State::Airdodge | State::Helpless
            )
        {
            for (i, l) in stage.ledges.iter().enumerate() {
                if self.ledge_blocked & (1 << (i & 7)) != 0 {
                    continue; // edgehogged
                }
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
        let prev_support = self.support;
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

            // Drop through a soft platform with a quick down flick (neutral
            // to hard down in one frame) or down + jump.
            let flick = input.stick.y < -HARD && self.stick_last.y > -DEADZONE;
            let jump =
                (self.prev_buttons & buttons::JUMP == 0) && (input.buttons & buttons::JUMP != 0);
            // Only from *standing on* the platform: you cannot fall through
            // one from the air by holding down.
            let drop_through = !p.solid
                && was_grounded
                && prev_support == Some(i)
                && input.stick.y < -HARD
                && (flick || jump);
            if drop_through {
                // Let go of the platform: airborne at once, and already a
                // hair below it so the next tick does not re-land.
                self.support = None;
                self.set_state(State::Air);
                self.pos.y -= 1.0;
                self.vel.y = -0.5;
                continue;
            }

            if falling && crossed {
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
                let md = attacks::data(self.character.id, id);
                let mut lag = if md.autocancels(self.state_frame) {
                    k::LANDING_LAG_NORMAL
                } else {
                    md.landing_lag
                };
                if self.lcancel_armed && lag > k::LANDING_LAG_NORMAL {
                    lag /= 2;
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
            State::Air | State::Helpless => {
                self.vel.x *= 0.85;
                self.set_state(State::LandLag {
                    total: k::LANDING_LAG_NORMAL,
                });
            }
            _ => {}
        }
    }

    fn grab_ledge(&mut self, idx: usize, l: super::stage::Ledge) {
        self.ledge = Some(idx);
        // Hang just outside the edge, hands on it, facing the stage.
        self.pos = Vec2::new(
            l.pos.x + l.side * self.character.half_width,
            l.pos.y - self.character.height,
        );
        self.vel = Vec2::ZERO;
        self.facing = -l.side;
        self.grounded = false;
        self.jumps_left = self.character.air_jumps;
        self.fastfalling = false;
        self.intangible = k::LEDGE_INTANGIBLE;
        self.set_state(State::LedgeGrab);
    }

    /// Tired (slow, punishable getup options) at 100 % and above.
    #[inline]
    pub fn ledge_tired(&self) -> usize {
        usize::from(self.percent >= k::LEDGE_TIRED_PERCENT)
    }

    /// `(total frames, intangible frames, hit frame)` of a ledge option for
    /// this fighter's current percent (hit frame is 0 unless it attacks).
    pub fn ledge_option(&self, kind: LedgeKind) -> (u32, u32, u32) {
        let t = self.ledge_tired();
        match kind {
            LedgeKind::Getup => {
                let (a, b) = k::LEDGE_GETUP[t];
                (a, b, 0)
            }
            LedgeKind::Roll => {
                let (a, b) = k::LEDGE_ROLL[t];
                (a, b, 0)
            }
            LedgeKind::Attack => k::LEDGE_ATTACK[t],
            LedgeKind::Jump => (k::LEDGE_JUMP.0 + k::LEDGE_JUMP.1, 0, 0),
        }
    }

    fn start_ledge_option(&mut self, kind: LedgeKind) {
        let (_, inv, _) = self.ledge_option(kind);
        // Option intangibility never shortens what the catch already gave.
        self.intangible = self.intangible.max(inv);
        self.set_state(State::LedgeAction { kind });
    }

    fn tick_ledge(&mut self, input: &PlayerInput) {
        let cur = input.buttons;
        let sx = input.stick.x;
        let sy = input.stick.y;
        let out_dir = -self.facing;
        // Hang limit: fresh fighters hold on longer than tired ones.
        if self.state_frame >= k::LEDGE_HANG[self.ledge_tired()] {
            self.ledge = None;
            self.ledge_regrab_cd = k::LEDGE_REGRAB_CD;
            self.intangible = 0;
            self.vel = Vec2::ZERO;
            self.set_state(State::Air);
            return;
        }
        if self.state_frame >= k::LEDGE_CATCH {
            // Jump / up: ledge jump. Attack: ledge attack. Shield: roll.
            // Toward the stage: getup. Down or away: let go.
            if self.pressed(cur, buttons::JUMP) || sy > HARD {
                self.start_ledge_option(LedgeKind::Jump);
            } else if self.pressed(cur, buttons::ATTACK) || self.pressed(cur, buttons::SPECIAL) {
                self.start_ledge_option(LedgeKind::Attack);
            } else if self.pressed(cur, buttons::SHIELD) {
                self.start_ledge_option(LedgeKind::Roll);
            } else if sx.signum() == self.facing && sx.abs() > HARD {
                self.start_ledge_option(LedgeKind::Getup);
            } else if sy < -HARD || (sx.signum() == out_dir && sx.abs() > HARD) {
                // Ledge drop keeps the catch intangibility (ledgedash).
                self.ledge = None;
                self.ledge_regrab_cd = k::LEDGE_REGRAB_CD;
                self.vel = Vec2::ZERO;
                self.set_state(State::Air);
            }
        }
    }

    /// Step onto the stage from the ledge, `step` units past the body edge.
    fn ledge_climb(&mut self, step: f32) {
        let ch = self.character;
        self.ledge = None;
        self.pos.y += ch.height + 1.0;
        self.pos.x += self.facing * (ch.half_width + step);
        self.vel = Vec2::ZERO;
        self.grounded = true;
    }

    fn tick_ledge_action(&mut self, kind: LedgeKind) {
        let ch = self.character;
        let sf = self.state_frame;
        let (total, _, hit) = self.ledge_option(kind);
        match kind {
            LedgeKind::Jump => {
                if sf == k::LEDGE_JUMP.0 {
                    // Leave the ledge: a committed jump up and in.
                    self.ledge = None;
                    self.grounded = false;
                    self.pos.x += self.facing * 2.0;
                    self.vel = Vec2::new(self.facing * ch.air_max * 0.9, ch.fullhop_v);
                    self.ledge_regrab_cd = k::LEDGE_REGRAB_CD;
                }
                if sf >= total || (self.grounded && sf > k::LEDGE_JUMP.0) {
                    self.set_state(if self.grounded {
                        State::Stand
                    } else {
                        State::Air
                    });
                }
            }
            LedgeKind::Getup => {
                if sf == total * 2 / 5 {
                    self.ledge_climb(k::LEDGE_GETUP_STEP);
                }
                if sf >= total {
                    self.set_state(State::Stand);
                }
            }
            LedgeKind::Roll => {
                if sf == total * 2 / 5 {
                    self.ledge_climb(k::LEDGE_ROLL_STEP);
                }
                if sf >= total {
                    self.set_state(State::Stand);
                }
            }
            LedgeKind::Attack => {
                if sf + 6 == hit {
                    self.ledge_climb(k::LEDGE_GETUP_STEP);
                }
                if sf >= total {
                    self.set_state(State::Stand);
                }
            }
        }
    }

    fn tick_hitstun(&mut self, input: &PlayerInput, stage: &Stage, tumble: bool) {
        let ch = self.character;
        if self.ground_stun {
            // Weak grounded hit: slide along the floor, no gravity, then stand.
            self.kb_vel.x = approach(self.kb_vel.x, 0.0, k::KB_DECAY + ch.ground_friction * 0.5);
            self.kb_vel.y = 0.0;
            self.vel = self.kb_vel;
            self.pos += self.vel;
            let main = stage.main();
            if self.pos.x < main.left || self.pos.x > main.right {
                // Slid off the edge: become airborne hitstun.
                self.ground_stun = false;
                self.grounded = false;
            } else if self.hitstun_timer > 0 {
                self.hitstun_timer -= 1;
                if self.hitstun_timer == 0 {
                    self.ground_stun = false;
                    self.vel = Vec2::ZERO;
                    self.set_state(State::Stand);
                }
            } else {
                self.ground_stun = false;
                self.set_state(State::Stand);
            }
            return;
        }
        // Knockback decays uniformly along its direction; gravity is a
        // separate fall velocity capped by fall speed.
        let len = self.kb_vel.length();
        if len > k::KB_DECAY {
            self.kb_vel = self.kb_vel * ((len - k::KB_DECAY) / len);
        } else {
            self.kb_vel = Vec2::ZERO;
        }
        self.kb_fall = (self.kb_fall - ch.gravity).max(-ch.max_fall);
        let _ = input;
        self.vel = self.kb_vel + Vec2::new(0.0, self.kb_fall);
        self.pos += self.vel;

        // Ground collision → tech or knockdown.
        let main = stage.main();
        let over = self.pos.x >= main.left && self.pos.x <= main.right;
        if self.vel.y < 0.0 && self.pos.y <= main.y && over {
            self.pos.y = main.y;
            let teching = tumble && self.tech_armed > 0;
            self.grounded = true;
            self.vel = Vec2::ZERO;
            self.kb_vel = Vec2::ZERO;
            self.kb_fall = 0.0;
            self.meteor = false;
            if teching {
                let sx = input.stick.x;
                let dir = if sx.abs() > HARD { sx.signum() } else { 0.0 };
                self.tech_armed = 0;
                self.intangible = if dir == 0.0 {
                    k::TECH_IN_PLACE.1
                } else {
                    k::TECH_ROLL.1
                };
                self.set_state(State::Tech { dir });
            } else if tumble {
                self.set_state(State::Knockdown);
            } else {
                self.set_state(State::LandLag {
                    total: k::LANDING_LAG_NORMAL,
                });
            }
            return;
        }

        // Meteor cancel: a spike can be jumped out of after a few frames.
        if self.meteor && self.state_frame >= k::METEOR_CANCEL_FRAMES {
            let cur = input.buttons;
            if self.pressed(cur, buttons::JUMP) && self.jumps_left > 0 {
                self.jumps_left -= 1;
                self.meteor = false;
                self.hitstun_timer = 0;
                self.kb_vel = Vec2::ZERO;
                self.kb_fall = 0.0;
                self.vel = Vec2::new(0.0, ch.doublejump_v);
                self.set_state(State::Air);
                return;
            }
            if self.pressed(cur, buttons::SPECIAL) && input.stick.y > HARD {
                self.meteor = false;
                self.hitstun_timer = 0;
                self.kb_vel = Vec2::ZERO;
                self.kb_fall = 0.0;
                self.vel = Vec2::ZERO;
                self.set_state(State::Attack {
                    id: MoveId::SpecialUp,
                    aerial: true,
                });
                return;
            }
        }

        if self.hitstun_timer > 0 {
            self.hitstun_timer -= 1;
            if self.hitstun_timer == 0 {
                self.meteor = false;
                self.set_state(State::Air);
                self.vel = self.kb_vel + Vec2::new(0.0, self.kb_fall);
            }
        } else {
            self.meteor = false;
            self.set_state(State::Air);
            self.vel = self.kb_vel + Vec2::new(0.0, self.kb_fall);
        }
    }

    // ---------------------- turns, techs, getups ----------------------

    fn tick_run_turn(&mut self, input: &PlayerInput) {
        let ch = self.character;
        if self.pressed(input.buttons, buttons::JUMP) {
            self.set_state(State::JumpSquat);
            self.jump_held_at_squat_start = true;
            return;
        }
        // Face the new way once the brake is half done.
        if self.state_frame == k::RUN_TURN / 2 {
            self.facing = -self.facing;
        }
        if self.state_frame >= k::RUN_TURN {
            let sx = input.stick.x;
            if sx.abs() > HARD && sx.signum() == self.facing {
                self.vel.x = self.facing * ch.dash_max * 0.5;
                self.set_state(State::Run);
            } else {
                self.set_state(State::Stand);
            }
        }
    }

    fn tick_tech(&mut self, dir: f32, stage: &Stage) {
        let (total, _) = if dir == 0.0 {
            k::TECH_IN_PLACE
        } else {
            k::TECH_ROLL
        };
        if dir != 0.0 && (4..24).contains(&self.state_frame) {
            self.slide_on_platform(dir * k::TECH_ROLL_DISTANCE / 20.0, stage);
            self.facing = dir;
        }
        self.vel = Vec2::ZERO;
        if self.state_frame >= total {
            self.set_state(State::Stand);
        }
    }

    fn tick_getup(&mut self, kind: GetupKind, stage: &Stage) {
        self.vel = Vec2::ZERO;
        let total = match kind {
            GetupKind::Stand => k::GETUP_STAND.0,
            GetupKind::Roll { dir } => {
                if (4..26).contains(&self.state_frame) {
                    self.slide_on_platform(dir * k::GETUP_ROLL_DISTANCE / 22.0, stage);
                    self.facing = dir;
                }
                k::GETUP_ROLL.0
            }
            GetupKind::Attack => k::GETUP_ATTACK.0,
        };
        if self.state_frame >= total {
            self.set_state(State::Stand);
        }
    }

    fn tick_knockdown(&mut self, input: &PlayerInput) {
        self.vel = Vec2::ZERO;
        if self.state_frame < k::KNOCKDOWN_BOUNCE {
            return;
        }
        let cur = input.buttons;
        let sx = input.stick.x;
        let sy = input.stick.y;
        let kind = if self.pressed(cur, buttons::ATTACK) || self.pressed(cur, buttons::SPECIAL) {
            Some(GetupKind::Attack)
        } else if sx.abs() > HARD {
            Some(GetupKind::Roll { dir: sx.signum() })
        } else if sy > HARD
            || self.pressed(cur, buttons::JUMP)
            || self.pressed(cur, buttons::SHIELD)
            || self.pressed(cur, buttons::GRAB)
        {
            Some(GetupKind::Stand)
        } else {
            None
        };
        if let Some(kind) = kind {
            let inv = match kind {
                GetupKind::Stand => k::GETUP_STAND.1,
                GetupKind::Roll { .. } => k::GETUP_ROLL.1,
                GetupKind::Attack => k::GETUP_ATTACK.1,
            };
            self.intangible = inv;
            self.set_state(State::Getup { kind });
        }
    }

    /// Damage multiplier for `id` from the stale-move queue.
    pub fn stale_multiplier(&self, id: MoveId) -> f32 {
        let mut m = 1.0;
        for (slot, used) in self.stale.iter().enumerate() {
            if *used == Some(id) {
                m -= k::STALE_STEPS[slot];
            }
        }
        m
    }

    /// Record a connected hit of `id` (newest first).
    pub fn stale_push(&mut self, id: MoveId) {
        self.stale.rotate_right(1);
        self.stale[0] = Some(id);
    }

    /// The grab's `(first hit frame, last hit frame, total)`, 1-based.
    pub fn grab_frames(&self) -> (u32, u32, u32) {
        if self.grab_dash {
            k::GRAB_DASH
        } else {
            k::GRAB_STAND
        }
    }

    /// Is the grab box out? (Read after the tick, when `state_frame` equals
    /// the 1-based frame of the move — the same convention as hitboxes.)
    pub fn grab_active(&self) -> bool {
        let (a, b, _) = self.grab_frames();
        matches!(self.state, State::Grab) && (a..=b).contains(&self.state_frame)
    }

    fn tick_grab(&mut self) {
        let (_, _, total) = self.grab_frames();
        if self.state_frame >= total {
            self.set_state(State::Stand);
        }
    }

    /// Holding an opponent: the stick (or C-stick) throws, attack / grab
    /// pummels, and the hold runs out on its own. Returns feedback events.
    fn tick_hold(&mut self, input: &PlayerInput) -> u16 {
        self.grab_timer = self.grab_timer.saturating_sub(1);
        let sx = input.stick.x;
        let sy = input.stick.y;
        let cur = input.buttons;
        let (tx, ty) = if input.cstick.length() > HARD {
            (input.cstick.x, input.cstick.y)
        } else {
            (sx, sy)
        };
        let want_throw = tx.abs() > HARD || ty.abs() > HARD;
        if want_throw {
            let id = if ty > HARD {
                MoveId::ThrowU
            } else if ty < -HARD {
                MoveId::ThrowD
            } else if tx.signum() == self.facing {
                MoveId::ThrowF
            } else {
                MoveId::ThrowB
            };
            self.set_state(State::Throw { id });
            0
        } else if self.grab_timer == 0 {
            // Grab release: both get lag, the victim is shoved off.
            self.grabbing = None;
            self.set_state(State::LandLag {
                total: k::GRAB_RELEASE_LAG,
            });
            0
        } else if (self.pressed(cur, buttons::ATTACK) || self.pressed(cur, buttons::GRAB))
            && self.pummel_cd == 0
        {
            self.pummel_cd = k::PUMMEL_COOLDOWN;
            ev::PUMMEL
        } else {
            0
        }
    }

    fn tick_throw(&mut self, id: MoveId) {
        let md = attacks::data(self.character.id, id);
        if self.state_frame >= md.total() {
            self.grabbing = None;
            self.set_state(State::Stand);
        }
    }

    /// Enter hitstun with a launch velocity and duration.
    /// Launch the fighter. A non-tumble hit on a grounded fighter with no
    /// upward component keeps them on the ground (grounded flinch/slide).
    pub fn apply_launch(&mut self, kb_vel: Vec2, hitstun: u32, tumble: bool, hitlag: u32) {
        let stay_grounded = self.grounded && !tumble && kb_vel.y <= 0.001;
        self.pending_launch = None;
        self.kb_vel = kb_vel;
        self.kb_fall = 0.0;
        self.vel = kb_vel;
        self.ground_stun = stay_grounded;
        if !stay_grounded {
            self.grounded = false;
            self.support = None;
        }
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

    /// Begin holding an opponent (successful grab). The hold lasts
    /// `⌊76 + 1.6 × victim percent⌋` frames minus whatever they mash off.
    pub fn catch(&mut self, victim: usize, victim_percent: f32) {
        self.grabbing = Some(victim);
        self.grab_timer =
            (k::GRAB_HOLD_BASE + k::GRAB_HOLD_PER_PERCENT * victim_percent).floor() as u32;
        self.pummel_cd = 0;
        self.vel = Vec2::ZERO;
        self.set_state(State::Hold);
    }

    /// Released from a hold that ran out: shoved away with release lag.
    pub fn grab_release(&mut self, away: f32) {
        self.grabbed_by = None;
        self.vel = Vec2::new(away * k::GRAB_RELEASE_PUSH, 0.0);
        self.set_state(State::LandLag {
            total: k::GRAB_RELEASE_LAG,
        });
    }

    /// Become grabbed by a holder.
    pub fn set_grabbed(&mut self, holder: usize) {
        self.grabbed_by = Some(holder);
        self.vel = Vec2::ZERO;
        self.set_state(State::Grabbed);
    }

    /// Take a hit while shielding: reduce shield, apply shield stun/pushback.
    /// Block a hit. Returns `true` if it was a powershield (raised within
    /// [`k::POWERSHIELD_WINDOW`] frames): no damage, stun or pushback.
    pub fn shield_hit(&mut self, damage: f32) -> bool {
        if matches!(self.state, State::Shield) && self.state_frame < k::POWERSHIELD_WINDOW {
            self.anim_flash = 4;
            return true;
        }
        self.shield_health -= damage * k::SHIELD_DAMAGE_MULT;
        let stun = super::knockback::shieldstun(damage);
        self.vel.x = -self.facing * super::knockback::shield_push(damage);
        if self.shield_health <= 0.0 {
            self.shield_health = 0.0;
            self.set_state(State::ShieldStun { total: 120 });
        } else {
            self.set_state(State::ShieldStun { total: stun });
        }
        false
    }
}
