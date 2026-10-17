//! Núcleo do GraphDraw.
//!
//! - [`model`]: modelo de documento (doc 02), unidade interna µm.
//! - [`gdr`]: formato nativo `.gdr` (doc 13).
//! - [`cdr`]: importação `.cdr`/`.cmx` via libcdr (doc 14).
//! - [`render`]: cena, texto e rasterização.
//! - [`export`]: SVG, PDF e PNG (doc 15).
//! - [`open`]: abertura com detecção de formato pelo conteúdo.

pub mod cdr;
pub mod cms;
pub mod edit;
pub mod export;
pub mod gdr;
pub mod geometry;
pub mod model;
pub mod open;
pub mod render;
pub mod report;
pub mod units;

pub use model::Document;
pub use open::{FileFormat, open_bytes, open_path};
pub use report::ImportReport;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("erro de E/S: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON inválido: {0}")]
    Json(#[from] serde_json::Error),
    #[error("pacote ZIP: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("arquivo corrompido: {0}")]
    Corrupt(String),
    #[error("versão não suportada do formato {format}: '{version}'")]
    UnsupportedVersion { format: String, version: String },
    #[error("formato de arquivo não reconhecido")]
    UnknownFormat,
    #[error("{0}")]
    Cdr(String),
    #[error("{0}")]
    Other(String),
}
