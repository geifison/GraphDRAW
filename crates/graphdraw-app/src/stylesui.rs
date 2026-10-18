//! Aba "Estilos" (doc 11): estilos de objeto, estilos de texto e símbolos
//! do documento. Cada linha aplica ao clicar; o menu da linha redefine pela
//! seleção, renomeia e exclui.

use eframe::egui;
use graphdraw_core::edit::styles;
use graphdraw_core::model::{Color, Fill, ObjectKind};
use kurbo::Point;

use crate::doc::DocState;
use crate::i18n::tr;
use crate::widgets;

/// Cor que representa o preenchimento na amostra.
fn fill_color(f: &Option<Fill>) -> Option<&Color> {
    match f {
        Some(Fill::Solid { color, .. }) => Some(color),
        Some(Fill::Gradient(g)) => g.stops.first().map(|s| &s.color),
        _ => None,
    }
}

#[derive(Default)]
pub struct StylesPanel {
    /// Linha em renomeação: (ID, nome em edição).
    renaming: Option<(String, String)>,
}

enum Act {
    Apply(String),
    Redefine(String),
    Delete(String),
    Rename(String, String),
}

impl StylesPanel {
    /// Linha com amostra, nome e menu; devolve a ação escolhida.
    fn row(
        &mut self,
        ui: &mut egui::Ui,
        id: &str,
        name: &str,
        has_sel: bool,
        sample: impl FnOnce(&mut egui::Ui),
    ) -> Option<Act> {
        let mut act = None;
        ui.horizontal(|ui| {
            sample(ui);
            if let Some((rid, text)) = &mut self.renaming
                && rid == id
            {
                let r = ui.text_edit_singleline(text);
                if r.lost_focus() {
                    if ui.input(|i| i.key_pressed(egui::Key::Enter)) && !text.trim().is_empty() {
                        act = Some(Act::Rename(id.into(), text.trim().into()));
                    }
                    self.renaming = None;
                } else {
                    r.request_focus();
                }
                return;
            }
            let r = ui
                .add_enabled(has_sel, egui::Button::new(name).frame(false))
                .on_hover_text(tr("Aplicar à seleção", "Apply to selection"))
                .on_disabled_hover_text(tr(
                    "Selecione objetos para aplicar",
                    "Select objects to apply",
                ));
            if r.clicked() {
                act = Some(Act::Apply(id.into()));
            }
            ui.menu_button("…", |ui| {
                if ui
                    .add_enabled(
                        has_sel,
                        egui::Button::new(tr("Redefinir pela seleção", "Redefine from selection")),
                    )
                    .clicked()
                {
                    act = Some(Act::Redefine(id.into()));
                }
                if ui.button(tr("Renomear", "Rename")).clicked() {
                    self.renaming = Some((id.into(), name.into()));
                }
                if ui.button(tr("Excluir", "Delete")).clicked() {
                    act = Some(Act::Delete(id.into()));
                }
            });
        });
        act
    }

    pub fn panel(&mut self, ui: &mut egui::Ui, doc: &mut DocState) {
        let sel = doc.selection.clone();
        let first = sel.first().cloned();
        let first_is_text = doc
            .selected_objects()
            .first()
            .is_some_and(|o| matches!(o.kind, ObjectKind::Text(_)));
        let has_sel = !sel.is_empty();
        let st = doc.editor.doc.styles.clone();

        // ---- estilos de objeto ----
        ui.horizontal(|ui| {
            ui.strong(tr("Estilos de objeto", "Object styles"));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add_enabled(first.is_some(), egui::Button::new("+"))
                    .on_hover_text(tr(
                        "Novo estilo com a aparência da seleção",
                        "New style from selection",
                    ))
                    .clicked()
                    && let Some(f) = &first
                {
                    let n = format!("{} {}", tr("Estilo", "Style"), st.object.len() + 1);
                    styles::create_object_style(&mut doc.editor, &n, f);
                }
            });
        });
        if st.object.is_empty() {
            ui.weak(tr(
                "Nenhum. Selecione um objeto e use +.",
                "None. Select an object and press +.",
            ));
        }
        for s in &st.object {
            let act = self.row(ui, &s.id, &s.name, has_sel, |ui| {
                widgets::swatch(ui, fill_color(&s.fill), 16.0);
            });
            match act {
                Some(Act::Apply(id)) => styles::apply_object_style(&mut doc.editor, &sel, &id),
                Some(Act::Redefine(id)) => {
                    if let Some(f) = &first {
                        styles::redefine_object_style(&mut doc.editor, &id, f);
                    }
                }
                Some(Act::Delete(id)) => styles::delete_object_style(&mut doc.editor, &id),
                Some(Act::Rename(id, n)) => styles::rename_style(&mut doc.editor, &id, &n),
                None => {}
            }
        }

        ui.separator();
        // ---- estilos de texto ----
        ui.horizontal(|ui| {
            ui.strong(tr("Estilos de texto", "Text styles"));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add_enabled(first_is_text, egui::Button::new("+"))
                    .on_hover_text(tr(
                        "Novo estilo com a fonte do texto selecionado",
                        "New style from selected text",
                    ))
                    .clicked()
                    && let Some(f) = &first
                {
                    let n = format!("{} {}", tr("Texto", "Text"), st.text.len() + 1);
                    styles::create_text_style(&mut doc.editor, &n, f);
                }
            });
        });
        if st.text.is_empty() {
            ui.weak(tr(
                "Nenhum. Selecione um texto e use +.",
                "None. Select a text and press +.",
            ));
        }
        for s in &st.text {
            let label = format!("{} · {} {:.0} pt", s.name, s.family, s.size / 352.778);
            let color = fill_color(&s.fill).map(widgets::to_color32);
            let (bold, italic) = (s.weight >= 600, s.italic);
            let act = self.row(ui, &s.id, &label, has_sel, |ui| {
                let mut t = egui::RichText::new("Aa").size(15.0);
                if bold {
                    t = t.strong();
                }
                if italic {
                    t = t.italics();
                }
                if let Some(c) = color {
                    t = t.color(c);
                }
                ui.label(t);
            });
            match act {
                Some(Act::Apply(id)) => styles::apply_text_style(&mut doc.editor, &sel, &id),
                Some(Act::Redefine(id)) => {
                    if let Some(f) = &first {
                        styles::redefine_text_style(&mut doc.editor, &id, f);
                    }
                }
                Some(Act::Delete(id)) => styles::delete_text_style(&mut doc.editor, &id),
                Some(Act::Rename(id, n)) => styles::rename_style(&mut doc.editor, &id, &n),
                None => {}
            }
        }

        ui.separator();
        // ---- símbolos ----
        ui.horizontal(|ui| {
            ui.strong(tr("Símbolos", "Symbols"));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add_enabled(has_sel, egui::Button::new("+"))
                    .on_hover_text(tr(
                        "Criar símbolo com a seleção",
                        "Create symbol from selection",
                    ))
                    .clicked()
                {
                    let n = format!("{} {}", tr("Símbolo", "Symbol"), st.symbols.len() + 1);
                    if let Some((_, inst)) = styles::create_symbol(&mut doc.editor, &sel, &n) {
                        doc.selection = vec![inst];
                    }
                }
            });
        });
        if st.symbols.is_empty() {
            ui.weak(tr(
                "Nenhum. Selecione objetos e use +.",
                "None. Select objects and press +.",
            ));
        }
        // Instância selecionada: o símbolo dela pode ser redefinido por ela.
        let inst_of = doc.selected_objects().first().and_then(|o| match &o.kind {
            ObjectKind::Group(g) => g.symbol.clone(),
            _ => None,
        });
        for s in &st.symbols {
            let count = s.objects.len();
            let mine = inst_of.as_deref() == Some(s.id.as_str());
            let act = self.row(ui, &s.id, &s.name, true, |ui| {
                ui.weak(format!("{count}▪"))
                    .on_hover_text(tr("Objetos no símbolo", "Objects in symbol"));
            });
            match act {
                Some(Act::Apply(id)) => {
                    // Inserir: nova instância no centro da página.
                    let (w, h) = (doc.page().width, doc.page().height);
                    let page = doc.page;
                    if let Some(layer) = doc.layer_for_new_object()
                        && let Some(i) = styles::insert_symbol(
                            &mut doc.editor,
                            page,
                            layer,
                            &id,
                            Point::new(w / 2.0, h / 2.0),
                        )
                    {
                        doc.selection = vec![i];
                    }
                }
                Some(Act::Redefine(id)) => {
                    if mine && let Some(f) = &first {
                        styles::redefine_symbol(&mut doc.editor, &id, f);
                    }
                }
                Some(Act::Delete(id)) => styles::delete_symbol(&mut doc.editor, &id),
                Some(Act::Rename(id, n)) => styles::rename_style(&mut doc.editor, &id, &n),
                None => {}
            }
        }
        if !st.symbols.is_empty() {
            ui.weak(tr(
                "Clique no símbolo para inserir. Para mudar todas as cópias, edite uma instância e use … → Redefinir.",
                "Click a symbol to insert. To change every copy, edit an instance and use … → Redefine.",
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use graphdraw_core::model::{Object, RectData};

    #[test]
    fn panel_lists_styles_and_symbols() {
        let mut doc = DocState::new_blank();
        let mut o = Object::new(ObjectKind::Rectangle(RectData {
            width: 10_000.0,
            height: 10_000.0,
            corners: None,
            extra: Default::default(),
        }));
        o.fill = Some(Fill::solid(Color::black()));
        let id = graphdraw_core::edit::ops::add_object(&mut doc.editor, 0, 0, o, "r");
        styles::create_object_style(&mut doc.editor, "Marca", &id);
        let (_, inst) =
            styles::create_symbol(&mut doc.editor, std::slice::from_ref(&id), "Logo").unwrap();
        doc.selection = vec![inst];
        let mut p = StylesPanel::default();
        let ctx = crate::theme::test_ctx();
        for k in 0..3 {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(320.0, 600.0),
                )),
                time: Some(k as f64 / 30.0),
                ..Default::default()
            };
            let mut out = ctx.run_ui(input, |ui| p.panel(ui, &mut doc));
            out.textures_delta.clear();
        }
        assert_eq!(doc.editor.doc.styles.object.len(), 1);
        assert_eq!(doc.editor.doc.styles.symbols.len(), 1);
    }
}
