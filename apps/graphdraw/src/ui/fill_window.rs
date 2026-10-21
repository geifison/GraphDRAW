//! Edit Fill (F11): a floating window to edit the fill of the selected
//! objects (or of new objects, with nothing selected). At the top the
//! modes: no fill, uniform, gradient, vector pattern, bitmap pattern,
//! two-colour pattern, texture and mesh (PostScript fills are not
//! offered). The gradient adds its types, the ramp with the nodes (double
//! click on the band adds one, on a node takes it away, drag to move,
//! Delete takes the chosen one, a palette colour dropped on the band
//! becomes a node), the chosen node's colour and position, and the angle,
//! centre, repetition and edge. At the bottom the new fill over the
//! original one, Help, OK and Cancel. The drawing shows the new fill on
//! the objects while the window is open (see `style_windows`).
//!
//! The gradient also edits each node's transparency and midpoint, the
//! steps, blending around the colour wheel, smoothing, size and the
//! elliptical type. Controls whose settings the drawing does not keep yet
//! (a uniform fill's transparency, editable two-colour cells, tile
//! rotation and offsets, texture detail and contrast) are shown dimmed.

use crate::app::App;
use crate::i18n::tr;
use crate::theme::Tokens;
use crate::ui::style_windows::{
    self as sw, checker, color32, color_popup, icon_button, none_mark, note,
    Cached, ColorPicker, Eyedropper, Footer, Outcome, PaletteList, SwatchDrop,
};
use egui::{Color32, Context, Rect, Sense, Stroke, Ui, Vec2};
use graphdraw_core::geometry::Size;
use graphdraw_core::{
    Color, Command, Document, Fill, Fountain, FountainKind, FountainRepeat, Mesh, Pattern,
    PatternTile, ShapeId, Stop, Texture, TextureKind, TileLayout,
};

/// The body's width.
const WIDTH: f32 = 560.0;
/// The ramp: midpoint row (kept for the midpoints), band and node row.
const RAMP_MID_ROW: f32 = 12.0;
const RAMP_HEIGHT: f32 = 24.0;
const RAMP_NODE_ROW: f32 = 16.0;
const RAMP_NODE: f32 = 10.0;
/// How near the pointer must be to a node to take it.
const HANDLE_HIT: f32 = 8.0;
/// The new and original samples.
const COMPARE_W: f32 = 132.0;
const COMPARE_H: f32 = 56.0;
/// A cell of the two-colour tile picture.
const PATTERN_CELL: f32 = 14.0;
/// The body scrolls past this height.
const BODY_MAX: f32 = 470.0;

/// The fill's mode (the buttons at the top).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    None,
    Solid,
    Gradient,
    Vector,
    Bitmap,
    TwoColor,
    Texture,
    Mesh,
}

impl Mode {
    /// The name key, which also names the icon (see `svg_icons`).
    fn key(self) -> &'static str {
        match self {
            Mode::None => "fillwin.mode_none",
            Mode::Solid => "fillwin.mode_solid",
            Mode::Gradient => "fillwin.mode_gradient",
            Mode::Vector => "fillwin.mode_vector",
            Mode::Bitmap => "fillwin.mode_bitmap",
            Mode::TwoColor => "fillwin.mode_two_color",
            Mode::Texture => "fillwin.mode_texture",
            Mode::Mesh => "fillwin.mode_mesh",
        }
    }

    const ALL: [Mode; 8] = [
        Mode::None,
        Mode::Solid,
        Mode::Gradient,
        Mode::Vector,
        Mode::Bitmap,
        Mode::TwoColor,
        Mode::Texture,
        Mode::Mesh,
    ];
}

/// The gradient types of the sub-bar: kind (always present) and name
/// key, which also names the icon.
const KINDS: [(Option<FountainKind>, &str); 6] = [
    (Some(FountainKind::Linear), "fill.linear"),
    (Some(FountainKind::Radial), "fill.radial"),
    (Some(FountainKind::Elliptical), "fillwin.elliptic"),
    (Some(FountainKind::Conical), "fill.conical"),
    (Some(FountainKind::Square), "fill.square"),
    (Some(FountainKind::Shaped), "fill.shaped"),
];

/// The i18n key of a two-colour tile's name.
fn tile_key(t: PatternTile) -> &'static str {
    match t {
        PatternTile::Checker => "fillwin.tile_checker",
        PatternTile::Dots => "fillwin.tile_dots",
        PatternTile::Stripes => "fillwin.tile_stripes",
        PatternTile::DiagonalStripes => "fillwin.tile_diagonal",
        PatternTile::Grid => "fillwin.tile_grid",
        PatternTile::Bricks => "fillwin.tile_bricks",
        PatternTile::Crosshatch => "fillwin.tile_crosshatch",
        PatternTile::Waves => "fillwin.tile_waves",
        PatternTile::Cells(_) => "fillwin.tile_custom",
    }
}

/// The colour the eyedropper (or a picker) changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Target {
    Solid,
    Stop,
    Front,
    Back,
}

/// The two-colour pattern being edited.
#[derive(Debug, Clone, PartialEq)]
struct TwoColor {
    tile: PatternTile,
    front: Color,
    back: Color,
    size_mm: f64,
    layout: TileLayout,
}

pub struct FillWindow {
    /// The objects edited; empty: the defaults of new objects.
    targets: Vec<ShapeId>,
    /// The selection last seen (a new one reloads an unchanged window).
    seen: Vec<ShapeId>,
    initial: Fill,
    initial_overprint: bool,
    /// The fill's own opacity when the window opened (1.0 opaque).
    initial_fill_opacity: f64,
    /// The fill's own opacity (Transparency is 1 less it, in percent).
    pub(crate) fill_opacity: f64,
    pub(crate) mode: Mode,
    pub(crate) color: Color,
    pub(crate) fountain: Fountain,
    pub(crate) sel_stop: usize,
    two_color: TwoColor,
    bitmap: Option<Pattern>,
    vector: Option<Pattern>,
    texture: Texture,
    mesh: Mesh,
    pub(crate) overprint: bool,
    pub(crate) changed: bool,
    /// Counts the changes (the canvas preview follows it).
    rev: u64,
    /// The first object's size (mm), for the samples' proportions.
    size: Size,
    palettes: PaletteList,
    picker: ColorPicker,
    eye: Eyedropper,
    eye_target: Target,
    drag: Option<usize>,
    /// The ramp's band last frame (tests).
    pub(crate) ramp_bar: Rect,
    /// The pointer was over the window last frame.
    hovered: bool,
    sample: Cached<(Fill, u32, u32)>,
    before: Cached<(u32, u32)>,
}

fn stop(pos: f64, color: Color) -> Stop {
    Stop::new(pos, color)
}

impl FillWindow {
    /// The window for the selection (or the defaults, with nothing
    /// selected).
    pub fn open(app: &App) -> Box<FillWindow> {
        let targets = sw::targets(app);
        let (initial, overprint, size) = match app.selected_shapes().first() {
            Some(s) => {
                let leaf = sw::first_leaf(s);
                let b = leaf.bounds();
                (
                    leaf.fill.clone(),
                    leaf.overprint_fill,
                    Size::new(b.width().max(1.0), b.height().max(1.0)),
                )
            }
            None => (
                app.doc().defaults.graphic.fill.clone(),
                false,
                Size::new(60.0, 30.0),
            ),
        };
        let fill_opacity = app
            .selected_shapes()
            .first()
            .map_or(1.0, |s| sw::first_leaf(s).fill_opacity);
        let mut w = FillWindow::new(initial, overprint, targets, size);
        w.set_initial_fill_opacity(fill_opacity);
        w.seen = app.selection.clone();
        w.palettes = sw::palettes_of(app);
        Box::new(w)
    }

    pub(crate) fn new(initial: Fill, overprint: bool, targets: Vec<ShapeId>, size: Size) -> Self {
        let white = Color::cmyk_pct(0.0, 0.0, 0.0, 0.0);
        let black = Color::cmyk_pct(0.0, 0.0, 0.0, 100.0);
        let mut w = FillWindow {
            targets,
            seen: Vec::new(),
            initial: initial.clone(),
            initial_overprint: overprint,
            initial_fill_opacity: 1.0,
            fill_opacity: 1.0,
            mode: Mode::None,
            color: white,
            fountain: Fountain::two(FountainKind::Linear, black, white, 0.0),
            sel_stop: 0,
            two_color: TwoColor {
                tile: PatternTile::Checker,
                front: black,
                back: white,
                size_mm: 10.0,
                layout: TileLayout::default(),
            },
            bitmap: None,
            vector: None,
            texture: Texture {
                kind: TextureKind::Clouds,
                color_a: black,
                color_b: white,
                scale: 20.0,
                seed: 1,
                ..Default::default()
            },
            mesh: Mesh::new(
                graphdraw_core::geometry::Rect::new(0.0, 0.0, 100.0, 100.0),
                2,
                2,
                white,
            ),
            overprint,
            changed: false,
            rev: 1,
            size,
            palettes: Vec::new(),
            picker: ColorPicker::default(),
            eye: Eyedropper::default(),
            eye_target: Target::Solid,
            drag: None,
            ramp_bar: Rect::NOTHING,
            hovered: false,
            sample: Cached::default(),
            before: Cached::default(),
        };
        match initial {
            Fill::None => {}
            Fill::Solid(c) => {
                w.mode = Mode::Solid;
                w.color = c;
            }
            Fill::Fountain(f) => {
                w.mode = Mode::Gradient;
                w.color = f.first_color();
                w.fountain = f;
                if w.fountain.stops.len() < 2 {
                    w.fountain.stops = w.fountain.sorted_stops();
                }
                w.sort_stops();
            }
            Fill::Pattern(p @ Pattern::TwoColor { .. }) => {
                if let Pattern::TwoColor {
                    tile,
                    front,
                    back,
                    size_mm,
                    layout,
                } = p
                {
                    w.mode = Mode::TwoColor;
                    w.two_color = TwoColor {
                        tile,
                        front,
                        back,
                        size_mm,
                        layout,
                    };
                }
            }
            Fill::Pattern(p @ Pattern::Bitmap { .. }) => {
                w.mode = Mode::Bitmap;
                w.bitmap = Some(p);
            }
            Fill::Pattern(p @ Pattern::Vector { .. }) => {
                w.mode = Mode::Vector;
                w.vector = Some(p);
            }
            Fill::Texture(t) => {
                w.mode = Mode::Texture;
                w.texture = t;
            }
            Fill::Mesh(m) => {
                w.mode = Mode::Mesh;
                w.mesh = m;
            }
        }
        w
    }

    /// The fill transparency the edited objects have (1.0 opaque).
    pub(crate) fn set_initial_fill_opacity(&mut self, o: f64) {
        let o = if o.is_nan() { 1.0 } else { o.clamp(0.0, 1.0) };
        self.initial_fill_opacity = o;
        self.fill_opacity = o;
    }

    pub fn revision(&self) -> u64 {
        self.rev
    }

    /// The window shows its fill on the drawing: once changed, on objects.
    pub fn previews(&self) -> bool {
        self.changed && !self.targets.is_empty()
    }

    fn touch(&mut self) {
        self.changed = true;
        self.rev = self.rev.wrapping_add(1);
    }

    /// The fill the window makes.
    pub(crate) fn result(&self) -> Fill {
        match self.mode {
            Mode::None => Fill::None,
            Mode::Solid => Fill::Solid(self.color),
            Mode::Gradient => Fill::Fountain(self.fountain.clone()),
            Mode::TwoColor => Fill::Pattern(Pattern::TwoColor {
                tile: self.two_color.tile,
                front: self.two_color.front,
                back: self.two_color.back,
                size_mm: self.two_color.size_mm,
                layout: self.two_color.layout,
            }),
            Mode::Bitmap => self.bitmap.clone().map_or(Fill::None, Fill::Pattern),
            Mode::Vector => Fill::Pattern(self.vector.clone().unwrap_or(Pattern::Vector {
                shapes: Vec::new(),
                tile: Size::new(10.0, 10.0),
                layout: Default::default(),
            })),
            Mode::Texture => Fill::Texture(self.texture.clone()),
            Mode::Mesh => Fill::Mesh(self.mesh.clone()),
        }
    }

    /// The commands OK runs on the objects (the preview runs them on a
    /// copy of the drawing).
    pub fn commands(&self, _doc: &Document) -> Vec<Command> {
        if !self.changed || self.targets.is_empty() {
            return Vec::new();
        }
        let mut v = vec![Command::SetFill {
            shapes: self.targets.clone(),
            fill: self.result(),
        }];
        if self.overprint != self.initial_overprint {
            v.push(Command::SetOverprint {
                shapes: self.targets.clone(),
                fill: Some(self.overprint),
                outline: None,
            });
        }
        if (self.fill_opacity - self.initial_fill_opacity).abs() > 1e-9 {
            v.push(Command::SetFillOpacity {
                shapes: self.targets.clone(),
                opacity: self.fill_opacity,
            });
        }
        v
    }

    /// OK: the objects get the fill (one undo step), or, with nothing
    /// selected, the defaults of new objects.
    pub fn apply(&self, app: &mut App) {
        if !self.changed {
            return;
        }
        if self.targets.is_empty() {
            app.request_default_change(crate::defaults::DefaultChange::Fill(self.result()));
            return;
        }
        if let Fill::Solid(c) = self.result() {
            app.current_colors.0 = c;
        }
        sw::run_as_one_step(app, self.commands(app.doc()));
    }

    /// Keep the stops in order, the chosen one still chosen.
    fn sort_stops(&mut self) {
        let n = self.fountain.stops.len();
        if n == 0 {
            self.fountain.stops = vec![stop(0.0, Color::BLACK), stop(1.0, Color::WHITE)];
        }
        let sel = self.sel_stop.min(self.fountain.stops.len().saturating_sub(1));
        let mut v: Vec<(Stop, bool)> = self
            .fountain
            .stops
            .drain(..)
            .enumerate()
            .map(|(i, s)| (s, i == sel))
            .collect();
        v.sort_by(|a, b| a.0.pos.total_cmp(&b.0.pos));
        self.sel_stop = v.iter().position(|(_, s)| *s).unwrap_or(0);
        self.fountain.stops = v.into_iter().map(|(s, _)| s).collect();
    }

    /// A new node at `at` (0 to 1) with `color`, or the gradient's own
    /// colour there.
    pub(crate) fn add_stop(&mut self, at: f64, color: Option<Color>) {
        let at = at.clamp(0.0, 1.0);
        let c = color.unwrap_or_else(|| {
            graphdraw_core::style::sample_stops(&self.fountain.sorted_stops(), at)
        });
        self.fountain.stops.push(stop(at, c));
        self.sel_stop = self.fountain.stops.len() - 1;
        self.sort_stops();
        self.touch();
    }

    /// Only the nodes between the ends can go, and two always stay.
    fn removable(&self, i: usize) -> bool {
        self.fountain.stops.len() > 2 && i > 0 && i + 1 < self.fountain.stops.len()
    }

    pub(crate) fn remove_stop(&mut self, i: usize) {
        if self.removable(i) {
            self.fountain.stops.remove(i);
            self.sel_stop = i.min(self.fountain.stops.len() - 1);
            self.touch();
        }
    }

    /// The first node goes to the end and the order mirrors.
    pub(crate) fn reverse(&mut self) {
        let n = self.fountain.stops.len();
        if n == 0 {
            return;
        }
        let old = self.fountain.stops.clone();
        self.fountain.stops = (0..n)
            .map(|j| {
                let k = n - 1 - j;
                let mut s = old[k];
                s.pos = 1.0 - s.pos;
                // Each run's midpoint mirrors with it.
                s.midpoint = if k > 0 {
                    1.0 - old[k - 1].midpoint
                } else {
                    0.5
                };
                s
            })
            .collect();
        self.sel_stop = n - 1 - self.sel_stop.min(n - 1);
        self.touch();
    }

    pub(crate) fn set_mode(&mut self, m: Mode) {
        if self.mode == m {
            return;
        }
        // A gradient starts from the uniform colour, and back.
        if m == Mode::Gradient && self.mode == Mode::Solid {
            if let Some(s) = self.fountain.stops.first_mut() {
                s.color = self.color;
            }
        }
        if m == Mode::Solid && self.mode == Mode::Gradient {
            if let Some(s) = self.fountain.stops.get(self.sel_stop) {
                self.color = s.color;
            }
        }
        if m == Mode::Mesh {
            self.mesh = Mesh::new(
                graphdraw_core::geometry::Rect::new(0.0, 0.0, self.size.width, self.size.height),
                2,
                2,
                self.color,
            );
        }
        self.mode = m;
        self.touch();
    }

    fn target_color(&self, t: Target) -> Color {
        match t {
            Target::Solid => self.color,
            Target::Stop => self
                .fountain
                .stops
                .get(self.sel_stop)
                .map_or(Color::BLACK, |s| s.color),
            Target::Front => match self.mode {
                Mode::Texture => self.texture.color_a,
                _ => self.two_color.front,
            },
            Target::Back => match self.mode {
                Mode::Texture => self.texture.color_b,
                _ => self.two_color.back,
            },
        }
    }

    fn set_target(&mut self, t: Target, c: Color) {
        match t {
            Target::Solid => self.color = c,
            Target::Stop => {
                if let Some(s) = self.fountain.stops.get_mut(self.sel_stop) {
                    s.color = c;
                }
            }
            Target::Front => match self.mode {
                Mode::Texture => self.texture.color_a = c,
                _ => self.two_color.front = c,
            },
            Target::Back => match self.mode {
                Mode::Texture => self.texture.color_b = c,
                _ => self.two_color.back = c,
            },
        }
        self.touch();
    }

    /// Reload for a new selection while nothing was changed.
    fn follow_selection(&mut self, app: &App) {
        if app.selection == self.seen {
            return;
        }
        if self.changed {
            self.seen = app.selection.clone();
            return;
        }
        let mut fresh = *FillWindow::open(app);
        fresh.picker = std::mem::take(&mut self.picker);
        *self = fresh;
    }

    /// Draw the window.
    pub fn show(&mut self, app: &mut App, ctx: &Context) -> Outcome {
        self.follow_selection(app);
        if self.eye.active() {
            if let Some(c) = self.eye.run(ctx) {
                let t = self.eye_target;
                self.set_target(t, c);
            }
            return Outcome::Open(Rect::NOTHING);
        }
        let title = if self.targets.is_empty() {
            tr("fillwin.title_defaults")
        } else {
            tr("fillwin.title")
        };
        let mut picked_image = false;
        let (out, closed, rect) = sw::floating(ctx, "fill", &title, |ui| {
            ui.set_width(WIDTH);
            self.mode_bar(ui);
            if self.mode == Mode::Gradient {
                self.kind_bar(ui);
            }
            ui.separator();
            egui::ScrollArea::vertical()
                .id_salt("fill_window_body")
                .max_height(BODY_MAX)
                .auto_shrink([false, true])
                .show(ui, |ui| match self.mode {
                    Mode::None => {
                        ui.add_space(16.0);
                        note(ui, &tr("fillwin.none_text"));
                        ui.add_space(16.0);
                    }
                    Mode::Solid => self.solid_ui(ui),
                    Mode::Gradient => self.gradient_ui(ui),
                    Mode::Mesh => self.mesh_ui(ui),
                    _ => picked_image = self.pattern_ui(ui, app),
                });
            ui.separator();
            let help = tr(help_key(self.mode));
            let hovered = self.hovered;
            sw::footer(ui, &help, hovered, |ui| {
                self.compare(ui);
                if !self.targets.is_empty()
                    && ui
                        .small_button(tr("stylewin.in_docker"))
                        .on_hover_text(tr("stylewin.in_docker_tip"))
                        .clicked()
                {
                    app.open_fill_editor();
                }
            })
        });
        let _ = picked_image;
        self.hovered = ctx
            .pointer_latest_pos()
            .is_some_and(|p| rect.contains(p));
        if closed {
            return Outcome::Cancel;
        }
        match out.flatten() {
            Some(Footer::Ok) => Outcome::Ok,
            Some(Footer::Cancel) => Outcome::Cancel,
            None => Outcome::Open(rect),
        }
    }

    fn mode_bar(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            for m in Mode::ALL {
                let name = m.key();
                // The mesh is made on the objects (the Mesh Fill tool);
                // here only when the fill is one already.
                let on = m != Mode::Mesh || self.mode == Mode::Mesh;
                let r = icon_button(ui, name, &tr(name), self.mode == m, on);
                if on && r.clicked() {
                    self.set_mode(m);
                }
            }
            icon_button(
                ui,
                "fillwin.postscript_tip",
                &tr("fillwin.postscript_tip"),
                false,
                false,
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.strong(tr(self.mode.key()));
            });
        });
    }

    fn kind_bar(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            ui.add_space(8.0);
            for (k, name) in KINDS {
                let on = k.is_some();
                let tip = if on {
                    tr(name)
                } else {
                    format!("{} ({})", tr(name), tr("stylewin.coming"))
                };
                let r = icon_button(ui, name, &tip, k == Some(self.fountain.kind), on);
                if let Some(k) = k {
                    if r.clicked() && self.fountain.kind != k {
                        self.fountain.kind = k;
                        self.touch();
                    }
                }
            }
            ui.add_space(6.0);
            note(
                ui,
                &tr(crate::ui::dockers::fountain_kind_key(self.fountain.kind)),
            );
        });
    }

    /// The picker inline, then transparency and overprint.
    fn solid_ui(&mut self, ui: &mut Ui) {
        let o = self.picker.show(ui, "solid", self.color, &self.palettes);
        if let Some(c) = o.color {
            self.set_target(Target::Solid, c);
        }
        if o.eyedrop {
            self.eye.start();
            self.eye_target = Target::Solid;
        }
        ui.add_space(8.0);
        self.transparency_slider(ui);
        self.overprint_box(ui);
    }

    /// The fill's own transparency, 0 (opaque) to 100 %; objects only
    /// (new objects get an opaque fill).
    fn transparency_slider(&mut self, ui: &mut Ui) {
        let objects = !self.targets.is_empty();
        let r = ui
            .add_enabled_ui(objects, |ui| {
                ui.horizontal(|ui| {
                    ui.label(tr("fillwin.transparency"));
                    let mut tp = ((1.0 - self.fill_opacity) * 100.0).clamp(0.0, 100.0);
                    let changed = ui
                        .add(egui::Slider::new(&mut tp, 0.0..=100.0).suffix(" %"))
                        .on_hover_text(tr("fillwin.transparency_tip"))
                        .changed();
                    if changed {
                        self.fill_opacity = (1.0 - tp / 100.0).clamp(0.0, 1.0);
                        self.touch();
                    }
                });
            })
            .response;
        r.on_disabled_hover_text(tr("fillwin.transparency_objects"));
    }

    fn overprint_box(&mut self, ui: &mut Ui) {
        if ui
            .checkbox(&mut self.overprint, tr("fillwin.overprint"))
            .on_hover_text(tr("fillwin.overprint_tip"))
            .changed()
        {
            self.touch();
        }
    }

    fn color_popup(&mut self, ui: &mut Ui, id: &str, t: Target) {
        let mut eye = false;
        let cur = self.target_color(t);
        if let Some(c) = color_popup(ui, id, cur, &mut self.picker, &self.palettes, &mut eye) {
            self.set_target(t, c);
        }
        if eye {
            self.eye.start();
            self.eye_target = t;
        }
    }

    fn gradient_ui(&mut self, ui: &mut Ui) {
        self.ramp(ui);
        ui.add_space(8.0);
        // The chosen node: colour, position and transparency.
        ui.horizontal(|ui| {
            ui.label(tr("fillwin.node"));
            self.color_popup(ui, "node", Target::Stop);
            ui.label(tr("fillwin.position"));
            let i = self.sel_stop.min(self.fountain.stops.len().saturating_sub(1));
            let pos = self.fountain.stops.get(i).map_or(0.0, |s| s.pos) * 100.0;
            if let Some(v) = crate::measure_field::number_field(ui, "node_pos", pos, 1, " %", 64.0)
            {
                if let Some(s) = self.fountain.stops.get_mut(i) {
                    s.pos = (v / 100.0).clamp(0.0, 1.0);
                }
                self.sort_stops();
                self.touch();
            }
            // The chosen node's transparency and the midpoint of its run
            // to the next node.
            ui.label(tr("fillwin.transparency"));
            let op = self.fountain.stops.get(i).map_or(1.0, |s| s.opacity);
            let mut tp = ((1.0 - op) * 100.0).round();
            if ui
                .add(egui::Slider::new(&mut tp, 0.0..=100.0).suffix("%"))
                .changed()
            {
                if let Some(s) = self.fountain.stops.get_mut(i) {
                    s.opacity = (1.0 - tp / 100.0).clamp(0.0, 1.0);
                }
                self.touch();
            }
            ui.label(tr("gradtool.midpoint"));
            let mid = self.fountain.stops.get(i).map_or(0.5, |s| s.midpoint) * 100.0;
            if let Some(v) = crate::measure_field::number_field(ui, "node_mid", mid, 0, "%", 52.0)
            {
                if let Some(s) = self.fountain.stops.get_mut(i) {
                    s.midpoint = (v / 100.0).clamp(0.01, 0.99);
                }
                self.touch();
            }
        });
        self.presets(ui);
        ui.separator();
        self.transform_ui(ui);
    }

    /// The built-in gradients and the current colours: a click replaces
    /// the nodes.
    fn presets(&mut self, ui: &mut Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::splat(3.0);
            for (i, (key, _)) in crate::gradient_presets::PRESETS.iter().enumerate() {
                let Some(stops) = crate::gradient_presets::stops(i) else {
                    continue;
                };
                let (r, resp) = ui.allocate_exact_size(Vec2::new(28.0, 16.0), Sense::click());
                band(ui.painter(), r, &stops);
                let line = if resp.hovered() {
                    Tokens::ACCENT
                } else {
                    Tokens::border()
                };
                ui.painter()
                    .rect_stroke(r, 0.0, Stroke::new(1.0, line), egui::StrokeKind::Inside);
                if resp.on_hover_text(tr(key)).clicked() {
                    self.fountain.stops = stops;
                    if let Some(a) = crate::gradient_presets::angle(i)
                        .filter(|_| self.fountain.kind == FountainKind::Linear)
                    {
                        self.fountain.angle = a;
                    }
                    self.sel_stop = 0;
                    self.touch();
                }
            }
        });
    }

    /// Angle, size, centre, steps, repetition, blending, smoothing, edge,
    /// and Reverse.
    fn transform_ui(&mut self, ui: &mut Ui) {
        let mut ch = false;
        let kind = self.fountain.kind;
        egui::Grid::new("fill_window_matrix")
            .num_columns(4)
            .spacing([12.0, 8.0])
            .show(ui, |ui| {
                let f = &mut self.fountain;
                ui.label(tr("fillwin.angle"));
                let angled = matches!(
                    kind,
                    FountainKind::Linear
                        | FountainKind::Conical
                        | FountainKind::Square
                        | FountainKind::Elliptical
                );
                ui.add_enabled_ui(angled, |ui| {
                    ui.horizontal(|ui| {
                        ch |= sw::angle_dial(ui, &mut f.angle);
                        if let Some(v) =
                            crate::measure_field::number_field(ui, "angle", f.angle, 1, "°", 64.0)
                        {
                            f.angle = v.clamp(-360.0, 360.0);
                            ch = true;
                        }
                    });
                });
                ui.label(tr("fillwin.size"));
                let sized = !matches!(kind, FountainKind::Conical | FountainKind::Shaped);
                ui.add_enabled_ui(sized, |ui| {
                    if let Some(v) = crate::measure_field::number_field(
                        ui,
                        "size",
                        f.scale * 100.0,
                        0,
                        "%",
                        64.0,
                    ) {
                        f.scale = (v / 100.0).clamp(
                            graphdraw_core::gradient::MIN_SCALE,
                            graphdraw_core::gradient::MAX_SCALE,
                        );
                        ch = true;
                    }
                });
                ui.end_row();
                let centred = !matches!(kind, FountainKind::Linear | FountainKind::Shaped);
                for (k, key) in [(0, "fillwin.center_x"), (1, "fillwin.center_y")] {
                    ui.label(tr(key));
                    ui.add_enabled_ui(centred, |ui| {
                        let v = if k == 0 { f.offset.x } else { f.offset.y } * 100.0;
                        if let Some(n) = crate::measure_field::number_field(
                            ui,
                            &format!("centre{k}"),
                            v,
                            0,
                            " %",
                            64.0,
                        ) {
                            let n = (n / 100.0).clamp(-1.0, 1.0);
                            if k == 0 {
                                f.offset.x = n;
                            } else {
                                f.offset.y = n;
                            }
                            ch = true;
                        }
                    })
                    .response
                    .on_hover_text(tr("fillwin.center_tip"));
                }
                ui.end_row();
                ui.label(tr("fillwin.steps"));
                ui.horizontal(|ui| {
                    let locked = f.steps > 0;
                    if icon_button(
                        ui,
                        "fillwin.steps_lock",
                        &tr("fillwin.steps_lock"),
                        locked,
                        true,
                    )
                    .clicked()
                    {
                        f.steps = if locked { 0 } else { 16 };
                        ch = true;
                    }
                    let mut n = if locked { f.steps } else { 16 };
                    let r = ui.add_enabled(
                        locked,
                        egui::DragValue::new(&mut n).range(2..=graphdraw_core::gradient::MAX_STEPS),
                    );
                    if r.changed() {
                        f.steps = n;
                        ch = true;
                    }
                    if !locked {
                        ui.label(tr("fillwin.continuous"));
                    }
                });
                ui.label(tr("fillwin.repeat"));
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    for (m, tip) in [
                        (FountainRepeat::Mirror, "fillwin.reflect_tip"),
                        (FountainRepeat::Repeat, "fillwin.repeat_tip"),
                    ] {
                        if icon_button(ui, tip, &tr(tip), f.repeat == m, true).clicked() {
                            if f.repeat == m {
                                f.repeat = FountainRepeat::None;
                            } else {
                                f.repeat = m;
                                if f.repeats < 2 {
                                    f.repeats = 2;
                                }
                            }
                            ch = true;
                        }
                    }
                    ui.add_enabled_ui(f.repeat != FountainRepeat::None, |ui| {
                        let mut n = f.repeats;
                        if ui
                            .add(egui::DragValue::new(&mut n).range(1..=Fountain::MAX_REPEATS))
                            .on_hover_text(tr("docker.repeat_count"))
                            .changed()
                        {
                            f.repeats = n;
                            ch = true;
                        }
                    });
                });
                ui.end_row();
                ui.label(tr("fillwin.blend"));
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    for (b, tip) in [
                        (graphdraw_core::FountainBlend::Direct, "fillwin.blend_direct"),
                        (graphdraw_core::FountainBlend::Clockwise, "fillwin.blend_cw"),
                        (graphdraw_core::FountainBlend::Counterclockwise, "fillwin.blend_ccw"),
                    ] {
                        if icon_button(ui, tip, &tr(tip), f.blend == b, true).clicked()
                            && f.blend != b
                        {
                            f.blend = b;
                            ch = true;
                        }
                    }
                });
                ui.label(tr("fillwin.smooth"));
                if icon_button(
                    ui,
                    "fillwin.smooth_tip",
                    &tr("fillwin.smooth_tip"),
                    f.smooth,
                    true,
                )
                .clicked()
                {
                    f.smooth = !f.smooth;
                    ch = true;
                }
                ui.end_row();
                ui.label(tr("docker.edge_pad"));
                if let Some(v) = crate::measure_field::number_field(
                    ui,
                    "edge",
                    f.edge_pad * 100.0,
                    0,
                    " %",
                    64.0,
                ) {
                    f.edge_pad = (v / 100.0).clamp(0.0, 0.49);
                    ch = true;
                }
                ui.label("");
                ui.horizontal(|ui| {
                    ch |= ui
                        .checkbox(&mut f.bilinear, tr("docker.bilinear"))
                        .on_hover_text(tr("docker.bilinear_tip"))
                        .changed();
                    ch |= ui
                        .checkbox(&mut f.invert, tr("docker.invert_colours"))
                        .on_hover_text(tr("docker.invert_colours_tip"))
                        .changed();
                });
                ui.end_row();
            });
        if ch {
            self.touch();
        }
        let reverse = ui
            .horizontal(|ui| {
                let a = icon_button(
                    ui,
                    "fillwin.reverse_tip",
                    &tr("fillwin.reverse_tip"),
                    false,
                    true,
                );
                let b = ui
                    .button(tr("fillwin.reverse"))
                    .on_hover_text(tr("fillwin.reverse_tip"));
                a.clicked() || b.clicked()
            })
            .inner;
        if reverse {
            self.reverse();
        }
    }

    /// The ramp: the gradient's band with its nodes under it. Double click
    /// on the band adds a node, on a node takes it away; drag to move;
    /// Delete takes the chosen node; a palette colour dropped on the band
    /// becomes a node.
    fn ramp(&mut self, ui: &mut Ui) {
        let w = ui.available_width().min(WIDTH);
        let (rect, resp) = ui.allocate_exact_size(
            Vec2::new(w, RAMP_MID_ROW + RAMP_HEIGHT + RAMP_NODE_ROW),
            Sense::click_and_drag(),
        );
        let pad = RAMP_NODE / 2.0 + 1.0;
        let bar = Rect::from_min_size(
            rect.min + Vec2::new(pad, RAMP_MID_ROW),
            Vec2::new(w - 2.0 * pad, RAMP_HEIGHT),
        );
        self.ramp_bar = bar;
        let x_of = |o: f64| bar.left() + (o.clamp(0.0, 1.0) as f32) * bar.width();
        let t_of = |x: f32| ((x - bar.left()) / bar.width()).clamp(0.0, 1.0) as f64;
        let p = ui.painter();
        checker(p, bar);
        band(p, bar, &self.fountain.sorted_stops());
        let hovering = resp.dnd_hover_payload::<SwatchDrop>().is_some();
        p.rect_stroke(
            bar,
            0.0,
            if hovering {
                Stroke::new(1.5, Tokens::ACCENT)
            } else {
                Stroke::new(1.0, Tokens::control_border())
            },
            egui::StrokeKind::Outside,
        );
        let stops = self.fountain.stops.clone();
        // The nodes: squares under the band, joined to it by a line.
        let node_y = bar.bottom() + RAMP_NODE_ROW / 2.0;
        for (k, s) in stops.iter().enumerate() {
            let x = x_of(s.pos);
            p.line_segment(
                [egui::pos2(x, bar.bottom()), egui::pos2(x, node_y)],
                Stroke::new(1.0, Tokens::text_dim()),
            );
            let r = Rect::from_center_size(egui::pos2(x, node_y), Vec2::splat(RAMP_NODE));
            p.rect_filled(r, 0.0, color32(s.color));
            let st = if k == self.sel_stop {
                Stroke::new(1.5, Tokens::ACCENT)
            } else {
                Stroke::new(1.0, Tokens::text())
            };
            p.rect_stroke(r, 0.0, st, egui::StrokeKind::Outside);
        }
        let hit = |q: egui::Pos2| -> Option<usize> {
            if q.y < bar.bottom() - 2.0 {
                return None;
            }
            stops
                .iter()
                .enumerate()
                .map(|(k, s)| (k, (x_of(s.pos) - q.x).abs()))
                .filter(|(_, d)| *d <= HANDLE_HIT)
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(k, _)| k)
        };
        if resp.drag_started() {
            if let Some(q) = ui.input(|i| i.pointer.press_origin()) {
                self.drag = hit(q);
                if let Some(k) = self.drag {
                    self.sel_stop = k;
                }
            }
        }
        if resp.dragged() && self.drag.is_some() {
            if let Some(q) = resp.interact_pointer_pos() {
                if let Some(s) = self.fountain.stops.get_mut(self.sel_stop) {
                    s.pos = t_of(q.x);
                }
                self.sort_stops();
                self.drag = Some(self.sel_stop);
                self.touch();
            }
        }
        if resp.drag_stopped() {
            self.drag = None;
        }
        if resp.double_clicked() {
            if let Some(q) = resp.interact_pointer_pos() {
                match hit(q) {
                    Some(k) => self.remove_stop(k),
                    None if bar.expand(2.0).contains(q) => self.add_stop(t_of(q.x), None),
                    None => {}
                }
            }
        } else if resp.clicked() {
            if let Some(k) = resp.interact_pointer_pos().and_then(hit) {
                self.sel_stop = k;
            }
        }
        if let Some(c) = resp.dnd_release_payload::<SwatchDrop>() {
            if let Some(q) = ui.input(|i| i.pointer.interact_pos()) {
                self.add_stop(t_of(q.x), Some(c.0));
            }
        }
        // Delete with the pointer over the window and no field being
        // typed in: the chosen node goes.
        if self.hovered
            && ui.memory(|m| m.focused().is_none())
            && ui.input(|i| i.key_pressed(egui::Key::Delete))
        {
            self.remove_stop(self.sel_stop);
        }
        resp.on_hover_text(tr("fillwin.ramp_tip"));
    }

    fn mesh_ui(&mut self, ui: &mut Ui) {
        let m = &mut self.mesh;
        let mut ch = false;
        ui.horizontal(|ui| {
            ui.label(tr("docker.grid"));
            let (mut r, mut c) = (m.rows, m.cols);
            if ui.add(egui::DragValue::new(&mut r).range(1..=50)).changed()
                || ui.add(egui::DragValue::new(&mut c).range(1..=50)).changed()
            {
                *m = m.regrid(r, c);
                ch = true;
            }
        });
        ch |= ui
            .checkbox(&mut m.smooth, tr("docker.smooth_colour"))
            .changed();
        note(ui, &tr("docker.mesh_hint"));
        if ch {
            self.touch();
        }
    }

    /// The patterns (vector, bitmap, two colours) and the texture, with
    /// the tile. True when an image file was asked for.
    fn pattern_ui(&mut self, ui: &mut Ui, app: &mut App) -> bool {
        let mut ch = false;
        let mut asked = false;
        let picked_key = "fill_window_bitmap";
        match self.mode {
            Mode::Bitmap => {
                ui.horizontal(|ui| {
                    if ui.button(tr("fillwin.choose_image")).clicked() {
                        crate::files::Dialog::new()
                            .add_filter(
                                tr("fillwin.images"),
                                &["png", "jpg", "jpeg", "tif", "tiff", "webp", "bmp"],
                            )
                            .pick_into(picked_key);
                        asked = true;
                    }
                    note(
                        ui,
                        &match &self.bitmap {
                            Some(Pattern::Bitmap {
                                width_px, height_px, ..
                            }) => crate::i18n::trf(
                                "docker.bitmap_tile_n",
                                &[("w", &width_px.to_string()), ("h", &height_px.to_string())],
                            ),
                            _ => tr("fillwin.no_image"),
                        },
                    );
                });
                if let Some(path) = crate::files::take_picked(picked_key) {
                    match crate::files::open_image(&path) {
                        Ok(img) => {
                            let img = img.to_rgba8();
                            if let Some(png) = crate::bitmap_fx::encode(&img) {
                                let size_mm = match &self.bitmap {
                                    Some(Pattern::Bitmap { size_mm, .. }) => *size_mm,
                                    _ => 20.0,
                                };
                                self.bitmap = Some(Pattern::Bitmap {
                                    png,
                                    width_px: img.width(),
                                    height_px: img.height(),
                                    size_mm,
                                    layout: Default::default(),
                                });
                                ch = true;
                            }
                        }
                        Err(e) => {
                            app.status = crate::i18n::trf(
                                "status.could_not_read",
                                &[("p", &path.display().to_string()), ("e", &e)],
                            );
                        }
                    }
                }
            }
            Mode::Vector => {
                let copied = app.clipboard.as_ref().map_or(0, |c| c.shapes.len());
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(copied > 0, egui::Button::new(tr("fillwin.use_copied")))
                        .on_disabled_hover_text(tr("fillwin.copy_first"))
                        .clicked()
                    {
                        if let Some(c) = &app.clipboard {
                            self.vector = Some(Pattern::vector_from_shapes(&c.shapes));
                            ch = true;
                        }
                    }
                    let n = match &self.vector {
                        Some(Pattern::Vector { shapes, .. }) => shapes.len(),
                        _ => 0,
                    };
                    note(
                        ui,
                        &crate::i18n::trf("docker.vector_tile_n", &[("n", &n.to_string())]),
                    );
                });
            }
            Mode::Texture => {
                let t = &mut self.texture;
                ui.horizontal(|ui| {
                    for (k, key) in [
                        (TextureKind::Clouds, "texture.clouds"),
                        (TextureKind::Marble, "texture.marble"),
                        (TextureKind::Wood, "texture.wood"),
                        (TextureKind::Noise, "texture.noise"),
                        (TextureKind::Granite, "fillwin.granite"),
                    ] {
                        if ui.selectable_label(t.kind == k, tr(key)).clicked() && t.kind != k {
                            t.kind = k;
                            ch = true;
                        }
                    }
                });
                egui::Grid::new("fill_window_texture")
                    .num_columns(4)
                    .spacing([12.0, 8.0])
                    .show(ui, |ui| {
                        ui.label(tr("docker.seed"));
                        ui.horizontal(|ui| {
                            ch |= ui.add(egui::DragValue::new(&mut t.seed)).changed();
                            if ui.small_button(tr("fillwin.shuffle")).clicked() {
                                let now = ui.input(|i| i.time);
                                t.seed = ((now * 1_000_003.0) as u64 % 100_000) as u32;
                                ch = true;
                            }
                        });
                        ui.label(tr("fillwin.noise_scale"));
                        if let Some(v) = crate::measure_field::length_field(
                            ui,
                            "noise",
                            t.scale,
                            crate::app::Units::Millimeters,
                            96.0,
                            72.0,
                        ) {
                            t.scale = v.clamp(0.5, 1000.0);
                            ch = true;
                        }
                        ui.end_row();
                        ui.label(tr("fillwin.detail"));
                        let mut d = t.safe_detail();
                        if ui
                            .add(
                                egui::DragValue::new(&mut d)
                                    .range(Texture::MIN_DETAIL..=Texture::MAX_DETAIL),
                            )
                            .on_hover_text(tr("fillwin.detail_tip"))
                            .changed()
                        {
                            t.detail = d;
                            ch = true;
                        }
                        ui.label(tr("fillwin.contrast"));
                        let mut c = (t.safe_contrast() * 100.0).round();
                        if ui
                            .add(egui::Slider::new(&mut c, 0.0..=100.0).suffix(" %"))
                            .changed()
                        {
                            t.contrast = c / 100.0;
                            ch = true;
                        }
                        ui.end_row();
                        ui.label(tr("fillwin.resolution"));
                        if let Some(v) = crate::measure_field::number_field(
                            ui,
                            "texture_ppi",
                            t.safe_resolution(),
                            0,
                            " ppp",
                            72.0,
                        ) {
                            t.resolution =
                                v.clamp(TileLayout::MIN_RESOLUTION, TileLayout::MAX_RESOLUTION);
                            ch = true;
                        }
                        ui.end_row();
                    });
                self.front_back(ui);
            }
            _ => {
                ui.horizontal_wrapped(|ui| {
                    for t in PatternTile::ALL {
                        if ui
                            .selectable_label(self.two_color.tile == t, tr(tile_key(t)))
                            .clicked()
                            && self.two_color.tile != t
                        {
                            self.two_color.tile = t;
                            ch = true;
                        }
                    }
                });
                ui.horizontal_top(|ui| {
                    if let Some((x, y)) = cell_picture(ui, &self.two_color) {
                        self.two_color.tile = self.two_color.tile.toggled(x, y);
                        ch = true;
                    }
                    note(ui, &tr("fillwin.cells_tip"));
                });
                self.front_back(ui);
            }
        }
        if self.mode != Mode::Texture {
            ui.add_space(8.0);
            ch |= self.tile_ui(ui);
        }
        if ch {
            self.touch();
        }
        asked
    }

    /// Tile size and the tile layout (scale, rotation, offsets, mirroring).
    fn tile_ui(&mut self, ui: &mut Ui) -> bool {
        let mut ch = false;
        let units = crate::app::Units::Millimeters;
        ui.horizontal(|ui| {
            ui.label(tr("fillwin.tile"));
            match self.mode {
                Mode::TwoColor => {
                    let t = &mut self.two_color;
                    if let Some(v) =
                        crate::measure_field::length_field(ui, "tile_w", t.size_mm, units, 96.0, 80.0)
                    {
                        t.size_mm = v.max(0.1);
                        ch = true;
                    }
                    ui.label("×");
                    ui.add_enabled_ui(false, |ui| {
                        crate::measure_field::length_field(ui, "tile_h", t.size_mm, units, 96.0, 80.0);
                    });
                }
                Mode::Bitmap => {
                    if let Some(Pattern::Bitmap {
                        size_mm,
                        width_px,
                        height_px,
                        ..
                    }) = &mut self.bitmap
                    {
                        if let Some(v) =
                            crate::measure_field::length_field(ui, "tile_w", *size_mm, units, 96.0, 80.0)
                        {
                            *size_mm = v.max(0.1);
                            ch = true;
                        }
                        ui.label("×");
                        let h = *size_mm * *height_px as f64 / (*width_px).max(1) as f64;
                        ui.add_enabled_ui(false, |ui| {
                            crate::measure_field::length_field(ui, "tile_h", h, units, 96.0, 80.0);
                        });
                    }
                }
                Mode::Vector => {
                    if let Some(Pattern::Vector { shapes, tile, .. }) = &mut self.vector {
                        let (mut w, mut h) = (tile.width, tile.height);
                        if let Some(v) =
                            crate::measure_field::length_field(ui, "tile_w", w, units, 96.0, 80.0)
                        {
                            w = v.max(Pattern::MIN_TILE_MM);
                        }
                        ui.label("×");
                        if let Some(v) =
                            crate::measure_field::length_field(ui, "tile_h", h, units, 96.0, 80.0)
                        {
                            h = v.max(Pattern::MIN_TILE_MM);
                        }
                        if (w - tile.width).abs() > 1e-9 || (h - tile.height).abs() > 1e-9 {
                            // The drawing scales with its tile.
                            let sx = w / tile.width.max(1e-6);
                            let sy = h / tile.height.max(1e-6);
                            for s in shapes.iter_mut() {
                                s.transform =
                                    graphdraw_core::geometry::Affine::scale_non_uniform(sx, sy)
                                        * s.transform;
                            }
                            tile.width = w;
                            tile.height = h;
                            ch = true;
                        }
                    }
                }
                _ => {}
            }
        });
        // Vector tiles stay vectors except in exports that rasterise
        // them; the resolution is the picture's for the others.
        let vector = self.mode == Mode::Vector;
        if let Some(l) = self.layout_mut() {
            ch |= layout_ui(ui, l, vector);
        }
        ch
    }

    /// The tile layout of the pattern being edited.
    fn layout_mut(&mut self) -> Option<&mut TileLayout> {
        match self.mode {
            Mode::TwoColor => Some(&mut self.two_color.layout),
            Mode::Bitmap => self.bitmap.as_mut().map(Pattern::layout_mut),
            Mode::Vector => self.vector.as_mut().map(Pattern::layout_mut),
            _ => None,
        }
    }

    /// Front and back colours (two colours, texture).
    fn front_back(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.label(tr("docker.front"));
            self.color_popup(ui, "front", Target::Front);
            ui.label(tr("docker.back"));
            self.color_popup(ui, "back", Target::Back);
        });
    }

    /// The new fill above the original one, over a chequerboard, with
    /// their names beside them.
    fn compare(&mut self, ui: &mut Ui) {
        let (w, h) = (COMPARE_W, COMPARE_H);
        let (rect, _) = ui.allocate_exact_size(Vec2::new(w, h), Sense::hover());
        let top = Rect::from_min_size(rect.min, Vec2::new(w, h / 2.0));
        let bottom = Rect::from_min_size(rect.min + Vec2::new(0.0, h / 2.0), Vec2::new(w, h / 2.0));
        checker(ui.painter(), rect);
        let ppp = ui.ctx().pixels_per_point();
        let (pw, ph) = ((w * ppp).round() as u32, (h / 2.0 * ppp).round() as u32);
        let size = self.size;
        let new = self.result();
        let paint = |ui: &Ui, tex: Option<egui::TextureId>, r: Rect| {
            if let Some(t) = tex {
                ui.painter().image(
                    t,
                    r,
                    Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                    Color32::WHITE,
                );
            }
        };
        if matches!(new, Fill::None) {
            none_mark(ui.painter(), top);
        } else {
            let key = (new.clone(), pw, ph);
            let tex = self
                .sample
                .get(ui.ctx(), "fill_window_new", key, || sample(&new, size, pw, ph));
            paint(ui, tex, top);
        }
        if matches!(self.initial, Fill::None) {
            none_mark(ui.painter(), bottom);
        } else {
            let initial = self.initial.clone();
            let tex = self.before.get(ui.ctx(), "fill_window_old", (pw, ph), || {
                sample(&initial, size, pw, ph)
            });
            paint(ui, tex, bottom);
        }
        let p = ui.painter();
        p.line_segment(
            [bottom.left_top(), bottom.right_top()],
            Stroke::new(1.0, Tokens::text_dim()),
        );
        p.rect_stroke(
            rect,
            0.0,
            Stroke::new(1.0, Tokens::control_border()),
            egui::StrokeKind::Outside,
        );
        let (lr, _) = ui.allocate_exact_size(Vec2::new(56.0, h), Sense::hover());
        for (r, key) in [(top, "fillwin.new"), (bottom, "fillwin.original")] {
            ui.painter().text(
                egui::pos2(lr.left() + 6.0, r.center().y),
                egui::Align2::LEFT_CENTER,
                tr(key),
                egui::FontId::proportional(10.0),
                Tokens::text_dim(),
            );
        }
    }
}

/// `fill` over a box the first object's size, as a `pw` by `ph` image.
fn sample(fill: &Fill, size: Size, pw: u32, ph: u32) -> Option<egui::ColorImage> {
    let w = size.width.max(1.0);
    // The box keeps the sample's proportions, as wide as the object.
    let h = w * ph.max(1) as f64 / pw.max(1) as f64;
    let bounds = graphdraw_core::geometry::Rect::new(0.0, 0.0, w, h);
    let dpi = pw as f64 / w * 25.4;
    let pm = match fill {
        Fill::Fountain(f) if f.kind == FountainKind::Shaped => graphdraw_render::render_fill_image_in(
            fill,
            &graphdraw_core::geometry::rect_path(bounds, 0.0),
            dpi,
        ),
        _ => graphdraw_render::render_fill_image(fill, bounds, dpi),
    }?;
    Some(sw::pixmap_image(&pm))
}

/// The stops drawn left to right.
fn band(p: &egui::Painter, r: Rect, stops: &[Stop]) {
    let mut mesh = egui::Mesh::default();
    let mut n = 0u32;
    let mut push = |x: f32, c: Color32, mesh: &mut egui::Mesh| {
        mesh.colored_vertex(egui::pos2(x, r.top()), c);
        mesh.colored_vertex(egui::pos2(x, r.bottom()), c);
        if n > 0 {
            let a = (n - 1) * 2;
            mesh.add_triangle(a, a + 1, a + 2);
            mesh.add_triangle(a + 1, a + 3, a + 2);
        }
        n += 1;
    };
    if let Some(first) = stops.first() {
        push(r.left(), color32(first.color), &mut mesh);
    }
    for s in stops {
        push(
            r.left() + r.width() * s.pos.clamp(0.0, 1.0) as f32,
            color32(s.color),
            &mut mesh,
        );
    }
    if let Some(last) = stops.last() {
        push(r.right(), color32(last.color), &mut mesh);
    }
    p.add(egui::Shape::mesh(mesh));
}

/// The two-colour tile as a grid of cells (front and back).
fn cell_picture(ui: &mut Ui, t: &TwoColor) -> Option<(usize, usize)> {
    let n = PatternTile::CELLS;
    let (r, resp) =
        ui.allocate_exact_size(Vec2::splat(n as f32 * PATTERN_CELL), Sense::click());
    let (fg, bg) = (color32(t.front), color32(t.back));
    for y in 0..n {
        for x in 0..n {
            let cr = Rect::from_min_size(
                r.min + Vec2::new(x as f32 * PATTERN_CELL, y as f32 * PATTERN_CELL),
                Vec2::splat(PATTERN_CELL),
            );
            ui.painter()
                .rect_filled(cr.shrink(0.5), 0.0, if t.tile.cell(x, y) { fg } else { bg });
        }
    }
    // A click swaps the cell under the pointer between front and back.
    let q = resp.interact_pointer_pos().filter(|_| resp.clicked())?;
    let x = ((q.x - r.left()) / PATTERN_CELL).floor();
    let y = ((q.y - r.top()) / PATTERN_CELL).floor();
    (x >= 0.0 && y >= 0.0).then_some((x as usize, y as usize))
}

/// Scale, rotation, resolution, row and column offsets, mirroring and
/// following the object, for any pattern's tiles. True when changed.
fn layout_ui(ui: &mut Ui, l: &mut TileLayout, vector: bool) -> bool {
    let mut ch = false;
    let field = |ui: &mut Ui, id: &str, v: f64, suffix: &str| {
        crate::measure_field::number_field(ui, id, v, 1, suffix, 72.0)
    };
    egui::Grid::new("fill_window_tile")
        .num_columns(4)
        .spacing([12.0, 8.0])
        .show(ui, |ui| {
            ui.label(tr("fillwin.scale"));
            if let Some(v) = field(ui, "tile_scale", l.safe_scale() * 100.0, " %") {
                l.scale = (v / 100.0).clamp(TileLayout::MIN_SCALE, TileLayout::MAX_SCALE);
                ch = true;
            }
            ui.label(tr("fillwin.rotation"));
            if let Some(v) = field(ui, "tile_rotation", l.rotation, "°") {
                l.rotation = v.rem_euclid(360.0);
                ch = true;
            }
            ui.end_row();
            ui.label(tr("fillwin.resolution"));
            ui.add_enabled_ui(!vector, |ui| {
                if let Some(v) = field(ui, "tile_ppi", l.safe_resolution(), " ppp") {
                    l.resolution =
                        v.clamp(TileLayout::MIN_RESOLUTION, TileLayout::MAX_RESOLUTION);
                    ch = true;
                }
            })
            .response
            .on_hover_text(tr("fillwin.resolution_tip"));
            ui.end_row();
            let (row, col) = l.safe_offsets();
            ui.label(tr("fillwin.row_offset"));
            if let Some(v) = field(ui, "tile_row", row * 100.0, " %") {
                l.row_offset = (v / 100.0).clamp(0.0, 1.0);
                ch = true;
            }
            ui.label(tr("fillwin.column_offset"));
            if let Some(v) = field(ui, "tile_col", col * 100.0, " %") {
                l.column_offset = (v / 100.0).clamp(0.0, 1.0);
                ch = true;
            }
            ui.end_row();
        });
    ch |= ui
        .checkbox(&mut l.mirror, tr("fillwin.mirror_tiles"))
        .changed();
    ch |= ui
        .checkbox(&mut l.with_object, tr("fillwin.transform_with_object"))
        .on_hover_text(tr("fillwin.transform_with_object_tip"))
        .changed();
    ch
}

/// The Help text of a mode.
fn help_key(m: Mode) -> &'static str {
    match m {
        Mode::None => "fillwin.help_none",
        Mode::Solid => "fillwin.help_solid",
        Mode::Gradient => "fillwin.help_gradient",
        Mode::Vector => "fillwin.help_vector",
        Mode::Bitmap => "fillwin.help_bitmap",
        Mode::TwoColor => "fillwin.help_two_color",
        Mode::Texture => "fillwin.help_texture",
        Mode::Mesh => "fillwin.help_mesh",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(f: &mut FillWindow, app: &mut App, frames: usize) {
        let ctx = Context::default();
        let screen = Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(1366.0, 768.0));
        for k in 0..frames {
            let input = egui::RawInput {
                screen_rect: Some(screen),
                time: Some(k as f64 / 30.0),
                ..Default::default()
            };
            let mut out = ctx.run_ui(input, |ui| {
                let o = f.show(app, ui.ctx());
                if let Outcome::Open(r) = o {
                    assert!(r.height() < 740.0, "{r:?}");
                }
            });
            out.textures_delta.clear();
        }
    }

    /// A gradient opens with its type, nodes and repetition, and the
    /// window draws every mode.
    #[test]
    fn opens_a_gradient_and_draws_every_mode() {
        let mut app = App::headless();
        let mut g = Fountain::two(
            FountainKind::Radial,
            Color::rgb8(255, 0, 0),
            Color::rgb8(0, 0, 255),
            0.0,
        );
        g.stops.insert(1, stop(0.4, Color::rgb8(0, 255, 0)));
        g.repeat = FountainRepeat::Mirror;
        g.repeats = 3;
        let mut f = FillWindow::new(Fill::Fountain(g), false, Vec::new(), Size::new(20.0, 10.0));
        assert_eq!((f.mode, f.fountain.kind), (Mode::Gradient, FountainKind::Radial));
        assert_eq!(f.fountain.stops.len(), 3);
        run(&mut f, &mut app, 3);
        assert!(f.ramp_bar.is_positive());
        for m in [
            Mode::None,
            Mode::Solid,
            Mode::Vector,
            Mode::Bitmap,
            Mode::TwoColor,
            Mode::Texture,
        ] {
            f.set_mode(m);
            run(&mut f, &mut app, 2);
            assert_eq!(f.mode, m);
        }
    }

    /// Nodes: a new one takes the gradient's colour there; only middle
    /// nodes go; Reverse mirrors the positions.
    #[test]
    fn nodes_add_remove_reverse() {
        let mut f = FillWindow::new(
            Fill::linear(Color::rgb8(0, 0, 0), Color::rgb8(255, 255, 255), 0.0),
            false,
            Vec::new(),
            Size::new(10.0, 10.0),
        );
        f.add_stop(0.5, None);
        assert_eq!((f.fountain.stops.len(), f.sel_stop), (3, 1));
        let [r, _, _] = f.fountain.stops[1].color.to_rgb8();
        assert!((r as i32 - 128).abs() <= 2, "{r}");
        f.remove_stop(0);
        assert_eq!(f.fountain.stops.len(), 3, "the ends stay");
        f.fountain.stops[1].pos = 0.3;
        f.reverse();
        assert!((f.fountain.stops[1].pos - 0.7).abs() < 1e-9);
        f.remove_stop(1);
        assert_eq!(f.fountain.stops.len(), 2);
        assert!(f.changed);
    }

    /// The ramp under the pointer: a double click on the band adds a node
    /// there, a double click on that node takes it away.
    #[test]
    fn ramp_pointer() {
        let mut app = App::headless();
        let ctx = Context::default();
        let screen = Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(1366.0, 768.0));
        let mut f = FillWindow::new(
            Fill::linear(Color::BLACK, Color::WHITE, 0.0),
            false,
            Vec::new(),
            Size::new(10.0, 10.0),
        );
        let mut t = 0.0;
        let mut frame = |f: &mut FillWindow, app: &mut App, events: Vec<egui::Event>| {
            t += 0.05;
            let input = egui::RawInput {
                screen_rect: Some(screen),
                time: Some(t),
                events,
                ..Default::default()
            };
            let mut out = ctx.run_ui(input, |ui| {
                f.show(app, ui.ctx());
            });
            out.textures_delta.clear();
        };
        for _ in 0..10 {
            frame(&mut f, &mut app, vec![]);
        }
        let bar = f.ramp_bar;
        assert!(bar.is_positive());
        let btn = |p: egui::Pos2, pressed: bool| egui::Event::PointerButton {
            pos: p,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        let at = egui::pos2(bar.left() + bar.width() * 0.25, bar.center().y);
        frame(&mut f, &mut app, vec![egui::Event::PointerMoved(at)]);
        for _ in 0..2 {
            frame(&mut f, &mut app, vec![btn(at, true)]);
            frame(&mut f, &mut app, vec![btn(at, false)]);
        }
        assert_eq!(f.fountain.stops.len(), 3, "a new node");
        assert!((f.fountain.stops[1].pos - 0.25).abs() < 0.01);
        let node = egui::pos2(at.x, bar.bottom() + RAMP_NODE_ROW / 2.0);
        frame(&mut f, &mut app, vec![egui::Event::PointerMoved(node)]);
        // A pause, so the next clicks are a double click of their own.
        for _ in 0..10 {
            frame(&mut f, &mut app, vec![]);
        }
        for _ in 0..2 {
            frame(&mut f, &mut app, vec![btn(node, true)]);
            frame(&mut f, &mut app, vec![btn(node, false)]);
        }
        assert_eq!(f.fountain.stops.len(), 2);
    }

    /// Each mode gives its kind of fill; overprint goes with the fill.
    #[test]
    fn results_and_commands() {
        let id = ShapeId(7);
        let mut f = FillWindow::new(Fill::None, false, vec![id], Size::new(10.0, 10.0));
        assert!(f.commands(&Document::default()).is_empty(), "nothing changed");
        f.set_mode(Mode::TwoColor);
        assert!(matches!(f.result(), Fill::Pattern(Pattern::TwoColor { .. })));
        f.set_mode(Mode::Texture);
        assert!(matches!(f.result(), Fill::Texture(_)));
        f.set_mode(Mode::Solid);
        f.overprint = true;
        let cmds = f.commands(&Document::default());
        assert!(matches!(&cmds[0], Command::SetFill { fill: Fill::Solid(_), .. }));
        assert!(matches!(
            &cmds[1],
            Command::SetOverprint {
                fill: Some(true),
                outline: None,
                ..
            }
        ));
        f.set_mode(Mode::None);
        assert_eq!(f.result(), Fill::None);
    }

    /// The fill transparency opens as the object has it, goes into the
    /// commands only when changed, and OK applies it as one undo step.
    #[test]
    fn fill_transparency_opens_changes_and_applies() {
        let mut app = App::headless();
        let id = app
            .new_shape(graphdraw_core::ShapeKind::Rect {
                rect: graphdraw_core::Rect::new(0.0, 0.0, 10.0, 10.0),
                radius: 0.0,
                corners: None,
            })
            .expect("shape");
        app.select(vec![id]);
        app.run(Command::SetFill {
            shapes: vec![id],
            fill: Fill::Solid(Color::rgb8(255, 0, 0)),
        });
        app.run(Command::SetFillOpacity {
            shapes: vec![id],
            opacity: 0.3,
        });
        let mut f = FillWindow::open(&app);
        assert_eq!(f.mode, Mode::Solid);
        assert!((f.fill_opacity - 0.3).abs() < 1e-9);
        run(&mut f, &mut app, 2);
        // A colour change alone leaves the transparency out.
        f.color = Color::rgb8(0, 0, 255);
        f.touch();
        let cmds = f.commands(app.doc());
        assert!(!cmds
            .iter()
            .any(|c| matches!(c, Command::SetFillOpacity { .. })));
        f.fill_opacity = 0.6;
        let cmds = f.commands(app.doc());
        assert!(cmds.iter().any(|c| matches!(
            c,
            Command::SetFillOpacity { shapes, opacity } if shapes == &vec![id] && (*opacity - 0.6).abs() < 1e-9
        )));
        f.apply(&mut app);
        let fo = |app: &App| app.doc().find_shape(id).map(|s| s.fill_opacity);
        assert_eq!(fo(&app), Some(0.6));
        app.undo();
        assert_eq!(fo(&app), Some(0.3), "one undo step for fill and transparency");
        // With nothing selected the slider is off and nothing changes it.
        let mut d = FillWindow::new(Fill::None, false, Vec::new(), Size::new(10.0, 10.0));
        d.set_mode(Mode::Solid);
        run(&mut d, &mut app, 2);
        assert_eq!(d.fill_opacity, 1.0);
        assert!(d.commands(app.doc()).is_empty());
    }
}
