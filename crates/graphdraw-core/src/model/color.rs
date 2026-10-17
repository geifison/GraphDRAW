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
/// - `hsb`: matiz, saturação, brilho, 0–1 (matiz 1 = 360°)
/// - `hls`: matiz, luminosidade, saturação, 0–1
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
    /// Com um perfil CMYK ativo no documento (GD-CMS-01), CMYK passa pelo
    /// LittleCMS; sem perfil, fórmulas simples.
    pub fn to_srgb(&self) -> [f64; 3] {
        match self.model.as_str() {
            "rgb" => {
                let rgb = [self.v(0), self.v(1), self.v(2)];
                crate::cms::current().map_or(rgb, |c| c.rgb_to_srgb(rgb))
            }
            "cmyk" => {
                if let Some(c) = crate::cms::current()
                    && let Some(rgb) = c.cmyk_to_srgb([self.v(0), self.v(1), self.v(2), self.v(3)])
                {
                    return rgb;
                }
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
            "hsb" => hsb_to_rgb(self.v(0), self.v(1), self.v(2)),
            "hls" => hls_to_rgb(self.v(0), self.v(1), self.v(2)),
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
            "rgb" | "cmyk" | "gray" | "lab" | "hsb" | "hls" | "spot" | "registration"
        )
    }
}

/// HSB/HSV (0–1) → RGB (0–1).
pub fn hsb_to_rgb(h: f64, s: f64, b: f64) -> [f64; 3] {
    let h6 = (h.rem_euclid(1.0)) * 6.0;
    let c = b * s;
    let x = c * (1.0 - (h6 % 2.0 - 1.0).abs());
    let m = b - c;
    let (r, g, bl) = match h6 as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    [r + m, g + m, bl + m]
}

/// RGB (0–1) → HSB (0–1).
pub fn rgb_to_hsb([r, g, b]: [f64; 3]) -> [f64; 3] {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;
    let h = if d == 0.0 {
        0.0
    } else if max == r {
        ((g - b) / d).rem_euclid(6.0) / 6.0
    } else if max == g {
        ((b - r) / d + 2.0) / 6.0
    } else {
        ((r - g) / d + 4.0) / 6.0
    };
    let s = if max == 0.0 { 0.0 } else { d / max };
    [h, s, max]
}

/// HLS (0–1) → RGB (0–1).
pub fn hls_to_rgb(h: f64, l: f64, s: f64) -> [f64; 3] {
    let b = l + s * l.min(1.0 - l);
    let sv = if b == 0.0 { 0.0 } else { 2.0 * (1.0 - l / b) };
    hsb_to_rgb(h, sv, b)
}

/// RGB (0–1) → HLS (0–1).
pub fn rgb_to_hls(rgb: [f64; 3]) -> [f64; 3] {
    let [h, sv, b] = rgb_to_hsb(rgb);
    let l = b * (1.0 - sv / 2.0);
    let s = if l == 0.0 || l == 1.0 {
        0.0
    } else {
        (b - l) / l.min(1.0 - l)
    };
    [h, l, s]
}

/// sRGB (0–1) → CIE L*a*b* (D50), inverso de [`lab_to_srgb`].
pub fn srgb_to_lab([r, g, b]: [f64; 3]) -> [f64; 3] {
    let lin = |c: f64| {
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    let (r, g, b) = (lin(r), lin(g), lin(b));
    // sRGB linear → XYZ D50 (matriz com adaptação de Bradford).
    let x = 0.4360747 * r + 0.3850649 * g + 0.1430804 * b;
    let y = 0.2225045 * r + 0.7168786 * g + 0.0606169 * b;
    let z = 0.0139322 * r + 0.0971045 * g + 0.7141733 * b;
    let (xn, yn, zn) = (0.96422, 1.0, 0.82521);
    let f = |t: f64| {
        if t > 0.008856 {
            t.cbrt()
        } else {
            7.787 * t + 16.0 / 116.0
        }
    };
    let (fx, fy, fz) = (f(x / xn), f(y / yn), f(z / zn));
    [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
}

/// CIE L*a*b* (D50) → sRGB, com adaptação de Bradford para D65.
pub fn lab_to_srgb(l: f64, a: f64, b: f64) -> [f64; 3] {
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
    fn gd_cor_01_hsb_hls_lab_round_trip() {
        for rgb in [
            [1.0, 0.0, 0.0],
            [0.2, 0.6, 0.4],
            [0.9, 0.8, 0.1],
            [0.5, 0.5, 0.5],
        ] {
            let near =
                |a: [f64; 3], b: [f64; 3], tol: f64| (0..3).all(|i| (a[i] - b[i]).abs() < tol);
            let [h, s, b] = rgb_to_hsb(rgb);
            assert!(near(hsb_to_rgb(h, s, b), rgb, 1e-9), "{rgb:?}");
            let [h, l, s] = rgb_to_hls(rgb);
            assert!(near(hls_to_rgb(h, l, s), rgb, 1e-9), "{rgb:?}");
            let [l, a, bb] = srgb_to_lab(rgb);
            assert!(near(lab_to_srgb(l, a, bb), rgb, 2e-3), "{rgb:?}");
        }
        // Modelo original preservado: HSB guarda HSB, exibe RGB.
        let c = Color::new("hsb", vec![1.0 / 3.0, 1.0, 1.0]);
        let [r, g, b] = c.to_srgb();
        assert!(r < 1e-9 && (g - 1.0).abs() < 1e-9 && b < 1e-9);
        assert!(c.is_known_model());
    }

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
