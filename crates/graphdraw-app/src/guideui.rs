//! Interface das guias (GD-GUI-01/02): menu de botão direito de uma guia
//! e janela de guias predefinidas.

use eframe::egui;
use graphdraw_core::edit::guides::{self, GuideRef, Presets};
use graphdraw_core::model::Guide;

use crate::doc::DocState;
use crate::i18n::tr;
use crate::theme;
use crate::widgets;

fn get(doc: &DocState, r: GuideRef) -> Option<Guide> {
    match r {
        GuideRef::Document(i) => doc.doc().data.settings.guides.get(i).cloned(),
        GuideRef::Page(p, i) => doc
            .doc()
            .data
            .pages
            .get(p)
            .and_then(|pg| pg.guides.get(i).cloned()),
    }
}

/// Menu da guia: posição/ângulo, travar, alcance e excluir. Devolve
/// `true` quando o menu deve fechar (a guia mudou de lugar na lista).
pub fn guide_menu(ui: &mut egui::Ui, doc: &mut DocState, r: GuideRef) -> bool {
    let Some(g) = get(doc, r) else {
        return true;
    };
    let mut new = g.clone();
    match g.kind.as_str() {
        "horizontal" | "vertical" => {
            ui.horizontal(|ui| {
                ui.label(if g.kind == "horizontal" { "Y" } else { "X" });
                if let Some(v) = widgets::length_field(ui, "guia-pos", g.position, None) {
                    new.position = v;
                }
            });
        }
        _ => {
            let p = g.point.unwrap_or([0.0, 0.0]);
            ui.horizontal(|ui| {
                ui.label("X");
                if let Some(v) = widgets::length_field(ui, "guia-x", p[0], None) {
                    new.point = Some([v, p[1]]);
                }
            });
            ui.horizontal(|ui| {
                ui.label("Y");
                if let Some(v) = widgets::length_field(ui, "guia-y", p[1], None) {
                    new.point = Some([p[0], v]);
                }
            });
        }
    }
    // Ângulo: numa guia reta, inclinar a transforma em guia inclinada.
    ui.horizontal(|ui| {
        ui.label(tr("Ângulo", "Angle"));
        let current = match g.kind.as_str() {
            "horizontal" => 0.0,
            "vertical" => 90.0,
            _ => g.angle,
        };
        if let Some(a) = widgets::number_field(ui, "guia-ang", current, "°") {
            let (o, _) = g.line();
            new = match ((a % 180.0 + 180.0) % 180.0) as i64 {
                0 if a.fract() == 0.0 => Guide {
                    kind: "horizontal".into(),
                    position: o.y,
                    point: None,
                    angle: 0.0,
                    ..g.clone()
                },
                90 if a.fract() == 0.0 => Guide {
                    kind: "vertical".into(),
                    position: o.x,
                    point: None,
                    angle: 0.0,
                    ..g.clone()
                },
                _ => Guide {
                    kind: "angled".into(),
                    point: Some([o.x, o.y]),
                    angle: a,
                    ..g.clone()
                },
            };
        }
    });
    ui.separator();
    let mut close = false;
    if ui
        .button(if g.locked {
            tr("Destravar guia", "Unlock guide")
        } else {
            tr("Travar guia", "Lock guide")
        })
        .clicked()
    {
        new.locked = !g.locked;
        close = true;
    }
    let all = matches!(r, GuideRef::Document(_));
    if ui
        .button(if all {
            tr("Só nesta página", "This page only")
        } else {
            tr("Em todas as páginas", "On all pages")
        })
        .clicked()
    {
        guides::move_scope(&mut doc.editor, r, doc.page);
        return true;
    }
    if ui.button(tr("Excluir guia", "Delete guide")).clicked() {
        guides::remove(&mut doc.editor, r);
        return true;
    }
    if new != g {
        guides::set(&mut doc.editor, r, new, tr("Editar guia", "Edit guide"));
    }
    close
}

/// Janela "Guias predefinidas" (GD-GUI-02).
pub struct PresetDialog {
    pub margin_on: bool,
    pub margin: f64,
    pub columns_on: bool,
    pub columns: u32,
    pub gutter: f64,
    pub center: bool,
    pub all_pages: bool,
}

impl Default for PresetDialog {
    fn default() -> Self {
        PresetDialog {
            margin_on: true,
            margin: 10_000.0,
            columns_on: false,
            columns: 2,
            gutter: 5_000.0,
            center: false,
            all_pages: true,
        }
    }
}

impl PresetDialog {
    pub fn presets(&self) -> Presets {
        Presets {
            margin: self.margin_on.then_some(self.margin),
            columns: self.columns_on.then_some((self.columns, self.gutter)),
            center: self.center,
        }
    }

    /// `Some(true)` = criar; `Some(false)` = cancelar.
    pub fn show(&mut self, ctx: &egui::Context) -> Option<bool> {
        let mut out = None;
        widgets::modal(ctx, egui::Id::unique("guias-predefinidas")).show(ctx, |ui| {
            ui.heading(tr("Guias predefinidas", "Guide presets"));
            ui.add_space(theme::SPACE_300);
            ui.horizontal(|ui| {
                ui.checkbox(&mut self.margin_on, tr("Margens", "Margins"));
                if let Some(v) = widgets::length_field(ui, "pre-margem", self.margin, None) {
                    self.margin = v.max(0.0);
                }
            });
            ui.horizontal(|ui| {
                ui.checkbox(&mut self.columns_on, tr("Colunas", "Columns"));
                ui.add(egui::DragValue::new(&mut self.columns).range(2..=24));
                ui.label(tr("medianiz", "gutter"));
                if let Some(v) = widgets::length_field(ui, "pre-medianiz", self.gutter, None) {
                    self.gutter = v.max(0.0);
                }
            });
            ui.checkbox(&mut self.center, tr("Centro da página", "Page center"));
            ui.separator();
            ui.horizontal(|ui| {
                ui.radio_value(
                    &mut self.all_pages,
                    true,
                    tr("Todas as páginas", "All pages"),
                );
                ui.radio_value(
                    &mut self.all_pages,
                    false,
                    tr("Só esta página", "This page only"),
                );
            });
            ui.add_space(theme::SPACE_300);
            widgets::footer(ui, |ui| {
                if ui
                    .add(widgets::primary_button(tr("Criar", "Create")))
                    .clicked()
                {
                    out = Some(true);
                }
                if ui
                    .add(widgets::secondary_button(tr("Cancelar", "Cancel")))
                    .clicked()
                    || ui.input(|i| i.key_pressed(egui::Key::Escape))
                {
                    out = Some(false);
                }
            });
        });
        out
    }
}
