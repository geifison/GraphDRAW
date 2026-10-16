//! Operações de edição sobre a seleção, cada uma numa transação com nome
//! legível (GD-UND-01). A seleção é uma lista ordenada de IDs (GD-SEL-01: a
//! ordem de seleção é preservada para "relativo ao último/primeiro").
//!
//! - GD-TRF-01: mover, escalar, girar, refletir, inclinar; posição e tamanho
//!   por ponto de referência (9 âncoras).
//! - GD-TRF-02: aplicar ao conjunto ou a cada objeto.
//! - GD-ALI-01: alinhar relativo ao último/primeiro selecionado ou à página.
//! - GD-ORD-01: ordem de empilhamento.

use kurbo::{Affine, Point, Rect, Vec2};

use super::history::{Editor, ObjLoc};
use super::query::{bounds, union_bounds};
use crate::model::{Fill, Object, Stroke, Transform};

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

/// Localizações dos objetos selecionados que ainda existem.
pub fn locs(ed: &Editor, sel: &[String]) -> Vec<ObjLoc> {
    sel.iter().filter_map(|id| ed.locate(id)).collect()
}

pub fn selected_objects<'a>(ed: &'a Editor, sel: &[String]) -> Vec<&'a Object> {
    locs(ed, sel)
        .into_iter()
        .filter_map(|l| ed.object(l))
        .collect()
}

pub fn selection_bounds(ed: &Editor, sel: &[String], visual: bool) -> Option<Rect> {
    union_bounds(selected_objects(ed, sel), visual)
}

fn plural(n: usize, one: &str, many: &str) -> String {
    if n == 1 {
        format!("1 {one}")
    } else {
        format!("{n} {many}")
    }
}

/// Aplica `m` (em coordenadas da página) à esquerda da matriz de cada objeto.
pub fn transform(ed: &mut Editor, sel: &[String], m: Affine, name: &str) {
    let locs = locs(ed, sel);
    if locs.is_empty() {
        return;
    }
    let title = format!("{name} {}", plural(locs.len(), "objeto", "objetos"));
    ed.transaction(title, |ed| {
        for l in locs {
            let mut o = ed.object(l).unwrap().clone();
            o.transform = Transform::from_affine(m * o.transform.affine());
            ed.replace_object(l, o);
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
    let locs = locs(ed, sel);
    let title = format!("{name} {}", plural(locs.len(), "objeto", "objetos"));
    ed.transaction(title, |ed| {
        for l in locs {
            let mut o = ed.object(l).unwrap().clone();
            if let Some(b) = bounds(&o, Affine::IDENTITY, false) {
                o.transform = Transform::from_affine(make(b) * o.transform.affine());
                ed.replace_object(l, o);
            }
        }
    });
}

/// Aplica em cópia, `copies` vezes, acumulando a transformação (GD-TRF-02:
/// "girar 30° × 11 cópias"). Devolve os IDs das cópias.
pub fn transform_copies(ed: &mut Editor, sel: &[String], m: Affine, copies: usize) -> Selection {
    let locs = locs(ed, sel);
    let mut out = Vec::new();
    ed.transaction(
        format!("Transformar em {}", plural(copies, "cópia", "cópias")),
        |ed| {
            for l in locs {
                let base = ed.object(l).unwrap().clone();
                let mut acc = Affine::IDENTITY;
                for k in 0..copies {
                    acc = m * acc;
                    let mut c = fresh_copy(&base);
                    c.transform = Transform::from_affine(acc * base.transform.affine());
                    out.push(c.id.clone());
                    let at = ed.objects(l.page, l.layer).len();
                    let _ = k;
                    ed.insert_object(ObjLoc { index: at, ..l }, c);
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
        .filter_map(|id| {
            let l = ed.locate(id)?;
            Some((id.clone(), bounds(ed.object(l)?, Affine::IDENTITY, visual)?))
        })
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
                let l = ed.locate(id).unwrap();
                let mut o = ed.object(l).unwrap().clone();
                o.transform = Transform::from_affine(Affine::translate(d) * o.transform.affine());
                ed.replace_object(l, o);
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

pub fn reorder(ed: &mut Editor, sel: &[String], how: Order) {
    let mut locs = locs(ed, sel);
    if locs.is_empty() {
        return;
    }
    locs.sort();
    let name = match how {
        Order::ToFront => "Trazer para frente",
        Order::ToBack => "Enviar para trás",
        Order::Forward => "Um passo à frente",
        Order::Backward => "Um passo atrás",
        Order::Reverse => "Inverter ordem",
    };
    let ids: Vec<String> = locs
        .iter()
        .map(|l| ed.object(*l).unwrap().id.clone())
        .collect();
    ed.transaction(name, |ed| match how {
        Order::ToFront => {
            for id in &ids {
                let l = ed.locate(id).unwrap();
                let top = ed.objects(l.page, l.layer).len() - 1;
                ed.move_object(l, top);
            }
        }
        Order::ToBack => {
            for (k, id) in ids.iter().enumerate() {
                let l = ed.locate(id).unwrap();
                ed.move_object(l, k);
            }
        }
        Order::Forward => {
            for id in ids.iter().rev() {
                let l = ed.locate(id).unwrap();
                let n = ed.objects(l.page, l.layer).len();
                if l.index + 1 < n && !ids.contains(&ed.objects(l.page, l.layer)[l.index + 1].id) {
                    ed.move_object(l, l.index + 1);
                }
            }
        }
        Order::Backward => {
            for id in &ids {
                let l = ed.locate(id).unwrap();
                if l.index > 0 && !ids.contains(&ed.objects(l.page, l.layer)[l.index - 1].id) {
                    ed.move_object(l, l.index - 1);
                }
            }
        }
        Order::Reverse => {
            let slots: Vec<ObjLoc> = ids.iter().map(|id| ed.locate(id).unwrap()).collect();
            let objs: Vec<Object> = slots
                .iter()
                .map(|l| ed.object(*l).unwrap().clone())
                .collect();
            for (slot, o) in slots.iter().zip(objs.into_iter().rev()) {
                ed.replace_object(*slot, o);
            }
        }
    });
}

// ---- criar, excluir, duplicar, atributos -------------------------------------

fn fresh_copy(o: &Object) -> Object {
    let mut c = o.clone();
    renew_ids(&mut c);
    c
}

fn renew_ids(o: &mut Object) {
    o.id = crate::model::new_id();
    if let crate::model::ObjectKind::Group(g) = &mut o.kind {
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
    let mut locs = locs(ed, sel);
    if locs.is_empty() {
        return;
    }
    locs.sort();
    ed.transaction(
        format!("Excluir {}", plural(locs.len(), "objeto", "objetos")),
        |ed| {
            for l in locs.into_iter().rev() {
                ed.remove_object(l);
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
    let locs = locs(ed, sel);
    ed.transaction(name, |ed| {
        for l in locs {
            let mut o = ed.object(l).unwrap().clone();
            if f(&mut o) {
                ed.replace_object(l, o);
            }
        }
    });
}

/// Preenchimento uniforme ou nenhum (GD-FIL-01/02).
pub fn set_fill(ed: &mut Editor, sel: &[String], fill: Option<Fill>) {
    modify(ed, sel, "Alterar preenchimento", |o| {
        if matches!(
            o.kind,
            crate::model::ObjectKind::Image(_) | crate::model::ObjectKind::Group(_)
        ) {
            return false;
        }
        o.fill = fill.clone();
        true
    });
}

/// Contorno (GD-OUT-01..04); `None` remove.
pub fn set_stroke(ed: &mut Editor, sel: &[String], stroke: Option<Stroke>) {
    modify(ed, sel, "Alterar contorno", |o| {
        if matches!(
            o.kind,
            crate::model::ObjectKind::Image(_) | crate::model::ObjectKind::Group(_)
        ) {
            return false;
        }
        o.stroke = stroke.clone();
        true
    });
}

/// Altera só alguns campos do contorno existente (cria um padrão se não houver).
pub fn edit_stroke(ed: &mut Editor, sel: &[String], f: impl Fn(&mut Stroke)) {
    modify(ed, sel, "Alterar contorno", |o| {
        if matches!(
            o.kind,
            crate::model::ObjectKind::Image(_) | crate::model::ObjectKind::Group(_)
        ) {
            return false;
        }
        let mut s = o
            .stroke
            .clone()
            .unwrap_or_else(|| Stroke::new(0.0, crate::model::Color::black()));
        f(&mut s);
        o.stroke = Some(s);
        true
    });
}

/// Converte primitivas em curvas (GD-CNV-01, subconjunto).
pub fn convert_to_curves(ed: &mut Editor, sel: &[String]) {
    modify(
        ed,
        sel,
        "Converter em curvas",
        |o| match super::convert::to_curves(o) {
            Some(c) => {
                *o = c;
                true
            }
            None => false,
        },
    );
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
}
