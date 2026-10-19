//! The Pick tool's drags in the real window, with pointer events and
//! modifier keys: Ctrl keeps a move straight, a right click during a drag
//! leaves the original, Space drops a copy, Shift scales from the centre,
//! Ctrl scales in steps of 100 % (mirror at the same size), Alt frees a
//! corner, the side handles of rotate mode skew, and the centre of
//! rotation moves. Also the pointer's shape over handles and objects.

use crate::app::{App, Drag, Handle, HandleMods};
use crate::interaction::skew_transform;
use crate::tools::Tool;
use crate::ui::cursors::{for_tool, resize_icon, ShapeMark, ToolCursor};
use egui::{Event, Modifiers, PointerButton, Pos2};
use tracedraw_core::geometry::{Affine, Point, Rect, Vec2};
use tracedraw_core::{ShapeId, ShapeKind};

fn frame(ctx: &egui::Context, app: &mut App, events: Vec<Event>, modifiers: Modifiers) {
    // The keys held come first, as the windowing layer sends them.
    let mut all = vec![Event::ModifiersChanged(modifiers)];
    all.extend(events);
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            Pos2::ZERO,
            egui::vec2(1600.0, 1000.0),
        )),
        events: all,
        ..Default::default()
    };
    let mut out = ctx.run_ui(input, |ui| crate::ui::root(app, ui));
    out.textures_delta.clear();
}

fn button(pos: Pos2, pressed: bool, button: PointerButton, modifiers: Modifiers) -> Event {
    Event::PointerButton {
        pos,
        button,
        pressed,
        modifiers,
    }
}

fn key(key: egui::Key, modifiers: Modifiers) -> Event {
    Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    }
}

/// A primary drag from `from` to `to` with `mods` held, `during` sent in
/// the middle of it.
fn drag(
    ctx: &egui::Context,
    app: &mut App,
    from: Pos2,
    to: Pos2,
    mods: Modifiers,
    during: Vec<Event>,
) {
    frame(ctx, app, vec![Event::PointerMoved(from)], mods);
    frame(
        ctx,
        app,
        vec![button(from, true, PointerButton::Primary, mods)],
        mods,
    );
    for i in 1..=4 {
        let p = from + (to - from) * (i as f32 / 4.0);
        let mut events = vec![Event::PointerMoved(p)];
        if i == 2 {
            events.extend(during.clone());
        }
        frame(ctx, app, events, mods);
    }
    frame(
        ctx,
        app,
        vec![button(to, false, PointerButton::Primary, mods)],
        mods,
    );
    frame(ctx, app, Vec::new(), Modifiers::NONE);
}

/// A drawing with a 50 x 30 mm rectangle at (10, 10), selected, the
/// page in view.
fn setup() -> (egui::Context, App, ShapeId) {
    let ctx = crate::theme::ui_context();
    let mut app = App::headless();
    app.show_welcome = false;
    let id = app
        .new_shape(ShapeKind::Rect {
            rect: Rect::new(10.0, 10.0, 60.0, 40.0),
            radius: 0.0,
            corners: None,
        })
        .expect("rect");
    app.set_tool(Tool::Pick);
    app.select(vec![id]);
    // Filled, so that a press inside it picks it.
    app.apply_fill(tracedraw_core::Fill::Solid(tracedraw_core::Color::rgb8(
        200, 30, 30,
    )));
    // The window's bars settle over the first frames; then the page fits.
    for _ in 0..3 {
        frame(&ctx, &mut app, Vec::new(), Modifiers::NONE);
    }
    app.zoom_to_page();
    for _ in 0..2 {
        frame(&ctx, &mut app, Vec::new(), Modifiers::NONE);
    }
    (ctx, app, id)
}

fn bounds(app: &App, id: ShapeId) -> Rect {
    app.doc().find_shape(id).map(|s| s.bounds()).expect("shape")
}

fn handle(app: &App, which: Handle) -> Pos2 {
    let b = app.selection_bounds().expect("selection");
    app.handle_positions(b)
        .iter()
        .find(|(h, _)| *h == which)
        .map(|(_, p)| *p)
        .expect("handle")
}

fn shape_count(app: &App) -> usize {
    app.doc()
        .all_layers()
        .map(|l| l.shapes.len())
        .sum::<usize>()
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 0.05
}

#[test]
fn ctrl_keeps_a_move_straight() {
    let (ctx, mut app, id) = setup();
    let from = app.view.to_screen(Point::new(35.0, 25.0));
    drag(
        &ctx,
        &mut app,
        from,
        from + egui::vec2(120.0, 35.0),
        Modifiers::CTRL,
        Vec::new(),
    );
    let b = bounds(&app, id);
    assert!(b.x0 > 15.0, "moved sideways: {b:?}");
    assert!(close(b.y0, 10.0), "kept its height: {b:?}");
    // Without Ctrl it follows the pointer both ways.
    let from = app.view.to_screen(b.center());
    drag(
        &ctx,
        &mut app,
        from,
        from + egui::vec2(0.0, -60.0),
        Modifiers::NONE,
        Vec::new(),
    );
    assert!(bounds(&app, id).y0 > b.y0 + 5.0);
}

#[test]
fn a_right_click_while_dragging_leaves_the_original() {
    let (ctx, mut app, id) = setup();
    let from = app.view.to_screen(Point::new(35.0, 25.0));
    let to = from + egui::vec2(150.0, 0.0);
    let right = |p: Pos2, down: bool| button(p, down, PointerButton::Secondary, Modifiers::NONE);
    let mid = from + (to - from) * 0.5;
    drag(
        &ctx,
        &mut app,
        from,
        to,
        Modifiers::NONE,
        vec![right(mid, true), right(mid, false)],
    );
    assert_eq!(shape_count(&app), 2);
    assert!(close(bounds(&app, id).x0, 10.0), "the original stays");
    // The copy is the selection, moved.
    let copy = app.selection.first().copied().expect("copy");
    assert_ne!(copy, id);
    assert!(bounds(&app, copy).x0 > 15.0);
    assert!(app.dialog_is_none_or_menu_closed());
}

#[test]
fn esc_cancels_a_drag() {
    let (ctx, mut app, id) = setup();
    let from = app.view.to_screen(Point::new(35.0, 25.0));
    drag(
        &ctx,
        &mut app,
        from,
        from + egui::vec2(150.0, 40.0),
        Modifiers::NONE,
        vec![key(egui::Key::Escape, Modifiers::NONE)],
    );
    assert!(close(bounds(&app, id).x0, 10.0), "nothing moved");
    assert_eq!(app.selection, vec![id], "the selection stays");
    assert!(matches!(app.drag, Drag::None));
}

#[test]
fn space_while_moving_drops_a_copy() {
    let (ctx, mut app, id) = setup();
    let from = app.view.to_screen(Point::new(35.0, 25.0));
    drag(
        &ctx,
        &mut app,
        from,
        from + egui::vec2(160.0, 0.0),
        Modifiers::NONE,
        vec![key(egui::Key::Space, Modifiers::NONE)],
    );
    // The original went all the way; a copy stayed half way.
    assert_eq!(shape_count(&app), 2);
    let moved = bounds(&app, id);
    let copy = app
        .doc()
        .all_layers()
        .flat_map(|l| &l.shapes)
        .find(|s| s.id != id)
        .map(|s| s.bounds())
        .expect("copy");
    assert!(
        copy.x0 > 10.5 && copy.x0 < moved.x0 - 0.5,
        "{copy:?} {moved:?}"
    );
    assert_eq!(app.tool, Tool::Pick);
}

#[test]
fn shift_scales_from_the_centre_and_ctrl_mirrors() {
    let (ctx, mut app, id) = setup();
    // Shift: the right side goes out, the left side as far the other way.
    let e = handle(&app, Handle::E);
    drag(
        &ctx,
        &mut app,
        e,
        e + egui::vec2(60.0, 0.0),
        Modifiers::SHIFT,
        Vec::new(),
    );
    let b = bounds(&app, id);
    assert!(close(b.center().x, 35.0), "{b:?}");
    assert!(b.width() > 55.0);
    app.undo();
    // Ctrl, dragged past the other side: the mirror image at 100 %.
    let e = handle(&app, Handle::E);
    let w = handle(&app, Handle::W);
    let past = w - egui::vec2((e.x - w.x) * 0.8, 0.0);
    drag(&ctx, &mut app, e, past, Modifiers::CTRL, Vec::new());
    let b = bounds(&app, id);
    assert!(close(b.width(), 50.0), "{b:?}");
    assert!(close(b.x1, 10.0) && close(b.x0, -40.0), "{b:?}");
    let t = app
        .doc()
        .find_shape(id)
        .map(|s| s.transform)
        .expect("shape");
    assert!(t.determinant() < 0.0, "mirrored");
}

#[test]
fn rotate_mode_skews_with_the_sides_and_turns_about_the_moved_centre() {
    let (ctx, mut app, id) = setup();
    app.rotate_mode = true;
    frame(&ctx, &mut app, Vec::new(), Modifiers::NONE);
    // The top handle slants the rectangle sideways; the bottom stays.
    let n = handle(&app, Handle::N);
    drag(
        &ctx,
        &mut app,
        n,
        n + egui::vec2(80.0, 0.0),
        Modifiers::NONE,
        Vec::new(),
    );
    let b = bounds(&app, id);
    assert!(b.width() > 55.0, "slanted: {b:?}");
    assert!(close(b.y0, 10.0) && close(b.y1, 40.0), "{b:?}");
    app.undo();
    // Move the centre of rotation onto the lower left corner, then turn.
    app.rotate_mode = true;
    frame(&ctx, &mut app, Vec::new(), Modifiers::NONE);
    let pivot = app.view.to_screen(Point::new(35.0, 25.0));
    let corner = app.view.to_screen(Point::new(10.0, 10.0));
    drag(&ctx, &mut app, pivot, corner, Modifiers::NONE, Vec::new());
    assert!(app.rotate_mode, "moving the centre keeps rotate mode");
    let c = app.rotation_center().expect("centre");
    assert!(close(c.x, 10.0) && close(c.y, 10.0), "{c:?}");
    // A quarter turn about (10, 10) with Ctrl (15 degree steps).
    let ne = handle(&app, Handle::NE);
    let centre = app.view.to_screen(c);
    let r = ne - centre;
    let quarter = centre + egui::vec2(-r.y, r.x);
    drag(&ctx, &mut app, ne, quarter, Modifiers::CTRL, Vec::new());
    let b = bounds(&app, id);
    // The corner on the centre stays where it was.
    assert!(
        close(b.x0, 10.0) || close(b.x1, 10.0),
        "turned about the corner: {b:?}"
    );
    assert!(close(b.width(), 30.0) && close(b.height(), 50.0), "{b:?}");
}

#[test]
fn handle_modifiers_shape_the_scale() {
    let app = App::headless();
    let b = Rect::new(0.0, 0.0, 40.0, 20.0);
    let scale = |h: Handle, anchor: Point, current: Point, mods: HandleMods| {
        app.preview_transform_of(&Drag::Scale {
            handle: h,
            anchor,
            start_bounds: b,
            current,
            grab: tracedraw_core::geometry::Vec2::ZERO,
            mods,
        })
        .expect("transform")
    };
    let none = HandleMods::default();
    let shift = HandleMods {
        from_center: true,
        ..none
    };
    let ctrl = HandleMods {
        steps: true,
        ..none
    };
    let alt = HandleMods { free: true, ..none };
    let at = |t: Affine, x: f64, y: f64| t * Point::new(x, y);
    // Plain: the left side stays.
    let t = scale(
        Handle::E,
        Point::new(0.0, 10.0),
        Point::new(80.0, 10.0),
        none,
    );
    assert!(close(at(t, 40.0, 10.0).x, 80.0) && close(at(t, 0.0, 10.0).x, 0.0));
    // Shift: about the centre (20).
    let t = scale(
        Handle::E,
        Point::new(0.0, 10.0),
        Point::new(60.0, 10.0),
        shift,
    );
    assert!(close(at(t, 40.0, 10.0).x, 60.0) && close(at(t, 0.0, 10.0).x, -20.0));
    // Ctrl: whole steps; past the anchor the mirror image at 100 %.
    let t = scale(
        Handle::E,
        Point::new(0.0, 10.0),
        Point::new(-35.0, 10.0),
        ctrl,
    );
    assert!(close(at(t, 40.0, 10.0).x, -40.0));
    let t = scale(
        Handle::E,
        Point::new(0.0, 10.0),
        Point::new(70.0, 10.0),
        ctrl,
    );
    assert!(close(at(t, 40.0, 10.0).x, 80.0));
    // Ctrl and Shift: steps about the centre.
    let both = HandleMods {
        from_center: true,
        steps: true,
        ..none
    };
    let t = scale(
        Handle::E,
        Point::new(0.0, 10.0),
        Point::new(-15.0, 10.0),
        both,
    );
    assert!(close(at(t, 40.0, 10.0).x, -20.0));
    // A corner keeps the proportions unless Alt frees it.
    let t = scale(
        Handle::NE,
        Point::new(0.0, 0.0),
        Point::new(80.0, 20.0),
        none,
    );
    assert!(close(at(t, 40.0, 20.0).y, 40.0));
    let t = scale(
        Handle::NE,
        Point::new(0.0, 0.0),
        Point::new(80.0, 20.0),
        alt,
    );
    assert!(close(at(t, 40.0, 20.0).x, 80.0) && close(at(t, 40.0, 20.0).y, 20.0));
}

#[test]
fn side_handles_skew() {
    let b = Rect::new(0.0, 0.0, 40.0, 20.0);
    let none = HandleMods::default();
    let at = |t: Affine, x: f64, y: f64| t * Point::new(x, y);
    // Top handle: the top moves by the drag, the bottom stays.
    let t = skew_transform(Handle::N, b, Vec2::new(20.0, 0.0), none, 15.0).expect("skew");
    assert!(close(at(t, 0.0, 20.0).x, 20.0) && close(at(t, 0.0, 0.0).x, 0.0));
    // Bottom handle: the other way round.
    let t = skew_transform(Handle::S, b, Vec2::new(20.0, 0.0), none, 15.0).expect("skew");
    assert!(close(at(t, 0.0, 0.0).x, 20.0) && close(at(t, 0.0, 20.0).x, 0.0));
    // Right handle: up and down.
    let t = skew_transform(Handle::E, b, Vec2::new(0.0, 10.0), none, 15.0).expect("skew");
    assert!(close(at(t, 40.0, 0.0).y, 10.0) && close(at(t, 0.0, 0.0).y, 0.0));
    // Shift: about the centre line.
    let shift = HandleMods {
        from_center: true,
        ..none
    };
    let t = skew_transform(Handle::N, b, Vec2::new(20.0, 0.0), shift, 15.0).expect("skew");
    assert!(close(at(t, 0.0, 20.0).x, 20.0) && close(at(t, 0.0, 0.0).x, -20.0));
    // Ctrl: the slant in 15 degree steps (37 degrees becomes 30).
    let ctrl = HandleMods {
        steps: true,
        ..none
    };
    let t = skew_transform(Handle::N, b, Vec2::new(15.0, 0.0), ctrl, 15.0).expect("skew");
    let x = at(t, 0.0, 20.0).x;
    assert!((x - 20.0 * 30f64.to_radians().tan()).abs() < 1e-6, "{x}");
    // Corners and flat boxes make none.
    assert!(skew_transform(Handle::NE, b, Vec2::new(5.0, 0.0), none, 15.0).is_none());
    let flat = Rect::new(0.0, 0.0, 40.0, 0.0);
    assert!(skew_transform(Handle::N, flat, Vec2::new(5.0, 0.0), none, 15.0).is_none());
}

#[test]
fn the_pointer_shows_what_a_press_would_do() {
    let (ctx, mut app, id) = setup();
    let cursor = |app: &App, at: Pos2| for_tool(app, false, false, false, Some(at));
    // Scale handles: resize arrows along their direction.
    for h in [
        Handle::N,
        Handle::S,
        Handle::E,
        Handle::W,
        Handle::NE,
        Handle::NW,
        Handle::SE,
        Handle::SW,
    ] {
        assert_eq!(
            cursor(&app, handle(&app, h)),
            ToolCursor::System(resize_icon(h)),
            "{h:?}"
        );
    }
    // The selected object: the move arrows; elsewhere the plain arrow.
    let inside = app.view.to_screen(Point::new(35.0, 25.0));
    assert_eq!(
        cursor(&app, inside),
        ToolCursor::System(egui::CursorIcon::Move)
    );
    let empty = app.view.to_screen(Point::new(150.0, 250.0));
    assert_eq!(
        cursor(&app, empty),
        ToolCursor::System(egui::CursorIcon::Default)
    );
    // Rotate mode: corners turn, sides slant, the centre moves.
    app.rotate_mode = true;
    assert_eq!(cursor(&app, handle(&app, Handle::NE)), ToolCursor::Rotate);
    assert_eq!(
        cursor(&app, handle(&app, Handle::N)),
        ToolCursor::Skew { horizontal: true }
    );
    assert_eq!(
        cursor(&app, handle(&app, Handle::E)),
        ToolCursor::Skew { horizontal: false }
    );
    assert_eq!(
        cursor(&app, inside),
        ToolCursor::System(egui::CursorIcon::Move)
    );
    app.rotate_mode = false;
    // The Shape tool: its arrow, marked over a node and over a segment of
    // a curve.
    app.select(vec![id]);
    app.convert_to_curves();
    app.set_tool(Tool::Shape);
    frame(&ctx, &mut app, Vec::new(), Modifiers::NONE);
    let node = app.view.to_screen(Point::new(10.0, 10.0));
    let segment = app.view.to_screen(Point::new(35.0, 10.0));
    assert_eq!(cursor(&app, node), ToolCursor::ShapeArrow(ShapeMark::Node));
    assert_eq!(
        cursor(&app, segment),
        ToolCursor::ShapeArrow(ShapeMark::Segment)
    );
    assert_eq!(
        cursor(&app, empty),
        ToolCursor::ShapeArrow(ShapeMark::Nothing)
    );
    // The handles framing selected nodes: resize arrows, or turning and
    // slant arrows.
    app.select_all_nodes();
    app.node_transform = crate::node_edit::NodeTransformMode::StretchScale;
    let at = |app: &App, h: Handle| {
        let p = app
            .node_transform_handles()
            .into_iter()
            .find(|(k, _)| *k == h)
            .map(|(_, p)| p)
            .expect("handle");
        app.view.to_screen(p)
    };
    assert_eq!(
        cursor(&app, at(&app, Handle::E)),
        ToolCursor::System(resize_icon(Handle::E))
    );
    app.node_transform = crate::node_edit::NodeTransformMode::RotateSkew;
    assert_eq!(cursor(&app, at(&app, Handle::NE)), ToolCursor::Rotate);
    assert_eq!(
        cursor(&app, at(&app, Handle::S)),
        ToolCursor::Skew { horizontal: true }
    );
    app.node_transform = crate::node_edit::NodeTransformMode::None;
    // While dragging, the pointer keeps the drag's shape.
    app.set_tool(Tool::Pick);
    app.drag = Drag::Rotate {
        center: Point::new(0.0, 0.0),
        start_angle: 0.0,
        current_angle: 0.0,
    };
    assert_eq!(cursor(&app, empty), ToolCursor::Rotate);
    // Moving: the arrow with a dotted box; once a right click makes the
    // move leave the original, the box gets a plus.
    app.drag = Drag::Move {
        start: Point::ZERO,
        last: Point::ZERO,
        total: Vec2::ZERO,
        start_bounds: Rect::ZERO,
        source: None,
    };
    assert_eq!(cursor(&app, empty), ToolCursor::DragMove { copy: false });
    app.drag_copy = true;
    assert_eq!(cursor(&app, empty), ToolCursor::DragMove { copy: true });
    app.drag_copy = false;
    app.drag = Drag::None;
}

impl App {
    /// No context menu or drop menu was left open by a right click
    /// during a drag.
    fn dialog_is_none_or_menu_closed(&self) -> bool {
        self.context_menu.is_none() && self.drop_menu.is_none()
    }
}

#[test]
fn the_status_bar_tells_how_far_how_much_and_what_angle() {
    let (_ctx, mut app, _id) = setup();
    let info = |app: &App| crate::ui::status::drag_info(app).unwrap_or_default();
    assert!(info(&app).is_empty(), "nothing while not dragging");
    app.drag = Drag::Move {
        start: Point::new(0.0, 0.0),
        last: Point::new(0.0, 0.0),
        total: Vec2::new(30.0, 40.0),
        start_bounds: Rect::new(10.0, 10.0, 60.0, 40.0),
        source: None,
    };
    let text = info(&app);
    assert!(text.contains("50.000") && text.contains("53.1"), "{text}");
    app.drag = Drag::Scale {
        handle: Handle::E,
        anchor: Point::new(10.0, 25.0),
        start_bounds: Rect::new(10.0, 10.0, 60.0, 40.0),
        current: Point::new(85.0, 25.0),
        grab: tracedraw_core::geometry::Vec2::ZERO,
        mods: HandleMods::default(),
    };
    let text = info(&app);
    assert!(text.contains("150.0") && text.contains("100.0"), "{text}");
    app.drag = Drag::Rotate {
        center: Point::new(0.0, 0.0),
        start_angle: 0.0,
        current_angle: 30f64.to_radians(),
    };
    assert!(info(&app).contains("30.0"), "{}", info(&app));
    app.drag = Drag::Skew {
        handle: Handle::N,
        start_bounds: Rect::new(10.0, 10.0, 60.0, 40.0),
        start: Point::new(35.0, 40.0),
        current: Point::new(65.0, 40.0),
        mods: HandleMods::default(),
    };
    assert!(info(&app).contains("45.0"), "{}", info(&app));
    // A copy says so.
    app.drag_copy = true;
    assert!(info(&app).contains(&crate::i18n::tr("status.drag_copy_short")));
    app.drag = Drag::None;
    app.drag_copy = false;
}

#[test]
fn objects_dropped_on_the_document_palette_give_their_colours() {
    let (ctx, mut app, id) = setup();
    app.settings.palette.show_document = true;
    for _ in 0..2 {
        frame(&ctx, &mut app, Vec::new(), Modifiers::NONE);
    }
    let row = app.doc_palette_rect.expect("the document palette shows");
    let from = app.view.to_screen(Point::new(35.0, 25.0));
    drag(
        &ctx,
        &mut app,
        from,
        row.center(),
        Modifiers::NONE,
        Vec::new(),
    );
    // The object stays; its fill joined the document palette.
    assert!(close(bounds(&app, id).x0, 10.0), "{:?}", bounds(&app, id));
    assert!(app
        .doc()
        .palette
        .contains(&tracedraw_core::Color::rgb8(200, 30, 30)));
}

fn click_at(ctx: &egui::Context, app: &mut App, at: Pos2, mods: Modifiers) {
    frame(ctx, app, vec![Event::PointerMoved(at)], mods);
    frame(
        ctx,
        app,
        vec![button(at, true, PointerButton::Primary, mods)],
        mods,
    );
    frame(
        ctx,
        app,
        vec![button(at, false, PointerButton::Primary, mods)],
        mods,
    );
    // Past the double-click time (frames advance the input clock by a
    // sixtieth of a second each), so the next click is a click of its own.
    for _ in 0..20 {
        frame(ctx, app, Vec::new(), mods);
    }
}

fn last_path(app: &App) -> tracedraw_core::BezPath {
    app.doc()
        .all_layers()
        .flat_map(|l| &l.shapes)
        .filter_map(|s| match &s.kind {
            ShapeKind::Path { .. } => Some(s.page_path()),
            _ => None,
        })
        .last()
        .expect("a curve")
}

#[test]
fn ctrl_keeps_lines_to_the_constrain_angle() {
    use crate::interaction::constrain_from;
    let o = Point::new(0.0, 0.0);
    // 5 degrees off the horizontal comes back to it, at the same reach
    // along it.
    let p = constrain_from(o, Point::new(10.0, 5f64.to_radians().tan() * 10.0), 15.0);
    assert!(p.y.abs() < 1e-9 && (p.x - 10.0).abs() < 1e-9, "{p:?}");
    // 40 degrees goes to 45.
    let a = 40f64.to_radians();
    let p = constrain_from(o, Point::new(a.cos() * 10.0, a.sin() * 10.0), 15.0);
    assert!((p.y.atan2(p.x).to_degrees() - 45.0).abs() < 1e-9);
    assert_eq!(constrain_from(o, o, 15.0), o);

    // Freehand: two clicks make a straight line; with Ctrl a level one.
    let (ctx, mut app, _id) = setup();
    app.set_tool(Tool::Freehand);
    let a = app.view.to_screen(Point::new(100.0, 100.0));
    let b = app.view.to_screen(Point::new(160.0, 104.0));
    click_at(&ctx, &mut app, a, Modifiers::NONE);
    click_at(&ctx, &mut app, b, Modifiers::CTRL);
    let path = last_path(&app);
    let pts: Vec<Point> = path
        .elements()
        .iter()
        .filter_map(|e| match e {
            tracedraw_core::geometry::PathEl::MoveTo(p)
            | tracedraw_core::geometry::PathEl::LineTo(p) => Some(*p),
            _ => None,
        })
        .collect();
    assert_eq!(pts.len(), 2, "{pts:?}");
    assert!((pts[0].y - pts[1].y).abs() < 1e-6, "level: {pts:?}");
    assert!(pts[1].x > pts[0].x + 50.0);

    // Polyline: Ctrl keeps the next segment to 15 degree steps.
    app.set_tool(Tool::Polyline);
    let a = app.view.to_screen(Point::new(100.0, 150.0));
    let b = app.view.to_screen(Point::new(150.0, 196.0));
    click_at(&ctx, &mut app, a, Modifiers::NONE);
    click_at(&ctx, &mut app, b, Modifiers::CTRL);
    app.finish_curve();
    let path = last_path(&app);
    let pts: Vec<Point> = path
        .elements()
        .iter()
        .filter_map(|e| match e {
            tracedraw_core::geometry::PathEl::MoveTo(p)
            | tracedraw_core::geometry::PathEl::LineTo(p) => Some(*p),
            _ => None,
        })
        .collect();
    let d = pts[1] - pts[0];
    assert!(
        (d.y.atan2(d.x).to_degrees() - 45.0).abs() < 1e-6,
        "45 degrees: {pts:?}"
    );
}

/// Two filled 20 x 20 mm squares grouped (at x 100 and 140, y 100) and a
/// third square on its own at (100, 150), nothing selected.
fn grouped() -> (egui::Context, App, [ShapeId; 3], ShapeId) {
    let (ctx, mut app, first) = setup();
    app.select(vec![first]);
    app.delete_selection();
    let mut ids = Vec::new();
    for (x, y) in [(100.0, 100.0), (140.0, 100.0), (100.0, 150.0)] {
        let id = app
            .new_shape(ShapeKind::Rect {
                rect: Rect::new(x, y, x + 20.0, y + 20.0),
                radius: 0.0,
                corners: None,
            })
            .expect("rect");
        app.select(vec![id]);
        app.apply_fill(tracedraw_core::Fill::Solid(tracedraw_core::Color::rgb8(
            30, 30, 200,
        )));
        ids.push(id);
    }
    app.select(vec![ids[0], ids[1]]);
    app.group_selection();
    let group = app.selection[0];
    // The group is moved, so its objects' space is not the page's.
    app.run(tracedraw_core::Command::TransformShapes {
        shapes: vec![group],
        transform: Affine::translate((5.0, 0.0)),
    });
    app.select(Vec::new());
    frame(&ctx, &mut app, Vec::new(), Modifiers::NONE);
    (ctx, app, [ids[0], ids[1], ids[2]], group)
}

fn page_bounds(app: &App, id: ShapeId) -> Rect {
    app.doc()
        .shape_on_page(id)
        .map(|s| s.bounds())
        .expect("shape")
}

#[test]
fn ctrl_click_chooses_an_object_inside_a_group() {
    let (ctx, mut app, [a, b, _c], group) = grouped();
    let on_a = app.view.to_screen(Point::new(115.0, 110.0));
    // A plain click takes the group.
    click_at(&ctx, &mut app, on_a, Modifiers::NONE);
    assert_eq!(app.selection, vec![group]);
    // Ctrl+click: the square inside it, with round handles and the status
    // bar saying where it is.
    click_at(&ctx, &mut app, on_a, Modifiers::CTRL);
    assert_eq!(app.selection, vec![a]);
    assert!(app.selection_nested());
    assert!(crate::ui::status::object_info(&app).contains("in a group"));
    let before_b = page_bounds(&app, b);
    // Dragging it moves that square only, by the drag on the page.
    let before = page_bounds(&app, a);
    let to = app.view.to_screen(Point::new(115.0, 130.0));
    drag(&ctx, &mut app, on_a, to, Modifiers::NONE, Vec::new());
    let after = page_bounds(&app, a);
    assert!(close(after.y0 - before.y0, 20.0), "{before:?} {after:?}");
    assert!(close(after.x0, before.x0));
    assert_eq!(page_bounds(&app, b), before_b);
    assert_eq!(app.doc().top_level_of(a), Some(group));
    // Undo and redo keep it chosen.
    app.undo();
    assert_eq!(app.selection, vec![a]);
    assert!(close(page_bounds(&app, a).y0, before.y0));
    app.redo();
    assert_eq!(app.selection, vec![a]);
    // A colour reaches it alone.
    app.apply_fill(tracedraw_core::Fill::Solid(tracedraw_core::Color::rgb8(
        0, 200, 0,
    )));
    let fill_of = |app: &App, id| app.doc().find_shape(id).map(|s| s.fill.clone());
    assert_ne!(fill_of(&app, a), fill_of(&app, b));
    // Ctrl+Shift+click adds the other square; the group is never selected
    // with its own objects.
    let on_b = app.view.to_screen(Point::new(155.0, 110.0));
    click_at(&ctx, &mut app, on_b, Modifiers::CTRL | Modifiers::SHIFT);
    assert_eq!(app.selection, vec![a, b]);
    app.select_add(group);
    assert_eq!(app.selection, vec![group]);
    // Delete takes the chosen square out of the group.
    app.select(vec![a]);
    app.delete_selection();
    assert!(app.doc().find_shape(a).is_none());
    assert_eq!(app.doc().top_level_of(b), Some(group));
}

#[test]
fn alt_click_digs_to_the_object_beneath() {
    let (ctx, mut app, [_a, _b, c], _group) = grouped();
    // A fourth square over the third one.
    let top = app
        .new_shape(ShapeKind::Rect {
            rect: Rect::new(105.0, 155.0, 125.0, 175.0),
            radius: 0.0,
            corners: None,
        })
        .expect("rect");
    app.select(vec![top]);
    app.apply_fill(tracedraw_core::Fill::Solid(tracedraw_core::Color::rgb8(
        200, 200, 0,
    )));
    let both = app.view.to_screen(Point::new(110.0, 160.0));
    click_at(&ctx, &mut app, both, Modifiers::NONE);
    assert_eq!(app.selection, vec![top]);
    click_at(&ctx, &mut app, both, Modifiers::ALT);
    assert_eq!(app.selection, vec![c]);
    // Once more: round to the top again.
    click_at(&ctx, &mut app, both, Modifiers::ALT);
    assert_eq!(app.selection, vec![top]);
    assert!(!app.rotate_mode);
}

#[test]
fn alt_marquees_take_what_they_touch() {
    let (ctx, mut app, [_a, _b, c], group) = grouped();
    // Across the lone square and the group's first square, holding
    // neither wholly.
    let from = app.view.to_screen(Point::new(90.0, 112.0));
    let to = app.view.to_screen(Point::new(112.0, 158.0));
    drag(&ctx, &mut app, from, to, Modifiers::NONE, Vec::new());
    assert!(app.selection.is_empty());
    drag(&ctx, &mut app, from, to, Modifiers::ALT, Vec::new());
    let mut sel = app.selection.clone();
    sel.sort_by_key(|s| s.raw());
    let mut want = vec![group, c];
    want.sort_by_key(|s| s.raw());
    assert_eq!(sel, want);
    // Round all three: everything.
    app.select(Vec::new());
    let from = app.view.to_screen(Point::new(90.0, 90.0));
    let to = app.view.to_screen(Point::new(180.0, 180.0));
    drag(&ctx, &mut app, from, to, Modifiers::NONE, Vec::new());
    assert_eq!(app.selection.len(), 2);
}

#[test]
fn the_touching_option_swaps_what_alt_does() {
    let (ctx, mut app, [_a, _b, c], group) = grouped();
    app.settings.marquee_touching = true;
    let from = app.view.to_screen(Point::new(90.0, 112.0));
    let to = app.view.to_screen(Point::new(112.0, 158.0));
    drag(&ctx, &mut app, from, to, Modifiers::NONE, Vec::new());
    let mut sel = app.selection.clone();
    sel.sort_by_key(|s| s.raw());
    let mut want = vec![group, c];
    want.sort_by_key(|s| s.raw());
    assert_eq!(sel, want);
    app.select(Vec::new());
    drag(&ctx, &mut app, from, to, Modifiers::ALT, Vec::new());
    assert!(app.selection.is_empty());
}

/// The screen points of a lasso through `pts` (page), in small steps as a
/// hand draws it.
fn lasso_path(app: &App, pts: &[Point]) -> Vec<Pos2> {
    let mut out = Vec::new();
    for w in pts.windows(2) {
        for i in 0..8 {
            let t = i as f64 / 8.0;
            out.push(app.view.to_screen(w[0] + (w[1] - w[0]) * t));
        }
    }
    if let Some(last) = pts.last() {
        out.push(app.view.to_screen(*last));
    }
    out
}

#[test]
fn the_freehand_pick_lassoes_and_has_the_pick_tools_handles() {
    let (ctx, mut app, [_a, _b, c], group) = grouped();
    app.set_tool(Tool::FreeformPick);
    // A loop round the lone square only.
    let pts = [
        Point::new(90.0, 140.0),
        Point::new(130.0, 140.0),
        Point::new(130.0, 180.0),
        Point::new(90.0, 180.0),
        Point::new(90.0, 141.0),
    ];
    let screen = lasso_path(&app, &pts);
    let lasso = |ctx: &egui::Context, app: &mut App, mods: Modifiers| {
        frame(ctx, app, vec![Event::PointerMoved(screen[0])], mods);
        frame(
            ctx,
            app,
            vec![button(screen[0], true, PointerButton::Primary, mods)],
            mods,
        );
        for p in &screen[1..] {
            frame(ctx, app, vec![Event::PointerMoved(*p)], mods);
        }
        frame(
            ctx,
            app,
            vec![button(
                *screen.last().expect("point"),
                false,
                PointerButton::Primary,
                mods,
            )],
            mods,
        );
        frame(ctx, app, Vec::new(), Modifiers::NONE);
    };
    lasso(&ctx, &mut app, Modifiers::NONE);
    assert_eq!(app.selection, vec![c]);
    // The handles scale as with the Pick tool.
    let before = page_bounds(&app, c);
    let ne = handle(&app, Handle::NE);
    drag(
        &ctx,
        &mut app,
        ne,
        ne + egui::vec2(40.0, -40.0),
        Modifiers::NONE,
        Vec::new(),
    );
    assert!(page_bounds(&app, c).width() > before.width() + 1.0);
    // A loop through the group's first square takes the group with Alt.
    app.select(Vec::new());
    let screen2 = lasso_path(
        &app,
        &[
            Point::new(110.0, 90.0),
            Point::new(130.0, 90.0),
            Point::new(130.0, 130.0),
            Point::new(110.0, 130.0),
        ],
    );
    let mods = Modifiers::ALT;
    frame(&ctx, &mut app, vec![Event::PointerMoved(screen2[0])], mods);
    frame(
        &ctx,
        &mut app,
        vec![button(screen2[0], true, PointerButton::Primary, mods)],
        mods,
    );
    for p in &screen2[1..] {
        frame(&ctx, &mut app, vec![Event::PointerMoved(*p)], mods);
    }
    frame(
        &ctx,
        &mut app,
        vec![button(
            *screen2.last().expect("point"),
            false,
            PointerButton::Primary,
            mods,
        )],
        mods,
    );
    frame(&ctx, &mut app, Vec::new(), Modifiers::NONE);
    assert_eq!(app.selection, vec![group]);
}

#[test]
fn clicking_the_selection_or_its_centre_mark_turns_the_handles() {
    let (ctx, mut app, [_a, _b, c], group) = grouped();
    app.select(vec![group, c]);
    frame(&ctx, &mut app, Vec::new(), Modifiers::NONE);
    // Any selected object toggles rotate mode for the whole selection.
    let on_c = app.view.to_screen(Point::new(110.0, 160.0));
    click_at(&ctx, &mut app, on_c, Modifiers::NONE);
    assert!(app.rotate_mode);
    assert_eq!(app.selection.len(), 2);
    // A moved centre of rotation stays where it was put when rotate mode
    // goes off and on again.
    let moved = Point::new(105.0, 105.0);
    app.set_pivot(moved);
    click_at(&ctx, &mut app, on_c, Modifiers::NONE);
    assert!(!app.rotate_mode);
    click_at(&ctx, &mut app, on_c, Modifiers::NONE);
    assert!(app.rotate_mode);
    assert_eq!(app.rotation_center(), Some(moved));
    // A double click on it puts it back in the middle.
    let middle = app.selection_bounds().expect("bounds").center();
    app.reset_pivot();
    assert_eq!(app.rotation_center(), Some(middle));
    // The centre mark of an outline-only ring: it lies on nothing (with
    // objects not treated as filled), yet a click there turns the handles
    // and a drag moves the ring.
    app.settings.treat_all_filled = false;
    app.select(Vec::new());
    let ring = app
        .new_shape(ShapeKind::Ellipse {
            rect: Rect::new(20.0, 200.0, 60.0, 240.0),
            arc: None,
        })
        .expect("ellipse");
    app.select(vec![ring]);
    frame(&ctx, &mut app, Vec::new(), Modifiers::NONE);
    let mark = app.view.to_screen(Point::new(40.0, 220.0));
    assert_eq!(app.hit_test(Point::new(40.0, 220.0)), None);
    assert_eq!(
        for_tool(&app, false, false, false, Some(mark)),
        ToolCursor::System(egui::CursorIcon::Move)
    );
    let before = page_bounds(&app, ring);
    let to = app.view.to_screen(Point::new(70.0, 220.0));
    drag(&ctx, &mut app, mark, to, Modifiers::NONE, Vec::new());
    assert!(close(page_bounds(&app, ring).x0 - before.x0, 30.0));
    assert_eq!(app.selection, vec![ring]);
    let moved = app.view.to_screen(Point::new(70.0, 220.0));
    click_at(&ctx, &mut app, moved, Modifiers::NONE);
    assert!(app.rotate_mode);
}

#[test]
fn the_pointer_shows_moves_for_objects_and_guidelines() {
    let (_ctx, mut app, [a, _b, _c], _group) = grouped();
    let cursor = |app: &App, at: Pos2| for_tool(app, false, false, false, Some(at));
    // Unselected objects show the move arrows too.
    assert!(app.selection.is_empty());
    let on_a = app.view.to_screen(Point::new(115.0, 110.0));
    assert_eq!(
        cursor(&app, on_a),
        ToolCursor::System(egui::CursorIcon::Move)
    );
    assert!(app.doc().is_nested(a));
    // Guidelines: arrows across them; a locked one keeps the arrow.
    app.add_guide(tracedraw_core::document::Guide::horizontal(250.0));
    app.add_guide(tracedraw_core::document::Guide::vertical(250.0));
    let on_h = app.view.to_screen(Point::new(200.0, 250.0));
    let on_v = app.view.to_screen(Point::new(250.0, 200.0));
    assert_eq!(
        cursor(&app, on_h),
        ToolCursor::System(egui::CursorIcon::ResizeVertical)
    );
    assert_eq!(
        cursor(&app, on_v),
        ToolCursor::System(egui::CursorIcon::ResizeHorizontal)
    );
    app.set_guides_locked(&[0], true);
    assert_eq!(
        cursor(&app, on_h),
        ToolCursor::System(egui::CursorIcon::Default)
    );
    // Dragging a new guideline out of a ruler.
    app.drag = Drag::NewGuide {
        horizontal: true,
        pos: Point::ZERO,
    };
    assert_eq!(
        crate::ui::cursors::ruler_drag_cursor(&app),
        Some(egui::CursorIcon::ResizeVertical)
    );
    app.drag = Drag::None;
}

#[test]
fn a_right_click_in_the_shape_tool_takes_the_node_or_segment_under_it() {
    let (ctx, mut app, id) = setup();
    app.convert_to_curves();
    app.set_tool(Tool::Shape);
    frame(&ctx, &mut app, Vec::new(), Modifiers::NONE);
    let right_click = |ctx: &egui::Context, app: &mut App, at: Pos2| {
        frame(ctx, app, vec![Event::PointerMoved(at)], Modifiers::NONE);
        frame(
            ctx,
            app,
            vec![button(at, true, PointerButton::Secondary, Modifiers::NONE)],
            Modifiers::NONE,
        );
        frame(
            ctx,
            app,
            vec![button(at, false, PointerButton::Secondary, Modifiers::NONE)],
            Modifiers::NONE,
        );
    };
    // The bottom side's middle: that segment's end node is chosen.
    let on_segment = app.view.to_screen(Point::new(35.0, 10.0));
    let (seg_shape, seg_index, _) = app.segment_at(Point::new(35.0, 10.0)).expect("segment");
    right_click(&ctx, &mut app, on_segment);
    assert!(app.context_menu.is_some());
    assert_eq!(app.node_selection, vec![(seg_shape, seg_index)]);
    assert_eq!(seg_shape, id);
    app.context_menu = None;
    frame(&ctx, &mut app, Vec::new(), Modifiers::NONE);
    // A corner: that node.
    let corner = app.view.to_screen(Point::new(60.0, 40.0));
    let hit = app.node_at(Point::new(60.0, 40.0)).expect("node");
    right_click(&ctx, &mut app, corner);
    assert_eq!(app.node_selection, vec![(hit.shape, hit.index)]);
    app.context_menu = None;
}

#[test]
fn scale_handles_snap_to_other_objects_not_to_the_selection() {
    let (ctx, mut app, id) = setup();
    // A second rectangle whose left edge is at x = 80.
    let other = app
        .new_shape(ShapeKind::Rect {
            rect: Rect::new(80.0, 10.0, 100.0, 40.0),
            radius: 0.0,
            corners: None,
        })
        .expect("rect");
    app.select(vec![id]);
    frame(&ctx, &mut app, Vec::new(), Modifiers::NONE);
    // Drag the right side handle to a little short of the other's edge.
    let e = handle(&app, Handle::E);
    let short = app.view.to_screen(Point::new(80.0, 25.0)) - egui::vec2(2.0, 0.0);
    drag(&ctx, &mut app, e, short, Modifiers::NONE, Vec::new());
    let b = bounds(&app, id);
    assert!((b.x1 - 80.0).abs() < 1e-6, "{b:?}");
    assert!((b.x0 - 10.0).abs() < 1e-6);
    // With snapping off the handle goes where the pointer is.
    app.undo();
    app.snap.off = true;
    let e = handle(&app, Handle::E);
    drag(&ctx, &mut app, e, short, Modifiers::NONE, Vec::new());
    let b = bounds(&app, id);
    assert!((b.x1 - 80.0).abs() > 0.1, "{b:?}");
    assert!(app.doc().find_shape(other).is_some());
}

#[test]
fn plus_duplicates_in_place_and_edits_nodes_in_the_shape_tool() {
    let (ctx, mut app, id) = setup();
    let n = shape_count(&app);
    frame(
        &ctx,
        &mut app,
        vec![key(egui::Key::Plus, Modifiers::NONE)],
        Modifiers::NONE,
    );
    assert_eq!(shape_count(&app), n + 1);
    let copy = app.selection[0];
    assert_ne!(copy, id);
    assert_eq!(bounds(&app, copy), bounds(&app, id));
    // The Shape tool: + adds a node in the middle of the segment ending at
    // the chosen node, - deletes the chosen nodes.
    app.select(vec![id]);
    app.convert_to_curves();
    app.set_tool(Tool::Shape);
    frame(&ctx, &mut app, Vec::new(), Modifiers::NONE);
    let count = |app: &App| {
        app.doc()
            .find_shape(id)
            .and_then(|s| match &s.kind {
                ShapeKind::Path { path, .. } => Some(tracedraw_core::nodes::nodes(path).len()),
                _ => None,
            })
            .expect("curve")
    };
    let before = count(&app);
    let hit = app.node_at(Point::new(60.0, 40.0)).expect("node");
    app.node_selection = vec![(hit.shape, hit.index)];
    frame(
        &ctx,
        &mut app,
        vec![key(egui::Key::Plus, Modifiers::NONE)],
        Modifiers::NONE,
    );
    assert_eq!(count(&app), before + 1);
    let hit = app.node_at(Point::new(60.0, 40.0)).expect("node");
    app.node_selection = vec![(hit.shape, hit.index)];
    frame(
        &ctx,
        &mut app,
        vec![key(egui::Key::Minus, Modifiers::NONE)],
        Modifiers::NONE,
    );
    assert_eq!(count(&app), before);
    // Nothing chosen: + zooms.
    app.node_selection.clear();
    let zoom = app.view.zoom;
    frame(
        &ctx,
        &mut app,
        vec![key(egui::Key::Plus, Modifiers::NONE)],
        Modifiers::NONE,
    );
    assert!(app.view.zoom > zoom);
}

#[test]
fn dragging_a_segment_bends_it_and_a_click_marks_where_a_node_goes() {
    let (ctx, mut app, id) = setup();
    app.convert_to_curves();
    app.set_tool(Tool::Shape);
    frame(&ctx, &mut app, Vec::new(), Modifiers::NONE);
    let path_of = |app: &App| match app.doc().find_shape(id).map(|s| s.kind.clone()) {
        Some(ShapeKind::Path { path, .. }) => path,
        other => panic!("not a curve: {other:?}"),
    };
    let before = path_of(&app);
    // Drag the middle of the bottom side 10 mm down.
    let (_, index, t) = app.segment_at(Point::new(35.0, 10.0)).expect("segment");
    let from = app.view.to_screen(Point::new(35.0, 10.0));
    let to = app.view.to_screen(Point::new(35.0, 0.0));
    drag(&ctx, &mut app, from, to, Modifiers::NONE, Vec::new());
    let bent = path_of(&app);
    let mid = tracedraw_core::nodes::point_on_segment(&bent, index, t).expect("point");
    assert!((mid - Point::new(35.0, 0.0)).hypot() < 0.3, "{mid:?}");
    // The corners stay; one undo puts the side back.
    let corners = |p: &tracedraw_core::BezPath| {
        tracedraw_core::nodes::nodes(p)
            .iter()
            .map(|n| n.pos)
            .collect::<Vec<_>>()
    };
    assert_eq!(corners(&bent), corners(&before));
    app.undo();
    assert_eq!(corners(&path_of(&app)), corners(&before));
    let mid = tracedraw_core::nodes::point_on_segment(&path_of(&app), index, t).expect("point");
    assert!((mid.y - 10.0).abs() < 1e-6);
    // A click on the right side chooses it and marks the spot; + puts a
    // node there.
    let n = corners(&path_of(&app)).len();
    let spot = Point::new(60.0, 32.0);
    let at = app.view.to_screen(spot);
    click_at(&ctx, &mut app, at, Modifiers::NONE);
    let (shape, seg, _) = app.segment_spot.expect("spot");
    assert_eq!(shape, id);
    assert_eq!(app.node_selection, vec![(id, seg)]);
    frame(
        &ctx,
        &mut app,
        vec![key(egui::Key::Plus, Modifiers::NONE)],
        Modifiers::NONE,
    );
    let after = corners(&path_of(&app));
    assert_eq!(after.len(), n + 1);
    assert!(after.iter().any(|q| (*q - spot).hypot() < 0.3), "{after:?}");
}

#[test]
fn node_keys_in_the_shape_tool() {
    let (ctx, mut app, id) = setup();
    // A curve with a smooth-able middle node.
    let mut path = tracedraw_core::BezPath::new();
    path.move_to((10.0, 10.0));
    path.curve_to((20.0, 10.0), (30.0, 20.0), (40.0, 30.0));
    path.curve_to((45.0, 40.0), (55.0, 30.0), (70.0, 20.0));
    app.select(vec![id]);
    app.delete_selection();
    let c = app
        .new_shape(ShapeKind::Path {
            path,
            closed: false,
        })
        .expect("curve");
    app.select(vec![c]);
    app.set_tool(Tool::Shape);
    frame(&ctx, &mut app, Vec::new(), Modifiers::NONE);
    let hit = app.node_at(Point::new(40.0, 30.0)).expect("node");
    app.node_selection = vec![(hit.shape, hit.index)];
    let ty = |app: &App| app.node_type_of(c, hit.index);
    use tracedraw_core::nodes::NodeType;
    assert_eq!(ty(&app), NodeType::Cusp);
    // C switches cusp and smooth, S symmetrical and smooth (a cusp node
    // becomes symmetrical), Y makes it symmetrical. The type is kept with
    // the curve, so the way back shows although the handles stay aligned.
    for (k, want) in [
        (egui::Key::C, NodeType::Smooth),
        (egui::Key::C, NodeType::Cusp),
        (egui::Key::S, NodeType::Symmetrical),
        (egui::Key::S, NodeType::Smooth),
        (egui::Key::S, NodeType::Symmetrical),
        (egui::Key::C, NodeType::Cusp),
        (egui::Key::Y, NodeType::Symmetrical),
    ] {
        frame(
            &ctx,
            &mut app,
            vec![key(k, Modifiers::NONE)],
            Modifiers::NONE,
        );
        assert_eq!(ty(&app), want, "{k:?}");
        // Still the Shape tool, nothing aligned.
        assert_eq!(app.tool, Tool::Shape);
    }
    // C makes the symmetrical node cusp; its handles stay aligned, and a
    // handle dragged now moves alone.
    frame(
        &ctx,
        &mut app,
        vec![key(egui::Key::C, Modifiers::NONE)],
        Modifiers::NONE,
    );
    assert_eq!(ty(&app), NodeType::Cusp);
    let handles = |app: &App| match app.doc().find_shape(c).map(|s| s.kind.clone()) {
        Some(ShapeKind::Path { path, .. }) => tracedraw_core::nodes::nodes(&path)
            .into_iter()
            .find(|n| n.index == hit.index)
            .map(|n| (n.ctrl_in, n.ctrl_out))
            .expect("node"),
        _ => panic!("curve"),
    };
    let (before_in, before_out) = handles(&app);
    let out = before_out.expect("out handle");
    let from = app.view.to_screen(out);
    drag(
        &ctx,
        &mut app,
        from,
        from + egui::vec2(0.0, -25.0),
        Modifiers::NONE,
        Vec::new(),
    );
    let (after_in, after_out) = handles(&app);
    assert_ne!(after_out, before_out, "the dragged handle moved");
    assert_eq!(after_in, before_in, "the other handle stayed");
    assert_eq!(ty(&app), NodeType::Cusp);
    app.node_selection = vec![(hit.shape, hit.index)];
    // Home and End choose the curve's first and last node.
    frame(
        &ctx,
        &mut app,
        vec![key(egui::Key::Home, Modifiers::NONE)],
        Modifiers::NONE,
    );
    assert_eq!(app.node_selection, vec![(c, 0)]);
    frame(
        &ctx,
        &mut app,
        vec![key(egui::Key::End, Modifiers::NONE)],
        Modifiers::NONE,
    );
    let last = app.node_selection.clone();
    assert_eq!(last.len(), 1);
    assert!(last[0].1 > hit.index, "{last:?}");
    // The arrow keys move the chosen node, not the curve.
    let before = app.doc().find_shape(c).map(|s| s.bounds()).expect("curve");
    frame(
        &ctx,
        &mut app,
        vec![key(egui::Key::ArrowRight, Modifiers::NONE)],
        Modifiers::NONE,
    );
    let after = app.doc().find_shape(c).map(|s| s.bounds()).expect("curve");
    assert!(
        (after.x0 - before.x0).abs() < 1e-9,
        "the start stays: {before:?} {after:?}"
    );
    assert!(
        after.x1 > before.x1 + 1e-6,
        "the end node moved right: {before:?} {after:?}"
    );
    // Ctrl+Space: the Pick tool.
    frame(
        &ctx,
        &mut app,
        vec![key(egui::Key::Space, Modifiers::COMMAND)],
        Modifiers::COMMAND,
    );
    assert_eq!(app.tool, Tool::Pick);
}

#[test]
fn two_fingers_pan_without_dragging_anything() {
    let (ctx, mut app, id) = setup();
    let before = bounds(&app, id);
    let origin = app.view.to_screen(Point::new(0.0, 0.0));
    let touch = |id: u64, phase: egui::TouchPhase, pos: Pos2| Event::Touch {
        device_id: egui::TouchDeviceId(1),
        id: egui::TouchId(id),
        phase,
        pos,
        force: None,
    };
    // Both fingers on the object, then both move 60 points right. The
    // windowing layer also turns the first finger into the pointer.
    let (a, b) = (
        app.view.to_screen(Point::new(30.0, 25.0)),
        app.view.to_screen(Point::new(40.0, 25.0)),
    );
    frame(
        &ctx,
        &mut app,
        vec![
            Event::PointerMoved(a),
            button(a, true, PointerButton::Primary, Modifiers::NONE),
            touch(0, egui::TouchPhase::Start, a),
            touch(1, egui::TouchPhase::Start, b),
        ],
        Modifiers::NONE,
    );
    for i in 1..=6 {
        let d = egui::vec2(10.0 * i as f32, 0.0);
        frame(
            &ctx,
            &mut app,
            vec![
                Event::PointerMoved(a + d),
                touch(0, egui::TouchPhase::Move, a + d),
                touch(1, egui::TouchPhase::Move, b + d),
            ],
            Modifiers::NONE,
        );
    }
    let end = egui::vec2(60.0, 0.0);
    frame(
        &ctx,
        &mut app,
        vec![
            button(a + end, false, PointerButton::Primary, Modifiers::NONE),
            touch(0, egui::TouchPhase::End, a + end),
            touch(1, egui::TouchPhase::End, b + end),
        ],
        Modifiers::NONE,
    );
    frame(&ctx, &mut app, Vec::new(), Modifiers::NONE);
    // The view moved with the fingers; the object did not.
    let moved = app.view.to_screen(Point::new(0.0, 0.0)) - origin;
    assert!(moved.x > 30.0, "{moved:?}");
    assert_eq!(bounds(&app, id), before);
    assert!(matches!(app.drag, Drag::None));
}

#[test]
fn a_moved_centre_of_rotation_is_kept_with_the_object() {
    let (ctx, mut app, id) = setup();
    // Rotate mode: drag the centre of rotation to the rectangle's lower
    // left corner area.
    let inside = app.view.to_screen(Point::new(35.0, 25.0));
    click_at(&ctx, &mut app, inside, Modifiers::NONE);
    assert!(app.rotate_mode);
    let centre = app.view.to_screen(app.rotation_center().expect("centre"));
    let target = app.view.to_screen(Point::new(20.0, 15.0));
    app.snap.off = true;
    drag(&ctx, &mut app, centre, target, Modifiers::NONE, Vec::new());
    let put = app.rotation_center().expect("centre");
    assert!((put - Point::new(20.0, 15.0)).hypot() < 0.5, "{put:?}");
    // Click away (nothing selected), then select the rectangle again and
    // turn rotate mode on: the centre is where it was put.
    let empty = app.view.to_screen(Point::new(150.0, 250.0));
    click_at(&ctx, &mut app, empty, Modifiers::NONE);
    assert!(app.selection.is_empty());
    click_at(&ctx, &mut app, inside, Modifiers::NONE);
    assert_eq!(app.selection, vec![id]);
    click_at(&ctx, &mut app, inside, Modifiers::NONE);
    assert!(app.rotate_mode);
    assert_eq!(app.rotation_center(), Some(put));
    // It travels with the object.
    app.transform_selection(Affine::translate((10.0, 5.0)));
    let after = app.rotation_center().expect("centre");
    assert!(
        (after - (put + Vec2::new(10.0, 5.0))).hypot() < 1e-9,
        "{after:?}"
    );
    // Another drawing has its own centres.
    app.new_document();
    assert!(app.pivots.single.is_empty());
}

/// The windowing layer delivers Ctrl+C, Ctrl+X and Ctrl+V as copy, cut
/// and paste events (a paste only while the system clipboard holds text),
/// never as key presses: those events copy, cut and paste objects.
#[test]
fn copy_cut_and_paste_events_work_on_objects() {
    let (ctx, mut app, id) = setup();
    let n = shape_count(&app);
    frame(&ctx, &mut app, vec![Event::Copy], Modifiers::COMMAND);
    assert!(app.clipboard.is_some());
    frame(
        &ctx,
        &mut app,
        vec![Event::Paste("<svg/>".into())],
        Modifiers::COMMAND,
    );
    assert_eq!(shape_count(&app), n + 1);
    let copy = app.selection[0];
    assert_ne!(copy, id);
    // One paste per frame, even if a key press comes along with it.
    frame(
        &ctx,
        &mut app,
        vec![
            key(egui::Key::V, Modifiers::COMMAND),
            Event::Paste("x".into()),
        ],
        Modifiers::COMMAND,
    );
    assert_eq!(shape_count(&app), n + 2);
    // Cut takes the selection away; paste brings it back.
    frame(&ctx, &mut app, vec![Event::Cut], Modifiers::COMMAND);
    assert_eq!(shape_count(&app), n + 1);
    frame(
        &ctx,
        &mut app,
        vec![Event::Paste("x".into())],
        Modifiers::COMMAND,
    );
    assert_eq!(shape_count(&app), n + 2);
    // With Shift held the paste lands in the middle of the view.
    frame(
        &ctx,
        &mut app,
        vec![Event::Paste("x".into())],
        Modifiers::COMMAND | Modifiers::SHIFT,
    );
    assert_eq!(shape_count(&app), n + 3);
    let view = app.view.visible_page_rect(app.canvas_rect).center();
    let b = app.selection_bounds().expect("bounds");
    assert!((b.center() - view).hypot() < 1e-6);
}

#[test]
fn handles_stand_outside_the_box_and_grabbing_one_does_not_jump() {
    let (ctx, mut app, id) = setup();
    let before = bounds(&app, id);
    let r = app.view.rect_to_screen(before);
    let off = crate::interaction::HANDLE_OFFSET;
    // Each handle stands off the box, so the outline stays in view.
    let e = handle(&app, Handle::E);
    let nw = handle(&app, Handle::NW);
    assert!((e.x - (r.right() + off)).abs() < 1e-3, "{e:?} {r:?}");
    assert!((nw.x - (r.left() - off)).abs() < 1e-3, "{nw:?} {r:?}");
    assert!((nw.y - (r.top() - off)).abs() < 1e-3, "{nw:?} {r:?}");
    // Dragging the right handle 20 points widens the object by 20 points,
    // not by the handle's distance from the box as well.
    drag(
        &ctx,
        &mut app,
        e,
        e + egui::vec2(20.0, 0.0),
        Modifiers::NONE,
        Vec::new(),
    );
    let after = bounds(&app, id);
    let grown = 20.0 / app.view.zoom as f64;
    assert!(
        close(after.width(), before.width() + grown),
        "{before:?} {after:?}"
    );
    assert!(close(after.x0, before.x0), "{before:?} {after:?}");
}

#[test]
fn turning_snaps_the_pointer_to_another_objects_node() {
    let (ctx, mut app, id) = setup();
    // Another rectangle up and to the right; its bottom left corner is a
    // node at (100, 80).
    let other = app
        .new_shape(ShapeKind::Rect {
            rect: Rect::new(100.0, 80.0, 130.0, 110.0),
            radius: 0.0,
            corners: None,
        })
        .expect("rect");
    app.select(vec![id]);
    app.rotate_mode = true;
    frame(&ctx, &mut app, Vec::new(), Modifiers::NONE);
    let c = app.rotation_center().expect("centre");
    let ne = handle(&app, Handle::NE);
    let start = app.view.to_page(ne);
    // Released a few points off the node: the turn ends on the node.
    let node = Point::new(100.0, 80.0);
    let near = app.view.to_screen(node) + egui::vec2(3.0, 2.0);
    drag(&ctx, &mut app, ne, near, Modifiers::NONE, Vec::new());
    let t = app
        .doc()
        .find_shape(id)
        .map(|s| s.transform)
        .expect("shape");
    let k = t.as_coeffs();
    let turned = k[1].atan2(k[0]);
    let want = (node - c).atan2() - (start - c).atan2();
    assert!((turned - want).abs() < 1e-6, "{turned} {want}");
    assert!(app.doc().find_shape(other).is_some());
}

#[test]
fn drawing_a_new_shape_lets_the_selection_go_at_once() {
    let (ctx, mut app, id) = setup();
    assert_eq!(app.selection, vec![id]);
    app.set_tool(Tool::Rectangle);
    // Somewhere empty on the page, away from the rectangle.
    let from = app.view.to_screen(Point::new(100.0, 150.0));
    let to = app.view.to_screen(Point::new(140.0, 120.0));
    let none = Modifiers::NONE;
    frame(&ctx, &mut app, vec![Event::PointerMoved(from)], none);
    frame(
        &ctx,
        &mut app,
        vec![button(from, true, PointerButton::Primary, none)],
        none,
    );
    for i in 1..=3 {
        let p = from + (to - from) * (i as f32 / 3.0);
        frame(&ctx, &mut app, vec![Event::PointerMoved(p)], none);
    }
    // While the new rectangle is being drawn, nothing is selected.
    assert!(matches!(app.drag, Drag::Box { .. }));
    assert!(app.selection.is_empty(), "{:?}", app.selection);
    frame(
        &ctx,
        &mut app,
        vec![button(to, false, PointerButton::Primary, none)],
        none,
    );
    frame(&ctx, &mut app, Vec::new(), none);
    // Then the new rectangle alone is.
    assert_eq!(app.selection.len(), 1);
    assert_ne!(app.selection[0], id);
}

#[test]
fn ctrl_click_goes_one_group_level_in_at_a_time() {
    let (ctx, mut app, [a, _b, c], inner) = grouped();
    // The group of two squares and the third square in an outer group.
    app.select(vec![inner, c]);
    app.group_selection();
    let outer = app.selection[0];
    app.select(Vec::new());
    frame(&ctx, &mut app, Vec::new(), Modifiers::NONE);
    let on_a = app.view.to_screen(Point::new(115.0, 110.0));
    click_at(&ctx, &mut app, on_a, Modifiers::NONE);
    assert_eq!(app.selection, vec![outer]);
    // Each Ctrl+click goes one level further in, then stays on the square.
    click_at(&ctx, &mut app, on_a, Modifiers::CTRL);
    assert_eq!(app.selection, vec![inner]);
    click_at(&ctx, &mut app, on_a, Modifiers::CTRL);
    assert_eq!(app.selection, vec![a]);
    click_at(&ctx, &mut app, on_a, Modifiers::CTRL);
    assert_eq!(app.selection, vec![a]);
}

#[test]
fn ctrl_keeps_the_marquee_square_and_space_copies_while_turning() {
    let (ctx, mut app, id) = setup();
    // Space while turning leaves a copy at the angle reached.
    app.rotate_mode = true;
    frame(&ctx, &mut app, Vec::new(), Modifiers::NONE);
    let ne = handle(&app, Handle::NE);
    let count = shape_count(&app);
    drag(
        &ctx,
        &mut app,
        ne,
        ne + egui::vec2(-40.0, -60.0),
        Modifiers::NONE,
        vec![key(egui::Key::Space, Modifiers::NONE)],
    );
    assert_eq!(shape_count(&app), count + 1);
    assert!(app.doc().find_shape(id).is_some());
    // A marquee dragged with Ctrl is square.
    app.rotate_mode = false;
    app.select(Vec::new());
    let from = app.view.to_screen(Point::new(150.0, 250.0));
    frame(
        &ctx,
        &mut app,
        vec![Event::PointerMoved(from)],
        Modifiers::CTRL,
    );
    frame(
        &ctx,
        &mut app,
        vec![button(from, true, PointerButton::Primary, Modifiers::CTRL)],
        Modifiers::CTRL,
    );
    for i in 1..=3 {
        let p = from + egui::vec2(60.0, 20.0) * (i as f32 / 3.0);
        frame(
            &ctx,
            &mut app,
            vec![Event::PointerMoved(p)],
            Modifiers::CTRL,
        );
    }
    match app.drag {
        Drag::Marquee { start, current, .. } => {
            let d = current - start;
            assert!((d.x.abs() - d.y.abs()).abs() < 1e-9, "{d:?}");
        }
        ref other => panic!("{other:?}"),
    }
    let end = from + egui::vec2(60.0, 20.0);
    frame(
        &ctx,
        &mut app,
        vec![button(end, false, PointerButton::Primary, Modifiers::CTRL)],
        Modifiers::CTRL,
    );
}

#[test]
fn selection_groups_store_recall_add_and_zoom() {
    let (ctx, mut app, id) = setup();
    let none = Modifiers::NONE;
    let press = |app: &mut App, k: egui::Key, m: Modifiers| frame(&ctx, app, vec![key(k, m)], m);
    // Ctrl+3 stores the selection; 3 selects it again.
    press(&mut app, egui::Key::Num3, Modifiers::COMMAND);
    assert_eq!(app.doc().selection_groups.get(&3), Some(&vec![id]));
    app.select(Vec::new());
    press(&mut app, egui::Key::Num3, none);
    assert_eq!(app.selection, vec![id]);
    // 3 again zooms to it.
    let zoom = app.view.zoom;
    press(&mut app, egui::Key::Num3, none);
    assert!(
        (app.view.zoom - zoom).abs() > 1e-6,
        "{zoom} {}",
        app.view.zoom
    );
    // Alt+3 adds another object to the group.
    let other = app
        .new_shape(ShapeKind::Rect {
            rect: Rect::new(100.0, 100.0, 120.0, 120.0),
            radius: 0.0,
            corners: None,
        })
        .expect("rect");
    app.select(vec![other]);
    press(&mut app, egui::Key::Num3, Modifiers::ALT);
    app.select(Vec::new());
    press(&mut app, egui::Key::Num3, none);
    assert_eq!(app.selection.len(), 2);
    // A deleted object drops out; an unused number does nothing.
    app.select(vec![other]);
    app.delete_selection();
    press(&mut app, egui::Key::Num3, none);
    assert_eq!(app.selection, vec![id]);
    press(&mut app, egui::Key::Num7, none);
    assert_eq!(app.selection, vec![id]);
}

#[test]
fn ctrl_keeps_the_freehand_selection_a_rectangle() {
    let (ctx, mut app, _id) = setup();
    app.set_tool(Tool::FreeformPick);
    app.select(Vec::new());
    let ctrl = Modifiers::CTRL;
    let from = app.view.to_screen(Point::new(150.0, 250.0));
    frame(&ctx, &mut app, vec![Event::PointerMoved(from)], ctrl);
    frame(
        &ctx,
        &mut app,
        vec![button(from, true, PointerButton::Primary, ctrl)],
        ctrl,
    );
    for i in 1..=4 {
        let p = from
            + egui::vec2(50.0, 30.0) * (i as f32 / 4.0)
            + egui::vec2(0.0, (i % 2) as f32 * 7.0);
        frame(&ctx, &mut app, vec![Event::PointerMoved(p)], ctrl);
    }
    match &app.drag {
        Drag::Lasso { points, .. } => {
            assert_eq!(points.len(), 4, "{points:?}");
            assert!(
                (points[0].y - points[1].y).abs() < 1e-9
                    && (points[1].x - points[2].x).abs() < 1e-9
            );
        }
        other => panic!("{other:?}"),
    }
    let end = from + egui::vec2(50.0, 30.0);
    frame(
        &ctx,
        &mut app,
        vec![button(end, false, PointerButton::Primary, ctrl)],
        ctrl,
    );
}

#[test]
fn node_tracking_drags_a_curves_node_with_the_pick_tool() {
    let (ctx, mut app, id) = setup();
    app.convert_to_curves();
    frame(&ctx, &mut app, Vec::new(), Modifiers::NONE);
    // The rectangle's corner node at (60, 40), dragged 10 mm right.
    let corner = app.view.to_screen(Point::new(60.0, 40.0));
    let to = app.view.to_screen(Point::new(70.0, 40.0));
    // Off (the default): the whole object moves.
    assert!(!app.settings.node_tracking);
    let before = bounds(&app, id);
    drag(&ctx, &mut app, corner, to, Modifiers::NONE, Vec::new());
    let after = bounds(&app, id);
    assert!(close(after.width(), before.width()), "{before:?} {after:?}");
    assert!(close(after.x0 - before.x0, 10.0), "{before:?} {after:?}");
    app.undo();
    // On: only that node moves; the object widens on its right side.
    app.settings.node_tracking = true;
    app.select(vec![id]);
    frame(&ctx, &mut app, Vec::new(), Modifiers::NONE);
    let before = bounds(&app, id);
    drag(&ctx, &mut app, corner, to, Modifiers::NONE, Vec::new());
    let after = bounds(&app, id);
    assert!(close(after.x0, before.x0), "{before:?} {after:?}");
    assert!(close(after.x1, before.x1 + 10.0), "{before:?} {after:?}");
    assert_eq!(app.tool, Tool::Pick);
    // One undo step puts the node back.
    app.undo();
    assert!(close(bounds(&app, id).x1, before.x1));
}

#[test]
fn bspline_clicks_place_control_points_of_a_smooth_curve() {
    let (ctx, mut app, _) = setup();
    app.select(Vec::new());
    app.set_tool(Tool::BSpline);
    let pts = [
        Point::new(100.0, 150.0),
        Point::new(130.0, 190.0),
        Point::new(180.0, 180.0),
        Point::new(190.0, 140.0),
    ];
    for p in pts {
        let at = app.view.to_screen(p);
        click_at(&ctx, &mut app, at, Modifiers::NONE);
    }
    app.finish_curve();
    let path = last_path(&app);
    let segs: Vec<_> = path.segments().collect();
    // Four control points: one cubic from the first to the last point,
    // the middle two as its handles (not four corners joined by lines).
    assert_eq!(segs.len(), 1, "{segs:?}");
    let tracedraw_core::geometry::PathSeg::Cubic(c) = segs[0] else {
        panic!("a cubic: {segs:?}");
    };
    let near = |a: Point, b: Point| (a - b).hypot() < 0.5;
    assert!(near(c.p0, pts[0]) && near(c.p3, pts[3]), "{c:?}");
    assert!(near(c.p1, pts[1]) && near(c.p2, pts[2]), "{c:?}");
}

/// With alignment guides on (and every other snapping off), a move of the
/// rectangle near another one lands its top edge on the other's top and
/// shows the line while the button is down; a scale handle lines up the
/// same way.
#[test]
fn alignment_guides_line_up_a_moved_object_and_show_the_line() {
    let (ctx, mut app, id) = setup();
    let other = app
        .new_shape(ShapeKind::Rect {
            rect: Rect::new(100.0, 60.0, 130.0, 90.0),
            radius: 0.0,
            corners: None,
        })
        .expect("rect");
    app.select(vec![id]);
    app.snap.objects = false;
    app.snap.page = false;
    app.snap.guides = false;
    app.snap.alignment_guides = true;
    // The radius is 10 px; the top edge goes 1 mm past y = 90.
    let from = app.view.to_screen(Point::new(35.0, 25.0));
    let to = app.view.to_screen(Point::new(35.0, 25.0 + 51.0));
    let none = Modifiers::NONE;
    frame(&ctx, &mut app, vec![Event::PointerMoved(from)], none);
    frame(
        &ctx,
        &mut app,
        vec![button(from, true, PointerButton::Primary, none)],
        none,
    );
    for i in 1..=4 {
        let p = from + (to - from) * (i as f32 / 4.0);
        frame(&ctx, &mut app, vec![Event::PointerMoved(p)], none);
    }
    {
        let o = app.live_guides.borrow();
        assert!(
            o.lines
                .iter()
                .any(|l| (l.a.y - 90.0).abs() < 1e-9 && (l.b.y - 90.0).abs() < 1e-9),
            "{o:?}"
        );
    }
    frame(
        &ctx,
        &mut app,
        vec![button(to, false, PointerButton::Primary, none)],
        none,
    );
    frame(&ctx, &mut app, Vec::new(), none);
    let b = bounds(&app, id);
    assert!((b.y1 - 90.0).abs() < 1e-6, "{b:?}");
    assert!(close(b.x0, 10.0), "{b:?}");
    // The guides go when the button is up.
    assert!(app.live_guides.borrow().is_empty());
    // A right side handle dragged near the other's left edge stops on it.
    let h = handle(&app, Handle::E);
    let target = app.view.to_screen(Point::new(99.0, b.center().y));
    drag(
        &ctx,
        &mut app,
        h,
        egui::pos2(target.x, h.y),
        none,
        Vec::new(),
    );
    let b = bounds(&app, id);
    assert!((b.x1 - 100.0).abs() < 1e-6, "{b:?}");
    // With snapping off (Alt+Q) the same move goes where the pointer goes.
    app.snap.off = true;
    let from = app.view.to_screen(b.center());
    let to = app.view.to_screen(b.center() + Vec2::new(0.0, -5.0));
    drag(&ctx, &mut app, from, to, none, Vec::new());
    let after = bounds(&app, id);
    assert!((after.y1 - 85.0).abs() < 0.2, "{after:?}");
    let _ = other;
}

/// With alignment guides on, a rectangle drawn near the size of another
/// object takes its width and height.
#[test]
fn a_rectangle_drawn_with_alignment_guides_takes_another_objects_size() {
    let (ctx, mut app, id) = setup();
    app.snap.objects = false;
    app.snap.page = false;
    app.snap.guides = false;
    app.snap.alignment_guides = true;
    app.settings.snap.align.page = false;
    app.set_tool(Tool::Rectangle);
    // The other rectangle is 50 x 30 mm; this one is dragged 51 x 31.
    let from = app.view.to_screen(Point::new(100.0, 100.0));
    let to = app.view.to_screen(Point::new(151.0, 131.0));
    drag(&ctx, &mut app, from, to, Modifiers::NONE, Vec::new());
    let new = app.selection.first().copied().expect("a new rectangle");
    assert_ne!(new, id);
    let b = bounds(&app, new);
    assert!((b.width() - 50.0).abs() < 1e-6, "{b:?}");
    assert!((b.height() - 30.0).abs() < 1e-6, "{b:?}");
    // Without the option it keeps the height dragged.
    app.settings.snap.align.equal_size = false;
    let from = app.view.to_screen(Point::new(100.0, 200.0));
    // (Far from the first one's edges, which it would line up with.)
    let to = app.view.to_screen(Point::new(157.0, 231.0));
    drag(&ctx, &mut app, from, to, Modifiers::NONE, Vec::new());
    let b = bounds(&app, app.selection.first().copied().expect("another"));
    assert!((b.height() - 31.0).abs() < 0.1, "{b:?}");
}

/// A curve tool's drag from `from` to `to`, node placed at the press.
fn pull(ctx: &egui::Context, app: &mut App, from: Point, to: Point) {
    let (a, b) = (app.view.to_screen(from), app.view.to_screen(to));
    drag(ctx, app, a, b, Modifiers::NONE, Vec::new());
    for _ in 0..20 {
        frame(ctx, app, Vec::new(), Modifiers::NONE);
    }
}

fn click_page(ctx: &egui::Context, app: &mut App, p: Point) {
    let at = app.view.to_screen(p);
    click_at(ctx, app, at, Modifiers::NONE);
}

fn cubic(seg: tracedraw_core::geometry::PathSeg) -> tracedraw_core::geometry::CubicBez {
    match seg {
        tracedraw_core::geometry::PathSeg::Cubic(c) => c,
        other => panic!("a cubic: {other:?}"),
    }
}

fn near(a: Point, b: Point) -> bool {
    (a - b).hypot() < 0.6
}

#[test]
fn a_pen_click_after_a_pulled_node_gives_a_straight_segment() {
    let (ctx, mut app, _) = setup();
    app.select(Vec::new());
    app.set_tool(Tool::Pen);
    let (a, b, c) = (
        Point::new(100.0, 150.0),
        Point::new(140.0, 150.0),
        Point::new(180.0, 150.0),
    );
    click_page(&ctx, &mut app, a);
    pull(&ctx, &mut app, b, Point::new(150.0, 170.0));
    click_page(&ctx, &mut app, c);
    app.finish_curve();
    let segs: Vec<_> = last_path(&app).segments().collect();
    assert_eq!(segs.len(), 2, "{segs:?}");
    // Into the pulled node: a curve arriving along its mirrored handle.
    let s0 = cubic(segs[0]);
    assert!(near(s0.p2, Point::new(130.0, 130.0)), "{s0:?}");
    // Out of it after a plain click: straight, the node a cusp.
    assert!(
        matches!(segs[1], tracedraw_core::geometry::PathSeg::Line(_)),
        "{segs:?}"
    );
}

#[test]
fn clicking_the_last_node_again_makes_it_a_cusp() {
    let (ctx, mut app, _) = setup();
    app.select(Vec::new());
    app.set_tool(Tool::Pen);
    let (a, b, c) = (
        Point::new(100.0, 150.0),
        Point::new(140.0, 150.0),
        Point::new(180.0, 150.0),
    );
    click_page(&ctx, &mut app, a);
    pull(&ctx, &mut app, b, Point::new(150.0, 170.0));
    // The pointer over the last node shows that a click edits it.
    let over = app.view.to_screen(b);
    frame(
        &ctx,
        &mut app,
        vec![Event::PointerMoved(over)],
        Modifiers::NONE,
    );
    assert_eq!(
        for_tool(&app, false, false, false, Some(over)),
        ToolCursor::CurveBadge(Tool::Pen, crate::ui::cursors::CurveMark::EditNode)
    );
    click_page(&ctx, &mut app, b);
    assert_eq!(app.curve.as_ref().map(|c| c.nodes.len()), Some(2));
    // The next node is pulled: the segment leaves the cusp without a
    // handle and arrives along the new one; the segment before keeps its
    // curve into the cusp.
    pull(&ctx, &mut app, c, Point::new(190.0, 130.0));
    app.finish_curve();
    let segs: Vec<_> = last_path(&app).segments().collect();
    assert_eq!(segs.len(), 2, "{segs:?}");
    let s0 = cubic(segs[0]);
    assert!(near(s0.p2, Point::new(130.0, 130.0)), "{s0:?}");
    let s1 = cubic(segs[1]);
    assert!(near(s1.p1, b), "{s1:?}");
    assert!(near(s1.p2, Point::new(170.0, 170.0)), "{s1:?}");
}

#[test]
fn dragging_from_the_first_node_shapes_only_the_closing_segment() {
    let (ctx, mut app, _) = setup();
    app.select(Vec::new());
    app.set_tool(Tool::Pen);
    let a = Point::new(100.0, 150.0);
    let a_out = Point::new(110.0, 170.0);
    pull(&ctx, &mut app, a, a_out);
    pull(
        &ctx,
        &mut app,
        Point::new(150.0, 190.0),
        Point::new(170.0, 190.0),
    );
    click_page(&ctx, &mut app, Point::new(160.0, 130.0));
    // The pointer over the first node shows a close.
    let over = app.view.to_screen(a);
    assert_eq!(
        for_tool(&app, false, false, false, Some(over)),
        ToolCursor::CurveBadge(Tool::Pen, crate::ui::cursors::CurveMark::Close)
    );
    pull(&ctx, &mut app, a, Point::new(90.0, 140.0));
    assert!(app.curve.is_none());
    let path = last_path(&app);
    let segs: Vec<_> = path.segments().collect();
    // The first node keeps its own outgoing handle.
    let s0 = cubic(segs[0]);
    assert!(near(s0.p1, a_out), "{s0:?}");
    // The closing segment arrives at the first node along the mirror of
    // the drag.
    let last = segs
        .iter()
        .rev()
        .find_map(|s| match s {
            tracedraw_core::geometry::PathSeg::Cubic(c) if near(c.p3, a) => Some(*c),
            _ => None,
        })
        .expect("a curved closing segment");
    assert!(near(last.p2, Point::new(110.0, 160.0)), "{last:?}");
    assert!(near(last.p1, Point::new(160.0, 130.0)), "{last:?}");
}

/// A curve from (100, 100) through (150, 100) to (200, 100), selected,
/// with the Shape tool.
fn line_curve(ctx: &egui::Context, app: &mut App) -> ShapeId {
    let mut path = tracedraw_core::BezPath::new();
    path.move_to((100.0, 100.0));
    path.line_to((150.0, 100.0));
    path.line_to((200.0, 100.0));
    let id = app
        .new_shape(ShapeKind::Path {
            path,
            closed: false,
        })
        .expect("curve");
    app.select(vec![id]);
    for _ in 0..2 {
        frame(ctx, app, Vec::new(), Modifiers::NONE);
    }
    id
}

fn node_count(app: &App, id: ShapeId) -> usize {
    match &app.doc().find_shape(id).expect("curve").kind {
        ShapeKind::Path { path, .. } => tracedraw_core::nodes::nodes(path).len(),
        _ => 0,
    }
}

#[test]
fn the_pen_adds_a_node_on_a_segment_and_drags_a_node_of_the_selected_curve() {
    let (ctx, mut app, _) = setup();
    let id = line_curve(&ctx, &mut app);
    app.set_tool(Tool::Pen);
    use crate::ui::cursors::CurveMark;
    let on_segment = app.view.to_screen(Point::new(125.0, 100.0));
    let on_node = app.view.to_screen(Point::new(150.0, 100.0));
    assert_eq!(
        for_tool(&app, false, false, false, Some(on_segment)),
        ToolCursor::CurveBadge(Tool::Pen, CurveMark::AddNode)
    );
    assert_eq!(
        for_tool(&app, false, false, false, Some(on_node)),
        ToolCursor::CurveBadge(Tool::Pen, CurveMark::EditNode)
    );
    click_at(&ctx, &mut app, on_segment, Modifiers::NONE);
    assert_eq!(node_count(&app, id), 4);
    assert!(app.curve.is_none(), "no new curve was started");
    // Dragging the middle node moves it.
    let to = app.view.to_screen(Point::new(150.0, 130.0));
    drag(&ctx, &mut app, on_node, to, Modifiers::NONE, Vec::new());
    assert!(!app.pen_editing);
    let path = match &app.doc().find_shape(id).expect("curve").kind {
        ShapeKind::Path { path, .. } => path.clone(),
        _ => unreachable!(),
    };
    assert!(
        tracedraw_core::nodes::nodes(&path)
            .iter()
            .any(|n| near(n.pos, Point::new(150.0, 130.0))),
        "{path:?}"
    );
}

#[test]
fn nodes_are_taken_within_the_pick_distance_and_a_near_miss_keeps_the_selection() {
    let (ctx, mut app, _) = setup();
    let id = line_curve(&ctx, &mut app);
    app.set_tool(Tool::Shape);
    // A press 4 screen pixels off the middle node takes it.
    let node = app.view.to_screen(Point::new(150.0, 100.0));
    let from = node + egui::vec2(3.0, 2.5);
    let to = app.view.to_screen(Point::new(150.0, 120.0));
    drag(&ctx, &mut app, from, to, Modifiers::NONE, Vec::new());
    let moved = match &app.doc().find_shape(id).expect("curve").kind {
        ShapeKind::Path { path, .. } => tracedraw_core::nodes::nodes(path)
            .iter()
            .any(|n| (n.pos.y - 100.0).abs() > 5.0),
        _ => false,
    };
    assert!(moved, "the node moved");
    let _ = app.undo();
    // A click 10 pixels from the end node and the segment: a near miss,
    // the curve stays selected.
    let end = app.view.to_screen(Point::new(200.0, 100.0));
    let miss = end + egui::vec2(10.0, 0.0);
    click_at(&ctx, &mut app, miss, Modifiers::NONE);
    assert_eq!(app.selection, vec![id]);
    // Clearly on empty canvas: the selection goes.
    let far = end + egui::vec2(120.0, 120.0);
    click_at(&ctx, &mut app, far, Modifiers::NONE);
    assert!(app.selection.is_empty());
}

#[test]
fn the_shape_tool_says_a_rectangle_is_still_a_rectangle() {
    let (ctx, mut app, id) = setup();
    app.set_tool(Tool::Shape);
    frame(&ctx, &mut app, Vec::new(), Modifiers::NONE);
    assert_eq!(
        crate::ui::status::object_info(&app),
        crate::i18n::tr("status.shape_rectangle")
    );
    app.select(vec![id]);
    app.convert_to_curves();
    let info = crate::ui::status::object_info(&app);
    assert!(info.contains('4'), "{info}");
    assert_ne!(info, crate::i18n::tr("status.shape_rectangle"));
}

#[test]
fn rounded_corners_keep_their_size_when_relative_scaling_is_off() {
    let (ctx, mut app, id) = setup();
    for fixed in [true, false] {
        let mut c = tracedraw_core::Corners::uniform(5.0, tracedraw_core::CornerKind::Round);
        c.fixed = fixed;
        let _ = app.engine.run(&tracedraw_core::Command::SetShapeKind {
            shape: id,
            kind: ShapeKind::rect_with_corners(Rect::new(10.0, 10.0, 60.0, 40.0), c),
        });
        let _ = app.engine.run(&tracedraw_core::Command::TransformShapes {
            shapes: vec![id],
            transform: Affine::IDENTITY,
        });
        app.select(vec![id]);
        app.set_tool(Tool::Pick);
        frame(&ctx, &mut app, Vec::new(), Modifiers::NONE);
        // Twice as wide and high with the corner handle.
        let from = handle(&app, Handle::NE);
        let b = bounds(&app, id);
        let target = Point::new(b.x0 + 2.0 * b.width(), b.y0 + 2.0 * b.height());
        let to = app.view.to_screen(target);
        drag(&ctx, &mut app, from, to, Modifiers::NONE, Vec::new());
        let r = app
            .doc()
            .find_shape(id)
            .and_then(|s| s.page_corners())
            .expect("corners")
            .radii[0];
        if fixed {
            assert!(close(r, 5.0), "fixed: {r}");
        } else {
            assert!(r > 8.0, "relative: {r}");
        }
        let _ = app.undo();
    }
}
