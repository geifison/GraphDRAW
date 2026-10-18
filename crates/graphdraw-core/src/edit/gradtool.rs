//! Edição interativa de gradiente na tela (GD-FIL-11, GD-FER-50): alças de
//! início/fim (linear) ou centro/raio (radial), paradas sobre a linha,
//! arrastar para mover e duplo clique para criar parada. Tudo em
//! coordenadas locais do objeto, como o próprio gradiente.

use kurbo::Point;

use crate::model::{Color, Fill, Gradient, GradientStop};

/// Alça do gradiente.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Handle {
    /// Início (linear) ou centro (radial).
    Start,
    /// Fim (linear) ou ponto do raio (radial).
    End,
    /// Parada de cor, pelo índice em `stops`.
    Stop(usize),
}

/// Eixo do gradiente: de onde a onde vai o parâmetro 0..1.
pub fn axis(g: &Gradient) -> Option<(Point, Point)> {
    if g.kind != "linear" {
        let c = g.center?;
        let r = g.radius?;
        Some((Point::new(c[0], c[1]), Point::new(c[0] + r, c[1])))
    } else {
        let (a, b) = (g.start?, g.end?);
        Some((Point::new(a[0], a[1]), Point::new(b[0], b[1])))
    }
}

/// Alças na ordem de prioridade do clique: início e fim primeiro, depois
/// as paradas (as das pontas ficam sob o início/fim).
pub fn handles(g: &Gradient) -> Vec<(Handle, Point)> {
    let Some((a, b)) = axis(g) else {
        return Vec::new();
    };
    let mut v = vec![(Handle::Start, a), (Handle::End, b)];
    for (i, s) in g.stops.iter().enumerate() {
        v.push((Handle::Stop(i), a.lerp(b, s.offset)));
    }
    v
}

/// Parâmetro 0..1 da projeção de `p` no eixo.
pub fn project(g: &Gradient, p: Point) -> Option<f64> {
    let (a, b) = axis(g)?;
    let d = b - a;
    let len2 = d.hypot2();
    if len2 <= 0.0 {
        return None;
    }
    Some(((p - a).dot(d) / len2).clamp(0.0, 1.0))
}

/// Distância de `p` ao segmento do eixo.
pub fn distance_to_axis(g: &Gradient, p: Point) -> Option<f64> {
    let (a, b) = axis(g)?;
    let t = project(g, p)?;
    Some((p - a.lerp(b, t)).hypot())
}

pub use crate::render::gradient::color_at;

/// Move a alça para `p`. Paradas deslizam sobre o eixo; a ordem das
/// paradas não muda durante o arrasto (a renderização ordena).
pub fn drag(g: &Gradient, h: Handle, p: Point) -> Gradient {
    let mut g = g.clone();
    let radial = g.kind != "linear";
    match h {
        Handle::Start if radial => g.center = Some([p.x, p.y]),
        Handle::Start => g.start = Some([p.x, p.y]),
        Handle::End if radial => {
            if let Some(c) = g.center {
                g.radius = Some((p - Point::new(c[0], c[1])).hypot().max(1.0));
            }
        }
        Handle::End => g.end = Some([p.x, p.y]),
        Handle::Stop(i) => {
            if let (Some(t), true) = (project(&g, p), i < g.stops.len()) {
                g.stops[i].offset = t;
            }
        }
    }
    g
}

/// Nova parada no ponto do eixo mais perto de `p`, com a cor que o
/// gradiente já tinha ali. Devolve o índice dela.
pub fn add_stop(g: &Gradient, p: Point) -> Option<(Gradient, usize)> {
    let t = project(g, p)?;
    let (color, opacity) = color_at(g, t);
    let mut g = g.clone();
    let i = g.stops.iter().filter(|s| s.offset <= t).count();
    g.stops.insert(
        i,
        GradientStop {
            offset: t,
            color,
            opacity,
            midpoint: 0.5,
            extra: Default::default(),
        },
    );
    Some((g, i))
}

/// Tira a parada `i` (sempre ficam ao menos duas).
pub fn remove_stop(g: &Gradient, i: usize) -> Option<Gradient> {
    if g.stops.len() <= 2 || i >= g.stops.len() {
        return None;
    }
    let mut g = g.clone();
    g.stops.remove(i);
    Some(g)
}

/// Põe as paradas em ordem; devolve o novo índice de `keep`.
pub fn sort_stops(g: &mut Gradient, keep: Option<usize>) -> Option<usize> {
    let mut idx: Vec<usize> = (0..g.stops.len()).collect();
    idx.sort_by(|a, b| g.stops[*a].offset.total_cmp(&g.stops[*b].offset));
    let stops = idx.iter().map(|i| g.stops[*i].clone()).collect();
    g.stops = stops;
    keep.and_then(|k| idx.iter().position(|i| *i == k))
}

/// Gradiente linear novo de `a` a `b`. As paradas vêm do preenchimento
/// atual: as de um gradiente, ou da cor sólida para o branco, ou preto
/// para branco.
pub fn linear_from(a: Point, b: Point, current: Option<&Fill>) -> Gradient {
    let stop = |offset: f64, color: Color| GradientStop {
        offset,
        color,
        opacity: 1.0,
        midpoint: 0.5,
        extra: Default::default(),
    };
    let stops = match current {
        Some(Fill::Gradient(g)) if g.stops.len() >= 2 => g.stops.clone(),
        Some(Fill::Solid { color, .. }) => vec![
            stop(0.0, color.clone()),
            stop(1.0, Color::rgb(1.0, 1.0, 1.0)),
        ],
        _ => vec![
            stop(0.0, Color::black()),
            stop(1.0, Color::rgb(1.0, 1.0, 1.0)),
        ],
    };
    let spread = match current {
        Some(Fill::Gradient(g)) => g.spread.clone(),
        _ => "pad".into(),
    };
    Gradient {
        kind: "linear".into(),
        stops,
        start: Some([a.x, a.y]),
        end: Some([b.x, b.y]),
        center: None,
        radius: None,
        radius_y: None,
        angle: None,
        spread,
        steps: None,
        smooth: false,
        color_path: "direct".into(),
        extra: Default::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bw() -> Gradient {
        linear_from(Point::new(0.0, 0.0), Point::new(100.0, 0.0), None)
    }

    /// GD-FIL-11: arrastar início/fim e paradas; duplo clique cria parada
    /// com a cor que já estava ali.
    #[test]
    fn gd_fil_11_drag_and_add_stops() {
        let g = bw();
        let h = handles(&g);
        assert_eq!(h[0], (Handle::Start, Point::new(0.0, 0.0)));
        assert_eq!(h[1], (Handle::End, Point::new(100.0, 0.0)));
        let g = drag(&g, Handle::End, Point::new(200.0, 0.0));
        assert_eq!(g.end, Some([200.0, 0.0]));
        // Parada nova no meio: cinza médio.
        let (g, i) = add_stop(&g, Point::new(100.0, 30.0)).unwrap();
        assert_eq!((i, g.stops.len()), (1, 3));
        assert!((g.stops[1].offset - 0.5).abs() < 1e-9);
        let c = g.stops[1].color.to_srgb();
        assert!((c[0] - 0.5).abs() < 1e-6, "{c:?}");
        // A parada desliza só até a ponta; ordenar devolve o novo índice.
        let slid = drag(&g, Handle::Stop(1), Point::new(500.0, 0.0));
        assert_eq!(slid.stops[1].offset, 1.0);
        let mut g2 = drag(&g, Handle::Stop(0), Point::new(150.0, 0.0));
        assert_eq!(
            sort_stops(&mut g2, Some(0)),
            Some(1),
            "a primeira passou da do meio"
        );
        assert_eq!(g2.stops[1].offset, 0.75);
        // Remover: nunca abaixo de duas paradas.
        let g = remove_stop(&g, 1).unwrap();
        assert!(remove_stop(&g, 0).is_none());
    }

    #[test]
    fn radial_handles_and_drag() {
        let mut g = bw();
        g.kind = "radial".into();
        g.center = Some([10.0, 10.0]);
        g.radius = Some(50.0);
        assert_eq!(
            axis(&g),
            Some((Point::new(10.0, 10.0), Point::new(60.0, 10.0)))
        );
        let g = drag(&g, Handle::End, Point::new(10.0, 90.0));
        assert_eq!(g.radius, Some(80.0));
        let g = drag(&g, Handle::Start, Point::new(0.0, 0.0));
        assert_eq!(g.center, Some([0.0, 0.0]));
    }

    #[test]
    fn new_gradient_takes_solid_color() {
        let red = Color::from_hex("#ff0000").unwrap();
        let g = linear_from(
            Point::ORIGIN,
            Point::new(1.0, 0.0),
            Some(&Fill::solid(red.clone())),
        );
        assert_eq!(g.stops[0].color, red);
        assert_eq!(g.stops[1].color, Color::rgb(1.0, 1.0, 1.0));
    }
}
