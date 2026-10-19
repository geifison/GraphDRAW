//! Keeps the rasterized page as an egui texture, re-rendering only when the
//! document, view, canvas size or drag preview changed.

use crate::view::View;
use egui::{ColorImage, Context, Rect, TextureHandle, TextureId, TextureOptions};
use tracedraw_core::{geometry::Affine, Document, PageId, ShapeId};
use tracedraw_render::{render_page, Preview, RenderOptions, ViewTransform};

#[derive(Default)]
pub struct Raster {
    texture: Option<TextureHandle>,
    key: Option<Key>,
    /// Proofed colours on a grid of screen colours (View > Proof Colors),
    /// made when first needed and dropped with the texture (the colour
    /// settings changed).
    proof: Option<ProofLut>,
}

/// Screen colours as they would print, on a `N` x `N` x `N` grid of the
/// screen's red, green and blue; colours between are interpolated.
struct ProofLut {
    table: Vec<[f32; 3]>,
}

impl ProofLut {
    const N: usize = 17;

    fn new() -> Self {
        let n = Self::N;
        let step = 1.0 / (n - 1) as f32;
        let mut table = Vec::with_capacity(n * n * n);
        for r in 0..n {
            for g in 0..n {
                for b in 0..n {
                    table.push(tracedraw_core::color::proof_rgb([
                        r as f32 * step,
                        g as f32 * step,
                        b as f32 * step,
                    ]));
                }
            }
        }
        ProofLut { table }
    }

    /// The proofed colour of `rgb` (0 to 1), interpolated between the
    /// eight grid colours around it.
    fn apply(&self, rgb: [f32; 3]) -> [f32; 3] {
        let n = Self::N;
        let scale = (n - 1) as f32;
        let mut i0 = [0usize; 3];
        let mut t = [0.0f32; 3];
        for k in 0..3 {
            let x = (rgb[k].clamp(0.0, 1.0)) * scale;
            let i = (x.floor() as usize).min(n - 2);
            i0[k] = i;
            t[k] = x - i as f32;
        }
        let at = |r: usize, g: usize, b: usize| self.table[(r * n + g) * n + b];
        let mut out = [0.0f32; 3];
        for (dr, wr) in [(0, 1.0 - t[0]), (1, t[0])] {
            for (dg, wg) in [(0, 1.0 - t[1]), (1, t[1])] {
                for (db, wb) in [(0, 1.0 - t[2]), (1, t[2])] {
                    let w = wr * wg * wb;
                    if w == 0.0 {
                        continue;
                    }
                    let c = at(i0[0] + dr, i0[1] + dg, i0[2] + db);
                    for k in 0..3 {
                        out[k] += c[k] * w;
                    }
                }
            }
        }
        out
    }

    /// A premultiplied pixel, proofed.
    fn pixel(&self, r: u8, g: u8, b: u8, a: u8) -> egui::Color32 {
        if a == 0 {
            return egui::Color32::TRANSPARENT;
        }
        let af = a as f32 / 255.0;
        let un = |v: u8| (v as f32 / 255.0 / af).min(1.0);
        let p = self.apply([un(r), un(g), un(b)]);
        let q = |v: f32| (v.clamp(0.0, 1.0) * af * 255.0).round() as u8;
        egui::Color32::from_rgba_premultiplied(q(p[0]), q(p[1]), q(p[2]), a)
    }
}

#[derive(PartialEq, Clone, Debug)]
struct Key {
    revision: u64,
    page: PageId,
    zoom: f32,
    /// The page origin relative to the canvas: what the render uses. A
    /// canvas that moves (a bar docked elsewhere) without changing size
    /// still needs a new render when the view stays put.
    offset: egui::Vec2,
    size: (u32, u32),
    preview: Option<(Vec<ShapeId>, [f64; 6])>,
    wireframe: bool,
    simulate_overprints: bool,
    complex_effects: bool,
    proof: bool,
    focus: Option<(ShapeId, u32)>,
    /// The Pixels view: the document pixels drawn (dpi, first column, top
    /// row).
    pixels: Option<(u64, i64, i64)>,
}

/// The document pixels that cover a screen rectangle: `w` by `h` pixels
/// whose top-left one is column `ix0`, row `iy1` (rows counted up from the
/// page's bottom edge, the row above `iy1` excluded), and where they land
/// on screen.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PixelWindow {
    pub ix0: i64,
    pub iy1: i64,
    pub w: u32,
    pub h: u32,
    pub screen: Rect,
}

/// Largest side of the Pixels view texture.
const MAX_PIXELS: i64 = 8192;

/// The document pixels at `dpi` behind `rect`, when one of them spans at
/// least one physical screen pixel (`ppp` per point); below that the
/// normal render already shows them.
pub fn pixel_window(view: &View, rect: Rect, dpi: f64, ppp: f32) -> Option<PixelWindow> {
    if !dpi.is_finite() || dpi <= 0.0 || !view.zoom.is_finite() || view.zoom <= 0.0 {
        return None;
    }
    let k = dpi / 25.4;
    if (view.zoom * ppp) as f64 / k < 1.0 {
        return None;
    }
    let tl = view.to_page(rect.left_top());
    let br = view.to_page(rect.right_bottom());
    let ix0 = (tl.x * k).floor() as i64;
    let ix1 = (br.x * k).ceil() as i64;
    let iy0 = (br.y * k).floor() as i64;
    let iy1 = (tl.y * k).ceil() as i64;
    let (w, h) = (ix1 - ix0, iy1 - iy0);
    if w <= 0 || h <= 0 || w > MAX_PIXELS || h > MAX_PIXELS {
        return None;
    }
    let a = view.to_screen(tracedraw_core::geometry::Point::new(
        ix0 as f64 / k,
        iy1 as f64 / k,
    ));
    let b = view.to_screen(tracedraw_core::geometry::Point::new(
        ix1 as f64 / k,
        iy0 as f64 / k,
    ));
    Some(PixelWindow {
        ix0,
        iy1,
        w: w as u32,
        h: h as u32,
        screen: Rect::from_min_max(a, b),
    })
}

impl Raster {
    /// Drop the cached texture so the next frame re-renders (and the
    /// proofed colours, which the colour settings decide).
    pub fn invalidate(&mut self) {
        self.key = None;
        self.proof = None;
    }

    #[allow(clippy::too_many_arguments)]
    pub fn texture(
        &mut self,
        ctx: &Context,
        doc: &Document,
        page: PageId,
        view: &View,
        rect: Rect,
        preview: Option<(Vec<ShapeId>, Affine)>,
        revision: u64,
        wireframe: bool,
        simulate_overprints: bool,
        complex_effects: bool,
        proof: bool,
        focus: Option<(ShapeId, f32)>,
        pixel_dpi: Option<f64>,
    ) -> Option<(TextureId, Rect)> {
        let ppp = ctx.pixels_per_point();
        let window = pixel_dpi.and_then(|dpi| pixel_window(view, rect, dpi, ppp));
        let (w, h) = match window {
            Some(win) => (win.w, win.h),
            None => (
                (rect.width() * ppp).round().max(1.0) as u32,
                (rect.height() * ppp).round().max(1.0) as u32,
            ),
        };
        let screen = window.map_or(rect, |win| win.screen);
        let key = Key {
            revision,
            page,
            zoom: view.zoom,
            offset: view.origin - rect.min,
            size: (w, h),
            preview: preview.as_ref().map(|(s, t)| (s.clone(), t.as_coeffs())),
            wireframe,
            simulate_overprints,
            complex_effects,
            proof,
            focus: focus.map(|(id, v)| (id, (v * 1000.0) as u32)),
            pixels: window
                .zip(pixel_dpi)
                .map(|(win, dpi)| ((dpi * 1000.0).round() as u64, win.ix0, win.iy1)),
        };
        if self.key.as_ref() == Some(&key) {
            if let Some(t) = &self.texture {
                return Some((t.id(), screen));
            }
        }
        let vt = match (window, pixel_dpi) {
            // One render pixel per document pixel, the window's top-left
            // one at the corner.
            (Some(win), Some(dpi)) => ViewTransform {
                zoom: dpi / 25.4,
                origin_x: -(win.ix0 as f64),
                origin_y: win.iy1 as f64,
            },
            _ => ViewTransform {
                zoom: (view.zoom * ppp) as f64,
                origin_x: ((view.origin.x - rect.min.x) * ppp) as f64,
                origin_y: ((view.origin.y - rect.min.y) * ppp) as f64,
            },
        };
        let opts = RenderOptions {
            width: w,
            height: h,
            view: vt,
            preview: preview.map(|(shapes, transform)| Preview { shapes, transform }),
            wireframe,
            simulate_overprints,
            complex_effects,
            anti_alias: true,
            draft: true,
            focus,
        };
        let pixmap = render_page(doc, page, &opts)?;
        let lut = if proof {
            Some(&*self.proof.get_or_insert_with(ProofLut::new))
        } else {
            None
        };
        let pixels: Vec<egui::Color32> = pixmap
            .pixels()
            .iter()
            .map(|p| match lut {
                Some(lut) => lut.pixel(p.red(), p.green(), p.blue(), p.alpha()),
                None => {
                    egui::Color32::from_rgba_premultiplied(p.red(), p.green(), p.blue(), p.alpha())
                }
            })
            .collect();
        let image = ColorImage {
            size: [w as usize, h as usize],
            source_size: egui::vec2(w as f32, h as f32),
            pixels,
        };
        // Document pixels are blown up as blocks, the way they will look.
        let filter = if window.is_some() {
            TextureOptions::NEAREST
        } else {
            TextureOptions::LINEAR
        };
        match &mut self.texture {
            Some(t) => t.set(image, filter),
            None => self.texture = Some(ctx.load_texture("page", image, filter)),
        }
        self.key = Some(key);
        self.texture.as_ref().map(|t| (t.id(), screen))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracedraw_core::geometry::Size;

    #[test]
    fn proofed_pixels_follow_the_grid_of_proofed_colours() {
        let lut = ProofLut::new();
        // On the grid, the proofed colour itself.
        let blue = lut.apply([0.0, 0.0, 1.0]);
        let want = tracedraw_core::color::proof_rgb([0.0, 0.0, 1.0]);
        for (a, b) in blue.iter().zip(want) {
            assert!((a - b).abs() < 1e-5);
        }
        // Between, close to the colour proofed directly.
        let c = [0.31, 0.62, 0.18];
        let (got, want) = (lut.apply(c), tracedraw_core::color::proof_rgb(c));
        for (a, b) in got.iter().zip(want) {
            assert!((a - b).abs() < 0.03, "{got:?} {want:?}");
        }
        // Transparent stays transparent; half-covered pixels keep their
        // coverage.
        assert_eq!(lut.pixel(0, 0, 0, 0), egui::Color32::TRANSPARENT);
        let half = lut.pixel(0, 0, 128, 128);
        assert_eq!(half.a(), 128);
        assert!(half.b() <= 128);
    }

    #[test]
    fn a_canvas_that_moves_renders_again() {
        let ctx = Context::default();
        let doc = Document::new("t", Size::new(100.0, 50.0));
        let page = doc.pages[0].id;
        let view = View::default();
        let mut raster = Raster::default();
        let at = |x: f32, y: f32| Rect::from_min_size(egui::pos2(x, y), egui::vec2(320.0, 200.0));
        let draw = |raster: &mut Raster, rect: Rect| {
            raster.texture(
                &ctx, &doc, page, &view, rect, None, 1, false, false, false, false, None, None,
            )
        };
        assert!(draw(&mut raster, at(40.0, 160.0)).is_some());
        let first = raster.key.clone();
        // Same canvas, same view: the cached texture.
        assert!(draw(&mut raster, at(40.0, 160.0)).is_some());
        assert_eq!(raster.key, first);
        // The canvas moves up by a bar's height and keeps its size: the
        // page sits elsewhere inside it, so the key changes.
        assert!(draw(&mut raster, at(40.0, 107.0)).is_some());
        assert_ne!(raster.key, first);
        assert_eq!(
            raster.key.as_ref().map(|k| k.offset),
            Some(view.origin - egui::pos2(40.0, 107.0))
        );
    }

    #[test]
    fn the_pixels_view_draws_whole_document_pixels_as_blocks() {
        // 72 dpi: a document pixel is 0.3528 mm; at 10 screen pixels per
        // mm it covers 3.5 screen pixels.
        let view = View {
            zoom: 10.0,
            origin: egui::pos2(0.0, 500.0),
        };
        let rect = Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(400.0, 500.0));
        let win = pixel_window(&view, rect, 72.0, 1.0).expect("window");
        assert_eq!(win.ix0, 0);
        // 50 mm of page height: 141.7 pixels, rounded up to whole ones.
        assert_eq!(win.iy1, 142);
        assert_eq!(win.w, 114);
        assert_eq!(win.h, 142);
        // Whole pixels: the blocks start on the page corner and may run
        // past the canvas a little.
        assert!(win.screen.min.x <= rect.min.x && win.screen.max.x >= rect.max.x);
        assert!(win.screen.min.y <= rect.min.y);
        assert!((win.screen.max.y - 500.0).abs() < 1e-3);
        // Zoomed out below one screen pixel per document pixel: the normal
        // render.
        let far = View { zoom: 1.0, ..view };
        assert_eq!(pixel_window(&far, rect, 72.0, 1.0), None);
        assert_eq!(pixel_window(&view, rect, 0.0, 1.0), None);
        assert_eq!(pixel_window(&view, rect, f64::NAN, 1.0), None);
        // A huge window (very high resolution) falls back too.
        assert_eq!(pixel_window(&view, rect, 1.0e6, 1.0), None);
    }

    #[test]
    fn the_pixels_view_texture_is_one_texel_per_document_pixel() {
        let ctx = Context::default();
        let doc = Document::new("t", Size::new(100.0, 50.0));
        let page = doc.pages[0].id;
        let view = View {
            zoom: 40.0,
            origin: egui::pos2(0.0, 400.0),
        };
        let rect = Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(400.0, 400.0));
        let mut raster = Raster::default();
        let (_, at) = raster
            .texture(
                &ctx,
                &doc,
                page,
                &view,
                rect,
                None,
                1,
                false,
                false,
                false,
                false,
                None,
                Some(96.0),
            )
            .expect("texture");
        let size = raster.texture.as_ref().map(|t| t.size());
        let win = pixel_window(&view, rect, 96.0, 1.0).expect("window");
        assert_eq!(size, Some([win.w as usize, win.h as usize]));
        assert_eq!(at, win.screen);
        // Without the Pixels view: screen-sized, painted on the canvas.
        let (_, at) = raster
            .texture(
                &ctx, &doc, page, &view, rect, None, 1, false, false, false, false, None, None,
            )
            .expect("texture");
        assert_eq!(at, rect);
        assert_eq!(raster.texture.as_ref().map(|t| t.size()), Some([400, 400]));
    }
}
