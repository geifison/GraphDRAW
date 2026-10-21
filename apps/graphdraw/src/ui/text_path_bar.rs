//! The property bar of a text fitted to a path (Pick tool), groups stacked
//! two rows high: orientation over the line of the letters on the stroke;
//! placement along the path over the two mirrors; the offset along the
//! path (a draggable icon, its value in the tip) and the distance from the
//! stroke over the corner rule and anti-collision; the letter spacing over
//! Badge, Stroke as Construction, Unlink and Straighten Text.

use crate::app::App;
use crate::i18n::tr;
use crate::theme::Tokens;
use crate::ui::propbar::{frame, row, sep, stacked};
use crate::ui::svg_icons::{paint, IconRef};
use egui::{Rect, Response, Sense, Ui, Vec2};
use graphdraw_core::document::{
    PathTextCorners, PathTextOrientation, PathTextVertical, TextAlign,
};

pub const ORIENTATIONS: [(PathTextOrientation, &str); 5] = [
    (PathTextOrientation::Rotate, "toolbar.path_rotate"),
    (PathTextOrientation::Upright, "toolbar.path_upright"),
    (
        PathTextOrientation::VerticalSkew,
        "toolbar.path_vertical_skew",
    ),
    (
        PathTextOrientation::HorizontalSkew,
        "toolbar.path_horizontal_skew",
    ),
    (PathTextOrientation::Warp, "toolbar.path_warp"),
];

pub const VERTICALS: [(PathTextVertical, &str); 6] = [
    (PathTextVertical::Baseline, "toolbar.path_on_baseline"),
    (PathTextVertical::Descender, "toolbar.path_on_descender"),
    (PathTextVertical::XHeight, "toolbar.path_on_xheight"),
    (PathTextVertical::CapHeight, "toolbar.path_on_cap"),
    (PathTextVertical::Center, "toolbar.path_on_center"),
    (PathTextVertical::Ascender, "toolbar.path_on_ascender"),
];

pub const PLACEMENTS: [(TextAlign, &str); 4] = [
    (TextAlign::Left, "toolbar.path_place_left"),
    (TextAlign::Center, "toolbar.path_place_center"),
    (TextAlign::Right, "toolbar.path_place_right"),
    (TextAlign::Justify, "toolbar.path_place_justify"),
];

pub const CORNERS: [(PathTextCorners, &str); 3] = [
    (PathTextCorners::Jump, "toolbar.path_corner_jump"),
    (PathTextCorners::Wrap, "toolbar.path_corner_wrap"),
    (PathTextCorners::Split, "toolbar.path_corner_split"),
];

/// Side of the small buttons of a stacked row.
const SMALL: f32 = 19.0;
/// Percent the letter spacing moves per screen point.
const DRAG_PCT_PER_PX: f64 = 0.5;

/// A small icon button of a stacked row, its icon by the text key of its
/// tip (see `svg_icons::KEYS`).
fn small(ui: &mut Ui, key: &str, pressed: bool) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(SMALL), Sense::click());
    frame(ui, rect, resp.hovered(), pressed);
    paint(
        ui.painter(),
        Rect::from_center_size(rect.center(), Vec2::splat(15.0)),
        IconRef::Key(key),
        Tokens::icon(),
    );
    resp.on_hover_text(tr(key))
}

/// One button per item, the current one pressed; the item clicked.
fn pick<T: Copy + PartialEq>(ui: &mut Ui, current: T, items: &[(T, &str)]) -> Option<T> {
    let mut out = None;
    for (v, key) in items {
        if small(ui, key, *v == current).clicked() && *v != current {
            out = Some(*v);
        }
    }
    out
}

/// A drag on an icon: the screen points it moved this frame (Shift ten
/// times as fast, Alt a tenth), and whether this drag already made its
/// undo step (`stepped`), which the caller sets once it has.
pub struct IconDrag {
    pub dx: f64,
    pub stepped: bool,
    id: egui::Id,
}

impl IconDrag {
    pub fn mark_stepped(&self, ui: &Ui) {
        ui.data_mut(|d| d.insert_temp(self.id, true));
    }
}

/// An icon to drag left and right for a continuous adjustment, with
/// `tip` (the value in it).
pub fn drag_icon(ui: &mut Ui, key: &str, tip: &str) -> Option<IconDrag> {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(18.0, SMALL), Sense::drag());
    paint(
        ui.painter(),
        Rect::from_center_size(rect.center(), Vec2::splat(14.0)),
        IconRef::Key(key),
        Tokens::icon(),
    );
    let resp = resp
        .on_hover_cursor(egui::CursorIcon::ResizeHorizontal)
        .on_hover_text(tip);
    let id = resp.id.with("stepped");
    if resp.drag_started() {
        ui.data_mut(|d| d.insert_temp(id, false));
    }
    if !resp.dragged() {
        return None;
    }
    let m = ui.input(|i| i.modifiers);
    let k = if m.shift {
        10.0
    } else if m.alt {
        0.1
    } else {
        1.0
    };
    let dx = f64::from(resp.drag_delta().x) * k;
    let stepped = ui.data(|d| d.get_temp::<bool>(id)).unwrap_or(false);
    (dx != 0.0).then_some(IconDrag { dx, stepped, id })
}

/// Letter spacing (any text): a draggable icon and the value in % of the
/// body inside its field.
pub fn letter_spacing_part(app: &mut App, ui: &mut Ui) {
    let Some(cur) = app.letter_spacing() else {
        return;
    };
    row(ui, |ui| {
        let tip = format!("{}: {:.0} %", tr("toolbar.letter_spacing_tip"), cur);
        if let Some(d) = drag_icon(ui, "toolbar.letter_spacing", &tip) {
            app.set_letter_spacing(cur + d.dx * DRAG_PCT_PER_PX, d.stepped);
            d.mark_stepped(ui);
        }
        if let Some(v) = crate::measure_field::number_field(ui, "letter_spacing", cur, 0, " %", 54.0)
        {
            app.set_letter_spacing(v, false);
        }
    });
}

pub fn text_path_bar(app: &mut App, ui: &mut Ui) {
    let Some(ft) = app.fitted_text() else {
        return;
    };
    let tp = ft.on_path.clone();
    stacked(ui, |ui| {
        row(ui, |ui| {
            if let Some(o) = pick(ui, tp.orientation, &ORIENTATIONS) {
                app.edit_fitted_text(|t, _| t.orientation = o);
            }
        });
        row(ui, |ui| {
            if let Some(v) = pick(ui, tp.vertical, &VERTICALS) {
                app.edit_fitted_text(|t, _| t.vertical = v);
            }
        });
    });
    sep(ui);
    let along = match ft.align {
        TextAlign::Center | TextAlign::Right => ft.align,
        a if a.is_justified() => TextAlign::Justify,
        _ => TextAlign::Left,
    };
    stacked(ui, |ui| {
        row(ui, |ui| {
            if let Some(a) = pick(ui, along, &PLACEMENTS) {
                app.edit_fitted_text(|_, align| *align = a);
            }
        });
        row(ui, |ui| {
            if small(ui, "toolbar.path_mirror_h", false).clicked() {
                app.mirror_text_reading();
            }
            if small(ui, "toolbar.path_mirror_v", tp.below).clicked() {
                app.flip_text_side();
            }
        });
    });
    sep(ui);
    let u = app.units;
    stacked(ui, |ui| {
        row(ui, |ui| {
            // Along the path: only the icon (drag it, or the diamond on
            // the canvas); the value is in its tip.
            let tip = format!(
                "{}: {}",
                tr("toolbar.path_offset_tip"),
                crate::measure_field::length_text(tp.offset, u, 96.0)
            );
            if let Some(d) = drag_icon(ui, "toolbar.path_offset", &tip) {
                let zoom = f64::from(app.view.zoom).max(1e-6);
                let mm = d.dx / zoom;
                app.edit_fitted_text_live(d.stepped, |t, _| t.offset += mm);
                d.mark_stepped(ui);
            }
            let tip = tr("toolbar.path_distance_tip");
            if let Some(d) = drag_icon(ui, "toolbar.path_distance", &tip) {
                let mm = d.dx / f64::from(app.view.zoom).max(1e-6);
                app.edit_fitted_text_live(d.stepped, |t, _| t.distance += mm);
                d.mark_stepped(ui);
            }
            if let Some(v) =
                crate::measure_field::length_field(ui, "path_distance", tp.distance, u, 96.0, 64.0)
            {
                app.edit_fitted_text(|t, _| t.distance = v);
            }
        });
        row(ui, |ui| {
            if let Some(c) = pick(ui, tp.corners, &CORNERS) {
                app.edit_fitted_text(|t, _| t.corners = c);
            }
            if small(ui, "toolbar.path_relax", tp.relax).clicked() {
                app.edit_fitted_text(|t, _| t.relax = !t.relax);
            }
        });
    });
    sep(ui);
    stacked(ui, |ui| {
        letter_spacing_part(app, ui);
        row(ui, |ui| {
            if app.can_split_badge() && small(ui, "toolbar.path_badge", false).clicked() {
                app.split_badge();
            }
            if let Some(g) = app.fitted_guide() {
                let on = app.doc().find_shape(g).is_some_and(|s| s.construction);
                if small(ui, "toolbar.path_construction", on).clicked() {
                    app.toggle_guide_construction();
                }
            }
            if small(ui, "toolbar.path_unlink", false).clicked() {
                app.unlink_text_path();
            }
            if small(ui, "toolbar.path_straighten", false).clicked() {
                app.straighten_text();
            }
        });
    });
}
