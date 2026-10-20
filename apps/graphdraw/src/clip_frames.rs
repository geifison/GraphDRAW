//! Clip frames from the Pick tool and the clip frame bar: Ctrl+click
//! edits a frame's contents (again on an inner frame to go deeper) and
//! Ctrl+click on an empty spot finishes; the contents are centred in their
//! frame, scaled to fill it or to fit it, or extracted; and a short notice
//! next to the pointer says why a request did nothing.
//! See docs/behavior/clip-frame.md.

use crate::app::App;
use crate::i18n::tr;
use graphdraw_core::{
    document::{Shape, ShapeKind},
    geometry::{Affine, Point, Rect, Vec2},
    Command, ShapeId,
};

/// How the contents of a clip frame are put in place in their frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentFit {
    /// Moved to the frame's centre, size unchanged.
    Center,
    /// Scaled in proportion until they cover the whole frame (what
    /// sticks out is clipped), then centred.
    Fill,
    /// Scaled in proportion until they just fit inside the frame, then
    /// centred.
    Fit,
}

/// The transform that puts `contents` (bounds) in `frame` (bounds) as
/// `how` asks; `None` when there is nothing to measure.
pub fn content_fit_transform(contents: Rect, frame: Rect, how: ContentFit) -> Option<Affine> {
    let finite = |r: Rect| [r.x0, r.y0, r.x1, r.y1].iter().all(|v| v.is_finite());
    if !finite(contents) || !finite(frame) {
        return None;
    }
    let (cw, ch) = (contents.width(), contents.height());
    let (fw, fh) = (frame.width(), frame.height());
    let ratios: Vec<f64> = [(fw, cw), (fh, ch)]
        .iter()
        .filter(|(_, c)| *c > 1e-9)
        .map(|(f, c)| f / c)
        .collect();
    let scale = match how {
        ContentFit::Center => 1.0,
        ContentFit::Fill => ratios.iter().copied().fold(f64::NAN, f64::max),
        ContentFit::Fit => ratios.iter().copied().fold(f64::NAN, f64::min),
    };
    let scale = if scale.is_finite() && scale > 0.0 { scale } else { 1.0 };
    let c = contents.center();
    let f = frame.center();
    Some(
        Affine::translate(Vec2::new(f.x, f.y))
            * Affine::scale(scale)
            * Affine::translate(Vec2::new(-c.x, -c.y)),
    )
}

/// `s` as text when it is a clip frame whose frame is text: the frame,
/// with the clip frame's id and the two transforms together, so the text
/// panels, the property bar and in-place editing work on it (the commands
/// they send to that id reach the frame, see `Command::SetShapeKind`).
/// Anything else is returned unchanged.
pub fn text_view(s: Shape) -> Shape {
    match &s.kind {
        ShapeKind::ClipFrame { frame, .. } if matches!(frame.kind, ShapeKind::Text { .. }) => {
            let mut f = (**frame).clone();
            f.id = s.id;
            f.transform = s.transform * f.transform;
            f
        }
        _ => s,
    }
}

/// How many clip frames deep `s` goes: 0 for an object that holds none,
/// 1 for a clip frame of plain objects, 2 when one of those is a clip
/// frame too...
pub fn clip_depth(s: &Shape) -> usize {
    match &s.kind {
        ShapeKind::ClipFrame { contents, .. } => {
            1 + contents.iter().map(clip_depth).max().unwrap_or(0)
        }
        ShapeKind::Group { children } => children.iter().map(clip_depth).max().unwrap_or(0),
        _ => 0,
    }
}

impl App {
    /// The clip frames among the selected objects (on a layer), in the
    /// document's own space (not page space).
    pub fn selected_clip_frames(&self) -> Vec<Shape> {
        self.selection
            .iter()
            .filter_map(|id| self.doc().shape(*id).ok().map(|(_, s)| s.clone()))
            .filter(|s| matches!(s.kind, ShapeKind::ClipFrame { .. }))
            .collect()
    }

    /// The clip frame bar's Center, Fill and Fit: the contents of each
    /// selected clip frame put in place in their frame; one undo step.
    pub fn fit_clip_contents(&mut self, how: ContentFit) {
        let mut cmds = Vec::new();
        for clip in self.selected_clip_frames() {
            let ShapeKind::ClipFrame { frame, contents } = clip.kind else {
                continue;
            };
            let Some(bounds) = contents
                .iter()
                .map(|c| c.bounds())
                .reduce(|a, b| a.union(b))
            else {
                continue;
            };
            let Some(t) = content_fit_transform(bounds, frame.bounds(), how) else {
                continue;
            };
            let contents = contents
                .into_iter()
                .map(|mut c| {
                    c.absorb(t);
                    c
                })
                .collect();
            cmds.push(Command::SetShapeKind {
                shape: clip.id,
                kind: ShapeKind::ClipFrame { frame, contents },
            });
        }
        if cmds.is_empty() {
            self.clip_notice(tr("status.clip_frame_empty"));
            return;
        }
        let name = match how {
            ContentFit::Center => "Center Contents",
            ContentFit::Fill => "Fill Frame Proportionally",
            ContentFit::Fit => "Fit Contents Proportionally",
        };
        if let Err(e) = self.engine.run_batch(name, &cmds) {
            self.status = e.to_string();
        }
    }

    /// Object > Clip to Frame > Extract Contents (and the bar's button):
    /// the contents of the selected clip frames become ordinary objects
    /// next to their frames, which stay; one undo step. Empty frames are
    /// left alone, with a notice when nothing could be extracted.
    pub fn extract_clip_contents(&mut self) {
        let clips = self.selected_clip_frames();
        if clips.is_empty() {
            return;
        }
        let mut contents = Vec::new();
        let cmds: Vec<Command> = clips
            .iter()
            .filter_map(|s| match &s.kind {
                ShapeKind::ClipFrame { contents: c, .. } if !c.is_empty() => {
                    contents.extend(c.iter().map(|c| c.id));
                    Some(Command::ExtractContents { clip: s.id })
                }
                _ => None,
            })
            .collect();
        if cmds.is_empty() {
            self.clip_notice(tr("status.clip_frame_nothing_to_extract"));
            return;
        }
        if let Err(e) = self.engine.run_batch("Extract Contents", &cmds) {
            self.status = e.to_string();
            return;
        }
        self.select(contents);
    }

    /// Ctrl+click with the Pick tool at `p`: on a clip frame, edit its
    /// contents (deeper when it lies among contents being edited); on an
    /// empty spot while editing, finish that level. False when the click
    /// is left to the ordinary Ctrl+click.
    pub fn clip_frame_ctrl_click(&mut self, p: Point) -> bool {
        match self.hit_test(p) {
            Some(id)
                if self
                    .doc()
                    .shape(id)
                    .is_ok_and(|(_, s)| matches!(s.kind, ShapeKind::ClipFrame { .. })) =>
            {
                self.select(vec![id]);
                self.edit_clip_frame();
                true
            }
            Some(_) => false,
            None if self.clip_frame_edit.is_some() => {
                self.finish_clip_frame_edit();
                true
            }
            None => false,
        }
    }

    /// While contents are edited: the level being edited (1 for the
    /// outermost frame) and how many levels the frames go down from the
    /// outermost one.
    pub fn clip_frame_edit_depth(&self) -> Option<(usize, usize)> {
        let (_, ids) = self.clip_frame_edit.as_ref()?;
        let level = self.clip_frame_outer.len() + 1;
        let below = ids
            .iter()
            .filter_map(|id| self.doc().find_shape(*id))
            .map(clip_depth)
            .max()
            .unwrap_or(0);
        Some((level, level + below))
    }

    /// While a clip frame's contents are edited only they can be picked
    /// (by a click or a marquee); the frame and everything else cannot.
    pub fn clip_edit_allows(&self, id: ShapeId) -> bool {
        match &self.clip_frame_edit {
            Some((_, ids)) => ids.contains(&id),
            None => true,
        }
    }

    /// Leave every level of clip frame editing, innermost first.
    pub fn finish_all_clip_frame_edits(&mut self) {
        // Each call drops one level, so this ends.
        while self.clip_frame_edit.is_some() {
            self.finish_clip_frame_edit();
        }
    }

    /// Place Inside Frame waiting for its frame, clicked at `p`: why
    /// nothing under the pointer can take the contents (a locked object,
    /// or nothing at all).
    pub fn clip_frame_target_missed(&mut self, p: Point) {
        let locked = self
            .page_layers()
            .iter()
            .filter(|l| l.visible)
            .flat_map(|l| l.shapes.iter().map(move |s| (l.locked, s)))
            .any(|(layer_locked, s)| {
                (layer_locked || s.locked) && s.visible && s.bounds().contains(p)
            });
        if locked {
            self.clip_notice(tr("status.clip_frame_locked"));
        } else {
            self.status = tr("status.clip_frame_cancelled");
        }
    }

    /// Say `text` on the status bar and, for a moment, next to the
    /// pointer.
    pub fn clip_notice(&mut self, text: String) {
        self.status = text.clone();
        self.clip_notice = Some((text, None));
    }

    /// The ids a list of shapes on a layer may take as contents of
    /// `frame`: everything but the frame, when any is left.
    pub fn clip_frame_contents_for(&self, frame: ShapeId) -> Result<Vec<ShapeId>, String> {
        let contents: Vec<ShapeId> = self
            .selection
            .iter()
            .copied()
            .filter(|id| *id != frame)
            .collect();
        if !contents.is_empty() {
            Ok(contents)
        } else if self.selection.contains(&frame) {
            Err(tr("status.clip_frame_other_frame"))
        } else {
            Err(tr("status.clip_frame_select_contents"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::Tool;
    use graphdraw_core::{Color, Fill};

    fn rect_shape(app: &mut App, r: Rect) -> ShapeId {
        let id = app
            .new_shape(ShapeKind::Rect {
                rect: r,
                radius: 0.0,
                corners: None,
            })
            .expect("a layer");
        app.run(Command::SetFill {
            shapes: vec![id],
            fill: Fill::Solid(Color::BLACK),
        });
        id
    }

    fn clip_bounds(app: &App, clip: ShapeId) -> Rect {
        let ShapeKind::ClipFrame { contents, .. } = &app.doc().find_shape(clip).unwrap().kind
        else {
            panic!("not a clip frame");
        };
        contents
            .iter()
            .map(|c| c.bounds())
            .reduce(|a, b| a.union(b))
            .unwrap()
    }

    /// A 100 by 50 frame at the origin holding a 20 by 40 rectangle in
    /// its lower left corner.
    fn framed(app: &mut App) -> (ShapeId, ShapeId) {
        let frame = rect_shape(app, Rect::new(0.0, 0.0, 100.0, 50.0));
        let inner = rect_shape(app, Rect::new(5.0, 5.0, 25.0, 45.0));
        app.place_inside(vec![inner], frame);
        (frame, inner)
    }

    #[test]
    fn fit_transforms_centre_fill_and_fit() {
        let c = Rect::new(0.0, 0.0, 20.0, 40.0);
        let f = Rect::new(0.0, 0.0, 100.0, 50.0);
        let r = |how| content_fit_transform(c, f, how).unwrap().transform_rect_bbox(c);
        let centred = r(ContentFit::Center);
        assert_eq!((centred.center().x, centred.center().y), (50.0, 25.0));
        assert!((centred.width() - 20.0).abs() < 1e-9);
        let filled = r(ContentFit::Fill);
        assert!((filled.width() - 100.0).abs() < 1e-9 && filled.height() >= 50.0);
        let fitted = r(ContentFit::Fit);
        assert!((fitted.height() - 50.0).abs() < 1e-9 && fitted.width() <= 100.0);
        // Nothing to measure: no transform; a flat line still centres.
        assert!(content_fit_transform(Rect::new(0.0, f64::NAN, 1.0, 1.0), f, ContentFit::Fill).is_none());
        let line = Rect::new(0.0, 0.0, 10.0, 0.0);
        let t = content_fit_transform(line, f, ContentFit::Fill).unwrap();
        assert!((t.transform_rect_bbox(line).width() - 100.0).abs() < 1e-9);
    }

    #[test]
    fn the_bar_centres_fills_and_fits_in_one_undo_step_each() {
        let mut app = App::headless();
        let (frame, _) = framed(&mut app);
        app.select(vec![frame]);
        for (how, check) in [
            (ContentFit::Center, (20.0, 40.0)),
            (ContentFit::Fill, (100.0, 200.0)),
            (ContentFit::Fit, (25.0, 50.0)),
        ] {
            let before = clip_bounds(&app, frame);
            app.fit_clip_contents(how);
            let b = clip_bounds(&app, frame);
            assert!((b.center().x - 50.0).abs() < 1e-6 && (b.center().y - 25.0).abs() < 1e-6);
            assert!((b.width() - check.0).abs() < 1e-6, "{how:?}: {b:?}");
            assert!((b.height() - check.1).abs() < 1e-6, "{how:?}: {b:?}");
            app.undo();
            assert_eq!(clip_bounds(&app, frame), before, "{how:?} undoes in one step");
            app.fit_clip_contents(how);
        }
    }

    #[test]
    fn ctrl_click_edits_and_ctrl_click_outside_finishes() {
        let mut app = App::headless();
        app.set_tool(Tool::Pick);
        let (frame, inner) = framed(&mut app);
        assert!(app.clip_frame_ctrl_click(Point::new(50.0, 25.0)));
        assert_eq!(app.clip_frame_edit.as_ref().map(|e| e.0), Some(frame));
        assert_eq!(app.selection, vec![inner]);
        assert_eq!(app.clip_frame_edit_depth(), Some((1, 1)));
        // Not a clip frame: the ordinary Ctrl+click.
        assert!(!app.clip_frame_ctrl_click(Point::new(15.0, 25.0)));
        // Only the contents can be picked: the frame cannot.
        assert_eq!(app.hit_test(Point::new(15.0, 25.0)), Some(inner));
        assert_eq!(app.hit_test(Point::new(80.0, 25.0)), None);
        assert!(!app.clip_edit_allows(frame));
        assert!(app.clip_frame_ctrl_click(Point::new(500.0, 500.0)));
        assert!(app.clip_frame_edit.is_none());
        assert!(matches!(
            app.doc().find_shape(frame).unwrap().kind,
            ShapeKind::ClipFrame { .. }
        ));
        assert_eq!(app.selection, vec![frame]);
        // Not editing: an empty spot is the ordinary click.
        assert!(!app.clip_frame_ctrl_click(Point::new(500.0, 500.0)));
    }

    #[test]
    fn nested_clip_frames_are_edited_level_by_level() {
        let mut app = App::headless();
        let (inner_frame, _) = framed(&mut app);
        let outer = rect_shape(&mut app, Rect::new(-10.0, -10.0, 110.0, 60.0));
        app.place_inside(vec![inner_frame], outer);
        app.select(vec![outer]);
        app.edit_clip_frame();
        assert_eq!(app.clip_frame_edit_depth(), Some((1, 2)));
        assert!(app.clip_frame_ctrl_click(Point::new(50.0, 25.0)));
        assert_eq!(app.clip_frame_edit_depth(), Some((2, 2)));
        app.finish_clip_frame_edit();
        assert_eq!(app.clip_frame_edit_depth(), Some((1, 2)));
        app.finish_clip_frame_edit();
        assert!(app.clip_frame_edit.is_none());
        let outer_shape = app.doc().find_shape(outer).unwrap().clone();
        assert_eq!(clip_depth(&outer_shape), 2);
    }

    #[test]
    fn an_empty_frame_stays_a_frame_after_editing() {
        let mut app = App::headless();
        let frame = rect_shape(&mut app, Rect::new(0.0, 0.0, 50.0, 50.0));
        app.select(vec![frame]);
        app.objects_to_clip_frames();
        app.select(vec![frame]);
        app.edit_clip_frame();
        app.finish_all_clip_frame_edits();
        assert!(matches!(
            &app.doc().find_shape(frame).unwrap().kind,
            ShapeKind::ClipFrame { contents, .. } if contents.is_empty()
        ));
    }

    #[test]
    fn extracting_an_empty_frame_says_so_and_changes_nothing() {
        let mut app = App::headless();
        let frame = rect_shape(&mut app, Rect::new(0.0, 0.0, 50.0, 50.0));
        app.select(vec![frame]);
        app.objects_to_clip_frames();
        app.select(vec![frame]);
        let rev = app.engine.revision();
        app.extract_clip_contents();
        assert_eq!(app.engine.revision(), rev);
        assert_eq!(app.status, tr("status.clip_frame_nothing_to_extract"));
        assert!(app.clip_notice.is_some());
        // With contents: they come out selected, the frame stays.
        let (full, inner) = framed(&mut app);
        app.select(vec![full]);
        app.extract_clip_contents();
        assert_eq!(app.selection, vec![inner]);
        assert!(app.doc().shape(full).is_ok() && app.doc().shape(inner).is_ok());
    }

    /// Text that became a clip frame is still text for the text tools: the
    /// selection reads as text, and a new font reaches the frame while the
    /// contents stay inside.
    #[test]
    fn a_text_clip_frame_keeps_its_text_properties() {
        let mut app = App::headless();
        let text = app
            .new_shape(ShapeKind::Text {
                spans: vec![graphdraw_core::TextSpan::new("Brasil", "Selawik", 32.0)],
                origin: Point::new(0.0, 0.0),
                frame: None,
                align: Default::default(),
                para: Default::default(),
                on_path: None,
            })
            .unwrap();
        let flag = rect_shape(&mut app, Rect::new(0.0, 0.0, 60.0, 20.0));
        app.place_inside(vec![flag], text);
        app.select(vec![text]);
        assert!(matches!(
            app.selected_shapes()[0].kind,
            ShapeKind::Text { .. }
        ));
        assert_eq!(app.selected_clip_frames().len(), 1);
        app.text_font = "Other Sans".into();
        app.apply_text_style();
        let ShapeKind::ClipFrame { frame, contents } = &app.doc().find_shape(text).unwrap().kind
        else {
            panic!("the clip frame was lost");
        };
        let ShapeKind::Text { spans, .. } = &frame.kind else {
            panic!("the frame is no longer text");
        };
        assert_eq!(spans[0].font_family, "Other Sans");
        assert_eq!(contents.len(), 1);
    }

    #[test]
    fn a_frame_cannot_take_itself_and_locked_targets_are_named() {
        let mut app = App::headless();
        let a = rect_shape(&mut app, Rect::new(0.0, 0.0, 50.0, 50.0));
        app.select(vec![a]);
        assert_eq!(
            app.clip_frame_contents_for(a),
            Err(tr("status.clip_frame_other_frame"))
        );
        app.select(Vec::new());
        assert_eq!(
            app.clip_frame_contents_for(a),
            Err(tr("status.clip_frame_select_contents"))
        );
        app.select(vec![a]);
        app.set_locked(true);
        app.clip_frame_target_missed(Point::new(25.0, 25.0));
        assert_eq!(app.status, tr("status.clip_frame_locked"));
        app.clip_frame_target_missed(Point::new(500.0, 500.0));
        assert_eq!(app.status, tr("status.clip_frame_cancelled"));
    }
}
