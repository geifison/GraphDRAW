//! Janela Imprimir (doc 29): impressora, layout, marcas, pré-impressão e a
//! prévia da folha; o envio roda em segundo plano com progresso.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, channel};

use eframe::egui;
use graphdraw_core::Document;
use graphdraw_core::export::{ExportOptions, PdfStandard};
use graphdraw_core::print::{self, Anchor, PageRange, Paper, Plan, PrintSettings, Scale};
use graphdraw_core::render::separation::{self, Plate};

use crate::doc::DocState;
use crate::i18n::tr;
use crate::printer::{self, DeviceInfo, PaperKind};
use crate::theme;
use crate::widgets;

const MM: f64 = 1000.0;
/// Resolução máxima da rasterização enviada (GD-PRT-02).
const MAX_DPI: f64 = 600.0;
/// Altura da prévia (pontos da interface).
const PREVIEW_H: f32 = 420.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    General,
    Layout,
    Marks,
    Prepress,
}

/// Papéis para imprimir em arquivo, sem impressora (GD-PRT-52).
const FILE_PAPERS: [(&str, f64, f64); 5] = [
    ("A4", 210.0, 297.0),
    ("A3", 297.0, 420.0),
    ("A2", 420.0, 594.0),
    ("Carta", 215.9, 279.4),
    ("Ofício", 215.9, 355.6),
];

enum Msg {
    Sheet(usize),
    Done(Result<String, String>),
}

struct Job {
    rx: Receiver<Msg>,
    cancel: Arc<AtomicBool>,
    total: usize,
    at: usize,
}

pub struct PrintDialog {
    printers: Vec<String>,
    /// Impressora escolhida; `None` = só arquivo.
    printer: Option<usize>,
    devmode: Vec<u8>,
    papers: Vec<PaperKind>,
    device: Option<DeviceInfo>,
    /// Sem impressora: papel do arquivo (índice em FILE_PAPERS; 5 = página).
    file_paper: usize,
    file_landscape: bool,
    pub settings: PrintSettings,
    range_text: String,
    percent: f64,
    tab: Tab,
    sheet: usize,
    plan: Result<Plan, String>,
    plan_key: Option<(u64, String)>,
    plates: Vec<(Plate, bool)>,
    preview: Option<((u64, String, usize), egui::TextureHandle)>,
    job: Option<Job>,
    message: Option<String>,
    pdf: PdfStandard,
}

impl PrintDialog {
    /// Abre com a impressora `remembered` (ou a padrão) e as opções da última
    /// impressão do documento.
    pub fn new(
        doc: &DocState,
        remembered: &str,
        last: Option<PrintSettings>,
        pdf: PdfStandard,
    ) -> PrintDialog {
        let (printers, default) = printer::printers();
        let pick = printers
            .iter()
            .position(|p| p == remembered)
            .or_else(|| default.and_then(|d| printers.iter().position(|p| *p == d)))
            .or(if printers.is_empty() { None } else { Some(0) });
        let job_name = doc
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| tr("Sem título", "Untitled").into());
        let mut settings = last.unwrap_or_default();
        settings.job_name = job_name;
        let percent = match settings.scale {
            Scale::Percent(p) => p,
            _ => 100.0,
        };
        let range_text = match &settings.range {
            PageRange::Custom(s) => s.clone(),
            _ => String::new(),
        };
        let plates = separation::plates(doc.doc())
            .into_iter()
            .map(|p| {
                let on = settings.plates.is_empty() || settings.plates.contains(&p);
                (p, on)
            })
            .collect();
        let mut d = PrintDialog {
            printers,
            printer: None,
            devmode: Vec::new(),
            papers: Vec::new(),
            device: None,
            file_paper: 0,
            file_landscape: false,
            settings,
            range_text,
            percent,
            tab: Tab::General,
            sheet: 0,
            plan: Err(String::new()),
            plan_key: None,
            plates,
            preview: None,
            job: None,
            message: None,
            pdf,
        };
        d.select_printer(pick);
        d
    }

    /// Nome da impressora escolhida (para guardar na área de trabalho).
    pub fn printer_name(&self) -> Option<&str> {
        self.printer.map(|i| self.printers[i].as_str())
    }

    fn select_printer(&mut self, i: Option<usize>) {
        self.printer = i;
        self.devmode.clear();
        self.papers.clear();
        if let Some(i) = i
            && let Some(dm) = printer::default_devmode(&self.printers[i])
        {
            self.papers = printer::papers(&self.printers[i], &dm);
            self.devmode = dm;
        }
        self.refresh_device();
    }

    fn refresh_device(&mut self) {
        self.device = self
            .printer
            .filter(|_| !self.devmode.is_empty())
            .and_then(|i| printer::device(&self.printers[i], &self.devmode));
        self.plan_key = None;
    }

    /// Folha do trabalho (impressora, ou o papel de arquivo).
    fn paper(&self, doc: &Document, page: usize) -> Paper {
        if let Some(d) = &self.device {
            return d.paper;
        }
        let (w, h) = match FILE_PAPERS.get(self.file_paper) {
            Some(&(_, w, h)) => (w * MM, h * MM),
            None => {
                let p = &doc.data.pages[page.min(doc.data.pages.len().saturating_sub(1))];
                (p.width, p.height)
            }
        };
        if self.file_landscape {
            Paper::full(h, w)
        } else {
            Paper::full(w, h)
        }
    }

    fn sync_settings(&mut self) {
        self.settings.plates = if self.plates.iter().all(|p| p.1) {
            Vec::new()
        } else {
            self.plates
                .iter()
                .filter(|p| p.1)
                .map(|p| p.0.clone())
                .collect()
        };
        if self.plates.iter().all(|p| !p.1) {
            // Nenhuma chapa marcada: volta a marcar todas.
            for p in &mut self.plates {
                p.1 = true;
            }
            self.settings.plates.clear();
        }
        if let PageRange::Custom(_) = self.settings.range {
            self.settings.range = PageRange::Custom(self.range_text.clone());
        }
        if let Scale::Percent(_) = self.settings.scale {
            self.settings.scale = Scale::Percent(self.percent);
        }
    }

    fn replan(&mut self, doc: &DocState) {
        let paper = self.paper(doc.doc(), doc.page);
        let key = (
            doc.editor.revision(),
            format!("{:?}{:?}{}", self.settings, paper, doc.page),
        );
        if self.plan_key.as_ref() == Some(&key) {
            return;
        }
        self.plan =
            print::plan(doc.doc(), doc.page, &self.settings, &paper).map_err(|e| e.to_string());
        if let Ok(p) = &self.plan {
            self.sheet = self.sheet.min(p.sheets.len().saturating_sub(1));
        }
        self.plan_key = Some(key);
    }

    /// Desenha a janela; devolve `true` quando ela fecha.
    pub fn show(&mut self, ctx: &egui::Context, doc: &DocState) -> bool {
        self.poll(ctx);
        self.sync_settings();
        self.replan(doc);
        let mut close = false;
        let busy = self.job.is_some();
        let resp = widgets::modal(ctx, egui::Id::unique("imprimir")).show(ctx, |ui| {
            ui.set_width(theme::PRINT_DIALOG_WIDTH);
            ui.heading(tr("Imprimir", "Print"));
            ui.add_space(theme::SPACE_200);
            ui.add_enabled_ui(!busy, |ui| {
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| {
                        ui.set_width(theme::PRINT_OPTIONS_WIDTH);
                        ui.horizontal(|ui| {
                            for (t, l) in [
                                (Tab::General, tr("Geral", "General")),
                                (Tab::Layout, tr("Layout", "Layout")),
                                (Tab::Marks, tr("Marcas", "Marks")),
                                (Tab::Prepress, tr("Pré-impressão", "Prepress")),
                            ] {
                                ui.selectable_value(&mut self.tab, t, l);
                            }
                        });
                        ui.separator();
                        egui::ScrollArea::vertical().max_height(PREVIEW_H).show(
                            ui,
                            |ui| match self.tab {
                                Tab::General => self.general(ui, doc),
                                Tab::Layout => self.layout(ui),
                                Tab::Marks => self.marks(ui),
                                Tab::Prepress => self.prepress(ui),
                            },
                        );
                    });
                    ui.add_space(theme::SPACE_400);
                    ui.vertical(|ui| self.preview_ui(ui, doc));
                });
            });
            ui.add_space(theme::SPACE_200);
            if let Some(j) = &self.job {
                ui.horizontal(|ui| {
                    ui.add(
                        egui::ProgressBar::new(j.at as f32 / j.total.max(1) as f32)
                            .desired_width(theme::PRINT_OPTIONS_WIDTH)
                            .text(format!(
                                "{} {} {} {}",
                                tr("Folha", "Sheet"),
                                (j.at + 1).min(j.total),
                                tr("de", "of"),
                                j.total
                            )),
                    );
                    if ui
                        .button(tr("Cancelar impressão", "Cancel printing"))
                        .clicked()
                    {
                        j.cancel.store(true, Ordering::Relaxed);
                    }
                });
            }
            if let Some(m) = &self.message {
                ui.label(m);
            }
            ui.separator();
            ui.horizontal(|ui| {
                let ok = self.plan.as_ref().is_ok_and(|p| !p.sheets.is_empty()) && !busy;
                if ui
                    .add_enabled(
                        ok,
                        egui::Button::new(tr(
                            "Imprimir para arquivo (PDF)…",
                            "Print to file (PDF)…",
                        )),
                    )
                    .clicked()
                {
                    self.print_to_file(ctx, doc);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add_enabled(
                            ok && self.device.is_some(),
                            egui::Button::new(tr("Imprimir", "Print")),
                        )
                        .clicked()
                    {
                        self.start(ctx, doc);
                    }
                    if ui
                        .add_enabled(!busy, egui::Button::new(tr("Fechar", "Close")))
                        .clicked()
                    {
                        close = true;
                    }
                });
            });
        });
        if resp.should_close() && !busy {
            close = true;
        }
        close
    }

    fn general(&mut self, ui: &mut egui::Ui, doc: &DocState) {
        ui.label(tr("Impressora", "Printer"));
        ui.horizontal(|ui| {
            let current = self
                .printer_name()
                .map(String::from)
                .unwrap_or_else(|| tr("(nenhuma — só arquivo)", "(none — file only)").into());
            let mut pick = self.printer;
            egui::ComboBox::from_id_salt("print-printer")
                .width(theme::PRINT_OPTIONS_WIDTH - 110.0)
                .selected_text(current)
                .show_ui(ui, |ui| {
                    for (i, p) in self.printers.iter().enumerate() {
                        ui.selectable_value(&mut pick, Some(i), p);
                    }
                });
            if pick != self.printer {
                self.select_printer(pick);
            }
            if ui
                .add_enabled(
                    self.printer.is_some(),
                    egui::Button::new(tr("Propriedades…", "Properties…")),
                )
                .on_hover_text(tr(
                    "Configurações do driver da impressora",
                    "Printer driver settings",
                ))
                .clicked()
                && let Some(i) = self.printer
                && printer::properties(&self.printers[i], &mut self.devmode, foreground())
            {
                self.refresh_device();
            }
        });
        if self.printers.is_empty() {
            ui.colored_label(
                theme::ACCENT_WARNING,
                tr(
                    "Nenhuma impressora instalada: use Imprimir para arquivo.",
                    "No printer installed: use Print to file.",
                ),
            );
        }
        ui.add_space(theme::SPACE_200);
        ui.label(tr("Papel", "Paper"));
        if self.device.is_some() {
            let id = printer::paper_id(&self.devmode);
            let name = self
                .papers
                .iter()
                .find(|p| p.id == id)
                .map(|p| p.name.clone())
                .unwrap_or_else(|| tr("(do driver)", "(from driver)").into());
            let mut pick = id;
            egui::ComboBox::from_id_salt("print-paper")
                .width(theme::PRINT_OPTIONS_WIDTH - 20.0)
                .selected_text(name)
                .show_ui(ui, |ui| {
                    for p in &self.papers {
                        let label = format!(
                            "{} ({:.0} × {:.0} mm)",
                            p.name,
                            p.size[0] / MM,
                            p.size[1] / MM
                        );
                        ui.selectable_value(&mut pick, p.id, label);
                    }
                });
            if pick != id {
                printer::set_paper(&mut self.devmode, pick);
                self.refresh_device();
            }
            let mut land = printer::landscape(&self.devmode);
            ui.horizontal(|ui| {
                let a = ui
                    .radio_value(&mut land, false, tr("Retrato", "Portrait"))
                    .changed();
                let b = ui
                    .radio_value(&mut land, true, tr("Paisagem", "Landscape"))
                    .changed();
                if a || b {
                    printer::set_landscape(&mut self.devmode, land);
                    self.refresh_device();
                }
            });
        } else {
            let name = FILE_PAPERS
                .get(self.file_paper)
                .map(|p| p.0.to_string())
                .unwrap_or_else(|| tr("Tamanho da página", "Page size").into());
            egui::ComboBox::from_id_salt("print-file-paper")
                .selected_text(name)
                .show_ui(ui, |ui| {
                    for (i, p) in FILE_PAPERS.iter().enumerate() {
                        ui.selectable_value(&mut self.file_paper, i, p.0);
                    }
                    ui.selectable_value(
                        &mut self.file_paper,
                        FILE_PAPERS.len(),
                        tr("Tamanho da página", "Page size"),
                    );
                });
            ui.horizontal(|ui| {
                ui.radio_value(&mut self.file_landscape, false, tr("Retrato", "Portrait"));
                ui.radio_value(&mut self.file_landscape, true, tr("Paisagem", "Landscape"));
            });
        }
        ui.add_space(theme::SPACE_200);
        ui.horizontal(|ui| {
            ui.label(tr("Cópias", "Copies"));
            ui.add(egui::DragValue::new(&mut self.settings.copies).range(1..=999));
            ui.checkbox(&mut self.settings.collate, tr("Agrupar", "Collate"))
                .on_hover_text(tr("Cópia 1 inteira, depois a 2…", "Whole copy 1, then 2…"));
        });
        ui.add_space(theme::SPACE_200);
        ui.label(tr("Páginas", "Pages"));
        let n = doc.doc().data.pages.len();
        ui.radio_value(
            &mut self.settings.range,
            PageRange::All,
            format!("{} ({n})", tr("Todas", "All")),
        );
        ui.radio_value(
            &mut self.settings.range,
            PageRange::Current,
            tr("Página atual", "Current page"),
        );
        ui.horizontal(|ui| {
            let custom = matches!(self.settings.range, PageRange::Custom(_));
            if ui.radio(custom, tr("Intervalo", "Range")).clicked() {
                self.settings.range = PageRange::Custom(self.range_text.clone());
            }
            let r = ui.add(
                egui::TextEdit::singleline(&mut self.range_text)
                    .hint_text("1,3-5")
                    .desired_width(120.0),
            );
            if r.changed() {
                self.settings.range = PageRange::Custom(self.range_text.clone());
            }
        });
    }

    fn layout(&mut self, ui: &mut egui::Ui) {
        let s = &mut self.settings;
        ui.label(tr("Escala", "Scale"));
        let mode = match s.scale {
            Scale::Actual => 0,
            Scale::Fit => 1,
            Scale::Percent(_) => 2,
        };
        let mut m = mode;
        ui.radio_value(&mut m, 0, tr("Tamanho real (100%)", "Actual size (100%)"));
        ui.radio_value(&mut m, 1, tr("Ajustar à folha", "Fit to sheet"));
        ui.horizontal(|ui| {
            ui.radio_value(&mut m, 2, tr("Porcentagem", "Percentage"));
            ui.add_enabled(
                m == 2,
                egui::DragValue::new(&mut self.percent)
                    .range(1.0..=1000.0)
                    .suffix(" %"),
            );
        });
        if m != mode {
            s.scale = match m {
                0 => Scale::Actual,
                1 => Scale::Fit,
                _ => Scale::Percent(self.percent),
            };
        }
        ui.add_space(theme::SPACE_200);
        ui.label(tr("Posição", "Position"));
        ui.horizontal(|ui| {
            ui.radio_value(&mut s.anchor, Anchor::Center, tr("Centralizar", "Center"));
            ui.radio_value(&mut s.anchor, Anchor::TopLeft, tr("Canto", "Corner"));
        });
        ui.horizontal(|ui| {
            ui.label("X");
            mm_drag(ui, &mut s.offset[0], -2000.0, 2000.0);
            ui.label("Y");
            mm_drag(ui, &mut s.offset[1], -2000.0, 2000.0);
        });
        ui.checkbox(
            &mut s.auto_rotate,
            tr("Girar automaticamente", "Auto-rotate"),
        );
        ui.add_space(theme::SPACE_200);
        ui.checkbox(&mut s.tile, tr("Lado a lado (ladrilhar)", "Tile"))
            .on_hover_text(tr(
                "Divide a página maior que a folha",
                "Splits a page larger than the sheet",
            ));
        ui.add_enabled_ui(s.tile, |ui| {
            ui.horizontal(|ui| {
                ui.label(tr("Sobreposição", "Overlap"));
                mm_drag(ui, &mut s.overlap, 0.0, 100.0);
            });
        });
        ui.add_space(theme::SPACE_200);
        ui.add_enabled_ui(!s.tile, |ui| {
            ui.checkbox(
                &mut s.nup,
                tr("Imposição (várias por folha)", "N-up (several per sheet)"),
            );
            ui.add_enabled_ui(s.nup, |ui| {
                ui.horizontal(|ui| {
                    ui.label(tr("Colunas", "Columns"));
                    ui.add(egui::DragValue::new(&mut s.nup_cols).range(0..=50))
                        .on_hover_text(tr("0 = o máximo que cabe", "0 = as many as fit"));
                    ui.label(tr("Linhas", "Rows"));
                    ui.add(egui::DragValue::new(&mut s.nup_rows).range(0..=50))
                        .on_hover_text(tr("0 = o máximo que cabe", "0 = as many as fit"));
                });
                ui.horizontal(|ui| {
                    ui.label(tr("Espaço", "Gap"));
                    mm_drag(ui, &mut s.nup_gap, 0.0, 100.0);
                });
                ui.checkbox(
                    &mut s.nup_distinct,
                    tr("Páginas diferentes nas células", "Different pages in cells"),
                );
            });
        });
    }

    fn marks(&mut self, ui: &mut egui::Ui) {
        let s = &mut self.settings;
        ui.checkbox(&mut s.crop_marks, tr("Marcas de corte", "Crop marks"));
        ui.checkbox(
            &mut s.reg_marks,
            tr("Marcas de registro", "Registration marks"),
        );
        ui.checkbox(&mut s.color_bar, tr("Barra de cores", "Color bar"));
        ui.checkbox(
            &mut s.info,
            tr(
                "Informações (arquivo, página, chapa, data)",
                "Information (file, page, plate, date)",
            ),
        );
        ui.add_space(theme::SPACE_200);
        ui.checkbox(&mut s.print_bleed, tr("Imprimir sangria", "Print bleed"))
            .on_hover_text(tr(
                "A página sai com a sangria do documento além do corte",
                "The page includes the document bleed",
            ));
        ui.add_space(theme::SPACE_200);
        ui.weak(tr(
            "As marcas saem na cor de registro (todas as chapas).",
            "Marks use the registration color (every plate).",
        ));
    }

    fn prepress(&mut self, ui: &mut egui::Ui) {
        let s = &mut self.settings;
        ui.checkbox(&mut s.mirror, tr("Espelhar", "Mirror"))
            .on_hover_text(tr(
                "Transfer, sublimação, verso de transparente",
                "Transfer, sublimation, back of clear film",
            ));
        ui.checkbox(&mut s.negative, tr("Negativo", "Negative"))
            .on_hover_text(tr(
                "Inverte as cores (filme); não vale para o PDF",
                "Inverts colors (film); not for PDF",
            ));
        ui.add_space(theme::SPACE_200);
        ui.checkbox(
            &mut s.separations,
            tr(
                "Separações (uma folha por chapa)",
                "Separations (one sheet per plate)",
            ),
        );
        ui.add_enabled_ui(s.separations, |ui| {
            for (p, on) in &mut self.plates {
                ui.checkbox(on, print::plate_name(p));
            }
        });
    }

    fn preview_ui(&mut self, ui: &mut egui::Ui, doc: &DocState) {
        let paper = self.paper(doc.doc(), doc.page);
        let plan = match &self.plan {
            Ok(p) => p.clone(),
            Err(e) => {
                ui.set_min_size(egui::vec2(PREVIEW_H * 0.75, PREVIEW_H));
                ui.colored_label(theme::ACCENT_DANGER, e);
                return;
            }
        };
        let n = plan.sheets.len();
        // Área da prévia: a folha cabe em PREVIEW_H de altura.
        let avail = egui::vec2(ui.available_width().max(PREVIEW_H * 0.75), PREVIEW_H);
        let k = (avail.x / paper.width as f32).min(avail.y / paper.height as f32);
        let size = egui::vec2(paper.width as f32 * k, paper.height as f32 * k);
        let (rect, _) = ui.allocate_exact_size(avail, egui::Sense::hover());
        let sheet_rect = egui::Rect::from_center_size(rect.center(), size);
        let p = ui.painter_at(rect);
        p.rect_filled(
            sheet_rect.translate(egui::vec2(3.0, 3.0)),
            0.0,
            egui::Color32::from_black_alpha(60),
        );
        if let Some(sh) = plan.sheets.get(self.sheet) {
            let ppp = ui.ctx().pixels_per_point();
            let (pw, ph) = ((size.x * ppp).round() as u32, (size.y * ppp).round() as u32);
            let key = (
                self.plan_key.as_ref().map_or(0, |k| k.0),
                self.plan_key
                    .as_ref()
                    .map_or(String::new(), |k| k.1.clone()),
                self.sheet,
            );
            let fresh = self.preview.as_ref().is_none_or(|(k, _)| *k != key);
            if fresh && pw > 0 && ph > 0 {
                let dpi = pw as f64 / (paper.width / graphdraw_core::units::UM_PER_INCH);
                let scenes = print::page_scenes_for(doc.doc(), sh);
                let marks = print::marks_scene(&plan, sh, &self.settings, &paper);
                if let Some(pm) = print::render_region(
                    &scenes,
                    &marks,
                    sh,
                    &paper,
                    &self.settings,
                    [dpi, dpi],
                    (0, 0, pw, ph),
                ) {
                    let rgba = graphdraw_core::render::raster::unpremultiply(&pm);
                    let img =
                        egui::ColorImage::from_rgba_unmultiplied([pw as usize, ph as usize], &rgba);
                    let tex =
                        ui.ctx()
                            .load_texture("print-preview", img, egui::TextureOptions::LINEAR);
                    self.preview = Some((key, tex));
                }
            }
            if let Some((_, tex)) = &self.preview {
                p.image(
                    tex.id(),
                    sheet_rect,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    egui::Color32::WHITE,
                );
            }
        }
        p.rect_stroke(
            sheet_rect,
            0.0,
            theme::border_default(ui),
            egui::StrokeKind::Outside,
        );
        // Área imprimível tracejada.
        let pr = paper.printable;
        let to = |x: f64, y: f64| {
            egui::pos2(
                sheet_rect.left() + x as f32 * k,
                sheet_rect.bottom() - y as f32 * k,
            )
        };
        let corners = [
            to(pr.x0, pr.y0),
            to(pr.x1, pr.y0),
            to(pr.x1, pr.y1),
            to(pr.x0, pr.y1),
            to(pr.x0, pr.y0),
        ];
        let dash = egui::Stroke::new(1.0, theme::ACCENT_PRIMARY.gamma_multiply(0.7));
        p.extend(egui::Shape::dashed_line(&corners, dash, 4.0, 3.0));
        ui.horizontal(|ui| {
            if ui
                .add_enabled(self.sheet > 0, egui::Button::new("◀"))
                .clicked()
            {
                self.sheet -= 1;
            }
            ui.label(format!(
                "{} {} {} {}",
                tr("Folha", "Sheet"),
                (self.sheet + 1).min(n),
                tr("de", "of"),
                n
            ));
            if ui
                .add_enabled(self.sheet + 1 < n, egui::Button::new("▶"))
                .clicked()
            {
                self.sheet += 1;
            }
        });
        let copies = self.settings.copies.max(1);
        ui.weak(format!(
            "{:.0} × {:.0} mm · {} {:.0}%{} · {} {}{}",
            paper.width / MM,
            paper.height / MM,
            tr("escala", "scale"),
            plan.scale * 100.0,
            if plan.rotated {
                tr(", girada", ", rotated")
            } else {
                ""
            },
            n,
            tr("folha(s)", "sheet(s)"),
            if copies > 1 {
                format!(" × {copies}")
            } else {
                String::new()
            },
        ));
        for w in &plan.warnings {
            ui.colored_label(theme::ACCENT_WARNING, w);
        }
        if self.settings.negative {
            ui.weak(tr(
                "Negativo não se aplica ao PDF.",
                "Negative does not apply to PDF.",
            ));
        }
    }

    /// Envia à impressora em segundo plano (GD-PRT-04).
    fn start(&mut self, ctx: &egui::Context, doc: &DocState) {
        let (Ok(plan), Some(i)) = (self.plan.clone(), self.printer) else {
            return;
        };
        let name = self.printers[i].clone();
        let devmode = self.devmode.clone();
        let document = doc.doc().clone();
        let s = self.settings.clone();
        let paper = self.paper(doc.doc(), doc.page);
        let order = plan.print_order(s.copies, s.collate);
        let (tx, rx) = channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let stop = cancel.clone();
        let repaint = ctx.clone();
        self.job = Some(Job {
            rx,
            cancel,
            total: order.len(),
            at: 0,
        });
        self.message = None;
        std::thread::spawn(move || {
            let scenes = print::page_scenes(&document, &plan);
            let mut marks: std::collections::HashMap<usize, graphdraw_core::render::scene::Scene> =
                Default::default();
            let mut band = |i: usize, dpi: [f64; 2], px: (i64, i64, u32, u32)| {
                let sh = &plan.sheets[order[i]];
                let m = marks
                    .entry(order[i])
                    .or_insert_with(|| print::marks_scene(&plan, sh, &s, &paper));
                let pm = print::render_region(&scenes, m, sh, &paper, &s, dpi, px)?;
                Some(graphdraw_core::render::raster::unpremultiply(&pm))
            };
            let mut progress = |i: usize| {
                let _ = tx.send(Msg::Sheet(i));
                repaint.request_repaint();
                !stop.load(Ordering::Relaxed)
            };
            let r = printer::print(
                &name,
                &devmode,
                &s.job_name,
                None,
                order.len(),
                MAX_DPI,
                &mut band,
                &mut progress,
            );
            let _ = tx.send(Msg::Done(r.map(|_| name)));
            repaint.request_repaint();
        });
    }

    /// Imprimir para arquivo: PDF das folhas (GD-PRT-03).
    fn print_to_file(&mut self, ctx: &egui::Context, doc: &DocState) {
        let Ok(plan) = self.plan.clone() else { return };
        let stem = self
            .settings
            .job_name
            .rsplit_once('.')
            .map_or(self.settings.job_name.as_str(), |s| s.0)
            .to_string();
        let Some(out) = rfd::FileDialog::new()
            .set_file_name(format!("{stem}-impressao.pdf"))
            .add_filter("PDF", &["pdf"])
            .save_file()
        else {
            return;
        };
        let document = doc.doc().clone();
        let s = self.settings.clone();
        let paper = self.paper(doc.doc(), doc.page);
        let opts = ExportOptions {
            pdf: self.pdf,
            ..Default::default()
        };
        let (tx, rx) = channel();
        let repaint = ctx.clone();
        self.job = Some(Job {
            rx,
            cancel: Arc::new(AtomicBool::new(false)),
            total: plan.sheets.len(),
            at: 0,
        });
        self.message = None;
        std::thread::spawn(move || {
            let r = print::sheets_pdf(&document, &plan, &paper, &s, &opts)
                .map_err(|e| e.to_string())
                .and_then(|(bytes, _)| std::fs::write(&out, bytes).map_err(|e| e.to_string()))
                .map(|_| out.display().to_string());
            let _ = tx.send(Msg::Done(r));
            repaint.request_repaint();
        });
    }

    fn poll(&mut self, ctx: &egui::Context) {
        let Some(job) = &mut self.job else { return };
        let mut done = None;
        while let Ok(m) = job.rx.try_recv() {
            match m {
                Msg::Sheet(i) => job.at = i,
                Msg::Done(r) => done = Some(r),
            }
        }
        match done {
            Some(Ok(to)) => {
                self.message = Some(format!("{} {to}", tr("Enviado para", "Sent to")));
                self.job = None;
            }
            Some(Err(e)) => {
                self.message = Some(format!(
                    "{}: {e}",
                    tr("Impressão não concluída", "Printing not finished")
                ));
                self.job = None;
            }
            None => ctx.request_repaint_after(std::time::Duration::from_millis(200)),
        }
    }
}

/// Campo em milímetros para um valor em µm.
fn mm_drag(ui: &mut egui::Ui, v: &mut f64, min: f64, max: f64) {
    let mut mm = *v / MM;
    if ui
        .add(
            egui::DragValue::new(&mut mm)
                .range(min..=max)
                .speed(0.5)
                .suffix(" mm")
                .max_decimals(1),
        )
        .changed()
    {
        *v = mm * MM;
    }
}

/// Janela em primeiro plano, dona da janela do driver.
fn foreground() -> isize {
    #[cfg(windows)]
    unsafe {
        windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow().0 as isize
    }
    #[cfg(not(windows))]
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dialog_plans_every_tab() {
        let doc = DocState::new_blank();
        let mut d = PrintDialog::new(&doc, "", None, PdfStandard::Screen);
        // Sem depender da máquina: imprimir para arquivo, em A4.
        d.select_printer(None);
        d.settings.crop_marks = true;
        d.settings.scale = Scale::Fit;
        let ctx = crate::theme::test_ctx();
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1366.0, 768.0));
        for (k, tab) in [Tab::General, Tab::Layout, Tab::Marks, Tab::Prepress]
            .into_iter()
            .enumerate()
        {
            d.tab = tab;
            let input = egui::RawInput {
                screen_rect: Some(screen),
                time: Some(k as f64 / 30.0),
                ..Default::default()
            };
            let mut out = ctx.run_ui(input, |ui| {
                assert!(!d.show(ui.ctx(), &doc));
            });
            out.textures_delta.clear();
        }
        let plan = d.plan.as_ref().expect("plano");
        assert_eq!(plan.sheets.len(), 1);
        assert!(d.preview.is_some(), "a prévia foi desenhada");
        // Separações: uma folha por chapa.
        d.settings.separations = true;
        d.replan(&doc);
        assert_eq!(d.plan.as_ref().unwrap().sheets.len(), 4);
    }

    #[test]
    #[ignore]
    fn shot_print_dialog() {
        use graphdraw_core::model::{Color, Fill, Object, ObjectKind, RectData};
        let mut doc = DocState::new_blank();
        let mut o = Object::new(ObjectKind::Rectangle(RectData {
            width: 90_000.0,
            height: 50_000.0,
            corners: None,
            extra: Default::default(),
        }));
        o.fill = Some(Fill::solid(Color::cmyk(0.0, 0.7, 1.0, 0.0)));
        graphdraw_core::edit::ops::add_object(&mut doc.editor, 0, 0, o, "r");
        doc.editor.doc.data.pages[0].width = 90_000.0;
        doc.editor.doc.data.pages[0].height = 50_000.0;
        let ctx = crate::theme::test_ctx();
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1366.0, 768.0));
        // Um Shot só: a textura das fontes vem uma vez.
        let mut shot = crate::uishot::Shot::default();
        for (name, tab) in [
            ("impressao-geral", Tab::General),
            ("impressao-layout", Tab::Layout),
            ("impressao-pre", Tab::Prepress),
        ] {
            let mut d = PrintDialog::new(&doc, "", None, PdfStandard::Screen);
            d.select_printer(None);
            d.tab = tab;
            d.settings.nup = true;
            d.settings.crop_marks = true;
            d.settings.reg_marks = true;
            d.settings.color_bar = true;
            d.settings.info = true;
            for k in 0..20 {
                let input = egui::RawInput {
                    screen_rect: Some(screen),
                    time: Some(k as f64 / 30.0),
                    ..Default::default()
                };
                let mut out = ctx.run_ui(input, |ui| {
                    d.show(ui.ctx(), &doc);
                });
                if k == 19 {
                    shot.save(&ctx, out, name);
                } else {
                    shot.textures(&mut out);
                }
            }
        }
    }
}
