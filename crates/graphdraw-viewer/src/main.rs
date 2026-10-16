//! `graphdraw-viewer [arquivo]` — visualizador do marco M0.
//! Abre `.gdr`, `.cdr` e `.cmx`; zoom/pan; exporta SVG/PDF/PNG.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod i18n;
mod view;

fn main() -> eframe::Result {
    let initial = std::env::args_os().nth(1).map(std::path::PathBuf::from);
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title(i18n::t().app_title)
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([480.0, 320.0])
            .with_drag_and_drop(true),
        ..Default::default()
    };
    eframe::run_native(
        "graphdraw-viewer",
        options,
        Box::new(move |cc| Ok(Box::new(app::ViewerApp::new(cc, initial)))),
    )
}
