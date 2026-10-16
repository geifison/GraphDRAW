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

/// Avalia uma expressão de comprimento (GD-TRF-04, GD-UI-11) e devolve µm.
///
/// Aceita números com vírgula ou ponto decimal, unidades por termo
/// (`mm`, `cm`, `in`, `pt`, `pc`, `px`, `um`/`µm`; sem unidade =
/// `default_unit`), porcentagem de `reference_um` (`50%`), `+ - * /` e
/// parênteses. Ex.: `10+5`, `2in`, `50%`, `(10mm+1cm)/2`.
pub fn parse_length(expr: &str, default_unit: Unit, reference_um: Option<f64>) -> Option<f64> {
    let mut p = ExprParser {
        s: expr.trim().as_bytes(),
        i: 0,
        unit: default_unit,
        reference: reference_um,
    };
    let v = p.sum(true)?;
    p.ws();
    (p.i == p.s.len() && v.is_finite()).then_some(v)
}

/// Avalia uma expressão numérica sem unidade (ângulos, porcentagens de escala).
pub fn parse_number(expr: &str) -> Option<f64> {
    let mut p = ExprParser {
        s: expr.trim().as_bytes(),
        i: 0,
        unit: Unit::Micrometer,
        reference: None,
    };
    let v = p.sum(false)?;
    p.ws();
    (p.i == p.s.len() && v.is_finite()).then_some(v)
}

struct ExprParser<'a> {
    s: &'a [u8],
    i: usize,
    unit: Unit,
    reference: Option<f64>,
}

impl ExprParser<'_> {
    fn ws(&mut self) {
        while self.i < self.s.len() && self.s[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }
    fn eat(&mut self, c: u8) -> bool {
        self.ws();
        if self.s.get(self.i) == Some(&c) {
            self.i += 1;
            true
        } else {
            false
        }
    }
    /// `lengths`: termos são comprimentos (com unidade); senão, números puros.
    fn sum(&mut self, lengths: bool) -> Option<f64> {
        let mut v = self.product(lengths)?;
        loop {
            if self.eat(b'+') {
                v += self.product(lengths)?;
            } else if self.eat(b'-') {
                v -= self.product(lengths)?;
            } else {
                return Some(v);
            }
        }
    }
    /// Em produtos, só o primeiro fator carrega unidade; os demais são escalares.
    fn product(&mut self, lengths: bool) -> Option<f64> {
        let mut v = self.atom(lengths)?;
        loop {
            if self.eat(b'*') {
                v *= self.atom(false)?;
            } else if self.eat(b'/') {
                let d = self.atom(false)?;
                if d == 0.0 {
                    return None;
                }
                v /= d;
            } else {
                return Some(v);
            }
        }
    }
    fn atom(&mut self, lengths: bool) -> Option<f64> {
        self.ws();
        if self.eat(b'-') {
            return Some(-self.atom(lengths)?);
        }
        if self.eat(b'(') {
            let v = self.sum(lengths)?;
            return self.eat(b')').then_some(v);
        }
        let start = self.i;
        while self.i < self.s.len()
            && (self.s[self.i].is_ascii_digit() || matches!(self.s[self.i], b'.' | b','))
        {
            self.i += 1;
        }
        let num: f64 = std::str::from_utf8(&self.s[start..self.i])
            .ok()?
            .replace(',', ".")
            .parse()
            .ok()?;
        self.ws();
        if self.eat(b'%') {
            return Some(if lengths {
                num / 100.0 * self.reference?
            } else {
                num
            });
        }
        if !lengths {
            return Some(num);
        }
        let ustart = self.i;
        while self.i < self.s.len()
            && !self.s[self.i].is_ascii_digit()
            && !b"+-*/() ".contains(&self.s[self.i])
        {
            self.i += 1;
        }
        let name = std::str::from_utf8(&self.s[ustart..self.i]).ok()?;
        let unit = if name.is_empty() {
            self.unit
        } else {
            Unit::from_name(name)?
        };
        Some(to_um(num, unit))
    }
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

    /// GD-TRF-04: expressões e unidades na entrada numérica.
    #[test]
    fn gd_trf_04_expressions() {
        let mm = Unit::Millimeter;
        assert_eq!(parse_length("10+5", mm, None), Some(15_000.0));
        assert_eq!(parse_length("2in", mm, None), Some(50_800.0));
        assert_eq!(parse_length("50%", mm, Some(80_000.0)), Some(40_000.0));
        assert_eq!(parse_length("10,5", mm, None), Some(10_500.0));
        assert_eq!(parse_length("(10mm + 1cm) / 2", mm, None), Some(10_000.0));
        assert_eq!(parse_length("3 * 2", mm, None), Some(6_000.0));
        assert_eq!(parse_length("-5", mm, None), Some(-5_000.0));
        assert!((parse_length("72pt", mm, None).unwrap() - 25_400.0).abs() < 1e-6);
        assert_eq!(parse_length("1/0", mm, None), None);
        assert_eq!(parse_length("10 parsecs", mm, None), None);
        assert_eq!(parse_length("50%", mm, None), None);
        assert_eq!(parse_number("30*2+0,5"), Some(60.5));
    }
}
