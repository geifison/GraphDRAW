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
    /// Guias de todas as páginas (GD-GUI-01).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub guides: Vec<Guide>,
    /// Gerenciamento de cor do documento (GD-CMS-02).
    #[serde(default, skip_serializing_if = "is_default_color")]
    pub color: crate::cms::ColorManagement,
    /// Caminhos abertos recebem preenchimento (GD-FIL-12).
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub fill_open_paths: bool,
    /// Limites da verificação prévia (GD-PRE-02).
    #[serde(default, skip_serializing_if = "is_default_preflight")]
    pub preflight: crate::preflight::PreflightSettings,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

fn is_true(b: &bool) -> bool {
    *b
}

fn is_default_preflight(p: &crate::preflight::PreflightSettings) -> bool {
    *p == crate::preflight::PreflightSettings::default()
}

fn is_default_color(c: &crate::cms::ColorManagement) -> bool {
    *c == crate::cms::ColorManagement::default()
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            display_unit: mm(),
            origin: bottom_left(),
            bleed: 0.0,
            drawing_scale: unit_scale(),
            resolution: dpi(),
            color: Default::default(),
            preflight: Default::default(),
            fill_open_paths: true,
            guides: Vec::new(),
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

/// Guia (GD-GUI-01): horizontal (`position` = y), vertical (`position` =
/// x) ou inclinada (passa por `point` com `angle` graus a partir de +X).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Guide {
    /// `horizontal`, `vertical` ou `angled`.
    pub kind: String,
    #[serde(default)]
    pub position: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub point: Option<[f64; 2]>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub angle: f64,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub locked: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<Color>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

fn is_zero(v: &f64) -> bool {
    *v == 0.0
}

impl Guide {
    pub fn horizontal(y: f64) -> Guide {
        Guide::new("horizontal", y, None, 0.0)
    }
    pub fn vertical(x: f64) -> Guide {
        Guide::new("vertical", x, None, 0.0)
    }
    /// Guia inclinada por `p` com `angle` graus.
    pub fn angled(p: kurbo::Point, angle: f64) -> Guide {
        Guide::new("angled", 0.0, Some([p.x, p.y]), angle)
    }
    fn new(kind: &str, position: f64, point: Option<[f64; 2]>, angle: f64) -> Guide {
        Guide {
            kind: kind.into(),
            position,
            point,
            angle,
            locked: false,
            color: None,
            extra: Map::new(),
        }
    }

    /// Um ponto da guia e a direção unitária dela.
    pub fn line(&self) -> (kurbo::Point, kurbo::Vec2) {
        match self.kind.as_str() {
            "horizontal" => (
                kurbo::Point::new(0.0, self.position),
                kurbo::Vec2::new(1.0, 0.0),
            ),
            "vertical" => (
                kurbo::Point::new(self.position, 0.0),
                kurbo::Vec2::new(0.0, 1.0),
            ),
            _ => {
                let p = self.point.unwrap_or([0.0, 0.0]);
                let a = self.angle.to_radians();
                (
                    kurbo::Point::new(p[0], p[1]),
                    kurbo::Vec2::new(a.cos(), a.sin()),
                )
            }
        }
    }

    /// Ponto da guia mais próximo de `p`.
    pub fn project(&self, p: kurbo::Point) -> kurbo::Point {
        let (o, d) = self.line();
        o + d * (p - o).dot(d)
    }

    pub fn distance(&self, p: kurbo::Point) -> f64 {
        (self.project(p) - p).hypot()
    }

    /// Cruzamento com outra guia (`None` se paralelas).
    pub fn intersect(&self, other: &Guide) -> Option<kurbo::Point> {
        let (p, r) = self.line();
        let (q, s) = other.line();
        let den = r.cross(s);
        if den.abs() < 1e-12 {
            return None;
        }
        let t = (q - p).cross(s) / den;
        Some(p + r * t)
    }

    /// Trecho visível da guia dentro de `area` (para desenhar).
    pub fn clip(&self, area: kurbo::Rect) -> Option<(kurbo::Point, kurbo::Point)> {
        let (o, d) = self.line();
        let (mut t0, mut t1) = (f64::NEG_INFINITY, f64::INFINITY);
        for (oc, dc, lo, hi) in [(o.x, d.x, area.x0, area.x1), (o.y, d.y, area.y0, area.y1)] {
            if dc.abs() < 1e-12 {
                if oc < lo || oc > hi {
                    return None;
                }
            } else {
                let (a, b) = ((lo - oc) / dc, (hi - oc) / dc);
                t0 = t0.max(a.min(b));
                t1 = t1.min(a.max(b));
            }
        }
        (t0 <= t1).then(|| (o + d * t0, o + d * t1))
    }
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
    /// Sangria própria em µm (GD-PGT-16); `None` = a do documento.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bleed: Option<f64>,
    /// Página bloqueada: nada nela é selecionável ou editável (GD-PGT-18).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub locked: bool,
    /// Etiqueta de cor da aba (GD-PGT-20).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag_color: Option<Color>,
    /// Guias só desta página (GD-GUI-01).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub guides: Vec<Guide>,
    #[serde(default)]
    pub layers: Vec<Layer>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Page {
    /// Cópia só das propriedades (sem camadas nem objetos).
    pub fn props_only(&self) -> Page {
        Page {
            id: self.id.clone(),
            name: self.name.clone(),
            width: self.width,
            height: self.height,
            background: self.background.clone(),
            hide_master: self.hide_master,
            bleed: self.bleed,
            locked: self.locked,
            tag_color: self.tag_color.clone(),
            guides: self.guides.clone(),
            layers: Vec::new(),
            extra: self.extra.clone(),
        }
    }

    /// Quantidade de objetos de primeiro nível em todas as camadas.
    pub fn object_count(&self) -> usize {
        self.layers.iter().map(|l| l.objects.len()).sum()
    }

    pub fn new(width: f64, height: f64) -> Page {
        Page {
            id: new_id(),
            name: None,
            width,
            height,
            background: None,
            hide_master: false,
            bleed: None,
            locked: false,
            tag_color: None,
            guides: Vec::new(),
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
    /// Camada de guias (GD-GUI-03): os objetos dela são guias — tracejados
    /// na tela, fora da impressão e da exportação.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub guides: bool,
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
            guides: false,
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
