//! Booleanas (GD-BOO-01..03, 10), combinar/separar (GD-CMB-01/02) e
//! contorno de deslocamento (GD-OFS-01).
//!
//! As áreas são poligonizadas com tolerância de [`FLATTEN_TOL`] (1 µm),
//! operadas com o `i_overlay` (regra de preenchimento de cada objeto
//! respeitada) e o resultado volta a curvas Bézier pelo ajuste de
//! [`super::fit`] com precisão [`FIT_ACCURACY`] (2 µm), o que também tira
//! os nós redundantes (GD-BOO-11). Só o preenchimento conta como área; o contorno não.

use i_overlay::core::fill_rule::FillRule as OFill;
use i_overlay::core::overlay_rule::OverlayRule;
use i_overlay::float::simplify::SimplifyShape;
use i_overlay::float::single::SingleFloatOverlay;
use i_overlay::mesh::float::outline::offset::OutlineOffset;
use i_overlay::mesh::float::style::{LineJoin, OutlineStyle};
use kurbo::{Affine, BezPath, PathEl, Point, Shape};

use super::convert::bez_to_subpaths;
use super::history::{Editor, ObjLoc};
use super::query::local_outline;
use super::tree::ObjRef;
use crate::model::geom::subpaths_to_bez;
use crate::model::{FillRule, Object, ObjectKind, PathData, Stroke, Transform};

/// Tolerância da poligonização das curvas, em µm.
pub const FLATTEN_TOL: f64 = 1.0;
/// Precisão do ajuste de curvas sobre o resultado, em µm.
pub const FIT_ACCURACY: f64 = 2.0;
/// Viradas abaixo deste ângulo (graus) entre segmentos são tratadas como
/// curva suave no ajuste; acima, viram canto.
const SMOOTH_TURN_DEG: f64 = 12.0;

type Contour = Vec<[f64; 2]>;
/// Formas do `i_overlay`: cada uma é o contorno externo seguido dos furos.
pub type Shapes = Vec<Vec<Contour>>;

fn ofill(r: FillRule) -> OFill {
    match r {
        FillRule::Evenodd => OFill::EvenOdd,
        FillRule::Nonzero => OFill::NonZero,
    }
}

/// Contornos poligonais de um caminho (subcaminhos abertos fecham, como no
/// preenchimento).
fn contours(path: &BezPath) -> Vec<Contour> {
    let mut out: Vec<Contour> = Vec::new();
    let mut cur: Contour = Vec::new();
    kurbo::flatten(path, FLATTEN_TOL, |el| match el {
        PathEl::MoveTo(p) => {
            if cur.len() >= 3 {
                out.push(std::mem::take(&mut cur));
            }
            cur.clear();
            cur.push([p.x, p.y]);
        }
        PathEl::LineTo(p) => cur.push([p.x, p.y]),
        PathEl::ClosePath => {
            if cur.len() >= 3 {
                out.push(std::mem::take(&mut cur));
            }
            cur.clear();
        }
        _ => {}
    });
    if cur.len() >= 3 {
        out.push(cur);
    }
    out
}

/// Área preenchida do objeto em coordenadas da página, já normalizada
/// (externos anti-horários, furos horários). Grupos = união dos filhos.
pub fn region(o: &Object, parent: Affine) -> Shapes {
    let m = parent * o.transform.affine();
    if let ObjectKind::Group(g) = &o.kind {
        return union_all(g.children.iter().map(|c| region(c, m)));
    }
    let Some(mut path) = local_outline(o) else {
        return Vec::new();
    };
    path.apply_affine(m);
    let rule = match &o.kind {
        ObjectKind::Path(p) => p.fill_rule,
        _ => FillRule::Nonzero,
    };
    let c = contours(&path);
    if c.is_empty() {
        return Vec::new();
    }
    c.simplify_shape(ofill(rule))
}

fn overlay(a: &Shapes, b: &Shapes, rule: OverlayRule) -> Shapes {
    if a.is_empty() && b.is_empty() {
        return Vec::new();
    }
    a.overlay(b, rule, OFill::NonZero)
}

pub fn union_all(parts: impl IntoIterator<Item = Shapes>) -> Shapes {
    parts
        .into_iter()
        .fold(Vec::new(), |acc, s| overlay(&acc, &s, OverlayRule::Union))
}

/// Contornos de volta a curvas, sem nós redundantes.
pub fn shapes_to_path(shapes: &Shapes) -> BezPath {
    let mut out = BezPath::new();
    for c in shapes.iter().flatten() {
        let pts: Vec<Point> = c.iter().map(|p| Point::new(p[0], p[1])).collect();
        let fitted = super::fit::fit_closed(&pts, FIT_ACCURACY, SMOOTH_TURN_DEG);
        out.extend(fitted.elements().iter().copied());
    }
    out
}

/// Área (em µm²) de um conjunto de formas.
pub fn area(shapes: &Shapes) -> f64 {
    shapes
        .iter()
        .flatten()
        .map(|c| {
            let n = c.len();
            (0..n)
                .map(|i| {
                    let (a, b) = (c[i], c[(i + 1) % n]);
                    a[0] * b[1] - b[0] * a[1]
                })
                .sum::<f64>()
                / 2.0
        })
        .sum::<f64>()
        .abs()
}

/// Booleana entre objetos (GD-BOO-01..03).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoolOp {
    /// Soldar: uma área com todos.
    Weld,
    /// Aparar: o alvo perde a área dos demais.
    Trim,
    /// Interseção: novo objeto com a área comum a todos.
    Intersect,
}

/// Opções das booleanas (GD-BOO-10). O alvo é o último selecionado e o
/// resultado herda os atributos dele.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoolOptions {
    /// Manter os demais selecionados (fontes).
    pub keep_sources: bool,
    /// Manter o alvo original.
    pub keep_target: bool,
}

impl BoolOp {
    /// Padrão de cada operação: soldar consome tudo; aparar mantém as
    /// fontes; interseção mantém todos.
    pub fn default_options(self) -> BoolOptions {
        match self {
            BoolOp::Weld => BoolOptions {
                keep_sources: false,
                keep_target: false,
            },
            BoolOp::Trim => BoolOptions {
                keep_sources: true,
                keep_target: false,
            },
            BoolOp::Intersect => BoolOptions {
                keep_sources: true,
                keep_target: true,
            },
        }
    }
}

/// Objeto de caminho (coordenadas da página) com os atributos de `like`.
fn path_object(path: &BezPath, like: &Object, rule: FillRule) -> Object {
    let mut o = Object::new(ObjectKind::Path(PathData {
        subpaths: bez_to_subpaths(path),
        fill_rule: rule,
        extra: Default::default(),
    }));
    o.fill = like.fill.clone();
    o.stroke = like.stroke.clone();
    o.opacity = like.opacity;
    o.printable = like.printable;
    o
}

/// Objeto de primeiro nível que contém `r` (o resultado entra ao lado dele).
fn top_loc(r: &ObjRef) -> ObjLoc {
    r.loc
}

/// Booleana sobre a seleção; devolve a nova seleção (o resultado), ou
/// `None` se não houver o que fazer (menos de dois objetos, resultado vazio).
pub fn boolean(
    ed: &mut Editor,
    sel: &[String],
    op: BoolOp,
    opts: BoolOptions,
) -> Option<Vec<String>> {
    let refs: Vec<ObjRef> = sel.iter().filter_map(|id| ed.find(id)).collect();
    if refs.len() < 2 {
        return None;
    }
    let target_ref = refs.last()?.clone();
    let regions: Vec<Shapes> = refs
        .iter()
        .map(|r| region(ed.get(r).unwrap(), ed.parent_affine(r)))
        .collect();
    let (target_region, sources) = regions.split_last()?;
    let result = match op {
        BoolOp::Weld => union_all(regions.iter().cloned()),
        BoolOp::Trim => {
            let cut = union_all(sources.iter().cloned());
            overlay(target_region, &cut, OverlayRule::Difference)
        }
        BoolOp::Intersect => sources.iter().fold(target_region.clone(), |acc, s| {
            overlay(&acc, s, OverlayRule::Intersect)
        }),
    };
    if area(&result) <= 0.0 {
        return None;
    }
    let target = ed.get(&target_ref)?.clone();
    let mut obj = path_object(&shapes_to_path(&result), &target, FillRule::Nonzero);
    if op == BoolOp::Trim && !opts.keep_target {
        // Aparar mantém a identidade do alvo (nome e ID).
        obj.id = target.id.clone();
        obj.name = target.name.clone();
    }
    let new_id = obj.id.clone();
    let name = match op {
        BoolOp::Weld => "Soldar",
        BoolOp::Trim => "Aparar",
        BoolOp::Intersect => "Interseção",
    };
    let remove: Vec<String> = refs
        .iter()
        .enumerate()
        .filter(|(i, _)| {
            let is_target = *i == refs.len() - 1;
            if is_target {
                !opts.keep_target
            } else {
                !opts.keep_sources
            }
        })
        .map(|(_, r)| ed.get(r).unwrap().id.clone())
        .collect();
    let at = top_loc(&target_ref);
    ed.transaction(name, |ed| {
        // Remove primeiro e insere no lugar do alvo (ou logo acima dele).
        let target_top = ed.object(at).map(|o| o.id.clone());
        for id in &remove {
            if let Some(r) = ed.find(id) {
                ed.remove_at(&r);
            }
        }
        let index = match target_top.and_then(|t| ed.find(&t)) {
            Some(r) => r.loc.index + 1,
            None => at.index.min(ed.objects(at.page, at.layer).len()),
        };
        ed.insert_object(ObjLoc { index, ..at }, obj);
    });
    Some(vec![new_id])
}

/// Caminho de um objeto em coordenadas da página (primitivas e texto viram
/// curvas; grupos juntam os filhos).
fn page_path(o: &Object, parent: Affine) -> BezPath {
    let m = parent * o.transform.affine();
    if let ObjectKind::Group(g) = &o.kind {
        let mut all = BezPath::new();
        for c in &g.children {
            all.extend(page_path(c, m).elements().iter().copied());
        }
        return all;
    }
    let mut p = local_outline(o).unwrap_or_default();
    p.apply_affine(m);
    p
}

/// Combinar (GD-CMB-01): os caminhos viram um objeto com subcaminhos; os
/// furos seguem a regra par-ímpar. Atributos do último selecionado.
pub fn combine(ed: &mut Editor, sel: &[String]) -> Option<String> {
    let refs: Vec<ObjRef> = sel.iter().filter_map(|id| ed.find(id)).collect();
    if refs.len() < 2 {
        return None;
    }
    let mut path = BezPath::new();
    for r in &refs {
        let o = ed.get(r).unwrap();
        path.extend(page_path(o, ed.parent_affine(r)).elements().iter().copied());
    }
    let target_ref = refs.last()?.clone();
    let target = ed.get(&target_ref)?.clone();
    let obj = path_object(&path, &target, FillRule::Evenodd);
    let id = obj.id.clone();
    let ids: Vec<String> = refs.iter().map(|r| ed.get(r).unwrap().id.clone()).collect();
    let at = target_ref.loc;
    ed.transaction("Combinar", |ed| {
        let top = ed.object(at).map(|o| o.id.clone());
        let index = top
            .and_then(|t| ed.find(&t))
            .map_or(at.index, |r| r.loc.index + 1);
        ed.insert_object(ObjLoc { index, ..at }, obj);
        for i in &ids {
            if let Some(r) = ed.find(i) {
                ed.remove_at(&r);
            }
        }
    });
    Some(id)
}

/// Profundidade de aninhamento de cada subcaminho (quantos outros o
/// contêm): par = parte externa, ímpar = furo.
fn nesting(path: &BezPath) -> Vec<usize> {
    let subs: Vec<BezPath> = split_subpaths(path);
    subs.iter()
        .enumerate()
        .map(|(i, s)| {
            let Some(p) = s.elements().iter().find_map(|e| match e {
                PathEl::MoveTo(p) => Some(*p),
                _ => None,
            }) else {
                return 0;
            };
            subs.iter()
                .enumerate()
                .filter(|(j, o)| *j != i && o.winding(p) != 0)
                .count()
        })
        .collect()
}

fn split_subpaths(path: &BezPath) -> Vec<BezPath> {
    let mut out: Vec<BezPath> = Vec::new();
    for el in path.elements() {
        if matches!(el, PathEl::MoveTo(_)) || out.is_empty() {
            out.push(BezPath::new());
        }
        out.last_mut().unwrap().push(*el);
    }
    out.retain(|p| !p.elements().is_empty());
    out
}

/// Separar (GD-CMB-02): cada subcaminho vira um objeto; com `keep_holes`,
/// os furos ficam com a parte externa que os contém. Devolve os novos IDs.
pub fn separate(ed: &mut Editor, id: &str, keep_holes: bool) -> Option<Vec<String>> {
    let r = ed.find(id)?;
    let o = ed.get(&r)?.clone();
    let ObjectKind::Path(p) = &o.kind else {
        return None;
    };
    if p.subpaths.len() < 2 {
        return None;
    }
    let local = subpaths_to_bez(&p.subpaths);
    let subs = split_subpaths(&local);
    let groups: Vec<BezPath> = if keep_holes {
        let depth = nesting(&local);
        let mut out: Vec<(usize, BezPath)> = Vec::new();
        for (i, s) in subs.iter().enumerate() {
            if depth[i].is_multiple_of(2) {
                out.push((i, s.clone()));
            }
        }
        for (i, s) in subs.iter().enumerate() {
            if !depth[i].is_multiple_of(2) {
                let probe = s.elements().iter().find_map(|e| match e {
                    PathEl::MoveTo(p) => Some(*p),
                    _ => None,
                });
                // Parte externa mais funda que contém o furo.
                if let Some(pt) = probe
                    && let Some((_, owner)) = out
                        .iter_mut()
                        .filter(|(j, b)| depth[*j] < depth[i] && b.winding(pt) != 0)
                        .max_by_key(|(j, _)| depth[*j])
                {
                    owner.extend(s.elements().iter().copied());
                }
            }
        }
        out.into_iter().map(|(_, b)| b).collect()
    } else {
        subs
    };
    let at = r.loc;
    let parent = ed.parent_affine(&r);
    let mut ids = Vec::new();
    let objs: Vec<Object> = groups
        .iter()
        .map(|g| {
            let mut n = o.clone();
            n.id = crate::model::new_id();
            n.transform = Transform::from_affine(parent * o.transform.affine());
            if let ObjectKind::Path(pd) = &mut n.kind {
                pd.subpaths = bez_to_subpaths(g);
            }
            ids.push(n.id.clone());
            n
        })
        .collect();
    ed.transaction("Separar", |ed| {
        ed.remove_at(&r);
        let index = if r.is_top() {
            at.index
        } else {
            ed.objects(at.page, at.layer).len()
        };
        for (k, n) in objs.into_iter().enumerate() {
            ed.insert_object(
                ObjLoc {
                    index: index + k,
                    ..at
                },
                n,
            );
        }
    });
    Some(ids)
}

/// Cantos do contorno de deslocamento (GD-OFS-01).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Join {
    Miter,
    Round,
    Bevel,
}

/// Área deslocada de `d` µm (positivo = para fora, negativo = para dentro).
pub fn offset_region(shapes: &Shapes, d: f64, join: Join) -> Shapes {
    if shapes.is_empty() || d == 0.0 {
        return shapes.clone();
    }
    let j = match join {
        // Ângulo interior mínimo do canto vivo: abaixo disso, chanfra.
        Join::Miter => LineJoin::Miter(10f64.to_radians()),
        // Segmentos do arco com comprimento até 1/20 do raio.
        Join::Round => LineJoin::Round(0.05),
        Join::Bevel => LineJoin::Bevel,
    };
    shapes.outline(&OutlineStyle::new(d).line_join(j))
}

/// Contorno de deslocamento da seleção: um caminho novo por objeto, acima
/// dele, só com contorno (o do original ou um fio de 0,25 mm). Devolve os IDs.
pub fn offset(ed: &mut Editor, sel: &[String], d: f64, join: Join) -> Vec<String> {
    let refs: Vec<ObjRef> = sel.iter().filter_map(|id| ed.find(id)).collect();
    let mut made: Vec<(ObjLoc, String, Object)> = Vec::new();
    for r in &refs {
        let o = ed.get(r).unwrap();
        let shapes = offset_region(&region(o, ed.parent_affine(r)), d, join);
        if area(&shapes) <= 0.0 {
            continue;
        }
        let mut n = path_object(&shapes_to_path(&shapes), o, FillRule::Nonzero);
        n.fill = None;
        n.stroke = o
            .stroke
            .clone()
            .or_else(|| Some(Stroke::new(250.0, crate::model::Color::black())));
        let top = ed.object(r.loc).map(|t| t.id.clone()).unwrap_or_default();
        made.push((r.loc, top, n));
    }
    let ids: Vec<String> = made.iter().map(|(_, _, n)| n.id.clone()).collect();
    if made.is_empty() {
        return ids;
    }
    ed.transaction("Contorno de deslocamento", |ed| {
        for (at, top, n) in made {
            let index = ed.find(&top).map_or(at.index, |r| r.loc.index + 1);
            ed.insert_object(ObjLoc { index, ..at }, n);
        }
    });
    ids
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Color, Document, EllipseData, Fill, Page, RectData};
    use serde_json::Map;

    fn editor() -> Editor {
        let mut d = Document::new();
        d.data.pages.push(Page::new(200_000.0, 200_000.0));
        Editor::new(d)
    }

    fn circle(ed: &mut Editor, x: f64, y: f64, r: f64) -> String {
        let mut o = Object::new(ObjectKind::Ellipse(EllipseData {
            rx: r,
            ry: r,
            mode: "ellipse".into(),
            start_angle: 0.0,
            end_angle: 360.0,
            extra: Map::new(),
        }));
        o.transform = Transform::translate(x, y);
        o.fill = Some(Fill::solid(Color::black()));
        super::super::ops::add_object(ed, 0, 0, o, "c")
    }

    fn rect(ed: &mut Editor, x: f64, y: f64, w: f64, h: f64) -> String {
        let mut o = Object::new(ObjectKind::Rectangle(RectData {
            width: w,
            height: h,
            corners: None,
            extra: Map::new(),
        }));
        o.transform = Transform::translate(x, y);
        o.fill = Some(Fill::solid(Color::black()));
        super::super::ops::add_object(ed, 0, 0, o, "r")
    }

    fn obj_area(ed: &Editor, id: &str) -> f64 {
        let r = ed.find(id).unwrap();
        area(&region(ed.get(&r).unwrap(), ed.parent_affine(&r)))
    }

    /// Aceite de GD-BOO-01: dois círculos de r = 10 mm com centros a 10 mm
    /// soldados viram 1 caminho com área 2πr² − lente, ± 0,1%.
    #[test]
    fn gd_boo_01_weld_two_circles() {
        let mut ed = editor();
        let r = 10_000.0;
        let a = circle(&mut ed, 50_000.0, 50_000.0, r);
        let b = circle(&mut ed, 60_000.0, 50_000.0, r);
        let out = boolean(
            &mut ed,
            &[a, b],
            BoolOp::Weld,
            BoolOp::Weld.default_options(),
        )
        .unwrap();
        assert_eq!(ed.objects(0, 0).len(), 1, "um caminho só");
        let o = &ed.objects(0, 0)[0];
        let ObjectKind::Path(p) = &o.kind else {
            panic!("caminho")
        };
        assert_eq!(p.subpaths.len(), 1);
        // Lente de dois círculos de raio r com centros a distância d = r:
        // 2r²·acos(d/2r) − (d/2)·√(4r² − d²).
        let d = r;
        let lens = 2.0 * r * r * (d / (2.0 * r)).acos() - d / 2.0 * (4.0 * r * r - d * d).sqrt();
        let want = 2.0 * std::f64::consts::PI * r * r - lens;
        let got = obj_area(&ed, &out[0]);
        assert!((got - want).abs() / want < 0.001, "{got} × {want}");
        // Curvas de volta e sem excesso de nós.
        assert!(
            p.subpaths[0].nodes.len() < 40,
            "{} nós",
            p.subpaths[0].nodes.len()
        );
        assert!(p.subpaths[0].nodes.iter().any(|n| n.handle_out.is_some()));
        ed.undo();
        assert_eq!(ed.objects(0, 0).len(), 2);
    }

    #[test]
    fn gd_boo_02_03_trim_and_intersect() {
        let mut ed = editor();
        let src = rect(&mut ed, 0.0, 0.0, 20_000.0, 20_000.0);
        let tgt = rect(&mut ed, 10_000.0, 0.0, 20_000.0, 20_000.0);
        let sel = vec![src.clone(), tgt.clone()];
        let i = boolean(
            &mut ed,
            &sel,
            BoolOp::Intersect,
            BoolOp::Intersect.default_options(),
        )
        .unwrap();
        assert!((obj_area(&ed, &i[0]) - 200e6).abs() < 1e3);
        assert_eq!(ed.objects(0, 0).len(), 3, "interseção mantém os originais");
        let t = boolean(&mut ed, &sel, BoolOp::Trim, BoolOp::Trim.default_options()).unwrap();
        assert_eq!(t[0], tgt, "aparar mantém o ID do alvo");
        assert!((obj_area(&ed, &tgt) - 200e6).abs() < 1e3);
        assert!(ed.find(&src).is_some(), "fonte fica");
        // Nada em comum: interseção não cria nada.
        let far = rect(&mut ed, 100_000.0, 100_000.0, 1_000.0, 1_000.0);
        assert!(
            boolean(
                &mut ed,
                &[src, far],
                BoolOp::Intersect,
                BoolOp::Intersect.default_options()
            )
            .is_none()
        );
    }

    #[test]
    fn gd_cmb_combine_and_separate_with_holes() {
        let mut ed = editor();
        let outer = rect(&mut ed, 0.0, 0.0, 30_000.0, 30_000.0);
        let hole = rect(&mut ed, 10_000.0, 10_000.0, 10_000.0, 10_000.0);
        let other = rect(&mut ed, 50_000.0, 0.0, 10_000.0, 10_000.0);
        let c = combine(&mut ed, &[outer, hole, other]).unwrap();
        assert_eq!(ed.objects(0, 0).len(), 1);
        // Par-ímpar: o quadrado do meio vira furo.
        assert!((obj_area(&ed, &c) - (900e6 - 100e6 + 100e6)).abs() < 1e3);
        let plain = separate(&mut ed, &c, false).unwrap();
        assert_eq!(plain.len(), 3);
        ed.undo();
        let kept = separate(&mut ed, &c, true).unwrap();
        assert_eq!(kept.len(), 2, "furo fica com a parte externa");
        let areas: Vec<f64> = kept.iter().map(|i| obj_area(&ed, i)).collect();
        assert!(areas.iter().any(|a| (a - 800e6).abs() < 1e3), "{areas:?}");
    }

    #[test]
    fn gd_ofs_01_offset_out_and_in() {
        let mut ed = editor();
        let r = rect(&mut ed, 0.0, 0.0, 20_000.0, 10_000.0);
        let out = offset(&mut ed, std::slice::from_ref(&r), 1_000.0, Join::Miter);
        assert_eq!(out.len(), 1);
        // Cantos vivos: 22 × 12 mm.
        let o = ed.find(&out[0]).unwrap();
        let b = page_path(ed.get(&o).unwrap(), Affine::IDENTITY).bounding_box();
        assert!(
            (b.width() - 22_000.0).abs() < 5.0 && (b.height() - 12_000.0).abs() < 5.0,
            "{b:?}"
        );
        let o = ed.get(&o).unwrap();
        assert!(o.fill.is_none() && o.stroke.is_some());
        // Arredondado: área = retângulo + faixa + 4 quartos de círculo.
        let round = offset_region(
            &region(ed.get(&ed.find(&r).unwrap()).unwrap(), Affine::IDENTITY),
            1_000.0,
            Join::Round,
        );
        let want = 200e6 + 2.0 * 1_000.0 * (20_000.0 + 10_000.0) + std::f64::consts::PI * 1e6;
        assert!(
            (area(&round) - want).abs() / want < 0.002,
            "{}",
            area(&round)
        );
        // Para dentro: 18 × 8 mm.
        let inner = offset_region(
            &region(ed.get(&ed.find(&r).unwrap()).unwrap(), Affine::IDENTITY),
            -1_000.0,
            Join::Miter,
        );
        assert!((area(&inner) - 144e6).abs() < 1e4, "{}", area(&inner));
    }
}
