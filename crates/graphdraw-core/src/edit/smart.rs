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
                });
            }
        }
    }
    lines.dedup();
    (Vec2::new(dx, dy), lines)
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
}
