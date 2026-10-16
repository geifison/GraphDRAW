//! Ícones próprios do GraphDraw (assets/icons, gerados por make-icons.mjs).
//! Todos monocromáticos; a cor vem do tema via `tint`.

use eframe::egui::{self, ImageSource, Vec2};

macro_rules! icon {
    ($name:literal) => {
        egui::include_image!(concat!("../assets/icons/", $name, ".svg"))
    };
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
        _ => icon!("history"),
    }
}

/// Botão de ícone pequeno (linhas de lista, como no painel de camadas).
pub fn small_button(ui: &mut egui::Ui, name: &str, tip: &str, enabled: bool) -> egui::Response {
    let color = ui.visuals().text_color();
    let img = egui::Image::new(get(name))
        .fit_to_exact_size(Vec2::splat(16.0))
        .tint(color);
    ui.add_enabled(enabled, egui::Button::image(img).frame(false))
        .on_hover_text(tip)
}

/// Botão só com ícone, dica ao passar o mouse e estado "selecionado".
pub fn button(
    ui: &mut egui::Ui,
    name: &str,
    tip: &str,
    selected: bool,
    enabled: bool,
) -> egui::Response {
    let color = ui.visuals().text_color();
    let img = egui::Image::new(get(name))
        .fit_to_exact_size(Vec2::splat(20.0))
        .tint(color);
    ui.add_enabled(
        enabled,
        egui::Button::image(img)
            .selected(selected)
            .min_size(Vec2::splat(30.0)),
    )
    .on_hover_text(tip)
}
