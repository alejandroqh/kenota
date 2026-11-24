//! Slide HTML parsing: the title, and the styled element tree.

use scraper::{ElementRef, Html, Selector};

use crate::anim::{self, Timeline};
use crate::style::{self, StyledNode};

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedSlide {
    /// Contents of `<title>`, whitespace collapsed. `None` when missing or blank.
    pub title: Option<String>,
    /// The `<html>` element, styled, with build steps and animations set.
    pub root: StyledNode,
    /// The slide's build steps and animations.
    pub timeline: Timeline,
    /// `<meta name="transition" content="…">`, unparsed.
    pub transition: Option<String>,
    /// CSS problems found while styling: unknown properties, bad values, …
    pub warnings: Vec<String>,
}

/// Parses a slide. `deck_css` is the deck's `style.css`, if any.
pub fn parse(source: &str, deck_css: Option<&str>) -> ParsedSlide {
    let doc = Html::parse_document(source);

    let title_selector = Selector::parse("title").expect("static selector is valid");
    let title = doc
        .select(&title_selector)
        .next()
        .map(|t| {
            t.text()
                .collect::<String>()
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
        })
        .filter(|t| !t.is_empty());

    let meta_selector =
        Selector::parse("meta[name=transition i]").expect("static selector is valid");
    let transition = doc
        .select(&meta_selector)
        .next()
        .and_then(|meta| meta.value().attr("content"))
        .map(|content| content.trim().to_string());

    let mut warnings = Vec::new();
    unsupported_tags(&doc, &mut warnings);
    let mut root = style::build(&doc, deck_css, &mut warnings);
    let timeline = anim::prepare(&mut root, &mut warnings);
    ParsedSlide {
        title,
        root,
        timeline,
        transition,
        warnings,
    }
}

/// Warns once per kind of element the engine can't show: scripts, media,
/// vector graphics and form controls.
fn unsupported_tags(doc: &Html, warnings: &mut Vec<String>) {
    let mut seen: Vec<&str> = Vec::new();
    for element in doc
        .root_element()
        .descendants()
        .filter_map(ElementRef::wrap)
    {
        let name = element.value().name();
        let hint = match name {
            "script" => "slides have no JavaScript",
            "svg" | "canvas" => "draw with <img> (PNG or GIF) or block characters instead",
            "video" | "audio" | "iframe" | "object" | "embed" => "only its fallback text shows",
            "input" | "button" | "select" | "textarea" => {
                "slides are not interactive; show a control with <kbd> or a styled box"
            }
            _ => continue,
        };
        if !seen.contains(&name) {
            seen.push(name);
            warnings.push(format!("<{name}> is not supported: {hint}"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_tags_warn_once_each() {
        let slide = parse(
            "<p>x</p><svg></svg><svg></svg><script>go()</script><button>OK</button>",
            None,
        );
        assert_eq!(
            slide.warnings,
            [
                "<svg> is not supported: draw with <img> (PNG or GIF) or block characters instead",
                "<script> is not supported: slides have no JavaScript",
                "<button> is not supported: slides are not interactive; show a control with <kbd> or a styled box",
            ]
        );
    }

    #[test]
    fn title_is_collapsed_and_optional() {
        let with = parse("<title>  Spreadsheet\n  Charting </title><p>x</p>", None);
        assert_eq!(with.title.as_deref(), Some("Spreadsheet Charting"));
        assert_eq!(parse("<p>x</p>", None).title, None);
        assert_eq!(parse("<title>  </title>", None).title, None);
    }

    #[test]
    fn transition_comes_from_meta() {
        let slide = parse(
            "<head><meta name='Transition' content=' wipe-left 1s '></head><p>x</p>",
            None,
        );
        assert_eq!(slide.transition.as_deref(), Some("wipe-left 1s"));
        assert_eq!(parse("<p>x</p>", None).transition, None);
    }

    #[test]
    fn root_is_html_with_body() {
        let slide = parse("<p>x</p>", None);
        assert_eq!(slide.root.tag(), Some("html"));
        assert!(slide.root.find("body").and_then(|b| b.find("p")).is_some());
        assert!(slide.warnings.is_empty());
    }
}
