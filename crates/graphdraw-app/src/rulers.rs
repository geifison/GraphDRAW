//! Réguas do canvas (GD-REG-01): faixas no alto e à esquerda com a escala
//! na unidade de exibição do documento (GD-DOC-01), o passo ajustado ao zoom
//! (frações binárias nas unidades imperiais), a marca do cursor e a origem
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

/// Passo das marcas grandes (na unidade) para que fiquem a pelo menos
/// `ruler-major-spacing` px uma da outra, e quantas divisões menores tem.
/// Imperial: 1/32, 1/16 … 1 com divisões em metades (frações binárias).
pub fn step_for(px_per_unit: f64, imperial: bool) -> (f64, u32) {
    if !imperial {
        return step_mm(px_per_unit);
    }
    let min = theme::RULER_MAJOR_SPACING as f64;
    for s in [1.0 / 32.0, 1.0 / 16.0, 1.0 / 8.0, 1.0 / 4.0, 1.0 / 2.0, 1.0] {
        if s * px_per_unit >= min {
            // Divisões binárias até ficarem com ao menos 5 px.
            let mut sub = 1;
            while sub < 16 && s / (sub * 2) as f64 * px_per_unit >= 5.0 {
                sub *= 2;
            }
            return (s, sub);
        }
    }
    step_mm(px_per_unit)
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

/// Marcas de uma régua entre `lo` e `hi` (µm do papel, a partir da
/// origem): posição no papel (µm), medida do mundo (na unidade) e se é
/// grande. `step`/`sub` estão na unidade do mundo; `scale` = mundo ÷
/// papel; `unit_um` = µm por unidade.
pub fn ticks(
    lo: f64,
    hi: f64,
    step: f64,
    sub: u32,
    scale: f64,
    unit_um: f64,
) -> Vec<(f64, f64, bool)> {
    let minor = step / sub as f64;
    let i0 = (lo / unit_um * scale / minor).floor() as i64;
    let i1 = (hi / unit_um * scale / minor).ceil() as i64;
    (i0..=i1)
        .map(|i| {
            let v = i as f64 * minor;
            (v / scale * unit_um, v, i.rem_euclid(sub as i64) == 0)
        })
        .collect()
}

fn label(v: f64) -> String {
    let v = if v.abs() < 1e-9 { 0.0 } else { v };
    if (v - v.round()).abs() < 1e-6 {
        format!("{}", v.round() as i64)
    } else {
        let s = format!("{v:.2}");
        s.trim_end_matches('0')
            .trim_end_matches('.')
            .replace('.', ",")
    }
}

/// Número da régua imperial: inteiro ou fração mista até 1/32 ("1 1/2").
fn imperial_label(v: f64) -> String {
    let n = (v * 32.0).round() as i64;
    if n % 32 == 0 {
        return label(v);
    }
    let sign = if n < 0 { "-" } else { "" };
    let (whole, mut num, mut den) = (n.abs() / 32, n.abs() % 32, 32);
    while num % 2 == 0 {
        num /= 2;
        den /= 2;
    }
    if whole == 0 {
        format!("{sign}{num}/{den}")
    } else {
        format!("{sign}{whole} {num}/{den}")
    }
}

/// Desenha as duas réguas. `to_screen` leva um ponto da página (µm) à tela;
/// `visible` é o retângulo da página (µm) que está na tela; `origin` é o
/// zero das réguas (µm, na página); `cursor` marca a posição do mouse;
/// `disp` traz a unidade e a escala de desenho (mundo ÷ papel,
/// GD-DOC-03): as marcas e os números mostram medidas do mundo.
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
    disp: graphdraw_core::units::Display,
) {
    let scale = if disp.scale.is_finite() && disp.scale > 0.0 {
        disp.scale
    } else {
        1.0
    };
    let unit_um = disp.unit_um();
    let imperial = disp.unit.imperial();
    let text = |v: f64| {
        if imperial {
            imperial_label(v)
        } else {
            label(v)
        }
    };
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
    let (step, sub) = step_for(px_per_um * unit_um / scale, imperial);
    let r = theme::RULER_SIZE;
    // Horizontal (x).
    let p_top = painter.with_clip_rect(top);
    for (at, mm, major) in ticks(
        visible.x0 - origin.x,
        visible.x1 - origin.x,
        step,
        sub,
        scale,
        unit_um,
    ) {
        let x = to_screen(Point::new(origin.x + at, origin.y)).x;
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
                text(mm),
                font.clone(),
                pal.fg_muted,
            );
        }
    }
    // Vertical (y, cresce para cima como a página).
    let p_left = painter.with_clip_rect(left);
    for (at, mm, major) in ticks(
        visible.y0 - origin.y,
        visible.y1 - origin.y,
        step,
        sub,
        scale,
        unit_um,
    ) {
        let y = to_screen(Point::new(origin.x, origin.y + at)).y;
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
            let g = painter.layout_no_wrap(text(mm), font.clone(), pal.fg_muted);
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

    /// GD-REG-01: régua imperial em frações binárias; números em fração mista.
    #[test]
    fn imperial_ruler_steps_and_labels() {
        // 20 px por polegada: marcas a cada 1 pol... e bem perto, 1/8.
        let (s, _) = step_for(100.0, true);
        assert_eq!(s, 1.0);
        let (s, sub) = step_for(1000.0, true);
        assert_eq!(s, 1.0 / 16.0);
        assert!(sub.is_power_of_two());
        assert_eq!(imperial_label(1.5), "1 1/2");
        assert_eq!(imperial_label(0.125), "1/8");
        assert_eq!(imperial_label(3.0), "3");
        assert_eq!(imperial_label(-0.25), "-1/4");
        // Em polegadas, a marca de 1 pol cai a 25,4 mm do papel.
        let t = ticks(0.0, 30_000.0, 1.0, 8, 1.0, 25_400.0);
        assert!(
            t.iter()
                .any(|(at, v, m)| *m && (*v - 1.0).abs() < 1e-9 && (*at - 25_400.0).abs() < 1e-6)
        );
    }

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

    /// GD-DOC-03: na escala 1 : 100, 10 mm de papel aparecem como 1000 mm
    /// na régua; a posição no papel é a mesma.
    #[test]
    fn gd_doc_03_drawing_scale_on_rulers() {
        let t = ticks(0.0, 10_000.0, 1000.0, 10, 100.0, 1000.0);
        let last = t.iter().rev().find(|(_, _, major)| *major).unwrap();
        assert!(
            (last.0 - 10_000.0).abs() < 1e-6 && (last.1 - 1000.0).abs() < 1e-6,
            "{last:?}"
        );
        let plain = ticks(0.0, 10_000.0, 10.0, 10, 1.0, 1000.0);
        assert!(
            plain.iter().any(|(at, mm, m)| *m
                && (*at - 10_000.0).abs() < 1e-6
                && (*mm - 10.0).abs() < 1e-6)
        );
    }
}
