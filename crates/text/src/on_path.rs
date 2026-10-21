//! Artistic text laid along a path by arc length (see docs/behavior/text.md,
//! "Text on a path"): the alignment along the path (start, centre, end,
//! justified), the line of the letters that sits on the stroke, the side
//! of the stroke, the letters' orientation (turned, upright, skewed or
//! warped), the sharp-corner rule, anti-collision on concave stretches,
//! and further lines running parallel to the first, one leading apart.

use super::{letter_frame, Line};
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

/// The letters of `lines` placed along `tp`, and where they lie on it.
pub(crate) fn place(lines: &[Line], tp: &TextOnPath, align: TextAlign) -> (BezPath, Option<PathSpan>) {
    let mut out = BezPath::new();
    let Some(track) = Track::new(tp) else {
        return (out, None);
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
    for line in lines {
        let glyphs = &line.glyphs;
        let n = glyphs.len();
        if n == 0 {
            continue;
        }
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
        let first = positions.first().map_or(start, |p| p.0);
        let end = positions.last().map_or(start, |p| p.0) + glyphs[n - 1].advance;
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
    )
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
