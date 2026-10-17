//! Importação `.cdr`/`.cmx` (doc 14). Requer o `gd-cdr-bridge` compilado
//! (native/cdr-bridge/build.sh); sem ele os testes são pulados, a menos
//! que `GRAPHDRAW_REQUIRE_BRIDGE` esteja definida.

mod common;

use common::*;
use graphdraw_core::model::{Fill, ObjectKind};
use graphdraw_core::{Error, open_bytes, open_path};

const UM_PER_IN: f64 = 25_400.0;

#[test]
fn gd_cdr_01_detects_by_content_not_extension() {
    if !bridge_available() {
        return;
    }
    let bytes = std::fs::read(cdr_fixture("fdo65220-2.cdr")).unwrap();
    assert_eq!(
        graphdraw_core::open::detect(&bytes, None),
        Some(graphdraw_core::FileFormat::Cdr)
    );
    let (doc, report) = open_bytes(&bytes, Some("qualquer")).unwrap();
    assert_eq!(report.source_format, "cdr");
    assert_eq!(doc.data.pages.len(), 1);
    let cmx = std::fs::read(cdr_fixture("fdo63782-1.cmx")).unwrap();
    assert_eq!(
        graphdraw_core::open::detect(&cmx, None),
        Some(graphdraw_core::FileFormat::Cmx)
    );
}

#[test]
fn gd_cdr_02_maps_pages_paths_colors_and_coordinates() {
    if !bridge_available() {
        return;
    }
    let (doc, report) = open_path(&cdr_fixture("fdo65220-2.cdr")).unwrap();
    assert_eq!(report.detected_version.as_deref(), Some("CDR versão 15"));
    let page = &doc.data.pages[0];
    // A4: 8,2677 × 11,6929 pol.
    assert!(
        (page.width - 210_000.0).abs() < 50.0,
        "largura {}",
        page.width
    );
    assert!(
        (page.height - 297_000.0).abs() < 50.0,
        "altura {}",
        page.height
    );
    let objs = &page.layers[0].objects;
    assert_eq!(objs.len(), 5);
    let ObjectKind::Path(p) = &objs[0].kind else {
        panic!("esperado caminho")
    };
    assert_eq!(p.subpaths.len(), 1);
    assert!(p.subpaths[0].closed);
    assert_eq!(p.subpaths[0].nodes.len(), 4, "nó final coincidente fundido");
    // Primeiro ponto: (0,8134 pol; 1,3911 pol a partir do topo) → Y para cima.
    let n0 = p.subpaths[0].nodes[0].p;
    assert!((n0[0] - 0.8134 * UM_PER_IN).abs() < 5.0);
    assert!((n0[1] - (page.height - 1.3911 * UM_PER_IN)).abs() < 5.0);
    match &objs[0].fill {
        Some(Fill::Solid { color, .. }) => assert_eq!(color.to_hex(), "#0000ff"),
        other => panic!("preenchimento inesperado: {other:?}"),
    }
    let stroke = objs[0].stroke.as_ref().unwrap();
    assert!((stroke.width - 0.0079 * UM_PER_IN).abs() < 1.0);
    assert_eq!(stroke.color.to_hex(), "#000000");
    // Transparência uniforme do preenchimento (draw:opacity 0%).
    match &objs[4].fill {
        Some(Fill::Solid { opacity, .. }) => assert_eq!(*opacity, 0.0),
        other => panic!("preenchimento inesperado: {other:?}"),
    }
}

#[test]
fn gd_cdr_02_cmx_curves_strokes_render() {
    if !bridge_available() {
        return;
    }
    let (doc, report) = open_path(&cdr_fixture("fdo63782-1.cmx")).unwrap();
    assert_eq!(report.source_format, "cmx");
    let mut count = 0;
    let mut curves = 0;
    doc.walk_objects(&mut |o| {
        count += 1;
        if let ObjectKind::Path(p) = &o.kind {
            curves += p
                .subpaths
                .iter()
                .flat_map(|s| &s.nodes)
                .filter(|n| n.handle_out.is_some())
                .count();
        }
    });
    assert_eq!(count, 116);
    assert!(curves > 100, "curvas Bézier preservadas");
    // Fidelidade: renderização comparada com a referência aprovada.
    let png = graphdraw_core::export::png_page(
        &doc,
        0,
        &graphdraw_core::export::ExportOptions {
            dpi: 72.0,
            background: Some([255; 4]),
            ..Default::default()
        },
    )
    .unwrap();
    let got = image::load_from_memory(&png).unwrap().to_rgba8();
    let reference = image::open(cdr_fixture("expected/fdo63782-1.png"))
        .unwrap()
        .to_rgba8();
    assert_eq!(got.dimensions(), reference.dimensions());
    let diff: f64 = got
        .pixels()
        .zip(reference.pixels())
        .map(|(a, b)| {
            (0..3)
                .map(|i| (a.0[i] as f64 - b.0[i] as f64).abs())
                .sum::<f64>()
        })
        .sum::<f64>()
        / (got.width() * got.height() * 3) as f64;
    assert!(
        diff < 2.0,
        "diferença média por canal {diff:.3} acima do limiar"
    );
}

/// GD-CDR-03 / GD-EXP-05: relatório de importação sempre presente.
#[test]
fn gd_cdr_03_import_report() {
    if !bridge_available() {
        return;
    }
    let (_, report) = open_path(&cdr_fixture("fdo65220-2.cdr")).unwrap();
    assert!(
        report.has("cdr.color.rgb-only"),
        "limitação de cor deve constar no relatório"
    );
    let json = serde_json::to_value(&report).unwrap();
    assert_eq!(json["sourceFormat"], "cdr");
    assert!(
        json["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|i| i["message"].is_string() && i["severity"].is_string())
    );
}

/// GD-CDR-06: arquivo não suportado/corrompido → mensagem clara com versão, nunca trava.
#[test]
fn gd_cdr_06_unsupported_and_corrupt_files() {
    if !bridge_available() {
        return;
    }
    match open_path(&cdr_fixture("EDB-35217-2.cdr")) {
        Err(Error::Cdr(msg)) => {
            assert!(msg.contains("versão detectada"), "{msg}");
            assert!(msg.contains("não suportado"), "{msg}");
        }
        other => panic!("esperado erro, veio {:?}", other.map(|_| ())),
    }
    // RIFF de versão reconhecível mas conteúdo truncado.
    let mut fake = b"RIFF\x10\x00\x00\x00CDRDvrsn".to_vec();
    fake.extend_from_slice(&[0u8; 32]);
    match open_bytes(&fake, None) {
        Err(Error::Cdr(msg)) => assert!(msg.contains("CDR versão 13"), "{msg}"),
        other => panic!("esperado erro, veio {:?}", other.map(|_| ())),
    }
    // Arquivo real cortado ao meio: erro ou importação parcial com aviso, sem pânico.
    let bytes = std::fs::read(cdr_fixture("fdo65220-2.cdr")).unwrap();
    for cut in [bytes.len() / 2, bytes.len() - 100, 64] {
        match open_bytes(&bytes[..cut], Some("cdr")) {
            Ok((_, report)) => assert!(report.has("cdr.partial") || !report.items.is_empty()),
            Err(Error::Cdr(msg)) => assert!(msg.contains("versão detectada"), "{msg}"),
            Err(e) => panic!("erro inesperado: {e}"),
        }
    }
}

/// GD-CDR-07 + GD-GDR-08: o documento importado vira `.gdr` válido e estável.
#[test]
fn gd_cdr_07_imported_documents_save_as_valid_gdr() {
    if !bridge_available() {
        return;
    }
    let v = schema_validator();
    for name in ["fdo65220-2.cdr", "fdo63782-1.cmx"] {
        let (doc, _) = open_path(&cdr_fixture(name)).unwrap();
        let bytes = graphdraw_core::gdr::write_bytes(&doc, &Default::default()).unwrap();
        let json: serde_json::Value =
            serde_json::from_slice(&member(&bytes, "document.json").unwrap()).unwrap();
        assert_valid(&v, &json, name);
        let (back, _) = graphdraw_core::gdr::read_bytes(&bytes).unwrap();
        assert_eq!(back.data, doc.data);
    }
}

/// Regressão com as amostras livres (conselho de 2026-10-17): cada uma abre
/// com o mesmo relatório e exporta em todos os formatos de produção sem
/// erro.
#[test]
fn fixtures_open_with_stable_report_and_export_everywhere() {
    use graphdraw_core::export::cut::{CutOptions, cut_strokes, dxf, hpgl};
    use graphdraw_core::export::{self, ExportOptions, PdfStandard};
    if !bridge_available() {
        return;
    }
    for (name, objects) in [("fdo65220-2.cdr", 5usize), ("fdo63782-1.cmx", 116)] {
        let (doc, report) = open_path(&cdr_fixture(name)).unwrap();
        let mut codes: Vec<&str> = report.items.iter().map(|i| i.code.as_str()).collect();
        codes.sort();
        codes.dedup();
        assert_eq!(
            codes,
            vec!["cdr.color.rgb-only"],
            "{name}: avisos novos na importação"
        );
        let mut n = 0;
        doc.walk_objects(&mut |_| n += 1);
        assert_eq!(n, objects, "{name}");
        let pdf = export::pdf_document(&doc, &ExportOptions::default()).unwrap();
        assert!(lopdf::Document::load_mem(&pdf).is_ok(), "{name}: PDF");
        let x1a = ExportOptions {
            pdf: PdfStandard::X1a(Default::default()),
            text_to_curves: true,
            ..Default::default()
        };
        assert!(export::pdf_document(&doc, &x1a).is_ok(), "{name}: PDF/X-1a");
        let svg = export::svg_page(&doc, 0, &ExportOptions::default()).unwrap();
        assert!(
            roxmltree::Document::parse(&svg).is_ok(),
            "{name}: SVG bem formado"
        );
        assert!(
            export::png_page(
                &doc,
                0,
                &ExportOptions {
                    dpi: 72.0,
                    ..Default::default()
                }
            )
            .is_ok()
        );
        let opts = CutOptions {
            only_marked: false,
            ..Default::default()
        };
        let strokes = cut_strokes(&doc, 0, &opts);
        assert!(!strokes.is_empty(), "{name}: contornos para corte");
        let w = doc.data.pages[0].width;
        assert!(hpgl(&strokes, w, &opts).starts_with("IN"), "{name}: PLT");
        assert!(dxf(&strokes, &opts, w).contains("EOF"), "{name}: DXF");
    }
}

/// Regressão da renderização da amostra .cdr (referência gerada e conferida
/// a olho em 2026-10-17; não é referência do autor).
#[test]
fn cdr_render_matches_regression_reference() {
    if !bridge_available() {
        return;
    }
    let (doc, _) = open_path(&cdr_fixture("fdo65220-2.cdr")).unwrap();
    let png = graphdraw_core::export::png_page(
        &doc,
        0,
        &graphdraw_core::export::ExportOptions {
            dpi: 72.0,
            background: Some([255; 4]),
            ..Default::default()
        },
    )
    .unwrap();
    let got = image::load_from_memory(&png).unwrap().to_rgba8();
    let reference = image::open(cdr_fixture("expected/fdo65220-2.png"))
        .unwrap()
        .to_rgba8();
    assert_eq!(got.dimensions(), reference.dimensions());
    let diff: f64 = got
        .pixels()
        .zip(reference.pixels())
        .map(|(a, b)| {
            (0..3)
                .map(|i| (a.0[i] as f64 - b.0[i] as f64).abs())
                .sum::<f64>()
        })
        .sum::<f64>()
        / (got.width() * got.height() * 3) as f64;
    assert!(diff < 1.0, "diferença média por canal {diff:.3}");
}
