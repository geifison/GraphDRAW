//! Desenho inteligente (GD-FER-27): o traço livre é reconhecido como
//! retângulo, elipse, triângulo, reta ou seta, conforme o nível de
//! reconhecimento (0 = só formas quase perfeitas, 1 = aceita traços tortos).
//!
//! O traço é reamostrado por comprimento (a velocidade do gesto não pesa).
//! Fechado: o retângulo é o de menor área que contém o fecho convexo, a
//! elipse vem da covariância dos pontos (eixos e giro) e o triângulo é o de
//! maior área com vértices no fecho; vence a forma com menor erro médio
//! (distância dos pontos à borda, relativa ao tamanho). Aberto: reta se
//! todos os pontos ficam perto dela; seta se a haste domina e o resto do
//! traço é uma ponta junto ao fim dela.

use kurbo::{Point, Vec2};

/// Forma reconhecida (coordenadas da página).
#[derive(Debug, Clone, PartialEq)]
pub enum Recognized {
    Line(Point, Point),
    Arrow(Point, Point),
    /// Centro, largura, altura e giro (rad).
    Rect {
        center: Point,
        w: f64,
        h: f64,
        angle: f64,
    },
    /// Centro, raios e giro (rad).
    Ellipse {
        center: Point,
        rx: f64,
        ry: f64,
        angle: f64,
    },
    Triangle([Point; 3]),
}

fn resample(pts: &[Point], n: usize) -> Vec<Point> {
    let len: f64 = pts.windows(2).map(|w| (w[1] - w[0]).hypot()).sum();
    if pts.len() < 2 || len <= 0.0 {
        return pts.to_vec();
    }
    let step = len / (n - 1) as f64;
    let mut out = vec![pts[0]];
    let mut acc = 0.0;
    let mut i = 1;
    let mut prev = pts[0];
    while i < pts.len() && out.len() < n {
        let d = (pts[i] - prev).hypot();
        if acc + d >= step && d > 0.0 {
            let t = (step - acc) / d;
            let q = prev + (pts[i] - prev) * t;
            out.push(q);
            prev = q;
            acc = 0.0;
        } else {
            acc += d;
            prev = pts[i];
            i += 1;
        }
    }
    if out.len() < n {
        out.push(*pts.last().unwrap());
    }
    out
}

fn hull(pts: &[Point]) -> Vec<Point> {
    let mut p: Vec<Point> = pts.to_vec();
    p.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    p.dedup_by(|a, b| (*a - *b).hypot() < 1e-9);
    if p.len() < 3 {
        return p;
    }
    let cross = |o: Point, a: Point, b: Point| (a - o).cross(b - o);
    let mut lower: Vec<Point> = Vec::new();
    for &q in &p {
        while lower.len() >= 2 && cross(lower[lower.len() - 2], lower[lower.len() - 1], q) <= 0.0 {
            lower.pop();
        }
        lower.push(q);
    }
    let mut upper: Vec<Point> = Vec::new();
    for &q in p.iter().rev() {
        while upper.len() >= 2 && cross(upper[upper.len() - 2], upper[upper.len() - 1], q) <= 0.0 {
            upper.pop();
        }
        upper.push(q);
    }
    lower.pop();
    upper.pop();
    lower.extend(upper);
    lower
}

fn seg_dist(p: Point, a: Point, b: Point) -> f64 {
    let ab = b - a;
    let t = if ab.hypot2() > 0.0 {
        ((p - a).dot(ab) / ab.hypot2()).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (p - (a + ab * t)).hypot()
}

fn poly_dist(p: Point, poly: &[Point]) -> f64 {
    (0..poly.len())
        .map(|i| seg_dist(p, poly[i], poly[(i + 1) % poly.len()]))
        .fold(f64::INFINITY, f64::min)
}

/// Retângulo de menor área que contém o fecho (um lado alinhado a uma aresta).
fn min_rect(h: &[Point]) -> (Point, f64, f64, f64) {
    let mut best = (Point::ORIGIN, 0.0, 0.0, 0.0, f64::INFINITY);
    for i in 0..h.len() {
        let e = h[(i + 1) % h.len()] - h[i];
        if e.hypot() < 1e-9 {
            continue;
        }
        let u = e / e.hypot();
        let v = Vec2::new(-u.y, u.x);
        let (mut a0, mut a1, mut b0, mut b1) = (
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
        );
        for q in h {
            let d = q.to_vec2();
            let (a, b) = (d.dot(u), d.dot(v));
            a0 = a0.min(a);
            a1 = a1.max(a);
            b0 = b0.min(b);
            b1 = b1.max(b);
        }
        let area = (a1 - a0) * (b1 - b0);
        if area < best.4 {
            let c = (u * ((a0 + a1) / 2.0) + v * ((b0 + b1) / 2.0)).to_point();
            best = (c, a1 - a0, b1 - b0, u.y.atan2(u.x), area);
        }
    }
    (best.0, best.1, best.2, best.3)
}

fn rect_corners(c: Point, w: f64, h: f64, a: f64) -> [Point; 4] {
    let (u, v) = (Vec2::new(a.cos(), a.sin()), Vec2::new(-a.sin(), a.cos()));
    let (hu, hv) = (u * (w / 2.0), v * (h / 2.0));
    [c - hu - hv, c + hu - hv, c + hu + hv, c - hu + hv]
}

/// Reconhece o traço `pts` (página); `level` 0–1. `None` = fica à mão livre.
pub fn recognize(pts: &[Point], level: f64) -> Option<Recognized> {
    if pts.len() < 4 {
        return None;
    }
    let p = resample(pts, 64);
    let (minx, maxx) = p
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), q| {
            (a.min(q.x), b.max(q.x))
        });
    let (miny, maxy) = p
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), q| {
            (a.min(q.y), b.max(q.y))
        });
    let size = (maxx - minx).hypot(maxy - miny);
    if size <= 0.0 {
        return None;
    }
    let level = level.clamp(0.0, 1.0);
    let tol = 0.025 + 0.075 * level;
    let (first, last) = (p[0], *p.last().unwrap());
    let closed = (last - first).hypot() < size * (0.12 + 0.18 * level);
    if !closed {
        let err = p
            .iter()
            .map(|q| seg_dist(*q, first, last))
            .fold(0.0, f64::max)
            / size;
        if err <= tol * 1.5 {
            return Some(Recognized::Line(first, last));
        }
        // Seta: haste do começo até o canto mais distante, e a ponta toda
        // perto do fim da haste.
        let (tip_i, _) = p
            .iter()
            .enumerate()
            .map(|(i, q)| (i, (*q - first).hypot()))
            .fold((0, 0.0), |b, x| if x.1 > b.1 { x } else { b });
        let tip = p[tip_i];
        let shaft = (tip - first).hypot();
        let shaft_err = p[..=tip_i]
            .iter()
            .map(|q| seg_dist(*q, first, tip))
            .fold(0.0, f64::max)
            / shaft.max(1e-9);
        let head_ok = p[tip_i..].iter().all(|q| (*q - tip).hypot() < shaft * 0.4);
        let head_len: f64 = p[tip_i..].windows(2).map(|w| (w[1] - w[0]).hypot()).sum();
        if tip_i > 4 && shaft_err <= tol * 2.0 && head_ok && head_len > shaft * 0.1 {
            return Some(Recognized::Arrow(first, tip));
        }
        return None;
    }
    let h = hull(&p);
    if h.len() < 3 {
        return None;
    }
    let err_of =
        |poly: &[Point]| p.iter().map(|q| poly_dist(*q, poly)).sum::<f64>() / p.len() as f64 / size;
    let mut cands: Vec<(f64, Recognized)> = Vec::new();
    // Retângulo.
    let (c, w, hh, a) = min_rect(&h);
    cands.push((
        err_of(&rect_corners(c, w, hh, a)),
        Recognized::Rect {
            center: c,
            w,
            h: hh,
            angle: a,
        },
    ));
    // Elipse pela covariância.
    let n = p.len() as f64;
    let m = p.iter().fold(Vec2::ZERO, |s, q| s + q.to_vec2()) / n;
    let (mut sxx, mut syy, mut sxy) = (0.0, 0.0, 0.0);
    for q in &p {
        let d = q.to_vec2() - m;
        sxx += d.x * d.x;
        syy += d.y * d.y;
        sxy += d.x * d.y;
    }
    let (sxx, syy, sxy) = (sxx / n, syy / n, sxy / n);
    let ang = 0.5 * (2.0 * sxy).atan2(sxx - syy);
    let (u, v) = (
        Vec2::new(ang.cos(), ang.sin()),
        Vec2::new(-ang.sin(), ang.cos()),
    );
    let rms = |ax: Vec2| {
        (p.iter()
            .map(|q| (q.to_vec2() - m).dot(ax).powi(2))
            .sum::<f64>()
            / n)
            .sqrt()
    };
    let (rx, ry) = (
        rms(u) * std::f64::consts::SQRT_2,
        rms(v) * std::f64::consts::SQRT_2,
    );
    let e_err = p
        .iter()
        .map(|q| {
            let d = q.to_vec2() - m;
            let (x, y) = (d.dot(u) / rx.max(1e-9), d.dot(v) / ry.max(1e-9));
            ((x * x + y * y).sqrt() - 1.0).abs() * rx.min(ry)
        })
        .sum::<f64>()
        / n
        / size;
    cands.push((
        e_err,
        Recognized::Ellipse {
            center: m.to_point(),
            rx,
            ry,
            angle: ang,
        },
    ));
    // Triângulo de maior área no fecho.
    let mut tri = [h[0], h[1], h[2]];
    let mut best = 0.0;
    for i in 0..h.len() {
        for j in i + 1..h.len() {
            for k in j + 1..h.len() {
                let ar = ((h[j] - h[i]).cross(h[k] - h[i])).abs();
                if ar > best {
                    best = ar;
                    tri = [h[i], h[j], h[k]];
                }
            }
        }
    }
    cands.push((err_of(&tri), Recognized::Triangle(tri)));
    cands.sort_by(|a, b| a.0.total_cmp(&b.0));
    let (e, r) = cands.into_iter().next()?;
    (e <= tol).then_some(r)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wobble(pts: Vec<Point>, amp: f64) -> Vec<Point> {
        pts.into_iter()
            .enumerate()
            .map(|(i, q)| q + Vec2::new((i as f64 * 1.7).sin() * amp, (i as f64 * 2.3).cos() * amp))
            .collect()
    }

    fn along(poly: &[Point], closed: bool, per: usize) -> Vec<Point> {
        let mut out = Vec::new();
        let n = poly.len();
        let segs = if closed { n } else { n - 1 };
        for i in 0..segs {
            let (a, b) = (poly[i], poly[(i + 1) % n]);
            for k in 0..per {
                out.push(a + (b - a) * (k as f64 / per as f64));
            }
        }
        out.push(if closed { poly[0] } else { poly[n - 1] });
        out
    }

    /// GD-FER-27: retângulo, elipse, triângulo, reta e seta tortos são
    /// reconhecidos; um rabisco não.
    #[test]
    fn gd_fer_27_shapes() {
        let rect = along(
            &[
                Point::new(0.0, 0.0),
                Point::new(100.0, 0.0),
                Point::new(100.0, 60.0),
                Point::new(0.0, 60.0),
            ],
            true,
            20,
        );
        match recognize(&wobble(rect, 1.5), 0.5) {
            Some(Recognized::Rect { w, h, .. }) => {
                assert!((w.max(h) - 100.0).abs() < 6.0 && (w.min(h) - 60.0).abs() < 6.0)
            }
            r => panic!("retângulo: {r:?}"),
        }
        let circle: Vec<Point> = (0..=80)
            .map(|i| {
                let t = i as f64 / 80.0 * std::f64::consts::TAU;
                Point::new(50.0 + 40.0 * t.cos(), 50.0 + 25.0 * t.sin())
            })
            .collect();
        match recognize(&wobble(circle, 1.0), 0.5) {
            Some(Recognized::Ellipse { rx, ry, .. }) => assert!(
                (rx.max(ry) - 40.0).abs() < 4.0 && (rx.min(ry) - 25.0).abs() < 4.0,
                "{rx} {ry}"
            ),
            r => panic!("elipse: {r:?}"),
        }
        let tri = along(
            &[
                Point::new(0.0, 0.0),
                Point::new(100.0, 0.0),
                Point::new(40.0, 80.0),
            ],
            true,
            25,
        );
        assert!(matches!(
            recognize(&wobble(tri, 1.0), 0.5),
            Some(Recognized::Triangle(_))
        ));
        let line = along(&[Point::new(0.0, 0.0), Point::new(100.0, 20.0)], false, 40);
        assert!(matches!(
            recognize(&wobble(line, 0.8), 0.5),
            Some(Recognized::Line(..))
        ));
        // Seta: haste e depois a ponta em V junto ao fim.
        let arrow = along(
            &[
                Point::new(0.0, 0.0),
                Point::new(100.0, 0.0),
                Point::new(85.0, 10.0),
                Point::new(100.0, 0.0),
                Point::new(85.0, -10.0),
            ],
            false,
            15,
        );
        assert!(
            matches!(recognize(&arrow, 0.5), Some(Recognized::Arrow(..))),
            "{:?}",
            recognize(&arrow, 0.5)
        );
        // Rabisco: zigue-zague aberto largo.
        let scribble = along(
            &[
                Point::new(0.0, 0.0),
                Point::new(30.0, 80.0),
                Point::new(60.0, 0.0),
                Point::new(90.0, 80.0),
            ],
            false,
            10,
        );
        assert!(recognize(&scribble, 0.5).is_none());
        // Nível 0 é exigente: o retângulo bem torto não passa.
        let bad = along(
            &[
                Point::new(0.0, 0.0),
                Point::new(100.0, 10.0),
                Point::new(90.0, 60.0),
                Point::new(5.0, 50.0),
            ],
            true,
            20,
        );
        assert!(!matches!(
            recognize(&wobble(bad, 4.0), 0.0),
            Some(Recognized::Rect { .. })
        ));
    }
}
