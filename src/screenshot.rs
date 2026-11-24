//! Turning a canvas into a picture: `kenota render --png`.
//!
//! Every cell becomes 8×16 pixels drawn with the Spleen 8×16 bitmap font
//! (`fonts/spleen-8x16.bdf`, BSD-2-Clause, see `fonts/LICENSE-spleen`), in
//! the cell's exact EGA colors. Box drawing, blocks and ░▒▓ come from the
//! font too, so lines join and fills tile like on a real text screen. An
//! 80×25 slide is 640×400 pixels, the size of a VGA text screen. The PNG
//! keeps the 16 colors as a palette, 4 bits a pixel, so files stay small.

use std::collections::HashMap;
use std::io::Write;
use std::sync::OnceLock;

use ratatui::buffer::Buffer;
use ratatui::style::{Color, Modifier};

use crate::palette::Ega;

/// Pixels per cell.
pub const CELL_WIDTH: u32 = 8;
pub const CELL_HEIGHT: u32 = 16;

const SPLEEN: &str = include_str!("fonts/spleen-8x16.bdf");

/// One glyph: a byte per row, the leftmost pixel in the highest bit.
type Glyph = [u8; 16];

/// Glyphs Spleen doesn't have, drawn for kenota in the same style.
const EXTRA: [(char, Glyph); 3] = [
    // ► and ◄: the footer's arrows and DOS list markers.
    (
        '►',
        [
            0, 0, 0, 0x40, 0x60, 0x70, 0x78, 0x7C, 0x7E, 0x7C, 0x78, 0x70, 0x60, 0x40, 0, 0,
        ],
    ),
    (
        '◄',
        [
            0, 0, 0, 0x02, 0x06, 0x0E, 0x1E, 0x3E, 0x7E, 0x3E, 0x1E, 0x0E, 0x06, 0x02, 0, 0,
        ],
    ),
    // ▪: a small square.
    (
        '▪',
        [0, 0, 0, 0, 0, 0, 0x3C, 0x3C, 0x3C, 0x3C, 0, 0, 0, 0, 0, 0],
    ),
];

fn font() -> &'static HashMap<char, Glyph> {
    static FONT: OnceLock<HashMap<char, Glyph>> = OnceLock::new();
    FONT.get_or_init(|| {
        let mut glyphs = parse_bdf(SPLEEN);
        glyphs.extend(EXTRA);
        glyphs
    })
}

/// Reads the glyphs of an 8×16 BDF font: `ENCODING` gives the character,
/// the 16 lines after `BITMAP` its rows in hex.
fn parse_bdf(source: &str) -> HashMap<char, Glyph> {
    let mut glyphs = HashMap::new();
    let mut lines = source.lines();
    let mut current: Option<char> = None;
    while let Some(line) = lines.next() {
        if let Some(code) = line.strip_prefix("ENCODING ") {
            current = code.trim().parse().ok().and_then(char::from_u32);
        } else if line == "BITMAP" {
            let mut glyph = [0; 16];
            for row in &mut glyph {
                *row = lines
                    .next()
                    .and_then(|hex| u8::from_str_radix(hex.trim(), 16).ok())
                    .unwrap_or(0);
            }
            if let Some(c) = current.take() {
                glyphs.insert(c, glyph);
            }
        }
    }
    glyphs
}

/// The glyph for a cell's symbol; `?` when the font lacks it.
fn glyph(symbol: &str) -> Glyph {
    let font = font();
    let c = symbol.chars().next().unwrap_or(' ');
    font.get(&c)
        .or_else(|| font.get(&'?'))
        .copied()
        .unwrap_or([0; 16])
}

/// A picture in the 16 EGA colors: one palette index per pixel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Picture {
    pub width: u32,
    pub height: u32,
    /// Row by row, each pixel an index into [`Ega::ALL`].
    pub pixels: Vec<u8>,
}

impl Picture {
    fn new(width: u32, height: u32) -> Self {
        let size = usize::try_from(width * height).unwrap_or(0);
        Self {
            width,
            height,
            pixels: vec![0; size],
        }
    }

    /// The color of the pixel at (`x`, `y`).
    pub fn get(&self, x: u32, y: u32) -> Ega {
        let index = usize::try_from(y * self.width + x).unwrap_or(0);
        Ega::ALL[usize::from(self.pixels[index])]
    }

    fn set(&mut self, x: u32, y: u32, color: u8) {
        if let Ok(index) = usize::try_from(y * self.width + x) {
            self.pixels[index] = color;
        }
    }

    /// Writes the picture as a 4-bit palette PNG.
    ///
    /// # Errors
    /// When writing fails.
    pub fn write_png(&self, out: impl Write) -> Result<(), png::EncodingError> {
        let mut encoder = png::Encoder::new(out, self.width, self.height);
        encoder.set_color(png::ColorType::Indexed);
        encoder.set_depth(png::BitDepth::Four);
        encoder.set_palette(
            Ega::ALL
                .iter()
                .flat_map(|ega| {
                    let (r, g, b) = ega.rgb();
                    [r, g, b]
                })
                .collect::<Vec<u8>>(),
        );
        let mut writer = encoder.write_header()?;
        // Two pixels a byte, the left one in the high half; rows are whole bytes.
        let row_bytes = usize::try_from(self.width.div_ceil(2)).unwrap_or(0);
        let mut packed = Vec::with_capacity(row_bytes * self.height as usize);
        for row in self.pixels.chunks(self.width.max(1) as usize) {
            for pair in row.chunks(2) {
                packed.push(pair[0] << 4 | pair.get(1).copied().unwrap_or(0));
            }
        }
        writer.write_image_data(&packed)?;
        writer.finish()
    }
}

/// `buf` as a picture, each pixel enlarged to `scale`×`scale`.
pub fn screenshot(buf: &Buffer, scale: u32) -> Picture {
    let scale = scale.max(1);
    let area = buf.area;
    let mut image = Picture::new(
        u32::from(area.width) * CELL_WIDTH * scale,
        u32::from(area.height) * CELL_HEIGHT * scale,
    );
    for row in 0..area.height {
        for column in 0..area.width {
            let cell = &buf[(area.x + column, area.y + row)];
            let mut fg = index(cell.fg, Ega::LightGray);
            let mut bg = index(cell.bg, Ega::Black);
            if cell.modifier.contains(Modifier::REVERSED) {
                std::mem::swap(&mut fg, &mut bg);
            }
            let mut glyph = glyph(cell.symbol());
            if cell.modifier.contains(Modifier::UNDERLINED) {
                glyph[14] = 0xFF;
            }
            if cell.modifier.contains(Modifier::CROSSED_OUT) {
                glyph[8] = 0xFF;
            }
            let left = u32::from(column) * CELL_WIDTH;
            let top = u32::from(row) * CELL_HEIGHT;
            for (y, bits) in (0..).zip(glyph) {
                for x in 0..CELL_WIDTH {
                    let on = bits & (0x80 >> x) != 0;
                    let color = if on { fg } else { bg };
                    for dy in 0..scale {
                        for dx in 0..scale {
                            image.set((left + x) * scale + dx, (top + y) * scale + dy, color);
                        }
                    }
                }
            }
        }
    }
    image
}

/// A cell color as an index into [`Ega::ALL`]; `Reset` is `default`.
fn index(color: Color, default: Ega) -> u8 {
    let ega = match color {
        Color::Rgb(r, g, b) => Ega::nearest(r, g, b),
        Color::Black => Ega::Black,
        Color::Blue => Ega::Blue,
        Color::Green => Ega::Green,
        Color::Cyan => Ega::Cyan,
        Color::Red => Ega::Red,
        Color::Magenta => Ega::Magenta,
        Color::Yellow => Ega::Brown,
        Color::Gray => Ega::LightGray,
        Color::DarkGray => Ega::DarkGray,
        Color::LightBlue => Ega::LightBlue,
        Color::LightGreen => Ega::LightGreen,
        Color::LightCyan => Ega::LightCyan,
        Color::LightRed => Ega::LightRed,
        Color::LightMagenta => Ega::LightMagenta,
        Color::LightYellow => Ega::Yellow,
        Color::White => Ega::White,
        Color::Reset | Color::Indexed(_) => default,
    };
    let position = Ega::ALL.iter().position(|e| *e == ega).unwrap_or(0);
    u8::try_from(position).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use ratatui::layout::Rect;
    use ratatui::style::Style;

    use super::*;

    #[test]
    fn the_font_covers_what_kenota_draws() {
        let font = font();
        for c in "AZaz09 •○■►◄♦░▒▓▀▄█▌─│┌┐└┘├┤┬┴┼═║╔╗╚╝╠╣╦╩╬╒╓╕╖╘╙╛╜".chars()
        {
            assert!(font.contains_key(&c), "missing {c:?}");
        }
        // Blocks fill exactly their half of the cell.
        assert_eq!(glyph("█"), [0xFF; 16]);
        assert_eq!(&glyph("▀")[..8], &[0xFF; 8]);
        assert_eq!(&glyph("▀")[8..], &[0; 8]);
        // A character the font lacks shows as `?`.
        assert_eq!(glyph("日"), glyph("?"));
    }

    #[test]
    fn cells_become_pixels_in_their_colors() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 2, 1));
        let style = Style::new().fg(Ega::Yellow.color()).bg(Ega::Blue.color());
        buf.set_string(0, 0, "▄ ", style);
        let image = screenshot(&buf, 1);
        assert_eq!((image.width, image.height), (16, 16));
        assert_eq!(image.get(3, 2), Ega::Blue, "top half of ▄ is background");
        assert_eq!(image.get(3, 12), Ega::Yellow, "bottom half is the color");
        assert_eq!(image.get(12, 12), Ega::Blue, "a space is all background");
    }

    #[test]
    fn scale_enlarges_every_pixel() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 1, 1));
        buf.set_string(0, 0, "█", Style::new().fg(Ega::White.color()));
        let image = screenshot(&buf, 3);
        assert_eq!((image.width, image.height), (24, 48));
        assert_eq!(image.get(23, 47), Ega::White);
    }

    #[test]
    fn png_keeps_the_palette_and_pixels() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 2, 1));
        buf.set_string(0, 0, "█", Style::new().fg(Ega::LightMagenta.color()));
        let mut bytes = Vec::new();
        screenshot(&buf, 1).write_png(&mut bytes).unwrap();
        let decoded = image::load_from_memory(&bytes).unwrap().to_rgb8();
        assert_eq!(decoded.dimensions(), (16, 16));
        assert_eq!(decoded.get_pixel(0, 0).0, [0xFF, 0x55, 0xFF]);
        assert_eq!(decoded.get_pixel(15, 15).0, [0, 0, 0]);
    }
}
