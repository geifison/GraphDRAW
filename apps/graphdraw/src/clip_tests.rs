//! Tests of copy, paste, import and dropped files, with the helpers they
//! share.

use crate::app::App;
use crate::i18n::tr;
use crate::ui::dialogs::Dialog;
use graphdraw_core::{document::ShapeKind, geometry::Rect};

/// A headless app with a canvas, so view-relative placement works.
pub(crate) fn app() -> App {
    let mut app = App::headless();
    app.canvas_rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
    app
}

/// [`app`] with one selected rectangle.
pub(crate) fn app_with_selected_rect() -> App {
    let mut app = app();
    let id = app
        .new_shape(ShapeKind::Rect {
            rect: Rect::new(10.0, 10.0, 40.0, 30.0),
            radius: 0.0,
            corners: None,
        })
        .expect("rectangle");
    app.select(vec![id]);
    app
}

pub(crate) fn undo_depth(app: &App) -> usize {
    app.engine.history_labels().0.len()
}

pub(crate) fn temp_file(name: &str, bytes: &[u8]) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join("graphdraw-clip-tests");
    std::fs::create_dir_all(&dir).expect("temp dir");
    let p = dir.join(name);
    std::fs::write(&p, bytes).expect("temp file");
    p
}

#[test]
fn importing_an_unsupported_file_raises_the_alert() {
    let mut app = app();
    let p = temp_file("unsupported.xyz", b"whatever");
    assert!(!app.import_path(&p));
    match &app.dialog {
        Dialog::Alert(text) => {
            assert!(text.contains(&tr("alert.unsupported_file")));
            assert!(text.contains("unsupported.xyz"));
        }
        other => panic!("expected the alert, got {other:?}"),
    }
}

#[test]
fn a_known_extension_that_does_not_decode_is_unsupported_too() {
    let mut app = app();
    let p = temp_file("broken.png", b"not a png");
    assert!(!app.import_path(&p));
    assert!(matches!(app.dialog, Dialog::Alert(_)));
}

#[test]
fn a_second_unsupported_file_joins_the_open_alert() {
    let mut app = app();
    app.import_path(&temp_file("first.xyz", b"1"));
    app.import_path(&temp_file("second.qqq", b"2"));
    match &app.dialog {
        Dialog::Alert(text) => {
            assert!(text.contains("first.xyz") && text.contains("second.qqq"));
            assert_eq!(text.matches(&tr("alert.unsupported_file")).count(), 1);
        }
        other => panic!("expected one alert, got {other:?}"),
    }
}

#[test]
fn pictures_with_an_unknown_extension_still_import() {
    let mut png = Vec::new();
    image::DynamicImage::ImageRgba8(image::RgbaImage::new(3, 3))
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .expect("png");
    let mut app = app();
    assert!(app.import_path(&temp_file("picture.dat", &png)));
    assert!(matches!(app.dialog, Dialog::None));
}

#[test]
fn gdr_files_are_imported_as_one_step() {
    let src = app_with_selected_rect();
    let p = std::env::temp_dir()
        .join("graphdraw-clip-tests")
        .join("import.gdr");
    std::fs::create_dir_all(p.parent().expect("dir")).expect("dir");
    graphdraw_io::save_native(src.doc(), &p).expect("save");
    let mut app = app();
    let steps = undo_depth(&app);
    assert!(app.import_path(&p));
    assert_eq!(app.selection.len(), 1);
    assert!(matches!(
        app.selected_shapes()[0].kind,
        ShapeKind::Rect { .. }
    ));
    assert_eq!(undo_depth(&app), steps + 1);
}

#[test]
fn copy_publishes_native_svg_text_and_png() {
    let mut app = app_with_selected_rect();
    app.copy_with_system();
    let p = crate::clip_system::fake::last_published().expect("published");
    let back = crate::clip_native::decode(p.native.as_deref().expect("native")).expect("decodes");
    assert_eq!(back.shapes, app.selected_shapes());
    assert!(p.svg.as_deref().is_some_and(|s| s.contains("<svg")));
    assert!(p.svg_as_text);
    let png = image::load_from_memory(p.png.as_deref().expect("png")).expect("png decodes");
    assert!(png.width().max(png.height()) <= app.settings.clipboard.png_max_side);
    assert!(app.published.is_some());
    assert!(app.clipboard.is_some(), "the internal copy stays");
}

#[test]
fn big_selections_get_a_picture_no_larger_than_the_limit() {
    let mut app = app_with_selected_rect();
    app.settings.clipboard.png_max_side = 64;
    app.copy_with_system();
    let p = crate::clip_system::fake::last_published().expect("published");
    let png = image::load_from_memory(p.png.as_deref().expect("png")).expect("png decodes");
    assert!(png.width().max(png.height()) <= 64);
}

#[test]
fn preferences_turn_formats_off() {
    let mut app = app_with_selected_rect();
    app.settings.clipboard.png = false;
    app.settings.clipboard.svg_as_text = false;
    app.copy_with_system();
    let p = crate::clip_system::fake::last_published().expect("published");
    assert!(p.png.is_none());
    assert!(!p.svg_as_text);
    assert!(p.native.is_some() && p.svg.is_some());
}

#[test]
fn the_size_limit_drops_the_picture_first() {
    let mut app = app_with_selected_rect();
    let p_all = {
        app.copy_with_system();
        crate::clip_system::fake::last_published().expect("published")
    };
    let png_len = p_all.png.as_ref().map_or(0, Vec::len);
    let rest =
        p_all.native.as_ref().map_or(0, Vec::len) + 3 * p_all.svg.as_ref().map_or(0, String::len);
    app.settings.clipboard.max_publish_bytes = rest + png_len / 2;
    app.copy_with_system();
    let p = crate::clip_system::fake::last_published().expect("published");
    assert!(p.png.is_none());
    assert!(p.native.is_some() && p.svg_as_text);
}

#[test]
fn copying_nothing_publishes_nothing() {
    let mut app = app();
    app.copy_with_system();
    assert!(app.published.is_none());
}
