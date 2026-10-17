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

/// Gradiente pedido pela interface (GD-FIL-03): tipo, paradas, ângulo
/// (linear, graus a partir de +X) e repetição; a geometria (início/fim ou
/// centro/raio) é calculada pela caixa de cada objeto.
#[derive(Debug, Clone, PartialEq)]
pub struct GradientSpec {
    /// `linear` ou `radial`.
    pub kind: String,
    pub stops: Vec<crate::model::GradientStop>,
    pub angle: f64,
    /// `pad`, `repeat` ou `reflect`.
    pub spread: String,
}

/// Gradiente com a geometria ajustada à caixa `b` (coordenadas locais).
pub fn gradient_for(spec: &GradientSpec, b: Rect) -> crate::model::Gradient {
    let c = b.center();
    let (start, end, center, radius) = if spec.kind == "radial" {
        let r = (b.width().hypot(b.height()) / 2.0).max(1e-9);
        (None, None, Some([c.x, c.y]), Some(r))
    } else {
        let a = spec.angle.to_radians();
        let d = Vec2::new(a.cos(), a.sin());
        let half = (b.width() / 2.0 * a.cos()).abs() + (b.height() / 2.0 * a.sin()).abs();
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
        spread: spec.spread.clone(),
        extra: Default::default(),
    }
}

/// Lê de volta o pedido de um gradiente existente (ângulo pelo início/fim).
pub fn gradient_spec(g: &crate::model::Gradient) -> GradientSpec {
    let angle = match (g.start, g.end) {
        (Some(a), Some(b)) => (b[1] - a[1]).atan2(b[0] - a[0]).to_degrees(),
        _ => 0.0,
    };
    GradientSpec {
        kind: if g.kind == "radial" {
            "radial".into()
        } else {
            "linear".into()
        },
        stops: g.stops.clone(),
        angle,
        spread: g.spread.clone(),
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
                let shape = close_all(&kurbo::stroke(
                    local,
                    &ks,
                    &kurbo::StrokeOpts::default(),
                    tol,
                ));
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
                extra: Map::new(),
            },
            GradientStop {
                offset: 1.0,
                color: Color::rgb(1.0, 1.0, 1.0),
                opacity: 1.0,
                extra: Map::new(),
            },
        ];
        let spec = GradientSpec {
            kind: "linear".into(),
            stops: stops.clone(),
            angle: 0.0,
            spread: "pad".into(),
        };
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
}
