//! Objetos do documento (doc 02, GD-OBJ-01..05, GD-GDR-05).
//!
//! Serialização feita via `serde_json::Value` para (1) preservar campos
//! desconhecidos (GD-GDR-03) e (2) nunca falhar na leitura: um objeto
//! inválido vira `ObjectKind::Unknown` com a mensagem de erro, e o resto do
//! documento abre normalmente (GD-GDR-10).

use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};

use super::geom::{FillRule, Subpath, Transform};
use super::paint::{Fill, Stroke};

/// Gera um ID novo (UUID v7, GD-GDR-04).
pub fn new_id() -> String {
    uuid::Uuid::now_v7().to_string()
}

#[derive(Debug, Clone, PartialEq)]
pub struct Object {
    pub id: String,
    pub name: Option<String>,
    pub transform: Transform,
    pub fill: Option<Fill>,
    pub stroke: Option<Stroke>,
    pub opacity: Option<f64>,
    pub visible: bool,
    pub locked: bool,
    /// `None` = herda da camada (GD-OBJ-04).
    pub printable: Option<bool>,
    pub user_data: Option<BTreeMap<String, String>>,
    /// Modo de mesclagem com o que está embaixo (GD-TRA-02).
    pub blend: BlendMode,
    pub kind: ObjectKind,
}

/// Modos de mesclagem (GD-TRA-02): os 16 do PDF.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BlendMode {
    #[default]
    Normal,
    Multiply,
    Screen,
    Overlay,
    Darken,
    Lighten,
    ColorDodge,
    ColorBurn,
    HardLight,
    SoftLight,
    Difference,
    Exclusion,
    Hue,
    Saturation,
    Color,
    Luminosity,
}

impl BlendMode {
    pub const ALL: [BlendMode; 16] = [
        BlendMode::Normal,
        BlendMode::Multiply,
        BlendMode::Screen,
        BlendMode::Overlay,
        BlendMode::Darken,
        BlendMode::Lighten,
        BlendMode::ColorDodge,
        BlendMode::ColorBurn,
        BlendMode::HardLight,
        BlendMode::SoftLight,
        BlendMode::Difference,
        BlendMode::Exclusion,
        BlendMode::Hue,
        BlendMode::Saturation,
        BlendMode::Color,
        BlendMode::Luminosity,
    ];
}

#[derive(Debug, Clone, PartialEq)]
pub enum ObjectKind {
    Path(PathData),
    Rectangle(RectData),
    Ellipse(EllipseData),
    Polygon(PolygonData),
    Group(GroupData),
    Text(TextData),
    Image(ImageData),
    /// Cota (GD-COT-01..03, doc 08).
    Dimension(DimensionData),
    /// Forma básica paramétrica (GD-PRI-07).
    Basic(BasicShapeData),
    /// Conector entre objetos (GD-FER-41, GD-CON-01).
    Connector(ConnectorData),
    /// B-spline cúbica uniforme (GD-BSP-01).
    BSpline(BSplineData),
    /// Tipo desconhecido ou objeto inválido: JSON original preservado.
    Unknown {
        raw: Value,
        error: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PathData {
    pub subpaths: Vec<Subpath>,
    #[serde(default)]
    pub fill_rule: FillRule,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Canto de retângulo (GD-PRI-01). `kind`: `round`, `chamfer`, `scallop`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Corner {
    pub kind: String,
    pub radius: f64,
}

/// Retângulo paramétrico ocupando `[0,width]×[0,height]` no espaço local.
/// `corners`, quando presente, segue a ordem: inferior-esquerdo,
/// inferior-direito, superior-direito, superior-esquerdo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RectData {
    pub width: f64,
    pub height: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub corners: Option<Vec<Corner>>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Elipse paramétrica centrada na origem local (GD-PRI-02).
/// Ângulos em graus, anti-horário a partir de +X.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EllipseData {
    pub rx: f64,
    pub ry: f64,
    /// `ellipse`, `arc` ou `pie`.
    #[serde(default = "ellipse_mode")]
    pub mode: String,
    #[serde(default)]
    pub start_angle: f64,
    #[serde(default = "full_turn")]
    pub end_angle: f64,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Polígono ou estrela paramétricos (GD-PRI-03/04), centrados na origem
/// local e inscritos na elipse de raios `rx`, `ry`; o primeiro vértice fica
/// em cima.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolygonData {
    pub rx: f64,
    pub ry: f64,
    /// Lados (polígono) ou pontas (estrelas), 3–500.
    pub sides: u32,
    /// `polygon`, `star` (pontas alternando raio externo e interno) ou
    /// `complex` (estrela de linhas cruzadas, com auto-interseção).
    #[serde(default = "polygon_mode")]
    pub mode: String,
    /// Nitidez da ponta, 0–1: na estrela, quanto o raio interno encolhe;
    /// na complexa, quantos vértices cada linha pula.
    #[serde(default = "half")]
    pub sharpness: f64,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

fn polygon_mode() -> String {
    "polygon".into()
}

fn half() -> f64 {
    0.5
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroupData {
    pub children: Vec<Object>,
    /// Simetria ao vivo (GD-FER-33): o grupo desenha também as cópias.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symmetry: Option<Symmetry>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Simetria do grupo (GD-FER-33): `mirror` = `count` eixos de espelho
/// (1 = um espelho; N = caleidoscópio de N eixos), `radial` = `count`
/// cópias giradas. Centro e ângulo do primeiro eixo no espaço do grupo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Symmetry {
    pub kind: String,
    pub count: u32,
    pub center: [f64; 2],
    #[serde(default)]
    pub angle: f64,
}

/// Fonte de um trecho de texto. `size` em µm (corpo).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FontSpec {
    pub family: String,
    pub size: f64,
    #[serde(default = "normal_weight")]
    pub weight: u16,
    #[serde(default)]
    pub italic: bool,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextSpan {
    pub text: String,
    pub font: FontSpec,
    /// Sobrepõe o preenchimento do objeto, se presente.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<Fill>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Paragraph {
    /// `left`, `center`, `right`, `justify`, `forceJustify`.
    #[serde(default = "left")]
    pub align: String,
    pub spans: Vec<TextSpan>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Texto (doc 06). A origem local é o canto superior esquerdo do bloco;
/// as linhas avançam para baixo (−Y). `frame` = largura/altura do quadro
/// (texto de parágrafo) ou da caixa usada para alinhamento.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextData {
    /// `artistic` ou `paragraph`.
    #[serde(default = "artistic")]
    pub text_kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame: Option<[f64; 2]>,
    pub paragraphs: Vec<Paragraph>,
    /// Entrelinha como múltiplo do corpo.
    #[serde(default = "line_spacing")]
    pub line_spacing: f64,
    /// Texto em caminho (GD-TXT-09).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<TextPath>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Caminho do texto (GD-TXT-09), em coordenadas locais do texto. A linha
/// de base corre sobre o primeiro subcaminho a partir de `offset` (µm);
/// `distance` afasta do caminho; `side` = `above` ou `below`;
/// `orientation` = `rotate` (gira cada letra), `skew-v`/`skew-h`
/// (inclina mantendo as verticais/horizontais) ou `upright` (de pé);
/// `mirror` percorre o caminho ao contrário. `guide` = ID do objeto de
/// onde a forma veio; `detached` = já separado dele (GD-CMB-03): o texto
/// mantém a curva, independente.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextPath {
    pub subpaths: Vec<Subpath>,
    #[serde(default)]
    pub offset: f64,
    #[serde(default)]
    pub distance: f64,
    #[serde(default = "above")]
    pub side: String,
    #[serde(default = "rotate")]
    pub orientation: String,
    #[serde(default)]
    pub mirror: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guide: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub detached: bool,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

fn above() -> String {
    "above".into()
}
fn rotate() -> String {
    "rotate".into()
}

/// Cota (GD-COT-01..03). `dim_kind`: `horizontal`, `vertical`,
/// `aligned` (paralela), `angular`, `radius`, `diameter` ou `segment`
/// (comprimento de um segmento de curva). `points` no espaço local:
/// lineares e segmento `[a, b]`; angular `[vértice, a, b]`; raio e
/// diâmetro `[centro, ponto da borda]`. `offset` (µm): distância da linha
/// de cota (lineares), raio do arco (angular) ou guia além da borda (raio).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DimensionData {
    pub dim_kind: String,
    pub points: Vec<[f64; 2]>,
    #[serde(default)]
    pub offset: f64,
    /// Nó ligado a cada ponto (GD-COT-02); `None` = ponto livre.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub links: Vec<Option<DimLink>>,
    /// Segmento medido (tipo `segment`): `node` = índice do segmento.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub segment: Option<DimLink>,
    #[serde(default)]
    pub format: DimFormat,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Forma básica (GD-PRI-07): `shape` da biblioteca (`render::basic`), caixa
/// local `[0,width]×[0,height]` e parâmetros de ajuste (0–1; a ponta dos
/// balões pode sair da faixa).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BasicShapeData {
    pub shape: String,
    pub width: f64,
    pub height: f64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub params: Vec<f64>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// B-spline (GD-BSP-01): pontos de controle; os fixados a curva atravessa,
/// os flutuantes a puxam. Aberta, a curva começa e termina nas pontas.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BSplineData {
    pub points: Vec<BSplinePoint>,
    #[serde(default)]
    pub closed: bool,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Ponto de controle da B-spline (espaço local).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BSplinePoint {
    pub p: [f64; 2],
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub fixed: bool,
}

/// Conector (GD-CON-01): `style` = `straight`, `elbow` (ângulo reto, com
/// `radius` nos cantos) ou `curved`; pontas na página.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectorData {
    pub style: String,
    pub from: ConnEnd,
    pub to: ConnEnd,
    #[serde(default)]
    pub radius: f64,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Ponta do conector: o ponto (página) e, se presa, o objeto e o encaixe
/// relativo à caixa dele (0–1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnEnd {
    pub point: [f64; 2],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchor: Option<[f64; 2]>,
}

/// Nó (ou segmento) de outro objeto a que a cota está ligada.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DimLink {
    pub object: String,
    pub subpath: usize,
    pub node: usize,
}

/// Formato da cota (GD-COT-03).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DimFormat {
    /// `mm`, `cm`, `m`, `in`, `pt`.
    pub unit: String,
    pub precision: u32,
    pub prefix: String,
    pub suffix: String,
    pub show_unit: bool,
    /// `above` (acima da linha) ou `inline` (sobre a linha).
    pub text_position: String,
    /// Ponta: `triangle`, `bar`, `circle` ou `none`.
    pub arrow: String,
    pub extension_lines: bool,
    /// Corpo do texto (µm) e família.
    pub text_size: f64,
    pub font: String,
}

impl Default for DimFormat {
    fn default() -> Self {
        DimFormat {
            unit: "mm".into(),
            precision: 1,
            prefix: String::new(),
            suffix: String::new(),
            show_unit: true,
            text_position: "above".into(),
            arrow: "triangle".into(),
            extension_lines: true,
            text_size: 3_500.0,
            font: "Arial".into(),
        }
    }
}

/// Imagem ocupando `[0,width]×[0,height]` no espaço local; `src` aponta
/// para `resources/images/<sha256>.<ext>`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImageData {
    pub width: f64,
    pub height: f64,
    pub src: String,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

fn ellipse_mode() -> String {
    "ellipse".into()
}
fn full_turn() -> f64 {
    360.0
}
fn normal_weight() -> u16 {
    400
}
fn left() -> String {
    "left".into()
}
fn artistic() -> String {
    "artistic".into()
}
fn line_spacing() -> f64 {
    1.2
}

const COMMON_KEYS: &[&str] = &[
    "id",
    "type",
    "name",
    "transform",
    "fill",
    "stroke",
    "opacity",
    "visible",
    "locked",
    "printable",
    "userData",
    "blendMode",
];

impl Object {
    pub fn new(kind: ObjectKind) -> Object {
        Object {
            id: new_id(),
            name: None,
            transform: Transform::IDENTITY,
            fill: None,
            stroke: None,
            opacity: None,
            visible: true,
            locked: false,
            printable: None,
            user_data: None,
            blend: BlendMode::Normal,
            kind,
        }
    }

    pub fn type_name(&self) -> &str {
        match &self.kind {
            ObjectKind::Path(_) => "path",
            ObjectKind::Rectangle(_) => "rectangle",
            ObjectKind::Ellipse(_) => "ellipse",
            ObjectKind::Polygon(_) => "polygon",
            ObjectKind::Group(_) => "group",
            ObjectKind::Text(_) => "text",
            ObjectKind::Image(_) => "image",
            ObjectKind::Dimension(_) => "dimension",
            ObjectKind::Basic(_) => "basic",
            ObjectKind::Connector(_) => "connector",
            ObjectKind::BSpline(_) => "bspline",
            ObjectKind::Unknown { raw, .. } => {
                raw.get("type").and_then(Value::as_str).unwrap_or("?")
            }
        }
    }

    /// Visita este objeto e todos os descendentes.
    pub fn walk<'a>(&'a self, f: &mut dyn FnMut(&'a Object)) {
        f(self);
        if let ObjectKind::Group(g) = &self.kind {
            for c in &g.children {
                c.walk(f);
            }
        }
    }

    pub fn from_value(v: Value) -> Object {
        match Self::try_from_value(&v) {
            Ok(o) => o,
            Err(e) => {
                let id = v
                    .get("id")
                    .and_then(Value::as_str)
                    .map(String::from)
                    .unwrap_or_else(new_id);
                let mut o = Object::new(ObjectKind::Unknown {
                    raw: v,
                    error: Some(e),
                });
                o.id = id;
                o
            }
        }
    }

    fn try_from_value(v: &Value) -> Result<Object, String> {
        let map = v.as_object().ok_or("objeto não é um mapa JSON")?;
        let id = map
            .get("id")
            .and_then(Value::as_str)
            .ok_or("campo 'id' ausente")?
            .to_string();
        let ty = map
            .get("type")
            .and_then(Value::as_str)
            .ok_or("campo 'type' ausente")?;
        let de = |key: &str| -> Result<Option<Value>, String> {
            Ok(map.get(key).filter(|x| !x.is_null()).cloned())
        };
        let transform = match de("transform")? {
            Some(t) => serde_json::from_value(t).map_err(|e| format!("transform inválido: {e}"))?,
            None => Transform::IDENTITY,
        };
        let fill = de("fill")?.map(Fill::from_value);
        let stroke = match de("stroke")? {
            Some(s) => {
                Some(serde_json::from_value(s).map_err(|e| format!("stroke inválido: {e}"))?)
            }
            None => None,
        };
        let opacity = map.get("opacity").and_then(Value::as_f64);
        let visible = map.get("visible").and_then(Value::as_bool).unwrap_or(true);
        let locked = map.get("locked").and_then(Value::as_bool).unwrap_or(false);
        let printable = map.get("printable").and_then(Value::as_bool);
        let user_data = match de("userData")? {
            Some(u) => {
                Some(serde_json::from_value(u).map_err(|e| format!("userData inválido: {e}"))?)
            }
            None => None,
        };
        let name = map.get("name").and_then(Value::as_str).map(String::from);
        // Modo desconhecido (versão futura) cai para normal.
        let blend = de("blendMode")?
            .and_then(|b| serde_json::from_value(b).ok())
            .unwrap_or_default();

        let mut rest = map.clone();
        for k in COMMON_KEYS {
            rest.remove(*k);
        }
        let rest = Value::Object(rest);
        fn parse<T: serde::de::DeserializeOwned>(v: Value, ty: &str) -> Result<T, String> {
            serde_json::from_value(v).map_err(|e| format!("objeto '{ty}' inválido: {e}"))
        }
        let kind = match ty {
            "path" => ObjectKind::Path(parse(rest, ty)?),
            "rectangle" => ObjectKind::Rectangle(parse(rest, ty)?),
            "ellipse" => ObjectKind::Ellipse(parse(rest, ty)?),
            "polygon" => ObjectKind::Polygon(parse(rest, ty)?),
            "group" => ObjectKind::Group(parse(rest, ty)?),
            "text" => ObjectKind::Text(parse(rest, ty)?),
            "image" => ObjectKind::Image(parse(rest, ty)?),
            "dimension" => ObjectKind::Dimension(parse(rest, ty)?),
            "basic" => ObjectKind::Basic(parse(rest, ty)?),
            "connector" => ObjectKind::Connector(parse(rest, ty)?),
            "bspline" => ObjectKind::BSpline(parse(rest, ty)?),
            _ => ObjectKind::Unknown {
                raw: v.clone(),
                error: None,
            },
        };
        Ok(Object {
            id,
            name,
            transform,
            fill,
            stroke,
            opacity,
            visible,
            locked,
            printable,
            user_data,
            blend,
            kind,
        })
    }

    pub fn to_value(&self) -> Value {
        if let ObjectKind::Unknown { raw, .. } = &self.kind {
            return raw.clone();
        }
        let mut m = Map::new();
        m.insert("id".into(), self.id.clone().into());
        m.insert("type".into(), self.type_name().into());
        if let Some(n) = &self.name {
            m.insert("name".into(), n.clone().into());
        }
        m.insert(
            "transform".into(),
            serde_json::to_value(self.transform).unwrap(),
        );
        if let Some(f) = &self.fill {
            m.insert("fill".into(), f.to_value());
        }
        if let Some(s) = &self.stroke {
            m.insert("stroke".into(), serde_json::to_value(s).unwrap());
        }
        if let Some(o) = self.opacity {
            m.insert("opacity".into(), o.into());
        }
        m.insert("visible".into(), self.visible.into());
        m.insert("locked".into(), self.locked.into());
        if let Some(p) = self.printable {
            m.insert("printable".into(), p.into());
        }
        if let Some(u) = &self.user_data {
            m.insert("userData".into(), serde_json::to_value(u).unwrap());
        }
        if self.blend != BlendMode::Normal {
            m.insert(
                "blendMode".into(),
                serde_json::to_value(self.blend).unwrap(),
            );
        }
        let kind = match &self.kind {
            ObjectKind::Path(d) => serde_json::to_value(d),
            ObjectKind::Rectangle(d) => serde_json::to_value(d),
            ObjectKind::Ellipse(d) => serde_json::to_value(d),
            ObjectKind::Polygon(d) => serde_json::to_value(d),
            ObjectKind::Group(d) => serde_json::to_value(d),
            ObjectKind::Text(d) => serde_json::to_value(d),
            ObjectKind::Image(d) => serde_json::to_value(d),
            ObjectKind::Dimension(d) => serde_json::to_value(d),
            ObjectKind::Basic(d) => serde_json::to_value(d),
            ObjectKind::Connector(d) => serde_json::to_value(d),
            ObjectKind::BSpline(d) => serde_json::to_value(d),
            ObjectKind::Unknown { .. } => unreachable!(),
        };
        if let Ok(Value::Object(k)) = kind {
            for (key, val) in k {
                m.entry(key).or_insert(val);
            }
        }
        Value::Object(m)
    }
}

impl Serialize for Object {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.to_value().serialize(s)
    }
}

impl<'de> Deserialize<'de> for Object {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(Object::from_value(Value::deserialize(d)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn unknown_fields_and_types_are_preserved() {
        let v = json!({
            "id": "0192a000-0000-7000-8000-000000000001", "type": "rectangle",
            "transform": [1,0,0,1,0,0], "visible": true, "locked": false,
            "width": 1000.0, "height": 2000.0, "futureField": {"x": 1}
        });
        let o = Object::from_value(v.clone());
        assert!(matches!(o.kind, ObjectKind::Rectangle(_)));
        assert_eq!(
            o.to_value(),
            json!({
                "id": "0192a000-0000-7000-8000-000000000001", "type": "rectangle",
                "transform": [1.0,0.0,0.0,1.0,0.0,0.0], "visible": true, "locked": false,
                "width": 1000.0, "height": 2000.0, "futureField": {"x": 1}
            })
        );

        let alien = json!({"id": "x", "type": "hologram", "beams": 3});
        assert_eq!(Object::from_value(alien.clone()).to_value(), alien);
    }

    #[test]
    fn invalid_object_becomes_unknown_with_error() {
        let bad = json!({"id": "x", "type": "rectangle", "width": "largo"});
        let o = Object::from_value(bad.clone());
        match &o.kind {
            ObjectKind::Unknown { error: Some(e), .. } => assert!(e.contains("rectangle")),
            other => panic!("esperado Unknown, veio {other:?}"),
        }
        assert_eq!(o.to_value(), bad);
    }
}
