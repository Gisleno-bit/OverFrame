//! Runtime exports and versioned fixtures for the art/QA exchange
//! (`docs/art/EXCHANGE.md`).
//!
//! Everything here is *measured* from the simulation, never typed in:
//! `frame-data.csv` is produced by driving real inputs through `sim::step`
//! and reading the hitbox the engine would test that tick; `characters.json`
//! and `stages.json` are the attribute blocks and platform tables the engine
//! runs on. Fixtures are scripted input files (`docs/art/fixtures/*.json`)
//! shared by the 3D capture tool, the headless renderer and CI, so every
//! image of a run comes from the same deterministic match.

use crate::sim::attacks::{self, MoveId};
use crate::sim::fighter::State;
use crate::sim::roster::CharacterId;
use crate::sim::stage::StageId;
use crate::sim::{buttons, GameState, MatchConfig, PlayerInput, Vec2};
use serde::{Deserialize, Serialize};
use std::fmt::Write as _;

/// Agreed header (EXCHANGE.md §"Exportación real de frame data").
pub const CSV_HEADER: &str = "source_sha,character_id,action_id,variant_id,sample_phase,tick_index,state_frame,hitbox_id,hitbox_active,center_x,center_y,radius,hitlag_remaining,contact_marker";

/// Snake-case action ids used in the CSV and the contact captures.
pub fn action_id(id: MoveId) -> &'static str {
    match id {
        MoveId::Jab => "jab",
        MoveId::Jab2 => "jab2",
        MoveId::Ftilt => "ftilt",
        MoveId::Utilt => "utilt",
        MoveId::Dtilt => "dtilt",
        MoveId::Fsmash => "fsmash",
        MoveId::Usmash => "usmash",
        MoveId::Dsmash => "dsmash",
        MoveId::DashAttack => "dash_attack",
        MoveId::Nair => "nair",
        MoveId::Fair => "fair",
        MoveId::Bair => "bair",
        MoveId::Uair => "uair",
        MoveId::Dair => "dair",
        MoveId::SpecialN => "special_n",
        MoveId::SpecialUp => "special_up",
        MoveId::SpecialSide => "special_side",
        MoveId::SpecialDown => "special_down",
        MoveId::ThrowF => "throw_f",
        MoveId::ThrowB => "throw_b",
        MoveId::ThrowU => "throw_u",
        MoveId::ThrowD => "throw_d",
    }
}

pub fn character_id(id: CharacterId) -> &'static str {
    match id {
        CharacterId::Kestrel => "kestrel",
        CharacterId::Boulder => "boulder",
        CharacterId::Viper => "viper",
    }
}

pub fn parse_character(s: &str) -> Option<CharacterId> {
    CharacterId::ALL
        .iter()
        .copied()
        .find(|c| character_id(*c) == s || c.name().eq_ignore_ascii_case(s))
}

pub fn stage_id(id: StageId) -> &'static str {
    match id {
        StageId::Lattice => "lattice",
        StageId::Meridian => "meridian",
        StageId::Tidegate => "tidegate",
    }
}

pub fn parse_stage(s: &str) -> Option<StageId> {
    StageId::ALL.iter().copied().find(|c| stage_id(*c) == s)
}

// ----------------------------------------------------------------- fixtures

pub const FIXTURE_IDLE: &str = include_str!("../docs/art/fixtures/idle-v1.json");
pub const FIXTURE_COMBAT: &str = include_str!("../docs/art/fixtures/combat-v1.json");

/// A scripted, versioned match: initial placement plus timed inputs.
#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct Fixture {
    pub schema_version: u32,
    pub id: String,
    pub seed: u32,
    pub stage: String,
    pub stocks: i32,
    pub players: Vec<FixturePlayer>,
    /// Ticks simulated (and drawn) before tick 0, to warm visual effects.
    pub warmup_ticks: u32,
    pub ticks: u32,
    #[serde(default)]
    pub inputs: Vec<FixtureInput>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct FixturePlayer {
    pub character: String,
    pub palette: u8,
    pub x: f32,
    pub facing: f32,
}

/// One input held from `tick` for `hold` ticks (default 1).
#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct FixtureInput {
    pub tick: u32,
    pub player: usize,
    #[serde(default)]
    pub stick: [f32; 2],
    #[serde(default)]
    pub cstick: [f32; 2],
    #[serde(default)]
    pub buttons: Vec<String>,
    #[serde(default = "one")]
    pub hold: u32,
}

fn one() -> u32 {
    1
}

pub fn button_mask(names: &[String]) -> Result<u16, String> {
    let mut m = 0u16;
    for n in names {
        m |= match n.as_str() {
            "attack" => buttons::ATTACK,
            "special" => buttons::SPECIAL,
            "jump" => buttons::JUMP,
            "shield" => buttons::SHIELD,
            "grab" => buttons::GRAB,
            other => return Err(format!("unknown button `{other}`")),
        };
    }
    Ok(m)
}

impl Fixture {
    pub fn parse(json: &str) -> Result<Fixture, String> {
        let f: Fixture = serde_json::from_str(json).map_err(|e| e.to_string())?;
        if f.schema_version != 1 {
            return Err("fixture: unsupported schema".into());
        }
        if f.players.is_empty() || f.players.len() > 4 {
            return Err("fixture: 1–4 players".into());
        }
        for p in &f.players {
            parse_character(&p.character)
                .ok_or_else(|| format!("fixture: unknown character `{}`", p.character))?;
        }
        parse_stage(&f.stage).ok_or_else(|| format!("fixture: unknown stage `{}`", f.stage))?;
        for i in &f.inputs {
            if i.player >= f.players.len() {
                return Err(format!("fixture: input for missing player {}", i.player));
            }
            button_mask(&i.buttons)?;
        }
        Ok(f)
    }

    /// The match at fixture tick 0 minus `warmup_ticks` — players placed
    /// on the main platform at their declared x, facing as declared.
    pub fn initial_state(&self) -> GameState {
        let mut cfg = MatchConfig {
            seed: self.seed,
            stage: parse_stage(&self.stage).unwrap_or(StageId::Lattice),
            stocks: self.stocks,
            ..MatchConfig::default()
        };
        for (i, p) in self.players.iter().enumerate() {
            cfg.chars[i] = parse_character(&p.character).unwrap_or(CharacterId::Kestrel);
            cfg.palettes[i] = p.palette;
        }
        let mut gs = GameState::new(self.players.len(), cfg);
        place_on_main(&mut gs, &self.players);
        gs
    }

    /// Input for `player` at fixture `tick` (ticks before 0 are neutral).
    pub fn input(&self, tick: i64, player: usize) -> PlayerInput {
        let mut out = PlayerInput::default();
        if tick < 0 {
            return out;
        }
        let t = tick as u32;
        for i in &self.inputs {
            if i.player == player && t >= i.tick && t < i.tick + i.hold.max(1) {
                out.stick = Vec2::new(i.stick[0], i.stick[1]);
                out.cstick = Vec2::new(i.cstick[0], i.cstick[1]);
                out.buttons |= button_mask(&i.buttons).unwrap_or(0);
            }
        }
        out
    }

    pub fn inputs_at(&self, tick: i64) -> Vec<PlayerInput> {
        (0..self.players.len())
            .map(|p| self.input(tick, p))
            .collect()
    }

    /// Run the fixture from its initial state to fixture tick `tick`
    /// (inclusive of the warmup), returning the state *after* that tick.
    pub fn state_at(&self, tick: u32) -> GameState {
        let mut gs = self.initial_state();
        let start = -(self.warmup_ticks as i64);
        for t in start..=(tick as i64) {
            let inp = self.inputs_at(t);
            gs.step(&inp);
        }
        gs
    }
}

/// Put every fighter standing on the stage's main platform.
pub fn place_on_main(gs: &mut GameState, players: &[FixturePlayer]) {
    let main = gs.stage.platforms.iter().position(|p| p.solid).unwrap_or(0);
    let y = gs.stage.platforms[main].y;
    for (i, p) in players.iter().enumerate() {
        let f = &mut gs.fighters[i];
        f.pos = Vec2::new(p.x, y);
        f.prev_pos = f.pos;
        f.vel = Vec2::ZERO;
        f.facing = if p.facing < 0.0 { -1.0 } else { 1.0 };
        f.grounded = true;
        f.support = Some(main);
        f.set_state_pub(State::Stand);
    }
}

// ----------------------------------------------------------------- frame data

/// One exported row (also the source for the contact captures).
#[derive(Debug, Clone, Serialize)]
pub struct Row {
    pub character_id: &'static str,
    pub action_id: &'static str,
    pub variant_id: &'static str,
    /// `after_step`: the state after `sim::step` of `tick_index`.
    /// `pre_step`: the state *before* the step of `tick_index` — only used
    /// for the context row of a move that starts on its very first input
    /// tick (there is no earlier after_step sample inside the recipe).
    pub sample_phase: &'static str,
    /// Ticks since the recipe's first input.
    pub tick_index: i64,
    pub state_frame: u32,
    pub hitbox_id: String,
    pub hitbox_active: bool,
    pub center: Option<(f32, f32)>,
    pub radius: Option<f32>,
    pub hitlag_remaining: u32,
    pub contact_marker: bool,
}

/// Outcome of trying one recipe on the real simulation.
#[derive(Debug, Clone, Serialize)]
pub struct CaseReport {
    pub character_id: &'static str,
    pub action_id: &'static str,
    pub variant_id: &'static str,
    /// `exported`, or why not (`recipe_did_not_produce_move: <state>`).
    pub status: String,
    pub rows: usize,
    /// Fixture-like description of the inputs used, for the manifest.
    pub recipe: String,
}

/// Snapshot of one tick of a case, for the capture tool (pose + hitbox).
#[derive(Debug, Clone)]
pub struct CaseTick {
    pub tick_index: i64,
    pub state: GameState,
    /// The fighter's own hitbox row.
    pub row: Row,
    /// The projectile the fighter owns this tick, as its own hitbox row
    /// (`hitbox_id = "projectile"`, radius = the sim's collision radius).
    pub projectile: Option<Row>,
    /// What this tick is, for captions: `pre_step`, `windup`, `active`,
    /// `recovery`, `throw`, `release`, `after`.
    pub label: &'static str,
}

struct Recipe {
    variant: &'static str,
    /// Where the practice target stands (x, facing), if the move needs one.
    target: Option<f32>,
    /// Inputs for player 0 at tick `t` (0-based from the first script tick).
    script: fn(u32) -> PlayerInput,
    description: &'static str,
}

fn stick(x: f32, y: f32, b: u16) -> PlayerInput {
    PlayerInput {
        stick: Vec2::new(x, y),
        buttons: b,
        ..Default::default()
    }
}
fn cstick(x: f32, y: f32) -> PlayerInput {
    PlayerInput {
        cstick: Vec2::new(x, y),
        ..Default::default()
    }
}
fn press(b: u16) -> PlayerInput {
    PlayerInput {
        buttons: b,
        ..Default::default()
    }
}

/// A full hop (jump held through the longest jumpsquat), then the aerial
/// input: attack is pressed with the stick direction at t8 and again at
/// t10 (every character is airborne by t7), with the stick centred in
/// between and afterwards so a down-stick cannot fast-fall the fighter into
/// the ground before the hitbox comes out.
fn aerial(x: f32, y: f32) -> fn(u32) -> PlayerInput {
    // Function pointers cannot capture, so the five recipes are spelled out.
    match (x as i32, y as i32) {
        (0, 1) => |t| {
            if t <= 6 {
                press(buttons::JUMP)
            } else if t == 8 || t == 10 {
                stick(0.0, 1.0, buttons::ATTACK)
            } else {
                PlayerInput::default()
            }
        },
        (0, -1) => |t| {
            if t <= 6 {
                press(buttons::JUMP)
            } else if t == 8 || t == 10 {
                stick(0.0, -1.0, buttons::ATTACK)
            } else {
                PlayerInput::default()
            }
        },
        (1, 0) => |t| {
            if t <= 6 {
                press(buttons::JUMP)
            } else if t == 8 || t == 10 {
                stick(0.6, 0.0, buttons::ATTACK)
            } else {
                PlayerInput::default()
            }
        },
        (-1, 0) => |t| {
            if t <= 6 {
                press(buttons::JUMP)
            } else if t == 8 || t == 10 {
                stick(-0.6, 0.0, buttons::ATTACK)
            } else {
                PlayerInput::default()
            }
        },
        _ => |t| {
            if t <= 6 {
                press(buttons::JUMP)
            } else if t == 8 || t == 10 {
                press(buttons::ATTACK)
            } else {
                PlayerInput::default()
            }
        },
    }
}

fn recipes(id: MoveId) -> Vec<Recipe> {
    let ground = |variant, script, description| Recipe {
        variant,
        target: None,
        script,
        description,
    };
    match id {
        MoveId::Jab => vec![ground(
            "ground",
            |t| {
                if t == 0 {
                    press(buttons::ATTACK)
                } else {
                    PlayerInput::default()
                }
            },
            "t0: attack",
        )],
        MoveId::Jab2 => vec![Recipe {
            variant: "after_jab1_hit",
            target: Some(11.0),
            script: |t| {
                if t == 0 || (t >= 4 && t % 2 == 0) {
                    press(buttons::ATTACK)
                } else {
                    PlayerInput::default()
                }
            },
            description:
                "target at +11; t0: attack (jab 1 hits); attack re-pressed every 2 ticks from t4",
        }],
        MoveId::Ftilt => vec![ground(
            "ground",
            |t| {
                if t == 0 {
                    stick(0.55, 0.0, buttons::ATTACK)
                } else {
                    PlayerInput::default()
                }
            },
            "t0: stick (0.55,0) + attack",
        )],
        MoveId::Utilt => vec![ground(
            "ground",
            |t| {
                if t == 0 {
                    stick(0.0, 0.55, buttons::ATTACK)
                } else {
                    PlayerInput::default()
                }
            },
            "t0: stick (0,0.55) + attack",
        )],
        MoveId::Dtilt => vec![ground(
            "ground",
            |t| {
                if t == 0 {
                    stick(0.0, -0.55, buttons::ATTACK)
                } else {
                    PlayerInput::default()
                }
            },
            "t0: stick (0,-0.55) + attack",
        )],
        MoveId::Fsmash => vec![
            ground(
                "cstick",
                |t| {
                    if t == 0 {
                        cstick(1.0, 0.0)
                    } else {
                        PlayerInput::default()
                    }
                },
                "t0: c-stick (1,0)",
            ),
            ground(
                "charged_max",
                |t| {
                    if t < 70 {
                        stick(1.0, 0.0, buttons::ATTACK)
                    } else {
                        PlayerInput::default()
                    }
                },
                "t0..70: stick flick (1,0) + attack held (full charge)",
            ),
        ],
        MoveId::Usmash => vec![ground(
            "cstick",
            |t| {
                if t == 0 {
                    cstick(0.0, 1.0)
                } else {
                    PlayerInput::default()
                }
            },
            "t0: c-stick (0,1)",
        )],
        MoveId::Dsmash => vec![ground(
            "cstick",
            |t| {
                if t == 0 {
                    cstick(0.0, -1.0)
                } else {
                    PlayerInput::default()
                }
            },
            "t0: c-stick (0,-1)",
        )],
        MoveId::DashAttack => vec![ground(
            "from_dash",
            |t| {
                if t < 4 {
                    stick(1.0, 0.0, 0)
                } else if t == 4 {
                    stick(1.0, 0.0, buttons::ATTACK)
                } else {
                    PlayerInput::default()
                }
            },
            "t0..4: stick (1,0) dash; t4: + attack",
        )],
        MoveId::Nair => vec![ground(
            "fullhop",
            aerial(0.0, 0.0),
            "t0..6: jump held (full hop); t8, t10: attack",
        )],
        MoveId::Fair => vec![ground(
            "fullhop",
            aerial(1.0, 0.0),
            "t0..6: jump held; t8, t10: stick (0.6,0) + attack",
        )],
        MoveId::Bair => vec![ground(
            "fullhop",
            aerial(-1.0, 0.0),
            "t0..6: jump held; t8, t10: stick (-0.6,0) + attack",
        )],
        MoveId::Uair => vec![ground(
            "fullhop",
            aerial(0.0, 1.0),
            "t0..6: jump held; t8, t10: stick (0,1) + attack",
        )],
        MoveId::Dair => vec![ground(
            "fullhop",
            aerial(0.0, -1.0),
            "t0..6: jump held; t8, t10: stick (0,-1) + attack",
        )],
        MoveId::SpecialN => vec![ground(
            "ground",
            |t| {
                if t == 0 {
                    press(buttons::SPECIAL)
                } else {
                    PlayerInput::default()
                }
            },
            "t0: special",
        )],
        MoveId::SpecialUp => vec![ground(
            "ground",
            |t| {
                if t == 0 {
                    stick(0.0, 1.0, buttons::SPECIAL)
                } else {
                    PlayerInput::default()
                }
            },
            "t0: stick (0,1) + special",
        )],
        MoveId::SpecialSide => vec![ground(
            "ground",
            |t| {
                if t == 0 {
                    stick(1.0, 0.0, buttons::SPECIAL)
                } else {
                    PlayerInput::default()
                }
            },
            "t0: stick (1,0) + special",
        )],
        MoveId::SpecialDown => vec![ground(
            "ground",
            |t| {
                if t == 0 {
                    stick(0.0, -1.0, buttons::SPECIAL)
                } else {
                    PlayerInput::default()
                }
            },
            "t0: stick (0,-1) + special",
        )],
        MoveId::ThrowF | MoveId::ThrowB | MoveId::ThrowU | MoveId::ThrowD => {
            let (script, description): (fn(u32) -> PlayerInput, &'static str) = match id {
                MoveId::ThrowF => (
                    |t| {
                        if t == 0 {
                            press(buttons::GRAB)
                        } else if t >= 12 {
                            stick(1.0, 0.0, 0)
                        } else {
                            PlayerInput::default()
                        }
                    },
                    "target at +10; t0: grab; t12+: stick (1,0)",
                ),
                MoveId::ThrowB => (
                    |t| {
                        if t == 0 {
                            press(buttons::GRAB)
                        } else if t >= 12 {
                            stick(-1.0, 0.0, 0)
                        } else {
                            PlayerInput::default()
                        }
                    },
                    "target at +10; t0: grab; t12+: stick (-1,0)",
                ),
                MoveId::ThrowU => (
                    |t| {
                        if t == 0 {
                            press(buttons::GRAB)
                        } else if t >= 12 {
                            stick(0.0, 1.0, 0)
                        } else {
                            PlayerInput::default()
                        }
                    },
                    "target at +10; t0: grab; t12+: stick (0,1)",
                ),
                _ => (
                    |t| {
                        if t == 0 {
                            press(buttons::GRAB)
                        } else if t >= 12 {
                            stick(0.0, -1.0, 0)
                        } else {
                            PlayerInput::default()
                        }
                    },
                    "target at +10; t0: grab; t12+: stick (0,-1)",
                ),
            };
            vec![Recipe {
                variant: "from_grab",
                target: Some(10.0),
                script,
                description,
            }]
        }
    }
}

fn is_move(state: State, id: MoveId) -> bool {
    match state {
        State::Attack { id: m, .. } => m == id,
        State::Throw { id: m } => m == id,
        _ => false,
    }
}

/// Run one recipe on a fresh, deterministic two-player match. Returns the
/// per-tick snapshots from one tick before the move starts to the tick it
/// ends, or an error naming the state the recipe produced instead.
pub fn run_case(ch: CharacterId, id: MoveId, variant: &str) -> Result<Vec<CaseTick>, String> {
    run_case_facing(ch, id, variant, 1.0)
}

/// Mirror a recipe input for a fighter facing −X (stick and C-stick X
/// negated; buttons unchanged).
fn mirror_input(mut i: PlayerInput, facing: f32) -> PlayerInput {
    if facing < 0.0 {
        i.stick.x = -i.stick.x;
        i.cstick.x = -i.cstick.x;
    }
    i
}

/// [`run_case`] with an explicit facing: the fighter starts facing
/// `facing` (±1) and the recipe's horizontal inputs and the practice
/// target are mirrored accordingly, so "forward" stays forward.
pub fn run_case_facing(
    ch: CharacterId,
    id: MoveId,
    variant: &str,
    facing: f32,
) -> Result<Vec<CaseTick>, String> {
    let facing = if facing < 0.0 { -1.0 } else { 1.0 };
    let recipe = recipes(id)
        .into_iter()
        .find(|r| r.variant == variant)
        .ok_or_else(|| format!("no recipe `{variant}` for {}", action_id(id)))?;
    let mut cfg = MatchConfig {
        stage: StageId::Lattice,
        ..MatchConfig::default()
    };
    cfg.chars[0] = ch;
    cfg.chars[1] = CharacterId::Boulder;
    let mut gs = GameState::new(2, cfg);
    let target_x = recipe.target.unwrap_or(120.0) * facing;
    place_on_main(
        &mut gs,
        &[
            FixturePlayer {
                character: character_id(ch).into(),
                palette: 0,
                x: 0.0,
                facing,
            },
            FixturePlayer {
                character: "boulder".into(),
                palette: 1,
                x: target_x,
                facing: -facing,
            },
        ],
    );
    // Settle for a few ticks so the placement is the engine's own.
    for _ in 0..8 {
        gs.step(&[PlayerInput::default(), PlayerInput::default()]);
    }
    let mut out: Vec<CaseTick> = Vec::new();
    let mut started = false;
    let mut prev: Option<GameState> = None;
    let mut seen_states: Vec<String> = Vec::new();
    let mut prev_target_hitlag = 0u32;
    let mut prev_target_grabbed = false;
    for t in 0..240u32 {
        let inp = [
            mirror_input((recipe.script)(t), facing),
            PlayerInput::default(),
        ];
        let before = gs.clone();
        gs.step(&inp);
        let f = &gs.fighters[0];
        let active = is_move(f.state, id);
        if !started {
            if active {
                started = true;
                // Context row: the last after_step sample before the move,
                // or — when the move starts on the first input tick — the
                // pre_step state of that tick, labelled as such.
                match prev.take() {
                    Some(p) => out.push(snapshot(
                        &p,
                        ch,
                        id,
                        recipe.variant,
                        "after_step",
                        t as i64 - 1,
                        0,
                        "windup",
                    )),
                    None => out.push(snapshot(
                        &before,
                        ch,
                        id,
                        recipe.variant,
                        "pre_step",
                        t as i64,
                        0,
                        "pre_step",
                    )),
                }
            } else {
                let s = format!("{:?}", f.state);
                if seen_states.last() != Some(&s) {
                    seen_states.push(s);
                }
                prev = Some(gs.clone());
                prev_target_grabbed = matches!(gs.fighters[1].state, State::Grabbed);
                continue;
            }
        } else if !active {
            // One row after the move: shows the recovery state.
            out.push(snapshot(
                &gs,
                ch,
                id,
                recipe.variant,
                "after_step",
                t as i64,
                0,
                "after",
            ));
            break;
        }
        let victim = &gs.fighters[1];
        let victim_hitlag = victim.hitlag;
        let grabbed = matches!(victim.state, State::Grabbed);
        let released = prev_target_grabbed && !grabbed;
        let contact = (victim_hitlag > 0 && prev_target_hitlag == 0) || released;
        let label = if released {
            "release"
        } else if matches!(f.state, State::Throw { .. }) {
            "throw"
        } else {
            let md = attacks::data(ch, id);
            if md.is_active(f.state_frame) {
                "active"
            } else if f.state_frame < md.startup {
                "windup"
            } else {
                "recovery"
            }
        };
        out.push(snapshot(
            &gs,
            ch,
            id,
            recipe.variant,
            "after_step",
            t as i64,
            if contact { 1 } else { 0 },
            label,
        ));
        prev_target_hitlag = victim_hitlag;
        prev_target_grabbed = grabbed;
    }
    if !started {
        return Err(format!(
            "recipe_did_not_produce_move: {}",
            seen_states.join(" > ")
        ));
    }
    Ok(out)
}

#[allow(clippy::too_many_arguments)]
fn snapshot(
    gs: &GameState,
    ch: CharacterId,
    id: MoveId,
    variant: &'static str,
    sample_phase: &'static str,
    tick_index: i64,
    contact: u8,
    label: &'static str,
) -> CaseTick {
    let f = &gs.fighters[0];
    let md = attacks::data(ch, id);
    let in_move = is_move(f.state, id);
    let active = in_move && matches!(f.state, State::Attack { .. }) && md.is_active(f.state_frame);
    let hb = if active { f.hitbox_geometry() } else { None };
    let hitbox_id = if !in_move || !matches!(f.state, State::Attack { .. }) {
        String::new()
    } else if md.is_clean(f.state_frame) {
        "clean".into()
    } else if active {
        "late".into()
    } else {
        String::new()
    };
    let base = Row {
        character_id: character_id(ch),
        action_id: action_id(id),
        variant_id: variant,
        sample_phase,
        tick_index,
        state_frame: f.state_frame,
        hitbox_id,
        hitbox_active: hb.is_some(),
        center: hb.map(|(_, c)| (c.x, c.y)),
        radius: hb.map(|(h, _)| h.radius),
        hitlag_remaining: f.hitlag,
        contact_marker: contact == 1,
    };
    // A live projectile owned by the fighter is a real runtime hitbox
    // (`PROJECTILE_RADIUS` around its position); export it as its own row.
    let projectile = gs
        .projectiles
        .iter()
        .find(|p| p.owner == 0 && p.active)
        .map(|p| Row {
            hitbox_id: "projectile".into(),
            hitbox_active: true,
            center: Some((p.pos.x, p.pos.y)),
            radius: Some(attacks::PROJECTILE_RADIUS),
            ..base.clone()
        });
    CaseTick {
        tick_index,
        state: gs.clone(),
        row: base,
        projectile,
        label,
    }
}

fn csv_row(out: &mut String, sha: &str, r: &Row) {
    let _ = writeln!(
        out,
        "{sha},{},{},{},{},{},{},{},{},{},{},{},{},{}",
        r.character_id,
        r.action_id,
        r.variant_id,
        r.sample_phase,
        r.tick_index,
        r.state_frame,
        r.hitbox_id,
        r.hitbox_active as u8,
        r.center.map(|c| format!("{:.3}", c.0)).unwrap_or_default(),
        r.center.map(|c| format!("{:.3}", c.1)).unwrap_or_default(),
        r.radius.map(|x| format!("{x:.3}")).unwrap_or_default(),
        r.hitlag_remaining,
        r.contact_marker as u8,
    );
}

/// Every (character, action, variant) the exporter knows.
pub fn all_cases() -> Vec<(CharacterId, MoveId, &'static str)> {
    let mut v = Vec::new();
    for ch in CharacterId::ALL {
        v.extend(cases_for(ch));
    }
    v
}

/// Every (action, variant) recipe of one character, in `ALL_MOVES` order.
/// The first variant of an action is its primary one.
pub fn cases_for(ch: CharacterId) -> Vec<(CharacterId, MoveId, &'static str)> {
    let mut v = Vec::new();
    for id in attacks::ALL_MOVES {
        for r in recipes(id) {
            v.push((ch, id, r.variant));
        }
    }
    v
}

/// Does this action define a fighter hitbox at all (throws do not)?
pub fn action_has_hitbox(id: MoveId) -> bool {
    !matches!(
        id,
        MoveId::ThrowF | MoveId::ThrowB | MoveId::ThrowU | MoveId::ThrowD
    )
}

/// Does this action spawn a projectile (its real hitbox lives in
/// `GameState::projectiles`)?
pub fn action_spawns_projectile(id: MoveId) -> bool {
    id == MoveId::SpecialN
}

/// `frame-data.csv` text plus one report per case.
pub fn frame_data_csv(source_sha: &str) -> (String, Vec<CaseReport>) {
    let mut csv = String::from(CSV_HEADER);
    csv.push('\n');
    let mut reports = Vec::new();
    for (ch, id, variant) in all_cases() {
        let recipe = recipes(id)
            .into_iter()
            .find(|r| r.variant == variant)
            .unwrap();
        match run_case(ch, id, variant) {
            Ok(ticks) => {
                for t in &ticks {
                    csv_row(&mut csv, source_sha, &t.row);
                    if let Some(p) = &t.projectile {
                        csv_row(&mut csv, source_sha, p);
                    }
                }
                reports.push(CaseReport {
                    character_id: character_id(ch),
                    action_id: action_id(id),
                    variant_id: variant,
                    status: "exported".into(),
                    rows: ticks.len(),
                    recipe: recipe.description.into(),
                });
            }
            Err(e) => reports.push(CaseReport {
                character_id: character_id(ch),
                action_id: action_id(id),
                variant_id: variant,
                status: e,
                rows: 0,
                recipe: recipe.description.into(),
            }),
        }
    }
    (csv, reports)
}

// ----------------------------------------------------------------- tables

/// `characters.json`: the attribute blocks and hurt capsules the engine uses.
pub fn characters_json() -> serde_json::Value {
    let mut chars = Vec::new();
    for id in CharacterId::ALL {
        let c = id.data();
        let mut moves = Vec::new();
        for m in attacks::ALL_MOVES {
            let md = attacks::data(id, m);
            moves.push(serde_json::json!({
                "action_id": action_id(m),
                "startup": md.startup,
                "active": md.active,
                "late_active": md.late_active,
                "endlag": md.endlag,
                "total": md.total(),
                "is_aerial": md.is_aerial,
                "landing_lag": md.landing_lag,
                "damage": md.hitbox.damage,
                "angle_deg": md.hitbox.angle_deg,
                "bkb": md.hitbox.bkb,
                "kbg": md.hitbox.kbg,
                "hitbox_offset": [md.hitbox.offset.x, md.hitbox.offset.y],
                "hitbox_radius": md.hitbox.radius,
                "late_scale": md.late_scale,
                "electric": md.electric,
            }));
        }
        chars.push(serde_json::json!({
            "character_id": character_id(id),
            "name": c.name,
            "archetype": c.archetype,
            "trait": c.trait_name,
            "height": c.height,
            "half_width": c.half_width,
            "hurt_radius": c.half_width + 1.5,
            "hurt_capsule_note": "vertical capsule; segment from r to max(h-r, r+0.5); h shrinks in crouch/shield/knockdown (Fighter::hurt_segment)",
            "weight": c.weight,
            "walk_max": c.walk_max,
            "dash_max": c.dash_max,
            "run_max": c.run_max,
            "air_max": c.air_max,
            "gravity": c.gravity,
            "max_fall": c.max_fall,
            "fastfall": c.fastfall,
            "fullhop_v": c.fullhop_v,
            "shorthop_v": c.shorthop_v,
            "doublejump_v": c.doublejump_v,
            "air_jumps": c.air_jumps,
            "jumpsquat": c.jumpsquat,
            "traction": c.traction,
            "smash_armor": c.smash_armor,
            "palettes": crate::sim::roster::PALETTES,
            "palette_names": (0..crate::sim::roster::PALETTES).map(|i| crate::model::palettes::get(id, i).name).collect::<Vec<_>>(),
            "moves": moves,
        }));
    }
    serde_json::json!({
        "schema_version": 1,
        "units": "game_units",
        "sim_hz": 60,
        "characters": chars,
    })
}

/// `stages.json`: platforms, ledges, blast zones and spawns from the sim.
pub fn stages_json() -> serde_json::Value {
    let mut stages = Vec::new();
    for id in StageId::ALL {
        let s = crate::sim::Stage::by_id(id);
        stages.push(serde_json::json!({
            "stage_id": stage_id(id),
            "name": s.name,
            "platforms": s.platforms.iter().map(|p| serde_json::json!({
                "left": p.left, "right": p.right, "y": p.y, "solid": p.solid
            })).collect::<Vec<_>>(),
            "ledges": s.ledges.iter().map(|l| serde_json::json!({
                "x": l.pos.x, "y": l.pos.y, "side": l.side
            })).collect::<Vec<_>>(),
            "blast": { "left": s.blast_left, "right": s.blast_right, "top": s.blast_top, "bottom": s.blast_bottom },
            "spawns": s.spawns.iter().map(|v| [v.x, v.y]).collect::<Vec<_>>(),
        }));
    }
    serde_json::json!({ "schema_version": 1, "units": "game_units", "stages": stages })
}

/// Write `runtime/frame-data.csv`, `runtime/characters.json`,
/// `runtime/stages.json` and `runtime/frame-data-report.json` under `dir`.
/// The measurement note emitted with every contact export, split so the
/// projectile-action sentence can state what is true **for this case**
/// instead of asserting one character's geometry for the whole cast.
const METHOD_HEAD: &str = "contact_piece = the strike limb's designated piece, evaluated on the pose and root the renderer draws (model::render_eval: eased facing, extras lag, hitlag rattle) after replaying the case from a fresh render history; its triangles are projected onto the XY fighting plane; mesh_distance = minimum distance from the hitbox centre to that projected surface (0 inside a triangle, else nearest edge); signed_separation = mesh_distance - radius (negative = penetrating); vertex_distance is diagnostic only. Exceptions: throws have no fighter hitbox (null separation); body-centred hitboxes measured on the chest read 0/-radius when the centre is inside the silhouette.";
const METHOD_TAIL: &str = " foot_support/support_reference: `support_reference` is `\"floor\"` when the sample is grounded (the sim keeps a grounded root at floor height, so `support_distance` is real ground clearance) or `\"root_relative_airborne_no_floor\"` when it is not (there is no floor under an airborne root; the same arithmetic then reports the foot's position relative to the root, not clearance from anything). Nothing here changes the simulation.";

/// The per-case measurement note. A projectile action that also carries a
/// fighter hitbox says so with its own real numbers; one declared
/// `no_melee` says plainly that it has none, so nobody reads a null
/// `signed_separation` as a missing measurement. Every other action is
/// unaffected, and mixed rosters stay honest case by case.
fn method_note(md: &attacks::MoveData) -> String {
    let clause = if md.no_melee {
        " This action has no fighter hitbox at all (declared `no_melee` in the move tables): its only causal geometry is the projectile it releases, reported tick by tick as `projectile` and measured against the piece as `projectile_separation`. `hitbox` and `signed_separation` are therefore null on every tick -- an absence the simulation really has, not a measurement that went missing.".to_string()
    } else if md.hitbox.damage == 0.0 && md.hitbox.radius > 0.0 {
        format!(" This action's fighter hitbox deals no damage but still exists and still collides (r={:.0} at [{:.0},{:.0}], active on startup..startup+active+late_active): zero damage is never treated as noncolliding.", md.hitbox.radius, md.hitbox.offset.x, md.hitbox.offset.y)
    } else {
        String::new()
    };
    format!("{METHOD_HEAD}{clause}{METHOD_TAIL}")
}

/// Per-tick contact measurements of one case (both facings), for
/// `runtime/contact/<character>/<action>-<variant>.json`.
pub fn contact_json(
    ch: CharacterId,
    id: MoveId,
    variant: &str,
) -> Result<serde_json::Value, String> {
    use crate::model::render_eval::{evaluate_and_measure, PortState};
    let model = crate::model::characters::build(ch);
    let md = attacks::data(ch, id);
    let mut facings = Vec::new();
    for facing in [1.0f32, -1.0] {
        let ticks = run_case_facing(ch, id, variant, facing)?;
        // A fresh render history per case and facing, replayed tick by
        // tick in order: the same reconstruction the capture tool does, so
        // the measured pose is the drawn one (eased facing included).
        let mut port = PortState::default();
        let mut samples = Vec::new();
        for t in &ticks {
            let f = &t.state.fighters[0];
            let hb = t.row.center.map(|c| {
                (
                    t.row.hitbox_id.as_str(),
                    [c.0, c.1],
                    t.row.radius.unwrap_or(0.0),
                )
            });
            let (ev, m) = evaluate_and_measure(&model, &mut port, f, t.state.frame, true, id, hb);
            // `m` above already measures the contact piece against `hb` —
            // A projectile owner's own fighter hitbox, when it still has
            // one (Boulder, Viper; Kestrel's is removed -- `no_melee`),
            // exactly like every other action. That is not the projectile:
            // the projectile is a second, separate runtime object (spawned
            // on the move's first frame, tracked in its own row), so it
            // gets its own, additional measurement here — never a
            // replacement for the hitbox one above.
            let projectile_separation = t.projectile.as_ref().and_then(|p| {
                let c = p.center?;
                let pm = crate::model::contact::measure_posed(
                    &model,
                    f,
                    &ev.pose,
                    &ev.root,
                    ev.eased_facing,
                    id,
                    Some(("projectile", [c.0, c.1], p.radius.unwrap_or(0.0))),
                )?;
                pm.signed_separation
            });
            // The feet against a Y plane, on the same drawn pose. While
            // grounded, `f.pos.y` really is the floor the fighter stands
            // on (the sim keeps a grounded root at floor height), so
            // `support_distance` is real floor clearance -- the
            // measurement the review asked for. Airborne, there is no
            // floor under the root to measure against: `f.pos.y` is just
            // wherever the fighter currently is in the air, so the same
            // arithmetic only reports the foot's position *relative to
            // the root*, never ground clearance. `support_reference`
            // below says which one a reader is looking at, per sample.
            let support = crate::model::contact::foot_support(&model, &ev.pose, &ev.root, f.pos.y);
            let support_reference = if f.grounded {
                "floor"
            } else {
                "root_relative_airborne_no_floor"
            };
            samples.push(serde_json::json!({
                "eased_facing": ev.eased_facing,
                "render_history": if ev.fresh { "fresh" } else { "continued" },
                "sample_phase": t.row.sample_phase,
                "tick_index": t.tick_index,
                "state_frame": t.row.state_frame,
                "label": t.label,
                "state": format!("{:?}", f.state),
                "root": [f.pos.x, f.pos.y],
                "facing": f.facing,
                "grounded": f.grounded,
                "hitlag_remaining": f.hitlag,
                "contact_marker": t.row.contact_marker,
                "hitbox": t.row.center.map(|c| serde_json::json!({
                    "id": t.row.hitbox_id, "center": [c.0, c.1], "radius": t.row.radius
                })),
                "projectile": t.projectile.as_ref().and_then(|p| p.center.map(|c| serde_json::json!({
                    "center": [c.0, c.1], "radius": p.radius
                }))),
                "capsule": crate::model::contact::capsule(f),
                "contact_piece": m,
                "projectile_separation": projectile_separation,
                "foot_support": support,
                "support_reference": support_reference,
            }));
        }
        facings.push(serde_json::json!({ "facing": facing, "samples": samples }));
    }
    Ok(serde_json::json!({
        "schema_version": 1,
        "character_id": character_id(ch),
        "action_id": action_id(id),
        "variant_id": variant,
        "units": "game_units",
        "method": method_note(&md),
        "facings": facings,
    }))
}

/// Everything the animation-direction round has to answer for one
/// character: which direction file was implemented (by content hash), what
/// each action's contact solve achieved against the runtime hit region, and
/// where the drawn feet sit relative to the plane the fighter stands on.
///
/// It reports rather than decides: an action the geometry cannot satisfy
/// comes out with `guaranteed_intersection: false` and a note, never with a
/// simulation, hitbox, capsule, scale or bone-length change behind it.
pub fn animation_json(ch: CharacterId) -> serde_json::Value {
    use crate::model::anim_directed as ad;
    let model = crate::model::characters::build(ch);
    let Some(dirs) = model.directions.as_ref() else {
        return serde_json::json!({
            "schema_version": 1,
            "character_id": character_id(ch),
            "status": "no_direction_file",
        });
    };
    let actions = ad::feasibility(&model);
    let unreachable: Vec<&str> = actions
        .iter()
        .filter(|a| a.intersection_required && !a.guaranteed_intersection)
        .map(|a| a.action_id)
        .collect();
    // The support states the review named: idle, crouch, hitstun and the
    // grounded recoveries (landing lag, shield stun, and the first recovery
    // tick of every grounded action).
    let mut support = Vec::new();
    let mut push_state = |name: String, st: State, sf: u32| {
        let mut f = crate::sim::fighter::Fighter::new(ch.data(), 0, Vec2::ZERO);
        f.grounded = true;
        f.facing = 1.0;
        f.set_state_pub(st);
        f.state_frame = sf;
        let pose = crate::model::anim_directed::fighter_pose(&model, &f, 90);
        let feet = crate::model::contact::foot_support(
            &model,
            &pose,
            &crate::model::math3::Xf::IDENTITY,
            0.0,
        );
        let lowest = feet
            .iter()
            .map(|x| x.support_distance)
            .fold(f32::MAX, f32::min);
        // Two real, finite foot measurements or this is not evidence: an
        // empty/short list folding to f32::MAX must never be written out
        // as if it were a measured value.
        assert!(
            feet.len() == 2 && lowest.is_finite(),
            "{name}: expected 2 finite foot measurements, got {feet:?}"
        );
        support.push(serde_json::json!({
            "state": name,
            "state_frame": sf,
            "feet": feet,
            "lowest_support_distance": lowest,
        }));
    };
    push_state("idle".into(), State::Stand, 6);
    push_state("crouch".into(), State::Crouch, 6);
    push_state("hitstun".into(), State::Hitstun { tumble: false }, 6);
    push_state("land_lag".into(), State::LandLag { total: 12 }, 1);
    push_state("shield_stun".into(), State::ShieldStun { total: 8 }, 2);
    for id in attacks::ALL_MOVES {
        let md = attacks::data(ch, id);
        if md.is_aerial || !action_has_hitbox(id) {
            continue;
        }
        let last = md.startup + md.active + md.late_active;
        push_state(
            format!("recovery:{}", action_id(id)),
            State::Attack { id, aerial: false },
            last,
        );
    }
    serde_json::json!({
        "schema_version": 1,
        "character_id": character_id(ch),
        "units": "game_units",
        "direction_file": format!("docs/art/procedural/anim/{}.json", character_id(ch)),
        "direction_sha256": dirs.sha256,
        "direction_status": dirs.status,
        "direction_source_sha": dirs.source_sha,
        "direction_evidence_commit": dirs.evidence_commit,
        "probe_kind": "synthetic",
        "method": "SYNTHETIC PROBE, not the real per-frame measurement: poses here are solved once, in the fighter's root-local frame, on a bare one-shot Fighter with no replayed render history, no eased facing, no hitlag rattle and no support_lift -- none of what model::render_eval::evaluate + model::contact::measure_posed apply for the real capture/contact_json pipeline. The contact piece named by the direction is placed on the runtime hit region by two-link inverse kinematics through the real joint chain, with the hitbox offset used signed (a rearward move aims rearward). `centroid_gap` is the distance from the achieved piece centroid to the aim point; the projected piece is convex, so a gap no larger than the radius proves intersection (a convex body's surface can only be nearer to an outside point than its centroid) -- a conservative, sufficient-but-not-necessary condition, always stricter than the real mesh-surface signed_separation reported by contact_json. `guaranteed_intersection: true` here is not proof the real per-frame contact passes; only contact_json's signed_separation is that proof. Anticipation peaks at the authored fraction of the available inactive startup, contact holds through startup+active+late_active, recovery starts after the real last active tick. Support distances are the lowest foot-piece vertex minus the plane the fighter stands on. No simulation value is written.",
        "actions": actions,
        "unreachable_actions": unreachable,
        "support": support,
    })
}

/// Write `runtime/frame-data.csv`, `runtime/characters.json`,
/// `runtime/stages.json`, `runtime/frame-data-report.json` and, for
/// `contact_characters`, `runtime/contact/<character>/<action>-<variant>.json`.
pub fn write_runtime_for(
    dir: &std::path::Path,
    source_sha: &str,
    contact_characters: &[CharacterId],
) -> std::io::Result<Vec<CaseReport>> {
    let rt = dir.join("runtime");
    std::fs::create_dir_all(&rt)?;
    let (csv, reports) = frame_data_csv(source_sha);
    std::fs::write(rt.join("frame-data.csv"), csv)?;
    let mut contact_report = Vec::new();
    for ch in contact_characters {
        for (ch, id, variant) in cases_for(*ch) {
            let rel = format!(
                "runtime/contact/{}/{}-{}.json",
                character_id(ch),
                action_id(id),
                variant
            );
            match contact_json(ch, id, variant) {
                Ok(j) => {
                    let d = rt.join("contact").join(character_id(ch));
                    std::fs::create_dir_all(&d)?;
                    // Compact: these files are read by tools, not people.
                    std::fs::write(
                        d.join(format!("{}-{}.json", action_id(id), variant)),
                        serde_json::to_string(&j).unwrap_or_default(),
                    )?;
                    contact_report.push(serde_json::json!({
                        "character_id": character_id(ch), "action_id": action_id(id),
                        "variant_id": variant, "path": rel, "status": "written",
                    }));
                }
                Err(e) => contact_report.push(serde_json::json!({
                    "character_id": character_id(ch), "action_id": action_id(id),
                    "variant_id": variant, "path": rel, "status": format!("error: {e}"),
                })),
            }
        }
    }
    std::fs::write(
        rt.join("contact-report.json"),
        serde_json::to_string_pretty(&contact_report).unwrap_or_default(),
    )?;
    // Animation-direction evidence for the characters this run measures.
    for ch in contact_characters {
        let d = rt.join("animation");
        std::fs::create_dir_all(&d)?;
        std::fs::write(
            d.join(format!("{}.json", character_id(*ch))),
            serde_json::to_string_pretty(&animation_json(*ch)).unwrap_or_default(),
        )?;
    }
    std::fs::write(
        rt.join("frame-data-report.json"),
        serde_json::to_string_pretty(&reports).unwrap_or_default(),
    )?;
    std::fs::write(
        rt.join("characters.json"),
        serde_json::to_string_pretty(&characters_json()).unwrap_or_default(),
    )?;
    std::fs::write(
        rt.join("stages.json"),
        serde_json::to_string_pretty(&stages_json()).unwrap_or_default(),
    )?;
    Ok(reports)
}

/// [`write_runtime_for`] with contact measurements for every character.
pub fn write_runtime(dir: &std::path::Path, source_sha: &str) -> std::io::Result<Vec<CaseReport>> {
    write_runtime_for(dir, source_sha, &CharacterId::ALL)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixtures_parse_and_place_players_where_declared() {
        for json in [FIXTURE_IDLE, FIXTURE_COMBAT] {
            let f = Fixture::parse(json).expect("fixture");
            let gs = f.initial_state();
            assert_eq!(gs.fighters.len(), f.players.len());
            for (i, p) in f.players.iter().enumerate() {
                let g = &gs.fighters[i];
                assert_eq!(g.pos.x, p.x);
                assert_eq!(g.facing, p.facing);
                assert!(g.grounded);
            }
            // After the warmup with no inputs they are still standing there.
            let mut gs = f.initial_state();
            for t in -(f.warmup_ticks as i64)..0 {
                gs.step(&f.inputs_at(t));
            }
            for (i, p) in f.players.iter().enumerate() {
                let g = &gs.fighters[i];
                assert!(
                    (g.pos.x - p.x).abs() < 1e-3,
                    "{}: drifted to {}",
                    f.id,
                    g.pos.x
                );
                assert!(matches!(g.state, State::Stand), "{}: {:?}", f.id, g.state);
            }
        }
    }

    #[test]
    fn idle_fixture_is_still_at_its_capture_tick() {
        let f = Fixture::parse(FIXTURE_IDLE).unwrap();
        let gs = f.state_at(120);
        for (i, p) in f.players.iter().enumerate() {
            assert!(matches!(gs.fighters[i].state, State::Stand));
            assert!((gs.fighters[i].pos.x - p.x).abs() < 1e-3);
        }
    }

    #[test]
    fn combat_fixture_has_movement_a_jump_and_a_contact() {
        let f = Fixture::parse(FIXTURE_COMBAT).unwrap();
        let mut gs = f.initial_state();
        let mut jumped = false;
        let mut contact = false;
        let mut moved = 0.0f32;
        let start_x = gs.fighters[0].pos.x;
        for t in -(f.warmup_ticks as i64)..(f.ticks as i64) {
            gs.step(&f.inputs_at(t));
            if !gs.fighters[0].grounded {
                jumped = true;
            }
            if gs.fighters[1].hitlag > 0 {
                contact = true;
            }
            moved = moved.max((gs.fighters[0].pos.x - start_x).abs());
        }
        assert!(jumped, "no jump in the combat fixture");
        assert!(moved > 20.0, "no real movement: {moved}");
        assert!(contact, "no contact in the combat fixture");
        assert!(
            gs.fighters.iter().all(|g| g.stocks == f.stocks),
            "nobody dies"
        );
    }

    #[test]
    fn every_recipe_produces_its_move_on_every_character() {
        let (csv, reports) = frame_data_csv("test");
        assert!(csv.starts_with(CSV_HEADER));
        let failed: Vec<String> = reports
            .iter()
            .filter(|r| r.status != "exported")
            .map(|r| {
                format!(
                    "{}/{}/{}: {}",
                    r.character_id, r.action_id, r.variant_id, r.status
                )
            })
            .collect();
        assert!(failed.is_empty(), "{}", failed.join("\n"));
        // Every case shows the causal geometry it really has: an active
        // fighter hitbox, or -- for an action whose whole effect is what it
        // spawns -- a real projectile row. Throws have neither by design.
        // An action declared `no_melee` must show no fighter hitbox at all,
        // on any tick: that is the point of the declaration.
        let (csv_ref, _) = (&csv, ());
        for (ch, id, variant) in all_cases() {
            if matches!(
                id,
                MoveId::ThrowF | MoveId::ThrowB | MoveId::ThrowU | MoveId::ThrowD
            ) {
                continue;
            }
            let md = attacks::data(ch, id);
            let ticks = run_case(ch, id, variant).unwrap();
            let melee = ticks.iter().any(|t| t.row.hitbox_active);
            let projectile = ticks.iter().any(|t| t.projectile.is_some());
            if md.no_melee {
                assert!(
                    !melee,
                    "{}/{}/{} declares no_melee but still exports an active fighter hitbox",
                    character_id(ch),
                    action_id(id),
                    variant
                );
                assert!(
                    projectile,
                    "{}/{}/{} declares no_melee, so its projectile must be the real event",
                    character_id(ch),
                    action_id(id),
                    variant
                );
                continue;
            }
            assert!(
                melee,
                "{}/{}/{} never shows an active hitbox",
                character_id(ch),
                action_id(id),
                variant
            );
        }
        let _ = csv_ref;
        // Column count of every row matches the header, and the phase
        // column only carries the two agreed values.
        let cols = CSV_HEADER.split(',').count();
        for line in csv.lines().skip(1) {
            assert_eq!(line.split(',').count(), cols, "{line}");
            let phase = line.split(',').nth(4).unwrap();
            assert!(phase == "after_step" || phase == "pre_step", "{line}");
        }
    }

    #[test]
    fn context_row_provenance_is_explicit() {
        // Kestrel's jab starts on its first input tick: the context row is
        // the pre_step state of tick 0, never a fabricated "tick -1".
        let jab = run_case(CharacterId::Kestrel, MoveId::Jab, "ground").unwrap();
        assert_eq!(jab[0].row.sample_phase, "pre_step");
        assert_eq!(jab[0].tick_index, 0);
        assert_eq!(jab[0].label, "pre_step");
        assert_eq!(jab[1].row.sample_phase, "after_step");
        assert_eq!(jab[1].tick_index, 0);
        assert!(jab.iter().all(|t| t.tick_index >= 0));
        // An aerial starts after the hop: its context row is a real
        // after_step sample of the previous tick.
        let nair = run_case(CharacterId::Kestrel, MoveId::Nair, "fullhop").unwrap();
        assert_eq!(nair[0].row.sample_phase, "after_step");
        assert_eq!(nair[0].tick_index + 1, nair[1].tick_index);
    }

    #[test]
    fn projectile_and_throw_cases_carry_real_runtime_evidence() {
        let sn = run_case(CharacterId::Kestrel, MoveId::SpecialN, "ground").unwrap();
        let with_proj: Vec<&CaseTick> = sn.iter().filter(|t| t.projectile.is_some()).collect();
        assert!(
            !with_proj.is_empty(),
            "special_n never shows its projectile"
        );
        let p = with_proj[0].projectile.as_ref().unwrap();
        assert_eq!(p.hitbox_id, "projectile");
        assert_eq!(p.radius, Some(attacks::PROJECTILE_RADIUS));
        assert!(p.center.unwrap().0 > 0.0, "projectile travels forward");
        // Throws: no fighter hitbox, but a measured release tick.
        for id in [
            MoveId::ThrowF,
            MoveId::ThrowB,
            MoveId::ThrowU,
            MoveId::ThrowD,
        ] {
            let t = run_case(CharacterId::Kestrel, id, "from_grab").unwrap();
            assert!(
                t.iter().all(|x| !x.row.hitbox_active),
                "{:?} has no hitbox",
                id
            );
            assert!(
                t.iter()
                    .any(|x| x.label == "release" && x.row.contact_marker),
                "{:?} never releases the victim",
                id
            );
        }
        assert!(!action_has_hitbox(MoveId::ThrowU));
        assert!(action_spawns_projectile(MoveId::SpecialN));
    }

    #[test]
    fn kestrel_has_twenty_three_cases_covering_all_twenty_two_actions() {
        let cases = cases_for(CharacterId::Kestrel);
        assert_eq!(cases.len(), 23);
        for id in attacks::ALL_MOVES {
            assert!(cases.iter().any(|c| c.1 == id), "{:?} has no recipe", id);
        }
        assert!(cases
            .iter()
            .any(|c| c.1 == MoveId::Fsmash && c.2 == "charged_max"));
        assert!(cases
            .iter()
            .any(|c| c.1 == MoveId::Jab2 && c.2 == "after_jab1_hit"));
    }

    #[test]
    fn contact_json_has_both_facings_and_measures() {
        let j = contact_json(CharacterId::Kestrel, MoveId::Ftilt, "ground").unwrap();
        let facings = j["facings"].as_array().unwrap();
        assert_eq!(facings.len(), 2);
        let active = facings[0]["samples"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["label"] == "active")
            .expect("an active sample");
        assert!(active["contact_piece"]["signed_separation"].is_number());
        assert!(active["contact_piece"]["mesh_distance"].is_number());
        assert_eq!(active["contact_piece"]["bone"], "foot_r");
        assert_eq!(active["eased_facing"], 1.0);
        assert_eq!(facings[1]["samples"][0]["render_history"], "fresh");
        assert!(active["capsule"]["radius"].as_f64().unwrap() > 0.0);
    }

    #[test]
    fn exported_hitboxes_match_the_engine_window() {
        // Kestrel jab: startup frames inactive, then `active` clean frames.
        let ticks = run_case(CharacterId::Kestrel, MoveId::Jab, "ground").unwrap();
        let md = attacks::data(CharacterId::Kestrel, MoveId::Jab);
        let active_rows: Vec<&CaseTick> = ticks.iter().filter(|t| t.row.hitbox_active).collect();
        assert_eq!(active_rows.len() as u32, md.active + md.late_active);
        // First active tick shows state_frame == startup (1-based frame == startup).
        assert_eq!(active_rows[0].row.state_frame, md.startup);
        // Geometry is where the engine put it: forward of the fighter.
        let c = active_rows[0].row.center.unwrap();
        let f = &active_rows[0].state.fighters[0];
        assert!(c.0 > f.pos.x, "jab hitbox in front");
        assert_eq!(active_rows[0].row.radius, Some(md.hitbox.radius));
        // The charged smash reports a bigger damage window than the flick one
        // (same geometry, more rows because of the charge hold).
        let a = run_case(CharacterId::Kestrel, MoveId::Fsmash, "cstick").unwrap();
        let b = run_case(CharacterId::Kestrel, MoveId::Fsmash, "charged_max").unwrap();
        assert!(b.len() > a.len() + 30, "charge hold shows in the row count");
    }

    #[test]
    fn the_animation_evidence_reports_every_direction_and_the_support_plane() {
        let j = animation_json(CharacterId::Kestrel);
        assert_eq!(j["character_id"], "kestrel");
        assert_eq!(j["direction_sha256"].as_str().unwrap().len(), 64);
        // The probe is labelled synthetic in the JSON itself, not only in
        // Rust doc comments -- a reader of the exported evidence must not
        // mistake `guaranteed_intersection` here for the real per-frame
        // contact result.
        assert_eq!(j["probe_kind"], "synthetic");
        assert!(j["method"].as_str().unwrap().contains("SYNTHETIC"));
        let actions = j["actions"].as_array().unwrap();
        assert_eq!(actions.len(), 22, "one entry per runtime action");
        // Every action that has a fighter hitbox intersects it with the
        // piece the direction names; the declared exceptions say why not.
        // An action that has one is never excused from it -- including a
        // projectile owner that also swings (Boulder's and Viper's
        // special_n). Kestrel's special_n has none at all any more
        // (`no_melee`), so it has nothing to intersect and says so: the
        // exemption comes from the simulation, not from an exception list.
        for a in actions {
            let id = a["action_id"].as_str().unwrap();
            if a["intersection_required"].as_bool().unwrap() {
                assert!(
                    a["guaranteed_intersection"].as_bool().unwrap(),
                    "{id}: {}",
                    a["note"]
                );
            } else {
                assert!(a["note"].is_string(), "{id}: an exception must say why");
            }
            assert!(!a["contact_piece_id"].as_str().unwrap().is_empty());
        }
        assert!(j["unreachable_actions"].as_array().unwrap().is_empty());
        // The exact pieces the review named.
        let piece = |id: &str| {
            actions.iter().find(|a| a["action_id"] == id).unwrap()["contact_piece_id"]
                .as_str()
                .unwrap()
                .to_string()
        };
        assert_eq!(piece("utilt"), "k_hand_r");
        assert_eq!(piece("dash_attack"), "k_hand_r");
        assert_eq!(piece("special_side"), "k_hand_r");
        assert_eq!(piece("special_down"), "k_chest");
        assert_eq!(piece("bair"), "k_foot_l");
        // No drawn foot goes through the floor in any support state.
        let support = j["support"].as_array().unwrap();
        assert!(support.len() > 5);
        for s in support {
            let d = s["lowest_support_distance"].as_f64().unwrap();
            assert!(d > -1e-3, "{}: foot {d} below the floor", s["state"]);
        }
    }

    #[test]
    fn every_active_contact_sample_intersects_its_hitbox() {
        // A real, per-tick regression guard over the actual measurement
        // pipeline (model::render_eval::evaluate + model::contact::
        // measure_posed, via contact_json) -- not the synthetic feasibility
        // probe in model::anim_directed::feasibility, which never replayed
        // a render history, applied eased facing/hitlag rattle, or ran
        // support_lift. Every active (clean and late) sample of every one
        // of Kestrel's 23 exported variants, both facings, must show its
        // named contact piece intersecting the real fighter hitbox, with
        // no exception: per review 91f9c5a2834353bbb7207dcc4d866a4e513fd48f
        // / art commit 343def9c8c45ec69dba36b2b7ce863255ee9b0e4, an event
        // that exists is measured and corrected like any other, never left
        // as a permanent restriction. An action with no fighter hitbox at
        // all (Kestrel's special_n, `no_melee`) has no active sample to
        // check -- the absence is in the simulation, not in this test. `signed_separation` itself must be present and
        // finite for every active sample -- a missing/NaN value is a
        // defect in the measurement, never something to silently skip
        // past. This also catches a regression like the review's original
        // bair (+21.010283u) or nair late-window (+8.329576u) defects.
        const TOL: f32 = 1e-3;
        let model = crate::model::characters::build(CharacterId::Kestrel);
        let dirs = model.directions.as_ref().unwrap();
        let mut offenders: Vec<(&str, &str, f64, i64, f64)> = Vec::new();
        let mut checked_active = 0usize;
        for (ch, id, variant) in cases_for(CharacterId::Kestrel) {
            let j = contact_json(ch, id, variant).unwrap();
            let expected_piece = dirs.get(id).map(|d| d.piece_id.clone());
            for fc in j["facings"].as_array().unwrap() {
                let facing = fc["facing"].as_f64().unwrap();
                for s in fc["samples"].as_array().unwrap() {
                    // support_reference must agree with `grounded` on every
                    // sample, active or not (the review's grounded-foot
                    // check and the airborne "no floor here" label both
                    // depend on this).
                    let grounded = s["grounded"].as_bool().unwrap();
                    let sref = s["support_reference"].as_str().unwrap();
                    assert_eq!(
                        sref,
                        if grounded {
                            "floor"
                        } else {
                            "root_relative_airborne_no_floor"
                        },
                        "{}/{variant} f{facing} sf{}: support_reference disagrees with grounded",
                        action_id(id),
                        s["state_frame"]
                    );
                    if s["hitbox"].is_null() {
                        continue;
                    }
                    checked_active += 1;
                    let cp = &s["contact_piece"];
                    if let Some(exp) = &expected_piece {
                        let ids: Vec<String> = cp["piece_ids"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|v| v.as_str().unwrap().to_string())
                            .collect();
                        assert_eq!(
                            ids,
                            vec![exp.clone()],
                            "{}/{variant} f{facing} sf{}: contact piece should be exactly {exp}",
                            action_id(id),
                            s["state_frame"]
                        );
                    }
                    // signed_separation must be present and finite for
                    // every active sample -- never a silent `continue`
                    // past a gap in the measurement itself.
                    let sep = cp["signed_separation"].as_f64().unwrap_or_else(|| {
                        panic!(
                            "{}/{variant} f{facing} sf{}: signed_separation missing",
                            action_id(id),
                            s["state_frame"]
                        )
                    });
                    assert!(
                        sep.is_finite(),
                        "{}/{variant} f{facing} sf{}: signed_separation is not finite ({sep})",
                        action_id(id),
                        s["state_frame"]
                    );
                    if sep as f32 > TOL {
                        offenders.push((
                            action_id(id),
                            variant,
                            facing,
                            s["state_frame"].as_i64().unwrap_or(-1),
                            sep,
                        ));
                    }
                }
            }
        }
        assert!(
            checked_active > 100,
            "sanity: {checked_active} active samples checked"
        );
        assert!(
            offenders.is_empty(),
            "unexpected non-intersecting active contact: {offenders:?}"
        );
        // The exact pieces the review named, on the real measured samples.
        let piece_used = |id: MoveId, variant: &str| -> String {
            let j = contact_json(CharacterId::Kestrel, id, variant).unwrap();
            let s = j["facings"][0]["samples"]
                .as_array()
                .unwrap()
                .iter()
                .find(|s| !s["hitbox"].is_null())
                .expect("an active sample exists");
            s["contact_piece"]["piece_ids"][0]
                .as_str()
                .unwrap()
                .to_string()
        };
        assert_eq!(piece_used(MoveId::Utilt, "ground"), "k_hand_r");
        assert_eq!(piece_used(MoveId::DashAttack, "from_dash"), "k_hand_r");
        assert_eq!(piece_used(MoveId::SpecialSide, "ground"), "k_hand_r");
        assert_eq!(piece_used(MoveId::SpecialDown, "ground"), "k_chest");
        assert_eq!(piece_used(MoveId::Bair, "fullhop"), "k_foot_l");
    }

    #[test]
    fn the_contact_export_carries_support_and_the_real_projectile_geometry() {
        // Every sample knows where the feet are relative to the plane.
        let j = contact_json(CharacterId::Kestrel, MoveId::Ftilt, "ground").unwrap();
        let s = &j["facings"][0]["samples"][0];
        assert!(s["foot_support"].as_array().unwrap().len() == 2);
        assert!(s["foot_support"][0]["support_distance"].is_number());
        // The projectile action leads the *emission line* — the palm is on
        // the runtime spawn point when the shot appears — and then stays
        // there while the projectile travels (it is never chased). The
        // exported projectile row is already one integration step
        // downrange, which is exactly why the pose is judged against the
        // spawn event and not against that row.
        let j = contact_json(CharacterId::Kestrel, MoveId::SpecialN, "ground").unwrap();
        let samples = j["facings"][0]["samples"].as_array().unwrap();
        let spawn = samples
            .iter()
            .position(|s| !s["projectile"].is_null())
            .expect("the case shows its projectile");
        let emission = |s: &serde_json::Value| {
            let c = s["contact_piece"]["centroid"].as_array().unwrap();
            let root = s["root"].as_array().unwrap();
            let f = s["facing"].as_f64().unwrap();
            let ex = root[0].as_f64().unwrap()
                + f * crate::model::anim_directed::PROJECTILE_SPAWN_LOCAL_X as f64;
            let ey = root[1].as_f64().unwrap() + 15.0;
            ((c[0].as_f64().unwrap() - ex).powi(2) + (c[1].as_f64().unwrap() - ey).powi(2)).sqrt()
        };
        let at_spawn = emission(&samples[spawn]);
        assert!(
            at_spawn < attacks::PROJECTILE_RADIUS as f64,
            "the palm leads the emission point: {at_spawn}u"
        );
        assert!(samples[spawn]["projectile_separation"].is_number());
        // Two ticks later the projectile is 14 units further on and the
        // hand has not followed it.
        if let Some(later) = samples.get(spawn + 2) {
            assert!(
                (emission(later) - at_spawn).abs() < 3.0,
                "the palm stays on the emission line: {} vs {at_spawn}",
                emission(later)
            );
        }
    }

    #[test]
    fn runtime_tables_reflect_the_sim() {
        let c = characters_json();
        assert_eq!(c["characters"].as_array().unwrap().len(), 3);
        assert_eq!(c["characters"][0]["height"], 30.0);
        assert_eq!(c["characters"][0]["hurt_radius"], 10.5);
        let s = stages_json();
        assert_eq!(s["stages"][0]["platforms"].as_array().unwrap().len(), 4);
    }
}

#[cfg(test)]
mod diag {
    use super::*;
    /// `cargo test --lib diag -- --ignored --nocapture` prints the combat fixture.
    #[test]
    #[ignore]
    fn print_combat_fixture() {
        let f = Fixture::parse(FIXTURE_COMBAT).unwrap();
        let mut gs = f.initial_state();
        for t in -(f.warmup_ticks as i64)..(f.ticks as i64) {
            gs.step(&f.inputs_at(t));
            if t >= 0 && (t % 6 == 0 || gs.fighters[1].hitlag > 0) {
                let a = &gs.fighters[0];
                let b = &gs.fighters[1];
                println!(
                    "t{t:>3} P1 x={:>6.1} y={:>5.1} {:<22} | P2 x={:>6.1} y={:>5.1} {:?} hl={}",
                    a.pos.x,
                    a.pos.y,
                    format!("{:?}", a.state),
                    b.pos.x,
                    b.pos.y,
                    b.state,
                    b.hitlag
                );
            }
        }
    }
}

#[cfg(test)]
mod diag_contact {
    use super::*;
    /// `cargo test --lib diag_contact -- --ignored --nocapture` prints, per
    /// Kestrel case and facing, the signed separation of every active tick.
    #[test]
    #[ignore]
    fn print_contact_gaps() {
        for (ch, id, variant) in cases_for(CharacterId::Kestrel) {
            let j = contact_json(ch, id, variant).unwrap();
            for fc in j["facings"].as_array().unwrap() {
                let facing = fc["facing"].as_f64().unwrap();
                let mut worst = f64::MIN;
                let mut n_active = 0;
                let mut n_pos = 0;
                let mut seps = Vec::new();
                for s in fc["samples"].as_array().unwrap() {
                    if s["hitbox"].is_null() {
                        continue;
                    }
                    n_active += 1;
                    let sep = s["contact_piece"]["signed_separation"].as_f64();
                    if let Some(v) = sep {
                        if v > 0.0 {
                            n_pos += 1;
                        }
                        worst = worst.max(v);
                        seps.push(format!("{}:{:.1}", s["state_frame"], v));
                    }
                }
                println!(
                    "{:<13}{:<15} f{:+} active={:<3} positive={:<3} worst={:>8.3}  [{}]",
                    action_id(id),
                    variant,
                    facing,
                    n_active,
                    n_pos,
                    if worst == f64::MIN { f64::NAN } else { worst },
                    seps.join(" ")
                );
            }
        }
    }
}

#[cfg(test)]
mod diag_anim {
    use super::*;
    /// `cargo test --lib diag_anim -- --ignored --nocapture` prints the
    /// animation feasibility and support tables.
    #[test]
    #[ignore]
    fn print_animation_evidence() {
        let j = animation_json(CharacterId::Kestrel);
        for a in j["actions"].as_array().unwrap() {
            println!(
                "{:<13}{:<18}{:<11} target({:>6.1},{:>5.1}) r{:>5.2} need{:>6.2} reach{:>6.2} gap{:>6.2} ok={} assist{:>5.1}  {}",
                a["action_id"].as_str().unwrap(),
                a["pose_family"].as_str().unwrap(),
                a["contact_piece_id"].as_str().unwrap(),
                a["target_local"][0].as_f64().unwrap(),
                a["target_local"][1].as_f64().unwrap(),
                a["hit_radius"].as_f64().unwrap(),
                a["required_reach"].as_f64().unwrap(),
                a["max_reach"].as_f64().unwrap(),
                a["centroid_gap"].as_f64().unwrap(),
                a["guaranteed_intersection"],
                a["lean_assist_deg"].as_f64().unwrap(),
                a["note"].as_str().unwrap_or(""),
            );
        }
        println!("unreachable: {}", j["unreachable_actions"]);
        for s in j["support"].as_array().unwrap() {
            println!(
                "support {:<22} lowest {:>7.3}",
                s["state"].as_str().unwrap(),
                s["lowest_support_distance"].as_f64().unwrap()
            );
        }
    }
}

#[cfg(test)]
mod method_note_tests {
    use super::*;

    #[test]
    fn the_emitted_method_note_states_each_case_truthfully() {
        // Kestrel's projectile action has no fighter hitbox at all; the
        // note must say so rather than assert a hitbox that is gone.
        let k = contact_json(CharacterId::Kestrel, MoveId::SpecialN, "ground").unwrap();
        let note = k["method"].as_str().unwrap();
        assert!(
            note.contains("no fighter hitbox at all"),
            "kestrel special_n note: {note}"
        );
        assert!(
            !note.contains("[16,21]"),
            "the old hard-coded claim is gone"
        );
        // …and the other owners keep theirs, with their own real numbers,
        // including the fact that zero damage still collides.
        for (ch, off) in [
            (CharacterId::Boulder, "[18,7]"),
            (CharacterId::Viper, "[15,6]"),
        ] {
            let j = contact_json(ch, MoveId::SpecialN, "ground").unwrap();
            let n = j["method"].as_str().unwrap();
            assert!(
                n.contains("still exists and still collides") && n.contains(off),
                "{ch:?} special_n note: {n}"
            );
        }
        // An ordinary action says nothing extra either way.
        let f = contact_json(CharacterId::Kestrel, MoveId::Ftilt, "ground").unwrap();
        let n = f["method"].as_str().unwrap();
        assert!(!n.contains("no fighter hitbox at all") && !n.contains("still collides"));
    }
}
