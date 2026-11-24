//! Pictures for `<img>`: PNG and GIF files read once when the deck loads,
//! then scaled and reduced to the 16 EGA colors when a slide is drawn.

use std::collections::HashMap;
use std::fmt;
use std::path::Path;
use std::sync::Arc;

use image::{RgbaImage, imageops};

use crate::palette::Ega;
use crate::pixels::Pixels;
use crate::style::StyledNode;

/// Larger pictures are shrunk to this many pixels on their longest side
/// when loaded. The tallest screen (80×50 cells) shows 80×100 pixels.
const MAX_SIDE: u32 = 200;

/// The 4×4 Bayer matrix for ordered dithering.
const BAYER: [[u8; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];

/// How far dithering may nudge a channel: the gap between EGA levels (0, 85, 170, 255).
const DITHER_SPREAD: f32 = 85.0;

pub struct Picture {
    /// Size of the original file, in pixels.
    width: u32,
    height: u32,
    image: RgbaImage,
}

impl fmt::Debug for Picture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Picture")
            .field("width", &self.width)
            .field("height", &self.height)
            .finish_non_exhaustive()
    }
}

impl Picture {
    /// Reads a PNG or GIF file (the first frame of an animation).
    ///
    /// # Errors
    /// If the file cannot be read or decoded.
    pub fn load(path: &Path) -> Result<Self, String> {
        let image = image::open(path).map_err(|e| e.to_string())?;
        Self::new(image.into_rgba8())
    }

    /// # Errors
    /// If the image has no pixels.
    pub fn new(image: RgbaImage) -> Result<Self, String> {
        let (width, height) = image.dimensions();
        if width == 0 || height == 0 {
            return Err("the image is empty".into());
        }
        let longest = width.max(height);
        let image = if longest > MAX_SIDE {
            let scale = |side: u32| {
                u32::try_from(u64::from(side) * u64::from(MAX_SIDE) / u64::from(longest))
                    .unwrap_or(1)
                    .max(1)
            };
            imageops::thumbnail(&image, scale(width), scale(height))
        } else {
            image
        };
        Ok(Self {
            width,
            height,
            image,
        })
    }

    /// Width of the original file in pixels.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Height of the original file in pixels.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// The picture scaled to `width`×`height` pixels and reduced to EGA
    /// colors, optionally with ordered dithering. Mostly transparent
    /// areas stay transparent.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub fn render(&self, width: usize, height: usize, dither: bool) -> Pixels {
        let mut out = Pixels::new(width, height);
        let (source_width, source_height) = self.image.dimensions();
        for y in 0..height {
            let (top, bottom) = span(y, height, source_height);
            for x in 0..width {
                let (left, right) = span(x, width, source_width);
                // Average the source pixels, weighting colors by opacity.
                let mut sum = [0u64; 4];
                let mut count = 0u64;
                for sy in top..bottom {
                    for sx in left..right {
                        let [r, g, b, a] = self.image.get_pixel(sx, sy).0;
                        let a = u64::from(a);
                        sum[0] += u64::from(r) * a;
                        sum[1] += u64::from(g) * a;
                        sum[2] += u64::from(b) * a;
                        sum[3] += a;
                        count += 1;
                    }
                }
                if sum[3] * 2 < 255 * count {
                    continue;
                }
                let nudge = if dither {
                    (f32::from(BAYER[y % 4][x % 4]) + 0.5) / 16.0 - 0.5
                } else {
                    0.0
                };
                let channel = |i: usize| {
                    let value = sum[i] as f32 / sum[3] as f32 + nudge * DITHER_SPREAD;
                    value.round().clamp(0.0, 255.0) as u8
                };
                out.set(x, y, Some(Ega::nearest(channel(0), channel(1), channel(2))));
            }
        }
        out
    }
}

/// The source pixels `start..end` that target pixel `t` (of `target`)
/// covers; always at least one.
fn span(t: usize, target: usize, source: u32) -> (u32, u32) {
    let (t, target, source) = (t as u64, target.max(1) as u64, u64::from(source));
    let start = (t * source / target).min(source - 1);
    let end = ((t + 1) * source).div_ceil(target).clamp(start + 1, source);
    (
        u32::try_from(start).unwrap_or(0),
        u32::try_from(end).unwrap_or(u32::MAX),
    )
}

/// The pictures a deck's slides use, by their `src`.
#[derive(Clone, Debug, Default)]
pub struct Images {
    by_src: HashMap<String, Arc<Picture>>,
}

impl Images {
    pub fn get(&self, src: &str) -> Option<&Picture> {
        self.by_src.get(src.trim()).map(Arc::as_ref)
    }

    pub fn insert(&mut self, src: &str, picture: Picture) {
        self.by_src
            .insert(src.trim().to_string(), Arc::new(picture));
    }

    /// Loads the pictures of the `<img>` elements under `node`, with
    /// `src` paths relative to `dir`. Problems are added to `warnings`;
    /// those images show their `alt` text instead.
    pub fn load_for(&mut self, node: &StyledNode, dir: &Path, warnings: &mut Vec<String>) {
        if node.tag() == Some("img") {
            let mut warn = |warning: String| {
                if !warnings.contains(&warning) {
                    warnings.push(warning);
                }
            };
            match node.attr("src").map(str::trim) {
                None | Some("") => warn("<img> without a src".into()),
                Some(src) if self.by_src.contains_key(src) => {}
                Some(src) if src.contains("://") || src.starts_with("data:") => {
                    warn(format!(
                        "image `{src}` not shown: use a file in the deck folder"
                    ));
                }
                Some(src) => match Picture::load(&dir.join(src)) {
                    Ok(picture) => self.insert(src, picture),
                    Err(problem) => warn(format!("image `{src}` not shown: {problem}")),
                },
            }
        }
        for child in &node.children {
            self.load_for(child, dir, warnings);
        }
    }
}

#[cfg(test)]
mod tests {
    use image::Rgba;

    use super::*;

    fn picture(width: u32, height: u32, pixel: impl Fn(u32, u32) -> [u8; 4]) -> Picture {
        Picture::new(RgbaImage::from_fn(width, height, |x, y| Rgba(pixel(x, y)))).unwrap()
    }

    #[test]
    fn scales_down_by_averaging() {
        // Left half red, right half blue, 8×4 → 2×1.
        let p = picture(8, 4, |x, _| {
            if x < 4 {
                [170, 0, 0, 255]
            } else {
                [0, 0, 170, 255]
            }
        });
        let pixels = p.render(2, 1, false);
        assert_eq!(pixels.get(0, 0), Some(Ega::Red));
        assert_eq!(pixels.get(1, 0), Some(Ega::Blue));
    }

    #[test]
    fn scales_up_by_repeating() {
        let p = picture(2, 1, |x, _| if x == 0 { [255; 4] } else { [0, 0, 0, 255] });
        let pixels = p.render(4, 2, false);
        assert_eq!(pixels.get(1, 1), Some(Ega::White));
        assert_eq!(pixels.get(2, 0), Some(Ega::Black));
    }

    #[test]
    fn transparent_areas_stay_transparent() {
        let p = picture(
            2,
            1,
            |x, _| if x == 0 { [255, 255, 85, 255] } else { [0; 4] },
        );
        let pixels = p.render(2, 1, false);
        assert_eq!(pixels.get(0, 0), Some(Ega::Yellow));
        assert_eq!(pixels.get(1, 0), None);
    }

    #[test]
    fn dithering_mixes_two_colors() {
        // Halfway between black and dark gray: plain snaps to one color, dithering mixes both.
        let gray = picture(4, 4, |_, _| [42, 42, 42, 255]);
        let count = |pixels: &Pixels, color| {
            (0..4)
                .flat_map(|y| (0..4).map(move |x| (x, y)))
                .filter(|&(x, y)| pixels.get(x, y) == Some(color))
                .count()
        };
        let plain = gray.render(4, 4, false);
        assert_eq!(count(&plain, Ega::Black), 16);
        let dithered = gray.render(4, 4, true);
        assert_eq!(count(&dithered, Ega::Black), 8);
        assert_eq!(count(&dithered, Ega::DarkGray), 8);
    }

    #[test]
    fn big_pictures_are_shrunk_on_load() {
        let p = picture(1000, 500, |_, _| [0, 0, 0, 255]);
        assert_eq!(
            (p.width(), p.height()),
            (1000, 500),
            "keeps the original size"
        );
        assert_eq!(p.image.dimensions(), (200, 100));
    }

    #[test]
    fn spans_cover_the_source() {
        assert_eq!(span(0, 2, 8), (0, 4));
        assert_eq!(span(1, 2, 8), (4, 8));
        assert_eq!(span(3, 4, 2), (1, 2));
        assert_eq!(span(1, 3, 4), (1, 3));
    }

    #[test]
    fn loads_gif_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("dot.gif");
        RgbaImage::from_pixel(4, 3, Rgba([255, 85, 85, 255]))
            .save(&path)
            .unwrap();
        let p = Picture::load(&path).unwrap();
        assert_eq!((p.width(), p.height()), (4, 3));
        assert_eq!(p.render(1, 1, false).get(0, 0), Some(Ega::LightRed));
    }

    #[test]
    fn loads_files_and_reports_problems() {
        let dir = tempfile::tempdir().unwrap();
        RgbaImage::from_pixel(3, 2, Rgba([0, 170, 0, 255]))
            .save(dir.path().join("dot.png"))
            .unwrap();
        std::fs::write(dir.path().join("bad.png"), "not a png").unwrap();

        let slide = crate::html::parse(
            "<img src='dot.png'><img src=' dot.png '><img src='bad.png'><img src='gone.gif'>
             <img src='http://example.com/a.png'><img>",
            None,
        );
        let mut images = Images::default();
        let mut warnings = Vec::new();
        images.load_for(&slide.root, dir.path(), &mut warnings);

        assert_eq!(images.get("dot.png").unwrap().width(), 3);
        assert_eq!(warnings.len(), 4, "{warnings:?}");
        assert!(
            warnings[0].starts_with("image `bad.png` not shown"),
            "{warnings:?}"
        );
        assert!(
            warnings[1].starts_with("image `gone.gif` not shown"),
            "{warnings:?}"
        );
        assert!(warnings[2].contains("use a file in the deck folder"));
        assert_eq!(warnings[3], "<img> without a src");
    }
}
