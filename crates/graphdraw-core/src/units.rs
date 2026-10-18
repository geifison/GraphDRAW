//! Unidades (GD-DOC-01). A unidade interna é o micrômetro (µm) em `f64`.
//! Conversões só acontecem na borda: UI, importação e exportação.

/// Micrômetros por polegada.
pub const UM_PER_INCH: f64 = 25_400.0;
/// Micrômetros por milímetro.
pub const UM_PER_MM: f64 = 1_000.0;
/// Micrômetros por ponto tipográfico (1/72 pol).
pub const UM_PER_PT: f64 = UM_PER_INCH / 72.0;
/// Ponto Didot tradicional (≈ 0,376 mm); o cícero vale 12 deles.
pub const UM_PER_DIDOT: f64 = 376.065;
/// Resolução do pixel quando o documento não diz outra (ppp).
pub const DEFAULT_PX_DPI: f64 = 96.0;

/// Unidades de exibição suportadas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    Micrometer,
    Millimeter,
    Centimeter,
    Meter,
    Kilometer,
    Inch,
    Foot,
    Yard,
    Point,
    Pica,
    Didot,
    Cicero,
    /// Pixel: 1/ppp de polegada (resolução do documento; 96 sem ela).
    Pixel,
}

impl Unit {
    /// Unidades de exibição, na ordem dos menus (GD-DOC-01).
    pub const ALL: [Unit; 13] = [
        Unit::Millimeter,
        Unit::Centimeter,
        Unit::Meter,
        Unit::Kilometer,
        Unit::Micrometer,
        Unit::Inch,
        Unit::Foot,
        Unit::Yard,
        Unit::Point,
        Unit::Pica,
        Unit::Didot,
        Unit::Cicero,
        Unit::Pixel,
    ];

    /// Quantos micrômetros vale uma unidade (pixel a 96 ppp; ver [`Unit::um_at`]).
    pub fn um(self) -> f64 {
        self.um_at(DEFAULT_PX_DPI)
    }

    /// Quantos micrômetros vale uma unidade, com o pixel a `dpi` ppp.
    pub fn um_at(self, dpi: f64) -> f64 {
        match self {
            Unit::Micrometer => 1.0,
            Unit::Millimeter => UM_PER_MM,
            Unit::Centimeter => 10.0 * UM_PER_MM,
            Unit::Meter => 1_000.0 * UM_PER_MM,
            Unit::Kilometer => 1_000_000.0 * UM_PER_MM,
            Unit::Inch => UM_PER_INCH,
            Unit::Foot => 12.0 * UM_PER_INCH,
            Unit::Yard => 36.0 * UM_PER_INCH,
            Unit::Point => UM_PER_PT,
            Unit::Pica => 12.0 * UM_PER_PT,
            Unit::Didot => UM_PER_DIDOT,
            Unit::Cicero => 12.0 * UM_PER_DIDOT,
            Unit::Pixel => {
                let dpi = if dpi.is_finite() && dpi > 0.0 {
                    dpi
                } else {
                    DEFAULT_PX_DPI
                };
                UM_PER_INCH / dpi
            }
        }
    }

    /// Nome ou abreviação (sem diferenciar maiúsculas), em português ou inglês.
    pub fn from_name(name: &str) -> Option<Unit> {
        Some(match name.trim().to_lowercase().as_str() {
            "um" | "µm" | "μm" | "micron" | "mícron" => Unit::Micrometer,
            "mm" => Unit::Millimeter,
            "cm" => Unit::Centimeter,
            "m" => Unit::Meter,
            "km" => Unit::Kilometer,
            "in" | "pol" | "\"" | "″" => Unit::Inch,
            "ft" | "pé" | "pe" | "pés" | "pes" | "'" | "′" => Unit::Foot,
            "yd" | "jd" | "jarda" | "jardas" => Unit::Yard,
            "pt" => Unit::Point,
            "pc" | "pica" | "paica" => Unit::Pica,
            "dd" | "didot" => Unit::Didot,
            "cc" | "cícero" | "cicero" => Unit::Cicero,
            "px" => Unit::Pixel,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Unit::Micrometer => "µm",
            Unit::Millimeter => "mm",
            Unit::Centimeter => "cm",
            Unit::Meter => "m",
            Unit::Kilometer => "km",
            Unit::Inch => "in",
            Unit::Foot => "ft",
            Unit::Yard => "yd",
            Unit::Point => "pt",
            Unit::Pica => "pc",
            Unit::Didot => "dd",
            Unit::Cicero => "cc",
            Unit::Pixel => "px",
        }
    }

    /// Unidade do sistema imperial (réguas em frações binárias).
    pub fn imperial(self) -> bool {
        matches!(self, Unit::Inch | Unit::Foot | Unit::Yard)
    }
}

/// Como os comprimentos aparecem e são digitados na interface (GD-DOC-01,
/// GD-DOC-03, GD-UI-11): unidade, resolução do pixel, escala de desenho
/// (mundo ÷ papel) e casas decimais. O documento guarda sempre µm; esta
/// conversão só acontece na borda.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Display {
    pub unit: Unit,
    /// Resolução do documento (ppp) para o pixel.
    pub dpi: f64,
    /// Escala de desenho (mundo ÷ papel); 1 = tamanho real.
    pub scale: f64,
    /// Casas decimais exibidas (0–6); o valor guardado não é arredondado.
    pub decimals: usize,
}

impl Default for Display {
    fn default() -> Self {
        Display {
            unit: Unit::Millimeter,
            dpi: DEFAULT_PX_DPI,
            scale: 1.0,
            decimals: 3,
        }
    }
}

impl Display {
    /// µm por unidade exibida.
    pub fn unit_um(&self) -> f64 {
        self.unit.um_at(self.dpi)
    }

    fn world(&self, world: bool) -> f64 {
        if world && self.scale.is_finite() && self.scale > 0.0 {
            self.scale
        } else {
            1.0
        }
    }

    /// Valor de `um` na unidade (com `world`, medida do mundo pela escala).
    pub fn value(&self, um: f64, world: bool) -> f64 {
        um * self.world(world) / self.unit_um()
    }

    /// Texto de `um` com as casas decimais pedidas, sem zeros à direita e
    /// sem resíduo de ponto flutuante (10 mm nunca vira 9,999999); ponto
    /// decimal (a interface troca pela vírgula).
    pub fn format(&self, um: f64, world: bool) -> String {
        let d = self.decimals.min(6);
        let s = format!("{:.d$}", self.value(um, world));
        let s = if s.contains('.') {
            s.trim_end_matches('0').trim_end_matches('.').to_string()
        } else {
            s
        };
        if s == "-0" { "0".into() } else { s }
    }

    /// Lê o que foi digitado (ver [`parse_input`]) e devolve µm do papel.
    pub fn parse(&self, text: &str, current_um: Option<f64>, world: bool) -> Option<f64> {
        let k = self.world(world);
        let cur = current_um.map(|c| c * k);
        parse_input(text, self.unit, self.dpi, cur).map(|v| v / k)
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

/// Entrada de um campo de comprimento (GD-UI-11): como [`parse_length`]
/// (o pixel a `dpi`), e mais:
/// - começando por operador, a conta é sobre o valor atual: `+15`, `-5mm`,
///   `*2`, `/3`, `+20%` (soma 20% dele); `50%` sozinho = metade dele;
/// - `=` no início força valor absoluto (`=-5` para negativo);
/// - frações mistas: `3 1/8 pol` = 3,125 pol.
pub fn parse_input(expr: &str, unit: Unit, dpi: f64, current_um: Option<f64>) -> Option<f64> {
    let t = expr.trim();
    if let Some(abs) = t.strip_prefix('=') {
        return parse_length_dpi(abs, unit, dpi, current_um);
    }
    let Some(cur) = current_um else {
        return parse_length_dpi(t, unit, dpi, None);
    };
    let op = t.chars().next()?;
    let rest = t[op.len_utf8()..].trim();
    match op {
        '+' => Some(cur + parse_length_dpi(rest, unit, dpi, Some(cur))?),
        '-' => Some(cur - parse_length_dpi(rest, unit, dpi, Some(cur))?),
        '*' | '×' => Some(cur * parse_number(rest)?),
        '/' | '÷' => {
            let d = parse_number(rest)?;
            (d != 0.0).then(|| cur / d)
        }
        _ => parse_length_dpi(t, unit, dpi, Some(cur)),
    }
}

/// Como [`parse_length`], com o pixel a `dpi` ppp.
pub fn parse_length_dpi(
    expr: &str,
    default_unit: Unit,
    dpi: f64,
    reference_um: Option<f64>,
) -> Option<f64> {
    let mut p = ExprParser {
        s: expr.trim().as_bytes(),
        i: 0,
        unit: default_unit,
        reference: reference_um,
        dpi,
        explicit: false,
    };
    let v = p.sum(true)?;
    p.ws();
    (p.i == p.s.len() && v.is_finite()).then_some(v)
}

/// Avalia uma expressão de comprimento (GD-TRF-04, GD-UI-11) e devolve µm.
///
/// Aceita números com vírgula ou ponto decimal, unidades por termo
/// (`mm`, `cm`, `in`, `pt`, `pc`, `px`, `um`/`µm`; sem unidade =
/// `default_unit`), porcentagem de `reference_um` (`50%`), `+ - * /` e
/// parênteses. Ex.: `10+5`, `2in`, `50%`, `(10mm+1cm)/2`.
pub fn parse_length(expr: &str, default_unit: Unit, reference_um: Option<f64>) -> Option<f64> {
    parse_length_dpi(expr, default_unit, DEFAULT_PX_DPI, reference_um)
}

/// Avalia uma expressão numérica sem unidade (ângulos, porcentagens de escala).
pub fn parse_number(expr: &str) -> Option<f64> {
    let mut p = ExprParser {
        s: expr.trim().as_bytes(),
        i: 0,
        unit: Unit::Micrometer,
        reference: None,
        dpi: DEFAULT_PX_DPI,
        explicit: false,
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
    dpi: f64,
    /// O último comprimento lido trouxe unidade escrita.
    explicit: bool,
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
        self.explicit = false;
        let mut v = self.atom(lengths)?;
        let explicit = self.explicit;
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
            // Unidade no fim da conta vale para ela toda ("1/2in").
            if lengths
                && !explicit
                && let Some(u) = self.unit_name()
            {
                v = v / self.unit.um_at(self.dpi) * u.um_at(self.dpi);
            }
        }
    }
    /// Lê um nome de unidade logo adiante (sem consumir se não for unidade).
    fn unit_name(&mut self) -> Option<Unit> {
        let save = self.i;
        self.ws();
        let st = self.i;
        while self.i < self.s.len()
            && !self.s[self.i].is_ascii_digit()
            && !b"+-*/() ".contains(&self.s[self.i])
        {
            self.i += 1;
        }
        let u = std::str::from_utf8(&self.s[st..self.i])
            .ok()
            .filter(|n| !n.is_empty())
            .and_then(Unit::from_name);
        if u.is_none() {
            self.i = save;
        }
        u
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
        let lit = std::str::from_utf8(&self.s[start..self.i]).ok()?;
        let mut num: f64 = lit.replace(',', ".").parse().ok()?;
        // Fração mista: "3 1/8" (inteiro, espaço, a/b).
        if !lit.contains(['.', ',']) {
            let save = self.i;
            self.ws();
            let digits = |p: &mut Self| {
                let st = p.i;
                while p.i < p.s.len() && p.s[p.i].is_ascii_digit() {
                    p.i += 1;
                }
                std::str::from_utf8(&p.s[st..p.i]).ok()?.parse::<f64>().ok()
            };
            match (self.i > save, digits(self)) {
                (true, Some(a)) if self.s.get(self.i) == Some(&b'/') => {
                    self.i += 1;
                    match digits(self) {
                        Some(b) if b > 0.0 => num += a / b,
                        _ => self.i = save,
                    }
                }
                _ => self.i = save,
            }
        }
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
            self.explicit = true;
            Unit::from_name(name)?
        };
        Some(num * unit.um_at(self.dpi))
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

    fn near(a: Option<f64>, b: f64) -> bool {
        a.is_some_and(|a| (a - b).abs() < 1e-6)
    }

    /// GD-DOC-01: todas as famílias de unidades, com fatores exatos.
    #[test]
    fn gd_doc_01_unit_families() {
        let mm = Unit::Millimeter;
        assert!(near(parse_length("1m", mm, None), 1_000_000.0));
        assert!(near(parse_length("0,5 km", mm, None), 500_000_000.0));
        assert!(near(parse_length("1yd", mm, None), 914_400.0));
        assert!(near(parse_length("1ft", mm, None), 304_800.0));
        assert!(near(parse_length("1 pol", mm, None), 25_400.0));
        assert!(near(parse_length("2\"", mm, None), 50_800.0));
        assert!(near(parse_length("1pc", mm, None), 12.0 * UM_PER_PT));
        assert!(near(parse_length("1 Didot", mm, None), UM_PER_DIDOT));
        assert!(near(parse_length("1cc", mm, None), 12.0 * UM_PER_DIDOT));
        assert!(near(
            parse_length("300px", mm, None),
            300.0 * 25_400.0 / 96.0
        ));
        for u in Unit::ALL {
            assert_eq!(Unit::from_name(u.name()), Some(u), "{u:?}");
        }
    }

    /// GD-UI-11: unidades misturadas na mesma conta (aceite do pedido:
    /// 120mm + 2pol - 15pt = 165,511 mm).
    #[test]
    fn gd_ui_11_mixed_units() {
        let v = parse_length("120mm + 2pol - 15pt", Unit::Millimeter, None).unwrap();
        assert!((v / 1000.0 - 165.508).abs() < 0.005, "{}", v / 1000.0);
        // Frações mistas da marcenaria: 3 1/8 pol = 79,375 mm.
        assert!(near(
            parse_length("3 1/8 pol", Unit::Millimeter, None),
            79_375.0
        ));
        assert!(near(
            parse_length("1/2in", Unit::Millimeter, None),
            12_700.0
        ));
    }

    /// GD-UI-11: operador no início age sobre o valor atual.
    #[test]
    fn gd_ui_11_relative_input() {
        let mm = Unit::Millimeter;
        let cur = Some(100_000.0);
        let p = |t: &str| parse_input(t, mm, 96.0, cur);
        assert!(near(p("+15"), 115_000.0));
        assert!(near(p("-5mm"), 95_000.0));
        assert!(near(p("*2"), 200_000.0));
        assert!(near(p("/3"), 100_000.0 / 3.0));
        assert!(near(p("+20%"), 120_000.0));
        assert!(near(p("50%"), 50_000.0));
        assert!(near(p("=-5"), -5_000.0), "= força valor absoluto");
        assert!(near(p("40"), 40_000.0));
        assert_eq!(p("/0"), None);
        // Sem valor atual, "-5" é negativo.
        assert!(near(parse_input("-5", mm, 96.0, None), -5_000.0));
    }

    /// GD-DOC-01: pixel pela resolução do documento — 25,4 mm = 300 px a
    /// 300 ppp e 150 px a 150 ppp, sem mudar o tamanho real.
    #[test]
    fn gd_doc_01_pixels_follow_dpi() {
        let d300 = Display {
            unit: Unit::Pixel,
            dpi: 300.0,
            ..Default::default()
        };
        let d150 = Display { dpi: 150.0, ..d300 };
        assert_eq!(d300.format(25_400.0, false), "300");
        assert_eq!(d150.format(25_400.0, false), "150");
        assert!(near(d300.parse("600", None, false), 50_800.0));
    }

    /// GD-UI-16: casas decimais só na exibição; ida e volta sem resíduo.
    #[test]
    fn gd_ui_16_decimals_and_no_drift() {
        let mut d = Display::default();
        assert_eq!(d.format(125.67, false), "0.126");
        d.decimals = 5;
        assert_eq!(d.format(125.67, false), "0.12567");
        d.decimals = 0;
        assert_eq!(d.format(125.67, false), "0");
        // Trocar a unidade só muda a exibição: o valor guardado não passa
        // por conversões repetidas (10 mm continua 10 mm).
        let um = 10_000.0;
        let mm = Display {
            decimals: 6,
            ..Default::default()
        };
        for u in Unit::ALL {
            let shown = Display {
                unit: u,
                decimals: 6,
                ..Default::default()
            };
            let _ = shown.format(um, false);
        }
        assert_eq!(mm.format(um, false), "10");
        // Resíduo binário não aparece: 9,9999999999 e 0,1 + 0,2.
        assert_eq!(mm.format(9_999.999_999_9, false), "10");
        assert_eq!(mm.format(100.0 + 200.0, false), "0.3");
        let inch = Display {
            unit: Unit::Inch,
            ..Default::default()
        };
        assert_eq!(inch.format(254_000.0, false), "10");
    }

    /// GD-DOC-03: com escala 1:10, 1000 mm de papel aparecem como 10 m e
    /// digitar 10 m volta a 1000 mm de papel.
    #[test]
    fn gd_doc_03_world_values() {
        let d = Display {
            unit: Unit::Meter,
            scale: 10.0,
            ..Default::default()
        };
        assert_eq!(d.format(1_000_000.0, true), "10");
        assert!(near(d.parse("10", None, true), 1_000_000.0));
        assert_eq!(d.format(1_000_000.0, false), "1", "sem mundo: papel");
    }
}
