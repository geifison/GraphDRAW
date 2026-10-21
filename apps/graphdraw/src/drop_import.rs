//! Files dropped on the window from the file manager (or a web page that
//! hands over a file): each is imported and embedded, centred where it
//! fell, the next ones a duplicate offset apart so they do not hide each
//! other. While files hover over the window a dashed frame and a hint
//! show the drop will import them. Unsupported files are named in one
//! alert.

use crate::app::App;
use crate::i18n::{tr, trf};
use crate::theme::Tokens;
use egui::{Color32, Context, Painter, Rect as ERect, Stroke};
use graphdraw_core::{
    geometry::{Affine, Point},
    Command,
};

impl App {
    /// Import the files dropped on the window this frame. A browser reads
    /// them in the background; each is imported when it has arrived.
    pub fn take_dropped_files(&mut self, ctx: &Context) {
        let files = ctx.input_mut(|i| std::mem::take(&mut i.raw.dropped_files));
        if files.is_empty() {
            return;
        }
        let pos = pointer_now(ctx);
        let at = pos
            .filter(|p| self.canvas_rect.contains(*p))
            .map(|p| self.view.to_page(p));
        #[cfg(not(target_arch = "wasm32"))]
        self.import_dropped(files.iter().map(|f| f.path().to_path_buf()).collect(), at);
        #[cfg(target_arch = "wasm32")]
        for f in files {
            crate::files::read_dropped(f, move |app, path| app.import_dropped(vec![path], at));
        }
    }

    /// Import `paths`, the first centred on `at` (page space; the view
    /// centre when the drop missed the canvas), all as one undo step;
    /// every imported object ends up selected.
    pub fn import_dropped(&mut self, paths: Vec<std::path::PathBuf>, at: Option<Point>) {
        self.as_one_step("Import", |app| app.import_dropped_now(paths, at));
    }

    fn import_dropped_now(&mut self, paths: Vec<std::path::PathBuf>, at: Option<Point>) {
        let target = at.unwrap_or_else(|| self.view.visible_page_rect(self.canvas_rect).center());
        let mut all = Vec::new();
        let mut placed = 0.0;
        for path in paths {
            if !self.import_path(&path) {
                continue;
            }
            if let Some(b) = self.selection_bounds() {
                let d = target + self.duplicate_offset * placed - b.center();
                if d.x.is_finite() && d.y.is_finite() {
                    // Part of the import's own undo step.
                    let cmd = Command::TransformShapes {
                        shapes: self.selection.clone(),
                        transform: Affine::translate(d),
                    };
                    if let Err(e) = self.engine.amend(&cmd) {
                        self.status = e.to_string();
                    }
                }
            }
            placed += 1.0;
            all.extend(self.selection.iter().copied());
        }
        if !all.is_empty() {
            self.select(all);
        }
    }
}

/// Where the pointer is, in the window's points. While a file is dragged
/// in from the file manager Windows sends the window no pointer moves, so
/// the system is asked.
#[cfg(windows)]
fn pointer_now(ctx: &Context) -> Option<egui::Pos2> {
    #[repr(C)]
    struct ScreenPoint {
        x: i32,
        y: i32,
    }
    #[link(name = "user32")]
    extern "system" {
        fn GetCursorPos(point: *mut ScreenPoint) -> i32;
    }
    let mut p = ScreenPoint { x: 0, y: 0 };
    // SAFETY: GetCursorPos writes one POINT to the pointer it is given,
    // which points to a live, properly laid out value.
    if unsafe { GetCursorPos(&mut p) } == 0 {
        return ctx.input(|i| i.pointer.latest_pos());
    }
    let Some(inner) = ctx.input(|i| i.viewport().inner_rect) else {
        return ctx.input(|i| i.pointer.latest_pos());
    };
    let ppp = ctx.pixels_per_point();
    Some(egui::pos2(p.x as f32 / ppp, p.y as f32 / ppp) - inner.min.to_vec2())
}

#[cfg(not(windows))]
fn pointer_now(ctx: &Context) -> Option<egui::Pos2> {
    ctx.input(|i| i.pointer.latest_pos())
}

/// The hint over the canvas while files hover over the window.
pub fn draw_drop_hint(ctx: &Context, painter: &Painter, rect: ERect) {
    let n = ctx.input(|i| i.raw.hovered_files.len());
    if n == 0 {
        return;
    }
    let accent = Tokens::ACCENT;
    let frame = rect.shrink(6.0);
    painter.rect_filled(frame, 6.0, accent.gamma_multiply(0.08));
    let corners = [
        frame.left_top(),
        frame.right_top(),
        frame.right_bottom(),
        frame.left_bottom(),
        frame.left_top(),
    ];
    painter.extend(egui::Shape::dashed_line(
        &corners,
        Stroke::new(2.0, accent),
        10.0,
        6.0,
    ));
    let text = if n == 1 {
        tr("drop.hint")
    } else {
        trf("drop.hint_many", &[("n", &n.to_string())])
    };
    let font = crate::theme::bold(16.0);
    let galley = painter.layout_no_wrap(text, font, Color32::WHITE);
    let label = ERect::from_center_size(frame.center(), galley.size() + egui::vec2(28.0, 16.0));
    painter.rect_filled(label, 8.0, accent);
    painter.galley(label.center() - galley.size() / 2.0, galley, Color32::WHITE);
}

#[cfg(test)]
mod tests {
    use crate::clip_tests::{app, temp_file, undo_depth};
    use crate::ui::dialogs::Dialog;
    use graphdraw_core::geometry::Point;

    fn png() -> Vec<u8> {
        let mut png = Vec::new();
        image::DynamicImage::ImageRgba8(image::RgbaImage::new(4, 4))
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .expect("png");
        png
    }

    #[test]
    fn a_dropped_svg_lands_centred_on_the_drop_point() {
        let mut app = app();
        let p = temp_file(
            "drop.svg",
            br#"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="20"><rect width="40" height="20"/></svg>"#,
        );
        let at = Point::new(50.0, 60.0);
        let steps = undo_depth(&app);
        app.import_dropped(vec![p], Some(at));
        let c = app.selection_bounds().expect("imported").center();
        assert!((c - at).hypot() < 1e-6, "{c:?}");
        assert_eq!(undo_depth(&app), steps + 1);
    }

    #[test]
    fn several_files_cascade_and_end_up_selected() {
        let mut app = app();
        let a = temp_file("drop-a.png", &png());
        let b = temp_file("drop-b.png", &png());
        app.import_dropped(vec![a, b], Some(Point::new(10.0, 10.0)));
        assert_eq!(app.selection.len(), 2);
        let shapes = app.selected_shapes();
        let ca = shapes[0].bounds().center();
        let cb = shapes[1].bounds().center();
        assert!(((cb - ca) - app.duplicate_offset).hypot() < 1e-6);
    }

    #[test]
    fn unsupported_dropped_files_give_one_alert_with_every_name() {
        let mut app = app();
        let a = temp_file("drop-a.xyz", b"1");
        let b = temp_file("drop-b.qqq", b"2");
        app.import_dropped(vec![a, b], None);
        match &app.dialog {
            Dialog::Alert(t) => assert!(t.contains("drop-a.xyz") && t.contains("drop-b.qqq")),
            other => panic!("expected one alert, got {other:?}"),
        }
        assert!(app.selection.is_empty());
    }
}
