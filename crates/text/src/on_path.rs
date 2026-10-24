//! Artistic text laid along a path by arc length (see docs/behavior/text.md,
//! "Text on a path"): the alignment along the path (start, centre, end,
//! justified), the line of the letters that sits on the stroke, the side
//! of the stroke, the letters' orientation (turned, upright, skewed or
//! warped), the sharp-corner rule, anti-collision on concave stretches,
//! and further lines running parallel to the first, one leading apart.

use super::{letter_frame, Line, LineBox};
use graphdraw_core::document::{
    PathTextCorners, PathTextOrientation, PathTextVertical, TextAlign, TextOnPath,
};
use graphdraw_core::geometry::{Affine, BezPath, PathEl, Point, Vec2};
use graphdraw_core::text_path::Track;

/// Where a text on a path lies along it (arc lengths of its track, in
/// reading order): where its letters start and end, and the track's
/// length. The canvas handles stand on it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PathSpan {
    pub start: f64,
    pub end: f64,
    pub total: f64,
}

/// Cap height as a share of the body (the side and the anti-collision
/// rule measure the letters by it).
const CAP: f64 = 0.72;

/// How far the baseline is moved for the line `v` of the letters to sit on
/// the stroke (along the letters' "up").
fn lift(v: PathTextVertical, size: f64, ascent: f64, descent: f64) -> f64 {
    match v {
        PathTextVertical::Baseline => 0.0,
        PathTextVertical::Ascender => -ascent,
        PathTextVertical::Descender => descent,
        PathTextVertical::Center => (descent - ascent) / 2.0,
        PathTextVertical::XHeight => -0.5 * size,
        PathTextVertical::CapHeight => -CAP * size,
    }
}

/// Where a caret stands on a text on a path: the point on its line's
/// baseline and the letters' "up" there (unit), in layout space.
pub type PathCaret = (Point, Vec2);

/// The letters of `lines` placed along `tp`, where they lie on it, and
/// for each line of `boxes` (one per line) where each of its character
/// edges falls on the path.
pub(crate) fn place(
    lines: &[Line],
    boxes: &[LineBox],
    tp: &TextOnPath,
    align: TextAlign,
) -> (BezPath, Option<PathSpan>, Vec<Vec<PathCaret>>) {
    let mut out = BezPath::new();
    let mut carets: Vec<Vec<PathCaret>> = Vec::with_capacity(lines.len());
    let Some(track) = Track::new(tp) else {
        return (out, None, carets);
    };
    let total = track.total();
    let flip = tp.flipped();
    // The letters' "up" is the left of the track, or its right when the
    // letters are flipped across it.
    let sgn = if flip { -1.0 } else { 1.0 };
    let flip_m = if flip {
        Affine::scale_non_uniform(1.0, -1.0)
    } else {
        Affine::IDENTITY
    };
    let y0 = lines.first().map_or(0.0, |l| l.y);
    let mut span: Option<(f64, f64)> = None;
    for (li, line) in lines.iter().enumerate() {
        let glyphs = &line.glyphs;
        let n = glyphs.len();
        // Pen position of each glyph along the line.
        let mut xs = Vec::with_capacity(n);
        let mut pen = 0.0;
        for g in glyphs {
            xs.push(pen);
            pen += g.advance;
        }
        let w = pen;
        let m = &line.metrics;
        let size = m.size;
        let justify = align.is_justified() && n > 1 && total > w;
        let extra = if justify {
            (total - w) / (n - 1) as f64
        } else {
            0.0
        };
        let start = match align {
            TextAlign::Center => (total - w) / 2.0,
            TextAlign::Right => total - w,
            _ => 0.0,
        } + tp.offset;
        let below = if tp.below { -CAP * size } else { 0.0 };
        // Distance of this line's baseline from the stroke, along "up":
        // the distance, the line of the letters on the stroke, the side
        // and the lines after the first (concentric).
        let off = tp.distance + lift(tp.vertical, size, m.ascent, m.descent) + below + (line.y - y0);
        let top = off + CAP * size;
        // Faces between corners (equal parts per side).
        let mut faces: Vec<(f64, f64)> = Vec::new();
        if tp.corners == PathTextCorners::Split {
            let mut a = 0.0;
            for c in track.corners.iter().copied().filter(|c| *c > 0.0 && *c < total) {
                faces.push((a, c));
                a = c;
            }
            faces.push((a, total));
        }
        let mut s = start;
        let mut positions: Vec<(f64, bool)> = Vec::with_capacity(n);
        for k in 0..n {
            let adv = glyphs[k].advance;
            let mut sc = s;
            let mut warp_here = tp.orientation == PathTextOrientation::Warp;
            match tp.corners {
                PathTextCorners::Split if faces.len() > 1 => {
                    let f = ((xs[k] + adv / 2.0) / w.max(1e-9)).clamp(0.0, 0.999_999);
                    let j = ((f * faces.len() as f64).floor() as usize).min(faces.len() - 1);
                    let local = f * faces.len() as f64 - j as f64;
                    let (fa, fb) = faces[j];
                    sc = fa + local * (fb - fa) - adv / 2.0;
                }
                PathTextCorners::Wrap => {
                    if track.corner_in(sc, sc + adv).is_some() {
                        warp_here = true;
                    }
                }
                PathTextCorners::Jump => {
                    // The letter that would fall on the corner goes whole
                    // on to the next side.
                    if let Some(c) = track.corner_in(sc, sc + adv) {
                        sc += (c - track.wrap(sc)).rem_euclid(total.max(1e-9));
                    }
                }
                _ => {}
            }
            positions.push((sc, warp_here));
            let step = if k + 1 < n { xs[k + 1] - xs[k] } else { adv };
            let mut next = sc + step + extra;
            if tp.relax && k + 1 < n && tp.corners != PathTextCorners::Split {
                // Concave: the tops come together; open the spacing by
                // the ratio between the width at the top and at the base.
                let kappa = track.curvature(sc + adv, (adv * 0.5).max(size * 0.1));
                let base = 1.0 - kappa * sgn * off;
                let ratio = if base.abs() > 1e-9 {
                    (1.0 - kappa * sgn * top) / base
                } else {
                    1.0
                };
                if ratio < 1.0 {
                    next += step * (1.0 / ratio.max(0.25) - 1.0);
                }
            }
            s = next;
        }
        carets.push(edge_carets(boxes.get(li), &xs, &positions, start, |arc| {
            let (p, dir) = track.at(arc);
            let nrm = Vec2::new(-dir.y, dir.x) * sgn;
            (p + nrm * off, nrm)
        }));
        let Some(last) = glyphs.last() else {
            continue;
        };
        let first = positions.first().map_or(start, |p| p.0);
        let end = positions.last().map_or(start, |p| p.0) + last.advance;
        span = Some(span.map_or((first, end), |(a, b)| (a.min(first), b.max(end))));
        for (k, g) in glyphs.iter().enumerate() {
            let (sc, warp_here) = positions[k];
            let gp = g.path.as_ref();
            if warp_here {
                // Every point of the letter goes onto the path: the base
                // follows the stroke, the top the outer arc.
                let map = |q: Point| {
                    let (pp, dir) = track.at(sc + q.x + g.x_offset);
                    let nrm = Vec2::new(-dir.y, dir.x) * sgn;
                    pp + nrm * (off + q.y + g.y_offset)
                };
                let tol = (size / 300.0).max(0.0002);
                graphdraw_core::geometry::flatten(gp, tol, &mut |el| match el {
                    PathEl::MoveTo(q) => out.move_to(map(q)),
                    PathEl::LineTo(q) => out.line_to(map(q)),
                    PathEl::ClosePath => out.close_path(),
                    _ => {}
                });
            } else {
                let (p, dir) = track.at(sc + g.advance / 2.0);
                let nrm = Vec2::new(-dir.y, dir.x) * sgn;
                let base = p + nrm * off;
                let placed = Affine::translate(base.to_vec2())
                    * letter_frame(tp.orientation, dir)
                    * flip_m
                    * Affine::translate((-g.advance / 2.0 + g.x_offset, g.y_offset));
                out.extend((placed * gp.clone()).elements().iter().copied());
            }
        }
    }
    (
        out,
        span.map(|(start, end)| PathSpan { start, end, total }),
        carets,
    )
}

/// Each character edge of a line (its pen x in the straight layout)
/// brought onto the path: the edge goes with the glyph it falls in, at
/// the same distance from that glyph's start as on the straight line.
/// `xs` are the glyphs' pen positions from the line start, `positions`
/// where each glyph starts along the path, `start` where the line starts
/// when it has no glyphs.
fn edge_carets(
    lb: Option<&LineBox>,
    xs: &[f64],
    positions: &[(f64, bool)],
    start: f64,
    at: impl Fn(f64) -> PathCaret,
) -> Vec<PathCaret> {
    let Some(lb) = lb else {
        return Vec::new();
    };
    let line_start = lb.edges.first().copied().unwrap_or(0.0);
    lb.edges
        .iter()
        .map(|&x| {
            let rel = x - line_start;
            let k = xs.iter().rposition(|&g| g <= rel + 1e-9).unwrap_or(0);
            let arc = match (xs.get(k), positions.get(k)) {
                (Some(&g), Some(&(sc, _))) => sc + (rel - g),
                _ => start + rel,
            };
            at(arc)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::{fonts, TextLayout};
    use graphdraw_core::document::{
        text_outline::TextRequest, ParagraphStyle, PathTextCorners, PathTextOrientation,
        PathTextVertical, TextAlign, TextOnPath, TextSpan,
    };
    use graphdraw_core::geometry::{BezPath, Shape as _};

    fn has_fonts() -> bool {
        !fonts().families().is_empty()
    }

    fn lay(text: &str, path: BezPath, align: TextAlign, edit: impl Fn(&mut TextOnPath)) -> TextLayout {
        let mut tp = TextOnPath::fitted(path, None);
        edit(&mut tp);
        let spans = [TextSpan::new(text, "DejaVu Sans", 24.0)];
        fonts().layout(&TextRequest {
            spans: &spans,
            frame: None,
            align,
            para: &ParagraphStyle::default(),
            on_path: Some(&tp),
        })
    }

    fn line(len: f64) -> BezPath {
        let mut p = BezPath::new();
        p.move_to((0.0, 0.0));
        p.line_to((len, 0.0));
        p
    }

    fn circle(r: f64) -> BezPath {
        graphdraw_core::geometry::Circle::new((0.0, 0.0), r).to_path(0.01)
    }

    #[test]
    fn alignment_along_the_path_justify_and_the_way_round_a_closed_one() {
        if !has_fonts() {
            return;
        }
        let span = |a| lay("ABC", line(200.0), a, |_| {}).path_span.expect("span");
        let s = span(TextAlign::Left);
        assert!(s.start.abs() < 1e-6 && (s.total - 200.0).abs() < 1e-6, "{s:?}");
        let c = span(TextAlign::Center);
        assert!(((c.start + c.end) / 2.0 - 100.0).abs() < 0.5, "{c:?}");
        let e = span(TextAlign::Right);
        assert!((e.end - 200.0).abs() < 0.5, "{e:?}");
        let j = span(TextAlign::Justify);
        assert!(j.start.abs() < 1e-6 && j.end > 199.0, "{j:?}");
        // A closed path: starting near its end the letters go on past its
        // start instead of piling up.
        let w = lay("ABCDEF", circle(50.0), TextAlign::Left, |p| p.offset = 300.0);
        let b = w.path.bounding_box();
        assert!(b.width() < 120.0 && b.width() > 1.0, "{b:?}");
    }

    #[test]
    fn carets_stand_on_the_path_across_the_letters() {
        if !has_fonts() {
            return;
        }
        // Along a straight path the carets fall where the straight layout
        // puts them.
        let l = lay("ABC", line(200.0), TextAlign::Left, |_| {});
        for i in 0..=3 {
            let (top, bottom) = l.path_caret(i).expect("caret");
            let (x, ..) = l.caret(i).expect("straight caret");
            assert!((top.x - x).abs() < 1e-6 && (bottom.x - x).abs() < 1e-6);
        }
        // Round a circle each caret stands on a radius, its foot on the
        // circle side, and a click at its middle finds it again.
        let c = lay("ABCDEF", circle(50.0), TextAlign::Left, |_| {});
        let mut feet = Vec::new();
        for i in 0..=6 {
            let (top, bottom) = c.path_caret(i).expect("caret");
            // The track is flattened: its tangents are a chord's, a few
            // hundredths of a radian off the true ones.
            let angle = top.to_vec2().angle() - bottom.to_vec2().angle();
            assert!(angle.sin().abs() < 0.03, "{i}: {top:?} {bottom:?}");
            let rt = top.to_vec2().hypot();
            let rb = bottom.to_vec2().hypot();
            let line = &c.lines[0];
            assert!(((rt - rb).abs() - (line.ascent + line.descent)).abs() < 0.05);
            let mid = top.midpoint(bottom);
            assert_eq!(c.hit_char(mid), Some(i));
            feet.push(bottom);
        }
        assert!(feet.windows(2).all(|w| (w[1] - w[0]).hypot() > 0.5), "{feet:?}");
        // Straight text has none.
        assert!(lay("ABC", BezPath::new(), TextAlign::Left, |_| {}).path_caret(0).is_none());
    }

    #[test]
    fn anti_collision_opens_concave_stretches_and_corners_follow_their_rule() {
        if !has_fonts() {
            return;
        }
        // Inside a circle (anticlockwise, the letters' tops towards the
        // centre): the tops come together, so the spacing opens.
        let len = |relax| {
            let s = lay("HHHHHH", circle(20.0), TextAlign::Left, |p| p.relax = relax)
                .path_span
                .expect("span");
            s.end - s.start
        };
        assert!(len(true) > len(false) + 0.1, "{} {}", len(true), len(false));
        // A corner at 100 mm: skipping it moves the letters on, never back.
        let mut l = BezPath::new();
        l.move_to((0.0, 0.0));
        l.line_to((100.0, 0.0));
        l.line_to((100.0, 100.0));
        let text = "WWWWWWWWWWWWWWWWWWWWWWWWWWW";
        let end = |c| {
            lay(text, l.clone(), TextAlign::Left, |p| {
                p.corners = c;
                p.relax = false
            })
            .path_span
            .expect("span")
            .end
        };
        assert!(end(PathTextCorners::Jump) >= end(PathTextCorners::Free) - 1e-6);
        // Equal parts per side: the text spreads over both sides.
        assert!(end(PathTextCorners::Split) > 150.0);
        // Warping bends the letters: the outline is no longer only turned.
        let count = |o| {
            lay("O", circle(30.0), TextAlign::Left, |p| p.orientation = o)
                .path
                .elements()
                .len()
        };
        assert!(count(PathTextOrientation::Warp) > count(PathTextOrientation::Rotate));
    }

    #[test]
    fn the_side_the_line_on_the_stroke_and_further_lines() {
        if !has_fonts() {
            return;
        }
        let b = |edit: &dyn Fn(&mut TextOnPath)| {
            lay("HH", line(100.0), TextAlign::Left, edit)
                .path
                .bounding_box()
        };
        let base = b(&|_| {});
        assert!(base.y0 > -0.05 && base.y1 > 4.0, "{base:?}");
        // Below: the capitals hang from the stroke, reading the same way.
        let below = b(&|p| p.below = true);
        assert!(below.y1 < 0.5 && below.y1 > -0.5, "{below:?}");
        assert!((below.x0 - base.x0).abs() < 1e-6);
        // The cap height and the x-height on the stroke.
        let cap = b(&|p| p.vertical = PathTextVertical::CapHeight);
        assert!(cap.y1.abs() < 0.6, "{cap:?}");
        let x = b(&|p| p.vertical = PathTextVertical::XHeight);
        assert!(x.y1 > cap.y1 && x.y1 < base.y1, "{x:?}");
        // A second line runs parallel below the first, a leading apart.
        let two = lay("HH\nHH", line(100.0), TextAlign::Left, |_| {});
        let t = two.path.bounding_box();
        assert!(t.y0 < -5.0 && t.x1 < base.x1 + 1e-6, "{t:?}");
    }
}
