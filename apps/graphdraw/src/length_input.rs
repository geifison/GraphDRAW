//! What is typed in a length field, read as millimetres: sums with units
//! per term (`100+10 mm`, `120mm + 2in - 15pt`), other units than the
//! field's (`8.5 in` in a millimetre field), mixed fractions (`3 1/8 in`),
//! percentages of a reference (`50%`) and, starting with an operator, a
//! change of the current value (`+15`, `*2`, `+20%`; `=` forces an
//! absolute value). Also how a length is shown: a few decimals, trailing
//! zeros dropped, no floating point residue (10 mm never shows 9.999999).

use crate::app::Units;

/// Pixel resolution when none is given (dpi).
pub const DEFAULT_PX_DPI: f64 = 96.0;

/// Millimetres per unit for a unit name or abbreviation (any case, in
/// English or Portuguese), with the pixel at `dpi`.
pub fn unit_mm(name: &str, dpi: f64) -> Option<f64> {
    let dpi = if dpi.is_finite() && dpi > 0.0 {
        dpi
    } else {
        DEFAULT_PX_DPI
    };
    let didot = 0.376_065;
    Some(match name.trim().to_lowercase().as_str() {
        "um" | "µm" | "μm" | "micron" | "mícron" => 0.001,
        "mm" => 1.0,
        "cm" => 10.0,
        "m" => 1000.0,
        "km" => 1_000_000.0,
        "in" | "inch" | "pol" | "\"" | "″" => 25.4,
        "ft" | "pé" | "pe" | "pés" | "pes" | "'" | "′" => 304.8,
        "yd" | "jd" | "jarda" | "jardas" => 914.4,
        "mi" => 1_609_344.0,
        "pt" => 25.4 / 72.0,
        "pc" | "pica" | "paica" => 25.4 / 6.0,
        "dd" | "didot" => didot,
        "cc" | "c" | "cícero" | "cicero" => 12.0 * didot,
        "px" => 25.4 / dpi,
        _ => return None,
    })
}

/// Millimetres per `unit`, with the pixel at `dpi` (not at the active
/// drawing's resolution).
pub fn units_mm(unit: Units, dpi: f64) -> f64 {
    if unit == Units::Pixels {
        unit_mm("px", dpi).unwrap_or(25.4 / DEFAULT_PX_DPI)
    } else {
        unit.mm()
    }
}

/// `mm` in `unit` (pixel at `dpi`) with at most `decimals` places,
/// trailing zeros dropped, a point as the decimal mark.
pub fn format(mm: f64, unit: Units, dpi: f64, decimals: usize) -> String {
    format_number(mm / units_mm(unit, dpi), decimals)
}

/// `v` with at most `decimals` places, trailing zeros dropped.
pub fn format_number(v: f64, decimals: usize) -> String {
    let d = decimals.min(6);
    let s = format!("{v:.d$}");
    let s = if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s
    };
    if s == "-0" {
        "0".into()
    } else {
        s
    }
}

/// A length field's text (see the module notes), in millimetres. The
/// field's unit is `default_mm` millimetres; `current` is the field's
/// value, the base of a leading operator and of `%`.
pub fn parse_input(text: &str, default_mm: f64, dpi: f64, current: Option<f64>) -> Option<f64> {
    let t = text.trim();
    if let Some(abs) = t.strip_prefix('=') {
        return parse_length(abs, default_mm, dpi, current);
    }
    let Some(cur) = current else {
        return parse_length(t, default_mm, dpi, None);
    };
    let op = t.chars().next()?;
    let rest = t[op.len_utf8()..].trim();
    match op {
        '+' => Some(cur + parse_length(rest, default_mm, dpi, Some(cur))?),
        '-' => Some(cur - parse_length(rest, default_mm, dpi, Some(cur))?),
        '*' | '×' => Some(cur * parse_number(rest)?),
        '/' | '÷' => {
            let d = parse_number(rest)?;
            (d != 0.0).then(|| cur / d)
        }
        _ => parse_length(t, default_mm, dpi, Some(cur)),
    }
}

/// A length expression in millimetres: numbers with a comma or a point,
/// a unit per term (none: `default_mm` millimetres per unit), `%` of
/// `reference`, `+ - * /` and brackets. `10+5`, `2in`, `50%`,
/// `(10mm+1cm)/2`.
pub fn parse_length(text: &str, default_mm: f64, dpi: f64, reference: Option<f64>) -> Option<f64> {
    let mut p = Parser {
        s: text.trim().as_bytes(),
        i: 0,
        unit: default_mm,
        reference,
        dpi,
        explicit: false,
    };
    let v = p.sum(true)?;
    p.ws();
    (p.i == p.s.len() && v.is_finite()).then_some(v)
}

/// A number expression without units (angles, percentages, dpi).
pub fn parse_number(text: &str) -> Option<f64> {
    let mut p = Parser {
        s: text.trim().as_bytes(),
        i: 0,
        unit: 1.0,
        reference: None,
        dpi: DEFAULT_PX_DPI,
        explicit: false,
    };
    let v = p.sum(false)?;
    p.ws();
    (p.i == p.s.len() && v.is_finite()).then_some(v)
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
    /// Millimetres per unit of a bare number.
    unit: f64,
    reference: Option<f64>,
    dpi: f64,
    /// The last length read had a unit written after it.
    explicit: bool,
}

impl Parser<'_> {
    fn ws(&mut self) {
        while self.s.get(self.i).is_some_and(|c| c.is_ascii_whitespace()) {
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

    /// `lengths`: the terms are lengths (with units); otherwise plain
    /// numbers.
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

    /// In a product only the first factor carries a unit; the others are
    /// scalars.
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
            // A unit at the end of the product counts for all of it
            // ("1/2in").
            if lengths && !explicit {
                if let Some(u) = self.unit_name() {
                    v = v / self.unit * u;
                }
            }
        }
    }

    fn name_end(&self, mut i: usize) -> usize {
        while self
            .s
            .get(i)
            .is_some_and(|c| !c.is_ascii_digit() && !b"+-*/() ".contains(c))
        {
            i += 1;
        }
        i
    }

    /// A unit name just ahead (not consumed when it is not one).
    fn unit_name(&mut self) -> Option<f64> {
        let save = self.i;
        self.ws();
        let st = self.i;
        self.i = self.name_end(st);
        let u = self
            .s
            .get(st..self.i)
            .and_then(|b| std::str::from_utf8(b).ok())
            .filter(|n| !n.is_empty())
            .and_then(|n| unit_mm(n, self.dpi));
        if u.is_none() {
            self.i = save;
        }
        u
    }

    fn digits(&mut self) -> Option<f64> {
        let st = self.i;
        while self.s.get(self.i).is_some_and(u8::is_ascii_digit) {
            self.i += 1;
        }
        std::str::from_utf8(self.s.get(st..self.i)?)
            .ok()?
            .parse::<f64>()
            .ok()
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
        while self
            .s
            .get(self.i)
            .is_some_and(|c| c.is_ascii_digit() || matches!(c, b'.' | b','))
        {
            self.i += 1;
        }
        let lit = std::str::from_utf8(self.s.get(start..self.i)?).ok()?;
        let mut num: f64 = lit.replace(',', ".").parse().ok()?;
        // A mixed fraction: "3 1/8" (whole number, space, a/b).
        if !lit.contains(['.', ',']) {
            let save = self.i;
            self.ws();
            let spaced = self.i > save;
            match (spaced, self.digits()) {
                (true, Some(a)) if self.s.get(self.i) == Some(&b'/') => {
                    self.i += 1;
                    match self.digits() {
                        Some(b) if b > 0.0 => num += a / b,
                        _ => self.i = save,
                    }
                }
                _ => self.i = save,
            }
        }
        self.ws();
        if self.eat(b'%') {
            return if lengths {
                Some(num / 100.0 * self.reference?)
            } else {
                Some(num)
            };
        }
        if !lengths {
            return Some(num);
        }
        let ustart = self.i;
        self.i = self.name_end(ustart);
        let name = std::str::from_utf8(self.s.get(ustart..self.i)?).ok()?;
        let unit = if name.is_empty() {
            self.unit
        } else {
            self.explicit = true;
            unit_mm(name, self.dpi)?
        };
        Some(num * unit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: Option<f64>, b: f64) -> bool {
        a.is_some_and(|a| (a - b).abs() < 1e-9)
    }

    fn mm(text: &str) -> Option<f64> {
        parse_length(text, 1.0, DEFAULT_PX_DPI, None)
    }

    /// 10 mm shows as 10 mm, 0.3937 in and 28.35 pt.
    #[test]
    fn conversions() {
        assert_eq!(format(10.0, Units::Millimeters, 96.0, 3), "10");
        assert_eq!(format(10.0, Units::Inches, 96.0, 4), "0.3937");
        assert_eq!(format(10.0, Units::Points, 96.0, 2), "28.35");
    }

    /// Sums and units in a field.
    #[test]
    fn expressions() {
        assert!(near(mm("10+5"), 15.0));
        assert!(near(mm("100+10 mm"), 110.0));
        assert!(near(mm("2in"), 50.8));
        assert!(near(mm("8.5 in"), 215.9));
        assert!(near(mm("21 cm"), 210.0));
        assert!(near(parse_length("50%", 1.0, 96.0, Some(80.0)), 40.0));
        assert!(near(mm("10,5"), 10.5));
        assert!(near(mm("(10mm + 1cm) / 2"), 10.0));
        assert!(near(mm("3 * 2"), 6.0));
        assert!(near(mm("-5"), -5.0));
        assert!((mm("72pt").unwrap_or(0.0) - 25.4).abs() < 1e-9);
        assert_eq!(mm("1/0"), None);
        assert_eq!(mm("10 parsecs"), None);
        assert_eq!(mm("50%"), None);
        assert_eq!(mm(""), None);
        assert_eq!(mm("(3"), None);
        assert_eq!(parse_number("30*2+0,5"), Some(60.5));
        // A bare number is in the field's unit: 2 in a centimetre field.
        assert!(near(parse_length("2", 10.0, 96.0, None), 20.0));
    }

    /// Every unit family, with exact factors.
    #[test]
    fn unit_families() {
        assert!(near(mm("1m"), 1000.0));
        assert!(near(mm("0,5 km"), 500_000.0));
        assert!(near(mm("1yd"), 914.4));
        assert!(near(mm("1ft"), 304.8));
        assert!(near(mm("1 pol"), 25.4));
        assert!(near(mm("2\""), 50.8));
        assert!(near(mm("1pc"), 25.4 / 6.0));
        assert!(near(mm("1 Didot"), 0.376_065));
        assert!(near(mm("1cc"), 12.0 * 0.376_065));
        assert!(near(mm("1000 µm"), 1.0));
        assert!(near(mm("300px"), 300.0 * 25.4 / 96.0));
        for u in Units::ALL {
            let factor = unit_mm(u.id(), 96.0);
            assert!(factor.is_some(), "{u:?}");
            if u != Units::Pixels {
                assert!(near(factor, u.mm()), "{u:?}");
            }
        }
    }

    /// Units mixed in one sum: 120mm + 2in - 15pt = 165.508 mm.
    #[test]
    fn mixed_units() {
        let v = mm("120mm + 2pol - 15pt").unwrap_or(0.0);
        assert!((v - 165.508).abs() < 0.005, "{v}");
        // Mixed fractions: 3 1/8 in = 79.375 mm.
        assert!(near(mm("3 1/8 pol"), 79.375));
        assert!(near(mm("1/2in"), 12.7));
    }

    /// A leading operator works on the current value.
    #[test]
    fn relative_input() {
        let p = |t: &str| parse_input(t, 1.0, 96.0, Some(100.0));
        assert!(near(p("+15"), 115.0));
        assert!(near(p("-5mm"), 95.0));
        assert!(near(p("*2"), 200.0));
        assert!(near(p("/3"), 100.0 / 3.0));
        assert!(near(p("+20%"), 120.0));
        assert!(near(p("50%"), 50.0));
        assert!(near(p("=-5"), -5.0), "= forces an absolute value");
        assert!(near(p("40"), 40.0));
        assert_eq!(p("/0"), None);
        // Without a current value, "-5" is negative.
        assert!(near(parse_input("-5", 1.0, 96.0, None), -5.0));
    }

    /// The pixel follows the resolution: 25.4 mm is 300 px at 300 dpi and
    /// 150 px at 150 dpi.
    #[test]
    fn pixels_follow_dpi() {
        assert_eq!(format(25.4, Units::Pixels, 300.0, 1), "300");
        assert_eq!(format(25.4, Units::Pixels, 150.0, 1), "150");
        let px300 = units_mm(Units::Pixels, 300.0);
        assert!(near(parse_input("600", px300, 300.0, None), 50.8));
        // 210 mm is 793.7 px at 96 dpi, and back.
        assert_eq!(format(210.0, Units::Pixels, 96.0, 1), "793.7");
        let back = parse_length("793.7", units_mm(Units::Pixels, 96.0), 96.0, None);
        assert!((back.unwrap_or(0.0) - 210.0).abs() < 0.01);
    }

    /// Decimals only in what is shown, no residue.
    #[test]
    fn decimals_and_no_drift() {
        assert_eq!(format(0.12567, Units::Millimeters, 96.0, 3), "0.126");
        assert_eq!(format(0.12567, Units::Millimeters, 96.0, 5), "0.12567");
        assert_eq!(format(0.12567, Units::Millimeters, 96.0, 0), "0");
        assert_eq!(format(9.999_999_999_9, Units::Millimeters, 96.0, 6), "10");
        assert_eq!(format(0.1 + 0.2, Units::Millimeters, 96.0, 6), "0.3");
        assert_eq!(format(254.0, Units::Inches, 96.0, 3), "10");
        assert_eq!(format(-0.0001, Units::Millimeters, 96.0, 2), "0");
    }

    /// Nothing typed makes it panic.
    #[test]
    fn junk_never_panics() {
        for t in [
            "", "-", "+", "((", "))", "1/", "3 1/", "3 1/0", "µ", "1e999", "--5", "%", "=",
            "1 2 3", "ä", "1ä", "*", "/0",
        ] {
            let _ = parse_input(t, 1.0, 96.0, Some(1.0));
            let _ = parse_input(t, 1.0, 0.0, None);
            let _ = parse_number(t);
        }
    }
}
