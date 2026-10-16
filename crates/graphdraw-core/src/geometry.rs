//! Cálculos sobre caminhos (GD-GEO-05): comprimento, área com sinal, ponto,
//! tangente e normal em parâmetro `t` e em distância `s`, ponto mais
//! próximo, interseções (segmento × segmento, caminho × caminho) e teste de
//! ponto dentro.
//!
//! Segmentos são indexados por subcaminho: o segmento `i` do subcaminho `k`
//! liga o nó `i` ao nó `i+1` (ou ao nó 0, no segmento de fechamento).

use kurbo::{
    CubicBez, Line, ParamCurve, ParamCurveArclen, ParamCurveDeriv, ParamCurveNearest, PathSeg,
    Point, Vec2,
};

use crate::model::{FillRule, Node, PathData, Subpath};

/// Precisão padrão dos cálculos numéricos: 0,001 µm.
pub const ACCURACY: f64 = 1e-3;

/// Referência a um segmento: subcaminho e índice do nó inicial.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SegRef {
    pub subpath: usize,
    pub index: usize,
}

fn pt(p: [f64; 2]) -> Point {
    Point::new(p[0], p[1])
}

/// Segmento entre dois nós consecutivos.
pub fn segment_between(a: &Node, b: &Node) -> PathSeg {
    if a.handle_out.is_none() && b.handle_in.is_none() {
        PathSeg::Line(Line::new(pt(a.p), pt(b.p)))
    } else {
        PathSeg::Cubic(CubicBez::new(
            pt(a.p),
            pt(a.handle_out.unwrap_or(a.p)),
            pt(b.handle_in.unwrap_or(b.p)),
            pt(b.p),
        ))
    }
}

/// Número de segmentos de um subcaminho.
pub fn segment_count(sp: &Subpath) -> usize {
    match sp.nodes.len() {
        0 | 1 => 0,
        n if sp.closed => n,
        n => n - 1,
    }
}

pub fn subpath_segments(sp: &Subpath) -> Vec<PathSeg> {
    let n = sp.nodes.len();
    (0..segment_count(sp))
        .map(|i| segment_between(&sp.nodes[i], &sp.nodes[(i + 1) % n]))
        .collect()
}

/// Todos os segmentos do caminho com sua referência.
pub fn segments(path: &PathData) -> Vec<(SegRef, PathSeg)> {
    path.subpaths
        .iter()
        .enumerate()
        .flat_map(|(k, sp)| {
            subpath_segments(sp)
                .into_iter()
                .enumerate()
                .map(move |(i, s)| {
                    (
                        SegRef {
                            subpath: k,
                            index: i,
                        },
                        s,
                    )
                })
        })
        .collect()
}

pub fn segment(path: &PathData, r: SegRef) -> Option<PathSeg> {
    let sp = path.subpaths.get(r.subpath)?;
    if r.index >= segment_count(sp) {
        return None;
    }
    Some(segment_between(
        &sp.nodes[r.index],
        &sp.nodes[(r.index + 1) % sp.nodes.len()],
    ))
}

// ---- comprimento e área ------------------------------------------------

pub fn segment_length(s: &PathSeg) -> f64 {
    s.arclen(ACCURACY)
}

pub fn subpath_length(sp: &Subpath) -> f64 {
    subpath_segments(sp).iter().map(segment_length).sum()
}

pub fn length(path: &PathData) -> f64 {
    path.subpaths.iter().map(subpath_length).sum()
}

/// Área com sinal de um subcaminho (considerado fechado). Positiva quando
/// os nós seguem o sentido anti-horário (eixo Y para cima).
pub fn subpath_signed_area(sp: &Subpath) -> f64 {
    let n = sp.nodes.len();
    if n < 2 {
        return 0.0;
    }
    let mut segs = subpath_segments(sp);
    if !sp.closed {
        segs.push(PathSeg::Line(Line::new(
            pt(sp.nodes[n - 1].p),
            pt(sp.nodes[0].p),
        )));
    }
    segs.iter().map(kurbo::ParamCurveArea::signed_area).sum()
}

/// Soma das áreas com sinal dos subcaminhos (furos em sentido oposto
/// subtraem).
pub fn signed_area(path: &PathData) -> f64 {
    path.subpaths.iter().map(subpath_signed_area).sum()
}

// ---- ponto, tangente e normal -------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frame {
    pub point: Point,
    /// Tangente unitária (sentido do caminho).
    pub tangent: Vec2,
    /// Normal unitária (tangente girada 90° no sentido anti-horário).
    pub normal: Vec2,
}

fn deriv_at(s: &PathSeg, t: f64) -> Vec2 {
    let d = match s {
        PathSeg::Line(l) => l.p1 - l.p0,
        PathSeg::Quad(q) => q.deriv().eval(t).to_vec2(),
        PathSeg::Cubic(c) => c.deriv().eval(t).to_vec2(),
    };
    if d.hypot2() > 1e-18 {
        return d;
    }
    // Alça coincidente com o nó: usa a direção até o próximo ponto útil.
    let eps = if t < 0.5 { 1e-4 } else { -1e-4 };
    (s.eval((t + eps).clamp(0.0, 1.0)) - s.eval(t)) * eps.signum()
}

pub fn frame_at(s: &PathSeg, t: f64) -> Frame {
    let tangent = deriv_at(s, t).normalize();
    Frame {
        point: s.eval(t),
        tangent,
        normal: Vec2::new(-tangent.y, tangent.x),
    }
}

/// Ponto/tangente/normal no parâmetro `t` (0–1) de um segmento.
pub fn frame_at_t(path: &PathData, r: SegRef, t: f64) -> Option<Frame> {
    Some(frame_at(&segment(path, r)?, t.clamp(0.0, 1.0)))
}

/// Ponto/tangente/normal a uma distância `s` (µm) do início de um subcaminho.
/// Devolve também o segmento e o `t` correspondentes.
pub fn frame_at_distance(path: &PathData, subpath: usize, s: f64) -> Option<(Frame, SegRef, f64)> {
    let sp = path.subpaths.get(subpath)?;
    let segs = subpath_segments(sp);
    let mut remaining = s.max(0.0);
    for (i, seg) in segs.iter().enumerate() {
        let len = segment_length(seg);
        if remaining <= len || i + 1 == segs.len() {
            let t = if len > 0.0 {
                seg.inv_arclen(remaining.min(len), ACCURACY)
            } else {
                0.0
            };
            return Some((frame_at(seg, t), SegRef { subpath, index: i }, t));
        }
        remaining -= len;
    }
    None
}

// ---- ponto mais próximo -------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Nearest {
    pub seg: SegRef,
    pub t: f64,
    pub point: Point,
    pub distance: f64,
}

pub fn nearest(path: &PathData, p: Point) -> Option<Nearest> {
    segments(path)
        .into_iter()
        .map(|(r, s)| {
            let n = s.nearest(p, ACCURACY);
            Nearest {
                seg: r,
                t: n.t,
                point: s.eval(n.t),
                distance: n.distance_sq.sqrt(),
            }
        })
        .min_by(|a, b| a.distance.total_cmp(&b.distance))
}

// ---- interseções --------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Intersection {
    pub point: Point,
    /// Parâmetros nos dois segmentos.
    pub t1: f64,
    pub t2: f64,
}

fn bbox(s: &PathSeg) -> kurbo::Rect {
    kurbo::Shape::bounding_box(s)
}

/// Interseções de dois segmentos. Retas: solução analítica; reta × curva:
/// raízes da cúbica; curva × curva: subdivisão recursiva até a tolerância.
pub fn intersect_segments(a: &PathSeg, b: &PathSeg) -> Vec<Intersection> {
    let mut out = Vec::new();
    match (a, b) {
        (PathSeg::Line(la), PathSeg::Line(lb)) => {
            if let Some((t1, t2)) = line_line(*la, *lb) {
                out.push(Intersection {
                    point: la.eval(t1),
                    t1,
                    t2,
                });
            }
        }
        (PathSeg::Line(l), curve) => {
            for x in curve.intersect_line(*l) {
                out.push(Intersection {
                    point: l.eval(x.line_t),
                    t1: x.line_t,
                    t2: x.segment_t,
                });
            }
        }
        (curve, PathSeg::Line(l)) => {
            for x in curve.intersect_line(*l) {
                out.push(Intersection {
                    point: l.eval(x.line_t),
                    t1: x.segment_t,
                    t2: x.line_t,
                });
            }
        }
        _ => curve_curve(a, (0.0, 1.0), b, (0.0, 1.0), 0, &mut out),
    }
    dedup(out)
}

fn line_line(a: Line, b: Line) -> Option<(f64, f64)> {
    let d1 = a.p1 - a.p0;
    let d2 = b.p1 - b.p0;
    let den = d1.cross(d2);
    if den.abs() < 1e-12 {
        return None; // paralelas ou colineares
    }
    let w = b.p0 - a.p0;
    let t1 = w.cross(d2) / den;
    let t2 = w.cross(d1) / den;
    let e = 1e-9;
    ((-e..=1.0 + e).contains(&t1) && (-e..=1.0 + e).contains(&t2))
        .then_some((t1.clamp(0.0, 1.0), t2.clamp(0.0, 1.0)))
}

fn curve_curve(
    a: &PathSeg,
    ra: (f64, f64),
    b: &PathSeg,
    rb: (f64, f64),
    depth: u32,
    out: &mut Vec<Intersection>,
) {
    let sa = a.subsegment(ra.0..ra.1);
    let sb = b.subsegment(rb.0..rb.1);
    if !rects_touch(bbox(&sa), bbox(&sb)) {
        return;
    }
    let size = bbox(&sa).size().max_side().max(bbox(&sb).size().max_side());
    if size < ACCURACY || depth > 48 {
        let ta = (ra.0 + ra.1) / 2.0;
        let tb = (rb.0 + rb.1) / 2.0;
        out.push(Intersection {
            point: a.eval(ta).midpoint(b.eval(tb)),
            t1: ta,
            t2: tb,
        });
        return;
    }
    let ma = (ra.0 + ra.1) / 2.0;
    let mb = (rb.0 + rb.1) / 2.0;
    for x in [(ra.0, ma), (ma, ra.1)] {
        for y in [(rb.0, mb), (mb, rb.1)] {
            curve_curve(a, x, b, y, depth + 1, out);
        }
    }
}

fn rects_touch(a: kurbo::Rect, b: kurbo::Rect) -> bool {
    a.x0 <= b.x1 && b.x0 <= a.x1 && a.y0 <= b.y1 && b.y0 <= a.y1
}

fn dedup(mut v: Vec<Intersection>) -> Vec<Intersection> {
    v.sort_by(|a, b| a.t1.total_cmp(&b.t1));
    let mut out: Vec<Intersection> = Vec::new();
    for x in v {
        if out
            .last()
            .is_none_or(|l| (l.point - x.point).hypot() > ACCURACY * 10.0)
        {
            out.push(x);
        }
    }
    out
}

/// Interseções entre dois caminhos (ou de um caminho consigo mesmo, quando
/// `a` e `b` são o mesmo: segmentos vizinhos não contam o nó compartilhado).
pub fn intersect_paths(a: &PathData, b: &PathData) -> Vec<(SegRef, SegRef, Intersection)> {
    let sa = segments(a);
    let sb = segments(b);
    let same = std::ptr::eq(a, b);
    let mut out = Vec::new();
    for (i, (ra, x)) in sa.iter().enumerate() {
        for (j, (rb, y)) in sb.iter().enumerate() {
            if same && j <= i {
                continue;
            }
            if !rects_touch(bbox(x), bbox(y)) {
                continue;
            }
            for hit in intersect_segments(x, y) {
                if same && ra.subpath == rb.subpath {
                    let shared_end = (hit.t1 > 1.0 - 1e-6 && hit.t2 < 1e-6)
                        || (hit.t1 < 1e-6 && hit.t2 > 1.0 - 1e-6);
                    if shared_end {
                        continue;
                    }
                }
                out.push((*ra, *rb, hit));
            }
        }
    }
    out
}

// ---- ponto dentro -------------------------------------------------------

/// Teste de ponto dentro respeitando a regra de preenchimento (GD-GEO-04).
/// Subcaminhos abertos são considerados fechados por uma reta.
pub fn contains(path: &PathData, p: Point) -> bool {
    let mut winding = 0;
    for sp in &path.subpaths {
        let n = sp.nodes.len();
        if n < 2 {
            continue;
        }
        let mut segs = subpath_segments(sp);
        if !sp.closed {
            segs.push(PathSeg::Line(Line::new(
                pt(sp.nodes[n - 1].p),
                pt(sp.nodes[0].p),
            )));
        }
        let mut bez = kurbo::BezPath::new();
        bez.move_to(segs[0].start());
        for s in &segs {
            match s {
                PathSeg::Line(l) => bez.line_to(l.p1),
                PathSeg::Quad(q) => bez.quad_to(q.p1, q.p2),
                PathSeg::Cubic(c) => bez.curve_to(c.p1, c.p2, c.p3),
            }
        }
        bez.close_path();
        winding += kurbo::Shape::winding(&bez, p);
    }
    match path.fill_rule {
        FillRule::Evenodd => winding % 2 != 0,
        FillRule::Nonzero => winding != 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Node, SubpathBuilder};
    use serde_json::Map;

    fn square(x0: f64, y0: f64, s: f64, ccw: bool) -> Subpath {
        let mut pts = vec![[x0, y0], [x0 + s, y0], [x0 + s, y0 + s], [x0, y0 + s]];
        if !ccw {
            pts.reverse();
        }
        Subpath {
            closed: true,
            nodes: pts.into_iter().map(|p| Node::corner(p[0], p[1])).collect(),
            extra: Map::new(),
        }
    }

    fn path(subpaths: Vec<Subpath>) -> PathData {
        PathData {
            subpaths,
            fill_rule: FillRule::Evenodd,
            extra: Map::new(),
        }
    }

    #[test]
    fn square_length_area_contains() {
        let p = path(vec![square(0.0, 0.0, 10.0, true)]);
        assert!((length(&p) - 40.0).abs() < 1e-9);
        assert!((signed_area(&p) - 100.0).abs() < 1e-9);
        let cw = path(vec![square(0.0, 0.0, 10.0, false)]);
        assert!((signed_area(&cw) + 100.0).abs() < 1e-9);
        assert!(contains(&p, Point::new(5.0, 5.0)));
        assert!(!contains(&p, Point::new(15.0, 5.0)));
    }

    #[test]
    fn evenodd_and_nonzero_holes() {
        let mut p = path(vec![
            square(0.0, 0.0, 10.0, true),
            square(3.0, 3.0, 4.0, true),
        ]);
        assert!(!contains(&p, Point::new(5.0, 5.0)), "par-ímpar: furo");
        p.fill_rule = FillRule::Nonzero;
        assert!(
            contains(&p, Point::new(5.0, 5.0)),
            "não-zero, mesmo sentido: preenchido"
        );
        p.subpaths[1] = square(3.0, 3.0, 4.0, false);
        assert!(
            !contains(&p, Point::new(5.0, 5.0)),
            "não-zero, sentido oposto: furo"
        );
    }

    #[test]
    fn frames_and_distance() {
        let p = path(vec![square(0.0, 0.0, 10.0, true)]);
        let f = frame_at_t(
            &p,
            SegRef {
                subpath: 0,
                index: 1,
            },
            0.5,
        )
        .unwrap();
        assert_eq!(f.point, Point::new(10.0, 5.0));
        assert!((f.tangent - Vec2::new(0.0, 1.0)).hypot() < 1e-9);
        assert!((f.normal - Vec2::new(-1.0, 0.0)).hypot() < 1e-9);
        let (f, r, t) = frame_at_distance(&p, 0, 25.0).unwrap();
        assert_eq!(r.index, 2);
        assert!((t - 0.5).abs() < 1e-9);
        assert!((f.point - Point::new(5.0, 10.0)).hypot() < 1e-9);
    }

    #[test]
    fn nearest_point() {
        let p = path(vec![square(0.0, 0.0, 10.0, true)]);
        let n = nearest(&p, Point::new(12.0, 4.0)).unwrap();
        assert_eq!(n.seg.index, 1);
        assert!((n.distance - 2.0).abs() < 1e-6);
        assert!((n.point - Point::new(10.0, 4.0)).hypot() < 1e-6);
    }

    #[test]
    fn intersections() {
        let a = path(vec![square(0.0, 0.0, 10.0, true)]);
        let b = path(vec![square(5.0, 5.0, 10.0, true)]);
        let hits = intersect_paths(&a, &b);
        let mut pts: Vec<(i64, i64)> = hits
            .iter()
            .map(|h| (h.2.point.x.round() as i64, h.2.point.y.round() as i64))
            .collect();
        pts.sort();
        pts.dedup();
        assert_eq!(pts, vec![(5, 10), (10, 5)]);

        // Curva × curva: dois arcos que se cruzam uma vez.
        let c1 = PathSeg::Cubic(CubicBez::new(
            (0.0, 0.0),
            (0.0, 10.0),
            (10.0, 10.0),
            (10.0, 0.0),
        ));
        let c2 = PathSeg::Cubic(CubicBez::new(
            (0.0, 8.0),
            (3.0, 0.0),
            (7.0, 0.0),
            (10.0, 8.0),
        ));
        let hits = intersect_segments(&c1, &c2);
        assert_eq!(hits.len(), 2);
        for h in hits {
            assert!((c1.eval(h.t1) - c2.eval(h.t2)).hypot() < 0.01);
        }
    }

    #[test]
    fn self_intersection_of_figure_eight() {
        let mut b = SubpathBuilder::default();
        b.move_to([0.0, 0.0]);
        b.line_to([10.0, 10.0]);
        b.line_to([10.0, 0.0]);
        b.line_to([0.0, 10.0]);
        b.close();
        let p = path(b.finish());
        let hits = intersect_paths(&p, &p);
        assert_eq!(hits.len(), 1);
        assert!((hits[0].2.point - Point::new(5.0, 5.0)).hypot() < 1e-9);
    }
}
