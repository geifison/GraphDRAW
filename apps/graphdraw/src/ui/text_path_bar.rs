//! The property bar of a text fitted to a path (Pick tool): orientation,
//! vertical placement, placement along the path, distance from it, offset
//! along it, the two mirrors and Straighten Text.

use crate::app::App;
use crate::i18n::tr;
use crate::ui::field::NumField;
use crate::ui::propbar::{pic_button, sep, Pic};
use egui::Ui;
use tracedraw_core::document::{PathTextOrientation, PathTextVertical, TextAlign};

const ORIENTATIONS: [(PathTextOrientation, &str); 4] = [
    (PathTextOrientation::Rotate, "toolbar.path_rotate"),
    (
        PathTextOrientation::VerticalSkew,
        "toolbar.path_vertical_skew",
    ),
    (
        PathTextOrientation::HorizontalSkew,
        "toolbar.path_horizontal_skew",
    ),
    (PathTextOrientation::Upright, "toolbar.path_upright"),
];

const VERTICALS: [(PathTextVertical, &str); 4] = [
    (PathTextVertical::Baseline, "toolbar.path_on_baseline"),
    (PathTextVertical::Ascender, "toolbar.path_on_ascender"),
    (PathTextVertical::Descender, "toolbar.path_on_descender"),
    (PathTextVertical::Center, "toolbar.path_on_center"),
];

const PLACEMENTS: [(TextAlign, &str); 3] = [
    (TextAlign::Left, "toolbar.path_place_left"),
    (TextAlign::Center, "toolbar.path_place_center"),
    (TextAlign::Right, "toolbar.path_place_right"),
];

/// A drop-down list of `items`; returns the one picked when it changed.
fn choice<T: Copy + PartialEq>(
    ui: &mut Ui,
    id: &str,
    tip: &str,
    current: T,
    items: &[(T, &str)],
    width: f32,
) -> Option<T> {
    let mut picked = None;
    let label = items
        .iter()
        .find(|(v, _)| *v == current)
        .map(|(_, k)| tr(k))
        .unwrap_or_default();
    egui::ComboBox::from_id_salt(id)
        .width(width)
        .selected_text(label)
        .show_ui(ui, |ui| {
            for (v, key) in items {
                if ui.selectable_label(*v == current, tr(key)).clicked() && *v != current {
                    picked = Some(*v);
                }
            }
        })
        .response
        .on_hover_text(tip);
    picked
}

pub fn text_path_bar(app: &mut App, ui: &mut Ui) {
    let Some(ft) = app.fitted_text() else {
        return;
    };
    let tp = ft.on_path.clone();
    if let Some(o) = choice(
        ui,
        "path_orientation",
        &tr("toolbar.path_orientation"),
        tp.orientation,
        &ORIENTATIONS,
        130.0,
    ) {
        app.edit_fitted_text(|t, _| t.orientation = o);
    }
    if let Some(v) = choice(
        ui,
        "path_vertical",
        &tr("toolbar.path_vertical"),
        tp.vertical,
        &VERTICALS,
        130.0,
    ) {
        app.edit_fitted_text(|t, _| t.vertical = v);
    }
    let along = match ft.align {
        TextAlign::Center | TextAlign::Right => ft.align,
        _ => TextAlign::Left,
    };
    if let Some(a) = choice(
        ui,
        "path_placement",
        &tr("toolbar.path_placement"),
        along,
        &PLACEMENTS,
        90.0,
    ) {
        app.edit_fitted_text(|_, align| *align = a);
    }
    sep(ui);
    let u = app.units;
    for (key, value, is_distance) in [
        ("toolbar.path_distance", tp.distance, true),
        ("toolbar.path_offset", tp.offset, false),
    ] {
        let mut v = u.from_mm(value);
        let r = ui
            .add(
                NumField::new(&mut v)
                    .speed(0.1)
                    .suffix(format!(" {}", u.short())),
            )
            .on_hover_text(tr(key));
        if r.changed() {
            let mm = u.to_mm(v);
            app.edit_fitted_text(|t, _| {
                if is_distance {
                    t.distance = mm;
                } else {
                    t.offset = mm;
                }
            });
        }
    }
    sep(ui);
    let (h, v) = tp.mirrors();
    if pic_button(ui, Pic::MirrorH, &tr("toolbar.mirror_h"), true, h).clicked() {
        app.edit_fitted_text(|t, _| t.set_mirrors(!h, v));
    }
    if pic_button(ui, Pic::MirrorV, &tr("toolbar.mirror_v"), true, v).clicked() {
        app.edit_fitted_text(|t, _| t.set_mirrors(h, !v));
    }
    sep(ui);
    if ui
        .button(tr("menu.text.straighten_text"))
        .on_hover_text(tr("toolbar.path_straighten_tip"))
        .clicked()
    {
        app.straighten_text();
    }
}
