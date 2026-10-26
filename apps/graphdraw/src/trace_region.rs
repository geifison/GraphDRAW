//! Regions of the outline tracer. The palette cuts a smooth shading into
//! bands; here the bands that only split a shading are merged back, and
//! each region gets the fill that explains its pixels: a flat colour or a
//! linear ramp with a few stops. See docs/behavior/bitmap-tracing.md.

// Small fixed-size matrix and moment arithmetic reads best with indices.
#![allow(clippy::needless_range_loop)]

use crate::trace_color::{oklab, CLEAR};
use graphdraw_core::geometry::{Point, Vec2};
use image::RgbaImage;
use std::collections::HashMap;

/// A linear colour ramp in pixel space: colour runs from `a` to `z`,
/// holding the end colours beyond them.
#[derive(Debug, Clone, PartialEq)]
pub struct Ramp {
    pub a: Point,
    pub z: Point,
    /// (position 0..1 from `a` to `z`, colour), sorted, at least two.
    pub stops: Vec<(f64, [u8; 3])>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RegionFill {
    Flat([u8; 3]),
    Ramp(Ramp),
}

/// How hard the merging works: the largest mean colour step across a
/// border that can still be inside one shading, and the mean squared
/// colour error a merge may add (Oklab units for the step, sRGB 0..1
/// for the error).
#[derive(Debug, Clone, Copy)]
pub struct MergeParams {
    pub soft_step: f64,
    pub tolerance: f64,
}

impl MergeParams {
    /// More detail keeps more borders.
    pub fn for_detail(detail: f64) -> Self {
        let d = 1.0 - detail.clamp(0.0, 100.0) / 100.0;
        MergeParams {
            soft_step: 0.03 + 0.03 * d,
            tolerance: 0.012 + 0.02 * d,
        }
    }
}

/// Sums over a set of pixels for a least-squares colour plane: colour as
/// `c0 + gx x + gy y` per sRGB channel (0..1). Fountains mix their
/// colours straight in sRGB, so a shading drawn by one is a plane here,
/// though it is curved in Oklab. Sums add, so the plane of a
/// union comes from the sums of its parts.
#[derive(Debug, Clone, Copy, Default)]
struct Moments {
    n: f64,
    sx: f64,
    sy: f64,
    sxx: f64,
    sxy: f64,
    syy: f64,
    sc: [f64; 3],
    scx: [f64; 3],
    scy: [f64; 3],
    scc: [f64; 3],
    /// Sums of `u^a v^b` (a + b <= 4) over normalised coordinates (about
    /// -1..1 over the picture), for the quadratic surface.
    uv: [[f64; 5]; 5],
    /// Sums of each channel times the quadratic basis.
    cq: [[f64; 6]; 3],
}

/// The quadratic basis `1, u, v, u^2, u v, v^2` as powers of (u, v).
const QUAD: [(usize, usize); 6] = [(0, 0), (1, 0), (0, 1), (2, 0), (1, 1), (0, 2)];

impl Moments {
    /// Add a pixel at (`x`, `y`) in pixels, at (`u`, `v`) normalised.
    fn add(&mut self, x: f64, y: f64, u: f64, v: f64, c: [f32; 3]) {
        let (mut pu, mut pv) = ([1.0; 5], [1.0; 5]);
        for e in 1..5 {
            pu[e] = pu[e - 1] * u;
            pv[e] = pv[e - 1] * v;
        }
        for a in 0..5 {
            for b in 0..5 - a {
                self.uv[a][b] += pu[a] * pv[b];
            }
        }
        for k in 0..3 {
            for (i, (a, b)) in QUAD.iter().enumerate() {
                self.cq[k][i] += c[k] as f64 * pu[*a] * pv[*b];
            }
        }
        self.n += 1.0;
        self.sx += x;
        self.sy += y;
        self.sxx += x * x;
        self.sxy += x * y;
        self.syy += y * y;
        for k in 0..3 {
            let v = c[k] as f64;
            self.sc[k] += v;
            self.scx[k] += v * x;
            self.scy[k] += v * y;
            self.scc[k] += v * v;
        }
    }

    fn merged(&self, o: &Moments) -> Moments {
        let mut m = *self;
        m.n += o.n;
        m.sx += o.sx;
        m.sy += o.sy;
        m.sxx += o.sxx;
        m.sxy += o.sxy;
        m.syy += o.syy;
        for k in 0..3 {
            m.sc[k] += o.sc[k];
            m.scx[k] += o.scx[k];
            m.scy[k] += o.scy[k];
            m.scc[k] += o.scc[k];
            for i in 0..6 {
                m.cq[k][i] += o.cq[k][i];
            }
        }
        for a in 0..5 {
            for b in 0..5 - a {
                m.uv[a][b] += o.uv[a][b];
            }
        }
        m
    }

    /// Summed squared error of the best quadratic colour surface: a
    /// shading that bends a little (a ramp with several stops) still fits.
    fn quad_sse(&self) -> f64 {
        let n = self.uv[0][0];
        if n < 1.0 {
            return 0.0;
        }
        // Normal equations, a touch of ridge for thin regions.
        let mut g = [[0.0f64; 9]; 6];
        for (i, (ai, bi)) in QUAD.iter().enumerate() {
            for (j, (aj, bj)) in QUAD.iter().enumerate() {
                g[i][j] = self.uv[ai + aj][bi + bj];
            }
            g[i][i] += 1e-7 * n;
            for k in 0..3 {
                g[i][6 + k] = self.cq[k][i];
            }
        }
        // Gauss-Jordan with partial pivoting.
        for col in 0..6 {
            let piv = (col..6)
                .max_by(|a, b| g[*a][col].abs().total_cmp(&g[*b][col].abs()))
                .unwrap_or(col);
            g.swap(col, piv);
            let d = g[col][col];
            if d.abs() < 1e-12 {
                continue;
            }
            for c in col..9 {
                g[col][c] /= d;
            }
            for r in 0..6 {
                if r != col {
                    let f = g[r][col];
                    if f != 0.0 {
                        for c in col..9 {
                            g[r][c] -= f * g[col][c];
                        }
                    }
                }
            }
        }
        let mut sse = 0.0;
        for k in 0..3 {
            let fit: f64 = (0..6).map(|i| g[i][6 + k] * self.cq[k][i]).sum();
            sse += (self.scc[k] - fit).max(0.0);
        }
        sse
    }

    fn mean(&self) -> [f32; 3] {
        let n = self.n.max(1.0);
        [
            (self.sc[0] / n) as f32,
            (self.sc[1] / n) as f32,
            (self.sc[2] / n) as f32,
        ]
    }

    fn centroid(&self) -> Point {
        let n = self.n.max(1.0);
        Point::new(self.sx / n, self.sy / n)
    }

    /// The colour gradient per channel and the summed squared error of
    /// the best plane.
    fn plane(&self) -> ([[f64; 2]; 3], f64) {
        let n = self.n;
        if n < 1.0 {
            return ([[0.0; 2]; 3], 0.0);
        }
        let (mx, my) = (self.sx / n, self.sy / n);
        // A little regularisation keeps thin regions (a line of pixels)
        // solvable: no gradient across what cannot be measured.
        let vxx = self.sxx / n - mx * mx + 1e-3;
        let vyy = self.syy / n - my * my + 1e-3;
        let vxy = self.sxy / n - mx * my;
        let det = vxx * vyy - vxy * vxy;
        let mut g = [[0.0; 2]; 3];
        let mut sse = 0.0;
        for k in 0..3 {
            let mc = self.sc[k] / n;
            let var = (self.scc[k] / n - mc * mc).max(0.0);
            let bx = self.scx[k] / n - mx * mc;
            let by = self.scy[k] / n - my * mc;
            let (gx, gy) = if det > 1e-9 {
                ((vyy * bx - vxy * by) / det, (vxx * by - vxy * bx) / det)
            } else {
                (0.0, 0.0)
            };
            g[k] = [gx, gy];
            sse += n * (var - (gx * bx + gy * by)).max(0.0);
        }
        (g, sse)
    }
}

/// Union-find over component ids.
fn find(parent: &mut [u32], mut i: u32) -> u32 {
    while parent[i as usize] != i {
        let p = parent[parent[i as usize] as usize];
        parent[i as usize] = p;
        i = p;
    }
    i
}

/// Regions of a cleaned palette label map, with soft borders merged.
/// Returns the region of every pixel ([`CLEAR`] for transparent ones)
/// and each region's fill. `img` gives the colours the fills are fitted
/// to.
pub fn regions(
    img: &RgbaImage,
    labels: &[u32],
    w: usize,
    h: usize,
    p: MergeParams,
) -> (Vec<u32>, Vec<RegionFill>) {
    if labels.len() != w * h || img.width() as usize != w || img.height() as usize != h {
        return (vec![CLEAR; labels.len()], Vec::new());
    }
    let mut cache = HashMap::<[u8; 3], [f32; 3]>::new();
    let lab: Vec<[f32; 3]> = img
        .pixels()
        .map(|q| {
            let c = [q[0], q[1], q[2]];
            *cache.entry(c).or_insert_with(|| oklab(c))
        })
        .collect();
    let rgb: Vec<[f32; 3]> = img
        .pixels()
        .map(|q| [q[0], q[1], q[2]].map(|v| v as f32 / 255.0))
        .collect();
    // Normalised coordinates keep the quadratic sums well conditioned.
    let (cx, cy) = (w as f64 / 2.0, h as f64 / 2.0);
    let half = (w.max(h) as f64 / 2.0).max(1.0);
    // 1. Connected pieces of one palette colour.
    let mut comp = vec![CLEAR; w * h];
    let mut moments: Vec<Moments> = Vec::new();
    let mut stack = Vec::new();
    for start in 0..w * h {
        if labels[start] == CLEAR || comp[start] != CLEAR {
            continue;
        }
        let id = moments.len() as u32;
        let l = labels[start];
        let mut m = Moments::default();
        comp[start] = id;
        stack.push(start);
        while let Some(i) = stack.pop() {
            let (x, y) = (i % w, i / w);
            let (px, py) = (x as f64 + 0.5, y as f64 + 0.5);
            m.add(px, py, (px - cx) / half, (py - cy) / half, rgb[i]);
            let ns = [
                (x > 0).then(|| i - 1),
                (x + 1 < w).then(|| i + 1),
                (y > 0).then(|| i - w),
                (y + 1 < h).then(|| i + w),
            ];
            for j in ns.into_iter().flatten() {
                if labels[j] == l && comp[j] == CLEAR {
                    comp[j] = id;
                    stack.push(j);
                }
            }
        }
        moments.push(m);
    }
    // 2. Colour steps across every border between two pieces.
    let mut borders: HashMap<(u32, u32), (f64, u32)> = HashMap::new();
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let a = comp[i];
            if a == CLEAR {
                continue;
            }
            for j in [(x + 1 < w).then(|| i + 1), (y + 1 < h).then(|| i + w)]
                .into_iter()
                .flatten()
            {
                let b = comp[j];
                if b == CLEAR || b == a {
                    continue;
                }
                let d = dist(lab[i], lab[j]);
                let e = borders.entry((a.min(b), a.max(b))).or_insert((0.0, 0));
                e.0 += d;
                e.1 += 1;
            }
        }
    }
    let mut soft: Vec<(f64, u32, u32)> = borders
        .iter()
        .filter_map(|(&(a, b), &(sum, n))| {
            let step = sum / n.max(1) as f64;
            (step <= p.soft_step).then_some((step, a, b))
        })
        .collect();
    soft.sort_by(|x, y| x.0.total_cmp(&y.0).then(x.1.cmp(&y.1)).then(x.2.cmp(&y.2)));
    // 3. Merge across soft borders, softest first, while one colour plane
    // still explains the union about as well as the parts.
    let mut parent: Vec<u32> = (0..moments.len() as u32).collect();
    let mut sse: Vec<f64> = moments.iter().map(|m| m.quad_sse()).collect();
    let tol = p.tolerance * p.tolerance;
    for _ in 0..4 {
        let mut merged = false;
        for &(_, a, b) in &soft {
            let (ra, rb) = (find(&mut parent, a), find(&mut parent, b));
            if ra == rb {
                continue;
            }
            let u = moments[ra as usize].merged(&moments[rb as usize]);
            let su = u.quad_sse();
            let added = su - sse[ra as usize] - sse[rb as usize];
            if added <= tol * u.n {
                parent[rb as usize] = ra;
                moments[ra as usize] = u;
                sse[ra as usize] = su;
                merged = true;
            }
        }
        if !merged {
            break;
        }
    }
    // 4. Number the regions and fit their fills.
    let mut index = vec![CLEAR; moments.len()];
    let mut roots = Vec::new();
    for c in 0..moments.len() as u32 {
        let r = find(&mut parent, c);
        if index[r as usize] == CLEAR {
            index[r as usize] = roots.len() as u32;
            roots.push(r);
        }
    }
    let region: Vec<u32> = comp
        .iter()
        .map(|c| {
            if *c == CLEAR {
                CLEAR
            } else {
                index[find(&mut parent, *c) as usize]
            }
        })
        .collect();
    let fills = fit_fills(&region, &rgb, w, roots.iter().map(|r| moments[*r as usize]).collect());
    (region, fills)
}

fn dist(a: [f32; 3], b: [f32; 3]) -> f64 {
    let d = [
        (a[0] - b[0]) as f64,
        (a[1] - b[1]) as f64,
        (a[2] - b[2]) as f64,
    ];
    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
}

/// Colour change below this across a whole region is not worth a ramp.
const FLAT_RANGE: f64 = 0.025;
/// Most stops a fitted ramp keeps.
const MAX_STOPS: usize = 8;
/// A stop is dropped when the colours either side of it, mixed, are
/// within this of it.
const STOP_TOLERANCE: f64 = 0.01;

fn fit_fills(region: &[u32], rgb: &[[f32; 3]], w: usize, moments: Vec<Moments>) -> Vec<RegionFill> {
    let k = moments.len();
    // Direction of fastest colour change: the main axis of the gradient
    // vectors of the three channels.
    let mut axis: Vec<Option<(Point, Vec2)>> = Vec::with_capacity(k);
    for m in &moments {
        let (g, _) = m.plane();
        let (mut a, mut b, mut c) = (0.0, 0.0, 0.0);
        for gk in g {
            a += gk[0] * gk[0];
            b += gk[0] * gk[1];
            c += gk[1] * gk[1];
        }
        // Largest eigenvalue of [[a, b], [b, c]] and its vector.
        let tr = a + c;
        let disc = ((a - c) * (a - c) / 4.0 + b * b).sqrt();
        let l = tr / 2.0 + disc;
        let v = if b.abs() > 1e-12 {
            Vec2::new(l - c, b)
        } else if a >= c {
            Vec2::new(1.0, 0.0)
        } else {
            Vec2::new(0.0, 1.0)
        };
        let len = v.hypot();
        axis.push((l > 0.0 && len > 0.0 && m.n >= 16.0).then(|| (m.centroid(), v / len)));
    }
    // Extent of each region along its axis.
    let mut range = vec![(f64::MAX, f64::MIN); k];
    for (i, r) in region.iter().enumerate() {
        let Some(Some((c, u))) = axis.get(*r as usize) else {
            continue;
        };
        let t = (Point::new((i % w) as f64 + 0.5, (i / w) as f64 + 0.5) - *c).dot(*u);
        let e = &mut range[*r as usize];
        e.0 = e.0.min(t);
        e.1 = e.1.max(t);
    }
    // Bins of mean colour along the axis.
    let bins_of = |r: usize| -> usize {
        let (t0, t1) = range[r];
        (((t1 - t0) / 2.0).ceil() as usize).clamp(2, 64)
    };
    let mut bins: Vec<Vec<[f64; 4]>> = (0..k)
        .map(|r| {
            if axis[r].is_some() && range[r].1 > range[r].0 {
                vec![[0.0; 4]; bins_of(r)]
            } else {
                Vec::new()
            }
        })
        .collect();
    for (i, r) in region.iter().enumerate() {
        let r = *r as usize;
        let Some(Some((c, u))) = axis.get(r) else {
            continue;
        };
        let bs = &mut bins[r];
        if bs.is_empty() {
            continue;
        }
        let (t0, t1) = range[r];
        let t = (Point::new((i % w) as f64 + 0.5, (i / w) as f64 + 0.5) - *c).dot(*u);
        let nb = bs.len();
        let f = ((t - t0) / (t1 - t0) * nb as f64).floor() as usize;
        let b = &mut bs[f.min(nb - 1)];
        for ch in 0..3 {
            b[ch] += rgb[i][ch] as f64;
        }
        b[3] += 1.0;
    }
    let mut out = Vec::with_capacity(k);
    for r in 0..k {
        let mean = moments[r].mean();
        let flat = RegionFill::Flat(to_u8(mean));
        let (Some((c, u)), false) = (axis[r], bins[r].is_empty()) else {
            out.push(flat);
            continue;
        };
        let (t0, t1) = range[r];
        let n = bins[r].len();
        let mut pts: Vec<(f64, [f64; 3])> = bins[r]
            .iter()
            .enumerate()
            .filter(|(_, b)| b[3] > 0.0)
            .map(|(j, b)| {
                (
                    (j as f64 + 0.5) / n as f64,
                    [b[0] / b[3], b[1] / b[3], b[2] / b[3]],
                )
            })
            .collect();
        if pts.len() < 2 {
            out.push(flat);
            continue;
        }
        // A light [1 2 1] pass against noise in sparse bins.
        let raw = pts.clone();
        for j in 1..pts.len().saturating_sub(1) {
            for ch in 0..3 {
                pts[j].1[ch] = (raw[j - 1].1[ch] + 2.0 * raw[j].1[ch] + raw[j + 1].1[ch]) / 4.0;
            }
        }
        let spread = pts
            .iter()
            .map(|p| dist3(p.1, pts[0].1))
            .fold(0.0, f64::max);
        if spread < FLAT_RANGE {
            out.push(flat);
            continue;
        }
        let keep = simplify(&pts);
        // The ends sit on the first and last bins' centres: positions
        // run 0..1 between them.
        let (p0, p1) = (pts[0].0, pts[pts.len() - 1].0);
        let span = (p1 - p0).max(1e-9);
        let stops: Vec<(f64, [u8; 3])> = keep
            .iter()
            .map(|&j| {
                let (pos, col) = pts[j];
                (
                    ((pos - p0) / span).clamp(0.0, 1.0),
                    to_u8([col[0] as f32, col[1] as f32, col[2] as f32]),
                )
            })
            .collect();
        let at = |f: f64| c + u * (t0 + (t1 - t0) * f);
        out.push(RegionFill::Ramp(Ramp {
            a: at(p0),
            z: at(p1),
            stops,
        }));
    }
    out
}

/// sRGB 0..1 to bytes.
fn to_u8(c: [f32; 3]) -> [u8; 3] {
    c.map(|v| (v * 255.0).round().clamp(0.0, 255.0) as u8)
}

fn dist3(a: [f64; 3], b: [f64; 3]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

/// The samples to keep as stops: the ends, then repeatedly the sample
/// farthest from the straight mix of its kept neighbours, until all are
/// within [`STOP_TOLERANCE`] or [`MAX_STOPS`] are kept.
fn simplify(pts: &[(f64, [f64; 3])]) -> Vec<usize> {
    let mut keep = vec![0, pts.len() - 1];
    while keep.len() < MAX_STOPS {
        let mut worst: Option<(f64, usize)> = None;
        for win in keep.windows(2) {
            let (a, b) = (win[0], win[1]);
            for (j, p) in pts.iter().enumerate().take(b).skip(a + 1) {
                let f = (p.0 - pts[a].0) / (pts[b].0 - pts[a].0).max(1e-12);
                let mix = [0, 1, 2].map(|ch| pts[a].1[ch] + (pts[b].1[ch] - pts[a].1[ch]) * f);
                let d = dist3(p.1, mix);
                if worst.is_none_or(|w| d > w.0) {
                    worst = Some((d, j));
                }
            }
        }
        match worst {
            Some((d, j)) if d > STOP_TOLERANCE => {
                let at = keep.partition_point(|k| *k < j);
                keep.insert(at, j);
            }
            _ => break,
        }
    }
    keep
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgba};

    #[test]
    fn a_plane_fits_a_linear_ramp_exactly() {
        let mut m = Moments::default();
        for y in 0..10 {
            for x in 0..20 {
                let v = 0.2 + 0.01 * x as f32;
                m.add(x as f64, y as f64, x as f64 / 20.0, y as f64 / 20.0, [v, 0.0, 0.0]);
            }
        }
        let (g, sse) = m.plane();
        assert!((g[0][0] - 0.01).abs() < 1e-6 && g[0][1].abs() < 1e-6, "{g:?}");
        assert!(sse < 1e-3, "{sse}");
    }

    #[test]
    fn bands_of_a_shading_merge_into_one_ramp() {
        // A horizontal ramp from red to blue, cut by the palette into
        // bands: one region comes back, with a ramp running left to right.
        let img: RgbaImage = ImageBuffer::from_fn(120, 40, |x, _| {
            let t = x as f32 / 119.0;
            Rgba([
                (220.0 * (1.0 - t) + 20.0 * t) as u8,
                30,
                (30.0 * (1.0 - t) + 220.0 * t) as u8,
                255,
            ])
        });
        let pal = crate::trace_color::palette(&img, 8);
        let labels = crate::trace_color::label(&img, &pal);
        let (region, fills) = regions(&img, &labels, 120, 40, MergeParams::for_detail(50.0));
        assert_eq!(fills.len(), 1, "{fills:?}");
        assert!(region.iter().all(|r| *r == 0));
        let RegionFill::Ramp(r) = &fills[0] else {
            panic!("{fills:?}");
        };
        assert!(r.z.x - r.a.x > 100.0 && (r.z.y - r.a.y).abs() < 2.0, "{r:?}");
        let first = r.stops[0].1;
        let last = r.stops[r.stops.len() - 1].1;
        assert!(first[0] > 190 && first[2] < 70, "{first:?}");
        assert!(last[2] > 190 && last[0] < 70, "{last:?}");
    }

    #[test]
    fn a_hard_edge_keeps_two_flat_regions() {
        let img: RgbaImage = ImageBuffer::from_fn(60, 30, |x, _| {
            if x < 30 {
                Rgba([200, 40, 40, 255])
            } else {
                Rgba([40, 40, 200, 255])
            }
        });
        let pal = crate::trace_color::palette(&img, 4);
        let labels = crate::trace_color::label(&img, &pal);
        let (_, fills) = regions(&img, &labels, 60, 30, MergeParams::for_detail(50.0));
        assert_eq!(fills.len(), 2);
        assert!(fills.iter().all(|f| matches!(f, RegionFill::Flat(_))), "{fills:?}");
    }

    #[test]
    fn mismatched_sizes_give_nothing() {
        let img = RgbaImage::new(4, 4);
        let (r, f) = regions(&img, &[0; 3], 4, 4, MergeParams::for_detail(50.0));
        assert!(f.is_empty() && r.iter().all(|r| *r == CLEAR));
    }
}
