//! `graphdraw-app [arquivo]` — aplicativo gráfico do GraphDraw.
//! Marco M1: editor básico (seleção, transformação, retângulo, elipse,
//! Bézier, edição de nós, preenchimento e contorno, desfazer, salvar `.gdr`).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod canvas;
mod colorpick;
mod colorui;
mod ctxmenu;
mod cutui;
mod dialogs;
mod doc;
mod doctabs;
mod eyedrop;
mod fillui;
mod fontpicker;
mod fontsui;
mod guideui;
mod i18n;
mod icons;
mod import;
mod keymap;
mod layertree;
mod newdoc;
mod pagebar;
mod paletteui;
mod panels;
mod recovery;
mod rulers;
mod strokeui;
mod textedit;
mod theme;
mod toolbox;
#[cfg(test)]
mod uishot;
mod view;
mod widgets;
mod workspace;

fn main() -> eframe::Result {
    recovery::install_panic_log();
    let initial = std::env::args_os().nth(1).map(std::path::PathBuf::from);
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("GraphDraw")
            .with_inner_size([1360.0, 860.0])
            .with_min_inner_size([720.0, 480.0])
            .with_drag_and_drop(true)
            .with_icon(std::sync::Arc::new(
                eframe::icon_data::from_png_bytes(include_bytes!("../assets/brand/app-256.png"))
                    .unwrap_or_default(),
            )),
        ..Default::default()
    };
    eframe::run_native(
        "graphdraw",
        options,
        Box::new(move |cc| Ok(Box::new(app::App::new(cc, initial)))),
    )
}
