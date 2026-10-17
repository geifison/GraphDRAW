//! Ajuste de curvas Bézier cúbicas a polígonos fechados (resultado das
//! booleanas e do deslocamento, GD-BOO-11).
//!
//! Cantos (virada acima de `corner_deg`) quebram o contorno em trechos; cada
//! trecho suave é ajustado por mínimos quadrados com parametrização por
//! comprimento de corda, reparametrização de Newton e subdivisão no ponto de
//! maior erro (método clássico de ajuste de curvas, publicado por P. J.
//! Schneider em 1990). Trechos de dois pontos ficam como retas.

use kurbo::{BezPath, Point, Vec2};

/// Ajusta um contorno fechado; `tol` é o erro máximo (mesma unidade dos
/// pontos).
pub fn fit_closed(points: &[Point], tol: f64, corner_deg: f64) -> BezPath {
    let mut pts: Vec<Point> = Vec::with_capacity(points.len());
    for p in points {
        if pts
            .last()
            .is_none_or(|q: &Point| (*q - *p).hypot() > tol * 1e-3)
        {
            pts.push(*p);
        }
    }
    while pts.len() > 1 && (pts[0] - pts[pts.len() - 1]).hypot() <= tol * 1e-3 {
        pts.pop();
    }
    let n = pts.len();
    let mut out = BezPath::new();
    if n < 3 {
        return out;
    }
    let cos_corner = corner_deg.to_radians().cos();
    let turn_is_corner = |i: usize| {
        let a = pts[(i + n - 1) % n];
        let b = pts[i];
        let c = pts[(i + 1) % n];
        let (u, v) = ((b - a), (c - b));
        let (lu, lv) = (u.hypot(), v.hypot());
        lu > 0.0 && lv > 0.0 && u.dot(v) / (lu * lv) < cos_corner
    };
    let corners: Vec<usize> = (0..n).filter(|&i| turn_is_corner(i)).collect();
    // Sem cantos: começa no ponto 0 com tangente central.
    let starts = if corners.is_empty() {
        vec![0]
    } else {
        corners.clone()
    };
    let smooth_ring = corners.is_empty();
    out.move_to(pts[starts[0]]);
    for k in 0..starts.len() {
        let s = starts[k];
        let e = if k + 1 < starts.len() {
            starts[k + 1]
        } else {
            starts[0] + n
        };
        let run: Vec<Point> = (s..=e).map(|i| pts[i % n]).collect();
        if run.len() == 2 {
            out.line_to(run[1]);
            continue;
        }
        let (t1, t2) = if smooth_ring {
            let t = (pts[1] - pts[n - 1]).normalize();
            (t, -t)
        } else {
            (
                (run[1] - run[0]).normalize(),
                (run[run.len() - 2] - run[run.len() - 1]).normalize(),
            )
        };
        fit_cubic(&run, t1, t2, tol, &mut out, 0);
    }
    out.close_path();
    out
}

/// Ajusta um trecho aberto de pontos com as tangentes das pontas dadas
/// (`t1` saindo do primeiro, `t2` chegando ao último, apontando para
/// dentro do trecho); `tol` é o erro máximo.
pub fn fit_open(points: &[Point], t1: Vec2, t2: Vec2, tol: f64) -> BezPath {
    let mut pts: Vec<Point> = Vec::with_capacity(points.len());
    for p in points {
        if pts
            .last()
            .is_none_or(|q: &Point| (*q - *p).hypot() > tol * 1e-3)
        {
            pts.push(*p);
        }
    }
    let mut out = BezPath::new();
    let Some(first) = pts.first() else {
        return out;
    };
    out.move_to(*first);
    if pts.len() >= 2 {
        fit_cubic(&pts, t1, t2, tol, &mut out, 0);
    }
    out
}

fn chord_params(p: &[Point]) -> Vec<f64> {
    let mut u = vec![0.0; p.len()];
    for i in 1..p.len() {
        u[i] = u[i - 1] + (p[i] - p[i - 1]).hypot();
    }
    let total = u[p.len() - 1].max(1e-12);
    u.iter_mut().for_each(|x| *x /= total);
    u
}

fn bez(c: &[Point; 4], t: f64) -> Point {
    let mt = 1.0 - t;
    let v = c[0].to_vec2() * (mt * mt * mt)
        + c[1].to_vec2() * (3.0 * mt * mt * t)
        + c[2].to_vec2() * (3.0 * mt * t * t)
        + c[3].to_vec2() * (t * t * t);
    v.to_point()
}

/// Mínimos quadrados para os comprimentos das alças nas tangentes dadas.
fn generate(p: &[Point], u: &[f64], t1: Vec2, t2: Vec2) -> [Point; 4] {
    let (first, last) = (p[0], p[p.len() - 1]);
    let (mut c00, mut c01, mut c11, mut x0, mut x1) = (0.0, 0.0, 0.0, 0.0, 0.0);
    for (i, &t) in u.iter().enumerate() {
        let mt = 1.0 - t;
        let b0 = mt * mt * mt;
        let b1 = 3.0 * mt * mt * t;
        let b2 = 3.0 * mt * t * t;
        let b3 = t * t * t;
        let a1 = t1 * b1;
        let a2 = t2 * b2;
        c00 += a1.dot(a1);
        c01 += a1.dot(a2);
        c11 += a2.dot(a2);
        let tmp = p[i].to_vec2() - (first.to_vec2() * (b0 + b1) + last.to_vec2() * (b2 + b3));
        x0 += a1.dot(tmp);
        x1 += a2.dot(tmp);
    }
    let det = c00 * c11 - c01 * c01;
    let seg = (last - first).hypot();
    let (mut a, mut b) = if det.abs() > 1e-12 {
        ((x0 * c11 - x1 * c01) / det, (c00 * x1 - c01 * x0) / det)
    } else {
        (seg / 3.0, seg / 3.0)
    };
    // Alças negativas ou minúsculas: cai para um terço da corda.
    let eps = 1e-6 * seg;
    if a < eps || b < eps {
        a = seg / 3.0;
        b = seg / 3.0;
    }
    [first, first + t1 * a, last + t2 * b, last]
}

fn max_error(p: &[Point], c: &[Point; 4], u: &[f64]) -> (f64, usize) {
    let mut worst = (0.0, p.len() / 2);
    for i in 1..p.len() - 1 {
        let d = (bez(c, u[i]) - p[i]).hypot2();
        if d > worst.0 {
            worst = (d, i);
        }
    }
    worst
}

fn reparam(p: &[Point], u: &mut [f64], c: &[Point; 4]) {
    let d1 = [
        (c[1] - c[0]) * 3.0,
        (c[2] - c[1]) * 3.0,
        (c[3] - c[2]) * 3.0,
    ];
    let d2 = [(d1[1] - d1[0]) * 2.0, (d1[2] - d1[1]) * 2.0];
    for (i, t) in u.iter_mut().enumerate() {
        let mt = 1.0 - *t;
        let q = bez(c, *t);
        let q1 = d1[0] * (mt * mt) + d1[1] * (2.0 * mt * *t) + d1[2] * (*t * *t);
        let q2 = d2[0] * mt + d2[1] * *t;
        let diff = q - p[i];
        let num = diff.dot(q1);
        let den = q1.dot(q1) + diff.dot(q2);
        if den.abs() > 1e-12 {
            *t = (*t - num / den).clamp(0.0, 1.0);
        }
    }
}

fn fit_cubic(p: &[Point], t1: Vec2, t2: Vec2, tol: f64, out: &mut BezPath, depth: usize) {
    let last = p[p.len() - 1];
    if p.len() == 2 {
        out.line_to(last);
        return;
    }
    let err2 = tol * tol;
    let mut u = chord_params(p);
    let mut c = generate(p, &u, t1, t2);
    let (mut e, mut split) = max_error(p, &c, &u);
    if e < err2 {
        out.curve_to(c[1], c[2], c[3]);
        return;
    }
    if e < err2 * 16.0 {
        for _ in 0..6 {
            reparam(p, &mut u, &c);
            c = generate(p, &u, t1, t2);
            (e, split) = max_error(p, &c, &u);
            if e < err2 {
                out.curve_to(c[1], c[2], c[3]);
                return;
            }
        }
    }
    if depth > 24 || p.len() < 4 {
        // Sem convergência: segmentos retos entre os pontos.
        for q in &p[1..] {
            out.line_to(*q);
        }
        return;
    }
    let split = split.clamp(1, p.len() - 2);
    let tc = (p[split - 1] - p[split + 1]).normalize();
    fit_cubic(&p[..=split], t1, tc, tol, out, depth + 1);
    fit_cubic(&p[split..], -tc, t2, tol, out, depth + 1);
}

#[cfg(test)]
mod tests {
    use super::*;
    use kurbo::Shape;

    #[test]
    fn circle_polygon_becomes_few_curves() {
        let r = 10_000.0;
        let pts: Vec<Point> = (0..200)
            .map(|i| {
                let a = i as f64 / 200.0 * std::f64::consts::TAU;
                Point::new(r * a.cos(), r * a.sin())
            })
            .collect();
        let p = fit_closed(&pts, 2.0, 12.0);
        let n = p.elements().len();
        assert!(n <= 24, "{n} elementos");
        let want = std::f64::consts::PI * r * r;
        assert!((p.area().abs() - want).abs() / want < 0.001);
    }

    #[test]
    fn square_keeps_corners_as_lines() {
        let pts = [
            Point::new(0.0, 0.0),
            Point::new(5.0, 0.0),
            Point::new(10.0, 0.0),
            Point::new(10.0, 10.0),
            Point::new(0.0, 10.0),
        ];
        let p = fit_closed(&pts, 0.01, 12.0);
        assert!((p.area().abs() - 100.0).abs() < 1e-6);
        let b = p.bounding_box();
        assert_eq!((b.width(), b.height()), (10.0, 10.0));
    }
}
