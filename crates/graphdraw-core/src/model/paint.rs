//! Preenchimento e contorno (doc 04: GD-FIL-01..03, GD-OUT-01..04, GD-OUT-07).

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};

use super::color::Color;

/// Parada de gradiente. `offset` 0–1; `opacity` 0–1.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GradientStop {
    pub offset: f64,
    pub color: Color,
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub opacity: f64,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Gradiente em coordenadas locais do objeto (µm).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Gradient {
    /// `linear` (usa `start`/`end`), `radial` (usa `center`/`radius`) ou
    /// `elliptic`, `conic`, `rectangular` (usam `center`, `radius`,
    /// `radiusY` e `angle`; ver `render::gradient`).
    pub kind: String,
    pub stops: Vec<GradientStop>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<[f64; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end: Option<[f64; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub center: Option<[f64; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub radius: Option<f64>,
    /// Raio em y (elíptico, retangular); padrão = `radius`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub radius_y: Option<f64>,
    /// Giro dos eixos em graus (elíptico, cônico, retangular).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub angle: Option<f64>,
    /// `pad` (padrão), `repeat` ou `reflect`.
    #[serde(default = "pad", skip_serializing_if = "is_pad")]
    pub spread: String,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Preenchimento. Tipos não reconhecidos são preservados como JSON bruto
/// (GD-GDR-03) e tratados como "nenhum" na renderização.
#[derive(Debug, Clone, PartialEq)]
pub enum Fill {
    None,
    /// `opacity` = transparência uniforme do preenchimento (GD-TRA-01), 0–1.
    Solid {
        color: Color,
        opacity: f64,
        overprint: bool,
        extra: Map<String, Value>,
    },
    Gradient(Gradient),
    Unknown(Value),
}

impl Fill {
    pub fn solid(color: Color) -> Fill {
        Fill::Solid {
            color,
            opacity: 1.0,
            overprint: false,
            extra: Map::new(),
        }
    }

    pub fn from_value(v: Value) -> Fill {
        let ty = v
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        match ty.as_str() {
            "none" => Fill::None,
            "solid" => {
                let Value::Object(mut map) = v.clone() else {
                    return Fill::Unknown(v);
                };
                map.remove("type");
                let overprint = map
                    .remove("overprint")
                    .and_then(|b| b.as_bool())
                    .unwrap_or(false);
                let opacity = map
                    .remove("opacity")
                    .and_then(|o| o.as_f64())
                    .unwrap_or(1.0);
                match map.remove("color").map(serde_json::from_value::<Color>) {
                    Some(Ok(color)) => Fill::Solid {
                        color,
                        opacity,
                        overprint,
                        extra: map,
                    },
                    _ => Fill::Unknown(v),
                }
            }
            "gradient" => {
                let Value::Object(mut map) = v.clone() else {
                    return Fill::Unknown(v);
                };
                map.remove("type");
                match serde_json::from_value::<Gradient>(Value::Object(map)) {
                    Ok(g) => Fill::Gradient(g),
                    Err(_) => Fill::Unknown(v),
                }
            }
            _ => Fill::Unknown(v),
        }
    }

    pub fn to_value(&self) -> Value {
        match self {
            Fill::None => serde_json::json!({ "type": "none" }),
            Fill::Solid {
                color,
                opacity,
                overprint,
                extra,
            } => {
                let mut m = Map::new();
                m.insert("type".into(), "solid".into());
                m.insert(
                    "color".into(),
                    serde_json::to_value(color).unwrap_or(Value::Null),
                );
                if *opacity != 1.0 {
                    m.insert("opacity".into(), (*opacity).into());
                }
                if *overprint {
                    m.insert("overprint".into(), true.into());
                }
                for (k, v) in extra {
                    m.entry(k.clone()).or_insert_with(|| v.clone());
                }
                Value::Object(m)
            }
            Fill::Gradient(g) => {
                let mut v = serde_json::to_value(g).unwrap_or(Value::Null);
                if let Value::Object(m) = &mut v {
                    m.insert("type".into(), "gradient".into());
                }
                v
            }
            Fill::Unknown(v) => v.clone(),
        }
    }
}

impl Serialize for Fill {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.to_value().serialize(s)
    }
}

impl<'de> Deserialize<'de> for Fill {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(Fill::from_value(Value::deserialize(d)?))
    }
}

/// Contorno (GD-OUT-01..04, 07). `width` em µm; 0 = fio de cabelo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stroke {
    pub width: f64,
    pub color: Color,
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub opacity: f64,
    /// Padrão traço/espaço em µm.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dash: Option<Vec<f64>>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub dash_offset: f64,
    /// `miter`, `round`, `bevel`.
    #[serde(default = "miter", skip_serializing_if = "is_miter")]
    pub join: String,
    #[serde(default = "four", skip_serializing_if = "is_four")]
    pub miter_limit: f64,
    /// `butt`, `round`, `square`.
    #[serde(default = "butt", skip_serializing_if = "is_butt")]
    pub cap: String,
    /// `center`, `inside`, `outside`.
    #[serde(default = "center", skip_serializing_if = "is_center")]
    pub position: String,
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub scale_with_object: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub behind_fill: bool,
    /// Pontas de seta no início e no fim de caminhos abertos (GD-OUT-05).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_arrow: Option<Arrow>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_arrow: Option<Arrow>,
    /// Caneta caligráfica (GD-OUT-06): largura do bico (0–1 da espessura,
    /// 1 = bico redondo, o contorno comum) e ângulo do bico em graus.
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub nib_stretch: f64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub nib_angle: f64,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Ponta de seta (GD-OUT-05). `shape` é uma forma da biblioteca própria
/// (`triangle`, `open`, `barbed`, `circle`, `square`, `bar`);
/// `scale` multiplica o tamanho padrão (4 × a espessura), `offset` (µm)
/// afasta a ponta da extremidade, `rotation` em graus, `mirror` espelha.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Arrow {
    pub shape: String,
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub scale: f64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub offset: f64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub rotation: f64,
    #[serde(default, skip_serializing_if = "is_false")]
    pub mirror: bool,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Arrow {
    pub fn new(shape: &str) -> Arrow {
        Arrow {
            shape: shape.into(),
            scale: 1.0,
            offset: 0.0,
            rotation: 0.0,
            mirror: false,
            extra: Map::new(),
        }
    }
}

impl Stroke {
    pub fn new(width: f64, color: Color) -> Stroke {
        Stroke {
            width,
            color,
            opacity: 1.0,
            dash: None,
            dash_offset: 0.0,
            join: miter(),
            miter_limit: 4.0,
            cap: butt(),
            position: center(),
            scale_with_object: true,
            behind_fill: false,
            start_arrow: None,
            end_arrow: None,
            nib_stretch: 1.0,
            nib_angle: 0.0,
            extra: Map::new(),
        }
    }
}

/// Espessura usada para "fio de cabelo" na tela/raster: 0,1 mm.
pub const HAIRLINE_UM: f64 = 100.0;

fn one() -> f64 {
    1.0
}
fn is_one(v: &f64) -> bool {
    *v == 1.0
}
fn is_zero(v: &f64) -> bool {
    *v == 0.0
}
fn four() -> f64 {
    4.0
}
fn is_four(v: &f64) -> bool {
    *v == 4.0
}
fn pad() -> String {
    "pad".into()
}
fn is_pad(s: &String) -> bool {
    s == "pad"
}
fn miter() -> String {
    "miter".into()
}
fn is_miter(s: &String) -> bool {
    s == "miter"
}
fn butt() -> String {
    "butt".into()
}
fn is_butt(s: &String) -> bool {
    s == "butt"
}
fn center() -> String {
    "center".into()
}
fn is_center(s: &String) -> bool {
    s == "center"
}
fn yes() -> bool {
    true
}
fn is_true(b: &bool) -> bool {
    *b
}
fn is_false(b: &bool) -> bool {
    !*b
}
