//! Texto contornando objeto (GD-TXT-10): objetos com `wrap` afastam o
//! texto de parágrafo da mesma página. As áreas ficam no próprio texto
//! (`exclusions`, não gravadas) e são refeitas ao fechar a transação.
//!
//! Limitação: só textos e objetos de primeiro nível nas camadas.

use kurbo::{Affine, PathEl};

use super::history::{Editor, ObjLoc};
use super::query::{bounds, local_outline};
use crate::model::{Document, Object, ObjectKind, Page, WrapZone};

/// Áreas para o texto `t` (de primeiro nível) vindas dos objetos da página.
fn zones_for(page: &Page, text: &Object) -> Vec<WrapZone> {
    let inv = text.transform.affine().inverse();
    let mut out = Vec::new();
    for layer in page.layers.iter().filter(|l| l.visible) {
        for o in layer
            .objects
            .iter()
            .filter(|o| o.visible && o.id != text.id)
        {
            let Some(w) = &o.wrap else {
                continue;
            };
            let polys = if w.style == "square" {
                bounds(o, Affine::IDENTITY, true)
                    .map(|b| {
                        vec![[b.x0, b.y0], [b.x1, b.y0], [b.x1, b.y1], [b.x0, b.y1]]
                            .into_iter()
                            .map(|p| {
                                let q = inv * kurbo::Point::new(p[0], p[1]);
                                [q.x, q.y]
                            })
                            .collect::<Vec<_>>()
                    })
                    .into_iter()
                    .collect::<Vec<_>>()
            } else {
                let Some(mut p) = local_outline(o) else {
                    continue;
                };
                p.apply_affine(inv * o.transform.affine());
                let mut polys = Vec::new();
                let mut cur: Vec<[f64; 2]> = Vec::new();
                kurbo::flatten(&p, 200.0, |el| match el {
                    PathEl::MoveTo(q) => {
                        if cur.len() >= 2 {
                            polys.push(std::mem::take(&mut cur));
                        }
                        cur = vec![[q.x, q.y]];
                    }
                    PathEl::LineTo(q) => cur.push([q.x, q.y]),
                    PathEl::ClosePath if cur.len() >= 2 => {
                        polys.push(std::mem::take(&mut cur));
                    }
                    _ => {}
                });
                if cur.len() >= 2 {
                    polys.push(cur);
                }
                polys
            };
            if !polys.is_empty() {
                out.push(WrapZone {
                    polys,
                    side: w.side.clone(),
                    distance: w.distance.max(0.0),
                });
            }
        }
    }
    out
}

fn is_paragraph(o: &Object) -> bool {
    matches!(&o.kind, ObjectKind::Text(t) if t.text_kind == "paragraph" && t.frame.is_some())
}

/// Textos cujas áreas mudaram: local e áreas novas.
fn changes(doc: &Document) -> Vec<(ObjLoc, Vec<WrapZone>)> {
    let mut out = Vec::new();
    for (pi, page) in doc.data.pages.iter().enumerate() {
        let any_wrap = page
            .layers
            .iter()
            .any(|l| l.objects.iter().any(|o| o.wrap.is_some()));
        for (li, layer) in page.layers.iter().enumerate() {
            for (oi, o) in layer.objects.iter().enumerate() {
                let ObjectKind::Text(t) = &o.kind else {
                    continue;
                };
                if !is_paragraph(o) || (!any_wrap && t.exclusions.0.is_empty()) {
                    continue;
                }
                let z = if any_wrap {
                    zones_for(page, o)
                } else {
                    Vec::new()
                };
                if z != t.exclusions.0 {
                    out.push((
                        ObjLoc {
                            page: pi,
                            layer: li,
                            index: oi,
                        },
                        z,
                    ));
                }
            }
        }
    }
    out
}

/// Refaz as áreas dos textos dentro da transação aberta.
pub fn refresh(ed: &mut Editor) {
    for (loc, z) in changes(&ed.doc) {
        let mut n = ed.doc.data.pages[loc.page].layers[loc.layer].objects[loc.index].clone();
        if let ObjectKind::Text(t) = &mut n.kind {
            t.exclusions.0 = z;
        }
        ed.replace_object_always(loc, n);
    }
}

/// Refaz as áreas direto no documento (ao abrir).
pub fn refresh_doc(doc: &mut Document) {
    for (loc, z) in changes(doc) {
        if let ObjectKind::Text(t) =
            &mut doc.data.pages[loc.page].layers[loc.layer].objects[loc.index].kind
        {
            t.exclusions.0 = z;
        }
    }
}

/// Faixa ocupada (x mínimo e máximo) do polígono entre `y0` e `y1`.
fn x_range(poly: &[[f64; 2]], y0: f64, y1: f64) -> Option<(f64, f64)> {
    let n = poly.len();
    let mut lo = f64::INFINITY;
    let mut hi = f64::NEG_INFINITY;
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        let (ya, yb) = (a[1], b[1]);
        if ya.max(yb) < y0 || ya.min(yb) > y1 {
            continue;
        }
        let at = |y: f64| {
            if (yb - ya).abs() < 1e-12 {
                a[0]
            } else {
                a[0] + (b[0] - a[0]) * ((y - ya) / (yb - ya)).clamp(0.0, 1.0)
            }
        };
        let (ta, tb) = (ya.clamp(y0, y1), yb.clamp(y0, y1));
        for x in [at(ta), at(tb)] {
            lo = lo.min(x);
            hi = hi.max(x);
        }
    }
    (lo <= hi).then_some((lo, hi))
}

/// Trechos livres de `[0, width]` na faixa `[y0, y1]` (espaço do texto),
/// da esquerda para a direita, com pelo menos `min` de largura.
pub fn free_slots(zones: &[WrapZone], width: f64, y0: f64, y1: f64, min: f64) -> Vec<(f64, f64)> {
    let mut blocked: Vec<(f64, f64)> = Vec::new();
    for z in zones {
        let d = z.distance;
        let range = z
            .polys
            .iter()
            .filter_map(|p| x_range(p, y0 - d, y1 + d))
            .reduce(|a, b| (a.0.min(b.0), a.1.max(b.1)));
        let Some((lo, hi)) = range else {
            continue;
        };
        let (lo, hi) = (lo - d, hi + d);
        blocked.push(match z.side.as_str() {
            "left" => (lo, f64::INFINITY),
            "right" => (f64::NEG_INFINITY, hi),
            "above-below" => (f64::NEG_INFINITY, f64::INFINITY),
            _ => (lo, hi),
        });
    }
    blocked.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut out = Vec::new();
    let mut x = 0.0;
    for (lo, hi) in blocked {
        if lo > x {
            out.push((x, lo.min(width)));
        }
        x = x.max(hi);
        if x >= width {
            break;
        }
    }
    if x < width {
        out.push((x, width));
    }
    out.retain(|(a, b)| b - a >= min);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Page, RectData, TextWrap, Transform};

    fn zone(side: &str) -> WrapZone {
        WrapZone {
            polys: vec![vec![
                [40.0, -60.0],
                [60.0, -60.0],
                [60.0, -40.0],
                [40.0, -40.0],
            ]],
            side: side.into(),
            distance: 5.0,
        }
    }

    /// GD-TXT-10: os quatro lados; fora da faixa do objeto, a linha toda.
    #[test]
    fn gd_txt_10_slots() {
        let z = [zone("both")];
        assert_eq!(
            free_slots(&z, 100.0, -55.0, -50.0, 1.0),
            vec![(0.0, 35.0), (65.0, 100.0)]
        );
        assert_eq!(free_slots(&z, 100.0, -20.0, -10.0, 1.0), vec![(0.0, 100.0)]);
        assert_eq!(
            free_slots(&[zone("left")], 100.0, -55.0, -50.0, 1.0),
            vec![(0.0, 35.0)]
        );
        assert_eq!(
            free_slots(&[zone("right")], 100.0, -55.0, -50.0, 1.0),
            vec![(65.0, 100.0)]
        );
        assert!(free_slots(&[zone("above-below")], 100.0, -55.0, -50.0, 1.0).is_empty());
        // Trecho estreito demais some.
        assert_eq!(free_slots(&z, 100.0, -55.0, -50.0, 40.0), vec![]);
    }

    /// As áreas acompanham o objeto no mesmo passo e são refeitas ao abrir.
    #[test]
    fn zones_follow_object() {
        let mut d = Document::new();
        let mut page = Page::new(200_000.0, 200_000.0);
        let mut text = Object::new(ObjectKind::Text(crate::model::TextData {
            text_kind: "paragraph".into(),
            frame: Some([100_000.0, 100_000.0]),
            paragraphs: Vec::new(),
            line_spacing: 1.2,
            path: None,
            exclusions: Default::default(),
            path_memory: None,
            tracking: 0.0,
            extra: Default::default(),
        }));
        text.transform = Transform::translate(0.0, 150_000.0);
        let tid = text.id.clone();
        let mut r = Object::new(ObjectKind::Rectangle(RectData {
            width: 20_000.0,
            height: 20_000.0,
            corners: None,
            extra: Default::default(),
        }));
        r.transform = Transform::translate(40_000.0, 100_000.0);
        r.wrap = Some(TextWrap {
            style: "square".into(),
            side: "both".into(),
            distance: 0.0,
        });
        let rid = r.id.clone();
        page.layers[0].objects.push(text);
        page.layers[0].objects.push(r.clone());
        d.data.pages.push(page);
        let mut ed = Editor::new(d);
        let ex = |ed: &Editor| match &ed.get(&ed.find(&tid).unwrap()).unwrap().kind {
            ObjectKind::Text(t) => t.exclusions.0.clone(),
            _ => unreachable!(),
        };
        let z = ex(&ed);
        assert_eq!(z.len(), 1);
        assert!((z[0].polys[0][0][1] - -50_000.0).abs() < 1e-6, "{z:?}");
        super::super::ops::transform(
            &mut ed,
            std::slice::from_ref(&rid),
            Affine::translate((0.0, -10_000.0)),
            "Mover",
        );
        assert!((ex(&ed)[0].polys[0][0][1] - -60_000.0).abs() < 1e-6);
        // Ida e volta: textWrap gravado, áreas não.
        let o = ed.get(&ed.find(&rid).unwrap()).unwrap().clone();
        let v = o.to_value();
        assert_eq!(v["textWrap"]["side"], "both");
        assert_eq!(Object::from_value(v), o);
        let _ = r;
    }
}
