//! Abertura de arquivos com detecção pelo conteúdo, não só pela extensão
//! (GD-CDR-01, GD-GDR-01).

use std::path::Path;

use crate::Error;
use crate::model::Document;
use crate::report::ImportReport;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileFormat {
    Gdr,
    Cdr,
    Cmx,
}

/// Detecta o formato pelos bytes; `ext_hint` só desempata quando o conteúdo
/// não é conclusivo.
pub fn detect(bytes: &[u8], ext_hint: Option<&str>) -> Option<FileFormat> {
    if crate::gdr::sniff(bytes) {
        return Some(FileFormat::Gdr);
    }
    if let Some(s) = crate::cdr::sniff(bytes) {
        return Some(if s.format == "cmx" {
            FileFormat::Cmx
        } else {
            FileFormat::Cdr
        });
    }
    match ext_hint.map(str::to_ascii_lowercase).as_deref() {
        // ZIP sem `mimetype` válido mas com extensão .gdr: tenta leitura tolerante.
        Some("gdr") if bytes.starts_with(b"PK") => Some(FileFormat::Gdr),
        Some("cdr") => Some(FileFormat::Cdr),
        Some("cmx") => Some(FileFormat::Cmx),
        _ => None,
    }
}

pub fn open_bytes(bytes: &[u8], ext_hint: Option<&str>) -> Result<(Document, ImportReport), Error> {
    let (doc, mut report) = match detect(bytes, ext_hint) {
        Some(FileFormat::Gdr) => crate::gdr::read_bytes(bytes)?,
        Some(FileFormat::Cdr) | Some(FileFormat::Cmx) => crate::cdr::import_bytes(bytes)?,
        None => return Err(Error::UnknownFormat),
    };
    check_fonts(&doc, &mut report);
    Ok((doc, report))
}

/// Fonte ausente: avisa e informa a substituta; o texto mantém o nome
/// original no documento (GD-FNT-02, GD-CDR-05).
pub fn check_fonts(doc: &Document, report: &mut ImportReport) {
    use std::collections::BTreeMap;
    let mut seen: BTreeMap<(String, u16, bool), ()> = BTreeMap::new();
    doc.walk_objects(&mut |o| {
        if let crate::model::ObjectKind::Text(t) = &o.kind {
            for p in &t.paragraphs {
                for s in &p.spans {
                    seen.insert((s.font.family.clone(), s.font.weight, s.font.italic), ());
                }
            }
        }
    });
    let mut done = std::collections::BTreeSet::new();
    for (family, weight, italic) in seen.into_keys() {
        if !done.insert(family.clone()) {
            continue;
        }
        match crate::render::text::resolve_family(&family, weight, italic) {
            Some((used, true)) => report.warn(
                "font.missing",
                format!("fonte '{family}' não instalada; exibida com '{used}' (o nome original é mantido no documento)"),
                None,
            ),
            None => report.warn("font.none", "nenhuma fonte disponível no sistema; texto não será exibido", None),
            _ => {}
        }
    }
}

pub fn open_path(path: &Path) -> Result<(Document, ImportReport), Error> {
    let bytes = std::fs::read(path)?;
    let ext = path.extension().and_then(|e| e.to_str());
    open_bytes(&bytes, ext)
}
