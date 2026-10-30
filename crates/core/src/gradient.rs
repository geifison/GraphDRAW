//! Gradient (fountain) colours and geometry beyond the plain run of
//! stops: the size and axis of the span, elliptical and square radii, the
//! midpoint and opacity of each stop, steps, blending around the colour
//! wheel, and the handles the Interactive Fill tool drags on the canvas
//! (start, end, the second radius and the stops on the axis; a double
//! click on the axis adds a stop). Everything is in page space over the
//! object's bounds, as the renderer draws it. See
//! `docs/behavior/gradient-fill.md`.

use crate::color::{hsb_to_rgb, rgb_to_hsb, Color};
use crate::geometry::{Point, Rect, Vec2};
use crate::style::{lerp_color, Fill, Fountain, FountainBlend, FountainKind, FountainRepeat, Stop};

/// Most bands a stepped fountain draws.
pub const MAX_STEPS: u32 = 256;

/// Smallest span size (against the automatic one).
pub const MIN_SCALE: f64 = 0.01;

/// Largest span size (against the automatic one).
pub const MAX_SCALE: f64 = 100.0;

/// Where, in 0..1 of the run between two stops, the colour is taken when
/// the midpoint is at `m`: `k` itself at 0.5, else `k^(ln 0.5 / ln m)`, so
/// that `m` maps to 0.5.
pub fn bias(k: f64, m: f64) -> f64 {
    let m = if m.is_finite() { m.clamp(0.01, 0.99) } else { 0.5 };
    let k = if k.is_finite() { k.clamp(0.0, 1.0) } else { 0.0 };
    if (m - 0.5).abs() < 1e-9 {
        return k;
    }
    k.powf(0.5f64.ln() / m.ln())
}

/// `a` to `b` at `t` around the colour wheel: hue increasing (`clockwise`)
/// or decreasing, saturation and brightness straight.
pub fn lerp_hue(a: Color, b: Color, t: f64, clockwise: bool) -> Color {
    let [r1, g1, b1] = a.to_rgb_f32();
    let [r2, g2, b2] = b.to_rgb_f32();
    let (h1, s1, v1) = rgb_to_hsb(r1, g1, b1);
    let (h2, s2, v2) = rgb_to_hsb(r2, g2, b2);
    let mut dh = h2 - h1;
    if clockwise && dh < 0.0 {
        dh += 360.0;
    } else if !clockwise && dh > 0.0 {
        dh -= 360.0;
    }
    let t = t.clamp(0.0, 1.0) as f32;
    let [r, g, bb] = hsb_to_rgb(h1 + dh * t, s1 + (s2 - s1) * t, v1 + (v2 - v1) * t);
    Color::Rgb { r, g, b: bb }
}

impl Fountain {
    /// Size, axis offset, midpoints, opacity, steps or blending differ
    /// from the plain run of stops.
    pub fn has_extras(&self) -> bool {
        self.has_span_extras() || self.has_run_extras()
    }

    /// The span is not the automatic one (size or axis offset).
    pub fn has_span_extras(&self) -> bool {
        self.scale != 1.0 || self.axis_offset != 0.0
    }

    /// The colours between the stops are not a straight mix (midpoints,
    /// steps, blending) or a stop is not opaque.
    pub fn has_run_extras(&self) -> bool {
        self.steps > 0
            || self.blend != FountainBlend::Direct
            || self
                .stops
                .iter()
                .any(|s| s.midpoint != 0.5 || s.opacity != 1.0)
    }

    /// The span size, kept in [`MIN_SCALE`]..[`MAX_SCALE`].
    pub fn span_scale(&self) -> f64 {
        if self.scale.is_finite() {
            self.scale.clamp(MIN_SCALE, MAX_SCALE)
        } else {
            1.0
        }
    }

    /// The y radius over the x radius, kept in 0.01..100.
    pub fn span_aspect(&self) -> f64 {
        if self.aspect.is_finite() {
            self.aspect.clamp(0.01, 100.0)
        } else {
            1.0
        }
    }

    /// Where on the span (0..1) the automatic parameter `t` falls (the
    /// parameter over the bounds-fitted span: 0 at the linear start or
    /// the centre, 1 at the end or the automatic radius). With a size or
    /// an axis offset, a repeating fountain keeps repeating beyond the
    /// span; otherwise the end colours hold.
    pub fn span_param(&self, t: f64) -> f64 {
        let t = if t.is_finite() { t } else { 0.0 };
        let s = self.span_scale();
        let x = match self.kind {
            FountainKind::Linear => {
                let o = if self.axis_offset.is_finite() {
                    self.axis_offset
                } else {
                    0.0
                };
                (2.0 * t - 1.0 - o + s) / (2.0 * s)
            }
            FountainKind::Radial | FountainKind::Elliptical | FountainKind::Square => t / s,
            FountainKind::Conical | FountainKind::Shaped => t,
        };
        if (0.0..=1.0).contains(&x) || !self.has_span_extras() {
            return x.clamp(0.0, 1.0);
        }
        match self.repeat {
            FountainRepeat::None => x.clamp(0.0, 1.0),
            FountainRepeat::Repeat => x - x.floor(),
            FountainRepeat::Mirror => {
                let m = x - 2.0 * (x / 2.0).floor();
                if m > 1.0 {
                    2.0 - m
                } else {
                    m
                }
            }
        }
    }

    /// Colour and opacity of the stops at `s` (0..1 along the stops): the
    /// steps, then the two stops around it mixed by the first one's
    /// midpoint and the blending.
    pub fn sample_run(&self, s: f64) -> (Color, f64) {
        let stops = self.sorted_stops();
        let mut s = if s.is_finite() { s } else { 0.0 };
        let n = self.steps.min(MAX_STEPS);
        if n >= 1 {
            let n = n as f64;
            s = (((s * n).floor() + 0.5) / n).clamp(0.0, 1.0);
        }
        let (Some(first), Some(last)) = (stops.first(), stops.last()) else {
            return (Color::BLACK, 1.0);
        };
        if s <= first.pos {
            return (first.color, first.opacity.clamp(0.0, 1.0));
        }
        for w in stops.windows(2) {
            let (a, b) = (w[0], w[1]);
            if s <= b.pos {
                let span = (b.pos - a.pos).max(1e-9);
                let k = bias((s - a.pos) / span, a.midpoint);
                let c = match self.blend {
                    FountainBlend::Direct => lerp_color(a.color, b.color, k as f32),
                    FountainBlend::Clockwise => lerp_hue(a.color, b.color, k, true),
                    FountainBlend::Counterclockwise => lerp_hue(a.color, b.color, k, false),
                };
                let o = a.opacity + (b.opacity - a.opacity) * k;
                return (c, o.clamp(0.0, 1.0));
            }
        }
        (last.color, last.opacity.clamp(0.0, 1.0))
    }

    /// Colour and opacity at the automatic parameter `t` (see
    /// [`Fountain::span_param`]).
    pub fn color_alpha_at(&self, t: f64) -> (Color, f64) {
        self.sample_run(self.stop_param(self.span_param(t)))
    }

    /// Opacity at the automatic parameter `t`.
    pub fn alpha_at(&self, t: f64) -> f64 {
        self.color_alpha_at(t).1
    }

    /// [`Fountain::resolved_stops`] for a fountain with extras: the
    /// colours sampled across 0..1, the points a straight mix would give
    /// within half a level dropped.
    pub(crate) fn sampled_stops(&self) -> Vec<Stop> {
        const N: usize = 512;
        let mut pts: Vec<Stop> = (0..=N)
            .map(|i| {
                let t = i as f64 / N as f64;
                let (color, opacity) = self.color_alpha_at(t);
                Stop {
                    opacity,
                    ..Stop::new(t, color)
                }
            })
            .collect();
        // Hard edges (steps, sawtooth jumps): found between samples by
        // halving the gap.
        let mut out: Vec<Stop> = Vec::with_capacity(pts.len());
        for w in pts.windows(2) {
            let (a, b) = (w[0], w[1]);
            out.push(a);
            if far(&a, &b, 24.0) {
                let (mut lo, mut hi) = (a.pos, b.pos);
                let ca = self.color_alpha_at(lo);
                for _ in 0..24 {
                    let mid = (lo + hi) / 2.0;
                    let cm = self.color_alpha_at(mid);
                    if far_c(ca, cm, 24.0) {
                        hi = mid;
                    } else {
                        lo = mid;
                    }
                }
                let (cl, ol) = self.color_alpha_at(lo);
                let (ch, oh) = self.color_alpha_at(hi);
                out.push(Stop {
                    opacity: ol,
                    ..Stop::new(lo, cl)
                });
                out.push(Stop {
                    opacity: oh,
                    ..Stop::new(lo, ch)
                });
            }
        }
        if let Some(l) = pts.pop() {
            out.push(l);
        }
        // Drop the points the neighbours give anyway.
        let mut kept: Vec<Stop> = Vec::with_capacity(out.len());
        for (i, s) in out.iter().enumerate() {
            let Some(prev) = kept.last().copied() else {
                kept.push(*s);
                continue;
            };
            let Some(next) = out.get(i + 1) else {
                kept.push(*s);
                continue;
            };
            let span = next.pos - prev.pos;
            if span <= 1e-12 || (s.pos - prev.pos) <= 1e-12 || (next.pos - s.pos) <= 1e-12 {
                kept.push(*s);
                continue;
            }
            let k = ((s.pos - prev.pos) / span) as f32;
            let mix = lerp_color(prev.color, next.color, k);
            let o = prev.opacity + (next.opacity - prev.opacity) * k as f64;
            if far_c((mix, o), (s.color, s.opacity), 0.6) {
                kept.push(*s);
            }
        }
        kept
    }

    /// The bounds' centre moved by the centre offset.
    pub fn center_in(&self, b: Rect) -> Point {
        let c = b.center();
        Point::new(
            c.x + self.offset.x * b.width() / 2.0,
            c.y + self.offset.y * b.height() / 2.0,
        )
    }

    /// Half the length of the automatic linear axis at the fountain's angle.
    pub fn auto_half(&self, b: Rect) -> f64 {
        let a = self.angle.to_radians();
        (b.width() * a.cos().abs() + b.height() * a.sin().abs()) / 2.0
    }

    /// The linear axis on the bounds, start to end.
    pub fn linear_axis(&self, b: Rect) -> (Point, Point) {
        let a = self.angle.to_radians();
        let u = Vec2::new(a.cos(), a.sin());
        let h = self.auto_half(b);
        let s = self.span_scale();
        let o = if self.axis_offset.is_finite() {
            self.axis_offset
        } else {
            0.0
        };
        let c = b.center();
        (c + u * (h * (o - s)), c + u * (h * (o + s)))
    }

    /// The automatic radii (x along the angle, y across it) of a centred
    /// kind, before the size.
    pub fn auto_radii(&self, b: Rect) -> (f64, f64) {
        let r2 = std::f64::consts::SQRT_2;
        match self.kind {
            FountainKind::Elliptical => (
                b.width() / 2.0 * r2,
                b.height() / 2.0 * r2 * self.span_aspect(),
            ),
            FountainKind::Square => (
                b.width() / 2.0,
                b.height() / 2.0 * self.span_aspect(),
            ),
            _ => {
                let r = b.width().max(b.height()) / 2.0 * r2;
                (r, r)
            }
        }
    }

    /// The automatic parameter at `(u, v)`, 0..1 across the bounds of
    /// size `w` x `h` (v up), for every kind but Shaped (0 then): the
    /// projection on the automatic linear axis, the distance from the
    /// centre over the automatic radius, the turn about the centre
    /// (conical, mirrored), or the larger side distance (square).
    pub fn raw_param_uv(&self, u: f64, v: f64, w: f64, h: f64) -> f64 {
        match self.kind {
            FountainKind::Linear => {
                let a = self.angle.to_radians();
                let (x, y) = ((u - 0.5) * w, (v - 0.5) * h);
                let half = (w * a.cos().abs() + h * a.sin().abs()) / 2.0;
                if half <= 1e-12 {
                    return 0.5;
                }
                (x * a.cos() + y * a.sin()) / (2.0 * half) + 0.5
            }
            FountainKind::Radial => {
                let x = (u - 0.5 - self.offset.x / 2.0) * w;
                let y = (v - 0.5 - self.offset.y / 2.0) * h;
                let r = w.max(h) / 2.0 * std::f64::consts::SQRT_2;
                if r <= 1e-12 {
                    return 0.0;
                }
                x.hypot(y) / r
            }
            FountainKind::Elliptical => {
                let x = (u - 0.5 - self.offset.x / 2.0) * w;
                let y = (v - 0.5 - self.offset.y / 2.0) * h;
                let a = self.angle.to_radians();
                let (ca, sa) = (a.cos(), a.sin());
                let (p, q) = (x * ca + y * sa, -x * sa + y * ca);
                let r2 = std::f64::consts::SQRT_2;
                let rx = (w / 2.0 * r2).max(1e-12);
                let ry = (h / 2.0 * r2 * self.span_aspect()).max(1e-12);
                ((p / rx).powi(2) + (q / ry).powi(2)).sqrt()
            }
            FountainKind::Conical => {
                let dx = u - 0.5 - self.offset.x / 2.0;
                let dy = v - 0.5 - self.offset.y / 2.0;
                let a = dy.atan2(dx) - self.angle.to_radians();
                let a = a.rem_euclid(std::f64::consts::TAU) / std::f64::consts::TAU;
                if a < 0.5 {
                    a * 2.0
                } else {
                    2.0 - a * 2.0
                }
            }
            FountainKind::Square => {
                let dx = u - 0.5 - self.offset.x / 2.0;
                let dy = v - 0.5 - self.offset.y / 2.0;
                dx.abs().max(dy.abs() / self.span_aspect()) * 2.0
            }
            FountainKind::Shaped => 0.0,
        }
    }
}

fn far(a: &Stop, b: &Stop, levels: f64) -> bool {
    far_c((a.color, a.opacity), (b.color, b.opacity), levels)
}

fn far_c(a: (Color, f64), b: (Color, f64), levels: f64) -> bool {
    let (x, y) = (a.0.to_rgb_f32(), b.0.to_rgb_f32());
    let d = (0..3)
        .map(|i| (x[i] - y[i]).abs() as f64)
        .fold((a.1 - b.1).abs(), f64::max);
    d * 255.0 > levels
}

/// A handle the Interactive Fill tool drags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Handle {
    /// The linear start, or the centre of the other kinds.
    Start,
    /// The linear end, or the end of the radius (x radius) of the others.
    End,
    /// The y radius of elliptical and square fountains.
    RadiusY,
    /// A colour stop on the axis, by its index in `stops`.
    Stop(usize),
}

/// What a handle drag is constrained to.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DragMods {
    /// Angles in 15 degree steps.
    pub snap_angle: bool,
    /// A linear end turns the axis about the bounds' centre (length kept)
    /// instead of moving that end alone.
    pub rotate: bool,
}

fn unit(deg: f64) -> Vec2 {
    let a = deg.to_radians();
    Vec2::new(a.cos(), a.sin())
}

fn snap15(deg: f64) -> f64 {
    (deg / 15.0).round() * 15.0
}

/// The axis the stops sit on: start to end (linear), centre to the end of
/// the radius (the others); none for a shaped fountain.
pub fn axis(f: &Fountain, b: Rect) -> Option<(Point, Point)> {
    match f.kind {
        FountainKind::Shaped => None,
        FountainKind::Linear => Some(f.linear_axis(b)),
        FountainKind::Radial | FountainKind::Conical => {
            let c = f.center_in(b);
            let (r, _) = f.auto_radii(b);
            let r = if f.kind == FountainKind::Radial {
                r * f.span_scale()
            } else {
                r
            };
            Some((c, c + unit(f.angle) * r))
        }
        FountainKind::Elliptical => {
            let c = f.center_in(b);
            let (rx, _) = f.auto_radii(b);
            Some((c, c + unit(f.angle) * (rx * f.span_scale())))
        }
        FountainKind::Square => {
            let c = f.center_in(b);
            let (rx, _) = f.auto_radii(b);
            Some((c, c + Vec2::new(rx * f.span_scale(), 0.0)))
        }
    }
}

/// Where the stop at `pos` sits on the axis (0..1): the first cycle of a
/// repeating fountain, the first half of a bilinear one, inside the edge
/// pad, reversed when inverted.
pub fn axis_t_of_stop(f: &Fountain, pos: f64) -> f64 {
    let mut p = if pos.is_finite() {
        pos.clamp(0.0, 1.0)
    } else {
        0.0
    };
    if f.invert {
        p = 1.0 - p;
    }
    let e = f.edge_pad.clamp(0.0, 0.49);
    let mut t = e + p * (1.0 - 2.0 * e);
    t /= f.cycles() as f64;
    if f.bilinear {
        t /= 2.0;
    }
    t
}

/// The stop position at `t` (0..1) on the axis; the inverse of
/// [`axis_t_of_stop`].
pub fn stop_of_axis_t(f: &Fountain, t: f64) -> f64 {
    let mut u = if t.is_finite() { t.clamp(0.0, 1.0) } else { 0.0 };
    if f.bilinear {
        u *= 2.0;
    }
    u = (u * f.cycles() as f64).clamp(0.0, 1.0);
    let e = f.edge_pad.clamp(0.0, 0.49);
    let mut p = ((u - e) / (1.0 - 2.0 * e)).clamp(0.0, 1.0);
    if f.invert {
        p = 1.0 - p;
    }
    p
}

/// The handles on the bounds, in the order a click tries them: start and
/// end first, the y radius, then the stops (the end stops lie under the
/// start and end).
pub fn handles(f: &Fountain, b: Rect) -> Vec<(Handle, Point)> {
    let Some((a, z)) = axis(f, b) else {
        return Vec::new();
    };
    let mut v = vec![(Handle::Start, a), (Handle::End, z)];
    if matches!(f.kind, FountainKind::Elliptical | FountainKind::Square) {
        let (_, ry) = f.auto_radii(b);
        let ry = ry * f.span_scale();
        let d = if f.kind == FountainKind::Square {
            Vec2::new(0.0, ry)
        } else {
            unit(f.angle + 90.0) * ry
        };
        v.push((Handle::RadiusY, a + d));
    }
    for (i, s) in f.stops.iter().enumerate() {
        v.push((Handle::Stop(i), a.lerp(z, axis_t_of_stop(f, s.pos))));
    }
    v
}

/// The parameter (0..1) of the point of the axis nearest `p`.
pub fn project(f: &Fountain, b: Rect, p: Point) -> Option<f64> {
    let (a, z) = axis(f, b)?;
    let d = z - a;
    let len2 = d.hypot2();
    if len2 <= 1e-18 {
        return None;
    }
    Some(((p - a).dot(d) / len2).clamp(0.0, 1.0))
}

/// The distance from `p` to the axis segment.
pub fn distance_to_axis(f: &Fountain, b: Rect, p: Point) -> Option<f64> {
    let (a, z) = axis(f, b)?;
    let t = project(f, b, p)?;
    Some((p - a.lerp(z, t)).hypot())
}

/// A linear fountain running from `a` to `z` on the bounds `b` (its
/// angle, size and axis offset); unchanged when the points meet.
pub fn set_linear_ends(f: &mut Fountain, b: Rect, a: Point, z: Point) {
    let d = z - a;
    let len = d.hypot();
    if !(len > 1e-9 && len.is_finite()) {
        return;
    }
    f.angle = d.atan2().to_degrees();
    let h = f.auto_half(b);
    if h <= 1e-9 {
        return;
    }
    f.scale = (len / (2.0 * h)).clamp(MIN_SCALE, MAX_SCALE);
    let mid = a.midpoint(z);
    f.axis_offset = (mid - b.center()).dot(unit(f.angle)) / h;
}

/// The fountain with handle `h` moved to `p` on the bounds `b`. Stops
/// slide on the axis; their order is kept during the drag (see
/// [`sort_stops`] for the end of it).
pub fn drag(f: &Fountain, b: Rect, h: Handle, p: Point, mods: DragMods) -> Fountain {
    let mut g = f.clone();
    let c = f.center_in(b);
    match (f.kind, h) {
        (FountainKind::Shaped, _) => {}
        (FountainKind::Linear, Handle::Start | Handle::End) => {
            let (a, z) = f.linear_axis(b);
            if mods.rotate {
                // About the bounds' centre: only the angle changes.
                let v = if h == Handle::End {
                    p - b.center()
                } else {
                    b.center() - p
                };
                if v.hypot() > 1e-9 {
                    let deg = v.atan2().to_degrees();
                    g.angle = if mods.snap_angle { snap15(deg) } else { deg };
                }
                return g;
            }
            let (fixed, mut moved) = if h == Handle::End { (a, p) } else { (z, p) };
            if mods.snap_angle {
                let d = moved - fixed;
                moved = fixed + unit(snap15(d.atan2().to_degrees())) * d.hypot();
            }
            if h == Handle::End {
                set_linear_ends(&mut g, b, fixed, moved);
            } else {
                set_linear_ends(&mut g, b, moved, fixed);
            }
        }
        (_, Handle::Start) => {
            if b.width() > 1e-9 && b.height() > 1e-9 {
                let bc = b.center();
                g.offset = Point::new(
                    ((p.x - bc.x) / (b.width() / 2.0)).clamp(-1.0, 1.0),
                    ((p.y - bc.y) / (b.height() / 2.0)).clamp(-1.0, 1.0),
                );
            }
        }
        (kind, Handle::End) => {
            let v = p - c;
            let len = v.hypot();
            if len <= 1e-9 {
                return g;
            }
            let deg = v.atan2().to_degrees();
            let deg = if mods.snap_angle { snap15(deg) } else { deg };
            let (rx, _) = f.auto_radii(b);
            match kind {
                FountainKind::Conical => g.angle = deg,
                FountainKind::Elliptical => {
                    g.angle = deg;
                    if rx > 1e-9 {
                        g.scale = (len / rx).clamp(MIN_SCALE, MAX_SCALE);
                    }
                }
                FountainKind::Square => {
                    if rx > 1e-9 {
                        g.scale = (v.x.abs().max(1e-9) / rx).clamp(MIN_SCALE, MAX_SCALE);
                    }
                }
                _ => {
                    if rx > 1e-9 {
                        g.scale = (len / rx).clamp(MIN_SCALE, MAX_SCALE);
                    }
                }
            }
        }
        (kind, Handle::RadiusY) => {
            let v = p - c;
            let d = if kind == FountainKind::Square {
                v.y.abs()
            } else {
                v.dot(unit(f.angle + 90.0)).abs()
            };
            let mut base = f.clone();
            base.aspect = 1.0;
            let (_, ry1) = base.auto_radii(b);
            let ry1 = ry1 * f.span_scale();
            if ry1 > 1e-9 && matches!(kind, FountainKind::Elliptical | FountainKind::Square) {
                g.aspect = (d / ry1).clamp(0.01, 100.0);
            }
        }
        (_, Handle::Stop(i)) => {
            if let (Some(t), true) = (project(f, b, p), i < g.stops.len()) {
                g.stops[i].pos = stop_of_axis_t(f, t);
            }
        }
    }
    g
}

/// A new stop at the point of the axis nearest `p`, with the colour and
/// opacity the stops already give there; its index.
pub fn add_stop(f: &Fountain, b: Rect, p: Point) -> Option<(Fountain, usize)> {
    let t = project(f, b, p)?;
    let pos = stop_of_axis_t(f, t);
    Some(insert_stop(f, pos, None))
}

/// A new stop at `pos` (0..1), with `color` or the colour the stops give
/// there; its index.
pub fn insert_stop(f: &Fountain, pos: f64, color: Option<Color>) -> (Fountain, usize) {
    let pos = if pos.is_finite() {
        pos.clamp(0.0, 1.0)
    } else {
        0.5
    };
    let (c, o) = f.sample_run(pos);
    let mut g = f.clone();
    let i = g.stops.iter().filter(|s| s.pos <= pos).count();
    g.stops.insert(
        i,
        Stop {
            opacity: o,
            ..Stop::new(pos, color.unwrap_or(c))
        },
    );
    (g, i)
}

/// The fountain without stop `i`; never fewer than two stops.
pub fn remove_stop(f: &Fountain, i: usize) -> Option<Fountain> {
    if f.stops.len() <= 2 || i >= f.stops.len() {
        return None;
    }
    let mut g = f.clone();
    g.stops.remove(i);
    Some(g)
}

/// Put the stops in order; the new index of `keep`.
pub fn sort_stops(f: &mut Fountain, keep: Option<usize>) -> Option<usize> {
    let mut idx: Vec<usize> = (0..f.stops.len()).collect();
    idx.sort_by(|a, b| f.stops[*a].pos.total_cmp(&f.stops[*b].pos));
    f.stops = idx.iter().map(|i| f.stops[*i]).collect();
    keep.and_then(|k| idx.iter().position(|i| *i == k))
}

/// The stops in reverse order: positions mirrored, each run's midpoint
/// mirrored with it.
pub fn reverse_stops(f: &mut Fountain) {
    let mut v = f.sorted_stops();
    let n = v.len();
    let old = v.clone();
    for (j, s) in v.iter_mut().enumerate() {
        let k = n - 1 - j;
        *s = old[k];
        s.pos = 1.0 - old[k].pos;
        s.midpoint = if k > 0 { 1.0 - old[k - 1].midpoint } else { 0.5 };
    }
    f.stops = v;
}

/// A new linear fountain from `a` to `z` on the bounds `b`. The stops and
/// the other options come from the current fill when it is a fountain;
/// a solid colour runs to white; anything else is `fallback` to white.
pub fn linear_from(a: Point, z: Point, b: Rect, current: &Fill, fallback: Color) -> Fountain {
    let mut f = match current {
        Fill::Fountain(g) if g.stops.len() >= 2 => Fountain {
            kind: FountainKind::Linear,
            ..g.clone()
        },
        Fill::Solid(c) => Fountain::two(FountainKind::Linear, *c, Color::WHITE, 0.0),
        _ => Fountain::two(FountainKind::Linear, fallback, Color::WHITE, 0.0),
    };
    f.offset = Point::ZERO;
    set_linear_ends(&mut f, b, a, z);
    f
}

/// A new fountain of `kind` dragged from `a` to `z` on the bounds `b`:
/// linear runs from `a` to `z`; the others are centred on `a` with `z`
/// at the end of their radius. Stops and options as [`linear_from`].
pub fn fountain_from(
    kind: FountainKind,
    a: Point,
    z: Point,
    b: Rect,
    current: &Fill,
    fallback: Color,
) -> Fountain {
    let mut f = linear_from(a, z, b, current, fallback);
    if kind == FountainKind::Linear || kind == FountainKind::Shaped {
        f.kind = kind;
        return f;
    }
    f.kind = kind;
    f.angle = 0.0;
    f.scale = 1.0;
    f.axis_offset = 0.0;
    f.aspect = 1.0;
    let mods = DragMods::default();
    let f = drag(&f, b, Handle::Start, a, mods);
    drag(&f, b, Handle::End, z, mods)
}

/// The index of the stop at the start (`end` false) or the end of the run.
pub fn end_stop(f: &Fountain, end: bool) -> Option<usize> {
    (0..f.stops.len()).reduce(|a, b| {
        let (pa, pb) = (f.stops[a].pos, f.stops[b].pos);
        if (pb > pa) == end {
            b
        } else {
            a
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bw() -> Fountain {
        Fountain::two(FountainKind::Linear, Color::BLACK, Color::WHITE, 0.0)
    }

    fn rect() -> Rect {
        Rect::new(0.0, 0.0, 100.0, 40.0)
    }

    fn near(a: Point, b: Point) -> bool {
        (a - b).hypot() < 1e-6
    }

    /// A drag makes the chosen kind: radial centred on the press point
    /// with its radius reaching the release point, linear between them.
    #[test]
    fn dragged_fountain_keeps_its_kind() {
        let b = rect();
        let (a, z) = (Point::new(30.0, 20.0), Point::new(60.0, 20.0));
        let cur = Fill::Fountain(bw());
        for kind in [FountainKind::Linear, FountainKind::Radial] {
            let f = fountain_from(kind, a, z, b, &cur, Color::BLACK);
            assert_eq!(f.kind, kind);
            let (s, e) = axis(&f, b).unwrap();
            assert!(near(s, a) && near(e, z), "{kind:?}: {s:?} {e:?}");
            assert_eq!(f.stops.len(), 2);
        }
    }

    /// GD-FIL-11: drag the start, the end and the stops; a double click
    /// on the axis adds a stop with the colour already there.
    #[test]
    fn drag_ends_and_stops_and_add_stops() {
        let b = rect();
        let g = bw();
        let h = handles(&g, b);
        assert_eq!(h[0].0, Handle::Start);
        assert!(near(h[0].1, Point::new(0.0, 20.0)), "{:?}", h[0].1);
        assert!(near(h[1].1, Point::new(100.0, 20.0)));
        // The end alone moves: the start stays where it was.
        let g = drag(&g, b, Handle::End, Point::new(50.0, 20.0), DragMods::default());
        let (a, z) = g.linear_axis(b);
        assert!(near(a, Point::new(0.0, 20.0)) && near(z, Point::new(50.0, 20.0)), "{a:?} {z:?}");
        assert!((g.scale - 0.5).abs() < 1e-9 && (g.axis_offset + 0.5).abs() < 1e-9);
        // The colour follows the new span: grey at 25, white beyond 50.
        let gray = |g: &Fountain, x: f64| g.color_alpha_at(x / 100.0).0.to_rgb8()[0];
        assert!((gray(&g, 25.0) as i32 - 128).abs() <= 1, "{}", gray(&g, 25.0));
        assert_eq!(gray(&g, 75.0), 255);
        // A new stop in the middle of the axis: middle grey.
        let (g, i) = add_stop(&g, b, Point::new(25.0, 30.0)).expect("on the axis");
        assert_eq!((i, g.stops.len()), (1, 3));
        assert!((g.stops[1].pos - 0.5).abs() < 1e-9);
        assert!((g.stops[1].color.to_rgb8()[0] as i32 - 128).abs() <= 1);
        // A stop slides only to the end of the axis; sorting gives its index.
        let slid = drag(&g, b, Handle::Stop(1), Point::new(500.0, 0.0), DragMods::default());
        assert_eq!(slid.stops[1].pos, 1.0);
        let mut g2 = drag(&g, b, Handle::Stop(0), Point::new(37.5, 0.0), DragMods::default());
        assert_eq!(sort_stops(&mut g2, Some(0)), Some(1), "the first passed the middle one");
        assert!((g2.stops[1].pos - 0.75).abs() < 1e-9);
        // Never fewer than two stops.
        let g = remove_stop(&g, 1).expect("removed");
        assert!(remove_stop(&g, 0).is_none());
    }

    #[test]
    fn rotate_and_snap_the_linear_ends() {
        let b = rect();
        let g = drag(
            &bw(),
            b,
            Handle::End,
            Point::new(50.0, 60.0),
            DragMods {
                rotate: true,
                snap_angle: false,
            },
        );
        assert!((g.angle - 90.0).abs() < 1e-9 && g.scale == 1.0);
        let g = drag(
            &bw(),
            b,
            Handle::End,
            Point::new(100.0, 26.0),
            DragMods {
                snap_angle: true,
                rotate: false,
            },
        );
        assert!(g.angle.abs() < 1e-9, "{}", g.angle);
    }

    #[test]
    fn centred_handles_set_centre_radius_and_aspect() {
        let b = rect();
        let mut g = Fountain::two(FountainKind::Radial, Color::BLACK, Color::WHITE, 0.0);
        let g1 = drag(&g, b, Handle::Start, Point::new(75.0, 30.0), DragMods::default());
        assert!((g1.offset.x - 0.5).abs() < 1e-9 && (g1.offset.y - 0.5).abs() < 1e-9);
        let (_, r) = g.auto_radii(b);
        let g2 = drag(&g, b, Handle::End, Point::new(50.0 + r / 2.0, 20.0), DragMods::default());
        assert!((g2.scale - 0.5).abs() < 1e-9);
        // The radius point sits at the new radius.
        let (_, e) = axis(&g2, b).expect("axis");
        assert!(near(e, Point::new(50.0 + r / 2.0, 20.0)));
        g.kind = FountainKind::Elliptical;
        let hs = handles(&g, b);
        assert_eq!(hs[2].0, Handle::RadiusY);
        let (_, ry) = g.auto_radii(b);
        let g3 = drag(&g, b, Handle::RadiusY, Point::new(50.0, 20.0 + ry / 2.0), DragMods::default());
        assert!((g3.aspect - 0.5).abs() < 1e-9, "{}", g3.aspect);
        let g4 = drag(&g, b, Handle::End, Point::new(50.0, 80.0), DragMods::default());
        assert!((g4.angle - 90.0).abs() < 1e-9);
        // Shaped: nothing to drag.
        let s = Fountain::two(FountainKind::Shaped, Color::BLACK, Color::WHITE, 0.0);
        assert!(handles(&s, b).is_empty());
    }

    #[test]
    fn stops_sit_inside_the_edge_pad_and_reverse_when_inverted() {
        let mut g = bw();
        g.edge_pad = 0.25;
        assert!((axis_t_of_stop(&g, 0.0) - 0.25).abs() < 1e-9);
        assert!((stop_of_axis_t(&g, 0.5) - 0.5).abs() < 1e-9);
        g.invert = true;
        assert!((axis_t_of_stop(&g, 0.0) - 0.75).abs() < 1e-9);
        for p in [0.0, 0.3, 1.0] {
            assert!((stop_of_axis_t(&g, axis_t_of_stop(&g, p)) - p).abs() < 1e-9);
        }
    }

    /// GD-FIL-03: midpoint, steps, hue blending and opacity.
    #[test]
    fn midpoint_steps_blend_and_opacity() {
        let mut g = bw();
        let gray = |g: &Fountain, t: f64| g.color_alpha_at(t).0.to_rgb_f32()[0];
        assert!((gray(&g, 0.5) - 0.5).abs() < 0.01);
        g.stops[0].midpoint = 0.2;
        assert!((gray(&g, 0.2) - 0.5).abs() < 0.01, "half and half at 20%");
        g.stops[0].midpoint = 0.5;
        g.steps = 4;
        assert_eq!(gray(&g, 0.01), gray(&g, 0.24), "one colour per band");
        assert!(gray(&g, 0.26) > gray(&g, 0.24));
        g.steps = 0;
        // Red to blue: clockwise through green, counter-clockwise through magenta.
        let mut h = Fountain::two(
            FountainKind::Linear,
            Color::rgb8(255, 0, 0),
            Color::rgb8(0, 0, 255),
            0.0,
        );
        h.blend = FountainBlend::Clockwise;
        let [_, gc, _] = h.color_alpha_at(0.5).0.to_rgb8();
        assert!(gc > 200, "{gc}");
        h.blend = FountainBlend::Counterclockwise;
        let [r, gc, b] = h.color_alpha_at(0.5).0.to_rgb8();
        assert!(gc < 30 && r > 200 && b > 200, "{r} {gc} {b}");
        g.stops[1].opacity = 0.0;
        assert!((g.alpha_at(0.5) - 0.5).abs() < 1e-9);
        assert_eq!(g.alpha_at(0.0), 1.0);
    }

    #[test]
    fn resolved_stops_work_the_extras_in() {
        let mut g = bw();
        g.stops[0].midpoint = 0.3;
        g.scale = 0.5;
        g.axis_offset = 0.25;
        g.repeat = FountainRepeat::Mirror;
        g.stops[1].opacity = 0.5;
        g.steps = 6;
        let r = g.resolved_stops();
        assert!(r.len() >= 2 && r.len() < 200, "{}", r.len());
        for i in 1..40 {
            let t = i as f64 / 40.0 + 0.003;
            let (c, o) = g.color_alpha_at(t);
            let s = crate::style::sample_stops(&r, t);
            let d = (c.to_rgb8()[0] as i32 - s.to_rgb8()[0] as i32).abs();
            assert!(d <= 3, "at {t}: {d}");
            let so = crate::style::sample_opacity(&r, t);
            assert!((so - o).abs() < 0.02, "at {t}: {so} {o}");
        }
        // The plain form keeps the colours and leaves the extras out.
        let p = g.to_plain(512);
        assert!(!p.has_extras() || p.stops.iter().any(|s| s.opacity != 1.0));
        assert_eq!((p.scale, p.axis_offset, p.steps), (1.0, 0.0, 0));
        let e = Fountain::two(FountainKind::Elliptical, Color::BLACK, Color::WHITE, 0.0);
        assert_eq!(e.to_plain(64).kind, FountainKind::Radial);
    }

    #[test]
    fn hue_wraps_and_bias_is_safe() {
        assert_eq!(bias(f64::NAN, 0.5), 0.0);
        assert!((bias(0.5, f64::NAN) - 0.5).abs() < 1e-12);
        assert!((bias(0.3, 0.3) - 0.5).abs() < 1e-9);
    }

    #[test]
    fn a_new_linear_fill_takes_the_current_colours() {
        let b = rect();
        let red = Color::rgb8(255, 0, 0);
        let g = linear_from(
            Point::new(10.0, 20.0),
            Point::new(60.0, 20.0),
            b,
            &Fill::Solid(red),
            Color::BLACK,
        );
        assert_eq!(g.stops[0].color, red);
        assert_eq!(g.stops[1].color, Color::WHITE);
        let (a, z) = g.linear_axis(b);
        assert!(near(a, Point::new(10.0, 20.0)) && near(z, Point::new(60.0, 20.0)));
        let mut three = bw();
        three.stops.insert(1, Stop::new(0.5, red));
        let g = linear_from(a, z, b, &Fill::Fountain(three), Color::BLACK);
        assert_eq!(g.stops.len(), 3);
        assert_eq!(end_stop(&g, true), Some(2));
        assert_eq!(end_stop(&g, false), Some(0));
    }

    #[test]
    fn extras_round_trip_and_old_stops_read() {
        let old = r#"{"pos":0.25,"color":{"model":"gray","v":1.0}}"#;
        let s: Stop = serde_json::from_str(old).expect("old stop");
        assert_eq!((s.opacity, s.midpoint), (1.0, 0.5));
        let json = serde_json::to_string(&bw()).expect("json");
        for field in ["opacity", "midpoint", "scale", "axis_offset", "aspect", "steps", "smooth", "blend"] {
            assert!(!json.contains(field), "{json}");
        }
        let mut g = bw();
        g.kind = FountainKind::Elliptical;
        g.stops[0].opacity = 0.4;
        g.stops[0].midpoint = 0.7;
        g.scale = 0.5;
        g.axis_offset = -0.3;
        g.aspect = 2.0;
        g.steps = 8;
        g.smooth = true;
        g.blend = FountainBlend::Counterclockwise;
        let json = serde_json::to_string(&g).expect("json");
        assert!(json.contains("\"elliptical\"") && json.contains("\"counterclockwise\""), "{json}");
        let back: Fountain = serde_json::from_str(&json).expect("reads");
        assert_eq!(back, g);
    }

    #[test]
    fn raw_params_match_the_kinds() {
        let mut g = bw();
        assert!((g.raw_param_uv(0.25, 0.5, 100.0, 40.0) - 0.25).abs() < 1e-9);
        g.kind = FountainKind::Elliptical;
        let r2 = std::f64::consts::SQRT_2;
        // On the x radius (w/2 * sqrt 2 from the centre): 1.
        let u = 0.5 + 50.0 * r2 / 100.0;
        assert!((g.raw_param_uv(u, 0.5, 100.0, 40.0) - 1.0).abs() < 1e-9);
        let v = 0.5 + 20.0 * r2 / 40.0;
        assert!((g.raw_param_uv(0.5, v, 100.0, 40.0) - 1.0).abs() < 1e-9);
        g.kind = FountainKind::Shaped;
        assert_eq!(g.raw_param_uv(0.1, 0.1, 10.0, 10.0), 0.0);
    }
}
