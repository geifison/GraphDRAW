//! Seletor de fonte (GD-FNT-01): cada família aparece desenhada na própria
//! fonte, e o campo de busca filtra a lista enquanto se digita. Setas
//! escolhem, Enter confirma, Esc fecha.

use std::collections::HashMap;

use eframe::egui::{self, Vec2};

use crate::i18n::tr;
use crate::theme;

#[derive(Default)]
pub struct FontPicker {
    query: String,
    /// Prévia por família (`None` = não deu para desenhar; mostra o nome).
    previews: HashMap<String, Option<egui::TextureHandle>>,
    /// Linha destacada na lista filtrada.
    highlight: usize,
    /// O popup acabou de abrir: foca a busca.
    just_opened: bool,
}

/// Prévias novas por quadro (o resto vem nos quadros seguintes).
const PREVIEWS_PER_FRAME: usize = 12;

/// Comparação sem diferença de maiúsculas nem acentos comuns.
fn fold(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' | 'Á' | 'À' | 'Â' | 'Ã' | 'Ä' => 'a',
            'é' | 'ê' | 'è' | 'É' | 'Ê' | 'È' => 'e',
            'í' | 'Í' => 'i',
            'ó' | 'ô' | 'õ' | 'ö' | 'Ó' | 'Ô' | 'Õ' | 'Ö' => 'o',
            'ú' | 'ü' | 'Ú' | 'Ü' => 'u',
            'ç' | 'Ç' => 'c',
            c => c.to_ascii_lowercase(),
        })
        .collect()
}

/// Famílias que contêm a busca; as que começam por ela vêm primeiro.
pub fn filter<'a>(all: &'a [String], query: &str) -> Vec<&'a String> {
    let q = fold(query.trim());
    if q.is_empty() {
        return all.iter().collect();
    }
    let mut starts: Vec<&String> = Vec::new();
    let mut contains: Vec<&String> = Vec::new();
    for f in all {
        let n = fold(f);
        if n.starts_with(&q) {
            starts.push(f);
        } else if n.contains(&q) {
            contains.push(f);
        }
    }
    starts.extend(contains);
    starts
}

impl FontPicker {
    /// Botão com a família atual; ao clicar abre a lista. Devolve a família
    /// escolhida.
    pub fn show(&mut self, ui: &mut egui::Ui, current: &str) -> Option<String> {
        let mut picked = None;
        let button = ui
            .add_sized(
                Vec2::new(theme::FONT_COMBO_WIDTH, ui.spacing().interact_size.y),
                egui::Button::new(current).truncate(),
            )
            .on_hover_text(tr(
                "Fonte (clique e digite para buscar)",
                "Font (click and type to search)",
            ));
        if button.clicked() {
            self.just_opened = true;
            self.query.clear();
            self.highlight = 0;
        }
        egui::Popup::from_toggle_button_response(&button)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .width(theme::FONT_POPUP_WIDTH)
            .show(|ui| {
                let search = ui.add(
                    egui::TextEdit::singleline(&mut self.query)
                        .hint_text(tr("Buscar fonte…", "Search font…"))
                        .desired_width(f32::INFINITY),
                );
                if self.just_opened {
                    search.request_focus();
                    self.just_opened = false;
                }
                if search.changed() {
                    self.highlight = 0;
                }
                let all = crate::textedit::families();
                let list = filter(all, &self.query);
                let (down, up, enter, esc) = ui.input(|i| {
                    (
                        i.key_pressed(egui::Key::ArrowDown),
                        i.key_pressed(egui::Key::ArrowUp),
                        i.key_pressed(egui::Key::Enter),
                        i.key_pressed(egui::Key::Escape),
                    )
                });
                if down {
                    self.highlight = (self.highlight + 1).min(list.len().saturating_sub(1));
                }
                if up {
                    self.highlight = self.highlight.saturating_sub(1);
                }
                if enter && let Some(f) = list.get(self.highlight) {
                    picked = Some((*f).clone());
                    ui.close();
                }
                if esc {
                    ui.close();
                }
                if list.is_empty() {
                    ui.label(
                        egui::RichText::new(tr("Nenhuma fonte encontrada", "No font found")).weak(),
                    );
                    return;
                }
                let row_h = theme::FONT_PREVIEW_HEIGHT + 2.0 * theme::SPACE_100;
                let mut rendered = 0;
                egui::ScrollArea::vertical()
                    .max_height(theme::FONT_LIST_HEIGHT)
                    .auto_shrink([false, true])
                    .show_rows(ui, row_h, list.len(), |ui, range| {
                        for i in range {
                            let f = list[i];
                            let tex = self.preview(ui.ctx(), f, &mut rendered);
                            let selected = f == current || i == self.highlight;
                            let r = row(ui, f, tex, selected, row_h);
                            if (down || up) && i == self.highlight {
                                r.scroll_to_me(None);
                            }
                            if r.clicked() {
                                picked = Some(f.clone());
                                ui.close();
                            }
                        }
                    });
                if rendered >= PREVIEWS_PER_FRAME {
                    ui.ctx().request_repaint();
                }
            });
        picked
    }

    /// Prévia da família (desenhada sob demanda, com limite por quadro).
    fn preview(
        &mut self,
        ctx: &egui::Context,
        family: &str,
        rendered: &mut usize,
    ) -> Option<egui::TextureHandle> {
        if let Some(t) = self.previews.get(family) {
            return t.clone();
        }
        if *rendered >= PREVIEWS_PER_FRAME {
            return None;
        }
        *rendered += 1;
        let ppp = ctx.pixels_per_point();
        let h = (theme::FONT_PREVIEW_HEIGHT * ppp).round() as u32;
        let max_w = (theme::FONT_PREVIEW_MAX_WIDTH * ppp).round() as u32;
        let tex = graphdraw_core::render::raster::font_preview(family, family, h, max_w).map(
            |(w, h, alpha)| {
                let rgba: Vec<u8> = alpha.iter().flat_map(|a| [255, 255, 255, *a]).collect();
                let img = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba);
                ctx.load_texture(format!("fonte-{family}"), img, egui::TextureOptions::LINEAR)
            },
        );
        self.previews.insert(family.to_string(), tex.clone());
        tex
    }
}

/// Linha da lista: o nome desenhado na própria fonte (ou em texto comum
/// enquanto a prévia não fica pronta).
fn row(
    ui: &mut egui::Ui,
    family: &str,
    tex: Option<egui::TextureHandle>,
    selected: bool,
    h: f32,
) -> egui::Response {
    let (rect, r) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), h), egui::Sense::click());
    let pal = theme::palette(ui);
    if selected || r.hovered() {
        let fill = if selected {
            pal.bg_surface_active
        } else {
            pal.bg_surface_hover
        };
        ui.painter().rect_filled(rect, theme::RADIUS_SM, fill);
    }
    let x = rect.left() + theme::SPACE_200;
    match tex {
        Some(t) => {
            let ppp = ui.ctx().pixels_per_point();
            let size = t.size_vec2() / ppp;
            let img_rect =
                egui::Rect::from_min_size(egui::pos2(x, rect.center().y - size.y / 2.0), size);
            egui::Image::new((t.id(), size))
                .tint(pal.fg_primary)
                .paint_at(ui, img_rect);
        }
        None => {
            ui.painter().text(
                egui::pos2(x, rect.center().y),
                egui::Align2::LEFT_CENTER,
                family,
                theme::font_body(),
                pal.fg_muted,
            );
        }
    }
    r.on_hover_text(family)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_filters_as_you_type_prefix_first() {
        let all: Vec<String> = [
            "Arial",
            "Arial Black",
            "Calibri",
            "Segoe UI",
            "Times New Roman",
            "Bahnschrift",
        ]
        .map(String::from)
        .to_vec();
        assert_eq!(filter(&all, "").len(), 6);
        assert_eq!(filter(&all, "ar"), vec!["Arial", "Arial Black"]);
        // Contém no meio: depois dos que começam ("ca" começa Calibri).
        assert_eq!(filter(&all, "ca"), vec!["Calibri"]);
        assert_eq!(filter(&all, "bri"), vec!["Calibri"]);
        assert_eq!(
            filter(&all, "r")[..2],
            ["Arial", "Arial Black"]
                .map(String::from)
                .iter()
                .collect::<Vec<_>>()[..]
        );
        // Sem acento e sem caixa.
        assert_eq!(filter(&all, "SÉGOE"), vec!["Segoe UI"]);
        assert_eq!(filter(&all, "TIMES"), vec!["Times New Roman"]);
        assert!(filter(&all, "zzz").is_empty());
    }

    #[test]
    fn preview_draws_installed_family() {
        if graphdraw_core::render::text::resolve_family("Arial", 400, false)
            .is_none_or(|(_, sub)| sub)
        {
            return;
        }
        let (w, h, alpha) =
            graphdraw_core::render::raster::font_preview("Arial", "Arial", 24, 300).unwrap();
        assert_eq!(h, 24);
        assert!(w > 20 && w <= 300);
        assert!(alpha.iter().any(|a| *a > 200), "tem tinta");
        assert!(
            graphdraw_core::render::raster::font_preview("Fonte Que Não Existe", "x", 24, 300)
                .is_none()
        );
    }
}
