//! Tracejado nos cantos (GD-OUT-03): além do padrão contínuo ao longo do
//! caminho (`standard`, nativo dos renderizadores), o tracejado pode
//! recomeçar em cada canto — `fixed` (os traços mantêm o comprimento exato)
//! ou `align` (o padrão estica ou encolhe em cada trecho para que haja um
//! traço inteiro passando por cada canto). Aqui o caminho vira os próprios
//! traços (subcaminhos abertos), desenhados sem tracejado.

use kurbo::{BezPath, ParamCurve, ParamCurveArclen, ParamCurveDeriv, PathEl, PathSeg, Point};

const ACC: f64 = 1e-3;

/// Trechos entre cantos de cada subcaminho, e se o subcaminho é fechado.
fn runs(path: &BezPath) -> Vec<(Vec<Vec<PathSeg>>, bool)> {
    let mut subs: Vec<(Vec<PathSeg>, bool)> = Vec::new();
    let mut cur: Vec<PathSeg> = Vec::new();
    let (mut start, mut last) = (Point::ORIGIN, Point::ORIGIN);
    let flush = |cur: &mut Vec<PathSeg>, closed: bool, subs: &mut Vec<(Vec<PathSeg>, bool)>| {
        if !cur.is_empty() {
            subs.push((std::mem::take(cur), closed));
        }
    };
    for el in path.elements() {
        match *el {
            PathEl::MoveTo(p) => {
                flush(&mut cur, false, &mut subs);
                start = p;
                last = p;
            }
            PathEl::LineTo(p) => {
                cur.push(PathSeg::Line(kurbo::Line::new(last, p)));
                last = p;
            }
            PathEl::QuadTo(a, p) => {
                cur.push(PathSeg::Quad(kurbo::QuadBez::new(last, a, p)));
                last = p;
            }
            PathEl::CurveTo(a, b, p) => {
                cur.push(PathSeg::Cubic(kurbo::CubicBez::new(last, a, b, p)));
                last = p;
            }
            PathEl::ClosePath => {
                if (last - start).hypot() > 1e-9 {
                    cur.push(PathSeg::Line(kurbo::Line::new(last, start)));
                }
                flush(&mut cur, true, &mut subs);
                last = start;
            }
        }
    }
    flush(&mut cur, false, &mut subs);
    let tangent = |s: &PathSeg, t: f64| -> kurbo::Vec2 {
        let d = match s {
            PathSeg::Line(l) => l.p1 - l.p0,
            PathSeg::Quad(q) => q.deriv().eval(t).to_vec2(),
            PathSeg::Cubic(c) => c.deriv().eval(t).to_vec2(),
        };
        if d.hypot() < 1e-12 {
            // Alça colapsada: usa a corda.
            s.eval(1.0) - s.eval(0.0)
        } else {
            d
        }
    };
    let corner = |a: &PathSeg, b: &PathSeg| {
        let (u, v) = (tangent(a, 1.0), tangent(b, 0.0));
        let (lu, lv) = (u.hypot(), v.hypot());
        lu > 0.0 && lv > 0.0 && (u.dot(v) / (lu * lv)) < (1.0f64.to_radians()).cos()
    };
    subs.into_iter()
        .map(|(segs, closed)| {
            let n = segs.len();
            let cut: Vec<bool> = (0..n)
                .map(|i| {
                    if i + 1 < n {
                        corner(&segs[i], &segs[i + 1])
                    } else {
                        closed && corner(&segs[i], &segs[0])
                    }
                })
                .collect();
            // Fechado com cantos: começa logo depois de um canto.
            let begin = if closed {
                cut.iter().position(|c| *c).map_or(0, |i| (i + 1) % n)
            } else {
                0
            };
            let mut out: Vec<Vec<PathSeg>> = vec![Vec::new()];
            for k in 0..n {
                let i = (begin + k) % n;
                out.last_mut().unwrap().push(segs[i]);
                if cut[i] && k + 1 < n {
                    out.push(Vec::new());
                }
            }
            // Fechado sem nenhum canto (círculo): um laço só.
            let looped = closed && !cut.iter().any(|c| *c);
            (out, closed && !looped)
        })
        .collect()
}

/// Intervalos de traço [a, b] num trecho de comprimento `len`.
fn intervals(len: f64, dash: &[f64], mode: &str) -> Vec<(f64, f64)> {
    let total: f64 = dash.iter().sum();
    if total <= 0.0 || len <= 0.0 {
        return vec![(0.0, len)];
    }
    // Número par de entradas (traço, espaço).
    let pat: Vec<f64> = if dash.len() % 2 == 1 {
        dash.iter().chain(dash).copied().collect()
    } else {
        dash.to_vec()
    };
    let total: f64 = pat.iter().sum();
    let k = if mode == "align" {
        // n repetições e mais um primeiro traço: termina num traço.
        let n = ((len - pat[0]) / total).round().max(0.0);
        len / (n * total + pat[0])
    } else {
        1.0
    };
    let mut out = Vec::new();
    let (mut x, mut i) = (0.0, 0usize);
    while x < len - 1e-9 {
        let d = pat[i % pat.len()] * k;
        if i % 2 == 0 && d > 0.0 {
            out.push((x, (x + d).min(len)));
        }
        x += d.max(1e-9);
        i += 1;
    }
    out
}

/// Pedaço [a, b] (por comprimento) de uma sequência de segmentos.
fn piece(segs: &[PathSeg], lens: &[f64], a: f64, b: f64) -> Vec<PathSeg> {
    let mut out = Vec::new();
    let mut s0 = 0.0;
    for (seg, l) in segs.iter().zip(lens) {
        let s1 = s0 + l;
        if s1 > a + 1e-9 && s0 < b - 1e-9 && *l > 0.0 {
            let t0 = if a <= s0 {
                0.0
            } else {
                seg.inv_arclen(a - s0, ACC)
            };
            let t1 = if b >= s1 {
                1.0
            } else {
                seg.inv_arclen(b - s0, ACC)
            };
            if t1 > t0 {
                out.push(seg.subsegment(t0..t1));
            }
        }
        s0 = s1;
    }
    out
}

/// Os traços do caminho com o padrão `dash` recomeçando em cada canto
/// (`mode` = `fixed` ou `align`). Os traços que tocam um canto dos dois
/// lados viram um subcaminho só, para o canto ter junção.
pub fn corner_dashes(path: &BezPath, dash: &[f64], mode: &str) -> BezPath {
    let mut out = BezPath::new();
    for (rs, closed) in runs(path) {
        // Cada traço: segmentos, se começa no início e se termina no fim do trecho.
        let mut dashes: Vec<(Vec<PathSeg>, bool, bool)> = Vec::new();
        for run in &rs {
            let lens: Vec<f64> = run.iter().map(|s| s.arclen(ACC)).collect();
            let len: f64 = lens.iter().sum();
            for (a, b) in intervals(len, dash, mode) {
                let p = piece(run, &lens, a, b);
                if !p.is_empty() {
                    dashes.push((p, a <= 1e-6, b >= len - 1e-6));
                }
            }
        }
        // Junta o traço que chega ao canto com o que sai dele.
        let mut merged: Vec<(Vec<PathSeg>, bool, bool)> = Vec::new();
        for d in dashes {
            match merged.last_mut() {
                Some(last) if last.2 && d.1 => {
                    last.0.extend(d.0);
                    last.2 = d.2;
                }
                _ => merged.push(d),
            }
        }
        if closed && merged.len() > 1 && merged.last().unwrap().2 && merged[0].1 {
            let (mut tail, _, _) = merged.pop().unwrap();
            tail.extend(std::mem::take(&mut merged[0].0));
            merged[0].0 = tail;
        }
        for (segs, _, _) in merged {
            out.move_to(segs[0].start());
            for s in segs {
                match s {
                    PathSeg::Line(l) => out.line_to(l.p1),
                    PathSeg::Quad(q) => out.quad_to(q.p1, q.p2),
                    PathSeg::Cubic(c) => out.curve_to(c.p1, c.p2, c.p3),
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use kurbo::Shape;

    fn square() -> BezPath {
        kurbo::Rect::new(0.0, 0.0, 100.0, 100.0).to_path(0.1)
    }

    fn subpaths(p: &BezPath) -> Vec<Vec<Point>> {
        let mut out: Vec<Vec<Point>> = Vec::new();
        for el in p.elements() {
            match *el {
                PathEl::MoveTo(q) => out.push(vec![q]),
                PathEl::LineTo(q) | PathEl::QuadTo(_, q) | PathEl::CurveTo(_, _, q) => {
                    out.last_mut().unwrap().push(q)
                }
                PathEl::ClosePath => {}
            }
        }
        out
    }

    /// Alinhar: cada canto do quadrado fica no meio de um traço (o traço
    /// passa pelo canto), e todo trecho termina num traço.
    #[test]
    fn align_puts_a_dash_through_every_corner() {
        let d = corner_dashes(&square(), &[12.0, 7.0], "align");
        let subs = subpaths(&d);
        for c in [
            Point::new(0.0, 0.0),
            Point::new(100.0, 0.0),
            Point::new(100.0, 100.0),
            Point::new(0.0, 100.0),
        ] {
            assert!(
                subs.iter()
                    .any(|s| s.len() >= 3
                        && s[1..s.len() - 1].iter().any(|q| (*q - c).hypot() < 1e-6)),
                "canto {c:?} dentro de um traço"
            );
        }
    }

    /// Fixo: o primeiro traço depois de cada canto tem o comprimento exato;
    /// linha reta sem cantos só recorta.
    #[test]
    fn fixed_keeps_exact_lengths() {
        let mut l = BezPath::new();
        l.move_to((0.0, 0.0));
        l.line_to((100.0, 0.0));
        l.line_to((100.0, 50.0));
        let d = corner_dashes(&l, &[10.0, 5.0], "fixed");
        let subs = subpaths(&d);
        assert!(
            (subs[0][1] - Point::new(10.0, 0.0)).hypot() < 1e-6,
            "{:?}",
            subs[0]
        );
        // Depois do canto em (100, 0), recomeça: 0..10 em y.
        assert!(subs.iter().any(|s| {
            (s[0] - Point::new(100.0, 0.0)).hypot() < 1e-6
                || s.iter()
                    .any(|q| (*q - Point::new(100.0, 10.0)).hypot() < 1e-6)
        }));
        // Círculo (sem cantos): é um laço só, os traços seguem o padrão.
        let c = kurbo::Circle::new((0.0, 0.0), 50.0).to_path(0.1);
        assert!(subpaths(&corner_dashes(&c, &[10.0, 10.0], "align")).len() > 5);
    }
}
