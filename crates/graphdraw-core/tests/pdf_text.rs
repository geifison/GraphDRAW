//! GD-EXP-02: texto no PDF sai vivo, com a fonte incorporada em
//! subconjunto (Type0/CID) e ToUnicode para copiar e buscar; com a opção
//! "texto em curvas", não há fonte no arquivo.

use graphdraw_core::edit::text::{insert, new_text};
use graphdraw_core::export::{self, ExportOptions, PdfStandard, PrintCondition};
use graphdraw_core::model::{Document, ObjectKind, Page};
use graphdraw_core::render::text::resolve_family;
use lopdf::{Document as Pdf, Object as O};

fn doc_with_text(s: &str) -> Option<Document> {
    // Sem fonte no sistema não há o que incorporar.
    resolve_family("Arial", 400, false)?;
    let mut d = Document::new();
    let mut p = Page::new(210_000.0, 297_000.0);
    let mut o = new_text(20_000.0, 250_000.0, None, "Arial", 10_000.0);
    if let ObjectKind::Text(t) = &mut o.kind {
        insert(t, 0, s);
    }
    p.layers[0].objects.push(o);
    d.data.pages.push(p);
    Some(d)
}

fn fonts_of(pdf: &Pdf) -> Vec<lopdf::Dictionary> {
    pdf.objects
        .values()
        .filter_map(|o| o.as_dict().ok())
        .filter(|d| d.get(b"Type").and_then(|t| t.as_name()).ok() == Some(b"Font".as_slice()))
        .cloned()
        .collect()
}

#[test]
fn gd_exp_02_live_text_with_embedded_subset_font() {
    let Some(doc) = doc_with_text("Olá, GraphDraw") else {
        return;
    };
    let bytes = export::pdf_document(&doc, &ExportOptions::default()).unwrap();
    // Para conferir com leitores externos: GRAPHDRAW_DUMP_PDF=arquivo.pdf.
    if let Ok(path) = std::env::var("GRAPHDRAW_DUMP_PDF") {
        std::fs::write(path, &bytes).unwrap();
    }
    let pdf = Pdf::load_mem(&bytes).unwrap();
    let fonts = fonts_of(&pdf);
    let type0 = fonts
        .iter()
        .find(|d| d.get(b"Subtype").and_then(|s| s.as_name()).ok() == Some(b"Type0".as_slice()))
        .expect("fonte Type0");
    assert_eq!(
        type0.get(b"Encoding").unwrap().as_name().unwrap(),
        b"Identity-H"
    );
    let base =
        String::from_utf8_lossy(type0.get(b"BaseFont").unwrap().as_name().unwrap()).to_string();
    assert!(
        base.len() > 7 && &base[6..7] == "+",
        "subconjunto com etiqueta: {base}"
    );
    assert!(type0.get(b"ToUnicode").is_ok());
    // Descritor com o arquivo da fonte.
    let has_file = pdf
        .objects
        .values()
        .filter_map(|o| o.as_dict().ok())
        .any(|d| {
            d.get(b"Type").and_then(|t| t.as_name()).ok() == Some(b"FontDescriptor".as_slice())
                && (d.get(b"FontFile2").is_ok() || d.get(b"FontFile3").is_ok())
        });
    assert!(has_file, "fonte incorporada");
    // O texto sai como operadores de texto (Tj), não só curvas.
    let content = pdf.get_page_content(*pdf.get_pages().values().next().unwrap());
    let ops = lopdf::content::Content::decode(&content)
        .unwrap()
        .operations;
    assert!(ops.iter().any(|o| o.operator == "Tj"));
    assert!(ops.iter().any(|o| o.operator == "Tf"));
    // Subconjunto: bem menor que a fonte inteira.
    assert!(bytes.len() < 200_000, "{} bytes", bytes.len());
}

#[test]
fn gd_exp_02_text_as_curves_has_no_font() {
    let Some(doc) = doc_with_text("Curvas") else {
        return;
    };
    let opts = ExportOptions {
        text_to_curves: true,
        ..Default::default()
    };
    let pdf = Pdf::load_mem(&export::pdf_document(&doc, &opts).unwrap()).unwrap();
    assert!(fonts_of(&pdf).is_empty());
}

#[test]
fn pdfx_keeps_truetype_text_live() {
    let Some(doc) = doc_with_text("PDF/X") else {
        return;
    };
    let opts = ExportOptions {
        pdf: PdfStandard::X1a(PrintCondition::Fogra39),
        ..Default::default()
    };
    let pdf = Pdf::load_mem(&export::pdf_document(&doc, &opts).unwrap()).unwrap();
    // Arial é TrueType: sai viva também no PDF/X-1a (FontFile2, PDF 1.3).
    let fonts = fonts_of(&pdf);
    assert!(!fonts.is_empty());
    let any_ff3 = pdf
        .objects
        .values()
        .filter_map(|o| o.as_dict().ok())
        .any(|d| d.get(b"FontFile3").is_ok());
    assert!(!any_ff3, "PDF 1.3 não aceita OpenType/CFF");
    let _ = O::Null;
}
