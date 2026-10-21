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
