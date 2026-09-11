//! A tiny 5x7 bitmap font, drawn through the [`super::Painter::fill_rect`]
//! primitive so the *same* text rendering works on both the macroquad backend
//! and the headless CPU rasteriser (no font files, no GPU text).

/// Each glyph is 7 rows of 5 bits (bit 4 = leftmost column).
fn glyph(c: char) -> [u8; 7] {
    match c.to_ascii_uppercase() {
        '0' => [
            0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110,
        ],
        '1' => [
            0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ],
        '2' => [
            0b01110, 0b10001, 0b00001, 0b00110, 0b01000, 0b10000, 0b11111,
        ],
        '3' => [
            0b11111, 0b00010, 0b00100, 0b00010, 0b00001, 0b10001, 0b01110,
        ],
        '4' => [
            0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010,
        ],
        '5' => [
            0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110,
        ],
        '6' => [
            0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110,
        ],
        '7' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000,
        ],
        '8' => [
            0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110,
        ],
        '9' => [
            0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100,
        ],
        'A' => [
            0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        'B' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110,
        ],
        'C' => [
            0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110,
        ],
        'D' => [
            0b11100, 0b10010, 0b10001, 0b10001, 0b10001, 0b10010, 0b11100,
        ],
        'E' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111,
        ],
        'F' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        'G' => [
            0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01111,
        ],
        'H' => [
            0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        'I' => [
            0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ],
        'J' => [
            0b00111, 0b00010, 0b00010, 0b00010, 0b10010, 0b10010, 0b01100,
        ],
        'K' => [
            0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001,
        ],
        'L' => [
            0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111,
        ],
        'M' => [
            0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001,
        ],
        'N' => [
            0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001,
        ],
        'O' => [
            0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'P' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        'Q' => [
            0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101,
        ],
        'R' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001,
        ],
        'S' => [
            0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110,
        ],
        'T' => [
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        'U' => [
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'V' => [
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100,
        ],
        'W' => [
            0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b11011, 0b10001,
        ],
        'X' => [
            0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001,
        ],
        'Y' => [
            0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        'Z' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111,
        ],
        '%' => [
            0b11001, 0b11010, 0b00010, 0b00100, 0b01000, 0b01011, 0b10011,
        ],
        ':' => [
            0b00000, 0b00100, 0b00100, 0b00000, 0b00100, 0b00100, 0b00000,
        ],
        '-' => [
            0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000,
        ],
        '.' => [
            0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b01100, 0b01100,
        ],
        '!' => [
            0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00000, 0b00100,
        ],
        '/' => [
            0b00001, 0b00010, 0b00010, 0b00100, 0b01000, 0b01000, 0b10000,
        ],
        '+' => [
            0b00000, 0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0b00000,
        ],
        '(' => [
            0b00010, 0b00100, 0b01000, 0b01000, 0b01000, 0b00100, 0b00010,
        ],
        ')' => [
            0b01000, 0b00100, 0b00010, 0b00010, 0b00010, 0b00100, 0b01000,
        ],
        '=' => [
            0b00000, 0b00000, 0b11111, 0b00000, 0b11111, 0b00000, 0b00000,
        ],
        '\'' => [
            0b00100, 0b00100, 0b01000, 0b00000, 0b00000, 0b00000, 0b00000,
        ],
        ',' => [
            0b00000, 0b00000, 0b00000, 0b00000, 0b01100, 0b00100, 0b01000,
        ],
        '_' => [
            0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b11111,
        ],
        '?' => [
            0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b00000, 0b00100,
        ],
        '*' => [
            0b00000, 0b10101, 0b01110, 0b11111, 0b01110, 0b10101, 0b00000,
        ],
        '<' => [
            0b00010, 0b00100, 0b01000, 0b10000, 0b01000, 0b00100, 0b00010,
        ],
        '>' => [
            0b01000, 0b00100, 0b00010, 0b00001, 0b00010, 0b00100, 0b01000,
        ],
        _ => [0; 7], // space and unknowns render blank
    }
}

/// Width of one glyph cell (5) plus 1px spacing, times scale.
pub const GLYPH_W: f32 = 6.0;
pub const GLYPH_H: f32 = 7.0;

/// Draw `text` at (`x`,`y`) top-left, each "pixel" a `scale`×`scale` rect.
pub fn draw_text<P: super::Painter>(
    p: &mut P,
    text: &str,
    x: f32,
    y: f32,
    scale: f32,
    color: super::Color,
) {
    let mut cx = x;
    for ch in text.chars() {
        let g = glyph(ch);
        for (row, bits) in g.iter().enumerate() {
            for col in 0..5u32 {
                if bits & (1 << (4 - col)) != 0 {
                    p.fill_rect(
                        cx + col as f32 * scale,
                        y + row as f32 * scale,
                        scale,
                        scale,
                        color,
                    );
                }
            }
        }
        cx += GLYPH_W * scale;
    }
}

/// Measure the pixel width of `text` at `scale`.
pub fn text_width(text: &str, scale: f32) -> f32 {
    text.chars().count() as f32 * GLYPH_W * scale
}

/// Fit `text` into `width` pixels: keep `base_scale` when it already fits,
/// otherwise shrink to the largest scale that does, down to `min_scale`, and
/// only then drop the characters that still cannot fit.
///
/// The evidence captions are laid out with this instead of a guessed
/// character budget, so a caption row can never run out of its cell (the
/// measured overflow of the review: 67 characters at 6 px × 1.4 need
/// 562.8 px inside a 512 px cell).
pub fn fit_line(text: &str, width: f32, base_scale: f32, min_scale: f32) -> (String, f32) {
    let n = text.chars().count() as f32;
    if n == 0.0 || width <= 0.0 {
        return (String::new(), base_scale);
    }
    let scale = if text_width(text, base_scale) <= width {
        base_scale
    } else {
        (width / (n * GLYPH_W)).max(min_scale)
    };
    let fits = (width / (GLYPH_W * scale)).floor().max(0.0) as usize;
    if text.chars().count() <= fits {
        return (text.to_string(), scale);
    }
    (text.chars().take(fits).collect(), scale)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_that_fits_keeps_its_scale() {
        let (t, s) = fit_line("hitbox clean r7.50", 512.0, 1.4, 0.9);
        assert_eq!(t, "hitbox clean r7.50");
        assert_eq!(s, 1.4);
        assert!(text_width(&t, s) <= 512.0);
    }

    #[test]
    fn a_long_line_shrinks_and_then_clips_but_never_overflows() {
        // A row of the shape the review measured: at 6 px × 1.4 it needs
        // more than the 512 px cell.
        let long = "first tick after last active  after_step tick 33  sf 1  [after]  x";
        assert!(text_width(long, 1.4) > 512.0);
        let (t, s) = fit_line(long, 512.0 - 16.0, 1.4, 0.9);
        assert!(
            text_width(&t, s) <= 512.0 - 16.0,
            "{} px",
            text_width(&t, s)
        );
        assert!((0.9..=1.4).contains(&s));
        assert_eq!(t, long, "it fits by shrinking, without losing a character");
        // Past the shrink floor the row is clipped, still inside the cell.
        let huge: String = "W".repeat(400);
        let (t, s) = fit_line(&huge, 496.0, 1.4, 0.9);
        assert_eq!(s, 0.9);
        assert!(text_width(&t, s) <= 496.0);
        assert!(t.chars().count() < 400);
    }

    #[test]
    fn empty_and_degenerate_widths_are_safe() {
        assert_eq!(fit_line("", 100.0, 1.4, 0.9).0, "");
        assert_eq!(fit_line("abc", 0.0, 1.4, 0.9).0, "");
    }
}
