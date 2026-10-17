//! Largura variável do contorno (GD-VAR-01, GD-OUT-08): nós de largura ao
//! longo do caminho (posição pelo comprimento, meia espessura de cada lado),
//! interpolados suavemente. O contorno vira a área entre as duas bordas.

use kurbo::{BezPath, PathEl, Point, Vec2};

use crate::model::WidthNode;

/// Perfis prontos: nome e nós.
pub const PRESETS: [&str; 4] = ["taper-end", "taper-start", "taper-both", "bulge"];

/// Nós do perfil pronto `name`.
pub fn preset(name: &str) -> Vec<WidthNode> {
    let n = |at: f64, k: f64| WidthNode {
        at,
        left: k,
        right: k,
    };
    match name {
        "taper-end" => vec![n(0.0, 1.0), n(1.0, 0.0)],
        "taper-start" => vec![n(0.0, 0.0), n(1.0, 1.0)],
        "taper-both" => vec![n(0.0, 0.0), n(0.5, 1.0), n(1.0, 0.0)],
        "bulge" => vec![n(0.0, 0.3), n(0.5, 1.6), n(1.0, 0.3)],
        _ => Vec::new(),
    }
}

/// Multiplicadores (esquerda, direita) em `s` (0–1), suaves entre os nós.
pub fn factors(nodes: &[WidthNode], s: f64) -> (f64, f64) {
    let mut v: Vec<&WidthNode> = nodes.iter().collect();
    v.sort_by(|a, b| a.at.total_cmp(&b.at));
    let (Some(first), Some(last)) = (v.first(), v.last()) else {
        return (1.0, 1.0);
    };
    if s <= first.at {
        return (first.left, first.right);
    }
    if s >= last.at {
        return (last.left, last.right);
    }
    for w in v.windows(2) {
        let (a, b) = (w[0], w[1]);
        if s >= a.at && s <= b.at {
            let t = if b.at > a.at {
                (s - a.at) / (b.at - a.at)
            } else {
                0.0
            };
            let t = t * t * (3.0 - 2.0 * t);
            return (
                a.left + (b.left - a.left) * t,
                a.right + (b.right - a.right) * t,
            );
        }
    }
    (last.left, last.right)
}

/// Trechos do caminho achatados: pontos e se fechado.
fn polylines(path: &BezPath, tol: f64) -> Vec<(Vec<Point>, bool)> {
    let mut out = Vec::new();
    let mut cur: Vec<Point> = Vec::new();
    let flush = |cur: &mut Vec<Point>, closed: bool, out: &mut Vec<(Vec<Point>, bool)>| {
        cur.dedup_by(|a, b| (*a - *b).hypot() < 1e-9);
        if closed && cur.len() > 2 && (cur[0] - *cur.last().unwrap()).hypot() < 1e-9 {
            cur.pop();
        }
        if cur.len() >= 2 {
            out.push((std::mem::take(cur), closed));
        }
        cur.clear();
    };
    kurbo::flatten(path, tol, |el| match el {
        PathEl::MoveTo(p) => {
            flush(&mut cur, false, &mut out);
            cur.push(p);
        }
        PathEl::LineTo(p) => cur.push(p),
        PathEl::ClosePath => flush(&mut cur, true, &mut out),
        _ => {}
    });
    flush(&mut cur, false, &mut out);
    out
}

/// Pontos extras para a largura mudar ao longo de trechos retos: no
/// mínimo ~200 amostras por trecho.
fn densify(pts: &[Point], closed: bool) -> Vec<Point> {
    let (_, total) = lengths(pts, closed);
    let step = (total / 200.0).max(1e-6);
    let n = pts.len();
    let segs = if closed { n } else { n - 1 };
    let mut out = Vec::with_capacity(n + 200);
    for i in 0..segs {
        let (a, b) = (pts[i], pts[(i + 1) % n]);
        let k = ((b - a).hypot() / step).ceil().max(1.0) as usize;
        for j in 0..k {
            out.push(a + (b - a) * (j as f64 / k as f64));
        }
    }
    if !closed {
        out.push(pts[n - 1]);
    }
    out
}

/// Comprimentos acumulados (o último fecha o laço, se fechado).
fn lengths(pts: &[Point], closed: bool) -> (Vec<f64>, f64) {
    let mut acc = vec![0.0];
    for w in pts.windows(2) {
        acc.push(acc.last().unwrap() + (w[1] - w[0]).hypot());
    }
    let total = acc.last().unwrap()
        + if closed {
            (pts[0] - *pts.last().unwrap()).hypot()
        } else {
            0.0
        };
    (acc, total)
}

/// Área do contorno de espessura `width` com os nós `nodes` ao longo de
/// `path` (o comprimento conta por trecho).
pub fn outline(path: &BezPath, width: f64, nodes: &[WidthNode]) -> BezPath {
    let half = width / 2.0;
    let tol = (width / 40.0).clamp(0.5, 50.0);
    let mut out = BezPath::new();
    for (pts, closed) in polylines(path, tol) {
        let pts = densify(&pts, closed);
        let n = pts.len();
        let (acc, total) = lengths(&pts, closed);
        if total <= 0.0 {
            continue;
        }
        let dir = |i: usize| -> (Vec2, f64) {
            let prev = if i > 0 {
                Some(pts[i] - pts[i - 1])
            } else if closed {
                Some(pts[0] - pts[n - 1])
            } else {
                None
            };
            let next = if i + 1 < n {
                Some(pts[i + 1] - pts[i])
            } else if closed {
                Some(pts[0] - pts[i])
            } else {
                None
            };
            let u = |v: Vec2| v / v.hypot().max(1e-12);
            // Direção média e o fator de quina (até 4×, como o limite de
            // quina padrão).
            match (prev, next) {
                (Some(a), Some(b)) => {
                    let d = u(a) + u(b);
                    if d.hypot() < 1e-9 {
                        (u(b), 1.0)
                    } else {
                        let d = u(d);
                        (d, (1.0 / d.dot(u(b)).max(0.25)).min(4.0))
                    }
                }
                (Some(a), None) => (u(a), 1.0),
                (None, Some(b)) => (u(b), 1.0),
                (None, None) => (Vec2::new(1.0, 0.0), 1.0),
            }
        };
        let mut left = Vec::with_capacity(n);
        let mut right = Vec::with_capacity(n);
        for i in 0..n {
            let (d, miter) = dir(i);
            let normal = Vec2::new(-d.y, d.x) * miter;
            let (l, r) = factors(nodes, acc[i] / total);
            left.push(pts[i] + normal * (l.max(0.0) * half));
            right.push(pts[i] - normal * (r.max(0.0) * half));
        }
        if closed {
            // Duas bordas: a de fora num sentido, a de dentro no outro.
            out.move_to(left[0]);
            for p in &left[1..] {
                out.line_to(*p);
            }
            out.close_path();
            out.move_to(*right.last().unwrap());
            for p in right.iter().rev().skip(1) {
                out.line_to(*p);
            }
            out.close_path();
        } else {
            out.move_to(left[0]);
            for p in &left[1..] {
                out.line_to(*p);
            }
            for p in right.iter().rev() {
                out.line_to(*p);
            }
            out.close_path();
        }
    }
    out
}

/// Posição (0–1 do comprimento do trecho) do ponto do caminho mais perto
/// de `p` e a distância até ele.
pub fn fraction_at(path: &BezPath, p: Point) -> Option<(f64, f64)> {
    let mut best: Option<(f64, f64)> = None;
    for (pts, closed) in polylines(path, 1.0) {
        let (acc, total) = lengths(&pts, closed);
        if total <= 0.0 {
            continue;
        }
        let n = pts.len();
        let segs = if closed { n } else { n - 1 };
        for i in 0..segs {
            let (a, b) = (pts[i], pts[(i + 1) % n]);
            let ab = b - a;
            let len2 = ab.hypot2();
            let t = if len2 > 0.0 {
                ((p - a).dot(ab) / len2).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let q = a + ab * t;
            let d = (p - q).hypot();
            if best.is_none_or(|x| d < x.1) {
                best = Some(((acc[i] + ab.hypot() * t) / total, d));
            }
        }
    }
    best
}

/// Ponto em `s` (0–1 do comprimento) do primeiro trecho do caminho.
pub fn point_at(path: &BezPath, s: f64) -> Option<Point> {
    let (pts, closed) = polylines(path, 1.0).into_iter().next()?;
    let (acc, total) = lengths(&pts, closed);
    let target = s.clamp(0.0, 1.0) * total;
    let n = pts.len();
    let segs = if closed { n } else { n - 1 };
    for i in 0..segs {
        let (a, b) = (pts[i], pts[(i + 1) % n]);
        let len = (b - a).hypot();
        if target <= acc[i] + len || i + 1 == segs {
            let t = if len > 0.0 {
                ((target - acc[i]) / len).clamp(0.0, 1.0)
            } else {
                0.0
            };
            return Some(a + (b - a) * t);
        }
    }
    pts.first().copied()
}

/// Põe (ou ajusta, se houver um a menos de 3%) o nó em `at` com o
/// multiplicador `k` dos dois lados. Sem nós, começa com as pontas em 1.
pub fn set_node(nodes: &mut Vec<WidthNode>, at: f64, k: f64) {
    if nodes.is_empty() {
        nodes.push(WidthNode {
            at: 0.0,
            left: 1.0,
            right: 1.0,
        });
        nodes.push(WidthNode {
            at: 1.0,
            left: 1.0,
            right: 1.0,
        });
    }
    let k = k.max(0.0);
    if let Some(n) = nodes.iter_mut().find(|n| (n.at - at).abs() < 0.03) {
        n.left = k;
        n.right = k;
    } else {
        nodes.push(WidthNode {
            at,
            left: k,
            right: k,
        });
        nodes.sort_by(|a, b| a.at.total_cmp(&b.at));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kurbo::Shape;

    fn line() -> BezPath {
        let mut p = BezPath::new();
        p.move_to((0.0, 0.0));
        p.line_to((1000.0, 0.0));
        p
    }

    /// GD-VAR-01: afinar no fim — larga no início, zero no fim, suave no
    /// meio; área de um triângulo suavizado ~ metade do retângulo.
    #[test]
    fn gd_var_01_taper() {
        let nodes = preset("taper-end");
        assert_eq!(factors(&nodes, 0.0), (1.0, 1.0));
        assert_eq!(factors(&nodes, 1.0), (0.0, 0.0));
        assert!((factors(&nodes, 0.5).0 - 0.5).abs() < 1e-9);
        let o = outline(&line(), 100.0, &nodes);
        let bb = o.bounding_box();
        assert!((bb.height() - 100.0).abs() < 1e-6, "{bb:?}");
        let a = o.area().abs();
        assert!((a - 50_000.0).abs() < 2_000.0, "{a}");
    }

    /// Reta de dois pontos com afinar nas pontas: larga no meio.
    #[test]
    fn two_point_line_profile() {
        let o = outline(&line(), 100.0, &preset("taper-both"));
        assert!(o.contains(Point::new(500.0, 40.0)), "meio largo");
        assert!(!o.contains(Point::new(50.0, 40.0)), "ponta fina");
    }

    /// Fechado: duas bordas em sentidos opostos — o miolo fica vazio.
    #[test]
    fn closed_ring_hole() {
        let r = kurbo::Rect::new(0.0, 0.0, 1000.0, 1000.0).to_path(0.1);
        let o = outline(&r, 100.0, &preset("bulge"));
        assert!(!o.contains(Point::new(500.0, 500.0)), "miolo vazio");
        assert!(o.contains(Point::new(500.0, 0.0)) || o.contains(Point::new(500.0, 1.0)));
    }

    /// Ferramenta Largura: posição pelo comprimento e nó ajustado/inserido.
    #[test]
    fn fraction_and_set_node() {
        let (s, d) = fraction_at(&line(), Point::new(250.0, 30.0)).unwrap();
        assert!((s - 0.25).abs() < 1e-9 && (d - 30.0).abs() < 1e-9);
        let mut nodes = Vec::new();
        set_node(&mut nodes, 0.25, 2.0);
        assert_eq!(nodes.len(), 3);
        assert_eq!(nodes[1].at, 0.25);
        set_node(&mut nodes, 0.26, 0.5);
        assert_eq!(nodes.len(), 3);
        assert_eq!(nodes[1].left, 0.5);
    }
}
