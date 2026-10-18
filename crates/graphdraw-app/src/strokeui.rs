//! Janela da Caneta de contorno (GD-OUT-01..08): coluna esquerda com cor,
//! espessura (unidade e predefinições), estilo da linha (com o editor de
//! tracejado e o comportamento nos cantos), cantos, pontas e posição;
//! coluna direita com as pontas de seta, a caneta caligráfica (bico
//! interativo), a largura variável e a prévia; embaixo, as opções de
//! comportamento e OK/Cancelar/Ajuda. Nada muda no documento até o OK.

use std::collections::HashMap;
use std::path::PathBuf;

use eframe::egui::{self, Color32, Vec2};
use graphdraw_core::edit::ops;
use graphdraw_core::model::{Arrow, Stroke};
use serde::{Deserialize, Serialize};

use crate::canvas::Defaults;
use crate::colorpick::{ColorPicker, Eyedropper, PaletteList, color_button};
use crate::dialogs::{
    DASHES, arrow_label, dash_for, detect_dash, merge_changes, stroke_preview, width_label,
};
use crate::doc::{DocState, default_stroke};
use crate::i18n::tr;
use crate::icons;
use crate::theme;
use crate::widgets;

/// Unidades da espessura: nome e µm por unidade.
const UNITS: [(&str, f64); 6] = [
    ("pt", graphdraw_core::units::UM_PER_PT),
    ("mm", 1_000.0),
    ("cm", 10_000.0),
    ("in", 25_400.0),
    ("px", 25_400.0 / 96.0),
    ("m", 1_000_000.0),
];

/// Espessuras prontas em pontos (0 = fio de cabelo).
const WIDTH_PRESETS: [f64; 16] = [
    0.0, 0.1, 0.25, 0.5, 0.75, 1.0, 1.5, 2.0, 3.0, 4.0, 6.0, 8.0, 12.0, 16.0, 24.0, 36.0,
];

/// Estilo de linha do usuário: nome e padrão em múltiplos da espessura.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LineStyle {
    pub name: String,
    pub pattern: Vec<f64>,
}

fn styles_file() -> Option<PathBuf> {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from))
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("GraphDraw").join("estilos-linha.json"))
}

pub fn load_styles() -> Vec<LineStyle> {
    styles_file()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

fn save_styles(v: &[LineStyle]) {
    if let Some(p) = styles_file() {
        if let Some(d) = p.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        if let Ok(t) = serde_json::to_string_pretty(v) {
            let _ = std::fs::write(p, t);
        }
    }
}

/// Unidade do tracejado: a espessura (mínimo 0,25 mm).
fn dash_unit(width: f64) -> f64 {
    width.max(250.0)
}

/// Células (traço = `true`) → padrão em múltiplos (corridas alternadas,
/// começando por um traço).
pub fn cells_to_pattern(cells: &[bool]) -> Vec<f64> {
    let Some(first) = cells.iter().position(|c| *c) else {
        return Vec::new();
    };
    let rot: Vec<bool> = cells[first..]
        .iter()
        .chain(&cells[..first])
        .copied()
        .collect();
    let mut out: Vec<f64> = Vec::new();
    let mut cur = true;
    let mut run = 0.0;
    for c in rot {
        if c == cur {
            run += 1.0;
        } else {
            out.push(run);
            cur = c;
            run = 1.0;
        }
    }
    out.push(run);
    if out.len() % 2 == 1 {
        // Termina num traço: emenda com o primeiro na repetição.
        if out.len() > 1 {
            let last = out.pop().unwrap();
            out[0] += last;
        } else {
            return Vec::new();
        }
    }
    out
}

/// Padrão em múltiplos → células (arredonda para células inteiras).
pub fn pattern_to_cells(p: &[f64]) -> Vec<bool> {
    let mut out = Vec::new();
    for (i, v) in p.iter().enumerate() {
        for _ in 0..(v.round().max(1.0) as usize) {
            out.push(i % 2 == 0);
        }
    }
    if out.is_empty() {
        out = vec![true, true, true, true, false, false];
    }
    out.truncate(64);
    out
}

/// Qual estilo de linha está escolhido.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    Builtin(usize),
    User(usize),
    Custom,
}

/// Editor de estilo de linha: régua de células de traço/espaço.
struct DashEditor {
    cells: Vec<bool>,
    name: String,
}

pub struct StrokeDialog {
    initial: Option<Stroke>,
    pub(crate) draft: Stroke,
    pub(crate) enabled: bool,
    style: Style,
    styles: Vec<LineStyle>,
    unit: usize,
    for_defaults: bool,
    pub(crate) palettes: PaletteList,
    picker: ColorPicker,
    eye: Eyedropper,
    editor: Option<DashEditor>,
    nib_drag: Option<bool>,
    preview: Option<((Stroke, bool), egui::TextureHandle)>,
    thumbs: HashMap<(String, bool), egui::TextureHandle>,
    pub(crate) rect: egui::Rect,
}

impl StrokeDialog {
    pub fn new(initial: Option<Stroke>, for_defaults: bool) -> StrokeDialog {
        let draft = initial.clone().unwrap_or_else(default_stroke);
        let styles = load_styles();
        let mut d = StrokeDialog {
            enabled: initial.is_some(),
            style: Style::Custom,
            styles,
            unit: 1,
            initial,
            draft,
            for_defaults,
            palettes: Vec::new(),
            picker: ColorPicker::default(),
            eye: Eyedropper::default(),
            editor: None,
            nib_drag: None,
            preview: None,
            thumbs: HashMap::new(),
            rect: egui::Rect::NOTHING,
        };
        d.style = d.detect_style();
        d
    }

    fn detect_style(&self) -> Style {
        let i = detect_dash(&self.draft);
        if i < DASHES.len() {
            return Style::Builtin(i);
        }
        let u = dash_unit(self.draft.width);
        let d = self.draft.dash.clone().unwrap_or_default();
        self.styles
            .iter()
            .position(|s| {
                s.pattern.len() == d.len()
                    && s.pattern
                        .iter()
                        .zip(&d)
                        .all(|(k, v)| (k * u - v).abs() < 1.0)
            })
            .map_or(Style::Custom, Style::User)
    }

    /// Reaplica o estilo escolhido na espessura atual.
    fn restyle(&mut self) {
        let u = dash_unit(self.draft.width);
        match self.style {
            Style::Builtin(i) => self.draft.dash = dash_for(i, self.draft.width),
            Style::User(i) => {
                if let Some(s) = self.styles.get(i) {
                    self.draft.dash = Some(s.pattern.iter().map(|k| k * u).collect());
                }
            }
            Style::Custom => {}
        }
    }

    fn result(&self) -> Option<Stroke> {
        self.enabled.then(|| self.draft.clone())
    }

    /// Mostra a janela; `true` quando ela foi fechada.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        doc: &mut DocState,
        defaults: &mut Defaults,
    ) -> bool {
        if self.eye.active() {
            if let Some(c) = self.eye.run(ctx) {
                self.draft.color = c;
            }
            return false;
        }
        let title = if self.for_defaults {
            tr(
                "Caneta de contorno dos novos objetos",
                "Outline pen for new objects",
            )
        } else {
            tr("Caneta de contorno", "Outline pen")
        };
        let resp = widgets::modal(ctx, egui::Id::unique("janela-contorno")).show(ctx, |ui| {
            ui.set_width(theme::STROKE_DIALOG_WIDTH);
            ui.horizontal(|ui| {
                ui.heading(title);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.checkbox(&mut self.enabled, tr("Com contorno", "Has outline"));
                });
            });
            ui.separator();
            let max_h = (ui.ctx().content_rect().height() * theme::MODAL_MAX_HEIGHT).max(240.0);
            egui::ScrollArea::vertical()
                .id_salt("contorno-corpo")
                .max_height(max_h)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    ui.add_enabled_ui(self.enabled, |ui| {
                        ui.horizontal_top(|ui| {
                            let col = (theme::STROKE_DIALOG_WIDTH - theme::SPACE_600) / 2.0;
                            ui.vertical(|ui| {
                                ui.set_width(col);
                                self.left(ui);
                            });
                            ui.add_space(theme::SPACE_600);
                            ui.vertical(|ui| {
                                ui.set_width(col);
                                self.right(ui);
                            });
                        });
                        ui.separator();
                        self.options(ui);
                    });
                });
            ui.separator();
            self.footer(ui)
        });
        self.rect = resp.response.rect;
        if let Some(ed) = &mut self.editor {
            match dash_editor(ctx, ed, &self.styles, self.style) {
                EditorOut::None => {}
                EditorOut::Close => self.editor = None,
                EditorOut::Use(p) => {
                    let u = dash_unit(self.draft.width);
                    self.draft.dash = (!p.is_empty()).then(|| p.iter().map(|k| k * u).collect());
                    self.style = self.detect_style();
                    self.editor = None;
                }
                EditorOut::Add(name, p) => {
                    self.styles.push(LineStyle {
                        name,
                        pattern: p.clone(),
                    });
                    save_styles(&self.styles);
                    self.style = Style::User(self.styles.len() - 1);
                    self.restyle();
                    self.editor = None;
                }
                EditorOut::Replace(i, p) => {
                    if let Some(s) = self.styles.get_mut(i) {
                        s.pattern = p;
                    }
                    save_styles(&self.styles);
                    self.style = Style::User(i);
                    self.restyle();
                    self.editor = None;
                }
            }
        }
        match resp.inner {
            None => false,
            Some(false) => true,
            Some(true) => {
                self.apply(doc, defaults);
                true
            }
        }
    }

    fn heading(ui: &mut egui::Ui, t: &str) {
        ui.add_space(theme::SPACE_200);
        ui.label(egui::RichText::new(t).text_style(theme::section_style()));
    }

    /// Coluna esquerda: cor, espessura, estilo da linha, cantos, pontas e posição.
    fn left(&mut self, ui: &mut egui::Ui) {
        Self::heading(ui, tr("Cor e espessura", "Color and width"));
        ui.horizontal(|ui| {
            let popup_id = ui.make_persistent_id("contorno-cor");
            let open = egui::Popup::is_id_open(ui.ctx(), popup_id);
            let cur = self.draft.color.clone();
            let r = color_button(ui, &cur, open).on_hover_text(crate::colorpick::color_text(&cur));
            let palettes = std::mem::take(&mut self.palettes);
            let mut eyedrop = false;
            egui::Popup::menu(&r)
                .id(popup_id)
                .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                .show(|ui| {
                    ui.set_width(theme::PICKER_WIDTH);
                    let o = self.picker.show(ui, "contorno", &cur, &palettes);
                    if let Some(c) = o.color {
                        self.draft.color = c;
                    }
                    if o.eyedrop {
                        eyedrop = true;
                        ui.close();
                    }
                });
            self.palettes = palettes;
            if eyedrop {
                self.eye.start();
            }
            ui.label(tr("Transparência", "Transparency"));
            let mut tp = ((1.0 - self.draft.opacity) * 100.0).round();
            if ui
                .add(egui::Slider::new(&mut tp, 0.0..=100.0).suffix(" %"))
                .changed()
            {
                self.draft.opacity = 1.0 - tp / 100.0;
            }
        });
        ui.horizontal(|ui| {
            ui.label(tr("Espessura", "Width"));
            let (uname, k) = UNITS[self.unit];
            let hair = self.draft.width <= 0.0;
            let mut v = self.draft.width / k;
            let decimals = if k >= 10_000.0 { 4 } else { 3 };
            if ui
                .add_enabled(
                    !hair,
                    egui::DragValue::new(&mut v)
                        .range(0.0..=100_000.0)
                        .speed(0.01 * (1_000.0 / k).max(0.01))
                        .max_decimals(decimals),
                )
                .changed()
            {
                self.draft.width = (v * k).max(0.0);
                self.restyle();
            }
            egui::ComboBox::from_id_salt("contorno-unidade")
                .selected_text(uname)
                .width(theme::UNIT_COMBO_WIDTH)
                .show_ui(ui, |ui| {
                    for (i, (n, _)) in UNITS.iter().enumerate() {
                        ui.selectable_value(&mut self.unit, i, *n);
                    }
                });
            ui.menu_button(tr("Prontas", "Presets"), |ui| {
                for pt in WIDTH_PRESETS {
                    let label = if pt == 0.0 {
                        tr("Fio de cabelo", "Hairline").to_string()
                    } else {
                        format!("{} pt", fmt_num(pt))
                    };
                    if ui.button(label).clicked() {
                        self.draft.width = pt * graphdraw_core::units::UM_PER_PT;
                        self.restyle();
                        ui.close();
                    }
                }
            })
            .response
            .on_hover_text(tr("Espessuras prontas", "Preset widths"));
            let mut h = hair;
            if ui
                .checkbox(&mut h, tr("Fio de cabelo", "Hairline"))
                .on_hover_text(tr(
                    "A linha mais fina que o dispositivo consegue imprimir",
                    "The thinnest line the output device can print",
                ))
                .changed()
            {
                self.draft.width = if h { 0.0 } else { 200.0 };
                self.restyle();
            }
        });
        Self::heading(ui, tr("Estilo da linha", "Line style"));
        ui.horizontal(|ui| {
            let name = match self.style {
                Style::Builtin(i) => tr(DASHES[i].0, DASHES[i].1).to_string(),
                Style::User(i) => self.styles.get(i).map_or(String::new(), |s| s.name.clone()),
                Style::Custom => tr("Personalizado", "Custom").to_string(),
            };
            let mut pick = None;
            egui::ComboBox::from_id_salt("contorno-estilo")
                .selected_text(name)
                .width(theme::LINE_STYLE_COMBO_WIDTH)
                .height(theme::LINE_STYLE_LIST_HEIGHT)
                .show_ui(ui, |ui| {
                    for (i, d) in DASHES.iter().enumerate() {
                        ui.horizontal(|ui| {
                            pattern_sample(ui, d.2);
                            if ui
                                .selectable_label(self.style == Style::Builtin(i), tr(d.0, d.1))
                                .clicked()
                            {
                                pick = Some(Style::Builtin(i));
                            }
                        });
                    }
                    if !self.styles.is_empty() {
                        ui.separator();
                    }
                    for (i, s) in self.styles.iter().enumerate() {
                        ui.horizontal(|ui| {
                            pattern_sample(ui, &s.pattern);
                            if ui
                                .selectable_label(self.style == Style::User(i), &s.name)
                                .clicked()
                            {
                                pick = Some(Style::User(i));
                            }
                        });
                    }
                });
            if let Some(p) = pick {
                self.style = p;
                self.restyle();
                if p == Style::Builtin(3) {
                    self.draft.cap = "round".into();
                }
            }
            if ui
                .button(tr("Editar…", "Edit…"))
                .on_hover_text(tr(
                    "Editor de estilo de linha: monte o ritmo de traços e espaços",
                    "Line style editor: build the rhythm of dashes and gaps",
                ))
                .clicked()
            {
                let u = dash_unit(self.draft.width);
                let p: Vec<f64> = self
                    .draft
                    .dash
                    .as_ref()
                    .map(|d| d.iter().map(|v| v / u).collect())
                    .unwrap_or_default();
                self.editor = Some(DashEditor {
                    cells: pattern_to_cells(&p),
                    name: match self.style {
                        Style::User(i) => {
                            self.styles.get(i).map_or(String::new(), |s| s.name.clone())
                        }
                        _ => String::new(),
                    },
                });
            }
        });
        ui.horizontal(|ui| {
            ui.label(tr("Nos cantos", "At corners"));
            let dashed = self.draft.dash.is_some();
            for (k, icon, tip) in [
                (
                    "standard",
                    "dash-standard",
                    tr(
                        "Tracejado padrão: segue o caminho sem compensar os cantos",
                        "Standard dashes: follow the path without corner compensation",
                    ),
                ),
                (
                    "align",
                    "dash-align",
                    tr(
                        "Alinhar tracejados: um traço inteiro passa por cada canto",
                        "Align dashes: a whole dash goes through every corner",
                    ),
                ),
                (
                    "fixed",
                    "dash-fixed",
                    tr(
                        "Tracejados fixos: recomeça em cada canto, com comprimento exato",
                        "Fixed dashes: restarts at every corner, with exact length",
                    ),
                ),
            ] {
                if icons::button(ui, icon, tip, self.draft.dash_corners == k, dashed).clicked() {
                    self.draft.dash_corners = k.into();
                }
            }
        });
        Self::heading(ui, tr("Cantos", "Corners"));
        ui.horizontal(|ui| {
            icon_choice(
                ui,
                &mut self.draft.join,
                &[
                    (
                        "miter",
                        "join-miter",
                        tr("Canto agudo (meia-esquadria)", "Mitered corner"),
                    ),
                    (
                        "round",
                        "join-round",
                        tr("Canto arredondado", "Round corner"),
                    ),
                    (
                        "bevel",
                        "join-bevel",
                        tr("Canto chanfrado", "Beveled corner"),
                    ),
                ],
            );
            ui.add_enabled_ui(self.draft.join == "miter", |ui| {
                ui.label(tr("Limite", "Limit"));
                ui.add(
                    egui::DragValue::new(&mut self.draft.miter_limit)
                        .range(1.0..=100.0)
                        .speed(0.1),
                )
                .on_hover_text(tr(
                    "Cantos mais fechados que isso viram chanfro, para a ponta não crescer demais",
                    "Sharper corners than this become bevels so the tip does not grow too long",
                ));
            });
        });
        Self::heading(ui, tr("Pontas da linha", "Line caps"));
        icon_choice(
            ui,
            &mut self.draft.cap,
            &[
                (
                    "butt",
                    "cap-butt",
                    tr("Reta: termina no nó", "Flat: ends at the node"),
                ),
                (
                    "round",
                    "cap-round",
                    tr(
                        "Redonda: meia-volta além do nó",
                        "Round: half circle past the node",
                    ),
                ),
                (
                    "square",
                    "cap-square",
                    tr(
                        "Estendida: meia espessura além do nó",
                        "Extended: half the width past the node",
                    ),
                ),
            ],
        );
        Self::heading(ui, tr("Posição", "Position"));
        icon_choice(
            ui,
            &mut self.draft.position,
            &[
                (
                    "outside",
                    "pos-outside",
                    tr(
                        "Por fora: a espessura toda para fora da forma",
                        "Outside: the whole width outside the shape",
                    ),
                ),
                (
                    "center",
                    "pos-center",
                    tr(
                        "Centrada: metade para cada lado",
                        "Centered: half on each side",
                    ),
                ),
                (
                    "inside",
                    "pos-inside",
                    tr(
                        "Por dentro: não passa da caixa da forma",
                        "Inside: does not go past the shape's box",
                    ),
                ),
            ],
        );
        ui.weak(tr(
            "Curvas abertas sempre usam centrada.",
            "Open curves always use centered.",
        ));
    }

    /// Coluna direita: pontas de seta, caneta caligráfica, largura variável e prévia.
    fn right(&mut self, ui: &mut egui::Ui) {
        Self::heading(ui, tr("Pontas de seta", "Arrowheads"));
        ui.horizontal(|ui| {
            let mut start = self.draft.start_arrow.clone();
            self.arrow_menu(ui, "seta-inicio", true, &mut start);
            self.draft.start_arrow = start;
            let mut end = self.draft.end_arrow.clone();
            self.arrow_menu(ui, "seta-fim", false, &mut end);
            self.draft.end_arrow = end;
            let r = ui.button(tr("Opções…", "Options…"));
            egui::Popup::menu(&r)
                .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                .show(|ui| {
                    if ui
                        .button(tr("Trocar início e fim", "Swap start and end"))
                        .clicked()
                    {
                        std::mem::swap(&mut self.draft.start_arrow, &mut self.draft.end_arrow);
                    }
                    ui.separator();
                    for (label, a) in [
                        (tr("Início", "Start"), &mut self.draft.start_arrow),
                        (tr("Fim", "End"), &mut self.draft.end_arrow),
                    ] {
                        ui.strong(label);
                        match a {
                            None => {
                                ui.weak(tr("Nenhuma", "None"));
                            }
                            Some(x) => arrow_props(ui, label, x),
                        }
                    }
                });
        });
        ui.weak(tr(
            "Só em curvas abertas; o tamanho acompanha a espessura.",
            "Open curves only; the size follows the width.",
        ));
        Self::heading(ui, tr("Caneta caligráfica", "Calligraphic pen"));
        ui.horizontal_top(|ui| {
            self.nib_view(ui);
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.label(tr("Alongamento", "Stretch"));
                    let mut pct = (self.draft.nib_stretch * 100.0).round();
                    if ui
                        .add(
                            egui::DragValue::new(&mut pct)
                                .range(1.0..=100.0)
                                .suffix(" %"),
                        )
                        .on_hover_text(tr(
                            "100% = bico redondo (largura igual em toda direção)",
                            "100% = round nib (same width in every direction)",
                        ))
                        .changed()
                    {
                        self.draft.nib_stretch = pct / 100.0;
                    }
                });
                ui.horizontal(|ui| {
                    ui.label(tr("Ângulo", "Angle"));
                    ui.add_enabled(
                        self.draft.nib_stretch < 0.999,
                        egui::DragValue::new(&mut self.draft.nib_angle)
                            .range(0.0..=360.0)
                            .suffix("°"),
                    );
                });
                if ui
                    .button(tr("Padrão", "Default"))
                    .on_hover_text(tr("Bico redondo, traço uniforme", "Round nib, even stroke"))
                    .clicked()
                {
                    self.draft.nib_stretch = 1.0;
                    self.draft.nib_angle = 0.0;
                }
            });
        });
        Self::heading(ui, tr("Largura variável", "Variable width"));
        ui.horizontal_wrapped(|ui| {
            if ui
                .selectable_label(self.draft.widths.is_empty(), tr("Uniforme", "Uniform"))
                .clicked()
            {
                self.draft.widths.clear();
            }
            for name in graphdraw_core::render::varwidth::PRESETS {
                let nodes = graphdraw_core::render::varwidth::preset(name);
                if ui
                    .selectable_label(self.draft.widths == nodes, width_label(name))
                    .clicked()
                {
                    self.draft.widths = nodes;
                }
            }
        });
        Self::heading(ui, tr("Prévia", "Preview"));
        self.preview(ui);
    }

    /// Opções de comportamento (faixa de baixo).
    fn options(&mut self, ui: &mut egui::Ui) {
        let s = &mut self.draft;
        ui.horizontal(|ui| {
            icons::button(ui, "behind-fill", "", s.behind_fill, false);
            ui.checkbox(&mut s.behind_fill, tr("Atrás do preenchimento", "Behind fill")).on_hover_text(tr(
                "O contorno fica atrás do preenchimento: só a metade de fora aparece, sem engolir o miolo de letras e formas pequenas",
                "The outline goes behind the fill: only the outer half shows, without swallowing the inside of letters and small shapes",
            ));
            ui.add_space(theme::SPACE_400);
            icons::button(ui, "scale-stroke", "", s.scale_with_object, false);
            ui.checkbox(&mut s.scale_with_object, tr("Escalar com o objeto", "Scale with object")).on_hover_text(tr(
                "Marcado, a espessura acompanha a escala do objeto; desmarcado, 2 mm continua 2 mm",
                "Checked, the width follows the object's scale; unchecked, 2 mm stays 2 mm",
            ));
            ui.add_space(theme::SPACE_400);
            ui.checkbox(&mut s.overprint, tr("Sobreimprimir contorno", "Overprint outline")).on_hover_text(tr(
                "O contorno imprime por cima das outras tintas, sem vazar o fundo (evita frestas de registro, GD-PRE-08)",
                "The outline prints over the other inks without knocking out (avoids registration gaps, GD-PRE-08)",
            ));
        });
    }

    fn footer(&mut self, ui: &mut egui::Ui) -> Option<bool> {
        let mut out = None;
        widgets::footer(ui, |ui| {
            if ui.add(widgets::primary_button(tr("OK", "OK"))).clicked() {
                out = Some(true);
            }
            if ui
                .add(widgets::secondary_button(tr("Cancelar", "Cancel")))
                .clicked()
            {
                out = Some(false);
            }
            let help = ui.add(widgets::secondary_button(tr("Ajuda", "Help")));
            egui::Popup::menu(&help)
                .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                .show(|ui| {
                    ui.set_max_width(theme::PICKER_WIDTH);
                    ui.label(tr(
                        "Esquerda: cor e transparência, espessura (com unidade e espessuras prontas), estilo da linha (Editar… monta o seu tracejado; Nos cantos decide como o tracejado passa pelos cantos), cantos, pontas e posição. Direita: pontas de seta (Opções troca início e fim e ajusta tamanho, giro e afastamento), caneta caligráfica (arraste a ponta da elipse para girar e a lateral para achatar), largura variável e a prévia. Embaixo: atrás do preenchimento, escalar com o objeto e sobreimprimir. Sem seleção, vale para os novos objetos.",
                        "Left: color and transparency, width (with unit and preset widths), line style (Edit… builds your own dashes; At corners decides how dashes go through corners), corners, caps and position. Right: arrowheads (Options swaps start and end and sets size, rotation and offset), calligraphic pen (drag the ellipse tip to rotate and its side to flatten), variable width and the preview. Bottom: behind fill, scale with object and overprint. With no selection, it applies to new objects.",
                    ));
                });
        });
        if ui.input(|i| i.key_pressed(egui::Key::Escape))
            && !egui::Popup::is_any_open(ui.ctx())
            && self.editor.is_none()
        {
            out = Some(false);
        }
        out
    }

    /// Miniatura de ponta de seta (com cache).
    fn thumb(
        &mut self,
        ctx: &egui::Context,
        shape: Option<&str>,
        start: bool,
    ) -> egui::TextureHandle {
        let key = (shape.unwrap_or("").to_string(), start);
        self.thumbs
            .entry(key)
            .or_insert_with(|| {
                ctx.load_texture(
                    "ponta-seta",
                    crate::dialogs::arrow_thumb(shape, start),
                    egui::TextureOptions::LINEAR,
                )
            })
            .clone()
    }

    /// Botão com a miniatura da ponta; o balão lista as formas.
    fn arrow_menu(&mut self, ui: &mut egui::Ui, id: &str, start: bool, a: &mut Option<Arrow>) {
        let size = Vec2::new(theme::ARROW_THUMB_W, theme::ARROW_THUMB_H);
        let cur = self.thumb(ui.ctx(), a.as_ref().map(|x| x.shape.as_str()), start);
        let tip = if start {
            tr("Ponta no início", "Start arrowhead")
        } else {
            tr("Ponta no fim", "End arrowhead")
        };
        let r = ui
            .add(egui::Button::image(egui::Image::new((cur.id(), size))))
            .on_hover_text(format!(
                "{tip}: {}",
                a.as_ref()
                    .map_or(tr("Nenhuma", "None"), |x| arrow_label(&x.shape))
            ));
        let mut choices: Vec<(Option<&str>, egui::TextureHandle)> =
            vec![(None, self.thumb(ui.ctx(), None, start))];
        for s in graphdraw_core::render::arrows::SHAPES {
            choices.push((Some(s), self.thumb(ui.ctx(), Some(s), start)));
        }
        egui::Popup::menu(&r)
            .id(ui.make_persistent_id(id))
            .close_behavior(egui::PopupCloseBehavior::CloseOnClick)
            .show(|ui| {
                for (shape, tex) in choices {
                    let on = a.as_ref().map(|x| x.shape.as_str()) == shape;
                    let label = shape.map_or(tr("Nenhuma", "None"), arrow_label);
                    if ui
                        .add(
                            egui::Button::image_and_text(egui::Image::new((tex.id(), size)), label)
                                .selected(on),
                        )
                        .clicked()
                    {
                        match (shape, a.as_mut()) {
                            (None, _) => *a = None,
                            (Some(s), Some(x)) => x.shape = s.into(),
                            (Some(s), None) => *a = Some(Arrow::new(s)),
                        }
                    }
                }
            });
    }

    /// Visor do bico: elipse que se arrasta pela ponta (gira) ou pela
    /// lateral (achata).
    fn nib_view(&mut self, ui: &mut egui::Ui) {
        let side = theme::NIB_VIEW;
        let (rect, resp) = ui.allocate_exact_size(Vec2::splat(side), egui::Sense::click_and_drag());
        let pal = theme::palette(ui);
        let p = ui.painter();
        p.rect_filled(rect, theme::RADIUS_XS, theme::PAGE_PAPER);
        p.rect_stroke(
            rect,
            theme::RADIUS_XS,
            theme::border_default(ui),
            egui::StrokeKind::Inside,
        );
        let c = rect.center();
        let major = side / 2.0 - theme::SPACE_300;
        let s = self.draft.nib_stretch.clamp(0.01, 1.0) as f32;
        let a = (self.draft.nib_angle as f32).to_radians();
        let (u, v) = (Vec2::new(a.cos(), -a.sin()), Vec2::new(a.sin(), a.cos()));
        let pts: Vec<egui::Pos2> = (0..64)
            .map(|k| {
                let t = k as f32 / 64.0 * std::f32::consts::TAU;
                c + u * (t.cos() * major) + v * (t.sin() * major * s)
            })
            .collect();
        p.add(egui::Shape::convex_polygon(
            pts,
            Color32::from_gray(40),
            egui::Stroke::new(theme::BORDER_WIDTH, pal.fg_muted),
        ));
        let tip = c + u * major;
        let side_h = c + v * (major * s);
        for (q, on) in [
            (tip, self.nib_drag == Some(true)),
            (side_h, self.nib_drag == Some(false)),
        ] {
            let r = egui::Rect::from_center_size(q, Vec2::splat(theme::HANDLE_SIZE));
            p.rect_filled(
                r,
                theme::RADIUS_XS,
                if on {
                    theme::ACCENT_PRIMARY
                } else {
                    theme::HANDLE_FILL
                },
            );
            p.rect_stroke(
                r,
                theme::RADIUS_XS,
                egui::Stroke::new(theme::BORDER_WIDTH, theme::ACCENT_PRIMARY),
                egui::StrokeKind::Middle,
            );
        }
        if resp.drag_started()
            && let Some(q) = ui.input(|i| i.pointer.press_origin())
        {
            // Mais perto da ponta: gira; da lateral (ou de dentro): achata.
            self.nib_drag = Some((q - tip).length() < (q - side_h).length());
        }
        if resp.dragged()
            && let (Some(q), Some(rotate)) = (resp.interact_pointer_pos(), self.nib_drag)
        {
            let d = q - c;
            if rotate {
                if d.length() > 2.0 {
                    let deg = ((-d.y).atan2(d.x) as f64).to_degrees().rem_euclid(360.0);
                    self.draft.nib_angle = deg.round();
                }
            } else {
                let along = d.dot(v).abs() / major;
                self.draft.nib_stretch = (along as f64).clamp(0.01, 1.0);
            }
        }
        if resp.drag_stopped() {
            self.nib_drag = None;
        }
        resp.on_hover_text(tr(
            "Arraste o quadrado da ponta para girar o bico e o da lateral para achatá-lo",
            "Drag the tip square to rotate the nib and the side one to flatten it",
        ));
    }

    fn preview(&mut self, ui: &mut egui::Ui) {
        let key = (self.draft.clone(), self.enabled);
        if self.preview.as_ref().is_none_or(|(k, _)| *k != key) {
            let img = stroke_preview(self.result().as_ref());
            match &mut self.preview {
                Some((k, tex)) => {
                    tex.set(img, egui::TextureOptions::LINEAR);
                    *k = key;
                }
                None => {
                    let tex =
                        ui.ctx()
                            .load_texture("previa-contorno", img, egui::TextureOptions::LINEAR);
                    self.preview = Some((key, tex));
                }
            }
        }
        if let Some((_, tex)) = &self.preview {
            let size = tex.size_vec2() / ui.ctx().pixels_per_point();
            let w = ui.available_width().min(size.x);
            ui.add(egui::Image::new((
                tex.id(),
                Vec2::new(w, size.y * w / size.x),
            )));
        }
    }

    fn apply(&self, doc: &mut DocState, defaults: &mut Defaults) {
        let result = self.result();
        if self.for_defaults {
            defaults.stroke = result;
            return;
        }
        if result == self.initial {
            return;
        }
        let sel = doc.selection.clone();
        match (result, &self.initial) {
            (None, _) => ops::set_stroke(&mut doc.editor, &sel, None),
            (Some(new), None) => ops::set_stroke(&mut doc.editor, &sel, Some(new)),
            (Some(new), Some(old)) => {
                let old = old.clone();
                ops::edit_stroke(&mut doc.editor, &sel, move |s| merge_changes(s, &old, &new));
            }
        }
    }
}

/// Grupo de botões de ícone mutuamente exclusivos.
fn icon_choice(ui: &mut egui::Ui, value: &mut String, options: &[(&str, &str, &str)]) {
    ui.horizontal(|ui| {
        for (key, icon, tip) in options {
            if icons::button(ui, icon, tip, value == key, true).clicked() {
                *value = (*key).to_string();
            }
        }
    });
}

/// Tamanho, giro, espelho e afastamento de uma ponta.
fn arrow_props(ui: &mut egui::Ui, id: &str, x: &mut Arrow) {
    ui.horizontal(|ui| {
        ui.label(arrow_label(&x.shape));
        let mut pct = (x.scale * 100.0).round();
        if ui
            .add(
                egui::DragValue::new(&mut pct)
                    .range(5.0..=5000.0)
                    .suffix(" %"),
            )
            .on_hover_text(tr("Tamanho", "Size"))
            .changed()
        {
            x.scale = pct / 100.0;
        }
        ui.add(
            egui::DragValue::new(&mut x.rotation)
                .range(-360.0..=360.0)
                .suffix("°"),
        )
        .on_hover_text(tr("Giro", "Rotation"));
        ui.checkbox(&mut x.mirror, tr("Espelhar", "Mirror"));
        if let Some(v) = widgets::labeled_length(
            ui,
            tr("Afast.", "Offset"),
            &format!("{id}-afast"),
            x.offset,
            None,
        ) {
            x.offset = v;
        }
    });
}

fn fmt_num(v: f64) -> String {
    let s = format!("{v:.2}");
    s.trim_end_matches('0')
        .trim_end_matches('.')
        .replace('.', ",")
}

/// Amostra de um padrão (múltiplos) na lista de estilos.
fn pattern_sample(ui: &mut egui::Ui, p: &[f64]) {
    let (rect, _) =
        ui.allocate_exact_size(Vec2::new(theme::DASH_SAMPLE_W, 12.0), egui::Sense::hover());
    let color = ui.visuals().text_color();
    let y = rect.center().y;
    let stroke = egui::Stroke::new(2.0, color);
    if p.is_empty() {
        ui.painter().line_segment(
            [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
            stroke,
        );
        return;
    }
    let unit = 2.0;
    let (mut x, mut i) = (rect.left(), 0);
    while x < rect.right() {
        let len = (p[i % p.len()] as f32 * unit).max(1.0);
        if i % 2 == 0 {
            ui.painter().line_segment(
                [egui::pos2(x, y), egui::pos2((x + len).min(rect.right()), y)],
                stroke,
            );
        }
        x += len;
        i += 1;
    }
}

enum EditorOut {
    None,
    Close,
    Use(Vec<f64>),
    Add(String, Vec<f64>),
    Replace(usize, Vec<f64>),
}

/// Editor de estilo de linha: régua de células (clique troca traço/espaço),
/// número de células, prévia, nome; Usar, Adicionar, Substituir, Fechar.
fn dash_editor(
    ctx: &egui::Context,
    ed: &mut DashEditor,
    styles: &[LineStyle],
    cur: Style,
) -> EditorOut {
    let mut out = EditorOut::None;
    widgets::modal(ctx, egui::Id::unique("editor-estilo-linha")).show(ctx, |ui| {
        ui.heading(tr("Editar estilo de linha", "Edit line style"));
        ui.add_space(theme::SPACE_200);
        ui.weak(tr(
            "Cada célula vale uma espessura de linha. Clique para trocar traço (cheia) e espaço (vazia).",
            "Each cell is one line width. Click to swap dash (filled) and gap (empty).",
        ));
        let cell = theme::DASH_CELL;
        let n = ed.cells.len();
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(cell * n as f32, cell), egui::Sense::click());
        let pal = theme::palette(ui);
        for (i, c) in ed.cells.iter().enumerate() {
            let r = egui::Rect::from_min_size(rect.min + Vec2::new(i as f32 * cell, 0.0), Vec2::splat(cell)).shrink(1.0);
            ui.painter().rect_filled(r, 0.0, if *c { pal.fg_strong } else { theme::PAGE_PAPER });
            ui.painter().rect_stroke(r, 0.0, egui::Stroke::new(theme::BORDER_WIDTH, pal.fg_muted), egui::StrokeKind::Inside);
        }
        if resp.clicked()
            && let Some(q) = resp.interact_pointer_pos()
        {
            let i = ((q.x - rect.left()) / cell) as usize;
            if let Some(c) = ed.cells.get_mut(i) {
                *c = !*c;
            }
        }
        ui.horizontal(|ui| {
            ui.label(tr("Células", "Cells"));
            let mut k = ed.cells.len();
            if ui.add(egui::DragValue::new(&mut k).range(2..=48)).changed() {
                ed.cells.resize(k, false);
            }
        });
        let p = cells_to_pattern(&ed.cells);
        ui.horizontal(|ui| {
            ui.label(tr("Prévia", "Preview"));
            pattern_sample(ui, &p);
        });
        ui.horizontal(|ui| {
            ui.label(tr("Nome", "Name"));
            ui.add(egui::TextEdit::singleline(&mut ed.name).desired_width(theme::FONT_COMBO_WIDTH));
        });
        ui.add_space(theme::SPACE_300);
        widgets::footer(ui, |ui| {
            if ui.add(widgets::primary_button(tr("Usar", "Use"))).clicked() {
                out = EditorOut::Use(p.clone());
            }
            let replace = match cur {
                Style::User(i) if i < styles.len() => Some(i),
                _ => None,
            };
            if ui
                .add_enabled(replace.is_some(), widgets::secondary_button(tr("Substituir", "Replace")))
                .on_disabled_hover_text(tr("Escolha antes um estilo seu na lista", "Pick one of your styles in the list first"))
                .clicked()
                && let Some(i) = replace
            {
                out = EditorOut::Replace(i, p.clone());
            }
            let name = ed.name.trim().to_string();
            if ui
                .add_enabled(!name.is_empty() && !p.is_empty(), widgets::secondary_button(tr("Adicionar", "Add")))
                .on_disabled_hover_text(tr("Dê um nome e marque ao menos um traço e um espaço", "Give a name and set at least one dash and one gap"))
                .clicked()
            {
                out = EditorOut::Add(name, p.clone());
            }
            if ui.add(widgets::secondary_button(tr("Fechar", "Close"))).clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                out = EditorOut::Close;
            }
        });
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use graphdraw_core::model::Color;

    #[test]
    fn cells_and_patterns_round_trip() {
        let c = |s: &str| s.chars().map(|c| c == '1').collect::<Vec<_>>();
        assert_eq!(cells_to_pattern(&c("111100")), vec![4.0, 2.0]);
        assert_eq!(cells_to_pattern(&c("0111001")), vec![3.0, 2.0, 1.0, 1.0]);
        assert_eq!(
            cells_to_pattern(&c("1110101100")),
            vec![3.0, 1.0, 1.0, 1.0, 2.0, 2.0]
        );
        assert!(cells_to_pattern(&c("0000")).is_empty());
        assert!(
            cells_to_pattern(&c("1111")).is_empty(),
            "sem espaço = sólido"
        );
        assert_eq!(pattern_to_cells(&[4.0, 2.0]), c("111100"));
        assert_eq!(
            cells_to_pattern(&pattern_to_cells(&[5.0, 2.0, 1.0, 2.0])),
            vec![5.0, 2.0, 1.0, 2.0]
        );
    }

    fn run(d: &mut StrokeDialog) {
        let ctx = crate::theme::test_ctx();
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(1366.0, 768.0));
        let mut doc = DocState::new_blank();
        let mut defaults = Defaults::default();
        for k in 0..30 {
            let input = egui::RawInput {
                screen_rect: Some(screen),
                time: Some(k as f64 / 30.0),
                ..Default::default()
            };
            let mut out = ctx.run_ui(input, |ui| {
                d.show(ui.ctx(), &mut doc, &mut defaults);
            });
            out.textures_delta.clear();
        }
        assert!(
            screen.contains_rect(d.rect) && d.rect.height() < 740.0,
            "{:?}",
            d.rect
        );
    }

    /// A janela cabe na tela com todas as partes; espessura nova reaplica o
    /// tracejado escolhido; o editor monta um estilo.
    #[test]
    fn outline_pen_fits_and_restyles() {
        let mut s = Stroke::new(500.0, Color::rgb(0.8, 0.1, 0.1));
        s.dash = dash_for(1, 500.0);
        s.end_arrow = Some(Arrow::new("triangle"));
        let mut d = StrokeDialog::new(Some(s), false);
        assert_eq!(d.style, Style::Builtin(1));
        run(&mut d);
        d.draft.width = 1_000.0;
        d.restyle();
        assert_eq!(d.draft.dash, dash_for(1, 1_000.0));
        d.editor = Some(DashEditor {
            cells: pattern_to_cells(&[3.0, 1.0]),
            name: "Meu".into(),
        });
        run(&mut d);
        assert!(d.editor.is_some(), "editor continua aberto");
    }

    #[test]
    #[ignore]
    fn shot_outline_pen() {
        let mut s = Stroke::new(1_500.0, Color::cmyk(0.0, 0.8, 1.0, 0.0));
        s.dash = dash_for(4, 1_500.0);
        s.end_arrow = Some(Arrow::new("triangle"));
        s.nib_stretch = 0.4;
        s.nib_angle = 30.0;
        let mut d = StrokeDialog::new(Some(s), false);
        let ctx = crate::theme::test_ctx();
        let mut shot = crate::uishot::Shot::default();
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(1366.0, 768.0));
        let mut doc = DocState::new_blank();
        let mut defaults = Defaults::default();
        for k in 0..40 {
            let input = egui::RawInput {
                screen_rect: Some(screen),
                time: Some(k as f64 / 30.0),
                ..Default::default()
            };
            let mut out = ctx.run_ui(input, |ui| {
                d.show(ui.ctx(), &mut doc, &mut defaults);
            });
            if k == 39 {
                shot.save(&ctx, out, "contorno");
            } else {
                shot.textures(&mut out);
            }
        }
    }
}
