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
use super::stage::Stage;

/// Match rules.
#[derive(Clone, Copy, Debug)]
pub struct MatchConfig {
    pub stocks: i32,
    pub seed: u32,
}

impl Default for MatchConfig {
    fn default() -> Self {
        MatchConfig {
            stocks: 4,
            seed: 0x1234_5678,
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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FxKind {
    Hit,
    Shield,
    Blast,
    Dust,
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
    /// Build a fresh match with `num_players` Kestrels on The Lattice.
    pub fn new(num_players: usize, config: MatchConfig) -> Self {
        let stage = Stage::lattice();
        let mut fighters = Vec::with_capacity(num_players);
        for i in 0..num_players {
            let spawn = stage.spawns[i % stage.spawns.len()];
            let mut f = Fighter::new(&k::KESTREL, i, Vec2::new(spawn.x, spawn.y + 40.0));
            f.stocks = config.stocks;
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

        // 8) Match end.
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
            }
        }

        self.frame += 1;
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
                let md = attacks::data(id);
                let released = md.is_active(frame_i);
                if released {
                    let already = self.fighters[i].already_hit;
                    if !already {
                        if let Some(j) = grabbing_i {
                            self.apply_hit(i, j, &md.hitbox, inputs, true);
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
            for j in 0..n {
                if j == i {
                    continue;
                }
                let victim = &self.fighters[j];
                if !victim.can_be_hit() {
                    continue;
                }
                let vc = victim.body_center();
                let dist = (vc - hb_world).length();
                if dist > hitbox.radius + victim.hurt_radius() {
                    continue;
                }

                // Shielding blocks if the attack comes from the front.
                let from_front = (hb_world.x - victim.pos.x) * victim.facing >= -2.0;
                if victim.is_shielding() && from_front {
                    let push = 0.6 + hitbox.damage * 0.05;
                    self.fighters[j].shield_hit(hitbox.damage, push);
                    let vp = self.fighters[j].body_center();
                    self.push_fx(vp, FxKind::Shield, hitbox.damage);
                    self.fighters[i].already_hit = true;
                    let hl = knockback::hitlag(hitbox.damage);
                    self.fighters[i].hitlag = hl;
                    break;
                }

                self.apply_hit(i, j, &hitbox, inputs, false);
                self.fighters[i].already_hit = true;
                break;
            }
            let _ = facing_i;
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
    ) {
        let attacker_facing = self.fighters[i].facing;

        // Damage.
        let victim_weight = self.fighters[j].character.weight;
        self.fighters[j].percent = (self.fighters[j].percent + hitbox.damage).min(999.0);
        let percent_after = self.fighters[j].percent;

        // Knockback magnitude.
        let kb = knockback::knockback(
            percent_after,
            hitbox.damage,
            victim_weight,
            hitbox.kbg,
            hitbox.bkb,
        );

        // World launch angle (mirror by attacker facing).
        let mut angle = hitbox.angle_deg.to_radians();
        if attacker_facing < 0.0 {
            angle = std::f32::consts::PI - angle;
        }
        // Victim DI.
        let di_stick = inputs.get(j).copied().unwrap_or_default().stick;
        let angle = knockback::apply_di(angle, di_stick);

        let launch = knockback::launch_velocity(kb, angle);
        let hitstun = knockback::hitstun(kb).max(if is_throw { 8 } else { 4 });
        let tumble = knockback::causes_tumble(kb);
        let hitlag = knockback::hitlag(hitbox.damage);

        self.fighters[j].apply_launch(launch, hitstun, tumble, hitlag);
        self.fighters[i].hitlag = hitlag;

        let vp = self.fighters[j].body_center();
        self.push_fx(vp, FxKind::Hit, hitbox.damage);
        self.camera_shake = (self.camera_shake + hitbox.damage * 0.35 + kb * 0.02).min(14.0);
        self.hitstop_flash = 1.0;
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
                if (v.body_center() - p.pos).length() < attacks::PROJECTILE_RADIUS + v.hurt_radius()
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
            self.apply_hit(owner, j, &hb, inputs, false);
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
                self.push_fx(bp, FxKind::Blast, 12.0);
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
        let life = match kind {
            FxKind::Hit => 10,
            FxKind::Shield => 8,
            FxKind::Blast => 22,
            FxKind::Dust => 12,
        };
        if self.fx.len() < 64 {
            self.fx.push(Fx {
                pos,
                life,
                max_life: life,
                kind,
                magnitude: clampf(magnitude, 1.0, 30.0),
            });
        }
    }
}
