//! File > New: the New Document window. On the left a library of page
//! formats with a search field and category filters, shown as cards with
//! a proportional sheet; on the right a preview of the page (with its
//! bleed), how many times it fits on an A4 and an A3 sheet, and the
//! measurements: name, unit, width, height (unit inside the field, sums
//! and other units accepted), orientation, bleed, resolution and colour
//! mode. "Create document" (Ctrl+Enter) opens the drawing in a new tab;
//! Cancel or Esc closes the window. "/" puts the cursor in the search.
//!
//! The drawing is made by [`NewDocSettings::build`], the same path File >
//! New takes without a window, so pages and layers get their localised
//! names and the settings are remembered for the next drawing.

use crate::app::{App, Units};
use crate::i18n::{tr, trf};
use crate::measure_field;
use crate::new_document::{NewDocSettings, PresetValues, CUSTOM};
use egui::{Pos2, Rect, Sense, Vec2};
use graphdraw_core::page_presets::{self, Category, PagePreset, PRESETS};
use graphdraw_core::Document;

/// The window's look: spacing, colours, radii and type sizes.
mod tokens {
    use egui::{Color32, FontId, Stroke};

    pub const SPACE_050: f32 = 2.0;
    pub const SPACE_100: f32 = 4.0;
    pub const SPACE_200: f32 = 8.0;
    pub const SPACE_300: f32 = 12.0;
    pub const SPACE_400: f32 = 16.0;
    pub const SPACE_600: f32 = 24.0;

    pub const RADIUS_NONE: u8 = 0;
    pub const RADIUS_XS: u8 = 2;
    pub const RADIUS_MD: u8 = 6;
    pub const RADIUS_LG: u8 = 10;
    pub const RADIUS_FULL: u8 = u8::MAX;

    pub const PAGE_PAPER: Color32 = Color32::WHITE;
    pub const FG_ON_ACCENT: Color32 = Color32::WHITE;
    pub const ACCENT_PRIMARY: Color32 = Color32::from_rgb(0x63, 0x66, 0xF1);
    pub const ACCENT_SUCCESS: Color32 = Color32::from_rgb(0x10, 0xB9, 0x81);
    pub const BORDER_FOCUS: Color32 = Color32::from_rgb(0x63, 0x66, 0xF1);

    pub const BORDER_WIDTH: f32 = 1.0;
    pub const FOCUS_WIDTH: f32 = 1.5;
    pub const GUIDE_WIDTH: f32 = 1.0;
    pub const BLEED_DASH: f32 = 4.0;
    pub const BLEED_GAP: f32 = 3.0;

    pub const BUTTON_DEFAULT: f32 = 28.0;
    pub const BUTTON_PROMINENT: f32 = 36.0;
    pub const BUTTON_MIN_WIDTH: f32 = 80.0;
    /// A window takes at most this share of the screen's height.
    pub const MODAL_MAX_HEIGHT: f32 = 0.75;

    pub const TYPE_DISPLAY: f32 = 18.0;
    pub const TYPE_BODY: f32 = 12.0;
    pub const TYPE_NUMERIC: f32 = 11.0;
    pub const TYPE_MICRO: f32 = 10.0;

    /// Surfaces, borders and text of a theme.
    pub struct Palette {
        pub bg_canvas: Color32,
        pub bg_surface_1: Color32,
        pub bg_surface_2: Color32,
        pub bg_surface_hover: Color32,
        pub bg_surface_toggled: Color32,
        pub border_subtle: Color32,
        pub border_default: Color32,
        pub fg_primary: Color32,
        pub fg_strong: Color32,
        pub fg_muted: Color32,
        pub shadow_page_alpha: u8,
        pub shadow_popover_alpha: u8,
    }

    pub const DARK: Palette = Palette {
        bg_canvas: Color32::from_rgb(0x10, 0x10, 0x12),
        bg_surface_1: Color32::from_rgb(0x1E, 0x1E, 0x23),
        bg_surface_2: Color32::from_rgb(0x27, 0x27, 0x2E),
        bg_surface_hover: Color32::from_rgb(0x32, 0x32, 0x3B),
        bg_surface_toggled: Color32::from_rgb(0x2A, 0x2D, 0x4A),
        border_subtle: Color32::from_rgb(0x26, 0x26, 0x2E),
        border_default: Color32::from_rgb(0x36, 0x36, 0x42),
        fg_primary: Color32::from_rgb(0xE2, 0xE2, 0xE8),
        fg_strong: Color32::from_rgb(0xF4, 0xF4, 0xF6),
        fg_muted: Color32::from_rgb(0xA0, 0xA0, 0xAB),
        shadow_page_alpha: 89,
        shadow_popover_alpha: 128,
    };

    pub const LIGHT: Palette = Palette {
        bg_canvas: Color32::from_rgb(0xD9, 0xD9, 0xDF),
        bg_surface_1: Color32::from_rgb(0xF6, 0xF6, 0xF8),
        bg_surface_2: Color32::from_rgb(0xFF, 0xFF, 0xFF),
        bg_surface_hover: Color32::from_rgb(0xE6, 0xE6, 0xEC),
        bg_surface_toggled: Color32::from_rgb(0xE2, 0xE3, 0xFC),
        border_subtle: Color32::from_rgb(0xDE, 0xDE, 0xE5),
        border_default: Color32::from_rgb(0xC6, 0xC6, 0xD0),
        fg_primary: Color32::from_rgb(0x26, 0x26, 0x2E),
        fg_strong: Color32::from_rgb(0x10, 0x10, 0x12),
        fg_muted: Color32::from_rgb(0x62, 0x62, 0x6D),
        shadow_page_alpha: 46,
        shadow_popover_alpha: 51,
    };

    pub fn palette(ui: &egui::Ui) -> &'static Palette {
        palette_for(ui.visuals())
    }

    pub fn palette_for(v: &egui::Visuals) -> &'static Palette {
        if v.dark_mode {
            &DARK
        } else {
            &LIGHT
        }
    }

    impl Palette {
        /// Offset 0/8 px, blur 30 px.
        pub fn shadow_page(&self) -> egui::epaint::Shadow {
            egui::epaint::Shadow {
                offset: [0, 8],
                blur: 30,
                spread: 0,
                color: Color32::from_black_alpha(self.shadow_page_alpha),
            }
        }

        /// Offset 0/10 px, blur 25 px.
        pub fn shadow_popover(&self) -> egui::epaint::Shadow {
            egui::epaint::Shadow {
                offset: [0, 10],
                blur: 25,
                spread: 0,
                color: Color32::from_black_alpha(self.shadow_popover_alpha),
            }
        }
    }

    pub fn border_focus() -> Stroke {
        Stroke::new(FOCUS_WIDTH, BORDER_FOCUS)
    }

    pub fn font_display() -> FontId {
        crate::theme::bold(TYPE_DISPLAY)
    }

    pub fn font_body() -> FontId {
        FontId::proportional(TYPE_BODY)
    }

    pub fn font_numeric() -> FontId {
        FontId::monospace(TYPE_NUMERIC)
    }

    pub fn font_micro() -> FontId {
        FontId::proportional(TYPE_MICRO)
    }
}

use tokens as t;

/// The library's filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Filter {
    All,
    Cat(Category),
    Custom,
}

/// The units offered in the window.
const UNITS: [Units; 5] = [
    Units::Millimeters,
    Units::Centimeters,
    Units::Inches,
    Units::Pixels,
    Units::Points,
];

/// The open window.
#[derive(Debug, Clone, PartialEq)]
pub struct NewDocWindow {
    pub search: String,
    pub filter: Filter,
    /// The chosen format (an index in [`PRESETS`]).
    pub preset: Option<usize>,
    /// Page size, mm.
    pub width: f64,
    pub height: f64,
    pub unit: Units,
    /// Bleed, mm (0: none).
    pub bleed: f64,
    pub dpi: f64,
    pub rgb: bool,
    pub name: String,
    /// The name the drawing gets when none is typed.
    pub untitled: String,
    focus_search: bool,
    /// The area taken in the last frame (layout tests).
    pub(crate) rect: Rect,
}

/// What a frame of the window ended with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Open,
    Cancel,
    Create,
}

/// A format's name in the interface language.
pub fn preset_label(p: &PagePreset) -> String {
    tr(&format!("paper.{}", p.id))
}

fn category_label(c: Category) -> String {
    tr(match c {
        Category::Print => "newdoc.cat_print",
        Category::Screen => "newdoc.cat_screen",
        Category::Social => "newdoc.cat_social",
    })
}

/// A format's size as its card shows it: "210 × 297 mm", "1920 × 1080 px".
fn preset_dims(p: &PagePreset) -> String {
    let unit = if p.pixels {
        Units::Pixels
    } else {
        Units::Millimeters
    };
    format!(
        "{} × {} {}",
        measure_field::localize(measure_field::unit_value_text(p.size.width, unit, p.dpi)),
        measure_field::localize(measure_field::unit_value_text(p.size.height, unit, p.dpi)),
        unit.id()
    )
}

fn preset_index(id: &str) -> Option<usize> {
    PRESETS.iter().position(|p| p.id == id)
}

impl Default for NewDocWindow {
    fn default() -> Self {
        Self::open(String::new())
    }
}

impl NewDocWindow {
    /// The window as it opens: A4, millimetres, 300 dpi, CMYK.
    pub fn open(untitled: String) -> Self {
        let mut w = NewDocWindow {
            search: String::new(),
            filter: Filter::All,
            preset: None,
            width: 210.0,
            height: 297.0,
            unit: Units::Millimeters,
            bleed: 0.0,
            dpi: 300.0,
            rgb: false,
            name: String::new(),
            untitled,
            focus_search: true,
            rect: Rect::NOTHING,
        };
        if let Some(i) = preset_index("a4") {
            w.choose(i);
        }
        w
    }

    /// Take format `i`: its size, resolution, colour mode and unit.
    pub fn choose(&mut self, i: usize) {
        let Some(p) = PRESETS.get(i) else {
            return;
        };
        self.preset = Some(i);
        self.width = p.size.width;
        self.height = p.size.height;
        self.dpi = p.dpi;
        self.rgb = p.rgb;
        self.unit = if p.pixels {
            Units::Pixels
        } else {
            Units::Millimeters
        };
    }

    /// The formats shown with the current search and filter.
    pub fn visible(&self) -> Vec<usize> {
        if self.filter == Filter::Custom {
            return Vec::new();
        }
        let found = page_presets::search_named(&self.search, preset_label);
        PRESETS
            .iter()
            .enumerate()
            .filter(|(_, p)| found.iter().any(|f| f.id == p.id))
            .filter(|(_, p)| match self.filter {
                Filter::Cat(c) => p.category == c,
                _ => true,
            })
            .map(|(i, _)| i)
            .collect()
    }

    /// How many times the page fits on an A4 and an A3 sheet.
    pub fn fit_badge(&self) -> String {
        let sheet = |id: &str| page_presets::by_id(id).map(|p| p.size);
        let (Some(a4), Some(a3)) = (sheet("a4"), sheet("a3")) else {
            return String::new();
        };
        let n4 = page_presets::fit_count(a4.width, a4.height, self.width, self.height);
        let n3 = page_presets::fit_count(a3.width, a3.height, self.width, self.height);
        match (n4, n3) {
            (0, 0) => tr("newdoc.fits_none"),
            (0, n) => trf("newdoc.fits_a3", &[("n", &n.to_string())]),
            (n, m) => trf(
                "newdoc.fits_a4_a3",
                &[("n", &n.to_string()), ("m", &m.to_string())],
            ),
        }
    }

    /// The settings for File > New with the window's choices, on top of
    /// `base` (the colour profiles and the saved presets stay).
    pub fn settings(&self, base: &NewDocSettings) -> NewDocSettings {
        let mut s = base.clone();
        s.pages = 1;
        s.values = PresetValues {
            rgb: self.rgb,
            page_size: self
                .preset
                .and_then(|i| PRESETS.get(i))
                .map(|p| p.name.to_string())
                .unwrap_or_else(|| CUSTOM.into()),
            width_mm: self.width,
            height_mm: self.height,
            units: self.unit.id().into(),
            dpi: self.dpi,
        };
        s.refresh_preset();
        s
    }

    /// The name the drawing gets.
    pub fn title(&self) -> String {
        let name = self.name.trim();
        if name.is_empty() {
            self.untitled.clone()
        } else {
            name.to_string()
        }
    }

    /// The drawing the window stands for.
    pub fn build(&self, base: &NewDocSettings) -> Document {
        let mut doc = self.settings(base).build(self.title());
        doc.metadata.bleed = self.bleed.max(0.0);
        doc
    }

    /// One frame of the window.
    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        let mut out = Outcome::Open;
        // "/" puts the cursor in the search (outside text fields).
        if !ctx.egui_wants_keyboard_input() && ctx.input(|i| i.key_pressed(egui::Key::Slash)) {
            self.focus_search = true;
        }
        let screen = ctx.content_rect();
        let max_h = (screen.height() * t::MODAL_MAX_HEIGHT).max(360.0);
        let pal = t::palette_for(&ctx.global_style().visuals);
        let frame = egui::Frame::window(&ctx.global_style())
            .fill(pal.bg_surface_1)
            .stroke(egui::Stroke::new(t::BORDER_WIDTH, pal.border_default))
            .corner_radius(t::RADIUS_LG)
            .shadow(pal.shadow_popover())
            .inner_margin(t::SPACE_600 as i8);
        let resp = egui::Modal::new(egui::Id::new("new_document_window"))
            .frame(frame)
            .show(ctx, |ui| {
                ui.set_width(860.0_f32.min(screen.width() - 2.0 * t::SPACE_600));
                ui.spacing_mut().item_spacing = Vec2::new(t::SPACE_200, t::SPACE_100);
                ui.label(egui::RichText::new(tr("newdoc.window_title")).font(t::font_display()));
                ui.add_space(t::SPACE_300);
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| {
                        ui.set_width(520.0);
                        self.library(ui, max_h - 140.0);
                    });
                    ui.add_space(t::SPACE_400);
                    ui.vertical(|ui| {
                        ui.set_width(300.0);
                        self.inspector(ui);
                    });
                });
                ui.add_space(t::SPACE_300);
                let size = Vec2::new(ui.available_width(), t::BUTTON_DEFAULT);
                ui.allocate_ui_with_layout(
                    size,
                    egui::Layout::right_to_left(egui::Align::Center),
                    |ui| {
                        let create = egui::Button::new(
                            egui::RichText::new(tr("newdoc.create")).color(t::FG_ON_ACCENT),
                        )
                        .fill(t::ACCENT_PRIMARY)
                        .stroke(egui::Stroke::NONE)
                        .min_size(Vec2::new(t::BUTTON_MIN_WIDTH, t::BUTTON_PROMINENT));
                        if ui.add(create).clicked()
                            || ui.input(|i| i.key_pressed(egui::Key::Enter) && i.modifiers.command)
                        {
                            out = Outcome::Create;
                        }
                        let cancel = egui::Button::new(tr("dialog.cancel"))
                            .min_size(Vec2::new(t::BUTTON_MIN_WIDTH, t::BUTTON_DEFAULT));
                        if ui.add(cancel).clicked()
                            || ui.input(|i| i.key_pressed(egui::Key::Escape))
                        {
                            out = Outcome::Cancel;
                        }
                    },
                );
            });
        self.rect = resp.response.rect;
        out
    }

    fn library(&mut self, ui: &mut egui::Ui, height: f32) {
        let pal = t::palette(ui);
        let r = ui.add(
            egui::TextEdit::singleline(&mut self.search)
                .hint_text(tr("newdoc.search_hint"))
                .desired_width(f32::INFINITY),
        );
        if self.focus_search {
            r.request_focus();
            self.focus_search = false;
        }
        ui.add_space(t::SPACE_200);
        ui.horizontal_wrapped(|ui| {
            for (f, label) in [
                (Filter::All, tr("newdoc.filter_all")),
                (Filter::Cat(Category::Print), category_label(Category::Print)),
                (Filter::Cat(Category::Screen), category_label(Category::Screen)),
                (Filter::Cat(Category::Social), category_label(Category::Social)),
                (Filter::Custom, tr("newdoc.preset_custom")),
            ] {
                if ui.selectable_label(self.filter == f, label).clicked() {
                    self.filter = f;
                    if f == Filter::Custom {
                        self.preset = None;
                    }
                }
            }
        });
        ui.add_space(t::SPACE_200);
        let list = self.visible();
        egui::ScrollArea::vertical()
            .id_salt("new_document_library")
            .max_height(height.max(160.0))
            .auto_shrink([false, true])
            .show(ui, |ui| {
                if self.filter == Filter::Custom {
                    ui.label(egui::RichText::new(tr("newdoc.custom_hint")).color(pal.fg_muted));
                    return;
                }
                if list.is_empty() {
                    ui.label(egui::RichText::new(tr("newdoc.no_format")).color(pal.fg_muted));
                }
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::splat(t::SPACE_200);
                    for i in list {
                        if self.card(ui, i) {
                            self.choose(i);
                        }
                    }
                });
            });
    }

    /// A format's card with its proportional sheet; true when clicked.
    fn card(&self, ui: &mut egui::Ui, i: usize) -> bool {
        let Some(p) = PRESETS.get(i) else {
            return false;
        };
        let pal = t::palette(ui);
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(122.0, 132.0), Sense::click());
        let selected = self.preset == Some(i);
        let fill = if selected {
            pal.bg_surface_toggled
        } else if resp.hovered() {
            pal.bg_surface_hover
        } else {
            pal.bg_surface_2
        };
        let painter = ui.painter();
        painter.rect_filled(rect, t::RADIUS_MD, fill);
        painter.rect_stroke(
            rect,
            t::RADIUS_MD,
            if selected {
                t::border_focus()
            } else {
                egui::Stroke::new(t::BORDER_WIDTH, pal.border_default)
            },
            egui::StrokeKind::Inside,
        );
        // The sheet, to scale in a 64 px box.
        let k = 64.0 / p.size.width.max(p.size.height).max(1e-6) as f32;
        let sheet = Vec2::new(p.size.width as f32 * k, p.size.height as f32 * k);
        let center = Pos2::new(rect.center().x, rect.top() + t::SPACE_300 + 32.0);
        let sr = Rect::from_center_size(center, sheet);
        painter.rect_filled(sr, t::RADIUS_XS, t::PAGE_PAPER);
        painter.rect_stroke(
            sr,
            t::RADIUS_XS,
            egui::Stroke::new(t::BORDER_WIDTH, pal.border_subtle),
            egui::StrokeKind::Outside,
        );
        let mut y = rect.top() + t::SPACE_300 + 64.0 + t::SPACE_200;
        let mode = if p.rgb { "RGB" } else { "CMYK" };
        for (text, font, color) in [
            (preset_label(p), t::font_body(), pal.fg_strong),
            (preset_dims(p), t::font_numeric(), pal.fg_primary),
            (
                format!("{:.0} {} · {mode}", p.dpi, tr("newdoc.dpi")),
                t::font_micro(),
                pal.fg_muted,
            ),
        ] {
            let g = painter.layout(text, font, color, rect.width() - 2.0 * t::SPACE_200);
            let h = g.size().y;
            painter.galley(Pos2::new(rect.center().x - g.size().x / 2.0, y), g, color);
            y += h + t::SPACE_050;
        }
        resp.on_hover_text(format!("{} · {}", preset_label(p), preset_dims(p)))
            .clicked()
    }

    fn inspector(&mut self, ui: &mut egui::Ui) {
        let pal = t::palette(ui);
        // The page to scale, with its bleed.
        let (area, _) = ui.allocate_exact_size(Vec2::new(300.0, 170.0), Sense::hover());
        ui.painter().rect_filled(area, t::RADIUS_MD, pal.bg_canvas);
        let total_w = (self.width + 2.0 * self.bleed).max(1e-6);
        let total_h = (self.height + 2.0 * self.bleed).max(1e-6);
        let k = ((area.width() - 2.0 * t::SPACE_400) as f64 / total_w)
            .min((area.height() - 2.0 * t::SPACE_400) as f64 / total_h);
        let sheet = Rect::from_center_size(
            area.center(),
            Vec2::new((self.width * k) as f32, (self.height * k) as f32),
        );
        ui.painter()
            .add(pal.shadow_page().as_shape(sheet, t::RADIUS_NONE));
        ui.painter().rect_filled(sheet, t::RADIUS_NONE, t::PAGE_PAPER);
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
                egui::Stroke::new(t::GUIDE_WIDTH, t::ACCENT_SUCCESS),
                t::BLEED_DASH,
                t::BLEED_GAP,
            ));
        }
        ui.add_space(t::SPACE_200);
        // How many fit on a sheet.
        let g = ui
            .painter()
            .layout_no_wrap(self.fit_badge(), t::font_micro(), t::FG_ON_ACCENT);
        let (br, _) = ui.allocate_exact_size(
            g.size() + Vec2::new(2.0 * t::SPACE_200, 2.0 * t::SPACE_100),
            Sense::hover(),
        );
        ui.painter()
            .rect_filled(br, t::RADIUS_FULL, t::ACCENT_SUCCESS);
        ui.painter().galley(
            br.min + Vec2::new(t::SPACE_200, t::SPACE_100),
            g,
            t::FG_ON_ACCENT,
        );
        ui.add_space(t::SPACE_300);
        egui::Grid::new("new_document_measures")
            .num_columns(2)
            .spacing(Vec2::new(t::SPACE_200, t::SPACE_200))
            .show(ui, |ui| {
                ui.label(tr("newdoc.name"));
                ui.add(
                    egui::TextEdit::singleline(&mut self.name)
                        .hint_text(self.untitled.clone())
                        .desired_width(180.0),
                );
                ui.end_row();
                ui.label(tr("newdoc.unit"));
                egui::ComboBox::from_id_salt("new_document_unit")
                    .selected_text(self.unit.id())
                    .width(80.0)
                    .show_ui(ui, |ui| {
                        for u in UNITS {
                            ui.selectable_value(&mut self.unit, u, u.id());
                        }
                    });
                ui.end_row();
                ui.label(tr("newdoc.width_label"));
                if let Some(v) =
                    measure_field::length_field(ui, "width", self.width, self.unit, self.dpi, 110.0)
                {
                    self.width = v.max(1.0);
                    self.preset = None;
                }
                ui.end_row();
                ui.label(tr("newdoc.height_label"));
                if let Some(v) = measure_field::length_field(
                    ui,
                    "height",
                    self.height,
                    self.unit,
                    self.dpi,
                    110.0,
                ) {
                    self.height = v.max(1.0);
                    self.preset = None;
                }
                ui.end_row();
                ui.label(tr("newdoc.orientation_label"));
                ui.horizontal(|ui| {
                    let portrait = self.height >= self.width;
                    for (landscape, tip) in [(false, "dialog.portrait"), (true, "dialog.landscape")]
                    {
                        let (r, _) = ui.allocate_exact_size(
                            Vec2::splat(crate::ui::chrome::FIELD_H),
                            Sense::hover(),
                        );
                        let selected = landscape != portrait;
                        if crate::new_document::orientation_button(ui, r, landscape, selected)
                            .on_hover_text(tr(tip))
                            .clicked()
                            && !selected
                        {
                            std::mem::swap(&mut self.width, &mut self.height);
                        }
                    }
                });
                ui.end_row();
                ui.label(tr("newdoc.bleed"));
                ui.horizontal(|ui| {
                    let mut on = self.bleed > 0.0;
                    if ui.checkbox(&mut on, "").changed() {
                        self.bleed = if on { 3.0 } else { 0.0 };
                    }
                    if on {
                        if let Some(v) = measure_field::length_field(
                            ui,
                            "bleed",
                            self.bleed,
                            Units::Millimeters,
                            self.dpi,
                            110.0,
                        ) {
                            self.bleed = v.max(0.0);
                        }
                    } else {
                        ui.label(
                            egui::RichText::new(tr("newdoc.no_bleed")).color(pal.fg_muted),
                        );
                    }
                });
                ui.end_row();
                ui.label(tr("newdoc.resolution_label"));
                let suffix = format!(" {}", tr("newdoc.dpi"));
                if let Some(v) = measure_field::number_field(ui, "dpi", self.dpi, 0, &suffix, 110.0)
                {
                    self.dpi = v.clamp(72.0, 2400.0);
                }
                ui.end_row();
                ui.label(tr("newdoc.color"));
                ui.horizontal(|ui| {
                    for (rgb, label) in [(false, "CMYK"), (true, "RGB")] {
                        if ui.selectable_label(self.rgb == rgb, label).clicked() {
                            self.rgb = rgb;
                        }
                    }
                });
                ui.end_row();
            });
        ui.label(
            egui::RichText::new(tr("newdoc.color_note"))
                .font(t::font_micro())
                .color(pal.fg_muted),
        );
    }
}

/// Draw the window; `close` is set when it should go away.
pub fn dialog(app: &mut App, ctx: &egui::Context, st: &mut NewDocWindow, close: &mut bool) {
    match st.show(ctx) {
        Outcome::Open => {}
        Outcome::Cancel => *close = true,
        Outcome::Create => {
            *close = true;
            create(app, st);
            app.save_settings();
        }
    }
}

/// Create document: open the drawing in a new tab, in the window's unit,
/// and keep the settings for File > New without the window.
pub fn create(app: &mut App, st: &NewDocWindow) {
    let settings = st.settings(&app.settings.new_document);
    let mut doc = st.build(&app.settings.new_document);
    app.settings.document_defaults.apply(&mut doc);
    // The name offered was the next Untitled-N: use it up.
    app.untitled_counter += 1;
    app.open_document(doc, None);
    app.units = st.unit;
    app.settings.new_document = settings;
}

#[cfg(test)]
mod tests {
    use super::*;
    use graphdraw_core::document::PrimaryColorMode;

    /// Searching "A5" shows the A5 card, and A5 fits twice on A4.
    #[test]
    fn search_and_fit_badge() {
        let mut d = NewDocWindow {
            search: "A5".into(),
            ..Default::default()
        };
        let ids: Vec<&str> = d.visible().iter().map(|i| PRESETS[*i].id).collect();
        assert!(ids.contains(&"a5"), "{ids:?}");
        d.choose(preset_index("a5").expect("a5"));
        assert!(d.fit_badge().contains("2×"), "{}", d.fit_badge());
        assert!(d.fit_badge().contains("A4"));
        // A category filters.
        d.search.clear();
        d.filter = Filter::Cat(Category::Social);
        let social = d.visible();
        assert!(!social.is_empty());
        assert!(social
            .iter()
            .all(|i| PRESETS[*i].category == Category::Social));
        // Custom shows no cards; nothing found shows none either.
        d.filter = Filter::Custom;
        assert!(d.visible().is_empty());
        d.filter = Filter::All;
        d.search = "no such format".into();
        assert!(d.visible().is_empty());
        // Bigger than A3.
        d.width = 1000.0;
        d.height = 2000.0;
        assert_eq!(d.fit_badge(), tr("newdoc.fits_none"));
    }

    /// The drawing has the size, bleed, resolution, colour mode and name
    /// chosen.
    #[test]
    fn build_uses_choices() {
        let base = NewDocSettings::default();
        let mut d = NewDocWindow::open("Untitled-1".into());
        d.choose(preset_index("square_post").expect("square post"));
        d.name = "Campaign".into();
        let doc = d.build(&base);
        let p = &doc.pages[0];
        assert!((p.size.width - p.size.height).abs() < 1e-9);
        assert!((p.size.width - 1080.0 * 25.4 / 96.0).abs() < 1e-9);
        assert_eq!(doc.metadata.bleed, 0.0, "screens have no bleed");
        assert_eq!(doc.metadata.resolution_dpi, 96.0);
        assert_eq!(doc.metadata.primary_color_mode, PrimaryColorMode::Rgb);
        assert_eq!(doc.title, "Campaign");
        assert_eq!(doc.pages.len(), 1);
        // Print: CMYK and 300 dpi; bleed is optional and starts off.
        let d = NewDocWindow::open("Untitled-2".into());
        let doc = d.build(&base);
        assert_eq!((doc.metadata.bleed, doc.metadata.resolution_dpi), (0.0, 300.0));
        assert_eq!(doc.metadata.primary_color_mode, PrimaryColorMode::Cmyk);
        assert_eq!(doc.title, "Untitled-2", "no name typed: the offered one");
        // Landscape, with a bleed.
        let mut d = NewDocWindow::open("U".into());
        std::mem::swap(&mut d.width, &mut d.height);
        d.bleed = 3.0;
        let doc = d.build(&base);
        assert!((doc.pages[0].size.width - 297.0).abs() < 1e-9);
        assert_eq!(doc.metadata.bleed, 3.0);
    }

    /// Create document opens a tab in the window's unit and remembers the
    /// settings for File > New without the window.
    #[test]
    fn create_opens_a_tab_and_remembers_the_settings() {
        let mut app = App::headless();
        let before = app.docs.len();
        let mut st = NewDocWindow::open(app.peek_untitled_name());
        st.choose(preset_index("letter").expect("letter"));
        st.unit = Units::Inches;
        st.bleed = 2.0;
        create(&mut app, &st);
        assert_eq!(app.docs.len(), before + 1);
        assert_eq!(app.units, Units::Inches);
        assert!((app.page_size().width - 215.9).abs() < 1e-9);
        assert_eq!(app.doc().metadata.bleed, 2.0);
        assert_eq!(app.settings.new_document.values.page_size, "Letter");
        assert_eq!(app.settings.new_document.values.units, "in");
        app.settings.show_new_document_dialog = false;
        app.request_new_document();
        assert_eq!(app.docs.len(), before + 2);
        assert!((app.page_size().height - 279.4).abs() < 1e-9);
    }

    /// The window fits on a 1366 × 768 screen.
    #[test]
    fn window_fits_on_screen() {
        let ctx = crate::theme::ui_context();
        let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(1366.0, 768.0));
        let mut d = NewDocWindow::default();
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
        assert!(screen.contains_rect(d.rect), "off screen: {:?}", d.rect);
    }

    /// Every format has its name in the language files.
    #[test]
    fn every_format_has_a_translated_name() {
        for p in PRESETS {
            assert!(
                crate::i18n::has_english(&format!("paper.{}", p.id)),
                "{}",
                p.id
            );
        }
    }

    /// 210 mm in pixels is 793.7 px at 96 dpi.
    #[test]
    fn unit_conversion_acceptance() {
        let px = measure_field::unit_value_text(210.0, Units::Pixels, 96.0);
        assert_eq!(px, "793.7");
        let all = preset_dims(page_presets::by_id("full_hd").expect("full hd"));
        assert!(all.starts_with("1920") && all.ends_with("px"), "{all}");
    }
}
