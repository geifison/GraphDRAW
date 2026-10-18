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
pub(crate) fn contours(path: &BezPath) -> Vec<Contour> {
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
/// (externos anti-horários, furos horários). Grupos = união dos filhos;
/// booleana ao vivo = o resultado.
pub fn region(o: &Object, parent: Affine) -> Shapes {
    let m = parent * o.transform.affine();
    if let ObjectKind::Group(g) = &o.kind {
        if g.boolean.is_some() {
            return live_shapes(g, m);
        }
        if g.effect.is_some() {
            return union_all(super::effects::expand(g).iter().map(|c| region(c, m)));
        }
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

pub(crate) fn overlay(a: &Shapes, b: &Shapes, rule: OverlayRule) -> Shapes {
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
pub(crate) fn path_object(path: &BezPath, like: &Object, rule: FillRule) -> Object {
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

/// Resultado da booleana ao vivo do grupo (`m` = matriz do grupo na
/// página): soldar une, aparar tira do filho de baixo os de cima,
/// interseção fica com o comum.
pub fn live_shapes(g: &crate::model::GroupData, m: Affine) -> Shapes {
    let regs: Vec<Shapes> = g.children.iter().map(|c| region(c, m)).collect();
    let Some((first, rest)) = regs.split_first() else {
        return Vec::new();
    };
    match g.boolean.as_deref() {
        Some("trim") => overlay(
            first,
            &union_all(rest.iter().cloned()),
            OverlayRule::Difference,
        ),
        Some("intersect") => rest
            .iter()
            .fold(first.clone(), |a, s| overlay(&a, s, OverlayRule::Intersect)),
        _ => union_all(regs.iter().cloned()),
    }
}

/// O resultado como caminho (página), com o estilo do filho de baixo.
pub fn live_object(g: &crate::model::GroupData, m: Affine) -> Option<Object> {
    let first = g.children.first()?;
    let shapes = live_shapes(g, m);
    if area(&shapes) <= 0.0 {
        return None;
    }
    Some(path_object(
        &shapes_to_path(&shapes),
        first,
        FillRule::Nonzero,
    ))
}

/// Booleana não destrutiva: agrupa `sel` como `op` (`weld`, `trim`,
/// `intersect`) num passo; devolve o ID do grupo.
pub fn live_boolean(ed: &mut Editor, sel: &[String], op: &str) -> Option<String> {
    if sel.len() < 2 {
        return None;
    }
    ed.begin(match op {
        "trim" => "Aparar ao vivo",
        "intersect" => "Interseção ao vivo",
        _ => "Soldar ao vivo",
    });
    let gid = super::ops::group(ed, sel);
    if let Some(gid) = &gid {
        super::ops::modify(ed, std::slice::from_ref(gid), "Booleana", |o| {
            if let ObjectKind::Group(g) = &mut o.kind {
                g.boolean = Some(op.into());
                return true;
            }
            false
        });
    }
    ed.commit();
    gid
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
        let above = super::ops::survivor_above(ed, at, &remove);
        for id in &remove {
            if let Some(r) = ed.find(id) {
                ed.remove_at(&r);
            }
        }
        let index = super::ops::index_below(ed, at, above.as_ref());
        ed.insert_object(ObjLoc { index, ..at }, obj);
    });
    Some(vec![new_id])
}

/// Booleanas que dependem da ordem de empilhamento (GD-BOO-04..08).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShapeOp {
    /// Simplificar: cada objeto perde a área coberta pelos que estão na
    /// frente (só a área visível fica).
    Simplify,
    /// Frente menos trás: o da frente perde a área dos demais.
    FrontMinusBack,
    /// Trás menos frente: o de trás perde a área dos demais.
    BackMinusFront,
    /// Excluir (XOR): só as áreas cobertas por um número ímpar de objetos.
    Exclude,
    /// Dividir: cada região de sobreposição vira um objeto separado.
    Divide,
    /// Limite: novo objeto, sem preenchimento, no contorno externo da
    /// seleção; os originais ficam.
    Boundary,
}

impl ShapeOp {
    pub fn label(self) -> &'static str {
        match self {
            ShapeOp::Simplify => "Simplificar",
            ShapeOp::FrontMinusBack => "Frente menos trás",
            ShapeOp::BackMinusFront => "Trás menos frente",
            ShapeOp::Exclude => "Excluir",
            ShapeOp::Divide => "Dividir",
            ShapeOp::Boundary => "Limite",
        }
    }
}

/// Caminho no espaço do pai de `r` (para substituir no lugar), com a
/// identidade de `like`.
fn local_path_object(ed: &Editor, r: &ObjRef, shapes: &Shapes, like: &Object) -> Object {
    let mut path = shapes_to_path(shapes);
    path.apply_affine(ed.parent_affine(r).inverse());
    let mut o = path_object(&path, like, FillRule::Nonzero);
    o.id = like.id.clone();
    o.name = like.name.clone();
    o.locked = like.locked;
    o
}

/// Remove `remove` e insere `objs` (de trás para a frente) logo acima de
/// `at` (ou no lugar dele, se ele saiu).
fn put_results(ed: &mut Editor, name: &str, remove: &[String], at: &ObjRef, objs: Vec<Object>) {
    let at = top_loc(at);
    ed.transaction(name, |ed| {
        let above = super::ops::survivor_above(ed, at, remove);
        for id in remove {
            if let Some(r) = ed.find(id) {
                ed.remove_at(&r);
            }
        }
        let base = super::ops::index_below(ed, at, above.as_ref());
        for (k, o) in objs.into_iter().enumerate() {
            ed.insert_object(
                ObjLoc {
                    index: base + k,
                    ..at
                },
                o,
            );
        }
    });
}

/// Booleana pela ordem de empilhamento (GD-BOO-04..08); devolve a nova
/// seleção, ou `None` se não houver o que fazer.
pub fn shape_op(ed: &mut Editor, sel: &[String], op: ShapeOp) -> Option<Vec<String>> {
    let mut refs: Vec<ObjRef> = sel.iter().filter_map(|id| ed.find(id)).collect();
    let min = if op == ShapeOp::Boundary { 1 } else { 2 };
    if refs.len() < min {
        return None;
    }
    // De trás para a frente.
    refs.sort();
    let objs: Vec<Object> = refs.iter().map(|r| ed.get(r).unwrap().clone()).collect();
    let regions: Vec<Shapes> = refs
        .iter()
        .zip(&objs)
        .map(|(r, o)| region(o, ed.parent_affine(r)))
        .collect();
    let ids: Vec<String> = objs.iter().map(|o| o.id.clone()).collect();
    let front = refs.last()?.clone();
    let last = refs.len() - 1;
    match op {
        ShapeOp::Simplify => {
            let mut changed = Vec::new();
            for i in 0..refs.len() {
                let above = union_all(regions[i + 1..].iter().cloned());
                let hit = overlay(&regions[i], &above, OverlayRule::Intersect);
                if area(&hit) <= 0.0 {
                    continue;
                }
                changed.push((i, overlay(&regions[i], &above, OverlayRule::Difference)));
            }
            if changed.is_empty() {
                return None;
            }
            ed.transaction(op.label(), |ed| {
                for (i, left) in changed {
                    let Some(r) = ed.find(&ids[i]) else {
                        continue;
                    };
                    if area(&left) <= 0.0 {
                        ed.remove_at(&r);
                    } else {
                        let o = local_path_object(ed, &r, &left, &objs[i]);
                        ed.replace_at(&r, o);
                    }
                }
            });
            Some(ids.into_iter().filter(|id| ed.find(id).is_some()).collect())
        }
        ShapeOp::FrontMinusBack | ShapeOp::BackMinusFront => {
            let k = if op == ShapeOp::FrontMinusBack {
                last
            } else {
                0
            };
            let others = union_all(
                regions
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| *i != k)
                    .map(|(_, r)| r.clone()),
            );
            let left = overlay(&regions[k], &others, OverlayRule::Difference);
            if area(&left) <= 0.0 {
                return None;
            }
            let mut o = path_object(&shapes_to_path(&left), &objs[k], FillRule::Nonzero);
            o.id = objs[k].id.clone();
            o.name = objs[k].name.clone();
            let id = o.id.clone();
            put_results(ed, op.label(), &ids, &front, vec![o]);
            Some(vec![id])
        }
        ShapeOp::Exclude => {
            let x = regions[1..].iter().fold(regions[0].clone(), |acc, r| {
                overlay(&acc, r, OverlayRule::Xor)
            });
            if area(&x) <= 0.0 {
                return None;
            }
            let o = path_object(&shapes_to_path(&x), &objs[last], FillRule::Nonzero);
            let id = o.id.clone();
            put_results(ed, op.label(), &ids, &front, vec![o]);
            Some(vec![id])
        }
        ShapeOp::Divide => {
            // Cada peça leva os atributos do objeto mais à frente que a cobre.
            let mut parts: Vec<(Shapes, usize)> = Vec::new();
            for (i, r) in regions.iter().enumerate() {
                let mut next = Vec::new();
                for (p, owner) in parts {
                    let inside = overlay(&p, r, OverlayRule::Intersect);
                    let outside = overlay(&p, r, OverlayRule::Difference);
                    if area(&inside) > 0.0 {
                        next.push((inside, i));
                    }
                    if area(&outside) > 0.0 {
                        next.push((outside, owner));
                    }
                }
                let covered = union_all(next.iter().map(|(p, _)| p.clone()));
                let fresh = overlay(r, &covered, OverlayRule::Difference);
                if area(&fresh) > 0.0 {
                    next.push((fresh, i));
                }
                parts = next;
            }
            // Peças desconexas viram objetos separados, de trás para a
            // frente pelo dono.
            let mut out: Vec<(usize, Object)> = Vec::new();
            for (p, owner) in &parts {
                for shape in p {
                    let piece: Shapes = vec![shape.clone()];
                    if area(&piece) > 0.0 {
                        let path = shapes_to_path(&piece);
                        out.push((*owner, path_object(&path, &objs[*owner], FillRule::Nonzero)));
                    }
                }
            }
            if out.len() < 2 {
                return None;
            }
            out.sort_by_key(|(owner, _)| *owner);
            let new: Vec<String> = out.iter().map(|(_, o)| o.id.clone()).collect();
            let objs = out.into_iter().map(|(_, o)| o).collect();
            put_results(ed, op.label(), &ids, &front, objs);
            Some(new)
        }
        ShapeOp::Boundary => {
            let all = union_all(regions.iter().cloned());
            // Só o contorno externo de cada forma (sem furos).
            let outer: Shapes = all
                .iter()
                .filter_map(|s| s.first().cloned())
                .map(|c| vec![c])
                .collect();
            if area(&outer) <= 0.0 {
                return None;
            }
            let mut o = path_object(&shapes_to_path(&outer), &objs[last], FillRule::Nonzero);
            o.fill = None;
            o.stroke = Some(Stroke::new(250.0, crate::model::Color::black()));
            o.opacity = None;
            o.printable = None;
            o.name = Some("Limite".into());
            let id = o.id.clone();
            put_results(ed, op.label(), &[], &front, vec![o]);
            Some(vec![id])
        }
    }
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

/// Cor especial da linha de corte: tinta `name` com prévia magenta.
pub fn cut_color(name: &str) -> crate::model::Color {
    let mut c = crate::model::Color::new("spot", vec![1.0]);
    c.name = Some(name.into());
    c.alternate = Some(Box::new(crate::model::Color::cmyk(0.0, 1.0, 0.0, 0.0)));
    c
}

/// Gera a linha de corte ao redor da seleção (GD-CRT-02): silhueta (união)
/// deslocada `d` µm para fora, cantos arredondados opcionais, furos
/// internos removidos opcionalmente. Vai para a camada `name` (criada no
/// topo se faltar), com contorno fio de cabelo na cor especial `name`.
pub fn cut_line(
    ed: &mut Editor,
    sel: &[String],
    page: usize,
    d: f64,
    round: bool,
    remove_holes: bool,
    name: &str,
) -> Option<String> {
    let refs: Vec<ObjRef> = sel.iter().filter_map(|id| ed.find(id)).collect();
    if refs.is_empty() {
        return None;
    }
    let silhouette = union_all(
        refs.iter()
            .map(|r| region(ed.get(r).unwrap(), ed.parent_affine(r))),
    );
    let join = if round { Join::Round } else { Join::Miter };
    let mut shapes = offset_region(&silhouette, d, join);
    if remove_holes {
        shapes = shapes
            .into_iter()
            .map(|s| s.into_iter().take(1).collect())
            .collect();
    }
    if area(&shapes) <= 0.0 {
        return None;
    }
    let mut o = Object::new(ObjectKind::Path(PathData {
        subpaths: bez_to_subpaths(&shapes_to_path(&shapes)),
        fill_rule: FillRule::Nonzero,
        extra: Default::default(),
    }));
    o.name = Some(format!("Linha de corte ({name})"));
    o.stroke = Some(Stroke::new(0.0, cut_color(name)));
    let id = o.id.clone();
    ed.transaction("Gerar linha de corte", |ed| {
        let layer = match ed.layers(page).iter().position(|l| l.name == name) {
            Some(i) => i,
            None => {
                let n = ed.layers(page).len();
                ed.insert_layer(page, n, crate::model::Layer::new(name));
                n
            }
        };
        let index = ed.objects(page, layer).len();
        ed.insert_object(ObjLoc { page, layer, index }, o);
    });
    Some(id)
}

/// Nome da camada das marcas de registro.
pub const REGISTRATION_LAYER: &str = "Registro";

/// Marcas de registro para impressão e corte (GD-CRT-03): círculos cheios
/// de `diameter` µm na cor de registro, a `margin` µm fora da arte (todos
/// os objetos visíveis, menos as camadas de corte e de registro). Com
/// `corners` = 3, o canto inferior direito fica sem marca, para o leitor
/// do plotter achar a orientação. Vão para a camada "Registro".
pub fn registration_marks(
    ed: &mut Editor,
    page: usize,
    margin: f64,
    diameter: f64,
    corners: usize,
    cut_name: &str,
) -> Option<Vec<String>> {
    let art = ed
        .layers(page)
        .iter()
        .filter(|l| l.visible && l.name != cut_name && l.name != REGISTRATION_LAYER)
        .flat_map(|l| l.objects.iter())
        .filter(|o| o.visible)
        .filter_map(|o| super::query::bounds(o, Affine::IDENTITY, true))
        .reduce(|a, b| a.union(b))?;
    let r = diameter / 2.0;
    let off = margin + r;
    let mut spots = vec![
        Point::new(art.x0 - off, art.y1 + off),
        Point::new(art.x1 + off, art.y1 + off),
        Point::new(art.x0 - off, art.y0 - off),
    ];
    if corners >= 4 {
        spots.push(Point::new(art.x1 + off, art.y0 - off));
    }
    let marks: Vec<Object> = spots
        .iter()
        .map(|c| {
            let mut o = Object::new(ObjectKind::Ellipse(crate::model::EllipseData {
                rx: r,
                ry: r,
                mode: "ellipse".into(),
                start_angle: 0.0,
                end_angle: 360.0,
                extra: Default::default(),
            }));
            o.name = Some("Marca de registro".into());
            o.transform = Transform::translate(c.x, c.y);
            o.fill = Some(crate::model::Fill::solid(crate::model::Color::new(
                "registration",
                vec![1.0],
            )));
            o
        })
        .collect();
    let ids: Vec<String> = marks.iter().map(|o| o.id.clone()).collect();
    ed.transaction("Marcas de registro", |ed| {
        let layer = match ed
            .layers(page)
            .iter()
            .position(|l| l.name == REGISTRATION_LAYER)
        {
            Some(i) => i,
            None => {
                let n = ed.layers(page).len();
                ed.insert_layer(page, n, crate::model::Layer::new(REGISTRATION_LAYER));
                n
            }
        };
        for o in marks {
            let index = ed.objects(page, layer).len();
            ed.insert_object(ObjLoc { page, layer, index }, o);
        }
    });
    Some(ids)
}

#[cfg(test)]
mod live_tests {
    use super::*;
    use crate::model::{Document, Page, RectData, Transform};

    fn sq(x: f64) -> Object {
        let mut o = Object::new(ObjectKind::Rectangle(RectData {
            width: 100.0,
            height: 100.0,
            corners: None,
            extra: Default::default(),
        }));
        o.transform = Transform::translate(x, 0.0);
        o.fill = Some(crate::model::Fill::solid(crate::model::Color::black()));
        o
    }

    /// Booleana não destrutiva: o grupo vale pelo resultado (área, caixa);
    /// mexer num filho muda o resultado; trocar a operação também;
    /// converter em curvas fecha; desagrupar devolve as formas.
    #[test]
    fn live_boolean_stays_editable() {
        let mut d = Document::new();
        d.data.pages.push(Page::new(1000.0, 1000.0));
        let mut ed = Editor::new(d);
        let a = super::super::ops::add_object(&mut ed, 0, 0, sq(0.0), "a");
        let b = super::super::ops::add_object(&mut ed, 0, 0, sq(50.0), "b");
        let undo = ed.undo_len();
        let gid = live_boolean(&mut ed, &[a.clone(), b.clone()], "trim").unwrap();
        assert_eq!(ed.undo_len(), undo + 1);
        let g = |ed: &Editor| ed.get(&ed.find(&gid).unwrap()).unwrap().clone();
        let reg = |ed: &Editor| area(&region(&g(ed), Affine::IDENTITY));
        assert!((reg(&ed) - 5_000.0).abs() < 50.0, "aparado: {}", reg(&ed));
        let bb = super::super::query::bounds(&g(&ed), Affine::IDENTITY, false).unwrap();
        assert!((bb.x1 - 50.0).abs() < 1.0, "caixa do resultado: {bb:?}");
        // O filho de cima se afasta: nada a aparar.
        super::super::ops::transform(
            &mut ed,
            std::slice::from_ref(&b),
            Affine::translate((200.0, 0.0)),
            "Mover",
        );
        assert!((reg(&ed) - 10_000.0).abs() < 50.0);
        super::super::ops::transform(
            &mut ed,
            std::slice::from_ref(&b),
            Affine::translate((-200.0, 0.0)),
            "Mover",
        );
        super::super::ops::modify(&mut ed, std::slice::from_ref(&gid), "op", |o| {
            if let ObjectKind::Group(g) = &mut o.kind {
                g.boolean = Some("weld".into());
            }
            true
        });
        assert!((reg(&ed) - 15_000.0).abs() < 50.0, "soldado: {}", reg(&ed));
        let c = super::super::convert::to_curves(&g(&ed)).unwrap();
        assert!(matches!(c.kind, ObjectKind::Path(_)));
        assert_eq!(c.id, gid);
        let back = super::super::ops::ungroup(&mut ed, std::slice::from_ref(&gid));
        assert_eq!(back, vec![a, b]);
        // Ida e volta no JSON.
        let mut o = sq(0.0);
        o.kind = ObjectKind::Group(crate::model::GroupData {
            children: vec![sq(0.0)],
            boolean: Some("intersect".into()),
            symmetry: None,
            effect: None,
            symbol: None,
            extra: Default::default(),
        });
        assert_eq!(Object::from_value(o.to_value()), o);
    }
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

    /// GD-CRT-02: linha 2 mm fora da união, sem furos, na camada de corte.
    #[test]
    fn gd_crt_02_cut_line_around_selection() {
        let mut ed = editor();
        let a = rect(&mut ed, 0.0, 0.0, 20_000.0, 20_000.0);
        let b = rect(&mut ed, 10_000.0, 0.0, 20_000.0, 20_000.0);
        let id = cut_line(&mut ed, &[a, b], 0, 2_000.0, false, true, "CutContour").unwrap();
        let li = ed
            .layers(0)
            .iter()
            .position(|l| l.name == "CutContour")
            .unwrap();
        assert_eq!(ed.objects(0, li).len(), 1);
        let r = ed.find(&id).unwrap();
        let o = ed.get(&r).unwrap();
        let bb = page_path(o, Affine::IDENTITY).bounding_box();
        assert!(
            (bb.width() - 34_000.0).abs() < 5.0 && (bb.height() - 24_000.0).abs() < 5.0,
            "{bb:?}"
        );
        let s = o.stroke.as_ref().unwrap();
        assert_eq!(
            (s.width, s.color.name.as_deref()),
            (0.0, Some("CutContour"))
        );
        ed.undo();
        assert!(
            ed.layers(0).iter().all(|l| l.name != "CutContour"),
            "desfazer tira a camada criada"
        );
    }

    #[test]
    fn gd_crt_03_registration_marks_outside_art() {
        let mut ed = editor();
        rect(&mut ed, 10_000.0, 10_000.0, 50_000.0, 30_000.0);
        let ids = registration_marks(&mut ed, 0, 5_000.0, 5_000.0, 3, "CutContour").unwrap();
        assert_eq!(ids.len(), 3);
        for id in &ids {
            let r = ed.find(id).unwrap();
            let b =
                super::super::query::bounds(ed.get(&r).unwrap(), Affine::IDENTITY, false).unwrap();
            // Fora da arte (10..60 × 10..40 mm) com 5 mm de folga.
            let clear = b.x1 <= 5_000.0 + 1.0 || b.x0 >= 65_000.0 - 1.0;
            assert!(clear, "{b:?}");
        }
        assert!(ed.layers(0).iter().any(|l| l.name == REGISTRATION_LAYER));
    }
    fn ids_in_order(ed: &Editor) -> Vec<String> {
        ed.objects(0, 0).iter().map(|o| o.id.clone()).collect()
    }

    /// GD-BOO-04: simplificar deixa em cada objeto só a área visível.
    #[test]
    fn gd_boo_04_simplify() {
        let mut ed = editor();
        let back = rect(&mut ed, 0.0, 0.0, 20_000.0, 20_000.0);
        let front = rect(&mut ed, 10_000.0, 0.0, 20_000.0, 20_000.0);
        let far = rect(&mut ed, 100_000.0, 0.0, 10_000.0, 10_000.0);
        let sel = shape_op(
            &mut ed,
            &[front.clone(), far.clone(), back.clone()],
            ShapeOp::Simplify,
        )
        .unwrap();
        assert_eq!(sel.len(), 3);
        assert!(
            (obj_area(&ed, &back) - 200_000_000.0).abs() < 1e5,
            "trás perde a metade"
        );
        assert!((obj_area(&ed, &front) - 400_000_000.0).abs() < 1e5);
        // O que não estava coberto continua primitiva.
        let r = ed.find(&far).unwrap();
        assert!(matches!(ed.get(&r).unwrap().kind, ObjectKind::Rectangle(_)));
        assert!(ed.undo().is_some());
        let r = ed.find(&back).unwrap();
        assert!(
            matches!(ed.get(&r).unwrap().kind, ObjectKind::Rectangle(_)),
            "um passo de desfazer"
        );
    }

    /// GD-BOO-05: frente menos trás e trás menos frente seguem a ordem z,
    /// não a ordem de seleção.
    #[test]
    fn gd_boo_05_front_and_back_minus() {
        for (op, want) in [
            (ShapeOp::FrontMinusBack, 1usize),
            (ShapeOp::BackMinusFront, 0),
        ] {
            let mut ed = editor();
            let a = rect(&mut ed, 0.0, 0.0, 20_000.0, 20_000.0);
            let b = rect(&mut ed, 10_000.0, 0.0, 20_000.0, 20_000.0);
            let keep = [a.clone(), b.clone()][want].clone();
            let sel = shape_op(&mut ed, &[b, a], op).unwrap();
            assert_eq!(sel, vec![keep.clone()]);
            assert_eq!(ids_in_order(&ed), vec![keep.clone()]);
            assert!((obj_area(&ed, &keep) - 200_000_000.0).abs() < 1e5);
        }
    }

    /// GD-BOO-06: excluir deixa só a área sem sobreposição.
    #[test]
    fn gd_boo_06_exclude() {
        let mut ed = editor();
        let a = rect(&mut ed, 0.0, 0.0, 20_000.0, 20_000.0);
        let b = rect(&mut ed, 10_000.0, 0.0, 20_000.0, 20_000.0);
        let sel = shape_op(&mut ed, &[a, b], ShapeOp::Exclude).unwrap();
        assert_eq!(ids_in_order(&ed), sel);
        assert!((obj_area(&ed, &sel[0]) - 400_000_000.0).abs() < 1e5);
    }

    /// GD-BOO-07: dividir: duas caixas sobrepostas viram três peças; o meio
    /// leva os atributos da frente.
    #[test]
    fn gd_boo_07_divide() {
        let mut ed = editor();
        let a = rect(&mut ed, 0.0, 0.0, 20_000.0, 20_000.0);
        let b = rect(&mut ed, 10_000.0, 0.0, 20_000.0, 20_000.0);
        let red = Some(Fill::solid(Color::from_hex("#ff0000").unwrap()));
        let r = ed.find(&b).unwrap();
        let mut o = ed.get(&r).unwrap().clone();
        o.fill = red.clone();
        ed.replace_at(&r, o);
        let sel = shape_op(&mut ed, &[a, b], ShapeOp::Divide).unwrap();
        assert_eq!(sel.len(), 3);
        let areas: Vec<f64> = sel.iter().map(|id| obj_area(&ed, id)).collect();
        assert!(
            areas.iter().all(|a| (a - 200_000_000.0).abs() < 1e5),
            "{areas:?}"
        );
        let reds = sel
            .iter()
            .filter(|id| ed.get(&ed.find(id).unwrap()).unwrap().fill == red)
            .count();
        assert_eq!(reds, 2, "meio e direita são da frente");
        assert_eq!(ids_in_order(&ed), sel);
    }

    /// GD-BOO-08: limite: novo objeto sem preenchimento no contorno externo;
    /// os originais ficam.
    #[test]
    fn gd_boo_08_boundary() {
        let mut ed = editor();
        let a = rect(&mut ed, 0.0, 0.0, 20_000.0, 20_000.0);
        let b = rect(&mut ed, 10_000.0, 0.0, 20_000.0, 20_000.0);
        let sel = shape_op(&mut ed, &[a.clone(), b.clone()], ShapeOp::Boundary).unwrap();
        assert_eq!(ids_in_order(&ed), vec![a, b, sel[0].clone()]);
        let o = ed.get(&ed.find(&sel[0]).unwrap()).unwrap().clone();
        assert!(o.fill.is_none() && o.stroke.is_some());
        let mut filled = o;
        filled.fill = Some(Fill::solid(Color::black()));
        assert!((area(&region(&filled, Affine::IDENTITY)) - 600_000_000.0).abs() < 1e5);
    }

    /// Revisão de 2026-10-17: com os selecionados espalhados na pilha, o
    /// resultado fica no lugar do de cima, não no topo da camada.
    #[test]
    fn results_keep_place_in_stack() {
        for op in [ShapeOp::Exclude, ShapeOp::FrontMinusBack, ShapeOp::Divide] {
            let mut ed = editor();
            let x = rect(&mut ed, 200.0, 0.0, 10.0, 10.0);
            let a = rect(&mut ed, 0.0, 0.0, 20_000.0, 20_000.0);
            let y = rect(&mut ed, 300.0, 0.0, 10.0, 10.0);
            let b = rect(&mut ed, 10_000.0, 0.0, 20_000.0, 20_000.0);
            let z = rect(&mut ed, 400.0, 0.0, 10.0, 10.0);
            let made = shape_op(&mut ed, &[a, b], op).unwrap();
            let ids = ids_in_order(&ed);
            assert_eq!(ids[..2], [x.clone(), y.clone()], "{op:?}");
            assert_eq!(ids.last(), Some(&z), "{op:?}: Z continua no topo");
            assert!(
                ids[2..ids.len() - 1].iter().all(|i| made.contains(i)),
                "{op:?}"
            );
        }
        let mut ed = editor();
        let a = rect(&mut ed, 0.0, 0.0, 20_000.0, 20_000.0);
        let y = rect(&mut ed, 300.0, 0.0, 10.0, 10.0);
        let b = rect(&mut ed, 10_000.0, 0.0, 20_000.0, 20_000.0);
        let z = rect(&mut ed, 400.0, 0.0, 10.0, 10.0);
        let w = boolean(
            &mut ed,
            &[a, b],
            BoolOp::Weld,
            BoolOp::Weld.default_options(),
        )
        .unwrap();
        assert_eq!(ids_in_order(&ed), vec![y, w[0].clone(), z], "soldar");
    }
}
