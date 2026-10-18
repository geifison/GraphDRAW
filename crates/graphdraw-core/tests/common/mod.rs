#![allow(dead_code)]

use std::io::{Cursor, Read};
use std::path::PathBuf;

pub fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

pub fn gdr_fixture(name: &str) -> PathBuf {
    repo().join("tests/fixtures/gdr").join(name)
}

pub fn cdr_fixture(name: &str) -> PathBuf {
    repo().join("tests/fixtures/cdr").join(name)
}

/// Fixtures `.gdr` válidas (abrem sem erro).
pub const VALID_GDR: &[&str] = &[
    "basic-shapes.gdr",
    "colors.gdr",
    "gradients.gdr",
    "text.gdr",
    "image.gdr",
    "multipage.gdr",
    "layers-print.gdr",
    "unknown-fields.gdr",
    "split-pages.gdr",
    "prepress.gdr",
    "patterns.gdr",
    "connectors.gdr",
    "bspline.gdr",
    "symmetry.gdr",
    "variable-width.gdr",
    "text-wrap.gdr",
];

/// Lê um membro de um pacote ZIP.
pub fn member(bytes: &[u8], name: &str) -> Option<Vec<u8>> {
    let mut z = zip::ZipArchive::new(Cursor::new(bytes)).ok()?;
    let mut f = z.by_name(name).ok()?;
    let mut out = Vec::new();
    f.read_to_end(&mut out).ok()?;
    Some(out)
}

pub fn member_names(bytes: &[u8]) -> Vec<String> {
    let z = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    z.file_names()
        .filter_map(|n| n.ok().map(|s| s.to_string()))
        .collect()
}

/// A ponte da libcdr está disponível? Sem ela, os testes de `.cdr` são
/// pulados — exceto quando `GRAPHDRAW_REQUIRE_BRIDGE` está definida (CI).
pub fn bridge_available() -> bool {
    if graphdraw_core::cdr::bridge::locate().is_some() {
        return true;
    }
    if std::env::var_os("GRAPHDRAW_REQUIRE_BRIDGE").is_some() {
        panic!("gd-cdr-bridge não encontrado e GRAPHDRAW_REQUIRE_BRIDGE está definida");
    }
    eprintln!(
        "AVISO: gd-cdr-bridge não encontrado; teste de .cdr pulado (compile com native/cdr-bridge/build.sh)"
    );
    false
}

pub fn schema_validator() -> jsonschema::Validator {
    let text = std::fs::read_to_string(repo().join("docs/format/gdr-1.0.schema.json")).unwrap();
    let schema: serde_json::Value = serde_json::from_str(&text).unwrap();
    jsonschema::validator_for(&schema).expect("schema JSON inválido")
}

pub fn assert_valid(v: &jsonschema::Validator, doc: &serde_json::Value, what: &str) {
    let errors: Vec<String> = v
        .iter_errors(doc)
        .map(|e| format!("{} em {}", e, e.instance_path()))
        .collect();
    assert!(
        errors.is_empty(),
        "{what} não valida contra o schema:\n{}",
        errors.join("\n")
    );
}

/// Pixel RGBA de um PNG.
pub fn png_pixel(png_bytes: &[u8], x: u32, y: u32) -> [u8; 4] {
    let img = image::load_from_memory(png_bytes).unwrap().to_rgba8();
    img.get_pixel(x, y).0
}

pub fn inflate_all_streams(pdf: &[u8]) -> String {
    // Extrai e descomprime todos os streams FlateDecode (busca simples por "stream\n").
    let mut out = String::new();
    let mut i = 0;
    while let Some(pos) = find(&pdf[i..], b"stream\n") {
        let start = i + pos + 7;
        let Some(end_rel) = find(&pdf[start..], b"endstream") else {
            break;
        };
        let data = &pdf[start..start + end_rel];
        if let Ok(dec) =
            miniz_oxide::inflate::decompress_to_vec_zlib(data.strip_suffix(b"\n").unwrap_or(data))
        {
            out.push_str(&String::from_utf8_lossy(&dec));
            out.push('\n');
        }
        i = start + end_rel + 9;
    }
    out
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

pub fn sha_hex(data: &[u8]) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(data)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
