//! GD-FID-30: a ferramenta de fidelidade roda sobre um corpus sintético
//! (gerado aqui, nunca o corpus privado) e classifica as páginas.

use graphdraw_core::export::{self, ExportOptions};
use graphdraw_core::fidelity::{self, Level};
use graphdraw_core::gdr;
use graphdraw_core::model::{Color, Document, Fill, Object, ObjectKind, Page, RectData, Transform};
use serde_json::Map;

fn doc(color: Color) -> Document {
    let mut d = Document::new();
    let mut p = Page::new(100_000.0, 60_000.0);
    let mut o = Object::new(ObjectKind::Rectangle(RectData {
        width: 50_000.0,
        height: 30_000.0,
        corners: None,
        extra: Map::new(),
    }));
    o.transform = Transform::translate(20_000.0, 15_000.0);
    o.fill = Some(Fill::solid(color));
    p.layers[0].objects.push(o);
    d.data.pages.push(p);
    d
}

fn case(root: &std::path::Path, name: &str, original: &Document, reference: &Document) {
    let dir = root.join("casos").join(name);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("original.gdr"),
        gdr::write_bytes(original, &Default::default()).unwrap(),
    )
    .unwrap();
    let opts = ExportOptions {
        text_to_curves: true,
        ..Default::default()
    };
    std::fs::write(
        dir.join("ref-curvas.pdf"),
        export::pdf_document(reference, &opts).unwrap(),
    )
    .unwrap();
    std::fs::write(
        dir.join("meta.json"),
        r#"{"categoria":"sintético","versao_cdr":"—"}"#,
    )
    .unwrap();
}

#[test]
fn gd_fid_30_synthetic_corpus() {
    if fidelity::poppler_tool("pdftoppm").is_none() {
        eprintln!("pdftoppm ausente: teste ignorado");
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    let black = doc(Color::black());
    case(tmp.path(), "0001", &black, &black);
    // Referência com o retângulo vermelho: o candidato (preto) falha.
    case(
        tmp.path(),
        "0002",
        &black,
        &doc(Color::from_hex("#ff0000").unwrap()),
    );
    let (r, out) = fidelity::run(tmp.path(), 72, "2026-01-01-0000").unwrap();
    assert_eq!(r.cases.len(), 2);
    assert!(
        r.cases
            .iter()
            .all(|c| c.opened && c.ref_pages == 1 && c.cand_pages == 1)
    );
    assert_eq!(r.cases[0].pages[0].level, Level::Fiel, "{:?}", r.cases[0]);
    assert_eq!(r.cases[1].pages[0].level, Level::Falha, "{:?}", r.cases[1]);
    assert_eq!((r.summary.fiel, r.summary.falha), (1, 1));
    assert!(
        out.join("relatorio.html").is_file()
            && out.join("diffs").join("0002-p1.diff.png").is_file()
    );
    // Segunda execução: tendência contra a anterior.
    let (r2, _) = fidelity::run(tmp.path(), 72, "2026-01-02-0000").unwrap();
    assert_eq!(r2.summary.trend_fiel_pp, Some(0.0));
}

fn text_doc(family: &str) -> Document {
    use graphdraw_core::edit::text::{insert, new_text};
    let mut d = Document::new();
    let mut p = Page::new(100_000.0, 60_000.0);
    let mut o = new_text(10_000.0, 40_000.0, None, family, 8_000.0);
    if let ObjectKind::Text(t) = &mut o.kind {
        insert(t, 0, "Fidelidade");
    }
    p.layers[0].objects.push(o);
    d.data.pages.push(p);
    d
}

/// GD-FID-13: com `ref-texto.pdf`, o texto extraído é comparado e as
/// fontes da referência que o candidato não usa são listadas.
#[test]
fn gd_fid_13_text_and_substituted_fonts() {
    use graphdraw_core::render::text::resolve_family;
    if fidelity::poppler_tool("pdffonts").is_none()
        || resolve_family("Arial", 400, false).is_none()
        || resolve_family("Times New Roman", 400, false).is_none()
    {
        eprintln!("Poppler ou fontes ausentes: teste ignorado");
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    let ours = text_doc("Arial");
    case(tmp.path(), "0001", &ours, &ours);
    let dir = tmp.path().join("casos").join("0001");
    // A referência "com texto" usou outra fonte.
    let live =
        export::pdf_document(&text_doc("Times New Roman"), &ExportOptions::default()).unwrap();
    std::fs::write(dir.join("ref-texto.pdf"), live).unwrap();
    let (r, _) = fidelity::run(tmp.path(), 72, "2026-01-01-0000").unwrap();
    let c = &r.cases[0];
    assert_eq!(c.text_equal, Some(true));
    assert!(
        c.substituted_fonts.iter().any(|f| f.contains("Times")),
        "{:?} / {:?}",
        c.ref_fonts,
        c.cand_fonts
    );
    assert!(c.cand_fonts.iter().any(|f| f.contains("Arial")));
    assert_eq!(r.summary.by_category["sintético"].pages, 1);
}

/// GD-FID-12: com `ref-cmyk.pdf`, os valores CMYK das cores sólidas são
/// comparados com o PDF/X-1a do candidato.
#[test]
fn gd_fid_12_cmyk_values() {
    use graphdraw_core::export::PdfStandard;
    if fidelity::poppler_tool("pdftoppm").is_none() {
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    let red = doc(Color::cmyk(0.0, 1.0, 1.0, 0.0));
    let x1a = ExportOptions {
        text_to_curves: true,
        pdf: PdfStandard::X1a(Default::default()),
        ..Default::default()
    };
    for (name, reference) in [
        ("0001", &red),
        ("0002", &doc(Color::cmyk(0.0, 0.9, 1.0, 0.0))),
    ] {
        case(tmp.path(), name, &red, &red);
        let dir = tmp.path().join("casos").join(name);
        std::fs::write(
            dir.join("ref-cmyk.pdf"),
            export::pdf_document(reference, &x1a).unwrap(),
        )
        .unwrap();
    }
    let (r, _) = fidelity::run(tmp.path(), 72, "2026-01-01-0000").unwrap();
    let pp = |i: usize| r.cases[i].pages[0].cmyk_pp.unwrap();
    assert!(pp(0) < 1e-6, "{}", pp(0));
    assert!((pp(1) - 10.0).abs() < 0.5, "{}", pp(1));
    assert_eq!(r.cases[0].pages[0].level, Level::Fiel);
    assert_eq!(r.cases[1].pages[0].level, Level::Falha);
}
