//! Conta-gotas (doc 27): leitura nativa da cor sob um ponto (sem passar
//! pela rasterização) e aplicação do que foi coletado — cor, rampa de
//! gradiente ou atributos de outro objeto — nos objetos de destino.

use kurbo::{Affine, Point, Rect};

use super::history::Editor;
use super::ops;
use super::query::{self, local_outline};
use crate::model::{Color, Fill, Gradient, Object, ObjectKind, Page, PathData, Stroke, Transform};
use crate::render::scene::all_closed;

/// Leitura nativa (GD-CTG-03): o objeto sob o ponto e o valor da cor do
/// próprio modelo, sem desvio de anti-serrilhado.
#[derive(Debug, Clone, PartialEq)]
pub struct NativeSample {
    pub id: String,
    pub color: Color,
    pub opacity: f64,
    /// Rampa inteira quando o ponto caiu num preenchimento gradiente, com a
    /// caixa local do objeto de origem (para reposicionar no destino).
    pub ramp: Option<(Gradient, Rect)>,
}

/// O que o balde despeja como tinta (GD-CTG-05/09).
#[derive(Debug, Clone, PartialEq)]
pub enum Paint {
    Color(Color),
    /// Rampa clonada (Alt + clique no gradiente) e a caixa local da origem.
    Ramp(Gradient, Rect),
}

/// Famílias de atributos transferidas no modo Atributos (GD-CTG-08).
/// Efeitos visuais ainda não existem no modelo (Beta).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttrFilter {
    pub fill: bool,
    pub stroke: bool,
    pub typography: bool,
    pub transform: bool,
}

impl Default for AttrFilter {
    fn default() -> Self {
        AttrFilter {
            fill: true,
            stroke: true,
            typography: true,
            transform: false,
        }
    }
}

/// Caixa da geometria no espaço local do objeto (sem a matriz dele).
pub fn local_box(o: &Object) -> Option<Rect> {
    let mut bare = o.clone();
    bare.transform = Transform::IDENTITY;
    query::bounds(&bare, Affine::IDENTITY, false)
}

fn path_data(o: &Object) -> Option<PathData> {
    match &o.kind {
        ObjectKind::Path(pd) => Some(pd.clone()),
        _ => Some(PathData {
            subpaths: super::convert::bez_to_subpaths(&local_outline(o)?),
            fill_rule: crate::model::FillRule::Nonzero,
            extra: Default::default(),
        }),
    }
}

/// Caminho aberto: o clique esquerdo pinta o traço (não há preenchimento
/// visível).
fn is_open(o: &Object) -> bool {
    matches!(o.kind, ObjectKind::Path(_)) && local_outline(o).is_some_and(|p| !all_closed(&p))
}

/// Cor do modelo sob `p` (página): preenchimento se o ponto cai dentro
/// dele, senão o contorno. Imagens ficam de fora (a tela responde por
/// elas).
pub fn sample_native(ed: &Editor, page: &Page, p: Point, tol: f64) -> Option<NativeSample> {
    let id = query::hit_test_deep(page, p, tol)?;
    let r = ed.find(&id)?;
    let o = ed.get(&r)?;
    if matches!(o.kind, ObjectKind::Image(_)) {
        return None;
    }
    let m = ed.parent_affine(&r) * o.transform.affine();
    let lp = m.inverse() * p;
    let filled = !matches!(o.fill, None | Some(Fill::None));
    let in_fill = filled
        && match &o.kind {
            ObjectKind::Text(_) => true,
            _ => path_data(o).is_some_and(|pd| crate::geometry::contains(&pd, lp)),
        };
    let mk = |color: Color, opacity: f64, ramp| NativeSample {
        id: id.clone(),
        color,
        opacity,
        ramp,
    };
    if in_fill {
        match o.fill.as_ref()? {
            Fill::Solid { color, opacity, .. } => return Some(mk(color.clone(), *opacity, None)),
            Fill::Gradient(g) => {
                let t = crate::render::gradient::param(g, lp);
                let (c, a) = crate::render::gradient::color_at(g, t);
                let ramp = local_box(o).map(|b| (g.clone(), b));
                return Some(mk(c, a, ramp));
            }
            _ => {}
        }
    }
    let s = o.stroke.as_ref()?;
    Some(mk(s.color.clone(), s.opacity, None))
}

/// Destinos de um clique: o objeto atingido; um grupo vira as folhas dele,
/// a não ser com `deep` (Ctrl), que já aponta o objeto de dentro
/// (GD-CTG-07).
pub fn targets(page: &Page, p: Point, tol: f64, deep: bool) -> Vec<String> {
    if deep {
        return query::hit_test_deep(page, p, tol).into_iter().collect();
    }
    let Some(hit) = query::hit_test(page, p, tol) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    fn leaves(o: &Object, out: &mut Vec<String>) {
        if o.locked {
            return;
        }
        match &o.kind {
            ObjectKind::Group(g) => g.children.iter().for_each(|c| leaves(c, out)),
            _ => out.push(o.id.clone()),
        }
    }
    leaves(&page.layers[hit.layer].objects[hit.index], &mut out);
    out
}

/// Leva a rampa da caixa `from` para a caixa `to` (mesma posição relativa
/// das paradas, do eixo e do ângulo).
pub fn fit_ramp(g: &Gradient, from: Rect, to: Rect) -> Gradient {
    let sx = if from.width() > 0.0 {
        to.width() / from.width()
    } else {
        1.0
    };
    let sy = if from.height() > 0.0 {
        to.height() / from.height()
    } else {
        1.0
    };
    let map = |p: [f64; 2]| [to.x0 + (p[0] - from.x0) * sx, to.y0 + (p[1] - from.y0) * sy];
    let mut out = g.clone();
    out.start = g.start.map(map);
    out.end = g.end.map(map);
    out.center = g.center.map(map);
    if g.kind == "radial" {
        out.radius = g.radius.map(|r| r * (sx * sy).sqrt());
    } else {
        out.radius = g.radius.map(|r| r * sx);
        out.radius_y = g
            .radius_y
            .or(g.radius)
            .map(|r| r * sy)
            .filter(|_| g.radius.is_some());
    }
    out
}

fn paint_fill(o: &mut Object, fill: Fill) {
    // Texto: a caixa inteira (as cores por trecho saem).
    if let ObjectKind::Text(t) = &mut o.kind {
        for para in &mut t.paragraphs {
            for s in &mut para.spans {
                s.fill = None;
            }
        }
    }
    o.fill = Some(fill);
}

/// Despeja a tinta (GD-CTG-06): esquerdo = preenchimento (linha aberta
/// recebe no traço), direito (`stroke`) = contorno, mantendo o resto. Quem
/// não tinha contorno recebe `new_stroke` com a cor. Um passo de desfazer.
pub fn apply_paint(
    ed: &mut Editor,
    ids: &[String],
    paint: &Paint,
    stroke: bool,
    new_stroke: &Stroke,
) -> bool {
    let mut any = false;
    ops::modify(ed, ids, "Conta-gotas: aplicar cor", |o| {
        if matches!(o.kind, ObjectKind::Image(_) | ObjectKind::Group(_)) || o.locked {
            return false;
        }
        if stroke || is_open(o) {
            let Paint::Color(c) = paint else {
                return false;
            };
            match &mut o.stroke {
                Some(s) => s.color = c.clone(),
                None => {
                    let mut s = new_stroke.clone();
                    s.color = c.clone();
                    o.stroke = Some(s);
                }
            }
        } else {
            match paint {
                Paint::Color(c) => {
                    let opacity = match &o.fill {
                        Some(Fill::Solid { opacity, .. }) => *opacity,
                        _ => 1.0,
                    };
                    let mut f = Fill::solid(c.clone());
                    if let Fill::Solid { opacity: a, .. } = &mut f {
                        *a = opacity;
                    }
                    paint_fill(o, f);
                }
                Paint::Ramp(g, from) => {
                    let g = local_box(o).map_or_else(|| g.clone(), |to| fit_ramp(g, *from, to));
                    paint_fill(o, Fill::Gradient(g));
                }
            }
        }
        any = true;
        true
    });
    any
}

/// Balde sobre uma parada da rampa (GD-CTG-09): só aquela parada muda.
pub fn replace_stop(ed: &mut Editor, id: &str, index: usize, color: &Color) -> bool {
    let mut done = false;
    ops::modify(ed, &[id.to_string()], "Conta-gotas: cor da parada", |o| {
        if let Some(Fill::Gradient(g)) = &mut o.fill
            && let Some(s) = g.stops.get_mut(index)
        {
            s.color = color.clone();
            done = true;
            return true;
        }
        false
    });
    done
}

/// Escala, giro e inclinação de `src` em `o`, com o centro de `o` parado.
fn copy_linear(src: &Object, o: &mut Object) {
    let Some(b) = local_box(o) else {
        return;
    };
    let c = b.center();
    let here = o.transform.affine() * c;
    let s = src.transform.0;
    let lin = Affine::new([s[0], s[1], s[2], s[3], 0.0, 0.0]);
    let moved = lin * c;
    o.transform = Transform([s[0], s[1], s[2], s[3], here.x - moved.x, here.y - moved.y]);
}

/// Modo Atributos (GD-CTG-08): copia de `src` as famílias marcadas em
/// `filter` para `ids` (a origem fica de fora). Um passo de desfazer.
pub fn apply_attributes(
    ed: &mut Editor,
    src: &Object,
    ids: &[String],
    filter: &AttrFilter,
) -> bool {
    let from = local_box(src);
    let mut any = false;
    ops::modify(ed, ids, "Conta-gotas: aplicar atributos", |o| {
        if o.id == src.id || o.locked || matches!(o.kind, ObjectKind::Group(_)) {
            return false;
        }
        let before = o.clone();
        let image = matches!(o.kind, ObjectKind::Image(_));
        if filter.fill && !image {
            match (&src.fill, from, local_box(o)) {
                (Some(Fill::Gradient(g)), Some(f), Some(t)) => {
                    paint_fill(o, Fill::Gradient(fit_ramp(g, f, t)))
                }
                (Some(f), _, _) => paint_fill(o, f.clone()),
                (None, _, _) => o.fill = None,
            }
        }
        if filter.stroke && !image {
            o.stroke = src.stroke.clone();
        }
        if filter.typography
            && let (ObjectKind::Text(st), ObjectKind::Text(t)) = (&src.kind, &mut o.kind)
            && let Some(first) = st.paragraphs.iter().flat_map(|p| &p.spans).next()
        {
            for para in &mut t.paragraphs {
                for s in &mut para.spans {
                    s.font.family = first.font.family.clone();
                    s.font.size = first.font.size;
                    s.font.weight = first.font.weight;
                    s.font.italic = first.font.italic;
                }
            }
            t.line_spacing = st.line_spacing;
        }
        if filter.transform {
            copy_linear(src, o);
        }
        let changed = *o != before;
        any |= changed;
        changed
    });
    any
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Document, GradientStop, RectData};
    use serde_json::Map;

    fn rect(x: f64, fill: Fill) -> Object {
        let mut o = Object::new(ObjectKind::Rectangle(RectData {
            width: 100.0,
            height: 100.0,
            corners: None,
            extra: Map::new(),
        }));
        o.transform = Transform::translate(x, 0.0);
        o.fill = Some(fill);
        o
    }

    fn ramp() -> Gradient {
        let stop = |offset, hex| GradientStop {
            offset,
            color: Color::from_hex(hex).unwrap(),
            opacity: 1.0,
            extra: Map::new(),
        };
        Gradient {
            kind: "linear".into(),
            stops: vec![stop(0.0, "#ff0000"), stop(1.0, "#0000ff")],
            start: Some([0.0, 50.0]),
            end: Some([100.0, 50.0]),
            center: None,
            radius: None,
            radius_y: None,
            angle: None,
            spread: "pad".into(),
            extra: Map::new(),
        }
    }

    fn setup(objs: Vec<Object>) -> (Editor, Vec<String>) {
        let mut d = Document::new();
        d.data.pages.push(Page::new(1000.0, 1000.0));
        let mut ed = Editor::new(d);
        let ids = objs
            .into_iter()
            .map(|o| ops::add_object(&mut ed, 0, 0, o, "r"))
            .collect();
        (ed, ids)
    }

    fn get(ed: &Editor, id: &str) -> Object {
        ed.get(&ed.find(id).unwrap()).unwrap().clone()
    }

    /// GD-CTG-03/09: leitura nativa dá a cor exata; no gradiente, a do
    /// ponto e a rampa inteira.
    #[test]
    fn gd_ctg_03_native_sample_solid_and_gradient() {
        let red = Color::from_hex("#ff0000").unwrap();
        let (ed, ids) = setup(vec![
            rect(0.0, Fill::solid(red.clone())),
            rect(200.0, Fill::Gradient(ramp())),
        ]);
        let page = &ed.doc.data.pages[0];
        let s = sample_native(&ed, page, Point::new(50.0, 50.0), 1.0).unwrap();
        assert_eq!((s.id.as_str(), &s.color), (ids[0].as_str(), &red));
        assert!(s.ramp.is_none());
        let s = sample_native(&ed, page, Point::new(250.0, 50.0), 1.0).unwrap();
        let [r, _, b] = s.color.to_srgb();
        assert!((r - b).abs() < 0.05, "meio da rampa: {:?}", s.color);
        let (g, from) = s.ramp.unwrap();
        assert_eq!(g.stops.len(), 2);
        assert_eq!(from, Rect::new(0.0, 0.0, 100.0, 100.0));
        assert!(sample_native(&ed, page, Point::new(150.0, 50.0), 1.0).is_none());
    }

    /// GD-CTG-06: esquerdo = preenchimento, direito = contorno (sem mexer
    /// no preenchimento); um passo de desfazer cada.
    #[test]
    fn gd_ctg_06_left_fill_right_stroke() {
        let blue = Color::from_hex("#0000ff").unwrap();
        let (mut ed, ids) = setup(vec![rect(0.0, Fill::solid(Color::black()))]);
        let tpl = Stroke::new(200.0, Color::black());
        assert!(apply_paint(
            &mut ed,
            &ids,
            &Paint::Color(blue.clone()),
            false,
            &tpl
        ));
        assert_eq!(get(&ed, &ids[0]).fill, Some(Fill::solid(blue.clone())));
        let green = Color::from_hex("#00ff00").unwrap();
        assert!(apply_paint(
            &mut ed,
            &ids,
            &Paint::Color(green.clone()),
            true,
            &tpl
        ));
        let o = get(&ed, &ids[0]);
        assert_eq!(o.fill, Some(Fill::solid(blue)));
        assert_eq!(o.stroke.unwrap().color, green);
        ed.undo();
        assert!(get(&ed, &ids[0]).stroke.is_none());
    }

    /// GD-CTG-07: sem Ctrl o grupo inteiro recebe; com Ctrl só o de dentro.
    #[test]
    fn gd_ctg_07_group_targets() {
        let (mut ed, ids) = setup(vec![
            rect(0.0, Fill::solid(Color::black())),
            rect(200.0, Fill::solid(Color::black())),
        ]);
        ops::group(&mut ed, &ids);
        let page = &ed.doc.data.pages[0];
        let mut all = targets(page, Point::new(50.0, 50.0), 1.0, false);
        all.sort();
        let mut want = ids.clone();
        want.sort();
        assert_eq!(all, want);
        assert_eq!(
            targets(page, Point::new(50.0, 50.0), 1.0, true),
            vec![ids[0].clone()]
        );
    }

    /// GD-CTG-09: a rampa clonada acompanha a caixa do destino; o balde
    /// numa parada troca só ela.
    #[test]
    fn gd_ctg_09_clone_ramp_and_replace_stop() {
        let (mut ed, ids) = setup(vec![rect(0.0, Fill::solid(Color::black()))]);
        let mut big = get(&ed, &ids[0]);
        big.kind = ObjectKind::Rectangle(RectData {
            width: 400.0,
            height: 200.0,
            corners: None,
            extra: Map::new(),
        });
        let r = ed.find(&ids[0]).unwrap();
        ed.transaction("t", |ed| ed.replace_at(&r, big));
        let tpl = Stroke::new(200.0, Color::black());
        apply_paint(
            &mut ed,
            &ids,
            &Paint::Ramp(ramp(), Rect::new(0.0, 0.0, 100.0, 100.0)),
            false,
            &tpl,
        );
        let Some(Fill::Gradient(g)) = get(&ed, &ids[0]).fill else {
            panic!("gradiente");
        };
        assert_eq!((g.start, g.end), (Some([0.0, 100.0]), Some([400.0, 100.0])));
        let green = Color::from_hex("#00ff00").unwrap();
        assert!(replace_stop(&mut ed, &ids[0], 1, &green));
        let Some(Fill::Gradient(g)) = get(&ed, &ids[0]).fill else {
            panic!("gradiente");
        };
        assert_eq!(g.stops[1].color, green);
        assert_eq!(g.stops[0].color, Color::from_hex("#ff0000").unwrap());
    }

    /// GD-CTG-08: só as famílias marcadas passam; transformação mantém o
    /// centro do destino.
    #[test]
    fn gd_ctg_08_attribute_filters() {
        let mut src = rect(0.0, Fill::solid(Color::from_hex("#ff0000").unwrap()));
        src.stroke = Some(Stroke::new(500.0, Color::from_hex("#00ff00").unwrap()));
        src.transform = Transform([0.0, 1.0, -1.0, 0.0, 0.0, 0.0]);
        let (mut ed, ids) = setup(vec![src, rect(300.0, Fill::solid(Color::black()))]);
        let src = get(&ed, &ids[0]);
        let only_stroke = AttrFilter {
            fill: false,
            stroke: true,
            typography: false,
            transform: false,
        };
        assert!(apply_attributes(&mut ed, &src, &ids[1..], &only_stroke));
        let o = get(&ed, &ids[1]);
        assert_eq!(o.fill, Some(Fill::solid(Color::black())));
        assert_eq!(o.stroke.as_ref().unwrap().width, 500.0);
        let tr = AttrFilter {
            fill: false,
            stroke: false,
            typography: false,
            transform: true,
        };
        let c0 = query::bounds(&o, Affine::IDENTITY, false).unwrap().center();
        assert!(apply_attributes(&mut ed, &src, &ids[1..], &tr));
        let o = get(&ed, &ids[1]);
        let c1 = query::bounds(&o, Affine::IDENTITY, false).unwrap().center();
        assert!((c0 - c1).hypot() < 1e-9);
        assert_eq!(&o.transform.0[..4], &[0.0, 1.0, -1.0, 0.0]);
        // A origem não muda a si mesma.
        assert!(!apply_attributes(
            &mut ed,
            &src,
            &ids[..1],
            &AttrFilter::default()
        ));
    }
}
