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

fn offer() -> crate::clip_offer::Offer {
    crate::clip_offer::Offer::default()
}

fn shape_count(app: &App) -> usize {
    app.doc().all_layers().map(|l| l.shapes.len()).sum()
}

fn png_bytes(w: u32, h: u32) -> Vec<u8> {
    let mut png = Vec::new();
    image::DynamicImage::ImageRgba8(image::RgbaImage::new(w, h))
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .expect("png");
    png
}

#[test]
fn system_content_wins_over_an_older_internal_copy() {
    let mut app = app_with_selected_rect();
    app.copy_with_system();
    crate::clip_system::fake::set(crate::clip_offer::Offer {
        text: Some("hello".into()),
        ..offer()
    });
    app.paste_any();
    assert!(matches!(
        app.selected_shapes()[0].kind,
        ShapeKind::Text { .. }
    ));
}

#[test]
fn our_own_publication_pastes_from_the_internal_copy() {
    let mut app = app_with_selected_rect();
    app.copy_with_system();
    let n = shape_count(&app);
    let steps = undo_depth(&app);
    app.paste_any();
    assert_eq!(shape_count(&app), n + 1);
    assert!(matches!(
        app.selected_shapes()[0].kind,
        ShapeKind::Rect { .. }
    ));
    assert_eq!(undo_depth(&app), steps + 1);
}

#[test]
fn native_payload_from_another_window_keeps_the_object() {
    let src = app_with_selected_rect();
    let bytes = crate::clip_native::encode(&src.selected_shapes(), &[], &[]).expect("encodes");
    let mut app = app();
    crate::clip_system::fake::set(crate::clip_offer::Offer {
        native: Some(bytes),
        svg: Some("<svg xmlns=\"http://www.w3.org/2000/svg\"/>".into()),
        ..offer()
    });
    app.paste_any();
    assert_eq!(app.selected_shapes()[0].kind, src.selected_shapes()[0].kind);
}

#[test]
fn a_broken_payload_falls_back_to_the_svg() {
    let mut app = app();
    crate::clip_system::fake::set(crate::clip_offer::Offer {
        native: Some(b"garbage".to_vec()),
        svg: Some(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><rect width="4" height="4"/></svg>"#
                .into(),
        ),
        ..offer()
    });
    app.paste_any();
    assert!(!app.selection.is_empty());
    assert!(matches!(app.dialog, Dialog::None));
}

#[test]
fn html_image_from_a_browser_becomes_a_bitmap() {
    use base64::Engine as _;
    let html = format!(
        r#"<img src="data:image/png;base64,{}">"#,
        base64::engine::general_purpose::STANDARD.encode(png_bytes(3, 2))
    );
    let mut app = app();
    crate::clip_system::fake::set(crate::clip_offer::Offer {
        html: Some(html),
        ..offer()
    });
    app.paste_any();
    assert!(matches!(
        app.selected_shapes()[0].kind,
        ShapeKind::Bitmap {
            width_px: 3,
            height_px: 2,
            ..
        }
    ));
}

#[test]
fn png_flavour_becomes_a_bitmap_and_oversize_pictures_are_refused() {
    let mut app = app();
    crate::clip_system::fake::set(crate::clip_offer::Offer {
        png: Some(png_bytes(5, 4)),
        ..offer()
    });
    app.paste_any();
    assert!(matches!(
        app.selected_shapes()[0].kind,
        ShapeKind::Bitmap { width_px: 5, .. }
    ));
    let mut app = self::app();
    crate::clip_system::fake::set(crate::clip_offer::Offer {
        image: Some(image::RgbaImage::new(
            crate::clipboard::MAX_PICTURE_SIDE + 1,
            1,
        )),
        ..offer()
    });
    app.paste_any();
    assert!(app.selection.is_empty());
    assert!(matches!(app.dialog, Dialog::Alert(_)));
}

#[test]
fn copied_files_are_imported_and_unsupported_ones_named() {
    let svg = temp_file(
        "pasted.svg",
        br#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><rect width="4" height="4"/></svg>"#,
    );
    let bad = temp_file("pasted.xyz", b"?");
    let mut app = app();
    crate::clip_system::fake::set(crate::clip_offer::Offer {
        files: vec![svg, bad],
        ..offer()
    });
    app.paste_any();
    assert!(!app.selection.is_empty());
    match &app.dialog {
        Dialog::Alert(t) => assert!(t.contains("pasted.xyz")),
        other => panic!("expected the alert, got {other:?}"),
    }
}

#[test]
fn unknown_clipboard_formats_raise_the_alert_and_empty_does_nothing() {
    let mut app = app();
    crate::clip_system::fake::set(crate::clip_offer::Offer {
        other_formats: true,
        ..offer()
    });
    app.paste_any();
    match &app.dialog {
        Dialog::Alert(t) => assert_eq!(t, &tr("alert.clipboard_unsupported")),
        other => panic!("expected the alert, got {other:?}"),
    }
    let mut app = self::app();
    crate::clip_system::fake::set(offer());
    let n = shape_count(&app);
    app.paste_any();
    assert_eq!(shape_count(&app), n);
    assert!(matches!(app.dialog, Dialog::None));
}

#[test]
fn view_centre_preference_pastes_centred_in_one_step() {
    let mut app = app_with_selected_rect();
    app.settings.clipboard.paste_place = crate::prefs::PastePlace::ViewCenter;
    app.copy_with_system();
    let steps = undo_depth(&app);
    app.paste_any();
    let c = app.selection_bounds().expect("pasted").center();
    let v = app.view.visible_page_rect(app.canvas_rect).center();
    assert!((c - v).hypot() < 1e-6, "{c:?} {v:?}");
    assert_eq!(undo_depth(&app), steps + 1);
}

#[test]
fn paste_in_view_is_one_step_too() {
    let mut app = app_with_selected_rect();
    app.copy_with_system();
    let steps = undo_depth(&app);
    app.paste_in_view();
    let c = app.selection_bounds().expect("pasted").center();
    let v = app.view.visible_page_rect(app.canvas_rect).center();
    assert!((c - v).hypot() < 1e-6);
    assert_eq!(undo_depth(&app), steps + 1);
}

#[test]
fn pasted_text_is_one_step() {
    let mut app = app();
    crate::clip_system::fake::set(crate::clip_offer::Offer {
        text: Some("words".into()),
        ..offer()
    });
    let steps = undo_depth(&app);
    app.paste_any();
    assert_eq!(undo_depth(&app), steps + 1);
}

#[test]
fn pasted_svg_text_is_sanitised() {
    let mut app = app();
    let secret = temp_file("secret.png", &png_bytes(2, 2));
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><script>alert(1)</script><image width="5" height="5" href="{}"/><rect width="4" height="4"/></svg>"#,
        secret.display().to_string().replace('\\', "/")
    );
    crate::clip_system::fake::set(crate::clip_offer::Offer {
        text: Some(svg),
        ..offer()
    });
    app.paste_any();
    let shapes = app.selected_shapes();
    assert!(!shapes.is_empty());
    fn has_bitmap(shapes: &[graphdraw_core::Shape]) -> bool {
        shapes.iter().any(|s| match &s.kind {
            ShapeKind::Bitmap { .. } => true,
            ShapeKind::Group { children } => has_bitmap(children),
            _ => false,
        })
    }
    assert!(!has_bitmap(&shapes));
}

#[test]
fn paste_special_forces_a_flavour() {
    let mut app = app();
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><rect width="4" height="4"/></svg>"#;
    let o = crate::clip_offer::Offer {
        text: Some(svg.into()),
        ..offer()
    };
    let text = crate::ui::dialogs::PASTE_SPECIAL[4].1;
    assert!(app.paste_with(&o, text));
    assert!(matches!(
        app.selected_shapes()[0].kind,
        ShapeKind::Text { .. }
    ));
}
