//! Janelas de edição do contorno (GD-OUT-01..04, GD-OUT-07) e do
//! preenchimento (GD-FIL-01/02), abertas por duplo clique nos indicadores
//! da barra de status (GD-UI-01).
//!
//! As janelas editam um rascunho; nada muda no documento até "OK". Ao
//! confirmar, só os campos alterados são aplicados, para não sobrescrever o
//! que for diferente entre os objetos de uma seleção múltipla. Sem seleção,
//! a janela edita o estilo dos próximos objetos.

use eframe::egui::{self, Color32, Vec2};
use graphdraw_core::edit::ops;
use graphdraw_core::model::{
    Color, Document, Fill, FillRule, Node, Object, ObjectKind, Page, PathData, Stroke, Subpath,
};
use graphdraw_core::render::raster::{self, RasterOptions};
use graphdraw_core::render::{ImageCache, SceneOptions, build_scene};
use kurbo::Affine;
use serde_json::Map;

use crate::canvas::Defaults;
use crate::doc::{DocState, default_stroke};
use crate::i18n::tr;
use crate::icons;
use crate::theme;
use crate::widgets::{color_editor, fmt_mm, length_field, to_color32};

#[allow(clippy::large_enum_variant)]
pub enum StyleDialog {
    Stroke(StrokeDialog),
    Fill(FillDialog),
}

/// Estilo mostrado nos indicadores e usado como ponto de partida das
/// janelas: o do primeiro objeto selecionado que tenha preenchimento e
/// contorno próprios (descendo nos grupos), ou o dos próximos objetos.
pub fn current_style(doc: &DocState, defaults: &Defaults) -> (Option<Fill>, Option<Stroke>, bool) {
    if doc.selection.is_empty() {
        return (defaults.fill.clone(), defaults.stroke.clone(), true);
    }
    fn leaf(o: &Object) -> Option<&Object> {
        match &o.kind {
            ObjectKind::Group(g) => g.children.iter().find_map(leaf),
            ObjectKind::Image(_) | ObjectKind::Unknown { .. } => None,
            _ => Some(o),
        }
    }
    match doc.selected_objects().into_iter().find_map(leaf) {
        Some(o) => (o.fill.clone(), o.stroke.clone(), false),
        None => (None, None, false),
    }
}

impl StyleDialog {
    pub fn stroke(doc: &DocState, defaults: &Defaults) -> StyleDialog {
        let (_, stroke, for_defaults) = current_style(doc, defaults);
        StyleDialog::Stroke(StrokeDialog::new(stroke, for_defaults))
    }

    pub fn fill(doc: &DocState, defaults: &Defaults) -> StyleDialog {
        let (fill, _, for_defaults) = current_style(doc, defaults);
        StyleDialog::Fill(FillDialog::new(fill, for_defaults))
    }

    /// Mostra a janela; devolve `true` quando ela foi fechada.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        doc: &mut DocState,
        defaults: &mut Defaults,
    ) -> bool {
        match self {
            StyleDialog::Stroke(d) => d.show(ctx, doc, defaults),
            StyleDialog::Fill(d) => d.show(ctx, doc, defaults),
        }
    }
}

enum Close {
    No,
    Cancel,
    Ok,
}

fn buttons(ui: &mut egui::Ui) -> Close {
    let mut out = Close::No;
    // Rodapé (DESIGN-SYSTEM 4.4): Cancelar (secundário) e OK (primário) à direita.
    crate::widgets::footer(ui, |ui| {
        if ui
            .add(crate::widgets::primary_button(tr("OK", "OK")))
            .clicked()
        {
            out = Close::Ok;
        }
        if ui
            .add(crate::widgets::secondary_button(tr("Cancelar", "Cancel")))
            .clicked()
        {
            out = Close::Cancel;
        }
    });
    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        out = Close::Cancel;
    }
    out
}

fn section(ui: &mut egui::Ui, title: &str, body: impl FnOnce(&mut egui::Ui)) {
    ui.label(egui::RichText::new(title).text_style(theme::section_style()));
    ui.indent(title, body);
    ui.add_space(theme::SPACE_300);
}

/// Grupo de botões de ícone mutuamente exclusivos.
fn icon_choice(
    ui: &mut egui::Ui,
    value: &mut String,
    options: &[(&str, &str, &str)],
    enabled: bool,
) {
    ui.horizontal(|ui| {
        for (key, icon, tip) in options {
            if icons::button(ui, icon, tip, value == key, enabled).clicked() {
                *value = (*key).to_string();
            }
        }
    });
}

// ---- contorno ------------------------------------------------------------------

/// Padrões de traço em múltiplos da espessura (mínimo 0,25 mm, para que
/// fio de cabelo também tenha tracejado visível).
const DASHES: &[(&str, &str, &[f64])] = &[
    ("Sólido", "Solid", &[]),
    ("Tracejado", "Dashed", &[4.0, 2.0]),
    ("Traço longo", "Long dash", &[8.0, 3.0]),
    ("Pontilhado", "Dotted", &[1.0, 2.0]),
    ("Traço e ponto", "Dash-dot", &[5.0, 2.0, 1.0, 2.0]),
];
const CUSTOM: usize = DASHES.len();

fn dash_unit(width: f64) -> f64 {
    width.max(250.0)
}

fn dash_for(preset: usize, width: f64) -> Option<Vec<f64>> {
    let p = DASHES.get(preset)?.2;
    (!p.is_empty()).then(|| p.iter().map(|k| k * dash_unit(width)).collect())
}

fn detect_dash(s: &Stroke) -> usize {
    match &s.dash {
        None => 0,
        Some(d) => DASHES
            .iter()
            .position(|(_, _, p)| {
                p.len() == d.len()
                    && p.iter()
                        .zip(d)
                        .all(|(k, v)| (k * dash_unit(s.width) - v).abs() < 1.0)
            })
            .unwrap_or(CUSTOM),
    }
}

fn dash_text(d: &Option<Vec<f64>>) -> String {
    d.as_ref()
        .map(|d| d.iter().map(|v| fmt_mm(*v)).collect::<Vec<_>>().join(" "))
        .unwrap_or_default()
}

/// "4 2" (mm) → µm; aceita vírgula decimal.
fn parse_dash(t: &str) -> Option<Vec<f64>> {
    let v: Option<Vec<f64>> = t
        .split_whitespace()
        .map(|s| {
            s.replace(',', ".")
                .parse::<f64>()
                .ok()
                .filter(|x| *x >= 0.0)
        })
        .map(|x| x.map(|mm| mm * 1000.0))
        .collect();
    v.filter(|v| !v.is_empty() && v.iter().any(|x| *x > 0.0))
}

pub struct StrokeDialog {
    initial: Option<Stroke>,
    draft: Stroke,
    enabled: bool,
    preset: usize,
    custom: String,
    for_defaults: bool,
    preview: Option<((Stroke, bool), egui::TextureHandle)>,
    /// Área ocupada no último quadro (usada nos testes de layout).
    rect: egui::Rect,
}

impl StrokeDialog {
    fn new(initial: Option<Stroke>, for_defaults: bool) -> StrokeDialog {
        let draft = initial.clone().unwrap_or_else(default_stroke);
        StrokeDialog {
            enabled: initial.is_some(),
            preset: detect_dash(&draft),
            custom: dash_text(&draft.dash),
            initial,
            draft,
            for_defaults,
            preview: None,
            rect: egui::Rect::NOTHING,
        }
    }

    fn result(&self) -> Option<Stroke> {
        self.enabled.then(|| self.draft.clone())
    }

    fn show(&mut self, ctx: &egui::Context, doc: &mut DocState, defaults: &mut Defaults) -> bool {
        let title = if self.for_defaults {
            tr("Contorno dos novos objetos", "Outline for new objects")
        } else {
            tr("Contorno", "Outline")
        };
        let resp =
            crate::widgets::modal(ctx, egui::Id::unique("janela-contorno")).show(ctx, |ui| {
                ui.set_width(560.0);
                ui.heading(title);
                ui.separator();
                // Sem separador vertical: ele pede a altura toda e o modal sai da tela.
                let max_h = (ui.ctx().content_rect().height() * theme::MODAL_MAX_HEIGHT).max(240.0);
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| {
                        ui.set_width(300.0);
                        egui::ScrollArea::vertical()
                            .max_height(max_h)
                            .auto_shrink([false, true])
                            .show(ui, |ui| self.form(ui));
                    });
                    ui.add_space(theme::SPACE_400);
                    ui.vertical(|ui| {
                        ui.label(tr("Prévia", "Preview"));
                        self.preview(ui);
                        ui.weak(tr(
                            "Prévia em tamanho real aproximado (cerca de 4 px por mm).",
                            "Approximately real-size preview (about 4 px per mm).",
                        ));
                    });
                });
                ui.separator();
                buttons(ui)
            });
        self.rect = resp.response.rect;
        match resp.inner {
            Close::No => false,
            Close::Cancel => true,
            Close::Ok => {
                self.apply(doc, defaults);
                true
            }
        }
    }

    fn form(&mut self, ui: &mut egui::Ui) {
        ui.checkbox(&mut self.enabled, tr("Com contorno", "Has outline"));
        let on = self.enabled;
        ui.add_enabled_ui(on, |ui| {
            let s = &mut self.draft;
            section(ui, tr("Espessura", "Width"), |ui| {
                ui.horizontal(|ui| {
                    let hair = s.width <= 0.0;
                    if !hair
                        && let Some(w) = length_field(ui, "dlg-espessura", s.width, None) {
                            s.width = w.max(0.0);
                            if self.preset > 0 && self.preset < CUSTOM {
                                s.dash = dash_for(self.preset, s.width);
                            }
                        }
                    let mut h = hair;
                    if ui
                        .checkbox(&mut h, tr("Fio de cabelo", "Hairline"))
                        .on_hover_text(tr(
                            "A linha mais fina que o dispositivo consegue imprimir",
                            "The thinnest line the output device can print",
                        ))
                        .changed()
                    {
                        s.width = if h { 0.0 } else { 200.0 };
                    }
                });
                ui.weak(tr(
                    "Aceita contas e unidades: 0,5  ·  1pt  ·  0.02in",
                    "Accepts math and units: 0.5  ·  1pt  ·  0.02in",
                ));
            });
            section(ui, tr("Cor", "Color"), |ui| {
                if let Some(c) = color_editor(ui, &s.color) {
                    s.color = c;
                }
                let mut op = (s.opacity * 100.0).round();
                if ui
                    .add(egui::Slider::new(&mut op, 0.0..=100.0).text(tr("Opacidade %", "Opacity %")))
                    .changed()
                {
                    s.opacity = op / 100.0;
                }
            });
            section(ui, tr("Estilo da linha", "Line style"), |ui| {
                let names: Vec<&str> = DASHES
                    .iter()
                    .map(|d| tr(d.0, d.1))
                    .chain([tr("Personalizado", "Custom")])
                    .collect();
                let mut p = self.preset;
                egui::ComboBox::from_id_salt("dlg-tracejado")
                    .selected_text(names[p])
                    .width(160.0)
                    .show_ui(ui, |ui| {
                        for (i, n) in names.iter().enumerate() {
                            ui.horizontal(|ui| {
                                dash_sample(ui, i);
                                ui.selectable_value(&mut p, i, *n);
                            });
                        }
                    });
                if p != self.preset {
                    self.preset = p;
                    if p < CUSTOM {
                        s.dash = dash_for(p, s.width);
                        if p == 3 {
                            s.cap = "round".into();
                        }
                    } else if s.dash.is_none() {
                        s.dash = dash_for(1, s.width);
                    }
                    self.custom = dash_text(&s.dash);
                }
                if self.preset == CUSTOM {
                    ui.horizontal(|ui| {
                        ui.label(tr("Traço e espaço (mm):", "Dash and gap (mm):"));
                        let r = ui.add(egui::TextEdit::singleline(&mut self.custom).desired_width(110.0));
                        if r.lost_focus() {
                            match parse_dash(&self.custom) {
                                Some(d) => s.dash = Some(d),
                                None => self.custom = dash_text(&s.dash),
                            }
                        }
                    });
                    ui.weak(tr(
                        "Valores alternados de traço e espaço, ex.: 3 1 0,5 1",
                        "Alternating dash and gap values, e.g. 3 1 0.5 1",
                    ));
                }
            });
            section(ui, tr("Cantos", "Corners"), |ui| {
                icon_choice(
                    ui,
                    &mut s.join,
                    &[
                        ("miter", "join-miter", tr("Agudo", "Miter")),
                        ("round", "join-round", tr("Arredondado", "Round")),
                        ("bevel", "join-bevel", tr("Chanfrado", "Bevel")),
                    ],
                    true,
                );
                ui.add_enabled_ui(s.join == "miter", |ui| {
                    ui.horizontal(|ui| {
                        ui.label(tr("Limite do canto agudo", "Miter limit"));
                        ui.add(egui::DragValue::new(&mut s.miter_limit).range(1.0..=100.0).speed(0.1));
                    })
                    .response
                    .on_hover_text(tr(
                        "Cantos mais fechados que isso viram chanfro, para a ponta não crescer demais",
                        "Sharper corners than this become bevels so the tip does not grow too long",
                    ));
                });
            });
            section(ui, tr("Pontas da linha", "Line ends"), |ui| {
                icon_choice(
                    ui,
                    &mut s.cap,
                    &[
                        ("butt", "cap-butt", tr("Reta, no fim do caminho", "Flat, at the path end")),
                        ("round", "cap-round", tr("Redonda", "Round")),
                        ("square", "cap-square", tr("Quadrada, passa do fim", "Square, past the end")),
                    ],
                    true,
                );
            });
            section(ui, tr("Caneta caligráfica", "Calligraphic pen"), |ui| {
                ui.horizontal(|ui| {
                    ui.label(tr("Bico", "Nib"));
                    let mut pct = (s.nib_stretch * 100.0).round();
                    if ui
                        .add(egui::DragValue::new(&mut pct).range(1.0..=100.0).suffix("%"))
                        .on_hover_text(tr(
                            "Largura do bico; 100% = bico redondo (contorno comum)",
                            "Nib width; 100% = round nib (regular outline)",
                        ))
                        .changed()
                    {
                        s.nib_stretch = pct / 100.0;
                    }
                    ui.label(tr("Ângulo", "Angle"));
                    ui.add_enabled(
                        s.nib_stretch < 0.999,
                        egui::DragValue::new(&mut s.nib_angle).range(-360.0..=360.0).suffix("°"),
                    );
                });
            });
            section(ui, tr("Pontas de seta", "Arrowheads"), |ui| {
                arrow_row(ui, "seta-inicio", tr("Início", "Start"), &mut s.start_arrow);
                arrow_row(ui, "seta-fim", tr("Fim", "End"), &mut s.end_arrow);
                ui.weak(tr(
                    "Só em curvas abertas; o tamanho acompanha a espessura.",
                    "Open curves only; the size follows the width.",
                ));
            });
            section(ui, tr("Posição", "Position"), |ui| {
                icon_choice(
                    ui,
                    &mut s.position,
                    &[
                        ("center", "pos-center", tr("Centrada na borda", "Centered on the edge")),
                        ("inside", "pos-inside", tr("Por dentro da borda", "Inside the edge")),
                        ("outside", "pos-outside", tr("Por fora da borda", "Outside the edge")),
                    ],
                    true,
                );
                ui.weak(tr(
                    "Curvas abertas sempre usam centrada.",
                    "Open curves always use centered.",
                ));
            });
            section(ui, tr("Opções", "Options"), |ui| {
                ui.horizontal(|ui| {
                    icons::button(ui, "behind-fill", "", s.behind_fill, false);
                    ui.checkbox(&mut s.behind_fill, tr("Atrás do preenchimento", "Behind the fill"))
                        .on_hover_text(tr(
                            "O preenchimento cobre a metade de dentro do contorno",
                            "The fill covers the inner half of the outline",
                        ));
                });
                ui.horizontal(|ui| {
                    icons::button(ui, "scale-stroke", "", s.scale_with_object, false);
                    ui.checkbox(
                        &mut s.scale_with_object,
                        tr("Escalar com o objeto", "Scale with object"),
                    )
                    .on_hover_text(tr(
                        "Ao aumentar ou reduzir o objeto, a espessura acompanha",
                        "When the object is resized, the width follows",
                    ));
                });
            });
        });
    }

    fn preview(&mut self, ui: &mut egui::Ui) {
        let key = (self.draft.clone(), self.enabled);
        let stale = self.preview.as_ref().is_none_or(|(k, _)| *k != key);
        if stale {
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
            ui.add(egui::Image::new((tex.id(), size)));
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

/// Copia para `s` só os campos em que `new` difere de `old`.
fn merge_changes(s: &mut Stroke, old: &Stroke, new: &Stroke) {
    macro_rules! take {
        ($($f:ident),*) => {$(
            if new.$f != old.$f {
                s.$f = new.$f.clone();
            }
        )*};
    }
    take!(
        width,
        color,
        opacity,
        dash,
        dash_offset,
        join,
        miter_limit,
        cap,
        position,
        scale_with_object,
        behind_fill,
        start_arrow,
        end_arrow,
        nib_stretch,
        nib_angle
    );
}

/// Nome da forma de ponta na interface.
fn arrow_label(shape: &str) -> &'static str {
    match shape {
        "open" => tr("Aberta", "Open"),
        "barbed" => tr("Farpada", "Barbed"),
        "circle" => tr("Círculo", "Circle"),
        "square" => tr("Quadrado", "Square"),
        "bar" => tr("Barra", "Bar"),
        _ => tr("Triângulo", "Triangle"),
    }
}

/// Uma linha de ponta de seta (GD-OUT-05): forma, escala, giro, espelho e
/// afastamento.
fn arrow_row(
    ui: &mut egui::Ui,
    id: &str,
    label: &str,
    a: &mut Option<graphdraw_core::model::Arrow>,
) {
    use graphdraw_core::model::Arrow;
    ui.horizontal(|ui| {
        ui.label(label);
        let text = a
            .as_ref()
            .map_or(tr("Nenhuma", "None"), |x| arrow_label(&x.shape));
        egui::ComboBox::from_id_salt(id)
            .selected_text(text)
            .width(theme::ARROW_COMBO_WIDTH)
            .show_ui(ui, |ui| {
                if ui
                    .selectable_label(a.is_none(), tr("Nenhuma", "None"))
                    .clicked()
                {
                    *a = None;
                }
                for shape in graphdraw_core::render::arrows::SHAPES {
                    let on = a.as_ref().is_some_and(|x| x.shape == shape);
                    if ui.selectable_label(on, arrow_label(shape)).clicked() {
                        match a {
                            Some(x) => x.shape = shape.into(),
                            None => *a = Some(Arrow::new(shape)),
                        }
                    }
                }
            });
        if let Some(x) = a {
            let mut pct = (x.scale * 100.0).round();
            if ui
                .add(
                    egui::DragValue::new(&mut pct)
                        .range(5.0..=5000.0)
                        .suffix("%"),
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
            if let Some(v) = crate::widgets::labeled_length(
                ui,
                tr("Afast.", "Offset"),
                &format!("{id}-afast"),
                x.offset,
                None,
            ) {
                x.offset = v;
            }
        }
    });
}

/// Amostra do padrão de traço para a lista.
fn dash_sample(ui: &mut egui::Ui, preset: usize) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(60.0, 12.0), egui::Sense::hover());
    let color = ui.visuals().text_color();
    let y = rect.center().y;
    let stroke = egui::Stroke::new(2.0, color);
    match DASHES.get(preset).map(|d| d.2) {
        Some([]) => {
            ui.painter().line_segment(
                [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
                stroke,
            );
        }
        Some(p) => {
            let unit = 2.0;
            let mut x = rect.left();
            let mut i = 0;
            while x < rect.right() {
                let len = p[i % p.len()] as f32 * unit;
                if i % 2 == 0 {
                    let end = (x + len.max(1.0)).min(rect.right());
                    ui.painter()
                        .line_segment([egui::pos2(x, y), egui::pos2(end, y)], stroke);
                }
                x += len.max(1.0);
                i += 1;
            }
        }
        None => {
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "…",
                theme::font_body(),
                color,
            );
        }
    }
}

const MM: f64 = 1000.0;
const PREVIEW_PX_PER_MM: f64 = 4.0;

fn poly(points: &[(f64, f64)], closed: bool) -> PathData {
    PathData {
        subpaths: vec![Subpath {
            closed,
            nodes: points
                .iter()
                .map(|(x, y)| Node::corner(x * MM, y * MM))
                .collect(),
            extra: Map::new(),
        }],
        fill_rule: FillRule::Nonzero,
        extra: Map::new(),
    }
}

/// Desenha um triângulo preenchido (cantos fechados) e um zigue-zague aberto
/// (pontas e cantos) com o contorno dado, pelo mesmo renderizador da
/// exportação.
fn stroke_preview(stroke: Option<&Stroke>) -> egui::ColorImage {
    let (w, h) = (70.0, 42.0);
    let mut page = Page::new(w * MM, h * MM);
    let mut tri = Object::new(ObjectKind::Path(poly(
        &[(7.0, 8.0), (31.0, 8.0), (19.0, 34.0)],
        true,
    )));
    tri.fill = Some(Fill::solid(Color::rgb(1.0, 0.85, 0.3)));
    tri.stroke = stroke.cloned();
    let mut zig = Object::new(ObjectKind::Path(poly(
        &[(40.0, 10.0), (47.0, 32.0), (55.0, 12.0), (63.0, 32.0)],
        false,
    )));
    zig.stroke = stroke.cloned();
    page.layers[0].objects = vec![tri, zig];
    let mut doc = Document::new();
    doc.data.pages.push(page);
    let scene = build_scene(
        &doc,
        &doc.data.pages[0],
        SceneOptions { print: true },
        &ImageCache::default(),
    );
    let s = PREVIEW_PX_PER_MM / MM;
    let (pw, ph) = (
        (w * PREVIEW_PX_PER_MM) as u32,
        (h * PREVIEW_PX_PER_MM) as u32,
    );
    let view = Affine::new([s, 0.0, 0.0, -s, 0.0, h * MM * s]);
    let opts = RasterOptions {
        antialias: true,
        background: Some([255, 255, 255, 255]),
    };
    match raster::render(&scene, view, pw, ph, &opts) {
        Some(pm) => {
            egui::ColorImage::from_rgba_premultiplied([pw as usize, ph as usize], pm.data())
        }
        None => egui::ColorImage::new([1, 1], vec![Color32::WHITE]),
    }
}

// ---- preenchimento --------------------------------------------------------------

/// Tipo de preenchimento na janela.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FillMode {
    None,
    Solid,
    Linear,
    Radial,
}

pub struct FillDialog {
    initial: Option<Fill>,
    mode: FillMode,
    color: Color,
    opacity: f64,
    overprint: bool,
    /// Gradiente (GD-FIL-03): paradas, parada escolhida, ângulo, repetição.
    stops: Vec<graphdraw_core::model::GradientStop>,
    sel_stop: usize,
    angle: f64,
    spread: String,
    /// O preenchimento atual é de um tipo que esta janela ainda não edita.
    other_kind: bool,
    changed: bool,
    for_defaults: bool,
    rect: egui::Rect,
    /// Prévias desenhadas pelo motor (amostra e barra de paradas).
    sample_tex: Option<(Option<Fill>, egui::TextureHandle)>,
    bar_tex: Option<(
        Vec<graphdraw_core::model::GradientStop>,
        egui::TextureHandle,
    )>,
}

fn stop(offset: f64, color: Color) -> graphdraw_core::model::GradientStop {
    graphdraw_core::model::GradientStop {
        offset,
        color,
        opacity: 1.0,
        extra: Map::new(),
    }
}

impl FillDialog {
    fn new(initial: Option<Fill>, for_defaults: bool) -> FillDialog {
        let white = Color::cmyk(0.0, 0.0, 0.0, 0.0);
        let mut d = FillDialog {
            initial: initial.clone(),
            mode: FillMode::None,
            color: white.clone(),
            opacity: 1.0,
            overprint: false,
            stops: vec![stop(0.0, Color::cmyk(0.0, 0.0, 0.0, 1.0)), stop(1.0, white)],
            sel_stop: 0,
            angle: 0.0,
            spread: "pad".into(),
            other_kind: false,
            changed: false,
            for_defaults,
            rect: egui::Rect::NOTHING,
            sample_tex: None,
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
                let spec = ops::gradient_spec(g);
                d.mode = if spec.kind == "radial" {
                    FillMode::Radial
                } else {
                    FillMode::Linear
                };
                if spec.stops.len() >= 2 {
                    d.stops = spec.stops;
                }
                d.angle = spec.angle;
                d.spread = spec.spread;
                d.color = d.stops[0].color.clone();
            }
            Some(_) => d.other_kind = true,
        }
        d
    }

    fn spec(&self) -> ops::GradientSpec {
        let mut stops = self.stops.clone();
        stops.sort_by(|a, b| a.offset.total_cmp(&b.offset));
        ops::GradientSpec {
            kind: if self.mode == FillMode::Radial {
                "radial"
            } else {
                "linear"
            }
            .into(),
            stops,
            angle: self.angle,
            spread: self.spread.clone(),
        }
    }

    /// Preenchimento para a amostra (gradiente ajustado à caixa `b`).
    fn result_in(&self, b: kurbo::Rect) -> Option<Fill> {
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
            FillMode::Linear | FillMode::Radial => {
                Some(Fill::Gradient(ops::gradient_for(&self.spec(), b)))
            }
        }
    }

    fn result(&self) -> Option<Fill> {
        self.result_in(kurbo::Rect::new(0.0, 0.0, 1.0, 1.0))
    }

    fn show(&mut self, ctx: &egui::Context, doc: &mut DocState, defaults: &mut Defaults) -> bool {
        let title = if self.for_defaults {
            tr("Preenchimento dos novos objetos", "Fill for new objects")
        } else {
            tr("Preenchimento", "Fill")
        };
        let resp =
            crate::widgets::modal(ctx, egui::Id::unique("janela-preenchimento")).show(ctx, |ui| {
                ui.set_width(theme::FILL_DIALOG_WIDTH);
                ui.heading(title);
                ui.separator();
                ui.horizontal(|ui| {
                    let mut modes = vec![
                        (
                            "fill-none",
                            tr("Sem preenchimento", "No fill"),
                            FillMode::None,
                        ),
                        ("tool-rect", tr("Uniforme", "Uniform"), FillMode::Solid),
                    ];
                    // Gradiente depende da caixa de cada objeto: só para a seleção.
                    if !self.for_defaults {
                        modes.push((
                            "fill-linear",
                            tr("Gradiente linear", "Linear gradient"),
                            FillMode::Linear,
                        ));
                        modes.push((
                            "fill-radial",
                            tr("Gradiente radial", "Radial gradient"),
                            FillMode::Radial,
                        ));
                    }
                    for (icon, tip, m) in &modes {
                        if icons::button(ui, icon, tip, self.mode == *m, true).clicked()
                            && self.mode != *m
                        {
                            if matches!(m, FillMode::Linear | FillMode::Radial)
                                && self.mode == FillMode::Solid
                            {
                                // Começa do uniforme atual até o branco.
                                self.stops[0].color = self.color.clone();
                            }
                            self.mode = *m;
                            self.changed = true;
                        }
                    }
                    let name = modes
                        .iter()
                        .find(|(_, _, m)| *m == self.mode)
                        .map_or("", |(_, t, _)| *t);
                    ui.label(name);
                });
                if self.other_kind && !self.changed {
                    ui.colored_label(
                        theme::ACCENT_WARNING,
                        tr(
                            "O preenchimento atual não é editável aqui; mudar algo o substitui.",
                            "The current fill is not editable here; changing anything replaces it.",
                        ),
                    );
                }
                ui.separator();
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| {
                        ui.set_width(theme::FILL_EDITOR_WIDTH);
                        match self.mode {
                            FillMode::None => {
                                ui.weak(tr("Sem preenchimento", "No fill"));
                            }
                            FillMode::Solid => self.solid_ui(ui),
                            FillMode::Linear | FillMode::Radial => self.gradient_ui(ui),
                        }
                    });
                    self.sample(ui);
                });
                ui.separator();
                buttons(ui)
            });
        self.rect = resp.response.rect;
        match resp.inner {
            Close::No => false,
            Close::Cancel => true,
            Close::Ok => {
                if self.changed {
                    let sel = doc.selection.clone();
                    match self.mode {
                        FillMode::Linear | FillMode::Radial if !self.for_defaults => {
                            ops::set_gradient(&mut doc.editor, &sel, &self.spec());
                        }
                        _ => {
                            let r = self.result();
                            if self.for_defaults {
                                defaults.fill = r;
                            } else {
                                ops::set_fill(&mut doc.editor, &sel, Some(r.unwrap_or(Fill::None)));
                            }
                        }
                    }
                }
                true
            }
        }
    }

    fn solid_ui(&mut self, ui: &mut egui::Ui) {
        if let Some(c) = color_editor(ui, &self.color) {
            self.color = c;
            self.changed = true;
        }
        let mut op = (self.opacity * 100.0).round();
        if ui
            .add(egui::Slider::new(&mut op, 0.0..=100.0).text(tr("Opacidade %", "Opacity %")))
            .changed()
        {
            self.opacity = op / 100.0;
            self.changed = true;
        }
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

    /// Paradas, ângulo e repetição (GD-FIL-03).
    fn gradient_ui(&mut self, ui: &mut egui::Ui) {
        self.stops_bar(ui);
        ui.horizontal(|ui| {
            if ui.small_button(tr("+ Parada", "+ Stop")).clicked() {
                // Entre a escolhida e a seguinte.
                let mut sorted: Vec<f64> = self.stops.iter().map(|s| s.offset).collect();
                sorted.sort_by(|a, b| a.total_cmp(b));
                let cur = self.stops[self.sel_stop].offset;
                let next = sorted.iter().copied().find(|o| *o > cur).unwrap_or(1.0);
                let at = if next > cur {
                    (cur + next) / 2.0
                } else {
                    (cur / 2.0).max(0.0)
                };
                self.stops
                    .push(stop(at, self.stops[self.sel_stop].color.clone()));
                self.sel_stop = self.stops.len() - 1;
                self.changed = true;
            }
            if ui
                .add_enabled(
                    self.stops.len() > 2,
                    egui::Button::new(tr("− Parada", "− Stop")).small(),
                )
                .clicked()
            {
                self.stops.remove(self.sel_stop);
                self.sel_stop = 0;
                self.changed = true;
            }
            if ui.small_button(tr("Inverter", "Reverse")).clicked() {
                for s in &mut self.stops {
                    s.offset = 1.0 - s.offset;
                }
                self.changed = true;
            }
        });
        let i = self.sel_stop.min(self.stops.len() - 1);
        let mut pos = (self.stops[i].offset * 100.0).round();
        if ui
            .add(egui::Slider::new(&mut pos, 0.0..=100.0).text(tr("Posição %", "Position %")))
            .changed()
        {
            self.stops[i].offset = pos / 100.0;
            self.changed = true;
        }
        if let Some(c) = color_editor(ui, &self.stops[i].color) {
            self.stops[i].color = c;
            self.changed = true;
        }
        let mut op = (self.stops[i].opacity * 100.0).round();
        if ui
            .add(egui::Slider::new(&mut op, 0.0..=100.0).text(tr("Opacidade %", "Opacity %")))
            .changed()
        {
            self.stops[i].opacity = op / 100.0;
            self.changed = true;
        }
        ui.horizontal(|ui| {
            if self.mode == FillMode::Linear {
                ui.label(tr("Ângulo", "Angle"));
                if ui
                    .add(
                        egui::DragValue::new(&mut self.angle)
                            .range(-360.0..=360.0)
                            .suffix("°"),
                    )
                    .changed()
                {
                    self.changed = true;
                }
            }
            ui.label(tr("Fora das pontas", "Beyond the ends"));
            let label = |s: &str| match s {
                "repeat" => tr("Repetir", "Repeat"),
                "reflect" => tr("Espelhar", "Reflect"),
                _ => tr("Estender", "Extend"),
            };
            egui::ComboBox::from_id_salt("gradiente-repeticao")
                .selected_text(label(&self.spread))
                .show_ui(ui, |ui| {
                    for k in ["pad", "repeat", "reflect"] {
                        if ui
                            .selectable_value(&mut self.spread, k.to_string(), label(k))
                            .changed()
                        {
                            self.changed = true;
                        }
                    }
                });
        });
    }

    /// Barra com o gradiente e as marcas das paradas: clicar escolhe,
    /// arrastar move, duplo clique na barra cria uma parada ali.
    fn stops_bar(&mut self, ui: &mut egui::Ui) {
        let w = theme::FILL_EDITOR_WIDTH;
        let (rect, resp) = ui.allocate_exact_size(
            Vec2::new(w, theme::GRADIENT_BAR_HEIGHT + theme::GRADIENT_MARK),
            egui::Sense::click_and_drag(),
        );
        let bar = egui::Rect::from_min_size(rect.min, Vec2::new(w, theme::GRADIENT_BAR_HEIGHT));
        let mut lin = self.spec();
        lin.kind = "linear".into();
        lin.angle = 0.0;
        if self.bar_tex.as_ref().is_none_or(|(s, _)| *s != lin.stops) {
            let ppp = ui.ctx().pixels_per_point();
            let (pw, ph) = ((w * ppp) as u32, (theme::GRADIENT_BAR_HEIGHT * ppp) as u32);
            let fill = Fill::Gradient(ops::gradient_for(
                &lin,
                kurbo::Rect::new(0.0, 0.0, pw as f64, ph as f64),
            ));
            if let Some(rgba) = raster::fill_preview(&fill, pw, ph) {
                let img =
                    egui::ColorImage::from_rgba_unmultiplied([pw as usize, ph as usize], &rgba);
                let t = ui
                    .ctx()
                    .load_texture("barra-gradiente", img, egui::TextureOptions::LINEAR);
                self.bar_tex = Some((lin.stops.clone(), t));
            }
        }
        checker(ui.painter(), bar);
        if let Some((_, t)) = &self.bar_tex {
            ui.painter().image(
                t.id(),
                bar,
                egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                Color32::WHITE,
            );
        }
        ui.painter().rect_stroke(
            bar,
            theme::RADIUS_NONE,
            theme::border_default(ui),
            egui::StrokeKind::Inside,
        );
        let x_of = |o: f64| bar.left() + (o.clamp(0.0, 1.0) as f32) * w;
        let pal = theme::palette(ui);
        for (k, s) in self.stops.iter().enumerate() {
            let x = x_of(s.offset);
            let top = bar.bottom();
            let m = theme::GRADIENT_MARK;
            let tri = vec![
                egui::pos2(x, top),
                egui::pos2(x - m / 2.0, top + m),
                egui::pos2(x + m / 2.0, top + m),
            ];
            let (fill, line) = if k == self.sel_stop {
                (theme::ACCENT_PRIMARY, pal.fg_strong)
            } else {
                (to_color32(&s.color), pal.fg_muted)
            };
            ui.painter().add(egui::Shape::convex_polygon(
                tri,
                fill,
                egui::Stroke::new(theme::BORDER_WIDTH, line),
            ));
        }
        let offset_at = |p: egui::Pos2| ((p.x - bar.left()) / w).clamp(0.0, 1.0) as f64;
        let nearest = |p: egui::Pos2, stops: &[graphdraw_core::model::GradientStop]| {
            stops
                .iter()
                .enumerate()
                .map(|(k, s)| (k, (x_of(s.offset) - p.x).abs()))
                .filter(|(_, d)| *d <= theme::HANDLE_HIT)
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(k, _)| k)
        };
        if resp.double_clicked()
            && let Some(p) = resp.interact_pointer_pos()
        {
            let o = offset_at(p);
            self.stops
                .push(stop(o, self.stops[self.sel_stop].color.clone()));
            self.sel_stop = self.stops.len() - 1;
            self.changed = true;
        } else if (resp.clicked() || resp.drag_started())
            && let Some(p) = resp.interact_pointer_pos()
            && let Some(k) = nearest(p, &self.stops)
        {
            self.sel_stop = k;
        }
        if resp.dragged()
            && let Some(p) = resp.interact_pointer_pos()
        {
            self.stops[self.sel_stop].offset = offset_at(p);
            self.changed = true;
        }
        resp.on_hover_text(tr(
            "Clique numa marca para escolher a parada · arraste para mover · duplo clique na barra cria uma parada",
            "Click a mark to pick the stop · drag to move · double-click the bar to add a stop",
        ));
    }

    /// Amostra sobre xadrez (mostra a opacidade), pelo motor de desenho.
    fn sample(&mut self, ui: &mut egui::Ui) {
        let side = theme::FILL_SAMPLE;
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(side), egui::Sense::hover());
        checker(ui.painter(), rect);
        let inner = rect.shrink(theme::SPACE_400);
        let ppp = ui.ctx().pixels_per_point();
        let px = (inner.width() * ppp) as u32;
        let fill = self.result_in(kurbo::Rect::new(0.0, 0.0, px as f64, px as f64));
        match &fill {
            None => {
                ui.painter().line_segment(
                    [inner.left_bottom(), inner.right_top()],
                    egui::Stroke::new(theme::FOCUS_WIDTH, theme::ACCENT_DANGER),
                );
            }
            Some(f) => {
                if self
                    .sample_tex
                    .as_ref()
                    .is_none_or(|(old, _)| old.as_ref() != Some(f))
                    && let Some(rgba) = raster::fill_preview(f, px, px)
                {
                    let img =
                        egui::ColorImage::from_rgba_unmultiplied([px as usize, px as usize], &rgba);
                    let t = ui.ctx().load_texture(
                        "amostra-preenchimento",
                        img,
                        egui::TextureOptions::LINEAR,
                    );
                    self.sample_tex = Some((fill.clone(), t));
                }
                if let Some((_, t)) = &self.sample_tex {
                    // Y da página para cima: a textura sai invertida na vertical.
                    ui.painter().image(
                        t.id(),
                        inner,
                        egui::Rect::from_min_max(egui::pos2(0.0, 1.0), egui::pos2(1.0, 0.0)),
                        Color32::WHITE,
                    );
                }
            }
        }
        ui.painter().rect_stroke(
            inner,
            theme::RADIUS_NONE,
            theme::border_default(ui),
            egui::StrokeKind::Middle,
        );
    }
}

/// Fundo xadrez (mostra transparência).
fn checker(p: &egui::Painter, rect: egui::Rect) {
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

/// Indicadores de preenchimento e contorno da barra de status (alinhados à
/// direita). Duplo clique abre a janela de edição correspondente.
pub fn status_indicators(
    ui: &mut egui::Ui,
    doc: &DocState,
    defaults: &Defaults,
) -> Option<StyleDialog> {
    let (fill, stroke, for_defaults) = current_style(doc, defaults);
    let mut open = None;
    let label = |ui: &mut egui::Ui, t: String| {
        ui.add(egui::Label::new(t).sense(egui::Sense::click()))
            .on_hover_text(tr("Duplo clique para editar", "Double-click to edit"))
    };
    // Da direita para a esquerda: contorno, depois preenchimento.
    let width = match &stroke {
        None => tr("nenhum", "none").to_string(),
        Some(s) if s.width <= 0.0 => tr("fio de cabelo", "hairline").to_string(),
        Some(s) => format!("{} mm", fmt_mm(s.width)),
    };
    let r = label(ui, width);
    let sw = stroke_swatch(ui, stroke.as_ref());
    let l = label(ui, tr("Contorno", "Outline").into());
    if [r, sw, l].iter().any(|r| r.double_clicked()) {
        open = Some(StyleDialog::stroke(doc, defaults));
    }
    ui.separator();
    let fw = fill_swatch(ui, fill.as_ref());
    let fl = label(ui, tr("Preench.", "Fill").into());
    if fw.double_clicked() || fl.double_clicked() {
        open = Some(StyleDialog::fill(doc, defaults));
    }
    if for_defaults {
        ui.weak(tr("Novos objetos:", "New objects:"));
    }
    open
}

fn indicator(ui: &mut egui::Ui) -> (egui::Rect, egui::Response) {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(30.0, 16.0), egui::Sense::click());
    let resp = resp.on_hover_text(tr("Duplo clique para editar", "Double-click to edit"));
    (rect, resp)
}

fn none_mark(p: &egui::Painter, rect: egui::Rect) {
    crate::widgets::no_color(p, rect);
}

fn fill_swatch(ui: &mut egui::Ui, fill: Option<&Fill>) -> egui::Response {
    let (rect, resp) = indicator(ui);
    let p = ui.painter();
    match fill {
        Some(Fill::Solid { color, .. }) => {
            p.rect_filled(rect, theme::RADIUS_XS, to_color32(color));
        }
        Some(Fill::Gradient(g)) => {
            // Duas metades com as cores das pontas, só como indicação.
            let a = g
                .stops
                .first()
                .map(|s| to_color32(&s.color))
                .unwrap_or(theme::palette(ui).fg_muted);
            let b = g
                .stops
                .last()
                .map(|s| to_color32(&s.color))
                .unwrap_or(theme::palette(ui).fg_muted);
            let (l, r) = rect.split_left_right_at_fraction(0.5);
            p.rect_filled(l, theme::RADIUS_NONE, a);
            p.rect_filled(r, theme::RADIUS_NONE, b);
        }
        _ => none_mark(p, rect),
    }
    p.rect_stroke(
        rect,
        theme::RADIUS_XS,
        theme::border_default(ui),
        egui::StrokeKind::Inside,
    );
    resp
}

fn stroke_swatch(ui: &mut egui::Ui, stroke: Option<&Stroke>) -> egui::Response {
    let (rect, resp) = indicator(ui);
    let p = ui.painter();
    match stroke {
        Some(s) => {
            p.rect_filled(rect, theme::RADIUS_XS, theme::PAGE_PAPER);
            p.rect_stroke(
                rect.shrink(theme::SPACE_050),
                theme::RADIUS_XS,
                egui::Stroke::new(3.0, to_color32(&s.color)),
                egui::StrokeKind::Inside,
            );
        }
        None => none_mark(p, rect),
    }
    p.rect_stroke(
        rect,
        theme::RADIUS_XS,
        theme::border_default(ui),
        egui::StrokeKind::Inside,
    );
    resp
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dash_presets_round_trip_and_custom_parse() {
        let mut s = Stroke::new(500.0, Color::black());
        for p in 0..CUSTOM {
            s.dash = dash_for(p, s.width);
            assert_eq!(detect_dash(&s), p);
        }
        s.dash = Some(vec![3000.0, 1000.0, 500.0]);
        assert_eq!(detect_dash(&s), CUSTOM);
        assert_eq!(
            parse_dash("3 1 0,5 1"),
            Some(vec![3000.0, 1000.0, 500.0, 1000.0])
        );
        assert_eq!(parse_dash("0 0"), None);
        assert_eq!(parse_dash("a"), None);
    }

    #[test]
    fn merge_keeps_fields_that_did_not_change() {
        let old = Stroke::new(500.0, Color::black());
        let mut new = old.clone();
        new.join = "round".into();
        // Objeto da seleção com outra espessura: só a junção muda.
        let mut other = Stroke::new(2000.0, Color::rgb(1.0, 0.0, 0.0));
        merge_changes(&mut other, &old, &new);
        assert_eq!(other.width, 2000.0);
        assert_eq!(other.color, Color::rgb(1.0, 0.0, 0.0));
        assert_eq!(other.join, "round");
    }

    /// Os dois modais cabem numa tela 1366×768 (antes, a coluna com separador
    /// vertical fazia o conteúdo sair da tela).
    #[test]
    fn dialogs_fit_on_screen() {
        let ctx = crate::theme::test_ctx();
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(1366.0, 768.0));
        let mut doc = DocState::new_blank();
        // Cores RGB mostram o seletor de cor do egui, que estica até onde
        // puder; era ele que deixava a janela do tamanho da tela.
        let mut defaults = Defaults {
            fill: Some(Fill::solid(Color::rgb(0.8, 0.7, 0.1))),
            stroke: Some(Stroke::new(3000.0, Color::rgb(0.8, 0.1, 0.1))),
            ..Default::default()
        };
        let mut dlgs = [
            StyleDialog::stroke(&doc, &defaults),
            StyleDialog::fill(&doc, &defaults),
        ];
        for d in &mut dlgs {
            for _ in 0..3 {
                let input = egui::RawInput {
                    screen_rect: Some(screen),
                    ..Default::default()
                };
                let mut out = ctx.run_ui(input, |ui| {
                    d.show(ui.ctx(), &mut doc, &mut defaults);
                });
                out.textures_delta.clear();
            }
            let r = match d {
                StyleDialog::Stroke(s) => s.rect,
                StyleDialog::Fill(f) => f.rect,
            };
            assert!(r.is_positive(), "modal não desenhado: {r:?}");
            assert!(screen.contains_rect(r), "modal fora da tela: {r:?}");
            assert!(r.height() < 740.0, "modal alto demais: {r:?}");
        }
    }

    /// GD-FIL-03: a janela abre um gradiente existente no modo certo,
    /// cabe na tela e a amostra é desenhada.
    #[test]
    fn gradient_fill_dialog_round_trip_and_fits() {
        let ctx = crate::theme::test_ctx();
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(1366.0, 768.0));
        let mut doc = DocState::new_blank();
        let spec = ops::GradientSpec {
            kind: "radial".into(),
            stops: vec![
                stop(0.0, Color::rgb(1.0, 0.0, 0.0)),
                stop(0.4, Color::rgb(0.0, 1.0, 0.0)),
                stop(1.0, Color::rgb(0.0, 0.0, 1.0)),
            ],
            angle: 0.0,
            spread: "reflect".into(),
        };
        let g = Fill::Gradient(ops::gradient_for(
            &spec,
            kurbo::Rect::new(0.0, 0.0, 10.0, 10.0),
        ));
        let mut f = FillDialog::new(Some(g), false);
        assert_eq!(f.mode, FillMode::Radial);
        assert_eq!((f.stops.len(), f.spread.as_str()), (3, "reflect"));
        let mut defaults = Defaults::default();
        for _ in 0..3 {
            let input = egui::RawInput {
                screen_rect: Some(screen),
                ..Default::default()
            };
            let mut out = ctx.run_ui(input, |ui| {
                f.show(ui.ctx(), &mut doc, &mut defaults);
            });
            out.textures_delta.clear();
        }
        assert!(
            screen.contains_rect(f.rect) && f.rect.height() < 740.0,
            "{:?}",
            f.rect
        );
        assert!(
            f.sample_tex.is_some() && f.bar_tex.is_some(),
            "prévias desenhadas"
        );
        assert_eq!(f.spec().kind, "radial");
    }

    #[test]
    fn preview_renders_stroke_pixels() {
        let img = stroke_preview(Some(&Stroke::new(1000.0, Color::rgb(1.0, 0.0, 0.0))));
        assert!(img.pixels.iter().any(|p| p.r() > 200 && p.g() < 60));
        let none = stroke_preview(None);
        assert!(!none.pixels.iter().any(|p| p.r() > 200 && p.g() < 60));
    }
}
