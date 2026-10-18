//! Renderização: cena achatada (`scene`), texto (`text`) e raster (`raster`).

pub mod arrows;
pub mod basic;
pub mod bspline;
pub mod dash;
pub mod gradient;
pub mod hyphen;
pub mod pattern;
pub mod raster;
pub mod scene;
pub mod separation;
pub mod text;
pub mod varwidth;

pub use scene::{ImageCache, Item, Paint, Scene, SceneOptions, build_scene};

use crate::model::Document;

/// Miniatura PNG da página `index`, com o lado maior igual a `max_side` px.
pub fn thumbnail_png(doc: &Document, index: usize, max_side: u32) -> Option<Vec<u8>> {
    let page = doc.data.pages.get(index)?;
    let scene = build_scene(
        doc,
        page,
        SceneOptions { print: false },
        &ImageCache::default(),
    );
    let longest = page.width.max(page.height);
    if longest <= 0.0 {
        return None;
    }
    let s = max_side as f64 / longest;
    let (w, h) = (
        (page.width * s).ceil() as u32,
        (page.height * s).ceil() as u32,
    );
    let view = kurbo::Affine::new([s, 0.0, 0.0, -s, 0.0, page.height * s]);
    let opts = raster::RasterOptions {
        antialias: true,
        background: Some([255, 255, 255, 255]),
    };
    let pm = raster::render(&scene, view, w, h, &opts)?;
    raster::encode_png(&pm, None).ok()
}
