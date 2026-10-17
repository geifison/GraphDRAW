//! Exportação SVG 1.1 (doc 15). Uma página por arquivo.
//!
//! O conteúdo fica num grupo com `matrix(0.001 0 0 -0.001 0 H)`: as
//! coordenadas são gravadas em µm com Y para cima (como no modelo) e o
//! `viewBox` fica em milímetros.

use std::fmt::Write as _;

use base64::Engine;
use kurbo::{Affine, BezPath, PathEl};

use super::ExportOptions;
use crate::model::{FillRule, Gradient};
use crate::render::raster;
use crate::render::scene::{
    ImageItem, Item, Paint, Scene, ShapeItem, StrokePosition, StrokeStyle, TextItem,
};

pub fn write_svg(scene: &Scene, opts: &ExportOptions) -> String {
    let w_mm = scene.width / 1000.0;
    let h_mm = scene.height / 1000.0;
    let mut w = SvgWriter {
        defs: String::new(),
        body: String::new(),
        next_id: 0,
    };
    if let Some(bg) = &scene.background {
        let _ = writeln!(
            w.body,
            r#"<rect x="0" y="0" width="{}" height="{}" fill="{}"/>"#,
            num(scene.width),
            num(scene.height),
            bg.to_hex()
        );
    }
    for item in &scene.items {
        // Modo de mesclagem (GD-TRA-02) em volta do item.
        let blend = scene.blend_of(item);
        let blended = blend != crate::model::BlendMode::Normal;
        if blended {
            let css = serde_json::to_value(blend)
                .ok()
                .and_then(|v| v.as_str().map(String::from))
                .unwrap_or_default();
            let _ = writeln!(w.body, r#"<g style="mix-blend-mode:{css}">"#);
        }
        match item {
            Item::Shape(s) => w.shape(s),
            Item::Image(i) => w.image(i),
            Item::Text(t) => {
                if opts.text_to_curves || t.layout.on_path {
                    for s in &t.outlines {
                        w.shape(s);
                    }
                } else {
                    w.text(t);
                }
            }
        }
        if blended {
            let _ = writeln!(w.body, "</g>");
        }
    }
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    let _ = writeln!(
        out,
        r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" version="1.1" width="{}mm" height="{}mm" viewBox="0 0 {} {}">"#,
        num(w_mm),
        num(h_mm),
        num(w_mm),
        num(h_mm)
    );
    out.push_str("<!-- Gerado pelo GraphDraw -->\n");
    if !w.defs.is_empty() {
        let _ = write!(out, "<defs>\n{}</defs>\n", w.defs);
    }
    let _ = writeln!(
        out,
        r#"<g transform="matrix(0.001 0 0 -0.001 0 {})">"#,
        num(h_mm)
    );
    out.push_str(&w.body);
    out.push_str("</g>\n</svg>\n");
    out
}

struct SvgWriter {
    defs: String,
    body: String,
    next_id: usize,
}

fn num(v: f64) -> String {
    let v = if v.abs() < 1e-9 { 0.0 } else { v };
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" { "0".into() } else { s.into() }
}

fn matrix(a: Affine) -> String {
    let c = a.as_coeffs();
    format!(
        "matrix({} {} {} {} {} {})",
        fmt6(c[0]),
        fmt6(c[1]),
        fmt6(c[2]),
        fmt6(c[3]),
        num(c[4]),
        num(c[5])
    )
}

fn fmt6(v: f64) -> String {
    let s = format!("{v:.6}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" || s.is_empty() {
        "0".into()
    } else {
        s.into()
    }
}

pub fn path_data(path: &BezPath) -> String {
    let mut d = String::new();
    for el in path.elements() {
        match *el {
            PathEl::MoveTo(p) => {
                let _ = write!(d, "M{} {}", num(p.x), num(p.y));
            }
            PathEl::LineTo(p) => {
                let _ = write!(d, "L{} {}", num(p.x), num(p.y));
            }
            PathEl::QuadTo(a, p) => {
                let _ = write!(d, "Q{} {} {} {}", num(a.x), num(a.y), num(p.x), num(p.y));
            }
            PathEl::CurveTo(a, b, p) => {
                let _ = write!(
                    d,
                    "C{} {} {} {} {} {}",
                    num(a.x),
                    num(a.y),
                    num(b.x),
                    num(b.y),
                    num(p.x),
                    num(p.y)
                );
            }
            PathEl::ClosePath => d.push('Z'),
        }
    }
    d
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

impl SvgWriter {
    fn id(&mut self, prefix: &str) -> String {
        self.next_id += 1;
        format!("{prefix}{}", self.next_id)
    }

    fn gradient(&mut self, g: &Gradient, m: Affine, alpha: f64) -> String {
        let id = self.id("g");
        let spread = match g.spread.as_str() {
            "repeat" | "reflect" => format!(r#" spreadMethod="{}""#, g.spread),
            _ => String::new(),
        };
        let head = if g.kind == "radial" {
            let c = g.center.unwrap_or([0.0, 0.0]);
            format!(
                r#"<radialGradient id="{id}" gradientUnits="userSpaceOnUse" cx="{}" cy="{}" r="{}" gradientTransform="{}"{spread}>"#,
                num(c[0]),
                num(c[1]),
                num(g.radius.unwrap_or(1.0)),
                matrix(m)
            )
        } else {
            let a = g.start.unwrap_or([0.0, 0.0]);
            let b = g.end.unwrap_or([1.0, 0.0]);
            format!(
                r#"<linearGradient id="{id}" gradientUnits="userSpaceOnUse" x1="{}" y1="{}" x2="{}" y2="{}" gradientTransform="{}"{spread}>"#,
                num(a[0]),
                num(a[1]),
                num(b[0]),
                num(b[1]),
                matrix(m)
            )
        };
        self.defs.push_str(&head);
        self.defs.push('\n');
        for s in &g.stops {
            let op = (s.opacity * alpha).clamp(0.0, 1.0);
            let _ = writeln!(
                self.defs,
                r#"<stop offset="{}" stop-color="{}"{}/>"#,
                fmt6(s.offset),
                s.color.to_hex(),
                if op < 1.0 {
                    format!(r#" stop-opacity="{}""#, fmt6(op))
                } else {
                    String::new()
                }
            );
        }
        self.defs.push_str(if g.kind == "radial" {
            "</radialGradient>\n"
        } else {
            "</linearGradient>\n"
        });
        format!("url(#{id})")
    }

    fn paint(&mut self, p: &Paint, attr: &str) -> String {
        match p {
            Paint::Solid { color, alpha } => {
                let mut s = format!(r#" {attr}="{}""#, color.to_hex());
                if *alpha < 1.0 {
                    let _ = write!(s, r#" {attr}-opacity="{}""#, fmt6(*alpha));
                }
                s
            }
            Paint::Gradient {
                gradient,
                transform,
                alpha,
            } => {
                let url = self.gradient(gradient, *transform, *alpha);
                format!(r#" {attr}="{url}""#)
            }
        }
    }

    fn stroke_attrs(&mut self, s: &StrokeStyle) -> String {
        let mut a = self.paint(&s.paint, "stroke");
        // Fio de cabelo: 0,1 mm.
        let width = if s.width <= 0.0 {
            crate::model::paint::HAIRLINE_UM
        } else {
            s.drawn_width()
        };
        let _ = write!(a, r#" stroke-width="{}""#, num(width));
        if s.cap != "butt" {
            let _ = write!(a, r#" stroke-linecap="{}""#, s.cap);
        }
        if s.join != "miter" {
            let _ = write!(a, r#" stroke-linejoin="{}""#, s.join);
        } else if s.miter_limit != 4.0 {
            let _ = write!(a, r#" stroke-miterlimit="{}""#, fmt6(s.miter_limit));
        }
        if let Some(d) = &s.dash {
            let list: Vec<String> = d.iter().map(|v| num(*v)).collect();
            let _ = write!(a, r#" stroke-dasharray="{}""#, list.join(" "));
            if s.dash_offset != 0.0 {
                let _ = write!(a, r#" stroke-dashoffset="{}""#, num(s.dash_offset));
            }
        }
        a
    }

    fn shape(&mut self, s: &ShapeItem) {
        let d = path_data(&s.path);
        if d.is_empty() {
            return;
        }
        let fill = match &s.fill {
            Some(p) => self.paint(p, "fill"),
            None => r#" fill="none""#.into(),
        };
        let rule = if s.fill_rule == FillRule::Evenodd && s.fill.is_some() {
            r#" fill-rule="evenodd""#
        } else {
            ""
        };
        if let Some(st) = s
            .stroke
            .as_ref()
            .filter(|st| st.position != StrokePosition::Center && st.width > 0.0)
        {
            self.positioned_shape(s, st, &d, &fill, rule);
            return;
        }
        let stroke = s
            .stroke
            .as_ref()
            .map(|st| self.stroke_attrs(st))
            .unwrap_or_default();
        let order = if s.stroke.as_ref().is_some_and(|st| st.behind_fill) {
            r#" paint-order="stroke""#
        } else {
            ""
        };
        let _ = writeln!(
            self.body,
            r#"<path id="o-{}" d="{d}"{fill}{rule}{stroke}{order}/>"#,
            esc(&s.object_id)
        );
    }

    /// Contorno interno (recorte pela forma) ou externo (máscara que esconde
    /// o interior), com traço de espessura dupla (GD-OUT-04).
    fn positioned_shape(
        &mut self,
        s: &ShapeItem,
        st: &StrokeStyle,
        d: &str,
        fill: &str,
        rule: &str,
    ) {
        let clip_rule = if s.fill_rule == FillRule::Evenodd {
            r#" clip-rule="evenodd""#
        } else {
            ""
        };
        let mask_rule = if s.fill_rule == FillRule::Evenodd {
            r#" fill-rule="evenodd""#
        } else {
            ""
        };
        let id = self.id("c");
        let reference = if st.position == StrokePosition::Inside {
            let _ = writeln!(
                self.defs,
                r#"<clipPath id="{id}" clipPathUnits="userSpaceOnUse"><path d="{d}"{clip_rule}/></clipPath>"#
            );
            format!(r#" clip-path="url(#{id})""#)
        } else {
            let pad = st.width * st.miter_limit.max(1.5);
            let b = kurbo::Shape::bounding_box(&s.path).inflate(pad, pad);
            let _ = writeln!(
                self.defs,
                r#"<mask id="{id}" maskUnits="userSpaceOnUse" x="{x}" y="{y}" width="{w}" height="{h}"><rect x="{x}" y="{y}" width="{w}" height="{h}" fill="white"/><path d="{d}" fill="black"{mask_rule}/></mask>"#,
                x = num(b.x0),
                y = num(b.y0),
                w = num(b.width()),
                h = num(b.height()),
            );
            format!(r#" mask="url(#{id})""#)
        };
        let stroke = self.stroke_attrs(st);
        let oid = esc(&s.object_id);
        let fill_el = format!(r#"<path d="{d}"{fill}{rule}/>"#);
        let stroke_el = format!(r#"<path d="{d}" fill="none"{stroke}{reference}/>"#);
        let (a, b) = if st.behind_fill {
            (stroke_el, fill_el)
        } else {
            (fill_el, stroke_el)
        };
        let _ = writeln!(self.body, r#"<g id="o-{oid}">{a}{b}</g>"#);
    }

    fn image(&mut self, img: &ImageItem) {
        let (bytes, mime) = match &img.image.original {
            Some((b, mime)) => (b.to_vec(), *mime),
            None => {
                let d = &img.image;
                let Some(pm) = raster::premultiplied_pixmap(&d.rgba, d.width, d.height) else {
                    return;
                };
                match raster::encode_png(&pm, None) {
                    Ok(b) => (b, "image/png"),
                    Err(_) => return,
                }
            }
        };
        let data = base64::engine::general_purpose::STANDARD.encode(bytes);
        // Imagem SVG cresce para baixo: espelha dentro do espaço local.
        let m = img.transform * Affine::new([1.0, 0.0, 0.0, -1.0, 0.0, img.height]);
        let _ = writeln!(
            self.body,
            r#"<image id="o-{}" x="0" y="0" width="{}" height="{}" preserveAspectRatio="none" transform="{}"{} xlink:href="data:{mime};base64,{data}"/>"#,
            esc(&img.object_id),
            num(img.width),
            num(img.height),
            matrix(m),
            if img.opacity < 1.0 {
                format!(r#" opacity="{}""#, fmt6(img.opacity))
            } else {
                String::new()
            }
        );
    }

    fn text(&mut self, t: &TextItem) {
        // Texto cresce para baixo no SVG: espelha Y no espaço local.
        let m = t.transform * Affine::new([1.0, 0.0, 0.0, -1.0, 0.0, 0.0]);
        let _ = writeln!(
            self.body,
            r#"<g id="o-{}" transform="{}">"#,
            esc(&t.object_id),
            matrix(m)
        );
        let mut i = 0;
        for line in &t.layout.lines {
            for run in &line.runs {
                let fill = t.outlines.get(i).and_then(|o| o.fill.clone());
                i += 1;
                let fill = match &fill {
                    // Gradientes de texto: usa a primeira parada (texto vivo no SVG).
                    Some(Paint::Gradient {
                        gradient, alpha, ..
                    }) => Paint::Solid {
                        color: gradient
                            .stops
                            .first()
                            .map(|s| s.color.clone())
                            .unwrap_or_else(crate::model::Color::black),
                        alpha: *alpha,
                    },
                    Some(p) => p.clone(),
                    None => Paint::Solid {
                        color: crate::model::Color::black(),
                        alpha: 1.0,
                    },
                };
                let fill = self.paint(&fill, "fill");
                let _ = writeln!(
                    self.body,
                    r#"<text x="{}" y="{}" font-family="{}" font-size="{}"{}{} xml:space="preserve"{fill}>{}</text>"#,
                    num(run.x),
                    num(-run.y),
                    esc(&run.family),
                    num(run.size),
                    if run.weight != 400 {
                        format!(r#" font-weight="{}""#, run.weight)
                    } else {
                        String::new()
                    },
                    if run.italic {
                        r#" font-style="italic""#
                    } else {
                        ""
                    },
                    esc(&run.text)
                );
            }
        }
        self.body.push_str("</g>\n");
    }
}
