//! The eyedroppers on screen: their property bar parts (mode, sample
//! size, the bucket and its Empty button), the floating magnifier that
//! follows the pointer (magnified screen pixels with the sample box, the
//! colour's HEX, CMYK and RGB values and name, the bucket, the last
//! tones), the fountain stops the colour bucket can hit, and the short
//! highlight on an object that received the bucket. The sampling and
//! applying are in `crate::eyedropper`.

use crate::app::App;
use crate::eyedropper::{self as ed, HoverInfo, CELL, GRID, SAMPLE_SIZES};
use crate::i18n::tr;
use crate::theme::Tokens;
use crate::tools::Tool;
use egui::{Color32, Painter, Pos2, Rect, Sense, Stroke, Ui, Vec2};
use graphdraw_core::{geometry::Point, Color, Fill};

fn c32(c: Color) -> Color32 {
    let [r, g, b] = c.to_rgb8();
    Color32::from_rgb(r, g, b)
}

fn dim(ui: &mut Ui, text: String) {
    ui.label(
        egui::RichText::new(text)
            .color(Tokens::text_dim())
            .size(11.0),
    );
}

/// The label of a sample size.
pub fn sample_label(n: u32) -> String {
    if n == 0 {
        tr("toolbar.eye_native")
    } else {
        format!("{n}x{n}")
    }
}

/// The two modes: Colour and Attributes (the two tools, with their own
/// icons); switching empties the bucket.
pub fn mode_switch(app: &mut App, ui: &mut Ui) {
    for (tool, key) in [
        (Tool::ColorEyedropper, "toolbar.eye_mode_color"),
        (Tool::AttributesEyedropper, "toolbar.eye_mode_attrs"),
    ] {
        let (rect, resp) =
            ui.allocate_exact_size(crate::ui::style_windows::ICON_BUTTON, Sense::click());
        if app.tool == tool {
            ui.painter().rect_filled(rect, 2.0, Tokens::tool_active());
        } else if resp.hovered() {
            ui.painter().rect_filled(rect, 2.0, Tokens::tool_hover());
        }
        crate::ui::icons::draw(
            ui.painter(),
            Rect::from_center_size(rect.center(), Vec2::splat(20.0)),
            tool,
            Tokens::icon(),
        );
        if resp.on_hover_text(tr(key)).clicked() {
            app.set_tool(tool);
        }
    }
}

/// The sample size list and the sampling outside the window (not yet
/// available: it needs a screen capture of the system).
pub fn sample_part(app: &mut App, ui: &mut Ui) {
    dim(ui, tr("toolbar.sample_size"));
    egui::ComboBox::from_id_salt("eyedropper_sample_size")
        .selected_text(sample_label(app.eyedropper_sample))
        .show_ui(ui, |ui| {
            for n in SAMPLE_SIZES {
                ui.selectable_value(&mut app.eyedropper_sample, n, sample_label(n))
                    .on_hover_text(if n == 0 {
                        tr("toolbar.eye_native_tip")
                    } else {
                        tr("toolbar.eye_average_tip")
                    });
            }
        });
    ui.add_enabled(false, egui::Button::new(tr("toolbar.eye_outside")))
        .on_disabled_hover_text(tr("toolbar.eye_outside_tip"));
}

/// What the bucket holds (a cloned ramp shows as its strip) and the
/// button that empties it.
pub fn bucket_part(app: &mut App, ui: &mut Ui) {
    if let Some(f) = &app.eyedrop.ramp {
        let (r, _) = ui.allocate_exact_size(Vec2::new(48.0, 18.0), Sense::hover());
        let n = 24;
        for i in 0..n {
            let t = (i as f64 + 0.5) / n as f64;
            let x0 = r.min.x + r.width() * i as f32 / n as f32;
            let x1 = r.min.x + r.width() * (i + 1) as f32 / n as f32;
            ui.painter().rect_filled(
                Rect::from_min_max(Pos2::new(x0, r.min.y), Pos2::new(x1, r.max.y)),
                0.0,
                c32(f.color_at(t)),
            );
        }
        ui.painter().rect_stroke(
            r,
            2.0,
            Stroke::new(1.0, Tokens::border()),
            egui::StrokeKind::Inside,
        );
        dim(ui, tr("toolbar.eye_ramp"));
    }
    if app.eyedropper_has_bucket()
        && crate::ui::style_windows::icon_button(
            ui,
            "toolbar.eye_empty",
            &tr("toolbar.eye_empty"),
            false,
            true,
        )
        .clicked()
    {
        app.eyedropper_reset();
    }
}

/// The gestures, shown on the bar.
pub fn hint(ui: &mut Ui, attributes: bool) {
    dim(
        ui,
        tr(if attributes {
            "hint.bar_eyedrop_attrs"
        } else {
            "hint.bar_eyedrop_cycle"
        }),
    );
}

/// What the magnifier asked for.
enum HudAction {
    AddToPalette(Color),
    Recent(Color),
}

/// The floating magnifier.
fn hud_ui(ctx: &egui::Context, app: &App, info: &HoverInfo) -> (Option<HudAction>, Rect) {
    let mut act = None;
    let e = &app.eyedrop;
    let side_px = app.eyedropper_sample.max(1) as f32;
    let area = egui::Area::new(egui::Id::new("eyedropper_magnifier"))
        .order(egui::Order::Foreground)
        .fixed_pos(e.hud.pos)
        .interactable(true)
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.spacing_mut().item_spacing = Vec2::splat(4.0);
                let side = CELL * GRID as f32;
                let (r, resp) = ui.allocate_exact_size(Vec2::splat(side), Sense::click());
                let p = ui.painter_at(r);
                for (i, c) in info.grid.iter().enumerate() {
                    let (x, y) = ((i % GRID) as f32, (i / GRID) as f32);
                    let cr = Rect::from_min_size(
                        r.min + Vec2::new(x * CELL, y * CELL),
                        Vec2::splat(CELL),
                    );
                    p.rect_filled(cr, 0.0, Color32::from_rgb(c[0], c[1], c[2]));
                }
                let line = Stroke::new(1.0, Color32::from_black_alpha(28));
                for k in 1..GRID {
                    let d = k as f32 * CELL;
                    p.vline(r.min.x + d, r.y_range(), line);
                    p.hline(r.x_range(), r.min.y + d, line);
                }
                // The sample box around the middle pixel (an even box grows
                // right and down, as the sample does).
                let c0 = (GRID / 2) as f32 * CELL;
                let half = ((side_px - 1.0) / 2.0).floor();
                let reticle = Rect::from_min_size(
                    r.min + Vec2::splat(c0 - half * CELL),
                    Vec2::splat(side_px * CELL),
                );
                p.rect_stroke(
                    reticle,
                    0.0,
                    Stroke::new(1.5, Color32::WHITE),
                    egui::StrokeKind::Outside,
                );
                p.rect_stroke(
                    reticle.expand(1.5),
                    0.0,
                    Stroke::new(1.0, Color32::BLACK),
                    egui::StrokeKind::Outside,
                );
                let resp = resp.on_hover_text(tr("eyedrop.hud_palette_tip"));
                if resp.secondary_clicked() {
                    act = Some(HudAction::AddToPalette(info.color));
                }
                ui.horizontal(|ui| {
                    let (sr, _) = ui.allocate_exact_size(Vec2::splat(14.0), Sense::hover());
                    ui.painter().rect_filled(sr, 2.0, c32(info.color));
                    ui.painter().rect_stroke(
                        sr,
                        2.0,
                        Stroke::new(1.0, Tokens::border()),
                        egui::StrokeKind::Inside,
                    );
                    ui.strong(info.color.to_hex().to_uppercase());
                });
                let (cmyk, rgb) = ed::values_text(info.color);
                ui.label(egui::RichText::new(cmyk).size(10.0));
                ui.label(egui::RichText::new(rgb).size(10.0));
                if let Some(n) = &info.name {
                    ui.label(egui::RichText::new(n).size(10.0).color(Tokens::text_dim()));
                }
                if app.eyedropper_has_bucket() {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(tr("eyedrop.bucket")).size(10.0));
                        if app.tool == Tool::AttributesEyedropper {
                            ui.label(egui::RichText::new(tr("eyedrop.attributes")).size(10.0));
                        } else if app.eyedrop.ramp.is_some() {
                            ui.label(egui::RichText::new(tr("toolbar.eye_ramp")).size(10.0));
                        } else if let Some(c) = app.eyedropper_color {
                            let (sr, _) = ui.allocate_exact_size(Vec2::splat(14.0), Sense::hover());
                            ui.painter().rect_filled(sr, 2.0, c32(c));
                        }
                    });
                }
                if !e.recent.is_empty() {
                    ui.horizontal(|ui| {
                        for c in e.recent.iter().copied() {
                            let (r, resp) =
                                ui.allocate_exact_size(Vec2::splat(14.0), Sense::click());
                            ui.painter()
                                .circle_filled(r.center(), r.width() / 2.0, c32(c));
                            ui.painter().circle_stroke(
                                r.center(),
                                r.width() / 2.0,
                                Stroke::new(1.0, Tokens::border()),
                            );
                            if resp.on_hover_text(c.to_hex().to_uppercase()).clicked() {
                                act = Some(HudAction::Recent(c));
                            }
                        }
                    });
                }
            });
        });
    (act, area.response.rect)
}

/// Every frame on the canvas: the apply highlight, and with an
/// eyedropper the stops the colour bucket can hit and the magnifier.
pub fn canvas_overlay(app: &mut App, ui: &mut Ui, painter: &Painter, canvas: Rect) {
    let active = ed::is_eyedropper(app.tool);
    app.raster.borrow_mut().keep_pixels = active;
    app.eyedrop.now = ui.input(|i| i.time);
    // The highlight on what received the bucket, fading out.
    match app.eyedrop.ripple_phase() {
        Some(k) => {
            if let Some((ids, _)) = app.eyedrop.ripple.clone() {
                let c = Tokens::SELECTION.gamma_multiply((1.0 - k) as f32);
                for id in ids {
                    if let Some(s) = app.eyedrop_page_shape(id) {
                        let r = app.view.rect_to_screen(s.bounds());
                        painter.rect_stroke(
                            r.expand(6.0 * k as f32),
                            2.0,
                            Stroke::new(2.0, c),
                            egui::StrokeKind::Outside,
                        );
                    }
                }
            }
            ui.ctx().request_repaint();
        }
        None => app.eyedrop.ripple = None,
    }
    if !active {
        app.eyedrop.hud.rect = Rect::NOTHING;
        return;
    }
    let (pointer, shift) = ui.input(|i| (i.pointer.hover_pos(), i.modifiers.shift));
    let on_canvas = pointer.filter(|q| {
        canvas.contains(*q)
            && !app.eyedrop.over_hud(*q)
            && app.context_menu.is_none()
            && !egui::Popup::is_any_open(ui.ctx())
    });
    if let Some(pos) = on_canvas {
        let p = app.view.to_page(pos);
        let size = if app.eyedrop.hud.rect.is_positive() {
            app.eyedrop.hud.rect.size()
        } else {
            Vec2::new(130.0, 200.0)
        };
        let now = app.eyedrop.now;
        app.eyedrop.hud.follow(pos, now, size, canvas);
        if !app.eyedrop.hud.frozen {
            if let Some(color) = app.sample_color_at(p, false) {
                let grid = app
                    .eyedrop_screen_grid(p)
                    .unwrap_or_else(|| vec![color.to_rgb8(); GRID * GRID]);
                let name = app.eyedrop_color_name(color);
                app.eyedrop.info = Some(HoverInfo { color, grid, name });
            }
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_secs_f64(ed::DWELL_S));
        }
        // The stops of the fountain under the pointer, for the colour
        // bucket.
        if app.tool == Tool::ColorEyedropper
            && !app.eyedropper_sampling(shift)
            && app.eyedrop.ramp.is_none()
        {
            draw_stops(app, painter, p);
        }
    }
    let show = on_canvas.is_some() || pointer.is_some_and(|q| app.eyedrop.over_hud(q));
    if !show {
        app.eyedrop.hud.rect = Rect::NOTHING;
        return;
    }
    let Some(info) = app.eyedrop.info.clone() else {
        return;
    };
    let (act, rect) = hud_ui(ui.ctx(), app, &info);
    app.eyedrop.hud.rect = rect;
    match act {
        Some(HudAction::AddToPalette(c)) => {
            app.add_to_document_palette(c);
            app.status = crate::i18n::trf(
                "status.eyedrop_palette",
                &[("c", &c.to_hex().to_uppercase())],
            );
        }
        Some(HudAction::Recent(c)) => app.eyedrop_load_color(c),
        None => {}
    }
}

/// The stops of the fountain fill under `p`, as squares in their colours.
fn draw_stops(app: &App, painter: &Painter, p: Point) {
    let Some(s) = app
        .eyedrop_deep_hit(p)
        .and_then(|id| app.eyedrop_page_shape(id))
    else {
        return;
    };
    let Fill::Fountain(f) = &s.fill else {
        return;
    };
    let hit = app.fountain_stop_at(p).map(|(_, i)| i);
    for (i, q) in ed::stop_points(f, s.bounds()) {
        let sp = app.view.to_screen(q);
        let r = Rect::from_center_size(sp, Vec2::splat(if hit == Some(i) { 11.0 } else { 9.0 }));
        let c = f.stops.get(i).map(|st| st.color).unwrap_or(Color::BLACK);
        painter.rect_filled(r, 0.0, c32(c));
        painter.rect_stroke(
            r,
            0.0,
            Stroke::new(1.0, Tokens::SELECTION),
            egui::StrokeKind::Outside,
        );
    }
}
