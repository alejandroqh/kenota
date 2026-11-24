//! Styles: the CSS subset dos-kenota understands, resolved for every element.
//!
//! Plain numbers are character cells: `margin: 1 2` is one row above and
//! below, two columns left and right.

mod boxes;
mod cascade;
mod color;
mod parse;
mod values;

pub use boxes::{
    Align, AutoMargin, Border, BorderSide, BorderStyle, Inset, Justify, Pattern, Position, Shadow,
};
pub use cascade::{Content, StyledNode, build};
pub use color::parse_color;

use crate::palette::Ega;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Display {
    Inline,
    Block,
    ListItem,
    /// Children side by side in one row.
    Flex,
    /// Rows and cells drawn with a box-drawing grid.
    Table,
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

impl From<[i32; 4]> for Edges {
    /// Top, right, bottom, left.
    fn from([top, right, bottom, left]: [i32; 4]) -> Self {
        Self {
            top,
            right,
            bottom,
            left,
        }
    }
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
    /// `--font`: the FIGlet font for big text, by name. `None` is ordinary text.
    pub font: Option<String>,

    // Not inherited.
    pub display: Display,
    /// `None` is transparent.
    pub background: Option<Ega>,
    pub margin: Edges,
    pub auto_margin: AutoMargin,
    pub padding: Edges,
    pub border: Border,
    /// Width of the border box (padding and border included). `None` is
    /// `auto`: as wide as the container allows.
    pub width: Option<Length>,
    /// Height of the border box in cells. `None` is `auto`: as tall as the content.
    pub height: Option<i32>,
    pub shadow: Option<Shadow>,
    /// `--fill`: a character pattern drawn over the background.
    pub pattern: Option<Pattern>,
    pub position: Position,
    pub inset: Inset,
    pub z_index: i32,
    /// `flex-grow`: this item's share of a flex row's spare width.
    pub flex_grow: f32,
    /// `flex-basis`: the width an item starts from before growing. `None`
    /// is `auto`: its `width`, else its content's width.
    pub flex_basis: Option<Length>,
    /// Columns between flex items.
    pub gap: i32,
    pub justify_content: Justify,
    pub align_items: Align,
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
            font: None,
            display: Display::Inline,
            background: None,
            margin: Edges::default(),
            auto_margin: AutoMargin::default(),
            padding: Edges::default(),
            border: Border::default(),
            width: None,
            height: None,
            shadow: None,
            pattern: None,
            position: Position::Static,
            inset: Inset::default(),
            z_index: 0,
            flex_grow: 0.0,
            flex_basis: None,
            gap: 0,
            justify_content: Justify::Start,
            align_items: Align::Stretch,
        }
    }

    /// The starting style of a child: inherited properties copied, the rest reset.
    pub fn inherit(&self) -> Self {
        Self {
            color: self.color,
            bold: self.bold,
            underline: self.underline,
            line_through: self.line_through,
            text_align: self.text_align,
            white_space: self.white_space,
            text_transform: self.text_transform,
            list_style_type: self.list_style_type.clone(),
            font: self.font.clone(),
            ..Self::root()
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
        use values::{cells, edges, four, keyword, length, margin};
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
                        ("flex", Display::Flex),
                        ("table", Display::Table),
                        ("none", Display::None),
                    ],
                )
                .ok_or(invalid)?;
            }
            "margin" => (self.margin, self.auto_margin) = values::margins(value).ok_or(invalid)?,
            "margin-top" => self.margin.top = margin(value).ok_or(invalid)?.unwrap_or(0),
            "margin-bottom" => self.margin.bottom = margin(value).ok_or(invalid)?.unwrap_or(0),
            "margin-right" => {
                let m = margin(value).ok_or(invalid)?;
                (self.margin.right, self.auto_margin.right) = (m.unwrap_or(0), m.is_none());
            }
            "margin-left" => {
                let m = margin(value).ok_or(invalid)?;
                (self.margin.left, self.auto_margin.left) = (m.unwrap_or(0), m.is_none());
            }
            "padding" => self.padding = edges(value).ok_or(invalid)?,
            "padding-top" => self.padding.top = cells(value).ok_or(invalid)?,
            "padding-right" => self.padding.right = cells(value).ok_or(invalid)?,
            "padding-bottom" => self.padding.bottom = cells(value).ok_or(invalid)?,
            "padding-left" => self.padding.left = cells(value).ok_or(invalid)?,
            "width" => self.width = length(value).ok_or(invalid)?,
            "height" => self.height = values::height(value).ok_or(invalid)?,
            "box-sizing" => {
                // Widths and heights always include padding and border.
                keyword(value, &[("border-box", ())]).ok_or(invalid)?;
            }
            "border" => self.border = Border::all(values::border(value).ok_or(invalid)?),
            "border-collapse" => {
                // Table grids always share lines between cells.
                keyword(value, &[("collapse", ())]).ok_or(invalid)?;
            }
            border if border.starts_with("border-") => {
                self.apply_border(&border["border-".len()..], value)?;
            }
            "box-shadow" => self.shadow = values::shadow(value).ok_or(invalid)?,
            "--fill" => self.pattern = values::pattern(value).ok_or(invalid)?,
            "--font" => self.font = values::font(value).ok_or(invalid)?,
            "position" => {
                self.position = keyword(
                    value,
                    &[
                        ("static", Position::Static),
                        ("relative", Position::Relative),
                        ("absolute", Position::Absolute),
                    ],
                )
                .ok_or(invalid)?;
            }
            "top" => self.inset.top = length(value).ok_or(invalid)?,
            "right" => self.inset.right = length(value).ok_or(invalid)?,
            "bottom" => self.inset.bottom = length(value).ok_or(invalid)?,
            "left" => self.inset.left = length(value).ok_or(invalid)?,
            "inset" => {
                let [top, right, bottom, left] = four(value, length).ok_or(invalid)?;
                self.inset = Inset {
                    top,
                    right,
                    bottom,
                    left,
                };
            }
            "z-index" => self.z_index = values::z_index(value).ok_or(invalid)?,
            "flex" => (self.flex_grow, self.flex_basis) = values::flex(value).ok_or(invalid)?,
            "flex-grow" => self.flex_grow = values::grow(value).ok_or(invalid)?,
            "flex-basis" => self.flex_basis = values::flex_basis(value).ok_or(invalid)?,
            "gap" | "column-gap" => self.gap = values::gap(value).ok_or(invalid)?,
            "justify-content" => {
                self.justify_content = keyword(
                    value,
                    &[
                        ("normal", Justify::Start),
                        ("start", Justify::Start),
                        ("flex-start", Justify::Start),
                        ("left", Justify::Start),
                        ("center", Justify::Center),
                        ("end", Justify::End),
                        ("flex-end", Justify::End),
                        ("right", Justify::End),
                        ("space-between", Justify::SpaceBetween),
                        ("space-around", Justify::SpaceAround),
                        ("space-evenly", Justify::SpaceEvenly),
                    ],
                )
                .ok_or(invalid)?;
            }
            "align-items" => {
                self.align_items = keyword(
                    value,
                    &[
                        ("normal", Align::Stretch),
                        ("stretch", Align::Stretch),
                        ("start", Align::Start),
                        ("flex-start", Align::Start),
                        ("baseline", Align::Start),
                        ("center", Align::Center),
                        ("end", Align::End),
                        ("flex-end", Align::End),
                    ],
                )
                .ok_or(invalid)?;
            }
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

    /// `border-<property>`: `style`, `color`, a side (`top`), or a side's
    /// `style`/`color` (`top-color`).
    fn apply_border(&mut self, property: &str, value: &str) -> Result<(), Rejected> {
        let invalid = Rejected::InvalidValue;
        match property {
            "style" => {
                let styles = values::four(value, values::border_style).ok_or(invalid)?;
                for (side, style) in self.border.sides_mut().into_iter().zip(styles) {
                    side.style = style;
                }
            }
            "color" => {
                let colors = values::four(value, parse_color).ok_or(invalid)?;
                for (side, color) in self.border.sides_mut().into_iter().zip(colors) {
                    side.color = Some(color);
                }
            }
            _ => {
                let (name, longhand) = property.split_once('-').unwrap_or((property, ""));
                let side = self
                    .border
                    .side_mut(name)
                    .ok_or(Rejected::UnknownProperty)?;
                match longhand {
                    "" => *side = values::border(value).ok_or(invalid)?,
                    "style" => side.style = values::border_style(value).ok_or(invalid)?,
                    "color" => side.color = Some(parse_color(value).ok_or(invalid)?),
                    _ => return Err(Rejected::UnknownProperty),
                }
            }
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

    #[test]
    fn border_longhands() {
        let mut style = ComputedStyle::root();
        style.apply("border", "single").unwrap();
        style.apply("border-style", "double none").unwrap();
        style.apply("border-left-color", "red").unwrap();
        style.apply("border-right", "thick blue").unwrap();
        assert_eq!(style.border.top.style, BorderStyle::Double);
        assert_eq!(style.border.left.style, BorderStyle::None);
        assert_eq!(style.border.left.color, Some(Ega::Red));
        assert_eq!(
            style.border.right,
            BorderSide {
                style: BorderStyle::Thick,
                color: Some(Ega::Blue)
            }
        );
        assert_eq!(style.border.widths(), Edges::from([1, 1, 1, 0]));
        assert_eq!(
            style.apply("border-radius", "2"),
            Err(Rejected::UnknownProperty)
        );
        assert_eq!(
            style.apply("border-top-width", "1"),
            Err(Rejected::UnknownProperty)
        );
    }

    #[test]
    fn positioning_properties() {
        let mut style = ComputedStyle::root();
        style.apply("position", "absolute").unwrap();
        style.apply("inset", "1 auto").unwrap();
        style.apply("left", "50%").unwrap();
        assert_eq!(style.position, Position::Absolute);
        assert_eq!(style.inset.top, Some(Length::Cells(1)));
        assert_eq!(style.inset.right, None);
        assert_eq!(style.inset.left, Some(Length::Percent(50.0)));
        assert_eq!(
            style.apply("position", "fixed"),
            Err(Rejected::InvalidValue)
        );
    }

    #[test]
    fn box_properties_are_not_inherited() {
        let mut parent = ComputedStyle::root();
        parent.apply("border", "double").unwrap();
        parent.apply("box-shadow", "2 1").unwrap();
        parent.apply("--fill", "light").unwrap();
        parent.apply("display", "flex").unwrap();
        parent.apply("gap", "2").unwrap();
        let child = parent.inherit();
        assert_eq!(child.border, Border::default());
        assert_eq!(child.shadow, None);
        assert_eq!(child.pattern, None);
        assert_eq!(child.gap, 0);
    }
}
