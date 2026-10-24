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
}

/// One traced region or stroke.
pub struct Traced {
    pub path: BezPath,
    pub color: Color,
    /// The path is closed (a filled region, or a stroke that loops).
    pub closed: bool,
    /// Stroke width in pixels for a centreline; 0 for a filled region.
    pub width: f64,
}

impl Traced {
    pub fn is_stroke(&self) -> bool {
        self.width > 0.0
    }
}

/// Trace `img` into shapes whose coordinates are in pixel space (y down).
/// The caller maps them into the bitmap's rectangle.
pub fn trace(img: &RgbaImage, s: &Settings) -> Vec<Traced> {
    if s.centerline {
        return crate::trace_center::centerline(img, s);
    }
    outline(img, s)
}

/// Fitting settings for `s`: detail sets the tolerance, smoothing the
/// passes over the points, corner smoothness the corner angle.
pub fn fit_params(s: &Settings) -> crate::trace_fit::FitParams {
    let detail = s.detail.clamp(0.0, 100.0);
    crate::trace_fit::FitParams {
        tolerance: 0.25 + (100.0 - detail) / 100.0 * 1.5,
        corner_angle: (50.0 + s.corner_smoothness.clamp(0.0, 100.0) * 0.85).to_radians(),
        smooth_passes: (s.smoothing.clamp(0.0, 100.0) / 25.0).round() as usize,
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

fn outline(img: &RgbaImage, s: &Settings) -> Vec<Traced> {
    use crate::trace_color::{self as tc, CLEAR};
    let (w, h) = (img.width() as usize, img.height() as usize);
    if w == 0 || h == 0 {
        return Vec::new();
    }
    // 1. Edge-preserving smoothing when detail is low: grain and
    // compression noise go, edges and corners stay.
    let src = if s.detail < 70.0 {
        let radius = if s.detail < 35.0 { 2 } else { 1 };
        tc::bilateral(img, radius, 12.0 + (70.0 - s.detail as f32) * 0.6)
    } else {
        img.clone()
    };
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
    let (palette, mut labels) = if s.black_white {
        let t = tc::otsu(&src);
        let labels = src
            .pixels()
            .map(|p| {
                if p[3] < 128 {
                    CLEAR
                } else if luma(*p) < t {
                    0
                } else {
                    1
                }
            })
            .collect();
        (vec![[0, 0, 0], [255, 255, 255]], labels)
    } else {
        let pal = tc::palette(&src, s.colors.clamp(2, 256) as usize);
        let labels = tc::label(&src, &pal);
        (pal, labels)
    };
    // 4. Specks and anti-aliasing fringes join their neighbours.
    tc::clean(
        &mut labels,
        w,
        h,
        &palette,
        speck_area(s.detail, w, h),
        !s.black_white,
        flood.as_ref().map(|f| f.1),
    );
    // 5. Without a flooded background, the background is the most common
    // colour touching the border.
    let mut area = vec![0usize; palette.len()];
    let mut border = vec![0usize; palette.len()];
    for (i, l) in labels.iter().enumerate() {
        let Some(a) = area.get_mut(*l as usize) else {
            continue;
        };
        *a += 1;
        let (x, y) = (i % w, i / w);
        if x == 0 || y == 0 || x + 1 == w || y + 1 == h {
            border[*l as usize] += 1;
        }
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
    // 6. Stack largest first: each layer also covers the pixels of the
    // layers above it that it surrounds or borders, so neighbouring
    // colours overlap instead of leaving hairline gaps.
    let mut order: Vec<usize> = (0..palette.len())
        .filter(|i| area[*i] > 0 && Some(*i) != background)
        .collect();
    order.sort_by_key(|i| std::cmp::Reverse(area[*i]));
    let mut rank = vec![-1i64; palette.len()];
    for (r, i) in order.iter().enumerate() {
        rank[*i] = r as i64;
    }
    let rank_of = |l: u32| -> i64 { rank.get(l as usize).copied().unwrap_or(-1) };
    let fp = fit_params(s);
    let mut out = Vec::new();
    for &ci in &order {
        let my = rank[ci];
        let Some((x0, y0, x1, y1)) = bbox(&labels, w, h, ci as u32) else {
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
        for ring in contours(&mask, cw, ch) {
            let pts: Vec<Point> = ring_midpoints(&ring)
                .into_iter()
                .map(|p| Point::new(p.x + x0 as f64 - 1.0, p.y + y0 as f64 - 1.0))
                .collect();
            let sub = crate::trace_fit::fit_closed(&pts, &fp);
            if sub.area().abs() >= 0.5 {
                path.extend(sub);
            }
        }
        if path.elements().is_empty() {
            continue;
        }
        let c = palette[ci];
        out.push(Traced {
            path,
            color: Color::rgb8(c[0], c[1], c[2]),
            closed: true,
            width: 0.0,
        });
    }
    out
}

/// Bounding box (x0, y0, x1, y1 inclusive) of the pixels labelled `l`.
fn bbox(labels: &[u32], w: usize, h: usize, l: u32) -> Option<(usize, usize, usize, usize)> {
    let mut b: Option<(usize, usize, usize, usize)> = None;
    for y in 0..h {
        for x in 0..w {
            if labels[y * w + x] == l {
                b = Some(match b {
                    None => (x, y, x, y),
                    Some((a, c, d, e)) => (a.min(x), c.min(y), d.max(x), e.max(y)),
                });
            }
        }
    }
    b
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
fn ring_midpoints(ring: &[Point]) -> Vec<Point> {
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
    let sx = rect.width() / img_w.max(1) as f64;
    let sy = rect.height() / img_h.max(1) as f64;
    graphdraw_core::geometry::Affine::new([sx, 0.0, 0.0, -sy, rect.x0, rect.y1]) * path.clone()
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
        let img = image::open(&inp).expect("picture").to_rgba8();
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
        }
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
