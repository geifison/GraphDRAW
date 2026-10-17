//! Prévia de separações (GD-SEP-01): cada chapa (C, M, Y, K e cada cor
//! especial) em tons de cinza pela quantidade de tinta, com o vazamento e a
//! sobreimpressão como na impressão; a composta junta as chapas e mostra a
//! sobreimpressão simulada.
//!
//! Objeto sem sobreimpressão vaza: pinta a chapa com a tinta dele, mesmo
//! zero (branco), e tapa o que está embaixo. Com sobreimpressão, onde a
//! tinta dele na chapa é zero ele não pinta, e o de baixo aparece.

use std::collections::HashMap;
use std::sync::Arc;

use kurbo::Affine;
use tiny_skia as sk;

use super::pattern::PatternTile;
use super::scene::{DecodedImage, ImageItem, Item, Paint, Scene, ShapeItem};
use crate::model::{Color, Document};

/// Chapa de impressão.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Plate {
    Cyan,
    Magenta,
    Yellow,
    Black,
    Spot(String),
}

impl Plate {
    pub const PROCESS: [Plate; 4] = [Plate::Cyan, Plate::Magenta, Plate::Yellow, Plate::Black];

    fn index(&self) -> Option<usize> {
        match self {
            Plate::Cyan => Some(0),
            Plate::Magenta => Some(1),
            Plate::Yellow => Some(2),
            Plate::Black => Some(3),
            Plate::Spot(_) => None,
        }
    }
}

/// Chapas do documento: as quatro de processo e as cores especiais usadas.
pub fn plates(doc: &Document) -> Vec<Plate> {
    let mut spots: Vec<String> = Vec::new();
    let mut visit = |c: &Color| {
        if c.model == "spot"
            && let Some(n) = &c.name
            && !spots.contains(n)
        {
            spots.push(n.clone());
        }
    };
    for page in &doc.data.pages {
        for layer in &page.layers {
            for o in &layer.objects {
                o.walk(&mut |o| {
                    if let Some(crate::model::Fill::Solid { color, .. }) = &o.fill {
                        visit(color);
                    }
                    if let Some(crate::model::Fill::Gradient(g)) = &o.fill {
                        g.stops.iter().for_each(|s| visit(&s.color));
                    }
                    if let Some(s) = &o.stroke {
                        visit(&s.color);
                    }
                });
            }
        }
    }
    let mut out = Plate::PROCESS.to_vec();
    out.extend(spots.into_iter().map(Plate::Spot));
    out
}

/// Tinta da cor na chapa (0–1).
pub fn ink(c: &Color, plate: &Plate) -> f64 {
    let tint = c.values.first().copied().unwrap_or(1.0).clamp(0.0, 1.0);
    match c.model.as_str() {
        "registration" => tint,
        "spot" => match plate {
            Plate::Spot(n) if c.name.as_deref() == Some(n) => tint,
            _ => 0.0,
        },
        _ => plate
            .index()
            .map_or(0.0, |i| crate::preflight::cmyk_of(c)[i]),
    }
}

fn gray(t: f64) -> Color {
    let v = 1.0 - t.clamp(0.0, 1.0);
    Color::rgb(v, v, v)
}

/// Tinta RGB da tela (padrões, imagens) na chapa, por fórmula.
fn rgb_ink(r: f64, g: f64, b: f64, plate: &Plate) -> f64 {
    let Some(i) = plate.index() else {
        return 0.0;
    };
    let k = 1.0 - r.max(g).max(b);
    if i == 3 {
        return k;
    }
    if k >= 1.0 - 1e-9 {
        return 0.0;
    }
    let v = [r, g, b][i];
    (1.0 - v - k) / (1.0 - k)
}

fn plate_rgba(rgba: &[u8], premultiplied: bool, plate: &Plate) -> Vec<u8> {
    let mut out = rgba.to_vec();
    for px in out.as_chunks_mut::<4>().0 {
        let a = px[3] as f64 / 255.0;
        let un = |v: u8| {
            if premultiplied && a > 0.0 {
                (v as f64 / 255.0 / a).min(1.0)
            } else {
                v as f64 / 255.0
            }
        };
        let t = rgb_ink(un(px[0]), un(px[1]), un(px[2]), plate);
        let g = 1.0 - t;
        let g8 = if premultiplied {
            (g * a * 255.0).round()
        } else {
            (g * 255.0).round()
        } as u8;
        px[0] = g8;
        px[1] = g8;
        px[2] = g8;
    }
    out
}

struct Mapper<'a> {
    plate: &'a Plate,
    tiles: HashMap<usize, Arc<PatternTile>>,
    images: HashMap<usize, Arc<DecodedImage>>,
}

impl Mapper<'_> {
    fn paint(&mut self, p: &Paint, overprint: bool) -> Option<Paint> {
        match p {
            Paint::Solid { color, alpha } => {
                let t = ink(color, self.plate);
                (!(overprint && t <= 1e-6)).then(|| Paint::Solid {
                    color: gray(t),
                    alpha: *alpha,
                })
            }
            Paint::Gradient {
                gradient,
                transform,
                alpha,
            } => {
                let mut g = gradient.clone();
                let mut any = false;
                for s in &mut g.stops {
                    let t = ink(&s.color, self.plate);
                    any |= t > 1e-6;
                    s.color = gray(t);
                }
                (any || !overprint).then_some(Paint::Gradient {
                    gradient: g,
                    transform: *transform,
                    alpha: *alpha,
                })
            }
            Paint::Pattern {
                tile,
                transform,
                alpha,
            } => {
                let key = Arc::as_ptr(tile) as usize;
                let plate = self.plate;
                let t = self
                    .tiles
                    .entry(key)
                    .or_insert_with(|| {
                        Arc::new(PatternTile {
                            rgba: plate_rgba(&tile.rgba, true, plate),
                            width: tile.width,
                            height: tile.height,
                            size: tile.size,
                        })
                    })
                    .clone();
                Some(Paint::Pattern {
                    tile: t,
                    transform: *transform,
                    alpha: *alpha,
                })
            }
        }
    }

    fn shape(&mut self, s: &ShapeItem, op: (bool, bool)) -> Option<ShapeItem> {
        let fill = s.fill.as_ref().and_then(|f| self.paint(f, op.0));
        let stroke = s.stroke.as_ref().and_then(|st| {
            let p = self.paint(&st.paint, op.1)?;
            let mut st = st.clone();
            st.paint = p;
            Some(st)
        });
        (fill.is_some() || stroke.is_some()).then(|| ShapeItem {
            object_id: s.object_id.clone(),
            path: s.path.clone(),
            fill,
            fill_rule: s.fill_rule,
            stroke,
        })
    }

    fn image(&mut self, i: &ImageItem) -> ImageItem {
        let key = Arc::as_ptr(&i.image) as usize;
        let plate = self.plate;
        let img = self
            .images
            .entry(key)
            .or_insert_with(|| {
                Arc::new(DecodedImage {
                    width: i.image.width,
                    height: i.image.height,
                    rgba: plate_rgba(&i.image.rgba, false, plate),
                    original: None,
                })
            })
            .clone();
        ImageItem {
            image: img,
            ..i.clone()
        }
    }
}

/// A cena vista numa chapa: cada tinta vira o cinza da quantidade dela.
pub fn plate_scene(scene: &Scene, plate: &Plate) -> Scene {
    let mut m = Mapper {
        plate,
        tiles: HashMap::new(),
        images: HashMap::new(),
    };
    let mut items = Vec::with_capacity(scene.items.len());
    for it in &scene.items {
        let op = scene.overprint_of(it);
        match it {
            Item::Shape(s) => {
                if let Some(s) = m.shape(s, op) {
                    items.push(Item::Shape(s));
                }
            }
            Item::Image(i) => items.push(Item::Image(m.image(i))),
            Item::Text(t) => {
                let mut t = t.clone();
                t.outlines = t.outlines.iter().filter_map(|s| m.shape(s, op)).collect();
                items.push(Item::Text(t));
            }
        }
    }
    Scene {
        width: scene.width,
        height: scene.height,
        // O papel não leva tinta; fundo da página em cor vira a tinta dele.
        background: scene.background.as_ref().map(|c| gray(ink(c, plate))),
        bleed: scene.bleed,
        items,
        missing_fonts: Vec::new(),
        blends: scene.blends.clone(),
        overprints: scene.overprints.clone(),
        item_bounds: Default::default(),
    }
}

/// Visão da prévia de separações.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SepView {
    /// Chapas juntas (sobreimpressão simulada).
    Composite,
    Plate(Plate),
}

/// Renderiza a visão: uma chapa em cinza, ou a composta.
pub fn render_view(
    doc: &Document,
    scene: &Scene,
    view: &SepView,
    affine: Affine,
    w: u32,
    h: u32,
    opts: &super::raster::RasterOptions,
) -> Option<sk::Pixmap> {
    match view {
        SepView::Plate(p) => super::raster::render(&plate_scene(scene, p), affine, w, h, opts),
        SepView::Composite => {
            let all = plates(doc);
            let layers: Vec<(Plate, sk::Pixmap)> = all
                .iter()
                .filter_map(|p| {
                    Some((
                        p.clone(),
                        super::raster::render(&plate_scene(scene, p), affine, w, h, opts)?,
                    ))
                })
                .collect();
            let mut out = sk::Pixmap::new(w, h)?;
            let n = (w * h) as usize;
            let mut rgb = vec![[1.0f64; 3]; n];
            let mut alpha = vec![0u8; n];
            for (plate, pm) in &layers {
                for (i, px) in pm.pixels().iter().enumerate() {
                    let a = px.alpha();
                    alpha[i] = alpha[i].max(a);
                    if a == 0 {
                        continue;
                    }
                    let c = px.demultiply();
                    let t = 1.0 - c.red() as f64 / 255.0;
                    let t = t * a as f64 / 255.0;
                    match plate {
                        Plate::Spot(name) => {
                            // Cor de prévia da especial (a alternativa), multiplicada.
                            let alt = spot_preview(doc, name);
                            for k in 0..3 {
                                rgb[i][k] *= 1.0 - t * (1.0 - alt[k]);
                            }
                        }
                        p => {
                            let k = p.index().unwrap();
                            if k == 3 {
                                for v in &mut rgb[i] {
                                    *v *= 1.0 - t;
                                }
                            } else {
                                rgb[i][k] *= 1.0 - t;
                            }
                        }
                    }
                }
            }
            for (i, px) in out.pixels_mut().iter_mut().enumerate() {
                let a = alpha[i];
                let c = |v: f64| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
                *px = sk::ColorU8::from_rgba(c(rgb[i][0]), c(rgb[i][1]), c(rgb[i][2]), a)
                    .premultiply();
            }
            Some(out)
        }
    }
}

/// sRGB de prévia da cor especial `name` (a alternativa), ou um cinza médio.
fn spot_preview(doc: &Document, name: &str) -> [f64; 3] {
    let mut found = None;
    for page in &doc.data.pages {
        for layer in &page.layers {
            for o in &layer.objects {
                o.walk(&mut |o| {
                    let mut look = |c: &Color| {
                        if found.is_none() && c.model == "spot" && c.name.as_deref() == Some(name) {
                            found = Some(
                                c.alternate
                                    .as_ref()
                                    .map_or([0.5, 0.5, 0.5], |a| a.to_srgb()),
                            );
                        }
                    };
                    if let Some(crate::model::Fill::Solid { color, .. }) = &o.fill {
                        look(color);
                    }
                    if let Some(s) = &o.stroke {
                        look(&s.color);
                    }
                });
            }
        }
    }
    found.unwrap_or([0.5, 0.5, 0.5])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Fill, Object, ObjectKind, Page, RectData, Transform};
    use crate::render::{ImageCache, SceneOptions, build_scene};

    fn rect(x: f64, c: Color, overprint: bool) -> Object {
        let mut o = Object::new(ObjectKind::Rectangle(RectData {
            width: 40.0,
            height: 40.0,
            corners: None,
            extra: Default::default(),
        }));
        o.transform = Transform::translate(x, 0.0);
        o.fill = Some(Fill::Solid {
            color: c,
            opacity: 1.0,
            overprint,
            extra: Default::default(),
        });
        o
    }

    fn red_at(pm: &sk::Pixmap, x: u32) -> u8 {
        pm.pixel(x, 20).unwrap().demultiply().red()
    }

    /// GD-SEP-01: fundo ciano e um preto por cima: sem sobreimpressão o
    /// preto vaza o ciano; com sobreimpressão, o ciano continua embaixo.
    #[test]
    fn gd_sep_01_knockout_and_overprint() {
        let mut d = Document::new();
        let mut p = Page::new(100.0, 40.0);
        p.layers[0].objects = vec![
            rect(0.0, Color::cmyk(1.0, 0.0, 0.0, 0.0), false),
            {
                let mut a = rect(0.0, Color::cmyk(0.0, 0.0, 0.0, 1.0), false);
                if let ObjectKind::Rectangle(r) = &mut a.kind {
                    r.width = 20.0;
                }
                a
            },
            rect(50.0, Color::cmyk(1.0, 0.0, 0.0, 0.0), false),
            {
                let mut b = rect(50.0, Color::cmyk(0.0, 0.0, 0.0, 1.0), true);
                if let ObjectKind::Rectangle(r) = &mut b.kind {
                    r.width = 20.0;
                }
                b
            },
        ];
        d.data.pages.push(p);
        let scene = build_scene(
            &d,
            &d.data.pages[0],
            SceneOptions { print: true },
            &ImageCache::default(),
        );
        let view = Affine::new([1.0, 0.0, 0.0, -1.0, 0.0, 40.0]);
        let opts = Default::default();
        let cyan = render_view(
            &d,
            &scene,
            &SepView::Plate(Plate::Cyan),
            view,
            100,
            40,
            &opts,
        )
        .unwrap();
        assert!(red_at(&cyan, 30) < 10, "ciano cheio ao lado do preto");
        assert!(
            red_at(&cyan, 10) > 245,
            "preto sem sobreimpressão vaza o ciano"
        );
        assert!(
            red_at(&cyan, 60) < 10,
            "preto com sobreimpressão: ciano embaixo"
        );
        let black = render_view(
            &d,
            &scene,
            &SepView::Plate(Plate::Black),
            view,
            100,
            40,
            &opts,
        )
        .unwrap();
        assert!(red_at(&black, 10) < 10 && red_at(&black, 30) > 245);
        let comp = render_view(&d, &scene, &SepView::Composite, view, 100, 40, &opts).unwrap();
        let px = comp.pixel(30, 20).unwrap().demultiply();
        assert!(
            px.red() < 30 && px.green() > 200 && px.blue() > 200,
            "ciano na composta"
        );
        assert_eq!(plates(&d).len(), 4);
    }

    /// Cor especial vira chapa própria; nas de processo, zero.
    #[test]
    fn spot_has_its_own_plate() {
        let mut spot = Color::new("spot", vec![0.5]);
        spot.name = Some("Ouro".into());
        assert_eq!(ink(&spot, &Plate::Spot("Ouro".into())), 0.5);
        assert_eq!(ink(&spot, &Plate::Cyan), 0.0);
        let mut d = Document::new();
        let mut p = Page::new(10.0, 10.0);
        p.layers[0].objects = vec![rect(0.0, spot, false)];
        d.data.pages.push(p);
        assert!(plates(&d).contains(&Plate::Spot("Ouro".into())));
    }
}
