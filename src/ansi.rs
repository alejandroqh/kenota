//! Turning a canvas into text for a terminal or a file: `dos-kenota render`.

use std::fmt::Write;

use ratatui::buffer::Buffer;
use ratatui::style::{Color, Modifier};
use unicode_width::UnicodeWidthStr;

use crate::palette::Ega;

/// How colors are written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorMode {
    /// Exact EGA RGB values (`ESC[38;2;r;g;bm`).
    Truecolor,
    /// The terminal's own 16 colors (`ESC[31m` …).
    Ansi16,
    /// Plain text, no escape codes; trailing spaces are trimmed.
    None,
}

/// `buf` as lines of text, one per row, colored as `mode` says. Every
/// colored line ends with a reset, so the output can be printed anywhere.
pub fn to_ansi(buf: &Buffer, mode: ColorMode) -> String {
    let area = buf.area;
    let mut out = String::new();
    for y in area.top()..area.bottom() {
        let mut line = String::new();
        let mut current: Option<String> = None;
        // Columns still covered by the last wide character.
        let mut covered = 0;
        for x in area.left()..area.right() {
            if covered > 0 {
                covered -= 1;
                continue;
            }
            let cell = &buf[(x, y)];
            covered = cell.symbol().width().saturating_sub(1);
            if mode != ColorMode::None {
                let sgr = sgr(cell.fg, cell.bg, cell.modifier, mode);
                if current.as_ref() != Some(&sgr) {
                    line.push_str(&sgr);
                    current = Some(sgr);
                }
            }
            line.push_str(cell.symbol());
        }
        if mode == ColorMode::None {
            out.push_str(line.trim_end());
        } else {
            out.push_str(&line);
            out.push_str("\x1b[0m");
        }
        out.push('\n');
    }
    out
}

/// The escape sequence that sets exactly these colors and attributes.
fn sgr(fg: Color, bg: Color, modifier: Modifier, mode: ColorMode) -> String {
    let mut codes = String::from("0");
    for (flag, code) in [
        (Modifier::BOLD, "1"),
        (Modifier::DIM, "2"),
        (Modifier::UNDERLINED, "4"),
        (Modifier::REVERSED, "7"),
        (Modifier::CROSSED_OUT, "9"),
    ] {
        if modifier.contains(flag) {
            let _ = write!(codes, ";{code}");
        }
    }
    color(&mut codes, fg, mode, false);
    color(&mut codes, bg, mode, true);
    format!("\x1b[{codes}m")
}

fn color(codes: &mut String, color: Color, mode: ColorMode, background: bool) {
    let color = match (mode, color) {
        (ColorMode::Ansi16, Color::Rgb(r, g, b)) => Ega::nearest(r, g, b).ansi(),
        _ => color,
    };
    let offset = if background { 10 } else { 0 };
    let mut named = |base: u8| {
        let _ = write!(codes, ";{}", base + offset);
    };
    match color {
        Color::Reset => {}
        Color::Rgb(r, g, b) => {
            let _ = write!(codes, ";{};2;{r};{g};{b}", 38 + offset);
        }
        Color::Indexed(i) => {
            let _ = write!(codes, ";{};5;{i}", 38 + offset);
        }
        Color::Black => named(30),
        Color::Red => named(31),
        Color::Green => named(32),
        Color::Yellow => named(33),
        Color::Blue => named(34),
        Color::Magenta => named(35),
        Color::Cyan => named(36),
        Color::Gray => named(37),
        Color::DarkGray => named(90),
        Color::LightRed => named(91),
        Color::LightGreen => named(92),
        Color::LightYellow => named(93),
        Color::LightBlue => named(94),
        Color::LightMagenta => named(95),
        Color::LightCyan => named(96),
        Color::White => named(97),
    }
}

#[cfg(test)]
mod tests {
    use ratatui::layout::Rect;
    use ratatui::style::Style;

    use super::*;

    fn canvas() -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, 4, 2));
        let blue = Style::new().fg(Ega::White.color()).bg(Ega::Blue.color());
        buf.set_string(0, 0, "Hi", blue);
        buf.set_string(0, 1, "x", Style::new().fg(Ega::Yellow.color()));
        buf
    }

    #[test]
    fn plain_text_trims_lines() {
        assert_eq!(to_ansi(&canvas(), ColorMode::None), "Hi\nx\n");
    }

    #[test]
    fn truecolor_writes_rgb_only_when_it_changes() {
        let text = to_ansi(&canvas(), ColorMode::Truecolor);
        let first = text.lines().next().unwrap();
        assert_eq!(
            first,
            "\x1b[0;38;2;255;255;255;48;2;0;0;170mHi\x1b[0m  \x1b[0m"
        );
        assert!(
            text.lines()
                .nth(1)
                .unwrap()
                .starts_with("\x1b[0;38;2;255;255;85mx")
        );
    }

    #[test]
    fn ansi16_uses_the_terminal_palette() {
        let text = to_ansi(&canvas(), ColorMode::Ansi16);
        // White on blue: bright white (97) on blue background (44).
        assert!(text.starts_with("\x1b[0;97;44mHi"), "{text:?}");
        assert!(text.lines().nth(1).unwrap().starts_with("\x1b[0;93mx"));
    }

    #[test]
    fn wide_characters_take_two_columns() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 4, 1));
        buf.set_string(0, 0, "日x", Style::new());
        assert_eq!(to_ansi(&buf, ColorMode::None), "日x\n");
    }

    #[test]
    fn attributes_are_kept() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 1, 1));
        buf.set_string(0, 0, "u", Style::new().add_modifier(Modifier::UNDERLINED));
        assert!(to_ansi(&buf, ColorMode::Truecolor).starts_with("\x1b[0;4mu"));
    }
}
