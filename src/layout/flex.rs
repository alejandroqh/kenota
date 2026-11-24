//! Flex rows: `display: flex` puts children side by side.
//!
//! Items start at their `flex-basis`, else their set `width`, else their
//! content's unwrapped width. Spare width goes to items with `flex-grow`
//! (`flex: 1` grows from a basis of 0, so equal factors mean equal widths),
//! then to `justify-content`. When the row is too narrow, items without a
//! set width shrink, wrapping their text. Those never get narrower than
//! their longest word. `gap` adds columns between items. Items stretch to
//! the row's height unless `align-items` says otherwise. One row only:
//! there is no wrapping onto a second row.

use super::measure::{self, Intrinsic};
use super::{Ctx, Layer, Pending, distribute, inline, is_absolute, layout_box};
use crate::assets::Assets;
use crate::style::{Align, Content, Justify, StyledNode};

/// Lays out the children of `node` in a row starting at (`x`, `y`),
/// `width` wide. `height` is the container's inner height when it has a
/// fixed one. Returns the row's height.
pub(super) fn layout<'a>(
    node: &'a StyledNode,
    x: i32,
    y: i32,
    width: i32,
    height: Option<i32>,
    ctx: &mut Ctx<'a>,
) -> i32 {
    let style = &node.style;
    let mut items = Vec::new();
    for child in &node.children {
        if is_absolute(child) {
            ctx.pending.push(Pending { node: child, x, y });
        } else if is_item(child) {
            items.push(child);
        }
    }
    if items.is_empty() {
        return height.unwrap_or(0);
    }

    let widths = widths(&items, width, style.gap, ctx.assets);
    let used: i32 = widths.iter().sum::<i32>()
        + items.iter().map(|item| margin_x(item)).sum::<i32>()
        + style.gap * gaps(items.len());
    let spacing = spacing(style.justify_content, width - used, items.len());

    // Heights first, so that items can stretch to the tallest.
    let heights: Vec<i32> = items
        .iter()
        .zip(&widths)
        .map(|(item, &width)| outer_height(item, width, ctx.assets))
        .collect();
    let row = height.unwrap_or_else(|| heights.iter().copied().max().unwrap_or(0));

    let mut cursor = x + spacing[0];
    for (i, item) in items.iter().enumerate() {
        let margin = item.style.margin;
        let item_x = cursor + margin.left;
        let free = row - heights[i];
        let offset = match style.align_items {
            Align::Center => free / 2,
            Align::End => free,
            Align::Start | Align::Stretch => 0,
        };
        if item.tag().is_none() {
            inline::layout(
                &[*item],
                &item.style,
                ctx.assets,
                item_x,
                y + offset,
                widths[i],
                &mut ctx.layer,
            );
        } else if style.align_items == Align::Stretch && item.style.height.is_none() {
            let stretch = row - margin.top - margin.bottom;
            layout_box(
                item,
                item_x,
                y + margin.top,
                widths[i],
                Some(stretch),
                None,
                ctx,
            );
        } else {
            layout_box(
                item,
                item_x,
                y + margin.top + offset,
                widths[i],
                None,
                None,
                ctx,
            );
        }
        cursor = item_x + widths[i] + margin.right + style.gap + spacing[i + 1];
    }
    row
}

/// The narrowest and widest the row's content can be.
pub(super) fn intrinsic(node: &StyledNode, assets: &Assets) -> Intrinsic {
    let items: Vec<&StyledNode> = node
        .children
        .iter()
        .filter(|child| !is_absolute(child) && is_item(child))
        .collect();
    let content = items.iter().fold(Intrinsic::default(), |sum, item| {
        sum + measure::intrinsic(item, assets) + margin_x(item)
    });
    content + node.style.gap * gaps(items.len())
}

/// Elements are items; so is text, unless it is only whitespace.
fn is_item(child: &StyledNode) -> bool {
    match &child.content {
        Content::Element { .. } => true,
        Content::Text(text) => !text.chars().all(inline::is_html_space),
    }
}

fn margin_x(item: &StyledNode) -> i32 {
    item.style.margin.left + item.style.margin.right
}

fn gaps(items: usize) -> i32 {
    i32::try_from(items.saturating_sub(1)).unwrap_or(0)
}

/// Border-box widths of `items` in a row `width` wide.
#[allow(clippy::cast_precision_loss)]
fn widths(items: &[&StyledNode], width: i32, gap: i32, assets: &Assets) -> Vec<i32> {
    let intrinsic: Vec<Intrinsic> = items
        .iter()
        .map(|item| measure::intrinsic(item, assets))
        .collect();
    let mut sizes: Vec<i32> = items
        .iter()
        .zip(&intrinsic)
        .map(|(item, size)| {
            let style = &item.style;
            match (style.flex_basis, style.width) {
                (Some(basis), _) => basis.resolve(width),
                (None, Some(w)) => w.resolve(width),
                (None, None) => size.max,
            }
        })
        .collect();
    // Items with a set width keep it when shrinking; the others stop at their longest word.
    let floors: Vec<Option<i32>> = items
        .iter()
        .zip(&intrinsic)
        .map(|(item, size)| item.style.width.is_none().then_some(size.min))
        .collect();
    let available =
        width - items.iter().map(|item| margin_x(item)).sum::<i32>() - gap * gaps(items.len());
    let free = available - sizes.iter().sum::<i32>();

    if free >= 0 {
        let factors: Vec<f32> = items.iter().map(|item| item.style.flex_grow).collect();
        let floors: Vec<i32> = floors.iter().map(|floor| floor.unwrap_or(0)).collect();
        grow(&mut sizes, &factors, &floors, available);
    } else {
        shrink(&mut sizes, &floors, -free);
        for (size, floor) in sizes.iter_mut().zip(&floors) {
            *size = (*size).max(floor.unwrap_or(0));
        }
    }
    sizes
}

/// Shares out what `available` leaves after `sizes` by the grow
/// `factors`. An item that would end up below its floor is held at the
/// floor and the others share the rest.
#[allow(clippy::needless_range_loop)]
fn grow(sizes: &mut [i32], factors: &[f32], floors: &[i32], available: i32) {
    let base = sizes.to_vec();
    let mut held = vec![false; sizes.len()];
    loop {
        let used: i32 = (0..sizes.len())
            .map(|i| if held[i] { sizes[i] } else { base[i] })
            .sum();
        let weights: Vec<f32> = (0..sizes.len())
            .map(|i| if held[i] { 0.0 } else { factors[i] })
            .collect();
        let shares = distribute(available - used, &weights);
        let mut changed = false;
        for i in 0..sizes.len() {
            if held[i] {
                continue;
            }
            sizes[i] = base[i] + shares[i];
            if sizes[i] < floors[i] {
                sizes[i] = floors[i];
                held[i] = true;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
}

/// Takes `deficit` cells from the items that have a floor, in proportion
/// to their size, never going below the floor.
#[allow(clippy::cast_precision_loss)]
fn shrink(sizes: &mut [i32], floors: &[Option<i32>], mut deficit: i32) {
    while deficit > 0 {
        let weights: Vec<f32> = sizes
            .iter()
            .zip(floors)
            .map(|(&size, floor)| match floor {
                Some(floor) if size > *floor => size as f32,
                _ => 0.0,
            })
            .collect();
        let cuts = distribute(deficit, &weights);
        let mut progress = false;
        for ((size, floor), cut) in sizes.iter_mut().zip(floors).zip(cuts) {
            let cut = cut.min(*size - floor.unwrap_or(*size));
            if cut > 0 {
                *size -= cut;
                deficit -= cut;
                progress = true;
            }
        }
        if !progress {
            break;
        }
    }
}

/// Spare columns before, between and after `n` items.
fn spacing(justify: Justify, free: i32, n: usize) -> Vec<i32> {
    let free = free.max(0);
    let mut slots = vec![0; n + 1];
    let mut weights = vec![0.0; n + 1];
    match justify {
        Justify::Start => slots[n] = free,
        Justify::End => slots[0] = free,
        Justify::Center => {
            slots[0] = free / 2;
            slots[n] = free - free / 2;
        }
        Justify::SpaceBetween if n > 1 => weights[1..n].fill(1.0),
        Justify::SpaceBetween => slots[n] = free,
        Justify::SpaceAround => {
            weights.fill(1.0);
            weights[0] = 0.5;
            weights[n] = 0.5;
        }
        Justify::SpaceEvenly => weights.fill(1.0),
    }
    if weights.iter().any(|w| *w > 0.0) {
        slots = distribute(free, &weights);
    }
    slots
}

/// Height of an item including its vertical margins.
fn outer_height(item: &StyledNode, width: i32, assets: &Assets) -> i32 {
    if item.tag().is_none() {
        return inline::layout(
            &[item],
            &item.style,
            assets,
            0,
            0,
            width,
            &mut Layer::default(),
        )
        .0;
    }
    let margin = item.style.margin;
    measure::height(item, width, assets) + margin.top + margin.bottom
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shrink_respects_floors_and_fixed_items() {
        let mut sizes = vec![20, 10, 10];
        shrink(&mut sizes, &[Some(5), None, Some(8)], 12);
        assert_eq!(sizes, vec![10, 10, 8]);
    }

    #[test]
    fn grow_holds_items_at_their_floor() {
        let mut sizes = vec![0, 0, 0];
        grow(&mut sizes, &[1.0, 1.0, 1.0], &[0, 0, 0], 68);
        assert_eq!(sizes, vec![23, 23, 22], "equal shares from a zero basis");

        let mut sizes = vec![0, 0];
        grow(&mut sizes, &[1.0, 1.0], &[15, 0], 20);
        assert_eq!(sizes, vec![15, 5], "the first is held at its floor");
    }

    #[test]
    fn justify_spacing() {
        assert_eq!(spacing(Justify::Start, 4, 2), vec![0, 0, 4]);
        assert_eq!(spacing(Justify::Center, 5, 2), vec![2, 0, 3]);
        assert_eq!(spacing(Justify::SpaceBetween, 5, 3), vec![0, 3, 2, 0]);
        assert_eq!(spacing(Justify::SpaceEvenly, 6, 2), vec![2, 2, 2]);
        assert_eq!(spacing(Justify::End, -3, 2), vec![0, 0, 0]);
    }
}
