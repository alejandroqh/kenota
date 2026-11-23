//! The 16-color EGA palette used by every slide.

use ratatui::buffer::Buffer;
use ratatui::style::Color;

/// One of the 16 EGA colors, in their classic order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ega {
    Black,
    Blue,
    Green,
    Cyan,
    Red,
    Magenta,
    Brown,
    LightGray,
    DarkGray,
    LightBlue,
    LightGreen,
    LightCyan,
    LightRed,
    LightMagenta,
    Yellow,
    White,
}

impl Ega {
    pub const ALL: [Ega; 16] = [
        Ega::Black,
        Ega::Blue,
        Ega::Green,
        Ega::Cyan,
        Ega::Red,
        Ega::Magenta,
        Ega::Brown,
        Ega::LightGray,
        Ega::DarkGray,
        Ega::LightBlue,
        Ega::LightGreen,
        Ega::LightCyan,
        Ega::LightRed,
        Ega::LightMagenta,
        Ega::Yellow,
        Ega::White,
    ];

    /// The exact RGB value the EGA hardware produced.
    pub const fn rgb(self) -> (u8, u8, u8) {
        match self {
            Ega::Black => (0x00, 0x00, 0x00),
            Ega::Blue => (0x00, 0x00, 0xAA),
            Ega::Green => (0x00, 0xAA, 0x00),
            Ega::Cyan => (0x00, 0xAA, 0xAA),
            Ega::Red => (0xAA, 0x00, 0x00),
            Ega::Magenta => (0xAA, 0x00, 0xAA),
            Ega::Brown => (0xAA, 0x55, 0x00),
            Ega::LightGray => (0xAA, 0xAA, 0xAA),
            Ega::DarkGray => (0x55, 0x55, 0x55),
            Ega::LightBlue => (0x55, 0x55, 0xFF),
            Ega::LightGreen => (0x55, 0xFF, 0x55),
            Ega::LightCyan => (0x55, 0xFF, 0xFF),
            Ega::LightRed => (0xFF, 0x55, 0x55),
            Ega::LightMagenta => (0xFF, 0x55, 0xFF),
            Ega::Yellow => (0xFF, 0xFF, 0x55),
            Ega::White => (0xFF, 0xFF, 0xFF),
        }
    }

    /// The high-intensity version of a dark color; bright colors stay as they are.
    pub const fn bright(self) -> Ega {
        match self {
            Ega::Black => Ega::DarkGray,
            Ega::Blue => Ega::LightBlue,
            Ega::Green => Ega::LightGreen,
            Ega::Cyan => Ega::LightCyan,
            Ega::Red => Ega::LightRed,
            Ega::Magenta => Ega::LightMagenta,
            Ega::Brown => Ega::Yellow,
            Ega::LightGray => Ega::White,
            bright => bright,
        }
    }

    /// The color as a truecolor terminal color.
    pub const fn color(self) -> Color {
        let (r, g, b) = self.rgb();
        Color::Rgb(r, g, b)
    }

    /// The matching ANSI-16 color, for terminals without truecolor.
    pub const fn ansi(self) -> Color {
        match self {
            Ega::Black => Color::Black,
            Ega::Blue => Color::Blue,
            Ega::Green => Color::Green,
            Ega::Cyan => Color::Cyan,
            Ega::Red => Color::Red,
            Ega::Magenta => Color::Magenta,
            Ega::Brown => Color::Yellow,
            Ega::LightGray => Color::Gray,
            Ega::DarkGray => Color::DarkGray,
            Ega::LightBlue => Color::LightBlue,
            Ega::LightGreen => Color::LightGreen,
            Ega::LightCyan => Color::LightCyan,
            Ega::LightRed => Color::LightRed,
            Ega::LightMagenta => Color::LightMagenta,
            Ega::Yellow => Color::LightYellow,
            Ega::White => Color::White,
        }
    }

    /// The palette entry closest to an arbitrary RGB color.
    pub fn nearest(r: u8, g: u8, b: u8) -> Ega {
        let distance = |e: &Ega| {
            let (er, eg, eb) = e.rgb();
            let d = |x: u8, y: u8| (i32::from(x) - i32::from(y)).pow(2);
            d(r, er) + d(g, eg) + d(b, eb)
        };
        Ega::ALL
            .into_iter()
            .min_by_key(distance)
            .expect("palette is not empty")
    }
}

/// Rewrites every truecolor cell in `buf` to its ANSI-16 equivalent.
pub fn downgrade_to_ansi16(buf: &mut Buffer) {
    let to_ansi = |color: Color| match color {
        Color::Rgb(r, g, b) => Ega::nearest(r, g, b).ansi(),
        other => other,
    };
    for cell in &mut buf.content {
        cell.fg = to_ansi(cell.fg);
        cell.bg = to_ansi(cell.bg);
    }
}

#[cfg(test)]
mod tests {
    use ratatui::layout::Rect;
    use ratatui::style::Style;

    use super::*;

    #[test]
    fn nearest_finds_exact_palette_entries() {
        for ega in Ega::ALL {
            let (r, g, b) = ega.rgb();
            assert_eq!(Ega::nearest(r, g, b), ega);
        }
    }

    #[test]
    fn nearest_snaps_off_palette_colors() {
        assert_eq!(Ega::nearest(0xB0, 0x60, 0x10), Ega::Brown);
        assert_eq!(Ega::nearest(0x10, 0x10, 0xC0), Ega::Blue);
    }

    #[test]
    fn downgrade_maps_rgb_and_keeps_other_colors() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 2, 1));
        buf.set_style(
            Rect::new(0, 0, 1, 1),
            Style::new().fg(Ega::Brown.color()).bg(Ega::Blue.color()),
        );
        downgrade_to_ansi16(&mut buf);
        assert_eq!(buf[(0, 0)].fg, Color::Yellow);
        assert_eq!(buf[(0, 0)].bg, Color::Blue);
        assert_eq!(buf[(1, 0)].fg, Color::Reset);
    }
}
