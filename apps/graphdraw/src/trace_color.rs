//! Colour side of the bitmap tracer: an edge-preserving filter, palette
//! reduction in the Oklab perceptual space, and the clean-up of the label
//! map (specks and anti-aliasing fringes merged into their neighbours).
//! See docs/behavior/bitmap-tracing.md.

use image::{Rgba, RgbaImage};

/// Label of a transparent pixel in a label map.
pub const CLEAR: u32 = u32::MAX;

fn to_linear(c: u8) -> f32 {
    let c = c as f32 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn from_linear(c: f32) -> u8 {
    let c = c.clamp(0.0, 1.0);
    let s = if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (s * 255.0).round().clamp(0.0, 255.0) as u8
}

/// sRGB to Oklab (L about 0..1, a and b about -0.4..0.4).
pub fn oklab(c: [u8; 3]) -> [f32; 3] {
    let (r, g, b) = (to_linear(c[0]), to_linear(c[1]), to_linear(c[2]));
    let l = (0.412_221_46 * r + 0.536_332_55 * g + 0.051_445_995 * b).cbrt();
    let m = (0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b).cbrt();
    let s = (0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b).cbrt();
    [
        0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s,
        1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s,
        0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s,
    ]
}

/// Oklab back to sRGB, clamped into gamut.
pub fn from_oklab(c: [f32; 3]) -> [u8; 3] {
    let l = c[0] + 0.396_337_78 * c[1] + 0.215_803_76 * c[2];
    let m = c[0] - 0.105_561_346 * c[1] - 0.063_854_17 * c[2];
    let s = c[0] - 0.089_484_18 * c[1] - 1.291_485_5 * c[2];
    let (l, m, s) = (l * l * l, m * m * m, s * s * s);
    [
        from_linear(4.076_741_7 * l - 3.307_711_6 * m + 0.230_969_94 * s),
        from_linear(-1.268_438 * l + 2.609_757_4 * m - 0.341_319_38 * s),
        from_linear(-0.004_196_086_3 * l - 0.703_418_6 * m + 1.707_614_7 * s),
    ]
}

fn dist2(a: [f32; 3], b: [f32; 3]) -> f32 {
    (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)
}

/// Edge-preserving smoothing: each pixel becomes the weighted mean of its
/// neighbours within `radius`, weighted by distance and by how close their
/// colour is (`sigma` in 0..255 units). Edges stay sharp, flat areas lose
/// grain and compression noise.
pub fn bilateral(img: &RgbaImage, radius: u32, sigma: f32) -> RgbaImage {
    let (w, h) = img.dimensions();
    if radius == 0 || w == 0 || h == 0 {
        return img.clone();
    }
    let r = radius as i64;
    let s2 = 2.0 * (r as f32 / 2.0 + 0.5).powi(2);
    let c2 = 2.0 * sigma.max(1.0).powi(2);
    let mut out = RgbaImage::new(w, h);
    for y in 0..h as i64 {
        for x in 0..w as i64 {
            let c = img.get_pixel(x as u32, y as u32);
            if c[3] < 128 {
                out.put_pixel(x as u32, y as u32, *c);
                continue;
            }
            let mut acc = [0f32; 3];
            let mut wsum = 0f32;
            for dy in -r..=r {
                let yy = y + dy;
                if yy < 0 || yy >= h as i64 {
                    continue;
                }
                for dx in -r..=r {
                    let xx = x + dx;
                    if xx < 0 || xx >= w as i64 {
                        continue;
                    }
                    let q = img.get_pixel(xx as u32, yy as u32);
                    if q[3] < 128 {
                        continue;
                    }
                    let dc = (0..3)
                        .map(|k| (q[k] as f32 - c[k] as f32).powi(2))
                        .sum::<f32>();
                    let wt = (-((dx * dx + dy * dy) as f32) / s2 - dc / c2).exp();
                    for k in 0..3 {
                        acc[k] += q[k] as f32 * wt;
                    }
                    wsum += wt;
                }
            }
            let px = if wsum > 0.0 {
                Rgba([
                    (acc[0] / wsum).round() as u8,
                    (acc[1] / wsum).round() as u8,
                    (acc[2] / wsum).round() as u8,
                    c[3],
                ])
            } else {
                *c
            };
            out.put_pixel(x as u32, y as u32, px);
        }
    }
    out
}

/// A palette of at most `n` colours for the opaque pixels of `img`:
/// median cut in Oklab, refined by a few rounds of k-means.
pub fn palette(img: &RgbaImage, n: usize) -> Vec<[u8; 3]> {
    let mut pixels: Vec<[f32; 3]> = Vec::new();
    let mut cache = std::collections::HashMap::<[u8; 3], [f32; 3]>::new();
    let total = img.pixels().filter(|p| p[3] >= 128).count();
    let step = total / 200_000 + 1;
    for p in img.pixels().filter(|p| p[3] >= 128).step_by(step) {
        let c = [p[0], p[1], p[2]];
        pixels.push(*cache.entry(c).or_insert_with(|| oklab(c)));
    }
    if pixels.is_empty() {
        return vec![[0, 0, 0]];
    }
    let n = n.max(1);
    let mut buckets: Vec<Vec<[f32; 3]>> = vec![pixels.clone()];
    while buckets.len() < n {
        // Split the bucket whose widest channel is widest overall.
        let mut best: Option<(usize, usize, f32)> = None;
        for (bi, b) in buckets.iter().enumerate() {
            if b.len() < 2 {
                continue;
            }
            for ch in 0..3 {
                let (mn, mx) = b.iter().fold((f32::MAX, f32::MIN), |(mn, mx), p| {
                    (mn.min(p[ch]), mx.max(p[ch]))
                });
                // Lightness counts double: it is what outlines shapes.
                let r = (mx - mn) * if ch == 0 { 2.0 } else { 1.0 };
                if best.is_none_or(|b| r > b.2) {
                    best = Some((bi, ch, r));
                }
            }
        }
        let Some((bi, ch, r)) = best else { break };
        if r <= 1e-6 {
            break;
        }
        let mut b = buckets.swap_remove(bi);
        b.sort_by(|p, q| p[ch].total_cmp(&q[ch]));
        // Cut at the median, but never through a run of equal values: a
        // picture of mostly black and white would otherwise put both in
        // one bucket and average them to grey.
        let v = b[b.len() / 2][ch];
        let mut mid = b.partition_point(|p| p[ch] < v);
        if mid == 0 {
            mid = b.partition_point(|p| p[ch] <= v);
        }
        if mid == 0 || mid == b.len() {
            buckets.push(b);
            break;
        }
        let hi = b.split_off(mid);
        buckets.push(b);
        buckets.push(hi);
    }
    let mut centres: Vec<[f32; 3]> = buckets
        .iter()
        .filter(|b| !b.is_empty())
        .map(|b| {
            let s = b.iter().fold([0f64; 3], |a, p| {
                [a[0] + p[0] as f64, a[1] + p[1] as f64, a[2] + p[2] as f64]
            });
            let k = b.len() as f64;
            [(s[0] / k) as f32, (s[1] / k) as f32, (s[2] / k) as f32]
        })
        .collect();
    for _ in 0..8 {
        let mut sums = vec![[0f64; 4]; centres.len()];
        for p in &pixels {
            let i = nearest(&centres, *p);
            for k in 0..3 {
                sums[i][k] += p[k] as f64;
            }
            sums[i][3] += 1.0;
        }
        let mut moved = 0f32;
        for (c, s) in centres.iter_mut().zip(&sums) {
            if s[3] == 0.0 {
                continue;
            }
            let m = [
                (s[0] / s[3]) as f32,
                (s[1] / s[3]) as f32,
                (s[2] / s[3]) as f32,
            ];
            moved = moved.max(dist2(m, *c));
            *c = m;
        }
        if moved < 1e-8 {
            break;
        }
    }
    let mut out: Vec<[u8; 3]> = centres.into_iter().map(from_oklab).collect();
    out.sort_unstable();
    out.dedup();
    out
}

fn nearest(centres: &[[f32; 3]], c: [f32; 3]) -> usize {
    let mut best = (f32::MAX, 0);
    for (i, p) in centres.iter().enumerate() {
        let d = dist2(*p, c);
        if d < best.0 {
            best = (d, i);
        }
    }
    best.1
}

/// Label every pixel with its nearest palette colour (in Oklab), or
/// [`CLEAR`] when it is transparent.
pub fn label(img: &RgbaImage, palette: &[[u8; 3]]) -> Vec<u32> {
    let lab: Vec<[f32; 3]> = palette.iter().map(|c| oklab(*c)).collect();
    let mut cache = std::collections::HashMap::<[u8; 3], u32>::new();
    img.pixels()
        .map(|p| {
            if p[3] < 128 {
                return CLEAR;
            }
            let c = [p[0], p[1], p[2]];
            *cache
                .entry(c)
                .or_insert_with(|| nearest(&lab, oklab(c)) as u32)
        })
        .collect()
}

/// The background of a picture with a plain or gently shaded one: the
/// pixels reached from the border through small colour steps (so a
/// gradient is followed but an edge is not), together with its colour.
/// `None` when the border has no dominant colour.
pub fn background(img: &RgbaImage) -> Option<(Vec<bool>, [u8; 3])> {
    let (w, h) = (img.width() as usize, img.height() as usize);
    if w < 3 || h < 3 {
        return None;
    }
    let lab: Vec<Option<[f32; 3]>> = img
        .pixels()
        .map(|p| (p[3] >= 128).then(|| oklab([p[0], p[1], p[2]])))
        .collect();
    let on_border = |i: usize| {
        let (x, y) = (i % w, i / w);
        x == 0 || y == 0 || x + 1 == w || y + 1 == h
    };
    let border: Vec<usize> = (0..w * h).filter(|i| on_border(*i)).collect();
    let opaque: Vec<[f32; 3]> = border.iter().filter_map(|i| lab[*i]).collect();
    if opaque.len() * 2 < border.len() {
        // Mostly transparent already: nothing to find.
        return None;
    }
    let mut seed = [0f32; 3];
    for (k, s) in seed.iter_mut().enumerate() {
        let mut v: Vec<f32> = opaque.iter().map(|c| c[k]).collect();
        v.sort_by(f32::total_cmp);
        *s = v[v.len() / 2];
    }
    const NEAR: f32 = 0.12;
    let near = opaque.iter().filter(|c| dist2(**c, seed) <= NEAR * NEAR).count();
    if near * 10 < opaque.len() * 6 {
        return None;
    }
    const STEP: f32 = 0.035;
    const FAR: f32 = 0.25;
    let mut mask = vec![false; w * h];
    let mut stack: Vec<usize> = border
        .iter()
        .copied()
        .filter(|i| lab[*i].is_some_and(|c| dist2(c, seed) <= NEAR * NEAR))
        .collect();
    for &i in &stack {
        mask[i] = true;
    }
    grow(&mut stack, &mut mask, &lab, seed, w, h, STEP, FAR);
    // Enclosed pieces of ground (the counters of letters) are ground too
    // when they are very close to its colour.
    const ENCLOSED: f32 = 0.05;
    let mut stack: Vec<usize> = (0..w * h)
        .filter(|i| !mask[*i] && lab[*i].is_some_and(|c| dist2(c, seed) <= ENCLOSED * ENCLOSED))
        .collect();
    for &i in &stack {
        mask[i] = true;
    }
    grow(&mut stack, &mut mask, &lab, seed, w, h, STEP, NEAR);
    Some((mask, from_oklab(seed)))
}

/// Flood `mask` from the pixels on `stack` to 4-neighbours within `step`
/// of the pixel they come from and within `far` of `seed`.
#[allow(clippy::too_many_arguments)]
fn grow(
    stack: &mut Vec<usize>,
    mask: &mut [bool],
    lab: &[Option<[f32; 3]>],
    seed: [f32; 3],
    w: usize,
    h: usize,
    step: f32,
    far: f32,
) {
    while let Some(i) = stack.pop() {
        let Some(c) = lab[i] else { continue };
        let (x, y) = (i % w, i / w);
        let ns = [
            (x > 0).then(|| i - 1),
            (x + 1 < w).then(|| i + 1),
            (y > 0).then(|| i - w),
            (y + 1 < h).then(|| i + w),
        ];
        for j in ns.into_iter().flatten() {
            if mask[j] {
                continue;
            }
            if let Some(q) = lab[j] {
                if dist2(q, c) <= step * step && dist2(q, seed) <= far * far {
                    mask[j] = true;
                    stack.push(j);
                }
            }
        }
    }
}

/// Luma threshold (0..255) that best splits the opaque pixels into dark
/// and light: the one with the largest between-class variance. A picture
/// with one tone gets 128.
pub fn otsu(img: &RgbaImage) -> f32 {
    let mut hist = [0u64; 256];
    for p in img.pixels().filter(|p| p[3] >= 128) {
        let l = 0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32;
        hist[(l.round() as usize).min(255)] += 1;
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
    // Dark is strictly below the threshold, so split just above `t`.
    best.1.map_or(128.0, |t| t as f32 + 0.5)
}

/// One 4-connected run of pixels with the same label.
struct Region {
    label: u32,
    area: usize,
    /// Pixel edges on its boundary (image border included).
    perimeter: usize,
    /// Shared edge lengths with neighbouring labels.
    borders: Vec<(u32, usize)>,
}

/// Connected regions of `labels` and, per pixel, the region it is in.
fn regions(labels: &[u32], w: usize, h: usize) -> (Vec<Region>, Vec<u32>) {
    let mut id = vec![u32::MAX; labels.len()];
    let mut out: Vec<Region> = Vec::new();
    let mut stack = Vec::new();
    for start in 0..labels.len() {
        if id[start] != u32::MAX {
            continue;
        }
        let rid = out.len() as u32;
        let l = labels[start];
        let mut r = Region {
            label: l,
            area: 0,
            perimeter: 0,
            borders: Vec::new(),
        };
        id[start] = rid;
        stack.push(start);
        while let Some(i) = stack.pop() {
            r.area += 1;
            let (x, y) = (i % w, i / w);
            let ns = [
                (x > 0).then(|| i - 1),
                (x + 1 < w).then(|| i + 1),
                (y > 0).then(|| i - w),
                (y + 1 < h).then(|| i + w),
            ];
            for nb in ns {
                let Some(j) = nb else {
                    r.perimeter += 1;
                    continue;
                };
                if labels[j] == l {
                    if id[j] == u32::MAX {
                        id[j] = rid;
                        stack.push(j);
                    }
                } else {
                    r.perimeter += 1;
                    match r.borders.iter_mut().find(|b| b.0 == labels[j]) {
                        Some(b) => b.1 += 1,
                        None => r.borders.push((labels[j], 1)),
                    }
                }
            }
        }
        out.push(r);
    }
    (out, id)
}

/// Is `c` a mix of `a` and `b` (as an anti-aliased edge between them
/// would be)? Returns how far along from `a` it lies.
fn blend_of(c: [f32; 3], a: [f32; 3], b: [f32; 3]) -> Option<f32> {
    let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let len2 = ab[0] * ab[0] + ab[1] * ab[1] + ab[2] * ab[2];
    if len2 < 1e-6 {
        return None;
    }
    let ac = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
    let t = (ac[0] * ab[0] + ac[1] * ab[1] + ac[2] * ab[2]) / len2;
    if !(0.05..=0.95).contains(&t) {
        return None;
    }
    let p = [a[0] + ab[0] * t, a[1] + ab[1] * t, a[2] + ab[2] * t];
    // Off the line by little compared with the gap it bridges.
    (dist2(c, p) <= (0.03f32).powi(2).max(len2 * 0.02)).then_some(t)
}

/// Merge specks smaller than `min_area` pixels and thin anti-aliasing
/// fringes into the neighbour they belong to. `fringes` turns the second
/// off (black and white, where there are no mixed colours). `clear` is
/// the colour transparent pixels had (a removed background), so fringes
/// against it can join it.
pub fn clean(
    labels: &mut [u32],
    w: usize,
    h: usize,
    palette: &[[u8; 3]],
    min_area: usize,
    fringes: bool,
    clear: Option<[u8; 3]>,
) {
    let lab: Vec<[f32; 3]> = palette.iter().map(|c| oklab(*c)).collect();
    let clear = clear.map(oklab);
    let colour = |l: u32| {
        if l == CLEAR {
            clear
        } else {
            lab.get(l as usize).copied()
        }
    };
    for _ in 0..4 {
        let (regs, id) = regions(labels, w, h);
        let mut target: Vec<Option<u32>> = vec![None; regs.len()];
        let mut any = false;
        for (ri, r) in regs.iter().enumerate() {
            if r.borders.is_empty() {
                continue;
            }
            let speck = r.area < min_area;
            // About one pixel thick: twice the area over the perimeter.
            let thin = fringes && r.label != CLEAR && 2 * r.area < 3 * r.perimeter / 2;
            if !speck && !thin {
                continue;
            }
            let longest = r
                .borders
                .iter()
                .max_by_key(|b| b.1)
                .map(|b| b.0)
                .unwrap_or(r.label);
            if speck {
                target[ri] = Some(longest);
                any = true;
                continue;
            }
            // A fringe: its colour lies between two neighbours; it joins
            // the one it is closer to.
            let Some(c) = colour(r.label) else { continue };
            let mut best: Option<(f32, u32)> = None;
            for (i, a) in r.borders.iter().enumerate() {
                for b in &r.borders[i + 1..] {
                    let (Some(ca), Some(cb)) = (colour(a.0), colour(b.0)) else {
                        continue;
                    };
                    if let Some(t) = blend_of(c, ca, cb) {
                        let pick = if t < 0.5 { a.0 } else { b.0 };
                        let score = (t - 0.5).abs();
                        if best.is_none_or(|bb| score > bb.0) {
                            best = Some((score, pick));
                        }
                    }
                }
            }
            if let Some((_, pick)) = best {
                target[ri] = Some(pick);
                any = true;
            }
        }
        if !any {
            break;
        }
        for (l, r) in labels.iter_mut().zip(&id) {
            if let Some(Some(t)) = target.get(*r as usize) {
                *l = *t;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oklab_round_trips() {
        for c in [[0, 0, 0], [255, 255, 255], [255, 0, 0], [12, 200, 77], [128, 128, 128]] {
            let back = from_oklab(oklab(c));
            for k in 0..3 {
                assert!((back[k] as i32 - c[k] as i32).abs() <= 1, "{c:?} {back:?}");
            }
        }
        assert!((oklab([255, 255, 255])[0] - 1.0).abs() < 1e-3);
    }

    #[test]
    fn palette_finds_the_real_colours() {
        let img = RgbaImage::from_fn(60, 20, |x, _| match x / 20 {
            0 => Rgba([200, 30, 30, 255]),
            1 => Rgba([30, 200, 30, 255]),
            _ => Rgba([30, 30, 200, 255]),
        });
        let pal = palette(&img, 3);
        assert_eq!(pal.len(), 3, "{pal:?}");
        for c in [[200, 30, 30], [30, 200, 30], [30, 30, 200]] {
            assert!(
                pal.iter()
                    .any(|p| (0..3).all(|k| (p[k] as i32 - c[k]).abs() <= 2)),
                "{c:?} in {pal:?}"
            );
        }
        assert_eq!(palette(&RgbaImage::new(3, 3), 4), vec![[0, 0, 0]]);
    }

    #[test]
    fn specks_and_fringes_merge_into_neighbours() {
        // Black square on white with a grey one-pixel fringe and a stray
        // grey speck.
        let (w, h) = (30, 30);
        let pal = [[0, 0, 0], [128, 128, 128], [255, 255, 255]];
        let mut labels = vec![2u32; w * h];
        for y in 9..21 {
            for x in 9..21 {
                labels[y * w + x] = 1;
            }
        }
        for y in 10..20 {
            for x in 10..20 {
                labels[y * w + x] = 0;
            }
        }
        labels[3 * w + 3] = 1;
        clean(&mut labels, w, h, &pal, 4, true, None);
        assert!(!labels.contains(&1), "grey left");
        assert_eq!(labels.iter().filter(|l| **l == 0).count() % 100, 0);
    }

    #[test]
    fn bilateral_keeps_edges() {
        let img = RgbaImage::from_fn(20, 20, |x, _| {
            if x < 10 {
                Rgba([0, 0, 0, 255])
            } else {
                Rgba([255, 255, 255, 255])
            }
        });
        let out = bilateral(&img, 2, 30.0);
        assert_eq!(out.get_pixel(9, 5)[0], 0);
        assert_eq!(out.get_pixel(10, 5)[0], 255);
    }
}
