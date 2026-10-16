//! Documento, páginas e camadas (doc 02: GD-DOC, GD-PAG, GD-CAM).

use std::collections::BTreeMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::color::Color;
use super::object::{Object, new_id};

pub const FORMAT_VERSION: &str = "1.0";

/// Conteúdo de `document.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentData {
    pub format_version: String,
    #[serde(default)]
    pub settings: Settings,
    #[serde(default)]
    pub master_page: MasterPage,
    pub pages: Vec<Page>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Configurações do documento (GD-DOC-01..03, GD-PAG-02).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    /// Unidade de exibição (`mm`, `cm`, `in`, `pt`, `pc`, `px`).
    #[serde(default = "mm")]
    pub display_unit: String,
    /// Canto de origem exibido na UI: `bottom-left` (padrão), `top-left`...
    #[serde(default = "bottom_left")]
    pub origin: String,
    /// Sangria em µm.
    #[serde(default)]
    pub bleed: f64,
    /// Escala de desenho `[papel, mundo]` (ex.: `[1, 100]`); afeta só réguas.
    #[serde(default = "unit_scale")]
    pub drawing_scale: [f64; 2],
    /// Resolução padrão de rasterização (ppp).
    #[serde(default = "dpi")]
    pub resolution: f64,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            display_unit: mm(),
            origin: bottom_left(),
            bleed: 0.0,
            drawing_scale: unit_scale(),
            resolution: dpi(),
            extra: Map::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct MasterPage {
    #[serde(default)]
    pub layers: Vec<Layer>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Página (GD-PAG-01, GD-PAG-05). Tamanho em µm.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Page {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub width: f64,
    pub height: f64,
    /// `None` = transparente.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<Color>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub hide_master: bool,
    #[serde(default)]
    pub layers: Vec<Layer>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Page {
    pub fn new(width: f64, height: f64) -> Page {
        Page {
            id: new_id(),
            name: None,
            width,
            height,
            background: None,
            hide_master: false,
            layers: vec![Layer::new("Camada 1")],
            extra: Map::new(),
        }
    }
}

/// Camada (GD-CAM-01).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Layer {
    pub id: String,
    pub name: String,
    #[serde(default = "yes")]
    pub visible: bool,
    #[serde(default = "yes")]
    pub printable: bool,
    #[serde(default)]
    pub locked: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<Color>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub wireframe: bool,
    #[serde(default)]
    pub objects: Vec<Object>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Layer {
    pub fn new(name: &str) -> Layer {
        Layer {
            id: new_id(),
            name: name.into(),
            visible: true,
            printable: true,
            locked: false,
            color: None,
            wireframe: false,
            objects: Vec::new(),
            extra: Map::new(),
        }
    }
}

/// `metadata.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Metadata {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keywords: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modified: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Cor nomeada/global (GD-PAL-01, GD-PAL-03).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NamedColor {
    pub id: String,
    pub name: String,
    pub color: Color,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `resources/colors.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ColorsResource {
    #[serde(default)]
    pub colors: Vec<NamedColor>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Documento em memória.
#[derive(Debug, Clone, PartialEq)]
pub struct Document {
    pub data: DocumentData,
    pub metadata: Metadata,
    pub colors: ColorsResource,
    /// Bytes das imagens indexados pelo caminho do membro
    /// (`resources/images/<sha256>.<ext>`).
    pub images: BTreeMap<String, Arc<Vec<u8>>>,
    /// Membros desconhecidos do pacote, preservados ao salvar (GD-GDR-03).
    pub extra_members: BTreeMap<String, Arc<Vec<u8>>>,
}

impl Document {
    pub fn new() -> Document {
        Document {
            data: DocumentData {
                format_version: FORMAT_VERSION.into(),
                settings: Settings::default(),
                master_page: MasterPage::default(),
                pages: Vec::new(),
                extra: Map::new(),
            },
            metadata: Metadata::default(),
            colors: ColorsResource::default(),
            images: BTreeMap::new(),
            extra_members: BTreeMap::new(),
        }
    }

    pub fn pages(&self) -> &[Page] {
        &self.data.pages
    }

    /// Registra bytes de imagem e devolve o caminho `resources/images/<sha>.<ext>`
    /// (deduplicado por hash, GD-GDR-02).
    pub fn add_image(&mut self, bytes: Vec<u8>, ext: &str) -> String {
        use sha2::{Digest, Sha256};
        let hash = hex(&Sha256::digest(&bytes));
        let path = format!("resources/images/{hash}.{ext}");
        self.images
            .entry(path.clone())
            .or_insert_with(|| Arc::new(bytes));
        path
    }

    /// Visita todos os objetos (página mestra e páginas).
    pub fn walk_objects<'a>(&'a self, f: &mut dyn FnMut(&'a Object)) {
        for l in &self.data.master_page.layers {
            for o in &l.objects {
                o.walk(f);
            }
        }
        for p in &self.data.pages {
            for l in &p.layers {
                for o in &l.objects {
                    o.walk(f);
                }
            }
        }
    }
}

impl Default for Document {
    fn default() -> Self {
        Document::new()
    }
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn mm() -> String {
    "mm".into()
}
fn bottom_left() -> String {
    "bottom-left".into()
}
fn unit_scale() -> [f64; 2] {
    [1.0, 1.0]
}
fn dpi() -> f64 {
    300.0
}
fn yes() -> bool {
    true
}
