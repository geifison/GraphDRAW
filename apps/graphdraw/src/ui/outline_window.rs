//! The Outline Pen (F12): a floating window to edit the outline of the
//! selected objects (or of new objects, with nothing selected). The left
//! column has the colour, the width (with its unit and preset widths), the
//! line style (Edit... builds one's own dashes), the corners, the line
//! caps and the position; the right column the arrowheads, the
//! calligraphic pen (drag the nib's tip to turn it and its side to
//! flatten it), the variable width and a preview. Underneath: behind fill,
//! scale with object and overprint. The drawing shows the new outline on
//! the objects while the window is open (see `style_windows`); with
//! several objects selected, OK changes only what was changed in the
//! window, each object keeping the rest of its own outline.
//!
//! Controls whose settings the drawing does not keep yet (outline
//! transparency, how dashes go through corners, the miter limit, the
//! position, the arrowheads' size, rotation, mirror and offset) are shown
//! dimmed.

use std::collections::HashMap;

use crate::app::{App, Units};
use crate::i18n::tr;
use crate::theme::Tokens;
use crate::ui::style_windows::{
    self as sw, color_popup, heading, icon_button, later, note, Cached, ColorPicker, Eyedropper,
    Footer, Outcome, PaletteList,
};
use egui::{Color32, Context, Rect, Sense, Stroke as EStroke, Ui, Vec2};
use graphdraw_core::geometry::{BezPath, Point, Size};
use graphdraw_core::{
    Arrowhead, Color, Command, Document, Fill, LineCap, LineJoin, Shape, ShapeId, ShapeKind,
    Stroke, WidthNode,
};
use serde::{Deserialize, Serialize};

/// The window's width.
const WIDTH: f32 = 720.0;
/// The units the width is shown in.
const UNITS: [Units; 6] = [
    Units::Points,
    Units::Millimeters,
    Units::Centimeters,
    Units::Inches,
    Units::Pixels,
    Units::Meters,
];
/// Preset widths in points (0: hairline).
const WIDTH_PRESETS: [f64; 16] = [
    0.0, 0.1, 0.25, 0.5, 0.75, 1.0, 1.5, 2.0, 3.0, 4.0, 6.0, 8.0, 12.0, 16.0, 24.0, 36.0,
];
/// The built-in line styles: name key and dash pattern (multiples of the
/// width; empty is solid).
const DASHES: [(&str, &[f64]); 5] = [
    ("outlinewin.dash_solid", &[]),
    ("outlinewin.dash_dashed", &[4.0, 2.0]),
    ("outlinewin.dash_long", &[8.0, 3.0]),
    ("outlinewin.dash_dotted", &[1.0, 2.0]),
    ("outlinewin.dash_dash_dot", &[5.0, 2.0, 1.0, 2.0]),
];
/// The variable width presets: name key and (place, share of the width).
const WIDTH_SHAPES: [(&str, &[(f64, f64)]); 4] = [
    ("outlinewin.taper_end", &[(0.0, 1.0), (1.0, 0.0)]),
    ("outlinewin.taper_start", &[(0.0, 0.0), (1.0, 1.0)]),
    ("outlinewin.taper_both", &[(0.0, 0.0), (0.5, 1.0), (1.0, 0.0)]),
    ("outlinewin.bulge", &[(0.0, 0.3), (0.5, 1.6), (1.0, 0.3)]),
];
/// A cell of the line style editor.
const DASH_CELL: f32 = 16.0;
/// A line style's sample in the list.
const DASH_SAMPLE_W: f32 = 60.0;
/// The nib view.
const NIB_VIEW: f32 = 96.0;
/// An arrowhead's thumbnail.
const ARROW_THUMB: Vec2 = Vec2::new(56.0, 18.0);
/// Points per millimetre.
const PT_PER_MM: f64 = 72.0 / 25.4;

/// A line style of the user's: its name and pattern in multiples of the
/// width. Kept in the settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LineStyle {
    pub name: String,
    pub pattern: Vec<f64>,
}

/// Dash cells (dash = true) as a pattern: alternating runs starting with
/// a dash; a solid or empty ruler gives no pattern.
pub fn cells_to_pattern(cells: &[bool]) -> Vec<f64> {
    let Some(first) = cells.iter().position(|c| *c) else {
        return Vec::new();
    };
    let rot: Vec<bool> = cells[first..]
        .iter()
        .chain(&cells[..first])
        .copied()
        .collect();
    let mut out: Vec<f64> = Vec::new();
    let mut cur = true;
    let mut run = 0.0;
    for c in rot {
        if c == cur {
            run += 1.0;
        } else {
            out.push(run);
            cur = c;
            run = 1.0;
        }
    }
    out.push(run);
    if out.len() % 2 == 1 {
        // It ends with a dash: that joins the first one when repeated.
        match out.pop() {
            Some(last) if !out.is_empty() => out[0] += last,
            _ => return Vec::new(),
        }
    }
    out
}

/// A pattern as cells (whole cells, at most 64).
pub fn pattern_to_cells(p: &[f64]) -> Vec<bool> {
    let mut out = Vec::new();
    for (i, v) in p.iter().enumerate() {
        for _ in 0..(v.round().max(1.0) as usize) {
            out.push(i % 2 == 0);
        }
    }
    if out.is_empty() {
        out = vec![true, true, true, true, false, false];
    }
    out.truncate(64);
    out
}

/// Which line style is chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    Builtin(usize),
    User(usize),
    Custom,
}

/// The line style editor: a ruler of dash and gap cells, and a name.
struct DashEditor {
    cells: Vec<bool>,
    name: String,
}

/// What the line style editor asked for.
enum EditorOut {
    None,
    Close,
    Use(Vec<f64>),
    Add(String, Vec<f64>),
    Replace(usize, Vec<f64>),
}

pub struct OutlineWindow {
    /// The objects edited; empty: the defaults of new objects.
    targets: Vec<ShapeId>,
    seen: Vec<ShapeId>,
    initial: Option<Stroke>,
    initial_overprint: bool,
    pub(crate) draft: Stroke,
    pub(crate) enabled: bool,
    pub(crate) overprint: bool,
    style: Style,
    unit: Units,
    changed: bool,
    rev: u64,
    palettes: PaletteList,
    picker: ColorPicker,
    eye: Eyedropper,
    editor: Option<DashEditor>,
    nib_drag: Option<bool>,
    hovered: bool,
    preview: Cached<(Option<Stroke>, u32)>,
    thumbs: HashMap<(String, bool), egui::TextureHandle>,
}

impl OutlineWindow {
    /// The window for the selection (or the defaults, with nothing
    /// selected).
    pub fn open(app: &App) -> Box<OutlineWindow> {
        let targets = sw::targets(app);
        let (initial, overprint) = match app.selected_shapes().first() {
            Some(s) => {
                let leaf = sw::first_leaf(s);
                (leaf.stroke.clone(), leaf.overprint_outline)
            }
            None => (app.doc().defaults.graphic.stroke.clone(), false),
        };
        let unit = if app.units == Units::Points {
            Units::Points
        } else {
            Units::Millimeters
        };
        let mut w = OutlineWindow::new(initial, overprint, targets, unit);
        w.seen = app.selection.clone();
        w.palettes = sw::palettes_of(app);
        w.style = w.detect_style(&app.settings.line_styles);
        Box::new(w)
    }

    pub(crate) fn new(
        initial: Option<Stroke>,
        overprint: bool,
        targets: Vec<ShapeId>,
        unit: Units,
    ) -> Self {
        let draft = initial
            .clone()
            .unwrap_or_else(|| Stroke::new(Color::BLACK, 0.2));
        let mut w = OutlineWindow {
            targets,
            seen: Vec::new(),
            enabled: initial.is_some(),
            initial,
            initial_overprint: overprint,
            draft,
            overprint,
            style: Style::Custom,
            unit,
            changed: false,
            rev: 1,
            palettes: Vec::new(),
            picker: ColorPicker::default(),
            eye: Eyedropper::default(),
            editor: None,
            nib_drag: None,
            hovered: false,
            preview: Cached::default(),
            thumbs: HashMap::new(),
        };
        w.style = w.detect_style(&[]);
        w
    }

    pub fn revision(&self) -> u64 {
        self.rev
    }

    pub fn previews(&self) -> bool {
        self.changed && !self.targets.is_empty()
    }

    fn touch(&mut self) {
        self.changed = true;
        self.rev = self.rev.wrapping_add(1);
    }

    /// A change made from outside the window (screenshots).
    #[cfg(test)]
    pub(crate) fn mark_changed(&mut self) {
        self.touch();
    }

    /// The outline the window makes.
    pub(crate) fn result(&self) -> Option<Stroke> {
        self.enabled.then(|| self.draft.clone())
    }

    fn detect_style(&self, user: &[LineStyle]) -> Style {
        let d = &self.draft.dash;
        let same = |p: &[f64]| p.len() == d.len() && p.iter().zip(d).all(|(a, b)| (a - b).abs() < 1e-6);
        if let Some(i) = DASHES.iter().position(|(_, p)| same(p)) {
            return Style::Builtin(i);
        }
        user.iter()
            .position(|s| same(&s.pattern))
            .map_or(Style::Custom, Style::User)
    }

    /// The commands OK runs: with an outline before and after, each
    /// object gets only the settings changed here.
    pub fn commands(&self, doc: &Document) -> Vec<Command> {
        if !self.changed || self.targets.is_empty() {
            return Vec::new();
        }
        let mut v = Vec::new();
        match (self.result(), &self.initial) {
            (None, _) => v.push(Command::SetStroke {
                shapes: self.targets.clone(),
                stroke: None,
            }),
            (Some(new), None) => v.push(Command::SetStroke {
                shapes: self.targets.clone(),
                stroke: Some(new),
            }),
            (Some(new), Some(old)) => {
                for id in &self.targets {
                    let own = doc
                        .find_shape(*id)
                        .and_then(|s| sw::first_leaf(s).stroke.clone());
                    let stroke = match own {
                        Some(mut s) => {
                            merge_changes(&mut s, old, &new);
                            s
                        }
                        None => new.clone(),
                    };
                    v.push(Command::SetStroke {
                        shapes: vec![*id],
                        stroke: Some(stroke),
                    });
                }
            }
        }
        if self.overprint != self.initial_overprint {
            v.push(Command::SetOverprint {
                shapes: self.targets.clone(),
                fill: None,
                outline: Some(self.overprint),
            });
        }
        v
    }

    /// OK: the objects get the outline (one undo step), or, with nothing
    /// selected, the defaults of new objects.
    pub fn apply(&self, app: &mut App) {
        if !self.changed {
            return;
        }
        if self.targets.is_empty() {
            app.request_default_change(crate::defaults::DefaultChange::Outline(self.result()));
            return;
        }
        if let Some(s) = self.result() {
            app.current_colors.1 = s.color;
        }
        let cmds = self.commands(app.doc());
        sw::run_as_one_step(app, cmds);
    }

    fn follow_selection(&mut self, app: &App) {
        if app.selection == self.seen {
            return;
        }
        if self.changed {
            self.seen = app.selection.clone();
            return;
        }
        let mut fresh = *OutlineWindow::open(app);
        fresh.picker = std::mem::take(&mut self.picker);
        fresh.unit = self.unit;
        fresh.thumbs = std::mem::take(&mut self.thumbs);
        *self = fresh;
    }

    /// Draw the window (and the line style editor when it is open).
    pub fn show(&mut self, app: &mut App, ctx: &Context) -> Outcome {
        self.follow_selection(app);
        if self.eye.active() {
            if let Some(c) = self.eye.run(ctx) {
                self.draft.color = c;
                self.touch();
            }
            return Outcome::Open(Rect::NOTHING);
        }
        let title = if self.targets.is_empty() {
            tr("outlinewin.title_defaults")
        } else {
            tr("outlinewin.title")
        };
        let user = app.settings.line_styles.clone();
        let custom_heads = app.settings.custom_arrowheads.clone();
        let (out, closed, rect) = sw::floating(ctx, "outline", &title, |ui| {
            ui.set_width(WIDTH);
            ui.horizontal(|ui| {
                if ui
                    .checkbox(&mut self.enabled, tr("outlinewin.has_outline"))
                    .changed()
                {
                    self.touch();
                }
            });
            ui.separator();
            let max_h = (ui.ctx().content_rect().height() * 0.75 - 120.0).max(240.0);
            egui::ScrollArea::vertical()
                .id_salt("outline_window_body")
                .max_height(max_h)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    ui.add_enabled_ui(self.enabled, |ui| {
                        ui.horizontal_top(|ui| {
                            let col = (WIDTH - 24.0) / 2.0;
                            ui.vertical(|ui| {
                                ui.set_width(col);
                                self.left(ui, &user);
                            });
                            ui.add_space(24.0);
                            ui.vertical(|ui| {
                                ui.set_width(col);
                                self.right(ui, &custom_heads);
                            });
                        });
                        ui.separator();
                        self.options(ui);
                    });
                });
            ui.separator();
            let hovered = self.hovered && self.editor.is_none();
            sw::footer(ui, &tr("outlinewin.help"), hovered, |ui| {
                if !self.targets.is_empty()
                    && ui
                        .small_button(tr("stylewin.in_docker"))
                        .on_hover_text(tr("stylewin.in_docker_tip"))
                        .clicked()
                {
                    app.open_outline_editor();
                }
            })
        });
        self.hovered = ctx.pointer_latest_pos().is_some_and(|p| rect.contains(p));
        self.dash_editor(app, ctx);
        if closed {
            return Outcome::Cancel;
        }
        match out.flatten() {
            Some(Footer::Ok) => Outcome::Ok,
            Some(Footer::Cancel) => Outcome::Cancel,
            None => Outcome::Open(rect),
        }
    }

    /// The left column: colour and width, line style, corners, caps and
    /// position.
    fn left(&mut self, ui: &mut Ui, user: &[LineStyle]) {
        heading(ui, &tr("outlinewin.color_width"));
        ui.horizontal(|ui| {
            let mut eye = false;
            if let Some(c) = color_popup(
                ui,
                "outline",
                self.draft.color,
                &mut self.picker,
                &self.palettes,
                &mut eye,
            ) {
                self.draft.color = c;
                self.touch();
            }
            if eye {
                self.eye.start();
            }
            later(ui, |ui| {
                ui.label(tr("fillwin.transparency"));
                let mut tp = 0.0;
                ui.add(egui::Slider::new(&mut tp, 0.0..=100.0).suffix(" %"));
            });
        });
        ui.horizontal(|ui| {
            ui.label(tr("outlinewin.width"));
            let hair = self.draft.width <= Stroke::HAIRLINE + 1e-9;
            ui.add_enabled_ui(!hair, |ui| {
                if let Some(v) = crate::measure_field::length_field_decimals(
                    ui,
                    "outline_width",
                    self.draft.width,
                    self.unit,
                    96.0,
                    3,
                    90.0,
                ) {
                    self.draft.width = v.clamp(0.001, 1000.0);
                    self.width_changed(hair);
                }
            });
            egui::ComboBox::from_id_salt("outline_unit")
                .selected_text(self.unit.id())
                .width(56.0)
                .show_ui(ui, |ui| {
                    for u in UNITS {
                        ui.selectable_value(&mut self.unit, u, u.id());
                    }
                })
                .response
                .on_hover_text(tr("outlinewin.unit"));
            let old_width = self.draft.width;
            let mut picked = None;
            ui.menu_button(tr("outlinewin.presets"), |ui| {
                for pt in WIDTH_PRESETS {
                    let label = if pt == 0.0 {
                        tr("docker.hairline")
                    } else {
                        format!("{} pt", crate::measure_field::localize(fmt_num(pt)))
                    };
                    if ui.button(label).clicked() {
                        picked = Some(if pt == 0.0 {
                            Stroke::HAIRLINE
                        } else {
                            pt / PT_PER_MM
                        });
                        ui.close();
                    }
                }
            })
            .response
            .on_hover_text(tr("outlinewin.presets_tip"));
            if let Some(w) = picked {
                self.draft.width = w;
                self.width_changed(old_width <= Stroke::HAIRLINE + 1e-9);
            }
            let mut h = hair;
            if ui
                .checkbox(&mut h, tr("docker.hairline"))
                .on_hover_text(tr("outlinewin.hairline_tip"))
                .changed()
            {
                self.draft.width = if h { Stroke::HAIRLINE } else { 0.2 };
                self.width_changed(hair);
            }
        });
        heading(ui, &tr("outlinewin.line_style"));
        ui.horizontal(|ui| {
            let name = match self.style {
                Style::Builtin(i) => tr(DASHES[i].0),
                Style::User(i) => user.get(i).map_or(String::new(), |s| s.name.clone()),
                Style::Custom => tr("outlinewin.dash_custom"),
            };
            let mut pick = None;
            egui::ComboBox::from_id_salt("outline_style")
                .selected_text(name)
                .width(170.0)
                .height(260.0)
                .show_ui(ui, |ui| {
                    for (i, (key, p)) in DASHES.iter().enumerate() {
                        ui.horizontal(|ui| {
                            pattern_sample(ui, p);
                            if ui
                                .selectable_label(self.style == Style::Builtin(i), tr(key))
                                .clicked()
                            {
                                pick = Some((Style::Builtin(i), p.to_vec()));
                            }
                        });
                    }
                    if !user.is_empty() {
                        ui.separator();
                    }
                    for (i, s) in user.iter().enumerate() {
                        ui.horizontal(|ui| {
                            pattern_sample(ui, &s.pattern);
                            if ui
                                .selectable_label(self.style == Style::User(i), &s.name)
                                .clicked()
                            {
                                pick = Some((Style::User(i), s.pattern.clone()));
                            }
                        });
                    }
                });
            if let Some((st, p)) = pick {
                self.style = st;
                self.draft.dash = p;
                // Dots look like dots with round ends.
                if st == Style::Builtin(3) {
                    self.draft.cap = LineCap::Round;
                }
                self.touch();
            }
            if ui
                .button(tr("outlinewin.edit_style"))
                .on_hover_text(tr("outlinewin.edit_style_tip"))
                .clicked()
            {
                self.editor = Some(DashEditor {
                    cells: pattern_to_cells(&self.draft.dash),
                    name: match self.style {
                        Style::User(i) => user.get(i).map_or(String::new(), |s| s.name.clone()),
                        _ => String::new(),
                    },
                });
            }
        });
        ui.horizontal(|ui| {
            ui.label(tr("outlinewin.at_corners"));
            later(ui, |ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                for k in [
                    "outlinewin.dash_standard",
                    "outlinewin.dash_align",
                    "outlinewin.dash_fixed",
                ] {
                    icon_button(ui, k, &tr(k), k == "outlinewin.dash_standard", true);
                }
            });
        });
        heading(ui, &tr("outlinewin.corners"));
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            for (j, key) in [
                (LineJoin::Miter, "outlinewin.join_miter_tip"),
                (LineJoin::Round, "outlinewin.join_round_tip"),
                (LineJoin::Bevel, "outlinewin.join_bevel_tip"),
            ] {
                if icon_button(ui, key, &tr(key), self.draft.join == j, true).clicked()
                    && self.draft.join != j
                {
                    self.draft.join = j;
                    self.touch();
                }
            }
            ui.add_space(8.0);
            later(ui, |ui| {
                ui.label(tr("outlinewin.miter_limit"));
                let mut l = 4.0;
                ui.add(egui::DragValue::new(&mut l));
            });
        });
        heading(ui, &tr("outlinewin.caps"));
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            for (c, key) in [
                (LineCap::Butt, "outlinewin.cap_butt_tip"),
                (LineCap::Round, "outlinewin.cap_round_tip"),
                (LineCap::Square, "outlinewin.cap_square_tip"),
            ] {
                if icon_button(ui, key, &tr(key), self.draft.cap == c, true).clicked()
                    && self.draft.cap != c
                {
                    self.draft.cap = c;
                    self.touch();
                }
            }
        });
        heading(ui, &tr("outlinewin.position"));
        later(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                for (key, on) in [
                    ("outlinewin.pos_outside_tip", false),
                    ("outlinewin.pos_center_tip", true),
                    ("outlinewin.pos_inside_tip", false),
                ] {
                    icon_button(ui, key, &tr(key), on, true);
                }
            });
        });
        note(ui, &tr("outlinewin.open_centered"));
    }

    /// After a new width: a variable width keeps its shape at the new
    /// width.
    fn width_changed(&mut self, was_hairline: bool) {
        let _ = was_hairline;
        if let Some(i) = self.width_shape() {
            self.draft.widths = width_nodes(i, self.draft.width);
        }
        self.touch();
    }

    /// The variable width preset the outline has, if any.
    fn width_shape(&self) -> Option<usize> {
        (0..WIDTH_SHAPES.len()).find(|i| {
            let n = width_nodes(*i, self.draft.width);
            !self.draft.widths.is_empty()
                && n.len() == self.draft.widths.len()
                && n.iter().zip(&self.draft.widths).all(|(a, b)| {
                    (a.at - b.at).abs() < 1e-6
                        && relative(a.left, b.left)
                        && relative(a.right, b.right)
                })
        })
        .or_else(|| {
            // The nodes of a preset at another width (before a change).
            (0..WIDTH_SHAPES.len()).find(|i| {
                let p = WIDTH_SHAPES[*i].1;
                let w = &self.draft.widths;
                p.len() == w.len()
                    && !w.is_empty()
                    && p.iter().zip(w).all(|(a, b)| (a.0 - b.at).abs() < 1e-6)
                    && {
                        let k0 = w.iter().zip(p).find(|(_, q)| q.1 > 0.0);
                        match k0 {
                            Some((n, q)) => {
                                let half = n.left / q.1;
                                w.iter().zip(p).all(|(n, q)| relative(n.left, q.1 * half))
                            }
                            None => false,
                        }
                    }
            })
        })
    }

    /// The right column: arrowheads, calligraphic pen, variable width and
    /// the preview.
    fn right(&mut self, ui: &mut Ui, custom: &[Arrowhead]) {
        heading(ui, &tr("outlinewin.arrowheads"));
        ui.horizontal(|ui| {
            let mut start = self.draft.start_arrow.clone();
            if self.arrow_menu(ui, "arrow_start", true, &mut start, custom) {
                self.draft.start_arrow = start;
                self.touch();
            }
            let mut end = self.draft.end_arrow.clone();
            if self.arrow_menu(ui, "arrow_end", false, &mut end, custom) {
                self.draft.end_arrow = end;
                self.touch();
            }
            let r = ui.button(tr("outlinewin.arrow_options"));
            let mut swap = false;
            egui::Popup::menu(&r)
                .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                .show(|ui| {
                    if ui.button(tr("outlinewin.swap_arrows")).clicked() {
                        swap = true;
                    }
                    ui.separator();
                    for (key, a) in [
                        ("outlinewin.start", &self.draft.start_arrow),
                        ("outlinewin.end", &self.draft.end_arrow),
                    ] {
                        ui.strong(tr(key));
                        if matches!(a, Arrowhead::None) {
                            note(ui, &tr("outlinewin.arrow_none"));
                        } else {
                            later(ui, |ui| {
                                ui.horizontal(|ui| {
                                    let (mut s, mut r, mut m) = (100.0, 0.0, false);
                                    ui.add(egui::DragValue::new(&mut s).suffix(" %"))
                                        .on_hover_text(tr("outlinewin.arrow_size"));
                                    ui.add(egui::DragValue::new(&mut r).suffix("°"))
                                        .on_hover_text(tr("outlinewin.arrow_rotation"));
                                    ui.checkbox(&mut m, tr("outlinewin.arrow_mirror"));
                                    let mut o = 0.0;
                                    ui.add(egui::DragValue::new(&mut o).suffix(" mm"))
                                        .on_hover_text(tr("outlinewin.arrow_offset"));
                                });
                            });
                        }
                    }
                });
            if swap {
                std::mem::swap(&mut self.draft.start_arrow, &mut self.draft.end_arrow);
                self.touch();
            }
        });
        note(ui, &tr("outlinewin.arrows_note"));
        heading(ui, &tr("outlinewin.calligraphic"));
        ui.horizontal_top(|ui| {
            self.nib_view(ui);
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.label(tr("outlinewin.stretch"));
                    let pct = (self.draft.stretch * 100.0).round();
                    if let Some(v) =
                        crate::measure_field::number_field(ui, "nib_stretch", pct, 0, " %", 64.0)
                    {
                        self.draft.stretch = (v / 100.0).clamp(0.01, 1.0);
                        self.touch();
                    }
                })
                .response
                .on_hover_text(tr("outlinewin.stretch_tip"));
                ui.horizontal(|ui| {
                    ui.label(tr("fillwin.angle"));
                    ui.add_enabled_ui(self.draft.stretch < 0.999, |ui| {
                        if let Some(v) = crate::measure_field::number_field(
                            ui,
                            "nib_angle",
                            self.draft.nib_angle,
                            0,
                            "°",
                            64.0,
                        ) {
                            self.draft.nib_angle = v.rem_euclid(360.0);
                            self.touch();
                        }
                    });
                });
                if ui
                    .button(tr("outlinewin.nib_default"))
                    .on_hover_text(tr("outlinewin.nib_default_tip"))
                    .clicked()
                {
                    self.draft.stretch = 1.0;
                    self.draft.nib_angle = 0.0;
                    self.touch();
                }
            });
        });
        heading(ui, &tr("outlinewin.variable_width"));
        ui.horizontal_wrapped(|ui| {
            if ui
                .selectable_label(self.draft.widths.is_empty(), tr("outlinewin.uniform"))
                .clicked()
                && !self.draft.widths.is_empty()
            {
                self.draft.widths.clear();
                self.touch();
            }
            let current = self.width_shape();
            for (i, (key, _)) in WIDTH_SHAPES.iter().enumerate() {
                if ui.selectable_label(current == Some(i), tr(key)).clicked() {
                    self.draft.widths = width_nodes(i, self.draft.width);
                    self.touch();
                }
            }
        });
        heading(ui, &tr("outlinewin.preview"));
        self.preview(ui);
    }

    /// The options under the columns.
    fn options(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            icon_button(
                ui,
                "outlinewin.behind_fill_tip",
                "",
                self.draft.behind_fill,
                false,
            );
            if ui
                .checkbox(&mut self.draft.behind_fill, tr("docker.behind_fill"))
                .on_hover_text(tr("outlinewin.behind_fill_tip"))
                .changed()
            {
                self.touch();
            }
            ui.add_space(16.0);
            icon_button(
                ui,
                "outlinewin.scale_tip",
                "",
                self.draft.scale_with_object,
                false,
            );
            if ui
                .checkbox(
                    &mut self.draft.scale_with_object,
                    tr("docker.scale_with_object"),
                )
                .on_hover_text(tr("outlinewin.scale_tip"))
                .changed()
            {
                self.touch();
            }
            ui.add_space(16.0);
            if ui
                .checkbox(&mut self.overprint, tr("outlinewin.overprint"))
                .on_hover_text(tr("outlinewin.overprint_tip"))
                .changed()
            {
                self.touch();
            }
        });
    }

    /// An arrowhead's thumbnail (kept).
    fn thumb(&mut self, ctx: &Context, a: &Arrowhead, start: bool) -> egui::TextureId {
        let key = (format!("{a:?}"), start);
        self.thumbs
            .entry(key)
            .or_insert_with(|| {
                ctx.load_texture(
                    "outline_arrow_thumb",
                    arrow_thumb(a, start),
                    egui::TextureOptions::LINEAR,
                )
            })
            .id()
    }

    /// A button with the arrowhead's thumbnail; its pop-up lists the
    /// heads. True when one was chosen.
    fn arrow_menu(
        &mut self,
        ui: &mut Ui,
        id: &str,
        start: bool,
        a: &mut Arrowhead,
        custom: &[Arrowhead],
    ) -> bool {
        let cur = self.thumb(ui.ctx(), a, start);
        let tip = tr(if start {
            "outlinewin.arrow_start"
        } else {
            "outlinewin.arrow_end"
        });
        let r = ui
            .add(egui::Button::image(egui::Image::new((cur, ARROW_THUMB))))
            .on_hover_text(format!("{tip}: {}", arrow_name(a, custom)));
        let choices: Vec<(Arrowhead, egui::TextureId)> = Arrowhead::ALL
            .iter()
            .chain(custom.iter())
            .map(|h| (h.clone(), self.thumb(ui.ctx(), h, start)))
            .collect();
        let mut changed = false;
        egui::Popup::menu(&r)
            .id(ui.make_persistent_id(id))
            .close_behavior(egui::PopupCloseBehavior::CloseOnClick)
            .show(|ui| {
                for (h, tex) in choices {
                    let label = arrow_name(&h, custom);
                    if ui
                        .add(
                            egui::Button::image_and_text(egui::Image::new((tex, ARROW_THUMB)), label)
                                .selected(*a == h),
                        )
                        .clicked()
                        && *a != h
                    {
                        *a = h;
                        changed = true;
                    }
                }
            });
        changed
    }

    /// The nib: an ellipse dragged by its tip (turns it) or its side
    /// (flattens it).
    fn nib_view(&mut self, ui: &mut Ui) {
        let (rect, resp) = ui.allocate_exact_size(Vec2::splat(NIB_VIEW), Sense::click_and_drag());
        let p = ui.painter();
        p.rect_filled(rect, 2.0, Color32::WHITE);
        p.rect_stroke(
            rect,
            2.0,
            EStroke::new(1.0, Tokens::control_border()),
            egui::StrokeKind::Inside,
        );
        let c = rect.center();
        let major = NIB_VIEW / 2.0 - 12.0;
        let s = self.draft.stretch.clamp(0.01, 1.0) as f32;
        let a = (self.draft.nib_angle as f32).to_radians();
        let (u, v) = (Vec2::new(a.cos(), -a.sin()), Vec2::new(a.sin(), a.cos()));
        let pts: Vec<egui::Pos2> = (0..64)
            .map(|k| {
                let t = k as f32 / 64.0 * std::f32::consts::TAU;
                c + u * (t.cos() * major) + v * (t.sin() * major * s)
            })
            .collect();
        p.add(egui::Shape::convex_polygon(
            pts,
            Color32::from_gray(40),
            EStroke::new(1.0, Tokens::text_dim()),
        ));
        let tip = c + u * major;
        let side = c + v * (major * s);
        for (q, on) in [
            (tip, self.nib_drag == Some(true)),
            (side, self.nib_drag == Some(false)),
        ] {
            let r = Rect::from_center_size(q, Vec2::splat(6.0));
            p.rect_filled(r, 1.0, if on { Tokens::ACCENT } else { Color32::WHITE });
            p.rect_stroke(
                r,
                1.0,
                EStroke::new(1.0, Tokens::ACCENT),
                egui::StrokeKind::Middle,
            );
        }
        if resp.drag_started() {
            if let Some(q) = ui.input(|i| i.pointer.press_origin()) {
                // Nearer the tip: turn; nearer the side (or inside): flatten.
                self.nib_drag = Some((q - tip).length() < (q - side).length());
            }
        }
        if resp.dragged() {
            if let (Some(q), Some(rotate)) = (resp.interact_pointer_pos(), self.nib_drag) {
                let d = q - c;
                if rotate {
                    if d.length() > 2.0 {
                        let deg = ((-d.y).atan2(d.x) as f64).to_degrees().rem_euclid(360.0);
                        self.draft.nib_angle = deg.round();
                        self.touch();
                    }
                } else {
                    let along = d.dot(v).abs() / major;
                    self.draft.stretch = (along as f64).clamp(0.01, 1.0);
                    self.touch();
                }
            }
        }
        if resp.drag_stopped() {
            self.nib_drag = None;
        }
        resp.on_hover_text(tr("outlinewin.nib_tip"));
    }

    fn preview(&mut self, ui: &mut Ui) {
        let ppp = ui.ctx().pixels_per_point();
        let px = (ppp * 4.0).round().max(1.0) as u32;
        let key = (self.result(), px);
        let stroke = self.result();
        let tex = self.preview.get(ui.ctx(), "outline_window_preview", key, || {
            Some(stroke_preview(stroke.as_ref(), px as f64))
        });
        if let Some(t) = tex {
            let w = ui.available_width().min(280.0);
            ui.add(egui::Image::new((t, Vec2::new(w, w * 42.0 / 70.0))));
        }
    }

    /// The line style editor, while open: a ruler of cells (a click turns
    /// a dash into a gap and back), the number of cells, a sample and a
    /// name; Use, Add, Replace and Close.
    fn dash_editor(&mut self, app: &mut App, ctx: &Context) {
        let Some(ed) = &mut self.editor else {
            return;
        };
        let styles = app.settings.line_styles.clone();
        let mut out = EditorOut::None;
        let (_, closed, _) = sw::floating(ctx, "line_style_editor", &tr("outlinewin.editor_title"), |ui| {
            note(ui, &tr("outlinewin.editor_text"));
            let n = ed.cells.len();
            let (rect, resp) =
                ui.allocate_exact_size(Vec2::new(DASH_CELL * n as f32, DASH_CELL), Sense::click());
            for (i, c) in ed.cells.iter().enumerate() {
                let r = Rect::from_min_size(
                    rect.min + Vec2::new(i as f32 * DASH_CELL, 0.0),
                    Vec2::splat(DASH_CELL),
                )
                .shrink(1.0);
                ui.painter()
                    .rect_filled(r, 0.0, if *c { Tokens::text() } else { Color32::WHITE });
                ui.painter().rect_stroke(
                    r,
                    0.0,
                    EStroke::new(1.0, Tokens::text_dim()),
                    egui::StrokeKind::Inside,
                );
            }
            if resp.clicked() {
                if let Some(q) = resp.interact_pointer_pos() {
                    let i = ((q.x - rect.left()) / DASH_CELL) as usize;
                    if let Some(c) = ed.cells.get_mut(i) {
                        *c = !*c;
                    }
                }
            }
            ui.horizontal(|ui| {
                ui.label(tr("outlinewin.cells"));
                let mut k = ed.cells.len();
                if ui.add(egui::DragValue::new(&mut k).range(2..=48)).changed() {
                    ed.cells.resize(k, false);
                }
            });
            let p = cells_to_pattern(&ed.cells);
            ui.horizontal(|ui| {
                ui.label(tr("outlinewin.preview"));
                pattern_sample(ui, &p);
            });
            ui.horizontal(|ui| {
                ui.label(tr("outlinewin.style_name"));
                ui.add(egui::TextEdit::singleline(&mut ed.name).desired_width(180.0));
            });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button(tr("outlinewin.use")).clicked() {
                    out = EditorOut::Use(p.clone());
                }
                let replace = match self.style {
                    Style::User(i) if i < styles.len() => Some(i),
                    _ => None,
                };
                if ui
                    .add_enabled(replace.is_some(), egui::Button::new(tr("outlinewin.replace")))
                    .on_disabled_hover_text(tr("outlinewin.replace_tip"))
                    .clicked()
                {
                    if let Some(i) = replace {
                        out = EditorOut::Replace(i, p.clone());
                    }
                }
                let name = ed.name.trim().to_string();
                if ui
                    .add_enabled(
                        !name.is_empty() && !p.is_empty(),
                        egui::Button::new(tr("outlinewin.add")),
                    )
                    .on_disabled_hover_text(tr("outlinewin.add_tip"))
                    .clicked()
                {
                    out = EditorOut::Add(name, p.clone());
                }
                if ui.button(tr("outlinewin.close")).clicked() {
                    out = EditorOut::Close;
                }
            });
        });
        if closed {
            out = EditorOut::Close;
        }
        match out {
            EditorOut::None => {}
            EditorOut::Close => self.editor = None,
            EditorOut::Use(p) => {
                self.draft.dash = p;
                self.style = self.detect_style(&styles);
                self.editor = None;
                self.touch();
            }
            EditorOut::Add(name, p) => {
                app.settings.line_styles.push(LineStyle {
                    name,
                    pattern: p.clone(),
                });
                app.save_settings();
                self.style = Style::User(app.settings.line_styles.len() - 1);
                self.draft.dash = p;
                self.editor = None;
                self.touch();
            }
            EditorOut::Replace(i, p) => {
                if let Some(s) = app.settings.line_styles.get_mut(i) {
                    s.pattern = p.clone();
                }
                app.save_settings();
                self.style = Style::User(i);
                self.draft.dash = p;
                self.editor = None;
                self.touch();
            }
        }
    }
}

/// Two reaches equal within a hundredth of a percent.
fn relative(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-4 * a.abs().max(b.abs()).max(1e-6)
}

/// The nodes of the variable width preset `i` for an outline `width` wide.
fn width_nodes(i: usize, width: f64) -> Vec<WidthNode> {
    let half = width.max(Stroke::HAIRLINE) / 2.0;
    WIDTH_SHAPES
        .get(i)
        .map(|(_, p)| {
            p.iter()
                .map(|(at, k)| WidthNode {
                    at: *at,
                    left: k * half,
                    right: k * half,
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Copy into `s` only the settings in which `new` differs from `old`.
pub fn merge_changes(s: &mut Stroke, old: &Stroke, new: &Stroke) {
    macro_rules! take {
        ($($f:ident),*) => {$(
            if new.$f != old.$f {
                s.$f = new.$f.clone();
            }
        )*};
    }
    take!(
        color,
        width,
        cap,
        join,
        dash,
        scale_with_object,
        behind_fill,
        start_arrow,
        end_arrow,
        stretch,
        nib_angle,
        fill,
        widths,
        painterly
    );
}

/// The name of an arrowhead (custom ones are numbered when unnamed).
fn arrow_name(a: &Arrowhead, custom: &[Arrowhead]) -> String {
    match a {
        Arrowhead::None => tr("outlinewin.arrow_none"),
        Arrowhead::Arrow => tr("outlinewin.arrow_arrow"),
        Arrowhead::OpenArrow => tr("outlinewin.arrow_open"),
        Arrowhead::Circle => tr("outlinewin.arrow_circle"),
        Arrowhead::Square => tr("outlinewin.arrow_square"),
        Arrowhead::Bar => tr("outlinewin.arrow_bar"),
        Arrowhead::Diamond => tr("outlinewin.arrow_diamond"),
        Arrowhead::Custom { name, .. } if !name.is_empty() => name.clone(),
        Arrowhead::Custom { .. } => {
            let n = custom.iter().position(|c| c == a).map_or(1, |i| i + 1);
            crate::i18n::trf("docker.custom_arrowhead_n", &[("n", &n.to_string())])
        }
    }
}

fn fmt_num(v: f64) -> String {
    let s = format!("{v:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// A dash pattern's sample in the style list.
fn pattern_sample(ui: &mut Ui, p: &[f64]) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(DASH_SAMPLE_W, 12.0), Sense::hover());
    let color = ui.visuals().text_color();
    let y = rect.center().y;
    let stroke = EStroke::new(2.0, color);
    if p.is_empty() {
        ui.painter().line_segment(
            [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
            stroke,
        );
        return;
    }
    let unit = 2.0;
    let (mut x, mut i) = (rect.left(), 0);
    while x < rect.right() {
        let len = (p[i % p.len()] as f32 * unit).max(1.0);
        if i % 2 == 0 {
            ui.painter().line_segment(
                [egui::pos2(x, y), egui::pos2((x + len).min(rect.right()), y)],
                stroke,
            );
        }
        x += len;
        i += 1;
    }
}

/// A path through `points` (mm, y up).
fn poly(points: &[(f64, f64)], closed: bool) -> BezPath {
    let mut path = BezPath::new();
    for (i, (x, y)) in points.iter().enumerate() {
        let p = Point::new(*x, *y);
        if i == 0 {
            path.move_to(p);
        } else {
            path.line_to(p);
        }
    }
    if closed {
        path.close_path();
    }
    path
}

/// A drawing `w` by `h` mm with `shapes` (path, closed, fill, outline),
/// rendered at `px` pixels per mm on white.
fn render_sample(
    w: f64,
    h: f64,
    px: f64,
    shapes: Vec<(BezPath, bool, Fill, Option<Stroke>)>,
) -> egui::ColorImage {
    let mut doc = Document::new("", Size::new(w, h));
    let page = doc.pages[0].id;
    for (path, closed, fill, stroke) in shapes {
        let id = doc.ids_mut().shape();
        let mut s = Shape::new(id, ShapeKind::Path { path, closed });
        s.fill = fill;
        s.stroke = stroke;
        if let Some(l) = doc.pages[0].layers.first_mut() {
            l.shapes.push(s);
        }
    }
    match graphdraw_render::render_page_image(&doc, page, px * 25.4) {
        Some(pm) => sw::pixmap_image(&pm),
        None => egui::ColorImage::new([1, 1], vec![Color32::WHITE]),
    }
}

/// The preview: a filled triangle (closed corners) and an open zigzag
/// (ends and corners) with the outline.
fn stroke_preview(stroke: Option<&Stroke>, px: f64) -> egui::ColorImage {
    render_sample(
        70.0,
        42.0,
        px,
        vec![
            (
                poly(&[(7.0, 8.0), (31.0, 8.0), (19.0, 34.0)], true),
                true,
                Fill::Solid(Color::rgb8(255, 217, 77)),
                stroke.cloned(),
            ),
            (
                poly(&[(40.0, 10.0), (47.0, 32.0), (55.0, 12.0), (63.0, 32.0)], false),
                false,
                Fill::None,
                stroke.cloned(),
            ),
        ],
    )
}

/// An arrowhead at the start or the end of a short line.
fn arrow_thumb(a: &Arrowhead, start: bool) -> egui::ColorImage {
    let mut st = Stroke::new(Color::BLACK, 0.5);
    if start {
        st.start_arrow = a.clone();
    } else {
        st.end_arrow = a.clone();
    }
    render_sample(
        16.0,
        5.0,
        4.0,
        vec![(poly(&[(2.5, 2.5), (13.5, 2.5)], false), false, Fill::None, Some(st))],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cells_and_patterns_round_trip() {
        let c = |s: &str| s.chars().map(|c| c == '1').collect::<Vec<_>>();
        assert_eq!(cells_to_pattern(&c("111100")), vec![4.0, 2.0]);
        assert_eq!(cells_to_pattern(&c("0111001")), vec![3.0, 2.0, 1.0, 1.0]);
        assert_eq!(
            cells_to_pattern(&c("1110101100")),
            vec![3.0, 1.0, 1.0, 1.0, 2.0, 2.0]
        );
        assert!(cells_to_pattern(&c("0000")).is_empty());
        assert!(cells_to_pattern(&c("1111")).is_empty(), "no gap = solid");
        assert_eq!(pattern_to_cells(&[4.0, 2.0]), c("111100"));
        assert_eq!(
            cells_to_pattern(&pattern_to_cells(&[5.0, 2.0, 1.0, 2.0])),
            vec![5.0, 2.0, 1.0, 2.0]
        );
    }

    /// The window draws all its parts and fits on the screen; the line
    /// style is recognised; a variable width follows a new width.
    #[test]
    fn outline_pen_draws_and_keeps_shapes() {
        let mut app = App::headless();
        let mut s = Stroke::new(Color::rgb8(200, 20, 20), 0.5);
        s.dash = vec![4.0, 2.0];
        s.end_arrow = Arrowhead::Arrow;
        let mut d = OutlineWindow::new(Some(s), false, Vec::new(), Units::Millimeters);
        assert_eq!(d.style, Style::Builtin(1));
        let ctx = Context::default();
        let screen = Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(1366.0, 768.0));
        for k in 0..3 {
            let input = egui::RawInput {
                screen_rect: Some(screen),
                time: Some(k as f64 / 30.0),
                ..Default::default()
            };
            let mut out = ctx.run_ui(input, |ui| {
                if let Outcome::Open(r) = d.show(&mut app, ui.ctx()) {
                    assert!(r.height() < 760.0, "{r:?}");
                }
            });
            out.textures_delta.clear();
        }
        d.draft.widths = width_nodes(2, 0.5);
        assert_eq!(d.width_shape(), Some(2));
        d.draft.width = 1.0;
        d.width_changed(false);
        assert_eq!(d.width_shape(), Some(2));
        assert!((d.draft.widths[1].left - 0.5).abs() < 1e-9);
    }

    /// Only what was changed reaches each object's own outline.
    #[test]
    fn merge_keeps_each_objects_own_settings() {
        let old = Stroke::new(Color::BLACK, 0.5);
        let mut new = old.clone();
        new.cap = LineCap::Round;
        let mut own = Stroke::new(Color::rgb8(0, 0, 255), 2.0);
        merge_changes(&mut own, &old, &new);
        assert_eq!(own.cap, LineCap::Round);
        assert_eq!(own.width, 2.0);
        assert_eq!(own.color, Color::rgb8(0, 0, 255));
    }

    /// The preview and the arrowhead thumbnails render.
    #[test]
    fn samples_render() {
        let s = Stroke::new(Color::BLACK, 1.0);
        let img = stroke_preview(Some(&s), 4.0);
        assert_eq!(img.size, [280, 168]);
        assert!(img.pixels.iter().any(|p| p.r() < 50), "the outline shows");
        let t = arrow_thumb(&Arrowhead::Arrow, false);
        assert_eq!(t.size, [64, 20]);
    }
}
