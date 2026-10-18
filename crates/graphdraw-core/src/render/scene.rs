//! Achata uma página em uma lista de itens de desenho em coordenadas da
//! página (µm, Y para cima). Todos os destinos (tela, PNG, PDF, SVG) partem
//! desta mesma cena, o que mantém a saída consistente entre eles.

use std::collections::HashMap;
use std::f64::consts::PI;
use std::sync::{Arc, Mutex, OnceLock};

use kurbo::{Affine, Arc as KArc, BezPath, PathEl, Point, Shape, Vec2};

use super::text::{TextLayout, layout_text};
use crate::model::geom::subpaths_to_bez;
use crate::model::{
    Color, Corner, Document, EllipseData, Fill, FillRule, Gradient, Object, ObjectKind, Page,
    PolygonData, RectData, Stroke,
};

/// Tinta resolvida.
#[derive(Debug, Clone)]
pub enum Paint {
    Solid {
        color: Color,
        alpha: f64,
    },
    /// Gradiente com coordenadas locais + matriz local→página.
    Gradient {
        gradient: Gradient,
        transform: Affine,
        alpha: f64,
    },
    /// Padrão (GD-FIL-04..06): superladrilho rasterizado e a matriz do
    /// espaço do padrão (µm) → página.
    Pattern {
        tile: Arc<super::pattern::PatternTile>,
        transform: Affine,
        alpha: f64,
    },
}

#[derive(Debug, Clone)]
pub struct StrokeStyle {
    /// Espessura na página (µm); 0 = fio de cabelo.
    pub width: f64,
    pub paint: Paint,
    pub dash: Option<Vec<f64>>,
    pub dash_offset: f64,
    pub cap: String,
    pub join: String,
    pub miter_limit: f64,
    pub behind_fill: bool,
    pub position: StrokePosition,
}

/// Posição do contorno em relação à borda (GD-OUT-04). Interno e externo
/// são desenhados como um traço de espessura dupla recortado pela forma
/// (ou pelo seu complemento); só valem para caminhos todos fechados.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StrokePosition {
    Center,
    Inside,
    Outside,
}

impl StrokePosition {
    pub fn parse(s: &str) -> StrokePosition {
        match s {
            "inside" => StrokePosition::Inside,
            "outside" => StrokePosition::Outside,
            _ => StrokePosition::Center,
        }
    }
}

impl StrokeStyle {
    /// Espessura do traço efetivamente desenhado (antes do recorte).
    pub fn drawn_width(&self) -> f64 {
        match self.position {
            StrokePosition::Center => self.width,
            _ => self.width * 2.0,
        }
    }

    /// Quanto o contorno avança para fora da borda geométrica.
    pub fn outer_extent(&self) -> f64 {
        match self.position {
            StrokePosition::Center => self.width * 0.5,
            StrokePosition::Inside => 0.0,
            StrokePosition::Outside => self.width,
        }
    }
}

/// Todos os subcaminhos são fechados (condição para contorno interno/externo).
pub fn all_closed(path: &BezPath) -> bool {
    let mut open = false;
    let mut any = false;
    for el in path.elements() {
        match el {
            PathEl::MoveTo(_) => {
                if open {
                    return false;
                }
                open = true;
                any = true;
            }
            PathEl::ClosePath => open = false,
            _ => {}
        }
    }
    any && !open
}

#[derive(Debug, Clone)]
pub struct ShapeItem {
    pub object_id: String,
    pub path: BezPath,
    pub fill: Option<Paint>,
    pub fill_rule: FillRule,
    pub stroke: Option<StrokeStyle>,
}

/// Imagem decodificada (RGBA8 não pré-multiplicado).
#[derive(Debug)]
pub struct DecodedImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    /// Bytes originais e tipo MIME, quando reaproveitáveis (PNG/JPEG) em SVG.
    pub original: Option<(Arc<Vec<u8>>, &'static str)>,
}

#[derive(Debug, Clone)]
pub struct ImageItem {
    pub object_id: String,
    /// Matriz local→página; a imagem ocupa `[0,width]×[0,height]` local.
    pub transform: Affine,
    pub width: f64,
    pub height: f64,
    pub image: Arc<DecodedImage>,
    pub opacity: f64,
}

#[derive(Debug, Clone)]
pub struct TextItem {
    pub object_id: String,
    /// Matriz local→página.
    pub transform: Affine,
    pub layout: Arc<TextLayout>,
    /// Glifos convertidos em curvas (coordenadas da página).
    pub outlines: Vec<ShapeItem>,
    pub alpha: f64,
}

#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum Item {
    Shape(ShapeItem),
    Image(ImageItem),
    Text(TextItem),
}

#[derive(Debug, Clone)]
pub struct Scene {
    pub width: f64,
    pub height: f64,
    pub background: Option<Color>,
    /// Sangria da página em µm (a própria ou a do documento).
    pub bleed: f64,
    pub items: Vec<Item>,
    /// Fontes ausentes substituídas (GD-FNT-02).
    pub missing_fonts: Vec<String>,
    /// Modo de mesclagem por objeto (GD-TRA-02); ausente = normal. Vale
    /// para cada item do objeto (como o /BM do PDF).
    pub blends: std::collections::HashMap<String, crate::model::BlendMode>,
    /// Sobreimpressão por objeto: (preenchimento, contorno) (GD-PRE-08).
    pub overprints: std::collections::HashMap<String, (bool, bool)>,
    /// Caixas dos itens (coordenadas da página, já incluindo a espessura do
    /// contorno), calculadas uma vez sob demanda para descarte rápido.
    pub item_bounds: OnceLock<Vec<kurbo::Rect>>,
}

impl Item {
    pub fn object_id(&self) -> &str {
        match self {
            Item::Shape(s) => &s.object_id,
            Item::Image(i) => &i.object_id,
            Item::Text(t) => &t.object_id,
        }
    }
}

impl Scene {
    /// Modo de mesclagem do item.
    pub fn blend_of(&self, item: &Item) -> crate::model::BlendMode {
        if self.blends.is_empty() {
            return crate::model::BlendMode::Normal;
        }
        self.blends
            .get(item.object_id())
            .copied()
            .unwrap_or_default()
    }

    /// Sobreimpressão do item: (preenchimento, contorno).
    pub fn overprint_of(&self, item: &Item) -> (bool, bool) {
        self.overprints
            .get(item.object_id())
            .copied()
            .unwrap_or_default()
    }

    pub fn bounds(&self) -> kurbo::Rect {
        kurbo::Rect::new(0.0, 0.0, self.width, self.height)
    }

    /// Caixa de cada item, na mesma ordem de `items`.
    pub fn item_bounds(&self) -> &[kurbo::Rect] {
        self.item_bounds
            .get_or_init(|| self.items.iter().map(item_bounds).collect())
    }
}

fn shape_bounds(s: &ShapeItem) -> kurbo::Rect {
    let b = s.path.bounding_box();
    match &s.stroke {
        // Meia espessura × limite de esquadria cobre juntas agudas e terminações.
        Some(st) => {
            let d = st.outer_extent() * st.miter_limit.max(1.5);
            b.inflate(d, d)
        }
        None => b,
    }
}

/// Caixa conservadora de um item em coordenadas da página.
pub fn item_bounds(item: &Item) -> kurbo::Rect {
    match item {
        Item::Shape(s) => shape_bounds(s),
        Item::Image(i) => i
            .transform
            .transform_rect_bbox(kurbo::Rect::new(0.0, 0.0, i.width, i.height)),
        Item::Text(t) => t
            .outlines
            .iter()
            .map(shape_bounds)
            .reduce(|a, b| a.union(b))
            .unwrap_or(kurbo::Rect::ZERO),
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SceneOptions {
    /// Saída para impressão/exportação: omite camadas e objetos não imprimíveis (GD-CAM-01).
    pub print: bool,
}

/// Cache de imagens decodificadas por documento.
#[derive(Default)]
pub struct ImageCache {
    map: Mutex<HashMap<String, Option<Arc<DecodedImage>>>>,
}

impl ImageCache {
    pub fn get(&self, doc: &Document, src: &str) -> Option<Arc<DecodedImage>> {
        let mut map = self.map.lock().unwrap();
        map.entry(src.to_string())
            .or_insert_with(|| {
                doc.images
                    .get(src)
                    .and_then(|bytes| decode_image(bytes.clone()))
                    .map(Arc::new)
            })
            .clone()
    }
}

pub fn decode_image(bytes: Arc<Vec<u8>>) -> Option<DecodedImage> {
    let format = image::guess_format(&bytes).ok();
    let img = image::load_from_memory(&bytes).ok()?.to_rgba8();
    let original = match format {
        Some(image::ImageFormat::Png) => Some((bytes.clone(), "image/png")),
        Some(image::ImageFormat::Jpeg) => Some((bytes.clone(), "image/jpeg")),
        _ => None,
    };
    Some(DecodedImage {
        width: img.width(),
        height: img.height(),
        rgba: img.into_raw(),
        original,
    })
}

pub fn build_scene(doc: &Document, page: &Page, opts: SceneOptions, images: &ImageCache) -> Scene {
    let mut b = Builder {
        doc,
        opts,
        images,
        items: Vec::new(),
        missing_fonts: Vec::new(),
        wireframe: false,
        guide_layer: false,
        blend: crate::model::BlendMode::Normal,
        blends: std::collections::HashMap::new(),
        overprints: std::collections::HashMap::new(),
    };
    if !page.hide_master {
        for layer in &doc.data.master_page.layers {
            b.layer(layer);
        }
    }
    for layer in &page.layers {
        b.layer(layer);
    }
    let mut missing = b.missing_fonts;
    missing.sort();
    missing.dedup();
    Scene {
        width: page.width,
        height: page.height,
        background: page.background.clone(),
        bleed: page.bleed.unwrap_or(doc.data.settings.bleed).max(0.0),
        items: b.items,
        missing_fonts: missing,
        blends: b.blends,
        overprints: b.overprints,
        item_bounds: OnceLock::new(),
    }
}

struct Builder<'a> {
    doc: &'a Document,
    opts: SceneOptions,
    images: &'a ImageCache,
    items: Vec<Item>,
    missing_fonts: Vec<String>,
    /// Camada atual em modo esboço (só na tela; a impressão sai normal).
    wireframe: bool,
    /// Camada de guias (GD-GUI-03): o esboço sai tracejado na cor das guias.
    guide_layer: bool,
    /// Modo de mesclagem herdado do grupo.
    blend: crate::model::BlendMode,
    blends: std::collections::HashMap<String, crate::model::BlendMode>,
    overprints: std::collections::HashMap<String, (bool, bool)>,
}

impl Builder<'_> {
    /// Tinta do preenchimento; padrões precisam do documento (ladrilho e
    /// imagens).
    fn fill_paint(&self, f: &Fill, m: Affine, alpha: f64) -> Option<Paint> {
        if let Fill::Pattern(p) = f {
            let tile = super::pattern::tile(self.doc, p, self.images)?;
            return Some(Paint::Pattern {
                tile,
                transform: super::pattern::pattern_space(p, m),
                alpha,
            });
        }
        resolve_fill(f, m, alpha)
    }

    fn layer(&mut self, layer: &crate::model::Layer) {
        if !layer.visible || (self.opts.print && (!layer.printable || layer.guides)) {
            return;
        }
        self.wireframe = (layer.wireframe || layer.guides) && !self.opts.print;
        self.guide_layer = layer.guides;
        for o in &layer.objects {
            self.object(o, Affine::IDENTITY, 1.0);
        }
        self.wireframe = false;
        self.guide_layer = false;
    }

    fn object(&mut self, o: &Object, parent: Affine, parent_alpha: f64) {
        use crate::model::BlendMode;
        let saved = self.blend;
        if o.blend != BlendMode::Normal {
            self.blend = o.blend;
        }
        if self.blend != BlendMode::Normal {
            self.blends.insert(o.id.clone(), self.blend);
        }
        let op = (
            matches!(
                o.fill,
                Some(crate::model::Fill::Solid {
                    overprint: true,
                    ..
                })
            ),
            o.stroke.as_ref().is_some_and(|s| s.overprint),
        );
        if op != (false, false) {
            self.overprints.insert(o.id.clone(), op);
        }
        self.object_items(o, parent, parent_alpha);
        self.blend = saved;
    }

    fn object_items(&mut self, o: &Object, parent: Affine, parent_alpha: f64) {
        if !o.visible || (self.opts.print && o.printable == Some(false)) {
            return;
        }
        let m = parent * o.transform.affine();
        let alpha = parent_alpha * o.opacity.unwrap_or(1.0).clamp(0.0, 1.0);
        let local_path = match &o.kind {
            ObjectKind::Path(p) => Some((subpaths_to_bez(&p.subpaths), p.fill_rule)),
            ObjectKind::Rectangle(r) => Some((rect_path(r), FillRule::Nonzero)),
            ObjectKind::Ellipse(e) => Some((ellipse_path(e), FillRule::Nonzero)),
            ObjectKind::Polygon(p) => Some((polygon_path(p), polygon_fill_rule(p))),
            ObjectKind::Spiral(s) => {
                Some((crate::edit::generate::spiral_path(s), FillRule::Nonzero))
            }
            ObjectKind::Basic(b) => Some((super::basic::path(b), FillRule::Nonzero)),
            ObjectKind::BSpline(b) => Some((super::bspline::path(b), FillRule::Nonzero)),
            // Efeito dinâmico (doc 10): o resultado gerado.
            ObjectKind::Group(g) if g.effect.is_some() => {
                if let Some((w, h, rgba, t, wu, hu)) = crate::edit::effects::shadow_image(g, m) {
                    self.items.push(Item::Image(ImageItem {
                        object_id: o.id.clone(),
                        transform: t * Affine::scale_non_uniform(1.0, 1.0),
                        width: wu,
                        height: hu,
                        image: Arc::new(DecodedImage {
                            width: w,
                            height: h,
                            rgba,
                            original: None,
                        }),
                        opacity: alpha,
                    }));
                }
                for c in crate::edit::effects::expand(g) {
                    self.object(&c, m, alpha);
                }
                None
            }
            // Booleana não destrutiva: só o resultado aparece.
            ObjectKind::Group(g) if g.boolean.is_some() => {
                if let Some(mut r) = crate::edit::shapeops::live_object(g, m) {
                    r.id = o.id.clone();
                    self.object_items(&r, Affine::IDENTITY, alpha);
                }
                None
            }
            ObjectKind::Group(g) => {
                for c in &g.children {
                    self.object(c, m, alpha);
                }
                // Cópias da simetria (GD-FER-33).
                for t in crate::edit::symmetry::copies(g.symmetry.as_ref()) {
                    for c in &g.children {
                        self.object(c, m * t, alpha);
                    }
                }
                None
            }
            // Conector (GD-CON-01): a rota entre as pontas resolvidas ao vivo,
            // com o contorno (e as pontas de seta) do objeto.
            ObjectKind::Connector(c) => {
                use crate::edit::connector as conn;
                let doc = self.doc;
                let (a, b) = conn::ends(c, m, &|id| crate::edit::dimension::find_in_doc(doc, id));
                let inv = m.inverse();
                let mut line = Object::new(ObjectKind::Path(crate::model::PathData {
                    subpaths: crate::edit::convert::bez_to_subpaths(&conn::route(
                        c,
                        inv * a,
                        inv * b,
                    )),
                    fill_rule: FillRule::Nonzero,
                    extra: Default::default(),
                }));
                line.id = o.id.clone();
                line.stroke = o.stroke.clone();
                self.object_items(&line, m, alpha);
                None
            }
            // Cota (GD-COT-01..03): vira linhas, pontas e texto comuns; as
            // ligações aos nós são resolvidas aqui, ao vivo.
            ObjectKind::Dimension(d) => {
                use crate::edit::dimension as dim;
                let doc = self.doc;
                let (pts, seg) = dim::resolve(d, m, &|id| dim::find_in_doc(doc, id));
                let value = dim::measure(d, &pts, seg, doc.data.settings.drawing_scale);
                for c in dim::expand(o, d, &pts, &dim::label(d, value)) {
                    self.object_items(&c, m, alpha);
                }
                None
            }
            ObjectKind::Image(img) => {
                if let Some(decoded) = self.images.get(self.doc, &img.src) {
                    self.items.push(Item::Image(ImageItem {
                        object_id: o.id.clone(),
                        transform: m,
                        width: img.width,
                        height: img.height,
                        image: decoded,
                        opacity: alpha,
                    }));
                }
                None
            }
            ObjectKind::Text(t) => {
                let layout = layout_text(t);
                self.missing_fonts
                    .extend(layout.missing_fonts.iter().cloned());
                let mut outlines = Vec::new();
                for line in &layout.lines {
                    for run in &line.runs {
                        let fill = run.fill.as_ref().or(o.fill.as_ref());
                        let paint = match fill {
                            Some(f) => self.fill_paint(f, m, alpha),
                            None => Some(Paint::Solid {
                                color: Color::black(),
                                alpha,
                            }),
                        };
                        let mut path = run.outline.clone();
                        path.apply_affine(m);
                        let mut item = ShapeItem {
                            object_id: o.id.clone(),
                            path,
                            fill: paint,
                            fill_rule: FillRule::Nonzero,
                            stroke: o.stroke.as_ref().map(|s| resolve_stroke(s, m, alpha)),
                        };
                        if self.wireframe {
                            wireframe(&mut item, self.guide_layer);
                        }
                        outlines.push(item);
                    }
                }
                self.items.push(Item::Text(TextItem {
                    object_id: o.id.clone(),
                    transform: m,
                    layout: Arc::new(layout),
                    outlines,
                    alpha,
                }));
                None
            }
            ObjectKind::Unknown { .. } => None,
        };
        if let Some((mut path, fill_rule)) = local_path {
            path.apply_affine(m);
            let mut stroke = o.stroke.as_ref().map(|s| resolve_stroke(s, m, alpha));
            if let Some(st) = &mut stroke
                && st.position != StrokePosition::Center
                && !all_closed(&path)
            {
                st.position = StrokePosition::Center;
            }
            let mut fill = o.fill.as_ref().and_then(|f| self.fill_paint(f, m, alpha));
            // GD-FIL-12: sem preencher caminhos abertos, linha aberta não
            // recebe preenchimento.
            if !self.doc.data.settings.fill_open_paths
                && matches!(&o.kind, ObjectKind::Path(p) if p.subpaths.iter().all(|s| !s.closed))
            {
                fill = None;
            }
            // Caneta caligráfica (GD-OUT-06): o traço vira a área varrida pelo
            // bico elíptico, preenchida com a tinta do contorno.
            // Tracejado alinhado ou fixo nos cantos: o caminho vira os traços.
            if let (Some(s), Some(st)) = (&o.stroke, &stroke)
                && s.dash_corners != "standard"
                && s.widths.is_empty()
                && s.nib_stretch >= 0.999
                && s.start_arrow.is_none()
                && s.end_arrow.is_none()
                && !self.wireframe
                && let Some(d) = &st.dash
            {
                let dashed = super::dash::corner_dashes(&path, d, &s.dash_corners);
                let mut line = st.clone();
                line.dash = None;
                line.position = StrokePosition::Center;
                let fill_item = fill.clone().map(|f| {
                    Item::Shape(ShapeItem {
                        object_id: o.id.clone(),
                        path: path.clone(),
                        fill: Some(f),
                        fill_rule,
                        stroke: None,
                    })
                });
                let stroke_item = Item::Shape(ShapeItem {
                    object_id: o.id.clone(),
                    path: dashed,
                    fill: None,
                    fill_rule: FillRule::Nonzero,
                    stroke: Some(line),
                });
                if st.behind_fill {
                    self.items.push(stroke_item);
                    self.items.extend(fill_item);
                } else {
                    self.items.extend(fill_item);
                    self.items.push(stroke_item);
                }
                return;
            }
            // Largura variável (GD-VAR-01) também vira área preenchida.
            let swept = match (&o.stroke, &stroke) {
                (Some(s), Some(st))
                    if !self.wireframe && !s.widths.is_empty() && st.width > 0.0 =>
                {
                    Some(super::varwidth::outline(&path, st.width, &s.widths))
                }
                (Some(s), Some(st)) if !self.wireframe && s.nib_stretch < 0.999 => {
                    Some(nib_outline(&path, st, s.nib_stretch, s.nib_angle))
                }
                _ => None,
            };
            if let (Some(outline), Some(st)) = (swept, &stroke) {
                if fill.is_some() && !st.behind_fill {
                    self.items.push(Item::Shape(ShapeItem {
                        object_id: o.id.clone(),
                        path: path.clone(),
                        fill: fill.clone(),
                        fill_rule,
                        stroke: None,
                    }));
                }
                self.items.push(Item::Shape(ShapeItem {
                    object_id: o.id.clone(),
                    path: outline,
                    fill: Some(st.paint.clone()),
                    fill_rule: FillRule::Nonzero,
                    stroke: None,
                }));
                if fill.is_some() && st.behind_fill {
                    self.items.push(Item::Shape(ShapeItem {
                        object_id: o.id.clone(),
                        path,
                        fill,
                        fill_rule,
                        stroke: None,
                    }));
                }
                return;
            }
            // Pontas de seta (GD-OUT-05): o traço recua e as pontas entram
            // como formas preenchidas com a tinta do contorno.
            let heads = match (&o.stroke, &stroke) {
                (Some(s), Some(st)) if !self.wireframe => super::arrows::arrows(
                    &path,
                    s.start_arrow.as_ref(),
                    s.end_arrow.as_ref(),
                    st.width,
                ),
                _ => None,
            };
            if let (Some((heads, trimmed)), Some(st)) = (heads, stroke.clone()) {
                if fill.is_some() {
                    self.items.push(Item::Shape(ShapeItem {
                        object_id: o.id.clone(),
                        path: path.clone(),
                        fill: fill.clone(),
                        fill_rule,
                        stroke: None,
                    }));
                }
                self.items.push(Item::Shape(ShapeItem {
                    object_id: o.id.clone(),
                    path: trimmed,
                    fill: None,
                    fill_rule,
                    stroke: Some(st.clone()),
                }));
                for h in heads {
                    self.items.push(Item::Shape(ShapeItem {
                        object_id: o.id.clone(),
                        path: h,
                        fill: Some(st.paint.clone()),
                        fill_rule: FillRule::Nonzero,
                        stroke: None,
                    }));
                }
                return;
            }
            let mut item = ShapeItem {
                object_id: o.id.clone(),
                path,
                fill,
                fill_rule,
                stroke,
            };
            if self.wireframe {
                wireframe(&mut item, self.guide_layer);
            }
            self.items.push(Item::Shape(item));
        }
    }
}

/// Área varrida por um bico elíptico de largura `width` (eixo maior),
/// achatado por `stretch` e girado `angle` graus, ao longo de `path`:
/// traça com bico redondo no espaço em que o bico é um círculo e volta.
pub fn nib_outline(path: &BezPath, st: &StrokeStyle, stretch: f64, angle: f64) -> BezPath {
    let w = st.width.max(crate::model::paint::HAIRLINE_UM);
    let a = Affine::rotate(angle.to_radians())
        * Affine::scale_non_uniform(1.0, stretch.clamp(0.01, 1.0));
    let mut local = path.clone();
    local.apply_affine(a.inverse());
    let mut ks = kurbo::Stroke::new(w)
        .with_caps(kurbo::Cap::Round)
        .with_join(kurbo::Join::Round);
    if let Some(d) = &st.dash {
        ks = ks.with_dashes(st.dash_offset, d.iter().copied());
    }
    let mut out = kurbo::stroke(
        local,
        &ks,
        &kurbo::StrokeOpts::default(),
        (w / 1000.0).max(0.01),
    );
    out.apply_affine(a);
    out
}

/// Modo esboço (GD-CAM-01): só o traço fino, sem preenchimento.
/// Cor das guias de objeto na tela (GD-GUI-03), igual à das guias.
pub const GUIDE_COLOR: [f64; 3] = [0.024, 0.714, 0.831];

fn wireframe(s: &mut ShapeItem, guide: bool) {
    s.fill = None;
    s.stroke = Some(StrokeStyle {
        width: 0.0,
        paint: Paint::Solid {
            color: if guide {
                Color::rgb(GUIDE_COLOR[0], GUIDE_COLOR[1], GUIDE_COLOR[2])
            } else {
                Color::black()
            },
            alpha: 1.0,
        },
        // Tracejado de 2 mm nas guias.
        dash: guide.then(|| vec![2_000.0, 1_500.0]),
        dash_offset: 0.0,
        cap: "butt".into(),
        join: "miter".into(),
        miter_limit: 4.0,
        behind_fill: false,
        position: StrokePosition::Center,
    });
}

pub fn resolve_fill(f: &Fill, m: Affine, alpha: f64) -> Option<Paint> {
    match f {
        // Padrões: pelo construtor da cena (`Builder::fill_paint`).
        Fill::None | Fill::Unknown(_) | Fill::Pattern(_) => None,
        Fill::Solid { color, opacity, .. } => Some(Paint::Solid {
            color: color.clone(),
            alpha: alpha * opacity.clamp(0.0, 1.0),
        }),
        Fill::Gradient(g) if g.stops.is_empty() => None,
        Fill::Gradient(g) => Some(Paint::Gradient {
            gradient: super::gradient::effective(g),
            transform: m,
            alpha,
        }),
    }
}

fn resolve_stroke(s: &Stroke, m: Affine, alpha: f64) -> StrokeStyle {
    let scale = if s.scale_with_object {
        Affine::determinant(m).abs().sqrt()
    } else {
        1.0
    };
    StrokeStyle {
        width: s.width * scale,
        paint: Paint::Solid {
            color: s.color.clone(),
            alpha: alpha * s.opacity.clamp(0.0, 1.0),
        },
        dash: s
            .dash
            .as_ref()
            .filter(|d| d.iter().any(|v| *v > 0.0))
            .map(|d| d.iter().map(|v| v * scale).collect()),
        dash_offset: s.dash_offset * scale,
        cap: s.cap.clone(),
        join: s.join.clone(),
        miter_limit: s.miter_limit,
        behind_fill: s.behind_fill,
        position: StrokePosition::parse(&s.position),
    }
}

/// Constante para aproximar quarto de círculo com Bézier cúbica: valor de
/// menor erro radial máximo (≈0,02%), em vez do que acerta só o ponto médio.
const KAPPA: f64 = 0.551_915_024_493_510_6;

/// Contorno do polígono/estrela (GD-PRI-03/04).
/// Vértices do polígono (espaço local), em ciclos fechados: um ciclo no
/// polígono e na estrela (externos nos pares, internos nos ímpares); na
/// estrela complexa, um por linha cruzada. Os nós livres já aplicados.
pub fn polygon_vertices(p: &PolygonData) -> Vec<Vec<Point>> {
    let n = p.sides.clamp(3, 500) as usize;
    let s = p.sharpness.clamp(0.0, 1.0);
    let at = |i: usize, count: usize, k: f64| {
        let a = std::f64::consts::FRAC_PI_2 + i as f64 * std::f64::consts::TAU / count as f64;
        Point::new(p.rx * k * a.cos(), p.ry * k * a.sin())
    };
    let free = |i: usize, q: Point| {
        p.free_nodes
            .iter()
            .find(|f| f.index as usize == i)
            .map_or(q, |f| Point::new(f.at[0], f.at[1]))
    };
    match p.mode.as_str() {
        "star" => {
            // Raio interno: de quase o externo (s = 0) até 5% dele (s = 1).
            let inner = 1.0 - 0.95 * s;
            vec![
                (0..2 * n)
                    .map(|i| free(i, at(i, 2 * n, if i % 2 == 0 { 1.0 } else { inner })))
                    .collect(),
            ]
        }
        "complex" if n >= 5 => {
            // Estrela {n/k}: cada linha liga um vértice ao k-ésimo seguinte.
            let max_k = (n - 1) / 2;
            let k = (2.0 + s * (max_k as f64 - 2.0))
                .round()
                .clamp(2.0, max_k as f64) as usize;
            let g = gcd(n, k);
            (0..g)
                .map(|start| {
                    let mut i = start;
                    let mut ring = vec![free(i, at(i, n, 1.0))];
                    for _ in 0..n / g - 1 {
                        i = (i + k) % n;
                        ring.push(free(i, at(i, n, 1.0)));
                    }
                    ring
                })
                .collect()
        }
        _ => vec![(0..n).map(|i| free(i, at(i, n, 1.0))).collect()],
    }
}

/// O vértice `i` do ciclo é vale (interno) da estrela.
fn is_valley(p: &PolygonData, i: usize) -> bool {
    p.mode == "star" && i % 2 == 1
}

pub fn polygon_path(p: &PolygonData) -> BezPath {
    let mut path = BezPath::new();
    for ring in polygon_vertices(p) {
        let m = ring.len();
        if m < 2 {
            continue;
        }
        if let Some([t, k]) = p.bend {
            // Lados curvos: o mesmo controle em todos (simetria viva).
            path.move_to(ring[0]);
            for i in 0..m {
                let (a, b) = (ring[i], ring[(i + 1) % m]);
                let d = b - a;
                // Sentido anti-horário: a normal para fora é (d.y, −d.x).
                let c = a + d * t + kurbo::Vec2::new(d.y, -d.x) * k;
                path.quad_to(c, b);
            }
            path.close_path();
            continue;
        }
        let radius = |i: usize| {
            if is_valley(p, i) {
                p.corner_inner
            } else {
                p.corner_outer
            }
            .max(0.0)
        };
        if (0..m).all(|i| radius(i) == 0.0) {
            path.move_to(ring[0]);
            for q in &ring[1..] {
                path.line_to(*q);
            }
            path.close_path();
            continue;
        }
        // Cantos arredondados: cada vértice vira um arco entre dois pontos
        // sobre os lados (no máximo metade de cada lado).
        let corner = |i: usize| -> (Point, Point, Point) {
            let (prev, cur, next) = (ring[(i + m - 1) % m], ring[i], ring[(i + 1) % m]);
            let (u, w) = (prev - cur, next - cur);
            let (lu, lw) = (u.hypot(), w.hypot());
            if lu < 1e-9 || lw < 1e-9 {
                return (cur, cur, cur);
            }
            let cos = (u.dot(w) / (lu * lw)).clamp(-1.0, 1.0);
            let half = cos.acos() / 2.0;
            let d = (radius(i) / half.tan().max(1e-9))
                .min(lu / 2.0)
                .min(lw / 2.0);
            (cur + u / lu * d, cur, cur + w / lw * d)
        };
        let (s0, _, _) = corner(0);
        path.move_to(s0);
        for i in 0..m {
            let (a, c, b) = corner(i);
            if i > 0 {
                path.line_to(a);
            }
            if (a - b).hypot() > 1e-9 {
                // Arco aproximado por cúbica (controles a 55% do canto).
                const K: f64 = 0.5523;
                path.curve_to(a + (c - a) * K, b + (c - b) * K, b);
            }
        }
        path.close_path();
    }
    path
}

fn gcd(a: usize, b: usize) -> usize {
    if b == 0 { a } else { gcd(b, a % b) }
}

#[cfg(test)]
mod polygon_tests {
    use super::*;

    fn poly(mode: &str, sides: u32) -> PolygonData {
        PolygonData {
            rx: 100.0,
            ry: 100.0,
            sides,
            mode: mode.into(),
            sharpness: 0.5,
            bend: None,
            corner_outer: 0.0,
            corner_inner: 0.0,
            fill_crossings: false,
            free_nodes: Vec::new(),
            extra: Default::default(),
        }
    }

    #[test]
    fn bend_corners_free_nodes_and_crossings() {
        use kurbo::Shape;
        let base = polygon_path(&poly("polygon", 6)).bounding_box();
        // Curvar para fora aumenta a caixa (os lados de pé saem para os lados).
        let mut b = poly("polygon", 6);
        b.bend = Some([0.5, 0.3]);
        let bb = polygon_path(&b).bounding_box();
        assert!(bb.width() > base.width() + 10.0, "{bb:?} {base:?}");
        // Cantos das pontas encolhem a caixa (as pontas viram arcos).
        let mut c = poly("star", 5);
        let sharp = polygon_path(&c).bounding_box();
        c.corner_outer = 20.0;
        c.corner_inner = 10.0;
        let round = polygon_path(&c).bounding_box();
        assert!(round.height() < sharp.height() - 1.0, "{round:?} {sharp:?}");
        // Nó livre: só ele se move.
        let mut f = poly("star", 5);
        f.free_nodes = vec![crate::model::FreeNode {
            index: 0,
            at: [0.0, 250.0],
        }];
        let v = polygon_vertices(&f);
        assert_eq!(v[0][0], Point::new(0.0, 250.0));
        assert!((v[0][2].to_vec2().hypot() - 100.0).abs() < 1e-6);
        // Estrela complexa: miolo vazado ou preenchido.
        let mut x = poly("complex", 7);
        assert_eq!(polygon_fill_rule(&x), FillRule::Evenodd);
        x.fill_crossings = true;
        assert_eq!(polygon_fill_rule(&x), FillRule::Nonzero);
        // Ida e volta no JSON com os campos novos.
        let o = crate::model::Object::new(crate::model::ObjectKind::Polygon(f));
        assert_eq!(crate::model::Object::from_value(o.to_value()), o);
    }
}

/// Regra de preenchimento do polígono: a estrela complexa usa par-ímpar
/// (o miolo cruzado fica vazado), a não ser com `fill_crossings`.
pub fn polygon_fill_rule(p: &PolygonData) -> FillRule {
    if p.mode == "complex" && !p.fill_crossings {
        FillRule::Evenodd
    } else {
        FillRule::Nonzero
    }
}

pub fn rect_path(r: &RectData) -> BezPath {
    let (w, h) = (r.width, r.height);
    let corners = [
        Point::new(0.0, 0.0),
        Point::new(w, 0.0),
        Point::new(w, h),
        Point::new(0.0, h),
    ];
    let default = Corner {
        kind: "round".into(),
        radius: 0.0,
    };
    let max_r = w.abs().min(h.abs()) / 2.0;
    let mut path = BezPath::new();
    for i in 0..4 {
        let c = corners[i];
        let prev = corners[(i + 3) % 4];
        let next = corners[(i + 1) % 4];
        let corner = r
            .corners
            .as_ref()
            .and_then(|cs| cs.get(i))
            .unwrap_or(&default);
        let rad = corner.radius.clamp(0.0, max_r);
        let a = unit(prev - c);
        let b = unit(next - c);
        let p1 = c + a * rad;
        let p2 = c + b * rad;
        if i == 0 {
            path.move_to(p1);
        } else {
            path.line_to(p1);
        }
        if rad > 0.0 {
            match corner.kind.as_str() {
                "chamfer" => path.line_to(p2),
                "scallop" => path.curve_to(p1 + b * (KAPPA * rad), p2 + a * (KAPPA * rad), p2),
                _ => path.curve_to(p1 - a * (KAPPA * rad), p2 - b * (KAPPA * rad), p2),
            }
        }
    }
    path.close_path();
    path
}

fn unit(v: Vec2) -> Vec2 {
    let l = v.hypot();
    if l > 0.0 { v / l } else { Vec2::ZERO }
}

pub fn ellipse_path(e: &EllipseData) -> BezPath {
    let tol = 0.1;
    if e.mode == "ellipse" {
        // Quatro arcos cúbicos (nós nos quadrantes), iguais à conversão em curvas.
        let (rx, ry) = (e.rx, e.ry);
        let (kx, ky) = (KAPPA * rx, KAPPA * ry);
        let mut p = BezPath::new();
        p.move_to((rx, 0.0));
        p.curve_to((rx, ky), (kx, ry), (0.0, ry));
        p.curve_to((-kx, ry), (-rx, ky), (-rx, 0.0));
        p.curve_to((-rx, -ky), (-kx, -ry), (0.0, -ry));
        p.curve_to((kx, -ry), (rx, -ky), (rx, 0.0));
        p.close_path();
        return p;
    }
    let start = e.start_angle.to_radians();
    let mut sweep = (e.end_angle - e.start_angle).to_radians();
    if sweep <= 0.0 {
        sweep += 2.0 * PI;
    }
    let arc = KArc::new(Point::ORIGIN, (e.rx, e.ry), start, sweep, 0.0);
    let mut path = BezPath::new();
    let mut first = true;
    for el in arc.path_elements(tol) {
        match el {
            PathEl::MoveTo(p) if first => {
                path.move_to(p);
                first = false;
            }
            PathEl::MoveTo(_) => {}
            other => path.push(other),
        }
    }
    if e.mode == "pie" {
        path.line_to(Point::ORIGIN);
        path.close_path();
    }
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounded_rect_area_is_less_than_plain() {
        let mut r = RectData {
            width: 100.0,
            height: 50.0,
            corners: None,
            extra: Default::default(),
        };
        let plain = rect_path(&r).area().abs();
        assert!((plain - 5000.0).abs() < 1e-6);
        r.corners = Some(vec![
            Corner {
                kind: "round".into(),
                radius: 10.0
            };
            4
        ]);
        let rounded = rect_path(&r).area().abs();
        // 4 cantos removem (r² − πr²/4) cada.
        let expected = 5000.0 - 4.0 * (100.0 - PI * 100.0 / 4.0);
        assert!((rounded - expected).abs() < 0.5, "{rounded} vs {expected}");
    }

    #[test]
    fn pie_is_closed_through_center() {
        let e = EllipseData {
            rx: 10.0,
            ry: 10.0,
            mode: "pie".into(),
            start_angle: 0.0,
            end_angle: 90.0,
            extra: Default::default(),
        };
        let a = ellipse_path(&e).area().abs();
        assert!((a - PI * 100.0 / 4.0).abs() < 0.05);
    }

    /// GD-OUT-06: bico achatado a 0°: traço horizontal fica fino, vertical
    /// fica com a largura toda.
    #[test]
    fn gd_out_06_calligraphic_nib() {
        use kurbo::Shape;
        let st = StrokeStyle {
            width: 1000.0,
            paint: Paint::Solid {
                color: Color::black(),
                alpha: 1.0,
            },
            dash: None,
            dash_offset: 0.0,
            cap: "butt".into(),
            join: "miter".into(),
            miter_limit: 4.0,
            behind_fill: false,
            position: StrokePosition::Center,
        };
        let line = |a: (f64, f64), b: (f64, f64)| {
            let mut p = BezPath::new();
            p.move_to(a);
            p.line_to(b);
            p
        };
        let h = nib_outline(&line((0.0, 0.0), (10_000.0, 0.0)), &st, 0.2, 0.0).bounding_box();
        let v = nib_outline(&line((0.0, 0.0), (0.0, 10_000.0)), &st, 0.2, 0.0).bounding_box();
        assert!((h.height() - 200.0).abs() < 5.0, "{h:?}");
        assert!((v.width() - 1000.0).abs() < 5.0, "{v:?}");
    }
}
