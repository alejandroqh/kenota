//! Styles: the CSS subset dos-kenota understands, resolved for every element.
//!
//! Plain numbers are character cells: `margin: 1 2` is one row above and
//! below, two columns left and right.

mod cascade;
mod color;
mod parse;
mod values;

pub use cascade::{Content, StyledNode, build};
pub use color::parse_color;

use crate::palette::Ega;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Display {
    Inline,
    Block,
    ListItem,
    None,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextAlign {
    Left,
    Center,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WhiteSpace {
    Normal,
    NoWrap,
    Pre,
    PreWrap,
}

impl WhiteSpace {
    /// Runs of spaces and newlines become a single space.
    pub fn collapses(self) -> bool {
        matches!(self, WhiteSpace::Normal | WhiteSpace::NoWrap)
    }

    /// Long lines wrap at the container's width.
    pub fn wraps(self) -> bool {
        matches!(self, WhiteSpace::Normal | WhiteSpace::PreWrap)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextTransform {
    None,
    Uppercase,
    Lowercase,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ListStyleType {
    None,
    Disc,
    Circle,
    Square,
    Decimal,
    LowerAlpha,
    UpperAlpha,
    LowerRoman,
    UpperRoman,
    /// A quoted string used as the marker, e.g. `"► "`.
    Text(String),
}

/// Top/right/bottom/left sizes in cells.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Edges {
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
    pub left: i32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Length {
    Cells(i32),
    Percent(f32),
}

impl Length {
    /// The length in cells, with percentages taken of `base`.
    #[allow(clippy::cast_possible_truncation)]
    pub fn resolve(self, base: i32) -> i32 {
        match self {
            Length::Cells(n) => n,
            Length::Percent(p) => (base as f32 * p / 100.0).round() as i32,
        }
    }
}

/// The resolved style of one element.
#[derive(Clone, Debug, PartialEq)]
pub struct ComputedStyle {
    // Inherited by children.
    pub color: Ega,
    /// Bold text is drawn in the bright version of its color, like DOS.
    pub bold: bool,
    pub underline: bool,
    pub line_through: bool,
    pub text_align: TextAlign,
    pub white_space: WhiteSpace,
    pub text_transform: TextTransform,
    pub list_style_type: ListStyleType,

    // Not inherited.
    pub display: Display,
    /// `None` is transparent.
    pub background: Option<Ega>,
    pub margin: Edges,
    pub padding: Edges,
    /// `None` is `auto`: as wide as the container allows.
    pub width: Option<Length>,
}

/// Why a declaration was ignored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rejected {
    UnknownProperty,
    InvalidValue,
}

impl ComputedStyle {
    /// The style of the root element before any rule applies.
    pub fn root() -> Self {
        Self {
            color: Ega::LightGray,
            bold: false,
            underline: false,
            line_through: false,
            text_align: TextAlign::Left,
            white_space: WhiteSpace::Normal,
            text_transform: TextTransform::None,
            list_style_type: ListStyleType::Disc,
            display: Display::Inline,
            background: None,
            margin: Edges::default(),
            padding: Edges::default(),
            width: None,
        }
    }

    /// The starting style of a child: inherited properties copied, the rest reset.
    pub fn inherit(&self) -> Self {
        Self {
            display: Display::Inline,
            background: None,
            margin: Edges::default(),
            padding: Edges::default(),
            width: None,
            ..self.clone()
        }
    }

    /// The foreground color as drawn: bright when bold.
    pub fn text_color(&self) -> Ega {
        if self.bold {
            self.color.bright()
        } else {
            self.color
        }
    }

    /// Applies one declaration. `name` must be lowercase.
    pub fn apply(&mut self, name: &str, value: &str) -> Result<(), Rejected> {
        use values::{cells, edges, keyword, length};
        let invalid = Rejected::InvalidValue;

        match name {
            "color" => self.color = parse_color(value).ok_or(invalid)?,
            "background" | "background-color" => {
                self.background = values::background(value).ok_or(invalid)?;
            }
            "font-weight" => self.bold = values::font_weight(value).ok_or(invalid)?,
            "text-decoration" | "text-decoration-line" => {
                (self.underline, self.line_through) =
                    values::text_decoration(value).ok_or(invalid)?;
            }
            "display" => {
                self.display = keyword(
                    value,
                    &[
                        ("inline", Display::Inline),
                        ("block", Display::Block),
                        ("list-item", Display::ListItem),
                        ("none", Display::None),
                    ],
                )
                .ok_or(invalid)?;
            }
            "margin" => self.margin = edges(value).ok_or(invalid)?,
            "margin-top" => self.margin.top = cells(value).ok_or(invalid)?,
            "margin-right" => self.margin.right = cells(value).ok_or(invalid)?,
            "margin-bottom" => self.margin.bottom = cells(value).ok_or(invalid)?,
            "margin-left" => self.margin.left = cells(value).ok_or(invalid)?,
            "padding" => self.padding = edges(value).ok_or(invalid)?,
            "padding-top" => self.padding.top = cells(value).ok_or(invalid)?,
            "padding-right" => self.padding.right = cells(value).ok_or(invalid)?,
            "padding-bottom" => self.padding.bottom = cells(value).ok_or(invalid)?,
            "padding-left" => self.padding.left = cells(value).ok_or(invalid)?,
            "width" => self.width = length(value).ok_or(invalid)?,
            "text-align" => {
                self.text_align = keyword(
                    value,
                    &[
                        ("left", TextAlign::Left),
                        ("start", TextAlign::Left),
                        ("center", TextAlign::Center),
                        ("right", TextAlign::Right),
                        ("end", TextAlign::Right),
                    ],
                )
                .ok_or(invalid)?;
            }
            "white-space" => {
                self.white_space = keyword(
                    value,
                    &[
                        ("normal", WhiteSpace::Normal),
                        ("nowrap", WhiteSpace::NoWrap),
                        ("pre", WhiteSpace::Pre),
                        ("pre-wrap", WhiteSpace::PreWrap),
                    ],
                )
                .ok_or(invalid)?;
            }
            "text-transform" => {
                self.text_transform = keyword(
                    value,
                    &[
                        ("none", TextTransform::None),
                        ("uppercase", TextTransform::Uppercase),
                        ("lowercase", TextTransform::Lowercase),
                    ],
                )
                .ok_or(invalid)?;
            }
            "list-style-type" => {
                self.list_style_type = values::list_style_type(value).ok_or(invalid)?
            }
            "list-style" => {
                // Only the marker type matters in text mode; `inside`/`outside` are ignored.
                self.list_style_type = parse::split_top_level(value, ' ')
                    .into_iter()
                    .find_map(values::list_style_type)
                    .ok_or(invalid)?;
            }
            // Custom properties are reserved for dos-kenota extensions and deck variables.
            custom if custom.starts_with("--") => {}
            _ => return Err(Rejected::UnknownProperty),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inherit_keeps_text_properties_only() {
        let mut parent = ComputedStyle::root();
        parent.apply("color", "yellow").unwrap();
        parent.apply("background", "blue").unwrap();
        parent.apply("margin", "1").unwrap();
        parent.apply("text-align", "center").unwrap();

        let child = parent.inherit();
        assert_eq!(child.color, Ega::Yellow);
        assert_eq!(child.text_align, TextAlign::Center);
        assert_eq!(child.background, None);
        assert_eq!(child.margin, Edges::default());
    }

    #[test]
    fn bold_brightens_color() {
        let mut style = ComputedStyle::root();
        style.apply("color", "blue").unwrap();
        assert_eq!(style.text_color(), Ega::Blue);
        style.apply("font-weight", "bold").unwrap();
        assert_eq!(style.text_color(), Ega::LightBlue);
    }

    #[test]
    fn rejects_unknown_and_invalid() {
        let mut style = ComputedStyle::root();
        assert_eq!(
            style.apply("font-size", "20px"),
            Err(Rejected::UnknownProperty)
        );
        assert_eq!(
            style.apply("color", "chartreuse-ish"),
            Err(Rejected::InvalidValue)
        );
        assert_eq!(style.apply("display", "grid"), Err(Rejected::InvalidValue));
        assert_eq!(style.apply("--fill", "\"▒\""), Ok(()));
    }

    #[test]
    fn list_style_shorthand_picks_the_type() {
        let mut style = ComputedStyle::root();
        style.apply("list-style", "inside upper-roman").unwrap();
        assert_eq!(style.list_style_type, ListStyleType::UpperRoman);
        style.apply("list-style", "'► '").unwrap();
        assert_eq!(style.list_style_type, ListStyleType::Text("► ".into()));
    }

    #[test]
    fn percent_width_resolves_against_base() {
        assert_eq!(Length::Percent(50.0).resolve(76), 38);
        assert_eq!(Length::Cells(10).resolve(76), 10);
    }
}
