//! Converter em curvas (GD-CNV-01, subconjunto do M1: retângulo e elipse).
//! A geometria resultante é a mesma desenhada na tela.

use kurbo::{BezPath, PathEl};
use serde_json::Map;

use crate::model::{FillRule, NodeKind, Object, ObjectKind, PathData, Subpath, SubpathBuilder};
use crate::render::scene::{ellipse_path, rect_path};

/// Converte um `BezPath` em subcaminhos de nós.
pub fn bez_to_subpaths(path: &BezPath) -> Vec<Subpath> {
    let mut b = SubpathBuilder::default();
    let mut cur = [0.0, 0.0];
    for el in path.elements() {
        match *el {
            PathEl::MoveTo(p) => {
                cur = [p.x, p.y];
                b.move_to(cur);
            }
            PathEl::LineTo(p) => {
                cur = [p.x, p.y];
                b.line_to(cur);
            }
            PathEl::QuadTo(c, p) => {
                b.quad_to([c.x, c.y], [p.x, p.y]);
                cur = [p.x, p.y];
            }
            PathEl::CurveTo(c1, c2, p) => {
                b.curve_to([c1.x, c1.y], [c2.x, c2.y], [p.x, p.y]);
                cur = [p.x, p.y];
            }
            PathEl::ClosePath => b.close(),
        }
    }
    let _ = cur;
    let mut subpaths = b.finish();
    for sp in &mut subpaths {
        mark_smooth_nodes(sp);
    }
    subpaths
}

/// Nós com alças colineares viram "suave" (ou "simétrico", se iguais).
pub fn mark_smooth_nodes(sp: &mut Subpath) {
    for n in &mut sp.nodes {
        if let (Some(i), Some(o)) = (n.handle_in, n.handle_out) {
            let a = kurbo::Vec2::new(n.p[0] - i[0], n.p[1] - i[1]);
            let b = kurbo::Vec2::new(o[0] - n.p[0], o[1] - n.p[1]);
            let (la, lb) = (a.hypot(), b.hypot());
            if la > 0.0 && lb > 0.0 && (a.cross(b) / (la * lb)).abs() < 1e-6 && a.dot(b) > 0.0 {
                n.kind = if (la - lb).abs() < 1e-6 * la.max(lb) {
                    NodeKind::Symmetric
                } else {
                    NodeKind::Smooth
                };
            }
        }
    }
}

/// Converte primitivas paramétricas em caminho, mantendo matriz, cor e
/// demais atributos. Caminhos e outros tipos são devolvidos sem mudança.
pub fn to_curves(o: &Object) -> Option<Object> {
    let bez = match &o.kind {
        ObjectKind::Rectangle(r) => rect_path(r),
        ObjectKind::Ellipse(e) => ellipse_path(e),
        ObjectKind::Basic(b) => crate::render::basic::path(b),
        ObjectKind::BSpline(b) => crate::render::bspline::path(b),
        ObjectKind::Polygon(p) => {
            let mut out = o.clone();
            out.kind = ObjectKind::Path(PathData {
                subpaths: bez_to_subpaths(&crate::render::scene::polygon_path(p)),
                fill_rule: crate::render::scene::polygon_fill_rule(p),
                extra: Map::new(),
            });
            return Some(out);
        }
        ObjectKind::Text(t) => return text_to_curves(o, t),
        _ => return None,
    };
    let mut out = o.clone();
    out.kind = ObjectKind::Path(PathData {
        subpaths: bez_to_subpaths(&bez),
        fill_rule: FillRule::Nonzero,
        extra: Map::new(),
    });
    Some(out)
}

/// Texto em curvas (GD-TXT-04): os glifos viram caminhos. Com a mesma cor
/// em todo o texto sai um caminho só; com cores por trecho, um grupo com um
/// caminho por trecho.
fn text_to_curves(o: &Object, t: &crate::model::TextData) -> Option<Object> {
    let layout = crate::render::text::layout_text(t);
    let runs: Vec<_> = layout.lines.iter().flat_map(|l| l.runs.iter()).collect();
    if runs.iter().all(|r| r.outline.elements().is_empty()) {
        return None;
    }
    let path = |bez: &BezPath| {
        ObjectKind::Path(PathData {
            subpaths: bez_to_subpaths(bez),
            fill_rule: FillRule::Nonzero,
            extra: Map::new(),
        })
    };
    let first = runs[0].fill.clone();
    let mut out = o.clone();
    if runs.iter().all(|r| r.fill == first) {
        let mut all = BezPath::new();
        for r in &runs {
            all.extend(r.outline.elements().iter().copied());
        }
        out.kind = path(&all);
        if first.is_some() {
            out.fill = first;
        }
        return Some(out);
    }
    let children = runs
        .iter()
        .filter(|r| !r.outline.elements().is_empty())
        .map(|r| {
            let mut c = Object::new(path(&r.outline));
            c.fill = r.fill.clone().or_else(|| o.fill.clone());
            c.stroke = o.stroke.clone();
            c
        })
        .collect();
    out.kind = ObjectKind::Group(crate::model::GroupData {
        children,
        extra: Map::new(),
    });
    Some(out)
}

#[cfg(test)]
mod polygon_tests {
    use super::*;
    use crate::model::PolygonData;
    use crate::render::scene::polygon_path;
    use kurbo::Shape;

    fn poly(sides: u32, mode: &str, sharpness: f64) -> PolygonData {
        PolygonData {
            rx: 10.0,
            ry: 10.0,
            sides,
            mode: mode.into(),
            sharpness,
            extra: Map::new(),
        }
    }

    #[test]
    fn gd_pri_03_04_polygon_star_and_complex() {
        // Hexágono regular de raio 10: área 3√3/2·r².
        let hex = polygon_path(&poly(6, "polygon", 0.5));
        let want = 3.0 * 3f64.sqrt() / 2.0 * 100.0;
        assert!((hex.area().abs() - want).abs() < 1e-9);
        // Primeiro vértice em cima.
        assert!(matches!(hex.elements()[0], PathEl::MoveTo(p) if (p.y - 10.0).abs() < 1e-9));
        // Estrela de 5 pontas: 10 vértices; mais nitidez = menos área.
        let soft = polygon_path(&poly(5, "star", 0.2));
        let sharp = polygon_path(&poly(5, "star", 0.8));
        assert_eq!(soft.elements().len(), 11);
        assert!(sharp.area().abs() < soft.area().abs());
        // Complexa {7/3}: um traçado só de 7 linhas; {8/2}... k=2 com 8 → 2 traçados.
        let c7 = polygon_path(&poly(7, "complex", 1.0));
        assert_eq!(
            c7.elements()
                .iter()
                .filter(|e| matches!(e, PathEl::MoveTo(_)))
                .count(),
            1
        );
        let c8 = polygon_path(&poly(8, "complex", 0.0));
        assert_eq!(
            c8.elements()
                .iter()
                .filter(|e| matches!(e, PathEl::MoveTo(_)))
                .count(),
            2
        );
        // Lados fora da faixa ficam em 3–500.
        assert_eq!(polygon_path(&poly(1, "polygon", 0.5)).elements().len(), 4);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry;
    use crate::model::{EllipseData, RectData};

    /// Aceite do doc 03: círculo de raio 10 mm convertido a curvas →
    /// comprimento 62,83 mm ± 0,01; área 314,16 mm² ± 0,05.
    #[test]
    fn gd_geo_acceptance_circle_to_curves() {
        let circle = Object::new(ObjectKind::Ellipse(EllipseData {
            rx: 10_000.0,
            ry: 10_000.0,
            mode: "ellipse".into(),
            start_angle: 0.0,
            end_angle: 360.0,
            extra: Map::new(),
        }));
        let curves = to_curves(&circle).unwrap();
        let ObjectKind::Path(p) = &curves.kind else {
            panic!()
        };
        assert_eq!(p.subpaths.len(), 1);
        assert_eq!(p.subpaths[0].nodes.len(), 4);
        assert!(
            p.subpaths[0]
                .nodes
                .iter()
                .all(|n| n.kind == NodeKind::Symmetric)
        );
        let len_mm = geometry::length(p) / 1000.0;
        let area_mm2 = geometry::signed_area(p).abs() / 1e6;
        assert!((len_mm - 62.83).abs() <= 0.01, "comprimento {len_mm}");
        assert!((area_mm2 - 314.16).abs() <= 0.05, "área {area_mm2}");
    }

    #[test]
    fn rectangle_to_curves_keeps_attributes() {
        let mut r = Object::new(ObjectKind::Rectangle(RectData {
            width: 10.0,
            height: 5.0,
            corners: None,
            extra: Map::new(),
        }));
        r.fill = Some(crate::model::Fill::solid(crate::model::Color::rgb(
            1.0, 0.0, 0.0,
        )));
        r.transform = crate::model::Transform::translate(3.0, 4.0);
        let c = to_curves(&r).unwrap();
        assert_eq!(c.id, r.id);
        assert_eq!(c.fill, r.fill);
        assert_eq!(c.transform, r.transform);
        let ObjectKind::Path(p) = &c.kind else {
            panic!()
        };
        assert_eq!(p.subpaths[0].nodes.len(), 4);
        assert!((geometry::signed_area(p).abs() - 50.0).abs() < 1e-9);
    }
}
