//! Verificar ortografia (GD-TXT-08): percorre os textos (os selecionados ou
//! todos da página), mostra cada palavra que o corretor do sistema não
//! conhece com o trecho em volta e as sugestões; Substituir, Substituir
//! todas, Ignorar, Ignorar todas, Adicionar ao dicionário.

use std::collections::HashSet;

use eframe::egui;
use graphdraw_core::edit::{ops, text as tedit};
use graphdraw_core::model::ObjectKind;

use crate::doc::DocState;
use crate::i18n::tr;
use crate::spell;
use crate::theme;
use crate::widgets;

/// Palavra com erro encontrada.
#[derive(Debug, Clone)]
pub struct Found {
    pub id: String,
    pub start: usize,
    pub word: String,
    pub lang: String,
    pub context: (String, String),
    pub suggestions: Vec<String>,
}

pub struct SpellDialog {
    targets: Vec<String>,
    /// Onde continuar: objeto e caractere.
    at: (usize, usize),
    pub(crate) current: Option<Found>,
    ignored: HashSet<String>,
    replacement: String,
    pub(crate) finished: bool,
    pub(crate) rect: egui::Rect,
}

fn text_of(doc: &DocState, id: &str) -> Option<(String, String)> {
    let o = doc.editor.get(&doc.editor.find(id)?)?;
    let ObjectKind::Text(t) = &o.kind else {
        return None;
    };
    let lang = t
        .paragraphs
        .first()
        .and_then(|p| p.lang.clone())
        .unwrap_or_else(|| "pt-BR".into());
    Some((graphdraw_core::render::text::plain_text(t), lang))
}

impl SpellDialog {
    /// Textos da seleção (ou da página toda, sem seleção).
    pub fn new(doc: &DocState) -> SpellDialog {
        let mut targets = Vec::new();
        let sel: HashSet<&String> = doc.selection.iter().collect();
        doc.doc().data.pages[doc.page].layers.iter().for_each(|l| {
            for o in &l.objects {
                o.walk(&mut |c| {
                    if matches!(c.kind, ObjectKind::Text(_))
                        && (sel.is_empty() || sel.contains(&c.id) || sel.contains(&o.id))
                    {
                        targets.push(c.id.clone());
                    }
                });
            }
        });
        let mut d = SpellDialog {
            targets,
            at: (0, 0),
            current: None,
            ignored: HashSet::new(),
            replacement: String::new(),
            finished: false,
            rect: egui::Rect::NOTHING,
        };
        d.advance(doc);
        d
    }

    /// Próxima palavra com erro a partir de `at`.
    fn advance(&mut self, doc: &DocState) {
        while self.at.0 < self.targets.len() {
            let id = self.targets[self.at.0].clone();
            if let Some((text, lang)) = text_of(doc, &id) {
                let chars: Vec<char> = text.chars().collect();
                for m in spell::check(&text, &lang) {
                    if m.start < self.at.1 {
                        continue;
                    }
                    let word: String = chars[m.start..m.start + m.len].iter().collect();
                    if self.ignored.contains(&word) {
                        continue;
                    }
                    let left: String = chars[m.start.saturating_sub(30)..m.start].iter().collect();
                    let right: String = chars
                        [(m.start + m.len)..(m.start + m.len + 30).min(chars.len())]
                        .iter()
                        .collect();
                    let suggestions = spell::suggest(&word, &lang);
                    self.replacement = suggestions.first().cloned().unwrap_or_else(|| word.clone());
                    self.current = Some(Found {
                        id,
                        start: m.start,
                        word,
                        lang,
                        context: (left.replace('\n', " "), right.replace('\n', " ")),
                        suggestions,
                    });
                    return;
                }
            }
            self.at = (self.at.0 + 1, 0);
        }
        self.current = None;
        self.finished = true;
    }

    fn replace_at(doc: &mut DocState, id: &str, start: usize, len: usize, with: &str) {
        ops::modify(
            &mut doc.editor,
            &[id.to_string()],
            tr("Corrigir ortografia", "Fix spelling"),
            |o| {
                if let ObjectKind::Text(t) = &mut o.kind {
                    tedit::delete(t, start, start + len);
                    tedit::insert(t, start, with);
                    return true;
                }
                false
            },
        );
    }

    /// `true` quando fechou.
    pub fn show(&mut self, ctx: &egui::Context, doc: &mut DocState) -> bool {
        let mut close = false;
        let resp = widgets::modal(ctx, egui::Id::unique("verificar-ortografia")).show(ctx, |ui| {
            ui.set_width(theme::SPELL_DIALOG_WIDTH);
            ui.heading(tr("Verificar ortografia", "Check spelling"));
            ui.add_space(theme::SPACE_200);
            if self.targets.is_empty() {
                ui.label(tr(
                    "Não há texto para verificar.",
                    "There is no text to check.",
                ));
            } else if let Some(f) = self.current.clone() {
                if !spell::available(&f.lang) {
                    ui.colored_label(
                        theme::ACCENT_WARNING,
                        format!(
                            "{} {}",
                            tr(
                                "O corretor do sistema não tem o idioma",
                                "The system spell checker lacks the language"
                            ),
                            f.lang
                        ),
                    );
                }
                ui.horizontal_wrapped(|ui| {
                    ui.label(tr("Não encontrada:", "Not found:"));
                    ui.label(format!("…{}", f.context.0));
                    ui.label(
                        egui::RichText::new(&f.word)
                            .strong()
                            .color(theme::ACCENT_DANGER),
                    );
                    ui.label(format!("{}…", f.context.1));
                });
                ui.add_space(theme::SPACE_200);
                ui.horizontal(|ui| {
                    ui.label(tr("Trocar por", "Change to"));
                    ui.add(
                        egui::TextEdit::singleline(&mut self.replacement)
                            .desired_width(theme::FONT_COMBO_WIDTH),
                    );
                });
                egui::ScrollArea::vertical()
                    .max_height(theme::SPELL_LIST_HEIGHT)
                    .show(ui, |ui| {
                        if f.suggestions.is_empty() {
                            ui.weak(tr("Sem sugestões", "No suggestions"));
                        }
                        for s in &f.suggestions {
                            if ui.selectable_label(self.replacement == *s, s).clicked() {
                                self.replacement = s.clone();
                            }
                        }
                    });
                ui.add_space(theme::SPACE_300);
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .add(widgets::primary_button(tr("Substituir", "Replace")))
                        .clicked()
                    {
                        let with = self.replacement.clone();
                        Self::replace_at(doc, &f.id, f.start, f.word.chars().count(), &with);
                        self.at = (self.at.0, f.start + with.chars().count());
                        self.advance(doc);
                    }
                    if ui
                        .add(widgets::secondary_button(tr(
                            "Substituir todas",
                            "Replace all",
                        )))
                        .clicked()
                    {
                        let with = self.replacement.clone();
                        for id in self.targets.clone() {
                            if let Some((text, lang)) = text_of(doc, &id) {
                                let chars: Vec<char> = text.chars().collect();
                                let mut hits: Vec<usize> = spell::check(&text, &lang)
                                    .into_iter()
                                    .filter(|m| {
                                        chars[m.start..m.start + m.len].iter().collect::<String>()
                                            == f.word
                                    })
                                    .map(|m| m.start)
                                    .collect();
                                hits.sort_unstable_by(|a, b| b.cmp(a));
                                for s in hits {
                                    Self::replace_at(doc, &id, s, f.word.chars().count(), &with);
                                }
                            }
                        }
                        self.at = (self.at.0, f.start + with.chars().count());
                        self.advance(doc);
                    }
                    if ui
                        .add(widgets::secondary_button(tr("Ignorar", "Ignore")))
                        .clicked()
                    {
                        self.at = (self.at.0, f.start + 1);
                        self.advance(doc);
                    }
                    if ui
                        .add(widgets::secondary_button(tr("Ignorar todas", "Ignore all")))
                        .clicked()
                    {
                        self.ignored.insert(f.word.clone());
                        spell::ignore(&f.word, &f.lang);
                        self.advance(doc);
                    }
                    if ui
                        .add(widgets::secondary_button(tr(
                            "Adicionar ao dicionário",
                            "Add to dictionary",
                        )))
                        .clicked()
                    {
                        spell::add(&f.word, &f.lang);
                        self.ignored.insert(f.word.clone());
                        self.advance(doc);
                    }
                });
            } else {
                ui.label(tr(
                    "Verificação concluída: nenhuma outra palavra desconhecida.",
                    "Check complete: no other unknown words.",
                ));
            }
            ui.add_space(theme::SPACE_300);
            widgets::footer(ui, |ui| {
                if ui
                    .add(widgets::secondary_button(tr("Fechar", "Close")))
                    .clicked()
                    || ui.input(|i| i.key_pressed(egui::Key::Escape))
                {
                    close = true;
                }
            });
        });
        self.rect = resp.response.rect;
        close
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dialog_draws_without_texts() {
        let mut doc = DocState::new_blank();
        let mut d = SpellDialog::new(&doc);
        assert!(d.finished && d.current.is_none());
        let ctx = crate::theme::test_ctx();
        let mut out = ctx.run_ui(Default::default(), |ui| {
            d.show(ui.ctx(), &mut doc);
        });
        out.textures_delta.clear();
    }

    /// Com o português do sistema: acha "cazza" e substitui num passo.
    #[test]
    fn finds_and_replaces() {
        if !spell::available("pt-BR") {
            return;
        }
        let mut doc = DocState::new_blank();
        let mut t = tedit::new_text(10_000.0, 100_000.0, None, "Arial", 12_000.0);
        if let ObjectKind::Text(td) = &mut t.kind {
            tedit::insert(td, 0, "A casa cazza azul");
        }
        let id = ops::add_object(&mut doc.editor, 0, 0, t, "t");
        let d = SpellDialog::new(&doc);
        let f = d.current.clone().expect("acha o erro");
        assert_eq!(f.word, "cazza");
        SpellDialog::replace_at(&mut doc, &id, f.start, 5, "casa");
        let (text, _) = text_of(&doc, &id).unwrap();
        assert_eq!(text, "A casa casa azul");
    }
}
