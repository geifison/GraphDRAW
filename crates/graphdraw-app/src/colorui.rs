//! Janela "Gerenciamento de cor" do documento (GD-CMS-01/02): perfil RGB,
//! perfil CMYK, intenção de renderização e compensação de ponto preto.

use eframe::egui;
use graphdraw_core::cms::{Cms, ColorManagement, INTENTS};

use crate::i18n::tr;
use crate::theme;
use crate::widgets;

pub struct ColorDialog {
    pub cm: ColorManagement,
    /// Avisos da última configuração testada (perfil ausente/errado).
    warnings: Vec<String>,
    /// Descrição do perfil CMYK lido.
    cmyk_name: Option<String>,
    checked: Option<ColorManagement>,
}

impl ColorDialog {
    pub fn new(cm: &ColorManagement) -> ColorDialog {
        ColorDialog {
            cm: cm.clone(),
            warnings: Vec::new(),
            cmyk_name: None,
            checked: None,
        }
    }

    /// `Some(true)` = aplicar; `Some(false)` = cancelar.
    pub fn show(&mut self, ctx: &egui::Context) -> Option<bool> {
        // Lê os perfis só quando a escolha muda.
        if self.checked.as_ref() != Some(&self.cm) {
            let cms = Cms::new(&self.cm);
            self.warnings = cms.warnings.clone();
            self.cmyk_name = cms.cmyk_name.clone();
            self.checked = Some(self.cm.clone());
        }
        let mut out = None;
        widgets::modal(ctx, egui::Id::unique("gerenciamento-cor")).show(ctx, |ui| {
            ui.heading(tr("Gerenciamento de cor", "Color management"));
            ui.add_space(theme::SPACE_300);
            profile_row(ui, tr("Perfil RGB", "RGB profile"), &mut self.cm.rgb_profile, tr("sRGB IEC61966-2.1 (embutido)", "sRGB IEC61966-2.1 (built in)"));
            profile_row(ui, tr("Perfil CMYK", "CMYK profile"), &mut self.cm.cmyk_profile, tr("Sem perfil (conversão por fórmula)", "No profile (formula conversion)"));
            if let (Some(n), Some(_)) = (&self.cmyk_name, &self.cm.cmyk_profile) {
                ui.label(egui::RichText::new(n).weak());
            }
            ui.horizontal(|ui| {
                ui.label(tr("Intenção", "Intent"));
                let current = INTENTS
                    .iter()
                    .find(|(k, _)| *k == self.cm.intent)
                    .map_or("Colorimétrica relativa", |(_, l)| *l);
                egui::ComboBox::from_id_salt("intencao-cor")
                    .selected_text(current)
                    .show_ui(ui, |ui| {
                        for (k, label) in INTENTS {
                            ui.selectable_value(&mut self.cm.intent, k.to_string(), label);
                        }
                    });
            });
            ui.checkbox(&mut self.cm.black_point, tr("Compensação de ponto preto", "Black point compensation"));
            for w in &self.warnings {
                ui.colored_label(theme::ACCENT_WARNING, w);
            }
            ui.label(
                egui::RichText::new(tr(
                    "As cores ficam no modelo original; o perfil CMYK vale para a tela e para converter RGB no PDF/X-1a (e vai no arquivo como perfil de saída).",
                    "Colors keep their original model; the CMYK profile is used on screen and to convert RGB in PDF/X-1a (and is embedded as the output profile).",
                ))
                .weak(),
            );
            ui.add_space(theme::SPACE_300);
            widgets::footer(ui, |ui| {
                if ui.add(widgets::primary_button(tr("Aplicar", "Apply"))).clicked() {
                    out = Some(true);
                }
                if ui.add(widgets::secondary_button(tr("Cancelar", "Cancel"))).clicked()
                    || ui.input(|i| i.key_pressed(egui::Key::Escape))
                {
                    out = Some(false);
                }
            });
        });
        out
    }
}

fn profile_row(ui: &mut egui::Ui, label: &str, slot: &mut Option<String>, builtin: &str) {
    ui.label(egui::RichText::new(label).strong());
    ui.horizontal(|ui| {
        if ui.radio(slot.is_none(), builtin).clicked() {
            *slot = None;
        }
    });
    ui.horizontal(|ui| {
        let shown = slot
            .as_deref()
            .and_then(|p| std::path::Path::new(p).file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| tr("Arquivo ICC…", "ICC file…").into());
        let chosen = slot.is_some();
        let r = ui.radio(chosen, shown);
        if let Some(p) = slot.as_deref() {
            r.clone().on_hover_text(p);
        }
        if (r.clicked() || ui.button(tr("Escolher…", "Choose…")).clicked())
            && let Some(f) = rfd::FileDialog::new()
                .add_filter("ICC", &["icc", "icm"])
                .pick_file()
        {
            *slot = Some(f.display().to_string());
        }
    });
}
