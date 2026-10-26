//! Outlines of two-tone pictures (black and white mode, and any picture
//! whose pixels are all mixes of two colours: most logos, icons and
//! lettering). Every pixel gets an amount of ink, 0 on paper, 1 on ink and
//! in between on the anti-aliased edge (paper and ink are the means of the
//! two Otsu classes). The outline is where that amount, interpolated
//! smoothly between the pixel centres, is one half: the edge falls between
//! the pixels where it lay in the picture, without a staircase. Each piece
//! of ink, with the holes in it, becomes one shape. See
//! docs/behavior/bitmap-tracing.md.

use crate::trace::{Settings, Traced};
use crate::trace_color::{from_oklab, oklab};
use graphdraw_core::{
    geometry::{BezPath, Point, Shape as _},
    Color,
};
use image::RgbaImage;

/// Most cells the finer grid the outline is followed on may have.
const FINE_MAX: usize = 40_000_000;
/// Finest the outline is followed: this many cells per pixel each way.
const FINE_K: usize = 8;
/// A picture is two-tone when this share of its opaque pixels lies on
/// the mix of its two main colours ...
const TWO_TONE_SHARE: f64 = 0.985;
/// ... within this distance in Oklab.
const TWO_TONE_NEAR: f32 = 0.06;
/// More pixels than this inside a patch of another colour make a third
/// colour.
const THIRD_COLOUR: usize = 12;

/// How many cells per pixel each way the outline of a `w` x `h` picture
/// is followed on.
pub fn fine_factor(w: usize, h: usize) -> usize {
    let px = (w * h).max(1);
    let k = ((FINE_MAX / px) as f64).sqrt().floor() as usize;
    k.clamp(1, FINE_K)
}

/// Largest distance (pixels) from the outline to the fitted curves: these
/// outlines carry no staircase, so they are fitted closely.
pub fn tolerance(detail: f64) -> f64 {
    0.2 + (100.0 - detail.clamp(0.0, 100.0)) / 100.0 * 0.6
}

/// Catmull-Rom weights for the samples at -1, 0, 1, 2 round `t` in 0..1.
fn cubic(t: f32) -> [f32; 4] {
    let (t2, t3) = (t * t, t * t * t);
    [
        (-t3 + 2.0 * t2 - t) / 2.0,
        (3.0 * t3 - 5.0 * t2 + 2.0) / 2.0,
        (-3.0 * t3 + 4.0 * t2 + t) / 2.0,
        (t3 - t2) / 2.0,
    ]
}

/// A colour within 24 of pure black or white in every channel is taken
/// as pure (what a logo in black or white means).
fn snap(c: [u8; 3]) -> [u8; 3] {
    if c.iter().all(|v| *v <= 24) {
        [0, 0, 0]
    } else if c.iter().all(|v| *v >= 231) {
        [255, 255, 255]
    } else {
        c
    }
}

fn d2(a: [f32; 3], b: [f32; 3]) -> f32 {
    (0..3).map(|k| (a[k] - b[k]).powi(2)).sum()
}

/// The two colours of a two-tone picture (darker first), when nearly all
/// its opaque pixels are mixes of them.
pub fn two_tones(img: &RgbaImage) -> Option<([u8; 3], [u8; 3])> {
    let mut cache = std::collections::HashMap::<[u8; 3], [f32; 3]>::new();
    let lab: Vec<[f32; 3]> = img
        .pixels()
        .filter(|p| p[3] >= 128)
        .map(|p| {
            let c = [p[0], p[1], p[2]];
            *cache.entry(c).or_insert_with(|| oklab(c))
        })
        .collect();
    if lab.len() < 16 {
        return None;
    }
    // Two means, started from the darkest and the lightest pixel.
    let by_l = |a: &&[f32; 3], b: &&[f32; 3]| a[0].total_cmp(&b[0]);
    let mut c = [*lab.iter().min_by(by_l)?, *lab.iter().max_by(by_l)?];
    for _ in 0..12 {
        let mut s = [[0f64; 4]; 2];
        for p in &lab {
            let k = (d2(*p, c[1]) < d2(*p, c[0])) as usize;
            for ch in 0..3 {
                s[k][ch] += p[ch] as f64;
            }
            s[k][3] += 1.0;
        }
        for k in 0..2 {
            if s[k][3] > 0.0 {
                c[k] = [0, 1, 2].map(|ch| (s[k][ch] / s[k][3]) as f32);
            }
        }
    }
    if d2(c[0], c[1]) < 0.1 * 0.1 {
        return None;
    }
    // The line between them; nearly every pixel must lie on it.
    let ab = [0, 1, 2].map(|ch| c[1][ch] - c[0][ch]);
    let len2 = ab.iter().map(|v| v * v).sum::<f32>();
    let on_line = |p: [f32; 3]| {
        let t = ((0..3).map(|ch| (p[ch] - c[0][ch]) * ab[ch]).sum::<f32>() / len2).clamp(0.0, 1.0);
        let q = [0, 1, 2].map(|ch| c[0][ch] + ab[ch] * t);
        d2(p, q) <= TWO_TONE_NEAR * TWO_TONE_NEAR
    };
    let on = lab.iter().filter(|p| on_line(**p)).count();
    if (on as f64) < TWO_TONE_SHARE * lab.len() as f64 {
        return None;
    }
    // Compression leaves stray off colours along the edges; a third colour
    // is a patch of them. Pixels off the mix with nearly all neighbours
    // off too are inside such a patch.
    let (w, h) = (img.width() as usize, img.height() as usize);
    let off: Vec<bool> = img
        .pixels()
        .map(|p| p[3] >= 128 && !on_line(*cache.get(&[p[0], p[1], p[2]]).unwrap_or(&c[0])))
        .collect();
    let mut inside = 0;
    for y in 1..h.saturating_sub(1) {
        for x in 1..w.saturating_sub(1) {
            let i = y * w + x;
            if !off[i] {
                continue;
            }
            let n = [i - w - 1, i - w, i - w + 1, i - 1, i + 1, i + w - 1, i + w, i + w + 1]
                .iter()
                .filter(|j| off[**j])
                .count();
            if n >= 7 {
                inside += 1;
            }
        }
    }
    if inside > THIRD_COLOUR {
        return None;
    }
    let (a, b) = (from_oklab(c[0]), from_oklab(c[1]));
    Some(if c[0][0] <= c[1][0] { (a, b) } else { (b, a) })
}

/// Black and white mode: the tones are luma, drawn in pure black and
/// white.
pub fn outline_bw(img: &RgbaImage, s: &Settings, unit: f64) -> Vec<Traced> {
    let value: Vec<f32> = img.pixels().map(|p| crate::trace::luma(*p)).collect();
    outline_tones(img, &value, ([0, 0, 0], [255, 255, 255]), s, unit)
}

/// A two-tone picture in its own two colours: the tone of a pixel is how
/// far it lies from the darker colour towards the lighter one.
pub fn outline_two_tone(
    img: &RgbaImage,
    tones: ([u8; 3], [u8; 3]),
    s: &Settings,
    unit: f64,
) -> Vec<Traced> {
    let (a, b) = (oklab(tones.0), oklab(tones.1));
    let ab = [0, 1, 2].map(|ch| b[ch] - a[ch]);
    let len2 = ab.iter().map(|v| v * v).sum::<f32>().max(1e-9);
    let value: Vec<f32> = img
        .pixels()
        .map(|p| {
            let c = oklab([p[0], p[1], p[2]]);
            let t = (0..3).map(|ch| (c[ch] - a[ch]) * ab[ch]).sum::<f32>() / len2;
            t.clamp(0.0, 1.0) * 255.0
        })
        .collect();
    outline_tones(img, &value, (snap(tones.0), snap(tones.1)), s, unit)
}

/// Otsu threshold of `value` (0..255) over the opaque pixels.
fn otsu(value: &[f32], opaque: &[bool]) -> f32 {
    let mut hist = [0u64; 256];
    for (v, o) in value.iter().zip(opaque) {
        if *o {
            hist[(v.round().clamp(0.0, 255.0)) as usize] += 1;
        }
    }
    let total: u64 = hist.iter().sum();
    let sum: f64 = hist.iter().enumerate().map(|(i, c)| i as f64 * *c as f64).sum();
    let (mut w0, mut sum0) = (0u64, 0f64);
    let mut best = (0f64, None);
    for (t, c) in hist.iter().enumerate() {
        w0 += c;
        sum0 += t as f64 * *c as f64;
        let w1 = total - w0;
        if w0 == 0 || w1 == 0 {
            continue;
        }
        let m0 = sum0 / w0 as f64;
        let m1 = (sum - sum0) / w1 as f64;
        let between = w0 as f64 * w1 as f64 * (m0 - m1).powi(2);
        if between > best.0 {
            best = (between, Some(t));
        }
    }
    best.1.map_or(128.0, |t| t as f32 + 0.5)
}

/// Trace the ink of a picture whose pixels have a tone `value` (0 dark ..
/// 255 light) between the colours `tones` (dark, light).
fn outline_tones(
    img: &RgbaImage,
    value: &[f32],
    tones: ([u8; 3], [u8; 3]),
    s: &Settings,
    unit: f64,
) -> Vec<Traced> {
    let (w, h) = (img.width() as usize, img.height() as usize);
    if w == 0 || h == 0 || value.len() != w * h {
        return Vec::new();
    }
    let opaque: Vec<bool> = img.pixels().map(|p| p[3] >= 128).collect();
    // Paper and ink tones: the means of the two classes.
    let t = otsu(value, &opaque);
    let (mut s0, mut n0, mut s1, mut n1) = (0f64, 0usize, 0f64, 0usize);
    for (l, o) in value.iter().zip(&opaque) {
        if !o {
            continue;
        }
        if *l < t {
            s0 += *l as f64;
            n0 += 1;
        } else {
            s1 += *l as f64;
            n1 += 1;
        }
    }
    if n0 == 0 || n1 == 0 {
        return Vec::new();
    }
    let (dark, light) = ((s0 / n0 as f64) as f32, (s1 / n1 as f64) as f32);
    if light - dark < 8.0 {
        return Vec::new();
    }
    // The ink is the tone the border is not (light lines on dark work).
    let (mut bd, mut bl) = (0usize, 0usize);
    for y in 0..h {
        for x in 0..w {
            if x != 0 && y != 0 && x + 1 != w && y + 1 != h {
                continue;
            }
            let i = y * w + x;
            if opaque[i] {
                if value[i] < t {
                    bd += 1;
                } else {
                    bl += 1;
                }
            }
        }
    }
    let ink_dark = bl >= bd;
    let mut f: Vec<f32> = value
        .iter()
        .zip(&opaque)
        .map(|(l, o)| {
            if !o {
                return 0.0;
            }
            let v = if ink_dark {
                (light - l) / (light - dark)
            } else {
                (l - dark) / (light - dark)
            };
            v.clamp(0.0, 1.0)
        })
        .collect();
    // Specks of ink and pinholes of paper, measured in the pixels the
    // user sees.
    let speck = ((crate::trace::speck_area(
        s.detail,
        (w as f64 * unit) as usize,
        (h as f64 * unit) as usize,
    ) as f64
        / (unit * unit))
        .round() as usize)
        .max(1);
    drop_small(&mut f, w, h, speck);
    // A picture without anti-aliasing has only steps at its edges; softened
    // a little, its edges run through the middle of the steps instead.
    if hard_edged(&f, w, h) {
        f = soften(&f, w, h);
    }
    // The ink's box, with a margin for the interpolation.
    let mut bb: Option<(usize, usize, usize, usize)> = None;
    for (i, v) in f.iter().enumerate() {
        if *v >= 0.5 {
            let (x, y) = (i % w, i / w);
            bb = Some(match bb {
                None => (x, y, x, y),
                Some((a, b, c, d)) => (a.min(x), b.min(y), c.max(x), d.max(y)),
            });
        }
    }
    let Some((bx0, by0, bx1, by1)) = bb else {
        return Vec::new();
    };
    let (x0, y0) = (bx0.saturating_sub(3), by0.saturating_sub(3));
    let (x1, y1) = ((bx1 + 4).min(w), (by1 + 4).min(h));
    let k = std::env::var("TRACE_K")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or_else(|| fine_factor(x1 - x0, y1 - y0));
    let fine = iso_mask(&f, w, h, (x0, y0, x1, y1), k);
    let (fw, fh) = ((x1 - x0) * k + 2, (y1 - y0) * k + 2);
    let mut fp = crate::trace::fit_params(s);
    fp.scale = k;
    fp.tolerance = tolerance(s.detail);
    if let Some(t) = std::env::var("TRACE_TOL").ok().and_then(|v| v.parse().ok()) {
        fp.tolerance = t;
    }
    if let Some(p) = std::env::var("TRACE_PASSES").ok().and_then(|v| v.parse().ok()) {
        fp.smooth_passes = p;
    }
    let kf = k as f64;
    let mut rings: Vec<BezPath> = Vec::new();
    for ring in crate::trace::contours(&fine, fw, fh) {
        let pts: Vec<Point> = crate::trace::ring_midpoints(&ring)
            .into_iter()
            .map(|p| {
                Point::new(
                    ((p.x - 1.0) / kf + x0 as f64).clamp(0.0, w as f64),
                    ((p.y - 1.0) / kf + y0 as f64).clamp(0.0, h as f64),
                )
            })
            .collect();
        let sub = crate::trace_fit::fit_closed(&pts, &fp);
        if sub.area().abs() >= 0.5 {
            rings.push(sub);
        }
    }
    let pieces = pieces(rings);
    if pieces.is_empty() {
        return Vec::new();
    }
    let rgb = |c: [u8; 3]| Color::rgb8(c[0], c[1], c[2]);
    let (ink, paper) = if ink_dark {
        (rgb(tones.0), rgb(tones.1))
    } else {
        (rgb(tones.1), rgb(tones.0))
    };
    let mut out = Vec::new();
    if !s.remove_background {
        // The paper as a rectangle under the ink.
        let mut rect = BezPath::new();
        rect.move_to((0.0, 0.0));
        rect.line_to((w as f64, 0.0));
        rect.line_to((w as f64, h as f64));
        rect.line_to((0.0, h as f64));
        rect.close_path();
        out.push(Traced {
            path: rect,
            color: paper,
            closed: true,
            width: 0.0,
            ramp: None,
        });
    }
    for path in pieces {
        out.push(Traced {
            path,
            color: ink,
            closed: true,
            width: 0.0,
            ramp: None,
        });
    }
    out
}

/// Group rings into pieces: each outer ring with the holes directly in
/// it (an island inside a hole is a piece of its own). Outer rings turn
/// like the largest ring and holes the other way; largest pieces first.
fn pieces(rings: Vec<BezPath>) -> Vec<BezPath> {
    let areas: Vec<f64> = rings.iter().map(|r| r.area()).collect();
    let outer_sign = areas
        .iter()
        .copied()
        .max_by(|a, b| a.abs().total_cmp(&b.abs()))
        .map_or(1.0, f64::signum);
    let outers: Vec<usize> = (0..rings.len())
        .filter(|i| areas[*i].signum() == outer_sign)
        .collect();
    let mut groups: Vec<BezPath> = outers.iter().map(|i| rings[*i].clone()).collect();
    for (i, ring) in rings.iter().enumerate() {
        if areas[i].signum() == outer_sign {
            continue;
        }
        let Some(p) = ring.elements().iter().find_map(|e| match e {
            graphdraw_core::geometry::PathEl::MoveTo(p) => Some(*p),
            _ => None,
        }) else {
            continue;
        };
        // The smallest outer ring round the hole.
        let host = outers
            .iter()
            .enumerate()
            .filter(|(_, o)| rings[**o].winding(p) != 0)
            .min_by(|a, b| areas[*a.1].abs().total_cmp(&areas[*b.1].abs()))
            .map(|(g, _)| g);
        match host {
            Some(g) => groups[g].extend(ring.clone()),
            None => groups.push(ring.clone()),
        }
    }
    groups.sort_by(|a, b| b.area().abs().total_cmp(&a.area().abs()));
    groups
}

/// `src` (`w` x `h`) interpolated (Catmull-Rom, its edge held beyond
/// it) at the centres of `k` x `k` cells per pixel over the crop
/// `(x0, y0, x1, y1)`.
fn upsample(src: &[f32], w: usize, h: usize, (x0, y0, x1, y1): (usize, usize, usize, usize), k: usize) -> Vec<f32> {
    let (fw, fh) = ((x1 - x0) * k, (y1 - y0) * k);
    let at = |x: i64, y: i64| -> f32 {
        let x = x.clamp(0, w as i64 - 1) as usize;
        let y = y.clamp(0, h as i64 - 1) as usize;
        src[y * w + x]
    };
    // Weights and first sample per fine column and row.
    let taps = |n: usize, o: usize| -> Vec<(i64, [f32; 4])> {
        (0..n)
            .map(|c| {
                // Cell centre in pixel-centre coordinates.
                let u = o as f32 + (c as f32 + 0.5) / k as f32 - 0.5;
                let base = u.floor();
                (base as i64 - 1, cubic(u - base))
            })
            .collect()
    };
    let cols = taps(fw, x0);
    let rows = taps(fh, y0);
    // Columns of the crop (plus two each side) interpolated down each
    // fine row, then across.
    let (lo, hi) = (x0 as i64 - 2, x1 as i64 + 2);
    let span = (hi - lo) as usize;
    let mut out = vec![0f32; fw * fh];
    let mut row = vec![0f32; span];
    for (r, (ry, wy)) in rows.iter().enumerate() {
        for (j, v) in row.iter_mut().enumerate() {
            let x = lo + j as i64;
            *v = (0..4).map(|q| wy[q] * at(x, ry + q as i64)).sum();
        }
        for (c, (cx, wx)) in cols.iter().enumerate() {
            out[r * fw + c] = (0..4)
                .map(|q| {
                    let j = (cx + q as i64 - lo).clamp(0, span as i64 - 1) as usize;
                    wx[q] * row[j]
                })
                .sum();
        }
    }
    out
}

/// The cells of the crop `(x0, y0, x1, y1)` of `f`, `k` per pixel each
/// way, where the amount of ink interpolated smoothly is at least one
/// half, with an empty frame of one cell round them.
fn iso_mask(
    f: &[f32],
    w: usize,
    h: usize,
    crop: (usize, usize, usize, usize),
    k: usize,
) -> Vec<bool> {
    let (x0, y0, x1, y1) = crop;
    let (cw, ch) = (x1 - x0, y1 - y0);
    let (fw, fh) = (cw * k, ch * k);
    let v = upsample(f, w, h, crop, k);
    let mut out = vec![false; (fw + 2) * (fh + 2)];
    for r in 0..fh {
        for c in 0..fw {
            out[(r + 1) * (fw + 2) + c + 1] = v[r * fw + c] >= 0.5;
        }
    }
    out
}

/// Below this share of mixed pixels on its edges a picture is taken as
/// drawn without anti-aliasing.
const MIXED_EDGES: f64 = 0.25;

/// Whether the pixels on the edges of the ink (next to a pixel on the
/// other side of one half) are nearly all wholly ink or paper.
fn hard_edged(f: &[f32], w: usize, h: usize) -> bool {
    let (mut edge, mut mixed) = (0usize, 0usize);
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let me = f[i] >= 0.5;
            let other = [
                (x > 0).then(|| i - 1),
                (x + 1 < w).then(|| i + 1),
                (y > 0).then(|| i - w),
                (y + 1 < h).then(|| i + w),
            ]
            .into_iter()
            .flatten()
            .any(|j| (f[j] >= 0.5) != me);
            if other {
                edge += 1;
                if f[i] > 0.1 && f[i] < 0.9 {
                    mixed += 1;
                }
            }
        }
    }
    edge > 0 && (mixed as f64) < MIXED_EDGES * edge as f64
}

/// A Gaussian blur of about 0.7 pixels (weights 1 3 1 each way, the
/// picture's edge held beyond it).
fn soften(f: &[f32], w: usize, h: usize) -> Vec<f32> {
    let kern = [0.2f32, 0.6, 0.2];
    let at = |v: &[f32], x: i64, y: i64| -> f32 {
        v[y.clamp(0, h as i64 - 1) as usize * w + x.clamp(0, w as i64 - 1) as usize]
    };
    let mut across = vec![0f32; w * h];
    for y in 0..h as i64 {
        for x in 0..w as i64 {
            across[y as usize * w + x as usize] =
                (0..3).map(|q| kern[q] * at(f, x + q as i64 - 1, y)).sum();
        }
    }
    let mut out = vec![0f32; w * h];
    for y in 0..h as i64 {
        for x in 0..w as i64 {
            out[y as usize * w + x as usize] =
                (0..3).map(|q| kern[q] * at(&across, x, y + q as i64 - 1)).sum();
        }
    }
    out
}

/// Ink specks (8-connected) and paper pinholes (4-connected, not on the
/// picture's border) smaller than `min_area` pixels are cleared or
/// filled in the amount of ink.
fn drop_small(f: &mut [f32], w: usize, h: usize, min_area: usize) {
    if min_area <= 1 {
        return;
    }
    for ink in [true, false] {
        let set: Vec<bool> = f.iter().map(|v| (*v >= 0.5) == ink).collect();
        let mut seen = vec![false; w * h];
        let mut stack = Vec::new();
        let mut comp = Vec::new();
        for start in 0..w * h {
            if !set[start] || seen[start] {
                continue;
            }
            seen[start] = true;
            stack.push(start);
            comp.clear();
            let mut border = false;
            while let Some(i) = stack.pop() {
                comp.push(i);
                let (x, y) = ((i % w) as i64, (i / w) as i64);
                if x == 0 || y == 0 || x + 1 == w as i64 || y + 1 == h as i64 {
                    border = true;
                }
                for dy in -1i64..=1 {
                    for dx in -1i64..=1 {
                        if (dx == 0 && dy == 0) || (!ink && dx != 0 && dy != 0) {
                            continue;
                        }
                        let (nx, ny) = (x + dx, y + dy);
                        if nx < 0 || ny < 0 || nx >= w as i64 || ny >= h as i64 {
                            continue;
                        }
                        let j = ny as usize * w + nx as usize;
                        if set[j] && !seen[j] {
                            seen[j] = true;
                            stack.push(j);
                        }
                    }
                }
            }
            if comp.len() < min_area && (ink || !border) {
                for &i in &comp {
                    f[i] = if ink { 0.0 } else { 1.0 };
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trace::Preset;
    use image::Rgba;

    /// Black rectangle `(x0, y0, x1, y1)` on white, each pixel grey by
    /// the exact share of it the rectangle covers.
    fn exact_rect(w: u32, h: u32, r: (f64, f64, f64, f64)) -> RgbaImage {
        RgbaImage::from_fn(w, h, |x, y| {
            let ox = ((x as f64 + 1.0).min(r.2) - (x as f64).max(r.0)).max(0.0);
            let oy = ((y as f64 + 1.0).min(r.3) - (y as f64).max(r.1)).max(0.0);
            let v = (255.0 * (1.0 - ox * oy)).round() as u8;
            Rgba([v, v, v, 255])
        })
    }

    /// A soft-edged black disc on white, with a red dot of radius `dot`.
    fn disc(dot: f32) -> RgbaImage {
        RgbaImage::from_fn(80, 80, |x, y| {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            if ((fx - 70.0).powi(2) + (fy - 10.0).powi(2)).sqrt() < dot {
                return Rgba([220, 20, 20, 255]);
            }
            let d = ((fx - 40.0).powi(2) + (fy - 40.0).powi(2)).sqrt();
            let v = (255.0 * (d - 25.0 + 0.5).clamp(0.0, 1.0)) as u8;
            Rgba([v, v, v, 255])
        })
    }

    #[test]
    fn two_tones_are_found_and_a_third_colour_is_not_lost() {
        let t = two_tones(&disc(0.0)).expect("two tones");
        assert!(t.0.iter().all(|v| *v < 30) && t.1.iter().all(|v| *v > 225), "{t:?}");
        assert_eq!(two_tones(&disc(4.0)), None, "a red dot is a third colour");
        // Traced in colour, the disc is one pure black shape.
        let out = crate::trace::trace(&disc(0.0), &Preset::Clipart.settings());
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].color.to_rgb8(), [0, 0, 0]);
        let area = out[0].path.area().abs();
        let want = std::f64::consts::PI * 25.0 * 25.0;
        assert!((area - want).abs() / want < 0.01, "{area} vs {want}");
    }

    #[test]
    fn edges_fall_where_the_picture_put_them() {
        let r = (10.25, 8.5, 30.75, 20.4);
        let img = exact_rect(44, 30, r);
        assert_eq!(crate::trace_grid::detect(&img), None, "not blocky");
        let t = outline_bw(&img, &Preset::LineArt.settings(), 1.0);
        assert_eq!(t.len(), 1);
        let bb = t[0].path.bounding_box();
        for (got, want) in [(bb.x0, r.0), (bb.y0, r.1), (bb.x1, r.2), (bb.y1, r.3)] {
            // Interpolating the share of a pixel an edge covers puts the
            // edge up to an eighth of a pixel off.
            assert!((got - want).abs() < 0.15, "{bb:?} vs {r:?}");
        }
    }
}

