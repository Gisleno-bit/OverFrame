//! macroquad front-end: window, menus, input, the fixed-timestep game loop and
//! the online (GGRS) match loop. All drawing goes through [`crate::viz`] via
//! [`MqPainter`], so the window renders the exact same scene the headless GIF
//! tool does.
//!
//! Screens: Menu → Versus / Training / Demo / Controls / Options / Online
//! (Host or Join → Lobby → OnlineMatch → OnlineEnded).

mod input;

use macroquad::prelude::*;

use crate::config::Settings;
use crate::gamepad::{mask_label, PadAction};
use crate::netcode::banlist::BanList;
use crate::netcode::{roomcode, Advance, NetMatch, Phase, Role};
use crate::sim::input::PlayerInput;
use crate::sim::math::Vec2 as SimVec2;
use crate::sim::{GameState, MatchConfig};
use crate::viz::{self, font, Color as VColor, Painter, SceneOpts};
use input::InputHub;

/// Adapts macroquad's immediate-mode drawing to the [`Painter`] trait.
struct MqPainter;

#[inline]
fn col(c: VColor) -> macroquad::color::Color {
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
        (screen_width(), screen_height())
    }
}

/// Command-line options (`overframe --host [port]`, `overframe --join <code>`).
#[derive(Clone, Debug, Default)]
pub struct LaunchOpts {
    pub host: Option<u16>,
    pub join: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Screen {
    Menu,
    Versus,
    Training,
    Demo,
    Controls,
    Options,
    OnlineMenu,
    Lobby,
    OnlineMatch,
    OnlineEnded,
}

const MENU_ITEMS: [&str; 7] = [
    "VERSUS  2P LOCAL",
    "ONLINE",
    "TRAINING  1P",
    "WATCH DEMO",
    "OPTIONS",
    "CONTROLS",
    "QUIT",
];
const ONLINE_ITEMS: [&str; 3] = ["HOST GAME", "JOIN GAME", "BACK"];

const ACCENT: VColor = VColor::rgb(255, 220, 120);
const DIM: VColor = VColor::rgba(200, 210, 230, 200);
const MUTED: VColor = VColor::rgba(150, 160, 190, 160);
const BAD: VColor = VColor::rgb(255, 90, 90);
const GOOD: VColor = VColor::rgb(120, 230, 150);

fn window_conf() -> Conf {
    Conf {
        window_title: "OVERFRAME".to_owned(),
        window_width: 1280,
        window_height: 720,
        high_dpi: true,
        ..Default::default()
    }
}

/// Launch the game (blocks until the window closes).
pub fn launch(opts: LaunchOpts) {
    macroquad::Window::from_config(window_conf(), amain(opts));
}

/// Menu navigation for this frame, merged from keyboard and any gamepad.
#[derive(Clone, Copy, Default)]
struct Nav {
    v: i32,
    h: i32,
    confirm: bool,
    back: bool,
}

struct App {
    settings: Settings,
    hub: InputHub,
    screen: Screen,
    frame_no: u64,

    menu_idx: usize,
    online_idx: usize,
    options_idx: usize,

    gs: GameState,
    demo_frame: u64,
    show_boxes: bool,
    acc: f32,

    net: Option<NetMatch>,
    join_text: String,
    status: String,
    waiting_ticks: u32,
    last_rollback_shown: u32,

    rebind: Option<(usize, PadAction)>,
    rebind_started: u64,

    stick_nav_cooldown: f32,
}

impl App {
    fn new(opts: LaunchOpts) -> App {
        let settings = Settings::load_or_create();
        let mut app = App {
            settings,
            hub: InputHub::new(),
            screen: Screen::Menu,
            frame_no: 0,
            menu_idx: 0,
            online_idx: 0,
            options_idx: 0,
            gs: GameState::new(2, MatchConfig::default()),
            demo_frame: 0,
            show_boxes: true,
            acc: 0.0,
            net: None,
            join_text: String::new(),
            status: String::new(),
            waiting_ticks: 0,
            last_rollback_shown: 0,
            rebind: None,
            rebind_started: 0,
            stick_nav_cooldown: 0.0,
        };
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
        // Gamepad: South confirms, East backs, left stick nudges with a repeat delay.
        if let Some(b) = self.hub.any_just_pressed() {
            match b {
                crate::gamepad::PadButton::South => n.confirm = true,
                crate::gamepad::PadButton::East => n.back = true,
                crate::gamepad::PadButton::DPadDown => n.v += 1,
                crate::gamepad::PadButton::DPadUp => n.v -= 1,
                crate::gamepad::PadButton::DPadLeft => n.h -= 1,
                crate::gamepad::PadButton::DPadRight => n.h += 1,
                _ => {}
            }
        }
        self.stick_nav_cooldown = (self.stick_nav_cooldown - get_frame_time()).max(0.0);
        let s = self.hub.any_stick();
        if self.stick_nav_cooldown <= 0.0 && s.length() > 0.6 {
            if s.y.abs() > s.x.abs() {
                n.v += if s.y < 0.0 { 1 } else { -1 };
            } else {
                n.h += if s.x > 0.0 { 1 } else { -1 };
            }
            self.stick_nav_cooldown = 0.22;
        }
        n
    }

    fn training_state() -> GameState {
        let mut gs = GameState::new(
            2,
            MatchConfig {
                stocks: 99,
                seed: 7,
            },
        );
        gs.fighters[0].pos = SimVec2::new(-40.0, 1.0);
        gs.fighters[1].pos = SimVec2::new(40.0, 1.0);
        gs
    }

    // ------------------------------------------------------------ online

    fn start_host(&mut self, port: u16) {
        match NetMatch::host(port, &self.settings, BanList::load_local()) {
            Ok(m) => {
                self.net = Some(m);
                self.status.clear();
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
        match NetMatch::join(addr, &self.settings) {
            Ok(m) => {
                self.net = Some(m);
                self.status.clear();
                self.screen = Screen::Lobby;
            }
            Err(e) => self.status = format!("CANNOT OPEN SOCKET: {e}").to_uppercase(),
        }
    }

    fn leave_online(&mut self) {
        self.net = None;
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
                let n = MENU_ITEMS.len() as i32;
                self.menu_idx = ((self.menu_idx as i32 + nav.v).rem_euclid(n)) as usize;
                if nav.confirm {
                    match self.menu_idx {
                        0 => {
                            self.gs = GameState::new(2, MatchConfig::default());
                            self.acc = 0.0;
                            self.screen = Screen::Versus;
                        }
                        1 => {
                            self.status.clear();
                            self.screen = Screen::OnlineMenu;
                        }
                        2 => {
                            self.gs = Self::training_state();
                            self.acc = 0.0;
                            self.screen = Screen::Training;
                        }
                        3 => {
                            self.gs = crate::demo::match_state();
                            self.demo_frame = 0;
                            self.acc = 0.0;
                            self.screen = Screen::Demo;
                        }
                        4 => {
                            self.options_idx = 0;
                            self.screen = Screen::Options;
                        }
                        5 => self.screen = Screen::Controls,
                        _ => std::process::exit(0),
                    }
                }
                if nav.back {
                    std::process::exit(0);
                }
            }
            Screen::Controls => {
                if nav.back || nav.confirm {
                    self.screen = Screen::Menu;
                }
            }
            Screen::Options => self.update_options(nav),
            Screen::OnlineMenu => {
                let n = ONLINE_ITEMS.len() as i32;
                self.online_idx = ((self.online_idx as i32 + nav.v).rem_euclid(n)) as usize;
                if nav.back {
                    self.screen = Screen::Menu;
                } else if nav.confirm {
                    match self.online_idx {
                        0 => {
                            let port = self.settings.host_port;
                            self.start_host(port);
                        }
                        1 => {
                            // Text entry mode: Enter confirms (handled below).
                            self.status = "TYPE THE HOST'S CODE OR IP:PORT".into();
                            self.online_idx = 1;
                            self.screen = Screen::Lobby; // guest entry lives in Lobby
                            self.net = None;
                        }
                        _ => self.screen = Screen::Menu,
                    }
                }
            }
            Screen::Lobby => self.update_lobby(nav),
            Screen::OnlineMatch => self.update_online_match(),
            Screen::OnlineEnded => {
                if nav.back || nav.confirm {
                    self.leave_online();
                }
            }
            Screen::Versus | Screen::Training | Screen::Demo => {
                if nav.back {
                    self.screen = Screen::Menu;
                }
                if self.screen == Screen::Training && is_key_pressed(KeyCode::Tab) {
                    self.show_boxes = !self.show_boxes;
                }
                if self.screen == Screen::Training
                    && (is_key_pressed(KeyCode::Backspace) || is_key_pressed(KeyCode::Key0))
                {
                    self.gs = Self::training_state();
                    self.acc = 0.0;
                }
                self.acc += get_frame_time().min(0.1);
                let dt_fixed = 1.0 / 60.0;
                let mut steps = 0;
                while self.acc >= dt_fixed && steps < 5 {
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
                    self.acc -= dt_fixed;
                    steps += 1;
                }
            }
        }
    }

    fn update_lobby(&mut self, nav: Nav) {
        // Guest text entry (no NetMatch yet).
        if self.net.is_none() {
            while let Some(c) = get_char_pressed() {
                if (c.is_ascii_alphanumeric() || matches!(c, '.' | ':' | '-'))
                    && self.join_text.len() < 24
                {
                    self.join_text.push(c.to_ascii_uppercase());
                }
            }
            if is_key_pressed(KeyCode::Backspace) {
                self.join_text.pop();
            }
            if is_key_pressed(KeyCode::Enter) {
                self.start_join();
            }
            if nav.back {
                self.screen = Screen::OnlineMenu;
            }
            return;
        }

        let Some(net) = self.net.as_mut() else {
            return;
        };
        net.poll();
        match net.phase().clone() {
            Phase::Running => {
                self.gs = net.initial_state();
                self.acc = 0.0;
                self.waiting_ticks = 0;
                self.screen = Screen::OnlineMatch;
            }
            Phase::Ended(reason) => {
                self.status = reason;
                self.screen = Screen::OnlineEnded;
            }
            _ => {}
        }
        if nav.back {
            self.leave_online();
        }
    }

    fn update_online_match(&mut self) {
        if is_key_pressed(KeyCode::Escape) {
            self.leave_online();
            return;
        }
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

        self.acc += get_frame_time().min(0.1);
        let dt_fixed = 1.0 / 60.0;
        let mut steps = 0;
        while self.acc >= dt_fixed && steps < 5 {
            let local = self.hub.player_input(0, &self.settings.pad[0]);
            match net.advance(local, &mut self.gs) {
                Advance::Advanced { rollback_frames } => {
                    self.waiting_ticks = 0;
                    if rollback_frames > 0 {
                        self.last_rollback_shown = rollback_frames;
                    }
                }
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
            self.acc -= dt_fixed;
            steps += 1;
        }
    }

    // ------------------------------------------------------------ options

    /// Rows of the options screen.
    fn option_rows(&self) -> Vec<String> {
        let mut rows = vec![
            format!("INPUT DELAY   {} FRAMES", self.settings.input_delay),
            format!("HOST PORT     {}", self.settings.host_port),
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
        // Capturing a button for a rebind?
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
            0 => {
                if nav.h != 0 {
                    let d = (self.settings.input_delay as i32 + nav.h).clamp(0, 8);
                    self.settings.input_delay = d as u8;
                    let _ = self.settings.save();
                }
            }
            1 => {
                if nav.h != 0 {
                    let step = if shift { 100 } else { 1 };
                    let p = (self.settings.host_port as i32 + nav.h * step).clamp(1024, 65535);
                    self.settings.host_port = p as u16;
                    let _ = self.settings.save();
                }
            }
            i if i >= 2 && i < 2 + 2 * pad_rows => {
                if nav.confirm {
                    let slot = (i - 2) / pad_rows;
                    let action = PadAction::ALL[(i - 2) % pad_rows];
                    self.rebind = Some((slot, action));
                    self.rebind_started = self.frame_no;
                }
            }
            i if i == 2 + 2 * pad_rows => {
                if nav.confirm {
                    self.settings.pad = Default::default();
                    let _ = self.settings.save();
                }
            }
            _ => {
                if nav.confirm {
                    let _ = self.settings.save();
                    self.screen = Screen::Menu;
                }
            }
        }
    }

    // ------------------------------------------------------------ draw

    fn draw(&self) {
        clear_background(col(VColor::rgb(12, 12, 20)));
        let mut p = MqPainter;
        match self.screen {
            Screen::Menu => self.draw_menu(&mut p),
            Screen::Controls => draw_controls(&mut p),
            Screen::Options => self.draw_options(&mut p),
            Screen::OnlineMenu => self.draw_online_menu(&mut p),
            Screen::Lobby => self.draw_lobby(&mut p),
            Screen::OnlineEnded => self.draw_ended(&mut p),
            Screen::Versus => {
                viz::draw_scene(
                    &mut p,
                    &self.gs,
                    SceneOpts {
                        training: false,
                        watermark: true,
                        local_player: None,
                    },
                );
            }
            Screen::Training => {
                viz::draw_scene(
                    &mut p,
                    &self.gs,
                    SceneOpts {
                        training: self.show_boxes,
                        watermark: true,
                        local_player: None,
                    },
                );
                hint(&mut p, "TAB BOXES   BACKSPACE RESET   ESC MENU");
            }
            Screen::Demo => {
                viz::draw_scene(
                    &mut p,
                    &self.gs,
                    SceneOpts {
                        training: false,
                        watermark: true,
                        local_player: None,
                    },
                );
                hint(&mut p, "DEMO   ESC MENU");
            }
            Screen::OnlineMatch => {
                let local = self.net.as_ref().map(|n| n.local_handle());
                viz::draw_scene(
                    &mut p,
                    &self.gs,
                    SceneOpts {
                        training: false,
                        watermark: true,
                        local_player: local,
                    },
                );
                self.draw_online_overlay(&mut p);
            }
        }
    }

    fn footer(&self, p: &mut MqPainter) {
        let (w, h) = p.dims();
        let pads = if self.hub.pad_count() == 0 {
            "NO GAMEPAD".to_owned()
        } else {
            format!(
                "{} GAMEPAD(S): {}",
                self.hub.pad_count(),
                self.hub.names.join(", ")
            )
        };
        let line = format!(
            "ID {:08X}   {}   MIT   FREE   ROLLBACK NETCODE",
            (self.settings.player_id >> 32) as u32,
            pads.to_uppercase()
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
        let title = "OVERFRAME";
        let sc = 8.0;
        font::draw_text(
            p,
            title,
            w * 0.5 - font::text_width(title, sc) * 0.5,
            h * 0.13,
            sc,
            VColor::rgb(255, 92, 74),
        );
        let sub = "AN OPEN PLATFORM FIGHTER";
        font::draw_text(
            p,
            sub,
            w * 0.5 - font::text_width(sub, 2.0) * 0.5,
            h * 0.13 + sc * 7.0 + 10.0,
            2.0,
            DIM,
        );
        draw_list(p, &MENU_ITEMS, self.menu_idx, h * 0.36, 44.0, 3.0);
        self.footer(p);
    }

    fn draw_online_menu(&self, p: &mut MqPainter) {
        let (w, h) = p.dims();
        title(p, "ONLINE", 50.0);
        draw_list(p, &ONLINE_ITEMS, self.online_idx, h * 0.36, 46.0, 3.0);
        let info = [
            format!("HOSTING USES UDP PORT {}", self.settings.host_port),
            "LAN: SHARE THE CODE SHOWN WHEN HOSTING".into(),
            "INTERNET: FORWARD THAT UDP PORT AND SHARE YOUR PUBLIC IP:PORT".into(),
        ];
        for (i, l) in info.iter().enumerate() {
            font::draw_text(
                p,
                l,
                w * 0.5 - font::text_width(l, 1.5) * 0.5,
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
                h * 0.58,
                2.0,
                BAD,
            );
        }
        self.footer(p);
    }

    fn draw_lobby(&self, p: &mut MqPainter) {
        let (w, h) = p.dims();
        match &self.net {
            None => {
                title(p, "JOIN GAME", 50.0);
                let label = "HOST CODE OR IP:PORT";
                font::draw_text(
                    p,
                    label,
                    w * 0.5 - font::text_width(label, 2.0) * 0.5,
                    h * 0.34,
                    2.0,
                    DIM,
                );
                // Text box.
                let bw = 520.0;
                let bx = w * 0.5 - bw * 0.5;
                let by = h * 0.42;
                p.fill_rect(bx, by, bw, 56.0, VColor::rgba(10, 12, 22, 200));
                p.fill_rect(bx, by + 54.0, bw, 2.0, ACCENT);
                let caret = if (self.frame_no / 30) % 2 == 0 {
                    "_"
                } else {
                    " "
                };
                let shown = format!("{}{}", self.join_text, caret);
                font::draw_text(
                    p,
                    &shown,
                    bx + 16.0,
                    by + 14.0,
                    4.0,
                    VColor::rgb(240, 240, 240),
                );
                let help = "ENTER  CONNECT      ESC  BACK";
                font::draw_text(
                    p,
                    help,
                    w * 0.5 - font::text_width(help, 1.5) * 0.5,
                    h * 0.56,
                    1.5,
                    MUTED,
                );
                if !self.status.is_empty() {
                    let c = if self.status.starts_with("TYPE") {
                        MUTED
                    } else {
                        BAD
                    };
                    font::draw_text(
                        p,
                        &self.status,
                        w * 0.5 - font::text_width(&self.status, 2.0) * 0.5,
                        h * 0.63,
                        2.0,
                        c,
                    );
                }
            }
            Some(net) => {
                let hosting = net.role() == Role::Host;
                title(p, if hosting { "HOSTING" } else { "CONNECTING" }, 50.0);
                if hosting {
                    let l = "ROOM CODE";
                    font::draw_text(
                        p,
                        l,
                        w * 0.5 - font::text_width(l, 2.0) * 0.5,
                        h * 0.30,
                        2.0,
                        DIM,
                    );
                    let code = &net.room_code;
                    let sc = 7.0;
                    font::draw_text(
                        p,
                        code,
                        w * 0.5 - font::text_width(code, sc) * 0.5,
                        h * 0.36,
                        sc,
                        ACCENT,
                    );
                    let addr = format!("LAN {}:{}", roomcode::local_ipv4(), net.local_addr.port());
                    font::draw_text(
                        p,
                        &addr,
                        w * 0.5 - font::text_width(&addr, 2.0) * 0.5,
                        h * 0.36 + sc * 7.0 + 18.0,
                        2.0,
                        DIM,
                    );
                    let tip = "INTERNET: FORWARD THIS UDP PORT AND SHARE PUBLIC-IP:PORT INSTEAD";
                    font::draw_text(
                        p,
                        tip,
                        w * 0.5 - font::text_width(tip, 1.5) * 0.5,
                        h * 0.36 + sc * 7.0 + 44.0,
                        1.5,
                        MUTED,
                    );
                }
                let dots = ".".repeat(((self.frame_no / 20) % 4) as usize);
                let st = match net.phase() {
                    Phase::Handshaking => {
                        if hosting {
                            format!("WAITING FOR A PLAYER{dots}")
                        } else {
                            format!("LOOKING FOR HOST{dots}")
                        }
                    }
                    Phase::Syncing => format!("SYNCHRONISING WITH {}{dots}", net.peer_name()),
                    _ => String::new(),
                };
                font::draw_text(
                    p,
                    &st,
                    w * 0.5 - font::text_width(&st, 2.5) * 0.5,
                    h * 0.72,
                    2.5,
                    GOOD,
                );
                hint(p, "ESC  CANCEL");
            }
        }
        self.footer(p);
    }

    fn draw_ended(&self, p: &mut MqPainter) {
        let (w, h) = p.dims();
        title(p, "MATCH ENDED", 50.0);
        let r = self.status.to_uppercase();
        font::draw_text(
            p,
            &r,
            w * 0.5 - font::text_width(&r, 3.0) * 0.5,
            h * 0.42,
            3.0,
            BAD,
        );
        let help = "ENTER / ESC  BACK TO MENU";
        font::draw_text(
            p,
            help,
            w * 0.5 - font::text_width(help, 1.5) * 0.5,
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
        let stats = net.stats().unwrap_or_default();
        let role = if net.role() == Role::Host {
            "HOST"
        } else {
            "GUEST"
        };
        let line = format!(
            "ONLINE {}  VS {}   PING {}MS   ROLLBACK {}F   AHEAD {:+}   DELAY {}F",
            role,
            net.peer_name(),
            stats.ping_ms,
            stats.rollback_frames,
            stats.frames_ahead,
            self.settings.input_delay
        );
        p.fill_rect(
            8.0,
            8.0,
            font::text_width(&line, 1.5) + 16.0,
            22.0,
            VColor::rgba(10, 12, 22, 170),
        );
        let c = if stats.ping_ms > 120 {
            BAD
        } else if stats.ping_ms > 60 {
            ACCENT
        } else {
            GOOD
        };
        font::draw_text(p, &line, 16.0, 13.0, 1.5, c);

        if net.role() == Role::Host {
            let code = format!("CODE {}", net.room_code);
            let x = w - font::text_width(&code, 1.5) - 16.0;
            font::draw_text(p, &code, x, 40.0, 1.5, MUTED);
        }
        if self.waiting_ticks > 6 {
            let t = "WAITING FOR PEER...";
            font::draw_text(
                p,
                t,
                w * 0.5 - font::text_width(t, 2.5) * 0.5,
                60.0,
                2.5,
                ACCENT,
            );
        }
        if stats.desynced {
            let t = "DESYNC DETECTED";
            p.fill_rect(
                w * 0.5 - 150.0,
                90.0,
                300.0,
                34.0,
                VColor::rgba(120, 0, 0, 200),
            );
            font::draw_text(
                p,
                t,
                w * 0.5 - font::text_width(t, 2.5) * 0.5,
                98.0,
                2.5,
                BAD,
            );
        }
    }

    fn draw_options(&self, p: &mut MqPainter) {
        let (w, h) = p.dims();
        title(p, "OPTIONS", 40.0);
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
            100.0,
            1.5,
            MUTED,
        );

        let rows = self.option_rows();
        let x = w * 0.5 - 330.0;
        let y0 = 135.0;
        let dy = 30.0;
        for (i, r) in rows.iter().enumerate() {
            let sel = i == self.options_idx;
            let y = y0 + i as f32 * dy;
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
            "UP/DOWN SELECT   LEFT/RIGHT ADJUST (SHIFT = X100 FOR PORT)   ENTER REBIND   ESC BACK"
                .into()
        };
        let c = if self.rebind.is_some() { GOOD } else { MUTED };
        font::draw_text(
            p,
            &help,
            w * 0.5 - font::text_width(&help, 1.5) * 0.5,
            h - 60.0,
            1.5,
            c,
        );
        let path = format!("SAVED TO {}", Settings::path().display()).to_uppercase();
        font::draw_text(
            p,
            &path,
            w * 0.5 - font::text_width(&path, 1.2) * 0.5,
            h - 34.0,
            1.2,
            MUTED,
        );
    }
}

async fn amain(opts: LaunchOpts) {
    let mut app = App::new(opts);
    loop {
        app.update();
        app.draw();
        next_frame().await;
    }
}

// ---------------------------------------------------------------- widgets

fn title(p: &mut MqPainter, t: &str, y: f32) {
    let (w, _) = p.dims();
    font::draw_text(
        p,
        t,
        w * 0.5 - font::text_width(t, 5.0) * 0.5,
        y,
        5.0,
        ACCENT,
    );
}

fn draw_list(p: &mut MqPainter, items: &[&str], idx: usize, y0: f32, dy: f32, scale: f32) {
    let (w, _) = p.dims();
    for (i, item) in items.iter().enumerate() {
        let selected = i == idx;
        let y = y0 + i as f32 * dy;
        let color = if selected { ACCENT } else { DIM };
        let x = w * 0.5 - font::text_width(item, scale) * 0.5;
        if selected {
            p.fill_rect(x - 22.0, y - 4.0, 12.0, font::GLYPH_H * scale + 8.0, ACCENT);
        }
        font::draw_text(p, item, x, y, scale, color);
    }
}

fn draw_controls(p: &mut MqPainter) {
    let (w, _h) = p.dims();
    title(p, "CONTROLS", 40.0);
    let lines = [
        "KEYBOARD  PLAYER 1              PLAYER 2",
        "MOVE      W A S D               ARROW KEYS",
        "C-STICK   Q E R F               I J K L",
        "JUMP      SPACE                 RIGHT SHIFT",
        "ATTACK    C                     . (PERIOD)",
        "SPECIAL   V                     / (SLASH)",
        "SHIELD    LEFT SHIFT            RIGHT CTRL",
        "GRAB      X                     , (COMMA)",
        "",
        "GAMEPAD (DEFAULT, REBIND IN OPTIONS)",
        "STICK / C-STICK   LEFT / RIGHT STICK",
        "ATTACK A   SPECIAL B   JUMP X OR Y   SHIELD LT/RT/LB   GRAB RB",
        "FIRST PAD = P1, SECOND PAD = P2. KEYBOARD ALWAYS WORKS TOO.",
        "",
        "TILT  ATTACK + HELD STICK     SMASH  C-STICK FLICK",
        "SHORT HOP  TAP JUMP    WAVEDASH  JUMP, THEN SHIELD DOWN-FORWARD",
        "L-CANCEL  SHIELD JUST BEFORE LANDING AN AERIAL",
        "",
        "ESC  BACK",
    ];
    for (i, l) in lines.iter().enumerate() {
        font::draw_text(p, l, w * 0.5 - 330.0, 110.0 + i as f32 * 27.0, 2.0, DIM);
    }
}

fn hint(p: &mut MqPainter, text: &str) {
    let (w, _h) = p.dims();
    font::draw_text(
        p,
        text,
        w * 0.5 - font::text_width(text, 1.5) * 0.5,
        16.0,
        1.5,
        VColor::rgba(180, 200, 230, 150),
    );
}
