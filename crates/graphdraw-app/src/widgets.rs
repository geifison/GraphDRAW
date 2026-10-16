//! Campos reutilizáveis: comprimento com expressões e unidades
//! (GD-UI-11/GD-TRF-04), número, seletor de cor que preserva o modelo
//! (GD-COR-02, GD-COR-03) e amostras de cor.

use eframe::egui::{self, Color32, Response, Sense, Vec2};
use graphdraw_core::model::Color;
use graphdraw_core::units::{self, Unit};

use crate::i18n::tr;

/// Formata µm em mm com vírgula decimal (pt-BR).
pub fn fmt_mm(um: f64) -> String {
    let s = format!("{:.3}", um / 1000.0);
    let s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    let s = if s == "-0" { "0".into() } else { s };
    if crate::i18n::english() {
        s
    } else {
        s.replace('.', ",")
    }
}

pub fn fmt_num(v: f64, decimals: usize) -> String {
    let s = format!("{v:.decimals$}");
    let s = if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s
    };
    if crate::i18n::english() {
        s
    } else {
        s.replace('.', ",")
    }
}

/// Campo de texto que aceita expressões; devolve o valor ao confirmar
/// (Enter ou perder o foco) se mudou.
fn expr_field(
    ui: &mut egui::Ui,
    id: egui::Id,
    shown: String,
    width: f32,
    parse: impl Fn(&str) -> Option<f64>,
) -> Option<f64> {
    let mut text: String = ui
        .data(|d| d.get_temp::<String>(id))
        .unwrap_or_else(|| shown.clone());
    let resp = ui.add(
        egui::TextEdit::singleline(&mut text)
            .id(id.with("te"))
            .desired_width(width),
    );
    let mut out = None;
    if resp.has_focus() {
        ui.data_mut(|d| d.insert_temp(id, text.clone()));
    } else if resp.lost_focus() {
        ui.data_mut(|d| d.remove::<String>(id));
        if text != shown {
            match parse(&text) {
                Some(v) => out = Some(v),
                None => {
                    ui.ctx().debug_text(tr("Valor inválido", "Invalid value"));
                }
            }
        }
    } else {
        ui.data_mut(|d| d.remove::<String>(id));
    }
    out
}

/// Comprimento em mm (aceita `10+5`, `2in`, `50%` de `reference_um`).
pub fn length_field(
    ui: &mut egui::Ui,
    id: &str,
    value_um: f64,
    reference_um: Option<f64>,
) -> Option<f64> {
    let id = ui.scope_id().with(id);
    let r = expr_field(ui, id, fmt_mm(value_um), 64.0, |t| {
        units::parse_length(t, Unit::Millimeter, reference_um.or(Some(value_um)))
    });
    ui.label("mm");
    r
}

pub fn number_field(ui: &mut egui::Ui, id: &str, value: f64, suffix: &str) -> Option<f64> {
    let id = ui.scope_id().with(id);
    let r = expr_field(ui, id, fmt_num(value, 2), 48.0, units::parse_number);
    if !suffix.is_empty() {
        ui.label(suffix);
    }
    r
}

pub fn to_color32(c: &Color) -> Color32 {
    let [r, g, b] = c.to_srgb();
    Color32::from_rgb(
        (r * 255.0).round() as u8,
        (g * 255.0).round() as u8,
        (b * 255.0).round() as u8,
    )
}

/// Amostra clicável; `None` desenha "sem cor" (diagonal).
pub fn swatch(ui: &mut egui::Ui, color: Option<&Color>, size: f32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    let p = ui.painter();
    match color {
        Some(c) => {
            p.rect_filled(rect, 2.0, to_color32(c));
        }
        None => {
            p.rect_filled(rect, 2.0, Color32::WHITE);
            p.line_segment(
                [rect.left_bottom(), rect.right_top()],
                egui::Stroke::new(1.5, Color32::from_rgb(0xe5, 0x39, 0x35)),
            );
        }
    }
    let stroke = if resp.hovered() {
        ui.visuals().widgets.hovered.fg_stroke
    } else {
        egui::Stroke::new(1.0, Color32::from_gray(120))
    };
    p.rect_stroke(rect, 2.0, stroke, egui::StrokeKind::Inside);
    resp
}

/// Editor de cor que mantém o modelo escolhido (RGB, CMYK, cinza).
/// Devolve a nova cor quando alterada.
pub fn color_editor(ui: &mut egui::Ui, color: &Color) -> Option<Color> {
    let mut c = color.clone();
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label(tr("Modelo:", "Model:"));
        for (m, label) in [
            ("rgb", "RGB"),
            ("cmyk", "CMYK"),
            ("gray", tr("Cinza", "Gray")),
        ] {
            if ui.selectable_label(c.model == m, label).clicked() && c.model != m {
                c = convert_model(&c, m);
                changed = true;
            }
        }
    });
    let comps: &[(&str, f64)] = match c.model.as_str() {
        "rgb" => &[("R", 255.0), ("G", 255.0), ("B", 255.0)],
        "cmyk" => &[("C", 100.0), ("M", 100.0), ("Y", 100.0), ("K", 100.0)],
        "gray" => &[("K", 100.0)],
        _ => &[],
    };
    if comps.is_empty() {
        ui.label(format!(
            "{} '{}' — {}",
            tr("Modelo", "Model"),
            c.model,
            tr(
                "edição não disponível nesta versão",
                "editing not available yet"
            )
        ));
    }
    for (i, (name, scale)) in comps.iter().enumerate() {
        let mut v = c.values.get(i).copied().unwrap_or(0.0);
        // Cinza: armazenado como luminosidade (0 = preto); exibido como % de preto.
        if c.model == "gray" {
            v = 1.0 - v;
        }
        let mut shown = (v * scale).round();
        if ui
            .add(egui::Slider::new(&mut shown, 0.0..=*scale).text(*name))
            .changed()
        {
            let nv = shown / scale;
            c.values.resize(comps.len(), 0.0);
            c.values[i] = if c.model == "gray" { 1.0 - nv } else { nv };
            changed = true;
        }
    }
    if c.model == "rgb" {
        let mut rgb = to_color32(&c);
        if egui::color_picker::color_picker_color32(ui, &mut rgb, egui::color_picker::Alpha::Opaque)
        {
            c = Color::rgb(
                rgb.r() as f64 / 255.0,
                rgb.g() as f64 / 255.0,
                rgb.b() as f64 / 255.0,
            );
            changed = true;
        }
    }
    changed.then_some(c)
}

/// Converte só quando o usuário troca o modelo explicitamente.
fn convert_model(c: &Color, model: &str) -> Color {
    let [r, g, b] = c.to_srgb();
    match model {
        "rgb" => Color::rgb(r, g, b),
        "gray" => Color::gray(0.299 * r + 0.587 * g + 0.114 * b),
        _ => {
            let k = 1.0 - r.max(g).max(b);
            if k >= 1.0 - 1e-9 {
                Color::cmyk(0.0, 0.0, 0.0, 1.0)
            } else {
                Color::cmyk(
                    (1.0 - r - k) / (1.0 - k),
                    (1.0 - g - k) / (1.0 - k),
                    (1.0 - b - k) / (1.0 - k),
                    k,
                )
            }
        }
    }
}

/// Paleta padrão (cores próprias, em CMYK para produção).
pub fn palette() -> Vec<Color> {
    let cmyk = |c: f64, m: f64, y: f64, k: f64| Color::cmyk(c, m, y, k);
    vec![
        cmyk(0.0, 0.0, 0.0, 1.0),
        cmyk(0.0, 0.0, 0.0, 0.8),
        cmyk(0.0, 0.0, 0.0, 0.6),
        cmyk(0.0, 0.0, 0.0, 0.4),
        cmyk(0.0, 0.0, 0.0, 0.2),
        cmyk(0.0, 0.0, 0.0, 0.0),
        cmyk(0.0, 1.0, 1.0, 0.0),
        cmyk(0.0, 0.6, 1.0, 0.0),
        cmyk(0.0, 0.0, 1.0, 0.0),
        cmyk(0.5, 0.0, 1.0, 0.0),
        cmyk(1.0, 0.0, 1.0, 0.0),
        cmyk(1.0, 0.0, 0.3, 0.0),
        cmyk(1.0, 0.0, 0.0, 0.0),
        cmyk(1.0, 0.5, 0.0, 0.0),
        cmyk(1.0, 1.0, 0.0, 0.0),
        cmyk(0.5, 1.0, 0.0, 0.0),
        cmyk(0.0, 1.0, 0.0, 0.0),
        cmyk(0.0, 0.5, 0.0, 0.0),
        cmyk(0.0, 0.3, 0.6, 0.3),
        cmyk(0.3, 0.6, 1.0, 0.4),
        cmyk(1.0, 0.7, 0.0, 0.5),
        cmyk(0.6, 0.0, 0.9, 0.4),
    ]
}
