//! Formato nativo `.gdr` (doc 13): contêiner ZIP com JSON.
//!
//! - GD-GDR-01/02: estrutura do pacote, `mimetype` primeiro e sem compressão.
//! - GD-GDR-03: versão maior compatível; campos e membros desconhecidos preservados.
//! - GD-GDR-09: salvamento atômico (temporário no mesmo diretório + renomear).
//! - GD-GDR-10: leitura tolerante, com avisos no relatório.
//! - GD-GDR-11: páginas em `pages/page-<n>.json` (obrigatório acima de 20 MB).

use std::collections::BTreeMap;
use std::io::{Cursor, Read, Write};
use std::path::Path;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::Error;
use crate::model::document::hex;
use crate::model::{ColorsResource, Document, DocumentData, Metadata, ObjectKind, Page};
use crate::report::{ImportReport, Severity};

pub const MIMETYPE: &str = "application/vnd.graphdraw.document";
/// Limite acima do qual as páginas são gravadas em membros separados.
pub const SPLIT_THRESHOLD: usize = 20 * 1024 * 1024;

const DOCUMENT: &str = "document.json";
const MANIFEST: &str = "manifest.json";
const METADATA: &str = "metadata.json";
const COLORS: &str = "resources/colors.json";
const STYLES: &str = "resources/styles.json";

/// `manifest.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub format_version: String,
    pub generator: Generator,
    pub members: Vec<ManifestMember>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Generator {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestMember {
    pub path: String,
    pub sha256: String,
    pub size: u64,
}

/// Verdadeiro se os bytes iniciais identificam um `.gdr` (GD-GDR-01).
pub fn sniff(bytes: &[u8]) -> bool {
    // Cabeçalho local ZIP: nome começa no byte 30; dados logo após o nome
    // (sem campo extra).
    bytes.len() >= 38 + MIMETYPE.len()
        && bytes.starts_with(b"PK\x03\x04")
        && &bytes[30..38] == b"mimetype"
        && bytes[38..].starts_with(MIMETYPE.as_bytes())
}

fn major(version: &str) -> Option<u32> {
    version.split('.').next()?.parse().ok()
}

/// Lê um `.gdr` da memória.
pub fn read_bytes(bytes: &[u8]) -> Result<(Document, ImportReport), Error> {
    let mut report = ImportReport::new("gdr");
    let mut zip = ZipArchive::new(Cursor::new(bytes))
        .map_err(|e| Error::Corrupt(format!("pacote ZIP ilegível: {e}")))?;

    // Carrega todos os membros legíveis; membro corrompido vira aviso.
    let mut members: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for i in 0..zip.len() {
        let mut f = match zip.by_index(i) {
            Ok(f) => f,
            Err(e) => {
                report.warn(
                    "gdr.member.unreadable",
                    format!("membro {i} do pacote ilegível: {e}"),
                    None,
                );
                continue;
            }
        };
        if f.is_dir() {
            continue;
        }
        let name = f
            .name()
            .map(|n| n.to_string())
            .unwrap_or_else(|_| format!("#{i}"));
        let mut buf = Vec::new();
        match f.read_to_end(&mut buf) {
            Ok(_) => {
                members.insert(name, buf);
            }
            Err(e) => report.warn(
                "gdr.member.corrupt",
                format!("membro '{name}' corrompido e ignorado: {e}"),
                None,
            ),
        }
    }

    match members.get("mimetype") {
        Some(m) if m.as_slice() == MIMETYPE.as_bytes() => {}
        Some(_) => report.warn(
            "gdr.mimetype.mismatch",
            "membro 'mimetype' com conteúdo inesperado",
            None,
        ),
        None => report.warn("gdr.mimetype.missing", "membro 'mimetype' ausente", None),
    }

    match members
        .get(MANIFEST)
        .map(|b| serde_json::from_slice::<Manifest>(b))
    {
        Some(Ok(manifest)) => {
            for m in &manifest.members {
                match members.get(&m.path) {
                    Some(data) if hex(&Sha256::digest(data)) == m.sha256 => {}
                    Some(_) => report.warn(
                        "gdr.member.checksum",
                        format!("membro '{}' não confere com o SHA-256 do manifesto", m.path),
                        None,
                    ),
                    None => report.warn(
                        "gdr.member.missing",
                        format!("membro '{}' listado no manifesto está ausente", m.path),
                        None,
                    ),
                }
            }
        }
        Some(Err(e)) => report.warn(
            "gdr.manifest.invalid",
            format!("manifest.json inválido: {e}"),
            None,
        ),
        None => report.warn("gdr.manifest.missing", "manifest.json ausente", None),
    }

    let doc_bytes = members
        .get(DOCUMENT)
        .ok_or_else(|| Error::Corrupt("document.json ausente".into()))?;
    let mut doc_value: Value = serde_json::from_slice(doc_bytes)
        .map_err(|e| Error::Corrupt(format!("document.json inválido: {e}")))?;
    let version = doc_value
        .get("formatVersion")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    match major(&version) {
        Some(1) => {}
        _ => {
            return Err(Error::UnsupportedVersion {
                format: "gdr".into(),
                version,
            });
        }
    }
    report.detected_version = Some(version);

    // Páginas: inline ou em membros separados (GD-GDR-11); cada página é
    // lida isoladamente para que uma página ruim não derrube o documento.
    let raw_pages = doc_value
        .get_mut("pages")
        .map(Value::take)
        .unwrap_or(Value::Array(vec![]));
    let mut pages = Vec::new();
    for (i, raw) in raw_pages
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .enumerate()
    {
        let raw = match raw.get("src").and_then(Value::as_str) {
            Some(src) => match members.get(src).map(|b| serde_json::from_slice::<Value>(b)) {
                Some(Ok(v)) => v,
                _ => {
                    report.warn(
                        "gdr.page.missing",
                        format!("página {} ('{src}') ausente ou inválida", i + 1),
                        Some(i + 1),
                    );
                    continue;
                }
            },
            None => raw,
        };
        match serde_json::from_value::<Page>(raw) {
            Ok(p) => pages.push(p),
            Err(e) => report.warn(
                "gdr.page.invalid",
                format!("página {} inválida e ignorada: {e}", i + 1),
                Some(i + 1),
            ),
        }
    }
    if let Value::Object(m) = &mut doc_value {
        m.insert("pages".into(), Value::Array(vec![]));
    }
    let mut data: DocumentData = match serde_json::from_value(doc_value) {
        Ok(d) => d,
        Err(e) => return Err(Error::Corrupt(format!("document.json inválido: {e}"))),
    };
    data.pages = pages;

    let metadata = match members
        .get(METADATA)
        .map(|b| serde_json::from_slice::<Metadata>(b))
    {
        Some(Ok(m)) => m,
        Some(Err(e)) => {
            report.warn(
                "gdr.metadata.invalid",
                format!("metadata.json inválido: {e}"),
                None,
            );
            Metadata::default()
        }
        None => Metadata::default(),
    };
    let colors = match members
        .get(COLORS)
        .map(|b| serde_json::from_slice::<ColorsResource>(b))
    {
        Some(Ok(c)) => c,
        Some(Err(e)) => {
            report.warn(
                "gdr.colors.invalid",
                format!("resources/colors.json inválido: {e}"),
                None,
            );
            ColorsResource::default()
        }
        None => ColorsResource::default(),
    };

    let mut doc = Document::new();
    doc.data = data;
    doc.metadata = metadata;
    doc.colors = colors;
    match members.get(STYLES).map(|b| serde_json::from_slice(b)) {
        Some(Ok(s)) => doc.styles = s,
        Some(Err(e)) => report.warn(
            "gdr.styles.invalid",
            format!("resources/styles.json inválido: {e}"),
            None,
        ),
        None => {}
    }
    for (name, bytes) in members {
        if name.starts_with("resources/images/") {
            if image::guess_format(&bytes).is_err() {
                report.warn(
                    "gdr.image.unreadable",
                    format!("imagem '{name}' ilegível; não será exibida"),
                    None,
                );
            }
            doc.images.insert(name, Arc::new(bytes));
        } else if !is_managed_member(&name) {
            // Fonte incorporada (GD-FNT-03): passa a valer para o texto.
            if name.starts_with("resources/fonts/") {
                crate::render::text::register_font(&name, bytes.clone());
            }
            doc.extra_members.insert(name, Arc::new(bytes));
        }
    }

    check_objects(&doc, &mut report);
    // Áreas do texto contornando objetos (derivadas, GD-TXT-10).
    crate::edit::textwrap::refresh_doc(&mut doc);
    Ok((doc, report))
}

/// Fontes usadas nos textos, inteiras, que a licença deixa incorporar
/// (GD-FNT-03): `resources/fonts/<sha256>.<ext>` → bytes.
pub fn embedded_fonts(doc: &Document) -> Vec<(String, Vec<u8>)> {
    use sha2::Digest;
    let mut want = std::collections::BTreeSet::new();
    for page in &doc.data.pages {
        for layer in &page.layers {
            for obj in &layer.objects {
                obj.walk(&mut |o| {
                    if let ObjectKind::Text(t) = &o.kind {
                        for p in &t.paragraphs {
                            for s in &p.spans {
                                want.insert((s.font.family.clone(), s.font.weight, s.font.italic));
                            }
                        }
                    }
                });
            }
        }
    }
    let mut out: Vec<(String, Vec<u8>)> = Vec::new();
    for (family, weight, italic) in want {
        if let Some((bytes, ext, ok)) = crate::render::text::font_file(&family, weight, italic)
            && ok
        {
            let hash = sha2::Sha256::digest(&bytes);
            let hex: String = hash.iter().map(|b| format!("{b:02x}")).collect();
            let name = format!("resources/fonts/{hex}.{ext}");
            if !out.iter().any(|(n, _)| *n == name) {
                out.push((name, bytes));
            }
        }
    }
    out
}

/// Membros que o gravador regenera (não vão para `extra_members`).
fn is_managed_member(name: &str) -> bool {
    matches!(
        name,
        "mimetype" | DOCUMENT | MANIFEST | METADATA | COLORS | STYLES
    ) || name.starts_with("thumbnails/")
        || name.starts_with("pages/")
}

fn check_objects(doc: &Document, report: &mut ImportReport) {
    let mut items = Vec::new();
    for (pi, page) in doc.data.pages.iter().enumerate() {
        for layer in &page.layers {
            for obj in &layer.objects {
                obj.walk(&mut |o| match &o.kind {
                    ObjectKind::Unknown { error: Some(e), .. } => items.push((
                        Severity::Warning,
                        "gdr.object.invalid",
                        format!("objeto inválido preservado sem exibir: {e}"),
                        pi + 1,
                        o.id.clone(),
                    )),
                    ObjectKind::Unknown { error: None, .. } => items.push((
                        Severity::Warning,
                        "gdr.object.unknown-type",
                        format!(
                            "tipo de objeto '{}' desconhecido nesta versão; preservado sem exibir",
                            o.type_name()
                        ),
                        pi + 1,
                        o.id.clone(),
                    )),
                    ObjectKind::Image(img) if !doc.images.contains_key(&img.src) => items.push((
                        Severity::Warning,
                        "gdr.image.missing",
                        format!("imagem '{}' ausente no pacote", img.src),
                        pi + 1,
                        o.id.clone(),
                    )),
                    _ => {}
                });
            }
        }
    }
    for (sev, code, msg, page, id) in items {
        report.push(sev, code, msg, Some(page), Some(id));
    }
}

/// Lê um `.gdr` do disco.
pub fn read_file(path: &Path) -> Result<(Document, ImportReport), Error> {
    let bytes = std::fs::read(path)?;
    read_bytes(&bytes)
}

/// Opções de gravação.
#[derive(Debug, Clone)]
pub struct WriteOptions {
    /// Gera `thumbnails/page-1.png` (lado maior de [`THUMBNAIL_SIDE`] px).
    pub thumbnail: bool,
    /// Força a divisão das páginas em membros separados.
    pub split_pages: bool,
    /// Tamanho de `document.json` acima do qual as páginas são divididas.
    pub split_threshold: usize,
}

impl Default for WriteOptions {
    fn default() -> Self {
        WriteOptions {
            thumbnail: true,
            split_pages: false,
            split_threshold: SPLIT_THRESHOLD,
        }
    }
}

/// Serializa o documento como `.gdr` em memória.
/// Lado maior da miniatura guardada no arquivo (px): nítida nos ícones
/// extragrandes do Explorador também em telas com zoom (GD-GDR-12).
pub const THUMBNAIL_SIDE: u32 = 512;

pub fn write_bytes(doc: &Document, opts: &WriteOptions) -> Result<Vec<u8>, Error> {
    let mut entries: Vec<(String, Vec<u8>, CompressionMethod)> = Vec::new();

    let mut data = doc.data.clone();
    data.format_version = crate::model::document::FORMAT_VERSION.into();
    let mut document_json = to_json(&data)?;
    if opts.split_pages || document_json.len() > opts.split_threshold {
        let pages = std::mem::take(&mut data.pages);
        let mut doc_value = serde_json::to_value(&data)?;
        let mut refs = Vec::new();
        for (i, page) in pages.iter().enumerate() {
            let name = format!("pages/page-{}.json", i + 1);
            entries.push((
                name.clone(),
                serde_json::to_vec(page)?,
                CompressionMethod::Deflated,
            ));
            refs.push(serde_json::json!({ "src": name }));
        }
        doc_value["pages"] = Value::Array(refs);
        document_json = serde_json::to_vec(&doc_value)?;
    }
    entries.push((DOCUMENT.into(), document_json, CompressionMethod::Deflated));
    entries.push((
        METADATA.into(),
        to_json(&doc.metadata)?,
        CompressionMethod::Deflated,
    ));
    entries.push((
        COLORS.into(),
        to_json(&doc.colors)?,
        CompressionMethod::Deflated,
    ));
    if !doc.styles.is_empty() {
        entries.push((
            STYLES.into(),
            to_json(&doc.styles)?,
            CompressionMethod::Deflated,
        ));
    }
    for (name, bytes) in &doc.images {
        entries.push((name.clone(), bytes.to_vec(), CompressionMethod::Stored));
    }
    for (name, bytes) in &doc.extra_members {
        entries.push((name.clone(), bytes.to_vec(), CompressionMethod::Deflated));
    }
    if doc.data.settings.embed_fonts {
        for (name, bytes) in embedded_fonts(doc) {
            if !doc.extra_members.contains_key(&name) {
                entries.push((name, bytes, CompressionMethod::Deflated));
            }
        }
    }
    if opts.thumbnail
        && !doc.data.pages.is_empty()
        && let Some(png) = crate::render::thumbnail_png(doc, 0, THUMBNAIL_SIDE)
    {
        entries.push((
            "thumbnails/page-1.png".into(),
            png,
            CompressionMethod::Stored,
        ));
    }

    let manifest = Manifest {
        format_version: crate::model::document::FORMAT_VERSION.into(),
        generator: Generator {
            name: "GraphDraw".into(),
            version: env!("CARGO_PKG_VERSION").into(),
        },
        members: entries
            .iter()
            .map(|(name, bytes, _)| ManifestMember {
                path: name.clone(),
                sha256: hex(&Sha256::digest(bytes)),
                size: bytes.len() as u64,
            })
            .collect(),
        extra: Map::new(),
    };

    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    zip.start_file("mimetype", stored)?;
    zip.write_all(MIMETYPE.as_bytes())?;
    zip.start_file(
        MANIFEST,
        SimpleFileOptions::default().compression_method(CompressionMethod::Deflated),
    )?;
    zip.write_all(&serde_json::to_vec_pretty(&manifest)?)?;
    for (name, bytes, method) in entries {
        zip.start_file(
            name,
            SimpleFileOptions::default().compression_method(method),
        )?;
        zip.write_all(&bytes)?;
    }
    Ok(zip.finish()?.into_inner())
}

/// Grava de forma atômica (GD-GDR-09): escreve num temporário no mesmo
/// diretório e renomeia por cima do destino.
pub fn write_file(doc: &Document, path: &Path, opts: &WriteOptions) -> Result<(), Error> {
    let bytes = write_bytes(doc, opts)?;
    write_atomic(path, &bytes)
}

pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), Error> {
    let dir = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let file_name = path
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "saida".into());
    let tmp = dir.join(format!(".{file_name}.{}.tmp", std::process::id()));
    {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    std::fs::rename(&tmp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })?;
    Ok(())
}

fn to_json<T: Serialize>(v: &T) -> Result<Vec<u8>, Error> {
    Ok(serde_json::to_vec(v)?)
}
