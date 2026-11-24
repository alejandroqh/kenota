//! Rendering a slide onto its canvas.
//!
//! The canvas is a plain `ratatui` [`Buffer`] the size of the deck's screen
//! (80×25 by default), starting at (0, 0). It does not depend on the
//! terminal, so slides can be tested by comparing canvas text. A slide is
//! laid out once ([`compile`]); drawing it is then a function of the
//! [`Clock`]: which build steps show and where the animations are.

use std::borrow::Cow;
use std::collections::HashMap;
use std::time::Duration;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use unicode_width::UnicodeWidthChar;

use crate::anim::{BLINK_HALF_PERIOD, Clock, Effect, TYPEWRITER_CPS, Timeline};
use crate::chrome::{self, FooterInfo};
use crate::deck::Deck;
use crate::layout::{self, DisplayList, Item, Region, Tag};
use crate::palette::Ega;

/// Rows taken by the footer bar when it is enabled.
pub const FOOTER_ROWS: u16 = 2;

/// Blank columns between the end of a marquee's text and its next pass.
const MARQUEE_GAP: i32 = 4;

/// The colors a rainbow cycles through.
const RAINBOW: [Ega; 6] = [
    Ega::LightRed,
    Ega::Yellow,
    Ega::LightGreen,
    Ega::LightCyan,
    Ega::LightBlue,
    Ega::LightMagenta,
];

/// A slide laid out, ready to draw at any moment.
#[derive(Debug)]
pub struct Compiled {
    pub list: DisplayList,
}

/// Lays out slide `index` of `deck`, including the body's fill pattern.
///
/// # Panics
/// If `index` is out of range.
pub fn compile(deck: &Deck, index: usize) -> Compiled {
    let slide = &deck.slides[index];
    let content = content_area(deck);
    let mut list = layout::layout(&slide.root, content, &deck.assets);
    if let Some((symbol, color)) = slide.pattern() {
        // Like the background, the body's fill pattern covers the whole slide.
        let pattern = Item::Pattern {
            x: 0,
            y: 0,
            width: i32::from(content.width),
            height: i32::from(content.height),
            symbol,
            color,
        };
        list.items.insert(0, pattern);
        list.tags.insert(0, Tag::default());
    }
    Compiled { list }
}

/// The slide area: the canvas above the footer.
pub fn content_area(deck: &Deck) -> Rect {
    let size = deck.config.size;
    let footer_rows = if deck.config.footer { FOOTER_ROWS } else { 0 };
    Rect::new(0, 0, size.width, size.height - footer_rows)
}

/// Draws slide `index` of `deck` as it looks at `clock`, footer included.
///
/// # Panics
/// If `index` is out of range.
pub fn draw(deck: &Deck, index: usize, compiled: &Compiled, clock: &Clock) -> Buffer {
    let size = deck.config.size;
    let area = Rect::new(0, 0, size.width, size.height);
    let slide = &deck.slides[index];

    let mut buf = Buffer::empty(area);
    buf.set_style(
        area,
        Style::new()
            .fg(Ega::LightGray.color())
            .bg(slide.background().color()),
    );
    let content = content_area(deck);
    paint_at(&compiled.list, &slide.timeline, clock, &mut buf, content);

    if content.height < area.height {
        chrome::footer(
            &mut buf,
            Rect::new(
                area.x,
                content.bottom(),
                area.width,
                area.height - content.height,
            ),
            &FooterInfo {
                slide_title: &slide.title,
                deck_title: deck.config.title.as_deref(),
                index,
                count: deck.slides.len(),
            },
        );
    }
    buf
}

/// Draws slide `index` of `deck` with every build step showing and the
/// animations at rest, footer included.
///
/// # Panics
/// If `index` is out of range.
pub fn slide_canvas(deck: &Deck, index: usize) -> Buffer {
    let clock = Clock::still(deck.slides[index].timeline.steps);
    draw(deck, index, &compile(deck, index), &clock)
}

/// How long until the slide looks different from how it looks at
/// `clock`; `None` when nothing on it moves.
pub fn next_change(compiled: &Compiled, timeline: &Timeline, clock: &Clock) -> Option<Duration> {
    let list = &compiled.list;
    let mut next = timeline.tick(clock);
    let mut soonest = |wait: Duration| next = Some(next.map_or(wait, |n| n.min(wait)));

    let visible = || {
        list.items
            .iter()
            .zip(&list.tags)
            .filter(|(_, tag)| tag.step <= clock.step)
    };
    let blinking = visible().any(|(item, _)| {
        matches!(item, Item::Text { style, .. } if style.add_modifier.contains(Modifier::SLOW_BLINK))
    });
    if blinking {
        let half = BLINK_HALF_PERIOD.as_millis();
        let into = clock.time.as_millis() % half;
        soonest(Duration::from_millis(
            u64::try_from(half - into).unwrap_or(1),
        ));
    }

    // Typewriters still typing.
    let mut lengths: HashMap<u32, usize> = HashMap::new();
    for (item, tag) in visible() {
        if let Item::Text { text, .. } = item {
            for (id, animation) in timeline.chain(tag.anim) {
                if animation.effect == Effect::Typewriter {
                    *lengths.entry(id).or_default() += text.chars().count();
                }
            }
        }
    }
    for (id, length) in lengths {
        let animation = &timeline.animations[usize::try_from(id).unwrap_or(0)];
        let typed = clock.typed(animation.step);
        if typed < length {
            // When the next character appears.
            #[allow(clippy::cast_precision_loss)]
            let at = (typed + 1) as f64 / TYPEWRITER_CPS;
            let wait = at - clock.step_time.as_secs_f64();
            soonest(Duration::from_secs_f64(wait.max(0.001)));
        }
    }
    next
}

/// Paints display-list items into `buf`, drawing nothing outside `clip`.
/// Every item shows and nothing animates.
pub fn paint(items: &[Item], buf: &mut Buffer, clip: Rect) {
    for item in items {
        paint_item(item, buf, clip, Looks::default());
    }
}

/// Paints a display list as it looks at `clock`: items of later build
/// steps are left out, and animations move, hide or color the rest.
pub fn paint_at(
    list: &DisplayList,
    timeline: &Timeline,
    clock: &Clock,
    buf: &mut Buffer,
    clip: Rect,
) {
    // Characters each typewriter has shown so far, in display order.
    let mut typed: HashMap<u32, usize> = HashMap::new();
    let blink_on = clock.blink_on();
    'items: for (item, tag) in list.items.iter().zip(&list.tags) {
        if tag.step > clock.step {
            continue;
        }
        let mut item = Cow::Borrowed(item);
        let mut clip = clip;
        let mut looks = Looks {
            rainbow: None,
            blink_on,
        };
        // A marquee's shift, and the distance to its next copy.
        let mut scroll = None;
        for (id, animation) in timeline.chain(tag.anim) {
            match animation.effect {
                Effect::Frame { index, count, fps } => {
                    if clock.frame(count, fps) != index {
                        continue 'items;
                    }
                }
                Effect::Typewriter => {
                    if let Item::Text { text, .. } = &*item {
                        let length = text.chars().count();
                        let used = typed.entry(id).or_default();
                        let shown = clock.typed(animation.step).saturating_sub(*used);
                        *used += length;
                        if shown == 0 {
                            continue 'items;
                        }
                        if shown < length {
                            let cut: String = text.chars().take(shown).collect();
                            if let Item::Text { text, .. } = item.to_mut() {
                                *text = cut;
                            }
                        }
                    }
                }
                Effect::Marquee => {
                    if let (Item::Text { .. }, Some(region)) = (&*item, list.regions.get(&id)) {
                        let period = (region.content_width + MARQUEE_GAP)
                            .max(region.width)
                            .max(1);
                        scroll = Some((clock.scrolled().rem_euclid(period), period));
                        clip = clip.intersection(region_rect(region));
                    }
                }
                Effect::Rainbow => looks.rainbow = Some(clock.rainbow_phase()),
            }
        }
        match scroll {
            None => paint_item(&item, buf, clip, looks),
            Some((offset, period)) => {
                for shift in [-offset, period - offset] {
                    let mut copy = item.clone().into_owned();
                    copy.translate(shift, 0);
                    paint_item(&copy, buf, clip, looks);
                }
            }
        }
    }
}

/// The part of a region on the canvas, as a rectangle.
fn region_rect(region: &Region) -> Rect {
    let clamp = |v: i32| u16::try_from(v.max(0)).unwrap_or(u16::MAX);
    let (left, top) = (clamp(region.x), clamp(region.y));
    let (right, bottom) = (
        clamp(region.x + region.width),
        clamp(region.y + region.height),
    );
    Rect::new(
        left,
        top,
        right.saturating_sub(left),
        bottom.saturating_sub(top),
    )
}

/// Per-item adjustments from animations.
#[derive(Clone, Copy)]
struct Looks {
    /// Colors text by column, shifted by this phase.
    rainbow: Option<i32>,
    /// Whether blinking text shows.
    blink_on: bool,
}

impl Default for Looks {
    fn default() -> Self {
        Self {
            rainbow: None,
            blink_on: true,
        }
    }
}

fn paint_item(item: &Item, buf: &mut Buffer, clip: Rect, looks: Looks) {
    let clip = clip.intersection(buf.area);
    let (left, top) = (i32::from(clip.left()), i32::from(clip.top()));
    let (right, bottom) = (i32::from(clip.right()), i32::from(clip.bottom()));
    // Callers only pass coordinates inside `clip`, which is inside the buffer.
    let at = |col: i32, row: i32| {
        (
            u16::try_from(col).unwrap_or_default(),
            u16::try_from(row).unwrap_or_default(),
        )
    };

    match item {
        Item::Fill {
            x,
            y,
            width,
            height,
            color,
        } => {
            for row in (*y).max(top)..(y + height).min(bottom) {
                for col in (*x).max(left)..(x + width).min(right) {
                    buf[at(col, row)].set_symbol(" ").set_bg(color.color());
                }
            }
        }
        Item::Pattern {
            x,
            y,
            width,
            height,
            symbol,
            color,
        } => {
            let symbol = symbol.encode_utf8(&mut [0; 4]).to_string();
            for row in (*y).max(top)..(y + height).min(bottom) {
                for col in (*x).max(left)..(x + width).min(right) {
                    buf[at(col, row)].set_symbol(&symbol).set_fg(color.color());
                }
            }
        }
        Item::Text { x, y, text, style } => {
            if *y < top || *y >= bottom {
                return;
            }
            let blinks = style.add_modifier.contains(Modifier::SLOW_BLINK);
            let style = style.remove_modifier(Modifier::SLOW_BLINK);
            let hidden = blinks && !looks.blink_on;
            if hidden && style.bg.is_none() {
                // Blinked off: whatever is behind shows.
                return;
            }
            let mut col = *x;
            for c in text.chars() {
                let width = c.width().unwrap_or(0);
                if width == 0 {
                    continue;
                }
                let cells = i32::try_from(width).unwrap_or(1);
                if col >= left && col + cells <= right {
                    let style = match looks.rainbow {
                        Some(phase) => {
                            let index = usize::try_from((col - phase).rem_euclid(6)).unwrap_or(0);
                            style.fg(RAINBOW[index].color())
                        }
                        None => style,
                    };
                    let shown = if hidden { ' ' } else { c };
                    let (cx, cy) = at(col, *y);
                    buf.set_stringn(cx, cy, shown.encode_utf8(&mut [0; 4]), width, style);
                }
                col += cells;
            }
        }
    }
}

/// The buffer's symbols, one line per row, trailing spaces removed.
#[cfg(test)]
pub(crate) fn buffer_text(buf: &Buffer) -> String {
    let area = buf.area;
    (area.top()..area.bottom())
        .map(|y| {
            let row: String = (area.left()..area.right())
                .map(|x| buf[(x, y)].symbol())
                .collect();
            row.trim_end().to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use ratatui::style::Color;

    use super::*;
    use crate::deck::{DeckConfig, test_deck, test_slide};

    fn single_slide_canvas(body: &str) -> Buffer {
        let mut deck = test_deck(&[]);
        deck.slides.push(test_slide("Test", body));
        assert!(
            deck.slides[0].warnings.is_empty(),
            "{:?}",
            deck.slides[0].warnings
        );
        slide_canvas(&deck, 0)
    }

    #[test]
    fn middle_slide_with_footer() {
        let mut deck = test_deck(&["Welcome", "Spreadsheet Charting", "The End"]);
        deck.config.title = Some("Works Tutorial".into());
        deck.slides[1] = test_slide(
            "Spreadsheet Charting",
            "<p>Once you've created a chart,<br>you can quickly and easily change its format.</p>
             <p>This paragraph is long enough that it has to wrap onto a second line
             inside the 76-column text area.</p>",
        );
        insta::assert_snapshot!(buffer_text(&slide_canvas(&deck, 1)));
    }

    #[test]
    fn typography_slide() {
        insta::assert_snapshot!(buffer_text(&single_slide_canvas(
            "<style>body { background: blue } .box { background: lightgray; color: black; padding: 1 2; width: 40 }</style>
             <h1>Text styles</h1>
             <h2>Headings, <b>bold</b>, <em>emphasis</em> and <mark>highlights</mark></h2>
             <p>Press <kbd>Ctrl</kbd>+<kbd>C</kbd> to copy. Type <code>dir /w</code> at the prompt.
                <u>Underline</u> and <s>strike</s> use terminal attributes.</p>
             <p style='text-align: center'>Centered text</p>
             <p style='text-align: right'>Right-aligned text</p>
             <hr>
             <div class='box'>A box with a background and padding.</div>"
        )));
    }

    #[test]
    fn lists_slide() {
        insta::assert_snapshot!(buffer_text(&single_slide_canvas(
            "<h2>Lists</h2>
             <ul>
               <li>Bullets hang, so long items wrap neatly under their first word instead of the bullet.</li>
               <li>Nested lists change symbol
                 <ul><li>square<ul><li>circle</li></ul></li></ul>
               </li>
             </ul>
             <ol start='9'><li>Numbers</li><li>stay aligned</li></ol>
             <ol style='list-style-type: upper-roman'><li>Roman</li><li>numerals</li></ol>
             <pre>
  ┌──────────┐
  │ pre text │
  └──────────┘</pre>"
        )));
    }

    const QUARTERS: &str = "<tr><th></th><th>North</th><th>South</th><th>East</th></tr>
        <tr><td>Q1</td><td>12</td><td>8</td><td>5</td></tr>
        <tr><td>Q2</td><td>18</td><td>11</td><td>9</td></tr>
        <tr><td>Q3</td><td>25</td><td>14</td><td>7</td></tr>
        <tr><td>Q4</td><td>31</td><td>12</td><td>15</td></tr>";

    #[test]
    fn charts_slide() {
        insta::assert_snapshot!(buffer_text(&single_slide_canvas(&format!(
            "<style>
               .row {{ display: flex; gap: 2; margin-bottom: 1 }}
               .row table {{ flex: 1; height: 11; margin: 0 }}
               th.light {{ --fill: light }}
             </style>
             <div class='row'>
               <table data-chart='bar'><caption>bar</caption>
                 <tr><th></th><th class='light'>North</th><th>South</th></tr>
                 <tr><td>Q1</td><td>12</td><td>8</td></tr>
                 <tr><td>Q2</td><td>18</td><td>-4</td></tr>
               </table>
               <table data-chart='line'><caption>line</caption>{QUARTERS}</table>
             </div>
             <div class='row'>
               <table data-chart='pie' style='height: 10'><caption>pie</caption>{QUARTERS}</table>
               <table data-chart='hilo' style='height: 10'><caption>hilo</caption>
                 <tr><th></th><th>High</th><th>Low</th><th>Close</th><th>Open</th></tr>
                 <tr><td>Mon</td><td>45</td><td>41</td><td>44</td><td>42</td></tr>
                 <tr><td>Tue</td><td>47</td><td>43</td><td>43.5</td><td>44</td></tr>
                 <tr><td>Wed</td><td>44</td><td>40</td><td>41</td><td>43</td></tr>
               </table>
             </div>"
        ))));
    }

    #[test]
    fn more_charts_slide() {
        insta::assert_snapshot!(buffer_text(&single_slide_canvas(&format!(
            "<style>
               .row {{ display: flex; gap: 2; margin-bottom: 1 }}
               .row table {{ flex: 1; height: 11; margin: 0 }}
             </style>
             <div class='row'>
               <table data-chart='stacked'><caption>stacked</caption>{QUARTERS}</table>
               <table data-chart='stacked100'><caption>stacked100</caption>{QUARTERS}</table>
             </div>
             <div class='row'>
               <table data-chart='area' style='height: 10'><caption>area</caption>{QUARTERS}</table>
               <table data-chart='xy' style='height: 10'><caption>xy</caption>
                 <tr><th>cm</th><th>kg</th></tr>
                 <tr><td>150</td><td>45</td></tr><tr><td>160</td><td>55</td></tr>
                 <tr><td>170</td><td>65</td></tr><tr><td>180</td><td>75</td></tr>
               </table>
             </div>"
        ))));
    }

    /// Slide `body` as it looks at `clock`, without the footer.
    fn at(body: &str, clock: Clock) -> Buffer {
        let mut deck = test_deck(&[]);
        deck.config.footer = false;
        deck.slides.push(test_slide("Test", body));
        assert!(
            deck.slides[0].warnings.is_empty(),
            "{:?}",
            deck.slides[0].warnings
        );
        draw(&deck, 0, &compile(&deck, 0), &clock)
    }

    fn clock(step: u32, ms: u64) -> Clock {
        Clock {
            step,
            time: Duration::from_millis(ms),
            step_time: Duration::from_millis(ms),
            forward: true,
        }
    }

    fn lines(buf: &Buffer) -> Vec<String> {
        buffer_text(buf)
            .lines()
            .map(|line| line.trim().to_string())
            .collect()
    }

    #[test]
    fn steps_hide_what_comes_later() {
        let body = "<style>body { margin: 0 }</style>
            <p style='margin: 0'>a <span data-step>b</span></p>
            <table data-build rules='none' style='margin: 0; border: none'>
              <tr><td>r1</td></tr><tr style='background: red'><td>r2</td></tr></table>";
        let at_step = |step| lines(&at(body, clock(step, 0)))[..3].to_vec();
        assert_eq!(at_step(0), ["a", "", ""]);
        assert_eq!(at_step(2), ["a b", "r1", ""]);
        assert_eq!(at_step(3), ["a b", "r1", "r2"]);
        let buf = at(body, clock(2, 0));
        assert_ne!(
            buf[(1, 2)].bg,
            Ega::Red.color(),
            "row backgrounds wait for their row"
        );
    }

    #[test]
    fn frames_take_turns_in_one_place() {
        let body = "<style>body { margin: 0 }</style>
            <div data-frames data-fps='2'><pre>one</pre><pre>two\nlines</pre></div><p>after</p>";
        assert_eq!(lines(&at(body, clock(0, 0)))[..4], ["one", "", "", "after"]);
        assert_eq!(
            lines(&at(body, clock(0, 600)))[..4],
            ["two", "lines", "", "after"]
        );
        assert_eq!(lines(&at(body, clock(0, 1000)))[0], "one");
    }

    #[test]
    fn marquee_scrolls_inside_its_box() {
        let body = "<style>body { margin: 0 } div { width: 10; border: single }</style>
            <div data-effect='marquee'>Hello there world</div>";
        assert_eq!(lines(&at(body, clock(0, 0)))[1], "│Hello th│");
        assert_eq!(lines(&at(body, clock(0, 500)))[1], "│ there w│");
        // 17 letters and a gap of 4: after 21 cells it starts over.
        assert_eq!(lines(&at(body, clock(0, 2100)))[1], "│Hello th│");
        assert_eq!(lines(&at(body, clock(0, 1900)))[1], "│  Hello │");
    }

    #[test]
    fn rainbow_colors_by_column_and_moves() {
        let body = "<style>body { margin: 0 }</style><p data-effect='rainbow'>abc</p>";
        let buf = at(body, clock(0, 0));
        assert_eq!(buf[(0, 0)].fg, Ega::LightRed.color());
        assert_eq!(buf[(1, 0)].fg, Ega::Yellow.color());
        let later = at(body, clock(0, 125));
        assert_eq!(later[(1, 0)].fg, Ega::LightRed.color());
    }

    #[test]
    fn blinking_text_comes_and_goes() {
        let body = "<style>body { margin: 0 }</style>
            <p style='margin: 0'><span style='text-decoration: blink'>on</span> off</p>
            <p><mark style='text-decoration: blink'>hi</mark></p>";
        let on = at(body, clock(0, 0));
        assert_eq!(lines(&on)[..2], ["on off", "hi"]);
        assert!(!on[(0, 0)].modifier.contains(Modifier::SLOW_BLINK));
        let off = at(body, clock(0, 300));
        assert_eq!(lines(&off)[..2], ["off", ""]);
        assert_eq!(off[(0, 1)].bg, Ega::Cyan.color(), "the highlight stays");
    }

    #[test]
    fn effects_leave_the_box_alone() {
        let body = "<style>body { margin: 0 }</style>
            <div data-effect='typewriter' style='border: single; width: 8'>abcdef</div>";
        assert_eq!(
            lines(&at(body, clock(0, 100)))[..3],
            ["┌──────┐", "│abc   │", "└──────┘"]
        );
    }

    #[test]
    fn nothing_moves_on_a_still_slide() {
        let mut deck = test_deck(&[]);
        deck.slides.push(test_slide("Still", "<p>quiet</p>"));
        deck.slides
            .push(test_slide("Busy", "<p data-effect='rainbow'>x</p>"));
        let wait =
            |i: usize| next_change(&compile(&deck, i), &deck.slides[i].timeline, &clock(0, 0));
        assert_eq!(wait(0), None);
        assert_eq!(wait(1), Some(Duration::from_millis(125)));
    }

    #[test]
    fn boxes_slide() {
        insta::assert_snapshot!(buffer_text(&single_slide_canvas(
            "<style>
               .row { display: flex; gap: 3; margin-bottom: 2 }
               .row div { flex: 1; padding: 0 1; box-shadow: 2 1 }
               .desk { --fill: light; padding: 1 0 }
               .dialog { width: 30; margin: 0 auto; border: double; background: lightgray; text-align: center }
               .tag { position: absolute; right: 2; top: 1; border: thick; z-index: 1 }
             </style>
             <h2>Boxes</h2>
             <div class='row'>
               <div style='border: single'>single</div>
               <div style='border: double'>double, and wrapping text</div>
               <div style='border: thick'>thick</div>
             </div>
             <div class='desk'><div class='dialog'>centered on a fill</div></div>
             <table rules='all'>
               <thead><tr><th>Key</th><th>Action</th></tr></thead>
               <tr><td>PgDn</td><td>next slide</td></tr>
               <tr><td>q</td><td>quit</td></tr>
             </table>
             <div class='tag'>NEW</div>"
        )));
    }

    #[test]
    fn body_background_fills_the_canvas() {
        let canvas = single_slide_canvas("<style>body { background: blue }</style><p>x</p>");
        assert_eq!(canvas[(0, 0)].bg, Ega::Blue.color());
        assert_eq!(canvas[(79, 22)].bg, Ega::Blue.color());
        assert_eq!(canvas[(0, 24)].bg, Ega::Black.color(), "footer stays black");
    }

    #[test]
    fn body_pattern_fills_the_canvas() {
        let canvas = single_slide_canvas(
            "<style>body { background: blue; --fill: light cyan }</style><p>x</p>",
        );
        assert_eq!(canvas[(0, 0)].symbol(), "░");
        assert_eq!(canvas[(0, 0)].fg, Ega::Cyan.color());
        assert_eq!(canvas[(79, 22)].symbol(), "░");
        assert_eq!(canvas[(2, 1)].symbol(), "x");
        assert_ne!(canvas[(0, 24)].symbol(), "░", "footer has no pattern");
    }

    #[test]
    fn content_is_clipped_above_the_footer() {
        let canvas = single_slide_canvas(&"<div>line</div>".repeat(30));
        let text = buffer_text(&canvas);
        let rows: Vec<&str> = text.lines().collect();
        assert_eq!(rows[22], "  line");
        assert!(
            rows[23].contains("Test"),
            "footer title row intact: {}",
            rows[23]
        );
    }

    #[test]
    fn arrows_dim_at_the_ends() {
        let deck = test_deck(&["One", "Two"]);
        let dim = Ega::DarkGray.color();
        let lit = Ega::White.color();

        let first = slide_canvas(&deck, 0);
        let row = buffer_text(&first).lines().last().unwrap().to_string();
        let pgup = u16::try_from(row.chars().position(|c| c == '◄').unwrap()).unwrap();
        let pgdn = u16::try_from(row.chars().position(|c| c == '►').unwrap()).unwrap();
        assert_eq!(first[(pgup, 24)].fg, dim);
        assert_eq!(first[(pgdn, 24)].fg, lit);

        let last = slide_canvas(&deck, 1);
        assert_eq!(last[(pgup, 24)].fg, lit);
        assert_eq!(last[(pgdn, 24)].fg, dim);
    }

    #[test]
    fn no_footer_leaves_bottom_rows_for_content() {
        let mut deck = test_deck(&[]);
        deck.slides
            .push(test_slide("Only", &"<div>line</div>".repeat(30)));
        deck.config = DeckConfig {
            footer: false,
            ..DeckConfig::default()
        };
        let text = buffer_text(&slide_canvas(&deck, 0));
        assert!(!text.contains("PgDn"), "{text}");
        assert_eq!(text.lines().last(), Some("  line"));
    }

    #[test]
    fn paint_clips_and_keeps_background() {
        let area = Rect::new(0, 0, 6, 2);
        let mut buf = Buffer::empty(area);
        let items = [
            Item::Fill {
                x: -2,
                y: 0,
                width: 4,
                height: 5,
                color: Ega::Blue,
            },
            Item::Text {
                x: -1,
                y: 0,
                text: "abcdefgh".into(),
                style: Style::new().fg(Color::Red),
            },
            Item::Text {
                x: 0,
                y: 2,
                text: "below".into(),
                style: Style::new(),
            },
        ];
        paint(&items, &mut buf, Rect::new(0, 0, 5, 2));
        assert_eq!(buffer_text(&buf), "bcdef\n");
        assert_eq!(buf[(0, 0)].bg, Ega::Blue.color(), "text keeps the fill");
        assert_eq!(buf[(1, 1)].bg, Ega::Blue.color());
        assert_eq!(buf[(2, 0)].bg, Color::Reset);
    }
}
