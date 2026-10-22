//! The floating Fill and Outline windows: Edit Fill (F11) and the
//! Outline Pen (F12). They float over the drawing and move by their title
//! bar, so the objects stay in sight while they are edited: the drawing
//! shows the new fill or outline on the objects at once (a preview; the
//! drawing itself changes only with OK, as one undo step, and Cancel, Esc
//! or the close button leave it as it was). With nothing selected they set
//! the properties new objects get (see `defaults.rs`).
//!
//! This module holds what both windows share: the window frame, the
//! colour picker (a hue ring around the saturation and brightness square,
//! the colour model's sliders and the hexadecimal code, the palettes with
//! a search, and the eyedropper that picks a colour anywhere in the
//! program window), the colour button that opens it in a pop-up, the icon
//! buttons, the transparency chequerboard and the canvas preview.

use crate::app::App;
use crate::i18n::tr;
use crate::theme::Tokens;
use crate::ui::fill_window::FillWindow;
use crate::ui::outline_window::OutlineWindow;
use crate::ui::svg_icons::{self, IconRef};
use egui::{
    Align2, Color32, Context, FontId, Frame, Margin, Pos2, Rect, Response, Sense, Stroke, Ui,
    Vec2,
};
use graphdraw_core::{Color, Command, Document, ShapeId};

/// Height of the title bar, as on the dialogs.
const TITLE_BAR: f32 = crate::ui::chrome::TITLE_BAR;
/// The ring of the hue wheel and the wheel's size.
const HUE_WHEEL: f32 = 168.0;
const HUE_RING: f32 = 16.0;
/// The colour picker's width.
pub const PICKER_WIDTH: f32 = 320.0;
/// Height of the palette grid.
const PICKER_GRID_HEIGHT: f32 = 150.0;
/// A palette swatch.
const SWATCH: f32 = 18.0;
/// The chequerboard behind colours that may be see-through.
const CHECKER_CELL: f32 = 6.0;
const CHECKER_LIGHT: Color32 = Color32::WHITE;
const CHECKER_DARK: Color32 = Color32::from_rgb(0xD9, 0xD9, 0xDF);
/// A cell of the eyedropper's magnifier.
const LOUPE_CELL: f32 = 9.0;
/// A marker on the wheel.
const HANDLE: f32 = 6.0;
/// An icon button: the size the dockers use.
pub const ICON_BUTTON: Vec2 = Vec2::new(28.0, 26.0);
/// The red of "no colour".
pub const NONE_RED: Color32 = Color32::from_rgb(0xE5, 0x39, 0x35);

/// The open windows, where they were drawn, and the drawing the canvas
/// shows while they are open.
#[derive(Default)]
pub struct StyleWindows {
    pub fill: Option<Box<FillWindow>>,
    pub outline: Option<Box<OutlineWindow>>,
    /// Where the windows were drawn last frame (screen points): the
    /// pointer over one of them gives it the keyboard.
    rects: Vec<Rect>,
    preview: Option<Preview>,
    serial: u64,
}

/// The drawing with the windows' changes applied, for the canvas.
struct Preview {
    /// The drawing's revision it was made from.
    base: u64,
    /// The windows' own revisions it shows.
    windows: (u64, u64),
    serial: u64,
    doc: Document,
}

impl StyleWindows {
    /// True while the pointer is over one of the windows: their keys
    /// (Delete takes a gradient node, Esc cancels, Enter is OK) are not
    /// the drawing's shortcuts then.
    pub fn owns_keyboard(&self, ctx: &Context) -> bool {
        let Some(p) = ctx.pointer_latest_pos() else {
            return false;
        };
        self.rects.iter().any(|r| r.contains(p))
    }

    /// The drawing the canvas shows instead of the active one, with the
    /// revision its rendering is cached under: the windows' changes on
    /// the objects they edit.
    pub fn canvas_doc(&self, revision: u64) -> Option<(&Document, u64)> {
        let p = self.preview.as_ref()?;
        (p.base == revision).then(|| {
            // Apart from every revision the drawing itself has.
            let key = (1u64 << 63) | (p.serial << 24) | (revision & 0xFF_FFFF);
            (&p.doc, key)
        })
    }

    /// Make the canvas preview again when a window changed or the drawing
    /// did; none while nothing is changed or the windows set the
    /// defaults.
    fn refresh_preview(&mut self, app: &App) {
        let fill = self.fill.as_ref().filter(|w| w.previews());
        let outline = self.outline.as_ref().filter(|w| w.previews());
        if fill.is_none() && outline.is_none() {
            self.preview = None;
            return;
        }
        let base = app.engine.revision();
        let windows = (
            fill.map_or(0, |w| w.revision()),
            outline.map_or(0, |w| w.revision()),
        );
        if let Some(p) = &self.preview {
            if p.base == base && p.windows == windows {
                return;
            }
        }
        let mut cmds = fill.map(|w| w.commands(app.doc())).unwrap_or_default();
        cmds.extend(outline.map(|w| w.commands(app.doc())).unwrap_or_default());
        let mut doc = app.doc().clone();
        {
            for c in &cmds {
                if let Err(e) = c.apply(&mut doc) {
                    log::warn!("fill/outline preview: {e}");
                }
            }
        }
        self.serial = self.serial.wrapping_add(1) & 0xFF_FFFF_FFFF;
        self.preview = Some(Preview {
            base,
            windows,
            serial: self.serial,
            doc,
        });
    }
}

/// How a window ended this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Still open, drawn at this rectangle.
    Open(Rect),
    Ok,
    Cancel,
}

/// Draw the open windows and keep the canvas preview current. Called once
/// a frame after the dialogs.
pub fn show(app: &mut App, ctx: &Context) {
    let mut sw = std::mem::take(&mut app.style_windows);
    sw.rects.clear();
    if let Some(mut w) = sw.fill.take() {
        match w.show(app, ctx) {
            Outcome::Open(r) => {
                sw.rects.push(r);
                sw.fill = Some(w);
            }
            Outcome::Ok => w.apply(app),
            Outcome::Cancel => {}
        }
    }
    if let Some(mut w) = sw.outline.take() {
        match w.show(app, ctx) {
            Outcome::Open(r) => {
                sw.rects.push(r);
                sw.outline = Some(w);
            }
            Outcome::Ok => w.apply(app),
            Outcome::Cancel => {}
        }
    }
    // A window opened while these were drawn (a button of the other one).
    let opened = std::mem::take(&mut app.style_windows);
    if opened.fill.is_some() {
        sw.fill = opened.fill;
    }
    if opened.outline.is_some() {
        sw.outline = opened.outline;
    }
    sw.refresh_preview(app);
    app.style_windows = sw;
}

/// Run `cmds` as one undo step: the first as any command, the others as
/// part of it.
pub fn run_as_one_step(app: &mut App, cmds: Vec<Command>) {
    let mut cmds = cmds.into_iter();
    let Some(first) = cmds.next() else {
        return;
    };
    let before = app.engine.revision();
    app.run(first);
    if app.engine.revision() == before {
        return;
    }
    for c in cmds {
        app.break_clone_links_for(&c);
        if let Err(e) = app.engine.amend(&c) {
            app.status = format!("{}: {e}", c.label());
            continue;
        }
        app.sync_effect_clones_for(&c);
    }
}

/// The selected objects a window edits: the selection, with groups
/// standing for their members (the fill and outline commands reach into
/// them). Empty with nothing selected.
pub fn targets(app: &App) -> Vec<ShapeId> {
    app.selection.clone()
}

/// The first object of the selection that has a fill and an outline of
/// its own: groups give their first member that does.
pub fn first_leaf(s: &graphdraw_core::Shape) -> &graphdraw_core::Shape {
    match &s.kind {
        graphdraw_core::ShapeKind::Group { children } => {
            children.first().map(first_leaf).unwrap_or(s)
        }
        // A clip frame's fill and outline are its frame's.
        graphdraw_core::ShapeKind::ClipFrame { frame, .. } => first_leaf(frame),
        _ => s,
    }
}

// ----- the window frame --------------------------------------------------------

/// A floating window in the dialogs' chrome (white title bar with the
/// close button, white edge, grey body) that moves by its title bar and
/// leaves the drawing usable. It opens at the right of the screen, over
/// the dockers, the first time. Returns what `add` returned, whether the
/// close button was clicked, and where the window is.
pub fn floating<R>(
    ctx: &Context,
    id: &str,
    title: &str,
    add: impl FnOnce(&mut Ui) -> R,
) -> (Option<R>, bool, Rect) {
    let screen = ctx.content_rect();
    let mut out = None;
    let mut closed = false;
    // Each window first opens a step down and left of the one before, so
    // two open together do not hide each other.
    let step = match id {
        "fill" => 0.0,
        "outline" => 1.0,
        _ => 2.0,
    } * 40.0;
    let resp = egui::Window::new(title)
        .id(egui::Id::new(("style_window", id)))
        .order(egui::Order::Middle)
        .title_bar(false)
        .collapsible(false)
        .resizable(false)
        .movable(true)
        .constrain(true)
        .pivot(Align2::RIGHT_TOP)
        .default_pos(Pos2::new(
            screen.right() - 24.0 - step,
            screen.top() + 96.0 + step,
        ))
        .frame(crate::ui::chrome::window_frame())
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing.y = 6.0;
            let top = ui.cursor().min;
            ui.add_space(TITLE_BAR);
            let inner = Frame::new()
                .fill(Tokens::panel())
                .inner_margin(Margin {
                    left: 14,
                    right: 14,
                    top: 10,
                    bottom: 10,
                })
                .outer_margin(Margin {
                    left: crate::ui::chrome::EDGE as i8,
                    right: crate::ui::chrome::EDGE as i8,
                    top: 0,
                    bottom: crate::ui::chrome::EDGE as i8,
                })
                .show(ui, |ui| {
                    crate::ui::chrome::form_style(ui);
                    add(ui)
                });
            out = Some(inner.inner);
            let bar = Rect::from_min_size(top, Vec2::new(ui.min_rect().width(), TITLE_BAR));
            // The bar moves the window (egui drags a window by any part
            // that does not take the pointer itself).
            if crate::ui::chrome::title_bar(ui, bar, title, id) {
                closed = true;
            }
        });
    let rect = resp.map_or(Rect::NOTHING, |r| r.response.rect);
    (out, closed, rect)
}

/// A section heading inside a window (the dialogs' heading with its line).
pub fn heading(ui: &mut Ui, text: &str) {
    crate::ui::chrome::section(ui, text);
}

/// What the footer's buttons asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Footer {
    Ok,
    Cancel,
}

/// The footer: OK (the default button) and Cancel on the right, Help
/// before them showing `help` in a pop-up. With the pointer over the
/// window, Enter is OK and Esc Cancel.
pub fn footer(ui: &mut Ui, help: &str, hovered: bool, left: impl FnOnce(&mut Ui)) -> Option<Footer> {
    let mut out = None;
    ui.horizontal(|ui| {
        left(ui);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            if ui
                .add_sized([100.0, 27.0], egui::Button::new(tr("dialog.cancel")))
                .clicked()
            {
                out = Some(Footer::Cancel);
            }
            let ok = ui
                .scope(|ui| {
                    let w = &mut ui.visuals_mut().widgets;
                    w.inactive.bg_stroke = Stroke::new(1.0, Tokens::ACCENT);
                    w.inactive.weak_bg_fill = Tokens::tool_hover();
                    ui.add_sized([100.0, 27.0], egui::Button::new(tr("dialog.ok")))
                })
                .inner;
            if ok.clicked() {
                out = Some(Footer::Ok);
            }
            let h = ui.add_sized([80.0, 27.0], egui::Button::new(tr("stylewin.help")));
            egui::Popup::menu(&h)
                .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                .show(|ui| {
                    ui.set_max_width(PICKER_WIDTH);
                    ui.label(help);
                });
        });
    });
    if out.is_none() && hovered && !egui::Popup::is_any_open(ui.ctx()) {
        let typing = ui.memory(|m| m.focused().is_some());
        ui.input(|i| {
            if i.key_pressed(egui::Key::Escape) {
                out = Some(Footer::Cancel);
            } else if i.key_pressed(egui::Key::Enter) && !typing {
                out = Some(Footer::Ok);
            }
        });
    }
    out
}

// ----- small controls ------------------------------------------------------------

/// The screen colour of `c`.
pub fn color32(c: Color) -> Color32 {
    let [r, g, b] = c.to_rgb8();
    Color32::from_rgb(r, g, b)
}

/// A 28 x 26 icon button with the picture of `key` (see `svg_icons`),
/// shaded while `selected`; dimmed and inert unless `enabled`.
pub fn icon_button(ui: &mut Ui, key: &str, tip: &str, selected: bool, enabled: bool) -> Response {
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, resp) = ui.allocate_exact_size(ICON_BUTTON, sense);
    if selected {
        ui.painter().rect_filled(rect, 2.0, Tokens::tool_active());
    } else if enabled && resp.hovered() {
        ui.painter().rect_filled(rect, 2.0, Tokens::tool_hover());
    }
    let color = if enabled {
        Tokens::icon()
    } else {
        Tokens::icon_off()
    };
    svg_icons::paint(
        ui.painter(),
        Rect::from_center_size(rect.center(), Vec2::splat(20.0)),
        IconRef::Key(key),
        color,
    );
    resp.on_hover_text(tip)
}

/// A colour swatch button (it opens the picker in a pop-up); framed in the
/// accent while `open` or under the pointer.
pub fn color_button(ui: &mut Ui, c: Color, open: bool) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(36.0, 22.0), Sense::click());
    let p = ui.painter();
    p.rect_filled(rect.shrink(3.0), 1.0, color32(c));
    let stroke = if open || resp.hovered() {
        Stroke::new(1.5, Tokens::ACCENT)
    } else {
        Stroke::new(1.0, Tokens::control_border())
    };
    p.rect_stroke(rect.shrink(0.5), 1.0, stroke, egui::StrokeKind::Inside);
    resp
}

/// A chequerboard under `rect`, showing what is see-through.
pub fn checker(p: &egui::Painter, rect: Rect) {
    let p = p.with_clip_rect(rect);
    let cols = (rect.width() / CHECKER_CELL).ceil() as i32;
    let rows = (rect.height() / CHECKER_CELL).ceil() as i32;
    for i in 0..cols {
        for j in 0..rows {
            let c = if (i + j) % 2 == 0 {
                CHECKER_LIGHT
            } else {
                CHECKER_DARK
            };
            p.rect_filled(
                Rect::from_min_size(
                    rect.min + Vec2::new(i as f32 * CHECKER_CELL, j as f32 * CHECKER_CELL),
                    Vec2::splat(CHECKER_CELL),
                ),
                0.0,
                c,
            );
        }
    }
}

/// "No fill": white paper crossed by a red diagonal.
pub fn none_mark(p: &egui::Painter, rect: Rect) {
    p.rect_filled(rect, 0.0, Color32::WHITE);
    p.line_segment(
        [rect.left_bottom(), rect.right_top()],
        Stroke::new(1.5, NONE_RED),
    );
}

/// A label in the dimmed text colour.
pub fn note(ui: &mut Ui, text: &str) {
    ui.label(egui::RichText::new(text).color(Tokens::text_dim()).size(11.0));
}

/// The angle dial: dragging turns the pointer (Shift: 15 degree steps).
/// True when the angle changed.
pub fn angle_dial(ui: &mut Ui, angle: &mut f64) -> bool {
    let d = 26.0;
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(d), Sense::click_and_drag());
    let c = rect.center();
    let r = d / 2.0 - 1.0;
    let p = ui.painter();
    p.circle(
        c,
        r,
        Tokens::field(),
        Stroke::new(1.0, Tokens::control_border()),
    );
    let a = angle.to_radians() as f32;
    let tip = c + Vec2::new(a.cos(), -a.sin()) * (r - 2.0);
    p.line_segment([c, tip], Stroke::new(1.5, Tokens::ACCENT));
    p.circle_filled(c, 2.0, Tokens::ACCENT);
    if resp.dragged() || resp.clicked() {
        if let Some(q) = resp.interact_pointer_pos() {
            let v = q - c;
            if v.length() > 2.0 {
                let mut deg = ((-v.y).atan2(v.x) as f64).to_degrees();
                if ui.input(|i| i.modifiers.shift) {
                    deg = (deg / 15.0).round() * 15.0;
                }
                *angle = deg.round();
                return true;
            }
        }
    }
    false
}

/// A colour dragged from the picker's palette (to the gradient ramp).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SwatchDrop(pub Color);

/// A small rendered picture kept as a texture while what it shows stays
/// the same.
pub struct Cached<K> {
    key: Option<K>,
    tex: Option<egui::TextureHandle>,
}

impl<K> Default for Cached<K> {
    fn default() -> Self {
        Cached {
            key: None,
            tex: None,
        }
    }
}

impl<K: PartialEq> Cached<K> {
    /// The texture for `key`, made with `make` (premultiplied RGBA and its
    /// size) when the key changed.
    pub fn get(
        &mut self,
        ctx: &Context,
        name: &str,
        key: K,
        make: impl FnOnce() -> Option<egui::ColorImage>,
    ) -> Option<egui::TextureId> {
        if self.key.as_ref() != Some(&key) || self.tex.is_none() {
            match make() {
                Some(img) => match &mut self.tex {
                    Some(t) => t.set(img, egui::TextureOptions::LINEAR),
                    None => {
                        self.tex =
                            Some(ctx.load_texture(name, img, egui::TextureOptions::LINEAR))
                    }
                },
                None => self.tex = None,
            }
            self.key = Some(key);
        }
        self.tex.as_ref().map(|t| t.id())
    }
}

/// A rendered pixmap as an egui image.
pub fn pixmap_image(pm: &tiny_skia::Pixmap) -> egui::ColorImage {
    egui::ColorImage::from_rgba_premultiplied(
        [pm.width() as usize, pm.height() as usize],
        pm.data(),
    )
}

// ----- the colour picker -------------------------------------------------------------

/// The palettes the picker offers: name and colours (name, colour).
pub type PaletteList = Vec<(String, Vec<(String, Color)>)>;

/// The drawing's palette and the palettes the program knows.
pub fn palettes_of(app: &App) -> PaletteList {
    let mut out: PaletteList = Vec::new();
    let doc = &app.doc().palette;
    if !doc.is_empty() {
        out.push((
            tr("stylewin.document_palette"),
            doc.iter().map(|c| (String::new(), *c)).collect(),
        ));
    }
    for p in &app.palettes {
        out.push((tr(p.name_key()), p.colors.clone()));
    }
    out
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum Tab {
    #[default]
    Models,
    Palettes,
}

/// The picker's tab, palette and search.
#[derive(Debug, Clone, Default)]
pub struct ColorPicker {
    tab: Tab,
    palette: usize,
    search: String,
}

/// What the picker gave this frame.
#[derive(Debug, Clone, Copy, Default)]
pub struct PickOut {
    pub color: Option<Color>,
    /// The eyedropper was asked for.
    pub eyedrop: bool,
}

/// The values of a colour in its model, for tips ("C 100 M 50 Y 0 K 0").
pub fn color_text(c: Color) -> String {
    let pct = |v: f32| (v * 100.0).round();
    let vals = match c {
        Color::Cmyk { c, m, y, k } => format!(
            "C {} M {} Y {} K {}",
            pct(c),
            pct(m),
            pct(y),
            pct(k)
        ),
        Color::Rgb { r, g, b } => format!(
            "R {} G {} B {}",
            (r * 255.0).round(),
            (g * 255.0).round(),
            (b * 255.0).round()
        ),
        Color::Hsb { h, s, b } => format!("H {} S {} B {}", h.round(), pct(s), pct(b)),
        Color::Hsl { h, s, l } => format!("H {} S {} L {}", h.round(), pct(s), pct(l)),
        Color::Lab { l, a, b } => format!("L {} a {} b {}", l.round(), a.round(), b.round()),
        Color::Gray { v } => format!("K {}", pct(1.0 - v)),
        Color::Yiq { y, i, q } => format!("Y {} I {} Q {}", pct(y), pct(i), pct(q)),
        Color::Registration | Color::Spot { .. } => String::new(),
    };
    if vals.is_empty() {
        c.to_hex()
    } else {
        format!("{vals} · {}", c.to_hex())
    }
}

/// The model a colour picked on the wheel or by its code comes back in:
/// the colour's own, process CMYK for spot and registration colours.
fn model_of(c: Color) -> &'static str {
    match c {
        Color::Spot { .. } | Color::Registration => "CMYK",
        _ => c.model_name(),
    }
}

impl ColorPicker {
    /// Draw the picker for `color`; `id` tells pickers in one window apart.
    pub fn show(&mut self, ui: &mut Ui, id: &str, color: Color, palettes: &PaletteList) -> PickOut {
        let mut out = PickOut::default();
        ui.horizontal(|ui| {
            for (t, key) in [
                (Tab::Models, "stylewin.models"),
                (Tab::Palettes, "stylewin.palettes"),
            ] {
                if ui.selectable_label(self.tab == t, tr(key)).clicked() {
                    self.tab = t;
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if icon_button(
                    ui,
                    "stylewin.eyedropper_tip",
                    &tr("stylewin.eyedropper_tip"),
                    false,
                    true,
                )
                .clicked()
                {
                    out.eyedrop = true;
                }
            });
        });
        match self.tab {
            Tab::Models => {
                let mut cur = color;
                if let Some(c) = hue_wheel(ui, ui.make_persistent_id(("hue_wheel", id)), cur) {
                    cur = c;
                    out.color = Some(c);
                }
                if let Some(c) = channels(ui, id, cur) {
                    out.color = Some(c);
                }
            }
            Tab::Palettes => {
                if let Some(c) = self.palettes_tab(ui, id, palettes) {
                    out.color = Some(c);
                }
            }
        }
        out
    }

    fn palettes_tab(&mut self, ui: &mut Ui, id: &str, palettes: &PaletteList) -> Option<Color> {
        let mut out = None;
        if palettes.is_empty() {
            note(ui, &tr("stylewin.no_palettes"));
            return None;
        }
        self.palette = self.palette.min(palettes.len() - 1);
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt(("picker_palette", id))
                .selected_text(&palettes[self.palette].0)
                .width(180.0)
                .show_ui(ui, |ui| {
                    for (i, (name, _)) in palettes.iter().enumerate() {
                        ui.selectable_value(&mut self.palette, i, name);
                    }
                });
            ui.add(
                egui::TextEdit::singleline(&mut self.search)
                    .hint_text(tr("stylewin.search_color"))
                    .desired_width(110.0),
            );
        });
        let q = self.search.trim().to_lowercase();
        let colors: Vec<&(String, Color)> = palettes[self.palette]
            .1
            .iter()
            .filter(|(n, c)| {
                q.is_empty() || n.to_lowercase().contains(&q) || c.to_hex().contains(&q)
            })
            .collect();
        egui::ScrollArea::vertical()
            .id_salt(("picker_grid", id))
            .max_height(PICKER_GRID_HEIGHT)
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::splat(2.0);
                    for (name, c) in &colors {
                        let tip = if name.is_empty() {
                            color_text(*c)
                        } else {
                            format!("{name}\n{}", color_text(*c))
                        };
                        let (rect, resp) =
                            ui.allocate_exact_size(Vec2::splat(SWATCH), Sense::click_and_drag());
                        ui.painter().rect_filled(rect, 0.0, color32(*c));
                        let line = if resp.hovered() {
                            Tokens::ACCENT
                        } else {
                            Tokens::border()
                        };
                        ui.painter().rect_stroke(
                            rect,
                            0.0,
                            Stroke::new(1.0, line),
                            egui::StrokeKind::Inside,
                        );
                        resp.dnd_set_drag_payload(SwatchDrop(*c));
                        if resp.on_hover_text(tip).clicked() {
                            out = Some(*c);
                        }
                    }
                });
                if colors.is_empty() {
                    note(ui, &tr("stylewin.nothing_found"));
                }
            });
        out
    }
}

/// The colour model chosen in a list, the model's sliders, and the
/// hexadecimal code (the model stays the chosen one). The new colour when
/// changed.
fn channels(ui: &mut Ui, id: &str, color: Color) -> Option<Color> {
    let mut c = color;
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label(tr("docker.model"));
        let model = model_of(c);
        egui::ComboBox::from_id_salt(("picker_model", id))
            .selected_text(model)
            .width(100.0)
            .show_ui(ui, |ui| {
                for m in crate::ui::color_docker::MODELS {
                    if ui.selectable_label(model == m, m).clicked() && model != m {
                        c = c.convert_to(m);
                        changed = true;
                    }
                }
            });
        ui.label(tr("stylewin.hex"));
        let mut hex = c.to_hex();
        let r = ui.add(egui::TextEdit::singleline(&mut hex).desired_width(70.0));
        if r.lost_focus() && hex != c.to_hex() {
            if let Some(rgb) = Color::from_hex(&hex) {
                c = rgb.convert_to(model_of(c));
                changed = true;
            }
        }
    });
    ui.spacing_mut().slider_width = 200.0;
    if crate::ui::color_docker::sliders(ui, &mut c) {
        changed = true;
    }
    changed.then_some(c)
}

/// The hue ring with the saturation (across) and brightness (up) square
/// inside it. The hue is kept under `id` so greys do not lose it. The
/// colour comes back in the model of `color`.
pub fn hue_wheel(ui: &mut Ui, id: egui::Id, color: Color) -> Option<Color> {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(HUE_WHEEL), Sense::click_and_drag());
    let (h0, s, b) = color.to_hsb();
    let stored: Option<f64> = ui.data(|d| d.get_temp(id));
    // Without saturation the hue is undefined: the last one used stays.
    let h = if s < 1e-6 || b < 1e-6 {
        stored.unwrap_or(h0)
    } else {
        h0
    };
    let c = rect.center();
    let outer = HUE_WHEEL / 2.0;
    let inner = outer - HUE_RING;
    let half = (inner - 8.0) / std::f32::consts::SQRT_2;
    let sq = Rect::from_center_size(c, Vec2::splat(half * 2.0));
    let hsb = |h: f64, s: f64, v: f64| color32(Color::from_hsb(h, s, v));
    // The ring: the hue grows counter-clockwise from the right.
    let mut mesh = egui::Mesh::default();
    let n = 96;
    let at = |a: f32, r: f32| c + Vec2::new(a.cos() * r, -a.sin() * r);
    for k in 0..n {
        let (a0, a1) = (
            k as f32 / n as f32 * std::f32::consts::TAU,
            (k + 1) as f32 / n as f32 * std::f32::consts::TAU,
        );
        let (c0, c1) = (
            hsb(k as f64 / n as f64 * 360.0, 1.0, 1.0),
            hsb((k + 1) as f64 / n as f64 * 360.0, 1.0, 1.0),
        );
        let i = mesh.vertices.len() as u32;
        mesh.colored_vertex(at(a0, inner), c0);
        mesh.colored_vertex(at(a0, outer), c0);
        mesh.colored_vertex(at(a1, outer), c1);
        mesh.colored_vertex(at(a1, inner), c1);
        mesh.add_triangle(i, i + 1, i + 2);
        mesh.add_triangle(i, i + 2, i + 3);
    }
    // The square: saturation across, brightness from 1 at the top to 0.
    let g = 24;
    let base = mesh.vertices.len() as u32;
    for y in 0..=g {
        for x in 0..=g {
            let (fx, fy) = (x as f32 / g as f32, y as f32 / g as f32);
            let p = Pos2::new(sq.left() + fx * sq.width(), sq.top() + fy * sq.height());
            mesh.colored_vertex(p, hsb(h, fx as f64, 1.0 - fy as f64));
        }
    }
    for y in 0..g {
        for x in 0..g {
            let i = base + (y * (g + 1) + x) as u32;
            let j = i + (g + 1) as u32;
            mesh.add_triangle(i, i + 1, j + 1);
            mesh.add_triangle(i, j + 1, j);
        }
    }
    let p = ui.painter();
    p.add(egui::Shape::mesh(mesh));
    let mark = |p: &egui::Painter, at: Pos2| {
        p.circle_stroke(at, HANDLE / 2.0 + 1.0, Stroke::new(1.5, Color32::BLACK));
        p.circle_stroke(at, HANDLE / 2.0, Stroke::new(1.0, Color32::WHITE));
    };
    let ha = (h / 360.0 * std::f64::consts::TAU) as f32;
    mark(p, at(ha, (inner + outer) / 2.0));
    mark(
        p,
        Pos2::new(
            sq.left() + s as f32 * sq.width(),
            sq.top() + (1.0 - b as f32) * sq.height(),
        ),
    );
    p.rect_stroke(
        sq,
        0.0,
        Stroke::new(1.0, Tokens::border()),
        egui::StrokeKind::Outside,
    );
    // Dragging: the part touched (ring or square) is chosen on the press.
    let target_id = id.with("target");
    let pointer = resp.interact_pointer_pos();
    if resp.drag_started() || resp.clicked() {
        if let Some(q) = ui.input(|i| i.pointer.press_origin()).or(pointer) {
            let d = (q - c).length();
            let t: u8 = if sq.expand(8.0).contains(q) {
                2
            } else if d >= inner - 8.0 && d <= outer + 8.0 {
                1
            } else {
                0
            };
            ui.data_mut(|m| m.insert_temp(target_id, t));
        }
    }
    let target: u8 = ui.data(|m| m.get_temp(target_id)).unwrap_or(0);
    if resp.drag_stopped() {
        ui.data_mut(|m| m.remove::<u8>(target_id));
    }
    if resp.dragged() || resp.clicked() {
        if let Some(q) = pointer {
            let (nh, ns, nb) = match target {
                1 => {
                    let v = q - c;
                    let a = (-v.y).atan2(v.x) as f64;
                    (
                        a.rem_euclid(std::f64::consts::TAU) / std::f64::consts::TAU * 360.0,
                        s,
                        b,
                    )
                }
                2 => (
                    h,
                    ((q.x - sq.left()) / sq.width()).clamp(0.0, 1.0) as f64,
                    (1.0 - (q.y - sq.top()) / sq.height()).clamp(0.0, 1.0) as f64,
                ),
                _ => return None,
            };
            ui.data_mut(|m| m.insert_temp(id, nh));
            return Some(Color::from_hsb(nh, ns, nb).convert_to(model_of(color)));
        }
    }
    None
}

/// A colour button that opens the picker in a pop-up. `picker` and
/// `palettes` are the window's; `eye` is set when the eyedropper was
/// asked for. The new colour when one was picked.
pub fn color_popup(
    ui: &mut Ui,
    id: &str,
    color: Color,
    picker: &mut ColorPicker,
    palettes: &PaletteList,
    eye: &mut bool,
) -> Option<Color> {
    let popup_id = ui.make_persistent_id(("color_popup", id));
    let open = egui::Popup::is_id_open(ui.ctx(), popup_id);
    let r = color_button(ui, color, open).on_hover_text(color_text(color));
    let mut out = None;
    egui::Popup::menu(&r)
        .id(popup_id)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            ui.set_width(PICKER_WIDTH);
            crate::ui::chrome::form_style(ui);
            let o = picker.show(ui, id, color, palettes);
            if o.eyedrop {
                *eye = true;
                ui.close();
            }
            out = o.color;
        });
    out
}

// ----- the eyedropper -------------------------------------------------------------

/// The tag of the eyedropper's screenshot.
const EYE_TAG: &str = "style_window_eyedropper";

enum Eye {
    Off,
    /// The window is hidden this frame; the screenshot is asked for next.
    Hide,
    /// Waiting for the screenshot (frames waited).
    Wait(u32),
    Pick(egui::TextureHandle, std::sync::Arc<egui::ColorImage>),
}

/// The windows' eyedropper: it hides the window, takes a picture of the
/// program window, shows it frozen with a magnifier and waits for a
/// click (right click or Esc gives up).
pub struct Eyedropper {
    state: Eye,
}

impl Default for Eyedropper {
    fn default() -> Self {
        Eyedropper { state: Eye::Off }
    }
}

impl Eyedropper {
    /// Start: the window stays hidden while [`Self::active`].
    pub fn start(&mut self) {
        self.state = Eye::Hide;
    }

    pub fn active(&self) -> bool {
        !matches!(self.state, Eye::Off)
    }

    /// One frame of the eyedropper; the colour once picked.
    pub fn run(&mut self, ctx: &Context) -> Option<Color> {
        match &mut self.state {
            Eye::Off => {}
            Eye::Hide => {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::new(
                    EYE_TAG,
                )));
                self.state = Eye::Wait(0);
                ctx.request_repaint();
            }
            Eye::Wait(frames) => {
                let img = ctx.input(|i| {
                    i.events.iter().find_map(|e| match e {
                        egui::Event::Screenshot {
                            user_data, image, ..
                        } if user_data
                            .data
                            .as_ref()
                            .and_then(|d| d.downcast_ref::<&str>())
                            == Some(&EYE_TAG) =>
                        {
                            Some(image.clone())
                        }
                        _ => None,
                    })
                });
                if let Some(img) = img {
                    let tex = ctx.load_texture(
                        "eyedropper_screen",
                        (*img).clone(),
                        egui::TextureOptions::NEAREST,
                    );
                    self.state = Eye::Pick(tex, img);
                } else if *frames > 120 {
                    self.state = Eye::Off;
                } else {
                    *frames += 1;
                    ctx.request_repaint();
                }
            }
            Eye::Pick(tex, img) => {
                let (tex, img) = (tex.clone(), img.clone());
                let picked = pick_frame(ctx, &tex, &img);
                if picked.is_some() {
                    self.state = Eye::Off;
                }
                return picked.flatten();
            }
        }
        None
    }
}

/// The frozen picture with the magnifier: `Some(Some(colour))` when
/// clicked, `Some(None)` when given up, `None` while waiting.
fn pick_frame(
    ctx: &Context,
    tex: &egui::TextureHandle,
    img: &egui::ColorImage,
) -> Option<Option<Color>> {
    let screen = ctx.content_rect();
    let mut out = None;
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        out = Some(None);
    }
    let (w, h) = (img.size[0], img.size[1]);
    if w == 0 || h == 0 {
        return Some(None);
    }
    egui::Area::new(egui::Id::new("eyedropper_area"))
        .order(egui::Order::Foreground)
        .fixed_pos(screen.min)
        .show(ctx, |ui| {
            let (r, resp) = ui.allocate_exact_size(screen.size(), Sense::click());
            let p = ui.painter();
            p.image(
                tex.id(),
                r,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
            ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
            let (sx, sy) = (w as f32 / r.width(), h as f32 / r.height());
            let px = |q: Pos2| {
                let x = ((q.x - r.left()) * sx) as i64;
                let y = ((q.y - r.top()) * sy) as i64;
                (
                    x.clamp(0, w as i64 - 1) as usize,
                    y.clamp(0, h as i64 - 1) as usize,
                )
            };
            let at = |x: usize, y: usize| img.pixels.get(y * w + x).copied().unwrap_or_default();
            if let Some(q) = resp.hover_pos() {
                // The magnifier: 11 x 11 pixels, the middle one framed.
                let (cx, cy) = px(q);
                let n = 11i64;
                let origin = q + Vec2::splat(24.0);
                let lens = Rect::from_min_size(origin, Vec2::splat(LOUPE_CELL * n as f32));
                p.rect_filled(lens.expand(2.0), 2.0, Color32::BLACK);
                for dy in 0..n {
                    for dx in 0..n {
                        let x = (cx as i64 + dx - n / 2).clamp(0, w as i64 - 1) as usize;
                        let y = (cy as i64 + dy - n / 2).clamp(0, h as i64 - 1) as usize;
                        let cr = Rect::from_min_size(
                            origin + Vec2::new(dx as f32 * LOUPE_CELL, dy as f32 * LOUPE_CELL),
                            Vec2::splat(LOUPE_CELL),
                        );
                        p.rect_filled(cr, 0.0, at(x, y));
                    }
                }
                let mid = Rect::from_min_size(
                    origin + Vec2::splat(LOUPE_CELL * (n / 2) as f32),
                    Vec2::splat(LOUPE_CELL),
                );
                p.rect_stroke(
                    mid,
                    0.0,
                    Stroke::new(1.5, Color32::WHITE),
                    egui::StrokeKind::Outside,
                );
                let c = at(cx, cy);
                p.text(
                    lens.left_bottom() + Vec2::new(0.0, 8.0),
                    Align2::LEFT_TOP,
                    format!("#{:02X}{:02X}{:02X}", c.r(), c.g(), c.b()),
                    FontId::proportional(10.0),
                    Color32::WHITE,
                );
            }
            if resp.clicked() {
                if let Some(q) = resp.interact_pointer_pos() {
                    let (x, y) = px(q);
                    let c = at(x, y);
                    out = Some(Some(Color::rgb8(c.r(), c.g(), c.b())));
                }
            }
            if resp.secondary_clicked() {
                out = Some(None);
            }
        });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_text_names_the_model_values() {
        assert_eq!(
            color_text(Color::cmyk_pct(100.0, 50.0, 0.0, 0.0))
                .split(" · ")
                .next(),
            Some("C 100 M 50 Y 0 K 0")
        );
        assert!(color_text(Color::rgb8(255, 0, 0)).starts_with("R 255 G 0 B 0"));
    }

    /// The picker draws both tabs; the search filters the grid; nothing
    /// is picked without the pointer.
    #[test]
    fn picker_draws_and_filters() {
        let ctx = Context::default();
        let pals: PaletteList = vec![(
            "Test".into(),
            vec![
                ("Red".into(), Color::rgb8(255, 0, 0)),
                ("Blue".into(), Color::rgb8(0, 0, 255)),
            ],
        )];
        let mut p = ColorPicker::default();
        for tab in [Tab::Models, Tab::Palettes] {
            p.tab = tab;
            p.search = "blu".into();
            let mut out = ctx.run_ui(Default::default(), |ui| {
                let o = p.show(ui, "t", Color::cmyk_pct(0.0, 0.0, 0.0, 100.0), &pals);
                assert!(o.color.is_none() && !o.eyedrop);
            });
            out.textures_delta.clear();
        }
    }

    /// Colours picked on the wheel come back in the colour's model; spot
    /// colours come back as process CMYK.
    #[test]
    fn wheel_keeps_the_model() {
        assert_eq!(model_of(Color::cmyk_pct(0.0, 0.0, 0.0, 0.0)), "CMYK");
        assert_eq!(model_of(Color::rgb8(1, 2, 3)), "RGB");
        assert_eq!(model_of(Color::Registration), "CMYK");
    }
}

#[cfg(test)]
mod canvas_tests {
    use crate::app::App;
    use graphdraw_core::{geometry::Rect, ShapeKind};

    fn run(ctx: &egui::Context, app: &mut App, time: f64, events: Vec<egui::Event>) {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1600.0, 1000.0),
            )),
            time: Some(time),
            events,
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| crate::ui::root(app, ui));
        out.textures_delta.clear();
    }

    /// With the Outline window open the drawing stays usable: a drag on
    /// the canvas moves the selected object.
    #[test]
    fn the_canvas_takes_drags_while_the_outline_window_is_open() {
        drag_with_the_window_open(false);
        drag_with_the_window_open(true);
    }

    /// A drag on the selected object (a clip frame when `clip`) moves it
    /// with the Outline window open.
    fn drag_with_the_window_open(clip: bool) {
        let mut app = App::headless();
        let id = app
            .new_shape(ShapeKind::Rect {
                rect: Rect::new(20.0, 20.0, 60.0, 60.0),
                radius: 0.0,
                corners: None,
            })
            .unwrap();
        if clip {
            let inner = app
                .new_shape(ShapeKind::Rect {
                    rect: Rect::new(30.0, 30.0, 50.0, 50.0),
                    radius: 0.0,
                    corners: None,
                })
                .unwrap();
            app.place_inside(vec![inner], id);
        }
        app.select(vec![id]);
        app.open_outline_window();
        let ctx = crate::theme::ui_context();
        let mut t = 0.0;
        for _ in 0..3 {
            t += 0.1;
            run(&ctx, &mut app, t, Vec::new());
        }
        assert!(app.style_windows.outline.is_some());
        // Bring the rectangle under the middle of the canvas.
        let c = app.canvas_rect.center();
        let mid = app.view.to_screen(graphdraw_core::geometry::Point::new(40.0, 40.0));
        app.view.origin += c - mid;
        run(&ctx, &mut app, t + 0.1, Vec::new());
        let before = app.doc().shape_on_page(id).unwrap().bounds();
        let at = |dx: f32| egui::pos2(c.x + dx, c.y);
        let press = |pressed| egui::Event::PointerButton {
            pos: at(if pressed { 0.0 } else { 60.0 }),
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        let steps = vec![
            vec![egui::Event::PointerMoved(at(0.0))],
            vec![press(true)],
            vec![egui::Event::PointerMoved(at(20.0))],
            vec![egui::Event::PointerMoved(at(40.0))],
            vec![egui::Event::PointerMoved(at(60.0))],
            vec![press(false)],
            Vec::new(),
        ];
        for (k, ev) in steps.into_iter().enumerate() {
            run(&ctx, &mut app, t + 0.2 + k as f64 * 0.05, ev);
        }
        let after = app.doc().shape_on_page(id).unwrap().bounds();
        assert!(after.x0 > before.x0 + 1.0, "{before:?} -> {after:?}");
        assert!(app.style_windows.outline.is_some());
    }
}
