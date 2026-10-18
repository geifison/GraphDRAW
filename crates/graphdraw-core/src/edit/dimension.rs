//! Cotas (GD-COT-01..03, doc 08): medida, rótulo, geometria e ligação aos
//! nós medidos.
//!
//! A cota não tem desenho próprio no renderizador: [`expand`] a transforma
//! em objetos comuns (linhas de extensão, linhas de cota com pontas de seta
//! e um texto artístico), que a cena, o PDF, o SVG e o PNG já sabem
//! desenhar. Os pontos ligados (GD-COT-02) seguem os nós: a cena resolve as
//! ligações ao vivo e o [`Editor`] grava os pontos novos no mesmo passo de
//! desfazer da alteração ([`refresh`]).

use kurbo::{Affine, BezPath, Point, Rect, Shape, Vec2};

use super::convert::bez_to_subpaths;
use super::history::{Editor, ObjLoc};
use super::query::local_outline;
use crate::geometry::{self, SegRef};
use crate::model::{
    Arrow, Color, DimFormat, DimLink, DimensionData, Document, Fill, FontSpec, Object, ObjectKind,
    Paragraph, PathData, Stroke, Subpath, TextData, TextSpan, Transform,
};

/// Linha de extensão: folga a partir do ponto medido e quanto passa da
/// linha de cota (µm).
pub const GAP: f64 = 1_000.0;
pub const OVERSHOOT: f64 = 1_500.0;
/// Distância do texto à linha de cota (µm).
pub const TEXT_GAP: f64 = 800.0;
/// Raio padrão do arco angular (µm).
pub const ARC_RADIUS: f64 = 10_000.0;
/// Espessura padrão das linhas (0,25 mm).
pub const LINE_WIDTH: f64 = 250.0;

fn pt(a: [f64; 2]) -> Point {
    Point::new(a[0], a[1])
}

/// Valor medido: comprimento no mundo (µm, já com a escala de desenho
/// `[papel, mundo]`, GD-DOC-03) ou graus (angular).
pub fn measure(d: &DimensionData, pts: &[Point], seg_len: Option<f64>, scale: [f64; 2]) -> f64 {
    let k = if scale[0] > 0.0 {
        scale[1] / scale[0]
    } else {
        1.0
    };
    let g = |i: usize| pts.get(i).copied().unwrap_or(Point::ORIGIN);
    match d.dim_kind.as_str() {
        "horizontal" => (g(1).x - g(0).x).abs() * k,
        "vertical" => (g(1).y - g(0).y).abs() * k,
        "segment" => seg_len.unwrap_or_else(|| (g(1) - g(0)).hypot()) * k,
        "angular" => {
            let (u, v) = (g(1) - g(0), g(2) - g(0));
            u.cross(v).atan2(u.dot(v)).abs().to_degrees()
        }
        "radius" => (g(1) - g(0)).hypot() * k,
        "diameter" => 2.0 * (g(1) - g(0)).hypot() * k,
        _ => (g(1) - g(0)).hypot() * k,
    }
}

/// Texto da cota (GD-COT-03): prefixo, número com vírgula decimal, unidade
/// e sufixo. Raio leva "R" e diâmetro "Ø" na frente.
pub fn label(d: &DimensionData, value: f64) -> String {
    let f = &d.format;
    let prec = f.precision.min(6) as usize;
    let (num, unit) = if d.dim_kind == "angular" {
        (value, "°")
    } else {
        let (div, u) = match f.unit.as_str() {
            "cm" => (10_000.0, "cm"),
            "m" => (1_000_000.0, "m"),
            "in" => (25_400.0, "in"),
            "pt" => (25_400.0 / 72.0, "pt"),
            _ => (1_000.0, "mm"),
        };
        (value / div, u)
    };
    let n = format!("{num:.prec$}").replace('.', ",");
    let sym = match d.dim_kind.as_str() {
        "radius" => "R",
        "diameter" => "Ø",
        _ => "",
    };
    let unit = if !f.show_unit {
        String::new()
    } else if unit == "°" {
        unit.into()
    } else {
        format!(" {unit}")
    };
    format!("{}{sym}{n}{unit}{}", f.prefix, f.suffix)
}

/// Desenho da cota no espaço local: linhas de extensão, linhas de cota
/// (cada uma com pontas no início/fim) e onde/como vai o texto.
#[derive(Debug, Clone)]
pub struct Geometry {
    pub extension: BezPath,
    pub lines: Vec<(BezPath, bool, bool)>,
    pub text_at: Point,
    /// Ângulo do texto (radianos), já legível (entre −90° e 90°).
    pub text_angle: f64,
}

fn readable(a: f64) -> f64 {
    let mut a = a;
    while a > std::f64::consts::FRAC_PI_2 + 1e-9 {
        a -= std::f64::consts::PI;
    }
    while a <= -std::f64::consts::FRAC_PI_2 + 1e-9 {
        a += std::f64::consts::PI;
    }
    a
}

fn seg(a: Point, b: Point) -> BezPath {
    let mut p = BezPath::new();
    p.move_to(a);
    p.line_to(b);
    p
}

pub fn geometry(d: &DimensionData, pts: &[Point]) -> Option<Geometry> {
    let ext_on = d.format.extension_lines;
    match d.dim_kind.as_str() {
        "angular" => {
            let (v, a, b) = (*pts.first()?, *pts.get(1)?, *pts.get(2)?);
            let r = if d.offset > 0.0 { d.offset } else { ARC_RADIUS };
            let (ua, ub) = (a - v, b - v);
            if ua.hypot() <= 0.0 || ub.hypot() <= 0.0 {
                return None;
            }
            let ta = ua.y.atan2(ua.x);
            let sweep = ua.cross(ub).atan2(ua.dot(ub));
            let arc = kurbo::Arc::new(v, (r, r), ta, sweep, 0.0);
            let mut extension = BezPath::new();
            if ext_on {
                for u in [ua, ub] {
                    if u.hypot() < r {
                        let n = u.normalize();
                        extension.extend(
                            seg(v + n * (u.hypot() + GAP), v + n * (r + OVERSHOOT))
                                .elements()
                                .iter()
                                .copied(),
                        );
                    }
                }
            }
            let mid = ta + sweep / 2.0;
            Some(Geometry {
                extension,
                lines: vec![(arc.to_path(0.1), true, true)],
                text_at: v + Vec2::new(mid.cos(), mid.sin())
                    * (r + TEXT_GAP + d.format.text_size * 0.6),
                text_angle: 0.0,
            })
        }
        "radius" | "diameter" => {
            let (c, q) = (*pts.first()?, *pts.get(1)?);
            let u = q - c;
            if u.hypot() <= 0.0 {
                return None;
            }
            let n = u.normalize();
            let lead = d.offset.max(0.0);
            let mut extension = BezPath::new();
            let lines = if d.dim_kind == "radius" {
                if lead > 0.0 {
                    extension = seg(q, q + n * lead);
                }
                vec![(seg(c, q), false, true)]
            } else {
                vec![(seg(c - u, q), true, true)]
            };
            let at = if d.dim_kind == "radius" {
                c + (q - c) * 0.5 + n * lead
            } else {
                c
            };
            Some(Geometry {
                extension,
                lines,
                text_at: at,
                text_angle: readable(u.y.atan2(u.x)),
            })
        }
        _ => {
            let (a, b) = (*pts.first()?, *pts.get(1)?);
            let dir = match d.dim_kind.as_str() {
                "horizontal" => Vec2::new(1.0, 0.0),
                "vertical" => Vec2::new(0.0, 1.0),
                _ => {
                    let v = b - a;
                    if v.hypot() > 0.0 {
                        v.normalize()
                    } else {
                        Vec2::new(1.0, 0.0)
                    }
                }
            };
            let n = Vec2::new(-dir.y, dir.x);
            let l0 = a + n * d.offset;
            let p1 = l0 + dir * (a - l0).dot(dir);
            let p2 = l0 + dir * (b - l0).dot(dir);
            let mut extension = BezPath::new();
            if ext_on {
                for (m, p) in [(a, p1), (b, p2)] {
                    let e = p - m;
                    if e.hypot() > GAP {
                        let u = e.normalize();
                        extension.extend(
                            seg(m + u * GAP, p + u * OVERSHOOT)
                                .elements()
                                .iter()
                                .copied(),
                        );
                    }
                }
            }
            Some(Geometry {
                extension,
                lines: vec![(seg(p1, p2), true, true)],
                text_at: p1.midpoint(p2),
                text_angle: readable(dir.y.atan2(dir.x)),
            })
        }
    }
}

fn line_stroke(o: &Object) -> Stroke {
    let mut s = o
        .stroke
        .clone()
        .unwrap_or_else(|| Stroke::new(LINE_WIDTH, Color::black()));
    s.start_arrow = None;
    s.end_arrow = None;
    s
}

fn arrow(f: &DimFormat) -> Option<Arrow> {
    match f.arrow.as_str() {
        "none" => None,
        shape => {
            let mut a = Arrow::new(shape);
            a.scale = 2.5;
            Some(a)
        }
    }
}

/// Texto artístico do rótulo, centrado em `at`, girado `angle`, acima da
/// linha (ou sobre ela).
fn text_object(o: &Object, d: &DimensionData, label: &str, at: Point, angle: f64) -> Object {
    let f = &d.format;
    let mut t = TextData {
        text_kind: "artistic".into(),
        frame: None,
        paragraphs: vec![Paragraph {
            align: "left".into(),
            spans: vec![TextSpan {
                text: label.into(),
                font: FontSpec {
                    family: f.font.clone(),
                    size: f.text_size,
                    weight: 400,
                    italic: false,
                    extra: Default::default(),
                },
                fill: None,
                extra: Default::default(),
            }],
            hyphenate: false,
            lang: None,
            extra: Default::default(),
        }],
        line_spacing: 1.2,
        path: None,
        exclusions: Default::default(),
        path_memory: None,
        tracking: 0.0,
        extra: Default::default(),
    };
    let layout = crate::render::text::layout_text(&t);
    let width = layout
        .lines
        .iter()
        .flat_map(|l| l.carets.iter().map(|c| c.1))
        .fold(0.0, f64::max);
    let width = if width > 0.0 {
        width
    } else {
        f.text_size * 0.6 * label.chars().count() as f64
    };
    // Pela linha de base: acima da linha de cota com folga para a
    // descendente, ou centrada nela (`inline`).
    let h = f.text_size;
    let base = layout.lines.first().map_or(-0.8 * h, |l| l.baseline);
    let y = if f.text_position == "inline" {
        -0.35 * h - base
    } else {
        TEXT_GAP + 0.25 * h - base
    };
    t.extra = Default::default();
    let mut text = Object::new(ObjectKind::Text(t));
    text.id = o.id.clone();
    let color = o
        .stroke
        .as_ref()
        .map_or_else(Color::black, |s| s.color.clone());
    text.fill = Some(Fill::solid(color));
    text.transform = Transform::from_affine(
        Affine::translate(at.to_vec2())
            * Affine::rotate(angle)
            * Affine::translate((-width / 2.0, y)),
    );
    text
}

/// Objetos comuns que desenham a cota (no espaço local dela, com o ID dela).
pub fn expand(o: &Object, d: &DimensionData, pts: &[Point], label: &str) -> Vec<Object> {
    let Some(g) = geometry(d, pts) else {
        return Vec::new();
    };
    let path_obj = |path: &BezPath, stroke: Stroke| {
        let mut p = Object::new(ObjectKind::Path(PathData {
            subpaths: bez_to_subpaths(path),
            fill_rule: Default::default(),
            extra: Default::default(),
        }));
        p.id = o.id.clone();
        p.stroke = Some(stroke);
        p.opacity = o.opacity;
        p
    };
    let mut out = Vec::new();
    if !g.extension.elements().is_empty() {
        out.push(path_obj(&g.extension, line_stroke(o)));
    }
    for (path, start, end) in &g.lines {
        let mut s = line_stroke(o);
        s.start_arrow = start.then(|| arrow(&d.format)).flatten();
        s.end_arrow = end.then(|| arrow(&d.format)).flatten();
        out.push(path_obj(path, s));
    }
    out.push(text_object(o, d, label, g.text_at, g.text_angle));
    out
}

/// Contorno para seleção e caixa: as linhas e a caixa aproximada do texto
/// (pontos guardados, escala 1:1).
pub fn outline(d: &DimensionData) -> BezPath {
    let pts: Vec<Point> = d.points.iter().copied().map(pt).collect();
    let mut out = BezPath::new();
    let Some(g) = geometry(d, &pts) else {
        return out;
    };
    out.extend(g.extension.elements().iter().copied());
    for (p, _, _) in &g.lines {
        out.extend(p.elements().iter().copied());
    }
    let text = label(d, measure(d, &pts, None, [1.0, 1.0]));
    let w = d.format.text_size * 0.6 * text.chars().count() as f64;
    let h = d.format.text_size;
    let y = if d.format.text_position == "inline" {
        -h / 2.0
    } else {
        TEXT_GAP
    };
    let mut r = Rect::new(-w / 2.0, y, w / 2.0, y + h).to_path(0.1);
    r.apply_affine(Affine::translate(g.text_at.to_vec2()) * Affine::rotate(g.text_angle));
    out.extend(r.elements().iter().copied());
    out
}

fn path_of(o: &Object) -> Option<PathData> {
    match &o.kind {
        ObjectKind::Path(p) => Some(p.clone()),
        ObjectKind::Group(_)
        | ObjectKind::Text(_)
        | ObjectKind::Image(_)
        | ObjectKind::Dimension(_) => None,
        _ => Some(PathData {
            subpaths: bez_to_subpaths(&local_outline(o)?),
            fill_rule: Default::default(),
            extra: Default::default(),
        }),
    }
}

/// Posição (página) do nó ligado.
fn node_page(o: &Object, parent: Affine, l: &DimLink) -> Option<Point> {
    let pd = path_of(o)?;
    let n = pd.subpaths.get(l.subpath)?.nodes.get(l.node)?;
    Some(parent * o.transform.affine() * pt(n.p))
}

/// Comprimento (página) do segmento ligado.
fn segment_page(o: &Object, parent: Affine, l: &DimLink) -> Option<f64> {
    let pd = path_of(o)?;
    let s = geometry::segment(
        &pd,
        SegRef {
            subpath: l.subpath,
            index: l.node,
        },
    )?;
    Some(geometry::segment_length(
        &((parent * o.transform.affine()) * s),
    ))
}

/// Pontos atuais (espaço local da cota `m` → página) seguindo as ligações;
/// `find` devolve o objeto ligado e a matriz do pai dele. Também o
/// comprimento do segmento ligado.
pub fn resolve(
    d: &DimensionData,
    m: Affine,
    find: &dyn Fn(&str) -> Option<(Object, Affine)>,
) -> (Vec<Point>, Option<f64>) {
    let inv = m.inverse();
    let pts = d
        .points
        .iter()
        .enumerate()
        .map(|(i, p)| {
            d.links
                .get(i)
                .and_then(|l| l.as_ref())
                .and_then(|l| {
                    let (o, parent) = find(&l.object)?;
                    node_page(&o, parent, l)
                })
                .map_or(pt(*p), |q| inv * q)
        })
        .collect();
    let seg_len = d.segment.as_ref().and_then(|l| {
        let (o, parent) = find(&l.object)?;
        let scale = m.determinant().abs().sqrt().max(1e-12);
        segment_page(&o, parent, l).map(|v| v / scale)
    });
    (pts, seg_len)
}

/// Procura o objeto (e a matriz do pai) no documento inteiro.
pub fn find_in_doc(doc: &Document, id: &str) -> Option<(Object, Affine)> {
    fn walk(o: &Object, parent: Affine, id: &str) -> Option<(Object, Affine)> {
        if o.id == id {
            return Some((o.clone(), parent));
        }
        if let ObjectKind::Group(g) = &o.kind {
            let m = parent * o.transform.affine();
            return g.children.iter().find_map(|c| walk(c, m, id));
        }
        None
    }
    doc.data
        .pages
        .iter()
        .flat_map(|p| &p.layers)
        .chain(&doc.data.master_page.layers)
        .flat_map(|l| &l.objects)
        .find_map(|o| walk(o, Affine::IDENTITY, id))
}

/// Grava os pontos novos das cotas ligadas (GD-COT-02). Chamado pelo
/// [`Editor`] ao fechar a transação, então entra no mesmo passo.
pub fn refresh(ed: &mut Editor) {
    let mut updates: Vec<(ObjLoc, Object)> = Vec::new();
    for (pi, page) in ed.doc.data.pages.iter().enumerate() {
        for (li, layer) in page.layers.iter().enumerate() {
            for (oi, o) in layer.objects.iter().enumerate() {
                let ObjectKind::Dimension(d) = &o.kind else {
                    continue;
                };
                if d.links.iter().all(Option::is_none) {
                    continue;
                }
                let doc = &ed.doc;
                let (pts, _) = resolve(d, o.transform.affine(), &|id| find_in_doc(doc, id));
                let new: Vec<[f64; 2]> = pts.iter().map(|p| [p.x, p.y]).collect();
                let moved = new
                    .iter()
                    .zip(&d.points)
                    .any(|(a, b)| (a[0] - b[0]).abs() > 1e-6 || (a[1] - b[1]).abs() > 1e-6);
                if moved {
                    let mut n = o.clone();
                    if let ObjectKind::Dimension(nd) = &mut n.kind {
                        nd.points = new;
                    }
                    updates.push((
                        ObjLoc {
                            page: pi,
                            layer: li,
                            index: oi,
                        },
                        n,
                    ));
                }
            }
        }
    }
    for (loc, n) in updates {
        ed.replace_object(loc, n);
    }
}

/// Cota nova (página) com o formato padrão e a espessura de linha padrão.
pub fn new_dimension(
    kind: &str,
    points: &[Point],
    offset: f64,
    links: Vec<Option<DimLink>>,
) -> Object {
    let mut o = Object::new(ObjectKind::Dimension(DimensionData {
        dim_kind: kind.into(),
        points: points.iter().map(|p| [p.x, p.y]).collect(),
        offset,
        links,
        segment: None,
        format: DimFormat::default(),
        extra: Default::default(),
    }));
    o.stroke = Some(Stroke::new(LINE_WIDTH, Color::black()));
    o
}

/// Nó (de um caminho ou do contorno de uma forma) perto de `p` (página),
/// para ligar a cota (GD-COT-02).
pub fn node_near(page: &crate::model::Page, p: Point, tol: f64) -> Option<(DimLink, Point)> {
    let mut best: Option<(f64, DimLink, Point)> = None;
    for layer in page.layers.iter().filter(|l| l.visible) {
        for o in &layer.objects {
            if matches!(o.kind, ObjectKind::Dimension(_)) {
                continue;
            }
            let Some(pd) = path_of(o) else {
                continue;
            };
            let m = o.transform.affine();
            for (si, sp) in pd.subpaths.iter().enumerate() {
                for (ni, n) in sp.nodes.iter().enumerate() {
                    let q = m * pt(n.p);
                    let dist = (q - p).hypot();
                    if dist <= tol && best.as_ref().is_none_or(|b| dist < b.0) {
                        best = Some((
                            dist,
                            DimLink {
                                object: o.id.clone(),
                                subpath: si,
                                node: ni,
                            },
                            q,
                        ));
                    }
                }
            }
        }
    }
    best.map(|(_, l, q)| (l, q))
}

/// Segmento (de caminho ou contorno) sob `p` (página): ligação e pontas.
pub fn segment_near(
    page: &crate::model::Page,
    p: Point,
    tol: f64,
) -> Option<(DimLink, Point, Point)> {
    for layer in page.layers.iter().rev().filter(|l| l.visible) {
        for o in layer.objects.iter().rev() {
            if matches!(o.kind, ObjectKind::Dimension(_)) {
                continue;
            }
            let Some(pd) = path_of(o) else {
                continue;
            };
            let m = o.transform.affine();
            let scale = m.determinant().abs().sqrt().max(1e-12);
            let Some(near) =
                geometry::nearest(&pd, m.inverse() * p).filter(|n| n.distance * scale <= tol)
            else {
                continue;
            };
            let sp: &Subpath = &pd.subpaths[near.seg.subpath];
            let a = sp.nodes[near.seg.index].p;
            let b = sp.nodes[(near.seg.index + 1) % sp.nodes.len()].p;
            return Some((
                DimLink {
                    object: o.id.clone(),
                    subpath: near.seg.subpath,
                    node: near.seg.index,
                },
                m * pt(a),
                m * pt(b),
            ));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Node, Page, RectData};
    use serde_json::Map;

    fn dim(kind: &str, pts: &[[f64; 2]]) -> DimensionData {
        DimensionData {
            dim_kind: kind.into(),
            points: pts.to_vec(),
            offset: 5_000.0,
            links: Vec::new(),
            segment: None,
            format: DimFormat::default(),
            extra: Map::new(),
        }
    }

    fn pts(d: &DimensionData) -> Vec<Point> {
        d.points.iter().copied().map(pt).collect()
    }

    /// GD-COT-01/03, GD-DOC-03: medidas, rótulos e escala de desenho.
    #[test]
    fn gd_cot_01_03_measure_and_label() {
        let h = dim("horizontal", &[[0.0, 0.0], [100_000.0, 30_000.0]]);
        assert_eq!(
            label(&h, measure(&h, &pts(&h), None, [1.0, 1.0])),
            "100,0 mm"
        );
        let v = dim("vertical", &[[0.0, 0.0], [100_000.0, 30_000.0]]);
        assert_eq!(
            label(&v, measure(&v, &pts(&v), None, [1.0, 1.0])),
            "30,0 mm"
        );
        let mut a = dim("aligned", &[[0.0, 0.0], [30_000.0, 40_000.0]]);
        a.format.unit = "m".into();
        a.format.precision = 2;
        // 1:100 — 50 mm no papel são 5 m no mundo.
        assert_eq!(
            label(&a, measure(&a, &pts(&a), None, [1.0, 100.0])),
            "5,00 m"
        );
        let ang = dim("angular", &[[0.0, 0.0], [10.0, 0.0], [0.0, 10.0]]);
        assert_eq!(
            label(&ang, measure(&ang, &pts(&ang), None, [1.0, 1.0])),
            "90,0°"
        );
        let mut r = dim("radius", &[[0.0, 0.0], [12_000.0, 0.0]]);
        r.format.prefix = "".into();
        assert_eq!(
            label(&r, measure(&r, &pts(&r), None, [1.0, 1.0])),
            "R12,0 mm"
        );
        let mut dd = dim("diameter", &[[0.0, 0.0], [12_000.0, 0.0]]);
        dd.format.show_unit = false;
        dd.format.suffix = " máx.".into();
        assert_eq!(
            label(&dd, measure(&dd, &pts(&dd), None, [1.0, 1.0])),
            "Ø24,0 máx."
        );
    }

    /// Geometria: a linha de cota horizontal fica a `offset` acima e tem as
    /// duas pontas; o texto fica no meio.
    #[test]
    fn linear_geometry() {
        let h = dim("horizontal", &[[0.0, 0.0], [100_000.0, 0.0]]);
        let g = geometry(&h, &pts(&h)).unwrap();
        let line = g.lines[0].0.bounding_box();
        assert!((line.y0 - 5_000.0).abs() < 1e-6 && (line.width() - 100_000.0).abs() < 1e-6);
        assert!((g.text_at - Point::new(50_000.0, 5_000.0)).hypot() < 1e-6);
        let o = new_dimension(
            "horizontal",
            &[Point::ORIGIN, Point::new(100_000.0, 0.0)],
            5_000.0,
            vec![],
        );
        let ObjectKind::Dimension(d) = &o.kind else {
            panic!()
        };
        let objs = expand(&o, d, &pts(d), "100,0 mm");
        assert_eq!(objs.len(), 3, "extensões, linha, texto");
        assert!(objs.iter().all(|x| x.id == o.id));
        assert!(objs[1].stroke.as_ref().unwrap().start_arrow.is_some());
        // Ida e volta no JSON.
        let back = Object::from_value(o.to_value());
        assert_eq!(back, o);
    }

    /// GD-COT-02: mover o objeto medido arrasta a cota no mesmo passo de
    /// desfazer; desfazer volta os dois.
    #[test]
    fn gd_cot_02_linked_dimension_follows() {
        let mut d = Document::new();
        d.data.pages.push(Page::new(300_000.0, 300_000.0));
        let mut ed = Editor::new(d);
        let mut path = Object::new(ObjectKind::Path(PathData {
            subpaths: vec![Subpath {
                nodes: vec![Node::corner(0.0, 0.0), Node::corner(100_000.0, 0.0)],
                closed: false,
                extra: Map::new(),
            }],
            fill_rule: Default::default(),
            extra: Map::new(),
        }));
        path.stroke = Some(Stroke::new(500.0, Color::black()));
        let pid = super::super::ops::add_object(&mut ed, 0, 0, path, "p");
        let (l0, p0) = node_near(&ed.doc.data.pages[0], Point::new(10.0, 10.0), 100.0).unwrap();
        let (l1, p1) = node_near(&ed.doc.data.pages[0], Point::new(100_000.0, 0.0), 100.0).unwrap();
        let dimo = new_dimension(
            "horizontal",
            &[p0, p1],
            5_000.0,
            vec![Some(l0), Some(l1.clone())],
        );
        let did = super::super::ops::add_object(&mut ed, 0, 0, dimo, "c");
        let undo = ed.undo_len();
        // Estica a linha: o nó 1 vai para x = 150 mm.
        let r = ed.find(&pid).unwrap();
        let mut o = ed.get(&r).unwrap().clone();
        if let ObjectKind::Path(pd) = &mut o.kind {
            pd.subpaths[0].nodes[1].p = [150_000.0, 0.0];
        }
        ed.transaction("Editar nó", |ed| ed.replace_at(&r, o));
        assert_eq!(ed.undo_len(), undo + 1, "um passo só");
        let get_pts = |ed: &Editor| match &ed.get(&ed.find(&did).unwrap()).unwrap().kind {
            ObjectKind::Dimension(d) => d.points.clone(),
            _ => panic!(),
        };
        assert_eq!(get_pts(&ed)[1], [150_000.0, 0.0]);
        ed.undo();
        assert_eq!(get_pts(&ed)[1], [100_000.0, 0.0]);
        // Segmento: comprimento da curva ligada.
        let (sl, a, b) =
            segment_near(&ed.doc.data.pages[0], Point::new(50_000.0, 0.0), 100.0).unwrap();
        let mut s = new_dimension("segment", &[a, b], 5_000.0, vec![]);
        if let ObjectKind::Dimension(d) = &mut s.kind {
            d.segment = Some(sl);
            let doc = &ed.doc;
            let (p, len) = resolve(d, Affine::IDENTITY, &|id| find_in_doc(doc, id));
            assert_eq!(label(d, measure(d, &p, len, [1.0, 1.0])), "100,0 mm");
        }
        let _ = RectData {
            width: 0.0,
            height: 0.0,
            corners: None,
            extra: Map::new(),
        };
    }
}
