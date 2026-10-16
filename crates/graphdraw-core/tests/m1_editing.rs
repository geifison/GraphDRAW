//! Fixture `m1-editing.gdr`: contorno por posição (GD-OUT-04), edição dentro
//! de grupo aninhado (GD-SEL-02) e camada em esboço (GD-CAM-01).

mod common;

use common::*;
use graphdraw_core::edit::{Editor, ops, query};
use graphdraw_core::open_path;
use graphdraw_core::render::{ImageCache, Item, SceneOptions, build_scene};
use kurbo::{Point, Vec2};

const MM: f64 = 1000.0;

fn editor() -> Editor {
    Editor::new(open_path(&gdr_fixture("m1-editing.gdr")).unwrap().0)
}

fn id_named(ed: &Editor, name: &str) -> String {
    let mut found = None;
    ed.doc.walk_objects(&mut |o| {
        if o.name.as_deref() == Some(name) {
            found = Some(o.id.clone());
        }
    });
    found.unwrap_or_else(|| panic!("objeto '{name}' não encontrado"))
}

#[test]
fn stroke_positions_change_visual_bounds() {
    let ed = editor();
    let left = |name: &str| {
        let r = ed.find(&id_named(&ed, name)).unwrap();
        ops::object_bounds(&ed, &r, true).unwrap().x0
    };
    // Bordas em 10, 40 e 70 mm; contorno de 3 mm.
    assert!((left("Contorno center") - 8.5 * MM).abs() < 1e-6);
    assert!((left("Contorno inside") - 40.0 * MM).abs() < 1e-6);
    assert!((left("Contorno outside") - 67.0 * MM).abs() < 1e-6);
}

#[test]
fn nested_child_is_found_hit_and_moved() {
    let mut ed = editor();
    let id = id_named(&ed, "Círculo no grupo interno");
    let r = ed.find(&id).unwrap();
    assert_eq!(
        r.path,
        vec![1, 0],
        "grupo externo → grupo interno → círculo"
    );
    // Centro do círculo: 10 (externo) + 0 (interno) + 6 = 16 mm; y: 10 + 15 + 6 = 31 mm.
    let b = ops::object_bounds(&ed, &r, false).unwrap();
    assert!((b.center() - Point::new(16.0 * MM, 31.0 * MM)).hypot() < 1e-6);
    let page = &ed.doc.data.pages[0];
    assert_eq!(
        query::hit_test_deep(page, b.center(), 100.0).as_deref(),
        Some(id.as_str())
    );

    let outer = id_named(&ed, "Grupo externo");
    let outer_before = ops::object_bounds(&ed, &ed.find(&outer).unwrap(), false).unwrap();
    ops::translate(&mut ed, std::slice::from_ref(&id), Vec2::new(5.0 * MM, 0.0));
    let moved = ops::object_bounds(&ed, &ed.find(&id).unwrap(), false).unwrap();
    assert!((moved.center() - Point::new(21.0 * MM, 31.0 * MM)).hypot() < 1e-6);
    assert_eq!(
        ed.find(&outer).unwrap().loc.index,
        3,
        "o grupo continua no mesmo lugar da camada"
    );
    let outer_after = ops::object_bounds(&ed, &ed.find(&outer).unwrap(), false).unwrap();
    assert!(
        outer_after.x1 >= outer_before.x1,
        "caixa do grupo acompanha o filho"
    );
    ed.undo();
    let back = ops::object_bounds(&ed, &ed.find(&id).unwrap(), false).unwrap();
    assert!((back.center() - b.center()).hypot() < 1e-6);
}

#[test]
fn wireframe_layer_only_on_screen() {
    let ed = editor();
    let page = &ed.doc.data.pages[0];
    let sketch_id = &page.layers[1].objects[0].id;
    let fill_of = |print: bool| {
        let scene = build_scene(
            &ed.doc,
            page,
            SceneOptions { print },
            &ImageCache::default(),
        );
        scene.items.iter().find_map(|it| match it {
            Item::Shape(s) if &s.object_id == sketch_id => Some(s.fill.is_some()),
            _ => None,
        })
    };
    assert_eq!(
        fill_of(false),
        Some(false),
        "esboço na tela: sem preenchimento"
    );
    assert_eq!(fill_of(true), Some(true), "impressão sai normal");
}
