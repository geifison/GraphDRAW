//! Preenchimento inteligente (GD-FER-52): o clique numa área delimitada por
//! objetos sobrepostos cria um objeto novo com a forma daquela área.
//!
//! A área é a face da sobreposição que contém o ponto: começa pela caixa de
//! tudo, fica com a interseção das áreas que contêm o ponto, tira as que
//! não contêm e tira as linhas (contornos abertos e contornos) como faixas
//! da espessura deles. Da sobra, vale a peça que contém o ponto.

use i_overlay::core::overlay_rule::OverlayRule;
use i_overlay::mesh::float::stroke::offset::StrokeOffset;
use i_overlay::mesh::float::style::{LineCap, LineJoin, StrokeStyle};
use kurbo::{Affine, PathEl, Point};

use super::history::Editor;
use super::query::{bounds, local_outline};
use super::shapeops::{Shapes, area, overlay, path_object, region, shapes_to_path, union_all};
use crate::model::{Fill, FillRule, Object, ObjectKind, Stroke};

/// Faixa da linha (contorno) do objeto, na página.
fn line_band(o: &Object, parent: Affine) -> Shapes {
    let Some(s) = &o.stroke else {
        return Vec::new();
    };
    let Some(mut path) = local_outline(o) else {
        return Vec::new();
    };
    let m = parent * o.transform.affine();
    path.apply_affine(m);
    let width = if s.scale_with_object {
        s.width * m.determinant().abs().sqrt()
    } else {
        s.width
    }
    .max(2.0);
    let style = StrokeStyle::new(width)
        .line_join(LineJoin::Round(0.2))
        .start_cap(LineCap::Round(0.2))
        .end_cap(LineCap::Round(0.2));
    let mut out = Vec::new();
    let mut cur: Vec<[f64; 2]> = Vec::new();
    let mut closed = false;
    let flush = |cur: &mut Vec<[f64; 2]>, closed: bool, out: &mut Vec<Shapes>| {
        if cur.len() >= 2 {
            out.push(cur.stroke(style.clone(), closed));
        }
        cur.clear();
    };
    kurbo::flatten(&path, 1.0, |el| match el {
        PathEl::MoveTo(p) => {
            flush(&mut cur, closed, &mut out);
            closed = false;
            cur.push([p.x, p.y]);
        }
        PathEl::LineTo(p) => cur.push([p.x, p.y]),
        PathEl::ClosePath => {
            closed = true;
            flush(&mut cur, true, &mut out);
        }
        _ => {}
    });
    flush(&mut cur, closed, &mut out);
    union_all(out)
}

fn contains(shapes: &Shapes, p: Point) -> bool {
    let mut inside = false;
    for c in shapes.iter().flatten() {
        let n = c.len();
        for i in 0..n {
            let (a, b) = (c[i], c[(i + 1) % n]);
            if (a[1] > p.y) != (b[1] > p.y)
                && p.x < a[0] + (p.y - a[1]) / (b[1] - a[1]) * (b[0] - a[0])
            {
                inside = !inside;
            }
        }
    }
    inside
}

/// Área sob `p` (página) delimitada pelos objetos da página, ou `None` se
/// o ponto não está cercado.
pub fn area_at(ed: &Editor, page: usize, p: Point) -> Option<Shapes> {
    let pg = ed.doc.data.pages.get(page)?;
    // Objetos de primeiro nível (grupos entram pela área deles).
    let mut tops: Vec<(&Object, Affine)> = Vec::new();
    for layer in pg.layers.iter().filter(|l| l.visible && !l.guides) {
        for o in &layer.objects {
            if o.visible && !matches!(o.kind, ObjectKind::Image(_) | ObjectKind::Text(_)) {
                tops.push((o, Affine::IDENTITY));
            }
        }
    }
    let all = tops
        .iter()
        .filter_map(|(o, m)| bounds(o, *m, true))
        .reduce(|a, b| a.union(b))?
        .inflate(1_000.0, 1_000.0);
    if !all.contains(p) {
        return None;
    }
    let mut face: Shapes = union_all([vec![vec![vec![
        [all.x0, all.y0],
        [all.x1, all.y0],
        [all.x1, all.y1],
        [all.x0, all.y1],
    ]]]]);
    let mut enclosed = false;
    for (o, m) in &tops {
        let filled = !matches!(o.fill, None | Some(Fill::None));
        let reg = region(o, *m);
        if filled && !reg.is_empty() {
            if contains(&reg, p) {
                face = overlay(&face, &reg, OverlayRule::Intersect);
                enclosed = true;
            } else {
                face = overlay(&face, &reg, OverlayRule::Difference);
            }
        }
        // Contornos (linhas abertas e bordas) cortam a área.
        let band = line_band(o, *m);
        if !band.is_empty() {
            face = overlay(&face, &band, OverlayRule::Difference);
        }
    }
    // A peça que contém o ponto.
    let piece: Shapes = face
        .into_iter()
        .filter(|s| contains(&vec![s.clone()], p))
        .collect();
    if piece.is_empty() {
        return None;
    }
    // Sem nenhuma área contendo o ponto, só vale se as linhas o cercam (a
    // peça não encosta na caixa de tudo).
    if !enclosed {
        let touches = piece.iter().flatten().flatten().any(|q| {
            (q[0] - all.x0).abs() < 1.0
                || (q[0] - all.x1).abs() < 1.0
                || (q[1] - all.y0).abs() < 1.0
                || (q[1] - all.y1).abs() < 1.0
        });
        if touches {
            return None;
        }
    }
    (area(&piece) > 0.0).then_some(piece)
}

/// Cria o objeto da área sob `p` com o preenchimento `fill` (e o contorno
/// `stroke`, se houver), acima de tudo na camada `layer`. Devolve o ID.
pub fn smart_fill(
    ed: &mut Editor,
    page: usize,
    layer: usize,
    p: Point,
    fill: Fill,
    stroke: Option<Stroke>,
) -> Option<String> {
    let piece = area_at(ed, page, p)?;
    let like = {
        let mut o = Object::new(ObjectKind::Path(crate::model::PathData {
            subpaths: Vec::new(),
            fill_rule: FillRule::Nonzero,
            extra: Default::default(),
        }));
        o.fill = Some(fill);
        o.stroke = stroke;
        o
    };
    let o = path_object(&shapes_to_path(&piece), &like, FillRule::Nonzero);
    Some(super::ops::add_object(
        ed,
        page,
        layer,
        o,
        "Preenchimento inteligente",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Color, Document, Node, Page, PathData, RectData, Subpath, Transform};

    fn setup(objs: Vec<Object>) -> Editor {
        let mut d = Document::new();
        d.data.pages.push(Page::new(300_000.0, 300_000.0));
        let mut ed = Editor::new(d);
        for o in objs {
            super::super::ops::add_object(&mut ed, 0, 0, o, "o");
        }
        ed
    }

    fn square(x: f64, y: f64, s: f64) -> Object {
        let mut o = Object::new(ObjectKind::Rectangle(RectData {
            width: s,
            height: s,
            corners: None,
            extra: Default::default(),
        }));
        o.transform = Transform::translate(x, y);
        o.fill = Some(Fill::solid(Color::black()));
        o
    }

    /// GD-FER-52: dois quadrados sobrepostos — o clique na sobreposição dá
    /// só a sobreposição; numa parte só de um, a parte dele sem o outro.
    #[test]
    fn gd_fer_52_overlap_and_part() {
        let mut ed = setup(vec![
            square(0.0, 0.0, 100_000.0),
            square(50_000.0, 50_000.0, 100_000.0),
        ]);
        let a = area_at(&ed, 0, Point::new(75_000.0, 75_000.0)).unwrap();
        assert!((area(&a) - 2.5e9).abs() < 2e7, "50×50 mm: {}", area(&a));
        let b = area_at(&ed, 0, Point::new(25_000.0, 25_000.0)).unwrap();
        assert!((area(&b) - 7.5e9).abs() < 3e7, "L: {}", area(&b));
        assert!(
            area_at(&ed, 0, Point::new(250_000.0, 250_000.0)).is_none(),
            "fora de tudo"
        );
        let id = smart_fill(
            &mut ed,
            0,
            0,
            Point::new(75_000.0, 75_000.0),
            Fill::solid(Color::rgb(1.0, 0.0, 0.0)),
            None,
        )
        .unwrap();
        assert_eq!(
            ed.doc.data.pages[0].layers[0].objects.last().unwrap().id,
            id
        );
    }

    /// Linhas cruzadas sem preenchimento cercam a área (triângulo de três
    /// retas).
    #[test]
    fn gd_fer_52_lines_enclose() {
        let line = |a: (f64, f64), b: (f64, f64)| {
            let mut o = Object::new(ObjectKind::Path(PathData {
                subpaths: vec![Subpath {
                    nodes: vec![Node::corner(a.0, a.1), Node::corner(b.0, b.1)],
                    closed: false,
                    extra: Default::default(),
                }],
                fill_rule: FillRule::Nonzero,
                extra: Default::default(),
            }));
            o.stroke = Some(Stroke::new(500.0, Color::black()));
            o
        };
        let ed = setup(vec![
            line((-10_000.0, 0.0), (210_000.0, 0.0)),
            line((0.0, -10_000.0), (110_000.0, 200_000.0)),
            line((200_000.0, -10_000.0), (90_000.0, 200_000.0)),
        ]);
        let t = area_at(&ed, 0, Point::new(100_000.0, 50_000.0)).unwrap();
        assert!(area(&t) > 5e9 && area(&t) < 2e10, "{}", area(&t));
        assert!(
            area_at(&ed, 0, Point::new(5_000.0, 150_000.0)).is_none(),
            "aberto para fora"
        );
    }
}
