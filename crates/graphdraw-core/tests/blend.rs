//! GD-TRA-02: modo de mesclagem gravado no .gdr e aplicado na tela/PNG,
//! no PDF (/BM) e no SVG (mix-blend-mode).

use graphdraw_core::export::{self, ExportOptions};
use graphdraw_core::gdr;
use graphdraw_core::model::{
    BlendMode, Color, Document, Fill, Object, ObjectKind, Page, RectData, Transform,
};
use serde_json::Map;

fn rect(x: f64, color: &str) -> Object {
    let mut o = Object::new(ObjectKind::Rectangle(RectData {
        width: 20_000.0,
        height: 20_000.0,
        corners: None,
        extra: Map::new(),
    }));
    o.transform = Transform::translate(x, 10_000.0);
    o.fill = Some(Fill::solid(Color::from_hex(color).unwrap()));
    o
}

fn doc() -> Document {
    let mut d = Document::new();
    let mut p = Page::new(50_000.0, 40_000.0);
    let mut top = rect(15_000.0, "#00ffff");
    top.blend = BlendMode::Multiply;
    p.layers[0].objects.extend([rect(5_000.0, "#ffff00"), top]);
    d.data.pages.push(p);
    d
}

#[test]
fn gd_tra_02_blend_modes() {
    let d = doc();
    let bytes = gdr::write_bytes(&d, &Default::default()).unwrap();
    let (back, _) = gdr::read_bytes(&bytes).unwrap();
    assert_eq!(
        back.data.pages[0].layers[0].objects[1].blend,
        BlendMode::Multiply
    );
    // Amarelo × ciano = verde onde se cobrem; ciano puro fora.
    let png = export::png_page(
        &back,
        0,
        &ExportOptions {
            dpi: 25.4,
            ..Default::default()
        },
    )
    .unwrap();
    let img = image::load_from_memory(&png).unwrap().to_rgba8();
    // 1 px = 1 mm; y da imagem cresce para baixo (página de 40 mm).
    assert_eq!(img.get_pixel(20, 20).0, [0, 255, 0, 255], "sobreposição");
    assert_eq!(img.get_pixel(30, 20).0, [0, 255, 255, 255], "só o de cima");
    let pdf = export::pdf_document(&back, &ExportOptions::default()).unwrap();
    let doc = lopdf::Document::load_mem(&pdf).unwrap();
    let has_bm = doc.objects.values().any(|o| {
        o.as_dict()
            .ok()
            .and_then(|d| d.get(b"BM").ok())
            .and_then(|b| b.as_name().ok())
            == Some(b"Multiply".as_slice())
    });
    assert!(has_bm, "ExtGState com /BM /Multiply");
    let svg = export::svg_page(&back, 0, &ExportOptions::default()).unwrap();
    assert!(svg.contains("mix-blend-mode:multiply"));
}
