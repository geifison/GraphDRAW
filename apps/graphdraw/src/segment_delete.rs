//! Segment Delete: removes the stretch of an outline between the nearest
//! places it crosses another outline (or itself), its virtual segment;
//! where nothing crosses, between its open ends, or a whole closed
//! subpath. A click removes the virtual segment under the pointer; a drag
//! removes every virtual segment lying wholly inside the rectangle.
//!
//! Crossings are found against the outlines of every visible object on
//! the page's unlocked layers (flattened to 0.02 mm), the object's other
//! subpaths and its own segments that are not next to the one cut. The
//! object becomes a curve; a closed subpath with a stretch removed opens;
//! an object with nothing left is deleted. One undo step.

use crate::app::App;
use tracedraw_core::{
    document::{Shape, ShapeKind},
    geometry::{BezPath, Line, ParamCurve, ParamCurveNearest, PathEl, PathSeg, Point, Rect},
    Command, ShapeId,
};

/// One subpath as segments.
#[derive(Debug, Clone)]
pub struct Sub {
    pub segs: Vec<PathSeg>,
    pub closed: bool,
}

/// The subpaths of `path`; one that ends where it started counts as
/// closed.
pub fn subs_of(path: &BezPath) -> Vec<Sub> {
    tracedraw_core::effects::subpaths(path)
        .into_iter()
        .filter_map(|(sub, closed)| {
            let mut segs: Vec<PathSeg> = sub.segments().collect();
            if segs.is_empty() {
                return None;
            }
            // `segments()` adds the closing line of a closed subpath; one
            // with no length is dropped.
            if segs
                .last()
                .is_some_and(|s| (s.end() - s.start()).hypot() < 1e-9 && segs.len() > 1)
            {
                segs.pop();
            }
            let ends_at_start =
                segs.len() > 1 && (segs[segs.len() - 1].end() - segs[0].start()).hypot() < 1e-6;
            Some(Sub {
                segs,
                closed: closed || ends_at_start,
            })
        })
        .collect()
}

/// `path` flattened into lines (a closed subpath's closing line too).
fn lines_of(path: &BezPath) -> Vec<Line> {
    let mut out = Vec::new();
    let mut start = Point::ZERO;
    let mut last = Point::ZERO;
    tracedraw_core::geometry::flatten(path, 0.02, &mut |el| match el {
        PathEl::MoveTo(p) => {
            start = p;
            last = p;
        }
        PathEl::LineTo(p) => {
            out.push(Line::new(last, p));
            last = p;
        }
        PathEl::ClosePath => {
            if (last - start).hypot() > 1e-9 {
                out.push(Line::new(last, start));
            }
            last = start;
        }
        _ => {}
    });
    out
}

/// A segment as a path of its own.
fn seg_path(s: &PathSeg) -> BezPath {
    let mut p = BezPath::new();
    p.move_to(s.start());
    match *s {
        PathSeg::Line(l) => p.line_to(l.p1),
        PathSeg::Quad(q) => p.quad_to(q.p1, q.p2),
        PathSeg::Cubic(c) => p.curve_to(c.p1, c.p2, c.p3),
    }
    p
}

/// Where subpath `which` of `subs` crosses `cutters` and the rest of its
/// own object: parameters `u` = segment index + t, sorted, in `0..n`
/// (`n` the number of segments).
pub fn crossings(subs: &[Sub], which: usize, cutters: &[Line]) -> Vec<f64> {
    let Some(sub) = subs.get(which) else {
        return Vec::new();
    };
    let n = sub.segs.len();
    // The object's other subpaths cut too.
    let mut own: Vec<Line> = Vec::new();
    for (k, other) in subs.iter().enumerate() {
        if k != which {
            for s in &other.segs {
                own.extend(lines_of(&seg_path(s)));
            }
        }
    }
    let mut out = Vec::new();
    for (i, seg) in sub.segs.iter().enumerate() {
        let hit = |line: Line, out: &mut Vec<f64>| {
            for h in seg.intersect_line(line) {
                if (-1e-9..=1.0 + 1e-9).contains(&h.line_t) {
                    out.push(i as f64 + h.segment_t.clamp(0.0, 1.0));
                }
            }
        };
        for l in cutters.iter().chain(own.iter()) {
            hit(*l, &mut out);
        }
        // Its own segments that are not next to it.
        for (j, other) in sub.segs.iter().enumerate() {
            let next = (j + 1) % n == i || (i + 1) % n == j;
            if j == i || (next && (sub.closed || j.abs_diff(i) == 1)) {
                continue;
            }
            for l in lines_of(&seg_path(other)) {
                hit(l, &mut out);
            }
        }
    }
    out.retain(|u| u.is_finite());
    out.sort_by(f64::total_cmp);
    out.dedup_by(|a, b| (*a - *b).abs() < 1e-7);
    // On a closed subpath the end is the start.
    if sub.closed {
        for u in &mut out {
            if *u >= n as f64 - 1e-9 {
                *u = 0.0;
            }
        }
        out.sort_by(f64::total_cmp);
        out.dedup_by(|a, b| (*a - *b).abs() < 1e-7);
    }
    out
}

/// The stretch `a..b` of an open run of segments as a path.
fn stretch(segs: &[PathSeg], a: f64, b: f64, into: &mut BezPath) {
    let n = segs.len();
    let mut started = !into.elements().is_empty();
    let first = (a.floor().max(0.0) as usize).min(n.saturating_sub(1));
    for (i, seg) in segs.iter().enumerate().skip(first) {
        let (t0, t1) = ((a - i as f64).max(0.0), (b - i as f64).min(1.0));
        if t0 >= 1.0 {
            continue;
        }
        if t1 <= 0.0 {
            break;
        }
        if t1 - t0 < 1e-9 {
            continue;
        }
        let part = seg.subsegment(t0..t1);
        if !started {
            into.move_to(part.start());
            started = true;
        }
        match part {
            PathSeg::Line(l) => into.line_to(l.p1),
            PathSeg::Quad(q) => into.quad_to(q.p1, q.p2),
            PathSeg::Cubic(c) => into.curve_to(c.p1, c.p2, c.p3),
        }
    }
}

/// What is left of `sub` with the stretches `cut` removed (sorted,
/// in `0..n`; on a closed subpath a stretch with `a > b` wraps past the
/// start). `None` when the subpath is kept whole.
pub fn without(sub: &Sub, cut: &[(f64, f64)]) -> Option<Vec<BezPath>> {
    if cut.is_empty() {
        return None;
    }
    let n = sub.segs.len() as f64;
    // Removed stretches as plain ranges of 0..n.
    let mut removed: Vec<(f64, f64)> = Vec::new();
    for &(a, b) in cut {
        if a <= b {
            removed.push((a, b));
        } else {
            removed.push((a, n));
            removed.push((0.0, b));
        }
    }
    removed.sort_by(|x, y| x.0.total_cmp(&y.0));
    // What stays.
    let mut keep: Vec<(f64, f64)> = Vec::new();
    let mut at = 0.0;
    for (a, b) in removed {
        if a > at + 1e-9 {
            keep.push((at, a));
        }
        at = at.max(b);
    }
    if at < n - 1e-9 {
        keep.push((at, n));
    }
    // A closed subpath: the stretch over the start is one piece.
    if sub.closed && keep.len() > 1 {
        let first = keep[0];
        let last = keep[keep.len() - 1];
        if first.0 <= 1e-9 && last.1 >= n - 1e-9 {
            keep.pop();
            keep.remove(0);
            let mut p = BezPath::new();
            stretch(&sub.segs, last.0, n, &mut p);
            stretch(&sub.segs, 0.0, first.1, &mut p);
            let mut pieces = vec![p];
            for (a, b) in keep {
                let mut q = BezPath::new();
                stretch(&sub.segs, a, b, &mut q);
                pieces.push(q);
            }
            pieces.retain(|p| p.elements().len() > 1);
            return Some(pieces);
        }
    }
    let mut pieces = Vec::new();
    for (a, b) in keep {
        let mut q = BezPath::new();
        stretch(&sub.segs, a, b, &mut q);
        pieces.push(q);
    }
    pieces.retain(|p| p.elements().len() > 1);
    Some(pieces)
}

/// The virtual segment of `sub` holding parameter `u`, given its
/// crossings: `(a, b)` (wrapping when `a > b`); the whole subpath when a
/// closed one has fewer than two crossings.
pub fn around(sub: &Sub, bounds: &[f64], u: f64) -> (f64, f64) {
    let n = sub.segs.len() as f64;
    if sub.closed {
        if bounds.len() < 2 {
            return (0.0, n);
        }
        let lo = bounds
            .iter()
            .rev()
            .find(|b| **b <= u)
            .or(bounds.last())
            .copied()
            .unwrap_or(0.0);
        let hi = bounds
            .iter()
            .find(|b| **b > u)
            .or(bounds.first())
            .copied()
            .unwrap_or(n);
        (lo, hi)
    } else {
        let lo = bounds
            .iter()
            .rev()
            .find(|b| **b <= u)
            .copied()
            .unwrap_or(0.0);
        let hi = bounds.iter().find(|b| **b > u).copied().unwrap_or(n);
        (lo.max(0.0), hi.min(n))
    }
}

/// Every virtual segment of `sub`: consecutive crossings (and the open
/// ends).
pub fn pieces(sub: &Sub, bounds: &[f64]) -> Vec<(f64, f64)> {
    let n = sub.segs.len() as f64;
    if sub.closed {
        if bounds.len() < 2 {
            return vec![(0.0, n)];
        }
        let mut out: Vec<(f64, f64)> = bounds.windows(2).map(|w| (w[0], w[1])).collect();
        out.push((bounds[bounds.len() - 1], bounds[0]));
        out
    } else {
        let mut b = vec![0.0];
        b.extend(
            bounds
                .iter()
                .copied()
                .filter(|u| *u > 1e-9 && *u < n - 1e-9),
        );
        b.push(n);
        b.windows(2).map(|w| (w[0], w[1])).collect()
    }
}

/// The point at parameter `u` of `sub`.
fn point_at(sub: &Sub, u: f64) -> Point {
    let n = sub.segs.len();
    let u = if sub.closed {
        u.rem_euclid(n as f64)
    } else {
        u.clamp(0.0, n as f64)
    };
    let i = (u.floor() as usize).min(n - 1);
    sub.segs[i].eval(u - i as f64)
}

fn cuttable(s: &Shape) -> bool {
    matches!(
        s.kind,
        ShapeKind::Path { .. }
            | ShapeKind::Rect { .. }
            | ShapeKind::Ellipse { .. }
            | ShapeKind::Polygon { .. }
    )
}

impl App {
    /// The visible objects of the page's unlocked layers.
    fn segment_targets(&self) -> Vec<Shape> {
        self.doc()
            .page(self.page)
            .map(|pg| {
                pg.layers
                    .iter()
                    .filter(|l| l.visible && !l.locked)
                    .flat_map(|l| l.shapes.iter().filter(|s| s.visible && !s.locked).cloned())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The other objects' outlines, flattened, page space.
    fn cutters_except(&self, all: &[Shape], id: ShapeId) -> Vec<Line> {
        let mut out = Vec::new();
        for s in all {
            if s.id != id {
                out.extend(lines_of(&s.page_path()));
            }
        }
        out
    }

    /// The command that leaves `s` (page path `subs`) with the stretches
    /// `cut[i]` removed from subpath `i`.
    fn segment_delete_command(
        &self,
        s: &Shape,
        subs: &[Sub],
        cut: &[Vec<(f64, f64)>],
    ) -> Option<Command> {
        let mut path = BezPath::new();
        let mut any_closed = false;
        let mut changed = false;
        for (sub, c) in subs.iter().zip(cut) {
            match without(sub, c) {
                None => {
                    for (k, seg) in sub.segs.iter().enumerate() {
                        if k == 0 {
                            path.move_to(seg.start());
                        }
                        match *seg {
                            PathSeg::Line(l) => path.line_to(l.p1),
                            PathSeg::Quad(q) => path.quad_to(q.p1, q.p2),
                            PathSeg::Cubic(cb) => path.curve_to(cb.p1, cb.p2, cb.p3),
                        }
                    }
                    if sub.closed {
                        path.close_path();
                        any_closed = true;
                    }
                }
                Some(pieces) => {
                    changed = true;
                    for p in pieces {
                        path.extend(p.elements().iter().copied());
                    }
                }
            }
        }
        if !changed {
            return None;
        }
        if path.elements().is_empty() {
            return Some(Command::DeleteShapes { shapes: vec![s.id] });
        }
        if s.transform.determinant().abs() < 1e-12 {
            return None;
        }
        Some(Command::SetShapeKind {
            shape: s.id,
            kind: ShapeKind::Path {
                path: s.transform.inverse() * path,
                closed: any_closed,
            },
        })
    }

    /// Remove the virtual segment under `p` (page space). False when no
    /// outline is near.
    pub fn delete_virtual_segment_at(&mut self, p: Point) -> bool {
        let tol = 5.0 / self.view.zoom.max(1e-6) as f64;
        let all = self.segment_targets();
        // The nearest outline within reach (the topmost of equals).
        let mut found: Option<(Shape, Vec<Sub>, usize, f64, f64)> = None;
        for s in all.iter().rev().filter(|s| cuttable(s)) {
            if !s.bounds().inflate(tol, tol).contains(p) {
                continue;
            }
            let subs = subs_of(&s.page_path());
            let mut best: Option<(usize, f64, f64)> = None;
            for (k, sub) in subs.iter().enumerate() {
                for (i, seg) in sub.segs.iter().enumerate() {
                    let n = seg.nearest(p, 1e-6);
                    let d = n.distance_sq.sqrt();
                    if d <= tol && best.is_none_or(|b| d < b.2) {
                        best = Some((k, i as f64 + n.t, d));
                    }
                }
            }
            if let Some((k, u, d)) = best {
                if found.as_ref().is_none_or(|f| d < f.4 - 1e-9) {
                    found = Some((s.clone(), subs, k, u, d));
                }
            }
        }
        let Some((s, subs, k, u, _)) = found else {
            return false;
        };
        let cutters = self.cutters_except(&all, s.id);
        let bounds = crossings(&subs, k, &cutters);
        let mut cut = vec![Vec::new(); subs.len()];
        cut[k].push(around(&subs[k], &bounds, u));
        if let Some(cmd) = self.segment_delete_command(&s, &subs, &cut) {
            if let Err(e) = self.engine.run_batch("Delete Segment", &[cmd]) {
                self.status = e.to_string();
            }
            self.selection
                .retain(|id| self.engine.document().find_shape(*id).is_some());
        }
        true
    }

    /// Remove every virtual segment lying wholly inside `rect` (page
    /// space).
    pub fn delete_virtual_segments_in(&mut self, rect: Rect) {
        if rect.width() < 1e-6 || rect.height() < 1e-6 {
            return;
        }
        let all = self.segment_targets();
        let mut cmds = Vec::new();
        for s in all.iter().filter(|s| cuttable(s)) {
            if s.bounds().intersect(rect).area() <= 0.0 && !rect.contains(s.bounds().center()) {
                continue;
            }
            let subs = subs_of(&s.page_path());
            let cutters = self.cutters_except(&all, s.id);
            let mut cut = vec![Vec::new(); subs.len()];
            for (k, sub) in subs.iter().enumerate() {
                let bounds = crossings(&subs, k, &cutters);
                let n = sub.segs.len() as f64;
                for (a, b) in pieces(sub, &bounds) {
                    let len = if a <= b { b - a } else { n - a + b };
                    let inside =
                        (0..=8).all(|j| rect.contains(point_at(sub, a + len * j as f64 / 8.0)));
                    if inside {
                        cut[k].push((a, b));
                    }
                }
            }
            if let Some(cmd) = self.segment_delete_command(s, &subs, &cut) {
                cmds.push(cmd);
            }
        }
        if cmds.is_empty() {
            return;
        }
        if let Err(e) = self.engine.run_batch("Delete Segment", &cmds) {
            self.status = e.to_string();
        }
        self.selection
            .retain(|id| self.engine.document().find_shape(*id).is_some());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracedraw_core::geometry::Shape as _;

    fn line(app: &mut App, a: (f64, f64), b: (f64, f64)) -> ShapeId {
        let mut p = BezPath::new();
        p.move_to(a);
        p.line_to(b);
        app.new_shape(ShapeKind::Path {
            path: p,
            closed: false,
        })
        .expect("line")
    }

    fn path_of(app: &App, id: ShapeId) -> BezPath {
        app.doc().find_shape(id).expect("object").page_path()
    }

    #[test]
    fn a_click_removes_the_stretch_between_crossings() {
        let mut app = App::headless();
        // A horizontal line crossed by two vertical ones.
        let h = line(&mut app, (0.0, 0.0), (30.0, 0.0));
        line(&mut app, (10.0, -5.0), (10.0, 5.0));
        line(&mut app, (20.0, -5.0), (20.0, 5.0));
        assert!(app.delete_virtual_segment_at(Point::new(15.0, 0.0)));
        let p = path_of(&app, h);
        let moves = p
            .elements()
            .iter()
            .filter(|e| matches!(e, PathEl::MoveTo(_)))
            .count();
        assert_eq!(moves, 2, "{p:?}");
        let b = p.bounding_box();
        assert!((b.x0 - 0.0).abs() < 1e-6 && (b.x1 - 30.0).abs() < 1e-6);
        // Nothing between 10 and 20 is left.
        assert!(p
            .segments()
            .all(|s| s.eval(0.5).x < 10.0 + 1e-6 || s.eval(0.5).x > 20.0 - 1e-6));
        // An end stretch: from the open end to the first crossing.
        assert!(app.delete_virtual_segment_at(Point::new(25.0, 0.0)));
        let b = path_of(&app, h).bounding_box();
        assert!((b.x1 - 10.0).abs() < 1e-6, "{b:?}");
        // The last stretch: the object goes.
        assert!(app.delete_virtual_segment_at(Point::new(5.0, 0.0)));
        assert!(app.doc().find_shape(h).is_none());
        // Far from any outline: nothing.
        assert!(!app.delete_virtual_segment_at(Point::new(500.0, 500.0)));
    }

    #[test]
    fn closed_outlines_open_where_they_lose_a_stretch() {
        let mut app = App::headless();
        let sq = app
            .new_shape(ShapeKind::Rect {
                rect: Rect::new(0.0, 0.0, 20.0, 20.0),
                radius: 0.0,
                corners: None,
            })
            .expect("square");
        // A line across the square's bottom side at x = 5 and x = 15.
        line(&mut app, (5.0, -5.0), (5.0, 5.0));
        line(&mut app, (15.0, -5.0), (15.0, 5.0));
        assert!(app.delete_virtual_segment_at(Point::new(10.0, 0.0)));
        let s = app.doc().find_shape(sq).expect("square");
        let ShapeKind::Path { closed, .. } = &s.kind else {
            panic!("a curve");
        };
        assert!(!closed);
        let p = s.page_path();
        // One open piece, from (15, 0) round the top to (5, 0).
        let moves = p
            .elements()
            .iter()
            .filter(|e| matches!(e, PathEl::MoveTo(_)))
            .count();
        assert_eq!(moves, 1);
        assert!(p.segments().all(|g| !(g.eval(0.5).y.abs() < 1e-9
            && g.eval(0.5).x > 5.0 + 1e-6
            && g.eval(0.5).x < 15.0 - 1e-6)));
        // An ellipse with nothing crossing it goes whole.
        let e = app
            .new_shape(ShapeKind::Ellipse {
                rect: Rect::new(100.0, 0.0, 120.0, 10.0),
                arc: None,
            })
            .expect("ellipse");
        assert!(app.delete_virtual_segment_at(Point::new(120.0, 5.0)));
        assert!(app.doc().find_shape(e).is_none());
    }

    #[test]
    fn a_drag_removes_the_stretches_inside() {
        let mut app = App::headless();
        let h = line(&mut app, (0.0, 0.0), (30.0, 0.0));
        line(&mut app, (10.0, -5.0), (10.0, 5.0));
        line(&mut app, (20.0, -5.0), (20.0, 5.0));
        let steps = app.engine.history_labels().0.len();
        // Around the middle stretch and the ends of the verticals' lower
        // halves: the middle stretch goes, the verticals lose their lower
        // halves, nothing else.
        app.delete_virtual_segments_in(Rect::new(9.0, -6.0, 21.0, 1.0));
        assert_eq!(app.engine.history_labels().0.len(), steps + 1);
        let p = path_of(&app, h);
        assert_eq!(
            p.elements()
                .iter()
                .filter(|e| matches!(e, PathEl::MoveTo(_)))
                .count(),
            2
        );
        let verticals: Vec<Rect> = app
            .doc()
            .all_layers()
            .flat_map(|l| &l.shapes)
            .filter(|s| s.id != h)
            .map(|s| s.page_path().bounding_box())
            .collect();
        assert_eq!(verticals.len(), 2);
        assert!(verticals
            .iter()
            .all(|b| (b.y0 - 0.0).abs() < 1e-6 && (b.y1 - 5.0).abs() < 1e-6));
    }

    #[test]
    fn pieces_and_stretches() {
        let sub = Sub {
            segs: vec![
                PathSeg::Line(Line::new((0.0, 0.0), (10.0, 0.0))),
                PathSeg::Line(Line::new((10.0, 0.0), (10.0, 10.0))),
                PathSeg::Line(Line::new((10.0, 10.0), (0.0, 0.0))),
            ],
            closed: true,
        };
        assert_eq!(pieces(&sub, &[]), vec![(0.0, 3.0)]);
        assert_eq!(pieces(&sub, &[0.5, 1.5]), vec![(0.5, 1.5), (1.5, 0.5)]);
        assert_eq!(around(&sub, &[0.5, 1.5], 2.0), (1.5, 0.5));
        assert_eq!(around(&sub, &[0.5, 1.5], 1.0), (0.5, 1.5));
        // The wrapping stretch removed: one piece from 0.5 to 1.5.
        let left = without(&sub, &[(1.5, 0.5)]).expect("changed");
        assert_eq!(left.len(), 1);
        let b = left[0].bounding_box();
        assert!(
            (b.x0 - 5.0).abs() < 1e-9 && (b.y1 - 5.0).abs() < 1e-9,
            "{b:?}"
        );
        // The whole loop removed: nothing left.
        assert!(without(&sub, &[(0.0, 3.0)]).expect("changed").is_empty());
        assert!(without(&sub, &[]).is_none());
    }
}
