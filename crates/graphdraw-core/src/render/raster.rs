//! Rasterização com tiny-skia: usada pela tela do visualizador, pela
//! exportação PNG (GD-EXP / doc 15) e pelas miniaturas do `.gdr`.

use kurbo::{Affine, BezPath, PathEl};
use tiny_skia as sk;

use super::scene::{ImageItem, Item, Paint, Scene, ShapeItem, StrokePosition, StrokeStyle};
use crate::model::{FillRule, Gradient};

#[derive(Debug, Clone, Copy)]
pub struct RasterOptions {
    pub antialias: bool,
    /// Cor de fundo RGBA (não pré-multiplicada); `None` mantém transparência
    /// (exceto o fundo próprio da página, se houver).
    pub background: Option<[u8; 4]>,
}

impl Default for RasterOptions {
    fn default() -> Self {
        RasterOptions {
            antialias: true,
            background: None,
        }
    }
}

/// Renderiza a cena num pixmap `width×height`; `view` leva coordenadas da
/// página (µm, Y para cima) para pixels (Y para baixo).
pub fn render(
    scene: &Scene,
    view: Affine,
    width: u32,
    height: u32,
    opts: &RasterOptions,
) -> Option<sk::Pixmap> {
    let (width, height) = (width.max(1), height.max(1));
    let mut pm = sk::Pixmap::new(width, height)?;
    // Faixas horizontais renderizadas em paralelo (GD-NF-02); cada faixa só
    // desenha os itens cuja caixa a intercepta. A ordem de pintura dentro da
    // faixa é a mesma da cena, então o resultado é idêntico ao sequencial.
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    let bands = threads.min((height / 64).max(1) as usize).max(1);
    let band_h = height.div_ceil(bands as u32);
    let boxes = device_boxes(scene, view);
    let row_bytes = width as usize * 4;
    let chunks: Vec<&mut [u8]> = pm
        .data_mut()
        .chunks_mut(band_h as usize * row_bytes)
        .collect();
    std::thread::scope(|s| {
        for (i, chunk) in chunks.into_iter().enumerate() {
            let boxes = &boxes;
            s.spawn(move || {
                let y0 = i as u32 * band_h;
                let h = (chunk.len() / row_bytes) as u32;
                if let Some(band) = render_band(scene, view, boxes, width, y0, h, opts) {
                    chunk.copy_from_slice(band.data());
                }
            });
        }
    });
    Some(pm)
}

/// Caixas dos itens em pixels do dispositivo (com folga de 2 px para
/// anti-serrilhado e fio de cabelo).
fn device_boxes(scene: &Scene, view: Affine) -> Vec<kurbo::Rect> {
    scene
        .item_bounds()
        .iter()
        .map(|b| view.transform_rect_bbox(*b).inflate(2.0, 2.0))
        .collect()
}

fn render_band(
    scene: &Scene,
    view: Affine,
    boxes: &[kurbo::Rect],
    width: u32,
    y0: u32,
    height: u32,
    opts: &RasterOptions,
) -> Option<sk::Pixmap> {
    let mut pm = sk::Pixmap::new(width, height)?;
    if let Some([r, g, b, a]) = opts.background {
        pm.fill(sk::Color::from_rgba8(r, g, b, a));
    }
    let band_view = Affine::translate((0.0, -(y0 as f64))) * view;
    if let Some(bg) = &scene.background {
        let [r, g, b] = bg.to_srgb();
        let mut page = BezPath::new();
        page.extend(kurbo::Shape::path_elements(&scene.bounds(), 0.1));
        let paint = Paint::Solid {
            color: crate::model::Color::rgb(r, g, b),
            alpha: 1.0,
        };
        fill_path(&mut pm, &page, band_view, &paint, FillRule::Nonzero, opts);
    }
    let (top, bottom) = (y0 as f64, (y0 + height) as f64);
    for (item, b) in scene.items.iter().zip(boxes) {
        if b.y1 < top || b.y0 > bottom || b.x1 < 0.0 || b.x0 > width as f64 {
            continue;
        }
        draw_item(&mut pm, item, band_view, opts);
    }
    Some(pm)
}

fn draw_item(pm: &mut sk::Pixmap, item: &Item, view: Affine, opts: &RasterOptions) {
    match item {
        Item::Shape(s) => draw_shape(pm, s, view, opts),
        Item::Image(i) => draw_image(pm, i, view, opts),
        Item::Text(t) => {
            for s in &t.outlines {
                draw_shape(pm, s, view, opts);
            }
        }
    }
}

pub fn draw_items(pm: &mut sk::Pixmap, items: &[Item], view: Affine, opts: &RasterOptions) {
    for item in items {
        draw_item(pm, item, view, opts);
    }
}

/// Matriz página→pixels para renderizar a página inteira a `dpi`.
pub fn page_view(scene_height_um: f64, dpi: f64) -> Affine {
    let s = dpi / crate::units::UM_PER_INCH;
    Affine::new([s, 0.0, 0.0, -s, 0.0, scene_height_um * s])
}

fn to_sk_path(path: &BezPath, view: Affine) -> Option<sk::Path> {
    let mut pb = sk::PathBuilder::new();
    let p = |pt: kurbo::Point| {
        let q = view * pt;
        (q.x as f32, q.y as f32)
    };
    for el in path.elements() {
        match *el {
            PathEl::MoveTo(a) => {
                let (x, y) = p(a);
                pb.move_to(x, y)
            }
            PathEl::LineTo(a) => {
                let (x, y) = p(a);
                pb.line_to(x, y)
            }
            PathEl::QuadTo(a, b) => {
                let (x1, y1) = p(a);
                let (x, y) = p(b);
                pb.quad_to(x1, y1, x, y)
            }
            PathEl::CurveTo(a, b, c) => {
                let (x1, y1) = p(a);
                let (x2, y2) = p(b);
                let (x, y) = p(c);
                pb.cubic_to(x1, y1, x2, y2, x, y)
            }
            PathEl::ClosePath => pb.close(),
        }
    }
    pb.finish()
}

fn sk_transform(a: Affine) -> sk::Transform {
    let c = a.as_coeffs();
    sk::Transform::from_row(
        c[0] as f32,
        c[1] as f32,
        c[2] as f32,
        c[3] as f32,
        c[4] as f32,
        c[5] as f32,
    )
}

fn sk_color(color: &crate::model::Color, alpha: f64) -> sk::Color {
    let [r, g, b] = color.to_srgb();
    sk::Color::from_rgba(r as f32, g as f32, b as f32, alpha.clamp(0.0, 1.0) as f32)
        .unwrap_or(sk::Color::BLACK)
}

fn gradient_shader(
    g: &Gradient,
    m: Affine,
    alpha: f64,
    view: Affine,
) -> Option<sk::Shader<'static>> {
    let stops: Vec<sk::GradientStop> = g
        .stops
        .iter()
        .map(|s| sk::GradientStop::new(s.offset as f32, sk_color(&s.color, alpha * s.opacity)))
        .collect();
    let mode = match g.spread.as_str() {
        "repeat" => sk::SpreadMode::Repeat,
        "reflect" => sk::SpreadMode::Reflect,
        _ => sk::SpreadMode::Pad,
    };
    let t = sk_transform(view * m);
    match g.kind.as_str() {
        "radial" => {
            let c = g.center.unwrap_or([0.0, 0.0]);
            let p = sk::Point::from_xy(c[0] as f32, c[1] as f32);
            sk::RadialGradient::new(p, 0.0, p, g.radius.unwrap_or(1.0) as f32, stops, mode, t)
        }
        _ => {
            let a = g.start.unwrap_or([0.0, 0.0]);
            let b = g.end.unwrap_or([1.0, 0.0]);
            sk::LinearGradient::new(
                sk::Point::from_xy(a[0] as f32, a[1] as f32),
                sk::Point::from_xy(b[0] as f32, b[1] as f32),
                stops,
                mode,
                t,
            )
        }
    }
}

fn make_paint(paint: &Paint, view: Affine, opts: &RasterOptions) -> Option<sk::Paint<'static>> {
    let mut p = sk::Paint {
        anti_alias: opts.antialias,
        ..Default::default()
    };
    match paint {
        Paint::Solid { color, alpha } => p.set_color(sk_color(color, *alpha)),
        Paint::Gradient {
            gradient,
            transform,
            alpha,
        } => p.shader = gradient_shader(gradient, *transform, *alpha, view)?,
    }
    Some(p)
}

fn fill_path(
    pm: &mut sk::Pixmap,
    path: &BezPath,
    view: Affine,
    paint: &Paint,
    rule: FillRule,
    opts: &RasterOptions,
) {
    let (Some(sp), Some(p)) = (to_sk_path(path, view), make_paint(paint, view, opts)) else {
        return;
    };
    let rule = match rule {
        FillRule::Evenodd => sk::FillRule::EvenOdd,
        FillRule::Nonzero => sk::FillRule::Winding,
    };
    pm.fill_path(&sp, &p, rule, sk::Transform::identity(), None);
}

fn stroke_path(
    pm: &mut sk::Pixmap,
    path: &BezPath,
    rule: FillRule,
    view: Affine,
    s: &StrokeStyle,
    opts: &RasterOptions,
) {
    let (Some(sp), Some(p)) = (to_sk_path(path, view), make_paint(&s.paint, view, opts)) else {
        return;
    };
    let scale = view.determinant().abs().sqrt();
    // Fio de cabelo: 1 pixel do dispositivo (sempre centrado).
    let positioned = s.position != StrokePosition::Center && s.width > 0.0;
    let width = if s.width <= 0.0 {
        1.0
    } else {
        (s.drawn_width() * scale).max(0.05)
    };
    // Interno: recorta pela forma; externo: pelo complemento (GD-OUT-04).
    let mask = if positioned {
        let Some(mut m) = sk::Mask::new(pm.width(), pm.height()) else {
            return;
        };
        let rule = match rule {
            FillRule::Evenodd => sk::FillRule::EvenOdd,
            FillRule::Nonzero => sk::FillRule::Winding,
        };
        m.fill_path(&sp, rule, opts.antialias, sk::Transform::identity());
        if s.position == StrokePosition::Outside {
            m.invert();
        }
        Some(m)
    } else {
        None
    };
    let dash = s.dash.as_ref().and_then(|d| {
        let mut arr: Vec<f32> = d.iter().map(|v| (v * scale) as f32).collect();
        if arr.len() % 2 == 1 {
            arr.extend(arr.clone());
        }
        sk::StrokeDash::new(arr, (s.dash_offset * scale) as f32)
    });
    let stroke = sk::Stroke {
        width: width as f32,
        miter_limit: s.miter_limit as f32,
        line_cap: match s.cap.as_str() {
            "round" => sk::LineCap::Round,
            "square" => sk::LineCap::Square,
            _ => sk::LineCap::Butt,
        },
        line_join: match s.join.as_str() {
            "round" => sk::LineJoin::Round,
            "bevel" => sk::LineJoin::Bevel,
            _ => sk::LineJoin::Miter,
        },
        dash,
    };
    pm.stroke_path(&sp, &p, &stroke, sk::Transform::identity(), mask.as_ref());
}

fn draw_shape(pm: &mut sk::Pixmap, s: &ShapeItem, view: Affine, opts: &RasterOptions) {
    let stroke_first = s.stroke.as_ref().is_some_and(|st| st.behind_fill);
    if stroke_first {
        stroke_path(
            pm,
            &s.path,
            s.fill_rule,
            view,
            s.stroke.as_ref().unwrap(),
            opts,
        );
    }
    if let Some(f) = &s.fill {
        fill_path(pm, &s.path, view, f, s.fill_rule, opts);
    }
    if !stroke_first && let Some(st) = &s.stroke {
        stroke_path(pm, &s.path, s.fill_rule, view, st, opts);
    }
}

fn draw_image(pm: &mut sk::Pixmap, img: &ImageItem, view: Affine, opts: &RasterOptions) {
    let d = &img.image;
    let Some(src) = premultiplied_pixmap(&d.rgba, d.width, d.height) else {
        return;
    };
    // Pixel (u,v) → local (u·w/pw, h − v·h/ph) → página → tela.
    let local = Affine::new([
        img.width / d.width as f64,
        0.0,
        0.0,
        -img.height / d.height as f64,
        0.0,
        img.height,
    ]);
    let shader = sk::Pattern::new(
        src.as_ref(),
        sk::SpreadMode::Pad,
        sk::FilterQuality::Bilinear,
        img.opacity as f32,
        sk_transform(view * img.transform * local),
    );
    let rect = kurbo::Rect::new(0.0, 0.0, img.width, img.height);
    let mut path = BezPath::new();
    path.extend(kurbo::Shape::path_elements(&rect, 0.1));
    path.apply_affine(img.transform);
    let Some(sp) = to_sk_path(&path, view) else {
        return;
    };
    let paint = sk::Paint {
        shader,
        anti_alias: opts.antialias,
        ..Default::default()
    };
    pm.fill_path(
        &sp,
        &paint,
        sk::FillRule::Winding,
        sk::Transform::identity(),
        None,
    );
}

/// Prévia de um preenchimento num retângulo de `w` × `h` px (janela de
/// preenchimento): RGBA sem pré-multiplicar.
pub fn fill_preview(fill: &crate::model::Fill, w: u32, h: u32) -> Option<Vec<u8>> {
    use kurbo::Shape;
    let mut pm = sk::Pixmap::new(w.max(1), h.max(1))?;
    let path = kurbo::Rect::new(0.0, 0.0, w as f64, h as f64).to_path(0.1);
    let item = Item::Shape(ShapeItem {
        object_id: String::new(),
        path,
        fill: super::scene::resolve_fill(fill, Affine::IDENTITY, 1.0),
        fill_rule: FillRule::Nonzero,
        stroke: None,
    });
    draw_items(
        &mut pm,
        &[item],
        Affine::IDENTITY,
        &RasterOptions::default(),
    );
    Some(unpremultiply(&pm))
}

/// Prévia de família de fonte (seletor de fonte, GD-FNT-01): cobertura
/// (alfa 0–255) de `text` desenhado na própria família, com `height` px de
/// altura e no máximo `max_width` px de largura. `None` se a família não
/// estiver instalada.
pub fn font_preview(
    family: &str,
    text: &str,
    height: u32,
    max_width: u32,
) -> Option<(u32, u32, Vec<u8>)> {
    use kurbo::Shape;
    let h = height.max(4) as f64;
    // Corpo para a ascendente ocupar ~80% da altura.
    let size = h * 0.72;
    let (path, ascent) = crate::render::text::family_outline(family, text, size)?;
    if path.elements().is_empty() {
        return None;
    }
    let bb = path.bounding_box();
    let w = ((bb.x1.max(1.0)).ceil() as u32 + 2).min(max_width.max(8));
    // Linha de base um pouco abaixo da ascendente; Y da tela para baixo.
    let base = (ascent.min(h * 0.8)).max(h * 0.6) + 1.0;
    let view = Affine::new([1.0, 0.0, 0.0, -1.0, 1.0, base]);
    let mut pm = sk::Pixmap::new(w, height)?;
    let p = to_sk_path(&path, view)?;
    let mut paint = sk::Paint::default();
    paint.set_color(sk::Color::WHITE);
    paint.anti_alias = true;
    pm.fill_path(
        &p,
        &paint,
        sk::FillRule::Winding,
        sk::Transform::identity(),
        None,
    );
    let alpha = pm.pixels().iter().map(|px| px.alpha()).collect();
    Some((w, height, alpha))
}

pub fn premultiplied_pixmap(rgba: &[u8], w: u32, h: u32) -> Option<sk::Pixmap> {
    let mut data = rgba.to_vec();
    for px in data.as_chunks_mut::<4>().0 {
        let a = px[3] as u16;
        if a < 255 {
            for c in &mut px[..3] {
                *c = ((*c as u16 * a + 127) / 255) as u8;
            }
        }
    }
    sk::Pixmap::from_vec(data, sk::IntSize::from_wh(w, h)?)
}

/// Converte um pixmap pré-multiplicado em RGBA8 comum.
pub fn unpremultiply(pm: &sk::Pixmap) -> Vec<u8> {
    let mut out = Vec::with_capacity(pm.data().len());
    for px in pm.pixels() {
        let c = px.demultiply();
        out.extend_from_slice(&[c.red(), c.green(), c.blue(), c.alpha()]);
    }
    out
}

/// Codifica um pixmap como PNG com fragmento sRGB e resolução física (pHYs).
pub fn encode_png(pm: &sk::Pixmap, dpi: Option<f64>) -> Result<Vec<u8>, crate::Error> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, pm.width(), pm.height());
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.set_source_srgb(png::SrgbRenderingIntent::RelativeColorimetric);
        if let Some(dpi) = dpi {
            let ppm = (dpi / 0.0254).round() as u32;
            enc.set_pixel_dims(Some(png::PixelDimensions {
                xppu: ppm,
                yppu: ppm,
                unit: png::Unit::Meter,
            }));
        }
        let mut w = enc
            .write_header()
            .map_err(|e| crate::Error::Other(e.to_string()))?;
        w.write_image_data(&unpremultiply(pm))
            .map_err(|e| crate::Error::Other(e.to_string()))?;
    }
    Ok(out)
}
