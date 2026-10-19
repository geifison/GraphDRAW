//! Measurement fields with the unit inside the field ("210 mm", "300
//! dpi"): typed into, never dragged (continuous adjustment belongs on an
//! icon next to a field, not on the field). The text accepts what
//! [`crate::length_input`] reads: sums, other units, fractions,
//! percentages and changes of the current value. Enter, Tab or leaving
//! the field applies the value, Esc gives up and shows the value again,
//! the up and down arrows add a step (Shift: ten steps) at once.

use crate::app::Units;
use crate::length_input;

/// Up and down arrows: 1 mm in length fields, 1 in number fields.
const LENGTH_ARROW_STEP: f64 = 1.0;
const NUMBER_ARROW_STEP: f64 = 1.0;

/// Languages that write a decimal comma.
const COMMA_LANGUAGES: [&str; 11] = [
    "pt-BR", "es", "fr", "de", "it", "nl", "pl", "ru", "tr", "cs", "id",
];

/// A number as the active language writes it.
pub fn localize(s: String) -> String {
    if COMMA_LANGUAGES.contains(&crate::i18n::language().as_str()) {
        s.replace('.', ",")
    } else {
        s
    }
}

/// `mm` in `unit` with its abbreviation, as the length fields show it:
/// one decimal (two in inches), pixel at `dpi`.
pub fn length_text(mm: f64, unit: Units, dpi: f64) -> String {
    format!("{} {}", localize(unit_value_text(mm, unit, dpi)), unit.id())
}

/// The number part of [`length_text`] (a point as the decimal mark).
pub fn unit_value_text(mm: f64, unit: Units, dpi: f64) -> String {
    let decimals = if unit == Units::Inches { 2 } else { 1 };
    length_input::format(mm, unit, dpi, decimals)
}

/// A length in `unit` (pixel at `dpi`); returns millimetres when a new
/// value is applied.
pub fn length_field(
    ui: &mut egui::Ui,
    id: &str,
    value_mm: f64,
    unit: Units,
    dpi: f64,
    width: f32,
) -> Option<f64> {
    let id = ui.id().with(id);
    let unit_mm = length_input::units_mm(unit, dpi);
    expr_field(
        ui,
        id,
        length_text(value_mm, unit, dpi),
        width,
        LENGTH_ARROW_STEP,
        |t| length_input::parse_input(t, unit_mm, dpi, Some(value_mm)),
    )
}

/// A number with `suffix` inside the field (" dpi", "°", "%"); typing
/// it with or without the suffix works.
pub fn number_field(
    ui: &mut egui::Ui,
    id: &str,
    value: f64,
    decimals: usize,
    suffix: &str,
    width: f32,
) -> Option<f64> {
    let id = ui.id().with(id);
    let sfx = suffix.trim().to_string();
    expr_field(
        ui,
        id,
        format!(
            "{}{suffix}",
            localize(length_input::format_number(value, decimals))
        ),
        width,
        NUMBER_ARROW_STEP,
        move |t| {
            let t = t.trim();
            let t = if sfx.is_empty() {
                t
            } else {
                t.strip_suffix(sfx.as_str()).unwrap_or(t).trim()
            };
            length_input::parse_number(t)
        },
    )
}

/// The text field under both: the value once applied and changed.
fn expr_field(
    ui: &mut egui::Ui,
    id: egui::Id,
    shown: String,
    width: f32,
    step: f64,
    parse: impl Fn(&str) -> Option<f64>,
) -> Option<f64> {
    let mut text: String = ui
        .data(|d| d.get_temp::<String>(id))
        .unwrap_or_else(|| shown.clone());
    let resp = ui.add(
        egui::TextEdit::singleline(&mut text)
            .id(id.with("te"))
            .font(egui::TextStyle::Monospace)
            .vertical_align(egui::Align::Center)
            .desired_width(width),
    );
    let mut out = None;
    // An arrow pressed in this frame: one step up or down, ten with Shift.
    let arrow = ui.input(|i| {
        i.events.iter().find_map(|e| match e {
            egui::Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } if matches!(key, egui::Key::ArrowUp | egui::Key::ArrowDown) => {
                let k = if modifiers.shift { 10.0 } else { 1.0 };
                Some(if *key == egui::Key::ArrowUp { k } else { -k })
            }
            _ => None,
        })
    });
    let esc = ui.input(|i| i.key_pressed(egui::Key::Escape));
    if resp.has_focus() {
        if let Some(n) = arrow {
            // The text being typed becomes its value plus the step; the
            // field keeps the focus and shows the new value next frame.
            ui.data_mut(|d| d.remove::<String>(id));
            out = parse(&text).map(|v| v + n * step);
        } else {
            ui.data_mut(|d| d.insert_temp(id, text.clone()));
        }
    } else if resp.lost_focus() {
        ui.data_mut(|d| d.remove::<String>(id));
        if !esc && text != shown {
            out = parse(&text);
        }
    } else {
        ui.data_mut(|d| d.remove::<String>(id));
    }
    out.filter(|v| v.is_finite())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(key: egui::Key, shift: bool) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers {
                shift,
                ..Default::default()
            },
        }
    }

    /// Field `id` focused in frame 0 with `typed` in it (if any), then
    /// one frame per list of events. Returns what `draw` gave in each
    /// event frame and whether typed text was left in memory.
    fn drive(
        id: &str,
        typed: Option<&str>,
        frames: Vec<Vec<egui::Event>>,
        mut draw: impl FnMut(&mut egui::Ui) -> Option<f64>,
    ) -> (Vec<Option<f64>>, bool) {
        let ctx = egui::Context::default();
        let mut out = Vec::new();
        let mut field = egui::Id::NULL;
        for (n, events) in std::iter::once(Vec::new()).chain(frames).enumerate() {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(800.0, 600.0),
                )),
                events,
                ..Default::default()
            };
            let mut o = ctx.run_ui(input, |ui| {
                field = ui.id().with(id);
                if n == 0 {
                    ui.memory_mut(|m| m.request_focus(field.with("te")));
                    if let Some(t) = typed {
                        ui.data_mut(|d| d.insert_temp(field, t.to_string()));
                    }
                }
                let r = draw(ui);
                if n > 0 {
                    out.push(r);
                }
            });
            o.textures_delta.clear();
        }
        let pending = ctx.data(|d| d.get_temp::<String>(field)).is_some();
        (out, pending)
    }

    #[test]
    fn enter_applies_typed_value() {
        let (r, _) = drive(
            "t",
            Some("25"),
            vec![vec![key(egui::Key::Enter, false)]],
            |ui| number_field(ui, "t", 10.0, 0, " dpi", 80.0),
        );
        assert_eq!(r, [Some(25.0)]);
    }

    #[test]
    fn escape_restores_value() {
        let (r, pending) = drive(
            "t",
            Some("25"),
            vec![vec![key(egui::Key::Escape, false)], vec![]],
            |ui| number_field(ui, "t", 10.0, 0, " dpi", 80.0),
        );
        assert_eq!(r, [None, None], "nothing applied");
        assert!(!pending, "the typed text is dropped");
    }

    /// The arrows add a step (1, or 1 mm in lengths); Shift ten steps.
    #[test]
    fn arrows_step_the_value() {
        let (r, _) = drive(
            "t",
            None,
            vec![
                vec![key(egui::Key::ArrowUp, false)],
                vec![key(egui::Key::ArrowDown, true)],
            ],
            |ui| number_field(ui, "t", 10.0, 0, "", 80.0),
        );
        assert_eq!(r, [Some(11.0), Some(0.0)]);
        let (r, _) = drive(
            "c",
            None,
            vec![vec![key(egui::Key::ArrowUp, false)]],
            |ui| length_field(ui, "c", 10.0, Units::Millimeters, 96.0, 110.0),
        );
        assert_eq!(r, [Some(11.0)]);
    }

    /// The unit is in the field, and a value typed in another unit or as
    /// a sum is read in millimetres.
    #[test]
    fn length_typed_with_units_and_sums() {
        assert_eq!(length_text(210.0, Units::Millimeters, 96.0), "210 mm");
        assert_eq!(length_text(215.9, Units::Inches, 96.0), "8.5 in");
        for (typed, want) in [("8.5 in", 215.9), ("100+10 mm", 110.0), ("21 cm", 210.0)] {
            let (r, _) = drive(
                "w",
                Some(typed),
                vec![vec![key(egui::Key::Enter, false)]],
                |ui| length_field(ui, "w", 210.0, Units::Millimeters, 96.0, 110.0),
            );
            assert!(
                r.len() == 1 && r[0].is_some_and(|v| (v - want).abs() < 1e-9),
                "{typed}: {r:?}"
            );
        }
        // Junk applies nothing.
        let (r, _) = drive(
            "w",
            Some("abc"),
            vec![vec![key(egui::Key::Enter, false)]],
            |ui| length_field(ui, "w", 210.0, Units::Millimeters, 96.0, 110.0),
        );
        assert_eq!(r, [None]);
    }
}
