//! Campos reutilizáveis: comprimento com expressões e unidades
//! (GD-UI-11/GD-TRF-04), número, seletor de cor que preserva o modelo
//! (GD-COR-02, GD-COR-03) e amostras de cor.

use eframe::egui::{self, Color32, Response, Sense, Vec2};
use graphdraw_core::model::Color;
use graphdraw_core::units::{self, Unit};

use crate::i18n::tr;
use crate::theme;

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

/// Largura dos campos de comprimento e de número.
const LENGTH_WIDTH: f32 = 64.0;
const NUMBER_WIDTH: f32 = 48.0;
/// Passo do arrasto no rótulo, por pixel: 0,1 mm e 0,5 (graus, %).
const LENGTH_DRAG_STEP: f64 = 100.0;
const NUMBER_DRAG_STEP: f64 = 0.5;

/// Há um rótulo de campo numérico sendo arrastado? A janela principal usa
/// isso para juntar o arrasto inteiro num passo só de desfazer.
pub fn field_dragging(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp::<bool>(drag_flag()).unwrap_or(false))
}

fn drag_flag() -> egui::Id {
    egui::Id::unique("arrasto-de-campo")
}

/// Rótulo arrastável de campo numérico (DESIGN-SYSTEM 4.2): esquerda e
/// direita mudam o valor; Shift = passo 10×, Alt = passo 0,1×. Devolve o
/// quanto somar ao valor. No quadro em que o arrasto começa não muda nada,
/// para a janela principal abrir antes a transação de desfazer.
fn drag_handle(ui: &mut egui::Ui, text: &str, step: f64) -> Option<f64> {
    let resp = ui
        .add(egui::Label::new(text).sense(Sense::drag()))
        .on_hover_cursor(egui::CursorIcon::ResizeHorizontal);
    if resp.drag_started() {
        ui.data_mut(|d| d.insert_temp(drag_flag(), true));
        return None;
    }
    if resp.drag_stopped() {
        ui.data_mut(|d| d.insert_temp(drag_flag(), false));
    }
    if !resp.dragged() {
        return None;
    }
    let dx = resp.drag_delta().x as f64;
    let m = ui.input(|i| i.modifiers);
    let k = if m.shift {
        10.0
    } else if m.alt {
        0.1
    } else {
        1.0
    };
    (dx != 0.0).then_some(dx * step * k)
}

/// Campo de texto que aceita expressões; devolve o valor ao confirmar
/// (Enter ou perder o foco) se mudou. Valor em `type-numeric`.
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
            .font(egui::TextStyle::Monospace)
            .align(egui::Align2::LEFT_CENTER)
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
    let r = expr_field(ui, id, fmt_mm(value_um), LENGTH_WIDTH, |t| {
        units::parse_length(t, Unit::Millimeter, reference_um.or(Some(value_um)))
    });
    // A unidade também é alça de arrasto.
    let d = drag_handle(ui, "mm", LENGTH_DRAG_STEP);
    r.or(d.map(|d| value_um + d))
}

/// Comprimento com rótulo arrastável à esquerda (W, H, X, Y…).
pub fn labeled_length(
    ui: &mut egui::Ui,
    label: &str,
    id: &str,
    value_um: f64,
    reference_um: Option<f64>,
) -> Option<f64> {
    let d = drag_handle(ui, label, LENGTH_DRAG_STEP);
    let r = length_field(ui, id, value_um, reference_um);
    r.or(d.map(|d| value_um + d))
}

pub fn number_field(ui: &mut egui::Ui, id: &str, value: f64, suffix: &str) -> Option<f64> {
    let id = ui.scope_id().with(id);
    let r = expr_field(ui, id, fmt_num(value, 2), NUMBER_WIDTH, units::parse_number);
    if suffix.is_empty() {
        return r;
    }
    let d = drag_handle(ui, suffix, NUMBER_DRAG_STEP);
    r.or(d.map(|d| value + d))
}

/// Número com rótulo arrastável à esquerda (ângulo, escala…).
pub fn labeled_number(
    ui: &mut egui::Ui,
    label: &str,
    id: &str,
    value: f64,
    suffix: &str,
) -> Option<f64> {
    let d = drag_handle(ui, label, NUMBER_DRAG_STEP);
    let r = number_field(ui, id, value, suffix);
    r.or(d.map(|d| value + d))
}

/// Botão primário: a ação principal de um grupo (DESIGN-SYSTEM 4.1).
pub fn primary_button(text: &str) -> egui::Button<'static> {
    egui::Button::new(egui::RichText::new(text.to_string()).color(theme::FG_ON_ACCENT))
        .fill(theme::ACCENT_PRIMARY)
        .stroke(egui::Stroke::NONE)
        .min_size(Vec2::new(theme::BUTTON_MIN_WIDTH, theme::BUTTON_DEFAULT))
}

/// Botão secundário de rodapé de janela.
pub fn secondary_button(text: &str) -> egui::Button<'static> {
    egui::Button::new(text.to_string())
        .min_size(Vec2::new(theme::BUTTON_MIN_WIDTH, theme::BUTTON_DEFAULT))
}

/// Rodapé de janela: botões alinhados à direita, numa linha de altura fixa
/// (dentro de um modal, um layout sem limite ocuparia a tela toda). Os
/// botões entram da direita para a esquerda: a ação primária primeiro.
pub fn footer<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let size = Vec2::new(ui.available_width(), theme::BUTTON_DEFAULT);
    ui.allocate_ui_with_layout(size, egui::Layout::right_to_left(egui::Align::Center), add)
        .inner
}

/// Painel da barra de propriedades: altura fixa de `bar-property-height`,
/// qualquer que seja o contexto (DESIGN-SYSTEM 4.6).
pub fn property_panel(ctx: &egui::Context) -> egui::Panel {
    egui::Panel::top("propriedades")
        .resizable(false)
        .exact_size(theme::BAR_PROPERTY_HEIGHT)
        .frame(
            egui::Frame::side_top_panel(&ctx.global_style()).inner_margin(egui::Margin::symmetric(
                theme::SPACE_200 as i8,
                theme::SPACE_150 as i8,
            )),
        )
}

/// Conteúdo de barra em uma linha só (DESIGN-SYSTEM 4.6): nada quebra
/// linha; o que não cabe rola na horizontal (roda do mouse ou arrasto, sem
/// barra de rolagem) e um degradê na borda avisa que há mais.
pub fn single_line_bar(ui: &mut egui::Ui, id: &str, add: impl FnOnce(&mut egui::Ui)) {
    let out = egui::ScrollArea::horizontal()
        .id_salt(id)
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
        .show(ui, |ui| {
            let dy = ui.input(|i| i.smooth_scroll_delta.y);
            if dy != 0.0 && ui.rect_contains_pointer(ui.clip_rect()) {
                ui.scroll_with_delta(Vec2::new(dy, 0.0));
            }
            ui.horizontal_centered(add);
        });
    let view = out.inner_rect;
    let x = out.state.clamped_offset().x;
    let fade = theme::SPACE_600;
    if x > 0.5 {
        fade_edge(
            ui,
            egui::Rect::from_min_size(view.min, Vec2::new(fade, view.height())),
            true,
        );
    }
    if out.content_size.x - x > view.width() + 0.5 {
        let min = egui::pos2(view.max.x - fade, view.min.y);
        fade_edge(
            ui,
            egui::Rect::from_min_size(min, Vec2::new(fade, view.height())),
            false,
        );
    }
}

/// Degradê do fundo do painel até transparente, na borda esquerda ou direita.
fn fade_edge(ui: &egui::Ui, r: egui::Rect, left: bool) {
    let c = ui.visuals().panel_fill;
    let t = Color32::TRANSPARENT;
    let (a, b) = if left { (c, t) } else { (t, c) };
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(r.left_top(), a);
    mesh.colored_vertex(r.right_top(), b);
    mesh.colored_vertex(r.right_bottom(), b);
    mesh.colored_vertex(r.left_bottom(), a);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    ui.painter().add(mesh);
}

/// Texto de ajuda numa linha, cortado com reticências; o texto completo
/// aparece na dica.
pub fn hint_line(ui: &mut egui::Ui, text: &str) {
    ui.horizontal_centered(|ui| {
        ui.add(egui::Label::new(text).truncate())
            .on_hover_text(text);
    });
}

/// Janela modal com o quadro de janela do tema: raio `radius-lg` e margens
/// `space-600` (o padrão do egui usaria o quadro de menu).
pub fn modal(ctx: &egui::Context, id: egui::Id) -> egui::Modal {
    egui::Modal::new(id).frame(egui::Frame::window(&ctx.global_style()))
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
            p.rect_filled(rect, theme::RADIUS_XS, to_color32(c));
        }
        None => no_color(p, rect),
    }
    let stroke = if resp.hovered() {
        ui.visuals().widgets.hovered.fg_stroke
    } else {
        theme::border_default(ui)
    };
    p.rect_stroke(rect, theme::RADIUS_XS, stroke, egui::StrokeKind::Inside);
    resp
}

/// Marca de "sem cor": papel branco cortado por uma diagonal.
pub fn no_color(p: &egui::Painter, rect: egui::Rect) {
    p.rect_filled(rect, theme::RADIUS_XS, theme::PAGE_PAPER);
    p.line_segment(
        [rect.left_bottom(), rect.right_top()],
        egui::Stroke::new(theme::FOCUS_WIDTH, theme::ACCENT_DANGER),
    );
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Altura da barra de propriedades numa tela de 1366 × 768, com o
    /// conteúdo dado.
    fn bar_height(content: impl Fn(&mut egui::Ui)) -> f32 {
        let ctx = crate::theme::test_ctx();
        let mut h = 0.0;
        for _ in 0..3 {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    Vec2::new(1366.0, 768.0),
                )),
                ..Default::default()
            };
            let mut out = ctx.run_ui(input, |ui| {
                let ctx = ui.ctx().clone();
                h = property_panel(&ctx)
                    .show(ui, |ui| content(ui))
                    .response
                    .rect
                    .height();
            });
            out.textures_delta.clear();
        }
        h
    }

    /// DESIGN-SYSTEM 4.6: a barra tem sempre 36 px, com pouco ou muito
    /// conteúdo, dica longa ou campos que não cabem.
    #[test]
    fn property_bar_keeps_its_height() {
        let empty = bar_height(|ui| {
            ui.label("Página");
        });
        let hint = bar_height(|ui| hint_line(ui, &"dica muito longa · ".repeat(80)));
        let crowded = bar_height(|ui| {
            single_line_bar(ui, "teste", |ui| {
                for i in 0..60 {
                    let _ = labeled_length(ui, "W", &format!("c{i}"), 148_000.0, None);
                }
            })
        });
        assert_eq!(empty, theme::BAR_PROPERTY_HEIGHT);
        assert_eq!(hint, theme::BAR_PROPERTY_HEIGHT);
        assert_eq!(crowded, theme::BAR_PROPERTY_HEIGHT);
    }
}
