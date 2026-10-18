//! Janela de edição do preenchimento (GD-FIL-01..06, doc 04): em cima, a
//! barra de modos (nenhum, uniforme, gradiente, padrão vetorial, bitmap,
//! duas cores, textura; PostScript fora da 1.0); no gradiente, a sub-barra
//! de tipos, a rampa com os nós e os pontos médios, a linha do nó escolhido
//! e a matriz de transformação; embaixo, a amostra original × nova e
//! OK/Cancelar/Ajuda. Nada muda no documento até o OK.

use std::sync::{Arc, Mutex};

use eframe::egui::{self, Color32, Vec2};
use graphdraw_core::edit::ops;
use graphdraw_core::model::{Color, Fill, GradientStop, Pattern, Texture};
use graphdraw_core::render::raster;
use serde_json::Map;

use crate::canvas::Defaults;
use crate::colorpick::{ColorPicker, PaletteList, color_button};
use crate::doc::DocState;
use crate::i18n::tr;
use crate::icons;
use crate::paletteui::SwatchDrop;
use crate::theme;
use crate::widgets::{self, to_color32};

/// Modo (motor) do preenchimento na barra de cima.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FillMode {
    None,
    Solid,
    Gradient,
    Vector,
    Bitmap,
    TwoColor,
    Texture,
}

impl FillMode {
    fn pattern_kind(self) -> Option<&'static str> {
        match self {
            FillMode::Vector => Some("vector"),
            FillMode::Bitmap => Some("bitmap"),
            FillMode::TwoColor => Some("two-color"),
            FillMode::Texture => Some("texture"),
            _ => None,
        }
    }
}

/// Tipos de gradiente da sub-barra: chave, ícone e nome.
const KINDS: [(&str, &str); 5] = [
    ("linear", "fill-linear"),
    ("radial", "fill-radial"),
    ("elliptic", "fill-elliptic"),
    ("conic", "fill-conic"),
    ("rectangular", "fill-rectangular"),
];

fn kind_label(k: &str) -> &'static str {
    match k {
        "radial" => tr("Radial", "Radial"),
        "elliptic" => tr("Elíptico", "Elliptical"),
        "conic" => tr("Cônico", "Conical"),
        "rectangular" => tr("Retangular", "Rectangular"),
        _ => tr("Linear", "Linear"),
    }
}

/// Objetos copiados (Ctrl+C) que viram o ladrilho do padrão vetorial.
static TILE_SOURCE: Mutex<Vec<graphdraw_core::model::Object>> = Mutex::new(Vec::new());

/// O app avisa o que foi copiado (fonte do ladrilho vetorial, GD-FIL-04).
pub fn set_tile_source(objs: Vec<graphdraw_core::model::Object>) {
    if let Ok(mut t) = TILE_SOURCE.lock() {
        *t = objs;
    }
}

/// Predefinições de duas cores (8 × 8, linha de cima primeiro).
pub fn two_color_presets() -> Vec<(&'static str, Vec<String>)> {
    let rows = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let diag: Vec<String> = (0..8)
        .map(|i| {
            (0..8)
                .map(|j| if (j + 8 - i) % 8 < 2 { '1' } else { '0' })
                .collect()
        })
        .collect();
    vec![
        (
            tr("Listras verticais", "Vertical stripes"),
            rows(&["11110000"; 8]),
        ),
        (
            tr("Listras horizontais", "Horizontal stripes"),
            rows(&[
                "11111111", "11111111", "11111111", "11111111", "00000000", "00000000", "00000000",
                "00000000",
            ]),
        ),
        (
            tr("Xadrez", "Checker"),
            rows(&[
                "11110000", "11110000", "11110000", "11110000", "00001111", "00001111", "00001111",
                "00001111",
            ]),
        ),
        (
            tr("Pontos", "Dots"),
            rows(&[
                "00000000", "00000000", "00000000", "00011000", "00011000", "00000000", "00000000",
                "00000000",
            ]),
        ),
        (
            tr("Grade", "Grid"),
            rows(&[
                "11111111", "10000000", "10000000", "10000000", "10000000", "10000000", "10000000",
                "10000000",
            ]),
        ),
        (tr("Diagonal", "Diagonal"), diag),
    ]
}

/// Parte da rampa sob o arrasto: um nó ou o ponto médio depois dele.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ramp {
    Node(usize),
    Mid(usize),
}

/// A cor que o conta-gotas vai trocar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Target {
    Solid,
    Stop,
    Fg,
    Bg,
}

/// Conta-gotas da janela: esconde a janela, fotografa a tela, mostra a foto
/// congelada com a lupa e espera o clique.
enum Eye {
    Off,
    Hide,
    Wait(Arc<Mutex<Option<Arc<egui::ColorImage>>>>, u32),
    Pick(egui::TextureHandle, Arc<egui::ColorImage>),
}

pub struct FillDialog {
    pub(crate) initial: Option<Fill>,
    pub(crate) mode: FillMode,
    /// Gradiente: tipo, paradas (ordenadas), ângulo, repetição, centro,
    /// tamanho, faixas, suavizar e rota de cor.
    pub(crate) spec: ops::GradientSpec,
    /// Faixas digitadas quando o cadeado está aberto (contínuo).
    steps_n: u32,
    pub(crate) sel_stop: usize,
    pub(crate) color: Color,
    pub(crate) opacity: f64,
    pub(crate) overprint: bool,
    pub(crate) pattern: Pattern,
    /// O preenchimento atual é de um tipo que esta janela não edita.
    other_kind: bool,
    pub(crate) changed: bool,
    for_defaults: bool,
    pub(crate) rect: egui::Rect,
    /// Caixa local do objeto: lê de volta centro e tamanho do gradiente.
    box_hint: Option<kurbo::Rect>,
    pub(crate) palettes: PaletteList,
    picker: ColorPicker,
    eye: Eye,
    eye_target: Target,
    drag: Option<Ramp>,
    /// Faixa da rampa no último quadro (testes).
    pub(crate) ramp_bar: egui::Rect,
    pub(crate) sample_tex: Option<(Option<Fill>, egui::TextureHandle)>,
    before_tex: Option<egui::TextureHandle>,
    pub(crate) bar_tex: Option<(Fill, egui::TextureHandle)>,
}

pub(crate) fn stop(offset: f64, color: Color) -> GradientStop {
    GradientStop {
        offset,
        color,
        opacity: 1.0,
        midpoint: 0.5,
        extra: Map::new(),
    }
}

impl FillDialog {
    pub fn is_for_defaults(&self) -> bool {
        self.for_defaults
    }

    pub fn new(
        initial: Option<Fill>,
        for_defaults: bool,
        box_hint: Option<kurbo::Rect>,
    ) -> FillDialog {
        let white = Color::cmyk(0.0, 0.0, 0.0, 0.0);
        let mut d = FillDialog {
            initial: initial.clone(),
            mode: FillMode::None,
            spec: ops::GradientSpec {
                stops: vec![
                    stop(0.0, Color::cmyk(0.0, 0.0, 0.0, 1.0)),
                    stop(1.0, white.clone()),
                ],
                ..Default::default()
            },
            steps_n: 16,
            sel_stop: 0,
            color: white,
            opacity: 1.0,
            overprint: false,
            pattern: Pattern {
                cells: two_color_presets().remove(2).1,
                fg: Some(Color::cmyk(0.0, 0.0, 0.0, 1.0)),
                bg: Some(Color::cmyk(0.0, 0.0, 0.0, 0.0)),
                ..Default::default()
            },
            other_kind: false,
            changed: false,
            for_defaults,
            rect: egui::Rect::NOTHING,
            box_hint,
            palettes: Vec::new(),
            picker: ColorPicker::default(),
            eye: Eye::Off,
            eye_target: Target::Solid,
            drag: None,
            ramp_bar: egui::Rect::NOTHING,
            sample_tex: None,
            before_tex: None,
            bar_tex: None,
        };
        match &initial {
            None | Some(Fill::None) => {}
            Some(Fill::Solid {
                color,
                opacity,
                overprint,
                ..
            }) => {
                d.mode = FillMode::Solid;
                d.color = color.clone();
                d.opacity = *opacity;
                d.overprint = *overprint;
            }
            Some(Fill::Gradient(g)) => {
                let spec = ops::gradient_spec_in(g, box_hint);
                d.mode = FillMode::Gradient;
                let stops = if spec.stops.len() >= 2 {
                    spec.stops.clone()
                } else {
                    d.spec.stops.clone()
                };
                d.spec = ops::GradientSpec { stops, ..spec };
                d.steps_n = d.spec.steps.unwrap_or(16);
                d.sort_stops();
                d.color = d.spec.stops[0].color.clone();
            }
            Some(Fill::Pattern(p)) => {
                d.mode = match p.kind.as_str() {
                    "vector" => FillMode::Vector,
                    "bitmap" => FillMode::Bitmap,
                    "texture" => FillMode::Texture,
                    _ => FillMode::TwoColor,
                };
                d.pattern = (**p).clone();
            }
            Some(_) => d.other_kind = true,
        }
        d
    }

    /// Ordena as paradas mantendo a escolhida.
    fn sort_stops(&mut self) {
        let sel = self.sel_stop.min(self.spec.stops.len().saturating_sub(1));
        let mut v: Vec<(GradientStop, bool)> = self
            .spec
            .stops
            .drain(..)
            .enumerate()
            .map(|(i, s)| (s, i == sel))
            .collect();
        v.sort_by(|a, b| a.0.offset.total_cmp(&b.0.offset));
        self.sel_stop = v.iter().position(|(_, s)| *s).unwrap_or(0);
        self.spec.stops = v.into_iter().map(|(s, _)| s).collect();
    }

    /// Gradiente ao longo de 0..1 (para a rampa e para a cor de um ponto).
    fn unit_gradient(&self) -> graphdraw_core::model::Gradient {
        ops::gradient_for(
            &ops::GradientSpec {
                kind: "linear".into(),
                angle: 0.0,
                spread: "pad".into(),
                center: [0.0, 0.0],
                scale: 1.0,
                ..self.spec.clone()
            },
            kurbo::Rect::new(0.0, 0.0, 1.0, 1.0),
        )
    }

    fn add_stop(&mut self, at: f64, color: Option<Color>) {
        let g = self.unit_gradient();
        let (c, o) = graphdraw_core::render::gradient::color_at(&g, at);
        let mut s = stop(at.clamp(0.0, 1.0), color.unwrap_or(c));
        s.opacity = o;
        self.spec.stops.push(s);
        self.sel_stop = self.spec.stops.len() - 1;
        self.sort_stops();
        self.changed = true;
    }

    fn removable(&self, i: usize) -> bool {
        self.spec.stops.len() > 2 && i > 0 && i + 1 < self.spec.stops.len()
    }

    fn remove_stop(&mut self, i: usize) {
        if self.removable(i) {
            self.spec.stops.remove(i);
            self.sel_stop = i.min(self.spec.stops.len() - 1);
            self.changed = true;
        }
    }

    /// Inverte as paradas (e os pontos médios dos trechos).
    pub(crate) fn reverse(&mut self) {
        let n = self.spec.stops.len();
        let old = self.spec.stops.clone();
        self.spec.stops = (0..n)
            .map(|j| {
                let k = n - 1 - j;
                let mut s = old[k].clone();
                s.offset = 1.0 - s.offset;
                s.midpoint = if k > 0 {
                    1.0 - old[k - 1].midpoint
                } else {
                    0.5
                };
                s
            })
            .collect();
        self.sel_stop = n - 1 - self.sel_stop.min(n - 1);
        self.changed = true;
    }

    /// Preenchimento para a amostra (gradiente ajustado à caixa `b`).
    pub(crate) fn result_in(&self, b: kurbo::Rect) -> Option<Fill> {
        match self.mode {
            FillMode::None => None,
            FillMode::Solid => Some(Fill::Solid {
                color: self.color.clone(),
                opacity: self.opacity,
                overprint: self.overprint,
                extra: match &self.initial {
                    Some(Fill::Solid { extra, .. }) => extra.clone(),
                    _ => Map::new(),
                },
            }),
            FillMode::Gradient => Some(Fill::Gradient(ops::gradient_for(&self.spec, b))),
            m => {
                let mut p = self.pattern.clone();
                p.kind = m.pattern_kind().unwrap_or("two-color").into();
                Some(Fill::Pattern(Box::new(p)))
            }
        }
    }

    pub(crate) fn result(&self) -> Option<Fill> {
        self.result_in(kurbo::Rect::new(0.0, 0.0, 1.0, 1.0))
    }

    fn set_mode(&mut self, m: FillMode) {
        if self.mode == m {
            return;
        }
        if m == FillMode::Gradient && self.mode == FillMode::Solid {
            // Começa do uniforme atual.
            self.spec.stops[0].color = self.color.clone();
        }
        if m == FillMode::Solid && self.mode == FillMode::Gradient {
            self.color = self.spec.stops[self.sel_stop].color.clone();
        }
        if m == FillMode::Texture && self.pattern.texture.is_none() {
            self.pattern.texture = Some(Texture::default());
            self.pattern.width = self.pattern.width.max(30_000.0);
            self.pattern.height = self.pattern.height.max(30_000.0);
            self.pattern.resolution = self.pattern.resolution.min(150.0);
        }
        if m == FillMode::TwoColor && self.pattern.cells.is_empty() {
            self.pattern.cells = two_color_presets().remove(2).1;
        }
        self.mode = m;
        self.changed = true;
    }

    /// Mostra a janela; `true` quando ela foi fechada.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        doc: &mut DocState,
        defaults: &mut Defaults,
    ) -> bool {
        if !matches!(self.eye, Eye::Off) {
            self.eyedrop(ctx);
            return false;
        }
        let title = if self.for_defaults {
            tr("Preenchimento dos novos objetos", "Fill for new objects")
        } else {
            tr("Editar preenchimento", "Edit fill")
        };
        let resp = widgets::modal(ctx, egui::Id::unique("janela-preenchimento")).show(ctx, |ui| {
            ui.set_width(theme::FILL_DIALOG_WIDTH);
            ui.heading(title);
            ui.add_space(theme::SPACE_200);
            self.mode_bar(ui);
            if self.other_kind && !self.changed {
                ui.colored_label(
                    theme::ACCENT_WARNING,
                    tr(
                        "O preenchimento atual não é editável aqui; mudar algo o substitui.",
                        "The current fill is not editable here; changing anything replaces it.",
                    ),
                );
            }
            if self.mode == FillMode::Gradient {
                self.kind_bar(ui);
            }
            ui.separator();
            egui::ScrollArea::vertical()
                .id_salt("preenchimento-corpo")
                .max_height(theme::FILL_BODY_MAX)
                .auto_shrink([false, true])
                .show(ui, |ui| match self.mode {
                    FillMode::None => {
                        ui.add_space(theme::SPACE_400);
                        ui.weak(tr(
                            "Sem preenchimento: o interior do objeto fica transparente.",
                            "No fill: the inside of the object is transparent.",
                        ));
                        ui.add_space(theme::SPACE_400);
                    }
                    FillMode::Solid => self.solid_ui(ui),
                    FillMode::Gradient => self.gradient_ui(ui),
                    _ => self.pattern_ui(ui, doc),
                });
            ui.separator();
            self.footer(ui, doc)
        });
        self.rect = resp.response.rect;
        match resp.inner {
            None => false,
            Some(false) => true,
            Some(true) => {
                if self.changed {
                    let sel = doc.selection.clone();
                    if self.mode == FillMode::Gradient && !self.for_defaults {
                        ops::set_gradient(&mut doc.editor, &sel, &self.spec);
                    } else {
                        let r = self.result();
                        if self.for_defaults {
                            defaults.fill = r;
                        } else {
                            ops::set_fill(&mut doc.editor, &sel, Some(r.unwrap_or(Fill::None)));
                        }
                    }
                }
                true
            }
        }
    }

    fn mode_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let modes = [
                (
                    FillMode::None,
                    "fill-none",
                    tr("Sem preenchimento", "No fill"),
                ),
                (
                    FillMode::Solid,
                    "tool-fill-solid",
                    tr("Uniforme", "Uniform"),
                ),
                (
                    FillMode::Gradient,
                    "fill-gradient",
                    tr("Gradiente", "Gradient"),
                ),
                (
                    FillMode::Vector,
                    "fill-vector",
                    tr("Padrão vetorial", "Vector pattern"),
                ),
                (
                    FillMode::Bitmap,
                    "fill-bitmap",
                    tr("Padrão de imagem", "Bitmap pattern"),
                ),
                (
                    FillMode::TwoColor,
                    "fill-two-color",
                    tr("Padrão de duas cores", "Two-color pattern"),
                ),
                (FillMode::Texture, "fill-texture", tr("Textura", "Texture")),
            ];
            for (m, icon, tip) in modes {
                // O gradiente depende da caixa de cada objeto: só na seleção.
                let on = !(self.for_defaults && m == FillMode::Gradient);
                let r = icons::button(ui, icon, tip, self.mode == m, on);
                let r = if on {
                    r
                } else {
                    r.on_disabled_hover_text(tr(
                        "Gradiente só para objetos selecionados (depende da caixa de cada um)",
                        "Gradient only for selected objects (it depends on each one's box)",
                    ))
                };
                if r.clicked() {
                    self.set_mode(m);
                }
            }
            icons::button(
                ui,
                "fill-postscript",
                tr("PostScript", "PostScript"),
                false,
                false,
            )
            .on_disabled_hover_text(tr(
                "Preenchimento PostScript: fora da 1.0 (exige interpretar PostScript)",
                "PostScript fill: not in 1.0 (needs a PostScript interpreter)",
            ));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let name = modes
                    .iter()
                    .find(|(m, _, _)| *m == self.mode)
                    .map_or("", |(_, _, t)| *t);
                ui.strong(name);
            });
        });
    }

    fn kind_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.add_space(theme::SPACE_200);
            for (k, icon) in KINDS {
                if icons::button(ui, icon, kind_label(k), self.spec.kind == k, true).clicked()
                    && self.spec.kind != k
                {
                    self.spec.kind = k.into();
                    self.changed = true;
                }
            }
            ui.weak(kind_label(&self.spec.kind));
        });
    }

    /// Cor com o seletor no balão (amostra que abre o seletor).
    fn color_popup(&mut self, ui: &mut egui::Ui, id: &str, target: Target) -> Option<Color> {
        let cur = match target {
            Target::Solid => self.color.clone(),
            Target::Stop => self.spec.stops[self.sel_stop].color.clone(),
            Target::Fg => self.pattern.fg.clone().unwrap_or_else(Color::black),
            Target::Bg => self
                .pattern
                .bg
                .clone()
                .unwrap_or_else(|| Color::rgb(1.0, 1.0, 1.0)),
        };
        let popup_id = ui.make_persistent_id(("balao-cor", id));
        let open = egui::Popup::is_id_open(ui.ctx(), popup_id);
        let r = color_button(ui, &cur, open).on_hover_text(crate::colorpick::color_text(&cur));
        let mut out = None;
        let palettes = std::mem::take(&mut self.palettes);
        egui::Popup::menu(&r)
            .id(popup_id)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| {
                ui.set_width(theme::PICKER_WIDTH);
                let o = self.picker.show(ui, id, &cur, &palettes);
                if o.eyedrop {
                    self.eye = Eye::Hide;
                    self.eye_target = target;
                    ui.close();
                }
                out = o.color;
            });
        self.palettes = palettes;
        out
    }

    fn set_target(&mut self, t: Target, c: Color) {
        match t {
            Target::Solid => self.color = c,
            Target::Stop => self.spec.stops[self.sel_stop].color = c,
            Target::Fg => self.pattern.fg = Some(c),
            Target::Bg => self.pattern.bg = Some(c),
        }
        self.changed = true;
    }

    fn solid_ui(&mut self, ui: &mut egui::Ui) {
        let palettes = std::mem::take(&mut self.palettes);
        let o = self
            .picker
            .show(ui, "uniforme", &self.color.clone(), &palettes);
        self.palettes = palettes;
        if let Some(c) = o.color {
            self.set_target(Target::Solid, c);
        }
        if o.eyedrop {
            self.eye = Eye::Hide;
            self.eye_target = Target::Solid;
        }
        ui.add_space(theme::SPACE_200);
        let mut tp = ((1.0 - self.opacity) * 100.0).round();
        ui.horizontal(|ui| {
            ui.label(tr("Transparência", "Transparency"));
            if ui
                .add(egui::Slider::new(&mut tp, 0.0..=100.0).suffix(" %"))
                .changed()
            {
                self.opacity = 1.0 - tp / 100.0;
                self.changed = true;
            }
        });
        if ui
            .checkbox(&mut self.overprint, tr("Sobreimprimir", "Overprint"))
            .on_hover_text(tr(
                "Na impressão, a tinta não apaga as cores de baixo",
                "When printing, the ink does not knock out the colors below",
            ))
            .changed()
        {
            self.changed = true;
        }
    }

    fn gradient_ui(&mut self, ui: &mut egui::Ui) {
        self.ramp(ui);
        ui.add_space(theme::SPACE_200);
        // Nó escolhido: cor, posição e transparência.
        let i = self.sel_stop.min(self.spec.stops.len() - 1);
        ui.horizontal(|ui| {
            ui.label(tr("Nó", "Node"));
            if let Some(c) = self.color_popup(ui, "no", Target::Stop) {
                self.set_target(Target::Stop, c);
            }
            ui.label(tr("Posição", "Position"));
            let mut pos = self.spec.stops[i].offset * 100.0;
            if ui
                .add(
                    egui::DragValue::new(&mut pos)
                        .range(0.0..=100.0)
                        .speed(0.5)
                        .max_decimals(1)
                        .suffix(" %"),
                )
                .changed()
            {
                self.spec.stops[i].offset = pos / 100.0;
                self.sort_stops();
                self.changed = true;
            }
            ui.label(tr("Transparência", "Transparency"));
            let mut tp = ((1.0 - self.spec.stops[self.sel_stop].opacity) * 100.0).round();
            if ui
                .add(egui::Slider::new(&mut tp, 0.0..=100.0).suffix(" %"))
                .changed()
            {
                self.spec.stops[self.sel_stop].opacity = 1.0 - tp / 100.0;
                self.changed = true;
            }
        });
        ui.separator();
        self.transform_ui(ui);
    }

    /// Ângulo, centro, tamanho, faixas, suavizar, repetição, inverter e rota.
    fn transform_ui(&mut self, ui: &mut egui::Ui) {
        let mut ch = false;
        let s = &mut self.spec;
        egui::Grid::new("gradiente-matriz")
            .num_columns(4)
            .spacing([theme::SPACE_300, theme::SPACE_200])
            .show(ui, |ui| {
                ui.label(tr("Ângulo", "Angle"));
                ui.add_enabled_ui(s.kind != "radial", |ui| {
                    ui.horizontal(|ui| {
                        ch |= angle_dial(ui, &mut s.angle);
                        ch |= ui
                            .add(
                                egui::DragValue::new(&mut s.angle)
                                    .range(-360.0..=360.0)
                                    .speed(1.0)
                                    .suffix("°"),
                            )
                            .changed();
                    });
                });
                ui.label(tr("Tamanho", "Size"));
                let mut sc = s.scale * 100.0;
                if ui
                    .add(
                        egui::DragValue::new(&mut sc)
                            .range(5.0..=1000.0)
                            .speed(1.0)
                            .suffix(" %"),
                    )
                    .on_hover_text(tr(
                        "Menor que 100%, a repetição (espelhada ou não) cobre o resto",
                        "Below 100%, the repetition (mirrored or not) covers the rest",
                    ))
                    .changed()
                {
                    s.scale = sc / 100.0;
                    ch = true;
                }
                ui.end_row();
                for (k, label) in [
                    (0, tr("Centro X", "Center X")),
                    (1, tr("Centro Y", "Center Y")),
                ] {
                    ui.label(label);
                    let mut v = s.center[k] * 100.0;
                    if ui
                        .add(
                            egui::DragValue::new(&mut v)
                                .range(-200.0..=200.0)
                                .speed(1.0)
                                .suffix(" %"),
                        )
                        .on_hover_text(tr(
                            "Deslocamento do centro em relação ao centro do objeto (100% = borda)",
                            "Center offset from the object's center (100% = edge)",
                        ))
                        .changed()
                    {
                        s.center[k] = v / 100.0;
                        ch = true;
                    }
                }
                ui.end_row();
                ui.label(tr("Faixas", "Steps"));
                ui.horizontal(|ui| {
                    let locked = s.steps.is_some();
                    if icons::button(
                        ui,
                        if locked { "lock-on" } else { "lock-off" },
                        tr(
                            "Faixas manuais: destrave para escolher o número de faixas",
                            "Manual steps: unlock to choose the number of steps",
                        ),
                        locked,
                        true,
                    )
                    .clicked()
                    {
                        s.steps = if locked { None } else { Some(self.steps_n) };
                        ch = true;
                    }
                    let mut n = s.steps.unwrap_or(self.steps_n);
                    let r = ui.add_enabled(locked, egui::DragValue::new(&mut n).range(2..=256));
                    if r.changed() {
                        self.steps_n = n;
                        s.steps = Some(n);
                        ch = true;
                    }
                    if !locked {
                        ui.weak(tr("contínuo", "continuous"));
                    }
                });
                ui.label(tr("Repetição", "Repeat"));
                ui.horizontal(|ui| {
                    for (k, icon, tip) in [
                        (
                            "reflect",
                            "grad-reflect",
                            tr(
                                "Repetir e espelhar (A → B → A)",
                                "Repeat and mirror (A → B → A)",
                            ),
                        ),
                        (
                            "repeat",
                            "grad-repeat",
                            tr("Repetir (A → B, A → B)", "Repeat (A → B, A → B)"),
                        ),
                    ] {
                        if icons::button(ui, icon, tip, s.spread == k, true).clicked() {
                            s.spread = if s.spread == k {
                                "pad".into()
                            } else {
                                k.into()
                            };
                            ch = true;
                        }
                    }
                });
                ui.end_row();
                ui.label(tr("Mistura", "Blend"));
                ui.horizontal(|ui| {
                    for (k, icon, tip) in [
                        (
                            "direct",
                            "blend-direct",
                            tr(
                                "Em linha reta entre as cores",
                                "Straight between the colors",
                            ),
                        ),
                        (
                            "cw",
                            "blend-cw",
                            tr(
                                "Pela roda de cores, sentido horário",
                                "Around the color wheel, clockwise",
                            ),
                        ),
                        (
                            "ccw",
                            "blend-ccw",
                            tr(
                                "Pela roda de cores, sentido anti-horário",
                                "Around the color wheel, counterclockwise",
                            ),
                        ),
                    ] {
                        if icons::button(ui, icon, tip, s.color_path == k, true).clicked()
                            && s.color_path != k
                        {
                            s.color_path = k.into();
                            ch = true;
                        }
                    }
                });
                ui.label(tr("Suavizar", "Smooth"));
                ui.horizontal(|ui| {
                    if icons::button(
                        ui,
                        "grad-smooth",
                        tr(
                            "Suavizar: pontilhado contra faixas visíveis",
                            "Smooth: dithering against visible banding",
                        ),
                        s.smooth,
                        true,
                    )
                    .clicked()
                    {
                        s.smooth = !s.smooth;
                        ch = true;
                    }
                });
                ui.end_row();
            });
        if ch {
            self.changed = true;
        }
        if ui
            .add(egui::Button::image_and_text(
                egui::Image::new(icons::get("grad-reverse"))
                    .fit_to_exact_size(Vec2::splat(theme::ICON_MICRO)),
                tr("Inverter", "Reverse"),
            ))
            .on_hover_text(tr(
                "O nó inicial vai para o fim, e a ordem se espelha",
                "The start node goes to the end, and the order mirrors",
            ))
            .clicked()
        {
            self.reverse();
        }
    }

    /// Rampa: a faixa do gradiente com os nós embaixo e os pontos médios em
    /// cima. Duplo clique na faixa põe um nó; no nó, tira; arrastar move;
    /// Delete tira o nó escolhido; uma amostra solta na rampa vira nó.
    fn ramp(&mut self, ui: &mut egui::Ui) {
        let w = ui.available_width().min(theme::FILL_DIALOG_WIDTH);
        let (mid_h, bar_h, node_h) = (
            theme::RAMP_MID_ROW,
            theme::RAMP_HEIGHT,
            theme::RAMP_NODE_ROW,
        );
        let (rect, resp) = ui.allocate_exact_size(
            Vec2::new(w, mid_h + bar_h + node_h),
            egui::Sense::click_and_drag(),
        );
        let pad = theme::RAMP_NODE / 2.0 + 1.0;
        let bar = egui::Rect::from_min_size(
            rect.min + Vec2::new(pad, mid_h),
            Vec2::new(w - 2.0 * pad, bar_h),
        );
        self.ramp_bar = bar;
        let x_of = |o: f64| bar.left() + (o.clamp(0.0, 1.0) as f32) * bar.width();
        let t_of = |x: f32| (((x - bar.left()) / bar.width()).clamp(0.0, 1.0)) as f64;
        // Faixa pelo motor (paradas assadas: ponto médio, faixas e rota).
        let ppp = ui.ctx().pixels_per_point();
        let (pw, ph) = (
            (bar.width() * ppp).max(1.0) as u32,
            (bar_h * ppp).max(1.0) as u32,
        );
        let fill = Fill::Gradient(ops::gradient_for(
            &ops::GradientSpec {
                kind: "linear".into(),
                angle: 0.0,
                spread: "pad".into(),
                center: [0.0, 0.0],
                scale: 1.0,
                smooth: false,
                ..self.spec.clone()
            },
            kurbo::Rect::new(0.0, 0.0, pw as f64, ph as f64),
        ));
        if self.bar_tex.as_ref().is_none_or(|(f, _)| *f != fill)
            && let Some(rgba) = raster::fill_preview(&fill, pw, ph)
        {
            let img = egui::ColorImage::from_rgba_unmultiplied([pw as usize, ph as usize], &rgba);
            let t = ui
                .ctx()
                .load_texture("rampa-gradiente", img, egui::TextureOptions::LINEAR);
            self.bar_tex = Some((fill, t));
        }
        let p = ui.painter();
        checker(p, bar);
        if let Some((_, t)) = &self.bar_tex {
            p.image(
                t.id(),
                bar,
                egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                Color32::WHITE,
            );
        }
        let hovering_swatch = resp.dnd_hover_payload::<SwatchDrop>().is_some();
        p.rect_stroke(
            bar,
            theme::RADIUS_NONE,
            if hovering_swatch {
                egui::Stroke::new(theme::FOCUS_WIDTH, theme::ACCENT_PRIMARY)
            } else {
                theme::border_default(ui)
            },
            egui::StrokeKind::Outside,
        );
        let pal = theme::palette(ui);
        let stops = self.spec.stops.clone();
        // Pontos médios: losangos acima da faixa.
        let mid_x = |i: usize| {
            let (a, b) = (stops[i].offset, stops[i + 1].offset);
            x_of(a + (b - a) * stops[i].midpoint)
        };
        let mid_y = rect.top() + mid_h / 2.0;
        for i in 0..stops.len().saturating_sub(1) {
            let c = egui::pos2(mid_x(i), mid_y);
            let r = theme::RAMP_MID / 2.0;
            let active = self.drag == Some(Ramp::Mid(i));
            p.add(egui::Shape::convex_polygon(
                vec![
                    c + Vec2::new(0.0, -r),
                    c + Vec2::new(r, 0.0),
                    c + Vec2::new(0.0, r),
                    c + Vec2::new(-r, 0.0),
                ],
                if active {
                    theme::ACCENT_PRIMARY
                } else {
                    pal.fg_muted
                },
                egui::Stroke::NONE,
            ));
        }
        // Nós: quadrados embaixo, com um traço até a faixa.
        let node_y = bar.bottom() + node_h / 2.0;
        for (k, s) in stops.iter().enumerate() {
            let x = x_of(s.offset);
            p.line_segment(
                [egui::pos2(x, bar.bottom()), egui::pos2(x, node_y)],
                egui::Stroke::new(theme::BORDER_WIDTH, pal.fg_muted),
            );
            let r =
                egui::Rect::from_center_size(egui::pos2(x, node_y), Vec2::splat(theme::RAMP_NODE));
            p.rect_filled(r, theme::RADIUS_NONE, to_color32(&s.color));
            let st = if k == self.sel_stop {
                egui::Stroke::new(theme::FOCUS_WIDTH, theme::ACCENT_PRIMARY)
            } else {
                egui::Stroke::new(theme::BORDER_WIDTH, pal.fg_strong)
            };
            p.rect_stroke(r, theme::RADIUS_NONE, st, egui::StrokeKind::Outside);
        }
        let hit = |q: egui::Pos2| -> Option<Ramp> {
            if q.y >= bar.bottom() - 2.0 {
                stops
                    .iter()
                    .enumerate()
                    .map(|(k, s)| (k, (x_of(s.offset) - q.x).abs()))
                    .filter(|(_, d)| *d <= theme::HANDLE_HIT)
                    .min_by(|a, b| a.1.total_cmp(&b.1))
                    .map(|(k, _)| Ramp::Node(k))
            } else if q.y <= bar.top() + 2.0 {
                (0..stops.len().saturating_sub(1))
                    .map(|i| (i, (mid_x(i) - q.x).abs()))
                    .filter(|(_, d)| *d <= theme::HANDLE_HIT)
                    .min_by(|a, b| a.1.total_cmp(&b.1))
                    .map(|(i, _)| Ramp::Mid(i))
            } else {
                None
            }
        };
        if resp.drag_started()
            && let Some(q) = ui.input(|i| i.pointer.press_origin())
        {
            self.drag = hit(q);
            if let Some(Ramp::Node(k)) = self.drag {
                self.sel_stop = k;
            }
        }
        if resp.dragged()
            && let Some(q) = resp.interact_pointer_pos()
        {
            match self.drag {
                Some(Ramp::Node(_)) => {
                    self.spec.stops[self.sel_stop].offset = t_of(q.x);
                    self.sort_stops();
                    self.drag = Some(Ramp::Node(self.sel_stop));
                    self.changed = true;
                }
                Some(Ramp::Mid(i)) if i + 1 < self.spec.stops.len() => {
                    let (a, b) = (
                        x_of(self.spec.stops[i].offset),
                        x_of(self.spec.stops[i + 1].offset),
                    );
                    if b - a > 1.0 {
                        self.spec.stops[i].midpoint =
                            (((q.x - a) / (b - a)) as f64).clamp(0.05, 0.95);
                        self.changed = true;
                    }
                }
                _ => {}
            }
        }
        if resp.drag_stopped() {
            self.drag = None;
        }
        if resp.double_clicked()
            && let Some(q) = resp.interact_pointer_pos()
        {
            match hit(q) {
                Some(Ramp::Node(k)) => self.remove_stop(k),
                Some(Ramp::Mid(i)) => {
                    self.spec.stops[i].midpoint = 0.5;
                    self.changed = true;
                }
                None if bar.expand(2.0).contains(q) => self.add_stop(t_of(q.x), None),
                None => {}
            }
        } else if resp.clicked()
            && let Some(q) = resp.interact_pointer_pos()
            && let Some(Ramp::Node(k)) = hit(q)
        {
            self.sel_stop = k;
        }
        if let Some(c) = resp.dnd_release_payload::<SwatchDrop>()
            && let Some(q) = ui.input(|i| i.pointer.interact_pos())
        {
            self.add_stop(t_of(q.x), Some(c.0.clone()));
        }
        // Delete sem campo de texto em foco: tira o nó escolhido.
        if ui.memory(|m| m.focused().is_none()) && ui.input(|i| i.key_pressed(egui::Key::Delete)) {
            self.remove_stop(self.sel_stop);
        }
        resp.on_hover_text(tr(
            "Duplo clique na faixa: novo nó · duplo clique no nó: tira · arraste nós e losangos (ponto médio) · Delete tira o nó · solte uma amostra para criar um nó",
            "Double-click the band: new node · double-click a node: remove · drag nodes and diamonds (midpoint) · Delete removes the node · drop a swatch to add a node",
        ));
    }

    /// Padrões: vetorial, imagem, duas cores e textura, com o ladrilho.
    fn pattern_ui(&mut self, ui: &mut egui::Ui, doc: &mut DocState) {
        let mut changed = false;
        match self.mode {
            FillMode::Bitmap => {
                let p = &mut self.pattern;
                ui.horizontal(|ui| {
                    if ui.button(tr("Escolher imagem…", "Choose image…")).clicked()
                        && let Some(path) = rfd::FileDialog::new()
                            .add_filter(
                                tr("Imagens", "Images"),
                                &["png", "jpg", "jpeg", "tif", "tiff", "webp"],
                            )
                            .pick_file()
                        && let Ok(bytes) = std::fs::read(&path)
                    {
                        let ext = path
                            .extension()
                            .map(|e| e.to_string_lossy().to_lowercase())
                            .unwrap_or_else(|| "png".into());
                        let ext = if ext == "jpeg" {
                            "jpg".to_string()
                        } else {
                            ext
                        };
                        p.src = Some(doc.editor.doc.add_image(bytes, &ext));
                        changed = true;
                    }
                    ui.weak(p.src.as_deref().map_or(tr("nenhuma", "none"), |_| {
                        tr("imagem escolhida", "image chosen")
                    }));
                });
            }
            FillMode::Vector => {
                let p = &mut self.pattern;
                let n = TILE_SOURCE.lock().map(|t| t.len()).unwrap_or(0);
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(
                            n > 0,
                            egui::Button::new(tr("Usar objetos copiados", "Use copied objects")),
                        )
                        .on_disabled_hover_text(tr(
                            "Copie (Ctrl+C) os objetos do ladrilho antes",
                            "Copy (Ctrl+C) the tile objects first",
                        ))
                        .clicked()
                        && let Ok(t) = TILE_SOURCE.lock()
                        && let Some(b) = graphdraw_core::edit::query::union_bounds(t.iter(), true)
                    {
                        // O ladrilho é a caixa dos objetos, levada à origem.
                        let m = kurbo::Affine::translate((-b.x0, -b.y0));
                        p.tile = t
                            .iter()
                            .map(|o| {
                                let mut o = o.clone();
                                o.transform = graphdraw_core::model::Transform::from_affine(
                                    m * o.transform.affine(),
                                );
                                o
                            })
                            .collect();
                        p.width = b.width().max(1.0);
                        p.height = b.height().max(1.0);
                        changed = true;
                    }
                    ui.weak(format!(
                        "{} {}",
                        p.tile.len(),
                        tr("objeto(s) no ladrilho", "object(s) in the tile")
                    ));
                });
            }
            FillMode::Texture => {
                let tx = self.pattern.texture.get_or_insert_with(Texture::default);
                ui.horizontal(|ui| {
                    for (k, label) in [
                        ("clouds", tr("Nuvens", "Clouds")),
                        ("marble", tr("Mármore", "Marble")),
                        ("wood", tr("Madeira", "Wood")),
                        ("granite", tr("Granito", "Granite")),
                    ] {
                        if ui.selectable_label(tx.style == k, label).clicked() && tx.style != k {
                            tx.style = k.into();
                            changed = true;
                        }
                    }
                });
                egui::Grid::new("textura-grade")
                    .num_columns(4)
                    .show(ui, |ui| {
                        ui.label(tr("Semente", "Seed"));
                        ui.horizontal(|ui| {
                            changed |= ui.add(egui::DragValue::new(&mut tx.seed)).changed();
                            if ui.small_button(tr("Sortear", "Shuffle")).clicked() {
                                let t = std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .map_or(1, |d| d.subsec_nanos());
                                tx.seed = t % 100_000;
                                changed = true;
                            }
                        });
                        ui.label(tr("Escala do ruído", "Noise scale"));
                        changed |= ui
                            .add(
                                egui::DragValue::new(&mut tx.frequency)
                                    .range(1.0..=32.0)
                                    .speed(0.2),
                            )
                            .changed();
                        ui.end_row();
                        ui.label(tr("Detalhe", "Detail"));
                        changed |= ui
                            .add(egui::DragValue::new(&mut tx.detail).range(1..=8))
                            .changed();
                        ui.label(tr("Contraste", "Contrast"));
                        let mut c = tx.contrast * 100.0;
                        if ui
                            .add(egui::Slider::new(&mut c, 0.0..=100.0).suffix(" %"))
                            .changed()
                        {
                            tx.contrast = c / 100.0;
                            changed = true;
                        }
                        ui.end_row();
                    });
                self.fg_bg(ui);
            }
            _ => {
                let p = &mut self.pattern;
                ui.horizontal_wrapped(|ui| {
                    for (name, cells) in two_color_presets() {
                        if ui.small_button(name).clicked() {
                            p.cells = cells;
                            changed = true;
                        }
                    }
                });
                changed |= cell_grid(ui, p);
                self.fg_bg(ui);
            }
        }
        let p = &mut self.pattern;
        ui.add_space(theme::SPACE_200);
        ui.horizontal(|ui| {
            ui.label(tr("Ladrilho", "Tile"));
            if let Some(v) = widgets::length_field(ui, "padrao-largura", p.width, None) {
                p.width = v.max(100.0);
                changed = true;
            }
            ui.label("×");
            if let Some(v) = widgets::length_field(ui, "padrao-altura", p.height, None) {
                p.height = v.max(100.0);
                changed = true;
            }
        });
        egui::Grid::new("padrao-grade")
            .num_columns(4)
            .spacing([theme::SPACE_300, theme::SPACE_200])
            .show(ui, |ui| {
                let mut drag = |ui: &mut egui::Ui,
                                label: &str,
                                v: &mut f64,
                                range: std::ops::RangeInclusive<f64>,
                                suffix: &str,
                                k: f64| {
                    ui.label(label);
                    let mut x = *v * k;
                    if ui
                        .add(egui::DragValue::new(&mut x).range(range).suffix(suffix))
                        .changed()
                    {
                        *v = x / k;
                        changed = true;
                    }
                };
                drag(
                    ui,
                    tr("Escala", "Scale"),
                    &mut p.scale,
                    1.0..=10_000.0,
                    " %",
                    100.0,
                );
                drag(
                    ui,
                    tr("Giro", "Rotation"),
                    &mut p.rotation,
                    -360.0..=360.0,
                    "°",
                    1.0,
                );
                ui.end_row();
                drag(
                    ui,
                    tr("Resolução", "Resolution"),
                    &mut p.resolution,
                    36.0..=1200.0,
                    " ppp",
                    1.0,
                );
                ui.label("");
                ui.label("");
                ui.end_row();
                drag(
                    ui,
                    tr("Deslocar linhas", "Row offset"),
                    &mut p.row_offset,
                    0.0..=100.0,
                    " %",
                    100.0,
                );
                drag(
                    ui,
                    tr("Deslocar colunas", "Column offset"),
                    &mut p.column_offset,
                    0.0..=100.0,
                    " %",
                    100.0,
                );
                ui.end_row();
            });
        let p = &mut self.pattern;
        changed |= ui
            .checkbox(
                &mut p.mirror,
                tr("Espelhar ladrilhos alternados", "Mirror alternate tiles"),
            )
            .changed();
        changed |= ui
            .checkbox(&mut p.transform_with_object, tr("Transformar com o objeto", "Transform with object"))
            .on_hover_text(tr(
                "Ligado, o padrão acompanha escala e giro do objeto; desligado, fica preso à página",
                "On, the pattern follows the object's scale and rotation; off, it stays fixed to the page",
            ))
            .changed();
        if changed {
            self.changed = true;
        }
    }

    /// Cores da frente e do fundo (duas cores, textura).
    fn fg_bg(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(tr("Frente", "Front"));
            if let Some(c) = self.color_popup(ui, "frente", Target::Fg) {
                self.set_target(Target::Fg, c);
            }
            ui.label(tr("Fundo", "Back"));
            if let Some(c) = self.color_popup(ui, "fundo", Target::Bg) {
                self.set_target(Target::Bg, c);
            }
        });
    }

    /// Rodapé: amostra original (embaixo) × nova (em cima), Ajuda, Cancelar
    /// e OK. `Some(true)` = OK, `Some(false)` = Cancelar.
    fn footer(&mut self, ui: &mut egui::Ui, doc: &DocState) -> Option<bool> {
        let mut out = None;
        ui.horizontal(|ui| {
            self.compare(ui, doc);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
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
                        ui.label(help_text(self.mode));
                    });
            });
        });
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) && !egui::Popup::is_any_open(ui.ctx()) {
            out = Some(false);
        }
        out
    }

    /// Amostra bipartida: em cima a nova, embaixo a original (sobre xadrez).
    fn compare(&mut self, ui: &mut egui::Ui, doc: &DocState) {
        let (w, h) = (theme::FILL_COMPARE_W, theme::FILL_COMPARE_H);
        let (rect, _) = ui.allocate_exact_size(Vec2::new(w, h), egui::Sense::hover());
        let top = egui::Rect::from_min_size(rect.min, Vec2::new(w, h / 2.0));
        let bottom =
            egui::Rect::from_min_size(rect.min + Vec2::new(0.0, h / 2.0), Vec2::new(w, h / 2.0));
        checker(ui.painter(), rect);
        let ppp = ui.ctx().pixels_per_point();
        let (pw, ph) = ((w * ppp) as u32, (h / 2.0 * ppp) as u32);
        let b = kurbo::Rect::new(0.0, 0.0, pw as f64, ph as f64);
        let paint = |ui: &egui::Ui, tex: &egui::TextureHandle, r: egui::Rect| {
            // Y da página para cima: a textura sai invertida na vertical.
            ui.painter().image(
                tex.id(),
                r,
                egui::Rect::from_min_max(egui::pos2(0.0, 1.0), egui::pos2(1.0, 0.0)),
                Color32::WHITE,
            );
        };
        let new = self.result_in(b);
        match &new {
            None => none_mark(ui.painter(), top),
            Some(f) => {
                if self
                    .sample_tex
                    .as_ref()
                    .is_none_or(|(old, _)| old.as_ref() != Some(f))
                    && let Some(rgba) = raster::fill_preview_in(doc.doc(), f, pw, ph)
                {
                    let img =
                        egui::ColorImage::from_rgba_unmultiplied([pw as usize, ph as usize], &rgba);
                    let t = ui.ctx().load_texture(
                        "amostra-preenchimento",
                        img,
                        egui::TextureOptions::LINEAR,
                    );
                    self.sample_tex = Some((new.clone(), t));
                }
                if let Some((_, t)) = &self.sample_tex {
                    paint(ui, t, top);
                }
            }
        }
        // Original: o gradiente do objeto refeito para a amostra.
        let before = match &self.initial {
            Some(Fill::Gradient(g)) => Some(Fill::Gradient(ops::gradient_for(
                &ops::gradient_spec_in(g, self.box_hint),
                b,
            ))),
            Some(Fill::None) | None => None,
            f => f.clone(),
        };
        match &before {
            None => none_mark(ui.painter(), bottom),
            Some(f) => {
                if self.before_tex.is_none()
                    && let Some(rgba) = raster::fill_preview_in(doc.doc(), f, pw, ph)
                {
                    let img =
                        egui::ColorImage::from_rgba_unmultiplied([pw as usize, ph as usize], &rgba);
                    self.before_tex = Some(ui.ctx().load_texture(
                        "amostra-original",
                        img,
                        egui::TextureOptions::LINEAR,
                    ));
                }
                if let Some(t) = &self.before_tex {
                    paint(ui, t, bottom);
                }
            }
        }
        let p = ui.painter();
        p.line_segment(
            [bottom.left_top(), bottom.right_top()],
            egui::Stroke::new(theme::BORDER_WIDTH, theme::palette(ui).fg_muted),
        );
        p.rect_stroke(
            rect,
            theme::RADIUS_NONE,
            theme::border_default(ui),
            egui::StrokeKind::Outside,
        );
        // Rótulos ao lado de cada metade.
        let (lr, _) = ui.allocate_exact_size(
            Vec2::new(theme::FILL_COMPARE_LABEL, h),
            egui::Sense::hover(),
        );
        let fg = theme::palette(ui).fg_muted;
        for (r, t) in [
            (top, tr("Nova", "New")),
            (bottom, tr("Original", "Original")),
        ] {
            ui.painter().text(
                egui::pos2(lr.left() + theme::SPACE_200, r.center().y),
                egui::Align2::LEFT_CENTER,
                t,
                theme::font_micro(),
                fg,
            );
        }
    }

    /// Conta-gotas: janela escondida → foto da tela → escolha na foto.
    fn eyedrop(&mut self, ctx: &egui::Context) {
        match &mut self.eye {
            Eye::Off => {}
            Eye::Hide => {
                let slot: Arc<Mutex<Option<Arc<egui::ColorImage>>>> = Arc::new(Mutex::new(None));
                let (s, c) = (slot.clone(), ctx.clone());
                ctx.request_screenshot(move |img| {
                    if let Ok(mut v) = s.lock() {
                        *v = Some(img);
                    }
                    c.request_repaint();
                });
                self.eye = Eye::Wait(slot, 0);
                ctx.request_repaint();
            }
            Eye::Wait(slot, frames) => {
                let img = slot.lock().ok().and_then(|mut v| v.take());
                if let Some(img) = img {
                    let tex = ctx.load_texture(
                        "conta-gotas-tela",
                        (*img).clone(),
                        egui::TextureOptions::NEAREST,
                    );
                    self.eye = Eye::Pick(tex, img);
                } else if *frames > 120 {
                    self.eye = Eye::Off;
                } else {
                    *frames += 1;
                    ctx.request_repaint();
                }
            }
            Eye::Pick(tex, img) => {
                let screen = ctx.content_rect();
                let (tex, img) = (tex.clone(), img.clone());
                let mut picked = None;
                let mut cancel = ctx.input(|i| i.key_pressed(egui::Key::Escape));
                egui::Area::new(egui::Id::unique("conta-gotas-janela"))
                    .order(egui::Order::Foreground)
                    .fixed_pos(screen.min)
                    .show(ctx, |ui| {
                        let (r, resp) = ui.allocate_exact_size(screen.size(), egui::Sense::click());
                        let p = ui.painter();
                        p.image(
                            tex.id(),
                            r,
                            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                            Color32::WHITE,
                        );
                        ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
                        let (sx, sy) = (
                            img.size[0] as f32 / r.width(),
                            img.size[1] as f32 / r.height(),
                        );
                        let px = |q: egui::Pos2| {
                            let x = ((q.x - r.left()) * sx) as i64;
                            let y = ((q.y - r.top()) * sy) as i64;
                            (
                                x.clamp(0, img.size[0] as i64 - 1) as usize,
                                y.clamp(0, img.size[1] as i64 - 1) as usize,
                            )
                        };
                        if let Some(q) = resp.hover_pos() {
                            // Lupa: 11 × 11 pixels ampliados, o do meio marcado.
                            let (cx, cy) = px(q);
                            let cell = theme::LOUPE_CELL;
                            let n = 11i64;
                            let origin = q + Vec2::new(theme::SPACE_600, theme::SPACE_600);
                            let lens =
                                egui::Rect::from_min_size(origin, Vec2::splat(cell * n as f32));
                            p.rect_filled(lens.expand(2.0), theme::RADIUS_XS, Color32::BLACK);
                            for dy in 0..n {
                                for dx in 0..n {
                                    let x = (cx as i64 + dx - n / 2)
                                        .clamp(0, img.size[0] as i64 - 1)
                                        as usize;
                                    let y = (cy as i64 + dy - n / 2)
                                        .clamp(0, img.size[1] as i64 - 1)
                                        as usize;
                                    let c = img.pixels[y * img.size[0] + x];
                                    let cr = egui::Rect::from_min_size(
                                        origin + Vec2::new(dx as f32 * cell, dy as f32 * cell),
                                        Vec2::splat(cell),
                                    );
                                    p.rect_filled(cr, 0.0, c);
                                }
                            }
                            let mid = egui::Rect::from_min_size(
                                origin + Vec2::splat(cell * (n / 2) as f32),
                                Vec2::splat(cell),
                            );
                            p.rect_stroke(
                                mid,
                                0.0,
                                egui::Stroke::new(theme::FOCUS_WIDTH, Color32::WHITE),
                                egui::StrokeKind::Outside,
                            );
                            let c = img.pixels[cy * img.size[0] + cx];
                            p.text(
                                lens.left_bottom() + Vec2::new(0.0, theme::SPACE_200),
                                egui::Align2::LEFT_TOP,
                                format!("#{:02X}{:02X}{:02X}", c.r(), c.g(), c.b()),
                                theme::font_micro(),
                                Color32::WHITE,
                            );
                        }
                        if resp.clicked()
                            && let Some(q) = resp.interact_pointer_pos()
                        {
                            let (x, y) = px(q);
                            let c = img.pixels[y * img.size[0] + x];
                            picked = Some(Color::rgb(
                                c.r() as f64 / 255.0,
                                c.g() as f64 / 255.0,
                                c.b() as f64 / 255.0,
                            ));
                        }
                        if resp.secondary_clicked() {
                            cancel = true;
                        }
                    });
                if let Some(c) = picked {
                    let t = self.eye_target;
                    self.set_target(t, c);
                    self.eye = Eye::Off;
                } else if cancel {
                    self.eye = Eye::Off;
                }
            }
        }
    }
}

/// Mostrador de ângulo: arrastar gira o ponteiro (Shift: de 15 em 15°).
fn angle_dial(ui: &mut egui::Ui, angle: &mut f64) -> bool {
    let d = theme::ANGLE_DIAL;
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(d), egui::Sense::click_and_drag());
    let c = rect.center();
    let r = d / 2.0 - 1.0;
    let pal = theme::palette(ui);
    let p = ui.painter();
    p.circle(c, r, pal.bg_surface_1, theme::border_default(ui));
    let a = angle.to_radians() as f32;
    let tip = c + Vec2::new(a.cos(), -a.sin()) * (r - 2.0);
    p.line_segment(
        [c, tip],
        egui::Stroke::new(theme::FOCUS_WIDTH, theme::ACCENT_PRIMARY),
    );
    p.circle_filled(c, 2.0, theme::ACCENT_PRIMARY);
    if (resp.dragged() || resp.clicked())
        && let Some(q) = resp.interact_pointer_pos()
    {
        let v = q - c;
        if v.length() > 2.0 {
            let mut deg = ((-v.y).atan2(v.x) as f64).to_degrees();
            if ui.input(|i| i.modifiers.shift) {
                deg = (deg / 15.0).round() * 15.0;
            }
            *angle = deg.round();
            return true;
        }
    }
    false
}

/// Grade de células do padrão de duas cores: clique troca frente/fundo.
fn cell_grid(ui: &mut egui::Ui, p: &mut Pattern) -> bool {
    let rows = p.cells.len().max(1);
    let cols = p.cells.iter().map(|r| r.len()).max().unwrap_or(8).max(1);
    let cell = theme::PATTERN_CELL;
    let (r, resp) = ui.allocate_exact_size(
        Vec2::new(cols as f32 * cell, rows as f32 * cell),
        egui::Sense::click(),
    );
    let fg = p.fg.as_ref().map_or(Color32::BLACK, to_color32);
    let bg = p.bg.as_ref().map_or(Color32::WHITE, to_color32);
    for (y, row) in p.cells.iter().enumerate() {
        for (x, c) in row.chars().enumerate() {
            let cr = egui::Rect::from_min_size(
                r.min + Vec2::new(x as f32 * cell, y as f32 * cell),
                Vec2::splat(cell),
            );
            ui.painter()
                .rect_filled(cr.shrink(0.5), 0.0, if c == '1' { fg } else { bg });
        }
    }
    if resp.clicked()
        && let Some(pos) = resp.interact_pointer_pos()
    {
        let (x, y) = (
            ((pos.x - r.min.x) / cell) as usize,
            ((pos.y - r.min.y) / cell) as usize,
        );
        if let Some(row) = p.cells.get_mut(y) {
            let mut chars: Vec<char> = row.chars().collect();
            if let Some(c) = chars.get_mut(x) {
                *c = if *c == '1' { '0' } else { '1' };
                *row = chars.into_iter().collect();
                return true;
            }
        }
    }
    false
}

/// Ajuda do modo atual (botão Ajuda do rodapé).
fn help_text(m: FillMode) -> &'static str {
    match m {
        FillMode::None => tr(
            "Sem preenchimento: o interior do objeto fica transparente; o contorno continua.",
            "No fill: the inside of the object is transparent; the outline stays.",
        ),
        FillMode::Solid => tr(
            "Uniforme: uma cor só. Escolha na roda (matiz no anel, saturação e brilho no quadrado), pelos canais do modelo, pelo hexadecimal, numa paleta ou com o conta-gotas.",
            "Uniform: a single color. Pick it on the wheel (hue on the ring, saturation and brightness in the square), by the model channels, hexadecimal, a palette or the eyedropper.",
        ),
        FillMode::Gradient => tr(
            "Gradiente: escolha o tipo na sub-barra. Na rampa, duplo clique cria um nó, duplo clique no nó o tira, os losangos ajustam o ponto médio entre dois nós. Abaixo: cor, posição e transparência do nó; ângulo, centro, tamanho, faixas, repetição, mistura pela roda de cores e suavizar.",
            "Gradient: pick the type on the sub-bar. On the ramp, double-click adds a node, double-click a node removes it, the diamonds set the midpoint between two nodes. Below: node color, position and transparency; angle, center, size, steps, repeat, blend around the color wheel and smoothing.",
        ),
        FillMode::Vector => tr(
            "Padrão vetorial: copie (Ctrl+C) os objetos do ladrilho e use \"Usar objetos copiados\". O ladrilho se repete sem perder a nitidez.",
            "Vector pattern: copy (Ctrl+C) the tile objects and use \"Use copied objects\". The tile repeats without losing sharpness.",
        ),
        FillMode::Bitmap => tr(
            "Padrão de imagem: escolha uma imagem; ela vira o ladrilho, repetido lado a lado.",
            "Bitmap pattern: choose an image; it becomes the tile, repeated side by side.",
        ),
        FillMode::TwoColor => tr(
            "Duas cores: clique nas células para trocar frente e fundo, ou parta de uma predefinição.",
            "Two colors: click the cells to swap front and back, or start from a preset.",
        ),
        FillMode::Texture => tr(
            "Textura: ruído fractal (nuvens, mármore, madeira, granito) entre a cor da frente e a do fundo; a semente muda o desenho e o ladrilho emenda nas bordas.",
            "Texture: fractal noise (clouds, marble, wood, granite) between the front and back colors; the seed changes the drawing and the tile wraps at the edges.",
        ),
    }
}

/// Fundo xadrez (mostra transparência).
pub(crate) fn checker(p: &egui::Painter, rect: egui::Rect) {
    let cell = theme::CHECKER_CELL;
    let (cols, rows) = (
        (rect.width() / cell).ceil() as i32,
        (rect.height() / cell).ceil() as i32,
    );
    let p = p.with_clip_rect(rect);
    for i in 0..cols {
        for j in 0..rows {
            let c = if (i + j) % 2 == 0 {
                theme::CHECKER_LIGHT
            } else {
                theme::CHECKER_DARK
            };
            p.rect_filled(
                egui::Rect::from_min_size(
                    rect.min + Vec2::new(i as f32 * cell, j as f32 * cell),
                    Vec2::splat(cell),
                ),
                theme::RADIUS_NONE,
                c,
            );
        }
    }
}

fn none_mark(p: &egui::Painter, rect: egui::Rect) {
    p.rect_filled(rect, theme::RADIUS_NONE, theme::PAGE_PAPER);
    p.line_segment(
        [rect.left_bottom(), rect.right_top()],
        egui::Stroke::new(theme::FOCUS_WIDTH, theme::ACCENT_DANGER),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use graphdraw_core::model::Color;

    fn run(f: &mut FillDialog, doc: &mut DocState) {
        let ctx = crate::theme::test_ctx();
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(1366.0, 768.0));
        let mut defaults = Defaults::default();
        for _ in 0..3 {
            let input = egui::RawInput {
                screen_rect: Some(screen),
                ..Default::default()
            };
            let mut out = ctx.run_ui(input, |ui| {
                f.show(ui.ctx(), doc, &mut defaults);
            });
            out.textures_delta.clear();
        }
        assert!(
            screen.contains_rect(f.rect) && f.rect.height() < 740.0,
            "{:?}",
            f.rect
        );
    }

    /// GD-FIL-03: a janela abre um gradiente existente (tipo, paradas,
    /// repetição, centro e tamanho), cabe na tela e desenha rampa e amostras.
    #[test]
    fn gradient_round_trip_and_fits() {
        let mut doc = DocState::new_blank();
        let b = kurbo::Rect::new(0.0, 0.0, 10.0, 10.0);
        let spec = ops::GradientSpec {
            kind: "radial".into(),
            stops: vec![
                stop(0.0, Color::rgb(1.0, 0.0, 0.0)),
                stop(0.4, Color::rgb(0.0, 1.0, 0.0)),
                stop(1.0, Color::rgb(0.0, 0.0, 1.0)),
            ],
            spread: "reflect".into(),
            center: [0.25, 0.0],
            scale: 0.5,
            ..Default::default()
        };
        let g = Fill::Gradient(ops::gradient_for(&spec, b));
        let mut f = FillDialog::new(Some(g), false, Some(b));
        assert_eq!(
            (f.mode, f.spec.kind.as_str()),
            (FillMode::Gradient, "radial")
        );
        assert_eq!((f.spec.stops.len(), f.spec.spread.as_str()), (3, "reflect"));
        assert!((f.spec.center[0] - 0.25).abs() < 1e-9 && (f.spec.scale - 0.5).abs() < 1e-9);
        run(&mut f, &mut doc);
        assert!(f.sample_tex.is_some() && f.bar_tex.is_some() && f.before_tex.is_some());
    }

    /// Nós: inserir pega a cor do ponto; só os do meio saem; inverter
    /// espelha posições e pontos médios.
    #[test]
    fn nodes_add_remove_reverse() {
        let mut f = FillDialog::new(None, false, None);
        f.mode = FillMode::Gradient;
        f.spec.stops = vec![
            stop(0.0, Color::rgb(0.0, 0.0, 0.0)),
            stop(1.0, Color::rgb(1.0, 1.0, 1.0)),
        ];
        f.add_stop(0.5, None);
        assert_eq!(f.spec.stops.len(), 3);
        assert_eq!(f.sel_stop, 1);
        let g = f.spec.stops[1].color.to_srgb()[0];
        assert!((g - 0.5).abs() < 0.02, "cor do ponto: {g}");
        f.remove_stop(0);
        assert_eq!(f.spec.stops.len(), 3, "a ponta fica");
        f.spec.stops[0].midpoint = 0.2;
        f.spec.stops[1].offset = 0.3;
        f.reverse();
        let o: Vec<f64> = f.spec.stops.iter().map(|s| s.offset).collect();
        assert!((o[1] - 0.7).abs() < 1e-9, "{o:?}");
        assert!(
            (f.spec.stops[1].midpoint - 0.8).abs() < 1e-9,
            "ponto médio espelhado"
        );
        f.remove_stop(1);
        assert_eq!(f.spec.stops.len(), 2);
    }

    /// Rampa com o ponteiro: duplo clique na faixa cria um nó ali; arrastar
    /// o losango muda o ponto médio; duplo clique no nó do meio o tira.
    #[test]
    fn ramp_pointer() {
        let ctx = crate::theme::test_ctx();
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(1366.0, 768.0));
        let mut doc = DocState::new_blank();
        let mut defaults = Defaults::default();
        let mut f = FillDialog::new(None, false, None);
        f.set_mode(FillMode::Gradient);
        let mut t = 0.0;
        let mut frame = |f: &mut FillDialog, events: Vec<egui::Event>| {
            t += 0.05;
            let input = egui::RawInput {
                screen_rect: Some(screen),
                time: Some(t),
                events,
                ..Default::default()
            };
            let mut out = ctx.run_ui(input, |ui| {
                f.show(ui.ctx(), &mut doc, &mut defaults);
            });
            out.textures_delta.clear();
        };
        for _ in 0..30 {
            frame(&mut f, vec![]);
        }
        let bar = f.ramp_bar;
        assert!(bar.is_positive());
        let btn = |p: egui::Pos2, pressed: bool| egui::Event::PointerButton {
            pos: p,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        let at = egui::pos2(bar.left() + bar.width() * 0.25, bar.center().y);
        frame(&mut f, vec![egui::Event::PointerMoved(at)]);
        for _ in 0..2 {
            frame(&mut f, vec![btn(at, true)]);
            frame(&mut f, vec![btn(at, false)]);
        }
        assert_eq!(f.spec.stops.len(), 3, "nó novo");
        assert!((f.spec.stops[1].offset - 0.25).abs() < 0.01);
        // Losango entre o nó 0 (0%) e o novo (25%): leva para 20% do trecho.
        let mid = egui::pos2(
            bar.left() + bar.width() * 0.125,
            bar.top() - theme::RAMP_MID_ROW / 2.0,
        );
        let to = egui::pos2(bar.left() + bar.width() * 0.05, mid.y);
        frame(&mut f, vec![egui::Event::PointerMoved(mid)]);
        frame(&mut f, vec![btn(mid, true)]);
        for k in 1..=5 {
            frame(
                &mut f,
                vec![egui::Event::PointerMoved(
                    mid + (to - mid) * (k as f32 / 5.0),
                )],
            );
        }
        frame(&mut f, vec![btn(to, false)]);
        assert!(
            (f.spec.stops[0].midpoint - 0.2).abs() < 0.03,
            "{}",
            f.spec.stops[0].midpoint
        );
        // Duplo clique no nó novo: sai.
        let node = egui::pos2(at.x, bar.bottom() + theme::RAMP_NODE_ROW / 2.0);
        frame(&mut f, vec![egui::Event::PointerMoved(node)]);
        for _ in 0..2 {
            frame(&mut f, vec![btn(node, true)]);
            frame(&mut f, vec![btn(node, false)]);
        }
        assert_eq!(f.spec.stops.len(), 2);
        assert!(f.changed);
    }

    /// Padrões: cada modo devolve o tipo certo; textura cria os parâmetros.
    #[test]
    fn pattern_modes() {
        let mut doc = DocState::new_blank();
        let mut f = FillDialog::new(None, false, None);
        for (m, k) in [
            (FillMode::TwoColor, "two-color"),
            (FillMode::Texture, "texture"),
            (FillMode::Vector, "vector"),
        ] {
            f.set_mode(m);
            run(&mut f, &mut doc);
            let Some(Fill::Pattern(p)) = f.result() else {
                panic!()
            };
            assert_eq!(p.kind, k);
        }
        assert!(f.pattern.texture.is_some());
        let again = FillDialog::new(f.result(), false, None);
        assert_eq!(again.mode, FillMode::Vector);
    }

    /// Capturas da janela em cada modo (para revisão visual).
    #[test]
    #[ignore]
    fn shot_fill_dialog() {
        let mut doc = DocState::new_blank();
        let pals: PaletteList = vec![(
            "Teste".to_string(),
            (0..24)
                .map(|i| {
                    (
                        format!("C{i}"),
                        Color::rgb(i as f64 / 24.0, 0.4, 1.0 - i as f64 / 24.0),
                    )
                })
                .collect(),
        )];
        let mut cases: Vec<(&str, FillDialog)> = Vec::new();
        let mut g = FillDialog::new(
            Some(Fill::solid(Color::cmyk(0.0, 0.6, 1.0, 0.0))),
            false,
            None,
        );
        g.set_mode(FillMode::Gradient);
        g.spec
            .stops
            .insert(1, stop(0.45, Color::rgb(1.0, 0.85, 0.0)));
        g.spec.stops[0].midpoint = 0.3;
        g.sel_stop = 1;
        cases.push(("gradiente", g));
        cases.push((
            "uniforme",
            FillDialog::new(Some(Fill::solid(Color::rgb(0.2, 0.5, 0.9))), false, None),
        ));
        let mut t = FillDialog::new(None, false, None);
        t.set_mode(FillMode::Texture);
        cases.push(("textura", t));
        let mut d = FillDialog::new(None, false, None);
        d.set_mode(FillMode::TwoColor);
        cases.push(("duas-cores", d));
        for (name, mut f) in cases {
            f.palettes = pals.clone();
            let ctx = crate::theme::test_ctx();
            let mut shot = crate::uishot::Shot::default();
            let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(1366.0, 768.0));
            let mut defaults = Defaults::default();
            for k in 0..40 {
                let input = egui::RawInput {
                    screen_rect: Some(screen),
                    time: Some(k as f64 / 30.0),
                    ..Default::default()
                };
                let mut out = ctx.run_ui(input, |ui| {
                    f.show(ui.ctx(), &mut doc, &mut defaults);
                });
                if k == 39 {
                    shot.save(&ctx, out, &format!("preenchimento-{name}"));
                } else {
                    shot.textures(&mut out);
                }
            }
        }
    }

    /// Uniforme e nenhum desenham; modos indisponíveis para os novos objetos.
    #[test]
    fn solid_none_and_defaults() {
        let mut doc = DocState::new_blank();
        let mut f = FillDialog::new(Some(Fill::solid(Color::rgb(0.8, 0.7, 0.1))), true, None);
        assert_eq!(f.mode, FillMode::Solid);
        f.palettes = vec![("P".into(), vec![("A".into(), Color::black())])];
        run(&mut f, &mut doc);
        f.set_mode(FillMode::None);
        run(&mut f, &mut doc);
        assert_eq!(f.result(), None);
    }
}
