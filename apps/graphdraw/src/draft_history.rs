//! Undo and redo of the drawing in progress, node by node (GD-UND-01):
//! the nodes of a Bezier, Pen, Polyline or B-Spline curve not finished
//! yet, the base of a 3-point tool and the points of a callout. Each frame
//! the in-progress state is compared with the last one; a change is a
//! step. A drag (a handle being pulled) is one step when released. Once
//! the drawing becomes an object (or is dropped) its steps go: from then
//! on the drawing's own history undoes.

use crate::app::{App, CurveInProgress, Drag};
use graphdraw_core::Point;

/// The drawing in progress at one moment.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DraftSnap {
    curve: Option<CurveInProgress>,
    three_point_base: Option<(Point, Point)>,
    dimension_points: Vec<Point>,
}

impl DraftSnap {
    fn is_empty(&self) -> bool {
        *self == DraftSnap::default()
    }
}

#[derive(Debug, Default)]
pub struct DraftHistory {
    undo: Vec<DraftSnap>,
    redo: Vec<DraftSnap>,
    last: DraftSnap,
    /// Revision of the drawing when the steps were last recorded.
    rev: u64,
}

impl App {
    fn draft_busy(&self) -> bool {
        !matches!(self.drag, Drag::None) || self.curve.as_ref().is_some_and(|c| c.dragging_handle)
    }

    fn draft_snap(&self) -> DraftSnap {
        DraftSnap {
            curve: self.curve.clone(),
            three_point_base: self.three_point_base,
            dimension_points: self.dimension_points.clone(),
        }
    }

    fn set_draft(&mut self, s: DraftSnap) {
        self.curve = s.curve.clone();
        self.three_point_base = s.three_point_base;
        self.dimension_points = s.dimension_points.clone();
        self.drafts.last = s;
        self.drafts.rev = self.engine.revision();
    }

    /// Record a step of the drawing in progress if it changed since the
    /// last frame.
    pub fn track_draft(&mut self) {
        if self.draft_busy() {
            return;
        }
        let cur = self.draft_snap();
        let rev = self.engine.revision();
        if cur == self.drafts.last {
            // A redo kept with nothing in progress loses its turn when the
            // drawing changes some other way.
            if cur.is_empty() && rev != self.drafts.rev {
                self.drafts.redo.clear();
            }
            return;
        }
        let prev = std::mem::replace(&mut self.drafts.last, cur.clone());
        self.drafts.redo.clear();
        if cur.is_empty() {
            self.drafts.undo.clear();
        } else {
            self.drafts.undo.push(prev);
        }
        self.drafts.rev = rev;
    }

    pub fn can_undo_draft(&self) -> bool {
        !self.draft_busy() && !self.drafts.undo.is_empty()
    }

    pub fn can_redo_draft(&self) -> bool {
        !self.draft_busy() && !self.drafts.redo.is_empty()
    }

    /// Take back the last node or point of the drawing in progress.
    pub fn undo_draft(&mut self) -> bool {
        if !self.can_undo_draft() {
            return false;
        }
        // Follow this frame's change first, so it is the step undone.
        self.track_draft();
        let Some(prev) = self.drafts.undo.pop() else {
            return false;
        };
        let cur = self.draft_snap();
        self.drafts.redo.push(cur);
        self.set_draft(prev);
        true
    }

    /// Put back the node or point just undone.
    pub fn redo_draft(&mut self) -> bool {
        if !self.can_redo_draft() {
            return false;
        }
        let Some(next) = self.drafts.redo.pop() else {
            return false;
        };
        let cur = self.draft_snap();
        self.drafts.undo.push(cur);
        self.set_draft(next);
        true
    }
}

#[cfg(test)]
mod tests {
    use crate::app::{App, CurveInProgress};
    use graphdraw_core::Point;

    fn place(app: &mut App, x: f64) {
        app.curve
            .get_or_insert_with(CurveInProgress::default)
            .nodes
            .push((Point::new(x, 0.0), None));
        app.track_draft();
    }

    #[test]
    fn nodes_of_the_curve_in_progress_undo_one_by_one() {
        let mut app = App::headless();
        app.track_draft();
        place(&mut app, 0.0);
        place(&mut app, 10.0);
        place(&mut app, 20.0);
        let nodes = |app: &App| app.curve.as_ref().map_or(0, |c| c.nodes.len());
        assert_eq!(nodes(&app), 3);
        app.undo();
        assert_eq!(nodes(&app), 2);
        app.undo();
        assert_eq!(nodes(&app), 1);
        app.redo();
        assert_eq!(nodes(&app), 2);
        app.undo();
        app.undo();
        assert!(app.curve.is_none(), "first node undone: nothing in progress");
        app.redo();
        assert_eq!(nodes(&app), 1);
    }

    #[test]
    fn a_finished_curve_is_undone_by_the_drawing() {
        let mut app = App::headless();
        app.track_draft();
        place(&mut app, 0.0);
        place(&mut app, 10.0);
        app.finish_curve();
        app.track_draft();
        assert!(!app.can_undo_draft());
        assert!(app.engine.undo_label().is_some());
    }
}
