//! Slide HTML parsing.
//!
//! For now this only pulls out the slide title and a plain-text version of
//! the body. Milestone 2 replaces the plain text with a styled tree.

use scraper::{ElementRef, Html, Selector};

/// The parts of a slide the shell needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlideText {
    /// Contents of `<title>`, whitespace collapsed. `None` when missing or blank.
    pub title: Option<String>,
    /// The body as plain text: one line per `\n`, blank lines between paragraphs.
    pub body: String,
}

pub fn extract(source: &str) -> SlideText {
    let doc = Html::parse_document(source);

    let title = doc
        .select(&selector("title"))
        .next()
        .map(|t| collapse_whitespace(&t.text().collect::<String>()))
        .filter(|t| !t.is_empty());

    let mut text = TextBuilder::default();
    if let Some(body) = doc.select(&selector("body")).next() {
        walk(body, &mut text, false);
    }

    SlideText {
        title,
        body: text.finish(),
    }
}

fn selector(s: &str) -> Selector {
    Selector::parse(s).expect("static selector is valid")
}

fn collapse_whitespace(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// How an element affects plain-text output.
enum Kind {
    /// Not rendered at all.
    Skip,
    /// `<br>`: a hard line break.
    Break,
    /// Starts and ends on its own line(s); the number is how many line breaks.
    Block(usize),
    /// Table cell: separated from its neighbours by a space.
    Cell,
    /// Flows with the surrounding text.
    Inline,
}

fn kind(tag: &str) -> Kind {
    match tag {
        "script" | "style" | "head" | "title" | "template" | "noscript" => Kind::Skip,
        "br" => Kind::Break,
        "p" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "ul" | "ol" | "pre" | "table"
        | "blockquote" | "figure" | "hr" => Kind::Block(2),
        "div" | "li" | "tr" | "section" | "article" | "header" | "footer" | "main" | "nav"
        | "aside" | "dl" | "dt" | "dd" | "center" | "address" | "caption" => Kind::Block(1),
        "td" | "th" => Kind::Cell,
        _ => Kind::Inline,
    }
}

fn walk(element: ElementRef<'_>, out: &mut TextBuilder, in_pre: bool) {
    for child in element.children() {
        if let Some(text) = child.value().as_text() {
            if in_pre {
                out.raw(text);
            } else {
                out.text(text);
            }
        } else if let Some(child) = ElementRef::wrap(child) {
            let tag = child.value().name();
            match kind(tag) {
                Kind::Skip => {}
                Kind::Break => out.hard_break(),
                Kind::Block(breaks) => {
                    out.block(breaks);
                    if tag == "li" {
                        out.text("• ");
                    }
                    walk(child, out, in_pre || tag == "pre");
                    out.block(breaks);
                }
                Kind::Cell => {
                    walk(child, out, in_pre);
                    out.space();
                }
                Kind::Inline => walk(child, out, in_pre),
            }
        }
    }
}

/// Accumulates text, collapsing whitespace the way a browser does.
#[derive(Default)]
struct TextBuilder {
    out: String,
    /// Line breaks owed before the next text.
    breaks: usize,
    /// A space is owed before the next text.
    space: bool,
}

impl TextBuilder {
    fn block(&mut self, breaks: usize) {
        self.breaks = self.breaks.max(breaks);
        self.space = false;
    }

    fn hard_break(&mut self) {
        self.breaks += 1;
        self.space = false;
    }

    fn space(&mut self) {
        self.space = true;
    }

    /// Normal text: runs of whitespace become one space.
    fn text(&mut self, s: &str) {
        if s.starts_with(char::is_whitespace) {
            self.space = true;
        }
        for (i, word) in s.split_whitespace().enumerate() {
            if i == 0 {
                self.flush();
            } else {
                self.out.push(' ');
            }
            self.out.push_str(word);
        }
        if s.ends_with(char::is_whitespace) {
            self.space = true;
        }
    }

    /// Preformatted text: kept exactly.
    fn raw(&mut self, s: &str) {
        if !s.is_empty() {
            self.flush();
            self.out.push_str(s);
        }
    }

    /// Emits owed breaks or space before new text.
    fn flush(&mut self) {
        if !self.out.is_empty() {
            if self.breaks > 0 {
                let already = self.out.len() - self.out.trim_end_matches('\n').len();
                for _ in already..self.breaks {
                    self.out.push('\n');
                }
            } else if self.space && !self.out.ends_with([' ', '\n']) {
                self.out.push(' ');
            }
        }
        self.breaks = 0;
        self.space = false;
    }

    fn finish(self) -> String {
        self.out.trim_end().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body(html: &str) -> String {
        extract(html).body
    }

    #[test]
    fn title_is_collapsed_and_optional() {
        let with = extract("<title>  Spreadsheet\n  Charting </title><p>x</p>");
        assert_eq!(with.title.as_deref(), Some("Spreadsheet Charting"));
        assert_eq!(extract("<p>x</p>").title, None);
        assert_eq!(extract("<title>  </title>").title, None);
    }

    #[test]
    fn paragraphs_are_separated_by_a_blank_line() {
        assert_eq!(
            body("<h1>Title</h1>\n<p>One\n  two</p><p>Three</p>"),
            "Title\n\nOne two\n\nThree"
        );
    }

    #[test]
    fn br_and_inline_tags() {
        assert_eq!(
            body("<p>Once you've <b>created</b> a chart,<br>you can change it.</p>"),
            "Once you've created a chart,\nyou can change it."
        );
    }

    #[test]
    fn list_items_get_bullets() {
        assert_eq!(
            body("<ul>\n  <li>First</li>\n  <li>Second <i>point</i></li>\n</ul>"),
            "• First\n• Second point"
        );
    }

    #[test]
    fn pre_keeps_whitespace() {
        assert_eq!(
            body("<p>Art:</p><pre>  /\\\n /  \\</pre><p>End</p>"),
            "Art:\n\n  /\\\n /  \\\n\nEnd"
        );
    }

    #[test]
    fn scripts_and_styles_are_ignored() {
        assert_eq!(
            body("<style>p { color: red }</style><script>alert(1)</script><p>Shown</p>"),
            "Shown"
        );
    }

    #[test]
    fn table_cells_are_spaced() {
        assert_eq!(
            body("<table><tr><td>a</td><td>b</td></tr><tr><td>c</td></tr></table>"),
            "a b\nc"
        );
    }
}
