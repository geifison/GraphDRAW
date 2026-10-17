//! Janela principal do GraphDraw (marco M1: editor básico).
//!
//! Estrutura (GD-UI-01): menus, barra de ferramentas padrão, barra de
//! propriedades contextual, caixa de ferramentas vertical, área de desenho,
//! painéis à direita, barra de paleta e barra de status.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, channel};

use eframe::egui::{self, Key};
use graphdraw_core::edit::ops::{self, Align, AlignTo, Anchor, Order};
use graphdraw_core::edit::query;
use graphdraw_core::export::{self, ExportFormat, ExportOptions};
use graphdraw_core::model::ObjectKind;
use graphdraw_core::report::Severity;
use graphdraw_core::{Document, ImportReport};
use kurbo::Vec2;

use crate::canvas::{CanvasState, Defaults, Tool};
use crate::doc::{DocState, default_stroke};
use crate::i18n::tr;
use crate::icons;
use crate::keymap::{self, Command, Keymap};
use crate::panels::{AlignUi, TransformUi};
use crate::theme;
use crate::view::ACTUAL_SIZE;
use crate::widgets::fmt_mm;

type OpenResult = Result<(Document, ImportReport), String>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RightTab {
    Transform,
    Align,
    Layers,
    Palettes,
    History,
    Pages,
    Report,
}

/// Opções da janela de contorno de deslocamento.
pub(crate) struct OffsetDialog {
    /// Distância em µm.
    distance: f64,
    outward: bool,
    join: graphdraw_core::edit::shapeops::Join,
}

/// Opções de "Converter em bitmap" (GD-CNV-03).
#[derive(Clone, Copy)]
pub(crate) struct BitmapDialog {
    dpi: f64,
    antialias: bool,
    transparent: bool,
}

impl Default for BitmapDialog {
    fn default() -> Self {
        BitmapDialog {
            dpi: 300.0,
            antialias: true,
            transparent: true,
        }
    }
}

impl Default for OffsetDialog {
    fn default() -> Self {
        OffsetDialog {
            distance: 2_000.0,
            outward: true,
            join: graphdraw_core::edit::shapeops::Join::Round,
        }
    }
}

/// Ação que pode exigir confirmar alterações não salvas (GD-UND-07).
#[derive(Clone)]
enum Pending {
    New,
    Open(Option<PathBuf>),
    /// Fechar a aba de documento com este índice.
    Close(usize),
    Quit,
}

/// Ação que "Repetir" refaz.
#[derive(Clone, Copy)]
enum LastAction {
    Command(Command),
    Nudge(Vec2),
}

/// Documento aberto numa aba (GD-UI-01: "área de desenho com abas por
/// documento"). Cada um tem a própria vista e a própria barra de páginas.
#[derive(Default)]
struct DocTab {
    doc: Option<DocState>,
    canvas: CanvasState,
    page_bar: crate::pagebar::PageBar,
}

pub struct App {
    pub(crate) doc: Option<DocState>,
    loading: Option<(PathBuf, Receiver<OpenResult>)>,
    pub(crate) tool: Tool,
    pub(crate) canvas: CanvasState,
    /// Objetos dentro das camadas: miniaturas, renomear e arrastar.
    pub(crate) layer_tree: crate::layertree::LayerTree,
    /// Seletor de fonte com prévia e busca.
    pub(crate) font_picker: crate::fontpicker::FontPicker,
    /// Paletas do documento e do usuário (GD-PAL-01..05).
    pub(crate) palettes: crate::paletteui::Palettes,
    /// Área de trabalho (GD-UI-03): recolhida, largura e painéis soltos;
    /// `ws_saved` é a última versão gravada no arquivo.
    pub(crate) ws: crate::workspace::Workspace,
    pub(crate) ws_saved: crate::workspace::Workspace,
    pub(crate) defaults: Defaults,
    pub(crate) right_tab: RightTab,
    pub(crate) show_right: bool,
    /// Arrasto em rótulo de campo numérico em andamento (transação aberta).
    field_drag: bool,
    /// Tema já enviado para a barra de título do sistema.
    window_theme: Option<egui::ThemePreference>,
    pub(crate) xf: TransformUi,
    pub(crate) align: AlignUi,
    pub(crate) anchor: Anchor,
    pub(crate) lock_ratio: bool,
    /// Cantos do retângulo que a barra altera (ordem do modelo: inf.-esq.,
    /// inf.-dir., sup.-dir., sup.-esq.).
    pub(crate) corners_sel: [bool; 4],
    message: Option<(String, String)>,
    confirm: Option<Pending>,
    allow_close: bool,
    show_about: bool,
    show_keys: bool,
    pub(crate) status: String,
    /// Deslocamento das setas (GD-FER-01): normal e com Shift.
    nudge: (f64, f64),
    /// Janela de contorno ou preenchimento aberta.
    style_dlg: Option<crate::dialogs::StyleDialog>,
    /// Barra de páginas em abas (doc 21), do documento ativo.
    page_bar: crate::pagebar::PageBar,
    /// Atalhos ativos (GD-UI-10) e a edição deles na janela de atalhos.
    keys: Keymap,
    key_capture: Option<Command>,
    keys_filter: String,
    /// Ferramenta anterior, para o comando "alternar com a Seleção".
    prev_tool: Option<Tool>,
    /// Última ação repetível (Repetir).
    last_action: Option<LastAction>,
    /// Janela "Exportar" (escolha do formato) aberta.
    export_choice: bool,
    /// Janela do contorno de deslocamento (GD-OFS-01) aberta.
    offset_dlg: Option<OffsetDialog>,
    bitmap_dlg: Option<BitmapDialog>,
    /// Exportar para corte (PLT/DXF) e gerar linha de corte (doc 17).
    cut_export: Option<crate::cutui::CutExport>,
    cut_line_dlg: Option<crate::cutui::CutLineDialog>,
    /// Janela de gerenciamento de cor (GD-CMS-02).
    color_dlg: Option<crate::colorui::ColorDialog>,
    /// Guias predefinidas (GD-GUI-02).
    preset_dlg: Option<crate::guideui::PresetDialog>,
    /// Janela "Novo documento" aberta (GD-DIM-10).
    new_doc: Option<crate::newdoc::NewDocDialog>,
    /// Condição de impressão escolhida para o PDF/X-1a (GD-EXP-07).
    print_condition: export::PrintCondition,
    /// Exportar texto em curvas em vez de texto vivo (GD-EXP-02).
    text_curves: bool,
    /// Tipo do próximo PDF exportado.
    pdf_standard: export::PdfStandard,
    /// Abas de documento. O ativo fica em `doc`/`canvas`/`page_bar`; a
    /// posição dele em `tabs` guarda só um lugar vazio.
    tabs: Vec<DocTab>,
    active: usize,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, initial: Option<PathBuf>) -> App {
        App::with_ctx(&cc.egui_ctx, initial)
    }

    /// Monta o app sobre um contexto do egui (também usado nos testes, sem janela).
    pub(crate) fn with_ctx(egui_ctx: &egui::Context, initial: Option<PathBuf>) -> App {
        egui_extras::install_image_loaders(egui_ctx);
        crate::theme::apply(egui_ctx);
        let mut app = App {
            doc: Some(DocState::new_blank()),
            loading: None,
            tool: Tool::Select,
            canvas: CanvasState::default(),
            defaults: Defaults {
                fill: None,
                stroke: Some(default_stroke()),
                ..Default::default()
            },
            right_tab: RightTab::Transform,
            layer_tree: Default::default(),
            font_picker: Default::default(),
            palettes: crate::paletteui::Palettes::load(),
            ws: Default::default(),
            ws_saved: Default::default(),
            show_right: true,
            field_drag: false,
            window_theme: None,
            xf: TransformUi::default(),
            align: AlignUi::default(),
            anchor: Anchor::Center,
            lock_ratio: true,
            corners_sel: [true; 4],
            message: None,
            confirm: None,
            allow_close: false,
            show_about: false,
            show_keys: false,
            status: String::new(),
            nudge: (1_000.0, 10_000.0),
            style_dlg: None,
            page_bar: Default::default(),
            tabs: vec![DocTab::default()],
            active: 0,
            keys: Keymap::load(),
            key_capture: None,
            keys_filter: String::new(),
            prev_tool: None,
            last_action: None,
            export_choice: false,
            offset_dlg: None,
            bitmap_dlg: None,
            cut_export: None,
            cut_line_dlg: None,
            color_dlg: None,
            preset_dlg: None,
            new_doc: None,
            print_condition: export::PrintCondition::Fogra39,
            pdf_standard: export::PdfStandard::Screen,
            text_curves: false,
        };
        keymap::set_current(&app.keys);
        // Testes não leem nem gravam o perfil do usuário.
        if !cfg!(test) {
            let ws = crate::workspace::Workspace::load();
            app.show_right = ws.panels;
            app.right_tab = ws.tab();
            app.tool = ws.tool();
            theme::set_density(egui_ctx, theme::Density::from_key(&ws.density));
            app.ws_saved = ws.clone();
            app.ws = ws;
        }
        if let Some(p) = initial {
            app.start_open(p, egui_ctx);
        }
        app
    }

    /// Algum documento aberto tem alterações não salvas?
    fn any_modified(&self) -> bool {
        (0..self.tabs.len()).any(|i| self.doc_modified(i))
    }

    fn doc_modified(&self, i: usize) -> bool {
        let d = if i == self.active {
            self.doc.as_ref()
        } else {
            self.tabs.get(i).and_then(|t| t.doc.as_ref())
        };
        d.is_some_and(|d| d.editor.is_modified())
    }

    // ---- abas de documento --------------------------------------------------

    /// Troca o documento ativo.
    fn switch_tab(&mut self, i: usize) {
        if i == self.active || i >= self.tabs.len() {
            return;
        }
        if let Some(d) = &mut self.doc {
            self.canvas.cancel(d);
        }
        self.style_dlg = None;
        self.tabs[self.active] = DocTab {
            doc: self.doc.take(),
            canvas: std::mem::take(&mut self.canvas),
            page_bar: std::mem::take(&mut self.page_bar),
        };
        let t = std::mem::take(&mut self.tabs[i]);
        self.doc = t.doc;
        self.canvas = t.canvas;
        self.page_bar = t.page_bar;
        self.active = i;
    }

    /// Abre um documento numa aba nova. Se a aba ativa for um documento em
    /// branco que ninguém tocou, ela é reaproveitada.
    fn open_in_tab(&mut self, doc: DocState) {
        let pristine = self.doc.as_ref().is_some_and(|d| {
            d.path.is_none()
                && !d.editor.is_modified()
                && d.doc().data.pages.len() == 1
                && d.doc().data.pages[0].object_count() == 0
        });
        let mut canvas = CanvasState::default();
        canvas.snap = self.canvas.snap;
        canvas.show_grid = self.canvas.show_grid;
        if !(pristine || self.doc.is_none() && !self.tabs.is_empty()) {
            if self.tabs.is_empty() {
                self.tabs.push(DocTab::default());
                self.active = 0;
            } else {
                self.tabs.push(DocTab::default());
                let last = self.tabs.len() - 1;
                self.switch_tab(last);
            }
        }
        if self.tabs.is_empty() {
            self.tabs.push(DocTab::default());
            self.active = 0;
        }
        self.style_dlg = None;
        self.doc = Some(doc);
        self.canvas = canvas;
        self.page_bar = Default::default();
    }

    /// Fecha a aba (sem perguntar; quem chama confirma antes).
    fn close_tab(&mut self, i: usize) {
        if i >= self.tabs.len() {
            return;
        }
        self.switch_tab(i);
        self.style_dlg = None;
        self.tabs.remove(self.active);
        self.doc = None;
        if self.tabs.is_empty() {
            self.active = 0;
            self.canvas.reset_for_new_document();
            self.page_bar = Default::default();
            return;
        }
        let next = self.active.min(self.tabs.len() - 1);
        let t = std::mem::take(&mut self.tabs[next]);
        self.doc = t.doc;
        self.canvas = t.canvas;
        self.page_bar = t.page_bar;
        self.active = next;
    }

    /// Índice da aba com este arquivo, se já estiver aberto.
    fn tab_with_path(&self, path: &std::path::Path) -> Option<usize> {
        let same = |d: &DocState| {
            d.path
                .as_ref()
                .is_some_and(|p| p == path || p.canonicalize().ok() == path.canonicalize().ok())
        };
        (0..self.tabs.len()).find(|&i| {
            if i == self.active {
                self.doc.as_ref().is_some_and(same)
            } else {
                self.tabs[i].doc.as_ref().is_some_and(same)
            }
        })
    }

    fn tab_title(&self, i: usize) -> Option<(String, Option<PathBuf>)> {
        let d = if i == self.active {
            self.doc.as_ref()
        } else {
            self.tabs.get(i).and_then(|t| t.doc.as_ref())
        }?;
        Some((d.title(), d.path.clone()))
    }

    /// Abas de documento no topo da área de trabalho (DESIGN-SYSTEM 4.5).
    fn doc_tabs(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let infos: Vec<crate::doctabs::TabInfo> = (0..self.tabs.len())
            .filter_map(|i| self.tab_title(i))
            .map(|(title, path)| crate::doctabs::TabInfo {
                title,
                tip: match path {
                    Some(p) => p.display().to_string(),
                    None => tr("Documento ainda não salvo", "Not saved yet").into(),
                },
            })
            .collect();
        let ev = crate::doctabs::strip(ui, &infos, self.active);
        if let Some(i) = ev.switch {
            self.switch_tab(i);
        }
        if let Some(i) = ev.close {
            self.request(Pending::Close(i), &ctx);
        }
        if ev.new {
            self.request(Pending::New, &ctx);
        }
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
                self.open_in_tab(DocState::from_document(doc, Some(path), report));
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

    /// Pede uma ação; fechar ou sair com alterações não salvas pergunta antes.
    /// Novo e Abrir criam outra aba, então não precisam perguntar.
    fn request(&mut self, action: Pending, ctx: &egui::Context) {
        match action {
            Pending::Close(i) if self.doc_modified(i) => {
                self.switch_tab(i);
                self.confirm = Some(Pending::Close(self.active));
            }
            Pending::Quit if self.any_modified() => {
                // Mostra um documento alterado por vez; ao resolver, pede de novo.
                let first = (0..self.tabs.len())
                    .find(|&i| self.doc_modified(i))
                    .unwrap();
                self.switch_tab(first);
                self.confirm = Some(Pending::Quit);
            }
            other => self.run(other, ctx),
        }
    }

    fn run(&mut self, action: Pending, ctx: &egui::Context) {
        match action {
            Pending::New => self.new_doc = Some(Default::default()),
            Pending::Open(Some(p)) => match self.tab_with_path(&p) {
                Some(i) => self.switch_tab(i),
                None => self.start_open(p, ctx),
            },
            Pending::Close(i) => self.close_tab(i),
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

    /// Exporta o documento em PDF/X-1a:2001 para gráfica (GD-EXP-07).
    fn export_pdfx(&mut self) {
        self.pdf_standard = export::PdfStandard::X1a(self.print_condition);
        self.export_pages(ExportFormat::Pdf, Vec::new());
        self.pdf_standard = export::PdfStandard::Screen;
    }

    /// Exporta a página atual (formatos de uma página) ou o documento (PDF).
    fn export(&mut self, format: ExportFormat) {
        let Some(doc) = &self.doc else { return };
        let pages = if format.single_page() {
            vec![doc.page]
        } else {
            Vec::new()
        };
        self.export_pages(format, pages);
    }

    /// Exporta as páginas dadas (vazio = todas), GD-PGT-19.
    fn export_pages(&mut self, format: ExportFormat, pages: Vec<usize>) {
        let Some(doc) = &self.doc else { return };
        let stem = doc
            .path
            .as_ref()
            .and_then(|p| p.file_stem())
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| tr("desenho", "drawing").into());
        let ext = format.extension();
        let many = doc.doc().data.pages.len() > 1;
        let name = match pages.as_slice() {
            [one] if many => format!("{stem}-{}.{ext}", one + 1),
            _ => format!("{stem}.{ext}"),
        };
        let Some(out) = rfd::FileDialog::new()
            .set_file_name(&name)
            .add_filter(ext.to_uppercase(), &[ext])
            .save_file()
        else {
            return;
        };
        let pdfx =
            format == ExportFormat::Pdf && matches!(self.pdf_standard, export::PdfStandard::X1a(_));
        let opts = ExportOptions {
            pages,
            dpi: doc.doc().data.settings.resolution,
            pdf: if format == ExportFormat::Pdf {
                self.pdf_standard
            } else {
                export::PdfStandard::Screen
            },
            text_to_curves: self.text_curves,
            ..Default::default()
        };
        match export::export_with_report(doc.doc(), &out, format, &opts) {
            Ok((files, warnings)) => {
                let path = files
                    .first()
                    .map(|f| f.display().to_string())
                    .unwrap_or_default();
                self.status = format!("{} {path}", tr("Exportado para", "Exported to"));
                if pdfx {
                    // Relatório do PDF/X-1a: o que foi convertido para a gráfica.
                    let body = if warnings.is_empty() {
                        tr(
                            "Nada precisou ser convertido: o documento já estava em CMYK, sem transparência e com imagens em boa resolução.",
                            "Nothing had to be converted: the document was already CMYK, without transparency and with good image resolution.",
                        )
                        .to_string()
                    } else {
                        warnings
                            .iter()
                            .map(|w| format!("• {w}"))
                            .collect::<Vec<_>>()
                            .join("\n")
                    };
                    self.message = Some((
                        tr("PDF/X-1a exportado", "PDF/X-1a exported").into(),
                        format!("{path}\n\n{body}"),
                    ));
                }
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
        if self.tool == Tool::Gradient && self.canvas.delete_grad_stop(d) {
            return;
        }
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

    /// Inverte a seleção entre os objetos de primeiro nível (GD-SEL-03).
    fn invert_selection(&mut self) {
        if let Some(d) = &mut self.doc {
            let page = d.page();
            let ids: Vec<String> = query::selectable(page)
                .into_iter()
                .map(|h| page.layers[h.layer].objects[h.index].id.clone())
                .filter(|id| !d.selection.contains(id))
                .collect();
            d.selection = ids;
            self.canvas.rotate_mode = false;
        }
    }

    /// Seleciona os objetos semelhantes ao primeiro selecionado (GD-SEL-03).
    fn select_similar(&mut self, by: query::Similar) {
        if let Some(d) = &mut self.doc
            && let Some(r) = d.selection.first().and_then(|id| d.editor.find(id))
            && let Some(reference) = d.editor.get(&r).cloned()
        {
            let page = d.page();
            d.selection = query::similar(page, &reference, by)
                .into_iter()
                .map(|h| page.layers[h.layer].objects[h.index].id.clone())
                .collect();
            self.canvas.rotate_mode = false;
        }
    }

    /// Objetos de texto alvo da barra de texto: o que está em edição ou os
    /// textos selecionados.
    pub(crate) fn text_targets(&self) -> Vec<String> {
        let Some(d) = &self.doc else {
            return Vec::new();
        };
        if let Some(t) = &self.canvas.text {
            return vec![t.id.clone()];
        }
        d.selection
            .iter()
            .filter(|id| {
                d.editor
                    .find(id)
                    .and_then(|r| d.editor.get(&r))
                    .is_some_and(|o| matches!(o.kind, ObjectKind::Text(_)))
            })
            .cloned()
            .collect()
    }

    /// Corpo do primeiro texto alvo.
    fn text_size(&self) -> Option<f64> {
        let d = self.doc.as_ref()?;
        let id = self.text_targets().into_iter().next()?;
        let r = d.editor.find(&id)?;
        match &d.editor.get(&r)?.kind {
            ObjectKind::Text(t) => Some(graphdraw_core::edit::text::current_style(t).1),
            _ => None,
        }
    }

    /// Trecho selecionado no texto em edição: objeto e `[início, fim)`.
    pub(crate) fn text_range(&self) -> Option<(String, usize, usize)> {
        let t = self.canvas.text.as_ref()?;
        (t.caret != t.anchor).then(|| (t.id.clone(), t.caret.min(t.anchor), t.caret.max(t.anchor)))
    }

    /// Onde a barra lê o estilo durante a edição: o primeiro caractere da
    /// seleção, ou o que está antes do cursor.
    pub(crate) fn text_style_index(&self) -> Option<usize> {
        let t = self.canvas.text.as_ref()?;
        Some(if t.caret != t.anchor {
            t.caret.min(t.anchor) + 1
        } else {
            t.caret
        })
    }

    /// Aplica fonte/corpo/estilo/alinhamento aos textos alvo (um passo, ou
    /// dentro da sessão de edição). Com trecho selecionado na edição, só
    /// nele (formatação por trecho, doc 06).
    pub(crate) fn text_style(&mut self, st: graphdraw_core::edit::text::TextStyle) {
        if let Some((id, a, b)) = self.text_range()
            && let Some(d) = &mut self.doc
        {
            ops::modify(
                &mut d.editor,
                &[id],
                tr("Estilo do trecho", "Range style"),
                |o| match &mut o.kind {
                    ObjectKind::Text(t) => {
                        graphdraw_core::edit::text::apply_style_range(t, a, b, &st);
                        true
                    }
                    _ => false,
                },
            );
            return;
        }
        let ids = self.text_targets();
        if let Some(d) = &mut self.doc
            && !ids.is_empty()
        {
            ops::modify(
                &mut d.editor,
                &ids,
                tr("Estilo do texto", "Text style"),
                |o| match &mut o.kind {
                    ObjectKind::Text(t) => {
                        graphdraw_core::edit::text::apply_style(t, &st);
                        true
                    }
                    _ => false,
                },
            );
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

    fn page_actions(&mut self, acts: Vec<crate::pagebar::PageAction>) {
        for a in acts {
            match a {
                crate::pagebar::PageAction::Switched => self.canvas.fit_pending = true,
                crate::pagebar::PageAction::Export(f, pages) => self.export_pages(f, pages),
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
            self.canvas.drop_three();
            if let Some(d) = &mut self.doc {
                self.canvas.end_text(d);
            }
            self.tool = t;
        }
    }

    // ---- atalhos de teclado (esquema padrão próprio, GD-UI-10) ----------------

    /// Atalhos (GD-UI-10): os comandos vêm do mapa ativo; só setas, Delete,
    /// Enter e Esc são fixos.
    fn shortcuts(&mut self, ctx: &egui::Context) {
        if self.key_capture.is_some() {
            return;
        }
        let editing = self.canvas.text.is_some();
        let state = keymap::State {
            selected: self.doc.as_ref().map_or(0, |d| d.selection.len()),
            editing_text: editing,
        };
        for c in self.keys.fired(ctx, state) {
            self.run_command(c, ctx);
        }
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        // Digitando no texto: o teclado é do quadro (GD-TXT-01/02).
        if editing {
            if let (Some(te), Some(d)) = (&mut self.canvas.text, &mut self.doc)
                && !te.handle_input(ctx, d)
            {
                self.canvas.end_text(d);
            }
            return;
        }
        let mods = ctx.input(|i| i.modifiers);
        let pressed = |k| ctx.input(|i| i.key_pressed(k));
        if pressed(Key::Delete) || pressed(Key::Backspace) && self.tool != Tool::Bezier {
            self.delete();
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
        if mods.ctrl || mods.alt {
            return;
        }
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
        if d != Vec2::ZERO {
            self.nudge_by(d);
            self.last_action = Some(LastAction::Nudge(d));
        }
    }

    /// Desloca a seleção (ou os nós, na ferramenta Forma).
    fn nudge_by(&mut self, d: Vec2) {
        let Some(doc) = &mut self.doc else { return };
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

    /// Executa um comando (de atalho, menu ou "Repetir").
    fn run_command(&mut self, c: Command, ctx: &egui::Context) {
        use Command as C;
        if !c.ready() {
            self.status = format!(
                "{}: {}",
                c.label(),
                tr(
                    "ainda não existe nesta versão (atalho reservado)",
                    "not available yet (shortcut reserved)"
                )
            );
            return;
        }
        if self.canvas.text.is_some()
            && !matches!(
                c,
                C::TextAlignLeft
                    | C::TextAlignCenter
                    | C::TextAlignRight
                    | C::TextJustify
                    | C::TextBigger
                    | C::TextSmaller
            )
            && let Some(d) = &mut self.doc
        {
            self.canvas.end_text(d);
        }
        let repeatable = matches!(
            c,
            C::Duplicate
                | C::PasteInPlace
                | C::ToCurves
                | C::OutlineToObject
                | C::Weld
                | C::Trim
                | C::Intersect
                | C::Simplify
                | C::FrontMinusBack
                | C::BackMinusFront
                | C::Exclude
                | C::Divide
                | C::Boundary
                | C::Combine
                | C::BreakApart
                | C::AlignTop
                | C::AlignBottom
                | C::AlignLeft
                | C::AlignRight
                | C::AlignCentersV
                | C::AlignCentersH
                | C::CenterOnPage
                | C::ToFront
                | C::ToBack
                | C::Forward
                | C::Backward
        );
        if repeatable {
            self.last_action = Some(LastAction::Command(c));
        }
        let align = |how| {
            move |ed: &mut graphdraw_core::edit::Editor, s: &[String]| {
                ops::align(ed, s, how, AlignTo::LastSelected, false);
                None
            }
        };
        let order = |how| {
            move |ed: &mut graphdraw_core::edit::Editor, s: &[String]| {
                ops::reorder(ed, s, how);
                None
            }
        };
        match c {
            C::New => self.request(Pending::New, ctx),
            C::Open => self.request(Pending::Open(None), ctx),
            C::Save => {
                self.save(false);
            }
            C::SaveAs => {
                self.save(true);
            }
            C::Import => self.import(),
            C::Export => self.export_choice = self.doc.is_some(),
            C::Close if !self.tabs.is_empty() => {
                let i = self.active;
                self.request(Pending::Close(i), ctx);
            }
            C::NextDoc if self.tabs.len() > 1 => {
                self.switch_tab((self.active + 1) % self.tabs.len())
            }
            C::PrevDoc if self.tabs.len() > 1 => {
                self.switch_tab((self.active + self.tabs.len() - 1) % self.tabs.len())
            }
            C::Undo => self.undo(),
            C::Redo => self.redo(),
            C::Duplicate => {
                let off = self.nudge.1 / 2.0;
                self.with_selection(|ed, s| Some(ops::duplicate(ed, s, Vec2::new(off, -off))));
            }
            C::PasteInPlace => {
                self.with_selection(|ed, s| Some(ops::duplicate(ed, s, Vec2::ZERO)));
            }
            C::Repeat => match self.last_action {
                Some(LastAction::Command(prev)) if prev != C::Repeat => self.run_command(prev, ctx),
                Some(LastAction::Nudge(d)) => self.nudge_by(d),
                _ => {
                    self.status = tr("Nada para repetir", "Nothing to repeat").into();
                }
            },
            C::SelectAll => self.select_all(),
            C::InvertSelection => self.invert_selection(),
            C::SelectSameKind => self.select_similar(query::Similar::Kind),
            C::SelectSameFill => self.select_similar(query::Similar::Fill),
            C::SelectSameStroke => self.select_similar(query::Similar::StrokeColor),
            C::ToCurves => self.with_selection(|ed, s| {
                ops::convert_to_curves(ed, s);
                None
            }),
            C::OutlineToObject => self.with_selection(|ed, s| {
                let n = ops::outline_to_object(ed, s);
                (!n.is_empty()).then_some(n)
            }),
            C::Group_ => self.group(),
            C::Weld | C::Trim | C::Intersect => {
                use graphdraw_core::edit::shapeops::{self, BoolOp};
                let op = match c {
                    C::Weld => BoolOp::Weld,
                    C::Trim => BoolOp::Trim,
                    _ => BoolOp::Intersect,
                };
                let mut done = false;
                self.with_selection(|ed, s| {
                    let r = shapeops::boolean(ed, s, op, op.default_options());
                    done = r.is_some();
                    r
                });
                if !done {
                    self.status = tr(
                        "Nada a fazer: selecione dois ou mais objetos com área em comum",
                        "Nothing to do: select two or more objects that overlap",
                    )
                    .into();
                }
            }
            C::Simplify
            | C::FrontMinusBack
            | C::BackMinusFront
            | C::Exclude
            | C::Divide
            | C::Boundary => {
                use graphdraw_core::edit::shapeops::{self, ShapeOp};
                let op = match c {
                    C::Simplify => ShapeOp::Simplify,
                    C::FrontMinusBack => ShapeOp::FrontMinusBack,
                    C::BackMinusFront => ShapeOp::BackMinusFront,
                    C::Exclude => ShapeOp::Exclude,
                    C::Divide => ShapeOp::Divide,
                    _ => ShapeOp::Boundary,
                };
                let mut done = false;
                self.with_selection(|ed, s| {
                    let r = shapeops::shape_op(ed, s, op);
                    done = r.is_some();
                    r
                });
                if !done {
                    self.status = if op == ShapeOp::Boundary {
                        tr(
                            "Nada a fazer: a seleção não tem área",
                            "Nothing to do: the selection has no area",
                        )
                    } else {
                        tr(
                            "Nada a fazer: selecione dois ou mais objetos que se sobrepõem",
                            "Nothing to do: select two or more objects that overlap",
                        )
                    }
                    .into();
                }
            }
            C::Combine => self.with_selection(|ed, s| {
                graphdraw_core::edit::shapeops::combine(ed, s).map(|id| vec![id])
            }),
            C::BreakApart => self.with_selection(|ed, s| {
                let mut out = Vec::new();
                for id in s {
                    out.extend(
                        graphdraw_core::edit::shapeops::separate(ed, id, true).unwrap_or_default(),
                    );
                }
                (!out.is_empty()).then_some(out)
            }),
            C::CutLine => {
                if self.doc.as_ref().is_some_and(|d| !d.selection.is_empty()) {
                    self.cut_line_dlg = Some(Default::default());
                }
            }
            C::RegistrationMarks => {
                if let Some(d) = &mut self.doc {
                    let page = d.page;
                    match graphdraw_core::edit::shapeops::registration_marks(
                        &mut d.editor,
                        page,
                        5_000.0,
                        5_000.0,
                        3,
                        graphdraw_core::export::cut::CUT_NAME,
                    ) {
                        Some(ids) => d.selection = ids,
                        None => {
                            self.status = tr(
                                "Página sem arte: nada para marcar",
                                "Empty page: nothing to mark",
                            )
                            .into()
                        }
                    }
                }
            }
            C::Offset => {
                if self.doc.as_ref().is_some_and(|d| !d.selection.is_empty()) {
                    self.offset_dlg = Some(OffsetDialog::default());
                }
            }
            C::Ungroup => self.with_selection(|ed, s| Some(ops::ungroup(ed, s))),
            C::ConvertToBitmap => {
                if self.doc.as_ref().is_some_and(|d| !d.selection.is_empty()) {
                    self.bitmap_dlg = Some(BitmapDialog::default());
                }
            }
            C::ClearTransformations => self.with_selection(|ed, s| {
                ops::clear_transformations(ed, s);
                Some(s.to_vec())
            }),
            C::TransformPosition | C::TransformRotate => {
                self.show_right = true;
                self.right_tab = RightTab::Transform;
                self.xf.mode = if c == C::TransformRotate { 1 } else { 0 };
            }
            C::AlignTop => self.with_selection(align(Align::Top)),
            C::AlignBottom => self.with_selection(align(Align::Bottom)),
            C::AlignLeft => self.with_selection(align(Align::Left)),
            C::AlignRight => self.with_selection(align(Align::Right)),
            C::AlignCentersV => self.with_selection(align(Align::CenterH)),
            C::AlignCentersH => self.with_selection(align(Align::CenterV)),
            C::CenterOnPage => {
                let Some((w, h)) = self.doc.as_ref().map(|d| d.page_size()) else {
                    return;
                };
                let page = kurbo::Rect::new(0.0, 0.0, w, h);
                self.with_selection(|ed, s| {
                    ed.transaction(tr("Centralizar na página", "Center on page"), |ed| {
                        ops::align(ed, s, Align::CenterH, AlignTo::Page(page), false);
                        ops::align(ed, s, Align::CenterV, AlignTo::Page(page), false);
                    });
                    None
                });
            }
            C::ToFront => self.with_selection(order(Order::ToFront)),
            C::ToBack => self.with_selection(order(Order::ToBack)),
            C::Forward => self.with_selection(order(Order::Forward)),
            C::Backward => self.with_selection(order(Order::Backward)),
            C::ZoomObjects => self.zoom_all(),
            C::ZoomPage => self.canvas.fit_pending = true,
            C::ActualSize => self.canvas.actual_pending = true,
            C::ZoomIn => self.zoom_center(1.25),
            C::ZoomOut => self.zoom_center(0.8),
            C::ToolSelect => self.set_tool(Tool::Select),
            C::ToolShape => self.set_tool(Tool::Shape),
            C::ToolZoom => self.set_tool(Tool::Zoom),
            C::ToolPan => self.set_tool(Tool::Pan),
            C::ToolBezier => self.set_tool(Tool::Bezier),
            C::ToolRect => self.set_tool(Tool::Rect),
            C::ToolEllipse => self.set_tool(Tool::Ellipse),
            C::ToolText => self.set_tool(Tool::Text),
            C::ToolPolygon => self.set_tool(Tool::Polygon),
            C::ToolInteractiveFill => self.set_tool(Tool::Gradient),
            C::TextAlignLeft | C::TextAlignCenter | C::TextAlignRight | C::TextJustify => {
                let align = match c {
                    C::TextAlignLeft => "left",
                    C::TextAlignCenter => "center",
                    C::TextAlignRight => "right",
                    _ => "justify",
                };
                self.text_style(graphdraw_core::edit::text::TextStyle {
                    align: Some(align.into()),
                    ..Default::default()
                });
            }
            C::TextBigger | C::TextSmaller => {
                // Um ponto por vez.
                let step = if c == C::TextBigger { 1.0 } else { -1.0 } * 25_400.0 / 72.0;
                if let Some(size) = self.text_size() {
                    self.text_style(graphdraw_core::edit::text::TextStyle {
                        size: Some((size + step).max(25_400.0 / 72.0)),
                        ..Default::default()
                    });
                }
            }
            C::ToolArtboard => self.set_tool(Tool::Artboard),
            C::ToolToggleSelect => {
                if self.tool != Tool::Select {
                    self.prev_tool = Some(self.tool);
                    self.set_tool(Tool::Select);
                } else if let Some(t) = self.prev_tool.take() {
                    self.set_tool(t);
                }
            }
            C::PageNext
            | C::PagePrev
            | C::PageFirst
            | C::PageLast
            | C::PageNew
            | C::PageJump
            | C::PageRename => {
                if let Some(d) = &mut self.doc {
                    let acts = self.page_bar.command(c, d);
                    self.page_actions(acts);
                }
            }
            _ => {}
        }
    }

    /// Importa desenho (.gdr/.cdr/.cmx: objetos da 1ª página, agrupados) ou
    /// imagem para a página atual, centralizado.
    fn import(&mut self) {
        if self.doc.is_none() {
            return;
        }
        let Some(path) = rfd::FileDialog::new()
            .add_filter(
                tr("Desenhos e imagens", "Drawings and images"),
                &[
                    "gdr", "cdr", "cmx", "png", "jpg", "jpeg", "bmp", "gif", "tif", "tiff", "webp",
                ],
            )
            .pick_file()
        else {
            return;
        };
        let doc = self.doc.as_mut().unwrap();
        match crate::import::into_page(doc, &path) {
            Ok(id) => {
                doc.selection = vec![id];
                self.status = format!("{} {}", tr("Importado:", "Imported:"), path.display());
            }
            Err(e) => {
                self.message = Some((
                    tr("Não foi possível importar", "Could not import").into(),
                    format!("{}\n\n{e}", path.display()),
                ))
            }
        }
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
                theme::menu_density(ui);
                if ui
                    .button(crate::keymap::menu(
                        tr("Novo", "New"),
                        crate::keymap::Command::New,
                    ))
                    .clicked()
                {
                    self.request(Pending::New, &ctx);
                }
                if ui
                    .button(crate::keymap::menu(
                        tr("Abrir…", "Open…"),
                        crate::keymap::Command::Open,
                    ))
                    .clicked()
                {
                    self.request(Pending::Open(None), &ctx);
                }
                ui.separator();
                if ui
                    .add_enabled(
                        has,
                        egui::Button::new(crate::keymap::menu(
                            tr("Salvar", "Save"),
                            crate::keymap::Command::Save,
                        )),
                    )
                    .clicked()
                {
                    self.save(false);
                }
                if ui
                    .add_enabled(
                        has,
                        egui::Button::new(crate::keymap::menu(
                            tr("Salvar como…", "Save as…"),
                            crate::keymap::Command::SaveAs,
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
                        egui::Button::new(keymap::menu(
                            tr("Importar…", "Import…"),
                            Command::Import,
                        )),
                    )
                    .clicked()
                {
                    self.run_command(Command::Import, &ctx);
                }
                if ui
                    .add_enabled(
                        has,
                        egui::Button::new(keymap::menu(
                            tr("Exportar…", "Export…"),
                            Command::Export,
                        )),
                    )
                    .clicked()
                {
                    self.run_command(Command::Export, &ctx);
                }
                if ui
                    .add_enabled(
                        has,
                        egui::Button::new(tr("Gerenciamento de cor…", "Color management…")),
                    )
                    .clicked()
                    && let Some(d) = &self.doc
                {
                    self.color_dlg = Some(crate::colorui::ColorDialog::new(
                        &d.doc().data.settings.color,
                    ));
                    ui.close();
                }
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
                            "Exportar documento como PDF (tela)…",
                            "Export document as PDF (screen)…",
                        )),
                    )
                    .clicked()
                {
                    self.export(ExportFormat::Pdf);
                }
                if ui
                    .add_enabled(
                        has,
                        egui::Button::new(tr(
                            "Exportar PDF/X-1a para gráfica…",
                            "Export PDF/X-1a for print…",
                        )),
                    )
                    .clicked()
                {
                    self.export_pdfx();
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
                if ui
                    .add_enabled(
                        has,
                        egui::Button::new(crate::keymap::menu(
                            tr("Fechar", "Close"),
                            crate::keymap::Command::Close,
                        )),
                    )
                    .clicked()
                {
                    let i = self.active;
                    self.request(Pending::Close(i), &ctx);
                }
                if ui.button(tr("Sair", "Quit")).clicked() {
                    self.request(Pending::Quit, &ctx);
                }
            });
            ui.menu_button(tr("Editar", "Edit"), |ui| {
                theme::menu_density(ui);
                if ui
                    .add_enabled(
                        can_undo,
                        egui::Button::new(crate::keymap::menu(
                            tr("Desfazer", "Undo"),
                            crate::keymap::Command::Undo,
                        )),
                    )
                    .clicked()
                {
                    self.undo();
                }
                if ui
                    .add_enabled(
                        can_redo,
                        egui::Button::new(crate::keymap::menu(
                            tr("Refazer", "Redo"),
                            crate::keymap::Command::Redo,
                        )),
                    )
                    .clicked()
                {
                    self.redo();
                }
                ui.separator();
                if ui
                    .add_enabled(
                        has_sel,
                        egui::Button::new(crate::keymap::menu(
                            tr("Duplicar", "Duplicate"),
                            crate::keymap::Command::Duplicate,
                        )),
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
                        egui::Button::new(crate::keymap::menu(
                            tr("Selecionar tudo", "Select all"),
                            crate::keymap::Command::SelectAll,
                        )),
                    )
                    .clicked()
                {
                    self.select_all();
                }
                for (label, cmd, enabled) in [
                    (
                        tr("Inverter seleção", "Invert selection"),
                        Command::InvertSelection,
                        has,
                    ),
                    (
                        tr("Selecionar mesmo tipo", "Select same type"),
                        Command::SelectSameKind,
                        has_sel,
                    ),
                    (
                        tr("Selecionar mesmo preenchimento", "Select same fill"),
                        Command::SelectSameFill,
                        has_sel,
                    ),
                    (
                        tr(
                            "Selecionar mesma cor de contorno",
                            "Select same outline color",
                        ),
                        Command::SelectSameStroke,
                        has_sel,
                    ),
                ] {
                    if ui
                        .add_enabled(enabled, egui::Button::new(keymap::menu(label, cmd)))
                        .clicked()
                    {
                        self.run_command(cmd, &ctx);
                    }
                }
                ui.separator();
                if ui
                    .add_enabled(
                        has_sel,
                        egui::Button::new(keymap::menu(
                            tr("Copiar no mesmo lugar", "Copy in place"),
                            Command::PasteInPlace,
                        )),
                    )
                    .clicked()
                {
                    self.run_command(Command::PasteInPlace, &ctx);
                }
                if ui
                    .add_enabled(
                        has,
                        egui::Button::new(keymap::menu(
                            tr("Repetir a última ação", "Repeat last action"),
                            Command::Repeat,
                        )),
                    )
                    .clicked()
                {
                    self.run_command(Command::Repeat, &ctx);
                }
            });
            ui.menu_button(tr("Objeto", "Object"), |ui| {
                theme::menu_density(ui);
                if ui
                    .add_enabled(
                        has_sel,
                        egui::Button::new(crate::keymap::menu(
                            tr("Converter em curvas", "Convert to curves"),
                            crate::keymap::Command::ToCurves,
                        )),
                    )
                    .clicked()
                {
                    self.with_selection(|ed, s| {
                        ops::convert_to_curves(ed, s);
                        None
                    });
                }
                if ui
                    .add_enabled(
                        has_sel,
                        egui::Button::new(keymap::menu(
                            tr("Converter contorno em objeto", "Convert outline to object"),
                            Command::OutlineToObject,
                        )),
                    )
                    .clicked()
                {
                    self.run_command(Command::OutlineToObject, &ctx);
                }
                ui.separator();
                let multi = self.doc.as_ref().is_some_and(|d| d.selection.len() >= 2);
                for (label, cmd, enabled) in [
                    (tr("Soldar", "Weld"), Command::Weld, multi),
                    (tr("Aparar", "Trim"), Command::Trim, multi),
                    (tr("Interseção", "Intersect"), Command::Intersect, multi),
                    (tr("Simplificar", "Simplify"), Command::Simplify, multi),
                    (
                        tr("Frente menos trás", "Front minus back"),
                        Command::FrontMinusBack,
                        multi,
                    ),
                    (
                        tr("Trás menos frente", "Back minus front"),
                        Command::BackMinusFront,
                        multi,
                    ),
                    (
                        tr("Excluir sobreposição", "Exclude overlap"),
                        Command::Exclude,
                        multi,
                    ),
                    (tr("Dividir", "Divide"), Command::Divide, multi),
                    (tr("Limite", "Boundary"), Command::Boundary, has_sel),
                    (tr("Combinar", "Combine"), Command::Combine, multi),
                    (tr("Separar", "Break apart"), Command::BreakApart, has_sel),
                    (
                        tr("Contorno de deslocamento…", "Offset path…"),
                        Command::Offset,
                        has_sel,
                    ),
                    (
                        tr("Gerar linha de corte…", "Create cut line…"),
                        Command::CutLine,
                        has_sel,
                    ),
                    (
                        tr("Marcas de registro", "Registration marks"),
                        Command::RegistrationMarks,
                        self.doc.is_some(),
                    ),
                ] {
                    if ui
                        .add_enabled(enabled, egui::Button::new(keymap::menu(label, cmd)))
                        .clicked()
                    {
                        self.run_command(cmd, &ctx);
                    }
                }
                // Mover/copiar para outra página (GD-CAM-03).
                if let Some(d) = &mut self.doc
                    && d.doc().data.pages.len() > 1
                {
                    for copy in [false, true] {
                        let title = if copy {
                            tr("Copiar para a página", "Copy to page")
                        } else {
                            tr("Mover para a página", "Move to page")
                        };
                        ui.add_enabled_ui(!d.selection.is_empty(), |ui| {
                            ui.menu_button(title, |ui| {
                                let names: Vec<String> = (0..d.doc().data.pages.len())
                                    .map(|p| {
                                        graphdraw_core::edit::pages::display_name(
                                            &d.doc().data.pages[p],
                                            p,
                                        )
                                    })
                                    .collect();
                                for (p, n) in names.iter().enumerate() {
                                    if p == d.page {
                                        continue;
                                    }
                                    if ui.button(n).clicked() {
                                        let sel = d.selection.clone();
                                        let layer = d.chosen_layer().unwrap_or(0);
                                        let ids =
                                            ops::move_to_page(&mut d.editor, &sel, p, layer, copy);
                                        if !copy {
                                            d.selection.clear();
                                        }
                                        self.status = format!(
                                            "{} {} → {n}",
                                            ids.len(),
                                            if copy {
                                                tr("copiado(s)", "copied")
                                            } else {
                                                tr("movido(s)", "moved")
                                            }
                                        );
                                        ui.close();
                                    }
                                }
                            });
                        });
                    }
                }
                ui.separator();
                if ui
                    .add_enabled(
                        has_sel,
                        egui::Button::new(keymap::menu(
                            tr("Centralizar na página", "Center on page"),
                            Command::CenterOnPage,
                        )),
                    )
                    .clicked()
                {
                    self.run_command(Command::CenterOnPage, &ctx);
                }
                ui.separator();
                if ui
                    .add_enabled(
                        has_sel,
                        egui::Button::new(crate::keymap::menu(
                            tr("Agrupar", "Group"),
                            crate::keymap::Command::Group_,
                        )),
                    )
                    .clicked()
                {
                    self.group();
                }
                if ui
                    .add_enabled(
                        has_sel,
                        egui::Button::new(crate::keymap::menu(
                            tr("Desagrupar", "Ungroup"),
                            crate::keymap::Command::Ungroup,
                        )),
                    )
                    .clicked()
                {
                    self.with_selection(|ed, s| Some(ops::ungroup(ed, s)));
                }
                if ui
                    .add_enabled(
                        has_sel,
                        egui::Button::new(crate::keymap::menu(
                            tr("Limpar transformações", "Clear transformations"),
                            crate::keymap::Command::ClearTransformations,
                        )),
                    )
                    .clicked()
                {
                    self.run_command(Command::ClearTransformations, &ctx);
                }
                if ui
                    .add_enabled(
                        has_sel,
                        egui::Button::new(crate::keymap::menu(
                            tr("Converter em bitmap…", "Convert to bitmap…"),
                            crate::keymap::Command::ConvertToBitmap,
                        )),
                    )
                    .clicked()
                {
                    self.run_command(Command::ConvertToBitmap, &ctx);
                }
                ui.separator();
                for (label, order) in [
                    (
                        crate::keymap::menu(
                            tr("Trazer para frente", "Bring to front"),
                            crate::keymap::Command::ToFront,
                        ),
                        Order::ToFront,
                    ),
                    (
                        crate::keymap::menu(
                            tr("Um passo à frente", "Bring forward"),
                            crate::keymap::Command::Forward,
                        ),
                        Order::Forward,
                    ),
                    (
                        crate::keymap::menu(
                            tr("Um passo atrás", "Send backward"),
                            crate::keymap::Command::Backward,
                        ),
                        Order::Backward,
                    ),
                    (
                        crate::keymap::menu(
                            tr("Enviar para trás", "Send to back"),
                            crate::keymap::Command::ToBack,
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
                theme::menu_density(ui);
                if ui
                    .button(crate::keymap::menu(
                        tr("Ajustar à página", "Fit page"),
                        crate::keymap::Command::ZoomPage,
                    ))
                    .clicked()
                {
                    self.canvas.fit_pending = true;
                }
                if ui
                    .button(crate::keymap::menu(
                        tr("Tamanho real", "Actual size"),
                        crate::keymap::Command::ActualSize,
                    ))
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
                ui.separator();
                ui.checkbox(&mut self.canvas.show_rulers, tr("Réguas", "Rulers"));
                ui.checkbox(&mut self.canvas.show_guides, tr("Guias", "Guides"));
                ui.checkbox(
                    &mut self.canvas.guides_locked,
                    tr("Travar guias", "Lock guides"),
                );
                ui.checkbox(
                    &mut self.canvas.snap.guides,
                    tr("Encaixar nas guias", "Snap to guides"),
                );
                ui.checkbox(
                    &mut self.canvas.smart_guides,
                    tr("Guias inteligentes", "Smart guides"),
                );
                if ui
                    .button(tr("Guias predefinidas…", "Guide presets…"))
                    .clicked()
                {
                    self.preset_dlg = Some(Default::default());
                    ui.close();
                }
                if ui
                    .add_enabled(
                        self.doc.is_some(),
                        egui::Button::new(tr("Remover guias da página", "Remove page guides")),
                    )
                    .clicked()
                    && let Some(d) = &mut self.doc
                {
                    graphdraw_core::edit::guides::clear(&mut d.editor, d.page, false);
                    ui.close();
                }
                ui.separator();
                ui.checkbox(&mut self.show_right, tr("Painéis", "Panels"));
                ui.menu_button(tr("Densidade dos painéis", "Panel density"), |ui| {
                    let cur = theme::density(ui.ctx());
                    for d in theme::Density::ALL {
                        if ui.radio(cur == d, d.label()).clicked() {
                            theme::set_density(ui.ctx(), d);
                            ui.close();
                        }
                    }
                });
                if ui
                    .button(tr("Restaurar área de trabalho padrão", "Reset workspace"))
                    .clicked()
                {
                    theme::set_density(ui.ctx(), theme::Density::Default);
                    let d = crate::workspace::Workspace::default();
                    self.show_right = d.panels;
                    self.right_tab = d.tab();
                    self.ws = d;
                    ui.close();
                }
                // Tema claro e escuro (GD-UI-04); o escuro é o padrão.
                ui.menu_button(tr("Tema", "Theme"), |ui| {
                    let cur = ui.ctx().options(|o| o.theme_preference);
                    for (pref, label) in [
                        (egui::ThemePreference::Dark, tr("Escuro", "Dark")),
                        (egui::ThemePreference::Light, tr("Claro", "Light")),
                        (egui::ThemePreference::System, tr("Do sistema", "System")),
                    ] {
                        if ui.radio(cur == pref, label).clicked() {
                            ui.ctx().set_theme(pref);
                        }
                    }
                });
            });
            ui.menu_button(tr("Ajuda", "Help"), |ui| {
                theme::menu_density(ui);
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
                crate::keymap::hint(tr("Novo", "New"), crate::keymap::Command::New),
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
                crate::keymap::hint(tr("Abrir", "Open"), crate::keymap::Command::Open),
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
                crate::keymap::hint(tr("Salvar", "Save"), crate::keymap::Command::Save),
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
                crate::keymap::hint(tr("Desfazer", "Undo"), crate::keymap::Command::Undo),
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
                crate::keymap::hint(tr("Refazer", "Redo"), crate::keymap::Command::Redo),
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
                crate::keymap::hint(
                    tr("Ajustar à página", "Fit page"),
                    crate::keymap::Command::ZoomPage,
                ),
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
                    if self.ws.is_floating(tab) {
                        // Já está numa janela própria.
                    } else if self.show_right && self.right_tab == tab && !self.ws.collapsed {
                        self.show_right = false;
                    } else {
                        self.show_right = true;
                        self.ws.collapsed = false;
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

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if self.loading.is_some() {
                ui.spinner();
            }
            if let Some(d) = &self.doc {
                let (w, h) = d.page_size();
                let which = format!(
                    "{} {}/{}",
                    tr("Página", "Page"),
                    d.page + 1,
                    d.doc().data.pages.len()
                );
                ui.label(format!("{which} · {} × {} mm", fmt_mm(w), fmt_mm(h)));
                if d.page_locked() {
                    ui.label(tr("· bloqueada", "· locked"));
                }
                ui.separator();
                ui.label(format!(
                    "{} {:.0}%",
                    tr("Zoom", "Zoom"),
                    self.canvas.view.zoom / ACTUAL_SIZE * 100.0
                ));
                ui.separator();
                cursor_position(ui, self.canvas.cursor_page);
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
            // Preenchimento e contorno atuais, à direita (GD-UI-01).
            if let Some(d) = &self.doc {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some(dlg) = crate::dialogs::status_indicators(ui, d, &self.defaults) {
                        self.style_dlg = Some(dlg);
                    }
                });
            }
        });
    }

    fn dialogs(&mut self, ctx: &egui::Context) {
        if let (Some(dlg), Some(doc)) = (&mut self.style_dlg, &mut self.doc)
            && dlg.show(ctx, doc, &mut self.defaults)
        {
            self.style_dlg = None;
        }
        if let Some((title, body)) = self.message.clone() {
            crate::widgets::modal(ctx, egui::Id::unique("mensagem")).show(ctx, |ui| {
                ui.heading(title);
                ui.add_space(theme::SPACE_300);
                ui.label(body);
                ui.add_space(theme::SPACE_300);
                crate::widgets::footer(ui, |ui| {
                    if ui.add(crate::widgets::primary_button("OK")).clicked()
                        || ui.input(|i| i.key_pressed(Key::Escape) || i.key_pressed(Key::Enter))
                    {
                        self.message = None;
                    }
                });
            });
        }
        if let Some(action) = self.confirm.clone() {
            // 0 = cancelar, 1 = salvar, 2 = descartar.
            let mut choice = None;
            let name = self.doc.as_ref().map(|d| d.title()).unwrap_or_default();
            crate::widgets::modal(ctx, egui::Id::unique("alteracoes-nao-salvas")).show(ctx, |ui| {
                ui.heading(tr("Alterações não salvas", "Unsaved changes"));
                ui.add_space(theme::SPACE_300);
                ui.label(format!(
                    "{} \"{}\" {}",
                    tr("O documento", "The document"),
                    name.trim_end_matches(" •"),
                    tr(
                        "tem alterações não salvas. Deseja salvá-las?",
                        "has unsaved changes. Save them?"
                    )
                ));
                ui.add_space(theme::SPACE_300);
                crate::widgets::footer(ui, |ui| {
                    if ui
                        .add(crate::widgets::primary_button(tr("Salvar", "Save")))
                        .clicked()
                    {
                        choice = Some(1);
                    }
                    if ui
                        .add(crate::widgets::secondary_button(tr("Descartar", "Discard")))
                        .clicked()
                    {
                        choice = Some(2);
                    }
                    if ui
                        .add(crate::widgets::secondary_button(tr("Cancelar", "Cancel")))
                        .clicked()
                        || ui.input(|i| i.key_pressed(Key::Escape))
                    {
                        choice = Some(0);
                    }
                });
            });
            if let Some(c) = choice {
                self.confirm = None;
                let ok = match c {
                    1 => self.save(false),
                    2 => true,
                    _ => false,
                };
                if ok {
                    match action {
                        // Sair: este documento está resolvido (salvo ou
                        // descartado); fecha a aba e pergunta pelo próximo.
                        Pending::Quit => {
                            self.close_tab(self.active);
                            self.request(Pending::Quit, ctx);
                        }
                        other => self.run(other, ctx),
                    }
                }
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
        if let Some(d) = &mut self.new_doc {
            match d.show(ctx) {
                crate::newdoc::Outcome::Open => {}
                crate::newdoc::Outcome::Cancel => self.new_doc = None,
                crate::newdoc::Outcome::Create(doc) => {
                    self.new_doc = None;
                    self.open_in_tab(*doc);
                }
            }
        }
        if self.export_choice {
            let mut pick = None;
            let mut pdfx = false;
            let mut cut_pick = None;
            let mut close = false;
            crate::widgets::modal(ctx, egui::Id::unique("escolher-exportacao")).show(ctx, |ui| {
                ui.set_width(320.0);
                ui.heading(tr("Exportar", "Export"));
                ui.add_space(theme::SPACE_300);
                for (label, f) in [
                    (
                        tr("PDF — documento inteiro", "PDF — whole document"),
                        ExportFormat::Pdf,
                    ),
                    (
                        tr("SVG — página atual", "SVG — current page"),
                        ExportFormat::Svg,
                    ),
                    (
                        tr("PNG — página atual", "PNG — current page"),
                        ExportFormat::Png,
                    ),
                ] {
                    if ui
                        .add(
                            egui::Button::new(label)
                                .min_size(egui::Vec2::new(300.0, theme::BUTTON_DEFAULT)),
                        )
                        .clicked()
                    {
                        pick = Some(f);
                    }
                }
                ui.checkbox(
                    &mut self.text_curves,
                    tr("Texto em curvas (sem fontes no arquivo)", "Text as curves (no fonts in the file)"),
                )
                .on_hover_text(tr(
                    "Desligado: PDF e SVG levam o texto vivo, com a fonte incorporada no PDF.",
                    "Off: PDF and SVG keep live text, with the font embedded in the PDF.",
                ));
                ui.add_space(theme::SPACE_300);
                ui.separator();
                ui.label(egui::RichText::new(tr("Para gráfica", "For print")).strong());
                if ui
                    .add(
                        egui::Button::new(tr(
                            "PDF/X-1a — documento inteiro",
                            "PDF/X-1a — whole document",
                        ))
                        .min_size(egui::Vec2::new(300.0, theme::BUTTON_DEFAULT)),
                    )
                    .on_hover_text(tr(
                        "Só CMYK e cores especiais, sem transparência, com sangria e área de corte (ISO 15930-1). RGB é convertido e a transparência é achatada; um relatório mostra o que mudou.",
                        "CMYK and spot colors only, no transparency, with bleed and trim (ISO 15930-1). RGB is converted and transparency flattened; a report shows what changed.",
                    ))
                    .clicked()
                {
                    pdfx = true;
                }
                ui.horizontal(|ui| {
                    ui.label(tr("Condição de impressão", "Print condition"));
                    egui::ComboBox::from_id_salt("condicao-impressao")
                        .selected_text(self.print_condition.description())
                        .width(220.0)
                        .show_ui(ui, |ui| {
                            for c in export::PrintCondition::ALL {
                                ui.selectable_value(&mut self.print_condition, c, c.description());
                            }
                        });
                });
                ui.add_space(theme::SPACE_300);
                ui.separator();
                ui.label(egui::RichText::new(tr("Para corte", "For cutting")).strong());
                for (label, f) in [
                    (
                        tr("PLT (HPGL) — plotter de recorte", "PLT (HPGL) — cutting plotter"),
                        crate::cutui::CutFormat::Plt,
                    ),
                    (tr("DXF — laser e CNC", "DXF — laser and CNC"), crate::cutui::CutFormat::Dxf),
                ] {
                    if ui
                        .add(egui::Button::new(label).min_size(egui::Vec2::new(300.0, theme::BUTTON_DEFAULT)))
                        .clicked()
                    {
                        cut_pick = Some(f);
                    }
                }
                ui.add_space(theme::SPACE_300);
                crate::widgets::footer(ui, |ui| {
                    if ui
                        .add(crate::widgets::secondary_button(tr("Cancelar", "Cancel")))
                        .clicked()
                        || ui.input(|i| i.key_pressed(Key::Escape))
                    {
                        close = true;
                    }
                });
            });
            if let Some(f) = cut_pick {
                self.export_choice = false;
                self.cut_export = Some(crate::cutui::CutExport::new(f));
            } else if let Some(f) = pick {
                self.export_choice = false;
                self.export(f);
            } else if pdfx {
                self.export_choice = false;
                self.export_pdfx();
            } else if close {
                self.export_choice = false;
            }
        }
        if self.show_keys {
            self.keys_window(ctx);
        }
        self.offset_window(ctx);
        self.bitmap_window(ctx);
        self.cut_windows(ctx);
        if let Some(dlg) = &mut self.preset_dlg
            && let Some(ok) = dlg.show(ctx)
        {
            let (presets, all) = (dlg.presets(), dlg.all_pages);
            self.preset_dlg = None;
            if ok && let Some(d) = &mut self.doc {
                let (w, h) = d.page_size();
                let g = graphdraw_core::edit::guides::presets(w, h, &presets);
                graphdraw_core::edit::guides::add(
                    &mut d.editor,
                    d.page,
                    all,
                    g,
                    tr("Guias predefinidas", "Guide presets"),
                );
            }
        }
        if let Some(dlg) = &mut self.color_dlg
            && let Some(ok) = dlg.show(ctx)
        {
            let cm = dlg.cm.clone();
            self.color_dlg = None;
            if ok && let Some(d) = &mut self.doc {
                d.editor
                    .transaction(tr("Gerenciamento de cor", "Color management"), |ed| {
                        ed.edit_settings(|s| s.color = cm);
                    });
            }
        }
    }

    /// Janelas da saída para corte (doc 17).
    fn cut_windows(&mut self, ctx: &egui::Context) {
        use crate::cutui::Outcome;
        if let (Some(dlg), Some(doc)) = (&mut self.cut_export, &self.doc) {
            match dlg.show(ctx, doc) {
                Outcome::Open => {}
                Outcome::Cancel => self.cut_export = None,
                Outcome::Save(format, opts) => {
                    self.cut_export = None;
                    self.save_cut(format, &opts);
                }
            }
        }
        if let Some(dlg) = &mut self.cut_line_dlg
            && let Some(ok) = dlg.show(ctx)
        {
            let (d, round, holes) = (dlg.distance, dlg.round, dlg.remove_holes);
            self.cut_line_dlg = None;
            if ok && let Some(doc) = &mut self.doc {
                let sel = doc.selection.clone();
                let page = doc.page;
                match graphdraw_core::edit::shapeops::cut_line(
                    &mut doc.editor,
                    &sel,
                    page,
                    d,
                    round,
                    holes,
                    graphdraw_core::export::cut::CUT_NAME,
                ) {
                    Some(id) => doc.selection = vec![id],
                    None => {
                        self.status = tr(
                            "A seleção não tem área para contornar",
                            "The selection has no area to outline",
                        )
                        .into()
                    }
                }
            }
        }
    }

    /// Grava o arquivo de corte da página atual.
    fn save_cut(
        &mut self,
        format: crate::cutui::CutFormat,
        opts: &graphdraw_core::export::cut::CutOptions,
    ) {
        use graphdraw_core::export::cut;
        let Some(doc) = &self.doc else { return };
        let ext = format.extension();
        let stem = doc
            .path
            .as_ref()
            .and_then(|p| p.file_stem())
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| tr("corte", "cut").into());
        let Some(out) = rfd::FileDialog::new()
            .set_file_name(format!("{stem}.{ext}"))
            .add_filter(ext.to_uppercase(), &[ext])
            .save_file()
        else {
            return;
        };
        let (pw, _) = doc.page_size();
        let strokes = cut::cut_strokes(doc.doc(), doc.page, opts);
        let text = match format {
            crate::cutui::CutFormat::Plt => cut::hpgl(&strokes, pw, opts),
            crate::cutui::CutFormat::Dxf => cut::dxf(&strokes, opts, pw),
        };
        self.status = match std::fs::write(&out, text) {
            Ok(()) => format!(
                "{} {}",
                tr("Corte exportado:", "Cut exported:"),
                out.display()
            ),
            Err(e) => format!("{} {e}", tr("Falha ao exportar:", "Export failed:")),
        };
    }

    /// Janela "Converter em bitmap" (GD-CNV-03): resolução, anti-serrilhado
    /// e fundo transparente ou branco.
    fn bitmap_window(&mut self, ctx: &egui::Context) {
        let Some(dlg) = &mut self.bitmap_dlg else {
            return;
        };
        let (mut apply, mut close) = (false, false);
        crate::widgets::modal(ctx, egui::Id::unique("converter-bitmap")).show(ctx, |ui| {
            ui.heading(tr("Converter em bitmap", "Convert to bitmap"));
            ui.add_space(theme::SPACE_300);
            ui.horizontal(|ui| {
                ui.label(tr("Resolução", "Resolution"));
                ui.add(
                    egui::DragValue::new(&mut dlg.dpi)
                        .range(10.0..=2400.0)
                        .speed(1.0)
                        .suffix(tr(" ppp", " dpi")),
                );
            });
            ui.checkbox(
                &mut dlg.antialias,
                tr(
                    "Suavizar bordas (anti-serrilhado)",
                    "Smooth edges (anti-aliasing)",
                ),
            );
            ui.checkbox(
                &mut dlg.transparent,
                tr("Fundo transparente", "Transparent background"),
            );
            ui.add_space(theme::SPACE_300);
            crate::widgets::footer(ui, |ui| {
                if ui
                    .add(crate::widgets::primary_button(tr("Converter", "Convert")))
                    .clicked()
                    || ui.input(|i| i.key_pressed(Key::Enter))
                {
                    apply = true;
                }
                if ui
                    .add(crate::widgets::secondary_button(tr("Cancelar", "Cancel")))
                    .clicked()
                    || ui.input(|i| i.key_pressed(Key::Escape))
                {
                    close = true;
                }
            });
        });
        if apply {
            let d = *dlg;
            self.bitmap_dlg = None;
            let mut made = false;
            self.with_selection(|ed, s| {
                let bg = (!d.transparent).then_some([255, 255, 255, 255]);
                let id = ops::convert_to_bitmap(ed, s, d.dpi, d.antialias, bg);
                made = id.is_some();
                id.map(|i| vec![i])
            });
            if !made {
                self.status = tr(
                    "Não deu para converter (seleção vazia ou imagem grande demais)",
                    "Could not convert (empty selection or image too large)",
                )
                .into();
            }
        } else if close {
            self.bitmap_dlg = None;
        }
    }

    /// Janela do contorno de deslocamento (GD-OFS-01).
    fn offset_window(&mut self, ctx: &egui::Context) {
        use graphdraw_core::edit::shapeops::{self, Join};
        let Some(dlg) = &mut self.offset_dlg else {
            return;
        };
        let mut apply = false;
        let mut close = false;
        crate::widgets::modal(ctx, egui::Id::unique("contorno-deslocamento")).show(ctx, |ui| {
            ui.heading(tr("Contorno de deslocamento", "Offset path"));
            ui.add_space(theme::SPACE_300);
            ui.horizontal(|ui| {
                ui.label(tr("Distância", "Distance"));
                if let Some(v) =
                    crate::widgets::length_field(ui, "deslocamento-d", dlg.distance, None)
                {
                    dlg.distance = v.max(0.0);
                }
            });
            ui.horizontal(|ui| {
                ui.radio_value(&mut dlg.outward, true, tr("Para fora", "Outside"));
                ui.radio_value(&mut dlg.outward, false, tr("Para dentro", "Inside"));
            });
            ui.horizontal(|ui| {
                ui.label(tr("Cantos", "Corners"));
                ui.radio_value(&mut dlg.join, Join::Miter, tr("Vivos", "Sharp"));
                ui.radio_value(&mut dlg.join, Join::Round, tr("Redondos", "Round"));
                ui.radio_value(&mut dlg.join, Join::Bevel, tr("Chanfrados", "Bevel"));
            });
            ui.add_space(theme::SPACE_300);
            crate::widgets::footer(ui, |ui| {
                if ui
                    .add(crate::widgets::primary_button(tr("Criar", "Create")))
                    .clicked()
                    || ui.input(|i| i.key_pressed(Key::Enter))
                {
                    apply = true;
                }
                if ui
                    .add(crate::widgets::secondary_button(tr("Cancelar", "Cancel")))
                    .clicked()
                    || ui.input(|i| i.key_pressed(Key::Escape))
                {
                    close = true;
                }
            });
        });
        if apply {
            let d = if dlg.outward {
                dlg.distance
            } else {
                -dlg.distance
            };
            let join = dlg.join;
            self.offset_dlg = None;
            let mut made = false;
            self.with_selection(|ed, s| {
                let ids = shapeops::offset(ed, s, d, join);
                made = !ids.is_empty();
                made.then_some(ids)
            });
            if !made {
                self.status = tr(
                    "O deslocamento não deixou área (distância maior que a forma?)",
                    "The offset left no area (distance larger than the shape?)",
                )
                .into();
            }
        } else if close {
            self.offset_dlg = None;
        }
    }

    /// Janela de atalhos (GD-UI-10): escolher perfil, ver e mudar atalhos,
    /// importar e exportar o mapa em JSON.
    fn keys_window(&mut self, ctx: &egui::Context) {
        // Captura da nova combinação.
        if let Some(cmd) = self.key_capture {
            let got = ctx.input(|i| {
                i.events.iter().find_map(|e| match e {
                    egui::Event::Key {
                        key,
                        pressed: true,
                        modifiers,
                        ..
                    } => Some((*key, *modifiers)),
                    _ => None,
                })
            });
            if let Some((key, m)) = got {
                match key {
                    Key::Escape => {}
                    Key::Backspace | Key::Delete if m.is_none() => self.keys.set(cmd, None),
                    _ => self.keys.set(
                        cmd,
                        Some(keymap::Chord {
                            key,
                            ctrl: m.ctrl || m.command,
                            shift: m.shift,
                            alt: m.alt,
                        }),
                    ),
                }
                self.key_capture = None;
                self.keys_changed();
            }
        }
        let mut open = true;
        let screen = ctx.content_rect();
        let mut new_profile = None;
        let mut import = false;
        let mut export_json = false;
        egui::Window::new(tr("Atalhos de teclado", "Keyboard shortcuts"))
            .open(&mut open)
            .collapsible(false)
            .default_size(egui::Vec2::new(620.0, screen.height() * 0.7))
            .max_size(screen.size() * 0.9)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(tr("Perfil", "Profile"));
                    egui::ComboBox::from_id_salt("perfil-atalhos")
                        .selected_text(self.keys.profile.label())
                        .width(240.0)
                        .show_ui(ui, |ui| {
                            for p in [keymap::Profile::GraphDraw, keymap::Profile::Alternative] {
                                if ui.selectable_label(self.keys.profile == p, p.label()).clicked() {
                                    new_profile = Some(p);
                                }
                            }
                        });
                    if ui.button(tr("Importar…", "Import…")).clicked() {
                        import = true;
                    }
                    if ui.button(tr("Exportar…", "Export…")).clicked() {
                        export_json = true;
                    }
                });
                ui.weak(tr(
                    "O perfil escolhido fica salvo e vale sempre que o GraphDraw abrir. Clique num atalho para trocar: pressione a nova combinação (Esc cancela, Backspace remove). Mudar um atalho cria o perfil Personalizado.",
                    "The chosen profile is saved and used every time GraphDraw starts. Click a shortcut to change it: press the new combination (Esc cancels, Backspace removes). Changing a shortcut creates the Custom profile.",
                ));
                ui.add(
                    egui::TextEdit::singleline(&mut self.keys_filter)
                        .hint_text(tr("Procurar comando…", "Search command…")),
                );
                ui.separator();
                let q = self.keys_filter.trim().to_lowercase();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for g in keymap::Group::ALL {
                        let cmds: Vec<Command> = Command::ALL
                            .iter()
                            .copied()
                            .filter(|c| c.group() == g)
                            .filter(|c| q.is_empty() || c.label().to_lowercase().contains(&q))
                            .collect();
                        if cmds.is_empty() {
                            continue;
                        }
                        ui.label(egui::RichText::new(g.label()).text_style(theme::section_style()));
                        egui::Grid::new(("atalhos", g.label()))
                            .num_columns(2)
                            .striped(true)
                            .min_col_width(260.0)
                            .show(ui, |ui| {
                                for c in cmds {
                                    let name = if c.ready() {
                                        egui::RichText::new(c.label())
                                    } else {
                                        egui::RichText::new(format!(
                                            "{}  {}",
                                            c.label(),
                                            tr("(em breve)", "(coming soon)")
                                        ))
                                        .weak()
                                    };
                                    ui.label(name);
                                    let keys: Vec<String> = self
                                        .keys
                                        .bindings
                                        .iter()
                                        .filter(|b| b.command == c)
                                        .map(|b| b.chord.text())
                                        .collect();
                                    let text = if self.key_capture == Some(c) {
                                        tr("Pressione as teclas…", "Press the keys…").to_string()
                                    } else if keys.is_empty() {
                                        "—".to_string()
                                    } else {
                                        keys.join("  ·  ")
                                    };
                                    if ui
                                        .add(
                                            egui::Button::new(egui::RichText::new(text).monospace())
                                                .min_size(egui::Vec2::new(150.0, theme::BUTTON_COMPACT)),
                                        )
                                        .clicked()
                                    {
                                        self.key_capture = Some(c);
                                    }
                                    ui.end_row();
                                }
                            });
                        ui.add_space(theme::SPACE_300);
                    }
                    ui.label(
                        egui::RichText::new(tr("Teclas fixas", "Fixed keys"))
                            .text_style(theme::section_style()),
                    );
                    egui::Grid::new("atalhos-fixos").num_columns(2).show(ui, |ui| {
                        for (k, d) in [
                            (tr("Setas (Shift)", "Arrows (Shift)"), tr("Deslocar 1 mm (10 mm)", "Nudge 1 mm (10 mm)")),
                            ("Delete", tr("Excluir", "Delete")),
                            ("Esc", tr("Cancelar / limpar seleção", "Cancel / clear selection")),
                            ("Enter", tr("Terminar curva Bézier", "Finish Bézier curve")),
                            (tr("Ctrl+clique", "Ctrl+click"), tr("Selecionar dentro do grupo", "Select inside group")),
                        ] {
                            ui.monospace(k);
                            ui.label(d);
                            ui.end_row();
                        }
                    });
                });
            });
        if !open {
            self.show_keys = false;
            self.key_capture = None;
        }
        if let Some(p) = new_profile {
            self.keys = Keymap::for_profile(p);
            self.keys_changed();
        }
        if export_json
            && let Some(path) = rfd::FileDialog::new()
                .set_file_name("atalhos.json")
                .add_filter("JSON", &["json"])
                .save_file()
            && let Err(e) = std::fs::write(&path, self.keys.to_json())
        {
            self.message = Some((
                tr("Não foi possível exportar", "Could not export").into(),
                e.to_string(),
            ));
        }
        if import
            && let Some(path) = rfd::FileDialog::new()
                .add_filter("JSON", &["json"])
                .pick_file()
        {
            match std::fs::read_to_string(&path)
                .map_err(|e| e.to_string())
                .and_then(|t| Keymap::from_json(&t))
            {
                Ok((k, skipped)) => {
                    self.keys = k;
                    self.keys.profile = keymap::Profile::Custom;
                    self.keys_changed();
                    if skipped > 0 {
                        self.status = format!(
                            "{skipped} {}",
                            tr(
                                "atalhos ignorados (comando ou tecla desconhecidos)",
                                "shortcuts skipped (unknown command or key)"
                            )
                        );
                    }
                }
                Err(e) => {
                    self.message = Some((
                        tr("Mapa de atalhos inválido", "Invalid shortcut map").into(),
                        e,
                    ));
                }
            }
        }
    }

    /// Salva o mapa e atualiza os rótulos de menus e dicas.
    fn keys_changed(&mut self) {
        keymap::set_current(&self.keys);
        if let Err(e) = self.keys.save() {
            self.status = format!("{} {e}", tr("Atalhos não salvos:", "Shortcuts not saved:"));
        }
    }
}

/// Posição do cursor na barra de status em largura fixa: fonte numérica,
/// sempre duas casas e cada número alinhado à direita numa célula do tamanho
/// de "-0000,00". Assim o resto da barra não treme quando o mouse anda, nem
/// pula quando o cursor sai do canvas (as células ficam vazias).
fn cursor_position(ui: &mut egui::Ui, p: Option<kurbo::Point>) {
    let font = theme::font_numeric();
    let num = |v: f64| {
        let s = format!("{:.2}", v / 1000.0);
        if crate::i18n::english() {
            s
        } else {
            s.replace('.', ",")
        }
    };
    let cell = ui
        .painter()
        .layout_no_wrap("-0000,00".into(), font.clone(), egui::Color32::WHITE)
        .size()
        .x;
    let height = ui.spacing().interact_size.y;
    for (axis, v) in [("x", p.map(|p| p.x)), ("y", p.map(|p| p.y))] {
        ui.label(axis);
        ui.allocate_ui_with_layout(
            egui::Vec2::new(cell, height),
            egui::Layout::right_to_left(egui::Align::Center),
            |ui| {
                ui.set_min_width(cell);
                ui.label(egui::RichText::new(v.map(num).unwrap_or_default()).font(font.clone()));
            },
        );
        ui.label("mm");
    }
}

pub(crate) fn kind_label(k: &ObjectKind) -> String {
    match k {
        ObjectKind::Path(_) => tr("curva", "curve"),
        ObjectKind::Rectangle(_) => tr("retângulo", "rectangle"),
        ObjectKind::Ellipse(_) => tr("elipse", "ellipse"),
        ObjectKind::Polygon(p) => match p.mode.as_str() {
            "star" => tr("estrela", "star"),
            "complex" => tr("estrela complexa", "complex star"),
            _ => tr("polígono", "polygon"),
        },
        ObjectKind::Group(_) => tr("grupo", "group"),
        ObjectKind::Text(_) => tr("texto", "text"),
        ObjectKind::Image(_) => tr("imagem", "image"),
        ObjectKind::Unknown { .. } => tr("objeto desconhecido", "unknown object"),
    }
    .into()
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.frame(ui);
    }
}

impl App {
    /// Um quadro da interface inteira.
    pub(crate) fn frame(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        self.poll_loading();
        theme::sync_window_theme(&ctx, &mut self.window_theme);
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
        if ctx.input(|i| i.viewport().close_requested()) && self.any_modified() && !self.allow_close
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            if self.confirm.is_none() {
                self.request(Pending::Quit, &ctx);
            }
        }
        if let Some(d) = &self.doc {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(format!(
                "{} — GraphDraw",
                d.title()
            )));
        }
        self.shortcuts(&ctx);
        if let Some(d) = &mut self.doc {
            d.sync();
            // Perfis do documento ativo valem para a tela (GD-CMS-01).
            graphdraw_core::cms::activate(&d.doc().data.settings.color);
        }
        self.canvas.space_pans = self.keys.space_is_free();

        egui::Panel::top("menu").show(ui, |ui| self.menu(ui));
        egui::Panel::top("padrao").show(ui, |ui| self.standard_toolbar(ui));
        crate::widgets::property_panel(&ctx).show(ui, |ui| self.property_bar(ui));
        // Barra de status em `bg-surface-0`.
        egui::Panel::bottom("status")
            .frame(
                egui::Frame::side_top_panel(&ctx.global_style())
                    .fill(theme::palette_for(&ctx.global_style().visuals).bg_surface_0),
            )
            .show(ui, |ui| self.status_bar(ui));
        egui::Panel::bottom("paleta").show(ui, |ui| self.palette_bar(ui));
        egui::Panel::left("ferramentas")
            .resizable(false)
            .exact_size(theme::BUTTON_COMPACT + 2.0 * theme::SPACE_200)
            .show(ui, |ui| self.toolbox(ui));
        let docked = self.ws.docked();
        if !docked.contains(&self.right_tab)
            && let Some(t) = docked.first()
        {
            self.right_tab = *t;
        }
        if self.show_right && !docked.is_empty() {
            if self.ws.collapsed {
                egui::Panel::right("paineis-recolhidos")
                    .resizable(false)
                    .exact_size(theme::PANEL_RAIL)
                    .show(ui, |ui| self.panel_rail(ui, &docked));
            } else {
                let r = egui::Panel::right("paineis")
                    .default_size(self.ws.width)
                    .size_range(theme::PANEL_MIN_WIDTH..=theme::PANEL_MAX_WIDTH)
                    .show(ui, |ui| self.right_panel(ui));
                self.ws.width = r.response.rect.width().round();
            }
        }
        egui::CentralPanel::no_frame().show(ui, |ui| {
            // Abas de documento no topo da área de trabalho; as páginas do
            // documento ativo ficam logo abaixo do desenho (GD-UI-01, doc 21).
            egui::Panel::top("documentos")
                .frame(egui::Frame::NONE)
                .show_separator_line(false)
                .resizable(false)
                .exact_size(theme::TAB_DOC_HEIGHT)
                .show(ui, |ui| self.doc_tabs(ui));
            if self.doc.is_none() {
                ui.centered_and_justified(|ui| {
                    ui.weak(tr(
                        "Nenhum documento aberto — Ctrl+N cria um, Ctrl+O abre um arquivo",
                        "No document open — Ctrl+N creates one, Ctrl+O opens a file",
                    ));
                });
                return;
            }
            egui::Panel::bottom("paginas").show(ui, |ui| {
                if let Some(d) = &mut self.doc {
                    let acts = self.page_bar.show(ui, d);
                    self.page_actions(acts);
                }
            });
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
        // Arrasto em rótulo de campo numérico: o arrasto inteiro vira um passo
        // só de desfazer (DESIGN-SYSTEM 4.2).
        let dragging = crate::widgets::field_dragging(&ctx);
        if let Some(d) = &mut self.doc {
            if dragging && !self.field_drag && !d.editor.in_transaction() {
                d.editor.begin(tr("Ajustar valor", "Adjust value"));
                self.field_drag = true;
            } else if !dragging && self.field_drag {
                d.editor.commit();
                self.field_drag = false;
            }
        }
        self.floating_panels(&ctx);
        self.dialogs(&ctx);
        self.save_workspace(&ctx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(k: Key, mods: egui::Modifiers) -> egui::Event {
        egui::Event::Key {
            key: k,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: mods,
        }
    }

    /// GD-CNV-03 / DS 4.4: a janela "Converter em bitmap" cabe em 1366 × 768.
    #[test]
    fn bitmap_window_fits_small_screen() {
        let ctx = crate::theme::test_ctx();
        let mut app = App::with_ctx(&ctx, None);
        app.bitmap_dlg = Some(BitmapDialog::default());
        let mut size = egui::Vec2::ZERO;
        for _ in 0..3 {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::Vec2::new(1366.0, 768.0),
                )),
                ..Default::default()
            };
            let mut out = ctx.run_ui(input, |ui| app.bitmap_window(ui.ctx()));
            out.textures_delta.clear();
            size = ctx.memory(|m| {
                m.area_rect(egui::Id::unique("converter-bitmap"))
                    .map_or(egui::Vec2::ZERO, |r| r.size())
            });
        }
        assert!(
            size.y > 0.0 && size.y <= 768.0 && size.x <= 1366.0,
            "{size:?}"
        );
        assert!(app.bitmap_dlg.is_some());
    }

    /// Doc 06: com um trecho selecionado na edição, estilo e cor da paleta
    /// valem só para ele; a barra mostra o estilo do ponto do cursor.
    #[test]
    fn text_range_style_and_palette_color() {
        use graphdraw_core::edit::text::{TextStyle, insert, new_text};
        let ctx = crate::theme::test_ctx();
        let mut app = App::with_ctx(&ctx, None);
        let id = {
            let d = app.doc.as_mut().unwrap();
            let mut o = new_text(10_000.0, 10_000.0, None, "Arial", 5_000.0);
            if let ObjectKind::Text(t) = &mut o.kind {
                insert(t, 0, "abcdef");
            }
            ops::add_object(&mut d.editor, 0, 0, o, "t")
        };
        let d = app.doc.as_mut().unwrap();
        app.canvas.edit_text(d, id.clone(), None);
        let te = app.canvas.text.as_mut().unwrap();
        (te.anchor, te.caret) = (2, 4);
        app.tool = Tool::Text;
        app.text_style(TextStyle {
            bold: Some(true),
            ..Default::default()
        });
        let red = graphdraw_core::model::Color::from_hex("#ff0000").unwrap();
        app.apply_pal(crate::paletteui::PalAction::Fill(Some(red.clone())));
        let spans = |app: &App| {
            let d = app.doc.as_ref().unwrap();
            match &d.editor.get(&d.editor.find(&id).unwrap()).unwrap().kind {
                ObjectKind::Text(t) => t.paragraphs[0].spans.clone(),
                _ => unreachable!(),
            }
        };
        let s = spans(&app);
        let got: Vec<(&str, u16, bool)> = s
            .iter()
            .map(|s| (s.text.as_str(), s.font.weight, s.fill.is_some()))
            .collect();
        assert_eq!(
            got,
            vec![("ab", 400, false), ("cd", 700, true), ("ef", 400, false)]
        );
        assert_eq!(app.text_style_index(), Some(3));
        // Sem seleção, volta a valer para o objeto todo.
        let te = app.canvas.text.as_mut().unwrap();
        te.anchor = te.caret;
        app.text_style(TextStyle {
            italic: Some(true),
            ..Default::default()
        });
        assert!(spans(&app).iter().all(|s| s.font.italic));
    }

    /// Só trocar de ferramenta, Esc e selecionar tudo não marcam o
    /// documento como alterado (título sem "•").
    #[test]
    fn keys_without_edits_keep_document_unmodified() {
        let ctx = crate::theme::test_ctx();
        let mut app = App::with_ctx(&ctx, None);
        {
            let d = app.doc.as_mut().unwrap();
            let o = graphdraw_core::model::Object::new(ObjectKind::Rectangle(
                graphdraw_core::model::RectData {
                    width: 10_000.0,
                    height: 10_000.0,
                    corners: None,
                    extra: Default::default(),
                },
            ));
            ops::add_object(&mut d.editor, 0, 0, o, "r");
            d.editor.mark_saved();
        }
        app.tool = Tool::Text;
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::new(1366.0, 768.0));
        let frame = |app: &mut App, mut events: Vec<egui::Event>| {
            let modifiers = events
                .iter()
                .find_map(|e| match e {
                    egui::Event::Key { modifiers, .. } => Some(*modifiers),
                    _ => None,
                })
                .unwrap_or_default();
            events.insert(0, egui::Event::ModifiersChanged(modifiers));
            let mut out = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(screen),
                    events,
                    ..Default::default()
                },
                |ui| app.frame(ui),
            );
            out.textures_delta.clear();
        };
        for _ in 0..3 {
            frame(&mut app, vec![]);
        }
        let steps: Vec<(&str, Vec<egui::Event>)> = vec![
            ("Esc", vec![key(Key::Escape, egui::Modifiers::NONE)]),
            (
                "V",
                vec![
                    egui::Event::Text("v".into()),
                    key(Key::V, egui::Modifiers::NONE),
                ],
            ),
            (
                "Ctrl+A",
                vec![key(
                    Key::A,
                    egui::Modifiers::CTRL | egui::Modifiers::COMMAND,
                )],
            ),
        ];
        for (name, ev) in steps {
            frame(&mut app, ev);
            frame(&mut app, vec![]);
            let d = app.doc.as_ref().unwrap();
            assert!(
                !d.editor.is_modified(),
                "{name} alterou o documento: {:?}",
                d.editor.history()
            );
        }
        assert_eq!(app.tool, Tool::Select);
        assert_eq!(app.doc.as_ref().unwrap().selection.len(), 1);
    }
}
