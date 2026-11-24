//! The pixel canvas behind pictures and charts.
//!
//! Each character cell shows two pixels, one above the other, with the
//! half blocks `▀` and `▄`: an 80×25 slide is 80×50 pixels. Pixels are
//! EGA colors (or, for charts, richer inks) or transparent; a transparent
//! half leaves whatever is behind the cell showing.

use crate::palette::Ega;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pixels<T = Ega> {
    width: usize,
    height: usize,
    data: Vec<Option<T>>,
}

/// One character cell of a pixel canvas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    pub symbol: char,
    pub fg: Ega,
    /// `None` keeps the background already under the cell.
    pub bg: Option<Ega>,
}

impl<T: Copy> Pixels<T> {
    /// A transparent canvas.
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            data: vec![None; width * height],
        }
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn get(&self, x: usize, y: usize) -> Option<T> {
        if x < self.width && y < self.height {
            self.data[y * self.width + x]
        } else {
            None
        }
    }

    /// Sets one pixel; coordinates off the canvas are ignored.
    pub fn set(&mut self, x: usize, y: usize, value: Option<T>) {
        if x < self.width && y < self.height {
            self.data[y * self.width + x] = value;
        }
    }

    /// The cells that show the canvas, row by row: `ceil(height / 2)` rows
    /// of `width` cells, each made by `cell` from its top and bottom pixel.
    pub fn cells_with(
        &self,
        cell: impl Fn(Option<T>, Option<T>) -> Option<Cell>,
    ) -> Vec<Vec<Option<Cell>>> {
        (0..self.height.div_ceil(2))
            .map(|row| {
                (0..self.width)
                    .map(|x| cell(self.get(x, row * 2), self.get(x, row * 2 + 1)))
                    .collect()
            })
            .collect()
    }
}

impl Pixels {
    /// The cells that show the canvas. `None` where both pixels are transparent.
    pub fn cells(&self) -> Vec<Vec<Option<Cell>>> {
        self.cells_with(cell)
    }
}

/// The cell for a top and a bottom pixel.
pub fn cell(top: Option<Ega>, bottom: Option<Ega>) -> Option<Cell> {
    let (symbol, fg, bg) = match (top, bottom) {
        (None, None) => return None,
        (Some(top), None) => ('▀', top, None),
        (None, Some(bottom)) => ('▄', bottom, None),
        (Some(top), Some(bottom)) if top == bottom => ('█', top, None),
        (Some(top), Some(bottom)) => ('▀', top, Some(bottom)),
    };
    Some(Cell { symbol, fg, bg })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_pixels_per_cell() {
        let mut pixels = Pixels::new(4, 3);
        pixels.set(0, 0, Some(Ega::Red));
        pixels.set(1, 1, Some(Ega::Blue));
        pixels.set(2, 0, Some(Ega::Green));
        pixels.set(2, 1, Some(Ega::Green));
        pixels.set(3, 0, Some(Ega::White));
        pixels.set(3, 1, Some(Ega::Black));
        pixels.set(0, 2, Some(Ega::Yellow));
        pixels.set(9, 9, Some(Ega::Yellow));

        let cells = pixels.cells();
        assert_eq!(cells.len(), 2, "an odd last row gets its own cell row");
        let symbols: Vec<String> = cells
            .iter()
            .map(|row| row.iter().map(|c| c.map_or('.', |c| c.symbol)).collect())
            .collect();
        assert_eq!(symbols, ["▀▄█▀", "▀..."]);
        assert_eq!(cells[0][1].unwrap().fg, Ega::Blue);
        assert_eq!(
            cells[0][1].unwrap().bg,
            None,
            "the transparent half shows through"
        );
        assert_eq!(cells[0][3].unwrap().bg, Some(Ega::Black));
    }
}
