//! Centreline tracing: dark strokes become open (or looping) curves with
//! the stroke width they had in the picture.
//!
//! 1. Ink is what is darker than the threshold that best splits the
//!    picture's tones; specks are dropped and pinholes filled.
//! 2. A distance transform gives the half-width at every ink pixel.
//! 3. The ink is thinned to a one pixel skeleton, which is read as a
//!    graph: ends, junctions and the runs between them.
//! 4. Short spurs that thinning grows at corners are pruned; at each
//!    junction the runs that continue each other most straightly are
//!    joined, so a crossing gives two strokes, not four.
//! 5. Each stroke is fitted with curves, coloured with the mean colour of
//!    its pixels and given twice its mean half-width.
//!
//! See docs/behavior/bitmap-tracing.md.

use crate::trace::{Settings, Traced};
use graphdraw_core::{geometry::Point, Color};
use image::RgbaImage;
use std::collections::{HashMap, HashSet};

/// Ring offsets, clockwise from north (y grows down).
const RING: [(i64, i64); 8] = [
    (0, -1),
    (1, -1),
    (1, 0),
    (1, 1),
    (0, 1),
    (-1, 1),
    (-1, 0),
    (-1, -1),
];

struct Grid {
    w: usize,
    h: usize,
}

impl Grid {
    fn at(&self, m: &[bool], x: i64, y: i64) -> bool {
        x >= 0 && y >= 0 && (x as usize) < self.w && (y as usize) < self.h
            && m[y as usize * self.w + x as usize]
    }
    fn neighbours(&self, m: &[bool], i: usize) -> Vec<usize> {
        let (x, y) = ((i % self.w) as i64, (i / self.w) as i64);
        RING.iter()
            .filter(|(dx, dy)| self.at(m, x + dx, y + dy))
            .map(|(dx, dy)| (y + dy) as usize * self.w + (x + dx) as usize)
            .collect()
    }
    fn centre(&self, i: usize) -> Point {
        Point::new((i % self.w) as f64 + 0.5, (i / self.w) as f64 + 0.5)
    }
}

pub fn centerline(img: &RgbaImage, s: &Settings) -> Vec<Traced> {
    let (w, h) = (img.width() as usize, img.height() as usize);
    if w == 0 || h == 0 {
        return Vec::new();
    }
    let g = Grid { w, h };
    let t = crate::trace_color::otsu(img);
    let dark: Vec<bool> = img
        .pixels()
        .map(|p| p[3] >= 128 && crate::trace::luma(*p) < t)
        .collect();
    // Ink is whichever tone the border is not: light lines on a dark
    // ground trace as well as dark lines on paper.
    let (mut on_dark, mut on_border) = (0usize, 0usize);
    for (i, d) in dark.iter().enumerate() {
        let (x, y) = (i % w, i / w);
        if (x == 0 || y == 0 || x + 1 == w || y + 1 == h) && img.as_raw()[i * 4 + 3] >= 128 {
            on_border += 1;
            on_dark += *d as usize;
        }
    }
    let light_ink = on_dark * 2 > on_border;
    let mut ink: Vec<bool> = if light_ink {
        img.pixels()
            .zip(&dark)
            .map(|(p, d)| p[3] >= 128 && !d)
            .collect()
    } else {
        dark
    };
    let min_area = crate::trace::speck_area(s.detail, w, h);
    drop_small(&mut ink, &g, min_area, true);
    drop_small(&mut ink, &g, min_area, false);
    let half = distance(&ink, &g);
    let mut skel = ink.clone();
    crate::trace::thin(&mut skel, w, h);
    tidy(&mut skel, &g);
    let mut strokes = graph_strokes(&skel, &g);
    // Spurs shorter than the stroke is wide are thinning artefacts.
    let mean_half = {
        let v: Vec<f64> = (0..skel.len()).filter(|i| skel[*i]).map(|i| half[i]).collect();
        if v.is_empty() {
            1.0
        } else {
            v.iter().sum::<f64>() / v.len() as f64
        }
    };
    strokes.prune((mean_half * 3.0).max(3.0) as usize);
    let mut fp = crate::trace::fit_params(s);
    fp.smooth_passes = fp.smooth_passes.max(1);
    let mut out = Vec::new();
    for (pixels, closed) in strokes.chains() {
        if pixels.len() < 2 {
            continue;
        }
        let pts: Vec<Point> = pixels.iter().map(|i| g.centre(*i)).collect();
        let path = if closed {
            crate::trace_fit::fit_closed(&pts, &fp)
        } else {
            crate::trace_fit::fit_open(&pts, &fp)
        };
        if path.elements().is_empty() {
            continue;
        }
        let n = pixels.len() as f64;
        let width = pixels.iter().map(|i| half[*i] * 2.0 - 1.0).sum::<f64>() / n;
        let mut c = [0f64; 3];
        for i in &pixels {
            let p = img.get_pixel((i % w) as u32, (i / w) as u32);
            for k in 0..3 {
                c[k] += p[k] as f64;
            }
        }
        let c = c.map(|v| (v / n).round().clamp(0.0, 255.0) as u8);
        out.push(Traced {
            path,
            color: Color::rgb8(c[0], c[1], c[2]),
            closed,
            width: width.clamp(1.0, 100.0),
        });
    }
    out
}

/// Remove 8-connected ink specks (`ink` true) or fill 4-connected
/// pinholes (`ink` false) smaller than `min_area`. Paper touching the
/// border is never filled.
fn drop_small(mask: &mut [bool], g: &Grid, min_area: usize, ink: bool) {
    let mut seen = vec![false; mask.len()];
    let mut stack = Vec::new();
    let mut comp = Vec::new();
    for start in 0..mask.len() {
        if mask[start] != ink || seen[start] {
            continue;
        }
        seen[start] = true;
        stack.push(start);
        comp.clear();
        let mut edge = false;
        while let Some(i) = stack.pop() {
            comp.push(i);
            let (x, y) = ((i % g.w) as i64, (i / g.w) as i64);
            if x == 0 || y == 0 || x as usize + 1 == g.w || y as usize + 1 == g.h {
                edge = true;
            }
            for (k, (dx, dy)) in RING.iter().enumerate() {
                if !ink && k % 2 == 1 {
                    continue;
                }
                let (nx, ny) = (x + dx, y + dy);
                if nx < 0 || ny < 0 || nx as usize >= g.w || ny as usize >= g.h {
                    continue;
                }
                let j = ny as usize * g.w + nx as usize;
                if mask[j] == ink && !seen[j] {
                    seen[j] = true;
                    stack.push(j);
                }
            }
        }
        if comp.len() < min_area && (ink || !edge) {
            for &i in &comp {
                mask[i] = !ink;
            }
        }
    }
}

/// Distance (pixels, chamfer 3-4) from each ink pixel centre to the
/// nearest paper pixel centre; outside the picture counts as paper.
fn distance(ink: &[bool], g: &Grid) -> Vec<f64> {
    let (w, h) = (g.w as i64, g.h as i64);
    let big = u32::MAX / 4;
    let mut d: Vec<u32> = ink.iter().map(|i| if *i { big } else { 0 }).collect();
    let get = |d: &[u32], x: i64, y: i64| -> u32 {
        if x < 0 || y < 0 || x >= w || y >= h {
            0
        } else {
            d[(y * w + x) as usize]
        }
    };
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) as usize;
            if d[i] == 0 {
                continue;
            }
            let v = [
                get(&d, x - 1, y) + 3,
                get(&d, x - 1, y - 1) + 4,
                get(&d, x, y - 1) + 3,
                get(&d, x + 1, y - 1) + 4,
            ];
            d[i] = d[i].min(v.into_iter().min().unwrap_or(big));
        }
    }
    for y in (0..h).rev() {
        for x in (0..w).rev() {
            let i = (y * w + x) as usize;
            if d[i] == 0 {
                continue;
            }
            let v = [
                get(&d, x + 1, y) + 3,
                get(&d, x + 1, y + 1) + 4,
                get(&d, x, y + 1) + 3,
                get(&d, x - 1, y + 1) + 4,
            ];
            d[i] = d[i].min(v.into_iter().min().unwrap_or(big));
        }
    }
    d.into_iter().map(|v| v as f64 / 3.0).collect()
}

/// Remove skeleton pixels that only round off a staircase: removing them
/// keeps their neighbours 8-connected and they are not line ends.
fn tidy(skel: &mut [bool], g: &Grid) {
    for i in 0..skel.len() {
        if !skel[i] {
            continue;
        }
        let (x, y) = ((i % g.w) as i64, (i / g.w) as i64);
        let ring: Vec<bool> = RING.iter().map(|(dx, dy)| g.at(skel, x + dx, y + dy)).collect();
        if ring.iter().filter(|b| **b).count() < 2 {
            continue;
        }
        if ring_components(&ring) == 1 {
            skel[i] = false;
        }
    }
}

/// 8-connected groups among a pixel's set ring neighbours.
fn ring_components(ring: &[bool]) -> usize {
    let mut seen = [false; 8];
    let mut count = 0;
    for s in 0..8 {
        if !ring[s] || seen[s] {
            continue;
        }
        count += 1;
        let mut stack = vec![s];
        seen[s] = true;
        while let Some(k) = stack.pop() {
            // Ring neighbours touch; edge cells (even) also touch the
            // next edge cell across a corner.
            let mut near = vec![(k + 1) % 8, (k + 7) % 8];
            if k % 2 == 0 {
                near.push((k + 2) % 8);
                near.push((k + 6) % 8);
            }
            for j in near {
                if ring[j] && !seen[j] {
                    seen[j] = true;
                    stack.push(j);
                }
            }
        }
    }
    count
}

/// A run of skeleton pixels between two nodes (or a loop with none).
struct Edge {
    pixels: Vec<usize>,
    /// Node clusters at the start and end; `None` for a loop.
    ends: Option<(usize, usize)>,
    alive: bool,
}

struct Strokes {
    edges: Vec<Edge>,
    /// Runs meeting at each node cluster (three or more at a junction).
    degree: Vec<usize>,
    /// Picture width, to turn pixel indices into positions.
    width: usize,
}

fn graph_strokes(skel: &[bool], g: &Grid) -> Strokes {
    let n = skel.len();
    let deg: Vec<usize> = (0..n)
        .map(|i| if skel[i] { g.neighbours(skel, i).len() } else { 0 })
        .collect();
    let is_node = |i: usize| skel[i] && deg[i] != 2;
    // Clusters of touching node pixels.
    let mut cluster = vec![usize::MAX; n];
    let mut clusters = 0;
    for i in 0..n {
        if !is_node(i) || cluster[i] != usize::MAX || deg[i] == 0 {
            continue;
        }
        let mut stack = vec![i];
        cluster[i] = clusters;
        while let Some(p) = stack.pop() {
            for q in g.neighbours(skel, p) {
                if is_node(q) && cluster[q] == usize::MAX {
                    cluster[q] = clusters;
                    stack.push(q);
                }
            }
        }
        clusters += 1;
    }
    let mut edges = Vec::new();
    let mut visited = vec![false; n];
    let mut direct: HashSet<(usize, usize)> = HashSet::new();
    for p in 0..n {
        if cluster[p] == usize::MAX {
            continue;
        }
        for q in g.neighbours(skel, p) {
            if cluster[q] != usize::MAX {
                if cluster[q] != cluster[p] && direct.insert((p.min(q), p.max(q))) {
                    edges.push(Edge {
                        pixels: vec![p, q],
                        ends: Some((cluster[p], cluster[q])),
                        alive: true,
                    });
                }
                continue;
            }
            if visited[q] {
                continue;
            }
            let mut pixels = vec![p, q];
            visited[q] = true;
            let (mut prev, mut cur) = (p, q);
            let mut end = None;
            loop {
                let next = g
                    .neighbours(skel, cur)
                    .into_iter()
                    // Back to the start node only round a loop.
                    .filter(|r| (*r != prev && *r != p) || (*r == p && pixels.len() > 2))
                    .find(|r| cluster[*r] != usize::MAX || !visited[*r]);
                let Some(r) = next else { break };
                pixels.push(r);
                if cluster[r] != usize::MAX {
                    end = Some(cluster[r]);
                    break;
                }
                visited[r] = true;
                prev = cur;
                cur = r;
            }
            let c = cluster[p];
            edges.push(Edge {
                pixels,
                ends: Some((c, end.unwrap_or(usize::MAX))),
                alive: true,
            });
        }
    }
    // What is left are loops with no node on them.
    for s in 0..n {
        if !skel[s] || visited[s] || cluster[s] != usize::MAX || deg[s] != 2 {
            continue;
        }
        let mut pixels = vec![s];
        visited[s] = true;
        let mut cur = s;
        while let Some(r) = g.neighbours(skel, cur).into_iter().find(|r| !visited[*r]) {
            visited[r] = true;
            pixels.push(r);
            cur = r;
        }
        edges.push(Edge {
            pixels,
            ends: None,
            alive: true,
        });
    }
    let mut degree = vec![0; clusters];
    for e in &edges {
        if let Some((a, b)) = e.ends {
            for c in [a, b] {
                if let Some(d) = degree.get_mut(c) {
                    *d += 1;
                }
            }
        }
    }
    Strokes {
        edges,
        degree,
        width: g.w,
    }
}

impl Strokes {
    /// Drop runs shorter than `len` pixels that go from a junction to a
    /// free end.
    fn prune(&mut self, len: usize) {
        let deg = |c: usize, d: &[usize]| d.get(c).copied().unwrap_or(1);
        for i in 0..self.edges.len() {
            let Some((a, b)) = self.edges[i].ends else {
                continue;
            };
            let (da, db) = (deg(a, &self.degree), deg(b, &self.degree));
            let spur = (da == 1 && db >= 3) || (db == 1 && da >= 3);
            if spur && self.edges[i].pixels.len() < len {
                self.edges[i].alive = false;
                for c in [a, b] {
                    if let Some(d) = self.degree.get_mut(c) {
                        *d = d.saturating_sub(1);
                    }
                }
            }
        }
    }

    /// Join the runs into strokes: at each junction, pair the run ends
    /// that continue each other most straightly. Returns each stroke's
    /// pixels and whether it closes on itself.
    fn chains(&self) -> Vec<(Vec<usize>, bool)> {
        // Run ends meeting at each cluster: (edge, 0 = start, 1 = end).
        let mut at: HashMap<usize, Vec<(usize, usize)>> = HashMap::new();
        for (i, e) in self.edges.iter().enumerate() {
            if !e.alive {
                continue;
            }
            if let Some((a, b)) = e.ends {
                at.entry(a).or_default().push((i, 0));
                if b != usize::MAX {
                    at.entry(b).or_default().push((i, 1));
                }
            }
        }
        let mut link: HashMap<(usize, usize), (usize, usize)> = HashMap::new();
        let mut keys: Vec<usize> = at.keys().copied().collect();
        keys.sort_unstable();
        for c in keys {
            let ends = &at[&c];
            if ends.len() < 2 {
                continue;
            }
            let v: Vec<(f64, f64)> = ends.iter().map(|e| self.out_dir(*e)).collect();
            let mut pairs = Vec::new();
            for i in 0..ends.len() {
                for j in i + 1..ends.len() {
                    if ends[i].0 == ends[j].0 {
                        continue;
                    }
                    let dot = v[i].0 * v[j].0 + v[i].1 * v[j].1;
                    pairs.push((dot, i, j));
                }
            }
            pairs.sort_by(|a, b| a.0.total_cmp(&b.0));
            let mut used = vec![false; ends.len()];
            for (dot, i, j) in pairs {
                // Two runs at a bend always join; at a real junction only
                // runs that carry on within 60 degrees of straight.
                if used[i] || used[j] || (ends.len() > 2 && dot > -0.5) {
                    continue;
                }
                used[i] = true;
                used[j] = true;
                link.insert(ends[i], ends[j]);
                link.insert(ends[j], ends[i]);
            }
        }
        let mut done = vec![false; self.edges.len()];
        let mut out = Vec::new();
        let walk = |first: (usize, usize), done: &mut Vec<bool>| {
            let mut pixels: Vec<usize> = Vec::new();
            let mut cur = first;
            let mut closed = false;
            loop {
                done[cur.0] = true;
                let px = &self.edges[cur.0].pixels;
                let seq: Box<dyn Iterator<Item = &usize>> = if cur.1 == 0 {
                    Box::new(px.iter())
                } else {
                    Box::new(px.iter().rev())
                };
                for p in seq {
                    if pixels.last() != Some(p) {
                        pixels.push(*p);
                    }
                }
                let exit = (cur.0, 1 - cur.1);
                match link.get(&exit) {
                    Some(&next) if next == first => {
                        closed = true;
                        break;
                    }
                    Some(&next) if !done[next.0] => cur = next,
                    _ => break,
                }
            }
            if closed && pixels.len() > 1 && pixels.first() == pixels.last() {
                pixels.pop();
            }
            (pixels, closed)
        };
        // Strokes with a free end first, then what loops.
        for i in 0..self.edges.len() {
            if !self.edges[i].alive || done[i] || self.edges[i].ends.is_none() {
                continue;
            }
            for side in 0..2 {
                if !done[i] && !link.contains_key(&(i, side)) {
                    out.push(walk((i, side), &mut done));
                }
            }
        }
        for i in 0..self.edges.len() {
            if self.edges[i].alive && !done[i] {
                if self.edges[i].ends.is_none() {
                    done[i] = true;
                    out.push((self.edges[i].pixels.clone(), true));
                } else {
                    out.push(walk((i, 0), &mut done));
                }
            }
        }
        out
    }

    /// Unit direction leaving a cluster along a run end, over up to six
    /// pixels.
    fn out_dir(&self, end: (usize, usize)) -> (f64, f64) {
        let px = &self.edges[end.0].pixels;
        let k = px.len().saturating_sub(1).min(6);
        let (a, b) = if end.1 == 0 {
            (px[0], px[k])
        } else {
            (px[px.len() - 1], px[px.len() - 1 - k])
        };
        let w = self.width.max(1);
        let (dx, dy) = (
            (b % w) as f64 - (a % w) as f64,
            (b / w) as f64 - (a / w) as f64,
        );
        let l = (dx * dx + dy * dy).sqrt();
        if l > 0.0 {
            (dx / l, dy / l)
        } else {
            (0.0, 0.0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trace::Preset;
    use graphdraw_core::geometry::Shape as _;
    use image::{ImageBuffer, Rgba};

    fn draw(w: u32, h: u32, ink: impl Fn(u32, u32) -> bool) -> RgbaImage {
        ImageBuffer::from_fn(w, h, |x, y| {
            if ink(x, y) {
                Rgba([0, 0, 0, 255])
            } else {
                Rgba([255, 255, 255, 255])
            }
        })
    }

    #[test]
    fn a_line_gives_one_stroke_with_its_width() {
        let img = draw(100, 40, |x, y| (18..=22).contains(&y) && x > 5 && x < 95);
        let t = centerline(&img, &Preset::Technical.settings());
        assert_eq!(t.len(), 1);
        let b = t[0].path.bounding_box();
        assert!(b.width() > 80.0 && b.height() < 2.0, "{b:?}");
        assert!((t[0].width - 5.0).abs() < 1.0, "{}", t[0].width);
        assert!(!t[0].closed);
        assert_eq!(t[0].path.segments().count(), 1, "a straight line");
    }

    #[test]
    fn a_cross_gives_two_strokes() {
        let img = draw(100, 100, |x, y| {
            ((48..=51).contains(&y) && (10..90).contains(&x))
                || ((48..=51).contains(&x) && (10..90).contains(&y))
        });
        let t = centerline(&img, &Preset::Technical.settings());
        assert_eq!(t.len(), 2, "{}", t.len());
        for s in &t {
            let b = s.path.bounding_box();
            assert!(b.width().max(b.height()) > 70.0, "{b:?}");
        }
    }

    #[test]
    fn a_ring_gives_one_closed_stroke() {
        let img = draw(80, 80, |x, y| {
            let d = ((x as f32 - 40.0).powi(2) + (y as f32 - 40.0).powi(2)).sqrt();
            (25.0..29.0).contains(&d)
        });
        let t = centerline(&img, &Preset::LineDrawing.settings());
        assert_eq!(t.len(), 1);
        assert!(t[0].closed);
        let area = t[0].path.area().abs();
        let r = 26.5f64;
        assert!((area / (std::f64::consts::PI * r * r) - 1.0).abs() < 0.08, "{area}");
    }

    #[test]
    fn light_lines_on_a_dark_ground_are_the_ink() {
        let img = ImageBuffer::from_fn(100, 40, |x, y| {
            if (18..=22).contains(&y) && x > 5 && x < 95 {
                Rgba([250, 250, 250, 255])
            } else {
                Rgba([20, 20, 40, 255])
            }
        });
        let t = centerline(&img, &Preset::Technical.settings());
        assert_eq!(t.len(), 1);
        assert!(t[0].color.to_rgb8()[0] > 200);
        assert!((t[0].width - 5.0).abs() < 1.0, "{}", t[0].width);
    }

    #[test]
    fn blank_pictures_give_nothing() {
        let s = Preset::Technical.settings();
        assert!(centerline(&RgbaImage::new(0, 0), &s).is_empty());
        assert!(centerline(&RgbaImage::new(5, 5), &s).is_empty());
        assert!(centerline(&draw(20, 20, |_, _| false), &s).is_empty());
    }
}
