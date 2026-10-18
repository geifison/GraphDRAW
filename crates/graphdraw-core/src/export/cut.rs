//! Saída para corte (doc 17): plotter de recorte (HPGL/PLT, GD-CRT-10) e
//! laser/CNC (DXF R12, GD-CRT-11).
//!
//! O percurso sai em polilinhas com tolerância (GD-CRT-13), na ordem de
//! corte (furos antes do contorno que os contém, vizinho mais próximo,
//! sentido escolhido — GD-CRT-04), com sobreposição, compensação de lâmina
//! e caixa de descasque opcionais (GD-CRT-05).

use std::fmt::Write as _;

use kurbo::{Affine, BezPath, PathEl, Point, Vec2};

use crate::edit::query::local_outline;
use crate::model::{Document, Object, ObjectKind};

/// Nome padrão da cor especial e da camada de corte (GD-CRT-01).
pub const CUT_NAME: &str = "CutContour";

/// Sentido de corte dos contornos fechados.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Direction {
    /// Como desenhado.
    #[default]
    Keep,
    Clockwise,
    CounterClockwise,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CutOptions {
    /// Nome da cor especial (ou da camada) que marca o corte.
    pub cut_name: String,
    /// `true`: só objetos marcados; `false`: todos os visíveis.
    pub only_marked: bool,
    /// Tolerância das curvas em segmentos, em µm (padrão 0,02 mm).
    pub tolerance: f64,
    /// Furos antes do contorno que os contém.
    pub inside_first: bool,
    pub direction: Direction,
    /// Sobreposição no fim dos contornos fechados, em µm.
    pub overcut: f64,
    /// Compensação de lâmina (distância do pivô à ponta), em µm.
    pub blade_offset: f64,
    /// Largura do corte do laser/fresa (kerf), em µm (GD-PRE-20).
    pub kerf: f64,
    /// Caixa de descasque com esta margem (µm) e linhas de divisão.
    pub weed_box: Option<f64>,
    /// Unidades do plotter por milímetro (HPGL).
    pub units_per_mm: f64,
    /// Velocidade (VS, cm/s) e força (FS) opcionais.
    pub velocity: Option<f64>,
    pub force: Option<f64>,
    /// Espelhar na horizontal (corte pelo verso).
    pub mirror: bool,
}

impl Default for CutOptions {
    fn default() -> Self {
        CutOptions {
            cut_name: CUT_NAME.into(),
            only_marked: true,
            tolerance: 20.0,
            inside_first: true,
            direction: Direction::Keep,
            overcut: 0.0,
            blade_offset: 0.0,
            kerf: 0.0,
            weed_box: None,
            units_per_mm: 40.0,
            velocity: None,
            force: None,
            mirror: false,
        }
    }
}

/// Um traço de corte: pontos na página (µm, Y para cima).
#[derive(Debug, Clone, PartialEq)]
pub struct CutPath {
    pub points: Vec<Point>,
    pub closed: bool,
}

fn is_marked(o: &Object, layer_is_cut: bool, name: &str) -> bool {
    layer_is_cut
        || o.stroke
            .as_ref()
            .is_some_and(|s| s.color.model == "spot" && s.color.name.as_deref() == Some(name))
}

fn collect(
    o: &Object,
    parent: Affine,
    layer_is_cut: bool,
    opts: &CutOptions,
    out: &mut Vec<BezPath>,
) {
    if !o.visible {
        return;
    }
    let m = parent * o.transform.affine();
    if let ObjectKind::Group(g) = &o.kind
        && g.boolean.is_some()
    {
        if let Some(r) = crate::edit::shapeops::live_object(g, m) {
            collect(&r, Affine::IDENTITY, layer_is_cut, opts, out);
        }
        return;
    }
    if let ObjectKind::Group(g) = &o.kind {
        let copies = crate::edit::symmetry::copies(g.symmetry.as_ref());
        for t in std::iter::once(Affine::IDENTITY).chain(copies) {
            for c in &g.children {
                collect(c, m * t, layer_is_cut, opts, out);
            }
        }
        return;
    }
    if opts.only_marked && !is_marked(o, layer_is_cut, &opts.cut_name) {
        return;
    }
    if let Some(mut p) = local_outline(o) {
        p.apply_affine(m);
        out.push(p);
    }
}

fn to_polylines(path: &BezPath, tol: f64) -> Vec<CutPath> {
    let mut out = Vec::new();
    let mut cur: Vec<Point> = Vec::new();
    let flush = |cur: &mut Vec<Point>, closed: bool, out: &mut Vec<CutPath>| {
        if cur.len() >= 2 {
            let mut pts = std::mem::take(cur);
            if closed && pts.first() == pts.last() {
                pts.pop();
            }
            out.push(CutPath {
                points: pts,
                closed,
            });
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

fn signed_area(p: &[Point]) -> f64 {
    let n = p.len();
    (0..n)
        .map(|i| {
            let (a, b) = (p[i], p[(i + 1) % n]);
            a.x * b.y - b.x * a.y
        })
        .sum::<f64>()
        / 2.0
}

fn contains(poly: &[Point], q: Point) -> bool {
    let mut inside = false;
    let n = poly.len();
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + n - 1) % n]);
        if (a.y > q.y) != (b.y > q.y) && q.x < (b.x - a.x) * (q.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
    }
    inside
}

/// `a` está dentro de `b` (fechado)?
fn inside(a: &CutPath, b: &CutPath) -> bool {
    b.closed && b.points.len() >= 3 && a.points.iter().all(|p| contains(&b.points, *p))
}

/// Ordem de corte (GD-CRT-04): um contorno só sai depois dos que estão
/// dentro dele; entre os prontos, o mais próximo da caneta. Fechados
/// começam no vértice mais próximo.
fn order(mut paths: Vec<CutPath>, opts: &CutOptions) -> Vec<CutPath> {
    let n = paths.len();
    let deps: Vec<Vec<usize>> = (0..n)
        .map(|i| {
            if !opts.inside_first {
                return Vec::new();
            }
            (0..n)
                .filter(|&j| j != i && inside(&paths[j], &paths[i]))
                .collect()
        })
        .collect();
    let mut done = vec![false; n];
    let mut pen = Point::ZERO;
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        let mut ready: Vec<usize> = (0..n)
            .filter(|&i| !done[i] && deps[i].iter().all(|&j| done[j]))
            .collect();
        // Com ciclo (contornos iguais), libera o restante.
        if ready.is_empty() {
            ready = (0..n).filter(|&i| !done[i]).collect();
        }
        let pick = ready
            .into_iter()
            .map(|i| {
                let d = paths[i]
                    .points
                    .iter()
                    .map(|p| (*p - pen).hypot2())
                    .fold(f64::INFINITY, f64::min);
                (i, d)
            })
            .min_by(|a, b| a.1.total_cmp(&b.1));
        let Some((i, _)) = pick else { break };
        done[i] = true;
        let p = &mut paths[i];
        if p.closed {
            if let Some(k) = (0..p.points.len()).min_by(|&a, &b| {
                (p.points[a] - pen)
                    .hypot2()
                    .total_cmp(&(p.points[b] - pen).hypot2())
            }) {
                p.points.rotate_left(k);
            }
        } else if (p.points[p.points.len() - 1] - pen).hypot2() < (p.points[0] - pen).hypot2() {
            p.points.reverse();
        }
        pen = if p.closed {
            p.points[0]
        } else {
            p.points[p.points.len() - 1]
        };
        out.push(p.clone());
    }
    out
}

fn set_direction(p: &mut CutPath, dir: Direction) {
    if !p.closed || dir == Direction::Keep {
        return;
    }
    // Página com Y para cima: área positiva = anti-horário.
    let ccw = signed_area(&p.points) > 0.0;
    if ccw != (dir == Direction::CounterClockwise) {
        p.points[1..].reverse();
    }
}

/// Fecha o contorno e segue `overcut` µm além do início.
fn with_overcut(p: &CutPath, overcut: f64) -> Vec<Point> {
    let mut pts = p.points.clone();
    if !p.closed {
        return pts;
    }
    pts.push(p.points[0]);
    let mut left = overcut;
    let n = p.points.len();
    let mut i = 0;
    while left > 0.0 && i < n {
        let (a, b) = (p.points[i], p.points[(i + 1) % n]);
        let len = (b - a).hypot();
        if len >= left {
            pts.push(a + (b - a) * (left / len));
            break;
        }
        pts.push(b);
        left -= len;
        i += 1;
    }
    pts
}

/// Compensação de lâmina arrastada: nos cantos, o pivô passa `d` além do
/// canto e gira num arco de raio `d` até a nova direção.
fn blade_compensate(pts: &[Point], d: f64) -> Vec<Point> {
    if d <= 0.0 || pts.len() < 3 {
        return pts.to_vec();
    }
    let mut out = vec![pts[0]];
    for i in 1..pts.len() - 1 {
        let (a, b, c) = (pts[i - 1], pts[i], pts[i + 1]);
        let (u, v) = (b - a, c - b);
        let (lu, lv) = (u.hypot(), v.hypot());
        if lu == 0.0 || lv == 0.0 {
            continue;
        }
        let (u, v) = (u / lu, v / lv);
        let turn = u.cross(v).atan2(u.dot(v));
        if turn.abs() < 10f64.to_radians() {
            out.push(b);
            continue;
        }
        out.push(b + u * d);
        let start = u.y.atan2(u.x);
        let steps = ((turn.abs() / 15f64.to_radians()).ceil() as usize).max(2);
        for k in 1..=steps {
            let ang = start + turn * k as f64 / steps as f64;
            out.push(b + Vec2::new(ang.cos(), ang.sin()) * d);
        }
    }
    out.push(pts[pts.len() - 1]);
    out
}

/// Caixa de descasque ao redor de tudo e linhas de divisão nos vãos entre
/// grupos de contornos (na horizontal).
fn weed(paths: &[CutPath], margin: f64) -> Vec<CutPath> {
    let boxes: Vec<kurbo::Rect> = paths
        .iter()
        .map(|p| {
            p.points.iter().fold(
                kurbo::Rect::from_points(p.points[0], p.points[0]),
                |r, q| r.union_pt(*q),
            )
        })
        .collect();
    let Some(all) = boxes.iter().copied().reduce(|a, b| a.union(b)) else {
        return Vec::new();
    };
    let r = all.inflate(margin, margin);
    let mut out = vec![CutPath {
        points: vec![
            Point::new(r.x0, r.y0),
            Point::new(r.x1, r.y0),
            Point::new(r.x1, r.y1),
            Point::new(r.x0, r.y1),
        ],
        closed: true,
    }];
    let mut spans: Vec<(f64, f64)> = boxes.iter().map(|b| (b.x0, b.x1)).collect();
    spans.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut reach = f64::NEG_INFINITY;
    for (x0, x1) in spans {
        if reach.is_finite() && x0 > reach {
            let x = (reach + x0) / 2.0;
            out.push(CutPath {
                points: vec![Point::new(x, r.y0), Point::new(x, r.y1)],
                closed: false,
            });
        }
        reach = reach.max(x1);
    }
    out
}

/// Kerf (GD-PRE-20): cada contorno fechado anda meio kerf — para fora nas
/// peças (aninhamento par), para dentro nos furos (ímpar). Abertos ficam.
fn kerf_compensate(paths: Vec<CutPath>, kerf: f64) -> Vec<CutPath> {
    use crate::edit::shapeops::{Join, offset_region, union_all};
    if kerf <= 0.0 {
        return paths;
    }
    let depth: Vec<usize> = (0..paths.len())
        .map(|i| {
            (0..paths.len())
                .filter(|&j| j != i && paths[i].closed && inside(&paths[i], &paths[j]))
                .count()
        })
        .collect();
    let mut out = Vec::new();
    for (p, d) in paths.into_iter().zip(depth) {
        if !p.closed || p.points.len() < 3 {
            out.push(p);
            continue;
        }
        let sign = if d % 2 == 0 { 1.0 } else { -1.0 };
        let contour: Vec<[f64; 2]> = p.points.iter().map(|q| [q.x, q.y]).collect();
        let shapes = union_all([vec![vec![contour]]]);
        for c in offset_region(&shapes, sign * kerf / 2.0, Join::Miter)
            .into_iter()
            .flatten()
        {
            if c.len() >= 3 {
                out.push(CutPath {
                    points: c.iter().map(|q| Point::new(q[0], q[1])).collect(),
                    closed: true,
                });
            }
        }
    }
    out
}

/// Percurso de corte da página, pronto para o plotter: cada item é uma
/// sequência de pontos com a caneta abaixada.
pub fn cut_strokes(doc: &Document, page: usize, opts: &CutOptions) -> Vec<Vec<Point>> {
    let Some(pg) = doc.data.pages.get(page) else {
        return Vec::new();
    };
    let mut beziers = Vec::new();
    for layer in &pg.layers {
        if !layer.visible {
            continue;
        }
        let cut_layer = layer.name == opts.cut_name;
        for o in &layer.objects {
            collect(o, Affine::IDENTITY, cut_layer, opts, &mut beziers);
        }
    }
    let tol = opts.tolerance.max(0.1);
    let mut paths: Vec<CutPath> = beziers.iter().flat_map(|b| to_polylines(b, tol)).collect();
    paths.retain(|p| p.points.len() >= 2);
    let mut paths = kerf_compensate(paths, opts.kerf);
    if let Some(m) = opts.weed_box {
        let extra = weed(&paths, m);
        paths.extend(extra);
    }
    let mut paths = order(paths, opts);
    for p in &mut paths {
        set_direction(p, opts.direction);
    }
    paths
        .iter()
        .map(|p| blade_compensate(&with_overcut(p, opts.overcut), opts.blade_offset))
        .collect()
}

/// HPGL/PLT (GD-CRT-10): `IN`, `VS`/`FS` opcionais, `PU`/`PD` em
/// coordenadas absolutas inteiras.
pub fn hpgl(strokes: &[Vec<Point>], page_width: f64, opts: &CutOptions) -> String {
    let k = opts.units_per_mm / 1000.0;
    let xy = |p: Point| {
        let x = if opts.mirror { page_width - p.x } else { p.x };
        ((x * k).round() as i64, (p.y * k).round() as i64)
    };
    let mut s = String::from("IN;");
    if let Some(v) = opts.velocity {
        let _ = write!(s, "VS{};", v.round() as i64);
    }
    if let Some(f) = opts.force {
        let _ = write!(s, "FS{};", f.round() as i64);
    }
    s.push_str("SP1;");
    for st in strokes {
        let Some(first) = st.first() else { continue };
        let (x, y) = xy(*first);
        let _ = write!(s, "PU{x},{y};");
        let pts: Vec<String> = st[1..]
            .iter()
            .map(|p| {
                let (x, y) = xy(*p);
                format!("{x},{y}")
            })
            .collect();
        if !pts.is_empty() {
            let _ = write!(s, "PD{};", pts.join(","));
        }
    }
    s.push_str("PU0,0;SP0;IN;\n");
    s
}

/// DXF R12 (GD-CRT-11): só linhas e polilinhas, em milímetros.
pub fn dxf(strokes: &[Vec<Point>], opts: &CutOptions, page_width: f64) -> String {
    let mm = |v: f64| format!("{:.4}", v / 1000.0);
    let mut s = String::new();
    let mut put = |code: &str, v: &str| {
        let _ = writeln!(s, "{code}\n{v}");
    };
    put("0", "SECTION");
    put("2", "HEADER");
    put("9", "$ACADVER");
    put("1", "AC1009");
    put("9", "$INSUNITS");
    put("70", "4");
    put("0", "ENDSEC");
    put("0", "SECTION");
    put("2", "ENTITIES");
    for st in strokes {
        let pts: Vec<Point> = st
            .iter()
            .map(|p| {
                if opts.mirror {
                    Point::new(page_width - p.x, p.y)
                } else {
                    *p
                }
            })
            .collect();
        if pts.len() == 2 {
            put("0", "LINE");
            put("8", &opts.cut_name);
            put("10", &mm(pts[0].x));
            put("20", &mm(pts[0].y));
            put("11", &mm(pts[1].x));
            put("21", &mm(pts[1].y));
            continue;
        }
        put("0", "POLYLINE");
        put("8", &opts.cut_name);
        put("66", "1");
        put("70", "0");
        for p in &pts {
            put("0", "VERTEX");
            put("8", &opts.cut_name);
            put("10", &mm(p.x));
            put("20", &mm(p.y));
        }
        put("0", "SEQEND");
    }
    put("0", "ENDSEC");
    put("0", "EOF");
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Color, Layer, Page, RectData, Stroke, Transform};
    use serde_json::Map;

    fn spot() -> Color {
        let mut c = Color::new("spot", vec![1.0]);
        c.name = Some(CUT_NAME.into());
        c
    }

    fn rect(x: f64, y: f64, w: f64, h: f64, marked: bool) -> Object {
        let mut o = Object::new(ObjectKind::Rectangle(RectData {
            width: w,
            height: h,
            corners: None,
            extra: Map::new(),
        }));
        o.transform = Transform::translate(x, y);
        o.stroke = Some(Stroke::new(
            0.0,
            if marked { spot() } else { Color::black() },
        ));
        o
    }

    fn doc_with(objs: Vec<Object>) -> Document {
        let mut d = Document::new();
        let mut p = Page::new(300_000.0, 200_000.0);
        p.layers[0].objects = objs;
        d.data.pages.push(p);
        d
    }

    /// Aceite do doc 17: retângulo de 100 × 50 mm a 40 u/mm → 4000 × 2000.
    #[test]
    fn plt_rectangle_100x50_is_4000x2000() {
        let d = doc_with(vec![rect(0.0, 0.0, 100_000.0, 50_000.0, true)]);
        let opts = CutOptions::default();
        let st = cut_strokes(&d, 0, &opts);
        let plt = hpgl(&st, 300_000.0, &opts);
        assert!(plt.starts_with("IN;SP1;PU0,0;PD"), "{plt}");
        assert!(plt.contains("4000,0") && plt.contains("4000,2000") && plt.contains("0,2000"));
        assert!(plt.trim_end().ends_with("PU0,0;SP0;IN;"));
    }

    /// Só o marcado sai; furo (dentro) é cortado antes do contorno externo.
    #[test]
    fn only_marked_and_holes_first() {
        let outer = rect(0.0, 0.0, 50_000.0, 50_000.0, true);
        let hole = rect(20_000.0, 20_000.0, 10_000.0, 10_000.0, true);
        let print_only = rect(100_000.0, 0.0, 10_000.0, 10_000.0, false);
        let d = doc_with(vec![outer, hole, print_only]);
        let st = cut_strokes(&d, 0, &CutOptions::default());
        assert_eq!(st.len(), 2);
        let first_box = st[0]
            .iter()
            .fold(kurbo::Rect::from_points(st[0][0], st[0][0]), |r, p| {
                r.union_pt(*p)
            });
        assert!(first_box.width() < 11_000.0, "o furo vem primeiro");
        // Camada chamada CutContour marca tudo dela.
        let mut d2 = doc_with(vec![]);
        let mut l = Layer::new(CUT_NAME);
        l.objects.push(rect(0.0, 0.0, 10.0, 10.0, false));
        d2.data.pages[0].layers.push(l);
        assert_eq!(cut_strokes(&d2, 0, &CutOptions::default()).len(), 1);
    }

    #[test]
    fn overcut_direction_blade_and_weed() {
        let d = doc_with(vec![rect(0.0, 0.0, 10_000.0, 10_000.0, true)]);
        let opts = CutOptions {
            overcut: 1_000.0,
            direction: Direction::Clockwise,
            ..Default::default()
        };
        let st = cut_strokes(&d, 0, &opts);
        let s = &st[0];
        // 4 cantos + volta ao início + 1 mm além.
        assert_eq!(s.len(), 6);
        assert_eq!(s[4], s[0]);
        assert!(((s[5] - s[4]).hypot() - 1_000.0).abs() < 1e-6);
        assert!(signed_area(&s[..4]) < 0.0, "horário");
        // Lâmina: arcos nos cantos acrescentam pontos.
        let b = cut_strokes(
            &d,
            0,
            &CutOptions {
                blade_offset: 250.0,
                ..Default::default()
            },
        );
        assert!(b[0].len() > 8);
        // Descasque: caixa + linha de divisão entre dois grupos.
        let d2 = doc_with(vec![
            rect(0.0, 0.0, 10_000.0, 10_000.0, true),
            rect(30_000.0, 0.0, 10_000.0, 10_000.0, true),
        ]);
        let w = cut_strokes(
            &d2,
            0,
            &CutOptions {
                weed_box: Some(5_000.0),
                ..Default::default()
            },
        );
        assert_eq!(w.len(), 4);
        assert!(
            w.iter()
                .any(|s| s.len() == 2 && (s[0].x - 20_000.0).abs() < 1e-6)
        );
    }

    /// GD-PRE-20: quadrado de 100 mm com furo de 20 mm, kerf 0,2 mm → peça
    /// de 100,2 mm e furo de 19,8 mm; o desenho não muda.
    #[test]
    fn gd_pre_20_kerf_grows_parts_and_shrinks_holes() {
        let d = doc_with(vec![
            rect(0.0, 0.0, 100_000.0, 100_000.0, true),
            rect(40_000.0, 40_000.0, 20_000.0, 20_000.0, true),
        ]);
        let st = cut_strokes(
            &d,
            0,
            &CutOptions {
                kerf: 200.0,
                ..Default::default()
            },
        );
        assert_eq!(st.len(), 2);
        let size = |s: &Vec<Point>| {
            let r = s
                .iter()
                .fold(kurbo::Rect::from_points(s[0], s[0]), |r, p| r.union_pt(*p));
            (r.width(), r.height())
        };
        let mut sizes: Vec<(f64, f64)> = st.iter().map(size).collect();
        sizes.sort_by(|a, b| a.0.total_cmp(&b.0));
        assert!(
            (sizes[0].0 - 19_800.0).abs() < 2.0 && (sizes[0].1 - 19_800.0).abs() < 2.0,
            "{sizes:?}"
        );
        assert!(
            (sizes[1].0 - 100_200.0).abs() < 2.0 && (sizes[1].1 - 100_200.0).abs() < 2.0,
            "{sizes:?}"
        );
        // O furo continua saindo antes da peça.
        assert!(size(&st[0]).0 < 50_000.0);
    }

    #[test]
    fn dxf_r12_has_polylines_in_mm() {
        let d = doc_with(vec![rect(0.0, 0.0, 100_000.0, 50_000.0, true)]);
        let opts = CutOptions::default();
        let t = dxf(&cut_strokes(&d, 0, &opts), &opts, 300_000.0);
        assert!(t.contains("AC1009") && t.contains("POLYLINE") && t.contains("SEQEND"));
        assert!(t.contains("\n100.0000\n") && t.contains("\n50.0000\n"));
        assert!(t.trim_end().ends_with("EOF"));
    }
}
