//! Curve fitting for the bitmap tracer: a chain of points (a pixel
//! boundary or a skeleton line) becomes cubic Beziers and straight lines.
//!
//! 1. Corners are found where the direction turns sharply compared with
//!    its neighbourhood (turn over a chord of `K` points each side).
//! 2. Each corner is snapped to the crossing of the straight runs on
//!    either side, which restores square corners the pixel grid blurred.
//! 3. Points between corners are smoothed (binomial filter, corners fixed).
//! 4. Each run between corners is a line when it is straight within the
//!    tolerance, otherwise least-squares cubics with tangents fixed at the
//!    ends, refined by Newton steps on the parameters and split at the
//!    worst point until every point is within the tolerance.
//!
//! See docs/behavior/bitmap-tracing.md for the formulas.

use graphdraw_core::geometry::{BezPath, Point, Vec2};

/// How closely and how smoothly to fit.
#[derive(Debug, Clone, Copy)]
pub struct FitParams {
    /// Largest distance (pixels) from a point to the fitted curve.
    pub tolerance: f64,
    /// Smallest turn (radians) that makes a corner.
    pub corner_angle: f64,
    /// Smoothing passes over the points before fitting.
    pub smooth_passes: usize,
    /// Points per pixel of outline (an outline followed k times finer
    /// spreads a corner over k times more points).
    pub scale: usize,
}

/// Points each side of a point over which its turn is measured.
const K: usize = 3;

/// Fit a closed ring of points.
pub fn fit_closed(points: &[Point], p: &FitParams) -> BezPath {
    let mut path = BezPath::new();
    let mut pts = dedup(points, true);
    let n = pts.len();
    if n < 3 {
        return path;
    }
    let k = K * p.scale.max(1);
    if n < 2 * k + 2 {
        // Too few points to tell corners from curves: keep the polygon.
        path.move_to(pts[0]);
        for q in &pts[1..] {
            path.line_to(*q);
        }
        path.close_path();
        return path;
    }
    let corners = find_corners(&pts, true, p.corner_angle, k);
    for &c in &corners {
        snap_corner(&mut pts, c, true, k);
    }
    smooth(&mut pts, &corners, true, p.smooth_passes * p.scale.max(1));
    if corners.is_empty() {
        // One smooth loop: start anywhere, same tangent on both sides of
        // the seam.
        let mut run = pts.clone();
        run.push(pts[0]);
        let t = unit(pts[1] - pts[n - 1]);
        let mut segs = Vec::new();
        fit_run(&run, t, -t, p.tolerance, &mut segs);
        emit(&mut path, &segs, true);
        return path;
    }
    let mut segs = Vec::new();
    for (i, &a) in corners.iter().enumerate() {
        let b = corners.get(i + 1).copied().unwrap_or(corners[0] + n);
        let run: Vec<Point> = (a..=b).map(|j| pts[j % n]).collect();
        fit_between_corners(&run, p.tolerance, &mut segs);
    }
    emit(&mut path, &segs, true);
    path
}

/// Fit an open chain of points; its ends stay where they are.
pub fn fit_open(points: &[Point], p: &FitParams) -> BezPath {
    let mut path = BezPath::new();
    let mut pts = dedup(points, false);
    let n = pts.len();
    if n < 2 {
        return path;
    }
    let k = K * p.scale.max(1);
    let mut corners = if n > 2 * k {
        find_corners(&pts, false, p.corner_angle, k)
    } else {
        Vec::new()
    };
    for &c in &corners {
        snap_corner(&mut pts, c, false, k);
    }
    corners.insert(0, 0);
    corners.push(n - 1);
    smooth(&mut pts, &corners, false, p.smooth_passes * p.scale.max(1));
    let mut segs = Vec::new();
    for w in corners.windows(2) {
        fit_between_corners(&pts[w[0]..=w[1]], p.tolerance, &mut segs);
    }
    emit(&mut path, &segs, false);
    path
}

/// A fitted piece: a line or a cubic.
#[derive(Debug, Clone, Copy)]
enum Seg {
    Line(Point, Point),
    Cubic([Point; 4]),
}

fn emit(path: &mut BezPath, segs: &[Seg], closed: bool) {
    let Some(first) = segs.first() else {
        return;
    };
    path.move_to(match first {
        Seg::Line(a, _) => *a,
        Seg::Cubic(c) => c[0],
    });
    let last = segs.len() - 1;
    for (i, s) in segs.iter().enumerate() {
        match s {
            // The closing line is the close itself.
            Seg::Line(_, _) if closed && i == last => {}
            Seg::Line(_, b) => path.line_to(*b),
            Seg::Cubic(c) => path.curve_to(c[1], c[2], c[3]),
        }
    }
    if closed {
        path.close_path();
    }
}

fn unit(v: Vec2) -> Vec2 {
    let l = v.hypot();
    if l > 1e-12 {
        v / l
    } else {
        Vec2::new(1.0, 0.0)
    }
}

/// Drop repeated points (and, for a ring, a last point equal to the first).
fn dedup(points: &[Point], closed: bool) -> Vec<Point> {
    let mut out: Vec<Point> = Vec::with_capacity(points.len());
    for q in points {
        if out.last().is_none_or(|l| (*l - *q).hypot2() > 1e-12) {
            out.push(*q);
        }
    }
    if closed && out.len() > 1 && (out[0] - out[out.len() - 1]).hypot2() <= 1e-12 {
        out.pop();
    }
    out
}

/// Turn (radians, 0 = straight) at point `i`, measured over `k` points
/// each side.
fn turn_at(pts: &[Point], i: usize, k: usize, closed: bool) -> f64 {
    let n = pts.len();
    let (a, b) = if closed {
        (pts[(i + n - k % n) % n], pts[(i + k) % n])
    } else {
        if i < k || i + k >= n {
            return 0.0;
        }
        (pts[i - k], pts[i + k])
    };
    let u = pts[i] - a;
    let v = b - pts[i];
    if u.hypot2() < 1e-12 || v.hypot2() < 1e-12 {
        return 0.0;
    }
    u.cross(v).atan2(u.dot(v)).abs()
}

/// Indices of corners, ascending. A corner turns by at least `angle` and
/// by clearly more than the points `2K` away, so small circles (whose
/// turn is the same everywhere) stay smooth.
fn find_corners(pts: &[Point], closed: bool, angle: f64, k: usize) -> Vec<usize> {
    let n = pts.len();
    let turns: Vec<f64> = (0..n).map(|i| turn_at(pts, i, k, closed)).collect();
    let at = |i: isize| -> Option<f64> {
        if closed {
            Some(turns[i.rem_euclid(n as isize) as usize])
        } else if i >= 0 && (i as usize) < n {
            Some(turns[i as usize])
        } else {
            None
        }
    };
    let mut out = Vec::new();
    for (i, &t) in turns.iter().enumerate() {
        if t < angle {
            continue;
        }
        let ii = i as isize;
        // Strict local maximum within k (ties go to the first).
        let is_max = (1..=k as isize).all(|d| {
            at(ii - d).is_none_or(|v| v < t) && at(ii + d).is_none_or(|v| v <= t)
        });
        if !is_max {
            continue;
        }
        let far = 2 * k as isize;
        let base = match (at(ii - far), at(ii + far)) {
            (Some(a), Some(b)) => a.min(b),
            (Some(a), None) | (None, Some(a)) => a,
            (None, None) => 0.0,
        };
        if t - base >= angle * 0.5 {
            out.push(i);
        }
    }
    out
}

/// Move corner `c` to where the straight runs before and after it cross,
/// when that is within a pixel.
fn snap_corner(pts: &mut [Point], c: usize, closed: bool, k: usize) {
    let n = pts.len();
    let idx = |d: isize| -> Option<usize> {
        let j = c as isize + d;
        if closed {
            Some(j.rem_euclid(n as isize) as usize)
        } else if j >= 0 && (j as usize) < n {
            Some(j as usize)
        } else {
            None
        }
    };
    // The runs are taken from `k` points away to `k / 3` (about a pixel)
    // from the corner: closer in, an outline followed finer than the
    // pixels already rounds the corner and would tilt them.
    let k = k as isize;
    let near = (k / 2).max(1);
    let far = 2 * k;
    if idx(-far).is_none() || idx(far).is_none() || 2 * far >= n as isize {
        return;
    }
    // Each run as the least-squares line through its points (an outline
    // followed on a fine grid steps by a cell here and there).
    let line = |from: isize, to: isize| -> Option<(Point, Vec2)> {
        let run: Vec<Point> = (from..=to).filter_map(&idx).map(|i| pts[i]).collect();
        let n = run.len() as f64;
        if n < 2.0 {
            return None;
        }
        let m = run.iter().fold(Vec2::ZERO, |a, p| a + p.to_vec2()) / n;
        let (mut sxx, mut sxy, mut syy) = (0.0, 0.0, 0.0);
        for p in &run {
            let d = p.to_vec2() - m;
            sxx += d.x * d.x;
            sxy += d.x * d.y;
            syy += d.y * d.y;
        }
        // Main axis of the points.
        let a = 0.5 * (2.0 * sxy).atan2(sxx - syy);
        Some((m.to_point(), Vec2::new(a.cos(), a.sin())))
    };
    let (Some((p, r)), Some((q, s))) = (line(-far, -near), line(near, far)) else {
        return;
    };
    let denom = r.cross(s);
    if denom.abs() < 1e-9 {
        return;
    }
    let t = (q - p).cross(s) / denom;
    let x = p + r * t;
    if (x - pts[c]).hypot() <= 1.0 {
        pts[c] = x;
    }
}

/// Binomial smoothing; `fixed` points (sorted indices) do not move.
fn smooth(pts: &mut [Point], fixed: &[usize], closed: bool, passes: usize) {
    let n = pts.len();
    if n < 3 || passes == 0 {
        return;
    }
    let mut keep = vec![false; n];
    for &f in fixed {
        if let Some(k) = keep.get_mut(f) {
            *k = true;
        }
    }
    if !closed {
        keep[0] = true;
        keep[n - 1] = true;
    }
    let mut next = pts.to_vec();
    for _ in 0..passes {
        for i in 0..n {
            if keep[i] {
                next[i] = pts[i];
                continue;
            }
            let a = pts[(i + n - 1) % n];
            let b = pts[(i + 1) % n];
            next[i] = Point::new(
                (a.x + 2.0 * pts[i].x + b.x) / 4.0,
                (a.y + 2.0 * pts[i].y + b.y) / 4.0,
            );
        }
        pts.copy_from_slice(&next);
    }
}

/// Direction leaving `pts[0]` into the run, from points within about two
/// pixels.
fn end_tangent(pts: &[Point]) -> Vec2 {
    let p0 = pts[0];
    let mut acc = Vec2::ZERO;
    for q in pts.iter().skip(1).take(4) {
        let d = *q - p0;
        let l = d.hypot();
        if l > 1e-9 {
            acc += d / l;
        }
        if l > 2.5 {
            break;
        }
    }
    unit(acc)
}

/// Largest distance from the points to the segment `a`-`b`.
fn max_line_dist(pts: &[Point], a: Point, b: Point) -> f64 {
    let d = b - a;
    let len = d.hypot();
    pts.iter()
        .map(|q| {
            if len < 1e-9 {
                (*q - a).hypot()
            } else {
                ((*q - a).cross(d) / len).abs()
            }
        })
        .fold(0.0, f64::max)
}

fn fit_between_corners(run: &[Point], tol: f64, out: &mut Vec<Seg>) {
    let n = run.len();
    if n < 2 {
        return;
    }
    let (a, b) = (run[0], run[n - 1]);
    // Straight runs stay straight; a little stricter than curves so gentle
    // arcs are not flattened.
    if n == 2 || max_line_dist(run, a, b) <= tol * 0.6 {
        out.push(Seg::Line(a, b));
        return;
    }
    let t1 = end_tangent(run);
    let rev: Vec<Point> = run.iter().rev().copied().collect();
    let t2 = end_tangent(&rev);
    fit_run(run, t1, t2, tol, out);
}

/// Least-squares cubics through `pts`, leaving the first point along `t1`
/// and arriving at the last from `t2` (pointing back into the run).
fn fit_run(pts: &[Point], t1: Vec2, t2: Vec2, tol: f64, out: &mut Vec<Seg>) {
    fit_cubic(pts, t1, t2, tol, out, 0);
}

fn fit_cubic(pts: &[Point], t1: Vec2, t2: Vec2, tol: f64, out: &mut Vec<Seg>, depth: u32) {
    let n = pts.len();
    if n < 2 {
        return;
    }
    let (p0, p3) = (pts[0], pts[n - 1]);
    if n == 2 {
        let d = (p3 - p0).hypot() / 3.0;
        out.push(Seg::Cubic([p0, p0 + t1 * d, p3 + t2 * d, p3]));
        return;
    }
    let mut u = chord_params(pts);
    let mut bez = generate(pts, &u, t1, t2);
    let (mut err, mut split) = max_error(pts, &bez, &u);
    let tol2 = tol * tol;
    if err <= tol2 {
        out.push(Seg::Cubic(bez));
        return;
    }
    if err <= tol2 * 16.0 {
        for _ in 0..6 {
            reparameterize(pts, &bez, &mut u);
            bez = generate(pts, &u, t1, t2);
            (err, split) = max_error(pts, &bez, &u);
            if err <= tol2 {
                out.push(Seg::Cubic(bez));
                return;
            }
        }
    }
    if depth > 24 || n < 3 {
        out.push(Seg::Cubic(bez));
        return;
    }
    let split = split.clamp(1, n - 2);
    let tc = unit(pts[split - 1] - pts[split + 1]);
    fit_cubic(&pts[..=split], t1, tc, tol, out, depth + 1);
    fit_cubic(&pts[split..], -tc, t2, tol, out, depth + 1);
}

fn chord_params(pts: &[Point]) -> Vec<f64> {
    let mut u = Vec::with_capacity(pts.len());
    let mut acc = 0.0;
    u.push(0.0);
    for w in pts.windows(2) {
        acc += (w[1] - w[0]).hypot();
        u.push(acc);
    }
    if acc > 1e-12 {
        for v in &mut u {
            *v /= acc;
        }
    }
    u
}

fn bernstein(t: f64) -> [f64; 4] {
    let s = 1.0 - t;
    [s * s * s, 3.0 * t * s * s, 3.0 * t * t * s, t * t * t]
}

fn eval(c: &[Point; 4], t: f64) -> Point {
    let b = bernstein(t);
    Point::new(
        c[0].x * b[0] + c[1].x * b[1] + c[2].x * b[2] + c[3].x * b[3],
        c[0].y * b[0] + c[1].y * b[1] + c[2].y * b[2] + c[3].y * b[3],
    )
}

/// Control points whose handle lengths minimise the squared distance to
/// the points at parameters `u`, with handle directions `t1` and `t2`.
fn generate(pts: &[Point], u: &[f64], t1: Vec2, t2: Vec2) -> [Point; 4] {
    let n = pts.len();
    let (p0, p3) = (pts[0], pts[n - 1]);
    let mut c = [[0.0f64; 2]; 2];
    let mut x = [0.0f64; 2];
    for (q, &t) in pts.iter().zip(u) {
        let b = bernstein(t);
        let a1 = t1 * b[1];
        let a2 = t2 * b[2];
        c[0][0] += a1.dot(a1);
        c[0][1] += a1.dot(a2);
        c[1][1] += a2.dot(a2);
        let base = p0.to_vec2() * (b[0] + b[1]) + p3.to_vec2() * (b[2] + b[3]);
        let r = q.to_vec2() - base;
        x[0] += a1.dot(r);
        x[1] += a2.dot(r);
    }
    c[1][0] = c[0][1];
    let det = c[0][0] * c[1][1] - c[0][1] * c[1][0];
    let seg = (p3 - p0).hypot();
    let (mut al, mut ar) = if det.abs() > 1e-12 {
        (
            (x[0] * c[1][1] - x[1] * c[0][1]) / det,
            (c[0][0] * x[1] - c[1][0] * x[0]) / det,
        )
    } else {
        (0.0, 0.0)
    };
    // Degenerate or wild handles: fall back to a third of the chord.
    let eps = 1e-6 * seg;
    if al < eps || ar < eps || al > seg * 2.0 || ar > seg * 2.0 {
        al = seg / 3.0;
        ar = seg / 3.0;
    }
    [p0, p0 + t1 * al, p3 + t2 * ar, p3]
}

/// Largest squared distance between a point and the curve at its
/// parameter, and the index of that point.
fn max_error(pts: &[Point], c: &[Point; 4], u: &[f64]) -> (f64, usize) {
    let n = pts.len();
    let mut best = (0.0, n / 2);
    for i in 1..n.saturating_sub(1) {
        let d = (eval(c, u[i]) - pts[i]).hypot2();
        if d >= best.0 {
            best = (d, i);
        }
    }
    best
}

/// One Newton step per point towards the nearest curve parameter.
fn reparameterize(pts: &[Point], c: &[Point; 4], u: &mut [f64]) {
    let d1 = [c[1] - c[0], c[2] - c[1], c[3] - c[2]].map(|v| v * 3.0);
    let d2 = [d1[1] - d1[0], d1[2] - d1[1]].map(|v| v * 2.0);
    for (q, t) in pts.iter().zip(u.iter_mut()) {
        let s = 1.0 - *t;
        let q0 = eval(c, *t);
        let q1 = d1[0] * (s * s) + d1[1] * (2.0 * s * *t) + d1[2] * (*t * *t);
        let q2 = d2[0] * s + d2[1] * *t;
        let diff = q0 - *q;
        let num = diff.dot(q1);
        let den = q1.dot(q1) + diff.dot(q2);
        if den.abs() > 1e-12 {
            *t = (*t - num / den).clamp(0.0, 1.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use graphdraw_core::geometry::{ParamCurveNearest, PathSeg, Shape as _};

    fn params() -> FitParams {
        FitParams {
            tolerance: 0.5,
            corner_angle: 50f64.to_radians(),
            smooth_passes: 1,
            scale: 1,
        }
    }

    fn max_dist(path: &BezPath, pts: &[Point]) -> f64 {
        let segs: Vec<PathSeg> = path.segments().collect();
        pts.iter()
            .map(|q| {
                segs.iter()
                    .map(|s| s.nearest(*q, 1e-6).distance_sq.sqrt())
                    .fold(f64::MAX, f64::min)
            })
            .fold(0.0, f64::max)
    }

    #[test]
    fn a_circle_fits_with_few_curves_and_no_corners() {
        let pts: Vec<Point> = (0..200)
            .map(|i| {
                let a = i as f64 / 200.0 * std::f64::consts::TAU;
                Point::new(50.0 + 30.0 * a.cos(), 50.0 + 30.0 * a.sin())
            })
            .collect();
        let path = fit_closed(&pts, &params());
        let curves = path.segments().count();
        assert!(curves <= 8, "{curves} segments");
        assert!(max_dist(&path, &pts) < 0.6);
        let area = path.area().abs();
        let expect = std::f64::consts::PI * 900.0;
        assert!((area - expect).abs() / expect < 0.01, "{area}");
    }

    #[test]
    fn a_square_keeps_four_sharp_corners_and_straight_sides() {
        // Midpoints of a 20 x 20 pixel square's unit edges, as the tracer
        // gives them: the corners themselves are missing.
        let mut pts = Vec::new();
        for i in 0..20 {
            pts.push(Point::new(i as f64 + 0.5, 0.0));
        }
        for i in 0..20 {
            pts.push(Point::new(20.0, i as f64 + 0.5));
        }
        for i in 0..20 {
            pts.push(Point::new(19.5 - i as f64, 20.0));
        }
        for i in 0..20 {
            pts.push(Point::new(0.0, 19.5 - i as f64));
        }
        let path = fit_closed(&pts, &params());
        let segs: Vec<PathSeg> = path.segments().collect();
        assert!(segs.iter().all(|s| matches!(s, PathSeg::Line(_))), "{segs:?}");
        assert!(segs.len() <= 5, "{segs:?}");
        assert!((path.area().abs() - 400.0).abs() < 1.0, "{}", path.area());
        let bb = path.bounding_box();
        assert!(bb.x0.abs() < 1e-6 && (bb.x1 - 20.0).abs() < 1e-6, "{bb:?}");
    }

    #[test]
    fn an_open_line_with_a_bend_keeps_the_bend() {
        let mut pts: Vec<Point> = (0..20).map(|i| Point::new(i as f64, 0.0)).collect();
        pts.extend((1..20).map(|i| Point::new(19.0, i as f64)));
        let path = fit_open(&pts, &params());
        let segs: Vec<PathSeg> = path.segments().collect();
        assert_eq!(segs.len(), 2, "{segs:?}");
        assert!(max_dist(&path, &pts) < 0.6);
    }

    #[test]
    fn tiny_and_degenerate_input_gives_no_panic() {
        assert!(fit_closed(&[], &params()).elements().is_empty());
        assert!(fit_open(&[Point::ZERO], &params()).elements().is_empty());
        let same = vec![Point::new(1.0, 1.0); 10];
        assert!(fit_closed(&same, &params()).elements().is_empty());
        let two = [Point::ZERO, Point::new(5.0, 0.0)];
        assert_eq!(fit_open(&two, &params()).segments().count(), 1);
    }
}
