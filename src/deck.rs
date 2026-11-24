//! Loading a deck: a folder with an optional `deck.toml` and `.html` slides.

use std::cmp::Ordering;
use std::fmt;
use std::fs;
use std::iter::Peekable;
use std::path::{Path, PathBuf};
use std::str::Chars;

use color_eyre::eyre::{Result, WrapErr, bail, eyre};
use serde::Deserialize;

use crate::anim::Timeline;
use crate::anim::transition::Transition;
use crate::assets::Assets;
use crate::fonts::Fonts;
use crate::html;
use crate::images::Images;
use crate::palette::Ega;
use crate::style::StyledNode;

pub const CONFIG_FILE: &str = "deck.toml";
/// Optional stylesheet applied to every slide, after the defaults.
pub const STYLESHEET_FILE: &str = "style.css";

#[derive(Debug)]
pub struct Deck {
    pub root: PathBuf,
    pub config: DeckConfig,
    /// Fonts (bundled plus the deck's own `.flf` files) and the slides' pictures.
    pub assets: Assets,
    /// Never empty.
    pub slides: Vec<Slide>,
}

#[derive(Debug)]
pub struct Slide {
    pub path: PathBuf,
    /// `<title>`, or the file name without extension.
    pub title: String,
    /// The styled `<html>` element.
    pub root: StyledNode,
    /// Build steps and animations.
    pub timeline: Timeline,
    /// `<meta name="transition">`: how this slide replaces the one before.
    /// `None` uses the deck's.
    pub transition: Option<Transition>,
    /// Problems found in this slide and the deck stylesheet: CSS that
    /// was ignored, fonts that do not exist, pictures that cannot be shown.
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DeckConfig {
    pub title: Option<String>,
    pub size: ScreenSize,
    pub footer: bool,
    /// Default transition for every slide: `"dissolve"`, `"push 0.8s"`, …
    pub transition: Option<Transition>,
}

impl Default for DeckConfig {
    fn default() -> Self {
        Self {
            title: None,
            size: ScreenSize::default(),
            footer: true,
            transition: None,
        }
    }
}

/// The canvas size in character cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(try_from = "String")]
pub struct ScreenSize {
    pub width: u16,
    pub height: u16,
}

impl ScreenSize {
    pub const SUPPORTED: [ScreenSize; 3] = [
        ScreenSize::new(80, 25),
        ScreenSize::new(80, 43),
        ScreenSize::new(80, 50),
    ];

    pub const fn new(width: u16, height: u16) -> Self {
        Self { width, height }
    }
}

impl Default for ScreenSize {
    fn default() -> Self {
        Self::SUPPORTED[0]
    }
}

impl fmt::Display for ScreenSize {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}×{}", self.width, self.height)
    }
}

impl TryFrom<String> for ScreenSize {
    type Error = String;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        let parsed = s.replace('×', "x").split_once('x').and_then(|(w, h)| {
            Some(ScreenSize::new(
                w.trim().parse().ok()?,
                h.trim().parse().ok()?,
            ))
        });
        match parsed {
            Some(size) if Self::SUPPORTED.contains(&size) => Ok(size),
            _ => Err(format!(
                "unsupported size \"{s}\": use \"80x25\", \"80x43\" or \"80x50\""
            )),
        }
    }
}

impl Deck {
    pub fn load(root: &Path) -> Result<Deck> {
        if !root.is_dir() {
            bail!("{} is not a folder", root.display());
        }

        let config_path = root.join(CONFIG_FILE);
        let config = if config_path.is_file() {
            let text = fs::read_to_string(&config_path)
                .wrap_err_with(|| format!("reading {}", config_path.display()))?;
            toml::from_str(&text).wrap_err_with(|| format!("parsing {}", config_path.display()))?
        } else {
            DeckConfig::default()
        };

        let stylesheet_path = root.join(STYLESHEET_FILE);
        let deck_css = if stylesheet_path.is_file() {
            Some(
                fs::read_to_string(&stylesheet_path)
                    .wrap_err_with(|| format!("reading {}", stylesheet_path.display()))?,
            )
        } else {
            None
        };

        let fonts = Fonts::load(root)?;

        let mut paths = Vec::new();
        for entry in fs::read_dir(root).wrap_err_with(|| format!("reading {}", root.display()))? {
            let path = entry?.path();
            if path.is_file() && is_html(&path) {
                paths.push(path);
            }
        }
        if paths.is_empty() {
            return Err(eyre!("no .html slides in {}", root.display()));
        }
        paths.sort_by(|a, b| {
            let (a, b) = (file_name(a), file_name(b));
            natural_cmp(&a, &b).then_with(|| a.cmp(&b))
        });

        let mut slides = paths
            .into_iter()
            .map(|path| {
                let source = fs::read_to_string(&path)
                    .wrap_err_with(|| format!("reading {}", path.display()))?;
                Ok(Slide::parse(path, &source, deck_css.as_deref(), &fonts))
            })
            .collect::<Result<Vec<_>>>()?;

        let mut images = Images::default();
        for slide in &mut slides {
            images.load_for(&slide.root, root, &mut slide.warnings);
        }

        Ok(Deck {
            root: root.to_path_buf(),
            config,
            assets: Assets { fonts, images },
            slides,
        })
    }
}

impl Slide {
    /// Builds a slide from its HTML source. `deck_css` is the deck's
    /// `style.css`; `fonts` are the fonts big text may use.
    pub fn parse(path: PathBuf, source: &str, deck_css: Option<&str>, fonts: &Fonts) -> Slide {
        let mut parsed = html::parse(source, deck_css);
        check_fonts(&parsed.root, fonts, &mut parsed.warnings);
        crate::chart::check(&parsed.root, &mut parsed.warnings);
        let transition =
            parsed
                .transition
                .as_deref()
                .and_then(|text| match Transition::parse(text) {
                    Ok(transition) => Some(transition),
                    Err(problem) => {
                        parsed.warnings.push(problem);
                        None
                    }
                });
        let title = parsed.title.unwrap_or_else(|| {
            path.file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default()
        });
        Slide {
            path,
            title,
            root: parsed.root,
            timeline: parsed.timeline,
            transition,
            warnings: parsed.warnings,
        }
    }

    /// How this slide replaces the one before: its own transition, else
    /// the deck's, else a cut.
    pub fn transition_in(&self, config: &DeckConfig) -> Transition {
        self.transition
            .or(config.transition)
            .unwrap_or(Transition::CUT)
    }

    /// The canvas color: the body's background, else the html element's, else black.
    pub fn background(&self) -> Ega {
        self.root
            .find("body")
            .and_then(|body| body.style.background)
            .or(self.root.style.background)
            .unwrap_or(Ega::Black)
    }

    /// The `--fill` pattern of the body (or the root), with its color.
    pub fn pattern(&self) -> Option<(char, Ega)> {
        [self.root.find("body"), Some(&self.root)]
            .into_iter()
            .flatten()
            .find_map(|node| {
                let pattern = node.style.pattern?;
                Some((
                    pattern.symbol,
                    pattern.color.unwrap_or_else(|| node.style.text_color()),
                ))
            })
    }
}

/// Warns about `--font` names that no font matches. Such text is drawn plainly.
fn check_fonts(node: &StyledNode, fonts: &Fonts, warnings: &mut Vec<String>) {
    if let Some(name) = &node.style.font
        && fonts.get(name).is_none()
    {
        let available = fonts.names().collect::<Vec<_>>().join(", ");
        let warning = format!("unknown font `{name}` drawn as plain text (fonts: {available})");
        if !warnings.contains(&warning) {
            warnings.push(warning);
        }
    }
    for child in &node.children {
        check_fonts(child, fonts, warnings);
    }
}

fn is_html(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("html") || e.eq_ignore_ascii_case("htm"))
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Compares strings so that digit runs sort by value: `2-x` before `10-x`.
fn natural_cmp(a: &str, b: &str) -> Ordering {
    let mut a = a.chars().peekable();
    let mut b = b.chars().peekable();
    loop {
        let ord = match (a.peek().copied(), b.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let (x, y) = (digits(&mut a), digits(&mut b));
                let (x, y) = (x.trim_start_matches('0'), y.trim_start_matches('0'));
                x.len().cmp(&y.len()).then_with(|| x.cmp(y))
            }
            (Some(x), Some(y)) => {
                a.next();
                b.next();
                x.to_lowercase().cmp(y.to_lowercase())
            }
        };
        if ord != Ordering::Equal {
            return ord;
        }
    }
}

fn digits(chars: &mut Peekable<Chars<'_>>) -> String {
    let mut s = String::new();
    while let Some(c) = chars.next_if(char::is_ascii_digit) {
        s.push(c);
    }
    s
}

/// An in-memory deck with one empty slide per title.
#[cfg(test)]
pub(crate) fn test_deck(titles: &[&str]) -> Deck {
    Deck {
        root: PathBuf::from("test-deck"),
        config: DeckConfig::default(),
        assets: Assets::default(),
        slides: titles.iter().map(|title| test_slide(title, "")).collect(),
    }
}

/// An in-memory slide with this title and body HTML.
#[cfg(test)]
pub(crate) fn test_slide(title: &str, body: &str) -> Slide {
    Slide::parse(
        PathBuf::from(format!("{title}.html")),
        &format!("<title>{title}</title>{body}"),
        None,
        &Fonts::bundled(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn deck_dir(files: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for (name, contents) in files {
            fs::write(dir.path().join(name), contents).unwrap();
        }
        dir
    }

    fn titles(deck: &Deck) -> Vec<&str> {
        deck.slides.iter().map(|s| s.title.as_str()).collect()
    }

    #[test]
    fn natural_order() {
        let mut names = vec!["10-end", "2-b", "01-a", "B", "a"];
        names.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(names, ["01-a", "2-b", "10-end", "a", "B"]);
    }

    #[test]
    fn loads_html_slides_in_natural_order() {
        let dir = deck_dir(&[
            ("10-end.html", "<title>End</title>"),
            ("2-middle.HTM", "<p>no title</p>"),
            ("01-start.html", "<title>Start</title><p>Hello</p>"),
            ("notes.txt", "ignored"),
        ]);
        let deck = Deck::load(dir.path()).unwrap();
        assert_eq!(titles(&deck), ["Start", "2-middle", "End"]);
        assert!(deck.slides[0].root.find("p").is_some());
        assert_eq!(deck.config, DeckConfig::default());
    }

    #[test]
    fn applies_deck_stylesheet() {
        let dir = deck_dir(&[
            (
                "style.css",
                "body { background: blue } p { color: yellow; size: 3 }",
            ),
            ("a.html", "<p>x</p>"),
        ]);
        let slide = &Deck::load(dir.path()).unwrap().slides[0];
        assert_eq!(slide.background(), Ega::Blue);
        assert_eq!(slide.root.find("p").unwrap().style.color, Ega::Yellow);
        assert_eq!(slide.warnings, ["unknown CSS property `size` ignored"]);
    }

    #[test]
    fn background_defaults_to_black() {
        let slide = test_slide("t", "<p>x</p>");
        assert_eq!(slide.background(), Ega::Black);
        let slide = test_slide("t", "<body style='background: transparent'>x</body>");
        assert_eq!(slide.background(), Ega::Black);
    }

    #[test]
    fn unknown_fonts_are_reported() {
        let slide = test_slide(
            "t",
            "<p data-font='gothic'>x</p><h2 style='--font: gothic'>y</h2>",
        );
        assert_eq!(
            slide.warnings,
            ["unknown font `gothic` drawn as plain text (fonts: big, block, small)"]
        );
    }

    #[test]
    fn deck_fonts_are_loaded() {
        let mut font = String::from("flf2a$ 1 1 3 -1 0\n");
        for _ in 32..=126 {
            font.push_str("#@@\n");
        }
        let dir = deck_dir(&[("tiny.flf", &font), ("a.html", "<p data-font='tiny'>x</p>")]);
        let deck = Deck::load(dir.path()).unwrap();
        assert!(deck.assets.fonts.get("tiny").is_some());
        assert!(
            deck.slides[0].warnings.is_empty(),
            "{:?}",
            deck.slides[0].warnings
        );
    }

    #[test]
    fn reads_deck_toml() {
        let dir = deck_dir(&[
            (
                "deck.toml",
                "title = \"Talk\"\nsize = \"80x50\"\nfooter = false\ntransition = \"push 1s\"\n",
            ),
            ("a.html", ""),
        ]);
        let config = Deck::load(dir.path()).unwrap().config;
        assert_eq!(config.title.as_deref(), Some("Talk"));
        assert_eq!(config.size, ScreenSize::new(80, 50));
        assert!(!config.footer);
        assert_eq!(
            config.transition.map(|t| t.duration),
            Some(std::time::Duration::from_secs(1))
        );
    }

    #[test]
    fn rejects_bad_config() {
        let unknown = deck_dir(&[("deck.toml", "colour = 1\n"), ("a.html", "")]);
        let err = format!("{:#}", Deck::load(unknown.path()).unwrap_err());
        assert!(err.contains("colour"), "{err}");

        let transition = deck_dir(&[("deck.toml", "transition = \"spin\"\n"), ("a.html", "")]);
        let err = format!("{:#}", Deck::load(transition.path()).unwrap_err());
        assert!(err.contains("unknown transition `spin`"), "{err}");

        let size = deck_dir(&[("deck.toml", "size = \"100x30\"\n"), ("a.html", "")]);
        let err = format!("{:#}", Deck::load(size.path()).unwrap_err());
        assert!(err.contains("unsupported size"), "{err}");
    }

    #[test]
    fn accepts_times_sign_in_size() {
        assert_eq!(
            ScreenSize::try_from("80×43".to_string()),
            Ok(ScreenSize::new(80, 43))
        );
    }

    #[test]
    fn errors_on_missing_folder_or_no_slides() {
        let dir = deck_dir(&[("readme.txt", "")]);
        let err = Deck::load(dir.path()).unwrap_err().to_string();
        assert!(err.contains("no .html slides"), "{err}");

        let err = Deck::load(&dir.path().join("nope"))
            .unwrap_err()
            .to_string();
        assert!(err.contains("is not a folder"), "{err}");
    }
}
