//! Exportação PDF (doc 15, GD-EXP-07): PDF comum (tela) e PDF/X-1a:2001
//! (gráfica, ISO 15930-1).
//!
//! **PDF comum (1.7):** cores no modelo original (RGB → DeviceRGB, CMYK →
//! DeviceCMYK, cinza → DeviceGray, especial/registro → Separation,
//! GD-COR-02); opacidade por ExtGState; gradientes por shading RGB; imagens
//! com SMask.
//!
//! **PDF/X-1a:2001 (1.3):** só CMYK, cinza e Separation com alternativa
//! CMYK; nada de transparência (opacidade, alfa de gradientes e imagens é
//! achatado sobre o branco do papel); imagens em DeviceCMYK sem SMask;
//! TrimBox/BleedBox/MediaBox com a sangria; OutputIntent GTS_PDFX; Info com
//! as chaves GTS e Trapped; /ID no trailer. O texto sai sempre em curvas, então
//! não há fontes a incorporar. Tudo o que foi convertido entra no
//! [`PdfReport`].
//!
//! Transparência: cada objeto transparente vira uma imagem CMYK de 300 ppp da
//! composição (ele + o que está embaixo), recortada pelo contorno vetorial
//! dele; o resto do documento continua vetorial.
//!
//! Sem gerenciamento de cor (GD-CMS, M2), RGB → CMYK é por fórmula — o
//! relatório avisa.

use std::collections::HashMap;

use kurbo::{Affine, BezPath, PathEl};
use pdf_writer::types::{
    FunctionShadingType, LineCapStyle, LineJoinStyle, OutputIntentSubtype, TrappingStatus,
};
use pdf_writer::{Content, Date, Filter, Finish, Name, Pdf, Rect, Ref, TextStr};

use super::{ExportOptions, PdfStandard};
use crate::model::{Color, FillRule, Gradient};
use crate::render::scene::{ImageItem, Item, Paint, Scene, ShapeItem, StrokePosition, StrokeStyle};
use crate::units::UM_PER_INCH;

/// µm → pt.
const K: f64 = 72.0 / UM_PER_INCH;

/// Resolução mínima recomendada para impressão.
pub const MIN_PRINT_PPI: f64 = 300.0;

/// O que a exportação PDF/X-1a precisou converter ou achatar.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PdfReport {
    /// Cores RGB (ou de outro modelo) convertidas para CMYK.
    pub colors_converted: usize,
    /// Pinturas com opacidade < 1 achatadas sobre o branco.
    pub flattened: usize,
    /// Gradientes convertidos para CMYK.
    pub gradients_converted: usize,
    /// Imagens convertidas para CMYK.
    pub images_converted: usize,
    /// Imagens com transparência achatadas sobre o branco.
    pub images_flattened: usize,
    /// Imagens abaixo de 300 ppp: (página base 1, ppp efetivo).
    pub low_res: Vec<(usize, f64)>,
}

impl PdfReport {
    /// Avisos para o usuário (vazio = nada precisou mudar).
    pub fn messages(&self) -> Vec<String> {
        let mut m = Vec::new();
        if self.colors_converted > 0 {
            m.push(format!(
                "{} cor(es) RGB convertida(s) para CMYK por fórmula (sem perfil ICC); confira cores vivas.",
                self.colors_converted
            ));
        }
        if self.gradients_converted > 0 {
            m.push(format!(
                "{} gradiente(s) convertido(s) para CMYK.",
                self.gradients_converted
            ));
        }
        if self.images_converted > 0 {
            m.push(format!(
                "{} imagem(ns) convertida(s) para CMYK.",
                self.images_converted
            ));
        }
        if self.flattened + self.images_flattened > 0 {
            m.push(format!(
                "{} objeto(s) com transparência achatado(s) em imagem CMYK de 300 ppp, recortada pelo contorno vetorial (aparência igual à da tela).",
                self.flattened + self.images_flattened
            ));
        }
        for (page, ppi) in &self.low_res {
            m.push(format!(
                "Página {page}: imagem com {ppi:.0} ppp (abaixo de {MIN_PRINT_PPI:.0} ppp recomendados para impressão)."
            ));
        }
        m
    }
}

struct W {
    pdf: Pdf,
    next: i32,
    x1a: bool,
    report: PdfReport,
    page_no: usize,
}

impl W {
    fn alloc(&mut self) -> Ref {
        let r = Ref::new(self.next);
        self.next += 1;
        r
    }

    /// Cor permitida no PDF/X-1a, com a opacidade achatada sobre o papel.
    fn print_color(&mut self, color: &Color, alpha: f64) -> Color {
        let a = alpha.clamp(0.0, 1.0);
        if a < 1.0 {
            self.report.flattened += 1;
        }
        let v = |i: usize| color.values.get(i).copied().unwrap_or(0.0).clamp(0.0, 1.0);
        match color.model.as_str() {
            "cmyk" => Color::cmyk(v(0) * a, v(1) * a, v(2) * a, v(3) * a),
            // Cinza guardado como luminosidade (1 = branco).
            "gray" => Color::gray(1.0 - (1.0 - v(0)) * a),
            "spot" | "registration" => {
                let mut c = color.clone();
                if let Some(t) = c.values.first_mut() {
                    *t = (*t * a).clamp(0.0, 1.0);
                } else {
                    c.values = vec![a];
                }
                if color.model == "spot"
                    && let Some(alt) = &color.alternate
                    && alt.model != "cmyk"
                {
                    self.report.colors_converted += 1;
                    c.alternate = Some(Box::new(rgb_to_cmyk(alt.to_srgb())));
                }
                c
            }
            _ => {
                self.report.colors_converted += 1;
                let k = rgb_to_cmyk(color.to_srgb());
                let w = |i: usize| k.values[i] * a;
                Color::cmyk(w(0), w(1), w(2), w(3))
            }
        }
    }
}

/// RGB (0–1) → CMYK por fórmula (remoção de cinza total). Sem perfil ICC.
pub fn rgb_to_cmyk([r, g, b]: [f64; 3]) -> Color {
    let k = 1.0 - r.max(g).max(b);
    if k >= 1.0 - 1e-9 {
        return Color::cmyk(0.0, 0.0, 0.0, 1.0);
    }
    Color::cmyk(
        (1.0 - r - k) / (1.0 - k),
        (1.0 - g - k) / (1.0 - k),
        (1.0 - b - k) / (1.0 - k),
        k,
    )
}

/// Exporta e devolve também o relatório (vazio no PDF comum).
pub fn write_pdf_report(
    scenes: &[Scene],
    title: Option<&str>,
    opts: &ExportOptions,
) -> (Vec<u8>, PdfReport) {
    let x1a = matches!(opts.pdf, PdfStandard::X1a(_));
    let mut w = W {
        pdf: Pdf::new(),
        next: 1,
        x1a,
        report: PdfReport::default(),
        page_no: 0,
    };
    if x1a {
        w.pdf.set_version(1, 3);
    }
    let catalog_id = w.alloc();
    let tree_id = w.alloc();
    let info_id = w.alloc();
    let page_ids: Vec<Ref> = scenes.iter().map(|_| w.alloc()).collect();

    {
        let mut cat = w.pdf.catalog(catalog_id);
        cat.pages(tree_id);
        if let PdfStandard::X1a(cond) = opts.pdf {
            let mut intents = cat.output_intents();
            intents
                .push()
                .subtype(OutputIntentSubtype::PDFX)
                .output_condition_identifier(TextStr(cond.identifier()))
                .output_condition(TextStr(cond.description()))
                .registry_name(TextStr("http://www.color.org"))
                .info(TextStr(cond.description()));
        }
    }
    w.pdf
        .pages(tree_id)
        .kids(page_ids.iter().copied())
        .count(page_ids.len() as i32);
    let now = now_utc();
    {
        let mut info = w.pdf.document_info(info_id);
        info.producer(TextStr(concat!("GraphDraw ", env!("CARGO_PKG_VERSION"))));
        info.creator(TextStr("GraphDraw"));
        let t = title.filter(|t| !t.trim().is_empty());
        if x1a {
            // PDF/X exige Title, datas e Trapped.
            info.title(TextStr(t.unwrap_or("Sem título")));
            info.creation_date(now);
            info.modified_date(now);
            info.trapped(TrappingStatus::NotTrapped);
            info.pair(Name(b"GTS_PDFXVersion"), TextStr("PDF/X-1:2001"));
            info.pair(Name(b"GTS_PDFXConformance"), TextStr("PDF/X-1a:2001"));
        } else if let Some(t) = t {
            info.title(TextStr(t));
        }
    }

    for (i, (scene, &page_id)) in scenes.iter().zip(&page_ids).enumerate() {
        w.page_no = i + 1;
        write_page(&mut w, scene, page_id, tree_id);
    }

    if x1a {
        // /ID: hash do conteúdo de identificação (PDF/X exige).
        use sha2::{Digest, Sha256};
        let mut h = Sha256::new();
        h.update(title.unwrap_or("").as_bytes());
        h.update(format!("{now:?}{}", scenes.len()).as_bytes());
        for s in scenes {
            h.update(format!("{}x{}:{}", s.width, s.height, s.items.len()).as_bytes());
        }
        let id = h.finalize()[..16].to_vec();
        w.pdf.set_file_id((id.clone(), id));
    }
    let report = w.report.clone();
    (w.pdf.finish(), report)
}

pub fn write_pdf(scenes: &[Scene], title: Option<&str>, opts: &ExportOptions) -> Vec<u8> {
    write_pdf_report(scenes, title, opts).0
}

fn write_page(w: &mut W, scene: &Scene, page_id: Ref, tree_id: Ref) {
    let mut res = Resources::default();
    let mut c = Content::new();
    // No PDF/X a página ganha a sangria em volta: o conteúdo é deslocado e a
    // área de corte (TrimBox) fica no meio.
    let bleed = if w.x1a { scene.bleed } else { 0.0 };
    c.transform([
        K as f32,
        0.0,
        0.0,
        K as f32,
        (bleed * K) as f32,
        (bleed * K) as f32,
    ]);
    if let Some(bg) = &scene.background {
        let col = if w.x1a {
            w.print_color(bg, 1.0)
        } else {
            bg.clone()
        };
        set_color(&mut c, &col, false, &mut res, w);
        c.rect(
            -bleed as f32,
            -bleed as f32,
            (scene.width + 2.0 * bleed) as f32,
            (scene.height + 2.0 * bleed) as f32,
        );
        c.fill_nonzero();
    }
    for (i, item) in scene.items.iter().enumerate() {
        // PDF/X-1a: objeto com transparência vira imagem CMYK da composição
        // (tudo o que está embaixo + ele), recortada pelo contorno vetorial.
        if w.x1a && is_transparent(item) {
            flatten_item(&mut c, scene, i, &mut res, w);
            continue;
        }
        match item {
            Item::Shape(s) => shape(&mut c, s, &mut res, w),
            Item::Image(i) => image(&mut c, i, &mut res, w),
            Item::Text(t) => {
                for s in &t.outlines {
                    shape(&mut c, s, &mut res, w);
                }
            }
        }
    }
    let content_id = w.alloc();
    let data = miniz_oxide::deflate::compress_to_vec_zlib(&c.finish(), 6);
    w.pdf.stream(content_id, &data).filter(Filter::FlateDecode);

    let pt = |v: f64| (v * K) as f32;
    let media = Rect::new(
        0.0,
        0.0,
        pt(scene.width + 2.0 * bleed),
        pt(scene.height + 2.0 * bleed),
    );
    let mut page = w.pdf.page(page_id);
    page.parent(tree_id).media_box(media).contents(content_id);
    if w.x1a {
        page.bleed_box(media);
        page.trim_box(Rect::new(
            pt(bleed),
            pt(bleed),
            pt(bleed + scene.width),
            pt(bleed + scene.height),
        ));
    }
    let mut r = page.resources();
    if !res.gstates_list.is_empty() {
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

/// Resolução do achatamento de transparência.
const FLATTEN_PPI: f64 = 300.0;

fn paint_alpha(p: &Paint) -> f64 {
    match p {
        Paint::Solid { alpha, .. } => *alpha,
        Paint::Gradient {
            gradient, alpha, ..
        } => alpha * gradient.stops.iter().map(|s| s.opacity).fold(1.0, f64::min),
    }
}

fn shape_transparent(s: &ShapeItem) -> bool {
    s.fill.as_ref().is_some_and(|f| paint_alpha(f) < 1.0)
        || s.stroke
            .as_ref()
            .is_some_and(|st| paint_alpha(&st.paint) < 1.0)
}

/// O item tem alguma transparência (opacidade, gradiente com opacidade,
/// imagem com canal alfa)?
fn is_transparent(item: &Item) -> bool {
    match item {
        Item::Shape(s) => shape_transparent(s),
        Item::Text(t) => t.outlines.iter().any(shape_transparent),
        Item::Image(i) => {
            i.opacity < 1.0 || i.image.rgba.as_chunks::<4>().0.iter().any(|p| p[3] < 255)
        }
    }
}

/// Áreas que o item cobre (em coordenadas da página), para recortar a imagem
/// achatada: preenchimento com a regra dele; contorno expandido em área.
fn coverage(item: &Item) -> Vec<(BezPath, FillRule)> {
    fn of_shape(s: &ShapeItem, out: &mut Vec<(BezPath, FillRule)>) {
        if s.fill.is_some() {
            out.push((s.path.clone(), s.fill_rule));
        }
        if let Some(st) = &s.stroke {
            let w = if st.width > 0.0 {
                st.drawn_width()
            } else {
                crate::model::paint::HAIRLINE_UM
            };
            let cap = match st.cap.as_str() {
                "round" => kurbo::Cap::Round,
                "square" => kurbo::Cap::Square,
                _ => kurbo::Cap::Butt,
            };
            let join = match st.join.as_str() {
                "round" => kurbo::Join::Round,
                "bevel" => kurbo::Join::Bevel,
                _ => kurbo::Join::Miter,
            };
            let ks = kurbo::Stroke::new(w)
                .with_caps(cap)
                .with_join(join)
                .with_miter_limit(st.miter_limit);
            let outline = kurbo::stroke(
                s.path.iter(),
                &ks,
                &kurbo::StrokeOpts::default(),
                (w / 1000.0).max(1.0),
            );
            out.push((outline, FillRule::Nonzero));
        }
    }
    let mut out = Vec::new();
    match item {
        Item::Shape(s) => of_shape(s, &mut out),
        Item::Text(t) => t.outlines.iter().for_each(|s| of_shape(s, &mut out)),
        Item::Image(i) => {
            let mut r = kurbo::Shape::to_path(&kurbo::Rect::new(0.0, 0.0, i.width, i.height), 0.1);
            r.apply_affine(i.transform);
            out.push((r, FillRule::Nonzero));
        }
    }
    out
}

/// Achata o item `index`: renderiza a composição dos itens 0..=index na caixa
/// dele a 300 ppp, converte para CMYK e desenha a imagem recortada pelas
/// áreas que ele cobre. O que está fora dessas áreas continua vetorial.
fn flatten_item(c: &mut Content, scene: &Scene, index: usize, res: &mut Resources, w: &mut W) {
    use crate::render::raster::{self, RasterOptions};
    let item = &scene.items[index];
    let area = kurbo::Rect::new(
        -scene.bleed,
        -scene.bleed,
        scene.width + scene.bleed,
        scene.height + scene.bleed,
    );
    let b = scene.item_bounds()[index].intersect(area);
    if b.width() <= 0.0 || b.height() <= 0.0 {
        return;
    }
    let s = FLATTEN_PPI / UM_PER_INCH;
    let (pw, ph) = (
        (b.width() * s).ceil().max(1.0) as u32,
        (b.height() * s).ceil().max(1.0) as u32,
    );
    // Não passar de ~40 megapixels por objeto.
    let k = (40.0e6 / (pw as f64 * ph as f64)).sqrt().min(1.0);
    let (pw, ph, s) = (
        ((pw as f64 * k).ceil() as u32).max(1),
        ((ph as f64 * k).ceil() as u32).max(1),
        s * k,
    );
    let sub = Scene {
        width: scene.width,
        height: scene.height,
        background: scene.background.clone(),
        bleed: scene.bleed,
        items: scene.items[..=index].to_vec(),
        missing_fonts: Vec::new(),
        item_bounds: Default::default(),
    };
    // Pixel (u, v) ← página: x = b.x0 + u/s, y = b.y1 − v/s.
    let view = Affine::new([s, 0.0, 0.0, -s, -b.x0 * s, b.y1 * s]);
    let opts = RasterOptions {
        antialias: true,
        background: Some([255, 255, 255, 255]),
    };
    let Some(pm) = raster::render(&sub, view, pw, ph, &opts) else {
        return;
    };
    let mut cmyk = Vec::with_capacity((pw * ph * 4) as usize);
    for px in pm.pixels() {
        let p = px.demultiply();
        let k = rgb_to_cmyk([
            p.red() as f64 / 255.0,
            p.green() as f64 / 255.0,
            p.blue() as f64 / 255.0,
        ]);
        cmyk.extend(
            k.values
                .iter()
                .map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8),
        );
    }
    let id = w.alloc();
    let data = miniz_oxide::deflate::compress_to_vec_zlib(&cmyk, 6);
    let mut x = w.pdf.image_xobject(id, &data);
    x.filter(Filter::FlateDecode);
    x.width(pw as i32).height(ph as i32).bits_per_component(8);
    x.color_space().device_cmyk();
    x.finish();
    let name = format!("Im{}", res.xobjects.len());
    res.xobjects.push((name.clone(), id));
    // A imagem cobre a caixa `b` (largura/altura reais do raster).
    let (iw, ih) = (pw as f64 / s, ph as f64 / s);
    for (path, rule) in coverage(item) {
        c.save_state();
        path_ops(c, &path);
        if rule == FillRule::Evenodd {
            c.clip_even_odd()
        } else {
            c.clip_nonzero()
        };
        c.end_path();
        c.transform([
            iw as f32,
            0.0,
            0.0,
            ih as f32,
            b.x0 as f32,
            (b.y1 - ih) as f32,
        ]);
        c.x_object(Name(name.as_bytes()));
        c.restore_state();
    }
    w.report.flattened += 1;
    w.report.colors_converted += non_cmyk_colors(item);
    if let Item::Image(img) = item {
        w.report.images_converted += 1;
        check_ppi(img, w);
    }
}

/// Data e hora atuais em UTC (sem dependência de calendário).
fn now_utc() -> Date {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs()) as i64;
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    // Dias desde 1970-01-01 → data civil (algoritmo de H. Hinnant).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    Date::new(y as u16)
        .month(m as u8)
        .day(d as u8)
        .hour((rem / 3600) as u8)
        .minute((rem % 3600 / 60) as u8)
        .second((rem % 60) as u8)
        .utc_offset_hour(0)
        .utc_offset_minute(0)
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
                let p0 = last_point(path, el);
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

fn last_point(path: &BezPath, target: &PathEl) -> kurbo::Point {
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

/// Opacidade por ExtGState (só no PDF comum; no PDF/X ela já foi achatada).
fn gstate(c: &mut Content, fill_alpha: f64, stroke_alpha: f64, res: &mut Resources, w: &mut W) {
    if w.x1a {
        return;
    }
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
            let id = w.alloc();
            w.pdf
                .ext_graphics(id)
                .non_stroking_alpha(key.0 as f32 / 1000.0)
                .stroking_alpha(key.1 as f32 / 1000.0);
            res.gstates.insert(key, name.clone());
            res.gstates_list.push((name.clone(), id));
            name
        }
    };
    c.set_parameters(Name(name.as_bytes()));
}

fn set_color(c: &mut Content, color: &Color, stroke: bool, res: &mut Resources, w: &mut W) {
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
                let a = color
                    .alternate
                    .as_deref()
                    .cloned()
                    .unwrap_or_else(|| Color::gray(0.0));
                // PDF/X-1a: a alternativa da cor especial tem de ser CMYK.
                if w.x1a && a.model != "cmyk" {
                    rgb_to_cmyk(a.to_srgb())
                } else {
                    a
                }
            };
            let key = format!("{ink}|{:?}|{:?}", alt.model, alt.values);
            let name = match res.spaces.get(&key) {
                Some(n) => n.clone(),
                None => {
                    let name = format!("CS{}", res.spaces.len());
                    let id = w.alloc();
                    let mut sep = w.pdf.color_space(id).separation(Name(ink.as_bytes()));
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

/// Valores das paradas do gradiente: RGB no PDF comum; CMYK (com a opacidade
/// achatada) no PDF/X-1a.
fn shading(g: &Gradient, alpha: f64, w: &mut W) -> Ref {
    let mut stops: Vec<(f64, Vec<f32>)> = g
        .stops
        .iter()
        .map(|s| {
            let vals = if w.x1a {
                // Só a conversão; as paradas não somam no contador de cores.
                let saved = w.report.clone();
                let c = w.print_color(&s.color, alpha * s.opacity);
                w.report = saved;
                let c = match c.model.as_str() {
                    "cmyk" => c,
                    "gray" => Color::cmyk(0.0, 0.0, 0.0, 1.0 - c.values[0]),
                    // Cor especial em gradiente: a alternativa CMYK na tinta pedida.
                    _ => {
                        let t = c.values.first().copied().unwrap_or(1.0);
                        let alt = c
                            .alternate
                            .as_deref()
                            .cloned()
                            .unwrap_or_else(|| Color::cmyk(0.0, 0.0, 0.0, 1.0));
                        let alt = if alt.model == "cmyk" {
                            alt
                        } else {
                            rgb_to_cmyk(alt.to_srgb())
                        };
                        Color::cmyk(
                            alt.values[0] * t,
                            alt.values[1] * t,
                            alt.values[2] * t,
                            alt.values[3] * t,
                        )
                    }
                };
                c.values.iter().map(|v| *v as f32).collect()
            } else {
                let [r, gg, b] = s.color.to_srgb();
                vec![r as f32, gg as f32, b as f32]
            };
            (s.offset.clamp(0.0, 1.0), vals)
        })
        .collect();
    if w.x1a {
        w.report.gradients_converted += 1;
        if alpha < 1.0 || g.stops.iter().any(|s| s.opacity < 1.0) {
            w.report.flattened += 1;
        }
    }
    stops.sort_by(|a, b| a.0.total_cmp(&b.0));
    if stops[0].0 > 0.0 {
        stops.insert(0, (0.0, stops[0].1.clone()));
    }
    if stops.last().unwrap().0 < 1.0 {
        stops.push((1.0, stops.last().unwrap().1.clone()));
    }
    if stops.len() == 1 {
        stops.push((1.0, stops[0].1.clone()));
    }
    let func_id = if stops.len() == 2 {
        let id = w.alloc();
        w.pdf
            .exponential_function(id)
            .domain([0.0, 1.0])
            .c0(stops[0].1.clone())
            .c1(stops[1].1.clone())
            .n(1.0);
        id
    } else {
        let parts: Vec<Ref> = stops
            .windows(2)
            .map(|win| {
                let id = w.alloc();
                w.pdf
                    .exponential_function(id)
                    .domain([0.0, 1.0])
                    .c0(win[0].1.clone())
                    .c1(win[1].1.clone())
                    .n(1.0);
                id
            })
            .collect();
        let id = w.alloc();
        let bounds: Vec<f32> = stops[1..stops.len() - 1]
            .iter()
            .map(|s| s.0 as f32)
            .collect();
        let encode: Vec<f32> = parts.iter().flat_map(|_| [0.0f32, 1.0]).collect();
        w.pdf
            .stitching_function(id)
            .domain([0.0, 1.0])
            .functions(parts)
            .bounds(bounds)
            .encode(encode);
        id
    };
    let id = w.alloc();
    let x1a = w.x1a;
    let mut sh = w.pdf.function_shading(id);
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
    if x1a {
        sh.color_space().device_cmyk();
    } else {
        sh.color_space().device_rgb();
    }
    sh.function(func_id).extend([true, true]);
    sh.finish();
    id
}

fn affine6(a: Affine) -> [f32; 6] {
    a.as_coeffs().map(|v| v as f32)
}

fn fill(c: &mut Content, s: &ShapeItem, p: &Paint, res: &mut Resources, w: &mut W) {
    match p {
        Paint::Solid { color, alpha } => {
            c.save_state();
            let col = if w.x1a {
                w.print_color(color, *alpha)
            } else {
                gstate(c, *alpha, 1.0, res, w);
                color.clone()
            };
            set_color(c, &col, false, res, w);
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
            let id = shading(gradient, *alpha, w);
            let name = format!("Sh{}", res.shadings.len());
            res.shadings.push((name.clone(), id));
            c.save_state();
            gstate(c, alpha * alpha_stop, 1.0, res, w);
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

fn stroke(c: &mut Content, s: &ShapeItem, st: &StrokeStyle, res: &mut Resources, w: &mut W) {
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
    let col = if w.x1a {
        w.print_color(&color, alpha)
    } else {
        gstate(c, 1.0, alpha, res, w);
        color
    };
    set_color(c, &col, true, res, w);
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

fn shape(c: &mut Content, s: &ShapeItem, res: &mut Resources, w: &mut W) {
    let behind = s.stroke.as_ref().is_some_and(|st| st.behind_fill);
    if behind {
        stroke(c, s, s.stroke.as_ref().unwrap(), res, w);
    }
    if let Some(p) = &s.fill {
        fill(c, s, p, res, w);
    }
    if !behind && let Some(st) = &s.stroke {
        stroke(c, s, st, res, w);
    }
}

/// Resolução efetiva da imagem no tamanho impresso; avisa abaixo de 300 ppp.
fn check_ppi(img: &ImageItem, w: &mut W) {
    let d = &img.image;
    let side = |v: kurbo::Vec2| {
        (img.transform * v.to_point() - img.transform * kurbo::Point::ORIGIN).hypot()
    };
    let w_in = side(kurbo::Vec2::new(img.width, 0.0)) / UM_PER_INCH;
    let h_in = side(kurbo::Vec2::new(0.0, img.height)) / UM_PER_INCH;
    let ppi = (d.width as f64 / w_in.max(1e-9)).min(d.height as f64 / h_in.max(1e-9));
    if ppi < MIN_PRINT_PPI - 0.5 {
        let page = w.page_no;
        w.report.low_res.push((page, ppi));
    }
}

/// Cores fora de CMYK/cinza/especial usadas pelo item (para o relatório).
fn non_cmyk_colors(item: &Item) -> usize {
    fn of_paint(p: &Paint) -> usize {
        let bad =
            |c: &Color| !matches!(c.model.as_str(), "cmyk" | "gray" | "spot" | "registration");
        match p {
            Paint::Solid { color, .. } => bad(color) as usize,
            Paint::Gradient { gradient, .. } => {
                gradient.stops.iter().filter(|s| bad(&s.color)).count()
            }
        }
    }
    fn of_shape(s: &ShapeItem) -> usize {
        s.fill.as_ref().map_or(0, of_paint) + s.stroke.as_ref().map_or(0, |st| of_paint(&st.paint))
    }
    match item {
        Item::Shape(s) => of_shape(s),
        Item::Text(t) => t.outlines.iter().map(of_shape).sum(),
        Item::Image(_) => 0,
    }
}

fn image(c: &mut Content, img: &ImageItem, res: &mut Resources, w: &mut W) {
    let d = &img.image;
    let n = (d.width * d.height) as usize;
    let id = if w.x1a {
        check_ppi(img, w);
        // Alfa e opacidade achatados sobre o branco; depois RGB → CMYK.
        let op = img.opacity.clamp(0.0, 1.0);
        let mut transparent = op < 1.0;
        let mut cmyk = Vec::with_capacity(n * 4);
        for px in d.rgba.as_chunks::<4>().0 {
            let a = px[3] as f64 / 255.0 * op;
            if px[3] < 255 {
                transparent = true;
            }
            let mix = |ch: u8| 1.0 - (1.0 - ch as f64 / 255.0) * a;
            let k = rgb_to_cmyk([mix(px[0]), mix(px[1]), mix(px[2])]);
            cmyk.extend(
                k.values
                    .iter()
                    .map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8),
            );
        }
        w.report.images_converted += 1;
        if transparent {
            w.report.images_flattened += 1;
        }
        let id = w.alloc();
        let data = miniz_oxide::deflate::compress_to_vec_zlib(&cmyk, 6);
        let mut x = w.pdf.image_xobject(id, &data);
        x.filter(Filter::FlateDecode);
        x.width(d.width as i32)
            .height(d.height as i32)
            .bits_per_component(8);
        x.color_space().device_cmyk();
        x.finish();
        id
    } else {
        let mut rgb = Vec::with_capacity(n * 3);
        let mut alpha = Vec::with_capacity(n);
        for px in d.rgba.as_chunks::<4>().0 {
            rgb.extend_from_slice(&px[..3]);
            alpha.push(px[3]);
        }
        let mask_id = if alpha.iter().any(|a| *a < 255) {
            let id = w.alloc();
            let data = miniz_oxide::deflate::compress_to_vec_zlib(&alpha, 6);
            let mut m = w.pdf.image_xobject(id, &data);
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
        let id = w.alloc();
        let data = miniz_oxide::deflate::compress_to_vec_zlib(&rgb, 6);
        let mut x = w.pdf.image_xobject(id, &data);
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
        id
    };
    let name = format!("Im{}", res.xobjects.len());
    res.xobjects.push((name.clone(), id));
    c.save_state();
    gstate(c, img.opacity, 1.0, res, w);
    c.transform(affine6(
        img.transform * Affine::new([img.width, 0.0, 0.0, img.height, 0.0, 0.0]),
    ));
    c.x_object(Name(name.as_bytes()));
    c.restore_state();
}
