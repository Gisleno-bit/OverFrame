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
    /// Ticks since the recipe's first input; `-1` is the context row (the
    /// tick before the move began).
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
    pub row: Row,
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
    let target_x = recipe.target.unwrap_or(120.0);
    place_on_main(
        &mut gs,
        &[
            FixturePlayer {
                character: character_id(ch).into(),
                palette: 0,
                x: 0.0,
                facing: 1.0,
            },
            FixturePlayer {
                character: "boulder".into(),
                palette: 1,
                x: target_x,
                facing: -1.0,
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
    for t in 0..240u32 {
        let inp = [(recipe.script)(t), PlayerInput::default()];
        let before = gs.clone();
        gs.step(&inp);
        let f = &gs.fighters[0];
        let active = is_move(f.state, id);
        if !started {
            if active {
                started = true;
                // Context row: the tick before the move began.
                if let Some(p) = prev.take().or(Some(before)) {
                    out.push(snapshot(&p, ch, id, recipe.variant, t as i64 - 1, 0));
                }
            } else {
                let s = format!("{:?}", f.state);
                if seen_states.last() != Some(&s) {
                    seen_states.push(s);
                }
                prev = Some(gs.clone());
                continue;
            }
        } else if !active {
            // One row after the move: shows the recovery state.
            out.push(snapshot(&gs, ch, id, recipe.variant, t as i64, 0));
            break;
        }
        let victim_hitlag = gs.fighters[1].hitlag;
        let contact = victim_hitlag > 0 && prev_target_hitlag == 0;
        out.push(snapshot(
            &gs,
            ch,
            id,
            recipe.variant,
            t as i64,
            if contact { 1 } else { 0 },
        ));
        prev_target_hitlag = victim_hitlag;
    }
    if !started {
        return Err(format!(
            "recipe_did_not_produce_move: {}",
            seen_states.join(" > ")
        ));
    }
    Ok(out)
}

fn snapshot(
    gs: &GameState,
    ch: CharacterId,
    id: MoveId,
    variant: &'static str,
    tick_index: i64,
    contact: u8,
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
    CaseTick {
        tick_index,
        state: gs.clone(),
        row: Row {
            character_id: character_id(ch),
            action_id: action_id(id),
            variant_id: variant,
            tick_index,
            state_frame: f.state_frame,
            hitbox_id,
            hitbox_active: hb.is_some(),
            center: hb.map(|(_, c)| (c.x, c.y)),
            radius: hb.map(|(h, _)| h.radius),
            hitlag_remaining: f.hitlag,
            contact_marker: contact == 1,
        },
    }
}

fn csv_row(out: &mut String, sha: &str, r: &Row) {
    let _ = writeln!(
        out,
        "{sha},{},{},{},after_step,{},{},{},{},{},{},{},{},{}",
        r.character_id,
        r.action_id,
        r.variant_id,
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
        for id in attacks::ALL_MOVES {
            for r in recipes(id) {
                v.push((ch, id, r.variant));
            }
        }
    }
    v
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
pub fn write_runtime(dir: &std::path::Path, source_sha: &str) -> std::io::Result<Vec<CaseReport>> {
    let rt = dir.join("runtime");
    std::fs::create_dir_all(&rt)?;
    let (csv, reports) = frame_data_csv(source_sha);
    std::fs::write(rt.join("frame-data.csv"), csv)?;
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
        // Every case has at least one active hitbox tick, except throws
        // (no hitbox by design).
        let (csv_ref, _) = (&csv, ());
        for (ch, id, variant) in all_cases() {
            if matches!(
                id,
                MoveId::ThrowF | MoveId::ThrowB | MoveId::ThrowU | MoveId::ThrowD
            ) {
                continue;
            }
            let ticks = run_case(ch, id, variant).unwrap();
            assert!(
                ticks.iter().any(|t| t.row.hitbox_active),
                "{}/{}/{} never shows an active hitbox",
                character_id(ch),
                action_id(id),
                variant
            );
        }
        let _ = csv_ref;
        // Column count of every row matches the header.
        let cols = CSV_HEADER.split(',').count();
        for line in csv.lines().skip(1) {
            assert_eq!(line.split(',').count(), cols, "{line}");
        }
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
