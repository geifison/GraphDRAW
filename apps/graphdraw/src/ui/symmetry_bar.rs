//! The symmetry bar: while a symmetry is edited (Object > Symmetry > Edit
//! Symmetry), a bar at the top left of the drawing window with Finish,
//! the number of mirror lines, the centre and the angle.

use crate::app::App;
use crate::i18n::tr;
use crate::theme::Tokens;
use egui::{Color32, Context, RichText};

const GREEN: Color32 = Color32::from_rgb(0x1E, 0x9E, 0x3C);

pub fn show(app: &mut App, ctx: &Context) {
    let Some(_) = app.symmetry_editing() else {
        return;
    };
    let Some((_, center, angle, lines)) = app.selected_symmetry() else {
        return;
    };
    let at = app.canvas_rect.min + egui::vec2(14.0, 14.0);
    egui::Area::new(egui::Id::new("symmetry_bar"))
        .fixed_pos(at)
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(Tokens::panel())
                .stroke(egui::Stroke::new(1.0, Tokens::border()))
                .inner_margin(egui::Margin::symmetric(10, 8))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let finish = ui
                            .add(
                                egui::Button::new(
                                    RichText::new(format!("      {}", tr("symmetry.finish")))
                                        .color(Tokens::text()),
                                )
                                .frame(false),
                            )
                            .on_hover_text(tr("menu.object.symmetry_finish"));
                        // The tick in green, over the label's start.
                        let r = finish.rect;
                        ui.painter().text(
                            egui::pos2(r.left() + 2.0, r.center().y),
                            egui::Align2::LEFT_CENTER,
                            "\u{2714}",
                            egui::FontId::proportional(16.0),
                            GREEN,
                        );
                        if finish.clicked() {
                            app.finish_symmetry_edit();
                            return;
                        }
                        ui.separator();
                        let (mut n, mut a) = (lines, angle);
                        let (mut c, mut changed) = (center, false);
                        changed |= ui
                            .add(egui::DragValue::new(&mut n).range(1..=12))
                            .on_hover_text(tr("dialog.mirror_lines"))
                            .changed();
                        ui.separator();
                        let u = app.units;
                        let (mut x, mut y) = (u.world_from_mm(c.x), u.world_from_mm(c.y));
                        ui.vertical(|ui| {
                            let cx = ui
                                .add(
                                    egui::DragValue::new(&mut x)
                                        .speed(0.5)
                                        .prefix("X: ")
                                        .suffix(format!(" {}", u.short())),
                                )
                                .changed();
                            let cy = ui
                                .add(
                                    egui::DragValue::new(&mut y)
                                        .speed(0.5)
                                        .prefix("Y: ")
                                        .suffix(format!(" {}", u.short())),
                                )
                                .changed();
                            if cx || cy {
                                c = tracedraw_core::geometry::Point::new(
                                    u.world_to_mm(x),
                                    u.world_to_mm(y),
                                );
                                changed = true;
                            }
                        });
                        ui.separator();
                        changed |= ui
                            .add(
                                egui::DragValue::new(&mut a)
                                    .speed(1.0)
                                    .range(0.0..=360.0)
                                    .prefix("\u{21BB} ")
                                    .suffix("°"),
                            )
                            .on_hover_text(tr("dialog.mirror_angle"))
                            .changed();
                        if changed {
                            app.set_symmetry(c, a, n);
                        }
                    });
                });
        });
}
