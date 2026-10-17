//! GD-OUT-05: pontas de seta gravadas no .gdr e desenhadas na cena.

use graphdraw_core::gdr;
use graphdraw_core::model::{
    Arrow, Color, Document, Node, Object, ObjectKind, Page, PathData, Stroke, Subpath,
};
use graphdraw_core::render::{ImageCache, Item, SceneOptions, build_scene};

#[test]
fn gd_out_05_arrows_round_trip_and_render() {
    let mut d = Document::new();
    let mut p = Page::new(100_000.0, 100_000.0);
    let mut o = Object::new(ObjectKind::Path(PathData {
        subpaths: vec![Subpath {
            closed: false,
            nodes: vec![
                Node::corner(10_000.0, 50_000.0),
                Node::corner(90_000.0, 50_000.0),
            ],
            extra: Default::default(),
        }],
        fill_rule: Default::default(),
        extra: Default::default(),
    }));
    let mut s = Stroke::new(500.0, Color::black());
    s.end_arrow = Some(Arrow::new("triangle"));
    let mut start = Arrow::new("circle");
    start.scale = 2.0;
    s.start_arrow = Some(start);
    o.stroke = Some(s);
    p.layers[0].objects.push(o);
    d.data.pages.push(p);
    let bytes = gdr::write_bytes(&d, &Default::default()).unwrap();
    let (back, _) = gdr::read_bytes(&bytes).unwrap();
    let st = back.data.pages[0].layers[0].objects[0]
        .stroke
        .clone()
        .unwrap();
    assert_eq!(st.end_arrow, Some(Arrow::new("triangle")));
    assert_eq!(st.start_arrow.unwrap().scale, 2.0);
    let scene = build_scene(
        &back,
        &back.data.pages[0],
        SceneOptions { print: false },
        &ImageCache::default(),
    );
    let shapes: Vec<_> = scene
        .items
        .iter()
        .filter_map(|i| match i {
            Item::Shape(s) => Some(s),
            _ => None,
        })
        .collect();
    assert_eq!(shapes.len(), 3, "traço + duas pontas");
    assert!(shapes[0].stroke.is_some() && shapes[1].fill.is_some() && shapes[2].fill.is_some());
}
