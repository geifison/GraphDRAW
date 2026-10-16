//! Janela principal do GraphDraw (marco M1: editor básico).
//!
//! Estrutura (GD-UI-01): menus, barra de ferramentas padrão, barra de
//! propriedades contextual, caixa de ferramentas vertical, área de desenho,
//! painéis à direita, barra de paleta e barra de status.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, channel};

use eframe::egui::{self, Key, KeyboardShortcut, Modifiers};
use graphdraw_core::edit::ops::{self, Anchor, Order};
use graphdraw_core::export::{self, ExportFormat, ExportOptions};
use graphdraw_core::model::ObjectKind;
use graphdraw_core::report::Severity;
use graphdraw_core::{Document, ImportReport};
use kurbo::Vec2;

use crate::canvas::{CanvasState, Defaults, Tool};
use crate::doc::{DocState, default_stroke};
use crate::i18n::tr;
use crate::icons;
use crate::panels::{AlignUi, TransformUi};
use crate::view::ACTUAL_SIZE;
use crate::widgets::fmt_mm;

type OpenResult = Result<(Document, ImportReport), String>;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RightTab {
    Transform,
    Align,
    Layers,
    History,
    Pages,
    Report,
}

/// Ação que exige confirmar alterações não salvas (GD-UND-07).
#[derive(Clone)]
enum Pending {
    New,
    Open(Option<PathBuf>),
    Quit,
}

pub struct App {
    pub(crate) doc: Option<DocState>,
    loading: Option<(PathBuf, Receiver<OpenResult>)>,
    pub(crate) tool: Tool,
    pub(crate) canvas: CanvasState,
    pub(crate) defaults: Defaults,
    pub(crate) right_tab: RightTab,
    show_right: bool,
    pub(crate) xf: TransformUi,
    pub(crate) align: AlignUi,
    pub(crate) anchor: Anchor,
    pub(crate) lock_ratio: bool,
    message: Option<(String, String)>,
    confirm: Option<Pending>,
    allow_close: bool,
    show_about: bool,
    show_keys: bool,
    status: String,
    /// Deslocamento das setas (GD-FER-01): normal e com Shift.
    nudge: (f64, f64),
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, initial: Option<PathBuf>) -> App {
        egui_extras::install_image_loaders(&cc.egui_ctx);
        let mut app = App {
            doc: Some(DocState::new_blank()),
            loading: None,
            tool: Tool::Select,
            canvas: CanvasState::default(),
            defaults: Defaults {
                fill: None,
                stroke: Some(default_stroke()),
            },
            right_tab: RightTab::Transform,
            show_right: true,
            xf: TransformUi::default(),
            align: AlignUi::default(),
            anchor: Anchor::Center,
            lock_ratio: true,
            message: None,
            confirm: None,
            allow_close: false,
            show_about: false,
            show_keys: false,
            status: String::new(),
            nudge: (1_000.0, 10_000.0),
        };
        if let Some(p) = initial {
            app.start_open(p, &cc.egui_ctx);
        }
        app
    }

    fn modified(&self) -> bool {
        self.doc.as_ref().is_some_and(|d| d.editor.is_modified())
    }

    // ---- arquivos ---------------------------------------------------------

    fn start_open(&mut self, path: PathBuf, ctx: &egui::Context) {
        let (tx, rx) = channel();
        let ctx = ctx.clone();
        let p = path.clone();
        std::thread::spawn(move || {
            let _ = tx.send(graphdraw_core::open_path(&p).map_err(|e| e.to_string()));
            ctx.request_repaint();
        });
        self.loading = Some((path, rx));
        self.status = tr("Abrindo…", "Opening…").into();
    }

    fn poll_loading(&mut self) {
        let Some((path, rx)) = &self.loading else {
            return;
        };
        let Ok(result) = rx.try_recv() else { return };
        let path = path.clone();
        self.loading = None;
        self.status.clear();
        match result {
            Ok((doc, report)) => {
                let warn = report.items.iter().any(|i| i.severity != Severity::Info);
                self.doc = Some(DocState::from_document(doc, Some(path), report));
                self.canvas.reset_for_new_document();
                if warn {
                    self.right_tab = RightTab::Report;
                    self.show_right = true;
                }
            }
            Err(e) => {
                self.message = Some((
                    tr("Não foi possível abrir", "Could not open").into(),
                    format!(
                        "{}\n\n{e}\n\n{}",
                        path.display(),
                        tr(
                            "Verifique se o arquivo não está aberto em outro programa e se é um .gdr, .cdr ou .cmx válido.",
                            "Check that the file is not open elsewhere and is a valid .gdr, .cdr or .cmx."
                        )
                    ),
                ))
            }
        }
    }

    fn request(&mut self, action: Pending, ctx: &egui::Context) {
        if self.modified() {
            self.confirm = Some(action);
        } else {
            self.run(action, ctx);
        }
    }

    fn run(&mut self, action: Pending, ctx: &egui::Context) {
        match action {
            Pending::New => {
                self.doc = Some(DocState::new_blank());
                self.canvas.reset_for_new_document();
            }
            Pending::Open(Some(p)) => self.start_open(p, ctx),
            Pending::Open(None) => {
                if let Some(p) = rfd::FileDialog::new()
                    .add_filter(
                        tr("Desenhos (.gdr, .cdr, .cmx)", "Drawings (.gdr, .cdr, .cmx)"),
                        &["gdr", "cdr", "cmx"],
                    )
                    .pick_file()
                {
                    self.start_open(p, ctx);
                }
            }
            Pending::Quit => {
                self.allow_close = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    /// Salva em `.gdr` (atômico, GD-GDR-09). Devolve `true` se salvou.
    fn save(&mut self, save_as: bool) -> bool {
        let Some(doc) = self.doc.as_mut() else {
            return false;
        };
        let target = if save_as || doc.path.is_none() || doc.needs_save_as {
            let stem = doc
                .path
                .as_ref()
                .and_then(|p| p.file_stem())
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| tr("desenho", "drawing").into());
            let mut d = rfd::FileDialog::new()
                .set_file_name(format!("{stem}.gdr"))
                .add_filter("GraphDraw (.gdr)", &["gdr"]);
            if let Some(dir) = doc.path.as_ref().and_then(|p| p.parent()) {
                d = d.set_directory(dir);
            }
            match d.save_file() {
                Some(p) => p.with_extension("gdr"),
                None => return false,
            }
        } else {
            doc.path.clone().unwrap()
        };
        match graphdraw_core::gdr::write_file(doc.doc(), &target, &Default::default()) {
            Ok(()) => {
                doc.editor.mark_saved();
                doc.path = Some(target.clone());
                doc.needs_save_as = false;
                self.status = format!("{} {}", tr("Salvo em", "Saved to"), target.display());
                true
            }
            Err(e) => {
                self.message = Some((
                    tr("Não foi possível salvar", "Could not save").into(),
                    format!("{}\n\n{e}", target.display()),
                ));
                false
            }
        }
    }

    fn export(&mut self, format: ExportFormat) {
        let Some(doc) = &self.doc else { return };
        let stem = doc
            .path
            .as_ref()
            .and_then(|p| p.file_stem())
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| tr("desenho", "drawing").into());
        let ext = format.extension();
        let many = format.single_page() && doc.doc().data.pages.len() > 1;
        let name = if many {
            format!("{stem}-{}.{ext}", doc.page + 1)
        } else {
            format!("{stem}.{ext}")
        };
        let Some(out) = rfd::FileDialog::new()
            .set_file_name(&name)
            .add_filter(ext.to_uppercase(), &[ext])
            .save_file()
        else {
            return;
        };
        let opts = ExportOptions {
            pages: if format.single_page() {
                vec![doc.page]
            } else {
                Vec::new()
            },
            dpi: doc.doc().data.settings.resolution,
            ..Default::default()
        };
        match export::export(doc.doc(), &out, format, &opts) {
            Ok(files) => {
                self.status = format!(
                    "{} {}",
                    tr("Exportado para", "Exported to"),
                    files
                        .first()
                        .map(|f| f.display().to_string())
                        .unwrap_or_default()
                )
            }
            Err(e) => {
                self.message = Some((
                    tr("Não foi possível exportar", "Could not export").into(),
                    format!("{}\n\n{e}", out.display()),
                ))
            }
        }
    }

    // ---- comandos de edição -------------------------------------------------

    fn undo(&mut self) {
        if let Some(d) = &mut self.doc {
            if self.canvas.is_dragging() {
                return;
            }
            if let Some(n) = d.editor.undo() {
                self.status = format!("{} {n}", tr("Desfeito:", "Undone:"));
            }
            d.prune_selection();
        }
    }

    fn redo(&mut self) {
        if let Some(d) = &mut self.doc {
            if let Some(n) = d.editor.redo() {
                self.status = format!("{} {n}", tr("Refeito:", "Redone:"));
            }
            d.prune_selection();
        }
    }

    fn delete(&mut self) {
        let Some(d) = &mut self.doc else { return };
        if self.tool == Tool::Shape && !d.nodes.is_empty() {
            if let Some((loc, o)) = crate::canvas::single_path(d).map(|s| (s.r, s.obj.clone())) {
                let mut no = o;
                if let ObjectKind::Path(p) = &mut no.kind {
                    graphdraw_core::edit::nodes::delete_nodes(p, &d.nodes);
                    let empty = p.subpaths.is_empty();
                    d.editor
                        .transaction(tr("Excluir nós", "Delete nodes"), |ed| {
                            if empty {
                                ed.remove_at(&loc);
                            } else {
                                ed.replace_at(&loc, no);
                            }
                        });
                }
                d.nodes.clear();
                d.prune_selection();
            }
            return;
        }
        let sel = std::mem::take(&mut d.selection);
        ops::delete(&mut d.editor, &sel);
    }

    fn select_all(&mut self) {
        if let Some(d) = &mut self.doc {
            d.selection = d
                .page()
                .layers
                .iter()
                .filter(|l| l.visible && !l.locked)
                .flat_map(|l| l.objects.iter().filter(|o| !o.locked).map(|o| o.id.clone()))
                .collect();
        }
    }

    fn with_selection(
        &mut self,
        f: impl FnOnce(&mut graphdraw_core::edit::Editor, &[String]) -> Option<Vec<String>>,
    ) {
        if let Some(d) = &mut self.doc {
            let sel = d.selection.clone();
            if sel.is_empty() {
                return;
            }
            if let Some(new_sel) = f(&mut d.editor, &sel) {
                d.selection = new_sel;
            }
        }
    }

    fn group(&mut self) {
        let Some(d) = &mut self.doc else { return };
        if d.selection.len() < 2 {
            return;
        }
        let sel = d.selection.clone();
        match ops::group(&mut d.editor, &sel) {
            Some(id) => d.selection = vec![id],
            None => {
                self.status = tr(
                    "Para agrupar, os objetos precisam estar na mesma camada e no mesmo grupo",
                    "To group, objects must be in the same layer and the same group",
                )
                .into()
            }
        }
    }

    fn zoom_selection(&mut self) {
        if let Some(d) = &self.doc
            && let Some(b) = ops::selection_bounds(&d.editor, &d.selection, true)
        {
            crate::canvas::zoom_to_rect(&mut self.canvas, b);
        }
    }

    fn zoom_all(&mut self) {
        if let Some(d) = &self.doc {
            let all = graphdraw_core::edit::query::union_bounds(
                d.page().layers.iter().flat_map(|l| l.objects.iter()),
                true,
            );
            match all {
                Some(b) => crate::canvas::zoom_to_rect(&mut self.canvas, b),
                None => self.canvas.fit_pending = true,
            }
        }
    }

    fn zoom_center(&mut self, factor: f64) {
        let s = self.canvas.canvas_size;
        self.canvas
            .view
            .zoom_at(factor, (s.x as f64 / 2.0, s.y as f64 / 2.0));
    }

    fn set_tool(&mut self, t: Tool) {
        if t != self.tool {
            if let Some(d) = &mut self.doc {
                self.canvas.finish_bezier(d, &self.defaults, false);
                d.nodes.clear();
            }
            self.tool = t;
        }
    }

    // ---- atalhos de teclado (esquema padrão próprio, GD-UI-10) ----------------

    fn shortcuts(&mut self, ctx: &egui::Context) {
        let cmd = |k| KeyboardShortcut::new(Modifiers::COMMAND, k);
        let cmd_shift = |k| KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::SHIFT, k);
        let fired = |s: KeyboardShortcut| ctx.input_mut(|i| i.consume_shortcut(&s));
        if fired(cmd_shift(Key::S)) {
            self.save(true);
        }
        if fired(cmd(Key::S)) {
            self.save(false);
        }
        if fired(cmd(Key::N)) {
            self.request(Pending::New, ctx);
        }
        if fired(cmd(Key::O)) {
            self.request(Pending::Open(None), ctx);
        }
        if fired(cmd_shift(Key::Z)) || fired(cmd(Key::Y)) {
            self.redo();
        }
        if fired(cmd(Key::Z)) {
            self.undo();
        }
        if fired(cmd(Key::D)) {
            let off = self.nudge.1 / 2.0;
            self.with_selection(|ed, s| Some(ops::duplicate(ed, s, Vec2::new(off, -off))));
        }
        if fired(cmd(Key::A)) {
            self.select_all();
        }
        if fired(cmd(Key::Q)) {
            self.with_selection(|ed, s| {
                ops::convert_to_curves(ed, s);
                None
            });
        }
        if fired(cmd(Key::G)) {
            self.group();
        }
        if fired(cmd(Key::U)) {
            self.with_selection(|ed, s| Some(ops::ungroup(ed, s)));
        }
        if fired(KeyboardShortcut::new(Modifiers::SHIFT, Key::PageUp)) {
            self.with_selection(|ed, s| {
                ops::reorder(ed, s, Order::ToFront);
                None
            });
        }
        if fired(KeyboardShortcut::new(Modifiers::SHIFT, Key::PageDown)) {
            self.with_selection(|ed, s| {
                ops::reorder(ed, s, Order::ToBack);
                None
            });
        }
        if fired(cmd(Key::PageUp)) {
            self.with_selection(|ed, s| {
                ops::reorder(ed, s, Order::Forward);
                None
            });
        }
        if fired(cmd(Key::PageDown)) {
            self.with_selection(|ed, s| {
                ops::reorder(ed, s, Order::Backward);
                None
            });
        }
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        let (keys, mods) = ctx.input(|i| (i.keys_down.clone(), i.modifiers));
        let pressed = |k| ctx.input(|i| i.key_pressed(k));
        if mods.is_none() || mods == Modifiers::SHIFT {
            for (k, t) in [
                (Key::V, Tool::Select),
                (Key::N, Tool::Shape),
                (Key::Z, Tool::Zoom),
                (Key::H, Tool::Pan),
                (Key::B, Tool::Bezier),
                (Key::R, Tool::Rect),
                (Key::E, Tool::Ellipse),
            ] {
                if mods.is_none() && pressed(k) {
                    self.set_tool(t);
                }
            }
        }
        if pressed(Key::Delete) || pressed(Key::Backspace) && self.tool != Tool::Bezier {
            self.delete();
        }
        if pressed(Key::Backspace) && self.tool == Tool::Bezier {
            // tratado no quadro (remove o último nó) — aqui só evita excluir objetos.
        }
        if pressed(Key::Enter)
            && self.tool == Tool::Bezier
            && let Some(d) = &mut self.doc
        {
            self.canvas.finish_bezier(d, &self.defaults, false);
        }
        if pressed(Key::Escape)
            && let Some(d) = &mut self.doc
        {
            if self.canvas.is_dragging() || self.canvas.has_draft() {
                self.canvas.cancel(d);
            } else {
                d.selection.clear();
                d.nodes.clear();
            }
        }
        // Setas: deslocamento fino; Shift = superdeslocamento (GD-FER-01).
        let step = if mods.shift {
            self.nudge.1
        } else {
            self.nudge.0
        };
        let mut d = Vec2::ZERO;
        if pressed(Key::ArrowLeft) {
            d.x -= step;
        }
        if pressed(Key::ArrowRight) {
            d.x += step;
        }
        if pressed(Key::ArrowUp) {
            d.y += step;
        }
        if pressed(Key::ArrowDown) {
            d.y -= step;
        }
        if d != Vec2::ZERO
            && let Some(doc) = &mut self.doc
        {
            if self.tool == Tool::Shape && !doc.nodes.is_empty() {
                if let Some((loc, o, m)) =
                    crate::canvas::single_path(doc).map(|s| (s.r, s.obj.clone(), s.m))
                {
                    let inv = m.inverse();
                    let ld = inv * kurbo::Point::new(d.x, d.y) - inv * kurbo::Point::ORIGIN;
                    let mut no = o;
                    if let ObjectKind::Path(p) = &mut no.kind {
                        graphdraw_core::edit::nodes::move_nodes(p, &doc.nodes, [ld.x, ld.y]);
                    }
                    doc.editor
                        .transaction(tr("Mover nós", "Move nodes"), |ed| ed.replace_at(&loc, no));
                }
            } else {
                let sel = doc.selection.clone();
                ops::translate(&mut doc.editor, &sel, d);
            }
        }
        if !mods.any() {
            if pressed(Key::Num0) {
                self.canvas.fit_pending = true;
            }
            if pressed(Key::Num1) {
                self.canvas.actual_pending = true;
            }
            if pressed(Key::Plus) || pressed(Key::Equals) {
                self.zoom_center(1.25);
            }
            if pressed(Key::Minus) {
                self.zoom_center(0.8);
            }
            if pressed(Key::PageDown)
                && let Some(d) = &mut self.doc
            {
                d.set_page(d.page + 1);
                self.canvas.fit_pending = true;
            }
            if pressed(Key::PageUp)
                && let Some(d) = &mut self.doc
                && d.page > 0
            {
                d.set_page(d.page - 1);
                self.canvas.fit_pending = true;
            }
        }
        let _ = keys;
    }

    // ---- interface ------------------------------------------------------------

    fn menu(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let has = self.doc.is_some();
        let has_sel = self.doc.as_ref().is_some_and(|d| !d.selection.is_empty());
        let can_undo = self.doc.as_ref().is_some_and(|d| d.editor.can_undo());
        let can_redo = self.doc.as_ref().is_some_and(|d| d.editor.can_redo());
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button(tr("Arquivo", "File"), |ui| {
                if ui.button(tr("Novo  (Ctrl+N)", "New  (Ctrl+N)")).clicked() {
                    self.request(Pending::New, &ctx);
                }
                if ui
                    .button(tr("Abrir…  (Ctrl+O)", "Open…  (Ctrl+O)"))
                    .clicked()
                {
                    self.request(Pending::Open(None), &ctx);
                }
                ui.separator();
                if ui
                    .add_enabled(
                        has,
                        egui::Button::new(tr("Salvar  (Ctrl+S)", "Save  (Ctrl+S)")),
                    )
                    .clicked()
                {
                    self.save(false);
                }
                if ui
                    .add_enabled(
                        has,
                        egui::Button::new(tr(
                            "Salvar como…  (Ctrl+Shift+S)",
                            "Save as…  (Ctrl+Shift+S)",
                        )),
                    )
                    .clicked()
                {
                    self.save(true);
                }
                ui.separator();
                if ui
                    .add_enabled(
                        has,
                        egui::Button::new(tr("Exportar página como SVG…", "Export page as SVG…")),
                    )
                    .clicked()
                {
                    self.export(ExportFormat::Svg);
                }
                if ui
                    .add_enabled(
                        has,
                        egui::Button::new(tr(
                            "Exportar documento como PDF…",
                            "Export document as PDF…",
                        )),
                    )
                    .clicked()
                {
                    self.export(ExportFormat::Pdf);
                }
                if ui
                    .add_enabled(
                        has,
                        egui::Button::new(tr("Exportar página como PNG…", "Export page as PNG…")),
                    )
                    .clicked()
                {
                    self.export(ExportFormat::Png);
                }
                ui.separator();
                if ui.button(tr("Sair", "Quit")).clicked() {
                    self.request(Pending::Quit, &ctx);
                }
            });
            ui.menu_button(tr("Editar", "Edit"), |ui| {
                if ui
                    .add_enabled(
                        can_undo,
                        egui::Button::new(tr("Desfazer  (Ctrl+Z)", "Undo  (Ctrl+Z)")),
                    )
                    .clicked()
                {
                    self.undo();
                }
                if ui
                    .add_enabled(
                        can_redo,
                        egui::Button::new(tr("Refazer  (Ctrl+Y)", "Redo  (Ctrl+Y)")),
                    )
                    .clicked()
                {
                    self.redo();
                }
                ui.separator();
                if ui
                    .add_enabled(
                        has_sel,
                        egui::Button::new(tr("Duplicar  (Ctrl+D)", "Duplicate  (Ctrl+D)")),
                    )
                    .clicked()
                {
                    let off = self.nudge.1 / 2.0;
                    self.with_selection(|ed, s| Some(ops::duplicate(ed, s, Vec2::new(off, -off))));
                }
                if ui
                    .add_enabled(
                        has_sel,
                        egui::Button::new(tr("Excluir  (Del)", "Delete  (Del)")),
                    )
                    .clicked()
                {
                    self.delete();
                }
                if ui
                    .add_enabled(
                        has,
                        egui::Button::new(tr("Selecionar tudo  (Ctrl+A)", "Select all  (Ctrl+A)")),
                    )
                    .clicked()
                {
                    self.select_all();
                }
            });
            ui.menu_button(tr("Objeto", "Object"), |ui| {
                if ui
                    .add_enabled(
                        has_sel,
                        egui::Button::new(tr(
                            "Converter em curvas  (Ctrl+Q)",
                            "Convert to curves  (Ctrl+Q)",
                        )),
                    )
                    .clicked()
                {
                    self.with_selection(|ed, s| {
                        ops::convert_to_curves(ed, s);
                        None
                    });
                }
                ui.separator();
                if ui
                    .add_enabled(
                        has_sel,
                        egui::Button::new(tr("Agrupar  (Ctrl+G)", "Group  (Ctrl+G)")),
                    )
                    .clicked()
                {
                    self.group();
                }
                if ui
                    .add_enabled(
                        has_sel,
                        egui::Button::new(tr("Desagrupar  (Ctrl+U)", "Ungroup  (Ctrl+U)")),
                    )
                    .clicked()
                {
                    self.with_selection(|ed, s| Some(ops::ungroup(ed, s)));
                }
                ui.separator();
                for (label, order) in [
                    (
                        tr(
                            "Trazer para frente  (Shift+PgUp)",
                            "Bring to front  (Shift+PgUp)",
                        ),
                        Order::ToFront,
                    ),
                    (
                        tr(
                            "Um passo à frente  (Ctrl+PgUp)",
                            "Bring forward  (Ctrl+PgUp)",
                        ),
                        Order::Forward,
                    ),
                    (
                        tr("Um passo atrás  (Ctrl+PgDn)", "Send backward  (Ctrl+PgDn)"),
                        Order::Backward,
                    ),
                    (
                        tr(
                            "Enviar para trás  (Shift+PgDn)",
                            "Send to back  (Shift+PgDn)",
                        ),
                        Order::ToBack,
                    ),
                    (tr("Inverter ordem", "Reverse order"), Order::Reverse),
                ] {
                    if ui.add_enabled(has_sel, egui::Button::new(label)).clicked() {
                        self.with_selection(|ed, s| {
                            ops::reorder(ed, s, order);
                            None
                        });
                    }
                }
            });
            ui.menu_button(tr("Exibir", "View"), |ui| {
                if ui
                    .button(tr("Ajustar à página  (0)", "Fit page  (0)"))
                    .clicked()
                {
                    self.canvas.fit_pending = true;
                }
                if ui
                    .button(tr("Tamanho real  (1)", "Actual size  (1)"))
                    .clicked()
                {
                    self.canvas.actual_pending = true;
                }
                if ui
                    .add_enabled(
                        has_sel,
                        egui::Button::new(tr("Zoom na seleção", "Zoom to selection")),
                    )
                    .clicked()
                {
                    self.zoom_selection();
                }
                if ui.button(tr("Zoom em tudo", "Zoom to all")).clicked() {
                    self.zoom_all();
                }
                ui.separator();
                ui.checkbox(&mut self.canvas.show_grid, tr("Mostrar grade", "Show grid"));
                ui.checkbox(
                    &mut self.canvas.snap.enabled,
                    tr("Encaixe ligado", "Snapping on"),
                );
                ui.checkbox(
                    &mut self.canvas.snap.objects,
                    tr("Encaixar em objetos", "Snap to objects"),
                );
                ui.checkbox(
                    &mut self.canvas.snap.page,
                    tr("Encaixar na página", "Snap to page"),
                );
                let mut grid_on = self.canvas.snap.grid.is_some();
                if ui
                    .checkbox(&mut grid_on, tr("Encaixar na grade", "Snap to grid"))
                    .changed()
                {
                    self.canvas.snap.grid = grid_on.then_some(5_000.0);
                }
                if let Some(g) = self.canvas.snap.grid {
                    ui.horizontal(|ui| {
                        ui.label(tr("Grade:", "Grid:"));
                        if let Some(v) = crate::widgets::length_field(ui, "grade", g, None) {
                            self.canvas.snap.grid = Some(v.max(10.0));
                        }
                    });
                }
                ui.separator();
                ui.checkbox(&mut self.show_right, tr("Painéis", "Panels"));
            });
            ui.menu_button(tr("Ajuda", "Help"), |ui| {
                if ui
                    .button(tr("Atalhos de teclado", "Keyboard shortcuts"))
                    .clicked()
                {
                    self.show_keys = true;
                }
                if ui
                    .button(tr("Sobre o GraphDraw", "About GraphDraw"))
                    .clicked()
                {
                    self.show_about = true;
                }
            });
        });
    }

    fn standard_toolbar(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let has = self.doc.is_some();
        let can_undo = self.doc.as_ref().is_some_and(|d| d.editor.can_undo());
        let can_redo = self.doc.as_ref().is_some_and(|d| d.editor.can_redo());
        ui.horizontal(|ui| {
            if icons::button(
                ui,
                "file-new",
                tr("Novo (Ctrl+N)", "New (Ctrl+N)"),
                false,
                true,
            )
            .clicked()
            {
                self.request(Pending::New, &ctx);
            }
            if icons::button(
                ui,
                "file-open",
                tr("Abrir (Ctrl+O)", "Open (Ctrl+O)"),
                false,
                true,
            )
            .clicked()
            {
                self.request(Pending::Open(None), &ctx);
            }
            if icons::button(
                ui,
                "file-save",
                tr("Salvar (Ctrl+S)", "Save (Ctrl+S)"),
                false,
                has,
            )
            .clicked()
            {
                self.save(false);
            }
            if icons::button(
                ui,
                "file-export",
                tr("Exportar PDF", "Export PDF"),
                false,
                has,
            )
            .clicked()
            {
                self.export(ExportFormat::Pdf);
            }
            ui.separator();
            if icons::button(
                ui,
                "undo",
                tr("Desfazer (Ctrl+Z)", "Undo (Ctrl+Z)"),
                false,
                can_undo,
            )
            .clicked()
            {
                self.undo();
            }
            if icons::button(
                ui,
                "redo",
                tr("Refazer (Ctrl+Y)", "Redo (Ctrl+Y)"),
                false,
                can_redo,
            )
            .clicked()
            {
                self.redo();
            }
            ui.separator();
            if icons::button(
                ui,
                "fit-page",
                tr("Ajustar à página (0)", "Fit page (0)"),
                false,
                has,
            )
            .clicked()
            {
                self.canvas.fit_pending = true;
            }
            if icons::button(
                ui,
                "fit-selection",
                tr("Zoom na seleção", "Zoom to selection"),
                false,
                has,
            )
            .clicked()
            {
                self.zoom_selection();
            }
            ui.separator();
            let snap_on = self.canvas.snap.enabled;
            if icons::button(
                ui,
                "snap",
                tr("Encaixe (liga/desliga)", "Snapping (on/off)"),
                snap_on,
                true,
            )
            .clicked()
            {
                self.canvas.snap.enabled = !snap_on;
            }
            let grid_on = self.canvas.show_grid;
            if icons::button(ui, "grid", tr("Mostrar grade", "Show grid"), grid_on, true).clicked()
            {
                self.canvas.show_grid = !grid_on;
                if self.canvas.show_grid && self.canvas.snap.grid.is_none() {
                    self.canvas.snap.grid = Some(5_000.0);
                }
            }
            ui.separator();
            for (icon, tip, tab) in [
                ("layers", tr("Camadas", "Layers"), RightTab::Layers),
                ("history", tr("Histórico", "History"), RightTab::History),
                (
                    "report",
                    tr("Relatório de importação", "Import report"),
                    RightTab::Report,
                ),
            ] {
                if icons::button(
                    ui,
                    icon,
                    tip,
                    self.show_right && self.right_tab == tab,
                    true,
                )
                .clicked()
                {
                    if self.show_right && self.right_tab == tab {
                        self.show_right = false;
                    } else {
                        self.show_right = true;
                        self.right_tab = tab;
                    }
                }
            }
        });
    }

    fn toolbox(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            for t in Tool::ALL {
                let (icon, tip, _) = t.info();
                if icons::button(ui, icon, tip, self.tool == t, true).clicked() {
                    self.set_tool(t);
                }
                if matches!(t, Tool::Shape | Tool::Pan) {
                    ui.separator();
                }
            }
        });
    }

    fn status_bar(&self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if self.loading.is_some() {
                ui.spinner();
            }
            if let Some(d) = &self.doc {
                let (w, h) = d.page_size();
                ui.label(format!(
                    "{} {}/{} · {} × {} mm",
                    tr("Página", "Page"),
                    d.page + 1,
                    d.doc().data.pages.len(),
                    fmt_mm(w),
                    fmt_mm(h)
                ));
                ui.separator();
                ui.label(format!(
                    "{} {:.0}%",
                    tr("Zoom", "Zoom"),
                    self.canvas.view.zoom / ACTUAL_SIZE * 100.0
                ));
                if let Some(p) = self.canvas.cursor_page {
                    ui.separator();
                    ui.label(format!("x {} mm  y {} mm", fmt_mm(p.x), fmt_mm(p.y)));
                }
                ui.separator();
                let n = d.selection.len();
                let what = match n {
                    0 => tr("Nada selecionado", "Nothing selected").to_string(),
                    1 => {
                        let kind = d
                            .selected_objects()
                            .first()
                            .map(|o| kind_label(&o.kind))
                            .unwrap_or_default();
                        format!("{} {kind}", tr("Selecionado:", "Selected:"))
                    }
                    n => format!("{n} {}", tr("objetos selecionados", "objects selected")),
                };
                ui.label(what);
                if d.editor.is_modified() {
                    ui.separator();
                    ui.label(tr("Alterações não salvas", "Unsaved changes"));
                }
            }
            if !self.status.is_empty() {
                ui.separator();
                ui.label(&self.status);
            }
        });
    }

    fn dialogs(&mut self, ctx: &egui::Context) {
        if let Some((title, body)) = self.message.clone() {
            egui::Window::new(title)
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.label(body);
                    if ui.button("OK").clicked() {
                        self.message = None;
                    }
                });
        }
        if let Some(action) = self.confirm.clone() {
            let mut close = false;
            egui::Window::new(tr("Alterações não salvas", "Unsaved changes"))
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.label(tr(
                        "O documento tem alterações não salvas. Deseja salvá-las?",
                        "The document has unsaved changes. Save them?",
                    ));
                    ui.horizontal(|ui| {
                        if ui.button(tr("Salvar", "Save")).clicked() {
                            close = true;
                            if self.save(false) {
                                self.run(action.clone(), ctx);
                            }
                        }
                        if ui.button(tr("Descartar", "Discard")).clicked() {
                            close = true;
                            self.run(action.clone(), ctx);
                        }
                        if ui.button(tr("Cancelar", "Cancel")).clicked() {
                            close = true;
                        }
                    });
                });
            if close {
                self.confirm = None;
            }
        }
        if self.show_about {
            egui::Window::new(tr("Sobre o GraphDraw", "About GraphDraw")).collapsible(false).resizable(false).open(&mut self.show_about).show(ctx, |ui| {
                ui.label(tr(
                    "GraphDraw — editor de ilustração vetorial open source (MPL-2.0) para gráficas, comunicação visual e corte.",
                    "GraphDraw — open source vector illustration editor (MPL-2.0) for print shops, signage and cutting.",
                ));
                ui.label(format!("v{}", env!("CARGO_PKG_VERSION")));
            });
        }
        if self.show_keys {
            egui::Window::new(tr("Atalhos de teclado", "Keyboard shortcuts"))
                .collapsible(false)
                .open(&mut self.show_keys)
                .show(ctx, |ui| {
                    egui::Grid::new("atalhos").striped(true).show(ui, |ui| {
                        for (k, d) in [
                            (
                                "V / N / Z / H",
                                tr(
                                    "Seleção / Forma / Zoom / Mão",
                                    "Select / Shape / Zoom / Hand",
                                ),
                            ),
                            (
                                "B / R / E",
                                tr(
                                    "Bézier / Retângulo / Elipse",
                                    "Bézier / Rectangle / Ellipse",
                                ),
                            ),
                            (
                                tr("Espaço (segurar)", "Space (hold)"),
                                tr("Mão temporária", "Temporary hand"),
                            ),
                            (
                                tr("Roda do mouse", "Mouse wheel"),
                                tr("Zoom no cursor", "Zoom at cursor"),
                            ),
                            (
                                "0 / 1 / + / −",
                                tr(
                                    "Ajustar página / tamanho real / aproximar / afastar",
                                    "Fit / actual size / zoom in / out",
                                ),
                            ),
                            (
                                tr("Setas (Shift)", "Arrows (Shift)"),
                                tr("Deslocar 1 mm (10 mm)", "Nudge 1 mm (10 mm)"),
                            ),
                            ("Ctrl+Z / Ctrl+Y", tr("Desfazer / refazer", "Undo / redo")),
                            (
                                "Ctrl+D / Del",
                                tr("Duplicar / excluir", "Duplicate / delete"),
                            ),
                            ("Ctrl+Q", tr("Converter em curvas", "Convert to curves")),
                            (
                                "Ctrl+G / Ctrl+U",
                                tr("Agrupar / desagrupar", "Group / ungroup"),
                            ),
                            (
                                tr("Ctrl+clique", "Ctrl+click"),
                                tr(
                                    "Selecionar objeto dentro do grupo",
                                    "Select object inside group",
                                ),
                            ),
                            (
                                tr("Shift ao arrastar", "Shift while dragging"),
                                tr(
                                    "Proporcional / somar à seleção",
                                    "Proportional / add to selection",
                                ),
                            ),
                            (
                                tr("Ctrl ao arrastar", "Ctrl while dragging"),
                                tr(
                                    "Do centro / restringir eixo / passos de 15°",
                                    "From center / constrain / 15° steps",
                                ),
                            ),
                            (
                                tr("Clique de novo na seleção", "Click selection again"),
                                tr("Alças de girar e inclinar", "Rotate and skew handles"),
                            ),
                        ] {
                            ui.monospace(k);
                            ui.label(d);
                            ui.end_row();
                        }
                    });
                });
        }
    }
}

fn kind_label(k: &ObjectKind) -> String {
    match k {
        ObjectKind::Path(_) => tr("curva", "curve"),
        ObjectKind::Rectangle(_) => tr("retângulo", "rectangle"),
        ObjectKind::Ellipse(_) => tr("elipse", "ellipse"),
        ObjectKind::Group(_) => tr("grupo", "group"),
        ObjectKind::Text(_) => tr("texto", "text"),
        ObjectKind::Image(_) => tr("imagem", "image"),
        ObjectKind::Unknown { .. } => tr("objeto desconhecido", "unknown object"),
    }
    .into()
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.poll_loading();
        // Arrastar-soltar (GD-CDR-01).
        if let Some(p) = ctx.input(|i| {
            i.raw.dropped_files.iter().find_map(|f| {
                let p = f.path();
                (!p.as_os_str().is_empty()).then(|| p.to_path_buf())
            })
        }) {
            self.request(Pending::Open(Some(p)), &ctx);
        }
        // Fechar a janela com alterações pendentes (GD-UND-07).
        if ctx.input(|i| i.viewport().close_requested()) && self.modified() && !self.allow_close {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.confirm = Some(Pending::Quit);
        }
        if let Some(d) = &self.doc {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(format!(
                "{} — GraphDraw",
                d.title()
            )));
        }
        self.shortcuts(&ctx);

        egui::Panel::top("menu").show(ui, |ui| self.menu(ui));
        egui::Panel::top("padrao").show(ui, |ui| self.standard_toolbar(ui));
        egui::Panel::top("propriedades").show(ui, |ui| self.property_bar(ui));
        egui::Panel::bottom("status").show(ui, |ui| self.status_bar(ui));
        egui::Panel::bottom("paleta").show(ui, |ui| self.palette_bar(ui));
        egui::Panel::left("ferramentas")
            .resizable(false)
            .exact_size(46.0)
            .show(ui, |ui| self.toolbox(ui));
        if self.show_right {
            egui::Panel::right("paineis")
                .default_size(260.0)
                .show(ui, |ui| self.right_panel(ui));
        }
        egui::CentralPanel::no_frame().show(ui, |ui| {
            if let Some(doc) = &mut self.doc {
                // Backspace na ferramenta Bézier remove o último nó do traçado.
                let ev = self.canvas.show(ui, doc, self.tool, &self.defaults);
                if let Some(t) = ev.switch_tool {
                    self.tool = t;
                }
                if let Some(s) = ev.status {
                    self.status = s;
                }
                doc.prune_selection();
            }
        });
        self.dialogs(&ctx);
    }
}
