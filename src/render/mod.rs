//! macroquad front-end: window, menus, character/stage select, options, the LAN
//! room browser, the online lobby, and the fixed-timestep game + GGRS loops.
//! All in-match drawing goes through [`crate::viz`]; the menus draw with the
//! same bitmap font so nothing needs a font file.

mod audio;
pub mod capture;
mod input;
mod scene3d;
mod widgets;

use macroquad::prelude::*;

use crate::config::Settings;
use crate::gamepad::{mask_label, PadAction};
use crate::netcode::banlist::BanList;
use crate::netcode::{roomcode, Advance, Browser, NetMatch, Phase, Role};
use crate::sim::roster::CharacterId;
use crate::sim::stage::StageId;
use crate::sim::{GameState, MatchConfig, PlayerInput};
use crate::viz::{self, font, Color as VColor, Painter, SceneOpts};
use input::InputHub;
use scene3d::Scene3D;
use widgets::*;

/// Adapts macroquad's immediate-mode drawing to the [`Painter`] trait.
pub(crate) struct MqPainter;

thread_local! {
    /// Drawing-surface size override while rendering into a texture (the
    /// capture tool); `None` = the window.
    static PAINTER_DIMS: std::cell::Cell<Option<(f32, f32)>> = const { std::cell::Cell::new(None) };
}

pub(crate) fn set_painter_dims(d: Option<(f32, f32)>) {
    PAINTER_DIMS.with(|c| c.set(d));
}

#[inline]
pub(crate) fn col(c: VColor) -> macroquad::color::Color {
    macroquad::color::Color::from_rgba(c.r, c.g, c.b, c.a)
}

impl Painter for MqPainter {
    fn fill_rect(&mut self, x: f32, y: f32, w: f32, h: f32, c: VColor) {
        draw_rectangle(x, y, w, h, col(c));
    }
    fn fill_circle(&mut self, cx: f32, cy: f32, r: f32, c: VColor) {
        draw_circle(cx, cy, r, col(c));
    }
    fn line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, thick: f32, c: VColor) {
        draw_line(x0, y0, x1, y1, thick, col(c));
    }
    fn dims(&self) -> (f32, f32) {
        PAINTER_DIMS
            .with(|c| c.get())
            .unwrap_or_else(|| (screen_width(), screen_height()))
    }
}

/// Command-line options (`overframe --host [port]`, `overframe --join <code>`,
/// `--versus`, `--demo`, `--screenshot`).
#[derive(Clone, Debug, Default)]
pub struct LaunchOpts {
    pub host: Option<u16>,
    pub join: Option<String>,
    /// Jump straight into a local versus match with these rules.
    pub versus: Option<MatchSpec>,
    /// Play the attract-mode demo (optionally with these rules).
    pub demo: Option<MatchSpec>,
    /// Save a PNG of the window after `at` drawn frames, then quit.
    pub screenshot: Option<(String, u64)>,
    /// Save every drawn frame from `at` for `count` frames as
    /// `<dir>/frame_NNNN.png`, then quit (for making GIFs).
    pub record: Option<(String, u64, u64)>,
    /// Open the animation viewer on this character, optionally at a clip
    /// index and frame (paused): `--anim viper:15:20`.
    pub anim: Option<(CharacterId, Option<usize>, Option<u32>)>,
    /// Render the fixed-camera evidence suite into a directory and quit
    /// (`--capture <dir>`; see `render::capture`).
    pub capture: Option<capture::CaptureOpts>,
}

/// A match description parsed from the command line:
/// `p1[,p2[,stage[,pal1[,pal2]]]]`, e.g. `kestrel,viper,tidegate,0,2`.
#[derive(Clone, Copy, Debug)]
pub struct MatchSpec {
    pub p1: CharacterId,
    pub p2: CharacterId,
    pub stage: StageId,
    pub pal1: u8,
    pub pal2: u8,
}

impl MatchSpec {
    pub fn parse(text: &str) -> Result<MatchSpec, String> {
        let mut spec = MatchSpec {
            p1: CharacterId::Kestrel,
            p2: CharacterId::Boulder,
            stage: StageId::Lattice,
            pal1: 0,
            pal2: 1,
        };
        let ch = |n: &str| {
            CharacterId::ALL
                .iter()
                .copied()
                .find(|c| c.name().eq_ignore_ascii_case(n))
                .ok_or_else(|| format!("unknown character '{n}'"))
        };
        let st = |n: &str| {
            StageId::ALL
                .iter()
                .copied()
                .find(|c| {
                    let full = c.name().replace(' ', "");
                    full.eq_ignore_ascii_case(n)
                        || full
                            .strip_prefix("The")
                            .is_some_and(|rest| rest.eq_ignore_ascii_case(n))
                })
                .ok_or_else(|| format!("unknown stage '{n}'"))
        };
        for (i, part) in text
            .split(',')
            .map(str::trim)
            .filter(|p| !p.is_empty())
            .enumerate()
        {
            match i {
                0 => spec.p1 = ch(part)?,
                1 => spec.p2 = ch(part)?,
                2 => spec.stage = st(part)?,
                3 => spec.pal1 = part.parse().map_err(|_| "bad palette".to_string())?,
                4 => spec.pal2 = part.parse().map_err(|_| "bad palette".to_string())?,
                _ => return Err("too many fields".into()),
            }
        }
        Ok(spec)
    }

    fn rules(&self) -> Rules {
        Rules {
            p1: self.p1,
            p2: self.p2,
            p1_pal: self.pal1,
            p2_pal: self.pal2,
            stage: self.stage,
            stocks: 4,
            time_secs: 0,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Screen {
    Menu,
    /// Full-screen scripted demo match (attract mode).
    Attract,
    /// Animation viewer (`--anim <character>`): every state and move on a
    /// pedestal, for checking models.
    AnimViewer,
    LocalSetup,
    TrainSetup,
    Versus,
    Training,
    Controls,
    Options,
    OnlineMenu,
    JoinBrowser,
    Lobby,
    OnlineMatch,
    OnlineEnded,
}

const MENU_ITEMS: [&str; 7] = [
    "VERSUS  2P LOCAL",
    "ONLINE",
    "TRAINING",
    "OPTIONS",
    "CONTROLS",
    "WATCH DEMO",
    "QUIT",
];
const ONLINE_ITEMS: [&str; 3] = ["HOST GAME", "JOIN GAME", "BACK"];

fn window_conf(opts: &LaunchOpts) -> Conf {
    // Captures want the real menu at 1920×1080 (EXCHANGE.md).
    let (w, h) = if opts.capture.is_some() {
        (1920, 1080)
    } else {
        (1280, 720)
    };
    Conf {
        window_title: "OVERFRAME".to_owned(),
        window_width: w,
        window_height: h,
        high_dpi: true,
        ..Default::default()
    }
}

/// Launch the game (blocks until the window closes).
pub fn launch(opts: LaunchOpts) {
    let conf = window_conf(&opts);
    macroquad::Window::from_config(conf, amain(opts));
}

/// Local match setup, edited on the setup screens and carried into the lobby as
/// the host's defaults.
#[derive(Clone, Copy)]
struct Rules {
    p1: CharacterId,
    p2: CharacterId,
    p1_pal: u8,
    p2_pal: u8,
    stage: StageId,
    stocks: i32,
    time_secs: u32,
}
impl Rules {
    fn to_config(self) -> MatchConfig {
        MatchConfig {
            stocks: self.stocks,
            time_limit_secs: self.time_secs,
            stage: self.stage,
            chars: [self.p1, self.p2, CharacterId::Kestrel, CharacterId::Kestrel],
            palettes: [self.p1_pal, self.p2_pal, 2, 3],
            ..MatchConfig::default()
        }
    }
    fn training_config(self) -> MatchConfig {
        MatchConfig {
            stocks: 99,
            ..self.to_config()
        }
    }
}

struct App {
    settings: Settings,
    hub: InputHub,
    scene: Scene3D,
    audio: Option<audio::AudioBank>,
    /// Last fx `born` frame voiced for rumble.
    rumble_seen: Vec<(u64, u8)>,
    screen: Screen,
    frame_no: u64,

    menu_idx: usize,
    online_idx: usize,
    options_idx: usize,
    setup_idx: usize,
    lobby_idx: usize,
    browser_idx: usize,

    rules: Rules,
    gs: GameState,
    demo_frame: u64,
    show_boxes: bool,
    acc: f32,

    net: Option<NetMatch>,
    browser: Option<Browser>,
    join_text: String,
    pass_text: String,
    chat_text: String,
    typing_chat: bool,
    status: String,
    waiting_ticks: u32,

    rebind: Option<(usize, PadAction)>,
    rebind_started: u64,
    nav_cd: f32,
    screenshot: Option<(String, u64)>,
    record: Option<(String, u64, u64)>,
    /// One simulation step per drawn frame regardless of wall-clock time —
    /// set while recording / screenshotting so frame N on disk is sim frame N
    /// even on a slow software renderer.
    fixed_step: bool,
    drawn: u64,
    viewer: Viewer,
}

/// Animation-viewer state: a synthetic fighter driven through every state.
struct Viewer {
    fighter: Option<crate::sim::fighter::Fighter>,
    clip: usize,
    frame: u64,
    angle: f32,
    paused: bool,
    orbit: bool,
}

/// One showcase clip: a label and how to put the fighter into it.
struct Clip {
    label: &'static str,
    state: crate::sim::fighter::State,
    grounded: bool,
    vel_y: f32,
    /// Frames to show (0 = the move's own length + a hold).
    len: u32,
}

fn viewer_clips() -> Vec<Clip> {
    use crate::sim::attacks::MoveId as M;
    use crate::sim::constants as K;
    use crate::sim::fighter::{GetupKind, LedgeKind, State as S};
    let g = |label, state, len| Clip {
        label,
        state,
        grounded: true,
        vel_y: 0.0,
        len,
    };
    let a = |label, state, vel_y| Clip {
        label,
        state,
        grounded: false,
        vel_y,
        len: 60,
    };
    let ledge = |label, kind| {
        let (total, _, _) = crate::sim::fighter::Fighter::new(
            &crate::sim::roster::KESTREL,
            0,
            crate::sim::Vec2::ZERO,
        )
        .ledge_option(kind);
        Clip {
            label,
            state: S::LedgeAction { kind },
            grounded: false,
            vel_y: 0.0,
            len: total,
        }
    };
    let atk = |label, id, aerial| Clip {
        label,
        state: S::Attack { id, aerial },
        grounded: !aerial,
        vel_y: if aerial { -1.0 } else { 0.0 },
        len: 0,
    };
    vec![
        g("STAND", S::Stand, 90),
        g("WALK", S::Walk, 90),
        g("RUN", S::Run, 90),
        g("CROUCH", S::Crouch, 60),
        g("JUMP SQUAT", S::JumpSquat, 30),
        a("AIR (RISING)", S::Air, 3.0),
        a("AIR (FALLING)", S::Air, -2.0),
        g("SHIELD", S::Shield, 60),
        g(
            "ROLL",
            S::Roll { dir: 1.0 },
            crate::sim::constants::ROLL_DURATION,
        ),
        g(
            "SPOTDODGE",
            S::Spotdodge,
            crate::sim::constants::SPOTDODGE_DURATION,
        ),
        a("AIRDODGE", S::Airdodge, 0.0),
        a("HELPLESS", S::Helpless, -2.0),
        g(
            "SHIELD DROP",
            S::ShieldDrop,
            crate::sim::constants::SHIELD_DROP,
        ),
        atk("JAB", M::Jab, false),
        atk("JAB 2", M::Jab2, false),
        atk("FORWARD TILT", M::Ftilt, false),
        atk("UP TILT", M::Utilt, false),
        atk("DOWN TILT", M::Dtilt, false),
        atk("FORWARD SMASH", M::Fsmash, false),
        atk("UP SMASH", M::Usmash, false),
        atk("DOWN SMASH", M::Dsmash, false),
        atk("DASH ATTACK", M::DashAttack, false),
        atk("NEUTRAL AIR", M::Nair, true),
        atk("FORWARD AIR", M::Fair, true),
        atk("BACK AIR", M::Bair, true),
        atk("UP AIR", M::Uair, true),
        atk("DOWN AIR", M::Dair, true),
        atk("NEUTRAL SPECIAL", M::SpecialN, false),
        atk("UP SPECIAL", M::SpecialUp, true),
        atk("SIDE SPECIAL", M::SpecialSide, false),
        atk("DOWN SPECIAL", M::SpecialDown, false),
        g("GRAB", S::Grab, 40),
        g("FORWARD THROW", S::Throw { id: M::ThrowF }, 0),
        g("FLINCH", S::Hitstun { tumble: false }, 40),
        a("TUMBLE", S::Hitstun { tumble: true }, 1.0),
        g("KNOCKDOWN", S::Knockdown, 60),
        g("TECH IN PLACE", S::Tech { dir: 0.0 }, K::TECH_IN_PLACE.0),
        g("TECH ROLL", S::Tech { dir: 1.0 }, K::TECH_ROLL.0),
        g(
            "GETUP STAND",
            S::Getup {
                kind: GetupKind::Stand,
            },
            K::GETUP_STAND.0,
        ),
        g(
            "GETUP ROLL",
            S::Getup {
                kind: GetupKind::Roll { dir: 1.0 },
            },
            K::GETUP_ROLL.0,
        ),
        g(
            "GETUP ATTACK",
            S::Getup {
                kind: GetupKind::Attack,
            },
            K::GETUP_ATTACK.0,
        ),
        g("RUN TURN", S::RunTurn, K::RUN_TURN),
        g("CLANK", S::Rebound { total: 12 }, 12),
        a("LEDGE HANG", S::LedgeGrab, 0.0),
        ledge("LEDGE GETUP", LedgeKind::Getup),
        ledge("LEDGE ATTACK", LedgeKind::Attack),
        ledge("LEDGE ROLL", LedgeKind::Roll),
        ledge("LEDGE JUMP", LedgeKind::Jump),
    ]
}

impl App {
    fn new(opts: LaunchOpts) -> App {
        let settings = Settings::load_or_create();
        let last = settings.last_character();
        let mut app = App {
            settings,
            hub: InputHub::new(),
            scene: Scene3D::new(),
            audio: None,
            rumble_seen: Vec::new(),
            screen: Screen::Menu,
            frame_no: 0,
            menu_idx: 0,
            online_idx: 0,
            options_idx: 0,
            setup_idx: 0,
            lobby_idx: 0,
            browser_idx: 0,
            rules: Rules {
                p1: last,
                p2: CharacterId::Boulder,
                p1_pal: 0,
                p2_pal: 1,
                stage: StageId::Lattice,
                stocks: 4,
                time_secs: 0,
            },
            gs: GameState::new(2, MatchConfig::default()),
            demo_frame: 0,
            show_boxes: true,
            acc: 0.0,
            net: None,
            browser: None,
            join_text: String::new(),
            pass_text: String::new(),
            chat_text: String::new(),
            typing_chat: false,
            status: String::new(),
            waiting_ticks: 0,
            rebind: None,
            rebind_started: 0,
            nav_cd: 0.0,
            screenshot: opts.screenshot.clone(),
            record: opts.record.clone(),
            fixed_step: opts.record.is_some() || opts.screenshot.is_some(),
            drawn: 0,
            viewer: Viewer {
                fighter: None,
                clip: 0,
                frame: 0,
                angle: 25.0,
                paused: false,
                orbit: true,
            },
        };
        // The menu plays the demo in the background.
        app.gs = crate::demo::match_state();
        if let Some(spec) = opts.versus {
            app.rules = spec.rules();
            app.gs = GameState::new(2, app.rules.to_config());
            app.scene.reset();
            app.screen = Screen::Versus;
        } else if let Some(spec) = opts.demo {
            app.rules = spec.rules();
            app.start_demo();
            app.screen = Screen::Attract;
        } else if let Some((id, clip, frame)) = opts.anim {
            app.rules.p1 = id;
            if let Some(c) = clip {
                app.viewer.clip = c;
            }
            app.viewer_set_character(id);
            if let Some(fr) = frame {
                app.viewer.paused = true;
                app.viewer.orbit = false;
                app.viewer.frame = fr as u64;
                if let Some(f) = app.viewer.fighter.as_mut() {
                    f.state_frame = fr;
                }
            }
            app.screen = Screen::AnimViewer;
        }
        if let Some(port) = opts.host {
            app.start_host(port);
        } else if let Some(code) = opts.join {
            app.join_text = code;
            app.start_join();
        }
        app
    }

    fn nav(&mut self) -> Nav {
        let mut n = Nav::default();
        if is_key_pressed(KeyCode::S) || is_key_pressed(KeyCode::Down) {
            n.v += 1;
        }
        if is_key_pressed(KeyCode::W) || is_key_pressed(KeyCode::Up) {
            n.v -= 1;
        }
        if is_key_pressed(KeyCode::D) || is_key_pressed(KeyCode::Right) {
            n.h += 1;
        }
        if is_key_pressed(KeyCode::A) || is_key_pressed(KeyCode::Left) {
            n.h -= 1;
        }
        if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::Space) {
            n.confirm = true;
        }
        if is_key_pressed(KeyCode::Escape) {
            n.back = true;
        }
        use crate::gamepad::PadButton as B;
        if let Some(b) = self.hub.any_just_pressed() {
            match b {
                B::South | B::Start => n.confirm = true,
                B::East => n.back = true,
                B::DPadDown => n.v += 1,
                B::DPadUp => n.v -= 1,
                B::DPadLeft => n.h -= 1,
                B::DPadRight => n.h += 1,
                _ => {}
            }
        }
        self.nav_cd = (self.nav_cd - get_frame_time()).max(0.0);
        let s = self.hub.any_stick();
        if self.nav_cd <= 0.0 && s.length() > 0.6 {
            if s.y.abs() > s.x.abs() {
                n.v += if s.y < 0.0 { 1 } else { -1 };
            } else {
                n.h += if s.x > 0.0 { 1 } else { -1 };
            }
            self.nav_cd = 0.2;
        }
        n
    }

    // ------------------------------------------------------------ online

    fn start_host(&mut self, port: u16) {
        match NetMatch::host(
            port,
            &self.settings,
            self.rules.to_config(),
            &self.pass_text,
            BanList::load_local(),
        ) {
            Ok(m) => {
                self.net = Some(m);
                self.status.clear();
                self.lobby_idx = 0;
                self.screen = Screen::Lobby;
            }
            Err(e) => {
                self.status = format!("CANNOT BIND UDP PORT {port}: {e}").to_uppercase();
                self.screen = Screen::OnlineMenu;
            }
        }
    }

    fn start_join(&mut self) {
        let Some(addr) = roomcode::decode(&self.join_text) else {
            self.status = "INVALID CODE OR ADDRESS".into();
            return;
        };
        match NetMatch::join(addr, &self.settings, &self.pass_text) {
            Ok(m) => {
                self.net = Some(m);
                self.browser = None;
                self.status.clear();
                self.lobby_idx = 0;
                self.screen = Screen::Lobby;
            }
            Err(e) => self.status = format!("CANNOT OPEN SOCKET: {e}").to_uppercase(),
        }
    }

    fn leave_online(&mut self) {
        if let Some(net) = &self.net {
            net.send_leave();
        }
        self.net = None;
        self.browser = None;
        self.typing_chat = false;
        self.waiting_ticks = 0;
        self.screen = Screen::Menu;
    }

    // ------------------------------------------------------------ update

    fn update(&mut self) {
        self.frame_no += 1;
        self.hub.poll();
        let nav = self.nav();
        match self.screen {
            Screen::Menu => {
                self.step_demo();
                self.update_menu(nav);
            }
            Screen::Attract => {
                self.step_demo();
                if nav.back || nav.confirm {
                    self.screen = Screen::Menu;
                }
            }
            Screen::AnimViewer => self.update_viewer(nav),
            Screen::LocalSetup => self.update_setup(nav, false),
            Screen::TrainSetup => self.update_setup(nav, true),
            Screen::Controls => {
                if nav.back || nav.confirm {
                    self.screen = Screen::Menu;
                }
            }
            Screen::Options => self.update_options(nav),
            Screen::OnlineMenu => self.update_online_menu(nav),
            Screen::JoinBrowser => self.update_browser(nav),
            Screen::Lobby => self.update_lobby(nav),
            Screen::OnlineMatch => self.update_online_match(),
            Screen::OnlineEnded => {
                if nav.back || nav.confirm {
                    self.leave_online();
                }
            }
            Screen::Versus | Screen::Training => self.update_local_match(nav),
        }
    }

    fn update_menu(&mut self, nav: Nav) {
        let n = MENU_ITEMS.len() as i32;
        self.menu_idx = ((self.menu_idx as i32 + nav.v).rem_euclid(n)) as usize;
        if nav.confirm {
            match self.menu_idx {
                0 => {
                    self.setup_idx = 0;
                    self.screen = Screen::LocalSetup;
                }
                1 => {
                    self.status.clear();
                    self.screen = Screen::OnlineMenu;
                }
                2 => {
                    self.setup_idx = 0;
                    self.screen = Screen::TrainSetup;
                }
                3 => {
                    self.options_idx = 0;
                    self.screen = Screen::Options;
                }
                4 => self.screen = Screen::Controls,
                5 => {
                    self.start_demo();
                    self.screen = Screen::Attract;
                }
                _ => std::process::exit(0),
            }
        }
        if nav.back {
            std::process::exit(0);
        }
    }

    // ------------------------------------------------------------ viewer

    fn viewer_set_character(&mut self, id: CharacterId) {
        let mut f = crate::sim::fighter::Fighter::new(id.data(), 0, crate::sim::math::Vec2::ZERO);
        f.palette = self.rules.p1_pal;
        f.facing = 1.0;
        self.viewer.fighter = Some(f);
        self.viewer.frame = 0;
        self.viewer_apply_clip();
    }

    /// Put the synthetic fighter into the current clip's state.
    fn viewer_apply_clip(&mut self) {
        let clips = viewer_clips();
        let c = &clips[self.viewer.clip % clips.len()];
        if let Some(f) = self.viewer.fighter.as_mut() {
            f.state = c.state;
            f.state_frame = 0;
            f.grounded = c.grounded;
            f.vel = crate::sim::math::Vec2::new(0.0, c.vel_y);
            f.fastfalling = false;
            f.already_hit = false;
            f.pos = crate::sim::math::Vec2::new(0.0, if c.grounded { 0.0 } else { 8.0 });
            f.anim_flash = 0;
        }
        self.viewer.frame = 0;
    }

    fn viewer_clip_len(&self) -> u32 {
        let clips = viewer_clips();
        let c = &clips[self.viewer.clip % clips.len()];
        if c.len > 0 {
            return c.len;
        }
        match c.state {
            crate::sim::fighter::State::Attack { id, .. }
            | crate::sim::fighter::State::Throw { id } => {
                let ch = self
                    .viewer
                    .fighter
                    .as_ref()
                    .map(|f| f.character.id)
                    .unwrap_or(CharacterId::Kestrel);
                crate::sim::attacks::data(ch, id).total() + 24
            }
            _ => 60,
        }
    }

    fn update_viewer(&mut self, nav: Nav) {
        if nav.back {
            self.screen = Screen::Menu;
            return;
        }
        let n = viewer_clips().len();
        if nav.h != 0 {
            self.viewer.clip = (self.viewer.clip as i32 + nav.h).rem_euclid(n as i32) as usize;
            self.viewer_apply_clip();
        }
        if nav.v != 0 {
            let idx =
                (self.rules.p1.index() as i32 - nav.v).rem_euclid(CharacterId::ALL.len() as i32);
            self.rules.p1 = CharacterId::ALL[idx as usize];
            self.viewer_set_character(self.rules.p1);
        }
        if is_key_pressed(KeyCode::P) {
            self.rules.p1_pal = (self.rules.p1_pal + 1) % crate::sim::roster::PALETTES;
            if let Some(f) = self.viewer.fighter.as_mut() {
                f.palette = self.rules.p1_pal;
            }
        }
        if is_key_pressed(KeyCode::Space) {
            self.viewer.paused = !self.viewer.paused;
        }
        if is_key_pressed(KeyCode::O) {
            self.viewer.orbit = !self.viewer.orbit;
        }
        if is_key_pressed(KeyCode::F) {
            if let Some(f) = self.viewer.fighter.as_mut() {
                f.facing = -f.facing;
            }
        }
        if self.viewer.orbit {
            self.viewer.angle += 0.35;
        }
        // Advance at 60 Hz (one state frame per drawn frame at 60 fps).
        let step = if self.viewer.paused { 0 } else { 1 };
        let len = self.viewer_clip_len();
        for _ in 0..step {
            self.viewer.frame += 1;
            let total = len as u64;
            if self.viewer.frame >= total {
                // Loop the clip.
                self.viewer_apply_clip();
                continue;
            }
            if let Some(f) = self.viewer.fighter.as_mut() {
                let sf = self.viewer.frame as u32;
                // Hold the last frame of a move so the recovery pose reads.
                f.state_frame = match f.state {
                    crate::sim::fighter::State::Attack { id, .. }
                    | crate::sim::fighter::State::Throw { id } => sf.min(
                        crate::sim::attacks::data(f.character.id, id)
                            .total()
                            .saturating_sub(1),
                    ),
                    _ => sf,
                };
                if matches!(
                    f.state,
                    crate::sim::fighter::State::Walk | crate::sim::fighter::State::Run
                ) {
                    // Gait cycles key off the global frame.
                }
            }
        }
    }

    fn draw_viewer(&mut self) {
        let clips = viewer_clips();
        let c = &clips[self.viewer.clip % clips.len()];
        let Some(f) = self.viewer.fighter.clone() else {
            return;
        };
        let len = self.viewer_clip_len();
        let phase = match f.state {
            crate::sim::fighter::State::Attack { id, .. } => {
                let md = crate::sim::attacks::data(f.character.id, id);
                if md.is_active(f.state_frame) {
                    "ACTIVE"
                } else if f.state_frame < md.startup {
                    "STARTUP"
                } else {
                    "ENDLAG"
                }
            }
            _ => "",
        };
        let sub = format!(
            "{}  {}  FRAME {:>2}/{}  {}   <> STATE  UP/DOWN FIGHTER  P PALETTE  F FLIP  O ORBIT  SPACE PAUSE  ESC",
            f.character.name.to_uppercase(),
            viz::palette_name(f.character.id, f.palette).to_uppercase(),
            self.viewer.frame.min(len as u64),
            len,
            phase
        );
        let frame = self.viewer.frame;
        let angle = self.viewer.angle;
        self.scene.draw_viewer(&f, frame, angle, c.label, &sub);
    }

    /// (Re)start the scripted demo with the current rules' characters/stage.
    fn start_demo(&mut self) {
        let mut cfg = self.rules.to_config();
        cfg.seed = 0xC0FFEE;
        self.gs = crate::demo::match_state_with(cfg);
        self.demo_frame = 0;
        self.acc = 0.0;
        self.scene.reset();
    }

    /// Wall-clock time to feed the fixed-step accumulator this frame.
    fn frame_dt(&self) -> f32 {
        if self.fixed_step {
            1.0 / 60.0
        } else {
            get_frame_time().min(0.1)
        }
    }

    /// Advance the demo match at 60 Hz, looping when the script ends.
    fn step_demo(&mut self) {
        self.acc += self.frame_dt();
        let dt = 1.0 / 60.0;
        let mut steps = 0;
        while self.acc >= dt && steps < 5 {
            if self.demo_frame >= crate::demo::DEMO_LEN + 90 || self.gs.match_over.is_some() {
                self.start_demo();
            }
            let d = crate::demo::inputs(&self.gs, self.demo_frame);
            self.demo_frame += 1;
            self.gs.step(&[d[0], d[1]]);
            self.acc -= dt;
            steps += 1;
        }
    }

    fn update_setup(&mut self, nav: Nav, training: bool) {
        let rows: i32 = if training { 4 } else { 6 };
        self.setup_idx = ((self.setup_idx as i32 + nav.v).rem_euclid(rows)) as usize;
        if nav.back {
            self.screen = Screen::Menu;
            return;
        }
        let idx = self.setup_idx;
        let r = &mut self.rules;
        if training {
            match idx {
                0 => cycle_char(&mut r.p1, &mut r.p1_pal, nav),
                1 => cycle_stage(&mut r.stage, nav),
                2 => cycle_stocks(&mut r.stocks, nav),
                3 if nav.confirm => {
                    self.gs = GameState::new(2, self.rules.training_config());
                    self.acc = 0.0;
                    self.scene.reset();
                    self.screen = Screen::Training;
                }
                _ => {}
            }
        } else {
            match idx {
                0 => cycle_char(&mut r.p1, &mut r.p1_pal, nav),
                1 => cycle_char(&mut r.p2, &mut r.p2_pal, nav),
                2 => cycle_stage(&mut r.stage, nav),
                3 => cycle_stocks(&mut r.stocks, nav),
                4 => cycle_time(&mut r.time_secs, nav),
                5 if nav.confirm => {
                    self.settings.last_character = self.rules.p1 as u8;
                    let _ = self.settings.save();
                    self.gs = GameState::new(2, self.rules.to_config());
                    self.acc = 0.0;
                    self.scene.reset();
                    self.screen = Screen::Versus;
                }
                _ => {}
            }
        }
    }

    fn update_local_match(&mut self, nav: Nav) {
        if nav.back {
            self.screen = match self.screen {
                Screen::Training => Screen::TrainSetup,
                _ => Screen::LocalSetup,
            };
            return;
        }
        if self.screen == Screen::Training && is_key_pressed(KeyCode::Tab) {
            self.show_boxes = !self.show_boxes;
        }
        if self.screen == Screen::Training
            && (is_key_pressed(KeyCode::Backspace) || is_key_pressed(KeyCode::Key0))
        {
            self.gs = GameState::new(2, self.rules.training_config());
            self.acc = 0.0;
        }
        self.acc += self.frame_dt();
        let dt = 1.0 / 60.0;
        let mut steps = 0;
        while self.acc >= dt && steps < 5 {
            let inputs: Vec<PlayerInput> = match self.screen {
                Screen::Versus => vec![
                    self.hub.player_input(0, &self.settings.pad[0]),
                    self.hub.player_input(1, &self.settings.pad[1]),
                ],
                Screen::Training => vec![
                    self.hub.player_input(0, &self.settings.pad[0]),
                    PlayerInput::default(),
                ],
                _ => {
                    let d = crate::demo::inputs(&self.gs, self.demo_frame);
                    self.demo_frame += 1;
                    vec![d[0], d[1]]
                }
            };
            self.gs.step(&inputs);
            self.acc -= dt;
            steps += 1;
        }
    }

    fn update_online_menu(&mut self, nav: Nav) {
        let n = ONLINE_ITEMS.len() as i32;
        self.online_idx = ((self.online_idx as i32 + nav.v).rem_euclid(n)) as usize;
        take_text(&mut self.pass_text, 20);
        if nav.back {
            self.screen = Screen::Menu;
        } else if nav.confirm {
            match self.online_idx {
                0 => {
                    let port = self.settings.host_port;
                    self.start_host(port);
                }
                1 => {
                    self.browser = Browser::new().ok();
                    self.browser_idx = 0;
                    self.join_text.clear();
                    self.status.clear();
                    self.screen = Screen::JoinBrowser;
                }
                _ => self.screen = Screen::Menu,
            }
        }
    }

    fn update_browser(&mut self, nav: Nav) {
        if let Some(b) = self.browser.as_mut() {
            b.poll();
        }
        let rooms = self.browser.as_ref().map(|b| b.rooms()).unwrap_or_default();
        take_text_filtered(&mut self.join_text, 24, |c| {
            c.is_ascii_alphanumeric() || matches!(c, '.' | ':' | '-')
        });
        if nav.back {
            self.browser = None;
            self.screen = Screen::OnlineMenu;
            return;
        }
        if !rooms.is_empty() {
            self.browser_idx =
                ((self.browser_idx as i32 + nav.v).rem_euclid(rooms.len() as i32)) as usize;
        }
        if is_key_pressed(KeyCode::Enter) {
            if !self.join_text.is_empty() {
                self.start_join();
            } else if let Some(room) = rooms.get(self.browser_idx) {
                self.join_text = format!("{}", room.addr);
                self.start_join();
            }
        }
    }

    fn update_lobby(&mut self, nav: Nav) {
        let Some(net) = self.net.as_mut() else {
            self.screen = Screen::Menu;
            return;
        };
        net.poll();
        match net.phase().clone() {
            Phase::Running => {
                self.gs = net.initial_state();
                self.acc = 0.0;
                self.waiting_ticks = 0;
                self.scene.reset();
                self.screen = Screen::OnlineMatch;
                return;
            }
            Phase::Ended(reason) => {
                self.status = reason;
                self.screen = Screen::OnlineEnded;
                return;
            }
            _ => {}
        }

        if self.typing_chat {
            take_text_filtered(&mut self.chat_text, 60, |c| {
                c.is_ascii_graphic() || c == ' '
            });
            if is_key_pressed(KeyCode::Enter) {
                let t = std::mem::take(&mut self.chat_text);
                net.send_chat(&t);
                self.typing_chat = false;
            }
            if is_key_pressed(KeyCode::Escape) {
                self.typing_chat = false;
                self.chat_text.clear();
            }
            return;
        }
        if is_key_pressed(KeyCode::T) {
            self.typing_chat = true;
            return;
        }
        if nav.back {
            self.leave_online();
            return;
        }

        let host = net.is_host();
        let rows: i32 = if host { 7 } else { 3 };
        self.lobby_idx = ((self.lobby_idx as i32 + nav.v).rem_euclid(rows)) as usize;
        match self.lobby_idx {
            0 if nav.h != 0 => {
                let mut c = net.lobby_view().my.character;
                let mut pal = net.lobby_view().my.palette;
                cycle_char(&mut c, &mut pal, nav);
                net.set_my_character(c);
                self.settings.last_character = c as u8;
                let _ = self.settings.save();
            }
            1 if nav.h != 0 => net.cycle_my_palette(nav.h),
            2 if nav.confirm => net.toggle_ready(),
            3 if host && nav.h != 0 => {
                let mut s = net.lobby_view().stage;
                cycle_stage(&mut s, nav);
                net.set_stage(s);
            }
            4 if host && nav.h != 0 => net.set_stocks(net.lobby_view().stocks + nav.h),
            5 if host && nav.h != 0 => {
                let cur = net.lobby_view().time_secs as i32;
                net.set_time((cur + nav.h * 30).clamp(0, 600) as u32);
            }
            6 if host && nav.confirm && net.can_start() => net.start_match(),
            _ => {}
        }
    }

    fn update_online_match(&mut self) {
        if is_key_pressed(KeyCode::Escape) {
            self.leave_online();
            return;
        }
        let frame_dt = self.frame_dt();
        let Some(net) = self.net.as_mut() else {
            self.screen = Screen::Menu;
            return;
        };
        net.poll();
        if let Phase::Ended(reason) = net.phase() {
            self.status = reason.clone();
            self.screen = Screen::OnlineEnded;
            return;
        }
        self.acc += frame_dt;
        let dt = 1.0 / 60.0;
        let mut steps = 0;
        while self.acc >= dt && steps < 5 {
            let local = self.hub.player_input(0, &self.settings.pad[0]);
            match net.advance(local, &mut self.gs) {
                Advance::Advanced { .. } => self.waiting_ticks = 0,
                Advance::Skipped => {}
                Advance::Waiting => self.waiting_ticks = self.waiting_ticks.saturating_add(1),
                Advance::Ended => {
                    if let Phase::Ended(reason) = net.phase() {
                        self.status = reason.clone();
                    }
                    self.screen = Screen::OnlineEnded;
                    return;
                }
            }
            self.acc -= dt;
            steps += 1;
        }
    }

    fn option_rows(&self) -> Vec<String> {
        let mut rows = vec![
            format!("INPUT DELAY   {} FRAMES", self.settings.input_delay),
            format!("HOST PORT     {}", self.settings.host_port),
            format!(
                "RENDERER      {}",
                if self.settings.render_3d {
                    "3D"
                } else {
                    "2D CLASSIC"
                }
            ),
        ];
        for slot in 0..2 {
            for a in PadAction::ALL {
                rows.push(format!(
                    "PAD {} {:<8} {}",
                    slot + 1,
                    a.label(),
                    mask_label(self.settings.pad[slot].get(a))
                ));
            }
        }
        rows.push("RESET PAD DEFAULTS".into());
        rows.push("BACK".into());
        rows
    }

    fn update_options(&mut self, nav: Nav) {
        if let Some((slot, action)) = self.rebind {
            if nav.back {
                self.rebind = None;
                return;
            }
            if self.frame_no > self.rebind_started {
                if let Some(b) = self.hub.just_pressed(slot) {
                    self.settings.pad[slot].rebind(action, b);
                    let _ = self.settings.save();
                    self.rebind = None;
                }
            }
            return;
        }
        let rows = self.option_rows().len() as i32;
        self.options_idx = ((self.options_idx as i32 + nav.v).rem_euclid(rows)) as usize;
        let idx = self.options_idx;
        let pad_rows = PadAction::ALL.len();
        let shift = is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift);
        if nav.back {
            let _ = self.settings.save();
            self.screen = Screen::Menu;
            return;
        }
        match idx {
            0 if nav.h != 0 => {
                self.settings.input_delay =
                    (self.settings.input_delay as i32 + nav.h).clamp(0, 8) as u8;
                let _ = self.settings.save();
            }
            1 if nav.h != 0 => {
                let step = if shift { 100 } else { 1 };
                self.settings.host_port =
                    (self.settings.host_port as i32 + nav.h * step).clamp(1024, 65535) as u16;
                let _ = self.settings.save();
            }
            2 if nav.h != 0 || nav.confirm => {
                self.settings.render_3d = !self.settings.render_3d;
                let _ = self.settings.save();
            }
            3 if nav.h != 0 => {
                self.settings.sfx_volume =
                    (self.settings.sfx_volume as i32 + nav.h).clamp(0, 10) as u8;
                let _ = self.settings.save();
            }
            4 if nav.h != 0 || nav.confirm => {
                self.settings.rumble = !self.settings.rumble;
                let _ = self.settings.save();
            }
            i if (5..5 + 2 * pad_rows).contains(&i) && nav.confirm => {
                self.rebind = Some(((i - 5) / pad_rows, PadAction::ALL[(i - 5) % pad_rows]));
                self.rebind_started = self.frame_no;
            }
            i if i == 5 + 2 * pad_rows && nav.confirm => {
                self.settings.pad = Default::default();
                let _ = self.settings.save();
            }
            i if i == 6 + 2 * pad_rows && nav.confirm => {
                let _ = self.settings.save();
                self.screen = Screen::Menu;
            }
            _ => {}
        }
    }

    // ------------------------------------------------------------ draw

    fn draw(&mut self) {
        clear_background(col(VColor::rgb(12, 12, 20)));
        let mut p = MqPainter;
        match self.screen {
            Screen::Menu => {
                // Live demo match behind the menu, dimmed.
                let opts = SceneOpts {
                    training: false,
                    watermark: false,
                    local_player: None,
                    hud: false,
                    hitboxes: false,
                };
                self.draw_match(&mut p, opts);
                let (w, h) = p.dims();
                p.fill_rect(0.0, 0.0, w, h, VColor::rgba(8, 8, 16, 165));
                self.draw_menu(&mut p);
            }
            Screen::Attract => {
                let opts = SceneOpts {
                    training: false,
                    watermark: true,
                    local_player: None,
                    hud: true,
                    hitboxes: false,
                };
                self.draw_match(&mut p, opts);
                self.feedback(true);
                hint(&mut p, "DEMO   ENTER / ESC BACK");
            }
            Screen::AnimViewer => self.draw_viewer(),
            Screen::LocalSetup => self.draw_setup(&mut p, false),
            Screen::TrainSetup => self.draw_setup(&mut p, true),
            Screen::Controls => widgets::draw_controls(&mut p),
            Screen::Options => self.draw_options(&mut p),
            Screen::OnlineMenu => self.draw_online_menu(&mut p),
            Screen::JoinBrowser => self.draw_browser(&mut p),
            Screen::Lobby => self.draw_lobby(&mut p),
            Screen::OnlineEnded => self.draw_ended(&mut p),
            Screen::Versus => {
                let opts = self.scene_opts(false, None);
                self.draw_match(&mut p, opts);
                self.feedback(true);
                self.draw_clock(&mut p);
            }
            Screen::Training => {
                let opts = self.scene_opts(self.show_boxes, None);
                self.draw_match(&mut p, opts);
                self.feedback(true);
                hint(&mut p, "TAB BOXES   BACKSPACE RESET   ESC BACK");
            }
            Screen::OnlineMatch => {
                let local = self.net.as_ref().map(|n| n.local_handle());
                let opts = self.scene_opts(false, local);
                self.draw_match(&mut p, opts);
                self.feedback(true);
                self.draw_clock(&mut p);
                self.draw_online_overlay(&mut p);
            }
        }
    }

    /// The match view: 3D scene by default, the classic 2D view if disabled
    /// in Options (or wanted for debugging).
    fn draw_match(&mut self, p: &mut MqPainter, opts: SceneOpts) {
        if self.settings.render_3d {
            self.scene.draw(&self.gs, opts);
        } else {
            viz::draw_scene(p, &self.gs, opts);
        }
    }

    /// One-shot feedback for effects born since the last drawn frame: sound
    /// and controller rumble. Pure function of the displayed state.
    fn feedback(&mut self, with_audio: bool) {
        if with_audio {
            if let Some(a) = self.audio.as_mut() {
                a.volume = self.settings.sfx_volume as f32 / 10.0;
                a.update(&self.gs);
            }
        }
        self.hub.rumble_enabled = self.settings.rumble;
        let frame = self.gs.frame;
        self.rumble_seen
            .retain(|(b, _)| *b + 40 >= frame && *b <= frame);
        for fx in &self.gs.fx {
            let tag = fx.kind as u8;
            if fx.born + 6 < frame
                || self
                    .rumble_seen
                    .iter()
                    .any(|(b, k)| *b == fx.born && *k == tag)
            {
                continue;
            }
            self.rumble_seen.push((fx.born, tag));
            let (victim, strength) = match fx.kind {
                crate::sim::FxKind::Hit => (fx.who, (0.35 + fx.magnitude / 40.0).min(1.0)),
                crate::sim::FxKind::Shield => (fx.who, 0.3),
                crate::sim::FxKind::Blast => (fx.who, 1.0),
                _ => continue,
            };
            // The victim's pad gets the full hit; the other player's a tap.
            for slot in 0..2usize {
                let s = if victim as usize == slot {
                    strength
                } else {
                    strength * 0.35
                };
                self.hub.rumble(slot, s);
            }
        }
    }

    fn lobby_preview(&mut self, x: f32, y: f32, id: CharacterId, palette: u8) {
        if !self.settings.render_3d {
            return;
        }
        let (px, py, pw, ph) = widgets::lobby_preview_rect(x, y, 280.0, 250.0);
        self.scene.draw_preview(id, palette, px, py, pw, ph);
    }

    /// 3D model preview inside a character card.
    fn card_preview(&mut self, x: f32, y: f32, bw: f32, bh: f32, id: CharacterId, palette: u8) {
        if !self.settings.render_3d {
            return;
        }
        let (px, py, pw, ph) = widgets::card_preview_rect(x, y, bw, bh);
        self.scene.draw_preview(id, palette, px, py, pw, ph);
    }

    fn scene_opts(&self, training: bool, local: Option<usize>) -> SceneOpts {
        SceneOpts {
            training,
            watermark: true,
            local_player: local,
            hud: true,
            hitboxes: false,
        }
    }

    fn draw_clock(&self, p: &mut MqPainter) {
        if let Some(secs) = self.gs.time_left_secs() {
            let (w, _) = p.dims();
            let t = format!("{}:{:02}", secs / 60, secs % 60);
            font::draw_text(
                p,
                &t,
                w * 0.5 - font::text_width(&t, 3.0) * 0.5,
                20.0,
                3.0,
                ACCENT,
            );
        }
    }

    fn footer(&self, p: &mut MqPainter) {
        let (w, h) = p.dims();
        let pads = if self.hub.pad_count() == 0 {
            "NO GAMEPAD".to_owned()
        } else {
            format!("{} GAMEPAD(S)", self.hub.pad_count())
        };
        let line = format!(
            "ID {:08X}   {}   MIT   FREE   ROLLBACK NETCODE",
            (self.settings.player_id >> 32) as u32,
            pads
        );
        font::draw_text(
            p,
            &line,
            w * 0.5 - font::text_width(&line, 1.5) * 0.5,
            h - 40.0,
            1.5,
            MUTED,
        );
    }

    fn draw_menu(&self, p: &mut MqPainter) {
        let (w, h) = p.dims();
        font::draw_text(
            p,
            "OVERFRAME",
            w * 0.5 - font::text_width("OVERFRAME", 8.0) * 0.5,
            h * 0.13,
            8.0,
            VColor::rgb(255, 92, 74),
        );
        let sub = "AN OPEN PLATFORM FIGHTER";
        font::draw_text(
            p,
            sub,
            w * 0.5 - font::text_width(sub, 2.0) * 0.5,
            h * 0.13 + 66.0,
            2.0,
            DIM,
        );
        draw_list(p, &MENU_ITEMS, self.menu_idx, h * 0.36, 44.0, 3.0);
        self.footer(p);
    }

    fn draw_setup(&mut self, p: &mut MqPainter, training: bool) {
        let (w, h) = p.dims();
        title(p, if training { "TRAINING" } else { "VERSUS SETUP" }, 28.0);

        // Two big character cards, a stage card with a live 3D preview, and
        // the rules column.
        let cw = w * 0.44;
        let ch = h * 0.46;
        let cy = h * 0.15;
        let x1 = w * 0.04;
        let x2 = w * 0.52;
        draw_char_card(
            p,
            x1,
            cy,
            cw,
            ch,
            "PLAYER 1",
            self.rules.p1,
            self.rules.p1_pal,
            self.setup_idx == 0,
        );
        self.card_preview(x1, cy, cw, ch, self.rules.p1, self.rules.p1_pal);
        let (slot2, sel2) = if training {
            ("DUMMY", false)
        } else {
            ("PLAYER 2", self.setup_idx == 1)
        };
        draw_char_card(
            p,
            x2,
            cy,
            cw,
            ch,
            slot2,
            self.rules.p2,
            self.rules.p2_pal,
            sel2,
        );
        self.card_preview(x2, cy, cw, ch, self.rules.p2, self.rules.p2_pal);

        let stage_row = if training { 1 } else { 2 };
        let sy = cy + ch + h * 0.03;
        let sh = h - sy - 44.0;
        let sw = w * 0.44;
        draw_stage_card(
            p,
            x1,
            sy,
            sw,
            sh,
            self.rules.stage,
            self.setup_idx == stage_row,
        );
        if self.settings.render_3d {
            let (px, py, pw, ph) = widgets::stage_preview_rect(x1, sy, sw, sh);
            self.scene
                .draw_stage_preview(self.rules.stage, px, py, pw, ph);
        }

        let rows: Vec<(String, bool)> = if training {
            vec![
                (
                    format!("STOCKS   {}", self.rules.stocks),
                    self.setup_idx == 2,
                ),
                ("START".into(), self.setup_idx == 3),
            ]
        } else {
            vec![
                (
                    format!("STOCKS   {}", self.rules.stocks),
                    self.setup_idx == 3,
                ),
                (
                    format!("TIME     {}", time_label(self.rules.time_secs)),
                    self.setup_idx == 4,
                ),
                ("START".into(), self.setup_idx == 5),
            ]
        };
        let x = x2 + 24.0;
        for (i, (t, sel)) in rows.iter().enumerate() {
            let y = sy + 16.0 + i as f32 * 36.0;
            if *sel {
                p.fill_rect(x - 18.0, y - 4.0, 10.0, 24.0, ACCENT);
            }
            font::draw_text(p, t, x, y, 2.6, if *sel { ACCENT } else { DIM });
        }
        hint(
            p,
            "UP/DOWN SELECT   LEFT/RIGHT CHANGE   ENTER START   ESC BACK",
        );
    }

    fn draw_online_menu(&self, p: &mut MqPainter) {
        let (w, h) = p.dims();
        title(p, "ONLINE", 50.0);
        draw_list(p, &ONLINE_ITEMS, self.online_idx, h * 0.34, 46.0, 3.0);
        let pw = if self.pass_text.is_empty() {
            "(NONE)".to_owned()
        } else {
            "*".repeat(self.pass_text.len())
        };
        let l = format!("ROOM PASSWORD (TYPE): {pw}");
        font::draw_text(
            p,
            &l,
            w * 0.5 - font::text_width(&l, 1.5) * 0.5,
            h * 0.60,
            1.5,
            DIM,
        );
        let info = [
            format!("HOSTING USES UDP PORT {}", self.settings.host_port),
            "LAN GAMES ARE FOUND AUTOMATICALLY IN JOIN".into(),
            "INTERNET: FORWARD THAT UDP PORT, SHARE YOUR PUBLIC IP:PORT".into(),
        ];
        for (i, t) in info.iter().enumerate() {
            font::draw_text(
                p,
                t,
                w * 0.5 - font::text_width(t, 1.5) * 0.5,
                h * 0.66 + i as f32 * 22.0,
                1.5,
                MUTED,
            );
        }
        if !self.status.is_empty() {
            font::draw_text(
                p,
                &self.status,
                w * 0.5 - font::text_width(&self.status, 2.0) * 0.5,
                h * 0.54,
                2.0,
                BAD,
            );
        }
        self.footer(p);
    }

    fn draw_browser(&self, p: &mut MqPainter) {
        let (w, h) = p.dims();
        title(p, "JOIN GAME", 40.0);
        let rooms = self.browser.as_ref().map(|b| b.rooms()).unwrap_or_default();
        font::draw_text(p, "LAN ROOMS", w * 0.12, 110.0, 2.0, DIM);
        if rooms.is_empty() {
            let e = if self.browser.is_none() {
                "DISCOVERY PORT BUSY (ANOTHER INSTANCE IS BROWSING)"
            } else {
                "SEARCHING... HOSTS APPEAR HERE AUTOMATICALLY"
            };
            font::draw_text(p, e, w * 0.12, 150.0, 1.5, MUTED);
        }
        for (i, r) in rooms.iter().enumerate().take(8) {
            let y = 145.0 + i as f32 * 34.0;
            let sel = i == self.browser_idx;
            if sel {
                p.fill_rect(w * 0.12 - 18.0, y - 4.0, 10.0, 24.0, ACCENT);
            }
            let lock = if r.locked { " [LOCK]" } else { "" };
            let ver = if r.version_ok { "" } else { " [VER?]" };
            let line = format!("{}  {}P{}{}", r.name, r.players, lock, ver);
            font::draw_text(
                p,
                &line.to_uppercase(),
                w * 0.12,
                y,
                2.0,
                if sel { ACCENT } else { DIM },
            );
        }

        let bx = w * 0.12;
        let by = h * 0.66;
        font::draw_text(p, "OR ENTER CODE / IP:PORT", bx, by - 26.0, 1.5, DIM);
        p.fill_rect(bx, by, 520.0, 46.0, VColor::rgba(10, 12, 22, 200));
        p.fill_rect(bx, by + 44.0, 520.0, 2.0, ACCENT);
        let caret = if (self.frame_no / 30) % 2 == 0 {
            "_"
        } else {
            " "
        };
        font::draw_text(
            p,
            &format!("{}{}", self.join_text, caret),
            bx + 12.0,
            by + 12.0,
            3.0,
            VColor::rgb(240, 240, 240),
        );
        font::draw_text(p, "ENTER  CONNECT     ESC  BACK", bx, by + 60.0, 1.5, MUTED);
        if !self.status.is_empty() {
            font::draw_text(p, &self.status, bx, by + 84.0, 1.8, BAD);
        }
    }

    fn draw_lobby(&mut self, p: &mut MqPainter) {
        let (w, h) = p.dims();
        let (lv, hosting, room_code, can_start) = match &self.net {
            Some(net) => (
                net.lobby_view(),
                net.role() == Role::Host,
                net.room_code.clone(),
                net.can_start(),
            ),
            None => return,
        };
        title(p, if hosting { "HOSTING" } else { "LOBBY" }, 36.0);

        if hosting {
            let code = format!("CODE {}", room_code);
            font::draw_text(
                p,
                &code,
                w * 0.5 - font::text_width(&code, 2.0) * 0.5,
                76.0,
                2.0,
                MUTED,
            );
        }

        draw_lobby_pick(
            p,
            w * 0.05,
            110.0,
            "YOU",
            lv.my,
            self.lobby_idx,
            true,
            hosting,
        );
        self.lobby_preview(w * 0.05, 110.0, lv.my.character, lv.my.palette);
        if lv.peer_present {
            draw_lobby_pick(
                p,
                w * 0.05 + 300.0,
                110.0,
                &lv.peer_name,
                lv.peer,
                99,
                false,
                false,
            );
            self.lobby_preview(w * 0.05 + 300.0, 110.0, lv.peer.character, lv.peer.palette);
        } else {
            font::draw_text(
                p,
                "WAITING FOR OPPONENT...",
                w * 0.05 + 320.0,
                150.0,
                2.0,
                MUTED,
            );
        }

        let rx = w * 0.66;
        font::draw_text(p, "RULES", rx, 110.0, 2.0, DIM);
        let rules = [
            (
                format!("STAGE   {}", lv.stage.name().to_uppercase()),
                3usize,
            ),
            (format!("STOCKS  {}", lv.stocks), 4),
            (format!("TIME    {}", time_label(lv.time_secs)), 5),
        ];
        for (i, (t, row)) in rules.iter().enumerate() {
            let y = 140.0 + i as f32 * 28.0;
            let sel = hosting && self.lobby_idx == *row;
            if sel {
                p.fill_rect(rx - 16.0, y - 4.0, 8.0, 20.0, ACCENT);
            }
            let c = if sel {
                ACCENT
            } else if hosting {
                DIM
            } else {
                MUTED
            };
            font::draw_text(p, t, rx, y, 2.0, c);
        }
        draw_stage_card(p, rx, 230.0, 260.0, 150.0, lv.stage, false);
        if self.settings.render_3d {
            let (px, py, pw, ph) = widgets::stage_preview_rect(rx, 230.0, 260.0, 150.0);
            self.scene.draw_stage_preview(lv.stage, px, py, pw, ph);
        }

        if hosting {
            let sel = self.lobby_idx == 6;
            let can = can_start;
            let label = if can {
                "START MATCH"
            } else {
                "START (NEED BOTH READY)"
            };
            let c = if !can {
                MUTED
            } else if sel {
                ACCENT
            } else {
                GOOD
            };
            if sel {
                p.fill_rect(rx - 16.0, h * 0.60 - 4.0, 8.0, 22.0, c);
            }
            font::draw_text(p, label, rx, h * 0.60, 2.2, c);
        }

        // Chat.
        let cx = w * 0.05;
        let cy = h * 0.58;
        font::draw_text(p, "CHAT  (T TO TYPE)", cx, cy - 22.0, 1.5, DIM);
        p.fill_rect(cx, cy, 560.0, 118.0, VColor::rgba(10, 12, 22, 160));
        let start = lv.chat.len().saturating_sub(6);
        for (i, (who, text)) in lv.chat[start..].iter().enumerate() {
            let y = cy + 6.0 + i as f32 * 18.0;
            let c = match who.as_str() {
                "YOU" => ACCENT,
                "SYSTEM" => GOOD,
                _ => DIM,
            };
            font::draw_text(p, &format!("{who}: {text}"), cx + 6.0, y, 1.5, c);
        }
        if self.typing_chat {
            let caret = if (self.frame_no / 30) % 2 == 0 {
                "_"
            } else {
                " "
            };
            p.fill_rect(cx, cy + 120.0, 560.0, 22.0, VColor::rgba(30, 34, 54, 220));
            font::draw_text(
                p,
                &format!("> {}{}", self.chat_text, caret),
                cx + 6.0,
                cy + 124.0,
                1.8,
                VColor::rgb(240, 240, 240),
            );
        }

        hint(
            p,
            "ARROWS SELECT/CHANGE   ENTER READY/START   T CHAT   ESC LEAVE",
        );
    }

    fn draw_ended(&self, p: &mut MqPainter) {
        let (w, h) = p.dims();
        title(p, "MATCH ENDED", 50.0);
        let r = self.status.to_uppercase();
        font::draw_text(
            p,
            &r,
            w * 0.5 - font::text_width(&r, 2.5) * 0.5,
            h * 0.42,
            2.5,
            BAD,
        );
        let msg = "ENTER / ESC  BACK TO MENU";
        font::draw_text(
            p,
            msg,
            w * 0.5 - font::text_width(msg, 1.5) * 0.5,
            h * 0.6,
            1.5,
            MUTED,
        );
    }

    fn draw_online_overlay(&self, p: &mut MqPainter) {
        let (w, _h) = p.dims();
        let Some(net) = &self.net else {
            return;
        };
        let s = net.stats().unwrap_or_default();
        let role = if net.role() == Role::Host {
            "HOST"
        } else {
            "GUEST"
        };
        let line = format!(
            "ONLINE {}  VS {}   PING {}MS   ROLLBACK {}F   AHEAD {:+}   DELAY {}F",
            role,
            net.peer_name(),
            s.ping_ms,
            s.rollback_frames,
            s.frames_ahead,
            self.settings.input_delay
        );
        p.fill_rect(
            8.0,
            8.0,
            font::text_width(&line, 1.5) + 16.0,
            22.0,
            VColor::rgba(10, 12, 22, 170),
        );
        let c = if s.ping_ms > 120 {
            BAD
        } else if s.ping_ms > 60 {
            ACCENT
        } else {
            GOOD
        };
        font::draw_text(p, &line, 16.0, 13.0, 1.5, c);
        if self.waiting_ticks > 6 {
            font::draw_text(p, "WAITING FOR PEER...", w * 0.5 - 130.0, 60.0, 2.5, ACCENT);
        }
        if s.desynced {
            p.fill_rect(
                w * 0.5 - 150.0,
                90.0,
                300.0,
                34.0,
                VColor::rgba(120, 0, 0, 200),
            );
            font::draw_text(
                p,
                "DESYNC DETECTED",
                w * 0.5 - font::text_width("DESYNC DETECTED", 2.5) * 0.5,
                98.0,
                2.5,
                BAD,
            );
        }
    }

    fn draw_options(&self, p: &mut MqPainter) {
        let (w, h) = p.dims();
        title(p, "OPTIONS", 36.0);
        let pads = if self.hub.pad_count() == 0 {
            match &self.hub.init_error {
                Some(e) => format!("GAMEPADS UNAVAILABLE: {}", e.to_uppercase()),
                None => "NO GAMEPAD DETECTED - PLUG ONE IN, IT IS PICKED UP LIVE".into(),
            }
        } else {
            format!("PADS: {}", self.hub.names.join("  |  ").to_uppercase())
        };
        font::draw_text(
            p,
            &pads,
            w * 0.5 - font::text_width(&pads, 1.5) * 0.5,
            92.0,
            1.5,
            MUTED,
        );
        let rows = self.option_rows();
        let x = w * 0.5 - 330.0;
        for (i, r) in rows.iter().enumerate() {
            let sel = i == self.options_idx;
            let y = 125.0 + i as f32 * 30.0;
            if sel {
                p.fill_rect(x - 22.0, y - 4.0, 10.0, 20.0, ACCENT);
            }
            font::draw_text(p, r, x, y, 2.0, if sel { ACCENT } else { DIM });
        }
        let help = if let Some((slot, a)) = self.rebind {
            format!(
                "PRESS A BUTTON ON PAD {} FOR {}   (ESC CANCEL)",
                slot + 1,
                a.label()
            )
        } else {
            "UP/DOWN SELECT   LEFT/RIGHT ADJUST (SHIFT X100)   ENTER REBIND   ESC BACK".into()
        };
        font::draw_text(
            p,
            &help,
            w * 0.5 - font::text_width(&help, 1.5) * 0.5,
            h - 40.0,
            1.5,
            if self.rebind.is_some() { GOOD } else { MUTED },
        );
    }
}

async fn amain(opts: LaunchOpts) {
    let capture = opts.capture.clone();
    let mut app = App::new(opts);
    if let Some(c) = capture {
        // Let the window settle (some drivers report a size only after a frame).
        next_frame().await;
        match capture::run(&mut app, &c).await {
            Ok(index) => {
                eprintln!(
                    "capture: {} files, {} skipped → {}",
                    index.files.len(),
                    index.skipped.len(),
                    c.out.display()
                );
                std::process::exit(0);
            }
            Err(e) => {
                eprintln!("capture failed: {e}");
                std::process::exit(1);
            }
        }
    }
    app.audio = audio::load().await;
    loop {
        app.update();
        app.draw();
        app.drawn += 1;
        if let Some((path, at)) = app.screenshot.clone() {
            if app.drawn >= at {
                let img = get_screen_data();
                img.export_png(&path);
                eprintln!("screenshot saved to {path}");
                std::process::exit(0);
            }
        }
        if let Some((dir, at, count)) = app.record.clone() {
            if app.drawn >= at {
                let n = app.drawn - at;
                if n >= count {
                    eprintln!("recorded {count} frames to {dir}");
                    std::process::exit(0);
                }
                let _ = std::fs::create_dir_all(&dir);
                let img = get_screen_data();
                img.export_png(&format!("{dir}/frame_{n:04}.png"));
            }
        }
        next_frame().await;
    }
}

// ---- keyboard text entry helpers ----

fn take_text(buf: &mut String, max: usize) {
    take_text_filtered(buf, max, |c| {
        c.is_ascii_alphanumeric() || matches!(c, '.' | ':' | '-' | '_')
    })
}
fn take_text_filtered<F: Fn(char) -> bool>(buf: &mut String, max: usize, ok: F) {
    while let Some(c) = get_char_pressed() {
        if ok(c) && buf.len() < max {
            buf.push(c.to_ascii_uppercase());
        }
    }
    if is_key_pressed(KeyCode::Backspace) {
        buf.pop();
    }
}
