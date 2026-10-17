//! Exportação SVG/PDF/PNG (doc 15) e conversão usada pela CLI (GD-EXP-06,
//! GD-CDR-07).

pub mod pdf;
pub mod svg;

use std::path::{Path, PathBuf};

use crate::Error;
use crate::model::Document;
use crate::render::raster::{self, RasterOptions};
use crate::render::{ImageCache, Scene, SceneOptions, build_scene};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Svg,
    Pdf,
    Png,
    Gdr,
}

impl ExportFormat {
    pub fn from_extension(ext: &str) -> Option<ExportFormat> {
        Some(match ext.to_ascii_lowercase().as_str() {
            "svg" => ExportFormat::Svg,
            "pdf" => ExportFormat::Pdf,
            "png" => ExportFormat::Png,
            "gdr" => ExportFormat::Gdr,
            _ => return None,
        })
    }

    pub fn extension(self) -> &'static str {
        match self {
            ExportFormat::Svg => "svg",
            ExportFormat::Pdf => "pdf",
            ExportFormat::Png => "png",
            ExportFormat::Gdr => "gdr",
        }
    }

    /// Formatos de uma página por arquivo.
    pub fn single_page(self) -> bool {
        matches!(self, ExportFormat::Svg | ExportFormat::Png)
    }
}

/// Condição de impressão do PDF/X (registrada no ICC, http://www.color.org).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PrintCondition {
    /// Offset em papel couché (ISO 12647-2:2004) — padrão.
    #[default]
    Fogra39,
    /// Offset em papel não couché.
    Fogra47,
    /// SWOP (publicações nos EUA).
    Swop,
}

impl PrintCondition {
    pub const ALL: [PrintCondition; 3] = [
        PrintCondition::Fogra39,
        PrintCondition::Fogra47,
        PrintCondition::Swop,
    ];
    pub fn identifier(self) -> &'static str {
        match self {
            PrintCondition::Fogra39 => "FOGRA39",
            PrintCondition::Fogra47 => "FOGRA47",
            PrintCondition::Swop => "CGATS TR 001",
        }
    }
    pub fn description(self) -> &'static str {
        match self {
            PrintCondition::Fogra39 => "Coated FOGRA39 (ISO 12647-2:2004)",
            PrintCondition::Fogra47 => "Uncoated FOGRA47 (ISO 12647-2:2004)",
            PrintCondition::Swop => "SWOP (CGATS TR 001)",
        }
    }
    pub fn from_name(s: &str) -> Option<PrintCondition> {
        match s.to_ascii_lowercase().replace([' ', '-', '_'], "").as_str() {
            "fogra39" | "couche" | "coated" => Some(PrintCondition::Fogra39),
            "fogra47" | "naocouche" | "uncoated" => Some(PrintCondition::Fogra47),
            "swop" | "cgatstr001" => Some(PrintCondition::Swop),
            _ => None,
        }
    }
}

/// Tipo de PDF (GD-EXP-07).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PdfStandard {
    /// PDF comum, para tela.
    #[default]
    Screen,
    /// PDF/X-1a:2001, para gráfica.
    X1a(PrintCondition),
}

#[derive(Debug, Clone)]
pub struct ExportOptions {
    /// Páginas (índices base 0); vazio = todas (GD-EXP-01).
    pub pages: Vec<usize>,
    /// Resolução PNG.
    pub dpi: f64,
    /// Converter texto em curvas (GD-EXP-02). PDF sempre usa curvas no M0.
    pub text_to_curves: bool,
    pub antialias: bool,
    /// Fundo PNG (RGBA); `None` = transparente.
    pub background: Option<[u8; 4]>,
    /// PDF comum ou PDF/X-1a.
    pub pdf: PdfStandard,
}

impl Default for ExportOptions {
    fn default() -> Self {
        ExportOptions {
            pages: Vec::new(),
            dpi: 300.0,
            text_to_curves: false,
            antialias: true,
            background: None,
            pdf: PdfStandard::Screen,
        }
    }
}

/// Interpreta `N`, `N-M` ou listas `1,3-5` (base 1) em índices base 0.
pub fn parse_pages(spec: &str, page_count: usize) -> Result<Vec<usize>, Error> {
    let mut out = Vec::new();
    for part in spec.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        let (a, b) = match part.split_once('-') {
            Some((a, b)) => (a.trim(), b.trim()),
            None => (part, part),
        };
        let parse = |s: &str| {
            s.parse::<usize>()
                .map_err(|_| Error::Other(format!("página inválida: '{s}'")))
        };
        let a = parse(a)?;
        let b = if b.is_empty() { page_count } else { parse(b)? };
        if a == 0 || b < a || b > page_count {
            return Err(Error::Other(format!(
                "intervalo de páginas fora do documento (1–{page_count}): '{part}'"
            )));
        }
        out.extend((a - 1)..b);
    }
    Ok(out)
}

fn selected(doc: &Document, opts: &ExportOptions) -> Vec<usize> {
    if opts.pages.is_empty() {
        (0..doc.data.pages.len()).collect()
    } else {
        opts.pages.clone()
    }
}

pub fn scene_for(doc: &Document, index: usize, cache: &ImageCache) -> Option<Scene> {
    let page = doc.data.pages.get(index)?;
    Some(build_scene(doc, page, SceneOptions { print: true }, cache))
}

pub fn svg_page(doc: &Document, index: usize, opts: &ExportOptions) -> Result<String, Error> {
    let scene = scene_for(doc, index, &ImageCache::default())
        .ok_or_else(|| Error::Other("página inexistente".into()))?;
    Ok(svg::write_svg(&scene, opts))
}

pub fn png_page(doc: &Document, index: usize, opts: &ExportOptions) -> Result<Vec<u8>, Error> {
    let scene = scene_for(doc, index, &ImageCache::default())
        .ok_or_else(|| Error::Other("página inexistente".into()))?;
    let s = opts.dpi / crate::units::UM_PER_INCH;
    let (w, h) = (
        (scene.width * s).round().max(1.0),
        (scene.height * s).round().max(1.0),
    );
    if w * h > 400e6 {
        return Err(Error::Other(format!(
            "imagem grande demais ({w}×{h} px); reduza o --dpi"
        )));
    }
    let view = raster::page_view(scene.height, opts.dpi);
    let ropts = RasterOptions {
        antialias: opts.antialias,
        background: opts.background,
    };
    let pm = raster::render(&scene, view, w as u32, h as u32, &ropts)
        .ok_or_else(|| Error::Other("falha ao alocar a imagem".into()))?;
    raster::encode_png(&pm, Some(opts.dpi))
}

pub fn pdf_document(doc: &Document, opts: &ExportOptions) -> Result<Vec<u8>, Error> {
    pdf_document_report(doc, opts).map(|r| r.0)
}

/// PDF com o relatório do que foi convertido (PDF/X-1a; vazio no comum).
pub fn pdf_document_report(
    doc: &Document,
    opts: &ExportOptions,
) -> Result<(Vec<u8>, pdf::PdfReport), Error> {
    let cache = ImageCache::default();
    let scenes: Vec<Scene> = selected(doc, opts)
        .into_iter()
        .filter_map(|i| scene_for(doc, i, &cache))
        .collect();
    if scenes.is_empty() {
        return Err(Error::Other("documento sem páginas".into()));
    }
    Ok(pdf::write_pdf_report(
        &scenes,
        doc.metadata.title.as_deref(),
        opts,
    ))
}

/// Exporta para `out`. Formatos de página única com várias páginas
/// selecionadas geram `<nome>-<n>.<ext>`. Devolve os arquivos gravados.
pub fn export(
    doc: &Document,
    out: &Path,
    format: ExportFormat,
    opts: &ExportOptions,
) -> Result<Vec<PathBuf>, Error> {
    export_with_report(doc, out, format, opts).map(|r| r.0)
}

/// Como [`export`], devolvendo também os avisos (conversões do PDF/X-1a).
pub fn export_with_report(
    doc: &Document,
    out: &Path,
    format: ExportFormat,
    opts: &ExportOptions,
) -> Result<(Vec<PathBuf>, Vec<String>), Error> {
    if doc.data.pages.is_empty() {
        return Err(Error::Other("documento sem páginas".into()));
    }
    match format {
        ExportFormat::Gdr => {
            crate::gdr::write_file(doc, out, &Default::default())?;
            Ok((vec![out.to_path_buf()], Vec::new()))
        }
        ExportFormat::Pdf => {
            let (bytes, report) = pdf_document_report(doc, opts)?;
            crate::gdr::write_atomic(out, &bytes)?;
            Ok((vec![out.to_path_buf()], report.messages()))
        }
        ExportFormat::Svg | ExportFormat::Png => {
            let pages = selected(doc, opts);
            let mut written = Vec::new();
            for &i in &pages {
                let path = if pages.len() > 1 {
                    numbered(out, i + 1)
                } else {
                    out.to_path_buf()
                };
                let bytes = match format {
                    ExportFormat::Svg => svg_page(doc, i, opts)?.into_bytes(),
                    _ => png_page(doc, i, opts)?,
                };
                crate::gdr::write_atomic(&path, &bytes)?;
                written.push(path);
            }
            Ok((written, Vec::new()))
        }
    }
}

fn numbered(path: &Path, n: usize) -> PathBuf {
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let ext = path
        .extension()
        .map(|s| format!(".{}", s.to_string_lossy()))
        .unwrap_or_default();
    path.with_file_name(format!("{stem}-{n}{ext}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_ranges() {
        assert_eq!(parse_pages("2", 3).unwrap(), vec![1]);
        assert_eq!(parse_pages("1-3", 3).unwrap(), vec![0, 1, 2]);
        assert_eq!(parse_pages("1,3", 3).unwrap(), vec![0, 2]);
        assert_eq!(parse_pages("2-", 3).unwrap(), vec![1, 2]);
        assert!(parse_pages("4", 3).is_err());
        assert!(parse_pages("0", 3).is_err());
    }
}
