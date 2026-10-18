//! Robustez (conselho de 2026-10-17, item f; GD-GDR-10, GD-CDR-06): abrir
//! arquivo corrompido nunca entra em pânico, e a ida e volta do `.gdr` com
//! os recursos novos é estável.

mod common;

use common::*;
use graphdraw_core::open_bytes;

/// Gerador determinístico (sem dependência de aleatoriedade).
struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }
}

fn no_panic(bytes: &[u8], ext: Option<&str>, what: &str) {
    let r = std::panic::catch_unwind(|| {
        let _ = open_bytes(bytes, ext);
    });
    assert!(r.is_ok(), "pânico ao abrir {what}");
}

/// GD-GDR-10: `.gdr` com bytes trocados ou cortado abre o que der ou dá
/// erro, sem pânico.
#[test]
fn corrupted_gdr_never_panics() {
    let mut rng = Lcg(42);
    for name in VALID_GDR {
        let orig = std::fs::read(gdr_fixture(name)).unwrap();
        for k in 0..40 {
            let mut b = orig.clone();
            // Algumas trocas de byte em lugares ao acaso.
            for _ in 0..(1 + k % 8) {
                let i = (rng.next() as usize) % b.len();
                b[i] = rng.next() as u8;
            }
            no_panic(&b, Some("gdr"), &format!("{name} #{k}"));
        }
        for cut in [0, 1, 10, orig.len() / 3, orig.len() / 2, orig.len() - 1] {
            no_panic(
                &orig[..cut],
                Some("gdr"),
                &format!("{name} cortado em {cut}"),
            );
        }
    }
    // Lixo puro, com e sem extensão.
    for n in [0usize, 1, 4, 100, 5000] {
        let junk: Vec<u8> = (0..n).map(|_| rng.next() as u8).collect();
        no_panic(&junk, None, "lixo");
        no_panic(&junk, Some("gdr"), "lixo .gdr");
    }
}

/// `document.json` válido como ZIP mas com JSON estranho por dentro.
#[test]
fn hostile_document_json_never_panics() {
    use std::io::Write;
    let docs = [
        "{}",
        "[]",
        "null",
        r#"{"formatVersion":"1.0","pages":"x"}"#,
        r#"{"formatVersion":"1.0","pages":[{"width":-1,"height":1e308,"layers":[{"objects":[{"type":"path"}]}]}]}"#,
        r#"{"formatVersion":"1.0","pages":[{"width":100,"height":100,"layers":[{"objects":[{"id":"a","type":"text","paragraphs":[{"spans":[{"text":"x","font":{"family":"","size":0}}]}],"path":{"subpaths":[]}}]}]}]}"#,
        r#"{"formatVersion":"1.0","pages":[{"width":100,"height":100,"layers":[{"objects":[{"id":"b","type":"rectangle","width":1e300,"height":-5,"fill":{"type":"gradient","kind":"conic","stops":[]},"stroke":{"width":-1,"color":{"model":"rgb","values":[]},"endArrow":{"shape":"?","scale":-3}},"blendMode":"nope"}]}]}]}"#,
        r#"{"formatVersion":"99.0","pages":[]}"#,
    ];
    for (k, d) in docs.iter().enumerate() {
        let mut buf = Vec::new();
        {
            let mut z = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
            let stored = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            z.start_file("mimetype", stored).unwrap();
            z.write_all(b"application/vnd.graphdraw.document").unwrap();
            z.start_file("document.json", stored).unwrap();
            z.write_all(d.as_bytes()).unwrap();
            z.finish().unwrap();
        }
        no_panic(&buf, Some("gdr"), &format!("document.json #{k}"));
        // O que abrir também tem que exportar sem pânico.
        if let Ok((doc, _)) = open_bytes(&buf, Some("gdr")) {
            let r = std::panic::catch_unwind(|| {
                for i in 0..doc.data.pages.len() {
                    let _ = graphdraw_core::export::svg_page(&doc, i, &Default::default());
                    let _ = graphdraw_core::export::png_page(
                        &doc,
                        i,
                        &graphdraw_core::export::ExportOptions {
                            dpi: 10.0,
                            ..Default::default()
                        },
                    );
                }
                let _ = graphdraw_core::export::pdf_document(&doc, &Default::default());
            });
            assert!(r.is_ok(), "pânico ao exportar document.json #{k}");
        }
    }
}

/// GD-CDR-06: `.cdr` real cortado em vários pontos não derruba o programa.
#[test]
fn truncated_cdr_never_panics() {
    if !bridge_available() {
        return;
    }
    let orig = std::fs::read(cdr_fixture("fdo65220-2.cdr")).unwrap();
    let mut rng = Lcg(7);
    for k in 0..12 {
        let cut = 16 + (rng.next() as usize) % (orig.len() - 16);
        no_panic(&orig[..cut], Some("cdr"), &format!("cortado em {cut}"));
        let mut b = orig.clone();
        for _ in 0..4 {
            let i = (rng.next() as usize) % b.len();
            b[i] = rng.next() as u8;
        }
        no_panic(&b, Some("cdr"), &format!("bytes trocados #{k}"));
    }
}

/// Ida e volta do `.gdr` com todos os campos novos (setas, caneta, mesclagem,
/// texto em caminho, gradientes centrados, dados de usuário): dados iguais e
/// `document.json` idêntico na segunda gravação; e válido pelo schema.
#[test]
fn gdr_round_trip_with_new_fields() {
    use graphdraw_core::model::*;
    let mut d = Document::new();
    let mut p = Page::new(100_000.0, 100_000.0);
    let mut path = Object::new(ObjectKind::Path(PathData {
        subpaths: vec![Subpath {
            closed: false,
            nodes: vec![Node::corner(1000.0, 1000.0), Node::corner(9000.0, 5000.0)],
            extra: Default::default(),
        }],
        fill_rule: Default::default(),
        extra: Default::default(),
    }));
    let mut st = Stroke::new(500.0, Color::cmyk(0.0, 1.0, 1.0, 0.0));
    let mut a = Arrow::new("barbed");
    a.scale = 1.5;
    a.mirror = true;
    st.end_arrow = Some(a);
    st.nib_stretch = 0.3;
    st.nib_angle = 30.0;
    path.stroke = Some(st);
    path.blend = BlendMode::ColorBurn;
    path.opacity = Some(0.5);
    let mut ud = std::collections::BTreeMap::new();
    ud.insert("código".to_string(), "OP-7".to_string());
    path.user_data = Some(ud);
    let mut rect = Object::new(ObjectKind::Rectangle(RectData {
        width: 20_000.0,
        height: 10_000.0,
        corners: None,
        extra: Default::default(),
    }));
    rect.fill = Some(Fill::Gradient(Gradient {
        kind: "conic".into(),
        stops: vec![
            GradientStop {
                offset: 0.0,
                color: Color::black(),
                opacity: 1.0,
                midpoint: 0.5,
                extra: Default::default(),
            },
            GradientStop {
                offset: 1.0,
                color: Color::rgb(1.0, 0.5, 0.0),
                opacity: 0.8,
                midpoint: 0.5,
                extra: Default::default(),
            },
        ],
        start: None,
        end: None,
        center: Some([10_000.0, 5_000.0]),
        radius: Some(10_000.0),
        radius_y: Some(5_000.0),
        angle: Some(45.0),
        spread: "reflect".into(),
        steps: None,
        smooth: false,
        color_path: "direct".into(),
        extra: Default::default(),
    }));
    let mut text = graphdraw_core::edit::text::new_text(0.0, 50_000.0, None, "Arial", 5_000.0);
    if let ObjectKind::Text(t) = &mut text.kind {
        graphdraw_core::edit::text::insert(t, 0, "curvo");
        t.path = Some(TextPath {
            subpaths: vec![Subpath {
                closed: false,
                nodes: vec![Node::corner(0.0, 0.0), Node::corner(50_000.0, 10_000.0)],
                extra: Default::default(),
            }],
            offset: 1000.0,
            distance: 200.0,
            side: "below".into(),
            orientation: "skew-v".into(),
            mirror: true,
            guide: None,
            detached: false,
            extra: Default::default(),
        });
    }
    p.layers[0].objects.extend([path, rect, text]);
    d.data.pages.push(p);
    let b1 = graphdraw_core::gdr::write_bytes(&d, &Default::default()).unwrap();
    let json: serde_json::Value =
        serde_json::from_slice(&member(&b1, "document.json").unwrap()).unwrap();
    assert_valid(&schema_validator(), &json, "campos novos");
    let (back, report) = graphdraw_core::gdr::read_bytes(&b1).unwrap();
    assert!(report.items.is_empty(), "{:?}", report.items);
    assert_eq!(back.data, d.data);
    let b2 = graphdraw_core::gdr::write_bytes(&back, &Default::default()).unwrap();
    assert_eq!(member(&b1, "document.json"), member(&b2, "document.json"));
}
