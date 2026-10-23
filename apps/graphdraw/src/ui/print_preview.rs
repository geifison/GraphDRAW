//! File > Print Preview: the sheets exactly as File > Print sends them,
//! with the imposition, marks, mirror, invert and each separation plate,
//! and the print options at hand beside them (see
//! `docs/behavior/printing.md`, Print Preview).

use crate::app::App;
use crate::export::{plate_page, plates, print_sheets};
use crate::i18n::{tr, trf};
use crate::theme::Tokens;
use crate::ui::dialogs::PrintState;
use egui::Ui;
use graphdraw_core::Document;

/// The open preview: the settings it shows, the sheet and plate on screen
/// and what was last built from them.
pub struct PrintPreview {
    pub st: PrintState,
    /// Index of the sheet shown.
    pub sheet: usize,
    /// 0 = composite, n = the n-th plate of [`plates`].
    pub plate: usize,
    /// Zoom over the fitted size (1 = the sheet fits the window).
    pub zoom: f32,
    /// Where the sheet's centre sits from the view's centre, in points.
    pub pan: egui::Vec2,
    /// Real size asked for: the zoom is set once the fitted size is known.
    real_size: bool,
    /// The zoom rectangle being dragged: where it started and the pointer.
    marquee: Option<(egui::Pos2, egui::Pos2)>,
    built: Option<(String, Document)>,
    texture: Option<(String, egui::TextureHandle)>,
}

impl PrintPreview {
    pub fn new(st: PrintState) -> Self {
        PrintPreview {
            st,
            sheet: 0,
            plate: 0,
            zoom: 1.0,
            pan: egui::Vec2::ZERO,
            real_size: false,
            marquee: None,
            built: None,
            texture: None,
        }
    }
}

/// The zoom's range over the fitted size.
const MIN_ZOOM: f32 = 0.1;
const MAX_ZOOM: f32 = 64.0;
/// Points of the sheet kept on screen however far it is panned.
const KEEP_VISIBLE: f32 = 48.0;
/// Largest side, in pixels, of a whole-sheet bitmap; past it only the
/// part on screen is rendered.
const WHOLE_SHEET_PX: f64 = 4096.0;

/// Zoom by `factor` keeping the point `at` (from the view's centre) still.
fn zoom_about(zoom: f32, pan: egui::Vec2, factor: f32, at: egui::Vec2) -> (f32, egui::Vec2) {
    let z = (zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
    let f = z / zoom.max(1e-6);
    (z, at - (at - pan) * f)
}

/// Zoom so that `rect` (from the view's centre) fills the view, centred.
fn zoom_to_rect(
    zoom: f32,
    pan: egui::Vec2,
    rect: egui::Rect,
    view: egui::Vec2,
) -> (f32, egui::Vec2) {
    let factor = (view.x / rect.width().max(1.0)).min(view.y / rect.height().max(1.0));
    let z = (zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
    let f = z / zoom.max(1e-6);
    (z, (pan - rect.center().to_vec2()) * f)
}

/// The pan kept so that part of the sheet stays on screen.
fn clamp_pan(pan: egui::Vec2, sheet: egui::Vec2, view: egui::Vec2) -> egui::Vec2 {
    let lim = ((view + sheet) * 0.5 - egui::vec2(KEEP_VISIBLE, KEEP_VISIBLE)).max(egui::Vec2::ZERO);
    egui::vec2(pan.x.clamp(-lim.x, lim.x), pan.y.clamp(-lim.y, lim.y))
}

/// The zoom over the fitted size that shows the sheet at its real size:
/// `real` and `fit` are points per millimetre.
fn real_size_zoom(real: f32, fit: f32) -> f32 {
    (real / fit.max(1e-6)).clamp(MIN_ZOOM, MAX_ZOOM)
}

/// Wheel notches this frame, from the raw events (one notch is one line or
/// 50 points), as the drawing window counts them.
fn wheel_notches(i: &egui::InputState) -> f32 {
    i.events
        .iter()
        .map(|e| match e {
            egui::Event::MouseWheel { unit, delta, .. } => match unit {
                egui::MouseWheelUnit::Line => delta.y,
                egui::MouseWheelUnit::Point => delta.y / 50.0,
                egui::MouseWheelUnit::Page => delta.y * 10.0,
            },
            _ => 0.0,
        })
        .sum()
}

/// The wheel and pinch zoom about the pointer, Ctrl+plus and Ctrl+minus
/// about the centre, the middle button pans, a left drag zooms into its
/// rectangle and a double click fits the sheet.
fn navigate(pv: &mut PrintPreview, ui: &Ui, response: &egui::Response, area: egui::Rect) {
    let centre = area.center();
    if let Some(h) = response.hover_pos() {
        let (notches, pinch) = ui.input(|i| (wheel_notches(i), i.zoom_delta()));
        let factor = if pinch != 1.0 {
            pinch
        } else {
            1.15f32.powf(notches)
        };
        if factor != 1.0 {
            (pv.zoom, pv.pan) = zoom_about(pv.zoom, pv.pan, factor, h - centre);
        }
    }
    let (zoom_in, zoom_out) = ui.input(|i| {
        (
            i.modifiers.command
                && (i.key_pressed(egui::Key::Plus) || i.key_pressed(egui::Key::Equals)),
            i.modifiers.command && i.key_pressed(egui::Key::Minus),
        )
    });
    if zoom_in || zoom_out {
        let factor = if zoom_in { 1.25 } else { 0.8 };
        (pv.zoom, pv.pan) = zoom_about(pv.zoom, pv.pan, factor, egui::Vec2::ZERO);
    }
    if response.dragged_by(egui::PointerButton::Middle) {
        pv.pan += response.drag_delta();
    }
    if response.dragged_by(egui::PointerButton::Primary) {
        if let Some(p) = response.interact_pointer_pos() {
            let start = pv.marquee.map(|(a, _)| a).unwrap_or(p);
            pv.marquee = Some((start, p));
        }
    }
    if response.drag_stopped() {
        if let Some((a, b)) = pv.marquee.take() {
            let r = egui::Rect::from_two_pos(a, b);
            if r.width() > 4.0 && r.height() > 4.0 {
                (pv.zoom, pv.pan) =
                    zoom_to_rect(pv.zoom, pv.pan, r.translate(-centre.to_vec2()), area.size());
            }
        }
    }
    if response.double_clicked() {
        pv.zoom = 1.0;
        pv.pan = egui::Vec2::ZERO;
    }
}

/// What the sheets depend on: every print setting but the preflight list.
fn settings_key(st: &PrintState) -> String {
    let mut st = st.clone();
    st.preflight = None;
    st.copies = 1;
    format!("{st:?}")
}

/// The sheets for the settings, built again only when they changed.
fn sheets<'a>(app: &App, pv: &'a mut PrintPreview) -> &'a Document {
    let key = settings_key(&pv.st);
    if pv.built.as_ref().map(|(k, _)| k != &key).unwrap_or(true) {
        pv.built = Some((key, print_sheets(app, &pv.st)));
        pv.texture = None;
    }
    // Just built when missing.
    &pv.built
        .get_or_insert_with(|| (String::new(), Document::default()))
        .1
}

/// The page shown: the sheet, or one plate of it.
pub fn shown_page(
    doc: &Document,
    sheet: usize,
    plate: usize,
    separations: bool,
) -> Option<graphdraw_core::document::Page> {
    let page = doc.pages.get(sheet)?;
    if !separations || plate == 0 {
        return Some(page.clone());
    }
    let p = plates(doc).get(plate - 1)?.0;
    Some(plate_page(page, p))
}

/// The names of the plates, translated for the process inks.
fn plate_names(doc: &Document) -> Vec<String> {
    let mut names = vec![tr("print.composite")];
    for (i, (_, name)) in plates(doc).into_iter().enumerate() {
        names.push(match i {
            0 => tr("fxp.cyan"),
            1 => tr("fxp.magenta"),
            2 => tr("fxp.yellow"),
            3 => tr("fxp.black"),
            _ => name,
        });
    }
    names
}

pub fn show(app: &mut App, ui: &mut Ui) {
    let Some(mut pv) = app.print_preview.take() else {
        return;
    };
    let mut close = false;
    let mut print = false;
    let ctx = ui.ctx().clone();
    // Points per millimetre at real size (Options > Display > Ruler
    // calibration).
    let real = crate::settings::screen_points_per_inch(&app.settings) / 25.4;

    egui::Panel::top("print_preview_bar")
        .frame(
            egui::Frame::new()
                .fill(Tokens::menu_bar())
                .inner_margin(egui::Margin::symmetric(8, 4)),
        )
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                if ui.button(tr("dialog.print")).clicked() {
                    print = true;
                }
                if ui.button(tr("dialog.close")).clicked() {
                    close = true;
                }
                ui.separator();
                let n = sheets(app, &mut pv).pages.len().max(1);
                pv.sheet = pv.sheet.min(n - 1);
                if ui
                    .add_enabled(pv.sheet > 0, egui::Button::new("<"))
                    .on_hover_text(tr("status.previous_page"))
                    .clicked()
                {
                    pv.sheet -= 1;
                }
                ui.label(trf(
                    "print.sheet_of",
                    &[
                        ("n", &(pv.sheet + 1).to_string()),
                        ("total", &n.to_string()),
                    ],
                ));
                if ui
                    .add_enabled(pv.sheet + 1 < n, egui::Button::new(">"))
                    .on_hover_text(tr("status.next_page"))
                    .clicked()
                {
                    pv.sheet += 1;
                }
                ui.separator();
                if pv.st.separations {
                    let names = plate_names(sheets(app, &mut pv));
                    pv.plate = pv.plate.min(names.len() - 1);
                    ui.label(tr("print.plate"));
                    egui::ComboBox::from_id_salt("print_preview_plate")
                        .selected_text(names[pv.plate].clone())
                        .show_ui(ui, |ui| {
                            for (i, name) in names.iter().enumerate() {
                                ui.selectable_value(&mut pv.plate, i, name.clone());
                            }
                        });
                    ui.separator();
                }
                ui.label(tr("print.zoom"));
                let before = pv.zoom;
                ui.add(
                    egui::Slider::new(&mut pv.zoom, MIN_ZOOM..=MAX_ZOOM)
                        .logarithmic(true)
                        .custom_formatter(|v, _| format!("{:.0} %", v * 100.0)),
                );
                if pv.zoom != before {
                    pv.pan *= pv.zoom / before.max(1e-6);
                }
                if ui.button(tr("print.fit")).clicked() {
                    pv.zoom = 1.0;
                    pv.pan = egui::Vec2::ZERO;
                }
                if ui
                    .button(tr("print.real_size"))
                    .on_hover_text(tr("print.real_size_hint"))
                    .clicked()
                {
                    pv.real_size = true;
                }
            });
        });

    egui::Panel::left("print_preview_options")
        .resizable(false)
        .exact_size(330.0)
        .show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                let st = &mut pv.st;
                ui.checkbox(&mut st.all_pages, tr("dialog.all_pages"));
                ui.horizontal(|ui| {
                    ui.checkbox(&mut st.fit_to_page, tr("dialog.fit_to_page"));
                    if !st.fit_to_page {
                        ui.add(
                            egui::DragValue::new(&mut st.scale)
                                .range(1.0..=1000.0)
                                .suffix(" %"),
                        );
                    }
                });
                ui.separator();
                ui.strong(tr("dialog.prepress"));
                ui.checkbox(&mut st.separations, tr("dialog.separations"));
                ui.checkbox(&mut st.bleed, tr("dialog.print_bleed"));
                ui.checkbox(&mut st.crop_marks, tr("dialog.crop_marks"));
                ui.checkbox(&mut st.registration_marks, tr("dialog.registration_marks"));
                ui.checkbox(&mut st.color_bars, tr("print.color_bars"));
                ui.checkbox(&mut st.densitometer, tr("print.densitometer"));
                ui.checkbox(&mut st.page_info, tr("print.page_info_mark"));
                ui.checkbox(&mut st.mirror, tr("dialog.mirror"));
                ui.checkbox(&mut st.invert, tr("dialog.invert"));
                ui.separator();
                crate::ui::dialogs::print_layout_fields(app, ui, st);
            });
        });

    egui::CentralPanel::default()
        .frame(egui::Frame::new().fill(egui::Color32::from_gray(90)))
        .show(ui, |ui| {
            let area = ui.available_rect_before_wrap();
            let response = ui
                .interact(
                    area,
                    ui.id().with("print_preview_view"),
                    egui::Sense::click_and_drag(),
                )
                .on_hover_text(tr("print.view_hint"));
            let separations = pv.st.separations;
            let (sheet, plate) = (pv.sheet, pv.plate);
            let Some(page) = shown_page(sheets(app, &mut pv), sheet, plate, separations) else {
                return;
            };
            let margin = 24.0;
            let fit = ((area.width() - 2.0 * margin) as f64 / page.size.width.max(1e-6))
                .min((area.height() - 2.0 * margin) as f64 / page.size.height.max(1e-6))
                .max(0.01);
            if pv.real_size {
                pv.real_size = false;
                pv.zoom = real_size_zoom(real, fit as f32);
                pv.pan = egui::Vec2::ZERO;
            }
            navigate(&mut pv, ui, &response, area);
            // Points per millimetre on screen.
            let scale = fit * pv.zoom as f64;
            let size = egui::vec2(
                (page.size.width * scale) as f32,
                (page.size.height * scale) as f32,
            );
            pv.pan = clamp_pan(pv.pan, size, area.size());
            let sheet_rect = egui::Rect::from_center_size(area.center() + pv.pan, size);
            let ppp = ctx.pixels_per_point() as f64;
            let px = scale * ppp;
            // The whole sheet while its bitmap stays small (panning then
            // only moves it), else just the part on screen.
            let whole = page.size.width.max(page.size.height) * px <= WHOLE_SHEET_PX;
            let target = if whole {
                sheet_rect
            } else {
                sheet_rect.intersect(area)
            };
            let p = ui.painter_at(area);
            p.rect_filled(
                sheet_rect.translate(egui::vec2(4.0, 4.0)),
                0.0,
                egui::Color32::from_black_alpha(90),
            );
            if target.width() >= 1.0 && target.height() >= 1.0 {
                let w = (target.width() as f64 * ppp).round().max(1.0) as u32;
                let h = (target.height() as f64 * ppp).round().max(1.0) as u32;
                let ox = (sheet_rect.min.x - target.min.x) as f64 * ppp;
                let oy = (sheet_rect.min.y - target.min.y) as f64 * ppp + page.size.height * px;
                let key = format!(
                    "{}|{sheet}|{plate}|{w}x{h}|{px:.4}|{ox:.1}|{oy:.1}",
                    pv.built.as_ref().map(|(k, _)| k.as_str()).unwrap_or("")
                );
                if pv.texture.as_ref().map(|(k, _)| k != &key).unwrap_or(true) {
                    if let Some(img) = render_sheet(&page, w, h, px, ox, oy) {
                        let tex =
                            ctx.load_texture("print_preview", img, egui::TextureOptions::LINEAR);
                        pv.texture = Some((key, tex));
                    }
                }
                if let Some((_, tex)) = &pv.texture {
                    p.image(
                        tex.id(),
                        target,
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        egui::Color32::WHITE,
                    );
                }
            }
            if let Some((a, b)) = pv.marquee {
                p.rect_stroke(
                    egui::Rect::from_two_pos(a, b),
                    0.0,
                    egui::Stroke::new(1.0, Tokens::ACCENT),
                    egui::StrokeKind::Inside,
                );
            }
        });

    let (prev, next, esc) = ctx.input(|i| {
        (
            i.key_pressed(egui::Key::PageUp),
            i.key_pressed(egui::Key::PageDown),
            i.key_pressed(egui::Key::Escape),
        )
    });
    if prev {
        pv.sheet = pv.sheet.saturating_sub(1);
    }
    if next {
        pv.sheet += 1;
    }
    if esc {
        close = true;
    }
    if print {
        match crate::export::print(app, &pv.st) {
            Ok(msg) => app.status = msg,
            Err(e) => app.status = format!("{}: {e}", tr("dialog.print_failed")),
        }
        if app.settings.printing.remember_last {
            app.settings.printing.remember(&pv.st);
            app.settings.save();
        }
        close = true;
    }
    if !close {
        app.print_preview = Some(pv);
    }
}

/// The sheet as a bitmap on white paper: `w` by `h` pixels at `zoom`
/// pixels per millimetre, the sheet's left edge at `origin_x` and its
/// bottom at `origin_y`, so that a part of the sheet can be rendered.
fn render_sheet(
    page: &graphdraw_core::document::Page,
    w: u32,
    h: u32,
    zoom: f64,
    origin_x: f64,
    origin_y: f64,
) -> Option<egui::ColorImage> {
    let mut doc = Document::new("sheet", page.size);
    doc.pages = vec![page.clone()];
    let opts = graphdraw_render::RenderOptions {
        width: w,
        height: h,
        view: graphdraw_render::ViewTransform {
            zoom,
            origin_x,
            origin_y,
        },
        preview: None,
        wireframe: false,
        ..graphdraw_render::RenderOptions::default()
    };
    let pm = graphdraw_render::render_page(&doc, page.id, &opts)?;
    let mut white = tiny_skia::Pixmap::new(pm.width(), pm.height())?;
    white.fill(tiny_skia::Color::WHITE);
    white.draw_pixmap(
        0,
        0,
        pm.as_ref(),
        &Default::default(),
        tiny_skia::Transform::identity(),
        None,
    );
    Some(egui::ColorImage::from_rgba_premultiplied(
        [white.width() as usize, white.height() as usize],
        white.data(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use graphdraw_core::{Color, Fill, Shape, ShapeKind};

    #[test]
    fn plates_split_the_sheet_by_ink() {
        let mut doc = Document::new("t", graphdraw_core::geometry::Size::new(100.0, 100.0));
        let id = doc.ids_mut().shape();
        let mut s = Shape::new(
            id,
            ShapeKind::Rect {
                rect: graphdraw_core::geometry::Rect::new(0.0, 0.0, 10.0, 10.0),
                radius: 0.0,
                corners: None,
            },
        );
        s.fill = Fill::Solid(Color::Cmyk {
            c: 1.0,
            m: 0.5,
            y: 0.0,
            k: 0.0,
        });
        doc.pages[0].layers[0].shapes.push(s);
        let grey = |plate| {
            let p = shown_page(&doc, 0, plate, true).expect("page");
            match p.layers[0].shapes[0].fill {
                Fill::Solid(Color::Gray { v }) => v,
                ref f => panic!("{f:?}"),
            }
        };
        assert!((grey(1) - 0.0).abs() < 1e-6, "cyan plate full");
        assert!((grey(2) - 0.5).abs() < 1e-6, "magenta plate half");
        assert!((grey(3) - 1.0).abs() < 1e-6, "yellow plate empty");
        // Composite, and separations off, show the sheet as it is.
        let p = shown_page(&doc, 0, 0, true).expect("page");
        assert!(matches!(
            p.layers[0].shapes[0].fill,
            Fill::Solid(Color::Cmyk { .. })
        ));
        let p = shown_page(&doc, 0, 2, false).expect("page");
        assert!(matches!(
            p.layers[0].shapes[0].fill,
            Fill::Solid(Color::Cmyk { .. })
        ));
        assert!(shown_page(&doc, 5, 0, false).is_none());
    }

    #[test]
    fn zooming_keeps_the_point_under_the_pointer() {
        let pan = egui::vec2(30.0, -10.0);
        let at = egui::vec2(100.0, 50.0);
        // The sheet point under `at`, in fitted-size units.
        let under = |zoom: f32, pan: egui::Vec2| (at - pan) / zoom;
        let (z, p) = zoom_about(1.0, pan, 2.0, at);
        assert!((z - 2.0).abs() < 1e-6);
        assert!((under(z, p) - under(1.0, pan)).length() < 1e-4);
        // Past the limits the zoom stops and the point still holds.
        let (z, p) = zoom_about(MAX_ZOOM, pan, 2.0, at);
        assert!((z - MAX_ZOOM).abs() < 1e-6);
        assert!((p - pan).length() < 1e-4);
        let (z, _) = zoom_about(MIN_ZOOM, pan, 0.5, at);
        assert!((z - MIN_ZOOM).abs() < 1e-6);
    }

    #[test]
    fn a_zoom_rectangle_fills_the_view_centred() {
        let view = egui::vec2(800.0, 600.0);
        // A 100 x 50 rectangle right of centre; the height limits it.
        let r = egui::Rect::from_center_size(egui::pos2(200.0, 0.0), egui::vec2(100.0, 50.0));
        let (z, p) = zoom_to_rect(1.0, egui::Vec2::ZERO, r, view);
        assert!((z - 8.0).abs() < 1e-5, "{z}");
        // The rectangle's centre now sits in the view's centre.
        let centre = (r.center().to_vec2() - egui::Vec2::ZERO) * z + p;
        assert!(centre.length() < 1e-3, "{centre:?}");
    }

    #[test]
    fn panning_keeps_part_of_the_sheet_on_screen() {
        let view = egui::vec2(800.0, 600.0);
        let sheet = egui::vec2(400.0, 300.0);
        let p = clamp_pan(egui::vec2(5000.0, -5000.0), sheet, view);
        assert!((p.x - (600.0 - KEEP_VISIBLE)).abs() < 1e-4);
        assert!((p.y + (450.0 - KEEP_VISIBLE)).abs() < 1e-4);
        let small = egui::vec2(10.0, 10.0);
        assert_eq!(clamp_pan(egui::vec2(1.0, 2.0), small, small), egui::Vec2::ZERO);
    }

    #[test]
    fn real_size_follows_the_ruler_calibration() {
        // 96 points per inch, fitted at 2 points per mm: real size is
        // 3.78 points per mm, 189 % of the fit.
        let real = 96.0 / 25.4;
        assert!((real_size_zoom(real, 2.0) - real / 2.0).abs() < 1e-6);
        assert!((real_size_zoom(real, 1e-3) - MAX_ZOOM).abs() < 1e-6);
    }

    #[test]
    fn a_part_of_the_sheet_renders_like_the_same_part_of_the_whole() {
        let mut doc = Document::new("t", graphdraw_core::geometry::Size::new(20.0, 20.0));
        let id = doc.ids_mut().shape();
        let mut s = Shape::new(
            id,
            ShapeKind::Rect {
                rect: graphdraw_core::geometry::Rect::new(10.0, 0.0, 20.0, 10.0),
                radius: 0.0,
                corners: None,
            },
        );
        s.fill = Fill::Solid(Color::BLACK);
        doc.pages[0].layers[0].shapes.push(s);
        let page = &doc.pages[0];
        // Whole sheet at 2 px per mm: the black square fills the bottom
        // right quarter (y up in the model).
        let whole = render_sheet(page, 40, 40, 2.0, 0.0, 40.0).expect("whole");
        // The bottom right quarter alone.
        let part = render_sheet(page, 20, 20, 2.0, -20.0, 20.0).expect("part");
        assert_eq!(whole.pixels[30 * 40 + 30], part.pixels[10 * 20 + 10]);
        assert_eq!(part.pixels[10 * 20 + 10], egui::Color32::BLACK);
        assert_eq!(whole.pixels[5 * 40 + 5], egui::Color32::WHITE);
    }

    #[test]
    fn the_settings_key_ignores_copies_and_preflight() {
        let a = PrintState::default();
        let mut b = a.clone();
        b.copies = 4;
        assert_eq!(settings_key(&a), settings_key(&b));
        b.crop_marks = true;
        assert_ne!(settings_key(&a), settings_key(&b));
    }
}
