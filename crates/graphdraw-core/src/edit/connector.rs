//! Conectores (GD-FER-41, GD-CON-01): linha reta, em ângulo reto (com cantos
//! arredondados opcionais) ou curva entre dois pontos, cada um preso a um
//! ponto de encaixe de um objeto (cantos, meios dos lados, centro) ou
//! livre. A rota é refeita quando os objetos se movem: a cena resolve ao
//! vivo e o editor grava as pontas novas no mesmo passo ([`refresh`]).

use kurbo::{Affine, BezPath, Point, Rect, Vec2};

use super::history::{Editor, ObjLoc};
use super::query::bounds;
use crate::model::{Color, ConnEnd, ConnectorData, Document, Object, ObjectKind, Stroke};

/// Pontos de encaixe (relativos à caixa: 0–1) — cantos, meios dos lados e
/// centro.
pub const ANCHORS: [[f64; 2]; 9] = [
    [0.0, 0.0],
    [0.5, 0.0],
    [1.0, 0.0],
    [0.0, 0.5],
    [0.5, 0.5],
    [1.0, 0.5],
    [0.0, 1.0],
    [0.5, 1.0],
    [1.0, 1.0],
];

fn at(b: Rect, a: [f64; 2]) -> Point {
    Point::new(b.x0 + a[0] * b.width(), b.y0 + a[1] * b.height())
}

/// Ponta resolvida (página): o encaixe no objeto ligado, ou o ponto guardado
/// (espaço do conector, levado à página por `m`).
pub fn resolve_end(
    e: &ConnEnd,
    m: Affine,
    find: &dyn Fn(&str) -> Option<(Object, Affine)>,
) -> Point {
    if let (Some(id), Some(a)) = (&e.object, e.anchor)
        && let Some((o, parent)) = find(id)
        && let Some(b) = bounds(&o, parent, false)
    {
        return at(b, a);
    }
    m * Point::new(e.point[0], e.point[1])
}

/// Direção de saída da ponta: lados esquerdo/direito saem na horizontal,
/// baixo/cima na vertical; centro e ponto livre, para o outro lado.
fn exit(e: &ConnEnd, from: Point, to: Point) -> Vec2 {
    match e.anchor {
        Some([x, _]) if x <= 0.0 => Vec2::new(-1.0, 0.0),
        Some([x, _]) if x >= 1.0 => Vec2::new(1.0, 0.0),
        Some([_, y]) if y <= 0.0 => Vec2::new(0.0, -1.0),
        Some([_, y]) if y >= 1.0 => Vec2::new(0.0, 1.0),
        _ => {
            let d = to - from;
            if d.x.abs() >= d.y.abs() {
                Vec2::new(d.x.signum(), 0.0)
            } else {
                Vec2::new(0.0, d.y.signum())
            }
        }
    }
}

/// Caminho do conector (página).
pub fn route(c: &ConnectorData, a: Point, b: Point) -> BezPath {
    let mut p = BezPath::new();
    p.move_to(a);
    let (da, db) = (exit(&c.from, a, b), exit(&c.to, b, a));
    match c.style.as_str() {
        "elbow" => {
            // Sai na direção de `a` e chega na de `b`: as duas na mesma
            // orientação quebram no meio; diferentes, num canto só.
            let pts = match (da.x != 0.0, db.x != 0.0) {
                (true, true) => {
                    let mx = (a.x + b.x) / 2.0;
                    vec![Point::new(mx, a.y), Point::new(mx, b.y), b]
                }
                (false, false) => {
                    let my = (a.y + b.y) / 2.0;
                    vec![Point::new(a.x, my), Point::new(b.x, my), b]
                }
                (true, false) => vec![Point::new(b.x, a.y), b],
                (false, true) => vec![Point::new(a.x, b.y), b],
            };
            let r = c.radius.max(0.0);
            if r <= 0.0 {
                for q in pts {
                    p.line_to(q);
                }
            } else {
                // Cantos arredondados: corta cada canto em `r` (no máximo
                // metade dos trechos) e liga por quadrática.
                let mut prev = a;
                for i in 0..pts.len() - 1 {
                    let (corner, next) = (pts[i], pts[i + 1]);
                    let (u, v) = (corner - prev, next - corner);
                    let rr = r.min(u.hypot() / 2.0).min(v.hypot() / 2.0);
                    if rr <= 0.0 || u.hypot() == 0.0 || v.hypot() == 0.0 {
                        p.line_to(corner);
                    } else {
                        p.line_to(corner - u.normalize() * rr);
                        p.quad_to(corner, corner + v.normalize() * rr);
                    }
                    prev = corner;
                }
                p.line_to(b);
            }
        }
        "curved" => {
            let k = (b - a).hypot() * 0.4;
            p.curve_to(a + da * k, b + db * k, b);
        }
        _ => p.line_to(b),
    }
    p
}

/// Pontas atuais do conector (página) seguindo as ligações; `m` = matriz do
/// conector.
pub fn ends(
    c: &ConnectorData,
    m: Affine,
    find: &dyn Fn(&str) -> Option<(Object, Affine)>,
) -> (Point, Point) {
    (resolve_end(&c.from, m, find), resolve_end(&c.to, m, find))
}

/// Encaixe mais próximo de `p` (página) dentro de `tol` em algum objeto da
/// página (não conectores): objeto, encaixe relativo e o ponto.
pub fn anchor_near(
    doc: &Document,
    page: usize,
    p: Point,
    tol: f64,
) -> Option<(String, [f64; 2], Point)> {
    let pg = doc.data.pages.get(page)?;
    let mut best: Option<(f64, String, [f64; 2], Point)> = None;
    for layer in pg.layers.iter().filter(|l| l.visible) {
        for o in &layer.objects {
            if matches!(o.kind, ObjectKind::Connector(_) | ObjectKind::Dimension(_)) {
                continue;
            }
            let Some(b) = bounds(o, Affine::IDENTITY, false) else {
                continue;
            };
            for a in ANCHORS {
                let q = at(b, a);
                let d = (q - p).hypot();
                if d <= tol && best.as_ref().is_none_or(|x| d < x.0) {
                    best = Some((d, o.id.clone(), a, q));
                }
            }
        }
    }
    best.map(|(_, id, a, q)| (id, a, q))
}

/// Conector novo (página) com o contorno padrão.
pub fn new_connector(style: &str, from: ConnEnd, to: ConnEnd) -> Object {
    let mut o = Object::new(ObjectKind::Connector(ConnectorData {
        style: style.into(),
        from,
        to,
        radius: 0.0,
        extra: Default::default(),
    }));
    o.stroke = Some(Stroke::new(500.0, Color::black()));
    o
}

/// Grava as pontas novas dos conectores ligados (chamado pelo editor ao
/// fechar a transação, no mesmo passo).
pub fn refresh(ed: &mut Editor) {
    let mut updates: Vec<(ObjLoc, Object)> = Vec::new();
    for (pi, page) in ed.doc.data.pages.iter().enumerate() {
        for (li, layer) in page.layers.iter().enumerate() {
            for (oi, o) in layer.objects.iter().enumerate() {
                let ObjectKind::Connector(c) = &o.kind else {
                    continue;
                };
                if c.from.object.is_none() && c.to.object.is_none() {
                    continue;
                }
                let doc = &ed.doc;
                let m = o.transform.affine();
                let (a, b) = ends(c, m, &|id| super::dimension::find_in_doc(doc, id));
                let (a, b) = (m.inverse() * a, m.inverse() * b);
                let moved = |e: &ConnEnd, q: Point| {
                    (e.point[0] - q.x).abs() > 1e-6 || (e.point[1] - q.y).abs() > 1e-6
                };
                if moved(&c.from, a) || moved(&c.to, b) {
                    let mut n = o.clone();
                    if let ObjectKind::Connector(nc) = &mut n.kind {
                        nc.from.point = [a.x, a.y];
                        nc.to.point = [b.x, b.y];
                    }
                    updates.push((
                        ObjLoc {
                            page: pi,
                            layer: li,
                            index: oi,
                        },
                        n,
                    ));
                }
            }
        }
    }
    for (loc, n) in updates {
        ed.replace_object(loc, n);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Page, RectData, Transform};
    use kurbo::Shape;

    fn square(x: f64, y: f64) -> Object {
        let mut o = Object::new(ObjectKind::Rectangle(RectData {
            width: 20_000.0,
            height: 20_000.0,
            corners: None,
            extra: Default::default(),
        }));
        o.transform = Transform::translate(x, y);
        o
    }

    fn linked(id: &str, a: [f64; 2], q: Point) -> ConnEnd {
        ConnEnd {
            point: [q.x, q.y],
            object: Some(id.into()),
            anchor: Some(a),
        }
    }

    /// GD-CON-01: preso no meio do lado direito de A e no esquerdo de B; mover
    /// B refaz a rota no mesmo passo de desfazer.
    #[test]
    fn gd_con_01_follows_objects() {
        let mut d = Document::new();
        d.data.pages.push(Page::new(300_000.0, 300_000.0));
        let mut ed = Editor::new(d);
        let a = super::super::ops::add_object(&mut ed, 0, 0, square(0.0, 0.0), "a");
        let b = super::super::ops::add_object(&mut ed, 0, 0, square(100_000.0, 50_000.0), "b");
        let (ia, aa, qa) =
            anchor_near(&ed.doc, 0, Point::new(19_000.0, 10_500.0), 2_000.0).unwrap();
        assert_eq!((ia.as_str(), aa), (a.as_str(), [1.0, 0.5]));
        let (ib, ab, qb) =
            anchor_near(&ed.doc, 0, Point::new(100_500.0, 60_000.0), 2_000.0).unwrap();
        assert_eq!(ib, b);
        let mut c = new_connector("elbow", linked(&ia, aa, qa), linked(&ib, ab, qb));
        let cid = c.id.clone();
        if let ObjectKind::Connector(cd) = &mut c.kind {
            cd.radius = 2_000.0;
        }
        super::super::ops::add_object(&mut ed, 0, 0, c, "c");
        let undo = ed.undo_len();
        super::super::ops::transform(
            &mut ed,
            std::slice::from_ref(&b),
            Affine::translate((0.0, 40_000.0)),
            "Mover",
        );
        assert_eq!(ed.undo_len(), undo + 1);
        let get = |ed: &Editor| match &ed.get(&ed.find(&cid).unwrap()).unwrap().kind {
            ObjectKind::Connector(c) => c.clone(),
            _ => unreachable!(),
        };
        assert_eq!(get(&ed).to.point, [100_000.0, 100_000.0]);
        let r = route(
            &get(&ed),
            Point::new(20_000.0, 10_000.0),
            Point::new(100_000.0, 100_000.0),
        );
        let bb = r.bounding_box();
        assert!((bb.width() - 80_000.0).abs() < 1.0 && (bb.height() - 90_000.0).abs() < 1.0);
        ed.undo();
        assert_eq!(get(&ed).to.point, [100_000.0, 60_000.0]);
        // Ida e volta no JSON.
        let o = ed.get(&ed.find(&cid).unwrap()).unwrap().clone();
        assert_eq!(Object::from_value(o.to_value()), o);
    }

    /// Reto e curvo ligam as pontas; o curvo sai na direção do encaixe.
    #[test]
    fn straight_and_curved_routes() {
        let free = |x: f64, y: f64| ConnEnd {
            point: [x, y],
            object: None,
            anchor: None,
        };
        let s = new_connector("straight", free(0.0, 0.0), free(100.0, 50.0));
        let ObjectKind::Connector(sc) = &s.kind else {
            panic!()
        };
        assert_eq!(
            route(sc, Point::ORIGIN, Point::new(100.0, 50.0))
                .elements()
                .len(),
            2
        );
        let mut c = sc.clone();
        c.style = "curved".into();
        c.from.anchor = Some([1.0, 0.5]);
        let r = route(&c, Point::ORIGIN, Point::new(100.0, 50.0));
        let kurbo::PathEl::CurveTo(c1, _, _) = r.elements()[1] else {
            panic!()
        };
        assert!(c1.y.abs() < 1e-9 && c1.x > 0.0, "sai para a direita");
        // Ângulo reto saindo na horizontal e chegando por baixo: um canto só.
        c.style = "elbow".into();
        c.to.anchor = Some([0.5, 0.0]);
        let r = route(&c, Point::ORIGIN, Point::new(100.0, 50.0));
        assert_eq!(r.elements().len(), 3);
        assert_eq!(
            r.elements()[1],
            kurbo::PathEl::LineTo(Point::new(100.0, 0.0))
        );
    }
}
