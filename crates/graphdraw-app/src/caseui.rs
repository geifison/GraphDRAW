//! Janela Mudar caixa (GD-TXT-11/12): cinco modos exclusivos, prévia do
//! resultado e Aplicar | Cancelar. Shift+F3 abre com o último modo usado;
//! com a janela aberta, Shift+F3 de novo percorre minúsculas → MAIÚSCULAS →
//! Todas as Iniciais Maiúsculas.

use eframe::egui::{self, Key};
use graphdraw_core::edit::case::{self, CaseMode};
use graphdraw_core::edit::ops;
use graphdraw_core::model::{ObjectKind, TextData};

use crate::doc::DocState;
use crate::i18n::tr;
use crate::theme;

/// Id da janela (também usado no teste de layout).
pub const WINDOW_ID: &str = "mudar-caixa";

/// O que a janela muda: o trecho selecionado na edição ou os textos
/// selecionados inteiros.
#[derive(Debug, Clone, PartialEq)]
pub enum CaseTarget {
    Range { id: String, a: usize, b: usize },
    Whole(Vec<String>),
}

pub struct CaseDialog {
    pub mode: CaseMode,
    pub target: CaseTarget,
}

pub enum CaseAction {
    Apply,
    Cancel,
}

/// Nome do modo, escrito do jeito que ele deixa o texto.
pub fn label(m: CaseMode) -> &'static str {
    match m {
        CaseMode::Sentence => tr("Primeira letra da frase maiúscula", "Sentence case"),
        CaseMode::Lower => tr("minúsculas", "lowercase"),
        CaseMode::Upper => tr("MAIÚSCULAS", "UPPERCASE"),
        CaseMode::Title => tr("Todas as Iniciais Maiúsculas", "Capitalize Each Word"),
        CaseMode::Toggle => tr("iNVERTER mAIÚSCULAS/mINÚSCULAS", "tOGGLE cASE"),
    }
}

fn text_of<'a>(doc: &'a DocState, id: &str) -> Option<&'a TextData> {
    let r = doc.editor.find(id)?;
    match &doc.editor.get(&r)?.kind {
        ObjectKind::Text(t) => Some(t),
        _ => None,
    }
}

/// Caracteres mostrados na prévia.
const PREVIEW_CHARS: usize = 120;

impl CaseDialog {
    pub fn new(mode: CaseMode, target: CaseTarget) -> CaseDialog {
        CaseDialog { mode, target }
    }

    /// Resultado no primeiro texto alvo (cortado), com as quebras de
    /// parágrafo como `¶`.
    pub fn preview(&self, doc: &DocState) -> Option<String> {
        let (id, a, b) = match &self.target {
            CaseTarget::Range { id, a, b } => (id.as_str(), *a, *b),
            CaseTarget::Whole(ids) => (ids.first()?.as_str(), 0, usize::MAX),
        };
        let t = text_of(doc, id)?;
        let b = b.min(graphdraw_core::edit::text::len(t));
        let (_, after) = case::preview(t, a, b, self.mode);
        let mut s: String = after
            .chars()
            .take(PREVIEW_CHARS)
            .map(|c| if c == '\n' { '¶' } else { c })
            .collect();
        if after.chars().count() > PREVIEW_CHARS {
            s.push('…');
        }
        Some(s)
    }

    /// Desenha a janela; `shortcut` = atalho do comando no perfil ativo
    /// (a dica do ciclo só aparece com atalho).
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        doc: &DocState,
        shortcut: Option<String>,
    ) -> Option<CaseAction> {
        let preview = self.preview(doc);
        let mut act = None;
        crate::widgets::modal(ctx, egui::Id::unique(WINDOW_ID)).show(ctx, |ui| {
            ui.set_width(theme::CASE_DIALOG_WIDTH);
            ui.heading(tr("Mudar caixa", "Change case"));
            ui.add_space(theme::SPACE_300);
            for m in CaseMode::ALL {
                ui.radio_value(&mut self.mode, m, label(m));
            }
            ui.add_space(theme::SPACE_300);
            ui.label(tr("Prévia", "Preview"));
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.add(egui::Label::new(preview.unwrap_or_default()).wrap());
            });
            if let Some(k) = shortcut {
                ui.add_space(theme::SPACE_200);
                ui.weak(format!(
                    "{k} {}",
                    tr(
                        "de novo: minúsculas → MAIÚSCULAS → Iniciais",
                        "again: lowercase → UPPERCASE → Capitalized",
                    )
                ));
            }
            ui.add_space(theme::SPACE_300);
            crate::widgets::footer(ui, |ui| {
                if ui
                    .add(crate::widgets::primary_button(tr("Aplicar", "Apply")))
                    .clicked()
                    || ui.input(|i| i.key_pressed(Key::Enter))
                {
                    act = Some(CaseAction::Apply);
                }
                if ui
                    .add(crate::widgets::secondary_button(tr("Cancelar", "Cancel")))
                    .clicked()
                    || ui.input(|i| i.key_pressed(Key::Escape))
                {
                    act = Some(CaseAction::Cancel);
                }
            });
        });
        act
    }
}

/// Muda a caixa (um passo do histórico). No trecho, devolve o novo fim
/// dele (o texto pode alongar: ß → SS).
pub fn apply(doc: &mut DocState, target: &CaseTarget, mode: CaseMode) -> Option<usize> {
    let name = tr("Mudar caixa", "Change case");
    match target {
        CaseTarget::Range { id, a, b } => {
            let mut end = None;
            ops::modify(
                &mut doc.editor,
                std::slice::from_ref(id),
                name,
                |o| match &mut o.kind {
                    ObjectKind::Text(t) => {
                        end = Some(case::change_case(t, *a, *b, mode));
                        true
                    }
                    _ => false,
                },
            );
            end
        }
        CaseTarget::Whole(ids) => {
            ops::modify(&mut doc.editor, ids, name, |o| match &mut o.kind {
                ObjectKind::Text(t) => {
                    case::change_case_all(t, mode);
                    true
                }
                _ => false,
            });
            None
        }
    }
}
