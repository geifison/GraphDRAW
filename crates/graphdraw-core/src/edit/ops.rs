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
            let children: Vec<Object> = data
                .children
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

#[cfg(test)]
mod tests {
    use super::*;
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
}
