//! Guias inteligentes (GD-ALI-05): ao arrastar, a caixa da seleção encaixa
//! nas bordas e centros dos outros objetos e da página, e as linhas de
//! alinhamento aparecem de borda a borda.

use kurbo::{Affine, Rect, Vec2};

use super::query::bounds;
use crate::model::Page;

/// Linha de alinhamento a desenhar. `vertical` = linha em x = `at` de
/// `from` a `to` em y (horizontal ao contrário).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SmartLine {
    pub vertical: bool,
    pub at: f64,
    pub from: f64,
    pub to: f64,
    /// Alinhamento com a página (senão, com outro objeto).
    pub page: bool,
    /// Marcador de espaçamento igual (o vão entre dois objetos).
    pub spacing: bool,
}

/// Caixas com que a seleção pode alinhar.
#[derive(Debug, Clone, Default)]
pub struct Targets {
    boxes: Vec<(Rect, bool)>,
}

/// Alvos da página: objetos visíveis fora de `exclude` e a própria página.
pub fn targets(page: &Page, exclude: &[String]) -> Targets {
    let mut boxes = vec![(Rect::new(0.0, 0.0, page.width, page.height), true)];
    for l in page.layers.iter().filter(|l| l.visible) {
        for o in l
            .objects
            .iter()
            .filter(|o| o.visible && !exclude.contains(&o.id))
        {
            if let Some(b) = bounds(o, Affine::IDENTITY, false) {
                boxes.push((b, false));
            }
        }
    }
    Targets { boxes }
}

fn keys_x(r: &Rect) -> [f64; 3] {
    [r.x0, r.center().x, r.x1]
}
fn keys_y(r: &Rect) -> [f64; 3] {
    [r.y0, r.center().y, r.y1]
}

/// Encaixe da caixa `moving` dentro de `radius`: devolve o ajuste a somar
/// ao deslocamento e as linhas de alinhamento já com o ajuste aplicado.
pub fn align(moving: Rect, t: &Targets, radius: f64) -> (Vec2, Vec<SmartLine>) {
    let best = |keys: fn(&Rect) -> [f64; 3]| -> Option<f64> {
        let mk = keys(&moving);
        let mut best: Option<f64> = None;
        for (b, _) in &t.boxes {
            for tk in keys(b) {
                for m in mk {
                    let d = tk - m;
                    if d.abs() <= radius && best.is_none_or(|v| d.abs() < v.abs()) {
                        best = Some(d);
                    }
                }
            }
        }
        best
    };
    let dx = best(keys_x).unwrap_or(0.0);
    let dy = best(keys_y).unwrap_or(0.0);
    let moved = moving + Vec2::new(dx, dy);
    let mut lines = Vec::new();
    let eps = radius * 1e-3 + 1e-6;
    for (b, page) in &t.boxes {
        for tk in keys_x(b) {
            if keys_x(&moved).iter().any(|m| (m - tk).abs() <= eps)
                && (dx != 0.0 || best(keys_x).is_some())
            {
                lines.push(SmartLine {
                    vertical: true,
                    at: tk,
                    from: moved.y0.min(b.y0),
                    to: moved.y1.max(b.y1),
                    page: *page,
                    spacing: false,
                });
            }
        }
        for tk in keys_y(b) {
            if keys_y(&moved).iter().any(|m| (m - tk).abs() <= eps)
                && (dy != 0.0 || best(keys_y).is_some())
            {
                lines.push(SmartLine {
                    vertical: false,
                    at: tk,
                    from: moved.x0.min(b.x0),
                    to: moved.x1.max(b.x1),
                    page: *page,
                    spacing: false,
                });
            }
        }
    }
    // Espaçamento igual entre os vizinhos (quando a borda não encaixou).
    let (mut dx, mut dy) = (dx, dy);
    let mut moved = moved;
    if dx == 0.0
        && let Some((x, marks)) = equal_gap(moved, &t.boxes, radius, true)
    {
        dx = x - moved.x0;
        moved = moved + Vec2::new(dx, 0.0);
        lines.extend(marks(moved));
    }
    if dy == 0.0
        && let Some((y, marks)) = equal_gap(moved, &t.boxes, radius, false)
    {
        dy = y - moved.y0;
        moved = moved + Vec2::new(0.0, dy);
        lines.extend(marks(moved));
    }
    let _ = moved;
    lines.dedup();
    (Vec2::new(dx, dy), lines)
}

type Marks = Box<dyn Fn(Rect) -> Vec<SmartLine>>;

/// Posição (x0 ou y0) que deixa vãos iguais até o vizinho anterior e o
/// seguinte no eixo, se estiver dentro de `radius`, e os marcadores.
fn equal_gap(
    m: Rect,
    boxes: &[(Rect, bool)],
    radius: f64,
    horizontal: bool,
) -> Option<(f64, Marks)> {
    // Vizinhos que se sobrepõem no outro eixo (fora a página).
    let overlap = |b: &Rect| {
        if horizontal {
            b.y0 < m.y1 && b.y1 > m.y0
        } else {
            b.x0 < m.x1 && b.x1 > m.x0
        }
    };
    let (lo, hi, size) = if horizontal {
        (m.x0, m.x1, m.width())
    } else {
        (m.y0, m.y1, m.height())
    };
    let end = |b: &Rect| if horizontal { b.x1 } else { b.y1 };
    let start = |b: &Rect| if horizontal { b.x0 } else { b.y0 };
    let prev = boxes
        .iter()
        .filter(|(b, page)| !page && overlap(b) && end(b) <= lo + radius)
        .map(|(b, _)| *b)
        .max_by(|a, b| end(a).total_cmp(&end(b)))?;
    let next = boxes
        .iter()
        .filter(|(b, page)| !page && overlap(b) && start(b) >= hi - radius)
        .map(|(b, _)| *b)
        .min_by(|a, b| start(a).total_cmp(&start(b)))?;
    let target = (end(&prev) + start(&next) - size) / 2.0;
    if (target - lo).abs() > radius || target < end(&prev) {
        return None;
    }
    let marks: Marks = Box::new(move |mv: Rect| {
        let (c, a0, a1, b0, b1) = if horizontal {
            (mv.center().y, prev.x1, mv.x0, mv.x1, next.x0)
        } else {
            (mv.center().x, prev.y1, mv.y0, mv.y1, next.y0)
        };
        [(a0, a1), (b0, b1)]
            .into_iter()
            .map(|(f, t)| SmartLine {
                vertical: !horizontal,
                at: c,
                from: f,
                to: t,
                page: false,
                spacing: true,
            })
            .collect()
    });
    Some((target, marks))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Object, ObjectKind, RectData, Transform};
    use serde_json::Map;

    fn rect(x: f64, y: f64, w: f64, h: f64) -> Object {
        let mut o = Object::new(ObjectKind::Rectangle(RectData {
            width: w,
            height: h,
            corners: None,
            extra: Map::new(),
        }));
        o.transform = Transform::translate(x, y);
        o
    }

    #[test]
    fn gd_ali_05_snaps_edges_and_centers_with_lines() {
        let mut p = Page::new(200.0, 100.0);
        let a = rect(20.0, 20.0, 30.0, 30.0);
        let moving = rect(0.0, 0.0, 10.0, 10.0);
        let mid = moving.id.clone();
        p.layers[0].objects.extend([a, moving]);
        let t = targets(&p, &[mid]);
        // Caixa quase com a borda esquerda em x = 20 (objeto) e o topo em y = 50.
        let (adj, lines) = align(Rect::new(21.0, 39.5, 31.0, 49.5), &t, 2.0);
        assert!(
            (adj.x + 1.0).abs() < 1e-9 && (adj.y - 0.5).abs() < 1e-9,
            "{adj:?}"
        );
        assert!(lines.iter().any(|l| l.vertical && l.at == 20.0 && !l.page));
        assert!(lines.iter().any(|l| !l.vertical && l.at == 50.0));
        // Centro da página (x = 100).
        let (adj, lines) = align(Rect::new(94.0, 70.0, 104.0, 80.0), &t, 2.0);
        assert!((adj.x - 1.0).abs() < 1e-9);
        assert!(lines.iter().any(|l| l.vertical && l.at == 100.0 && l.page));
        // Longe de tudo: nada muda, sem linhas.
        let (adj, lines) = align(Rect::new(140.0, 65.0, 145.0, 68.0), &t, 1.0);
        assert_eq!(adj, Vec2::ZERO);
        assert!(lines.is_empty());
    }

    /// Espaçamento igual: entre dois objetos, o vão da esquerda iguala o da
    /// direita, com dois marcadores.
    #[test]
    fn gd_ali_05_equal_spacing() {
        let mut p = Page::new(300.0, 100.0);
        p.layers[0]
            .objects
            .extend([rect(10.0, 40.0, 20.0, 20.0), rect(110.0, 40.0, 20.0, 20.0)]);
        let t = targets(&p, &[]);
        // Caixa de 20 entre x = 30 e x = 110: centrada em x0 = 60; está em 61.
        let (adj, lines) = align(Rect::new(61.0, 42.0, 81.0, 58.0), &t, 2.5);
        assert!((adj.x + 1.0).abs() < 1e-9, "{adj:?}");
        let marks: Vec<_> = lines.iter().filter(|l| l.spacing).collect();
        assert_eq!(marks.len(), 2);
        assert!((marks[0].to - marks[0].from - 30.0).abs() < 1e-9);
        assert!((marks[1].to - marks[1].from - 30.0).abs() < 1e-9);
    }
}
