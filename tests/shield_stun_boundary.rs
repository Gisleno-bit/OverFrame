//! Does a raised shield still cover its owner during the shieldstun of a
//! block it survived -- and what happens on the tick that stun ends?
//!
//! `Fighter::is_shielding()` is true only for `State::Shield`, while
//! `can_be_hit()` allows `State::ShieldStun`. Both block sites -- the melee
//! one in `resolve_combat` and the projectile one in `update_projectiles`
//! -- gate on the same predicate, so the first question is whether a second
//! frontal impact arriving inside the first block's shieldstun goes through
//! the shield and hits the body. The second question is the seam on the way
//! out: the stun ends by entering `State::Shield` with `state_frame` reset
//! to 0, which is exactly what the powershield window reads.
//!
//! Everything here is demonstrated with real impacts from real attacks. The
//! defender's state is never assigned by hand, and the age it was in when a
//! hit resolved is **measured**, not computed: the same bout is replayed
//! with the studied attack removed, and the defender's state after that
//! tick is what the impact met. No formula is duplicated from the
//! simulation to decide what should have happened.
//!
//! The file is deliberately written against behaviour only -- no new field
//! or method is named anywhere -- so the *same bytes* compile and run on
//! both sides of the fix.
//!
//! The trap this must not fall into: a **shield break** uses the same
//! `State::ShieldStun` as an ordinary block, and the flag that says which
//! is which is written at *two* sites (broken by damage, broken by holding
//! it too long). Both are exercised, and both are followed through to the
//! recovery on the far side.
//!
//! Run with: `cargo test --test shield_stun_boundary -- --nocapture`

use overframe::sim::attacks::{self, MoveId};
use overframe::sim::constants as k;
use overframe::sim::fighter::State;
use overframe::sim::knockback;
use overframe::sim::math::Vec2;
use overframe::sim::roster::CharacterId;
use overframe::sim::{buttons, FxKind, GameState, MatchConfig, PlayerInput};

// --------------------------------------------------------------- geometry

/// The defender's port. Ports 0, 2 and 3 are attackers.
const DEF: usize = 1;

/// Ticks the defender holds shield before anything is thrown at it, so the
/// first impact meets a settled shield well outside the powershield window.
const PREWARM: u32 = 10;
const _: () = assert!(PREWARM > k::POWERSHIELD_WINDOW);

/// Offsets along the attack axis, signed: **negative is the side the
/// defender faces** (its front), positive is its back.
const FAR: f32 = -90.0;
const MID: f32 = -55.0;
const SMASH_RANGE: f32 = -26.0;
const JAB_RANGE: f32 = -20.0;
/// How far in front of the defender's centre a swept attacker's hitbox is
/// planted: comfortably inside the hurt capsule, comfortably on the front
/// side of the `from_front` test.
const FRONTAL_BITE: f32 = 10.0;
/// The same, on the other side: far enough past the defender's centre that
/// the hitbox is unambiguously behind it.
const REAR_BITE: f32 = 10.0;
/// Where an unused attacker stands: behind the defender, doing nothing.
const IDLE: f32 = 60.0;

fn smash_damage() -> f32 {
    attacks::data(CharacterId::Kestrel, MoveId::Fsmash)
        .hitbox
        .damage
}
fn jab_damage() -> f32 {
    attacks::data(CharacterId::Kestrel, MoveId::Jab)
        .hitbox
        .damage
}

/// The tick a move pressed on `press_tick` first connects: a move's first
/// active frame is `startup - 1` ticks after the press.
fn swing_impact_tick(press_tick: u32, id: MoveId) -> u32 {
    press_tick + attacks::data(CharacterId::Kestrel, id).startup - 1
}

// ----------------------------------------------------------------- inputs

fn neutral() -> PlayerInput {
    PlayerInput::default()
}
fn press(b: u16) -> PlayerInput {
    PlayerInput {
        buttons: b,
        ..Default::default()
    }
}
fn cstick(x: f32) -> PlayerInput {
    PlayerInput {
        cstick: Vec2::new(x, 0.0),
        ..Default::default()
    }
}

/// What an attacker does on a given tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Act {
    /// Press SPECIAL: Kestrel's SpecialN releases its shot at once.
    Fire,
    /// C-stick forward: a forward smash.
    Smash,
    /// Press ATTACK with a neutral stick: a jab.
    Jab,
}

/// One attacker: where it stands, which way it looks, and what it does.
/// `face` is `+1` to look towards growing offsets and `-1` the other way.
#[derive(Clone, Debug)]
struct Plan {
    port: usize,
    offset: f32,
    face: f32,
    script: Vec<(u32, Act)>,
}

/// An attacker that looks back towards the defender's starting place.
fn plan(port: usize, offset: f32, script: &[(u32, Act)]) -> Plan {
    let face = if offset < 0.0 { 1.0 } else { -1.0 };
    plan_facing(port, offset, face, script)
}

/// An attacker whose facing is stated outright -- needed once the defender
/// has been pushed past where it started.
fn plan_facing(port: usize, offset: f32, face: f32, script: &[(u32, Act)]) -> Plan {
    Plan {
        port,
        offset,
        face,
        script: script.to_vec(),
    }
}

/// The same attackers, standing in the same places, but only the listed
/// ports still act. Used to replay a bout without the attack under study.
fn only(plans: &[Plan], keep: &[usize]) -> Vec<Plan> {
    plans
        .iter()
        .map(|p| Plan {
            port: p.port,
            offset: p.offset,
            face: p.face,
            script: if keep.contains(&p.port) {
                p.script.clone()
            } else {
                Vec::new()
            },
        })
        .collect()
}

/// When the defender holds SHIELD.
#[derive(Clone, Copy, Debug)]
enum Hold {
    Always,
    Never,
    /// Nothing until this tick, then held from it on -- a genuine new raise.
    From(u32),
    /// Held, except released for `[a, b)`.
    Except(u32, u32),
}

impl Hold {
    fn holding(self, tick: u32) -> bool {
        match self {
            Hold::Always => true,
            Hold::Never => false,
            Hold::From(t) => tick >= t,
            Hold::Except(a, b) => !(tick >= a && tick < b),
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Defence {
    /// Shield health to start with; `None` is a full shield.
    health: Option<f32>,
    hold: Hold,
    /// The defender faces the attackers' side (`true`) or turns its back.
    face_front: bool,
}

impl Default for Defence {
    fn default() -> Self {
        Defence {
            health: None,
            hold: Hold::Always,
            face_front: true,
        }
    }
}

// ---------------------------------------------------------------- records

/// What caused an impact, derived from the simulation itself: a projectile
/// that was consumed this tick, or an attacker whose hitbox was spent on
/// this tick. Never assigned by the fixture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Source {
    Melee(usize),
    Projectile(usize),
    Unattributed,
}

/// What the defender looked like around the tick an impact resolved.
#[derive(Clone, Debug)]
struct Impact {
    tick: u32,
    source: Source,
    /// Projectiles consumed by a collision on this tick.
    consumed: usize,
    /// The defender's state and age going *into* the tick.
    state_before: State,
    age_before: u32,
    /// The state and age the impact actually met, measured by replaying the
    /// same bout without this attack. `Stand`/0 until filled in.
    state_at_resolution: State,
    age_at_resolution: u32,
    state_after: State,
    shield_before: f32,
    shield_after: f32,
    percent_before: f32,
    percent_after: f32,
    vel: Vec2,
    hitlag: u32,
    pending_launch: bool,
    hit_fx: bool,
    shield_fx: bool,
    power_fx: bool,
    defender_facing: f32,
    /// `(approach x - defender x) * defender facing`: positive is the side
    /// the defender faces, negative is its back. For a projectile the
    /// approach is the start of the shot's travel this tick; for a melee it
    /// is the attacker's own position.
    approach_side: f32,
}

impl Impact {
    fn blocked(&self) -> bool {
        self.percent_after == self.percent_before
    }
    fn with_resolution(mut self, m: (State, u32)) -> Self {
        self.state_at_resolution = m.0;
        self.age_at_resolution = m.1;
        self
    }
    fn line(&self, label: &str) -> String {
        format!(
            "[BOUNDARY] {label} tick={:>3} src={:?} consumed={} | entered {:?} age {} | MET {:?} age {} | left {:?} | shield {:.3}->{:.3} | percent {}->{} | vel=({:+.3},{:+.3}) hitlag={} launch={} | fx hit={} shield={} power={} | side={:+.1} | BLOCKED={}",
            self.tick,
            self.source,
            self.consumed,
            self.state_before,
            self.age_before,
            self.state_at_resolution,
            self.age_at_resolution,
            self.state_after,
            self.shield_before,
            self.shield_after,
            self.percent_before,
            self.percent_after,
            self.vel.x,
            self.vel.y,
            self.hitlag,
            self.pending_launch,
            self.hit_fx,
            self.shield_fx,
            self.power_fx,
            self.approach_side,
            self.blocked()
        )
    }
}

/// Collects failed expectations instead of aborting on the first one, so a
/// run reports **every** facing and every case it checked.
#[derive(Default)]
struct Failures {
    what: String,
    items: Vec<String>,
}

impl Failures {
    fn new(what: &str) -> Self {
        Failures {
            what: what.to_string(),
            items: Vec::new(),
        }
    }
    fn check(&mut self, ok: bool, msg: impl FnOnce() -> String) -> bool {
        if !ok {
            self.items.push(msg());
        }
        ok
    }
    fn fail(&mut self, msg: String) {
        self.items.push(msg);
    }
    fn finish(self) {
        if !self.items.is_empty() {
            panic!(
                "{}: {} failed expectation(s)\n  - {}",
                self.what,
                self.items.len(),
                self.items.join("\n  - ")
            );
        }
    }
}

fn near(a: f32, b: f32, tol: f32) -> bool {
    (a - b).abs() <= tol
}

/// Tolerance for a single f32 add/subtract chain on shield health and
/// velocity. Not a behaviour tolerance: the expected values are exact.
const EPS: f32 = 1e-3;

// ------------------------------------------------------------------ bout

/// Four fighters. The defender (port `DEF`) stands in the middle; the other
/// three stand where their plan puts them, on the defender's front or back
/// side, and are kept untouchable so that they never intercept anything
/// aimed at the defender and never fight each other. Nothing about the
/// defender is altered by that.
///
/// `facing` is the axis sign: with `facing = +1` the defender's front is
/// towards smaller x, with `-1` towards larger x. Every case runs both.
struct Bout {
    gs: GameState,
    defence: Defence,
    plans: Vec<Plan>,
    impacts: Vec<Impact>,
    /// `(tick, state, age, shield health, percent, x)` after every tick.
    trace: Vec<(u32, State, u32, f32, f32, f32)>,
    shots: usize,
}

impl Bout {
    fn new(facing: f32, defence: Defence, plans: &[Plan]) -> Self {
        let cfg = MatchConfig {
            chars: [CharacterId::Kestrel; 4],
            ..Default::default()
        };
        let mut gs = GameState::new(4, cfg);
        for _ in 0..90 {
            gs.step(&[neutral(), neutral(), neutral(), neutral()]);
        }
        let d = 20.0 * facing;
        gs.fighters[DEF].pos = Vec2::new(d, 0.0);
        gs.fighters[DEF].facing = if defence.face_front { -facing } else { facing };
        for port in [0usize, 2, 3] {
            let here = plans.iter().find(|p| p.port == port);
            let offset = here.map(|p| p.offset).unwrap_or(IDLE);
            let face = here.map(|p| p.face).unwrap_or(-1.0);
            gs.fighters[port].pos = Vec2::new(d + offset * facing, 0.0);
            gs.fighters[port].facing = face * facing;
        }
        for f in gs.fighters.iter_mut() {
            f.intangible = 0;
        }
        if let Some(h) = defence.health {
            gs.fighters[DEF].shield_health = h;
        }
        Bout {
            gs,
            defence,
            plans: plans.to_vec(),
            impacts: Vec::new(),
            trace: Vec::new(),
            shots: 0,
        }
    }

    fn step(&mut self, tick: u32) {
        let n = self.gs.fighters.len();
        let d = &self.gs.fighters[DEF];
        let state_before = d.state;
        let age_before = d.state_frame;
        let shield_before = d.shield_health;
        let percent_before = d.percent;
        let def_pos = d.pos;
        let def_facing = d.facing;

        let mut inputs = vec![neutral(); n];
        inputs[DEF] = if self.defence.hold.holding(tick) {
            press(buttons::SHIELD)
        } else {
            neutral()
        };
        for p in &self.plans {
            for &(t, act) in &p.script {
                if t != tick {
                    continue;
                }
                inputs[p.port] = match act {
                    Act::Fire => {
                        self.shots += 1;
                        press(buttons::SPECIAL)
                    }
                    Act::Smash => cstick(self.gs.fighters[p.port].facing),
                    Act::Jab => press(buttons::ATTACK),
                };
            }
        }

        // The attackers are targets for nobody: keeping them untouchable
        // stops a near attacker from eating a shot aimed at the defender and
        // stops two attackers from trading with each other. It changes
        // nothing about the defender, which is the only thing measured.
        for i in 0..n {
            if i != DEF {
                self.gs.fighters[i].intangible = 600;
            }
        }

        // Snapshots for attribution.
        let hit_before: Vec<bool> = self.gs.fighters.iter().map(|f| f.already_hit).collect();
        let proj_before: Vec<(usize, u32, Vec2)> = self
            .gs
            .projectiles
            .iter()
            .filter(|p| p.active)
            .map(|p| (p.owner, p.life, p.pos))
            .collect();
        // Effects are diffed by the frame they were born on, not by the
        // length of the list: expiring effects are dropped from it, so an
        // index taken before the tick does not survive it.
        let frame_before = self.gs.frame;

        self.gs.step(&inputs);

        let d = &self.gs.fighters[DEF];
        let percent_after = d.percent;
        let shield_after = d.shield_health;
        let state_after = d.state;
        self.trace.push((
            tick,
            state_after,
            d.state_frame,
            shield_after,
            percent_after,
            d.pos.x,
        ));

        // Effects raised for the defender specifically.
        let mut hit_fx = false;
        let mut shield_fx = false;
        let mut power_fx = false;
        for fx in self.gs.fx.iter() {
            if fx.born < frame_before || fx.who as usize != DEF {
                continue;
            }
            match fx.kind {
                FxKind::Hit => hit_fx = true,
                FxKind::Shield => shield_fx = true,
                FxKind::Powershield => power_fx = true,
                _ => {}
            }
        }

        // A shot that is gone from the list while it still had life to spare
        // was spent on a collision -- and the defender is the only fighter
        // here that can be collided with. Shots are matched by owner and by
        // the one tick of life they lose, not by index: the simulation drops
        // spent shots out of the list, so indices do not survive a tick.
        let mut survivors: Vec<(usize, u32)> = self
            .gs
            .projectiles
            .iter()
            .map(|p| (p.owner, p.life))
            .collect();
        let mut consumed = 0usize;
        let mut proj_source = None;
        let mut approach = None;
        for &(owner, life, pos_before) in &proj_before {
            if let Some(idx) = survivors
                .iter()
                .position(|&(o, l)| o == owner && l + 1 == life)
            {
                survivors.remove(idx);
                continue;
            }
            if life > 1 {
                consumed += 1;
                proj_source = Some(owner);
                approach = Some(pos_before.x);
            }
        }

        // An attacker whose hitbox was spent this tick.
        let mut melee_source = None;
        for (i, &was_spent) in hit_before.iter().enumerate().take(n) {
            if i == DEF {
                continue;
            }
            if !was_spent && self.gs.fighters[i].already_hit {
                melee_source = Some(i);
                approach = approach.or(Some(self.gs.fighters[i].pos.x));
            }
        }

        let source = match (melee_source, proj_source) {
            (Some(i), None) => Source::Melee(i),
            (None, Some(o)) => Source::Projectile(o),
            _ => Source::Unattributed,
        };

        // The ordinary holding drain is not an impact; a block or a body hit
        // is. A block costs strictly more than the drain.
        let took_shield = shield_before - shield_after > k::SHIELD_DECAY + 1e-4;
        let took_body = percent_after > percent_before;
        if took_shield || took_body || power_fx {
            self.impacts.push(Impact {
                tick,
                source,
                consumed,
                state_before,
                age_before,
                state_at_resolution: State::Stand,
                age_at_resolution: 0,
                state_after,
                shield_before,
                shield_after,
                percent_before,
                percent_after,
                vel: d.vel,
                hitlag: d.hitlag,
                pending_launch: d.pending_launch.is_some(),
                hit_fx,
                shield_fx,
                power_fx,
                defender_facing: def_facing,
                approach_side: (approach.unwrap_or(def_pos.x) - def_pos.x) * def_facing,
            });
        }
    }
}

/// Run a whole bout and return its impacts and trace.
fn run(facing: f32, defence: Defence, plans: &[Plan], ticks: u32) -> Bout {
    let mut b = Bout::new(facing, defence, plans);
    for tick in 0..ticks {
        b.step(tick);
    }
    b
}

/// The defender's real state and age when combat resolved on `tick`,
/// **measured**: the same bout is replayed with only `keep`'s attacks in it,
/// so nothing touches the defender on that tick and the state left at the
/// end of it is exactly the state the removed impact met.
fn met_on(
    facing: f32,
    defence: Defence,
    plans: &[Plan],
    keep: &[usize],
    tick: u32,
) -> (State, u32) {
    let reduced = only(plans, keep);
    let b = run(facing, defence, &reduced, tick + 1);
    let last = b
        .trace
        .last()
        .copied()
        .expect("the witness bout ran at least one tick");
    assert_eq!(last.0, tick, "witness replay stopped on the wrong tick");
    (last.1, last.2)
}

// -------------------------------------------------------------- measuring

/// Find the press tick that makes the next impact after `before` land
/// inside `[lo, hi]`, by **running the bout** for each candidate. `before`
/// is the list of ticks the impacts already in the fixture land on, so a
/// candidate that gets in front of one of them is rejected.
///
/// A flight time measured in some other bout would be a guess here: a shield
/// that blocks is pushed backwards, so the gap a later shot has to cross is
/// not the gap the first one crossed, and one tick of delay can move the
/// arrival by two. Searching the real fixture keeps the aim honest and makes
/// an unreachable tick a loud failure with the table of what was tried.
fn aim<F>(facing: f32, defence: Defence, build: F, before: &[u32], lo: u32, hi: u32) -> (u32, u32)
where
    F: Fn(u32) -> Vec<Plan>,
{
    let want = before.len();
    let horizon = hi + 8;
    let mut tried = Vec::new();
    for fire in 0..=hi {
        let b = run(facing, defence, &build(fire), horizon);
        let ticks: Vec<u32> = b.impacts.iter().map(|i| i.tick).collect();
        // Everything that was already landing must still land where it did,
        // so a candidate press that overtakes an earlier attack is rejected
        // instead of quietly renumbering the impacts.
        if ticks.len() <= want || ticks[..want] != *before {
            tried.push(format!("press {fire} -> impacts {ticks:?}"));
            continue;
        }
        if ticks[want] >= lo && ticks[want] <= hi {
            return (fire, ticks[want]);
        }
        tried.push(format!("press {fire} -> impacts {ticks:?}"));
    }
    panic!(
        "facing {facing:+}: no press tick lands impact #{want} in [{lo},{hi}] after {before:?}:\n  {}",
        tried.join("\n  ")
    );
}

/// Where the defender really stands when combat resolves on `tick`, in
/// units along the attack axis measured from where it started: the shield
/// is pushed backwards by every block, so an attacker that must reach it
/// has to be placed against the position it will actually be in.
fn defender_drift(facing: f32, defence: Defence, plans: &[Plan], keep: &[usize], tick: u32) -> f32 {
    let b = run(facing, defence, &only(plans, keep), tick + 1);
    let last = b.trace.last().copied().expect("the witness bout ran");
    assert_eq!(last.0, tick, "witness replay stopped on the wrong tick");
    last.5 / facing - 20.0
}

/// The tick the defender's shieldstun really ends, measured: the same bout is
/// replayed with only `keep`'s attacks in it, and this is the first tick
/// after `after` on which the replay shows it holding a shield again.
fn seam_after(
    facing: f32,
    defence: Defence,
    plans: &[Plan],
    keep: &[usize],
    after: u32,
    horizon: u32,
) -> u32 {
    let b = run(facing, defence, &only(plans, keep), horizon);
    b.trace
        .iter()
        .find(|(t, s, _, _, _, _)| *t > after && matches!(s, State::Shield))
        .map(|(t, _, _, _, _, _)| *t)
        .unwrap_or_else(|| {
            panic!("facing {facing:+}: the shieldstun never ended within {horizon} ticks")
        })
}

// ------------------------------------------------------------- assertions

/// Everything a *blocked* impact must be: no body damage, no launch, no
/// body hitlag, no vertical motion, the exact shield cost and the exact
/// shield pushback the tables give for that damage, the block effect and
/// only the block effect, the stun the tables give, and -- for a shot --
/// exactly one projectile spent.
#[allow(clippy::too_many_arguments)]
fn check_blocked_properly(f: &mut Failures, tag: &str, im: &Impact, damage: f32) {
    let l = im.line(tag);
    f.check(im.blocked(), || {
        format!("{tag}: the impact went through the shield and hit the body -- {l}")
    });
    f.check(!im.pending_launch, || {
        format!("{tag}: a blocked impact must not queue a launch -- {l}")
    });
    f.check(im.hitlag == 0, || {
        format!(
            "{tag}: a blocked impact must leave the defender's body hitlag at 0, got {} -- {l}",
            im.hitlag
        )
    });
    f.check(im.vel.y == 0.0, || {
        format!(
            "{tag}: a blocked impact must not move the defender vertically, vel.y={} -- {l}",
            im.vel.y
        )
    });
    f.check(!im.hit_fx, || {
        format!("{tag}: a blocked impact must not raise a Hit effect -- {l}")
    });
    f.check(!im.power_fx, || {
        format!("{tag}: this block must not be a powershield -- {l}")
    });
    f.check(im.shield_fx, || {
        format!("{tag}: a block must raise the Shield effect -- {l}")
    });

    // Shield cost. The holding drain runs only for a tick that *began* in
    // `State::Shield`; the regen tick runs for every state that is not
    // `State::Shield`, which after a block is always the stun.
    let drain = if matches!(im.state_before, State::Shield) {
        k::SHIELD_DECAY
    } else {
        0.0
    };
    let expected = im.shield_before - drain - damage * k::SHIELD_DAMAGE_MULT + k::SHIELD_REGEN;
    f.check(near(im.shield_after, expected, EPS), || {
        format!(
            "{tag}: shield cost must be exactly drain {drain:.3} + {damage}x{:.2} back {:.2} -> {expected:.3}, got {:.3} -- {l}",
            k::SHIELD_DAMAGE_MULT,
            k::SHIELD_REGEN,
            im.shield_after
        )
    });

    // Pushback: away from the attack, at the table's speed for that damage.
    let expected_push = -im.defender_facing * knockback::shield_push(damage);
    f.check(near(im.vel.x, expected_push, EPS), || {
        format!(
            "{tag}: shield pushback must be {expected_push:+.4}, got {:+.4} -- {l}",
            im.vel.x
        )
    });

    // The stun the block leaves behind.
    let expected_stun = knockback::shieldstun(damage);
    f.check(im.state_after == State::ShieldStun { total: expected_stun }, || {
        format!(
            "{tag}: a block of {damage} damage must leave ShieldStun{{ total: {expected_stun} }}, got {:?} -- {l}",
            im.state_after
        )
    });

    // One shot spends exactly one projectile; a melee spends none.
    let expected_consumed = usize::from(matches!(im.source, Source::Projectile(_)));
    f.check(im.consumed == expected_consumed, || {
        format!(
            "{tag}: expected {expected_consumed} projectile(s) consumed, got {} -- {l}",
            im.consumed
        )
    });
}

/// The impact really landed inside a shieldstun that was still running: the
/// state it met was a stun, and the age it met was genuinely below the
/// stun's total at the moment combat resolved.
fn check_landed_inside_the_stun(f: &mut Failures, tag: &str, im: &Impact) {
    let l = im.line(tag);
    match im.state_at_resolution {
        State::ShieldStun { total } => {
            f.check(im.age_at_resolution < total, || {
                format!(
                    "{tag}: the impact must resolve with the stun still running, age {} of {total} -- {l}",
                    im.age_at_resolution
                )
            });
        }
        other => f.fail(format!(
            "{tag}: the impact must resolve during a shieldstun, it resolved in {other:?} -- {l}"
        )),
    }
}

/// The first block: a settled shield, well outside the powershield window,
/// that survived and went into an ordinary stun.
fn check_survived_first_block(f: &mut Failures, tag: &str, im: &Impact) {
    let l = im.line(tag);
    f.check(im.state_at_resolution == State::Shield, || {
        format!(
            "{tag}: the first impact must meet a held shield, it met {:?} -- {l}",
            im.state_at_resolution
        )
    });
    f.check(im.age_at_resolution > k::POWERSHIELD_WINDOW, || {
        format!(
            "{tag}: the first impact must meet a settled shield, age {} vs window {} -- {l}",
            im.age_at_resolution,
            k::POWERSHIELD_WINDOW
        )
    });
    f.check(im.shield_after > 0.0, || {
        format!(
            "{tag}: the shield must survive the first block, it read {:.3} -- {l}",
            im.shield_after
        )
    });
    f.check(matches!(im.state_after, State::ShieldStun { .. }), || {
        format!(
            "{tag}: the first block must put the defender in shieldstun, got {:?} -- {l}",
            im.state_after
        )
    });
}

/// The impact hit the body: damage, a queued launch and a hit effect.
fn check_hit_the_body(f: &mut Failures, tag: &str, im: &Impact) {
    let l = im.line(tag);
    f.check(!im.blocked(), || {
        format!("{tag}: this impact must reach the body, it was blocked -- {l}")
    });
    f.check(im.hit_fx, || {
        format!("{tag}: a body hit must raise the Hit effect -- {l}")
    });
    f.check(!im.shield_fx && !im.power_fx, || {
        format!("{tag}: a body hit must raise no shield effect -- {l}")
    });
}

fn dump(tag: &str, impacts: &[Impact]) {
    for (i, im) in impacts.iter().enumerate() {
        println!("{}", im.line(&format!("{tag} #{i}")));
    }
}

// ------------------------------------------------------- the two probes

/// A settled shield blocks a smash; the smash's stun is still running when a
/// frontal **shot** arrives.
#[test]
fn a_second_frontal_projectile_inside_a_block_s_shieldstun_is_still_blocked() {
    let mut f = Failures::new("second frontal projectile inside a block's shieldstun");
    for &facing in &[1.0f32, -1.0] {
        let first_tick = swing_impact_tick(PREWARM, MoveId::Fsmash);
        let defence = Defence::default();
        let build = |fire: u32| {
            vec![
                plan(0, FAR, &[(fire, Act::Fire)]),
                plan(2, SMASH_RANGE, &[(PREWARM, Act::Smash)]),
            ]
        };
        // Anywhere inside the smash's stun will do; the fixture then proves
        // the shot really landed there rather than assuming it.
        let (fire, second_tick) = aim(
            facing,
            defence,
            build,
            &[first_tick],
            first_tick + 1,
            first_tick + 6,
        );
        let plans = build(fire);
        let b = run(facing, defence, &plans, second_tick + 12);
        let tag = format!("proj-in-stun facing {facing:+}");
        dump(&tag, &b.impacts);
        if b.impacts.len() != 2 {
            f.fail(format!(
                "{tag}: the fixture must land exactly two impacts, it landed {}",
                b.impacts.len()
            ));
            continue;
        }
        let first = b.impacts[0].clone().with_resolution(met_on(
            facing,
            defence,
            &plans,
            &[],
            b.impacts[0].tick,
        ));
        let second = b.impacts[1].clone().with_resolution(met_on(
            facing,
            defence,
            &plans,
            &[2],
            b.impacts[1].tick,
        ));
        println!("{}", first.line(&format!("{tag} MEASURED#0")));
        println!("{}", second.line(&format!("{tag} MEASURED#1")));

        f.check(first.source == Source::Melee(2), || {
            format!(
                "{tag}: the first impact must be port 2's smash, got {:?}",
                first.source
            )
        });
        f.check(second.source == Source::Projectile(0), || {
            format!(
                "{tag}: the second impact must be port 0's shot, got {:?}",
                second.source
            )
        });
        f.check(
            first.approach_side > 0.0 && second.approach_side > 0.0,
            || {
                format!(
                    "{tag}: both impacts must arrive on the side the defender faces, sides {:+.1} and {:+.1}",
                    first.approach_side, second.approach_side
                )
            },
        );
        check_survived_first_block(&mut f, &format!("{tag} first"), &first);
        check_landed_inside_the_stun(&mut f, &format!("{tag} second"), &second);
        check_blocked_properly(
            &mut f,
            &format!("{tag} second"),
            &second,
            attacks::PROJECTILE_DAMAGE,
        );
    }
    f.finish();
}

/// The same boundary at the other collision site: the second impact inside
/// the stun is a real **melee** hitbox, from a second attacker.
#[test]
fn a_second_frontal_melee_inside_a_block_s_shieldstun_is_still_blocked() {
    let mut f = Failures::new("second frontal melee inside a block's shieldstun");
    for &facing in &[1.0f32, -1.0] {
        let first_tick = swing_impact_tick(PREWARM, MoveId::Fsmash);
        let jab_press = first_tick + 2;
        let plans = [
            plan(2, SMASH_RANGE, &[(PREWARM, Act::Smash)]),
            plan(3, JAB_RANGE, &[(jab_press, Act::Jab)]),
        ];
        let defence = Defence::default();
        let b = run(facing, defence, &plans, first_tick + 16);
        let tag = format!("melee-in-stun facing {facing:+}");
        dump(&tag, &b.impacts);
        if b.impacts.len() != 2 {
            f.fail(format!(
                "{tag}: the fixture must land exactly two impacts, it landed {}",
                b.impacts.len()
            ));
            continue;
        }
        let first = b.impacts[0].clone().with_resolution(met_on(
            facing,
            defence,
            &plans,
            &[],
            b.impacts[0].tick,
        ));
        let second = b.impacts[1].clone().with_resolution(met_on(
            facing,
            defence,
            &plans,
            &[2],
            b.impacts[1].tick,
        ));
        println!("{}", first.line(&format!("{tag} MEASURED#0")));
        println!("{}", second.line(&format!("{tag} MEASURED#1")));

        f.check(first.source == Source::Melee(2), || {
            format!(
                "{tag}: the first impact must be port 2's smash, got {:?}",
                first.source
            )
        });
        f.check(second.source == Source::Melee(3), || {
            format!(
                "{tag}: the second impact must be port 3's jab, got {:?}",
                second.source
            )
        });
        f.check(second.approach_side > 0.0, || {
            format!(
                "{tag}: the jab must come from the side the defender faces, side {:+.1}",
                second.approach_side
            )
        });
        check_survived_first_block(&mut f, &format!("{tag} first"), &first);
        check_landed_inside_the_stun(&mut f, &format!("{tag} second"), &second);
        check_blocked_properly(&mut f, &format!("{tag} second"), &second, jab_damage());
    }
    f.finish();
}

/// Two shots in a row, from two different owners: the first makes the stun,
/// the second arrives inside it. Both are blocked, each spending exactly its
/// own projectile.
#[test]
fn two_consecutive_frontal_projectiles_are_both_blocked() {
    let mut f = Failures::new("two consecutive frontal projectiles");
    for &facing in &[1.0f32, -1.0] {
        let defence = Defence::default();
        let first_target = PREWARM + 5;
        let build_first = |fire: u32| vec![plan(0, FAR, &[(fire, Act::Fire)])];
        let (fire_a, first_tick) = aim(
            facing,
            defence,
            build_first,
            &[],
            first_target,
            first_target + 6,
        );
        let build = |fire_b: u32| {
            vec![
                plan(0, FAR, &[(fire_a, Act::Fire)]),
                plan(3, MID, &[(fire_b, Act::Fire)]),
            ]
        };
        let (fire_b, second_tick) = aim(
            facing,
            defence,
            build,
            &[first_tick],
            first_tick + 1,
            first_tick + 3,
        );
        let plans = build(fire_b);
        let b = run(facing, defence, &plans, second_tick + 12);
        let tag = format!("two-shots facing {facing:+}");
        dump(&tag, &b.impacts);
        if b.impacts.len() != 2 {
            f.fail(format!(
                "{tag}: the fixture must land exactly two impacts, it landed {}",
                b.impacts.len()
            ));
            continue;
        }
        let first = b.impacts[0].clone().with_resolution(met_on(
            facing,
            defence,
            &plans,
            &[],
            b.impacts[0].tick,
        ));
        let second = b.impacts[1].clone().with_resolution(met_on(
            facing,
            defence,
            &plans,
            &[0],
            b.impacts[1].tick,
        ));
        println!("{}", first.line(&format!("{tag} MEASURED#0")));
        println!("{}", second.line(&format!("{tag} MEASURED#1")));

        f.check(first.source == Source::Projectile(0), || {
            format!(
                "{tag}: the first shot must belong to port 0, got {:?}",
                first.source
            )
        });
        f.check(second.source == Source::Projectile(3), || {
            format!(
                "{tag}: the second shot must belong to port 3, got {:?}",
                second.source
            )
        });
        check_survived_first_block(&mut f, &format!("{tag} first"), &first);
        check_blocked_properly(
            &mut f,
            &format!("{tag} first"),
            &first,
            attacks::PROJECTILE_DAMAGE,
        );
        check_landed_inside_the_stun(&mut f, &format!("{tag} second"), &second);
        check_blocked_properly(
            &mut f,
            &format!("{tag} second"),
            &second,
            attacks::PROJECTILE_DAMAGE,
        );
    }
    f.finish();
}

// ------------------------------------------------- the seam on the way out

/// The stun ends by entering `State::Shield` with the frame counter reset to
/// zero -- which is exactly what the powershield window reads. Holding the
/// shield through a stun is **not** a new raise, so no impact anywhere across
/// that seam may be parried for free.
///
/// The sweep walks a real smash across the whole stun, over the tick it ends,
/// and into the first ticks of the shield the defender is holding again --
/// one tick at a time, both facings. A melee is used because its impact tick
/// is exact: the shield is being pushed backwards the whole time, so a shot
/// cannot be aimed at every single tick of the seam.
#[test]
fn leaving_shieldstun_does_not_hand_out_a_free_powershield() {
    let mut f = Failures::new("leaving shieldstun must not renew the powershield window");
    for &facing in &[1.0f32, -1.0] {
        let first_tick = swing_impact_tick(PREWARM, MoveId::Fsmash);
        let defence = Defence::default();
        let base = [plan(2, SMASH_RANGE, &[(PREWARM, Act::Smash)])];
        // Where the stun really ends, read off a replay of this same bout.
        let seam = seam_after(facing, defence, &base, &[2], first_tick, first_tick + 40);
        println!(
            "[BOUNDARY] exit-sweep facing {facing:+}: block on tick {first_tick}, shield restored on tick {seam}"
        );
        let mut saw_stun = false;
        let mut saw_fresh_looking_shield = false;
        for target in (first_tick + 1)..=(seam + 3) {
            let press = target + 1 - attacks::data(CharacterId::Kestrel, MoveId::Fsmash).startup;
            // Every block shoves the shield backwards, so the second
            // attacker is placed against where the defender will actually be
            // on the tick it swings -- measured from a replay, so its smash
            // lands `FRONTAL_BITE` in front of the defender's centre on
            // every tick of the sweep instead of falling short at the end.
            let drift = defender_drift(facing, defence, &base, &[2], target);
            let reach = attacks::data(CharacterId::Kestrel, MoveId::Fsmash)
                .hitbox
                .offset
                .x;
            let plans = [
                plan(2, SMASH_RANGE, &[(PREWARM, Act::Smash)]),
                plan_facing(3, drift - reach - FRONTAL_BITE, 1.0, &[(press, Act::Smash)]),
            ];
            let b = run(facing, defence, &plans, target + 8);
            let tag = format!("exit-sweep facing {facing:+} tick {target}");
            if b.impacts.len() != 2 || b.impacts[1].tick != target {
                dump(&tag, &b.impacts);
                f.fail(format!(
                    "{tag}: the fixture must land its second impact on tick {target}, it landed {} impact(s) {:?}",
                    b.impacts.len(),
                    b.impacts.iter().map(|i| i.tick).collect::<Vec<_>>()
                ));
                continue;
            }
            let second =
                b.impacts[1]
                    .clone()
                    .with_resolution(met_on(facing, defence, &plans, &[2], target));
            println!("{}", second.line(&tag));
            match second.state_at_resolution {
                State::ShieldStun { .. } => saw_stun = true,
                State::Shield if second.age_at_resolution <= k::POWERSHIELD_WINDOW => {
                    saw_fresh_looking_shield = true;
                }
                _ => {}
            }
            f.check(second.source == Source::Melee(3), || {
                format!(
                    "{tag}: the swept impact must be port 3's smash, got {:?}",
                    second.source
                )
            });
            // The whole point: never a free parry, anywhere on the seam --
            // the defender never pressed SHIELD again.
            check_blocked_properly(&mut f, &tag, &second, smash_damage());
        }
        f.check(saw_stun && saw_fresh_looking_shield, || {
            format!(
                "facing {facing:+}: the sweep must cross the stun's end into a shield whose frame counter still reads inside the powershield window -- stun seen={saw_stun}, reset-looking shield seen={saw_fresh_looking_shield}"
            )
        });
    }
    f.finish();
}

/// The same seam at the other collision site: a **shot** arriving on the
/// exact tick the stun hands the shield back. Both blocks here are shots, so
/// the pushback is small enough to aim one at that single tick.
#[test]
fn a_shot_landing_on_the_tick_the_stun_ends_is_not_a_free_powershield() {
    let mut f = Failures::new("a shot on the tick the stun ends is not a free powershield");
    for &facing in &[1.0f32, -1.0] {
        let defence = Defence::default();
        let first_target = PREWARM + 5;
        let build_first = |fire: u32| vec![plan(0, FAR, &[(fire, Act::Fire)])];
        let (fire_a, first_tick) = aim(
            facing,
            defence,
            build_first,
            &[],
            first_target,
            first_target + 6,
        );
        let base = [plan(0, FAR, &[(fire_a, Act::Fire)])];
        let seam = seam_after(facing, defence, &base, &[0], first_tick, first_tick + 40);
        let build = |fire_b: u32| {
            vec![
                plan(0, FAR, &[(fire_a, Act::Fire)]),
                plan(3, MID, &[(fire_b, Act::Fire)]),
            ]
        };
        let (fire_b, landed) = aim(facing, defence, build, &[first_tick], seam, seam);
        let plans = build(fire_b);
        let b = run(facing, defence, &plans, landed + 10);
        let tag = format!("shot-on-the-seam facing {facing:+}");
        println!("[BOUNDARY] {tag}: block on tick {first_tick}, shield restored on tick {seam}");
        dump(&tag, &b.impacts);
        if b.impacts.len() != 2 {
            f.fail(format!(
                "{tag}: the fixture must land exactly two impacts, it landed {}",
                b.impacts.len()
            ));
            continue;
        }
        let second = b.impacts[1].clone().with_resolution(met_on(
            facing,
            defence,
            &plans,
            &[0],
            b.impacts[1].tick,
        ));
        println!("{}", second.line(&format!("{tag} MEASURED")));
        f.check(second.source == Source::Projectile(3), || {
            format!(
                "{tag}: the seam impact must be port 3's shot, got {:?}",
                second.source
            )
        });
        // It really is the seam: a shield whose frame counter was just reset
        // by the stun, with no new press anywhere in the bout.
        f.check(second.state_at_resolution == State::Shield, || {
            format!(
                "{tag}: the shot must meet the shield the stun handed back, it met {:?}",
                second.state_at_resolution
            )
        });
        f.check(second.age_at_resolution <= k::POWERSHIELD_WINDOW, || {
            format!(
                "{tag}: the shot must meet the reset frame counter, age {} vs window {}",
                second.age_at_resolution,
                k::POWERSHIELD_WINDOW
            )
        });
        check_blocked_properly(&mut f, &tag, &second, attacks::PROJECTILE_DAMAGE);
    }
    f.finish();
}

/// The control that keeps the fix honest: a shield raised by a **real new
/// press** inside the window still parries. Whatever closes the seam above
/// must not close this.
#[test]
fn control_a_genuinely_fresh_raise_still_powershields() {
    let mut f = Failures::new("a genuinely fresh raise still powershields");
    for &facing in &[1.0f32, -1.0] {
        let raise = PREWARM + 5;
        let defence = Defence {
            hold: Hold::From(raise),
            ..Default::default()
        };
        let build = |fire: u32| vec![plan(0, FAR, &[(fire, Act::Fire)])];
        let (fire, landed) = aim(facing, defence, build, &[], raise, raise);
        let plans = build(fire);
        let b = run(facing, defence, &plans, landed + 10);
        let tag = format!("fresh-raise facing {facing:+}");
        dump(&tag, &b.impacts);
        if b.impacts.len() != 1 {
            f.fail(format!(
                "{tag}: the fixture must land exactly one impact, it landed {}",
                b.impacts.len()
            ));
            continue;
        }
        let im = b.impacts[0].clone().with_resolution(met_on(
            facing,
            defence,
            &plans,
            &[],
            b.impacts[0].tick,
        ));
        println!("{}", im.line(&format!("{tag} MEASURED")));
        let l = im.line(&tag);

        f.check(im.state_at_resolution == State::Shield, || {
            format!(
                "{tag}: the shot must meet a shield, it met {:?} -- {l}",
                im.state_at_resolution
            )
        });
        f.check(im.age_at_resolution < k::POWERSHIELD_WINDOW, || {
            format!(
                "{tag}: the shot must land inside the powershield window, age {} of {} -- {l}",
                im.age_at_resolution,
                k::POWERSHIELD_WINDOW
            )
        });
        f.check(im.power_fx, || {
            format!("{tag}: a real new raise inside the window must still parry -- {l}")
        });
        f.check(!im.shield_fx && !im.hit_fx, || {
            format!("{tag}: a parry raises only the Powershield effect -- {l}")
        });
        f.check(im.blocked(), || {
            format!("{tag}: a parry takes no damage -- {l}")
        });
        f.check(near(im.shield_after, im.shield_before, EPS), || {
            format!(
                "{tag}: a parry costs no shield, {:.3}->{:.3} -- {l}",
                im.shield_before, im.shield_after
            )
        });
        f.check(im.state_after == State::Shield, || {
            format!(
                "{tag}: a parry takes no stun, left {:?} -- {l}",
                im.state_after
            )
        });
        f.check(im.vel.x == 0.0 && im.vel.y == 0.0, || {
            format!(
                "{tag}: a parry takes no pushback, vel=({:+.3},{:+.3}) -- {l}",
                im.vel.x, im.vel.y
            )
        });
        f.check(im.consumed == 1, || {
            format!(
                "{tag}: the shot is spent on the parry exactly once, {} -- {l}",
                im.consumed
            )
        });
    }
    f.finish();
}

// ------------------------------------------------------------- controls

/// Renamed honestly: the old control never reached `ShieldStun` at all.
/// This one does -- a frontal block that survives, and then a jab into the
/// defender's **back** while that stun is running. A shield covers the side
/// it faces, stun or no stun.
#[test]
fn control_an_impact_from_behind_during_the_stun_still_hits() {
    let mut f = Failures::new("an impact from behind during the stun still hits");
    for &facing in &[1.0f32, -1.0] {
        let first_tick = swing_impact_tick(PREWARM, MoveId::Fsmash);
        let jab_press = first_tick + 4;
        let defence = Defence::default();
        let base = [plan(2, SMASH_RANGE, &[(PREWARM, Act::Smash)])];
        // The first block pushes the defender *towards* this attacker, so
        // standing it at a fixed distance would put its jab in front of the
        // defender by the time it swings. Place it against the measured
        // position instead, far enough back that the hitbox still arrives
        // from behind.
        let drift = defender_drift(facing, defence, &base, &[2], jab_press);
        let reach = attacks::data(CharacterId::Kestrel, MoveId::Jab)
            .hitbox
            .offset
            .x;
        let plans = [
            plan(2, SMASH_RANGE, &[(PREWARM, Act::Smash)]),
            plan_facing(3, drift + reach + REAR_BITE, -1.0, &[(jab_press, Act::Jab)]),
        ];
        let b = run(facing, defence, &plans, first_tick + 16);
        let tag = format!("rear-in-stun facing {facing:+}");
        dump(&tag, &b.impacts);
        if b.impacts.len() != 2 {
            f.fail(format!(
                "{tag}: the fixture must land exactly two impacts, it landed {}",
                b.impacts.len()
            ));
            continue;
        }
        let first = b.impacts[0].clone().with_resolution(met_on(
            facing,
            defence,
            &plans,
            &[],
            b.impacts[0].tick,
        ));
        let second = b.impacts[1].clone().with_resolution(met_on(
            facing,
            defence,
            &plans,
            &[2],
            b.impacts[1].tick,
        ));
        println!("{}", first.line(&format!("{tag} MEASURED#0")));
        println!("{}", second.line(&format!("{tag} MEASURED#1")));

        f.check(first.source == Source::Melee(2), || {
            format!(
                "{tag}: the first impact must be port 2's smash, got {:?}",
                first.source
            )
        });
        f.check(second.source == Source::Melee(3), || {
            format!(
                "{tag}: the second impact must be port 3's jab, got {:?}",
                second.source
            )
        });
        f.check(first.approach_side > 0.0, || {
            format!(
                "{tag}: the smash must come from the front, side {:+.1}",
                first.approach_side
            )
        });
        f.check(second.approach_side < 0.0, || {
            format!(
                "{tag}: the jab must genuinely come from behind, side {:+.1}",
                second.approach_side
            )
        });
        check_survived_first_block(&mut f, &format!("{tag} first"), &first);
        check_landed_inside_the_stun(&mut f, &format!("{tag} second"), &second);
        check_hit_the_body(&mut f, &format!("{tag} second"), &second);
    }
    f.finish();
}

/// A shield pointed the wrong way covers nothing at all: both impacts reach
/// the body, and they are the two impacts the fixture actually fired.
#[test]
fn control_a_shield_turned_away_from_the_attacks_blocks_nothing() {
    let mut f = Failures::new("a shield turned away blocks nothing");
    for &facing in &[1.0f32, -1.0] {
        let first_tick = swing_impact_tick(PREWARM, MoveId::Fsmash);
        let defence = Defence {
            face_front: false,
            ..Default::default()
        };
        let build = |fire: u32| {
            vec![
                plan(0, FAR, &[(fire, Act::Fire)]),
                plan(2, SMASH_RANGE, &[(PREWARM, Act::Smash)]),
            ]
        };
        let (fire, second_tick) = aim(
            facing,
            defence,
            build,
            &[first_tick],
            first_tick + 1,
            first_tick + 6,
        );
        let plans = build(fire);
        let b = run(facing, defence, &plans, second_tick + 12);
        let tag = format!("turned-away facing {facing:+}");
        dump(&tag, &b.impacts);
        if b.impacts.len() != 2 {
            f.fail(format!(
                "{tag}: the fixture must land exactly two impacts, it landed {}",
                b.impacts.len()
            ));
            continue;
        }
        f.check(b.impacts[0].source == Source::Melee(2), || {
            format!(
                "{tag}: impact #0 must be port 2's smash, got {:?}",
                b.impacts[0].source
            )
        });
        f.check(b.impacts[1].source == Source::Projectile(0), || {
            format!(
                "{tag}: impact #1 must be port 0's shot, got {:?}",
                b.impacts[1].source
            )
        });
        f.check(b.impacts[1].consumed == 1, || {
            format!(
                "{tag}: the shot is spent exactly once, got {}",
                b.impacts[1].consumed
            )
        });
        for (i, im) in b.impacts.iter().enumerate() {
            f.check(im.approach_side < 0.0, || {
                format!(
                    "{tag} #{i}: the attack must reach the defender's back, side {:+.1}",
                    im.approach_side
                )
            });
            check_hit_the_body(&mut f, &format!("{tag} #{i}"), im);
        }
    }
    f.finish();
}

/// No shield at all: both impacts reach the body, one melee and one shot.
#[test]
fn control_an_unshielded_defender_takes_both_impacts() {
    let mut f = Failures::new("an unshielded defender takes both impacts");
    for &facing in &[1.0f32, -1.0] {
        let first_tick = swing_impact_tick(PREWARM, MoveId::Fsmash);
        let defence = Defence {
            hold: Hold::Never,
            ..Default::default()
        };
        let build = |fire: u32| {
            vec![
                plan(0, FAR, &[(fire, Act::Fire)]),
                plan(2, SMASH_RANGE, &[(PREWARM, Act::Smash)]),
            ]
        };
        let (fire, second_tick) = aim(
            facing,
            defence,
            build,
            &[first_tick],
            first_tick + 1,
            first_tick + 6,
        );
        let plans = build(fire);
        let b = run(facing, defence, &plans, second_tick + 12);
        let tag = format!("bare facing {facing:+}");
        dump(&tag, &b.impacts);
        if b.impacts.len() != 2 {
            f.fail(format!(
                "{tag}: the fixture must land exactly two impacts, it landed {}",
                b.impacts.len()
            ));
            continue;
        }
        f.check(b.impacts[0].source == Source::Melee(2), || {
            format!(
                "{tag}: impact #0 must be port 2's smash, got {:?}",
                b.impacts[0].source
            )
        });
        f.check(b.impacts[1].source == Source::Projectile(0), || {
            format!(
                "{tag}: impact #1 must be port 0's shot, got {:?}",
                b.impacts[1].source
            )
        });
        f.check(b.impacts[1].consumed == 1, || {
            format!(
                "{tag}: the shot is spent exactly once, got {}",
                b.impacts[1].consumed
            )
        });
        for (i, im) in b.impacts.iter().enumerate() {
            check_hit_the_body(&mut f, &format!("{tag} #{i}"), im);
        }
    }
    f.finish();
}

// ------------------------------------------------------------- the trap

/// Shared body of the two break controls: break the shield, prove a shot
/// during the break reaches the body, then let the defender get its shield
/// back and prove it blocks again.
///
/// `keep_for_during` / `keep_for_after` are the ports whose attacks stay in
/// the replay that measures what each impact met -- everything except the
/// impact being measured.
#[allow(clippy::too_many_arguments)]
fn break_and_recover(
    f: &mut Failures,
    tag: &str,
    facing: f32,
    defence: Defence,
    plans: &[Plan],
    during: (u32, &[usize]),
    after: (u32, &[usize]),
) {
    let (during_tick, keep_for_during) = during;
    let (after_tick, keep_for_after) = after;
    let b = run(facing, defence, plans, after_tick + 10);
    dump(tag, &b.impacts);

    // The break itself, read off the trace: a 120-frame shieldstun with the
    // shield gone. It does not read 0.00 afterwards -- the regen that runs
    // for any non-`Shield` state puts `SHIELD_REGEN` straight back on top of
    // the zero a break leaves, which is why shield health can never be used
    // to tell a break from an ordinary block.
    let broke = b
        .trace
        .iter()
        .find(|(_, s, _, _, _, _)| matches!(s, State::ShieldStun { total } if *total == 120));
    match broke {
        Some(&(t, s, _, health, _, _)) => {
            println!("[BOUNDARY] {tag} BREAK tick={t} -> {s:?} shield={health:.3}");
            f.check(health <= k::SHIELD_REGEN + 1e-4, || {
                format!("{tag}: the break must empty the shield, it read {health:.3} at tick {t}")
            });
        }
        None => {
            f.fail(format!("{tag}: the fixture never broke the shield"));
            return;
        }
    }

    match b.impacts.iter().find(|i| i.tick == during_tick).cloned() {
        Some(im) => {
            let im =
                im.with_resolution(met_on(facing, defence, plans, keep_for_during, during_tick));
            println!("{}", im.line(&format!("{tag} DURING-BREAK")));
            f.check(im.source == Source::Projectile(0), || {
                format!(
                    "{tag}: the shot during the break must belong to port 0, got {:?}",
                    im.source
                )
            });
            f.check(
                matches!(im.state_at_resolution, State::ShieldStun { total } if total == 120),
                || {
                    format!(
                        "{tag}: the shot must land during the break's own stun, it met {:?}",
                        im.state_at_resolution
                    )
                },
            );
            check_hit_the_body(f, &format!("{tag} during-break"), &im);
        }
        None => f.fail(format!(
            "{tag}: nothing landed on tick {during_tick}, so the break was never tested"
        )),
    }

    match b.impacts.iter().find(|i| i.tick == after_tick).cloned() {
        Some(im) => {
            let im = im.with_resolution(met_on(facing, defence, plans, keep_for_after, after_tick));
            println!("{}", im.line(&format!("{tag} AFTER-RECOVERY")));
            f.check(im.source == Source::Projectile(3), || {
                format!(
                    "{tag}: the shot after recovery must belong to port 3, got {:?}",
                    im.source
                )
            });
            f.check(im.state_at_resolution == State::Shield, || {
                format!(
                    "{tag}: the defender must be holding a real shield again, it met {:?}",
                    im.state_at_resolution
                )
            });
            f.check(im.age_at_resolution > k::POWERSHIELD_WINDOW, || {
                format!(
                    "{tag}: the recovery block must be a settled shield, age {}",
                    im.age_at_resolution
                )
            });
            check_blocked_properly(
                f,
                &format!("{tag} after-recovery"),
                &im,
                attacks::PROJECTILE_DAMAGE,
            );
        }
        None => f.fail(format!(
            "{tag}: nothing landed on tick {after_tick}, so the recovery was never tested"
        )),
    }
}

/// Broken by damage. The flag that says "this stun is a break" is written in
/// the damage path; a shot during the break must reach the body, and the
/// shield must come back afterwards.
#[test]
fn control_a_shield_broken_by_damage_stays_vulnerable_and_recovers() {
    let mut f = Failures::new("a shield broken by damage stays vulnerable and recovers");
    for &facing in &[1.0f32, -1.0] {
        let break_tick = swing_impact_tick(PREWARM, MoveId::Fsmash);
        let reraise = 230u32;
        // Enough shield to hold until the smash, not enough to survive it.
        let barely = smash_damage() * k::SHIELD_DAMAGE_MULT - 0.5;
        let defence = Defence {
            health: Some(barely),
            // Let go after the break so the shield can regrow, then raise it
            // again well before the second shot arrives.
            hold: Hold::Except(break_tick + 1, reraise),
            ..Default::default()
        };
        let build_during = |fire: u32| {
            vec![
                plan(0, FAR, &[(fire, Act::Fire)]),
                plan(2, SMASH_RANGE, &[(PREWARM, Act::Smash)]),
            ]
        };
        let (fire_during, during) = aim(
            facing,
            defence,
            build_during,
            &[break_tick],
            break_tick + 1,
            break_tick + 20,
        );
        let build = |fire_after: u32| {
            vec![
                plan(0, FAR, &[(fire_during, Act::Fire)]),
                plan(2, SMASH_RANGE, &[(PREWARM, Act::Smash)]),
                plan(3, MID, &[(fire_after, Act::Fire)]),
            ]
        };
        let (fire_after, after) = aim(
            facing,
            defence,
            build,
            &[break_tick, during],
            reraise + 6,
            reraise + 60,
        );
        let plans = build(fire_after);
        break_and_recover(
            &mut f,
            &format!("break-by-damage facing {facing:+}"),
            facing,
            defence,
            &plans,
            (during, &[2, 3]),
            (after, &[0, 2]),
        );
    }
    f.finish();
}

/// Broken by holding it too long. The same flag is written in a *second*
/// place -- the holding drain -- and this proves that site behaves the same.
#[test]
fn control_a_shield_broken_by_holding_stays_vulnerable_and_recovers() {
    let mut f = Failures::new("a shield broken by holding stays vulnerable and recovers");
    for &facing in &[1.0f32, -1.0] {
        // Just enough shield that the holding drain alone empties it.
        let thin = 3.0f32;
        let reraise = 230u32;
        let defence = Defence {
            health: Some(thin),
            hold: Hold::Except(20, reraise),
            ..Default::default()
        };
        let build_during = |fire: u32| vec![plan(0, FAR, &[(fire, Act::Fire)])];
        let (fire_during, during) = aim(facing, defence, build_during, &[], 13, 25);
        let build = |fire_after: u32| {
            vec![
                plan(0, FAR, &[(fire_during, Act::Fire)]),
                plan(3, MID, &[(fire_after, Act::Fire)]),
            ]
        };
        let (fire_after, after) = aim(facing, defence, build, &[during], reraise + 6, reraise + 60);
        let plans = build(fire_after);
        break_and_recover(
            &mut f,
            &format!("break-by-holding facing {facing:+}"),
            facing,
            defence,
            &plans,
            (during, &[3]),
            (after, &[0]),
        );
    }
    f.finish();
}

// ----------------------------------------------------------- for the record

#[test]
fn the_boundary_itself_reported_for_the_record() {
    // The two predicates the block sites depend on, measured rather than
    // asserted: `is_shielding()` is only true in `State::Shield`, while
    // `can_be_hit()` is true in `ShieldStun` as well. That asymmetry is what
    // every test above is about.
    let cfg = MatchConfig {
        chars: [CharacterId::Kestrel; 4],
        ..Default::default()
    };
    let mut gs = GameState::new(2, cfg);
    for _ in 0..90 {
        gs.step(&[neutral(), neutral()]);
    }
    let f = &mut gs.fighters[0];
    f.set_state_pub(State::Shield);
    println!(
        "[BOUNDARY] State::Shield     -> is_shielding={} can_be_hit={}",
        f.is_shielding(),
        f.can_be_hit()
    );
    let shielding_in_shield = f.is_shielding();
    f.set_state_pub(State::ShieldStun { total: 8 });
    println!(
        "[BOUNDARY] State::ShieldStun -> is_shielding={} can_be_hit={}",
        f.is_shielding(),
        f.can_be_hit()
    );
    assert!(shielding_in_shield);
    assert!(
        f.can_be_hit(),
        "a fighter in shieldstun is hittable -- that half of the asymmetry is deliberate"
    );
    println!(
        "[BOUNDARY] POWERSHIELD_WINDOW={} SHIELD_DECAY={} SHIELD_REGEN={} SHIELD_DAMAGE_MULT={}",
        k::POWERSHIELD_WINDOW,
        k::SHIELD_DECAY,
        k::SHIELD_REGEN,
        k::SHIELD_DAMAGE_MULT
    );
    println!(
        "[BOUNDARY] shieldstun: smash {} -> {} frames | projectile {} -> {} frames | jab {} -> {} frames",
        smash_damage(),
        knockback::shieldstun(smash_damage()),
        attacks::PROJECTILE_DAMAGE,
        knockback::shieldstun(attacks::PROJECTILE_DAMAGE),
        jab_damage(),
        knockback::shieldstun(jab_damage())
    );
}
