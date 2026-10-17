//! Ícones próprios do GraphDraw (assets/icons, gerados por make-icons.mjs).
//! Todos monocromáticos; a cor vem do tema via `tint`. Grade de 20 px na
//! interface, 16 px nos microcontroles e 24 px nos cursores
//! (DESIGN-SYSTEM 3.1).

use eframe::egui::{self, Color32, ImageSource, Rect, Sense, Vec2};

use crate::theme;

macro_rules! icon {
    ($name:literal) => {
        egui::include_image!(concat!("../assets/icons/", $name, ".svg"))
    };
}

macro_rules! micro {
    ($name:literal) => {
        egui::include_image!(concat!("../assets/icons/16/", $name, ".svg"))
    };
}

/// Versão de 16 px (microcontroles), quando existe; senão a de 20 px.
pub fn get_micro(name: &str) -> ImageSource<'static> {
    match name {
        "eye-on" => micro!("eye-on"),
        "eye-off" => micro!("eye-off"),
        "print-on" => micro!("print-on"),
        "print-off" => micro!("print-off"),
        "lock-on" => micro!("lock-on"),
        "lock-off" => micro!("lock-off"),
        "doc" => micro!("doc"),
        "bleed" => micro!("bleed"),
        other => get(other),
    }
}

pub fn get(name: &str) -> ImageSource<'static> {
    match name {
        "tool-select" => icon!("tool-select"),
        "tool-shape" => icon!("tool-shape"),
        "tool-zoom" => icon!("tool-zoom"),
        "tool-pan" => icon!("tool-pan"),
        "tool-bezier" => icon!("tool-bezier"),
        "tool-rect" => icon!("tool-rect"),
        "tool-ellipse" => icon!("tool-ellipse"),
        "tool-artboard" => icon!("tool-artboard"),
        "tool-text" => icon!("tool-text"),
        "tool-gradient" => icon!("tool-gradient"),
        "tool-fill-solid" => icon!("tool-fill-solid"),
        "tool-eyedropper" => icon!("tool-eyedropper"),
        "tool-knife" => icon!("tool-knife"),
        "tool-connector" => icon!("tool-connector"),
        "tool-smart-fill" => icon!("tool-smart-fill"),
        "shape-basic" => icon!("shape-basic"),
        "tool-dimension" => icon!("tool-dimension"),
        "tool-free-transform" => icon!("tool-free-transform"),
        "tool-freehand" => icon!("tool-freehand"),
        "tool-polyline" => icon!("tool-polyline"),
        "tool-line2" => icon!("tool-line2"),
        "tool-curve3" => icon!("tool-curve3"),
        "tool-crop" => icon!("tool-crop"),
        "tool-eraser" => icon!("tool-eraser"),
        "tool-virtual-delete" => icon!("tool-virtual-delete"),
        "tool-spiral" => icon!("tool-spiral"),
        "tool-graph" => icon!("tool-graph"),
        "tool-polygon" => icon!("tool-polygon"),
        "poly-star" => icon!("poly-star"),
        "poly-complex" => icon!("poly-complex"),
        "text-bold" => icon!("text-bold"),
        "text-italic" => icon!("text-italic"),
        "text-left" => icon!("text-left"),
        "text-center" => icon!("text-center"),
        "text-right" => icon!("text-right"),
        "text-justify" => icon!("text-justify"),
        "file-new" => icon!("file-new"),
        "file-open" => icon!("file-open"),
        "file-save" => icon!("file-save"),
        "file-export" => icon!("file-export"),
        "undo" => icon!("undo"),
        "redo" => icon!("redo"),
        "delete" => icon!("delete"),
        "duplicate" => icon!("duplicate"),
        "to-curves" => icon!("to-curves"),
        "mirror-h" => icon!("mirror-h"),
        "mirror-v" => icon!("mirror-v"),
        "rotate-ccw" => icon!("rotate-ccw"),
        "rotate-cw" => icon!("rotate-cw"),
        "order-front" => icon!("order-front"),
        "order-back" => icon!("order-back"),
        "order-forward" => icon!("order-forward"),
        "order-backward" => icon!("order-backward"),
        "align-left" => icon!("align-left"),
        "align-center-h" => icon!("align-center-h"),
        "align-right" => icon!("align-right"),
        "align-top" => icon!("align-top"),
        "align-center-v" => icon!("align-center-v"),
        "align-bottom" => icon!("align-bottom"),
        "snap" => icon!("snap"),
        "grid" => icon!("grid"),
        "node-add" => icon!("node-add"),
        "node-delete" => icon!("node-delete"),
        "node-join" => icon!("node-join"),
        "node-connect" => icon!("node-connect"),
        "node-break" => icon!("node-break"),
        "path-close" => icon!("path-close"),
        "path-open" => icon!("path-open"),
        "path-reverse" => icon!("path-reverse"),
        "seg-line" => icon!("seg-line"),
        "seg-curve" => icon!("seg-curve"),
        "node-corner" => icon!("node-corner"),
        "node-smooth" => icon!("node-smooth"),
        "node-symmetric" => icon!("node-symmetric"),
        "fill-none" => icon!("fill-none"),
        "stroke-none" => icon!("stroke-none"),
        "fit-page" => icon!("fit-page"),
        "fit-selection" => icon!("fit-selection"),
        "report" => icon!("report"),
        "group" => icon!("group"),
        "ungroup" => icon!("ungroup"),
        "layers" => icon!("layers"),
        "layer-add" => icon!("layer-add"),
        "layer-move-to" => icon!("layer-move-to"),
        "eye-on" => icon!("eye-on"),
        "eye-off" => icon!("eye-off"),
        "print-on" => icon!("print-on"),
        "print-off" => icon!("print-off"),
        "lock-on" => icon!("lock-on"),
        "lock-off" => icon!("lock-off"),
        "wireframe" => icon!("wireframe"),
        "join-miter" => icon!("join-miter"),
        "join-round" => icon!("join-round"),
        "join-bevel" => icon!("join-bevel"),
        "cap-butt" => icon!("cap-butt"),
        "cap-round" => icon!("cap-round"),
        "cap-square" => icon!("cap-square"),
        "pos-center" => icon!("pos-center"),
        "pos-inside" => icon!("pos-inside"),
        "pos-outside" => icon!("pos-outside"),
        "behind-fill" => icon!("behind-fill"),
        "scale-stroke" => icon!("scale-stroke"),
        "page-grid" => icon!("page-grid"),
        "page-list" => icon!("page-list"),
        "page-prev" => icon!("page-prev"),
        "page-next" => icon!("page-next"),
        "page-add" => icon!("page-add"),
        "cur-shape" => icon!("cur-shape"),
        "cur-move" => icon!("cur-move"),
        "cur-grab" => icon!("cur-grab"),
        "cur-rotate" => icon!("cur-rotate"),
        "cur-select" => icon!("cur-select"),
        "cur-zoom" => icon!("cur-zoom"),
        "cur-pan" => icon!("cur-pan"),
        "page-portrait" => icon!("page-portrait"),
        "page-landscape" => icon!("page-landscape"),
        "fit-art" => icon!("fit-art"),
        "bleed" => icon!("bleed"),
        "doc" => icon!("doc"),
        "panel-collapse" => icon!("panel-collapse"),
        "fill-linear" => icon!("fill-linear"),
        "palette" => icon!("palette"),
        "fill-radial" => icon!("fill-radial"),
        "fill-pattern" => icon!("fill-pattern"),
        "fill-elliptic" => icon!("fill-elliptic"),
        "fill-conic" => icon!("fill-conic"),
        "fill-rectangular" => icon!("fill-rectangular"),
        "dist-left" => icon!("dist-left"),
        "dist-bottom" => icon!("dist-bottom"),
        "dist-center-h" => icon!("dist-center-h"),
        "dist-center-v" => icon!("dist-center-v"),
        "dist-right" => icon!("dist-right"),
        "dist-top" => icon!("dist-top"),
        "dist-space-h" => icon!("dist-space-h"),
        "dist-space-v" => icon!("dist-space-v"),
        "node-reduce" => icon!("node-reduce"),
        "align-baseline" => icon!("align-baseline"),
        "node-align-h" => icon!("node-align-h"),
        "node-align-v" => icon!("node-align-v"),
        "node-extract" => icon!("node-extract"),
        "clear-transform" => icon!("clear-transform"),
        "bool-weld" => icon!("bool-weld"),
        "bool-trim" => icon!("bool-trim"),
        "bool-intersect" => icon!("bool-intersect"),
        "bool-simplify" => icon!("bool-simplify"),
        "bool-front-minus" => icon!("bool-front-minus"),
        "bool-back-minus" => icon!("bool-back-minus"),
        "bool-exclude" => icon!("bool-exclude"),
        "bool-divide" => icon!("bool-divide"),
        "bool-boundary" => icon!("bool-boundary"),
        "combine" => icon!("combine"),
        "break-apart" => icon!("break-apart"),
        "offset" => icon!("offset"),
        "panel-expand" => icon!("panel-expand"),
        "panel-undock" => icon!("panel-undock"),
        "outline-to-object" => icon!("outline-to-object"),
        "order-in-front-of" => icon!("order-in-front-of"),
        "order-behind-of" => icon!("order-behind-of"),
        "ungroup-all" => icon!("ungroup-all"),
        "show-all" => icon!("show-all"),
        "unlock-all" => icon!("unlock-all"),
        "text-sample" => icon!("text-sample"),
        "text-convert" => icon!("text-convert"),
        "text-frame" => icon!("text-frame"),
        "image-frame" => icon!("image-frame"),
        "text-path" => icon!("text-path"),
        "cut-line" => icon!("cut-line"),
        "export-selection" => icon!("export-selection"),
        "focus" => icon!("focus"),
        "paste" => icon!("paste"),
        "select-all" => icon!("select-all"),
        "clip-inside" => icon!("clip-inside"),
        "copy-props" => icon!("copy-props"),
        "copy-stroke" => icon!("copy-stroke"),
        "copy-fill" => icon!("copy-fill"),
        "center-on" => icon!("center-on"),
        "move-here" => icon!("move-here"),
        "instance" => icon!("instance"),
        "cancel" => icon!("cancel"),
        "preflight" => icon!("preflight"),
        "guide-object" => icon!("guide-object"),
        _ => icon!("history"),
    }
}

/// Botão de ícone pequeno (linhas de lista, como no painel de camadas):
/// ícone de 16 px num botão ghost de 20 px.
pub fn small_button(ui: &mut egui::Ui, name: &str, tip: &str, enabled: bool) -> egui::Response {
    ghost(
        ui,
        get_micro(name),
        tip,
        false,
        enabled,
        theme::ICON_MICRO + 2.0 * theme::SPACE_050,
        theme::ICON_MICRO,
    )
}

/// Botão de ferramenta (ghost, Compact 24 px) só com ícone, dica e estado
/// "ligado" (DESIGN-SYSTEM 4.1).
pub fn button(
    ui: &mut egui::Ui,
    name: &str,
    tip: &str,
    selected: bool,
    enabled: bool,
) -> egui::Response {
    ghost(
        ui,
        get(name),
        tip,
        selected,
        enabled,
        theme::BUTTON_COMPACT,
        theme::ICON_UI,
    )
}

/// Botão ghost de ícone: transparente em repouso; hover, pressionado e
/// ligado conforme a tabela de estados da seção 4.1.
fn ghost(
    ui: &mut egui::Ui,
    image: ImageSource<'static>,
    tip: &str,
    selected: bool,
    enabled: bool,
    size: f32,
    icon: f32,
) -> egui::Response {
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), sense);
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, enabled, selected, tip));
    if ui.is_rect_visible(rect) {
        let pal = theme::palette(ui);
        let pressed = enabled && resp.is_pointer_button_down_on();
        let (fill, tint, border) = if !enabled {
            (Color32::TRANSPARENT, pal.fg_muted.gamma_multiply(0.4), None)
        } else if pressed {
            (pal.bg_surface_active, pal.fg_pressed, None)
        } else if selected {
            (
                pal.bg_surface_toggled,
                theme::ACCENT_PRIMARY,
                Some(theme::border_focus()),
            )
        } else if resp.hovered() {
            (pal.bg_surface_hover, pal.fg_strong, None)
        } else {
            (Color32::TRANSPARENT, pal.fg_muted, None)
        };
        let p = ui.painter();
        p.rect_filled(rect, theme::RADIUS_SM, fill);
        if let Some(b) = border {
            p.rect_stroke(rect, theme::RADIUS_SM, b, egui::StrokeKind::Inside);
        }
        let shift = if pressed { theme::PRESS_OFFSET } else { 0.0 };
        let r = Rect::from_center_size(rect.center() + Vec2::new(0.0, shift), Vec2::splat(icon));
        egui::Image::new(image).tint(tint).paint_at(ui, r);
    }
    resp.on_hover_text(tip)
}
