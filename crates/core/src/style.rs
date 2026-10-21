//! Fill and outline (stroke) properties of a shape.

use crate::color::Color;
use crate::geometry::{Affine, Point, Rect, Size};
use serde::{Deserialize, Serialize};

/// Fountain (gradient) fill geometry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum FountainKind {
    #[default]
    Linear,
    Radial,
    Conical,
    Square,
    /// Follows the object's outline: 0 on its edge, 1 at the point
    /// deepest inside it.
    Shaped,
    /// An ellipse fitted to the bounds (radii `w/2 * sqrt 2` and
    /// `h/2 * sqrt 2 * aspect`), turned by the angle.
    Elliptical,
}

impl FountainKind {
    /// Every kind, in the order the fill editor lists them.
    pub const ALL: [FountainKind; 6] = [
        FountainKind::Linear,
        FountainKind::Radial,
        FountainKind::Conical,
        FountainKind::Square,
        FountainKind::Shaped,
        FountainKind::Elliptical,
    ];
}

/// How a fountain runs its colours over its span.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum FountainRepeat {
    /// Once; the end colours hold beyond the span.
    #[default]
    None,
    /// Sawtooth: each cycle starts again at the first colour.
    Repeat,
    /// Triangular: cycles run there and back.
    Mirror,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Stop {
    /// 0.0 at the start, 1.0 at the end.
    pub pos: f64,
    pub color: Color,
    /// Opacity of the colour, 0..1 (1 = opaque).
    #[serde(default = "one_f64", skip_serializing_if = "is_one_f64")]
    pub opacity: f64,
    /// Where, in 0..1 of the run to the next stop, the two colours mix
    /// half and half (0.5 = halfway).
    #[serde(default = "half_f64", skip_serializing_if = "is_half_f64")]
    pub midpoint: f64,
}

impl Stop {
    /// An opaque stop with its midpoint halfway.
    pub const fn new(pos: f64, color: Color) -> Stop {
        Stop {
            pos,
            color,
            opacity: 1.0,
            midpoint: 0.5,
        }
    }
}

impl Default for Stop {
    fn default() -> Self {
        Stop::new(0.0, Color::BLACK)
    }
}

fn one_f64() -> f64 {
    1.0
}

fn is_one_f64(v: &f64) -> bool {
    *v == 1.0
}

fn half_f64() -> f64 {
    0.5
}

fn is_half_f64(v: &f64) -> bool {
    *v == 0.5
}

/// How the colours run from one stop to the next.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum FountainBlend {
    /// Straight between the colours (RGB).
    #[default]
    Direct,
    /// Around the colour wheel, hue increasing.
    Clockwise,
    /// Around the colour wheel, hue decreasing.
    Counterclockwise,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Fountain {
    pub kind: FountainKind,
    /// At least two stops, sorted by position.
    pub stops: Vec<Stop>,
    /// Degrees (0 = left to right, counter-clockwise positive).
    pub angle: f64,
    /// Centre offset in bounds-relative units (-1..1), for radial/conical/square.
    pub offset: Point,
    /// Edge padding in 0..0.49: fraction of the span held at the end colours.
    pub edge_pad: f64,
    /// Repeat mode over the span.
    #[serde(default, skip_serializing_if = "is_default")]
    pub repeat: FountainRepeat,
    /// Cycles across the span when `repeat` is not None (1 to
    /// [`Fountain::MAX_REPEATS`]).
    #[serde(default = "one_u32", skip_serializing_if = "is_one_u32")]
    pub repeats: u32,
    /// Mirrored about the centre: the colours run first to last and back.
    #[serde(default, skip_serializing_if = "is_default")]
    pub bilinear: bool,
    /// Colours in reverse order.
    #[serde(default, skip_serializing_if = "is_default")]
    pub invert: bool,
    /// Size of the span against the automatic one (1 = fitted to the
    /// bounds): the length of a linear axis, the radius of the others.
    #[serde(default = "one_f64", skip_serializing_if = "is_one_f64")]
    pub scale: f64,
    /// Linear: the axis centre along its direction, in half lengths of
    /// the automatic axis (0 = the bounds' centre).
    #[serde(default, skip_serializing_if = "is_default")]
    pub axis_offset: f64,
    /// Elliptical and square: the y radius over the x radius.
    #[serde(default = "one_f64", skip_serializing_if = "is_one_f64")]
    pub aspect: f64,
    /// Bands of one colour each; 0 = continuous.
    #[serde(default, skip_serializing_if = "is_default")]
    pub steps: u32,
    /// Dithered on screen and in pictures, against visible banding.
    #[serde(default, skip_serializing_if = "is_default")]
    pub smooth: bool,
    /// How the colours run from one stop to the next.
    #[serde(default, skip_serializing_if = "is_default")]
    pub blend: FountainBlend,
}

fn is_default<T: Default + PartialEq>(v: &T) -> bool {
    *v == T::default()
}

fn one_u32() -> u32 {
    1
}

fn is_one_u32(v: &u32) -> bool {
    *v == 1
}

impl Default for Fountain {
    /// Black to white, left to right, running once.
    fn default() -> Self {
        Fountain::two(FountainKind::Linear, Color::BLACK, Color::WHITE, 0.0)
    }
}

impl Fountain {
    /// Most cycles a repeating fountain runs across its span.
    pub const MAX_REPEATS: u32 = 50;

    pub fn two(kind: FountainKind, from: Color, to: Color, angle: f64) -> Self {
        Fountain {
            kind,
            stops: vec![Stop::new(0.0, from), Stop::new(1.0, to)],
            angle,
            offset: Point::ZERO,
            edge_pad: 0.0,
            repeat: FountainRepeat::None,
            repeats: 1,
            bilinear: false,
            invert: false,
            scale: 1.0,
            axis_offset: 0.0,
            aspect: 1.0,
            steps: 0,
            smooth: false,
            blend: FountainBlend::Direct,
        }
    }

    pub fn first_color(&self) -> Color {
        self.stops.first().map(|s| s.color).unwrap_or(Color::BLACK)
    }

    pub fn last_color(&self) -> Color {
        self.stops.last().map(|s| s.color).unwrap_or(Color::WHITE)
    }

    /// Cycles across the span (1 unless the fountain repeats).
    pub fn cycles(&self) -> u32 {
        match self.repeat {
            FountainRepeat::None => 1,
            _ => self.repeats.clamp(1, Self::MAX_REPEATS),
        }
    }

    /// No repeat, bilinear or invert: the stops run once over the span.
    pub fn runs_once(&self) -> bool {
        self.cycles() == 1 && !self.bilinear && !self.invert
    }

    /// The stops sorted by position, at least two.
    pub fn sorted_stops(&self) -> Vec<Stop> {
        let mut v = self.stops.clone();
        v.sort_by(|a, b| {
            a.pos
                .partial_cmp(&b.pos)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        if v.len() < 2 {
            let c = v.first().map(|s| s.color).unwrap_or(Color::BLACK);
            v = vec![Stop::new(0.0, c), Stop::new(1.0, c)];
        }
        v
    }

    /// Where in the stops the colour at `t` (0..1 across the span) is
    /// taken: the bilinear mirror, then the repeat cycles, the edge pad
    /// and the inversion.
    pub fn stop_param(&self, t: f64) -> f64 {
        let mut u = if t.is_finite() {
            t.clamp(0.0, 1.0)
        } else {
            0.0
        };
        if self.bilinear {
            u = 1.0 - (2.0 * u - 1.0).abs();
        }
        let n = self.cycles() as f64;
        if n > 1.0 {
            let x = u * n;
            let k = x.floor();
            let fr = x - k;
            u = match self.repeat {
                FountainRepeat::Mirror if (k as i64) % 2 == 1 => 1.0 - fr,
                FountainRepeat::Mirror => fr,
                _ if x >= n => 1.0,
                _ => fr,
            };
        }
        let e = self.edge_pad.clamp(0.0, 0.49);
        if e > 0.0 {
            u = ((u - e) / (1.0 - 2.0 * e)).clamp(0.0, 1.0);
        }
        if self.invert {
            u = 1.0 - u;
        }
        u
    }

    /// Colour at `t` in 0..1 (sRGB interpolation; the size, axis offset,
    /// midpoints, steps and blending of `crate::gradient` in).
    pub fn color_at(&self, t: f64) -> Color {
        if self.has_extras() {
            return self.color_alpha_at(t).0;
        }
        sample_stops(&self.sorted_stops(), self.stop_param(t))
    }

    /// The fountain as plain stops over 0..1 that give the same colours
    /// in one run, with the edge pad, repeat cycles, bilinear mirror and
    /// inversion worked in (for gradient shaders and exports that only
    /// know a single run of stops). Equal positions are hard edges. The
    /// size and axis offset are worked in over the automatic span, and
    /// the midpoints, steps and blending into the stops; the stops keep
    /// their opacity.
    pub fn resolved_stops(&self) -> Vec<Stop> {
        if self.has_span_extras()
            || self.steps > 0
            || self.blend != FountainBlend::Direct
            || self.stops.iter().any(|s| s.midpoint != 0.5)
        {
            return self.sampled_stops();
        }
        let base = self.sorted_stops();
        let cycles = self.cycles();
        let n = cycles as f64;
        let e = self.edge_pad.clamp(0.0, 0.49);
        // Where the mapping from t to the stop parameter bends.
        let mut us = Vec::new();
        for k in 0..=cycles {
            let k = k as f64;
            us.push(k / n);
            if e > 0.0 && k < n {
                us.push((k + e) / n);
                us.push((k + 1.0 - e) / n);
            }
        }
        let mut ts: Vec<f64> = Vec::new();
        for u in us {
            if self.bilinear {
                ts.push(u / 2.0);
                ts.push(1.0 - u / 2.0);
            } else {
                ts.push(u);
            }
        }
        ts.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        ts.dedup_by(|a, b| (*a - *b).abs() < 1e-12);
        let mut out: Vec<Stop> = Vec::new();
        let mut push = |pos: f64, color: Color, opacity: f64| {
            if let Some(l) = out.last() {
                if (l.pos - pos).abs() < 1e-12 && l.color == color && l.opacity == opacity {
                    return;
                }
            }
            out.push(Stop {
                opacity,
                ..Stop::new(pos, color)
            });
        };
        for w in ts.windows(2) {
            let (a, b) = (w[0], w[1]);
            if b - a < 1e-12 {
                continue;
            }
            // The mapping is straight between bends: its ends from two
            // samples inside, so a sawtooth's jump lands on the right side.
            let (q1, q3) = (a + (b - a) * 0.25, a + (b - a) * 0.75);
            let (p1, p3) = (self.stop_param(q1), self.stop_param(q3));
            let slope = (p3 - p1) / (q3 - q1);
            let pa = (p1 - slope * (q1 - a)).clamp(0.0, 1.0);
            let pb = (p3 + slope * (b - q3)).clamp(0.0, 1.0);
            push(a, sample_stops(&base, pa), sample_opacity(&base, pa));
            if (pb - pa).abs() > 1e-12 {
                let (lo, hi) = (pa.min(pb), pa.max(pb));
                let mut inner: Vec<Stop> = base
                    .iter()
                    .filter(|s| s.pos > lo + 1e-9 && s.pos < hi - 1e-9)
                    .map(|s| Stop {
                        pos: a + (s.pos - pa) / (pb - pa) * (b - a),
                        ..*s
                    })
                    .collect();
                if pb < pa {
                    inner.reverse();
                }
                for s in inner {
                    push(s.pos, s.color, s.opacity);
                }
            }
            push(b, sample_stops(&base, pb), sample_opacity(&base, pb));
        }
        if out.len() < 2 {
            return base;
        }
        out
    }

    /// The closest fountain that runs once with no edge pad and one of
    /// the four geometric kinds, for formats that know nothing more: the
    /// options are worked into the stops (at most `max_stops`), and a
    /// shaped fountain becomes a square one running from the centre out.
    /// A fountain with none of the options comes back unchanged.
    pub fn to_plain(&self, max_stops: usize) -> Fountain {
        let extras = self.has_span_extras()
            || self.steps > 0
            || self.blend != FountainBlend::Direct
            || self.stops.iter().any(|s| s.midpoint != 0.5);
        if self.runs_once()
            && !extras
            && !matches!(self.kind, FountainKind::Shaped | FountainKind::Elliptical)
        {
            return self.clone();
        }
        let mut stops = self.resolved_stops();
        if self.kind == FountainKind::Shaped {
            // Shaped: 0 at the edge; square: 0 at the centre.
            stops.reverse();
            for s in &mut stops {
                s.pos = 1.0 - s.pos;
            }
        }
        let max_stops = max_stops.max(2);
        if stops.len() > max_stops {
            let k = (max_stops - 1) as f64;
            stops = (0..max_stops)
                .map(|i| {
                    let pos = i as f64 / k;
                    Stop {
                        opacity: sample_opacity(&stops, pos),
                        ..Stop::new(pos, sample_stops(&stops, pos))
                    }
                })
                .collect();
        }
        Fountain {
            kind: match self.kind {
                FountainKind::Shaped => FountainKind::Square,
                // The nearest kind formats know: a circle.
                FountainKind::Elliptical => FountainKind::Radial,
                k => k,
            },
            stops,
            angle: self.angle,
            offset: self.offset,
            edge_pad: 0.0,
            repeat: FountainRepeat::None,
            repeats: 1,
            bilinear: false,
            invert: false,
            ..Fountain::default()
        }
    }
}

/// Opacity at `t` along `stops` (sorted by position), straight between
/// the two around it.
pub fn sample_opacity(stops: &[Stop], t: f64) -> f64 {
    let Some(first) = stops.first() else {
        return 1.0;
    };
    if t <= first.pos {
        return first.opacity;
    }
    for w in stops.windows(2) {
        if t <= w[1].pos {
            let span = (w[1].pos - w[0].pos).max(1e-9);
            let k = ((t - w[0].pos) / span).clamp(0.0, 1.0);
            return w[0].opacity + (w[1].opacity - w[0].opacity) * k;
        }
    }
    stops.last().map(|s| s.opacity).unwrap_or(1.0)
}

/// Colour at `t` along `stops` (sorted by position): the two around it
/// mixed in sRGB, the end colours beyond them.
pub fn sample_stops(stops: &[Stop], t: f64) -> Color {
    let Some(first) = stops.first() else {
        return Color::BLACK;
    };
    if t <= first.pos {
        return first.color;
    }
    for w in stops.windows(2) {
        if t <= w[1].pos {
            let span = (w[1].pos - w[0].pos).max(1e-9);
            let k = ((t - w[0].pos) / span).clamp(0.0, 1.0) as f32;
            return lerp_color(w[0].color, w[1].color, k);
        }
    }
    stops.last().map(|s| s.color).unwrap_or(first.color)
}

pub fn lerp_color(a: Color, b: Color, t: f32) -> Color {
    let [r1, g1, b1] = a.to_rgb8();
    let [r2, g2, b2] = b.to_rgb8();
    let l = |x: u8, y: u8| {
        (x as f32 + (y as f32 - x as f32) * t)
            .round()
            .clamp(0.0, 255.0) as u8
    };
    Color::rgb8(l(r1, r2), l(g1, g2), l(b1, b2))
}

/// Two-colour pattern tiles (the two-colour pattern fill): the built-in
/// drawings, or a grid of cells the user set one by one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum PatternTile {
    #[default]
    Checker,
    Dots,
    Stripes,
    DiagonalStripes,
    Grid,
    Bricks,
    Crosshatch,
    Waves,
    /// [`PatternTile::CELLS`] by [`PatternTile::CELLS`] cells, one byte per
    /// row from the top; in each byte the highest bit is the left cell and
    /// a set bit is the front colour.
    Cells([u8; 8]),
}

impl PatternTile {
    pub const ALL: [PatternTile; 8] = [
        PatternTile::Checker,
        PatternTile::Dots,
        PatternTile::Stripes,
        PatternTile::DiagonalStripes,
        PatternTile::Grid,
        PatternTile::Bricks,
        PatternTile::Crosshatch,
        PatternTile::Waves,
    ];

    pub fn name(self) -> &'static str {
        match self {
            PatternTile::Checker => "Checker",
            PatternTile::Dots => "Dots",
            PatternTile::Stripes => "Stripes",
            PatternTile::DiagonalStripes => "Diagonal stripes",
            PatternTile::Grid => "Grid",
            PatternTile::Bricks => "Bricks",
            PatternTile::Crosshatch => "Crosshatch",
            PatternTile::Waves => "Waves",
            PatternTile::Cells(_) => "Cells",
        }
    }

    /// Cells on each side of an editable tile.
    pub const CELLS: usize = 8;

    /// The tile as cells (row 0 at the top): a built-in drawing sampled
    /// at each cell's centre, or the cells themselves.
    pub fn cells(self) -> [u8; 8] {
        if let PatternTile::Cells(rows) = self {
            return rows;
        }
        let n = Self::CELLS as f64;
        let mut rows = [0u8; 8];
        for (y, row) in rows.iter_mut().enumerate() {
            for x in 0..Self::CELLS {
                let u = (x as f64 + 0.5) / n;
                let v = 1.0 - (y as f64 + 0.5) / n;
                if self.front(u, v) {
                    *row |= 0x80 >> x;
                }
            }
        }
        rows
    }

    /// Is the cell in column `x`, row `y` (from the top) the front colour?
    /// Cells outside the grid are the back colour.
    pub fn cell(self, x: usize, y: usize) -> bool {
        x < Self::CELLS && self.cells().get(y).is_some_and(|r| r & (0x80 >> x) != 0)
    }

    /// The tile with the cell in column `x`, row `y` swapped between front
    /// and back; a built-in drawing becomes cells first. A cell outside
    /// the grid leaves the tile as it is.
    pub fn toggled(self, x: usize, y: usize) -> PatternTile {
        if x >= Self::CELLS || y >= Self::CELLS {
            return self;
        }
        let mut rows = self.cells();
        if let Some(r) = rows.get_mut(y) {
            *r ^= 0x80 >> x;
        }
        PatternTile::Cells(rows)
    }

    /// Is the tile pixel at (u, v) in 0..1 "front" (true) or "back" colour?
    pub fn front(self, u: f64, v: f64) -> bool {
        match self {
            PatternTile::Cells(rows) => {
                let n = Self::CELLS as f64;
                let x = (u.rem_euclid(1.0) * n).floor() as usize;
                let y = ((1.0 - v.rem_euclid(1.0)) * n).floor() as usize;
                let x = x.min(Self::CELLS - 1);
                let y = y.min(Self::CELLS - 1);
                rows.get(y).is_some_and(|r| r & (0x80 >> x) != 0)
            }
            PatternTile::Checker => (u < 0.5) == (v < 0.5),
            PatternTile::Dots => ((u - 0.5).powi(2) + (v - 0.5).powi(2)).sqrt() < 0.3,
            PatternTile::Stripes => v < 0.5,
            PatternTile::DiagonalStripes => (u + v).rem_euclid(1.0) < 0.5,
            PatternTile::Grid => u < 0.12 || v < 0.12,
            PatternTile::Bricks => {
                let row = v < 0.5;
                let uu = if row { u } else { (u + 0.5).rem_euclid(1.0) };
                v.rem_euclid(0.5) < 0.06 || uu < 0.06
            }
            PatternTile::Crosshatch => {
                (u + v).rem_euclid(0.5) < 0.07 || (u - v).rem_euclid(0.5) < 0.07
            }
            PatternTile::Waves => ((v - 0.5) - 0.2 * (u * std::f64::consts::TAU).sin()).abs() < 0.1,
        }
    }
}

/// How a pattern's tile repeats: its scale and rotation, the offset of
/// every other row or column, mirrored neighbours, and whether the tiles
/// follow the object or stay put on the page.
///
/// Pattern space has its origin at the page origin (or, with
/// `with_object`, at the object's own origin), Y up, in millimetres. A
/// point `p` of the tile grid lands on the page at
/// `object * rotate(rotation) * scale(scale) * p`, where `object` is the
/// object's transform with `with_object` and the identity without it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TileLayout {
    /// Size factor of the tile (1 = 100 %), kept in
    /// [`TileLayout::MIN_SCALE`]..=[`TileLayout::MAX_SCALE`].
    pub scale: f64,
    /// Degrees, counter-clockwise.
    pub rotation: f64,
    /// Every other row moves right by this fraction of the tile width (0..1).
    pub row_offset: f64,
    /// Every other column moves up by this fraction of the tile height (0..1).
    pub column_offset: f64,
    /// Every other tile (as the squares of a chequerboard) is mirrored left
    /// to right.
    pub mirror: bool,
    /// The tiles follow the object's moves, scaling and rotation; off,
    /// they stay fixed to the page and the object shows them through.
    pub with_object: bool,
    /// Pixels per inch when the pattern becomes an image (PDF and
    /// PostScript exports, SVG for two-colour tiles).
    pub resolution: f64,
}

impl Default for TileLayout {
    fn default() -> Self {
        TileLayout {
            scale: 1.0,
            rotation: 0.0,
            row_offset: 0.0,
            column_offset: 0.0,
            mirror: false,
            with_object: false,
            resolution: TileLayout::DEFAULT_RESOLUTION,
        }
    }
}

impl TileLayout {
    pub const MIN_SCALE: f64 = 0.01;
    pub const MAX_SCALE: f64 = 100.0;
    /// The resolution exports used before it could be set.
    pub const DEFAULT_RESOLUTION: f64 = 150.0;
    pub const MIN_RESOLUTION: f64 = 30.0;
    pub const MAX_RESOLUTION: f64 = 1200.0;

    pub fn is_default(&self) -> bool {
        *self == TileLayout::default()
    }

    /// The scale, made safe: finite and within the limits.
    pub fn safe_scale(&self) -> f64 {
        if self.scale.is_finite() {
            self.scale.clamp(Self::MIN_SCALE, Self::MAX_SCALE)
        } else {
            1.0
        }
    }

    /// The row and column offsets, made safe: fractions in 0..1.
    pub fn safe_offsets(&self) -> (f64, f64) {
        let f = |v: f64| {
            if v.is_finite() {
                v.rem_euclid(1.0)
            } else {
                0.0
            }
        };
        (f(self.row_offset), f(self.column_offset))
    }

    /// The resolution, made safe.
    pub fn safe_resolution(&self) -> f64 {
        safe_resolution(self.resolution)
    }

    /// Do the tiles need the 2 by 2 arrangement (offsets or mirroring)?
    pub fn needs_super_tile(&self) -> bool {
        let (r, c) = self.safe_offsets();
        self.mirror || r > 1e-9 || c > 1e-9
    }

    /// Pattern space to page space for an object with `object` as its
    /// transform (see the type's documentation).
    pub fn pattern_to_page(&self, object: Affine) -> Affine {
        let rotation = if self.rotation.is_finite() {
            self.rotation.to_radians()
        } else {
            0.0
        };
        let own = Affine::rotate(rotation) * Affine::scale(self.safe_scale());
        if self.with_object {
            object * own
        } else {
            own
        }
    }
}

fn safe_resolution(ppi: f64) -> f64 {
    if ppi.is_finite() {
        ppi.clamp(TileLayout::MIN_RESOLUTION, TileLayout::MAX_RESOLUTION)
    } else {
        TileLayout::DEFAULT_RESOLUTION
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "pattern", rename_all = "lowercase")]
pub enum Pattern {
    TwoColor {
        tile: PatternTile,
        front: Color,
        back: Color,
        size_mm: f64,
        #[serde(default, skip_serializing_if = "TileLayout::is_default")]
        layout: TileLayout,
    },
    /// Tiled PNG, `size_mm` is the tile width; height follows the aspect ratio.
    Bitmap {
        #[serde(with = "crate::document::png_bytes")]
        png: Vec<u8>,
        width_px: u32,
        height_px: u32,
        size_mm: f64,
        #[serde(default, skip_serializing_if = "TileLayout::is_default")]
        layout: TileLayout,
    },
    /// Full-colour vector tile (the vector pattern fill):
    /// `shapes` are in tile-local coordinates, origin at the bottom-left
    /// corner of the tile, Y up, and the tile repeats every `tile.width` by
    /// `tile.height` millimetres. The shapes carry their own transforms,
    /// fills and outlines; `layout` places the tile grid.
    Vector {
        #[serde(default)]
        shapes: Vec<crate::document::Shape>,
        tile: Size,
        #[serde(default, skip_serializing_if = "TileLayout::is_default")]
        layout: TileLayout,
    },
}

impl Pattern {
    /// How the tile repeats.
    pub fn layout(&self) -> &TileLayout {
        match self {
            Pattern::TwoColor { layout, .. }
            | Pattern::Bitmap { layout, .. }
            | Pattern::Vector { layout, .. } => layout,
        }
    }

    pub fn layout_mut(&mut self) -> &mut TileLayout {
        match self {
            Pattern::TwoColor { layout, .. }
            | Pattern::Bitmap { layout, .. }
            | Pattern::Vector { layout, .. } => layout,
        }
    }

    /// The tile's size in pattern space (mm), before the layout's scale:
    /// at least [`Pattern::MIN_TILE_MM`] each way, finite.
    pub fn tile_size(&self) -> Size {
        let safe = |v: f64| {
            if v.is_finite() {
                v.abs().max(Self::MIN_TILE_MM)
            } else {
                10.0
            }
        };
        match self {
            Pattern::TwoColor { size_mm, .. } => Size::new(safe(*size_mm), safe(*size_mm)),
            Pattern::Bitmap {
                width_px,
                height_px,
                size_mm,
                ..
            } => {
                let w = safe(*size_mm);
                Size::new(w, safe(w * *height_px as f64 / (*width_px).max(1) as f64))
            }
            Pattern::Vector { tile, .. } => Size::new(safe(tile.width), safe(tile.height)),
        }
    }

    /// Smallest tile edge, in mm, so a degenerate selection still tiles.
    pub const MIN_TILE_MM: f64 = 0.01;

    /// Build a vector pattern from page-space shapes: the shapes are moved
    /// so their joint bounds start at the origin and the tile takes the
    /// size of those bounds.
    pub fn vector_from_shapes(shapes: &[crate::document::Shape]) -> Pattern {
        let mut bounds: Option<Rect> = None;
        for s in shapes {
            let b = s.bounds();
            if !(b.x0.is_finite() && b.y0.is_finite() && b.x1.is_finite() && b.y1.is_finite()) {
                continue;
            }
            bounds = Some(match bounds {
                Some(acc) => acc.union(b),
                None => b,
            });
        }
        let bounds = bounds.unwrap_or(Rect::new(0.0, 0.0, 0.0, 0.0));
        let shift = Affine::translate((-bounds.x0, -bounds.y0));
        let shapes = shapes
            .iter()
            .map(|s| {
                let mut s = s.clone();
                s.transform = shift * s.transform;
                s
            })
            .collect();
        Pattern::Vector {
            shapes,
            tile: Size::new(
                bounds.width().max(Self::MIN_TILE_MM),
                bounds.height().max(Self::MIN_TILE_MM),
            ),
            layout: TileLayout::default(),
        }
    }
}

/// Procedural textures (the texture fill).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum TextureKind {
    #[default]
    Clouds,
    Marble,
    Noise,
    Wood,
    /// Speckled stone: folded noise (the distance of each octave from its
    /// middle), so fine bright and dark grains sit side by side.
    Granite,
}

impl TextureKind {
    pub const ALL: [TextureKind; 5] = [
        TextureKind::Clouds,
        TextureKind::Marble,
        TextureKind::Noise,
        TextureKind::Wood,
        TextureKind::Granite,
    ];
    pub fn name(self) -> &'static str {
        match self {
            TextureKind::Clouds => "Clouds",
            TextureKind::Marble => "Marble",
            TextureKind::Noise => "Noise",
            TextureKind::Wood => "Wood",
            TextureKind::Granite => "Granite",
        }
    }

    /// Does the detail (the octaves of noise) change this kind? Plain
    /// noise is a single octave.
    pub fn has_detail(self) -> bool {
        self != TextureKind::Noise
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Texture {
    pub kind: TextureKind,
    pub color_a: Color,
    pub color_b: Color,
    /// Feature size in mm.
    pub scale: f64,
    pub seed: u32,
    /// Octaves of noise, [`Texture::MIN_DETAIL`]..=[`Texture::MAX_DETAIL`]:
    /// each adds finer grain at half the strength of the one before.
    #[serde(
        default = "Texture::default_detail",
        skip_serializing_if = "Texture::is_default_detail"
    )]
    pub detail: u8,
    /// 0..1: 0 leaves the tones as they come, 1 pushes them five times
    /// further from the middle (clipped at the two colours).
    #[serde(default, skip_serializing_if = "is_default")]
    pub contrast: f64,
    /// Pixels per inch of the texture's picture: on screen the texture
    /// shows pixels this size when zoomed in past it, and exports turn it
    /// into an image at this resolution.
    #[serde(
        default = "Texture::default_resolution",
        skip_serializing_if = "Texture::is_default_resolution"
    )]
    pub resolution: f64,
}

impl Default for Texture {
    fn default() -> Self {
        Texture {
            kind: TextureKind::Clouds,
            color_a: Color::BLACK,
            color_b: Color::WHITE,
            scale: 20.0,
            seed: 1,
            detail: Texture::DEFAULT_DETAIL,
            contrast: 0.0,
            resolution: TileLayout::DEFAULT_RESOLUTION,
        }
    }
}

impl Texture {
    pub const MIN_DETAIL: u8 = 1;
    pub const MAX_DETAIL: u8 = 8;
    /// The octaves textures had before the detail could be set.
    pub const DEFAULT_DETAIL: u8 = 5;

    fn default_detail() -> u8 {
        Self::DEFAULT_DETAIL
    }

    fn is_default_detail(d: &u8) -> bool {
        *d == Self::DEFAULT_DETAIL
    }

    fn default_resolution() -> f64 {
        TileLayout::DEFAULT_RESOLUTION
    }

    fn is_default_resolution(r: &f64) -> bool {
        *r == TileLayout::DEFAULT_RESOLUTION
    }

    /// The detail, made safe.
    pub fn safe_detail(&self) -> u8 {
        self.detail.clamp(Self::MIN_DETAIL, Self::MAX_DETAIL)
    }

    /// The contrast, made safe (0..1).
    pub fn safe_contrast(&self) -> f64 {
        if self.contrast.is_finite() {
            self.contrast.clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    /// The resolution, made safe.
    pub fn safe_resolution(&self) -> f64 {
        safe_resolution(self.resolution)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(tag = "fill", rename_all = "lowercase")]
pub enum Fill {
    #[default]
    None,
    Solid(Color),
    Fountain(Fountain),
    Pattern(Pattern),
    Texture(Texture),
    Mesh(Mesh),
}

/// Mesh fill: a grid of (rows+1) x (cols+1) nodes with a colour each;
/// cells are bilinear patches (Gouraud) clipped by the object.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Mesh {
    pub rows: u32,
    pub cols: u32,
    /// Row-major, (rows+1)*(cols+1) nodes in local space.
    pub nodes: Vec<MeshNode>,
    #[serde(default)]
    pub smooth: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MeshNode {
    pub pos: Point,
    pub color: Color,
    /// 1.0 = opaque.
    #[serde(default = "one_f32")]
    pub alpha: f32,
}

fn one_f32() -> f32 {
    1.0
}

impl Mesh {
    pub fn new(bounds: crate::geometry::Rect, rows: u32, cols: u32, base: Color) -> Self {
        let mut nodes = Vec::new();
        for r in 0..=rows {
            for c in 0..=cols {
                nodes.push(MeshNode {
                    pos: Point::new(
                        bounds.x0 + bounds.width() * c as f64 / cols as f64,
                        bounds.y0 + bounds.height() * r as f64 / rows as f64,
                    ),
                    color: base,
                    alpha: 1.0,
                });
            }
        }
        Mesh {
            rows,
            cols,
            nodes,
            smooth: false,
        }
    }

    pub fn node(&self, r: u32, c: u32) -> Option<&MeshNode> {
        self.nodes.get((r * (self.cols + 1) + c) as usize)
    }

    /// Index of the node nearest to `p` (local space).
    pub fn nearest(&self, p: Point) -> Option<usize> {
        self.nodes
            .iter()
            .enumerate()
            .min_by(|a, b| {
                (a.1.pos - p)
                    .hypot()
                    .partial_cmp(&(b.1.pos - p).hypot())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(i, _)| i)
    }

    /// The node grid's (row, column) of node `i`.
    pub fn row_col(&self, i: usize) -> (usize, usize) {
        let cols = self.cols as usize + 1;
        (i / cols, i % cols)
    }

    /// The mesh as the grid sees it at row `v` and column `u` (fractions
    /// of the rows and columns, 0 to 1): position, colour and opacity
    /// mixed from the four nodes around it.
    pub fn sample(&self, u: f64, v: f64) -> Option<MeshNode> {
        let (rows, cols) = (self.rows.max(1) as f64, self.cols.max(1) as f64);
        let (x, y) = (u.clamp(0.0, 1.0) * cols, v.clamp(0.0, 1.0) * rows);
        let c = (x.floor() as u32).min(self.cols.saturating_sub(1));
        let r = (y.floor() as u32).min(self.rows.saturating_sub(1));
        let (fu, fv) = (x - c as f64, y - r as f64);
        let (a, b, d, e) = (
            *self.node(r, c)?,
            *self.node(r, c + 1)?,
            *self.node(r + 1, c)?,
            *self.node(r + 1, c + 1)?,
        );
        let mix = |p: MeshNode, q: MeshNode, t: f64| MeshNode {
            pos: p.pos.lerp(q.pos, t),
            color: lerp_color(p.color, q.color, t as f32),
            alpha: p.alpha + (q.alpha - p.alpha) * t as f32,
        };
        Some(mix(mix(a, b, fu), mix(d, e, fu), fv))
    }

    /// The same mesh on a grid of `rows` by `cols` cells spread evenly over
    /// its nodes' bounds, each new node taking the colour and opacity the
    /// old grid has there.
    pub fn regrid(&self, rows: u32, cols: u32) -> Mesh {
        let (rows, cols) = (rows.clamp(1, 50), cols.clamp(1, 50));
        let bounds = self
            .nodes
            .iter()
            .map(|n| crate::geometry::Rect::from_points(n.pos, n.pos))
            .reduce(|a, b| a.union(b))
            .unwrap_or_default();
        let base = self.nodes.first().map(|n| n.color).unwrap_or(Color::WHITE);
        let mut out = Mesh::new(bounds, rows, cols, base);
        out.smooth = self.smooth;
        for r in 0..=rows {
            for c in 0..=cols {
                let i = (r * (cols + 1) + c) as usize;
                if let (Some(old), Some(n)) = (
                    self.sample(c as f64 / cols as f64, r as f64 / rows as f64),
                    out.nodes.get_mut(i),
                ) {
                    n.color = old.color;
                    n.alpha = old.alpha;
                }
            }
        }
        out
    }

    /// Remove the grid lines through node `i` that are not on the edge:
    /// its column, its row, or both. False when both are on the edge.
    pub fn remove_lines_through(&mut self, i: usize) -> bool {
        if i >= self.nodes.len() {
            return false;
        }
        let (r, c) = self.row_col(i);
        let cols = self.cols as usize + 1;
        let drop_col = c > 0 && c < self.cols as usize;
        let drop_row = r > 0 && r < self.rows as usize;
        if !drop_col && !drop_row {
            return false;
        }
        let nodes: Vec<MeshNode> = self
            .nodes
            .iter()
            .enumerate()
            .filter(|(k, _)| !(drop_col && k % cols == c) && !(drop_row && k / cols == r))
            .map(|(_, n)| *n)
            .collect();
        self.nodes = nodes;
        if drop_col {
            self.cols -= 1;
        }
        if drop_row {
            self.rows -= 1;
        }
        true
    }

    /// Add a grid line through the node nearest to `p` (double-click).
    pub fn add_line(&mut self, p: Point) {
        let Some(i) = self.nearest(p) else { return };
        let cols = self.cols as usize + 1;
        let (r, c) = (i / cols, i % cols);
        // Insert a column after c (interpolating positions and colours),
        // or a row after r, whichever edge is nearer.
        let pos = self.nodes[i].pos;
        let add_col = (p.x - pos.x).abs() >= (p.y - pos.y).abs() && c + 1 < cols;
        if add_col {
            let mut new_nodes = Vec::new();
            for rr in 0..=self.rows as usize {
                for cc in 0..cols {
                    let n = self.nodes[rr * cols + cc];
                    new_nodes.push(n);
                    if cc == c {
                        let next = self.nodes[rr * cols + cc + 1];
                        new_nodes.push(MeshNode {
                            pos: n.pos.midpoint(next.pos),
                            color: lerp_color(n.color, next.color, 0.5),
                            alpha: (n.alpha + next.alpha) / 2.0,
                        });
                    }
                }
            }
            self.cols += 1;
            self.nodes = new_nodes;
        } else if r < self.rows as usize {
            let mut new_nodes = Vec::new();
            for rr in 0..=self.rows as usize {
                for cc in 0..cols {
                    new_nodes.push(self.nodes[rr * cols + cc]);
                }
                if rr == r && rr < self.rows as usize {
                    for cc in 0..cols {
                        let n = self.nodes[rr * cols + cc];
                        let next = self.nodes[(rr + 1) * cols + cc];
                        new_nodes.push(MeshNode {
                            pos: n.pos.midpoint(next.pos),
                            color: lerp_color(n.color, next.color, 0.5),
                            alpha: (n.alpha + next.alpha) / 2.0,
                        });
                    }
                }
            }
            self.rows += 1;
            self.nodes = new_nodes;
        }
    }
}

impl Fill {
    pub fn linear(from: Color, to: Color, angle: f64) -> Self {
        Fill::Fountain(Fountain::two(FountainKind::Linear, from, to, angle))
    }

    pub fn radial(from: Color, to: Color) -> Self {
        Fill::Fountain(Fountain::two(FountainKind::Radial, from, to, 0.0))
    }

    /// A representative colour (first stop, front colour) for swatches.
    pub fn preview_color(&self) -> Option<Color> {
        match self {
            Fill::None => None,
            Fill::Solid(c) => Some(*c),
            Fill::Fountain(f) => Some(f.first_color()),
            Fill::Pattern(Pattern::TwoColor { front, .. }) => Some(*front),
            Fill::Pattern(Pattern::Bitmap { .. }) => Some(Color::Gray { v: 0.5 }),
            Fill::Pattern(Pattern::Vector { shapes, .. }) => Some(
                shapes
                    .iter()
                    .find_map(|s| s.fill.preview_color())
                    .unwrap_or(Color::Gray { v: 0.5 }),
            ),
            Fill::Texture(t) => Some(t.color_a),
            Fill::Mesh(m) => m.nodes.first().map(|n| n.color),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum LineCap {
    #[default]
    Butt,
    Round,
    Square,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum LineJoin {
    #[default]
    Miter,
    Round,
    Bevel,
}

/// The preset arrowhead size in outline widths (the head-space unit; see
/// [`Arrowhead`]). Hairlines count as 0.25 mm and the size is at least 1 mm.
pub const HEAD_SIZE_PER_WIDTH: f64 = 4.0;

/// Arrowheads at the ends of open curves.
///
/// Presets are drawn by [`arrowhead_paths`]; a `Custom` head carries its own
/// outline in a normalised head space: the bounding box is centred on
/// y = 0, the tip (largest x) sits at x = 0 and the shape extends towards
/// negative x, i.e. back along the curve, exactly one unit tall
/// ([`Arrowhead::from_shape_path`]). Heads an importer reads with their own
/// placement and size ([`Arrowhead::from_placed_path`]) keep both instead:
/// the origin is the curve's end point and the head may reach past it. At
/// draw time the unit is the preset size ([`HEAD_SIZE_PER_WIDTH`] outline
/// widths, at least 1 mm), x follows the curve direction at the end point
/// and y its left normal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Arrowhead {
    #[default]
    None,
    Arrow,
    OpenArrow,
    Circle,
    Square,
    Bar,
    Diamond,
    /// User-drawn head (Object > Create > Arrowhead).
    Custom {
        path: crate::geometry::BezPath,
        name: String,
    },
}

impl Arrowhead {
    /// The built-in presets, in menu order. Custom heads are not listed.
    pub const ALL: [Arrowhead; 7] = [
        Arrowhead::None,
        Arrowhead::Arrow,
        Arrowhead::OpenArrow,
        Arrowhead::Circle,
        Arrowhead::Square,
        Arrowhead::Bar,
        Arrowhead::Diamond,
    ];

    pub fn name(&self) -> &str {
        match self {
            Arrowhead::None => "None",
            Arrowhead::Arrow => "Arrow",
            Arrowhead::OpenArrow => "Open arrow",
            Arrowhead::Circle => "Circle",
            Arrowhead::Square => "Square",
            Arrowhead::Bar => "Bar",
            Arrowhead::Diamond => "Diamond",
            Arrowhead::Custom { name, .. } => name,
        }
    }

    pub fn is_custom(&self) -> bool {
        matches!(self, Arrowhead::Custom { .. })
    }

    /// Build a custom head from any closed outline (page or local space,
    /// orientation: tip on the right, pointing towards +x). The outline is
    /// moved and scaled into head space (see the enum docs). An empty or
    /// degenerate outline (no height and no width) gives `Arrowhead::None`.
    pub fn from_shape_path(path: &crate::geometry::BezPath, name: impl Into<String>) -> Arrowhead {
        use crate::geometry::Shape as _;
        if path.elements().is_empty() {
            return Arrowhead::None;
        }
        let b = path.bounding_box();
        if !(b.x0.is_finite() && b.y0.is_finite() && b.x1.is_finite() && b.y1.is_finite()) {
            return Arrowhead::None;
        }
        // A flat outline (a bar seen edge on) still gets a height from its
        // width so the head stays visible.
        let height = if b.height() > 1e-9 {
            b.height()
        } else if b.width() > 1e-9 {
            b.width()
        } else {
            return Arrowhead::None;
        };
        let scale = 1.0 / height;
        let center_y = (b.y0 + b.y1) / 2.0;
        let normalised =
            Affine::scale(scale) * Affine::translate((-b.x1, -center_y)) * path.clone();
        Arrowhead::Custom {
            path: normalised,
            name: name.into(),
        }
    }

    /// Build a custom head from an imported outline that is already placed
    /// the way the file draws it: the curve's end point at the origin, the
    /// curve coming in from -x, and `units_per_width` outline units making
    /// one outline width. The placement and size are kept as they are (no
    /// normalising), so the head is drawn as the file sizes it, whatever
    /// part of it lies ahead of or beside the end point. A head larger than
    /// 100 outline widths is scaled down to that. An empty or degenerate
    /// outline, or a unit that is not a finite positive number, gives
    /// `Arrowhead::None`.
    pub fn from_placed_path(
        path: &crate::geometry::BezPath,
        name: impl Into<String>,
        units_per_width: f64,
    ) -> Arrowhead {
        use crate::geometry::Shape as _;
        if path.elements().is_empty() || !(units_per_width.is_finite() && units_per_width > 0.0) {
            return Arrowhead::None;
        }
        let b = path.bounding_box();
        if !(b.x0.is_finite() && b.y0.is_finite() && b.x1.is_finite() && b.y1.is_finite()) {
            return Arrowhead::None;
        }
        let extent = b.width().max(b.height()) / units_per_width;
        if extent <= 1e-9 {
            return Arrowhead::None;
        }
        // Head space: one unit is HEAD_SIZE_PER_WIDTH outline widths.
        let scale = 1.0 / (units_per_width * HEAD_SIZE_PER_WIDTH) * (100.0 / extent).min(1.0);
        Arrowhead::Custom {
            path: Affine::scale(scale) * path.clone(),
            name: name.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stroke {
    pub color: Color,
    /// Width in millimetres. A "hairline" is 0.0762 mm (0.216 pt).
    pub width: f64,
    pub cap: LineCap,
    pub join: LineJoin,
    /// Dash pattern as alternating on/off lengths, in multiples of the width.
    /// Empty = solid.
    pub dash: Vec<f64>,
    /// When true, the outline scales with the object ("Scale with
    /// object"). When false, width stays fixed under transforms.
    pub scale_with_object: bool,
    /// Draw the outline behind the fill.
    pub behind_fill: bool,
    #[serde(default)]
    pub start_arrow: Arrowhead,
    #[serde(default)]
    pub end_arrow: Arrowhead,
    /// Calligraphic nib stretch 0..1 and angle (1.0 = round nib).
    #[serde(default = "one")]
    pub stretch: f64,
    #[serde(default)]
    pub nib_angle: f64,
    /// A fill for the outline itself (a gradient, pattern or texture);
    /// None paints it with `color`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<Box<Fill>>,
    /// Width nodes of a variable-width outline, in order along the path.
    /// Empty: the outline has `width` everywhere.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub widths: Vec<WidthNode>,
    /// Drawn as painterly brush marks along the path instead of a line.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub painterly: Option<crate::painterly::Painterly>,
}

/// A point of a variable-width outline: where it is along the path and
/// how far the outline reaches on each side of it there.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WidthNode {
    /// Place along the path, 0 (start) to 1 (end), by length over all
    /// subpaths.
    pub at: f64,
    /// Reach to the left of the path's direction, in millimetres.
    pub left: f64,
    /// Reach to the right, in millimetres.
    pub right: f64,
}

impl WidthNode {
    pub fn symmetric(at: f64, width: f64) -> Self {
        WidthNode {
            at,
            left: width / 2.0,
            right: width / 2.0,
        }
    }
}

/// The left and right reach of a variable outline at `t` (0 to 1): the
/// path's ends keep half the base width unless a node sits there, and
/// the reach eases between neighbouring nodes.
pub fn width_at(base: f64, nodes: &[WidthNode], t: f64) -> (f64, f64) {
    let half = (base / 2.0).max(0.0);
    let t = t.clamp(0.0, 1.0);
    let mut prev = WidthNode {
        at: 0.0,
        left: half,
        right: half,
    };
    let mut next = WidthNode {
        at: 1.0,
        left: half,
        right: half,
    };
    let mut found_next = false;
    for n in nodes {
        let n = WidthNode {
            at: n.at.clamp(0.0, 1.0),
            left: n.left.max(0.0),
            right: n.right.max(0.0),
        };
        if n.at <= t {
            prev = n;
        } else if !found_next {
            next = n;
            found_next = true;
        }
    }
    let span = next.at - prev.at;
    if span <= 1e-12 {
        return (prev.left, prev.right);
    }
    let u = ((t - prev.at) / span).clamp(0.0, 1.0);
    let s = u * u * (3.0 - 2.0 * u);
    (
        prev.left + (next.left - prev.left) * s,
        prev.right + (next.right - prev.right) * s,
    )
}

fn one() -> f64 {
    1.0
}

impl Stroke {
    pub const HAIRLINE: f64 = 0.0762;

    pub fn hairline(color: Color) -> Self {
        Stroke {
            color,
            width: Self::HAIRLINE,
            ..Default::default()
        }
    }

    pub fn new(color: Color, width: f64) -> Self {
        Stroke {
            color,
            width,
            ..Default::default()
        }
    }
}

impl Default for Stroke {
    fn default() -> Self {
        Stroke {
            color: Color::BLACK,
            width: Self::HAIRLINE,
            cap: LineCap::Butt,
            join: LineJoin::Miter,
            dash: Vec::new(),
            scale_with_object: false,
            behind_fill: false,
            start_arrow: Arrowhead::None,
            end_arrow: Arrowhead::None,
            stretch: 1.0,
            nib_angle: 0.0,
            fill: None,
            widths: Vec::new(),
            painterly: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meshes_regrid_and_lose_lines_keeping_their_colours() {
        let b = crate::geometry::Rect::new(0.0, 0.0, 20.0, 20.0);
        let mut m = Mesh::new(b, 1, 1, Color::WHITE);
        // Left column red, right column blue.
        for (i, n) in m.nodes.iter_mut().enumerate() {
            n.color = if i % 2 == 0 {
                Color::rgb8(255, 0, 0)
            } else {
                Color::rgb8(0, 0, 255)
            };
        }
        let g = m.regrid(2, 4);
        assert_eq!((g.rows, g.cols, g.nodes.len()), (2, 4, 15));
        // The middle column is the mix of red and blue; the ends keep theirs.
        let mid = g.node(1, 2).expect("node").color.to_rgb8();
        assert!(
            (mid[0] as i32 - 128).abs() <= 2 && (mid[2] as i32 - 128).abs() <= 2,
            "{mid:?}"
        );
        assert_eq!(g.node(0, 0).expect("node").color.to_rgb8(), [255, 0, 0]);
        assert_eq!(g.node(2, 4).expect("node").color.to_rgb8(), [0, 0, 255]);
        assert_eq!(g.node(2, 4).expect("node").pos, Point::new(20.0, 20.0));
        // The middle node's lines go; an edge node's cannot.
        let mut h = g.clone();
        let centre = 7; // row 1, column 2
        assert_eq!(h.row_col(centre), (1, 2));
        assert!(h.remove_lines_through(centre));
        assert_eq!((h.rows, h.cols, h.nodes.len()), (1, 3, 8));
        assert!(!h.remove_lines_through(0));
        assert!(!h.remove_lines_through(99));
        // Sampling between nodes mixes them.
        let s = m.sample(0.5, 0.5).expect("sample");
        assert_eq!(s.pos, Point::new(10.0, 10.0));
    }

    #[test]
    fn fountain_interpolates_stops() {
        let f = Fountain {
            kind: FountainKind::Linear,
            stops: vec![
                Stop::new(0.0, Color::rgb8(0, 0, 0)),
                Stop::new(0.5, Color::rgb8(255, 0, 0)),
                Stop::new(1.0, Color::rgb8(255, 255, 255)),
            ],
            angle: 0.0,
            offset: Point::ZERO,
            edge_pad: 0.0,
            ..Default::default()
        };
        assert_eq!(f.color_at(0.25).to_rgb8(), [128, 0, 0]);
        assert_eq!(f.color_at(0.5).to_rgb8(), [255, 0, 0]);
        assert_eq!(f.color_at(1.5).to_rgb8(), [255, 255, 255]);
    }

    fn black_white() -> Fountain {
        Fountain::two(FountainKind::Linear, Color::BLACK, Color::WHITE, 0.0)
    }

    fn grey(c: Color) -> u8 {
        c.to_rgb8()[0]
    }

    #[test]
    fn fountain_repeats_mirrors_and_inverts() {
        // Sawtooth, three cycles: dark again at the start of each.
        let mut f = black_white();
        f.repeat = FountainRepeat::Repeat;
        f.repeats = 3;
        assert_eq!(grey(f.color_at(0.0)), 0);
        assert!(grey(f.color_at(0.33)) > 240);
        assert!(grey(f.color_at(0.34)) < 15);
        assert_eq!(grey(f.color_at(1.0)), 255);
        // Triangular: there and back.
        f.repeat = FountainRepeat::Mirror;
        f.repeats = 2;
        assert_eq!(grey(f.color_at(0.5)), 255);
        assert_eq!(grey(f.color_at(1.0)), 0);
        assert_eq!(grey(f.color_at(0.25)), grey(f.color_at(0.75)));
        // No repeat: the count is ignored.
        f.repeat = FountainRepeat::None;
        assert_eq!(f.cycles(), 1);
        assert_eq!(grey(f.color_at(0.5)), 128);
        // Bilinear: first colour at both ends, last in the middle.
        let mut b = black_white();
        b.bilinear = true;
        assert_eq!(grey(b.color_at(0.0)), 0);
        assert_eq!(grey(b.color_at(0.5)), 255);
        assert_eq!(grey(b.color_at(1.0)), 0);
        // Inverted: the order reversed.
        let mut i = black_white();
        i.invert = true;
        assert_eq!(grey(i.color_at(0.0)), 255);
        assert_eq!(grey(i.color_at(1.0)), 0);
        // Out of range and not a number stay inside the stops.
        assert_eq!(grey(i.color_at(f64::NAN)), 255);
        let mut huge = black_white();
        huge.repeat = FountainRepeat::Repeat;
        huge.repeats = u32::MAX;
        assert_eq!(huge.cycles(), Fountain::MAX_REPEATS);
    }

    #[test]
    fn resolved_stops_draw_the_same_colours_in_one_run() {
        let three = Fountain {
            stops: vec![
                Stop::new(0.0, Color::rgb8(255, 0, 0)),
                Stop::new(0.4, Color::rgb8(0, 255, 0)),
                Stop::new(1.0, Color::rgb8(0, 0, 255)),
            ],
            ..Default::default()
        };
        for repeat in [
            FountainRepeat::None,
            FountainRepeat::Repeat,
            FountainRepeat::Mirror,
        ] {
            for repeats in [1, 2, 3] {
                for bilinear in [false, true] {
                    for invert in [false, true] {
                        for edge_pad in [0.0, 0.2] {
                            let f = Fountain {
                                repeat,
                                repeats,
                                bilinear,
                                invert,
                                edge_pad,
                                ..three.clone()
                            };
                            let r = f.resolved_stops();
                            assert_eq!(r[0].pos, 0.0);
                            assert!((r[r.len() - 1].pos - 1.0).abs() < 1e-12);
                            assert!(r.windows(2).all(|w| w[0].pos <= w[1].pos));
                            for k in 0..=200 {
                                // Away from the sawtooth's jumps.
                                let t = (k as f64 + 0.37) / 201.0;
                                let a = f.color_at(t).to_rgb8();
                                let b = sample_stops(&r, t).to_rgb8();
                                for c in 0..3 {
                                    assert!(
                                        (a[c] as i32 - b[c] as i32).abs() <= 2,
                                        "{f:?} at {t}: {a:?} {b:?}"
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
        // A sawtooth's jump is a hard edge: two stops at one place.
        let mut saw = black_white();
        saw.repeat = FountainRepeat::Repeat;
        saw.repeats = 2;
        let r = saw.resolved_stops();
        assert_eq!(r.len(), 4);
        assert!((r[1].pos - 0.5).abs() < 1e-12 && (r[2].pos - 0.5).abs() < 1e-12);
        assert_eq!((grey(r[1].color), grey(r[2].color)), (255, 0));
        // A plain fountain keeps its stops.
        assert_eq!(three.resolved_stops(), three.stops);
    }

    #[test]
    fn plain_versions_carry_the_options_in_their_stops() {
        let f = black_white();
        assert_eq!(f.to_plain(64), f);
        let mut i = black_white();
        i.invert = true;
        i.edge_pad = 0.1;
        let p = i.to_plain(64);
        assert!(p.runs_once() && p.edge_pad == 0.0);
        assert_eq!(grey(p.first_color()), 255);
        // Shaped becomes square, the inside (1) at the centre (0).
        let s = Fountain::two(FountainKind::Shaped, Color::BLACK, Color::WHITE, 0.0).to_plain(64);
        assert_eq!(s.kind, FountainKind::Square);
        assert_eq!(grey(s.first_color()), 255);
        assert_eq!(grey(s.last_color()), 0);
        // Too many stops: sampled down evenly.
        let mut many = black_white();
        many.repeat = FountainRepeat::Repeat;
        many.repeats = 50;
        assert_eq!(many.to_plain(16).stops.len(), 16);
    }

    #[test]
    fn fountains_without_the_options_read_and_write_as_before() {
        // As files written before the options had it.
        let old = r#"{"kind":"radial","stops":[{"pos":0.0,"color":{"model":"rgb","r":0.0,"g":0.0,"b":0.0}},{"pos":1.0,"color":{"model":"gray","v":1.0}}],"angle":0.0,"offset":{"x":0.0,"y":0.0},"edge_pad":0.1}"#;
        let f: Fountain = serde_json::from_str(old).expect("old fountain reads");
        assert_eq!(f.kind, FountainKind::Radial);
        assert!(f.runs_once());
        assert_eq!((f.repeat, f.repeats), (FountainRepeat::None, 1));
        // Defaults are not written, so files stay as they were.
        let json = serde_json::to_string(&black_white()).expect("json");
        for field in ["repeat", "bilinear", "invert"] {
            assert!(!json.contains(field), "{json}");
        }
        let mut g = black_white();
        g.kind = FountainKind::Shaped;
        g.repeat = FountainRepeat::Mirror;
        g.repeats = 4;
        g.bilinear = true;
        g.invert = true;
        let json = serde_json::to_string(&g).expect("json");
        assert!(
            json.contains("\"shaped\"") && json.contains("\"mirror\""),
            "{json}"
        );
        let back: Fountain = serde_json::from_str(&json).expect("reads");
        assert_eq!(back, g);
    }

    #[test]
    fn pattern_tiles_have_both_colours() {
        for t in PatternTile::ALL {
            let mut front = 0;
            for i in 0..20 {
                for j in 0..20 {
                    if t.front(i as f64 / 20.0, j as f64 / 20.0) {
                        front += 1;
                    }
                }
            }
            assert!(front > 0 && front < 400, "{t:?}");
        }
    }

    fn square(x0: f64, y0: f64, x1: f64, y1: f64) -> crate::document::Shape {
        let mut s = crate::document::Shape::new(
            crate::ShapeId(1),
            crate::document::ShapeKind::Rect {
                rect: Rect::new(x0, y0, x1, y1),
                radius: 0.0,
                corners: None,
            },
        );
        s.fill = Fill::Solid(Color::rgb8(255, 0, 0));
        s.stroke = None;
        s
    }

    #[test]
    fn vector_pattern_from_shapes_moves_bounds_to_origin() {
        let shapes = vec![
            square(30.0, 40.0, 35.0, 45.0),
            square(35.0, 45.0, 40.0, 50.0),
        ];
        let Pattern::Vector { shapes, tile, .. } = Pattern::vector_from_shapes(&shapes) else {
            panic!("expected a vector pattern");
        };
        assert_eq!(tile, Size::new(10.0, 10.0));
        assert_eq!(shapes.len(), 2);
        let b = shapes[0].bounds();
        assert!((b.x0).abs() < 1e-9 && (b.y0).abs() < 1e-9, "{b:?}");
        assert!(
            (b.x1 - 5.0).abs() < 1e-9 && (b.y1 - 5.0).abs() < 1e-9,
            "{b:?}"
        );
        let b = shapes[1].bounds();
        assert!(
            (b.x1 - 10.0).abs() < 1e-9 && (b.y1 - 10.0).abs() < 1e-9,
            "{b:?}"
        );
    }

    #[test]
    fn vector_pattern_from_nothing_has_a_minimal_tile() {
        let Pattern::Vector { shapes, tile, .. } = Pattern::vector_from_shapes(&[]) else {
            panic!("expected a vector pattern");
        };
        assert!(shapes.is_empty());
        assert!(tile.width > 0.0 && tile.height > 0.0);
    }

    #[test]
    fn vector_pattern_round_trips_through_json() {
        let fill = Fill::Pattern(Pattern::vector_from_shapes(&[square(0.0, 0.0, 5.0, 5.0)]));
        let json = serde_json::to_string(&fill).unwrap();
        assert!(json.contains("\"pattern\":\"vector\""));
        let back: Fill = serde_json::from_str(&json).unwrap();
        assert_eq!(back, fill);
    }

    #[test]
    fn older_pattern_fills_still_load() {
        let json = r#"{"fill":"pattern","pattern":"twocolor","tile":"dots","front":{"model":"gray","v":0.0},"back":{"model":"gray","v":1.0},"size_mm":5.0}"#;
        let back: Fill = serde_json::from_str(json).unwrap();
        assert!(matches!(
            back,
            Fill::Pattern(Pattern::TwoColor {
                tile: PatternTile::Dots,
                ..
            })
        ));
        // A vector tile without shapes is still a valid (empty) tile.
        let json = r#"{"fill":"pattern","pattern":"vector","tile":{"width":4.0,"height":2.0}}"#;
        let back: Fill = serde_json::from_str(json).unwrap();
        assert!(
            matches!(back, Fill::Pattern(Pattern::Vector { ref shapes, .. }) if shapes.is_empty())
        );
    }

    #[test]
    fn unchanged_patterns_and_textures_save_as_before() {
        let fill = Fill::Pattern(Pattern::TwoColor {
            tile: PatternTile::Dots,
            front: Color::BLACK,
            back: Color::WHITE,
            size_mm: 5.0,
            layout: TileLayout::default(),
        });
        let json = serde_json::to_string(&fill).unwrap();
        assert!(!json.contains("layout"), "{json}");
        assert!(json.contains("\"tile\":\"dots\""), "{json}");
        let t = Fill::Texture(Texture::default());
        let json = serde_json::to_string(&t).unwrap();
        for key in ["detail", "contrast", "resolution"] {
            assert!(!json.contains(key), "{json}");
        }
        // An older texture reads with the settings it was drawn with.
        let old = r#"{"fill":"texture","kind":"wood","color_a":{"model":"gray","v":0.0},"color_b":{"model":"gray","v":1.0},"scale":5.0,"seed":3}"#;
        let Fill::Texture(back) = serde_json::from_str::<Fill>(old).unwrap() else {
            panic!("a texture");
        };
        assert_eq!(
            (back.detail, back.contrast, back.resolution),
            (Texture::DEFAULT_DETAIL, 0.0, TileLayout::DEFAULT_RESOLUTION)
        );
    }

    #[test]
    fn tile_layout_cells_and_granite_round_trip() {
        let layout = TileLayout {
            scale: 2.5,
            rotation: 30.0,
            row_offset: 0.5,
            column_offset: 0.25,
            mirror: true,
            with_object: true,
            resolution: 600.0,
        };
        let tile = PatternTile::Checker.toggled(0, 0);
        let fills = [
            Fill::Pattern(Pattern::TwoColor {
                tile,
                front: Color::BLACK,
                back: Color::WHITE,
                size_mm: 5.0,
                layout,
            }),
            Fill::Pattern(Pattern::Vector {
                shapes: Vec::new(),
                tile: Size::new(4.0, 2.0),
                layout,
            }),
            Fill::Texture(Texture {
                kind: TextureKind::Granite,
                detail: 8,
                contrast: 0.4,
                resolution: 300.0,
                ..Texture::default()
            }),
        ];
        for fill in fills {
            let json = serde_json::to_string(&fill).unwrap();
            let back: Fill = serde_json::from_str(&json).unwrap();
            assert_eq!(back, fill, "{json}");
        }
        // A layout written with only some settings keeps the others.
        let json = r#"{"fill":"pattern","pattern":"twocolor","tile":"dots","front":{"model":"gray","v":0.0},"back":{"model":"gray","v":1.0},"size_mm":5.0,"layout":{"rotation":45.0}}"#;
        let Fill::Pattern(p) = serde_json::from_str::<Fill>(json).unwrap() else {
            panic!("a pattern");
        };
        assert_eq!(p.layout().rotation, 45.0);
        assert_eq!(p.layout().scale, 1.0);
    }

    #[test]
    fn cells_toggle_and_sample_the_built_in_tiles() {
        // The checker is front on the top right and bottom left quarters.
        let rows = PatternTile::Checker.cells();
        assert_eq!(rows[0], 0x0F);
        assert_eq!(rows[7], 0xF0);
        let t = PatternTile::Checker.toggled(0, 0);
        assert!(matches!(t, PatternTile::Cells(_)));
        assert!(t.cell(0, 0) && !t.cell(1, 0) && t.cell(0, 4) && !t.cell(4, 4));
        // The top left cell is now front; its neighbour still back.
        assert!(t.front(0.01, 0.99) && !t.front(0.2, 0.99));
        assert_eq!(t.toggled(0, 0), PatternTile::Cells(rows));
        // Outside the grid nothing changes and nothing panics.
        assert_eq!(t.toggled(8, 0), t);
        assert!(!t.cell(99, 99));
        assert!(t.front(-0.99, 1.99) == t.front(0.01, 0.99));
    }

    #[test]
    fn tile_layout_places_the_grid_on_the_page_or_the_object() {
        let mut l = TileLayout {
            scale: 2.0,
            ..TileLayout::default()
        };
        let object = Affine::translate((10.0, 0.0)) * Affine::scale(3.0);
        let p = Point::new(1.0, 1.0);
        assert_eq!(l.pattern_to_page(object) * p, Point::new(2.0, 2.0));
        l.with_object = true;
        assert_eq!(l.pattern_to_page(object) * p, Point::new(16.0, 6.0));
        l.rotation = 90.0;
        l.with_object = false;
        let q = l.pattern_to_page(object) * Point::new(1.0, 0.0);
        assert!((q.x).abs() < 1e-9 && (q.y - 2.0).abs() < 1e-9, "{q:?}");
        // Bad numbers from a file stay safe.
        let bad = TileLayout {
            scale: f64::NAN,
            rotation: f64::INFINITY,
            row_offset: 7.25,
            column_offset: f64::NAN,
            resolution: -5.0,
            ..TileLayout::default()
        };
        assert_eq!(bad.safe_scale(), 1.0);
        assert_eq!(bad.safe_offsets(), (0.25, 0.0));
        assert_eq!(bad.safe_resolution(), TileLayout::MIN_RESOLUTION);
        assert!(bad.pattern_to_page(Affine::IDENTITY).is_finite());
    }
}

/// Arrowhead outlines (page space) for an open path's ends, sized by the
/// outline width (hairlines use 0.25 mm). Filled with the outline colour.
pub fn arrowhead_paths(
    path: &crate::geometry::BezPath,
    stroke: &Stroke,
) -> Vec<crate::geometry::BezPath> {
    use crate::geometry::{BezPath, PathEl, Vec2};
    let w = if stroke.width <= Stroke::HAIRLINE + 1e-9 {
        0.25
    } else {
        stroke.width
    };
    let size = (w * HEAD_SIZE_PER_WIDTH).max(1.0);
    let mut pts: Vec<Point> = Vec::new();
    let mut closed = false;
    kurbo::flatten(path.elements().iter().copied(), 0.05, &mut |el| match el {
        PathEl::MoveTo(p) => {
            if pts.is_empty() {
                pts.push(p)
            }
        }
        PathEl::LineTo(p) => pts.push(p),
        PathEl::ClosePath => closed = true,
        _ => {}
    });
    if closed || pts.len() < 2 {
        return Vec::new();
    }
    let head = |tip: Point, dir: Vec2, kind: &Arrowhead| -> Option<BezPath> {
        let dir = dir.normalize();
        if !dir.x.is_finite() {
            return None;
        }
        let n = Vec2::new(-dir.y, dir.x);
        let mut p = BezPath::new();
        match kind {
            Arrowhead::None => return None,
            Arrowhead::Custom { path: outline, .. } => {
                // Head space (unit tall, tip at the origin, body towards -x)
                // to page space: x along the curve direction, y along its
                // left normal, scaled to the preset size.
                let place =
                    Affine::new([dir.x, dir.y, n.x, n.y, tip.x, tip.y]) * Affine::scale(size);
                p = place * outline.clone();
            }
            Arrowhead::Arrow => {
                p.move_to(tip);
                p.line_to(tip - dir * size + n * (size * 0.4));
                p.line_to(tip - dir * size * 0.7);
                p.line_to(tip - dir * size - n * (size * 0.4));
                p.close_path();
            }
            Arrowhead::OpenArrow => {
                let t = w.max(0.2);
                p.move_to(tip);
                p.line_to(tip - dir * size + n * (size * 0.45));
                p.line_to(tip - dir * size + n * (size * 0.45) - dir * 0.0 + n * (-t) - dir * t);
                p.line_to(tip - dir * (t * 1.4));
                p.line_to(tip - dir * size - n * (size * 0.45) + n * t - dir * t);
                p.line_to(tip - dir * size - n * (size * 0.45));
                p.close_path();
            }
            Arrowhead::Circle => {
                let c = tip - dir * (size * 0.35);
                p = crate::geometry::Circle::new(c, size * 0.35).to_path(0.01);
            }
            Arrowhead::Square => {
                let c = tip - dir * (size * 0.35);
                let h = size * 0.35;
                p.move_to(c + dir * h + n * h);
                p.line_to(c - dir * h + n * h);
                p.line_to(c - dir * h - n * h);
                p.line_to(c + dir * h - n * h);
                p.close_path();
            }
            Arrowhead::Bar => {
                let t = w.max(0.2);
                p.move_to(tip + n * (size * 0.5));
                p.line_to(tip - dir * t + n * (size * 0.5));
                p.line_to(tip - dir * t - n * (size * 0.5));
                p.line_to(tip - n * (size * 0.5));
                p.close_path();
            }
            Arrowhead::Diamond => {
                let h = size * 0.5;
                p.move_to(tip);
                p.line_to(tip - dir * h + n * (h * 0.6));
                p.line_to(tip - dir * h * 2.0);
                p.line_to(tip - dir * h - n * (h * 0.6));
                p.close_path();
            }
        }
        Some(p)
    };
    use crate::geometry::Shape as _;
    let mut out = Vec::new();
    let n = pts.len();
    if let Some(p) = head(pts[0], pts[0] - pts[1.min(n - 1)], &stroke.start_arrow) {
        out.push(p);
    }
    if let Some(p) = head(pts[n - 1], pts[n - 1] - pts[n - 2], &stroke.end_arrow) {
        out.push(p);
    }
    out
}

#[cfg(test)]
mod arrowhead_tests {
    use super::*;
    use crate::geometry::{BezPath, Shape as _};

    /// A triangle pointing right: tip at (30, 15), base from (10, 5) to (10, 25).
    fn triangle() -> BezPath {
        let mut p = BezPath::new();
        p.move_to((10.0, 5.0));
        p.line_to((30.0, 15.0));
        p.line_to((10.0, 25.0));
        p.close_path();
        p
    }

    #[test]
    fn custom_head_is_normalised_to_head_space() {
        let head = Arrowhead::from_shape_path(&triangle(), "Tri");
        let Arrowhead::Custom { path, name } = &head else {
            panic!("expected a custom head");
        };
        assert_eq!(name, "Tri");
        assert!(head.is_custom());
        assert_eq!(head.name(), "Tri");
        let b = path.bounding_box();
        assert!((b.height() - 1.0).abs() < 1e-9, "{b:?}");
        assert!(
            (b.y0 + 0.5).abs() < 1e-9 && (b.y1 - 0.5).abs() < 1e-9,
            "{b:?}"
        );
        assert!(b.x1.abs() < 1e-9, "tip must be at x = 0: {b:?}");
        assert!((b.x0 + 1.0).abs() < 1e-9, "20 wide over 20 tall: {b:?}");
    }

    #[test]
    fn placed_head_keeps_its_size_and_place() {
        let mut line = BezPath::new();
        line.move_to((0.0, 0.0));
        line.line_to((50.0, 0.0));
        // 20 wide and 20 tall, tip at (10, 0): with 10 units per outline
        // width it is two widths tall and reaches one width past the end
        // point.
        let mut placed = BezPath::new();
        placed.move_to((-10.0, -10.0));
        placed.line_to((10.0, 0.0));
        placed.line_to((-10.0, 10.0));
        placed.close_path();
        let head = Arrowhead::from_placed_path(&placed, "Tri", 10.0);
        assert_eq!(head.name(), "Tri");
        for width in [1.0, 0.5, 2.0] {
            let stroke = Stroke {
                end_arrow: head.clone(),
                ..Stroke::new(Color::BLACK, width)
            };
            let heads = arrowhead_paths(&line, &stroke);
            assert_eq!(heads.len(), 1);
            let b = heads[0].bounding_box();
            assert!((b.height() - 2.0 * width).abs() < 1e-9, "{width}: {b:?}");
            assert!((b.x1 - (50.0 + width)).abs() < 1e-9, "{width}: {b:?}");
            assert!((b.y0 + width).abs() < 1e-9, "{width}: {b:?}");
        }
        // Huge heads are capped at 100 outline widths.
        let Arrowhead::Custom { path, .. } = Arrowhead::from_placed_path(&triangle(), "Tri", 1e-6)
        else {
            panic!("expected a custom head");
        };
        assert!((path.bounding_box().height() - 100.0 / HEAD_SIZE_PER_WIDTH).abs() < 1e-9);
        for bad in [0.0, -3.0, f64::NAN, f64::INFINITY] {
            assert_eq!(
                Arrowhead::from_placed_path(&triangle(), "Tri", bad),
                Arrowhead::None
            );
        }
        assert_eq!(
            Arrowhead::from_placed_path(&BezPath::new(), "empty", 5.0),
            Arrowhead::None
        );
    }

    #[test]
    fn degenerate_outlines_give_no_head() {
        assert_eq!(
            Arrowhead::from_shape_path(&BezPath::new(), "empty"),
            Arrowhead::None
        );
        let mut dot = BezPath::new();
        dot.move_to((3.0, 3.0));
        dot.line_to((3.0, 3.0));
        dot.close_path();
        assert_eq!(Arrowhead::from_shape_path(&dot, "dot"), Arrowhead::None);
        // A flat bar keeps a height taken from its width.
        let mut bar = BezPath::new();
        bar.move_to((0.0, 1.0));
        bar.line_to((4.0, 1.0));
        bar.close_path();
        assert!(Arrowhead::from_shape_path(&bar, "bar").is_custom());
    }

    #[test]
    fn presets_are_not_custom_and_all_lists_only_presets() {
        for a in Arrowhead::ALL.iter() {
            assert!(!a.is_custom(), "{a:?}");
        }
        assert_eq!(Arrowhead::ALL.len(), 7);
        assert_eq!(Arrowhead::Arrow.name(), "Arrow");
    }

    #[test]
    fn custom_head_round_trips_through_json_and_presets_stay_strings() {
        let stroke = Stroke {
            start_arrow: Arrowhead::Diamond,
            end_arrow: Arrowhead::from_shape_path(&triangle(), "Tri"),
            ..Stroke::new(Color::BLACK, 1.0)
        };
        let json = serde_json::to_string(&stroke).unwrap();
        assert!(json.contains("\"start_arrow\":\"diamond\""));
        assert!(json.contains("\"end_arrow\":{\"custom\":"));
        let back: Stroke = serde_json::from_str(&json).unwrap();
        assert_eq!(back, stroke);
        // Files written before custom heads existed still load.
        let old = r#"{"color":{"model":"gray","v":0.0},"width":1.0,"cap":"butt","join":"miter","dash":[],"scale_with_object":false,"behind_fill":false,"start_arrow":"none","end_arrow":"arrow","stretch":1.0,"nib_angle":0.0}"#;
        let back: Stroke = serde_json::from_str(old).unwrap();
        assert_eq!(back.end_arrow, Arrowhead::Arrow);
    }

    #[test]
    fn custom_head_is_placed_at_the_line_end_pointing_forward() {
        // A horizontal line from (0, 0) to (50, 0) with a custom end head.
        let mut line = BezPath::new();
        line.move_to((0.0, 0.0));
        line.line_to((50.0, 0.0));
        let stroke = Stroke {
            end_arrow: Arrowhead::from_shape_path(&triangle(), "Tri"),
            ..Stroke::new(Color::BLACK, 1.0)
        };
        let heads = arrowhead_paths(&line, &stroke);
        assert_eq!(heads.len(), 1);
        let b = heads[0].bounding_box();
        // Size is four widths = 4 mm: the head is 4 mm tall, centred on the
        // line, with its tip at the end point and its body behind it.
        assert!((b.x1 - 50.0).abs() < 1e-6, "{b:?}");
        assert!((b.x0 - 46.0).abs() < 1e-6, "{b:?}");
        assert!(
            (b.y0 + 2.0).abs() < 1e-6 && (b.y1 - 2.0).abs() < 1e-6,
            "{b:?}"
        );

        // The same head at the start points the other way.
        let stroke = Stroke {
            start_arrow: stroke.end_arrow.clone(),
            end_arrow: Arrowhead::None,
            ..Stroke::new(Color::BLACK, 1.0)
        };
        let heads = arrowhead_paths(&line, &stroke);
        assert_eq!(heads.len(), 1);
        let b = heads[0].bounding_box();
        assert!(b.x0.abs() < 1e-6 && (b.x1 - 4.0).abs() < 1e-6, "{b:?}");
    }
}
