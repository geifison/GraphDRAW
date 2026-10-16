//! Importação `.cdr`/`.cmx` (doc 14) sobre a libcdr + librevenge (MPL-2.0),
//! executadas no processo auxiliar `gd-cdr-bridge`.

pub mod bridge;
pub mod mapper;
pub mod sniff;

use crate::Error;
use crate::model::Document;
use crate::report::{ImportReport, Severity};

pub use sniff::{Sniffed, sniff};

/// Importa `.cdr`/`.cmx` da memória.
pub fn import_bytes(bytes: &[u8]) -> Result<(Document, ImportReport), Error> {
    let sniffed = sniff(bytes);
    let label = sniffed
        .as_ref()
        .map(Sniffed::version_label)
        .unwrap_or_else(|| "formato não identificado".into());
    let mut report = ImportReport::new(sniffed.as_ref().map(|s| s.format).unwrap_or("cdr"));
    report.detected_version = Some(label.clone());

    let out = bridge::run(bytes)?;
    if out.timed_out {
        return Err(Error::Cdr(format!(
            "a importação excedeu {} s e foi interrompida (versão detectada: {label})",
            bridge::TIMEOUT.as_secs()
        )));
    }
    let produced_pages = out
        .stdout
        .windows(b"\"startPage\"".len())
        .any(|w| w == b"\"startPage\"");
    match out.exit_code {
        Some(0) => {}
        Some(2) => {
            return Err(Error::Cdr(format!(
                "arquivo não suportado, protegido ou corrompido (versão detectada: {label}). \
                 Verifique se o arquivo abre no programa de origem e tente salvá-lo numa versão anterior."
            )));
        }
        code if produced_pages => report.push(
            Severity::Warning,
            "cdr.partial",
            format!(
                "o arquivo foi lido apenas em parte (código {}); o conteúdo pode estar incompleto",
                code.map(|c| c.to_string())
                    .unwrap_or_else(|| "encerrado".into())
            ),
            None,
            None,
        ),
        code => {
            let detail = out.stderr.lines().next().unwrap_or("").trim().to_string();
            return Err(Error::Cdr(format!(
                "não foi possível ler o arquivo (versão detectada: {label}; código {}){}",
                code.map(|c| c.to_string())
                    .unwrap_or_else(|| "encerrado".into()),
                if detail.is_empty() {
                    String::new()
                } else {
                    format!(": {detail}")
                }
            )));
        }
    }
    if out.stdout.windows(9).any(|w| w == b"\"format\":")
        && let Some(fmt) = std::str::from_utf8(&out.stdout)
            .ok()
            .and_then(|s| s.lines().next())
            .and_then(|l| {
                serde_json::from_str::<serde_json::Value>(l)
                    .ok()?
                    .get("format")?
                    .as_str()
                    .map(String::from)
            })
    {
        report.source_format = fmt;
    }

    let doc = mapper::map_events(&out.stdout, &mut report);
    report.info(
        "cdr.color.rgb-only",
        "cores importadas em RGB: a libcdr converte CMYK/cores especiais antes de entregá-las (GD-CDR-04 parcial)",
        None,
    );
    if doc.data.pages.is_empty() {
        return Err(Error::Cdr(format!(
            "nenhuma página encontrada no arquivo (versão detectada: {label})"
        )));
    }
    Ok((doc, report))
}
