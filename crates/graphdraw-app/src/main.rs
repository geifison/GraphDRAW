//! `graphdraw-app [arquivo]` — aplicativo gráfico do GraphDraw.
//! Marco M1: editor básico (seleção, transformação, retângulo, elipse,
//! Bézier, edição de nós, preenchimento e contorno, desfazer, salvar `.gdr`).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod canvas;
mod colorui;
mod ctxmenu;
mod cutui;
mod dialogs;
mod doc;
mod doctabs;
mod fontpicker;
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
mod rulers;
mod textedit;
mod theme;
mod toolbox;
mod view;
mod widgets;
mod workspace;

fn main() -> eframe::Result {
    let initial = std::env::args_os().nth(1).map(std::path::PathBuf::from);
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("GraphDraw")
            .with_inner_size([1360.0, 860.0])
            .with_min_inner_size([720.0, 480.0])
            .with_drag_and_drop(true),
        ..Default::default()
    };
    eframe::run_native(
        "graphdraw",
        options,
        Box::new(move |cc| Ok(Box::new(app::App::new(cc, initial)))),
    )
}
