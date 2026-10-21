//! Colour and Attributes eyedroppers.
//!
//! Both tools have two phases. In the sample phase (the pipette) a click
//! reads from the object under the pointer and the tool turns into a
//! bucket; in the apply phase (the bucket) a click writes what was sampled
//! to the object under the pointer. On the canvas Shift+click samples
//! again and keeps the bucket, Alt+click switches between pipette and
//! bucket (on a fountain fill it clones the whole ramp), Ctrl+click
//! reaches the object inside a group and the right button works on the
//! outline. Esc empties the bucket. The Colour eyedropper reads the
//! model's own colour (a solid fill, the exact point of a fountain ramp,
//! the outline), a bitmap pixel averaged over the sample box or the screen
//! pixels; it applies to the fill (an open line takes it on its outline,
//! text on the whole object), to the outline with the right button, or to
//! one fountain stop when the click is on it. The Attributes eyedropper
//! copies the ticked groups: object properties (outline, fill, text),
//! transformations (size, rotation, skew, position) and effects (opacity,
//! drop shadow, live effects).

use crate::app::App;
use crate::tools::Tool;
use egui::{Color32, Pos2, Rect as ERect, Vec2 as EVec2};
use graphdraw_core::{
    document::{ParagraphStyle, Shadow, Shape, ShapeKind, TextSpan},
    geometry::{Affine, Point, Rect, Vec2},
    id::ShapeId,
    live::Effect,
    style::{Fountain, FountainKind},
    Color, Command, Fill, Stroke,
};

/// Which groups the Attributes eyedropper copies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AttrGroups {
    pub outline: bool,
    pub fill: bool,
    pub text: bool,
    pub size: bool,
    pub rotation: bool,
    pub position: bool,
    pub effects: bool,
    /// The slant of the transform (off by default).
    pub skew: bool,
}

impl Default for AttrGroups {
    fn default() -> Self {
        AttrGroups {
            outline: true,
            fill: true,
            text: true,
            size: false,
            rotation: false,
            position: false,
            effects: false,
            skew: false,
        }
    }
}

/// What the Attributes eyedropper picked up.
#[derive(Clone, Debug, PartialEq)]
pub struct SampledAttrs {
    pub fill: Fill,
    pub stroke: Option<Stroke>,
    pub text: Option<TextSpan>,
    pub width: f64,
    pub height: f64,
    pub rotation: f64,
    pub center: Point,
    pub opacity: f64,
    pub shadow: Option<Shadow>,
    pub effects: Vec<Effect>,
    /// The object it was taken from: it is never a target.
    pub source: Option<ShapeId>,
    /// Shear factor of the transform (see [`skew_of`]).
    pub skew: f64,
    pub extra_fills: Vec<Fill>,
    pub extra_strokes: Vec<Stroke>,
    /// The paragraph spacing of a text object (leading, character and
    /// word spacing).
    pub para: Option<ParagraphStyle>,
}

/// Rotation of a shape's transform in radians (angle of its x axis).
pub fn rotation_of(t: Affine) -> f64 {
    let c = t.as_coeffs();
    c[1].atan2(c[0])
}

/// Shear factor of a transform: its linear part is `R(angle) * U` with
/// `U` upper triangular `[[a, b], [0, d]]`; the factor is `b / d`.
pub fn skew_of(t: Affine) -> f64 {
    let (a, b, d) = upper(t);
    let _ = a;
    if d.abs() < 1e-12 || !d.is_finite() {
        0.0
    } else {
        b / d
    }
}

/// The upper triangular part `(a, b, d)` of a transform's linear part
/// after its rotation is taken out.
fn upper(t: Affine) -> (f64, f64, f64) {
    let c = t.as_coeffs();
    let th = c[1].atan2(c[0]);
    let (cs, sn) = (th.cos(), th.sin());
    let a = c[0].hypot(c[1]);
    let b = c[2] * cs + c[3] * sn;
    let d = -c[2] * sn + c[3] * cs;
    (a, b, d)
}

/// The change that gives `target` the shear factor `skew`, keeping its
/// rotation and scale, about `center`.
pub fn skew_delta(target: Affine, skew: f64, center: Point) -> Affine {
    let c = target.as_coeffs();
    let th = c[1].atan2(c[0]);
    let (cs, sn) = (th.cos(), th.sin());
    let (a, _, d) = upper(target);
    let b = skew * d;
    // Columns of R(th) * [[a, b], [0, d]].
    let new = Affine::new([a * cs, a * sn, b * cs - d * sn, b * sn + d * cs, 0.0, 0.0]);
    let old = Affine::new([c[0], c[1], c[2], c[3], 0.0, 0.0]);
    if old.determinant().abs() < 1e-12 {
        return Affine::IDENTITY;
    }
    Affine::translate(center.to_vec2()) * new * old.inverse() * Affine::translate(-center.to_vec2())
}

/// Whether `t` is one of the two eyedropper tools.
pub fn is_eyedropper(t: Tool) -> bool {
    matches!(t, Tool::ColorEyedropper | Tool::AttributesEyedropper)
}

/// The sample sizes offered: 0 is the native value of the object's model
/// (bitmaps: their pixel; empty areas: the screen pixel), the others the
/// side of the averaged box in pixels.
pub const SAMPLE_SIZES: [u32; 5] = [0, 1, 2, 3, 5];

/// Side of the magnifier grid in screen pixels.
pub const GRID: usize = 11;
/// Size of one magnified pixel.
pub const CELL: f32 = 10.0;
/// Gap between the pointer and the magnifier.
pub const HUD_OFFSET: f32 = 24.0;
/// The magnifier stops after the pointer rests this long, so it can be
/// reached (right click, recent tones).
pub const DWELL_S: f64 = 0.4;
/// It follows again once the pointer is this far outside it.
pub const HUD_RELEASE: f32 = 40.0;
/// How many recent tones are kept.
pub const RECENT: usize = 5;
/// The apply highlight lasts this long.
pub const RIPPLE_S: f64 = 0.15;

/// The colour under the pointer and the screen pixels around it, for the
/// magnifier.
#[derive(Debug, Clone, PartialEq)]
pub struct HoverInfo {
    pub color: Color,
    /// `GRID` x `GRID` screen pixels around the pointer (sRGB).
    pub grid: Vec<[u8; 3]>,
    /// The colour's name in the document (a colour style), when it is the
    /// same colour.
    pub name: Option<String>,
}

/// The floating magnifier: where it is, whether it waits for the
/// pointer, and its box in the last frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hud {
    pub pos: Pos2,
    pub frozen: bool,
    pub placed: bool,
    last: Pos2,
    moved_at: f64,
    /// Box drawn in the last frame (nothing when not drawn).
    pub rect: ERect,
}

impl Default for Hud {
    fn default() -> Self {
        Hud {
            pos: Pos2::ZERO,
            frozen: false,
            placed: false,
            last: Pos2::ZERO,
            moved_at: 0.0,
            rect: ERect::NOTHING,
        }
    }
}

impl Hud {
    /// Follow the pointer, beside it (on the other side near the edges of
    /// `bounds`). When the pointer rests for [`DWELL_S`] the magnifier
    /// stops and waits; it follows again once the pointer goes away.
    pub fn follow(&mut self, pointer: Pos2, now: f64, size: EVec2, bounds: ERect) {
        if pointer != self.last {
            self.last = pointer;
            self.moved_at = now;
        }
        let rect = ERect::from_min_size(self.pos, size);
        if self.frozen {
            if rect.expand(HUD_RELEASE).contains(pointer) {
                return;
            }
            self.frozen = false;
        } else if self.placed && now - self.moved_at >= DWELL_S {
            self.frozen = true;
            return;
        }
        let mut p = pointer + EVec2::splat(HUD_OFFSET);
        if p.x + size.x > bounds.max.x {
            p.x = pointer.x - HUD_OFFSET - size.x;
        }
        if p.y + size.y > bounds.max.y {
            p.y = pointer.y - HUD_OFFSET - size.y;
        }
        self.pos = p;
        self.placed = true;
    }
}

/// What the eyedroppers keep besides the sampled colour and attributes.
#[derive(Debug, Clone, Default)]
pub struct EyedropState {
    /// Alt+click on a fountain fill: the whole ramp is in the bucket.
    pub ramp: Option<Fountain>,
    /// The last tones sampled, newest first, without repeats.
    pub recent: Vec<Color>,
    /// Objects that just received the bucket and when.
    pub ripple: Option<(Vec<ShapeId>, f64)>,
    pub hud: Hud,
    pub info: Option<HoverInfo>,
    /// The interface clock (seconds), updated every frame.
    pub now: f64,
}

impl EyedropState {
    /// Put `c` at the top of the recent tones, without repeating it.
    pub fn push_recent(&mut self, c: Color) {
        let hex = c.to_hex();
        self.recent.retain(|r| r.to_hex() != hex);
        self.recent.insert(0, c);
        self.recent.truncate(RECENT);
    }

    /// The highlight under way: the part of it already gone, 0 to 1.
    pub fn ripple_phase(&self) -> Option<f64> {
        let (_, t0) = self.ripple.as_ref()?;
        let k = (self.now - t0) / RIPPLE_S;
        (0.0..1.0).contains(&k).then_some(k)
    }

    pub fn over_hud(&self, p: Pos2) -> bool {
        self.hud.rect.is_positive() && self.hud.rect.contains(p)
    }
}

/// A click of an eyedropper on the canvas.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EyeClick {
    /// The right button: the outline.
    pub right: bool,
    /// Sample again, keeping the bucket.
    pub shift: bool,
    /// Switch pipette and bucket; on a fountain, clone the ramp.
    pub alt: bool,
    /// The object inside a group.
    pub ctrl: bool,
}

/// `n` x `n` screen pixels around pixel (`cx`, `cy`) of a premultiplied
/// picture `size` wide and high, laid over the background `bg(x, y)`
/// (outside the picture: the background only).
pub fn grid_from(
    pixels: &[Color32],
    size: [usize; 2],
    cx: i64,
    cy: i64,
    n: usize,
    bg: impl Fn(i64, i64) -> [u8; 3],
) -> Vec<[u8; 3]> {
    let h = n as i64 / 2;
    let mut out = Vec::with_capacity(n * n);
    for y in cy - h..cy - h + n as i64 {
        for x in cx - h..cx - h + n as i64 {
            let b = bg(x, y);
            let inside = x >= 0 && y >= 0 && (x as usize) < size[0] && (y as usize) < size[1];
            let px = if inside {
                pixels.get(y as usize * size[0] + x as usize)
            } else {
                None
            };
            let Some(px) = px else {
                out.push(b);
                continue;
            };
            let a = px.a() as u32;
            let c = [px.r(), px.g(), px.b()];
            let mix =
                |k: usize| (c[k] as u32 + (b[k] as u32 * (255 - a) + 127) / 255).min(255) as u8;
            out.push([mix(0), mix(1), mix(2)]);
        }
    }
    out
}

/// The average of the `side` x `side` box at the middle of an `n` x `n`
/// grid; an even box starts at the middle pixel and grows right and down,
/// as a bitmap sample box does.
pub fn average(grid: &[[u8; 3]], n: usize, side: usize) -> Option<Color> {
    if grid.len() != n * n || side == 0 || side > n {
        return None;
    }
    let c = (n / 2) as i64;
    let half = (side as i64 - 1) / 2;
    let mut sum = [0.0f64; 3];
    let mut cnt = 0.0;
    for y in c - half..c - half + side as i64 {
        for x in c - half..c - half + side as i64 {
            if x < 0 || y < 0 || x >= n as i64 || y >= n as i64 {
                continue;
            }
            let p = grid[y as usize * n + x as usize];
            for k in 0..3 {
                sum[k] += p[k] as f64;
            }
            cnt += 1.0;
        }
    }
    if cnt == 0.0 {
        return None;
    }
    let ch = |k: usize| (sum[k] / cnt).round().clamp(0.0, 255.0) as u8;
    Some(Color::rgb8(ch(0), ch(1), ch(2)))
}

/// The CMYK and RGB lines of the magnifier (a CMYK colour shows its own
/// values).
pub fn values_text(c: Color) -> (String, String) {
    let cmyk = match c {
        Color::Cmyk { .. } => c,
        _ => c.convert_to("cmyk"),
    };
    let (cc, m, y, k) = match cmyk {
        Color::Cmyk { c, m, y, k } => (c, m, y, k),
        _ => (0.0, 0.0, 0.0, 0.0),
    };
    let pc = |v: f32| (v * 100.0).round() as i64;
    let [r, g, b] = c.to_rgb8();
    (
        format!("C:{} M:{} Y:{} K:{}", pc(cc), pc(m), pc(y), pc(k)),
        format!("R:{r} G:{g} B:{b}"),
    )
}

/// Where along a fountain fill (0 to 1 across its span, before the
/// repeats and the edge pad) the page point `p` falls, for the object's
/// page bounds `b`; `None` for a shaped fountain.
pub fn fountain_t(f: &Fountain, b: Rect, p: Point) -> Option<f64> {
    let (w, h) = (b.width(), b.height());
    if !(w > 0.0 || h > 0.0) {
        return None;
    }
    let c = b.center();
    match f.kind {
        FountainKind::Shaped => None,
        FountainKind::Linear => {
            let a = f.angle.to_radians();
            let dir = Vec2::new(a.cos(), a.sin());
            let half = (w * a.cos().abs() + h * a.sin().abs()) / 2.0;
            if half <= 0.0 {
                return Some(0.0);
            }
            let start = c - dir * half;
            Some((p - start).dot(dir) / (2.0 * half))
        }
        FountainKind::Radial | FountainKind::Elliptical => {
            let cr = Point::new(c.x + f.offset.x * w / 2.0, c.y + f.offset.y * h / 2.0);
            let radius = (w.max(h) / 2.0) * std::f64::consts::SQRT_2;
            (radius > 0.0).then(|| (p - cr).hypot() / radius)
        }
        FountainKind::Conical | FountainKind::Square => {
            if w <= 0.0 || h <= 0.0 {
                return Some(0.0);
            }
            let u = (p.x - b.x0) / w;
            let v = (p.y - b.y0) / h;
            let dx = u - 0.5 - f.offset.x / 2.0;
            let dy = v - 0.5 - f.offset.y / 2.0;
            Some(if f.kind == FountainKind::Conical {
                let a = dy.atan2(dx) - f.angle.to_radians();
                let a = a.rem_euclid(std::f64::consts::TAU) / std::f64::consts::TAU;
                if a < 0.5 {
                    a * 2.0
                } else {
                    2.0 - a * 2.0
                }
            } else {
                (dx.abs().max(dy.abs()) * 2.0).min(1.0)
            })
        }
    }
}

/// The page positions of a fountain's stops (index into `f.stops`) on
/// the object's page bounds `b`, where the bucket can hit them: on the
/// axis of a linear fill, from the centre to the right for the radial and
/// square ones, around the centre for a conical one (the first run of
/// the colours).
pub fn stop_points(f: &Fountain, b: Rect) -> Vec<(usize, Point)> {
    let (w, h) = (b.width(), b.height());
    let c = b.center();
    let e = f.edge_pad.clamp(0.0, 0.49);
    let n = f.cycles().max(1) as f64;
    let along = |pos: f64| {
        let mut t = pos.clamp(0.0, 1.0);
        if f.invert {
            t = 1.0 - t;
        }
        t = e + t * (1.0 - 2.0 * e);
        if n > 1.0 {
            t /= n;
        }
        if f.bilinear {
            t /= 2.0;
        }
        t
    };
    let at = |t: f64| -> Option<Point> {
        match f.kind {
            FountainKind::Shaped => None,
            FountainKind::Linear => {
                let a = f.angle.to_radians();
                let dir = Vec2::new(a.cos(), a.sin());
                let half = (w * a.cos().abs() + h * a.sin().abs()) / 2.0;
                Some(c - dir * half + dir * (2.0 * half * t))
            }
            FountainKind::Radial | FountainKind::Elliptical => {
                let cr = Point::new(c.x + f.offset.x * w / 2.0, c.y + f.offset.y * h / 2.0);
                let radius = (w.max(h) / 2.0) * std::f64::consts::SQRT_2;
                Some(Point::new(cr.x + radius * t, cr.y))
            }
            FountainKind::Square => {
                let u = 0.5 + f.offset.x / 2.0 + t / 2.0;
                let v = 0.5 + f.offset.y / 2.0;
                Some(Point::new(b.x0 + u * w, b.y0 + v * h))
            }
            FountainKind::Conical => {
                let ang = f.angle.to_radians() + t * std::f64::consts::PI;
                let u = 0.5 + f.offset.x / 2.0 + ang.cos() * 0.25;
                let v = 0.5 + f.offset.y / 2.0 + ang.sin() * 0.25;
                Some(Point::new(b.x0 + u * w, b.y0 + v * h))
            }
        }
    };
    f.stops
        .iter()
        .enumerate()
        .filter_map(|(i, s)| at(along(s.pos)).map(|p| (i, p)))
        .collect()
}

impl App {
    /// The usual hit test, else the topmost unfilled closed object whose
    /// interior contains `p`: what the paint bucket fills, and what a
    /// connector or blend drag starts from and ends on.
    pub fn hit_test_inside(&self, p: Point) -> Option<ShapeId> {
        if let Some(id) = self.hit_test(p) {
            return Some(id);
        }
        for layer in self.page_layers().into_iter().rev() {
            if !layer.visible || layer.locked {
                continue;
            }
            for s in layer.shapes.iter().rev() {
                if s.locked || !s.visible || !s.bounds().contains(p) {
                    continue;
                }
                if crate::tools2::point_in_path(&s.page_path(), p) {
                    return Some(s.id);
                }
            }
        }
        None
    }

    /// Object `id` (top level or inside a group) with its page transform.
    pub fn eyedrop_page_shape(&self, id: ShapeId) -> Option<Shape> {
        let s = self.doc().find_shape(id)?;
        let parent = self.doc().parent_transform(id).unwrap_or(Affine::IDENTITY);
        let mut s = s.clone();
        s.transform = parent * s.transform;
        Some(s)
    }

    /// The innermost object under `p` (inside groups), else the topmost.
    pub fn eyedrop_deep_hit(&self, p: Point) -> Option<ShapeId> {
        self.hits_at(p, true)
            .into_iter()
            .next()
            .or_else(|| self.hit_test(p))
    }

    /// What a bucket click at `p` paints: with `ctrl` the object inside a
    /// group, else the whole object (a group passes it on to everything in
    /// it), else the unfilled closed object around the point.
    pub fn eyedrop_target(&self, p: Point, ctrl: bool) -> Option<ShapeId> {
        if ctrl {
            if let Some(id) = self.hits_at(p, true).into_iter().next() {
                return Some(id);
            }
        }
        self.hit_test_inside(p)
    }

    /// The pixel of bitmap `s` (page space) under `p`, averaged over the
    /// `n` x `n` box anchored at it and clipped to the image.
    fn bitmap_pixel(s: &Shape, p: Point, n: u32) -> Option<Color> {
        let ShapeKind::Bitmap {
            rect,
            width_px,
            height_px,
            png,
            fx: _,
        } = &s.kind
        else {
            return None;
        };
        let lp = s.transform.inverse() * p;
        if !(rect.contains(lp) && rect.width() > 0.0 && rect.height() > 0.0) {
            return None;
        }
        let img = crate::bitmap_fx::decode(png)?;
        let fx = (lp.x - rect.x0) / rect.width() * *width_px as f64;
        let fy = (rect.y1 - lp.y) / rect.height() * *height_px as f64;
        let n = n.max(1) as i64;
        let half = (n - 1) / 2;
        let (mut r, mut g, mut b, mut cnt) = (0u64, 0u64, 0u64, 0u64);
        for dy in -half..(n - half) {
            for dx in -half..(n - half) {
                let x = fx.floor() as i64 + dx;
                let y = fy.floor() as i64 + dy;
                if x < 0 || y < 0 || x >= img.width() as i64 || y >= img.height() as i64 {
                    continue;
                }
                let px = img.get_pixel(x as u32, y as u32);
                r += px[0] as u64;
                g += px[1] as u64;
                b += px[2] as u64;
                cnt += 1;
            }
        }
        match (r.checked_div(cnt), g.checked_div(cnt), b.checked_div(cnt)) {
            (Some(r), Some(g), Some(b)) => Some(Color::rgb8(r as u8, g as u8, b as u8)),
            _ => None,
        }
    }

    /// The model's own colour of `s` (page space) at `p`: its fill where
    /// the point is inside it (a fountain at that exact point of its
    /// ramp, with the ramp), else its outline.
    fn native_color(&self, s: &Shape, p: Point) -> Option<(Color, Option<Fountain>)> {
        let filled = !matches!(s.fill, Fill::None);
        let inside = filled
            && (matches!(
                s.kind,
                ShapeKind::Text { .. } | ShapeKind::Table(_) | ShapeKind::SymbolInstance { .. }
            ) || crate::tools2::point_in_path(&s.page_path(), p));
        let fill_color = || match &s.fill {
            Fill::Solid(c) => Some((*c, None)),
            Fill::Fountain(f) => {
                let c = match fountain_t(f, s.bounds(), p) {
                    Some(t) => f.color_at(t),
                    None => f.first_color(),
                };
                Some((c, Some(f.clone())))
            }
            Fill::Mesh(m) => m
                .nodes
                .iter()
                .min_by(|a, b| {
                    let da = (s.transform * a.pos - p).hypot();
                    let db = (s.transform * b.pos - p).hypot();
                    da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|n| (n.color, None)),
            Fill::Texture(t) => Some((t.color_a, None)),
            _ => None,
        };
        if inside {
            if let Some(c) = fill_color() {
                return Some(c);
            }
        }
        if let Some(st) = &s.stroke {
            return Some((st.color, None));
        }
        fill_color()
    }

    /// The screen pixels around page point `p` from the last drawing of
    /// the page (kept while an eyedropper is active), over the paper or
    /// the desktop.
    pub fn eyedrop_screen_grid(&self, p: Point) -> Option<Vec<[u8; 3]>> {
        let raster = self.raster.borrow();
        let kept = raster.kept.as_ref()?;
        let (w, h) = (kept.size[0], kept.size[1]);
        if w == 0 || h == 0 || !kept.screen.is_positive() {
            return None;
        }
        let pos = self.view.to_screen(p);
        let sx = w as f32 / kept.screen.width();
        let sy = h as f32 / kept.screen.height();
        let cx = ((pos.x - kept.screen.min.x) * sx).floor() as i64;
        let cy = ((pos.y - kept.screen.min.y) * sy).floor() as i64;
        let paper = crate::canvas::page_screen_rect(self);
        let desk = self.desktop_color();
        let screen = kept.screen;
        Some(grid_from(&kept.pixels, kept.size, cx, cy, GRID, |x, y| {
            let q = Pos2::new(
                screen.min.x + (x as f32 + 0.5) / sx,
                screen.min.y + (y as f32 + 0.5) / sy,
            );
            let c = if paper.contains(q) {
                crate::theme::Tokens::PAGE
            } else {
                desk
            };
            [c.r(), c.g(), c.b()]
        }))
    }

    /// The colour under `p`: a bitmap pixel (averaged over the sample box),
    /// else with a sample size the screen pixels averaged, else the model's
    /// colour (a fountain at that point of its ramp, the outline outside
    /// the fill), else the screen pixel. With `outline` the outline colour
    /// is read first.
    pub fn sample_color_at(&self, p: Point, outline: bool) -> Option<Color> {
        let n = self.eyedropper_sample;
        let s = self
            .eyedrop_deep_hit(p)
            .and_then(|id| self.eyedrop_page_shape(id));
        if let Some(s) = &s {
            if outline {
                if let Some(st) = &s.stroke {
                    return Some(st.color);
                }
            }
            if matches!(s.kind, ShapeKind::Bitmap { .. }) {
                if let Some(c) = Self::bitmap_pixel(s, p, n) {
                    return Some(c);
                }
            }
        }
        let screen = |side: u32| {
            self.eyedrop_screen_grid(p)
                .and_then(|g| average(&g, GRID, side as usize))
        };
        if n > 0 {
            if let Some(c) = screen(n) {
                return Some(c);
            }
        }
        if let Some(s) = &s {
            if let Some((c, _)) = self.native_color(s, p) {
                return Some(c);
            }
        }
        screen(1)
    }

    /// The fountain fill under `p` (inside groups too), for Alt+click.
    pub fn fountain_at(&self, p: Point) -> Option<Fountain> {
        let s = self.eyedrop_page_shape(self.eyedrop_deep_hit(p)?)?;
        match self.native_color(&s, p)? {
            (_, Some(f)) => Some(f),
            _ => None,
        }
    }

    /// The fountain stop of the object under `p` within a few screen
    /// pixels of it: the object and the stop's index.
    pub fn fountain_stop_at(&self, p: Point) -> Option<(ShapeId, usize)> {
        let id = self.eyedrop_deep_hit(p)?;
        let s = self.eyedrop_page_shape(id)?;
        let Fill::Fountain(f) = &s.fill else {
            return None;
        };
        let tol = 6.0 / self.view.zoom as f64;
        stop_points(f, s.bounds())
            .into_iter()
            .filter(|(_, q)| (*q - p).hypot() <= tol)
            .min_by(|a, b| {
                (a.1 - p)
                    .hypot()
                    .partial_cmp(&(b.1 - p).hypot())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(i, _)| (id, i))
    }

    /// Give stop `index` of object `id`'s fountain the colour `c` (one
    /// undo step); the other stops stay.
    pub fn replace_fountain_stop(&mut self, id: ShapeId, index: usize, c: Color) -> bool {
        let Some(s) = self.doc().find_shape(id) else {
            return false;
        };
        let Fill::Fountain(mut f) = s.fill.clone() else {
            return false;
        };
        let Some(stop) = f.stops.get_mut(index) else {
            return false;
        };
        stop.color = c;
        self.run(Command::SetFill {
            shapes: vec![id],
            fill: Fill::Fountain(f),
        });
        true
    }

    /// Paint `fill` on object `id` as one step; text takes it on the whole
    /// object (its letters' own fills are dropped).
    fn eyedrop_fill(&mut self, id: ShapeId, fill: Fill) -> bool {
        let Some(s) = self.doc().find_shape(id) else {
            return false;
        };
        if matches!(s.kind, ShapeKind::Bitmap { .. }) || s.locked {
            return false;
        }
        let mut cmds = Vec::new();
        if let ShapeKind::Text { spans, .. } = &s.kind {
            if spans.iter().any(|sp| sp.fill.is_some()) {
                let mut kind = s.kind.clone();
                if let ShapeKind::Text { spans, .. } = &mut kind {
                    for sp in spans {
                        sp.fill = None;
                    }
                }
                cmds.push(Command::SetShapeKind { shape: id, kind });
            }
        }
        cmds.push(Command::SetFill {
            shapes: vec![id],
            fill,
        });
        if cmds.len() == 1 {
            if let Some(c) = cmds.pop() {
                self.run(c);
            }
        } else if let Err(e) = self.engine.run_batch("Fill", &cmds) {
            self.status = e.to_string();
            return false;
        }
        true
    }

    /// Whether a left click with the colour bucket goes to `id`'s outline:
    /// an open line whose fill does not show.
    fn paints_outline(&self, id: ShapeId) -> bool {
        self.doc().find_shape(id).is_some_and(|s| {
            matches!(s.kind, ShapeKind::Path { closed: false, .. })
                && !self.doc().metadata.fill_open_curves
        })
    }

    /// Give object `id` the outline colour `c`; one without an outline
    /// gets the outline new objects get, in that colour.
    fn eyedrop_outline(&mut self, id: ShapeId, c: Color) -> bool {
        let Some(s) = self.doc().find_shape(id) else {
            return false;
        };
        if matches!(s.kind, ShapeKind::Bitmap { .. }) || s.locked {
            return false;
        }
        let mut st = s
            .stroke
            .clone()
            .or_else(|| self.default_stroke())
            .unwrap_or_default();
        st.color = c;
        self.run(Command::SetStroke {
            shapes: vec![id],
            stroke: Some(st),
        });
        true
    }

    /// Apply a sampled colour to the object under `p`, to its outline when
    /// `outline` (an object without one gets the outline new objects get).
    /// An open line takes the colour on its outline; text on the whole
    /// object.
    // The point-based calls are kept for the tests and for callers that
    // only know a point; the canvas goes through `eyedropper_gesture`.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn apply_color_at(&mut self, p: Point, c: Color, outline: bool) -> bool {
        let Some(id) = self.hit_test_inside(p) else {
            return false;
        };
        self.apply_color_to(id, c, outline)
    }

    fn apply_color_to(&mut self, id: ShapeId, c: Color, outline: bool) -> bool {
        if outline || self.paints_outline(id) {
            self.eyedrop_outline(id, c)
        } else {
            self.eyedrop_fill(id, Fill::Solid(c))
        }
    }

    /// Read every attribute group from the object under `p` (the object
    /// inside a group).
    pub fn sample_attrs_at(&self, p: Point) -> Option<SampledAttrs> {
        let id = self.eyedrop_deep_hit(p)?;
        let s = self.eyedrop_page_shape(id)?;
        let b = s.bounds();
        let (text, para) = match &s.kind {
            ShapeKind::Text { spans, para, .. } => (spans.first().cloned(), Some(para.clone())),
            _ => (None, None),
        };
        Some(SampledAttrs {
            fill: s.fill.clone(),
            stroke: s.stroke.clone(),
            text,
            width: b.width(),
            height: b.height(),
            rotation: rotation_of(s.transform),
            center: b.center(),
            opacity: s.opacity,
            shadow: s.shadow,
            effects: s.effects.clone(),
            source: Some(id),
            skew: skew_of(s.transform),
            extra_fills: s.extra_fills.clone(),
            extra_strokes: s.extra_strokes.clone(),
            para,
        })
    }

    /// Write the ticked groups of the sample to the object under `p`.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn apply_attrs_at(&mut self, p: Point, a: &SampledAttrs, g: AttrGroups) -> bool {
        let Some(id) = self.hit_test_inside(p) else {
            return false;
        };
        self.apply_attrs_to(id, a, g)
    }

    /// Write the ticked groups of the sample to object `id`, as one undo
    /// step. The object the sample came from is left alone.
    pub fn apply_attrs_to(&mut self, id: ShapeId, a: &SampledAttrs, g: AttrGroups) -> bool {
        if a.source == Some(id) {
            return false;
        }
        let Some(s) = self.eyedrop_page_shape(id) else {
            return false;
        };
        if s.locked {
            return false;
        }
        let mut cmds = Vec::new();
        let shapes = vec![id];
        let bitmap = matches!(s.kind, ShapeKind::Bitmap { .. });
        if g.fill && !bitmap {
            cmds.push(Command::SetFill {
                shapes: shapes.clone(),
                fill: a.fill.clone(),
            });
        }
        if g.outline && !bitmap {
            cmds.push(Command::SetStroke {
                shapes: shapes.clone(),
                stroke: a.stroke.clone(),
            });
        }
        if (g.fill || g.outline) && !bitmap && !matches!(s.kind, ShapeKind::Group { .. }) {
            let fills = if g.fill {
                a.extra_fills.clone()
            } else {
                s.extra_fills.clone()
            };
            let strokes = if g.outline {
                a.extra_strokes.clone()
            } else {
                s.extra_strokes.clone()
            };
            if fills != s.extra_fills || strokes != s.extra_strokes {
                cmds.push(Command::SetExtraPaint {
                    shapes: shapes.clone(),
                    fills,
                    strokes,
                });
            }
        }
        // Text: the sampled font and spacing on every letter (the text
        // itself is kept); a fill copied drops the letters' own fills.
        if let ShapeKind::Text { .. } = &s.kind {
            let mut kind = s.kind.clone();
            if let ShapeKind::Text { spans, para, .. } = &mut kind {
                if g.text {
                    if let Some(t) = &a.text {
                        for sp in spans.iter_mut() {
                            sp.font_family = t.font_family.clone();
                            sp.size_pt = t.size_pt;
                            sp.bold = t.bold;
                            sp.italic = t.italic;
                            sp.tracking_pct = t.tracking_pct;
                        }
                    }
                    if let Some(ap) = &a.para {
                        para.leading_pct = ap.leading_pct;
                        para.leading_unit = ap.leading_unit;
                        para.char_spacing_pct = ap.char_spacing_pct;
                        para.word_spacing_pct = ap.word_spacing_pct;
                    }
                }
                if g.fill {
                    for sp in spans.iter_mut() {
                        sp.fill = None;
                    }
                }
            }
            if kind != s.kind {
                cmds.push(Command::SetShapeKind { shape: id, kind });
            }
        }
        if g.effects {
            cmds.push(Command::SetOpacity {
                shapes: shapes.clone(),
                opacity: a.opacity,
            });
            cmds.push(Command::SetShadow {
                shapes: shapes.clone(),
                shadow: a.shadow,
            });
            cmds.push(Command::SetEffects {
                shape: id,
                effects: a.effects.clone(),
            });
        }
        // Transformations, about the object's centre: slant to the sampled
        // skew, rotate to the sampled angle, scale to the sampled size, then
        // move to the sampled position.
        let b = s.bounds();
        let c = b.center();
        let mut t = Affine::IDENTITY;
        if g.skew {
            t = skew_delta(s.transform, a.skew, c) * t;
        }
        if g.rotation {
            t = Affine::rotate_about(a.rotation - rotation_of(s.transform), c) * t;
        }
        if g.size && b.width() > 1e-9 && b.height() > 1e-9 {
            let sx = a.width / b.width();
            let sy = a.height / b.height();
            t = Affine::translate(c.to_vec2())
                * Affine::scale_non_uniform(sx, sy)
                * Affine::translate(-c.to_vec2())
                * t;
        }
        if g.position {
            t = Affine::translate(a.center - c) * t;
        }
        if t != Affine::IDENTITY {
            cmds.push(Command::TransformShapes {
                shapes: shapes.clone(),
                transform: t,
            });
        }
        if cmds.is_empty() {
            return false;
        }
        if let Err(e) = self.engine.run_batch("Copy Attributes", &cmds) {
            self.status = e.to_string();
            return false;
        }
        true
    }

    /// Clicks of both eyedropper tools. In Select mode a click samples and
    /// switches to Apply mode; in Apply mode a click applies. For the
    /// colour eyedropper Shift reads or writes the outline colour; for the
    /// attributes eyedropper Shift samples again for this click. (The
    /// canvas goes through [`App::eyedropper_gesture`].)
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn eyedropper_click(&mut self, p: Point, attributes: bool, shift: bool) {
        if attributes {
            let apply = self.eyedropper_apply && self.eyedropper_attrs.is_some() && !shift;
            if apply {
                if let Some(a) = self.eyedropper_attrs.clone() {
                    let g = self.eyedropper_groups;
                    self.apply_attrs_at(p, &a, g);
                }
            } else if let Some(a) = self.sample_attrs_at(p) {
                self.eyedropper_attrs = Some(a);
                self.eyedropper_apply = true;
                self.status = crate::i18n::tr("status.sampled_attrs");
            }
        } else {
            let apply = self.eyedropper_apply && self.eyedropper_color.is_some();
            if apply {
                if let Some(c) = self.eyedropper_color {
                    self.apply_color_at(p, c, shift);
                }
            } else if let Some(c) = self.sample_color_at(p, shift) {
                self.eyedrop_load_color(c);
            }
        }
    }

    /// Whether something is in the bucket of the active eyedropper.
    pub fn eyedropper_has_bucket(&self) -> bool {
        if self.tool == Tool::AttributesEyedropper {
            self.eyedropper_attrs.is_some()
        } else {
            self.eyedropper_color.is_some() || self.eyedrop.ramp.is_some()
        }
    }

    /// Whether the next click samples (the pipette) rather than applies;
    /// Shift samples again without dropping the bucket.
    pub fn eyedropper_sampling(&self, shift: bool) -> bool {
        !self.eyedropper_apply || !self.eyedropper_has_bucket() || shift
    }

    /// A sampled colour goes into the bucket (and the recent tones).
    pub fn eyedrop_load_color(&mut self, c: Color) {
        self.eyedropper_color = Some(c);
        self.eyedrop.ramp = None;
        self.eyedropper_apply = true;
        self.eyedrop.push_recent(c);
        self.status = crate::i18n::trf(
            "status.sampled_color",
            &[("c", &crate::app::color_description(c))],
        );
    }

    /// A cloned fountain ramp goes into the bucket.
    pub fn eyedrop_load_ramp(&mut self, f: Fountain) {
        let first = f.first_color();
        self.eyedropper_color = Some(first);
        self.eyedrop.push_recent(first);
        self.eyedrop.ramp = Some(f);
        self.eyedropper_apply = true;
        self.status = crate::i18n::tr("status.eyedrop_ramp");
    }

    /// A click of an eyedropper on the canvas (see [`EyeClick`]): sample
    /// with the pipette, apply with the bucket. Every application is one
    /// undo step; sampling and switching change nothing in the drawing.
    pub fn eyedropper_gesture(&mut self, p: Point, attributes: bool, g: EyeClick) {
        if g.alt && !g.shift {
            if self.eyedropper_apply && self.eyedropper_has_bucket() {
                self.eyedropper_apply = false;
                self.status = crate::i18n::tr("status.eyedrop_pipette");
                return;
            }
            if !attributes {
                if let Some(f) = self.fountain_at(p) {
                    self.eyedrop_load_ramp(f);
                    return;
                }
            }
            if self.eyedropper_has_bucket() {
                self.eyedropper_apply = true;
                self.status = crate::i18n::tr("status.eyedrop_bucket");
                return;
            }
        }
        if self.eyedropper_sampling(g.shift) {
            if attributes {
                match self.sample_attrs_at(p) {
                    Some(a) => {
                        self.eyedropper_attrs = Some(a);
                        self.eyedropper_apply = true;
                        self.status = crate::i18n::tr("status.sampled_attrs");
                    }
                    None => self.status = crate::i18n::tr("status.eyedrop_nothing"),
                }
            } else {
                match self.sample_color_at(p, g.right) {
                    Some(c) => self.eyedrop_load_color(c),
                    None => self.status = crate::i18n::tr("status.eyedrop_nothing"),
                }
            }
            return;
        }
        let Some(id) = self.eyedrop_target(p, g.ctrl) else {
            return;
        };
        let done = if attributes {
            match self.eyedropper_attrs.clone() {
                Some(a) => {
                    let groups = self.eyedropper_groups;
                    self.apply_attrs_to(id, &a, groups)
                }
                None => false,
            }
        } else if let Some(f) = self.eyedrop.ramp.clone() {
            if g.right {
                self.status = crate::i18n::tr("status.eyedrop_ramp_fill");
                false
            } else {
                // The fountain is laid out on each object's own bounds, so
                // the ramp fits the target's box.
                self.eyedrop_fill(id, Fill::Fountain(f))
            }
        } else if let Some(c) = self.eyedropper_color {
            match (g.right, self.fountain_stop_at(p)) {
                (false, Some((sid, i))) => self.replace_fountain_stop(sid, i, c),
                _ => self.apply_color_to(id, c, g.right),
            }
        } else {
            false
        };
        if done {
            self.eyedrop.ripple = Some((vec![id], self.eyedrop.now));
        }
    }

    /// A canvas click with an eyedropper, `right` for the right button.
    pub fn eyedropper_canvas_click(&mut self, p: Point, mods: egui::Modifiers, right: bool) {
        let attributes = self.tool == Tool::AttributesEyedropper;
        let g = EyeClick {
            right,
            shift: mods.shift,
            alt: mods.alt,
            ctrl: mods.ctrl || mods.mac_cmd,
        };
        self.eyedropper_gesture(p, attributes, g);
    }

    /// The right button with an eyedropper: it works on the outline and
    /// opens no menu. True when it was taken.
    pub fn eyedropper_right_click(&mut self, p: Point, mods: egui::Modifiers) -> bool {
        if !is_eyedropper(self.tool) {
            return false;
        }
        self.eyedropper_canvas_click(p, mods, true);
        true
    }

    /// Esc with an eyedropper holding something: empty the bucket and go
    /// back to the pipette, staying in the tool. True when it was taken.
    pub fn eyedropper_escape(&mut self) -> bool {
        if !is_eyedropper(self.tool) || !(self.eyedropper_has_bucket() || self.eyedropper_apply) {
            return false;
        }
        self.eyedropper_reset();
        self.status = crate::i18n::tr("status.eyedrop_emptied");
        true
    }

    /// Esc or the Select Color button: back to sampling.
    pub fn eyedropper_reset(&mut self) {
        self.eyedropper_color = None;
        self.eyedropper_attrs = None;
        self.eyedropper_apply = false;
        self.eyedrop.ramp = None;
    }

    /// The colour's name in the document: a colour style of that colour.
    pub fn eyedrop_color_name(&self, c: Color) -> Option<String> {
        self.doc()
            .color_styles
            .iter()
            .find(|s| s.color == c || s.color.to_hex() == c.to_hex())
            .map(|s| s.name.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use graphdraw_core::geometry::Rect;
    use graphdraw_core::style::Stop;

    fn rect(app: &mut App, r: Rect) -> ShapeId {
        app.new_shape(ShapeKind::Rect {
            rect: r,
            radius: 0.0,
            corners: None,
        })
        .expect("shape")
    }

    fn solid(app: &mut App, id: ShapeId, c: Color) {
        app.run(Command::SetFill {
            shapes: vec![id],
            fill: Fill::Solid(c),
        });
    }

    fn undo_len(app: &App) -> usize {
        app.engine.history_labels().0.len()
    }

    const RED: Color = Color::Rgb {
        r: 1.0,
        g: 0.0,
        b: 0.0,
    };
    const GREEN: Color = Color::Rgb {
        r: 0.0,
        g: 1.0,
        b: 0.0,
    };
    const BLUE: Color = Color::Rgb {
        r: 0.0,
        g: 0.0,
        b: 1.0,
    };

    fn click() -> EyeClick {
        EyeClick::default()
    }

    #[test]
    fn colour_eyedropper_samples_fill_then_applies_to_fill_or_outline() {
        let mut app = App::headless();
        let a = rect(&mut app, Rect::new(0.0, 0.0, 10.0, 10.0));
        let b = rect(&mut app, Rect::new(20.0, 0.0, 30.0, 10.0));
        app.run(Command::SetFill {
            shapes: vec![a],
            fill: Fill::Solid(Color::rgb8(200, 10, 10)),
        });
        app.select(Vec::new());
        app.eyedropper_click(Point::new(5.0, 5.0), false, false);
        assert_eq!(app.eyedropper_color, Some(Color::rgb8(200, 10, 10)));
        // An unfilled object is hit inside too when applying.
        app.eyedropper_click(Point::new(25.0, 5.0), false, false);
        let sb = app.doc().find_shape(b).expect("b");
        assert_eq!(sb.fill, Fill::Solid(Color::rgb8(200, 10, 10)));
        // Shift+click in the apply phase colours the outline.
        app.eyedropper_click(Point::new(25.0, 5.0), false, true);
        let sb = app.doc().find_shape(b).expect("b");
        assert_eq!(
            sb.stroke.as_ref().map(|s| s.color),
            Some(Color::rgb8(200, 10, 10))
        );
    }

    #[test]
    fn attributes_eyedropper_copies_ticked_groups_only() {
        let mut app = App::headless();
        let a = rect(&mut app, Rect::new(0.0, 0.0, 10.0, 20.0));
        let b = rect(&mut app, Rect::new(40.0, 0.0, 50.0, 10.0));
        app.run(Command::SetFill {
            shapes: vec![a],
            fill: Fill::Solid(Color::rgb8(0, 0, 250)),
        });
        app.run(Command::SetOpacity {
            shapes: vec![a],
            opacity: 0.5,
        });
        app.select(Vec::new());
        app.eyedropper_click(Point::new(5.0, 5.0), true, false);
        assert!(app.eyedropper_attrs.is_some());
        // Defaults: fill and outline, no transformations, no effects.
        app.eyedropper_click(Point::new(45.0, 5.0), true, false);
        let sb = app.doc().find_shape(b).expect("b");
        assert_eq!(sb.fill, Fill::Solid(Color::rgb8(0, 0, 250)));
        assert!((sb.opacity - 1.0).abs() < 1e-9);
        assert!((sb.bounds().width() - 10.0).abs() < 1e-9);
        // Tick size and effects: b takes a's size about its own centre.
        app.eyedropper_groups.size = true;
        app.eyedropper_groups.effects = true;
        app.eyedropper_click(Point::new(45.0, 5.0), true, false);
        let sb = app.doc().find_shape(b).expect("b");
        let bb = sb.bounds();
        assert!((bb.height() - 20.0).abs() < 1e-6, "{bb:?}");
        assert!((bb.center().x - 45.0).abs() < 1e-6, "{bb:?}");
        assert!((sb.opacity - 0.5).abs() < 1e-9);
        // Shift+click samples again instead of applying.
        app.eyedropper_click(Point::new(45.0, 5.0), true, true);
        let s = app.eyedropper_attrs.as_ref().expect("sample");
        assert!((s.height - 20.0).abs() < 1e-6);
    }

    #[test]
    fn rotation_of_reads_the_transform_angle() {
        let t = Affine::rotate(0.3);
        assert!((rotation_of(t) - 0.3).abs() < 1e-9);
    }

    /// The acceptance of the sample-and-pour cycle: pipette on red, click
    /// on blue makes it red in one undo step; the right button on green
    /// gives it a red outline and keeps its fill; Shift+click resamples
    /// without changing the drawing; Alt switches pipette and bucket
    /// without an undo step; Esc empties the bucket.
    #[test]
    fn sample_and_pour_cycle() {
        let mut app = App::headless();
        app.tool = Tool::ColorEyedropper;
        let r = rect(&mut app, Rect::new(0.0, 0.0, 10.0, 10.0));
        let b = rect(&mut app, Rect::new(20.0, 0.0, 30.0, 10.0));
        let g = rect(&mut app, Rect::new(40.0, 0.0, 50.0, 10.0));
        solid(&mut app, r, RED);
        solid(&mut app, b, BLUE);
        solid(&mut app, g, GREEN);
        app.select(Vec::new());
        let (red_at, blue_at, green_at) = (
            Point::new(5.0, 5.0),
            Point::new(25.0, 5.0),
            Point::new(45.0, 5.0),
        );
        assert!(app.eyedropper_sampling(false));
        app.eyedropper_gesture(red_at, false, click());
        assert!(
            !app.eyedropper_sampling(false),
            "the pipette became a bucket"
        );
        let before = undo_len(&app);
        app.eyedropper_gesture(blue_at, false, click());
        assert_eq!(app.doc().find_shape(b).expect("b").fill, Fill::Solid(RED));
        assert_eq!(undo_len(&app), before + 1, "one undo step");
        assert!(app.eyedrop.ripple.is_some(), "apply highlight");
        // Right button: the outline, the fill stays.
        app.eyedropper_gesture(
            green_at,
            false,
            EyeClick {
                right: true,
                ..click()
            },
        );
        let sg = app.doc().find_shape(g).expect("g");
        assert_eq!(sg.fill, Fill::Solid(GREEN));
        assert_eq!(sg.stroke.as_ref().map(|s| s.color), Some(RED));
        // Shift+click samples green without changing the drawing.
        let undo = undo_len(&app);
        app.eyedropper_gesture(
            green_at,
            false,
            EyeClick {
                shift: true,
                ..click()
            },
        );
        assert_eq!(undo_len(&app), undo);
        assert_eq!(app.eyedropper_color, Some(GREEN));
        assert!(!app.eyedropper_sampling(false), "the bucket stays");
        assert_eq!(app.eyedrop.recent.len(), 2);
        // Alt twice: pipette, then bucket, no undo step.
        let alt = EyeClick {
            alt: true,
            ..click()
        };
        app.eyedropper_gesture(blue_at, false, alt);
        assert!(app.eyedropper_sampling(false));
        app.eyedropper_gesture(blue_at, false, alt);
        assert!(!app.eyedropper_sampling(false));
        assert_eq!(undo_len(&app), undo, "switching applies nothing");
        // Esc empties the bucket and keeps the tool; the next Esc leaves.
        assert!(app.eyedropper_escape());
        assert!(app.eyedropper_color.is_none() && app.eyedropper_sampling(false));
        assert!(!app.eyedropper_escape());
    }

    #[test]
    fn open_lines_take_the_colour_on_their_outline_and_bare_objects_get_the_default_outline() {
        let mut app = App::headless();
        app.tool = Tool::ColorEyedropper;
        let mut path = graphdraw_core::geometry::BezPath::new();
        path.move_to((0.0, 5.0));
        path.line_to((30.0, 5.0));
        let line = app
            .new_shape(ShapeKind::Path {
                path,
                closed: false,
            })
            .expect("line");
        let r = rect(&mut app, Rect::new(0.0, 20.0, 10.0, 30.0));
        app.run(Command::SetStroke {
            shapes: vec![r],
            stroke: None,
        });
        app.select(Vec::new());
        app.eyedrop_load_color(BLUE);
        app.eyedropper_gesture(Point::new(15.0, 5.0), false, click());
        let sl = app.doc().find_shape(line).expect("line");
        assert_eq!(sl.stroke.as_ref().map(|s| s.color), Some(BLUE));
        assert_eq!(sl.fill, Fill::None);
        // No outline: the default outline of new objects in blue.
        app.eyedropper_gesture(
            Point::new(5.0, 25.0),
            false,
            EyeClick {
                right: true,
                ..click()
            },
        );
        let sr = app.doc().find_shape(r).expect("r");
        let want = app.default_stroke().map(|s| s.width).unwrap_or(0.0);
        let st = sr.stroke.as_ref().expect("outline");
        assert_eq!(st.color, BLUE);
        assert!((st.width - want).abs() < 1e-9);
    }

    #[test]
    fn ctrl_reaches_inside_a_group_otherwise_the_whole_group_takes_it() {
        let mut app = App::headless();
        app.tool = Tool::ColorEyedropper;
        let a = rect(&mut app, Rect::new(0.0, 0.0, 10.0, 10.0));
        let b = rect(&mut app, Rect::new(20.0, 0.0, 30.0, 10.0));
        solid(&mut app, a, GREEN);
        solid(&mut app, b, GREEN);
        app.select(vec![a, b]);
        app.group_selection();
        app.select(Vec::new());
        app.eyedrop_load_color(RED);
        app.eyedropper_gesture(
            Point::new(5.0, 5.0),
            false,
            EyeClick {
                ctrl: true,
                ..click()
            },
        );
        assert_eq!(app.doc().find_shape(a).expect("a").fill, Fill::Solid(RED));
        assert_eq!(app.doc().find_shape(b).expect("b").fill, Fill::Solid(GREEN));
        app.eyedrop_load_color(BLUE);
        app.eyedropper_gesture(Point::new(5.0, 5.0), false, click());
        assert_eq!(app.doc().find_shape(a).expect("a").fill, Fill::Solid(BLUE));
        assert_eq!(app.doc().find_shape(b).expect("b").fill, Fill::Solid(BLUE));
    }

    fn red_blue() -> Fountain {
        Fountain::two(FountainKind::Linear, RED, BLUE, 0.0)
    }

    #[test]
    fn a_click_on_a_fountain_reads_the_exact_point_and_alt_clones_the_ramp() {
        let mut app = App::headless();
        app.tool = Tool::ColorEyedropper;
        let src = rect(&mut app, Rect::new(0.0, 0.0, 100.0, 100.0));
        let dst = rect(&mut app, Rect::new(200.0, 0.0, 600.0, 200.0));
        app.run(Command::SetFill {
            shapes: vec![src],
            fill: Fill::Fountain(red_blue()),
        });
        app.select(Vec::new());
        // The middle of the ramp: as much red as blue.
        let c = app
            .sample_color_at(Point::new(50.0, 50.0), false)
            .expect("c");
        let [r, _, b] = c.to_rgb8();
        assert!((r as i32 - b as i32).abs() < 12, "{c:?}");
        // Near the start: red.
        let c = app
            .sample_color_at(Point::new(1.0, 50.0), false)
            .expect("c");
        assert!(c.to_rgb8()[0] > 240, "{c:?}");
        // Alt+click clones the ramp; a click pours it on the big rectangle,
        // whose own bounds carry the axis from edge to edge.
        app.eyedropper_gesture(
            Point::new(50.0, 50.0),
            false,
            EyeClick {
                alt: true,
                ..click()
            },
        );
        assert!(app.eyedrop.ramp.is_some());
        app.eyedropper_gesture(Point::new(400.0, 100.0), false, click());
        let sd = app.doc().find_shape(dst).expect("dst");
        let Fill::Fountain(f) = &sd.fill else {
            panic!("fountain");
        };
        assert_eq!(f, &red_blue());
        let pts = stop_points(f, sd.bounds());
        assert!((pts[0].1.x - 200.0).abs() < 1e-9 && (pts[1].1.x - 600.0).abs() < 1e-9);
        // The right button with a ramp: the ramp only goes to fills.
        let undo = undo_len(&app);
        app.eyedropper_gesture(
            Point::new(400.0, 100.0),
            false,
            EyeClick {
                right: true,
                ..click()
            },
        );
        assert_eq!(undo_len(&app), undo);
    }

    #[test]
    fn the_colour_bucket_on_a_stop_changes_only_that_stop() {
        let mut app = App::headless();
        app.tool = Tool::ColorEyedropper;
        let r = rect(&mut app, Rect::new(0.0, 0.0, 100.0, 100.0));
        app.run(Command::SetFill {
            shapes: vec![r],
            fill: Fill::Fountain(red_blue()),
        });
        app.select(Vec::new());
        app.eyedrop_load_color(GREEN);
        // The end stop sits on the right edge, half way up.
        assert_eq!(app.fountain_stop_at(Point::new(99.5, 50.0)), Some((r, 1)));
        let undo = undo_len(&app);
        app.eyedropper_gesture(Point::new(99.5, 50.0), false, click());
        let Fill::Fountain(f) = &app.doc().find_shape(r).expect("r").fill else {
            panic!("fountain");
        };
        assert_eq!(f.stops[0].color, RED);
        assert_eq!(f.stops[1].color, GREEN);
        assert_eq!(undo_len(&app), undo + 1);
        // Away from the stops the whole fill becomes the colour.
        app.eyedropper_gesture(Point::new(50.0, 50.0), false, click());
        assert_eq!(app.doc().find_shape(r).expect("r").fill, Fill::Solid(GREEN));
    }

    #[test]
    fn stop_points_follow_inversion_and_edge_pad() {
        let mut f = red_blue();
        f.stops.push(Stop::new(0.5, GREEN));
        let b = Rect::new(0.0, 0.0, 100.0, 10.0);
        let pts = stop_points(&f, b);
        assert!((pts[2].1.x - 50.0).abs() < 1e-9);
        f.invert = true;
        assert!((stop_points(&f, b)[0].1.x - 100.0).abs() < 1e-9);
        f.invert = false;
        f.edge_pad = 0.25;
        assert!((stop_points(&f, b)[0].1.x - 25.0).abs() < 1e-9);
        // The point of a stop reads back as its own place on the ramp.
        let f = red_blue();
        for (i, p) in stop_points(&f, b) {
            let t = fountain_t(&f, b, p).expect("t");
            assert!((t - f.stops[i].pos).abs() < 1e-9);
        }
    }

    #[test]
    fn text_takes_the_colour_on_the_whole_object() {
        let mut app = App::headless();
        app.tool = Tool::ColorEyedropper;
        let mut span = TextSpan::new("Ab", "Sans", 24.0);
        span.fill = Some(Fill::Solid(GREEN));
        let t = app
            .new_shape(ShapeKind::Text {
                spans: vec![span],
                origin: Point::new(0.0, 0.0),
                frame: None,
                align: Default::default(),
                para: Default::default(),
                on_path: None,
            })
            .expect("text");
        assert!(app.eyedrop_fill(t, Fill::Solid(RED)));
        let s = app.doc().find_shape(t).expect("t");
        assert_eq!(s.fill, Fill::Solid(RED));
        let ShapeKind::Text { spans, .. } = &s.kind else {
            panic!("text");
        };
        assert!(spans.iter().all(|sp| sp.fill.is_none()));
        // One undo step brings both back.
        let _ = app.engine.undo();
        let s = app.doc().find_shape(t).expect("t");
        let ShapeKind::Text { spans, .. } = &s.kind else {
            panic!("text");
        };
        assert_eq!(spans[0].fill, Some(Fill::Solid(GREEN)));
    }

    #[test]
    fn attributes_copy_typography_and_skew_and_never_the_source_itself() {
        let mut app = App::headless();
        app.tool = Tool::AttributesEyedropper;
        let text = |app: &mut App, family: &str, x: f64| {
            let mut span = TextSpan::new("Ab", family, 12.0);
            span.tracking_pct = 0.0;
            app.new_shape(ShapeKind::Text {
                spans: vec![span],
                origin: Point::new(x, 0.0),
                frame: None,
                align: Default::default(),
                para: Default::default(),
                on_path: None,
            })
            .expect("text")
        };
        let a = text(&mut app, "Serif", 0.0);
        let b = text(&mut app, "Sans", 100.0);
        if let Some(ShapeKind::Text {
            mut spans,
            origin,
            frame,
            align,
            mut para,
            on_path,
        }) = app.doc().find_shape(a).map(|s| s.kind.clone())
        {
            spans[0].tracking_pct = 25.0;
            spans[0].size_pt = 30.0;
            para.leading_pct = 150.0;
            app.run(Command::SetShapeKind {
                shape: a,
                kind: ShapeKind::Text {
                    spans,
                    origin,
                    frame,
                    align,
                    para,
                    on_path,
                },
            });
        }
        app.run(Command::TransformShapes {
            shapes: vec![a],
            transform: Affine::skew(0.4, 0.0),
        });
        let mut attrs = app
            .sample_attrs_at(app.doc().find_shape(a).expect("a").bounds().center())
            .expect("attrs");
        assert_eq!(attrs.source, Some(a));
        assert!((attrs.skew - 0.4).abs() < 1e-9, "{}", attrs.skew);
        // The source is never a target of its own sample.
        assert!(!app.apply_attrs_to(a, &attrs, AttrGroups::default()));
        let groups = AttrGroups {
            skew: true,
            ..AttrGroups::default()
        };
        attrs.source = Some(a);
        assert!(app.apply_attrs_to(b, &attrs, groups));
        let sb = app.doc().find_shape(b).expect("b");
        let ShapeKind::Text { spans, para, .. } = &sb.kind else {
            panic!("text");
        };
        assert_eq!(spans[0].font_family, "Serif");
        assert!((spans[0].size_pt - 30.0).abs() < 1e-9);
        assert!((spans[0].tracking_pct - 25.0).abs() < 1e-9);
        assert!((para.leading_pct - 150.0).abs() < 1e-9);
        assert_eq!(spans[0].text, "Ab");
        assert!((skew_of(sb.transform) - 0.4).abs() < 1e-9);
    }

    #[test]
    fn skew_delta_keeps_rotation_scale_and_centre() {
        let t = Affine::rotate(0.5) * Affine::scale_non_uniform(2.0, 3.0);
        let c = Point::new(10.0, 20.0);
        let d = skew_delta(t, 0.25, c);
        let n = d * t;
        assert!((skew_of(n) - 0.25).abs() < 1e-9);
        assert!((rotation_of(n) - 0.5).abs() < 1e-9);
        assert!(((d * c) - c).hypot() < 1e-9);
    }

    #[test]
    fn grid_and_average() {
        // 3 x 1 pixels: opaque black, opaque white, transparent.
        let px = [
            Color32::from_rgba_premultiplied(0, 0, 0, 255),
            Color32::from_rgba_premultiplied(255, 255, 255, 255),
            Color32::TRANSPARENT,
        ];
        let g = grid_from(&px, [3, 1], 1, 0, 3, |_, _| [255, 0, 0]);
        assert_eq!(g[4], [255, 255, 255], "middle");
        assert_eq!(g[3], [0, 0, 0]);
        assert_eq!(g[5], [255, 0, 0], "transparent shows the background");
        assert_eq!(g[0], [255, 0, 0], "outside shows the background");
        assert_eq!(average(&g, 3, 1), Some(Color::rgb8(255, 255, 255)));
        let avg = average(&g, 3, 3).expect("avg").to_rgb8();
        assert_eq!(avg[0], ((255.0 * 7.0 + 255.0) / 9.0f64).round() as u8);
        // An even box starts at the middle and grows right and down.
        let two = average(&g, 3, 2).expect("avg").to_rgb8();
        // White and three background pixels.
        assert_eq!(two, [255, 64, 64]);
        assert_eq!(average(&g, 3, 4), None);
    }

    #[test]
    fn recent_tones_keep_five_without_repeats() {
        let mut e = EyedropState::default();
        for i in 0..7u8 {
            e.push_recent(Color::rgb8(i * 20, 0, 0));
        }
        e.push_recent(Color::rgb8(80, 0, 0));
        assert_eq!(e.recent.len(), RECENT);
        assert_eq!(e.recent[0], Color::rgb8(80, 0, 0));
        let hexes: std::collections::HashSet<_> = e.recent.iter().map(|c| c.to_hex()).collect();
        assert_eq!(hexes.len(), RECENT);
    }

    #[test]
    fn the_magnifier_waits_after_a_rest_and_follows_again_far_away() {
        let mut h = Hud::default();
        let bounds = ERect::from_min_size(Pos2::ZERO, EVec2::splat(1000.0));
        let size = EVec2::new(120.0, 200.0);
        h.follow(Pos2::new(100.0, 100.0), 0.0, size, bounds);
        assert_eq!(h.pos, Pos2::new(100.0 + HUD_OFFSET, 100.0 + HUD_OFFSET));
        h.follow(Pos2::new(100.0, 100.0), 1.0, size, bounds);
        assert!(h.frozen);
        let at = h.pos;
        // Going to it: it stays.
        h.follow(at + EVec2::splat(10.0), 1.1, size, bounds);
        assert_eq!(h.pos, at);
        // Far away: it follows again.
        h.follow(Pos2::new(600.0, 600.0), 1.2, size, bounds);
        assert!(!h.frozen);
        assert_ne!(h.pos, at);
        // Near the edge: the other side of the pointer.
        h.follow(Pos2::new(990.0, 990.0), 1.3, size, bounds);
        assert!(h.pos.x < 990.0 && h.pos.y < 990.0);
    }

    #[test]
    fn the_ripple_fades_out() {
        let mut e = EyedropState {
            ripple: Some((vec![ShapeId(1)], 1.0)),
            now: 1.0,
            ..Default::default()
        };
        assert_eq!(e.ripple_phase(), Some(0.0));
        e.now = 1.0 + RIPPLE_S / 2.0;
        assert!((e.ripple_phase().expect("phase") - 0.5).abs() < 1e-9);
        e.now = 1.0 + RIPPLE_S * 1.01;
        assert_eq!(e.ripple_phase(), None);
    }

    #[test]
    fn values_text_shows_cmyk_and_rgb() {
        let (cmyk, rgb) = values_text(Color::cmyk_pct(0.0, 100.0, 100.0, 0.0));
        assert_eq!(cmyk, "C:0 M:100 Y:100 K:0");
        assert!(rgb.starts_with("R:"));
        let (_, rgb) = values_text(Color::rgb8(1, 2, 3));
        assert_eq!(rgb, "R:1 G:2 B:3");
    }

    #[test]
    fn switching_between_the_eyedroppers_empties_the_bucket() {
        let mut app = App::headless();
        app.set_tool(Tool::ColorEyedropper);
        app.eyedrop_load_color(RED);
        app.set_tool(Tool::AttributesEyedropper);
        assert!(app.eyedropper_color.is_none() && !app.eyedropper_apply);
    }

    #[test]
    fn a_bitmap_gives_its_pixel() {
        let mut app = App::headless();
        let mut img = image::RgbaImage::new(2, 2);
        for (x, y, p) in img.enumerate_pixels_mut() {
            *p = if x == 0 {
                image::Rgba([255, 0, 0, 255])
            } else {
                image::Rgba([0, 0, 255, 255])
            };
            let _ = y;
        }
        let mut png = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .expect("png");
        app.new_shape(ShapeKind::Bitmap {
            rect: Rect::new(0.0, 0.0, 20.0, 20.0),
            width_px: 2,
            height_px: 2,
            png,
            fx: Default::default(),
        })
        .expect("bitmap");
        app.select(Vec::new());
        assert_eq!(app.sample_color_at(Point::new(5.0, 5.0), false), Some(RED));
        assert_eq!(
            app.sample_color_at(Point::new(15.0, 5.0), false),
            Some(BLUE)
        );
        app.eyedropper_sample = 2;
        let c = app.sample_color_at(Point::new(5.0, 5.0), false).expect("c");
        assert_eq!(c, Color::rgb8(127, 0, 127));
    }
}
