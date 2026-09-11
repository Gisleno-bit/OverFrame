//! The top-level game state and the single pure `step` function.
//!
//! `GameState::step(&mut self, inputs)` advances the whole match by exactly one
//! 60 Hz tick. It reads nothing but its own fields and the inputs, so it is
//! deterministic and fully save/load-able — which is precisely what the GGRS
//! rollback layer (see `netcode.rs`) requires.

use super::attacks::{self, MoveId, Projectile};
use super::constants as k;
use super::fighter::{Fighter, State};
use super::input::PlayerInput;
use super::knockback;
use super::math::{clampf, Rng, Vec2};
use super::roster::CharacterId;
use super::stage::{Stage, StageId};

/// Match rules. `Copy` on purpose: it is sent over the lobby protocol and
/// stored in the saved state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MatchConfig {
    pub stocks: i32,
    /// Time limit in seconds; `0` = no limit. On expiry the player with the
    /// most stocks wins (tie-break: lower percent; otherwise a draw).
    pub time_limit_secs: u32,
    pub seed: u32,
    pub stage: StageId,
    /// Character per player slot (only the first `num_players` are used).
    pub chars: [CharacterId; 4],
    /// Colour palette per player slot.
    pub palettes: [u8; 4],
}

impl Default for MatchConfig {
    fn default() -> Self {
        MatchConfig {
            stocks: 4,
            time_limit_secs: 0,
            seed: 0x1234_5678,
            stage: StageId::Lattice,
            chars: [CharacterId::Kestrel; 4],
            palettes: [0, 1, 2, 3],
        }
    }
}

impl MatchConfig {
    /// Frames remaining before the time limit, if any.
    pub fn time_limit_frames(&self) -> Option<u64> {
        if self.time_limit_secs == 0 {
            None
        } else {
            Some(self.time_limit_secs as u64 * super::constants::FPS as u64)
        }
    }
}

/// A short-lived visual effect (hit spark, blast flash). Part of the state so
/// rollback reproduces it, but it never affects gameplay.
#[derive(Clone, Copy, Debug)]
pub struct Fx {
    pub pos: Vec2,
    pub life: u32,
    pub max_life: u32,
    pub kind: FxKind,
    pub magnitude: f32,
    /// Direction the effect points (launch direction for hits, unit vector).
    pub dir: Vec2,
    /// Frame the effect was created on (lets the renderer trigger one-shot
    /// feedback such as sound exactly once per event, even across rollbacks).
    pub born: u64,
    /// The fighter the effect belongs to (victim for hits/blocks, actor for
    /// the rest); `u8::MAX` if none. Used for controller rumble.
    pub who: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FxKind {
    Hit,
    Shield,
    Powershield,
    Blast,
    Dust,
    /// Landing thump (no visual beyond dust).
    Land,
    Jump,
    /// Attack start (whiff swing sound).
    Swing,
    Tech,
    /// Two attacks clashed (rebound).
    Clank,
}

/// The entire simulation state for one match.
#[derive(Clone, Debug)]
pub struct GameState {
    pub frame: u64,
    pub rng: Rng,
    pub stage: Stage,
    pub fighters: Vec<Fighter>,
    pub projectiles: Vec<Projectile>,
    pub fx: Vec<Fx>,
    pub config: MatchConfig,
    pub match_over: Option<usize>,
    pub camera_shake: f32,
    pub hitstop_flash: f32,
}

impl GameState {
    /// Build a fresh match from its rules (stage, characters, stocks, time).
    pub fn new(num_players: usize, config: MatchConfig) -> Self {
        let stage = Stage::by_id(config.stage);
        let mut fighters = Vec::with_capacity(num_players);
        for i in 0..num_players {
            let spawn = stage.spawns[i % stage.spawns.len()];
            let ch = config.chars[i % 4].data();
            let mut f = Fighter::new(ch, i, Vec2::new(spawn.x, spawn.y + 40.0));
            f.stocks = config.stocks;
            f.palette = config.palettes[i % 4] % super::roster::PALETTES;
            fighters.push(f);
        }
        GameState {
            frame: 0,
            rng: Rng::new(config.seed),
            stage,
            fighters,
            projectiles: Vec::new(),
            fx: Vec::new(),
            config,
            match_over: None,
            camera_shake: 0.0,
            hitstop_flash: 0.0,
        }
    }

    /// A deterministic checksum of **every** piece of state that can affect a
    /// future tick. Used by the GGRS `SyncTest` to prove that save → load →
    /// re-simulate reproduces the exact same state, so it has to be complete:
    /// a field left out here is a desync the test cannot see.
    ///
    /// Deliberately excluded (presentation only, never read by `step`):
    /// `fx`, `camera_shake`, `hitstop_flash`, `anim_flash`, `last_hit_frame`,
    /// `palette`, and `ledge_blocked` (recomputed from scratch every tick).
    /// `tests/determinism.rs` checks field by field that everything else is in.
    pub fn checksum(&self) -> u128 {
        let mut h = Fnv::new();
        h.u64(self.frame);
        h.u32(self.rng.state());
        h.u32(self.stage.id as u32);
        h.opt_usize(self.match_over);
        h.u32(self.config.stocks as u32);
        h.u32(self.config.time_limit_secs);

        for f in &self.fighters {
            h.u32(f.character.id as u32);
            h.u32(f.port as u32);
            // --- body ---
            h.vec2(f.pos);
            h.vec2(f.vel);
            h.f32(f.facing);
            h.bool(f.grounded);
            h.opt_usize(f.support);
            h.vec2(f.prev_pos);
            // --- action ---
            let (tag, payload) = f.state.code();
            h.u32(tag);
            h.u32(payload);
            h.u32(f.state_frame);
            h.bool(f.already_hit);
            // --- jumps / fall ---
            h.u32(f.jumps_left as u32);
            h.bool(f.fastfalling);
            h.bool(f.jump_held_at_squat_start);
            h.u32(f.coyote);
            // --- damage / stocks ---
            h.f32(f.percent);
            h.u32(f.stocks as u32);
            h.u32(f.respawn_timer);
            // --- input memory (gates presses, flicks and SDI) ---
            h.u32(f.prev_buttons as u32);
            h.f32(f.prev_cstick_len);
            h.vec2(f.stick_last);
            h.vec2(f.prev_stick);
            h.u32(f.stick_flick);
            // --- defence ---
            h.f32(f.shield_health);
            h.bool(f.shield_broken);
            h.bool(f.shield_parry_armed);
            h.u32(f.intangible);
            h.u32(f.hitlag);
            h.u32(f.hitstun_timer);
            h.u32(f.tech_lockout);
            h.u32(f.tech_armed);
            h.vec2(f.airdodge_dir);
            h.bool(f.lcancel_armed);
            // --- knockback ---
            h.vec2(f.kb_vel);
            h.f32(f.kb_fall);
            h.bool(f.ground_stun);
            h.bool(f.meteor);
            match f.pending_launch {
                Some((kb, angle)) => {
                    h.u32(1);
                    h.f32(kb);
                    h.f32(angle);
                }
                None => h.u32(0),
            }
            // --- ledge ---
            h.opt_usize(f.ledge);
            h.u32(f.ledge_regrab_cd);
            // --- grabs ---
            h.opt_usize(f.grabbing);
            h.opt_usize(f.grabbed_by);
            h.u32(f.grab_timer);
            h.bool(f.grab_dash);
            h.u32(f.pummel_cd);
            // --- attacks ---
            h.u32(f.charge);
            h.bool(f.charge_armed);
            for slot in &f.stale {
                h.u32(match slot {
                    Some(id) => *id as u32 + 1,
                    None => 0,
                });
            }
        }

        for p in &self.projectiles {
            h.vec2(p.pos);
            h.vec2(p.vel);
            h.f32(p.facing);
            h.u32(p.life);
            h.u32(p.owner as u32);
            h.bool(p.active);
        }
        h.finish()
    }

    /// Advance one tick.
    pub fn step(&mut self, inputs: &[PlayerInput]) {
        // Decay visual state.
        self.camera_shake *= 0.85;
        self.hitstop_flash *= 0.80;
        self.fx.retain(|f| f.life > 0);
        for f in &mut self.fx {
            f.life -= 1;
        }

        // 1) Advance each fighter's own state machine.
        let n = self.fighters.len();
        let mut spawn_reqs: Vec<usize> = Vec::new();
        // Edgehogging: a ledge someone else is holding cannot be grabbed.
        let held: Vec<Option<usize>> = self
            .fighters
            .iter()
            .map(|f| {
                if matches!(f.state, State::LedgeGrab | State::LedgeAction { .. }) {
                    f.ledge
                } else {
                    None
                }
            })
            .collect();
        for i in 0..n {
            self.fighters[i].ledge_blocked = (0..n)
                .filter(|&j| j != i)
                .filter_map(|j| held[j])
                .fold(0u8, |m, l| m | (1 << (l & 7)));
            let input = inputs.get(i).copied().unwrap_or_default();
            let out = self.fighters[i].tick(&input, &self.stage);
            if out.spawn_projectile {
                spawn_reqs.push(i);
            }
            if out.events != 0 {
                use super::fighter::ev;
                let (feet, speed, facing, centre, swing_dmg) = {
                    let f = &self.fighters[i];
                    let heavy = match f.state {
                        State::Attack { id, .. } => attacks::data(f.character.id, id).hitbox.damage,
                        _ => 5.0,
                    };
                    (f.pos, f.vel.length(), f.facing, f.body_center(), heavy)
                };
                let me = i as u8;
                if out.events & ev::LAND != 0 {
                    self.push_fx_who(feet, FxKind::Dust, 6.0 + speed, Vec2::new(0.0, 1.0), me);
                    self.push_fx_who(feet, FxKind::Land, 4.0 + speed, Vec2::new(0.0, 1.0), me);
                }
                if out.events & ev::WAVEDASH != 0 {
                    self.push_fx_dir(feet, FxKind::Dust, 10.0, Vec2::new(-facing, 0.3));
                    self.push_fx(feet, FxKind::Land, 6.0);
                }
                if out.events & ev::JUMP != 0 {
                    self.push_fx_dir(feet, FxKind::Dust, 5.0, Vec2::new(0.0, 1.0));
                    self.push_fx(feet, FxKind::Jump, 5.0);
                }
                if out.events & ev::DOUBLE_JUMP != 0 {
                    self.push_fx(feet, FxKind::Jump, 3.0);
                }
                if out.events & ev::DASH != 0 {
                    self.push_fx_dir(feet, FxKind::Dust, 7.0, Vec2::new(-facing, 0.2));
                }
                if out.events & ev::SWING != 0 {
                    self.push_fx_dir(centre, FxKind::Swing, swing_dmg, Vec2::new(facing, 0.0));
                }
                if out.events & ev::TECH != 0 {
                    self.push_fx(feet, FxKind::Tech, 8.0);
                }
                if out.events & ev::MASH != 0 {
                    if let Some(h) = self.fighters[i].grabbed_by {
                        let t = &mut self.fighters[h].grab_timer;
                        *t = t.saturating_sub(k::GRAB_MASH_FRAMES);
                    }
                }
                if out.events & ev::PUMMEL != 0 {
                    if let Some(v) = self.fighters[i].grabbing {
                        let vc = self.fighters[v].body_center();
                        self.fighters[v].percent += k::PUMMEL_DAMAGE;
                        self.fighters[v].hitlag = k::PUMMEL_HITLAG;
                        self.fighters[v].anim_flash = 3;
                        self.fighters[i].hitlag = k::PUMMEL_HITLAG;
                        self.push_fx_who(
                            vc,
                            FxKind::Hit,
                            k::PUMMEL_DAMAGE,
                            Vec2::new(facing, 0.0),
                            v as u8,
                        );
                    }
                }
            }
        }

        // 1b) Grab releases: a holder whose hold ran out let go this tick —
        //     shove the victim off with release lag.
        for i in 0..n {
            if let Some(v) = self.fighters[i].grabbed_by {
                let holder_let_go =
                    !matches!(self.fighters[v].state, State::Hold | State::Throw { .. })
                        || self.fighters[v].grabbing != Some(i);
                if holder_let_go && matches!(self.fighters[i].state, State::Grabbed) {
                    // The victim is held in front of the holder: shove it on.
                    let away = self.fighters[v].facing;
                    self.fighters[i].grab_release(away);
                }
            }
        }

        // 2) Spawn requested projectiles.
        for i in spawn_reqs {
            let f = &self.fighters[i];
            let pos = Vec2::new(
                f.pos.x + f.facing * 14.0,
                f.pos.y + f.character.height * 0.5,
            );
            self.projectiles.push(Projectile {
                pos,
                vel: Vec2::new(f.facing * attacks::PROJECTILE_SPEED, 0.0),
                facing: f.facing,
                life: attacks::PROJECTILE_LIFE,
                owner: i,
                active: true,
            });
        }

        // 3) Melee hitbox resolution + grabs + throws.
        self.resolve_combat(inputs);

        // 4) Keep grabbed fighters attached to their holders.
        self.update_grabs();

        // 5) Projectiles.
        self.update_projectiles(inputs);

        // 6) Shield regen for non-shielding fighters.
        for f in &mut self.fighters {
            if !f.is_shielding() {
                f.shield_health = (f.shield_health + k::SHIELD_REGEN).min(k::SHIELD_MAX);
            }
        }

        // 7) KO checks.
        self.check_kos();

        // 8) Match end: last one standing, or the clock running out.
        if self.match_over.is_none() {
            let alive: Vec<usize> = self
                .fighters
                .iter()
                .enumerate()
                .filter(|(_, f)| f.stocks > 0)
                .map(|(i, _)| i)
                .collect();
            if n >= 2 && alive.len() <= 1 {
                self.match_over = alive.first().copied().or(Some(usize::MAX));
            } else if let Some(limit) = self.config.time_limit_frames() {
                if self.frame + 1 >= limit {
                    self.match_over = Some(self.timeout_winner());
                }
            }
        }

        self.frame += 1;
    }

    /// Winner when the clock expires: most stocks, then lowest percent; a
    /// perfect tie is a draw (`usize::MAX`).
    fn timeout_winner(&self) -> usize {
        let mut best: Option<usize> = None;
        let mut tie = false;
        for (i, f) in self.fighters.iter().enumerate() {
            match best {
                None => best = Some(i),
                Some(b) => {
                    let bf = &self.fighters[b];
                    if f.stocks > bf.stocks || (f.stocks == bf.stocks && f.percent < bf.percent) {
                        best = Some(i);
                        tie = false;
                    } else if f.stocks == bf.stocks && f.percent == bf.percent {
                        tie = true;
                    }
                }
            }
        }
        if tie {
            usize::MAX
        } else {
            best.unwrap_or(usize::MAX)
        }
    }

    /// Seconds left on the clock, if the match is timed.
    pub fn time_left_secs(&self) -> Option<u64> {
        self.config.time_limit_frames().map(|limit| {
            let left = limit.saturating_sub(self.frame);
            left.div_ceil(super::constants::FPS as u64)
        })
    }

    /// Clank / priority between grounded attacks whose hitboxes touch this
    /// tick: close damages cancel both into a rebound, otherwise the weaker
    /// hitbox is simply cancelled and the stronger one goes on to land.
    fn resolve_clanks(&mut self) {
        let n = self.fighters.len();
        for i in 0..n {
            for j in (i + 1)..n {
                let (a, b) = (&self.fighters[i], &self.fighters[j]);
                let grounded_attack = |f: &Fighter| {
                    f.grounded && matches!(f.state, State::Attack { aerial: false, .. })
                };
                if !grounded_attack(a) || !grounded_attack(b) {
                    continue;
                }
                let (Some((ha, pa)), Some((hb, pb))) = (a.active_hitbox(), b.active_hitbox())
                else {
                    continue;
                };
                if (pa - pb).length() > ha.radius + hb.radius {
                    continue;
                }
                let mid = (pa + pb) * 0.5;
                let diff = (ha.damage - hb.damage).abs();
                if diff < k::CLANK_DIFF {
                    let total = (ha.damage.max(hb.damage) / 3.0) as u32 + k::REBOUND_BASE;
                    for (idx, other) in [(i, j), (j, i)] {
                        let f = &mut self.fighters[idx];
                        f.already_hit = true;
                        f.vel.x = -f.facing * 1.5;
                        f.set_state_pub(State::Rebound { total });
                        let _ = other;
                    }
                    self.push_fx_dir(
                        mid,
                        FxKind::Clank,
                        ha.damage.max(hb.damage),
                        Vec2::new(0.0, 1.0),
                    );
                    self.camera_shake = (self.camera_shake + 1.5).min(12.0);
                } else {
                    let weaker = if ha.damage < hb.damage { i } else { j };
                    self.fighters[weaker].already_hit = true;
                }
            }
        }
    }

    fn resolve_combat(&mut self, inputs: &[PlayerInput]) {
        self.resolve_clanks();
        let n = self.fighters.len();
        for i in 0..n {
            // Copy attacker's relevant snapshot to avoid overlapping borrows.
            let (state_i, frame_i, facing_i, grabbing_i) = {
                let a = &self.fighters[i];
                (a.state, a.state_frame, a.facing, a.grabbing)
            };

            // --- grab catch ---
            if self.fighters[i].grab_active() {
                let a = &self.fighters[i];
                let hand = Vec2::new(
                    a.pos.x + a.facing * k::GRAB_REACH,
                    a.pos.y + a.character.height * 0.5,
                );
                for j in 0..n {
                    if j == i {
                        continue;
                    }
                    let can = {
                        let v = &self.fighters[j];
                        v.can_be_hit()
                            && !matches!(v.state, State::Grabbed)
                            && !matches!(v.state, State::Hitstun { .. })
                            && v.grounded
                    };
                    if !can {
                        continue;
                    }
                    let vc = self.fighters[j].body_center();
                    if (vc - hand).length() < k::GRAB_REACH {
                        let pct = self.fighters[j].percent;
                        self.fighters[i].catch(j, pct);
                        self.fighters[j].set_grabbed(i);
                        break;
                    }
                }
                continue;
            }

            // --- throw release ---
            if let State::Throw { id } = state_i {
                let md = attacks::data(self.fighters[i].character.id, id);
                let released = md.is_active(frame_i);
                if released {
                    let already = self.fighters[i].already_hit;
                    if !already {
                        if let Some(j) = grabbing_i {
                            self.apply_hit(i, j, &md.hitbox, inputs, true, false, md.rearward);
                            self.fighters[i].already_hit = true;
                            self.fighters[i].grabbing = None;
                            self.fighters[j].grabbed_by = None;
                        }
                    }
                }
                continue;
            }

            // --- normal hitboxes ---
            let hb = match self.fighters[i].active_hitbox() {
                Some(h) => h,
                None => continue,
            };
            let (hitbox, hb_world) = hb;
            let hb_prev = self.fighters[i].active_hitbox_prev().unwrap_or(hb_world);
            let (electric, rearward) = match state_i {
                State::Attack { id, .. } => {
                    let md = attacks::data(self.fighters[i].character.id, id);
                    (md.electric, md.rearward)
                }
                _ => (false, false),
            };
            for j in 0..n {
                if j == i {
                    continue;
                }
                let victim = &self.fighters[j];
                if !victim.can_be_hit() {
                    continue;
                }
                // Swept sphere (this tick's travel) against the hurt capsule.
                let (h0, h1) = victim.hurt_segment();
                let dist = super::math::segment_distance(hb_prev, hb_world, h0, h1);
                if dist > hitbox.radius + victim.hurt_radius() {
                    continue;
                }

                // Shielding blocks if the attack comes from the front.
                let from_front = (hb_world.x - victim.pos.x) * victim.facing >= -2.0;
                if victim.shield_covers() && from_front {
                    let ps = self.fighters[j].shield_hit(hitbox.damage);
                    let vp = self.fighters[j].body_center();
                    if ps {
                        self.push_fx_who(
                            vp,
                            FxKind::Powershield,
                            hitbox.damage,
                            Vec2::new(facing_i, 0.0),
                            j as u8,
                        );
                        self.fighters[i].hitlag = knockback::hitlag(hitbox.damage, electric, false);
                    } else {
                        self.push_fx_who(
                            vp,
                            FxKind::Shield,
                            hitbox.damage,
                            Vec2::new(facing_i, 0.0),
                            j as u8,
                        );
                        let hl = knockback::hitlag(hitbox.damage, electric, false);
                        self.fighters[i].hitlag = hl;
                        // The attacker is nudged back a little too.
                        let push = knockback::shield_push(hitbox.damage) * 0.35;
                        if self.fighters[i].grounded {
                            self.fighters[i].vel.x -= facing_i * push;
                        }
                    }
                    self.fighters[i].already_hit = true;
                    break;
                }

                let stale_id = match state_i {
                    State::Attack { id, .. } => Some(id),
                    _ => None,
                };
                self.apply_hit_staled(i, j, &hitbox, inputs, false, electric, stale_id, rearward);
                self.fighters[i].already_hit = true;
                break;
            }
        }
    }

    /// A normal hit that also goes through the attacker's stale-move queue.
    #[allow(clippy::too_many_arguments)]
    fn apply_hit_staled(
        &mut self,
        i: usize,
        j: usize,
        hitbox: &attacks::Hitbox,
        inputs: &[PlayerInput],
        is_throw: bool,
        electric: bool,
        stale_id: Option<MoveId>,
        rearward: bool,
    ) {
        let mult = stale_id
            .map(|id| self.fighters[i].stale_multiplier(id))
            .unwrap_or(1.0);
        self.apply_hit_with(i, j, hitbox, inputs, is_throw, electric, mult, rearward);
        if let Some(id) = stale_id {
            self.fighters[i].stale_push(id);
        }
    }

    /// Apply a clean hit from attacker `i` onto victim `j`.
    #[allow(clippy::too_many_arguments)]
    fn apply_hit(
        &mut self,
        i: usize,
        j: usize,
        hitbox: &attacks::Hitbox,
        inputs: &[PlayerInput],
        is_throw: bool,
        electric: bool,
        rearward: bool,
    ) {
        self.apply_hit_with(i, j, hitbox, inputs, is_throw, electric, 1.0, rearward);
    }

    /// Apply a hit whose *damage* is scaled by `stale` (knockback still uses
    /// the fresh damage, as in the reference).
    #[allow(clippy::too_many_arguments)]
    fn apply_hit_with(
        &mut self,
        i: usize,
        j: usize,
        hitbox: &attacks::Hitbox,
        inputs: &[PlayerInput],
        is_throw: bool,
        electric: bool,
        stale: f32,
        rearward: bool,
    ) {
        let attacker_facing = self.fighters[i].facing;

        // Damage (staled, to a tenth), then knockback from the fresh damage.
        let victim_weight = self.fighters[j].character.weight;
        let dealt = (hitbox.damage * stale * 10.0).round() / 10.0;
        self.fighters[j].percent = (self.fighters[j].percent + dealt).min(999.0);
        self.fighters[j].last_hit_frame = self.frame;
        let percent_after = self.fighters[j].percent;

        // Knockback magnitude; crouch-cancelling (crouching on the ground)
        // takes a third off, and the victim's hitlag with it.
        let (victim_grounded, crouch_cancel) = {
            let v = &self.fighters[j];
            (
                v.grounded,
                v.grounded && matches!(v.state, State::Crouch) && !is_throw,
            )
        };
        let mut kb = knockback::knockback(
            percent_after,
            hitbox.damage,
            victim_weight,
            hitbox.kbg,
            hitbox.bkb,
        );
        if crouch_cancel {
            kb *= k::CROUCH_CANCEL;
        }

        // World launch angle (Sakurai angle resolved here; mirror by the
        // direction the move actually sends the victim). DI is applied by
        // the victim on the last hitlag frame (see
        // `Fighter::pending_launch`), not here.
        //
        // `rearward` moves (back throw, back air — declared per move in
        // `attacks`, never inferred from a hitbox offset) send the victim
        // behind the attacker, so the horizontal direction is the attacker's
        // back. Mirroring an angle about the vertical leaves `sin` alone, so
        // knockback magnitude and the whole vertical component are untouched
        // — and so is the attacker's own `facing`, which is read here but
        // never written.
        let launch_dir = if rearward {
            -attacker_facing
        } else {
            attacker_facing
        };
        let angle_deg = knockback::resolve_angle(hitbox.angle_deg, kb, victim_grounded);
        let mut angle = angle_deg.to_radians();
        if launch_dir < 0.0 {
            angle = std::f32::consts::PI - angle;
        }
        let _ = inputs;

        let hitlag_attacker = knockback::hitlag(hitbox.damage, electric, false);
        let hitlag_victim = knockback::hitlag(hitbox.damage, electric, crouch_cancel);

        // Super armour (e.g. Boulder's *Bulwark*): during smash startup, weak
        // knockback is absorbed — damage is taken, but no launch, no hitstun.
        let armored = {
            let v = &self.fighters[j];
            let armor = v.character.smash_armor;
            match v.state {
                State::Attack { id, aerial: false } if attacks::is_smash(id) => {
                    let startup = attacks::data(v.character.id, id).startup;
                    armor > 0.0 && !is_throw && kb < armor && v.state_frame < startup
                }
                _ => false,
            }
        };
        if armored {
            self.fighters[j].anim_flash = 6;
            self.fighters[j].hitlag = hitlag_victim / 2;
            self.fighters[i].hitlag = hitlag_attacker;
            let vp = self.fighters[j].body_center();
            self.push_fx(vp, FxKind::Shield, hitbox.damage);
            return;
        }

        let launch = knockback::launch_velocity(kb, angle);
        let hitstun = knockback::hitstun(kb).max(if is_throw { 8 } else { 4 });
        let tumble = knockback::causes_tumble(kb);

        self.fighters[j].apply_launch(launch, hitstun, tumble, hitlag_victim);
        if !self.fighters[j].ground_stun {
            self.fighters[j].pending_launch = Some((kb, angle));
        }
        // Spikes can be meteor-cancelled (angle measured before mirroring).
        self.fighters[j].meteor = !is_throw
            && !self.fighters[j].ground_stun
            && (k::METEOR_ANGLES.0..=k::METEOR_ANGLES.1).contains(&angle_deg);
        self.fighters[i].hitlag = hitlag_attacker;

        let vp = self.fighters[j].body_center();
        let dir = Vec2::new(angle.cos(), angle.sin());
        self.push_fx_who(vp, FxKind::Hit, hitbox.damage + kb * 0.15, dir, j as u8);
        // Camera: a small kick on every hit, a real shake only on big ones.
        let strong = (kb - 70.0).max(0.0) * 0.06;
        self.camera_shake = (self.camera_shake + hitbox.damage * 0.18 + strong).min(12.0);
        self.hitstop_flash = if kb > 90.0 { 1.0 } else { 0.55 };
    }

    fn update_grabs(&mut self) {
        let n = self.fighters.len();
        for i in 0..n {
            let (is_hold, grabbing, facing, pos, h) = {
                let a = &self.fighters[i];
                (
                    matches!(a.state, State::Hold | State::Throw { .. }),
                    a.grabbing,
                    a.facing,
                    a.pos,
                    a.character.height,
                )
            };
            if is_hold {
                if let Some(j) = grabbing {
                    if matches!(self.fighters[j].state, State::Grabbed) {
                        self.fighters[j].pos = Vec2::new(pos.x + facing * 16.0, pos.y);
                        self.fighters[j].vel = Vec2::ZERO;
                        let _ = h;
                    } else {
                        self.fighters[i].grabbing = None;
                    }
                }
            }
        }
    }

    fn update_projectiles(&mut self, inputs: &[PlayerInput]) {
        let n = self.fighters.len();
        for p in &mut self.projectiles {
            if !p.active {
                continue;
            }
            p.pos += p.vel;
            if p.life > 0 {
                p.life -= 1;
            }
            if p.life == 0 || self.stage.is_ko(p.pos) {
                p.active = false;
            }
        }

        // Collide projectiles with fighters. `approach` is the start of this
        // tick's swept travel — where the shot genuinely came from — and is
        // carried to the block test below so the defence is decided by the
        // projectile's own approach, never by where its owner happens to
        // stand or face by the time it lands.
        let mut hits: Vec<(usize, usize, f32)> = Vec::new(); // (proj, victim, approach x)
        for (pi, p) in self.projectiles.iter().enumerate() {
            if !p.active {
                continue;
            }
            for j in 0..n {
                if j == p.owner {
                    continue;
                }
                let v = &self.fighters[j];
                if !v.can_be_hit() {
                    continue;
                }
                let (h0, h1) = v.hurt_segment();
                let prev = p.pos - p.vel;
                if super::math::segment_distance(prev, p.pos, h0, h1)
                    < attacks::PROJECTILE_RADIUS + v.hurt_radius()
                {
                    hits.push((pi, j, prev.x));
                    break;
                }
            }
        }
        for (pi, j, approach_x) in hits {
            let owner_facing = self.projectiles[pi].facing;
            let hb = attacks::Hitbox {
                offset: Vec2::ZERO,
                radius: attacks::PROJECTILE_RADIUS,
                damage: attacks::PROJECTILE_DAMAGE,
                angle_deg: attacks::PROJECTILE_ANGLE,
                kbg: attacks::PROJECTILE_KBG,
                bkb: attacks::PROJECTILE_BKB,
            };
            let owner = self.projectiles[pi].owner;

            // A shot into a raised frontal shield is blocked by exactly the
            // rules a melee hit meets (`resolve_combat`): `shield_hit` takes
            // the shield health, the shieldstun and the defender's pushback,
            // or reports a powershield inside its window and takes neither.
            // The shot is spent on the shield — consumed once, no body
            // damage and no body launch — and the owner is not touched at
            // all: a thrown object neither freezes nor shoves its thrower.
            let blocked = {
                let v = &self.fighters[j];
                let from_front = (approach_x - v.pos.x) * v.facing >= -2.0;
                v.shield_covers() && from_front
            };
            if blocked {
                let powershielded = self.fighters[j].shield_hit(hb.damage);
                let vp = self.fighters[j].body_center();
                let kind = if powershielded {
                    FxKind::Powershield
                } else {
                    FxKind::Shield
                };
                self.push_fx_who(vp, kind, hb.damage, Vec2::new(owner_facing, 0.0), j as u8);
                self.projectiles[pi].active = false;
                continue;
            }

            // Reuse apply_hit with a synthetic attacker facing via the owner.
            let saved_facing = self.fighters[owner].facing;
            let saved_hitlag = self.fighters[owner].hitlag;
            self.fighters[owner].facing = owner_facing;
            self.apply_hit(owner, j, &hb, inputs, false, false, false);
            self.fighters[owner].facing = saved_facing;
            // A projectile never freezes its owner — and never *thaws* one
            // either: unrelated hitlag the owner was already in (someone
            // else's hit, a pummel) survives its shot connecting downrange.
            self.fighters[owner].hitlag = saved_hitlag;
            self.projectiles[pi].active = false;
        }
        self.projectiles.retain(|p| p.active);
    }

    fn check_kos(&mut self) {
        let n = self.fighters.len();
        for i in 0..n {
            let ko = {
                let f = &self.fighters[i];
                !matches!(f.state, State::Dead) && self.stage.is_ko(f.pos)
            };
            if ko {
                let bp = self.fighters[i].pos;
                self.push_fx_who(bp, FxKind::Blast, 12.0, Vec2::new(0.0, 1.0), i as u8);
                self.camera_shake = (self.camera_shake + 10.0).min(16.0);
                let f = &mut self.fighters[i];
                f.stocks -= 1;
                f.grabbing = None;
                f.grabbed_by = None;
                f.state = State::Dead;
                f.state_frame = 0;
                f.vel = Vec2::ZERO;
                f.respawn_timer = if f.stocks > 0 { 40 } else { u32::MAX };
            }
        }
    }

    fn push_fx(&mut self, pos: Vec2, kind: FxKind, magnitude: f32) {
        self.push_fx_dir(pos, kind, magnitude, Vec2::new(1.0, 0.0));
    }

    fn push_fx_dir(&mut self, pos: Vec2, kind: FxKind, magnitude: f32, dir: Vec2) {
        self.push_fx_who(pos, kind, magnitude, dir, u8::MAX);
    }

    fn push_fx_who(&mut self, pos: Vec2, kind: FxKind, magnitude: f32, dir: Vec2, who: u8) {
        let life = match kind {
            FxKind::Hit => 12,
            FxKind::Shield => 8,
            FxKind::Powershield => 10,
            FxKind::Clank => 9,
            FxKind::Blast => 22,
            FxKind::Dust => 14,
            FxKind::Land | FxKind::Jump | FxKind::Swing | FxKind::Tech => 2,
        };
        if self.fx.len() < 64 {
            self.fx.push(Fx {
                pos,
                life,
                max_life: life,
                kind,
                magnitude: clampf(magnitude, 1.0, 40.0),
                dir,
                born: self.frame,
                who,
            });
        }
    }
}

/// FNV-1a over the simulation's fields, for [`GameState::checksum`].
struct Fnv(u128);

impl Fnv {
    const PRIME: u128 = 0x0000_0000_0100_0000_0000_0000_0000_013b;

    fn new() -> Self {
        Fnv(0x6c62_272e_07bb_0142_62b8_2175_6295_c58d)
    }
    #[inline]
    fn byte(&mut self, b: u8) {
        self.0 ^= b as u128;
        self.0 = self.0.wrapping_mul(Self::PRIME);
    }
    #[inline]
    fn u32(&mut self, v: u32) {
        for b in v.to_le_bytes() {
            self.byte(b);
        }
    }
    #[inline]
    fn u64(&mut self, v: u64) {
        for b in v.to_le_bytes() {
            self.byte(b);
        }
    }
    /// `f32` by its bits, with every NaN normalised so two NaNs never look
    /// different (and `-0.0` never differs from `0.0`).
    #[inline]
    fn f32(&mut self, v: f32) {
        let bits = if v.is_nan() {
            0x7fc0_0000
        } else if v == 0.0 {
            0
        } else {
            v.to_bits()
        };
        self.u32(bits);
    }
    #[inline]
    fn vec2(&mut self, v: Vec2) {
        self.f32(v.x);
        self.f32(v.y);
    }
    #[inline]
    fn bool(&mut self, v: bool) {
        self.byte(u8::from(v));
    }
    #[inline]
    fn opt_usize(&mut self, v: Option<usize>) {
        match v {
            Some(i) => {
                self.byte(1);
                self.u64(i as u64);
            }
            None => self.byte(0),
        }
    }
    fn finish(self) -> u128 {
        self.0
    }
}
