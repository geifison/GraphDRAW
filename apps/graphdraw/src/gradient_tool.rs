//! Interactive Fill on the canvas: the gradient's handles and stops.
//!
//! On a selected object with a gradient: the axis between the start and
//! end handles (the centre and the radius end for the centred kinds, plus
//! the y radius of elliptical and square ones) and the colour stops on it.
//! Dragging the start or the end moves that end alone (Shift: the axis
//! turns about the centre; Ctrl: 15 degree steps); dragging a stop slides
//! it along the axis; a double click on the axis adds a stop with the
//! colour already there; a click on a stop (or on an end, its end stop)
//! chooses it and opens a balloon with its colour, opacity and midpoint;
//! Delete removes the chosen stop (never below two); a palette colour
//! clicked goes to the chosen stop, and one dropped on any stop of any
//! object recolours that stop. Each change is one undo step; a drag is one
//! step. The geometry is `graphdraw_core::gradient`; see
//! `docs/behavior/gradient-fill.md`.

use crate::app::{App, Drag};
use crate::i18n::tr;
use crate::theme::Tokens;
use crate::tools::Tool;
use egui::{Color32, Painter, PointerButton, Pos2, Response, Stroke as EStroke};
use graphdraw_core::geometry::{Point, Rect};
use graphdraw_core::gradient::{self, DragMods, Handle};
use graphdraw_core::{Color, Command, Fill, Fountain, ShapeId};

/// How near a handle the pointer must be, screen points.
pub const HANDLE_HIT: f64 = 7.0;
/// The size of the start and end squares, screen points.
const END_SIZE: f32 = 9.0;
/// The diameter of a stop mark, screen points.
const STOP_SIZE: f32 = 9.0;
/// The balloon's width.
const POPUP_WIDTH: f32 = 260.0;

/// A handle being dragged.
#[derive(Debug, Clone)]
pub struct HandleDrag {
    pub shape: ShapeId,
    pub handle: Handle,
    pub original: Fountain,
    pub bounds: Rect,
    /// The first change is recorded; the next ones join it.
    pub begun: bool,
}

/// The Interactive Fill tool's own state.
#[derive(Debug, Clone, Default)]
pub struct GradientToolState {
    /// The chosen stop: object and index.
    pub stop: Option<(ShapeId, usize)>,
    /// The drawing's revision when the stop was chosen; after any other
    /// change (an undo among them) the choice no longer holds.
    pub rev: u64,
    /// The stop balloon: where it opens (screen) and whether it has been
    /// drawn once.
    pub popup: Option<(Pos2, bool)>,
    pub drag: Option<HandleDrag>,
    picker: crate::ui::style_windows::ColorPicker,
    /// A slider of the balloon is being dragged: its changes join one step.
    popup_dragging: bool,
}

impl App {
    /// The object selected alone, when its fill is a gradient: its id,
    /// gradient and bounds.
    pub fn gradient_target(&self) -> Option<(ShapeId, Fountain, Rect)> {
        if self.selection.len() != 1 {
            return None;
        }
        self.selected_fountain()
    }

    /// The handle of the selected gradient under the page point `p`.
    pub fn gradient_handle_at(&self, p: Point) -> Option<Handle> {
        let (_, f, b) = self.gradient_target()?;
        let tol = HANDLE_HIT / self.view.zoom.max(1e-6) as f64;
        gradient::handles(&f, b)
            .into_iter()
            .find(|(_, q)| (*q - p).hypot() <= tol)
            .map(|(h, _)| h)
    }

    /// The chosen stop when it still belongs to the selected gradient.
    pub fn gradient_stop(&self) -> Option<(ShapeId, usize)> {
        let (id, f, _) = self.gradient_target()?;
        if self.engine.revision() != self.gradient_tool.rev {
            return None;
        }
        self.gradient_tool
            .stop
            .filter(|(s, i)| *s == id && *i < f.stops.len())
    }

    /// The chosen stop stays chosen across the change just made.
    pub fn gradient_keep_stop(&mut self) {
        self.gradient_tool.rev = self.engine.revision();
    }

    fn choose_stop(&mut self, id: ShapeId, i: usize) {
        self.gradient_tool.stop = Some((id, i));
        self.gradient_keep_stop();
    }

    /// Set the fill of `id`, as a step of its own or (`join`) as part of
    /// the step just recorded.
    fn set_gradient(&mut self, id: ShapeId, f: Fountain, join: bool) {
        let cmd = Command::SetFill {
            shapes: vec![id],
            fill: Fill::Fountain(f),
        };
        if join {
            if let Err(e) = self.engine.amend(&cmd) {
                self.status = format!("{}: {e}", cmd.label());
            }
        } else {
            self.run(cmd);
        }
    }

    /// Canvas input of the gradient handles; true when it took the event
    /// (the rest of the Interactive Fill tool's input follows otherwise).
    pub fn gradient_input(&mut self, response: &Response, p: Point) -> bool {
        let m = response.ctx.input(|i| i.modifiers);
        let mods = DragMods {
            snap_angle: m.command || m.ctrl,
            rotate: m.shift,
        };
        if response.drag_started_by(PointerButton::Primary) {
            let origin = response
                .ctx
                .input(|i| i.pointer.press_origin())
                .map(|s| self.view.to_page(s))
                .unwrap_or(p);
            return self.begin_gradient_drag(origin);
        }
        if response.dragged_by(PointerButton::Primary) && matches!(self.drag, Drag::GradientHandle) {
            self.drag_gradient_to(p, mods);
            return true;
        }
        if response.double_clicked_by(PointerButton::Primary) && self.add_gradient_stop_at(p) {
            return true;
        }
        if response.clicked_by(PointerButton::Primary) {
            let at = response
                .interact_pointer_pos()
                .unwrap_or_else(|| self.view.to_screen(p));
            if self.gradient_click(p, at) {
                return true;
            }
            self.gradient_tool.popup = None;
            self.gradient_tool.stop = None;
        }
        false
    }

    /// Take the handle under `p` for a drag; false when there is none.
    pub fn begin_gradient_drag(&mut self, p: Point) -> bool {
        let Some((id, f, b)) = self.gradient_target() else {
            return false;
        };
        let Some(handle) = self.gradient_handle_at(p) else {
            return false;
        };
        if let Handle::Stop(i) = handle {
            self.choose_stop(id, i);
        }
        self.gradient_tool.popup = None;
        self.gradient_tool.drag = Some(HandleDrag {
            shape: id,
            handle,
            original: f,
            bounds: b,
            begun: false,
        });
        self.drag = Drag::GradientHandle;
        true
    }

    /// Move the dragged handle to `p`; one undo step for the whole drag.
    pub fn drag_gradient_to(&mut self, p: Point, mods: DragMods) {
        let Some(d) = self.gradient_tool.drag.clone() else {
            return;
        };
        let g = gradient::drag(&d.original, d.bounds, d.handle, p, mods);
        let keep = self.gradient_stop().is_some();
        self.set_gradient(d.shape, g, d.begun);
        if keep {
            self.gradient_keep_stop();
        }
        if let Some(d) = &mut self.gradient_tool.drag {
            d.begun = true;
        }
    }

    /// The end of a handle drag: the stops back in order (the dragged
    /// one stays chosen), in the same undo step.
    pub fn finish_gradient_drag(&mut self) {
        let Some(d) = self.gradient_tool.drag.take() else {
            return;
        };
        if !d.begun {
            return;
        }
        let Some((id, mut f, _)) = self.gradient_target() else {
            return;
        };
        let cur = match d.handle {
            Handle::Stop(i) => Some(i),
            _ => self.gradient_stop().map(|(_, i)| i),
        };
        let keep = gradient::sort_stops(&mut f, cur);
        self.set_gradient(id, f, true);
        match keep {
            Some(k) => self.choose_stop(id, k),
            None => self.gradient_keep_stop(),
        }
    }

    /// A click on a handle chooses its stop (an end: the end stop) and
    /// opens the stop balloon at `at` (screen); a click on the axis does
    /// nothing else. False elsewhere.
    pub fn gradient_click(&mut self, p: Point, at: Pos2) -> bool {
        let Some((id, f, b)) = self.gradient_target() else {
            return false;
        };
        if let Some(h) = self.gradient_handle_at(p) {
            let i = match h {
                Handle::Stop(i) => Some(i),
                Handle::Start => gradient::end_stop(&f, false),
                Handle::End => gradient::end_stop(&f, true),
                Handle::RadiusY => None,
            };
            if let Some(i) = i {
                self.choose_stop(id, i);
                self.gradient_tool.popup = Some((at, false));
            }
            return true;
        }
        let tol = HANDLE_HIT / self.view.zoom.max(1e-6) as f64;
        gradient::distance_to_axis(&f, b, p).is_some_and(|d| d <= tol)
    }

    /// A new stop on the selected gradient's axis near `p`, with the
    /// colour there; it becomes the chosen stop.
    pub fn add_gradient_stop_at(&mut self, p: Point) -> bool {
        let Some((id, f, b)) = self.gradient_target() else {
            return false;
        };
        let tol = HANDLE_HIT / self.view.zoom.max(1e-6) as f64;
        if !gradient::distance_to_axis(&f, b, p).is_some_and(|d| d <= tol) {
            return false;
        }
        let Some((g, i)) = gradient::add_stop(&f, b, p) else {
            return false;
        };
        self.set_gradient(id, g, false);
        self.choose_stop(id, i);
        true
    }

    /// Change the chosen stop with `edit`; false when no stop is chosen.
    /// `join`: part of the step just recorded (a slider being dragged).
    pub fn edit_gradient_stop(&mut self, join: bool, edit: impl FnOnce(&mut graphdraw_core::Stop)) -> bool {
        let Some((id, i)) = self.gradient_stop() else {
            return false;
        };
        let Some((_, mut f, _)) = self.gradient_target() else {
            return false;
        };
        let Some(s) = f.stops.get_mut(i) else {
            return false;
        };
        let before = *s;
        edit(s);
        s.opacity = s.opacity.clamp(0.0, 1.0);
        s.midpoint = s.midpoint.clamp(0.01, 0.99);
        s.pos = s.pos.clamp(0.0, 1.0);
        if *s == before {
            return true;
        }
        let mut keep = Some(i);
        if s.pos != before.pos {
            keep = gradient::sort_stops(&mut f, Some(i));
        }
        self.set_gradient(id, f, join);
        self.choose_stop(id, keep.unwrap_or(i));
        true
    }

    /// The chosen stop's colour (a palette click); false when no stop is
    /// chosen.
    pub fn set_gradient_stop_color(&mut self, c: Color) -> bool {
        self.edit_gradient_stop(false, |s| s.color = c)
    }

    /// Remove the chosen stop (Delete); false when none was chosen. Two
    /// stops always stay.
    pub fn delete_gradient_stop(&mut self) -> bool {
        let Some((id, i)) = self.gradient_stop() else {
            return false;
        };
        self.gradient_tool.stop = None;
        self.gradient_tool.popup = None;
        let Some((_, f, _)) = self.gradient_target() else {
            return true;
        };
        if let Some(g) = gradient::remove_stop(&f, i) {
            self.set_gradient(id, g, false);
        }
        true
    }

    /// The gradient stop of any object under the page point `p`.
    pub fn gradient_stop_under(&self, p: Point) -> Option<(ShapeId, usize)> {
        let id = self.hit_test_inside(p).or_else(|| self.hit_test(p))?;
        let s = self.doc().find_shape(id)?;
        let Fill::Fountain(f) = &s.fill else {
            return None;
        };
        let tol = HANDLE_HIT / self.view.zoom.max(1e-6) as f64;
        gradient::handles(f, s.bounds())
            .into_iter()
            .find_map(|(h, q)| match h {
                Handle::Stop(i) if (q - p).hypot() <= tol => Some((id, i)),
                _ => None,
            })
    }

    /// A palette colour dropped at `at` (screen) on a stop recolours that
    /// stop; false when no stop is there.
    pub fn drop_color_on_stop(&mut self, color: Color, at: Pos2) -> bool {
        if !self.canvas_rect.contains(at) || !self.has_document() {
            return false;
        }
        let p = self.view.to_page(at);
        let Some((id, i)) = self.gradient_stop_under(p) else {
            return false;
        };
        let Some(Fill::Fountain(mut f)) = self.doc().find_shape(id).map(|s| s.fill.clone()) else {
            return false;
        };
        let Some(s) = f.stops.get_mut(i) else {
            return false;
        };
        s.color = color;
        let chosen = self.gradient_stop() == Some((id, i));
        self.set_gradient(id, f, false);
        if chosen {
            self.gradient_keep_stop();
        }
        true
    }

    /// A new linear gradient dragged from `a` to `z` on `shape`: the
    /// stops of its current gradient, or its colour to white, or the
    /// default colour to white.
    pub fn finish_fill_gradient(&mut self, shape: ShapeId, a: Point, z: Point) {
        let Some(s) = self.doc().find_shape(shape).cloned() else {
            return;
        };
        let fallback = match &self.default_fill() {
            Fill::Solid(c) => *c,
            _ => Color::cmyk_pct(0.0, 0.0, 0.0, 100.0),
        };
        let f = gradient::linear_from(a, z, s.bounds(), &s.fill, fallback);
        self.gradient_tool.stop = None;
        self.set_gradient(shape, f, false);
    }
}

/// Where a new gradient's end goes: `p`, or with Ctrl at 15 degree steps
/// from `start`.
pub fn snap_end(start: Point, p: Point, ctrl: bool) -> Point {
    if !ctrl {
        return p;
    }
    let d = p - start;
    let a = ((d.atan2().to_degrees() / 15.0).round() * 15.0).to_radians();
    start + graphdraw_core::geometry::Vec2::new(a.cos(), a.sin()) * d.hypot()
}

fn rgb32(c: Color, opacity: f64) -> Color32 {
    let [r, g, b] = c.to_rgb8();
    let a = (opacity.clamp(0.0, 1.0) * 255.0).round() as u8;
    Color32::from_rgba_unmultiplied(r, g, b, a.max(60))
}

/// Draw the selected gradient's axis, handles and stops; false when there
/// is nothing to draw (another tool, or no gradient selected alone).
pub fn draw(app: &App, painter: &Painter) -> bool {
    if app.tool != Tool::InteractiveFill {
        return false;
    }
    let Some((_, f, b)) = app.gradient_target() else {
        return false;
    };
    let Some((a, z)) = gradient::axis(&f, b) else {
        // A shaped gradient follows the outline: nothing to drag.
        return true;
    };
    let view = &app.view;
    let (sa, sz) = (view.to_screen(a), view.to_screen(z));
    painter.line_segment([sa, sz], EStroke::new(3.0, Color32::WHITE));
    painter.line_segment([sa, sz], EStroke::new(1.0, Tokens::SELECTION));
    let chosen = app.gradient_stop().map(|(_, i)| i);
    for (h, q) in gradient::handles(&f, b) {
        let c = view.to_screen(q);
        match h {
            Handle::Start | Handle::End => {
                let r = egui::Rect::from_center_size(c, egui::vec2(END_SIZE, END_SIZE));
                painter.rect_filled(r, 0.0, Color32::WHITE);
                painter.rect_stroke(
                    r,
                    0.0,
                    EStroke::new(1.0, Tokens::SELECTION),
                    egui::StrokeKind::Outside,
                );
            }
            Handle::RadiusY => {
                let s = END_SIZE / 2.0 + 1.0;
                let pts = vec![
                    c + egui::vec2(0.0, -s),
                    c + egui::vec2(s, 0.0),
                    c + egui::vec2(0.0, s),
                    c + egui::vec2(-s, 0.0),
                ];
                painter.add(egui::Shape::convex_polygon(
                    pts,
                    Color32::WHITE,
                    EStroke::new(1.0, Tokens::SELECTION),
                ));
            }
            Handle::Stop(i) => {
                let Some(s) = f.stops.get(i) else {
                    continue;
                };
                painter.circle(
                    c,
                    STOP_SIZE / 2.0,
                    rgb32(s.color, s.opacity),
                    EStroke::new(1.0, Tokens::SELECTION),
                );
                if chosen == Some(i) {
                    painter.circle_stroke(
                        c,
                        STOP_SIZE / 2.0 + 2.5,
                        EStroke::new(2.0, Tokens::ACCENT),
                    );
                }
            }
        }
    }
    true
}

/// While a palette colour is dragged: the stops of the gradient under the
/// pointer (screen `at`), so it can be dropped on one.
pub fn paint_hover_stops(app: &App, painter: &Painter, at: Pos2) {
    if !app.canvas_rect.contains(at) || !app.has_document() {
        return;
    }
    let p = app.view.to_page(at);
    let Some(id) = app.hit_test_inside(p).or_else(|| app.hit_test(p)) else {
        return;
    };
    let Some(s) = app.doc().find_shape(id) else {
        return;
    };
    let Fill::Fountain(f) = &s.fill else {
        return;
    };
    for (h, q) in gradient::handles(f, s.bounds()) {
        if let Handle::Stop(i) = h {
            let Some(st) = f.stops.get(i) else {
                continue;
            };
            let c = app.view.to_screen(q);
            painter.circle_filled(c, STOP_SIZE / 2.0, rgb32(st.color, 1.0));
            painter.circle_stroke(c, STOP_SIZE / 2.0, EStroke::new(2.0, Color32::WHITE));
            painter.circle_stroke(
                c,
                STOP_SIZE / 2.0 + 1.0,
                EStroke::new(1.0, Tokens::SELECTION),
            );
        }
    }
}

/// The chosen stop's balloon: its colour, opacity and midpoint, applied
/// at once; OK, Esc or a click outside closes it.
pub fn stop_popup(app: &mut App, ctx: &egui::Context) {
    let Some((pos, shown)) = app.gradient_tool.popup else {
        return;
    };
    let current = app.gradient_stop().and_then(|(_, i)| {
        let (_, f, _) = app.gradient_target()?;
        Some((i, *f.stops.get(i)?))
    });
    let Some((i, stop)) = current.filter(|_| app.tool == Tool::InteractiveFill) else {
        app.gradient_tool.popup = None;
        return;
    };
    let palettes = crate::ui::style_windows::palettes_of(app);
    let mut picker = std::mem::take(&mut app.gradient_tool.picker);
    let mut new_color = None;
    let mut new_opacity = None;
    let mut new_mid = None;
    let mut dragging = false;
    let mut close = false;
    let area = egui::Area::new(egui::Id::new("gradient_stop_popup"))
        .order(egui::Order::Foreground)
        .fixed_pos(pos + egui::vec2(HANDLE_HIT as f32, HANDLE_HIT as f32))
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_width(POPUP_WIDTH);
                ui.label(
                    egui::RichText::new(format!(
                        "{} {} \u{b7} {:.0}%",
                        tr("gradtool.stop"),
                        i + 1,
                        stop.pos * 100.0
                    ))
                    .strong(),
                );
                let out = picker.show(ui, "gradient_stop", stop.color, &palettes);
                if let Some(c) = out.color {
                    new_color = Some(c);
                }
                ui.add_space(4.0);
                let mut op = (stop.opacity * 100.0).round();
                let r = ui
                    .add(egui::Slider::new(&mut op, 0.0..=100.0).suffix("%").text(tr("gradtool.opacity")))
                    .on_hover_text(tr("gradtool.opacity_tip"));
                dragging |= r.dragged();
                if r.changed() {
                    new_opacity = Some(op / 100.0);
                }
                let mut mid = (stop.midpoint * 100.0).round();
                let r = ui
                    .add(egui::Slider::new(&mut mid, 1.0..=99.0).suffix("%").text(tr("gradtool.midpoint")))
                    .on_hover_text(tr("gradtool.midpoint_tip"));
                dragging |= r.dragged();
                if r.changed() {
                    new_mid = Some(mid / 100.0);
                }
                if ui.button(tr("gradtool.ok")).clicked() {
                    close = true;
                }
            });
        });
    app.gradient_tool.picker = picker;
    let join = dragging && app.gradient_tool.popup_dragging;
    if new_color.is_some() || new_opacity.is_some() || new_mid.is_some() {
        app.edit_gradient_stop(join, |s| {
            if let Some(c) = new_color {
                s.color = c;
            }
            if let Some(o) = new_opacity {
                s.opacity = o;
            }
            if let Some(m) = new_mid {
                s.midpoint = m;
            }
        });
    }
    app.gradient_tool.popup_dragging = dragging;
    let outside = shown
        && ctx.input(|i| i.pointer.any_pressed())
        && !area.response.contains_pointer();
    if close || outside || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        app.gradient_tool.popup = None;
    } else {
        app.gradient_tool.popup = Some((pos, true));
    }
}

fn dim(ui: &mut egui::Ui, key: &str) {
    ui.label(
        egui::RichText::new(tr(key))
            .color(Tokens::text_dim())
            .size(11.0),
    );
}

/// The Interactive Fill bar's gradient fields after the kind, colours,
/// angle and edge: the size, the y radius (elliptical, square), steps,
/// blending, smoothing and Reverse change `f` (true when one did); the
/// chosen stop's position, opacity and midpoint change the stop at once.
pub fn bar_fields(app: &mut App, ui: &mut egui::Ui, f: &mut Fountain) -> bool {
    use crate::measure_field::number_field;
    use crate::ui::style_windows::icon_button;
    use graphdraw_core::{FountainBlend, FountainKind};
    let mut changed = false;
    if f.kind != FountainKind::Shaped && f.kind != FountainKind::Conical {
        dim(ui, "fillwin.size");
        if let Some(v) = number_field(ui, "gradbar_size", f.scale * 100.0, 0, "%", 56.0) {
            f.scale = (v / 100.0).clamp(gradient::MIN_SCALE, gradient::MAX_SCALE);
            changed = true;
        }
    }
    if matches!(f.kind, FountainKind::Elliptical | FountainKind::Square) {
        dim(ui, "gradtool.aspect");
        if let Some(v) = number_field(ui, "gradbar_aspect", f.aspect * 100.0, 0, "%", 56.0) {
            f.aspect = (v / 100.0).clamp(0.01, 100.0);
            changed = true;
        }
    }
    dim(ui, "fillwin.steps");
    let locked = f.steps > 0;
    if icon_button(ui, "fillwin.steps_lock", &tr("fillwin.steps_lock"), locked, true).clicked() {
        f.steps = if locked { 0 } else { 16 };
        changed = true;
    }
    if locked {
        if let Some(v) = number_field(ui, "gradbar_steps", f.steps as f64, 0, "", 40.0) {
            f.steps = (v.round().max(2.0) as u32).min(gradient::MAX_STEPS);
            changed = true;
        }
    }
    for (b, key) in [
        (FountainBlend::Direct, "fillwin.blend_direct"),
        (FountainBlend::Clockwise, "fillwin.blend_cw"),
        (FountainBlend::Counterclockwise, "fillwin.blend_ccw"),
    ] {
        if icon_button(ui, key, &tr(key), f.blend == b, true).clicked() && f.blend != b {
            f.blend = b;
            changed = true;
        }
    }
    if icon_button(ui, "fillwin.smooth_tip", &tr("fillwin.smooth_tip"), f.smooth, true).clicked() {
        f.smooth = !f.smooth;
        changed = true;
    }
    if icon_button(ui, "fillwin.reverse_tip", &tr("fillwin.reverse_tip"), false, true).clicked() {
        gradient::reverse_stops(f);
        app.gradient_tool.stop = None;
        changed = true;
    }
    // The chosen stop.
    if let Some((_, i)) = app.gradient_stop() {
        if let Some(s) = f.stops.get(i).copied() {
            ui.add(egui::Separator::default().vertical().spacing(6.0));
            dim(ui, "gradtool.stop");
            let mut edited = false;
            let mut c = s.color;
            let palette = app.palette.clone();
            if crate::ui::fill_tool_bars::color_pick(
                ui,
                &palette,
                "gradbar_stop",
                &mut c,
                &tr("gradtool.stop_color"),
                true,
            ) {
                edited |= app.set_gradient_stop_color(c);
            }
            if let Some(v) = number_field(ui, "gradbar_stop_pos", s.pos * 100.0, 0, "%", 52.0) {
                edited |= app.edit_gradient_stop(false, |s| s.pos = v / 100.0);
            }
            dim(ui, "gradtool.opacity");
            if let Some(v) = number_field(ui, "gradbar_stop_op", s.opacity * 100.0, 0, "%", 52.0) {
                edited |= app.edit_gradient_stop(false, |s| s.opacity = v / 100.0);
            }
            dim(ui, "gradtool.midpoint");
            if let Some(v) = number_field(ui, "gradbar_stop_mid", s.midpoint * 100.0, 0, "%", 52.0)
            {
                edited |= app.edit_gradient_stop(false, |s| s.midpoint = v / 100.0);
            }
            // A stop field changed the drawing already: the bar goes on
            // from there.
            if let Some((_, g, _)) = app.gradient_target().filter(|_| edited) {
                *f = g;
            }
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    use graphdraw_core::document::ShapeKind;

    fn app_with_rect(fill: Fill) -> (App, ShapeId) {
        let mut app = App::headless();
        let id = app
            .new_shape(ShapeKind::Rect {
                rect: Rect::new(50.0, 100.0, 110.0, 140.0),
                radius: 0.0,
                corners: None,
            })
            .expect("shape");
        app.select(vec![id]);
        app.apply_fill(fill);
        app.tool = Tool::InteractiveFill;
        (app, id)
    }

    fn fountain(app: &App, id: ShapeId) -> Fountain {
        match &app.doc().find_shape(id).expect("shape").fill {
            Fill::Fountain(f) => f.clone(),
            f => panic!("no gradient: {f:?}"),
        }
    }

    /// The old tool's walk-through: a new gradient from the drag, its end
    /// dragged, a stop added by double click, coloured, deleted, and each
    /// step undone in turn.
    #[test]
    fn interactive_gradient_tool() {
        let red = Color::rgb8(255, 0, 0);
        let (mut app, id) = app_with_rect(Fill::Solid(red));
        // From (55, 120) to (105, 120) over the object.
        app.finish_fill_gradient(id, Point::new(55.0, 120.0), Point::new(105.0, 120.0));
        let f = fountain(&app, id);
        let b = app.doc().find_shape(id).expect("shape").bounds();
        let (a, z) = f.linear_axis(b);
        assert!((a - Point::new(55.0, 120.0)).hypot() < 1e-6, "{a:?}");
        assert!((z - Point::new(105.0, 120.0)).hypot() < 1e-6, "{z:?}");
        assert_eq!(f.stops[0].color, red, "starts from the solid colour");
        // Drag the end handle to (85, 120): one step.
        assert!(app.begin_gradient_drag(Point::new(105.0, 120.0)));
        for x in [100.0, 95.0, 90.0, 85.0] {
            app.drag_gradient_to(Point::new(x, 120.0), DragMods::default());
        }
        app.end_drag();
        let (_, z) = fountain(&app, id).linear_axis(b);
        assert!((z.x - 85.0).abs() < 1e-6, "{z:?}");
        // Double click halfway along the axis: a new stop, chosen.
        assert!(app.add_gradient_stop_at(Point::new(70.0, 120.0)));
        let f = fountain(&app, id);
        assert_eq!(f.stops.len(), 3);
        assert_eq!(app.gradient_stop().map(|s| s.1), Some(1));
        assert!((f.stops[1].pos - 0.5).abs() < 0.02, "{}", f.stops[1].pos);
        // A palette colour goes to the chosen stop.
        let blue = Color::rgb8(0, 0, 255);
        app.apply_fill(Fill::Solid(blue));
        assert_eq!(fountain(&app, id).stops[1].color, blue);
        assert!(app.delete_gradient_stop());
        assert_eq!(fountain(&app, id).stops.len(), 2);
        // Undo goes back step by step: delete, colour, stop, handle, new.
        app.undo();
        assert_eq!(fountain(&app, id).stops.len(), 3);
        // After an undo the choice no longer holds.
        assert!(!app.delete_gradient_stop());
        app.undo();
        assert_ne!(fountain(&app, id).stops[1].color, blue);
        app.undo();
        assert_eq!(fountain(&app, id).stops.len(), 2);
        assert!((fountain(&app, id).linear_axis(b).1.x - 85.0).abs() < 1e-6);
        app.undo();
        assert!((fountain(&app, id).linear_axis(b).1.x - 105.0).abs() < 1e-6);
        app.undo();
        assert!(matches!(
            app.doc().find_shape(id).expect("shape").fill,
            Fill::Solid(_)
        ));
    }

    /// A click on the gradient's end chooses the end stop and opens the
    /// balloon; Esc closes it.
    #[test]
    fn clicking_gradient_end_opens_stop_popup() {
        let (mut app, _) = app_with_rect(Fill::linear(Color::BLACK, Color::WHITE, 0.0));
        assert!(app.gradient_click(Point::new(110.0, 120.0), Pos2::new(10.0, 10.0)));
        assert!(app.gradient_tool.popup.is_some(), "balloon open");
        assert_eq!(app.gradient_stop().map(|s| s.1), Some(1), "the end stop");
        let ctx = crate::theme::ui_context();
        let frame = |app: &mut App, events: Vec<egui::Event>| {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, egui::vec2(1200.0, 800.0))),
                events,
                ..Default::default()
            };
            let mut out = ctx.run_ui(input, |ui| stop_popup(app, ui.ctx()));
            out.textures_delta.clear();
        };
        frame(&mut app, vec![]);
        assert!(app.gradient_tool.popup.is_some());
        let esc = egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        frame(&mut app, vec![esc]);
        assert!(app.gradient_tool.popup.is_none());
        // A click off the handles and the axis takes the choice away.
        assert!(!app.gradient_click(Point::new(60.0, 135.0), Pos2::ZERO));
    }

    #[test]
    fn stops_drag_along_the_axis_and_stay_chosen_in_order() {
        let (mut app, id) = app_with_rect(Fill::linear(Color::BLACK, Color::WHITE, 0.0));
        let undo_len = |app: &App| app.engine.history_labels().0.len();
        // Stops at 50% (index 1) and 25% (index 1, the other moves to 2).
        assert!(app.add_gradient_stop_at(Point::new(80.0, 120.0)));
        assert!(app.add_gradient_stop_at(Point::new(65.0, 120.0)));
        assert_eq!(app.gradient_stop().map(|s| s.1), Some(1));
        let quarter = fountain(&app, id).stops[1];
        // Drag the 25% stop past the 50% one: one step, still chosen.
        let before = undo_len(&app);
        assert!(app.begin_gradient_drag(Point::new(65.0, 120.0)));
        app.drag_gradient_to(Point::new(95.0, 120.0), DragMods::default());
        app.drag_gradient_to(Point::new(100.0, 120.0), DragMods::default());
        app.end_drag();
        assert_eq!(undo_len(&app), before + 1, "one step");
        let f = fountain(&app, id);
        assert!(f.stops.windows(2).all(|w| w[0].pos <= w[1].pos), "in order");
        assert_eq!(app.gradient_stop().map(|s| s.1), Some(2), "still chosen");
        assert_eq!(f.stops[2].color, quarter.color);
        assert!((f.stops[2].pos - 50.0 / 60.0).abs() < 1e-6, "{}", f.stops[2].pos);
    }

    #[test]
    fn colours_dropped_on_a_stop_recolour_it() {
        let (mut app, id) = app_with_rect(Fill::linear(Color::BLACK, Color::WHITE, 0.0));
        app.select(Vec::new());
        app.canvas_rect = egui::Rect::from_min_size(Pos2::ZERO, egui::vec2(4000.0, 4000.0));
        let at = app.view.to_screen(Point::new(110.0, 120.0));
        if !app.canvas_rect.contains(at) {
            return;
        }
        assert_eq!(app.gradient_stop_under(Point::new(110.0, 120.0)), Some((id, 1)));
        let green = Color::rgb8(0, 200, 0);
        assert!(app.drop_color_on_stop(green, at));
        assert_eq!(fountain(&app, id).stops[1].color, green);
        assert!(app.selection.is_empty(), "the selection stays");
        // Off the stops the drop is not taken here.
        let mid = app.view.to_screen(Point::new(80.0, 130.0));
        assert!(!app.drop_color_on_stop(green, mid));
    }

    #[test]
    fn ctrl_snaps_a_new_gradient_to_15_degrees() {
        let e = snap_end(Point::ZERO, Point::new(10.0, 1.0), true);
        assert!(e.y.abs() < 1e-9 && (e.x - 101f64.sqrt()).abs() < 1e-9);
        assert_eq!(snap_end(Point::ZERO, Point::new(10.0, 1.0), false), Point::new(10.0, 1.0));
    }
}
