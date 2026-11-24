//! Finding problems in a deck without showing it: `kenota check`.
//!
//! A problem is either a warning collected while reading a slide (CSS that
//! was ignored, an unknown font, a missing picture, an unsupported tag, a
//! bad transition) or content that falls outside the slide area, where it
//! would be cut off.

use std::fmt;

use ratatui::layout::Rect;
use unicode_width::UnicodeWidthStr;

use crate::anim::Timeline;
use crate::deck::Deck;
use crate::layout::{DisplayList, Item};
use crate::render;

/// Something wrong with one slide.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Problem {
    /// The slide's file name.
    pub file: String,
    pub message: String,
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.file, self.message)
    }
}

/// Every problem in `deck`, slide by slide.
pub fn check(deck: &Deck) -> Vec<Problem> {
    let area = render::content_area(deck);
    let mut problems = Vec::new();
    for (index, slide) in deck.slides.iter().enumerate() {
        let file = slide.path.file_name().map_or_else(
            || slide.path.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        );
        let mut report = |message: String| {
            problems.push(Problem {
                file: file.clone(),
                message,
            });
        };
        for warning in &slide.warnings {
            report(warning.clone());
        }
        let compiled = render::compile(deck, index);
        for message in outside(&compiled.list, &slide.timeline, area) {
            report(message);
        }
    }
    problems
}

/// What reaches past the edges of `area`, with every build step showing.
/// A marquee's text is longer than its box on purpose, so it doesn't count.
fn outside(list: &DisplayList, timeline: &Timeline, area: Rect) -> Vec<String> {
    let (width, height) = (i32::from(area.width), i32::from(area.height));
    let scrolls = |anim: Option<u32>| {
        timeline
            .chain(anim)
            .any(|(id, _)| list.regions.contains_key(&id))
    };
    let mut edges = Edges::default();
    for (item, tag) in list.items.iter().zip(&list.tags) {
        if scrolls(tag.anim) {
            continue;
        }
        let (x, y, w, h, text) = match item {
            Item::Fill {
                x,
                y,
                width,
                height,
                ..
            }
            | Item::Pattern {
                x,
                y,
                width,
                height,
                ..
            } => (*x, *y, *width, *height, None),
            Item::Text { x, y, text, .. } => {
                let w = i32::try_from(text.width()).unwrap_or(i32::MAX);
                (*x, *y, w, 1, Some(text.trim()))
            }
        };
        if w <= 0 || h <= 0 {
            continue;
        }
        edges.see(Side::Bottom, y + h - 1, height - 1, text, |v, limit| {
            v > limit
        });
        edges.see(Side::Right, x + w - 1, width - 1, text, |v, limit| {
            v > limit
        });
        edges.see(Side::Top, y, 0, text, |v, limit| v < limit);
        edges.see(Side::Left, x, 0, text, |v, limit| v < limit);
    }
    edges.messages(width, height)
}

#[derive(Clone, Copy)]
enum Side {
    Bottom,
    Right,
    Top,
    Left,
}

/// The farthest point past each edge, and the first text found there.
#[derive(Default)]
struct Edges {
    found: [Option<(i32, Option<String>)>; 4],
}

impl Edges {
    fn see(
        &mut self,
        side: Side,
        value: i32,
        limit: i32,
        text: Option<&str>,
        past: impl Fn(i32, i32) -> bool,
    ) {
        if !past(value, limit) {
            return;
        }
        let slot = &mut self.found[side as usize];
        let text = text.filter(|t| !t.is_empty()).map(quote);
        match slot {
            None => *slot = Some((value, text)),
            Some((farthest, quoted)) => {
                if past(value, *farthest) {
                    *farthest = value;
                }
                if quoted.is_none() {
                    *quoted = text;
                }
            }
        }
    }

    fn messages(&self, width: i32, height: i32) -> Vec<String> {
        let mut messages = Vec::new();
        for (index, found) in self.found.iter().enumerate() {
            let Some((value, text)) = found else {
                continue;
            };
            let what = match text {
                Some(text) => format!("content ({text})"),
                None => "content".to_string(),
            };
            messages.push(match index {
                0 => format!(
                    "{what} reaches row {value}, past the bottom of the slide area (row {})",
                    height - 1
                ),
                1 => format!(
                    "{what} reaches column {value}, past the right edge (column {})",
                    width - 1
                ),
                2 => format!("{what} starts at row {value}, above the top of the screen"),
                _ => format!("{what} starts at column {value}, left of the screen"),
            });
        }
        messages
    }
}

/// The start of `text`, quoted, to point at where the problem is.
fn quote(text: &str) -> String {
    const MAX: usize = 30;
    let mut chars = text.chars();
    let start: String = chars.by_ref().take(MAX).collect();
    if chars.next().is_some() {
        format!("“{}…”", start.trim_end())
    } else {
        format!("“{start}”")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deck::{test_deck, test_slide};

    fn problems(body: &str) -> Vec<String> {
        let mut deck = test_deck(&[]);
        deck.slides.push(test_slide("S", body));
        check(&deck).into_iter().map(|p| p.message).collect()
    }

    #[test]
    fn a_clean_slide_has_no_problems() {
        assert_eq!(problems("<p>Hello</p>"), Vec::<String>::new());
    }

    #[test]
    fn warnings_are_problems() {
        let found = problems("<p style='colour: red'>x</p>");
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].contains("colour"), "{found:?}");
    }

    #[test]
    fn content_below_the_slide_area_is_reported_with_its_text() {
        let found = problems("<p style='margin-top: 21'>first line<br>second line</p>");
        assert_eq!(
            found,
            ["content (“second line”) reaches row 23, past the bottom of the slide area (row 22)"]
        );
    }

    #[test]
    fn boxes_and_shadows_count() {
        let found = problems(
            "<div style='position: absolute; left: 70; top: 2; width: 9; height: 2;
                         background: blue; box-shadow: 2 1'></div>",
        );
        assert_eq!(
            found,
            ["content reaches column 80, past the right edge (column 79)"]
        );
        let found = problems("<div style='position: absolute; left: -3; width: 5'>x</div>");
        assert_eq!(
            found,
            ["content (“x”) starts at column -3, left of the screen"]
        );
    }

    #[test]
    fn marquees_and_hidden_steps_are_judged_fairly() {
        // A marquee's line is longer than its box on purpose.
        let long = "word ".repeat(40);
        assert_eq!(
            problems(&format!("<div data-effect='marquee'>{long}</div>")),
            Vec::<String>::new()
        );
        // Content of later build steps counts: the slide must fit when complete.
        let found = problems("<p style='margin-top: 21' data-step>a<br>b</p>");
        assert_eq!(found.len(), 1, "{found:?}");
    }
}
