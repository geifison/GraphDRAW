//! Padrões de preenchimento (GD-FIL-04/05/06, GD-FIL-10): o ladrilho
//! (vetorial, imagem ou duas cores) vira um superladrilho 2 × 2 em pixels,
//! já com os deslocamentos de linha/coluna e o espelhamento, que a tela, o
//! PDF e o SVG repetem.

use std::sync::Arc;

use kurbo::Affine;
use tiny_skia as sk;

use super::ImageCache;
use crate::model::{Document, Object, Page, Pattern};

/// Superladrilho rasterizado: RGBA pré-multiplicado, linha 0 em cima; cobre
/// `[0, 2w] × [0, 2h]` (µm) no espaço do padrão.
#[derive(Debug)]
pub struct PatternTile {
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
    /// Tamanho do superladrilho no espaço do padrão (µm).
    pub size: (f64, f64),
}

impl PatternTile {
    pub fn pixmap(&self) -> Option<sk::PixmapRef<'_>> {
        sk::PixmapRef::from_bytes(&self.rgba, self.width, self.height)
    }

    /// RGBA não pré-multiplicado (para PDF/SVG).
    pub fn straight_rgba(&self) -> Vec<u8> {
        let mut out = self.rgba.clone();
        for px in out.as_chunks_mut::<4>().0 {
            let a = px[3] as u32;
            if a > 0 && a < 255 {
                for c in &mut px[..3] {
                    *c = ((*c as u32 * 255 + a / 2) / a).min(255) as u8;
                }
            }
        }
        out
    }
}

/// Limite de pixels por lado do ladrilho (memória).
const MAX_TILE_PX: f64 = 1024.0;

/// Espaço do padrão (µm) → página: a origem, o giro e a escala; com
/// "transformar com o objeto" (GD-FIL-10) vem também a matriz do objeto.
pub fn pattern_space(p: &Pattern, object: Affine) -> Affine {
    let own = Affine::translate((p.origin[0], p.origin[1]))
        * Affine::rotate(p.rotation.to_radians())
        * Affine::scale(if p.scale > 0.0 { p.scale } else { 1.0 });
    if p.transform_with_object {
        object * own
    } else {
        own
    }
}

/// Pixel do superladrilho → espaço do padrão (µm, Y para cima).
pub fn pixel_to_pattern(t: &PatternTile) -> Affine {
    let kx = t.size.0 / t.width as f64;
    let ky = t.size.1 / t.height as f64;
    Affine::new([kx, 0.0, 0.0, -ky, 0.0, t.size.1])
}

thread_local! {
    static TILES: std::cell::RefCell<std::collections::HashMap<u64, Arc<PatternTile>>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

fn key(p: &Pattern) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    serde_json::to_string(p).unwrap_or_default().hash(&mut h);
    h.finish()
}

/// Superladrilho do padrão (em cache por conteúdo).
pub fn tile(doc: &Document, p: &Pattern, images: &ImageCache) -> Option<Arc<PatternTile>> {
    let k = key(p);
    if let Some(t) = TILES.with(|c| c.borrow().get(&k).cloned()) {
        return Some(t);
    }
    let t = Arc::new(render_tile(doc, p, images)?);
    TILES.with(|c| {
        let mut c = c.borrow_mut();
        if c.len() > 64 {
            c.clear();
        }
        c.insert(k, t.clone());
    });
    Some(t)
}

/// Ladrilho básico em pixels (`w × h`), Y para baixo.
fn base_tile(
    doc: &Document,
    p: &Pattern,
    images: &ImageCache,
    w: u32,
    h: u32,
) -> Option<sk::Pixmap> {
    let mut pm = sk::Pixmap::new(w, h)?;
    let (tw, th) = (p.width.max(1.0), p.height.max(1.0));
    match p.kind.as_str() {
        "bitmap" => {
            let src = p.src.as_ref()?;
            let img = images.get(doc, src)?;
            let mut rgba = img.rgba.clone();
            for px in rgba.as_chunks_mut::<4>().0 {
                let a = px[3] as u32;
                for c in &mut px[..3] {
                    *c = ((*c as u32 * a + 127) / 255) as u8;
                }
            }
            let ip = sk::PixmapRef::from_bytes(&rgba, img.width, img.height)?;
            // 1 px a mais de cada lado: a borda filtrada não deixa alfa
            // parcial (emenda) na repetição.
            let t = sk::Transform::from_row(
                (w + 2) as f32 / img.width as f32,
                0.0,
                0.0,
                (h + 2) as f32 / img.height as f32,
                -1.0,
                -1.0,
            );
            pm.draw_pixmap(
                0,
                0,
                ip,
                &sk::PixmapPaint {
                    quality: sk::FilterQuality::Bilinear,
                    ..Default::default()
                },
                t,
                None,
            );
        }
        "two-color" => {
            let rows: Vec<&str> = p
                .cells
                .iter()
                .map(String::as_str)
                .filter(|r| !r.is_empty())
                .collect();
            let (nr, nc) = (
                rows.len().max(1),
                rows.iter().map(|r| r.len()).max().unwrap_or(1).max(1),
            );
            let to = |c: &Option<crate::model::Color>| {
                c.as_ref().map(|c| {
                    let [r, g, b] = c.to_srgb();
                    sk::Color::from_rgba(r as f32, g as f32, b as f32, 1.0)
                        .unwrap_or(sk::Color::BLACK)
                })
            };
            if let Some(bg) = to(&p.bg) {
                pm.fill(bg);
            }
            let fg = to(&p.fg).unwrap_or(sk::Color::BLACK);
            let mut paint = sk::Paint::default();
            paint.set_color(fg);
            paint.anti_alias = false;
            let (cw, ch) = (w as f32 / nc as f32, h as f32 / nr as f32);
            for (y, row) in rows.iter().enumerate() {
                for (x, c) in row.chars().enumerate() {
                    if c == '1'
                        && let Some(r) = sk::Rect::from_xywh(x as f32 * cw, y as f32 * ch, cw, ch)
                    {
                        pm.fill_rect(r, &paint, sk::Transform::identity(), None);
                    }
                }
            }
        }
        "texture" => {
            let tx = p.texture.clone().unwrap_or_default();
            let to = |c: &Option<crate::model::Color>, d: [f64; 3]| {
                c.as_ref().map_or(d, |c| c.to_srgb())
            };
            let (fg, bg) = (to(&p.fg, [0.0, 0.0, 0.0]), to(&p.bg, [1.0, 1.0, 1.0]));
            let data = pm.pixels_mut();
            for y in 0..h {
                for x in 0..w {
                    let v = texture_value(&tx, x as f64 / w as f64, y as f64 / h as f64);
                    let m = |i: usize| {
                        ((bg[i] + (fg[i] - bg[i]) * v).clamp(0.0, 1.0) * 255.0).round() as u8
                    };
                    data[(y * w + x) as usize] =
                        sk::ColorU8::from_rgba(m(0), m(1), m(2), 255).premultiply();
                }
            }
        }
        _ => {
            // Vetorial: os objetos do ladrilho numa página do tamanho dele.
            let mut page = Page::new(tw, th);
            page.layers[0].objects = p.tile.iter().map(Object::clone).collect();
            let scene = super::build_scene(doc, &page, super::SceneOptions { print: true }, images);
            let view = Affine::new([w as f64 / tw, 0.0, 0.0, -(h as f64) / th, 0.0, h as f64]);
            pm = super::raster::render(&scene, view, w, h, &Default::default())?;
        }
    }
    Some(pm)
}

/// Valor (0–1) da textura procedural em (u, v) do ladrilho (0–1): ruído de
/// valor fractal com a grade repetida a cada ladrilho, para emendar.
pub fn texture_value(t: &crate::model::Texture, u: f64, v: f64) -> f64 {
    let hash = |x: i64, y: i64, o: u32| -> f64 {
        let mut h = (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
            ^ (y as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
            ^ ((t.seed as u64) << 32 | o as u64).wrapping_mul(0x1656_67B1_9E37_79F9);
        h ^= h >> 29;
        h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
        h ^= h >> 32;
        (h >> 11) as f64 / (1u64 << 53) as f64
    };
    let noise = |x: f64, y: f64, period: i64, o: u32| -> f64 {
        let (xi, yi) = (x.floor(), y.floor());
        let (fx, fy) = (x - xi, y - yi);
        let s = |f: f64| f * f * (3.0 - 2.0 * f);
        let (sx, sy) = (s(fx), s(fy));
        let w = |a: f64| (a as i64).rem_euclid(period.max(1));
        let c = |dx: f64, dy: f64| hash(w(xi + dx), w(yi + dy), o);
        let top = c(0.0, 0.0) + (c(1.0, 0.0) - c(0.0, 0.0)) * sx;
        let bot = c(0.0, 1.0) + (c(1.0, 1.0) - c(0.0, 1.0)) * sx;
        top + (bot - top) * sy
    };
    let f = t.frequency.round().max(1.0);
    let (mut sum, mut amp, mut norm, mut freq) = (0.0, 1.0, 0.0, f);
    for o in 0..t.detail.clamp(1, 8) {
        let n = noise(u * freq, v * freq, freq as i64, o);
        sum += if t.style == "granite" {
            (n * 2.0 - 1.0).abs() * amp
        } else {
            n * amp
        };
        norm += amp;
        amp *= 0.5;
        freq *= 2.0;
    }
    let n = sum / norm;
    let raw = match t.style.as_str() {
        "marble" => 0.5 + 0.5 * ((u * f + n * 4.0) * std::f64::consts::TAU).sin(),
        // Veios na vertical, ondulados pelo ruído (emenda nos dois sentidos).
        "wood" => ((v * f + n * 2.0).fract() * 2.0 - 1.0).abs(),
        _ => n,
    };
    // Contraste em torno do meio.
    let k = 1.0 + t.contrast.clamp(0.0, 1.0) * 4.0;
    (0.5 + (raw - 0.5) * k).clamp(0.0, 1.0)
}

fn render_tile(doc: &Document, p: &Pattern, images: &ImageCache) -> Option<PatternTile> {
    let (tw, th) = (p.width.max(1.0), p.height.max(1.0));
    let k = (p.resolution.max(1.0) / 25_400.0).min(MAX_TILE_PX / tw.max(th));
    let (w, h) = (
        (tw * k).round().max(1.0) as u32,
        (th * k).round().max(1.0) as u32,
    );
    let base = base_tile(doc, p, images, w, h)?;
    let mut sup = sk::Pixmap::new(2 * w, 2 * h)?;
    let (wf, hf) = (w as f32, h as f32);
    let ro = ((p.row_offset.rem_euclid(1.0) as f32) * wf).round();
    let co = ((p.column_offset.rem_euclid(1.0) as f32) * hf).round();
    let paint = sk::PixmapPaint::default();
    // Cada célula (coluna, linha): deslocamento da linha ímpar em x, da
    // coluna ímpar em y; espelhamento alternado (xadrez).
    for (cx, cy) in [(0u32, 0u32), (1, 0), (0, 1), (1, 1)] {
        let dx = cx as f32 * wf + if cy == 1 { ro } else { 0.0 };
        let dy = cy as f32 * hf + if cx == 1 { co } else { 0.0 };
        let mirror = p.mirror && (cx + cy) % 2 == 1;
        // Cópias deslocadas de um superladrilho para fechar a repetição.
        for (ox, oy) in [
            (0.0, 0.0),
            (-2.0 * wf, 0.0),
            (0.0, -2.0 * hf),
            (-2.0 * wf, -2.0 * hf),
        ] {
            let mut t = sk::Transform::from_translate(dx + ox, dy + oy);
            if mirror {
                t = t.pre_concat(sk::Transform::from_row(-1.0, 0.0, 0.0, 1.0, wf, 0.0));
            }
            sup.draw_pixmap(0, 0, base.as_ref(), &paint, t, None);
        }
    }
    Some(PatternTile {
        rgba: sup.data().to_vec(),
        width: 2 * w,
        height: 2 * h,
        size: (2.0 * tw, 2.0 * th),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Color;

    fn two_color() -> Pattern {
        Pattern {
            kind: "two-color".into(),
            cells: vec!["10".into(), "01".into()],
            fg: Some(Color::rgb(0.0, 0.0, 0.0)),
            bg: Some(Color::rgb(1.0, 1.0, 1.0)),
            width: 2_540.0,
            height: 2_540.0,
            resolution: 20.0,
            ..Pattern::default()
        }
    }

    /// GD-FIL-06: xadrez de duas cores; o superladrilho tem 2 × 2 ladrilhos.
    #[test]
    fn texture_tile_is_seamless_and_seeded() {
        let t = crate::model::Texture::default();
        for style in ["clouds", "marble", "granite", "wood"] {
            let s = crate::model::Texture {
                style: style.into(),
                ..t.clone()
            };
            for y in [0.1, 0.5, 0.9] {
                let a = texture_value(&s, 0.0, y);
                let b = texture_value(&s, 1.0, y);
                assert!((a - b).abs() < 1e-9, "{style}: emenda em x");
            }
        }
        let other = crate::model::Texture {
            seed: 2,
            ..t.clone()
        };
        assert_ne!(texture_value(&t, 0.3, 0.3), texture_value(&other, 0.3, 0.3));
        assert_eq!(texture_value(&t, 0.3, 0.7), texture_value(&t, 0.3, 0.7));
        let p = Pattern {
            kind: "texture".into(),
            texture: Some(t),
            fg: Some(crate::model::Color::black()),
            ..Pattern::default()
        };
        assert!(tile(&Document::new(), &p, &ImageCache::default()).is_some());
    }

    #[test]
    fn gd_fil_06_two_color_tile() {
        let doc = Document::new();
        let t = tile(&doc, &two_color(), &ImageCache::default()).unwrap();
        assert_eq!((t.width, t.height), (4, 4));
        let px = |x: u32, y: u32| t.rgba[((y * t.width + x) * 4) as usize];
        assert_eq!(px(0, 0), 0, "preto no canto");
        assert_eq!(px(1, 0), 255, "branco ao lado");
        assert_eq!(px(2, 0), 0, "repete");
    }

    /// GD-FIL-04: deslocamento de linha meio ladrilho; espelhamento.
    #[test]
    fn gd_fil_04_row_offset_and_mirror() {
        let doc = Document::new();
        let mut p = two_color();
        p.cells = vec!["10".into(), "10".into()];
        p.row_offset = 0.5;
        let t = tile(&doc, &p, &ImageCache::default()).unwrap();
        let px = |x: u32, y: u32| t.rgba[((y * t.width + x) * 4) as usize];
        assert_eq!((px(0, 0), px(1, 0)), (0, 255));
        assert_eq!(
            (px(0, 2), px(1, 2)),
            (255, 0),
            "segunda linha andou meio ladrilho"
        );
        let mut m = two_color();
        m.cells = vec!["10".into(), "10".into()];
        m.mirror = true;
        let t = tile(&doc, &m, &ImageCache::default()).unwrap();
        let px = |x: u32, y: u32| t.rgba[((y * t.width + x) * 4) as usize];
        assert_eq!((px(2, 0), px(3, 0)), (255, 0), "vizinho espelhado");
    }

    /// GD-FIL-10: com "transformar com o objeto" o padrão segue a matriz.
    #[test]
    fn gd_fil_10_transform_with_object() {
        let mut p = two_color();
        let obj = Affine::scale(2.0);
        p.transform_with_object = true;
        assert_eq!(
            pattern_space(&p, obj) * kurbo::Point::new(1.0, 1.0),
            kurbo::Point::new(2.0, 2.0)
        );
        p.transform_with_object = false;
        assert_eq!(
            pattern_space(&p, obj) * kurbo::Point::new(1.0, 1.0),
            kurbo::Point::new(1.0, 1.0)
        );
    }
}
