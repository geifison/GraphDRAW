//! Exportação PDF 1.7 (doc 15, exportação básica do M0).
//!
//! - Cores mantêm o modelo original: RGB → DeviceRGB, CMYK → DeviceCMYK,
//!   cinza → DeviceGray, cor especial/registro → Separation (GD-COR-02).
//! - Opacidade via ExtGState; gradientes via shading axial/radial (RGB).
//! - Texto convertido em curvas (texto vivo com fontes incorporadas: M2).
//! - Perfis PDF/X, marcas de corte e sangria (GD-EXP-03): M2.

use std::collections::HashMap;

use kurbo::{Affine, BezPath, PathEl};
use pdf_writer::types::{FunctionShadingType, LineCapStyle, LineJoinStyle};
use pdf_writer::{Content, Filter, Finish, Name, Pdf, Rect, Ref, TextStr};

use super::ExportOptions;
use crate::model::{Color, FillRule, Gradient};
use crate::render::scene::{ImageItem, Item, Paint, Scene, ShapeItem, StrokePosition, StrokeStyle};
use crate::units::UM_PER_INCH;

/// µm → pt.
const K: f64 = 72.0 / UM_PER_INCH;

pub fn write_pdf(scenes: &[Scene], title: Option<&str>, opts: &ExportOptions) -> Vec<u8> {
    let _ = opts;
    let mut pdf = Pdf::new();
    let mut next = 1;
    let mut alloc = || {
        let r = Ref::new(next);
        next += 1;
        r
    };
    let catalog_id = alloc();
    let tree_id = alloc();
    let info_id = alloc();
    let page_ids: Vec<Ref> = scenes.iter().map(|_| alloc()).collect();

    pdf.catalog(catalog_id).pages(tree_id);
    pdf.pages(tree_id)
        .kids(page_ids.iter().copied())
        .count(page_ids.len() as i32);
    {
        let mut info = pdf.document_info(info_id);
        info.producer(TextStr(concat!("GraphDraw ", env!("CARGO_PKG_VERSION"))));
        info.creator(TextStr("GraphDraw"));
        if let Some(t) = title {
            info.title(TextStr(t));
        }
    }

    for (scene, &page_id) in scenes.iter().zip(&page_ids) {
        let mut res = Resources::default();
        let mut c = Content::new();
        c.transform([K as f32, 0.0, 0.0, K as f32, 0.0, 0.0]);
        if let Some(bg) = &scene.background {
            set_color(&mut c, bg, false, &mut res, &mut pdf, &mut alloc);
            c.rect(0.0, 0.0, scene.width as f32, scene.height as f32);
            c.fill_nonzero();
        }
        for item in &scene.items {
            match item {
                Item::Shape(s) => shape(&mut c, s, &mut res, &mut pdf, &mut alloc),
                Item::Image(i) => image(&mut c, i, &mut res, &mut pdf, &mut alloc),
                Item::Text(t) => {
                    for s in &t.outlines {
                        shape(&mut c, s, &mut res, &mut pdf, &mut alloc);
                    }
                }
            }
        }
        let content_id = alloc();
        let data = miniz_oxide::deflate::compress_to_vec_zlib(&c.finish(), 6);
        pdf.stream(content_id, &data).filter(Filter::FlateDecode);

        let mut page = pdf.page(page_id);
        page.parent(tree_id)
            .media_box(Rect::new(
                0.0,
                0.0,
                (scene.width * K) as f32,
                (scene.height * K) as f32,
            ))
            .contents(content_id);
        let mut r = page.resources();
        if !res.gstates.is_empty() {
            let mut d = r.ext_g_states();
            for (name, id) in &res.gstates_list {
                d.pair(Name(name.as_bytes()), *id);
            }
        }
        if !res.shadings.is_empty() {
            let mut d = r.shadings();
            for (name, id) in &res.shadings {
                d.pair(Name(name.as_bytes()), *id);
            }
        }
        if !res.xobjects.is_empty() {
            let mut d = r.x_objects();
            for (name, id) in &res.xobjects {
                d.pair(Name(name.as_bytes()), *id);
            }
        }
        if !res.spaces_list.is_empty() {
            let mut d = r.color_spaces();
            for (name, id) in &res.spaces_list {
                d.pair(Name(name.as_bytes()), *id);
            }
        }
        r.finish();
        page.finish();
    }
    pdf.finish()
}

#[derive(Default)]
struct Resources {
    /// (ca, CA) em milésimos → nome.
    gstates: HashMap<(u32, u32), String>,
    gstates_list: Vec<(String, Ref)>,
    shadings: Vec<(String, Ref)>,
    xobjects: Vec<(String, Ref)>,
    spaces: HashMap<String, String>,
    spaces_list: Vec<(String, Ref)>,
}

fn path_ops(c: &mut Content, path: &BezPath) {
    for el in path.elements() {
        match *el {
            PathEl::MoveTo(p) => {
                c.move_to(p.x as f32, p.y as f32);
            }
            PathEl::LineTo(p) => {
                c.line_to(p.x as f32, p.y as f32);
            }
            PathEl::QuadTo(a, p) => {
                // Eleva quadrática para cúbica.
                let p0 = last_point(c, path, el);
                let c1 = p0 + (a - p0) * (2.0 / 3.0);
                let c2 = p + (a - p) * (2.0 / 3.0);
                c.cubic_to(
                    c1.x as f32,
                    c1.y as f32,
                    c2.x as f32,
                    c2.y as f32,
                    p.x as f32,
                    p.y as f32,
                );
            }
            PathEl::CurveTo(a, b, p) => {
                c.cubic_to(
                    a.x as f32, a.y as f32, b.x as f32, b.y as f32, p.x as f32, p.y as f32,
                );
            }
            PathEl::ClosePath => {
                c.close_path();
            }
        }
    }
}

fn last_point(_c: &Content, path: &BezPath, target: &PathEl) -> kurbo::Point {
    let mut last = kurbo::Point::ORIGIN;
    let mut start = kurbo::Point::ORIGIN;
    for el in path.elements() {
        if std::ptr::eq(el, target) {
            return last;
        }
        match *el {
            PathEl::MoveTo(p) => {
                last = p;
                start = p;
            }
            PathEl::LineTo(p) | PathEl::QuadTo(_, p) | PathEl::CurveTo(_, _, p) => last = p,
            PathEl::ClosePath => last = start,
        }
    }
    last
}

fn gstate(
    c: &mut Content,
    fill_alpha: f64,
    stroke_alpha: f64,
    res: &mut Resources,
    pdf: &mut Pdf,
    alloc: &mut impl FnMut() -> Ref,
) {
    let key = (
        (fill_alpha.clamp(0.0, 1.0) * 1000.0).round() as u32,
        (stroke_alpha.clamp(0.0, 1.0) * 1000.0).round() as u32,
    );
    if key == (1000, 1000) {
        return;
    }
    let name = match res.gstates.get(&key) {
        Some(n) => n.clone(),
        None => {
            let name = format!("GS{}", res.gstates.len());
            let id = alloc();
            pdf.ext_graphics(id)
                .non_stroking_alpha(key.0 as f32 / 1000.0)
                .stroking_alpha(key.1 as f32 / 1000.0);
            res.gstates.insert(key, name.clone());
            res.gstates_list.push((name.clone(), id));
            name
        }
    };
    c.set_parameters(Name(name.as_bytes()));
}

fn set_color(
    c: &mut Content,
    color: &Color,
    stroke: bool,
    res: &mut Resources,
    pdf: &mut Pdf,
    alloc: &mut impl FnMut() -> Ref,
) {
    let v = |i: usize| color.values.get(i).copied().unwrap_or(0.0).clamp(0.0, 1.0) as f32;
    match color.model.as_str() {
        "cmyk" => {
            if stroke {
                c.set_stroke_cmyk(v(0), v(1), v(2), v(3))
            } else {
                c.set_fill_cmyk(v(0), v(1), v(2), v(3))
            };
        }
        "gray" => {
            if stroke {
                c.set_stroke_gray(v(0))
            } else {
                c.set_fill_gray(v(0))
            };
        }
        "spot" | "registration" => {
            let ink = if color.model == "registration" {
                "All".to_string()
            } else {
                color.name.clone().unwrap_or_else(|| "Spot".into())
            };
            let alt = if color.model == "registration" {
                Color::cmyk(1.0, 1.0, 1.0, 1.0)
            } else {
                color
                    .alternate
                    .as_deref()
                    .cloned()
                    .unwrap_or_else(|| Color::gray(0.0))
            };
            let key = format!("{ink}|{:?}|{:?}", alt.model, alt.values);
            let name = match res.spaces.get(&key) {
                Some(n) => n.clone(),
                None => {
                    let name = format!("CS{}", res.spaces.len());
                    let id = alloc();
                    let mut sep = pdf.color_space(id).separation(Name(ink.as_bytes()));
                    let (c1, is_cmyk): (Vec<f32>, bool) = match alt.model.as_str() {
                        "cmyk" => (alt.values.iter().map(|x| *x as f32).collect(), true),
                        _ => {
                            let [r, g, b] = alt.to_srgb();
                            (vec![r as f32, g as f32, b as f32], false)
                        }
                    };
                    let c0: Vec<f32> = if is_cmyk { vec![0.0; 4] } else { vec![1.0; 3] };
                    if is_cmyk {
                        sep.alternate_color_space().device_cmyk();
                    } else {
                        sep.alternate_color_space().device_rgb();
                    }
                    sep.tint_exponential()
                        .domain([0.0, 1.0])
                        .c0(c0)
                        .c1(c1)
                        .n(1.0);
                    res.spaces.insert(key, name.clone());
                    res.spaces_list.push((name.clone(), id));
                    name
                }
            };
            let tint = color.values.first().copied().unwrap_or(1.0).clamp(0.0, 1.0) as f32;
            if stroke {
                c.set_stroke_color_space(Name(name.as_bytes()));
                c.set_stroke_color([tint]);
            } else {
                c.set_fill_color_space(Name(name.as_bytes()));
                c.set_fill_color([tint]);
            }
        }
        "rgb" => {
            if stroke {
                c.set_stroke_rgb(v(0), v(1), v(2))
            } else {
                c.set_fill_rgb(v(0), v(1), v(2))
            };
        }
        _ => {
            let [r, g, b] = color.to_srgb();
            if stroke {
                c.set_stroke_rgb(r as f32, g as f32, b as f32)
            } else {
                c.set_fill_rgb(r as f32, g as f32, b as f32)
            };
        }
    }
}

fn shading(g: &Gradient, pdf: &mut Pdf, alloc: &mut impl FnMut() -> Ref) -> Ref {
    let mut stops: Vec<(f64, [f32; 3])> = g
        .stops
        .iter()
        .map(|s| {
            let [r, gg, b] = s.color.to_srgb();
            (s.offset.clamp(0.0, 1.0), [r as f32, gg as f32, b as f32])
        })
        .collect();
    stops.sort_by(|a, b| a.0.total_cmp(&b.0));
    if stops[0].0 > 0.0 {
        stops.insert(0, (0.0, stops[0].1));
    }
    if stops.last().unwrap().0 < 1.0 {
        stops.push((1.0, stops.last().unwrap().1));
    }
    if stops.len() == 1 {
        stops.push((1.0, stops[0].1));
    }
    let func_id = if stops.len() == 2 {
        let id = alloc();
        pdf.exponential_function(id)
            .domain([0.0, 1.0])
            .c0(stops[0].1)
            .c1(stops[1].1)
            .n(1.0);
        id
    } else {
        let parts: Vec<Ref> = stops
            .windows(2)
            .map(|w| {
                let id = alloc();
                pdf.exponential_function(id)
                    .domain([0.0, 1.0])
                    .c0(w[0].1)
                    .c1(w[1].1)
                    .n(1.0);
                id
            })
            .collect();
        let id = alloc();
        let bounds: Vec<f32> = stops[1..stops.len() - 1]
            .iter()
            .map(|s| s.0 as f32)
            .collect();
        let encode: Vec<f32> = parts.iter().flat_map(|_| [0.0f32, 1.0]).collect();
        pdf.stitching_function(id)
            .domain([0.0, 1.0])
            .functions(parts)
            .bounds(bounds)
            .encode(encode);
        id
    };
    let id = alloc();
    let mut sh = pdf.function_shading(id);
    if g.kind == "radial" {
        let c = g.center.unwrap_or([0.0, 0.0]);
        let r = g.radius.unwrap_or(1.0) as f32;
        sh.shading_type(FunctionShadingType::Radial).coords([
            c[0] as f32,
            c[1] as f32,
            0.0,
            c[0] as f32,
            c[1] as f32,
            r,
        ]);
    } else {
        let a = g.start.unwrap_or([0.0, 0.0]);
        let b = g.end.unwrap_or([1.0, 0.0]);
        sh.shading_type(FunctionShadingType::Axial).coords([
            a[0] as f32,
            a[1] as f32,
            b[0] as f32,
            b[1] as f32,
        ]);
    }
    sh.color_space().device_rgb();
    sh.function(func_id).extend([true, true]);
    sh.finish();
    id
}

fn affine6(a: Affine) -> [f32; 6] {
    a.as_coeffs().map(|v| v as f32)
}

fn fill(
    c: &mut Content,
    s: &ShapeItem,
    p: &Paint,
    res: &mut Resources,
    pdf: &mut Pdf,
    alloc: &mut impl FnMut() -> Ref,
) {
    match p {
        Paint::Solid { color, alpha } => {
            c.save_state();
            gstate(c, *alpha, 1.0, res, pdf, alloc);
            set_color(c, color, false, res, pdf, alloc);
            path_ops(c, &s.path);
            if s.fill_rule == FillRule::Evenodd {
                c.fill_even_odd()
            } else {
                c.fill_nonzero()
            };
            c.restore_state();
        }
        Paint::Gradient {
            gradient,
            transform,
            alpha,
        } => {
            let alpha_stop = gradient.stops.iter().map(|s| s.opacity).fold(1.0, f64::min);
            let id = shading(gradient, pdf, alloc);
            let name = format!("Sh{}", res.shadings.len());
            res.shadings.push((name.clone(), id));
            c.save_state();
            gstate(c, alpha * alpha_stop, 1.0, res, pdf, alloc);
            path_ops(c, &s.path);
            if s.fill_rule == FillRule::Evenodd {
                c.clip_even_odd()
            } else {
                c.clip_nonzero()
            };
            c.end_path();
            c.transform(affine6(*transform));
            c.shading(Name(name.as_bytes()));
            c.restore_state();
        }
    }
}

fn stroke(
    c: &mut Content,
    s: &ShapeItem,
    st: &StrokeStyle,
    res: &mut Resources,
    pdf: &mut Pdf,
    alloc: &mut impl FnMut() -> Ref,
) {
    let (color, alpha) = match &st.paint {
        Paint::Solid { color, alpha } => (color.clone(), *alpha),
        Paint::Gradient {
            gradient, alpha, ..
        } => (
            gradient
                .stops
                .first()
                .map(|s| s.color.clone())
                .unwrap_or_else(Color::black),
            *alpha,
        ),
    };
    c.save_state();
    // Interno/externo (GD-OUT-04): traço duplo recortado pela forma ou pelo
    // complemento. O complemento usa par-ímpar com uma moldura em volta, o que
    // é exato para formas sem auto-sobreposição.
    if st.width > 0.0 && st.position != StrokePosition::Center {
        if st.position == StrokePosition::Outside {
            let pad = st.width * st.miter_limit.max(1.5) + 1.0;
            let b = kurbo::Shape::bounding_box(&s.path).inflate(pad, pad);
            c.rect(
                b.x0 as f32,
                b.y0 as f32,
                b.width() as f32,
                b.height() as f32,
            );
            path_ops(c, &s.path);
            c.clip_even_odd();
        } else {
            path_ops(c, &s.path);
            if s.fill_rule == FillRule::Evenodd {
                c.clip_even_odd()
            } else {
                c.clip_nonzero()
            };
        }
        c.end_path();
    }
    gstate(c, 1.0, alpha, res, pdf, alloc);
    set_color(c, &color, true, res, pdf, alloc);
    // Largura 0 em PDF = linha mais fina do dispositivo (fio de cabelo).
    c.set_line_width(if st.width > 0.0 {
        st.drawn_width() as f32
    } else {
        0.0
    });
    c.set_line_cap(match st.cap.as_str() {
        "round" => LineCapStyle::RoundCap,
        "square" => LineCapStyle::ProjectingSquareCap,
        _ => LineCapStyle::ButtCap,
    });
    c.set_line_join(match st.join.as_str() {
        "round" => LineJoinStyle::RoundJoin,
        "bevel" => LineJoinStyle::BevelJoin,
        _ => LineJoinStyle::MiterJoin,
    });
    c.set_miter_limit(st.miter_limit as f32);
    if let Some(d) = &st.dash {
        c.set_dash_pattern(d.iter().map(|v| *v as f32), st.dash_offset as f32);
    }
    path_ops(c, &s.path);
    c.stroke();
    c.restore_state();
}

fn shape(
    c: &mut Content,
    s: &ShapeItem,
    res: &mut Resources,
    pdf: &mut Pdf,
    alloc: &mut impl FnMut() -> Ref,
) {
    let behind = s.stroke.as_ref().is_some_and(|st| st.behind_fill);
    if behind {
        stroke(c, s, s.stroke.as_ref().unwrap(), res, pdf, alloc);
    }
    if let Some(p) = &s.fill {
        fill(c, s, p, res, pdf, alloc);
    }
    if !behind && let Some(st) = &s.stroke {
        stroke(c, s, st, res, pdf, alloc);
    }
}

fn image(
    c: &mut Content,
    img: &ImageItem,
    res: &mut Resources,
    pdf: &mut Pdf,
    alloc: &mut impl FnMut() -> Ref,
) {
    let d = &img.image;
    let mut rgb = Vec::with_capacity((d.width * d.height * 3) as usize);
    let mut alpha = Vec::with_capacity((d.width * d.height) as usize);
    for px in d.rgba.as_chunks::<4>().0 {
        rgb.extend_from_slice(&px[..3]);
        alpha.push(px[3]);
    }
    let mask_id = if alpha.iter().any(|a| *a < 255) {
        let id = alloc();
        let data = miniz_oxide::deflate::compress_to_vec_zlib(&alpha, 6);
        let mut m = pdf.image_xobject(id, &data);
        m.filter(Filter::FlateDecode);
        m.width(d.width as i32)
            .height(d.height as i32)
            .bits_per_component(8);
        m.color_space().device_gray();
        m.finish();
        Some(id)
    } else {
        None
    };
    let id = alloc();
    let data = miniz_oxide::deflate::compress_to_vec_zlib(&rgb, 6);
    let mut x = pdf.image_xobject(id, &data);
    x.filter(Filter::FlateDecode);
    x.width(d.width as i32)
        .height(d.height as i32)
        .bits_per_component(8);
    x.color_space().device_rgb();
    if let Some(m) = mask_id {
        x.s_mask(m);
    }
    x.interpolate(true);
    x.finish();
    let name = format!("Im{}", res.xobjects.len());
    res.xobjects.push((name.clone(), id));
    c.save_state();
    gstate(c, img.opacity, 1.0, res, pdf, alloc);
    c.transform(affine6(
        img.transform * Affine::new([img.width, 0.0, 0.0, img.height, 0.0, 0.0]),
    ));
    c.x_object(Name(name.as_bytes()));
    c.restore_state();
}
