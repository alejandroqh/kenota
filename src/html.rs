//! Slide HTML parsing: the title, and the styled element tree.

use scraper::{Html, Selector};

use crate::style::{self, StyledNode};

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedSlide {
    /// Contents of `<title>`, whitespace collapsed. `None` when missing or blank.
    pub title: Option<String>,
    /// The `<html>` element, styled.
    pub root: StyledNode,
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

    let mut warnings = Vec::new();
    let root = style::build(&doc, deck_css, &mut warnings);
    ParsedSlide {
        title,
        root,
        warnings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_is_collapsed_and_optional() {
        let with = parse("<title>  Spreadsheet\n  Charting </title><p>x</p>", None);
        assert_eq!(with.title.as_deref(), Some("Spreadsheet Charting"));
        assert_eq!(parse("<p>x</p>", None).title, None);
        assert_eq!(parse("<title>  </title>", None).title, None);
    }

    #[test]
    fn root_is_html_with_body() {
        let slide = parse("<p>x</p>", None);
        assert_eq!(slide.root.tag(), Some("html"));
        assert!(slide.root.find("body").and_then(|b| b.find("p")).is_some());
        assert!(slide.warnings.is_empty());
    }
}
