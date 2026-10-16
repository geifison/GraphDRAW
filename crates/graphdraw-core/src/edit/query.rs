//! Consultas geométricas sobre objetos: caixas delimitadoras (GD-OBJ-06),
//! acerto de clique e seleção por retângulo (GD-SEL-01, GD-SEL-03).

use kurbo::{Affine, BezPath, Point, Rect, Shape};

use crate::geometry;
use crate::model::geom::subpaths_to_bez;
use crate::model::{Fill, Object, ObjectKind, Page};
use crate::render::scene::{ellipse_path, rect_path};
use crate::render::text::layout_text;

/// Contorno local de objetos com geometria de caminho.
pub fn local_outline(o: &Object) -> Option<BezPath> {
    Some(match &o.kind {
        ObjectKind::Path(p) => subpaths_to_bez(&p.subpaths),
        ObjectKind::Rectangle(r) => rect_path(r),
        ObjectKind::Ellipse(e) => ellipse_path(e),
        ObjectKind::Image(i) => Rect::new(0.0, 0.0, i.width, i.height).to_path(0.1),
        ObjectKind::Text(t) => {
            let mut all = BezPath::new();
            for line in layout_text(t).lines {
                for run in line.runs {
                    all.extend(run.outline);
                }
            }
            all
        }
        _ => return None,
    })
}

/// Caixa delimitadora em coordenadas da página (ou do pai, via `parent`).
/// `visual = true` inclui a espessura do contorno (GD-OBJ-06, GD-ALI-02).
pub fn bounds(o: &Object, parent: Affine, visual: bool) -> Option<Rect> {
    let m = parent * o.transform.affine();
    let r = match &o.kind {
        ObjectKind::Group(g) => g
            .children
            .iter()
            .filter_map(|c| bounds(c, m, visual))
            .reduce(|a, b| a.union(b))?,
        ObjectKind::Text(t) => {
            let mut path = local_outline(o)?;
            if path.elements().is_empty() {
                // Sem fonte: usa o quadro (ou um quadrado do corpo).
                let w = t.frame.map(|f| f[0]).unwrap_or(1000.0);
                let h = t.frame.map(|f| f[1]).unwrap_or(1000.0);
                path = Rect::new(0.0, -h, w, 0.0).to_path(0.1);
            }
            path.apply_affine(m);
            path.bounding_box()
        }
        _ => {
            let mut path = local_outline(o)?;
            path.apply_affine(m);
            path.bounding_box()
        }
    };
    if visual && let Some(s) = &o.stroke {
        let w = if s.scale_with_object {
            s.width * m.determinant().abs().sqrt()
        } else {
            s.width
        };
        let half = w / 2.0;
        return Some(r.inflate(half, half));
    }
    Some(r)
}

/// União das caixas de vários objetos.
pub fn union_bounds<'a>(objs: impl IntoIterator<Item = &'a Object>, visual: bool) -> Option<Rect> {
    objs.into_iter()
        .filter_map(|o| bounds(o, Affine::IDENTITY, visual))
        .reduce(|a, b| a.union(b))
}

/// O ponto `p` (página) atinge o objeto? Considera o preenchimento (se
/// houver) e a proximidade do contorno até `tol` (µm) além da meia espessura.
pub fn hits(o: &Object, parent: Affine, p: Point, tol: f64) -> bool {
    if !o.visible {
        return false;
    }
    let m = parent * o.transform.affine();
    match &o.kind {
        ObjectKind::Group(g) => g.children.iter().any(|c| hits(c, m, p, tol)),
        ObjectKind::Image(_) | ObjectKind::Text(_) => {
            bounds(o, parent, false).is_some_and(|b| b.inflate(tol, tol).contains(p))
        }
        ObjectKind::Unknown { .. } => false,
        _ => {
            let Some(local) = local_outline(o) else {
                return false;
            };
            let inv = m.inverse();
            let lp = inv * p;
            let scale = m.determinant().abs().sqrt().max(1e-12);
            let ltol = tol / scale;
            let filled = !matches!(o.fill, None | Some(Fill::None));
            let pd = match &o.kind {
                ObjectKind::Path(pd) => pd.clone(),
                _ => crate::model::PathData {
                    subpaths: super::convert::bez_to_subpaths(&local),
                    fill_rule: crate::model::FillRule::Nonzero,
                    extra: Default::default(),
                },
            };
            if filled && geometry::contains(&pd, lp) {
                return true;
            }
            let half = o.stroke.as_ref().map_or(0.0, |s| {
                if s.scale_with_object {
                    s.width / 2.0
                } else {
                    s.width / 2.0 / scale
                }
            });
            geometry::nearest(&pd, lp).is_some_and(|n| n.distance <= ltol + half)
        }
    }
}

/// Localização de um objeto de primeiro nível dentro da página.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hit {
    pub layer: usize,
    pub index: usize,
}

/// Objeto mais à frente sob o ponto, considerando só camadas visíveis e
/// destravadas e objetos destravados.
pub fn hit_test(page: &Page, p: Point, tol: f64) -> Option<Hit> {
    for (l, layer) in page.layers.iter().enumerate().rev() {
        if !layer.visible || layer.locked {
            continue;
        }
        for (i, o) in layer.objects.iter().enumerate().rev() {
            if !o.locked && hits(o, Affine::IDENTITY, p, tol) {
                return Some(Hit { layer: l, index: i });
            }
        }
    }
    None
}

/// Objetos inteiramente dentro do retângulo (seleção por área).
pub fn objects_in_rect(page: &Page, r: Rect) -> Vec<Hit> {
    let mut out = Vec::new();
    for (l, layer) in page.layers.iter().enumerate() {
        if !layer.visible || layer.locked {
            continue;
        }
        for (i, o) in layer.objects.iter().enumerate() {
            if o.visible
                && !o.locked
                && bounds(o, Affine::IDENTITY, false).is_some_and(|b| r.contains_rect(b))
            {
                out.push(Hit { layer: l, index: i });
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Color, EllipseData, GroupData, Layer, RectData, Stroke, Transform};
    use serde_json::Map;

    fn rect(x: f64, y: f64, w: f64, h: f64) -> Object {
        let mut o = Object::new(ObjectKind::Rectangle(RectData {
            width: w,
            height: h,
            corners: None,
            extra: Map::new(),
        }));
        o.transform = Transform::translate(x, y);
        o.fill = Some(Fill::solid(Color::black()));
        o
    }

    #[test]
    fn gd_obj_06_geometric_and_visual_bounds() {
        let mut r = rect(10.0, 20.0, 30.0, 40.0);
        assert_eq!(
            bounds(&r, Affine::IDENTITY, false),
            Some(Rect::new(10.0, 20.0, 40.0, 60.0))
        );
        r.stroke = Some(Stroke::new(4.0, Color::black()));
        assert_eq!(
            bounds(&r, Affine::IDENTITY, true),
            Some(Rect::new(8.0, 18.0, 42.0, 62.0))
        );
        // Girado 90°: caixa troca largura e altura.
        r.transform = Transform::from_affine(
            Affine::translate((100.0, 0.0)) * Affine::rotate(std::f64::consts::FRAC_PI_2),
        );
        let b = bounds(&r, Affine::IDENTITY, false).unwrap();
        assert!((b.width() - 40.0).abs() < 1e-9 && (b.height() - 30.0).abs() < 1e-9);
        // Grupo = união dos filhos.
        let g = Object::new(ObjectKind::Group(GroupData {
            children: vec![rect(0.0, 0.0, 1.0, 1.0), rect(5.0, 5.0, 1.0, 1.0)],
            extra: Map::new(),
        }));
        assert_eq!(
            bounds(&g, Affine::IDENTITY, false),
            Some(Rect::new(0.0, 0.0, 6.0, 6.0))
        );
    }

    #[test]
    fn hit_testing_fill_stroke_and_z_order() {
        let mut page = Page::new(100.0, 100.0);
        let mut outline = Object::new(ObjectKind::Ellipse(EllipseData {
            rx: 10.0,
            ry: 10.0,
            mode: "ellipse".into(),
            start_angle: 0.0,
            end_angle: 360.0,
            extra: Map::new(),
        }));
        outline.transform = Transform::translate(50.0, 50.0);
        outline.stroke = Some(Stroke::new(2.0, Color::black()));
        page.layers[0].objects.push(rect(0.0, 0.0, 30.0, 30.0));
        page.layers[0].objects.push(outline);
        page.layers[0].objects.push(rect(20.0, 20.0, 30.0, 30.0));
        assert_eq!(
            hit_test(&page, Point::new(25.0, 25.0), 0.5),
            Some(Hit { layer: 0, index: 2 }),
            "o da frente vence"
        );
        assert_eq!(
            hit_test(&page, Point::new(5.0, 5.0), 0.5),
            Some(Hit { layer: 0, index: 0 })
        );
        assert_eq!(
            hit_test(&page, Point::new(60.5, 50.0), 0.5),
            Some(Hit { layer: 0, index: 1 }),
            "borda do contorno"
        );
        assert_eq!(
            hit_test(&page, Point::new(55.0, 52.0), 0.5),
            None,
            "elipse sem preenchimento: miolo vazio"
        );
        page.layers[0].objects[2].locked = true;
        assert_eq!(
            hit_test(&page, Point::new(45.0, 45.0), 0.5),
            None,
            "travado não é selecionável"
        );
        let mut hidden = Layer::new("oculta");
        hidden.visible = false;
        hidden.objects.push(rect(0.0, 0.0, 100.0, 100.0));
        page.layers.push(hidden);
        assert_eq!(
            hit_test(&page, Point::new(5.0, 5.0), 0.5),
            Some(Hit { layer: 0, index: 0 })
        );
    }

    #[test]
    fn rectangle_selection_requires_full_containment() {
        let mut page = Page::new(100.0, 100.0);
        page.layers[0].objects.push(rect(0.0, 0.0, 10.0, 10.0));
        page.layers[0].objects.push(rect(20.0, 20.0, 10.0, 10.0));
        assert_eq!(
            objects_in_rect(&page, Rect::new(-1.0, -1.0, 15.0, 25.0)),
            vec![Hit { layer: 0, index: 0 }]
        );
        assert_eq!(
            objects_in_rect(&page, Rect::new(-1.0, -1.0, 31.0, 31.0)).len(),
            2
        );
    }
}
