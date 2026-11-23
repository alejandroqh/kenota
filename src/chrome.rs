//! Everything around the slide content: the footer bar, the border around
//! the canvas, and the "terminal too small" screen.

use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph, Widget, Wrap};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::palette::Ega;

/// What the footer bar shows.
pub struct FooterInfo<'a> {
    pub slide_title: &'a str,
    pub deck_title: Option<&'a str>,
    /// Zero-based index of the current slide.
    pub index: usize,
    pub count: usize,
}

/// The footer panel is at least this wide, like the MS Works tutorials.
const MIN_PANEL_WIDTH: u16 = 36;

/// Draws the 2-row footer bar into `area`:
///
/// ```text
///                                           ─────────── Slide title ───────────
///  Deck title                                    ◄◄◄ PgUp • 2/4 • PgDn ►►►
/// ```
pub fn footer(buf: &mut Buffer, area: Rect, info: &FooterInfo) {
    if area.height < 2 || area.width == 0 {
        return;
    }
    buf.set_style(area, Style::new().bg(Ega::Black.color()));

    let plain = Style::new().fg(Ega::LightGray.color());
    let arrows = |enabled: bool| {
        let color = if enabled { Ega::White } else { Ega::DarkGray };
        Style::new().fg(color.color())
    };
    let controls = Line::from(vec![
        Span::styled("◄◄◄ PgUp", arrows(info.index > 0)),
        Span::styled(" • ", plain),
        Span::styled(format!("{}/{}", info.index + 1, info.count), plain),
        Span::styled(" • ", plain),
        Span::styled("PgDn ►►►", arrows(info.index + 1 < info.count)),
    ]);
    let controls_width = width(&controls.to_string());
    let panel_width = MIN_PANEL_WIDTH
        .max(controls_width)
        .max(width(info.slide_title) + 6)
        .min(area.width);
    let panel_x = area.right() - panel_width;

    let title_y = area.y;
    buf.set_string(
        panel_x,
        title_y,
        "─".repeat(usize::from(panel_width)),
        plain,
    );
    let title = truncate(info.slide_title, usize::from(panel_width.saturating_sub(2)));
    buf.set_string(
        panel_x + (panel_width - width(&title)) / 2,
        title_y,
        &title,
        Style::new().fg(Ega::Cyan.color()),
    );

    let controls_y = area.y + 1;
    buf.set_line(
        panel_x + panel_width.saturating_sub(controls_width) / 2,
        controls_y,
        &controls,
        panel_width,
    );

    if let Some(deck_title) = info.deck_title {
        // One column of margin on the left, one column of gap before the panel.
        let room = panel_x.saturating_sub(area.x + 2);
        buf.set_string(
            area.x + 1,
            controls_y,
            truncate(deck_title, usize::from(room)),
            Style::new().fg(Ega::DarkGray.color()),
        );
    }
}

/// Copies `canvas` into the middle of `area`, with a border when there is
/// room. Shows a "terminal too small" message when the canvas does not fit.
pub fn present(buf: &mut Buffer, area: Rect, canvas: &Buffer) {
    let (width, height) = (canvas.area.width, canvas.area.height);
    let Some(target) = centered(area, width, height) else {
        too_small(buf, area, width, height);
        return;
    };

    if area.width >= width + 2 && area.height >= height + 2 {
        let border = Rect::new(target.x - 1, target.y - 1, width + 2, height + 2);
        Block::bordered()
            .border_style(Style::new().fg(Ega::DarkGray.color()))
            .render(border, buf);
    }

    for y in 0..height {
        for x in 0..width {
            buf[(target.x + x, target.y + y)] =
                canvas[(canvas.area.x + x, canvas.area.y + y)].clone();
        }
    }
}

/// A `width`×`height` rectangle centered in `area`, or `None` if it does not fit.
pub fn centered(area: Rect, width: u16, height: u16) -> Option<Rect> {
    (area.width >= width && area.height >= height).then(|| {
        Rect::new(
            area.x + (area.width - width) / 2,
            area.y + (area.height - height) / 2,
            width,
            height,
        )
    })
}

fn too_small(buf: &mut Buffer, area: Rect, width: u16, height: u16) {
    let lines = vec![
        Line::styled(
            "Terminal too small",
            Style::new().add_modifier(Modifier::BOLD),
        ),
        Line::raw(""),
        Line::raw(format!(
            "This deck needs {width}×{height}; the terminal is {}×{}.",
            area.width, area.height
        )),
        Line::raw("Resize the window, or press q to quit."),
    ];
    let text_height = u16::try_from(lines.len())
        .unwrap_or(u16::MAX)
        .min(area.height);
    let top = (area.height - text_height) / 2;
    Paragraph::new(lines)
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true })
        .render(
            Rect::new(area.x, area.y + top, area.width, area.height - top),
            buf,
        );
}

fn width(s: &str) -> u16 {
    u16::try_from(s.width()).unwrap_or(u16::MAX)
}

/// Shortens `s` to at most `max` columns, ending with `…` when cut.
fn truncate(s: &str, max: usize) -> String {
    if s.width() <= max {
        return s.to_string();
    }
    if max == 0 {
        return String::new();
    }
    let mut out = String::new();
    let mut used = 0;
    for c in s.chars() {
        let w = c.width().unwrap_or(0);
        if used + w + 1 > max {
            break;
        }
        out.push(c);
        used += w;
    }
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::buffer_text;

    fn canvas_with_marker() -> Buffer {
        let mut canvas = Buffer::empty(Rect::new(0, 0, 80, 25));
        canvas.set_string(0, 0, "X", Style::new());
        canvas.set_string(79, 24, "Y", Style::new());
        canvas
    }

    #[test]
    fn centered_fits_or_refuses() {
        let size = |w, h| Rect::new(0, 0, w, h);
        assert_eq!(centered(size(80, 25), 80, 25), Some(size(80, 25)));
        assert_eq!(
            centered(size(101, 30), 80, 25),
            Some(Rect::new(10, 2, 80, 25))
        );
        assert_eq!(centered(size(79, 25), 80, 25), None);
        assert_eq!(centered(size(80, 24), 80, 25), None);
    }

    #[test]
    fn present_centers_canvas_inside_a_border() {
        let area = Rect::new(0, 0, 84, 29);
        let mut buf = Buffer::empty(area);
        present(&mut buf, area, &canvas_with_marker());
        assert_eq!(buf[(1, 1)].symbol(), "┌");
        assert_eq!(buf[(82, 27)].symbol(), "┘");
        assert_eq!(buf[(2, 2)].symbol(), "X");
        assert_eq!(buf[(81, 26)].symbol(), "Y");
    }

    #[test]
    fn present_skips_border_on_exact_fit() {
        let area = Rect::new(0, 0, 80, 25);
        let mut buf = Buffer::empty(area);
        present(&mut buf, area, &canvas_with_marker());
        assert_eq!(buf[(0, 0)].symbol(), "X");
        assert_eq!(buf[(79, 24)].symbol(), "Y");
    }

    #[test]
    fn present_explains_when_terminal_is_too_small() {
        let area = Rect::new(0, 0, 60, 20);
        let mut buf = Buffer::empty(area);
        present(&mut buf, area, &canvas_with_marker());
        let text = buffer_text(&buf);
        assert!(text.contains("Terminal too small"), "{text}");
        assert!(
            text.contains("needs 80×25; the terminal is 60×20"),
            "{text}"
        );
        assert!(!text.contains('X'), "{text}");
    }

    #[test]
    fn truncate_adds_ellipsis() {
        assert_eq!(truncate("Hello world", 20), "Hello world");
        assert_eq!(truncate("Hello world", 8), "Hello w…");
        assert_eq!(truncate("Hello", 0), "");
    }
}
