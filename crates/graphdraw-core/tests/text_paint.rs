//! Preenchimento e contorno aplicados a texto chegam à cena (tela/PDF).

use graphdraw_core::edit::text::{insert, new_text};
use graphdraw_core::edit::{Editor, ops};
use graphdraw_core::model::{Color, Document, Fill, ObjectKind, Page, Stroke};
use graphdraw_core::render::{ImageCache, Item, Paint, SceneOptions, build_scene};

#[test]
fn text_fill_and_stroke_reach_the_scene() {
    if graphdraw_core::render::text::resolve_family("Arial", 400, false).is_none() {
        return;
    }
    let mut d = Document::new();
    d.data.pages.push(Page::new(100_000.0, 100_000.0));
    let mut ed = Editor::new(d);
    let mut o = new_text(10_000.0, 50_000.0, None, "Arial", 10_000.0);
    if let ObjectKind::Text(t) = &mut o.kind {
        insert(t, 0, "Abc");
    }
    let id = ops::add_object(&mut ed, 0, 0, o, "t");
    let red = Color::rgb(1.0, 0.0, 0.0);
    ops::set_fill(
        &mut ed,
        std::slice::from_ref(&id),
        Some(Fill::solid(red.clone())),
    );
    ops::set_stroke(
        &mut ed,
        std::slice::from_ref(&id),
        Some(Stroke::new(500.0, Color::black())),
    );
    let scene = build_scene(
        &ed.doc,
        &ed.doc.data.pages[0],
        SceneOptions { print: false },
        &ImageCache::default(),
    );
    let t = scene
        .items
        .iter()
        .find_map(|i| match i {
            Item::Text(t) => Some(t),
            _ => None,
        })
        .expect("texto na cena");
    let s = &t.outlines[0];
    match &s.fill {
        Some(Paint::Solid { color, .. }) => {
            assert_eq!(color.to_srgb(), [1.0, 0.0, 0.0], "preenchimento")
        }
        other => panic!("preenchimento: {other:?}"),
    }
    assert!(s.stroke.is_some(), "contorno");
}
