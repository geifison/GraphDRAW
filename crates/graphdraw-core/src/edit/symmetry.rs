//! Simetria ao vivo (GD-FER-33): um grupo com `symmetry` desenha os
//! filhos e as cópias espelhadas ou giradas em torno do centro. Editar um
//! filho muda as cópias na hora; desagrupar assa as cópias em objetos.

use kurbo::{Affine, Point, Rect, Vec2};

use super::history::Editor;
use crate::model::{GroupData, Object, ObjectKind, Symmetry, Transform};

/// Matrizes das cópias (sem a identidade), no espaço do grupo.
pub fn copies(s: Option<&Symmetry>) -> Vec<Affine> {
    let Some(s) = s else {
        return Vec::new();
    };
    let n = s.count.clamp(1, 64) as usize;
    let c = Vec2::new(s.center[0], s.center[1]);
    let about = |a: Affine| Affine::translate(c) * a * Affine::translate(-c);
    let step = std::f64::consts::TAU / n as f64;
    let base = s.angle.to_radians();
    let mut out = Vec::new();
    for k in 1..n {
        out.push(about(Affine::rotate(step * k as f64)));
    }
    if s.kind == "mirror" {
        // Reflexões nos eixos a `base + k·π/n`.
        for k in 0..n {
            let t = base + step / 2.0 * k as f64;
            let (sn, cs) = (2.0 * t).sin_cos();
            out.push(about(Affine::new([cs, sn, sn, -cs, 0.0, 0.0])));
        }
    }
    out
}

/// Filhos com as cópias assadas (IDs novos), no espaço do grupo.
pub fn baked(g: &GroupData) -> Vec<Object> {
    let mut out = g.children.clone();
    for t in copies(g.symmetry.as_ref()) {
        for c in &g.children {
            let mut n = c.clone();
            super::ops::renew_ids(&mut n);
            n.transform = Transform::from_affine(t * c.transform.affine());
            out.push(n);
        }
    }
    out
}

/// Cria um grupo de simetria com os objetos `sel` (do mesmo contêiner) ou
/// vazio na camada `layer` de `page`; centro em `center` (página).
pub fn create(
    ed: &mut Editor,
    page: usize,
    layer: usize,
    sel: &[String],
    kind: &str,
    count: u32,
    center: Point,
) -> Option<String> {
    let sym = Symmetry {
        kind: kind.into(),
        count,
        center: [center.x, center.y],
        angle: 90.0,
    };
    let single = match sel {
        [one] => {
            let r = ed.find(one)?;
            Some((r.clone(), ed.get(&r)?.clone()))
        }
        _ => None,
    };
    ed.begin("Simetria");
    let id = if sel.len() >= 2 {
        super::ops::group(ed, sel)
    } else if let Some((r, o)) = single {
        // Um objeto só: grupo de um filho.
        let g = Object::new(ObjectKind::Group(GroupData {
            children: vec![o],
            symmetry: None,
            extra: Default::default(),
        }));
        let gid = g.id.clone();
        ed.replace_at(&r, g);
        Some(gid)
    } else {
        Some(super::ops::add_object(
            ed,
            page,
            layer,
            Object::new(ObjectKind::Group(GroupData {
                children: Vec::new(),
                symmetry: None,
                extra: Default::default(),
            })),
            "Simetria",
        ))
    };
    if let Some(gid) = &id
        && let Some(r) = ed.find(gid)
        && let Some(mut g) = ed.get(&r).cloned()
    {
        let inv = (ed.parent_affine(&r) * g.transform.affine()).inverse();
        let c = inv * center;
        if let ObjectKind::Group(d) = &mut g.kind {
            d.symmetry = Some(Symmetry {
                center: [c.x, c.y],
                ..sym
            });
        }
        ed.replace_at(&r, g);
    }
    ed.commit();
    id
}

/// Põe `o` (página) dentro do grupo `gid`, levando ao espaço dele.
pub fn add_to_group(ed: &mut Editor, gid: &str, mut o: Object, name: &str) -> Option<String> {
    let r = ed.find(gid)?;
    let g = ed.get(&r)?;
    if !matches!(g.kind, ObjectKind::Group(_)) {
        return None;
    }
    let inv = (ed.parent_affine(&r) * g.transform.affine()).inverse();
    o.transform = Transform::from_affine(inv * o.transform.affine());
    let id = o.id.clone();
    ed.transaction(name, |ed| ed.edit_children(&r, |list| list.push(o)));
    Some(id)
}

/// Centro (página) e caixa padrão para a simetria: o meio da seleção ou da
/// página.
pub fn default_center(bounds: Option<Rect>, page_w: f64, page_h: f64) -> Point {
    bounds.map_or(Point::new(page_w / 2.0, page_h / 2.0), |b| b.center())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Document, Page, RectData};

    fn sym(kind: &str, count: u32) -> Symmetry {
        Symmetry {
            kind: kind.into(),
            count,
            center: [100.0, 0.0],
            angle: 90.0,
        }
    }

    /// GD-FER-33: espelho de um eixo vertical em x = 100 leva (90, 5) a
    /// (110, 5); radial de 4 dá 3 cópias giradas; caleidoscópio de 3 eixos,
    /// 2 rotações e 3 reflexões.
    #[test]
    fn gd_fer_33_copies() {
        let m = copies(Some(&sym("mirror", 1)));
        assert_eq!(m.len(), 1);
        let q = m[0] * Point::new(90.0, 5.0);
        assert!((q - Point::new(110.0, 5.0)).hypot() < 1e-9, "{q:?}");
        let r = copies(Some(&sym("radial", 4)));
        assert_eq!(r.len(), 3);
        let q = r[0] * Point::new(110.0, 0.0);
        assert!((q - Point::new(100.0, 10.0)).hypot() < 1e-9, "{q:?}");
        assert_eq!(copies(Some(&sym("mirror", 3))).len(), 5);
        assert!(copies(None).is_empty());
    }

    /// Desenhar dentro do grupo de simetria: as cópias acompanham, a caixa
    /// inclui as cópias e desagrupar assa os objetos.
    #[test]
    fn live_group_and_bake() {
        let mut d = Document::new();
        d.data.pages.push(Page::new(200_000.0, 200_000.0));
        let mut ed = Editor::new(d);
        let gid = create(
            &mut ed,
            0,
            0,
            &[],
            "mirror",
            1,
            Point::new(100_000.0, 100_000.0),
        )
        .unwrap();
        let mut o = Object::new(ObjectKind::Rectangle(RectData {
            width: 10_000.0,
            height: 10_000.0,
            corners: None,
            extra: Default::default(),
        }));
        o.transform = Transform::translate(60_000.0, 50_000.0);
        let undo = ed.undo_len();
        add_to_group(&mut ed, &gid, o, "Retângulo").unwrap();
        assert_eq!(ed.undo_len(), undo + 1);
        let g = ed.get(&ed.find(&gid).unwrap()).unwrap().clone();
        let b = super::super::query::bounds(&g, Affine::IDENTITY, false).unwrap();
        assert!(
            (b.x0 - 60_000.0).abs() < 1e-6 && (b.x1 - 140_000.0).abs() < 1e-6,
            "{b:?}"
        );
        // Ida e volta.
        assert_eq!(Object::from_value(g.to_value()), g);
        let out = super::super::ops::ungroup(&mut ed, std::slice::from_ref(&gid));
        assert_eq!(out.len(), 2);
        let objs = &ed.doc.data.pages[0].layers[0].objects;
        let xs: Vec<f64> = objs
            .iter()
            .map(|o| {
                super::super::query::bounds(o, Affine::IDENTITY, false)
                    .unwrap()
                    .x0
            })
            .collect();
        assert!(xs.iter().any(|x| (x - 130_000.0).abs() < 1e-6), "{xs:?}");
    }
}
