//! Janela "Novo documento" (doc 22, GD-DIM-10..14): biblioteca de
//! predefinições com busca e categorias à esquerda; prévia e medidas à
//! direita. Tudo nos tokens do DESIGN-SYSTEM.

use eframe::egui::{self, Pos2, Rect, Sense, Vec2};
use graphdraw_core::ImportReport;
use graphdraw_core::edit::pagesize::{self, Category, PRESETS, Preset};
use graphdraw_core::model::{Document, Page};
use graphdraw_core::units::{self, Unit};

use crate::doc::DocState;
use crate::i18n::tr;
use crate::{icons, theme, widgets};

/// Filtro da biblioteca.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Filter {
    All,
    Cat(Category),
    Custom,
}

pub struct NewDocDialog {
    search: String,
    filter: Filter,
    /// Predefinição escolhida (índice em `PRESETS`).
    preset: Option<usize>,
    width: f64,
    height: f64,
    unit: Unit,
    bleed: f64,
    dpi: f64,
    /// `cmyk` ou `rgb`.
    color: &'static str,
    name: String,
    focus_search: bool,
    /// Área usada no último quadro (testes de layout).
    pub(crate) rect: Rect,
}

pub enum Outcome {
    Open,
    Cancel,
    Create(Box<DocState>),
}

const UNITS: [Unit; 5] = [
    Unit::Millimeter,
    Unit::Centimeter,
    Unit::Inch,
    Unit::Pixel,
    Unit::Point,
];

fn category_label(c: Category) -> &'static str {
    match c {
        Category::Print => tr("Impressão", "Print"),
        Category::Screen => tr("Telas/UI", "Screens/UI"),
        Category::Social => tr("Social", "Social"),
    }
}

/// Valor em `unit` com até 1 casa (2 para polegadas), vírgula em pt-BR.
pub fn fmt_unit(um: f64, unit: Unit) -> String {
    let decimals = if unit == Unit::Inch { 2 } else { 1 };
    widgets::fmt_num(units::from_um(um, unit), decimals)
}

fn preset_dims(p: &Preset) -> String {
    let unit = if p.pixels {
        Unit::Pixel
    } else {
        Unit::Millimeter
    };
    format!(
        "{} × {} {}",
        fmt_unit(p.width, unit),
        fmt_unit(p.height, unit),
        unit.name()
    )
}

impl Default for NewDocDialog {
    fn default() -> Self {
        let a4 = PRESETS.iter().position(|p| p.name == "A4");
        let mut d = NewDocDialog {
            search: String::new(),
            filter: Filter::All,
            preset: None,
            width: 210_000.0,
            height: 297_000.0,
            unit: Unit::Millimeter,
            bleed: 0.0,
            dpi: 300.0,
            color: "cmyk",
            name: String::new(),
            focus_search: true,
            rect: Rect::NOTHING,
        };
        if let Some(i) = a4 {
            d.choose(i);
        }
        d
    }
}

impl NewDocDialog {
    fn choose(&mut self, i: usize) {
        let p = &PRESETS[i];
        self.preset = Some(i);
        self.width = p.width;
        self.height = p.height;
        self.dpi = p.dpi;
        self.color = p.color;
        self.unit = if p.pixels {
            Unit::Pixel
        } else {
            Unit::Millimeter
        };
    }

    /// Predefinições visíveis com a busca e o filtro atuais.
    fn visible(&self) -> Vec<usize> {
        if self.filter == Filter::Custom {
            return Vec::new();
        }
        let found = pagesize::search(&self.search);
        PRESETS
            .iter()
            .enumerate()
            .filter(|(_, p)| found.iter().any(|f| f.name == p.name))
            .filter(|(_, p)| match self.filter {
                Filter::Cat(c) => p.category == c,
                _ => true,
            })
            .map(|(i, _)| i)
            .collect()
    }

    /// Selo de aproveitamento (GD-DIM-13).
    fn fit_badge(&self) -> String {
        let sheet = |name: &str| PRESETS.iter().find(|p| p.name == name).unwrap();
        let (a4, a3) = (sheet("A4"), sheet("A3"));
        let n4 = pagesize::fit_count(a4.width, a4.height, self.width, self.height);
        let n3 = pagesize::fit_count(a3.width, a3.height, self.width, self.height);
        match (n4, n3) {
            (0, 0) => tr("Maior que uma folha A3", "Larger than an A3 sheet").into(),
            (0, n) => format!("{} {n}× {}", tr("Cabe", "Fits"), tr("em A3", "on A3")),
            (n, m) => format!(
                "{} {n}× {} · {m}× {}",
                tr("Cabe", "Fits"),
                tr("em A4", "on A4"),
                tr("em A3", "on A3")
            ),
        }
    }

    /// Documento novo com as escolhas da janela.
    pub fn build(&self) -> DocState {
        let mut doc = Document::new();
        let mut page = Page::new(self.width, self.height);
        page.name = Some(tr("Página 1", "Page 1").into());
        doc.data.pages.push(page);
        doc.data.settings.bleed = self.bleed;
        // Perfil CMYK padrão de produção (GD-CMS-01): offset couché FOGRA39.
        doc.data.settings.color.cmyk_profile = Some(graphdraw_core::cms::FOGRA39.into());
        doc.data.settings.resolution = self.dpi;
        // Modelo de cor do documento (GD-DIM-14); o perfil ICC vem com o GD-CMS.
        doc.data
            .settings
            .extra
            .insert("colorModel".into(), self.color.into());
        doc.data.settings.display_unit = self.unit.name().into();
        let name = self.name.trim();
        if !name.is_empty() {
            doc.metadata.title = Some(name.into());
        }
        DocState::from_document(doc, None, ImportReport::new("gdr"))
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        let mut out = Outcome::Open;
        // "/" foca a busca (fora de campos de texto).
        if !ctx.egui_wants_keyboard_input() && ctx.input(|i| i.key_pressed(egui::Key::Slash)) {
            self.focus_search = true;
        }
        let screen = ctx.content_rect();
        let max_h = (screen.height() * theme::MODAL_MAX_HEIGHT).max(360.0);
        let resp = widgets::modal(ctx, egui::Id::unique("novo-documento")).show(ctx, |ui| {
            ui.set_width(860.0_f32.min(screen.width() - 2.0 * theme::SPACE_600));
            ui.label(
                egui::RichText::new(tr("Novo documento", "New document"))
                    .font(theme::font_display()),
            );
            ui.add_space(theme::SPACE_300);
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(520.0);
                    self.library(ui, max_h - 140.0);
                });
                ui.add_space(theme::SPACE_400);
                ui.vertical(|ui| {
                    ui.set_width(300.0);
                    self.inspector(ui);
                });
            });
            ui.add_space(theme::SPACE_300);
            widgets::footer(ui, |ui| {
                if ui
                    .add(
                        widgets::primary_button(tr("Criar documento", "Create document"))
                            .min_size(Vec2::new(0.0, theme::BUTTON_PROMINENT)),
                    )
                    .clicked()
                    || ui.input(|i| i.key_pressed(egui::Key::Enter) && i.modifiers.command)
                {
                    out = Outcome::Create(Box::new(self.build()));
                }
                if ui
                    .add(widgets::secondary_button(tr("Cancelar", "Cancel")))
                    .clicked()
                    || ui.input(|i| i.key_pressed(egui::Key::Escape))
                {
                    out = Outcome::Cancel;
                }
            });
        });
        self.rect = resp.response.rect;
        out
    }

    fn library(&mut self, ui: &mut egui::Ui, height: f32) {
        let pal = theme::palette(ui);
        let r = ui.add(
            egui::TextEdit::singleline(&mut self.search)
                .hint_text(tr(
                    "Procurar formato: A5, cartão, 1080…  ( / )",
                    "Search format: A5, card, 1080…  ( / )",
                ))
                .desired_width(f32::INFINITY),
        );
        if self.focus_search {
            r.request_focus();
            self.focus_search = false;
        }
        ui.add_space(theme::SPACE_200);
        ui.horizontal_wrapped(|ui| {
            for (f, label) in [
                (Filter::All, tr("Todos", "All")),
                (
                    Filter::Cat(Category::Print),
                    category_label(Category::Print),
                ),
                (
                    Filter::Cat(Category::Screen),
                    category_label(Category::Screen),
                ),
                (
                    Filter::Cat(Category::Social),
                    category_label(Category::Social),
                ),
                (Filter::Custom, tr("Personalizado", "Custom")),
            ] {
                if ui.selectable_label(self.filter == f, label).clicked() {
                    self.filter = f;
                    if f == Filter::Custom {
                        self.preset = None;
                    }
                }
            }
        });
        ui.add_space(theme::SPACE_200);
        let list = self.visible();
        egui::ScrollArea::vertical()
            .id_salt("biblioteca-formatos")
            .max_height(height.max(160.0))
            .auto_shrink([false, true])
            .show(ui, |ui| {
                if self.filter == Filter::Custom {
                    ui.label(
                        egui::RichText::new(tr(
                            "Defina largura, altura e unidade à direita.",
                            "Set width, height and unit on the right.",
                        ))
                        .color(pal.fg_muted),
                    );
                    return;
                }
                if list.is_empty() {
                    ui.label(
                        egui::RichText::new(tr("Nenhum formato encontrado.", "No format found."))
                            .color(pal.fg_muted),
                    );
                }
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::splat(theme::SPACE_200);
                    for i in list {
                        if self.card(ui, i) {
                            self.choose(i);
                        }
                    }
                });
            });
    }

    /// Cartão proporcional do formato (GD-DIM-11). Devolve `true` ao clicar.
    fn card(&self, ui: &mut egui::Ui, i: usize) -> bool {
        let p = &PRESETS[i];
        let pal = theme::palette(ui);
        let size = Vec2::new(122.0, 132.0);
        let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
        let selected = self.preset == Some(i);
        let fill = if selected {
            pal.bg_surface_toggled
        } else if resp.hovered() {
            pal.bg_surface_hover
        } else {
            pal.bg_surface_2
        };
        let painter = ui.painter();
        painter.rect_filled(rect, theme::RADIUS_MD, fill);
        painter.rect_stroke(
            rect,
            theme::RADIUS_MD,
            if selected {
                theme::border_focus()
            } else {
                theme::border_default(ui)
            },
            egui::StrokeKind::Inside,
        );
        // Folha proporcional numa caixa de 64 px.
        let k = 64.0 / p.width.max(p.height) as f32;
        let sheet = Vec2::new(p.width as f32 * k, p.height as f32 * k);
        let center = Pos2::new(rect.center().x, rect.top() + theme::SPACE_300 + 32.0);
        let sr = Rect::from_center_size(center, sheet);
        painter.rect_filled(sr, theme::RADIUS_XS, theme::PAGE_PAPER);
        painter.rect_stroke(
            sr,
            theme::RADIUS_XS,
            theme::border_subtle(ui),
            egui::StrokeKind::Outside,
        );
        let mut y = rect.top() + theme::SPACE_300 + 64.0 + theme::SPACE_200;
        for (text, font, color) in [
            (p.name.to_string(), theme::font_body(), pal.fg_strong),
            (preset_dims(p), theme::font_numeric(), pal.fg_primary),
            (
                format!("{:.0} ppp · {}", p.dpi, p.color.to_uppercase()),
                theme::font_micro(),
                pal.fg_muted,
            ),
        ] {
            let g = painter.layout(text, font, color, rect.width() - 2.0 * theme::SPACE_200);
            let h = g.size().y;
            painter.galley(Pos2::new(rect.center().x - g.size().x / 2.0, y), g, color);
            y += h + theme::SPACE_050;
        }
        resp.on_hover_text(format!("{} · {}", p.name, preset_dims(p)))
            .clicked()
    }

    fn inspector(&mut self, ui: &mut egui::Ui) {
        let pal = theme::palette(ui);
        // Prévia proporcional com a sangria.
        let (area, _) = ui.allocate_exact_size(Vec2::new(300.0, 170.0), Sense::hover());
        ui.painter()
            .rect_filled(area, theme::RADIUS_MD, pal.bg_canvas);
        let total_w = self.width + 2.0 * self.bleed;
        let total_h = self.height + 2.0 * self.bleed;
        let k = ((area.width() - 2.0 * theme::SPACE_400) as f64 / total_w)
            .min((area.height() - 2.0 * theme::SPACE_400) as f64 / total_h);
        let sheet = Rect::from_center_size(
            area.center(),
            Vec2::new((self.width * k) as f32, (self.height * k) as f32),
        );
        ui.painter()
            .add(pal.shadow_page().as_shape(sheet, theme::RADIUS_NONE));
        ui.painter()
            .rect_filled(sheet, theme::RADIUS_NONE, theme::PAGE_PAPER);
        if self.bleed > 0.0 {
            let b = sheet.expand((self.bleed * k) as f32);
            ui.painter().extend(egui::Shape::dashed_line(
                &[
                    b.left_top(),
                    b.right_top(),
                    b.right_bottom(),
                    b.left_bottom(),
                    b.left_top(),
                ],
                egui::Stroke::new(theme::GUIDE_WIDTH, theme::ACCENT_SUCCESS),
                theme::BLEED_DASH,
                theme::BLEED_GAP,
            ));
        }
        ui.add_space(theme::SPACE_200);
        // Selo de aproveitamento.
        let badge = self.fit_badge();
        let g = ui
            .painter()
            .layout_no_wrap(badge, theme::font_micro(), theme::FG_ON_ACCENT);
        let (br, _) = ui.allocate_exact_size(
            g.size() + Vec2::new(2.0 * theme::SPACE_200, 2.0 * theme::SPACE_100),
            Sense::hover(),
        );
        ui.painter()
            .rect_filled(br, theme::RADIUS_FULL, theme::ACCENT_SUCCESS);
        ui.painter().galley(
            br.min + Vec2::new(theme::SPACE_200, theme::SPACE_100),
            g,
            theme::FG_ON_ACCENT,
        );
        ui.add_space(theme::SPACE_300);
        egui::Grid::new("novo-doc-medidas")
            .num_columns(2)
            .spacing(Vec2::new(theme::SPACE_200, theme::SPACE_200))
            .show(ui, |ui| {
                ui.label(tr("Nome", "Name"));
                ui.add(
                    egui::TextEdit::singleline(&mut self.name)
                        .hint_text(tr("Sem título", "Untitled"))
                        .desired_width(180.0),
                );
                ui.end_row();
                ui.label(tr("Unidade", "Unit"));
                egui::ComboBox::from_id_salt("novo-doc-unidade")
                    .selected_text(self.unit.name())
                    .width(80.0)
                    .show_ui(ui, |ui| {
                        for u in UNITS {
                            ui.selectable_value(&mut self.unit, u, u.name());
                        }
                    });
                ui.end_row();
                ui.label(tr("Largura", "Width"));
                if let Some(v) = unit_field(ui, "novo-l", self.width, self.unit) {
                    self.width = v.max(1_000.0);
                    self.preset = None;
                }
                ui.end_row();
                ui.label(tr("Altura", "Height"));
                if let Some(v) = unit_field(ui, "novo-a", self.height, self.unit) {
                    self.height = v.max(1_000.0);
                    self.preset = None;
                }
                ui.end_row();
                ui.label(tr("Orientação", "Orientation"));
                ui.horizontal(|ui| {
                    let portrait = self.height >= self.width;
                    if icons::button(
                        ui,
                        "page-portrait",
                        tr("Retrato", "Portrait"),
                        portrait,
                        true,
                    )
                    .clicked()
                        && !portrait
                    {
                        std::mem::swap(&mut self.width, &mut self.height);
                    }
                    if icons::button(
                        ui,
                        "page-landscape",
                        tr("Paisagem", "Landscape"),
                        !portrait,
                        true,
                    )
                    .clicked()
                        && portrait
                    {
                        std::mem::swap(&mut self.width, &mut self.height);
                    }
                });
                ui.end_row();
                ui.label(tr("Sangria", "Bleed"));
                ui.horizontal(|ui| {
                    let mut on = self.bleed > 0.0;
                    if ui.checkbox(&mut on, "").changed() {
                        self.bleed = if on { 3_000.0 } else { 0.0 };
                    }
                    if on && let Some(v) = unit_field(ui, "novo-s", self.bleed, Unit::Millimeter) {
                        self.bleed = v.max(0.0);
                    }
                    if !on {
                        ui.label(
                            egui::RichText::new(tr("sem sangria", "no bleed"))
                                .color(theme::palette(ui).fg_muted),
                        );
                    }
                });
                ui.end_row();
                ui.label(tr("Resolução", "Resolution"));
                ui.add(
                    egui::DragValue::new(&mut self.dpi)
                        .range(72.0..=2400.0)
                        .speed(1.0)
                        .suffix(" ppp"),
                );
                ui.end_row();
                ui.label(tr("Cor", "Color"));
                ui.horizontal(|ui| {
                    for (m, label) in [("cmyk", "CMYK"), ("rgb", "RGB")] {
                        if ui.selectable_label(self.color == m, label).clicked() {
                            self.color = m;
                        }
                    }
                });
                ui.end_row();
            });
        ui.label(
            egui::RichText::new(tr(
                "CMYK para impressão, RGB para telas. O perfil ICC entra com o gerenciamento de cor.",
                "CMYK for print, RGB for screens. ICC profiles come with color management.",
            ))
            .font(theme::font_micro())
            .color(pal.fg_muted),
        );
    }
}

/// Campo de medida numa unidade qualquer (aceita contas e outras unidades:
/// "10in" num campo em mm converte). Devolve µm ao confirmar.
fn unit_field(ui: &mut egui::Ui, id: &str, value_um: f64, unit: Unit) -> Option<f64> {
    let id = ui.scope_id().with(id);
    // A unidade vai dentro do campo (padrão dos campos de medida).
    let shown = format!("{} {}", fmt_unit(value_um, unit), unit.name());
    let mut text: String = ui
        .data(|d| d.get_temp::<String>(id))
        .unwrap_or_else(|| shown.clone());
    let mut out = None;
    ui.horizontal(|ui| {
        let r = ui.add(
            egui::TextEdit::singleline(&mut text)
                .id(id.with("campo"))
                .font(theme::font_numeric())
                .desired_width(110.0),
        );
        if r.has_focus() {
            ui.data_mut(|d| d.insert_temp(id, text.clone()));
        } else {
            ui.data_mut(|d| d.remove::<String>(id));
        }
        if r.lost_focus() && text != shown {
            out = units::parse_length(&text.replace(',', "."), unit, Some(value_um));
        }
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Aceite: 210 mm em px = 793,7 px (96 ppp); de volta, 210 mm.
    #[test]
    fn unit_conversion_acceptance() {
        let px = fmt_unit(210_000.0, Unit::Pixel);
        assert!(px == "793,7" || px == "793.7", "{px}");
        let back = units::parse_length("793.7", Unit::Pixel, None).unwrap();
        assert!((units::from_um(back, Unit::Millimeter) - 210.0).abs() < 0.01);
    }

    /// Aceite: buscar "A5" mostra o cartão A5, e o selo diz "Cabe 2× em A4".
    #[test]
    fn search_and_fit_badge() {
        let mut d = NewDocDialog {
            search: "A5".into(),
            ..Default::default()
        };
        let names: Vec<&str> = d.visible().iter().map(|i| PRESETS[*i].name).collect();
        assert!(names.contains(&"A5"), "{names:?}");
        let a5 = PRESETS.iter().position(|p| p.name == "A5").unwrap();
        d.choose(a5);
        assert!(d.fit_badge().contains("2×"), "{}", d.fit_badge());
        assert!(d.fit_badge().contains("A4"));
        // Categoria filtra.
        d.search.clear();
        d.filter = Filter::Cat(Category::Social);
        assert!(
            d.visible()
                .iter()
                .all(|i| PRESETS[*i].category == Category::Social)
        );
    }

    /// O documento criado tem as medidas, sangria, resolução e cor escolhidas.
    #[test]
    fn build_uses_choices() {
        let mut d = NewDocDialog::default();
        let post = PRESETS
            .iter()
            .position(|p| p.name == "Post quadrado")
            .unwrap();
        d.choose(post);
        d.name = "Campanha".into();
        let doc = d.build();
        let p = &doc.doc().data.pages[0];
        assert!((p.width - p.height).abs() < 1e-6);
        let s = &doc.doc().data.settings;
        assert_eq!(s.bleed, 0.0, "telas sem sangria");
        assert_eq!(s.resolution, 96.0);
        assert_eq!(
            s.extra.get("colorModel").and_then(|v| v.as_str()),
            Some("rgb")
        );
        assert_eq!(doc.doc().metadata.title.as_deref(), Some("Campanha"));
        // Impressão: CMYK e 300 ppp; sangria é opcional e começa desligada.
        let d = NewDocDialog::default();
        let doc = d.build();
        let s = &doc.doc().data.settings;
        assert_eq!((s.bleed, s.resolution), (0.0, 300.0));
        assert_eq!(
            s.extra.get("colorModel").and_then(|v| v.as_str()),
            Some("cmyk")
        );
    }

    /// A janela cabe numa tela de 1366 × 768 (DESIGN-SYSTEM 4.4).
    #[test]
    fn dialog_fits_on_screen() {
        let ctx = crate::theme::test_ctx();
        let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(1366.0, 768.0));
        let mut d = NewDocDialog::default();
        for _ in 0..3 {
            let input = egui::RawInput {
                screen_rect: Some(screen),
                ..Default::default()
            };
            let mut out = ctx.run_ui(input, |ui| {
                let _ = d.show(ui.ctx());
            });
            out.textures_delta.clear();
        }
        assert!(d.rect.is_positive(), "{:?}", d.rect);
        assert!(screen.contains_rect(d.rect), "fora da tela: {:?}", d.rect);
    }
}
