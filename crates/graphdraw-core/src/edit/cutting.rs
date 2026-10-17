//! Ferramentas de corte (doc 07): faca (GD-FER-17), recortar (GD-FER-16),
//! borracha (GD-FER-18) e excluir segmento virtual (GD-FER-19).
//!
//! Formas fechadas são cortadas como áreas (booleanas do `i_overlay`, como
//! em [`super::shapeops`]); caminhos abertos são partidos nos parâmetros
//! das interseções e viram pedaços de curva exatos (sem poligonizar).

use i_overlay::core::overlay_rule::OverlayRule;
use i_overlay::mesh::float::stroke::offset::StrokeOffset;
use i_overlay::mesh::float::style::{LineCap, LineJoin, StrokeStyle};
use kurbo::{Affine, BezPath, ParamCurve, PathEl, PathSeg, Point, Rect};

use super::convert::bez_to_subpaths;
use super::history::{Editor, ObjLoc};
use super::query::{self, local_outline};
use super::shapeops::{
    self, Shapes, area, overlay, path_object, region, shapes_to_path, union_all,
};
use crate::geometry::{self, SegRef};
use crate::model::{FillRule, Object, ObjectKind, Page, PathData, Subpath};

/// Lacuna mínima da faca (µm): sem lacuna, as peças ficam encostadas.
pub const MIN_GAP: f64 = 2.0;

/// Pedaço do subcaminho entre os parâmetros globais `u0 < u1` (índice do
/// segmento + t; em subcaminho fechado `u1` pode passar de `n`).
fn piece(segs: &[PathSeg], u0: f64, u1: f64) -> BezPath {
    let n = segs.len();
    let mut b = BezPath::new();
    let mut u = u0;
    let mut started = false;
    while u < u1 - 1e-9 {
        let base = u.floor();
        let i = (base as usize) % n;
        let t0 = u - base;
        let t1 = (u1 - base).min(1.0);
        let s = segs[i].subsegment(t0..t1);
        if !started {
            b.move_to(s.start());
            started = true;
        }
        match s {
            PathSeg::Line(l) => b.line_to(l.p1),
            PathSeg::Quad(q) => b.quad_to(q.p1, q.p2),
            PathSeg::Cubic(c) => b.curve_to(c.p1, c.p2, c.p3),
        }
        u = base + t1;
        if t1 >= 1.0 - 1e-12 {
            u = base + 1.0;
        }
    }
    b
}

/// Intervalos `(u0, u1)` entre os cortes (ordenados) de um subcaminho com
/// `n` segmentos. Fechado sem corte: nenhum (fica inteiro).
fn intervals(n: usize, closed: bool, cuts: &[f64]) -> Vec<(f64, f64)> {
    let nf = n as f64;
    let mut c: Vec<f64> = cuts
        .iter()
        .copied()
        .filter(|u| *u >= 0.0 && *u <= nf)
        .collect();
    c.sort_by(f64::total_cmp);
    c.dedup_by(|a, b| (*a - *b).abs() < 1e-7);
    let mut out = Vec::new();
    if closed {
        if c.is_empty() {
            return out;
        }
        for w in c.windows(2) {
            out.push((w[0], w[1]));
        }
        out.push((*c.last().unwrap(), c[0] + nf));
    } else {
        let mut b = vec![0.0];
        b.extend(c.iter().copied().filter(|u| *u > 1e-7 && *u < nf - 1e-7));
        b.push(nf);
        for w in b.windows(2) {
            out.push((w[0], w[1]));
        }
    }
    out.retain(|(a, b)| b - a > 1e-6);
    out
}

/// Ponto no parâmetro global `u` do subcaminho.
fn eval_at(segs: &[PathSeg], u: f64) -> Point {
    let n = segs.len();
    let base = u.floor().min(n as f64 - 1.0).max(0.0);
    segs[(base as usize) % n].eval((u - base).clamp(0.0, 1.0))
}

/// Contorno do objeto como `PathData` (no espaço local).
fn path_data_of(o: &Object) -> Option<PathData> {
    match &o.kind {
        ObjectKind::Path(p) => Some(p.clone()),
        ObjectKind::Group(_) | ObjectKind::Image(_) | ObjectKind::Text(_) => None,
        _ => Some(PathData {
            subpaths: bez_to_subpaths(&local_outline(o)?),
            fill_rule: FillRule::Nonzero,
            extra: Default::default(),
        }),
    }
}

fn bez_to_pd(b: &BezPath) -> PathData {
    PathData {
        subpaths: bez_to_subpaths(b),
        fill_rule: FillRule::Nonzero,
        extra: Default::default(),
    }
}

/// Objeto tratado como linha (caminho só com subcaminhos abertos): parte
/// nos cortes em vez de recortar área.
fn is_line(o: &Object) -> bool {
    matches!(&o.kind, ObjectKind::Path(p) if !p.subpaths.is_empty() && p.subpaths.iter().all(|s| !s.closed))
}

/// Cortes (parâmetro global por subcaminho) de `pd` com `other`.
fn cuts_with(pd: &PathData, other: &PathData) -> Vec<Vec<f64>> {
    let mut out = vec![Vec::new(); pd.subpaths.len()];
    for (ra, _, hit) in geometry::intersect_paths(pd, other) {
        out[ra.subpath].push(ra.index as f64 + hit.t1);
    }
    out
}

/// Faixa da lâmina/pincel: o traço `pts` (página) com largura `width`,
/// pontas e cantos redondos.
fn band(pts: &[Point], width: f64) -> Shapes {
    let path: Vec<[f64; 2]> = pts.iter().map(|p| [p.x, p.y]).collect();
    if path.len() < 2 {
        return Vec::new();
    }
    let style = StrokeStyle::new(width.max(MIN_GAP))
        .line_join(LineJoin::Round(0.2))
        .start_cap(LineCap::Round(0.2))
        .end_cap(LineCap::Round(0.2));
    union_all([path.stroke(style, false)])
}

fn flat_points(path: &BezPath, tol: f64) -> Vec<Point> {
    let mut v = Vec::new();
    kurbo::flatten(path, tol, |el| match el {
        PathEl::MoveTo(p) | PathEl::LineTo(p) => v.push(p),
        _ => {}
    });
    v
}

/// Aplica `m` a todos os nós e alças.
fn transform_pd(pd: &mut PathData, m: Affine) {
    let t = |p: &mut [f64; 2]| {
        let q = m * Point::new(p[0], p[1]);
        *p = [q.x, q.y];
    };
    for sp in &mut pd.subpaths {
        for n in &mut sp.nodes {
            t(&mut n.p);
            if let Some(h) = &mut n.handle_in {
                t(h);
            }
            if let Some(h) = &mut n.handle_out {
                t(h);
            }
        }
    }
}

/// Objetos de primeiro nível (destravados, em camadas visíveis e
/// destravadas) cujo contorno cruza `line` (página).
pub fn crossed(page: &Page, line: &BezPath) -> Vec<String> {
    let lp = bez_to_pd(line);
    let mut out = Vec::new();
    for layer in &page.layers {
        if !layer.visible || layer.locked {
            continue;
        }
        for o in &layer.objects {
            if o.locked || !o.visible {
                continue;
            }
            let Some(mut pd) = path_data_of(o) else {
                continue;
            };
            let m = o.transform.affine();
            transform_pd(&mut pd, m);
            if !geometry::intersect_paths(&pd, &lp).is_empty() {
                out.push(o.id.clone());
            }
        }
    }
    out
}

fn loc_of(ed: &Editor, id: &str) -> Option<(ObjLoc, Affine)> {
    let r = ed.find(id)?;
    if !r.is_top() {
        return None;
    }
    Some((r.loc, ed.parent_affine(&r)))
}

/// Objeto de caminho (no espaço da página) a partir de formas, com a
/// identidade de `like`.
fn shapes_object(shapes: &Shapes, like: &Object, keep_id: bool) -> Object {
    let mut o = path_object(&shapes_to_path(shapes), like, FillRule::Nonzero);
    if keep_id {
        o.id = like.id.clone();
        o.name = like.name.clone();
    }
    o
}

/// Caminho aberto com os subcaminhos `subs` (espaço local), atributos de
/// `like`.
fn line_object(subs: Vec<Subpath>, like: &Object, keep_id: bool) -> Object {
    let mut o = like.clone();
    if let ObjectKind::Path(p) = &mut o.kind {
        p.subpaths = subs;
    }
    if !keep_id {
        o.id = crate::model::new_id();
    }
    o
}

/// Faca (GD-FER-17): corta os objetos `ids` ao longo de `line` (página). Área
/// fechada vira peças separadas, afastadas por `gap` (µm); linha aberta é
/// partida nos cruzamentos. Devolve os IDs das peças novas; um passo de
/// desfazer.
pub fn knife(ed: &mut Editor, ids: &[String], line: &BezPath, gap: f64) -> Vec<String> {
    let line_pts = flat_points(line, shapeops::FLATTEN_TOL);
    let cutter = band(&line_pts, gap.max(MIN_GAP));
    let mut plan: Vec<(ObjLoc, Vec<Object>)> = Vec::new();
    for id in ids {
        let Some((loc, parent)) = loc_of(ed, id) else {
            continue;
        };
        let o = ed.object(loc).unwrap().clone();
        if is_line(&o) {
            let m = parent * o.transform.affine();
            let mut local = line.clone();
            local.apply_affine(m.inverse());
            let ObjectKind::Path(pd) = &o.kind else {
                continue;
            };
            let cuts = cuts_with(pd, &bez_to_pd(&local));
            if cuts.iter().all(|c| c.is_empty()) {
                continue;
            }
            let mut keep = Vec::new();
            let mut parts = Vec::new();
            for (sp, c) in pd.subpaths.iter().zip(&cuts) {
                if c.is_empty() {
                    keep.push(sp.clone());
                    continue;
                }
                let segs = geometry::subpath_segments(sp);
                for (u0, u1) in intervals(segs.len(), sp.closed, c) {
                    parts.push(bez_to_subpaths(&piece(&segs, u0, u1)));
                }
            }
            let mut objs = Vec::new();
            if !keep.is_empty() {
                objs.push(line_object(keep, &o, true));
            }
            for (k, subs) in parts.into_iter().enumerate() {
                objs.push(line_object(subs, &o, k == 0 && objs.is_empty()));
            }
            plan.push((loc, objs));
            continue;
        }
        if matches!(
            o.kind,
            ObjectKind::Group(_) | ObjectKind::Image(_) | ObjectKind::Text(_)
        ) {
            continue;
        }
        let shapes = overlay(&region(&o, parent), &cutter, OverlayRule::Difference);
        if shapes.len() < 2 {
            continue;
        }
        let objs = shapes
            .iter()
            .enumerate()
            .map(|(k, s)| shapes_object(&vec![s.clone()], &o, k == 0))
            .collect();
        plan.push((loc, objs));
    }
    apply_plan(ed, "Faca", plan)
}

/// Troca cada objeto pelo plano (de trás para a frente, para os índices
/// valerem). Devolve os IDs dos objetos novos/alterados.
fn apply_plan(ed: &mut Editor, name: &str, mut plan: Vec<(ObjLoc, Vec<Object>)>) -> Vec<String> {
    if plan.is_empty() {
        return Vec::new();
    }
    plan.sort_by_key(|a| std::cmp::Reverse((a.0.page, a.0.layer, a.0.index)));
    let mut ids = Vec::new();
    ed.transaction(name, |ed| {
        for (loc, objs) in plan {
            ed.remove_object(loc);
            for (k, o) in objs.into_iter().enumerate() {
                ids.push(o.id.clone());
                ed.insert_object(
                    ObjLoc {
                        index: loc.index + k,
                        ..loc
                    },
                    o,
                );
            }
        }
    });
    ids
}

enum Crop {
    Keep,
    Remove,
    Replace(Box<Object>),
}

fn rect_shapes(r: Rect) -> Shapes {
    union_all([vec![vec![vec![
        [r.x0, r.y0],
        [r.x1, r.y0],
        [r.x1, r.y1],
        [r.x0, r.y1],
    ]]]])
}

fn crop_object(o: &Object, parent: Affine, rect: Rect, skipped: &mut usize) -> Crop {
    if !o.visible || o.locked {
        return Crop::Keep;
    }
    let Some(b) = query::bounds(o, parent, true) else {
        return Crop::Keep;
    };
    if rect.contains_rect(b) {
        return Crop::Keep;
    }
    if !b.overlaps(rect) {
        return Crop::Remove;
    }
    let m = parent * o.transform.affine();
    match &o.kind {
        ObjectKind::Group(g) => {
            let mut children = Vec::new();
            for c in &g.children {
                match crop_object(c, m, rect, skipped) {
                    Crop::Keep => children.push(c.clone()),
                    Crop::Remove => {}
                    Crop::Replace(n) => children.push(*n),
                }
            }
            if children.is_empty() {
                return Crop::Remove;
            }
            let mut n = o.clone();
            if let ObjectKind::Group(g) = &mut n.kind {
                g.children = children;
            }
            Crop::Replace(Box::new(n))
        }
        // Imagens e textos cortados pela metade ficam inteiros (Beta).
        ObjectKind::Image(_) | ObjectKind::Text(_) | ObjectKind::Unknown { .. } => {
            *skipped += 1;
            Crop::Keep
        }
        ObjectKind::Path(pd) if is_line(o) => {
            let mut local = rect_shapes(rect);
            let corners = shapes_to_path(&local);
            local.clear();
            let mut lr = corners;
            lr.apply_affine(m.inverse());
            let cuts = cuts_with(pd, &bez_to_pd(&lr));
            let mut subs = Vec::new();
            for (sp, c) in pd.subpaths.iter().zip(&cuts) {
                let segs = geometry::subpath_segments(sp);
                if segs.is_empty() {
                    continue;
                }
                let iv = if c.is_empty() {
                    vec![(0.0, segs.len() as f64)]
                } else {
                    intervals(segs.len(), sp.closed, c)
                };
                for (u0, u1) in iv {
                    if rect.contains(m * eval_at(&segs, (u0 + u1) / 2.0)) {
                        subs.extend(bez_to_subpaths(&piece(&segs, u0, u1)));
                    }
                }
            }
            if subs.is_empty() {
                Crop::Remove
            } else {
                Crop::Replace(Box::new(line_object(subs, o, true)))
            }
        }
        _ => {
            let shapes = overlay(
                &region(o, parent),
                &rect_shapes(rect),
                OverlayRule::Intersect,
            );
            if area(&shapes) <= 0.0 {
                return Crop::Remove;
            }
            let mut path = shapes_to_path(&shapes);
            path.apply_affine(parent.inverse());
            let mut n = path_object(&path, o, FillRule::Nonzero);
            n.id = o.id.clone();
            n.name = o.name.clone();
            Crop::Replace(Box::new(n))
        }
    }
}

/// Recortar (GD-FER-16): tudo o que está fora de `rect` (página) sai da
/// página `page`; o que cruza a borda é aparado. Devolve (alterados,
/// removidos, mantidos inteiros por não poderem ser aparados — imagens e
/// textos). Um passo de desfazer.
pub fn crop(ed: &mut Editor, page: usize, rect: Rect) -> (usize, usize, usize) {
    let Some(pg) = ed.doc.data.pages.get(page) else {
        return (0, 0, 0);
    };
    let mut skipped = 0;
    let mut ops: Vec<(ObjLoc, Crop)> = Vec::new();
    for (l, layer) in pg.layers.iter().enumerate() {
        if !layer.visible || layer.locked {
            continue;
        }
        for (i, o) in layer.objects.iter().enumerate() {
            let c = crop_object(o, Affine::IDENTITY, rect, &mut skipped);
            if !matches!(c, Crop::Keep) {
                ops.push((
                    ObjLoc {
                        page,
                        layer: l,
                        index: i,
                    },
                    c,
                ));
            }
        }
    }
    let (mut changed, mut removed) = (0, 0);
    if ops.is_empty() {
        return (0, 0, skipped);
    }
    ed.transaction("Recortar", |ed| {
        for (loc, c) in ops.into_iter().rev() {
            match c {
                Crop::Remove => {
                    ed.remove_object(loc);
                    removed += 1;
                }
                Crop::Replace(n) => {
                    ed.replace_object(loc, *n);
                    changed += 1;
                }
                Crop::Keep => {}
            }
        }
    });
    (changed, removed, skipped)
}

/// Pontos a cada `step` (µm) no máximo, para o teste contra o pincel.
fn densify(pts: &[Point], step: f64) -> Vec<Point> {
    let mut out = Vec::with_capacity(pts.len());
    for w in pts.windows(2) {
        let (a, b) = (w[0], w[1]);
        let n = ((b - a).hypot() / step).ceil().max(1.0) as usize;
        for k in 0..n {
            out.push(a.lerp(b, k as f64 / n as f64));
        }
    }
    if let Some(l) = pts.last() {
        out.push(*l);
    }
    out
}

/// Ponto dentro das formas (par-ímpar sobre todos os contornos).
fn inside_shapes(shapes: &Shapes, p: Point) -> bool {
    let mut inside = false;
    for c in shapes.iter().flatten() {
        let n = c.len();
        for i in 0..n {
            let (a, b) = (c[i], c[(i + 1) % n]);
            if (a[1] > p.y) != (b[1] > p.y) {
                let x = a[0] + (p.y - a[1]) / (b[1] - a[1]) * (b[0] - a[0]);
                if p.x < x {
                    inside = !inside;
                }
            }
        }
    }
    inside
}

/// Borracha (GD-FER-18): apaga dos objetos `ids` a área sob o pincel
/// `brush` (pontos da página) de largura `width`. Formas perdem a área;
/// linhas perdem os trechos dentro do pincel. Devolve quantos mudaram; um
/// passo de desfazer.
pub fn erase(ed: &mut Editor, ids: &[String], brush: &[Point], width: f64) -> usize {
    let cutter = band(brush, width);
    if cutter.is_empty() {
        return 0;
    }
    let reach = brush
        .iter()
        .fold(Rect::from_points(brush[0], brush[0]), |r, p| r.union_pt(*p))
        .inflate(width, width);
    let mut plan: Vec<(ObjLoc, Vec<Object>)> = Vec::new();
    for id in ids {
        let Some((loc, parent)) = loc_of(ed, id) else {
            continue;
        };
        let o = ed.object(loc).unwrap().clone();
        if matches!(
            o.kind,
            ObjectKind::Group(_) | ObjectKind::Image(_) | ObjectKind::Text(_)
        ) {
            continue;
        }
        if query::bounds(&o, parent, true).is_none_or(|b| !b.overlaps(reach)) {
            continue;
        }
        let m = parent * o.transform.affine();
        if is_line(&o) {
            let ObjectKind::Path(pd) = &o.kind else {
                continue;
            };
            let mut subs = Vec::new();
            let mut changed = false;
            for sp in &pd.subpaths {
                let mut b = crate::model::geom::subpaths_to_bez(std::slice::from_ref(sp));
                b.apply_affine(m);
                let pts = densify(&flat_points(&b, 5.0), (width / 4.0).max(10.0));
                let mut run: Vec<Point> = Vec::new();
                let flush = |run: &mut Vec<Point>, subs: &mut Vec<Subpath>| {
                    if run.len() >= 2 {
                        let mut path = BezPath::new();
                        path.move_to(m.inverse() * run[0]);
                        for p in &run[1..] {
                            path.line_to(m.inverse() * *p);
                        }
                        subs.extend(bez_to_subpaths(&path));
                    }
                    run.clear();
                };
                for p in pts {
                    if inside_shapes(&cutter, p) {
                        changed = true;
                        flush(&mut run, &mut subs);
                    } else {
                        run.push(p);
                    }
                }
                flush(&mut run, &mut subs);
            }
            if !changed {
                continue;
            }
            let objs = if subs.is_empty() {
                Vec::new()
            } else {
                vec![line_object(subs, &o, true)]
            };
            plan.push((loc, objs));
            continue;
        }
        let before = region(&o, parent);
        let after = overlay(&before, &cutter, OverlayRule::Difference);
        if (area(&after) - area(&before)).abs() < (area(&before) * 1e-6).max(1.0) {
            continue;
        }
        let objs = if area(&after) <= 0.0 {
            Vec::new()
        } else {
            vec![shapes_object(&after, &o, true)]
        };
        plan.push((loc, objs));
    }
    let n = plan.len();
    apply_plan(ed, "Borracha", plan);
    n
}

/// Excluir segmento virtual (GD-FER-19): no caminho `id`, apaga o trecho em
/// volta de `p` (página) entre os cruzamentos mais próximos — com ele mesmo
/// e com os outros objetos da página; sem cruzamento, entre os nós do
/// segmento. Formas viram curvas. `false` se nada mudou.
pub fn delete_virtual_segment(ed: &mut Editor, page: usize, id: &str, p: Point, tol: f64) -> bool {
    let Some((loc, parent)) = loc_of(ed, id) else {
        return false;
    };
    let o = ed.object(loc).unwrap().clone();
    let Some(pd) = path_data_of(&o) else {
        return false;
    };
    let m = parent * o.transform.affine();
    let lp = m.inverse() * p;
    let scale = m.determinant().abs().sqrt().max(1e-12);
    let Some(near) = geometry::nearest(&pd, lp).filter(|n| n.distance * scale <= tol) else {
        return false;
    };
    let SegRef { subpath: si, index } = near.seg;
    let u_click = index as f64 + near.t;
    // Cortes no subcaminho clicado: com o próprio caminho e com os outros.
    let mut cuts: Vec<f64> = Vec::new();
    for (ra, rb, hit) in geometry::intersect_paths(&pd, &pd) {
        if ra.subpath == si {
            cuts.push(ra.index as f64 + hit.t1);
        }
        if rb.subpath == si {
            cuts.push(rb.index as f64 + hit.t2);
        }
    }
    let inv = m.inverse();
    if let Some(pg) = ed.doc.data.pages.get(page) {
        for layer in &pg.layers {
            if !layer.visible {
                continue;
            }
            for other in &layer.objects {
                if other.id == o.id {
                    continue;
                }
                let Some(mut opd) = path_data_of(other) else {
                    continue;
                };
                let om = inv * other.transform.affine();
                transform_pd(&mut opd, om);
                for (ra, _, hit) in geometry::intersect_paths(&pd, &opd) {
                    if ra.subpath == si {
                        cuts.push(ra.index as f64 + hit.t1);
                    }
                }
            }
        }
    }
    let sp = &pd.subpaths[si];
    let segs = geometry::subpath_segments(sp);
    let nf = segs.len() as f64;
    let eps = 1e-7;
    cuts.retain(|u| (*u - u_click).abs() > eps);
    // Sem cruzamento: os nós do segmento clicado.
    let (lo, hi) = if cuts.is_empty() {
        (index as f64, index as f64 + 1.0)
    } else if sp.closed {
        let below = cuts
            .iter()
            .copied()
            .filter(|u| *u < u_click)
            .fold(f64::NEG_INFINITY, f64::max);
        let above = cuts
            .iter()
            .copied()
            .filter(|u| *u > u_click)
            .fold(f64::INFINITY, f64::min);
        let lo = if below.is_finite() {
            below
        } else {
            cuts.iter().copied().fold(f64::NEG_INFINITY, f64::max) - nf
        };
        let hi = if above.is_finite() {
            above
        } else {
            cuts.iter().copied().fold(f64::INFINITY, f64::min) + nf
        };
        (lo, hi)
    } else {
        let lo = cuts
            .iter()
            .copied()
            .filter(|u| *u < u_click)
            .fold(0.0, f64::max);
        let hi = cuts
            .iter()
            .copied()
            .filter(|u| *u > u_click)
            .fold(nf, f64::min);
        (lo, hi)
    };
    let mut rest: Vec<BezPath> = Vec::new();
    if sp.closed {
        // Fica o resto da volta, aberto: de `hi` até `lo` + n.
        if hi < lo + nf - eps {
            let (a, b) = if hi >= nf {
                (hi - nf, lo)
            } else {
                (hi, lo + nf)
            };
            rest.push(piece(&segs, a, b));
        }
    } else {
        if lo > eps {
            rest.push(piece(&segs, 0.0, lo));
        }
        if hi < nf - eps {
            rest.push(piece(&segs, hi, nf));
        }
    }
    let mut subs: Vec<Subpath> = Vec::new();
    for (k, s) in pd.subpaths.iter().enumerate() {
        if k == si {
            for r in &rest {
                subs.extend(bez_to_subpaths(r));
            }
        } else {
            subs.push(s.clone());
        }
    }
    let mut n = o.clone();
    n.kind = ObjectKind::Path(PathData {
        subpaths: subs,
        fill_rule: pd.fill_rule,
        extra: pd.extra.clone(),
    });
    let empty = matches!(&n.kind, ObjectKind::Path(p) if p.subpaths.is_empty());
    ed.transaction("Excluir segmento virtual", |ed| {
        if empty {
            ed.remove_object(loc);
        } else {
            ed.replace_object(loc, n);
        }
    });
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Color, Document, Fill, Node, RectData, Stroke, Transform};
    use serde_json::Map;

    fn setup(objs: Vec<Object>) -> (Editor, Vec<String>) {
        let mut d = Document::new();
        d.data.pages.push(Page::new(300_000.0, 300_000.0));
        let mut ed = Editor::new(d);
        let ids = objs
            .into_iter()
            .map(|o| super::super::ops::add_object(&mut ed, 0, 0, o, "o"))
            .collect();
        (ed, ids)
    }

    fn rect(x: f64, y: f64, w: f64, h: f64) -> Object {
        let mut o = Object::new(ObjectKind::Rectangle(RectData {
            width: w,
            height: h,
            corners: None,
            extra: Map::new(),
        }));
        o.transform = Transform::translate(x, y);
        o.fill = Some(Fill::solid(Color::black()));
        o
    }

    fn line(pts: &[(f64, f64)]) -> Object {
        let mut o = Object::new(ObjectKind::Path(PathData {
            subpaths: vec![Subpath {
                nodes: pts.iter().map(|p| Node::corner(p.0, p.1)).collect(),
                closed: false,
                extra: Map::new(),
            }],
            fill_rule: FillRule::Nonzero,
            extra: Map::new(),
        }));
        o.stroke = Some(Stroke::new(500.0, Color::black()));
        o
    }

    fn seg(a: (f64, f64), b: (f64, f64)) -> BezPath {
        let mut p = BezPath::new();
        p.move_to(a);
        p.line_to(b);
        p
    }

    fn objects(ed: &Editor) -> Vec<Object> {
        ed.objects(0, 0).to_vec()
    }

    fn bounds(o: &Object) -> Rect {
        query::bounds(o, Affine::IDENTITY, false).unwrap()
    }

    /// GD-FER-17: a faca atravessa o quadrado e deixa duas peças com a
    /// lacuna; linha aberta parte no cruzamento; um passo de desfazer.
    #[test]
    fn gd_fer_17_knife_splits_shapes_and_lines() {
        let (mut ed, ids) = setup(vec![rect(0.0, 0.0, 100_000.0, 100_000.0)]);
        let undo = ed.undo_len();
        let cut = seg((50_000.0, -10_000.0), (50_000.0, 110_000.0));
        let made = knife(&mut ed, &ids, &cut, 2_000.0);
        assert_eq!(made.len(), 2);
        assert_eq!(ed.undo_len(), undo + 1);
        let mut xs: Vec<Rect> = objects(&ed).iter().map(bounds).collect();
        xs.sort_by(|a, b| a.x0.total_cmp(&b.x0));
        assert!(
            (xs[0].x1 - 49_000.0).abs() < 5.0 && (xs[1].x0 - 51_000.0).abs() < 5.0,
            "{xs:?}"
        );
        assert!(
            objects(&ed).iter().all(|o| o.fill.is_some()),
            "atributos ficam"
        );
        // Faca que não atravessa: nada muda.
        let (mut ed2, ids2) = setup(vec![rect(0.0, 0.0, 100_000.0, 100_000.0)]);
        assert!(
            knife(
                &mut ed2,
                &ids2,
                &seg((50_000.0, -10_000.0), (50_000.0, 50_000.0)),
                0.0
            )
            .is_empty()
        );
        // Linha aberta: dois pedaços.
        let (mut ed3, ids3) = setup(vec![line(&[(0.0, 0.0), (100_000.0, 0.0)])]);
        let made = knife(
            &mut ed3,
            &ids3,
            &seg((30_000.0, -5_000.0), (30_000.0, 5_000.0)),
            0.0,
        );
        assert_eq!(made.len(), 2);
        let mut ws: Vec<f64> = objects(&ed3).iter().map(|o| bounds(o).width()).collect();
        ws.sort_by(f64::total_cmp);
        assert!(
            (ws[0] - 30_000.0).abs() < 1.0 && (ws[1] - 70_000.0).abs() < 1.0,
            "{ws:?}"
        );
        assert_eq!(
            crossed(
                &ed3.doc.data.pages[0],
                &seg((60_000.0, -1.0), (60_000.0, 1.0))
            )
            .len(),
            1
        );
    }

    /// GD-FER-16: o que está fora sai, o que cruza a borda é aparado, o de
    /// dentro fica; linha aberta é aparada na borda.
    #[test]
    fn gd_fer_16_crop_rect() {
        let (mut ed, ids) = setup(vec![
            rect(10_000.0, 10_000.0, 20_000.0, 20_000.0),
            rect(150_000.0, 150_000.0, 10_000.0, 10_000.0),
            rect(80_000.0, 10_000.0, 40_000.0, 20_000.0),
            line(&[(50_000.0, 50_000.0), (150_000.0, 50_000.0)]),
        ]);
        let (changed, removed, skipped) =
            crop(&mut ed, 0, Rect::new(0.0, 0.0, 100_000.0, 100_000.0));
        assert_eq!((changed, removed, skipped), (2, 1, 0));
        let find = |id: &str| objects(&ed).into_iter().find(|o| o.id == id);
        assert!(find(&ids[1]).is_none(), "fora sai");
        assert_eq!(
            bounds(&find(&ids[0]).unwrap()),
            Rect::new(10_000.0, 10_000.0, 30_000.0, 30_000.0)
        );
        let cut = bounds(&find(&ids[2]).unwrap());
        assert!(
            (cut.x1 - 100_000.0).abs() < 5.0 && (cut.x0 - 80_000.0).abs() < 5.0,
            "{cut:?}"
        );
        let l = bounds(&find(&ids[3]).unwrap());
        assert!(
            (l.x1 - 100_000.0).abs() < 1.0 && (l.x0 - 50_000.0).abs() < 1.0,
            "{l:?}"
        );
        ed.undo();
        assert_eq!(objects(&ed).len(), 4);
    }

    /// GD-FER-18: o pincel abre um furo no quadrado e tira um trecho da linha.
    #[test]
    fn gd_fer_18_eraser() {
        let (mut ed, ids) = setup(vec![
            rect(0.0, 0.0, 100_000.0, 100_000.0),
            line(&[(0.0, 200_000.0), (100_000.0, 200_000.0)]),
        ]);
        let a0 = area(&region(&objects(&ed)[0], Affine::IDENTITY));
        let brush = [
            Point::new(50_000.0, 40_000.0),
            Point::new(50_000.0, 60_000.0),
        ];
        assert_eq!(erase(&mut ed, &ids, &brush, 10_000.0), 1);
        let a1 = area(&region(&objects(&ed)[0], Affine::IDENTITY));
        assert!(a1 < a0 - 200_000_000.0, "{a0} → {a1}");
        let brush2 = [
            Point::new(50_000.0, 190_000.0),
            Point::new(50_000.0, 210_000.0),
        ];
        assert_eq!(erase(&mut ed, &ids, &brush2, 10_000.0), 1);
        let o = objects(&ed).into_iter().find(|o| o.id == ids[1]).unwrap();
        let ObjectKind::Path(pd) = &o.kind else {
            panic!()
        };
        assert_eq!(pd.subpaths.len(), 2, "dois trechos");
    }

    /// GD-FER-19: numa cruz, clicar no braço direito da horizontal apaga só
    /// o trecho além do cruzamento.
    #[test]
    fn gd_fer_19_delete_virtual_segment() {
        let (mut ed, ids) = setup(vec![
            line(&[(0.0, 50_000.0), (100_000.0, 50_000.0)]),
            line(&[(40_000.0, 0.0), (40_000.0, 100_000.0)]),
        ]);
        assert!(delete_virtual_segment(
            &mut ed,
            0,
            &ids[0],
            Point::new(80_000.0, 50_000.0),
            1_000.0
        ));
        let o = objects(&ed).into_iter().find(|o| o.id == ids[0]).unwrap();
        let b = bounds(&o);
        assert!((b.x1 - 40_000.0).abs() < 1.0 && b.x0.abs() < 1.0, "{b:?}");
        // Sem cruzamento: o segmento inteiro sai (e o objeto vazio também).
        let (mut ed2, ids2) = setup(vec![line(&[(0.0, 0.0), (10_000.0, 0.0)])]);
        assert!(delete_virtual_segment(
            &mut ed2,
            0,
            &ids2[0],
            Point::new(5_000.0, 0.0),
            1_000.0
        ));
        assert!(objects(&ed2).is_empty());
        // Retângulo: o lado clicado sai; vira caminho aberto.
        let (mut ed3, ids3) = setup(vec![rect(0.0, 0.0, 10_000.0, 10_000.0)]);
        assert!(delete_virtual_segment(
            &mut ed3,
            0,
            &ids3[0],
            Point::new(5_000.0, 0.0),
            1_000.0
        ));
        let o = &objects(&ed3)[0];
        let ObjectKind::Path(pd) = &o.kind else {
            panic!("{:?}", o.kind)
        };
        assert!(!pd.subpaths[0].closed);
        assert_eq!(geometry::segment_count(&pd.subpaths[0]), 3);
    }
}
