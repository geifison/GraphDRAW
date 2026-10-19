//! The Knife tool. A cut, a straight line between two points (extended
//! across the objects) or a freehand stroke, splits the objects it
//! crosses: the selection, or every object on the page's unlocked layers.
//! Closed objects lose a band along the cut (as wide as the bar's gap, a
//! hair when the parts should meet) and each separate piece becomes an
//! object of its own (or they stay one object, combined); open curves are
//! split where the cut crosses them. Pieces keep the fill, outline and
//! opacity. Bitmaps, groups and text are not cut. One undo step.

use crate::app::App;
use serde::{Deserialize, Serialize};
use tracedraw_core::{
    document::{Shape, ShapeKind},
    geometry::{BezPath, Line, ParamCurve, PathSeg, Point, Rect},
    shaping, Command, ShapeId,
};

/// How the cut is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnifeMode {
    #[default]
    TwoPoint,
    Freehand,
}

/// The Knife's bar.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct KnifeSettings {
    pub mode: KnifeMode,
    /// Width taken out along the cut, mm (0: the parts meet).
    pub gap: f64,
    /// The pieces of each object stay one object.
    pub one_object: bool,
}

impl Default for KnifeSettings {
    fn default() -> Self {
        KnifeSettings {
            mode: KnifeMode::TwoPoint,
            gap: 0.0,
            one_object: false,
        }
    }
}

/// The band taken out when the parts should meet, mm.
const HAIR: f64 = 0.01;

/// The cut as a polyline: two points stretched across `reach` (a line), or
/// the stroke as drawn.
fn cut_line(points: &[Point], reach: Rect) -> Vec<Point> {
    match points {
        [a, b] => {
            let d = *b - *a;
            let len = d.hypot();
            if len < 1e-9 {
                return points.to_vec();
            }
            let far = (reach.width() + reach.height()) * 2.0 + (reach.center() - *a).hypot();
            let u = d / len;
            vec![*a - u * far, *b + u * far]
        }
        _ => points.to_vec(),
    }
}

/// Where the open path `path` crosses the polyline `cut`: (segment, t).
pub(crate) fn crossings(path: &BezPath, cut: &[Point]) -> Vec<(usize, f64)> {
    let mut out = Vec::new();
    for (i, seg) in path.segments().enumerate() {
        for w in cut.windows(2) {
            let line = Line::new(w[0], w[1]);
            for hit in seg.intersect_line(line) {
                if (0.0..=1.0).contains(&hit.line_t) {
                    out.push((i, hit.segment_t));
                }
            }
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)));
    out.dedup_by(|a, b| a.0 == b.0 && (a.1 - b.1).abs() < 1e-9);
    out
}

/// `path` (one open subpath) split at `cuts` (segment, t), in order.
pub(crate) fn split_open(path: &BezPath, cuts: &[(usize, f64)]) -> Vec<BezPath> {
    let segs: Vec<PathSeg> = path.segments().collect();
    let mut pieces = Vec::new();
    let mut cur = BezPath::new();
    let push_seg = |cur: &mut BezPath, s: PathSeg| {
        if cur.elements().is_empty() {
            cur.move_to(s.start());
        }
        match s {
            PathSeg::Line(l) => cur.line_to(l.p1),
            PathSeg::Quad(q) => cur.quad_to(q.p1, q.p2),
            PathSeg::Cubic(c) => cur.curve_to(c.p1, c.p2, c.p3),
        }
    };
    for (i, seg) in segs.iter().enumerate() {
        let mut from = 0.0;
        for &(_, t) in cuts.iter().filter(|(k, _)| *k == i) {
            if t > from + 1e-9 {
                push_seg(&mut cur, seg.subsegment(from..t));
            }
            if !cur.elements().is_empty() {
                pieces.push(std::mem::take(&mut cur));
            }
            from = t;
        }
        if from < 1.0 - 1e-9 {
            push_seg(&mut cur, seg.subsegment(from..1.0));
        }
    }
    if !cur.elements().is_empty() {
        pieces.push(cur);
    }
    pieces.retain(|p| p.elements().len() > 1);
    pieces
}

/// The subpaths of a path, each with whether it is closed.
fn subpaths(path: &BezPath) -> Vec<(BezPath, bool)> {
    tracedraw_core::effects::subpaths(path)
}

/// The pieces `s` (in page space) falls into along `cut` (page space),
/// each a page-space path with whether it is closed; None when the cut
/// does not split it.
fn pieces_of(s: &Shape, cut: &[Point], gap: f64) -> Option<Vec<(BezPath, bool)>> {
    let page = s.page_path();
    let closed_kind = match &s.kind {
        ShapeKind::Path { closed, .. } => *closed,
        ShapeKind::Rect { .. } | ShapeKind::Polygon { .. } => true,
        ShapeKind::Ellipse { arc, .. } => arc.as_ref().is_none_or(|a| a.pie),
        _ => return None,
    };
    let mut out = Vec::new();
    let mut split = false;
    if closed_kind {
        let band = shaping::stroke_band(cut, gap.max(HAIR), false);
        let parts = shaping::overlay_parts(&page, &band, shaping::Op::Trim);
        // Split means more than one piece, or a piece less (cut away).
        if parts.len() < 2 {
            return None;
        }
        out.extend(parts.into_iter().map(|p| (p, true)));
        split = true;
    } else {
        for (sub, closed) in subpaths(&page) {
            if closed {
                out.push((sub, true));
                continue;
            }
            let cuts = crossings(&sub, cut);
            if cuts.is_empty() {
                out.push((sub, false));
                continue;
            }
            let parts = split_open(&sub, &cuts);
            split |= parts.len() > 1;
            out.extend(parts.into_iter().map(|p| (p, false)));
        }
    }
    split.then_some(out)
}

impl App {
    /// Cut along `points` (page space): two points for a straight cut,
    /// more for a freehand one.
    pub fn knife_cut(&mut self, points: &[Point]) {
        if points.len() < 2 {
            return;
        }
        let targets: Vec<Shape> = if self.selection.is_empty() {
            self.doc()
                .page(self.page)
                .map(|p| {
                    p.layers
                        .iter()
                        .filter(|l| !l.locked && l.visible)
                        .flat_map(|l| l.shapes.clone())
                        .collect()
                })
                .unwrap_or_default()
        } else {
            self.selected_shapes()
        };
        let Some(layer) = self.active_layer() else {
            return;
        };
        let gap = self.knife.gap.clamp(0.0, 1000.0);
        let mut cmds = Vec::new();
        let mut new_ids: Vec<ShapeId> = Vec::new();
        for s in targets {
            if s.locked {
                continue;
            }
            let b = s.bounds();
            let cut = cut_line(points, b.inflate(1.0, 1.0));
            let reach = cut
                .iter()
                .fold(Rect::from_points(cut[0], cut[0]), |r, p| r.union_pt(*p))
                .inflate(gap + 1.0, gap + 1.0);
            if reach.intersect(b).is_zero_area() && !b.is_zero_area() {
                continue;
            }
            let Some(pieces) = pieces_of(&s, &cut, gap) else {
                continue;
            };
            let inv = s.transform.inverse();
            let local = |p: &BezPath| inv * p.clone();
            let pieces: Vec<(BezPath, bool)> = if self.knife.one_object {
                let mut all = BezPath::new();
                let mut closed = true;
                for (p, c) in &pieces {
                    all.extend(local(p).elements().iter().copied());
                    closed &= *c;
                }
                vec![(all, closed)]
            } else {
                pieces.iter().map(|(p, c)| (local(p), *c)).collect()
            };
            let mut it = pieces.into_iter();
            // The first piece keeps the object (its id, its place).
            if let Some((p, closed)) = it.next() {
                cmds.push(Command::SetShapeKind {
                    shape: s.id,
                    kind: ShapeKind::Path { path: p, closed },
                });
                new_ids.push(s.id);
            }
            for (p, closed) in it {
                let id = self.engine.new_shape_id();
                let mut piece = Shape::new(id, ShapeKind::Path { path: p, closed });
                piece.transform = s.transform;
                piece.fill = s.fill.clone();
                piece.stroke = s.stroke.clone();
                piece.opacity = s.opacity;
                cmds.push(Command::AddShape {
                    layer,
                    shape: piece,
                });
                new_ids.push(id);
            }
        }
        if cmds.is_empty() {
            return;
        }
        if let Err(e) = self.engine.run_batch("Knife", &cmds) {
            self.status = e.to_string();
            return;
        }
        self.select(new_ids);
    }

    /// The two-point knife's drag ended at `b`.
    pub fn knife(&mut self, a: Point, b: Point) {
        self.knife_cut(&[a, b]);
    }
}

/// Points along a straight drag, for tests.
#[cfg(test)]
fn line(a: Point, b: Point, n: usize) -> Vec<Point> {
    (0..=n).map(|i| a.lerp(b, i as f64 / n as f64)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(app: &mut App) -> ShapeId {
        app.new_shape(ShapeKind::Rect {
            rect: Rect::new(0.0, 0.0, 40.0, 40.0),
            radius: 0.0,
            corners: None,
        })
        .expect("shape")
    }

    fn count(app: &App) -> usize {
        app.doc().pages[0].layers[0].shapes.len()
    }

    /// A straight cut through the middle: two halves that meet, one undo
    /// step; with a gap they are that far apart; kept as one object they
    /// are one.
    #[test]
    fn straight_cuts_split_closed_objects() {
        let mut app = App::headless();
        square(&mut app);
        app.select(Vec::new());
        app.knife(Point::new(20.0, 10.0), Point::new(20.0, 30.0));
        assert_eq!(count(&app), 2);
        let bounds: Vec<Rect> = app.doc().pages[0].layers[0]
            .shapes
            .iter()
            .map(|s| s.bounds())
            .collect();
        assert!(
            bounds.iter().all(|b| (b.width() - 20.0).abs() < 0.05),
            "{bounds:?}"
        );
        app.undo();
        assert_eq!(count(&app), 1);
        app.knife.gap = 4.0;
        app.knife(Point::new(20.0, 10.0), Point::new(20.0, 30.0));
        let mut xs: Vec<f64> = app.doc().pages[0].layers[0]
            .shapes
            .iter()
            .map(|s| s.bounds().x1)
            .collect();
        xs.sort_by(f64::total_cmp);
        assert!((xs[0] - 18.0).abs() < 0.1, "{xs:?}");
        app.undo();
        app.knife.gap = 0.0;
        app.knife.one_object = true;
        app.knife(Point::new(20.0, 10.0), Point::new(20.0, 30.0));
        assert_eq!(count(&app), 1);
        // A cut beside the object leaves it alone.
        app.undo();
        app.knife(Point::new(60.0, 0.0), Point::new(60.0, 10.0));
        assert_eq!(count(&app), 1);
    }

    /// A freehand cut across a square: the pieces on either side of the
    /// stroke; an open line is split where the cut crosses it.
    #[test]
    fn freehand_cuts_and_open_curves() {
        let mut app = App::headless();
        square(&mut app);
        app.select(Vec::new());
        // A bent stroke from the left side out the bottom.
        let mut stroke = line(Point::new(-5.0, 30.0), Point::new(20.0, 20.0), 10);
        stroke.extend(line(Point::new(20.0, 20.0), Point::new(25.0, -5.0), 10));
        app.knife_cut(&stroke);
        assert_eq!(count(&app), 2);
        app.undo();
        // An open line across a vertical cut: two lines.
        let mut path = BezPath::new();
        path.move_to(Point::new(0.0, 60.0));
        path.line_to(Point::new(40.0, 60.0));
        let l = app
            .new_shape(ShapeKind::Path {
                path,
                closed: false,
            })
            .expect("line");
        app.select(vec![l]);
        app.knife(Point::new(10.0, 50.0), Point::new(10.0, 70.0));
        let lines: Vec<Rect> = app
            .selection
            .iter()
            .filter_map(|id| app.doc().shape_on_page(*id))
            .map(|s| s.bounds())
            .collect();
        assert_eq!(lines.len(), 2);
        let mut widths: Vec<f64> = lines.iter().map(|b| b.width()).collect();
        widths.sort_by(f64::total_cmp);
        assert!(
            (widths[0] - 10.0).abs() < 1e-6 && (widths[1] - 30.0).abs() < 1e-6,
            "{widths:?}"
        );
        // Open pieces stay open.
        for id in &app.selection {
            let s = app.doc().find_shape(*id).expect("piece");
            assert!(matches!(s.kind, ShapeKind::Path { closed: false, .. }));
        }
    }
}
