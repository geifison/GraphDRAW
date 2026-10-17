//! Pontas de seta nas extremidades de caminhos abertos (GD-OUT-05).
//! Desenhos próprios: cada ponta é uma forma preenchida, com a ponta no
//! ponto (0, 0) apontando para +x e comprimento 1, escalada pela espessura
//! do contorno.

use kurbo::{Affine, BezPath, ParamCurve, ParamCurveArclen, ParamCurveDeriv, PathSeg, Point, Vec2};

use crate::model::Arrow;

/// Formas da biblioteca, na ordem mostrada na interface.
pub const SHAPES: [&str; 6] = ["triangle", "open", "barbed", "circle", "square", "bar"];

/// Comprimento da ponta em múltiplos da espessura (com escala 1).
pub const BASE: f64 = 4.0;
/// Espessura mínima considerada (µm), para o fio de cabelo ter ponta visível.
pub const MIN_WIDTH: f64 = 250.0;

/// Contorno unitário da forma e quanto o traço deve recuar (em
/// comprimentos de ponta) para não aparecer além dela.
fn unit(shape: &str) -> (BezPath, f64) {
    let mut p = BezPath::new();
    let setback = match shape {
        "open" => {
            // Chevron: duas hastes grossas.
            p.move_to((0.0, 0.0));
            p.line_to((-1.0, 0.5));
            p.line_to((-1.0, 0.3));
            p.line_to((-0.35, 0.0));
            p.line_to((-1.0, -0.3));
            p.line_to((-1.0, -0.5));
            p.close_path();
            0.3
        }
        "barbed" => {
            p.move_to((0.0, 0.0));
            p.line_to((-1.0, 0.45));
            p.line_to((-0.7, 0.0));
            p.line_to((-1.0, -0.45));
            p.close_path();
            0.65
        }
        "circle" => {
            let c = kurbo::Circle::new((-0.5, 0.0), 0.5);
            p = kurbo::Shape::to_path(&c, 1e-4);
            0.5
        }
        "square" => {
            p.move_to((0.0, -0.5));
            p.line_to((0.0, 0.5));
            p.line_to((-1.0, 0.5));
            p.line_to((-1.0, -0.5));
            p.close_path();
            0.5
        }
        "bar" => {
            p.move_to((0.0, -0.6));
            p.line_to((0.0, 0.6));
            p.line_to((-0.2, 0.6));
            p.line_to((-0.2, -0.6));
            p.close_path();
            0.0
        }
        _ => {
            // Triângulo.
            p.move_to((0.0, 0.0));
            p.line_to((-1.0, 0.45));
            p.line_to((-1.0, -0.45));
            p.close_path();
            0.9
        }
    };
    (p, setback)
}

/// Extremidade de um caminho: o ponto e a direção de saída (para fora).
#[derive(Debug, Clone, Copy)]
struct End {
    point: Point,
    out: Vec2,
}

fn tangent(seg: &PathSeg, t: f64) -> Vec2 {
    let d = match seg {
        PathSeg::Line(l) => l.p1 - l.p0,
        PathSeg::Quad(q) => q.deriv().eval(t).to_vec2(),
        PathSeg::Cubic(c) => c.deriv().eval(t).to_vec2(),
    };
    if d.hypot() > 1e-12 {
        return d.normalize();
    }
    // Alça nula na ponta: usa a corda.
    let c = seg.eval(1.0) - seg.eval(0.0);
    if c.hypot() > 0.0 {
        c.normalize()
    } else {
        Vec2::new(1.0, 0.0)
    }
}

/// Subcaminhos abertos: (primeiro segmento, último segmento) de cada um.
fn open_runs(path: &BezPath) -> Vec<Vec<PathSeg>> {
    let mut runs: Vec<(Vec<PathSeg>, bool)> = Vec::new();
    let mut start = Point::ORIGIN;
    let mut last = Point::ORIGIN;
    for el in path.elements() {
        match *el {
            kurbo::PathEl::MoveTo(p) => {
                runs.push((Vec::new(), false));
                start = p;
                last = p;
            }
            kurbo::PathEl::LineTo(p) => {
                if let Some(r) = runs.last_mut() {
                    r.0.push(PathSeg::Line(kurbo::Line::new(last, p)));
                }
                last = p;
            }
            kurbo::PathEl::QuadTo(a, p) => {
                if let Some(r) = runs.last_mut() {
                    r.0.push(PathSeg::Quad(kurbo::QuadBez::new(last, a, p)));
                }
                last = p;
            }
            kurbo::PathEl::CurveTo(a, b, p) => {
                if let Some(r) = runs.last_mut() {
                    r.0.push(PathSeg::Cubic(kurbo::CubicBez::new(last, a, b, p)));
                }
                last = p;
            }
            kurbo::PathEl::ClosePath => {
                if let Some(r) = runs.last_mut() {
                    r.1 = true;
                }
                last = start;
            }
        }
    }
    runs.into_iter()
        .filter(|(s, closed)| !closed && !s.is_empty())
        .map(|(s, _)| s)
        .collect()
}

/// Corta `len` do começo (`at_start`) ou do fim do segmento.
fn trim(seg: PathSeg, len: f64, at_start: bool) -> PathSeg {
    let total = seg.arclen(1e-3);
    if len <= 0.0 || total <= 0.0 {
        return seg;
    }
    let keep = (total - len).max(total * 0.05);
    if at_start {
        let t = seg.inv_arclen(total - keep, 1e-3);
        seg.subsegment(t..1.0)
    } else {
        let t = seg.inv_arclen(keep, 1e-3);
        seg.subsegment(0.0..t)
    }
}

fn seg_el(s: &PathSeg, path: &mut BezPath) {
    match s {
        PathSeg::Line(l) => path.line_to(l.p1),
        PathSeg::Quad(q) => path.quad_to(q.p1, q.p2),
        PathSeg::Cubic(c) => path.curve_to(c.p1, c.p2, c.p3),
    }
}

/// Forma da ponta já na página.
fn place(a: &Arrow, end: End, width: f64) -> (BezPath, f64) {
    let size = width.max(MIN_WIDTH) * BASE * a.scale.clamp(0.05, 50.0);
    let (mut p, setback) = unit(&a.shape);
    let dir = end.out.y.atan2(end.out.x) + a.rotation.to_radians();
    let tip = end.point + end.out * a.offset;
    let mirror = if a.mirror {
        Affine::scale_non_uniform(1.0, -1.0)
    } else {
        Affine::IDENTITY
    };
    p.apply_affine(
        Affine::translate(tip.to_vec2()) * Affine::rotate(dir) * mirror * Affine::scale(size),
    );
    (p, (setback * size - a.offset).max(0.0))
}

/// Pontas do contorno (`start`/`end`) para `path` (na página) com
/// espessura `width`: as formas das pontas e o caminho com as pontas
/// recuadas para o traço não passar delas.
pub fn arrows(
    path: &BezPath,
    start: Option<&Arrow>,
    end: Option<&Arrow>,
    width: f64,
) -> Option<(Vec<BezPath>, BezPath)> {
    if start.is_none() && end.is_none() {
        return None;
    }
    let runs = open_runs(path);
    if runs.is_empty() {
        return None;
    }
    let mut heads = Vec::new();
    let mut trimmed = BezPath::new();
    // Subcaminhos fechados continuam como estão.
    let mut closed = BezPath::new();
    let mut cur: Vec<kurbo::PathEl> = Vec::new();
    for el in path.elements() {
        if matches!(el, kurbo::PathEl::MoveTo(_)) {
            cur.clear();
        }
        cur.push(*el);
        if matches!(el, kurbo::PathEl::ClosePath) {
            closed.extend(cur.drain(..));
        }
    }
    for mut segs in runs {
        if let Some(a) = start {
            let s0 = segs[0];
            let e = End {
                point: s0.eval(0.0),
                out: -tangent(&s0, 0.0),
            };
            let (shape, back) = place(a, e, width);
            heads.push(shape);
            segs[0] = trim(s0, back, true);
        }
        if let Some(a) = end {
            let n = segs.len() - 1;
            let s1 = segs[n];
            let e = End {
                point: s1.eval(1.0),
                out: tangent(&s1, 1.0),
            };
            let (shape, back) = place(a, e, width);
            heads.push(shape);
            segs[n] = trim(s1, back, false);
        }
        trimmed.move_to(segs[0].eval(0.0));
        for s in &segs {
            seg_el(s, &mut trimmed);
        }
    }
    trimmed.extend(closed.elements().iter().copied());
    Some((heads, trimmed))
}

#[cfg(test)]
mod tests {
    use super::*;
    use kurbo::Shape;

    fn arrow(shape: &str) -> Arrow {
        Arrow {
            shape: shape.into(),
            scale: 1.0,
            offset: 0.0,
            rotation: 0.0,
            mirror: false,
            extra: Default::default(),
        }
    }

    /// GD-OUT-05: a ponta fica na extremidade, alinhada à direção do
    /// caminho, com o tamanho pela espessura; o traço recua.
    #[test]
    fn gd_out_05_arrow_at_end() {
        let mut p = BezPath::new();
        p.move_to((0.0, 0.0));
        p.line_to((10_000.0, 0.0));
        let (heads, trimmed) = arrows(&p, None, Some(&arrow("triangle")), 500.0).unwrap();
        assert_eq!(heads.len(), 1);
        let b = heads[0].bounding_box();
        // Comprimento 4 × 500 = 2000 µm, ponta em x = 10 000.
        assert!(
            (b.x1 - 10_000.0).abs() < 1e-6 && (b.x0 - 8_000.0).abs() < 1e-6,
            "{b:?}"
        );
        assert!((b.height() - 1_800.0).abs() < 1e-6);
        let tb = trimmed.bounding_box();
        assert!((tb.x1 - 8_200.0).abs() < 1.0, "recuo: {tb:?}");
        // No começo, aponta para trás; espelhar e girar mudam a forma.
        let (heads, _) = arrows(&p, Some(&arrow("triangle")), None, 500.0).unwrap();
        let b = heads[0].bounding_box();
        assert!(
            (b.x0 - 0.0).abs() < 1e-6 && (b.x1 - 2_000.0).abs() < 1e-6,
            "{b:?}"
        );
        let mut turned = arrow("bar");
        turned.rotation = 90.0;
        let (heads, _) = arrows(&p, None, Some(&turned), 500.0).unwrap();
        let b = heads[0].bounding_box();
        assert!(b.width() > b.height(), "barra girada: {b:?}");
        // Caminho fechado não tem pontas.
        let r = kurbo::Rect::new(0.0, 0.0, 10.0, 10.0).to_path(0.1);
        assert!(arrows(&r, None, Some(&arrow("triangle")), 500.0).is_none());
    }
}
