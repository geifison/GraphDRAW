//! The handles of a text on a path on the canvas (Pick tool, the text
//! selected on its own), one job each, with the action named by the
//! pointer: a red diamond at the start slides the text along the path
//! (drawn to the start, middle and end of the path), a green dot moves it
//! away from the stroke, a double arrow puts it on the other side (a
//! click), and a blue dot at the end opens or closes the letter spacing.
//! On a closed path a drag does not jump when it passes the start. A drag
//! shows where the letters would go and is one undo step on release.
//! Also: the outline of the object under the pointer while Fit Text to
//! Path waits for its guide (green when it can guide, red when not).

use crate::app::{App, Drag};
use crate::i18n::tr;
use egui::{Color32, Painter, Pos2, Stroke as EStroke, Vec2 as EVec2};
use graphdraw_core::{
    document::ShapeKind,
    geometry::{Affine, Point, Vec2},
    text_path::Track,
    Command,
};

/// What a handle of a text on a path does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathHandle {
    Slide,
    Distance,
    Flip,
    Spacing,
}

impl PathHandle {
    /// The action's name shown by the pointer.
    pub fn key(self) -> &'static str {
        match self {
            PathHandle::Slide => "text_path.handle_slide",
            PathHandle::Distance => "text_path.handle_distance",
            PathHandle::Flip => "text_path.handle_flip",
            PathHandle::Spacing => "text_path.handle_spacing",
        }
    }
}

/// Screen distance at which a handle is under the pointer.
const HIT_PX: f32 = 7.0;
/// Screen distance the start of the text is drawn to the start, middle
/// and end of the path.
const MAGNET_PX: f64 = 8.0;

/// Where the handles of the selected text on a path are, and what the
/// drags measure them against.
pub struct PathHandles {
    pub id: graphdraw_core::ShapeId,
    /// Page positions of the handles.
    pub start: Point,
    pub lift: Point,
    pub flip: Point,
    pub spacing: Point,
    track: Track,
    span: graphdraw_text::PathSpan,
    /// From the text's path space to the page.
    to_page: Affine,
    /// The point of the track where the text starts and the letters'
    /// "up" there (path space).
    p0: Point,
    n0: Vec2,
    /// Height of the green dot above the baseline.
    lift_height: f64,
    size: f64,
    glyphs: usize,
    offset: f64,
}

impl PathHandles {
    fn s_at(&self, page: Point) -> f64 {
        self.track.project(self.to_page.inverse() * page)
    }

    /// On a closed path `s` taken round to near `near`, so a drag does
    /// not jump a whole turn when it passes the start of the path.
    fn unwrap_s(&self, s: f64, near: f64) -> f64 {
        let t = self.span.total;
        if !self.track.closed() || t <= 0.0 {
            return s;
        }
        s + ((near - s) / t).round() * t
    }
}

impl App {
    /// The handles of the text on a path selected on its own.
    pub fn path_handles(&self) -> Option<PathHandles> {
        use crate::tools::Tool;
        if !matches!(self.tool, Tool::Pick | Tool::FreeformPick) || self.rotate_mode {
            return None;
        }
        let ft = self.fitted_text()?;
        let (_, s) = self.doc().shape(ft.id).ok()?;
        let ShapeKind::Text { spans, para, .. } = &s.kind else {
            return None;
        };
        let span = crate::text_path_link::path_span(spans, para, ft.align, &ft.on_path)?;
        let track = Track::new(&ft.on_path)?;
        let size = crate::text_path_link::body_mm(spans);
        let det = ft.to_page.determinant();
        if !det.is_finite() || det.abs() < 1e-12 {
            return None;
        }
        let sgn = if ft.on_path.flipped() { -1.0 } else { 1.0 };
        let up = |d: Vec2| Vec2::new(-d.y, d.x) * sgn;
        let (p0, d0) = track.at(span.start);
        let n0 = up(d0);
        let lift_height = size * 1.1;
        let d = ft.on_path.distance;
        let lift = p0 + n0 * (d + lift_height);
        let (pe, de) = track.at(span.end);
        let spacing = pe + up(de) * (d + size * 0.35);
        let glyphs = graphdraw_core::document::spans_text(spans)
            .chars()
            .filter(|c| *c != '\n')
            .count()
            .saturating_sub(1);
        let m = ft.to_page;
        Some(PathHandles {
            id: ft.id,
            start: m * p0,
            lift: m * lift,
            flip: m * (lift + d0 * size * 0.8),
            spacing: m * spacing,
            track,
            span,
            to_page: m,
            p0,
            n0,
            lift_height,
            size,
            glyphs,
            offset: ft.on_path.offset,
        })
    }

    /// The handle of the selected text on a path under the screen point.
    pub fn path_handle_at(&self, screen: Pos2) -> Option<PathHandle> {
        let h = self.path_handles()?;
        let near = |q: Point| self.view.to_screen(q).distance(screen) <= HIT_PX;
        // What lies on top wins where they overlap.
        if near(h.flip) {
            Some(PathHandle::Flip)
        } else if near(h.lift) {
            Some(PathHandle::Distance)
        } else if near(h.start) {
            Some(PathHandle::Slide)
        } else if near(h.spacing) {
            Some(PathHandle::Spacing)
        } else {
            None
        }
    }

    /// A press on a handle of the text on a path (Pick tool): a drag
    /// starts, or a click on the double arrow puts the text on the other
    /// side. True when the input was used here.
    pub fn path_handle_input(
        &mut self,
        response: &egui::Response,
        p: Point,
        screen: Pos2,
    ) -> bool {
        if !matches!(self.drag, Drag::None) {
            return false;
        }
        let press = response.drag_started_by(egui::PointerButton::Primary);
        let click = response.clicked_by(egui::PointerButton::Primary);
        if !press && !click {
            return false;
        }
        // The press position: where the drag began.
        let at_screen = response
            .ctx
            .input(|i| i.pointer.press_origin())
            .unwrap_or(screen);
        let Some(handle) = self.path_handle_at(at_screen) else {
            return false;
        };
        let start = self.view.to_page(at_screen);
        match (handle, press) {
            (PathHandle::Flip, _) => {
                if click {
                    self.flip_text_side();
                }
            }
            (h, true) => {
                self.drag = Drag::PathHandle {
                    handle: h,
                    start,
                    current: p,
                };
            }
            _ => {}
        }
        true
    }

    /// The text on a path as a drag of `handle` from `start` to `current`
    /// (page) leaves it.
    pub fn path_handle_kind(
        &self,
        handle: PathHandle,
        start: Point,
        current: Point,
    ) -> Option<(graphdraw_core::ShapeId, ShapeKind, String)> {
        let h = self.path_handles()?;
        let zoom = f64::from(self.view.zoom).max(1e-6);
        let u = self.units;
        match handle {
            PathHandle::Slide => {
                let grab = h.s_at(start);
                let s = h.unwrap_s(h.s_at(current), grab);
                let mut d = s - grab;
                // Drawn to the start, middle and end of the path.
                let w = h.span.end - h.span.start;
                let snap = MAGNET_PX / zoom;
                let new_start = h.span.start + d;
                let total = h.span.total;
                for target in [0.0, total / 2.0 - w / 2.0, total - w] {
                    if (new_start - target).abs() <= snap {
                        d = target - h.span.start;
                    }
                }
                let offset = h.offset + d;
                let kind = self.fitted_kind_with(h.id, false, |tp, _| tp.offset = offset)?;
                let label = format!(
                    "{}: {:.2} {}",
                    tr("toolbar.path_offset"),
                    u.from_mm(offset),
                    u.short()
                );
                Some((h.id, kind, label))
            }
            PathHandle::Distance => {
                let q = h.to_page.inverse() * current;
                let dist = (q - h.p0).dot(h.n0) - h.lift_height;
                let kind = self.fitted_kind_with(h.id, false, |tp, _| tp.distance = dist)?;
                let label = format!(
                    "{}: {:.2} {}",
                    tr("toolbar.path_distance"),
                    u.from_mm(dist),
                    u.short()
                );
                Some((h.id, kind, label))
            }
            PathHandle::Spacing => {
                // How far the end went is shared between the letters.
                let grab = h.s_at(start);
                let s = h.unwrap_s(h.s_at(current), grab);
                let delta = (s - grab) / (h.glyphs.max(1) as f64 * h.size) * 100.0;
                let mut shown = 0.0;
                let kind = self.fitted_kind_with(h.id, true, |_, spans| {
                    for sp in spans.iter_mut() {
                        sp.tracking_pct = (sp.tracking_pct + delta).clamp(-90.0, 1000.0);
                    }
                    shown = spans.first().map_or(0.0, |sp| sp.tracking_pct);
                })?;
                let label = format!("{}: {:.0} %", tr("toolbar.letter_spacing"), shown);
                Some((h.id, kind, label))
            }
            PathHandle::Flip => None,
        }
    }

    /// The end of a handle drag: one undo step.
    pub fn finish_path_handle(&mut self, handle: PathHandle, start: Point, current: Point) {
        if (current - start).hypot() < 1e-9 {
            return;
        }
        let Some((id, kind, _)) = self.path_handle_kind(handle, start, current) else {
            return;
        };
        let same = self.doc().shape(id).is_ok_and(|(_, s)| s.kind == kind);
        if same {
            return;
        }
        if let Err(e) = self
            .engine
            .run_batch("Text on Path", &[Command::SetShapeKind { shape: id, kind }])
        {
            self.status = e.to_string();
        }
    }

    /// The curve whose outline is under the page point (the Text tool
    /// there starts a text on it).
    pub fn text_curve_at(&self, p: Point) -> Option<graphdraw_core::ShapeId> {
        let tolerance = 4.0 / f64::from(self.view.zoom).max(1e-6);
        let id = self.hit_test(p)?;
        let (_, s) = self.doc().shape(id).ok()?;
        if !graphdraw_core::text_path::can_guide(&s.kind) {
            return None;
        }
        let path = s.page_path();
        let near = path.segments().any(|seg| {
            kurbo::ParamCurveNearest::nearest(&seg, p, 1e-3).distance_sq <= tolerance * tolerance
        });
        near.then_some(id)
    }
}

fn red() -> Color32 {
    Color32::from_rgb(0xD9, 0x30, 0x25)
}

fn green() -> Color32 {
    Color32::from_rgb(0x1E, 0x9E, 0x4A)
}

fn blue() -> Color32 {
    crate::theme::Tokens::ACCENT
}

/// The handles of the selected text on a path, the action under the
/// pointer, a handle drag's preview, and the guide picking outline.
pub fn draw(app: &App, painter: &Painter) {
    draw_pick_guide(app, painter);
    if let Drag::PathHandle {
        handle,
        start,
        current,
    } = app.drag
    {
        draw_drag(app, painter, handle, start, current);
        return;
    }
    let Some(h) = app.path_handles() else {
        return;
    };
    let s = |q: Point| app.view.to_screen(q);
    let (start, lift, flip, sp) = (s(h.start), s(h.lift), s(h.flip), s(h.spacing));
    let edge = EStroke::new(1.0, Color32::WHITE);
    let r = 4.0;
    // Slide: the red diamond at the start, joined to the green dot.
    painter.line_segment([start, lift], EStroke::new(1.5, green()));
    let k = 5.5;
    painter.add(egui::Shape::convex_polygon(
        vec![
            start + EVec2::new(0.0, -k),
            start + EVec2::new(k, 0.0),
            start + EVec2::new(0.0, k),
            start + EVec2::new(-k, 0.0),
        ],
        red(),
        edge,
    ));
    // Away from the stroke: the green dot.
    painter.circle(lift, r, green(), edge);
    // Other side: a hollow green dot with the double arrow.
    painter.circle(flip, r + 1.0, Color32::WHITE, EStroke::new(1.0, green()));
    let st = EStroke::new(1.2, green());
    painter.line_segment([flip + EVec2::new(0.0, -3.5), flip + EVec2::new(0.0, 3.5)], st);
    for d in [-1.0f32, 1.0] {
        let tip = flip + EVec2::new(0.0, 3.5 * d);
        painter.line_segment([tip, tip + EVec2::new(-2.0, -2.0 * d)], st);
        painter.line_segment([tip, tip + EVec2::new(2.0, -2.0 * d)], st);
    }
    // Letter spacing: the blue dot at the end, arrows to both sides.
    painter.circle(sp, r, blue(), edge);
    let ast = EStroke::new(1.5, blue());
    for d in [-1.0f32, 1.0] {
        let tip = sp + EVec2::new(12.0 * d, 0.0);
        painter.line_segment([sp + EVec2::new((r + 2.0) * d, 0.0), tip], ast);
        painter.line_segment([tip, tip + EVec2::new(-3.0 * d, -3.0)], ast);
        painter.line_segment([tip, tip + EVec2::new(-3.0 * d, 3.0)], ast);
    }
    // The action's name by the pointer.
    if let Some(pos) = painter.ctx().input(|i| i.pointer.hover_pos()) {
        if let Some(kind) = app.path_handle_at(pos) {
            label(painter, pos, &tr(kind.key()));
        }
    }
}

fn label(painter: &Painter, pos: Pos2, text: &str) {
    let font = egui::FontId::proportional(11.0);
    let galley = painter.layout_no_wrap(text.to_string(), font, Color32::WHITE);
    let at = pos + EVec2::new(14.0, 14.0);
    let bg = egui::Rect::from_min_size(at, galley.size() + EVec2::splat(6.0))
        .translate(EVec2::splat(-3.0));
    painter.rect_filled(bg, 3.0, Color32::from_black_alpha(200));
    painter.galley(at, galley, Color32::WHITE);
}

/// A handle being dragged: the letters where they would go, outlined in
/// blue, and the value by the pointer.
fn draw_drag(app: &App, painter: &Painter, handle: PathHandle, start: Point, current: Point) {
    let Some((id, kind, text)) = app.path_handle_kind(handle, start, current) else {
        return;
    };
    let Some(ft) = app.fitted_text().filter(|f| f.id == id) else {
        return;
    };
    let ShapeKind::Text {
        spans,
        align,
        para,
        on_path,
        ..
    } = &kind
    else {
        return;
    };
    let req = graphdraw_core::document::text_outline::TextRequest {
        spans,
        frame: None,
        align: *align,
        para,
        on_path: on_path.as_ref(),
    };
    let letters = graphdraw_core::document::text_outline::outline(&req).unwrap_or_default();
    let halo = EStroke::new(3.0, Color32::from_white_alpha(220));
    let stroke = EStroke::new(1.0, crate::theme::Tokens::DRAG_OUTLINE);
    for (mut pts, closed) in crate::canvas::flatten(&(ft.to_page * letters), &app.view) {
        if closed {
            if let Some(first) = pts.first().copied() {
                pts.push(first);
            }
        }
        painter.add(egui::Shape::line(pts.clone(), halo));
        painter.add(egui::Shape::line(pts, stroke));
    }
    label(painter, app.view.to_screen(current), &text);
}

/// Fit Text to Path waiting for its guide: the object under the pointer
/// outlined in green when it can guide the text, in red when not.
fn draw_pick_guide(app: &App, painter: &Painter) {
    if !app.pending_text_path {
        return;
    }
    let Some(pos) = painter.ctx().input(|i| i.pointer.hover_pos()) else {
        return;
    };
    let p = app.view.to_page(pos);
    let Some(id) = app.hit_test(p).filter(|id| !app.selection.contains(id)) else {
        return;
    };
    let Some(s) = app.doc().shape_on_page(id) else {
        return;
    };
    let color = if app.can_guide_text(id) { green() } else { red() };
    let stroke = EStroke::new(2.5, color);
    for (mut pts, closed) in crate::canvas::flatten(&s.page_path(), &app.view) {
        if closed {
            if let Some(first) = pts.first().copied() {
                pts.push(first);
            }
        }
        painter.add(egui::Shape::line(pts, stroke));
    }
}
