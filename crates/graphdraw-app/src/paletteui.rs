//! Paletas na interface (GD-PAL-01..05): painel com as cores do documento
//! (globais: mudar a definição muda os objetos) e as paletas do usuário
//! (importadas de ASE, GPL, CSV ou JSON e guardadas no perfil), escolha da
//! paleta da barra, mistura com Ctrl (tint) e arrastar amostra até o objeto.

use std::path::PathBuf;

use eframe::egui::{self, Vec2};
use graphdraw_core::edit::palette::{self as pal, Palette, PaletteFormat};
use graphdraw_core::model::{Color, Fill};

use crate::doc::DocState;
use crate::i18n::tr;
use crate::theme;
use crate::widgets::{self, swatch};

/// O que o usuário pediu com uma amostra.
#[derive(Debug, Clone, PartialEq)]
pub enum PalAction {
    Fill(Option<Color>),
    Stroke(Option<Color>),
    /// Ctrl + clique: mistura 10% da cor no preenchimento atual.
    Tint(Color),
}

/// Amostra arrastada até um objeto do canvas (GD-PAL-05).
#[derive(Debug, Clone)]
pub struct SwatchDrop(pub Color);

#[derive(Default)]
pub struct Palettes {
    /// Paletas do usuário (guardadas em `%APPDATA%\GraphDraw\paletas`).
    pub user: Vec<Palette>,
    /// Paleta da barra: 0 = padrão, 1 = documento, 2.. = do usuário.
    pub bar_source: usize,
    /// Cor do documento em edição (ID e cor provisória).
    editing: Option<(String, Color)>,
    renaming: Option<(String, String)>,
    pub status: Option<String>,
}

fn dir() -> Option<PathBuf> {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from))
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("GraphDraw").join("paletas"))
}

/// Mistura `t` de `add` em `base` (mesmo modelo: componente a componente;
/// modelos diferentes: em RGB).
pub fn tint(base: &Color, add: &Color, t: f64) -> Color {
    if base.model == add.model && base.values.len() == add.values.len() {
        let mut c = base.clone();
        c.reference = None;
        for (v, a) in c.values.iter_mut().zip(&add.values) {
            *v += (a - *v) * t;
        }
        return c;
    }
    let (b, a) = (base.to_srgb(), add.to_srgb());
    Color::rgb(
        b[0] + (a[0] - b[0]) * t,
        b[1] + (a[1] - b[1]) * t,
        b[2] + (a[2] - b[2]) * t,
    )
}

impl Palettes {
    /// Paletas do usuário salvas (nada nos testes).
    pub fn load() -> Palettes {
        let mut p = Palettes::default();
        if cfg!(test) {
            return p;
        }
        if let Some(d) = dir()
            && let Ok(rd) = std::fs::read_dir(d)
        {
            let mut files: Vec<PathBuf> = rd.filter_map(|e| e.ok().map(|e| e.path())).collect();
            files.sort();
            for f in files {
                if let Ok(bytes) = std::fs::read(&f)
                    && let Ok(pl) = pal::read_palette(&bytes, PaletteFormat::Json, "")
                {
                    p.user.push(pl);
                }
            }
        }
        p
    }

    fn save_user(p: &Palette) {
        if cfg!(test) {
            return;
        }
        if let Some(d) = dir() {
            let _ = std::fs::create_dir_all(&d);
            let safe: String = p
                .name
                .chars()
                .map(|c| {
                    if c.is_alphanumeric() || c == ' ' || c == '-' {
                        c
                    } else {
                        '_'
                    }
                })
                .collect();
            let _ = std::fs::write(
                d.join(format!("{safe}.json")),
                pal::write_palette(p, PaletteFormat::Json),
            );
        }
    }

    /// Nomes das fontes de paleta para a barra.
    pub fn sources(&self) -> Vec<String> {
        let mut v = vec![
            tr("Padrão", "Default").to_string(),
            tr("Documento", "Document").to_string(),
        ];
        v.extend(self.user.iter().map(|p| p.name.clone()));
        v
    }

    /// Cores da paleta escolhida na barra (as do documento vêm ligadas).
    pub fn bar_colors(&self, doc: Option<&DocState>) -> Vec<(String, Color)> {
        match self.bar_source {
            0 => widgets::palette()
                .into_iter()
                .map(|c| (String::new(), c))
                .collect(),
            1 => doc
                .map(|d| {
                    d.doc()
                        .colors
                        .colors
                        .iter()
                        .filter_map(|c| pal::linked(&d.editor, &c.id).map(|l| (c.name.clone(), l)))
                        .collect()
                })
                .unwrap_or_default(),
            n => self
                .user
                .get(n - 2)
                .map(|p| {
                    p.colors
                        .iter()
                        .map(|c| (c.name.clone(), c.color.clone()))
                        .collect()
                })
                .unwrap_or_default(),
        }
    }

    /// Painel Paletas.
    pub fn panel(&mut self, ui: &mut egui::Ui, doc: &mut DocState) -> Option<PalAction> {
        let mut act = None;
        ui.label(egui::RichText::new(tr("Cores do documento", "Document colors")).strong());
        ui.weak(tr(
            "Cores globais: mudar uma muda todos os objetos que a usam",
            "Global colors: changing one changes every object using it",
        ));
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(
                    !doc.selection.is_empty(),
                    egui::Button::new(tr("+ Da seleção", "+ From selection")).small(),
                )
                .on_hover_text(tr(
                    "Guarda o preenchimento do objeto selecionado",
                    "Stores the selected object's fill",
                ))
                .clicked()
                && let Some(Fill::Solid { color, .. }) =
                    doc.selected_objects().first().and_then(|o| o.fill.clone())
            {
                let n = doc.doc().colors.colors.len() + 1;
                let id = pal::add_named(
                    &mut doc.editor,
                    &format!("{} {n}", tr("Cor", "Color")),
                    color,
                );
                // O objeto passa a usar a cor global.
                if let Some(l) = pal::linked(&doc.editor, &id) {
                    act = Some(PalAction::Fill(Some(l)));
                }
            }
            if ui
                .small_button(tr("+ Cores usadas", "+ Used colors"))
                .on_hover_text(tr(
                    "Acrescenta as cores usadas no documento",
                    "Adds the colors used in the document",
                ))
                .clicked()
            {
                let have: Vec<Color> = doc
                    .doc()
                    .colors
                    .colors
                    .iter()
                    .map(|c| c.color.clone())
                    .collect();
                let used = pal::colors_used(&doc.editor);
                doc.editor
                    .transaction(tr("Cores usadas", "Used colors"), |ed| {
                        for (k, c) in used.into_iter().filter(|c| !have.contains(c)).enumerate() {
                            pal::add_named(ed, &format!("{} {}", tr("Usada", "Used"), k + 1), c);
                        }
                    });
            }
            if ui.small_button(tr("Exportar…", "Export…")).clicked() {
                self.export(doc);
            }
        });
        let colors = doc.doc().colors.colors.clone();
        for c in &colors {
            ui.horizontal(|ui| {
                let r = swatch_drag(ui, &c.color, theme::SWATCH);
                if let Some(a) = swatch_action(
                    ui,
                    &r,
                    pal::linked(&doc.editor, &c.id).unwrap_or(c.color.clone()),
                ) {
                    act = Some(a);
                }
                match &mut self.renaming {
                    Some((id, buf)) if *id == c.id => {
                        let e = ui.text_edit_singleline(buf);
                        e.request_focus();
                        if e.lost_focus() {
                            if !buf.trim().is_empty() {
                                pal::rename_named(&mut doc.editor, &c.id, buf.trim());
                            }
                            self.renaming = None;
                        }
                    }
                    _ => {
                        let l = ui.selectable_label(
                            self.editing.as_ref().is_some_and(|(id, _)| *id == c.id),
                            &c.name,
                        );
                        if l.double_clicked() {
                            self.renaming = Some((c.id.clone(), c.name.clone()));
                        } else if l.clicked() {
                            self.editing = match &self.editing {
                                Some((id, _)) if *id == c.id => None,
                                _ => Some((c.id.clone(), c.color.clone())),
                            };
                        }
                    }
                }
                if crate::icons::small_button(
                    ui,
                    "delete",
                    tr("Remover da paleta", "Remove from palette"),
                    true,
                )
                .clicked()
                {
                    pal::remove_named(&mut doc.editor, &c.id);
                }
            });
            if let Some((id, col)) = &mut self.editing
                && *id == c.id
            {
                if let Some(n) = widgets::color_editor(ui, col) {
                    *col = n;
                }
                if ui
                    .button(tr("Aplicar a todos os objetos", "Apply to all objects"))
                    .clicked()
                {
                    pal::set_named(&mut doc.editor, &c.id, col.clone());
                    self.editing = None;
                }
            }
        }
        if colors.is_empty() {
            ui.weak(tr(
                "Nenhuma cor salva neste documento",
                "No colors saved in this document",
            ));
        }
        ui.separator();
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(tr("Minhas paletas", "My palettes")).strong());
            if ui.small_button(tr("Importar…", "Import…")).clicked() {
                self.import();
            }
        });
        let mut remove = None;
        let mut to_doc = None;
        for (k, p) in self.user.iter().enumerate() {
            egui::CollapsingHeader::new(format!("{} ({})", p.name, p.colors.len()))
                .id_salt(("paleta-usuario", k))
                .show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::splat(theme::SPACE_050);
                        for c in &p.colors {
                            let r = swatch_drag(ui, &c.color, theme::SWATCH).on_hover_text(&c.name);
                            if let Some(a) = swatch_action(ui, &r, c.color.clone()) {
                                act = Some(a);
                            }
                        }
                    });
                    ui.horizontal(|ui| {
                        if ui
                            .small_button(tr("Pôr no documento", "Add to document"))
                            .clicked()
                        {
                            to_doc = Some(k);
                        }
                        if ui
                            .small_button(tr("Remover paleta", "Remove palette"))
                            .clicked()
                        {
                            remove = Some(k);
                        }
                    });
                });
        }
        if let Some(k) = to_doc {
            let p = self.user[k].clone();
            doc.editor
                .transaction(tr("Paleta no documento", "Palette into document"), |ed| {
                    for c in p.colors {
                        pal::add_named(ed, &c.name, c.color);
                    }
                });
        }
        if let Some(k) = remove {
            let p = self.user.remove(k);
            if !cfg!(test)
                && let Some(d) = dir()
            {
                let safe: String = p
                    .name
                    .chars()
                    .map(|c| {
                        if c.is_alphanumeric() || c == ' ' || c == '-' {
                            c
                        } else {
                            '_'
                        }
                    })
                    .collect();
                let _ = std::fs::remove_file(d.join(format!("{safe}.json")));
            }
            self.bar_source = self.bar_source.min(1 + self.user.len());
        }
        act
    }

    fn import(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter(tr("Paletas", "Palettes"), &["ase", "gpl", "csv", "json"])
            .pick_file()
        else {
            return;
        };
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let name = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let res = PaletteFormat::from_extension(ext)
            .ok_or_else(|| tr("formato desconhecido", "unknown format").to_string())
            .and_then(|f| {
                std::fs::read(&path)
                    .map_err(|e| e.to_string())
                    .and_then(|b| pal::read_palette(&b, f, &name))
            });
        match res {
            Ok(mut p) => {
                if p.name.trim().is_empty() {
                    p.name = name;
                }
                self.status = Some(format!(
                    "{} {} ({} {})",
                    tr("Paleta importada:", "Palette imported:"),
                    p.name,
                    p.colors.len(),
                    tr("cores", "colors")
                ));
                Self::save_user(&p);
                self.user.push(p);
            }
            Err(e) => {
                self.status = Some(format!(
                    "{} {e}",
                    tr("Não foi possível importar:", "Could not import:")
                ))
            }
        }
    }

    fn export(&mut self, doc: &DocState) {
        let Some(path) = rfd::FileDialog::new()
            .set_file_name("paleta.ase")
            .add_filter("ASE", &["ase"])
            .add_filter("GPL", &["gpl"])
            .add_filter("CSV", &["csv"])
            .add_filter("JSON", &["json"])
            .save_file()
        else {
            return;
        };
        let fmt = path
            .extension()
            .and_then(|e| e.to_str())
            .and_then(PaletteFormat::from_extension)
            .unwrap_or(PaletteFormat::Ase);
        let p = pal::document_palette(&doc.editor, &doc.title());
        self.status = Some(match std::fs::write(&path, pal::write_palette(&p, fmt)) {
            Ok(()) => format!(
                "{} {}",
                tr("Paleta exportada:", "Palette exported:"),
                path.display()
            ),
            Err(e) => format!("{} {e}", tr("Falha ao exportar:", "Export failed:")),
        });
    }
}

/// Amostra que também pode ser arrastada até um objeto do canvas.
pub fn swatch_drag(ui: &mut egui::Ui, c: &Color, size: f32) -> egui::Response {
    let r = swatch(ui, Some(c), size).interact(egui::Sense::click_and_drag());
    r.dnd_set_drag_payload(SwatchDrop(c.clone()));
    r
}

/// Clique = preenchimento, botão direito = contorno, Ctrl + clique = mistura.
pub fn swatch_action(ui: &egui::Ui, r: &egui::Response, c: Color) -> Option<PalAction> {
    if r.clicked() {
        if ui.input(|i| i.modifiers.command) {
            return Some(PalAction::Tint(c));
        }
        return Some(PalAction::Fill(Some(c)));
    }
    if r.secondary_clicked() {
        return Some(PalAction::Stroke(Some(c)));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use graphdraw_core::edit::palette::PaletteColor;

    #[test]
    fn gd_pal_05_tint_mixes_ten_percent() {
        let t = tint(
            &Color::cmyk(0.0, 0.0, 0.0, 0.0),
            &Color::cmyk(1.0, 0.0, 0.0, 0.0),
            0.1,
        );
        assert!((t.values[0] - 0.1).abs() < 1e-12 && t.model == "cmyk");
        let m = tint(&Color::rgb(1.0, 1.0, 1.0), &Color::gray(0.0), 0.5);
        assert_eq!(m.model, "rgb");
        assert!((m.values[0] - 0.5).abs() < 1e-9);
    }

    /// O painel desenha cores do documento e paletas do usuário sem mudar nada.
    #[test]
    fn palettes_panel_draws() {
        let ctx = crate::theme::test_ctx();
        let mut doc = DocState::new_blank();
        pal::add_named(&mut doc.editor, "Marca", Color::cmyk(0.0, 1.0, 0.0, 0.0));
        let rev = doc.editor.revision();
        let mut p = Palettes::default();
        p.user.push(Palette {
            name: "Minha".into(),
            colors: vec![PaletteColor {
                name: "A".into(),
                color: Color::rgb(0.1, 0.2, 0.3),
            }],
        });
        for _ in 0..2 {
            let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
                egui::CentralPanel::default().show(ui, |ui| {
                    assert!(p.panel(ui, &mut doc).is_none());
                });
            });
            out.textures_delta.clear();
        }
        assert_eq!(doc.editor.revision(), rev);
    }

    #[test]
    fn bar_sources_and_colors() {
        let mut p = Palettes::default();
        p.user.push(Palette {
            name: "Minha".into(),
            colors: vec![PaletteColor {
                name: "A".into(),
                color: Color::rgb(0.1, 0.2, 0.3),
            }],
        });
        assert_eq!(p.sources(), vec!["Padrão", "Documento", "Minha"]);
        let mut doc = DocState::new_blank();
        let id = pal::add_named(&mut doc.editor, "Marca", Color::cmyk(0.0, 1.0, 0.0, 0.0));
        p.bar_source = 1;
        let b = p.bar_colors(Some(&doc));
        assert_eq!(b.len(), 1);
        assert_eq!(
            b[0].1.reference.as_deref(),
            Some(id.as_str()),
            "do documento vem ligada"
        );
        p.bar_source = 2;
        assert_eq!(p.bar_colors(Some(&doc))[0].0, "A");
    }
}
