//! Achata uma página em uma lista de itens de desenho em coordenadas da
//! página (µm, Y para cima). Todos os destinos (tela, PNG, PDF, SVG) partem
//! desta mesma cena, o que mantém a saída consistente entre eles.

use std::collections::HashMap;
use std::f64::consts::PI;
use std::sync::{Arc, Mutex, OnceLock};

use kurbo::{Affine, Arc as KArc, BezPath, Ellipse, PathEl, Point, Shape, Vec2};

use super::text::{TextLayout, layout_text};
use crate::model::geom::subpaths_to_bez;
use crate::model::{
    Color, Corner, Document, EllipseData, Fill, FillRule, Gradient, Object, ObjectKind, Page,
    RectData, Stroke,
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
    pub items: Vec<Item>,
    /// Fontes ausentes substituídas (GD-FNT-02).
    pub missing_fonts: Vec<String>,
    /// Caixas dos itens (coordenadas da página, já incluindo a espessura do
    /// contorno), calculadas uma vez sob demanda para descarte rápido.
    pub item_bounds: OnceLock<Vec<kurbo::Rect>>,
}

impl Scene {
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
        Some(st) => b.inflate(
            st.width * 0.5 * st.miter_limit.max(1.5),
            st.width * 0.5 * st.miter_limit.max(1.5),
        ),
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
        items: b.items,
        missing_fonts: missing,
        item_bounds: OnceLock::new(),
    }
}

struct Builder<'a> {
    doc: &'a Document,
    opts: SceneOptions,
    images: &'a ImageCache,
    items: Vec<Item>,
    missing_fonts: Vec<String>,
}

impl Builder<'_> {
    fn layer(&mut self, layer: &crate::model::Layer) {
        if !layer.visible || (self.opts.print && !layer.printable) {
            return;
        }
        for o in &layer.objects {
            self.object(o, Affine::IDENTITY, 1.0);
        }
    }

    fn object(&mut self, o: &Object, parent: Affine, parent_alpha: f64) {
        if !o.visible || (self.opts.print && o.printable == Some(false)) {
            return;
        }
        let m = parent * o.transform.affine();
        let alpha = parent_alpha * o.opacity.unwrap_or(1.0).clamp(0.0, 1.0);
        let local_path = match &o.kind {
            ObjectKind::Path(p) => Some((subpaths_to_bez(&p.subpaths), p.fill_rule)),
            ObjectKind::Rectangle(r) => Some((rect_path(r), FillRule::Nonzero)),
            ObjectKind::Ellipse(e) => Some((ellipse_path(e), FillRule::Nonzero)),
            ObjectKind::Group(g) => {
                for c in &g.children {
                    self.object(c, m, alpha);
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
                            Some(f) => resolve_fill(f, m, alpha),
                            None => Some(Paint::Solid {
                                color: Color::black(),
                                alpha,
                            }),
                        };
                        let mut path = run.outline.clone();
                        path.apply_affine(m);
                        outlines.push(ShapeItem {
                            object_id: o.id.clone(),
                            path,
                            fill: paint,
                            fill_rule: FillRule::Nonzero,
                            stroke: o.stroke.as_ref().map(|s| resolve_stroke(s, m, alpha)),
                        });
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
            self.items.push(Item::Shape(ShapeItem {
                object_id: o.id.clone(),
                path,
                fill: o.fill.as_ref().and_then(|f| resolve_fill(f, m, alpha)),
                fill_rule,
                stroke: o.stroke.as_ref().map(|s| resolve_stroke(s, m, alpha)),
            }));
        }
    }
}

pub fn resolve_fill(f: &Fill, m: Affine, alpha: f64) -> Option<Paint> {
    match f {
        Fill::None | Fill::Unknown(_) => None,
        Fill::Solid { color, opacity, .. } => Some(Paint::Solid {
            color: color.clone(),
            alpha: alpha * opacity.clamp(0.0, 1.0),
        }),
        Fill::Gradient(g) if g.stops.is_empty() => None,
        Fill::Gradient(g) => Some(Paint::Gradient {
            gradient: g.clone(),
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
    }
}

/// Constante para aproximar quarto de círculo com Bézier cúbica.
const KAPPA: f64 = 0.552_284_749_830_793_4;

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
        return Ellipse::new(Point::ORIGIN, (e.rx, e.ry), 0.0).to_path(tol);
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
}
