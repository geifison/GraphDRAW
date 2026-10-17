//! Abas de documento no topo da área de trabalho, no padrão de abas de
//! navegador (DESIGN-SYSTEM 4.5; GD-UI-01 "área de desenho com abas por
//! documento"). Desenho próprio: a aba ativa se funde com a linha de base
//! por cantos curvados para fora; as inativas ficam sem fundo, separadas
//! por divisores finos.

use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, Vec2};

use crate::i18n::tr;
use crate::icons;
use crate::theme;

/// Abaixo desta largura, o `×` só aparece na aba ativa ou sob o mouse.
const CLOSE_MIN_WIDTH: f32 = 120.0;
/// Pontos por quarto de círculo nos cantos da aba.
const ARC_STEPS: usize = 6;

/// O que a faixa mostra de cada aba.
pub struct TabInfo {
    pub title: String,
    /// Dica: caminho do arquivo (ou aviso de documento não salvo).
    pub tip: String,
}

/// Pedidos da faixa para a janela principal.
#[derive(Default, Debug, PartialEq)]
pub struct TabEvents {
    pub switch: Option<usize>,
    pub close: Option<usize>,
    pub new: bool,
}

/// Desenha a faixa de abas ocupando a largura disponível.
pub fn strip(ui: &mut egui::Ui, tabs: &[TabInfo], active: usize) -> TabEvents {
    let mut ev = TabEvents::default();
    let height = theme::TAB_DOC_HEIGHT;
    let pal = theme::palette(ui);
    let full = ui.available_rect_before_wrap();
    let full = Rect::from_min_size(full.min, Vec2::new(full.width(), height));
    // Faixa e linha de base.
    ui.painter()
        .rect_filled(full, theme::RADIUS_NONE, pal.bg_surface_0);
    ui.painter().hline(
        full.x_range(),
        full.bottom() - theme::BORDER_WIDTH / 2.0,
        theme::border_subtle(ui),
    );

    let lead = theme::SPACE_200;
    let plus_w = theme::BUTTON_COMPACT + 2.0 * theme::SPACE_200;
    let n = tabs.len().max(1) as f32;
    let width =
        ((full.width() - lead - plus_w) / n).clamp(theme::TAB_DOC_MIN_WIDTH, theme::TAB_DOC_WIDTH);
    let content = lead + width * tabs.len() as f32 + plus_w;

    egui::ScrollArea::horizontal()
        .id_salt("abas-documentos")
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
        .show(ui, |ui| {
            let (rect, _) = ui
                .allocate_exact_size(Vec2::new(content.max(full.width()), height), Sense::hover());
            let tab_h = height - theme::SPACE_100;
            let rects: Vec<Rect> = (0..tabs.len())
                .map(|i| {
                    Rect::from_min_size(
                        Pos2::new(rect.min.x + lead + width * i as f32, rect.bottom() - tab_h),
                        Vec2::new(width, tab_h),
                    )
                })
                .collect();
            let resps: Vec<egui::Response> = rects
                .iter()
                .enumerate()
                .map(|(i, r)| ui.interact(*r, ui.scope_id().with(("aba-doc", i)), Sense::click()))
                .collect();
            let hovered: Vec<bool> = resps.iter().map(|r| r.hovered()).collect();

            // Fundo: hover das inativas, depois a ativa por cima.
            for (i, r) in rects.iter().enumerate() {
                if i != active && hovered[i] {
                    paint_shape(ui, *r, pal.bg_surface_hover);
                }
            }
            // Divisores entre abas que não são a ativa nem estão sob o mouse.
            let quiet = |k: usize| k != active && !hovered[k];
            for (i, r) in rects.iter().enumerate().skip(1) {
                if quiet(i - 1) && quiet(i) {
                    let x = r.left();
                    let c = r.center().y;
                    ui.painter().vline(
                        x,
                        (c - tab_h / 4.0)..=(c + tab_h / 4.0),
                        Stroke::new(theme::BORDER_WIDTH, pal.border_default),
                    );
                }
            }
            if let Some(r) = rects.get(active) {
                paint_shape(ui, *r, pal.bg_surface_1);
            }

            for (i, (tab, r)) in tabs.iter().zip(&rects).enumerate() {
                let is_active = i == active;
                let show_close = is_active || hovered[i] || width >= CLOSE_MIN_WIDTH;
                let close = show_close.then(|| close_button(ui, *r, i));
                let text = if is_active {
                    pal.fg_strong
                } else if hovered[i] {
                    pal.fg_primary
                } else {
                    pal.fg_muted
                };
                paint_content(ui, *r, tab, text, close.as_ref().map(|c| c.rect));
                let resp = resps[i].clone().on_hover_text(&tab.tip);
                if close.as_ref().is_some_and(|c| c.clicked()) {
                    ev.close = Some(i);
                }
                if resp.clicked() && ev.close.is_none() {
                    ev.switch = Some(i);
                }
                if resp.middle_clicked() {
                    ev.close = Some(i);
                }
                resp.context_menu(|ui| {
                    if ui.button(tr("Fechar", "Close")).clicked() {
                        ev.close = Some(i);
                        ui.close();
                    }
                });
            }

            // "+" logo depois da última aba.
            let after = rects.last().map_or(rect.min.x + lead, |r| r.right());
            let plus = Rect::from_center_size(
                Pos2::new(after + plus_w / 2.0, rect.bottom() - tab_h / 2.0),
                Vec2::splat(theme::BUTTON_COMPACT),
            );
            if plus_button(ui, plus)
                .on_hover_text(crate::keymap::hint(
                    tr("Novo documento", "New document"),
                    crate::keymap::Command::New,
                ))
                .clicked()
            {
                ev.new = true;
            }
        });
    ev
}

/// Contorno da aba: cantos de cima arredondados para dentro e cantos de
/// baixo curvados para fora, até a linha de base.
fn outline(r: Rect) -> Vec<Pos2> {
    let rad = theme::RADIUS_MD as f32;
    let arc = |c: Pos2, from: f32, to: f32| {
        (0..=ARC_STEPS).map(move |k| {
            let a = from + (to - from) * k as f32 / ARC_STEPS as f32;
            c + Vec2::angled(a.to_radians()) * rad
        })
    };
    let (l, t, rr, b) = (r.left(), r.top(), r.right(), r.bottom());
    let mut pts = Vec::new();
    // Curva de baixo à esquerda (para fora), da base subindo até a lateral.
    pts.extend(arc(Pos2::new(l - rad, b - rad), 90.0, 0.0));
    pts.extend(arc(Pos2::new(l + rad, t + rad), 180.0, 270.0));
    pts.extend(arc(Pos2::new(rr - rad, t + rad), 270.0, 360.0));
    pts.extend(arc(Pos2::new(rr + rad, b - rad), 180.0, 90.0));
    pts
}

/// Preenche a forma da aba. O contorno é visível inteiro a partir do meio
/// da base, então um leque de triângulos a partir dali cobre a forma; um
/// traço fino por cima suaviza as bordas.
fn paint_shape(ui: &egui::Ui, r: Rect, color: Color32) {
    let pts = outline(r);
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(Pos2::new(r.center().x, r.bottom()), color);
    for p in &pts {
        mesh.colored_vertex(*p, color);
    }
    for k in 1..pts.len() as u32 {
        mesh.add_triangle(0, k, k + 1);
    }
    ui.painter().add(mesh);
    ui.painter().add(egui::Shape::line(
        pts,
        Stroke::new(theme::BORDER_WIDTH, color),
    ));
}

/// Ícone de documento, título cortado com reticências e espaço para o `×`.
fn paint_content(ui: &egui::Ui, r: Rect, tab: &TabInfo, text: Color32, close: Option<Rect>) {
    let pad = theme::SPACE_300;
    let icon = Rect::from_center_size(
        Pos2::new(r.left() + pad + theme::ICON_MICRO / 2.0, r.center().y),
        Vec2::splat(theme::ICON_MICRO),
    );
    egui::Image::new(icons::get_micro("doc"))
        .tint(text)
        .paint_at(ui, icon);
    let x0 = icon.right() + theme::SPACE_150;
    let x1 = close.map_or(r.right() - pad, |c| c.left() - theme::SPACE_100);
    if x1 <= x0 {
        return;
    }
    let mut job =
        egui::text::LayoutJob::simple_singleline(tab.title.clone(), theme::font_body(), text);
    job.wrap = egui::text::TextWrapping::truncate_at_width(x1 - x0);
    let galley = ui.fonts_mut(|f| f.layout_job(job));
    let y = r.center().y - galley.size().y / 2.0;
    ui.painter().galley(Pos2::new(x0, y), galley, text);
}

/// `×` de 16 px à direita da aba; círculo de fundo no hover.
fn close_button(ui: &egui::Ui, r: Rect, i: usize) -> egui::Response {
    let rect = Rect::from_center_size(
        Pos2::new(
            r.right() - theme::SPACE_200 - theme::ICON_MICRO / 2.0,
            r.center().y,
        ),
        Vec2::splat(theme::ICON_MICRO),
    );
    let resp = ui
        .interact(rect, ui.scope_id().with(("fechar-doc", i)), Sense::click())
        .on_hover_text(crate::keymap::hint(
            tr("Fechar", "Close"),
            crate::keymap::Command::Close,
        ));
    let pal = theme::palette(ui);
    if resp.hovered() {
        ui.painter()
            .circle_filled(rect.center(), rect.width() / 2.0, pal.bg_surface_active);
    }
    let color = if resp.hovered() {
        pal.fg_strong
    } else {
        pal.fg_muted
    };
    let d = theme::ICON_MICRO / 4.0;
    let st = Stroke::new(theme::FOCUS_WIDTH, color);
    let c = rect.center();
    ui.painter()
        .line_segment([c + Vec2::new(-d, -d), c + Vec2::new(d, d)], st);
    ui.painter()
        .line_segment([c + Vec2::new(-d, d), c + Vec2::new(d, -d)], st);
    resp
}

/// Botão "+" circular ghost de 24 px.
fn plus_button(ui: &egui::Ui, rect: Rect) -> egui::Response {
    let resp = ui.interact(rect, ui.scope_id().with("nova-aba-doc"), Sense::click());
    let pal = theme::palette(ui);
    if resp.hovered() {
        ui.painter()
            .circle_filled(rect.center(), rect.width() / 2.0, pal.bg_surface_hover);
    }
    let color = if resp.hovered() {
        pal.fg_strong
    } else {
        pal.fg_muted
    };
    let d = theme::ICON_MICRO / 3.0;
    let st = Stroke::new(theme::FOCUS_WIDTH, color);
    let c = rect.center();
    ui.painter()
        .line_segment([c - Vec2::new(d, 0.0), c + Vec2::new(d, 0.0)], st);
    ui.painter()
        .line_segment([c - Vec2::new(0.0, d), c + Vec2::new(0.0, d)], st);
    resp
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tabs(n: usize) -> Vec<TabInfo> {
        (0..n)
            .map(|i| TabInfo {
                title: format!("Documento {i}"),
                tip: String::new(),
            })
            .collect()
    }

    /// Roda a faixa e devolve a altura usada e os eventos do último quadro.
    fn run(n: usize, events: Vec<egui::Event>) -> (f32, TabEvents) {
        let ctx = theme::test_ctx();
        let t = tabs(n);
        let mut h = 0.0;
        let mut ev = TabEvents::default();
        for k in 0..3 {
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1366.0, 768.0))),
                events: if k == 2 { events.clone() } else { Vec::new() },
                ..Default::default()
            };
            let mut out = ctx.run_ui(input, |ui| {
                egui::Panel::top("documentos")
                    .frame(egui::Frame::NONE)
                    .show(ui, |ui| {
                        ev = strip(ui, &t, 0);
                        h = ui.min_rect().height();
                    });
            });
            out.textures_delta.clear();
        }
        (h, ev)
    }

    /// DESIGN-SYSTEM 4.5: faixa de 32 px, com poucas ou muitas abas.
    #[test]
    fn strip_has_fixed_height() {
        assert_eq!(run(3, vec![]).0, theme::TAB_DOC_HEIGHT);
        assert_eq!(run(40, vec![]).0, theme::TAB_DOC_HEIGHT);
    }

    /// Clicar na segunda aba pede para ativá-la.
    #[test]
    fn click_switches_tab() {
        // Segunda aba: começa depois do recuo + uma aba de largura máxima.
        let x = theme::SPACE_200 + theme::TAB_DOC_WIDTH + theme::TAB_DOC_WIDTH / 3.0;
        let pos = Pos2::new(x, theme::TAB_DOC_HEIGHT / 2.0 + theme::SPACE_100);
        let press = |pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        let ctx = theme::test_ctx();
        let t = tabs(3);
        let mut ev = TabEvents::default();
        let frames = [
            vec![egui::Event::PointerMoved(pos)],
            vec![press(true)],
            vec![press(false)],
        ];
        for events in frames {
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1366.0, 768.0))),
                events,
                ..Default::default()
            };
            let mut out = ctx.run_ui(input, |ui| {
                egui::Panel::top("documentos")
                    .frame(egui::Frame::NONE)
                    .show(ui, |ui| ev = strip(ui, &t, 0));
            });
            out.textures_delta.clear();
        }
        assert_eq!(ev.switch, Some(1));
    }
}
