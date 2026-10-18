//! Converte as chamadas da librevenge (emitidas pela ponte) no modelo do
//! GraphDraw (GD-CDR-02), registrando aproximações no relatório (GD-CDR-03).
//!
//! Coordenadas da librevenge: polegadas, Y para baixo, origem no canto
//! superior esquerdo da página. Modelo: µm, Y para cima, origem no canto
//! inferior esquerdo (GD-DOC-02).

use std::f64::consts::PI;

use base64::Engine;
use kurbo::Affine;
use serde_json::{Map, Value};

use crate::model::FillRule;
use crate::model::{
    Color, Document, EllipseData, Fill, FontSpec, Gradient, GradientStop, GroupData, ImageData,
    Layer, Object, ObjectKind, Page, Paragraph, PathData, RectData, Stroke, SubpathBuilder,
    TextData, TextSpan, Transform,
};
use crate::report::ImportReport;
use crate::units::{UM_PER_INCH, UM_PER_PT};

/// Lista de propriedades de uma chamada.
#[derive(Default, Clone)]
pub struct Props(Map<String, Value>);

impl Props {
    fn raw(&self, key: &str) -> Option<&Value> {
        self.0.get(key)
    }
    pub fn str(&self, key: &str) -> Option<&str> {
        self.raw(key)?.get(0)?.as_str()
    }
    fn unit(&self, key: &str) -> Option<&str> {
        self.raw(key)?.get(1)?.as_str()
    }
    fn num(&self, key: &str) -> Option<f64> {
        self.raw(key)?.get(2)?.as_f64()
    }
    /// Comprimento em µm (unidade padrão da librevenge: polegada).
    pub fn len_um(&self, key: &str) -> Option<f64> {
        let v = self.num(key)?;
        Some(match self.unit(key)? {
            "pt" => v * UM_PER_PT,
            "twip" => v * UM_PER_INCH / 1440.0,
            "%" => return None,
            _ => v * UM_PER_INCH,
        })
    }
    /// Valor adimensional; porcentagens vêm como fração (0–1).
    pub fn ratio(&self, key: &str) -> Option<f64> {
        self.num(key)
    }
    pub fn int(&self, key: &str) -> Option<i64> {
        let v = self.num(key)?;
        Some(v as i64)
    }
    fn vec(&self, key: &str) -> Vec<Props> {
        self.raw(key)
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_object().cloned().map(Props))
                    .collect()
            })
            .unwrap_or_default()
    }
    fn is_true(&self, key: &str) -> bool {
        matches!(self.str(key), Some("true") | Some("1"))
    }
}

struct TextBuild {
    obj: Object,
    data: TextData,
    para: Option<Paragraph>,
    span: Option<TextSpan>,
}

struct Mapper<'r> {
    doc: Document,
    report: &'r mut ImportReport,
    page: Option<Page>,
    in_master: bool,
    page_height: f64,
    style: Props,
    /// Pilha de grupos abertos (filhos acumulados).
    groups: Vec<Vec<Object>>,
    text: Option<TextBuild>,
    skip_depth: usize,
}

/// Mapeia linhas JSON produzidas pela ponte.
pub fn map_events(stdout: &[u8], report: &mut ImportReport) -> Document {
    let mut m = Mapper {
        doc: Document::new(),
        report,
        page: None,
        in_master: false,
        page_height: 0.0,
        style: Props::default(),
        groups: Vec::new(),
        text: None,
        skip_depth: 0,
    };
    for line in stdout.split(|b| *b == b'\n') {
        if line.iter().all(|b| b.is_ascii_whitespace()) {
            continue;
        }
        let Ok(Value::Object(ev)) = serde_json::from_slice::<Value>(line) else {
            m.report.warn(
                "cdr.bridge.bad-line",
                "linha ilegível na saída do leitor .cdr; ignorada",
                m.page_no(),
            );
            continue;
        };
        let op = ev
            .get("op")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let props = Props(
            ev.get("p")
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default(),
        );
        let text = ev.get("text").and_then(Value::as_str).unwrap_or("");
        m.event(&op, &props, text);
    }
    m.finish_page();
    m.doc
}

impl Mapper<'_> {
    fn page_no(&self) -> Option<usize> {
        if self.page.is_some() {
            Some(self.doc.data.pages.len() + 1)
        } else {
            None
        }
    }

    fn warn(&mut self, code: &str, msg: &str) {
        let p = self.page_no();
        self.report.warn(code, msg, p);
    }

    fn x(&self, inches: f64) -> f64 {
        inches * UM_PER_INCH
    }
    fn y(&self, inches: f64) -> f64 {
        self.page_height - inches * UM_PER_INCH
    }
    fn pt(&self, p: &Props, kx: &str, ky: &str) -> Option<[f64; 2]> {
        Some([self.x(p.num(kx)?), self.y(p.num(ky)?)])
    }

    fn event(&mut self, op: &str, p: &Props, text: &str) {
        if self.skip_depth > 0 {
            match op {
                "startEmbeddedGraphics" | "startTableObject" => self.skip_depth += 1,
                "endEmbeddedGraphics" | "endTableObject" => self.skip_depth -= 1,
                _ => {}
            }
            return;
        }
        match op {
            "bridge"
            | "startDocument"
            | "endDocument"
            | "defineParagraphStyle"
            | "defineCharacterStyle" => {}
            "setDocumentMetaData" => {
                if let Some(t) = p.str("dc:title") {
                    self.doc.metadata.title = Some(t.into());
                }
                if let Some(a) = p.str("dc:creator") {
                    self.doc.metadata.author = Some(a.into());
                }
            }
            "defineEmbeddedFont" => self.warn(
                "cdr.font.embedded",
                "fonte incorporada no arquivo ignorada; usando fontes do sistema",
            ),
            "startPage" | "startMasterPage" => {
                self.finish_page();
                let w = p.len_um("svg:width").unwrap_or(210_000.0);
                let h = p.len_um("svg:height").unwrap_or(297_000.0);
                let mut page = Page::new(w, h);
                page.layers = vec![Layer::new("Importado")];
                page.name = p
                    .str("draw:name")
                    .or(p.str("librevenge:name"))
                    .map(String::from);
                self.page_height = h;
                self.in_master = op == "startMasterPage";
                self.page = Some(page);
                self.groups = vec![Vec::new()];
            }
            "endPage" | "endMasterPage" => self.finish_page(),
            "setStyle" => self.style = p.clone(),
            "startLayer" | "openGroup" => self.groups.push(Vec::new()),
            "endLayer" | "closeGroup" => {
                if self.groups.len() > 1 {
                    let children = self.groups.pop().unwrap();
                    if !children.is_empty() {
                        self.push(Object::new(ObjectKind::Group(GroupData {
                            children,
                            boolean: None,
                            symmetry: None,
                            extra: Map::new(),
                        })));
                    }
                }
            }
            "drawPath" => self.draw_path(p),
            "drawPolygon" | "drawPolyline" => self.draw_poly(p, op == "drawPolygon"),
            "drawRectangle" => self.draw_rect(p),
            "drawEllipse" => self.draw_ellipse(p),
            "drawGraphicObject" => self.draw_image(p),
            "drawConnector" => {
                self.warn("cdr.connector", "conector importado como linha simples");
                self.draw_path(p)
            }
            "startTextObject" => self.start_text(p),
            "openParagraph" | "openListElement" => self.open_paragraph(p),
            "closeParagraph" | "closeListElement" => self.close_paragraph(),
            "openSpan" => self.open_span(p),
            "closeSpan" => self.close_span(),
            "insertText" => self.insert_text(text),
            "insertSpace" => self.insert_text(" "),
            "insertTab" => self.insert_text("\t"),
            "insertLineBreak" => self.insert_text("\n"),
            "endTextObject" => self.end_text(),
            "insertField" => self.warn(
                "cdr.text.field",
                "campo de texto dinâmico não suportado; ignorado",
            ),
            "openLink"
            | "closeLink"
            | "openOrderedListLevel"
            | "openUnorderedListLevel"
            | "closeOrderedListLevel"
            | "closeUnorderedListLevel" => {}
            "startEmbeddedGraphics" => {
                self.skip_depth = 1;
                self.warn(
                    "cdr.embedded-graphics",
                    "gráfico incorporado não suportado; omitido",
                );
            }
            "startTableObject" => {
                self.skip_depth = 1;
                self.warn("cdr.table", "tabela não suportada; omitida");
            }
            other => {
                let msg = format!("chamada '{other}' do leitor .cdr não tratada");
                self.warn("cdr.unhandled", &msg);
            }
        }
    }

    fn finish_page(&mut self) {
        let Some(mut page) = self.page.take() else {
            return;
        };
        while self.groups.len() > 1 {
            let children = self.groups.pop().unwrap();
            self.groups
                .last_mut()
                .unwrap()
                .push(Object::new(ObjectKind::Group(GroupData {
                    children,
                    boolean: None,
                    symmetry: None,
                    extra: Map::new(),
                })));
        }
        page.layers[0].objects = self.groups.pop().unwrap_or_default();
        if self.in_master {
            let mut layers = page.layers;
            for l in &mut layers {
                l.name = "Mestra (importada)".into();
            }
            self.doc.data.master_page.layers.extend(layers);
        } else {
            self.doc.data.pages.push(page);
        }
        self.groups.clear();
    }

    fn push(&mut self, o: Object) {
        if self.page.is_none() {
            self.report.warn(
                "cdr.object.outside-page",
                "objeto fora de página ignorado",
                None,
            );
            return;
        }
        if self.groups.is_empty() {
            self.groups.push(Vec::new());
        }
        self.groups.last_mut().unwrap().push(o);
    }

    // ---- estilo ---------------------------------------------------------

    fn fill_from_style(&mut self, bbox: kurbo::Rect) -> Option<Fill> {
        let s = self.style.clone();
        let opacity = s.ratio("draw:opacity").unwrap_or(1.0).clamp(0.0, 1.0);
        let fill = match s.str("draw:fill").unwrap_or("none") {
            "none" => return None,
            "solid" => {
                let color = s
                    .str("draw:fill-color")
                    .and_then(Color::from_hex)
                    .unwrap_or_else(Color::black);
                Fill::Solid {
                    color,
                    opacity,
                    overprint: false,
                    extra: Map::new(),
                }
            }
            "gradient" => {
                let g = self.gradient(&s, bbox, opacity)?;
                Fill::Gradient(g)
            }
            "bitmap" => {
                self.warn(
                    "cdr.fill.pattern",
                    "preenchimento de padrão/bitmap aproximado por cor sólida média",
                );
                let color = s
                    .str("draw:fill-image")
                    .and_then(|b64| {
                        base64::engine::general_purpose::STANDARD
                            .decode(b64.trim())
                            .ok()
                    })
                    .and_then(|bytes| average_color(&bytes))
                    .unwrap_or_else(|| Color::gray(0.5));
                Fill::Solid {
                    color,
                    opacity,
                    overprint: false,
                    extra: Map::new(),
                }
            }
            other => {
                let msg = format!("tipo de preenchimento '{other}' não suportado; omitido");
                self.warn("cdr.fill.unsupported", &msg);
                return None;
            }
        };
        Some(fill)
    }

    /// Gradiente no estilo ODF (como a libcdr emite) → gradiente GraphDraw.
    fn gradient(&mut self, s: &Props, bbox: kurbo::Rect, opacity: f64) -> Option<Gradient> {
        let start_color = s.str("draw:start-color").and_then(Color::from_hex);
        let end_color = s.str("draw:end-color").and_then(Color::from_hex);
        let border = s.ratio("draw:border").unwrap_or(0.0).clamp(0.0, 0.99);
        let mut stops: Vec<GradientStop> = s
            .vec("svg:linearGradient")
            .iter()
            .filter_map(|st| {
                Some(GradientStop {
                    offset: st.ratio("svg:offset").unwrap_or(0.0).clamp(0.0, 1.0),
                    color: Color::from_hex(st.str("svg:stop-color")?)?,
                    opacity: st.ratio("svg:stop-opacity").unwrap_or(1.0).clamp(0.0, 1.0),
                    midpoint: 0.5,
                    extra: Map::new(),
                })
            })
            .collect();
        let stop = |offset: f64, color: Color| GradientStop {
            offset,
            color,
            opacity,
            midpoint: 0.5,
            extra: Map::new(),
        };
        let style = s.str("draw:style").unwrap_or("linear");
        let center = kurbo::Point::new(bbox.x0 + bbox.width() / 2.0, bbox.y0 + bbox.height() / 2.0);
        match style {
            "radial" | "square" | "ellipsoid" | "rectangular" => {
                // ODF: cor inicial na borda, cor final no centro.
                if stops.is_empty() {
                    stops = vec![stop(0.0, end_color?), stop(1.0 - border, start_color?)];
                }
                let cx = bbox.x0 + s.ratio("svg:cx").unwrap_or(0.5) * bbox.width();
                // Y da librevenge cresce para baixo.
                let cy = bbox.y1 - s.ratio("svg:cy").unwrap_or(0.5) * bbox.height();
                let r = [bbox.x0, bbox.x1]
                    .iter()
                    .flat_map(|x| {
                        [bbox.y0, bbox.y1].map(|y| ((x - cx).powi(2) + (y - cy).powi(2)).sqrt())
                    })
                    .fold(0.0, f64::max);
                // Meias medidas até a borda mais longe, em x e em y.
                let hx = (cx - bbox.x0).abs().max((bbox.x1 - cx).abs()).max(1.0);
                let hy = (cy - bbox.y0).abs().max((bbox.y1 - cy).abs()).max(1.0);
                let turn = s.num("draw:angle").filter(|a| *a != 0.0);
                let (kind, radius, radius_y, angle) = match style {
                    // Elipse que passa pelos cantos da caixa.
                    "ellipsoid" => (
                        "elliptic",
                        hx * std::f64::consts::SQRT_2,
                        Some(hy * std::f64::consts::SQRT_2),
                        turn,
                    ),
                    "rectangular" => ("rectangular", hx, Some(hy), turn),
                    "square" => ("rectangular", hx.max(hy), Some(hx.max(hy)), turn),
                    _ => ("radial", r.max(1.0), None, None),
                };
                Some(Gradient {
                    kind: kind.into(),
                    stops,
                    start: None,
                    end: None,
                    center: Some([cx, cy]),
                    radius: Some(radius),
                    radius_y,
                    angle,
                    spread: "pad".into(),
                    steps: None,
                    smooth: false,
                    color_path: "direct".into(),
                    extra: Map::new(),
                })
            }
            other => {
                if !matches!(other, "linear" | "axial") {
                    let msg = format!("gradiente '{other}' aproximado como linear");
                    self.warn("cdr.gradient.approx", &msg);
                }
                if stops.is_empty() {
                    stops = if other == "axial" {
                        // ODF axial: cor inicial nas duas bordas, final no meio.
                        let b = border / 2.0;
                        let (sc, ec) = (start_color?, end_color?);
                        vec![stop(b, sc.clone()), stop(0.5, ec), stop(1.0 - b, sc)]
                    } else {
                        vec![stop(border, start_color?), stop(1.0, end_color?)]
                    };
                }
                // ODF: ângulo 0 = do topo para a base, girando no sentido anti-horário.
                let a = (s.num("draw:angle").unwrap_or(0.0)).to_radians();
                let d = kurbo::Vec2::new(a.sin(), -a.cos());
                let half = [bbox.x0, bbox.x1]
                    .iter()
                    .flat_map(|x| {
                        [bbox.y0, bbox.y1].map(|y| (kurbo::Point::new(*x, y) - center).dot(d).abs())
                    })
                    .fold(0.0, f64::max)
                    .max(1.0);
                let p0 = center - d * half;
                let p1 = center + d * half;
                Some(Gradient {
                    kind: "linear".into(),
                    stops,
                    start: Some([p0.x, p0.y]),
                    end: Some([p1.x, p1.y]),
                    center: None,
                    radius: None,
                    radius_y: None,
                    angle: None,
                    spread: "pad".into(),
                    steps: None,
                    smooth: false,
                    color_path: "direct".into(),
                    extra: Map::new(),
                })
            }
        }
    }

    fn stroke_from_style(&mut self) -> Option<Stroke> {
        let s = self.style.clone();
        let kind = s.str("draw:stroke").unwrap_or("none");
        if kind == "none" {
            return None;
        }
        let width = s.len_um("svg:stroke-width").unwrap_or(0.0).max(0.0);
        let color = s
            .str("svg:stroke-color")
            .and_then(Color::from_hex)
            .unwrap_or_else(Color::black);
        let mut st = Stroke::new(width, color);
        if let Some(o) = s.ratio("svg:stroke-opacity") {
            st.opacity = o.clamp(0.0, 1.0);
        }
        st.cap = match s.str("svg:stroke-linecap") {
            Some("round") => "round",
            Some("square") => "square",
            _ => "butt",
        }
        .into();
        st.join = match s.str("svg:stroke-linejoin") {
            Some("round") => "round",
            Some("bevel") => "bevel",
            _ => "miter",
        }
        .into();
        if kind == "dash" {
            let dist = s.len_um("draw:distance").unwrap_or(width.max(UM_PER_PT));
            let mut dash = Vec::new();
            for (n, l) in [
                ("draw:dots1", "draw:dots1-length"),
                ("draw:dots2", "draw:dots2-length"),
            ] {
                let count = s.int(n).unwrap_or(0).clamp(0, 64);
                let len = s
                    .len_um(l)
                    .filter(|v| *v > 0.0)
                    .unwrap_or(width.max(UM_PER_PT));
                for _ in 0..count {
                    dash.push(len);
                    dash.push(dist);
                }
            }
            if !dash.is_empty() {
                st.dash = Some(dash);
            }
        }
        if s.raw("draw:marker-start-path").is_some() || s.raw("draw:marker-end-path").is_some() {
            self.warn(
                "cdr.stroke.arrows",
                "pontas de seta do contorno ignoradas (previstas para versão futura)",
            );
        }
        Some(st)
    }

    fn styled(&mut self, kind: ObjectKind, bbox: kurbo::Rect) -> Object {
        let mut o = Object::new(kind);
        o.fill = self.fill_from_style(bbox);
        o.stroke = self.stroke_from_style();
        o
    }

    // ---- formas ---------------------------------------------------------

    fn draw_path(&mut self, p: &Props) {
        let mut b = SubpathBuilder::default();
        let mut cur = [0.0, 0.0];
        for el in p.vec("svg:d") {
            let action = el.str("librevenge:path-action").unwrap_or("");
            let pt = self.pt(&el, "svg:x", "svg:y");
            match action {
                "M" | "m" => {
                    if let Some(q) = pt {
                        b.move_to(q);
                        cur = q;
                    }
                }
                "L" | "l" => {
                    if let Some(q) = pt {
                        b.line_to(q);
                        cur = q;
                    }
                }
                "H" | "h" => {
                    if let Some(x) = el.num("svg:x") {
                        cur = [self.x(x), cur[1]];
                        b.line_to(cur);
                    }
                }
                "V" | "v" => {
                    if let Some(y) = el.num("svg:y") {
                        cur = [cur[0], self.y(y)];
                        b.line_to(cur);
                    }
                }
                "C" | "c" => {
                    if let (Some(c1), Some(c2), Some(q)) = (
                        self.pt(&el, "svg:x1", "svg:y1"),
                        self.pt(&el, "svg:x2", "svg:y2"),
                        pt,
                    ) {
                        b.curve_to(c1, c2, q);
                        cur = q;
                    }
                }
                "Q" | "q" => {
                    if let (Some(c), Some(q)) = (self.pt(&el, "svg:x1", "svg:y1"), pt) {
                        b.quad_to(c, q);
                        cur = q;
                    }
                }
                "A" | "a" => {
                    if let Some(q) = pt {
                        self.warn(
                            "cdr.path.arc",
                            "arco elíptico em caminho aproximado por reta",
                        );
                        b.line_to(q);
                        cur = q;
                    }
                }
                "Z" | "z" => b.close(),
                other => {
                    let msg = format!("comando de caminho '{other}' ignorado");
                    self.warn("cdr.path.command", &msg);
                }
            }
        }
        let subpaths = b.finish();
        if subpaths.is_empty() {
            return;
        }
        let fill_rule = match self.style.str("svg:fill-rule") {
            Some("nonzero") => FillRule::Nonzero,
            _ => FillRule::Evenodd,
        };
        let data = PathData {
            subpaths,
            fill_rule,
            extra: Map::new(),
        };
        let bbox = path_bbox(&data);
        let o = self.styled(ObjectKind::Path(data), bbox);
        self.push(o);
    }

    fn draw_poly(&mut self, p: &Props, closed: bool) {
        let mut b = SubpathBuilder::default();
        for (i, el) in p.vec("svg:points").iter().enumerate() {
            if let Some(q) = self.pt(el, "svg:x", "svg:y") {
                if i == 0 { b.move_to(q) } else { b.line_to(q) }
            }
        }
        if closed {
            b.close();
        }
        let data = PathData {
            subpaths: b.finish(),
            fill_rule: FillRule::Evenodd,
            extra: Map::new(),
        };
        if data.subpaths.is_empty() {
            return;
        }
        let bbox = path_bbox(&data);
        let o = self.styled(ObjectKind::Path(data), bbox);
        self.push(o);
    }

    fn draw_rect(&mut self, p: &Props) {
        let (Some(x), Some(y), Some(w), Some(h)) = (
            p.len_um("svg:x"),
            p.len_um("svg:y"),
            p.len_um("svg:width"),
            p.len_um("svg:height"),
        ) else {
            return;
        };
        let r = p.len_um("svg:rx").unwrap_or(0.0);
        let corners = (r > 0.0).then(|| {
            vec![
                crate::model::Corner {
                    kind: "round".into(),
                    radius: r
                };
                4
            ]
        });
        let x0 = x;
        let y0 = self.page_height - y - h;
        let bbox = kurbo::Rect::new(x0, y0, x0 + w, y0 + h);
        let mut o = self.styled(
            ObjectKind::Rectangle(RectData {
                width: w,
                height: h,
                corners,
                extra: Map::new(),
            }),
            bbox,
        );
        o.transform = Transform::translate(x0, y0);
        relocal_fill(&mut o);
        self.push(o);
    }

    fn draw_ellipse(&mut self, p: &Props) {
        let (Some(cx), Some(cy), Some(rx), Some(ry)) = (
            p.len_um("svg:cx"),
            p.len_um("svg:cy"),
            p.len_um("svg:rx"),
            p.len_um("svg:ry"),
        ) else {
            return;
        };
        let cy = self.page_height - cy;
        let bbox = kurbo::Rect::new(cx - rx, cy - ry, cx + rx, cy + ry);
        let mut o = self.styled(
            ObjectKind::Ellipse(EllipseData {
                rx,
                ry,
                mode: "ellipse".into(),
                start_angle: 0.0,
                end_angle: 360.0,
                extra: Map::new(),
            }),
            bbox,
        );
        let rot = p.num("librevenge:rotate").unwrap_or(0.0).to_radians();
        o.transform = Transform::from_affine(Affine::translate((cx, cy)) * Affine::rotate(rot));
        relocal_fill(&mut o);
        self.push(o);
    }

    fn draw_image(&mut self, p: &Props) {
        let (Some(x), Some(y), Some(w), Some(h)) = (
            p.len_um("svg:x"),
            p.len_um("svg:y"),
            p.len_um("svg:width"),
            p.len_um("svg:height"),
        ) else {
            return;
        };
        let mime = p.str("librevenge:mime-type").unwrap_or("");
        let Some(bytes) = p.str("office:binary-data").and_then(|b| {
            base64::engine::general_purpose::STANDARD
                .decode(b.trim())
                .ok()
        }) else {
            self.warn("cdr.image.data", "imagem sem dados; omitida");
            return;
        };
        let (bytes, ext) = match mime {
            "image/png" => (bytes, "png"),
            "image/jpeg" => (bytes, "jpg"),
            "image/svg+xml" => {
                self.warn(
                    "cdr.image.svg",
                    "imagem vetorial incorporada (SVG) não suportada; omitida",
                );
                return;
            }
            _ => match to_png(&bytes) {
                Some(png) => (png, "png"),
                None => {
                    let msg =
                        format!("imagem em formato '{mime}' não pôde ser decodificada; omitida");
                    self.warn("cdr.image.decode", &msg);
                    return;
                }
            },
        };
        let src = self.doc.add_image(bytes, ext);
        let cx = x + w / 2.0;
        let cy = self.page_height - (y + h / 2.0);
        let rot = p.num("librevenge:rotate").unwrap_or(0.0).to_radians();
        let sx = if p.is_true("draw:mirror-horizontal") {
            -1.0
        } else {
            1.0
        };
        let sy = if p.is_true("draw:mirror-vertical") {
            -1.0
        } else {
            1.0
        };
        let m = Affine::translate((cx, cy))
            * Affine::rotate(rot)
            * Affine::scale_non_uniform(sx, sy)
            * Affine::translate((-w / 2.0, -h / 2.0));
        let mut o = Object::new(ObjectKind::Image(ImageData {
            width: w,
            height: h,
            src,
            extra: Map::new(),
        }));
        o.transform = Transform::from_affine(m);
        self.push(o);
    }

    // ---- texto ----------------------------------------------------------

    fn start_text(&mut self, p: &Props) {
        let x = p.len_um("svg:x").unwrap_or(0.0);
        let y = self.page_height - p.len_um("svg:y").unwrap_or(0.0);
        let w = p.len_um("svg:width").unwrap_or(0.0);
        let h = p.len_um("svg:height").unwrap_or(0.0);
        let rot = p.num("librevenge:rotate").unwrap_or(0.0) * PI / 180.0;
        let mut obj = Object::new(ObjectKind::Unknown {
            raw: Value::Null,
            error: None,
        });
        obj.transform = Transform::from_affine(Affine::translate((x, y)) * Affine::rotate(rot));
        let data = TextData {
            text_kind: "artistic".into(),
            frame: (w > 0.0).then_some([w, h]),
            paragraphs: Vec::new(),
            line_spacing: 1.2,
            path: None,
            exclusions: Default::default(),
            extra: Map::new(),
        };
        self.text = Some(TextBuild {
            obj,
            data,
            para: None,
            span: None,
        });
    }

    fn open_paragraph(&mut self, p: &Props) {
        let Some(t) = &mut self.text else { return };
        let align = match p.str("fo:text-align") {
            Some("center") => "center",
            Some("end") | Some("right") => "right",
            Some("justify") => "justify",
            Some("full") => "forceJustify",
            _ => "left",
        };
        t.para = Some(Paragraph {
            align: align.into(),
            spans: Vec::new(),
            hyphenate: false,
            lang: None,
            extra: Map::new(),
        });
    }

    fn close_paragraph(&mut self) {
        self.close_span();
        let Some(t) = &mut self.text else { return };
        if let Some(para) = t.para.take() {
            t.data.paragraphs.push(para);
        }
    }

    fn open_span(&mut self, p: &Props) {
        let Some(t) = &mut self.text else { return };
        if t.para.is_none() {
            t.para = Some(Paragraph {
                align: "left".into(),
                spans: Vec::new(),
                hyphenate: false,
                lang: None,
                extra: Map::new(),
            });
        }
        let size = p.len_um("fo:font-size").unwrap_or(12.0 * UM_PER_PT);
        let weight = match p.str("fo:font-weight") {
            Some("bold") => 700,
            Some(w) => w.parse().unwrap_or(400),
            None => 400,
        };
        t.span = Some(TextSpan {
            text: String::new(),
            font: FontSpec {
                family: p.str("style:font-name").unwrap_or("sans-serif").to_string(),
                size,
                weight,
                italic: p.str("fo:font-style") == Some("italic"),
                extra: Map::new(),
            },
            fill: p.str("fo:color").and_then(Color::from_hex).map(Fill::solid),
            extra: Map::new(),
        });
    }

    fn close_span(&mut self) {
        let Some(t) = &mut self.text else { return };
        if let (Some(span), Some(para)) = (t.span.take(), t.para.as_mut())
            && !span.text.is_empty()
        {
            para.spans.push(span);
        }
    }

    fn insert_text(&mut self, s: &str) {
        let Some(t) = &mut self.text else { return };
        if t.span.is_none() {
            self.open_span(&Props::default());
        }
        if let Some(span) = self.text.as_mut().and_then(|t| t.span.as_mut()) {
            span.text.push_str(s);
        }
    }

    fn end_text(&mut self) {
        self.close_paragraph();
        let Some(t) = self.text.take() else { return };
        if t.data.paragraphs.iter().all(|p| p.spans.is_empty()) {
            return;
        }
        let mut obj = t.obj;
        obj.kind = ObjectKind::Text(t.data);
        self.push(obj);
    }
}

fn path_bbox(p: &PathData) -> kurbo::Rect {
    use kurbo::Shape;
    crate::model::geom::subpaths_to_bez(&p.subpaths).bounding_box()
}

/// Gradientes são calculados em coordenadas de página; leva-os ao espaço
/// local do objeto (inverso da sua matriz).
fn relocal_fill(o: &mut Object) {
    if let Some(Fill::Gradient(g)) = &mut o.fill {
        let inv = o.transform.affine().inverse();
        let map = |p: [f64; 2]| {
            let q = inv * kurbo::Point::new(p[0], p[1]);
            [q.x, q.y]
        };
        g.start = g.start.map(map);
        g.end = g.end.map(map);
        g.center = g.center.map(map);
    }
}

fn to_png(bytes: &[u8]) -> Option<Vec<u8>> {
    let img = image::load_from_memory(bytes).ok()?;
    let mut out = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
        .ok()?;
    Some(out)
}

fn average_color(bytes: &[u8]) -> Option<Color> {
    let img = image::load_from_memory(bytes).ok()?.to_rgb8();
    let n = (img.width() as u64 * img.height() as u64).max(1);
    let mut sum = [0u64; 3];
    for px in img.pixels() {
        for i in 0..3 {
            sum[i] += px[i] as u64;
        }
    }
    Some(Color::rgb(
        sum[0] as f64 / n as f64 / 255.0,
        sum[1] as f64 / n as f64 / 255.0,
        sum[2] as f64 / n as f64 / 255.0,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Fill, ObjectKind};

    fn gradient_of(style: &str) -> (crate::model::Gradient, ImportReport) {
        let len = |n: f64| format!(r#"["{n}in","in",{n}]"#);
        let txt = |v: &str| format!(r#"["{v}"]"#);
        let events = [
            format!(r#"{{"op":"startPage","p":{{"svg:width":{},"svg:height":{}}}}}"#, len(4.0), len(4.0)),
            format!(
                r#"{{"op":"setStyle","p":{{"draw:fill":{},"draw:style":{},"draw:start-color":{},"draw:end-color":{}}}}}"#,
                txt("gradient"),
                txt(style),
                txt("#000000"),
                txt("#ffffff")
            ),
            format!(
                r#"{{"op":"drawRectangle","p":{{"svg:x":{},"svg:y":{},"svg:width":{},"svg:height":{}}}}}"#,
                len(1.0),
                len(1.0),
                len(2.0),
                len(1.0)
            ),
            r#"{"op":"endPage","p":{}}"#.to_string(),
        ]
        .join("
");
        let mut report = ImportReport::new("cdr");
        let doc = map_events(events.as_bytes(), &mut report);
        let o = doc.data.pages[0].layers[0].objects[0].clone();
        assert!(matches!(
            o.kind,
            ObjectKind::Rectangle(_) | ObjectKind::Path(_)
        ));
        match o.fill {
            Some(Fill::Gradient(g)) => (g, report),
            f => panic!("sem gradiente: {f:?}"),
        }
    }

    /// GD-FIL-03 / doc 14: gradientes elipsoide, quadrado, retangular e
    /// axial da libcdr chegam como o tipo certo, sem aviso de aproximação.
    #[test]
    fn cdr_gradient_styles_map_to_kinds() {
        for (style, kind) in [
            ("ellipsoid", "elliptic"),
            ("square", "rectangular"),
            ("rectangular", "rectangular"),
            ("radial", "radial"),
            ("axial", "linear"),
        ] {
            let (g, report) = gradient_of(style);
            assert_eq!(g.kind, kind, "{style}");
            assert!(
                !report.items.iter().any(|i| i.code == "cdr.gradient.approx"),
                "{style}: {:?}",
                report.items
            );
            if style == "axial" {
                assert_eq!(g.stops.len(), 3);
            }
        }
        let (r, _) = gradient_of("rectangular");
        let (hx, hy) = (r.radius.unwrap(), r.radius_y.unwrap());
        assert!((hx - 2.0 * hy).abs() < 1.0, "caixa 2 × 1 pol: {hx} {hy}");
    }
}
