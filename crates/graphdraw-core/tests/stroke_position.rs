//! GD-OUT-04: contorno centrado, interno e externo, na tela/PNG e no SVG.

use graphdraw_core::export::{self, ExportOptions};
use graphdraw_core::model::{
    Color, Document, Fill, Object, ObjectKind, Page, RectData, Stroke, Transform,
};
use graphdraw_core::render::raster::{self, RasterOptions};
use graphdraw_core::render::{ImageCache, SceneOptions, build_scene};
use kurbo::Affine;
use serde_json::Map;

/// Página 100×100 µm com um quadrado 60×60 em (20,20), contorno vermelho de
/// 10 µm na posição pedida e preenchimento branco.
fn doc(position: &str) -> Document {
    let mut d = Document::new();
    let mut page = Page::new(100.0, 100.0);
    let mut o = Object::new(ObjectKind::Rectangle(RectData {
        width: 60.0,
        height: 60.0,
        corners: None,
        extra: Map::new(),
    }));
    o.transform = Transform::translate(20.0, 20.0);
    o.fill = Some(Fill::solid(Color::rgb(1.0, 1.0, 1.0)));
    let mut s = Stroke::new(10.0, Color::rgb(1.0, 0.0, 0.0));
    s.position = position.into();
    o.stroke = Some(s);
    page.layers[0].objects.push(o);
    d.data.pages.push(page);
    d
}

/// Renderiza a 1 px/µm sobre fundo preto e devolve "é vermelho?" no pixel
/// da coluna `x` (página) na linha do meio.
fn red_at(d: &Document, xs: &[f64]) -> Vec<bool> {
    let scene = build_scene(
        d,
        &d.data.pages[0],
        SceneOptions { print: true },
        &ImageCache::default(),
    );
    let view = Affine::new([1.0, 0.0, 0.0, -1.0, 0.0, 100.0]);
    let pm = raster::render(
        &scene,
        view,
        100,
        100,
        &RasterOptions {
            antialias: false,
            background: Some([0, 0, 0, 255]),
        },
    )
    .unwrap();
    xs.iter()
        .map(|x| {
            let px = pm.pixel(*x as u32, 50).unwrap();
            px.red() > 200 && px.green() < 50
        })
        .collect()
}

#[test]
fn gd_out_04_raster_positions() {
    // Borda esquerda em x=20. Amostras: 13 (fora, a 7), 17 (fora, a 3),
    // 23 (dentro, a 3), 27 (dentro, a 7).
    let xs = [13.5, 17.5, 22.5, 27.5];
    assert_eq!(red_at(&doc("center"), &xs), [false, true, true, false]);
    assert_eq!(red_at(&doc("inside"), &xs), [false, false, true, true]);
    assert_eq!(red_at(&doc("outside"), &xs), [true, true, false, false]);
}

#[test]
fn gd_out_04_svg_uses_clip_and_mask() {
    let opts = ExportOptions::default();
    let inside = export::svg_page(&doc("inside"), 0, &opts).unwrap();
    assert!(inside.contains("<clipPath"), "{inside}");
    assert!(inside.contains(r#"stroke-width="20""#));
    let outside = export::svg_page(&doc("outside"), 0, &opts).unwrap();
    assert!(outside.contains("<mask"), "{outside}");
    roxmltree::Document::parse(&outside).expect("SVG malformado");
    let center = export::svg_page(&doc("center"), 0, &opts).unwrap();
    assert!(!center.contains("<clipPath") && !center.contains("<mask"));
    assert!(center.contains(r#"stroke-width="10""#));
}

#[test]
fn gd_out_04_visual_bounds_follow_position() {
    use graphdraw_core::edit::query::bounds;
    let b = |p: &str| {
        let d = doc(p);
        bounds(
            &d.data.pages[0].layers[0].objects[0],
            Affine::IDENTITY,
            true,
        )
        .unwrap()
        .x0
    };
    assert_eq!(b("center"), 15.0);
    assert_eq!(b("inside"), 20.0);
    assert_eq!(b("outside"), 10.0);
}
