//! Blocky pictures: a small picture enlarged without smoothing is made of
//! square blocks of one colour each. Tracing the blocks would draw their
//! steps; tracing one sample per block traces the picture that was
//! enlarged, whose outlines the tracer can then follow between its
//! pixels. See docs/behavior/bitmap-tracing.md.

use image::{Rgba, RgbaImage};

/// The block grid of a picture: block size and where the first whole
/// block starts, across and down (pixels; sizes need not be whole).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Grid {
    pub x0: f64,
    pub bx: f64,
    pub y0: f64,
    pub by: f64,
}

/// Two neighbouring pixels differ when a channel differs by more than
/// this (some slack for compression).
const DIFFERENT: i32 = 24;
/// Smallest block size taken as blocky.
const MIN_BLOCK: f64 = 3.0;
/// Largest block size looked for.
const MAX_BLOCK: f64 = 64.0;
/// Share of the colour changes that must fall on the grid lines.
const ON_GRID: f64 = 0.9;
/// Share of the pixels inside the blocks that must match their centre.
const SAME_IN_BLOCK: f64 = 0.85;

fn differ(a: &Rgba<u8>, b: &Rgba<u8>) -> bool {
    (0..4).any(|k| (a[k] as i32 - b[k] as i32).abs() > DIFFERENT)
}

/// How many pixel pairs change colour across each line between columns
/// (`across`) or rows.
fn changes(img: &RgbaImage, across: bool) -> Vec<f64> {
    let (w, h) = img.dimensions();
    let n = if across { w } else { h };
    let mut out = vec![0.0; n as usize];
    for (i, o) in out.iter_mut().enumerate().skip(1) {
        let i = i as u32;
        let m = if across { h } else { w };
        let mut c = 0;
        for j in 0..m {
            let (a, b) = if across {
                (img.get_pixel(i - 1, j), img.get_pixel(i, j))
            } else {
                (img.get_pixel(j, i - 1), img.get_pixel(j, i))
            };
            if differ(a, b) {
                c += 1;
            }
        }
        *o = c as f64;
    }
    out
}

/// The largest block size (and its phase) whose grid lines carry nearly
/// all the colour changes along one direction.
fn period(ch: &[f64]) -> Option<(f64, f64)> {
    let total: f64 = ch.iter().sum();
    if total <= 0.0 {
        return None;
    }
    let n = ch.len() as f64;
    // Blocks at least 3 wide leave most lines without a change; a smooth
    // picture changes nearly everywhere (and is not searched further).
    let nz: Vec<(f64, f64)> = ch
        .iter()
        .enumerate()
        .filter(|(_, c)| **c > 0.0)
        .map(|(i, c)| (i as f64, *c))
        .collect();
    if nz.len() as f64 > 0.7 * n {
        return None;
    }
    // For a block size: the changes on its best grid, that grid's phase
    // (centred on them) and their mean distance from its lines.
    let fit = |b: f64| -> (f64, f64, f64) {
        let tol = slack(b);
        let mut best = (0.0, 0.0);
        let mut phase = 0.0;
        while phase < b {
            // A change at the line before pixel i lies at position i.
            let on: f64 = nz
                .iter()
                .filter(|(i, _)| {
                    let d = (i - phase).rem_euclid(b);
                    d.min(b - d) <= tol
                })
                .map(|(_, c)| *c)
                .sum();
            if on > best.0 {
                best = (on, phase);
            }
            phase += 0.25;
        }
        let (on, phase) = best;
        let (mut s, mut dev, mut wsum) = (0.0, 0.0, 0.0);
        for (i, c) in &nz {
            let mut d = (i - phase).rem_euclid(b);
            if d > b / 2.0 {
                d -= b;
            }
            if d.abs() <= tol {
                s += d * c;
                wsum += c;
            }
        }
        let shift = if wsum > 0.0 { s / wsum } else { 0.0 };
        for (i, c) in &nz {
            let mut d = (i - phase - shift).rem_euclid(b);
            if d > b / 2.0 {
                d -= b;
            }
            if d.abs() <= tol {
                dev += d.abs() * c;
            }
        }
        let dev = if wsum > 0.0 { dev / wsum } else { f64::MAX };
        (on, (phase + shift).rem_euclid(b), dev)
    };
    // The largest size whose grid carries nearly all changes; then, as the
    // slack lets a slightly wrong size pass on few blocks, the size close
    // below it whose lines the changes follow best.
    let mut b = MAX_BLOCK.min(n / 3.0);
    while b >= MIN_BLOCK {
        if fit(b).0 >= ON_GRID * total {
            let mut pick: Option<(f64, f64, f64)> = None;
            let mut c = b;
            while c >= (b * 0.85).max(MIN_BLOCK) {
                let (on, phase, dev) = fit(c);
                if on >= ON_GRID * total && pick.is_none_or(|p| dev < p.2 - 1e-9) {
                    pick = Some((c, phase, dev));
                }
                c -= 0.05;
            }
            // A phase just short of a whole block is a grid starting at
            // the picture's edge.
            return pick.map(|(c, phase, _)| (c, if phase > c - slack(c) { phase - c } else { phase }));
        }
        b -= 0.05;
    }
    None
}

/// The block grid of `img`, when it is made of blocks at least
/// [`MIN_BLOCK`] pixels wide whose pixels nearly all match their centre.
pub fn detect(img: &RgbaImage) -> Option<Grid> {
    let (w, h) = img.dimensions();
    if w < 9 || h < 9 {
        return None;
    }
    let (bx, x0) = period(&changes(img, true))?;
    let (by, y0) = period(&changes(img, false))?;
    // Blocks of a picture enlarged evenly are about square.
    if (bx / by - 1.0).abs() > 0.1 {
        return None;
    }
    let g = Grid { x0, bx, y0, by };
    let (cols, rows) = size(&g, w, h);
    if cols < 2 || rows < 2 {
        return None;
    }
    // Check the blocks really are of one colour each.
    let small = sample(img, &g);
    let mut same = 0usize;
    let mut all = 0usize;
    for y in 0..h {
        let r = ((y as f64 - y0) / by).floor();
        if r < 0.0 || r >= rows as f64 {
            continue;
        }
        if near_edge(y as f64 + 0.5 - y0, by) {
            continue;
        }
        for x in 0..w {
            let c = ((x as f64 - x0) / bx).floor();
            if c < 0.0 || c >= cols as f64 {
                continue;
            }
            if near_edge(x as f64 + 0.5 - x0, bx) {
                continue;
            }
            all += 1;
            if !differ(img.get_pixel(x, y), small.get_pixel(c as u32, r as u32)) {
                same += 1;
            }
        }
    }
    (all > 0 && same as f64 >= SAME_IN_BLOCK * all as f64).then_some(g)
}

/// How far from a grid line a colour change may lie: a picture enlarged
/// and then resampled smears each line over a pixel or two.
fn slack(b: f64) -> f64 {
    (0.15 * b).clamp(0.6, 1.25)
}

/// A pixel centre `d` past a grid line lies in the smeared band of a
/// line; only the inside of the blocks is checked for one colour.
fn near_edge(d: f64, b: f64) -> bool {
    let r = d.rem_euclid(b);
    r.min(b - r) < slack(b)
}

/// Whole blocks across and down.
fn size(g: &Grid, w: u32, h: u32) -> (u32, u32) {
    (
        ((w as f64 - g.x0) / g.bx + 0.05).floor().max(0.0) as u32,
        ((h as f64 - g.y0) / g.by + 0.05).floor().max(0.0) as u32,
    )
}

/// One pixel per whole block: the colour at its centre.
pub fn sample(img: &RgbaImage, g: &Grid) -> RgbaImage {
    let (w, h) = img.dimensions();
    let (cols, rows) = size(g, w, h);
    RgbaImage::from_fn(cols.max(1), rows.max(1), |c, r| {
        let x = (g.x0 + (c as f64 + 0.5) * g.bx).floor() as u32;
        let y = (g.y0 + (r as f64 + 0.5) * g.by).floor() as u32;
        *img.get_pixel(x.min(w - 1), y.min(h - 1))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 12 x 9 picture of a few colours, enlarged `b` times without
    /// smoothing, shifted by `off` pixels.
    fn blocky(b: u32, off: u32) -> RgbaImage {
        let small = RgbaImage::from_fn(12, 9, |x, y| {
            if (x + y) % 3 == 0 {
                Rgba([240, 120, 30, 255])
            } else if x % 4 == 1 {
                Rgba([90, 50, 20, 255])
            } else {
                Rgba([0, 0, 0, 255])
            }
        });
        RgbaImage::from_fn(12 * b + off, 9 * b + off, |x, y| {
            let (sx, sy) = (x.saturating_sub(off) / b, y.saturating_sub(off) / b);
            *small.get_pixel(sx.min(11), sy.min(8))
        })
    }

    #[test]
    fn an_enlarged_picture_shows_its_blocks() {
        for (b, off) in [(10, 0), (7, 3), (4, 1)] {
            let img = blocky(b, off);
            let g = detect(&img).expect("blocks");
            assert!((g.bx - b as f64).abs() < 0.3 && (g.by - b as f64).abs() < 0.3, "{b}: {g:?}");
            let s = sample(&img, &g);
            assert!(s.width() >= 11 && s.height() >= 8, "{b}: {}x{}", s.width(), s.height());
        }
    }

    #[test]
    fn a_smooth_picture_has_no_blocks() {
        let img = RgbaImage::from_fn(80, 80, |x, y| {
            let d = ((x as f32 - 40.0).powi(2) + (y as f32 - 40.0).powi(2)).sqrt();
            let v = (255.0 * (30.0 - d).clamp(0.0, 1.0)) as u8;
            Rgba([v, v / 2, 0, 255])
        });
        assert_eq!(detect(&img), None);
        assert_eq!(detect(&RgbaImage::new(5, 5)), None);
        assert_eq!(detect(&RgbaImage::new(50, 50)), None);
    }
}
