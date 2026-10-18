//! Operações de edição sobre a seleção, cada uma numa transação com nome
//! legível (GD-UND-01). A seleção é uma lista ordenada de IDs (GD-SEL-01: a
//! ordem de seleção é preservada para "relativo ao último/primeiro"). Os IDs
//! podem ser de objetos dentro de grupos (GD-SEL-02); as transformações são
//! dadas em coordenadas da página e convertidas para o espaço do pai.
//!
//! - GD-TRF-01: mover, escalar, girar, refletir, inclinar; posição e tamanho
//!   por ponto de referência (9 âncoras).
//! - GD-TRF-02: aplicar ao conjunto ou a cada objeto.
//! - GD-ALI-01: alinhar relativo ao último/primeiro selecionado ou à página.
//! - GD-ORD-01: ordem de empilhamento.
//! - Agrupar e desagrupar (doc 02: grupos contêm objetos).

use std::collections::BTreeMap;

use kurbo::{Affine, Point, Rect, Vec2};

use super::history::{Editor, ObjLoc};
use super::query::bounds;
use super::tree::{Container, ObjRef};
use crate::model::{Fill, GroupData, Object, ObjectKind, Stroke, Transform};

pub type Selection = Vec<String>;

/// Ponto de referência (9 âncoras da caixa).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Anchor {
    TopLeft,
    Top,
    TopRight,
    Left,
    Center,
    Right,
    BottomLeft,
    Bottom,
    BottomRight,
}

impl Anchor {
    pub const ALL: [Anchor; 9] = [
        Anchor::TopLeft,
        Anchor::Top,
        Anchor::TopRight,
        Anchor::Left,
        Anchor::Center,
        Anchor::Right,
        Anchor::BottomLeft,
        Anchor::Bottom,
        Anchor::BottomRight,
    ];

    /// Fração (x, y) da caixa, com Y para cima (0 = base).
    pub fn fraction(self) -> (f64, f64) {
        match self {
            Anchor::TopLeft => (0.0, 1.0),
            Anchor::Top => (0.5, 1.0),
            Anchor::TopRight => (1.0, 1.0),
            Anchor::Left => (0.0, 0.5),
            Anchor::Center => (0.5, 0.5),
            Anchor::Right => (1.0, 0.5),
            Anchor::BottomLeft => (0.0, 0.0),
            Anchor::Bottom => (0.5, 0.0),
            Anchor::BottomRight => (1.0, 0.0),
        }
    }

    pub fn point(self, r: Rect) -> Point {
        let (fx, fy) = self.fraction();
        Point::new(r.x0 + fx * r.width(), r.y0 + fy * r.height())
    }

    /// Âncora oposta (usada ao escalar por uma alça).
    pub fn opposite(self) -> Anchor {
        let (fx, fy) = self.fraction();
        Anchor::ALL
            .into_iter()
            .find(|a| a.fraction() == (1.0 - fx, 1.0 - fy))
            .unwrap()
    }
}

/// Referências dos objetos selecionados que ainda existem.
pub fn refs(ed: &Editor, sel: &[String]) -> Vec<ObjRef> {
    sel.iter().filter_map(|id| ed.find(id)).collect()
}

pub fn selected_objects<'a>(ed: &'a Editor, sel: &[String]) -> Vec<&'a Object> {
    refs(ed, sel).iter().filter_map(|r| ed.get(r)).collect()
}

/// Caixa do objeto em coordenadas da página (considera os grupos acima dele).
pub fn object_bounds(ed: &Editor, r: &ObjRef, visual: bool) -> Option<Rect> {
    bounds(ed.get(r)?, ed.parent_affine(r), visual)
}

pub fn selection_bounds(ed: &Editor, sel: &[String], visual: bool) -> Option<Rect> {
    refs(ed, sel)
        .iter()
        .filter_map(|r| object_bounds(ed, r, visual))
        .reduce(|a, b| a.union(b))
}

/// Matriz do objeto depois de aplicar `m` (coordenadas da página).
pub fn page_transform(ed: &Editor, r: &ObjRef, o: &Object, m: Affine) -> Transform {
    let p = ed.parent_affine(r);
    let local = if p == Affine::IDENTITY {
        m
    } else {
        p.inverse() * m * p
    };
    Transform::from_affine(local * o.transform.affine())
}

fn plural(n: usize, one: &str, many: &str) -> String {
    if n == 1 {
        format!("1 {one}")
    } else {
        format!("{n} {many}")
    }
}

/// Aplica `m` (em coordenadas da página) a cada objeto.
pub fn transform(ed: &mut Editor, sel: &[String], m: Affine, name: &str) {
    let refs = refs(ed, sel);
    if refs.is_empty() {
        return;
    }
    let title = format!("{name} {}", plural(refs.len(), "objeto", "objetos"));
    ed.transaction(title, |ed| {
        for r in refs {
            let mut o = ed.get(&r).unwrap().clone();
            o.transform = page_transform(ed, &r, &o, m);
            ed.replace_at(&r, o);
        }
    });
}

pub fn translate(ed: &mut Editor, sel: &[String], d: Vec2) {
    transform(ed, sel, Affine::translate(d), "Mover");
}

/// Escala em torno de `origin`.
pub fn scale_about(origin: Point, sx: f64, sy: f64) -> Affine {
    Affine::translate(origin.to_vec2())
        * Affine::scale_non_uniform(sx, sy)
        * Affine::translate(-origin.to_vec2())
}

/// Rotação anti-horária (graus) em torno de `center`.
pub fn rotate_about(center: Point, degrees: f64) -> Affine {
    Affine::rotate_about(degrees.to_radians(), center)
}

/// Inclinação (graus) horizontal e vertical em torno de `center`.
pub fn skew_about(center: Point, hdeg: f64, vdeg: f64) -> Affine {
    let sk = Affine::new([
        1.0,
        vdeg.to_radians().tan(),
        hdeg.to_radians().tan(),
        1.0,
        0.0,
        0.0,
    ]);
    Affine::translate(center.to_vec2()) * sk * Affine::translate(-center.to_vec2())
}

/// Reflexão horizontal (`horizontal = true`, espelha em X) ou vertical.
pub fn mirror_about(center: Point, horizontal: bool) -> Affine {
    if horizontal {
        scale_about(center, -1.0, 1.0)
    } else {
        scale_about(center, 1.0, -1.0)
    }
}

/// Transformação aplicada ao conjunto (uma caixa para todos) ou a cada objeto
/// em relação à própria caixa (GD-TRF-02). `make` recebe a caixa de referência.
pub fn transform_each(
    ed: &mut Editor,
    sel: &[String],
    individually: bool,
    name: &str,
    make: impl Fn(Rect) -> Affine,
) {
    if !individually {
        if let Some(b) = selection_bounds(ed, sel, false) {
            transform(ed, sel, make(b), name);
        }
        return;
    }
    let refs = refs(ed, sel);
    let title = format!("{name} {}", plural(refs.len(), "objeto", "objetos"));
    ed.transaction(title, |ed| {
        for r in refs {
            if let Some(b) = object_bounds(ed, &r, false) {
                let mut o = ed.get(&r).unwrap().clone();
                o.transform = page_transform(ed, &r, &o, make(b));
                ed.replace_at(&r, o);
            }
        }
    });
}

/// Aplica em cópia, `copies` vezes, acumulando a transformação (GD-TRF-02:
/// "girar 30° × 11 cópias"). As cópias vão para o topo do mesmo contêiner
/// (camada ou grupo) do original. Devolve os IDs das cópias.
pub fn transform_copies(ed: &mut Editor, sel: &[String], m: Affine, copies: usize) -> Selection {
    let refs = refs(ed, sel);
    let mut out = Vec::new();
    ed.transaction(
        format!("Transformar em {}", plural(copies, "cópia", "cópias")),
        |ed| {
            for r in refs {
                // Copiar para um grupo muda os índices de quem vem depois nele.
                let Some(r) = ed.get(&r).map(|o| o.id.clone()).and_then(|id| ed.find(&id)) else {
                    continue;
                };
                let base = ed.get(&r).unwrap().clone();
                let c = r.container();
                let mut acc = Affine::IDENTITY;
                for _ in 0..copies {
                    acc = m * acc;
                    let mut copy = fresh_copy(&base);
                    copy.transform = page_transform(ed, &r, &base, acc);
                    out.push(copy.id.clone());
                    let at = ed.container_len(&c);
                    ed.insert_at(&c, at, copy);
                }
            }
        },
    );
    out
}

/// Posiciona a seleção para que a âncora fique em `p` (GD-TRF-01).
pub fn set_position(ed: &mut Editor, sel: &[String], anchor: Anchor, p: Point) {
    if let Some(b) = selection_bounds(ed, sel, false) {
        translate(ed, sel, p - anchor.point(b));
    }
}

/// Define o tamanho da seleção mantendo fixa a âncora (GD-TRF-01).
pub fn set_size(
    ed: &mut Editor,
    sel: &[String],
    anchor: Anchor,
    w: Option<f64>,
    h: Option<f64>,
    proportional: bool,
) {
    let Some(b) = selection_bounds(ed, sel, false) else {
        return;
    };
    let mut sx = w.filter(|_| b.width() > 0.0).map_or(1.0, |w| w / b.width());
    let mut sy = h
        .filter(|_| b.height() > 0.0)
        .map_or(1.0, |h| h / b.height());
    if proportional {
        if w.is_some() && h.is_none() {
            sy = sx;
        } else if h.is_some() && w.is_none() {
            sx = sy;
        }
    }
    if sx.abs() < 1e-12 || sy.abs() < 1e-12 {
        return;
    }
    transform(
        ed,
        sel,
        scale_about(anchor.point(b), sx, sy),
        "Redimensionar",
    );
}

// ---- alinhamento (GD-ALI-01) ----------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Left,
    CenterH,
    Right,
    Top,
    CenterV,
    Bottom,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AlignTo {
    LastSelected,
    FirstSelected,
    /// Página (centro/bordas da página).
    Page(Rect),
    /// Caixa envolvente da seleção.
    Selection,
}

/// Referência efetiva do alinhamento (GD-ALI-06): com um só objeto não há
/// outro para servir de base, então ele se alinha pela página.
pub fn align_target(to: AlignTo, count: usize, page: Rect) -> AlignTo {
    match to {
        AlignTo::Page(_) => to,
        _ if count == 1 => AlignTo::Page(page),
        _ => to,
    }
}

pub fn align(ed: &mut Editor, sel: &[String], how: Align, to: AlignTo, visual: bool) {
    let objs: Vec<(String, Rect)> = sel
        .iter()
        .filter_map(|id| Some((id.clone(), object_bounds(ed, &ed.find(id)?, visual)?)))
        .collect();
    if objs.is_empty() {
        return;
    }
    let target = match to {
        AlignTo::LastSelected => objs.last().unwrap().1,
        AlignTo::FirstSelected => objs[0].1,
        AlignTo::Page(r) => r,
        AlignTo::Selection => objs.iter().map(|o| o.1).reduce(|a, b| a.union(b)).unwrap(),
    };
    let movers: Vec<&(String, Rect)> = match to {
        AlignTo::LastSelected => objs[..objs.len() - 1].iter().collect(),
        AlignTo::FirstSelected => objs[1..].iter().collect(),
        _ => objs.iter().collect(),
    };
    let name = match how {
        Align::Left => "Alinhar à esquerda",
        Align::CenterH => "Centralizar na horizontal",
        Align::Right => "Alinhar à direita",
        Align::Top => "Alinhar ao topo",
        Align::CenterV => "Centralizar na vertical",
        Align::Bottom => "Alinhar à base",
    };
    ed.transaction(name, |ed| {
        for (id, b) in movers {
            let d = match how {
                Align::Left => Vec2::new(target.x0 - b.x0, 0.0),
                Align::Right => Vec2::new(target.x1 - b.x1, 0.0),
                Align::CenterH => Vec2::new(target.center().x - b.center().x, 0.0),
                Align::Bottom => Vec2::new(0.0, target.y0 - b.y0),
                Align::Top => Vec2::new(0.0, target.y1 - b.y1),
                Align::CenterV => Vec2::new(0.0, target.center().y - b.center().y),
            };
            if d.hypot() > 0.0 {
                let r = ed.find(id).unwrap();
                let mut o = ed.get(&r).unwrap().clone();
                o.transform = page_transform(ed, &r, &o, Affine::translate(d));
                ed.replace_at(&r, o);
            }
        }
    });
}

/// Altura (y da página) da primeira linha de base de um texto.
pub fn text_baseline(ed: &Editor, r: &ObjRef) -> Option<f64> {
    let o = ed.get(r)?;
    let ObjectKind::Text(t) = &o.kind else {
        return None;
    };
    let layout = crate::render::text::layout_text(t);
    let line = layout.lines.first()?;
    let m = ed.parent_affine(r) * o.transform.affine();
    Some((m * Point::new(0.0, line.baseline)).y)
}

/// Alinhar pela linha de base (GD-ALI-04): textos pela primeira linha de
/// base, os outros objetos pela base da caixa; o alvo segue `to` (com
/// página ou seleção, vale a base delas).
pub fn align_baseline(ed: &mut Editor, sel: &[String], to: AlignTo, visual: bool) {
    let line = |ed: &Editor, id: &str| -> Option<f64> {
        let r = ed.find(id)?;
        text_baseline(ed, &r).or_else(|| object_bounds(ed, &r, visual).map(|b| b.y0))
    };
    let objs: Vec<(String, f64)> = sel
        .iter()
        .filter_map(|id| Some((id.clone(), line(ed, id)?)))
        .collect();
    if objs.is_empty() {
        return;
    }
    let target = match to {
        AlignTo::LastSelected => objs.last().unwrap().1,
        AlignTo::FirstSelected => objs[0].1,
        AlignTo::Page(r) => r.y0,
        AlignTo::Selection => objs.iter().map(|o| o.1).fold(f64::INFINITY, f64::min),
    };
    let movers: Vec<&(String, f64)> = match to {
        AlignTo::LastSelected => objs[..objs.len() - 1].iter().collect(),
        AlignTo::FirstSelected => objs[1..].iter().collect(),
        _ => objs.iter().collect(),
    };
    ed.transaction("Alinhar pela linha de base", |ed| {
        for (id, y) in movers {
            let d = Vec2::new(0.0, target - y);
            if d.y.abs() > 0.0 {
                let r = ed.find(id).unwrap();
                let mut o = ed.get(&r).unwrap().clone();
                o.transform = page_transform(ed, &r, &o, Affine::translate(d));
                ed.replace_at(&r, o);
            }
        }
    });
}

// ---- ordem (GD-ORD-01) ------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Order {
    ToFront,
    ToBack,
    Forward,
    Backward,
    Reverse,
}

/// Seleção agrupada por contêiner, na ordem de empilhamento. Os contêineres
/// mais fundos vêm primeiro: reordená-los não muda a posição dos de cima.
fn by_container(ed: &Editor, sel: &[String]) -> Vec<(Container, Vec<String>)> {
    let mut refs = refs(ed, sel);
    refs.sort();
    let mut map: BTreeMap<Container, Vec<String>> = BTreeMap::new();
    for r in refs {
        let id = ed.get(&r).unwrap().id.clone();
        map.entry(r.container()).or_default().push(id);
    }
    let mut out: Vec<_> = map.into_iter().collect();
    out.sort_by_key(|(c, _)| {
        std::cmp::Reverse(match c {
            Container::Layer { .. } => 0,
            Container::Group(g) => g.path.len() + 1,
        })
    });
    out
}

pub fn reorder(ed: &mut Editor, sel: &[String], how: Order) {
    let groups = by_container(ed, sel);
    if groups.is_empty() {
        return;
    }
    let name = match how {
        Order::ToFront => "Trazer para frente",
        Order::ToBack => "Enviar para trás",
        Order::Forward => "Um passo à frente",
        Order::Backward => "Um passo atrás",
        Order::Reverse => "Inverter ordem",
    };
    ed.transaction(name, |ed| {
        for (c, ids) in &groups {
            reorder_in(ed, c, ids, how);
        }
    });
}

fn reorder_in(ed: &mut Editor, c: &Container, ids: &[String], how: Order) {
    let pos = |ed: &Editor, id: &str| ed.position_in(c, id).unwrap();
    match how {
        Order::ToFront => {
            for id in ids {
                let top = ed.container_len(c) - 1;
                let i = pos(ed, id);
                ed.move_in(c, i, top);
            }
        }
        Order::ToBack => {
            for (k, id) in ids.iter().enumerate() {
                let i = pos(ed, id);
                ed.move_in(c, i, k);
            }
        }
        Order::Forward => {
            for id in ids.iter().rev() {
                let i = pos(ed, id);
                let n = ed.container_len(c);
                if i + 1 < n && !ids.iter().any(|s| Some(s.as_str()) == ed.id_in(c, i + 1)) {
                    ed.move_in(c, i, i + 1);
                }
            }
        }
        Order::Backward => {
            for id in ids {
                let i = pos(ed, id);
                if i > 0 && !ids.iter().any(|s| Some(s.as_str()) == ed.id_in(c, i - 1)) {
                    ed.move_in(c, i, i - 1);
                }
            }
        }
        Order::Reverse => {
            let slots: Vec<ObjRef> = ids.iter().map(|id| ed.child_ref(c, pos(ed, id))).collect();
            let objs: Vec<Object> = slots.iter().map(|r| ed.get(r).unwrap().clone()).collect();
            for (slot, o) in slots.iter().zip(objs.into_iter().rev()) {
                ed.replace_at(slot, o);
            }
        }
    }
}

// ---- agrupar (doc 02) ------------------------------------------------------

/// Agrupa objetos que estão no mesmo contêiner (camada ou grupo). O grupo
/// fica na posição do objeto mais à frente. Devolve o ID do grupo.
pub fn group(ed: &mut Editor, sel: &[String]) -> Option<String> {
    let mut refs = refs(ed, sel);
    if refs.len() < 2 {
        return None;
    }
    refs.sort();
    let c = refs[0].container();
    if refs.iter().any(|r| r.container() != c) {
        return None;
    }
    let children: Vec<Object> = refs.iter().map(|r| ed.get(r).unwrap().clone()).collect();
    let g = Object::new(ObjectKind::Group(GroupData {
        children,
        boolean: None,
        symmetry: None,
        effect: None,
        extra: Default::default(),
    }));
    let id = g.id.clone();
    let indices: Vec<usize> = refs.iter().map(ObjRef::index).collect();
    let at = indices.last().unwrap() + 1 - indices.len();
    ed.transaction(
        format!("Agrupar {}", plural(refs.len(), "objeto", "objetos")),
        |ed| match &c {
            Container::Layer { page, layer } => {
                for &i in indices.iter().rev() {
                    ed.remove_object(ObjLoc {
                        page: *page,
                        layer: *layer,
                        index: i,
                    });
                }
                ed.insert_object(
                    ObjLoc {
                        page: *page,
                        layer: *layer,
                        index: at,
                    },
                    g,
                );
            }
            Container::Group(parent) => ed.edit_children(parent, |list| {
                for &i in indices.iter().rev() {
                    list.remove(i);
                }
                list.insert(at, g);
            }),
        },
    );
    Some(id)
}

/// Desfaz os grupos selecionados: os filhos ocupam o lugar do grupo, com a
/// matriz e a opacidade do grupo incorporadas. Devolve os IDs dos filhos
/// (os objetos selecionados que não são grupos continuam selecionados).
pub fn ungroup(ed: &mut Editor, sel: &[String]) -> Selection {
    let mut out = Vec::new();
    let mut any = false;
    for id in sel {
        match ed.find(id).and_then(|r| ed.get(&r)).map(|o| &o.kind) {
            Some(ObjectKind::Group(_)) => any = true,
            Some(_) => out.push(id.clone()),
            None => {}
        }
    }
    if !any {
        return out;
    }
    ed.transaction("Desagrupar", |ed| {
        for id in sel {
            let Some(r) = ed.find(id) else { continue };
            let g = ed.get(&r).unwrap().clone();
            let ObjectKind::Group(data) = g.kind else {
                continue;
            };
            let gm = g.transform.affine();
            let source = if data.effect.is_some() {
                super::effects::baked(ed, &data, gm)
            } else {
                super::symmetry::baked(&data)
            };
            let children: Vec<Object> = source
                .into_iter()
                .map(|mut ch| {
                    ch.transform = Transform::from_affine(gm * ch.transform.affine());
                    if let Some(go) = g.opacity {
                        ch.opacity = Some(ch.opacity.unwrap_or(1.0) * go);
                    }
                    if !g.visible {
                        ch.visible = false;
                    }
                    ch
                })
                .collect();
            out.extend(children.iter().map(|c| c.id.clone()));
            let i = r.index();
            match r.container() {
                Container::Layer { page, layer } => {
                    ed.remove_object(r.loc);
                    for (k, ch) in children.into_iter().enumerate() {
                        ed.insert_object(
                            ObjLoc {
                                page,
                                layer,
                                index: i + k,
                            },
                            ch,
                        );
                    }
                }
                Container::Group(parent) => ed.edit_children(&parent, |list| {
                    list.splice(i..=i, children);
                }),
            }
        }
    });
    out
}

// ---- criar, excluir, duplicar, atributos -------------------------------------

fn fresh_copy(o: &Object) -> Object {
    let mut c = o.clone();
    renew_ids(&mut c);
    c
}

/// Dá IDs novos ao objeto e a todos os descendentes (cópias não podem
/// repetir IDs, ou a busca por ID acharia o original).
pub fn renew_ids(o: &mut Object) {
    o.id = crate::model::new_id();
    if let ObjectKind::Group(g) = &mut o.kind {
        for c in &mut g.children {
            renew_ids(c);
        }
    }
}

/// Adiciona um objeto no topo da camada; devolve seu ID.
/// Grade imprimível (GD-GRD-01): camada nova no fundo da página com as
/// linhas da grade de documento (`spacing`) e da linha base (`baseline` =
/// entrelinha e início a partir do topo), em fio de 0,1 mm cinza.
pub fn printable_grid(
    ed: &mut Editor,
    page: usize,
    w: f64,
    h: f64,
    spacing: Option<f64>,
    baseline: Option<(f64, f64)>,
    name: &str,
) -> Option<String> {
    use crate::model::{Color, FillRule, Layer, Node, PathData, Stroke, Subpath};
    let line = |a: (f64, f64), b: (f64, f64)| Subpath {
        nodes: vec![Node::corner(a.0, a.1), Node::corner(b.0, b.1)],
        closed: false,
        extra: Default::default(),
    };
    let mut objs = Vec::new();
    let mut push = |subpaths: Vec<Subpath>, gray: f64, label: &str| {
        if subpaths.is_empty() {
            return;
        }
        let mut o = Object::new(ObjectKind::Path(PathData {
            subpaths,
            fill_rule: FillRule::Nonzero,
            extra: Default::default(),
        }));
        o.stroke = Some(Stroke::new(100.0, Color::cmyk(0.0, 0.0, 0.0, gray)));
        o.name = Some(label.into());
        objs.push(o);
    };
    if let Some(g) = spacing.filter(|g| *g > 0.0 && w / g < 5_000.0 && h / g < 5_000.0) {
        let mut v = Vec::new();
        let mut x = 0.0;
        while x <= w + 1e-6 {
            v.push(line((x, 0.0), (x, h)));
            x += g;
        }
        let mut y = 0.0;
        while y <= h + 1e-6 {
            v.push(line((0.0, y), (w, y)));
            y += g;
        }
        push(v, 0.3, name);
    }
    if let Some((b, start)) = baseline.filter(|(b, _)| *b > 0.0 && h / *b < 5_000.0) {
        let mut v = Vec::new();
        let mut y = h - start;
        while y >= -1e-6 {
            v.push(line((0.0, y), (w, y)));
            y -= b;
        }
        push(v, 0.5, "Linha base");
    }
    if objs.is_empty() {
        return None;
    }
    let layer = Layer::new(name);
    let lid = layer.id.clone();
    ed.transaction(format!("Criar {name}"), |ed| {
        ed.insert_layer(page, 0, layer);
        for (i, o) in objs.into_iter().enumerate() {
            ed.insert_object(
                ObjLoc {
                    page,
                    layer: 0,
                    index: i,
                },
                o,
            );
        }
    });
    Some(lid)
}

pub fn add_object(ed: &mut Editor, page: usize, layer: usize, o: Object, name: &str) -> String {
    let id = o.id.clone();
    let index = ed.objects(page, layer).len();
    ed.transaction(name, |ed| {
        ed.insert_object(ObjLoc { page, layer, index }, o)
    });
    id
}

pub fn delete(ed: &mut Editor, sel: &[String]) {
    let n = refs(ed, sel).len();
    if n == 0 {
        return;
    }
    ed.transaction(
        format!("Excluir {}", plural(n, "objeto", "objetos")),
        |ed| {
            // Cada remoção muda índices; por isso cada ID é localizado de novo.
            for id in sel {
                if let Some(r) = ed.find(id) {
                    ed.remove_at(&r);
                }
            }
        },
    );
}

/// Move objetos de primeiro nível (ou tirados de grupos) para o topo de
/// outra camada da mesma página, mantendo a ordem relativa (GD-CAM-03).
/// Converter em guia (GD-GUI-03): leva a seleção para a camada de guias da
/// página (criada no topo se faltar). Devolve o índice da camada; um passo.
pub fn to_guides(ed: &mut Editor, sel: &[String], page: usize) -> Option<usize> {
    if sel.is_empty() || page >= ed.doc.data.pages.len() {
        return None;
    }
    let mut out = None;
    ed.transaction("Converter em guia", |ed| {
        let idx = match ed.layers(page).iter().position(|l| l.guides) {
            Some(i) => i,
            None => {
                let mut l = crate::model::Layer::new("Guias");
                l.guides = true;
                l.printable = false;
                let at = ed.layers(page).len();
                ed.insert_layer(page, at, l);
                at
            }
        };
        move_to_layer(ed, sel, page, idx);
        out = Some(idx);
    });
    out
}

pub fn move_to_layer(ed: &mut Editor, sel: &[String], page: usize, layer: usize) {
    let mut refs: Vec<ObjRef> = refs(ed, sel)
        .into_iter()
        .filter(|r| r.loc.page == page && !(r.is_top() && r.loc.layer == layer))
        .collect();
    if refs.is_empty() {
        return;
    }
    refs.sort();
    let moved: Vec<(String, Affine)> = refs
        .iter()
        .map(|r| (ed.get(r).unwrap().id.clone(), ed.parent_affine(r)))
        .collect();
    ed.transaction(
        format!(
            "Mover {} para a camada",
            plural(moved.len(), "objeto", "objetos")
        ),
        |ed| {
            for (id, parent) in moved {
                let Some(r) = ed.find(&id) else { continue };
                let Some(mut o) = ed.remove_at(&r) else {
                    continue;
                };
                o.transform = Transform::from_affine(parent * o.transform.affine());
                let index = ed.objects(page, layer).len();
                ed.insert_object(ObjLoc { page, layer, index }, o);
            }
        },
    );
}

/// Gradiente pedido pela interface (GD-FIL-03): tipo, paradas, ângulo
/// (linear, graus a partir de +X) e repetição; a geometria (início/fim ou
/// centro/raio) é calculada pela caixa de cada objeto.
#[derive(Debug, Clone, PartialEq)]
pub struct GradientSpec {
    /// `linear`, `radial`, `elliptic`, `conic` ou `rectangular`.
    pub kind: String,
    pub stops: Vec<crate::model::GradientStop>,
    pub angle: f64,
    /// `pad`, `repeat` ou `reflect`.
    pub spread: String,
    /// Deslocamento do centro em fração da meia caixa (−1 a 1 = borda).
    pub center: [f64; 2],
    /// Tamanho em relação ao que cobre a caixa (1 = 100%).
    pub scale: f64,
    pub steps: Option<u32>,
    pub smooth: bool,
    pub color_path: String,
}

impl Default for GradientSpec {
    fn default() -> Self {
        GradientSpec {
            kind: "linear".into(),
            stops: Vec::new(),
            angle: 0.0,
            spread: "pad".into(),
            center: [0.0, 0.0],
            scale: 1.0,
            steps: None,
            smooth: false,
            color_path: "direct".into(),
        }
    }
}

/// Gradiente com a geometria ajustada à caixa `b` (coordenadas locais).
pub fn gradient_for(spec: &GradientSpec, b: Rect) -> crate::model::Gradient {
    let s = spec.scale.max(1e-6);
    let c = b.center()
        + Vec2::new(
            spec.center[0] * b.width() / 2.0,
            spec.center[1] * b.height() / 2.0,
        );
    let (mut radius_y, mut angle) = (None, None);
    let (start, end, center, radius) = if spec.kind == "radial" {
        let r = (b.width().hypot(b.height()) / 2.0 * s).max(1e-9);
        (None, None, Some([c.x, c.y]), Some(r))
    } else if matches!(spec.kind.as_str(), "elliptic" | "rectangular" | "conic") {
        // Elíptico chega aos cantos; retangular às bordas; cônico só usa o
        // ângulo.
        let k = if spec.kind == "elliptic" {
            std::f64::consts::SQRT_2
        } else {
            1.0
        };
        radius_y = Some((b.height() / 2.0 * k * s).max(1e-9));
        angle = (spec.angle != 0.0).then_some(spec.angle);
        (
            None,
            None,
            Some([c.x, c.y]),
            Some((b.width() / 2.0 * k * s).max(1e-9)),
        )
    } else {
        let a = spec.angle.to_radians();
        let d = Vec2::new(a.cos(), a.sin());
        let half = ((b.width() / 2.0 * a.cos()).abs() + (b.height() / 2.0 * a.sin()).abs()) * s;
        let (p0, p1) = (c - d * half, c + d * half);
        (Some([p0.x, p0.y]), Some([p1.x, p1.y]), None, None)
    };
    crate::model::Gradient {
        kind: spec.kind.clone(),
        stops: spec.stops.clone(),
        start,
        end,
        center,
        radius,
        radius_y,
        angle,
        spread: spec.spread.clone(),
        steps: spec.steps,
        smooth: spec.smooth,
        color_path: spec.color_path.clone(),
        extra: Default::default(),
    }
}

/// Lê de volta o pedido de um gradiente existente (ângulo pelo início/fim).
pub fn gradient_spec(g: &crate::model::Gradient) -> GradientSpec {
    gradient_spec_in(g, None)
}

/// Como [`gradient_spec`]; com a caixa local `b` do objeto, recupera
/// também o deslocamento do centro e a escala.
pub fn gradient_spec_in(g: &crate::model::Gradient, b: Option<Rect>) -> GradientSpec {
    let angle = match (g.start, g.end) {
        (Some(a), Some(b)) => (b[1] - a[1]).atan2(b[0] - a[0]).to_degrees(),
        _ => g.angle.unwrap_or(0.0),
    };
    let kind: String = match g.kind.as_str() {
        k @ ("radial" | "elliptic" | "conic" | "rectangular") => k.into(),
        _ => "linear".into(),
    };
    let (mut center, mut scale) = ([0.0, 0.0], 1.0);
    if let Some(b) = b.filter(|b| b.width() > 0.0 && b.height() > 0.0) {
        let bc = b.center();
        let (hw, hh) = (b.width() / 2.0, b.height() / 2.0);
        let mid = match (g.start, g.end, g.center) {
            (Some(a), Some(e), _) => Some(Point::new((a[0] + e[0]) / 2.0, (a[1] + e[1]) / 2.0)),
            (_, _, Some(c)) => Some(Point::new(c[0], c[1])),
            _ => None,
        };
        if let Some(m) = mid {
            center = [(m.x - bc.x) / hw, (m.y - bc.y) / hh];
        }
        let def = gradient_for(
            &GradientSpec {
                kind: kind.clone(),
                angle,
                ..Default::default()
            },
            b,
        );
        scale = match (g.start, g.end, def.start, def.end) {
            (Some(a), Some(e), Some(da), Some(de)) => {
                let len = |p: [f64; 2], q: [f64; 2]| (q[0] - p[0]).hypot(q[1] - p[1]);
                len(a, e) / len(da, de).max(1e-9)
            }
            _ => g.radius.unwrap_or(1.0) / def.radius.unwrap_or(1.0).max(1e-9),
        };
    }
    GradientSpec {
        kind,
        stops: g.stops.clone(),
        angle,
        spread: g.spread.clone(),
        center,
        scale,
        steps: g.steps,
        smooth: g.smooth,
        color_path: g.color_path.clone(),
    }
}

/// Aplica o gradiente à seleção, cada objeto (ou folha de grupo) com a
/// geometria pela própria caixa local. Um passo no histórico.
pub fn set_gradient(ed: &mut Editor, sel: &[String], spec: &GradientSpec) {
    modify(ed, sel, "Aplicar gradiente", |o| {
        for_leaves(o, &mut |o| {
            if matches!(o.kind, ObjectKind::Image(_)) {
                return false;
            }
            let Some(b) = super::query::local_outline(o).map(|p| kurbo::Shape::bounding_box(&p))
            else {
                return false;
            };
            o.fill = Some(Fill::Gradient(gradient_for(spec, b)));
            true
        })
    });
}

/// Distribuir (GD-ALI-03): por qual referência os objetos ficam com
/// espaçamento igual.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Distribute {
    Left,
    CenterH,
    Right,
    /// Vãos iguais entre as caixas, na horizontal.
    SpaceH,
    Top,
    CenterV,
    Bottom,
    /// Vãos iguais entre as caixas, na vertical.
    SpaceV,
}

impl Distribute {
    fn horizontal(self) -> bool {
        matches!(
            self,
            Distribute::Left | Distribute::CenterH | Distribute::Right | Distribute::SpaceH
        )
    }
}

/// Distribui a seleção (GD-ALI-03). `within`: `None` = na extensão da
/// própria seleção (os extremos ficam onde estão); `Some(r)` = na
/// extensão de `r` (ex.: a página). `gap`: espaçamento fixo (µm) entre
/// as referências (ou entre as caixas, nos modos de vão), começando pelo
/// primeiro objeto. Um passo no histórico.
pub fn distribute(
    ed: &mut Editor,
    sel: &[String],
    how: Distribute,
    within: Option<Rect>,
    gap: Option<f64>,
    visual: bool,
) {
    let mut objs: Vec<(String, Rect)> = sel
        .iter()
        .filter_map(|id| Some((id.clone(), object_bounds(ed, &ed.find(id)?, visual)?)))
        .collect();
    let min = if within.is_some() || gap.is_some() {
        2
    } else {
        3
    };
    if objs.len() < min {
        return;
    }
    let h = how.horizontal();
    // Referência de cada caixa no eixo e o tamanho dela.
    let key = |b: &Rect| -> f64 {
        match how {
            Distribute::Left => b.x0,
            Distribute::CenterH => b.center().x,
            Distribute::Right => b.x1,
            Distribute::SpaceH => b.x0,
            Distribute::Bottom => b.y0,
            Distribute::CenterV => b.center().y,
            Distribute::Top => b.y1,
            Distribute::SpaceV => b.y0,
        }
    };
    let size = |b: &Rect| if h { b.width() } else { b.height() };
    objs.sort_by(|a, b| key(&a.1).total_cmp(&key(&b.1)));
    let n = objs.len();
    let space = matches!(how, Distribute::SpaceH | Distribute::SpaceV);
    // Posições-alvo da referência de cada caixa.
    let targets: Vec<f64> = if space {
        let total: f64 = objs.iter().map(|o| size(&o.1)).sum();
        let (start, end) = match within {
            Some(r) => {
                if h {
                    (r.x0, r.x1)
                } else {
                    (r.y0, r.y1)
                }
            }
            None => {
                let first = &objs[0].1;
                let last = &objs[n - 1].1;
                if h {
                    (first.x0, last.x1)
                } else {
                    (first.y0, last.y1)
                }
            }
        };
        let g = gap.unwrap_or((end - start - total) / (n as f64 - 1.0));
        let mut at = start;
        objs.iter()
            .map(|o| {
                let t = at;
                at += size(&o.1) + g;
                t
            })
            .collect()
    } else {
        let (start, end) = match within {
            Some(r) => {
                // Na página: as referências vão das bordas (descontado o
                // tamanho dos extremos) às bordas.
                let (lo, hi) = if h { (r.x0, r.x1) } else { (r.y0, r.y1) };
                let first = size(&objs[0].1);
                let last = size(&objs[n - 1].1);
                match how {
                    Distribute::Left | Distribute::Bottom => (lo, hi - last),
                    Distribute::Right | Distribute::Top => (lo + first, hi),
                    _ => (lo + first / 2.0, hi - last / 2.0),
                }
            }
            None => (key(&objs[0].1), key(&objs[n - 1].1)),
        };
        let step = gap.unwrap_or((end - start) / (n as f64 - 1.0));
        (0..n).map(|i| start + step * i as f64).collect()
    };
    let name = if h {
        "Distribuir na horizontal"
    } else {
        "Distribuir na vertical"
    };
    ed.transaction(name, |ed| {
        for ((id, b), t) in objs.iter().zip(targets) {
            let now = if space {
                if h { b.x0 } else { b.y0 }
            } else {
                key(b)
            };
            let d = t - now;
            if d.abs() > 1e-9 {
                let v = if h {
                    Vec2::new(d, 0.0)
                } else {
                    Vec2::new(0.0, d)
                };
                let r = ed.find(id).unwrap();
                let mut o = ed.get(&r).unwrap().clone();
                o.transform = page_transform(ed, &r, &o, Affine::translate(v));
                ed.replace_at(&r, o);
            }
        }
    });
}

/// Destino de um objeto arrastado no painel de camadas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DropAt {
    /// Logo acima (na frente) do objeto com este ID, no mesmo contêiner.
    Above(String),
    /// Logo abaixo (atrás) do objeto com este ID, no mesmo contêiner.
    Below(String),
    /// No topo da camada.
    LayerTop { page: usize, layer: usize },
}

/// Move um objeto para outra posição de empilhamento, outra camada ou para
/// dentro/fora de um grupo, mantendo a aparência na página (GD-CAM-03).
/// Um passo no histórico. Devolve `false` quando não há o que fazer (o
/// destino é o próprio objeto ou está dentro dele).
pub fn move_to(ed: &mut Editor, id: &str, at: &DropAt) -> bool {
    let Some(r) = ed.find(id) else {
        return false;
    };
    let target = match at {
        DropAt::Above(t) | DropAt::Below(t) => Some(t.as_str()),
        DropAt::LayerTop { .. } => None,
    };
    if let Some(t) = target {
        match ed.find(t) {
            // O destino não pode ser o próprio objeto nem um descendente.
            Some(tr) if t != id && !tr.is_within(&r) => {}
            _ => return false,
        }
    }
    let origin = r.loc;
    let parent = ed.parent_affine(&r);
    ed.transaction("Mover objeto", |ed| {
        let Some(mut o) = ed.remove_at(&r) else {
            return;
        };
        let page_m = parent * o.transform.affine();
        // Posição calculada depois de remover: os índices já estão certos.
        let place = match at {
            DropAt::LayerTop { page, layer } => {
                let c = Container::Layer {
                    page: *page,
                    layer: *layer,
                };
                Some((c, ed.objects(*page, *layer).len()))
            }
            DropAt::Above(t) | DropAt::Below(t) => ed.find(t).map(|tr| {
                let c = tr.container();
                let i = tr.index() + usize::from(matches!(at, DropAt::Above(_)));
                (c, i)
            }),
        };
        // Destino sumiu (era o grupo que ficou vazio): topo da camada de origem.
        let (c, index) = place.unwrap_or_else(|| {
            let c = Container::Layer {
                page: origin.page,
                layer: origin.layer,
            };
            let n = ed.objects(origin.page, origin.layer).len();
            (c, n)
        });
        let dest_m = match &c {
            Container::Layer { .. } => Affine::IDENTITY,
            Container::Group(g) => {
                ed.parent_affine(g) * ed.get(g).map_or(Affine::IDENTITY, |g| g.transform.affine())
            }
        };
        let local = dest_m.inverse() * page_m;
        if local != o.transform.affine() {
            o.transform = Transform::from_affine(local);
        }
        ed.insert_at(&c, index, o);
    });
    true
}

/// Move ou copia a seleção para o topo da camada `layer` de outra página
/// (GD-CAM-03), na mesma posição (as páginas têm o mesmo tamanho). Objetos
/// de dentro de grupos saem com a transformação na página. Devolve os IDs
/// na página de destino (as cópias ganham IDs novos). Um passo.
pub fn move_to_page(
    ed: &mut Editor,
    sel: &[String],
    page: usize,
    layer: usize,
    copy: bool,
) -> Vec<String> {
    let mut refs: Vec<ObjRef> = refs(ed, sel);
    refs.sort();
    let items: Vec<(String, Object, Affine)> = refs
        .iter()
        .filter_map(|r| {
            Some((
                ed.get(r)?.id.clone(),
                ed.get(r)?.clone(),
                ed.parent_affine(r),
            ))
        })
        .collect();
    if items.is_empty() || page >= ed.doc.data.pages.len() {
        return Vec::new();
    }
    let mut ids = Vec::new();
    let name = if copy {
        "Copiar para a página"
    } else {
        "Mover para a página"
    };
    ed.transaction(name, |ed| {
        for (id, mut o, parent) in items {
            if copy {
                renew_ids(&mut o);
            } else if let Some(r) = ed.find(&id) {
                ed.remove_at(&r);
            }
            o.transform = Transform::from_affine(parent * o.transform.affine());
            let layer = layer.min(ed.layers(page).len().saturating_sub(1));
            let index = ed.objects(page, layer).len();
            ids.push(o.id.clone());
            ed.insert_object(ObjLoc { page, layer, index }, o);
        }
    });
    ids
}

/// Duplica com deslocamento (GD-DUP-01); devolve os IDs das cópias.
pub fn duplicate(ed: &mut Editor, sel: &[String], offset: Vec2) -> Selection {
    transform_copies(ed, sel, Affine::translate(offset), 1)
}

/// Substitui objetos aplicando `f` a cada um (atributos, conversões).
pub fn modify(ed: &mut Editor, sel: &[String], name: &str, mut f: impl FnMut(&mut Object) -> bool) {
    let refs = refs(ed, sel);
    ed.transaction(name, |ed| {
        for r in refs {
            let mut o = ed.get(&r).unwrap().clone();
            if f(&mut o) {
                ed.replace_at(&r, o);
            }
        }
    });
}

/// Aplica `f` ao objeto ou, num grupo, a todos os descendentes que não são
/// grupos (atributos aplicados a um grupo valem para o conteúdo).
fn for_leaves(o: &mut Object, f: &mut dyn FnMut(&mut Object) -> bool) -> bool {
    match &mut o.kind {
        ObjectKind::Group(g) => {
            let mut changed = false;
            for c in &mut g.children {
                changed |= for_leaves(c, f);
            }
            changed
        }
        _ => f(o),
    }
}

/// Preenchimento uniforme ou nenhum (GD-FIL-01/02).
pub fn set_fill(ed: &mut Editor, sel: &[String], fill: Option<Fill>) {
    modify(ed, sel, "Alterar preenchimento", |o| {
        for_leaves(o, &mut |o| {
            if matches!(o.kind, ObjectKind::Image(_)) {
                return false;
            }
            o.fill = fill.clone();
            true
        })
    });
}

/// Contorno (GD-OUT-01..04); `None` remove.
pub fn set_stroke(ed: &mut Editor, sel: &[String], stroke: Option<Stroke>) {
    modify(ed, sel, "Alterar contorno", |o| {
        for_leaves(o, &mut |o| {
            if matches!(o.kind, ObjectKind::Image(_)) {
                return false;
            }
            o.stroke = stroke.clone();
            true
        })
    });
}

/// Altera só alguns campos do contorno existente (cria um padrão se não houver).
pub fn edit_stroke(ed: &mut Editor, sel: &[String], f: impl Fn(&mut Stroke)) {
    modify(ed, sel, "Alterar contorno", |o| {
        for_leaves(o, &mut |o| {
            if matches!(o.kind, ObjectKind::Image(_)) {
                return false;
            }
            let mut s = o
                .stroke
                .clone()
                .unwrap_or_else(|| Stroke::new(0.0, crate::model::Color::black()));
            f(&mut s);
            o.stroke = Some(s);
            true
        })
    });
}

/// Converte o contorno em objeto (GD-OUT-09): gera um caminho fechado com
/// preenchimento na cor do contorno, logo acima do original (abaixo, se o
/// contorno estava atrás do preenchimento). O original perde o contorno; se
/// não tinha preenchimento, é substituído. Devolve os IDs dos objetos novos.
///
/// Limitação: contorno interno/externo é convertido como centrado (falta a
/// operação booleana para recortar).
pub fn outline_to_object(ed: &mut Editor, sel: &[String]) -> Selection {
    use crate::model::{FillRule, PathData};
    let mut out = Vec::new();
    let targets: Vec<String> = sel
        .iter()
        .filter(|id| {
            ed.find(id)
                .and_then(|r| ed.get(&r))
                .is_some_and(|o| o.stroke.is_some() && super::query::local_outline(o).is_some())
        })
        .cloned()
        .collect();
    if targets.is_empty() {
        return out;
    }
    ed.transaction(
        format!(
            "Converter contorno em objeto ({})",
            plural(targets.len(), "objeto", "objetos")
        ),
        |ed| {
            for id in &targets {
                let Some(r) = ed.find(id) else { continue };
                let o = ed.get(&r).unwrap().clone();
                let (Some(st), Some(local)) = (o.stroke.clone(), super::query::local_outline(&o))
                else {
                    continue;
                };
                // Espessura no espaço local do objeto.
                let m = ed.parent_affine(&r) * o.transform.affine();
                let scale = m.determinant().abs().sqrt().max(1e-12);
                let page_w = if st.width <= 0.0 {
                    crate::model::paint::HAIRLINE_UM
                } else if st.scale_with_object {
                    st.width * scale
                } else {
                    st.width
                };
                let w = page_w / scale;
                let cap = match st.cap.as_str() {
                    "round" => kurbo::Cap::Round,
                    "square" => kurbo::Cap::Square,
                    _ => kurbo::Cap::Butt,
                };
                let join = match st.join.as_str() {
                    "round" => kurbo::Join::Round,
                    "bevel" => kurbo::Join::Bevel,
                    _ => kurbo::Join::Miter,
                };
                let mut ks = kurbo::Stroke::new(w)
                    .with_caps(cap)
                    .with_join(join)
                    .with_miter_limit(st.miter_limit);
                if let Some(d) = st.dash.as_ref().filter(|d| d.iter().any(|v| *v > 0.0)) {
                    let k = if st.scale_with_object {
                        1.0
                    } else {
                        1.0 / scale
                    };
                    ks = ks.with_dashes(st.dash_offset * k, d.iter().map(|v| v * k));
                }
                let tol = (w / 1000.0).max(0.01);
                let shape = if st.widths.is_empty() {
                    close_all(&kurbo::stroke(
                        local,
                        &ks,
                        &kurbo::StrokeOpts::default(),
                        tol,
                    ))
                } else {
                    crate::render::varwidth::outline(&local, w, &st.widths)
                };
                let mut obj = Object::new(ObjectKind::Path(PathData {
                    subpaths: super::convert::bez_to_subpaths(&shape),
                    fill_rule: FillRule::Nonzero,
                    extra: Default::default(),
                }));
                obj.transform = o.transform;
                obj.fill = Some(Fill::Solid {
                    color: st.color.clone(),
                    opacity: st.opacity,
                    overprint: false,
                    extra: Default::default(),
                });
                obj.opacity = o.opacity;
                obj.name = o.name.as_ref().map(|n| format!("{n} (contorno)"));
                out.push(obj.id.clone());
                let filled = !matches!(o.fill, None | Some(Fill::None));
                let c = r.container();
                if filled {
                    let mut base = o.clone();
                    base.stroke = None;
                    ed.replace_at(&r, base);
                    let at = if st.behind_fill {
                        r.index()
                    } else {
                        r.index() + 1
                    };
                    ed.insert_at(&c, at, obj);
                } else {
                    ed.replace_at(&r, obj);
                }
            }
        },
    );
    out
}

/// Fecha todos os subcaminhos (a expansão de traço devolve contornos que
/// voltam ao ponto inicial sem o comando de fechar).
fn close_all(path: &kurbo::BezPath) -> kurbo::BezPath {
    use kurbo::PathEl;
    let mut out = kurbo::BezPath::new();
    let mut open = false;
    for el in path.elements() {
        match el {
            PathEl::MoveTo(_) => {
                if open {
                    out.close_path();
                }
                open = true;
                out.push(*el);
            }
            PathEl::ClosePath => {
                if open {
                    out.close_path();
                }
                open = false;
            }
            other => out.push(*other),
        }
    }
    if open {
        out.close_path();
    }
    out
}

/// Converte primitivas em curvas (GD-CNV-01, subconjunto).
pub fn convert_to_curves(ed: &mut Editor, sel: &[String]) {
    modify(ed, sel, "Converter em curvas", |o| {
        for_leaves(o, &mut |o| match super::convert::to_curves(o) {
            Some(c) => {
                *o = c;
                true
            }
            None => false,
        })
    });
}

/// Ajustar texto ao caminho (GD-TXT-09): o texto passa a correr sobre o
/// contorno de `path_id` (copiado para o espaço local do texto; o objeto
/// do caminho continua). Texto de parágrafo vira artístico.
pub fn fit_text_to_path(ed: &mut Editor, text_id: &str, path_id: &str) -> bool {
    let (Some(rt), Some(rp)) = (ed.find(text_id), ed.find(path_id)) else {
        return false;
    };
    let (Some(t), Some(po)) = (ed.get(&rt).cloned(), ed.get(&rp).cloned()) else {
        return false;
    };
    if !matches!(t.kind, ObjectKind::Text(_))
        || matches!(po.kind, ObjectKind::Text(_) | ObjectKind::Group(_))
    {
        return false;
    }
    let Some(mut outline) = super::query::local_outline(&po) else {
        return false;
    };
    let to_page = ed.parent_affine(&rp) * po.transform.affine();
    let text_m = ed.parent_affine(&rt) * t.transform.affine();
    outline.apply_affine(text_m.inverse() * to_page);
    let subpaths = super::convert::bez_to_subpaths(&outline);
    let ids = [text_id.to_string()];
    modify(ed, &ids, "Ajustar texto ao caminho", |o| {
        let ObjectKind::Text(td) = &mut o.kind else {
            return false;
        };
        td.text_kind = "artistic".into();
        td.frame = None;
        td.path = Some(crate::model::TextPath {
            subpaths: subpaths.clone(),
            offset: 0.0,
            distance: 0.0,
            side: "above".into(),
            orientation: "rotate".into(),
            mirror: false,
            guide: Some(path_id.to_string()),
            detached: false,
            extra: Default::default(),
        });
        true
    });
    true
}

/// Separar o texto do caminho: volta a ser texto em linha reta.
pub fn detach_text_path(ed: &mut Editor, sel: &[String]) {
    modify(ed, sel, "Separar texto do caminho", |o| match &mut o.kind {
        ObjectKind::Text(t) if t.path.is_some() => {
            t.path = None;
            true
        }
        _ => false,
    });
}

/// Muda as opções do texto em caminho (posição, distância, lado,
/// orientação, espelho).
pub fn edit_text_path(ed: &mut Editor, sel: &[String], f: impl Fn(&mut crate::model::TextPath)) {
    modify(ed, sel, "Texto em caminho", |o| match &mut o.kind {
        ObjectKind::Text(t) => match &mut t.path {
            Some(p) => {
                let before = p.clone();
                f(p);
                *p != before
            }
            None => false,
        },
        _ => false,
    });
}

/// Modo de mesclagem da seleção (GD-TRA-02).
pub fn set_blend(ed: &mut Editor, sel: &[String], mode: crate::model::BlendMode) {
    modify(ed, sel, "Modo de mesclagem", |o| {
        let changed = o.blend != mode;
        o.blend = mode;
        changed
    });
}

/// Opacidade do objeto inteiro (0–1); 1 tira o campo.
pub fn set_opacity(ed: &mut Editor, sel: &[String], opacity: f64) {
    let v = opacity.clamp(0.0, 1.0);
    let new = (v < 1.0).then_some(v);
    modify(ed, sel, "Opacidade do objeto", |o| {
        let changed = o.opacity != new;
        o.opacity = new;
        changed
    });
}

/// Dados de usuário do objeto (GD-OBJ-05); mapa vazio tira o campo.
pub fn set_user_data(ed: &mut Editor, id: &str, data: BTreeMap<String, String>) {
    let ids = [id.to_string()];
    modify(ed, &ids, "Dados do objeto", |o| {
        let new = (!data.is_empty()).then(|| data.clone());
        if o.user_data == new {
            return false;
        }
        o.user_data = new;
        true
    });
}

/// Extrair subcaminhos (GD-NOD-08): os subcaminhos `which` saem do caminho
/// `id` e viram um objeto novo logo acima, com os mesmos atributos.
/// Devolve o ID do novo objeto.
pub fn extract_subpaths(ed: &mut Editor, id: &str, which: &[usize]) -> Option<String> {
    let r = ed.find(id)?;
    let o = ed.get(&r)?.clone();
    let ObjectKind::Path(p) = &o.kind else {
        return None;
    };
    let mut keep = Vec::new();
    let mut take = Vec::new();
    for (i, sp) in p.subpaths.iter().enumerate() {
        if which.contains(&i) {
            take.push(sp.clone());
        } else {
            keep.push(sp.clone());
        }
    }
    if take.is_empty() || keep.is_empty() {
        return None;
    }
    let mut rest = o.clone();
    if let ObjectKind::Path(pd) = &mut rest.kind {
        pd.subpaths = keep;
    }
    let mut new = o.clone();
    new.id = crate::model::new_id();
    new.name = None;
    if let ObjectKind::Path(pd) = &mut new.kind {
        pd.subpaths = take;
    }
    // Fora de grupo, vai para a camada com a matriz completa.
    if !r.is_top() {
        new.transform = Transform::from_affine(ed.parent_affine(&r) * o.transform.affine());
    }
    let new_id = new.id.clone();
    let at = r.loc;
    ed.transaction("Extrair subcaminho", |ed| {
        ed.replace_at(&r, rest);
        let index = if r.is_top() {
            at.index + 1
        } else {
            ed.objects(at.page, at.layer).len()
        };
        ed.insert_object(ObjLoc { index, ..at }, new);
    });
    Some(new_id)
}

/// Limpar transformações (GD-TRF-03): primitivas e textos voltam ao tamanho
/// e ao ângulo originais (sem escala, giro nem inclinação), com o centro
/// no mesmo lugar. Caminhos, imagens e grupos não mudam.
pub fn clear_transformations(ed: &mut Editor, sel: &[String]) {
    modify(ed, sel, "Limpar transformações", |o| {
        let primitive = matches!(
            o.kind,
            ObjectKind::Rectangle(_)
                | ObjectKind::Ellipse(_)
                | ObjectKind::Polygon(_)
                | ObjectKind::Text(_)
        );
        if !primitive {
            return false;
        }
        let Some(b) = super::query::local_outline(o).map(|p| kurbo::Shape::bounding_box(&p)) else {
            return false;
        };
        let m = o.transform.affine();
        let c = m * b.center();
        let t = Transform::from_affine(Affine::translate(c.to_vec2() - b.center().to_vec2()));
        if t == o.transform {
            return false;
        }
        o.transform = t;
        true
    });
}

// ---- botão direito (doc 26) ------------------------------------------------------

/// Desagrupar tudo (GD-BDR-42): desfaz todos os níveis de grupo da seleção.
pub fn ungroup_all(ed: &mut Editor, sel: &[String]) -> Selection {
    let mut cur = sel.to_vec();
    ed.transaction("Desagrupar tudo", |ed| {
        for _ in 0..64 {
            let has_group = cur.iter().any(|id| {
                matches!(
                    ed.find(id).and_then(|r| ed.get(&r)).map(|o| &o.kind),
                    Some(ObjectKind::Group(_))
                )
            });
            if !has_group {
                break;
            }
            cur = ungroup(ed, &cur);
        }
    });
    cur
}

/// Travar/destravar ou ocultar/mostrar a seleção (GD-BDR-42).
pub fn set_locked(ed: &mut Editor, sel: &[String], locked: bool) {
    let name = if locked { "Bloquear" } else { "Desbloquear" };
    modify(ed, sel, name, |o| {
        let changed = o.locked != locked;
        o.locked = locked;
        changed
    });
}

pub fn set_visible(ed: &mut Editor, sel: &[String], visible: bool) {
    let name = if visible { "Mostrar" } else { "Ocultar" };
    modify(ed, sel, name, |o| {
        let changed = o.visible != visible;
        o.visible = visible;
        changed
    });
}

/// IDs de todos os objetos da página (inclusive dentro de grupos) que
/// satisfazem `f`.
fn page_ids(ed: &Editor, page: usize, f: impl Fn(&Object) -> bool) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(p) = ed.doc.data.pages.get(page) {
        for l in &p.layers {
            for o in &l.objects {
                o.walk(&mut |o| {
                    if f(o) {
                        out.push(o.id.clone());
                    }
                });
            }
        }
    }
    out
}

/// Mostrar todos (GD-BDR-42/44): objetos ocultos da página voltam a
/// aparecer. Devolve os que voltaram.
pub fn show_all(ed: &mut Editor, page: usize) -> Vec<String> {
    let ids = page_ids(ed, page, |o| !o.visible);
    set_visible(ed, &ids, true);
    ids
}

/// Desbloquear todos (GD-BDR-44).
pub fn unlock_all(ed: &mut Editor, page: usize) -> Vec<String> {
    let ids = page_ids(ed, page, |o| o.locked);
    set_locked(ed, &ids, false);
    ids
}

/// "À frente de…" / "Atrás de…" (GD-BDR-41): a seleção vai logo acima
/// (ou abaixo) de `reference`, mantendo a ordem entre os selecionados.
pub fn place_relative(ed: &mut Editor, sel: &[String], reference: &str, above: bool) {
    let mut refs: Vec<ObjRef> = refs(ed, sel)
        .into_iter()
        .filter(|r| ed.get(r).is_some_and(|o| o.id != reference))
        .collect();
    refs.sort();
    let ids: Vec<String> = refs.iter().map(|r| ed.get(r).unwrap().id.clone()).collect();
    let name = if above { "À frente de" } else { "Atrás de" };
    ed.transaction(name, |ed| {
        if above {
            // De cima para baixo: o último inserido fica logo acima da referência.
            for id in ids.iter().rev() {
                move_to(ed, id, &DropAt::Above(reference.to_string()));
            }
        } else {
            for id in &ids {
                move_to(ed, id, &DropAt::Below(reference.to_string()));
            }
        }
    });
}

/// O que "Copiar propriedades" leva (GD-BDR-27..29).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CopyProps {
    pub fill: bool,
    pub stroke: bool,
    /// Opacidade e modo de mesclagem do objeto.
    pub look: bool,
}

impl CopyProps {
    pub const ALL: CopyProps = CopyProps {
        fill: true,
        stroke: true,
        look: true,
    };
    pub const FILL: CopyProps = CopyProps {
        fill: true,
        stroke: false,
        look: false,
    };
    pub const STROKE: CopyProps = CopyProps {
        fill: false,
        stroke: true,
        look: false,
    };
}

/// Copia propriedades de `src` para `targets` (GD-BDR-35: da seleção
/// arrastada para o alvo); a geometria dos alvos fica.
pub fn copy_properties(ed: &mut Editor, src: &str, targets: &[String], what: CopyProps) -> bool {
    let Some(s) = ed.find(src).and_then(|r| ed.get(&r)).cloned() else {
        return false;
    };
    let name = match (what.fill, what.stroke) {
        (true, false) => "Copiar preenchimento",
        (false, true) => "Copiar contorno",
        _ => "Copiar propriedades",
    };
    modify(ed, targets, name, |o| {
        if o.id == s.id || matches!(o.kind, ObjectKind::Image(_)) {
            return false;
        }
        if what.fill {
            o.fill = s.fill.clone();
        }
        if what.stroke {
            o.stroke = s.stroke.clone();
        }
        if what.look {
            o.opacity = s.opacity;
            o.blend = s.blend;
        }
        true
    });
    true
}

/// Centralizar no alvo (GD-BDR-33): o centro da caixa geométrica da seleção
/// vai para o centro da do alvo.
pub fn center_on(ed: &mut Editor, sel: &[String], target: &str) -> bool {
    let (Some(b), Some(t)) = (
        selection_bounds(ed, sel, false),
        ed.find(target).and_then(|r| object_bounds(ed, &r, false)),
    ) else {
        return false;
    };
    let d = t.center() - b.center();
    transform(ed, sel, Affine::translate(d), "Centralizar no alvo");
    true
}

/// Documento só com a seleção (GD-BDR-81, exportar seleção): uma página do
/// tamanho da caixa visual da seleção, com os objetos na mesma aparência,
/// sem página mestra nem fundo.
pub fn selection_document(ed: &Editor, sel: &[String]) -> Option<crate::model::Document> {
    let mut refs = refs(ed, sel);
    refs.sort();
    let b = selection_bounds(ed, sel, true)?;
    let mut d = ed.doc.clone();
    d.data.master_page.layers.clear();
    let mut page = crate::model::Page::new(b.width().max(1.0), b.height().max(1.0));
    let shift = Affine::translate((-b.x0, -b.y0));
    for r in &refs {
        let mut o = ed.get(r)?.clone();
        o.transform = Transform::from_affine(shift * ed.parent_affine(r) * o.transform.affine());
        page.layers[0].objects.push(o);
    }
    d.data.pages = vec![page];
    Some(d)
}

/// Documento só com a arte (GD-EXP-08, PNG): cada página pedida (vazio =
/// todas) vira uma página do tamanho da caixa visual dos seus objetos
/// visíveis em camadas visíveis, sem a página, a página mestra nem fundo.
/// Páginas sem arte ficam de fora; `None` se nenhuma tiver.
pub fn artwork_document(ed: &Editor, pages: &[usize]) -> Option<crate::model::Document> {
    let all: Vec<usize> = ed.page_indices().collect();
    let pages = if pages.is_empty() { &all[..] } else { pages };
    let mut out = Vec::new();
    for &p in pages {
        let ids: Vec<String> = ed
            .layers(p)
            .iter()
            .filter(|l| l.visible)
            .flat_map(|l| l.objects.iter())
            .filter(|o| o.visible)
            .map(|o| o.id.clone())
            .collect();
        if let Some(mut d) = selection_document(ed, &ids) {
            out.append(&mut d.data.pages);
        }
    }
    if out.is_empty() {
        return None;
    }
    let mut d = ed.doc.clone();
    d.data.master_page.layers.clear();
    d.data.pages = out;
    Some(d)
}

/// Primeiro objeto acima de `at` (na mesma camada) que não está em
/// `removed`: os resultados que substituem os removidos entram logo abaixo
/// dele, mantendo o lugar na pilha mesmo com removidos espalhados.
pub(crate) fn survivor_above(ed: &Editor, at: ObjLoc, removed: &[String]) -> Option<String> {
    ed.objects(at.page, at.layer)
        .iter()
        .skip(at.index + 1)
        .find(|o| !removed.contains(&o.id))
        .map(|o| o.id.clone())
}

/// Índice para inserir logo abaixo de `above` (ou no topo da camada).
pub(crate) fn index_below(ed: &Editor, at: ObjLoc, above: Option<&String>) -> usize {
    above
        .and_then(|id| ed.find(id))
        .filter(|r| r.is_top() && r.loc.layer == at.layer && r.loc.page == at.page)
        .map(|r| r.loc.index)
        .unwrap_or_else(|| ed.objects(at.page, at.layer).len())
}

/// Converter em bitmap (GD-CNV-03): a seleção vira uma imagem PNG a `dpi`,
/// com ou sem anti-serrilhado, fundo transparente (`background = None`)
/// ou de cor, no lugar do objeto mais à frente. Devolve o ID da imagem.
pub fn convert_to_bitmap(
    ed: &mut Editor,
    sel: &[String],
    dpi: f64,
    antialias: bool,
    background: Option<[u8; 4]>,
) -> Option<String> {
    use crate::render::{ImageCache, SceneOptions, build_scene, raster};
    let mut refs = refs(ed, sel);
    if refs.is_empty() {
        return None;
    }
    refs.sort();
    let page_index = refs[0].loc.page;
    let bbox = selection_bounds(ed, sel, true)?;
    // Documento só com a seleção (sem a página mestra).
    let mut d = ed.doc.clone();
    d.data.master_page.layers.clear();
    let mut page = d.data.pages.get(page_index)?.clone();
    for l in &mut page.layers {
        l.objects.clear();
    }
    for r in refs.iter().filter(|r| r.loc.page == page_index) {
        let mut o = ed.get(r)?.clone();
        o.transform = Transform::from_affine(ed.parent_affine(r) * o.transform.affine());
        o.visible = true;
        page.layers.get_mut(r.loc.layer)?.objects.push(o);
    }
    page.background = None;
    for l in &mut page.layers {
        l.visible = true;
        l.wireframe = false;
    }
    let dpi = dpi.clamp(10.0, 2400.0);
    let s = dpi / crate::units::UM_PER_INCH;
    let view = raster::page_view(page.height, dpi);
    let db = view.transform_rect_bbox(bbox);
    let (x0, y0, x1, y1) = (db.x0.floor(), db.y0.floor(), db.x1.ceil(), db.y1.ceil());
    let (w, h) = ((x1 - x0).max(1.0), (y1 - y0).max(1.0));
    if w * h > 200e6 {
        return None;
    }
    let scene = build_scene(
        &d,
        &page,
        SceneOptions { print: false },
        &ImageCache::default(),
    );
    let pm = raster::render(
        &scene,
        Affine::translate((-x0, -y0)) * view,
        w as u32,
        h as u32,
        &raster::RasterOptions {
            antialias,
            background,
        },
    )?;
    let png = raster::encode_png(&pm, Some(dpi)).ok()?;
    let src = ed.doc.add_image(png, "png");
    let mut img = Object::new(ObjectKind::Image(crate::model::ImageData {
        width: w / s,
        height: h / s,
        src,
        extra: Default::default(),
    }));
    img.transform = Transform::translate(x0 / s, page.height - y1 / s);
    img.name = Some("Bitmap".into());
    let id = img.id.clone();
    let front = refs.last()?.loc;
    let remove: Vec<String> = refs
        .iter()
        .filter_map(|r| ed.get(r).map(|o| o.id.clone()))
        .collect();
    ed.transaction("Converter em bitmap", |ed| {
        let above = survivor_above(ed, front, &remove);
        for rid in &remove {
            if let Some(r) = ed.find(rid) {
                ed.remove_at(&r);
            }
        }
        let index = index_below(ed, front, above.as_ref());
        ed.insert_object(ObjLoc { index, ..front }, img);
    });
    Some(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// GD-GRD-01: a grade imprimível é uma camada no fundo com as linhas da
    /// grade (11 + 6 numa página 100 × 50 mm a cada 10 mm) e da linha base.
    #[test]
    fn gd_grd_01_printable_grid() {
        let mut d = crate::model::Document::new();
        d.data
            .pages
            .push(crate::model::Page::new(100_000.0, 50_000.0));
        let mut ed = Editor::new(d);
        printable_grid(
            &mut ed,
            0,
            100_000.0,
            50_000.0,
            Some(10_000.0),
            Some((5_000.0, 0.0)),
            "Grade",
        )
        .unwrap();
        let l = &ed.doc.data.pages[0].layers[0];
        assert_eq!(l.objects.len(), 2);
        let ObjectKind::Path(p) = &l.objects[0].kind else {
            panic!()
        };
        assert_eq!(p.subpaths.len(), 17);
        let ObjectKind::Path(b) = &l.objects[1].kind else {
            panic!()
        };
        assert_eq!(b.subpaths.len(), 11);
        assert_eq!(ed.undo_len(), 1);
    }
    use crate::model::{Color, Document, ObjectKind, Page, RectData};
    use serde_json::Map;

    fn setup() -> (Editor, Selection) {
        let mut d = Document::new();
        d.data.pages.push(Page::new(200.0, 100.0));
        let mut ed = Editor::new(d);
        let mut ids = Vec::new();
        for (x, w) in [(10.0, 10.0), (50.0, 20.0), (100.0, 30.0)] {
            let mut o = Object::new(ObjectKind::Rectangle(RectData {
                width: w,
                height: w,
                corners: None,
                extra: Map::new(),
            }));
            o.transform = Transform::translate(x, x / 2.0);
            ids.push(add_object(&mut ed, 0, 0, o, "Criar retângulo"));
        }
        (ed, ids)
    }

    fn b(ed: &Editor, id: &str) -> Rect {
        bounds(
            ed.object(ed.locate(id).unwrap()).unwrap(),
            Affine::IDENTITY,
            false,
        )
        .unwrap()
    }

    #[test]
    fn gd_trf_01_position_size_rotate_mirror_skew() {
        let (mut ed, ids) = setup();
        let one = vec![ids[0].clone()];
        set_position(&mut ed, &one, Anchor::Center, Point::new(0.0, 0.0));
        assert_eq!(b(&ed, &ids[0]).center(), Point::new(0.0, 0.0));
        set_size(&mut ed, &one, Anchor::BottomLeft, Some(40.0), None, true);
        let r = b(&ed, &ids[0]);
        assert!((r.width() - 40.0).abs() < 1e-9 && (r.height() - 40.0).abs() < 1e-9);
        assert_eq!(
            r.origin(),
            Point::new(-5.0, -5.0),
            "âncora inferior esquerda fixa"
        );
        let c = r.center();
        transform(&mut ed, &one, rotate_about(c, 45.0), "Girar");
        let rr = b(&ed, &ids[0]);
        assert!(
            (rr.width() - 40.0 * 2f64.sqrt()).abs() < 1e-6,
            "quadrado girado 45°"
        );
        assert!((rr.center() - c).hypot() < 1e-9);
        transform(&mut ed, &one, mirror_about(c, true), "Espelhar");
        assert!((b(&ed, &ids[0]).center() - c).hypot() < 1e-9);
        let before = b(&ed, &ids[1]);
        transform(
            &mut ed,
            &[ids[1].clone()],
            skew_about(before.center(), 45.0, 0.0),
            "Inclinar",
        );
        let after = b(&ed, &ids[1]);
        assert!((after.width() - (before.width() + before.height())).abs() < 1e-6);
        assert_eq!(ed.history().last().unwrap().0, "Inclinar 1 objeto");
    }

    #[test]
    fn gd_trf_02_each_and_copies() {
        let (mut ed, ids) = setup();
        transform_each(&mut ed, &ids, true, "Escalar", |r| {
            scale_about(r.center(), 2.0, 2.0)
        });
        assert!((b(&ed, &ids[0]).width() - 20.0).abs() < 1e-9);
        assert_eq!(b(&ed, &ids[0]).center(), Point::new(15.0, 10.0));
        let copies = transform_copies(
            &mut ed,
            &[ids[0].clone()],
            rotate_about(Point::new(0.0, 0.0), 30.0),
            11,
        );
        assert_eq!(copies.len(), 11);
        assert_eq!(ed.objects(0, 0).len(), 14);
        // A 12ª posição (11 cópias + original) fecha a volta de 360°.
        let last = b(&ed, copies.last().unwrap());
        let first = b(&ed, &ids[0]);
        let ang = |r: Rect| r.center().y.atan2(r.center().x).to_degrees();
        assert!(((ang(last) - ang(first) + 360.0) % 360.0 - 330.0).abs() < 1e-6);
        ed.undo();
        assert_eq!(ed.objects(0, 0).len(), 3, "as 11 cópias são um passo");
    }

    #[test]
    fn gd_ali_01_align_relative_to_last_first_page() {
        let (mut ed, ids) = setup();
        align(&mut ed, &ids, Align::Left, AlignTo::LastSelected, false);
        assert!(ids.iter().all(|id| (b(&ed, id).x0 - 100.0).abs() < 1e-9));
        align(&mut ed, &ids, Align::Bottom, AlignTo::FirstSelected, false);
        assert!(ids.iter().all(|id| (b(&ed, id).y0 - 5.0).abs() < 1e-9));
        let page = Rect::new(0.0, 0.0, 200.0, 100.0);
        align(&mut ed, &ids, Align::CenterH, AlignTo::Page(page), false);
        assert!(
            ids.iter()
                .all(|id| (b(&ed, id).center().x - 100.0).abs() < 1e-9)
        );
        align(&mut ed, &ids, Align::Top, AlignTo::Page(page), false);
        assert!(ids.iter().all(|id| (b(&ed, id).y1 - 100.0).abs() < 1e-9));
        ed.undo();
        assert!((b(&ed, &ids[0]).y0 - 5.0).abs() < 1e-9);
    }

    /// GD-ALI-06: um objeto só se alinha pela página; dois ou mais, entre si.
    #[test]
    fn gd_ali_06_single_object_aligns_to_page() {
        let (mut ed, ids) = setup();
        let page = Rect::new(0.0, 0.0, 200.0, 100.0);
        let one = &ids[..1];
        let to = align_target(AlignTo::LastSelected, one.len(), page);
        assert_eq!(to, AlignTo::Page(page));
        align(&mut ed, one, Align::Right, to, false);
        assert!((b(&ed, &ids[0]).x1 - 200.0).abs() < 1e-9);
        align(&mut ed, one, Align::Top, to, false);
        assert!((b(&ed, &ids[0]).y1 - 100.0).abs() < 1e-9);
        assert_eq!(
            align_target(AlignTo::LastSelected, 2, page),
            AlignTo::LastSelected
        );
    }

    #[test]
    fn gd_ali_02_visual_bounds_include_stroke() {
        let (mut ed, ids) = setup();
        set_stroke(
            &mut ed,
            &[ids[0].clone()],
            Some(Stroke::new(4.0, Color::black())),
        );
        align(&mut ed, &ids[..2], Align::Left, AlignTo::LastSelected, true);
        let o = ed.object(ed.locate(&ids[0]).unwrap()).unwrap();
        assert!((bounds(o, Affine::IDENTITY, true).unwrap().x0 - 50.0).abs() < 1e-9);
    }

    #[test]
    fn gd_ord_01_ordering() {
        let (mut ed, ids) = setup();
        let order = |ed: &Editor| {
            ed.objects(0, 0)
                .iter()
                .map(|o| ids.iter().position(|i| *i == o.id).unwrap())
                .collect::<Vec<_>>()
        };
        reorder(&mut ed, &[ids[0].clone()], Order::ToFront);
        assert_eq!(order(&ed), vec![1, 2, 0]);
        reorder(&mut ed, &[ids[0].clone()], Order::ToBack);
        assert_eq!(order(&ed), vec![0, 1, 2]);
        reorder(&mut ed, &[ids[0].clone()], Order::Forward);
        assert_eq!(order(&ed), vec![1, 0, 2]);
        reorder(&mut ed, &[ids[2].clone()], Order::Backward);
        assert_eq!(order(&ed), vec![1, 2, 0]);
        reorder(&mut ed, &ids, Order::Reverse);
        assert_eq!(order(&ed), vec![0, 2, 1]);
        ed.goto(3);
        assert_eq!(order(&ed), vec![0, 1, 2]);
    }

    #[test]
    fn delete_duplicate_and_attributes() {
        let (mut ed, ids) = setup();
        let dup = duplicate(&mut ed, &[ids[1].clone()], Vec2::new(5.0, 5.0));
        assert_eq!(dup.len(), 1);
        assert_ne!(dup[0], ids[1]);
        assert_eq!(
            b(&ed, &dup[0]).origin(),
            b(&ed, &ids[1]).origin() + Vec2::new(5.0, 5.0)
        );
        set_fill(
            &mut ed,
            &ids,
            Some(Fill::solid(Color::cmyk(0.0, 1.0, 0.0, 0.0))),
        );
        edit_stroke(&mut ed, &ids, |s| s.width = 500.0);
        let o = ed.object(ed.locate(&ids[2]).unwrap()).unwrap();
        assert_eq!(o.stroke.as_ref().unwrap().width, 500.0);
        convert_to_curves(&mut ed, &ids);
        assert!(matches!(
            ed.object(ed.locate(&ids[0]).unwrap()).unwrap().kind,
            ObjectKind::Path(_)
        ));
        delete(&mut ed, &ids);
        assert_eq!(ed.objects(0, 0).len(), 1);
        assert_eq!(ed.history().last().unwrap().0, "Excluir 3 objetos");
    }

    /// Aceite de GD-OUT-09: linha de 2 mm com terminação redonda vira caminho
    /// fechado com semicírculos de raio 1 mm nas pontas.
    #[test]
    fn gd_out_09_outline_to_object() {
        use crate::model::{Node, PathData, Subpath};
        let mut d = Document::new();
        d.data.pages.push(Page::new(100_000.0, 100_000.0));
        let mut ed = Editor::new(d);
        let mut line = Object::new(ObjectKind::Path(PathData {
            subpaths: vec![Subpath {
                closed: false,
                nodes: vec![Node::corner(0.0, 0.0), Node::corner(10_000.0, 0.0)],
                extra: Map::new(),
            }],
            fill_rule: Default::default(),
            extra: Map::new(),
        }));
        let mut st = Stroke::new(2_000.0, Color::cmyk(0.0, 1.0, 0.0, 0.0));
        st.cap = "round".into();
        line.stroke = Some(st);
        let id = add_object(&mut ed, 0, 0, line, "Linha");
        let new = outline_to_object(&mut ed, std::slice::from_ref(&id));
        assert_eq!(new.len(), 1);
        assert!(
            ed.find(&id).is_none(),
            "linha sem preenchimento é substituída"
        );
        let o = selected_objects(&ed, &new)[0].clone();
        assert!(o.stroke.is_none());
        assert!(
            matches!(&o.fill, Some(Fill::Solid { color, .. }) if *color == Color::cmyk(0.0, 1.0, 0.0, 0.0))
        );
        let ObjectKind::Path(p) = &o.kind else {
            panic!()
        };
        assert!(p.subpaths.iter().all(|s| s.closed), "caminho fechado");
        let b = bounds(&o, Affine::IDENTITY, false).unwrap();
        assert!(
            (b.x0 + 1_000.0).abs() < 5.0 && (b.x1 - 11_000.0).abs() < 5.0,
            "{b:?}"
        );
        assert!(
            (b.y0 + 1_000.0).abs() < 5.0 && (b.y1 - 1_000.0).abs() < 5.0,
            "{b:?}"
        );
        let area = crate::geometry::signed_area(p).abs();
        let expected = 10_000.0 * 2_000.0 + std::f64::consts::PI * 1_000.0 * 1_000.0;
        assert!(
            (area - expected).abs() / expected < 0.005,
            "{area} vs {expected}"
        );
        ed.undo();
        assert!(ed.find(&id).is_some());
    }

    fn b_any(ed: &Editor, id: &str) -> Rect {
        object_bounds(ed, &ed.find(id).unwrap(), false).unwrap()
    }

    #[test]
    fn group_and_ungroup_keep_geometry() {
        let (mut ed, ids) = setup();
        let before: Vec<Rect> = ids.iter().map(|id| b_any(&ed, id)).collect();
        let g = group(&mut ed, &ids[..2]).unwrap();
        assert_eq!(ed.objects(0, 0).len(), 2);
        assert_eq!(ed.objects(0, 0)[0].id, g, "grupo no lugar do mais à frente");
        assert_eq!(ed.objects(0, 0)[1].id, ids[2]);
        // Mover o grupo e desagrupar: filhos ficam onde o grupo os deixou.
        translate(&mut ed, std::slice::from_ref(&g), Vec2::new(7.0, 0.0));
        let out = ungroup(&mut ed, std::slice::from_ref(&g));
        assert_eq!(out, ids[..2].to_vec());
        assert_eq!(ed.objects(0, 0).len(), 3);
        for k in 0..2 {
            assert_eq!(b_any(&ed, &ids[k]), before[k] + Vec2::new(7.0, 0.0));
        }
        ed.goto(3);
        assert_eq!(ed.objects(0, 0).len(), 3);
        assert_eq!(b_any(&ed, &ids[0]), before[0]);
        assert!(
            group(&mut ed, &ids[..1]).is_none(),
            "um objeto só não agrupa"
        );
    }

    #[test]
    fn gd_sel_02_edit_inside_group() {
        let (mut ed, ids) = setup();
        let g = group(&mut ed, &ids).unwrap();
        // Grupo deslocado e escalado: o filho precisa respeitar a matriz do pai.
        transform(
            &mut ed,
            std::slice::from_ref(&g),
            Affine::translate((100.0, 0.0)) * Affine::scale(2.0),
            "Teste",
        );
        let child = vec![ids[1].clone()];
        let b0 = b_any(&ed, &ids[1]);
        let other = b_any(&ed, &ids[0]);
        translate(&mut ed, &child, Vec2::new(5.0, 3.0));
        let b1 = b_any(&ed, &ids[1]);
        assert!(((b1.origin() - b0.origin()) - Vec2::new(5.0, 3.0)).hypot() < 1e-9);
        assert_eq!(b_any(&ed, &ids[0]), other, "irmão não se mexe");
        set_size(&mut ed, &child, Anchor::BottomLeft, Some(10.0), None, true);
        assert!((b_any(&ed, &ids[1]).width() - 10.0).abs() < 1e-9);

        // Preencher o grupo preenche os filhos; preencher um filho, só ele.
        set_fill(
            &mut ed,
            std::slice::from_ref(&g),
            Some(Fill::solid(Color::black())),
        );
        assert!(selected_objects(&ed, &ids).iter().all(|o| o.fill.is_some()));
        set_fill(&mut ed, &child, None);
        assert!(selected_objects(&ed, &child)[0].fill.is_none());
        assert!(selected_objects(&ed, &ids[..1])[0].fill.is_some());

        // Ordem dentro do grupo.
        reorder(&mut ed, &[ids[0].clone()], Order::ToFront);
        let r = ed.find(&ids[0]).unwrap();
        assert_eq!(r.path, vec![2]);
        assert_eq!(ed.objects(0, 0).len(), 1, "o grupo continua um objeto só");

        // Duplicar um filho cria a cópia dentro do grupo.
        let dup = duplicate(&mut ed, &child, Vec2::new(1.0, 0.0));
        assert_eq!(ed.find(&dup[0]).unwrap().path.len(), 1);

        // Excluir filhos; o último a sair leva o grupo junto.
        delete(&mut ed, &[ids[0].clone(), ids[1].clone(), dup[0].clone()]);
        assert_eq!(ed.find(&g).unwrap().path.len(), 0);
        delete(&mut ed, &[ids[2].clone()]);
        assert!(ed.find(&g).is_none());
        assert!(ed.objects(0, 0).is_empty());
    }

    #[test]
    fn gd_cam_03_move_to_layer() {
        let (mut ed, ids) = setup();
        let g = group(&mut ed, &ids[1..]).unwrap();
        transform(&mut ed, &[g], Affine::translate((0.0, 50.0)), "Mover");
        let before = b_any(&ed, &ids[2]);
        ed.insert_layer(0, 1, crate::model::Layer::new("Camada 2"));
        move_to_layer(&mut ed, &[ids[0].clone(), ids[2].clone()], 0, 1);
        assert_eq!(ed.objects(0, 1).len(), 2);
        let r = ed.find(&ids[2]).unwrap();
        assert!(r.is_top() && r.loc.layer == 1, "saiu do grupo");
        assert_eq!(b_any(&ed, &ids[2]), before, "mesma posição na página");
        ed.undo();
        assert_eq!(ed.objects(0, 1).len(), 0);
        assert_eq!(ed.find(&ids[2]).unwrap().path.len(), 1);
    }

    /// Painel de camadas: arrastar muda a ordem, a camada e entra/sai de
    /// grupos sem mudar a aparência; desfazer volta tudo num passo.
    #[test]
    fn move_to_reorders_changes_layer_and_enters_groups() {
        let (mut ed, ids) = setup();
        let order = |ed: &Editor, l: usize| {
            ed.objects(0, l)
                .iter()
                .map(|o| ids.iter().position(|i| *i == o.id).unwrap_or(99))
                .collect::<Vec<_>>()
        };
        // 0 vai para cima do 2 (topo).
        assert!(move_to(&mut ed, &ids[0], &DropAt::Above(ids[2].clone())));
        assert_eq!(order(&ed, 0), vec![1, 2, 0]);
        // 0 volta para baixo do 1.
        move_to(&mut ed, &ids[0], &DropAt::Below(ids[1].clone()));
        assert_eq!(order(&ed, 0), vec![0, 1, 2]);
        // Outra camada.
        ed.insert_layer(0, 1, crate::model::Layer::new("B"));
        let before = b(&ed, &ids[1]);
        move_to(&mut ed, &ids[1], &DropAt::LayerTop { page: 0, layer: 1 });
        assert_eq!((order(&ed, 0), order(&ed, 1)), (vec![0, 2], vec![1]));
        assert_eq!(b(&ed, &ids[1]), before);
        // Para dentro de um grupo deslocado: a caixa na página não muda.
        let g = group(&mut ed, &[ids[0].clone(), ids[2].clone()]).unwrap();
        transform(
            &mut ed,
            std::slice::from_ref(&g),
            Affine::translate((5.0, 7.0)),
            "Mover",
        );
        let before = b(&ed, &ids[1]);
        move_to(&mut ed, &ids[1], &DropAt::Above(ids[0].clone()));
        let r = ed.find(&ids[1]).unwrap();
        assert_eq!(r.path, vec![1], "entre os filhos do grupo, acima do 0");
        let o = ed.get(&r).unwrap();
        let now = bounds(o, ed.parent_affine(&r), false).unwrap();
        assert!((now.origin() - before.origin()).hypot() < 1e-9);
        assert!((now.width() - before.width()).abs() < 1e-9);
        // Destino dentro do próprio objeto: nada muda.
        assert!(!move_to(&mut ed, &g, &DropAt::Above(ids[0].clone())));
        ed.undo();
        assert_eq!(order(&ed, 1), vec![1]);
    }

    /// GD-ALI-03: distribuir por bordas, centros, vãos iguais e espaçamento
    /// fixo, na extensão da seleção ou da página.
    #[test]
    fn gd_ali_03_distribute() {
        // setup(): x = 10 (w 10), 50 (w 20), 100 (w 30).
        let (mut ed, ids) = setup();
        let lefts = |ed: &Editor| ids.iter().map(|i| b(ed, i).x0).collect::<Vec<_>>();
        distribute(&mut ed, &ids, Distribute::Left, None, None, false);
        assert_eq!(lefts(&ed), vec![10.0, 55.0, 100.0]);
        ed.undo();
        // Vãos iguais: caixas somam 60 entre 10 e 130 → vãos de 30.
        distribute(&mut ed, &ids, Distribute::SpaceH, None, None, false);
        assert_eq!(lefts(&ed), vec![10.0, 50.0, 100.0], "já estavam com vão 30");
        // Espaçamento fixo de 5 entre as caixas, a partir do primeiro.
        distribute(&mut ed, &ids, Distribute::SpaceH, None, Some(5.0), false);
        assert_eq!(lefts(&ed), vec![10.0, 25.0, 50.0]);
        // Centros na largura da página (200): extremos encostam nas bordas.
        distribute(
            &mut ed,
            &ids,
            Distribute::CenterH,
            Some(Rect::new(0.0, 0.0, 200.0, 100.0)),
            None,
            false,
        );
        let c: Vec<f64> = ids.iter().map(|i| b(&ed, i).center().x).collect();
        assert!((c[0] - 5.0).abs() < 1e-9 && (c[2] - 185.0).abs() < 1e-9);
        assert!((c[1] - 95.0).abs() < 1e-9);
        // Com menos de 3 na seleção não há o que distribuir.
        let before = lefts(&ed);
        distribute(&mut ed, &ids[..2], Distribute::Left, None, None, false);
        assert_eq!(lefts(&ed), before);
    }

    /// GD-FIL-03: gradiente linear pelo ângulo atravessa a caixa toda;
    /// radial no centro com raio até o canto; ida e volta do ângulo.
    #[test]
    fn gd_fil_03_gradient_geometry() {
        use crate::model::GradientStop;
        let stops = vec![
            GradientStop {
                offset: 0.0,
                color: Color::black(),
                opacity: 1.0,
                midpoint: 0.5,
                extra: Map::new(),
            },
            GradientStop {
                offset: 1.0,
                color: Color::rgb(1.0, 1.0, 1.0),
                opacity: 1.0,
                midpoint: 0.5,
                extra: Map::new(),
            },
        ];
        let spec = GradientSpec {
            kind: "linear".into(),
            stops: stops.clone(),
            angle: 0.0,
            spread: "pad".into(),
            ..Default::default()
        };
        // Centro deslocado e escala voltam pela caixa.
        for kind in ["linear", "elliptic"] {
            let b = Rect::new(0.0, 0.0, 40.0, 20.0);
            let s = GradientSpec {
                kind: kind.into(),
                center: [0.5, -0.25],
                scale: 0.6,
                steps: Some(5),
                ..spec.clone()
            };
            let back = gradient_spec_in(&gradient_for(&s, b), Some(b));
            assert!(
                (back.center[0] - 0.5).abs() < 1e-9 && (back.center[1] + 0.25).abs() < 1e-9,
                "{kind}"
            );
            assert!((back.scale - 0.6).abs() < 1e-9, "{kind}: {}", back.scale);
            assert_eq!(back.steps, Some(5));
        }
        let g = gradient_for(&spec, Rect::new(0.0, 0.0, 40.0, 20.0));
        assert_eq!((g.start, g.end), (Some([0.0, 10.0]), Some([40.0, 10.0])));
        let v = gradient_for(
            &GradientSpec {
                angle: 90.0,
                ..spec.clone()
            },
            Rect::new(0.0, 0.0, 40.0, 20.0),
        );
        let (a, b) = (v.start.unwrap(), v.end.unwrap());
        assert!(
            (a[1] - 0.0).abs() < 1e-9 && (b[1] - 20.0).abs() < 1e-9 && (a[0] - 20.0).abs() < 1e-9
        );
        assert!((gradient_spec(&v).angle - 90.0).abs() < 1e-9);
        let r = gradient_for(
            &GradientSpec {
                kind: "radial".into(),
                ..spec.clone()
            },
            Rect::new(0.0, 0.0, 30.0, 40.0),
        );
        assert_eq!((r.center, r.radius), (Some([15.0, 20.0]), Some(25.0)));
        // Aplicar em objetos: cada um pela própria caixa local.
        let (mut ed, ids) = setup();
        set_gradient(&mut ed, &ids, &spec);
        let o = ed.object(ed.locate(&ids[1]).unwrap()).unwrap();
        let Some(Fill::Gradient(g)) = &o.fill else {
            panic!("gradiente")
        };
        assert_eq!((g.start, g.end), (Some([0.0, 10.0]), Some([20.0, 10.0])));
        ed.undo();
        assert!(matches!(
            ed.object(ed.locate(&ids[1]).unwrap()).unwrap().fill,
            None | Some(Fill::Solid { .. })
        ));
    }

    /// GD-CAM-03: mover e copiar para outra página, na mesma posição.
    #[test]
    fn gd_cam_03_move_and_copy_to_page() {
        let (mut ed, ids) = setup();
        ed.doc
            .data
            .pages
            .push(crate::model::Page::new(200.0, 100.0));
        let before = b(&ed, &ids[0]);
        let copies = move_to_page(&mut ed, &ids[..1], 1, 0, true);
        assert_eq!(ed.objects(0, 0).len(), 3, "cópia mantém o original");
        assert_eq!(ed.objects(1, 0).len(), 1);
        assert_ne!(copies[0], ids[0], "cópia com ID novo");
        let moved = move_to_page(&mut ed, &ids[1..2], 1, 0, false);
        assert_eq!(moved, vec![ids[1].clone()]);
        assert_eq!((ed.objects(0, 0).len(), ed.objects(1, 0).len()), (2, 2));
        let o = &ed.objects(1, 0)[0];
        assert_eq!(
            bounds(o, Affine::IDENTITY, false).unwrap(),
            before,
            "mesma posição"
        );
        ed.undo();
        assert_eq!(ed.objects(0, 0).len(), 3);
    }
    /// GD-NOD-08: extrair um subcaminho vira objeto novo logo acima, com os
    /// mesmos atributos; um passo de desfazer.
    #[test]
    fn gd_nod_08_extract_subpath() {
        use crate::model::{Document, Node, Page, PathData, Subpath};
        let mut d = Document::new();
        d.data.pages.push(Page::new(100.0, 100.0));
        let mut ed = Editor::new(d);
        let sq = |x: f64| Subpath {
            closed: true,
            nodes: vec![
                Node::corner(x, 0.0),
                Node::corner(x + 5.0, 0.0),
                Node::corner(x + 5.0, 5.0),
            ],
            extra: Default::default(),
        };
        let mut o = Object::new(ObjectKind::Path(PathData {
            subpaths: vec![sq(0.0), sq(10.0), sq(20.0)],
            fill_rule: Default::default(),
            extra: Default::default(),
        }));
        o.fill = Some(Fill::solid(crate::model::Color::black()));
        let id = add_object(&mut ed, 0, 0, o, "p");
        let new = extract_subpaths(&mut ed, &id, &[1]).unwrap();
        let objs = ed.objects(0, 0);
        assert_eq!(objs.len(), 2);
        assert_eq!(
            (objs[0].id.as_str(), objs[1].id.as_str()),
            (id.as_str(), new.as_str())
        );
        let count = |o: &Object| match &o.kind {
            ObjectKind::Path(p) => p.subpaths.len(),
            _ => 0,
        };
        assert_eq!((count(&objs[0]), count(&objs[1])), (2, 1));
        assert_eq!(objs[1].fill, objs[0].fill);
        assert!(
            extract_subpaths(&mut ed, &new, &[0]).is_none(),
            "não esvazia o caminho"
        );
        ed.undo();
        assert_eq!(count(&ed.objects(0, 0)[0]), 3);
    }

    /// GD-TRF-03: limpar transformações tira giro e escala das primitivas,
    /// com o centro no mesmo lugar.
    #[test]
    fn gd_trf_03_clear_transformations() {
        use crate::model::{Document, Page, RectData};
        let mut d = Document::new();
        d.data.pages.push(Page::new(100.0, 100.0));
        let mut ed = Editor::new(d);
        let mut o = Object::new(ObjectKind::Rectangle(RectData {
            width: 20.0,
            height: 10.0,
            corners: None,
            extra: Default::default(),
        }));
        o.transform = Transform::from_affine(
            Affine::translate((50.0, 50.0))
                * Affine::rotate(0.7)
                * Affine::scale_non_uniform(2.0, 3.0),
        );
        let id = add_object(&mut ed, 0, 0, o, "r");
        let r = ed.find(&id).unwrap();
        let before = object_bounds(&ed, &r, false).unwrap().center();
        clear_transformations(&mut ed, std::slice::from_ref(&id));
        let r = ed.find(&id).unwrap();
        let b = object_bounds(&ed, &r, false).unwrap();
        assert!((b.width() - 20.0).abs() < 1e-9 && (b.height() - 10.0).abs() < 1e-9);
        assert!((b.center() - before).hypot() < 1e-9);
    }

    /// GD-OBJ-05: pares chave/valor no objeto, com desfazer.
    #[test]
    fn gd_obj_05_user_data() {
        let (mut ed, ids) = setup();
        let mut m = BTreeMap::new();
        m.insert("código".to_string(), "OP-123".to_string());
        set_user_data(&mut ed, &ids[0], m.clone());
        let get = |ed: &Editor| {
            ed.object(ed.locate(&ids[0]).unwrap())
                .unwrap()
                .user_data
                .clone()
        };
        assert_eq!(get(&ed), Some(m));
        set_user_data(&mut ed, &ids[0], BTreeMap::new());
        assert_eq!(get(&ed), None);
        ed.undo();
        assert!(get(&ed).is_some());
    }

    /// GD-CNV-03: a seleção vira uma imagem no mesmo lugar, na resolução
    /// pedida, com fundo transparente.
    #[test]
    fn gd_cnv_03_convert_to_bitmap() {
        use crate::model::{Color, Document, Page, RectData};
        let mut d = Document::new();
        d.data.pages.push(Page::new(100_000.0, 100_000.0));
        let mut ed = Editor::new(d);
        let mut o = Object::new(ObjectKind::Rectangle(RectData {
            width: 10_000.0,
            height: 10_000.0,
            corners: None,
            extra: Default::default(),
        }));
        o.transform = Transform::translate(20_000.0, 30_000.0);
        o.fill = Some(Fill::solid(Color::from_hex("#ff0000").unwrap()));
        let id = add_object(&mut ed, 0, 0, o, "r");
        let img = convert_to_bitmap(&mut ed, &[id], 254.0, true, None).unwrap();
        let objs = ed.objects(0, 0);
        assert_eq!(objs.len(), 1);
        let ObjectKind::Image(im) = &objs[0].kind else {
            panic!("imagem")
        };
        assert_eq!(objs[0].id, img);
        assert!((im.width - 10_000.0).abs() <= 100.0 && (im.height - 10_000.0).abs() <= 100.0);
        let t = objs[0].transform.affine().as_coeffs();
        assert!(
            (t[4] - 20_000.0).abs() <= 100.0 && (t[5] - 30_000.0).abs() <= 100.0,
            "{t:?}"
        );
        let png = ed.doc.images.get(&im.src).unwrap();
        let px = image::load_from_memory(png).unwrap().to_rgba8();
        assert_eq!((px.width(), px.height()), (100, 100));
        assert_eq!(px.get_pixel(50, 50).0, [255, 0, 0, 255]);
        ed.undo();
        assert!(matches!(ed.objects(0, 0)[0].kind, ObjectKind::Rectangle(_)));
    }

    /// GD-ALI-04: textos de corpos diferentes ficam com a mesma linha de base.
    #[test]
    fn gd_ali_04_align_baseline() {
        use crate::edit::text::{insert, new_text};
        use crate::model::{Document, Page};
        if crate::render::text::resolve_family("Arial", 400, false).is_none() {
            return;
        }
        let mut d = Document::new();
        d.data.pages.push(Page::new(200_000.0, 200_000.0));
        let mut ed = Editor::new(d);
        let mk = |ed: &mut Editor, x: f64, y: f64, size: f64| {
            let mut o = new_text(x, y, None, "Arial", size);
            if let ObjectKind::Text(t) = &mut o.kind {
                insert(t, 0, "Base");
            }
            add_object(ed, 0, 0, o, "t")
        };
        let a = mk(&mut ed, 10_000.0, 100_000.0, 5_000.0);
        let b = mk(&mut ed, 60_000.0, 130_000.0, 15_000.0);
        align_baseline(
            &mut ed,
            &[a.clone(), b.clone()],
            AlignTo::LastSelected,
            false,
        );
        let ya = text_baseline(&ed, &ed.find(&a).unwrap()).unwrap();
        let yb = text_baseline(&ed, &ed.find(&b).unwrap()).unwrap();
        assert!((ya - yb).abs() < 1e-6, "{ya} {yb}");
    }

    /// GD-TXT-09: texto ajustado a uma reta vertical fica de lado, ao longo
    /// dela; separar volta ao normal; mudar a posição desloca as letras.
    #[test]
    fn gd_txt_09_text_on_path() {
        use crate::edit::text::{insert, new_text};
        use crate::model::{Document, Node, Page, PathData, Subpath};
        if crate::render::text::resolve_family("Arial", 400, false).is_none() {
            return;
        }
        let mut d = Document::new();
        d.data.pages.push(Page::new(200_000.0, 200_000.0));
        let mut ed = Editor::new(d);
        let mut t = new_text(10_000.0, 10_000.0, None, "Arial", 10_000.0);
        if let ObjectKind::Text(td) = &mut t.kind {
            insert(td, 0, "ABCDE");
        }
        let tid = add_object(&mut ed, 0, 0, t, "t");
        let line = Object::new(ObjectKind::Path(PathData {
            subpaths: vec![Subpath {
                closed: false,
                nodes: vec![
                    Node::corner(100_000.0, 20_000.0),
                    Node::corner(100_000.0, 180_000.0),
                ],
                extra: Default::default(),
            }],
            fill_rule: Default::default(),
            extra: Default::default(),
        }));
        let pid = add_object(&mut ed, 0, 0, line, "p");
        let bounds = |ed: &Editor| object_bounds(ed, &ed.find(&tid).unwrap(), false).unwrap();
        let flat = bounds(&ed);
        assert!(flat.width() > flat.height());
        assert!(fit_text_to_path(&mut ed, &tid, &pid));
        let on = bounds(&ed);
        assert!(on.height() > on.width(), "de lado: {on:?}");
        // Texto acima (à esquerda) de uma reta que sobe, perto de x = 100 mm.
        assert!(on.x1 <= 100_000.0 + 500.0 && on.x0 > 80_000.0, "{on:?}");
        assert!(
            on.y0 >= 20_000.0 - 500.0,
            "começa no início do caminho: {on:?}"
        );
        edit_text_path(&mut ed, std::slice::from_ref(&tid), |p| p.offset = 50_000.0);
        let moved = bounds(&ed);
        assert!(
            (moved.y0 - on.y0 - 50_000.0).abs() < 1_000.0,
            "{on:?} → {moved:?}"
        );
        let svg = crate::export::svg_page(ed_doc(&ed), 0, &Default::default()).unwrap();
        assert!(!svg.contains("<text"), "em caminho sai em curvas");
        detach_text_path(&mut ed, std::slice::from_ref(&tid));
        let back = bounds(&ed);
        assert!((back.width() - flat.width()).abs() < 1.0);
    }

    fn ed_doc(ed: &Editor) -> &crate::model::Document {
        &ed.doc
    }
    fn bdr_rect(ed: &mut Editor, x: f64, y: f64, w: f64, h: f64) -> String {
        let mut o = Object::new(ObjectKind::Rectangle(crate::model::RectData {
            width: w,
            height: h,
            corners: None,
            extra: Default::default(),
        }));
        o.transform = Transform::translate(x, y);
        add_object(ed, 0, 0, o, "r")
    }

    fn bdr_editor() -> Editor {
        let mut d = crate::model::Document::new();
        d.data
            .pages
            .push(crate::model::Page::new(300_000.0, 300_000.0));
        Editor::new(d)
    }

    /// GD-BDR-28 (aceite): copiar só contorno de A para B; preenchimento de
    /// B e todo o A ficam.
    #[test]
    fn gd_bdr_28_copy_only_stroke() {
        use crate::model::Color;
        let mut ed = bdr_editor();
        let a = bdr_rect(&mut ed, 0.0, 0.0, 10_000.0, 10_000.0);
        let b = bdr_rect(&mut ed, 50_000.0, 0.0, 10_000.0, 10_000.0);
        let mut red = Stroke::new(705.6, Color::from_hex("#ff0000").unwrap());
        red.dash = Some(vec![2000.0, 1000.0]);
        let blue = Some(Fill::solid(Color::from_hex("#0000ff").unwrap()));
        let yellow = Some(Fill::solid(Color::from_hex("#ffff00").unwrap()));
        modify(&mut ed, std::slice::from_ref(&a), "x", |o| {
            o.stroke = Some(red.clone());
            o.fill = blue.clone();
            true
        });
        modify(&mut ed, std::slice::from_ref(&b), "x", |o| {
            o.stroke = Some(Stroke::new(352.8, Color::black()));
            o.fill = yellow.clone();
            true
        });
        assert!(copy_properties(
            &mut ed,
            &a,
            std::slice::from_ref(&b),
            CopyProps::STROKE
        ));
        let get = |ed: &Editor, id: &str| ed.get(&ed.find(id).unwrap()).unwrap().clone();
        assert_eq!(get(&ed, &b).stroke, Some(red.clone()));
        assert_eq!(get(&ed, &b).fill, yellow);
        assert_eq!(get(&ed, &a).fill, blue, "A inalterado");
        ed.undo();
        assert_eq!(get(&ed, &b).stroke.unwrap().width, 352.8, "um passo");
    }

    /// GD-BDR-33 (aceite): A 10×10 mm em (0,0) centraliza no centro de B
    /// 40×20 mm com centro em (100,50).
    #[test]
    fn gd_bdr_33_center_on_target() {
        let mut ed = bdr_editor();
        let a = bdr_rect(&mut ed, 0.0, 0.0, 10_000.0, 10_000.0);
        let b = bdr_rect(&mut ed, 80_000.0, 40_000.0, 40_000.0, 20_000.0);
        assert!(center_on(&mut ed, std::slice::from_ref(&a), &b));
        let c = object_bounds(&ed, &ed.find(&a).unwrap(), false)
            .unwrap()
            .center();
        assert!(
            (c.x - 100_000.0).abs() <= 1.0 && (c.y - 50_000.0).abs() <= 1.0,
            "{c:?}"
        );
    }

    /// GD-BDR-41/42: à frente de / atrás de, desagrupar tudo, mostrar e
    /// desbloquear todos.
    #[test]
    fn gd_bdr_41_42_order_and_organize() {
        let mut ed = bdr_editor();
        let a = bdr_rect(&mut ed, 0.0, 0.0, 1000.0, 1000.0);
        let b = bdr_rect(&mut ed, 0.0, 0.0, 1000.0, 1000.0);
        let c = bdr_rect(&mut ed, 0.0, 0.0, 1000.0, 1000.0);
        let order = |ed: &Editor| {
            ed.objects(0, 0)
                .iter()
                .map(|o| o.id.clone())
                .collect::<Vec<_>>()
        };
        place_relative(&mut ed, std::slice::from_ref(&c), &a, false);
        assert_eq!(
            order(&ed),
            vec![c.clone(), a.clone(), b.clone()],
            "atrás de A"
        );
        place_relative(&mut ed, &[c.clone(), a.clone()], &b, true);
        assert_eq!(
            order(&ed),
            vec![b.clone(), c.clone(), a.clone()],
            "à frente de B, ordem mantida"
        );
        // Grupo dentro de grupo: desagrupar tudo chega às folhas.
        let g1 = group(&mut ed, &[a.clone(), c.clone()]).unwrap();
        let g2 = group(&mut ed, &[g1, b.clone()]).unwrap();
        let leaves = ungroup_all(&mut ed, &[g2]);
        let mut l = leaves.clone();
        l.sort();
        let mut want = vec![a.clone(), b.clone(), c.clone()];
        want.sort();
        assert_eq!(l, want);
        set_visible(&mut ed, std::slice::from_ref(&a), false);
        set_locked(&mut ed, std::slice::from_ref(&b), true);
        assert_eq!(show_all(&mut ed, 0), vec![a.clone()]);
        assert_eq!(unlock_all(&mut ed, 0), vec![b.clone()]);
        assert!(ed.objects(0, 0).iter().all(|o| o.visible && !o.locked));
    }

    /// GD-BDR-81: o documento da seleção tem uma página do tamanho dela, com
    /// os objetos encostados na origem.
    #[test]
    fn gd_bdr_81_selection_document() {
        let mut ed = bdr_editor();
        let a = bdr_rect(&mut ed, 20_000.0, 30_000.0, 10_000.0, 5_000.0);
        let _b = bdr_rect(&mut ed, 200_000.0, 200_000.0, 10_000.0, 5_000.0);
        let d = selection_document(&ed, std::slice::from_ref(&a)).unwrap();
        let p = &d.data.pages[0];
        assert_eq!((p.width, p.height), (10_000.0, 5_000.0));
        assert_eq!(p.layers[0].objects.len(), 1);
        let t = p.layers[0].objects[0].transform.affine().as_coeffs();
        assert!(t[4].abs() < 1e-9 && t[5].abs() < 1e-9);
    }

    /// GD-EXP-08: o PNG leva só a arte — página do tamanho dos objetos
    /// visíveis, sem os ocultos nem a página mestra.
    #[test]
    fn gd_exp_08_artwork_document() {
        let mut ed = bdr_editor();
        let _a = bdr_rect(&mut ed, 20_000.0, 30_000.0, 10_000.0, 5_000.0);
        let _b = bdr_rect(&mut ed, 50_000.0, 40_000.0, 10_000.0, 5_000.0);
        let c = bdr_rect(&mut ed, 150_000.0, 150_000.0, 10_000.0, 5_000.0);
        set_visible(&mut ed, std::slice::from_ref(&c), false);
        let d = artwork_document(&ed, &[0]).unwrap();
        assert_eq!(d.data.pages.len(), 1);
        let p = &d.data.pages[0];
        assert_eq!(
            (p.width, p.height),
            (40_000.0, 15_000.0),
            "só a arte visível"
        );
        assert_eq!(p.layers[0].objects.len(), 2);
        assert!(d.data.master_page.layers.is_empty());
        assert_eq!(
            artwork_document(&ed, &[]).map(|d| d.data.pages.len()),
            Some(1)
        );
        // O PNG sai do tamanho da arte, com o fundo transparente.
        let dir = std::env::temp_dir().join(format!("gd-exp08-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let out = dir.join("arte.png");
        let opts = crate::export::ExportOptions {
            dpi: 254.0,
            ..Default::default()
        };
        crate::export::export_with_report(&d, &out, crate::export::ExportFormat::Png, &opts)
            .unwrap();
        let img = image::open(&out).unwrap().to_rgba8();
        // 40 × 15 mm a 254 ppp = 400 × 150 px.
        assert_eq!((img.width(), img.height()), (400, 150));
        assert_eq!(
            img.get_pixel(399, 0)[3],
            0,
            "entre os retângulos: transparente"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
