//! Área de desenho: renderização, sobreposições (seleção, alças, nós,
//! prévia) e as ferramentas do M1.
//!
//! Modificadores (doc 07): Shift = proporcional/somar à seleção; Ctrl = a
//! partir do centro / restringir eixo / passos de 15°.

use eframe::egui::{
    self, Color32, CursorIcon, Key, PointerButton, Pos2, Rect, Sense, Stroke as EStroke, Vec2,
};
use graphdraw_core::edit::ObjRef;
use graphdraw_core::edit::nodes::{self, Handle, NodeRef};
use graphdraw_core::edit::ops::{self, Anchor};
use graphdraw_core::edit::query;
use graphdraw_core::edit::snap::{self, SnapKind, SnapSettings, Snapped};
use graphdraw_core::geometry::{self, SegRef};
use graphdraw_core::model::{
    EllipseData, Fill, FillRule, Node, NodeKind, Object, ObjectKind, PathData, RectData, Stroke,
    Subpath, Transform,
};
use graphdraw_core::render::raster::{self, RasterOptions};
use kurbo::{Affine, BezPath, Point};
use serde_json::Map;

use crate::doc::DocState;
use crate::i18n::tr;
use crate::view::View;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    Select,
    Shape,
    Zoom,
    Pan,
    Bezier,
    Rect,
    Ellipse,
}

impl Tool {
    pub fn info(self) -> (&'static str, &'static str, &'static str) {
        match self {
            Tool::Select => ("tool-select", tr("Seleção (V)", "Select (V)"), "V"),
            Tool::Shape => (
                "tool-shape",
                tr("Forma — editar nós (N)", "Shape — edit nodes (N)"),
                "N",
            ),
            Tool::Zoom => ("tool-zoom", tr("Zoom (Z)", "Zoom (Z)"), "Z"),
            Tool::Pan => (
                "tool-pan",
                tr(
                    "Mão — mover a vista (H; espaço temporário)",
                    "Hand — pan (H; hold space)",
                ),
                "H",
            ),
            Tool::Bezier => ("tool-bezier", tr("Bézier (B)", "Bézier (B)"), "B"),
            Tool::Rect => ("tool-rect", tr("Retângulo (R)", "Rectangle (R)"), "R"),
            Tool::Ellipse => ("tool-ellipse", tr("Elipse (E)", "Ellipse (E)"), "E"),
        }
    }
    pub const ALL: [Tool; 7] = [
        Tool::Select,
        Tool::Shape,
        Tool::Zoom,
        Tool::Pan,
        Tool::Bezier,
        Tool::Rect,
        Tool::Ellipse,
    ];
}

/// Atributos aplicados aos objetos criados.
#[derive(Clone)]
pub struct Defaults {
    pub fill: Option<Fill>,
    pub stroke: Option<Stroke>,
}

enum Drag {
    None,
    Pan,
    Rubber {
        start: Pos2,
    },
    ZoomRect {
        start: Pos2,
    },
    Move {
        grab: Point,
        start: Point,
        originals: Vec<(ObjRef, Object)>,
        cands: Vec<(Point, SnapKind)>,
    },
    Scale {
        handle: Anchor,
        bounds: kurbo::Rect,
        from_center: bool,
        originals: Vec<(ObjRef, Object)>,
    },
    Rotate {
        center: Point,
        start: Point,
        originals: Vec<(ObjRef, Object)>,
    },
    Skew {
        horizontal: bool,
        edge: f64,
        center: Point,
        start: Point,
        originals: Vec<(ObjRef, Object)>,
    },
    MoveCenter,
    Nodes {
        start: Point,
        loc: ObjRef,
        original: Object,
        cands: Vec<(Point, SnapKind)>,
    },
    NodeHandle {
        node: NodeRef,
        side: Handle,
        loc: ObjRef,
        original: Object,
    },
    Segment {
        seg: SegRef,
        t: f64,
        start: Point,
        loc: ObjRef,
        original: Object,
    },
    NodeRubber {
        start: Pos2,
    },
    DrawRect {
        start: Point,
        ellipse: bool,
        cands: Vec<(Point, SnapKind)>,
    },
    BezierHandle,
}

struct BezierDraft {
    nodes: Vec<Node>,
}

#[derive(Clone, Copy, PartialEq)]
struct RenderKey {
    page: usize,
    rev: u64,
    view: View,
    size: [usize; 2],
    ppp: f32,
}

pub struct CanvasState {
    pub view: View,
    pub fit_pending: bool,
    pub actual_pending: bool,
    pub zoom_to: Option<kurbo::Rect>,
    texture: Option<egui::TextureHandle>,
    rendered: Option<RenderKey>,
    drag: Drag,
    pub rotate_mode: bool,
    pub rot_center: Option<Point>,
    bezier: Option<BezierDraft>,
    pub cursor_page: Option<Point>,
    pub snap_hit: Option<Snapped>,
    pub snap: SnapSettings,
    pub show_grid: bool,
    pub canvas_size: Vec2,
}

impl Default for CanvasState {
    fn default() -> Self {
        CanvasState {
            view: View::default(),
            fit_pending: true,
            actual_pending: false,
            zoom_to: None,
            texture: None,
            rendered: None,
            drag: Drag::None,
            rotate_mode: false,
            rot_center: None,
            bezier: None,
            cursor_page: None,
            snap_hit: None,
            snap: SnapSettings {
                grid: Some(5_000.0),
                ..Default::default()
            },
            show_grid: false,
            canvas_size: Vec2::new(800.0, 600.0),
        }
    }
}

const HANDLE: f32 = 5.0;
const ACCENT: Color32 = Color32::from_rgb(0x1e, 0x88, 0xe5);

fn accent() -> Color32 {
    ACCENT
}

/// Retorno do quadro para a janela principal.
#[derive(Default)]
pub struct CanvasEvents {
    pub switch_tool: Option<Tool>,
    pub status: Option<String>,
}

impl CanvasState {
    /// Novo documento: mantém encaixe e grade, zera vista e estado de arrasto.
    pub fn reset_for_new_document(&mut self) {
        let (snap, grid) = (self.snap, self.show_grid);
        *self = CanvasState::default();
        self.snap = snap;
        self.show_grid = grid;
    }

    pub fn is_dragging(&self) -> bool {
        !matches!(self.drag, Drag::None)
    }

    pub fn has_draft(&self) -> bool {
        self.bezier.is_some()
    }

    /// Encerra o traçado Bézier em andamento (cria o objeto se tiver 2+ nós).
    pub fn finish_bezier(&mut self, doc: &mut DocState, defaults: &Defaults, close: bool) {
        let Some(mut draft) = self.bezier.take() else {
            return;
        };
        // Cliques repetidos no mesmo ponto (duplo clique) não geram nós extras.
        draft.nodes.dedup_by(|b, a| {
            (a.p[0] - b.p[0]).abs() < 1e-6
                && (a.p[1] - b.p[1]).abs() < 1e-6
                && b.handle_out.is_none()
        });
        if draft.nodes.len() < 2 {
            return;
        }
        let mut nodes = draft.nodes;
        nodes[0].handle_in = if close { nodes[0].handle_in } else { None };
        if !close {
            nodes.last_mut().unwrap().handle_out = None;
        }
        let mut o = Object::new(ObjectKind::Path(PathData {
            subpaths: vec![Subpath {
                closed: close,
                nodes,
                extra: Map::new(),
            }],
            fill_rule: FillRule::Evenodd,
            extra: Map::new(),
        }));
        o.fill = if close { defaults.fill.clone() } else { None };
        o.stroke = defaults.stroke.clone();
        if let Some(layer) = doc.layer_for_new_object() {
            let id = ops::add_object(
                &mut doc.editor,
                doc.page,
                layer,
                o,
                tr("Desenhar curva", "Draw curve"),
            );
            doc.selection = vec![id];
        }
    }

    pub fn cancel(&mut self, doc: &mut DocState) {
        if doc.editor.in_transaction() {
            doc.editor.cancel();
        }
        self.drag = Drag::None;
        self.bezier = None;
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        doc: &mut DocState,
        tool: Tool,
        defaults: &Defaults,
    ) -> CanvasEvents {
        let mut ev = CanvasEvents::default();
        let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        self.canvas_size = rect.size();
        let painter = ui.painter_at(rect);
        let desk = if ui.visuals().dark_mode {
            Color32::from_gray(48)
        } else {
            Color32::from_gray(205)
        };
        painter.rect_filled(rect, 0.0, desk);

        let (pw, ph) = doc.page_size();
        let size = rect.size();
        if self.fit_pending {
            self.view = View::fit(pw, ph, size.x as f64, size.y as f64);
            self.fit_pending = false;
        }
        if self.actual_pending {
            self.view = View::actual_size(pw, ph, size.x as f64, size.y as f64);
            self.actual_pending = false;
        }
        if let Some(r) = self.zoom_to.take() {
            self.view = fit_rect(r, ph, size);
        }

        let view = self.view;
        let to_page = |pos: Pos2| -> Point {
            let l = pos - rect.min;
            let (x, y) = view.screen_to_page(ph, l.x as f64, l.y as f64);
            Point::new(x, y)
        };

        // ---- navegação comum a todas as ferramentas ----
        let (space, mods, scroll, pinch, hover) = ui.input(|i| {
            (
                i.key_down(Key::Space),
                i.modifiers,
                i.smooth_scroll_delta.y,
                i.zoom_delta(),
                i.pointer.hover_pos(),
            )
        });
        let panning_tool = tool == Tool::Pan || space;
        if let Some(pos) = response.hover_pos() {
            let local = pos - rect.min;
            let factor = pinch as f64 * (scroll as f64 * 0.0025).exp();
            if (factor - 1.0).abs() > 1e-4 {
                self.view.zoom_at(factor, (local.x as f64, local.y as f64));
            }
        }
        if response.dragged_by(PointerButton::Middle)
            || (matches!(self.drag, Drag::Pan) && response.dragged())
        {
            let d = response.drag_delta();
            self.view.pan_by(d.x as f64, d.y as f64);
        }
        self.cursor_page = hover.filter(|p| rect.contains(*p)).map(to_page);
        let tol = 5.0 / self.view.zoom; // 5 px em µm
        // Página bloqueada (GD-PGT-18): só zoom e mão funcionam.
        let blocked = doc.page_locked() && !matches!(tool, Tool::Zoom | Tool::Pan);
        let locked_msg = || {
            tr(
                "Página bloqueada: desbloqueie pelo menu da aba para editar",
                "Page locked: unlock it from the tab menu to edit",
            )
            .to_string()
        };

        // ---- ferramentas ----
        if response.drag_started_by(PointerButton::Primary) {
            let origin = ui
                .input(|i| i.pointer.press_origin())
                .unwrap_or(rect.center());
            if panning_tool {
                self.drag = Drag::Pan;
            } else if blocked {
                ev.status = Some(locked_msg());
            } else {
                self.start_drag(
                    tool,
                    doc,
                    to_page(origin),
                    origin,
                    mods,
                    tol,
                    rect,
                    defaults,
                );
            }
        }
        if response.dragged_by(PointerButton::Primary)
            && !matches!(self.drag, Drag::Pan)
            && let Some(cur) = response.interact_pointer_pos()
        {
            self.update_drag(doc, to_page(cur), mods, tol);
        }
        if response.drag_stopped() {
            let end = response
                .interact_pointer_pos()
                .or(hover)
                .unwrap_or(rect.center());
            self.end_drag(doc, to_page(end), end, mods, rect, ph, defaults);
        }
        if response.clicked() && blocked {
            ev.status = Some(locked_msg());
        }
        if response.clicked()
            && !panning_tool
            && !blocked
            && let Some(pos) = response.interact_pointer_pos()
        {
            self.click(
                tool,
                doc,
                to_page(pos),
                pos,
                mods,
                tol,
                rect,
                defaults,
                &mut ev,
            );
        }
        if response.secondary_clicked()
            && tool == Tool::Zoom
            && let Some(pos) = response.interact_pointer_pos()
        {
            let l = pos - rect.min;
            self.view.zoom_at(0.5, (l.x as f64, l.y as f64));
        }
        if response.double_clicked()
            && !panning_tool
            && !blocked
            && let Some(pos) = response.interact_pointer_pos()
        {
            self.double_click(tool, doc, to_page(pos), tol, defaults, &mut ev);
        }
        if tool == Tool::Bezier
            && let (Some(_), Some(_)) = (&self.bezier, self.cursor_page)
        {
            ui.ctx().request_repaint();
        }
        // Encaixe sob o cursor (indicação visual, GD-SNP-02) nas ferramentas de desenho.
        if !self.is_dragging() && matches!(tool, Tool::Rect | Tool::Ellipse | Tool::Bezier) {
            self.snap_hit = self.cursor_page.and_then(|p| {
                let c = snap::candidates(doc.page(), &self.snap, &[]);
                snap::snap(p, &c, &self.snap, 8.0 / self.view.zoom)
            });
        }

        // Cursor.
        let cursor = if panning_tool || matches!(self.drag, Drag::Pan) {
            if self.is_dragging() {
                CursorIcon::Grabbing
            } else {
                CursorIcon::Grab
            }
        } else {
            match tool {
                Tool::Zoom => CursorIcon::ZoomIn,
                Tool::Bezier | Tool::Rect | Tool::Ellipse => CursorIcon::Crosshair,
                Tool::Shape => CursorIcon::Default,
                _ => match self.drag {
                    Drag::Move { .. } => CursorIcon::Move,
                    _ => CursorIcon::Default,
                },
            }
        };
        if response.hovered() {
            ui.ctx().set_cursor_icon(cursor);
        }

        // ---- desenho ----
        self.paint_page(ui, &painter, doc, rect);
        self.paint_overlays(ui, &painter, doc, rect, tool);
        ev
    }

    fn paint_page(
        &mut self,
        ui: &egui::Ui,
        painter: &egui::Painter,
        doc: &mut DocState,
        rect: Rect,
    ) {
        let (pw, ph) = doc.page_size();
        let v = self.view;
        let s = |x: f64, y: f64| {
            let (a, b) = v.page_to_screen(ph, x, y);
            rect.min + Vec2::new(a as f32, b as f32)
        };
        let page_rect = Rect::from_min_max(s(0.0, ph), s(pw, 0.0));
        painter.rect_filled(
            page_rect.translate(Vec2::new(3.0, 3.0)),
            0.0,
            Color32::from_black_alpha(60),
        );
        painter.rect_filled(page_rect, 0.0, Color32::WHITE);

        let ppp = ui.ctx().pixels_per_point();
        let size = rect.size();
        let px = [
            (size.x * ppp).round().max(1.0) as usize,
            (size.y * ppp).round().max(1.0) as usize,
        ];
        let key = RenderKey {
            page: doc.page,
            rev: doc.editor.revision(),
            view: v,
            size: px,
            ppp,
        };
        let panning = matches!(self.drag, Drag::Pan)
            && self.rendered.is_some_and(|r| {
                r.view.zoom == v.zoom && r.size == px && r.rev == key.rev && r.page == key.page
            });
        if self.rendered != Some(key) && !panning {
            let scene = doc.scene();
            if let Some(pm) = raster::render(
                &scene,
                v.raster_affine(ph, ppp as f64),
                px[0] as u32,
                px[1] as u32,
                &RasterOptions::default(),
            ) {
                let image = egui::ColorImage::from_rgba_premultiplied(px, pm.data());
                match &mut self.texture {
                    Some(t) => t.set(image, egui::TextureOptions::LINEAR),
                    None => {
                        self.texture = Some(ui.ctx().load_texture(
                            "pagina",
                            image,
                            egui::TextureOptions::LINEAR,
                        ))
                    }
                }
                self.rendered = Some(key);
            }
        }
        if let (Some(tex), Some(r)) = (&self.texture, self.rendered)
            && r.page == doc.page
        {
            let shift = Vec2::new(
                (v.pan.0 - r.view.pan.0) as f32,
                (v.pan.1 - r.view.pan.1) as f32,
            );
            let img = Rect::from_min_size(
                rect.min + shift,
                Vec2::new(r.size[0] as f32 / ppp, r.size[1] as f32 / ppp),
            );
            painter.image(
                tex.id(),
                img,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
        }
        painter.rect_stroke(
            page_rect,
            0.0,
            EStroke::new(1.0, Color32::from_gray(140)),
            egui::StrokeKind::Outside,
        );

        // Grade (quando ligada e legível).
        if self.show_grid
            && let Some(g) = self.snap.grid.filter(|g| *g * v.zoom >= 6.0)
        {
            let stroke = EStroke::new(1.0, Color32::from_rgba_unmultiplied(30, 136, 229, 40));
            let mut x = 0.0;
            while x <= pw {
                painter.line_segment([s(x, 0.0), s(x, ph)], stroke);
                x += g;
            }
            let mut y = 0.0;
            while y <= ph {
                painter.line_segment([s(0.0, y), s(pw, y)], stroke);
                y += g;
            }
        }
    }

    fn screen(&self, rect: Rect, ph: f64, p: Point) -> Pos2 {
        let (a, b) = self.view.page_to_screen(ph, p.x, p.y);
        rect.min + Vec2::new(a as f32, b as f32)
    }

    fn screen_rect(&self, rect: Rect, ph: f64, r: kurbo::Rect) -> Rect {
        Rect::from_two_pos(
            self.screen(rect, ph, Point::new(r.x0, r.y0)),
            self.screen(rect, ph, Point::new(r.x1, r.y1)),
        )
    }

    fn handle_rects(&self, rect: Rect, ph: f64, b: kurbo::Rect) -> Vec<(Anchor, Pos2)> {
        Anchor::ALL
            .into_iter()
            .filter(|a| *a != Anchor::Center)
            .map(|a| (a, self.screen(rect, ph, a.point(b))))
            .collect()
    }

    fn rotation_center(&self, b: kurbo::Rect) -> Point {
        self.rot_center.unwrap_or(b.center())
    }

    fn paint_overlays(
        &self,
        ui: &egui::Ui,
        painter: &egui::Painter,
        doc: &DocState,
        rect: Rect,
        tool: Tool,
    ) {
        let ph = doc.page_size().1;
        let thin = EStroke::new(1.0, accent());
        // Contorno fino de cada objeto selecionado + caixa da seleção com alças.
        for r in ops::refs(&doc.editor, &doc.selection) {
            if let Some(b) = ops::object_bounds(&doc.editor, &r, false) {
                painter.rect_stroke(
                    self.screen_rect(rect, ph, b),
                    0.0,
                    EStroke::new(1.0, Color32::from_rgba_unmultiplied(30, 136, 229, 110)),
                    egui::StrokeKind::Middle,
                );
            }
        }
        if tool == Tool::Select
            && let Some(b) = ops::selection_bounds(&doc.editor, &doc.selection, false)
        {
            let sr = self.screen_rect(rect, ph, b);
            painter.rect_stroke(sr, 0.0, thin, egui::StrokeKind::Middle);
            if self.rotate_mode {
                for (a, p) in self.handle_rects(rect, ph, b) {
                    let corner = matches!(
                        a,
                        Anchor::TopLeft
                            | Anchor::TopRight
                            | Anchor::BottomLeft
                            | Anchor::BottomRight
                    );
                    if corner {
                        painter.circle(p, HANDLE, Color32::WHITE, thin);
                    } else {
                        let d = HANDLE;
                        let pts = vec![
                            p + Vec2::new(0.0, -d),
                            p + Vec2::new(d, 0.0),
                            p + Vec2::new(0.0, d),
                            p + Vec2::new(-d, 0.0),
                        ];
                        painter.add(egui::Shape::convex_polygon(pts, Color32::WHITE, thin));
                    }
                }
                let c = self.screen(rect, ph, self.rotation_center(b));
                painter.circle_stroke(c, 6.0, thin);
                painter.line_segment([c - Vec2::new(9.0, 0.0), c + Vec2::new(9.0, 0.0)], thin);
                painter.line_segment([c - Vec2::new(0.0, 9.0), c + Vec2::new(0.0, 9.0)], thin);
            } else {
                for (_, p) in self.handle_rects(rect, ph, b) {
                    let r = Rect::from_center_size(p, Vec2::splat(HANDLE * 2.0));
                    painter.rect_filled(r, 1.0, Color32::WHITE);
                    painter.rect_stroke(r, 1.0, thin, egui::StrokeKind::Middle);
                }
            }
        }
        if tool == Tool::Shape {
            self.paint_nodes(painter, doc, rect, ph);
        }
        // Retângulos elásticos e prévias.
        let hover = ui.input(|i| i.pointer.hover_pos());
        match &self.drag {
            Drag::Rubber { start } | Drag::NodeRubber { start } | Drag::ZoomRect { start } => {
                if let Some(h) = hover {
                    let r = Rect::from_two_pos(*start, h);
                    painter.rect_filled(r, 0.0, Color32::from_rgba_unmultiplied(30, 136, 229, 25));
                    painter.rect_stroke(r, 0.0, thin, egui::StrokeKind::Middle);
                }
            }
            Drag::DrawRect { start, ellipse, .. } => {
                if let Some(cur) = self.cursor_page {
                    let mods = ui.input(|i| i.modifiers);
                    let r = draw_rect(*start, self.snap_hit.map_or(cur, |s| s.point), mods);
                    let sr = self.screen_rect(rect, ph, r);
                    if *ellipse {
                        painter.add(egui::Shape::ellipse_stroke(
                            sr.center(),
                            sr.size() / 2.0,
                            thin,
                        ));
                    } else {
                        painter.rect_stroke(sr, 0.0, thin, egui::StrokeKind::Middle);
                    }
                }
            }
            _ => {}
        }
        if let Some(d) = &self.bezier {
            let mut path = BezPath::new();
            let pts: Vec<Point> = d.nodes.iter().map(|n| Point::new(n.p[0], n.p[1])).collect();
            path.move_to(pts[0]);
            for w in d.nodes.windows(2) {
                let (a, b) = (&w[0], &w[1]);
                path.curve_to(
                    pt(a.handle_out.unwrap_or(a.p)),
                    pt(b.handle_in.unwrap_or(b.p)),
                    pt(b.p),
                );
            }
            if let (Some(cur), Some(last)) = (self.cursor_page, d.nodes.last())
                && !matches!(self.drag, Drag::BezierHandle)
            {
                let cur = self.snap_hit.map_or(cur, |s| s.point);
                path.curve_to(pt(last.handle_out.unwrap_or(last.p)), cur, cur);
            }
            self.stroke_bez(painter, rect, ph, &path, thin);
            for n in &d.nodes {
                let p = self.screen(rect, ph, pt(n.p));
                painter.rect_filled(Rect::from_center_size(p, Vec2::splat(7.0)), 1.0, accent());
                for h in [n.handle_in, n.handle_out].into_iter().flatten() {
                    let hp = self.screen(rect, ph, pt(h));
                    painter.line_segment([p, hp], thin);
                    painter.circle_filled(hp, 3.0, accent());
                }
            }
        }
        if let Some(s) = self.snap_hit {
            let p = self.screen(rect, ph, s.point);
            painter.circle_stroke(
                p,
                6.0,
                EStroke::new(2.0, Color32::from_rgb(0xe5, 0x39, 0x35)),
            );
            painter.text(
                p + Vec2::new(9.0, -9.0),
                egui::Align2::LEFT_BOTTOM,
                s.kind.label(),
                egui::FontId::proportional(11.0),
                Color32::from_rgb(0xe5, 0x39, 0x35),
            );
        }
    }

    fn stroke_bez(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        ph: f64,
        path: &BezPath,
        stroke: EStroke,
    ) {
        let mut pts: Vec<Pos2> = Vec::new();
        let flush = |pts: &mut Vec<Pos2>| {
            if pts.len() > 1 {
                painter.add(egui::Shape::line(std::mem::take(pts), stroke));
            }
            pts.clear();
        };
        kurbo::flatten(
            path.elements().iter().copied(),
            0.2 / self.view.zoom,
            |el| match el {
                kurbo::PathEl::MoveTo(p) => {
                    flush(&mut pts);
                    pts.push(self.screen(rect, ph, p));
                }
                kurbo::PathEl::LineTo(p) => pts.push(self.screen(rect, ph, p)),
                kurbo::PathEl::ClosePath => {
                    if let Some(f) = pts.first().copied() {
                        pts.push(f);
                    }
                }
                _ => {}
            },
        );
        flush(&mut pts);
    }

    fn paint_nodes(&self, painter: &egui::Painter, doc: &DocState, rect: Rect, ph: f64) {
        let Some(PathSel { path, m, .. }) = single_path(doc) else {
            return;
        };
        let mut bez = graphdraw_core::model::geom::subpaths_to_bez(&path.subpaths);
        bez.apply_affine(m);
        self.stroke_bez(painter, rect, ph, &bez, EStroke::new(1.0, accent()));
        let thin = EStroke::new(1.0, accent());
        for r in nodes::all_nodes(path) {
            let n = nodes::node(path, r).unwrap();
            let p = self.screen(rect, ph, m * pt(n.p));
            let selected = doc.nodes.contains(&r);
            if selected {
                for h in [n.handle_in, n.handle_out].into_iter().flatten() {
                    let hp = self.screen(rect, ph, m * pt(h));
                    painter.line_segment([p, hp], thin);
                    painter.circle(hp, 3.5, Color32::WHITE, thin);
                }
            }
            let r = Rect::from_center_size(p, Vec2::splat(8.0));
            let fill = if selected { accent() } else { Color32::WHITE };
            match n.kind {
                NodeKind::Corner => {
                    painter.rect_filled(r, 0.0, fill);
                    painter.rect_stroke(r, 0.0, thin, egui::StrokeKind::Middle);
                }
                _ => {
                    painter.circle(p, 4.5, fill, thin);
                }
            }
        }
    }

    // ---- interação --------------------------------------------------------

    #[allow(clippy::too_many_arguments)]
    fn start_drag(
        &mut self,
        tool: Tool,
        doc: &mut DocState,
        p: Point,
        origin: Pos2,
        mods: egui::Modifiers,
        tol: f64,
        rect: Rect,
        _defaults: &Defaults,
    ) {
        let ph = doc.page_size().1;
        match tool {
            Tool::Pan => self.drag = Drag::Pan,
            Tool::Zoom => self.drag = Drag::ZoomRect { start: origin },
            Tool::Rect | Tool::Ellipse => {
                let cands = snap::candidates(doc.page(), &self.snap, &[]);
                let start =
                    snap::snap(p, &cands, &self.snap, 8.0 / self.view.zoom).map_or(p, |s| s.point);
                self.drag = Drag::DrawRect {
                    start,
                    ellipse: tool == Tool::Ellipse,
                    cands,
                };
            }
            Tool::Bezier => {
                if self.bezier_press(doc, p, origin, rect, ph) {
                    self.finish_bezier(doc, _defaults, true);
                } else {
                    self.drag = Drag::BezierHandle;
                }
            }
            Tool::Select => self.start_select_drag(doc, p, origin, mods, tol, rect, ph),
            Tool::Shape => self.start_shape_drag(doc, p, origin, mods, tol, rect, ph),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn start_select_drag(
        &mut self,
        doc: &mut DocState,
        p: Point,
        origin: Pos2,
        mods: egui::Modifiers,
        tol: f64,
        rect: Rect,
        ph: f64,
    ) {
        if let Some(b) = ops::selection_bounds(&doc.editor, &doc.selection, false) {
            let near = |q: Pos2| (q - origin).length() <= HANDLE + 3.0;
            if self.rotate_mode {
                let c = self.rotation_center(b);
                if near(self.screen(rect, ph, c)) {
                    self.drag = Drag::MoveCenter;
                    return;
                }
                for (a, hp) in self.handle_rects(rect, ph, b) {
                    if near(hp) {
                        let originals = originals(doc);
                        let n = originals.len();
                        let corner = matches!(
                            a,
                            Anchor::TopLeft
                                | Anchor::TopRight
                                | Anchor::BottomLeft
                                | Anchor::BottomRight
                        );
                        if corner {
                            doc.editor
                                .begin(format!("{} {}", tr("Girar", "Rotate"), count(n)));
                            self.drag = Drag::Rotate {
                                center: c,
                                start: p,
                                originals,
                            };
                        } else {
                            let horizontal = matches!(a, Anchor::Top | Anchor::Bottom);
                            let edge = if horizontal {
                                a.point(b).y
                            } else {
                                a.point(b).x
                            };
                            doc.editor
                                .begin(format!("{} {}", tr("Inclinar", "Skew"), count(n)));
                            self.drag = Drag::Skew {
                                horizontal,
                                edge,
                                center: c,
                                start: p,
                                originals,
                            };
                        }
                        return;
                    }
                }
            } else {
                for (a, hp) in self.handle_rects(rect, ph, b) {
                    if near(hp) {
                        let originals = originals(doc);
                        doc.editor.begin(format!(
                            "{} {}",
                            tr("Redimensionar", "Resize"),
                            count(originals.len())
                        ));
                        self.drag = Drag::Scale {
                            handle: a,
                            bounds: b,
                            from_center: mods.ctrl,
                            originals,
                        };
                        return;
                    }
                }
            }
        }
        match pick(doc, p, tol, mods.ctrl) {
            Some(id) => {
                if !doc.selection.contains(&id) {
                    if mods.shift {
                        doc.selection.push(id);
                    } else {
                        doc.selection = vec![id];
                        self.rotate_mode = false;
                        self.rot_center = None;
                    }
                }
                let originals = originals(doc);
                let cands = snap::candidates(doc.page(), &self.snap, &doc.selection);
                // Ponto de pega: canto/centro mais próximo da seleção, para encaixar bordas.
                let b = ops::selection_bounds(&doc.editor, &doc.selection, false)
                    .unwrap_or(kurbo::Rect::from_points(p, p));
                let grab = Anchor::ALL
                    .iter()
                    .map(|a| a.point(b))
                    .min_by(|x, y| (*x - p).hypot().total_cmp(&(*y - p).hypot()))
                    .unwrap_or(p);
                let grab = if (grab - p).hypot() * self.view.zoom < 12.0 {
                    grab
                } else {
                    p
                };
                doc.editor.begin(format!(
                    "{} {}",
                    tr("Mover", "Move"),
                    count(originals.len())
                ));
                self.drag = Drag::Move {
                    grab,
                    start: p,
                    originals,
                    cands,
                };
            }
            None => {
                if !mods.shift {
                    doc.selection.clear();
                    self.rotate_mode = false;
                }
                self.drag = Drag::Rubber { start: origin };
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn start_shape_drag(
        &mut self,
        doc: &mut DocState,
        p: Point,
        origin: Pos2,
        mods: egui::Modifiers,
        tol: f64,
        rect: Rect,
        ph: f64,
    ) {
        let Some((loc, o, path, m)) =
            single_path(doc).map(|s| (s.r, s.obj.clone(), s.path.clone(), s.m))
        else {
            self.drag = Drag::Rubber { start: origin };
            return;
        };
        let near = |q: Point| (self.screen(rect, ph, m * q) - origin).length() <= 6.0;
        // Alças dos nós selecionados.
        for r in doc.nodes.clone() {
            if let Some(n) = nodes::node(&path, r) {
                for (h, side) in [(n.handle_in, Handle::In), (n.handle_out, Handle::Out)] {
                    if h.is_some_and(|h| near(pt(h))) {
                        doc.editor.begin(tr("Mover alça", "Move handle"));
                        self.drag = Drag::NodeHandle {
                            node: r,
                            side,
                            loc,
                            original: o,
                        };
                        return;
                    }
                }
            }
        }
        // Nós.
        if let Some(r) = nodes::all_nodes(&path)
            .into_iter()
            .find(|r| near(pt(nodes::node(&path, *r).unwrap().p)))
        {
            if !doc.nodes.contains(&r) {
                if !mods.shift {
                    doc.nodes.clear();
                }
                doc.nodes.push(r);
            }
            let cands = snap::candidates(doc.page(), &self.snap, std::slice::from_ref(&o.id));
            doc.editor.begin(format!(
                "{} {}",
                tr("Mover", "Move"),
                plural_nodes(doc.nodes.len())
            ));
            self.drag = Drag::Nodes {
                start: p,
                loc,
                original: o,
                cands,
            };
            return;
        }
        // Segmento: arrastar deforma mantendo os extremos (GD-NOD-02).
        let lp = m.inverse() * p;
        let scale = m.determinant().abs().sqrt().max(1e-12);
        if let Some(n) = geometry::nearest(&path, lp).filter(|n| n.distance * scale <= tol) {
            doc.editor.begin(tr("Deformar segmento", "Reshape segment"));
            self.drag = Drag::Segment {
                seg: n.seg,
                t: n.t,
                start: p,
                loc,
                original: o,
            };
            return;
        }
        if !mods.shift {
            doc.nodes.clear();
        }
        self.drag = Drag::NodeRubber { start: origin };
    }

    fn update_drag(&mut self, doc: &mut DocState, cur: Point, mods: egui::Modifiers, _tol: f64) {
        let radius = 8.0 / self.view.zoom;
        self.snap_hit = None;
        match &self.drag {
            Drag::Move {
                grab,
                start,
                originals,
                cands,
            } => {
                let mut d = cur - *start;
                if mods.ctrl {
                    if d.x.abs() > d.y.abs() {
                        d.y = 0.0
                    } else {
                        d.x = 0.0
                    }
                }
                if let Some(s) = snap::snap(*grab + d, cands, &self.snap, radius) {
                    d = s.point - *grab;
                    self.snap_hit = Some(s);
                }
                let m = Affine::translate(d);
                apply_live(doc, originals, m);
            }
            Drag::Scale {
                handle,
                bounds,
                from_center,
                originals,
            } => {
                let fixed = if *from_center || mods.ctrl {
                    bounds.center()
                } else {
                    handle.opposite().point(*bounds)
                };
                let hp = handle.point(*bounds);
                let (fx, fy) = handle.fraction();
                let mut sx = if fx != 0.5 && (hp.x - fixed.x).abs() > 1e-9 {
                    (cur.x - fixed.x) / (hp.x - fixed.x)
                } else {
                    1.0
                };
                let mut sy = if fy != 0.5 && (hp.y - fixed.y).abs() > 1e-9 {
                    (cur.y - fixed.y) / (hp.y - fixed.y)
                } else {
                    1.0
                };
                let corner = fx != 0.5 && fy != 0.5;
                if corner && !mods.shift {
                    let s = if sx.abs() > sy.abs() { sx } else { sy };
                    sx = s.abs() * sx.signum();
                    sy = s.abs() * sy.signum();
                }
                let clamp = |v: f64| {
                    if v.abs() < 1e-3 {
                        1e-3f64.copysign(v)
                    } else {
                        v
                    }
                };
                apply_live(
                    doc,
                    originals,
                    ops::scale_about(fixed, clamp(sx), clamp(sy)),
                );
            }
            Drag::Rotate {
                center,
                start,
                originals,
            } => {
                let a0 = (*start - *center).atan2();
                let a1 = (cur - *center).atan2();
                let mut deg = (a1 - a0).to_degrees();
                if mods.ctrl {
                    deg = (deg / 15.0).round() * 15.0;
                }
                apply_live(doc, originals, ops::rotate_about(*center, deg));
            }
            Drag::Skew {
                horizontal,
                edge,
                center,
                start,
                originals,
            } => {
                let m = if *horizontal {
                    let k = (cur.x - start.x) / (edge - center.y).abs().max(1e-9)
                        * (edge - center.y).signum();
                    Affine::translate(center.to_vec2())
                        * Affine::new([1.0, 0.0, k, 1.0, 0.0, 0.0])
                        * Affine::translate(-center.to_vec2())
                } else {
                    let k = (cur.y - start.y) / (edge - center.x).abs().max(1e-9)
                        * (edge - center.x).signum();
                    Affine::translate(center.to_vec2())
                        * Affine::new([1.0, k, 0.0, 1.0, 0.0, 0.0])
                        * Affine::translate(-center.to_vec2())
                };
                apply_live(doc, originals, m);
            }
            Drag::MoveCenter => self.rot_center = Some(cur),
            Drag::Nodes {
                start,
                loc,
                original,
                cands,
            } => {
                let m = doc.editor.parent_affine(loc) * original.transform.affine();
                let mut target = cur;
                // Encaixa o nó agarrado (o primeiro selecionado).
                if let (Some(r), ObjectKind::Path(p)) = (doc.nodes.first(), &original.kind)
                    && let Some(n) = nodes::node(p, *r)
                {
                    let grabbed = m * pt(n.p);
                    if let Some(s) = snap::snap(grabbed + (cur - *start), cands, &self.snap, radius)
                    {
                        target = *start + (s.point - grabbed);
                        self.snap_hit = Some(s);
                    }
                }
                let ld = m.inverse() * target - m.inverse() * *start;
                let mut o = original.clone();
                if let ObjectKind::Path(p) = &mut o.kind {
                    nodes::move_nodes(p, &doc.nodes, [ld.x, ld.y]);
                }
                doc.editor.replace_at(loc, o);
            }
            Drag::NodeHandle {
                node,
                side,
                loc,
                original,
            } => {
                let lp =
                    (doc.editor.parent_affine(loc) * original.transform.affine()).inverse() * cur;
                let mut o = original.clone();
                if let ObjectKind::Path(p) = &mut o.kind {
                    nodes::move_handle(p, *node, *side, [lp.x, lp.y]);
                }
                doc.editor.replace_at(loc, o);
            }
            Drag::Segment {
                seg,
                t,
                start,
                loc,
                original,
            } => {
                let inv = (doc.editor.parent_affine(loc) * original.transform.affine()).inverse();
                let d = inv * cur - inv * *start;
                let mut o = original.clone();
                if let ObjectKind::Path(p) = &mut o.kind {
                    nodes::drag_segment(p, *seg, *t, [d.x, d.y]);
                }
                doc.editor.replace_at(loc, o);
            }
            Drag::DrawRect { cands, .. } => {
                self.snap_hit = snap::snap(cur, cands, &self.snap, radius);
            }
            Drag::BezierHandle => {
                if let Some(d) = &mut self.bezier
                    && let Some(n) = d.nodes.last_mut()
                {
                    let p = pt(n.p);
                    if (cur - p).hypot() * self.view.zoom > 2.0 {
                        n.handle_out = Some([cur.x, cur.y]);
                        let mirror = p + (p - cur);
                        n.handle_in = Some([mirror.x, mirror.y]);
                        n.kind = NodeKind::Symmetric;
                    }
                }
            }
            _ => {}
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn end_drag(
        &mut self,
        doc: &mut DocState,
        cur: Point,
        end: Pos2,
        mods: egui::Modifiers,
        rect: Rect,
        ph: f64,
        defaults: &Defaults,
    ) {
        let drag = std::mem::replace(&mut self.drag, Drag::None);
        match drag {
            Drag::Rubber { start } => {
                let a = self.page_of(rect, ph, start);
                let r = kurbo::Rect::from_points(a, cur);
                let hits = query::objects_in_rect(doc.page(), r);
                let ids: Vec<String> = hits
                    .iter()
                    .map(|h| doc.page().layers[h.layer].objects[h.index].id.clone())
                    .collect();
                if mods.shift {
                    for id in ids {
                        if !doc.selection.contains(&id) {
                            doc.selection.push(id);
                        }
                    }
                } else {
                    doc.selection = ids;
                }
                self.rotate_mode = false;
            }
            Drag::NodeRubber { start } => {
                if let Some(PathSel { path, m, .. }) = single_path(doc) {
                    let inv = m.inverse();
                    let a = self.page_of(rect, ph, start);
                    let r = inv.transform_rect_bbox(kurbo::Rect::from_points(a, cur));
                    let found = nodes::nodes_in_rect(path, r);
                    if !mods.shift {
                        doc.nodes.clear();
                    }
                    doc.nodes.extend(found);
                    doc.nodes.sort();
                    doc.nodes.dedup();
                }
            }
            Drag::ZoomRect { start } => {
                let a = self.page_of(rect, ph, start);
                let r = kurbo::Rect::from_points(a, cur);
                if (end - start).length() > 4.0 {
                    self.zoom_to = Some(r);
                } else {
                    let l = end - rect.min;
                    self.view.zoom_at(2.0, (l.x as f64, l.y as f64));
                }
            }
            Drag::DrawRect { start, ellipse, .. } => {
                let endp = self.snap_hit.map_or(cur, |s| s.point);
                let r = draw_rect(start, endp, mods);
                if r.width() * self.view.zoom >= 2.0 && r.height() * self.view.zoom >= 2.0 {
                    let kind = if ellipse {
                        ObjectKind::Ellipse(EllipseData {
                            rx: r.width() / 2.0,
                            ry: r.height() / 2.0,
                            mode: "ellipse".into(),
                            start_angle: 0.0,
                            end_angle: 360.0,
                            extra: Map::new(),
                        })
                    } else {
                        ObjectKind::Rectangle(RectData {
                            width: r.width(),
                            height: r.height(),
                            corners: None,
                            extra: Map::new(),
                        })
                    };
                    let mut o = Object::new(kind);
                    o.transform = if ellipse {
                        Transform::translate(r.center().x, r.center().y)
                    } else {
                        Transform::translate(r.x0, r.y0)
                    };
                    o.fill = defaults.fill.clone();
                    o.stroke = defaults.stroke.clone();
                    if let Some(layer) = doc.layer_for_new_object() {
                        let name = if ellipse {
                            tr("Criar elipse", "Create ellipse")
                        } else {
                            tr("Criar retângulo", "Create rectangle")
                        };
                        let id = ops::add_object(&mut doc.editor, doc.page, layer, o, name);
                        doc.selection = vec![id];
                    }
                }
            }
            Drag::Move { .. }
            | Drag::Scale { .. }
            | Drag::Rotate { .. }
            | Drag::Skew { .. }
            | Drag::Nodes { .. }
            | Drag::NodeHandle { .. }
            | Drag::Segment { .. } => {
                doc.editor.commit();
            }
            Drag::BezierHandle | Drag::Pan | Drag::MoveCenter | Drag::None => {}
        }
        self.snap_hit = None;
    }

    fn page_of(&self, rect: Rect, ph: f64, pos: Pos2) -> Point {
        let l = pos - rect.min;
        let (x, y) = self.view.screen_to_page(ph, l.x as f64, l.y as f64);
        Point::new(x, y)
    }

    /// Adiciona um nó ao traçado; devolve `true` se o clique foi no primeiro
    /// nó (pedido para fechar o caminho).
    fn bezier_press(
        &mut self,
        doc: &mut DocState,
        p: Point,
        origin: Pos2,
        rect: Rect,
        ph: f64,
    ) -> bool {
        let p = snap::snap(
            p,
            &snap::candidates(doc.page(), &self.snap, &[]),
            &self.snap,
            8.0 / self.view.zoom,
        )
        .map_or(p, |s| s.point);
        if let Some(d) = &self.bezier
            && d.nodes.len() >= 2
            && (self.screen(rect, ph, pt(d.nodes[0].p)) - origin).length() <= 7.0
        {
            return true;
        }
        let draft = self.bezier.get_or_insert(BezierDraft { nodes: Vec::new() });
        draft.nodes.push(Node::corner(p.x, p.y));
        false
    }

    #[allow(clippy::too_many_arguments)]
    fn click(
        &mut self,
        tool: Tool,
        doc: &mut DocState,
        p: Point,
        pos: Pos2,
        mods: egui::Modifiers,
        tol: f64,
        rect: Rect,
        defaults: &Defaults,
        ev: &mut CanvasEvents,
    ) {
        let ph = doc.page_size().1;
        match tool {
            Tool::Select => match pick(doc, p, tol, mods.ctrl) {
                Some(id) => {
                    if mods.shift {
                        if let Some(i) = doc.selection.iter().position(|s| *s == id) {
                            doc.selection.remove(i);
                        } else {
                            doc.selection.push(id);
                        }
                    } else if doc.selection.contains(&id) {
                        // Segundo clique alterna alças de rotação/inclinação (GD-FER-01).
                        self.rotate_mode = !self.rotate_mode;
                        self.rot_center = None;
                    } else {
                        doc.selection = vec![id];
                        self.rotate_mode = false;
                        self.rot_center = None;
                    }
                }
                None => {
                    if !mods.shift {
                        doc.selection.clear();
                        self.rotate_mode = false;
                    }
                }
            },
            Tool::Shape => {
                if let Some((path, m)) = single_path(doc).map(|s| (s.path.clone(), s.m))
                    && let Some(r) = nodes::all_nodes(&path).into_iter().find(|r| {
                        (self.screen(rect, ph, m * pt(nodes::node(&path, *r).unwrap().p)) - pos)
                            .length()
                            <= 6.0
                    }) {
                        if mods.shift {
                            if let Some(i) = doc.nodes.iter().position(|n| *n == r) {
                                doc.nodes.remove(i);
                            } else {
                                doc.nodes.push(r);
                            }
                        } else {
                            doc.nodes = vec![r];
                        }
                        return;
                    }
                // Clique em outro objeto o seleciona (descendo nos grupos: a
                // ferramenta Forma edita o caminho, não o grupo).
                match pick(doc, p, tol, true) {
                    Some(id) => {
                        if doc.selection != [id.clone()] {
                            doc.selection = vec![id];
                            doc.nodes.clear();
                        } else if !mods.shift {
                            doc.nodes.clear();
                        }
                    }
                    None => {
                        doc.nodes.clear();
                    }
                }
            }
            Tool::Zoom => {
                let l = pos - rect.min;
                self.view.zoom_at(2.0, (l.x as f64, l.y as f64));
            }
            Tool::Bezier
                // Clique simples (sem arrasto) = nó de canto; no primeiro nó, fecha.
                if self.bezier_press(doc, p, pos, rect, ph) => {
                    self.finish_bezier(doc, defaults, true);
                }
            _ => {}
        }
        let _ = ev;
    }

    fn double_click(
        &mut self,
        tool: Tool,
        doc: &mut DocState,
        p: Point,
        tol: f64,
        defaults: &Defaults,
        ev: &mut CanvasEvents,
    ) {
        match tool {
            Tool::Select => {
                if let Some(o) = pick(doc, p, tol, true)
                    .and_then(|id| doc.editor.find(&id))
                    .and_then(|r| doc.editor.get(&r))
                    && matches!(
                        o.kind,
                        ObjectKind::Path(_) | ObjectKind::Rectangle(_) | ObjectKind::Ellipse(_)
                    )
                {
                    doc.selection = vec![o.id.clone()];
                    ev.switch_tool = Some(Tool::Shape);
                }
            }
            Tool::Shape => {
                // Duplo clique no segmento adiciona nó (GD-NOD-03).
                if let Some((loc, o, path, m)) =
                    single_path(doc).map(|s| (s.r, s.obj.clone(), s.path.clone(), s.m))
                {
                    let scale = m.determinant().abs().sqrt().max(1e-12);
                    if let Some(n) = geometry::nearest(&path, m.inverse() * p)
                        .filter(|n| n.distance * scale <= tol)
                    {
                        let mut no = o.clone();
                        if let ObjectKind::Path(pd) = &mut no.kind
                            && let Some(r) = nodes::add_node(pd, n.seg, n.t)
                        {
                            doc.editor
                                .transaction(tr("Adicionar nó", "Add node"), |ed| {
                                    ed.replace_at(&loc, no)
                                });
                            doc.nodes = vec![r];
                        }
                    }
                }
            }
            Tool::Bezier => {
                self.finish_bezier(doc, defaults, false);
            }
            _ => {}
        }
    }
}

fn pt(p: [f64; 2]) -> Point {
    Point::new(p[0], p[1])
}

fn count(n: usize) -> String {
    if n == 1 {
        tr("1 objeto", "1 object").into()
    } else {
        format!("{n} {}", tr("objetos", "objects"))
    }
}

fn plural_nodes(n: usize) -> String {
    if n == 1 {
        tr("1 nó", "1 node").into()
    } else {
        format!("{n} {}", tr("nós", "nodes"))
    }
}

/// Retângulo desenhado entre dois pontos; Shift = quadrado, Ctrl = a partir do centro.
fn draw_rect(a: Point, b: Point, mods: egui::Modifiers) -> kurbo::Rect {
    let mut d = b - a;
    if mods.shift {
        let s = d.x.abs().max(d.y.abs());
        d = kurbo::Vec2::new(s * d.x.signum(), s * d.y.signum());
    }
    if mods.ctrl {
        kurbo::Rect::from_points(a - d, a + d)
    } else {
        kurbo::Rect::from_points(a, a + d)
    }
}

/// Aplica `m` (página) aos originais (pré-visualização ao vivo dentro da transação).
fn apply_live(doc: &mut DocState, originals: &[(ObjRef, Object)], m: Affine) {
    for (r, o) in originals {
        let mut n = o.clone();
        n.transform = ops::page_transform(&doc.editor, r, o, m);
        doc.editor.replace_at(r, n);
    }
}

/// Objetos selecionados com suas referências (para arrastos ao vivo).
fn originals(doc: &DocState) -> Vec<(ObjRef, Object)> {
    ops::refs(&doc.editor, &doc.selection)
        .into_iter()
        .filter_map(|r| doc.editor.get(&r).cloned().map(|o| (r, o)))
        .collect()
}

/// Objeto sob o cursor. Com `deep` (Ctrl+clique) entra nos grupos e pega o
/// objeto mais fundo (GD-SEL-02); sem, pega o objeto de primeiro nível — a
/// não ser que o objeto de dentro já esteja selecionado, para poder
/// arrastá-lo sem voltar a selecionar o grupo inteiro.
fn pick(doc: &DocState, p: Point, tol: f64, deep: bool) -> Option<String> {
    let hit = query::hit_test(doc.page(), p, tol)?;
    let top = doc.page().layers[hit.layer].objects[hit.index].id.clone();
    let inner = query::hit_test_deep(doc.page(), p, tol);
    match inner {
        Some(i) if deep || doc.selection.contains(&i) => Some(i),
        _ => Some(top),
    }
}

/// Caminho único selecionado, editável por nós (pode estar dentro de grupo).
pub struct PathSel<'a> {
    pub r: ObjRef,
    pub obj: &'a Object,
    pub path: &'a PathData,
    /// Espaço local do caminho → página (inclui os grupos acima).
    pub m: Affine,
}

pub fn single_path(doc: &DocState) -> Option<PathSel<'_>> {
    if doc.selection.len() != 1 {
        return None;
    }
    let r = doc.editor.find(&doc.selection[0])?;
    let obj = doc.editor.get(&r)?;
    let m = doc.editor.parent_affine(&r) * obj.transform.affine();
    match &obj.kind {
        ObjectKind::Path(path) => Some(PathSel { r, obj, path, m }),
        _ => None,
    }
}

fn fit_rect(r: kurbo::Rect, ph: f64, size: Vec2) -> View {
    let margin = 20.0;
    let zoom = ((size.x as f64 - 2.0 * margin) / r.width().max(1.0))
        .min((size.y as f64 - 2.0 * margin) / r.height().max(1.0));
    let zoom = zoom.clamp(crate::view::MIN_ZOOM, crate::view::MAX_ZOOM);
    let mut v = View {
        zoom,
        pan: (0.0, 0.0),
    };
    let (cx, cy) = v.page_to_screen(ph, r.center().x, r.center().y);
    v.pan_by(size.x as f64 / 2.0 - cx, size.y as f64 / 2.0 - cy);
    v
}

/// Ajusta a vista para mostrar `r` (zoom para seleção/tudo, GD-FER-04).
pub fn zoom_to_rect(cs: &mut CanvasState, r: kurbo::Rect) {
    cs.zoom_to = Some(r.inflate(r.width() * 0.05, r.height() * 0.05));
}
