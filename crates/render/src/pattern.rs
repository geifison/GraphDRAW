//! Pattern tiles and texture values.
//!
//! A pattern's tile (two colours, an image or vector objects) becomes a
//! pixmap; with row or column offsets or mirroring, four tiles are
//! arranged into a 2 by 2 "super tile" that repeats seamlessly. The tile
//! grid sits in pattern space (see [`graphdraw_core::TileLayout`]).

use graphdraw_core::geometry::{Affine, Size};
use graphdraw_core::{Color, PatternTile, Texture, TextureKind, TileLayout};
use tiny_skia::{Pixmap, PixmapPaint, Transform};

/// Largest tile edge in pixels; a tile that must be arranged into a super
/// tile is kept to half of it.
pub(crate) const MAX_TILE_PX: u32 = 2048;

/// Pixels across a tile of `mm` millimetres at `density` pixels per
/// millimetre, within `1..=max`.
pub(crate) fn tile_px(mm: f64, density: f64, max: u32) -> u32 {
    let px = (mm * density).round();
    if px.is_finite() {
        (px as u32).clamp(1, max)
    } else {
        1
    }
}

/// A two-colour tile `w` by `h` pixels (row 0 at the top).
pub(crate) fn two_color_tile(
    tile: PatternTile,
    front: Color,
    back: Color,
    w: u32,
    h: u32,
) -> Option<Pixmap> {
    let mut pm = Pixmap::new(w, h)?;
    let [fr, fg, fb] = front.to_rgb8();
    let [br, bg, bb] = back.to_rgb8();
    let f = tiny_skia::ColorU8::from_rgba(fr, fg, fb, 255).premultiply();
    let b = tiny_skia::ColorU8::from_rgba(br, bg, bb, 255).premultiply();
    for (i, px) in pm.pixels_mut().iter_mut().enumerate() {
        let (x, y) = ((i as u32 % w) as f64, (i as u32 / w) as f64);
        let u = (x + 0.5) / w as f64;
        let v = 1.0 - (y + 0.5) / h as f64;
        *px = if tile.front(u, v) { f } else { b };
    }
    Some(pm)
}

/// A copy of `src` scaled to fit within `max` pixels a side.
pub(crate) fn fit(src: &Pixmap, max: u32) -> Option<Pixmap> {
    let (w, h) = (src.width(), src.height());
    if w <= max && h <= max {
        return Some(src.clone());
    }
    let k = max as f64 / w.max(h) as f64;
    let nw = ((w as f64 * k).round() as u32).max(1);
    let nh = ((h as f64 * k).round() as u32).max(1);
    let mut out = Pixmap::new(nw, nh)?;
    let paint = PixmapPaint {
        quality: tiny_skia::FilterQuality::Bilinear,
        ..Default::default()
    };
    out.draw_pixmap(
        0,
        0,
        src.as_ref(),
        &paint,
        Transform::from_scale(nw as f32 / w as f32, nh as f32 / h as f32),
        None,
    );
    Some(out)
}

/// Four tiles in a 2 by 2 block: the second row moved right by the row
/// offset, the second column moved up by the column offset, and, with
/// mirroring, the tiles on the block's diagonal from the top right to the
/// bottom left flipped left to right. The block repeats seamlessly.
pub(crate) fn super_tile(base: &Pixmap, layout: &TileLayout) -> Option<Pixmap> {
    let (w, h) = (base.width(), base.height());
    let mut sup = Pixmap::new(w * 2, h * 2)?;
    let (wf, hf) = (w as f32, h as f32);
    let (row, col) = layout.safe_offsets();
    let ro = (row as f32 * wf).round();
    let co = (col as f32 * hf).round();
    let paint = PixmapPaint::default();
    for (cx, cy) in [(0u32, 0u32), (1, 0), (0, 1), (1, 1)] {
        // Row 0 of the pixmap is the top: the second row of tiles (the
        // bottom one, cy = 1) moves right, the second column (cx = 1)
        // moves up (towards row 0).
        let dx = cx as f32 * wf + if cy == 1 { ro } else { 0.0 };
        let dy = cy as f32 * hf - if cx == 1 { co } else { 0.0 };
        let mirror = layout.mirror && (cx + cy) % 2 == 1;
        // Copies a block away on every side close the repeat.
        for ox in [-2.0 * wf, 0.0, 2.0 * wf] {
            for oy in [-2.0 * hf, 0.0, 2.0 * hf] {
                let (x, y) = (dx + ox, dy + oy);
                if x >= 2.0 * wf || y >= 2.0 * hf || x + wf <= 0.0 || y + hf <= 0.0 {
                    continue;
                }
                let mut t = Transform::from_translate(x, y);
                if mirror {
                    t = t.pre_concat(Transform::from_row(-1.0, 0.0, 0.0, 1.0, wf, 0.0));
                }
                sup.draw_pixmap(0, 0, base.as_ref(), &paint, t, None);
            }
        }
    }
    Some(sup)
}

/// Pixels of a tile pixmap (`pw` by `ph`, row 0 at the top) to pattern
/// space (mm, Y up), the pixmap covering `cover` millimetres from the
/// origin.
pub(crate) fn pixel_to_pattern(pw: u32, ph: u32, cover: Size) -> Affine {
    Affine::new([
        cover.width / pw.max(1) as f64,
        0.0,
        0.0,
        -cover.height / ph.max(1) as f64,
        0.0,
        cover.height,
    ])
}

/// The scale an affine map applies to areas, as a length factor.
pub(crate) fn length_scale(m: Affine) -> f64 {
    let c = m.as_coeffs();
    (c[0] * c[3] - c[1] * c[2]).abs().sqrt()
}

/// Does `m` keep the pixel grid square to the screen (no rotation or skew)?
pub(crate) fn axis_aligned(m: Affine) -> bool {
    let c = m.as_coeffs();
    c[1].abs() < 1e-9 && c[2].abs() < 1e-9
}

pub(crate) fn sk_transform(m: Affine) -> Option<Transform> {
    let c = m.as_coeffs();
    if !c.iter().all(|v| v.is_finite()) {
        return None;
    }
    Some(Transform::from_row(
        c[0] as f32,
        c[1] as f32,
        c[2] as f32,
        c[3] as f32,
        c[4] as f32,
        c[5] as f32,
    ))
}

/// Value noise in 0..1 (one octave).
pub(crate) fn noise(x: f64, y: f64, seed: u32) -> f64 {
    fn hash(ix: i64, iy: i64, seed: u32) -> f64 {
        let mut h = (ix as u64).wrapping_mul(0x9E3779B97F4A7C15)
            ^ (iy as u64).wrapping_mul(0xC2B2AE3D27D4EB4F)
            ^ (seed as u64).wrapping_mul(0x165667B19E3779F9);
        h ^= h >> 31;
        h = h.wrapping_mul(0x9E3779B97F4A7C15);
        h ^= h >> 29;
        (h & 0xFFFF) as f64 / 65535.0
    }
    let (ix, iy) = (x.floor() as i64, y.floor() as i64);
    let (fx, fy) = (x - x.floor(), y - y.floor());
    let (sx, sy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
    let a = hash(ix, iy, seed);
    let b = hash(ix + 1, iy, seed);
    let c = hash(ix, iy + 1, seed);
    let d = hash(ix + 1, iy + 1, seed);
    let top = a + (b - a) * sx;
    let bot = c + (d - c) * sx;
    top + (bot - top) * sy
}

/// Fractal noise with `octaves` octaves (each at twice the frequency and
/// half the strength of the one before). The sum is scaled so its range
/// matches the five octaves textures always had: five octaves give
/// exactly the values they gave before the detail could be set.
pub(crate) fn fbm(x: f64, y: f64, seed: u32, octaves: u8, fold: bool) -> f64 {
    let mut v = 0.0;
    let mut amp = 0.5;
    let mut f = 1.0;
    let mut total = 0.0;
    for _ in 0..octaves.max(1) {
        let n = noise(x * f, y * f, seed);
        v += amp * if fold { (n * 2.0 - 1.0).abs() } else { n };
        total += amp;
        amp *= 0.5;
        f *= 2.0;
    }
    // Five octaves add up to 31/32.
    v * (31.0 / 32.0) / total
}

/// The texture's value (0 = first colour, 1 = second) at (x, y), in
/// units of the texture's scale: the kind's drawing with the texture's
/// detail, then its contrast around the middle.
pub fn texture_value_of(t: &Texture, x: f64, y: f64) -> f64 {
    let detail = t.safe_detail();
    let seed = t.seed;
    let raw = match t.kind {
        TextureKind::Clouds => fbm(x, y, seed, detail, false).clamp(0.0, 1.0),
        TextureKind::Marble => ((x + 4.0 * fbm(x, y, seed, detail, false)) * 2.0)
            .sin()
            .abs(),
        TextureKind::Noise => noise(x * 8.0, y * 8.0, seed),
        TextureKind::Wood => {
            let r = ((x - 0.5).powi(2) + (y * 0.2).powi(2)).sqrt() * 6.0
                + fbm(x, y, seed, detail, false) * 1.5;
            r.fract()
        }
        TextureKind::Granite => {
            // Fine folded noise: grains several to a scale unit, bright
            // where an octave is far from its middle.
            let v = fbm(x * 6.0, y * 6.0, seed, detail, true);
            (1.0 - v * 1.6).clamp(0.0, 1.0)
        }
    };
    let k = 1.0 + t.safe_contrast() * 4.0;
    (0.5 + (raw - 0.5) * k).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn super_tile_offsets_rows_and_mirrors_neighbours() {
        // A 2 by 2 tile: black on the left, white on the right.
        let mut half = Pixmap::new(2, 2).expect("pm");
        for (i, px) in half.pixels_mut().iter_mut().enumerate() {
            let c = if i % 2 == 0 { 0 } else { 255 };
            *px = tiny_skia::ColorU8::from_rgba(c, c, c, 255).premultiply();
        }
        let red = |pm: &Pixmap, x: u32, y: u32| pm.pixel(x, y).map(|p| p.red()).unwrap_or(9);
        let row = TileLayout {
            row_offset: 0.5,
            ..TileLayout::default()
        };
        let s = super_tile(&half, &row).expect("super");
        assert_eq!((s.width(), s.height()), (4, 4));
        assert_eq!((red(&s, 0, 0), red(&s, 1, 0)), (0, 255));
        // The second row moved right by half a tile.
        assert_eq!((red(&s, 0, 2), red(&s, 1, 2)), (255, 0));
        let m = TileLayout {
            mirror: true,
            ..TileLayout::default()
        };
        let s = super_tile(&half, &m).expect("super");
        assert_eq!((red(&s, 0, 0), red(&s, 1, 0)), (0, 255));
        assert_eq!((red(&s, 2, 0), red(&s, 3, 0)), (255, 0), "mirrored");
        assert_eq!((red(&s, 2, 2), red(&s, 3, 2)), (0, 255), "diagonal plain");
        let c = TileLayout {
            column_offset: 0.5,
            ..TileLayout::default()
        };
        let mut top = Pixmap::new(2, 2).expect("pm");
        for (i, px) in top.pixels_mut().iter_mut().enumerate() {
            let c = if i < 2 { 0 } else { 255 };
            *px = tiny_skia::ColorU8::from_rgba(c, c, c, 255).premultiply();
        }
        let s = super_tile(&top, &c).expect("super");
        assert_eq!((red(&s, 0, 0), red(&s, 0, 1)), (0, 255));
        // The second column moved up by half a tile.
        assert_eq!((red(&s, 2, 0), red(&s, 2, 1)), (255, 0));
    }

    #[test]
    fn five_octaves_draw_as_before_and_detail_changes_the_grain() {
        let t = Texture::default();
        // The value five octaves always gave (sum of 0.5^k noise).
        let (x, y) = (1.3, 2.7);
        let mut old = 0.0;
        let (mut amp, mut f) = (0.5, 1.0);
        for _ in 0..5 {
            old += amp * noise(x * f, y * f, t.seed);
            amp *= 0.5;
            f *= 2.0;
        }
        assert!((texture_value_of(&t, x, y) - old.clamp(0.0, 1.0)).abs() < 1e-12);
        let one = Texture { detail: 1, ..t.clone() };
        let eight = Texture { detail: 8, ..t.clone() };
        assert_ne!(texture_value_of(&one, x, y), texture_value_of(&eight, x, y));
        // Out of range details are clamped, not a panic.
        let zero = Texture { detail: 0, ..t.clone() };
        assert_eq!(texture_value_of(&zero, x, y), texture_value_of(&one, x, y));
    }

    #[test]
    fn contrast_spreads_the_tones_and_granite_is_speckled() {
        let t = Texture::default();
        let hard = Texture {
            contrast: 1.0,
            ..t.clone()
        };
        let spread = |t: &Texture| {
            let (mut lo, mut hi) = (1.0f64, 0.0f64);
            for i in 0..400 {
                let v = texture_value_of(t, i as f64 * 0.037, i as f64 * 0.051);
                lo = lo.min(v);
                hi = hi.max(v);
            }
            hi - lo
        };
        assert!(spread(&hard) > spread(&t) + 0.2);
        let granite = Texture {
            kind: TextureKind::Granite,
            ..t.clone()
        };
        // Neighbouring samples a fraction of a scale unit apart differ a
        // lot more than in clouds: fine grain.
        let rough = |t: &Texture| {
            (0..400)
                .map(|i| {
                    let x = i as f64 * 0.05;
                    (texture_value_of(t, x, 0.3) - texture_value_of(t, x + 0.05, 0.3)).abs()
                })
                .sum::<f64>()
        };
        assert!(rough(&granite) > rough(&t) * 2.0);
        for i in 0..200 {
            let v = texture_value_of(&granite, i as f64 * 0.13, 1.0 - i as f64 * 0.07);
            assert!((0.0..=1.0).contains(&v));
        }
    }
}
