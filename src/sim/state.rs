//! The top-level game state and the single pure `step` function.
//!
//! `GameState::step(&mut self, inputs)` advances the whole match by exactly one
//! 60 Hz tick. It reads nothing but its own fields and the inputs, so it is
//! deterministic and fully save/load-able — which is precisely what the GGRS
//! rollback layer (see `netcode.rs`) requires.

use super::attacks::{self, Projectile};
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

    /// A deterministic checksum of the gameplay-relevant state. Used by the
    /// GGRS `SyncTest` to prove that save→load→re-simulate reproduces the exact
    /// same state (i.e. the simulation is rollback-safe).
    pub fn checksum(&self) -> u128 {
        // FNV-1a over the meaningful fields.
        let mut h: u128 = 0x6c62_272e_07bb_0142_62b8_2175_6295_c58d;
        let prime: u128 = 0x0000_0000_0100_0000_0000_0000_0000_013b;
        let mix = |v: u32, h: &mut u128| {
            for b in v.to_le_bytes() {
                *h ^= b as u128;
                *h = h.wrapping_mul(prime);
            }
        };
        mix(self.frame as u32, &mut h);
        for f in &self.fighters {
            mix(f.pos.x.to_bits(), &mut h);
            mix(f.pos.y.to_bits(), &mut h);
            mix(f.vel.x.to_bits(), &mut h);
            mix(f.vel.y.to_bits(), &mut h);
            mix(f.percent.to_bits(), &mut h);
            mix(f.facing.to_bits(), &mut h);
            mix(f.stocks as u32, &mut h);
            mix(f.state_frame, &mut h);
            mix(f.intangible, &mut h);
            mix(f.hitstun_timer, &mut h);
        }
        for p in &self.projectiles {
            mix(p.pos.x.to_bits(), &mut h);
            mix(p.pos.y.to_bits(), &mut h);
            mix(p.life, &mut h);
        }
        h
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
        for i in 0..n {
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

    fn resolve_combat(&mut self, inputs: &[PlayerInput]) {
        let n = self.fighters.len();
        for i in 0..n {
            // Copy attacker's relevant snapshot to avoid overlapping borrows.
            let (state_i, frame_i, facing_i, grabbing_i) = {
                let a = &self.fighters[i];
                (a.state, a.state_frame, a.facing, a.grabbing)
            };

            // --- grab catch ---
            if matches!(state_i, State::Grab) && (6..=11).contains(&frame_i) {
                let a = &self.fighters[i];
                let hand = Vec2::new(
                    a.pos.x + a.facing * 16.0,
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
                    if (vc - hand).length() < 16.0 {
                        self.fighters[i].catch(j);
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
                            self.apply_hit(i, j, &md.hitbox, inputs, true, false);
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
            let electric = match state_i {
                State::Attack { id, .. } => {
                    attacks::data(self.fighters[i].character.id, id).electric
                }
                _ => false,
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
                if victim.is_shielding() && from_front {
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

                self.apply_hit(i, j, &hitbox, inputs, false, electric);
                self.fighters[i].already_hit = true;
                break;
            }
        }
    }

    /// Apply a clean hit from attacker `i` onto victim `j`.
    fn apply_hit(
        &mut self,
        i: usize,
        j: usize,
        hitbox: &attacks::Hitbox,
        inputs: &[PlayerInput],
        is_throw: bool,
        electric: bool,
    ) {
        let attacker_facing = self.fighters[i].facing;

        // Damage.
        let victim_weight = self.fighters[j].character.weight;
        self.fighters[j].percent = (self.fighters[j].percent + hitbox.damage).min(999.0);
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

        // World launch angle (Sakurai angle resolved here; mirror by facing).
        // DI is applied by the victim on the last hitlag frame (see
        // `Fighter::pending_launch`), not here.
        let angle_deg = knockback::resolve_angle(hitbox.angle_deg, kb, victim_grounded);
        let mut angle = angle_deg.to_radians();
        if attacker_facing < 0.0 {
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

        // Collide projectiles with fighters.
        let mut hits: Vec<(usize, usize)> = Vec::new(); // (proj idx, victim)
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
                    hits.push((pi, j));
                    break;
                }
            }
        }
        for (pi, j) in hits {
            let owner_facing = self.projectiles[pi].facing;
            let hb = attacks::Hitbox {
                offset: Vec2::ZERO,
                radius: attacks::PROJECTILE_RADIUS,
                damage: attacks::PROJECTILE_DAMAGE,
                angle_deg: attacks::PROJECTILE_ANGLE,
                kbg: attacks::PROJECTILE_KBG,
                bkb: attacks::PROJECTILE_BKB,
            };
            // Reuse apply_hit with a synthetic attacker facing via the owner.
            let owner = self.projectiles[pi].owner;
            let saved_facing = self.fighters[owner].facing;
            self.fighters[owner].facing = owner_facing;
            self.apply_hit(owner, j, &hb, inputs, false, false);
            self.fighters[owner].facing = saved_facing;
            self.fighters[owner].hitlag = 0; // projectile owner doesn't freeze
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
