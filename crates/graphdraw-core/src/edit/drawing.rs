//! Ferramentas de linha (doc 07): mão livre (GD-FER-20), polilinha
//! (GD-FER-25), linha 2 pontos com tangente/perpendicular (GD-FER-21) e
//! curva 3 pontos (GD-FER-26).

use kurbo::{Affine, BezPath, ParamCurve, ParamCurveDeriv, PathEl, Point, Vec2};

use super::convert::bez_to_subpaths;
use super::fit::fit_open;
use crate::geometry;
use crate::model::{Node, PathData, Subpath};

/// Reamostra o traço a cada `step` de comprimento (tira o tremor de pontos
/// muito juntos e deixa a detecção de cantos estável).
fn resample(points: &[Point], step: f64) -> Vec<Point> {
    let mut out = Vec::new();
    let Some(&first) = points.first() else {
        return out;
    };
    out.push(first);
    let mut carry = 0.0;
    for w in points.windows(2) {
        let (a, b) = (w[0], w[1]);
        let len = (b - a).hypot();
        if len <= 0.0 {
            continue;
        }
        let mut d = step - carry;
        while d <= len {
            out.push(a.lerp(b, d / len));
            d += step;
        }
        carry = len - (d - step);
    }
    if let Some(&last) = points.last()
        && out.last().is_none_or(|p| (*p - last).hypot() > step * 0.25)
    {
        out.push(last);
    }
    out
}

/// Ajusta um traço aberto em curvas com erro `tol`; viradas acima de
/// `corner_deg` (medidas numa janela de alguns passos) viram cantos.
pub fn fit_stroke(points: &[Point], tol: f64, corner_deg: f64) -> BezPath {
    let tol = tol.max(1e-3);
    let pts = resample(points, tol.max(1.0));
    let mut out = BezPath::new();
    if pts.len() < 2 {
        return out;
    }
    let n = pts.len();
    let k = 3.min(n / 2).max(1);
    let cos_c = corner_deg.to_radians().cos();
    let mut cuts = vec![0];
    let mut i = k;
    while i + k < n {
        let (u, v) = (pts[i] - pts[i - k], pts[i + k] - pts[i]);
        let (lu, lv) = (u.hypot(), v.hypot());
        if lu > 0.0 && lv > 0.0 && u.dot(v) / (lu * lv) < cos_c {
            cuts.push(i);
            i += k;
        } else {
            i += 1;
        }
    }
    cuts.push(n - 1);
    out.move_to(pts[0]);
    for w in cuts.windows(2) {
        let run = &pts[w[0]..=w[1]];
        if run.len() == 2 {
            out.line_to(run[1]);
            continue;
        }
        let a = run.len().min(k + 1) - 1;
        let t1 = (run[a] - run[0]).normalize();
        let t2 = (run[run.len() - 1 - a] - run[run.len() - 1]).normalize();
        let piece = fit_open(run, t1, t2, tol);
        out.extend(
            piece
                .elements()
                .iter()
                .copied()
                .filter(|e| !matches!(e, PathEl::MoveTo(_))),
        );
    }
    out
}

/// Mão livre (GD-FER-20): subcaminho a partir do traço; `smooth` (0–100)
/// aumenta a tolerância do ajuste a partir de `px` (µm por pixel de tela).
/// O fim perto do início (`close_dist`) fecha a curva.
pub fn freehand(points: &[Point], px: f64, smooth: f64, close_dist: f64) -> Option<Subpath> {
    if points.len() < 2 {
        return None;
    }
    let tol = px * (0.5 + smooth.clamp(0.0, 100.0) / 100.0 * 6.0);
    let close = points.len() > 3 && (points[0] - *points.last().unwrap()).hypot() <= close_dist;
    let path = fit_stroke(points, tol, 70.0);
    let mut sp = bez_to_subpaths(&path).into_iter().next()?;
    if close && sp.nodes.len() > 2 {
        // O último nó cai sobre o primeiro: funde e fecha.
        let last = sp.nodes.pop().unwrap();
        sp.nodes[0].handle_in = last.handle_in;
        sp.closed = true;
    }
    Some(sp)
}

/// Curva 3 pontos (GD-FER-26): de `a` a `b`, passando por `c` no meio.
pub fn curve3(a: Point, b: Point, c: Point) -> Subpath {
    // Quadrática com controle q que passa por c em t = 1/2, elevada a cúbica.
    let q = Point::new(2.0 * c.x - (a.x + b.x) / 2.0, 2.0 * c.y - (a.y + b.y) / 2.0);
    let c1 = a + (q - a) * (2.0 / 3.0);
    let c2 = b + (q - b) * (2.0 / 3.0);
    let mut n0 = Node::corner(a.x, a.y);
    n0.handle_out = Some([c1.x, c1.y]);
    let mut n1 = Node::corner(b.x, b.y);
    n1.handle_in = Some([c2.x, c2.y]);
    Subpath {
        nodes: vec![n0, n1],
        closed: false,
        extra: Default::default(),
    }
}

/// Ponto do contorno `pd` (local; `m` leva à página) mais próximo de `p`,
/// com a normal unitária na página.
pub fn normal_at(pd: &PathData, m: Affine, p: Point, tol: f64) -> Option<(Point, Vec2)> {
    let near = geometry::nearest(pd, m.inverse() * p)?;
    let scale = m.determinant().abs().sqrt().max(1e-12);
    if near.distance * scale > tol {
        return None;
    }
    let seg = geometry::segment(pd, near.seg)?;
    let q = m * seg.eval(near.t);
    let d = m * (seg.eval(near.t) + deriv(&seg, near.t)) - q;
    let len = d.hypot();
    if len <= 0.0 {
        return None;
    }
    Some((q, Vec2::new(-d.y, d.x) / len))
}

fn deriv(s: &kurbo::PathSeg, t: f64) -> Vec2 {
    match s {
        kurbo::PathSeg::Line(l) => l.p1 - l.p0,
        kurbo::PathSeg::Quad(q) => q.deriv().eval(t).to_vec2(),
        kurbo::PathSeg::Cubic(c) => c.deriv().eval(t).to_vec2(),
    }
}

/// Ponto de tangência no contorno `pd` visto de `from` (página), o mais
/// próximo de `near`: onde a reta `from → q` encosta na curva.
pub fn tangent_point(pd: &PathData, m: Affine, from: Point, near: Point) -> Option<Point> {
    let mut best: Option<(f64, Point)> = None;
    for (_, seg) in geometry::segments(pd) {
        let f = |t: f64| {
            let q = m * seg.eval(t);
            let d = m * (seg.eval(t) + deriv(&seg, t)) - q;
            (q - from).cross(d)
        };
        let steps = 64;
        let mut prev = f(0.0);
        for i in 1..=steps {
            let t = i as f64 / steps as f64;
            let cur = f(t);
            if prev == 0.0 || prev.signum() != cur.signum() {
                // Bisseção até a raiz.
                let (mut lo, mut hi) = ((i - 1) as f64 / steps as f64, t);
                for _ in 0..40 {
                    let mid = (lo + hi) / 2.0;
                    if f(lo).signum() == f(mid).signum() {
                        lo = mid;
                    } else {
                        hi = mid;
                    }
                }
                let q = m * seg.eval((lo + hi) / 2.0);
                if (q - from).hypot() > 1e-6 {
                    let d = (q - near).hypot();
                    if best.is_none_or(|(bd, _)| d < bd) {
                        best = Some((d, q));
                    }
                }
            }
            prev = cur;
        }
    }
    best.map(|(_, q)| q)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::FillRule;

    /// GD-FER-20: traço tremido vira poucos nós; o "L" mantém o canto;
    /// voltar ao início fecha.
    #[test]
    fn gd_fer_20_freehand_fit() {
        let wobbly: Vec<Point> = (0..=200)
            .map(|i| Point::new(i as f64 * 500.0, if i % 2 == 0 { 0.0 } else { 40.0 }))
            .collect();
        let sp = freehand(&wobbly, 100.0, 50.0, 1_000.0).unwrap();
        assert!(sp.nodes.len() <= 4, "{} nós", sp.nodes.len());
        assert!(!sp.closed);
        let mut l: Vec<Point> = (0..=50)
            .map(|i| Point::new(0.0, 50_000.0 - i as f64 * 1_000.0))
            .collect();
        l.extend((1..=50).map(|i| Point::new(i as f64 * 1_000.0, 0.0)));
        let sp = freehand(&l, 100.0, 20.0, 1_000.0).unwrap();
        assert!(
            sp.nodes
                .iter()
                .any(|n| (n.p[0]).abs() < 600.0 && (n.p[1]).abs() < 600.0),
            "canto no (0,0): {:?}",
            sp.nodes.iter().map(|n| n.p).collect::<Vec<_>>()
        );
        let ring: Vec<Point> = (0..=64)
            .map(|i| {
                let a = i as f64 / 64.0 * std::f64::consts::TAU;
                Point::new(10_000.0 * a.cos(), 10_000.0 * a.sin())
            })
            .collect();
        assert!(freehand(&ring, 100.0, 50.0, 1_000.0).unwrap().closed);
    }

    /// GD-FER-26: a curva passa pelo ponto do meio.
    #[test]
    fn gd_fer_26_curve_through_middle() {
        let sp = curve3(
            Point::new(0.0, 0.0),
            Point::new(100.0, 0.0),
            Point::new(50.0, 40.0),
        );
        let s = geometry::subpath_segments(&sp)[0];
        let mid = s.eval(0.5);
        assert!((mid - Point::new(50.0, 40.0)).hypot() < 1e-9);
    }

    /// GD-FER-21: normal num quadrado e tangente num círculo.
    #[test]
    fn gd_fer_21_tangent_and_perpendicular() {
        let square = PathData {
            subpaths: vec![Subpath {
                nodes: vec![
                    Node::corner(0.0, 0.0),
                    Node::corner(100.0, 0.0),
                    Node::corner(100.0, 100.0),
                    Node::corner(0.0, 100.0),
                ],
                closed: true,
                extra: Default::default(),
            }],
            fill_rule: FillRule::Nonzero,
            extra: Default::default(),
        };
        let (q, n) = normal_at(&square, Affine::IDENTITY, Point::new(50.0, 1.0), 5.0).unwrap();
        assert!((q - Point::new(50.0, 0.0)).hypot() < 1e-6);
        assert!(n.x.abs() < 1e-9 && (n.y.abs() - 1.0).abs() < 1e-9);
        // Círculo de raio 10 na origem; de (20, 0) a tangente toca em x = 5.
        let circle = PathData {
            subpaths: bez_to_subpaths(&kurbo::Shape::to_path(
                &kurbo::Circle::new((0.0, 0.0), 10.0),
                0.01,
            )),
            fill_rule: FillRule::Nonzero,
            extra: Default::default(),
        };
        let q = tangent_point(
            &circle,
            Affine::IDENTITY,
            Point::new(20.0, 0.0),
            Point::new(5.0, 9.0),
        )
        .unwrap();
        assert!(
            (q.x - 5.0).abs() < 0.05 && (q.y - 8.66).abs() < 0.05,
            "{q:?}"
        );
    }
}
