//! Selection marks that read on any drawing: handles, selection outlines
//! and the text cursor take black or white from the pixels under them
//! (the last drawing of the page, over the paper or the desktop), so a
//! selection on a black object is as clear as one on the white page.

use crate::app::App;
use egui::{epaint, Color32, Painter, Pos2, Rect as ERect, Stroke as EStroke};
use graphdraw_core::document::{Shape, ShapeKind};

/// Half the side, in screen pixels, of the square read around a point.
const SAMPLE_RADIUS: i64 = 1;

/// More flattened points than this in the selection and the objects'
/// own outlines are left out (the handles still show).
const OUTLINE_POINT_LIMIT: usize = 40_000;

/// How long the text cursor stays on, then off, in seconds.
pub const BLINK_HALF_PERIOD: f64 = 0.53;

/// Whether the canvas keeps a copy of its drawn pixels: the eyedroppers
/// read them, and so do the selection marks while something is selected
/// or a text is edited.
pub fn keep_pixels(app: &App) -> bool {
    crate::eyedropper::is_eyedropper(app.tool)
        || !app.selection.is_empty()
        || app.text_edit.is_some()
        || app.table_edit.is_some()
}

/// The colour shown at screen point `pos`: the drawing's pixels averaged
/// over a few pixels, over the paper on the page and the desktop around
/// it. Without kept pixels, the paper or the desktop alone.
pub fn backdrop(app: &App, pos: Pos2) -> Color32 {
    let paper = crate::canvas::page_screen_rect(app);
    let desk = app.desktop_color();
    let base = |q: Pos2| {
        if paper.contains(q) {
            crate::theme::Tokens::PAGE
        } else {
            desk
        }
    };
    let raster = app.raster.borrow();
    let Some(kept) = raster.kept.as_ref() else {
        return base(pos);
    };
    let (w, h) = (kept.size[0], kept.size[1]);
    if w == 0 || h == 0 || !kept.screen.is_positive() {
        return base(pos);
    }
    let sx = w as f32 / kept.screen.width();
    let sy = h as f32 / kept.screen.height();
    let cx = ((pos.x - kept.screen.min.x) * sx).floor() as i64;
    let cy = ((pos.y - kept.screen.min.y) * sy).floor() as i64;
    let (mut r, mut g, mut b, mut n) = (0u32, 0u32, 0u32, 0u32);
    for y in cy - SAMPLE_RADIUS..=cy + SAMPLE_RADIUS {
        for x in cx - SAMPLE_RADIUS..=cx + SAMPLE_RADIUS {
            let q = Pos2::new(
                kept.screen.min.x + (x as f32 + 0.5) / sx,
                kept.screen.min.y + (y as f32 + 0.5) / sy,
            );
            let under = base(q);
            let inside = x >= 0 && y >= 0 && (x as usize) < w && (y as usize) < h;
            let px = if inside {
                kept.pixels.get(y as usize * w + x as usize).copied()
            } else {
                None
            };
            let c = match px {
                Some(px) => over(px, under),
                None => under,
            };
            r += c.r() as u32;
            g += c.g() as u32;
            b += c.b() as u32;
            n += 1;
        }
    }
    let n = n.max(1);
    Color32::from_rgb((r / n) as u8, (g / n) as u8, (b / n) as u8)
}

/// Premultiplied `top` over the opaque `under`.
fn over(top: Color32, under: Color32) -> Color32 {
    let rest = 255 - top.a() as u32;
    let mix = |t: u8, u: u8| (t as u32 + (u as u32 * rest + 127) / 255).min(255) as u8;
    Color32::from_rgb(
        mix(top.r(), under.r()),
        mix(top.g(), under.g()),
        mix(top.b(), under.b()),
    )
}

/// Black on light colours, white on dark ones (by perceived brightness).
pub fn ink_on(bg: Color32) -> Color32 {
    let y = 0.299 * bg.r() as f32 + 0.587 * bg.g() as f32 + 0.114 * bg.b() as f32;
    if y >= 128.0 {
        Color32::BLACK
    } else {
        Color32::WHITE
    }
}

/// The other one of black and white.
pub fn opposite(ink: Color32) -> Color32 {
    if ink == Color32::BLACK {
        Color32::WHITE
    } else {
        Color32::BLACK
    }
}

/// The ink that stands out at screen point `pos`.
pub fn ink_at(app: &App, pos: Pos2) -> Color32 {
    ink_on(backdrop(app, pos))
}

/// A selection handle at `p`: a square (a circle for objects chosen
/// inside a group) in the ink of what is under it, edged in the other
/// ink so it stays clear where light and dark meet.
pub fn draw_handle(app: &App, painter: &Painter, p: Pos2, size: f32, round: bool) {
    let ink = ink_at(app, p);
    let edge = EStroke::new(1.0, opposite(ink));
    if round {
        painter.circle(p, size / 2.0 + 0.5, ink, edge);
    } else {
        painter.rect(
            ERect::from_center_size(p, egui::vec2(size, size)),
            0.0,
            ink,
            edge,
            epaint::StrokeKind::Outside,
        );
    }
}

/// A one-pixel polyline, solid or dashed, whose every stretch takes the
/// ink of what lies under its middle.
pub fn draw_polyline(app: &App, painter: &Painter, pts: &[Pos2], dashed: bool) {
    let put = |run: Vec<Pos2>, ink: Color32| {
        let stroke = EStroke::new(1.0, ink);
        if dashed {
            painter.extend(egui::Shape::dashed_line(&run, stroke, 4.0, 3.0));
        } else {
            painter.add(epaint::PathShape::line(run, stroke));
        }
    };
    let mut run: Vec<Pos2> = Vec::new();
    let mut run_ink = Color32::TRANSPARENT;
    for w in pts.windows(2) {
        let (a, b) = (w[0], w[1]);
        let ink = ink_at(app, a + (b - a) * 0.5);
        if ink != run_ink && run.len() > 1 {
            put(std::mem::take(&mut run), run_ink);
        }
        if run.is_empty() || ink != run_ink {
            run.clear();
            run.push(a);
        }
        run.push(b);
        run_ink = ink;
    }
    if run.len() > 1 {
        put(run, run_ink);
    }
}

/// The outline the selection marks follow for `s`, in screen points:
/// a paragraph text has its frame (drawn with the frames), an artistic
/// text the box of its lines, every other object (texts converted to
/// curves too) its own path.
pub fn selection_outline(app: &App, s: &Shape) -> Vec<Vec<Pos2>> {
    if let ShapeKind::Text { .. } = &s.kind {
        if s.text_frame_corners().is_some() {
            return Vec::new();
        }
        let r = app.view.rect_to_screen(s.bounds());
        return vec![vec![
            r.left_top(),
            r.right_top(),
            r.right_bottom(),
            r.left_bottom(),
            r.left_top(),
        ]];
    }
    crate::canvas::flatten(&s.page_path(), &app.view)
        .into_iter()
        .filter(|(pts, _)| pts.len() > 1)
        .map(|(mut pts, closed)| {
            if closed {
                if let Some(first) = pts.first().copied() {
                    pts.push(first);
                }
            }
            pts
        })
        .collect()
}

/// A hairline along each selected object, in the ink of what is under it:
/// solid for the Pick tools, dashed for the Shape tool.
pub fn draw_selection_outlines(app: &App, painter: &Painter, shapes: &[Shape], dashed: bool) {
    let lines: Vec<Vec<Pos2>> = shapes
        .iter()
        .flat_map(|s| selection_outline(app, s))
        .collect();
    if lines.iter().map(Vec::len).sum::<usize>() > OUTLINE_POINT_LIMIT {
        return;
    }
    for pts in &lines {
        draw_polyline(app, painter, pts, dashed);
    }
}

/// Whether the text cursor shows at `now`, `since` being when it last
/// moved: on for half a period, off for the next, starting on.
pub fn caret_on(now: f64, since: f64) -> bool {
    let k = ((now - since).max(0.0) / BLINK_HALF_PERIOD).floor() as i64;
    k % 2 == 0
}

/// The text cursor from `top` to `bottom`: blinking, in the ink of what
/// is under it, edged in the other ink. It shows steadily again each
/// time `key` (where it is) changes.
pub fn draw_caret(app: &App, painter: &Painter, top: Pos2, bottom: Pos2, key: u64) {
    let ctx = painter.ctx();
    let now = ctx.input(|i| i.time);
    let id = egui::Id::new("graphdraw.caret_blink");
    let since = ctx.data_mut(|d| {
        let (k, t) = d.get_temp::<(u64, f64)>(id).unwrap_or((key, now));
        let t = if k == key { t } else { now };
        d.insert_temp(id, (key, t));
        t
    });
    let left = BLINK_HALF_PERIOD - ((now - since).max(0.0) % BLINK_HALF_PERIOD);
    ctx.request_repaint_after(std::time::Duration::from_secs_f64(left.max(0.01)));
    if !caret_on(now, since) {
        return;
    }
    let ink = ink_at(app, top + (bottom - top) * 0.5);
    painter.line_segment([top, bottom], EStroke::new(3.0, opposite(ink)));
    painter.line_segment([top, bottom], EStroke::new(1.5, ink));
}

/// A key for where a cursor is, so its blink restarts when it moves.
pub fn caret_key(parts: impl std::hash::Hash) -> u64 {
    use std::hash::Hasher;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    parts.hash(&mut h);
    h.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ink_is_the_opposite_of_the_background() {
        assert_eq!(ink_on(Color32::BLACK), Color32::WHITE);
        assert_eq!(ink_on(Color32::WHITE), Color32::BLACK);
        assert_eq!(ink_on(Color32::from_rgb(0, 0, 160)), Color32::WHITE);
        assert_eq!(ink_on(Color32::from_rgb(255, 230, 0)), Color32::BLACK);
        assert_eq!(opposite(Color32::BLACK), Color32::WHITE);
        assert_eq!(opposite(Color32::WHITE), Color32::BLACK);
    }

    #[test]
    fn transparent_pixels_show_what_is_under_them() {
        assert_eq!(over(Color32::TRANSPARENT, Color32::WHITE), Color32::WHITE);
        assert_eq!(over(Color32::BLACK, Color32::WHITE), Color32::BLACK);
        let half = Color32::from_rgba_premultiplied(0, 0, 0, 128);
        assert!(over(half, Color32::WHITE).r() < 130);
    }

    #[test]
    fn the_cursor_blinks_and_starts_on() {
        assert!(caret_on(10.0, 10.0));
        assert!(caret_on(10.0 + BLINK_HALF_PERIOD * 0.9, 10.0));
        assert!(!caret_on(10.0 + BLINK_HALF_PERIOD * 1.1, 10.0));
        assert!(caret_on(10.0 + BLINK_HALF_PERIOD * 2.1, 10.0));
        // A clock behind the move counts as just moved.
        assert!(caret_on(9.0, 10.0));
    }

    #[test]
    fn a_black_object_under_the_handles_turns_them_white() {
        use graphdraw_core::geometry::{Point, Rect};
        let app = App::headless();
        let page = app.page_rect();
        let center = Point::new(page.center().x, page.center().y);
        let screen = app.view.to_screen(center);
        // The paper alone: black ink.
        assert_eq!(ink_at(&app, screen), Color32::BLACK);
        // A black drawing under the point: white ink.
        let full = app.view.rect_to_screen(Rect::new(
            center.x - 50.0,
            center.y - 50.0,
            center.x + 50.0,
            center.y + 50.0,
        ));
        app.raster.borrow_mut().kept = Some(crate::raster::KeptPixels {
            pixels: std::sync::Arc::new(vec![Color32::BLACK; 16]),
            size: [4, 4],
            screen: full,
        });
        assert_eq!(ink_at(&app, screen), Color32::WHITE);
    }

    #[test]
    fn artistic_text_is_marked_by_its_box_and_curves_by_their_path() {
        use graphdraw_core::geometry::Point;
        let mut app = App::headless();
        app.start_text(Point::new(30.0, 100.0), None);
        app.text_insert("Hi");
        let id = app.text_edit.as_ref().expect("editing").shape;
        app.finish_text();
        let (_, text) = app.doc().shape(id).expect("the text");
        let text = text.clone();
        let outline = selection_outline(&app, &text);
        assert_eq!(outline.len(), 1);
        assert_eq!(outline[0].len(), 5);
        // Converted to curves: each letter's own outline, not a box.
        let mut path = graphdraw_core::geometry::BezPath::new();
        for x in [30.0, 40.0] {
            path.move_to(Point::new(x, 100.0));
            path.line_to(Point::new(x + 5.0, 100.0));
            path.line_to(Point::new(x + 5.0, 92.0));
            path.close_path();
        }
        let mut curves = text.clone();
        curves.kind = ShapeKind::Path { path, closed: true };
        curves.transform = graphdraw_core::geometry::Affine::IDENTITY;
        let outline = selection_outline(&app, &curves);
        assert_eq!(outline.len(), 2);
        assert!(outline.iter().all(|pts| pts.first() == pts.last()));
    }
}
