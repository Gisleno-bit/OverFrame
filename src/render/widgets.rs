//! Small UI helpers for the menus: colours, value cyclers, and the character /
//! stage preview cards. Kept out of `mod.rs` so the screen logic stays readable.

use super::MqPainter;
use crate::sim::roster::{CharacterId, PALETTES};
use crate::sim::stage::{Stage, StageId};
use crate::viz::{fighter_color, font, Color as VColor, Painter};
use macroquad::prelude::*;

pub const ACCENT: VColor = VColor::rgb(255, 220, 120);
pub const DIM: VColor = VColor::rgba(200, 210, 230, 200);
pub const MUTED: VColor = VColor::rgba(150, 160, 190, 160);
pub const BAD: VColor = VColor::rgb(255, 90, 90);
pub const GOOD: VColor = VColor::rgb(120, 230, 150);

/// Menu navigation intent for one frame.
#[derive(Clone, Copy, Default)]
pub struct Nav {
    pub v: i32,
    pub h: i32,
    pub confirm: bool,
    pub back: bool,
}

pub fn title(p: &mut MqPainter, t: &str, y: f32) {
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

pub fn hint(p: &mut MqPainter, text: &str) {
    let (w, h) = p.dims();
    font::draw_text(
        p,
        text,
        w * 0.5 - font::text_width(text, 1.5) * 0.5,
        h - 24.0,
        1.5,
        VColor::rgba(180, 200, 230, 150),
    );
}

pub fn draw_list(p: &mut MqPainter, items: &[&str], idx: usize, y0: f32, dy: f32, scale: f32) {
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

// ---- value cyclers used by setup / lobby ----

pub fn cycle_char(c: &mut CharacterId, pal: &mut u8, nav: Nav) {
    if nav.h != 0 {
        let n = CharacterId::ALL.len() as i32;
        let i = (c.index() as i32 + nav.h).rem_euclid(n) as usize;
        *c = CharacterId::ALL[i];
    }
    if nav.v != 0 {
        // (unused here; palette changes via a dedicated row)
    }
    let _ = pal;
}

pub fn cycle_stage(s: &mut StageId, nav: Nav) {
    if nav.h != 0 {
        let n = StageId::ALL.len() as i32;
        let i = (s.index() as i32 + nav.h).rem_euclid(n) as usize;
        *s = StageId::ALL[i];
    }
}

pub fn cycle_stocks(stocks: &mut i32, nav: Nav) {
    if nav.h != 0 {
        *stocks = (*stocks + nav.h).clamp(1, 9);
    }
}

pub fn cycle_time(secs: &mut u32, nav: Nav) {
    if nav.h != 0 {
        *secs = (*secs as i32 + nav.h * 30).clamp(0, 600) as u32;
    }
}

pub fn time_label(secs: u32) -> String {
    if secs == 0 {
        "OFF".into()
    } else {
        format!("{}:{:02}", secs / 60, secs % 60)
    }
}

// ---- preview cards ----

/// A character preview card: capsule in its palette colour, name, archetype and
/// signature trait — the 2D stand-in for a model preview.
/// Where the 3D model preview goes inside a character card (relative x, y,
/// width, height).
pub const CARD_PREVIEW: (f32, f32, f32, f32) = (6.0, 30.0, 110.0, 150.0);
/// Same for a lobby slot.
pub const LOBBY_PREVIEW: (f32, f32, f32, f32) = (85.0, 72.0, 110.0, 130.0);

pub fn draw_char_card(
    p: &mut MqPainter,
    x: f32,
    y: f32,
    slot: &str,
    id: CharacterId,
    palette: u8,
    selected: bool,
) {
    let ch = id.data();
    let c = fighter_color(id.index(), palette);
    let bw = 300.0;
    let bh = 210.0;
    let bg = if selected {
        VColor::rgba(40, 44, 66, 220)
    } else {
        VColor::rgba(18, 20, 34, 200)
    };
    p.fill_rect(x, y, bw, bh, bg);
    p.fill_rect(x, y, bw, 3.0, if selected { ACCENT } else { c });

    font::draw_text(
        p,
        slot,
        x + 12.0,
        y + 10.0,
        1.5,
        if selected { ACCENT } else { MUTED },
    );

    // (The 3D preview is drawn by the app into CARD_PREVIEW.)
    // palette swatches + name
    for i in 0..PALETTES {
        let sw = fighter_color(id.index(), i);
        let sx = x + 12.0 + i as f32 * 17.0;
        p.fill_rect(sx, y + 182.0, 13.0, 13.0, sw);
        if i == palette {
            p.fill_rect(sx - 1.0, y + 197.0, 15.0, 2.0, VColor::rgb(255, 255, 255));
        }
    }
    font::draw_text(
        p,
        &crate::viz::palette_name(id, palette).to_uppercase(),
        x + 12.0 + PALETTES as f32 * 17.0 + 6.0,
        y + 184.0,
        1.2,
        MUTED,
    );

    // Text on the right.
    let tx = x + 120.0;
    font::draw_text(p, &ch.name.to_uppercase(), tx, y + 34.0, 3.0, c);
    font::draw_text(p, ch.archetype, tx, y + 66.0, 1.5, DIM);
    font::draw_text(
        p,
        &format!("TRAIT: {}", ch.trait_name),
        tx,
        y + 92.0,
        1.5,
        ACCENT,
    );
    // wrap the trait description in ~18-char lines
    for (i, line) in wrap(ch.trait_desc, 20).iter().enumerate().take(4) {
        font::draw_text(p, line, tx, y + 112.0 + i as f32 * 16.0, 1.2, MUTED);
    }
}

/// A stage preview: its platforms drawn to scale inside a small frame.
pub fn draw_stage_card(p: &mut MqPainter, x: f32, y: f32, id: StageId, selected: bool) {
    let stage = Stage::by_id(id);
    let bw = 260.0;
    let bh = 150.0;
    p.fill_rect(x, y, bw, bh, VColor::rgba(18, 20, 34, 220));
    p.fill_rect(
        x,
        y,
        bw,
        3.0,
        if selected {
            ACCENT
        } else {
            crate::viz::Color::rgb(stage.theme.lip.0, stage.theme.lip.1, stage.theme.lip.2)
        },
    );
    font::draw_text(
        p,
        &id.name().to_uppercase(),
        x + 10.0,
        y + 10.0,
        1.8,
        if selected { ACCENT } else { DIM },
    );

    // Map world coords into the lower part of the card.
    let world_half = 210.0;
    let scale = (bw - 20.0) / (2.0 * world_half);
    let cx = x + bw * 0.5;
    let base_y = y + bh - 24.0;
    for plat in &stage.platforms {
        let px0 = cx + plat.left * scale;
        let px1 = cx + plat.right * scale;
        let py = base_y - plat.y * scale;
        let c = if plat.solid {
            crate::viz::Color::rgb(stage.theme.slab.0, stage.theme.slab.1, stage.theme.slab.2)
        } else {
            crate::viz::Color::rgb(stage.theme.soft.0, stage.theme.soft.1, stage.theme.soft.2)
        };
        let th = if plat.solid { 10.0 } else { 4.0 };
        p.fill_rect(px0, py, px1 - px0, th, c);
        p.fill_rect(
            px0,
            py,
            px1 - px0,
            2.0,
            crate::viz::Color::rgb(stage.theme.lip.0, stage.theme.lip.1, stage.theme.lip.2),
        );
    }
}

/// One player's slot in the lobby: character, palette swatches, ready state.
#[allow(clippy::too_many_arguments)]
pub fn draw_lobby_pick(
    p: &mut MqPainter,
    x: f32,
    y: f32,
    who: &str,
    pick: crate::netcode::Pick,
    lobby_idx: usize,
    is_me: bool,
    _host: bool,
) {
    let ch = pick.character.data();
    let c = fighter_color(pick.character.index(), pick.palette);
    let bw = 280.0;
    let bh = 250.0;
    p.fill_rect(x, y, bw, bh, VColor::rgba(18, 20, 34, 210));
    p.fill_rect(x, y, bw, 3.0, c);

    font::draw_text(
        p,
        &who.to_uppercase(),
        x + 12.0,
        y + 10.0,
        2.0,
        if is_me { ACCENT } else { DIM },
    );
    // (3D preview drawn by the app into LOBBY_PREVIEW.)
    font::draw_text(p, &ch.name.to_uppercase(), x + 12.0, y + 40.0, 2.5, c);

    // Row markers when this is the editable (my) card.
    if is_me {
        let char_sel = lobby_idx == 0;
        let pal_sel = lobby_idx == 1;
        if char_sel {
            font::draw_text(p, "< CHARACTER >", x + 12.0, y + 68.0, 1.5, ACCENT);
        }
        if pal_sel {
            font::draw_text(p, "< PALETTE >", x + 150.0, y + 68.0, 1.5, ACCENT);
        }
    }

    // palette swatches
    for i in 0..PALETTES {
        let sw = fighter_color(pick.character.index(), i);
        let sx = x + 14.0 + i as f32 * 18.0;
        p.fill_rect(sx, y + bh - 40.0, 14.0, 14.0, sw);
        if i == pick.palette {
            p.fill_rect(
                sx - 1.0,
                y + bh - 25.0,
                16.0,
                2.0,
                VColor::rgb(255, 255, 255),
            );
        }
    }

    // ready badge
    let (txt, c2) = if pick.ready {
        ("READY", GOOD)
    } else {
        ("NOT READY", MUTED)
    };
    font::draw_text(
        p,
        txt,
        x + bw - font::text_width(txt, 1.8) - 12.0,
        y + bh - 40.0,
        1.8,
        c2,
    );
    if is_me && lobby_idx == 2 {
        font::draw_text(
            p,
            "ENTER = TOGGLE",
            x + bw - 150.0,
            y + bh - 20.0,
            1.2,
            ACCENT,
        );
    }
}

fn wrap(s: &str, width: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut line = String::new();
    for word in s.split_whitespace() {
        if line.len() + word.len() + 1 > width && !line.is_empty() {
            out.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    if !line.is_empty() {
        out.push(line);
    }
    out
}

pub fn draw_controls(p: &mut MqPainter) {
    let (w, _h) = p.dims();
    title(p, "CONTROLS", 36.0);
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
        "GAMEPAD   ATTACK A   SPECIAL B   JUMP X/Y   SHIELD LT/RT/LB   GRAB RB",
        "          LEFT STICK = STICK, RIGHT STICK = C-STICK. REBIND IN OPTIONS.",
        "",
        "TILT  ATTACK + HELD STICK      SMASH  C-STICK FLICK",
        "SHORT HOP TAP JUMP   WAVEDASH JUMP THEN SHIELD DOWN-FORWARD",
        "L-CANCEL  SHIELD JUST BEFORE LANDING AN AERIAL",
        "",
        "MENUS  ARROWS/STICK MOVE   ENTER SELECT   ESC BACK   T CHAT (LOBBY)",
        "",
        "ESC  BACK",
    ];
    for (i, l) in lines.iter().enumerate() {
        font::draw_text(p, l, w * 0.5 - 340.0, 100.0 + i as f32 * 26.0, 2.0, DIM);
    }
}
