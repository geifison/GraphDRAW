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
    Preflight,
    Styles,
}

/// Opções da janela de contorno de deslocamento.
pub(crate) struct OffsetDialog {
    /// Distância em µm.
    distance: f64,
    outward: bool,
    join: graphdraw_core::edit::shapeops::Join,
}

/// Cor da paleta escolhida sem seleção, esperando a resposta "vira padrão
/// para novos objetos?" (GD-BDR-10).
#[derive(Clone)]
pub(crate) enum DefaultAsk {
    Fill(Option<graphdraw_core::model::Fill>),
    Stroke(Option<graphdraw_core::model::Color>),
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

/// Fim de uma exportação em segundo plano: texto da barra de status e,
/// se houver, uma mensagem (título, corpo).
pub(crate) struct ExportDone {
    pub status: String,
    pub message: Option<(String, String)>,
}

/// Exportação em andamento (GD-UI-15): nome do arquivo e o resultado.
pub(crate) struct Exporting {
    pub name: String,
    rx: Receiver<ExportDone>,
}

pub struct App {
    pub(crate) doc: Option<DocState>,
    loading: Option<(PathBuf, Receiver<OpenResult>)>,
    /// Exportação rodando em segundo plano (a barra de status mostra).
    pub(crate) exporting: Option<Exporting>,
    /// Contexto da interface, para a exportação pedir um quadro ao terminar.
    egui_ctx: egui::Context,
    pub(crate) tool: Tool,
    pub(crate) canvas: CanvasState,
    /// Objetos dentro das camadas: miniaturas, renomear e arrastar.
    pub(crate) layer_tree: crate::layertree::LayerTree,
    /// Seletor de fonte com prévia e busca.
    pub(crate) font_picker: crate::fontpicker::FontPicker,
    /// Paletas do documento e do usuário (GD-PAL-01..05).
    pub(crate) palettes: crate::paletteui::Palettes,
    pub(crate) styles_panel: crate::stylesui::StylesPanel,
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
    pub(crate) ask_default: Option<DefaultAsk>,
    /// Documento aberto com perfis diferentes dos de trabalho, esperando a
    /// escolha (GD-CMS-04).
    pub(crate) profile_ask: Option<String>,
    /// Salvamento automático desta execução (None nos testes).
    recovery: Option<crate::recovery::Session>,
    /// Cópias de uma execução que caiu, oferecidas ao abrir.
    pub(crate) recover_offer: Vec<crate::recovery::Orphan>,
    /// Registro de falha da execução anterior, ainda não mostrado.
    crash_log: Option<PathBuf>,
    next_autosave: f64,
    /// Área de transferência interna: objetos com a matriz completa (página).
    clipboard: Vec<graphdraw_core::model::Object>,
    /// Último formato exportado (exportar seleção usa o mesmo).
    last_export: ExportFormat,
    /// Última escolha de cada grupo da caixa de ferramentas.
    tool_group_last: std::collections::HashMap<&'static str, usize>,
    /// Gaveta aberta: grupo, variantes, posição e quadro em que abriu.
    tool_drawer: Option<(&'static str, Vec<crate::toolbox::Variant>, egui::Pos2, u64)>,
    bitmap_dlg: Option<BitmapDialog>,
    /// Exportar para corte (PLT/DXF) e gerar linha de corte (doc 17).
    cut_export: Option<crate::cutui::CutExport>,
    cut_line_dlg: Option<crate::cutui::CutLineDialog>,
    nest_dlg: Option<crate::cutui::NestDialog>,
    fonts_dlg: Option<crate::fontsui::MissingFontsDialog>,
    spell_dlg: Option<crate::spellui::SpellDialog>,
    print_dlg: Option<crate::printui::PrintDialog>,
    /// Opções da última impressão por documento, nesta sessão (GD-PRT-05).
    print_settings: std::collections::HashMap<String, graphdraw_core::print::PrintSettings>,
    nest_opts: graphdraw_core::edit::nest::NestOptions,
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
            exporting: None,
            egui_ctx: egui_ctx.clone(),
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
            styles_panel: Default::default(),
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
            ask_default: None,
            profile_ask: None,
            recovery: None,
            recover_offer: Vec::new(),
            crash_log: None,
            next_autosave: 0.0,
            clipboard: Vec::new(),
            last_export: ExportFormat::Pdf,
            tool_group_last: Default::default(),
            tool_drawer: None,
            bitmap_dlg: None,
            cut_export: None,
            cut_line_dlg: None,
            nest_dlg: None,
            fonts_dlg: None,
            spell_dlg: None,
            print_dlg: None,
            print_settings: Default::default(),
            nest_opts: Default::default(),
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
        if let Some(dir) = crate::recovery::dir() {
            app.recovery = crate::recovery::Session::start(&dir);
            let own = app
                .recovery
                .as_ref()
                .map(|s| s.tag().to_string())
                .unwrap_or_default();
            app.recover_offer = crate::recovery::orphans(&dir, &own);
        }
        app.crash_log = crate::recovery::take_crash_log();
        if let Some(p) = initial {
            app.start_open(p, egui_ctx);
        }
        app
    }

    /// Salvamento automático: a cada `recovery::INTERVAL_S`, copia os
    /// documentos alterados para a pasta de recuperação; os salvos ou
    /// fechados saem de lá.
    fn autosave(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        if now < self.next_autosave {
            return;
        }
        self.next_autosave = now + crate::recovery::INTERVAL_S;
        ctx.request_repaint_after(std::time::Duration::from_secs_f64(
            crate::recovery::INTERVAL_S,
        ));
        let Some(rec) = &mut self.recovery else {
            return;
        };
        let mut open = Vec::new();
        for d in self
            .doc
            .iter()
            .chain(self.tabs.iter().filter_map(|t| t.doc.as_ref()))
        {
            open.push(d.key.clone());
            if !d.editor.is_modified() {
                rec.forget(&d.key);
                continue;
            }
            let rev = d.editor.revision();
            if rec.stale(&d.key, rev) {
                let title = d
                    .path
                    .as_ref()
                    .and_then(|p| p.file_name())
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| tr("Sem título", "Untitled").into());
                let meta = crate::recovery::Meta {
                    title,
                    original: d.path.clone(),
                    saved_at: crate::recovery::now_secs(),
                };
                rec.save(&d.key, rev, d.doc().clone(), meta);
            }
        }
        rec.sweep(&open);
    }

    /// Abre a cópia recuperada `i` numa aba (conta como alterada) e apaga
    /// a cópia.
    pub(crate) fn recover(&mut self, i: usize) {
        if i >= self.recover_offer.len() {
            return;
        }
        let o = self.recover_offer.remove(i);
        match graphdraw_core::gdr::read_file(&o.file) {
            Ok((doc, report)) => {
                let mut d = DocState::from_document(doc, o.meta.original.clone(), report);
                d.editor.mark_unsaved();
                self.open_in_tab(d);
                o.discard();
                self.status = format!("{} {}", tr("Recuperado:", "Recovered:"), o.meta.title);
            }
            Err(e) => {
                self.message = Some((
                    tr("Não foi possível recuperar", "Could not recover").into(),
                    format!("{}\n\n{e}", o.file.display()),
                ));
            }
        }
    }

    /// Janela do início depois de uma queda: documentos para recuperar e o
    /// registro da falha.
    fn recovery_window(&mut self, ctx: &egui::Context) {
        if self.recover_offer.is_empty() && self.crash_log.is_none() {
            return;
        }
        let mut pick: Option<(usize, bool)> = None;
        let (mut all, mut close) = (false, false);
        let now = crate::recovery::now_secs();
        crate::widgets::modal(ctx, egui::Id::unique("recuperar")).show(ctx, |ui| {
            ui.heading(tr("Recuperar trabalho", "Recover work"));
            ui.add_space(theme::SPACE_300);
            if let Some(p) = &self.crash_log {
                ui.label(tr(
                    "O GraphDraw fechou por causa de uma falha. O registro ficou em:",
                    "GraphDraw closed because of a failure. The log is in:",
                ));
                ui.monospace(p.display().to_string());
                ui.add_space(theme::SPACE_200);
            }
            if !self.recover_offer.is_empty() {
                ui.label(tr(
                    "Estes documentos tinham alterações não salvas:",
                    "These documents had unsaved changes:",
                ));
                for (i, o) in self.recover_offer.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.strong(&o.meta.title);
                        let mins = now.saturating_sub(o.meta.saved_at) / 60;
                        ui.weak(if mins < 60 {
                            format!("{} {mins} min", tr("há", "ago:"))
                        } else {
                            format!("{} {} h", tr("há", "ago:"), mins / 60)
                        });
                        if let Some(p) = &o.meta.original {
                            ui.weak(p.display().to_string());
                        }
                        if ui.button(tr("Recuperar", "Recover")).clicked() {
                            pick = Some((i, true));
                        }
                        if ui.button(tr("Descartar", "Discard")).clicked() {
                            pick = Some((i, false));
                        }
                    });
                }
            }
            ui.add_space(theme::SPACE_300);
            crate::widgets::footer(ui, |ui| {
                if ui
                    .add(crate::widgets::secondary_button(tr("Fechar", "Close")))
                    .clicked()
                {
                    close = true;
                }
                if !self.recover_offer.is_empty()
                    && ui
                        .add(crate::widgets::secondary_button(tr(
                            "Descartar todos",
                            "Discard all",
                        )))
                        .clicked()
                {
                    all = true;
                }
            });
        });
        match pick {
            Some((i, true)) => self.recover(i),
            Some((i, false)) => self.recover_offer.remove(i).discard(),
            None => {}
        }
        if all {
            for o in self.recover_offer.drain(..) {
                o.discard();
            }
        }
        if close {
            // O que não foi escolhido volta a ser oferecido no próximo início.
            self.recover_offer.clear();
            self.crash_log = None;
        }
        if self.recover_offer.is_empty() && pick.is_some() {
            self.crash_log = None;
        }
    }

    /// Política de perfis ao abrir (GD-CMS-04): manter, converter ou atribuir
    /// os de trabalho. `ask` deixa a pergunta para a janela.
    pub(crate) fn apply_profile_policy(&mut self, policy: &str) {
        let Some(work) = self.ws.working_color.clone() else {
            return;
        };
        let Some(d) = &mut self.doc else {
            return;
        };
        if !graphdraw_core::cms::profiles_differ(&d.doc().data.settings.color, &work) {
            return;
        }
        match policy {
            "keep" => {}
            "convert" => {
                let n = graphdraw_core::edit::palette::convert_colors(&mut d.editor, &work);
                self.status = format!(
                    "{n} {}",
                    tr(
                        "cor(es) convertida(s) para os perfis de trabalho",
                        "color(s) converted to the working profiles"
                    )
                );
            }
            "ignore" => d.editor.transaction(
                tr("Usar perfis de trabalho", "Use working profiles"),
                |ed| ed.edit_settings(|s| s.color = work),
            ),
            _ => self.profile_ask = Some(d.title()),
        }
    }

    /// Pergunta da política de perfis (GD-CMS-04).
    fn profile_window(&mut self, ctx: &egui::Context) {
        let Some(title) = self.profile_ask.clone() else {
            return;
        };
        let mut pick: Option<&str> = None;
        let mut remember = false;
        crate::widgets::modal(ctx, egui::Id::unique("perfis-diferentes")).show(ctx, |ui| {
            ui.heading(tr("Perfis de cor diferentes", "Different color profiles"));
            ui.add_space(theme::SPACE_300);
            ui.label(format!(
                "{title}: {}",
                tr(
                    "o arquivo usa perfis de cor diferentes dos seus perfis de trabalho.",
                    "the file uses color profiles different from your working profiles."
                )
            ));
            ui.checkbox(
                &mut remember,
                tr("Lembrar a escolha", "Remember the choice"),
            );
            ui.add_space(theme::SPACE_300);
            crate::widgets::footer(ui, |ui| {
                for (k, pt, en) in crate::colorui::POLICIES.iter().skip(1).rev() {
                    if ui
                        .add(crate::widgets::secondary_button(tr(pt, en)))
                        .clicked()
                    {
                        pick = Some(k);
                    }
                }
            });
        });
        if let Some(k) = pick {
            self.profile_ask = None;
            if remember {
                self.ws.color_policy = k.to_string();
            }
            self.apply_profile_policy(k);
        }
    }

    /// Janela Sobre: marca, versão, licença, terceiros e onde ficam as
    /// cópias de recuperação e os registros de falha (suporte).
    fn about_window(&mut self, ctx: &egui::Context) {
        let info = format!(
            "GraphDraw {} · {} {}",
            env!("CARGO_PKG_VERSION"),
            std::env::consts::OS,
            std::env::consts::ARCH
        );
        let folder = crate::recovery::config_dir()
            .map(|d| d.display().to_string())
            .unwrap_or_default();
        let mut close = false;
        crate::widgets::modal(ctx, egui::Id::unique("sobre")).show(ctx, |ui| {
            ui.set_width(theme::ABOUT_WIDTH);
            ui.horizontal(|ui| {
                ui.add(
                    egui::Image::new(egui::include_image!("../assets/brand/app-128.png"))
                        .fit_to_exact_size(egui::Vec2::splat(theme::ABOUT_LOGO)),
                );
                ui.vertical(|ui| {
                    ui.heading("GraphDraw");
                    ui.label(format!(
                        "{} {}",
                        tr("Versão", "Version"),
                        env!("CARGO_PKG_VERSION")
                    ));
                    ui.label(tr(
                        "Editor de ilustração vetorial para gráficas, comunicação visual e corte.",
                        "Vector illustration editor for print shops, signage and cutting.",
                    ));
                    ui.label(tr(
                        "Código aberto — licença MPL-2.0.",
                        "Open source — MPL-2.0 license.",
                    ));
                });
            });
            ui.separator();
            egui::CollapsingHeader::new(tr("Componentes de terceiros", "Third-party components"))
                .id_salt("sobre-terceiros")
                .show(ui, |ui| {
                    egui::ScrollArea::vertical()
                        .max_height(theme::ABOUT_LIST_HEIGHT)
                        .show(ui, |ui| {
                            ui.label(
                                egui::RichText::new(include_str!("../../../THIRD_PARTY.md"))
                                    .monospace()
                                    .size(theme::TYPE_MICRO),
                            );
                        });
                });
            ui.label(
                egui::RichText::new(format!(
                    "{} {folder}",
                    tr(
                        "Recuperação e registros de falha em:",
                        "Recovery and crash logs in:"
                    )
                ))
                .weak(),
            );
            ui.add_space(theme::SPACE_300);
            crate::widgets::footer(ui, |ui| {
                if ui
                    .add(crate::widgets::primary_button(tr("Fechar", "Close")))
                    .clicked()
                    || ui.input(|i| i.key_pressed(Key::Escape))
                {
                    close = true;
                }
                if ui
                    .add(crate::widgets::secondary_button(tr(
                        "Copiar informações",
                        "Copy information",
                    )))
                    .clicked()
                {
                    ui.ctx().copy_text(format!(
                        "{info}
{folder}"
                    ));
                    self.status = tr("Informações copiadas", "Information copied").into();
                }
            });
        });
        if close {
            self.show_about = false;
        }
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
        canvas.grid = self.canvas.grid;
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
        // Fontes ausentes (GD-FNT-02/03): trocar, curvas ou manter.
        self.fonts_dlg = crate::fontsui::MissingFontsDialog::for_doc(&doc);
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
                let policy = self.ws.color_policy.clone();
                self.apply_profile_policy(&policy);
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
        // Cópia do anterior (`.gdr.bak`): se algo der errado, o último
        // arquivo bom continua lá.
        if target.exists() {
            let _ = std::fs::copy(&target, target.with_extension("gdr.bak"));
        }
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
    /// Roda `job` em segundo plano (GD-UI-15): a barra de status mostra o
    /// disquete com "Exportando <nome>…" até terminar. Uma por vez.
    fn spawn_export(
        &mut self,
        out: &std::path::Path,
        job: impl FnOnce() -> ExportDone + Send + 'static,
    ) {
        let (tx, rx) = channel();
        let ctx = self.egui_ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(job());
            ctx.request_repaint();
        });
        let name = out
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        self.status.clear();
        self.exporting = Some(Exporting { name, rx });
    }

    /// Polígono selecionado na ferramenta Forma: um lado a menos (o mínimo
    /// é 3; 5 na estrela complexa). `false` = não era polígono.
    fn polygon_remove_side(&mut self) -> bool {
        let Some(d) = &mut self.doc else { return false };
        let [id] = d.selection.as_slice() else {
            return false;
        };
        let is_poly = d
            .editor
            .find(id)
            .and_then(|r| d.editor.get(&r))
            .is_some_and(|o| matches!(o.kind, ObjectKind::Polygon(_)));
        if !is_poly {
            return false;
        }
        let sel = d.selection.clone();
        ops::modify(&mut d.editor, &sel, tr("Tirar lado", "Remove side"), |o| {
            if let ObjectKind::Polygon(p) = &mut o.kind {
                let min = if p.mode == "complex" { 5 } else { 3 };
                if p.sides > min {
                    p.sides -= 1;
                    let count = if p.mode == "star" {
                        2 * p.sides
                    } else {
                        p.sides
                    };
                    p.free_nodes.retain(|f| f.index < count);
                    return true;
                }
            }
            false
        });
        true
    }

    /// Já há uma exportação rodando? Avisa e não começa outra.
    fn export_busy(&mut self) -> bool {
        if let Some(e) = &self.exporting {
            self.status = format!(
                "{} {}…",
                tr("Aguarde: exportando", "Please wait: exporting"),
                e.name
            );
            return true;
        }
        false
    }

    /// Exportação terminada: o indicador some e fica o resultado.
    fn poll_export(&mut self) {
        let Some(e) = &self.exporting else { return };
        let done = match e.rx.try_recv() {
            Ok(d) => d,
            Err(std::sync::mpsc::TryRecvError::Empty) => return,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => ExportDone {
                status: format!("{} {}", tr("Falha ao exportar:", "Export failed:"), e.name),
                message: None,
            },
        };
        self.exporting = None;
        self.status = done.status;
        if let Some(m) = done.message {
            self.message = Some(m);
        }
    }

    fn export_pdfx(&mut self) {
        self.pdf_standard = export::PdfStandard::X1a(self.print_condition);
        self.export_pages(ExportFormat::Pdf, Vec::new());
        self.pdf_standard = export::PdfStandard::Screen;
    }

    /// Exporta a página atual (formatos de uma página) ou o documento (PDF).
    /// Cola a área de transferência interna (na mesma posição ou com o
    /// centro em `at`, coordenadas da página), na camada atual.
    pub(crate) fn paste_at(&mut self, at: Option<kurbo::Point>) {
        let Some(d) = &mut self.doc else { return };
        if self.clipboard.is_empty() {
            self.status = tr("Nada para colar", "Nothing to paste").into();
            return;
        }
        let Some(layer) = d.layer_for_new_object() else {
            return;
        };
        let mut objs: Vec<graphdraw_core::model::Object> = self.clipboard.clone();
        for o in &mut objs {
            ops::renew_ids(o);
        }
        if let Some(p) = at {
            let b = objs
                .iter()
                .filter_map(|o| {
                    graphdraw_core::edit::query::bounds(o, kurbo::Affine::IDENTITY, false)
                })
                .reduce(|a, b| a.union(b));
            if let Some(b) = b {
                let m = kurbo::Affine::translate(p - b.center());
                for o in &mut objs {
                    o.transform =
                        graphdraw_core::model::Transform::from_affine(m * o.transform.affine());
                }
            }
        }
        let page = d.page;
        let ids: Vec<String> = objs.iter().map(|o| o.id.clone()).collect();
        d.editor.transaction(tr("Colar", "Paste"), |ed| {
            for o in objs {
                let index = ed.objects(page, layer).len();
                ed.insert_object(graphdraw_core::edit::ObjLoc { page, layer, index }, o);
            }
        });
        d.selection = ids;
    }

    /// Exportar seleção (GD-BDR-81): só os objetos selecionados, numa página
    /// do tamanho deles, no último formato usado (PNG com fundo
    /// transparente).
    fn export_selection(&mut self) {
        if self.export_busy() {
            return;
        }
        let Some(d) = &self.doc else { return };
        let Some(sel_doc) = ops::selection_document(&d.editor, &d.selection) else {
            return;
        };
        let format = self.last_export;
        let ext = format.extension();
        let Some(out) = rfd::FileDialog::new()
            .set_file_name(format!("{}.{ext}", tr("selecao", "selection")))
            .add_filter(ext.to_uppercase(), &[ext])
            .save_file()
        else {
            return;
        };
        let opts = ExportOptions {
            dpi: d.doc().data.settings.resolution,
            text_to_curves: self.text_curves,
            ..Default::default()
        };
        let path = out.clone();
        self.spawn_export(&out, move || ExportDone {
            status: match export::export_with_report(&sel_doc, &path, format, &opts) {
                Ok(_) => format!(
                    "{} {}",
                    tr("Seleção exportada para", "Selection exported to"),
                    path.display()
                ),
                Err(e) => format!("{} {e}", tr("Falha ao exportar:", "Export failed:")),
            },
            message: None,
        });
    }

    fn export(&mut self, format: ExportFormat) {
        self.last_export = format;
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
        if self.export_busy() {
            return;
        }
        // PNG: só a arte (GD-EXP-08) — a seleção (exportando a página
        // atual) ou os objetos visíveis de cada página, sem a página nem fundo.
        let png = match (&self.doc, format) {
            (Some(doc), ExportFormat::Png) => {
                let selection = pages.as_slice() == [doc.page] && !doc.selection.is_empty();
                let art = if selection {
                    ops::selection_document(&doc.editor, &doc.selection)
                } else {
                    ops::artwork_document(&doc.editor, &pages)
                };
                if art.is_none() {
                    self.status = tr(
                        "Nada para exportar: a página não tem objetos visíveis",
                        "Nothing to export: the page has no visible objects",
                    )
                    .into();
                    return;
                }
                art
            }
            _ => None,
        };
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
            // O documento da arte já tem só as páginas pedidas.
            pages: if png.is_some() { Vec::new() } else { pages },
            dpi: doc.doc().data.settings.resolution,
            pdf: if format == ExportFormat::Pdf {
                self.pdf_standard
            } else {
                export::PdfStandard::Screen
            },
            text_to_curves: self.text_curves,
            ..Default::default()
        };
        let document = png.unwrap_or_else(|| doc.doc().clone());
        let target = out.clone();
        self.spawn_export(&out, move || {
            let out = target;
            let mut done = ExportDone {
                status: String::new(),
                message: None,
            };
        match export::export_with_report(&document, &out, format, &opts) {
            Ok((files, warnings)) => {
                let path = files
                    .first()
                    .map(|f| f.display().to_string())
                    .unwrap_or_default();
                done.status = format!("{} {path}", tr("Exportado para", "Exported to"));
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
                    done.message = Some((
                        tr("PDF/X-1a exportado", "PDF/X-1a exported").into(),
                        format!("{path}\n\n{body}"),
                    ));
                }
            }
            Err(e) => {
                done.message = Some((
                    tr("Não foi possível exportar", "Could not export").into(),
                    format!("{}\n\n{e}", out.display()),
                ))
            }
        }
            done
        });
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

    /// Negrito e itálico do texto alvo (no ponto do cursor, na edição).
    pub(crate) fn text_flags(&self) -> (bool, bool) {
        let Some(d) = &self.doc else {
            return (false, false);
        };
        let Some(id) = self.text_targets().into_iter().next() else {
            return (false, false);
        };
        match d
            .editor
            .find(&id)
            .and_then(|r| d.editor.get(&r))
            .map(|o| &o.kind)
        {
            Some(ObjectKind::Text(t)) => {
                let st = match self.text_style_index() {
                    Some(i) => graphdraw_core::edit::text::style_at(t, i),
                    None => graphdraw_core::edit::text::current_style(t),
                };
                (st.2, st.3)
            }
            _ => (false, false),
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

    /// Submenu Grades (GD-GRD-01): documento (espaçamento ou frequência,
    /// linhas ou pontos), linha base, pixels e grade imprimível.
    fn grids_menu(&mut self, ui: &mut egui::Ui) {
        let c = &mut self.canvas;
        if ui
            .checkbox(&mut c.show_grid, tr("Grade de documento", "Document grid"))
            .changed()
            && c.show_grid
            && c.snap.grid.is_none()
        {
            c.snap.grid = Some(5_000.0);
        }
        ui.add_enabled_ui(c.show_grid, |ui| {
            let g = c.snap.grid.unwrap_or(5_000.0);
            ui.horizontal(|ui| {
                ui.label(tr("Espaçamento", "Spacing"));
                if let Some(v) = crate::widgets::length_field(ui, "grade-esp", g, None) {
                    c.snap.grid = Some(v.max(10.0));
                }
            });
            ui.horizontal(|ui| {
                ui.label(tr("Frequência", "Frequency"));
                let mut f = (10_000.0 / g * 100.0).round() / 100.0;
                if ui
                    .add(
                        egui::DragValue::new(&mut f)
                            .range(0.1..=100.0)
                            .speed(0.1)
                            .suffix(tr(" por cm", " per cm")),
                    )
                    .changed()
                {
                    c.snap.grid = Some((10_000.0 / f.max(0.1)).max(10.0));
                }
            });
            ui.checkbox(&mut c.grid.dots, tr("Como pontos", "As dots"));
        });
        ui.separator();
        let mut base = c.grid.baseline.is_some();
        if ui
            .checkbox(&mut base, tr("Grade de linha base", "Baseline grid"))
            .changed()
        {
            c.grid.baseline = base.then_some(12.0 * graphdraw_core::units::UM_PER_PT);
        }
        if let Some(b) = c.grid.baseline {
            ui.horizontal(|ui| {
                ui.label(tr("Entrelinha", "Leading"));
                if let Some(v) = crate::widgets::length_field(ui, "grade-base", b, None) {
                    c.grid.baseline = Some(v.max(100.0));
                }
            });
            ui.horizontal(|ui| {
                ui.label(tr("Início no topo", "Start from top"));
                if let Some(v) =
                    crate::widgets::length_field(ui, "grade-base-ini", c.grid.baseline_start, None)
                {
                    c.grid.baseline_start = v.max(0.0);
                }
            });
        }
        ui.checkbox(
            &mut c.grid.pixel,
            tr(
                "Grade de pixels (96 ppp, de perto)",
                "Pixel grid (96 ppi, up close)",
            ),
        );
        ui.separator();
        let spacing = c.snap.grid.filter(|_| c.show_grid);
        let baseline = c.grid.baseline.map(|b| (b, c.grid.baseline_start));
        if ui
            .add_enabled(
                spacing.is_some() || baseline.is_some(),
                egui::Button::new(tr("Criar grade imprimível", "Create printable grid")),
            )
            .on_hover_text(tr(
                "Põe as linhas das grades visíveis numa camada nova, que imprime e exporta",
                "Puts the visible grids' lines on a new layer that prints and exports",
            ))
            .clicked()
            && let Some(d) = &mut self.doc
        {
            let (pw, ph) = d.page_size();
            let page = d.page;
            graphdraw_core::edit::ops::printable_grid(
                &mut d.editor,
                page,
                pw,
                ph,
                spacing,
                baseline,
                tr("Grade", "Grid"),
            );
            ui.close();
        }
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
        let drafting = matches!(self.tool, Tool::Bezier | Tool::Polyline | Tool::BSpline);
        if pressed(Key::Delete) || pressed(Key::Backspace) && !drafting {
            // Ferramenta Forma num polígono: apagar um nó tira um lado.
            if !(self.tool == Tool::Shape && self.polygon_remove_side()) {
                self.delete();
            }
        }
        if pressed(Key::Enter)
            && drafting
            && let Some(d) = &mut self.doc
        {
            self.canvas.finish_bezier(d, &self.defaults, false);
        }
        if pressed(Key::Escape)
            && let Some(d) = &mut self.doc
        {
            if self.canvas.escape(d) {
                // Menu, arrasto direito, escolha de referência ou modo foco.
            } else if self.canvas.is_dragging() || self.canvas.has_draft() {
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
                | C::LiveWeld
                | C::LiveTrim
                | C::LiveIntersect
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
        // Um objeto só: alinha pela página (GD-ALI-06).
        let page = self.doc.as_ref().map_or(kurbo::Rect::ZERO, |d| {
            let (w, h) = d.page_size();
            kurbo::Rect::new(0.0, 0.0, w, h)
        });
        let align = |how| {
            move |ed: &mut graphdraw_core::edit::Editor, s: &[String]| {
                let to = ops::align_target(AlignTo::LastSelected, s.len(), page);
                ops::align(ed, s, how, to, false);
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
            C::Export => {
                self.export_choice = self.doc.is_some();
                // PDF/X: a condição de impressão acompanha o perfil embutido.
                if let Some(b) = self.doc.as_ref().and_then(|d| {
                    graphdraw_core::cms::builtin(
                        d.doc().data.settings.color.cmyk_profile.as_deref()?,
                    )
                }) {
                    self.print_condition = b.condition;
                }
            }
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
            C::Copy | C::Cut => {
                if let Some(d) = &mut self.doc {
                    let refs = ops::refs(&d.editor, &d.selection);
                    self.clipboard = refs
                        .iter()
                        .filter_map(|r| {
                            let mut o = d.editor.get(r)?.clone();
                            o.transform = graphdraw_core::model::Transform::from_affine(
                                d.editor.parent_affine(r) * o.transform.affine(),
                            );
                            Some(o)
                        })
                        .collect();
                    let n = self.clipboard.len();
                    crate::dialogs::set_tile_source(self.clipboard.clone());
                    // Texto na área de transferência do sistema: sem ela, o
                    // Ctrl+V seguinte não chega ao programa (área vazia).
                    ctx.copy_text(format!("GraphDraw: {n} {}", tr("objeto(s)", "object(s)")));
                    if c == C::Cut {
                        let sel = std::mem::take(&mut d.selection);
                        ops::delete(&mut d.editor, &sel);
                    }
                    self.status = format!("{n} {}", tr("objeto(s) copiado(s)", "object(s) copied"));
                }
            }
            C::Paste => self.paste_at(None),
            C::UngroupAll => self.with_selection(|ed, s| Some(ops::ungroup_all(ed, s))),
            C::LockSelection | C::UnlockSelection | C::HideSelection => {
                if let Some(d) = &mut self.doc {
                    let sel = d.selection.clone();
                    match c {
                        C::LockSelection => ops::set_locked(&mut d.editor, &sel, true),
                        C::UnlockSelection => ops::set_locked(&mut d.editor, &sel, false),
                        _ => {
                            ops::set_visible(&mut d.editor, &sel, false);
                            d.selection.clear();
                        }
                    }
                }
            }
            C::ShowAll | C::UnlockAll => {
                if let Some(d) = &mut self.doc {
                    let page = d.page;
                    let ids = if c == C::ShowAll {
                        ops::show_all(&mut d.editor, page)
                    } else {
                        ops::unlock_all(&mut d.editor, page)
                    };
                    self.status = format!("{} {}", ids.len(), tr("objeto(s)", "object(s)"));
                }
            }
            C::InFrontOf | C::BehindOf => {
                if self.doc.as_ref().is_some_and(|d| !d.selection.is_empty()) {
                    self.canvas.start_pick_reference(c == C::InFrontOf);
                    self.status = tr(
                        "Clique no objeto de referência (Esc cancela)",
                        "Click the reference object (Esc cancels)",
                    )
                    .into();
                }
            }
            C::FocusMode => {
                if let Some(d) = &mut self.doc {
                    self.canvas.toggle_focus(d);
                }
            }
            C::ExportSelection => self.export_selection(),
            C::EffectContour
            | C::EffectBlend
            | C::EffectDropShadow
            | C::EffectBlock
            | C::EffectEnvelope => {
                use graphdraw_core::model::Effect;
                let kind = match c {
                    C::EffectContour => "contour",
                    C::EffectBlend => "blend",
                    C::EffectDropShadow => "drop-shadow",
                    C::EffectBlock => "block",
                    _ => "envelope",
                };
                let effect = Effect::initial(kind).expect("tipo conhecido");
                if c == C::EffectBlend && self.doc.as_ref().is_some_and(|d| d.selection.len() != 2)
                {
                    self.status = tr(
                        "A mistura precisa de dois objetos selecionados",
                        "Blend needs two selected objects",
                    )
                    .into();
                } else {
                    self.with_selection(|ed, s| {
                        graphdraw_core::edit::effects::apply(ed, s, effect).map(|g| vec![g])
                    });
                }
            }
            C::Print => {
                if let Some(d) = &self.doc {
                    let last = self.print_settings.get(&d.key).cloned();
                    self.print_dlg = Some(crate::printui::PrintDialog::new(
                        d,
                        &self.ws.printer,
                        last,
                        self.pdf_standard,
                    ));
                }
            }
            C::SpellCheck => {
                if let Some(d) = &self.doc {
                    self.spell_dlg = Some(crate::spellui::SpellDialog::new(d));
                }
            }
            C::EditOutline => {
                if let Some(d) = &self.doc {
                    self.style_dlg = Some(crate::dialogs::StyleDialog::stroke(d, &self.defaults));
                }
            }
            C::EditFill | C::EditUniformFill => {
                if let Some(d) = &self.doc {
                    let mut dlg = crate::dialogs::StyleDialog::fill(d, &self.defaults);
                    if c == C::EditFill {
                        dlg.force_gradient();
                    } else {
                        dlg.force_solid();
                    }
                    self.style_dlg = Some(dlg);
                }
            }
            C::Symmetry => {
                if let Some(d) = &mut self.doc {
                    use graphdraw_core::edit::symmetry;
                    let (w, h) = d.page_size();
                    let center = symmetry::default_center(
                        ops::selection_bounds(&d.editor, &d.selection, false),
                        w,
                        h,
                    );
                    let sel = d.selection.clone();
                    let page = d.page;
                    if let Some(layer) = d.layer_for_new_object()
                        && let Some(gid) =
                            symmetry::create(&mut d.editor, page, layer, &sel, "mirror", 1, center)
                    {
                        d.symmetry = Some(gid.clone());
                        d.selection = vec![gid];
                    }
                }
            }
            C::ToGuides => {
                if let Some(d) = &mut self.doc {
                    let sel = std::mem::take(&mut d.selection);
                    let page = d.page;
                    if ops::to_guides(&mut d.editor, &sel, page).is_some() {
                        self.status = format!(
                            "{} {}",
                            sel.len(),
                            tr("objeto(s) viraram guias", "object(s) became guides")
                        );
                    }
                }
            }
            C::Preflight => {
                self.right_tab = RightTab::Preflight;
                self.show_right = true;
            }
            C::ExtendToBleed => {
                if let Some(d) = &mut self.doc {
                    let sel = d.selection.clone();
                    let page = d.page;
                    let done =
                        graphdraw_core::preflight::extend_to_bleed(&mut d.editor, page, &sel);
                    self.status = if done.is_empty() {
                        tr(
                            "Nada encostado no corte (ou a página não tem sangria)",
                            "Nothing touching the trim (or the page has no bleed)",
                        )
                        .into()
                    } else {
                        format!(
                            "{} {}",
                            done.len(),
                            tr("objeto(s) até a sangria", "object(s) to the bleed")
                        )
                    };
                }
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
            C::TextBold | C::TextItalic => {
                // Liga/desliga pelo estado do texto (ou do trecho em edição).
                let (bold, italic) = self.text_flags();
                let st = if c == C::TextBold {
                    graphdraw_core::edit::text::TextStyle {
                        bold: Some(!bold),
                        ..Default::default()
                    }
                } else {
                    graphdraw_core::edit::text::TextStyle {
                        italic: Some(!italic),
                        ..Default::default()
                    }
                };
                self.text_style(st);
            }
            C::TextToCurves => {
                let ids = self.text_targets();
                if let Some(d) = &mut self.doc {
                    self.canvas.end_text(d);
                    ops::convert_to_curves(&mut d.editor, &ids);
                }
            }
            C::FitTextToPath => {
                // GD-TXT-09: um texto e um objeto com contorno selecionados.
                if let Some(d) = &mut self.doc {
                    self.canvas.end_text(d);
                    let is_text = |d: &crate::doc::DocState, id: &str| {
                        d.editor
                            .find(id)
                            .and_then(|r| d.editor.get(&r))
                            .is_some_and(|o| matches!(o.kind, ObjectKind::Text(_)))
                    };
                    let sel = d.selection.clone();
                    let text = sel.iter().find(|id| is_text(d, id)).cloned();
                    let path = sel.iter().find(|id| !is_text(d, id)).cloned();
                    let ok = match (text, path) {
                        (Some(t), Some(p)) if sel.len() == 2 => {
                            let ok = ops::fit_text_to_path(&mut d.editor, &t, &p);
                            if ok {
                                d.selection = vec![t];
                            }
                            ok
                        }
                        _ => false,
                    };
                    if !ok {
                        self.status = tr(
                            "Selecione um texto e um objeto com contorno",
                            "Select a text and an object with an outline",
                        )
                        .into();
                    }
                }
            }
            C::ReattachTextPath => {
                if let Some(d) = &mut self.doc {
                    self.canvas.end_text(d);
                    let ids = d.selection.clone();
                    for id in ids {
                        graphdraw_core::edit::textpath::reattach(&mut d.editor, &id);
                    }
                }
            }
            C::DetachTextPath => {
                let ids = self.text_targets();
                if let Some(d) = &mut self.doc {
                    ops::detach_text_path(&mut d.editor, &ids);
                }
            }
            C::ConvertTextKind => {
                // GD-TXT-03: cada texto troca de tipo mantendo as linhas.
                if let Some(d) = &mut self.doc {
                    self.canvas.end_text(d);
                }
                let ids = self.text_targets();
                if let Some(d) = &mut self.doc {
                    ops::modify(
                        &mut d.editor,
                        &ids,
                        tr("Converter tipo de texto", "Convert text kind"),
                        graphdraw_core::edit::text::convert_kind,
                    );
                }
            }
            C::OutlineToObject => self.with_selection(|ed, s| {
                let n = ops::outline_to_object(ed, s);
                (!n.is_empty()).then_some(n)
            }),
            C::Group_ => self.group(),
            C::LiveWeld | C::LiveTrim | C::LiveIntersect => {
                let op = match c {
                    C::LiveWeld => "weld",
                    C::LiveTrim => "trim",
                    _ => "intersect",
                };
                self.with_selection(|ed, s| {
                    graphdraw_core::edit::shapeops::live_boolean(ed, s, op).map(|g| vec![g])
                });
            }
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
            C::BreakApart => {
                let any = self.doc.as_ref().is_some_and(|d| !d.selection.is_empty());
                let mut done = false;
                self.with_selection(|ed, s| {
                    let r = graphdraw_core::edit::breakapart::break_apart(ed, s);
                    done = r.is_some();
                    r
                });
                if any && !done {
                    self.status = tr(
                        "Nada a separar: a seleção já está no nível mais simples",
                        "Nothing to break apart: the selection is already at its simplest",
                    )
                    .into();
                }
            }
            C::Nest => {
                if self.doc.as_ref().is_some_and(|d| !d.selection.is_empty()) {
                    self.nest_dlg = Some(crate::cutui::NestDialog {
                        opts: self.nest_opts.clone(),
                    });
                }
            }
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
            C::ToolSolidFill => self.set_tool(Tool::SolidFill),
            C::ToolEyedropper => self.set_tool(Tool::Eyedropper),
            C::ToolKnife => self.set_tool(Tool::Knife),
            C::ToolConnector => self.set_tool(Tool::Connector),
            C::ToolWidth => self.set_tool(Tool::Width),
            C::ToolSmartFill => self.set_tool(Tool::SmartFill),
            C::ToolDimension => self.set_tool(Tool::Dimension),
            C::ToolFreeTransform => self.set_tool(Tool::FreeTransform),
            C::ToolFreehand => self.set_tool(Tool::Freehand),
            C::ToolPolyline => self.set_tool(Tool::Polyline),
            C::ToolSmartDraw => self.set_tool(Tool::SmartDraw),
            C::ToolFxContour => self.set_tool(Tool::FxContour),
            C::ToolFxBlend => self.set_tool(Tool::FxBlend),
            C::ToolFxShadow => self.set_tool(Tool::FxShadow),
            C::ToolFxBlock => self.set_tool(Tool::FxBlock),
            C::ToolFxEnvelope => self.set_tool(Tool::FxEnvelope),
            C::ToolBSpline => self.set_tool(Tool::BSpline),
            C::ToolLine2 => self.set_tool(Tool::Line2),
            C::ToolCurve3 => self.set_tool(Tool::Curve3),
            C::ToolCrop => self.set_tool(Tool::Crop),
            C::ToolEraser => self.set_tool(Tool::Eraser),
            C::ToolVirtualDelete => self.set_tool(Tool::VirtualDelete),
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
                    .add(crate::keymap::item(ui, tr("Novo", "New"),
                        crate::keymap::Command::New,))
                    .clicked()
                {
                    self.request(Pending::New, &ctx);
                }
                if ui
                    .add(crate::keymap::item(ui, tr("Abrir…", "Open…"),
                        crate::keymap::Command::Open,))
                    .clicked()
                {
                    self.request(Pending::Open(None), &ctx);
                }
                ui.separator();
                if ui
                    .add_enabled(
                        has,
                        crate::keymap::item(ui, tr("Salvar", "Save"),
                            crate::keymap::Command::Save,),
                    )
                    .clicked()
                {
                    self.save(false);
                }
                if ui
                    .add_enabled(
                        has,
                        crate::keymap::item(ui, tr("Salvar como…", "Save as…"),
                            crate::keymap::Command::SaveAs,),
                    )
                    .clicked()
                {
                    self.save(true);
                }
                if let Some(d) = &mut self.doc {
                    let mut embed = d.doc().data.settings.embed_fonts;
                    if ui
                        .checkbox(&mut embed, tr("Incorporar fontes ao salvar", "Embed fonts when saving"))
                        .on_hover_text(tr(
                            "Leva as fontes inteiras dentro deste arquivo (só as que a licença permite), para abrir igual em outra máquina; o arquivo fica maior",
                            "Carries the whole fonts inside this file (only those whose license allows it), to open the same on another machine; the file gets bigger",
                        ))
                        .changed()
                    {
                        d.editor.transaction(tr("Incorporar fontes", "Embed fonts"), |ed| {
                            ed.edit_settings(|s| s.embed_fonts = embed)
                        });
                    }
                }
                ui.separator();
                if ui
                    .add_enabled(
                        has,
                        crate::keymap::item(ui, tr("Importar…", "Import…"),
                            Command::Import,),
                    )
                    .clicked()
                {
                    self.run_command(Command::Import, &ctx);
                }
                if ui
                    .add_enabled(
                        has,
                        crate::keymap::item(ui, tr("Exportar…", "Export…"),
                            Command::Export,),
                    )
                    .clicked()
                {
                    self.run_command(Command::Export, &ctx);
                }
                if ui
                    .add_enabled(
                        has,
                        crate::keymap::item(ui, tr("Imprimir…", "Print…"), Command::Print),
                    )
                    .clicked()
                {
                    self.run_command(Command::Print, &ctx);
                }
                let has_sel_file = self.doc.as_ref().is_some_and(|d| !d.selection.is_empty());
                if ui
                    .add_enabled(
                        has_sel_file,
                        crate::keymap::item(ui, tr("Exportar seleção…", "Export selection…"),
                            Command::ExportSelection,),
                    )
                    .clicked()
                {
                    self.run_command(Command::ExportSelection, &ctx);
                }
                if ui
                    .add_enabled(
                        has,
                        crate::keymap::icon_item(ui, tr("Gerenciamento de cor…", "Color management…"), "palette"),
                    )
                    .clicked()
                    && let Some(d) = &self.doc
                {
                    let mut dlg = crate::colorui::ColorDialog::new(&d.doc().data.settings.color);
                    dlg.policy = self.ws.color_policy.clone();
                    self.color_dlg = Some(dlg);
                    ui.close();
                }
                if ui
                    .add_enabled(
                        has,
                        crate::keymap::icon_item(ui, tr("Exportar página como SVG…", "Export page as SVG…"), "file-export"),
                    )
                    .clicked()
                {
                    self.export(ExportFormat::Svg);
                }
                if ui
                    .add_enabled(
                        has,
                        crate::keymap::icon_item(ui, tr(
                            "Exportar documento como PDF (tela)…",
                            "Export document as PDF (screen)…",
                        ), "file-export"),
                    )
                    .clicked()
                {
                    self.export(ExportFormat::Pdf);
                }
                if ui
                    .add_enabled(
                        has,
                        crate::keymap::icon_item(ui, tr(
                            "Exportar PDF/X-1a para gráfica…",
                            "Export PDF/X-1a for print…",
                        ), "file-export"),
                    )
                    .clicked()
                {
                    self.export_pdfx();
                }
                if ui
                    .add_enabled(
                        has,
                        crate::keymap::icon_item(ui, tr("Exportar página como PNG…", "Export page as PNG…"), "file-export"),
                    )
                    .clicked()
                {
                    self.export(ExportFormat::Png);
                }
                ui.separator();
                if ui
                    .add_enabled(
                        has,
                        crate::keymap::item(ui, tr("Fechar", "Close"),
                            crate::keymap::Command::Close,),
                    )
                    .clicked()
                {
                    let i = self.active;
                    self.request(Pending::Close(i), &ctx);
                }
                if ui.add(crate::keymap::icon_item(ui, tr("Sair", "Quit"), "doc-close")).clicked() {
                    self.request(Pending::Quit, &ctx);
                }
            });
            ui.menu_button(tr("Editar", "Edit"), |ui| {
                theme::menu_density(ui);
                if ui
                    .add_enabled(
                        can_undo,
                        crate::keymap::item(ui, tr("Desfazer", "Undo"),
                            crate::keymap::Command::Undo,),
                    )
                    .clicked()
                {
                    self.undo();
                }
                if ui
                    .add_enabled(
                        can_redo,
                        crate::keymap::item(ui, tr("Refazer", "Redo"),
                            crate::keymap::Command::Redo,),
                    )
                    .clicked()
                {
                    self.redo();
                }
                ui.separator();
                let has_sel_edit = self.doc.as_ref().is_some_and(|d| !d.selection.is_empty());
                let can_paste = !self.clipboard.is_empty();
                for (label, cmd, on) in [
                    (tr("Recortar", "Cut"), Command::Cut, has_sel_edit),
                    (tr("Copiar", "Copy"), Command::Copy, has_sel_edit),
                    (tr("Colar", "Paste"), Command::Paste, can_paste),
                ] {
                    if ui
                        .add_enabled(on, crate::keymap::item(ui, label, cmd))
                        .clicked()
                    {
                        self.run_command(cmd, &ctx);
                    }
                }
                ui.separator();
                if ui
                    .add_enabled(
                        has_sel,
                        crate::keymap::item(ui, tr("Duplicar", "Duplicate"),
                            crate::keymap::Command::Duplicate,),
                    )
                    .clicked()
                {
                    let off = self.nudge.1 / 2.0;
                    self.with_selection(|ed, s| Some(ops::duplicate(ed, s, Vec2::new(off, -off))));
                }
                if ui
                    .add_enabled(
                        has_sel,
                        crate::keymap::icon_item(ui, tr("Excluir  (Del)", "Delete  (Del)"), "delete"),
                    )
                    .clicked()
                {
                    self.delete();
                }
                if ui
                    .add_enabled(
                        has,
                        crate::keymap::item(ui, tr("Selecionar tudo", "Select all"),
                            crate::keymap::Command::SelectAll,),
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
                        .add_enabled(enabled, crate::keymap::item(ui, label, cmd))
                        .clicked()
                    {
                        self.run_command(cmd, &ctx);
                    }
                }
                ui.separator();
                if ui
                    .add_enabled(
                        has_sel,
                        crate::keymap::item(ui, tr("Copiar no mesmo lugar", "Copy in place"),
                            Command::PasteInPlace,),
                    )
                    .clicked()
                {
                    self.run_command(Command::PasteInPlace, &ctx);
                }
                if ui
                    .add_enabled(
                        has,
                        crate::keymap::item(ui, tr("Repetir a última ação", "Repeat last action"),
                            Command::Repeat,),
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
                        crate::keymap::item(ui, tr("Converter em curvas", "Convert to curves"),
                            crate::keymap::Command::ToCurves,),
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
                        crate::keymap::item(ui, tr("Converter contorno em objeto", "Convert outline to object"),
                            Command::OutlineToObject,),
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
                    (tr("Soldar ao vivo", "Live weld"), Command::LiveWeld, multi),
                    (tr("Aparar ao vivo", "Live trim"), Command::LiveTrim, multi),
                    (tr("Interseção ao vivo", "Live intersect"), Command::LiveIntersect, multi),
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
                        tr("Aninhar na chapa…", "Nest on sheet…"),
                        Command::Nest,
                        has_sel,
                    ),
                    (
                        tr("Marcas de registro", "Registration marks"),
                        Command::RegistrationMarks,
                        self.doc.is_some(),
                    ),
                ] {
                    if ui
                        .add_enabled(enabled, crate::keymap::item(ui, label, cmd))
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
                        crate::keymap::item(ui, tr("Centralizar na página", "Center on page"),
                            Command::CenterOnPage,),
                    )
                    .clicked()
                {
                    self.run_command(Command::CenterOnPage, &ctx);
                }
                ui.separator();
                if ui
                    .add_enabled(
                        has_sel,
                        crate::keymap::item(ui, tr("Agrupar", "Group"),
                            crate::keymap::Command::Group_,),
                    )
                    .clicked()
                {
                    self.group();
                }
                if ui
                    .add_enabled(
                        has_sel,
                        crate::keymap::item(ui, tr("Desagrupar", "Ungroup"),
                            crate::keymap::Command::Ungroup,),
                    )
                    .clicked()
                {
                    self.with_selection(|ed, s| Some(ops::ungroup(ed, s)));
                }
                for (label, cmd, on) in [
                    (tr("Desagrupar tudo", "Ungroup all"), Command::UngroupAll, has_sel),
                    (tr("À frente de…", "In front of…"), Command::InFrontOf, has_sel),
                    (tr("Atrás de…", "Behind…"), Command::BehindOf, has_sel),
                    (tr("Bloquear", "Lock"), Command::LockSelection, has_sel),
                    (tr("Desbloquear", "Unlock"), Command::UnlockSelection, has_sel),
                    (tr("Ocultar", "Hide"), Command::HideSelection, has_sel),
                    (tr("Mostrar todos", "Show all"), Command::ShowAll, true),
                    (tr("Desbloquear todos", "Unlock all"), Command::UnlockAll, true),
                    (tr("Estender até a sangria", "Extend to bleed"), Command::ExtendToBleed, has_sel),
                    (tr("Converter em guia", "Convert to guide"), Command::ToGuides, has_sel),
                    (tr("Simetria", "Symmetry"), Command::Symmetry, true),
                ] {
                    if ui
                        .add_enabled(on, crate::keymap::item(ui, label, cmd))
                        .clicked()
                    {
                        self.run_command(cmd, &ctx);
                    }
                }
                if ui
                    .add_enabled(
                        has_sel,
                        crate::keymap::item(ui, tr("Limpar transformações", "Clear transformations"),
                            crate::keymap::Command::ClearTransformations,),
                    )
                    .clicked()
                {
                    self.run_command(Command::ClearTransformations, &ctx);
                }
                if ui
                    .add_enabled(
                        has_sel,
                        crate::keymap::item(ui, tr("Converter em bitmap…", "Convert to bitmap…"),
                            crate::keymap::Command::ConvertToBitmap,),
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
                    let icon = match order {
                        Order::ToFront => "order-front",
                        Order::Forward => "order-forward",
                        Order::Backward => "order-backward",
                        Order::ToBack => "order-back",
                        _ => "path-reverse",
                    };
                    if ui.add_enabled(has_sel, crate::keymap::icon_item(ui, label, icon)).clicked() {
                        self.with_selection(|ed, s| {
                            ops::reorder(ed, s, order);
                            None
                        });
                    }
                }
            });
            // Menu Texto: tudo o que é de texto num lugar só.
            ui.menu_button(tr("Efeitos", "Effects"), |ui| {
                theme::menu_density(ui);
                let n = self.doc.as_ref().map_or(0, |d| d.selection.len());
                for (label, cmd, on) in [
                    (tr("Contorno", "Contour"), Command::EffectContour, n >= 1),
                    (tr("Mistura (dois objetos)", "Blend (two objects)"), Command::EffectBlend, n == 2),
                    (tr("Sombra projetada", "Drop shadow"), Command::EffectDropShadow, n >= 1),
                    (tr("Sombra de bloco", "Block shadow"), Command::EffectBlock, n >= 1),
                    (tr("Envelope", "Envelope"), Command::EffectEnvelope, n >= 1),
                ] {
                    if ui.add_enabled(on, crate::keymap::item(ui, label, cmd)).clicked() {
                        self.run_command(cmd, &ctx);
                        ui.close();
                    }
                }
                ui.separator();
                ui.weak(tr("Separar o efeito: Desagrupar (congela o resultado)", "Separate the effect: Ungroup (freezes the result)"));
            });
            ui.menu_button(tr("Texto", "Text"), |ui| {
                theme::menu_density(ui);
                let has_text = !self.text_targets().is_empty();
                let two = self.doc.as_ref().is_some_and(|d| d.selection.len() == 2);
                let item = |ui: &mut egui::Ui, label: &str, cmd: Command, on: bool| {
                    ui.add_enabled(on, crate::keymap::item(ui, label, cmd)).clicked()
                };
                let mut run = None;
                if item(ui, tr("Verificar ortografia…", "Check spelling…"), Command::SpellCheck, self.doc.is_some()) {
                    run = Some(Command::SpellCheck);
                }
                if item(ui, tr("Ferramenta Texto", "Text tool"), Command::ToolText, true) {
                    run = Some(Command::ToolText);
                }
                ui.separator();
                for (label, cmd) in [
                    (tr("Negrito", "Bold"), Command::TextBold),
                    (tr("Itálico", "Italic"), Command::TextItalic),
                    (tr("Aumentar a fonte", "Increase font size"), Command::TextBigger),
                    (tr("Diminuir a fonte", "Decrease font size"), Command::TextSmaller),
                ] {
                    if item(ui, label, cmd, has_text) {
                        run = Some(cmd);
                    }
                }
                ui.separator();
                for (label, cmd) in [
                    (tr("Alinhar à esquerda", "Align left"), Command::TextAlignLeft),
                    (tr("Centralizar", "Center"), Command::TextAlignCenter),
                    (tr("Alinhar à direita", "Align right"), Command::TextAlignRight),
                    (tr("Justificar", "Justify"), Command::TextJustify),
                ] {
                    if item(ui, label, cmd, has_text) {
                        run = Some(cmd);
                    }
                }
                ui.separator();
                for (label, cmd, on) in [
                    (
                        tr("Converter artístico ↔ parágrafo", "Convert artistic ↔ paragraph"),
                        Command::ConvertTextKind,
                        has_text,
                    ),
                    (tr("Ajustar texto ao caminho", "Fit text to path"), Command::FitTextToPath, two),
                    (tr("Separar texto do caminho", "Detach text from path"), Command::DetachTextPath, has_text),
                    (tr("Converter texto em curvas", "Convert text to curves"), Command::TextToCurves, has_text),
                ] {
                    if item(ui, label, cmd, on) {
                        run = Some(cmd);
                    }
                }
                if let Some(cmd) = run {
                    self.run_command(cmd, &ctx);
                }
            });
            ui.menu_button(tr("Exibir", "View"), |ui| {
                theme::menu_density(ui);
                if ui
                    .add(crate::keymap::item(ui, tr("Ajustar à página", "Fit page"),
                        crate::keymap::Command::ZoomPage,))
                    .clicked()
                {
                    self.canvas.fit_pending = true;
                }
                if ui
                    .add(crate::keymap::item(ui, tr("Tamanho real", "Actual size"),
                        crate::keymap::Command::ActualSize,))
                    .clicked()
                {
                    self.canvas.actual_pending = true;
                }
                if ui
                    .add_enabled(
                        has_sel,
                        crate::keymap::icon_item(ui, tr("Zoom na seleção", "Zoom to selection"), "fit-selection"),
                    )
                    .clicked()
                {
                    self.zoom_selection();
                }
                if ui.add(crate::keymap::icon_item(ui, tr("Zoom em tudo", "Zoom to all"), "fit-art")).clicked() {
                    self.zoom_all();
                }
                ui.separator();
                ui.menu_button(tr("Grades", "Grids"), |ui| self.grids_menu(ui));
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
                let focus_label = if self.canvas.focus.is_some() {
                    tr("Sair do modo foco", "Leave focus mode")
                } else {
                    tr("Modo foco (só a seleção)", "Focus mode (selection only)")
                };
                if ui
                    .add(crate::keymap::item(ui, focus_label, Command::FocusMode))
                    .clicked()
                {
                    self.run_command(Command::FocusMode, &ctx);
                }
                if ui
                    .add(crate::keymap::item(ui, tr("Verificação prévia", "Preflight"),
                        Command::Preflight,))
                    .clicked()
                {
                    self.run_command(Command::Preflight, &ctx);
                }
                // Prévia de separações (GD-SEP-01).
                if let Some(d) = &self.doc {
                    use graphdraw_core::render::separation::{SepView, plates};
                    let all = plates(d.doc());
                    ui.menu_button(tr("Prévia de separações", "Separations preview"), |ui| {
                        let mut pick = |ui: &mut egui::Ui, v: Option<SepView>, label: String| {
                            if ui.selectable_label(self.canvas.separation == v, label).clicked() {
                                self.canvas.separation = v;
                                ui.close();
                            }
                        };
                        pick(ui, None, tr("Desligada", "Off").into());
                        pick(
                            ui,
                            Some(SepView::Composite),
                            tr("Composta (sobreimpressão simulada)", "Composite (simulated overprint)").into(),
                        );
                        ui.separator();
                        for p in all {
                            let label = plate_label(&p);
                            pick(ui, Some(SepView::Plate(p)), label);
                        }
                    });
                }
                ui.checkbox(&mut self.canvas.show_rulers, tr("Réguas", "Rulers"));
                ui.checkbox(
                    &mut self.canvas.spread,
                    tr("Páginas vizinhas lado a lado", "Neighbor pages side by side"),
                );
                // Escala de desenho (GD-DOC-03): só réguas; a geometria não muda.
                if let Some(d) = &mut self.doc {
                    let [paper, world] = d.doc().data.settings.drawing_scale;
                    let mut n = world / paper.max(1e-9);
                    ui.horizontal(|ui| {
                        ui.label(tr("Escala de desenho  1 :", "Drawing scale  1 :"));
                        let r = ui
                            .add(egui::DragValue::new(&mut n).range(0.001..=1_000_000.0).speed(1.0))
                            .on_hover_text(tr(
                                "As réguas mostram medidas do mundo (ex.: 1 : 100); o desenho não muda",
                                "Rulers show real-world sizes (e.g. 1 : 100); the drawing does not change",
                            ));
                        // Arrastar o valor é um passo só de desfazer.
                        if r.drag_started() {
                            d.editor.begin(tr("Escala de desenho", "Drawing scale"));
                        }
                        if r.changed() {
                            d.editor.transaction(tr("Escala de desenho", "Drawing scale"), |ed| {
                                ed.edit_settings(|s| s.drawing_scale = [1.0, n])
                            });
                        }
                        if r.drag_stopped() {
                            d.editor.commit();
                        }
                    });
                    // GD-FIL-12: opção do documento.
                    let mut open = d.doc().data.settings.fill_open_paths;
                    if ui
                        .checkbox(&mut open, tr("Preencher caminhos abertos", "Fill open paths"))
                        .on_hover_text(tr(
                            "Desligado, linhas abertas mostram só o traço, mesmo com preenchimento",
                            "Off, open lines show only the outline, even with a fill",
                        ))
                        .changed()
                    {
                        d.editor.transaction(tr("Preencher caminhos abertos", "Fill open paths"), |ed| {
                            ed.edit_settings(|s| s.fill_open_paths = open)
                        });
                    }
                }
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
                        crate::keymap::icon_item(ui, tr("Remover guias da página", "Remove page guides"), "delete"),
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

    /// Escolhe uma variante de grupo da caixa de ferramentas.
    fn choose_variant(&mut self, group: &'static str, k: usize, v: crate::toolbox::Variant) {
        v.apply(&mut self.defaults);
        self.tool_group_last.insert(group, k);
        // Mesma ferramenta com outra família: troca mesmo assim.
        if self.tool == v.tool {
            self.tool = v.tool;
        } else {
            self.set_tool(v.tool);
        }
    }

    fn toolbox(&mut self, ui: &mut egui::Ui) {
        use crate::toolbox::{
            Slot, drawer_mark, hits_mark, separator_after, separator_after_group, slots,
        };
        let mut open: Option<(&'static str, Vec<crate::toolbox::Variant>, egui::Pos2)> = None;
        let mut chosen = None;
        let mut page_rect = false;
        ui.vertical_centered(|ui| {
            for slot in slots() {
                match slot {
                    Slot::One(t) => {
                        let (icon, tip, _) = t.info();
                        if icons::button(ui, icon, tip, self.tool == t, true).clicked() {
                            self.set_tool(t);
                        }
                        if separator_after(t) {
                            ui.separator();
                        }
                    }
                    Slot::Group(name, vars) => {
                        let on = vars
                            .iter()
                            .position(|v| v.active(self.tool, &self.defaults));
                        let k = on.unwrap_or_else(|| *self.tool_group_last.get(name).unwrap_or(&0));
                        let v = vars[k.min(vars.len() - 1)];
                        let tip = format!(
                            "{} — {}{}",
                            v.tip,
                            tr(
                                "triângulo ou botão direito: mais ferramentas",
                                "triangle or right click: more tools"
                            ),
                            if v.tool == Tool::Rect {
                                tr(
                                    " · duplo clique: retângulo do tamanho da página",
                                    " · double click: page-size rectangle",
                                )
                            } else {
                                ""
                            }
                        );
                        let r = icons::button(
                            ui,
                            v.icon,
                            crate::keymap::intern_tip(tip),
                            on.is_some(),
                            true,
                        );
                        drawer_mark(ui, r.rect);
                        let at_mark = hits_mark(r.rect, r.interact_pointer_pos());
                        if r.secondary_clicked() || (r.clicked() && at_mark) {
                            open = Some((name, vars.clone(), r.rect.right_top()));
                        } else if r.clicked() {
                            chosen = Some((name, k, v));
                        }
                        // Duplo clique no Retângulo: um do tamanho da página.
                        if r.double_clicked() && !at_mark && v.tool == Tool::Rect {
                            page_rect = true;
                        }
                        if separator_after_group(name) {
                            ui.separator();
                        }
                    }
                }
            }
        });
        if let Some((name, k, v)) = chosen {
            self.choose_variant(name, k, v);
        }
        // Retângulo da página criado: segue na Seleção, com ele selecionado.
        if page_rect && let Some(d) = &mut self.doc {
            crate::canvas::add_page_rect(d, &self.defaults);
            self.set_tool(Tool::Select);
        }
        if let Some((name, vars, at)) = open {
            self.tool_drawer = Some((name, vars, at, ui.ctx().cumulative_frame_nr()));
        }
        self.tool_drawer_ui(ui.ctx());
    }

    /// Gaveta aberta: as ferramentas do grupo em linha, ao lado do botão.
    fn tool_drawer_ui(&mut self, ctx: &egui::Context) {
        let Some((name, vars, at, opened)) = self.tool_drawer.clone() else {
            return;
        };
        let mut pick = None;
        let area = egui::Area::new(egui::Id::unique("gaveta-ferramentas"))
            .order(egui::Order::Foreground)
            .fixed_pos(at + egui::vec2(theme::SPACE_100, 0.0))
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        for (k, v) in vars.iter().enumerate() {
                            if icons::button(
                                ui,
                                v.icon,
                                v.tip,
                                v.active(self.tool, &self.defaults),
                                true,
                            )
                            .clicked()
                            {
                                pick = Some((k, *v));
                            }
                        }
                    });
                });
            });
        let outside = ctx.input(|i| i.pointer.any_pressed())
            && !area.response.contains_pointer()
            && ctx.cumulative_frame_nr() > opened + 1;
        if let Some((k, v)) = pick {
            self.tool_drawer = None;
            self.choose_variant(name, k, v);
        } else if outside || ctx.input(|i| i.key_pressed(Key::Escape)) {
            self.tool_drawer = None;
        }
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            // Prévia de separações (GD-SEP-01): qual chapa; clicar desliga.
            if let Some(sv) = &self.canvas.separation {
                use graphdraw_core::render::separation::SepView;
                let which = match sv {
                    SepView::Composite => tr("composta", "composite").to_string(),
                    SepView::Plate(p) => plate_label(p),
                };
                if ui
                    .button(format!(
                        "◐ {} {which} — {}",
                        tr("Separação:", "Separation:"),
                        tr("sair", "leave")
                    ))
                    .clicked()
                {
                    self.canvas.separation = None;
                }
            }
            // Modo foco (GD-BDR-82): indicador; clicar sai.
            if self.canvas.focus.is_some()
                && ui
                    .button(tr("◉ Modo foco — sair", "◉ Focus mode — leave"))
                    .clicked()
                && let Some(d) = &mut self.doc
            {
                self.canvas.toggle_focus(d);
            }
            if self.loading.is_some() {
                ui.spinner();
            }
            // Exportando (GD-UI-15): disquete + nome; some ao terminar.
            if let Some(e) = &self.exporting {
                ui.add(
                    egui::Image::new(icons::get("file-save"))
                        .fit_to_exact_size(egui::Vec2::splat(theme::ICON_MICRO))
                        .tint(theme::palette(ui).fg_primary),
                );
                ui.label(format!("{} {}…", tr("Exportando", "Exporting"), e.name));
                ui.spinner();
                ui.separator();
            }
            // Verificação prévia (GD-PRE-01): total de problemas; clique abre o painel.
            if let Some(d) = &mut self.doc {
                let n = d.preflight().len();
                if n > 0
                    && ui
                        .add(egui::Button::new(
                            egui::RichText::new(format!(
                                "⚠ {n} {}",
                                tr("na verificação", "in preflight")
                            ))
                            .color(theme::ACCENT_WARNING),
                        ))
                        .on_hover_text(tr("Abrir o painel Verificação", "Open the Preflight panel"))
                        .clicked()
                {
                    self.right_tab = RightTab::Preflight;
                    self.show_right = true;
                }
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
        if let (Some(dlg), Some(doc)) = (&mut self.print_dlg, &self.doc)
            && dlg.show(ctx, doc)
        {
            self.print_settings
                .insert(doc.key.clone(), dlg.settings.clone());
            if let Some(p) = dlg.printer_name() {
                self.ws.printer = p.into();
            }
            self.print_dlg = None;
        }
        if let (Some(dlg), Some(doc)) = (&mut self.spell_dlg, &mut self.doc)
            && dlg.show(ctx, doc)
        {
            self.spell_dlg = None;
        }
        if let (Some(dlg), Some(doc)) = (&mut self.fonts_dlg, &mut self.doc)
            && dlg.show(ctx, doc)
        {
            self.fonts_dlg = None;
        }
        if let (Some(dlg), Some(doc)) = (&mut self.style_dlg, &self.doc) {
            dlg.set_palettes(|| self.palettes.all_lists(Some(doc)));
        }
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
            self.about_window(ctx);
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
        self.default_color_window(ctx);
        self.profile_window(ctx);
        self.recovery_window(ctx);
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
            if ok {
                self.ws.color_policy = dlg.policy.clone();
                if dlg.make_working {
                    self.ws.working_color = Some(cm.clone());
                }
            }
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
        if let Some(dlg) = &mut self.nest_dlg
            && let Some(ok) = dlg.show(ctx)
        {
            let opts = dlg.opts.clone();
            self.nest_dlg = None;
            self.nest_opts = opts.clone();
            if ok && let Some(doc) = &mut self.doc {
                let sel = doc.selection.clone();
                let page = doc.page;
                let r = graphdraw_core::edit::nest::nest(&mut doc.editor, page, &sel, &opts);
                self.status = format!(
                    "{} {} {} {} · {} {:.0}%{}",
                    r.placed,
                    tr("peças em", "pieces on"),
                    r.sheets,
                    tr("chapa(s)", "sheet(s)"),
                    tr("aproveitamento", "usage"),
                    r.usage * 100.0,
                    if r.unplaced.is_empty() {
                        String::new()
                    } else {
                        format!(
                            " · {} {}",
                            r.unplaced.len(),
                            tr("não cabem na chapa", "do not fit the sheet")
                        )
                    }
                );
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
        if self.export_busy() {
            return;
        }
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
        let (document, page, opts) = (doc.doc().clone(), doc.page, opts.clone());
        let path = out.clone();
        self.spawn_export(&out, move || {
            let strokes = cut::cut_strokes(&document, page, &opts);
            let text = match format {
                crate::cutui::CutFormat::Plt => cut::hpgl(&strokes, pw, &opts),
                crate::cutui::CutFormat::Dxf => cut::dxf(&strokes, &opts, pw),
            };
            ExportDone {
                status: match std::fs::write(&path, text) {
                    Ok(()) => format!(
                        "{} {}",
                        tr("Corte exportado:", "Cut exported:"),
                        path.display()
                    ),
                    Err(e) => format!("{} {e}", tr("Falha ao exportar:", "Export failed:")),
                },
                message: None,
            }
        });
    }

    /// Pergunta da cor sem seleção (GD-BDR-10): vira padrão dos novos
    /// objetos? Com "não perguntar de novo", passa a aplicar direto.
    fn default_color_window(&mut self, ctx: &egui::Context) {
        let Some(ask) = self.ask_default.clone() else {
            return;
        };
        let (mut yes, mut no) = (false, false);
        let mut never = false;
        crate::widgets::modal(ctx, egui::Id::unique("cor-padrao")).show(ctx, |ui| {
            ui.heading(tr("Nada selecionado", "Nothing selected"));
            ui.add_space(theme::SPACE_300);
            ui.label(match ask {
                DefaultAsk::Fill(_) => tr(
                    "Usar esta cor como preenchimento dos próximos objetos?",
                    "Use this color as the fill of the next objects?",
                ),
                DefaultAsk::Stroke(_) => tr(
                    "Usar esta cor como contorno dos próximos objetos?",
                    "Use this color as the outline of the next objects?",
                ),
            });
            ui.checkbox(&mut never, tr("Não perguntar de novo", "Don't ask again"));
            ui.add_space(theme::SPACE_300);
            crate::widgets::footer(ui, |ui| {
                if ui
                    .add(crate::widgets::primary_button(tr(
                        "Usar como padrão",
                        "Use as default",
                    )))
                    .clicked()
                    || ui.input(|i| i.key_pressed(Key::Enter))
                {
                    yes = true;
                }
                if ui
                    .add(crate::widgets::secondary_button(tr("Cancelar", "Cancel")))
                    .clicked()
                    || ui.input(|i| i.key_pressed(Key::Escape))
                {
                    no = true;
                }
            });
        });
        if yes {
            self.ask_default = None;
            if never {
                self.ws.default_color_no_ask = true;
            }
            match ask {
                DefaultAsk::Fill(f) => self.defaults.fill = f,
                DefaultAsk::Stroke(c) => {
                    self.defaults.stroke = c.map(|c| {
                        let mut s = self
                            .defaults
                            .stroke
                            .clone()
                            .unwrap_or_else(crate::doc::default_stroke);
                        s.color = c;
                        s
                    })
                }
            }
        } else if no {
            self.ask_default = None;
        }
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
        // Captura da nova combinação. O evento é consumido: não executa o
        // comando nem reaciona o botão focado (Enter/Espaço).
        if let Some(cmd) = self.key_capture {
            let got = ctx.input_mut(|i| {
                // Ctrl/Shift/Alt sozinhos: espera a tecla que vem junto.
                let pos = i.events.iter().position(|e| {
                    matches!(e, egui::Event::Key { pressed: true, key, .. } if !keymap::is_modifier(*key))
                        || Keymap::clipboard_chord(e).is_some()
                })?;
                match i.events.remove(pos) {
                    egui::Event::Key { key, modifiers, .. } => {
                        // Tira também o "soltar" e o texto digitado.
                        i.events.retain(|e| {
                            !matches!(e, egui::Event::Key { key: k, .. } if *k == key)
                                && !matches!(e, egui::Event::Text(_))
                        });
                        Some((key, modifiers))
                    }
                    e => Keymap::clipboard_chord(&e).map(|c| (c.key, egui::Modifiers::CTRL)),
                }
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
                    if ui
                        .button(tr("Restaurar padrão", "Restore defaults"))
                        .on_hover_text(tr(
                            "Volta todos os atalhos ao perfil GraphDraw de fábrica (desfaz as personalizações)",
                            "Resets every shortcut to the factory GraphDraw profile (discards customizations)",
                        ))
                        .clicked()
                    {
                        new_profile = Some(keymap::Profile::GraphDraw);
                        self.key_capture = None;
                        self.status =
                            tr("Atalhos restaurados ao padrão", "Shortcuts restored to defaults").into();
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
                                    ui.horizontal(|ui| {
                                        ui.add(crate::keymap::menu_icon(ui, c.icon()));
                                        ui.label(name);
                                    });
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

/// Nome da chapa na interface (GD-SEP-01).
pub(crate) fn plate_label(p: &graphdraw_core::render::separation::Plate) -> String {
    use graphdraw_core::render::separation::Plate;
    match p {
        Plate::Cyan => tr("Ciano", "Cyan").into(),
        Plate::Magenta => tr("Magenta", "Magenta").into(),
        Plate::Yellow => tr("Amarelo", "Yellow").into(),
        Plate::Black => tr("Preto", "Black").into(),
        Plate::Spot(n) => n.clone(),
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
        ObjectKind::Spiral(_) => tr("espiral", "spiral"),
        ObjectKind::Group(_) => tr("grupo", "group"),
        ObjectKind::Text(_) => tr("texto", "text"),
        ObjectKind::Image(_) => tr("imagem", "image"),
        ObjectKind::Dimension(_) => tr("cota", "dimension"),
        ObjectKind::Basic(_) => tr("forma básica", "basic shape"),
        ObjectKind::Connector(_) => tr("conector", "connector"),
        ObjectKind::BSpline(_) => tr("B-spline", "B-spline"),
        ObjectKind::Unknown { .. } => tr("objeto desconhecido", "unknown object"),
    }
    .into()
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.frame(ui);
    }

    /// Saída normal: as cópias de recuperação desta execução saem.
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if let Some(r) = self.recovery.take() {
            r.end();
        }
    }
}

impl App {
    /// Um quadro da interface inteira.
    pub(crate) fn frame(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        self.poll_loading();
        self.poll_export();
        self.autosave(&ctx);
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
        let mut pending_cmd = None;
        let mut pending_paste = None;
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
                self.canvas.can_paste = !self.clipboard.is_empty();
                self.canvas.three_point = self.defaults.three_point(self.tool);
                let ev = self.canvas.show(ui, doc, self.tool, &self.defaults);
                if let Some(t) = ev.switch_tool {
                    self.tool = t;
                }
                if let Some(c) = ev.command {
                    pending_cmd = Some(c);
                }
                if let Some(p) = ev.paste_at {
                    pending_paste = Some(p);
                }
                if ev.open_solid_fill {
                    let mut dlg = crate::dialogs::StyleDialog::fill(doc, &self.defaults);
                    dlg.force_solid();
                    self.style_dlg = Some(dlg);
                }
                if let Some(s) = ev.status {
                    self.status = s;
                }
                doc.prune_selection();
            }
        });
        // Menu do botão direito: o comando escolhido roda como no menu principal.
        if let Some(c) = pending_cmd {
            self.run_command(c, &ctx);
        }
        if let Some(p) = pending_paste {
            self.paste_at(Some(p));
        }
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
    /// Janela Sobre: abre, desenha e fecha com Esc.
    #[test]
    fn about_window_opens_and_closes() {
        let ctx = crate::theme::test_ctx();
        let mut app = App::with_ctx(&ctx, None);
        app.show_about = true;
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::new(1366.0, 768.0));
        for k in 0..3 {
            let mut input = egui::RawInput {
                screen_rect: Some(screen),
                ..Default::default()
            };
            if k == 2 {
                input.events.push(egui::Event::Key {
                    key: Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                });
            }
            let mut out = ctx.run_ui(input, |ui| app.about_window(ui.ctx()));
            out.textures_delta.clear();
        }
        assert!(!app.show_about, "Esc fecha");
    }

    /// GD-CMS-04: arquivo com perfis diferentes dos de trabalho — "ask"
    /// pergunta, "ignore" atribui os de trabalho, "keep" não mexe.
    #[test]
    fn gd_cms_04_profile_policy_on_open() {
        let ctx = crate::theme::test_ctx();
        let mut app = App::with_ctx(&ctx, None);
        let work = graphdraw_core::cms::ColorManagement {
            cmyk_profile: Some("C:/perfis/trabalho.icc".into()),
            ..Default::default()
        };
        app.ws.working_color = Some(work.clone());
        app.apply_profile_policy("keep");
        assert!(app.profile_ask.is_none());
        assert_ne!(app.doc.as_ref().unwrap().doc().data.settings.color, work);
        app.apply_profile_policy("ask");
        assert!(app.profile_ask.is_some(), "pergunta");
        app.profile_ask = None;
        app.apply_profile_policy("ignore");
        assert_eq!(app.doc.as_ref().unwrap().doc().data.settings.color, work);
        // Iguais: nada a perguntar.
        app.apply_profile_policy("ask");
        assert!(app.profile_ask.is_none());
    }

    /// Recuperar: a cópia de uma execução que caiu abre numa aba, alterada
    /// (fechar pergunta), e sai da pasta; salvar por cima deixa o `.bak`.
    #[test]
    fn recover_crashed_copy_and_backup_on_save() {
        let dir = std::env::temp_dir().join(format!("graphdraw-app-rec-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut doc = DocState::new_blank();
        let o = graphdraw_core::model::Object::new(ObjectKind::Rectangle(
            graphdraw_core::model::RectData {
                width: 10_000.0,
                height: 10_000.0,
                corners: None,
                extra: Default::default(),
            },
        ));
        ops::add_object(&mut doc.editor, 0, 0, o, "r");
        let mut crashed = crate::recovery::Session::start(&dir).unwrap();
        let meta = crate::recovery::Meta {
            title: "Cartaz".into(),
            original: None,
            saved_at: crate::recovery::now_secs(),
        };
        crashed
            .save(&doc.key, 1, doc.doc().clone(), meta)
            .join()
            .unwrap();
        drop(crashed);
        let ctx = crate::theme::test_ctx();
        let mut app = App::with_ctx(&ctx, None);
        app.recover_offer = crate::recovery::orphans(&dir, "outra");
        assert_eq!(app.recover_offer.len(), 1);
        app.recover(0);
        let d = app.doc.as_ref().unwrap();
        assert_eq!(d.page().object_count(), 1);
        assert!(d.editor.is_modified(), "recuperado conta como alterado");
        assert!(
            crate::recovery::orphans(&dir, "outra").is_empty(),
            "cópia apagada"
        );
        // Salvar por cima de um arquivo existente deixa o anterior em .gdr.bak.
        let path = dir.join("cartaz.gdr");
        std::fs::write(&path, b"anterior").unwrap();
        app.doc.as_mut().unwrap().path = Some(path.clone());
        assert!(app.save(false));
        assert_eq!(
            std::fs::read(path.with_extension("gdr.bak")).unwrap(),
            b"anterior"
        );
        assert!(!app.doc.as_ref().unwrap().editor.is_modified());
        let _ = std::fs::remove_dir_all(&dir);
    }

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

    /// GD-PRI-03: Delete na ferramenta Forma tira um lado do polígono.
    #[test]
    fn delete_on_polygon_removes_side() {
        use graphdraw_core::model::{Object, ObjectKind, PolygonData};
        let ctx = crate::theme::test_ctx();
        let mut app = App::with_ctx(&ctx, None);
        let d = app.doc.as_mut().unwrap();
        let o = Object::new(ObjectKind::Polygon(PolygonData {
            rx: 10_000.0,
            ry: 10_000.0,
            sides: 6,
            mode: "polygon".into(),
            sharpness: 0.5,
            bend: None,
            corner_outer: 0.0,
            corner_inner: 0.0,
            fill_crossings: false,
            free_nodes: Vec::new(),
            extra: Default::default(),
        }));
        let id = ops::add_object(&mut d.editor, 0, 0, o, "p");
        d.selection = vec![id.clone()];
        app.tool = Tool::Shape;
        assert!(app.polygon_remove_side());
        let d = app.doc.as_ref().unwrap();
        let ObjectKind::Polygon(p) = &d.editor.get(&d.editor.find(&id).unwrap()).unwrap().kind
        else {
            panic!()
        };
        assert_eq!(p.sides, 5, "o objeto continua, com um lado a menos");
    }

    #[test]
    #[ignore]
    fn shot_text_path_bar() {
        use graphdraw_core::model::{Color, EllipseData, Fill, Object, ObjectKind};
        let ctx = crate::theme::test_ctx();
        let mut shot = crate::uishot::Shot::default();
        let mut app = App::with_ctx(&ctx, None);
        let d = app.doc.as_mut().unwrap();
        let mut c = Object::new(ObjectKind::Ellipse(EllipseData {
            rx: 50_000.0,
            ry: 50_000.0,
            mode: "ellipse".into(),
            start_angle: 0.0,
            end_angle: 360.0,
            extra: Default::default(),
        }));
        c.transform = graphdraw_core::model::Transform::translate(105_000.0, 150_000.0);
        c.stroke = Some(graphdraw_core::model::Stroke::new(300.0, Color::black()));
        let cid = ops::add_object(&mut d.editor, 0, 0, c, "c");
        let mut t = graphdraw_core::edit::text::new_text(0.0, 0.0, None, "Arial", 9_000.0);
        if let ObjectKind::Text(td) = &mut t.kind {
            graphdraw_core::edit::text::insert(td, 0, "TEXTO NO CAMINHO");
        }
        t.fill = Some(Fill::solid(Color::cmyk(0.0, 0.8, 1.0, 0.0)));
        let tid = ops::add_object(&mut d.editor, 0, 0, t, "t");
        assert!(ops::fit_text_to_path(&mut d.editor, &tid, &cid));
        d.selection = vec![tid];
        app.tool = Tool::Select;
        for k in 0..20 {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::Vec2::new(1920.0, 900.0),
                )),
                time: Some(k as f64 / 30.0),
                ..Default::default()
            };
            let mut out = ctx.run_ui(input, |ui| app.frame(ui));
            if k == 19 {
                shot.save(&ctx, out, "texto-caminho-barra");
            } else {
                shot.textures(&mut out);
            }
        }
    }

    #[test]
    #[ignore]
    fn shot_effect_bars() {
        use graphdraw_core::model::{Color, Effect, Fill, Object, ObjectKind, RectData};
        let ctx = crate::theme::test_ctx();
        let mut shot = crate::uishot::Shot::default();
        for (name, tool, kind) in [
            ("efeito-bloco", Tool::FxBlock, "block"),
            ("efeito-contorno", Tool::FxContour, "contour"),
            ("efeito-sombra", Tool::FxShadow, "drop-shadow"),
            ("efeito-envelope", Tool::FxEnvelope, "envelope"),
        ] {
            let mut app = App::with_ctx(&ctx, None);
            let d = app.doc.as_mut().unwrap();
            let mut o = Object::new(ObjectKind::Rectangle(RectData {
                width: 60_000.0,
                height: 40_000.0,
                corners: None,
                extra: Default::default(),
            }));
            o.transform = graphdraw_core::model::Transform::translate(60_000.0, 150_000.0);
            o.fill = Some(Fill::solid(Color::cmyk(0.0, 0.7, 1.0, 0.0)));
            let id = ops::add_object(&mut d.editor, 0, 0, o, "r");
            let mut e = Effect::initial(kind).unwrap();
            if let Effect::Block {
                depth, end_color, ..
            } = &mut e
            {
                *depth = 20_000.0;
                *end_color = Some(Color::cmyk(0.0, 0.3, 0.6, 0.6));
            }
            let g = graphdraw_core::edit::effects::apply(&mut d.editor, &[id], e).unwrap();
            d.selection = vec![g];
            app.tool = tool;
            for k in 0..20 {
                let input = egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::Vec2::new(1366.0, 768.0),
                    )),
                    time: Some(k as f64 / 30.0),
                    ..Default::default()
                };
                let mut out = ctx.run_ui(input, |ui| app.frame(ui));
                if k == 19 {
                    shot.save(&ctx, out, name);
                } else {
                    shot.textures(&mut out);
                }
            }
        }
    }

    /// GD-UI-15: exportar roda em segundo plano; a barra de status mostra o
    /// disquete com o nome enquanto dura, recusa outra exportação e o
    /// indicador some ao terminar, ficando o resultado.
    #[test]
    fn gd_ui_13_export_runs_in_background_with_indicator() {
        let ctx = crate::theme::test_ctx();
        let mut app = App::with_ctx(&ctx, None);
        let dir = std::env::temp_dir().join(format!("gd-exporta-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let out = dir.join("cartaz.pdf");
        let (go_tx, go_rx) = std::sync::mpsc::channel::<()>();
        let doc = app.doc.as_ref().unwrap().doc().clone();
        let target = out.clone();
        app.spawn_export(&out, move || {
            go_rx.recv().unwrap();
            let r =
                export::export_with_report(&doc, &target, ExportFormat::Pdf, &Default::default());
            ExportDone {
                status: match r {
                    Ok(_) => format!("Exportado para {}", target.display()),
                    Err(e) => format!("Falha {e}"),
                },
                message: None,
            }
        });
        let frame = |app: &mut App| {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::Vec2::new(1366.0, 768.0),
                )),
                ..Default::default()
            };
            let mut out = ctx.run_ui(input, |ui| app.frame(ui));
            out.textures_delta.clear();
        };
        frame(&mut app);
        assert_eq!(
            app.exporting.as_ref().map(|e| e.name.as_str()),
            Some("cartaz.pdf")
        );
        assert!(app.export_busy(), "outra exportação espera");
        assert!(app.status.contains("cartaz.pdf"));
        go_tx.send(()).unwrap();
        for _ in 0..200 {
            frame(&mut app);
            if app.exporting.is_none() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(app.exporting.is_none(), "indicador some ao terminar");
        assert!(app.status.starts_with("Exportado para"), "{}", app.status);
        assert!(out.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// GD-PAL-05/07 de ponta a ponta: com um objeto selecionado, clicar numa
    /// amostra da barra pinta o objeto; segurar abre o mosaico e clicar num
    /// tom pinta com ele.
    #[test]
    fn palette_bar_click_and_mosaic_paint_selection() {
        use crate::paletteui::SWATCH_RECTS;
        let ctx = crate::theme::test_ctx();
        let mut app = App::with_ctx(&ctx, None);
        let id = {
            let d = app.doc.as_mut().unwrap();
            let o = graphdraw_core::model::Object::new(ObjectKind::Rectangle(
                graphdraw_core::model::RectData {
                    width: 10_000.0,
                    height: 10_000.0,
                    corners: None,
                    extra: Default::default(),
                },
            ));
            let id = ops::add_object(&mut d.editor, 0, 0, o, "r");
            d.selection = vec![id.clone()];
            id
        };
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::new(1366.0, 768.0));
        let mut t = 0.0;
        let mut frame = |app: &mut App, events: Vec<egui::Event>, dt: f64| {
            t += dt;
            SWATCH_RECTS.with(|v| v.borrow_mut().clear());
            let mut out = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(screen),
                    time: Some(t),
                    events,
                    ..Default::default()
                },
                |ui| app.frame(ui),
            );
            out.textures_delta.clear();
            SWATCH_RECTS.with(|v| v.borrow().clone())
        };
        let btn = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        let fill = |app: &App| {
            let d = app.doc.as_ref().unwrap();
            d.editor
                .get(&d.editor.find(&id).unwrap())
                .unwrap()
                .fill
                .clone()
        };
        let mut rects = Vec::new();
        for _ in 0..3 {
            rects = frame(&mut app, vec![], 0.05);
        }
        assert!(rects.len() > 3, "amostras da barra: {}", rects.len());
        // Clique simples na 3ª amostra da barra.
        let p = rects[2].center();
        frame(&mut app, vec![egui::Event::PointerMoved(p)], 0.05);
        frame(&mut app, vec![btn(p, true)], 0.05);
        frame(&mut app, vec![btn(p, false)], 0.05);
        let first = fill(&app);
        assert!(first.is_some(), "clique na amostra pinta o objeto");
        assert_eq!(app.doc.as_ref().unwrap().selection, vec![id.clone()]);
        // Segurar a 5ª amostra abre o mosaico; clicar num tom pinta.
        let p = rects[4].center();
        frame(&mut app, vec![egui::Event::PointerMoved(p)], 0.05);
        frame(&mut app, vec![btn(p, true)], 0.05);
        // Segura bem mais que o tempo de clique, com o mouse tremendo.
        for k in 0..15 {
            let j = egui::vec2((k % 3) as f32 - 1.0, 0.0);
            frame(&mut app, vec![egui::Event::PointerMoved(p + j)], 0.1);
        }
        frame(&mut app, vec![btn(p, false)], 0.05);
        assert_eq!(fill(&app), first, "soltar não aplica");
        let area = ctx
            .memory(|mem| mem.area_rect(egui::Id::unique("mosaico-tons-area")))
            .expect("mosaico aberto");
        let q = area.left_top() + egui::Vec2::splat(crate::theme::SWATCH);
        // Passa o mouse até a célula e espera a dica aparecer.
        for k in 0..10 {
            let m = p + (q - p) * (k as f32 / 9.0);
            frame(&mut app, vec![egui::Event::PointerMoved(m)], 0.15);
        }
        frame(&mut app, vec![btn(q, true)], 0.08);
        frame(&mut app, vec![btn(q, false)], 0.08);
        let after = fill(&app);
        assert!(
            after.is_some() && after != first,
            "tom do mosaico pinta: {after:?}"
        );
        assert_eq!(app.doc.as_ref().unwrap().selection, vec![id.clone()]);
        // Segurar e deslizar até o tom, soltando em cima dele, também pinta.
        let p = rects[6].center();
        frame(&mut app, vec![egui::Event::PointerMoved(p)], 0.05);
        frame(&mut app, vec![btn(p, true)], 0.05);
        for _ in 0..8 {
            frame(&mut app, vec![], 0.1);
        }
        let area = ctx
            .memory(|mem| mem.area_rect(egui::Id::unique("mosaico-tons-area")))
            .expect("mosaico aberto de novo");
        assert!(!area.contains(p), "o mosaico não cobre a amostra");
        let q = area.right_bottom() - egui::Vec2::splat(crate::theme::SWATCH);
        for k in 1..=8 {
            let m = p + (q - p) * (k as f32 / 8.0);
            frame(&mut app, vec![egui::Event::PointerMoved(m)], 0.05);
        }
        frame(&mut app, vec![btn(q, false)], 0.05);
        let slid = fill(&app);
        assert!(
            slid.is_some() && slid != after,
            "deslizar e soltar pinta: {slid:?}"
        );
    }

    /// GD-BDR-10/11: cor sem seleção pergunta antes de virar padrão; o X com
    /// o botão direito tira o contorno (um passo de desfazer).
    #[test]
    fn gd_bdr_10_11_palette_without_and_with_selection() {
        use crate::paletteui::PalAction;
        let ctx = crate::theme::test_ctx();
        let mut app = App::with_ctx(&ctx, None);
        let red = graphdraw_core::model::Color::from_hex("#ff0000").unwrap();
        app.apply_pal(PalAction::Fill(Some(red.clone())));
        assert!(app.ask_default.is_some(), "pergunta");
        assert!(app.defaults.fill.is_none(), "ainda não aplicou");
        app.ask_default = None;
        app.ws.default_color_no_ask = true;
        app.apply_pal(PalAction::Fill(Some(red.clone())));
        assert!(app.ask_default.is_none());
        assert_eq!(
            app.defaults.fill,
            Some(graphdraw_core::model::Fill::solid(red))
        );
        // Com seleção: X com o direito tira o contorno; desfazer volta.
        let id = {
            let d = app.doc.as_mut().unwrap();
            let mut o = graphdraw_core::model::Object::new(ObjectKind::Rectangle(
                graphdraw_core::model::RectData {
                    width: 10_000.0,
                    height: 10_000.0,
                    corners: None,
                    extra: Default::default(),
                },
            ));
            o.stroke = Some(graphdraw_core::model::Stroke::new(
                352.8,
                graphdraw_core::model::Color::black(),
            ));
            let id = ops::add_object(&mut d.editor, 0, 0, o, "r");
            d.selection = vec![id.clone()];
            id
        };
        app.apply_pal(PalAction::Stroke(None));
        let stroke = |app: &App| {
            let d = app.doc.as_ref().unwrap();
            d.editor
                .get(&d.editor.find(&id).unwrap())
                .unwrap()
                .stroke
                .clone()
        };
        assert!(stroke(&app).is_none());
        app.doc.as_mut().unwrap().editor.undo();
        assert_eq!(stroke(&app).unwrap().width, 352.8);
    }

    /// Menu Texto: negrito liga e desliga; converter em curvas tira o texto.
    #[test]
    fn text_menu_commands() {
        use graphdraw_core::edit::text::{insert, new_text};
        let ctx = crate::theme::test_ctx();
        let mut app = App::with_ctx(&ctx, None);
        let id = {
            let d = app.doc.as_mut().unwrap();
            let mut o = new_text(10_000.0, 10_000.0, None, "Arial", 5_000.0);
            if let ObjectKind::Text(t) = &mut o.kind {
                insert(t, 0, "abc");
            }
            let id = ops::add_object(&mut d.editor, 0, 0, o, "t");
            d.selection = vec![id.clone()];
            id
        };
        let weight = |app: &App| {
            let d = app.doc.as_ref().unwrap();
            match &d.editor.get(&d.editor.find(&id).unwrap()).unwrap().kind {
                ObjectKind::Text(t) => t.paragraphs[0].spans[0].font.weight,
                _ => 0,
            }
        };
        app.run_command(Command::TextBold, &ctx);
        assert_eq!(weight(&app), 700);
        app.run_command(Command::TextBold, &ctx);
        assert_eq!(weight(&app), 400);
        app.run_command(Command::TextToCurves, &ctx);
        let d = app.doc.as_ref().unwrap();
        assert!(!matches!(
            d.page().layers[0].objects[0].kind,
            ObjectKind::Text(_)
        ));
    }

    /// GD-PRE-01/06/10: a verificação acha o fundo encostado e a tinta
    /// demais; "Estender até a sangria" leva o fundo à sangria; o painel
    /// desenha a lista e "Verificação prévia" abre a aba.
    #[test]
    fn gd_pre_preflight_panel_and_extend_command() {
        let ctx = crate::theme::test_ctx();
        let mut app = App::with_ctx(&ctx, None);
        let id = {
            let d = app.doc.as_mut().unwrap();
            d.editor.edit_settings(|s| s.bleed = 3_000.0);
            let mut o = graphdraw_core::model::Object::new(ObjectKind::Rectangle(
                graphdraw_core::model::RectData {
                    width: 100_000.0,
                    height: 100_000.0,
                    corners: None,
                    extra: Default::default(),
                },
            ));
            o.transform = graphdraw_core::model::Transform::translate(1_000.0, 50_000.0);
            o.fill = Some(graphdraw_core::model::Fill::solid(
                graphdraw_core::model::Color::cmyk(1.0, 1.0, 1.0, 1.0),
            ));
            let id = ops::add_object(&mut d.editor, 0, 0, o, "r");
            d.selection = vec![id.clone()];
            id
        };
        let kinds = |app: &mut App| -> Vec<graphdraw_core::preflight::IssueKind> {
            app.doc
                .as_mut()
                .unwrap()
                .preflight()
                .iter()
                .map(|i| i.kind.clone())
                .collect()
        };
        let k = kinds(&mut app);
        assert!(
            k.contains(&graphdraw_core::preflight::IssueKind::NearEdge),
            "{k:?}"
        );
        assert!(
            k.iter()
                .any(|k| matches!(k, graphdraw_core::preflight::IssueKind::Ink { .. }))
        );
        app.run_command(Command::ExtendToBleed, &ctx);
        let d = app.doc.as_ref().unwrap();
        let b = ops::object_bounds(&d.editor, &d.editor.find(&id).unwrap(), false).unwrap();
        assert!((b.x0 + 3_000.0).abs() < 1e-6, "{b:?}");
        assert!(!kinds(&mut app).contains(&graphdraw_core::preflight::IssueKind::NearEdge));
        app.run_command(Command::Preflight, &ctx);
        assert_eq!(app.right_tab, RightTab::Preflight);
        let mut out = ctx.run_ui(Default::default(), |ui| {
            app.tab_content(ui, RightTab::Preflight)
        });
        out.textures_delta.clear();
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
