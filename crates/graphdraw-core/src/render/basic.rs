//! Formas básicas (GD-PRI-07): biblioteca própria de setas, fluxograma,
//! faixas e balões, paramétricas, na caixa local `[0,w] × [0,h]` (Y para
//! cima). Cada parâmetro tem uma alça de ajuste; a ponta do balão de fala é
//! a chamada (GD-FER-42), que pode sair da caixa.

use kurbo::{BezPath, Point, Shape};

use crate::model::BasicShapeData;

/// Formas da biblioteca, na ordem da interface.
pub const SHAPES: [&str; 13] = [
    "arrow",
    "double-arrow",
    "chevron",
    "flow-process",
    "flow-decision",
    "flow-terminator",
    "flow-data",
    "flow-document",
    "banner",
    "speech",
    "thought",
    "plus",
    "heart",
];

/// Parâmetros padrão da forma.
pub fn default_params(shape: &str) -> Vec<f64> {
    match shape {
        "arrow" => vec![0.35, 0.5],
        "double-arrow" => vec![0.25, 0.5],
        "chevron" => vec![0.3],
        "flow-data" => vec![0.2],
        "flow-document" => vec![0.12],
        "banner" => vec![0.12],
        "speech" | "thought" => vec![0.25, -0.35],
        "plus" => vec![0.34],
        _ => Vec::new(),
    }
}

fn p(d: &BasicShapeData, i: usize) -> f64 {
    d.params
        .get(i)
        .copied()
        .unwrap_or_else(|| default_params(&d.shape).get(i).copied().unwrap_or(0.0))
}

fn poly(pts: &[(f64, f64)]) -> BezPath {
    let mut b = BezPath::new();
    for (i, q) in pts.iter().enumerate() {
        if i == 0 {
            b.move_to(*q);
        } else {
            b.line_to(*q);
        }
    }
    b.close_path();
    b
}

/// Retângulo arredondado com uma "cauda" saindo da borda de baixo (ou de
/// cima) até `tip`.
fn balloon(w: f64, h: f64, tip: Point) -> BezPath {
    let r = 0.18 * w.min(h);
    let below = tip.y < h / 2.0;
    let tw = (0.18 * w).min(w - 2.0 * r).max(0.0);
    let cx = tip.x.clamp(r + tw / 2.0, w - r - tw / 2.0);
    let k = 0.552_284_75 * r;
    let mut b = BezPath::new();
    // Sentido anti-horário a partir do canto de baixo à esquerda.
    b.move_to((r, 0.0));
    if below {
        b.line_to((cx - tw / 2.0, 0.0));
        b.line_to(tip);
        b.line_to((cx + tw / 2.0, 0.0));
    }
    b.line_to((w - r, 0.0));
    b.curve_to((w - r + k, 0.0), (w, r - k), (w, r));
    b.line_to((w, h - r));
    b.curve_to((w, h - r + k), (w - r + k, h), (w - r, h));
    if !below {
        b.line_to((cx + tw / 2.0, h));
        b.line_to(tip);
        b.line_to((cx - tw / 2.0, h));
    }
    b.line_to((r, h));
    b.curve_to((r - k, h), (0.0, h - r + k), (0.0, h - r));
    b.line_to((0.0, r));
    b.curve_to((0.0, r - k), (r - k, 0.0), (r, 0.0));
    b.close_path();
    b
}

/// Contorno da forma (espaço local).
pub fn path(d: &BasicShapeData) -> BezPath {
    let (w, h) = (d.width.max(1.0), d.height.max(1.0));
    match d.shape.as_str() {
        "arrow" => {
            let head = w * p(d, 0).clamp(0.05, 0.95);
            let half = h * p(d, 1).clamp(0.05, 1.0) / 2.0;
            let x = w - head;
            poly(&[
                (0.0, h / 2.0 - half),
                (x, h / 2.0 - half),
                (x, 0.0),
                (w, h / 2.0),
                (x, h),
                (x, h / 2.0 + half),
                (0.0, h / 2.0 + half),
            ])
        }
        "double-arrow" => {
            let head = w * p(d, 0).clamp(0.05, 0.49);
            let half = h * p(d, 1).clamp(0.05, 1.0) / 2.0;
            poly(&[
                (0.0, h / 2.0),
                (head, 0.0),
                (head, h / 2.0 - half),
                (w - head, h / 2.0 - half),
                (w - head, 0.0),
                (w, h / 2.0),
                (w - head, h),
                (w - head, h / 2.0 + half),
                (head, h / 2.0 + half),
                (head, h),
            ])
        }
        "chevron" => {
            let n = w * p(d, 0).clamp(0.0, 0.9);
            poly(&[
                (0.0, 0.0),
                (w - n, 0.0),
                (w, h / 2.0),
                (w - n, h),
                (0.0, h),
                (n, h / 2.0),
            ])
        }
        "flow-decision" => poly(&[(w / 2.0, 0.0), (w, h / 2.0), (w / 2.0, h), (0.0, h / 2.0)]),
        "flow-terminator" => kurbo::RoundedRect::new(0.0, 0.0, w, h, w.min(h) / 2.0).to_path(0.1),
        "flow-data" => {
            let s = w * p(d, 0).clamp(0.0, 0.5);
            poly(&[(0.0, 0.0), (w - s, 0.0), (w, h), (s, h)])
        }
        "flow-document" => {
            let a = h * p(d, 0).clamp(0.0, 0.3);
            let mut b = BezPath::new();
            b.move_to((0.0, h));
            b.line_to((0.0, a));
            b.curve_to((w / 3.0, -a * 0.9), (2.0 * w / 3.0, a * 2.9), (w, a));
            b.line_to((w, h));
            b.close_path();
            b
        }
        "banner" => {
            let n = w * p(d, 0).clamp(0.0, 0.4);
            poly(&[
                (0.0, 0.0),
                (w, 0.0),
                (w - n, h / 2.0),
                (w, h),
                (0.0, h),
                (n, h / 2.0),
            ])
        }
        "speech" => balloon(w, h, Point::new(p(d, 0) * w, p(d, 1) * h)),
        "thought" => {
            let mut b =
                kurbo::Ellipse::new((w / 2.0, h / 2.0), (w / 2.0, h / 2.0), 0.0).to_path(0.1);
            let tip = Point::new(p(d, 0) * w, p(d, 1) * h);
            let from = Point::new(w / 2.0, h / 2.0);
            let r0 = 0.06 * w.min(h);
            for (k, t) in [0.62, 0.8, 1.0].into_iter().enumerate() {
                let c = from.lerp(tip, t);
                let r = r0 * (1.0 - 0.3 * k as f64);
                b.extend(
                    kurbo::Circle::new(c, r)
                        .to_path(0.1)
                        .elements()
                        .iter()
                        .copied(),
                );
            }
            b
        }
        "plus" => {
            let t = p(d, 0).clamp(0.05, 0.95);
            let (tx, ty) = (w * t / 2.0, h * t / 2.0);
            let (cx, cy) = (w / 2.0, h / 2.0);
            poly(&[
                (cx - tx, 0.0),
                (cx + tx, 0.0),
                (cx + tx, cy - ty),
                (w, cy - ty),
                (w, cy + ty),
                (cx + tx, cy + ty),
                (cx + tx, h),
                (cx - tx, h),
                (cx - tx, cy + ty),
                (0.0, cy + ty),
                (0.0, cy - ty),
                (cx - tx, cy - ty),
            ])
        }
        "heart" => {
            let mut b = BezPath::new();
            b.move_to((w / 2.0, 0.0));
            b.curve_to(
                (w * 0.15, h * 0.28),
                (-w * 0.05, h * 0.62),
                (w * 0.25, h * 0.95),
            );
            b.curve_to(
                (w * 0.38, h * 1.06),
                (w * 0.5, h * 0.92),
                (w / 2.0, h * 0.78),
            );
            b.curve_to(
                (w * 0.5, h * 0.92),
                (w * 0.62, h * 1.06),
                (w * 0.75, h * 0.95),
            );
            b.curve_to((w * 1.05, h * 0.62), (w * 0.85, h * 0.28), (w / 2.0, 0.0));
            b.close_path();
            b
        }
        // flow-process e desconhecidas: retângulo.
        _ => kurbo::Rect::new(0.0, 0.0, w, h).to_path(0.1),
    }
}

/// Alças de ajuste (espaço local), uma por parâmetro (balões: uma, a ponta).
pub fn handles(d: &BasicShapeData) -> Vec<Point> {
    let (w, h) = (d.width.max(1.0), d.height.max(1.0));
    match d.shape.as_str() {
        "arrow" => vec![
            Point::new(w * (1.0 - p(d, 0)), h),
            Point::new(0.0, h / 2.0 + h * p(d, 1) / 2.0),
        ],
        "double-arrow" => vec![
            Point::new(w * p(d, 0), h),
            Point::new(w / 2.0, h / 2.0 + h * p(d, 1) / 2.0),
        ],
        "chevron" | "banner" => vec![Point::new(w * p(d, 0), h / 2.0)],
        "flow-data" => vec![Point::new(w * p(d, 0), h)],
        "flow-document" => vec![Point::new(w / 2.0, h * p(d, 0))],
        "speech" | "thought" => vec![Point::new(w * p(d, 0), h * p(d, 1))],
        "plus" => vec![Point::new(w / 2.0 + w * p(d, 0) / 2.0, h)],
        _ => Vec::new(),
    }
}

/// Move a alça `i` para `q` (espaço local) e ajusta o parâmetro.
pub fn set_handle(d: &mut BasicShapeData, i: usize, q: Point) {
    let (w, h) = (d.width.max(1.0), d.height.max(1.0));
    if d.params.is_empty() {
        d.params = default_params(&d.shape);
    }
    let set = |d: &mut BasicShapeData, k: usize, v: f64| {
        if let Some(x) = d.params.get_mut(k) {
            *x = v;
        }
    };
    match (d.shape.as_str(), i) {
        ("arrow", 0) => set(d, 0, (1.0 - q.x / w).clamp(0.05, 0.95)),
        ("arrow", 1) | ("double-arrow", 1) => {
            set(d, 1, ((q.y - h / 2.0) * 2.0 / h).clamp(0.05, 1.0))
        }
        ("double-arrow", 0) => set(d, 0, (q.x / w).clamp(0.05, 0.49)),
        ("chevron", 0) => set(d, 0, (q.x / w).clamp(0.0, 0.9)),
        ("banner", 0) => set(d, 0, (q.x / w).clamp(0.0, 0.4)),
        ("flow-data", 0) => set(d, 0, (q.x / w).clamp(0.0, 0.5)),
        ("flow-document", 0) => set(d, 0, (q.y / h).clamp(0.0, 0.3)),
        ("speech", 0) | ("thought", 0) => {
            set(d, 0, q.x / w);
            set(d, 1, q.y / h);
        }
        ("plus", 0) => set(d, 0, ((q.x - w / 2.0) * 2.0 / w).clamp(0.05, 0.95)),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kurbo::Shape;

    fn shape(name: &str) -> BasicShapeData {
        BasicShapeData {
            shape: name.into(),
            width: 100.0,
            height: 50.0,
            params: default_params(name),
            extra: Default::default(),
        }
    }

    /// GD-PRI-07: toda forma da biblioteca gera um contorno fechado dentro da
    /// caixa (balões podem sair pela ponta).
    #[test]
    fn gd_pri_07_every_shape_has_outline() {
        for s in SHAPES {
            let b = path(&shape(s));
            assert!(!b.elements().is_empty(), "{s}");
            let bb = b.bounding_box();
            if !matches!(s, "speech" | "thought" | "heart") {
                assert!(
                    bb.x0 >= -1e-6
                        && bb.y0 >= -1e-6
                        && bb.x1 <= 100.0 + 1e-6
                        && bb.y1 <= 50.0 + 1e-6,
                    "{s}: {bb:?}"
                );
            }
            assert_eq!(
                handles(&shape(s)).len(),
                match s {
                    "arrow" | "double-arrow" => 2,
                    "flow-process" | "flow-decision" | "flow-terminator" | "heart" => 0,
                    _ => 1,
                },
                "{s}"
            );
        }
    }

    /// Alça muda o parâmetro e o desenho; a ponta da chamada sai da caixa.
    #[test]
    fn gd_pri_07_handles_adjust_and_callout_tip() {
        let mut a = shape("arrow");
        set_handle(&mut a, 0, Point::new(50.0, 50.0));
        assert!((a.params[0] - 0.5).abs() < 1e-9);
        assert_eq!(handles(&a)[0], Point::new(50.0, 50.0));
        let mut s = shape("speech");
        set_handle(&mut s, 0, Point::new(10.0, -40.0));
        let bb = path(&s).bounding_box();
        assert!((bb.y0 + 40.0).abs() < 1e-6, "ponta abaixo da caixa: {bb:?}");
        assert!((s.params[1] + 0.8).abs() < 1e-9);
    }
}
