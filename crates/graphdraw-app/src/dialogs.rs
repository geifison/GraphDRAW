//! Janelas de edição do contorno (GD-OUT-01..04, GD-OUT-07) e do
//! preenchimento (GD-FIL-01/02), abertas por duplo clique nos indicadores
//! da barra de status (GD-UI-01).
//!
//! As janelas editam um rascunho; nada muda no documento até "OK". Ao
//! confirmar, só os campos alterados são aplicados, para não sobrescrever o
//! que for diferente entre os objetos de uma seleção múltipla. Sem seleção,
//! a janela edita o estilo dos próximos objetos.

use eframe::egui::{self, Color32, Vec2};
use graphdraw_core::model::{
    Color, Document, Fill, FillRule, Node, Object, ObjectKind, Page, PathData, Stroke, Subpath,
};
use graphdraw_core::render::raster::{self, RasterOptions};
use graphdraw_core::render::{ImageCache, SceneOptions, build_scene};
use kurbo::Affine;
use serde_json::Map;

use crate::canvas::Defaults;
use crate::doc::DocState;
use crate::i18n::tr;
use crate::theme;
use crate::widgets::to_color32;

#[allow(clippy::large_enum_variant)]
pub enum StyleDialog {
    Stroke(crate::strokeui::StrokeDialog),
    Fill(FillDialog),
}

/// Estilo mostrado nos indicadores e usado como ponto de partida das
/// janelas: o do primeiro objeto selecionado que tenha preenchimento e
/// contorno próprios (descendo nos grupos), ou o dos próximos objetos.
/// Primeira folha (não grupo) com preenchimento próprio.
fn first_leaf(o: &Object) -> Option<&Object> {
    match &o.kind {
        ObjectKind::Group(g) => g.children.iter().find_map(first_leaf),
        ObjectKind::Image(_) | ObjectKind::Unknown { .. } => None,
        _ => Some(o),
    }
}

pub fn current_style(doc: &DocState, defaults: &Defaults) -> (Option<Fill>, Option<Stroke>, bool) {
    if doc.selection.is_empty() {
        return (defaults.fill.clone(), defaults.stroke.clone(), true);
    }
    match doc.selected_objects().into_iter().find_map(first_leaf) {
        Some(o) => (o.fill.clone(), o.stroke.clone(), false),
        None => (None, None, false),
    }
}

impl StyleDialog {
    pub fn stroke(doc: &DocState, defaults: &Defaults) -> StyleDialog {
        let (_, stroke, for_defaults) = current_style(doc, defaults);
        StyleDialog::Stroke(crate::strokeui::StrokeDialog::new(stroke, for_defaults))
    }

    /// Janela de preenchimento já em cor sólida (ferramenta Preenchimento
    /// uniforme): um gradiente começa pela cor da primeira parada.
    pub fn force_solid(&mut self) {
        if let StyleDialog::Fill(d) = self
            && d.mode != FillMode::Solid
        {
            if d.mode == FillMode::Gradient {
                d.color = d.spec.stops[0].color.clone();
            }
            d.mode = FillMode::Solid;
        }
    }

    /// Janela de preenchimento já no gradiente (F11).
    pub fn force_gradient(&mut self) {
        if let StyleDialog::Fill(d) = self
            && d.mode != FillMode::Gradient
            && !d.is_for_defaults()
        {
            if d.mode == FillMode::Solid {
                d.spec.stops[0].color = d.color.clone();
            }
            d.mode = FillMode::Gradient;
            d.changed = true;
        }
    }

    /// Paletas do seletor de cor (uma vez, ao abrir).
    pub fn set_palettes(&mut self, make: impl FnOnce() -> crate::colorpick::PaletteList) {
        match self {
            StyleDialog::Fill(d) if d.palettes.is_empty() => d.palettes = make(),
            StyleDialog::Stroke(d) if d.palettes.is_empty() => d.palettes = make(),
            _ => {}
        }
    }

    pub fn fill(doc: &DocState, defaults: &Defaults) -> StyleDialog {
        let (fill, _, for_defaults) = current_style(doc, defaults);
        // Caixa local do objeto: lê de volta centro e tamanho do gradiente.
        let hint = doc
            .selected_objects()
            .into_iter()
            .find_map(|o| first_leaf(o).and_then(graphdraw_core::edit::query::local_outline))
            .map(|p| kurbo::Shape::bounding_box(&p));
        StyleDialog::Fill(FillDialog::new(fill, for_defaults, hint))
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

// ---- contorno ------------------------------------------------------------------

/// Padrões de traço em múltiplos da espessura (mínimo 0,25 mm, para que
/// fio de cabelo também tenha tracejado visível).
pub(crate) const DASHES: &[(&str, &str, &[f64])] = &[
    ("Sólido", "Solid", &[]),
    ("Tracejado", "Dashed", &[4.0, 2.0]),
    ("Traço longo", "Long dash", &[8.0, 3.0]),
    ("Pontilhado", "Dotted", &[1.0, 2.0]),
    ("Traço e ponto", "Dash-dot", &[5.0, 2.0, 1.0, 2.0]),
];

fn dash_unit(width: f64) -> f64 {
    width.max(250.0)
}

pub(crate) fn dash_for(preset: usize, width: f64) -> Option<Vec<f64>> {
    let p = DASHES.get(preset)?.2;
    (!p.is_empty()).then(|| p.iter().map(|k| k * dash_unit(width)).collect())
}

pub(crate) fn detect_dash(s: &Stroke) -> usize {
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
            .unwrap_or(DASHES.len()),
    }
}

/// Copia para `s` só os campos em que `new` difere de `old`.
pub(crate) fn merge_changes(s: &mut Stroke, old: &Stroke, new: &Stroke) {
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
        nib_angle,
        overprint,
        dash_corners,
        widths
    );
}

/// Nome do perfil de largura variável na interface (GD-VAR-01).
pub(crate) fn width_label(name: &str) -> &'static str {
    match name {
        "taper-end" => tr("Afinar no fim", "Taper end"),
        "taper-start" => tr("Afinar no início", "Taper start"),
        "taper-both" => tr("Afinar nas pontas", "Taper both"),
        "bulge" => tr("Bojo", "Bulge"),
        _ => tr("Uniforme", "Uniform"),
    }
}

/// Nome da forma de ponta na interface.
pub(crate) fn arrow_label(shape: &str) -> &'static str {
    match shape {
        "open" => tr("Aberta", "Open"),
        "barbed" => tr("Farpada", "Barbed"),
        "circle" => tr("Círculo", "Circle"),
        "square" => tr("Quadrado", "Square"),
        "bar" => tr("Barra", "Bar"),
        _ => tr("Triângulo", "Triangle"),
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
/// Miniatura da ponta de seta `shape` (nenhuma = só a linha), no início ou
/// no fim de uma linha horizontal, pelo renderizador da exportação.
pub(crate) fn arrow_thumb(shape: Option<&str>, start: bool) -> egui::ColorImage {
    let (w, h) = (16.0, 5.0);
    let mut page = Page::new(w * MM, h * MM);
    let mut line = Object::new(ObjectKind::Path(poly(&[(2.5, 2.5), (13.5, 2.5)], false)));
    let mut st = Stroke::new(0.5 * MM, Color::black());
    let a = shape.map(graphdraw_core::model::Arrow::new);
    if start {
        st.start_arrow = a;
    } else {
        st.end_arrow = a;
    }
    line.stroke = Some(st);
    page.layers[0].objects = vec![line];
    let mut doc = Document::new();
    doc.data.pages.push(page);
    let scene = build_scene(
        &doc,
        &doc.data.pages[0],
        SceneOptions { print: true },
        &ImageCache::default(),
    );
    let px = 4.0;
    let s = px / MM;
    let (pw, ph) = ((w * px) as u32, (h * px) as u32);
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

pub(crate) fn stroke_preview(stroke: Option<&Stroke>) -> egui::ColorImage {
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

use crate::fillui::FillMode;
pub use crate::fillui::{FillDialog, set_tile_source};

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
        Some(s) => crate::widgets::fmt_len(s.width),
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
    fn dash_presets_round_trip() {
        let mut s = Stroke::new(500.0, Color::black());
        for p in 0..DASHES.len() {
            s.dash = dash_for(p, s.width);
            assert_eq!(detect_dash(&s), p);
        }
        s.dash = Some(vec![3000.0, 1000.0, 500.0]);
        assert_eq!(detect_dash(&s), DASHES.len());
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

    #[test]
    fn preview_renders_stroke_pixels() {
        let img = stroke_preview(Some(&Stroke::new(1000.0, Color::rgb(1.0, 0.0, 0.0))));
        assert!(img.pixels.iter().any(|p| p.r() > 200 && p.g() < 60));
        let none = stroke_preview(None);
        assert!(!none.pixels.iter().any(|p| p.r() > 200 && p.g() < 60));
    }
}
