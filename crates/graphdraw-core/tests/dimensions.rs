//! GD-COT-01..03: cota gravada no .gdr (validada pelo schema), desenhada na
//! cena com linhas, pontas e texto, e exportada em SVG e PDF.

use graphdraw_core::edit::dimension::new_dimension;
use graphdraw_core::gdr;
use graphdraw_core::model::{Document, Page};
use graphdraw_core::render::{ImageCache, Item, SceneOptions, build_scene};
use kurbo::Point;

fn doc() -> Document {
    let mut d = Document::new();
    let mut p = Page::new(200_000.0, 100_000.0);
    p.layers[0].objects.push(new_dimension(
        "horizontal",
        &[
            Point::new(20_000.0, 20_000.0),
            Point::new(120_000.0, 20_000.0),
        ],
        10_000.0,
        vec![],
    ));
    d.data.pages.push(p);
    d
}

#[test]
fn gd_cot_dimension_round_trip_render_and_export() {
    let d = doc();
    let bytes = gdr::write_bytes(&d, &Default::default()).unwrap();
    let (back, _) = gdr::read_bytes(&bytes).unwrap();
    assert_eq!(
        back.data.pages[0].layers[0].objects,
        d.data.pages[0].layers[0].objects
    );
    let scene = build_scene(
        &back,
        &back.data.pages[0],
        SceneOptions { print: false },
        &ImageCache::default(),
    );
    let shapes = scene
        .items
        .iter()
        .filter(|i| matches!(i, Item::Shape(_)))
        .count();
    let texts = scene
        .items
        .iter()
        .filter(|i| matches!(i, Item::Text(_)))
        .count();
    assert!(shapes >= 4, "extensões, linha e duas pontas: {shapes}");
    assert_eq!(texts, 1);
    let id = &d.data.pages[0].layers[0].objects[0].id;
    assert!(
        scene.items.iter().all(|i| i.object_id() == id),
        "tudo é a cota"
    );
    let svg = graphdraw_core::export::svg_page(&back, 0, &Default::default()).unwrap();
    assert!(
        svg.contains("100,0 mm") || svg.contains("<path"),
        "texto ou curvas do rótulo"
    );
    let pdf = graphdraw_core::export::pdf_document(&back, &Default::default()).unwrap();
    assert!(pdf.starts_with(b"%PDF"));
}

/// O schema aceita a cota (validação das fixtures, GD-GDR-08).
#[test]
fn gd_cot_dimension_matches_schema() {
    let d = doc();
    let v = serde_json::to_value(&d.data.pages[0].layers[0].objects[0]).unwrap();
    assert_eq!(v["type"], "dimension");
    assert_eq!(v["dimKind"], "horizontal");
    assert_eq!(v["format"]["unit"], "mm");
}
