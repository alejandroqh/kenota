//! Box properties: borders, shadows, fill patterns, positioning and flex rows.

use super::{Edges, Length};
use crate::palette::Ega;

/// How a border line is drawn. Ordered from lightest to heaviest, so where
/// lines of two styles meet, the heavier one decides the junction.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum BorderStyle {
    #[default]
    None,
    /// `─│┌┐` (CSS `solid` means the same).
    Single,
    /// `═║╔╗`
    Double,
    /// Half and full blocks: `▀` on top, `█` at the sides, `▄` at the bottom.
    Thick,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BorderSide {
    pub style: BorderStyle,
    /// `None` is the element's text color (`currentcolor`).
    pub color: Option<Ega>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Border {
    pub top: BorderSide,
    pub right: BorderSide,
    pub bottom: BorderSide,
    pub left: BorderSide,
}

impl Border {
    pub fn all(side: BorderSide) -> Self {
        Self {
            top: side,
            right: side,
            bottom: side,
            left: side,
        }
    }

    /// Cells taken by each side: 1 when it has a border, else 0.
    pub fn widths(&self) -> Edges {
        let width = |side: &BorderSide| i32::from(side.style != BorderStyle::None);
        Edges {
            top: width(&self.top),
            right: width(&self.right),
            bottom: width(&self.bottom),
            left: width(&self.left),
        }
    }

    /// Top, right, bottom, left: the order of CSS shorthands.
    pub(super) fn sides_mut(&mut self) -> [&mut BorderSide; 4] {
        [
            &mut self.top,
            &mut self.right,
            &mut self.bottom,
            &mut self.left,
        ]
    }

    pub(super) fn side_mut(&mut self, name: &str) -> Option<&mut BorderSide> {
        match name {
            "top" => Some(&mut self.top),
            "right" => Some(&mut self.right),
            "bottom" => Some(&mut self.bottom),
            "left" => Some(&mut self.left),
            _ => None,
        }
    }
}

/// `box-shadow: 2 1 black`: a solid block offset right and down.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shadow {
    pub x: i32,
    pub y: i32,
    pub color: Ega,
}

/// `--fill: "░" blue`: a character repeated over the background.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pattern {
    pub symbol: char,
    /// `None` is the element's text color.
    pub color: Option<Ega>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Position {
    #[default]
    Static,
    /// Shifted by `top`/`left`/… without moving anything else; holds absolute children.
    Relative,
    /// Taken out of the flow and placed in the nearest positioned ancestor (or the slide).
    Absolute,
}

/// `top`, `right`, `bottom`, `left`. `None` is `auto`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Inset {
    pub top: Option<Length>,
    pub right: Option<Length>,
    pub bottom: Option<Length>,
    pub left: Option<Length>,
}

/// Which horizontal margins are `auto`. `margin: 0 auto` centers a box that has a width.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AutoMargin {
    pub left: bool,
    pub right: bool,
}

/// `justify-content` in a flex row: where leftover space goes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Justify {
    #[default]
    Start,
    Center,
    End,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

/// `align-items` in a flex row: how items sit in the row's height.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Align {
    /// Every item as tall as the row (the CSS default).
    #[default]
    Stretch,
    Start,
    Center,
    End,
}
