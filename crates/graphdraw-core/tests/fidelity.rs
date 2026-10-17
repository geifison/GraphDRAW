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
