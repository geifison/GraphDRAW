//! Text fitted to a path once it is there: the selected fitted text, its
//! settings edited from the property bar, and dragging it along the path
//! and away from it with the Pick tool (see docs/behavior/text.md).

use crate::app::App;
use tracedraw_core::{
    document::{ShapeKind, TextAlign, TextOnPath},
    geometry::{Affine, BezPath, Point},
    Command, ShapeId,
};

/// The one selected text fitted to a path.
#[derive(Debug, Clone)]
pub struct FittedText {
    pub id: ShapeId,
    pub on_path: TextOnPath,
    pub align: TextAlign,
    /// From the text's path space to the page.
    pub to_page: Affine,
}

impl App {
    /// The selection is a single text fitted to a path.
    pub fn fitted_text(&self) -> Option<FittedText> {
        let [id] = self.selection.as_slice() else {
            return None;
        };
        let (_, s) = self.doc().shape(*id).ok()?;
        match &s.kind {
            ShapeKind::Text {
                on_path: Some(tp),
                origin,
                align,
                ..
            } => Some(FittedText {
                id: *id,
                on_path: tp.clone(),
                align: *align,
                to_page: s.transform * Affine::translate(origin.to_vec2()),
            }),
            _ => None,
        }
    }

    /// Change the selected fitted text's settings: one undo step, nothing
    /// done when `f` leaves them as they were.
    pub fn edit_fitted_text(&mut self, f: impl FnOnce(&mut TextOnPath, &mut TextAlign)) {
        let Some(ft) = self.fitted_text() else {
            return;
        };
        let Ok((_, s)) = self.doc().shape(ft.id) else {
            return;
        };
        let ShapeKind::Text {
            spans,
            origin,
            frame,
            para,
            ..
        } = s.kind.clone()
        else {
            return;
        };
        let (mut tp, mut align) = (ft.on_path.clone(), ft.align);
        f(&mut tp, &mut align);
        if tp == ft.on_path && align == ft.align {
            return;
        }
        self.run(Command::SetShapeKind {
            shape: ft.id,
            kind: ShapeKind::Text {
                spans,
                origin,
                frame,
                align,
                para,
                on_path: Some(tp),
            },
        });
    }

    /// The fitted text's settings with the pointer dragged from `start`
    /// to `current` (page): moved along the path by the way the pointer
    /// went along it, and away from it by the way it went across.
    pub fn along_path_settings(&self, start: Point, current: Point) -> Option<FittedText> {
        let mut ft = self.fitted_text()?;
        let to_local = ft.to_page.inverse();
        let guide = ft.on_path.guide();
        let (s0, d0) = guide.nearest(to_local * start)?;
        let (s1, d1) = guide.nearest(to_local * current)?;
        let mut ds = s1 - s0;
        // Round a closed path the short way.
        let total = guide.length();
        if guide.is_closed() && total > 0.0 {
            if ds > total / 2.0 {
                ds -= total;
            } else if ds < -total / 2.0 {
                ds += total;
            }
        }
        // Distance grows towards the letters' tops: the guide's left, or
        // its right when the letters are flipped.
        let across = if ft.on_path.flipped() {
            d0 - d1
        } else {
            d1 - d0
        };
        ft.on_path.offset += ds;
        ft.on_path.distance += across;
        Some(ft)
    }

    /// What a drag along the path shows: the letters' outline and the
    /// line they stand on, both in page space.
    pub fn along_path_preview(&self, start: Point, current: Point) -> Option<(BezPath, BezPath)> {
        let ft = self.along_path_settings(start, current)?;
        let (_, s) = self.doc().shape(ft.id).ok()?;
        let ShapeKind::Text { spans, para, .. } = &s.kind else {
            return None;
        };
        let req = tracedraw_core::document::text_outline::TextRequest {
            spans,
            frame: None,
            align: ft.align,
            para,
            on_path: Some(&ft.on_path),
        };
        let letters = tracedraw_core::document::text_outline::outline(&req).unwrap_or_default();
        let base = ft.on_path.baseline();
        let total = base.length();
        let steps = ((total / 1.0).ceil() as usize).clamp(2, 4000);
        let mut line = BezPath::new();
        for i in 0..=steps {
            if let Some((p, _)) = base.at(total * i as f64 / steps as f64) {
                if i == 0 {
                    line.move_to(p);
                } else {
                    line.line_to(p);
                }
            }
        }
        Some((ft.to_page * letters, ft.to_page * line))
    }

    /// The end of a drag along the path: the new offset and distance, one
    /// undo step.
    pub fn finish_along_path(&mut self, start: Point, current: Point) {
        if (current - start).hypot() < 1e-9 {
            return;
        }
        let Some(ft) = self.along_path_settings(start, current) else {
            return;
        };
        self.edit_fitted_text(|tp, _| {
            tp.offset = ft.on_path.offset;
            tp.distance = ft.on_path.distance;
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracedraw_core::document::{PathTextOrientation, PathTextVertical};
    use tracedraw_core::geometry::Rect;

    fn fitted_on_line(app: &mut App) -> ShapeId {
        let mut line = BezPath::new();
        line.move_to((10.0, 50.0));
        line.line_to((110.0, 50.0));
        let l = app
            .new_shape(ShapeKind::Path {
                path: line,
                closed: false,
            })
            .expect("line");
        app.start_text(Point::new(10.0, 120.0), None);
        app.text_insert("Path");
        app.finish_text();
        let t = app.selection[0];
        app.selection = vec![t, l];
        app.begin_fit_text_to_path();
        t
    }

    #[test]
    fn the_bar_settings_edit_the_text_with_undo_and_come_back_on_reselect() {
        let mut app = App::headless();
        let t = fitted_on_line(&mut app);
        assert!(app.fitted_text().is_some());
        app.edit_fitted_text(|tp, align| {
            tp.orientation = PathTextOrientation::Upright;
            tp.vertical = PathTextVertical::Center;
            tp.distance = 3.0;
            tp.offset = 7.0;
            tp.set_mirrors(true, false);
            *align = TextAlign::Right;
        });
        // Reselected: the values come back from the text.
        app.selection.clear();
        assert!(app.fitted_text().is_none());
        app.selection = vec![t];
        let ft = app.fitted_text().expect("fitted");
        assert_eq!(ft.on_path.orientation, PathTextOrientation::Upright);
        assert_eq!(ft.on_path.vertical, PathTextVertical::Center);
        assert_eq!((ft.on_path.distance, ft.on_path.offset), (3.0, 7.0));
        assert_eq!(ft.on_path.mirrors(), (true, false));
        assert_eq!(ft.align, TextAlign::Right);
        // One step back undoes all of it.
        app.undo();
        app.selection = vec![t];
        let ft = app.fitted_text().expect("fitted");
        assert_eq!(ft.on_path.orientation, PathTextOrientation::Rotate);
        assert_eq!(ft.on_path.distance, 0.0);
        // An edit that changes nothing adds no undo step.
        let before = app.engine.history_labels().0.len();
        app.edit_fitted_text(|_, _| {});
        assert_eq!(app.engine.history_labels().0.len(), before);
    }

    #[test]
    fn dragging_moves_the_text_along_and_away_from_the_path() {
        let mut app = App::headless();
        let t = fitted_on_line(&mut app);
        app.selection = vec![t];
        // 20 mm along the line and 4 mm above it.
        let ft = app
            .along_path_settings(Point::new(40.0, 50.0), Point::new(60.0, 54.0))
            .expect("settings");
        assert!((ft.on_path.offset - 20.0).abs() < 1e-6, "{:?}", ft.on_path);
        assert!((ft.on_path.distance - 4.0).abs() < 1e-6, "{:?}", ft.on_path);
        let (letters, line) = app
            .along_path_preview(Point::new(40.0, 50.0), Point::new(60.0, 54.0))
            .expect("preview");
        assert!(!line.elements().is_empty());
        let _ = letters;
        app.finish_along_path(Point::new(40.0, 50.0), Point::new(60.0, 54.0));
        let ft = app.fitted_text().expect("fitted");
        assert!((ft.on_path.offset - 20.0).abs() < 1e-6);
        assert!((ft.on_path.distance - 4.0).abs() < 1e-6);
        // Flipped letters grow away from the path on the other side.
        app.edit_fitted_text(|tp, _| tp.set_mirrors(false, true));
        let ft = app
            .along_path_settings(Point::new(40.0, 50.0), Point::new(40.0, 47.0))
            .expect("settings");
        assert!((ft.on_path.distance - 7.0).abs() < 1e-6, "{:?}", ft.on_path);
    }

    #[test]
    fn fit_text_to_path_with_nothing_selected_leaves_no_stray_text() {
        let mut app = App::headless();
        let e = app
            .new_shape(ShapeKind::Ellipse {
                rect: Rect::new(40.0, 100.0, 140.0, 160.0),
                arc: None,
            })
            .expect("ellipse");
        let count = |app: &App| -> usize {
            app.doc().pages[0]
                .layers
                .iter()
                .map(|l| l.shapes.len())
                .sum()
        };
        // Picked, nothing typed, left: only the ellipse remains.
        app.selection.clear();
        app.begin_fit_text_to_path();
        app.fit_text_to_picked_path(e);
        assert_eq!(count(&app), 2);
        app.finish_text();
        assert_eq!(count(&app), 1);
        // Picked and typed: one text, on the path, no straight copy.
        app.selection.clear();
        app.begin_fit_text_to_path();
        app.fit_text_to_picked_path(e);
        app.text_insert("round");
        app.finish_text();
        assert_eq!(count(&app), 2);
        let texts: Vec<_> = app.doc().pages[0]
            .layers
            .iter()
            .flat_map(|l| &l.shapes)
            .filter_map(|s| match &s.kind {
                ShapeKind::Text { on_path, .. } => Some(on_path.is_some()),
                _ => None,
            })
            .collect();
        assert_eq!(texts, vec![true]);
        // Choosing the command again while typing an empty text drops it.
        app.start_text(Point::new(10.0, 10.0), None);
        app.begin_fit_text_to_path();
        assert_eq!(count(&app), 2);
    }
}
