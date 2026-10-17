//! B-spline (GD-BSP-01): cúbica uniforme pelos pontos de controle,
//! convertida em Bézier trecho a trecho. Um ponto fixado entra três vezes
//! na sequência — a curva passa por ele (com quina); a curva aberta também
//! repete as pontas, para começar e terminar nelas.

use kurbo::{BezPath, Point};

use crate::model::BSplineData;

/// Sequência de controle expandida (fixados e pontas triplicados).
fn expanded(b: &BSplineData) -> Vec<Point> {
    let n = b.points.len();
    let mut q = Vec::with_capacity(n * 3);
    for (i, c) in b.points.iter().enumerate() {
        let p = Point::new(c.p[0], c.p[1]);
        let end = !b.closed && (i == 0 || i + 1 == n);
        let k = if c.fixed || end { 3 } else { 1 };
        q.extend(std::iter::repeat_n(p, k));
    }
    q
}

/// Trecho uniforme P0..P3 em Bézier: início, dois controles, fim.
fn segment(p: [Point; 4]) -> [Point; 4] {
    let v = |a: Point| a.to_vec2();
    let [p0, p1, p2, p3] = p.map(v);
    [
        ((p0 + p1 * 4.0 + p2) / 6.0).to_point(),
        ((p1 * 2.0 + p2) / 3.0).to_point(),
        ((p1 + p2 * 2.0) / 3.0).to_point(),
        ((p1 + p2 * 4.0 + p3) / 6.0).to_point(),
    ]
}

/// Caminho da B-spline (espaço local).
pub fn path(b: &BSplineData) -> BezPath {
    let q = expanded(b);
    let mut out = BezPath::new();
    let n = q.len();
    let windows: Vec<[Point; 4]> = if b.closed {
        if b.points.len() < 3 {
            return out;
        }
        (0..n)
            .map(|i| [q[i], q[(i + 1) % n], q[(i + 2) % n], q[(i + 3) % n]])
            .collect()
    } else {
        if b.points.len() < 2 {
            return out;
        }
        q.windows(4).map(|w| [w[0], w[1], w[2], w[3]]).collect()
    };
    let mut started = false;
    for w in windows {
        if w.iter().all(|p| *p == w[0]) {
            continue;
        }
        let [a, c1, c2, e] = segment(w);
        if !started {
            out.move_to(a);
            started = true;
        }
        out.curve_to(c1, c2, e);
    }
    if b.closed && started {
        out.close_path();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::BSplinePoint;
    use kurbo::{ParamCurve, PathSeg};

    fn data(pts: &[(f64, f64, bool)], closed: bool) -> BSplineData {
        BSplineData {
            points: pts
                .iter()
                .map(|&(x, y, fixed)| BSplinePoint { p: [x, y], fixed })
                .collect(),
            closed,
            extra: Default::default(),
        }
    }

    fn near(p: &BezPath, q: Point) -> f64 {
        p.segments()
            .flat_map(|s: PathSeg| (0..=100).map(move |i| s.eval(i as f64 / 100.0)))
            .map(|r| (r - q).hypot())
            .fold(f64::INFINITY, f64::min)
    }

    /// GD-BSP-01: aberta começa e termina nas pontas; o ponto flutuante só
    /// puxa a curva; o fixado ela atravessa.
    #[test]
    fn gd_bsp_01_fixed_and_floating() {
        let float = data(
            &[(0.0, 0.0, false), (50.0, 100.0, false), (100.0, 0.0, false)],
            false,
        );
        let p = path(&float);
        let pts: Vec<Point> = p.segments().map(|s| s.eval(0.0)).collect();
        assert_eq!(pts[0], Point::ORIGIN);
        assert_eq!(
            p.segments().last().unwrap().eval(1.0),
            Point::new(100.0, 0.0)
        );
        assert!(
            near(&p, Point::new(50.0, 100.0)) > 10.0,
            "flutuante não é tocado"
        );
        let fixed = data(
            &[(0.0, 0.0, false), (50.0, 100.0, true), (100.0, 0.0, false)],
            false,
        );
        assert!(
            near(&path(&fixed), Point::new(50.0, 100.0)) < 1e-9,
            "fixado é tocado"
        );
    }

    /// Fechada: contínua e sem tocar os flutuantes; dois pontos não fecham.
    #[test]
    fn closed_loop() {
        let sq = data(
            &[
                (0.0, 0.0, false),
                (100.0, 0.0, false),
                (100.0, 100.0, false),
                (0.0, 100.0, false),
            ],
            true,
        );
        let p = path(&sq);
        let segs: Vec<PathSeg> = p.segments().collect();
        assert_eq!(segs.len(), 4);
        assert!((segs[3].eval(1.0) - segs[0].eval(0.0)).hypot() < 1e-9);
        assert!(near(&p, Point::new(50.0, 50.0)) > 20.0);
        assert!(
            path(&data(&[(0.0, 0.0, false), (1.0, 1.0, false)], true))
                .elements()
                .is_empty()
        );
        // Dois pontos abertos: uma reta.
        let l = path(&data(&[(0.0, 0.0, false), (100.0, 0.0, false)], false));
        assert!(near(&l, Point::new(50.0, 0.0)) < 1e-9);
    }
}
