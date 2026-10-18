//! Testes do formato `.gdr` (doc 13: GD-GDR-01..11).

mod common;

use common::*;
use graphdraw_core::gdr::{self, WriteOptions};
use graphdraw_core::model::{Document, Object, ObjectKind};
use graphdraw_core::{Error, open_path};
use serde_json::Value;

fn read(name: &str) -> Vec<u8> {
    std::fs::read(gdr_fixture(name)).unwrap()
}

fn document_json(bytes: &[u8]) -> Value {
    serde_json::from_slice(&member(bytes, "document.json").unwrap()).unwrap()
}

/// Documento com as páginas resolvidas (para comparar inline × dividido).
fn document_with_pages(bytes: &[u8]) -> Value {
    let mut v = document_json(bytes);
    if let Some(pages) = v["pages"].as_array_mut() {
        for p in pages.iter_mut() {
            if let Some(src) = p.get("src").and_then(Value::as_str).map(String::from) {
                *p = serde_json::from_slice(&member(bytes, &src).unwrap()).unwrap();
            }
        }
    }
    v
}

#[test]
fn gd_gdr_01_mimetype_first_and_stored() {
    for name in VALID_GDR {
        let bytes = read(name);
        assert!(
            gdr::sniff(&bytes),
            "{name}: detecção pelos bytes iniciais falhou"
        );
        let mut z = zip::ZipArchive::new(std::io::Cursor::new(&bytes)).unwrap();
        let first = z.by_index(0).unwrap();
        assert_eq!(first.name().unwrap(), "mimetype");
        assert_eq!(first.compression(), zip::CompressionMethod::Stored);
        drop(first);
        assert_eq!(
            member(&bytes, "mimetype").unwrap(),
            gdr::MIMETYPE.as_bytes()
        );
    }
}

#[test]
fn gd_gdr_02_package_structure_and_manifest() {
    for name in VALID_GDR {
        let bytes = read(name);
        let names = member_names(&bytes);
        for required in [
            "manifest.json",
            "document.json",
            "metadata.json",
            "resources/colors.json",
            "thumbnails/page-1.png",
        ] {
            assert!(
                names.iter().any(|n| n == required),
                "{name}: falta {required}"
            );
        }
        // Manifesto: versão, gerador, lista de membros com SHA-256 correto.
        let manifest: Value =
            serde_json::from_slice(&member(&bytes, "manifest.json").unwrap()).unwrap();
        assert_eq!(manifest["formatVersion"], "1.0");
        assert_eq!(manifest["generator"]["name"], "GraphDraw");
        let members = manifest["members"].as_array().unwrap();
        for n in names
            .iter()
            .filter(|n| *n != "mimetype" && *n != "manifest.json")
        {
            let entry = members
                .iter()
                .find(|m| m["path"] == n.as_str())
                .unwrap_or_else(|| panic!("{name}: {n} fora do manifesto"));
            let data = member(&bytes, n).unwrap();
            assert_eq!(entry["sha256"], sha_hex(&data), "{name}: hash de {n}");
            assert_eq!(entry["size"], data.len() as u64);
        }
        // Miniatura de 256 px (arquivos de antes) até THUMBNAIL_SIDE px.
        let thumb =
            image::load_from_memory(&member(&bytes, "thumbnails/page-1.png").unwrap()).unwrap();
        let side = thumb.width().max(thumb.height());
        assert!(
            (256..=graphdraw_core::gdr::THUMBNAIL_SIDE).contains(&side),
            "{name}: miniatura {}x{}",
            thumb.width(),
            thumb.height()
        );
        // Imagens deduplicadas por hash: nome = sha256 do conteúdo.
        for n in names.iter().filter(|n| n.starts_with("resources/images/")) {
            let data = member(&bytes, n).unwrap();
            assert!(
                n.contains(&sha_hex(&data)),
                "{name}: nome de {n} não é o hash"
            );
        }
    }
}

#[test]
fn gd_gdr_02_images_deduplicated() {
    let bytes = read("image.gdr");
    let images: Vec<_> = member_names(&bytes)
        .into_iter()
        .filter(|n| n.starts_with("resources/images/"))
        .collect();
    assert_eq!(
        images.len(),
        1,
        "duas instâncias da mesma imagem devem gerar um único membro"
    );
}

/// Aceite do doc 13: abrir → salvar → abrir produz document.json
/// semanticamente idêntico (ignorando ordem de chaves).
#[test]
fn gd_gdr_acceptance_roundtrip_identical() {
    for name in VALID_GDR {
        let original = read(name);
        let (doc, _) = gdr::read_bytes(&original).unwrap();
        let saved = gdr::write_bytes(&doc, &WriteOptions::default()).unwrap();
        let (doc2, _) = gdr::read_bytes(&saved).unwrap();
        assert_eq!(
            document_with_pages(&original),
            document_with_pages(&saved),
            "{name}: document.json mudou no round-trip"
        );
        assert_eq!(doc, doc2, "{name}: modelo mudou no round-trip");
        assert_eq!(
            member(&original, "metadata.json")
                .map(|b| serde_json::from_slice::<Value>(&b).unwrap()),
            member(&saved, "metadata.json").map(|b| serde_json::from_slice::<Value>(&b).unwrap())
        );
        assert_eq!(
            member(&original, "resources/colors.json")
                .map(|b| serde_json::from_slice::<Value>(&b).unwrap()),
            member(&saved, "resources/colors.json")
                .map(|b| serde_json::from_slice::<Value>(&b).unwrap())
        );
    }
}

#[test]
fn gd_gdr_03_unknown_fields_types_and_members_preserved() {
    let bytes = read("unknown-fields.gdr");
    let (doc, report) = gdr::read_bytes(&bytes).unwrap();
    assert!(report.has("gdr.object.unknown-type"));
    let saved = gdr::write_bytes(&doc, &WriteOptions::default()).unwrap();
    let v = document_json(&saved);
    assert_eq!(v["futureDocumentField"], serde_json::json!([1, 2, 3]));
    assert_eq!(v["pages"][0]["futurePageSetting"]["fold"], "z");
    let objs = &v["pages"][0]["layers"][0]["objects"];
    assert_eq!(objs[0]["futureBevel"]["depth"], 2500);
    assert_eq!(objs[1]["type"], "hologram");
    assert_eq!(objs[1]["beams"], 3);
    assert_eq!(
        member(&saved, "plugins/exemplo.json").unwrap(),
        br#"{"plugin":"exemplo"}"#
    );
}

#[test]
fn gd_gdr_03_future_major_version_rejected() {
    match open_path(&gdr_fixture("future-version.gdr")) {
        Err(Error::UnsupportedVersion { format, version }) => {
            assert_eq!(format, "gdr");
            assert_eq!(version, "2.0");
        }
        other => panic!("esperado UnsupportedVersion, veio {:?}", other.map(|_| ())),
    }
}

#[test]
fn gd_gdr_03_minor_version_accepted() {
    let bytes = read("colors.gdr");
    let mut v = document_json(&bytes);
    v["formatVersion"] = "1.7".into();
    let (doc, _) = gdr::read_bytes(&bytes).unwrap();
    let data: graphdraw_core::model::DocumentData = serde_json::from_value(v).unwrap();
    assert_eq!(data.pages.len(), doc.data.pages.len());
}

#[test]
fn gd_gdr_04_ids_are_uuid_v7_and_coordinates_in_um() {
    let o = Object::new(ObjectKind::Group(graphdraw_core::model::GroupData {
        children: vec![],
        boolean: None,
        symmetry: None,
        effect: None,
        symbol: None,
        extra: Default::default(),
    }));
    let id = uuid::Uuid::parse_str(&o.id).unwrap();
    assert_eq!(id.get_version_num(), 7);
    // Página A4 gravada em µm.
    let v = document_json(&read("basic-shapes.gdr"));
    assert_eq!(v["pages"][0]["width"], 210000.0);
    assert_eq!(v["pages"][0]["height"], 297000.0);
}

#[test]
fn gd_gdr_05_06_07_object_path_and_color_shapes() {
    let v = document_json(&read("basic-shapes.gdr"));
    let objs = v["pages"][0]["layers"][0]["objects"].as_array().unwrap();
    for o in objs {
        for k in ["id", "type", "transform", "visible", "locked"] {
            assert!(o.get(k).is_some(), "objeto sem '{k}': {o}");
        }
        assert_eq!(o["transform"].as_array().unwrap().len(), 6);
    }
    let path = objs.iter().find(|o| o["type"] == "path").unwrap();
    assert_eq!(path["fillRule"], "evenodd");
    let sp = &path["subpaths"][1];
    assert_eq!(sp["closed"], true);
    let node = &sp["nodes"][0];
    assert!(node["p"].is_array() && node["in"].is_array() && node["out"].is_array());
    assert_eq!(node["kind"], "smooth");

    let colors = document_json(&read("colors.gdr"));
    let fills: Vec<&Value> = colors["pages"][0]["layers"][0]["objects"]
        .as_array()
        .unwrap()
        .iter()
        .map(|o| &o["fill"]["color"])
        .collect();
    assert_eq!(fills[1]["model"], "cmyk");
    assert_eq!(fills[1]["values"], serde_json::json!([0.0, 1.0, 1.0, 0.0]));
    assert_eq!(fills[5]["model"], "spot");
    assert_eq!(fills[5]["name"], "Laranja Especial 021");
    assert_eq!(fills[5]["alternate"]["model"], "cmyk");
    assert_eq!(fills[8]["ref"], "cor-ciano");
}

#[test]
fn gd_gdr_08_fixtures_validate_against_schema() {
    let v = schema_validator();
    for name in VALID_GDR {
        let bytes = read(name);
        assert_valid(&v, &document_json(&bytes), name);
        // Páginas separadas (GD-GDR-11) também seguem o schema de página.
        assert_valid(
            &v,
            &document_with_pages(&bytes),
            &format!("{name} (páginas resolvidas)"),
        );
    }
}

#[test]
fn gd_gdr_08_schema_rejects_invalid_documents() {
    let v = schema_validator();
    let bad = serde_json::json!({ "formatVersion": "1.0", "pages": [ { "id": "p", "width": -5, "height": 10 } ] });
    assert!(!v.is_valid(&bad));
    let bad_color = serde_json::json!({ "formatVersion": "1.0", "pages": [ { "id": "p", "width": 5, "height": 10, "layers": [
        { "id": "l", "name": "x", "objects": [ { "id": "o", "type": "rectangle", "width": 1, "height": 1,
          "fill": { "type": "solid", "color": { "model": "cmyk", "values": [0, 1] } } } ] } ] } ] });
    assert!(!v.is_valid(&bad_color));
}

#[test]
fn gd_gdr_09_atomic_save_replaces_and_leaves_no_temp() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("saida.gdr");
    std::fs::write(&path, b"conteudo antigo").unwrap();
    let (doc, _) = open_path(&gdr_fixture("colors.gdr")).unwrap();
    gdr::write_file(&doc, &path, &WriteOptions::default()).unwrap();
    assert!(gdr::sniff(&std::fs::read(&path).unwrap()));
    let entries: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(entries.len(), 1, "sobrou arquivo temporário: {entries:?}");

    // Falha de gravação não deixa destino parcial.
    let bad = dir.path().join("nao-existe").join("x.gdr");
    assert!(gdr::write_file(&doc, &bad, &WriteOptions::default()).is_err());
    assert!(!bad.exists());
}

#[test]
fn gd_gdr_10_tolerant_read_of_corrupt_members() {
    let (doc, report) = open_path(&gdr_fixture("corrupt-member.gdr")).unwrap();
    assert_eq!(doc.data.pages.len(), 1);
    assert_eq!(
        doc.data.pages[0].layers[0].objects.len(),
        3,
        "objetos válidos e inválidos devem ser mantidos"
    );
    for code in [
        "gdr.member.checksum",
        "gdr.metadata.invalid",
        "gdr.object.invalid",
        "gdr.image.unreadable",
    ] {
        assert!(
            report.has(code),
            "relatório sem '{code}': {}",
            report.to_text()
        );
    }
    // O documento ainda renderiza e exporta (a imagem ilegível é omitida).
    let png = graphdraw_core::export::png_page(
        &doc,
        0,
        &graphdraw_core::export::ExportOptions {
            dpi: 20.0,
            ..Default::default()
        },
    );
    assert!(png.is_ok());
}

#[test]
fn gd_gdr_10_never_panics_on_garbage() {
    let good = read("basic-shapes.gdr");
    let cases: Vec<Vec<u8>> = vec![
        Vec::new(),
        b"PK\x03\x04 lixo".to_vec(),
        good[..good.len() / 2].to_vec(),
        good.iter()
            .enumerate()
            .map(|(i, b)| if i % 97 == 0 { b ^ 0x5a } else { *b })
            .collect(),
    ];
    for bytes in cases {
        let r = std::panic::catch_unwind(|| graphdraw_core::open_bytes(&bytes, Some("gdr")));
        assert!(r.is_ok(), "leitura entrou em pânico");
    }
    // Sem document.json: erro claro.
    let mut doc = Document::new();
    doc.data
        .pages
        .push(graphdraw_core::model::Page::new(1000.0, 1000.0));
    let bytes = gdr::write_bytes(
        &doc,
        &WriteOptions {
            thumbnail: false,
            ..Default::default()
        },
    )
    .unwrap();
    let names = member_names(&bytes);
    let mut z = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for n in names.iter().filter(|n| *n != "document.json") {
        z.start_file(
            n.as_str(),
            zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored),
        )
        .unwrap();
        std::io::Write::write_all(&mut z, &member(&bytes, n).unwrap()).unwrap();
    }
    let broken = z.finish().unwrap().into_inner();
    match gdr::read_bytes(&broken) {
        Err(Error::Corrupt(msg)) => assert!(msg.contains("document.json")),
        other => panic!("esperado erro Corrupt, veio {:?}", other.map(|_| ())),
    }
}

#[test]
fn gd_gdr_11_split_pages() {
    let bytes = read("split-pages.gdr");
    let v = document_json(&bytes);
    assert_eq!(v["pages"][0]["src"], "pages/page-1.json");
    assert!(member(&bytes, "pages/page-3.json").is_some());
    let (split, _) = gdr::read_bytes(&bytes).unwrap();
    let (inline, _) = open_path(&gdr_fixture("multipage.gdr")).unwrap();
    assert_eq!(split.data.pages, inline.data.pages);

    // Divisão automática acima do limite.
    let opts = WriteOptions {
        thumbnail: false,
        split_threshold: 100,
        ..Default::default()
    };
    let auto = gdr::write_bytes(&inline, &opts).unwrap();
    assert!(member(&auto, "pages/page-2.json").is_some());
    let small = gdr::write_bytes(
        &inline,
        &WriteOptions {
            thumbnail: false,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(member(&small, "pages/page-1.json").is_none());
}

#[test]
fn gd_doc_02_y_axis_up_origin_bottom_left() {
    // Retângulo vermelho em (10..40 mm, 10..40 mm) a partir do canto inferior esquerdo.
    let (doc, _) = open_path(&gdr_fixture("layers-print.gdr")).unwrap();
    let opts = graphdraw_core::export::ExportOptions {
        dpi: 25.4,
        ..Default::default()
    }; // 1 px = 1 mm
    let png = graphdraw_core::export::png_page(&doc, 0, &opts).unwrap();
    assert_eq!(
        png_pixel(&png, 25, 100 - 25),
        [255, 0, 0, 255],
        "vermelho deve estar embaixo à esquerda"
    );
    assert_eq!(
        png_pixel(&png, 25, 25)[3],
        0,
        "camada não imprimível no alto não deve sair"
    );
}

/// GD-CMS-02 e GD-COR-01: gerenciamento de cor do documento e cores HSB/HLS
/// gravam, seguem o schema e voltam iguais; o padrão não é gravado.
#[test]
fn gd_cms_settings_and_hsb_round_trip() {
    use graphdraw_core::model::{Color, Fill, Page, RectData};
    let mut doc = Document::new();
    doc.data.settings.color.cmyk_profile = Some("C:/perfis/Coated.icc".into());
    doc.data.settings.color.intent = "perceptual".into();
    let mut p = Page::new(100_000.0, 100_000.0);
    let mut o = Object::new(ObjectKind::Rectangle(RectData {
        width: 1_000.0,
        height: 1_000.0,
        corners: None,
        extra: Default::default(),
    }));
    o.fill = Some(Fill::solid(Color::new("hsb", vec![0.5, 0.8, 0.6])));
    let mut q = o.clone();
    q.id = graphdraw_core::model::new_id();
    q.fill = Some(Fill::solid(Color::new("hls", vec![0.1, 0.4, 0.9])));
    p.layers[0].objects.extend([o, q]);
    doc.data.pages.push(p);
    let saved = gdr::write_bytes(&doc, &WriteOptions::default()).unwrap();
    let json = document_json(&saved);
    assert_eq!(json["settings"]["color"]["intent"], "perceptual");
    assert_valid(&schema_validator(), &document_with_pages(&saved), "cor");
    let (back, _) = gdr::read_bytes(&saved).unwrap();
    assert_eq!(back.data.settings.color, doc.data.settings.color);
    // Padrão: não grava a chave.
    let plain = gdr::write_bytes(&Document::new(), &WriteOptions::default()).unwrap();
    assert!(document_json(&plain)["settings"].get("color").is_none());
}

/// GD-PRI-03/04: polígono grava, segue o schema e volta igual.
#[test]
fn gd_pri_03_polygon_round_trip() {
    use graphdraw_core::model::{Page, PolygonData};
    let mut doc = Document::new();
    let mut p = Page::new(100_000.0, 100_000.0);
    p.layers[0]
        .objects
        .push(Object::new(ObjectKind::Polygon(PolygonData {
            rx: 10_000.0,
            ry: 8_000.0,
            sides: 7,
            mode: "complex".into(),
            sharpness: 0.3,
            bend: None,
            corner_outer: 0.0,
            corner_inner: 0.0,
            fill_crossings: false,
            free_nodes: Vec::new(),
            extra: Default::default(),
        })));
    doc.data.pages.push(p);
    let saved = gdr::write_bytes(&doc, &WriteOptions::default()).unwrap();
    assert_valid(
        &schema_validator(),
        &document_with_pages(&saved),
        "polígono",
    );
    let (back, _) = gdr::read_bytes(&saved).unwrap();
    assert_eq!(
        back.data.pages[0].layers[0].objects[0].kind,
        doc.data.pages[0].layers[0].objects[0].kind
    );
}

/// GD-GUI-01: guias da página e do documento gravam e seguem o schema.
#[test]
fn gd_gui_01_guides_round_trip() {
    use graphdraw_core::model::{Guide, Page};
    let mut doc = Document::new();
    let mut p = Page::new(100_000.0, 100_000.0);
    p.guides.push(Guide::vertical(10_000.0));
    let mut a = Guide::angled(kurbo::Point::new(5_000.0, 5_000.0), 30.0);
    a.locked = true;
    p.guides.push(a);
    doc.data.pages.push(p);
    doc.data.settings.guides.push(Guide::horizontal(50_000.0));
    let saved = gdr::write_bytes(&doc, &WriteOptions::default()).unwrap();
    assert_valid(&schema_validator(), &document_with_pages(&saved), "guias");
    let (back, _) = gdr::read_bytes(&saved).unwrap();
    assert_eq!(back.data.pages[0].guides, doc.data.pages[0].guides);
    assert_eq!(back.data.settings.guides, doc.data.settings.guides);
}

/// GD-GDR-12: ao salvar, a miniatura sai com o lado maior de
/// THUMBNAIL_SIDE px (nítida nos ícones grandes do Explorador).
#[test]
fn gd_gdr_12_saved_thumbnail_size() {
    let mut d = graphdraw_core::model::Document::new();
    d.data
        .pages
        .push(graphdraw_core::model::Page::new(210_000.0, 297_000.0));
    let bytes = gdr::write_bytes(&d, &Default::default()).unwrap();
    let thumb = image::load_from_memory(&member(&bytes, "thumbnails/page-1.png").unwrap()).unwrap();
    assert_eq!(thumb.height(), gdr::THUMBNAIL_SIDE, "A4 em pé");
    assert!(thumb.width() < thumb.height());
}
