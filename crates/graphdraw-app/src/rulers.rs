//! Réguas do canvas (GD-REG-01): faixas no alto e à esquerda com a escala
//! em milímetros, o passo ajustado ao zoom, a marca do cursor e a origem
//! (o zero) que pode ser arrastada.

use eframe::egui::{self, Pos2, Rect, Vec2};
use kurbo::Point;

use crate::theme;

/// Áreas das réguas dentro do canvas: (cima, esquerda, canto).
pub fn areas(canvas: Rect) -> (Rect, Rect, Rect) {
    let r = theme::RULER_SIZE;
    let top = Rect::from_min_max(
        egui::pos2(canvas.min.x + r, canvas.min.y),
        egui::pos2(canvas.max.x, canvas.min.y + r),
    );
    let left = Rect::from_min_max(
        egui::pos2(canvas.min.x, canvas.min.y + r),
        egui::pos2(canvas.min.x + r, canvas.max.y),
    );
    let corner = Rect::from_min_size(canvas.min, Vec2::splat(r));
    (top, left, corner)
}

/// Passo das marcas grandes (mm) para que fiquem a pelo menos
/// `ruler-major-spacing` px uma da outra, e quantas divisões menores ele tem.
pub fn step_mm(px_per_mm: f64) -> (f64, u32) {
    let min = theme::RULER_MAJOR_SPACING as f64;
    let mut base = 0.001;
    loop {
        for (m, sub) in [(1.0, 10), (2.0, 2), (5.0, 5)] {
            let s = base * m;
            if s * px_per_mm >= min {
                return (s, sub);
            }
        }
        base *= 10.0;
        if base > 1e7 {
            return (1e7, 10);
        }
    }
}

fn label(v_mm: f64) -> String {
    let v = if v_mm.abs() < 1e-9 { 0.0 } else { v_mm };
    if (v - v.round()).abs() < 1e-6 {
        format!("{}", v.round() as i64)
    } else {
        let s = format!("{v:.2}");
        s.trim_end_matches('0')
            .trim_end_matches('.')
            .replace('.', ",")
    }
}

/// Desenha as duas réguas. `to_screen` leva um ponto da página (µm) à tela;
/// `visible` é o retângulo da página (µm) que está na tela; `origin` é o
/// zero das réguas (µm, na página); `cursor` marca a posição do mouse.
#[allow(clippy::too_many_arguments)]
pub fn paint(
    ui: &egui::Ui,
    painter: &egui::Painter,
    canvas: Rect,
    to_screen: &dyn Fn(Point) -> Pos2,
    visible: kurbo::Rect,
    px_per_um: f64,
    origin: Point,
    cursor: Option<Point>,
) {
    let (top, left, corner) = areas(canvas);
    let pal = theme::palette(ui);
    for r in [top, left, corner] {
        painter.rect_filled(r, theme::RADIUS_NONE, pal.bg_surface_1);
    }
    let line = egui::Stroke::new(theme::BORDER_WIDTH, pal.border_default);
    painter.line_segment([top.left_bottom(), top.right_bottom()], line);
    painter.line_segment([left.right_top(), left.right_bottom()], line);
    painter.line_segment([corner.right_top(), corner.right_bottom()], line);
    painter.line_segment([corner.left_bottom(), corner.right_bottom()], line);
    let tick = egui::Stroke::new(theme::BORDER_WIDTH, pal.fg_muted);
    let font = theme::font_micro();
    let (step, sub) = step_mm(px_per_um * 1000.0);
    let minor = step / sub as f64;
    let r = theme::RULER_SIZE;
    // Horizontal (x).
    let p_top = painter.with_clip_rect(top);
    let x0 = ((visible.x0 - origin.x) / 1000.0 / minor).floor() as i64;
    let x1 = ((visible.x1 - origin.x) / 1000.0 / minor).ceil() as i64;
    for i in x0..=x1 {
        let mm = i as f64 * minor;
        let x = to_screen(Point::new(origin.x + mm * 1000.0, origin.y)).x;
        let major = i.rem_euclid(sub as i64) == 0;
        let len = if major {
            theme::RULER_TICK_MAJOR
        } else {
            theme::RULER_TICK_MINOR
        };
        p_top.line_segment(
            [egui::pos2(x, top.max.y - len), egui::pos2(x, top.max.y)],
            tick,
        );
        if major {
            p_top.text(
                egui::pos2(x + 2.0, top.min.y + 1.0),
                egui::Align2::LEFT_TOP,
                label(mm),
                font.clone(),
                pal.fg_muted,
            );
        }
    }
    // Vertical (y, cresce para cima como a página).
    let p_left = painter.with_clip_rect(left);
    let y0 = ((visible.y0 - origin.y) / 1000.0 / minor).floor() as i64;
    let y1 = ((visible.y1 - origin.y) / 1000.0 / minor).ceil() as i64;
    for i in y0..=y1 {
        let mm = i as f64 * minor;
        let y = to_screen(Point::new(origin.x, origin.y + mm * 1000.0)).y;
        let major = i.rem_euclid(sub as i64) == 0;
        let len = if major {
            theme::RULER_TICK_MAJOR
        } else {
            theme::RULER_TICK_MINOR
        };
        p_left.line_segment(
            [egui::pos2(left.max.x - len, y), egui::pos2(left.max.x, y)],
            tick,
        );
        if major {
            // Texto de pé, lido de baixo para cima.
            let g = painter.layout_no_wrap(label(mm), font.clone(), pal.fg_muted);
            let pos = egui::pos2(left.min.x + 1.0, y - 2.0);
            p_left.add(
                egui::epaint::TextShape::new(pos, g, pal.fg_muted)
                    .with_angle(-std::f32::consts::FRAC_PI_2),
            );
        }
    }
    // Cursor.
    if let Some(c) = cursor {
        let s = to_screen(c);
        let mark = egui::Stroke::new(theme::BORDER_WIDTH, theme::ACCENT_PRIMARY);
        if s.x > top.min.x {
            p_top.line_segment(
                [egui::pos2(s.x, top.min.y), egui::pos2(s.x, top.max.y)],
                mark,
            );
        }
        if s.y > left.min.y {
            p_left.line_segment(
                [egui::pos2(left.min.x, s.y), egui::pos2(left.max.x, s.y)],
                mark,
            );
        }
    }
    // Canto: cruzinha da origem.
    let c = corner.center();
    let k = r * 0.25;
    painter.line_segment([egui::pos2(c.x - k, c.y), egui::pos2(c.x + k, c.y)], tick);
    painter.line_segment([egui::pos2(c.x, c.y - k), egui::pos2(c.x, c.y + k)], tick);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_follows_zoom() {
        // 1 px/mm: marcas a cada 100 mm (60 px no mínimo).
        assert_eq!(step_mm(1.0), (100.0, 10));
        // 10 px/mm: a cada 10 mm.
        assert_eq!(step_mm(10.0), (10.0, 10));
        // 25 px/mm: a cada 5 mm.
        assert_eq!(step_mm(25.0).0, 5.0);
        assert_eq!(label(12.5), "12,5");
        assert_eq!(label(-0.0), "0");
    }
}
