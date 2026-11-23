//! Loading a deck: a folder with an optional `deck.toml` and `.html` slides.

use std::cmp::Ordering;
use std::fmt;
use std::fs;
use std::iter::Peekable;
use std::path::{Path, PathBuf};
use std::str::Chars;

use color_eyre::eyre::{Result, WrapErr, bail, eyre};
use serde::Deserialize;

use crate::html;

pub const CONFIG_FILE: &str = "deck.toml";

#[derive(Debug)]
pub struct Deck {
    pub root: PathBuf,
    pub config: DeckConfig,
    /// Never empty.
    pub slides: Vec<Slide>,
}

#[derive(Debug)]
pub struct Slide {
    pub path: PathBuf,
    /// `<title>`, or the file name without extension.
    pub title: String,
    /// Plain-text body (temporary until the layout engine lands).
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DeckConfig {
    pub title: Option<String>,
    pub size: ScreenSize,
    pub footer: bool,
    /// Default transition for every slide. Not used until milestone 6.
    pub transition: Option<String>,
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

        let slides = paths
            .into_iter()
            .map(Slide::load)
            .collect::<Result<Vec<_>>>()?;

        Ok(Deck {
            root: root.to_path_buf(),
            config,
            slides,
        })
    }
}

impl Slide {
    fn load(path: PathBuf) -> Result<Slide> {
        let source =
            fs::read_to_string(&path).wrap_err_with(|| format!("reading {}", path.display()))?;
        let extracted = html::extract(&source);
        let title = extracted.title.unwrap_or_else(|| {
            path.file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default()
        });
        Ok(Slide {
            path,
            title,
            text: extracted.body,
        })
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
        slides: titles
            .iter()
            .map(|title| Slide {
                path: PathBuf::from(format!("{title}.html")),
                title: (*title).to_string(),
                text: String::new(),
            })
            .collect(),
    }
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
        assert_eq!(deck.slides[0].text, "Hello");
        assert_eq!(deck.config, DeckConfig::default());
    }

    #[test]
    fn reads_deck_toml() {
        let dir = deck_dir(&[
            (
                "deck.toml",
                "title = \"Talk\"\nsize = \"80x50\"\nfooter = false\n",
            ),
            ("a.html", ""),
        ]);
        let config = Deck::load(dir.path()).unwrap().config;
        assert_eq!(config.title.as_deref(), Some("Talk"));
        assert_eq!(config.size, ScreenSize::new(80, 50));
        assert!(!config.footer);
    }

    #[test]
    fn rejects_bad_config() {
        let unknown = deck_dir(&[("deck.toml", "colour = 1\n"), ("a.html", "")]);
        let err = format!("{:#}", Deck::load(unknown.path()).unwrap_err());
        assert!(err.contains("colour"), "{err}");

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
