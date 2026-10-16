//! Exportação SVG/PDF/PNG (doc 15) e regras de impressão de camadas (GD-CAM-01).

mod common;

use common::*;
use graphdraw_core::export::{self, ExportFormat, ExportOptions};
use graphdraw_core::open_path;
use graphdraw_core::render::{ImageCache, Item, SceneOptions, build_scene};

fn open(name: &str) -> graphdraw_core::Document {
    open_path(&gdr_fixture(name)).unwrap().0
}

fn svg_doc(svg: &str) -> roxmltree::Document<'_> {
    roxmltree::Document::parse(svg).expect("SVG malformado")
}

#[test]
fn svg_is_well_formed_with_physical_size() {
    let doc = open("basic-shapes.gdr");
    let svg = export::svg_page(&doc, 0, &ExportOptions::default()).unwrap();
    let x = svg_doc(&svg);
    let root = x.root_element();
    assert_eq!(root.attribute("width"), Some("210mm"));
    assert_eq!(root.attribute("height"), Some("297mm"));
    assert_eq!(root.attribute("viewBox"), Some("0 0 210 297"));
    let paths = x.descendants().filter(|n| n.has_tag_name("path")).count();
    assert_eq!(paths, 7, "7 formas (o grupo tem 2 filhas)");
    assert!(svg.contains(r#"fill-rule="evenodd""#));
    assert!(svg.contains(r#"stroke-dasharray="6000 3000""#));
    assert!(svg.contains(r#"stroke-linecap="round""#));
}

#[test]
fn svg_gradients_images_text() {
    let svg = export::svg_page(&open("gradients.gdr"), 0, &ExportOptions::default()).unwrap();
    let x = svg_doc(&svg);
    assert_eq!(
        x.descendants()
            .filter(|n| n.has_tag_name("linearGradient"))
            .count(),
        1
    );
    assert_eq!(
        x.descendants()
            .filter(|n| n.has_tag_name("radialGradient"))
            .count(),
        1
    );
    assert!(svg.contains(r#"fill-opacity="0.4""#));

    let svg = export::svg_page(&open("image.gdr"), 0, &ExportOptions::default()).unwrap();
    let x = svg_doc(&svg);
    let images: Vec<_> = x
        .descendants()
        .filter(|n| n.has_tag_name("image"))
        .collect();
    assert_eq!(images.len(), 2);
    assert!(
        images[0]
            .attribute(("http://www.w3.org/1999/xlink", "href"))
            .unwrap()
            .starts_with("data:image/png;base64,")
    );

    // GD-EXP-02: texto vivo por padrão; curvas com --text-to-curves.
    let doc = open("text.gdr");
    let live = export::svg_page(&doc, 0, &ExportOptions::default()).unwrap();
    let x = svg_doc(&live);
    let texts: Vec<String> = x
        .descendants()
        .filter(|n| n.has_tag_name("text"))
        .filter_map(|n| n.text().map(String::from))
        .collect();
    assert!(texts.iter().any(|t| t.contains("GraphDraw")), "{texts:?}");
    assert!(
        live.contains(r#"font-family="Fonte Que Nao Existe GD""#),
        "nome original da fonte deve ser mantido"
    );
    let curves = export::svg_page(
        &doc,
        0,
        &ExportOptions {
            text_to_curves: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(!curves.contains("<text"));
    if graphdraw_core::render::text::resolve_family("Arial", 400, false).is_some() {
        assert!(
            svg_doc(&curves)
                .descendants()
                .filter(|n| n.has_tag_name("path"))
                .count()
                > 3
        );
    }
}

#[test]
fn pdf_structure_pages_and_sizes() {
    let doc = open("multipage.gdr");
    let pdf = export::pdf_document(&doc, &ExportOptions::default()).unwrap();
    assert!(pdf.starts_with(b"%PDF-1.7"));
    let text = String::from_utf8_lossy(&pdf);
    assert_eq!(
        text.matches("/Type /Page\n").count()
            + text.matches("/Type /Page ").count()
            + text.matches("/Type /Page/").count(),
        3,
        "3 páginas"
    );
    // A4 = 595,28 × 841,89 pt; cartão 90×50 mm = 255,12 × 141,73 pt.
    assert!(
        text.contains("595.27") || text.contains("595.28"),
        "MediaBox A4"
    );
    assert!(
        text.contains("255.11") || text.contains("255.12"),
        "MediaBox cartão"
    );

    // GD-EXP-01: intervalo de páginas.
    let one = export::pdf_document(
        &doc,
        &ExportOptions {
            pages: vec![1],
            ..Default::default()
        },
    )
    .unwrap();
    assert!(String::from_utf8_lossy(&one).contains("/Count 1"));
}

#[test]
fn pdf_keeps_cmyk_and_spot_colors() {
    let pdf = export::pdf_document(&open("colors.gdr"), &ExportOptions::default()).unwrap();
    let text = String::from_utf8_lossy(&pdf);
    let content = inflate_all_streams(&pdf);
    assert!(
        content.contains("0 1 1 0 k"),
        "CMYK deve sair como DeviceCMYK (operador k)"
    );
    assert!(content.contains("0.5 g"), "cinza deve sair como DeviceGray");
    assert!(
        text.contains("/Separation") && text.contains("/Laranja#20Especial#20021"),
        "cor especial como Separation"
    );
    assert!(text.contains("/All"), "registro como Separation /All");
}

#[test]
fn pdf_gradients_opacity_images() {
    let pdf = export::pdf_document(&open("gradients.gdr"), &ExportOptions::default()).unwrap();
    let text = String::from_utf8_lossy(&pdf);
    assert!(text.contains("/ShadingType 2") && text.contains("/ShadingType 3"));
    assert!(text.contains("/ca 0.4"));
    let pdf = export::pdf_document(&open("image.gdr"), &ExportOptions::default()).unwrap();
    let text = String::from_utf8_lossy(&pdf);
    assert!(text.contains("/Subtype /Image"));
    assert!(
        text.contains("/SMask"),
        "alfa da imagem preservado como SMask"
    );
}

#[test]
fn png_dpi_size_background_and_antialias() {
    let doc = open("multipage.gdr");
    let png = export::png_page(
        &doc,
        0,
        &ExportOptions {
            dpi: 72.0,
            ..Default::default()
        },
    )
    .unwrap();
    let img = image::load_from_memory(&png).unwrap();
    assert_eq!((img.width(), img.height()), (595, 842));
    // Fundo transparente por padrão; branco quando pedido.
    let doc = open("layers-print.gdr");
    let t = export::png_page(
        &doc,
        0,
        &ExportOptions {
            dpi: 25.4,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(png_pixel(&t, 50, 50)[3], 0);
    let w = export::png_page(
        &doc,
        0,
        &ExportOptions {
            dpi: 25.4,
            background: Some([255, 255, 255, 255]),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(png_pixel(&w, 50, 50), [255, 255, 255, 255]);
    // pHYs grava a resolução.
    let decoder = png::Decoder::new(std::io::Cursor::new(&png[..]));
    let reader = decoder.read_info().unwrap();
    let dims = reader.info().pixel_dims.unwrap();
    assert_eq!(dims.xppu, (72.0f64 / 0.0254).round() as u32);
    // Sem anti-serrilhado: só pixels totalmente opacos ou transparentes na borda.
    let doc = open("basic-shapes.gdr");
    let hard = export::png_page(
        &doc,
        0,
        &ExportOptions {
            dpi: 50.0,
            antialias: false,
            ..Default::default()
        },
    )
    .unwrap();
    let img = image::load_from_memory(&hard).unwrap().to_rgba8();
    assert!(
        img.pixels().all(|p| p.0[3] == 0 || p.0[3] >= 200),
        "pixels parcialmente transparentes sem anti-serrilhado"
    );
}

/// GD-CAM-01 (aceite): camada não imprimível não aparece em PDF de
/// impressão, mas aparece na tela. GD-OBJ-04: objeto não imprimível idem.
#[test]
fn gd_cam_01_non_printable_layer_only_on_screen() {
    let doc = open("layers-print.gdr");
    let page = &doc.data.pages[0];
    let cache = ImageCache::default();
    let count = |print| {
        build_scene(&doc, page, SceneOptions { print }, &cache)
            .items
            .iter()
            .filter(|i| matches!(i, Item::Shape(_)))
            .count()
    };
    assert_eq!(
        count(false),
        3,
        "tela: arte + objeto não imprimível + camada não imprimível (a oculta não)"
    );
    assert_eq!(count(true), 1, "impressão: só a arte");

    let pdf = export::pdf_document(&doc, &ExportOptions::default()).unwrap();
    let content = inflate_all_streams(&pdf);
    assert!(content.contains("1 0 0 rg"), "vermelho imprimível no PDF");
    assert!(
        !content.contains("0 0 1 rg"),
        "camada não imprimível (azul) não pode ir ao PDF"
    );
    assert!(
        !content.contains("0 1 0 rg"),
        "objeto não imprimível (verde) não pode ir ao PDF"
    );
    assert!(
        !content.contains("1 0 1 rg"),
        "camada oculta (magenta) não pode ir ao PDF"
    );
}

#[test]
fn export_dispatch_multipage_suffixes_and_single_page() {
    let doc = open("multipage.gdr");
    let dir = tempfile::tempdir().unwrap();
    let written = export::export(
        &doc,
        &dir.path().join("p.svg"),
        ExportFormat::Svg,
        &ExportOptions::default(),
    )
    .unwrap();
    let names: Vec<_> = written
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, ["p-1.svg", "p-2.svg", "p-3.svg"]);
    let written = export::export(
        &doc,
        &dir.path().join("so2.png"),
        ExportFormat::Png,
        &ExportOptions {
            pages: vec![1],
            dpi: 10.0,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(written.len(), 1);
    assert!(written[0].ends_with("so2.png"));
    // Master page aparece na página 1 e não na 3 (hideMaster).
    let p1 = export::svg_page(&doc, 0, &ExportOptions::default()).unwrap();
    let p3 = export::svg_page(&doc, 2, &ExportOptions::default()).unwrap();
    assert_eq!(
        svg_doc(&p1)
            .descendants()
            .filter(|n| n.has_tag_name("path"))
            .count(),
        2
    );
    assert_eq!(
        svg_doc(&p3)
            .descendants()
            .filter(|n| n.has_tag_name("path"))
            .count(),
        1
    );
}

#[test]
fn gd_fnt_02_missing_font_reported_and_name_kept() {
    let (doc, report) = open_path(&gdr_fixture("text.gdr")).unwrap();
    if graphdraw_core::render::text::resolve_family("x", 400, false).is_none() {
        assert!(report.has("font.none"));
        return;
    }
    assert!(report.has("font.missing"), "{}", report.to_text());
    assert!(
        report
            .items
            .iter()
            .any(|i| i.message.contains("Fonte Que Nao Existe GD"))
    );
    let v = serde_json::to_value(&doc.data).unwrap();
    assert!(v.to_string().contains("Fonte Que Nao Existe GD"));
}

/// GD-NF-02: a renderização em faixas paralelas com descarte por caixa deve
/// produzir o mesmo resultado que a renderização sequencial.
#[test]
fn gd_nf_02_parallel_bands_match_sequential() {
    use graphdraw_core::render::raster::{self, RasterOptions};
    for name in [
        "basic-shapes.gdr",
        "gradients.gdr",
        "image.gdr",
        "text.gdr",
        "multipage.gdr",
    ] {
        let doc = open(name);
        let page = &doc.data.pages[0];
        let scene = build_scene(
            &doc,
            page,
            SceneOptions { print: false },
            &ImageCache::default(),
        );
        let (w, h) = (701u32, 997u32);
        let s = (w as f64 / page.width).min(h as f64 / page.height) * 1.7;
        let view = kurbo::Affine::new([s, 0.0, 0.0, -s, -40.0, page.height * s - 300.0]);
        let opts = RasterOptions {
            antialias: true,
            background: Some([250, 250, 250, 255]),
        };
        let parallel = raster::render(&scene, view, w, h, &opts).unwrap();
        let mut seq = tiny_skia::Pixmap::new(w, h).unwrap();
        seq.fill(tiny_skia::Color::from_rgba8(250, 250, 250, 255));
        raster::draw_items(&mut seq, &scene.items, view, &opts);
        // Curvas cortadas na borda da faixa podem variar alguns níveis de
        // anti-serrilhado; objeto ausente ou duplicado daria diferença ~255.
        let diffs: Vec<u8> = parallel
            .data()
            .iter()
            .zip(seq.data())
            .map(|(a, b)| a.abs_diff(*b))
            .collect();
        let max = diffs.iter().copied().max().unwrap_or(0);
        let edge = diffs
            .chunks(4)
            .filter(|px| px.iter().any(|d| *d > 1))
            .count();
        assert!(
            max <= 64,
            "{name}: diferença máxima {max} indica objeto ausente/duplicado"
        );
        assert!(
            edge * 1000 <= (w * h) as usize,
            "{name}: {edge} pixels divergentes (> 0,1%)"
        );
    }
}
