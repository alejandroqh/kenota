//! The fonts a deck can draw big text with: the bundled ones, plus any
//! `.flf` file in the deck folder or its `fonts/` folder, named after the
//! file (`fonts/Slant.flf` is `slant`).

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::sync::{Arc, OnceLock};

use color_eyre::eyre::{Result, WrapErr, eyre};

use crate::figlet::Font;

/// The bundled fonts, drawn for kenota with block characters.
const BUNDLED: [(&str, &str); 3] = [
    ("block", include_str!("fonts/block.flf")),
    ("small", include_str!("fonts/small.flf")),
    ("big", include_str!("fonts/big.flf")),
];

/// Deck fonts may also live in this subfolder.
pub const FONT_DIR: &str = "fonts";

#[derive(Clone, Debug)]
pub struct Fonts {
    by_name: BTreeMap<String, Arc<Font>>,
}

impl Default for Fonts {
    fn default() -> Self {
        Self::bundled()
    }
}

impl Fonts {
    pub fn bundled() -> Self {
        static PARSED: OnceLock<Vec<Arc<Font>>> = OnceLock::new();
        let parsed = PARSED.get_or_init(|| {
            BUNDLED
                .iter()
                .map(|(name, text)| {
                    Arc::new(Font::parse(name, text).expect("bundled fonts are valid"))
                })
                .collect()
        });
        Self {
            by_name: parsed
                .iter()
                .map(|font| (font.name().to_string(), Arc::clone(font)))
                .collect(),
        }
    }

    /// The bundled fonts plus the `.flf` files in `dir` and `dir/fonts`.
    /// A deck font replaces a bundled font of the same name.
    ///
    /// # Errors
    /// If a font file cannot be read or is not a valid FIGlet font.
    pub fn load(dir: &Path) -> Result<Self> {
        let mut fonts = Self::bundled();
        for folder in [dir.to_path_buf(), dir.join(FONT_DIR)] {
            if !folder.is_dir() {
                continue;
            }
            let entries =
                fs::read_dir(&folder).wrap_err_with(|| format!("reading {}", folder.display()))?;
            for entry in entries {
                let path = entry?.path();
                let is_font = path
                    .extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("flf"));
                let Some(stem) = path.file_stem().filter(|_| is_font && path.is_file()) else {
                    continue;
                };
                let name = stem.to_string_lossy().to_lowercase();
                let text = fs::read_to_string(&path)
                    .wrap_err_with(|| format!("reading {}", path.display()))?;
                let font = Font::parse(&name, &text)
                    .map_err(|problem| eyre!("{}: {problem}", path.display()))?;
                fonts.by_name.insert(name, Arc::new(font));
            }
        }
        Ok(fonts)
    }

    /// Looks a font up by name, ignoring case.
    pub fn get(&self, name: &str) -> Option<&Font> {
        self.by_name.get(&name.to_lowercase()).map(Arc::as_ref)
    }

    /// Font names in alphabetical order.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.by_name.keys().map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_fonts_parse_and_cover_ascii() {
        let fonts = Fonts::bundled();
        assert_eq!(fonts.names().collect::<Vec<_>>(), ["big", "block", "small"]);
        for (name, height) in [("block", 4), ("small", 3), ("big", 8)] {
            let font = fonts.get(name).unwrap();
            assert_eq!(font.height(), height, "{name}");
            for c in (32..=126u8).map(char::from).chain("ÄÖÜäöüß".chars()) {
                assert!(font.has(c), "{name} lacks {c:?}");
            }
        }
        assert!(fonts.get("BLOCK").is_some(), "names ignore case");
    }

    #[test]
    fn block_font_letters() {
        let font = Fonts::bundled();
        let art = font
            .get("block")
            .unwrap()
            .render(&"Hi".chars().map(|c| (c, ())).collect::<Vec<_>>());
        let rows: Vec<String> = art
            .rows
            .iter()
            .map(|row| {
                row.iter()
                    .map(|cell| cell.map_or(' ', |(c, ())| c))
                    .collect()
            })
            .collect();
        assert_eq!(rows, ["█   █ ▀", "█▄▄▄█ █", "█   █ █", "▀   ▀ ▀"]);
    }

    #[test]
    fn loads_deck_fonts_from_both_folders() {
        let dir = tempfile::tempdir().unwrap();
        let mut font = String::from("flf2a$ 1 1 3 -1 0\n");
        for _ in 32..=126 {
            font.push_str("#@@\n");
        }
        fs::write(dir.path().join("Tiny.flf"), &font).unwrap();
        fs::create_dir(dir.path().join(FONT_DIR)).unwrap();
        fs::write(dir.path().join(FONT_DIR).join("small.flf"), &font).unwrap();

        let fonts = Fonts::load(dir.path()).unwrap();
        assert_eq!(fonts.get("tiny").unwrap().height(), 1);
        assert_eq!(
            fonts.get("small").unwrap().height(),
            1,
            "replaces the bundled font"
        );

        fs::write(dir.path().join("broken.flf"), "not a font").unwrap();
        let error = Fonts::load(dir.path()).unwrap_err().to_string();
        assert!(error.contains("broken.flf"), "{error}");
    }
}
