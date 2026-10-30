//! The clip frame bar: with clip frames selected, a small bar under them
//! (above them when there is no room below) with Edit Contents, Center
//! Contents, Fill Frame Proportionally, Fit Contents Proportionally,
//! Extract Contents and Lock Contents to Frame. While contents are edited
//! it shows the level being edited and Finish. A notice that a clip frame
//! request did nothing shows next to the pointer for a moment.
//! See docs/behavior/clip-frame.md.

use crate::app::{App, Drag};
use crate::clip_frames::ContentFit;
use crate::i18n::{tr, trf};
use crate::theme::Tokens;
use crate::tools::Tool;
use crate::ui::svg_icons::{paint, IconRef};
use egui::{Context, Pos2, Rect, RichText, Sense, Vec2};

/// Side of a bar button (points).
const BUTTON: f32 = 28.0;
/// Gap between the objects and the bar (points), clear of the handles.
const GAP: f32 = 18.0;
/// How long a notice stays next to the pointer (seconds).
const NOTICE_SECONDS: f64 = 2.5;

#[derive(Clone, Copy)]
enum Action {
    Edit,
    Fit(ContentFit),
    Extract,
    Lock,
    Finish,
}

pub fn show(app: &mut App, ctx: &Context) {
    show_notice(app, ctx);
    let idle = matches!(app.tool, Tool::Pick | Tool::FreeformPick)
        && matches!(app.drag, Drag::None)
        && app.text_edit.is_none()
        && !app.pending_clip_frame;
    if !idle {
        return;
    }
    let editing = app.clip_frame_edit_depth();
    let anchor = match (&app.clip_frame_edit, editing) {
        (Some((frame, _)), Some(_)) => app.doc().shape_on_page(*frame).map(|s| s.bounds()),
        _ if !app.selected_clip_frames().is_empty() => app.selection_bounds(),
        _ => None,
    };
    let Some(anchor) = anchor else {
        return;
    };
    let screen = app.view.rect_to_screen(anchor);
    let canvas = app.canvas_rect;
    if !canvas.intersects(screen) {
        return;
    }
    let locked = app
        .selected_clip_frames()
        .iter()
        .all(|s| app.clip_frame_locked(s.id));
    let mut chosen = None;
    let area = egui::Area::new(egui::Id::new("clip_frame_bar"))
        .order(egui::Order::Foreground)
        .pivot(egui::Align2::CENTER_TOP)
        .fixed_pos(bar_position(screen, canvas, ctx))
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(Tokens::panel())
                .stroke(egui::Stroke::new(1.0, Tokens::border()))
                .corner_radius(6.0)
                .inner_margin(egui::Margin::symmetric(6, 4))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        match editing {
                            Some((level, levels)) => {
                                editing_row(ui, level, levels, &mut chosen);
                            }
                            None => frame_row(ui, locked, &mut chosen),
                        }
                    });
                });
        });
    // The bar keeps clear of the drawing window's edges once its size is
    // known (the first frame it shows uses an estimate).
    ctx.data_mut(|d| d.insert_temp(egui::Id::new("clip_frame_bar_size"), area.response.rect.size()));
    match chosen {
        Some(Action::Edit) => app.edit_clip_frame(),
        Some(Action::Fit(how)) => app.fit_clip_contents(how),
        Some(Action::Extract) => app.extract_clip_contents(),
        Some(Action::Lock) => app.toggle_clip_frame_lock(),
        Some(Action::Finish) => app.finish_clip_frame_edit(),
        None => {}
    }
}

/// Where the bar's top centre goes: under the objects at `screen`, or
/// above them when it would leave the drawing window, kept inside it.
fn bar_position(screen: Rect, canvas: Rect, ctx: &Context) -> Pos2 {
    let size = ctx
        .data(|d| d.get_temp::<Vec2>(egui::Id::new("clip_frame_bar_size")))
        .unwrap_or(egui::vec2(6.0 * BUTTON + 24.0, BUTTON + 10.0));
    let below = screen.bottom() + GAP;
    let y = if below + size.y <= canvas.bottom() - 4.0 {
        below
    } else {
        (screen.top() - GAP - size.y).max(canvas.top() + 4.0)
    };
    let half = size.x / 2.0;
    let x = screen
        .center()
        .x
        .clamp(canvas.left() + half + 4.0, (canvas.right() - half - 4.0).max(canvas.left() + half + 4.0));
    egui::pos2(x, y.min(canvas.bottom() - size.y - 4.0).max(canvas.top() + 4.0))
}

/// The buttons for selected clip frames.
fn frame_row(ui: &mut egui::Ui, locked: bool, chosen: &mut Option<Action>) {
    let edit_tip = trf(
        "clipbar.with_shortcut",
        &[("label", &tr("menu.object.clip_frame_edit")), ("keys", "Ctrl+Click")],
    );
    let rows: [(&str, String, Action); 5] = [
        ("menu.object.clip_frame_edit", edit_tip, Action::Edit),
        (
            "menu.object.clip_frame_center",
            tr("menu.object.clip_frame_center"),
            Action::Fit(ContentFit::Center),
        ),
        (
            "menu.object.clip_frame_fill",
            tr("menu.object.clip_frame_fill"),
            Action::Fit(ContentFit::Fill),
        ),
        (
            "menu.object.clip_frame_fit",
            tr("menu.object.clip_frame_fit"),
            Action::Fit(ContentFit::Fit),
        ),
        (
            "menu.object.clip_frame_extract",
            tr("menu.object.clip_frame_extract"),
            Action::Extract,
        ),
    ];
    for (key, tip, action) in rows {
        let icon = crate::ui::menus::menu_icon_ref(key);
        if icon_button(ui, icon, &tip) {
            *chosen = Some(action);
        }
    }
    ui.separator();
    let lock_tip = if locked {
        tr("clipbar.locked")
    } else {
        tr("clipbar.unlocked")
    };
    let lock = IconRef::Pic(crate::ui::propbar::Pic::Lock(locked));
    if icon_button(ui, Some(lock), &lock_tip) {
        *chosen = Some(Action::Lock);
    }
}

/// While contents are edited: the level and Finish.
fn editing_row(ui: &mut egui::Ui, level: usize, levels: usize, chosen: &mut Option<Action>) {
    let finish_tip = trf(
        "clipbar.with_shortcut",
        &[("label", &tr("menu.object.clip_frame_finish")), ("keys", "Esc")],
    );
    let finish = crate::ui::menus::menu_icon_ref("menu.object.clip_frame_finish");
    if icon_button(ui, finish, &finish_tip) {
        *chosen = Some(Action::Finish);
    }
    ui.separator();
    let n = level.to_string();
    let m = levels.max(level).to_string();
    ui.label(
        RichText::new(trf("clipbar.editing", &[("n", &n), ("m", &m)]))
            .color(Tokens::text()),
    );
    ui.separator();
    keycap(ui, "Ctrl");
    ui.label(RichText::new(tr("clipbar.ctrl_click_out")).color(Tokens::text_dim()));
}

/// A square button showing `icon`, with `tip` as its hover text and as
/// what screen readers announce; true when clicked or pressed from the
/// keyboard.
pub(crate) fn icon_button(ui: &mut egui::Ui, icon: Option<IconRef<'_>>, tip: &str) -> bool {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(BUTTON, BUTTON), Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, tip));
    if response.hovered() || response.has_focus() {
        ui.painter()
            .rect_filled(rect, 4.0, Tokens::tool_active().gamma_multiply(0.6));
    }
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect,
            4.0,
            egui::Stroke::new(1.5, Tokens::ACCENT),
            egui::StrokeKind::Inside,
        );
    }
    if let Some(icon) = icon {
        paint(
            ui.painter(),
            Rect::from_center_size(rect.center(), egui::vec2(20.0, 20.0)),
            icon,
            Tokens::text(),
        );
    }
    let response = response.on_hover_text(tip);
    response.clicked()
}

/// A key drawn as a key cap, for the shortcut hint.
fn keycap(ui: &mut egui::Ui, key: &str) {
    egui::Frame::new()
        .stroke(egui::Stroke::new(1.0, Tokens::text_dim()))
        .corner_radius(3.0)
        .inner_margin(egui::Margin::symmetric(4, 0))
        .show(ui, |ui| {
            ui.label(RichText::new(key).size(11.0).color(Tokens::text()));
        });
}

/// The notice of [`App::clip_notice`], next to the pointer for a moment.
fn show_notice(app: &mut App, ctx: &Context) {
    let Some((text, since)) = app.clip_notice.clone() else {
        return;
    };
    let now = ctx.input(|i| i.time);
    let since = since.unwrap_or(now);
    if now - since > NOTICE_SECONDS {
        app.clip_notice = None;
        return;
    }
    app.clip_notice = Some((text.clone(), Some(since)));
    let at = ctx
        .pointer_latest_pos()
        .unwrap_or(app.canvas_rect.center())
        + egui::vec2(18.0, 22.0);
    egui::Area::new(egui::Id::new("clip_frame_notice"))
        .order(egui::Order::Tooltip)
        .fixed_pos(at)
        .interactable(false)
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.label(RichText::new(text).color(Tokens::text()));
            });
        });
    ctx.request_repaint_after(std::time::Duration::from_millis(250));
}
