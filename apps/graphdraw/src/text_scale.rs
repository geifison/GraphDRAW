//! A text scaled with the Pick tool's handles (or the property bar's size
//! and scale fields) takes the vertical part of the scale as its size:
//! artistic text of 24 pt made twice as tall is 48 pt and its transform
//! keeps no scale; stretched in width only, the stretch stays in the
//! transform and the size follows the letters' height. A paragraph text
//! scaled with Ctrl held grows its letters and its frame the same way.
//! See docs/behavior/text.md.

use graphdraw_core::geometry::{Affine, Point, Size};
use graphdraw_core::paragraphs::ParagraphFormat;
use graphdraw_core::{Command, Fill, LeadingUnit, ParagraphStyle, Shape, ShapeKind};

/// Lengths of a paragraph style (mm, or points for a line spacing in
/// points) made `k` times as long; the baseline grid is the page's and
/// stays.
fn scale_para(p: &mut ParagraphStyle, k: f64) {
    if p.leading_unit == LeadingUnit::Points {
        p.leading_pct *= k;
    }
    for v in [
        &mut p.space_before,
        &mut p.space_after,
        &mut p.first_line_indent,
        &mut p.left_indent,
        &mut p.right_indent,
        &mut p.gutter,
        &mut p.bullet_indent,
    ] {
        *v *= k;
    }
    p.tabs.iter_mut().for_each(|t| *t *= k);
}

/// [`scale_para`] for a paragraph's own formatting.
fn scale_format(p: &mut ParagraphFormat, k: f64) {
    if p.leading_unit == LeadingUnit::Points {
        p.leading_pct *= k;
    }
    for v in [
        &mut p.space_before,
        &mut p.space_after,
        &mut p.first_line_indent,
        &mut p.left_indent,
        &mut p.right_indent,
        &mut p.bullet_indent,
    ] {
        *v *= k;
    }
    p.tabs.iter_mut().for_each(|t| *t *= k);
}

/// The commands that give text `s` the transform `t` (page space) with
/// its vertical scale written into its letters: the letters (and a
/// paragraph frame) `k` times as large about the text's origin, the
/// transform made to undo that, an outline that scales with the object
/// `k` times as wide. `None` when there is nothing to write (no change of
/// height, not a text) or the text keeps its transform as before: on a
/// path, with a pattern, texture or mesh fill, or with live effects.
pub(crate) fn baked_text(s: &Shape, t: Affine) -> Option<Vec<Command>> {
    let ShapeKind::Text {
        spans,
        origin,
        frame,
        align,
        para,
        on_path: None,
    } = &s.kind
    else {
        return None;
    };
    if !matches!(s.fill, Fill::None | Fill::Solid(_) | Fill::Fountain(_)) || !s.effects.is_empty()
    {
        return None;
    }
    let tc = t * s.transform;
    let [_, _, c, d, _, _] = tc.as_coeffs();
    let k = c.hypot(d);
    if !k.is_finite() || k < 1e-6 || (k - 1.0).abs() < 1e-9 || s.transform.determinant().abs() < 1e-12 {
        return None;
    }
    let mut spans = spans.clone();
    for sp in &mut spans {
        sp.size_pt *= k;
        sp.baseline_shift_pt *= k;
        if let Some(f) = sp.paragraph.as_mut() {
            scale_format(f, k);
        }
    }
    let mut para = para.clone();
    scale_para(&mut para, k);
    let o: Point = *origin;
    let unscale = Affine::translate(o.to_vec2())
        * Affine::scale(1.0 / k)
        * Affine::translate(-o.to_vec2());
    let new_transform = tc * unscale;
    let mut cmds = vec![
        Command::SetShapeKind {
            shape: s.id,
            kind: ShapeKind::Text {
                spans,
                origin: o,
                frame: frame.map(|f| Size::new(f.width * k, f.height * k)),
                align: *align,
                para,
                on_path: None,
            },
        },
        Command::TransformShapes {
            shapes: vec![s.id],
            transform: new_transform * s.transform.inverse(),
        },
    ];
    if let Some(st) = s.stroke.as_ref().filter(|st| st.scale_with_object) {
        let mut st = st.clone();
        st.width *= k;
        cmds.push(Command::SetStroke {
            shapes: vec![s.id],
            stroke: Some(st),
        });
    }
    Some(cmds)
}

#[cfg(test)]
mod tests {
    use crate::app::App;
    use graphdraw_core::geometry::{Affine, Point, Rect, Size};
    use graphdraw_core::{ShapeId, ShapeKind};

    fn text(app: &mut App, frame: Option<Size>) -> ShapeId {
        graphdraw_text::install();
        app.start_text(Point::new(20.0, 20.0), frame);
        for c in "Some words to scale".chars() {
            app.text_insert(&c.to_string());
        }
        let id = app.selection[0];
        app.finish_text();
        app.select(vec![id]);
        id
    }

    fn size_of(app: &App, id: ShapeId) -> f64 {
        match &app.doc().shape(id).expect("shape").1.kind {
            ShapeKind::Text { spans, .. } => spans[0].size_pt,
            _ => panic!("text"),
        }
    }

    fn near(a: Rect, b: Rect) -> bool {
        [a.x0 - b.x0, a.y0 - b.y0, a.x1 - b.x1, a.y1 - b.y1]
            .iter()
            .all(|d| d.abs() < 1e-3)
    }

    /// Twice the size about the selection's lower left corner.
    fn doubled(app: &App) -> (Affine, Rect) {
        let b = app.selection_bounds().expect("bounds");
        let at = Point::new(b.x0, b.y0).to_vec2();
        let t = Affine::translate(at) * Affine::scale(2.0) * Affine::translate(-at);
        (t, b)
    }

    #[test]
    fn scaling_artistic_text_writes_the_scale_into_its_size() {
        let mut app = App::headless();
        let id = text(&mut app, None);
        let before = size_of(&app, id);
        let (t, b) = doubled(&app);
        app.transform_selection(t);
        // 24 pt twice as large is 48 pt, the transform keeps no scale and
        // the letters sit where the scale put them.
        assert!((size_of(&app, id) - 2.0 * before).abs() < 1e-9);
        let s = app.doc().shape(id).expect("shape").1.clone();
        let [a, _, _, d, _, _] = s.transform.as_coeffs();
        assert!((a - 1.0).abs() < 1e-9 && (d - 1.0).abs() < 1e-9, "{:?}", s.transform);
        let want = Rect::new(b.x0, b.y0, b.x0 + 2.0 * b.width(), b.y0 + 2.0 * b.height());
        assert!(near(app.selection_bounds().expect("bounds"), want));
        // One undo step brings the old size back.
        app.undo();
        assert_eq!(size_of(&app, id), before);
        assert!(near(app.selection_bounds().expect("bounds"), b));
    }

    #[test]
    fn stretching_artistic_text_keeps_the_stretch_and_sizes_by_the_height() {
        let mut app = App::headless();
        let id = text(&mut app, None);
        let before = size_of(&app, id);
        // Three times as wide and half as tall.
        app.transform_selection(Affine::scale_non_uniform(3.0, 0.5));
        assert!((size_of(&app, id) - before / 2.0).abs() < 1e-9);
        let s = app.doc().shape(id).expect("shape").1.clone();
        let [a, _, _, d, _, _] = s.transform.as_coeffs();
        assert!((a - 6.0).abs() < 1e-9 && (d - 1.0).abs() < 1e-9, "{:?}", s.transform);
        // A turn changes no size.
        app.transform_selection(Affine::rotate(0.4));
        assert!((size_of(&app, id) - before / 2.0).abs() < 1e-9);
    }

    #[test]
    fn the_round_handle_shows_the_kind_and_a_double_click_converts() {
        let mut app = App::headless();
        let id = text(&mut app, None);
        app.set_tool(crate::tools::Tool::Pick);
        app.select(vec![id]);
        let (p, paragraph) = app.text_kind_handle().expect("handle");
        assert!(!paragraph);
        assert!(app.text_kind_handle_at(p));
        app.toggle_text_kind();
        assert!(app.text_kind_handle().expect("handle").1);
    }

    #[test]
    fn ctrl_scales_the_letters_of_paragraph_text_with_its_frame() {
        let mut app = App::headless();
        let id = text(&mut app, Some(Size::new(60.0, 30.0)));
        let before = size_of(&app, id);
        let (t, b) = doubled(&app);
        // Without Ctrl the frame grows and the letters stay.
        app.transform_selection(t);
        assert_eq!(size_of(&app, id), before);
        app.undo();
        // With Ctrl the letters and the frame double.
        app.scale_letters = true;
        app.transform_selection(t);
        app.scale_letters = false;
        assert!((size_of(&app, id) - 2.0 * before).abs() < 1e-9);
        match &app.doc().shape(id).expect("shape").1.kind {
            ShapeKind::Text { frame: Some(f), .. } => {
                assert!((f.width - 120.0).abs() < 1e-6 && (f.height - 60.0).abs() < 1e-6, "{f:?}")
            }
            other => panic!("not paragraph text: {other:?}"),
        }
        let want = Rect::new(b.x0, b.y0, b.x0 + 2.0 * b.width(), b.y0 + 2.0 * b.height());
        assert!(near(app.selection_bounds().expect("bounds"), want));
    }
}
