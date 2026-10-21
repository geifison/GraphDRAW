//! Text on a path, the editing that goes beyond the settings: the live
//! link to the guide curve (fit at a point, pick the guide with a retry,
//! unlink), the ghost path kept by Straighten Text and Reactivate Source
//! Path, Mirror Horizontally keeping the text in place, the top and
//! bottom badge on a closed path, the guide as a construction line and
//! the letter spacing (see docs/behavior/text.md).

use crate::app::App;
use graphdraw_core::{
    document::{ShapeKind, TextAlign, TextOnPath, TextSpan},
    geometry::{Affine, BezPath, Point},
    text_path::{can_guide, offset_for, text_to_page, Track},
    Color, Command, Shape, ShapeId, Stroke,
};

/// Cap height as a share of the body.
const CAP: f64 = 0.72;

/// The body of a text (its first span), mm.
pub fn body_mm(spans: &[TextSpan]) -> f64 {
    spans
        .first()
        .map_or(12.0, |s| s.size_pt)
        .max(0.1)
        * graphdraw_text::PT_MM
}

/// How far the baseline is moved for the line `v` of the letters to sit on
/// the stroke (the font's own ascent and descent taken as usual shares of
/// the body).
fn lift(v: graphdraw_core::PathTextVertical, size: f64) -> f64 {
    use graphdraw_core::PathTextVertical as V;
    match v {
        V::Baseline => 0.0,
        V::Ascender => -0.93 * size,
        V::Descender => 0.24 * size,
        V::Center => -0.35 * size,
        V::XHeight => -0.5 * size,
        V::CapHeight => -CAP * size,
    }
}

/// Where the text `spans` laid on `tp` lies along it.
pub fn path_span(
    spans: &[TextSpan],
    para: &graphdraw_core::ParagraphStyle,
    align: TextAlign,
    tp: &TextOnPath,
) -> Option<graphdraw_text::PathSpan> {
    graphdraw_text::fonts()
        .layout(&graphdraw_core::document::text_outline::TextRequest {
            spans,
            frame: None,
            align,
            para,
            on_path: Some(tp),
        })
        .path_span
}

/// The parts of a text shape on a path.
struct OnPath {
    spans: Vec<TextSpan>,
    origin: Point,
    align: TextAlign,
    para: graphdraw_core::ParagraphStyle,
    tp: TextOnPath,
}

fn parts(s: &Shape) -> Option<OnPath> {
    match &s.kind {
        ShapeKind::Text {
            spans,
            origin,
            align,
            para,
            on_path: Some(tp),
            ..
        } => Some(OnPath {
            spans: spans.clone(),
            origin: *origin,
            align: *align,
            para: para.clone(),
            tp: tp.clone(),
        }),
        _ => None,
    }
}

fn kind_of(p: OnPath) -> ShapeKind {
    ShapeKind::Text {
        spans: p.spans,
        origin: p.origin,
        frame: None,
        align: p.align,
        para: p.para,
        on_path: Some(p.tp),
    }
}

/// A justified text on a path made to start with the spacing the
/// justification gave it (the same look), so its letter spacing can come
/// down: justified it always fills the path. Unchanged otherwise.
fn unjustified(p: &mut OnPath) {
    if !p.align.is_justified() {
        return;
    }
    let Some(now) = path_span(&p.spans, &p.para, p.align, &p.tp) else {
        return;
    };
    let mut start = OnPath {
        spans: p.spans.clone(),
        origin: p.origin,
        align: TextAlign::Left,
        para: p.para.clone(),
        tp: TextOnPath {
            offset: now.start,
            ..p.tp.clone()
        },
    };
    let Some(nat) = path_span(&start.spans, &start.para, start.align, &start.tp) else {
        return;
    };
    let glyphs = graphdraw_core::document::spans_text(&p.spans)
        .chars()
        .filter(|c| *c != '\n')
        .count()
        .saturating_sub(1)
        .max(1);
    let extra_pct = ((now.end - now.start) - (nat.end - nat.start))
        / (glyphs as f64 * body_mm(&p.spans))
        * 100.0;
    for s in &mut start.spans {
        s.tracking_pct += extra_pct;
    }
    *p = start;
}

impl App {
    /// `id` can guide a text: a curve or shape with an outline.
    pub fn can_guide_text(&self, id: ShapeId) -> bool {
        self.doc()
            .find_shape(id)
            .is_some_and(|s| can_guide(&s.kind) && !s.local_path().elements().is_empty())
    }

    /// The offset that makes the text start (or centre, or end, by its
    /// alignment) at the point of `tp` nearest the page point `at`, for a
    /// text whose path space goes to the page by `to_page`.
    pub fn path_offset_at(tp: &TextOnPath, align: TextAlign, to_page: Affine, at: Point) -> f64 {
        let Some(track) = Track::new(tp) else {
            return tp.offset;
        };
        let det = to_page.determinant();
        if !det.is_finite() || det.abs() < 1e-12 {
            return tp.offset;
        }
        let s = track.project(to_page.inverse() * at);
        offset_for(align, s, track.total())
    }

    /// Fit the selected text to `curve`, starting at the page point `at`
    /// (the click, the drop): one undo step.
    pub fn fit_text_to_path_at(&mut self, text: ShapeId, curve: ShapeId, at: Point) {
        self.selection = vec![text, curve];
        self.fit_text_to_path();
        let Some(ft) = self.fitted_text() else {
            return;
        };
        let offset = Self::path_offset_at(&ft.on_path, ft.align, ft.to_page, at);
        let Ok((_, s)) = self.doc().shape(text) else {
            return;
        };
        let Some(mut p) = parts(s) else {
            return;
        };
        p.tp.offset = offset;
        let _ = self.engine.amend(&Command::SetShapeKind {
            shape: text,
            kind: kind_of(p),
        });
    }

    /// The click of Fit Text to Path that picks the guide missed a curve:
    /// a window asks whether to try again.
    pub fn text_path_pick_missed(&mut self) {
        self.dialog = crate::ui::dialogs::Dialog::RetryTextPath;
    }

    /// Mirror Horizontally (bar): the text reads the other way along the
    /// path without leaving its place or the band it fills beside the
    /// stroke, which rights a text that came out upside down along the
    /// bottom of a circle. One undo step.
    pub fn mirror_text_reading(&mut self) {
        let Some(ft) = self.fitted_text() else {
            return;
        };
        let Ok((_, s)) = self.doc().shape(ft.id) else {
            return;
        };
        let Some(mut p) = parts(s) else {
            return;
        };
        let Some(span) = path_span(&p.spans, &p.para, p.align, &p.tp) else {
            return;
        };
        let size = body_mm(&p.spans);
        let below = |tp: &TextOnPath| if tp.below { -CAP * size } else { 0.0 };
        // The band of the letters: from the baseline (`off`) to the cap
        // height along the letters' "up"; reading the other way, "up"
        // changes sides, so the same band starts at -(off + cap).
        let off = p.tp.distance + lift(p.tp.vertical, size) + below(&p.tp);
        let (h, v) = p.tp.mirrors();
        let mut n = p.tp.clone();
        n.set_mirrors(!h, !v);
        n.distance = -(off + CAP * size) - lift(n.vertical, size) - below(&n);
        // The middle of the text stays where it was (on the reversed path,
        // total - s).
        let w = span.end - span.start;
        let mid = span.total - (span.start + span.end) / 2.0;
        n.offset = match p.align {
            TextAlign::Center => mid - span.total / 2.0,
            TextAlign::Right => mid + w / 2.0 - span.total,
            _ => mid - w / 2.0,
        };
        p.tp = n;
        self.set_text_kind("Mirror Horizontally", ft.id, kind_of(p));
    }

    /// Mirror Vertically (bar) and the canvas double arrow: the letters go
    /// to the other side of the stroke, reading the same way.
    pub fn flip_text_side(&mut self) {
        self.edit_fitted_text(|tp, _| tp.below = !tp.below);
    }

    fn set_text_kind(&mut self, label: &'static str, id: ShapeId, kind: ShapeKind) {
        if let Err(e) = self
            .engine
            .run_batch(label, &[Command::SetShapeKind { shape: id, kind }])
        {
            self.status = e.to_string();
        }
    }

    /// The guide of the selected fitted text, when it is in the drawing.
    pub fn fitted_guide(&self) -> Option<ShapeId> {
        let g = self.fitted_text()?.on_path.guide?;
        self.doc().find_shape(g).map(|_| g)
    }

    /// Stroke as Construction (bar): the selected text's guide shows on
    /// screen and is left out of printing, PDF, SVG and cutting, or back.
    pub fn toggle_guide_construction(&mut self) {
        let Some(g) = self.fitted_guide() else {
            return;
        };
        let on = self.doc().find_shape(g).is_some_and(|s| s.construction);
        self.run(Command::SetConstruction {
            shapes: vec![g],
            construction: !on,
        });
    }

    /// A curve to stand in for a guide that is gone: the path the text
    /// follows, as a thin black line on the text's layer.
    fn recreated_guide(&mut self, text: &Shape, tp: &TextOnPath) -> Option<(ShapeId, Command)> {
        let (layer, _) = self.doc().locate(text.id).ok()?;
        let page = text_to_page(self.doc(), text) * tp.path.clone();
        let id = self.engine.new_shape_id();
        let mut line = Shape::new(
            id,
            ShapeKind::Path {
                path: page,
                closed: false,
            },
        );
        line.fill = graphdraw_core::Fill::None;
        line.stroke = Some(Stroke::new(Color::rgb8(0, 0, 0), 0.25));
        Some((id, Command::AddShape { layer, shape: line }))
    }

    /// The commands that unlink the text on a path `id` from its guide
    /// (Ctrl+K on it, Unlink on the bar): both become independent, the
    /// text keeping its curve; a guide that is gone is drawn again from
    /// the path the text follows. `None` when `id` is no linked text.
    pub fn text_path_unlink_commands(&mut self, id: ShapeId) -> Option<Vec<Command>> {
        self.text_path_unlink(id).map(|(cmds, _)| cmds)
    }

    /// [`App::text_path_unlink_commands`] with the guide the text leaves
    /// (the one drawn again when it was gone).
    fn text_path_unlink(&mut self, id: ShapeId) -> Option<(Vec<Command>, ShapeId)> {
        let s = self.doc().find_shape(id)?.clone();
        let mut p = parts(&s)?;
        // Only a linked text (its guide there or gone).
        p.tp.guide?;
        let live = p.tp.guide.filter(|g| self.doc().find_shape(*g).is_some());
        let mut cmds = Vec::new();
        let guide = match live {
            Some(g) => g,
            None => {
                let (g, add) = self.recreated_guide(&s, &p.tp)?;
                cmds.push(add);
                cmds.push(Command::OrderRelative {
                    shapes: vec![g],
                    reference: id,
                    in_front: false,
                });
                g
            }
        };
        p.tp.guide = None;
        cmds.push(Command::SetShapeKind {
            shape: id,
            kind: kind_of(p),
        });
        Some((cmds, guide))
    }

    /// Unlink (bar): text and path become independent objects, the text
    /// keeping its curve; both come out selected.
    pub fn unlink_text_path(&mut self) {
        let Some(ft) = self.fitted_text() else {
            return;
        };
        let Some((cmds, guide)) = self.text_path_unlink(ft.id) else {
            return;
        };
        if let Err(e) = self.engine.run_batch("Unlink Text from Path", &cmds) {
            self.status = e.to_string();
            return;
        }
        self.select(vec![guide, ft.id]);
    }

    /// The ghost path of a straightened text, in its space after
    /// straightening, from the text before it (`old`) and the transform it
    /// gets (`new_transform`).
    pub fn ghost_path(old: &Shape, new_transform: Affine) -> Option<Box<TextOnPath>> {
        let p = parts(old)?;
        let o = Affine::translate(p.origin.to_vec2());
        let det = new_transform.determinant();
        if !det.is_finite() || det.abs() < 1e-12 {
            return None;
        }
        let path: BezPath = o.inverse() * new_transform.inverse() * old.transform * o * p.tp.path.clone();
        Some(Box::new(TextOnPath { path, ..p.tp }))
    }

    /// The selection can go back to its source path: one text that was
    /// straightened (it keeps the path) or is on a path without its guide.
    pub fn can_reactivate_text_path(&self) -> bool {
        let [id] = self.selection.as_slice() else {
            return false;
        };
        let Ok((_, s)) = self.doc().shape(*id) else {
            return false;
        };
        match &s.kind {
            ShapeKind::Text {
                on_path: Some(tp), ..
            } => tp
                .guide
                .is_none_or(|g| self.doc().find_shape(g).is_none()),
            ShapeKind::Text { on_path: None, .. } => s.text_path_memory.is_some(),
            _ => false,
        }
    }

    /// Reactivate Source Path (context menu): the text goes back to the
    /// path it kept and to the link with its guide; a guide that is gone
    /// is drawn again below the text with the kept shape. One undo step.
    pub fn reactivate_text_path(&mut self) {
        if !self.can_reactivate_text_path() {
            return;
        }
        let [id] = self.selection.as_slice() else {
            return;
        };
        let id = *id;
        let Ok((_, s)) = self.doc().shape(id) else {
            return;
        };
        let s = s.clone();
        let ShapeKind::Text {
            spans,
            origin,
            align,
            para,
            on_path,
            ..
        } = s.kind.clone()
        else {
            return;
        };
        let Some(mut tp) = on_path.or_else(|| s.text_path_memory.as_deref().cloned()) else {
            return;
        };
        let mut cmds = Vec::new();
        let live = tp.guide.filter(|g| self.doc().find_shape(*g).is_some());
        let guide = match live {
            Some(g) => g,
            None => {
                let Some((g, add)) = self.recreated_guide(&s, &tp) else {
                    return;
                };
                cmds.push(add);
                cmds.push(Command::OrderRelative {
                    shapes: vec![g],
                    reference: id,
                    in_front: false,
                });
                g
            }
        };
        tp.guide = Some(guide);
        cmds.push(Command::SetShapeKind {
            shape: id,
            kind: ShapeKind::Text {
                spans,
                origin,
                frame: None,
                align,
                para,
                on_path: Some(tp),
            },
        });
        cmds.push(Command::SetTextPathMemory {
            shape: id,
            memory: None,
        });
        if let Err(e) = self.engine.run_batch("Reactivate Source Path", &cmds) {
            self.status = e.to_string();
        }
        self.selection = vec![id];
    }

    /// The text on a closed path can become a badge.
    pub fn can_split_badge(&self) -> bool {
        self.fitted_text()
            .and_then(|ft| Track::new(&ft.on_path))
            .is_some_and(|t| t.closed())
    }

    /// Badge (closed path): the text becomes the top one, reading outside
    /// centred on the highest point, and gains a bottom one reading left
    /// to right inside centred on the lowest point, its cap height on the
    /// stroke (both in the same band). Both come out selected; one undo
    /// step.
    pub fn split_badge(&mut self) {
        let Some(ft) = self.fitted_text() else {
            return;
        };
        let Ok((layer, _)) = self.doc().locate(ft.id) else {
            return;
        };
        let Ok((_, s)) = self.doc().shape(ft.id) else {
            return;
        };
        let s = s.clone();
        let Some(mut top) = parts(&s) else {
            return;
        };
        top.align = TextAlign::Center;
        top.tp.set_mirrors(false, false);
        top.tp.vertical = graphdraw_core::PathTextVertical::Baseline;
        top.tp.below = false;
        let Some(track) = Track::new(&top.tp).filter(|t| t.closed()) else {
            return;
        };
        top.tp.offset = track.extreme(true) - track.total() / 2.0;
        let mut bottom = OnPath {
            spans: top.spans.clone(),
            origin: top.origin,
            align: TextAlign::Center,
            para: top.para.clone(),
            tp: top.tp.clone(),
        };
        bottom.tp.set_mirrors(true, true);
        bottom.tp.vertical = graphdraw_core::PathTextVertical::CapHeight;
        let Some(track) = Track::new(&bottom.tp) else {
            return;
        };
        bottom.tp.offset = track.extreme(false) - track.total() / 2.0;
        let label = crate::i18n::tr("text_path.bottom_text");
        bottom.spans.truncate(1);
        if let Some(first) = bottom.spans.first_mut() {
            first.text = label;
        }
        let twin_id = self.engine.new_shape_id();
        let mut twin = s.clone();
        twin.id = twin_id;
        twin.name = None;
        twin.uid = None;
        twin.kind = kind_of(bottom);
        let cmds = [
            Command::SetShapeKind {
                shape: ft.id,
                kind: kind_of(top),
            },
            Command::AddShape { layer, shape: twin },
        ];
        if let Err(e) = self.engine.run_batch("Badge", &cmds) {
            self.status = e.to_string();
            return;
        }
        self.select(vec![ft.id, twin_id]);
    }

    /// The letter spacing of the selected texts, % of the body.
    pub fn letter_spacing(&self) -> Option<f64> {
        self.text_shapes().first().and_then(|s| match &s.kind {
            ShapeKind::Text { spans, .. } => spans.first().map(|sp| sp.tracking_pct),
            _ => None,
        })
    }

    /// Set the letter spacing of every selected text to `pct` % of the
    /// body (a justified text on a path first starts with the spacing its
    /// justification gave, so the spacing can come down). With `amend`
    /// the change joins the step just made (a drag of the icon).
    pub fn set_letter_spacing(&mut self, pct: f64, amend: bool) {
        let pct = pct.clamp(-90.0, 1000.0);
        let mut cmds = Vec::new();
        for s in self.text_shapes() {
            let mut kind = s.kind.clone();
            if let Some(mut p) = parts(&s) {
                let before = p.spans.first().map_or(0.0, |sp| sp.tracking_pct);
                unjustified(&mut p);
                let delta = pct - before;
                for sp in &mut p.spans {
                    sp.tracking_pct += delta;
                }
                kind = kind_of(p);
            } else if let ShapeKind::Text { spans, .. } = &mut kind {
                for sp in spans.iter_mut() {
                    sp.tracking_pct = pct;
                }
            }
            if kind != s.kind {
                cmds.push(Command::SetShapeKind { shape: s.id, kind });
            }
        }
        if cmds.is_empty() {
            return;
        }
        let r = if amend {
            self.engine.amend_batch("Letter Spacing", &cmds)
        } else {
            self.engine.run_batch("Letter Spacing", &cmds)
        };
        if let Err(e) = r {
            self.status = e.to_string();
        }
    }

    /// [`App::edit_fitted_text`] for a continuous adjustment (an icon
    /// dragged on the bar): with `amend` the change joins the step the
    /// drag began, so the whole drag is one undo step.
    pub fn edit_fitted_text_live(
        &mut self,
        amend: bool,
        f: impl FnOnce(&mut TextOnPath, &mut TextAlign),
    ) {
        let Some(ft) = self.fitted_text() else {
            return;
        };
        let Ok((_, s)) = self.doc().shape(ft.id) else {
            return;
        };
        let Some(mut p) = parts(s) else {
            return;
        };
        let before = (p.tp.clone(), p.align);
        f(&mut p.tp, &mut p.align);
        if (p.tp.clone(), p.align) == before {
            return;
        }
        let cmds = [Command::SetShapeKind {
            shape: ft.id,
            kind: kind_of(p),
        }];
        let r = if amend {
            self.engine.amend_batch("Text on Path", &cmds)
        } else {
            self.engine.run_batch("Text on Path", &cmds)
        };
        if let Err(e) = r {
            self.status = e.to_string();
        }
    }

    /// The text on a path `id` with its settings, spacing and alignment
    /// changed by `f` (the canvas handles): the kind to set.
    pub fn fitted_kind_with(
        &self,
        id: ShapeId,
        unjustify: bool,
        f: impl FnOnce(&mut TextOnPath, &mut Vec<TextSpan>),
    ) -> Option<ShapeKind> {
        let (_, s) = self.doc().shape(id).ok()?;
        let mut p = parts(s)?;
        if unjustify {
            unjustified(&mut p);
        }
        f(&mut p.tp, &mut p.spans);
        Some(kind_of(p))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text_path_handles::PathHandle;
    use graphdraw_core::geometry::{Rect, Vec2};

    fn line_at(app: &mut App, y: f64) -> ShapeId {
        let mut line = BezPath::new();
        line.move_to((10.0, y));
        line.line_to((110.0, y));
        app.new_shape(ShapeKind::Path {
            path: line,
            closed: false,
        })
        .expect("line")
    }

    fn text(app: &mut App, s: &str) -> ShapeId {
        app.start_text(Point::new(10.0, 150.0), None);
        app.text_insert(s);
        app.finish_text();
        app.selection[0]
    }

    /// A text fitted to a line from (10, 50) to (110, 50): (text, line).
    fn fitted(app: &mut App) -> (ShapeId, ShapeId) {
        let l = line_at(app, 50.0);
        let t = text(app, "Path");
        app.selection = vec![t, l];
        app.begin_fit_text_to_path();
        (t, l)
    }

    fn on_path(app: &App, t: ShapeId) -> Option<TextOnPath> {
        match &app.doc().find_shape(t)?.kind {
            ShapeKind::Text { on_path, .. } => on_path.clone(),
            _ => None,
        }
    }

    fn start_y(tp: &TextOnPath) -> f64 {
        tp.path
            .elements()
            .first()
            .and_then(|e| e.end_point())
            .map_or(f64::NAN, |p| p.y)
    }

    fn fonts() -> bool {
        graphdraw_text::install();
        !graphdraw_text::fonts().families().is_empty()
    }

    #[test]
    fn the_guide_lays_the_text_again_and_unlink_keeps_the_curve() {
        let mut app = App::headless();
        let (t, l) = fitted(&mut app);
        let tp = on_path(&app, t).expect("on the path");
        assert_eq!(tp.guide, Some(l));
        assert!(tp.relax && tp.corners == graphdraw_core::PathTextCorners::Jump);
        let base = start_y(&tp);
        app.run(Command::TransformShapes {
            shapes: vec![l],
            transform: Affine::translate((0.0, 20.0)),
        });
        let moved = on_path(&app, t).expect("on the path");
        assert!((start_y(&moved) - base - 20.0).abs() < 1e-6);
        // Unlink: the text keeps its curve and no longer follows.
        app.selection = vec![t];
        app.unlink_text_path();
        assert_eq!(app.selection, vec![l, t]);
        let free = on_path(&app, t).expect("still on its curve");
        assert_eq!(free.guide, None);
        app.run(Command::TransformShapes {
            shapes: vec![l],
            transform: Affine::translate((0.0, 20.0)),
        });
        assert!((start_y(&on_path(&app, t).expect("curve")) - start_y(&free)).abs() < 1e-9);
        // Ctrl+K on a linked text unlinks it too, one undo step.
        app.undo();
        app.undo();
        assert_eq!(on_path(&app, t).and_then(|p| p.guide), Some(l));
        app.selection = vec![t];
        app.break_apart();
        assert_eq!(on_path(&app, t).and_then(|p| p.guide), None);
        app.undo();
        assert_eq!(on_path(&app, t).and_then(|p| p.guide), Some(l));
    }

    #[test]
    fn straighten_keeps_a_ghost_path_and_reactivate_redraws_a_lost_guide() {
        let mut app = App::headless();
        let (t, l) = fitted(&mut app);
        app.selection = vec![t];
        app.straighten_text();
        assert!(on_path(&app, t).is_none());
        assert!(app.can_reactivate_text_path());
        // The guide is gone: reactivating draws it again below the text.
        app.run(Command::DeleteShapes { shapes: vec![l] });
        app.selection = vec![t];
        let steps = app.engine.history_labels().0.len();
        app.reactivate_text_path();
        assert_eq!(app.engine.history_labels().0.len(), steps + 1);
        let tp = on_path(&app, t).expect("back on the path");
        let g = tp.guide.expect("linked");
        assert_ne!(g, l);
        let (layer, gi) = app.doc().locate(g).expect("guide");
        let (_, ti) = app.doc().locate(t).expect("text");
        assert!(gi < ti, "the guide goes below the text");
        let _ = layer;
        // Where the path was: on the line at y = 50.
        let guide = app.doc().find_shape(g).expect("guide").page_path();
        let b = graphdraw_core::geometry::Shape::bounding_box(&guide);
        assert!((b.y0 - 50.0).abs() < 1e-6 && (b.x1 - 110.0).abs() < 1e-6, "{b:?}");
        assert!(!app.can_reactivate_text_path());
        app.undo();
        assert!(on_path(&app, t).is_none());
    }

    #[test]
    fn mirror_horizontally_keeps_the_text_in_place() {
        if !fonts() {
            return;
        }
        let mut app = App::headless();
        let e = app
            .new_shape(ShapeKind::Ellipse {
                rect: Rect::new(40.0, 100.0, 140.0, 200.0),
                arc: None,
            })
            .expect("ellipse");
        let t = text(&mut app, "SEAL OF QUALITY");
        app.selection = vec![t, e];
        app.begin_fit_text_to_path();
        // Down to the bottom of the circle, where it reads upside down.
        app.edit_fitted_text(|tp, _| {
            tp.offset += std::f64::consts::PI * 50.0;
            tp.relax = false;
        });
        let centre = |app: &App| {
            let s = app.doc().shape(t).expect("text").1.clone();
            app.visual_bounds(&s).center()
        };
        let before = centre(&app);
        let mirrors = on_path(&app, t).expect("on").mirrors();
        app.mirror_text_reading();
        let after = on_path(&app, t).expect("on");
        assert_eq!(after.mirrors(), (!mirrors.0, !mirrors.1));
        assert!((centre(&app) - before).hypot() < 3.0, "{before:?} {:?}", centre(&app));
        app.mirror_text_reading();
        assert!((centre(&app) - before).hypot() < 3.0);
    }

    #[test]
    fn badge_makes_a_top_and_a_bottom_text_and_the_guide_can_be_construction() {
        let mut app = App::headless();
        let e = app
            .new_shape(ShapeKind::Ellipse {
                rect: Rect::new(40.0, 100.0, 140.0, 200.0),
                arc: None,
            })
            .expect("ellipse");
        let t = text(&mut app, "SEAL");
        app.selection = vec![t, e];
        app.begin_fit_text_to_path();
        assert!(app.can_split_badge());
        app.split_badge();
        let [top, bottom] = app.selection.as_slice() else {
            panic!("two texts");
        };
        let (a, b) = (on_path(&app, *top).expect("top"), on_path(&app, *bottom).expect("bottom"));
        assert_eq!(a.mirrors(), (false, false));
        assert_eq!(b.mirrors(), (true, true));
        assert_eq!(b.vertical, graphdraw_core::PathTextVertical::CapHeight);
        assert_eq!((a.guide, b.guide), (Some(e), Some(e)));
        if fonts() {
            let c = |id: ShapeId| {
                let s = app.doc().shape(id).expect("text").1.clone();
                app.visual_bounds(&s).center()
            };
            assert!(c(*top).y > c(*bottom).y + 40.0);
        }
        // Undo takes both back at once.
        let bottom = *bottom;
        app.undo();
        assert!(app.doc().find_shape(bottom).is_none());
        app.selection = vec![t];
        app.toggle_guide_construction();
        assert!(app.doc().find_shape(e).is_some_and(|s| s.construction));
        app.undo();
        assert!(app.doc().find_shape(e).is_some_and(|s| !s.construction));
    }

    #[test]
    fn handles_slide_with_a_magnet_move_away_flip_and_space_letters() {
        if !fonts() {
            return;
        }
        let mut app = App::headless();
        let (t, _) = fitted(&mut app);
        app.selection = vec![t];
        let h = app.path_handles().expect("handles");
        // The diamond is at the start of the line.
        assert!((h.start - Point::new(10.0, 50.0)).hypot() < 1e-6, "{:?}", h.start);
        let steps = app.engine.history_labels().0.len();
        app.finish_path_handle(PathHandle::Slide, h.start, h.start + Vec2::new(20.0, 3.0));
        let tp = on_path(&app, t).expect("on");
        assert!((tp.offset - 20.0).abs() < 1e-6, "{tp:?}");
        assert_eq!(app.engine.history_labels().0.len(), steps + 1);
        // Back near the start: drawn to it.
        let h = app.path_handles().expect("handles");
        let k = 4.0 / f64::from(app.view.zoom);
        app.finish_path_handle(PathHandle::Slide, h.start, h.start - Vec2::new(20.0 - k, 0.0));
        assert!(on_path(&app, t).expect("on").offset.abs() < 1e-9);
        // The green dot: 5 mm further from the stroke.
        let h = app.path_handles().expect("handles");
        app.finish_path_handle(PathHandle::Distance, h.lift, h.lift + Vec2::new(0.0, 5.0));
        assert!((on_path(&app, t).expect("on").distance - 5.0).abs() < 1e-6);
        // The double arrow: the other side.
        app.flip_text_side();
        assert!(on_path(&app, t).expect("on").below);
        // The blue dot: the end 9 mm further, shared by the 3 gaps.
        let h = app.path_handles().expect("handles");
        let size = 12.0 * graphdraw_text::PT_MM;
        app.finish_path_handle(PathHandle::Spacing, h.spacing, h.spacing + Vec2::new(9.0, 0.0));
        let pct = app.letter_spacing().expect("spacing");
        let want = app.text_size_pt / 12.0;
        assert!((pct - 100.0 * 9.0 / (3.0 * size * want)).abs() < 0.5, "{pct}");
    }

    #[test]
    fn a_missed_pick_asks_to_try_again_and_a_drop_on_a_curve_fits_there() {
        let mut app = App::headless();
        let l = line_at(&mut app, 50.0);
        let t = text(&mut app, "Drop");
        assert!(app.can_guide_text(l) && !app.can_guide_text(t));
        app.text_path_pick_missed();
        assert!(matches!(app.dialog, crate::ui::dialogs::Dialog::RetryTextPath));
        // Dragged with the right button onto the line, dropped at x = 60.
        app.selection = vec![t];
        let menu = crate::drop_menu::DropMenu {
            screen: app.view.to_screen(Point::new(60.0, 52.0)),
            offset: Vec2::ZERO,
            target: Some(l),
        };
        assert!(app.drop_fits_to_path(&menu));
        app.drop_action(menu, crate::drop_menu::DropAction::FitToPath);
        let tp = on_path(&app, t).expect("on the line");
        assert_eq!(tp.guide, Some(l));
        assert!((tp.offset - 50.0).abs() < 0.01, "{tp:?}");
        // One undo step takes it off.
        app.undo();
        assert!(on_path(&app, t).is_none());
    }

    #[test]
    fn letter_spacing_unjustifies_and_is_one_step() {
        let mut app = App::headless();
        let (t, _) = fitted(&mut app);
        app.selection = vec![t];
        app.set_letter_spacing(20.0, false);
        assert_eq!(app.letter_spacing(), Some(20.0));
        app.set_letter_spacing(30.0, true);
        assert_eq!(app.letter_spacing(), Some(30.0));
        app.undo();
        assert_eq!(app.letter_spacing(), Some(0.0));
        if !fonts() {
            return;
        }
        app.edit_fitted_text(|_, a| *a = TextAlign::Justify);
        app.set_letter_spacing(-5.0, false);
        let ft = app.fitted_text().expect("fitted");
        assert_eq!(ft.align, TextAlign::Left);
        assert!(ft.on_path.offset.abs() < 1e-6);
    }

    #[test]
    fn copies_follow_their_guide() {
        let mut app = App::headless();
        let (t, l) = fitted(&mut app);
        app.selection = vec![l, t];
        app.duplicate();
        let [nl, nt] = app.selection.as_slice() else {
            panic!("two copies");
        };
        assert_eq!(on_path(&app, *nt).and_then(|p| p.guide), Some(*nl));
        let (a, b) = (on_path(&app, t).expect("t"), on_path(&app, *nt).expect("copy"));
        // Moved with its guide, the copy keeps its own path: it did not go
        // back onto the original's guide.
        assert!(graphdraw_core::text_path::same_path(&a.path, &b.path));
    }
}
