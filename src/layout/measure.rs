//! Intrinsic widths: how narrow a box can get, and how wide it is unwrapped.
//! Flex rows, table columns and absolutely positioned boxes size from these.

use std::ops::Add;

use super::{
    Ctx, flex, inline, is_absolute, is_block_level, is_table, layout_box, marker, picture, plot,
    table,
};
use crate::assets::Assets;
use crate::chart;
use crate::style::{Display, Length, StyledNode};

/// Border-box widths in cells.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Intrinsic {
    /// The narrowest width without overflow: usually the longest word.
    pub min: i32,
    /// The width with no wrapping at all.
    pub max: i32,
}

impl Intrinsic {
    pub fn fixed(width: i32) -> Self {
        Self {
            min: width,
            max: width,
        }
    }

    pub fn max(self, other: Self) -> Self {
        Self {
            min: self.min.max(other.min),
            max: self.max.max(other.max),
        }
    }

    /// The CSS shrink-to-fit width: as wide as the content wants, at most
    /// `available`, but never narrower than the minimum.
    pub fn fit(self, available: i32) -> i32 {
        self.max.min(available).max(self.min)
    }
}

impl Add<i32> for Intrinsic {
    type Output = Self;
    fn add(self, extra: i32) -> Self {
        Self {
            min: self.min + extra,
            max: self.max + extra,
        }
    }
}

impl Add for Intrinsic {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Self {
            min: self.min + other.min,
            max: self.max + other.max,
        }
    }
}

/// The intrinsic border-box widths of `node`.
pub(super) fn intrinsic(node: &StyledNode, assets: &Assets) -> Intrinsic {
    let style = &node.style;
    if let Some(Length::Cells(width)) = style.width {
        return Intrinsic::fixed(width.max(0));
    }
    if node.tag().is_none() {
        return inline::intrinsic(&[node], style, assets);
    }
    if chart::kind(node).is_some() {
        return plot::intrinsic(node, assets);
    }
    if is_table(node) {
        return table::intrinsic(node, assets);
    }
    if node.tag() == Some("img") {
        return picture::intrinsic(node, assets);
    }
    let border = style.border.widths();
    let frame = style.padding.left + style.padding.right + border.left + border.right;
    let content = match style.display {
        Display::Flex => flex::intrinsic(node, assets),
        _ if node.tag() == Some("hr") => Intrinsic::default(),
        _ => flow(node, assets),
    };
    content + frame
}

/// Block flow: the widest child or line.
fn flow(node: &StyledNode, assets: &Assets) -> Intrinsic {
    let markers = marker::for_children(node);
    let mut result = Intrinsic::default();
    let mut run = Vec::new();
    for (child, marker) in node.children.iter().zip(&markers) {
        if is_absolute(child) {
            continue;
        }
        if is_block_level(child) {
            result = result.max(inline::intrinsic(&run, &node.style, assets));
            run.clear();
            let margin = child.style.margin;
            let marker = marker.as_ref().map_or(0, |m| m.width);
            result = result.max(intrinsic(child, assets) + (margin.left + margin.right + marker));
        } else {
            run.push(child);
        }
    }
    result.max(inline::intrinsic(&run, &node.style, assets))
}

/// The border-box height `node` takes when laid out `width` wide.
pub(super) fn height(node: &StyledNode, width: i32, assets: &Assets) -> i32 {
    let mut scratch = Ctx::new(assets);
    layout_box(node, 0, 0, width, None, None, &mut scratch).height
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::html;

    fn body_child(html: &str) -> StyledNode {
        let slide = html::parse(html, None);
        assert!(slide.warnings.is_empty(), "{:?}", slide.warnings);
        slide.root.find("body").unwrap().children[0].clone()
    }

    #[test]
    fn longest_word_and_whole_line() {
        let p = body_child("<p>a lengthy paragraph</p>");
        assert_eq!(
            intrinsic(&p, &Assets::default()),
            Intrinsic { min: 9, max: 19 }
        );
    }

    #[test]
    fn padding_border_and_children() {
        let div = body_child(
            "<div style='padding: 0 2; border: single'><p>tiny</p><p style='margin-left: 3'>words here</p></div>",
        );
        assert_eq!(
            intrinsic(&div, &Assets::default()),
            Intrinsic {
                min: 6 + 8,
                max: 13 + 6
            }
        );
    }

    #[test]
    fn fixed_width_wins() {
        let div = body_child("<div style='width: 12'>a very long line of text</div>");
        assert_eq!(intrinsic(&div, &Assets::default()), Intrinsic::fixed(12));
    }

    #[test]
    fn list_markers_count() {
        let ul = body_child("<ul><li>abc</li></ul>");
        assert_eq!(
            intrinsic(&ul, &Assets::default()),
            Intrinsic {
                min: 2 + 2 + 3,
                max: 7
            }
        );
    }

    #[test]
    fn fit_clamps() {
        let i = Intrinsic { min: 5, max: 20 };
        assert_eq!(i.fit(30), 20);
        assert_eq!(i.fit(10), 10);
        assert_eq!(i.fit(3), 5);
    }
}
