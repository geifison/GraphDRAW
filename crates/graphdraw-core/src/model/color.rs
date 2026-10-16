//! Cor (GD-GDR-07, GD-COR-02, GD-COR-03).
//!
//! A cor guarda sempre o modelo e os valores originais. Conversões para
//! exibição são feitas sob demanda e nunca gravadas de volta.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Cor no formato `{ "model", "values", "name?", "ref?", "alternate?" }`.
///
/// Faixas de `values` por modelo:
/// - `rgb`: 3 componentes 0–1
/// - `cmyk`: 4 componentes 0–1
/// - `gray`: 1 componente 0–1 (0 = preto)
/// - `lab`: L* 0–100, a* e b* −128–127 (valores CIE)
/// - `spot`: 1 componente (tinta 0–1); `name` = nome da tinta; `alternate` = cor de prévia
/// - `registration`: 1 componente (tinta 0–1)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Color {
    pub model: String,
    pub values: Vec<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, rename = "ref", skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alternate: Option<Box<Color>>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Color {
    pub fn rgb(r: f64, g: f64, b: f64) -> Color {
        Color::new("rgb", vec![r, g, b])
    }
    pub fn cmyk(c: f64, m: f64, y: f64, k: f64) -> Color {
        Color::new("cmyk", vec![c, m, y, k])
    }
    pub fn gray(v: f64) -> Color {
        Color::new("gray", vec![v])
    }
    pub fn black() -> Color {
        Color::rgb(0.0, 0.0, 0.0)
    }
    pub fn new(model: &str, values: Vec<f64>) -> Color {
        Color {
            model: model.into(),
            values,
            name: None,
            reference: None,
            alternate: None,
            extra: Map::new(),
        }
    }

    /// Lê `#rrggbb` (forma usada pela librevenge).
    pub fn from_hex(hex: &str) -> Option<Color> {
        let h = hex.trim().strip_prefix('#')?;
        if h.len() != 6 {
            return None;
        }
        let v = u32::from_str_radix(h, 16).ok()?;
        Some(Color::rgb(
            ((v >> 16) & 0xff) as f64 / 255.0,
            ((v >> 8) & 0xff) as f64 / 255.0,
            (v & 0xff) as f64 / 255.0,
        ))
    }

    fn v(&self, i: usize) -> f64 {
        self.values.get(i).copied().unwrap_or(0.0).clamp(0.0, 1.0)
    }

    /// Conversão para sRGB 0–1 usada apenas para exibição/raster.
    ///
    /// M0 usa fórmulas simples (sem ICC); o gerenciamento de cor com
    /// LittleCMS (GD-CMS-01) entra no M2.
    pub fn to_srgb(&self) -> [f64; 3] {
        match self.model.as_str() {
            "rgb" => [self.v(0), self.v(1), self.v(2)],
            "cmyk" => {
                let k = self.v(3);
                [
                    (1.0 - self.v(0)) * (1.0 - k),
                    (1.0 - self.v(1)) * (1.0 - k),
                    (1.0 - self.v(2)) * (1.0 - k),
                ]
            }
            "gray" => {
                let g = self.v(0);
                [g, g, g]
            }
            "registration" => {
                let t = 1.0 - self.v(0);
                [t, t, t]
            }
            "spot" => {
                let tint = self.values.first().copied().unwrap_or(1.0).clamp(0.0, 1.0);
                let base = self
                    .alternate
                    .as_ref()
                    .map(|a| a.to_srgb())
                    .unwrap_or([0.0, 0.0, 0.0]);
                [
                    1.0 - tint * (1.0 - base[0]),
                    1.0 - tint * (1.0 - base[1]),
                    1.0 - tint * (1.0 - base[2]),
                ]
            }
            "lab" => lab_to_srgb(
                self.values.first().copied().unwrap_or(0.0),
                self.values.get(1).copied().unwrap_or(0.0),
                self.values.get(2).copied().unwrap_or(0.0),
            ),
            _ => [0.0, 0.0, 0.0],
        }
    }

    /// `#rrggbb` para SVG.
    pub fn to_hex(&self) -> String {
        let [r, g, b] = self.to_srgb();
        let c = |x: f64| (x.clamp(0.0, 1.0) * 255.0).round() as u8;
        format!("#{:02x}{:02x}{:02x}", c(r), c(g), c(b))
    }

    pub fn is_known_model(&self) -> bool {
        matches!(
            self.model.as_str(),
            "rgb" | "cmyk" | "gray" | "lab" | "spot" | "registration"
        )
    }
}

/// CIE L*a*b* (D50) → sRGB, com adaptação de Bradford para D65.
fn lab_to_srgb(l: f64, a: f64, b: f64) -> [f64; 3] {
    let fy = (l + 16.0) / 116.0;
    let fx = fy + a / 500.0;
    let fz = fy - b / 200.0;
    let finv = |t: f64| {
        if t.powi(3) > 0.008856 {
            t.powi(3)
        } else {
            (t - 16.0 / 116.0) / 7.787
        }
    };
    let (x, y, z) = (0.9642 * finv(fx), finv(fy), 0.8249 * finv(fz));
    // D50 → sRGB linear (matriz já com adaptação cromática de Bradford).
    let r = 3.1338561 * x - 1.6168667 * y - 0.4906146 * z;
    let g = -0.9787684 * x + 1.9161415 * y + 0.0334540 * z;
    let bl = 0.0719453 * x - 0.2289914 * y + 1.4052427 * z;
    let gamma = |c: f64| {
        let c = c.clamp(0.0, 1.0);
        if c <= 0.0031308 {
            12.92 * c
        } else {
            1.055 * c.powf(1.0 / 2.4) - 0.055
        }
    };
    [gamma(r), gamma(g), gamma(bl)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_roundtrip() {
        let c = Color::from_hex("#1b1918").unwrap();
        assert_eq!(c.model, "rgb");
        assert_eq!(c.to_hex(), "#1b1918");
    }

    /// GD-COR-02: CMYK mantém modelo e valores; conversão é só para exibição.
    #[test]
    fn gd_cor_02_cmyk_preserved() {
        let c = Color::cmyk(0.0, 1.0, 1.0, 0.0);
        let json = serde_json::to_value(&c).unwrap();
        assert_eq!(json["model"], "cmyk");
        assert_eq!(json["values"], serde_json::json!([0.0, 1.0, 1.0, 0.0]));
        assert_eq!(c.to_hex(), "#ff0000");
    }

    #[test]
    fn lab_white_is_white() {
        let c = Color::new("lab", vec![100.0, 0.0, 0.0]);
        let [r, g, b] = c.to_srgb();
        assert!(r > 0.98 && g > 0.98 && b > 0.98);
    }
}
