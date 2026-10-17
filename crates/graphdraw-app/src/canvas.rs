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
    /// Preenchimento interativo: gradiente editado na tela (GD-FIL-11,
    /// GD-FER-50).
    Gradient,
    /// Preenchimento uniforme: clicar num objeto abre a janela de
    /// preenchimento em cor sólida.
    SolidFill,
    /// Conta-gotas: copia preenchimento e cor do contorno do objeto clicado
    /// para a seleção (ou para os novos objetos, sem seleção).
    Eyedropper,
    /// Faca (GD-FER-17): arrastar corta em reta; Alt + arrastar, à mão livre.
    Knife,
    /// Recortar (GD-FER-16): retângulo; Enter ou duplo clique aplica.
    Crop,
    /// Borracha (GD-FER-18): arrastar apaga a área sob o pincel.
    Eraser,
    /// Excluir segmento virtual (GD-FER-19): clique no trecho.
    VirtualDelete,
    /// Mão livre (GD-FER-20): arrastar desenha; a suavização sai da barra.
    Freehand,
    /// Polilinha (GD-FER-25): cliques = retas; arrastar = trecho livre.
    Polyline,
    /// Linha 2 pontos (GD-FER-21): livre, tangente ou perpendicular.
    Line2,
    /// Curva 3 pontos (GD-FER-26): início e fim arrastando; clique dá a curvatura.
    Curve3,
}

/// Modo da linha 2 pontos (GD-FER-21).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineMode {
    #[default]
    Free,
    /// Termina tangente ao objeto sob o cursor.
    Tangent,
    /// Sai perpendicular ao objeto onde começa.
    Perpendicular,
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
            Tool::Gradient => (
                "tool-gradient",
                crate::keymap::hint(
                    tr(
                        "Preenchimento interativo — arraste sobre o objeto para criar o gradiente; arraste alças e paradas; duplo clique na linha cria parada",
                        "Interactive fill — drag over the object to create a gradient; drag handles and stops; double-click the line to add a stop",
                    ),
                    crate::keymap::Command::ToolInteractiveFill,
                ),
                "G",
            ),
            Tool::SolidFill => (
                "tool-fill-solid",
                crate::keymap::hint(
                    tr(
                        "Preenchimento uniforme — clique num objeto para escolher a cor sólida",
                        "Uniform fill — click an object to choose its solid color",
                    ),
                    crate::keymap::Command::ToolSolidFill,
                ),
                "",
            ),
            Tool::Eyedropper => (
                "tool-eyedropper",
                crate::keymap::hint(
                    tr(
                        "Conta-gotas — clique para amostrar a cor ou os atributos; o cursor vira balde: clique aplica (direito: contorno)",
                        "Eyedropper — click to sample a color or attributes; the cursor becomes a bucket: click applies (right: outline)",
                    ),
                    crate::keymap::Command::ToolEyedropper,
                ),
                "",
            ),
            Tool::Freehand => (
                "tool-freehand",
                crate::keymap::hint(
                    tr(
                        "Mão livre — arraste para desenhar; volte ao início para fechar",
                        "Freehand — drag to draw; return to the start to close",
                    ),
                    crate::keymap::Command::ToolFreehand,
                ),
                "",
            ),
            Tool::Polyline => (
                "tool-polyline",
                crate::keymap::hint(
                    tr(
                        "Polilinha — cliques fazem retas, arrastar faz trecho livre; Enter ou duplo clique termina",
                        "Polyline — clicks make straight lines, dragging makes a free stretch; Enter or double-click ends",
                    ),
                    crate::keymap::Command::ToolPolyline,
                ),
                "",
            ),
            Tool::Line2 => (
                "tool-line2",
                crate::keymap::hint(
                    tr(
                        "Linha 2 pontos — arraste de um ponto ao outro (Shift: 15°)",
                        "2-point line — drag from one point to the other (Shift: 15°)",
                    ),
                    crate::keymap::Command::ToolLine2,
                ),
                "",
            ),
            Tool::Curve3 => (
                "tool-curve3",
                crate::keymap::hint(
                    tr(
                        "Curva 3 pontos — arraste do início ao fim, depois clique para dar a curvatura",
                        "3-point curve — drag from start to end, then click to set the curvature",
                    ),
                    crate::keymap::Command::ToolCurve3,
                ),
                "",
            ),
            Tool::Knife => (
                "tool-knife",
                crate::keymap::hint(
                    tr(
                        "Faca — arraste através dos objetos para cortar em reta (Alt: à mão livre)",
                        "Knife — drag across objects to cut in a straight line (Alt: freehand)",
                    ),
                    crate::keymap::Command::ToolKnife,
                ),
                "",
            ),
            Tool::Crop => (
                "tool-crop",
                crate::keymap::hint(
                    tr(
                        "Recortar — arraste o retângulo; Enter ou duplo clique apaga o que está fora",
                        "Crop — drag the rectangle; Enter or double-click removes what is outside",
                    ),
                    crate::keymap::Command::ToolCrop,
                ),
                "",
            ),
            Tool::Eraser => (
                "tool-eraser",
                crate::keymap::hint(
                    tr(
                        "Borracha — arraste para apagar a área sob o pincel",
                        "Eraser — drag to erase the area under the brush",
                    ),
                    crate::keymap::Command::ToolEraser,
                ),
                "",
            ),
            Tool::VirtualDelete => (
                "tool-virtual-delete",
                crate::keymap::hint(
                    tr(
                        "Excluir segmento virtual — clique no trecho entre cruzamentos para apagá-lo",
                        "Virtual segment delete — click the stretch between crossings to delete it",
                    ),
                    crate::keymap::Command::ToolVirtualDelete,
                ),
                "",
            ),
        }
    }
    pub const ALL: [Tool; 21] = [
        Tool::Select,
        Tool::Shape,
        Tool::Zoom,
        Tool::Pan,
        Tool::Bezier,
        Tool::Rect,
        Tool::Ellipse,
        Tool::Polygon,
        Tool::Text,
        Tool::Gradient,
        Tool::SolidFill,
        Tool::Eyedropper,
        Tool::Artboard,
        Tool::Knife,
        Tool::Crop,
        Tool::Eraser,
        Tool::VirtualDelete,
        Tool::Freehand,
        Tool::Polyline,
        Tool::Line2,
        Tool::Curve3,
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
    /// Família da ferramenta Polígono (GD-FER-32): `polygon` (polígono e
    /// estrelas), `spiral` ou `graph` (papel gráfico).
    pub shape_family: String,
    /// Espiral (GD-PRI-05): voltas, logarítmica e fator de expansão por volta.
    pub spiral_turns: f64,
    pub spiral_log: bool,
    pub spiral_expansion: f64,
    /// Papel gráfico (GD-PRI-06): colunas e linhas.
    pub grid_cols: u32,
    pub grid_rows: u32,
}

impl Default for Defaults {
    fn default() -> Self {
        Defaults {
            fill: None,
            stroke: None,
            polygon_sides: 5,
            polygon_mode: "polygon".into(),
            polygon_sharpness: 0.5,
            shape_family: "polygon".into(),
            spiral_turns: 4.0,
            spiral_log: false,
            spiral_expansion: 1.5,
            grid_cols: 4,
            grid_rows: 3,
        }
    }
}

/// Primitiva desenhada pelo arrasto (ou pelos 3 pontos).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Prim {
    Rect,
    Ellipse,
    Polygon,
    Spiral,
    Grid,
}

impl Prim {
    fn of(tool: Tool, d: &Defaults) -> Prim {
        match tool {
            Tool::Ellipse => Prim::Ellipse,
            Tool::Polygon => match d.shape_family.as_str() {
                "spiral" => Prim::Spiral,
                "graph" => Prim::Grid,
                _ => Prim::Polygon,
            },
            _ => Prim::Rect,
        }
    }
}

/// Botão direito pressionado (GD-BDR-01).
#[derive(Clone)]
struct RightPress {
    start: Pos2,
    time: f64,
    /// Objeto sob o cursor ao pressionar.
    over: Option<String>,
    dragging: bool,
}

/// Arrasto direito menor que isto (px de tela) é clique (GD-BDR-01).
const RIGHT_DRAG_PX: f32 = 4.0;
/// Parado mais que isto (s) é toque longo, não clique (GD-BDR-01).
const LONG_PRESS_S: f64 = 0.5;

#[derive(Clone)]
enum MenuTarget {
    Ctx(crate::ctxmenu::Target),
    Guide(graphdraw_core::edit::guides::GuideRef),
}

/// Menu do botão direito aberto: ponto da tela, alvo, ponto da página e
/// quadro em que abriu.
#[derive(Clone)]
struct Menu {
    pos: Pos2,
    target: MenuTarget,
    at: Point,
    opened: u64,
}

enum Drag {
    None,
    /// Faca: pontos do traço (reta = início e ponto atual).
    Knife {
        pts: Vec<Point>,
        free: bool,
    },
    /// Retângulo do recorte sendo puxado.
    CropRect {
        start: Point,
        cur: Point,
    },
    /// Pincel da borracha.
    Erase {
        pts: Vec<Point>,
    },
    /// Traço de mão livre (também o trecho livre da polilinha).
    Freehand {
        pts: Vec<Point>,
        poly: bool,
    },
    /// Linha 2 pontos: início (já encaixado) e fim; normal do objeto no
    /// início, no modo perpendicular.
    Line2 {
        start: Point,
        cur: Point,
        normal: Option<kurbo::Vec2>,
    },
    /// Base da curva 3 pontos sendo puxada.
    Curve3Base {
        start: Point,
        cur: Point,
    },
    /// Arrasto com o botão direito (GD-BDR-20): a seleção não se move até a
    /// escolha no menu; `over` é o alvo sob o cursor.
    RightMove {
        ids: Vec<String>,
        start: Point,
        cur: Point,
        over: Option<String>,
    },
    /// Alça ou parada do gradiente sendo arrastada (matriz local → página).
    GradHandle {
        handle: graphdraw_core::edit::gradtool::Handle,
        r: ObjRef,
        original: Object,
        m: Affine,
    },
    /// Novo gradiente linear sendo puxado sobre o objeto.
    GradNew {
        start: Point,
        r: ObjRef,
        original: Object,
        m: Affine,
    },
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
    /// Vértice de polígono/estrela sendo arrastado (GD-PRI-03): o externo
    /// muda tamanho e giro de todas as pontas, o interno a nitidez.
    PolyVertex {
        inner: bool,
        r: ObjRef,
        original: Object,
    },
    /// Quadro de texto de parágrafo sendo desenhado.
    TextFrame {
        start: Point,
    },
    /// Guia nova saindo da régua (horizontal = régua de cima).
    GuideNew {
        horizontal: bool,
    },
    /// Guia existente sendo arrastada.
    GuideMove {
        r: graphdraw_core::edit::guides::GuideRef,
        original: graphdraw_core::model::Guide,
        grab: Point,
    },
    /// Origem (zero) das réguas sendo arrastada do canto.
    RulerOrigin,
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
    right: Option<RightPress>,
    menu: Option<Menu>,
    /// Tipo do nó e se há dois extremos escolhidos (menu de nó).
    menu_node: Option<(NodeKind, bool)>,
    /// Cópias deixadas no mover atual (GD-BDR-70).
    stamped: usize,
    /// Há o que colar (a área de transferência fica no App).
    pub can_paste: bool,
    /// Lacuna da faca e largura do pincel da borracha (µm).
    pub knife_gap: f64,
    pub eraser_width: f64,
    /// Recorte esperando Enter ou duplo clique (página).
    pub crop_pending: Option<kurbo::Rect>,
    /// Mensagem para a barra de status no fim do quadro.
    note: Option<String>,
    /// Suavização da mão livre (0–100) e modo da linha 2 pontos.
    pub freehand_smooth: f64,
    pub line_mode: LineMode,
    /// Base da curva 3 pontos, esperando o clique da curvatura.
    curve3: Option<(Point, Point)>,
    /// Conta-gotas (doc 27).
    pub eyedrop: crate::eyedrop::Eyedrop,
    /// Último quadro renderizado (RGBA pré-multiplicado), guardado só com o
    /// conta-gotas ativo: a lupa e as médias leem dele.
    pixels: Option<(RenderKey, std::sync::Arc<Vec<u8>>)>,
    /// Modo foco (GD-BDR-82): só estes objetos (e o que há dentro deles)
    /// aparecem e respondem a cliques; não muda o documento.
    pub focus: Option<Vec<String>>,
    /// "À frente de…" (`true`) / "Atrás de…" esperando o clique na
    /// referência (GD-BDR-41).
    pub pick_reference: Option<bool>,
    /// Páginas vizinhas lado a lado (GD-PAG-04): a anterior à esquerda e a
    /// seguinte à direita, só para ver; clicar numa delas vai para ela.
    pub spread: bool,
    /// Miniaturas das vizinhas: (ID da página, revisão, lado em px, textura).
    spread_tex: Vec<(String, u64, u32, egui::TextureHandle)>,
    /// Área de cada vizinha na tela no último quadro.
    spread_rects: Vec<(usize, Rect)>,
    /// Documento das miniaturas (endereço do estado): outra aba, outro cache.
    spread_doc: usize,
    pub canvas_size: Vec2,
    /// Segurar Espaço vira a mão (quando o perfil de atalhos não usa o Espaço).
    pub space_pans: bool,
    /// Prancheta solta com objetos na página: falta escolher manter ou
    /// escalar (GD-DIM-25). Novo retângulo da página e onde abrir a escolha.
    artboard_pending: Option<(kurbo::Rect, Pos2)>,
    /// Guias inteligentes ao mover (GD-ALI-05).
    pub smart_guides: bool,
    smart: Option<(kurbo::Rect, graphdraw_core::edit::smart::Targets)>,
    smart_lines: Vec<graphdraw_core::edit::smart::SmartLine>,
    /// Ponto do desenho já encaixado pelas guias inteligentes.
    smart_point: Option<Point>,
    /// Parada de gradiente escolhida (ferramenta de preenchimento interativo).
    /// (objeto, índice): vale só enquanto aquele objeto está selecionado e
    /// o índice existe.
    pub grad_stop: Option<(String, usize)>,
    /// Revisão do documento quando a parada foi escolhida: mudou por fora
    /// (desfazer, outra ferramenta), a escolha deixa de valer.
    grad_rev: u64,
    /// Balão de cor da parada aberto: posição na tela e o quadro em que
    /// abriu. Enquanto aberto, as mudanças formam um passo de desfazer.
    grad_popup: Option<(Pos2, u64)>,
    /// Réguas e guias (GD-REG-01, GD-GUI-01).
    pub show_rulers: bool,
    pub show_guides: bool,
    pub guides_locked: bool,
    /// Zero das réguas (µm na página).
    pub ruler_origin: Point,
    /// Guias que valem na página atual (cache do quadro).
    guides: Vec<graphdraw_core::model::Guide>,
    /// Texto em edição (ferramenta Texto).
    pub text: Option<crate::textedit::TextEditing>,
    /// Retângulo e elipse no modo 3 pontos (GD-FER-30/31).
    pub three_point: bool,
    /// Parâmetros do próximo polígono (para a prévia do arrasto).
    poly_preview: (u32, String, f64),
    /// Atributos do próximo objeto, para a prévia de espiral e papel gráfico.
    shape_preview: Defaults,
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
            right: None,
            menu: None,
            menu_node: None,
            stamped: 0,
            can_paste: false,
            knife_gap: 0.0,
            eraser_width: 5_000.0,
            crop_pending: None,
            note: None,
            freehand_smooth: 50.0,
            line_mode: LineMode::Free,
            curve3: None,
            eyedrop: Default::default(),
            pixels: None,
            focus: None,
            pick_reference: None,
            spread: false,
            spread_tex: Vec::new(),
            spread_rects: Vec::new(),
            spread_doc: 0,
            canvas_size: Vec2::new(800.0, 600.0),
            space_pans: true,
            artboard_pending: None,
            text: None,
            smart_guides: true,
            smart: None,
            smart_lines: Vec::new(),
            smart_point: None,
            grad_stop: None,
            grad_rev: 0,
            grad_popup: None,
            show_rulers: true,
            show_guides: true,
            guides_locked: false,
            ruler_origin: Point::ORIGIN,
            guides: Vec::new(),
            three_point: false,
            poly_preview: (5, "polygon".into(), 0.5),
            shape_preview: Defaults::default(),
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
    /// Comando escolhido num menu do botão direito (o App executa).
    pub command: Option<crate::keymap::Command>,
    /// "Colar" do menu da área vazia: no ponto clicado (página).
    pub paste_at: Option<Point>,
    pub status: Option<String>,
    /// Abrir a janela de preenchimento em cor sólida (ferramenta
    /// Preenchimento uniforme).
    pub open_solid_fill: bool,
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

    /// Guia sob o ponto (até `handle-hit` px), com a referência dela.
    fn guide_at(
        &self,
        doc: &DocState,
        p: Point,
    ) -> Option<(
        graphdraw_core::edit::guides::GuideRef,
        graphdraw_core::model::Guide,
    )> {
        let tol = theme::HANDLE_HIT as f64 / self.view.zoom.max(1e-12);
        graphdraw_core::edit::guides::refs(&doc.editor, doc.page)
            .into_iter()
            .filter(|(_, g)| !g.locked)
            .map(|(r, g)| (g.distance(p), r, g))
            .filter(|(d, _, _)| *d <= tol)
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, r, g)| (r, g))
    }

    /// Guias da página, mais a guia sendo criada ou movida.
    fn paint_guides(&self, painter: &egui::Painter, rect: Rect, ph: f64) {
        let view = self.view;
        let to_page = |pos: Pos2| -> Point {
            let l = pos - rect.min;
            let (x, y) = view.screen_to_page(ph, l.x as f64, l.y as f64);
            Point::new(x, y)
        };
        let area = kurbo::Rect::from_points(to_page(rect.left_bottom()), to_page(rect.right_top()));
        let draw = |g: &graphdraw_core::model::Guide, active: bool| {
            let Some((a, b)) = g.clip(area) else { return };
            let mut c = g
                .color
                .as_ref()
                .map(crate::widgets::to_color32)
                .unwrap_or(theme::GUIDE_USER);
            if g.locked {
                c = Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), theme::GUIDE_LOCKED_ALPHA);
            }
            let w = if active {
                theme::FOCUS_WIDTH
            } else {
                theme::GUIDE_WIDTH
            };
            painter.line_segment(
                [self.screen(rect, ph, a), self.screen(rect, ph, b)],
                EStroke::new(w, c),
            );
        };
        // Durante o arrasto, a guia movida aparece na posição nova.
        let moving = match &self.drag {
            Drag::GuideMove { original, grab, .. } => self
                .cursor_page
                .map(|c| (original.clone(), moved_guide(original, *grab, c))),
            _ => None,
        };
        for g in &self.guides {
            if moving.as_ref().is_some_and(|(o, _)| o == g) {
                continue;
            }
            draw(g, false);
        }
        if let Some((_, g)) = &moving {
            draw(g, true);
        }
        if let (Drag::GuideNew { horizontal }, Some(c)) = (&self.drag, self.cursor_page) {
            let p = self.snap_hit.map_or(c, |s| s.point);
            let g = if *horizontal {
                graphdraw_core::model::Guide::horizontal(p.y)
            } else {
                graphdraw_core::model::Guide::vertical(p.x)
            };
            draw(&g, true);
        }
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

    /// Entra no modo foco com a seleção, ou sai dele (GD-BDR-82).
    pub fn toggle_focus(&mut self, doc: &DocState) {
        self.focus = match self.focus {
            Some(_) => None,
            None if !doc.selection.is_empty() => Some(doc.selection.clone()),
            None => None,
        };
        self.rendered = None;
    }

    /// Espera o clique no objeto de referência de "À frente de…/Atrás de…".
    pub fn start_pick_reference(&mut self, above: bool) {
        self.pick_reference = Some(above);
    }

    /// IDs visíveis no modo foco (os escolhidos e tudo o que há dentro).
    fn focus_ids(&self, doc: &DocState) -> Option<std::collections::HashSet<String>> {
        let ids = self.focus.as_ref()?;
        let mut out = std::collections::HashSet::new();
        for id in ids {
            if let Some(o) = doc.editor.find(id).and_then(|r| doc.editor.get(&r)) {
                o.walk(&mut |c| {
                    out.insert(c.id.clone());
                });
            }
        }
        Some(out)
    }

    // ---- botão direito (doc 26) ---------------------------------------------

    /// Ponto da página sob a posição de tela.
    fn page_at(&self, rect: Rect, ph: f64, pos: Pos2) -> Point {
        let l = pos - rect.min;
        let (x, y) = self.view.screen_to_page(ph, l.x as f64, l.y as f64);
        Point::new(x, y)
    }

    /// Gestos do botão direito (GD-BDR-01/02): clique (< 4 px e < 500 ms)
    /// abre o menu do alvo; arrasto (≥ 4 px) a partir de um objeto leva a
    /// prévia fantasma e abre o menu ao soltar; com o esquerdo pressionado,
    /// carimba cópia (mover) ou cancela a ação (GD-BDR-70/71).
    #[allow(clippy::too_many_arguments)]
    fn right_button(
        &mut self,
        ui: &egui::Ui,
        doc: &mut DocState,
        tool: Tool,
        rect: Rect,
        tol: f64,
        mods: egui::Modifiers,
        blocked: bool,
        ev: &mut CanvasEvents,
    ) {
        let ph = doc.page_size().1;
        let (pressed, released, down, primary_down, pos, now) = ui.input(|i| {
            (
                i.pointer.button_pressed(PointerButton::Secondary),
                i.pointer.button_released(PointerButton::Secondary),
                i.pointer.button_down(PointerButton::Secondary),
                i.pointer.primary_down(),
                i.pointer.interact_pos().or(i.pointer.hover_pos()),
                i.time,
            )
        });
        let Some(pos) = pos else { return };
        // Conta-gotas: o direito na lupa é dela (GD-CTG-10).
        if pressed && tool == Tool::Eyedropper && self.eyedrop.over_hud(pos) {
            self.right = None;
            return;
        }
        if pressed && rect.contains(pos) {
            if primary_down || self.is_dragging() && !matches!(self.drag, Drag::RightMove { .. }) {
                // GD-BDR-02: com ação em curso, nunca abre menu.
                self.right_during_left(doc, ev);
                self.right = None;
                return;
            }
            self.menu = None;
            let p = self.page_at(rect, ph, pos);
            let over = if blocked || matches!(tool, Tool::Zoom | Tool::Pan | Tool::Eyedropper) {
                None
            } else {
                self.pick_focus(doc, p, tol, mods.ctrl)
            };
            self.right = Some(RightPress {
                start: pos,
                time: now,
                over,
                dragging: false,
            });
        }
        let Some(rp) = self.right.clone() else { return };
        if down && !rp.dragging && (pos - rp.start).length() >= RIGHT_DRAG_PX {
            if let Some(id) = rp.over.clone() {
                if !doc.selection.contains(&id) {
                    doc.selection = vec![id];
                }
                let ids: Vec<String> = doc
                    .selection
                    .iter()
                    .filter(|i| {
                        doc.editor
                            .find(i)
                            .and_then(|r| doc.editor.get(&r))
                            .is_some_and(|o| !o.locked)
                    })
                    .cloned()
                    .collect();
                if !ids.is_empty() {
                    let start = self.page_at(rect, ph, rp.start);
                    self.drag = Drag::RightMove {
                        ids,
                        start,
                        cur: start,
                        over: None,
                    };
                }
            }
            if let Some(r) = &mut self.right {
                r.dragging = true;
            }
        }
        let here = self.page_at(rect, ph, pos);
        if let Drag::RightMove { ids, cur, over, .. } = &mut self.drag {
            *cur = here;
            // Alvo: o objeto mais acima sob o cursor, fora da seleção.
            *over = pick(doc, here, tol, mods.ctrl).filter(|id| !ids.contains(id));
        }
        if released {
            self.right = None;
            if rp.dragging {
                if let Drag::RightMove {
                    start, cur, over, ..
                } = std::mem::replace(&mut self.drag, Drag::None)
                {
                    let target = crate::ctxmenu::Target::Drop {
                        over,
                        delta: cur - start,
                    };
                    self.open_menu(pos, MenuTarget::Ctx(target), cur, ui.ctx());
                }
                return;
            }
            let quick = (pos - rp.start).length() < RIGHT_DRAG_PX && now - rp.time < LONG_PRESS_S;
            if quick && tool == Tool::Eyedropper {
                // Conta-gotas: o direito despeja no contorno (GD-CTG-06).
                let p = self.page_at(rect, ph, pos);
                if !blocked
                    && let Some(msg) = self.eyedrop_click(doc, p, pos, mods, tol, rect, true)
                {
                    ev.status = Some(msg);
                }
            } else if quick {
                self.right_click(ui, doc, tool, rect, pos, tol, mods, blocked);
            }
        }
    }

    /// Clique direito: menu do que está sob o cursor (GD-BDR-40..51).
    #[allow(clippy::too_many_arguments)]
    fn right_click(
        &mut self,
        ui: &egui::Ui,
        doc: &mut DocState,
        tool: Tool,
        rect: Rect,
        pos: Pos2,
        tol: f64,
        mods: egui::Modifiers,
        blocked: bool,
    ) {
        let ph = doc.page_size().1;
        let p = self.page_at(rect, ph, pos);
        // GD-BDR-03: Zoom afasta.
        if tool == Tool::Zoom {
            let l = pos - rect.min;
            self.view.zoom_at(0.5, (l.x as f64, l.y as f64));
            return;
        }
        if blocked {
            return;
        }
        // Ferramenta Forma: nó ou segmento da curva selecionada (GD-BDR-50/51).
        if tool == Tool::Shape
            && let Some(ps) = single_path(doc)
        {
            let (m, path) = (ps.m, ps.path.clone());
            let hit_node = nodes::all_nodes(&path).into_iter().find(|r| {
                nodes::node(&path, *r).is_some_and(|n| {
                    (self.screen(rect, ph, m * pt(n.p)) - pos).length() <= theme::HANDLE_HIT
                })
            });
            if let Some(r) = hit_node {
                // GD-BDR-52: fora da seleção de nós, age só nele.
                if !doc.nodes.contains(&r) {
                    doc.nodes = vec![r];
                }
                let kind = nodes::node(&path, r).map(|n| n.kind).unwrap_or_default();
                let ends =
                    doc.nodes.len() == 2 && doc.nodes.iter().all(|n| nodes::is_endpoint(&path, *n));
                self.open_menu(
                    pos,
                    MenuTarget::Ctx(crate::ctxmenu::Target::Node(r)),
                    p,
                    ui.ctx(),
                );
                self.menu_node = Some((kind, ends));
                return;
            }
            let scale = m.determinant().abs().sqrt().max(1e-12);
            if let Some(n) =
                geometry::nearest(&path, m.inverse() * p).filter(|n| n.distance * scale <= tol)
            {
                self.open_menu(
                    pos,
                    MenuTarget::Ctx(crate::ctxmenu::Target::Segment(n.seg, n.t)),
                    p,
                    ui.ctx(),
                );
                return;
            }
        }
        if let Some(id) = self.pick_focus(doc, p, tol, mods.ctrl) {
            // Clique direito sobre objeto fora da seleção: seleciona só ele.
            if !doc.selection.contains(&id) {
                doc.selection = vec![id.clone()];
            }
            self.open_menu(
                pos,
                MenuTarget::Ctx(crate::ctxmenu::Target::Object(id)),
                p,
                ui.ctx(),
            );
            return;
        }
        // Bloqueado: o esquerdo passa reto; só o direito chega nele, com o
        // menu de desbloquear e ordem. A seleção só muda ao escolher.
        if let Some(id) = query::hit_test_locked(doc.page(), p, tol)
            .filter(|id| self.focus_ids(doc).is_none_or(|f| f.contains(id)))
        {
            self.open_menu(
                pos,
                MenuTarget::Ctx(crate::ctxmenu::Target::Object(id)),
                p,
                ui.ctx(),
            );
            return;
        }
        if self.show_guides
            && let Some((r, _)) = self.guide_at(doc, p)
        {
            self.open_menu(pos, MenuTarget::Guide(r), p, ui.ctx());
            return;
        }
        self.open_menu(
            pos,
            MenuTarget::Ctx(crate::ctxmenu::Target::Empty),
            p,
            ui.ctx(),
        );
    }

    fn open_menu(&mut self, pos: Pos2, target: MenuTarget, at: Point, ctx: &egui::Context) {
        self.menu = Some(Menu {
            pos,
            target,
            at,
            opened: ctx.cumulative_frame_nr(),
        });
    }

    /// Objeto sob o cursor respeitando o modo foco.
    fn pick_focus(&self, doc: &DocState, p: Point, tol: f64, deep: bool) -> Option<String> {
        let id = pick(doc, p, tol, deep)?;
        match self.focus_ids(doc) {
            Some(f) if !f.contains(&id) => None,
            _ => Some(id),
        }
    }

    /// Botão direito durante ação com o esquerdo: mover carimba cópia
    /// (GD-BDR-70); o resto cancela (GD-BDR-71).
    fn right_during_left(&mut self, doc: &mut DocState, ev: &mut CanvasEvents) {
        match &self.drag {
            Drag::Move { .. } => {
                let sel = doc.selection.clone();
                ops::transform_copies(&mut doc.editor, &sel, Affine::IDENTITY, 1);
                self.stamped += 1;
                doc.editor.rename_open(format!(
                    "{} {}",
                    tr("Mover e copiar", "Move and copy"),
                    self.stamped
                ));
                ev.status = Some(format!(
                    "{} {}",
                    self.stamped,
                    tr("cópia(s) deixada(s)", "copy(ies) left")
                ));
            }
            Drag::None => {}
            _ => self.cancel_action(doc),
        }
    }

    /// Cancela a ação em curso (GD-BDR-71/72): volta ao estado de antes de
    /// pressionar o esquerdo; nada entra no histórico.
    pub fn cancel_action(&mut self, doc: &mut DocState) {
        if matches!(self.drag, Drag::BezierHandle) {
            // Desenhando com Bézier: só o nó que estava sendo posto sai.
            if let Some(b) = &mut self.bezier {
                b.nodes.pop();
                if b.nodes.is_empty() {
                    self.bezier = None;
                }
            }
            self.drag = Drag::None;
            return;
        }
        if doc.editor.in_transaction() {
            doc.editor.cancel();
        }
        self.drag = Drag::None;
        self.smart_lines.clear();
        self.snap_hit = None;
    }

    /// Esc: fecha menu, cancela arrasto direito ou escolha de referência, sai
    /// do modo foco (GD-BDR-04/20/41/82). `true` se usou o Esc.
    pub fn escape(&mut self, doc: &mut DocState) -> bool {
        if self.menu.take().is_some() {
            return true;
        }
        if matches!(self.drag, Drag::RightMove { .. }) {
            self.drag = Drag::None;
            self.right = None;
            return true;
        }
        if self.pick_reference.take().is_some() {
            return true;
        }
        if self.curve3.take().is_some() {
            return true;
        }
        if self.crop_pending.take().is_some()
            || matches!(
                self.drag,
                Drag::Knife { .. } | Drag::Erase { .. } | Drag::CropRect { .. }
            )
        {
            self.drag = Drag::None;
            return true;
        }
        if self.eyedrop.active && self.eyedrop.empty() {
            return true;
        }
        if self.focus.take().is_some() {
            self.rendered = None;
            let _ = doc;
            return true;
        }
        false
    }

    /// Menu aberto, no ponto do cursor e dentro da tela (GD-BDR-04).
    fn context_menu_ui(&mut self, ctx: &egui::Context, doc: &mut DocState, ev: &mut CanvasEvents) {
        use crate::ctxmenu::{self, Target};
        let Some(m) = self.menu.clone() else { return };
        let mut chosen = None;
        let mut close = false;
        let focus = self.focus.is_some();
        let area = egui::Area::new(egui::Id::unique("menu-botao-direito"))
            .order(egui::Order::Foreground)
            .fixed_pos(m.pos)
            .constrain(true)
            .show(ctx, |ui| {
                egui::Frame::menu(ui.style()).show(ui, |ui| {
                    theme::menu_density(ui);
                    ui.set_min_width(theme::CONTEXT_MENU_WIDTH);
                    match &m.target {
                        MenuTarget::Guide(r) => {
                            if crate::guideui::guide_menu(ui, doc, *r) {
                                close = true;
                            }
                        }
                        MenuTarget::Ctx(t) => {
                            let entries = match t {
                                Target::Object(id) => {
                                    if let Some(o) = doc
                                        .editor
                                        .find(id)
                                        .and_then(|r| doc.editor.get(&r))
                                        .cloned()
                                        && !o.locked
                                    {
                                        if let Some(a) = crate::layertree::name_field(ui, &o) {
                                            crate::layertree::apply(doc, a);
                                            close = true;
                                        }
                                        ui.separator();
                                    }
                                    ctxmenu::object_entries(doc, id, focus)
                                }
                                Target::Empty => ctxmenu::empty_entries(self.can_paste, focus),
                                Target::Node(_) => {
                                    let (k, ends) =
                                        self.menu_node.unwrap_or((NodeKind::Corner, false));
                                    ctxmenu::node_entries(k, ends)
                                }
                                Target::Segment(..) => ctxmenu::segment_entries(),
                                Target::Drop { over, .. } => ctxmenu::drop_entries(over.is_some()),
                            };
                            chosen = ctxmenu::show(ui, &entries);
                        }
                    }
                });
            });
        // Fora: nem no menu nem num submenu (camadas da frente).
        let on_popup = ctx
            .pointer_interact_pos()
            .and_then(|p| ctx.layer_id_at(p))
            .is_some_and(|l| matches!(l.order, egui::Order::Foreground | egui::Order::Tooltip));
        let outside = ctx.input(|i| i.pointer.any_pressed())
            && !area.response.contains_pointer()
            && !on_popup
            && ctx.cumulative_frame_nr() > m.opened + 1;
        if let Some(c) = chosen {
            self.menu = None;
            self.exec_menu(c, &m, doc, ev);
        } else if close || outside {
            self.menu = None;
        }
    }

    /// Executa o item escolhido (cada um uma transação, GD-BDR-05).
    fn exec_menu(
        &mut self,
        c: crate::ctxmenu::Cmd,
        m: &Menu,
        doc: &mut DocState,
        ev: &mut CanvasEvents,
    ) {
        use crate::ctxmenu::{Cmd, Target};
        use graphdraw_core::edit::shapeops::{self, BoolOp, BoolOptions};
        let target = match &m.target {
            MenuTarget::Ctx(t) => t.clone(),
            MenuTarget::Guide(_) => return,
        };
        // Objeto bloqueado: o comando (desbloquear, ordem) vale para ele.
        if let (Target::Object(id), Cmd::Run(_)) = (&target, &c)
            && doc
                .editor
                .find(id)
                .and_then(|r| doc.editor.get(&r))
                .is_some_and(|o| o.locked)
        {
            doc.selection = vec![id.clone()];
        }
        let sel = doc.selection.clone();
        match c {
            Cmd::Run(crate::keymap::Command::Paste) => ev.paste_at = Some(m.at),
            Cmd::Run(cmd) => ev.command = Some(cmd),
            Cmd::Cancel | Cmd::ClipInside | Cmd::ImageFrame | Cmd::LinkedInstance => {}
            Cmd::ExitFocus => {
                self.focus = None;
                self.rendered = None;
            }
            Cmd::NodeKind(k) => self.edit_path(
                doc,
                tr("Alterar tipo de nó", "Change node type"),
                |p, s| {
                    for r in s.iter() {
                        nodes::set_kind(p, *r, k);
                    }
                },
            ),
            Cmd::BreakNode => self.edit_path(doc, tr("Quebrar no nó", "Break at node"), |p, s| {
                let mut s = s.clone();
                s.sort();
                for r in s.into_iter().rev() {
                    nodes::break_at(p, r);
                }
            }),
            Cmd::JoinEnds => self.edit_path(doc, tr("Unir extremos", "Join ends"), |p, s| {
                if let [a, b] = s.as_slice() {
                    nodes::join(p, *a, *b, true);
                }
            }),
            Cmd::DeleteNode => self.edit_path(doc, tr("Excluir nó", "Delete node"), |p, s| {
                nodes::delete_nodes(p, s);
            }),
            Cmd::SegToCurve | Cmd::SegToLine | Cmd::AddNodeHere | Cmd::Reverse => {
                let Target::Segment(seg, t) = target else {
                    return;
                };
                let name = match c {
                    Cmd::SegToCurve => tr("Converter em curva", "To curve"),
                    Cmd::SegToLine => tr("Converter em reta", "To line"),
                    Cmd::AddNodeHere => tr("Adicionar nó", "Add node"),
                    _ => tr("Inverter direção", "Reverse direction"),
                };
                self.edit_path(doc, name, |p, _| match c {
                    Cmd::SegToCurve => nodes::to_curve(p, seg),
                    Cmd::SegToLine => nodes::to_line(p, seg),
                    Cmd::AddNodeHere => {
                        nodes::add_node(p, seg, t);
                    }
                    _ => nodes::reverse(p, seg.subpath),
                });
            }
            Cmd::SampleText => {
                let Target::Object(id) = target else { return };
                ops::modify(
                    &mut doc.editor,
                    &[id],
                    tr("Texto de exemplo", "Sample text"),
                    |o| match &mut o.kind {
                        ObjectKind::Text(t) => {
                            fill_sample_text(t);
                            true
                        }
                        _ => false,
                    },
                );
            }
            Cmd::UseAsTextFrame => {
                let Target::Object(id) = target else { return };
                let Some(b) = doc
                    .editor
                    .find(&id)
                    .and_then(|r| ops::object_bounds(&doc.editor, &r, false))
                else {
                    return;
                };
                let Some(layer) = doc.layer_for_new_object() else {
                    return;
                };
                let t = graphdraw_core::edit::text::new_text(
                    b.x0,
                    b.y1,
                    Some([b.width(), b.height()]),
                    &crate::textedit::default_family(),
                    graphdraw_core::edit::text::DEFAULT_SIZE / 2.0,
                );
                let tid = ops::add_object(
                    &mut doc.editor,
                    doc.page,
                    layer,
                    t,
                    tr("Quadro de texto", "Text frame"),
                );
                doc.selection = vec![tid.clone()];
                ev.switch_tool = Some(Tool::Text);
                self.edit_text(doc, tid, None);
            }
            Cmd::MoveHere | Cmd::CopyHere => {
                let Target::Drop { delta, .. } = target else {
                    return;
                };
                if c == Cmd::MoveHere {
                    ops::transform(
                        &mut doc.editor,
                        &sel,
                        Affine::translate(delta),
                        tr("Mover para cá", "Move here"),
                    );
                } else {
                    let copies =
                        ops::transform_copies(&mut doc.editor, &sel, Affine::translate(delta), 1);
                    doc.selection = copies;
                }
            }
            Cmd::CopyAll
            | Cmd::CopyStroke
            | Cmd::CopyFill
            | Cmd::CenterOnTarget
            | Cmd::Weld
            | Cmd::Trim
            | Cmd::Intersect => {
                let Target::Drop {
                    over: Some(over), ..
                } = target
                else {
                    return;
                };
                let Some(src) = sel.first().cloned() else {
                    return;
                };
                match c {
                    Cmd::CopyAll => {
                        ops::copy_properties(
                            &mut doc.editor,
                            &src,
                            std::slice::from_ref(&over),
                            ops::CopyProps::ALL,
                        );
                    }
                    Cmd::CopyStroke => {
                        ops::copy_properties(
                            &mut doc.editor,
                            &src,
                            std::slice::from_ref(&over),
                            ops::CopyProps::STROKE,
                        );
                    }
                    Cmd::CopyFill => {
                        ops::copy_properties(
                            &mut doc.editor,
                            &src,
                            std::slice::from_ref(&over),
                            ops::CopyProps::FILL,
                        );
                    }
                    Cmd::CenterOnTarget => {
                        ops::center_on(&mut doc.editor, &sel, &over);
                    }
                    _ => {
                        // GD-BDR-35: o alvo é o último; o resultado herda dele e
                        // os originais saem.
                        let op = match c {
                            Cmd::Weld => BoolOp::Weld,
                            Cmd::Trim => BoolOp::Trim,
                            _ => BoolOp::Intersect,
                        };
                        let mut ids = sel.clone();
                        ids.push(over);
                        let opts = BoolOptions {
                            keep_sources: false,
                            keep_target: false,
                        };
                        match shapeops::boolean(&mut doc.editor, &ids, op, opts) {
                            Some(n) => doc.selection = n,
                            None => {
                                ev.status = Some(
                                    tr(
                                        "Nada a fazer: os objetos não se sobrepõem",
                                        "Nothing to do: the objects do not overlap",
                                    )
                                    .into(),
                                )
                            }
                        }
                    }
                }
            }
        }
    }

    /// Aplica uma edição à curva selecionada sozinha (nós escolhidos em `s`).
    fn edit_path(
        &mut self,
        doc: &mut DocState,
        name: &str,
        f: impl FnOnce(&mut PathData, &mut Vec<NodeRef>),
    ) {
        let Some((loc, o)) = single_path(doc).map(|s| (s.r, s.obj.clone())) else {
            return;
        };
        let mut no = o;
        let mut s = doc.nodes.clone();
        let ObjectKind::Path(p) = &mut no.kind else {
            return;
        };
        f(p, &mut s);
        let empty = p.subpaths.is_empty();
        doc.editor.transaction(name, |ed| {
            if empty {
                ed.remove_at(&loc);
            } else {
                ed.replace_at(&loc, no);
            }
        });
        if empty {
            doc.selection.clear();
            doc.nodes.clear();
        } else {
            doc.nodes = s;
        }
    }

    /// Prévia fantasma do arrasto direito e realce do alvo (GD-BDR-20/21).
    fn paint_right_drag(&self, painter: &egui::Painter, doc: &DocState, rect: Rect, ph: f64) {
        let Drag::RightMove {
            ids,
            start,
            cur,
            over,
        } = &self.drag
        else {
            return;
        };
        let d = *cur - *start;
        let ghost = EStroke::new(theme::GUIDE_WIDTH, theme::ACCENT_PRIMARY);
        for id in ids {
            if let Some(b) = doc
                .editor
                .find(id)
                .and_then(|r| ops::object_bounds(&doc.editor, &r, false))
            {
                let r = self.screen_rect(rect, ph, b + d);
                painter.rect_filled(r, theme::RADIUS_NONE, theme::SELECTION_FILL);
                painter.rect_stroke(r, theme::RADIUS_NONE, ghost, egui::StrokeKind::Middle);
            }
        }
        if let Some(t) = over
            && let Some(b) = doc
                .editor
                .find(t)
                .and_then(|r| ops::object_bounds(&doc.editor, &r, false))
        {
            let r = self.screen_rect(rect, ph, b);
            painter.rect_stroke(
                r,
                theme::RADIUS_NONE,
                EStroke::new(theme::DROP_TARGET_WIDTH, theme::ACCENT_PRIMARY),
                egui::StrokeKind::Outside,
            );
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
        if self.shape_preview.shape_family != defaults.shape_family
            || self.shape_preview.spiral_turns != defaults.spiral_turns
            || self.shape_preview.spiral_log != defaults.spiral_log
            || self.shape_preview.spiral_expansion != defaults.spiral_expansion
            || (self.shape_preview.grid_cols, self.shape_preview.grid_rows)
                != (defaults.grid_cols, defaults.grid_rows)
        {
            self.shape_preview = Defaults {
                fill: None,
                stroke: None,
                ..defaults.clone()
            };
        }
        let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        self.canvas_size = rect.size();
        self.guides = if self.show_guides {
            graphdraw_core::edit::snap::page_guides(doc.doc(), doc.page)
        } else {
            Vec::new()
        };
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
        self.eyedrop.active = tool == Tool::Eyedropper;
        self.eyedrop.shift = mods.shift;
        self.eyedrop.now = ui.input(|i| i.time);
        self.eyedrop.new_stroke = Some(
            defaults
                .stroke
                .clone()
                .unwrap_or_else(crate::doc::default_stroke),
        );
        let bg = theme::palette(ui).bg_canvas;
        self.eyedrop.canvas_bg = [bg.r(), bg.g(), bg.b()];
        if !self.eyedrop.active {
            self.eyedrop.hud.rect = Rect::NOTHING;
            self.pixels = None;
        }
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
            let (rtop, rleft, rcorner) = crate::rulers::areas(rect);
            let on_ruler = self.show_rulers && (rtop.contains(origin) || rleft.contains(origin));
            let guide_hit = (tool == Tool::Select && !self.guides_locked && self.show_guides)
                .then(|| self.guide_at(doc, to_page(origin)))
                .flatten()
                .filter(|_| pick(doc, to_page(origin), tol, false).is_none());
            if self.show_rulers && rcorner.contains(origin) {
                self.drag = Drag::RulerOrigin;
            } else if on_ruler && !blocked {
                self.drag = Drag::GuideNew {
                    horizontal: rtop.contains(origin),
                };
            } else if let Some((r, g)) = guide_hit.filter(|_| !blocked) {
                self.drag = Drag::GuideMove {
                    r,
                    original: g,
                    grab: to_page(origin),
                };
            } else if panning_tool {
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
        // Arrasto com o esquerdo pelo estado do botão: o egui encerra o
        // arrasto quando o direito é tocado no meio (GD-BDR-70).
        let (left_down, left_up, ptr) = ui.input(|i| {
            (
                i.pointer.primary_down(),
                i.pointer.primary_released(),
                i.pointer.interact_pos().or(i.pointer.hover_pos()),
            )
        });
        let left_drag =
            self.is_dragging() && !matches!(self.drag, Drag::Pan | Drag::RightMove { .. });
        if (response.dragged_by(PointerButton::Primary) || (left_drag && left_down))
            && !matches!(self.drag, Drag::Pan)
            && let Some(cur) = response.interact_pointer_pos().or(ptr)
        {
            self.update_drag(doc, to_page(cur), mods, tol);
        }
        if response.drag_stopped_by(PointerButton::Primary) || (left_drag && left_up) {
            let end = response
                .interact_pointer_pos()
                .or(ptr)
                .or(hover)
                .unwrap_or(rect.center());
            self.end_drag(doc, to_page(end), end, mods, rect, ph, defaults);
        }
        // Clique numa página vizinha (lado a lado): vai para ela, e o clique
        // não chega à ferramenta (nada é criado na página que fica).
        let spread_hit =
            (self.spread && !panning_tool && (response.clicked() || response.double_clicked()))
                .then(|| response.interact_pointer_pos())
                .flatten()
                .and_then(|pos| {
                    self.spread_rects
                        .iter()
                        .find(|(_, r)| r.contains(pos))
                        .map(|(i, _)| *i)
                });
        if let Some(i) = spread_hit {
            self.end_text(doc);
            self.cancel(doc);
            doc.set_page(i);
            self.fit_pending = true;
        }
        if response.clicked() && blocked && spread_hit.is_none() {
            ev.status = Some(locked_msg());
        }
        if response.clicked()
            && spread_hit.is_none()
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
        // Amostra da paleta solta sobre um objeto: preenchimento (GD-PAL-05).
        if let Some(drop) = response.dnd_release_payload::<crate::paletteui::SwatchDrop>()
            && !blocked
            && let Some(pos) = hover
            && let Some(id) = pick(doc, to_page(pos), tol, false)
        {
            ops::set_fill(&mut doc.editor, &[id], Some(Fill::solid(drop.0.clone())));
        }
        // Botão direito (doc 26): gestos, menus e arrasto direito.
        self.right_button(ui, doc, tool, rect, tol, mods, blocked, &mut ev);
        self.context_menu_ui(ui.ctx(), doc, &mut ev);
        if self.show_rulers
            && response.double_clicked()
            && response
                .interact_pointer_pos()
                .is_some_and(|p| crate::rulers::areas(rect).2.contains(p))
        {
            self.ruler_origin = Point::ORIGIN;
        } else if response.double_clicked()
            && spread_hit.is_none()
            && !panning_tool
            && !blocked
            && let Some(pos) = response.interact_pointer_pos()
        {
            self.double_click(tool, doc, to_page(pos), tol, defaults, &mut ev);
        }
        if (matches!(tool, Tool::Bezier | Tool::Polyline) && self.bezier.is_some()
            || self.curve3.is_some())
            && self.cursor_page.is_some()
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
                let c = snap::candidates_doc(doc.doc(), doc.page, &self.snap, &[]);
                snap::snap_guided_on(&self.guides, p, &c, &self.snap, snap_radius(&self.view))
            });
        }

        // ---- desenho ----
        self.paint_spread(ui, &painter, doc, rect);
        self.paint_page(ui, &painter, doc, rect);
        self.paint_guides(&painter, rect, ph);
        for l in &self.smart_lines {
            let (a, b) = if l.vertical {
                (Point::new(l.at, l.from), Point::new(l.at, l.to))
            } else {
                (Point::new(l.from, l.at), Point::new(l.to, l.at))
            };
            let c = if l.page {
                theme::GUIDE_PAGE
            } else {
                theme::GUIDE_OBJECT
            };
            let (sa, sb) = (self.screen(rect, ph, a), self.screen(rect, ph, b));
            let st = EStroke::new(theme::GUIDE_WIDTH, c);
            painter.line_segment([sa, sb], st);
            // Espaçamento igual: traços nas pontas do vão.
            if l.spacing {
                let n = if l.vertical {
                    Vec2::new(theme::SPACING_TICK, 0.0)
                } else {
                    Vec2::new(0.0, theme::SPACING_TICK)
                };
                for p in [sa, sb] {
                    painter.line_segment([p - n, p + n], st);
                }
                // Distância ao vivo (DS 5.2), ao lado do meio do vão.
                let mm = (l.to - l.from).abs() / 1000.0;
                let mid = sa + (sb - sa) / 2.0;
                let (off, align) = if l.vertical {
                    (
                        Vec2::new(theme::SPACING_TICK * 1.5, 0.0),
                        egui::Align2::LEFT_CENTER,
                    )
                } else {
                    (
                        Vec2::new(0.0, -theme::SPACING_TICK * 1.5),
                        egui::Align2::CENTER_BOTTOM,
                    )
                };
                painter.text(
                    mid + off,
                    align,
                    format!("{} mm", crate::widgets::fmt_num(mm, 1)),
                    theme::font_micro(),
                    c,
                );
            }
        }
        self.paint_overlays(ui, &painter, doc, rect, tool);
        self.paint_right_drag(&painter, doc, rect, ph);
        self.eyedrop_overlay(ui, &painter, doc, rect, ph, tol, &mut ev);
        self.paint_cut_tools(ui, &painter, rect, ph, tool);
        if tool == Tool::Crop
            && self.crop_pending.is_some()
            && ui.input(|i| i.key_pressed(Key::Enter))
        {
            self.apply_crop(doc);
        }
        if let Some(n) = self.note.take() {
            ev.status = Some(n);
        }
        self.stop_popup(ui.ctx(), doc, tool);
        if tool == Tool::Artboard {
            self.paint_page_handles(&painter, doc, rect);
        }
        self.artboard_choice(ui, doc);
        if self.show_rulers {
            let to_screen = |q: Point| self.screen(rect, ph, q);
            let a = to_page(rect.left_bottom());
            let b = to_page(rect.right_top());
            let cursor = match &self.drag {
                Drag::GuideNew { .. } | Drag::GuideMove { .. } | Drag::RulerOrigin => {
                    self.cursor_page
                }
                _ => self.cursor_page,
            };
            crate::rulers::paint(
                ui,
                &painter,
                rect,
                &to_screen,
                kurbo::Rect::from_points(a, b),
                self.view.zoom,
                self.ruler_origin,
                cursor,
                {
                    let [paper, world] = doc.doc().data.settings.drawing_scale;
                    world / paper
                },
            );
        }

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

    /// Vizinhas da página ativa, lado a lado (GD-PAG-04), alinhadas pelo
    /// topo e afastadas por `spread-gap`.
    fn paint_spread(&mut self, ui: &egui::Ui, painter: &egui::Painter, doc: &DocState, rect: Rect) {
        self.spread_rects.clear();
        if !self.spread {
            return;
        }
        let doc_key = doc as *const DocState as usize;
        if doc_key != self.spread_doc {
            self.spread_tex.clear();
            self.spread_doc = doc_key;
        }
        // Editando a página ativa, as vizinhas não mudam: não refaz durante
        // o arrasto (a revisão sobe a cada quadro).
        let busy = self.is_dragging() || doc.editor.in_transaction();
        let pages = &doc.doc().data.pages;
        let (pw, ph) = doc.page_size();
        let gap = theme::SPREAD_GAP as f64 / self.view.zoom.max(1e-9);
        let mut wanted = Vec::new();
        if doc.page > 0 {
            let p = &pages[doc.page - 1];
            wanted.push((doc.page - 1, -gap - p.width, p.width, p.height));
        }
        if doc.page + 1 < pages.len() {
            let p = &pages[doc.page + 1];
            wanted.push((doc.page + 1, pw + gap, p.width, p.height));
        }
        let ppp = ui.ctx().pixels_per_point();
        for (i, x0, w, h) in wanted {
            let r = Rect::from_min_max(
                self.screen(rect, ph, Point::new(x0, ph)),
                self.screen(rect, ph, Point::new(x0 + w, ph - h)),
            );
            if !r.intersects(rect) {
                continue;
            }
            // Miniatura na resolução da tela, refeita ao mudar de zoom ou de
            // conteúdo (em degraus para não refazer a cada quadro).
            let side = ((r.width().max(r.height()) * ppp).ceil() as u32).clamp(16, 2048);
            let side = side.next_power_of_two().min(2048);
            let id = pages[i].id.clone();
            let rev = doc.editor.revision();
            let pos = self.spread_tex.iter().position(|t| t.0 == id);
            let fresh = pos.is_some_and(|k| {
                (self.spread_tex[k].1 == rev || busy) && self.spread_tex[k].2 == side
            });
            if !fresh {
                let img = doc.thumbnail(i, side as f64);
                let tex = ui.ctx().load_texture(
                    format!("vizinha-{id}"),
                    img,
                    egui::TextureOptions::LINEAR,
                );
                match pos {
                    Some(k) => self.spread_tex[k] = (id.clone(), rev, side, tex),
                    None => self.spread_tex.push((id.clone(), rev, side, tex)),
                }
            }
            let Some(t) = self.spread_tex.iter().find(|t| t.0 == id) else {
                continue;
            };
            painter.add(
                theme::palette(ui)
                    .shadow_page()
                    .as_shape(r, theme::RADIUS_NONE),
            );
            painter.image(
                t.3.id(),
                r,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
            painter.rect_filled(r, theme::RADIUS_NONE, theme::SPREAD_DIM);
            painter.text(
                r.left_top() - Vec2::new(0.0, theme::SPACE_100),
                egui::Align2::LEFT_BOTTOM,
                graphdraw_core::edit::pages::display_name(&pages[i], i),
                theme::font_micro(),
                theme::palette(ui).fg_muted,
            );
            self.spread_rects.push((i, r));
        }
        self.spread_tex
            .retain(|t| pages.iter().any(|p| p.id == t.0));
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
        let want_pixels = self.eyedrop.active && self.pixels.as_ref().map(|p| p.0) != Some(key);
        if (self.rendered != Some(key) || want_pixels) && !panning {
            let mut scene = doc.scene();
            // Modo foco (GD-BDR-82): só os objetos do foco aparecem.
            if let Some(ids) = self.focus_ids(doc) {
                let mut items = Vec::new();
                let mut blends = std::collections::HashMap::new();
                for it in &scene.items {
                    if ids.contains(it.object_id()) {
                        items.push(it.clone());
                    }
                }
                for (k, v) in &scene.blends {
                    blends.insert(k.clone(), *v);
                }
                scene = std::sync::Arc::new(graphdraw_core::render::Scene {
                    width: scene.width,
                    height: scene.height,
                    background: scene.background.clone(),
                    bleed: scene.bleed,
                    items,
                    missing_fonts: Vec::new(),
                    blends,
                    overprints: scene.overprints.clone(),
                    item_bounds: Default::default(),
                });
            }
            if let Some(pm) = raster::render(
                &scene,
                v.raster_affine(ph, ppp as f64),
                px[0] as u32,
                px[1] as u32,
                &RasterOptions::default(),
            ) {
                if self.eyedrop.active {
                    self.pixels = Some((key, std::sync::Arc::new(pm.data().to_vec())));
                }
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
            Tool::Gradient => Glyph::Cross {
                badge: "tool-gradient",
            },
            Tool::SolidFill => Glyph::Cross {
                badge: "tool-fill-solid",
            },
            Tool::Freehand => Glyph::Cross {
                badge: "tool-freehand",
            },
            Tool::Polyline => Glyph::Cross {
                badge: "tool-polyline",
            },
            Tool::Line2 => Glyph::Cross {
                badge: "tool-line2",
            },
            Tool::Curve3 => Glyph::Cross {
                badge: "tool-curve3",
            },
            Tool::Knife => Glyph::Cross {
                badge: "tool-knife",
            },
            Tool::Crop => Glyph::Cross { badge: "tool-crop" },
            Tool::Eraser => Glyph::Cross {
                badge: "tool-eraser",
            },
            Tool::VirtualDelete => Glyph::Cross {
                badge: "tool-virtual-delete",
            },
            // Pipeta para amostrar; balde para despejar (GD-CTG-05).
            Tool::Eyedropper => Glyph::Cross {
                badge: if self.eyedrop.sampling() {
                    "tool-eyedropper"
                } else {
                    "tool-fill-solid"
                },
            },
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
                // Cor de identificação da camada (GD-CAM-01), se houver.
                let c = doc
                    .page()
                    .layers
                    .get(r.loc.layer)
                    .and_then(|l| l.color.as_ref())
                    .map(crate::widgets::to_color32)
                    .unwrap_or(theme::SELECTION_OBJECT);
                painter.rect_stroke(
                    self.screen_rect(rect, ph, b),
                    0.0,
                    EStroke::new(theme::GUIDE_WIDTH, c),
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
        if tool == Tool::Gradient {
            self.paint_gradient(painter, doc, rect, ph);
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
                    let b = base_end(*start, self.draw_end(cur), mods);
                    painter.line_segment(
                        [self.screen(rect, ph, *start), self.screen(rect, ph, b)],
                        thin,
                    );
                }
            }
            Drag::DrawRect { start, prim, .. } => {
                if let Some(cur) = self.cursor_page {
                    let mods = ui.input(|i| i.modifiers);
                    let r = draw_rect(*start, self.draw_end(cur), mods);
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
                        Prim::Spiral | Prim::Grid => {
                            let mut path = generated_outline(
                                *prim,
                                r.width(),
                                r.height(),
                                &self.shape_preview,
                            );
                            path.apply_affine(Affine::translate((r.x0, r.y0)));
                            self.preview_lines(painter, rect, ph, &path, thin);
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
            let c = self.draw_end(cur);
            if let Some((w, h, m)) = three_point_frame(t.a, t.b, c) {
                let mut path = if matches!(t.prim, Prim::Spiral | Prim::Grid) {
                    generated_outline(t.prim, w, h, &self.shape_preview)
                } else {
                    shape_outline_with(t.prim, w, h, _defaults_view(&self.poly_preview))
                };
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
                let cur = self.draw_end(cur);
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

    /// Linha do gradiente, alças de início/fim e paradas com a cor de cada uma.
    fn paint_gradient(&self, painter: &egui::Painter, doc: &DocState, rect: Rect, ph: f64) {
        use graphdraw_core::edit::gradtool::{self, Handle};
        let Some((_, obj, g, m)) = single_gradient(doc) else {
            return;
        };
        let Some((a, b)) = gradtool::axis(&g) else {
            return;
        };
        let (sa, sb) = (self.screen(rect, ph, m * a), self.screen(rect, ph, m * b));
        painter.line_segment(
            [sa, sb],
            EStroke::new(theme::HANDLE_STROKE + 1.0, theme::HANDLE_FILL),
        );
        painter.line_segment([sa, sb], thin());
        let hs = handle_stroke();
        for (h, q) in gradtool::handles(&g) {
            let c = self.screen(rect, ph, m * q);
            match h {
                Handle::Start | Handle::End => {
                    let r = Rect::from_center_size(c, Vec2::splat(theme::HANDLE_SIZE + 2.0));
                    painter.rect_filled(r, theme::RADIUS_XS, theme::HANDLE_FILL);
                    painter.rect_stroke(r, theme::RADIUS_XS, hs, egui::StrokeKind::Middle);
                }
                Handle::Stop(i) => {
                    let s = &g.stops[i];
                    let fill = crate::widgets::to_color32(&s.color);
                    painter.circle(c, theme::GRADIENT_MARK / 2.0, fill, hs);
                    if self.stop_of(doc, &obj.id, g.stops.len()) == Some(i) {
                        painter.circle_stroke(
                            c,
                            theme::GRADIENT_MARK / 2.0 + theme::SPACE_050 + 1.0,
                            EStroke::new(theme::FOCUS_WIDTH, accent()),
                        );
                    }
                }
            }
        }
    }

    /// Prévia em linhas abertas, uma por subcaminho (fechados voltam ao início).
    fn preview_lines(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        ph: f64,
        path: &BezPath,
        st: EStroke,
    ) {
        let mut lines: Vec<Vec<Pos2>> = Vec::new();
        kurbo::flatten(path.iter(), 0.5 / self.view.zoom.max(1e-9), |el| match el {
            kurbo::PathEl::MoveTo(q) => lines.push(vec![self.screen(rect, ph, q)]),
            kurbo::PathEl::LineTo(q) => {
                if let Some(l) = lines.last_mut() {
                    l.push(self.screen(rect, ph, q));
                }
            }
            kurbo::PathEl::ClosePath => {
                if let Some(l) = lines.last_mut()
                    && let Some(f) = l.first().copied()
                {
                    l.push(f);
                }
            }
            _ => {}
        });
        for l in lines {
            painter.add(egui::Shape::line(l, st));
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
        if let Some((_, _, p, m)) = single_polygon(doc) {
            for (_, q) in polygon_handles(&p) {
                let c = self.screen(rect, ph, m * q);
                let r = Rect::from_center_size(c, Vec2::splat(theme::HANDLE_SIZE));
                painter.rect_filled(r, theme::RADIUS_XS, theme::HANDLE_FILL);
                painter.rect_stroke(
                    r,
                    theme::RADIUS_XS,
                    handle_stroke(),
                    egui::StrokeKind::Middle,
                );
            }
            return;
        }
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
            Tool::Knife => {
                self.drag = Drag::Knife {
                    pts: vec![p, p],
                    free: mods.alt,
                }
            }
            Tool::Crop => {
                self.crop_pending = None;
                self.drag = Drag::CropRect { start: p, cur: p };
            }
            Tool::Eraser => self.drag = Drag::Erase { pts: vec![p] },
            Tool::Freehand => {
                self.drag = Drag::Freehand {
                    pts: vec![p],
                    poly: false,
                }
            }
            Tool::Polyline => {
                if self.bezier_press(doc, p, origin, rect, ph) {
                    self.finish_bezier(doc, _defaults, true);
                } else {
                    let start = self
                        .bezier
                        .as_ref()
                        .and_then(|d| d.nodes.last())
                        .map_or(p, |n| pt(n.p));
                    self.drag = Drag::Freehand {
                        pts: vec![start],
                        poly: true,
                    };
                }
            }
            Tool::Line2 => {
                let mut start = snap::snap_guided_on(
                    &self.guides,
                    p,
                    &snap::candidates_doc(doc.doc(), doc.page, &self.snap, &[]),
                    &self.snap,
                    snap_radius(&self.view),
                )
                .map_or(p, |s| s.point);
                let mut normal = None;
                if self.line_mode == LineMode::Perpendicular
                    && let Some((q, n)) = outline_normal(doc, p, tol)
                {
                    start = q;
                    normal = Some(n);
                }
                self.drag = Drag::Line2 {
                    start,
                    cur: start,
                    normal,
                };
            }
            Tool::Curve3 => {
                if self.curve3.is_none() {
                    self.drag = Drag::Curve3Base { start: p, cur: p };
                }
            }
            Tool::Pan => self.drag = Drag::Pan,
            Tool::Zoom => self.drag = Drag::ZoomRect { start: origin },
            Tool::Text => {
                self.end_text(doc);
                self.drag = Drag::TextFrame { start: p };
            }
            Tool::Rect | Tool::Ellipse | Tool::Polygon => {
                let cands = snap::candidates_doc(doc.doc(), doc.page, &self.snap, &[]);
                let start = snap::snap_guided_on(
                    &self.guides,
                    p,
                    &cands,
                    &self.snap,
                    snap_radius(&self.view),
                )
                .map_or(p, |s| s.point);
                // Com a base pronta, apertar o botão fecha a forma.
                if let Some(t) = self.three.take() {
                    self.finish_three(doc, t, start, _defaults);
                    return;
                }
                self.smart = Some((
                    kurbo::Rect::from_points(start, start),
                    graphdraw_core::edit::smart::targets(doc.page(), &[]),
                ));
                self.smart_lines.clear();
                self.smart_point = None;
                self.drag = Drag::DrawRect {
                    start,
                    prim: Prim::of(tool, _defaults),
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
            Tool::Gradient => self.start_gradient_drag(doc, p, origin, tol, rect, ph),
            // Ferramentas de clique: arrastar não faz nada.
            Tool::SolidFill | Tool::Eyedropper | Tool::VirtualDelete => {}
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

    /// Preenchimento interativo: alça do gradiente do objeto selecionado, ou
    /// um gradiente novo puxado sobre o objeto clicado.
    fn start_gradient_drag(
        &mut self,
        doc: &mut DocState,
        p: Point,
        origin: Pos2,
        tol: f64,
        rect: Rect,
        ph: f64,
    ) {
        use graphdraw_core::edit::gradtool;
        if let Some((r, o, g, m)) = single_gradient(doc) {
            let hit = gradtool::handles(&g).into_iter().find(|(_, q)| {
                (self.screen(rect, ph, m * *q) - origin).length() <= theme::HANDLE_HIT
            });
            if let Some((handle, _)) = hit {
                if let gradtool::Handle::Stop(i) = handle {
                    self.grad_stop = Some((o.id.clone(), i));
                }
                doc.editor.begin(tr("Editar gradiente", "Edit gradient"));
                self.drag = Drag::GradHandle {
                    handle,
                    r,
                    original: o,
                    m,
                };
                return;
            }
        }
        let id = pick(doc, p, tol, true).or_else(|| match doc.selection.as_slice() {
            [one] => Some(one.clone()),
            _ => None,
        });
        let target = id.and_then(|id| {
            let r = doc.editor.find(&id)?;
            let o = doc.editor.get(&r)?.clone();
            (!matches!(o.kind, ObjectKind::Group(_) | ObjectKind::Image(_)) && !o.locked)
                .then_some((r, o))
        });
        let Some((r, o)) = target else {
            self.drag = Drag::Rubber {
                start: origin,
                lasso: None,
            };
            return;
        };
        if doc.selection != [o.id.clone()] {
            doc.selection = vec![o.id.clone()];
            self.grad_stop = None;
        }
        let m = doc.editor.parent_affine(&r) * o.transform.affine();
        doc.editor
            .begin(tr("Gradiente interativo", "Interactive gradient"));
        self.drag = Drag::GradNew {
            start: p,
            r,
            original: o,
            m,
        };
    }

    /// Cor da parada escolhida (clique na paleta); `false` se não há parada.
    pub fn set_grad_stop_color(
        &mut self,
        doc: &mut DocState,
        color: graphdraw_core::model::Color,
    ) -> bool {
        let Some((r, mut o, mut g, _)) = single_gradient(doc) else {
            return false;
        };
        let Some(i) = self.stop_of(doc, &o.id, g.stops.len()) else {
            return false;
        };
        let Some(s) = g.stops.get_mut(i) else {
            return false;
        };
        s.color = color;
        o.fill = Some(Fill::Gradient(g));
        doc.editor
            .transaction(tr("Cor da parada", "Stop color"), |ed| ed.replace_at(&r, o));
        self.grad_rev = doc.editor.revision();
        true
    }

    /// Índice da parada escolhida se ela é do objeto `id` e ainda existe.
    fn stop_of(&self, doc: &DocState, id: &str, len: usize) -> Option<usize> {
        if doc.editor.revision() != self.grad_rev {
            return None;
        }
        self.grad_stop
            .as_ref()
            .filter(|(o, i)| o == id && *i < len)
            .map(|(_, i)| *i)
    }

    /// Clique sem arrasto na ferramenta de gradiente: escolhe a parada sob
    /// o cursor, senão seleciona o objeto clicado.
    fn gradient_click(&mut self, doc: &mut DocState, p: Point, pos: Pos2, tol: f64, rect: Rect) {
        use graphdraw_core::edit::gradtool;
        let ph = doc.page_size().1;
        if let Some((_, o, g, m)) = single_gradient(doc)
            && let Some((h, _)) = gradtool::handles(&g)
                .into_iter()
                .find(|(_, q)| (self.screen(rect, ph, m * *q) - pos).length() <= theme::HANDLE_HIT)
        {
            // Início/fim: a parada da ponta (a de menor/maior posição).
            let by_offset = |max: bool| {
                (0..g.stops.len()).reduce(|a, b| {
                    let (oa, ob) = (g.stops[a].offset, g.stops[b].offset);
                    if (ob > oa) == max { b } else { a }
                })
            };
            let i = match h {
                gradtool::Handle::Stop(i) => Some(i),
                gradtool::Handle::Start => by_offset(false),
                gradtool::Handle::End => by_offset(true),
            };
            if let Some(i) = i {
                self.close_stop_popup(doc);
                self.grad_stop = Some((o.id.clone(), i));
                self.grad_rev = doc.editor.revision();
                // Clicar na parada abre o balão de cor e opacidade.
                doc.editor.begin(tr("Cor da parada", "Stop color"));
                self.grad_popup = Some((pos, 0));
            }
            return;
        }
        self.close_stop_popup(doc);
        self.grad_stop = None;
        doc.selection = pick(doc, p, tol, true).into_iter().collect();
    }

    /// Fecha o balão de cor da parada (fecha o passo de desfazer).
    pub fn close_stop_popup(&mut self, doc: &mut DocState) {
        if self.grad_popup.take().is_some() && doc.editor.in_transaction() {
            doc.editor.commit();
        }
    }

    /// Balão de cor da parada: cor (no modelo da cor) e opacidade, aplicados
    /// na hora; clique fora ou Esc fecha.
    fn stop_popup(&mut self, ctx: &egui::Context, doc: &mut DocState, tool: Tool) {
        let Some((pos, opened)) = self.grad_popup else {
            return;
        };
        let frame = ctx.cumulative_frame_nr();
        if opened == 0 {
            self.grad_popup = Some((pos, frame));
        }
        let current = single_gradient(doc).and_then(|(r, o, g, _)| {
            let (id, i) = self.grad_stop.clone()?;
            (id == o.id && i < g.stops.len()).then_some((r, o, g, i))
        });
        let Some((r, o, g, i)) = current.filter(|_| tool == Tool::Gradient) else {
            self.close_stop_popup(doc);
            return;
        };
        let stop = g.stops[i].clone();
        let mut new_color = None;
        let mut new_opacity = None;
        let mut close = false;
        let area = egui::Area::new(egui::Id::unique("balao-parada"))
            .order(egui::Order::Foreground)
            .fixed_pos(pos + Vec2::new(theme::HANDLE_HIT, theme::HANDLE_HIT))
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_width(theme::STOP_POPUP_WIDTH);
                    ui.label(format!(
                        "{} {} · {:.0}%",
                        tr("Parada", "Stop"),
                        i + 1,
                        stop.offset * 100.0
                    ));
                    if let Some(c) = crate::widgets::color_editor(ui, &stop.color) {
                        new_color = Some(c);
                    }
                    let mut op = (stop.opacity * 100.0).round();
                    if ui
                        .add(
                            egui::Slider::new(&mut op, 0.0..=100.0)
                                .text(tr("Opacidade %", "Opacity %")),
                        )
                        .changed()
                    {
                        new_opacity = Some(op / 100.0);
                    }
                    if ui.button(tr("OK", "OK")).clicked() {
                        close = true;
                    }
                });
            });
        if new_color.is_some() || new_opacity.is_some() {
            let mut o = o;
            let mut g = g;
            if let Some(c) = new_color {
                g.stops[i].color = c;
            }
            if let Some(v) = new_opacity {
                g.stops[i].opacity = v;
            }
            o.fill = Some(Fill::Gradient(g));
            doc.editor.replace_at(&r, o);
            self.grad_rev = doc.editor.revision();
        }
        let outside = ctx.input(|i| i.pointer.any_pressed())
            && !area.response.contains_pointer()
            && opened != 0
            && frame > opened + 1;
        if close || outside || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.close_stop_popup(doc);
        }
    }

    /// Faca solta: corta a seleção (ou o que a linha cruza) e seleciona as peças.
    fn knife_end(&mut self, doc: &mut DocState, pts: &[Point]) {
        use graphdraw_core::edit::cutting;
        if pts.len() < 2
            || (pts[0] - *pts.last().unwrap()).hypot() * self.view.zoom < 3.0 && pts.len() == 2
        {
            return;
        }
        let mut line = BezPath::new();
        line.move_to(pts[0]);
        for p in &pts[1..] {
            line.line_to(*p);
        }
        let sel: Vec<String> = doc
            .selection
            .iter()
            .filter(|id| doc.editor.find(id).is_some_and(|r| r.is_top()))
            .cloned()
            .collect();
        let ids = if sel.is_empty() {
            cutting::crossed(doc.page(), &line)
        } else {
            sel
        };
        let made = cutting::knife(&mut doc.editor, &ids, &line, self.knife_gap);
        if made.is_empty() {
            self.note = Some(
                tr(
                    "A faca precisa atravessar o objeto de um lado ao outro",
                    "The knife must go across the object from side to side",
                )
                .into(),
            );
        } else {
            doc.selection = made;
        }
    }

    /// Borracha solta: apaga na seleção (ou em tudo o que o pincel toca).
    fn erase_end(&mut self, doc: &mut DocState, pts: &[Point]) {
        let ids: Vec<String> = if doc.selection.is_empty() {
            doc.page()
                .layers
                .iter()
                .filter(|l| l.visible && !l.locked)
                .flat_map(|l| l.objects.iter().filter(|o| !o.locked).map(|o| o.id.clone()))
                .collect()
        } else {
            doc.selection.clone()
        };
        graphdraw_core::edit::cutting::erase(&mut doc.editor, &ids, pts, self.eraser_width);
    }

    /// Aplica o recorte pendente (GD-FER-16).
    pub fn apply_crop(&mut self, doc: &mut DocState) {
        let Some(r) = self.crop_pending.take() else {
            return;
        };
        let page = doc.page;
        let (changed, removed, skipped) =
            graphdraw_core::edit::cutting::crop(&mut doc.editor, page, r);
        let mut msg = format!(
            "{} {changed} {}, {removed} {}",
            tr("Recorte:", "Crop:"),
            tr("aparado(s)", "trimmed"),
            tr("removido(s)", "removed")
        );
        if skipped > 0 {
            msg.push_str(&format!(
                "; {skipped} {}",
                tr(
                    "imagem(ns)/texto(s) na borda ficaram inteiros",
                    "image(s)/text(s) on the edge kept whole"
                )
            ));
        }
        self.note = Some(msg);
    }

    /// Objeto desenhado (linha aberta ou curva fechada) com os padrões.
    fn add_drawn(&mut self, doc: &mut DocState, sp: Subpath, defaults: &Defaults, name: &str) {
        let closed = sp.closed;
        let mut o = Object::new(ObjectKind::Path(PathData {
            subpaths: vec![sp],
            fill_rule: FillRule::Evenodd,
            extra: Map::new(),
        }));
        o.fill = if closed { defaults.fill.clone() } else { None };
        o.stroke = defaults.stroke.clone();
        if let Some(layer) = doc.layer_for_new_object() {
            let id = ops::add_object(&mut doc.editor, doc.page, layer, o, name);
            doc.selection = vec![id];
        }
    }

    /// Mão livre solta (GD-FER-20).
    fn freehand_end(&mut self, doc: &mut DocState, pts: &[Point], defaults: &Defaults) {
        let px = 1.0 / self.view.zoom;
        if let Some(sp) =
            graphdraw_core::edit::drawing::freehand(pts, px, self.freehand_smooth, 7.0 * px)
        {
            self.add_drawn(
                doc,
                sp,
                defaults,
                tr("Desenhar à mão livre", "Draw freehand"),
            );
        }
    }

    /// Trecho livre da polilinha (GD-FER-25): vira nós ajustados no rascunho.
    fn polyline_stretch(&mut self, pts: &[Point]) {
        let px = 1.0 / self.view.zoom;
        let moved = pts.windows(2).map(|w| (w[1] - w[0]).hypot()).sum::<f64>();
        if moved < 3.0 * px {
            return;
        }
        let path = graphdraw_core::edit::drawing::fit_stroke(
            pts,
            px * (0.5 + self.freehand_smooth / 100.0 * 6.0),
            70.0,
        );
        let Some(sp) = graphdraw_core::edit::convert::bez_to_subpaths(&path)
            .into_iter()
            .next()
        else {
            return;
        };
        let draft = self.bezier.get_or_insert(BezierDraft { nodes: Vec::new() });
        let mut nodes = sp.nodes.into_iter();
        if let (Some(first), Some(last)) = (nodes.next(), draft.nodes.last_mut()) {
            last.handle_out = first.handle_out;
        }
        draft.nodes.extend(nodes);
    }

    /// Prévia da faca, do recorte e da borracha.
    fn paint_cut_tools(
        &self,
        ui: &egui::Ui,
        painter: &egui::Painter,
        rect: Rect,
        ph: f64,
        tool: Tool,
    ) {
        let accent = theme::ACCENT_PRIMARY;
        let line = |pts: &[Point]| -> Vec<Pos2> {
            pts.iter().map(|p| self.screen(rect, ph, *p)).collect()
        };
        let thin = EStroke::new(theme::BORDER_WIDTH, accent);
        match &self.drag {
            Drag::Freehand { pts, .. } => {
                painter.add(egui::Shape::line(line(pts), thin));
            }
            Drag::Line2 { start, cur, .. } | Drag::Curve3Base { start, cur } => {
                painter.line_segment(
                    [self.screen(rect, ph, *start), self.screen(rect, ph, *cur)],
                    thin,
                );
            }
            _ => {}
        }
        if let (Some((a, b)), Some(c)) = (self.curve3, self.cursor_page) {
            let sp = graphdraw_core::edit::drawing::curve3(a, b, c);
            let path = graphdraw_core::model::geom::subpaths_to_bez(&[sp]);
            self.stroke_bez(painter, rect, ph, &path, thin);
        }
        match &self.drag {
            Drag::Knife { pts, .. } => {
                painter.add(egui::Shape::line(
                    line(pts),
                    EStroke::new(theme::FOCUS_WIDTH, accent),
                ));
            }
            Drag::Erase { pts } => {
                let w = (self.eraser_width * self.view.zoom) as f32;
                painter.add(egui::Shape::line(
                    line(pts),
                    EStroke::new(
                        w,
                        theme::ACCENT_DANGER.gamma_multiply(theme::ERASER_PREVIEW_ALPHA),
                    ),
                ));
            }
            _ => {}
        }
        let crop = match &self.drag {
            Drag::CropRect { start, cur } => Some(kurbo::Rect::from_points(*start, *cur)),
            _ => self.crop_pending.filter(|_| tool == Tool::Crop),
        };
        if let Some(c) = crop {
            let r = self.screen_rect(rect, ph, c);
            let veil = egui::Color32::from_black_alpha(theme::CROP_VEIL_ALPHA);
            for part in [
                Rect::from_min_max(rect.min, egui::pos2(rect.max.x, r.min.y)),
                Rect::from_min_max(egui::pos2(rect.min.x, r.max.y), rect.max),
                Rect::from_min_max(
                    egui::pos2(rect.min.x, r.min.y),
                    egui::pos2(r.min.x, r.max.y),
                ),
                Rect::from_min_max(
                    egui::pos2(r.max.x, r.min.y),
                    egui::pos2(rect.max.x, r.max.y),
                ),
            ] {
                painter.rect_filled(part, 0.0, veil);
            }
            painter.rect_stroke(
                r,
                0.0,
                EStroke::new(theme::FOCUS_WIDTH, accent),
                egui::StrokeKind::Middle,
            );
        }
        // Pincel da borracha sob o cursor.
        if tool == Tool::Eraser
            && let Some(p) = ui
                .input(|i| i.pointer.hover_pos())
                .filter(|p| rect.contains(*p))
        {
            painter.circle_stroke(
                p,
                (self.eraser_width * self.view.zoom / 2.0) as f32,
                EStroke::new(theme::BORDER_WIDTH, accent),
            );
        }
    }

    /// Conta-gotas (doc 27): clique esquerdo ou direito (`right`). Na pipeta
    /// amostra e passa ao balde; no balde despeja nos destinos. Devolve a
    /// mensagem da barra de status.
    #[allow(clippy::too_many_arguments)]
    fn eyedrop_click(
        &mut self,
        doc: &mut DocState,
        p: Point,
        pos: Pos2,
        mods: egui::Modifiers,
        tol: f64,
        rect: Rect,
        right: bool,
    ) -> Option<String> {
        use crate::eyedrop::{Bucket, Mode};
        use graphdraw_core::edit::eyedrop::{self as ed, Paint};
        // Alt alterna pipeta ↔ balde; na pipeta, sobre um gradiente, clona
        // a rampa inteira (GD-CTG-05/09).
        if mods.alt && !mods.shift {
            if self.eyedrop.applying {
                self.eyedrop.applying = false;
                return Some(
                    tr(
                        "Pipeta: clique para amostrar",
                        "Eyedropper: click to sample",
                    )
                    .into(),
                );
            }
            if self.eyedrop.mode == Mode::Color
                && let Some(s) = ed::sample_native(&doc.editor, doc.page(), p, tol)
                && let Some((g, from)) = s.ramp
            {
                self.eyedrop.load(Bucket::Paint(Paint::Ramp(g, from)));
                return Some(
                    tr(
                        "Rampa clonada: clique num objeto para aplicar",
                        "Ramp cloned: click an object to apply",
                    )
                    .into(),
                );
            }
            if self.eyedrop.bucket.is_some() {
                self.eyedrop.applying = true;
                return Some(tr("Balde: clique para aplicar", "Bucket: click to apply").into());
            }
        }
        if self.eyedrop.sampling() {
            return Some(self.eyedrop_sample(doc, p, pos, tol, rect));
        }
        let bucket = self.eyedrop.bucket.clone()?;
        let ids = ed::targets(doc.page(), p, tol, mods.ctrl);
        if ids.is_empty() {
            return None;
        }
        let tpl = self
            .eyedrop
            .new_stroke
            .clone()
            .unwrap_or_else(crate::doc::default_stroke);
        let done = match &bucket {
            // Balde sobre uma parada: só ela muda (GD-CTG-09).
            Bucket::Paint(Paint::Color(c)) if !right => match self
                .stop_under(doc, p, pos, rect, tol)
            {
                Some((id, i)) => ed::replace_stop(&mut doc.editor, &id, i, c),
                None => {
                    ed::apply_paint(&mut doc.editor, &ids, &Paint::Color(c.clone()), false, &tpl)
                }
            },
            Bucket::Paint(paint) => ed::apply_paint(&mut doc.editor, &ids, paint, right, &tpl),
            Bucket::Attrs(src) => {
                ed::apply_attributes(&mut doc.editor, src, &ids, &self.eyedrop.filter)
            }
        };
        if done {
            self.eyedrop.ripple = Some((ids, self.eyedrop.now));
            None
        } else if right && matches!(bucket, Bucket::Paint(Paint::Ramp(..))) {
            Some(
                tr(
                    "A rampa vai no preenchimento (botão esquerdo)",
                    "The ramp goes to the fill (left button)",
                )
                .into(),
            )
        } else {
            None
        }
    }

    /// Pipeta: cor (pelo raio escolhido) ou atributos do objeto sob o ponto.
    fn eyedrop_sample(
        &mut self,
        doc: &DocState,
        p: Point,
        pos: Pos2,
        tol: f64,
        rect: Rect,
    ) -> String {
        use crate::eyedrop::{Bucket, Mode};
        use graphdraw_core::edit::eyedrop::Paint;
        let nothing = || tr("Nada para amostrar aqui", "Nothing to sample here").to_string();
        if self.eyedrop.mode == Mode::Attributes {
            let Some(o) = query::hit_test_deep(doc.page(), p, tol)
                .and_then(|id| doc.editor.find(&id))
                .and_then(|r| doc.editor.get(&r).cloned())
            else {
                return nothing();
            };
            self.eyedrop.load(Bucket::Attrs(Box::new(o)));
            return tr(
                "Atributos coletados: clique nos objetos de destino",
                "Attributes picked: click the target objects",
            )
            .into();
        }
        let Some(c) = self.hover_color(doc, p, pos, rect, tol) else {
            return nothing();
        };
        let hex = c.to_hex().to_uppercase();
        self.eyedrop.load(Bucket::Paint(Paint::Color(c)));
        format!(
            "{} {hex} — {}",
            tr("Amostrado", "Sampled"),
            tr(
                "clique para aplicar (direito: contorno)",
                "click to apply (right: outline)"
            )
        )
    }

    /// Cor sob o cursor pelo raio de coleta: nativa do vetor (imagens e
    /// vazio pela tela) ou a média dos pixels (GD-CTG-03).
    fn hover_color(
        &self,
        doc: &DocState,
        p: Point,
        pos: Pos2,
        rect: Rect,
        tol: f64,
    ) -> Option<graphdraw_core::model::Color> {
        use crate::eyedrop::{GRID, Radius, average};
        let grid = || self.pixel_grid(doc, rect, pos);
        match self.eyedrop.radius {
            Radius::Native => {
                graphdraw_core::edit::eyedrop::sample_native(&doc.editor, doc.page(), p, tol)
                    .map(|s| s.color)
                    .or_else(|| grid().and_then(|g| average(&g, GRID, 1)))
            }
            r => grid().and_then(|g| average(&g, GRID, r.side())),
        }
    }

    /// Pixels físicos em volta do cursor, do último quadro renderizado,
    /// compostos sobre o papel (na folha) ou sobre o fundo do canvas.
    fn pixel_grid(&self, doc: &DocState, rect: Rect, pos: Pos2) -> Option<Vec<[u8; 3]>> {
        let (key, px) = self.pixels.as_ref()?;
        if key.page != doc.page {
            return None;
        }
        let ppp = key.ppp;
        let shift = Vec2::new(
            (self.view.pan.0 - key.view.pan.0) as f32,
            (self.view.pan.1 - key.view.pan.1) as f32,
        );
        let local = (pos - rect.min - shift) * ppp;
        let (pw, ph) = doc.page_size();
        let v = key.view;
        let s = |x: f64, y: f64| {
            let (a, b) = v.page_to_screen(ph, x, y);
            Pos2::new(a as f32 * ppp, b as f32 * ppp)
        };
        let sheet = Rect::from_two_pos(s(0.0, ph), s(pw, 0.0));
        let paper = [
            theme::PAGE_PAPER.r(),
            theme::PAGE_PAPER.g(),
            theme::PAGE_PAPER.b(),
        ];
        let canvas = self.eyedrop.canvas_bg;
        Some(crate::eyedrop::grid_from(
            px,
            key.size,
            local.x.floor() as i64,
            local.y.floor() as i64,
            crate::eyedrop::GRID,
            |x, y| {
                if sheet.contains(Pos2::new(x as f32 + 0.5, y as f32 + 0.5)) {
                    paper
                } else {
                    canvas
                }
            },
        ))
    }

    /// Parada de gradiente sob o cursor (objeto e índice).
    fn stop_under(
        &self,
        doc: &DocState,
        p: Point,
        pos: Pos2,
        rect: Rect,
        tol: f64,
    ) -> Option<(String, usize)> {
        use graphdraw_core::edit::gradtool;
        let id = query::hit_test_deep(doc.page(), p, tol)?;
        let r = doc.editor.find(&id)?;
        let o = doc.editor.get(&r)?;
        let Some(Fill::Gradient(g)) = &o.fill else {
            return None;
        };
        let m = doc.editor.parent_affine(&r) * o.transform.affine();
        let ph = doc.page_size().1;
        gradtool::handles(g).into_iter().find_map(|(h, q)| match h {
            gradtool::Handle::Stop(i)
                if (self.screen(rect, ph, m * q) - pos).length() <= theme::HANDLE_HIT =>
            {
                Some((id.clone(), i))
            }
            _ => None,
        })
    }

    /// Lupa, paradas visíveis para o balde e o realce de quem recebeu
    /// (doc 27).
    #[allow(clippy::too_many_arguments)]
    fn eyedrop_overlay(
        &mut self,
        ui: &egui::Ui,
        painter: &egui::Painter,
        doc: &mut DocState,
        rect: Rect,
        ph: f64,
        tol: f64,
        ev: &mut CanvasEvents,
    ) {
        use crate::eyedrop::{Bucket, HoverInfo, HudAction};
        use graphdraw_core::edit::eyedrop::Paint;
        // Realce de 150 ms (GD-CTG-12).
        match self.eyedrop.ripple_phase() {
            Some(k) => {
                if let Some((ids, _)) = &self.eyedrop.ripple {
                    let c = theme::ACCENT_PRIMARY.gamma_multiply((1.0 - k) as f32);
                    for id in ids {
                        if let Some(b) = doc
                            .editor
                            .find(id)
                            .and_then(|r| ops::object_bounds(&doc.editor, &r, true))
                        {
                            let r = Rect::from_two_pos(
                                self.screen(rect, ph, Point::new(b.x0, b.y0)),
                                self.screen(rect, ph, Point::new(b.x1, b.y1)),
                            );
                            painter.rect_stroke(
                                r.expand(theme::EYEDROP_RIPPLE_GROW * k as f32),
                                theme::RADIUS_XS,
                                egui::Stroke::new(theme::EYEDROP_RIPPLE_WIDTH, c),
                                egui::StrokeKind::Outside,
                            );
                        }
                    }
                }
                ui.ctx().request_repaint();
            }
            None => self.eyedrop.ripple = None,
        }
        if !self.eyedrop.active {
            return;
        }
        let pointer = ui.input(|i| i.pointer.hover_pos());
        let on_canvas = pointer
            .filter(|q| rect.contains(*q) && !self.eyedrop.over_hud(*q) && self.menu.is_none());
        if let Some(pos) = on_canvas {
            let p = self.page_at(rect, ph, pos);
            let size = if self.eyedrop.hud.rect.is_positive() {
                self.eyedrop.hud.rect.size()
            } else {
                Vec2::new(150.0, 230.0)
            };
            self.eyedrop.follow(pos, self.eyedrop.now, size, rect);
            // Parada: a lupa guarda a cor de onde o cursor parou.
            if !self.eyedrop.hud.frozen {
                if let Some(color) = self.hover_color(doc, p, pos, rect, tol) {
                    let c = crate::widgets::to_color32(&color);
                    let grid = self.pixel_grid(doc, rect, pos).unwrap_or_else(|| {
                        vec![[c.r(), c.g(), c.b()]; crate::eyedrop::GRID.pow(2)]
                    });
                    let name = palette_name(doc, &color);
                    self.eyedrop.info = Some(HoverInfo { color, grid, name });
                }
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_secs_f64(
                        theme::EYEDROP_DWELL_S,
                    ));
            }
            // Paradas à vista para o balde de cor (GD-CTG-09).
            if !self.eyedrop.sampling()
                && matches!(self.eyedrop.bucket, Some(Bucket::Paint(Paint::Color(_))))
            {
                self.paint_hover_stops(painter, doc, p, rect, ph, tol);
            }
        }
        let show = on_canvas.is_some() || pointer.is_some_and(|q| self.eyedrop.over_hud(q));
        if !show {
            self.eyedrop.hud.rect = Rect::NOTHING;
            return;
        }
        let Some(info) = self.eyedrop.info.clone() else {
            return;
        };
        match crate::eyedrop::hud_ui(ui.ctx(), &mut self.eyedrop, &info) {
            Some(HudAction::AddToPalette(c)) => {
                let hex = c.to_hex().to_uppercase();
                graphdraw_core::edit::palette::add_named(&mut doc.editor, &hex, c);
                ev.status = Some(format!(
                    "{hex} {}",
                    tr(
                        "adicionada à paleta do documento",
                        "added to the document palette"
                    )
                ));
            }
            Some(HudAction::Recent(c)) => self.eyedrop.load(Bucket::Paint(Paint::Color(c))),
            None => {}
        }
    }

    /// Paradas do gradiente do objeto sob o cursor, para o balde mirar.
    fn paint_hover_stops(
        &self,
        painter: &egui::Painter,
        doc: &DocState,
        p: Point,
        rect: Rect,
        ph: f64,
        tol: f64,
    ) {
        use graphdraw_core::edit::gradtool;
        let Some(r) = query::hit_test_deep(doc.page(), p, tol).and_then(|id| doc.editor.find(&id))
        else {
            return;
        };
        let Some(o) = doc.editor.get(&r) else {
            return;
        };
        let Some(Fill::Gradient(g)) = &o.fill else {
            return;
        };
        let m = doc.editor.parent_affine(&r) * o.transform.affine();
        for (h, q) in gradtool::handles(g) {
            if let gradtool::Handle::Stop(i) = h {
                let at = self.screen(rect, ph, m * q);
                let rad = theme::GRADIENT_MARK / 2.0;
                painter.circle_filled(at, rad, crate::widgets::to_color32(&g.stops[i].color));
                painter.circle_stroke(
                    at,
                    rad,
                    egui::Stroke::new(theme::FOCUS_WIDTH, egui::Color32::WHITE),
                );
            }
        }
    }

    /// Tira a parada escolhida do gradiente (Delete); `false` se não havia.
    pub fn delete_grad_stop(&mut self, doc: &mut DocState) -> bool {
        let Some((r, mut o, g, _)) = single_gradient(doc) else {
            return false;
        };
        let Some(i) = self.stop_of(doc, &o.id, g.stops.len()) else {
            return false;
        };
        self.grad_stop = None;
        let Some(ng) = graphdraw_core::edit::gradtool::remove_stop(&g, i) else {
            return true;
        };
        o.fill = Some(Fill::Gradient(ng));
        doc.editor
            .transaction(tr("Excluir parada", "Delete stop"), |ed| {
                ed.replace_at(&r, o)
            });
        true
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
                        self.smart = Some((
                            b,
                            graphdraw_core::edit::smart::targets(doc.page(), &doc.selection),
                        ));
                        self.smart_lines.clear();
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
        match self.pick_focus(doc, p, tol, mods.ctrl) {
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
                let cands = snap::candidates_doc(doc.doc(), doc.page, &self.snap, &doc.selection);
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
                self.smart = Some((
                    b,
                    graphdraw_core::edit::smart::targets(doc.page(), &doc.selection),
                ));
                self.smart_lines.clear();
                self.stamped = 0;
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
        // Polígono: arrastar um vértice repete a mudança em todas as pontas.
        if let Some((r, o, pd, m)) = single_polygon(doc) {
            if let Some((inner, _)) = polygon_handles(&pd).into_iter().find(|(_, q)| {
                (self.screen(rect, ph, m * *q) - origin).length() <= theme::HANDLE_HIT
            }) {
                doc.editor.begin(tr("Editar polígono", "Edit polygon"));
                self.drag = Drag::PolyVertex {
                    inner,
                    r,
                    original: o,
                };
                return;
            }
            self.drag = Drag::Rubber {
                start: origin,
                lasso: None,
            };
            return;
        }
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
            let cands =
                snap::candidates_doc(doc.doc(), doc.page, &self.snap, std::slice::from_ref(&o.id));
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

    /// Fim do traço de desenho: o encaixe normal, senão o das guias
    /// inteligentes, senão o cursor.
    fn draw_end(&self, cur: Point) -> Point {
        self.snap_hit
            .map(|s| s.point)
            .or(self.smart_point)
            .unwrap_or(cur)
    }

    fn update_drag(&mut self, doc: &mut DocState, cur: Point, mods: egui::Modifiers, _tol: f64) {
        let px = 2.0 / self.view.zoom;
        let line_mode = self.line_mode;
        let zoom = self.view.zoom;
        match &mut self.drag {
            Drag::Freehand { pts, .. } => {
                if pts.last().is_none_or(|l| (cur - *l).hypot() > px / 2.0) {
                    pts.push(cur);
                }
                return;
            }
            Drag::Line2 {
                start,
                cur: c,
                normal,
            } => {
                let mut end = cur;
                if let Some(n) = normal {
                    end = *start + *n * (cur - *start).dot(*n);
                } else if line_mode == LineMode::Tangent
                    && let Some(q) = outline_tangent(doc, *start, cur, 30.0 / zoom)
                {
                    end = q;
                } else if mods.shift {
                    let d = cur - *start;
                    let step = 15f64.to_radians();
                    let a = (d.y.atan2(d.x) / step).round() * step;
                    end = *start + kurbo::Vec2::new(a.cos(), a.sin()) * d.hypot();
                }
                *c = end;
                return;
            }
            Drag::Curve3Base { cur: c, .. } => {
                *c = cur;
                return;
            }
            Drag::Knife { pts, free: true } | Drag::Erase { pts } => {
                if pts.last().is_none_or(|l| (cur - *l).hypot() > px) {
                    pts.push(cur);
                }
                return;
            }
            Drag::Knife { pts, free: false } => {
                *pts.last_mut().unwrap() = cur;
                return;
            }
            Drag::CropRect { cur: c, .. } => {
                *c = cur;
                return;
            }
            _ => {}
        }
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
                if let Some(s) =
                    snap::snap_guided_on(&self.guides, *grab + d, cands, &self.snap, radius)
                {
                    d = s.point - *grab;
                    self.snap_hit = Some(s);
                }
                // Guias inteligentes: bordas e centros alinham com os outros
                // objetos e com a página quando o ponto não encaixou.
                self.smart_lines.clear();
                if self.smart_guides
                    && self.snap.enabled
                    && self.snap_hit.is_none()
                    && let Some((b0, t)) = &self.smart
                {
                    let (adj, lines) = graphdraw_core::edit::smart::align(*b0 + d, t, radius);
                    if mods.ctrl {
                        // Restrito a um eixo: só ajusta nele.
                        if d.x == 0.0 {
                            d.y += adj.y;
                        } else {
                            d.x += adj.x;
                        }
                    } else {
                        d += adj;
                    }
                    self.smart_lines = lines;
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
                // Guias inteligentes: a alça encaixa nas bordas e centros dos
                // outros objetos e da página.
                self.smart_lines.clear();
                if self.smart_guides
                    && self.snap.enabled
                    && let Some((_, t)) = &self.smart
                {
                    let np = Point::new(
                        fixed.x + (hp.x - fixed.x) * sx,
                        fixed.y + (hp.y - fixed.y) * sy,
                    );
                    let ext = kurbo::Rect::from_points(fixed, np);
                    let (adj, mut lines) = graphdraw_core::edit::smart::align_point(
                        np,
                        ext,
                        t,
                        radius,
                        fx != 0.5 && (hp.x - fixed.x).abs() > 1e-9,
                        fy != 0.5 && (hp.y - fixed.y).abs() > 1e-9,
                    );
                    let fit_x = adj.x != 0.0 || lines.iter().any(|l| l.vertical);
                    let fit_y = adj.y != 0.0 || lines.iter().any(|l| !l.vertical);
                    if corner && !mods.shift {
                        // Proporcional: o eixo que encaixou manda.
                        if fit_x {
                            sx = (np.x + adj.x - fixed.x) / (hp.x - fixed.x);
                            sy = sx.abs() * sy.signum();
                            lines.retain(|l| l.vertical);
                        } else if fit_y {
                            sy = (np.y + adj.y - fixed.y) / (hp.y - fixed.y);
                            sx = sy.abs() * sx.signum();
                        }
                    } else {
                        if fit_x {
                            sx = (np.x + adj.x - fixed.x) / (hp.x - fixed.x);
                        }
                        if fit_y {
                            sy = (np.y + adj.y - fixed.y) / (hp.y - fixed.y);
                        }
                    }
                    self.smart_lines = lines;
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
                    if let Some(s) = snap::snap_guided_on(
                        &self.guides,
                        grabbed + (cur - *start),
                        cands,
                        &self.snap,
                        radius,
                    ) {
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
            Drag::DrawRect { start, cands, .. } => {
                self.snap_hit = snap::snap_guided_on(&self.guides, cur, cands, &self.snap, radius);
                self.smart_lines.clear();
                self.smart_point = None;
                if self.smart_guides
                    && self.snap.enabled
                    && self.snap_hit.is_none()
                    && let Some((_, t)) = &self.smart
                {
                    let ext = kurbo::Rect::from_points(*start, cur);
                    let (adj, lines) =
                        graphdraw_core::edit::smart::align_point(cur, ext, t, radius, true, true);
                    if !lines.is_empty() {
                        self.smart_point = Some(cur + adj);
                        self.smart_lines = lines;
                    }
                }
            }
            Drag::GradHandle {
                handle,
                r,
                original,
                m,
            } => {
                if let Some(Fill::Gradient(g)) = &original.fill {
                    let ng = graphdraw_core::edit::gradtool::drag(g, *handle, m.inverse() * cur);
                    let mut o = original.clone();
                    o.fill = Some(Fill::Gradient(ng));
                    doc.editor.replace_at(r, o);
                }
            }
            Drag::GradNew {
                start,
                r,
                original,
                m,
            } => {
                let mut end = cur;
                if mods.ctrl {
                    // Ângulo em passos de 15°.
                    let d = cur - *start;
                    let a = (d.y.atan2(d.x).to_degrees() / 15.0).round() * 15.0;
                    let a = a.to_radians();
                    end = *start + kurbo::Vec2::new(a.cos(), a.sin()) * d.hypot();
                }
                let mut o = original.clone();
                if (end - *start).hypot() * self.view.zoom >= 3.0 {
                    let g = graphdraw_core::edit::gradtool::linear_from(
                        m.inverse() * *start,
                        m.inverse() * end,
                        original.fill.as_ref(),
                    );
                    o.fill = Some(Fill::Gradient(g));
                }
                doc.editor.replace_at(r, o);
            }
            Drag::PolyVertex { inner, r, original } => {
                if let ObjectKind::Polygon(p0) = &original.kind {
                    let m = doc.editor.parent_affine(r) * original.transform.affine();
                    let q = m.inverse() * cur;
                    // No espaço da elipse de raios rx, ry, o vértice externo
                    // está a 1 do centro.
                    let e = kurbo::Vec2::new(q.x / p0.rx.max(1e-9), q.y / p0.ry.max(1e-9));
                    let k = e.hypot().max(1e-3);
                    let mut o = original.clone();
                    if let ObjectKind::Polygon(p) = &mut o.kind {
                        if *inner {
                            p.sharpness = ((1.0 - k) / 0.95).clamp(0.0, 1.0);
                        } else {
                            p.rx = p0.rx * k;
                            p.ry = p0.ry * k;
                            let turn = e.y.atan2(e.x) - std::f64::consts::FRAC_PI_2;
                            o.transform = Transform::from_affine(
                                original.transform.affine() * Affine::rotate(turn),
                            );
                        }
                    }
                    doc.editor.replace_at(r, o);
                }
            }
            Drag::GuideNew { .. } | Drag::RulerOrigin => {
                let c = snap::candidates_doc(doc.doc(), doc.page, &self.snap, &[]);
                self.snap_hit = snap::snap_guided_on(&[], cur, &c, &self.snap, radius);
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
        let cur = match drag {
            Drag::DrawRect { .. } => self.draw_end(cur),
            _ => cur,
        };
        self.smart = None;
        self.smart_lines.clear();
        self.smart_point = None;
        match drag {
            Drag::Knife { pts, .. } => self.knife_end(doc, &pts),
            Drag::Freehand { mut pts, poly } => {
                pts.push(cur);
                if poly {
                    self.polyline_stretch(&pts);
                } else {
                    self.freehand_end(doc, &pts, defaults);
                }
            }
            Drag::Line2 {
                start, cur: end, ..
            } => {
                if (end - start).hypot() * self.view.zoom > 2.0 {
                    let sp = Subpath {
                        nodes: vec![Node::corner(start.x, start.y), Node::corner(end.x, end.y)],
                        closed: false,
                        extra: Map::new(),
                    };
                    self.add_drawn(doc, sp, defaults, tr("Desenhar linha", "Draw line"));
                }
            }
            Drag::Curve3Base { start, cur: end } => {
                self.curve3 =
                    ((end - start).hypot() * self.view.zoom > 3.0).then_some((start, end));
            }
            Drag::CropRect { start, cur } => {
                let r = kurbo::Rect::from_points(start, cur);
                self.crop_pending = (r.width() * self.view.zoom > 4.0
                    && r.height() * self.view.zoom > 4.0)
                    .then_some(r);
            }
            Drag::Erase { mut pts } => {
                pts.push(cur);
                self.erase_end(doc, &pts);
            }
            // O arrasto direito termina no próprio tratamento do botão direito.
            d @ Drag::RightMove { .. } => self.drag = d,
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
                let endp = base_end(start, self.draw_end(cur), mods);
                if (endp - start).hypot() * self.view.zoom >= 2.0 {
                    self.three = Some(ThreeDraft {
                        a: start,
                        b: endp,
                        prim,
                    });
                }
            }
            Drag::DrawRect { start, prim, .. } => {
                let endp = self.draw_end(cur);
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
            | Drag::Segment { .. }
            | Drag::PolyVertex { .. } => {
                doc.editor.commit();
            }
            Drag::GradHandle { r, .. } => {
                // Paradas em ordem; a escolhida continua escolhida.
                if let Some(mut o) = doc.editor.get(&r).cloned()
                    && let Some(Fill::Gradient(g)) = &mut o.fill
                {
                    let mut g = g.clone();
                    // Durante o arrasto a revisão muda: compara só objeto e índice.
                    let cur = self
                        .grad_stop
                        .as_ref()
                        .filter(|(id, i)| *id == o.id && *i < g.stops.len())
                        .map(|(_, i)| *i);
                    let keep = graphdraw_core::edit::gradtool::sort_stops(&mut g, cur);
                    self.grad_stop = keep.map(|k| (o.id.clone(), k));
                    o.fill = Some(Fill::Gradient(g));
                    doc.editor.replace_at(&r, o);
                }
                doc.editor.commit();
                self.grad_rev = doc.editor.revision();
            }
            Drag::GradNew { start, .. } => {
                if (cur - start).hypot() * self.view.zoom >= 3.0 {
                    self.grad_stop = None;
                    doc.editor.commit();
                } else {
                    doc.editor.cancel();
                }
            }
            Drag::GuideNew { horizontal } => {
                let (rtop, rleft, _) = crate::rulers::areas(rect);
                let p = self.draw_end(cur);
                if rect.contains(end) && !rtop.contains(end) && !rleft.contains(end) {
                    let g = if horizontal {
                        graphdraw_core::model::Guide::horizontal(p.y)
                    } else {
                        graphdraw_core::model::Guide::vertical(p.x)
                    };
                    graphdraw_core::edit::guides::add(
                        &mut doc.editor,
                        doc.page,
                        false,
                        vec![g],
                        tr("Nova guia", "New guide"),
                    );
                }
            }
            Drag::GuideMove { r, original, grab } => {
                let (rtop, rleft, _) = crate::rulers::areas(rect);
                if self.show_rulers && (rtop.contains(end) || rleft.contains(end)) {
                    graphdraw_core::edit::guides::remove(&mut doc.editor, r);
                } else {
                    let g = moved_guide(&original, grab, cur);
                    if g != original {
                        graphdraw_core::edit::guides::set(
                            &mut doc.editor,
                            r,
                            g,
                            tr("Mover guia", "Move guide"),
                        );
                    }
                }
            }
            Drag::RulerOrigin => {
                if rect.contains(end) {
                    self.ruler_origin = self.draw_end(cur);
                }
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
        let p = snap::snap_guided_on(
            &self.guides,
            p,
            &snap::candidates_doc(doc.doc(), doc.page, &self.snap, &[]),
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
        // "À frente de…/Atrás de…": este clique escolhe a referência; área
        // vazia cancela (GD-BDR-41).
        if let Some(above) = self.pick_reference.take() {
            match self.pick_focus(doc, p, tol, true) {
                Some(id) if !doc.selection.contains(&id) => {
                    let sel = doc.selection.clone();
                    ops::place_relative(&mut doc.editor, &sel, &id, above);
                }
                _ => ev.status = Some(tr("Cancelado", "Cancelled").into()),
            }
            return;
        }
        match tool {
            Tool::Gradient => self.gradient_click(doc, p, pos, tol, rect),
            Tool::SolidFill => {
                if let Some(id) = pick(doc, p, tol, true) {
                    doc.selection = vec![id];
                    ev.open_solid_fill = true;
                }
            }
            Tool::Eyedropper => {
                if let Some(msg) = self.eyedrop_click(doc, p, pos, mods, tol, rect, false) {
                    ev.status = Some(msg);
                }
            }
            Tool::Polyline
                // Clique simples = nó de canto (reta); no primeiro nó, fecha.
                if self.bezier_press(doc, p, pos, rect, ph) => {
                    self.finish_bezier(doc, defaults, true);
                }
            Tool::Curve3 => {
                if let Some((a, b)) = self.curve3.take() {
                    let sp = graphdraw_core::edit::drawing::curve3(a, b, p);
                    self.add_drawn(doc, sp, defaults, tr("Desenhar curva", "Draw curve"));
                }
            }
            Tool::VirtualDelete => {
                let page = doc.page;
                let done = pick(doc, p, tol, false).is_some_and(|id| {
                    graphdraw_core::edit::cutting::delete_virtual_segment(&mut doc.editor, page, &id, p, tol)
                });
                if !done {
                    ev.status = Some(tr("Clique sobre o contorno de um objeto", "Click on an object outline").into());
                }
            }
            Tool::Select => match self.pick_focus(doc, p, tol, mods.ctrl) {
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
            Tool::Crop if self.crop_pending.is_some_and(|r| r.contains(p)) => self.apply_crop(doc),
            Tool::Gradient => {
                use graphdraw_core::edit::gradtool;
                if let Some((r, mut o, g, m)) = single_gradient(doc) {
                    let q = m.inverse() * p;
                    // Tolerância de tela levada ao espaço local.
                    let scale = (m.determinant().abs()).sqrt().max(1e-9);
                    if gradtool::distance_to_axis(&g, q).is_some_and(|d| d * scale <= tol)
                        && let Some((ng, i)) = gradtool::add_stop(&g, q)
                    {
                        o.fill = Some(Fill::Gradient(ng));
                        let id = o.id.clone();
                        doc.editor
                            .transaction(tr("Adicionar parada", "Add stop"), |ed| {
                                ed.replace_at(&r, o)
                            });
                        self.grad_stop = Some((id, i));
                        self.grad_rev = doc.editor.revision();
                    }
                }
            }
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
                        ObjectKind::Path(_)
                            | ObjectKind::Rectangle(_)
                            | ObjectKind::Ellipse(_)
                            | ObjectKind::Polygon(_)
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
            Tool::Bezier | Tool::Polyline => {
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

/// Espiral ou papel gráfico no quadro (0, 0)–(w, h), para a prévia.
fn generated_outline(prim: Prim, w: f64, h: f64, d: &Defaults) -> BezPath {
    use graphdraw_core::edit::generate;
    match prim {
        Prim::Spiral => graphdraw_core::model::geom::subpaths_to_bez(
            &generate::spiral(w, h, d.spiral_turns, d.spiral_log, d.spiral_expansion).subpaths,
        ),
        _ => {
            let mut p = BezPath::new();
            let (c, r) = (d.grid_cols.clamp(1, 200), d.grid_rows.clamp(1, 200));
            for i in 0..=c {
                let x = w * i as f64 / c as f64;
                p.move_to((x, 0.0));
                p.line_to((x, h));
            }
            for j in 0..=r {
                let y = h * j as f64 / r as f64;
                p.move_to((0.0, y));
                p.line_to((w, y));
            }
            p
        }
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
        Prim::Rect | Prim::Grid => kurbo::Rect::new(0.0, 0.0, w, h).to_path(0.1),
        Prim::Spiral => BezPath::new(),
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
    if matches!(prim, Prim::Spiral | Prim::Grid) {
        add_generated(doc, prim, w, h, m, defaults);
        return;
    }
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
        Prim::Rect | Prim::Spiral | Prim::Grid => ObjectKind::Rectangle(RectData {
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
            _ => tr("Criar retângulo", "Create rectangle"),
        };
        let id = ops::add_object(&mut doc.editor, doc.page, layer, o, name);
        doc.selection = vec![id];
    }
}

/// Espiral (caminho aberto, só contorno) ou papel gráfico (grupo de
/// retângulos) no quadro levado à página por `m`.
fn add_generated(doc: &mut DocState, prim: Prim, w: f64, h: f64, m: Affine, defaults: &Defaults) {
    use graphdraw_core::edit::generate;
    let Some(layer) = doc.layer_for_new_object() else {
        return;
    };
    let mut like = Object::new(ObjectKind::Group(graphdraw_core::model::GroupData {
        children: Vec::new(),
        extra: Map::new(),
    }));
    like.fill = defaults.fill.clone();
    like.stroke = defaults.stroke.clone();
    let (mut o, name) = match prim {
        Prim::Spiral => {
            let mut o = Object::new(ObjectKind::Path(generate::spiral(
                w,
                h,
                defaults.spiral_turns,
                defaults.spiral_log,
                defaults.spiral_expansion,
            )));
            // Caminho aberto: contorno, sem preenchimento.
            o.stroke = defaults.stroke.clone().or_else(|| {
                Some(graphdraw_core::model::Stroke::new(
                    250.0,
                    graphdraw_core::model::Color::black(),
                ))
            });
            (o, tr("Criar espiral", "Create spiral"))
        }
        _ => (
            generate::graph_paper(w, h, defaults.grid_cols, defaults.grid_rows, &like),
            tr("Criar papel gráfico", "Create graph paper"),
        ),
    };
    o.transform = Transform::from_affine(m);
    let id = ops::add_object(&mut doc.editor, doc.page, layer, o, name);
    doc.selection = vec![id];
}

/// Guia arrastada de `grab` até `cur`: horizontal só muda y, vertical só
/// x, inclinada desloca o ponto.
fn moved_guide(
    g: &graphdraw_core::model::Guide,
    grab: Point,
    cur: Point,
) -> graphdraw_core::model::Guide {
    let mut n = g.clone();
    let d = cur - grab;
    match g.kind.as_str() {
        "horizontal" => n.position += d.y,
        "vertical" => n.position += d.x,
        _ => {
            let p = g.point.unwrap_or([0.0, 0.0]);
            n.point = Some([p[0] + d.x, p[1] + d.y]);
        }
    }
    n
}

/// Polígono selecionado sozinho: referência, objeto, dados e matriz local →
/// página.
fn single_polygon(
    doc: &DocState,
) -> Option<(ObjRef, Object, graphdraw_core::model::PolygonData, Affine)> {
    let [id] = doc.selection.as_slice() else {
        return None;
    };
    let r = doc.editor.find(id)?;
    let o = doc.editor.get(&r)?.clone();
    let ObjectKind::Polygon(p) = &o.kind else {
        return None;
    };
    let m = doc.editor.parent_affine(&r) * o.transform.affine();
    Some((r, o.clone(), p.clone(), m))
}

/// Alças do polígono no espaço local: o primeiro vértice externo e, nas
/// estrelas, o primeiro interno.
fn polygon_handles(p: &graphdraw_core::model::PolygonData) -> Vec<(bool, Point)> {
    let n = p.sides.clamp(3, 500) as f64;
    let at = |a: f64, k: f64| Point::new(p.rx * k * a.cos(), p.ry * k * a.sin());
    let top = std::f64::consts::FRAC_PI_2;
    let mut v = vec![(false, at(top, 1.0))];
    if p.mode == "star" {
        let inner = 1.0 - 0.95 * p.sharpness.clamp(0.0, 1.0);
        v.push((true, at(top + std::f64::consts::PI / n, inner)));
    }
    v
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

/// Objeto selecionado sozinho com gradiente: referência, objeto, gradiente
/// e matriz local → página.
fn single_gradient(
    doc: &DocState,
) -> Option<(ObjRef, Object, graphdraw_core::model::Gradient, Affine)> {
    let [id] = doc.selection.as_slice() else {
        return None;
    };
    let r = doc.editor.find(id)?;
    let o = doc.editor.get(&r)?.clone();
    let Some(Fill::Gradient(g)) = &o.fill else {
        return None;
    };
    let g = g.clone();
    let m = doc.editor.parent_affine(&r) * o.transform.affine();
    Some((r, o, g, m))
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

/// Ponto e normal (página) do contorno do objeto sob `p` (GD-FER-21).
fn outline_normal(doc: &DocState, p: Point, tol: f64) -> Option<(Point, kurbo::Vec2)> {
    let id = query::hit_test_deep(doc.page(), p, tol)?;
    let r = doc.editor.find(&id)?;
    let o = doc.editor.get(&r)?;
    let pd = outline_pd(o)?;
    let m = doc.editor.parent_affine(&r) * o.transform.affine();
    graphdraw_core::edit::drawing::normal_at(&pd, m, p, tol)
}

/// Ponto de tangência, visto de `from`, no objeto sob `cur` (GD-FER-21).
fn outline_tangent(doc: &DocState, from: Point, cur: Point, tol: f64) -> Option<Point> {
    let id = query::hit_test_deep(doc.page(), cur, tol)?;
    let r = doc.editor.find(&id)?;
    let o = doc.editor.get(&r)?;
    let pd = outline_pd(o)?;
    let m = doc.editor.parent_affine(&r) * o.transform.affine();
    graphdraw_core::edit::drawing::tangent_point(&pd, m, from, cur)
        .filter(|q| (*q - cur).hypot() <= tol * 3.0)
}

fn outline_pd(o: &Object) -> Option<PathData> {
    match &o.kind {
        ObjectKind::Path(p) => Some(p.clone()),
        ObjectKind::Group(_) | ObjectKind::Image(_) | ObjectKind::Text(_) => None,
        _ => Some(PathData {
            subpaths: graphdraw_core::edit::convert::bez_to_subpaths(&query::local_outline(o)?),
            fill_rule: FillRule::Nonzero,
            extra: Map::new(),
        }),
    }
}

/// Nome da cor na paleta do documento quando é a mesma (GD-CTG-02).
fn palette_name(doc: &DocState, c: &graphdraw_core::model::Color) -> Option<String> {
    let hex = c.to_hex();
    doc.doc().colors.colors.iter().find_map(|n| {
        graphdraw_core::edit::palette::linked(&doc.editor, &n.id)
            .filter(|l| l.to_hex() == hex)
            .map(|_| n.name.clone())
    })
}

/// Texto de exemplo (GD-BDR-60): frases neutras do projeto, até encher o
/// quadro sem passar dele.
fn fill_sample_text(t: &mut graphdraw_core::model::TextData) {
    use graphdraw_core::edit::text;
    let sentence = tr(
        "Este é um texto de exemplo para ver a fonte, o corpo e o espaço do quadro. ",
        "This is sample text to preview the font, the size and the frame space. ",
    );
    let n = text::len(t);
    text::delete(t, 0, n);
    for _ in 0..200 {
        let before = text::len(t);
        text::insert(t, before, sentence);
        if graphdraw_core::render::text::layout_text(t).overflow {
            let after = text::len(t);
            text::delete(t, before, after);
            break;
        }
    }
    let end = text::len(t);
    if end > 0 && graphdraw_core::render::text::plain_text(t).ends_with(' ') {
        text::delete(t, end - 1, end);
    }
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
    /// GD-PRI-03: na ferramenta Forma, arrastar o vértice interno da estrela
    /// muda a nitidez de todas as pontas; o externo, o tamanho de todas.
    #[test]
    fn polygon_vertex_edit_is_symmetric() {
        let ctx = crate::theme::test_ctx();
        let mut cs = CanvasState::default();
        cs.snap.enabled = false;
        let mut doc = DocState::new_blank();
        let mut o = Object::new(ObjectKind::Polygon(graphdraw_core::model::PolygonData {
            rx: 30_000.0,
            ry: 30_000.0,
            sides: 5,
            mode: "star".into(),
            sharpness: 0.5,
            extra: Map::new(),
        }));
        o.transform = graphdraw_core::model::Transform::translate(100_000.0, 150_000.0);
        let id = o.id.clone();
        doc.editor.insert_object(
            graphdraw_core::edit::ObjLoc {
                page: 0,
                layer: 0,
                index: 0,
            },
            o,
        );
        doc.selection = vec![id.clone()];
        for _ in 0..2 {
            canvas_frame(&ctx, &mut cs, &mut doc, Tool::Shape, vec![]);
        }
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(1366.0, 768.0));
        let (_, ph) = doc.page_size();
        let btn = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        let drag = |cs: &mut CanvasState, doc: &mut DocState, a: Point, b: Point| {
            let (sa, sb) = (cs.screen(rect, ph, a), cs.screen(rect, ph, b));
            canvas_frame(
                &ctx,
                cs,
                doc,
                Tool::Shape,
                vec![egui::Event::PointerMoved(sa)],
            );
            canvas_frame(&ctx, cs, doc, Tool::Shape, vec![btn(sa, true)]);
            for k in 1..=6 {
                canvas_frame(
                    &ctx,
                    cs,
                    doc,
                    Tool::Shape,
                    vec![egui::Event::PointerMoved(sa + (sb - sa) * (k as f32 / 6.0))],
                );
            }
            canvas_frame(&ctx, cs, doc, Tool::Shape, vec![btn(sb, false)]);
        };
        let get = |doc: &DocState| match &doc.page().layers[0].objects[0].kind {
            ObjectKind::Polygon(p) => p.clone(),
            _ => unreachable!(),
        };
        // Vértice externo no topo (100, 180) → puxar para (100, 190): raio 40.
        drag(
            &mut cs,
            &mut doc,
            Point::new(100_000.0, 180_000.0),
            Point::new(100_000.0, 190_000.0),
        );
        let p = get(&doc);
        let tol = 3.0 / cs.view.zoom;
        assert!(
            (p.rx - 40_000.0).abs() < tol && (p.ry - 40_000.0).abs() < tol,
            "{} {}",
            p.rx,
            p.ry
        );
        // Vértice interno (raio 0,525 · 40 a 126°) → levar para raio 0,81·40: nitidez 0,2.
        let a = std::f64::consts::FRAC_PI_2 + std::f64::consts::PI / 5.0;
        let at = |k: f64| {
            Point::new(
                100_000.0 + 40_000.0 * k * a.cos(),
                150_000.0 + 40_000.0 * k * a.sin(),
            )
        };
        drag(&mut cs, &mut doc, at(0.525), at(0.81));
        assert!(
            (get(&doc).sharpness - 0.2).abs() < 0.02,
            "{}",
            get(&doc).sharpness
        );
        doc.editor.undo();
        assert!(
            (get(&doc).sharpness - 0.5).abs() < 1e-9,
            "desfazer: um passo por arrasto"
        );
    }

    /// GD-ALI-05: ao mover, a borda encaixa na borda de outro objeto.
    #[test]
    fn smart_guides_align_while_moving() {
        let ctx = crate::theme::test_ctx();
        let mut cs = CanvasState::default();
        cs.snap.grid = None;
        cs.snap.objects = false;
        cs.snap.page = false;
        cs.snap.guides = false;
        let mut doc = DocState::new_blank();
        let sq = |x: f64, y: f64| {
            let mut o = Object::new(ObjectKind::Rectangle(RectData {
                width: 20_000.0,
                height: 20_000.0,
                corners: None,
                extra: Map::new(),
            }));
            o.transform = graphdraw_core::model::Transform::translate(x, y);
            o.fill = Some(Fill::solid(graphdraw_core::model::Color::black()));
            o
        };
        let (a, b) = (sq(30_000.0, 150_000.0), sq(100_000.0, 100_000.0));
        let bid = b.id.clone();
        doc.editor.insert_object(
            graphdraw_core::edit::ObjLoc {
                page: 0,
                layer: 0,
                index: 0,
            },
            a,
        );
        doc.editor.insert_object(
            graphdraw_core::edit::ObjLoc {
                page: 0,
                layer: 0,
                index: 1,
            },
            b,
        );
        doc.selection = vec![bid.clone()];
        for _ in 0..2 {
            canvas_frame(&ctx, &mut cs, &mut doc, Tool::Select, vec![]);
        }
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(1366.0, 768.0));
        let (_, ph) = doc.page_size();
        // Pega no meio do B e leva até a borda esquerda ficar a 0,3 mm de x = 30 mm.
        let s0 = cs.screen(rect, ph, Point::new(110_000.0, 110_000.0));
        let s1 = cs.screen(rect, ph, Point::new(40_300.0, 110_000.0));
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
            Tool::Select,
            vec![egui::Event::PointerMoved(s0)],
        );
        canvas_frame(&ctx, &mut cs, &mut doc, Tool::Select, vec![btn(s0, true)]);
        let mut saw_line = false;
        for k in 1..=8 {
            let pos = s0 + (s1 - s0) * (k as f32 / 8.0);
            canvas_frame(
                &ctx,
                &mut cs,
                &mut doc,
                Tool::Select,
                vec![egui::Event::PointerMoved(pos)],
            );
            saw_line |= cs
                .smart_lines
                .iter()
                .any(|l| l.vertical && (l.at - 30_000.0).abs() < 1e-6);
        }
        canvas_frame(&ctx, &mut cs, &mut doc, Tool::Select, vec![btn(s1, false)]);
        assert!(saw_line, "linha de alinhamento em x = 30 mm");
        let r = doc.editor.find(&bid).unwrap();
        let b = ops::object_bounds(&doc.editor, &r, false).unwrap();
        assert!((b.x0 - 30_000.0).abs() < 1e-6, "encaixou: {}", b.x0);
        assert!(cs.smart_lines.is_empty(), "linhas somem ao soltar");
    }

    /// GD-ALI-05: ao redimensionar, a alça encaixa na borda de outro
    /// objeto; ao desenhar, o canto também.
    #[test]
    fn smart_guides_on_resize_and_draw() {
        let ctx = crate::theme::test_ctx();
        let mut cs = CanvasState::default();
        cs.snap.grid = None;
        cs.snap.objects = false;
        cs.snap.page = false;
        cs.snap.guides = false;
        let mut doc = DocState::new_blank();
        let sq = |x: f64, y: f64| {
            let mut o = Object::new(ObjectKind::Rectangle(RectData {
                width: 20_000.0,
                height: 20_000.0,
                corners: None,
                extra: Map::new(),
            }));
            o.transform = graphdraw_core::model::Transform::translate(x, y);
            o.fill = Some(Fill::solid(graphdraw_core::model::Color::black()));
            o
        };
        let (a, b) = (sq(30_000.0, 150_000.0), sq(100_000.0, 100_000.0));
        let bid = b.id.clone();
        for (i, o) in [a, b].into_iter().enumerate() {
            doc.editor.insert_object(
                graphdraw_core::edit::ObjLoc {
                    page: 0,
                    layer: 0,
                    index: i,
                },
                o,
            );
        }
        doc.selection = vec![bid.clone()];
        for _ in 0..2 {
            canvas_frame(&ctx, &mut cs, &mut doc, Tool::Select, vec![]);
        }
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(1366.0, 768.0));
        let (_, ph) = doc.page_size();
        let btn = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        let drag = |cs: &mut CanvasState, doc: &mut DocState, tool: Tool, a: Point, b: Point| {
            let (sa, sb) = (cs.screen(rect, ph, a), cs.screen(rect, ph, b));
            canvas_frame(&ctx, cs, doc, tool, vec![egui::Event::PointerMoved(sa)]);
            canvas_frame(&ctx, cs, doc, tool, vec![btn(sa, true)]);
            let mut saw = false;
            for k in 1..=6 {
                let pos = sa + (sb - sa) * (k as f32 / 6.0);
                canvas_frame(&ctx, cs, doc, tool, vec![egui::Event::PointerMoved(pos)]);
                saw |= !cs.smart_lines.is_empty();
            }
            canvas_frame(&ctx, cs, doc, tool, vec![btn(sb, false)]);
            saw
        };
        // Alça do meio à direita do B (120, 110) até x = 130,3: nada perto,
        // a largura segue o cursor.
        drag(
            &mut cs,
            &mut doc,
            Tool::Select,
            Point::new(120_000.0, 110_000.0),
            Point::new(130_300.0, 110_000.0),
        );
        let r = doc.editor.find(&bid).unwrap();
        let bb = ops::object_bounds(&doc.editor, &r, false).unwrap();
        assert!((bb.x1 - 130_300.0).abs() < 50.0, "{}", bb.x1);
        // Agora até 0,3 mm do centro da página em x (105 mm).
        doc.editor.undo();
        let saw = drag(
            &mut cs,
            &mut doc,
            Tool::Select,
            Point::new(120_000.0, 110_000.0),
            Point::new(105_300.0, 110_000.0),
        );
        let r = doc.editor.find(&bid).unwrap();
        let bb = ops::object_bounds(&doc.editor, &r, false).unwrap();
        assert!(saw, "linha ao redimensionar");
        assert!((bb.x1 - 105_000.0).abs() < 1e-6, "alça encaixou: {}", bb.x1);
        assert!(
            (bb.y1 - bb.y0 - 20_000.0).abs() < 1e-6,
            "lado não muda a altura"
        );
        // Desenho: canto final a 0,3 mm da borda esquerda de A (x = 30).
        doc.selection.clear();
        let saw = drag(
            &mut cs,
            &mut doc,
            Tool::Rect,
            Point::new(10_000.0, 40_000.0),
            Point::new(29_700.0, 60_000.0),
        );
        assert!(saw, "linha ao desenhar");
        let layer = &doc.page().layers[0].objects;
        let o = layer.last().unwrap();
        let r = doc.editor.find(&o.id).unwrap();
        let nb = ops::object_bounds(&doc.editor, &r, false).unwrap();
        assert!((nb.x1 - 30_000.0).abs() < 1e-6, "canto encaixou: {}", nb.x1);
    }

    /// GD-FIL-11: na ferramenta de preenchimento interativo, arrastar sobre o
    /// objeto cria o gradiente; arrastar a alça do fim muda o eixo; duplo
    /// clique na linha cria parada e Delete a tira.
    #[test]
    fn interactive_gradient_tool() {
        use graphdraw_core::model::Gradient;
        let ctx = crate::theme::test_ctx();
        let mut cs = CanvasState::default();
        cs.snap.enabled = false;
        let mut doc = DocState::new_blank();
        let mut o = Object::new(ObjectKind::Rectangle(RectData {
            width: 60_000.0,
            height: 40_000.0,
            corners: None,
            extra: Map::new(),
        }));
        o.transform = graphdraw_core::model::Transform::translate(50_000.0, 100_000.0);
        let red = graphdraw_core::model::Color::from_hex("#ff0000").unwrap();
        o.fill = Some(Fill::solid(red.clone()));
        let id = o.id.clone();
        doc.editor.insert_object(
            graphdraw_core::edit::ObjLoc {
                page: 0,
                layer: 0,
                index: 0,
            },
            o,
        );
        let t = Tool::Gradient;
        for _ in 0..2 {
            canvas_frame(&ctx, &mut cs, &mut doc, t, vec![]);
        }
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(1366.0, 768.0));
        let (_, ph) = doc.page_size();
        let btn = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        let drag = |cs: &mut CanvasState, doc: &mut DocState, a: Point, b: Point| {
            let (sa, sb) = (cs.screen(rect, ph, a), cs.screen(rect, ph, b));
            canvas_frame(&ctx, cs, doc, t, vec![egui::Event::PointerMoved(sa)]);
            canvas_frame(&ctx, cs, doc, t, vec![btn(sa, true)]);
            for k in 1..=6 {
                let pos = sa + (sb - sa) * (k as f32 / 6.0);
                canvas_frame(&ctx, cs, doc, t, vec![egui::Event::PointerMoved(pos)]);
            }
            canvas_frame(&ctx, cs, doc, t, vec![btn(sb, false)]);
        };
        let grad = |doc: &DocState| -> Gradient {
            match &doc.page().layers[0].objects[0].fill {
                Some(Fill::Gradient(g)) => g.clone(),
                f => panic!("sem gradiente: {f:?}"),
            }
        };
        let tol = 2.0 / cs.view.zoom;
        // Da esquerda (55, 120) à direita (105, 120) da página.
        drag(
            &mut cs,
            &mut doc,
            Point::new(55_000.0, 120_000.0),
            Point::new(105_000.0, 120_000.0),
        );
        assert_eq!(doc.selection, vec![id.clone()]);
        let g = grad(&doc);
        let (a, b) = (g.start.unwrap(), g.end.unwrap());
        // Coordenadas locais: o objeto começa em (50, 100).
        assert!(
            (a[0] - 5_000.0).abs() < tol && (a[1] - 20_000.0).abs() < tol,
            "{a:?}"
        );
        assert!((b[0] - 55_000.0).abs() < tol, "{b:?}");
        assert_eq!(g.stops[0].color, red, "parte da cor sólida");
        // Arrastar a alça do fim para (85, 120).
        drag(
            &mut cs,
            &mut doc,
            Point::new(105_000.0, 120_000.0),
            Point::new(85_000.0, 120_000.0),
        );
        assert!((grad(&doc).end.unwrap()[0] - 35_000.0).abs() < tol);
        // Duplo clique no meio da linha: parada nova.
        let mid = cs.screen(rect, ph, Point::new(70_000.0, 120_000.0));
        canvas_frame(
            &ctx,
            &mut cs,
            &mut doc,
            t,
            vec![egui::Event::PointerMoved(mid)],
        );
        for _ in 0..2 {
            canvas_frame(&ctx, &mut cs, &mut doc, t, vec![btn(mid, true)]);
            canvas_frame(&ctx, &mut cs, &mut doc, t, vec![btn(mid, false)]);
        }
        let g = grad(&doc);
        assert_eq!(g.stops.len(), 3);
        assert_eq!(cs.grad_stop.as_ref().map(|s| s.1), Some(1));
        assert!(
            (g.stops[1].offset - 0.5).abs() < 0.02,
            "{}",
            g.stops[1].offset
        );
        let blue = graphdraw_core::model::Color::from_hex("#0000ff").unwrap();
        assert!(cs.set_grad_stop_color(&mut doc, blue.clone()));
        assert_eq!(grad(&doc).stops[1].color, blue);
        assert!(cs.delete_grad_stop(&mut doc));
        assert_eq!(grad(&doc).stops.len(), 2);
        // Desfazer volta passo a passo: exclusão, cor, parada, alça, criação.
        doc.editor.undo();
        assert_eq!(grad(&doc).stops.len(), 3);
        // Depois de desfazer, a parada escolhida não vale mais (Delete e a
        // paleta não agem numa parada que pode ser outra).
        assert!(!cs.delete_grad_stop(&mut doc));
        doc.editor.undo();
        assert_ne!(grad(&doc).stops[1].color, blue);
        doc.editor.undo();
        assert_eq!(grad(&doc).stops.len(), 2);
        assert!((grad(&doc).end.unwrap()[0] - 35_000.0).abs() < tol);
        doc.editor.undo();
        assert!((grad(&doc).end.unwrap()[0] - 55_000.0).abs() < tol);
        doc.editor.undo();
        assert!(matches!(
            doc.page().layers[0].objects[0].fill,
            Some(Fill::Solid { .. })
        ));
    }

    /// GD-FER-32: a família da ferramenta Polígono escolhe espiral (caminho
    /// aberto, com contorno) ou papel gráfico (grupo de células).
    #[test]
    fn polygon_tool_draws_spiral_and_graph_paper() {
        let mut doc = DocState::new_blank();
        let mut d = Defaults {
            shape_family: "spiral".into(),
            ..Default::default()
        };
        assert_eq!(Prim::of(Tool::Polygon, &d), Prim::Spiral);
        add_shape(
            &mut doc,
            Prim::Spiral,
            40_000.0,
            40_000.0,
            Affine::translate((10_000.0, 10_000.0)),
            &d,
        );
        let o = doc.page().layers[0].objects.last().unwrap().clone();
        let ObjectKind::Path(p) = &o.kind else {
            panic!("espiral é caminho")
        };
        assert!(!p.subpaths[0].closed && o.stroke.is_some() && o.fill.is_none());
        d.shape_family = "graph".into();
        d.grid_cols = 3;
        d.grid_rows = 2;
        assert_eq!(Prim::of(Tool::Polygon, &d), Prim::Grid);
        add_shape(
            &mut doc,
            Prim::Grid,
            30_000.0,
            20_000.0,
            Affine::translate((0.0, 0.0)),
            &d,
        );
        let g = doc.page().layers[0].objects.last().unwrap();
        let ObjectKind::Group(gd) = &g.kind else {
            panic!("papel gráfico é grupo")
        };
        assert_eq!(gd.children.len(), 6);
        assert_eq!(doc.selection, vec![g.id.clone()]);
    }

    /// Clicar na ponta do gradiente abre o balão da parada da ponta; Esc
    /// fecha e fecha o passo de desfazer.
    #[test]
    fn clicking_gradient_end_opens_stop_popup() {
        let ctx = crate::theme::test_ctx();
        let mut cs = CanvasState::default();
        cs.snap.enabled = false;
        let mut doc = DocState::new_blank();
        let mut o = Object::new(ObjectKind::Rectangle(RectData {
            width: 60_000.0,
            height: 40_000.0,
            corners: None,
            extra: Map::new(),
        }));
        o.transform = graphdraw_core::model::Transform::translate(50_000.0, 100_000.0);
        o.fill = Some(Fill::Gradient(graphdraw_core::edit::gradtool::linear_from(
            Point::new(0.0, 20_000.0),
            Point::new(60_000.0, 20_000.0),
            None,
        )));
        let id = o.id.clone();
        doc.editor.insert_object(
            graphdraw_core::edit::ObjLoc {
                page: 0,
                layer: 0,
                index: 0,
            },
            o,
        );
        doc.selection = vec![id.clone()];
        let t = Tool::Gradient;
        for _ in 0..2 {
            canvas_frame(&ctx, &mut cs, &mut doc, t, vec![]);
        }
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(1366.0, 768.0));
        let (_, ph) = doc.page_size();
        let end = cs.screen(rect, ph, Point::new(110_000.0, 120_000.0));
        let btn = |pressed| egui::Event::PointerButton {
            pos: end,
            button: PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        canvas_frame(
            &ctx,
            &mut cs,
            &mut doc,
            t,
            vec![egui::Event::PointerMoved(end)],
        );
        canvas_frame(&ctx, &mut cs, &mut doc, t, vec![btn(true)]);
        canvas_frame(&ctx, &mut cs, &mut doc, t, vec![btn(false)]);
        canvas_frame(&ctx, &mut cs, &mut doc, t, vec![]);
        assert!(cs.grad_popup.is_some(), "balão aberto");
        assert_eq!(cs.grad_stop.as_ref().map(|s| s.1), Some(1), "parada do fim");
        assert!(doc.editor.in_transaction());
        let esc = egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        canvas_frame(&ctx, &mut cs, &mut doc, t, vec![esc]);
        assert!(cs.grad_popup.is_none());
        assert!(!doc.editor.in_transaction(), "passo fechado");
    }

    fn bdr_setup() -> (egui::Context, CanvasState, DocState, String, Rect, f64) {
        let ctx = crate::theme::test_ctx();
        let mut cs = CanvasState::default();
        cs.snap.enabled = false;
        cs.smart_guides = false;
        let mut doc = DocState::new_blank();
        let mut o = Object::new(ObjectKind::Rectangle(RectData {
            width: 20_000.0,
            height: 20_000.0,
            corners: None,
            extra: Map::new(),
        }));
        o.transform = graphdraw_core::model::Transform::translate(50_000.0, 100_000.0);
        o.fill = Some(Fill::solid(graphdraw_core::model::Color::black()));
        let id = o.id.clone();
        doc.editor.insert_object(
            graphdraw_core::edit::ObjLoc {
                page: 0,
                layer: 0,
                index: 0,
            },
            o,
        );
        for _ in 0..2 {
            canvas_frame(&ctx, &mut cs, &mut doc, Tool::Select, vec![]);
        }
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(1366.0, 768.0));
        let (_, ph) = doc.page_size();
        (ctx, cs, doc, id, rect, ph)
    }

    fn button(pos: Pos2, b: PointerButton, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button: b,
            pressed,
            modifiers: egui::Modifiers::NONE,
        }
    }

    fn x_of(doc: &DocState, id: &str) -> f64 {
        ops::object_bounds(&doc.editor, &doc.editor.find(id).unwrap(), false)
            .unwrap()
            .x0
    }

    /// Bloqueado: o esquerdo não seleciona; o direito abre o menu dele
    /// (Desbloquear e Ordem) e desbloquear vale para ele.
    #[test]
    fn locked_object_reached_only_by_right_click() {
        let (ctx, mut cs, mut doc, id, rect, ph) = bdr_setup();
        ops::set_locked(&mut doc.editor, std::slice::from_ref(&id), true);
        let at = cs.screen(rect, ph, Point::new(60_000.0, 110_000.0));
        canvas_frame(
            &ctx,
            &mut cs,
            &mut doc,
            Tool::Select,
            vec![egui::Event::PointerMoved(at)],
        );
        canvas_frame(
            &ctx,
            &mut cs,
            &mut doc,
            Tool::Select,
            vec![button(at, PointerButton::Primary, true)],
        );
        canvas_frame(
            &ctx,
            &mut cs,
            &mut doc,
            Tool::Select,
            vec![button(at, PointerButton::Primary, false)],
        );
        assert!(doc.selection.is_empty(), "esquerdo passa reto");
        canvas_frame(
            &ctx,
            &mut cs,
            &mut doc,
            Tool::Select,
            vec![button(at, PointerButton::Secondary, true)],
        );
        canvas_frame(
            &ctx,
            &mut cs,
            &mut doc,
            Tool::Select,
            vec![button(at, PointerButton::Secondary, false)],
        );
        let m = cs.menu.clone().expect("menu do bloqueado");
        assert!(
            matches!(&m.target, MenuTarget::Ctx(crate::ctxmenu::Target::Object(o)) if *o == id)
        );
        assert!(doc.selection.is_empty(), "abrir o menu não seleciona");
        let mut ev = CanvasEvents::default();
        cs.exec_menu(
            crate::ctxmenu::Cmd::Run(crate::keymap::Command::UnlockSelection),
            &m,
            &mut doc,
            &mut ev,
        );
        assert_eq!(doc.selection, vec![id]);
        assert_eq!(ev.command, Some(crate::keymap::Command::UnlockSelection));
    }

    /// GD-FER-20/21/25/26 na tela: mão livre e linha 2 pontos (com
    /// perpendicular) criam caminhos; polilinha junta retas e trecho livre;
    /// curva 3 pontos passa pelo clique.
    #[test]
    fn line_tools_on_canvas() {
        let (ctx, mut cs, mut doc, _id, rect, ph) = bdr_setup();
        let view = cs.view;
        let at = |x: f64, y: f64| {
            let (a, b) = view.page_to_screen(ph, x, y);
            rect.min + Vec2::new(a as f32, b as f32)
        };
        let drag = |cs: &mut CanvasState, doc: &mut DocState, tool: Tool, pts: &[Pos2]| {
            canvas_frame(&ctx, cs, doc, tool, vec![egui::Event::PointerMoved(pts[0])]);
            canvas_frame(
                &ctx,
                cs,
                doc,
                tool,
                vec![button(pts[0], PointerButton::Primary, true)],
            );
            for w in pts.windows(2) {
                for k in 1..=4 {
                    let q = w[0] + (w[1] - w[0]) * (k as f32 / 4.0);
                    canvas_frame(&ctx, cs, doc, tool, vec![egui::Event::PointerMoved(q)]);
                }
            }
            let last = *pts.last().unwrap();
            canvas_frame(
                &ctx,
                cs,
                doc,
                tool,
                vec![button(last, PointerButton::Primary, false)],
            );
        };
        let click = |cs: &mut CanvasState, doc: &mut DocState, tool: Tool, p: Pos2| {
            canvas_frame(&ctx, cs, doc, tool, vec![egui::Event::PointerMoved(p)]);
            canvas_frame(
                &ctx,
                cs,
                doc,
                tool,
                vec![button(p, PointerButton::Primary, true)],
            );
            canvas_frame(
                &ctx,
                cs,
                doc,
                tool,
                vec![button(p, PointerButton::Primary, false)],
            );
        };
        let last_path = |doc: &DocState| -> PathData {
            match &doc.page().layers[0].objects.last().unwrap().kind {
                ObjectKind::Path(p) => p.clone(),
                k => panic!("{k:?}"),
            }
        };
        let count = |doc: &DocState| doc.page().layers[0].objects.len();
        // Mão livre.
        let n0 = count(&doc);
        drag(
            &mut cs,
            &mut doc,
            Tool::Freehand,
            &[
                at(120_000.0, 50_000.0),
                at(140_000.0, 60_000.0),
                at(160_000.0, 50_000.0),
            ],
        );
        assert_eq!(count(&doc), n0 + 1);
        assert!(!last_path(&doc).subpaths[0].closed);
        // Linha 2 pontos, perpendicular à base do quadrado (y = 100 mm).
        cs.line_mode = LineMode::Perpendicular;
        drag(
            &mut cs,
            &mut doc,
            Tool::Line2,
            &[at(60_000.0, 100_000.0), at(75_000.0, 80_000.0)],
        );
        let l = last_path(&doc);
        let (a, b) = (l.subpaths[0].nodes[0].p, l.subpaths[0].nodes[1].p);
        assert!((a[0] - b[0]).abs() < 1.0, "perpendicular: {a:?} {b:?}");
        assert!(b[1] < 85_000.0);
        cs.line_mode = LineMode::Free;
        // Curva 3 pontos.
        drag(
            &mut cs,
            &mut doc,
            Tool::Curve3,
            &[at(120_000.0, 150_000.0), at(180_000.0, 150_000.0)],
        );
        let n1 = count(&doc);
        click(&mut cs, &mut doc, Tool::Curve3, at(150_000.0, 170_000.0));
        assert_eq!(count(&doc), n1 + 1);
        let c = graphdraw_core::geometry::subpath_segments(&last_path(&doc).subpaths[0])[0];
        assert!(
            (kurbo::ParamCurve::eval(&c, 0.5) - Point::new(150_000.0, 170_000.0)).hypot() < 300.0
        );
        // Polilinha: dois cliques (reta) e um trecho livre; termina.
        click(&mut cs, &mut doc, Tool::Polyline, at(120_000.0, 200_000.0));
        click(&mut cs, &mut doc, Tool::Polyline, at(150_000.0, 200_000.0));
        drag(
            &mut cs,
            &mut doc,
            Tool::Polyline,
            &[
                at(150_000.0, 200_000.0),
                at(165_000.0, 215_000.0),
                at(180_000.0, 200_000.0),
            ],
        );
        cs.finish_bezier(&mut doc, &Defaults::default(), false);
        let pl = last_path(&doc);
        assert!(
            pl.subpaths[0].nodes.len() >= 3,
            "{:?}",
            pl.subpaths[0].nodes.len()
        );
        let end = pl.subpaths[0].nodes.last().unwrap().p;
        assert!((end[0] - 180_000.0).abs() < 1_000.0, "{end:?}");
    }

    /// GD-FER-16/17/18/19 na tela: faca arrastada corta o quadrado em dois;
    /// recorte com Enter tira o que está fora; borracha abre furo; clique no
    /// segmento o apaga.
    #[test]
    fn cut_tools_on_canvas() {
        let (ctx, mut cs, mut doc, id, rect, ph) = bdr_setup();
        let at = |cs: &CanvasState, x: f64, y: f64| cs.screen(rect, ph, Point::new(x, y));
        let drag = |cs: &mut CanvasState, doc: &mut DocState, tool: Tool, a: Pos2, b: Pos2| {
            canvas_frame(&ctx, cs, doc, tool, vec![egui::Event::PointerMoved(a)]);
            canvas_frame(
                &ctx,
                cs,
                doc,
                tool,
                vec![button(a, PointerButton::Primary, true)],
            );
            for k in 1..=6 {
                let q = a + (b - a) * (k as f32 / 6.0);
                canvas_frame(&ctx, cs, doc, tool, vec![egui::Event::PointerMoved(q)]);
            }
            canvas_frame(
                &ctx,
                cs,
                doc,
                tool,
                vec![button(b, PointerButton::Primary, false)],
            );
        };
        // Faca de cima a baixo pelo meio do quadrado (50..70, 100..120 mm).
        let (a, b) = (at(&cs, 60_000.0, 125_000.0), at(&cs, 60_000.0, 95_000.0));
        drag(&mut cs, &mut doc, Tool::Knife, a, b);
        assert_eq!(doc.page().layers[0].objects.len(), 2, "duas peças");
        assert_eq!(doc.selection.len(), 2);
        doc.editor.undo();
        assert_eq!(doc.page().layers[0].objects.len(), 1);
        // Recorte: retângulo pela metade esquerda; Enter aplica.
        doc.selection.clear();
        let (a, b) = (at(&cs, 40_000.0, 130_000.0), at(&cs, 60_000.0, 90_000.0));
        drag(&mut cs, &mut doc, Tool::Crop, a, b);
        assert!(cs.crop_pending.is_some());
        canvas_frame(
            &ctx,
            &mut cs,
            &mut doc,
            Tool::Crop,
            vec![egui::Event::Key {
                key: Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        assert!(cs.crop_pending.is_none());
        let b0 = ops::object_bounds(&doc.editor, &doc.editor.find(&id).unwrap(), false).unwrap();
        assert!((b0.x1 - 60_000.0).abs() < 10.0, "{b0:?}");
        doc.editor.undo();
        // Borracha no meio: a área diminui.
        let area = |doc: &DocState| {
            graphdraw_core::edit::shapeops::area(&graphdraw_core::edit::shapeops::region(
                &doc.page().layers[0].objects[0],
                kurbo::Affine::IDENTITY,
            ))
        };
        let a0 = area(&doc);
        cs.eraser_width = 4_000.0;
        let (a, b) = (at(&cs, 60_000.0, 112_000.0), at(&cs, 60_000.0, 108_000.0));
        drag(&mut cs, &mut doc, Tool::Eraser, a, b);
        assert!(area(&doc) < a0 - 10_000_000.0);
        doc.editor.undo();
        // Excluir segmento virtual: clique na base do quadrado.
        let p = at(&cs, 60_000.0, 100_000.0);
        canvas_frame(
            &ctx,
            &mut cs,
            &mut doc,
            Tool::VirtualDelete,
            vec![egui::Event::PointerMoved(p)],
        );
        canvas_frame(
            &ctx,
            &mut cs,
            &mut doc,
            Tool::VirtualDelete,
            vec![button(p, PointerButton::Primary, true)],
        );
        canvas_frame(
            &ctx,
            &mut cs,
            &mut doc,
            Tool::VirtualDelete,
            vec![button(p, PointerButton::Primary, false)],
        );
        let o = &doc.page().layers[0].objects[0];
        assert!(
            matches!(&o.kind, ObjectKind::Path(pd) if !pd.subpaths[0].closed),
            "{:?}",
            o.kind
        );
    }

    /// GD-BDR-01: 3 px ainda é clique (abre o menu); 5 px é arrasto (não abre).
    #[test]
    fn gd_bdr_01_click_vs_drag_threshold() {
        let (ctx, mut cs, mut doc, _id, rect, ph) = bdr_setup();
        let at = cs.screen(rect, ph, Point::new(150_000.0, 200_000.0)); // área vazia
        for (dx, opens) in [(3.0, true), (5.0, false)] {
            cs.menu = None;
            let to = at + Vec2::new(dx, 0.0);
            canvas_frame(
                &ctx,
                &mut cs,
                &mut doc,
                Tool::Select,
                vec![egui::Event::PointerMoved(at)],
            );
            canvas_frame(
                &ctx,
                &mut cs,
                &mut doc,
                Tool::Select,
                vec![button(at, PointerButton::Secondary, true)],
            );
            canvas_frame(
                &ctx,
                &mut cs,
                &mut doc,
                Tool::Select,
                vec![egui::Event::PointerMoved(to)],
            );
            canvas_frame(
                &ctx,
                &mut cs,
                &mut doc,
                Tool::Select,
                vec![button(to, PointerButton::Secondary, false)],
            );
            assert_eq!(cs.menu.is_some(), opens, "{dx} px");
        }
    }

    /// GD-BDR-20/22/25: arrastar com o direito não move até escolher;
    /// Cancelar não muda nada nem o histórico; Mover para cá translada.
    #[test]
    fn gd_bdr_25_right_drag_cancel_and_move() {
        let (ctx, mut cs, mut doc, id, rect, ph) = bdr_setup();
        let a = cs.screen(rect, ph, Point::new(60_000.0, 110_000.0));
        let b = cs.screen(rect, ph, Point::new(110_000.0, 110_000.0));
        let undo0 = doc.editor.undo_len();
        let drag = |cs: &mut CanvasState, doc: &mut DocState| {
            canvas_frame(
                &ctx,
                cs,
                doc,
                Tool::Select,
                vec![egui::Event::PointerMoved(a)],
            );
            canvas_frame(
                &ctx,
                cs,
                doc,
                Tool::Select,
                vec![button(a, PointerButton::Secondary, true)],
            );
            for k in 1..=5 {
                let p = a + (b - a) * (k as f32 / 5.0);
                canvas_frame(
                    &ctx,
                    cs,
                    doc,
                    Tool::Select,
                    vec![egui::Event::PointerMoved(p)],
                );
            }
            assert!(
                (x_of(doc, &id) - 50_000.0).abs() < 1e-6,
                "não se move durante o arrasto"
            );
            canvas_frame(
                &ctx,
                cs,
                doc,
                Tool::Select,
                vec![button(b, PointerButton::Secondary, false)],
            );
        };
        drag(&mut cs, &mut doc);
        let m = cs.menu.clone().expect("menu ao soltar");
        assert!(matches!(
            m.target,
            MenuTarget::Ctx(crate::ctxmenu::Target::Drop { over: None, .. })
        ));
        let mut ev = CanvasEvents::default();
        cs.exec_menu(crate::ctxmenu::Cmd::Cancel, &m, &mut doc, &mut ev);
        assert!((x_of(&doc, &id) - 50_000.0).abs() < 1e-6);
        assert_eq!(
            doc.editor.undo_len(),
            undo0,
            "Cancelar não entra no histórico"
        );
        drag(&mut cs, &mut doc);
        let m = cs.menu.clone().unwrap();
        cs.exec_menu(crate::ctxmenu::Cmd::MoveHere, &m, &mut doc, &mut ev);
        let moved = x_of(&doc, &id) - 50_000.0;
        assert!((moved - 50_000.0).abs() < 200.0, "50 mm: {moved}");
    }

    /// GD-BDR-70: mover com o esquerdo, tocar o direito no meio deixa uma
    /// cópia; tudo é um passo de desfazer.
    #[test]
    fn gd_bdr_70_stamp_copy_while_moving() {
        let (ctx, mut cs, mut doc, id, rect, ph) = bdr_setup();
        doc.selection = vec![id.clone()];
        let p = |x: f64| cs.screen(rect, ph, Point::new(x, 110_000.0));
        let (p0, p30, p60) = (p(60_000.0), p(90_000.0), p(120_000.0));
        let undo0 = doc.editor.undo_len();
        canvas_frame(
            &ctx,
            &mut cs,
            &mut doc,
            Tool::Select,
            vec![egui::Event::PointerMoved(p0)],
        );
        canvas_frame(
            &ctx,
            &mut cs,
            &mut doc,
            Tool::Select,
            vec![button(p0, PointerButton::Primary, true)],
        );
        for k in 1..=4 {
            let q = p0 + (p30 - p0) * (k as f32 / 4.0);
            canvas_frame(
                &ctx,
                &mut cs,
                &mut doc,
                Tool::Select,
                vec![egui::Event::PointerMoved(q)],
            );
        }
        canvas_frame(
            &ctx,
            &mut cs,
            &mut doc,
            Tool::Select,
            vec![button(p30, PointerButton::Secondary, true)],
        );
        canvas_frame(
            &ctx,
            &mut cs,
            &mut doc,
            Tool::Select,
            vec![button(p30, PointerButton::Secondary, false)],
        );
        for k in 1..=4 {
            let q = p30 + (p60 - p30) * (k as f32 / 4.0);
            canvas_frame(
                &ctx,
                &mut cs,
                &mut doc,
                Tool::Select,
                vec![egui::Event::PointerMoved(q)],
            );
        }
        canvas_frame(
            &ctx,
            &mut cs,
            &mut doc,
            Tool::Select,
            vec![button(p60, PointerButton::Primary, false)],
        );
        let xs: Vec<f64> = doc.page().layers[0]
            .objects
            .iter()
            .map(|o| {
                ops::object_bounds(&doc.editor, &doc.editor.find(&o.id).unwrap(), false)
                    .unwrap()
                    .x0
            })
            .collect();
        assert_eq!(xs.len(), 2, "original + 1 cópia");
        let tol = 3.0 / cs.view.zoom;
        assert!(
            xs.iter().any(|x| (x - 80_000.0).abs() < tol),
            "cópia a 30 mm: {xs:?}"
        );
        assert!(
            xs.iter().any(|x| (x - 110_000.0).abs() < tol),
            "original a 60 mm: {xs:?}"
        );
        assert_eq!(doc.editor.undo_len(), undo0 + 1, "uma transação");
        doc.editor.undo();
        assert_eq!(doc.page().layers[0].objects.len(), 1);
        assert!((x_of(&doc, &id) - 50_000.0).abs() < 1e-6);
    }

    /// GD-BDR-71: durante outra ação (escalar), tocar o direito cancela tudo.
    #[test]
    fn gd_bdr_71_right_cancels_other_actions() {
        let (ctx, mut cs, mut doc, id, rect, ph) = bdr_setup();
        doc.selection = vec![id.clone()];
        for _ in 0..2 {
            canvas_frame(&ctx, &mut cs, &mut doc, Tool::Select, vec![]);
        }
        let undo0 = doc.editor.undo_len();
        // Alça do canto superior direito (70, 120) arrastada para (90, 140).
        let h = cs.screen(rect, ph, Point::new(70_000.0, 120_000.0));
        let to = cs.screen(rect, ph, Point::new(90_000.0, 140_000.0));
        canvas_frame(
            &ctx,
            &mut cs,
            &mut doc,
            Tool::Select,
            vec![egui::Event::PointerMoved(h)],
        );
        canvas_frame(
            &ctx,
            &mut cs,
            &mut doc,
            Tool::Select,
            vec![button(h, PointerButton::Primary, true)],
        );
        for k in 1..=4 {
            let q = h + (to - h) * (k as f32 / 4.0);
            canvas_frame(
                &ctx,
                &mut cs,
                &mut doc,
                Tool::Select,
                vec![egui::Event::PointerMoved(q)],
            );
        }
        let b = ops::object_bounds(&doc.editor, &doc.editor.find(&id).unwrap(), false).unwrap();
        assert!(b.width() > 25_000.0, "escalou durante o arrasto: {b:?}");
        canvas_frame(
            &ctx,
            &mut cs,
            &mut doc,
            Tool::Select,
            vec![button(to, PointerButton::Secondary, true)],
        );
        canvas_frame(
            &ctx,
            &mut cs,
            &mut doc,
            Tool::Select,
            vec![button(to, PointerButton::Secondary, false)],
        );
        canvas_frame(
            &ctx,
            &mut cs,
            &mut doc,
            Tool::Select,
            vec![button(to, PointerButton::Primary, false)],
        );
        let b = ops::object_bounds(&doc.editor, &doc.editor.find(&id).unwrap(), false).unwrap();
        assert!((b.width() - 20_000.0).abs() < 1e-6, "voltou: {b:?}");
        assert_eq!(doc.editor.undo_len(), undo0, "histórico inalterado");
        assert!(cs.menu.is_none(), "GD-BDR-02: sem menu");
    }

    /// GD-CTG-05/06/12: clicar amostra e vira balde; clicar noutro aplica
    /// no preenchimento, o direito no contorno; Shift reamostra, Alt
    /// alterna, Esc esvazia; quem recebe ganha o realce.
    #[test]
    fn gd_ctg_05_pick_and_apply_cycle() {
        let ctx = crate::theme::test_ctx();
        let mut cs = CanvasState::default();
        cs.snap.enabled = false;
        let mut doc = DocState::new_blank();
        let hex = |h: &str| graphdraw_core::model::Color::from_hex(h).unwrap();
        let sq = |x: f64, h: &str| {
            let mut o = Object::new(ObjectKind::Rectangle(RectData {
                width: 20_000.0,
                height: 20_000.0,
                corners: None,
                extra: Map::new(),
            }));
            o.transform = graphdraw_core::model::Transform::translate(x, 100_000.0);
            o.fill = Some(Fill::solid(hex(h)));
            o
        };
        let objs = [
            sq(30_000.0, "#ff0000"),
            sq(100_000.0, "#0000ff"),
            sq(170_000.0, "#00ff00"),
        ];
        let ids: Vec<String> = objs.iter().map(|o| o.id.clone()).collect();
        for (i, o) in objs.into_iter().enumerate() {
            doc.editor.insert_object(
                graphdraw_core::edit::ObjLoc {
                    page: 0,
                    layer: 0,
                    index: i,
                },
                o,
            );
        }
        for _ in 0..2 {
            canvas_frame(&ctx, &mut cs, &mut doc, Tool::Eyedropper, vec![]);
        }
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(1366.0, 768.0));
        let (_, ph) = doc.page_size();
        let at = |x: f64| cs.screen(rect, ph, Point::new(x, 110_000.0));
        let (red_at, blue_at, green_at) = (at(40_000.0), at(110_000.0), at(180_000.0));
        let click = |cs: &mut CanvasState,
                     doc: &mut DocState,
                     pos: Pos2,
                     b: PointerButton,
                     m: egui::Modifiers| {
            let btn = |pressed| egui::Event::PointerButton {
                pos,
                button: b,
                pressed,
                modifiers: m,
            };
            let frame = |cs: &mut CanvasState, doc: &mut DocState, e: egui::Event| {
                let ev = vec![egui::Event::ModifiersChanged(m), e];
                canvas_frame(&ctx, cs, doc, Tool::Eyedropper, ev);
            };
            frame(cs, doc, egui::Event::PointerMoved(pos));
            frame(cs, doc, btn(true));
            frame(cs, doc, btn(false));
        };
        let fill = |doc: &DocState, id: &str| {
            doc.editor
                .get(&doc.editor.find(id).unwrap())
                .unwrap()
                .fill
                .clone()
        };
        let none = egui::Modifiers::NONE;
        // Pipeta no vermelho → balde; despeja no azul.
        click(&mut cs, &mut doc, red_at, PointerButton::Primary, none);
        assert!(cs.eyedrop.applying && !cs.eyedrop.sampling(), "virou balde");
        click(&mut cs, &mut doc, blue_at, PointerButton::Primary, none);
        assert_eq!(fill(&doc, &ids[1]), Some(Fill::solid(hex("#ff0000"))));
        assert!(cs.eyedrop.ripple.is_some(), "realce");
        // Direito no verde: contorno vermelho, preenchimento fica.
        click(&mut cs, &mut doc, green_at, PointerButton::Secondary, none);
        let g = doc
            .editor
            .get(&doc.editor.find(&ids[2]).unwrap())
            .unwrap()
            .clone();
        assert_eq!(g.fill, Some(Fill::solid(hex("#00ff00"))));
        assert_eq!(g.stroke.unwrap().color, hex("#ff0000"));
        assert!(cs.menu.is_none(), "direito do conta-gotas não abre menu");
        // Shift + clique reamostra (verde) sem aplicar.
        let undo = doc.editor.undo_len();
        click(
            &mut cs,
            &mut doc,
            green_at,
            PointerButton::Primary,
            egui::Modifiers::SHIFT,
        );
        assert_eq!(doc.editor.undo_len(), undo);
        assert_eq!(
            cs.eyedrop.bucket,
            Some(crate::eyedrop::Bucket::Paint(
                graphdraw_core::edit::eyedrop::Paint::Color(hex("#00ff00"))
            ))
        );
        assert_eq!(cs.eyedrop.recent.len(), 2);
        // Alt alterna para a pipeta e de volta ao balde.
        click(
            &mut cs,
            &mut doc,
            blue_at,
            PointerButton::Primary,
            egui::Modifiers::ALT,
        );
        assert!(cs.eyedrop.sampling());
        click(
            &mut cs,
            &mut doc,
            blue_at,
            PointerButton::Primary,
            egui::Modifiers::ALT,
        );
        assert!(!cs.eyedrop.sampling());
        assert_eq!(doc.editor.undo_len(), undo, "alternar não aplica");
        // Esc esvazia.
        assert!(cs.escape(&mut doc));
        assert!(cs.eyedrop.bucket.is_none() && cs.eyedrop.sampling());
    }

    /// GD-PAG-04: com lado a lado, as vizinhas aparecem e clicar numa
    /// delas vai para ela.
    #[test]
    fn spread_shows_neighbors_and_click_switches() {
        let ctx = crate::theme::test_ctx();
        let mut cs = CanvasState::default();
        let mut doc = DocState::new_blank();
        graphdraw_core::edit::pages::add_after(&mut doc.editor, 0);
        graphdraw_core::edit::pages::add_after(&mut doc.editor, 1);
        doc.sync();
        doc.set_page(1);
        cs.spread = true;
        for _ in 0..2 {
            canvas_frame(&ctx, &mut cs, &mut doc, Tool::Select, vec![]);
        }
        // Afasta o zoom para as vizinhas caberem na tela.
        cs.view.zoom_at(0.3, (683.0, 384.0));
        for _ in 0..2 {
            canvas_frame(&ctx, &mut cs, &mut doc, Tool::Select, vec![]);
        }
        assert_eq!(cs.spread_rects.len(), 2, "anterior e seguinte");
        let (i, r) = *cs.spread_rects.iter().find(|(i, _)| *i == 2).unwrap();
        assert_eq!(i, 2);
        let c = r.center();
        let btn = |pressed| egui::Event::PointerButton {
            pos: c,
            button: PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        canvas_frame(
            &ctx,
            &mut cs,
            &mut doc,
            Tool::Select,
            vec![egui::Event::PointerMoved(c)],
        );
        canvas_frame(&ctx, &mut cs, &mut doc, Tool::Select, vec![btn(true)]);
        canvas_frame(&ctx, &mut cs, &mut doc, Tool::Select, vec![btn(false)]);
        assert_eq!(doc.page, 2);
    }

    /// GD-GUI-01: arrastar da régua cria guia; arrastar a guia de volta
    /// para a régua apaga.
    #[test]
    fn ruler_drag_creates_and_removes_guide() {
        let ctx = crate::theme::test_ctx();
        let mut cs = CanvasState::default();
        cs.snap.grid = None;
        cs.snap.enabled = false;
        let mut doc = DocState::new_blank();
        for _ in 0..2 {
            canvas_frame(&ctx, &mut cs, &mut doc, Tool::Select, vec![]);
        }
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(1366.0, 768.0));
        let (_, ph) = doc.page_size();
        let btn = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        let drag = |cs: &mut CanvasState, doc: &mut DocState, a: Pos2, b: Pos2| {
            canvas_frame(
                &ctx,
                cs,
                doc,
                Tool::Select,
                vec![egui::Event::PointerMoved(a)],
            );
            canvas_frame(&ctx, cs, doc, Tool::Select, vec![btn(a, true)]);
            for k in 1..=6 {
                let pos = a + (b - a) * (k as f32 / 6.0);
                canvas_frame(
                    &ctx,
                    cs,
                    doc,
                    Tool::Select,
                    vec![egui::Event::PointerMoved(pos)],
                );
            }
            canvas_frame(&ctx, cs, doc, Tool::Select, vec![btn(b, false)]);
            canvas_frame(&ctx, cs, doc, Tool::Select, vec![]);
        };
        // Da régua de cima até y = 100 mm na página (fora dos objetos).
        let target = cs.screen(rect, ph, Point::new(5_000.0, 100_000.0));
        let from = egui::pos2(target.x, theme::RULER_SIZE / 2.0);
        drag(&mut cs, &mut doc, from, target);
        let g = &doc.page().guides;
        assert_eq!(g.len(), 1, "guia criada");
        assert_eq!(g[0].kind, "horizontal");
        assert!(
            (g[0].position - 100_000.0).abs() < 2.0 / cs.view.zoom,
            "{}",
            g[0].position
        );
        // De volta para a régua: apaga.
        drag(&mut cs, &mut doc, target, from);
        assert!(doc.page().guides.is_empty(), "guia removida");
    }

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
                "mira+tool-gradient",
                "mira+tool-fill-solid",
                "mira+tool-eyedropper",
                "cur-select",
                "mira+tool-knife",
                "mira+tool-crop",
                "mira+tool-eraser",
                "mira+tool-virtual-delete",
                "mira+tool-freehand",
                "mira+tool-polyline",
                "mira+tool-line2",
                "mira+tool-curve3"
            ]
        );
        assert_eq!(
            name(cs.cursor_glyph(Tool::Select, true, &doc, pos, rect, 297_000.0)),
            "cur-pan",
            "espaço = mão"
        );
    }
}
