//! Consultas geométricas sobre objetos: caixas delimitadoras (GD-OBJ-06),
//! acerto de clique e seleção por retângulo (GD-SEL-01, GD-SEL-03).

use kurbo::{Affine, BezPath, Point, Rect, Shape};

use crate::geometry;
use crate::model::geom::subpaths_to_bez;
use crate::model::{Fill, Object, ObjectKind, Page};
use crate::render::scene::{all_closed, ellipse_path, rect_path};
use crate::render::text::layout_text;

/// Contorno local de objetos com geometria de caminho.
pub fn local_outline(o: &Object) -> Option<BezPath> {
    Some(match &o.kind {
        ObjectKind::Path(p) => subpaths_to_bez(&p.subpaths),
        ObjectKind::Rectangle(r) => rect_path(r),
        ObjectKind::Ellipse(e) => ellipse_path(e),
        ObjectKind::Polygon(p) => crate::render::scene::polygon_path(p),
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
        ObjectKind::Dimension(d) => super::dimension::outline(d),
        ObjectKind::Basic(b) => crate::render::basic::path(b),
        ObjectKind::BSpline(b) => crate::render::bspline::path(b),
        ObjectKind::Connector(c) => super::connector::route(
            c,
            Point::new(c.from.point[0], c.from.point[1]),
            Point::new(c.to.point[0], c.to.point[1]),
        ),
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
        // Contorno interno não sai da borda; externo sai a espessura inteira
        // (GD-OUT-04). Caminhos abertos sempre desenham centrado.
        let closed = local_outline(o).is_some_and(|p| all_closed(&p));
        let out = match (s.position.as_str(), closed) {
            ("inside", true) => 0.0,
            ("outside", true) => w,
            _ => w / 2.0,
        };
        return Some(r.inflate(out, out));
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
        ObjectKind::Image(_) | ObjectKind::Text(_) | ObjectKind::Dimension(_) => {
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

/// Objeto bloqueado mais à frente sob o ponto (também dentro de grupos),
/// se nada destravado o cobre: só o botão direito chega nele, para
/// desbloquear (GD-BDR-45). Camadas ocultas ou travadas ficam de fora.
pub fn hit_test_locked(page: &Page, p: Point, tol: f64) -> Option<String> {
    fn find(o: &Object, parent: Affine, p: Point, tol: f64) -> Option<Option<String>> {
        if !o.visible || !hits(o, parent, p, tol) {
            return None;
        }
        if o.locked {
            return Some(Some(o.id.clone()));
        }
        if let ObjectKind::Group(g) = &o.kind {
            let m = parent * o.transform.affine();
            return g
                .children
                .iter()
                .rev()
                .find_map(|c| find(c, m, p, tol))
                .or(Some(None));
        }
        // Destravado por cima: o esquerdo já pega ele.
        Some(None)
    }
    for layer in page.layers.iter().rev() {
        if !layer.visible || layer.locked {
            continue;
        }
        if let Some(r) = layer
            .objects
            .iter()
            .rev()
            .find_map(|o| find(o, Affine::IDENTITY, p, tol))
        {
            return r;
        }
    }
    None
}

/// Objeto (não grupo) mais à frente sob o ponto, descendo nos grupos
/// (GD-SEL-02). Devolve o ID.
pub fn hit_test_deep(page: &Page, p: Point, tol: f64) -> Option<String> {
    fn deepest(o: &Object, parent: Affine, p: Point, tol: f64) -> Option<&Object> {
        if !o.visible || o.locked {
            return None;
        }
        match &o.kind {
            ObjectKind::Group(g) => {
                let m = parent * o.transform.affine();
                g.children.iter().rev().find_map(|c| deepest(c, m, p, tol))
            }
            _ => hits(o, parent, p, tol).then_some(o),
        }
    }
    let hit = hit_test(page, p, tol)?;
    deepest(
        &page.layers[hit.layer].objects[hit.index],
        Affine::IDENTITY,
        p,
        tol,
    )
    .map(|o| o.id.clone())
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

/// Objetos de primeiro nível que podem ser selecionados: visíveis, sem
/// trava, em camadas visíveis e destravadas.
pub fn selectable(page: &Page) -> Vec<Hit> {
    let mut out = Vec::new();
    for (l, layer) in page.layers.iter().enumerate() {
        if !layer.visible || layer.locked {
            continue;
        }
        for (i, o) in layer.objects.iter().enumerate() {
            if o.visible && !o.locked {
                out.push(Hit { layer: l, index: i });
            }
        }
    }
    out
}

/// Pontos do contorno em coordenadas da página, para testar se o objeto cabe
/// numa região. Sem contorno, usa os cantos da caixa.
fn outline_points(o: &Object, parent: Affine, out: &mut Vec<Point>) {
    let m = parent * o.transform.affine();
    if let ObjectKind::Group(g) = &o.kind {
        for c in &g.children {
            outline_points(c, m, out);
        }
        return;
    }
    match local_outline(o).filter(|p| !p.elements().is_empty()) {
        Some(mut path) => {
            path.apply_affine(m);
            let tol = (path.bounding_box().size().max_side() / 200.0).max(1e-3);
            kurbo::flatten(path, tol, |el| match el {
                kurbo::PathEl::MoveTo(q) | kurbo::PathEl::LineTo(q) => out.push(q),
                _ => {}
            });
        }
        None => {
            if let Some(b) = bounds(o, parent, false) {
                out.extend([
                    Point::new(b.x0, b.y0),
                    Point::new(b.x1, b.y0),
                    Point::new(b.x1, b.y1),
                    Point::new(b.x0, b.y1),
                ]);
            }
        }
    }
}

/// Seleção por laço (GD-SEL-03): objetos cujo contorno inteiro fica dentro
/// do polígono desenhado (`lasso`, em coordenadas da página).
pub fn objects_in_lasso(page: &Page, lasso: &[Point]) -> Vec<Hit> {
    if lasso.len() < 3 {
        return Vec::new();
    }
    let mut poly = BezPath::new();
    poly.move_to(lasso[0]);
    for q in &lasso[1..] {
        poly.line_to(*q);
    }
    poly.close_path();
    let area = poly.bounding_box();
    selectable(page)
        .into_iter()
        .filter(|h| {
            let o = &page.layers[h.layer].objects[h.index];
            if !bounds(o, Affine::IDENTITY, false).is_some_and(|b| area.contains_rect(b)) {
                return false;
            }
            let mut pts = Vec::new();
            outline_points(o, Affine::IDENTITY, &mut pts);
            !pts.is_empty() && pts.iter().all(|q| poly.contains(*q))
        })
        .collect()
}

/// Critério de "selecionar semelhantes" (GD-SEL-03).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Similar {
    /// Mesmo tipo de objeto (retângulo, elipse, curva, texto…).
    Kind,
    /// Mesmo preenchimento.
    Fill,
    /// Mesma cor de contorno (objetos sem contorno combinam entre si).
    StrokeColor,
}

/// Objetos selecionáveis semelhantes a `reference` pelo critério dado.
pub fn similar(page: &Page, reference: &Object, by: Similar) -> Vec<Hit> {
    let stroke_color = |o: &Object| o.stroke.as_ref().map(|s| s.color.clone());
    let fill = |o: &Object| o.fill.clone().filter(|f| *f != Fill::None);
    selectable(page)
        .into_iter()
        .filter(|h| {
            let o = &page.layers[h.layer].objects[h.index];
            match by {
                Similar::Kind => {
                    std::mem::discriminant(&o.kind) == std::mem::discriminant(&reference.kind)
                }
                Similar::Fill => fill(o) == fill(reference),
                Similar::StrokeColor => stroke_color(o) == stroke_color(reference),
            }
        })
        .collect()
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

    /// Bloqueado: hit_test não acha; hit_test_locked acha (também dentro de
    /// grupo), mas não por baixo de um destravado.
    #[test]
    fn gd_bdr_45_locked_only_by_right_click() {
        let mut page = crate::model::Page::new(100.0, 100.0);
        let mut a = rect(0.0, 0.0, 10.0, 10.0);
        a.locked = true;
        let mut inner = rect(20.0, 0.0, 10.0, 10.0);
        inner.locked = true;
        let g = Object::new(ObjectKind::Group(GroupData {
            children: vec![inner.clone(), rect(40.0, 0.0, 10.0, 10.0)],
            extra: Map::new(),
        }));
        let cover = rect(60.0, 0.0, 10.0, 10.0);
        let mut under = rect(60.0, 0.0, 10.0, 10.0);
        under.locked = true;
        page.layers[0].objects = vec![a.clone(), g, under, cover];
        let at = |x| Point::new(x, 5.0);
        assert!(
            hit_test(&page, at(5.0), 0.1).is_none(),
            "esquerdo passa reto"
        );
        assert_eq!(hit_test_locked(&page, at(5.0), 0.1), Some(a.id.clone()));
        assert_eq!(
            hit_test_locked(&page, at(25.0), 0.1),
            Some(inner.id.clone())
        );
        assert_eq!(hit_test_locked(&page, at(45.0), 0.1), None);
        assert_eq!(hit_test_locked(&page, at(65.0), 0.1), None, "coberto");
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
    fn gd_sel_02_deep_hit_finds_child() {
        let mut page = Page::new(100.0, 100.0);
        let a = rect(0.0, 0.0, 10.0, 10.0);
        let b = rect(20.0, 0.0, 10.0, 10.0);
        let b_id = b.id.clone();
        let mut g = Object::new(ObjectKind::Group(GroupData {
            children: vec![a, b],
            extra: Map::new(),
        }));
        g.transform = Transform::translate(50.0, 50.0);
        let g_id = g.id.clone();
        page.layers[0].objects.push(g);
        let p = Point::new(75.0, 55.0);
        let hit = hit_test(&page, p, 0.5).unwrap();
        assert_eq!(page.layers[0].objects[hit.index].id, g_id);
        assert_eq!(hit_test_deep(&page, p, 0.5), Some(b_id));
        assert_eq!(hit_test_deep(&page, Point::new(65.0, 55.0), 0.5), None);
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

    #[test]
    fn gd_sel_03_lasso_needs_whole_outline_inside() {
        let mut page = Page::new(100.0, 100.0);
        page.layers[0].objects.push(rect(10.0, 10.0, 10.0, 10.0));
        page.layers[0].objects.push(rect(60.0, 10.0, 10.0, 10.0));
        // Triângulo que cobre só o primeiro retângulo.
        let tri = [
            Point::new(0.0, 0.0),
            Point::new(50.0, 0.0),
            Point::new(0.0, 50.0),
        ];
        assert_eq!(
            objects_in_lasso(&page, &tri),
            vec![Hit { layer: 0, index: 0 }]
        );
        // A caixa cabe no triângulo menor, mas um canto fica fora: não entra.
        let small = [
            Point::new(5.0, 5.0),
            Point::new(30.0, 5.0),
            Point::new(5.0, 30.0),
        ];
        assert!(objects_in_lasso(&page, &small).is_empty());
    }

    #[test]
    fn gd_sel_03_similar_by_kind_fill_and_stroke() {
        let mut page = Page::new(100.0, 100.0);
        let a = rect(0.0, 0.0, 10.0, 10.0);
        let mut b = rect(20.0, 0.0, 10.0, 10.0);
        b.fill = Some(Fill::solid(Color::rgb(1.0, 0.0, 0.0)));
        b.stroke = Some(Stroke::new(1.0, Color::black()));
        let mut e = Object::new(ObjectKind::Ellipse(EllipseData {
            rx: 5.0,
            ry: 5.0,
            mode: "ellipse".into(),
            start_angle: 0.0,
            end_angle: 360.0,
            extra: Map::new(),
        }));
        e.fill = Some(Fill::solid(Color::black()));
        e.stroke = Some(Stroke::new(2.0, Color::black()));
        page.layers[0].objects.extend([a.clone(), b, e]);
        let idx = |v: Vec<Hit>| v.into_iter().map(|h| h.index).collect::<Vec<_>>();
        assert_eq!(idx(similar(&page, &a, Similar::Kind)), vec![0, 1]);
        assert_eq!(idx(similar(&page, &a, Similar::Fill)), vec![0, 2]);
        let b = page.layers[0].objects[1].clone();
        assert_eq!(idx(similar(&page, &b, Similar::StrokeColor)), vec![1, 2]);
        assert_eq!(idx(similar(&page, &a, Similar::StrokeColor)), vec![0]);
    }
}
