//! Edição de nós (doc 03, GD-NOD-01..05). Funções puras sobre `PathData`
//! em coordenadas locais do objeto; a interface converte o cursor para o
//! espaço local antes de chamá-las.

use kurbo::{
    CubicBez, ParamCurve, ParamCurveArclen, ParamCurveNearest, PathEl, PathSeg, Point, Vec2,
};

use crate::geometry::{SegRef, segment, segment_count};
use crate::model::{Node, NodeKind, PathData, Subpath};

/// Referência a um nó.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeRef {
    pub subpath: usize,
    pub index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Handle {
    In,
    Out,
}

fn v(p: [f64; 2]) -> Vec2 {
    Vec2::new(p[0], p[1])
}
fn a(p: Vec2) -> [f64; 2] {
    [p.x, p.y]
}
fn pt(p: [f64; 2]) -> Point {
    Point::new(p[0], p[1])
}

pub fn node(path: &PathData, r: NodeRef) -> Option<&Node> {
    path.subpaths.get(r.subpath)?.nodes.get(r.index)
}

/// Todos os nós do caminho (GD-NOD-01 "todos").
pub fn all_nodes(path: &PathData) -> Vec<NodeRef> {
    path.subpaths
        .iter()
        .enumerate()
        .flat_map(|(k, sp)| {
            (0..sp.nodes.len()).map(move |i| NodeRef {
                subpath: k,
                index: i,
            })
        })
        .collect()
}

/// Nós dentro de um retângulo (seleção por retângulo, GD-NOD-01).
pub fn nodes_in_rect(path: &PathData, r: kurbo::Rect) -> Vec<NodeRef> {
    all_nodes(path)
        .into_iter()
        .filter(|n| r.contains(pt(node(path, *n).unwrap().p)))
        .collect()
}

/// É extremidade de subcaminho aberto?
pub fn is_endpoint(path: &PathData, r: NodeRef) -> bool {
    path.subpaths
        .get(r.subpath)
        .is_some_and(|sp| !sp.closed && (r.index == 0 || r.index + 1 == sp.nodes.len()))
}

// ---- mover (GD-NOD-02) --------------------------------------------------

/// Move nós (e suas alças) por `d`.
pub fn move_nodes(path: &mut PathData, refs: &[NodeRef], d: [f64; 2]) {
    let mut seen = refs.to_vec();
    seen.sort();
    seen.dedup();
    for r in seen {
        if let Some(n) = path
            .subpaths
            .get_mut(r.subpath)
            .and_then(|sp| sp.nodes.get_mut(r.index))
        {
            n.p = [n.p[0] + d[0], n.p[1] + d[1]];
            n.handle_in = n.handle_in.map(|h| [h[0] + d[0], h[1] + d[1]]);
            n.handle_out = n.handle_out.map(|h| [h[0] + d[0], h[1] + d[1]]);
        }
    }
}

/// Move uma alça para `pos`, respeitando o tipo do nó: suave mantém a alça
/// oposta colinear (com seu comprimento); simétrico espelha a oposta.
pub fn move_handle(path: &mut PathData, r: NodeRef, side: Handle, pos: [f64; 2]) {
    let Some(n) = path
        .subpaths
        .get_mut(r.subpath)
        .and_then(|sp| sp.nodes.get_mut(r.index))
    else {
        return;
    };
    let p = v(n.p);
    let dir = v(pos) - p;
    let (this, other) = match side {
        Handle::In => (&mut n.handle_in, &mut n.handle_out),
        Handle::Out => (&mut n.handle_out, &mut n.handle_in),
    };
    *this = Some(pos);
    if dir.hypot() < 1e-12 {
        return;
    }
    match n.kind {
        NodeKind::Corner => {}
        NodeKind::Smooth => {
            if let Some(o) = *other {
                let len = (v(o) - p).hypot();
                *other = Some(a(p - dir.normalize() * len));
            }
        }
        NodeKind::Symmetric => *other = Some(a(p - dir)),
    }
}

/// Arrasta um ponto do segmento (parâmetro `t`) por `d`, mantendo os nós
/// extremos: distribui o deslocamento nas alças pela solução de menor norma.
/// Retas viram curvas.
pub fn drag_segment(path: &mut PathData, s: SegRef, t: f64, d: [f64; 2]) {
    to_curve(path, s);
    let t = t.clamp(0.05, 0.95);
    let b1 = 3.0 * (1.0 - t).powi(2) * t;
    let b2 = 3.0 * (1.0 - t) * t * t;
    let den = b1 * b1 + b2 * b2;
    let (w1, w2) = (b1 / den, b2 / den);
    let sp = &mut path.subpaths[s.subpath];
    let n = sp.nodes.len();
    let (i, j) = (s.index, (s.index + 1) % n);
    let o = sp.nodes[i].handle_out.unwrap_or(sp.nodes[i].p);
    sp.nodes[i].handle_out = Some([o[0] + d[0] * w1, o[1] + d[1] * w1]);
    let h = sp.nodes[j].handle_in.unwrap_or(sp.nodes[j].p);
    sp.nodes[j].handle_in = Some([h[0] + d[0] * w2, h[1] + d[1] * w2]);
    for k in [i, j] {
        enforce_kind(&mut sp.nodes[k], None);
    }
}

// ---- adicionar e excluir (GD-NOD-03) -------------------------------------

/// Divide o segmento em `t`, preservando a forma. Devolve o novo nó.
pub fn add_node(path: &mut PathData, s: SegRef, t: f64) -> Option<NodeRef> {
    let seg = segment(path, s)?;
    let t = t.clamp(1e-6, 1.0 - 1e-6);
    let sp = &mut path.subpaths[s.subpath];
    let n = sp.nodes.len();
    let (i, j) = (s.index, (s.index + 1) % n);
    let new = match seg {
        PathSeg::Line(l) => Node::corner(l.eval(t).x, l.eval(t).y),
        other => {
            let c = other.to_cubic();
            let (l, r) = (c.subsegment(0.0..t), c.subsegment(t..1.0));
            sp.nodes[i].handle_out = Some([l.p1.x, l.p1.y]);
            sp.nodes[j].handle_in = Some([r.p2.x, r.p2.y]);
            let mut m = Node::corner(l.p3.x, l.p3.y);
            m.handle_in = Some([l.p2.x, l.p2.y]);
            m.handle_out = Some([r.p1.x, r.p1.y]);
            m.kind = NodeKind::Smooth;
            m
        }
    };
    sp.nodes.insert(i + 1, new);
    Some(NodeRef {
        subpath: s.subpath,
        index: i + 1,
    })
}

/// Exclui nós; o trecho resultante é reajustado por mínimos quadrados para
/// o menor desvio em relação à forma original (GD-NOD-03).
pub fn delete_nodes(path: &mut PathData, refs: &[NodeRef]) {
    let mut refs = refs.to_vec();
    refs.sort();
    refs.dedup();
    for r in refs.into_iter().rev() {
        delete_node(path, r);
    }
    path.subpaths.retain(|sp| sp.nodes.len() >= 2);
}

fn delete_node(path: &mut PathData, r: NodeRef) {
    let Some(sp) = path.subpaths.get_mut(r.subpath) else {
        return;
    };
    let n = sp.nodes.len();
    if r.index >= n {
        return;
    }
    if n <= 2 || (!sp.closed && (r.index == 0 || r.index + 1 == n)) {
        sp.nodes.remove(r.index);
        if !sp.closed && !sp.nodes.is_empty() {
            sp.nodes[0].handle_in = None;
            let last = sp.nodes.len() - 1;
            sp.nodes[last].handle_out = None;
        }
        return;
    }
    let i = (r.index + n - 1) % n;
    let j = (r.index + 1) % n;
    let before = crate::geometry::segment_between(&sp.nodes[i], &sp.nodes[r.index]);
    let after = crate::geometry::segment_between(&sp.nodes[r.index], &sp.nodes[j]);
    let both_lines = matches!(before, PathSeg::Line(_)) && matches!(after, PathSeg::Line(_));
    if both_lines {
        sp.nodes.remove(r.index);
        return;
    }
    let samples: Vec<Point> = (1..16)
        .map(|k| before.eval(k as f64 / 16.0))
        .chain(std::iter::once(before.eval(1.0)))
        .chain((1..16).map(|k| after.eval(k as f64 / 16.0)))
        .collect();
    let p0 = pt(sp.nodes[i].p);
    let p3 = pt(sp.nodes[j].p);
    let t0 = tangent_dir(sp.nodes[i].handle_out, p0, samples[0]);
    let t1 = tangent_dir(sp.nodes[j].handle_in, p3, *samples.last().unwrap());
    let c = fit_cubic(p0, t0, p3, t1, &samples);
    sp.nodes[i].handle_out = Some([c.p1.x, c.p1.y]);
    sp.nodes[j].handle_in = Some([c.p2.x, c.p2.y]);
    sp.nodes.remove(r.index);
}

fn tangent_dir(handle: Option<[f64; 2]>, p: Point, fallback: Point) -> Vec2 {
    let d = handle
        .map(|h| pt(h) - p)
        .filter(|d| d.hypot() > 1e-9)
        .unwrap_or(fallback - p);
    if d.hypot() > 1e-12 {
        d.normalize()
    } else {
        Vec2::new(1.0, 0.0)
    }
}

/// Ajusta uma cúbica com extremos e direções de tangente fixos aos pontos:
/// mínimos quadrados nos comprimentos das alças (α, β), partindo da
/// parametrização por corda e refinando os parâmetros pelo ponto mais
/// próximo da cúbica a cada iteração.
pub fn fit_cubic(p0: Point, t0: Vec2, p3: Point, t1: Vec2, pts: &[Point]) -> CubicBez {
    let mut chord = vec![0.0];
    let mut prev = p0;
    for p in pts.iter().chain(std::iter::once(&p3)) {
        chord.push(chord.last().unwrap() + (*p - prev).hypot());
        prev = *p;
    }
    let total = chord.last().copied().unwrap_or(1.0).max(1e-12);
    let mut params: Vec<f64> = (0..pts.len()).map(|k| chord[k + 1] / total).collect();
    let dist = (p3 - p0).hypot();
    let mut best = CubicBez::new(p0, p0 + t0 * (dist / 3.0), p3 + t1 * (dist / 3.0), p3);
    for _ in 0..8 {
        let (mut c00, mut c01, mut c11, mut x0, mut x1) = (0.0, 0.0, 0.0, 0.0, 0.0);
        for (p, &u) in pts.iter().zip(&params) {
            let b0 = (1.0 - u).powi(3);
            let b1 = 3.0 * (1.0 - u).powi(2) * u;
            let b2 = 3.0 * (1.0 - u) * u * u;
            let b3 = u.powi(3);
            let a1 = t0 * b1;
            let a2 = t1 * b2;
            let r = p.to_vec2() - (p0.to_vec2() * (b0 + b1) + p3.to_vec2() * (b2 + b3));
            c00 += a1.dot(a1);
            c01 += a1.dot(a2);
            c11 += a2.dot(a2);
            x0 += a1.dot(r);
            x1 += a2.dot(r);
        }
        let det = c00 * c11 - c01 * c01;
        if det.abs() <= 1e-12 {
            break;
        }
        let (alpha, beta) = ((x0 * c11 - x1 * c01) / det, (c00 * x1 - c01 * x0) / det);
        if alpha < 1e-6 * dist || beta < 1e-6 * dist {
            break;
        }
        best = CubicBez::new(p0, p0 + t0 * alpha, p3 + t1 * beta, p3);
        for (u, p) in params.iter_mut().zip(pts) {
            *u = kurbo::ParamCurveNearest::nearest(&best, *p, 1e-9).t;
        }
    }
    best
}

// ---- unir, quebrar, fechar, inverter (GD-NOD-04) ---------------------------

/// Inverte o sentido de um subcaminho (a geometria não muda).
pub fn reverse(path: &mut PathData, subpath: usize) {
    let Some(sp) = path.subpaths.get_mut(subpath) else {
        return;
    };
    if sp.closed && sp.nodes.len() > 1 {
        // Mantém o primeiro nó como início.
        sp.nodes[1..].reverse();
    } else {
        sp.nodes.reverse();
    }
    for n in &mut sp.nodes {
        std::mem::swap(&mut n.handle_in, &mut n.handle_out);
    }
}

/// Fecha (com segmento até o primeiro nó) ou abre (removendo o segmento de
/// fechamento) um subcaminho.
pub fn set_closed(path: &mut PathData, subpath: usize, closed: bool) {
    let Some(sp) = path.subpaths.get_mut(subpath) else {
        return;
    };
    if sp.closed == closed || sp.nodes.len() < 2 {
        return;
    }
    sp.closed = closed;
    if !closed {
        sp.nodes[0].handle_in = None;
        let last = sp.nodes.len() - 1;
        sp.nodes[last].handle_out = None;
    }
}

/// Quebra o caminho num nó: em subcaminho fechado, abre começando e
/// terminando nesse nó; em aberto, divide em dois subcaminhos.
pub fn break_at(path: &mut PathData, r: NodeRef) {
    let Some(sp) = path.subpaths.get_mut(r.subpath) else {
        return;
    };
    let n = sp.nodes.len();
    if r.index >= n {
        return;
    }
    if sp.closed {
        sp.nodes.rotate_left(r.index);
        let mut end = sp.nodes[0].clone();
        end.handle_out = None;
        end.kind = NodeKind::Corner;
        sp.nodes[0].handle_in = None;
        sp.nodes[0].kind = NodeKind::Corner;
        sp.nodes.push(end);
        sp.closed = false;
    } else if r.index > 0 && r.index + 1 < n {
        let mut tail: Vec<Node> = sp.nodes.split_off(r.index);
        let mut end = tail[0].clone();
        end.handle_out = None;
        end.kind = NodeKind::Corner;
        sp.nodes.push(end);
        tail[0].handle_in = None;
        tail[0].kind = NodeKind::Corner;
        let extra = sp.extra.clone();
        path.subpaths.insert(
            r.subpath + 1,
            Subpath {
                closed: false,
                nodes: tail,
                extra,
            },
        );
    }
}

/// Une dois nós extremos. `merge = true` funde num único nó no ponto médio
/// ("unir nós"); `false` liga os dois por um segmento reto ("unir com segmento").
/// Devolve `false` se os nós não forem extremos válidos.
pub fn join(path: &mut PathData, a_ref: NodeRef, b_ref: NodeRef, merge: bool) -> bool {
    if a_ref == b_ref || !is_endpoint(path, a_ref) || !is_endpoint(path, b_ref) {
        return false;
    }
    if a_ref.subpath == b_ref.subpath {
        let sp = &mut path.subpaths[a_ref.subpath];
        if sp.nodes.len() < 2 {
            return false;
        }
        if merge {
            let last = sp.nodes.pop().unwrap();
            let first = &mut sp.nodes[0];
            let mid = (v(first.p) + v(last.p)) * 0.5;
            let shift_first = mid - v(first.p);
            first.p = a(mid);
            first.handle_out = first.handle_out.map(|h| a(v(h) + shift_first));
            first.handle_in = last.handle_in.map(|h| a(v(h) + (mid - v(last.p))));
        }
        sp.closed = true;
        return true;
    }
    // Orienta: subcaminho de `a` termina em `a`; o de `b` começa em `b`.
    if a_ref.index == 0 && path.subpaths[a_ref.subpath].nodes.len() > 1 {
        reverse(path, a_ref.subpath);
    }
    if b_ref.index != 0 {
        reverse(path, b_ref.subpath);
    }
    let mut b = path.subpaths[b_ref.subpath].nodes.clone();
    let sa = &mut path.subpaths[a_ref.subpath].nodes;
    if merge {
        let last = sa.last_mut().unwrap();
        let first = b.remove(0);
        let mid = (v(last.p) + v(first.p)) * 0.5;
        last.handle_in = last.handle_in.map(|h| a(v(h) + (mid - v(last.p))));
        last.handle_out = first.handle_out.map(|h| a(v(h) + (mid - v(first.p))));
        last.p = a(mid);
    }
    sa.extend(b);
    path.subpaths.remove(b_ref.subpath);
    true
}

// ---- segmentos e tipos de nó (GD-NOD-05) ----------------------------------

/// Segmento curvo vira reta (remove as alças envolvidas).
pub fn to_line(path: &mut PathData, s: SegRef) {
    let Some(sp) = path.subpaths.get_mut(s.subpath) else {
        return;
    };
    if s.index >= segment_count(sp) {
        return;
    }
    let n = sp.nodes.len();
    sp.nodes[s.index].handle_out = None;
    sp.nodes[(s.index + 1) % n].handle_in = None;
}

/// Segmento reto vira curva (alças a 1/3 e 2/3, forma inalterada).
pub fn to_curve(path: &mut PathData, s: SegRef) {
    let Some(sp) = path.subpaths.get_mut(s.subpath) else {
        return;
    };
    if s.index >= segment_count(sp) {
        return;
    }
    let n = sp.nodes.len();
    let j = (s.index + 1) % n;
    if sp.nodes[s.index].handle_out.is_some() || sp.nodes[j].handle_in.is_some() {
        return;
    }
    let (p0, p3) = (v(sp.nodes[s.index].p), v(sp.nodes[j].p));
    sp.nodes[s.index].handle_out = Some(a(p0 + (p3 - p0) / 3.0));
    sp.nodes[j].handle_in = Some(a(p0 + (p3 - p0) * (2.0 / 3.0)));
}

/// Altera o tipo do nó e ajusta as alças: suave = colineares (direção
/// média, comprimentos mantidos); simétrico = colineares e iguais.
pub fn set_kind(path: &mut PathData, r: NodeRef, kind: NodeKind) {
    let Some(n) = path
        .subpaths
        .get_mut(r.subpath)
        .and_then(|sp| sp.nodes.get_mut(r.index))
    else {
        return;
    };
    n.kind = kind;
    enforce_kind(n, None);
}

fn enforce_kind(n: &mut Node, _hint: Option<Handle>) {
    let (Some(hi), Some(ho)) = (n.handle_in, n.handle_out) else {
        return;
    };
    let p = v(n.p);
    let di = p - v(hi); // aponta no sentido do caminho
    let dout = v(ho) - p;
    let (li, lo) = (di.hypot(), dout.hypot());
    if li < 1e-12 || lo < 1e-12 {
        return;
    }
    let dir = (di.normalize() + dout.normalize()).normalize();
    let dir = if dir.x.is_finite() {
        dir
    } else {
        dout.normalize()
    };
    match n.kind {
        NodeKind::Corner => {}
        NodeKind::Smooth => {
            n.handle_in = Some(a(p - dir * li));
            n.handle_out = Some(a(p + dir * lo));
        }
        NodeKind::Symmetric => {
            let l = (li + lo) / 2.0;
            n.handle_in = Some(a(p - dir * l));
            n.handle_out = Some(a(p + dir * l));
        }
    }
}

/// Reduzir nós (GD-NOD-07): nos subcaminhos `which` (todos, se vazio),
/// troca os nós entre cantos de verdade por menos curvas, com desvio de no
/// máximo `tol` do desenho original. Cantos (virada acima de
/// `corner_deg`) e pontas de caminho aberto ficam. Devolve quantos nós
/// saíram.
pub fn reduce(path: &mut PathData, which: &[usize], tol: f64, corner_deg: f64) -> usize {
    let before: usize = path.subpaths.iter().map(|s| s.nodes.len()).sum();
    for (si, sp) in path.subpaths.iter_mut().enumerate() {
        if !which.is_empty() && !which.contains(&si) {
            continue;
        }
        if let Some(new) = reduce_subpath(sp, tol, corner_deg) {
            sp.nodes = new;
        }
    }
    let after: usize = path.subpaths.iter().map(|s| s.nodes.len()).sum();
    before.saturating_sub(after)
}

/// Segmento `i` (do nó `i` ao seguinte) como cúbica.
fn seg_cubic(sp: &crate::model::Subpath, i: usize) -> CubicBez {
    let n = sp.nodes.len();
    let (s, e) = (&sp.nodes[i], &sp.nodes[(i + 1) % n]);
    let (p0, p3) = (pt(s.p), pt(e.p));
    CubicBez::new(
        p0,
        s.handle_out.map_or(p0, pt),
        e.handle_in.map_or(p3, pt),
        p3,
    )
}

fn is_line(c: &CubicBez) -> bool {
    c.p1 == c.p0 && c.p2 == c.p3
}

/// Direção de saída/chegada de um segmento (cai na corda se a alça é nula).
fn dir_out(c: &CubicBez) -> Vec2 {
    [c.p1, c.p2, c.p3]
        .iter()
        .map(|q| *q - c.p0)
        .find(|d| d.hypot() > 1e-9)
        .unwrap_or(Vec2::ZERO)
}
fn dir_in(c: &CubicBez) -> Vec2 {
    [c.p2, c.p1, c.p0]
        .iter()
        .map(|q| c.p3 - *q)
        .find(|d| d.hypot() > 1e-9)
        .unwrap_or(Vec2::ZERO)
}

/// As pontas de todas as retas do trecho ficam a até `tol` da reta que
/// liga o começo ao fim.
fn collinear(run: &[CubicBez], tol: f64) -> bool {
    let line = kurbo::Line::new(run[0].p0, run[run.len() - 1].p3);
    run.iter()
        .all(|c| line.nearest(c.p3, 1e-9).distance_sq.sqrt() <= tol)
}

fn reduce_subpath(sp: &crate::model::Subpath, tol: f64, corner_deg: f64) -> Option<Vec<Node>> {
    let n = sp.nodes.len();
    let segs = crate::geometry::segment_count(sp);
    if n < 3 || segs < 2 {
        return None;
    }
    let cubics: Vec<CubicBez> = (0..segs).map(|i| seg_cubic(sp, i)).collect();
    let cos_corner = corner_deg.to_radians().cos();
    // O nó fica se é ponta de caminho aberto ou se o desenho vira nele.
    let fixed = |i: usize| -> bool {
        if !sp.closed && (i == 0 || i == n - 1) {
            return true;
        }
        let (u, w) = (
            dir_in(&cubics[(i + segs - 1) % segs]),
            dir_out(&cubics[i % segs]),
        );
        let (lu, lw) = (u.hypot(), w.hypot());
        lu > 0.0 && lw > 0.0 && u.dot(w) / (lu * lw) < cos_corner
    };
    let mut anchors: Vec<usize> = (0..n).filter(|&i| fixed(i)).collect();
    if anchors.is_empty() {
        anchors.push(0);
    }
    let count = if sp.closed {
        anchors.len()
    } else {
        anchors.len() - 1
    };
    // Cada trecho: nós (o primeiro é o canto do começo) e a alça de
    // chegada no canto do fim.
    let mut runs: Vec<(Vec<Node>, Option<[f64; 2]>)> = Vec::new();
    for k in 0..count {
        let s = anchors[k];
        let e = if k + 1 < anchors.len() {
            anchors[k + 1]
        } else {
            anchors[0] + n
        };
        let run: Vec<CubicBez> = (s..e).map(|i| cubics[i % segs]).collect();
        let mut start = sp.nodes[s].clone();
        start.handle_out = None;
        start.handle_in = None;
        let mut nodes_run = vec![start];
        let mut end_in = None;
        if run.iter().all(is_line) && collinear(&run, tol) {
            // Retas alinhadas viram uma reta só.
        } else if run.len() == 1 {
            let c = run[0];
            if !is_line(&c) {
                nodes_run[0].handle_out = Some(a(c.p1.to_vec2()));
                end_in = Some(a(c.p2.to_vec2()));
            }
        } else {
            // Amostra o desenho original e reajusta com a tolerância.
            let step = (tol * 0.5).max(1e-9);
            let mut pts = vec![run[0].p0];
            for c in &run {
                let k = ((c.arclen(step * 0.1) / step).ceil() as usize).clamp(4, 400);
                pts.extend((1..=k).map(|j| c.eval(j as f64 / k as f64)));
            }
            let t1 = dir_out(&run[0]).normalize();
            let t2 = -dir_in(&run[run.len() - 1]).normalize();
            let fitted = super::fit::fit_open(&pts, t1, t2, tol);
            let els: Vec<PathEl> = fitted.elements().iter().copied().skip(1).collect();
            for (j, el) in els.iter().enumerate() {
                let (h_out, h_in, p) = match *el {
                    PathEl::CurveTo(c1, c2, p) => (Some(a(c1.to_vec2())), Some(a(c2.to_vec2())), p),
                    PathEl::LineTo(p) => (None, None, p),
                    _ => continue,
                };
                nodes_run.last_mut().unwrap().handle_out = h_out;
                if j + 1 < els.len() {
                    let mut nd = Node::corner(p.x, p.y);
                    nd.kind = NodeKind::Smooth;
                    nd.handle_in = h_in;
                    nodes_run.push(nd);
                } else {
                    end_in = h_in;
                }
            }
        }
        runs.push((nodes_run, end_in));
    }
    // A alça de chegada vai para o canto que abre o trecho seguinte.
    for k in 0..runs.len() {
        let h = runs[k].1;
        if k + 1 < runs.len() {
            runs[k + 1].0[0].handle_in = h;
        } else if sp.closed {
            runs[0].0[0].handle_in = h;
        }
    }
    let mut out: Vec<Node> = runs.iter().flat_map(|r| r.0.clone()).collect();
    if !sp.closed {
        let mut last = sp.nodes[n - 1].clone();
        last.handle_out = None;
        last.handle_in = runs.last().and_then(|r| r.1);
        out.push(last);
    }
    (out.len() < n).then_some(out)
}

/// Alinhar nós (GD-NOD-06): `horizontal` põe todos na mesma altura (y),
/// senão na mesma coluna (x), no meio da caixa dos nós escolhidos; as
/// alças acompanham cada nó.
pub fn align_nodes(path: &mut PathData, refs: &[NodeRef], horizontal: bool) {
    let axis = usize::from(horizontal);
    let vals: Vec<f64> = refs
        .iter()
        .filter_map(|r| node(path, *r))
        .map(|n| n.p[axis])
        .collect();
    if vals.len() < 2 {
        return;
    }
    let lo = vals.iter().copied().fold(f64::INFINITY, f64::min);
    let hi = vals.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let target = (lo + hi) / 2.0;
    let mut seen = refs.to_vec();
    seen.sort();
    seen.dedup();
    for r in seen {
        let Some(cur) = node(path, r).map(|n| n.p[axis]) else {
            continue;
        };
        let mut d = [0.0, 0.0];
        d[axis] = target - cur;
        move_nodes(path, &[r], d);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::{self, subpath_segments};
    use crate::model::{FillRule, SubpathBuilder};
    use serde_json::Map;

    fn wave() -> PathData {
        let mut b = SubpathBuilder::default();
        b.move_to([0.0, 0.0]);
        b.curve_to([10.0, 20.0], [30.0, 20.0], [40.0, 0.0]);
        b.curve_to([50.0, -20.0], [70.0, -20.0], [80.0, 0.0]);
        b.line_to([80.0, -30.0]);
        PathData {
            subpaths: b.finish(),
            fill_rule: FillRule::Evenodd,
            extra: Map::new(),
        }
    }

    fn samples(p: &PathData) -> Vec<Point> {
        p.subpaths
            .iter()
            .flat_map(subpath_segments)
            .flat_map(|s| (0..=8).map(move |k| s.eval(k as f64 / 8.0)))
            .collect()
    }

    /// Distância máxima de pontos de `a` à curva `b`.
    fn deviation(a: &PathData, b: &PathData) -> f64 {
        samples(a)
            .into_iter()
            .map(|p| geometry::nearest(b, p).unwrap().distance)
            .fold(0.0, f64::max)
    }

    #[test]
    fn add_node_preserves_shape() {
        let orig = wave();
        let mut p = orig.clone();
        let r = add_node(
            &mut p,
            SegRef {
                subpath: 0,
                index: 0,
            },
            0.3,
        )
        .unwrap();
        assert_eq!(r.index, 1);
        assert_eq!(p.subpaths[0].nodes.len(), 5);
        assert!(deviation(&p, &orig) < 1e-4 && deviation(&orig, &p) < 1e-4);
        // Também em segmento reto.
        add_node(
            &mut p,
            SegRef {
                subpath: 0,
                index: 3,
            },
            0.5,
        )
        .unwrap();
        assert!(deviation(&orig, &p) < 1e-4);
    }

    #[test]
    fn delete_node_refits_with_minimum_deviation() {
        let orig = wave();
        let mut p = orig.clone();
        let r = add_node(
            &mut p,
            SegRef {
                subpath: 0,
                index: 1,
            },
            0.5,
        )
        .unwrap();
        delete_nodes(&mut p, &[r]);
        assert_eq!(p.subpaths[0].nodes.len(), orig.subpaths[0].nodes.len());
        assert!(
            deviation(&orig, &p) < 0.05,
            "desvio {}",
            deviation(&orig, &p)
        );
        // Remover nó do meio entre duas curvas: aproxima as duas por uma.
        let mut q = orig.clone();
        delete_nodes(
            &mut q,
            &[NodeRef {
                subpath: 0,
                index: 1,
            }],
        );
        assert_eq!(q.subpaths[0].nodes.len(), 3);
        let first = &q.subpaths[0].nodes[0];
        assert!(first.handle_out.is_some());
        // Remover extremidade.
        let mut e = orig.clone();
        delete_nodes(
            &mut e,
            &[NodeRef {
                subpath: 0,
                index: 3,
            }],
        );
        assert_eq!(e.subpaths[0].nodes.len(), 3);
        assert!(e.subpaths[0].nodes[2].handle_out.is_none());
    }

    #[test]
    fn reverse_keeps_geometry() {
        let orig = wave();
        let mut p = orig.clone();
        reverse(&mut p, 0);
        assert_eq!(p.subpaths[0].nodes[0].p, [80.0, -30.0]);
        assert!(
            deviation(&orig, &p) < 1e-4 && deviation(&p, &orig) < 1e-4,
            "{} {}",
            deviation(&orig, &p),
            deviation(&p, &orig)
        );
        assert!((geometry::length(&p) - geometry::length(&orig)).abs() < 1e-6);
    }

    #[test]
    fn break_close_and_join() {
        let mut p = wave();
        break_at(
            &mut p,
            NodeRef {
                subpath: 0,
                index: 2,
            },
        );
        assert_eq!(p.subpaths.len(), 2);
        assert_eq!(
            p.subpaths[0].nodes.last().unwrap().p,
            p.subpaths[1].nodes[0].p
        );
        assert!((geometry::length(&p) - geometry::length(&wave())).abs() < 1e-6);
        // Unir de volta fundindo os nós coincidentes.
        let last0 = NodeRef {
            subpath: 0,
            index: p.subpaths[0].nodes.len() - 1,
        };
        assert!(join(
            &mut p,
            last0,
            NodeRef {
                subpath: 1,
                index: 0
            },
            true
        ));
        assert_eq!(p.subpaths.len(), 1);
        assert!(deviation(&wave(), &p) < 1e-4);
        // Fechar e abrir.
        set_closed(&mut p, 0, true);
        assert!(p.subpaths[0].closed);
        let closed_len = geometry::length(&p);
        assert!(closed_len > geometry::length(&wave()));
        set_closed(&mut p, 0, false);
        assert!((geometry::length(&p) - geometry::length(&wave())).abs() < 1e-6);
        // Unir com segmento os extremos do mesmo subcaminho = fechar.
        let n = p.subpaths[0].nodes.len();
        assert!(join(
            &mut p,
            NodeRef {
                subpath: 0,
                index: 0
            },
            NodeRef {
                subpath: 0,
                index: n - 1
            },
            false
        ));
        assert!(p.subpaths[0].closed);
        // Quebrar um fechado abre no nó.
        break_at(
            &mut p,
            NodeRef {
                subpath: 0,
                index: 1,
            },
        );
        assert!(!p.subpaths[0].closed);
        assert_eq!(p.subpaths[0].nodes.len(), n + 1);
    }

    #[test]
    fn join_two_subpaths_with_segment_orients_them() {
        let mut b = SubpathBuilder::default();
        b.move_to([0.0, 0.0]);
        b.line_to([10.0, 0.0]);
        b.move_to([30.0, 0.0]);
        b.line_to([20.0, 0.0]);
        let mut p = PathData {
            subpaths: b.finish(),
            fill_rule: FillRule::Evenodd,
            extra: Map::new(),
        };
        assert!(join(
            &mut p,
            NodeRef {
                subpath: 0,
                index: 1
            },
            NodeRef {
                subpath: 1,
                index: 1
            },
            false
        ));
        let xs: Vec<f64> = p.subpaths[0].nodes.iter().map(|n| n.p[0]).collect();
        assert_eq!(xs, vec![0.0, 10.0, 20.0, 30.0]);
        assert!(
            !join(
                &mut p,
                NodeRef {
                    subpath: 0,
                    index: 1
                },
                NodeRef {
                    subpath: 0,
                    index: 2
                },
                true
            ),
            "nós internos não unem"
        );
    }

    #[test]
    fn segment_conversion_and_kinds() {
        let mut p = wave();
        let s = SegRef {
            subpath: 0,
            index: 2,
        };
        to_curve(&mut p, s);
        assert!(matches!(
            geometry::segment(&p, s).unwrap(),
            PathSeg::Cubic(_)
        ));
        assert!(
            deviation(&wave(), &p) < 1e-4,
            "conversão para curva não muda a forma"
        );
        to_line(
            &mut p,
            SegRef {
                subpath: 0,
                index: 0,
            },
        );
        assert!(matches!(
            geometry::segment(
                &p,
                SegRef {
                    subpath: 0,
                    index: 0
                }
            )
            .unwrap(),
            PathSeg::Line(_)
        ));

        let mut p = wave();
        let r = NodeRef {
            subpath: 0,
            index: 1,
        };
        // Deixa as alças tortas e depois força suave/simétrico.
        p.subpaths[0].nodes[1].handle_out = Some([45.0, -5.0]);
        set_kind(&mut p, r, NodeKind::Smooth);
        let n = &p.subpaths[0].nodes[1];
        let di = v(n.p) - v(n.handle_in.unwrap());
        let dout = v(n.handle_out.unwrap()) - v(n.p);
        assert!(di.cross(dout).abs() < 1e-6 * di.hypot() * dout.hypot());
        set_kind(&mut p, r, NodeKind::Symmetric);
        let n = &p.subpaths[0].nodes[1];
        let li = (v(n.p) - v(n.handle_in.unwrap())).hypot();
        let lo = (v(n.handle_out.unwrap()) - v(n.p)).hypot();
        assert!((li - lo).abs() < 1e-4);
        // Mover alça de nó simétrico espelha a outra.
        move_handle(&mut p, r, Handle::Out, [50.0, 10.0]);
        let n = &p.subpaths[0].nodes[1];
        assert_eq!(n.handle_in, Some([30.0, -10.0]));
    }

    #[test]
    fn drag_segment_moves_point_keeps_ends() {
        let mut p = wave();
        let s = SegRef {
            subpath: 0,
            index: 2,
        };
        let before = geometry::segment(&p, s).unwrap();
        let ends = (before.start(), before.end());
        drag_segment(&mut p, s, 0.5, [5.0, 0.0]);
        let after = geometry::segment(&p, s).unwrap();
        assert_eq!((after.start(), after.end()), ends);
        let moved = after.eval(0.5) - before.eval(0.5);
        assert!((moved - Vec2::new(5.0, 0.0)).hypot() < 1e-4);
    }

    #[test]
    fn move_nodes_moves_handles() {
        let mut p = wave();
        move_nodes(
            &mut p,
            &[
                NodeRef {
                    subpath: 0,
                    index: 1,
                },
                NodeRef {
                    subpath: 0,
                    index: 1,
                },
            ],
            [1.0, 2.0],
        );
        let n = &p.subpaths[0].nodes[1];
        assert_eq!(n.p, [41.0, 2.0]);
        assert_eq!(n.handle_in, Some([31.0, 22.0]));
    }
    /// GD-NOD-07: reduzir nós mantém o desvio ≤ tolerância; cantos e retas
    /// alinhadas viram o mínimo de nós.
    #[test]
    fn gd_nod_07_reduce_nodes() {
        // Círculo de raio 50 desenhado com 64 retas: vira poucas curvas.
        let mut sp = Subpath {
            closed: true,
            nodes: (0..64)
                .map(|i| {
                    let t = i as f64 / 64.0 * std::f64::consts::TAU;
                    Node::corner(50.0 * t.cos(), 50.0 * t.sin())
                })
                .collect(),
            extra: Default::default(),
        };
        // Retas: sem canto de verdade (virada de 5,6° < 10°).
        let mut path = PathData {
            subpaths: vec![sp.clone()],
            fill_rule: Default::default(),
            extra: Default::default(),
        };
        let original = path.clone();
        let removed = reduce(&mut path, &[], 0.5, 10.0);
        assert!(
            path.subpaths[0].nodes.len() <= 8,
            "{}",
            path.subpaths[0].nodes.len()
        );
        assert_eq!(removed, 64 - path.subpaths[0].nodes.len());
        assert!(
            deviation(&original, &path) <= 0.5 + 1e-6,
            "{}",
            deviation(&original, &path)
        );
        assert!(path.subpaths[0].closed);
        // Quadrado com nós a mais nas bordas: só os 4 cantos ficam, e retos.
        sp.nodes = vec![
            Node::corner(0.0, 0.0),
            Node::corner(5.0, 0.0),
            Node::corner(10.0, 0.0),
            Node::corner(10.0, 3.0),
            Node::corner(10.0, 10.0),
            Node::corner(0.0, 10.0),
            Node::corner(0.0, 5.0),
        ];
        let mut sq = PathData {
            subpaths: vec![sp],
            fill_rule: Default::default(),
            extra: Default::default(),
        };
        reduce(&mut sq, &[], 0.01, 10.0);
        let ps: Vec<[f64; 2]> = sq.subpaths[0].nodes.iter().map(|n| n.p).collect();
        assert_eq!(ps, vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]]);
        assert!(
            sq.subpaths[0]
                .nodes
                .iter()
                .all(|n| n.handle_in.is_none() && n.handle_out.is_none())
        );
        // Caminho aberto: as pontas ficam.
        let mut open = PathData {
            subpaths: vec![Subpath {
                closed: false,
                nodes: (0..=20)
                    .map(|i| Node::corner(i as f64, (i as f64 * 0.3).sin() * 4.0))
                    .collect(),
                extra: Default::default(),
            }],
            fill_rule: Default::default(),
            extra: Default::default(),
        };
        let before = open.clone();
        reduce(&mut open, &[], 0.2, 10.0);
        let nodes = &open.subpaths[0].nodes;
        assert!(nodes.len() < 21);
        assert_eq!(nodes[0].p, [0.0, 0.0]);
        assert_eq!(nodes.last().unwrap().p, before.subpaths[0].nodes[20].p);
        assert!(
            deviation(&before, &open) <= 0.2 + 1e-6,
            "{}",
            deviation(&before, &open)
        );
    }

    /// GD-NOD-06: alinhar nós na horizontal põe todos na mesma altura, com
    /// as alças junto.
    #[test]
    fn gd_nod_06_align_nodes() {
        let mut p = wave();
        let refs = all_nodes(&p);
        align_nodes(&mut p, &refs, true);
        let ys: Vec<f64> = p.subpaths[0].nodes.iter().map(|n| n.p[1]).collect();
        assert!(ys.windows(2).all(|w| (w[0] - w[1]).abs() < 1e-9), "{ys:?}");
        let x0 = wave().subpaths[0].nodes[0].p[0];
        assert_eq!(p.subpaths[0].nodes[0].p[0], x0, "x não muda");
    }
}
