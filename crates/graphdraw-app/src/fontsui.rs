//! Fontes ausentes ao abrir (GD-FNT-02/03): para cada família que não está
//! instalada nem incorporada no arquivo, trocar por outra (com a sugestão
//! da mais parecida), converter os textos em curvas ou manter (mostrada
//! com a substituta, o nome original fica no documento).

use eframe::egui;
use graphdraw_core::edit::fonts::{self, MissingFont};

use crate::doc::DocState;
use crate::i18n::tr;
use crate::theme;
use crate::widgets;

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Replace(String),
    Curves,
    Keep,
}

pub struct MissingFontsDialog {
    pub(crate) rows: Vec<(MissingFont, Action)>,
    families: Vec<String>,
    pub(crate) rect: egui::Rect,
}

impl MissingFontsDialog {
    /// `None` se não falta nenhuma fonte.
    pub fn for_doc(doc: &DocState) -> Option<MissingFontsDialog> {
        let m = fonts::missing_fonts(doc.doc());
        if m.is_empty() {
            return None;
        }
        let families = graphdraw_core::render::text::font_families();
        let rows = m
            .into_iter()
            .map(|f| {
                let a = f.suggestion.clone().map_or(Action::Keep, Action::Replace);
                (f, a)
            })
            .collect();
        Some(MissingFontsDialog {
            rows,
            families,
            rect: egui::Rect::NOTHING,
        })
    }

    /// Aplica as escolhas num passo do histórico.
    pub fn apply(&self, doc: &mut DocState) {
        if self.rows.iter().all(|(_, a)| *a == Action::Keep) {
            return;
        }
        doc.editor
            .begin(tr("Resolver fontes ausentes", "Resolve missing fonts"));
        for (f, a) in &self.rows {
            match a {
                Action::Replace(to) => {
                    fonts::replace_font(&mut doc.editor, &f.texts, &f.family, to)
                }
                Action::Curves => {
                    graphdraw_core::edit::ops::convert_to_curves(&mut doc.editor, &f.texts)
                }
                Action::Keep => {}
            }
        }
        doc.editor.commit();
    }

    /// `true` quando fechou (aplicando ou mantendo).
    pub fn show(&mut self, ctx: &egui::Context, doc: &mut DocState) -> bool {
        let mut done = None;
        let resp = widgets::modal(ctx, egui::Id::unique("fontes-ausentes")).show(ctx, |ui| {
            ui.set_width(theme::MISSING_FONTS_WIDTH);
            ui.heading(tr("Fontes ausentes", "Missing fonts"));
            ui.add_space(theme::SPACE_200);
            ui.label(tr(
                "Este arquivo usa fontes que não estão instaladas nem vieram dentro dele. Escolha o que fazer com cada uma:",
                "This file uses fonts that are not installed and did not come inside it. Choose what to do with each one:",
            ));
            ui.add_space(theme::SPACE_300);
            egui::ScrollArea::vertical().max_height(theme::MISSING_FONTS_LIST).show(ui, |ui| {
                egui::Grid::new("fontes-ausentes-grade")
                    .num_columns(3)
                    .striped(true)
                    .spacing([theme::SPACE_400, theme::SPACE_300])
                    .show(ui, |ui| {
                        ui.strong(tr("Fonte", "Font"));
                        ui.strong(tr("Textos", "Texts"));
                        ui.strong(tr("O que fazer", "What to do"));
                        ui.end_row();
                        for (i, (f, a)) in self.rows.iter_mut().enumerate() {
                            ui.label(&f.family);
                            ui.label(f.texts.len().to_string());
                            ui.horizontal(|ui| {
                                let mut replace = matches!(a, Action::Replace(_));
                                if ui.radio(replace, tr("Trocar por", "Replace with")).clicked() && !replace {
                                    replace = true;
                                    *a = Action::Replace(
                                        f.suggestion.clone().or_else(|| self.families.first().cloned()).unwrap_or_default(),
                                    );
                                }
                                ui.add_enabled_ui(replace, |ui| {
                                    let cur = match a {
                                        Action::Replace(s) => s.clone(),
                                        _ => f.suggestion.clone().unwrap_or_default(),
                                    };
                                    egui::ComboBox::from_id_salt(("fonte-troca", i))
                                        .selected_text(&cur)
                                        .width(theme::FONT_COMBO_WIDTH)
                                        .height(theme::MISSING_FONTS_LIST)
                                        .show_ui(ui, |ui| {
                                            for fam in &self.families {
                                                let label = if Some(fam) == f.suggestion.as_ref() {
                                                    format!("{fam}  ★")
                                                } else {
                                                    fam.clone()
                                                };
                                                if ui.selectable_label(*fam == cur, label).clicked() {
                                                    *a = Action::Replace(fam.clone());
                                                }
                                            }
                                        });
                                });
                                if ui
                                    .radio(*a == Action::Curves, tr("Converter em curvas", "Convert to curves"))
                                    .on_hover_text(tr(
                                        "Os textos viram desenho com a fonte substituta e deixam de ser editáveis",
                                        "The texts become drawings with the substitute font and are no longer editable",
                                    ))
                                    .clicked()
                                {
                                    *a = Action::Curves;
                                }
                                if ui
                                    .radio(*a == Action::Keep, tr("Manter", "Keep"))
                                    .on_hover_text(tr(
                                        "Mostra com a substituta; o nome original fica no documento e volta quando a fonte for instalada",
                                        "Shows with the substitute; the original name stays in the document and returns when the font is installed",
                                    ))
                                    .clicked()
                                {
                                    *a = Action::Keep;
                                }
                            });
                            ui.end_row();
                        }
                    });
            });
            ui.add_space(theme::SPACE_300);
            ui.weak(tr(
                "★ = sugestão (a instalada mais parecida). Para mandar o arquivo a outra máquina com as fontes, ligue Arquivo → Incorporar fontes ao salvar.",
                "★ = suggestion (the most similar installed font). To send the file to another machine with its fonts, turn on File → Embed fonts when saving.",
            ));
            ui.add_space(theme::SPACE_300);
            widgets::footer(ui, |ui| {
                if ui.add(widgets::primary_button(tr("Aplicar", "Apply"))).clicked() {
                    done = Some(true);
                }
                if ui.add(widgets::secondary_button(tr("Manter todas", "Keep all"))).clicked()
                    || ui.input(|i| i.key_pressed(egui::Key::Escape))
                {
                    done = Some(false);
                }
            });
        });
        self.rect = resp.response.rect;
        match done {
            Some(true) => {
                self.apply(doc);
                true
            }
            Some(false) => true,
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dialog_fits_and_applies() {
        use graphdraw_core::model::{FontSpec, Object, ObjectKind, Paragraph, TextData, TextSpan};
        let mut doc = DocState::new_blank();
        let t = Object::new(ObjectKind::Text(TextData {
            text_kind: "artistic".into(),
            frame: None,
            paragraphs: vec![Paragraph {
                align: "left".into(),
                spans: vec![TextSpan {
                    text: "Oi".into(),
                    font: FontSpec {
                        family: "Fonte Que Nao Existe GD".into(),
                        size: 12_000.0,
                        weight: 400,
                        italic: false,
                        extra: Default::default(),
                    },
                    fill: None,
                    extra: Default::default(),
                }],
                extra: Default::default(),
            }],
            line_spacing: 1.2,
            path: None,
            exclusions: Default::default(),
            extra: Default::default(),
        }));
        graphdraw_core::edit::ops::add_object(&mut doc.editor, 0, 0, t, "t");
        if graphdraw_core::render::text::font_families().is_empty() {
            return;
        }
        let mut d = MissingFontsDialog::for_doc(&doc).expect("falta a fonte");
        let ctx = crate::theme::test_ctx();
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1366.0, 768.0));
        for k in 0..20 {
            let input = egui::RawInput {
                screen_rect: Some(screen),
                time: Some(k as f64 / 30.0),
                ..Default::default()
            };
            let mut out = ctx.run_ui(input, |ui| {
                d.show(ui.ctx(), &mut doc);
            });
            out.textures_delta.clear();
        }
        assert!(screen.contains_rect(d.rect), "{:?}", d.rect);
        d.rows[0].1 = Action::Curves;
        d.apply(&mut doc);
        assert!(MissingFontsDialog::for_doc(&doc).is_none(), "virou curvas");
        doc.editor.undo();
        assert!(MissingFontsDialog::for_doc(&doc).is_some(), "um passo só");
    }
}
