//! Janela principal do visualizador (marco M0).
//!
//! - Abrir por diálogo, linha de comando ou arrastar-soltar (GD-CDR-01).
//! - Zoom na roda do mouse centrado no cursor, pan arrastando.
//! - Painel "Relatório de importação" (GD-UI-02, GD-CDR-03).
//! - Exportação SVG/PDF/PNG (doc 15).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, channel};

use eframe::egui::{
    self, Color32, Key, KeyboardShortcut, Modifiers, Pos2, Rect, Sense, Stroke, Vec2,
};
use graphdraw_core::export::{self, ExportFormat, ExportOptions};
use graphdraw_core::render::raster::{self, RasterOptions};
use graphdraw_core::render::{ImageCache, Scene, SceneOptions, build_scene};
use graphdraw_core::report::Severity;
use graphdraw_core::{Document, ImportReport};

use crate::i18n::t;
use crate::view::{ACTUAL_SIZE, View};

type OpenResult = Result<(Document, ImportReport), String>;

struct Loaded {
    path: PathBuf,
    doc: Document,
    report: ImportReport,
    scenes: Vec<Option<Arc<Scene>>>,
    images: ImageCache,
    object_counts: Vec<usize>,
    page: usize,
}

impl Loaded {
    fn new(path: PathBuf, doc: Document, report: ImportReport) -> Loaded {
        let object_counts = doc
            .data
            .pages
            .iter()
            .map(|p| {
                let mut n = 0;
                for l in &p.layers {
                    for o in &l.objects {
                        o.walk(&mut |_| n += 1);
                    }
                }
                n
            })
            .collect();
        let scenes = vec![None; doc.data.pages.len()];
        Loaded {
            path,
            doc,
            report,
            scenes,
            images: ImageCache::default(),
            object_counts,
            page: 0,
        }
    }

    /// Cena da página atual, construída sob demanda e mantida em cache.
    /// A tela mostra também o que não imprime (GD-CAM-01).
    fn scene(&mut self) -> Option<Arc<Scene>> {
        let i = self.page;
        if self.scenes.get(i)?.is_none() {
            let page = self.doc.data.pages.get(i)?;
            self.scenes[i] = Some(Arc::new(build_scene(
                &self.doc,
                page,
                SceneOptions { print: false },
                &self.images,
            )));
        }
        self.scenes[i].clone()
    }

    fn page_size(&self) -> (f64, f64) {
        self.doc
            .data
            .pages
            .get(self.page)
            .map(|p| (p.width, p.height))
            .unwrap_or((1.0, 1.0))
    }
}

#[derive(Clone, Copy, PartialEq)]
struct RenderKey {
    page: usize,
    view: View,
    size: [usize; 2],
    ppp: f32,
}

pub struct ViewerApp {
    loaded: Option<Loaded>,
    loading: Option<(PathBuf, Receiver<OpenResult>)>,
    view: View,
    fit_pending: bool,
    texture: Option<egui::TextureHandle>,
    rendered: Option<RenderKey>,
    show_report: bool,
    show_about: bool,
    message: Option<(String, String)>,
    status: String,
    cursor_page: Option<(f64, f64)>,
}

impl ViewerApp {
    pub fn new(cc: &eframe::CreationContext<'_>, initial: Option<PathBuf>) -> ViewerApp {
        let mut app = ViewerApp {
            loaded: None,
            loading: None,
            view: View::default(),
            fit_pending: false,
            texture: None,
            rendered: None,
            show_report: false,
            show_about: false,
            message: None,
            status: String::new(),
            cursor_page: None,
        };
        if let Some(p) = initial {
            app.open(p, &cc.egui_ctx);
        }
        app
    }

    fn open(&mut self, path: PathBuf, ctx: &egui::Context) {
        let (tx, rx) = channel();
        let ctx = ctx.clone();
        let p = path.clone();
        std::thread::spawn(move || {
            let result = graphdraw_core::open_path(&p).map_err(|e| e.to_string());
            let _ = tx.send(result);
            ctx.request_repaint();
        });
        self.loading = Some((path, rx));
        self.status = t().opening.into();
    }

    fn poll_loading(&mut self, ctx: &egui::Context) {
        let Some((path, rx)) = &self.loading else {
            return;
        };
        let Ok(result) = rx.try_recv() else { return };
        let path = path.clone();
        self.loading = None;
        self.status.clear();
        match result {
            Ok((doc, report)) => {
                let warnings = report.items.iter().any(|i| i.severity != Severity::Info);
                self.loaded = Some(Loaded::new(path.clone(), doc, report));
                self.fit_pending = true;
                self.rendered = None;
                self.show_report = warnings;
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                ctx.send_viewport_cmd(egui::ViewportCommand::Title(format!("{name} — GraphDraw")));
            }
            Err(e) => {
                self.message = Some((
                    t().error_title.into(),
                    format!("{}\n\n{e}\n\n{}", path.display(), t().open_failed_hint),
                ));
            }
        }
    }

    fn pick_and_open(&mut self, ctx: &egui::Context) {
        if let Some(p) = rfd::FileDialog::new()
            .add_filter(t().supported_files, &["gdr", "cdr", "cmx"])
            .pick_file()
        {
            self.open(p, ctx);
        }
    }

    fn export(&mut self, format: ExportFormat) {
        let Some(l) = &self.loaded else { return };
        let stem = l
            .path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "desenho".into());
        let ext = format.extension();
        let suggested = if format.single_page() && l.doc.data.pages.len() > 1 {
            format!("{stem}-{}.{ext}", l.page + 1)
        } else {
            format!("{stem}.{ext}")
        };
        let mut dialog = rfd::FileDialog::new()
            .set_file_name(&suggested)
            .add_filter(ext.to_uppercase(), &[ext]);
        if let Some(dir) = l.path.parent() {
            dialog = dialog.set_directory(dir);
        }
        let Some(out) = dialog.save_file() else {
            return;
        };
        let opts = ExportOptions {
            pages: if format.single_page() {
                vec![l.page]
            } else {
                Vec::new()
            },
            dpi: l.doc.data.settings.resolution,
            ..Default::default()
        };
        match export::export(&l.doc, &out, format, &opts) {
            Ok(files) => self.status = format!("{} {}", t().exported_to, display_list(&files)),
            Err(e) => {
                self.message = Some((t().error_title.into(), format!("{}\n\n{e}", out.display())))
            }
        }
    }

    fn set_page(&mut self, page: usize) {
        if let Some(l) = &mut self.loaded
            && page < l.doc.data.pages.len()
            && page != l.page
        {
            l.page = page;
            self.fit_pending = true;
        }
    }

    fn shortcuts(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let open = KeyboardShortcut::new(Modifiers::COMMAND, Key::O);
        if ui.input_mut(|i| i.consume_shortcut(&open)) {
            self.pick_and_open(&ctx);
        }
        let (plus, minus, zero, one, next, prev) = ui.input(|i| {
            (
                i.key_pressed(Key::Plus) || i.key_pressed(Key::Equals),
                i.key_pressed(Key::Minus),
                i.key_pressed(Key::Num0),
                i.key_pressed(Key::Num1),
                i.key_pressed(Key::PageDown),
                i.key_pressed(Key::PageUp),
            )
        });
        if ui.ctx().egui_wants_keyboard_input() {
            return;
        }
        let center = ui.ctx().content_rect().size() / 2.0;
        if plus {
            self.view.zoom_at(1.25, (center.x as f64, center.y as f64));
        }
        if minus {
            self.view.zoom_at(0.8, (center.x as f64, center.y as f64));
        }
        if zero {
            self.fit_pending = true;
        }
        if one {
            self.actual_size_pending();
        }
        if let Some(page) = self.loaded.as_ref().map(|l| l.page) {
            if next {
                self.set_page(page + 1);
            }
            if prev && page > 0 {
                self.set_page(page - 1);
            }
        }
    }

    fn actual_size_pending(&mut self) {
        // Aplicado no próximo desenho da área (precisa do tamanho dela).
        self.view.zoom = -1.0;
    }

    fn menu(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let has_doc = self.loaded.is_some();
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button(t().menu_file, |ui| {
                if ui.button(t().open).clicked() {
                    self.pick_and_open(&ctx);
                }
                ui.separator();
                if ui
                    .add_enabled(has_doc, egui::Button::new(t().export_svg))
                    .clicked()
                {
                    self.export(ExportFormat::Svg);
                }
                if ui
                    .add_enabled(has_doc, egui::Button::new(t().export_pdf))
                    .clicked()
                {
                    self.export(ExportFormat::Pdf);
                }
                if ui
                    .add_enabled(has_doc, egui::Button::new(t().export_png))
                    .clicked()
                {
                    self.export(ExportFormat::Png);
                }
                ui.separator();
                if ui.button(t().quit).clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
            ui.menu_button(t().menu_view, |ui| {
                if ui
                    .add_enabled(has_doc, egui::Button::new(format!("{}  (0)", t().fit_page)))
                    .clicked()
                {
                    self.fit_pending = true;
                }
                if ui
                    .add_enabled(
                        has_doc,
                        egui::Button::new(format!("{}  (1)", t().actual_size)),
                    )
                    .clicked()
                {
                    self.actual_size_pending();
                }
                if ui
                    .add_enabled(has_doc, egui::Button::new(format!("{}  (+)", t().zoom_in)))
                    .clicked()
                {
                    let c = ctx.content_rect().center();
                    self.view.zoom_at(1.25, (c.x as f64, c.y as f64));
                }
                if ui
                    .add_enabled(has_doc, egui::Button::new(format!("{}  (−)", t().zoom_out)))
                    .clicked()
                {
                    let c = ctx.content_rect().center();
                    self.view.zoom_at(0.8, (c.x as f64, c.y as f64));
                }
                ui.separator();
                ui.checkbox(&mut self.show_report, t().import_report);
            });
            ui.menu_button(t().menu_help, |ui| {
                if ui.button(t().about).clicked() {
                    self.show_about = true;
                }
            });
        });
    }

    fn status_bar(&self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if self.loading.is_some() {
                ui.spinner();
            }
            if let Some(l) = &self.loaded {
                let (w, h) = l.page_size();
                ui.label(format!(
                    "{} {}/{} · {:.1} × {:.1} mm · {} {}",
                    t().page,
                    l.page + 1,
                    l.doc.data.pages.len(),
                    w / 1000.0,
                    h / 1000.0,
                    l.object_counts.get(l.page).copied().unwrap_or(0),
                    t().objects
                ));
                ui.separator();
                ui.label(format!(
                    "{} {:.0}%",
                    t().zoom,
                    self.view.zoom / ACTUAL_SIZE * 100.0
                ));
                if let Some((x, y)) = self.cursor_page {
                    ui.separator();
                    ui.label(format!(
                        "{} x {:.2} mm  y {:.2} mm",
                        t().cursor,
                        x / 1000.0,
                        y / 1000.0
                    ));
                }
            }
            if !self.status.is_empty() {
                ui.separator();
                ui.label(&self.status);
            }
        });
    }

    fn pages_panel(&mut self, ui: &mut egui::Ui) {
        let Some(l) = &self.loaded else { return };
        let mut select = None;
        ui.heading(t().pages);
        egui::ScrollArea::vertical().show(ui, |ui| {
            for (i, p) in l.doc.data.pages.iter().enumerate() {
                let label = match &p.name {
                    Some(n) => format!(
                        "{}. {n}\n{:.0} × {:.0} mm",
                        i + 1,
                        p.width / 1000.0,
                        p.height / 1000.0
                    ),
                    None => format!(
                        "{} {}\n{:.0} × {:.0} mm",
                        t().page,
                        i + 1,
                        p.width / 1000.0,
                        p.height / 1000.0
                    ),
                };
                if ui.selectable_label(i == l.page, label).clicked() {
                    select = Some(i);
                }
            }
        });
        if let Some(i) = select {
            self.set_page(i);
        }
    }

    fn report_panel(&mut self, ui: &mut egui::Ui) {
        let Some(l) = &self.loaded else { return };
        ui.heading(t().import_report);
        ui.label(format!(
            "{}: {}",
            l.report.source_format.to_uppercase(),
            l.path.display()
        ));
        if let Some(v) = &l.report.detected_version {
            ui.label(format!("{}: {v}", t().detected_version));
        }
        ui.separator();
        if l.report.items.is_empty() {
            ui.label(t().report_empty);
        }
        let mut goto = None;
        egui::ScrollArea::vertical().show(ui, |ui| {
            for item in &l.report.items {
                let (tag, color) = match item.severity {
                    Severity::Info => (t().info, ui.visuals().weak_text_color()),
                    Severity::Warning => (t().warning, Color32::from_rgb(0xd0, 0x8a, 0x00)),
                    Severity::Error => (t().error, Color32::from_rgb(0xd0, 0x30, 0x30)),
                };
                ui.horizontal_wrapped(|ui| {
                    ui.colored_label(color, format!("[{tag}]"));
                    if let Some(p) = item.page
                        && ui.small_button(format!("{} {p}", t().page)).clicked()
                    {
                        goto = Some(p - 1);
                    }
                    let count = if item.count > 1 {
                        format!(" (×{})", item.count)
                    } else {
                        String::new()
                    };
                    ui.label(format!("{}{count}", item.message));
                });
            }
        });
        if let Some(p) = goto {
            self.set_page(p);
        }
    }

    fn canvas(&mut self, ui: &mut egui::Ui) {
        let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        let painter = ui.painter_at(rect);
        let desk = if ui.visuals().dark_mode {
            Color32::from_gray(48)
        } else {
            Color32::from_gray(205)
        };
        painter.rect_filled(rect, 0.0, desk);

        let Some(l) = &mut self.loaded else {
            let msg = if self.loading.is_some() {
                t().opening
            } else {
                t().drop_hint
            };
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                msg,
                egui::FontId::proportional(16.0),
                ui.visuals().weak_text_color(),
            );
            painter.text(
                rect.center() - Vec2::new(0.0, 32.0),
                egui::Align2::CENTER_CENTER,
                t().no_document,
                egui::FontId::proportional(22.0),
                ui.visuals().text_color(),
            );
            return;
        };
        let (pw, ph) = l.page_size();
        let size = rect.size();
        if self.fit_pending {
            self.view = View::fit(pw, ph, size.x as f64, size.y as f64);
            self.fit_pending = false;
        } else if self.view.zoom < 0.0 {
            self.view = View::actual_size(pw, ph, size.x as f64, size.y as f64);
        }

        // Interação: arrastar = pan; roda/pinça = zoom no cursor.
        if response.dragged() {
            let d = response.drag_delta();
            self.view.pan_by(d.x as f64, d.y as f64);
        }
        if let Some(pos) = response.hover_pos() {
            let local = pos - rect.min;
            let (scroll, pinch) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
            let factor = pinch as f64 * (scroll as f64 * 0.0025).exp();
            if (factor - 1.0).abs() > 1e-4 {
                self.view.zoom_at(factor, (local.x as f64, local.y as f64));
            }
            self.cursor_page = Some(self.view.screen_to_page(ph, local.x as f64, local.y as f64));
        } else {
            self.cursor_page = None;
        }
        if response.double_clicked() {
            self.fit_pending = true;
        }

        // Papel com sombra.
        let (x0, y0) = self.view.page_to_screen(ph, 0.0, ph);
        let (x1, y1) = self.view.page_to_screen(ph, pw, 0.0);
        let page_rect = Rect::from_min_max(
            rect.min + Vec2::new(x0 as f32, y0 as f32),
            rect.min + Vec2::new(x1 as f32, y1 as f32),
        );
        painter.rect_filled(
            page_rect.translate(Vec2::new(3.0, 3.0)),
            0.0,
            Color32::from_black_alpha(60),
        );
        painter.rect_filled(page_rect, 0.0, Color32::WHITE);
        painter.rect_stroke(
            page_rect,
            0.0,
            Stroke::new(1.0, Color32::from_gray(150)),
            egui::StrokeKind::Outside,
        );

        // Conteúdo: rasteriza só a área visível; durante o arrasto reaproveita a textura deslocada.
        let ppp = ui.ctx().pixels_per_point();
        let px = [
            (size.x * ppp).round().max(1.0) as usize,
            (size.y * ppp).round().max(1.0) as usize,
        ];
        let key = RenderKey {
            page: l.page,
            view: self.view,
            size: px,
            ppp,
        };
        let dragging = response.dragged()
            && self
                .rendered
                .is_some_and(|r| r.view.zoom == self.view.zoom && r.size == px);
        if self.rendered != Some(key)
            && !dragging
            && let Some(scene) = l.scene()
        {
            let affine = self.view.raster_affine(ph, ppp as f64);
            if let Some(pm) = raster::render(
                &scene,
                affine,
                px[0] as u32,
                px[1] as u32,
                &RasterOptions::default(),
            ) {
                let image = egui::ColorImage::from_rgba_premultiplied(px, pm.data());
                match &mut self.texture {
                    Some(tex) => tex.set(image, egui::TextureOptions::LINEAR),
                    None => {
                        self.texture = Some(ui.ctx().load_texture(
                            "pagina",
                            image,
                            egui::TextureOptions::LINEAR,
                        ))
                    }
                }
                self.rendered = Some(key);
            }
        }
        if let (Some(tex), Some(r)) = (&self.texture, self.rendered)
            && r.page == l.page
        {
            let shift = Vec2::new(
                (self.view.pan.0 - r.view.pan.0) as f32,
                (self.view.pan.1 - r.view.pan.1) as f32,
            );
            let img_rect = Rect::from_min_size(
                rect.min + shift,
                Vec2::new(r.size[0] as f32 / ppp, r.size[1] as f32 / ppp),
            );
            painter.image(
                tex.id(),
                img_rect,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
        }
    }
}

fn display_list(files: &[PathBuf]) -> String {
    match files {
        [one] => one.display().to_string(),
        many => format!(
            "{} … ({})",
            many.first()
                .map(|p| p.display().to_string())
                .unwrap_or_default(),
            many.len()
        ),
    }
}

fn dropped_path(ctx: &egui::Context) -> Option<PathBuf> {
    ctx.input(|i| {
        i.raw.dropped_files.iter().find_map(|f| {
            let p = f.path();
            (!p.as_os_str().is_empty()).then(|| p.to_path_buf())
        })
    })
}

fn is_supported(p: &Path) -> bool {
    // A detecção real é pelo conteúdo; aqui só evitamos pastas.
    p.is_file()
}

impl eframe::App for ViewerApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.poll_loading(&ctx);
        if let Some(p) = dropped_path(&ctx).filter(|p| is_supported(p)) {
            self.open(p, &ctx);
        }
        self.shortcuts(ui);

        egui::Panel::top("menu").show(ui, |ui| self.menu(ui));
        egui::Panel::bottom("status").show(ui, |ui| self.status_bar(ui));
        if self
            .loaded
            .as_ref()
            .is_some_and(|l| l.doc.data.pages.len() > 1)
        {
            egui::Panel::left("paginas")
                .default_size(170.0)
                .show(ui, |ui| self.pages_panel(ui));
        }
        if self.show_report && self.loaded.is_some() {
            egui::Panel::right("relatorio")
                .default_size(320.0)
                .show(ui, |ui| self.report_panel(ui));
        }
        egui::CentralPanel::no_frame().show(ui, |ui| self.canvas(ui));

        if let Some((title, body)) = self.message.clone() {
            egui::Window::new(title)
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .show(&ctx, |ui| {
                    ui.label(body);
                    if ui.button(t().ok).clicked() {
                        self.message = None;
                    }
                });
        }
        if self.show_about {
            egui::Window::new(t().about)
                .collapsible(false)
                .resizable(false)
                .open(&mut self.show_about)
                .show(&ctx, |ui| {
                    ui.label(t().about_text);
                    ui.label(format!("v{}", env!("CARGO_PKG_VERSION")));
                });
        }
    }
}
