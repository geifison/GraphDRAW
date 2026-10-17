//! Parâmetro dos gradientes (GD-FIL-03): para cada ponto local, quanto
//! (0..1) do caminho entre as paradas. Linear e radial têm suporte nativo
//! nos renderizadores; elíptico, cônico e retangular usam esta função
//! (pixel a pixel na tela, função PostScript no PDF, imagem no SVG).
//!
//! Elíptico, cônico e retangular usam `center`, `radius` (raio em x, ou
//! meia largura), `radiusY` (raio em y; padrão = `radius`) e `angle`
//! (graus, giro dos eixos; no cônico, onde fica a parada 0).

use kurbo::Point;

use crate::model::{Color, Gradient, GradientStop};

/// Tipos com desenho nativo (tiny-skia, PDF axial/radial, SVG).
pub fn is_native(g: &Gradient) -> bool {
    !matches!(g.kind.as_str(), "elliptic" | "conic" | "rectangular")
}

/// Centro, raios e ângulo (rad) dos tipos centrados.
fn frame(g: &Gradient) -> (Point, f64, f64, f64) {
    let c = g.center.unwrap_or([0.0, 0.0]);
    let rx = g.radius.unwrap_or(1.0).abs().max(1e-9);
    let ry = g.radius_y.unwrap_or(rx).abs().max(1e-9);
    (
        Point::new(c[0], c[1]),
        rx,
        ry,
        g.angle.unwrap_or(0.0).to_radians(),
    )
}

/// Parâmetro antes do espalhamento.
pub fn raw(g: &Gradient, p: Point) -> f64 {
    match g.kind.as_str() {
        "elliptic" | "conic" | "rectangular" => {
            let (c, rx, ry, a) = frame(g);
            let (dx, dy) = (p.x - c.x, p.y - c.y);
            let (ca, sa) = (a.cos(), a.sin());
            let u = dx * ca + dy * sa;
            let v = -dx * sa + dy * ca;
            match g.kind.as_str() {
                "elliptic" => ((u / rx).powi(2) + (v / ry).powi(2)).sqrt(),
                "rectangular" => (u / rx).abs().max((v / ry).abs()),
                _ => {
                    let t = v.atan2(u).to_degrees();
                    (if t < 0.0 { t + 360.0 } else { t }) / 360.0
                }
            }
        }
        "radial" => {
            let c = g.center.unwrap_or([0.0, 0.0]);
            (p - Point::new(c[0], c[1])).hypot() / g.radius.unwrap_or(1.0).abs().max(1e-9)
        }
        _ => {
            let a = g.start.unwrap_or([0.0, 0.0]);
            let b = g.end.unwrap_or([1.0, 0.0]);
            let (a, b) = (Point::new(a[0], a[1]), Point::new(b[0], b[1]));
            let d = b - a;
            (p - a).dot(d) / d.hypot2().max(1e-18)
        }
    }
}

/// Espalhamento: `pad` prende em 0..1, `repeat` repete, `reflect` espelha.
pub fn spread(g: &Gradient, t: f64) -> f64 {
    match g.spread.as_str() {
        "repeat" => t - t.floor(),
        "reflect" => {
            let m = t - 2.0 * (t / 2.0).floor();
            if m > 1.0 { 2.0 - m } else { m }
        }
        _ => t.clamp(0.0, 1.0),
    }
}

pub fn param(g: &Gradient, p: Point) -> f64 {
    spread(g, raw(g, p))
}

fn lerp_color(a: &Color, b: &Color, t: f64) -> Color {
    let (ca, cb) = (a.to_srgb(), b.to_srgb());
    let m = |i: usize| ca[i] + (cb[i] - ca[i]) * t;
    Color::rgb(m(0), m(1), m(2))
}

/// Cor e opacidade do gradiente no parâmetro `t`.
pub fn color_at(g: &Gradient, t: f64) -> (Color, f64) {
    let mut stops: Vec<&GradientStop> = g.stops.iter().collect();
    stops.sort_by(|a, b| a.offset.total_cmp(&b.offset));
    let (Some(first), Some(last)) = (stops.first(), stops.last()) else {
        return (Color::black(), 1.0);
    };
    if t <= first.offset {
        return (first.color.clone(), first.opacity);
    }
    if t >= last.offset {
        return (last.color.clone(), last.opacity);
    }
    for w in stops.windows(2) {
        let (a, b) = (w[0], w[1]);
        if t >= a.offset && t <= b.offset {
            let k = if b.offset > a.offset {
                (t - a.offset) / (b.offset - a.offset)
            } else {
                0.0
            };
            return (
                lerp_color(&a.color, &b.color, k),
                a.opacity + (b.opacity - a.opacity) * k,
            );
        }
    }
    (last.color.clone(), last.opacity)
}

thread_local! {
    /// Tabelas já calculadas (paradas + alfa → cores), por linha de
    /// execução: a conversão de cor das 256 posições pesa quando há muitos
    /// objetos com o mesmo gradiente.
    static LUTS: std::cell::RefCell<std::collections::HashMap<u64, std::sync::Arc<Vec<[u8; 4]>>>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

fn lut_key(g: &Gradient, alpha: f64) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    alpha.to_bits().hash(&mut h);
    // Perfil de cor ativo: trocar de perfil muda as cores da tabela.
    crate::cms::current()
        .map(|c| std::sync::Arc::as_ptr(&c) as usize)
        .hash(&mut h);
    for s in &g.stops {
        s.offset.to_bits().hash(&mut h);
        s.opacity.to_bits().hash(&mut h);
        s.color.model.hash(&mut h);
        for v in &s.color.values {
            v.to_bits().hash(&mut h);
        }
        s.color.reference.hash(&mut h);
    }
    h.finish()
}

/// Tabela de cores com cache (ver [`lut`]).
pub fn lut_cached(g: &Gradient, alpha: f64) -> std::sync::Arc<Vec<[u8; 4]>> {
    let key = lut_key(g, alpha);
    LUTS.with(|m| {
        let mut m = m.borrow_mut();
        if let Some(v) = m.get(&key) {
            return v.clone();
        }
        if m.len() > 4096 {
            m.clear();
        }
        let v = std::sync::Arc::new(lut(g, alpha));
        m.insert(key, v.clone());
        v
    })
}

/// Tabela de 256 cores RGBA (sRGB, sem pré-multiplicar) ao longo de 0..1.
pub fn lut(g: &Gradient, alpha: f64) -> Vec<[u8; 4]> {
    (0..256)
        .map(|i| {
            let (c, o) = color_at(g, i as f64 / 255.0);
            let [r, gg, b] = c.to_srgb();
            let q = |v: f64| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
            [q(r), q(gg), q(b), q(o * alpha)]
        })
        .collect()
}

fn num(v: f64) -> String {
    let s = format!("{v:.6}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-" || s == "-0" {
        "0".into()
    } else {
        s.into()
    }
}

/// Código PostScript (função tipo 4 do PDF) que leva `x y` locais ao
/// parâmetro já espalhado; o chamador acrescenta as cores.
pub fn ps_param(g: &Gradient) -> String {
    let (c, rx, ry, a) = frame(g);
    let (ca, sa) = (a.cos(), a.sin());
    // x y → dx dy → u v
    let mut s = format!(
        "{} sub exch {} sub exch 2 copy {} mul exch {} mul add 3 1 roll {} mul exch {} mul add ",
        num(c.y),
        num(c.x),
        num(sa),
        num(ca),
        num(ca),
        num(-sa)
    );
    // Pilha: u v.
    match g.kind.as_str() {
        "elliptic" => {
            s += &format!(
                "{} div dup mul exch {} div dup mul add sqrt ",
                num(ry),
                num(rx)
            )
        }
        "rectangular" => {
            s += &format!(
                "{} div abs exch {} div abs 2 copy lt {{exch}} if pop ",
                num(ry),
                num(rx)
            )
        }
        // No centro exato, `0 0 atan` é erro no PostScript: vale 0.
        _ => s += "exch 2 copy abs exch abs add 0 eq {pop pop 0} {atan 360 div} ifelse ",
    }
    match g.spread.as_str() {
        "repeat" => s += "dup floor sub ",
        "reflect" => s += "dup 2 div floor 2 mul sub dup 1 gt {2 exch sub} if ",
        _ => s += "dup 0 lt {pop 0} if dup 1 gt {pop 1} if ",
    }
    s
}

/// Função PostScript completa `{ x y → c1 … cn }` com as paradas (valores
/// por parada já no espaço de cor do PDF, ordenadas por posição).
pub fn ps_function(g: &Gradient, stops: &[(f64, Vec<f32>)]) -> String {
    let n = stops.first().map_or(0, |s| s.1.len());
    // Interpolação entre a parada i e i+1, com t na pilha.
    let seg = |i: usize| -> String {
        let (o0, c0) = (&stops[i].0, &stops[i].1);
        let (o1, c1) = (&stops[i + 1].0, &stops[i + 1].1);
        let w = o1 - o0;
        if w < 1e-6 {
            // Parada dura: a cor da parada seguinte, sem dividir por zero.
            let mut s = "pop ".to_string();
            for v in c1.iter().take(n) {
                s += &format!("{} ", num(*v as f64));
            }
            return s;
        }
        let mut s = format!("{} sub {} div ", num(*o0), num(w));
        for k in 0..n {
            let d = c1[k] as f64 - c0[k] as f64;
            if k + 1 < n {
                s += &format!("dup {} mul {} add exch ", num(d), num(c0[k] as f64));
            } else {
                s += &format!("{} mul {} add ", num(d), num(c0[k] as f64));
            }
        }
        s
    };
    let mut body = seg(stops.len() - 2);
    for i in (0..stops.len() - 2).rev() {
        body = format!(
            "dup {} le {{ {} }} {{ {} }} ifelse ",
            num(stops[i + 1].0),
            seg(i),
            body
        );
    }
    format!("{{ {}{}}}", ps_param(g), body)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn g(kind: &str) -> Gradient {
        Gradient {
            kind: kind.into(),
            stops: vec![
                GradientStop {
                    offset: 0.0,
                    color: Color::black(),
                    opacity: 1.0,
                    extra: Default::default(),
                },
                GradientStop {
                    offset: 1.0,
                    color: Color::rgb(1.0, 1.0, 1.0),
                    opacity: 1.0,
                    extra: Default::default(),
                },
            ],
            start: None,
            end: None,
            center: Some([0.0, 0.0]),
            radius: Some(100.0),
            radius_y: Some(50.0),
            angle: None,
            spread: "pad".into(),
            extra: Default::default(),
        }
    }

    /// GD-FIL-03: elíptico pelos dois raios, retangular pela maior razão,
    /// cônico pelo ângulo.
    #[test]
    fn gd_fil_03_centered_kinds() {
        let e = g("elliptic");
        assert!((param(&e, Point::new(100.0, 0.0)) - 1.0).abs() < 1e-9);
        assert!((param(&e, Point::new(0.0, 25.0)) - 0.5).abs() < 1e-9);
        let r = g("rectangular");
        assert!((param(&r, Point::new(50.0, 40.0)) - 0.8).abs() < 1e-9);
        let c = g("conic");
        assert!((param(&c, Point::new(0.0, 10.0)) - 0.25).abs() < 1e-9);
        assert!((param(&c, Point::new(-10.0, -0.0001)) - 0.5).abs() < 1e-4);
        let mut rot = g("rectangular");
        rot.angle = Some(90.0);
        assert!(
            (param(&rot, Point::new(0.0, 100.0)) - 1.0).abs() < 1e-9,
            "eixos girados"
        );
        assert!(!is_native(&e) && is_native(&g("radial")) && is_native(&g("linear")));
        let mut rep = g("elliptic");
        rep.spread = "reflect".into();
        assert!((param(&rep, Point::new(150.0, 0.0)) - 0.5).abs() < 1e-9);
    }
}
