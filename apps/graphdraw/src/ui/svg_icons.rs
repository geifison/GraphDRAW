//! The interface icons drawn from the monochrome SVG set in
//! `assets/icons` (white shapes, tinted with the theme's icon colour).
//!
//! [`paint`] is the one entry point every icon goes through: the toolbox,
//! the toolbars, the property bar, the menus, the context menus, the hints
//! and the dockers all call `icons::draw`, `icons::draw_action`,
//! `propbar::draw_pic` or `menus::draw_menu_picture`, which come here. When
//! [`svg_for`] names a drawing for the icon it is painted from the SVG,
//! otherwise the icon painted in code is drawn. So a command shows the
//! same picture wherever it appears. `docs/icon-map.md` lists the map.
//!
//! The SVGs are embedded, rasterised once per size at the screen's pixel
//! density and kept as textures in the egui context.

use crate::tools::Tool;
use crate::ui::icons::Action;
use crate::ui::menu_glyphs::Glyph;
use crate::ui::propbar::Pic;
use egui::{Color32, ColorImage, Painter, Pos2, Rect, TextureHandle, TextureId, Vec2};
use std::collections::HashMap;
use std::sync::Arc;

/// What an icon stands for: a tool, a standard toolbar action, a property
/// bar picture, a menu row picture, or a menu row by its key (rows that
/// have no painted picture of their own).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconRef<'a> {
    Tool(Tool),
    Action(Action),
    Pic(Pic),
    Glyph(Glyph),
    Key(&'a str),
}

macro_rules! embed {
    ($dir:literal; $($name:literal),* $(,)?) => {
        &[$(($name, include_str!(concat!("../../assets/icons/", $dir, $name, ".svg")))),*]
    };
}

/// The drawings on the 20 px grid, by file stem. Only the ones the map
/// below uses are embedded.
static FILES: &[(&str, &str)] = embed!["";
    "align-baseline", "align-bottom", "align-center-h", "align-center-v", "align-left",
    "align-right", "align-top", "bool-back-minus", "bool-boundary", "bool-front-minus",
    "bool-intersect", "bool-simplify", "bool-trim", "bool-weld", "break-apart", "cancel", "center-on",
    "clear-transform", "clip-center", "clip-edit", "clip-extract", "clip-fill", "clip-finish",
    "clip-fit", "clip-inside", "combine", "copy", "copy-fill", "copy-props", "copy-stroke", "corner-chamfer",
    "corner-round", "corner-scallop", "cur-move", "cur-pick-path", "cut", "delete", "dist-bottom", "dist-center-h",
    "dist-center-v", "dist-left", "dist-right", "dist-space-h", "dist-space-v", "dist-top",
    "doc-close", "duplicate", "ellipse-arc", "ellipse-pie", "ellipse-whole", "eye-off",
    "file-export", "file-import", "file-new", "file-open", "file-print", "file-save",
    "file-save-as", "fit-art", "fit-page", "fit-selection", "glyphs", "grid", "group", "instance",
    "layers", "lock-off", "lock-on", "mirror-h", "mirror-v", "move-here", "node-add", "node-break",
    "node-connect", "node-corner", "node-delete", "node-extract", "node-join", "node-reduce",
    "node-smooth", "node-symmetric", "nudge", "order-back", "order-backward",
    "order-behind-of", "order-forward", "order-front", "order-in-front-of", "outline-pen",
    "outline-to-object", "page-add", "page-first", "page-jump", "page-landscape",
    "page-last", "page-next", "page-portrait", "page-prev",
    "page-rename", "paste", "path-close", "path-reverse", "poly-complex", "poly-star",
    "preflight", "redo", "repeat", "rotate-cw", "seg-curve", "seg-line", "select-all", "shape-basic",
    "show-all", "spell", "symmetry", "text-case", "text-center", "text-convert", "text-edit",
    "text-justify", "text-left", "text-path", "text-props", "text-right", "to-bitmap",
    "to-curves", "tool-artistic", "tool-bezier", "tool-blend", "tool-bspline",
    "tool-connector", "tool-contour", "tool-crop", "tool-curve3", "tool-dimension",
    "tool-ellipse", "tool-ellipse-3pt", "tool-envelope", "tool-eraser", "tool-extrude",
    "tool-eyedropper", "tool-free-transform", "tool-freehand", "tool-gradient", "tool-graph",
    "tool-knife", "tool-line2", "tool-mesh", "tool-pan", "tool-polygon", "tool-polyline",
    "tool-rect", "tool-rect-3pt", "tool-select", "tool-shadow", "tool-shape",
    "tool-smart-draw", "tool-smart-fill", "tool-spiral", "tool-text", "tool-virtual-delete",
    "tool-width", "tool-zoom", "tp-straighten", "undo", "ungroup", "ungroup-all",
    "unlock-all", "zoom-in", "zoom-out",
];

/// Versions drawn for the 16 px grid, used at that size.
static MICRO: &[(&str, &str)] = embed!["16/"; "eye-off", "lock-off", "lock-on"];

const TOOLS: &[(Tool, &str)] = &[
    (Tool::Pick, "tool-select"),
    (Tool::FreeTransform, "tool-free-transform"),
    (Tool::Shape, "tool-shape"),
    (Tool::VariableOutline, "tool-width"),
    (Tool::Crop, "tool-crop"),
    (Tool::Knife, "tool-knife"),
    (Tool::SegmentDelete, "tool-virtual-delete"),
    (Tool::Eraser, "tool-eraser"),
    (Tool::Zoom, "tool-zoom"),
    (Tool::Pan, "tool-pan"),
    (Tool::Freehand, "tool-freehand"),
    (Tool::TwoPointLine, "tool-line2"),
    (Tool::Bezier, "tool-bezier"),
    (Tool::BSpline, "tool-bspline"),
    (Tool::Polyline, "tool-polyline"),
    (Tool::ThreePointCurve, "tool-curve3"),
    (Tool::BrushStrokes, "tool-artistic"),
    (Tool::ShapeRecognition, "tool-smart-draw"),
    (Tool::Rectangle, "tool-rect"),
    (Tool::ThreePointRectangle, "tool-rect-3pt"),
    (Tool::Ellipse, "tool-ellipse"),
    (Tool::ThreePointEllipse, "tool-ellipse-3pt"),
    (Tool::Polygon, "tool-polygon"),
    (Tool::Star, "poly-star"),
    (Tool::Spiral, "tool-spiral"),
    (Tool::CommonShapes, "shape-basic"),
    (Tool::GraphPaper, "tool-graph"),
    (Tool::Text, "tool-text"),
    (Tool::HorizontalVerticalDimension, "tool-dimension"),
    (Tool::Connector, "tool-connector"),
    (Tool::DropShadow, "tool-shadow"),
    (Tool::Contour, "tool-contour"),
    (Tool::Blend, "tool-blend"),
    (Tool::Envelope, "tool-envelope"),
    (Tool::Extrude, "tool-extrude"),
    (Tool::ColorEyedropper, "tool-eyedropper"),
    (Tool::InteractiveFill, "tool-gradient"),
    (Tool::AreaFill, "tool-smart-fill"),
    (Tool::MeshFill, "tool-mesh"),
    (Tool::OutlinePen, "outline-pen"),
];

const ACTIONS: &[(Action, &str)] = &[
    (Action::New, "file-new"),
    (Action::Open, "file-open"),
    (Action::Save, "file-save"),
    (Action::Print, "file-print"),
    (Action::Cut, "cut"),
    (Action::Copy, "copy"),
    (Action::Paste, "paste"),
    (Action::Undo, "undo"),
    (Action::Redo, "redo"),
    (Action::Import, "file-import"),
    (Action::Export, "file-export"),
    (Action::ShowGrid, "grid"),
];

const PICS: &[(Pic, &str)] = &[
    (Pic::Portrait, "page-portrait"),
    (Pic::Landscape, "page-landscape"),
    (Pic::Nudge, "nudge"),
    (Pic::Rotate, "rotate-cw"),
    (Pic::Lock(true), "lock-on"),
    (Pic::Lock(false), "lock-off"),
    (Pic::MirrorH, "mirror-h"),
    (Pic::MirrorV, "mirror-v"),
    (Pic::Pen, "outline-pen"),
    (Pic::ZoomIn, "zoom-in"),
    (Pic::ZoomOut, "zoom-out"),
    (Pic::ZoomSelected, "fit-selection"),
    (Pic::ZoomAll, "fit-art"),
    (Pic::ZoomPage, "fit-page"),
    (Pic::Ellipse, "ellipse-whole"),
    (Pic::Pie, "ellipse-pie"),
    (Pic::Arc, "ellipse-arc"),
    (Pic::CornerRound, "corner-round"),
    (Pic::CornerScallop, "corner-scallop"),
    (Pic::CornerChamfer, "corner-chamfer"),
    (Pic::ToFront, "order-front"),
    (Pic::ToBack, "order-back"),
    (Pic::ToCurves, "to-curves"),
    (Pic::AddNode, "node-add"),
    (Pic::DeleteNode, "node-delete"),
    (Pic::JoinNodes, "node-join"),
    (Pic::BreakCurve, "node-break"),
    (Pic::ToLine, "seg-line"),
    (Pic::ToCurve, "seg-curve"),
    (Pic::NodeCusp, "node-corner"),
    (Pic::NodeSmooth, "node-smooth"),
    (Pic::NodeSymmetrical, "node-symmetric"),
    (Pic::ReverseDirection, "path-reverse"),
    (Pic::ExtendToClose, "node-connect"),
    (Pic::ExtractSubpath, "node-extract"),
    (Pic::CloseCurve, "path-close"),
    (Pic::ReduceNodes, "node-reduce"),
    (Pic::Star, "poly-star"),
    (Pic::ComplexStar, "poly-complex"),
    (Pic::Group, "group"),
    (Pic::Ungroup, "ungroup"),
    (Pic::UngroupAll, "ungroup-all"),
    (Pic::Combine, "combine"),
    (Pic::BreakApart, "break-apart"),
    (Pic::Weld, "bool-weld"),
    (Pic::Trim, "bool-trim"),
    (Pic::Intersect, "bool-intersect"),
    (Pic::Simplify, "bool-simplify"),
    (Pic::FrontMinusBack, "bool-front-minus"),
    (Pic::BackMinusFront, "bool-back-minus"),
    (Pic::Boundary, "bool-boundary"),
    (Pic::Position, "cur-move"),
    (Pic::TextLeft, "text-left"),
    (Pic::TextCenter, "text-center"),
    (Pic::TextRight, "text-right"),
    (Pic::TextJustify, "text-justify"),
    (Pic::TextProperties, "text-props"),
    (Pic::EditText, "text-edit"),
    (Pic::ConvertText, "text-convert"),
    (Pic::FitTextToPath, "text-path"),
    (Pic::StraightenText, "tp-straighten"),
    (Pic::AlignBaseline, "align-baseline"),
    (Pic::ChangeCase, "text-case"),
    (Pic::ConvertToBitmap, "to-bitmap"),
];

const GLYPHS: &[(Glyph, &str)] = &[
    (Glyph::SaveAs, "file-save-as"),
    (Glyph::CloseDoc, "doc-close"),
    (Glyph::Preflight, "preflight"),
    (Glyph::Delete, "delete"),
    (Glyph::Duplicate, "duplicate"),
    (Glyph::Clone, "instance"),
    (Glyph::SelectAll, "select-all"),
    (Glyph::CopyProperties, "copy-props"),
    (Glyph::Repeat, "repeat"),
    (Glyph::PageInsert, "page-add"),
    (Glyph::PageRename, "page-rename"),
    (Glyph::PageGoto, "page-jump"),
    (Glyph::AlignLeft, "align-left"),
    (Glyph::AlignRight, "align-right"),
    (Glyph::AlignTop, "align-top"),
    (Glyph::AlignBottom, "align-bottom"),
    // Align::CenterH lines the centres up on a horizontal line (it moves
    // objects along y): the drawing with the horizontal axis, and the
    // other way round for CenterV.
    (Glyph::AlignCenterH, "align-center-v"),
    (Glyph::AlignCenterV, "align-center-h"),
    (Glyph::CenterPage, "center-on"),
    (Glyph::DistLeft, "dist-left"),
    (Glyph::DistCenterH, "dist-center-h"),
    (Glyph::DistSpaceH, "dist-space-h"),
    (Glyph::DistRight, "dist-right"),
    (Glyph::DistTop, "dist-top"),
    (Glyph::DistCenterV, "dist-center-v"),
    (Glyph::DistSpaceV, "dist-space-v"),
    (Glyph::DistBottom, "dist-bottom"),
    (Glyph::Forward, "order-forward"),
    (Glyph::Backward, "order-backward"),
    (Glyph::InFront, "order-in-front-of"),
    (Glyph::Behind, "order-behind-of"),
    (Glyph::Hide, "eye-off"),
    (Glyph::ShowAll, "show-all"),
    (Glyph::Unlock, "lock-off"),
    (Glyph::UnlockAll, "unlock-all"),
    (Glyph::OutlineToObject, "outline-to-object"),
    (Glyph::ClearTransform, "clear-transform"),
    (Glyph::ObjectsDocker, "layers"),
    (Glyph::ClipInside, "clip-inside"),
    (Glyph::ClipExtract, "clip-extract"),
    (Glyph::ClipEdit, "clip-edit"),
    (Glyph::ClipFinish, "clip-finish"),
    (Glyph::ClipCenter, "clip-center"),
    (Glyph::ClipFill, "clip-fill"),
    (Glyph::ClipFit, "clip-fit"),
    (Glyph::Symmetry, "symmetry"),
    (Glyph::Blend, "tool-blend"),
    (Glyph::Contour, "tool-contour"),
    (Glyph::Extrude, "tool-extrude"),
];

/// Commands that have no painted picture of the shared kinds, by the
/// text key of their menu row or button.
const KEYS: &[(&str, &str)] = &[
    ("menu.text.glyphs", "glyphs"),
    ("menu.text.spell_check", "spell"),
    ("status.first_page", "page-first"),
    ("status.previous_page", "page-prev"),
    ("status.next_page", "page-next"),
    ("status.last_page", "page-last"),
    // The menu after dragging with the right button.
    ("drop.move_here", "move-here"),
    ("drop.copy_here", "duplicate"),
    ("drop.copy_fill_here", "copy-fill"),
    ("drop.copy_outline_here", "copy-stroke"),
    ("drop.copy_all_properties", "copy-props"),
    ("drop.place_inside", "clip-inside"),
    // Place Inside Frame waiting for the frame: the cursor's sideways arrow.
    ("status.click_frame", "cur-pick-path"),
    ("drop.cancel", "cancel"),
];

fn find<K: PartialEq>(table: &[(K, &'static str)], key: &K) -> Option<&'static str> {
    table.iter().find(|(k, _)| k == key).map(|(_, v)| *v)
}

/// The SVG file stem drawn for `icon`, when there is one.
pub fn svg_for(icon: IconRef<'_>) -> Option<&'static str> {
    match icon {
        IconRef::Tool(t) => find(TOOLS, &t),
        IconRef::Action(a) => find(ACTIONS, &a),
        IconRef::Pic(p) => find(PICS, &p),
        IconRef::Glyph(g) => find(GLYPHS, &g),
        IconRef::Key(k) => KEYS.iter().find(|(key, _)| *key == k).map(|(_, v)| *v),
    }
}

/// The embedded drawing of `name`: the 16 px version when `micro` and
/// there is one, else the 20 px one.
fn source(name: &str, micro: bool) -> Option<&'static str> {
    let pick =
        |t: &'static [(&str, &'static str)]| t.iter().find(|(n, _)| *n == name).map(|(_, s)| *s);
    if micro {
        if let Some(s) = pick(MICRO) {
            return Some(s);
        }
    }
    pick(FILES)
}

/// Draw `icon` into `r` in `color`: from its SVG when it has one, else
/// the picture painted in code.
pub fn paint(painter: &Painter, r: Rect, icon: IconRef<'_>, color: Color32) {
    if paint_svg(painter, r, icon, color) {
        return;
    }
    match icon {
        IconRef::Tool(t) => crate::ui::icons::draw_painted(painter, r, t, color),
        IconRef::Action(a) => crate::ui::icons::draw_action_painted(painter, r, a, color),
        IconRef::Pic(p) => crate::ui::propbar::draw_pic_painted(painter, r, p, color),
        IconRef::Glyph(g) => crate::ui::menu_glyphs::draw_painted(painter, r, g, color),
        IconRef::Key(_) => {}
    }
}

/// Whether `icon` is drawn from an SVG.
pub fn has_svg(icon: IconRef<'_>) -> bool {
    svg_for(icon).is_some()
}

/// The design grid an icon of width `w` is drawn on: 16, 20 or 24 px for
/// the usual sizes (so strokes land on whole pixels), else `w`.
fn grid_side(w: f32) -> f32 {
    if (13.5..17.5).contains(&w) {
        16.0
    } else if (17.5..22.5).contains(&w) {
        20.0
    } else if (22.5..26.0).contains(&w) {
        24.0
    } else {
        w.max(1.0)
    }
}

/// Paint `icon`'s SVG centred in `r`, tinted with `color`; false when it
/// has none or it cannot be drawn (the caller paints the fallback).
pub fn paint_svg(painter: &Painter, r: Rect, icon: IconRef<'_>, color: Color32) -> bool {
    let Some(name) = svg_for(icon) else {
        return false;
    };
    if !painter.clip_rect().intersects(r.expand(4.0)) {
        // Off screen: nothing to draw, and no reason to fall back.
        return true;
    }
    let side = grid_side(r.width().min(r.height()));
    let ppp = painter.ctx().pixels_per_point();
    let px = (side * ppp).round().clamp(1.0, 512.0) as u32;
    let Some(tex) = texture(painter.ctx(), name, px, side <= 16.0) else {
        return false;
    };
    // Whole physical pixels, so the raster maps one to one onto the screen.
    let half = px as f32 / 2.0;
    let min = Pos2::new(
        (r.center().x * ppp - half).round() / ppp,
        (r.center().y * ppp - half).round() / ppp,
    );
    let dst = Rect::from_min_size(min, Vec2::splat(px as f32 / ppp));
    let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
    painter.image(tex, dst, uv, color);
    true
}

/// Textures made so far, by file stem, pixel size and grid.
#[derive(Clone, Default)]
struct Cache(Arc<egui::mutex::Mutex<HashMap<(&'static str, u32, bool), Option<TextureHandle>>>>);

fn texture(ctx: &egui::Context, name: &'static str, px: u32, micro: bool) -> Option<TextureId> {
    let cache = ctx.data_mut(|d| {
        d.get_temp_mut_or_default::<Cache>(egui::Id::new("svg_icons_cache"))
            .clone()
    });
    let mut map = cache.0.lock();
    if let Some(t) = map.get(&(name, px, micro)) {
        return t.as_ref().map(|t| t.id());
    }
    let tex = source(name, micro)
        .and_then(|svg| rasterize(svg, px))
        .map(|rgba| {
            let side = px as usize;
            ctx.load_texture(
                format!("svg-icon-{name}-{px}"),
                ColorImage::from_rgba_premultiplied([side, side], &rgba),
                egui::TextureOptions::LINEAR,
            )
        });
    if tex.is_none() {
        log::warn!("icon {name}: the SVG could not be drawn");
    }
    let id = tex.as_ref().map(|t| t.id());
    map.insert((name, px, micro), tex);
    id
}

/// Rasterise `svg` into a `px` by `px` premultiplied RGBA buffer (white
/// shapes on transparent), or `None` when it does not parse.
pub fn rasterize(svg: &str, px: u32) -> Option<Vec<u8>> {
    let tree = resvg::usvg::Tree::from_str(svg, &resvg::usvg::Options::default()).ok()?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(px, px)?;
    let size = tree.size();
    let ts = resvg::tiny_skia::Transform::from_scale(
        px as f32 / size.width(),
        px as f32 / size.height(),
    );
    resvg::render(&tree, ts, &mut pixmap.as_mut());
    Some(pixmap.take())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all_names() -> Vec<&'static str> {
        let mut v: Vec<&'static str> = TOOLS.iter().map(|(_, n)| *n).collect();
        v.extend(ACTIONS.iter().map(|(_, n)| *n));
        v.extend(PICS.iter().map(|(_, n)| *n));
        v.extend(GLYPHS.iter().map(|(_, n)| *n));
        v.extend(KEYS.iter().map(|(_, n)| *n));
        v
    }

    fn no_duplicates<K: PartialEq + std::fmt::Debug>(table: &[(K, &str)]) {
        for (i, (k, _)) in table.iter().enumerate() {
            assert!(
                !table[..i].iter().any(|(o, _)| o == k),
                "{k:?} is mapped twice"
            );
        }
    }

    #[test]
    fn the_map_has_no_duplicate_keys() {
        no_duplicates(TOOLS);
        no_duplicates(ACTIONS);
        no_duplicates(PICS);
        no_duplicates(GLYPHS);
        no_duplicates(KEYS);
        for t in [FILES, MICRO] {
            no_duplicates(t);
        }
    }

    #[test]
    fn every_mapped_file_is_embedded_and_every_embedded_file_is_mapped() {
        let names = all_names();
        for n in &names {
            assert!(source(n, false).is_some(), "{n}.svg is not embedded");
        }
        for (n, _) in FILES.iter().chain(MICRO) {
            assert!(names.contains(n), "{n}.svg is embedded but not used");
        }
    }

    #[test]
    fn every_embedded_svg_parses_and_draws_something() {
        for (n, svg) in FILES.iter().chain(MICRO) {
            assert!(
                resvg::usvg::Tree::from_str(svg, &resvg::usvg::Options::default()).is_ok(),
                "{n}.svg does not parse"
            );
            for px in [16, 20, 24, 40] {
                let rgba = rasterize(svg, px).unwrap_or_default();
                assert_eq!(rgba.len(), (px * px * 4) as usize, "{n} at {px}");
                let ink = rgba.chunks(4).filter(|p| p[3] > 0).count();
                assert!(ink > 0, "{n}.svg draws nothing at {px} px");
                // Monochrome: every pixel is white, only alpha varies.
                assert!(
                    rgba.chunks(4)
                        .all(|p| p[0] == p[3] && p[1] == p[3] && p[2] == p[3]),
                    "{n}.svg is not white on transparent"
                );
            }
        }
    }

    #[test]
    fn a_broken_svg_gives_no_raster_instead_of_a_panic() {
        assert!(rasterize("<svg", 20).is_none());
        assert!(rasterize("not an svg at all", 20).is_none());
        assert!(rasterize("", 16).is_none());
    }

    #[test]
    fn micro_versions_are_used_at_16_px_only() {
        let micro = source("lock-on", true);
        let normal = source("lock-on", false);
        assert!(micro.is_some() && normal.is_some());
        assert_ne!(micro, normal);
        // No 16 px drawing: the 20 px one at every size.
        assert_eq!(source("cut", true), source("cut", false));
    }

    #[test]
    fn sizes_snap_to_the_design_grids() {
        assert_eq!(grid_side(16.0), 16.0);
        assert_eq!(grid_side(15.0), 16.0);
        assert_eq!(grid_side(18.0), 20.0);
        assert_eq!(grid_side(22.0), 20.0);
        assert_eq!(grid_side(24.0), 24.0);
        assert_eq!(grid_side(48.0), 48.0);
    }

    #[test]
    fn a_command_has_one_icon_everywhere() {
        // The same drawing for a command reached as a tool and as a menu
        // row picture.
        assert_eq!(
            svg_for(IconRef::Tool(Tool::Blend)),
            svg_for(IconRef::Glyph(Glyph::Blend))
        );
        assert_eq!(
            svg_for(IconRef::Tool(Tool::Star)),
            svg_for(IconRef::Pic(Pic::Star))
        );
        assert_eq!(
            svg_for(IconRef::Tool(Tool::OutlinePen)),
            svg_for(IconRef::Pic(Pic::Pen))
        );
        // Menu rows resolve through the same lookup as the toolbar.
        assert_eq!(svg_for(IconRef::Action(Action::Cut)), Some("cut"));
        assert_eq!(svg_for(IconRef::Key("menu.text.glyphs")), Some("glyphs"));
        assert_eq!(svg_for(IconRef::Key("menu.no.such.row")), None);
    }

    #[test]
    fn every_key_in_the_map_is_a_real_text_key() {
        let en = include_str!("../../lang/en.toml");
        for (k, _) in KEYS {
            assert!(
                en.lines().any(|l| l.starts_with(&format!("{k} = "))),
                "{k} is not a text key"
            );
        }
        // The whole right-drag menu has icons.
        for a in crate::drop_menu::DropAction::ALL {
            assert!(svg_for(IconRef::Key(a.key())).is_some(), "{}", a.key());
        }
    }

    #[test]
    fn the_icon_map_document_lists_every_mapped_file() {
        let doc = include_str!("../../../../docs/icon-map.md");
        for n in all_names() {
            assert!(
                doc.contains(&format!("`{n}.svg`")),
                "{n}.svg missing from docs/icon-map.md"
            );
        }
    }

    #[test]
    fn painting_makes_one_texture_per_size_and_falls_back_without_an_svg() {
        let ctx = egui::Context::default();
        let mut shapes = 0;
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            let r = Rect::from_min_size(Pos2::new(10.0, 10.0), Vec2::splat(20.0));
            let p = ui.painter();
            paint(p, r, IconRef::Tool(Tool::Pick), Color32::WHITE);
            paint(p, r, IconRef::Tool(Tool::Pick), Color32::RED);
            paint(p, r, IconRef::Tool(Tool::Smear), Color32::WHITE);
            paint(p, r, IconRef::Key("menu.no.such.row"), Color32::WHITE);
            shapes = 1;
        });
        assert_eq!(shapes, 1);
        assert!(!out.textures_delta.set.is_empty(), "the icon was uploaded");
        out.textures_delta.clear();
        let cache = ctx.data_mut(|d| {
            d.get_temp_mut_or_default::<Cache>(egui::Id::new("svg_icons_cache"))
                .clone()
        });
        let map = cache.0.lock();
        assert_eq!(map.len(), 1, "one texture for the two Pick icons");
        assert!(map.values().all(|t| t.is_some()));
    }
}
