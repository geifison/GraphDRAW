//! GD-FIL-04..06/10/12: padrões no .gdr, na tela, no PDF e no SVG; caminho
//! aberto sem preenchimento quando a opção do documento está desligada.

use graphdraw_core::gdr;
use graphdraw_core::model::{
    Color, Document, Fill, Node, Object, ObjectKind, Page, PathData, Pattern, RectData, Subpath,
    Transform,
};
use graphdraw_core::render::raster::{RasterOptions, render};
use graphdraw_core::render::{ImageCache, Item, SceneOptions, build_scene};
use kurbo::Affine;

fn checker() -> Pattern {
    Pattern {
        kind: "two-color".into(),
        cells: vec!["10".into(), "01".into()],
        fg: Some(Color::rgb(0.0, 0.0, 0.0)),
        bg: Some(Color::rgb(1.0, 1.0, 1.0)),
        width: 10_000.0,
        height: 10_000.0,
        ..Pattern::default()
    }
}

fn doc_with(o: Object) -> Document {
    let mut d = Document::new();
    let mut p = Page::new(100_000.0, 100_000.0);
    p.layers[0].objects.push(o);
    d.data.pages.push(p);
    d
}

fn square() -> Object {
    let mut o = Object::new(ObjectKind::Rectangle(RectData {
        width: 100_000.0,
        height: 100_000.0,
        corners: None,
        extra: Default::default(),
    }));
    o.fill = Some(Fill::Pattern(Box::new(checker())));
    o
}

#[test]
fn gd_fil_06_pattern_round_trip_and_render() {
    let d = doc_with(square());
    let bytes = gdr::write_bytes(&d, &Default::default()).unwrap();
    let (back, _) = gdr::read_bytes(&bytes).unwrap();
    assert_eq!(
        back.data.pages[0].layers[0].objects[0].fill,
        d.data.pages[0].layers[0].objects[0].fill
    );
    // 1 px = 1 mm: células de 5 mm alternam preto e branco.
    let scene = build_scene(
        &back,
        &back.data.pages[0],
        SceneOptions { print: false },
        &ImageCache::default(),
    );
    let view = Affine::new([0.001, 0.0, 0.0, -0.001, 0.0, 100.0]);
    let pm = render(&scene, view, 100, 100, &RasterOptions::default()).unwrap();
    let at = |x: u32, y: u32| pm.pixel(x, y).unwrap().red();
    // Linha de cima da página = topo do ladrilho: célula "1" (preta) à esquerda.
    assert!(at(2, 2) < 60, "preto: {}", at(2, 2));
    assert!(at(7, 2) > 200, "branco: {}", at(7, 2));
    assert!(at(12, 2) < 60, "repete a cada 10 mm");
    assert!(at(2, 7) > 200, "linha de baixo do ladrilho invertida");
}

#[test]
fn gd_fil_04_pattern_exports() {
    let d = doc_with(square());
    let pdf = graphdraw_core::export::pdf_document(&d, &Default::default()).unwrap();
    let has = |pat: &[u8]| pdf.windows(pat.len()).any(|w| w == pat);
    assert!(
        has(b"/PatternType 1") && has(b"/Pattern"),
        "padrão de ladrilho no PDF"
    );
    let svg = graphdraw_core::export::svg_page(&d, 0, &Default::default()).unwrap();
    assert!(svg.contains("<pattern") && svg.contains("url(#pt"));
    // PDF/X-1a: o padrão sai achatado (CMYK), sem padrão RGB.
    let x = graphdraw_core::export::pdf_document(
        &d,
        &graphdraw_core::export::ExportOptions {
            pdf: graphdraw_core::export::PdfStandard::X1a(Default::default()),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(!x.windows(14).any(|w| w == b"/PatternType 1"));
}

/// GD-FIL-10: sem "transformar com o objeto", girar o objeto não gira o
/// padrão (a cena leva a mesma matriz do padrão).
#[test]
fn gd_fil_10_pattern_fixed_to_page() {
    let mut o = square();
    o.transform = Transform::from_affine(Affine::rotate(0.3));
    let paint_m = |o: &Object| {
        let d = doc_with(o.clone());
        let scene = build_scene(
            &d,
            &d.data.pages[0],
            SceneOptions { print: false },
            &ImageCache::default(),
        );
        match &scene.items[0] {
            Item::Shape(s) => match &s.fill {
                Some(graphdraw_core::render::Paint::Pattern { transform, .. }) => *transform,
                _ => panic!("padrão"),
            },
            _ => panic!(),
        }
    };
    assert_eq!(paint_m(&o), Affine::rotate(0.3));
    if let Some(Fill::Pattern(p)) = &mut o.fill {
        p.transform_with_object = false;
    }
    assert_eq!(paint_m(&o), Affine::IDENTITY);
}

/// GD-FIL-12: linha aberta com preenchimento: preenche por padrão; com a
/// opção desligada, só o traço.
#[test]
fn gd_fil_12_open_paths_fill_option() {
    let mut o = Object::new(ObjectKind::Path(PathData {
        subpaths: vec![Subpath {
            nodes: vec![
                Node::corner(0.0, 0.0),
                Node::corner(50_000.0, 50_000.0),
                Node::corner(100_000.0, 0.0),
            ],
            closed: false,
            extra: Default::default(),
        }],
        fill_rule: Default::default(),
        extra: Default::default(),
    }));
    o.fill = Some(Fill::solid(Color::black()));
    let mut d = doc_with(o);
    let filled = |d: &Document| {
        let scene = build_scene(
            d,
            &d.data.pages[0],
            SceneOptions { print: false },
            &ImageCache::default(),
        );
        scene
            .items
            .iter()
            .any(|i| matches!(i, Item::Shape(s) if s.fill.is_some()))
    };
    assert!(filled(&d));
    d.data.settings.fill_open_paths = false;
    assert!(!filled(&d));
    let json = serde_json::to_value(&d.data.settings).unwrap();
    assert_eq!(json["fillOpenPaths"], false);
}
