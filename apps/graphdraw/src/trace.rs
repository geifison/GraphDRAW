//! Bitmap to vector tracing: outline mode (colour regions become filled
//! curves) and centreline mode (strokes become open curves). See
//! docs/behavior/bitmap-tracing.md for the algorithm and defaults.

use image::{Rgba, RgbaImage};
use graphdraw_core::{
    geometry::{BezPath, Point, Rect, Shape as _},
    Color,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Preset {
    // Centreline
    Technical,
    LineDrawing,
    // Outline
    LineArt,
    Logo,
    DetailedLogo,
    Clipart,
    LowQualityImage,
    HighQualityImage,
}

impl Preset {
    /// Every preset, centreline ones first.
    pub const ALL: [Preset; 8] = [
        Preset::Technical,
        Preset::LineDrawing,
        Preset::LineArt,
        Preset::Logo,
        Preset::DetailedLogo,
        Preset::Clipart,
        Preset::LowQualityImage,
        Preset::HighQualityImage,
    ];

    pub fn is_centerline(self) -> bool {
        matches!(self, Preset::Technical | Preset::LineDrawing)
    }
    pub fn key(self) -> &'static str {
        match self {
            Preset::Technical => "menu.bitmaps.trace_technical",
            Preset::LineDrawing => "menu.bitmaps.trace_line_drawing",
            Preset::LineArt => "menu.bitmaps.trace_line_art",
            Preset::Logo => "menu.bitmaps.trace_logo",
            Preset::DetailedLogo => "menu.bitmaps.trace_detailed_logo",
            Preset::Clipart => "menu.bitmaps.trace_clipart",
            Preset::LowQualityImage => "menu.bitmaps.trace_low_quality",
            Preset::HighQualityImage => "menu.bitmaps.trace_high_quality",
        }
    }
    pub fn settings(self) -> Settings {
        let (detail, smoothing, colors, bw) = match self {
            Preset::Technical => (60.0, 30.0, 2, true),
            Preset::LineDrawing => (50.0, 50.0, 2, true),
            Preset::LineArt => (40.0, 25.0, 2, true),
            Preset::Logo => (50.0, 25.0, 8, false),
            Preset::DetailedLogo => (75.0, 20.0, 24, false),
            Preset::Clipart => (60.0, 25.0, 32, false),
            Preset::LowQualityImage => (35.0, 40.0, 48, false),
            Preset::HighQualityImage => (80.0, 15.0, 96, false),
        };
        Settings {
            detail,
            smoothing,
            corner_smoothness: 0.0,
            colors,
            black_white: bw,
            remove_background: true,
            merge_adjacent: false,
            centerline: self.is_centerline(),
            delete_original: false,
            gradients: !bw,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    pub detail: f64,
    pub smoothing: f64,
    pub corner_smoothness: f64,
    pub colors: u32,
    pub black_white: bool,
    pub remove_background: bool,
    pub merge_adjacent: bool,
    pub centerline: bool,
    pub delete_original: bool,
    /// Shaded regions become one shape with a linear ramp instead of
    /// bands of flat colour (colour outline modes).
    pub gradients: bool,
}

/// One traced region or stroke.
pub struct Traced {
    pub path: BezPath,
    /// The flat colour, or the middle of the ramp.
    pub color: Color,
    /// The path is closed (a filled region, or a stroke that loops).
    pub closed: bool,
    /// Stroke width in pixels for a centreline; 0 for a filled region.
    pub width: f64,
    /// A shaded region's ramp (pixel space).
    pub ramp: Option<crate::trace_region::Ramp>,
}

impl Traced {
    pub fn is_stroke(&self) -> bool {
        self.width > 0.0
    }
}

/// Trace `img` into shapes whose coordinates are in pixel space (y down).
/// The caller maps them into the bitmap's rectangle.
pub fn trace(img: &RgbaImage, s: &Settings) -> Vec<Traced> {
    // A blocky picture (a small one enlarged without smoothing) is traced
    // from one pixel per block, then put back over the blocks.
    if let Some(g) = crate::trace_grid::detect(img) {
        let small = crate::trace_grid::sample(img, &g);
        let mut out = trace_pixels(&small, s, g.bx);
        let back = graphdraw_core::geometry::Affine::new([g.bx, 0.0, 0.0, g.by, g.x0, g.y0]);
        for t in &mut out {
            t.path = back * t.path.clone();
            t.width *= g.bx;
            if let Some(r) = &mut t.ramp {
                r.a = back * r.a;
                r.z = back * r.z;
            }
        }
        return out;
    }
    trace_pixels(img, s, 1.0)
}

/// Trace `img`, one of whose pixels is `unit` pixels of the picture the
/// user sees (more than 1 for a blocky picture sampled per block).
fn trace_pixels(img: &RgbaImage, s: &Settings, unit: f64) -> Vec<Traced> {
    if s.centerline {
        return crate::trace_center::centerline(img, s);
    }
    outline(img, s, unit)
}

/// Fitting settings for `s`: detail sets the tolerance, smoothing the
/// passes over the points, corner smoothness the corner angle.
pub fn fit_params(s: &Settings) -> crate::trace_fit::FitParams {
    let detail = s.detail.clamp(0.0, 100.0);
    crate::trace_fit::FitParams {
        tolerance: 0.25 + (100.0 - detail) / 100.0 * 1.5,
        corner_angle: (50.0 + s.corner_smoothness.clamp(0.0, 100.0) * 0.85).to_radians(),
        smooth_passes: (s.smoothing.clamp(0.0, 100.0) / 25.0).round() as usize,
        scale: 1,
    }
}

/// Specks smaller than this many pixels are merged away: more at low
/// detail, and more in big pictures.
pub fn speck_area(detail: f64, w: usize, h: usize) -> usize {
    let d = 1.0 - detail.clamp(0.0, 100.0) / 100.0;
    let scale = ((w * h) as f64 / 250_000.0).sqrt().max(1.0);
    ((1.0 + 40.0 * d * d) * scale).round() as usize
}

// ----- outline mode -----------------------------------------------------------

/// Longer side a small picture's outlines are worked out at.
const ENLARGE_TO: usize = 1600;
/// Most pixels the enlarged outlines may cover.
const ENLARGE_MAX_PIXELS: usize = 4_000_000;

/// How many times finer than the picture's pixels a `w` x `h` picture's
/// outlines are followed (1 to 4).
pub fn enlarge_factor(w: usize, h: usize) -> usize {
    let long = w.max(h).max(1);
    let mut k = (ENLARGE_TO / long).clamp(1, 4);
    while k > 1 && w * h * k * k > ENLARGE_MAX_PIXELS {
        k -= 1;
    }
    k
}

/// A layer `k` times finer, its staircases smoothed. `cov` is how much
/// of each pixel the layer covers (1 or 0, or a share where the pixel is
/// an anti-aliased mix of the layer and its neighbour). The coverage is
/// softened (weights 1 6 1 each way, so a one pixel line keeps most of
/// its weight), sampled between the pixels and cut at one half: a step
/// edge becomes a continuous line through the middle of its steps, and a
/// mixed pixel moves the line to where the true edge crosses it. Cells
/// inside a pixel the layer covers whole stay in it a little longer (cut
/// at 0.35), so thin lines and small dots survive.
fn smooth_mask(cov: &[f32], w: usize, h: usize, k: usize) -> Vec<bool> {
    let at = |x: isize, y: isize| -> f32 {
        if x < 0 || y < 0 || x >= w as isize || y >= h as isize {
            0.0
        } else {
            cov[y as usize * w + x as usize]
        }
    };
    // Softened values at pixel centres.
    let mut soft = vec![0f32; w * h];
    let kern = [1.0f32 / 8.0, 6.0 / 8.0, 1.0 / 8.0];
    for y in 0..h as isize {
        for x in 0..w as isize {
            let mut s = 0.0;
            for (j, wy) in kern.iter().enumerate() {
                for (i, wx) in kern.iter().enumerate() {
                    s += wx * wy * at(x + i as isize - 1, y + j as isize - 1);
                }
            }
            soft[y as usize * w + x as usize] = s;
        }
    }
    let sv = |x: isize, y: isize| -> f32 {
        if x < 0 || y < 0 || x >= w as isize || y >= h as isize {
            0.0
        } else {
            soft[y as usize * w + x as usize]
        }
    };
    let (bw, bh) = (w * k, h * k);
    let mut out = vec![false; bw * bh];
    for by in 0..bh {
        // The cell's centre in pixel-centre coordinates.
        let fy = (by as f32 + 0.5) / k as f32 - 0.5;
        let y0 = fy.floor();
        let ty = fy - y0;
        for bx in 0..bw {
            let fx = (bx as f32 + 0.5) / k as f32 - 0.5;
            let x0 = fx.floor();
            let tx = fx - x0;
            let (xi, yi) = (x0 as isize, y0 as isize);
            let v = (sv(xi, yi) * (1.0 - tx) + sv(xi + 1, yi) * tx) * (1.0 - ty)
                + (sv(xi, yi + 1) * (1.0 - tx) + sv(xi + 1, yi + 1) * tx) * ty;
            let inside = cov[(by / k) * w + bx / k] >= 0.95;
            out[by * bw + bx] = v >= if inside { 0.35 } else { 0.5 };
        }
    }
    out
}

fn outline(img: &RgbaImage, s: &Settings, unit: f64) -> Vec<Traced> {
    use crate::trace_color as tc;
    let (w, h) = (img.width() as usize, img.height() as usize);
    if w == 0 || h == 0 {
        return Vec::new();
    }
    // A picture of two colours: the outline of the amount of ink, see
    // trace_ink.rs.
    if !s.black_white {
        if let Some(tones) = crate::trace_ink::two_tones(img) {
            return crate::trace_ink::outline_two_tone(img, tones, s, unit);
        }
    }
    // 1. Edge-preserving smoothing when detail is low: grain and
    // compression noise go, edges and corners stay.
    // (Not for a blocky picture: its blocks carry no grain.)
    let src = if s.detail < 70.0 && unit <= 1.0 {
        let radius = if s.detail < 35.0 { 2 } else { 1 };
        tc::bilateral(img, radius, 12.0 + (70.0 - s.detail as f32) * 0.6)
    } else {
        img.clone()
    };
    // Black and white: the outline of the amount of ink, see trace_ink.rs.
    if s.black_white {
        return crate::trace_ink::outline_bw(&src, s, unit);
    }
    // 2. Background: with a plain or gently shaded one, flood it from the
    // border and leave it out before choosing colours, so the palette is
    // spent on the picture itself.
    let flood = if s.remove_background {
        tc::background(&src)
    } else {
        None
    };
    let mut src = src;
    if let Some((mask, _)) = &flood {
        for (p, m) in src.pixels_mut().zip(mask) {
            if *m {
                p[3] = 0;
            }
        }
    }
    // 3. Palette and labels.
    let palette = tc::palette(&src, s.colors.clamp(2, 256) as usize);
    let mut labels = tc::label(&src, &palette);
    // 4. Specks and anti-aliasing fringes join their neighbours.
    tc::clean(
        &mut labels,
        w,
        h,
        &palette,
        // Specks measured in the pixels the user sees.
        ((speck_area(s.detail, (w as f64 * unit) as usize, (h as f64 * unit) as usize) as f64
            / (unit * unit))
            .round() as usize)
            .max(1),
        true,
        flood.as_ref().map(|f| f.1),
        // In a blocky picture every mixed block is an edge, however wide.
        if unit > 1.0 { usize::MAX / 8 } else { 1 },
    );
    // 5. Layers: with gradients, the pieces of one shading merge back into
    // a region with a ramp, and flat regions of one colour share a layer;
    // otherwise each palette colour is a layer.
    use crate::trace_region::RegionFill;
    // (Not in a blocky picture: its mixed edge blocks would read as
    // shading.)
    let (labels, fills): (Vec<u32>, Vec<RegionFill>) = if s.gradients && unit <= 1.0 {
        let (region, fills) = crate::trace_region::regions(
            &src,
            &labels,
            w,
            h,
            crate::trace_region::MergeParams::for_detail(s.detail),
        );
        share_flat_layers(region, fills)
    } else {
        (labels, palette.iter().map(|c| RegionFill::Flat(*c)).collect())
    };
    // 6. Without a flooded background, the background is the layer
    // touching the border most.
    let mut area = vec![0usize; fills.len()];
    let mut border = vec![0usize; fills.len()];
    let mut boxes: Vec<Option<(usize, usize, usize, usize)>> = vec![None; fills.len()];
    for (i, l) in labels.iter().enumerate() {
        let Some(a) = area.get_mut(*l as usize) else {
            continue;
        };
        *a += 1;
        let (x, y) = (i % w, i / w);
        if x == 0 || y == 0 || x + 1 == w || y + 1 == h {
            border[*l as usize] += 1;
        }
        let b = &mut boxes[*l as usize];
        *b = Some(match *b {
            None => (x, y, x, y),
            Some((a, c, d, e)) => (a.min(x), c.min(y), d.max(x), e.max(y)),
        });
    }
    let background = if s.remove_background && flood.is_none() {
        border
            .iter()
            .enumerate()
            .filter(|(_, c)| **c > 0)
            .max_by_key(|(_, c)| **c)
            .map(|(i, _)| i)
    } else {
        None
    };
    // 7. Stack largest first: each layer also covers the pixels of the
    // layers above it that it surrounds or borders, so neighbouring
    // colours overlap instead of leaving hairline gaps.
    let mut order: Vec<usize> = (0..fills.len())
        .filter(|i| area[*i] > 0 && Some(*i) != background)
        .collect();
    order.sort_by_key(|i| std::cmp::Reverse(area[*i]));
    let mut rank = vec![-1i64; fills.len()];
    for (r, i) in order.iter().enumerate() {
        rank[*i] = r as i64;
    }
    let rank_of = |l: u32| -> i64 { rank.get(l as usize).copied().unwrap_or(-1) };
    // Outlines of small pictures are followed k times finer.
    let k = enlarge_factor(w, h);
    let mut fp = fit_params(s);
    fp.scale = k;
    // Colours for the coverage of mixed pixels (only when outlines are
    // followed finer than the pixels).
    let mix = (k > 1).then(|| Mix::new(&src, &fills, flood.as_ref().map(|f| f.1)));
    let mut out = Vec::new();
    for &ci in &order {
        let my = rank[ci];
        let Some((x0, y0, x1, y1)) = boxes[ci] else {
            continue;
        };
        // A margin of 3 keeps the grown mask off the crop's edge, and a
        // one pixel empty frame keeps the outside connected.
        let (x0, y0) = (x0.saturating_sub(3), y0.saturating_sub(3));
        let (x1, y1) = ((x1 + 4).min(w), (y1 + 4).min(h));
        let (cw, ch) = (x1 - x0 + 2, y1 - y0 + 2);
        let mut mask = vec![false; cw * ch];
        let mut above = vec![false; cw * ch];
        for y in y0..y1 {
            for x in x0..x1 {
                let l = labels[y * w + x];
                let k = (y - y0 + 1) * cw + (x - x0 + 1);
                mask[k] = l == ci as u32;
                above[k] = rank_of(l) > my;
            }
        }
        grow_under(&mut mask, &above, cw, ch, 2);
        fill_holes_under(&mut mask, &above, cw, ch);
        let mut path = BezPath::new();
        let (fine, fw, fh) = if let Some(mix) = &mix {
            let cov = mix.coverage(&mask, &labels, w, h, ci, (x0, y0, cw, ch), &rank_of);
            (smooth_mask(&cov, cw, ch, k), cw * k, ch * k)
        } else {
            (mask, cw, ch)
        };
        let kf = k as f64;
        for ring in contours(&fine, fw, fh) {
            let pts: Vec<Point> = ring_midpoints(&ring)
                .into_iter()
                .map(|p| {
                    Point::new(
                        (p.x / kf + x0 as f64 - 1.0).clamp(0.0, w as f64),
                        (p.y / kf + y0 as f64 - 1.0).clamp(0.0, h as f64),
                    )
                })
                .collect();
            let sub = crate::trace_fit::fit_closed(&pts, &fp);
            if sub.area().abs() >= 0.5 {
                path.extend(sub);
            }
        }
        if path.elements().is_empty() {
            continue;
        }
        let (c, ramp) = match &fills[ci] {
            RegionFill::Flat(c) => (*c, None),
            RegionFill::Ramp(r) => (r.stops[r.stops.len() / 2].1, Some(r.clone())),
        };
        out.push(Traced {
            path,
            color: Color::rgb8(c[0], c[1], c[2]),
            closed: true,
            width: 0.0,
            ramp,
        });
    }
    out
}


/// Colours (Oklab) for telling how much of a pixel a layer covers: an
/// anti-aliased edge pixel is a mix of the colours either side.
struct Mix {
    pixel: Vec<[f32; 3]>,
    layer: Vec<[f32; 3]>,
    /// The removed ground's colour, for pixels labelled clear.
    clear: Option<[f32; 3]>,
}

impl Mix {
    fn new(
        src: &RgbaImage,
        fills: &[crate::trace_region::RegionFill],
        clear: Option<[u8; 3]>,
    ) -> Mix {
        use crate::trace_color::oklab;
        use crate::trace_region::RegionFill;
        let mut cache = std::collections::HashMap::<[u8; 3], [f32; 3]>::new();
        Mix {
            pixel: src
                .pixels()
                .map(|p| {
                    let c = [p[0], p[1], p[2]];
                    *cache.entry(c).or_insert_with(|| oklab(c))
                })
                .collect(),
            layer: fills
                .iter()
                .map(|f| match f {
                    RegionFill::Flat(c) => oklab(*c),
                    RegionFill::Ramp(r) => oklab(r.stops[r.stops.len() / 2].1),
                })
                .collect(),
            clear: clear.map(oklab),
        }
    }

    fn colour_of(&self, l: u32) -> Option<[f32; 3]> {
        if l == crate::trace_color::CLEAR {
            self.clear
        } else {
            self.layer.get(l as usize).copied()
        }
    }

    /// Coverage of layer `ci` over a crop (`x0, y0` its first pixel, a
    /// one cell frame round it) whose grown mask is `mask`: whole where
    /// the mask is set, less on its own edge pixels that mix towards a
    /// neighbour below it, and a share of the outside pixels next to it
    /// that mix towards it.
    #[allow(clippy::too_many_arguments)]
    fn coverage(
        &self,
        mask: &[bool],
        labels: &[u32],
        w: usize,
        h: usize,
        ci: usize,
        (x0, y0, cw, ch): (usize, usize, usize, usize),
        rank_of: &dyn Fn(u32) -> i64,
    ) -> Vec<f32> {
        let ci = ci as u32;
        let Some(cl) = self.colour_of(ci) else {
            return mask.iter().map(|m| *m as u8 as f32).collect();
        };
        let mine = rank_of(ci);
        let cov = (0..cw * ch)
            .map(|kk| {
                let (cx, cy) = (kk % cw, kk / cw);
                let whole = mask[kk] as u8 as f32;
                if cx == 0 || cy == 0 {
                    return whole;
                }
                let (x, y) = (x0 + cx - 1, y0 + cy - 1);
                if x >= w || y >= h {
                    return whole;
                }
                let i = y * w + x;
                let l = labels[i];
                if mask[kk] && l != ci {
                    // Grown under a layer above, or a hole it covers.
                    return 1.0;
                }
                let c = self.pixel[i];
                // Labels of the 8 neighbours.
                let near = (-1i64..=1).flat_map(|dy| (-1i64..=1).map(move |dx| (dx, dy)));
                let mut best: Option<f32> = None;
                for (dx, dy) in near {
                    let (nx, ny) = (x as i64 + dx, y as i64 + dy);
                    if nx < 0 || ny < 0 || nx >= w as i64 || ny >= h as i64 {
                        continue;
                    }
                    let m = labels[ny as usize * w + nx as usize];
                    if mask[kk] {
                        // My edge pixel: how far it leans to a neighbour
                        // below me.
                        if m == ci || rank_of(m) > mine {
                            continue;
                        }
                        if let Some(t) = self
                            .colour_of(m)
                            .and_then(|cb| crate::trace_color::blend_of(c, cl, cb))
                        {
                            best = Some(best.map_or(t, |b| b.max(t)));
                        }
                    } else if m == ci {
                        // A pixel next to me: how far it leans to me.
                        if let Some(t) = self
                            .colour_of(l)
                            .and_then(|ca| crate::trace_color::blend_of(c, ca, cl))
                        {
                            best = Some(t);
                        }
                        break;
                    }
                }
                match (mask[kk], best) {
                    (true, Some(t)) => 1.0 - t,
                    (true, None) => 1.0,
                    (false, Some(t)) => t,
                    (false, None) => 0.0,
                }
            })
            .collect::<Vec<f32>>();
        // Past the picture's edge a layer goes on as at the edge, so
        // smoothing does not round it off there (the outline is cut back
        // to the edge afterwards).
        let inside = |c: usize, o: usize, n: usize| -> usize {
            // Crop cell `c` of a crop starting at picture pixel `o`, kept
            // within the picture's `n` pixels.
            let g = (o + c).saturating_sub(1).min(n - 1);
            g + 1 - o
        };
        let mut out = cov.clone();
        for cy in 0..ch {
            for cx in 0..cw {
                let off_x = (cx == 0 && x0 == 0) || x0 + cx > w;
                let off_y = (cy == 0 && y0 == 0) || y0 + cy > h;
                if off_x || off_y {
                    let (ix, iy) = (inside(cx, x0, w).min(cw - 1), inside(cy, y0, h).min(ch - 1));
                    out[cy * cw + cx] = cov[iy * cw + ix];
                }
            }
        }
        out
    }
}

/// Flat regions within this Oklab distance of each other share a layer
/// (one compound shape), as the letters of a word in one colour do.
const SAME_FLAT: f32 = 0.015;

/// Renumber `region` so flat regions of about one colour share a layer
/// (their colour the area-weighted mean); every ramp keeps its own.
fn share_flat_layers(
    region: Vec<u32>,
    fills: Vec<crate::trace_region::RegionFill>,
) -> (Vec<u32>, Vec<crate::trace_region::RegionFill>) {
    use crate::trace_color::{from_oklab, oklab, CLEAR};
    use crate::trace_region::RegionFill;
    let mut area = vec![0f64; fills.len()];
    for r in &region {
        if let Some(a) = area.get_mut(*r as usize) {
            *a += 1.0;
        }
    }
    // Biggest regions first, so a layer's colour starts from its main
    // piece.
    let mut by_area: Vec<usize> = (0..fills.len()).collect();
    by_area.sort_by(|a, b| area[*b].total_cmp(&area[*a]));
    let mut layer_of = vec![CLEAR; fills.len()];
    let mut layers: Vec<RegionFill> = Vec::new();
    // Per flat layer: summed Oklab and area.
    let mut flat: Vec<(usize, [f32; 3], f64)> = Vec::new();
    for r in by_area {
        match &fills[r] {
            RegionFill::Ramp(_) => {
                layer_of[r] = layers.len() as u32;
                layers.push(fills[r].clone());
            }
            RegionFill::Flat(c) => {
                let c = oklab(*c);
                let near = flat.iter_mut().find(|f| {
                    let m = [0, 1, 2].map(|k| f.1[k] / f.2 as f32);
                    (0..3).map(|k| (m[k] - c[k]).powi(2)).sum::<f32>() <= SAME_FLAT * SAME_FLAT
                });
                let a = area[r];
                match near {
                    Some(f) => {
                        for (s, v) in f.1.iter_mut().zip(c) {
                            *s += v * a as f32;
                        }
                        f.2 += a;
                        layer_of[r] = f.0 as u32;
                    }
                    None => {
                        layer_of[r] = layers.len() as u32;
                        flat.push((layers.len(), c.map(|v| v * a as f32), a));
                        layers.push(RegionFill::Flat([0; 3]));
                    }
                }
            }
        }
    }
    for (l, sum, a) in flat {
        let a = a.max(1.0) as f32;
        layers[l] = RegionFill::Flat(from_oklab(sum.map(|v| v / a)));
    }
    let region = region
        .into_iter()
        .map(|r| layer_of.get(r as usize).copied().unwrap_or(CLEAR))
        .collect();
    (region, layers)
}

/// Grow `mask` by `n` pixels (8-neighbourhood) into cells where `under`
/// is set.
fn grow_under(mask: &mut [bool], under: &[bool], w: usize, h: usize, n: usize) {
    for _ in 0..n {
        let prev = mask.to_vec();
        for y in 1..h.saturating_sub(1) {
            for x in 1..w.saturating_sub(1) {
                let i = y * w + x;
                if prev[i] || !under[i] {
                    continue;
                }
                let near = [
                    i - w - 1,
                    i - w,
                    i - w + 1,
                    i - 1,
                    i + 1,
                    i + w - 1,
                    i + w,
                    i + w + 1,
                ];
                if near.iter().any(|j| prev[*j]) {
                    mask[i] = true;
                }
            }
        }
    }
}

/// Fill the holes of `mask` that lie wholly in `under` cells: a layer
/// above covers them, so the layer below need not cut them out.
fn fill_holes_under(mask: &mut [bool], under: &[bool], w: usize, h: usize) {
    let mut seen = vec![false; mask.len()];
    let mut stack = Vec::new();
    let mut comp = Vec::new();
    for start in 0..mask.len() {
        if mask[start] || seen[start] {
            continue;
        }
        seen[start] = true;
        stack.push(start);
        comp.clear();
        let mut outside = false;
        let mut covered = true;
        while let Some(i) = stack.pop() {
            comp.push(i);
            let (x, y) = (i % w, i / w);
            if x == 0 || y == 0 || x + 1 == w || y + 1 == h {
                outside = true;
            }
            covered &= under[i];
            let ns = [
                (x > 0).then(|| i - 1),
                (x + 1 < w).then(|| i + 1),
                (y > 0).then(|| i - w),
                (y + 1 < h).then(|| i + w),
            ];
            for j in ns.into_iter().flatten() {
                if !mask[j] && !seen[j] {
                    seen[j] = true;
                    stack.push(j);
                }
            }
        }
        if !outside && covered {
            for &i in &comp {
                mask[i] = true;
            }
        }
    }
}

/// The midpoints of a ring's unit pixel edges. On a staircase they lie on
/// the line the stairs approximate, which is what the fitter wants.
pub(crate) fn ring_midpoints(ring: &[Point]) -> Vec<Point> {
    let mut out = Vec::new();
    let n = ring.len();
    for i in 0..n {
        let a = ring[i];
        let b = ring[(i + 1) % n];
        let d = b - a;
        let steps = d.x.abs().max(d.y.abs()).round() as usize;
        if steps == 0 {
            continue;
        }
        let u = d / steps as f64;
        for k in 0..steps {
            out.push(a + u * (k as f64 + 0.5));
        }
    }
    out
}

pub(crate) fn luma(p: Rgba<u8>) -> f32 {
    0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32
}

/// Boundary rings of a binary mask (pixel-edge following). Outer rings are
/// counter-clockwise and holes clockwise, so even-odd filling is right.
pub fn contours(mask: &[bool], w: usize, h: usize) -> Vec<Vec<Point>> {
    // Build the set of boundary edges between mask and non-mask cells, then
    // chain them into rings. Each edge is a directed unit segment keeping
    // the filled cell on its left.
    use std::collections::HashMap;
    let at = |x: i64, y: i64| -> bool {
        if x < 0 || y < 0 || x >= w as i64 || y >= h as i64 {
            false
        } else {
            mask[y as usize * w + x as usize]
        }
    };
    // Directed edges keyed by start vertex.
    let mut edges: HashMap<(i64, i64), Vec<(i64, i64)>> = HashMap::new();
    let mut push = |a: (i64, i64), b: (i64, i64)| edges.entry(a).or_default().push(b);
    for y in 0..h as i64 {
        for x in 0..w as i64 {
            if !at(x, y) {
                continue;
            }
            // Image y grows down; walk so the filled cell is on the left in
            // image space (clockwise on screen), which flips to CCW in page space.
            if !at(x, y - 1) {
                push((x, y), (x + 1, y)); // top edge, left to right
            }
            if !at(x + 1, y) {
                push((x + 1, y), (x + 1, y + 1)); // right edge, down
            }
            if !at(x, y + 1) {
                push((x + 1, y + 1), (x, y + 1)); // bottom edge, right to left
            }
            if !at(x - 1, y) {
                push((x, y + 1), (x, y)); // left edge, up
            }
        }
    }
    let mut rings = Vec::new();
    let mut keys: Vec<(i64, i64)> = edges.keys().copied().collect();
    keys.sort_unstable();
    for start in keys {
        while let Some(first) = edges.get_mut(&start).and_then(|v| v.pop()) {
            let mut ring = vec![Point::new(start.0 as f64, start.1 as f64)];
            let mut cur = first;
            let mut prev = start;
            let mut guard = 0;
            while cur != start && guard < w * h * 4 {
                ring.push(Point::new(cur.0 as f64, cur.1 as f64));
                let Some(nexts) = edges.get_mut(&cur) else {
                    break;
                };
                if nexts.is_empty() {
                    break;
                }
                // Prefer turning right (keeps separate touching rings apart).
                let dir = (cur.0 - prev.0, cur.1 - prev.1);
                let right = (-dir.1, dir.0);
                let pick = nexts
                    .iter()
                    .position(|n| (n.0 - cur.0, n.1 - cur.1) == right)
                    .or_else(|| nexts.iter().position(|n| (n.0 - cur.0, n.1 - cur.1) == dir))
                    .unwrap_or(0);
                let next = nexts.remove(pick);
                prev = cur;
                cur = next;
                guard += 1;
            }
            // Drop collinear points.
            let mut compact: Vec<Point> = Vec::new();
            for p in ring {
                let n = compact.len();
                if n >= 2 {
                    let a = compact[n - 2];
                    let b = compact[n - 1];
                    if (b.x - a.x) * (p.y - b.y) - (b.y - a.y) * (p.x - b.x) == 0.0 {
                        compact.pop();
                    }
                }
                compact.push(p);
            }
            if compact.len() >= 3 {
                rings.push(compact);
            }
        }
    }
    rings
}

/// Zhang-Suen thinning.
pub fn thin(mask: &mut [bool], w: usize, h: usize) {
    let at = |m: &[bool], x: i64, y: i64| -> bool {
        x >= 0 && y >= 0 && (x as usize) < w && (y as usize) < h && m[y as usize * w + x as usize]
    };
    loop {
        let mut changed = false;
        for pass in 0..2 {
            let mut remove = Vec::new();
            for y in 0..h as i64 {
                for x in 0..w as i64 {
                    if !at(mask, x, y) {
                        continue;
                    }
                    let p2 = at(mask, x, y - 1);
                    let p3 = at(mask, x + 1, y - 1);
                    let p4 = at(mask, x + 1, y);
                    let p5 = at(mask, x + 1, y + 1);
                    let p6 = at(mask, x, y + 1);
                    let p7 = at(mask, x - 1, y + 1);
                    let p8 = at(mask, x - 1, y);
                    let p9 = at(mask, x - 1, y - 1);
                    let ring = [p2, p3, p4, p5, p6, p7, p8, p9];
                    let b = ring.iter().filter(|v| **v).count();
                    if !(2..=6).contains(&b) {
                        continue;
                    }
                    let a = (0..8).filter(|i| !ring[*i] && ring[(i + 1) % 8]).count();
                    if a != 1 {
                        continue;
                    }
                    let (c1, c2) = if pass == 0 {
                        (p2 && p4 && p6, p4 && p6 && p8)
                    } else {
                        (p2 && p4 && p8, p2 && p6 && p8)
                    };
                    if c1 || c2 {
                        continue;
                    }
                    remove.push((x as usize, y as usize));
                }
            }
            if !remove.is_empty() {
                changed = true;
            }
            for (x, y) in remove {
                mask[y * w + x] = false;
            }
        }
        if !changed {
            break;
        }
    }
}

/// Draw traced shapes (pixel space, y down) into a transparent image of
/// `w` x `h`: regions filled with their colour, centrelines stroked at
/// their width. The trace dialog previews a trace this way.
pub fn render_traced(traced: &[Traced], w: u32, h: u32) -> RgbaImage {
    let Some(mut pm) = tiny_skia::Pixmap::new(w.max(1), h.max(1)) else {
        return RgbaImage::new(w.max(1), h.max(1));
    };
    for t in traced {
        let mut pb = tiny_skia::PathBuilder::new();
        for el in t.path.elements() {
            match *el {
                graphdraw_core::geometry::PathEl::MoveTo(p) => pb.move_to(p.x as f32, p.y as f32),
                graphdraw_core::geometry::PathEl::LineTo(p) => pb.line_to(p.x as f32, p.y as f32),
                graphdraw_core::geometry::PathEl::QuadTo(a, p) => {
                    pb.quad_to(a.x as f32, a.y as f32, p.x as f32, p.y as f32)
                }
                graphdraw_core::geometry::PathEl::CurveTo(a, b, p) => pb.cubic_to(
                    a.x as f32, a.y as f32, b.x as f32, b.y as f32, p.x as f32, p.y as f32,
                ),
                graphdraw_core::geometry::PathEl::ClosePath => pb.close(),
            }
        }
        let Some(path) = pb.finish() else {
            continue;
        };
        let [r, g, b] = t.color.to_rgb8();
        let mut paint = tiny_skia::Paint::default();
        paint.set_color_rgba8(r, g, b, 255);
        paint.anti_alias = true;
        if let Some(ramp) = &t.ramp {
            let stops = ramp
                .stops
                .iter()
                .map(|(pos, c)| {
                    tiny_skia::GradientStop::new(
                        *pos as f32,
                        tiny_skia::Color::from_rgba8(c[0], c[1], c[2], 255),
                    )
                })
                .collect();
            if let Some(shader) = tiny_skia::LinearGradient::new(
                tiny_skia::Point::from_xy(ramp.a.x as f32, ramp.a.y as f32),
                tiny_skia::Point::from_xy(ramp.z.x as f32, ramp.z.y as f32),
                stops,
                tiny_skia::SpreadMode::Pad,
                tiny_skia::Transform::identity(),
            ) {
                paint.shader = shader;
            }
        }
        if !t.is_stroke() {
            pm.fill_path(
                &path,
                &paint,
                tiny_skia::FillRule::EvenOdd,
                tiny_skia::Transform::identity(),
                None,
            );
        } else {
            let stroke = tiny_skia::Stroke {
                width: t.width.max(0.5) as f32,
                line_cap: tiny_skia::LineCap::Round,
                line_join: tiny_skia::LineJoin::Round,
                ..Default::default()
            };
            pm.stroke_path(
                &path,
                &paint,
                &stroke,
                tiny_skia::Transform::identity(),
                None,
            );
        }
    }
    let mut out = RgbaImage::new(pm.width(), pm.height());
    for (px, p) in out.pixels_mut().zip(pm.pixels()) {
        let c = p.demultiply();
        *px = image::Rgba([c.red(), c.green(), c.blue(), c.alpha()]);
    }
    out
}

/// Map traced pixel-space paths into a rectangle (page mm, Y up).
pub fn to_rect(path: &BezPath, img_w: u32, img_h: u32, rect: Rect) -> BezPath {
    rect_affine(img_w, img_h, rect) * path.clone()
}

/// The map from pixel space (y down) into `rect` (page mm, Y up).
pub fn rect_affine(img_w: u32, img_h: u32, rect: Rect) -> graphdraw_core::geometry::Affine {
    let sx = rect.width() / img_w.max(1) as f64;
    let sy = rect.height() / img_h.max(1) as f64;
    graphdraw_core::geometry::Affine::new([sx, 0.0, 0.0, -sy, rect.x0, rect.y1])
}

/// The fountain fill of a traced ramp on a shape whose page-space path
/// is `page_path`; `to_page` maps pixel space to page space.
pub fn ramp_fill(
    ramp: &crate::trace_region::Ramp,
    to_page: graphdraw_core::geometry::Affine,
    page_path: &BezPath,
) -> graphdraw_core::Fill {
    use graphdraw_core::style::{Fountain, FountainKind, Stop};
    let stops: Vec<Stop> = ramp
        .stops
        .iter()
        .map(|(pos, c)| Stop::new(*pos, Color::rgb8(c[0], c[1], c[2])))
        .collect();
    let (Some(first), Some(last)) = (stops.first(), stops.last()) else {
        return graphdraw_core::Fill::None;
    };
    let mut f = Fountain::two(FountainKind::Linear, first.color, last.color, 0.0);
    if stops.len() >= 2 {
        f.stops = stops;
    }
    graphdraw_core::gradient::set_linear_ends(
        &mut f,
        page_path.bounding_box(),
        to_page * ramp.a,
        to_page * ramp.z,
    );
    graphdraw_core::Fill::Fountain(f)
}

#[cfg(test)]
mod tests {
    use super::*;
    use graphdraw_core::geometry::PathSeg;
    use image::ImageBuffer;

    fn circle_image() -> RgbaImage {
        ImageBuffer::from_fn(100, 100, |x, y| {
            let d = ((x as f32 - 50.0).powi(2) + (y as f32 - 50.0).powi(2)).sqrt();
            if d < 30.0 {
                Rgba([0, 0, 0, 255])
            } else {
                Rgba([255, 255, 255, 255])
            }
        })
    }

    #[test]
    fn black_and_white_logos_keep_their_colours() {
        // A white disc on black with a black square inside and a soft
        // edge, mostly black pixels: the disc stays white, not grey.
        let img: RgbaImage = ImageBuffer::from_fn(120, 120, |x, y| {
            let d = ((x as f32 - 60.0).powi(2) + (y as f32 - 60.0).powi(2)).sqrt();
            let inner = (45..75).contains(&x) && (45..75).contains(&y);
            let v = if inner {
                0.0
            } else {
                (255.0 * (40.0 - d).clamp(0.0, 1.0)).round()
            };
            let v = v as u8;
            Rgba([v, v, v, 255])
        });
        for colors in [2, 8] {
            let mut s = Preset::Logo.settings();
            s.colors = colors;
            let t = trace(&img, &s);
            let biggest = t
                .iter()
                .max_by(|a, b| a.path.area().abs().total_cmp(&b.path.area().abs()))
                .expect("a region");
            let [r, g, b] = biggest.color.to_rgb8();
            assert!(
                r > 240 && g > 240 && b > 240,
                "{colors} colours: {r},{g},{b}"
            );
        }
        let pal = crate::trace_color::palette(&img, 2);
        assert!(
            pal.contains(&[0, 0, 0]) && pal.iter().any(|c| c[0] > 245),
            "{pal:?}"
        );
    }

    #[test]
    fn outline_traces_a_circle_with_right_area_and_few_nodes() {
        let img = circle_image();
        let t = trace(&img, &Preset::LineArt.settings());
        assert_eq!(t.len(), 1, "background removed, one region");
        let area = t[0].path.area().abs();
        let expect = std::f64::consts::PI * 30.0 * 30.0;
        assert!(
            (area - expect).abs() / expect < 0.02,
            "area {area} vs {expect}"
        );
        assert_eq!(t[0].color.to_rgb8(), [0, 0, 0]);
        let segs = t[0].path.segments().count();
        assert!(segs <= 12, "{segs} segments for a circle");
    }

    #[test]
    fn a_square_stays_square() {
        let img = ImageBuffer::from_fn(60, 60, |x, y| {
            if (10..50).contains(&x) && (15..45).contains(&y) {
                Rgba([200, 20, 20, 255])
            } else {
                Rgba([255, 255, 255, 255])
            }
        });
        let t = trace(&img, &Preset::Logo.settings());
        assert_eq!(t.len(), 1);
        let segs: Vec<PathSeg> = t[0].path.segments().collect();
        assert!(segs.len() <= 5, "{segs:?}");
        assert!(segs.iter().all(|s| matches!(s, PathSeg::Line(_))), "{segs:?}");
        let bb = t[0].path.bounding_box();
        assert!((bb.x0 - 10.0).abs() < 0.01 && (bb.y1 - 45.0).abs() < 0.01, "{bb:?}");
        assert!((t[0].path.area().abs() - 1200.0).abs() < 1.0);
    }

    #[test]
    fn anti_aliased_edges_and_noise_leave_no_fringe_or_specks() {
        // A soft-edged red disc on white with scattered noise pixels.
        let img = ImageBuffer::from_fn(120, 120, |x, y| {
            let d = ((x as f32 - 60.0).powi(2) + (y as f32 - 60.0).powi(2)).sqrt();
            let a = (40.0 - d).clamp(0.0, 1.0);
            let mix = |c: f32| (255.0 * (1.0 - a) + c * a).round() as u8;
            if (x * 7 + y * 13) % 97 == 0 && d > 45.0 {
                Rgba([90, 90, 90, 255])
            } else {
                Rgba([mix(220.0), mix(30.0), mix(30.0), 255])
            }
        });
        let t = trace(&img, &Preset::Logo.settings());
        assert_eq!(t.len(), 1, "one red disc, no fringe ring or specks");
        let [r, g, _] = t[0].color.to_rgb8();
        assert!(r > 200 && g < 60, "{r},{g}");
    }

    #[test]
    fn a_shaded_background_goes_and_leaves_colours_to_the_picture() {
        // A vertical gradient ground with a white bar and a red bar: the
        // ground is removed whole, and with only 3 colours both bars keep
        // their own.
        let img = ImageBuffer::from_fn(120, 120, |x, y| {
            if (20..100).contains(&x) && (30..50).contains(&y) {
                Rgba([250, 250, 250, 255])
            } else if (20..100).contains(&x) && (70..90).contains(&y) {
                Rgba([220, 30, 30, 255])
            } else {
                let v = (30 + y / 3) as u8;
                Rgba([v / 2, v / 2, v, 255])
            }
        });
        let mut s = Preset::Logo.settings();
        s.colors = 3;
        let t = trace(&img, &s);
        assert_eq!(t.len(), 2, "{:?}", t.iter().map(|t| t.color.to_rgb8()).collect::<Vec<_>>());
        let colours: Vec<[u8; 3]> = t.iter().map(|t| t.color.to_rgb8()).collect();
        assert!(colours.iter().any(|c| c[1] > 240), "{colours:?}");
        assert!(colours.iter().any(|c| c[0] > 200 && c[1] < 60), "{colours:?}");
    }

    #[test]
    fn ground_inside_a_letter_is_a_hole() {
        // A white ring on navy: the navy in the middle is ground too.
        let img = ImageBuffer::from_fn(100, 100, |x, y| {
            let d = ((x as f32 - 50.0).powi(2) + (y as f32 - 50.0).powi(2)).sqrt();
            if (20.0..35.0).contains(&d) {
                Rgba([250, 250, 250, 255])
            } else {
                Rgba([25, 30, 60, 255])
            }
        });
        let t = trace(&img, &Preset::Logo.settings());
        assert_eq!(t.len(), 1);
        let ring = std::f64::consts::PI * (35.0f64.powi(2) - 20.0f64.powi(2));
        let area = t[0].path.area().abs();
        assert!((area - ring).abs() / ring < 0.04, "{area} vs {ring}");
    }

    #[test]
    fn neighbouring_colours_leave_no_gap() {
        // Two colours meeting along a slanted edge: drawn back, no pixel
        // along the seam shows the transparent background through.
        let img = ImageBuffer::from_fn(80, 80, |x, y| {
            if (x as f32) < 20.0 + y as f32 * 0.4 {
                Rgba([30, 60, 200, 255])
            } else {
                Rgba([240, 200, 20, 255])
            }
        });
        let mut s = Preset::Logo.settings();
        s.remove_background = false;
        let t = trace(&img, &s);
        assert_eq!(t.len(), 2);
        let back = render_traced(&t, 80, 80);
        for y in 2..78 {
            for x in 2..78 {
                assert_eq!(back.get_pixel(x, y)[3], 255, "gap at {x},{y}");
            }
        }
    }

    #[test]
    fn a_shaded_shape_traces_to_one_ramp_on_the_page() {
        // A disc shaded red to blue left to right on white: one shape with
        // a ramp, whose fountain on the page runs where the ramp does.
        let img = ImageBuffer::from_fn(120, 120, |x, y| {
            let d = ((x as f32 - 60.0).powi(2) + (y as f32 - 60.0).powi(2)).sqrt();
            if d < 45.0 {
                let t = (x as f32 - 15.0) / 90.0;
                Rgba([(220.0 * (1.0 - t)) as u8, 40, (220.0 * t) as u8, 255])
            } else {
                Rgba([255, 255, 255, 255])
            }
        });
        let t = trace(&img, &Preset::Clipart.settings());
        assert_eq!(t.len(), 1, "{}", t.len());
        let ramp = t[0].ramp.clone().expect("a ramp");
        assert!(ramp.z.x - ramp.a.x > 60.0, "{ramp:?}");
        let rect = Rect::new(0.0, 0.0, 60.0, 60.0);
        let to_page = rect_affine(120, 120, rect);
        let page = to_rect(&t[0].path, 120, 120, rect);
        let graphdraw_core::Fill::Fountain(f) = ramp_fill(&ramp, to_page, &page) else {
            panic!("a fountain");
        };
        // The axis runs through the bounds' centre; along its direction
        // its ends match the ramp's (across it a linear fill is the same).
        let (a, z) = f.linear_axis(page.bounding_box());
        let u = (z - a) / (z - a).hypot();
        let (ra, rz) = (to_page * ramp.a, to_page * ramp.z);
        assert!((a - ra).dot(u).abs() < 1e-6, "{a:?} {ra:?}");
        assert!((z - rz).dot(u).abs() < 1e-6, "{z:?} {rz:?}");
        assert!((rz - ra).cross(u).abs() < 1e-6, "same direction");
        assert!(f.stops.first().is_some_and(|s| s.color.to_rgb8()[0] > 180));
        assert!(f.stops.last().is_some_and(|s| s.color.to_rgb8()[2] > 180));
        // Drawn back, the middle of the disc is the mix of the two.
        let back = render_traced(&t, 120, 120);
        let p = back.get_pixel(60, 60);
        assert!((p[0] as i32 - p[2] as i32).abs() < 40, "{p:?}");
    }

    /// A quarter ring (radii 12 and 18 round the bottom-left corner of a
    /// 24 px picture) drawn with anti-aliased grey edges, then enlarged
    /// 10 times without smoothing: big grey and black blocks.
    fn blocky_arc() -> RgbaImage {
        let small = ImageBuffer::from_fn(24, 24, |x, y| {
            // Coverage from 8 x 8 samples per pixel.
            let mut n = 0;
            for sy in 0..8 {
                for sx in 0..8 {
                    let px = x as f32 + (sx as f32 + 0.5) / 8.0;
                    let py = y as f32 + (sy as f32 + 0.5) / 8.0;
                    let d = (px * px + (24.0 - py).powi(2)).sqrt();
                    if (12.0..18.0).contains(&d) {
                        n += 1;
                    }
                }
            }
            let v = 255 - (255 * n / 64) as u8;
            Rgba([v, v, v, 255])
        });
        image::imageops::resize(&small, 240, 240, image::imageops::FilterType::Nearest)
    }

    #[test]
    fn a_blocky_picture_traces_as_the_shape_it_shows() {
        let img = blocky_arc();
        if let Ok(dir) = std::env::var("TRACE_OUT") {
            let _ = img.save(format!("{dir}/blocky-arc.png"));
        }
        let g = crate::trace_grid::detect(&img).expect("blocks");
        assert!((g.bx - 10.0).abs() < 0.3, "{g:?}");
        let t = trace(&img, &Preset::Logo.settings());
        assert_eq!(t.len(), 1, "{}", t.len());
        // The true quarter ring, 10 times larger.
        let expect = std::f64::consts::PI / 4.0 * (180.0f64.powi(2) - 120.0f64.powi(2));
        let area = t[0].path.area().abs();
        assert!((area - expect).abs() / expect < 0.05, "{area} vs {expect}");
        let segs = t[0].path.segments().count();
        assert!(segs <= 12, "{segs} segments");
        let [r, g, b] = t[0].color.to_rgb8();
        assert!(r < 60 && g < 60 && b < 60, "{r},{g},{b}");
    }

    #[test]
    fn small_pictures_get_smooth_outlines() {
        // A 30 px disc: its outline follows the circle between the
        // pixels, so its area is close to the true one with few nodes.
        let img = ImageBuffer::from_fn(30, 30, |x, y| {
            let d = ((x as f32 + 0.5 - 15.0).powi(2) + (y as f32 + 0.5 - 15.0).powi(2)).sqrt();
            if d < 10.0 {
                Rgba([0, 0, 0, 255])
            } else {
                Rgba([255, 255, 255, 255])
            }
        });
        assert_eq!(enlarge_factor(30, 30), 4);
        let t = trace(&img, &Preset::LineArt.settings());
        assert_eq!(t.len(), 1);
        let area = t[0].path.area().abs();
        let expect = std::f64::consts::PI * 100.0;
        assert!((area - expect).abs() / expect < 0.05, "{area} vs {expect}");
        assert!(t[0].path.segments().count() <= 8);
    }

    #[test]
    fn empty_and_transparent_pictures_trace_to_nothing() {
        for (w, h) in [(0, 0), (1, 1), (3, 1)] {
            let img = RgbaImage::new(w, h);
            for p in Preset::ALL {
                assert!(trace(&img, &p.settings()).is_empty(), "{w}x{h} {p:?}");
            }
        }
    }

    /// Trace a picture with every preset and write each result drawn back
    /// next to the original, for looking at by eye:
    /// `TRACE_IN=pic.png TRACE_OUT=dir cargo test -p graphdraw trace_a_file -- --ignored`.
    #[test]
    #[ignore]
    fn trace_a_file() {
        let (Ok(inp), Ok(dir)) = (std::env::var("TRACE_IN"), std::env::var("TRACE_OUT")) else {
            return;
        };
        let mut img = image::open(&inp).expect("picture").to_rgba8();
        // TRACE_SCALE=0.15 shrinks the picture first (a small picture).
        if let Some(f) = std::env::var("TRACE_SCALE").ok().and_then(|v| v.parse::<f64>().ok()) {
            let (w, h) = ((img.width() as f64 * f) as u32, (img.height() as f64 * f) as u32);
            img = image::imageops::resize(&img, w.max(1), h.max(1), image::imageops::FilterType::Triangle);
        }
        for p in Preset::ALL {
            let start = std::time::Instant::now();
            let t = trace(&img, &p.settings());
            let ms = start.elapsed().as_millis();
            let nodes: usize = t.iter().map(|t| t.path.elements().len()).sum();
            println!("{p:?}: {} shapes, {nodes} nodes, {ms} ms", t.len());
            let back = render_traced(&t, img.width(), img.height());
            let mut side = RgbaImage::from_pixel(
                img.width() * 2,
                img.height(),
                Rgba([255, 255, 255, 255]),
            );
            image::imageops::overlay(&mut side, &img, 0, 0);
            image::imageops::overlay(&mut side, &back, img.width() as i64, 0);
            side.save(format!("{dir}/{p:?}.png")).expect("write");
            // TRACE_CROP=x,y,w,h also writes that part of the picture and
            // of the trace, one over the other, at full size.
            let crop: Vec<u32> = std::env::var("TRACE_CROP")
                .unwrap_or_default()
                .split(',')
                .filter_map(|v| v.trim().parse().ok())
                .collect();
            if let [x, y, cw, ch] = crop[..] {
                let a = image::imageops::crop_imm(&img, x, y, cw, ch).to_image();
                let b = image::imageops::crop_imm(&back, x, y, cw, ch).to_image();
                let mut both = RgbaImage::from_pixel(cw, ch * 2, Rgba([255, 255, 255, 255]));
                image::imageops::overlay(&mut both, &a, 0, 0);
                image::imageops::overlay(&mut both, &b, 0, ch as i64);
                both.save(format!("{dir}/{p:?}-crop.png")).expect("write");
            }
        }
    }

    /// Trace a picture with one preset and write the shapes as an SVG in
    /// the picture's pixels, for comparing with a reference trace:
    /// `TRACE_IN=pic.jpg TRACE_SVG=out.svg TRACE_PRESET=LineArt cargo test
    /// --release -p graphdraw trace_to_svg -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn trace_to_svg() {
        let (Ok(inp), Ok(out)) = (std::env::var("TRACE_IN"), std::env::var("TRACE_SVG")) else {
            return;
        };
        let name = std::env::var("TRACE_PRESET").unwrap_or_else(|_| "LineArt".into());
        let preset = Preset::ALL
            .into_iter()
            .find(|p| format!("{p:?}") == name)
            .expect("a preset name");
        let img = image::open(&inp).expect("picture").to_rgba8();
        let start = std::time::Instant::now();
        let t = trace(&img, &preset.settings());
        let ms = start.elapsed().as_millis();
        let mut svg = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {} {}\">\n",
            img.width(),
            img.height()
        );
        for s in &t {
            let [r, g, b] = s.color.to_rgb8();
            let d = graphdraw_io::svg::path_data(&s.path);
            if s.is_stroke() {
                svg += &format!(
                    "<path d=\"{d}\" fill=\"none\" stroke=\"#{r:02x}{g:02x}{b:02x}\" stroke-width=\"{}\" stroke-linecap=\"round\" stroke-linejoin=\"round\"/>\n",
                    s.width
                );
            } else {
                svg += &format!("<path d=\"{d}\" fill=\"#{r:02x}{g:02x}{b:02x}\" fill-rule=\"evenodd\"/>\n");
            }
        }
        svg += "</svg>\n";
        std::fs::write(&out, svg).expect("write");
        let nodes: usize = t.iter().map(|t| t.path.elements().len()).sum();
        println!("{name}: {} shapes, {nodes} elements, {ms} ms", t.len());
    }

    /// The whole way a user goes: import a picture, Quick Trace it (the
    /// original deleted), export the page as SVG. The SVG's view box is
    /// then set to the picture's place on the page, for comparing with a
    /// reference trace of the picture:
    /// `TRACE_IN=pic.jpg TRACE_SVG=out.svg cargo test --release -p
    /// graphdraw quick_trace_to_svg -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn quick_trace_to_svg() {
        let (Ok(inp), Ok(out)) = (std::env::var("TRACE_IN"), std::env::var("TRACE_SVG")) else {
            return;
        };
        let mut app = crate::app::App::headless();
        app.settings.tracing.delete_original = true;
        app.import_bitmap(std::path::Path::new(&inp));
        let Some((_, rect, ..)) = app.bitmap_shapes().into_iter().next() else {
            panic!("no picture imported: {}", app.status);
        };
        app.quick_trace();
        let path = std::path::Path::new(&out);
        graphdraw_io::save_svg(app.doc(), 0, path).expect("export");
        let svg = std::fs::read_to_string(path).expect("read back");
        let page = app.page_size();
        // Page y runs up, SVG y down.
        let vb = format!(
            "viewBox=\"{} {} {} {}\"",
            rect.x0,
            page.height - rect.y1,
            rect.width(),
            rect.height()
        );
        // The root tag again, framed on the picture.
        let start = svg.find("<svg").expect("a root");
        let end = start + svg[start..].find('>').expect("its end") + 1;
        let svg = format!(
            "{}<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}mm\" height=\"{}mm\" {vb}>{}",
            &svg[..start],
            rect.width(),
            rect.height(),
            &svg[end..]
        );
        std::fs::write(path, svg).expect("write");
        let shapes: usize = app.doc().all_layers().map(|l| l.shapes.len()).sum();
        println!("exported: {shapes} top-level shapes, {vb}");
    }

    #[test]
    fn contours_of_a_square_with_hole() {
        let w = 10;
        let mut mask = vec![false; 100];
        for y in 2..8 {
            for x in 2..8 {
                mask[y * w + x] = true;
            }
        }
        for y in 4..6 {
            for x in 4..6 {
                mask[y * w + x] = false;
            }
        }
        let rings = contours(&mask, w, 10);
        assert_eq!(rings.len(), 2);
    }
}
