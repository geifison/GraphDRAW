//! Text on a path: the geometry the layout and the editor share (a track
//! along the path by arc length with its sharp corners and curvature),
//! the live link between a fitted text and its guide curve, and the
//! ghost path a straightened text keeps (see docs/behavior/text.md).

use crate::document::{Baseline, Document, Shape, ShapeKind, TextAlign, TextOnPath};
use crate::geometry::{Affine, BezPath, Point, Vec2};
use crate::id::ShapeId;

/// The path text is fitted to, in page space: its first subpath, and for a
/// closed one turned to run clockwise (so the letters stand on its
/// outside) and started at its highest point. Returns the path and whether
/// it is closed.
pub fn path_for_text(path: &BezPath) -> (BezPath, bool) {
    use kurbo::{ParamCurve, ParamCurveExtrema, PathSeg};
    let mut first = BezPath::new();
    let mut closed = false;
    for el in path.elements() {
        match el {
            crate::geometry::PathEl::MoveTo(_) if !first.elements().is_empty() => break,
            crate::geometry::PathEl::ClosePath => {
                closed = true;
                break;
            }
            e => first.push(*e),
        }
    }
    let mut segs: Vec<PathSeg> = first.segments().collect();
    if let (Some(a), Some(b)) = (segs.first(), segs.last()) {
        if (a.start() - b.end()).hypot() < 1e-6 && segs.len() > 1 {
            closed = true;
        }
    }
    if !closed || segs.is_empty() {
        return (first, false);
    }
    if let (Some(a), Some(b)) = (segs.first(), segs.last()) {
        if (a.start() - b.end()).hypot() >= 1e-6 {
            segs.push(PathSeg::Line(crate::geometry::Line::new(
                b.end(),
                a.start(),
            )));
        }
    }
    // Shoelace over the flattened outline: positive runs anticlockwise
    // (y up), whose left side, where letters stand, is the inside.
    let mut area = 0.0;
    let mut prev: Option<Point> = None;
    let mut start: Option<Point> = None;
    crate::geometry::flatten(&first, 0.1, &mut |el| match el {
        crate::geometry::PathEl::MoveTo(p) => {
            start = Some(p);
            prev = Some(p);
        }
        crate::geometry::PathEl::LineTo(p) => {
            if let Some(q) = prev {
                area += q.x * p.y - p.x * q.y;
            }
            prev = Some(p);
        }
        _ => {}
    });
    if let (Some(q), Some(p)) = (prev, start) {
        area += q.x * p.y - p.x * q.y;
    }
    if area > 0.0 {
        segs = segs.iter().rev().map(|s| s.reverse()).collect();
    }
    // The highest point: the segment and parameter where y is largest.
    let mut best = (0usize, 0.0f64, f64::NEG_INFINITY);
    for (i, seg) in segs.iter().enumerate() {
        let mut ts = vec![0.0, 1.0];
        ts.extend(seg.extrema());
        for t in ts {
            let y = seg.eval(t).y;
            if y > best.2 + 1e-9 {
                best = (i, t, y);
            }
        }
    }
    let (k, t, _) = best;
    let mut order: Vec<PathSeg> = Vec::new();
    if t < 1.0 - 1e-9 {
        order.push(segs[k].subsegment(t..1.0));
    }
    order.extend(segs[k + 1..].iter().copied());
    order.extend(segs[..k].iter().copied());
    if t > 1e-9 {
        order.push(segs[k].subsegment(0.0..t));
    }
    let mut out = BezPath::new();
    if let Some(s0) = order.first() {
        out.move_to(s0.start());
    }
    for seg in order {
        match seg {
            PathSeg::Line(l) => out.line_to(l.p1),
            PathSeg::Quad(q) => out.quad_to(q.p1, q.p2),
            PathSeg::Cubic(c) => out.curve_to(c.p1, c.p2, c.p3),
        }
    }
    out.close_path();
    (out, true)
}

/// Sharpest turn, in radians, at one point of the flattened path that
/// still counts as smooth; more is a corner.
const CORNER_TURN: f64 = 0.45;

/// The path of a text on a path as the letters follow it, by arc length:
/// the first subpath, followed backwards when mirrored horizontally, with
/// the lengths where it turns sharply (corner nodes).
#[derive(Debug, Clone)]
pub struct Track {
    base: Baseline,
    /// Arc lengths of the sharp corners, in order.
    pub corners: Vec<f64>,
}

impl Track {
    /// The track of `tp` (`None` for a path with no length).
    pub fn new(tp: &TextOnPath) -> Option<Track> {
        let base = Baseline::new(&tp.path, 0.0, tp.mirrors().0);
        if base.points().len() < 2 || base.length() <= 0.0 {
            return None;
        }
        let pts = base.points();
        let lens = base.lengths();
        let mut corners = Vec::new();
        for (i, w) in pts.windows(3).enumerate() {
            let (u, v) = (w[1] - w[0], w[2] - w[1]);
            let turn = u.cross(v).atan2(u.dot(v));
            if turn.abs() > CORNER_TURN {
                if let Some(l) = lens.get(i + 1) {
                    corners.push(*l);
                }
            }
        }
        Some(Track { base, corners })
    }

    pub fn total(&self) -> f64 {
        self.base.length()
    }

    pub fn closed(&self) -> bool {
        self.base.is_closed()
    }

    /// A length brought round into a closed path (it goes on past its
    /// start); unchanged on an open one.
    pub fn wrap(&self, s: f64) -> f64 {
        let t = self.total();
        if self.closed() && t > 0.0 {
            s.rem_euclid(t)
        } else {
            s
        }
    }

    /// The point at arc length `s` and the unit direction there. Past the
    /// ends of an open path the track goes straight on along the end's
    /// direction.
    pub fn at(&self, s: f64) -> (Point, Vec2) {
        let s = if s.is_finite() { self.wrap(s) } else { 0.0 };
        let total = self.total();
        let pts = self.base.points();
        let lens = self.base.lengths();
        let n = pts.len();
        let seg = if s <= 0.0 {
            0
        } else if s >= total {
            n - 2
        } else {
            lens.partition_point(|l| *l <= s)
                .saturating_sub(1)
                .min(n - 2)
        };
        let (a, b) = (pts[seg], pts[seg + 1]);
        let d = b - a;
        let len = d.hypot().max(1e-12);
        let dir = d / len;
        (a + dir * (s - lens[seg]), dir)
    }

    /// Signed curvature at `s` (positive turning left), averaged over
    /// `±h`.
    pub fn curvature(&self, s: f64, h: f64) -> f64 {
        let h = h.max(1e-6);
        let (_, a) = self.at(s - h);
        let (_, b) = self.at(s + h);
        let turn = a.cross(b).atan2(a.dot(b));
        turn / (2.0 * h)
    }

    /// The arc length of the point of the track nearest `p`.
    pub fn project(&self, p: Point) -> f64 {
        self.base.nearest(p).map_or(0.0, |(s, _)| s)
    }

    /// The arc length of the highest (`top`) or lowest point.
    pub fn extreme(&self, top: bool) -> f64 {
        let pts = self.base.points();
        let mut best = 0;
        for (i, p) in pts.iter().enumerate() {
            let b = pts[best];
            if (top && p.y > b.y) || (!top && p.y < b.y) {
                best = i;
            }
        }
        self.base.lengths().get(best).copied().unwrap_or(0.0)
    }

    /// The first corner strictly inside `(a, b)` (round the start of a
    /// closed path too).
    pub fn corner_in(&self, a: f64, b: f64) -> Option<f64> {
        let (a, b) = (self.wrap(a), self.wrap(b));
        self.corners.iter().copied().find(|c| {
            (a < b && *c > a + 1e-6 && *c < b - 1e-6) || (a > b && (*c > a + 1e-6 || *c < b - 1e-6))
        })
    }
}

/// The offset that puts the start of the text (left or justified), its
/// middle (centred) or its end (right) at arc length `s` of a track
/// `total` long.
pub fn offset_for(align: TextAlign, s: f64, total: f64) -> f64 {
    match align {
        TextAlign::Center => s - total / 2.0,
        TextAlign::Right => s - total,
        _ => s,
    }
}

/// The shape kinds that can guide a text: anything with an outline that is
/// not a text, a group, a clip frame or a bitmap.
pub fn can_guide(kind: &ShapeKind) -> bool {
    !matches!(
        kind,
        ShapeKind::Text { .. }
            | ShapeKind::Group { .. }
            | ShapeKind::ClipFrame { .. }
            | ShapeKind::Bitmap { .. }
    )
}

/// From the local space of the text `text` (its origin included) to the
/// page.
pub fn text_to_page(doc: &Document, text: &Shape) -> Affine {
    let origin = match &text.kind {
        ShapeKind::Text { origin, .. } => origin.to_vec2(),
        _ => Vec2::ZERO,
    };
    doc.parent_transform(text.id).unwrap_or(Affine::IDENTITY)
        * text.transform
        * Affine::translate(origin)
}

/// The path of the curve `guide` as the text `text` follows it: in page
/// space turned and started as [`path_for_text`] does, then in the text's
/// local space. `None` when there is no such curve or it cannot guide.
pub fn guide_path(doc: &Document, guide: ShapeId, text: &Shape) -> Option<BezPath> {
    if guide == text.id {
        return None;
    }
    let g = doc.find_shape(guide)?;
    if !can_guide(&g.kind) {
        return None;
    }
    let page = doc.parent_transform(guide)? * g.page_path();
    if page.elements().is_empty() {
        return None;
    }
    let (path, _) = path_for_text(&page);
    let m = text_to_page(doc, text);
    let det = m.determinant();
    if !det.is_finite() || det.abs() < 1e-12 {
        return None;
    }
    Some(m.inverse() * path)
}

/// Two paths with the same elements, their points within a ten-thousandth
/// of a micrometre.
pub fn same_path(a: &BezPath, b: &BezPath) -> bool {
    let (ea, eb) = (a.elements(), b.elements());
    ea.len() == eb.len()
        && ea.iter().zip(eb).all(|(x, y)| {
            std::mem::discriminant(x) == std::mem::discriminant(y)
                && match (x.end_point(), y.end_point()) {
                    (Some(p), Some(q)) => (p - q).hypot() < 1e-7,
                    _ => true,
                }
        })
}

impl Document {
    /// The live link: every text on a path linked to a guide curve is laid
    /// on that curve again when it changed (or the text moved without
    /// it). Texts whose guide is gone keep the path they have.
    pub fn refresh_text_paths(&mut self) {
        fn collect(s: &Shape, out: &mut Vec<(ShapeId, ShapeId)>) {
            match &s.kind {
                ShapeKind::Text {
                    on_path: Some(tp), ..
                } => {
                    if let Some(g) = tp.guide {
                        out.push((s.id, g));
                    }
                }
                ShapeKind::Group { children } => {
                    for c in children {
                        collect(c, out);
                    }
                }
                ShapeKind::ClipFrame { frame, contents } => {
                    collect(frame, out);
                    for c in contents {
                        collect(c, out);
                    }
                }
                _ => {}
            }
        }
        let mut linked = Vec::new();
        for l in self.all_layers() {
            for s in &l.shapes {
                collect(s, &mut linked);
            }
        }
        let mut updates = Vec::new();
        for (t, g) in linked {
            let Some(text) = self.find_shape(t) else {
                continue;
            };
            let Some(path) = guide_path(self, g, text) else {
                continue;
            };
            if let ShapeKind::Text {
                on_path: Some(tp), ..
            } = &text.kind
            {
                if !same_path(&tp.path, &path) {
                    updates.push((t, path));
                }
            }
        }
        for (t, path) in updates {
            if let Some(ShapeKind::Text {
                on_path: Some(tp), ..
            }) = self.find_shape_mut(t).map(|s| &mut s.kind)
            {
                tp.path = path;
            }
        }
    }
}

/// Copies just made of shapes (`pairs` maps each original id to its
/// copy's): a copied text on a path follows the copy of its guide when
/// the guide was copied with it, and is unlinked otherwise (so it stays
/// where it is put instead of going back onto the original's guide).
pub fn relink_copies(shapes: &mut [Shape], pairs: &[(ShapeId, ShapeId)]) {
    fn relink(tp: &mut TextOnPath, pairs: &[(ShapeId, ShapeId)]) {
        if let Some(g) = tp.guide {
            tp.guide = pairs
                .iter()
                .find(|(old, _)| *old == g)
                .map(|(_, new)| *new);
        }
    }
    for s in shapes.iter_mut() {
        if let Some(m) = &mut s.text_path_memory {
            relink(m, pairs);
        }
        match &mut s.kind {
            ShapeKind::Text {
                on_path: Some(tp), ..
            } => relink(tp, pairs),
            ShapeKind::Group { children } => relink_copies(children, pairs),
            ShapeKind::ClipFrame { contents, .. } => relink_copies(contents, pairs),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::Command;
    use crate::document::{ParagraphStyle, TextSpan};
    use crate::geometry::Size;
    use crate::Engine;

    fn line(x0: f64, x1: f64, y: f64) -> BezPath {
        let mut p = BezPath::new();
        p.move_to((x0, y));
        p.line_to((x1, y));
        p
    }

    fn setup() -> (Engine, ShapeId, ShapeId) {
        let mut engine = Engine::new(Document::new("t", Size::new(200.0, 200.0)));
        let layer = engine.document().pages[0].layers[0].id;
        let gid = engine.new_shape_id();
        let guide = Shape::new(
            gid,
            ShapeKind::Path {
                path: line(10.0, 110.0, 50.0),
                closed: false,
            },
        );
        engine
            .run(&Command::AddShape {
                layer,
                shape: guide,
            })
            .expect("guide");
        let t = engine.new_shape_id();
        let text = Shape::new(
            t,
            ShapeKind::Text {
                spans: vec![TextSpan::new("abc", "DejaVu Sans", 12.0)],
                origin: Point::ZERO,
                frame: None,
                align: TextAlign::Left,
                para: ParagraphStyle::default(),
                on_path: Some(TextOnPath::fitted(line(10.0, 110.0, 50.0), Some(gid))),
            },
        );
        engine
            .run(&Command::AddShape { layer, shape: text })
            .expect("text");
        (engine, gid, t)
    }

    fn path_of(engine: &Engine, t: ShapeId) -> BezPath {
        match &engine.document().find_shape(t).map(|s| s.kind.clone()) {
            Some(ShapeKind::Text {
                on_path: Some(tp), ..
            }) => tp.path.clone(),
            _ => BezPath::new(),
        }
    }

    #[test]
    fn moving_the_guide_lays_the_text_on_it_again_and_undo_takes_both_back() {
        let (mut engine, g, t) = setup();
        engine
            .run(&Command::TransformShapes {
                shapes: vec![g],
                transform: Affine::translate((0.0, 30.0)),
            })
            .expect("move");
        let p = path_of(&engine, t);
        assert!(same_path(&p, &line(10.0, 110.0, 80.0)), "{p:?}");
        // Moving the text alone: it goes back onto its guide.
        engine
            .run(&Command::TransformShapes {
                shapes: vec![t],
                transform: Affine::translate((5.0, 0.0)),
            })
            .expect("move text");
        assert!(same_path(&path_of(&engine, t), &line(5.0, 105.0, 80.0)));
        engine.undo().expect("undo");
        engine.undo().expect("undo");
        assert!(same_path(&path_of(&engine, t), &line(10.0, 110.0, 50.0)));
    }

    #[test]
    fn a_deleted_guide_leaves_the_text_where_it_is() {
        let (mut engine, g, t) = setup();
        engine
            .run(&Command::DeleteShapes { shapes: vec![g] })
            .expect("delete");
        assert!(same_path(&path_of(&engine, t), &line(10.0, 110.0, 50.0)));
    }

    #[test]
    fn copies_follow_a_copied_guide_or_are_unlinked() {
        let (engine, g, t) = setup();
        let Some(text) = engine.document().find_shape(t).cloned() else {
            panic!("text");
        };
        let (ng, nt) = (ShapeId(900), ShapeId(901));
        let guide_of = |s: &Shape| match &s.kind {
            ShapeKind::Text {
                on_path: Some(tp), ..
            } => tp.guide,
            _ => None,
        };
        let mut both = vec![text.clone()];
        relink_copies(&mut both, &[(g, ng), (t, nt)]);
        assert_eq!(guide_of(&both[0]), Some(ng));
        let mut alone = vec![text];
        relink_copies(&mut alone, &[(t, nt)]);
        assert_eq!(guide_of(&alone[0]), None);
    }

    #[test]
    fn the_track_finds_corners_goes_on_past_open_ends_and_wraps_closed_ones() {
        let mut l = BezPath::new();
        l.move_to((0.0, 0.0));
        l.line_to((100.0, 0.0));
        l.line_to((100.0, 100.0));
        let tr = Track::new(&TextOnPath::fitted(l, None)).expect("track");
        assert_eq!(tr.corners.len(), 1);
        assert!((tr.corners[0] - 100.0).abs() < 1e-6);
        assert_eq!(tr.corner_in(90.0, 110.0), Some(tr.corners[0]));
        assert_eq!(tr.corner_in(10.0, 20.0), None);
        // Past the end it goes straight on.
        let (p, _) = tr.at(210.0);
        assert!((p.y - 110.0).abs() < 1e-6, "{p:?}");
        // Turning left at the corner.
        assert!(tr.curvature(100.0, 1.0) > 0.0);
        assert!((tr.extreme(true) - 200.0).abs() < 1e-6);
        let mut sq = BezPath::new();
        sq.move_to((0.0, 0.0));
        sq.line_to((50.0, 0.0));
        sq.line_to((50.0, 50.0));
        sq.line_to((0.0, 50.0));
        sq.close_path();
        let tr = Track::new(&TextOnPath::fitted(sq, None)).expect("track");
        assert!(tr.closed());
        assert!((tr.wrap(210.0) - 10.0).abs() < 1e-6);
        assert!((offset_for(TextAlign::Center, 60.0, 200.0) + 40.0).abs() < 1e-9);
        assert!((offset_for(TextAlign::Right, 60.0, 200.0) + 140.0).abs() < 1e-9);
    }

    #[test]
    fn old_files_without_the_new_fields_still_open() {
        let tp = TextOnPath::fitted(line(0.0, 10.0, 0.0), Some(ShapeId(7)));
        let mut v = serde_json::to_value(&tp).expect("json");
        if let Some(o) = v.as_object_mut() {
            for k in ["guide", "below", "corners", "relax"] {
                o.remove(k);
            }
        }
        let back: TextOnPath = serde_json::from_value(v).expect("read");
        assert_eq!(back.guide, None);
        assert_eq!(back.corners, crate::document::PathTextCorners::Free);
        assert!(!back.relax && !back.below);
        // A round trip keeps them.
        let s = serde_json::to_string(&tp).expect("json");
        let again: TextOnPath = serde_json::from_str(&s).expect("read");
        assert_eq!(again, tp);
    }
}
