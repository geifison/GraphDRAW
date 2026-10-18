//! Efeitos dinâmicos (doc 10): um grupo com `effect` guarda os objetos de
//! origem e os parâmetros; o resultado é gerado a cada desenho, então
//! editar a origem ou um parâmetro refaz o efeito. Desagrupar (Separar,
//! GD-CMB-04) congela: o resultado vira objetos comuns.
//!
//! - **Contorno**: anéis paralelos para fora ou para dentro da forma, com a
//!   cor indo da origem até a cor final.
//! - **Mistura**: passos intermediários entre o primeiro e o segundo filho
//!   (forma, cor, opacidade e espessura do contorno).
//! - **Sombra projetada**: a silhueta desfocada (imagem), deslocada, atrás.
//! - **Sombra de bloco**: o volume vetorial que a forma varre na direção e
//!   profundidade dadas, atrás.
//! - **Envelope**: a forma deformada pela caixa de controle (cantos e meios
//!   dos lados), por um retalho de Coons.

use kurbo::{Affine, BezPath, PathEl, Point, Rect, Shape, Vec2};

use super::history::Editor;
use super::shapeops::{
    Join, Shapes, offset_region, path_object, region, shapes_to_path, union_all,
};
use crate::model::{Color, Effect, Fill, FillRule, GroupData, Object, ObjectKind};

fn solid_color(o: &Object) -> Option<Color> {
    match &o.fill {
        Some(Fill::Solid { color, .. }) => Some(color.clone()),
        _ => o.stroke.as_ref().map(|s| s.color.clone()),
    }
}

fn lerp_color(a: &Color, b: &Color, t: f64) -> Color {
    if a.model == b.model && a.values.len() == b.values.len() {
        let mut c = a.clone();
        c.reference = None;
        for (v, w) in c.values.iter_mut().zip(&b.values) {
            *v += (w - *v) * t;
        }
        return c;
    }
    let (x, y) = (a.to_srgb(), b.to_srgb());
    Color::rgb(
        x[0] + (y[0] - x[0]) * t,
        x[1] + (y[1] - x[1]) * t,
        x[2] + (y[2] - x[2]) * t,
    )
}

fn fill_of(o: &mut Object, c: Color) {
    o.fill = Some(Fill::solid(c));
}

fn translate(s: &Shapes, d: Vec2) -> Shapes {
    s.iter()
        .map(|sh| {
            sh.iter()
                .map(|c| c.iter().map(|p| [p[0] + d.x, p[1] + d.y]).collect())
                .collect()
        })
        .collect()
}

/// Primeiro anel externo da forma, como pontos.
fn outer_ring(o: &Object) -> Vec<Point> {
    let r = region(o, Affine::IDENTITY);
    r.into_iter()
        .flat_map(|s| s.into_iter().next())
        .max_by(|a, b| ring_area(a).abs().total_cmp(&ring_area(b).abs()))
        .map(|c| c.into_iter().map(|p| Point::new(p[0], p[1])).collect())
        .unwrap_or_default()
}

fn ring_area(c: &[[f64; 2]]) -> f64 {
    let n = c.len();
    (0..n)
        .map(|i| c[i][0] * c[(i + 1) % n][1] - c[(i + 1) % n][0] * c[i][1])
        .sum::<f64>()
        / 2.0
}

fn resample_ring(pts: &[Point], n: usize) -> Vec<Point> {
    let m = pts.len();
    if m < 2 {
        return pts.to_vec();
    }
    let lens: Vec<f64> = (0..m)
        .map(|i| (pts[(i + 1) % m] - pts[i]).hypot())
        .collect();
    let total: f64 = lens.iter().sum();
    let step = total / n as f64;
    let mut out = Vec::with_capacity(n);
    let (mut i, mut acc) = (0usize, 0.0);
    for k in 0..n {
        let target = k as f64 * step;
        while i < m - 1 && acc + lens[i] < target {
            acc += lens[i];
            i += 1;
        }
        let t = if lens[i] > 0.0 {
            ((target - acc) / lens[i]).clamp(0.0, 1.0)
        } else {
            0.0
        };
        out.push(pts[i] + (pts[(i + 1) % m] - pts[i]) * t);
    }
    out
}

fn polygon_object(pts: &[Point], like: &Object) -> Object {
    let mut p = BezPath::new();
    if let Some(f) = pts.first() {
        p.move_to(*f);
        for q in &pts[1..] {
            p.line_to(*q);
        }
        p.close_path();
    }
    let mut o = path_object(&p, like, FillRule::Nonzero);
    o.transform = crate::model::Transform::IDENTITY;
    o
}

/// Retalho de Coons do envelope: a caixa `frame` (x0, y0, x1, y1) vira a
/// forma de bordas curvas pelos 8 pontos (cantos e meios, a partir do canto
/// de baixo à esquerda, no sentido anti-horário).
pub fn envelope_map(frame: [f64; 4], pts: &[[f64; 2]; 8], p: Point) -> Point {
    let [x0, y0, x1, y1] = frame;
    let u = ((p.x - x0) / (x1 - x0).max(1e-9)).clamp(-0.5, 1.5);
    let v = ((p.y - y0) / (y1 - y0).max(1e-9)).clamp(-0.5, 1.5);
    let q = |i: usize| Point::new(pts[i][0], pts[i][1]);
    // Borda quadrática passando pelo meio: controle = 2·meio − (a + b)/2.
    let edge = |a: Point, m: Point, b: Point, t: f64| {
        let c = (m.to_vec2() * 2.0 - (a.to_vec2() + b.to_vec2()) / 2.0).to_point();
        let (s, t1) = (1.0 - t, t);
        (a.to_vec2() * (s * s) + c.to_vec2() * (2.0 * s * t1) + b.to_vec2() * (t1 * t1)).to_point()
    };
    let (bl, bm, br, rm, tr, tm, tl, lm) = (q(0), q(1), q(2), q(3), q(4), q(5), q(6), q(7));
    let bottom = edge(bl, bm, br, u);
    let top = edge(tl, tm, tr, u);
    let left = edge(bl, lm, tl, v);
    let right = edge(br, rm, tr, v);
    let lin = |a: Point, b: Point, t: f64| a + (b - a) * t;
    let ruled_v = lin(bottom, top, v);
    let ruled_u = lin(left, right, u);
    let bil = lin(lin(bl, br, u), lin(tl, tr, u), v);
    (ruled_v.to_vec2() + ruled_u.to_vec2() - bil.to_vec2()).to_point()
}

/// Pontos iniciais do envelope para a caixa (sem deformação).
pub fn envelope_points(b: Rect) -> [[f64; 2]; 8] {
    let (cx, cy) = ((b.x0 + b.x1) / 2.0, (b.y0 + b.y1) / 2.0);
    [
        [b.x0, b.y0],
        [cx, b.y0],
        [b.x1, b.y0],
        [b.x1, cy],
        [b.x1, b.y1],
        [cx, b.y1],
        [b.x0, b.y1],
        [b.x0, cy],
    ]
}

fn warp(o: &Object, parent: Affine, frame: [f64; 4], pts: &[[f64; 2]; 8]) -> Option<Object> {
    let m = parent * o.transform.affine();
    if let ObjectKind::Group(g) = &o.kind {
        let mut n = o.clone();
        n.transform = crate::model::Transform::IDENTITY;
        n.kind = ObjectKind::Group(GroupData {
            children: g
                .children
                .iter()
                .filter_map(|c| warp(c, m, frame, pts))
                .collect(),
            ..g.clone()
        });
        return Some(n);
    }
    let mut path = super::query::local_outline(o)?;
    path.apply_affine(m);
    let tol = ((frame[2] - frame[0]).abs() + (frame[3] - frame[1]).abs()) / 2000.0;
    // Retas subdivididas: a deformação curva também os lados retos.
    let step = ((frame[2] - frame[0]).abs().max((frame[3] - frame[1]).abs()) / 64.0).max(1.0);
    let mut out = BezPath::new();
    let mut last = Point::ORIGIN;
    let mut start = Point::ORIGIN;
    let seg = |out: &mut BezPath, a: Point, b: Point| {
        let n = ((b - a).hypot() / step).ceil().max(1.0) as usize;
        for k in 1..=n {
            out.line_to(envelope_map(
                frame,
                pts,
                a + (b - a) * (k as f64 / n as f64),
            ));
        }
    };
    kurbo::flatten(&path, tol.max(1.0), |el| match el {
        PathEl::MoveTo(p) => {
            out.move_to(envelope_map(frame, pts, p));
            last = p;
            start = p;
        }
        PathEl::LineTo(p) => {
            seg(&mut out, last, p);
            last = p;
        }
        PathEl::ClosePath => {
            seg(&mut out, last, start);
            out.close_path();
            last = start;
        }
        _ => {}
    });
    let rule = match &o.kind {
        ObjectKind::Path(p) => p.fill_rule,
        _ => FillRule::Nonzero,
    };
    let mut n = path_object(&out, o, rule);
    n.id = o.id.clone();
    n.name = o.name.clone();
    Some(n)
}

/// Objetos a desenhar (espaço do grupo), na ordem, com o efeito aplicado.
/// A sombra projetada desenha os filhos (a imagem vem de [`shadow_image`]).
pub fn expand(g: &GroupData) -> Vec<Object> {
    let Some(e) = &g.effect else {
        return g.children.clone();
    };
    match e {
        Effect::Contour {
            direction,
            steps,
            distance,
            end_color,
        } => {
            let Some(src) = g.children.first() else {
                return Vec::new();
            };
            let base = union_all(g.children.iter().map(|c| region(c, Affine::IDENTITY)));
            let c0 = solid_color(src).unwrap_or_else(Color::black);
            let c1 = end_color.clone().unwrap_or_else(|| c0.clone());
            let outside = direction != "inside";
            let n = (*steps).clamp(1, 200);
            let mut rings: Vec<Object> = (1..=n)
                .filter_map(|k| {
                    let d = distance.abs() * k as f64 * if outside { 1.0 } else { -1.0 };
                    let s = offset_region(&base, d, Join::Round);
                    if s.is_empty() {
                        return None;
                    }
                    let mut o = path_object(&shapes_to_path(&s), src, FillRule::Nonzero);
                    o.transform = crate::model::Transform::IDENTITY;
                    fill_of(&mut o, lerp_color(&c0, &c1, k as f64 / n as f64));
                    Some(o)
                })
                .collect();
            if outside {
                // Os maiores no fundo; a origem por cima.
                rings.reverse();
                rings.extend(g.children.iter().cloned());
                rings
            } else {
                let mut v = g.children.clone();
                v.extend(rings);
                v
            }
        }
        Effect::Blend { steps } => {
            let (Some(a), Some(b)) = (g.children.first(), g.children.get(1)) else {
                return g.children.clone();
            };
            let (ra, mut rb) = (outer_ring(a), outer_ring(b));
            if ra.len() < 3 || rb.len() < 3 {
                return g.children.clone();
            }
            const N: usize = 128;
            let pa = resample_ring(&ra, N);
            // Mesmo sentido e começo mais próximo, para não torcer.
            let area = |v: &[Point]| {
                v.iter()
                    .enumerate()
                    .map(|(i, p)| p.to_vec2().cross(v[(i + 1) % v.len()].to_vec2()))
                    .sum::<f64>()
            };
            if area(&ra).signum() != area(&rb).signum() {
                rb.reverse();
            }
            let mut pb = resample_ring(&rb, N);
            let ca = pa.iter().fold(Vec2::ZERO, |s, p| s + p.to_vec2()) / N as f64;
            let cb = pb.iter().fold(Vec2::ZERO, |s, p| s + p.to_vec2()) / N as f64;
            let shift = (0..N)
                .min_by(|&i, &j| {
                    let d = |k: usize| ((pb[k].to_vec2() - cb) - (pa[0].to_vec2() - ca)).hypot();
                    d(i).total_cmp(&d(j))
                })
                .unwrap_or(0);
            pb.rotate_left(shift);
            let (ca_col, cb_col) = (
                solid_color(a).unwrap_or_else(Color::black),
                solid_color(b).unwrap_or_else(Color::black),
            );
            let n = (*steps).clamp(1, 500);
            let mut out = vec![a.clone()];
            for k in 1..=n {
                let t = k as f64 / (n + 1) as f64;
                let pts: Vec<Point> = pa
                    .iter()
                    .zip(&pb)
                    .map(|(p, q)| *p + (*q - *p) * t)
                    .collect();
                let mut o = polygon_object(&pts, a);
                if a.fill.is_some() || b.fill.is_some() {
                    fill_of(&mut o, lerp_color(&ca_col, &cb_col, t));
                }
                if let (Some(sa), Some(sb)) = (&a.stroke, &b.stroke) {
                    let mut s = sa.clone();
                    s.width = sa.width + (sb.width - sa.width) * t;
                    s.color = lerp_color(&sa.color, &sb.color, t);
                    o.stroke = Some(s);
                }
                let (oa, ob) = (a.opacity.unwrap_or(1.0), b.opacity.unwrap_or(1.0));
                o.opacity = ((oa - ob).abs() > 1e-9 || oa < 1.0).then_some(oa + (ob - oa) * t);
                out.push(o);
            }
            out.extend(g.children.iter().skip(1).cloned());
            out
        }
        Effect::Block {
            angle,
            depth,
            color,
        } => {
            let base = union_all(g.children.iter().map(|c| region(c, Affine::IDENTITY)));
            let a = angle.to_radians();
            let d = Vec2::new(a.cos(), a.sin()) * *depth;
            // Varredura: a forma, a forma deslocada e um quadrilátero por aresta.
            let mut parts = vec![base.clone(), translate(&base, d)];
            for sh in &base {
                for c in sh {
                    let n = c.len();
                    for i in 0..n {
                        let (p, q) = (c[i], c[(i + 1) % n]);
                        parts.push(vec![vec![vec![
                            p,
                            q,
                            [q[0] + d.x, q[1] + d.y],
                            [p[0] + d.x, p[1] + d.y],
                        ]]]);
                    }
                }
            }
            let sweep = union_all(parts);
            let mut out = Vec::new();
            if let Some(src) = g.children.first() {
                let mut o = path_object(&shapes_to_path(&sweep), src, FillRule::Nonzero);
                o.transform = crate::model::Transform::IDENTITY;
                fill_of(&mut o, color.clone());
                o.stroke = None;
                out.push(o);
            }
            out.extend(g.children.iter().cloned());
            out
        }
        Effect::Envelope { frame, points } => g
            .children
            .iter()
            .filter_map(|c| warp(c, Affine::IDENTITY, *frame, points))
            .collect(),
        Effect::DropShadow { .. } => g.children.clone(),
    }
}

/// Sombra projetada: imagem RGBA (largura, altura em pixels, dados sem
/// pré-multiplicar), matriz imagem→página da caixa `[0,w]×[0,h]` (µm, topo
/// em y = h) e o tamanho em µm. `m` = matriz do grupo na página.
pub fn shadow_image(g: &GroupData, m: Affine) -> Option<(u32, u32, Vec<u8>, Affine, f64, f64)> {
    let Some(Effect::DropShadow {
        offset,
        blur,
        color,
        opacity,
    }) = &g.effect
    else {
        return None;
    };
    let shapes = union_all(g.children.iter().map(|c| region(c, m)));
    let path = shapes_to_path(&shapes);
    let b = path.bounding_box();
    if b.width() <= 0.0 || b.height() <= 0.0 {
        return None;
    }
    let pad = blur.abs() * 2.0 + 500.0;
    let b = b.inflate(pad, pad);
    // Resolução: até 1024 px no lado maior, no máximo 150 ppp.
    let um_per_px = (b.width().max(b.height()) / 1024.0).max(25_400.0 / 150.0);
    let (w, h) = (
        (b.width() / um_per_px).ceil() as u32,
        (b.height() / um_per_px).ceil() as u32,
    );
    let mut pm = tiny_skia::Pixmap::new(w.max(1), h.max(1))?;
    let mut pb = tiny_skia::PathBuilder::new();
    kurbo::flatten(&path, um_per_px / 4.0, |el| match el {
        PathEl::MoveTo(p) => pb.move_to(
            ((p.x - b.x0) / um_per_px) as f32,
            ((b.y1 - p.y) / um_per_px) as f32,
        ),
        PathEl::LineTo(p) => pb.line_to(
            ((p.x - b.x0) / um_per_px) as f32,
            ((b.y1 - p.y) / um_per_px) as f32,
        ),
        PathEl::ClosePath => pb.close(),
        _ => {}
    });
    let sp = pb.finish()?;
    let mut paint = tiny_skia::Paint::default();
    paint.set_color_rgba8(0, 0, 0, 255);
    paint.anti_alias = true;
    pm.fill_path(
        &sp,
        &paint,
        tiny_skia::FillRule::Winding,
        tiny_skia::Transform::identity(),
        None,
    );
    // Desfoque: três passadas de caixa (aproxima o gaussiano).
    let mut a: Vec<f32> = pm
        .pixels()
        .iter()
        .map(|p| p.alpha() as f32 / 255.0)
        .collect();
    let r = ((blur / um_per_px) / 2.0).round().max(0.0) as usize;
    if r > 0 {
        for _ in 0..3 {
            box_blur(&mut a, w as usize, h as usize, r);
        }
    }
    let [cr, cg, cb] = color.to_srgb();
    let q = |v: f64| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    let mut rgba = Vec::with_capacity(a.len() * 4);
    for v in a {
        rgba.extend([q(cr), q(cg), q(cb), q(v as f64 * opacity)]);
    }
    let (wu, hu) = (w as f64 * um_per_px, h as f64 * um_per_px);
    let t = Affine::translate((b.x0 + offset[0], b.y1 - hu + offset[1]));
    Some((w, h, rgba, t, wu, hu))
}

fn box_blur(a: &mut [f32], w: usize, h: usize, r: usize) {
    let mut tmp = vec![0f32; a.len()];
    let k = (2 * r + 1) as f32;
    for y in 0..h {
        let row = &a[y * w..(y + 1) * w];
        let mut s: f32 = (0..=r).map(|i| row[i.min(w - 1)]).sum::<f32>() + row[0] * r as f32;
        for x in 0..w {
            tmp[y * w + x] = s / k;
            let add = row[(x + r + 1).min(w - 1)];
            let sub = row[x.saturating_sub(r)];
            s += add - sub;
        }
    }
    for x in 0..w {
        let col = |y: usize| tmp[y.min(h - 1) * w + x];
        let mut s: f32 = (0..=r).map(col).sum::<f32>() + col(0) * r as f32;
        for y in 0..h {
            a[y * w + x] = s / k;
            s += col(y + r + 1) - col(y.saturating_sub(r));
        }
    }
}

/// Aplica o efeito: agrupa `sel` (ou envolve um objeto só) num grupo com o
/// efeito, num passo. Devolve o ID do grupo.
pub fn apply(ed: &mut Editor, sel: &[String], effect: Effect) -> Option<String> {
    if sel.is_empty() {
        return None;
    }
    let single = match sel {
        [one] => {
            let r = ed.find(one)?;
            Some((r.clone(), ed.get(&r)?.clone()))
        }
        _ => None,
    };
    ed.begin("Aplicar efeito");
    let gid = match single {
        Some((r, o)) => {
            let g = Object::new(ObjectKind::Group(GroupData {
                children: vec![o],
                boolean: None,
                symmetry: None,
                effect: None,
                extra: Default::default(),
            }));
            let id = g.id.clone();
            ed.replace_at(&r, g);
            Some(id)
        }
        None => super::ops::group(ed, sel),
    };
    if let Some(gid) = &gid {
        // Envelope: a caixa do conteúdo no espaço do grupo.
        let effect = match effect {
            Effect::Envelope { .. } => {
                let r = ed.find(gid);
                let b = r
                    .and_then(|r| ed.get(&r))
                    .and_then(|o| match &o.kind {
                        ObjectKind::Group(g) => {
                            super::query::union_bounds(g.children.iter(), false)
                        }
                        _ => None,
                    })
                    .unwrap_or(Rect::new(0.0, 0.0, 1.0, 1.0));
                Effect::Envelope {
                    frame: [b.x0, b.y0, b.x1, b.y1],
                    points: envelope_points(b),
                }
            }
            e => e,
        };
        super::ops::modify(ed, std::slice::from_ref(gid), "Efeito", |o| {
            if let ObjectKind::Group(g) = &mut o.kind {
                g.effect = Some(effect.clone());
                return true;
            }
            false
        });
    }
    ed.commit();
    gid
}

/// Resultado congelado (Separar): os objetos gerados com IDs novos (os de
/// origem mantêm os seus) e, na sombra projetada, a imagem.
pub fn baked(ed: &mut Editor, g: &GroupData, m: Affine) -> Vec<Object> {
    let ids: std::collections::HashSet<&str> = g.children.iter().map(|c| c.id.as_str()).collect();
    let mut out: Vec<Object> = expand(g)
        .into_iter()
        .map(|mut o| {
            if !ids.contains(o.id.as_str()) || matches!(g.effect, Some(Effect::Envelope { .. })) {
                super::ops::renew_ids(&mut o);
            }
            o
        })
        .collect();
    if let Some((w, h, rgba, t, wu, hu)) = shadow_image(g, Affine::IDENTITY)
        && let Some(png) = encode_png(w, h, &rgba)
    {
        let src = ed.doc.add_image(png, "png");
        let mut img = Object::new(ObjectKind::Image(crate::model::ImageData {
            width: wu,
            height: hu,
            src,
            extra: Default::default(),
        }));
        img.transform = crate::model::Transform::from_affine(t);
        out.insert(0, img);
    }
    let _ = m;
    out
}

fn encode_png(w: u32, h: u32, rgba: &[u8]) -> Option<Vec<u8>> {
    let mut buf = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut buf, w, h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut wr = enc.write_header().ok()?;
        wr.write_image_data(rgba).ok()?;
    }
    Some(buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Document, Page, RectData, Transform};

    fn sq(x: f64, w: f64, c: Color) -> Object {
        let mut o = Object::new(ObjectKind::Rectangle(RectData {
            width: w,
            height: w,
            corners: None,
            extra: Default::default(),
        }));
        o.transform = Transform::translate(x, 0.0);
        o.fill = Some(Fill::solid(c));
        o
    }

    fn group(children: Vec<Object>, e: Effect) -> GroupData {
        GroupData {
            children,
            boolean: None,
            symmetry: None,
            effect: Some(e),
            extra: Default::default(),
        }
    }

    fn bbox(o: &Object) -> Rect {
        super::super::query::bounds(o, Affine::IDENTITY, false).unwrap()
    }

    #[test]
    fn contour_outside_and_inside() {
        let g = group(
            vec![sq(0.0, 100_000.0, Color::rgb(1.0, 0.0, 0.0))],
            Effect::Contour {
                direction: "outside".into(),
                steps: 3,
                distance: 10_000.0,
                end_color: Some(Color::rgb(0.0, 0.0, 1.0)),
            },
        );
        let v = expand(&g);
        assert_eq!(v.len(), 4);
        assert!(
            (bbox(&v[0]).width() - 160_000.0).abs() < 200.0,
            "maior no fundo: {:?}",
            bbox(&v[0])
        );
        assert_eq!(v[3].id, g.children[0].id, "origem por cima");
        let gi = group(
            vec![sq(0.0, 100_000.0, Color::black())],
            Effect::Contour {
                direction: "inside".into(),
                steps: 10,
                distance: 10_000.0,
                end_color: None,
            },
        );
        // Para dentro: a partir de 50 a forma some.
        assert_eq!(expand(&gi).len(), 1 + 4);
    }

    #[test]
    fn blend_interpolates() {
        let g = group(
            vec![
                sq(0.0, 100.0, Color::rgb(1.0, 0.0, 0.0)),
                sq(400.0, 50.0, Color::rgb(0.0, 0.0, 1.0)),
            ],
            Effect::Blend { steps: 3 },
        );
        let v = expand(&g);
        assert_eq!(v.len(), 5);
        let mid = bbox(&v[2]);
        assert!((mid.width() - 75.0).abs() < 3.0, "{mid:?}");
        assert!((mid.x0 - 200.0).abs() < 3.0, "{mid:?}");
        let Some(Fill::Solid { color, .. }) = &v[2].fill else {
            panic!()
        };
        let c = color.to_srgb();
        assert!((c[0] - 0.5).abs() < 0.05 && (c[2] - 0.5).abs() < 0.05);
    }

    #[test]
    fn block_and_envelope() {
        let g = group(
            vec![sq(0.0, 100.0, Color::black())],
            Effect::Block {
                angle: -45.0,
                depth: 50.0,
                color: Color::rgb(0.5, 0.5, 0.5),
            },
        );
        let v = expand(&g);
        let b = bbox(&v[0]);
        assert!(
            (b.width() - (100.0 + 50.0 / 2f64.sqrt())).abs() < 1.0,
            "{b:?}"
        );
        // Envelope sem deformação: igual; com o meio de cima subido, cresce.
        let r = Rect::new(0.0, 0.0, 100.0, 100.0);
        let mut pts = envelope_points(r);
        let id = envelope_map([0.0, 0.0, 100.0, 100.0], &pts, Point::new(30.0, 70.0));
        assert!((id - Point::new(30.0, 70.0)).hypot() < 1e-9);
        pts[5][1] = 140.0;
        let g = group(
            vec![sq(0.0, 100.0, Color::black())],
            Effect::Envelope {
                frame: [0.0, 0.0, 100.0, 100.0],
                points: pts,
            },
        );
        let b = bbox(&expand(&g)[0]);
        assert!((b.height() - 140.0).abs() < 2.0, "{b:?}");
    }

    #[test]
    fn shadow_and_apply_and_bake() {
        let g = group(
            vec![sq(0.0, 10_000.0, Color::black())],
            Effect::DropShadow {
                offset: [1_000.0, -1_000.0],
                blur: 2_000.0,
                color: Color::black(),
                opacity: 0.5,
            },
        );
        let (w, h, rgba, _, _, _) = shadow_image(&g, Affine::IDENTITY).unwrap();
        assert_eq!(rgba.len(), (w * h * 4) as usize);
        let centre = rgba[(((h / 2) * w + w / 2) * 4 + 3) as usize];
        assert!(centre > 100 && centre <= 128, "meio da sombra: {centre}");
        let mut d = Document::new();
        d.data.pages.push(Page::new(100_000.0, 100_000.0));
        let mut ed = Editor::new(d);
        let id =
            super::super::ops::add_object(&mut ed, 0, 0, sq(0.0, 10_000.0, Color::black()), "q");
        let gid = apply(
            &mut ed,
            &[id],
            Effect::Contour {
                direction: "outside".into(),
                steps: 2,
                distance: 1_000.0,
                end_color: None,
            },
        )
        .unwrap();
        assert_eq!(ed.undo_len(), 2);
        let out = super::super::ops::ungroup(&mut ed, std::slice::from_ref(&gid));
        assert_eq!(out.len(), 3, "congelado: anéis + origem");
        let o = Object::new(ObjectKind::Group(group(vec![], Effect::Blend { steps: 4 })));
        assert_eq!(Object::from_value(o.to_value()), o);
    }
}
