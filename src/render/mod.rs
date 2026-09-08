//! macroquad front-end: window, menus, input mapping and the fixed-timestep
//! game loop. All actual drawing goes through [`crate::viz`] via [`MqPainter`],
//! so the window renders the exact same scene the headless GIF tool does.

use macroquad::prelude::*;

use crate::sim::input::{buttons, PlayerInput};
use crate::sim::math::Vec2 as SimVec2;
use crate::sim::{GameState, MatchConfig};
use crate::viz::{self, Color as VColor, Painter, SceneOpts};

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

/// A player's keyboard binding.
struct KeyMap {
    up: KeyCode,
    down: KeyCode,
    left: KeyCode,
    right: KeyCode,
    cup: KeyCode,
    cdown: KeyCode,
    cleft: KeyCode,
    cright: KeyCode,
    jump: KeyCode,
    attack: KeyCode,
    special: KeyCode,
    shield: KeyCode,
    grab: KeyCode,
}

fn player_one_keys() -> KeyMap {
    KeyMap {
        up: KeyCode::W,
        down: KeyCode::S,
        left: KeyCode::A,
        right: KeyCode::D,
        cup: KeyCode::R,
        cdown: KeyCode::F,
        cleft: KeyCode::Q,
        cright: KeyCode::E,
        jump: KeyCode::Space,
        attack: KeyCode::C,
        special: KeyCode::V,
        shield: KeyCode::LeftShift,
        grab: KeyCode::X,
    }
}

fn player_two_keys() -> KeyMap {
    KeyMap {
        up: KeyCode::Up,
        down: KeyCode::Down,
        left: KeyCode::Left,
        right: KeyCode::Right,
        cup: KeyCode::I,
        cdown: KeyCode::K,
        cleft: KeyCode::J,
        cright: KeyCode::L,
        jump: KeyCode::RightShift,
        attack: KeyCode::Period,
        special: KeyCode::Slash,
        shield: KeyCode::RightControl,
        grab: KeyCode::Comma,
    }
}

fn read_input(k: &KeyMap) -> PlayerInput {
    let axis = |neg: KeyCode, pos: KeyCode| -> f32 {
        (is_key_down(pos) as i32 as f32) - (is_key_down(neg) as i32 as f32)
    };
    let stick = SimVec2::new(axis(k.left, k.right), axis(k.down, k.up));
    let cstick = SimVec2::new(axis(k.cleft, k.cright), axis(k.cdown, k.cup));
    let mut b = 0u16;
    if is_key_down(k.jump) {
        b |= buttons::JUMP;
    }
    if is_key_down(k.attack) {
        b |= buttons::ATTACK;
    }
    if is_key_down(k.special) {
        b |= buttons::SPECIAL;
    }
    if is_key_down(k.shield) {
        b |= buttons::SHIELD;
    }
    if is_key_down(k.grab) {
        b |= buttons::GRAB;
    }
    PlayerInput {
        stick,
        cstick,
        buttons: b,
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Screen {
    Menu,
    Versus,
    Training,
    Demo,
    Controls,
}

const MENU_ITEMS: [&str; 5] = [
    "VERSUS  2P LOCAL",
    "TRAINING  1P",
    "WATCH DEMO",
    "CONTROLS",
    "QUIT",
];

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
pub fn launch() {
    macroquad::Window::from_config(window_conf(), amain());
}

async fn amain() {
    let p1 = player_one_keys();
    let p2 = player_two_keys();

    let mut screen = Screen::Menu;
    let mut menu_idx = 0usize;

    let mut gs = GameState::new(2, MatchConfig::default());
    let mut demo_frame = 0u64;
    let mut show_boxes = true;

    // Fixed-timestep accumulator.
    let dt_fixed = 1.0 / 60.0;
    let mut acc = 0.0f32;

    loop {
        // ---------- update ----------
        match screen {
            Screen::Menu => {
                if is_key_pressed(KeyCode::S) || is_key_pressed(KeyCode::Down) {
                    menu_idx = (menu_idx + 1) % MENU_ITEMS.len();
                }
                if is_key_pressed(KeyCode::W) || is_key_pressed(KeyCode::Up) {
                    menu_idx = (menu_idx + MENU_ITEMS.len() - 1) % MENU_ITEMS.len();
                }
                if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::Space) {
                    match menu_idx {
                        0 => {
                            gs = GameState::new(2, MatchConfig::default());
                            acc = 0.0;
                            screen = Screen::Versus;
                        }
                        1 => {
                            gs = training_state();
                            acc = 0.0;
                            screen = Screen::Training;
                        }
                        2 => {
                            gs = crate::demo::match_state();
                            demo_frame = 0;
                            acc = 0.0;
                            screen = Screen::Demo;
                        }
                        3 => screen = Screen::Controls,
                        _ => return,
                    }
                }
                if is_key_pressed(KeyCode::Escape) {
                    return;
                }
            }
            Screen::Controls => {
                if is_key_pressed(KeyCode::Escape) || is_key_pressed(KeyCode::Enter) {
                    screen = Screen::Menu;
                }
            }
            Screen::Versus | Screen::Training | Screen::Demo => {
                if is_key_pressed(KeyCode::Escape) {
                    screen = Screen::Menu;
                }
                if matches!(screen, Screen::Training) && is_key_pressed(KeyCode::Tab) {
                    show_boxes = !show_boxes;
                }
                if matches!(screen, Screen::Training)
                    && (is_key_pressed(KeyCode::Backspace) || is_key_pressed(KeyCode::Key0))
                {
                    gs = training_state();
                    acc = 0.0;
                }

                acc += get_frame_time().min(0.1);
                let mut steps = 0;
                while acc >= dt_fixed && steps < 5 {
                    let inputs: Vec<PlayerInput> = match screen {
                        Screen::Versus => vec![read_input(&p1), read_input(&p2)],
                        Screen::Training => vec![read_input(&p1), PlayerInput::default()],
                        Screen::Demo => {
                            let d = crate::demo::inputs(&gs, demo_frame);
                            demo_frame += 1;
                            vec![d[0], d[1]]
                        }
                        _ => vec![],
                    };
                    gs.step(&inputs);
                    acc -= dt_fixed;
                    steps += 1;
                }
            }
        }

        // ---------- draw ----------
        clear_background(col(VColor::rgb(12, 12, 20)));
        let mut painter = MqPainter;
        match screen {
            Screen::Menu => draw_menu(&mut painter, menu_idx),
            Screen::Controls => draw_controls(&mut painter),
            Screen::Versus => {
                viz::draw_scene(
                    &mut painter,
                    &gs,
                    SceneOpts {
                        training: false,
                        watermark: true,
                    },
                );
            }
            Screen::Training => {
                viz::draw_scene(
                    &mut painter,
                    &gs,
                    SceneOpts {
                        training: show_boxes,
                        watermark: true,
                    },
                );
                hint(&mut painter, "TAB BOXES   BACKSPACE RESET   ESC MENU");
            }
            Screen::Demo => {
                viz::draw_scene(
                    &mut painter,
                    &gs,
                    SceneOpts {
                        training: false,
                        watermark: true,
                    },
                );
                hint(&mut painter, "DEMO   ESC MENU");
            }
        }

        next_frame().await;
    }
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

fn draw_menu<P: Painter>(p: &mut P, idx: usize) {
    let (w, h) = p.dims();
    // Title.
    let title = "OVERFRAME";
    let sc = 8.0;
    let tw = viz::font::text_width(title, sc);
    viz::font::draw_text(
        p,
        title,
        w * 0.5 - tw * 0.5,
        h * 0.16,
        sc,
        VColor::rgb(255, 92, 74),
    );
    let sub = "AN OPEN PLATFORM FIGHTER";
    let ssc = 2.0;
    viz::font::draw_text(
        p,
        sub,
        w * 0.5 - viz::font::text_width(sub, ssc) * 0.5,
        h * 0.16 + sc * 7.0 + 10.0,
        ssc,
        VColor::rgba(200, 210, 240, 200),
    );

    for (i, item) in MENU_ITEMS.iter().enumerate() {
        let selected = i == idx;
        let scale = 3.0;
        let y = h * 0.42 + i as f32 * 46.0;
        let color = if selected {
            VColor::rgb(255, 220, 120)
        } else {
            VColor::rgba(200, 210, 230, 200)
        };
        let x = w * 0.5 - viz::font::text_width(item, scale) * 0.5;
        if selected {
            p.fill_rect(
                x - 22.0,
                y - 4.0,
                12.0,
                viz::font::GLYPH_H * scale + 8.0,
                VColor::rgb(255, 220, 120),
            );
        }
        viz::font::draw_text(p, item, x, y, scale, color);
    }

    let foot = "MIT LICENSED   FREE   ROLLBACK-READY";
    viz::font::draw_text(
        p,
        foot,
        w * 0.5 - viz::font::text_width(foot, 1.5) * 0.5,
        h - 40.0,
        1.5,
        VColor::rgba(150, 160, 190, 160),
    );
}

fn draw_controls<P: Painter>(p: &mut P) {
    let (w, h) = p.dims();
    let title = "CONTROLS";
    viz::font::draw_text(
        p,
        title,
        w * 0.5 - viz::font::text_width(title, 5.0) * 0.5,
        50.0,
        5.0,
        VColor::rgb(255, 220, 120),
    );

    let lines = [
        "PLAYER 1                 PLAYER 2",
        "MOVE      W A S D        ARROW KEYS",
        "C-STICK   Q E R F        I J K L",
        "JUMP      SPACE          RIGHT SHIFT",
        "ATTACK    C              . (PERIOD)",
        "SPECIAL   V              / (SLASH)",
        "SHIELD    LEFT SHIFT     RIGHT CTRL",
        "GRAB      X              , (COMMA)",
        "",
        "TILTS  ATTACK + STICK      SMASH  C-STICK",
        "SHORT HOP  TAP JUMP        WAVEDASH  JUMP THEN",
        "                           AIR-DODGE (SHIELD) DOWN-FORWARD",
        "L-CANCEL  SHIELD JUST BEFORE LANDING AN AERIAL",
        "",
        "ESC  BACK TO MENU",
    ];
    let sc = 2.0;
    for (i, l) in lines.iter().enumerate() {
        viz::font::draw_text(
            p,
            l,
            w * 0.5 - 300.0,
            120.0 + i as f32 * 28.0,
            sc,
            VColor::rgba(210, 220, 240, 220),
        );
    }
    let _ = h;
}

fn hint<P: Painter>(p: &mut P, text: &str) {
    let (w, _h) = p.dims();
    viz::font::draw_text(
        p,
        text,
        w * 0.5 - viz::font::text_width(text, 1.5) * 0.5,
        16.0,
        1.5,
        VColor::rgba(180, 200, 230, 150),
    );
}
