//! Unidades (GD-DOC-01). A unidade interna é o micrômetro (µm) em `f64`.
//! Conversões só acontecem na borda: UI, importação e exportação.

/// Micrômetros por polegada.
pub const UM_PER_INCH: f64 = 25_400.0;
/// Micrômetros por milímetro.
pub const UM_PER_MM: f64 = 1_000.0;
/// Micrômetros por ponto tipográfico (1/72 pol).
pub const UM_PER_PT: f64 = UM_PER_INCH / 72.0;

/// Unidades de exibição suportadas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    Micrometer,
    Millimeter,
    Centimeter,
    Inch,
    Point,
    Pica,
    /// Pixel a 96 ppp.
    Pixel,
}

impl Unit {
    /// Quantos micrômetros vale uma unidade.
    pub fn um(self) -> f64 {
        match self {
            Unit::Micrometer => 1.0,
            Unit::Millimeter => UM_PER_MM,
            Unit::Centimeter => 10.0 * UM_PER_MM,
            Unit::Inch => UM_PER_INCH,
            Unit::Point => UM_PER_PT,
            Unit::Pica => 12.0 * UM_PER_PT,
            Unit::Pixel => UM_PER_INCH / 96.0,
        }
    }

    pub fn from_name(name: &str) -> Option<Unit> {
        Some(match name {
            "um" | "µm" => Unit::Micrometer,
            "mm" => Unit::Millimeter,
            "cm" => Unit::Centimeter,
            "in" | "pol" => Unit::Inch,
            "pt" => Unit::Point,
            "pc" | "pica" => Unit::Pica,
            "px" => Unit::Pixel,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Unit::Micrometer => "µm",
            Unit::Millimeter => "mm",
            Unit::Centimeter => "cm",
            Unit::Inch => "in",
            Unit::Point => "pt",
            Unit::Pica => "pc",
            Unit::Pixel => "px",
        }
    }
}

/// Converte um valor numa unidade para micrômetros.
pub fn to_um(value: f64, unit: Unit) -> f64 {
    value * unit.um()
}

/// Converte micrômetros para uma unidade de exibição.
pub fn from_um(value_um: f64, unit: Unit) -> f64 {
    value_um / unit.um()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Aceite GD-DOC-01: retângulo de 10 mm → 10000 µm; exibido 10 mm, 0,3937 pol, 28,35 pt.
    #[test]
    fn gd_doc_01_conversions() {
        let w = to_um(10.0, Unit::Millimeter);
        assert_eq!(w, 10_000.0);
        assert!((from_um(w, Unit::Millimeter) - 10.0).abs() < 1e-12);
        assert!((from_um(w, Unit::Inch) - 0.3937).abs() < 5e-5);
        assert!((from_um(w, Unit::Point) - 28.35).abs() < 5e-3);
    }
}
