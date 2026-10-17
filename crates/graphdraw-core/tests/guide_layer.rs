//! GD-GUI-03: objeto vira guia — vai para a camada de guias, aparece
//! tracejado na tela, sai da impressão e continua servindo de encaixe.

use graphdraw_core::edit::{Editor, ops, snap};
use graphdraw_core::model::{Color, Document, Fill, Object, ObjectKind, Page, RectData, Transform};
use graphdraw_core::render::{ImageCache, Item, SceneOptions, build_scene};

#[test]
fn gd_gui_03_object_becomes_guide() {
    let mut d = Document::new();
    d.data.pages.push(Page::new(100_000.0, 100_000.0));
    let mut ed = Editor::new(d);
    let mut o = Object::new(ObjectKind::Rectangle(RectData {
        width: 30_000.0,
        height: 20_000.0,
        corners: None,
        extra: Default::default(),
    }));
    o.transform = Transform::translate(10_000.0, 10_000.0);
    o.fill = Some(Fill::solid(Color::black()));
    let id = ops::add_object(&mut ed, 0, 0, o, "r");
    let undo = ed.undo_len();
    let layer = ops::to_guides(&mut ed, std::slice::from_ref(&id), 0).unwrap();
    assert_eq!(ed.undo_len(), undo + 1);
    let page = &ed.doc.data.pages[0];
    assert!(page.layers[layer].guides && !page.layers[layer].printable);
    assert_eq!(page.layers[layer].objects[0].id, id);
    let screen = build_scene(
        &ed.doc,
        page,
        SceneOptions { print: false },
        &ImageCache::default(),
    );
    let Item::Shape(s) = &screen.items[0] else {
        panic!()
    };
    assert!(
        s.fill.is_none() && s.stroke.as_ref().is_some_and(|st| st.dash.is_some()),
        "tracejado sem preenchimento"
    );
    let print = build_scene(
        &ed.doc,
        page,
        SceneOptions { print: true },
        &ImageCache::default(),
    );
    assert!(print.items.is_empty(), "fora da impressão");
    let snaps = snap::candidates_doc(&ed.doc, 0, &Default::default(), &[]);
    assert!(
        snaps
            .iter()
            .any(|(p, _)| (p.x - 40_000.0).abs() < 1.0 && (p.y - 30_000.0).abs() < 1.0),
        "encaixa no canto"
    );
    // Segunda conversão reaproveita a camada.
    ed.undo();
    assert!(ed.doc.data.pages[0].layers.iter().all(|l| !l.guides));
}
