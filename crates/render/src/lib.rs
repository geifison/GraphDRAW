//! CPU rasterizer. Draws a page region into an RGBA pixmap with tiny-skia:
//! correct non-convex fills, fountain fills, outline widths, caps, joins,
//! dashes, hairlines and anti-aliasing. The GPU renderer, when it exists,
//! must match this one.

#![allow(clippy::field_reassign_with_default)]
mod image_cache;
mod pattern;

pub use pattern::texture_value_of;

use graphdraw_core::geometry::Shape as _;
use graphdraw_core::{
    document::{Shape, ShapeKind},
    geometry::{Affine, BezPath, PathEl, Point, Rect},
    Color, Document, Fill, LineCap as LCap, LineJoin as LJoin, PageId, ShapeId, Stroke,
};
use tiny_skia::{
    Color as SkColor, FillRule, GradientStop, LineCap, LineJoin, LinearGradient, Paint,
    Path as SkPath, PathBuilder, Pixmap, RadialGradient, SpreadMode, Stroke as SkStroke,
    StrokeDash, Transform,
};

/// Page (mm, Y up) to pixel mapping for one render.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewTransform {
    /// Pixels per millimetre.
    pub zoom: f64,
    /// Pixel position of the page origin (bottom-left corner) inside the target.
    pub origin_x: f64,
    pub origin_y: f64,
}

impl ViewTransform {
    pub fn affine(&self) -> Affine {
        Affine::new([
            self.zoom,
            0.0,
            0.0,
            -self.zoom,
            self.origin_x,
            self.origin_y,
        ])
    }
}

/// Objects to draw displaced by a transform (live drag preview).
#[derive(Debug, Clone, Default)]
pub struct Preview {
    pub shapes: Vec<ShapeId>,
    pub transform: Affine,
}

/// Runs bitmap effects over straight (not premultiplied) RGBA pixels of
/// `width` x `height`, in place. The renderer has no bitmap effects of its
/// own: the application that has them registers this for the Bitmap
/// Effects lens; without it the lens shows what is beneath unchanged.
pub type PixelEffects = fn(&[graphdraw_core::BitmapEffect], u32, u32, &mut Vec<u8>);

static PIXEL_EFFECTS: std::sync::OnceLock<PixelEffects> = std::sync::OnceLock::new();

/// Register the bitmap effects (once; later calls are ignored).
pub fn set_pixel_effects(f: PixelEffects) {
    let _ = PIXEL_EFFECTS.set(f);
}

pub struct RenderOptions {
    pub width: u32,
    pub height: u32,
    pub view: ViewTransform,
    pub preview: Option<Preview>,
    /// Draw outlines only (the Wireframe view).
    pub wireframe: bool,
    /// Show overprints (View > Simulate Overprints): a fill marked
    /// `overprint_fill` and an outline marked `overprint_outline` are
    /// multiplied over what lies beneath instead of knocking it out.
    pub simulate_overprints: bool,
    /// Rasterise lenses and non-uniform transparency ("Rasterize complex
    /// effects" in Enhanced view). When false
    /// those objects are drawn plainly, which is much faster.
    pub complex_effects: bool,
    /// Smooth edges (Convert to Bitmap's Anti-aliasing); off gives hard
    /// pixel edges.
    pub anti_alias: bool,
    /// On screen: large procedural fills (textures, conical and square
    /// gradients) may be worked out on a coarser grid. Exports keep every
    /// pixel.
    pub draft: bool,
    /// Focus mode: this object drawn as it is and everything else faded
    /// to the given visibility (0 to 1).
    pub focus: Option<(graphdraw_core::ShapeId, f32)>,
    /// On screen only: objects on the layers left out (the frame of a
    /// clip frame whose contents are being edited, shown as a hairline
    /// by the app instead). Exports leave it empty.
    pub hidden: Vec<graphdraw_core::ShapeId>,
    /// On screen only: the contents of a clip frame being edited, on the
    /// layers. Everything else fades to half under a white veil and these
    /// are drawn again over it as they are. Exports leave it empty.
    pub editing: Vec<graphdraw_core::ShapeId>,
}

impl Default for RenderOptions {
    fn default() -> Self {
        RenderOptions {
            width: 1,
            height: 1,
            view: ViewTransform {
                zoom: 1.0,
                origin_x: 0.0,
                origin_y: 0.0,
            },
            preview: None,
            wireframe: false,
            simulate_overprints: false,
            complex_effects: true,
            anti_alias: true,
            draft: false,
            focus: None,
            hidden: Vec::new(),
            editing: Vec::new(),
        }
    }
}

/// How much of the drawing around a clip frame's contents shows while
/// they are edited.
pub const CLIP_EDIT_VISIBILITY: f32 = 0.5;

/// Fade everything drawn so far to `visibility` (0 to 1): on the white
/// page and desk it reads as a white veil.
fn fade(pixmap: &mut Pixmap, visibility: f32) {
    let v = visibility.clamp(0.0, 1.0);
    for px in pixmap.pixels_mut() {
        let c = |x: u8| (x as f32 * v).round() as u8;
        if let Some(faded) = tiny_skia::PremultipliedColorU8::from_rgba(
            c(px.red()),
            c(px.green()),
            c(px.blue()),
            c(px.alpha()),
        ) {
            *px = faded;
        }
    }
}

/// Per-renderer behaviour flags, copied into every nested renderer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Flags {
    wireframe: bool,
    simulate_overprints: bool,
    complex_effects: bool,
    /// Fill open subpaths (the document's "Fill open curves").
    fill_open_curves: bool,
    anti_alias: bool,
    draft: bool,
}

impl Flags {
    fn from_options(opts: &RenderOptions) -> Self {
        Flags {
            wireframe: opts.wireframe,
            simulate_overprints: opts.simulate_overprints,
            complex_effects: opts.complex_effects,
            fill_open_curves: false,
            anti_alias: opts.anti_alias,
            draft: opts.draft,
        }
    }
}

impl Default for Flags {
    fn default() -> Self {
        Flags {
            wireframe: false,
            simulate_overprints: false,
            complex_effects: true,
            fill_open_curves: false,
            anti_alias: true,
            draft: false,
        }
    }
}

/// The subpaths of `path` that are closed: ended with a close command, or
/// ending where they start. Open curves are not filled unless the
/// document asks for it.
pub fn closed_subpaths(path: &BezPath) -> BezPath {
    let mut out = BezPath::new();
    let mut cur: Vec<PathEl> = Vec::new();
    let flush = |cur: &mut Vec<PathEl>, out: &mut BezPath| {
        if cur.is_empty() {
            return;
        }
        let start = match cur.first() {
            Some(PathEl::MoveTo(p)) => Some(*p),
            _ => None,
        };
        let end = match cur.last() {
            Some(PathEl::LineTo(p))
            | Some(PathEl::QuadTo(_, p))
            | Some(PathEl::CurveTo(_, _, p)) => Some(*p),
            _ => None,
        };
        let closed = cur.iter().any(|e| matches!(e, PathEl::ClosePath))
            || matches!((start, end), (Some(a), Some(b)) if (a - b).hypot() < 1e-6);
        if closed {
            for e in cur.drain(..) {
                out.push(e);
            }
        }
        cur.clear();
    };
    for el in path.elements() {
        if matches!(el, PathEl::MoveTo(_)) {
            flush(&mut cur, &mut out);
        }
        cur.push(*el);
    }
    flush(&mut cur, &mut out);
    out
}

/// Render the objects of one page onto a transparent pixmap.
pub fn render_page(doc: &Document, page: PageId, opts: &RenderOptions) -> Option<Pixmap> {
    let mut pixmap = Pixmap::new(opts.width.max(1), opts.height.max(1))?;
    let page = doc.page(page).ok()?;
    let screen = opts.view.affine();
    let mut flags = Flags::from_options(opts);
    flags.fill_open_curves = doc.metadata.fill_open_curves;
    let mut r = Renderer {
        pixmap: &mut pixmap,
        screen,
        zoom: opts.view.zoom,
        flags,
        symbols: &doc.symbols,
        preview: opts.preview.as_ref(),
        fill_object: Affine::IDENTITY,
    };
    if let Some(bg) = &page.background {
        let mut bg_shape = Shape::new(
            graphdraw_core::ShapeId(0),
            ShapeKind::Rect {
                rect: page.rect(),
                radius: 0.0,
                corners: None,
            },
        );
        bg_shape.fill = bg.clone();
        bg_shape.stroke = None;
        r.draw_shape(&bg_shape, Affine::IDENTITY);
    }
    let layers = doc.layers_for_page(page.id).ok()?;
    for layer in layers {
        if !layer.visible {
            continue;
        }
        for shape in layer.shapes.iter().filter(|s| !opts.hidden.contains(&s.id)) {
            let extra = r.child_transform(shape, Affine::IDENTITY);
            r.draw_shape(shape, extra);
        }
    }
    // Editing a clip frame's contents: the rest fades, then the contents
    // are drawn again over it, in their stacking order.
    if !opts.editing.is_empty() {
        fade(r.pixmap, CLIP_EDIT_VISIBILITY);
        for layer in doc.layers_for_page(page.id).ok()? {
            if !layer.visible {
                continue;
            }
            for shape in layer.shapes.iter().filter(|s| opts.editing.contains(&s.id)) {
                let extra = r.child_transform(shape, Affine::IDENTITY);
                r.draw_shape(shape, extra);
            }
        }
    }
    // Focus mode: everything fades, then the object in focus is drawn
    // again over it as it is.
    if let Some((id, visibility)) = opts.focus {
        if let (Some(shape), Some(parent)) = (doc.find_shape(id), doc.parent_transform(id)) {
            fade(r.pixmap, visibility);
            let extra = r.child_transform(shape, parent);
            r.draw_shape(shape, extra);
        }
    }
    Some(pixmap)
}

struct Renderer<'a> {
    pixmap: &'a mut Pixmap,
    screen: Affine,
    zoom: f64,
    flags: Flags,
    symbols: &'a [graphdraw_core::Symbol],
    /// Objects drawn displaced (the live drag preview), on a layer or
    /// inside a group or a clip frame.
    preview: Option<&'a Preview>,
    /// The transform of the object whose fill or outline is being drawn
    /// (patterns that follow their object use it).
    fill_object: Affine,
}

impl Renderer<'_> {
    /// A renderer with the same settings drawing into another pixmap.
    fn sub<'b>(&self, pixmap: &'b mut Pixmap) -> Renderer<'b>
    where
        Self: 'b,
    {
        Renderer {
            pixmap,
            screen: self.screen,
            zoom: self.zoom,
            flags: self.flags,
            symbols: self.symbols,
            preview: self.preview,
            fill_object: self.fill_object,
        }
    }

    /// The transform `child` is drawn with inside `transform`: the live
    /// drag preview's first when the child is being dragged.
    fn child_transform(&self, child: &Shape, transform: Affine) -> Affine {
        match self.preview {
            Some(p) if p.shapes.contains(&child.id) => p.transform * transform,
            _ => transform,
        }
    }

    /// Blend mode for a fill or outline: multiply when it overprints and
    /// overprint simulation is on, otherwise ordinary source-over.
    fn blend_for(&self, overprint: bool) -> tiny_skia::BlendMode {
        if overprint && self.flags.simulate_overprints {
            tiny_skia::BlendMode::Multiply
        } else {
            tiny_skia::BlendMode::SourceOver
        }
    }

    fn draw_shape(&mut self, shape: &Shape, parent: Affine) {
        // Bitmap effects on a vector object: its picture in its place.
        if let Some(pic) = shape.drawn_as().filter(|_| !self.flags.wireframe) {
            return self.draw_shape(&pic, parent);
        }
        // Construction lines show on screen only, not in exports or print.
        if !shape.visible || (shape.construction && !self.flags.draft) {
            return;
        }
        // Character backgrounds go under the text, in its transform.
        if matches!(shape.kind, ShapeKind::Text { .. }) && !self.flags.wireframe {
            for (rect, fill) in graphdraw_core::document::text_backgrounds(&shape.kind) {
                let mut bg = Shape::new(
                    shape.id,
                    ShapeKind::Rect {
                        rect,
                        radius: 0.0,
                        corners: None,
                    },
                );
                bg.transform = shape.transform;
                bg.fill = fill;
                bg.stroke = None;
                bg.opacity = shape.opacity;
                self.draw_shape(&bg, parent);
            }
        }
        // Live effects: geometry warps, extra objects below and above, and
        // the pixel effects (lens, non-uniform transparency).
        if !shape.effects.is_empty() {
            let ev = graphdraw_core::live::evaluate(shape);
            for b in &ev.below {
                let mut b = b.clone();
                b.effects.clear();
                self.draw_shape(&b, parent);
            }
            let lens = shape.effects.iter().find_map(|e| match e {
                graphdraw_core::live::Effect::Lens(l) => Some(l.clone()),
                _ => None,
            });
            let transparency = shape.effects.iter().find_map(|e| match e {
                graphdraw_core::live::Effect::Transparency {
                    mask,
                    merge,
                    target,
                } => Some((mask.clone(), *merge, *target)),
                _ => None,
            });
            let mut main = ev.main.clone();
            main.effects.clear();
            if !self.flags.complex_effects || self.flags.wireframe {
                // Complex effects off: the object is drawn as it is, with
                // no pixel work for lenses or transparency masks.
                self.draw_shape(&main, parent);
            } else if let Some(l) = lens {
                self.draw_lens(&main, parent, &l);
            } else if let Some((mask, merge, target)) = transparency {
                self.draw_with_mask(&main, parent, &mask, merge, target);
            } else {
                self.draw_shape(&main, parent);
            }
            for a in &ev.above {
                let mut a = a.clone();
                a.effects.clear();
                self.draw_shape(&a, parent);
            }
            return;
        }
        let transform = parent * shape.transform;
        if let ShapeKind::Group { children } = &shape.kind {
            for c in children {
                let t = self.child_transform(c, transform);
                self.draw_shape(c, t);
            }
            return;
        }
        if matches!(
            shape.kind,
            ShapeKind::Table(_) | ShapeKind::SymbolInstance { .. }
        ) {
            for c in shape.expand(self.symbols) {
                self.draw_shape(&c, transform);
            }
            return;
        }
        if let (true, ShapeKind::ClipFrame { frame, contents }) =
            (self.flags.wireframe, &shape.kind)
        {
            // Wireframe: the frame and everything inside it, unclipped.
            self.draw_shape(frame, transform);
            for c in contents {
                let t = self.child_transform(c, transform);
                self.draw_shape(c, t);
            }
            return;
        }
        if let (ShapeKind::ClipFrame { frame, contents }, Some(om)) =
            (&shape.kind, shape.opacity_mask)
        {
            // Opacity mask: the frame drawn alone gives, by its brightness,
            // how much of the contents shows; the frame itself is not drawn.
            let (w, h) = (self.pixmap.width(), self.pixmap.height());
            if let (Some(mut frame_px), Some(mut layer)) = (Pixmap::new(w, h), Pixmap::new(w, h)) {
                {
                    let mut fill_only = (**frame).clone();
                    fill_only.stroke = None;
                    let mut sub = self.sub(&mut frame_px);
                    sub.draw_shape(&fill_only, transform);
                }
                if om.invert {
                    // Black shows: white where the frame is black, and
                    // nothing outside the frame.
                    for p in frame_px.pixels_mut() {
                        let c = p.demultiply();
                        let a = c.alpha();
                        let inv = |v: u8| 255 - v;
                        *p = tiny_skia::ColorU8::from_rgba(
                            inv(c.red()),
                            inv(c.green()),
                            inv(c.blue()),
                            a,
                        )
                        .premultiply();
                    }
                }
                let mask =
                    tiny_skia::Mask::from_pixmap(frame_px.as_ref(), tiny_skia::MaskType::Luminance);
                {
                    let mut sub = self.sub(&mut layer);
                    for c in contents {
                        let t = sub.child_transform(c, transform);
                        sub.draw_shape(c, t);
                    }
                }
                let pp = tiny_skia::PixmapPaint {
                    opacity: shape.opacity.clamp(0.0, 1.0) as f32,
                    ..Default::default()
                };
                self.pixmap.draw_pixmap(
                    0,
                    0,
                    layer.as_ref(),
                    &pp,
                    Transform::identity(),
                    Some(&mask),
                );
            }
            return;
        }
        if let ShapeKind::ClipFrame { frame, contents } = &shape.kind {
            // Frame's fill first, contents clipped to the frame, then the
            // frame's outline; an outline behind the fill goes first of all,
            // so the fill and the contents cover its inner half.
            let behind = frame.stroke.as_ref().is_some_and(|s| s.behind_fill);
            let mut outline_only = (**frame).clone();
            outline_only.fill = Fill::None;
            outline_only.opacity = shape.opacity;
            if let Some(st) = outline_only.stroke.as_mut() {
                st.behind_fill = false;
            }
            if behind {
                self.draw_shape(&outline_only, transform);
            }
            let mut frame_fill_only = (**frame).clone();
            frame_fill_only.stroke = None;
            frame_fill_only.opacity = shape.opacity;
            self.draw_shape(&frame_fill_only, transform);
            let frame_path = self.screen * (transform * frame.page_path());
            if let Some(sk_frame) = to_sk_path(&frame_path) {
                let (w, h) = (self.pixmap.width(), self.pixmap.height());
                if let (Some(mut layer), Some(mut mask)) =
                    (Pixmap::new(w, h), tiny_skia::Mask::new(w, h))
                {
                    // The frame clips by the rule its fill is drawn with:
                    // text glyphs (overlapping contours of synthetic bold
                    // and outlined fonts) by non-zero winding, so a glyph
                    // clips by its whole inside and not by its edges only.
                    mask.fill_path(&sk_frame, fill_rule_of(frame), true, Transform::identity());
                    {
                        let mut sub = self.sub(&mut layer);
                        for c in contents {
                            let t = sub.child_transform(c, transform);
                            sub.draw_shape(c, t);
                        }
                    }
                    let pp = tiny_skia::PixmapPaint {
                        opacity: shape.opacity.clamp(0.0, 1.0) as f32,
                        ..Default::default()
                    };
                    self.pixmap.draw_pixmap(
                        0,
                        0,
                        layer.as_ref(),
                        &pp,
                        Transform::identity(),
                        Some(&mask),
                    );
                }
            }
            if frame.stroke.is_some() && !behind {
                self.draw_shape(&outline_only, transform);
            }
            return;
        }
        // Further fills and outlines: the parts, bottom first.
        if let Some(parts) = shape.layers() {
            for p in &parts {
                self.draw_shape(p, parent);
            }
            return;
        }
        let local = shape.local_path();
        if local.elements().is_empty() {
            return;
        }
        let page_path = transform * local;
        let Some(sk) = to_sk_path(&(self.screen * page_path.clone())) else {
            return;
        };
        if self.flags.wireframe {
            // Wireframe: every object as its bare path, a hairline in its
            // outline's colour (black without one) whatever its width,
            // fill, opacity or effects; bitmaps as grey boxes.
            if matches!(shape.kind, ShapeKind::Bitmap { .. }) {
                let mut paint = Paint::default();
                paint.anti_alias = self.flags.anti_alias;
                paint.set_color_rgba8(208, 208, 208, 255);
                self.pixmap
                    .fill_path(&sk, &paint, FillRule::Winding, Transform::identity(), None);
            }
            let color = shape
                .stroke
                .as_ref()
                .map(|s| s.color)
                .unwrap_or(graphdraw_core::Color::BLACK);
            self.hairline(&sk, color);
            return;
        }
        let opacity = shape.opacity.clamp(0.0, 1.0) as f32;
        if opacity <= 0.0 {
            return;
        }

        // Drop shadow: blurred silhouette behind the object.
        if let Some(sh) = &shape.shadow {
            if !self.flags.wireframe && sh.opacity > 0.0 {
                let (w, h) = (self.pixmap.width(), self.pixmap.height());
                if let Some(mut layer) = Pixmap::new(w, h) {
                    let shifted = Affine::translate(sh.offset) * page_path.clone();
                    if let Some(sk_sh) = to_sk_path(&(self.screen * shifted)) {
                        let mut paint = Paint::default();
                        paint.anti_alias = self.flags.anti_alias;
                        paint.set_color(sk_color(sh.color));
                        layer.fill_path(
                            &sk_sh,
                            &paint,
                            FillRule::Winding,
                            Transform::identity(),
                            None,
                        );
                        let radius = (sh.blur * self.zoom).round() as i32;
                        if radius > 0 {
                            box_blur(&mut layer, radius.min(64));
                        }
                        let pp = tiny_skia::PixmapPaint {
                            opacity: sh.opacity.clamp(0.0, 1.0) as f32,
                            ..Default::default()
                        };
                        self.pixmap.draw_pixmap(
                            0,
                            0,
                            layer.as_ref(),
                            &pp,
                            Transform::identity(),
                            None,
                        );
                    }
                }
            }
        }

        // Semi-transparent objects render into a scratch layer first so fill
        // and outline do not double up where they overlap.
        if opacity < 1.0 {
            let (w, h) = (self.pixmap.width(), self.pixmap.height());
            if let Some(mut layer) = Pixmap::new(w, h) {
                let mut sub = self.sub(&mut layer);
                let mut opaque = shape.clone();
                opaque.opacity = 1.0;
                sub.draw_shape(&opaque, parent);
                let paint = tiny_skia::PixmapPaint {
                    opacity,
                    ..Default::default()
                };
                self.pixmap
                    .draw_pixmap(0, 0, layer.as_ref(), &paint, Transform::identity(), None);
            }
            return;
        }

        if let ShapeKind::Bitmap {
            rect,
            width_px,
            height_px,
            png,
            fx: _,
        } = &shape.kind
        {
            if !self.flags.wireframe {
                if let Some(img) = image_cache::decode(png) {
                    // Image pixel space (y down) to local rect (y up) to screen.
                    let sx = rect.width() / (*width_px).max(1) as f64;
                    let sy = rect.height() / (*height_px).max(1) as f64;
                    let img_to_local = Affine::new([sx, 0.0, 0.0, -sy, rect.x0, rect.y1]);
                    let m = (self.screen * transform * img_to_local).as_coeffs();
                    let t = Transform::from_row(
                        m[0] as f32,
                        m[1] as f32,
                        m[2] as f32,
                        m[3] as f32,
                        m[4] as f32,
                        m[5] as f32,
                    );
                    let mut paint = Paint::default();
                    paint.anti_alias = self.flags.anti_alias;
                    paint.blend_mode = self.blend_for(shape.overprint_fill);
                    paint.shader = tiny_skia::Pattern::new(
                        Pixmap::as_ref(&img),
                        SpreadMode::Pad,
                        tiny_skia::FilterQuality::Bilinear,
                        1.0,
                        t,
                    );
                    self.pixmap.fill_path(
                        &sk,
                        &paint,
                        FillRule::Winding,
                        Transform::identity(),
                        None,
                    );
                }
            }
            if let Some(stroke) = &shape.stroke {
                let (mut paint, sk_stroke) = self.stroke_paint(stroke, transform);
                paint.blend_mode = self.blend_for(shape.overprint_outline);
                let mut c = sk_color(stroke.color);
                c.apply_opacity(stroke.safe_opacity() as f32);
                paint.set_color(c);
                self.pixmap
                    .stroke_path(&sk, &paint, &sk_stroke, Transform::identity(), None);
            } else if self.flags.wireframe {
                let mut paint = Paint::default();
                paint.set_color_rgba8(0, 0, 0, 255);
                let s = SkStroke {
                    width: 0.0,
                    ..Default::default()
                };
                self.pixmap
                    .stroke_path(&sk, &paint, &s, Transform::identity(), None);
            }
            return;
        }

        // An outline set behind the fill: the outline first, then the
        // fill over it (half the outline's width shows outside).
        if shape.stroke.as_ref().is_some_and(|st| st.behind_fill)
            && !matches!(shape.fill, Fill::None)
        {
            let mut outline = shape.clone();
            outline.fill = Fill::None;
            if let Some(st) = outline.stroke.as_mut() {
                st.behind_fill = false;
            }
            let mut fill = shape.clone();
            fill.stroke = None;
            fill.shadow = None;
            outline.shadow = None;
            self.draw_shape(&outline, parent);
            self.draw_shape(&fill, parent);
            return;
        }

        self.fill_object = transform;
        // The fill's own transparency (NaN reads as opaque).
        let fill_alpha = if shape.fill_opacity.is_nan() {
            1.0
        } else {
            shape.fill_opacity.clamp(0.0, 1.0) as f32
        };
        if !self.flags.wireframe && fill_alpha > 0.0 {
            let rule = fill_rule_of(shape);
            let blend = self.blend_for(shape.overprint_fill);
            if let Fill::Mesh(m) = &shape.fill {
                // Mesh nodes are local: bring them to page space first.
                let mut pm = m.clone();
                for n in pm.nodes.iter_mut() {
                    n.pos = transform * n.pos;
                }
                if fill_alpha < 1.0 {
                    let (w, h) = (self.pixmap.width(), self.pixmap.height());
                    if let Some(mut layer) = Pixmap::new(w, h) {
                        {
                            let mut sub = self.sub(&mut layer);
                            sub.fill_mesh(&pm, &sk, rule, tiny_skia::BlendMode::SourceOver);
                        }
                        let pp = tiny_skia::PixmapPaint {
                            opacity: fill_alpha,
                            blend_mode: blend,
                            ..Default::default()
                        };
                        self.pixmap.draw_pixmap(
                            0,
                            0,
                            layer.as_ref(),
                            &pp,
                            Transform::identity(),
                            None,
                        );
                    }
                } else {
                    self.fill_mesh(&pm, &sk, rule, blend);
                }
            } else if !self.flags.fill_open_curves
                && matches!(shape.kind, ShapeKind::Path { closed: false, .. })
                && !matches!(shape.fill, Fill::None)
            {
                // Only the closed subpaths of a curve get the fill.
                let closed = closed_subpaths(&page_path);
                if let Some(sk_fill) = to_sk_path(&(self.screen * closed.clone())) {
                    self.draw_fill_alpha(&shape.fill, &closed, &sk_fill, rule, blend, fill_alpha);
                }
            } else {
                self.draw_fill_alpha(&shape.fill, &page_path, &sk, rule, blend, fill_alpha);
            }
        }
        if let Some(stroke) = &shape.stroke {
            // A see-through outline goes through a layer of its own, so
            // its arrowheads and the stroke do not double up where they
            // overlap.
            let alpha = stroke.safe_opacity() as f32;
            if alpha >= 1.0 {
                self.draw_outline(shape, stroke, transform, &page_path, &sk);
            } else if alpha > 0.0 {
                let (w, h) = (self.pixmap.width(), self.pixmap.height());
                if let Some(mut layer) = Pixmap::new(w, h) {
                    {
                        let mut sub = self.sub(&mut layer);
                        sub.draw_outline(shape, stroke, transform, &page_path, &sk);
                    }
                    let pp = tiny_skia::PixmapPaint {
                        opacity: alpha,
                        blend_mode: self.blend_for(shape.overprint_outline),
                        ..Default::default()
                    };
                    self.pixmap
                        .draw_pixmap(0, 0, layer.as_ref(), &pp, Transform::identity(), None);
                }
            }
        } else if self.flags.wireframe {
            let mut paint = Paint::default();
            paint.set_color_rgba8(0, 0, 0, 255);
            paint.anti_alias = self.flags.anti_alias;
            let s = SkStroke {
                width: 0.0,
                ..Default::default()
            };
            self.pixmap
                .stroke_path(&sk, &paint, &s, Transform::identity(), None);
        }
    }

    /// The outline of `shape` (page path `page_path`, the same on screen
    /// as `sk`), fully opaque: a painterly brush, a band (calligraphic,
    /// variable width, outside or inside, dashes through corners) or a
    /// plain stroke, then the arrowheads.
    fn draw_outline(
        &mut self,
        shape: &Shape,
        stroke: &Stroke,
        transform: Affine,
        page_path: &BezPath,
        sk: &SkPath,
    ) {
        let (mut paint, sk_stroke) = self.stroke_paint(stroke, transform);
        paint.blend_mode = self.blend_for(shape.overprint_outline);
        if let Some(brush) = stroke.painterly.as_ref().filter(|_| !self.flags.wireframe) {
            // Painterly brush: its marks, each with its own opacity.
            let c = transform.as_coeffs();
            let scale = if stroke.scale_with_object {
                ((c[0] * c[3] - c[1] * c[2]).abs()).sqrt().max(1e-6)
            } else {
                1.0
            };
            let [r, g, b] = stroke.color.to_rgb8();
            for mark in graphdraw_core::painterly::marks(page_path, stroke.width * scale, brush) {
                let Some(mp) = to_sk_path(&(self.screen * mark.path)) else {
                    continue;
                };
                paint.set_color_rgba8(r, g, b, (mark.alpha * 255.0).round() as u8);
                self.pixmap
                    .fill_path(&mp, &paint, FillRule::Winding, Transform::identity(), None);
            }
            return;
        }
        let object_scale = if stroke.scale_with_object {
            let c = transform.as_coeffs();
            ((c[0] * c[3] - c[1] * c[2]).abs()).sqrt().max(1e-6)
        } else {
            1.0
        };
        if let Some(band) = graphdraw_core::shaping::outline_band(page_path, stroke, object_scale) {
            // A band (calligraphic, variable, outside or inside, dashes
            // through corners): fill it instead
            // of stroking, with the outline's own fill when it has one.
            if let Some(bp) = to_sk_path(&(self.screen * band.clone())) {
                match outline_paint(stroke, transform) {
                    Some(fill) => {
                        use graphdraw_core::geometry::Shape as _;
                        let b = bbox(&band).to_path(0.01);
                        self.draw_fill(&fill, &b, &bp, FillRule::Winding);
                    }
                    None => self.pixmap.fill_path(
                        &bp,
                        &paint,
                        FillRule::Winding,
                        Transform::identity(),
                        None,
                    ),
                }
            }
        } else if let Some(fill) =
            outline_paint(stroke, transform).filter(|_| sk_stroke.width > 0.0)
        {
            // A filled outline: the stroke's area painted with the fill,
            // spread over the outline's bounds.
            if let Some(band) = sk.stroke(&sk_stroke, 1.0) {
                use graphdraw_core::geometry::Shape as _;
                let pad = (sk_stroke.width as f64 / self.zoom.max(1e-9)) / 2.0;
                let b = bbox(page_path).inflate(pad, pad);
                self.draw_fill(&fill, &b.to_path(0.01), &band, FillRule::Winding);
            }
        } else {
            self.pixmap
                .stroke_path(sk, &paint, &sk_stroke, Transform::identity(), None);
        }
        for head in graphdraw_core::style::arrowhead_paths(page_path, stroke) {
            if let Some(hp) = to_sk_path(&(self.screen * head)) {
                self.pixmap
                    .fill_path(&hp, &paint, FillRule::Winding, Transform::identity(), None);
            }
        }
    }

    /// A one pixel line along `sk` (the Wireframe view).
    fn hairline(&mut self, sk: &tiny_skia::Path, color: graphdraw_core::Color) {
        let mut paint = Paint::default();
        let [r, g, b] = color.to_rgb8();
        paint.set_color_rgba8(r, g, b, 255);
        paint.anti_alias = self.flags.anti_alias;
        let s = SkStroke {
            width: 0.0,
            ..Default::default()
        };
        self.pixmap
            .stroke_path(sk, &paint, &s, Transform::identity(), None);
    }

    /// Lens: transform the pixels already drawn beneath the shape's area.
    fn draw_lens(&mut self, shape: &Shape, parent: Affine, lens: &graphdraw_core::live::Lens) {
        use graphdraw_core::live::Lens;
        let transform = parent * shape.transform;
        let path = self.screen * (transform * shape.local_path());
        let Some(sk) = to_sk_path(&path) else {
            return;
        };
        let (w, h) = (self.pixmap.width(), self.pixmap.height());
        let Some(mut mask) = tiny_skia::Mask::new(w, h) else {
            return;
        };
        mask.fill_path(&sk, FillRule::EvenOdd, true, Transform::identity());
        let source = self.pixmap.clone();
        let b = path.bounding_box();
        let center = b.center();
        let (x0, y0) = (b.x0.max(0.0) as u32, b.y0.max(0.0) as u32);
        let (x1, y1) = ((b.x1.ceil() as u32).min(w), (b.y1.ceil() as u32).min(h));
        let mdata = mask.data();
        let src_px = |x: i64, y: i64| -> [f32; 3] {
            if x < 0 || y < 0 || x >= w as i64 || y >= h as i64 {
                return [1.0, 1.0, 1.0];
            }
            let p = source.pixel(x as u32, y as u32).map(|p| p.demultiply());
            match p {
                Some(p) if p.alpha() > 0 => {
                    let a = p.alpha() as f32 / 255.0;
                    // Composite over white (the page).
                    [
                        p.red() as f32 / 255.0 * a + (1.0 - a),
                        p.green() as f32 / 255.0 * a + (1.0 - a),
                        p.blue() as f32 / 255.0 * a + (1.0 - a),
                    ]
                }
                _ => [1.0, 1.0, 1.0],
            }
        };
        // Bitmap Effects lens: the effects run over what lies beneath (with
        // a margin, so blurs and the like see past the edge), then the
        // result shows inside the lens.
        let filtered: Option<(u32, u32, u32, Vec<u8>)> = match lens {
            Lens::Effects { effects } => PIXEL_EFFECTS.get().and_then(|run| {
                let m = 24;
                let (rx0, ry0) = (x0.saturating_sub(m), y0.saturating_sub(m));
                let (rx1, ry1) = ((x1 + m).min(w), (y1 + m).min(h));
                let (rw, rh) = (rx1.checked_sub(rx0)?, ry1.checked_sub(ry0)?);
                if rw == 0 || rh == 0 {
                    return None;
                }
                let mut buf = Vec::with_capacity((rw * rh * 4) as usize);
                for y in ry0..ry1 {
                    for x in rx0..rx1 {
                        let c = src_px(x as i64, y as i64);
                        buf.extend_from_slice(&[
                            (c[0] * 255.0).round() as u8,
                            (c[1] * 255.0).round() as u8,
                            (c[2] * 255.0).round() as u8,
                            255,
                        ]);
                    }
                }
                run(effects, rw, rh, &mut buf);
                (buf.len() == (rw * rh * 4) as usize).then_some((rx0, ry0, rw, buf))
            }),
            _ => None,
        };
        let pixels = self.pixmap.pixels_mut();
        for y in y0..y1 {
            for x in x0..x1 {
                let m = mdata[(y * w + x) as usize];
                if m == 0 {
                    continue;
                }
                let c = match lens {
                    Lens::Effects { .. } => match &filtered {
                        Some((rx0, ry0, rw, buf)) => {
                            let i = (((y - ry0) * rw + (x - rx0)) * 4) as usize;
                            match buf.get(i..i + 3) {
                                Some(p) => [
                                    p[0] as f32 / 255.0,
                                    p[1] as f32 / 255.0,
                                    p[2] as f32 / 255.0,
                                ],
                                None => src_px(x as i64, y as i64),
                            }
                        }
                        None => src_px(x as i64, y as i64),
                    },
                    Lens::FishEye { rate } => {
                        let r = (*rate / 1000.0).clamp(-0.95, 10.0);
                        let dx = x as f64 - center.x;
                        let dy = y as f64 - center.y;
                        let radius = (b.width().min(b.height()) / 2.0).max(1.0);
                        let d = (dx * dx + dy * dy).sqrt() / radius;
                        let k = if d > 0.0 { d.powf(1.0 + r) / d } else { 1.0 };
                        src_px(
                            (center.x + dx * k).round() as i64,
                            (center.y + dy * k).round() as i64,
                        )
                    }
                    Lens::Magnify { amount } => {
                        let a = amount.max(1.0);
                        let dx = (x as f64 - center.x) / a;
                        let dy = (y as f64 - center.y) / a;
                        src_px(
                            (center.x + dx).round() as i64,
                            (center.y + dy).round() as i64,
                        )
                    }
                    Lens::Wireframe { fill, .. } => {
                        let [r, g, bl] = fill.to_rgb8();
                        [r as f32 / 255.0, g as f32 / 255.0, bl as f32 / 255.0]
                    }
                    other => other.map_color(src_px(x as i64, y as i64)),
                };
                let mf = m as f32 / 255.0;
                let old = src_px(x as i64, y as i64);
                let out = [
                    old[0] + (c[0] - old[0]) * mf,
                    old[1] + (c[1] - old[1]) * mf,
                    old[2] + (c[2] - old[2]) * mf,
                ];
                if let Some(px) = tiny_skia::ColorU8::from_rgba(
                    (out[0].clamp(0.0, 1.0) * 255.0).round() as u8,
                    (out[1].clamp(0.0, 1.0) * 255.0).round() as u8,
                    (out[2].clamp(0.0, 1.0) * 255.0).round() as u8,
                    255,
                )
                .premultiply()
                .into()
                {
                    pixels[(y * w + x) as usize] = px;
                }
            }
        }
        // The lens object's own outline.
        if let Some(stroke) = &shape.stroke {
            let mut outline = shape.clone();
            outline.fill = Fill::None;
            outline.stroke = Some(stroke.clone());
            self.draw_shape(&outline, parent);
        }
    }

    /// Non-uniform transparency applied to the fill (`target` 0), the
    /// outline (1) or the whole object (2): the part it applies to goes
    /// through the mask and the merge mode, the other is drawn as it is,
    /// each in its usual order.
    fn draw_with_mask(
        &mut self,
        shape: &Shape,
        parent: Affine,
        mask_fill: &Fill,
        merge: graphdraw_core::live::MergeMode,
        target: u8,
    ) {
        if target >= 2 {
            self.draw_masked(shape, parent, mask_fill, merge);
            return;
        }
        let fill = shape_part(shape, true);
        let outline = shape_part(shape, false);
        let behind = shape.stroke.as_ref().is_some_and(|s| s.behind_fill);
        let order = if behind {
            [(&outline, target == 1), (&fill, target == 0)]
        } else {
            [(&fill, target == 0), (&outline, target == 1)]
        };
        for (part, masked) in order {
            if masked {
                self.draw_masked(part, parent, mask_fill, merge);
            } else {
                self.draw_shape(part, parent);
            }
        }
    }

    /// Render the shape to a layer, multiply its alpha by the luminance of
    /// the mask fill, composite with the merge mode.
    fn draw_masked(
        &mut self,
        shape: &Shape,
        parent: Affine,
        mask_fill: &Fill,
        merge: graphdraw_core::live::MergeMode,
    ) {
        let (w, h) = (self.pixmap.width(), self.pixmap.height());
        let (Some(mut layer), Some(mut mask_pm)) = (Pixmap::new(w, h), Pixmap::new(w, h)) else {
            return;
        };
        {
            let mut sub = self.sub(&mut layer);
            sub.draw_shape(shape, parent);
        }
        let transform = parent * shape.transform;
        let page_path = transform * shape.local_path();
        let bounds = page_path.bounding_box();
        let rect_path = graphdraw_core::geometry::rect_path(bounds, 0.0);
        // A stop's opacity scales its level: 0% hides the object there.
        let faded;
        let mask_fill = match mask_fill {
            Fill::Fountain(f) if f.stops.iter().any(|s| s.opacity < 1.0) => {
                let mut g = f.clone();
                for s in &mut g.stops {
                    let a = s.opacity.clamp(0.0, 1.0) as f32;
                    s.color =
                        graphdraw_core::style::lerp_color(graphdraw_core::Color::BLACK, s.color, a);
                    s.opacity = 1.0;
                }
                faded = Fill::Fountain(g);
                &faded
            }
            _ => mask_fill,
        };
        if let Some(sk) = to_sk_path(&(self.screen * rect_path.clone())) {
            let mut sub = self.sub(&mut mask_pm);
            sub.flags.wireframe = false;
            match mask_fill {
                // A shaped mask follows the object, over its whole bounds.
                Fill::Fountain(f) if f.kind == graphdraw_core::FountainKind::Shaped => {
                    if let Some(shape_sk) = to_sk_path(&(self.screen * page_path.clone())) {
                        sub.fill_shaped(f, &shape_sk, &sk, FillRule::EvenOdd);
                    }
                }
                _ => sub.draw_fill(mask_fill, &rect_path, &sk, FillRule::Winding),
            }
        }
        let mdata: Vec<u8> = mask_pm
            .pixels()
            .iter()
            .map(|p| {
                let d = p.demultiply();
                if d.alpha() == 0 {
                    255
                } else {
                    ((d.red() as u32 * 54 + d.green() as u32 * 183 + d.blue() as u32 * 19) / 256)
                        as u8
                }
            })
            .collect();
        for (px, m) in layer.pixels_mut().iter_mut().zip(mdata) {
            let d = px.demultiply();
            let a = (d.alpha() as u32 * m as u32 / 255) as u8;
            if let Some(np) = tiny_skia::ColorU8::from_rgba(d.red(), d.green(), d.blue(), a)
                .premultiply()
                .into()
            {
                *px = np;
            }
        }
        use graphdraw_core::live::MergeMode as M;
        use tiny_skia::BlendMode as B;
        // Normal and the modes that mix the channels together go through
        // tiny-skia; the others through the core's formula, pixel by pixel.
        let blend = match merge {
            M::Normal => B::SourceOver,
            M::Hue => B::Hue,
            M::Saturation => B::Saturation,
            M::Color => B::Color,
            _ => {
                composite_merge(self.pixmap, &layer, merge);
                return;
            }
        };
        let paint = tiny_skia::PixmapPaint {
            blend_mode: blend,
            ..Default::default()
        };
        self.pixmap
            .draw_pixmap(0, 0, layer.as_ref(), &paint, Transform::identity(), None);
    }

    /// Fill a screen-space path with any fill type.
    fn draw_fill(&mut self, fill: &Fill, page_path: &BezPath, sk: &SkPath, rule: FillRule) {
        self.draw_fill_blended(fill, page_path, sk, rule, tiny_skia::BlendMode::SourceOver);
    }

    /// `draw_fill` with a blend mode. Solid fills blend directly; the other
    /// fill types render into a scratch layer that is then composited with
    /// the mode, so every fill type can overprint.
    fn draw_fill_blended(
        &mut self,
        fill: &Fill,
        page_path: &BezPath,
        sk: &SkPath,
        rule: FillRule,
        blend: tiny_skia::BlendMode,
    ) {
        self.draw_fill_alpha(fill, page_path, sk, rule, blend, 1.0);
    }

    /// `draw_fill_blended` with the fill's own transparency `alpha`
    /// (0..1): a solid colour painted with that alpha, any other fill
    /// rendered into a scratch layer composited with it.
    fn draw_fill_alpha(
        &mut self,
        fill: &Fill,
        page_path: &BezPath,
        sk: &SkPath,
        rule: FillRule,
        blend: tiny_skia::BlendMode,
        alpha: f32,
    ) {
        if alpha <= 0.0 || matches!(fill, Fill::None) {
            return;
        }
        if alpha < 1.0 && !matches!(fill, Fill::Solid(_)) {
            let (w, h) = (self.pixmap.width(), self.pixmap.height());
            let Some(mut layer) = Pixmap::new(w, h) else {
                return;
            };
            {
                let mut sub = self.sub(&mut layer);
                sub.draw_fill(fill, page_path, sk, rule);
            }
            let pp = tiny_skia::PixmapPaint {
                opacity: alpha,
                blend_mode: blend,
                ..Default::default()
            };
            self.pixmap
                .draw_pixmap(0, 0, layer.as_ref(), &pp, Transform::identity(), None);
            return;
        }
        if blend != tiny_skia::BlendMode::SourceOver && !matches!(fill, Fill::Solid(_)) {
            let (w, h) = (self.pixmap.width(), self.pixmap.height());
            let Some(mut layer) = Pixmap::new(w, h) else {
                return;
            };
            {
                let mut sub = self.sub(&mut layer);
                sub.draw_fill(fill, page_path, sk, rule);
            }
            let pp = tiny_skia::PixmapPaint {
                blend_mode: blend,
                ..Default::default()
            };
            self.pixmap
                .draw_pixmap(0, 0, layer.as_ref(), &pp, Transform::identity(), None);
            return;
        }
        let mut paint = Paint::default();
        paint.anti_alias = self.flags.anti_alias;
        paint.blend_mode = blend;
        match fill {
            Fill::None => {}
            Fill::Solid(c) => {
                let mut color = sk_color(*c);
                color.apply_opacity(alpha.min(1.0));
                paint.set_color(color);
                self.pixmap
                    .fill_path(sk, &paint, rule, Transform::identity(), None);
            }
            Fill::Fountain(f) => {
                let b = bbox(page_path);
                // The edge pad, repeats, mirror, inversion, size, axis
                // offset, midpoints, steps and blending are worked into one
                // run of stops over the automatic span, which the shader
                // pads beyond; the stops keep their opacity.
                let stops: Vec<GradientStop> = f
                    .resolved_stops()
                    .iter()
                    .map(|s| GradientStop::new(s.pos as f32, sk_color_alpha(s.color, s.opacity)))
                    .collect();
                // Smoothing dithers per pixel: drawn procedurally.
                let shader = !f.smooth;
                match f.kind {
                    graphdraw_core::FountainKind::Shaped => {
                        self.fill_shaped(f, sk, sk, rule);
                    }
                    graphdraw_core::FountainKind::Linear if shader => {
                        let a = f.angle.to_radians();
                        let (cx, cy) = (b.center().x, b.center().y);
                        let half = (b.width() * a.cos().abs() + b.height() * a.sin().abs()) / 2.0;
                        let p0 = self.screen * Point::new(cx - half * a.cos(), cy - half * a.sin());
                        let p1 = self.screen * Point::new(cx + half * a.cos(), cy + half * a.sin());
                        if let Some(sh) = LinearGradient::new(
                            tiny_skia::Point::from_xy(p0.x as f32, p0.y as f32),
                            tiny_skia::Point::from_xy(p1.x as f32, p1.y as f32),
                            stops,
                            SpreadMode::Pad,
                            Transform::identity(),
                        ) {
                            paint.shader = sh;
                            self.pixmap
                                .fill_path(sk, &paint, rule, Transform::identity(), None);
                        }
                    }
                    graphdraw_core::FountainKind::Radial if shader => {
                        let c = Point::new(
                            b.center().x + f.offset.x * b.width() / 2.0,
                            b.center().y + f.offset.y * b.height() / 2.0,
                        );
                        let radius = (b.width().max(b.height()) / 2.0) * std::f64::consts::SQRT_2;
                        let sc = self.screen * c;
                        if let Some(sh) = RadialGradient::new(
                            tiny_skia::Point::from_xy(sc.x as f32, sc.y as f32),
                            0.0,
                            tiny_skia::Point::from_xy(sc.x as f32, sc.y as f32),
                            (radius * self.zoom) as f32,
                            stops,
                            SpreadMode::Pad,
                            Transform::identity(),
                        ) {
                            paint.shader = sh;
                            self.pixmap
                                .fill_path(sk, &paint, rule, Transform::identity(), None);
                        }
                    }
                    _ => {
                        // Conical, square and elliptical fountains (and
                        // smoothed ones) pixel by pixel, the parameter from
                        // `Fountain::raw_param_uv`.
                        let f2 = f.clone();
                        let base = f.sorted_stops();
                        let plain = !f.has_extras();
                        let (w, h) = (b.width(), b.height());
                        let key = format!("{f:?}");
                        self.fill_procedural_alpha(page_path, sk, rule, &key, move |u, v| {
                            let t = f2.raw_param_uv(u, v, w, h);
                            let (c, a) = if plain {
                                let s = f2.stop_param(t.min(1.0));
                                (graphdraw_core::style::sample_stops(&base, s), 1.0)
                            } else {
                                f2.color_alpha_at(t)
                            };
                            if f2.smooth {
                                (dither(c, u, v), a as f32)
                            } else {
                                (c, a as f32)
                            }
                        });
                    }
                }
            }
            Fill::Pattern(p) => self.fill_pattern(p, sk, rule),
            Fill::Mesh(m) => self.fill_mesh(m, sk, rule, tiny_skia::BlendMode::SourceOver),
            Fill::Texture(t) => {
                let b = bbox(page_path);
                let t2 = t.clone();
                let key = format!("{t:?}");
                // The end colours once (a CMYK colour's conversion is not
                // cheap), then blended per pixel as `lerp_color` does.
                let ca = t.color_a.to_rgb8();
                let cb = t.color_b.to_rgb8();
                let ppi = Some(t.safe_resolution());
                self.fill_procedural_at(page_path, sk, rule, &key, ppi, move |u, v| {
                    let x = u * b.width() / t2.scale.max(0.1);
                    let y = v * b.height() / t2.scale.max(0.1);
                    let n = texture_value_of(&t2, x, y) as f32;
                    let l = |x: u8, y: u8| {
                        (x as f32 + (y as f32 - x as f32) * n)
                            .round()
                            .clamp(0.0, 255.0) as u8
                    };
                    Color::rgb8(l(ca[0], cb[0]), l(ca[1], cb[1]), l(ca[2], cb[2]))
                });
            }
        }
    }

    /// Mesh fill (nodes in page space): each cell is split into small
    /// flat-coloured quads, clipped by the path. Colours mix from the
    /// cell's four nodes (bilinear), or with Smooth colour from the sixteen
    /// around it (Catmull-Rom across the grid), so they flow through the
    /// nodes without creases.
    fn fill_mesh(
        &mut self,
        m: &graphdraw_core::Mesh,
        sk: &SkPath,
        rule: FillRule,
        blend: tiny_skia::BlendMode,
    ) {
        let (w, h) = (self.pixmap.width(), self.pixmap.height());
        let (Some(mut layer), Some(mut mask)) = (Pixmap::new(w, h), tiny_skia::Mask::new(w, h))
        else {
            return;
        };
        mask.fill_path(sk, rule, true, Transform::identity());
        let cols = m.cols as usize + 1;
        let node = |r: usize, c: usize| m.nodes.get(r * cols + c).copied();
        let lerp = |a: graphdraw_core::MeshNode, b: graphdraw_core::MeshNode, t: f64| {
            graphdraw_core::MeshNode {
                pos: a.pos + (b.pos - a.pos) * t,
                color: graphdraw_core::style::lerp_color(a.color, b.color, t as f32),
                alpha: a.alpha + (b.alpha - a.alpha) * t as f32,
            }
        };
        // Smooth colour: each channel through the 4 x 4 nodes around the
        // cell with Catmull-Rom weights (rows and columns past the edge
        // repeat the edge).
        let rgb: Vec<[f32; 3]> = m.nodes.iter().map(|n| n.color.to_rgb_f32()).collect();
        let weights = |t: f64| {
            let (t2, t3) = (t * t, t * t * t);
            [
                (-t3 + 2.0 * t2 - t) / 2.0,
                (3.0 * t3 - 5.0 * t2 + 2.0) / 2.0,
                (-3.0 * t3 + 4.0 * t2 + t) / 2.0,
                (t3 - t2) / 2.0,
            ]
        };
        let smooth_at = |r: usize, c: usize, u: f64, v: f64| -> Option<([u8; 3], f32)> {
            let (wu, wv) = (weights(u), weights(v));
            let mut acc = [0.0f64; 3];
            let mut alpha = 0.0f64;
            for (j, wj) in wv.iter().enumerate() {
                let rr = (r + j).saturating_sub(1).min(m.rows as usize);
                for (i, wi) in wu.iter().enumerate() {
                    let cc = (c + i).saturating_sub(1).min(m.cols as usize);
                    let k = rr * cols + cc;
                    let (n, c3) = (m.nodes.get(k)?, rgb.get(k)?);
                    let w = wi * wj;
                    for ch in 0..3 {
                        acc[ch] += w * c3[ch] as f64;
                    }
                    alpha += w * n.alpha as f64;
                }
            }
            let byte = |x: f64| (x.clamp(0.0, 1.0) * 255.0).round() as u8;
            Some((
                [byte(acc[0]), byte(acc[1]), byte(acc[2])],
                alpha.clamp(0.0, 1.0) as f32,
            ))
        };
        let mut paint = Paint::default();
        paint.anti_alias = false;
        for r in 0..m.rows as usize {
            for c in 0..m.cols as usize {
                let (Some(n00), Some(n10), Some(n01), Some(n11)) = (
                    node(r, c),
                    node(r, c + 1),
                    node(r + 1, c),
                    node(r + 1, c + 1),
                ) else {
                    continue;
                };
                // Subdivisions from the cell's screen size.
                let p00 = self.screen * n00.pos;
                let p11 = self.screen * n11.pos;
                let size = (p11 - p00).hypot();
                let k = ((size / 6.0).ceil() as usize).clamp(2, 24);
                for i in 0..k {
                    for j in 0..k {
                        let (u0, u1) = (i as f64 / k as f64, (i + 1) as f64 / k as f64);
                        let (v0, v1) = (j as f64 / k as f64, (j + 1) as f64 / k as f64);
                        let at = |u: f64, v: f64| lerp(lerp(n00, n10, u), lerp(n01, n11, u), v);
                        let q = [at(u0, v0), at(u1, v0), at(u1, v1), at(u0, v1)];
                        let mid = at((u0 + u1) / 2.0, (v0 + v1) / 2.0);
                        let mut pb = tiny_skia::PathBuilder::new();
                        for (idx, n) in q.iter().enumerate() {
                            let sp = self.screen * n.pos;
                            if idx == 0 {
                                pb.move_to(sp.x as f32, sp.y as f32);
                            } else {
                                pb.line_to(sp.x as f32, sp.y as f32);
                            }
                        }
                        pb.close();
                        let Some(path) = pb.finish() else { continue };
                        let smooth = m
                            .smooth
                            .then(|| smooth_at(r, c, (u0 + u1) / 2.0, (v0 + v1) / 2.0))
                            .flatten();
                        let ([cr, cg, cb], a) = smooth.unwrap_or((mid.color.to_rgb8(), mid.alpha));
                        paint.set_color_rgba8(cr, cg, cb, (a.clamp(0.0, 1.0) * 255.0) as u8);
                        // Slightly overdraw to hide seams between quads.
                        layer.fill_path(
                            &path,
                            &paint,
                            FillRule::Winding,
                            Transform::identity(),
                            None,
                        );
                        let s = tiny_skia::Stroke {
                            width: 0.7,
                            ..Default::default()
                        };
                        layer.stroke_path(&path, &paint, &s, Transform::identity(), None);
                    }
                }
            }
        }
        let pp = tiny_skia::PixmapPaint {
            blend_mode: blend,
            ..Default::default()
        };
        self.pixmap.draw_pixmap(
            0,
            0,
            layer.as_ref(),
            &pp,
            Transform::identity(),
            Some(&mask),
        );
    }

    /// Fill with a per-pixel function over the path's bounding box
    /// (u, v in 0..1, v up). Only the part on the canvas is worked out;
    /// large areas are worked out on a coarser grid (the functions are
    /// smooth) and on several threads, and the last results are kept, so
    /// a drawing redrawn with the object in the same place (another object
    /// dragged, the selection changed) does not compute it again.
    /// Shaped fountain: the colour follows the distance from the edge of
    /// `mask` (screen space) towards its inside, 0 on the edge and 1 at
    /// the deepest point; `area` is painted with it. The distances are
    /// worked out on a grid of at most `SHAPED_GRID` pixels across the
    /// whole mask, so zooming and panning keep the colours in place.
    fn fill_shaped(
        &mut self,
        f: &graphdraw_core::Fountain,
        mask: &SkPath,
        area: &SkPath,
        rule: FillRule,
    ) {
        let b = mask.bounds();
        let (bw, bh) = (b.width(), b.height());
        if !(bw > 0.0 && bh > 0.0 && bw.is_finite() && bh.is_finite()) {
            return;
        }
        let scale = (SHAPED_GRID / bw.max(bh)).min(1.0);
        let gw = (bw * scale).ceil() as u32 + 2;
        let gh = (bh * scale).ceil() as u32 + 2;
        // Grid pixel (1, 1) is the mask's top-left corner.
        let to_grid = Transform::from_translate(1.0, 1.0)
            .pre_scale(scale, scale)
            .pre_translate(-b.left(), -b.top());
        let key = {
            use std::hash::{Hash, Hasher};
            let mut h = std::collections::hash_map::DefaultHasher::new();
            for p in mask.points() {
                (((p.x - b.left()) * scale * 64.0).round() as i64).hash(&mut h);
                (((p.y - b.top()) * scale * 64.0).round() as i64).hash(&mut h);
            }
            mask.verbs().len().hash(&mut h);
            matches!(rule, FillRule::EvenOdd).hash(&mut h);
            format!("shaped|{f:?}|{:x}|{gw}x{gh}", h.finish())
        };
        let pm = match procedural_cache_get(&key) {
            Some(pm) => pm,
            None => {
                let Some(mut m) = Pixmap::new(gw, gh) else {
                    return;
                };
                let mut paint = Paint::default();
                paint.set_color_rgba8(0, 0, 0, 255);
                paint.anti_alias = false;
                m.fill_path(mask, &paint, rule, to_grid, None);
                let inside: Vec<bool> = m.pixels().iter().map(|p| p.alpha() >= 128).collect();
                let dist = edge_distance(&inside, gw as usize, gh as usize);
                let deepest = dist.iter().cloned().fold(0.0f64, f64::max).max(1e-9);
                let base = f.sorted_stops();
                let Some(mut pm) = Pixmap::new(gw, gh) else {
                    return;
                };
                let extras = f.has_run_extras();
                for (px, d) in pm.pixels_mut().iter_mut().zip(&dist) {
                    let s = f.stop_param(d / deepest);
                    let (c, a) = if extras {
                        f.sample_run(s)
                    } else {
                        (graphdraw_core::style::sample_stops(&base, s), 1.0)
                    };
                    let [r, g, bb] = c.to_rgb8();
                    let a = (a.clamp(0.0, 1.0) * 255.0).round() as u8;
                    *px = tiny_skia::ColorU8::from_rgba(r, g, bb, a).premultiply();
                }
                procedural_cache_put(key, pm.clone());
                pm
            }
        };
        let Some(to_screen) = to_grid.invert() else {
            return;
        };
        let mut paint = Paint::default();
        paint.anti_alias = self.flags.anti_alias;
        paint.shader = tiny_skia::Pattern::new(
            pm.as_ref(),
            SpreadMode::Pad,
            tiny_skia::FilterQuality::Bilinear,
            1.0,
            to_screen,
        );
        self.pixmap
            .fill_path(area, &paint, rule, Transform::identity(), None);
    }

    /// [`Self::fill_procedural_full`] worked out at most `ppi` pixels per inch:
    /// zoomed in past it, each worked out pixel covers several on screen
    /// (the texture's own resolution, as an export shows it).
    fn fill_procedural_at(
        &mut self,
        page_path: &BezPath,
        sk: &SkPath,
        rule: FillRule,
        key: &str,
        ppi: Option<f64>,
        f: impl Fn(f64, f64) -> Color + Sync,
    ) {
        self.fill_procedural_full(page_path, sk, rule, key, ppi, move |u, v| (f(u, v), 1.0));
    }

    /// [`Self::fill_procedural_full`] with an opacity (0..1) for each point.
    fn fill_procedural_alpha(
        &mut self,
        page_path: &BezPath,
        sk: &SkPath,
        rule: FillRule,
        key: &str,
        f: impl Fn(f64, f64) -> (Color, f32) + Sync,
    ) {
        self.fill_procedural_full(page_path, sk, rule, key, None, f);
    }

    /// The procedural fill itself: a colour and an opacity for each point,
    /// worked out at most `ppi` pixels per inch when given.
    fn fill_procedural_full(
        &mut self,
        page_path: &BezPath,
        sk: &SkPath,
        rule: FillRule,
        key: &str,
        ppi: Option<f64>,
        f: impl Fn(f64, f64) -> (Color, f32) + Sync,
    ) {
        use graphdraw_core::geometry::Shape as _;
        let b = bbox(page_path);
        let sb = (self.screen * b.to_path(0.01)).bounding_box();
        if !(sb.width() > 0.0 && sb.height() > 0.0) {
            return;
        }
        // Zoomed in, the object may be far larger than the window; a pixel
        // of margin keeps the edges smooth.
        let vis = sb.intersect(graphdraw_core::Rect::new(
            -1.0,
            -1.0,
            self.pixmap.width() as f64 + 1.0,
            self.pixmap.height() as f64 + 1.0,
        ));
        if !(vis.width() > 0.0 && vis.height() > 0.0) {
            return;
        }
        let (x0, y0) = (vis.x0.floor(), vis.y0.floor());
        let full_w = ((vis.x1.ceil() - x0) as u32).clamp(1, 8192);
        let full_h = ((vis.y1.ceil() - y0) as u32).clamp(1, 8192);
        let step = if self.flags.draft {
            procedural_step(full_w, full_h)
        } else {
            1
        };
        // Screen pixels per texture pixel at the texture's resolution.
        let res_step = ppi
            .filter(|p| p.is_finite() && *p > 0.0)
            .map(|p| ((self.zoom * 25.4 / p).floor() as u32).clamp(1, 64))
            .unwrap_or(1);
        let step = step.max(res_step);
        // Several screen pixels per worked out pixel: the grid lines up
        // with the object, not the window, so a part of it shows the same
        // pixels however the view is scrolled.
        let (x0, y0, full_w, full_h) = if step > 1 {
            let st = step as f64;
            let ax = sb.x0 + ((x0 - sb.x0) / st).floor() * st;
            let ay = sb.y0 + ((y0 - sb.y0) / st).floor() * st;
            (
                ax,
                ay,
                full_w + (x0 - ax).ceil() as u32,
                full_h + (y0 - ay).ceil() as u32,
            )
        } else {
            (x0, y0, full_w, full_h)
        };
        let w = full_w.div_ceil(step).max(1);
        let h = full_h.div_ceil(step).max(1);
        let cache_key = format!(
            "{key}|{:.2},{:.2},{:.2},{:.2}|{x0},{y0},{w},{h},{step}",
            sb.x0, sb.y0, sb.x1, sb.y1
        );
        let pm = match procedural_cache_get(&cache_key) {
            Some(pm) => pm,
            None => {
                let Some(mut pm) = Pixmap::new(w, h) else {
                    return;
                };
                let (bw, bh) = (sb.width(), sb.height());
                let st = step as f64;
                let row = |y: u32, out: &mut [tiny_skia::PremultipliedColorU8]| {
                    for (x, px) in out.iter_mut().enumerate() {
                        let u = (x0 + (x as f64 + 0.5) * st - sb.x0) / bw;
                        let v = 1.0 - (y0 + (y as f64 + 0.5) * st - sb.y0) / bh;
                        let (c, a) = f(u, v);
                        let [r, g, bb] = c.to_rgb8();
                        let a = (a.clamp(0.0, 1.0) * 255.0).round() as u8;
                        *px = tiny_skia::ColorU8::from_rgba(r, g, bb, a).premultiply();
                    }
                };
                for_rows(pm.pixels_mut(), w as usize, &row);
                procedural_cache_put(cache_key, pm.clone());
                pm
            }
        };
        let mut paint = Paint::default();
        paint.anti_alias = self.flags.anti_alias;
        paint.shader = tiny_skia::Pattern::new(
            pm.as_ref(),
            SpreadMode::Pad,
            tiny_skia::FilterQuality::Bilinear,
            1.0,
            Transform::from_translate(x0 as f32, y0 as f32).pre_scale(step as f32, step as f32),
        );
        self.pixmap
            .fill_path(sk, &paint, rule, Transform::identity(), None);
    }

    /// Pattern fill: the tile (two colours, an image or vector objects)
    /// as pixels at the screen's density, arranged into a 2 by 2 block
    /// when rows or columns are offset or tiles mirrored, and repeated
    /// along the pattern space of its layout (on the page, or with the
    /// object's transform).
    fn fill_pattern(&mut self, p: &graphdraw_core::Pattern, sk: &SkPath, rule: FillRule) {
        let layout = *p.layout();
        let to_screen = self.screen * layout.pattern_to_page(self.fill_object);
        let density = pattern::length_scale(to_screen);
        if !(density.is_finite() && density > 1e-9) {
            return;
        }
        let size = p.tile_size();
        let max = if layout.needs_super_tile() {
            pattern::MAX_TILE_PX / 2
        } else {
            pattern::MAX_TILE_PX
        };
        let tw = pattern::tile_px(size.width, density, max);
        let th = pattern::tile_px(size.height, density, max);
        let (base, crisp) = match p {
            graphdraw_core::Pattern::TwoColor {
                tile, front, back, ..
            } => (
                pattern::two_color_tile(*tile, *front, *back, tw, th).map(std::sync::Arc::new),
                true,
            ),
            graphdraw_core::Pattern::Bitmap { png, .. } => {
                let Some(img) = image_cache::decode(png) else {
                    return;
                };
                // The image's own pixels; a very large one is scaled down.
                let img = if img.width() > max || img.height() > max {
                    pattern::fit(&img, max).map(std::sync::Arc::new)
                } else {
                    Some(img)
                };
                (img, false)
            }
            graphdraw_core::Pattern::Vector { shapes, .. } => (
                self.render_vector_tile(shapes, size, tw, th)
                    .map(std::sync::Arc::new),
                false,
            ),
        };
        let Some(base) = base else {
            return;
        };
        let (pm, cover) = if layout.needs_super_tile() {
            let Some(sup) = pattern::super_tile(&base, &layout) else {
                return;
            };
            (
                std::sync::Arc::new(sup),
                graphdraw_core::geometry::Size::new(size.width * 2.0, size.height * 2.0),
            )
        } else {
            (base, size)
        };
        let m = to_screen * pattern::pixel_to_pattern(pm.width(), pm.height(), cover);
        let Some(t) = pattern::sk_transform(m) else {
            return;
        };
        // Two-colour cells stay sharp while the grid is square to the
        // screen; turned, or for images and drawings, pixels blend.
        let quality = if crisp && pattern::axis_aligned(m) {
            tiny_skia::FilterQuality::Nearest
        } else {
            tiny_skia::FilterQuality::Bilinear
        };
        let mut paint = Paint::default();
        paint.anti_alias = self.flags.anti_alias;
        paint.shader =
            tiny_skia::Pattern::new(Pixmap::as_ref(&pm), SpreadMode::Repeat, quality, 1.0, t);
        self.pixmap
            .fill_path(sk, &paint, rule, Transform::identity(), None);
    }

    /// Rasterise a vector pattern tile (`tile` millimetres) into `tw` by
    /// `th` pixels. The tile's shapes go through the ordinary shape
    /// drawing, so nested groups, fills, outlines and text in the tile
    /// render exactly as on the page. The pixmap is a whole number of
    /// pixels, so the repeat is seamless; the tile's millimetre size is
    /// stretched to fit by at most half a pixel on each axis.
    fn render_vector_tile(
        &self,
        shapes: &[Shape],
        tile: graphdraw_core::geometry::Size,
        tw: u32,
        th: u32,
    ) -> Option<Pixmap> {
        if !(tile.width.is_finite() && tile.height.is_finite())
            || tile.width <= 0.0
            || tile.height <= 0.0
        {
            return None;
        }
        let mut pm = Pixmap::new(tw.max(1), th.max(1))?;
        // Tile space (mm, Y up, origin bottom-left) to tile pixels (Y down).
        let screen = Affine::new([
            tw as f64 / tile.width,
            0.0,
            0.0,
            -(th as f64) / tile.height,
            0.0,
            th as f64,
        ]);
        let mut sub = Renderer {
            pixmap: &mut pm,
            screen,
            zoom: tw as f64 / tile.width,
            flags: Flags {
                wireframe: false,
                ..self.flags
            },
            symbols: self.symbols,
            preview: None,
            fill_object: Affine::IDENTITY,
        };
        for s in shapes {
            sub.draw_shape(s, Affine::IDENTITY);
        }
        Some(pm)
    }

    fn stroke_paint(&self, stroke: &Stroke, transform: Affine) -> (Paint<'static>, SkStroke) {
        let mut paint = Paint::default();
        paint.anti_alias = self.flags.anti_alias;
        paint.set_color(sk_color(stroke.color));
        let hair = stroke.width <= Stroke::HAIRLINE + 1e-9;
        // "Scale with object" uses the object's own scale factor.
        let object_scale = if stroke.scale_with_object {
            let c = transform.as_coeffs();
            ((c[0] * c[3] - c[1] * c[2]).abs()).sqrt().max(1e-6)
        } else {
            1.0
        };
        let true_px = (stroke.width * object_scale * self.zoom) as f32;
        // An outline thinner than a pixel is drawn one pixel wide (a
        // hairline) rather than faded until it disappears, as previews of
        // such files show it.
        let width_px = if hair || true_px < 1.0 { 0.0 } else { true_px };
        let dash = if stroke.dash.is_empty() || hair {
            None
        } else {
            let unit = true_px.max(1.0);
            let arr: Vec<f32> = stroke
                .dash
                .iter()
                .map(|d| (*d as f32 * unit).max(0.01))
                .collect();
            let arr = if arr.len() % 2 == 1 {
                [arr.clone(), arr].concat()
            } else {
                arr
            };
            StrokeDash::new(arr, 0.0)
        };
        let sk = SkStroke {
            width: width_px,
            miter_limit: stroke.safe_miter_limit() as f32,
            line_cap: match stroke.cap {
                LCap::Butt => LineCap::Butt,
                LCap::Round => LineCap::Round,
                LCap::Square => LineCap::Square,
            },
            line_join: match stroke.join {
                LJoin::Miter => LineJoin::Miter,
                LJoin::Round => LineJoin::Round,
                LJoin::Bevel => LineJoin::Bevel,
            },
            dash,
        };
        (paint, sk)
    }
}

/// Three-pass box blur (approximates a Gaussian) on premultiplied RGBA.
pub fn box_blur(pm: &mut Pixmap, radius: i32) {
    let w = pm.width() as usize;
    let h = pm.height() as usize;
    let r = radius.max(1) as usize;
    let data = pm.data_mut();
    let mut tmp = vec![0u8; data.len()];
    for _ in 0..3 {
        blur_pass(data, &mut tmp, w, h, r, true);
        blur_pass(&tmp, data, w, h, r, false);
    }
}

fn blur_pass(src: &[u8], dst: &mut [u8], w: usize, h: usize, r: usize, horizontal: bool) {
    let (outer, inner) = if horizontal { (h, w) } else { (w, h) };
    let idx = |o: usize, i: usize| -> usize {
        if horizontal {
            (o * w + i) * 4
        } else {
            (i * w + o) * 4
        }
    };
    let span = (2 * r + 1) as u32;
    for o in 0..outer {
        let mut acc = [0u32; 4];
        // Prime the window with the first pixel repeated, like edge clamping.
        for i in 0..=r {
            let p = idx(o, i.min(inner - 1));
            for c in 0..4 {
                acc[c] += src[p + c] as u32;
            }
        }
        for _ in 0..r {
            let p = idx(o, 0);
            for c in 0..4 {
                acc[c] += src[p + c] as u32;
            }
        }
        for i in 0..inner {
            let d = idx(o, i);
            for c in 0..4 {
                dst[d + c] = (acc[c] / span) as u8;
            }
            let add = idx(o, (i + r + 1).min(inner - 1));
            let sub = idx(o, i.saturating_sub(r));
            for c in 0..4 {
                acc[c] = acc[c] + src[add + c] as u32 - src[sub + c] as u32;
            }
        }
    }
}

/// An outline's own paint when it has one other than a plain colour (a
/// gradient, pattern, texture or mesh), ready to draw in page space: a
/// mesh's nodes are local to the object and go through `transform`.
fn outline_paint(stroke: &Stroke, transform: Affine) -> Option<Fill> {
    match stroke.fill.as_deref()? {
        Fill::None | Fill::Solid(_) => None,
        Fill::Mesh(m) => {
            let mut m = m.clone();
            for n in m.nodes.iter_mut() {
                n.pos = transform * n.pos;
            }
            Some(Fill::Mesh(m))
        }
        other => Some(other.clone()),
    }
}

/// Largest side, in pixels, of the grid a shaped fountain measures its
/// distances on.
const SHAPED_GRID: f32 = 384.0;

/// For each pixel of a `w` by `h` grid, the distance in pixels from its
/// centre to the edge of the region `inside` marks (0 outside): exact
/// Euclidean distance to the nearest outside pixel, less the half pixel
/// to the edge between them.
fn edge_distance(inside: &[bool], w: usize, h: usize) -> Vec<f64> {
    const INF: f64 = 1e20;
    let mut g: Vec<f64> = inside.iter().map(|&i| if i { INF } else { 0.0 }).collect();
    if g.len() != w * h || w == 0 || h == 0 {
        return vec![0.0; inside.len()];
    }
    let n = w.max(h);
    let (mut f, mut d) = (vec![0.0; n], vec![0.0; n]);
    let (mut v, mut z) = (vec![0usize; n], vec![0.0; n + 1]);
    for x in 0..w {
        for y in 0..h {
            f[y] = g[y * w + x];
        }
        distance_1d(&f[..h], &mut d[..h], &mut v, &mut z);
        for y in 0..h {
            g[y * w + x] = d[y];
        }
    }
    for y in 0..h {
        f[..w].copy_from_slice(&g[y * w..(y + 1) * w]);
        distance_1d(&f[..w], &mut d[..w], &mut v, &mut z);
        g[y * w..(y + 1) * w].copy_from_slice(&d[..w]);
    }
    g.iter()
        .zip(inside)
        .map(|(s, &i)| if i { (s.sqrt() - 0.5).max(0.0) } else { 0.0 })
        .collect()
}

/// One pass of the squared distance transform: `d[q] = min over p of
/// (q - p)^2 + f[p]`, by the lower envelope of the parabolas.
fn distance_1d(f: &[f64], d: &mut [f64], v: &mut [usize], z: &mut [f64]) {
    let n = f.len();
    if n == 0 {
        return;
    }
    let sq = |i: usize| (i * i) as f64;
    let mut k = 0usize;
    v[0] = 0;
    z[0] = f64::NEG_INFINITY;
    z[1] = f64::INFINITY;
    for q in 1..n {
        loop {
            let p = v[k];
            let s = ((f[q] + sq(q)) - (f[p] + sq(p))) / (2.0 * (q as f64 - p as f64));
            // z[0] is minus infinity, so this stops at the first parabola.
            if k > 0 && s <= z[k] {
                k -= 1;
                continue;
            }
            k += 1;
            v[k] = q;
            z[k] = s;
            z[k + 1] = f64::INFINITY;
            break;
        }
    }
    let mut k = 0usize;
    for (q, out) in d.iter_mut().enumerate() {
        while z[k + 1] < q as f64 {
            k += 1;
        }
        let p = v[k];
        let dq = q as f64 - p as f64;
        *out = dq * dq + f[p];
    }
}

/// Cheap value noise in 0..1 for texture fills.
/// The grid of a procedural fill over `w` by `h` pixels: every pixel up
/// to about a third of a megapixel, then every second or third.
fn procedural_step(w: u32, h: u32) -> u32 {
    match w as u64 * h as u64 {
        0..=350_000 => 1,
        350_001..=1_400_000 => 2,
        _ => 3,
    }
}

/// Fill the rows of `px` (`w` pixels wide) with `row`, on several threads
/// where there are some.
fn for_rows(
    px: &mut [tiny_skia::PremultipliedColorU8],
    w: usize,
    row: &(impl Fn(u32, &mut [tiny_skia::PremultipliedColorU8]) + Sync),
) {
    if w == 0 {
        return;
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let threads = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1)
            .min(8);
        let rows = px.len() / w;
        if threads > 1 && rows >= 64 {
            let per = rows.div_ceil(threads);
            std::thread::scope(|scope| {
                for (k, chunk) in px.chunks_mut(per * w).enumerate() {
                    scope.spawn(move || {
                        for (i, line) in chunk.chunks_mut(w).enumerate() {
                            row((k * per + i) as u32, line);
                        }
                    });
                }
            });
            return;
        }
    }
    for (y, line) in px.chunks_mut(w).enumerate() {
        row(y as u32, line);
    }
}

/// The last procedural fills worked out, newest last: up to this many,
/// and up to this many pixels together.
const PROCEDURAL_CACHE: usize = 12;
const PROCEDURAL_CACHE_PIXELS: usize = 12_000_000;

thread_local! {
    static PROCEDURAL: std::cell::RefCell<Vec<(String, Pixmap)>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

fn procedural_cache_get(key: &str) -> Option<Pixmap> {
    PROCEDURAL.with(|c| {
        let mut c = c.borrow_mut();
        let i = c.iter().position(|(k, _)| k == key)?;
        let hit = c.remove(i);
        let pm = hit.1.clone();
        c.push(hit);
        Some(pm)
    })
}

fn procedural_cache_put(key: String, pm: Pixmap) {
    PROCEDURAL.with(|c| {
        let mut c = c.borrow_mut();
        c.push((key, pm));
        let mut total: usize = c
            .iter()
            .map(|(_, p)| p.width() as usize * p.height() as usize)
            .sum();
        while c.len() > PROCEDURAL_CACHE || (total > PROCEDURAL_CACHE_PIXELS && c.len() > 1) {
            let (_, old) = c.remove(0);
            total -= old.width() as usize * old.height() as usize;
        }
    })
}

/// The value (0..1) of a texture of `kind` with the settings textures
/// had before detail and contrast could be set (see [`texture_value_of`]).
pub fn texture_value(kind: graphdraw_core::TextureKind, x: f64, y: f64, seed: u32) -> f64 {
    let t = graphdraw_core::Texture {
        kind,
        seed,
        ..graphdraw_core::Texture::default()
    };
    texture_value_of(&t, x, y)
}

fn bbox(p: &BezPath) -> Rect {
    use graphdraw_core::geometry::Shape as _;
    p.bounding_box()
}

pub fn sk_color(c: Color) -> SkColor {
    let [r, g, b] = c.to_rgb8();
    SkColor::from_rgba8(r, g, b, 255)
}

/// [`sk_color`] with an opacity (0..1).
pub fn sk_color_alpha(c: Color, opacity: f64) -> SkColor {
    let [r, g, b] = c.to_rgb8();
    let a = if opacity.is_finite() {
        (opacity.clamp(0.0, 1.0) * 255.0).round() as u8
    } else {
        255
    };
    SkColor::from_rgba8(r, g, b, a)
}

/// A smoothed gradient's colour at `(u, v)`: up to a level of noise
/// either way (a hash of the point), so the quantised bands blend.
fn dither(c: Color, u: f64, v: f64) -> Color {
    let x = (u * 7919.0 + v * 104_729.0).to_bits();
    let mut h = x ^ (x >> 29);
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^= h >> 32;
    let n = ((h & 0xFFFF) as f32 / 65535.0 - 0.5) / 255.0 * 1.5;
    let [r, g, b] = c.to_rgb_f32();
    Color::Rgb {
        r: (r + n).clamp(0.0, 1.0),
        g: (g + n).clamp(0.0, 1.0),
        b: (b + n).clamp(0.0, 1.0),
    }
}

/// The rule `shape`'s fill is drawn with, which is also how it clips as
/// the frame of a clip frame: non-zero winding for text, even-odd for
/// everything else (holes of combined curves stay open).
pub fn fill_rule_of(shape: &Shape) -> FillRule {
    match &shape.kind {
        ShapeKind::Text { .. } => FillRule::Winding,
        _ => FillRule::EvenOdd,
    }
}

/// The object with only its fills (`fill`) or only its outlines, the
/// objects of a group included.
fn shape_part(shape: &Shape, fill: bool) -> Shape {
    let mut part = shape.clone();
    if fill {
        part.stroke = None;
        part.extra_strokes.clear();
    } else {
        part.fill = Fill::None;
        part.extra_fills.clear();
        if let Some(st) = part.stroke.as_mut() {
            st.behind_fill = false;
        }
        // A picture is its fill: the outline part keeps only its frame.
        if let ShapeKind::Bitmap { rect, .. } = &part.kind {
            part.kind = ShapeKind::Rect {
                rect: *rect,
                radius: 0.0,
                corners: None,
            };
        }
    }
    if let ShapeKind::Group { children } = &mut part.kind {
        for c in children.iter_mut() {
            *c = shape_part(c, fill);
        }
    }
    part
}

/// Composite `src` over `dst` with a separable merge mode: where both are
/// opaque the colour is the mode's, where only one is, that one's own
/// (`co = cs(1 - ab) + cb(1 - as) + as ab B(cb, cs)`, premultiplied).
fn composite_merge(dst: &mut Pixmap, src: &Pixmap, merge: graphdraw_core::live::MergeMode) {
    let unit = |v: u8| f32::from(v) / 255.0;
    for (d, s) in dst.pixels_mut().iter_mut().zip(src.pixels()) {
        let sa = unit(s.alpha());
        if sa <= 0.0 {
            continue;
        }
        let ds = s.demultiply();
        let cs = [unit(ds.red()), unit(ds.green()), unit(ds.blue())];
        let db = d.demultiply();
        let ba = unit(db.alpha());
        let cb = [unit(db.red()), unit(db.green()), unit(db.blue())];
        let mixed = merge.blend(cb, cs).unwrap_or(cs);
        let oa = sa + ba * (1.0 - sa);
        let ch = |i: usize| {
            let c = cs[i] * sa * (1.0 - ba) + cb[i] * ba * (1.0 - sa) + mixed[i] * sa * ba;
            (c.clamp(0.0, oa) * 255.0).round() as u8
        };
        let a = (oa.clamp(0.0, 1.0) * 255.0).round() as u8;
        if let Some(p) = tiny_skia::PremultipliedColorU8::from_rgba(ch(0), ch(1), ch(2), a) {
            *d = p;
        }
    }
}

pub fn to_sk_path(path: &BezPath) -> Option<SkPath> {
    let mut pb = PathBuilder::new();
    for el in path.elements() {
        match el {
            PathEl::MoveTo(p) => pb.move_to(p.x as f32, p.y as f32),
            PathEl::LineTo(p) => pb.line_to(p.x as f32, p.y as f32),
            PathEl::QuadTo(c, p) => pb.quad_to(c.x as f32, c.y as f32, p.x as f32, p.y as f32),
            PathEl::CurveTo(c1, c2, p) => pb.cubic_to(
                c1.x as f32,
                c1.y as f32,
                c2.x as f32,
                c2.y as f32,
                p.x as f32,
                p.y as f32,
            ),
            PathEl::ClosePath => pb.close(),
        }
    }
    pb.finish()
}

/// Rasterise a fill over `bounds` (page mm) at `dpi`, y up mapped to image
/// rows top-down. Used by exporters that have no native pattern or
/// texture support (PDF, SVG fallback).
pub fn render_fill_image(fill: &Fill, bounds: Rect, dpi: f64) -> Option<Pixmap> {
    render_fill_image_placed(fill, bounds, Affine::IDENTITY, dpi)
}

/// The resolution (pixels per inch) an exporter turns `fill` into an
/// image at over `bounds` (page mm): the texture's or the pattern's own
/// resolution (a bitmap pattern: its image's pixel density, at least
/// 150), 150 for other fills, lowered so the image stays within the
/// renderer's 8192 pixels a side.
pub fn export_fill_resolution(fill: &Fill, bounds: Rect) -> f64 {
    use graphdraw_core::TileLayout;
    let wanted = match fill {
        Fill::Texture(t) => t.safe_resolution(),
        Fill::Pattern(p @ graphdraw_core::Pattern::Bitmap { width_px, .. }) => {
            let mm = p.tile_size().width * p.layout().safe_scale();
            let own = *width_px as f64 / (mm / 25.4);
            if own.is_finite() {
                own.clamp(TileLayout::DEFAULT_RESOLUTION, 600.0)
            } else {
                TileLayout::DEFAULT_RESOLUTION
            }
        }
        Fill::Pattern(p) => p.layout().safe_resolution(),
        _ => TileLayout::DEFAULT_RESOLUTION,
    };
    let side = bounds.width().max(bounds.height());
    if side.is_finite() && side > 0.0 {
        wanted.min(8000.0 * 25.4 / side).max(1.0)
    } else {
        wanted
    }
}

/// [`render_fill_image`] for the fill of an object whose transform is
/// `object`: a pattern that follows its object is placed with it.
pub fn render_fill_image_placed(
    fill: &Fill,
    bounds: Rect,
    object: Affine,
    dpi: f64,
) -> Option<Pixmap> {
    let zoom = dpi / 25.4;
    let w = (bounds.width() * zoom).ceil().max(1.0) as u32;
    let h = (bounds.height() * zoom).ceil().max(1.0) as u32;
    if w > 8192 || h > 8192 {
        return None;
    }
    let mut pixmap = Pixmap::new(w, h)?;
    let view = ViewTransform {
        zoom,
        origin_x: -bounds.x0 * zoom,
        origin_y: bounds.y1 * zoom,
    };
    let screen = view.affine();
    let page_path = graphdraw_core::geometry::rect_path(bounds, 0.0);
    let sk = to_sk_path(&(screen * page_path.clone()))?;
    let mut r = Renderer {
        pixmap: &mut pixmap,
        screen,
        zoom,
        flags: Flags::default(),
        symbols: &[],
        preview: None,
        fill_object: Affine::IDENTITY,
    };
    r.fill_object = object;
    r.draw_fill(fill, &page_path, &sk, FillRule::Winding);
    Some(pixmap)
}

/// [`render_fill_image`] over the bounds of `path` (page space, mm) for
/// a fill that depends on the object's outline: a shaped fountain
/// measures its distances from the edge of `path` (even-odd) and covers
/// the whole rectangle, so an exporter can clip it with the path. Other
/// fills render as [`render_fill_image`] does.
pub fn render_fill_image_in(fill: &Fill, path: &BezPath, dpi: f64) -> Option<Pixmap> {
    let bounds = bbox(path);
    let Fill::Fountain(f) = fill else {
        return render_fill_image(fill, bounds, dpi);
    };
    if f.kind != graphdraw_core::FountainKind::Shaped {
        return render_fill_image(fill, bounds, dpi);
    }
    let zoom = dpi / 25.4;
    let w = (bounds.width() * zoom).ceil().max(1.0) as u32;
    let h = (bounds.height() * zoom).ceil().max(1.0) as u32;
    if w > 8192 || h > 8192 {
        return None;
    }
    let mut pixmap = Pixmap::new(w, h)?;
    let screen = ViewTransform {
        zoom,
        origin_x: -bounds.x0 * zoom,
        origin_y: bounds.y1 * zoom,
    }
    .affine();
    let rect = to_sk_path(&(screen * graphdraw_core::geometry::rect_path(bounds, 0.0)))?;
    let mask = to_sk_path(&(screen * path.clone()))?;
    let mut r = Renderer {
        pixmap: &mut pixmap,
        screen,
        zoom,
        flags: Flags::default(),
        symbols: &[],
        preview: None,
        fill_object: Affine::IDENTITY,
    };
    r.fill_shaped(f, &mask, &rect, FillRule::EvenOdd);
    Some(pixmap)
}

/// Render a whole page to an RGBA image at `dpi`, white background (exports, thumbnails, tests).
pub fn render_page_image(doc: &Document, page: PageId, dpi: f64) -> Option<Pixmap> {
    render_page_image_with(doc, page, dpi, &RenderOptions::default())
}

/// `render_page_image` with the behaviour flags of `opts` (`wireframe`,
/// `simulate_overprints`, `complex_effects`); its size, view and preview
/// are replaced by the page's own.
pub fn render_page_image_with(
    doc: &Document,
    page: PageId,
    dpi: f64,
    opts: &RenderOptions,
) -> Option<Pixmap> {
    let p = doc.page(page).ok()?;
    let zoom = dpi / 25.4;
    let w = (p.size.width * zoom).ceil() as u32;
    let h = (p.size.height * zoom).ceil() as u32;
    let view = ViewTransform {
        zoom,
        origin_x: 0.0,
        origin_y: h as f64,
    };
    let mut objects = render_page(
        doc,
        page,
        &RenderOptions {
            width: w,
            height: h,
            view,
            preview: None,
            wireframe: opts.wireframe,
            simulate_overprints: opts.simulate_overprints,
            complex_effects: opts.complex_effects,
            anti_alias: opts.anti_alias,
            draft: false,
            focus: None,
            hidden: Vec::new(),
            editing: Vec::new(),
        },
    )?;
    let mut out = Pixmap::new(w, h)?;
    out.fill(SkColor::WHITE);
    out.draw_pixmap(
        0,
        0,
        objects.as_ref(),
        &tiny_skia::PixmapPaint::default(),
        Transform::identity(),
        None,
    );
    let _ = &mut objects;
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use graphdraw_core::document::{Shape, ShapeKind};
    use graphdraw_core::{
        Fountain, FountainKind, Pattern, PatternTile, Stop, Texture, TextureKind,
    };

    fn px(pm: &Pixmap, x: u32, y: u32) -> (u8, u8, u8) {
        let p = pm.pixel(x, y).unwrap().demultiply();
        (p.red(), p.green(), p.blue())
    }

    fn doc_with(shapes: Vec<Shape>) -> (Document, PageId) {
        let mut doc = Document::new("t", graphdraw_core::geometry::Size::new(100.0, 100.0));
        let page = doc.pages[0].id;
        let layer = doc.pages[0].layers[0].id;
        for s in shapes {
            doc.layer_mut(layer).unwrap().shapes.push(s);
        }
        (doc, page)
    }

    fn rect(id: u64, r: Rect, fill: Color) -> Shape {
        let mut s = Shape::new(
            graphdraw_core::ShapeId(id),
            ShapeKind::Rect {
                rect: r,
                radius: 0.0,
                corners: None,
            },
        );
        s.fill = Fill::Solid(fill);
        s.stroke = None;
        s
    }

    /// A further fill paints over the fill, a further outline over the
    /// outline.
    #[test]
    fn further_fills_and_outlines_paint_over_the_first() {
        let mut r = rect(1, Rect::new(10.0, 10.0, 90.0, 90.0), Color::rgb8(255, 0, 0));
        r.extra_fills = vec![Fill::Solid(Color::rgb8(0, 0, 255))];
        r.stroke = Some(graphdraw_core::Stroke::new(Color::BLACK, 4.0));
        r.extra_strokes = vec![graphdraw_core::Stroke::new(Color::rgb8(0, 255, 0), 2.0)];
        let (doc, page) = doc_with(vec![r]);
        let opts = RenderOptions {
            width: 100,
            height: 100,
            view: ViewTransform {
                zoom: 1.0,
                origin_x: 0.0,
                origin_y: 100.0,
            },
            ..RenderOptions::default()
        };
        let pm = render_page(&doc, page, &opts).unwrap();
        assert_eq!(px(&pm, 50, 50), (0, 0, 255));
        // On the edge the thin green outline lies over the black one.
        assert_eq!(px(&pm, 50, 89), (0, 255, 0));
        // Just outside it, the black one shows.
        assert_eq!(px(&pm, 50, 91), (0, 0, 0));
    }

    /// A clip frame as an opacity mask: the contents show where the
    /// frame is light, not where it is dark, and nothing outside it; the
    /// frame is not drawn. Inverted, the other way round.
    /// Text clips by non-zero winding (overlapping glyph contours fill),
    /// other frames by even-odd (holes stay open).
    #[test]
    fn text_frames_clip_by_winding_and_curves_by_even_odd() {
        let text = Shape::new(
            graphdraw_core::ShapeId(1),
            ShapeKind::Text {
                spans: vec![graphdraw_core::TextSpan::new("A", "Sans", 12.0)],
                origin: graphdraw_core::geometry::Point::ZERO,
                frame: None,
                align: Default::default(),
                para: Default::default(),
                on_path: None,
            },
        );
        assert_eq!(fill_rule_of(&text), FillRule::Winding);
        let r = rect(2, Rect::new(0.0, 0.0, 10.0, 10.0), Color::BLACK);
        assert_eq!(fill_rule_of(&r), FillRule::EvenOdd);
    }

    /// Objects listed in `hidden` are left out of the drawing.
    #[test]
    fn hidden_objects_are_not_drawn() {
        let red = rect(1, Rect::new(0.0, 0.0, 100.0, 100.0), Color::rgb8(255, 0, 0));
        let (doc, page) = doc_with(vec![red]);
        let mut opts = RenderOptions {
            width: 10,
            height: 10,
            view: ViewTransform {
                zoom: 0.1,
                origin_x: 0.0,
                origin_y: 10.0,
            },
            ..RenderOptions::default()
        };
        let px = |opts: &RenderOptions| render_page(&doc, page, opts).unwrap().pixel(5, 5).unwrap();
        assert_eq!(px(&opts).red(), 255);
        opts.hidden = vec![graphdraw_core::ShapeId(1)];
        assert_eq!(px(&opts).alpha(), 0);
    }

    #[test]
    fn a_clip_frame_outline_behind_the_fill_shows_only_its_outer_half() {
        let red = Color::rgb8(255, 0, 0);
        let green = Color::rgb8(0, 200, 0);
        let contents = rect(1, Rect::new(0.0, 0.0, 100.0, 100.0), green);
        let mut frame = rect(2, Rect::new(20.0, 20.0, 80.0, 80.0), Color::BLACK);
        frame.stroke = Some(graphdraw_core::style::Stroke {
            width: 10.0,
            color: red,
            ..Default::default()
        });
        let clip = |frame: Shape| {
            Shape::new(
                graphdraw_core::ShapeId(3),
                ShapeKind::ClipFrame {
                    frame: Box::new(frame),
                    contents: vec![contents.clone()],
                },
            )
        };
        let opts = RenderOptions {
            width: 100,
            height: 100,
            view: ViewTransform {
                zoom: 1.0,
                origin_x: 0.0,
                origin_y: 100.0,
            },
            ..RenderOptions::default()
        };
        let render = |s: Shape| {
            let (doc, page) = doc_with(vec![s]);
            render_page(&doc, page, &opts).unwrap()
        };
        // In front: the inner half of the outline covers the contents.
        let pm = render(clip(frame.clone()));
        assert_eq!(px(&pm, 22, 50), (255, 0, 0));
        assert_eq!(px(&pm, 17, 50), (255, 0, 0));
        // Behind the fill: the contents cover the inner half.
        frame.stroke.as_mut().unwrap().behind_fill = true;
        let pm = render(clip(frame));
        assert_eq!(px(&pm, 22, 50), (0, 200, 0));
        assert_eq!(px(&pm, 17, 50), (255, 0, 0));
    }

    #[test]
    fn opacity_masks_show_the_contents_by_the_frame_brightness() {
        let red = Color::rgb8(255, 0, 0);
        let contents = rect(1, Rect::new(0.0, 0.0, 100.0, 100.0), red);
        // The frame: white on the left half, black on the right, covering
        // the middle 80 mm.
        let mut frame = rect(2, Rect::new(10.0, 10.0, 90.0, 90.0), Color::WHITE);
        frame.fill = Fill::Fountain(Fountain {
            kind: FountainKind::Linear,
            stops: vec![
                Stop::new(0.0, Color::WHITE),
                Stop::new(0.499, Color::WHITE),
                Stop::new(0.501, Color::BLACK),
                Stop::new(1.0, Color::BLACK),
            ],
            ..Fountain::two(FountainKind::Linear, Color::WHITE, Color::BLACK, 0.0)
        });
        let mut clip = Shape::new(
            graphdraw_core::ShapeId(3),
            ShapeKind::ClipFrame {
                frame: Box::new(frame),
                contents: vec![contents],
            },
        );
        clip.opacity_mask = Some(graphdraw_core::document::OpacityMask { invert: false });
        let opts = RenderOptions {
            width: 100,
            height: 100,
            view: ViewTransform {
                zoom: 1.0,
                origin_x: 0.0,
                origin_y: 100.0,
            },
            ..RenderOptions::default()
        };
        let render = |clip: &Shape| {
            let (doc, page) = doc_with(vec![clip.clone()]);
            render_page(&doc, page, &opts).unwrap()
        };
        let pm = render(&clip);
        // White part: the red shows fully; black part and outside: nothing.
        assert_eq!(pm.pixel(25, 50).unwrap().alpha(), 255);
        assert_eq!(px(&pm, 25, 50), (255, 0, 0));
        assert_eq!(pm.pixel(75, 50).unwrap().alpha(), 0);
        assert_eq!(pm.pixel(5, 50).unwrap().alpha(), 0);
        clip.opacity_mask = Some(graphdraw_core::document::OpacityMask { invert: true });
        let pm = render(&clip);
        assert_eq!(pm.pixel(25, 50).unwrap().alpha(), 0);
        assert_eq!(pm.pixel(75, 50).unwrap().alpha(), 255);
        assert_eq!(pm.pixel(5, 50).unwrap().alpha(), 0);
    }

    /// The drag preview moves an object inside a moved group by the
    /// page-space transform, and only that object.
    #[test]
    fn the_drag_preview_reaches_objects_inside_groups() {
        let red = Color::rgb8(255, 0, 0);
        let blue = Color::rgb8(0, 0, 255);
        let a = rect(1, Rect::new(0.0, 0.0, 10.0, 10.0), red);
        let b = rect(2, Rect::new(20.0, 0.0, 30.0, 10.0), blue);
        let mut g = Shape::new(
            graphdraw_core::ShapeId(3),
            ShapeKind::Group {
                children: vec![a, b],
            },
        );
        g.transform = Affine::translate((10.0, 10.0));
        let (doc, page) = doc_with(vec![g]);
        let mut opts = RenderOptions {
            width: 100,
            height: 100,
            view: ViewTransform {
                zoom: 1.0,
                origin_x: 0.0,
                origin_y: 100.0,
            },
            ..RenderOptions::default()
        };
        // Without a preview: red at page (15, 15), screen (15, 85).
        let pm = render_page(&doc, page, &opts).unwrap();
        assert_eq!(px(&pm, 15, 85), (255, 0, 0));
        opts.preview = Some(Preview {
            shapes: vec![graphdraw_core::ShapeId(1)],
            transform: Affine::translate((0.0, 40.0)),
        });
        let pm = render_page(&doc, page, &opts).unwrap();
        // The red square moved up by 40 mm; the blue one stayed.
        assert_eq!(pm.pixel(15, 85).unwrap().alpha(), 0);
        assert_eq!(px(&pm, 15, 45), (255, 0, 0));
        assert_eq!(px(&pm, 35, 85), (0, 0, 255));
    }

    #[test]
    fn open_curves_are_not_filled_unless_the_document_says_so() {
        use graphdraw_core::geometry::Point as P;
        // A "U" (open) and a triangle (closed) in one curve.
        let mut path = BezPath::new();
        path.move_to(P::new(10.0, 90.0));
        path.line_to(P::new(10.0, 10.0));
        path.line_to(P::new(40.0, 10.0));
        path.line_to(P::new(40.0, 90.0));
        path.move_to(P::new(60.0, 10.0));
        path.line_to(P::new(90.0, 10.0));
        path.line_to(P::new(75.0, 60.0));
        path.close_path();
        let closed = closed_subpaths(&path);
        assert_eq!(closed.elements().len(), 4);
        let mut s = Shape::new(
            graphdraw_core::ShapeId(1),
            ShapeKind::Path {
                path,
                closed: false,
            },
        );
        s.fill = Fill::Solid(Color::rgb8(255, 0, 0));
        s.stroke = None;
        let (mut doc, page) = doc_with(vec![s]);
        let opts = RenderOptions {
            width: 100,
            height: 100,
            view: ViewTransform {
                zoom: 1.0,
                origin_x: 0.0,
                origin_y: 100.0,
            },
            ..RenderOptions::default()
        };
        let pm = render_page(&doc, page, &opts).unwrap();
        // Inside the U (page 25, 50 -> screen 25, 50): nothing.
        assert_eq!(pm.pixel(25, 50).unwrap().alpha(), 0);
        // Inside the triangle (page 75, 20 -> screen 75, 80): red.
        assert_eq!(px(&pm, 75, 80), (255, 0, 0));
        doc.metadata.fill_open_curves = true;
        let pm = render_page(&doc, page, &opts).unwrap();
        assert_eq!(px(&pm, 25, 50), (255, 0, 0));
    }

    #[test]
    fn calligraphic_outline_is_wide_across_the_nib_and_thin_along_it() {
        // 25.4 dpi: one pixel per millimetre. Flat 6 mm nib at 0 degrees.
        let mut nib = graphdraw_core::Stroke::hairline(Color::BLACK);
        nib.width = 6.0;
        nib.stretch = 0.1;
        nib.nib_angle = 0.0;
        let mut vertical = Shape::new(
            graphdraw_core::ShapeId(1),
            ShapeKind::Path {
                path: {
                    let mut p = graphdraw_core::geometry::BezPath::new();
                    p.move_to(graphdraw_core::geometry::Point::new(30.0, 10.0));
                    p.line_to(graphdraw_core::geometry::Point::new(30.0, 90.0));
                    p
                },
                closed: false,
            },
        );
        vertical.fill = Fill::None;
        vertical.stroke = Some(nib.clone());
        let mut horizontal = vertical.clone();
        horizontal.id = graphdraw_core::ShapeId(2);
        horizontal.kind = ShapeKind::Path {
            path: {
                let mut p = graphdraw_core::geometry::BezPath::new();
                p.move_to(graphdraw_core::geometry::Point::new(10.0, 70.0));
                p.line_to(graphdraw_core::geometry::Point::new(90.0, 70.0));
                p
            },
            closed: false,
        };
        horizontal.stroke = Some(nib);
        let (doc, page) = doc_with(vec![vertical, horizontal]);
        let pm = render_page_image(&doc, page, 25.4).unwrap();
        // The vertical stroke covers x 27..33 at y = 50 (image y = 50).
        assert_eq!(px(&pm, 28, 50), (0, 0, 0));
        assert_eq!(px(&pm, 32, 50), (0, 0, 0));
        assert_ne!(px(&pm, 25, 50), (0, 0, 0));
        // The horizontal stroke is only 0.6 mm tall: two pixels away it is gone.
        assert_ne!(px(&pm, 60, 28), (0, 0, 0));
        assert_ne!(px(&pm, 60, 32), (0, 0, 0));
    }

    #[test]
    fn invert_lens_inverts_what_is_beneath() {
        let base = rect(1, Rect::new(0.0, 0.0, 100.0, 100.0), Color::rgb8(255, 0, 0));
        let mut lens = rect(2, Rect::new(25.0, 25.0, 75.0, 75.0), Color::WHITE);
        lens.effects.push(graphdraw_core::live::Effect::Lens(
            graphdraw_core::live::Lens::Invert,
        ));
        let (doc, page) = doc_with(vec![base, lens]);
        let pm = render_page_image(&doc, page, 25.4).unwrap();
        assert_eq!(px(&pm, 50, 50), (0, 255, 255));
        assert_eq!(px(&pm, 5, 5), (255, 0, 0));
    }

    #[test]
    fn bitmap_effects_lens_runs_the_registered_effects_beneath_it() {
        // A stand-in for the application's effects: "swap" swaps red and
        // blue.
        fn run(effects: &[graphdraw_core::BitmapEffect], _w: u32, _h: u32, rgba: &mut Vec<u8>) {
            if effects.iter().any(|e| e.id == "swap") {
                for px in rgba.chunks_mut(4) {
                    px.swap(0, 2);
                }
            }
        }
        set_pixel_effects(run);
        let base = rect(1, Rect::new(0.0, 0.0, 100.0, 100.0), Color::rgb8(255, 0, 0));
        let mut lens = rect(2, Rect::new(25.0, 25.0, 75.0, 75.0), Color::WHITE);
        lens.effects.push(graphdraw_core::live::Effect::Lens(
            graphdraw_core::live::Lens::Effects {
                effects: vec![graphdraw_core::BitmapEffect {
                    id: "swap".into(),
                    params: Default::default(),
                    visible: true,
                }],
            },
        ));
        let (doc, page) = doc_with(vec![base, lens]);
        let pm = render_page_image(&doc, page, 25.4).unwrap();
        assert_eq!(px(&pm, 50, 50), (0, 0, 255));
        assert_eq!(px(&pm, 5, 5), (255, 0, 0));
    }

    #[test]
    fn simulated_overprint_multiplies_the_fill_over_what_is_beneath() {
        let yellow = rect(
            1,
            Rect::new(0.0, 0.0, 100.0, 100.0),
            Color::rgb8(255, 255, 0),
        );
        let mut grey = rect(
            2,
            Rect::new(25.0, 25.0, 75.0, 75.0),
            Color::rgb8(128, 128, 128),
        );
        grey.overprint_fill = true;
        let (doc, page) = doc_with(vec![yellow, grey]);
        // Off (the default): the grey knocks the yellow out.
        let pm = render_page_image(&doc, page, 25.4).unwrap();
        assert_eq!(px(&pm, 50, 50), (128, 128, 128));
        // On: multiply gives a darker yellow, and the surroundings stay yellow.
        let opts = RenderOptions {
            simulate_overprints: true,
            ..RenderOptions::default()
        };
        let pm = render_page_image_with(&doc, page, 25.4, &opts).unwrap();
        let (r, g, b) = px(&pm, 50, 50);
        assert!((120..=136).contains(&r) && r == g && b == 0, "{r} {g} {b}");
        assert_eq!(px(&pm, 5, 5), (255, 255, 0));
    }

    #[test]
    fn simulated_overprint_applies_to_outlines_and_leaves_plain_objects_alone() {
        let yellow = rect(
            1,
            Rect::new(0.0, 0.0, 100.0, 100.0),
            Color::rgb8(255, 255, 0),
        );
        // A thick grey outline across the page, no fill.
        let mut line = Shape::new(
            graphdraw_core::ShapeId(2),
            ShapeKind::Path {
                path: {
                    let mut p = graphdraw_core::geometry::BezPath::new();
                    p.move_to((0.0, 50.0));
                    p.line_to((100.0, 50.0));
                    p
                },
                closed: false,
            },
        );
        line.fill = Fill::None;
        line.stroke = Some(Stroke::new(Color::rgb8(128, 128, 128), 10.0));
        line.overprint_outline = true;
        // A plain grey square that does not overprint.
        let plain = rect(
            3,
            Rect::new(5.0, 5.0, 20.0, 20.0),
            Color::rgb8(128, 128, 128),
        );
        let (doc, page) = doc_with(vec![yellow, line, plain]);
        let opts = RenderOptions {
            simulate_overprints: true,
            ..RenderOptions::default()
        };
        let pm = render_page_image_with(&doc, page, 25.4, &opts).unwrap();
        let (r, g, b) = px(&pm, 50, 50);
        assert!(
            (120..=136).contains(&r) && r == g && b == 0,
            "outline {r} {g} {b}"
        );
        assert_eq!(px(&pm, 12, 87), (128, 128, 128), "plain fill knocks out");
    }

    #[test]
    fn outlines_thinner_than_a_pixel_stay_one_pixel_wide() {
        // A 0.1 mm black line at 25.4 dpi (a tenth of a pixel): drawn as a
        // full-strength one-pixel line, not faded to a tenth.
        let mut line = Shape::new(
            graphdraw_core::ShapeId(1),
            ShapeKind::Path {
                path: {
                    let mut p = graphdraw_core::geometry::BezPath::new();
                    p.move_to((0.0, 50.5));
                    p.line_to((100.0, 50.5));
                    p
                },
                closed: false,
            },
        );
        line.fill = Fill::None;
        line.stroke = Some(Stroke::new(Color::BLACK, 0.1));
        let (doc, page) = doc_with(vec![line]);
        let pm = render_page_image(&doc, page, 25.4).unwrap();
        let darkest = (0..100).map(|y| px(&pm, 50, y).0).min().unwrap_or(255);
        assert!(darkest < 64, "darkest {darkest}");
    }

    #[test]
    fn outlines_behind_the_fill_show_only_outside_it() {
        // A red square with a 10 mm white outline behind its fill: inside
        // the square stays red, the outline shows only outside.
        let mut s = rect(1, Rect::new(20.0, 20.0, 80.0, 80.0), Color::rgb8(255, 0, 0));
        let mut st = Stroke::new(Color::rgb8(0, 0, 255), 10.0);
        st.behind_fill = true;
        s.stroke = Some(st);
        let (doc, page) = doc_with(vec![s]);
        let pm = render_page_image(&doc, page, 25.4).unwrap();
        assert_eq!(px(&pm, 22, 50), (255, 0, 0), "fill over the inner half");
        assert_eq!(px(&pm, 17, 50), (0, 0, 255), "outline outside");
    }

    #[test]
    fn wireframe_draws_hairlines_whatever_the_outline() {
        // A red square with a 10 mm outline at half opacity.
        let mut s = rect(1, Rect::new(20.0, 20.0, 80.0, 80.0), Color::rgb8(255, 0, 0));
        s.stroke = Some(Stroke::new(Color::BLACK, 10.0));
        s.opacity = 0.5;
        let (doc, page) = doc_with(vec![s]);
        let opts = RenderOptions {
            wireframe: true,
            ..RenderOptions::default()
        };
        let pm = render_page_image_with(&doc, page, 25.4, &opts).unwrap();
        assert_eq!(px(&pm, 50, 50), (255, 255, 255), "no fill");
        assert_eq!(px(&pm, 24, 50), (255, 255, 255), "no outline width");
        assert_eq!(px(&pm, 16, 50), (255, 255, 255), "no outline width outside");
        let edge = (18..23).map(|x| px(&pm, x, 50).0).min().unwrap();
        assert!(edge < 200, "a dark hairline on the edge, darkest {edge}");
        // The hairline takes the outline's colour.
        let mut s = rect(1, Rect::new(20.0, 20.0, 80.0, 80.0), Color::WHITE);
        s.stroke = Some(Stroke::new(Color::rgb8(0, 0, 255), 10.0));
        let (doc, page) = doc_with(vec![s]);
        let pm = render_page_image_with(&doc, page, 25.4, &opts).unwrap();
        let blue = (18..23)
            .map(|x| px(&pm, x, 50))
            .find(|p| p.2 > 200 && p.0 < 160);
        assert!(blue.is_some(), "a blue hairline");
    }

    #[test]
    fn complex_effects_off_skips_the_lens() {
        let base = rect(1, Rect::new(0.0, 0.0, 100.0, 100.0), Color::rgb8(255, 0, 0));
        let mut lens = rect(2, Rect::new(25.0, 25.0, 75.0, 75.0), Color::WHITE);
        lens.effects.push(graphdraw_core::live::Effect::Lens(
            graphdraw_core::live::Lens::Invert,
        ));
        let (doc, page) = doc_with(vec![base, lens]);
        let full = render_page_image(&doc, page, 25.4).unwrap();
        assert_eq!(px(&full, 50, 50), (0, 255, 255), "lens inverts");
        let opts = RenderOptions {
            complex_effects: false,
            ..RenderOptions::default()
        };
        let fast = render_page_image_with(&doc, page, 25.4, &opts).unwrap();
        assert_eq!(
            px(&fast, 50, 50),
            (255, 255, 255),
            "plain white fill instead"
        );
        assert_eq!(px(&fast, 5, 5), (255, 0, 0));
        assert_ne!(full.data(), fast.data());
    }

    #[test]
    fn complex_effects_off_skips_the_transparency_mask() {
        let mut s = rect(1, Rect::new(0.0, 0.0, 100.0, 10.0), Color::rgb8(0, 0, 0));
        s.effects.push(graphdraw_core::live::Effect::Transparency {
            mask: Fill::linear(Color::WHITE, Color::BLACK, 0.0),
            merge: graphdraw_core::live::MergeMode::Normal,
            target: 2,
        });
        let (doc, page) = doc_with(vec![s]);
        let opts = RenderOptions {
            complex_effects: false,
            ..RenderOptions::default()
        };
        let pm = render_page_image_with(&doc, page, 25.4, &opts).unwrap();
        // Without the mask the bar is solid black from end to end.
        assert_eq!(px(&pm, 3, 95), (0, 0, 0));
        assert_eq!(px(&pm, 96, 95), (0, 0, 0));
    }

    #[test]
    fn custom_arrowhead_paints_the_line_end() {
        // A right-pointing triangle, 20 wide and 20 tall, as a custom head.
        let mut tri = graphdraw_core::geometry::BezPath::new();
        tri.move_to((0.0, 0.0));
        tri.line_to((20.0, 10.0));
        tri.line_to((0.0, 20.0));
        tri.close_path();
        let head = graphdraw_core::Arrowhead::from_shape_path(&tri, "Tri");
        let mut line = Shape::new(
            graphdraw_core::ShapeId(1),
            ShapeKind::Path {
                path: {
                    let mut p = graphdraw_core::geometry::BezPath::new();
                    p.move_to((10.0, 50.0));
                    p.line_to((60.0, 50.0));
                    p
                },
                closed: false,
            },
        );
        line.fill = Fill::None;
        line.stroke = Some(Stroke {
            end_arrow: head,
            ..Stroke::new(Color::rgb8(0, 0, 255), 2.0)
        });
        let (doc, page) = doc_with(vec![line]);
        let pm = render_page_image(&doc, page, 25.4).unwrap();
        // Size = 4 widths = 8 mm: the head spans x 52..60, y 46..54. Its
        // base is 8 mm tall at x = 52, well outside the 2 mm line; its tip
        // is at x = 60.
        let blue = |x: u32, y: u32| {
            let (r, g, b) = px(&pm, x, y);
            assert!(
                b == 255 && r < 40 && g < 40,
                "({x}, {y}) should be blue: {r} {g} {b}"
            );
        };
        blue(53, 48);
        blue(53, 52);
        blue(58, 50);
        // Beside the line, before the head, there is nothing.
        assert_eq!(px(&pm, 40, 47), (255, 255, 255));
        // Beyond the tip, nothing either.
        assert_eq!(px(&pm, 63, 50), (255, 255, 255));
        // Without a head the same spots beside the line end stay white.
        let mut plain = doc.clone();
        if let Some(s) = plain.pages[0].layers[0].shapes.first_mut() {
            if let Some(st) = s.stroke.as_mut() {
                st.end_arrow = graphdraw_core::Arrowhead::None;
            }
        }
        let pm2 = render_page_image(&plain, page, 25.4).unwrap();
        assert_eq!(px(&pm2, 53, 48), (255, 255, 255));
        assert_eq!(px(&pm2, 53, 52), (255, 255, 255));
    }

    #[test]
    fn fountain_transparency_fades_the_object() {
        let mut s = rect(1, Rect::new(0.0, 0.0, 100.0, 10.0), Color::rgb8(0, 0, 0));
        s.effects.push(graphdraw_core::live::Effect::Transparency {
            mask: Fill::linear(Color::WHITE, Color::BLACK, 0.0),
            merge: graphdraw_core::live::MergeMode::Normal,
            target: 2,
        });
        let (doc, page) = doc_with(vec![s]);
        let pm = render_page_image(&doc, page, 25.4).unwrap();
        let left = px(&pm, 3, 95).0;
        let right = px(&pm, 96, 95).0;
        assert!(left < 40, "left should be opaque black: {left}");
        assert!(
            right > 215,
            "right should be transparent (white page): {right}"
        );
    }

    #[test]
    fn transparency_applies_to_the_fill_or_the_outline_alone() {
        // A red bar with a thick black outline fading left to right.
        let bar = |target: u8| {
            let mut s = rect(1, Rect::new(10.0, 10.0, 90.0, 30.0), Color::rgb8(255, 0, 0));
            s.stroke = Some(Stroke::new(Color::BLACK, 4.0));
            s.effects.push(graphdraw_core::live::Effect::Transparency {
                mask: Fill::linear(Color::WHITE, Color::BLACK, 0.0),
                merge: graphdraw_core::live::MergeMode::Normal,
                target,
            });
            let (doc, page) = doc_with(vec![s]);
            render_page_image(&doc, page, 25.4).unwrap()
        };
        // Fill only: the fill fades out on the right, the outline stays.
        let fill = bar(0);
        assert!(px(&fill, 84, 80).1 > 200, "{:?}", px(&fill, 84, 80));
        assert_eq!(px(&fill, 89, 80), (0, 0, 0));
        // Outline only: the outline fades out, the fill stays red.
        let outline = bar(1);
        assert_eq!(px(&outline, 84, 80), (255, 0, 0));
        assert!(px(&outline, 89, 80).0 > 200, "{:?}", px(&outline, 89, 80));
        // Both: the two fade together.
        let both = bar(2);
        assert!(px(&both, 84, 80).1 > 200 && px(&both, 89, 80).0 > 200);
    }

    #[test]
    fn transparency_stop_opacity_fades_the_object() {
        // Both stops white (opaque); the end's opacity at 0 fades it out.
        let mut s = rect(1, Rect::new(10.0, 10.0, 90.0, 30.0), Color::rgb8(255, 0, 0));
        let mut mask = Fill::linear(Color::WHITE, Color::WHITE, 0.0);
        if let Fill::Fountain(f) = &mut mask {
            f.stops[1].opacity = 0.0;
        }
        s.effects.push(graphdraw_core::live::Effect::Transparency {
            mask,
            merge: graphdraw_core::live::MergeMode::Normal,
            target: 2,
        });
        let (doc, page) = doc_with(vec![s]);
        let pm = render_page_image(&doc, page, 25.4).unwrap();
        assert!(px(&pm, 15, 80).1 < 60, "{:?}", px(&pm, 15, 80));
        assert!(px(&pm, 85, 80).1 > 200, "{:?}", px(&pm, 85, 80));
    }

    #[test]
    fn transparency_works_on_pictures() {
        // A red picture fading left to right, whatever part is chosen:
        // the picture is the fill.
        let mut red = Pixmap::new(4, 4).unwrap();
        red.fill(tiny_skia::Color::from_rgba8(255, 0, 0, 255));
        let png = red.encode_png().unwrap();
        let picture = |target: Option<u8>, opacity: f64| {
            let mut s = Shape::new(
                graphdraw_core::ShapeId(1),
                ShapeKind::Bitmap {
                    rect: Rect::new(10.0, 10.0, 90.0, 30.0),
                    width_px: 4,
                    height_px: 4,
                    png: png.clone(),
                    fx: None,
                },
            );
            s.fill = Fill::None;
            s.stroke = None;
            s.opacity = opacity;
            if let Some(target) = target {
                s.effects.push(graphdraw_core::live::Effect::Transparency {
                    mask: Fill::linear(Color::WHITE, Color::BLACK, 0.0),
                    merge: graphdraw_core::live::MergeMode::Normal,
                    target,
                });
            }
            let (doc, page) = doc_with(vec![s]);
            render_page_image(&doc, page, 25.4).unwrap()
        };
        for target in [0, 2] {
            let pm = picture(Some(target), 1.0);
            let (l, r) = (px(&pm, 15, 80), px(&pm, 85, 80));
            assert!(l.0 > 200 && l.1 < 60, "target {target} left {l:?}");
            assert!(r.1 > 200, "target {target} right {r:?}");
        }
        // On the outline alone the picture itself stays whole.
        assert_eq!(px(&picture(Some(1), 1.0), 85, 80), (255, 0, 0));
        let half = px(&picture(None, 0.5), 50, 80);
        assert!(half.1 > 100 && half.1 < 155, "{half:?}");
    }

    #[test]
    fn transparency_merge_modes_mix_with_what_is_beneath() {
        let over = |merge, top: Color| {
            let under = rect(
                1,
                Rect::new(0.0, 0.0, 100.0, 100.0),
                Color::rgb8(200, 100, 50),
            );
            let mut s = rect(2, Rect::new(20.0, 20.0, 80.0, 80.0), top);
            s.effects.push(graphdraw_core::live::Effect::Transparency {
                mask: Fill::Solid(Color::WHITE),
                merge,
                target: 2,
            });
            let (doc, page) = doc_with(vec![under, s]);
            px(&render_page_image(&doc, page, 25.4).unwrap(), 50, 50)
        };
        use graphdraw_core::live::MergeMode as M;
        let grey = Color::rgb8(128, 128, 128);
        let near = |a: (u8, u8, u8), b: (u8, u8, u8)| {
            assert!(
                a.0.abs_diff(b.0) <= 2 && a.1.abs_diff(b.1) <= 2 && a.2.abs_diff(b.2) <= 2,
                "{a:?} != {b:?}"
            );
        };
        near(over(M::Multiply, grey), (100, 50, 25));
        near(over(M::Screen, grey), (228, 178, 153));
        near(over(M::Subtract, Color::rgb8(100, 100, 100)), (100, 0, 0));
        near(over(M::Red, Color::rgb8(0, 255, 255)), (0, 100, 50));
        near(over(M::Normal, grey), (128, 128, 128));
    }

    #[test]
    fn shaped_transparency_follows_the_object() {
        // White at the edge (opaque), black inside (clear): a black disc
        // fades towards its middle, the same all round.
        let id = graphdraw_core::ShapeId(1);
        let mut s = Shape::new(
            id,
            ShapeKind::Ellipse {
                rect: Rect::new(0.0, 0.0, 60.0, 60.0),
                arc: None,
            },
        );
        s.fill = Fill::Solid(Color::BLACK);
        s.stroke = None;
        s.effects.push(graphdraw_core::live::Effect::Transparency {
            mask: Fill::Fountain(Fountain::two(
                FountainKind::Shaped,
                Color::WHITE,
                Color::BLACK,
                0.0,
            )),
            merge: graphdraw_core::live::MergeMode::Normal,
            target: 2,
        });
        let (doc, page) = doc_with(vec![s]);
        let pm = render_page_image(&doc, page, 25.4).unwrap();
        let (centre, east, north) = (px(&pm, 30, 70).0, px(&pm, 52, 70).0, px(&pm, 30, 48).0);
        assert!(centre > 215, "the middle is clear: {centre}");
        assert!(
            east < 120 && (east as i32 - north as i32).abs() < 12,
            "{east} {north}"
        );
    }

    #[test]
    fn contour_effect_draws_outside_steps() {
        let mut s = rect(1, Rect::new(40.0, 40.0, 60.0, 60.0), Color::rgb8(255, 0, 0));
        s.effects.push(graphdraw_core::live::Effect::Contour {
            steps: 2,
            offset: 5.0,
            outside: true,
            to_center: false,
            fill_to: Color::rgb8(0, 0, 255),
            outline_to: None,
            color_blend: 0,
            corners: None,
            accel_objects: 0.0,
            accel_colors: 0.0,
        });
        let (doc, page) = doc_with(vec![s]);
        let pm = render_page_image(&doc, page, 25.4).unwrap();
        // y is flipped in the image: page y 50 is image row 50 either way (100 tall).
        assert_eq!(px(&pm, 50, 50), (255, 0, 0));
        let (r, _, b) = px(&pm, 32, 50);
        assert!(b > r, "outer contour step should be bluish");
        assert_eq!(px(&pm, 5, 5), (255, 255, 255));
    }

    /// Smooth colour carries a bright middle node's colour further into
    /// its cells than the bilinear mix does.
    #[test]
    fn smooth_mesh_colour_flows_through_the_nodes() {
        let render = |smooth: bool| {
            let mut s = rect(1, Rect::new(0.0, 0.0, 100.0, 100.0), Color::WHITE);
            let mut m =
                graphdraw_core::Mesh::new(Rect::new(0.0, 0.0, 100.0, 100.0), 2, 2, Color::BLACK);
            m.nodes[4].color = Color::WHITE; // the middle node
            m.smooth = smooth;
            s.fill = Fill::Mesh(m);
            let (doc, page) = doc_with(vec![s]);
            render_page_image(&doc, page, 25.4).unwrap()
        };
        let (flat, smooth) = (render(false), render(true));
        let (f, _, _) = px(&flat, 30, 50);
        let (g, _, _) = px(&smooth, 30, 50);
        assert!(g > f + 8, "smooth {g} should be brighter than bilinear {f}");
        // Next to the middle node it is nearly white either way.
        assert!(px(&smooth, 50, 50).0 > 200 && px(&flat, 50, 50).0 > 200);
    }

    #[test]
    fn mesh_fill_blends_corner_colours() {
        let mut s = rect(1, Rect::new(0.0, 0.0, 100.0, 100.0), Color::WHITE);
        let mut m =
            graphdraw_core::Mesh::new(Rect::new(0.0, 0.0, 100.0, 100.0), 1, 1, Color::WHITE);
        m.nodes[0].color = Color::rgb8(255, 0, 0); // bottom-left
        m.nodes[1].color = Color::rgb8(0, 0, 255); // bottom-right
        m.nodes[2].color = Color::rgb8(255, 0, 0);
        m.nodes[3].color = Color::rgb8(0, 0, 255);
        s.fill = Fill::Mesh(m);
        let (doc, page) = doc_with(vec![s]);
        let pm = render_page_image(&doc, page, 25.4).unwrap();
        let (r, _, b) = px(&pm, 5, 50);
        assert!(r > 200 && b < 60, "left should be red: {r} {b}");
        let (r, _, b) = px(&pm, 95, 50);
        assert!(b > 200 && r < 60, "right should be blue: {r} {b}");
        let (r, _, b) = px(&pm, 50, 50);
        assert!(
            (r as i32 - b as i32).abs() < 40,
            "middle should be purple: {r} {b}"
        );
    }

    #[test]
    fn linear_fountain_three_stops_passes_through_middle_colour() {
        let fill = Fill::Fountain(Fountain {
            kind: FountainKind::Linear,
            stops: vec![
                Stop::new(0.0, Color::rgb8(0, 0, 0)),
                Stop::new(0.5, Color::rgb8(255, 0, 0)),
                Stop::new(1.0, Color::rgb8(255, 255, 255)),
            ],
            angle: 0.0,
            offset: Point::ZERO,
            edge_pad: 0.0,
            ..Default::default()
        });
        let pm = render_fill_image(&fill, Rect::new(0.0, 0.0, 100.0, 10.0), 25.4).unwrap();
        assert_eq!(pm.width(), 100);
        let (r, g, b) = px(&pm, 50, 5);
        assert!(r > 240 && g < 20 && b < 20, "middle {r} {g} {b}");
        let (r, _, _) = px(&pm, 2, 5);
        assert!(r < 20);
        let (_, g, _) = px(&pm, 97, 5);
        assert!(g > 230);
    }

    #[test]
    fn conical_fountain_differs_around_centre() {
        let fill = Fill::Fountain(Fountain::two(
            FountainKind::Conical,
            Color::rgb8(0, 0, 255),
            Color::rgb8(255, 255, 0),
            0.0,
        ));
        let pm = render_fill_image(&fill, Rect::new(0.0, 0.0, 100.0, 100.0), 25.4).unwrap();
        let a = px(&pm, 90, 50);
        let b = px(&pm, 10, 50);
        assert_ne!(a, b);
    }

    #[test]
    fn checker_pattern_alternates() {
        let fill = Fill::Pattern(Pattern::TwoColor {
            tile: PatternTile::Checker,
            front: Color::rgb8(0, 0, 0),
            back: Color::rgb8(255, 255, 255),
            size_mm: 20.0,
            layout: Default::default(),
        });
        let pm = render_fill_image(&fill, Rect::new(0.0, 0.0, 40.0, 40.0), 25.4).unwrap();
        let a = px(&pm, 5, 5);
        let b = px(&pm, 15, 5);
        assert_ne!(a.0, b.0, "checker cells should differ");
        assert!(a.0 == 0 || a.0 == 255);
    }

    #[test]
    fn vector_pattern_tiles_the_shape() {
        // A 10 mm tile with a red square in its lower-left quarter.
        let tile_square = rect(7, Rect::new(0.0, 0.0, 5.0, 5.0), Color::rgb8(255, 0, 0));
        let mut s = rect(1, Rect::new(0.0, 0.0, 20.0, 20.0), Color::WHITE);
        s.fill = Fill::Pattern(Pattern::Vector {
            shapes: vec![tile_square],
            tile: graphdraw_core::geometry::Size::new(10.0, 10.0),
            layout: Default::default(),
        });
        let mut doc = Document::new("t", graphdraw_core::geometry::Size::new(20.0, 20.0));
        let page = doc.pages[0].id;
        let layer = doc.pages[0].layers[0].id;
        doc.layer_mut(layer).unwrap().shapes.push(s);
        let pm = render_page_image(&doc, page, 25.4).unwrap(); // 1 px per mm
        assert_eq!((pm.width(), pm.height()), (20, 20));
        // Tiles start at the top-left corner; the red square sits in the
        // lower-left quarter of each tile (image rows 5..10 and 15..20).
        for (x, y) in [(2, 7), (12, 7), (2, 17), (12, 17)] {
            assert_eq!(px(&pm, x, y), (255, 0, 0), "({x}, {y}) must be red");
        }
        for (x, y) in [(2, 2), (7, 7), (7, 2), (12, 12), (17, 17), (17, 7)] {
            assert_eq!(
                px(&pm, x, y),
                (255, 255, 255),
                "({x}, {y}) must be page white"
            );
        }
    }

    #[test]
    fn vector_pattern_tile_draws_nested_groups_and_outlines() {
        let mut child = rect(8, Rect::new(2.0, 2.0, 8.0, 8.0), Color::rgb8(0, 0, 255));
        child.stroke = Some(Stroke::new(Color::rgb8(0, 255, 0), 2.0));
        let group = Shape::new(
            graphdraw_core::ShapeId(9),
            ShapeKind::Group {
                children: vec![child],
            },
        );
        let fill = Fill::Pattern(Pattern::Vector {
            shapes: vec![group],
            tile: graphdraw_core::geometry::Size::new(10.0, 10.0),
            layout: Default::default(),
        });
        let pm = render_fill_image(&fill, Rect::new(0.0, 0.0, 10.0, 10.0), 254.0).unwrap();
        // 10 px per mm: the centre is blue, the 2 mm outline band is green.
        assert_eq!(px(&pm, 50, 50), (0, 0, 255));
        assert_eq!(px(&pm, 20, 50), (0, 255, 0));
        assert_eq!(px(&pm, 5, 5), (0, 0, 0));
        assert_eq!(
            pm.pixel(5, 5).unwrap().alpha(),
            0,
            "outside the tile content is clear"
        );
    }

    #[test]
    fn degenerate_vector_pattern_does_not_panic() {
        for tile in [
            graphdraw_core::geometry::Size::new(0.0, 10.0),
            graphdraw_core::geometry::Size::new(f64::NAN, 10.0),
            graphdraw_core::geometry::Size::new(0.0001, 0.0001),
            graphdraw_core::geometry::Size::new(1.0e9, 1.0e9),
        ] {
            let fill = Fill::Pattern(Pattern::Vector {
                shapes: vec![rect(1, Rect::new(0.0, 0.0, 1.0, 1.0), Color::BLACK)],
                tile,
                layout: Default::default(),
            });
            assert!(render_fill_image(&fill, Rect::new(0.0, 0.0, 10.0, 10.0), 96.0).is_some());
        }
    }

    #[test]
    fn texture_is_deterministic_for_seed() {
        let t = Fill::Texture(Texture {
            kind: TextureKind::Clouds,
            color_a: Color::rgb8(0, 0, 0),
            color_b: Color::rgb8(255, 255, 255),
            scale: 10.0,
            seed: 7,
            ..Default::default()
        });
        let a = render_fill_image(&t, Rect::new(0.0, 0.0, 30.0, 30.0), 25.4).unwrap();
        let b = render_fill_image(&t, Rect::new(0.0, 0.0, 30.0, 30.0), 25.4).unwrap();
        assert_eq!(a.data(), b.data());
        let v: std::collections::HashSet<u8> = (0..30).map(|x| px(&a, x, 15).0).collect();
        assert!(v.len() > 3, "texture should vary");
    }

    #[test]
    fn large_textures_on_screen_are_quick_and_exports_keep_every_pixel() {
        let mut doc = Document::new("t", graphdraw_core::geometry::Size::new(300.0, 300.0));
        let layer = doc.pages[0].layers[0].id;
        let id = doc.ids_mut().shape();
        let mut s = Shape::new(
            id,
            ShapeKind::Rect {
                rect: Rect::new(0.0, 0.0, 300.0, 300.0),
                radius: 0.0,
                corners: None,
            },
        );
        s.fill = Fill::Texture(Texture {
            kind: TextureKind::Clouds,
            color_a: Color::Cmyk {
                c: 1.0,
                m: 0.2,
                y: 0.0,
                k: 0.1,
            },
            color_b: Color::rgb8(255, 255, 255),
            scale: 40.0,
            seed: 11,
            ..Default::default()
        });
        s.stroke = None;
        doc.layer_mut(layer).unwrap().shapes.push(s);
        let page = doc.pages[0].id;
        let opts = |draft: bool| RenderOptions {
            width: 1600,
            height: 1000,
            view: ViewTransform {
                zoom: 6.0,
                origin_x: -100.0,
                origin_y: 1500.0,
            },
            draft,
            ..Default::default()
        };
        let exact = render_page(&doc, page, &opts(false)).unwrap();
        let t = std::time::Instant::now();
        let draft = render_page(&doc, page, &opts(true)).unwrap();
        let first = t.elapsed();
        let t = std::time::Instant::now();
        let again = render_page(&doc, page, &opts(true)).unwrap();
        let cached = t.elapsed();
        assert_eq!(draft.data(), again.data(), "the kept result is the same");
        assert!(cached <= first, "{cached:?} {first:?}");
        // The coarser grid stays close to the exact picture.
        let mut worst = 0i32;
        for (a, b) in exact.pixels().iter().zip(draft.pixels()).step_by(97) {
            worst = worst.max((a.red() as i32 - b.red() as i32).abs());
        }
        assert!(worst < 40, "{worst}");
    }

    #[test]
    fn outlines_take_a_gradient_fill() {
        let mut doc = Document::new("t", graphdraw_core::geometry::Size::new(100.0, 20.0));
        let layer = doc.pages[0].layers[0].id;
        let id = doc.ids_mut().shape();
        let mut path = graphdraw_core::BezPath::new();
        path.move_to((5.0, 10.0));
        path.line_to((95.0, 10.0));
        let mut s = Shape::new(
            id,
            ShapeKind::Path {
                path,
                closed: false,
            },
        );
        s.fill = Fill::None;
        let mut stroke = Stroke::new(Color::rgb8(0, 255, 0), 6.0);
        stroke.fill = Some(Box::new(Fill::Fountain(graphdraw_core::Fountain::two(
            graphdraw_core::FountainKind::Linear,
            Color::rgb8(255, 0, 0),
            Color::rgb8(0, 0, 255),
            0.0,
        ))));
        s.stroke = Some(stroke);
        doc.layer_mut(layer).unwrap().shapes.push(s);
        let img = render_page_image(&doc, doc.pages[0].id, 25.4).unwrap();
        let left = img.pixel(8, 10).unwrap();
        let right = img.pixel(92, 10).unwrap();
        assert!(left.red() > 200 && left.blue() < 60, "{left:?}");
        assert!(right.blue() > 200 && right.red() < 60, "{right:?}");
        assert!(left.green() < 30, "not the outline colour");
        // Outside the stroke nothing is painted.
        assert_eq!(img.pixel(50, 2).unwrap().green(), 255);
    }

    /// A 100 x 10 mm rectangle filled with `fill`, drawn at 1 px/mm.
    fn strip(fill: Fill) -> Pixmap {
        let mut doc = Document::new("t", graphdraw_core::geometry::Size::new(100.0, 10.0));
        let page = doc.pages[0].id;
        let mut s = rect(1, Rect::new(0.0, 0.0, 100.0, 10.0), Color::WHITE);
        s.fill = fill;
        s.stroke = None;
        doc.layer_mut(doc.pages[0].layers[0].id)
            .unwrap()
            .shapes
            .push(s);
        render_page_image(&doc, page, 25.4).unwrap()
    }

    #[test]
    fn linear_fountains_repeat_mirror_and_invert() {
        let bw = || Fountain::two(FountainKind::Linear, Color::BLACK, Color::WHITE, 0.0);
        let red_at = |img: &Pixmap, x: u32| img.pixel(x, 5).unwrap().red();
        // Sawtooth twice: light just before the middle, dark just after.
        let mut f = bw();
        f.repeat = graphdraw_core::FountainRepeat::Repeat;
        f.repeats = 2;
        let img = strip(Fill::Fountain(f));
        assert!(red_at(&img, 48) > 220, "{}", red_at(&img, 48));
        assert!(red_at(&img, 51) < 35, "{}", red_at(&img, 51));
        assert!((red_at(&img, 25) as i32 - 128).abs() < 20);
        // Triangular twice: white in the middle, black at both ends.
        let mut m = bw();
        m.repeat = graphdraw_core::FountainRepeat::Mirror;
        m.repeats = 2;
        let img = strip(Fill::Fountain(m));
        assert!(red_at(&img, 50) > 240 && red_at(&img, 1) < 15 && red_at(&img, 98) < 15);
        // Bilinear gives the same run.
        let mut b = bw();
        b.bilinear = true;
        let bimg = strip(Fill::Fountain(b));
        for x in [1, 25, 50, 75, 98] {
            assert!((red_at(&img, x) as i32 - red_at(&bimg, x) as i32).abs() <= 3);
        }
        // Inverted: white on the left.
        let mut i = bw();
        i.invert = true;
        let img = strip(Fill::Fountain(i));
        assert!(red_at(&img, 1) > 240 && red_at(&img, 98) < 15);
        // Conical and square fountains take the options too.
        let mut sq = Fountain::two(FountainKind::Square, Color::BLACK, Color::WHITE, 0.0);
        sq.invert = true;
        let img = strip(Fill::Fountain(sq));
        assert!(red_at(&img, 50) > 200, "{}", red_at(&img, 50));
        assert!(red_at(&img, 1) < 30, "{}", red_at(&img, 1));
    }

    /// GD-FIL-03: the size and axis offset, midpoints, steps, opacity,
    /// smoothing and the elliptical kind as drawn.
    #[test]
    fn fountain_extras_are_drawn() {
        let bw = || Fountain::two(FountainKind::Linear, Color::BLACK, Color::WHITE, 0.0);
        let red_at = |img: &Pixmap, x: u32| img.pixel(x, 5).unwrap().red() as i32;
        // Half the span, starting on the left: white from the middle on.
        let mut f = bw();
        f.scale = 0.5;
        f.axis_offset = -0.5;
        let img = strip(Fill::Fountain(f.clone()));
        assert!((red_at(&img, 25) - 128).abs() < 20, "{}", red_at(&img, 25));
        assert!(red_at(&img, 75) > 245);
        // The same with a repeat keeps repeating past the span.
        f.repeat = graphdraw_core::FountainRepeat::Repeat;
        let img = strip(Fill::Fountain(f));
        assert!(red_at(&img, 52) < 30, "{}", red_at(&img, 52));
        // The midpoint at 20%: half grey there.
        let mut m = bw();
        m.stops[0].midpoint = 0.2;
        let img = strip(Fill::Fountain(m));
        assert!((red_at(&img, 20) - 128).abs() < 20, "{}", red_at(&img, 20));
        // Four steps: one colour per band.
        let mut s = bw();
        s.steps = 4;
        let img = strip(Fill::Fountain(s));
        assert!((red_at(&img, 3) - red_at(&img, 21)).abs() <= 2);
        assert!(red_at(&img, 28) > red_at(&img, 21) + 40);
        // A transparent end shows the white page.
        let mut o = bw();
        o.stops[0].opacity = 0.0;
        let img = strip(Fill::Fountain(o));
        assert!(red_at(&img, 1) > 240, "{}", red_at(&img, 1));
        // Smoothed: about the same colours.
        let mut sm = bw();
        sm.smooth = true;
        let img = strip(Fill::Fountain(sm));
        assert!((red_at(&img, 50) - 128).abs() < 20 && red_at(&img, 1) < 20);
        // Elliptical: dark in the centre, lighter towards the ends.
        let e = Fountain::two(FountainKind::Elliptical, Color::BLACK, Color::WHITE, 0.0);
        let img = strip(Fill::Fountain(e));
        assert!(
            red_at(&img, 50) < 20 && red_at(&img, 2) > 120,
            "{}",
            red_at(&img, 2)
        );
    }

    #[test]
    fn shaped_fountains_follow_the_outline() {
        // Black on the edge, white deepest inside.
        let f = Fountain::two(FountainKind::Shaped, Color::BLACK, Color::WHITE, 0.0);
        let img = strip(Fill::Fountain(f.clone()));
        let at = |img: &Pixmap, x: u32, y: u32| img.pixel(x, y).unwrap().red() as i32;
        assert!(at(&img, 50, 0) < 40, "edge {}", at(&img, 50, 0));
        assert!(at(&img, 50, 5) > 200, "middle {}", at(&img, 50, 5));
        // Along a long strip the colour depends on the distance to the
        // nearest edge, not on the distance to the centre.
        assert!((at(&img, 20, 5) - at(&img, 80, 5)).abs() <= 6);
        assert!((at(&img, 20, 5) - at(&img, 50, 5)).abs() <= 6);
        assert!(at(&img, 2, 5) < at(&img, 20, 5));
        // A circle: rings of equal colour.
        let (mut doc, page) = doc_with(vec![]);
        let id = doc.ids_mut().shape();
        let mut c = Shape::new(
            id,
            ShapeKind::Ellipse {
                rect: Rect::new(0.0, 0.0, 60.0, 60.0),
                arc: None,
            },
        );
        c.fill = Fill::Fountain(f);
        c.stroke = None;
        doc.layer_mut(doc.pages[0].layers[0].id)
            .unwrap()
            .shapes
            .push(c);
        let img = render_page_image(&doc, page, 25.4).unwrap();
        // Page y is up: the circle's centre is at (30, page height - 30).
        let cy = img.height() - 30;
        let (e, w, n) = (at(&img, 45, cy), at(&img, 15, cy), at(&img, 30, cy - 15));
        assert!((e - w).abs() <= 8 && (e - n).abs() <= 8, "{e} {w} {n}");
        assert!(at(&img, 30, cy) > 230, "{}", at(&img, 30, cy));
        assert!(at(&img, 2, cy) < at(&img, 15, cy));
    }

    #[test]
    fn edge_distance_measures_to_the_nearest_outside_pixel() {
        // A 7 x 5 grid with a 5 x 3 block inside.
        let (w, h) = (7usize, 5usize);
        let inside: Vec<bool> = (0..w * h)
            .map(|i| (1..6).contains(&(i % w)) && (1..4).contains(&(i / w)))
            .collect();
        let d = edge_distance(&inside, w, h);
        assert_eq!(d[0], 0.0);
        assert!((d[w + 1] - 0.5).abs() < 1e-9);
        assert!((d[2 * w + 3] - 1.5).abs() < 1e-9);
        assert!(edge_distance(&[], 0, 0).is_empty());
    }

    #[test]
    fn outlines_take_pattern_texture_mesh_and_shaped_paints() {
        let line = |paint: Fill| {
            let mut doc = Document::new("t", graphdraw_core::geometry::Size::new(100.0, 20.0));
            let layer = doc.pages[0].layers[0].id;
            let id = doc.ids_mut().shape();
            let mut path = graphdraw_core::BezPath::new();
            path.move_to((5.0, 10.0));
            path.line_to((95.0, 10.0));
            let mut s = Shape::new(
                id,
                ShapeKind::Path {
                    path,
                    closed: false,
                },
            );
            s.fill = Fill::None;
            let mut stroke = Stroke::new(Color::rgb8(0, 255, 0), 6.0);
            stroke.fill = Some(Box::new(paint));
            s.stroke = Some(stroke);
            doc.layer_mut(layer).unwrap().shapes.push(s);
            render_page_image(&doc, doc.pages[0].id, 25.4).unwrap()
        };
        let red = Color::rgb8(255, 0, 0);
        let blue = Color::rgb8(0, 0, 255);
        // A two-colour pattern: both colours along the outline, never the
        // outline colour, nothing beside it.
        let img = line(Fill::Pattern(Pattern::TwoColor {
            tile: graphdraw_core::PatternTile::Stripes,
            front: red,
            back: blue,
            size_mm: 4.0,
            layout: Default::default(),
        }));
        let row: Vec<_> = (6..94).map(|x| img.pixel(x, 10).unwrap()).collect();
        assert!(row.iter().all(|p| p.green() < 30));
        assert_eq!(img.pixel(50, 2).unwrap().green(), 255);
        let col: Vec<_> = (7..13).map(|y| img.pixel(50, y).unwrap()).collect();
        assert!(col.iter().any(|p| p.red() > 200) && col.iter().any(|p| p.blue() > 200));
        // A texture.
        let img = line(Fill::Texture(Texture {
            kind: graphdraw_core::TextureKind::Noise,
            color_a: red,
            color_b: blue,
            scale: 2.0,
            seed: 3,
            ..Default::default()
        }));
        let p = img.pixel(50, 10).unwrap();
        assert!(p.green() < 30 && p.red() + p.blue() > 150, "{p:?}");
        // A mesh: its nodes are local to the object, red on the left and
        // blue on the right.
        let mut m = graphdraw_core::Mesh::new(Rect::new(5.0, 7.0, 95.0, 13.0), 1, 1, red);
        for (i, n) in m.nodes.iter_mut().enumerate() {
            if i % 2 == 1 {
                n.color = blue;
            }
        }
        let img = line(Fill::Mesh(m));
        let (l, r) = (img.pixel(8, 10).unwrap(), img.pixel(92, 10).unwrap());
        assert!(l.red() > 200 && l.blue() < 60, "{l:?}");
        assert!(r.blue() > 200 && r.red() < 60, "{r:?}");
        // A shaped gradient follows the outline's own area: its edge
        // dark, its middle light.
        let img = line(Fill::Fountain(Fountain::two(
            FountainKind::Shaped,
            Color::BLACK,
            Color::WHITE,
            0.0,
        )));
        let (edge, mid) = (img.pixel(50, 7).unwrap(), img.pixel(50, 10).unwrap());
        assert!(mid.red() > edge.red() + 60, "{edge:?} {mid:?}");
    }

    #[test]
    fn procedural_fills_work_out_only_the_visible_part() {
        let mut doc = Document::new("t", graphdraw_core::geometry::Size::new(100.0, 100.0));
        let layer = doc.pages[0].layers[0].id;
        let id = doc.ids_mut().shape();
        let mut s = Shape::new(
            id,
            ShapeKind::Rect {
                rect: Rect::new(0.0, 0.0, 2.0, 2.0),
                radius: 0.0,
                corners: None,
            },
        );
        s.fill = Fill::Texture(Texture {
            kind: TextureKind::Clouds,
            color_a: Color::rgb8(0, 0, 0),
            color_b: Color::rgb8(255, 255, 255),
            scale: 0.5,
            seed: 3,
            ..Default::default()
        });
        s.stroke = None;
        doc.layer_mut(layer).unwrap().shapes.push(s);
        let page = doc.pages[0].id;
        let view = |w: u32, ox: f64, zoom: f64| RenderOptions {
            width: w,
            height: 64,
            view: ViewTransform {
                zoom,
                origin_x: ox,
                origin_y: 64.0,
            },
            ..Default::default()
        };
        // The whole square, then its right half alone: the same pixels.
        let whole = render_page(&doc, page, &view(64, 0.0, 32.0)).unwrap();
        let half = render_page(&doc, page, &view(32, -32.0, 32.0)).unwrap();
        for y in [5u32, 30, 60] {
            for x in [2u32, 15, 29] {
                assert_eq!(
                    whole.pixel(x + 32, y).unwrap(),
                    half.pixel(x, y).unwrap(),
                    "{x},{y}"
                );
            }
        }
        // Zoomed so far that the square is 200 000 pixels wide: only the
        // window is worked out, at once.
        let t = std::time::Instant::now();
        let deep = render_page(&doc, page, &view(64, 0.0, 100_000.0)).unwrap();
        assert!(t.elapsed().as_secs_f64() < 2.0);
        assert!(deep.pixel(10, 10).unwrap().alpha() == 255);
    }

    #[test]
    fn star_fill_covers_center_and_not_corner() {
        let mut doc = Document::new("t", graphdraw_core::geometry::Size::new(100.0, 100.0));
        let layer = doc.pages[0].layers[0].id;
        let id = doc.ids_mut().shape();
        let mut s = Shape::new(
            id,
            ShapeKind::Polygon {
                rect: Rect::new(10.0, 10.0, 90.0, 90.0),
                points: 5,
                sharpness: 0.6,
                complex: None,
                twist: 0.0,
            },
        );
        s.fill = Fill::Solid(Color::rgb8(255, 0, 0));
        s.stroke = None;
        doc.layer_mut(layer).unwrap().shapes.push(s);
        let page = doc.pages[0].id;
        let img = render_page_image(&doc, page, 25.4).unwrap(); // 1 px per mm
        let px = |x: u32, y: u32| img.pixel(x, y).unwrap();
        // Centre of the star is red; the page corner is white.
        assert_eq!(px(50, 50).red(), 255);
        assert_eq!(px(50, 50).green(), 0);
        assert_eq!(px(2, 2).green(), 255);
    }

    #[test]
    fn without_anti_aliasing_edges_are_hard() {
        let mut ellipse = Shape::new(
            graphdraw_core::ShapeId(3),
            ShapeKind::Ellipse {
                rect: Rect::new(10.0, 10.0, 90.0, 70.0),
                arc: None,
            },
        );
        ellipse.fill = Fill::Solid(Color::rgb8(0, 0, 0));
        ellipse.stroke = None;
        let (doc, page) = doc_with(vec![ellipse]);
        let render = |anti_alias: bool| {
            let opts = RenderOptions {
                width: 100,
                height: 100,
                view: ViewTransform {
                    zoom: 1.0,
                    origin_x: 0.0,
                    origin_y: 100.0,
                },
                anti_alias,
                ..RenderOptions::default()
            };
            render_page(&doc, page, &opts).unwrap()
        };
        let partial = |pm: &Pixmap| {
            pm.pixels()
                .iter()
                .filter(|p| p.alpha() != 0 && p.alpha() != 255)
                .count()
        };
        assert!(partial(&render(true)) > 20);
        assert_eq!(partial(&render(false)), 0);
    }

    #[test]
    fn half_transparent_red_over_white_is_pink() {
        let mut doc = Document::new("t", graphdraw_core::geometry::Size::new(20.0, 20.0));
        let layer = doc.pages[0].layers[0].id;
        let id = doc.ids_mut().shape();
        let mut s = Shape::new(
            id,
            ShapeKind::Rect {
                rect: Rect::new(0.0, 0.0, 20.0, 20.0),
                radius: 0.0,
                corners: None,
            },
        );
        s.fill = Fill::Solid(Color::rgb8(255, 0, 0));
        s.stroke = None;
        s.opacity = 0.5;
        doc.layer_mut(layer).unwrap().shapes.push(s);
        let page = doc.pages[0].id;
        let img = render_page_image(&doc, page, 25.4).unwrap();
        let p = img.pixel(10, 10).unwrap();
        assert_eq!(p.red(), 255);
        assert!((120..=136).contains(&p.green()), "green was {}", p.green());
    }

    #[test]
    fn shadow_darkens_pixels_beside_the_object() {
        let mut doc = Document::new("t", graphdraw_core::geometry::Size::new(40.0, 40.0));
        let layer = doc.pages[0].layers[0].id;
        let id = doc.ids_mut().shape();
        let mut s = Shape::new(
            id,
            ShapeKind::Rect {
                rect: Rect::new(10.0, 10.0, 25.0, 25.0),
                radius: 0.0,
                corners: None,
            },
        );
        s.fill = Fill::Solid(Color::rgb8(0, 0, 255));
        s.stroke = None;
        s.shadow = Some(graphdraw_core::Shadow {
            offset: graphdraw_core::geometry::Vec2::new(5.0, -5.0),
            opacity: 0.8,
            blur: 0.0,
            color: Color::BLACK,
        });
        doc.layer_mut(layer).unwrap().shapes.push(s);
        let page = doc.pages[0].id;
        let img = render_page_image(&doc, page, 25.4).unwrap();
        // Just right of the rect and below: shadow only (y down in the image).
        let p = img.pixel(28, 32).unwrap();
        assert!(p.red() < 100, "red was {}", p.red());
        let far = img.pixel(2, 2).unwrap();
        assert_eq!(far.red(), 255);
    }

    #[test]
    fn dashed_hairline_does_not_panic() {
        let mut doc = Document::new("t", graphdraw_core::geometry::Size::new(50.0, 50.0));
        let layer = doc.pages[0].layers[0].id;
        let id = doc.ids_mut().shape();
        let mut s = Shape::new(
            id,
            ShapeKind::Rect {
                rect: Rect::new(5.0, 5.0, 45.0, 45.0),
                radius: 0.0,
                corners: None,
            },
        );
        let mut st = Stroke::new(Color::BLACK, 1.0);
        st.dash = vec![3.0, 1.0, 1.0];
        s.stroke = Some(st);
        doc.layer_mut(layer).unwrap().shapes.push(s);
        let page = doc.pages[0].id;
        assert!(render_page_image(&doc, page, 96.0).is_some());
    }

    #[test]
    fn fill_transparency_leaves_the_outline_opaque() {
        // A red square at 50 % fill transparency with a 10 mm black outline.
        let near = |a: u8, b: u8| (a as i32 - b as i32).abs() <= 3;
        let mut s = rect(1, Rect::new(20.0, 20.0, 80.0, 80.0), Color::rgb8(255, 0, 0));
        s.stroke = Some(Stroke::new(Color::BLACK, 10.0));
        s.fill_opacity = 0.5;
        let (doc, page) = doc_with(vec![s.clone()]);
        let pm = render_page_image(&doc, page, 25.4).unwrap();
        let mid = px(&pm, 50, 50);
        assert!(
            mid.0 == 255 && near(mid.1, 128) && near(mid.2, 128),
            "{mid:?}"
        );
        assert_eq!(px(&pm, 20, 50), (0, 0, 0), "the outline stays opaque");
        // With the object's own opacity too, the two multiply (25 % red).
        s.opacity = 0.5;
        let (doc, page) = doc_with(vec![s.clone()]);
        let pm = render_page_image(&doc, page, 25.4).unwrap();
        let mid = px(&pm, 50, 50);
        assert!(
            mid.0 == 255 && near(mid.1, 191) && near(mid.2, 191),
            "{mid:?}"
        );
        let edge = px(&pm, 20, 50);
        assert!(
            near(edge.0, 128),
            "the outline at the object's opacity: {edge:?}"
        );
        // No fill at all with a fill transparency of 100 %.
        s.opacity = 1.0;
        s.fill_opacity = 0.0;
        let (doc, page) = doc_with(vec![s]);
        let pm = render_page_image(&doc, page, 25.4).unwrap();
        assert_eq!(px(&pm, 50, 50), (255, 255, 255));
    }

    #[test]
    fn fill_transparency_applies_to_every_fill_kind() {
        let near = |a: u8, b: u8| (a as i32 - b as i32).abs() <= 3;
        let black = Color::rgb8(0, 0, 0);
        let fills = [
            Fill::Fountain(graphdraw_core::Fountain::two(
                graphdraw_core::FountainKind::Linear,
                black,
                black,
                0.0,
            )),
            Fill::Mesh(graphdraw_core::Mesh::new(
                Rect::new(20.0, 20.0, 80.0, 80.0),
                2,
                2,
                black,
            )),
        ];
        for fill in fills {
            let mut s = rect(1, Rect::new(20.0, 20.0, 80.0, 80.0), Color::WHITE);
            s.stroke = None;
            s.fill = fill.clone();
            s.fill_opacity = 0.5;
            let (doc, page) = doc_with(vec![s]);
            let pm = render_page_image(&doc, page, 25.4).unwrap();
            let mid = px(&pm, 50, 50);
            assert!(
                near(mid.0, 128) && near(mid.1, 128) && near(mid.2, 128),
                "{fill:?}: {mid:?}"
            );
        }
    }
}

/// The outline's transparency, position and dashes through corners on
/// screen.
#[cfg(test)]
mod outline_option_tests {
    use super::*;
    use graphdraw_core::{DashCorners, StrokePosition};

    fn px(pm: &Pixmap, x: u32, y: u32) -> (u8, u8, u8) {
        let p = pm.pixel(x, y).unwrap().demultiply();
        (p.red(), p.green(), p.blue())
    }

    fn page(shapes: Vec<Shape>) -> Pixmap {
        let mut doc = Document::new("t", graphdraw_core::geometry::Size::new(100.0, 100.0));
        let page = doc.pages[0].id;
        let layer = doc.pages[0].layers[0].id;
        for s in shapes {
            doc.layer_mut(layer).unwrap().shapes.push(s);
        }
        // One pixel per millimetre.
        render_page_image(&doc, page, 25.4).unwrap()
    }

    /// A 60 mm square from 20 to 80 with a 10 mm black outline, no fill.
    fn square(stroke: Stroke) -> Shape {
        let mut s = Shape::new(
            graphdraw_core::ShapeId(1),
            ShapeKind::Rect {
                rect: Rect::new(20.0, 20.0, 80.0, 80.0),
                radius: 0.0,
                corners: None,
            },
        );
        s.fill = Fill::None;
        s.stroke = Some(stroke);
        s
    }

    fn line(stroke: Stroke) -> Shape {
        let mut p = BezPath::new();
        p.move_to((10.0, 50.5));
        p.line_to((90.0, 50.5));
        let mut s = Shape::new(
            graphdraw_core::ShapeId(1),
            ShapeKind::Path {
                path: p,
                closed: false,
            },
        );
        s.fill = Fill::None;
        s.stroke = Some(stroke);
        s
    }

    fn dark(c: (u8, u8, u8)) -> bool {
        c.0 < 60 && c.1 < 60 && c.2 < 60
    }

    fn white(c: (u8, u8, u8)) -> bool {
        c.0 > 230 && c.1 > 230 && c.2 > 230
    }

    #[test]
    fn outside_and_inside_outlines_lie_on_one_side_of_the_path() {
        let black = Stroke::new(Color::BLACK, 10.0);
        // Centred: 5 mm each side of x = 20.
        let pm = page(vec![square(black.clone())]);
        assert!(dark(px(&pm, 17, 50)) && dark(px(&pm, 23, 50)));
        assert!(white(px(&pm, 12, 50)) && white(px(&pm, 28, 50)));
        // Outside: 10 mm out, nothing in.
        let out = Stroke {
            position: StrokePosition::Outside,
            ..black.clone()
        };
        let pm = page(vec![square(out)]);
        assert!(dark(px(&pm, 12, 50)) && dark(px(&pm, 17, 50)));
        assert!(white(px(&pm, 23, 50)) && white(px(&pm, 8, 50)));
        // Outside corners stay sharp (mitred).
        assert!(dark(px(&pm, 12, 12)));
        // Inside: 10 mm in, nothing out.
        let inside = Stroke {
            position: StrokePosition::Inside,
            ..black.clone()
        };
        let pm = page(vec![square(inside)]);
        assert!(dark(px(&pm, 23, 50)) && dark(px(&pm, 28, 50)));
        assert!(white(px(&pm, 17, 50)) && white(px(&pm, 32, 50)));
        // An open line is always centred.
        let pm = page(vec![line(Stroke {
            position: StrokePosition::Inside,
            ..black
        })]);
        assert!(dark(px(&pm, 50, 47)) && dark(px(&pm, 50, 53)));
    }

    #[test]
    fn a_see_through_outline_does_not_double_up_under_its_arrowhead() {
        let half = Stroke {
            opacity: 0.5,
            end_arrow: graphdraw_core::Arrowhead::Square,
            ..Stroke::new(Color::BLACK, 4.0)
        };
        let pm = page(vec![line(half)]);
        let (r, g, b) = px(&pm, 50, 50);
        assert!((115..=140).contains(&r) && r == g && g == b, "{r} {g} {b}");
        // Where the head covers the line's end: the same grey.
        let (r2, _, _) = px(&pm, 88, 50);
        assert!((r2 as i32 - r as i32).abs() <= 4, "{r} {r2}");
        // Transparent: nothing.
        let none = Stroke {
            opacity: 0.0,
            ..Stroke::new(Color::BLACK, 4.0)
        };
        assert!(white(px(&page(vec![line(none)]), 50, 50)));
    }

    #[test]
    fn aligned_dashes_put_a_dash_through_every_corner() {
        // 8 mm dashes and 6 mm gaps (2 mm wide) around a 60 mm square:
        // standard dashes leave a corner bare, aligned and fixed ones
        // cover every corner.
        let mut s = Stroke::new(Color::BLACK, 2.0);
        s.dash = vec![4.0, 3.0];
        let corner = |mode| {
            let st = Stroke {
                dash_corners: mode,
                ..s.clone()
            };
            let pm = page(vec![square(st)]);
            // The four corners (image y down).
            [(20, 20), (80, 20), (20, 80), (80, 80)]
                .iter()
                .filter(|(x, y)| dark(px(&pm, *x, *y)))
                .count()
        };
        assert_eq!(corner(DashCorners::Align), 4);
        assert_eq!(corner(DashCorners::Fixed), 4);
        assert!(corner(DashCorners::Standard) < 4);
    }
}
