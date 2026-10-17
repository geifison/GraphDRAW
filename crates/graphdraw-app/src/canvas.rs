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
use crate::theme;
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
    /// Polígono, estrela e estrela complexa (GD-PRI-03/04, GD-FER-32).
    Polygon,
    /// Texto artístico (clique) e de parágrafo (arrastar um quadro), doc 06.
    Text,
    /// Prancheta: redimensiona a página pelas bordas (doc 22, GD-DIM-23).
    Artboard,
}

impl Tool {
    pub fn info(self) -> (&'static str, &'static str, &'static str) {
        match self {
            Tool::Select => (
                "tool-select",
                crate::keymap::hint(tr("Seleção", "Select"), crate::keymap::Command::ToolSelect),
                "V",
            ),
            Tool::Shape => (
                "tool-shape",
                crate::keymap::hint(
                    tr("Forma — editar nós", "Shape — edit nodes"),
                    crate::keymap::Command::ToolShape,
                ),
                "N",
            ),
            Tool::Zoom => (
                "tool-zoom",
                crate::keymap::hint(tr("Zoom", "Zoom"), crate::keymap::Command::ToolZoom),
                "Z",
            ),
            Tool::Pan => (
                "tool-pan",
                crate::keymap::hint(
                    tr("Mão — mover a vista", "Hand — pan"),
                    crate::keymap::Command::ToolPan,
                ),
                "H",
            ),
            Tool::Bezier => (
                "tool-bezier",
                crate::keymap::hint(tr("Bézier", "Bézier"), crate::keymap::Command::ToolBezier),
                "B",
            ),
            Tool::Rect => (
                "tool-rect",
                crate::keymap::hint(
                    tr("Retângulo", "Rectangle"),
                    crate::keymap::Command::ToolRect,
                ),
                "R",
            ),
            Tool::Ellipse => (
                "tool-ellipse",
                crate::keymap::hint(tr("Elipse", "Ellipse"), crate::keymap::Command::ToolEllipse),
                "E",
            ),
            Tool::Polygon => (
                "tool-polygon",
                crate::keymap::hint(
                    tr("Polígono e estrela", "Polygon and star"),
                    crate::keymap::Command::ToolPolygon,
                ),
                "Y",
            ),
            Tool::Text => (
                "tool-text",
                crate::keymap::hint(
                    tr(
                        "Texto — clique para texto artístico, arraste para um quadro de parágrafo",
                        "Text — click for artistic text, drag for a paragraph frame",
                    ),
                    crate::keymap::Command::ToolText,
                ),
                "T",
            ),
            Tool::Artboard => (
                "tool-artboard",
                crate::keymap::hint(
                    tr("Prancheta — tamanho da página", "Artboard — page size"),
                    crate::keymap::Command::ToolArtboard,
                ),
                "Shift+P",
            ),
        }
    }
    pub const ALL: [Tool; 10] = [
        Tool::Select,
        Tool::Shape,
        Tool::Zoom,
        Tool::Pan,
        Tool::Bezier,
        Tool::Rect,
        Tool::Ellipse,
        Tool::Polygon,
        Tool::Text,
        Tool::Artboard,
    ];
}

/// Atributos aplicados aos objetos criados.
#[derive(Clone)]
pub struct Defaults {
    pub fill: Option<Fill>,
    pub stroke: Option<Stroke>,
    /// Próximo polígono: lados, modo e nitidez.
    pub polygon_sides: u32,
    pub polygon_mode: String,
    pub polygon_sharpness: f64,
}

impl Default for Defaults {
    fn default() -> Self {
        Defaults {
            fill: None,
            stroke: None,
            polygon_sides: 5,
            polygon_mode: "polygon".into(),
            polygon_sharpness: 0.5,
        }
    }
}

/// Primitiva desenhada pelo arrasto (ou pelos 3 pontos).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Prim {
    Rect,
    Ellipse,
    Polygon,
}

impl Prim {
    fn of(tool: Tool) -> Prim {
        match tool {
            Tool::Ellipse => Prim::Ellipse,
            Tool::Polygon => Prim::Polygon,
            _ => Prim::Rect,
        }
    }
}

enum Drag {
    None,
    Pan,
    Rubber {
        start: Pos2,
        /// Laço (Alt + arrastar, GD-SEL-03): pontos na página.
        lasso: Option<Vec<Point>>,
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
        prim: Prim,
        cands: Vec<(Point, SnapKind)>,
    },
    BezierHandle,
    /// Quadro de texto de parágrafo sendo desenhado.
    TextFrame {
        start: Point,
    },
    /// Prancheta: borda/canto da página sendo arrastado.
    Artboard {
        handle: Anchor,
        rect: kurbo::Rect,
        snapped: Option<&'static str>,
    },
}

/// Retângulo/elipse de 3 pontos: base já definida, falta a altura.
struct ThreeDraft {
    a: Point,
    b: Point,
    prim: Prim,
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
    /// Segurar Espaço vira a mão (quando o perfil de atalhos não usa o Espaço).
    pub space_pans: bool,
    /// Prancheta solta com objetos na página: falta escolher manter ou
    /// escalar (GD-DIM-25). Novo retângulo da página e onde abrir a escolha.
    artboard_pending: Option<(kurbo::Rect, Pos2)>,
    /// Objeto do menu de botão direito aberto no canvas.
    menu_target: Option<String>,
    /// Texto em edição (ferramenta Texto).
    pub text: Option<crate::textedit::TextEditing>,
    /// Retângulo e elipse no modo 3 pontos (GD-FER-30/31).
    pub three_point: bool,
    /// Parâmetros do próximo polígono (para a prévia do arrasto).
    poly_preview: (u32, String, f64),
    three: Option<ThreeDraft>,
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
            space_pans: true,
            artboard_pending: None,
            menu_target: None,
            text: None,
            three_point: false,
            poly_preview: (5, "polygon".into(), 0.5),
            three: None,
        }
    }
}

fn accent() -> Color32 {
    theme::ACCENT_PRIMARY
}

/// Linha da seleção e das prévias: 1 px `accent-primary` (DESIGN-SYSTEM 5.1).
fn thin() -> EStroke {
    EStroke::new(theme::GUIDE_WIDTH, accent())
}

/// Borda das alças e nós: 1,5 px `accent-primary`.
fn handle_stroke() -> EStroke {
    EStroke::new(theme::HANDLE_STROKE, accent())
}

/// Cor da marca de encaixe: página, margem, sangria e grade em
/// `guide-page`; objetos em `guide-object` (5.2).
fn guide_color(kind: SnapKind) -> Color32 {
    match kind {
        SnapKind::Grid | SnapKind::PageCorner | SnapKind::PageEdge | SnapKind::PageCenter => {
            theme::GUIDE_PAGE
        }
        _ => theme::GUIDE_OBJECT,
    }
}

/// Raio de atração do encaixe em µm no zoom atual.
fn snap_radius(view: &View) -> f64 {
    theme::SNAP_RADIUS_PX / view.zoom
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
        self.bezier.is_some() || self.three.is_some()
    }

    /// Termina a edição de texto em andamento (um passo no histórico).
    pub fn end_text(&mut self, doc: &mut DocState) {
        if let Some(t) = self.text.take() {
            t.finish(doc);
        }
    }

    /// Edita o texto `id`; com `at` (ponto da página), o cursor vai para o
    /// caractere mais próximo, senão para o fim.
    pub fn edit_text(&mut self, doc: &mut DocState, id: String, at: Option<Point>) {
        self.end_text(doc);
        let Some(r) = doc.editor.find(&id) else {
            return;
        };
        let Some(o) = doc.editor.get(&r) else { return };
        let ObjectKind::Text(t) = &o.kind else { return };
        let m = doc.editor.parent_affine(&r) * o.transform.affine();
        let layout = graphdraw_core::render::text::layout_text(t);
        let caret = match at {
            Some(p) => layout.index_at(m.inverse() * p),
            None => graphdraw_core::edit::text::len(t),
        };
        self.text = Some(crate::textedit::TextEditing::start(doc, id, caret, false));
    }

    /// Cria texto novo com origem (canto superior esquerdo) em `at` e começa
    /// a editar; criar e digitar viram um passo só.
    fn new_text(&mut self, doc: &mut DocState, at: Point, frame: Option<[f64; 2]>) {
        self.end_text(doc);
        let Some(layer) = doc.layer_for_new_object() else {
            return;
        };
        let size = graphdraw_core::edit::text::DEFAULT_SIZE;
        // Texto artístico: o clique marca a linha de base da primeira linha.
        let y = if frame.is_some() {
            at.y
        } else {
            at.y + size * 0.8
        };
        let o = graphdraw_core::edit::text::new_text(
            at.x,
            y,
            frame,
            &crate::textedit::default_family(),
            size,
        );
        let id = o.id.clone();
        doc.editor.begin(tr("Criar texto", "Create text"));
        let index = doc.editor.objects(doc.page, layer).len();
        doc.editor.insert_object(
            graphdraw_core::edit::ObjLoc {
                page: doc.page,
                layer,
                index,
            },
            o,
        );
        self.text = Some(crate::textedit::TextEditing::start(doc, id, 0, true));
    }

    /// Descarta o retângulo/elipse de 3 pontos pela metade.
    pub fn drop_three(&mut self) {
        self.three = None;
    }

    /// Fecha o retângulo/elipse de 3 pontos com a altura até `c`.
    fn finish_three(&mut self, doc: &mut DocState, t: ThreeDraft, c: Point, defaults: &Defaults) {
        if let Some((w, h, m)) = three_point_frame(t.a, t.b, c)
            && h * self.view.zoom >= 2.0
        {
            add_shape(doc, t.prim, w, h, m, defaults);
        }
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
        self.three = None;
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        doc: &mut DocState,
        tool: Tool,
        defaults: &Defaults,
    ) -> CanvasEvents {
        let mut ev = CanvasEvents::default();
        self.poly_preview = (
            defaults.polygon_sides,
            defaults.polygon_mode.clone(),
            defaults.polygon_sharpness,
        );
        let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        self.canvas_size = rect.size();
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, theme::RADIUS_NONE, theme::palette(ui).bg_canvas);

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
                i.key_down(Key::Space) && self.space_pans && !i.modifiers.any(),
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
        // Botão direito sobre um objeto: edições rápidas (travar, ocultar, nome).
        if response.secondary_clicked()
            && tool != Tool::Zoom
            && !blocked
            && let Some(pos) = response.interact_pointer_pos()
        {
            self.menu_target = pick(doc, to_page(pos), tol, mods.ctrl);
            if let Some(id) = &self.menu_target
                && !doc.selection.contains(id)
            {
                doc.selection = vec![id.clone()];
            }
        }
        if let Some(id) = self.menu_target.clone() {
            let mut menu_act = None;
            let open = response.context_menu(|ui| {
                match doc.editor.find(&id).and_then(|r| doc.editor.get(&r)) {
                    Some(o) => menu_act = crate::layertree::object_menu(ui, o),
                    None => ui.close(),
                }
            });
            if open.is_none() && !response.secondary_clicked() {
                self.menu_target = None;
            }
            if let Some(a) = menu_act {
                crate::layertree::apply(doc, a);
            }
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
        if !self.is_dragging()
            && matches!(
                tool,
                Tool::Rect | Tool::Ellipse | Tool::Polygon | Tool::Bezier
            )
        {
            self.snap_hit = self.cursor_page.and_then(|p| {
                let c = snap::candidates(doc.page(), &self.snap, &[]);
                snap::snap(p, &c, &self.snap, snap_radius(&self.view))
            });
        }

        // ---- desenho ----
        self.paint_page(ui, &painter, doc, rect);
        self.paint_overlays(ui, &painter, doc, rect, tool);
        if tool == Tool::Artboard {
            self.paint_page_handles(&painter, doc, rect);
        }
        self.artboard_choice(ui, doc);

        // Cursor da ferramenta (desenhado por cima, no lugar do cursor do sistema).
        if response.hovered()
            && let Some(pos) = hover
        {
            let ph = doc.page_size().1;
            match self.cursor_glyph(tool, panning_tool, doc, pos, rect, ph) {
                Glyph::System(c) => ui.ctx().set_cursor_icon(c),
                g => {
                    ui.ctx().set_cursor_icon(CursorIcon::None);
                    paint_glyph(ui.ctx(), &g, pos);
                }
            }
        }
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
        let bleed = doc.page().bleed.unwrap_or(doc.doc().data.settings.bleed);
        // Com sangria, a folha inclui a área de sangria: a sombra fica fora
        // dela e não aparece faixa cinza entre o corte e a linha tracejada.
        let sheet = if bleed > 0.0 {
            Rect::from_min_max(s(-bleed, ph + bleed), s(pw + bleed, -bleed))
        } else {
            page_rect
        };
        // Folha `page-paper` com `shadow-page` sobre o canvas (5.3).
        painter.add(
            theme::palette(ui)
                .shadow_page()
                .as_shape(sheet, theme::RADIUS_NONE),
        );
        painter.rect_filled(sheet, theme::RADIUS_NONE, theme::PAGE_PAPER);

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
        // Sangria (GD-DIM-21): contorno tracejado na distância da sangria e
        // marcas de corte nos cantos da página.
        if bleed > 0.0 {
            let b = sheet;
            // Linha de corte: a borda da página dentro da folha.
            painter.rect_stroke(
                page_rect,
                theme::RADIUS_NONE,
                EStroke::new(theme::GUIDE_WIDTH, theme::palette(ui).border_default),
                egui::StrokeKind::Middle,
            );
            let st = EStroke::new(theme::GUIDE_WIDTH, theme::ACCENT_SUCCESS);
            let pts = [
                b.left_top(),
                b.right_top(),
                b.right_bottom(),
                b.left_bottom(),
                b.left_top(),
            ];
            painter.extend(egui::Shape::dashed_line(
                &pts,
                st,
                theme::BLEED_DASH,
                theme::BLEED_GAP,
            ));
            let mark = EStroke::new(theme::GUIDE_WIDTH, theme::palette(ui).fg_muted);
            let (o, l) = (theme::CROP_MARK_OFFSET, theme::CROP_MARK);
            for (corner, dx, dy) in [
                (page_rect.left_top(), -1.0, -1.0),
                (page_rect.right_top(), 1.0, -1.0),
                (page_rect.right_bottom(), 1.0, 1.0),
                (page_rect.left_bottom(), -1.0, 1.0),
            ] {
                // Fora da área de sangria, alinhadas às bordas de corte.
                let ox = (b.width() - page_rect.width()) / 2.0 + o;
                let oy = (b.height() - page_rect.height()) / 2.0 + o;
                let hx = corner + Vec2::new(dx * ox, 0.0);
                painter.line_segment([hx, hx + Vec2::new(dx * l, 0.0)], mark);
                let vy = corner + Vec2::new(0.0, dy * oy);
                painter.line_segment([vy, vy + Vec2::new(0.0, dy * l)], mark);
            }
        }
        // Grade (quando ligada e legível).
        if self.show_grid
            && let Some(g) = self.snap.grid.filter(|g| *g * v.zoom >= 6.0)
        {
            let stroke = EStroke::new(theme::GUIDE_WIDTH, theme::GRID_LINE);
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

    /// Qual cursor mostrar: o da ferramenta, ou o de redimensionar/girar
    /// sobre as alças da seleção.
    fn cursor_glyph(
        &self,
        tool: Tool,
        panning: bool,
        doc: &DocState,
        pos: Pos2,
        rect: Rect,
        ph: f64,
    ) -> Glyph {
        let icon = |name, hx, hy| Glyph::Icon {
            name,
            hotspot: (hx, hy),
        };
        if panning || matches!(self.drag, Drag::Pan) {
            return if self.is_dragging() {
                icon("cur-grab", 12.0, 12.0)
            } else {
                icon("cur-pan", 12.0, 12.0)
            };
        }
        let resize = |a: Anchor| {
            Glyph::System(match a {
                Anchor::TopLeft | Anchor::BottomRight => CursorIcon::ResizeNwSe,
                Anchor::TopRight | Anchor::BottomLeft => CursorIcon::ResizeNeSw,
                Anchor::Top | Anchor::Bottom => CursorIcon::ResizeVertical,
                _ => CursorIcon::ResizeHorizontal,
            })
        };
        match &self.drag {
            Drag::Move { .. } => return icon("cur-move", 12.0, 12.0),
            Drag::Rotate { .. } => return icon("cur-rotate", 12.0, 12.0),
            Drag::Scale { handle, .. } => return resize(*handle),
            Drag::Skew { horizontal, .. } => {
                return Glyph::System(if *horizontal {
                    CursorIcon::ResizeHorizontal
                } else {
                    CursorIcon::ResizeVertical
                });
            }
            _ => {}
        }
        if tool == Tool::Select
            && let Some(b) = ops::selection_bounds(&doc.editor, &doc.selection, false)
        {
            for (a, hp) in self.handle_rects(rect, ph, b) {
                if (hp - pos).length() <= theme::HANDLE_HIT {
                    let corner = matches!(
                        a,
                        Anchor::TopLeft
                            | Anchor::TopRight
                            | Anchor::BottomLeft
                            | Anchor::BottomRight
                    );
                    return match (self.rotate_mode, corner) {
                        (true, true) => icon("cur-rotate", 12.0, 12.0),
                        (true, false) => {
                            Glyph::System(if matches!(a, Anchor::Top | Anchor::Bottom) {
                                CursorIcon::ResizeHorizontal
                            } else {
                                CursorIcon::ResizeVertical
                            })
                        }
                        (false, _) => resize(a),
                    };
                }
            }
        }
        match tool {
            Tool::Select => icon("cur-select", 6.0, 3.5),
            Tool::Shape => icon("cur-shape", 5.0, 3.5),
            Tool::Zoom => icon("cur-zoom", 10.5, 10.5),
            Tool::Pan => icon("cur-pan", 12.0, 12.0),
            Tool::Bezier => Glyph::Cross {
                badge: "tool-bezier",
            },
            Tool::Rect => Glyph::Cross { badge: "tool-rect" },
            Tool::Ellipse => Glyph::Cross {
                badge: "tool-ellipse",
            },
            Tool::Text => Glyph::Cross { badge: "tool-text" },
            Tool::Polygon => Glyph::Cross {
                badge: "tool-polygon",
            },
            Tool::Artboard => {
                let (pw, ph) = doc.page_size();
                match self.page_handle_at(rect, ph, kurbo::Rect::new(0.0, 0.0, pw, ph), pos) {
                    Some(a) => resize(a),
                    None => icon("cur-select", 6.0, 3.5),
                }
            }
        }
    }

    /// Alças nos cantos e meios das bordas da página (Prancheta, GD-DIM-23).
    fn paint_page_handles(&self, painter: &egui::Painter, doc: &DocState, rect: Rect) {
        if matches!(self.drag, Drag::Artboard { .. }) {
            return;
        }
        let (pw, ph) = doc.page_size();
        let page = kurbo::Rect::new(0.0, 0.0, pw, ph);
        painter.rect_stroke(
            self.screen_rect(rect, ph, page),
            theme::RADIUS_NONE,
            EStroke::new(theme::GUIDE_WIDTH, theme::ACCENT_PRIMARY),
            egui::StrokeKind::Middle,
        );
        for a in Anchor::ALL.into_iter().filter(|a| *a != Anchor::Center) {
            let c = self.screen(rect, ph, a.point(page));
            let r = Rect::from_center_size(c, Vec2::splat(theme::HANDLE_SIZE));
            painter.rect_filled(r, theme::RADIUS_XS, theme::HANDLE_FILL);
            painter.rect_stroke(
                r,
                theme::RADIUS_XS,
                EStroke::new(theme::HANDLE_STROKE, theme::ACCENT_PRIMARY),
                egui::StrokeKind::Middle,
            );
        }
    }

    /// Depois de soltar a Prancheta com objetos: manter posição ou escalar o
    /// conteúdo (GD-DIM-25). Esc cancela.
    fn artboard_choice(&mut self, ui: &mut egui::Ui, doc: &mut DocState) {
        use graphdraw_core::edit::pagesize::{self, ResizeMode};
        let Some((r, at)) = self.artboard_pending else {
            return;
        };
        let mut pick = None;
        let mut cancel = ui.input(|i| i.key_pressed(Key::Escape));
        egui::Area::new(ui.scope_id().with("prancheta-escolha"))
            .order(egui::Order::Foreground)
            .fixed_pos(at + Vec2::splat(theme::SPACE_200))
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.label(
                        egui::RichText::new(tr("Objetos nas páginas", "Objects on the pages"))
                            .font(theme::font_section()),
                    );
                    let w = 220.0;
                    if ui
                        .add(
                            egui::Button::new(tr("Manter posição", "Keep position"))
                                .min_size(Vec2::new(w, theme::BUTTON_DEFAULT)),
                        )
                        .on_hover_text(tr(
                            "Os objetos ficam onde estão em relação ao papel que não se moveu",
                            "Objects stay where they are relative to the paper that did not move",
                        ))
                        .clicked()
                    {
                        pick = Some(ResizeMode::KeepPosition);
                    }
                    if ui
                        .add(
                            egui::Button::new(tr("Escalar proporcional", "Scale proportionally"))
                                .min_size(Vec2::new(w, theme::BUTTON_DEFAULT)),
                        )
                        .on_hover_text(tr(
                            "Os objetos crescem ou diminuem por igual e ficam centralizados",
                            "Objects grow or shrink evenly and are centered",
                        ))
                        .clicked()
                    {
                        pick = Some(ResizeMode::ScaleContent);
                    }
                    if ui.button(tr("Cancelar (Esc)", "Cancel (Esc)")).clicked() {
                        cancel = true;
                    }
                });
            });
        if let Some(mode) = pick {
            pagesize::resize(
                &mut doc.editor,
                doc.page,
                r,
                mode,
                tr("Redimensionar páginas", "Resize pages"),
            );
            self.artboard_pending = None;
            self.fit_pending = true;
        } else if cancel {
            self.artboard_pending = None;
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
        let thin = thin();
        let hs = handle_stroke();
        // Contorno fino de cada objeto selecionado + caixa da seleção com alças.
        for r in ops::refs(&doc.editor, &doc.selection) {
            if let Some(b) = ops::object_bounds(&doc.editor, &r, false) {
                painter.rect_stroke(
                    self.screen_rect(rect, ph, b),
                    0.0,
                    EStroke::new(theme::GUIDE_WIDTH, theme::SELECTION_OBJECT),
                    egui::StrokeKind::Middle,
                );
            }
        }
        if tool == Tool::Select
            && let Some(b) = ops::selection_bounds(&doc.editor, &doc.selection, false)
        {
            let sr = self.screen_rect(rect, ph, b);
            painter.rect_stroke(sr, theme::RADIUS_NONE, thin, egui::StrokeKind::Middle);
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
                        painter.circle(p, theme::HANDLE_SIZE / 2.0, theme::HANDLE_FILL, hs);
                    } else {
                        let d = theme::HANDLE_SIZE / 2.0 + theme::SPACE_050;
                        let pts = vec![
                            p + Vec2::new(0.0, -d),
                            p + Vec2::new(d, 0.0),
                            p + Vec2::new(0.0, d),
                            p + Vec2::new(-d, 0.0),
                        ];
                        painter.add(egui::Shape::convex_polygon(pts, theme::HANDLE_FILL, hs));
                    }
                }
                let c = self.screen(rect, ph, self.rotation_center(b));
                let (cr, arm) = (theme::ROTATION_CENTER, theme::ROTATION_CENTER_ARM);
                painter.circle_stroke(c, cr, thin);
                painter.line_segment([c - Vec2::new(arm, 0.0), c + Vec2::new(arm, 0.0)], thin);
                painter.line_segment([c - Vec2::new(0.0, arm), c + Vec2::new(0.0, arm)], thin);
            } else {
                for (_, p) in self.handle_rects(rect, ph, b) {
                    // Âncora: 6 × 6 px, borda 1,5 px, raio `radius-xs` (5.1).
                    let r = Rect::from_center_size(p, Vec2::splat(theme::HANDLE_SIZE));
                    painter.rect_filled(r, theme::RADIUS_XS, theme::HANDLE_FILL);
                    painter.rect_stroke(r, theme::RADIUS_XS, hs, egui::StrokeKind::Middle);
                }
            }
        }
        if tool == Tool::Shape {
            self.paint_nodes(painter, doc, rect, ph);
        }
        // Retângulos elásticos e prévias.
        let hover = ui.input(|i| i.pointer.hover_pos());
        match &self.drag {
            Drag::Rubber {
                lasso: Some(pts), ..
            } => {
                let mut line: Vec<Pos2> = pts.iter().map(|q| self.screen(rect, ph, *q)).collect();
                if let Some(h) = hover {
                    line.push(h);
                }
                if line.len() >= 2 {
                    painter.add(egui::Shape::closed_line(line, thin));
                }
            }
            Drag::Rubber { start, .. } | Drag::NodeRubber { start } | Drag::ZoomRect { start } => {
                if let Some(h) = hover {
                    let r = Rect::from_two_pos(*start, h);
                    painter.rect_filled(r, theme::RADIUS_NONE, theme::SELECTION_FILL);
                    painter.rect_stroke(r, theme::RADIUS_NONE, thin, egui::StrokeKind::Middle);
                }
            }
            Drag::Artboard {
                rect: r, snapped, ..
            } => {
                let sr = self.screen_rect(rect, ph, *r);
                painter.extend(egui::Shape::dashed_line(
                    &[
                        sr.left_top(),
                        sr.right_top(),
                        sr.right_bottom(),
                        sr.left_bottom(),
                        sr.left_top(),
                    ],
                    EStroke::new(theme::FOCUS_WIDTH, theme::ACCENT_PRIMARY),
                    theme::BLEED_DASH,
                    theme::BLEED_GAP,
                ));
                let pal = theme::palette(ui);
                let text = match snapped {
                    Some(name) => format!(
                        "{} {name}  ·  {} × {} mm",
                        tr("Encaixado em: ISO", "Snapped to: ISO"),
                        crate::widgets::fmt_mm(r.width()),
                        crate::widgets::fmt_mm(r.height())
                    ),
                    None => format!(
                        "{} × {} mm",
                        crate::widgets::fmt_mm(r.width()),
                        crate::widgets::fmt_mm(r.height())
                    ),
                };
                let galley = painter.layout_no_wrap(
                    text,
                    theme::font_numeric(),
                    if snapped.is_some() {
                        theme::FG_ON_ACCENT
                    } else {
                        pal.fg_primary
                    },
                );
                let pad = Vec2::new(theme::SPACE_200, theme::SPACE_100);
                let at = sr.center_top()
                    - Vec2::new(
                        galley.size().x / 2.0 + pad.x,
                        galley.size().y + 2.0 * pad.y + theme::SPACE_200,
                    );
                let bg = Rect::from_min_size(at, galley.size() + 2.0 * pad);
                painter.rect_filled(
                    bg,
                    theme::RADIUS_FULL,
                    if snapped.is_some() {
                        theme::ACCENT_PRIMARY
                    } else {
                        pal.bg_surface_1
                    },
                );
                painter.galley(at + pad, galley, pal.fg_primary);
            }
            Drag::DrawRect { start, .. } if self.three_point => {
                if let Some(cur) = self.cursor_page {
                    let mods = ui.input(|i| i.modifiers);
                    let b = base_end(*start, self.snap_hit.map_or(cur, |s| s.point), mods);
                    painter.line_segment(
                        [self.screen(rect, ph, *start), self.screen(rect, ph, b)],
                        thin,
                    );
                }
            }
            Drag::DrawRect { start, prim, .. } => {
                if let Some(cur) = self.cursor_page {
                    let mods = ui.input(|i| i.modifiers);
                    let r = draw_rect(*start, self.snap_hit.map_or(cur, |s| s.point), mods);
                    let sr = self.screen_rect(rect, ph, r);
                    match prim {
                        Prim::Ellipse => {
                            painter.add(egui::Shape::ellipse_stroke(
                                sr.center(),
                                sr.size() / 2.0,
                                thin,
                            ));
                        }
                        Prim::Rect => {
                            painter.rect_stroke(
                                sr,
                                theme::RADIUS_NONE,
                                thin,
                                egui::StrokeKind::Middle,
                            );
                        }
                        Prim::Polygon => {
                            let mut path = shape_outline_with(
                                *prim,
                                r.width(),
                                r.height(),
                                _defaults_view(&self.poly_preview),
                            );
                            path.apply_affine(Affine::translate((r.x0, r.y0)));
                            let mut line = Vec::new();
                            kurbo::flatten(path, 0.5 / self.view.zoom.max(1e-9), |el| {
                                if let kurbo::PathEl::MoveTo(q) | kurbo::PathEl::LineTo(q) = el {
                                    line.push(self.screen(rect, ph, q));
                                }
                            });
                            painter.add(egui::Shape::closed_line(line, thin));
                        }
                    }
                }
            }
            _ => {}
        }
        if let (Drag::TextFrame { start }, Some(cur)) = (&self.drag, self.cursor_page) {
            let sr = self.screen_rect(rect, ph, kurbo::Rect::from_points(*start, cur));
            painter.extend(egui::Shape::dashed_line(
                &[
                    sr.left_top(),
                    sr.right_top(),
                    sr.right_bottom(),
                    sr.left_bottom(),
                    sr.left_top(),
                ],
                thin,
                theme::BLEED_DASH,
                theme::BLEED_GAP,
            ));
        }
        let to_screen = |q: Point| self.screen(rect, ph, q);
        match &self.text {
            Some(te) => te.paint(painter, doc, to_screen, ui.input(|i| i.time)),
            None => {
                // Quadro e aviso de excedente dos textos de parágrafo selecionados.
                for id in &doc.selection {
                    if let Some(r) = doc.editor.find(id)
                        && let Some(o) = doc.editor.get(&r)
                        && let ObjectKind::Text(t) = &o.kind
                    {
                        let m = doc.editor.parent_affine(&r) * o.transform.affine();
                        let l = graphdraw_core::render::text::layout_text(t);
                        crate::textedit::frame_and_overflow(painter, t, &l, &|q| to_screen(m * q));
                    }
                }
            }
        }
        if let (Some(t), Some(cur)) = (&self.three, self.cursor_page) {
            let c = self.snap_hit.map_or(cur, |s| s.point);
            if let Some((w, h, m)) = three_point_frame(t.a, t.b, c) {
                let mut path = shape_outline_with(t.prim, w, h, _defaults_view(&self.poly_preview));
                path.apply_affine(m);
                let mut line = Vec::new();
                kurbo::flatten(path, 0.5 / self.view.zoom.max(1e-9), |el| {
                    if let kurbo::PathEl::MoveTo(q) | kurbo::PathEl::LineTo(q) = el {
                        line.push(self.screen(rect, ph, q));
                    }
                });
                painter.add(egui::Shape::closed_line(line, thin));
            }
            painter.line_segment(
                [self.screen(rect, ph, t.a), self.screen(rect, ph, t.b)],
                thin,
            );
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
                painter.rect_filled(
                    Rect::from_center_size(p, Vec2::splat(theme::HANDLE_SIZE)),
                    theme::RADIUS_XS,
                    accent(),
                );
                for h in [n.handle_in, n.handle_out].into_iter().flatten() {
                    let hp = self.screen(rect, ph, pt(h));
                    painter.line_segment([p, hp], thin);
                    painter.circle_filled(hp, theme::NODE_SIZE / 2.0, accent());
                }
            }
        }
        if let Some(s) = self.snap_hit {
            let p = self.screen(rect, ph, s.point);
            let color = guide_color(s.kind);
            painter.circle_stroke(p, theme::SNAP_MARK, EStroke::new(theme::FOCUS_WIDTH, color));
            painter.text(
                p + Vec2::new(theme::SNAP_MARK, -theme::SNAP_MARK) * 1.5,
                egui::Align2::LEFT_BOTTOM,
                s.kind.label(),
                theme::font_micro(),
                color,
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
        let thin = thin();
        let hs = handle_stroke();
        self.stroke_bez(painter, rect, ph, &bez, thin);
        for r in nodes::all_nodes(path) {
            let n = nodes::node(path, r).unwrap();
            let p = self.screen(rect, ph, m * pt(n.p));
            let selected = doc.nodes.contains(&r);
            if selected {
                for h in [n.handle_in, n.handle_out].into_iter().flatten() {
                    let hp = self.screen(rect, ph, m * pt(h));
                    painter.line_segment([p, hp], thin);
                    painter.circle(hp, theme::NODE_SIZE / 2.0, theme::HANDLE_FILL, hs);
                }
            }
            // Nó de canto = quadrado, nó suave = círculo; selecionado em
            // `accent-primary`, senão branco com borda (5.1).
            let r = Rect::from_center_size(p, Vec2::splat(theme::NODE_SIZE));
            let fill = if selected {
                accent()
            } else {
                theme::HANDLE_FILL
            };
            match n.kind {
                NodeKind::Corner => {
                    painter.rect_filled(r, theme::RADIUS_XS, fill);
                    painter.rect_stroke(r, theme::RADIUS_XS, hs, egui::StrokeKind::Middle);
                }
                _ => {
                    painter.circle(p, theme::NODE_SIZE / 2.0, fill, hs);
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
            Tool::Text => {
                self.end_text(doc);
                self.drag = Drag::TextFrame { start: p };
            }
            Tool::Rect | Tool::Ellipse | Tool::Polygon => {
                let cands = snap::candidates(doc.page(), &self.snap, &[]);
                let start = snap::snap(p, &cands, &self.snap, snap_radius(&self.view))
                    .map_or(p, |s| s.point);
                // Com a base pronta, apertar o botão fecha a forma.
                if let Some(t) = self.three.take() {
                    self.finish_three(doc, t, start, _defaults);
                    return;
                }
                self.drag = Drag::DrawRect {
                    start,
                    prim: Prim::of(tool),
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
            Tool::Artboard => {
                let (pw, _) = doc.page_size();
                let page = kurbo::Rect::new(0.0, 0.0, pw, ph);
                if let Some(a) = self.page_handle_at(rect, ph, page, origin) {
                    self.drag = Drag::Artboard {
                        handle: a,
                        rect: page,
                        snapped: None,
                    };
                }
            }
        }
    }

    /// Alça da página (cantos e meios das bordas) sob o ponto de tela.
    fn page_handle_at(&self, rect: Rect, ph: f64, page: kurbo::Rect, at: Pos2) -> Option<Anchor> {
        Anchor::ALL
            .into_iter()
            .filter(|a| *a != Anchor::Center)
            .find(|a| (self.screen(rect, ph, a.point(page)) - at).length() <= theme::HANDLE_HIT)
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
        // Alt + arrastar: laço à mão livre (GD-SEL-03, GD-FER-01).
        if mods.alt {
            if !mods.shift {
                doc.selection.clear();
                self.rotate_mode = false;
            }
            self.drag = Drag::Rubber {
                start: origin,
                lasso: Some(vec![p]),
            };
            return;
        }
        if let Some(b) = ops::selection_bounds(&doc.editor, &doc.selection, false) {
            let near = |q: Pos2| (q - origin).length() <= theme::HANDLE_HIT;
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
                self.drag = Drag::Rubber {
                    start: origin,
                    lasso: None,
                };
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
            self.drag = Drag::Rubber {
                start: origin,
                lasso: None,
            };
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
        if let Drag::Rubber {
            lasso: Some(pts), ..
        } = &mut self.drag
        {
            // Um ponto a cada 2 px de tela basta para o laço.
            if pts
                .last()
                .is_none_or(|l| (cur - *l).hypot() * self.view.zoom > 2.0)
            {
                pts.push(cur);
            }
            return;
        }
        let radius = snap_radius(&self.view);
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
            Drag::Artboard { handle, rect, .. } => {
                let (handle, mut r) = (*handle, *rect);
                let (fx, fy) = handle.fraction();
                let min = 1_000.0;
                if fx == 0.0 {
                    r.x0 = cur.x.min(r.x1 - min);
                } else if fx == 1.0 {
                    r.x1 = cur.x.max(r.x0 + min);
                }
                if fy == 0.0 {
                    r.y0 = cur.y.min(r.y1 - min);
                } else if fy == 1.0 {
                    r.y1 = cur.y.max(r.y0 + min);
                }
                // Encaixe em formatos conhecidos (GD-DIM-24); Alt desliga.
                let mut snapped = None;
                if !mods.alt {
                    let tol = (theme::SNAP_RADIUS_PX * 2.0 / self.view.zoom).max(500.0);
                    if let Some((p, w, h)) =
                        graphdraw_core::edit::pagesize::snap_format(r.width(), r.height(), tol)
                    {
                        if fx == 0.0 {
                            r.x0 = r.x1 - w;
                        } else if fx == 1.0 {
                            r.x1 = r.x0 + w;
                        }
                        if fy == 0.0 {
                            r.y0 = r.y1 - h;
                        } else if fy == 1.0 {
                            r.y1 = r.y0 + h;
                        }
                        if (r.width() - w).abs() < 1.0 && (r.height() - h).abs() < 1.0 {
                            snapped = Some(p.name);
                        }
                    }
                }
                self.drag = Drag::Artboard {
                    handle,
                    rect: r,
                    snapped,
                };
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
            Drag::Rubber { start, lasso } => {
                let hits = match lasso {
                    Some(mut pts) => {
                        pts.push(cur);
                        query::objects_in_lasso(doc.page(), &pts)
                    }
                    None => {
                        let a = self.page_of(rect, ph, start);
                        query::objects_in_rect(doc.page(), kurbo::Rect::from_points(a, cur))
                    }
                };
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
            Drag::DrawRect { start, prim, .. } if self.three_point => {
                // 3 pontos: o arrasto define a base; a altura vem a seguir.
                let endp = base_end(start, self.snap_hit.map_or(cur, |s| s.point), mods);
                if (endp - start).hypot() * self.view.zoom >= 2.0 {
                    self.three = Some(ThreeDraft {
                        a: start,
                        b: endp,
                        prim,
                    });
                }
            }
            Drag::DrawRect { start, prim, .. } => {
                let endp = self.snap_hit.map_or(cur, |s| s.point);
                let r = draw_rect(start, endp, mods);
                if r.width() * self.view.zoom >= 2.0 && r.height() * self.view.zoom >= 2.0 {
                    let m = Affine::translate((r.x0, r.y0));
                    add_shape(doc, prim, r.width(), r.height(), m, defaults);
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
            Drag::TextFrame { start } => {
                let r = kurbo::Rect::from_points(start, cur);
                if r.width() * self.view.zoom >= 8.0 && r.height() * self.view.zoom >= 8.0 {
                    // Origem no canto superior esquerdo (página com Y para cima).
                    self.new_text(doc, Point::new(r.x0, r.y1), Some([r.width(), r.height()]));
                }
            }
            Drag::Artboard { rect: r, .. } => {
                let (pw, _) = doc.page_size();
                let unchanged =
                    (r.x0.abs() + r.y0.abs() + (r.x1 - pw).abs() + (r.y1 - ph).abs()) < 1.0;
                let has_objects = doc.doc().data.pages.iter().any(|p| p.object_count() > 0);
                if unchanged {
                } else if has_objects {
                    self.artboard_pending = Some((r, end));
                } else {
                    graphdraw_core::edit::pagesize::resize(
                        &mut doc.editor,
                        doc.page,
                        r,
                        graphdraw_core::edit::pagesize::ResizeMode::KeepPosition,
                        tr("Redimensionar páginas", "Resize pages"),
                    );
                    self.fit_pending = true;
                }
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
            snap_radius(&self.view),
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
            Tool::Rect | Tool::Ellipse | Tool::Polygon => {
                if let Some(t) = self.three.take() {
                    let c = self.snap_hit.map_or(p, |s| s.point);
                    self.finish_three(doc, t, c, defaults);
                }
            }
            Tool::Text => {
                let hit = pick(doc, p, tol, true).filter(|id| {
                    doc.editor
                        .find(id)
                        .and_then(|r| doc.editor.get(&r))
                        .is_some_and(|o| matches!(o.kind, ObjectKind::Text(_)))
                });
                match (hit, &mut self.text) {
                    (Some(id), Some(te)) if te.id == id => te.click(doc, p, mods.shift),
                    (Some(id), _) => self.edit_text(doc, id, Some(p)),
                    (None, _) => {
                        let was_editing = self.text.is_some();
                        self.end_text(doc);
                        // Clique fora do texto em edição só termina a edição.
                        if !was_editing {
                            self.new_text(doc, p, None);
                        }
                    }
                }
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
            Tool::Select
                if pick(doc, p, tol, true).is_some_and(|id| {
                    doc.editor
                        .find(&id)
                        .and_then(|r| doc.editor.get(&r))
                        .is_some_and(|o| matches!(o.kind, ObjectKind::Text(_)))
                }) =>
            {
                if let Some(id) = pick(doc, p, tol, true) {
                    ev.switch_tool = Some(Tool::Text);
                    self.edit_text(doc, id, Some(p));
                }
            }
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

/// Cursor desenhado sobre o canvas.
enum Glyph {
    /// Ícone com o ponto ativo em coordenadas da grade 24×24.
    Icon {
        name: &'static str,
        hotspot: (f32, f32),
    },
    /// Mira precisa com o ícone da ferramenta ao lado (ferramentas de desenho).
    Cross { badge: &'static str },
    /// Cursor do sistema (alças de redimensionar).
    System(CursorIcon),
}

const CURSOR_SIZE: f32 = theme::ICON_CURSOR;

/// Desenha o ícone em preto com um halo branco, para aparecer tanto no papel
/// quanto na área de trabalho escura.
fn paint_icon(ctx: &egui::Context, painter: &egui::Painter, name: &str, r: Rect) {
    let img = egui::Image::new(crate::icons::get(name));
    let Ok(egui::load::TexturePoll::Ready { texture }) =
        img.load_for_size(ctx, r.size() * ctx.pixels_per_point())
    else {
        return;
    };
    let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
    for (dx, dy) in [
        (-1.0, 0.0),
        (1.0, 0.0),
        (0.0, -1.0),
        (0.0, 1.0),
        (-1.0, -1.0),
        (1.0, 1.0),
        (-1.0, 1.0),
        (1.0, -1.0),
    ] {
        painter.image(
            texture.id,
            r.translate(Vec2::new(dx, dy)),
            uv,
            theme::CURSOR_HALO,
        );
    }
    painter.image(texture.id, r, uv, theme::CURSOR_INK);
}

fn paint_glyph(ctx: &egui::Context, g: &Glyph, pos: Pos2) {
    let layer = egui::LayerId::new(egui::Order::Tooltip, egui::Id::unique("cursor-ferramenta"));
    let painter = ctx.layer_painter(layer);
    match g {
        Glyph::Icon { name, hotspot } => {
            let k = CURSOR_SIZE / 24.0;
            let min = pos - Vec2::new(hotspot.0 * k, hotspot.1 * k);
            paint_icon(
                ctx,
                &painter,
                name,
                Rect::from_min_size(min, Vec2::splat(CURSOR_SIZE)),
            );
        }
        Glyph::Cross { badge } => {
            let (gap, len) = (theme::CROSS_GAP, theme::CROSS_ARM);
            let ink = theme::GUIDE_WIDTH;
            for (w, c) in [
                (ink + 2.0 * theme::CURSOR_HALO_WIDTH, theme::CURSOR_HALO),
                (ink, theme::CURSOR_INK),
            ] {
                let st = EStroke::new(w, c);
                for (a, b) in [
                    (Vec2::new(-len, 0.0), Vec2::new(-gap, 0.0)),
                    (Vec2::new(gap, 0.0), Vec2::new(len, 0.0)),
                    (Vec2::new(0.0, -len), Vec2::new(0.0, -gap)),
                    (Vec2::new(0.0, gap), Vec2::new(0.0, len)),
                ] {
                    painter.line_segment([pos + a, pos + b], st);
                }
            }
            let off = len - 1.0;
            let r = Rect::from_min_size(pos + Vec2::splat(off), Vec2::splat(theme::CROSS_BADGE));
            paint_icon(ctx, &painter, badge, r);
        }
        Glyph::System(_) => {}
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
/// Fim da base do modo 3 pontos; Shift prende o ângulo em passos de 15°.
fn base_end(a: Point, b: Point, mods: egui::Modifiers) -> Point {
    if !mods.shift {
        return b;
    }
    let d = b - a;
    let step = 15f64.to_radians();
    let ang = (d.y.atan2(d.x) / step).round() * step;
    a + kurbo::Vec2::new(ang.cos(), ang.sin()) * d.hypot()
}

/// Retângulo de 3 pontos: base `a`→`b` e altura até `c`, medida na
/// perpendicular. Devolve largura, altura e a matriz que leva o retângulo
/// local (0, 0)–(w, h) à página, com o eixo x ao longo da base.
fn three_point_frame(a: Point, b: Point, c: Point) -> Option<(f64, f64, Affine)> {
    let base = b - a;
    let w = base.hypot();
    if w <= 0.0 {
        return None;
    }
    let u = base / w;
    let n = kurbo::Vec2::new(-u.y, u.x);
    let h = (c - a).dot(n);
    let origin = if h < 0.0 { a + n * h } else { a };
    let m = Affine::translate(origin.to_vec2()) * Affine::rotate(u.y.atan2(u.x));
    Some((w, h.abs(), m))
}

/// Parâmetros do polígono como dados do modelo (raios definidos depois).
fn _defaults_view(p: &(u32, String, f64)) -> graphdraw_core::model::PolygonData {
    graphdraw_core::model::PolygonData {
        rx: 1.0,
        ry: 1.0,
        sides: p.0,
        mode: p.1.clone(),
        sharpness: p.2,
        extra: Map::new(),
    }
}

/// Contorno local da forma (0, 0)–(w, h), para a prévia.
fn shape_outline_with(
    prim: Prim,
    w: f64,
    h: f64,
    poly: graphdraw_core::model::PolygonData,
) -> BezPath {
    use kurbo::Shape;
    match prim {
        Prim::Ellipse => {
            kurbo::Ellipse::new((w / 2.0, h / 2.0), (w / 2.0, h / 2.0), 0.0).to_path(0.1)
        }
        Prim::Rect => kurbo::Rect::new(0.0, 0.0, w, h).to_path(0.1),
        Prim::Polygon => {
            let mut p =
                graphdraw_core::render::scene::polygon_path(&graphdraw_core::model::PolygonData {
                    rx: w / 2.0,
                    ry: h / 2.0,
                    ..poly
                });
            p.apply_affine(Affine::translate((w / 2.0, h / 2.0)));
            p
        }
    }
}

/// Cria um retângulo ou elipse com o quadro (0, 0)–(w, h) levado à página
/// por `m`, com os atributos padrão, e o seleciona.
fn add_shape(doc: &mut DocState, prim: Prim, w: f64, h: f64, m: Affine, defaults: &Defaults) {
    let kind = match prim {
        Prim::Ellipse => ObjectKind::Ellipse(EllipseData {
            rx: w / 2.0,
            ry: h / 2.0,
            mode: "ellipse".into(),
            start_angle: 0.0,
            end_angle: 360.0,
            extra: Map::new(),
        }),
        Prim::Polygon => ObjectKind::Polygon(graphdraw_core::model::PolygonData {
            rx: w / 2.0,
            ry: h / 2.0,
            sides: defaults.polygon_sides,
            mode: defaults.polygon_mode.clone(),
            sharpness: defaults.polygon_sharpness,
            extra: Map::new(),
        }),
        Prim::Rect => ObjectKind::Rectangle(RectData {
            width: w,
            height: h,
            corners: None,
            extra: Map::new(),
        }),
    };
    let mut o = Object::new(kind);
    // Elipse e polígono têm a origem no centro.
    let m = if prim == Prim::Rect {
        m
    } else {
        m * Affine::translate((w / 2.0, h / 2.0))
    };
    let c = m.as_coeffs();
    o.transform = if c[0] == 1.0 && c[1] == 0.0 && c[2] == 0.0 && c[3] == 1.0 {
        Transform::translate(c[4], c[5])
    } else {
        Transform::from_affine(m)
    };
    o.fill = defaults.fill.clone();
    o.stroke = defaults.stroke.clone();
    if let Some(layer) = doc.layer_for_new_object() {
        let name = match prim {
            Prim::Ellipse => tr("Criar elipse", "Create ellipse"),
            Prim::Polygon => tr("Criar polígono", "Create polygon"),
            Prim::Rect => tr("Criar retângulo", "Create rectangle"),
        };
        let id = ops::add_object(&mut doc.editor, doc.page, layer, o, name);
        doc.selection = vec![id];
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    /// Roda um quadro do canvas, sem janela, com os eventos dados.
    fn canvas_frame(
        ctx: &egui::Context,
        cs: &mut CanvasState,
        doc: &mut DocState,
        tool: Tool,
        events: Vec<egui::Event>,
    ) {
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1366.0, 768.0))),
            events,
            ..Default::default()
        };
        let defaults = Defaults::default();
        let mut out = ctx.run_ui(input, |ui| {
            egui::CentralPanel::no_frame().show(ui, |ui| {
                cs.show(ui, doc, tool, &defaults);
            });
        });
        out.textures_delta.clear();
    }

    /// Aceite de GD-DIM-24: arrastar o canto de uma A4 até perto de 148 × 210
    /// mm gruda em A5; sem objetos, o tamanho muda direto em todas as páginas
    /// (GD-PAG-01), num passo do histórico.
    /// GD-FER-32: arrastar com a ferramenta Polígono cria o polígono com os
    /// parâmetros atuais, inscrito na caixa arrastada.
    #[test]
    fn polygon_tool_drag_creates_polygon() {
        let ctx = crate::theme::test_ctx();
        let mut cs = CanvasState::default();
        cs.snap.grid = None;
        let mut doc = DocState::new_blank();
        for _ in 0..2 {
            canvas_frame(&ctx, &mut cs, &mut doc, Tool::Polygon, vec![]);
        }
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(1366.0, 768.0));
        let (_, ph) = doc.page_size();
        let (s0, s1) = (
            cs.screen(rect, ph, Point::new(20_000.0, 100_000.0)),
            cs.screen(rect, ph, Point::new(60_000.0, 140_000.0)),
        );
        let btn = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        canvas_frame(
            &ctx,
            &mut cs,
            &mut doc,
            Tool::Polygon,
            vec![egui::Event::PointerMoved(s0)],
        );
        canvas_frame(&ctx, &mut cs, &mut doc, Tool::Polygon, vec![btn(s0, true)]);
        for k in 1..=6 {
            let pos = s0 + (s1 - s0) * (k as f32 / 6.0);
            canvas_frame(
                &ctx,
                &mut cs,
                &mut doc,
                Tool::Polygon,
                vec![egui::Event::PointerMoved(pos)],
            );
        }
        canvas_frame(&ctx, &mut cs, &mut doc, Tool::Polygon, vec![btn(s1, false)]);
        let o = &doc.page().layers[0].objects[0];
        let ObjectKind::Polygon(p) = &o.kind else {
            panic!("polígono")
        };
        let tol = 2.0 / cs.view.zoom;
        assert_eq!((p.sides, p.mode.as_str()), (5, "polygon"));
        assert!((p.rx - 20_000.0).abs() < tol && (p.ry - 20_000.0).abs() < tol);
    }

    /// Ferramenta Texto: clique cria texto artístico já em edição; arrastar
    /// cria texto de parágrafo com o quadro desenhado.
    #[test]
    fn text_tool_click_and_drag() {
        let ctx = crate::theme::test_ctx();
        let mut cs = CanvasState::default();
        cs.snap.grid = None;
        let mut doc = DocState::new_blank();
        for _ in 0..2 {
            canvas_frame(&ctx, &mut cs, &mut doc, Tool::Text, vec![]);
        }
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(1366.0, 768.0));
        let (_, ph) = doc.page_size();
        let at = |cs: &CanvasState, x: f64, y: f64| cs.screen(rect, ph, Point::new(x, y));
        let btn = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        let c = at(&cs, 50_000.0, 200_000.0);
        canvas_frame(
            &ctx,
            &mut cs,
            &mut doc,
            Tool::Text,
            vec![egui::Event::PointerMoved(c)],
        );
        canvas_frame(&ctx, &mut cs, &mut doc, Tool::Text, vec![btn(c, true)]);
        canvas_frame(&ctx, &mut cs, &mut doc, Tool::Text, vec![btn(c, false)]);
        assert!(cs.text.is_some(), "editando");
        assert!(doc.editor.in_transaction());
        cs.end_text(&mut doc);
        assert!(doc.page().layers[0].objects.is_empty(), "texto vazio sai");
        // Arrastar um quadro de 60 × 30 mm.
        let (s0, s1) = (at(&cs, 20_000.0, 250_000.0), at(&cs, 80_000.0, 220_000.0));
        canvas_frame(
            &ctx,
            &mut cs,
            &mut doc,
            Tool::Text,
            vec![egui::Event::PointerMoved(s0)],
        );
        canvas_frame(&ctx, &mut cs, &mut doc, Tool::Text, vec![btn(s0, true)]);
        for k in 1..=6 {
            let pos = s0 + (s1 - s0) * (k as f32 / 6.0);
            canvas_frame(
                &ctx,
                &mut cs,
                &mut doc,
                Tool::Text,
                vec![egui::Event::PointerMoved(pos)],
            );
        }
        canvas_frame(&ctx, &mut cs, &mut doc, Tool::Text, vec![btn(s1, false)]);
        let te = cs.text.as_ref().expect("editando o parágrafo");
        let r = doc.editor.find(&te.id).unwrap();
        let ObjectKind::Text(t) = &doc.editor.get(&r).unwrap().kind else {
            panic!("texto")
        };
        let [w, h] = t.frame.unwrap();
        let tol = 2.0 / cs.view.zoom;
        assert_eq!(t.text_kind, "paragraph");
        assert!(
            (w - 60_000.0).abs() < tol && (h - 30_000.0).abs() < tol,
            "{w} × {h}"
        );
    }

    /// Retângulo de 3 pontos: base girada 90°, altura para qualquer lado.
    #[test]
    fn three_point_frame_follows_base_and_height() {
        let a = Point::new(10.0, 10.0);
        let b = Point::new(10.0, 40.0);
        // Base vertical (x local = +y da página); a normal aponta para -x.
        let (w, h, m) = three_point_frame(a, b, Point::new(-10.0, 25.0)).unwrap();
        assert_eq!((w, h), (30.0, 20.0));
        let corners = [(0.0, 0.0), (w, 0.0), (w, h), (0.0, h)].map(|(x, y)| m * Point::new(x, y));
        let near = |p: Point, q: (f64, f64)| (p - Point::new(q.0, q.1)).hypot() < 1e-9;
        assert!(near(corners[0], (10.0, 10.0)));
        assert!(near(corners[1], (10.0, 40.0)));
        assert!(near(corners[2], (-10.0, 40.0)));
        // Altura do outro lado: o canto de origem desloca, a forma não espelha.
        let (_, h2, m2) = three_point_frame(a, b, Point::new(25.0, 0.0)).unwrap();
        assert_eq!(h2, 15.0);
        let r = kurbo::Rect::new(0.0, 0.0, 30.0, 15.0);
        let bb = m2.transform_rect_bbox(r);
        assert!((bb.x0 - 10.0).abs() < 1e-9 && (bb.x1 - 25.0).abs() < 1e-9);
        assert!(m2.determinant() > 0.0);
        assert!(three_point_frame(a, a, b).is_none());
    }

    /// GD-FER-30: no modo 3 pontos, arrastar a base e clicar na altura cria
    /// um retângulo girado com as medidas certas, num passo do histórico.
    #[test]
    fn three_point_rectangle_by_drag_and_click() {
        let ctx = crate::theme::test_ctx();
        let mut cs = CanvasState {
            three_point: true,
            ..Default::default()
        };
        cs.snap.grid = None;
        let mut doc = DocState::new_blank();
        for _ in 0..2 {
            canvas_frame(&ctx, &mut cs, &mut doc, Tool::Rect, vec![]);
        }
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(1366.0, 768.0));
        let (_, ph) = doc.page_size();
        let at = |cs: &CanvasState, x: f64, y: f64| cs.screen(rect, ph, Point::new(x, y));
        let btn = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        // Base diagonal de (50, 50) mm a (90, 80) mm: 50 mm de comprimento.
        let (s0, s1) = (at(&cs, 50_000.0, 50_000.0), at(&cs, 90_000.0, 80_000.0));
        canvas_frame(
            &ctx,
            &mut cs,
            &mut doc,
            Tool::Rect,
            vec![egui::Event::PointerMoved(s0)],
        );
        canvas_frame(&ctx, &mut cs, &mut doc, Tool::Rect, vec![btn(s0, true)]);
        for k in 1..=8 {
            let pos = s0 + (s1 - s0) * (k as f32 / 8.0);
            canvas_frame(
                &ctx,
                &mut cs,
                &mut doc,
                Tool::Rect,
                vec![egui::Event::PointerMoved(pos)],
            );
        }
        canvas_frame(&ctx, &mut cs, &mut doc, Tool::Rect, vec![btn(s1, false)]);
        assert!(cs.has_draft(), "base pronta, falta a altura");
        assert_eq!(doc.page().object_count(), 0);
        // Altura de 20 mm na perpendicular: normal (-0,6; 0,8).
        let c = at(&cs, 90_000.0 - 12_000.0, 80_000.0 + 16_000.0);
        canvas_frame(
            &ctx,
            &mut cs,
            &mut doc,
            Tool::Rect,
            vec![egui::Event::PointerMoved(c)],
        );
        canvas_frame(&ctx, &mut cs, &mut doc, Tool::Rect, vec![btn(c, true)]);
        canvas_frame(&ctx, &mut cs, &mut doc, Tool::Rect, vec![btn(c, false)]);
        canvas_frame(&ctx, &mut cs, &mut doc, Tool::Rect, vec![]);
        assert!(!cs.has_draft());
        let objs = &doc
            .page()
            .layers
            .iter()
            .flat_map(|l| l.objects.iter())
            .collect::<Vec<_>>();
        assert_eq!(objs.len(), 1);
        let ObjectKind::Rectangle(r) = &objs[0].kind else {
            panic!("retângulo")
        };
        // Tolerância de meio pixel de tela convertido em µm.
        let tol = 1.0 / cs.view.zoom;
        assert!((r.width - 50_000.0).abs() < tol, "{}", r.width);
        assert!((r.height - 20_000.0).abs() < tol, "{}", r.height);
        let m = objs[0].transform.affine();
        let ang = m.as_coeffs()[1].atan2(m.as_coeffs()[0]).to_degrees();
        assert!((ang - 36.87).abs() < 0.5, "{ang}");
        doc.editor.undo();
        assert_eq!(doc.page().object_count(), 0);
    }

    /// GD-SEL-03: Alt + arrastar desenha um laço; só o objeto cercado entra
    /// na seleção.
    #[test]
    fn alt_drag_selects_with_lasso() {
        let ctx = crate::theme::test_ctx();
        let mut cs = CanvasState::default();
        let mut doc = DocState::new_blank();
        let square = |x: f64| {
            let mut o = Object::new(ObjectKind::Rectangle(RectData {
                width: 20_000.0,
                height: 20_000.0,
                corners: None,
                extra: Map::new(),
            }));
            o.transform = graphdraw_core::model::Transform::translate(x, 100_000.0);
            o
        };
        let (a, b) = (square(30_000.0), square(150_000.0));
        let a_id = a.id.clone();
        for (k, o) in [a, b].into_iter().enumerate() {
            let loc = graphdraw_core::edit::ObjLoc {
                page: doc.page,
                layer: 0,
                index: k,
            };
            doc.editor.insert_object(loc, o);
        }
        for _ in 0..2 {
            canvas_frame(&ctx, &mut cs, &mut doc, Tool::Select, vec![]);
        }
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(1366.0, 768.0));
        let (_, ph) = doc.page_size();
        // Laço em volta do primeiro quadrado (coordenadas da página, µm).
        let ring: Vec<Pos2> = [
            (20_000.0, 90_000.0),
            (60_000.0, 90_000.0),
            (60_000.0, 130_000.0),
            (20_000.0, 130_000.0),
            (20_000.0, 95_000.0),
        ]
        .iter()
        .map(|&(x, y)| cs.screen(rect, ph, Point::new(x, y)))
        .collect();
        let alt = egui::Modifiers::ALT;
        let btn = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: alt,
        };
        let held = |e: egui::Event| vec![egui::Event::ModifiersChanged(alt), e];
        canvas_frame(
            &ctx,
            &mut cs,
            &mut doc,
            Tool::Select,
            held(egui::Event::PointerMoved(ring[0])),
        );
        canvas_frame(
            &ctx,
            &mut cs,
            &mut doc,
            Tool::Select,
            held(btn(ring[0], true)),
        );
        for w in ring.windows(2) {
            for k in 1..=5 {
                let pos = w[0] + (w[1] - w[0]) * (k as f32 / 5.0);
                canvas_frame(
                    &ctx,
                    &mut cs,
                    &mut doc,
                    Tool::Select,
                    held(egui::Event::PointerMoved(pos)),
                );
            }
        }
        assert!(
            matches!(cs.drag, Drag::Rubber { lasso: Some(_), .. }),
            "laço ativo"
        );
        let last = *ring.last().unwrap();
        canvas_frame(
            &ctx,
            &mut cs,
            &mut doc,
            Tool::Select,
            held(btn(last, false)),
        );
        canvas_frame(&ctx, &mut cs, &mut doc, Tool::Select, vec![]);
        assert_eq!(doc.selection, vec![a_id]);
    }

    #[test]
    fn artboard_snaps_a4_to_a5() {
        let ctx = crate::theme::test_ctx();
        let mut cs = CanvasState::default();
        let mut doc = DocState::new_blank();
        graphdraw_core::edit::pages::add_after(&mut doc.editor, 0);
        for _ in 0..2 {
            canvas_frame(&ctx, &mut cs, &mut doc, Tool::Artboard, vec![]);
        }
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(1366.0, 768.0));
        let (pw, ph) = doc.page_size();
        let start = cs.screen(rect, ph, Point::new(pw, 0.0));
        // 1 mm além do A5: o encaixe corrige.
        let end = cs.screen(rect, ph, Point::new(149_000.0, ph - 211_000.0));
        let btn = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        canvas_frame(
            &ctx,
            &mut cs,
            &mut doc,
            Tool::Artboard,
            vec![egui::Event::PointerMoved(start)],
        );
        canvas_frame(
            &ctx,
            &mut cs,
            &mut doc,
            Tool::Artboard,
            vec![btn(start, true)],
        );
        let mut saw_snap = false;
        for k in 1..=10 {
            let t = k as f32 / 10.0;
            let pos = start + (end - start) * t;
            canvas_frame(
                &ctx,
                &mut cs,
                &mut doc,
                Tool::Artboard,
                vec![egui::Event::PointerMoved(pos)],
            );
            if let Drag::Artboard {
                snapped: Some("A5"),
                ..
            } = cs.drag
            {
                saw_snap = true;
            }
        }
        assert!(saw_snap, "o selo 'Encaixado em: ISO A5' apareceu");
        canvas_frame(
            &ctx,
            &mut cs,
            &mut doc,
            Tool::Artboard,
            vec![btn(end, false)],
        );
        canvas_frame(&ctx, &mut cs, &mut doc, Tool::Artboard, vec![]);
        for p in &doc.doc().data.pages {
            assert!((p.width - 148_000.0).abs() < 1.0, "{}", p.width);
            assert!((p.height - 210_000.0).abs() < 1.0, "{}", p.height);
        }
        doc.editor.undo();
        assert!(doc.doc().data.pages.iter().all(|p| p.width == 210_000.0));
    }

    /// Cada ferramenta tem o próprio cursor; alças da seleção usam os de
    /// redimensionar do sistema.
    #[test]
    fn each_tool_has_its_cursor() {
        let cs = CanvasState::default();
        let doc = DocState::new_blank();
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        let pos = Pos2::new(400.0, 300.0);
        let name = |g: Glyph| match g {
            Glyph::Icon { name, .. } => name.to_string(),
            Glyph::Cross { badge } => format!("mira+{badge}"),
            Glyph::System(c) => format!("{c:?}"),
        };
        let got: Vec<String> = Tool::ALL
            .iter()
            .map(|t| name(cs.cursor_glyph(*t, false, &doc, pos, rect, 297_000.0)))
            .collect();
        assert_eq!(
            got,
            [
                "cur-select",
                "cur-shape",
                "cur-zoom",
                "cur-pan",
                "mira+tool-bezier",
                "mira+tool-rect",
                "mira+tool-ellipse",
                "mira+tool-polygon",
                "mira+tool-text",
                "cur-select"
            ]
        );
        assert_eq!(
            name(cs.cursor_glyph(Tool::Select, true, &doc, pos, rect, 297_000.0)),
            "cur-pan",
            "espaço = mão"
        );
    }
}
