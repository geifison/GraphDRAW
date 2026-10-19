//! Strokes of the shaping brushes (Smooth, Smear, Twirl, Attract and
//! Repel, Smudge, Roughen): a drag over the selected objects (or the one
//! under the pointer when nothing is selected) reshapes the curves under
//! the nib, step by step, the whole stroke one undo step. Objects that are
//! not curves are converted first. The settings come from the property bar
//! (`ui::brush_bars`).

use crate::app::{App, Drag};
use crate::tools::Tool;
use crate::ui::brush_bars::BrushSettings;
use egui::{Modifiers, PointerButton, Response};
use tracedraw_core::{effects, geometry::Point, Command, ShapeKind};

/// How far Twirl and Attract and Repel go per second while the button is
/// held still, as a pointer move (mm).
pub const HOLD_MM_PER_S: f64 = 10.0;

impl App {
    /// A stroke begins at `p`.
    pub fn brush_begin(&mut self, p: Point) {
        if self.selection.is_empty() {
            if let Some(id) = self.hit_test(p) {
                self.select(vec![id]);
            }
        }
        // Brushes work on curves.
        self.convert_to_curves();
        self.drag = Drag::Node { last: p };
        self.brush_stepped = false;
    }

    /// The stroke moves on to `p`, `dt` seconds after the last step;
    /// `swap` (Alt or Shift held) swaps Attract and Repel and Twirl's
    /// turn. Twirl and Attract and Repel go on while the button is held
    /// still, as if the pointer moved [`HOLD_MM_PER_S`].
    pub fn brush_step(&mut self, p: Point, swap: bool, dt: f64) {
        let Drag::Node { last } = self.drag else {
            return;
        };
        let delta = p - last;
        let radius = self.brush_radius;
        let b = self.brush;
        let held = if dt.is_finite() {
            dt.clamp(0.0, 0.1)
        } else {
            0.0
        };
        let motion = delta.hypot() + HOLD_MM_PER_S * held;
        let turn = if swap { -1.0 } else { 1.0 };
        let mut cmds = Vec::new();
        for s in self.selected_shapes() {
            let ShapeKind::Path { path, closed } = &s.kind else {
                continue;
            };
            let inv = s.transform.inverse();
            let lp = inv * p;
            let ld = (inv * (Point::ZERO + delta)) - (inv * Point::ZERO);
            let new_path = match self.tool {
                Tool::Smear => effects::smear(
                    path,
                    lp,
                    ld * BrushSettings::pressure(b.smear_pressure),
                    radius,
                ),
                Tool::Twirl => effects::twirl(path, lp, motion * b.twirl_rate() * turn, radius),
                Tool::AttractRepel => {
                    effects::attract(path, lp, radius, motion * b.attract_rate(swap))
                }
                Tool::Smudge => effects::smear(
                    &effects::smooth(path, lp, radius, 0.2),
                    lp,
                    ld * BrushSettings::pressure(b.smudge_pressure),
                    radius,
                ),
                Tool::Roughen => {
                    effects::roughen(path, lp, radius, self.roughen_amount, b.roughen_spacing())
                }
                Tool::Smooth => effects::smooth(path, lp, radius, b.smooth_tolerance()),
                _ => continue,
            };
            if &new_path == path {
                continue;
            }
            cmds.push(Command::SetShapeKind {
                shape: s.id,
                kind: ShapeKind::Path {
                    path: new_path,
                    closed: *closed,
                },
            });
        }
        self.drag = Drag::Node { last: p };
        if cmds.is_empty() {
            return;
        }
        let label = match self.tool {
            Tool::Smear => "Smear",
            Tool::Twirl => "Twirl",
            Tool::AttractRepel => "Attract",
            Tool::Smudge => "Smudge",
            Tool::Roughen => "Roughen",
            _ => "Smooth",
        };
        // One undo step for the whole stroke.
        if self.brush_stepped && self.engine.undo_label() == Some(label) {
            let _ = self.engine.undo();
        }
        if self.engine.run_batch(label, &cmds).is_ok() {
            self.brush_stepped = true;
        }
    }

    /// The brushes' pointer input.
    pub fn brush_input(&mut self, response: &Response, p: Point, mods: Modifiers) {
        if response.drag_started_by(PointerButton::Primary) {
            self.brush_begin(p);
        }
        if response.dragged_by(PointerButton::Primary) {
            let held = matches!(self.tool, Tool::Twirl | Tool::AttractRepel);
            let dt = if held {
                // Keep going while the button is held still.
                response.ctx.request_repaint();
                response.ctx.input(|i| i.stable_dt) as f64
            } else {
                0.0
            };
            self.brush_step(p, mods.alt || mods.shift, dt);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracedraw_core::{Rect, ShapeId};

    fn square(app: &mut App) -> ShapeId {
        let id = app
            .new_shape(ShapeKind::Rect {
                rect: Rect::new(0.0, 0.0, 40.0, 40.0),
                radius: 0.0,
                corners: None,
            })
            .expect("shape");
        app.select(vec![id]);
        id
    }

    fn bounds(app: &App, id: ShapeId) -> Rect {
        app.doc().shape_on_page(id).expect("shape").bounds()
    }

    /// A stroke is one undo step whatever its length; the next stroke is
    /// another. Smear's pressure scales how far the curve follows.
    #[test]
    fn a_stroke_is_one_step_and_follows_the_settings() {
        let mut app = App::headless();
        let id = square(&mut app);
        app.tool = Tool::Smear;
        app.brush_radius = 10.0;
        app.brush_begin(Point::new(40.0, 20.0));
        let depth = app.engine.history_labels().0.len();
        for x in [42.0, 44.0, 46.0] {
            app.brush_step(Point::new(x, 20.0), false, 0.0);
        }
        assert_eq!(app.engine.history_labels().0.len(), depth + 1);
        let full = bounds(&app, id).x1;
        assert!(full > 42.5, "{full}");
        app.undo();
        // Half the pressure: half as far.
        app.brush.smear_pressure = 50;
        app.brush_begin(Point::new(40.0, 20.0));
        for x in [42.0, 44.0, 46.0] {
            app.brush_step(Point::new(x, 20.0), false, 0.0);
        }
        let half = bounds(&app, id).x1;
        assert!(half > 40.5 && half < full - 0.5, "{half} vs {full}");
    }

    /// Attract pulls and Repel pushes; Shift swaps them. Twirl turns the
    /// other way when set clockwise.
    #[test]
    fn attract_repel_and_twirl_follow_their_modes() {
        let mut app = App::headless();
        let id = square(&mut app);
        app.tool = Tool::AttractRepel;
        app.brush_radius = 8.0;
        // How far out the corner by the nib, (40, 40), now is (x + y).
        let stroke = |app: &mut App, shift: bool| {
            app.brush_begin(Point::new(36.0, 36.0));
            app.brush_step(Point::new(37.0, 36.0), shift, 0.0);
            app.brush_step(Point::new(36.0, 36.0), shift, 0.0);
            let reach = match &app.doc().find_shape(id).expect("shape").kind {
                ShapeKind::Path { path, .. } => path
                    .elements()
                    .iter()
                    .filter_map(|e| match e {
                        tracedraw_core::geometry::PathEl::MoveTo(q)
                        | tracedraw_core::geometry::PathEl::LineTo(q) => Some(q.x + q.y),
                        _ => None,
                    })
                    .fold(f64::MIN, f64::max),
                _ => 0.0,
            };
            app.undo();
            reach
        };
        assert!(stroke(&mut app, false) < 80.0);
        assert!(stroke(&mut app, true) > 80.0);
        app.brush.repel = true;
        assert!(stroke(&mut app, false) > 80.0);
        assert!(stroke(&mut app, true) < 80.0);
        // Twirl near the top right corner: the corner goes up one way and
        // across the other.
        app.tool = Tool::Twirl;
        app.brush_radius = 15.0;
        let twirl = |app: &mut App| {
            app.brush_begin(Point::new(35.0, 35.0));
            for x in [36.0, 38.0, 40.0, 42.0] {
                app.brush_step(Point::new(x, 35.0), false, 0.0);
            }
            let b = bounds(app, id);
            app.undo();
            b
        };
        let ccw = twirl(&mut app);
        app.brush.twirl_clockwise = true;
        let cw = twirl(&mut app);
        assert_ne!(ccw, cw);
        // Held still, Twirl still turns.
        let before = bounds(&app, id);
        app.brush_begin(Point::new(35.0, 35.0));
        for _ in 0..5 {
            app.brush_step(Point::new(35.0, 35.0), false, 0.1);
        }
        assert_ne!(bounds(&app, id), before);
    }
}
