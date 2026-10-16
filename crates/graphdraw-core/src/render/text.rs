//! Diagramação simples de texto (doc 06, subconjunto do M0): seleção de
//! fonte do sistema, substituição de fonte ausente (GD-FNT-02), shaping com
//! rustybuzz (porte Rust do HarfBuzz, GD-TXT-05) e conversão de glifos em
//! curvas (GD-TXT-04).
//!
//! Limitações do M0: sem bidi/hifenização; justificação tratada como
//! alinhamento à esquerda; quebra automática apenas para texto de parágrafo.

use std::sync::OnceLock;

use kurbo::{Affine, BezPath, Point};
use rustybuzz::ttf_parser;

use crate::model::{Fill, TextData};

/// Um trecho já posicionado (coordenadas locais do objeto de texto).
#[derive(Debug, Clone)]
pub struct Run {
    pub text: String,
    /// Posição da linha de base do início do trecho.
    pub x: f64,
    pub y: f64,
    pub width: f64,
    /// Família solicitada no documento.
    pub family: String,
    /// Família efetivamente usada (pode ser substituta).
    pub used_family: String,
    pub size: f64,
    pub weight: u16,
    pub italic: bool,
    pub fill: Option<Fill>,
    /// Glifos em curvas, coordenadas locais.
    pub outline: BezPath,
}

#[derive(Debug, Clone)]
pub struct Line {
    pub runs: Vec<Run>,
}

#[derive(Debug, Clone, Default)]
pub struct TextLayout {
    pub lines: Vec<Line>,
    pub missing_fonts: Vec<String>,
}

struct Fonts {
    db: fontdb::Database,
}

fn fonts() -> &'static Fonts {
    static FONTS: OnceLock<Fonts> = OnceLock::new();
    FONTS.get_or_init(|| {
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        if let Ok(dir) = std::env::var("GRAPHDRAW_FONT_DIR") {
            db.load_fonts_dir(dir);
        }
        Fonts { db }
    })
}

/// Fonte resolvida: (id da face, nome usado, era substituta?).
fn resolve_font(family: &str, weight: u16, italic: bool) -> Option<(fontdb::ID, String, bool)> {
    let db = &fonts().db;
    let style = if italic {
        fontdb::Style::Italic
    } else {
        fontdb::Style::Normal
    };
    let q = |families: &[fontdb::Family]| {
        db.query(&fontdb::Query {
            families,
            weight: fontdb::Weight(weight),
            style,
            ..Default::default()
        })
    };
    if let Some(id) = q(&[fontdb::Family::Name(family)]) {
        return Some((id, family.to_string(), false));
    }
    let fallback = q(&[fontdb::Family::SansSerif])
        .or_else(|| q(&[fontdb::Family::Name("Arial")]))
        .or_else(|| q(&[fontdb::Family::Name("DejaVu Sans")]))
        .or_else(|| q(&[fontdb::Family::Name("Liberation Sans")]))
        .or_else(|| db.faces().next().map(|f| f.id))?;
    let name = db
        .face(fallback)
        .and_then(|f| f.families.first().map(|(n, _)| n.clone()))
        .unwrap_or_default();
    Some((fallback, name, true))
}

/// Família efetivamente usada para `family`; `None` se não houver fonte alguma.
/// O booleano indica substituição (fonte ausente, GD-FNT-02).
pub fn resolve_family(family: &str, weight: u16, italic: bool) -> Option<(String, bool)> {
    resolve_font(family, weight, italic).map(|(_, name, substituted)| (name, substituted))
}

struct Shaped {
    outline: BezPath,
    advance: f64,
    ascent: f64,
}

/// Faz o shaping de `text` e devolve curvas com origem na linha de base em (0,0).
fn shape(id: fontdb::ID, text: &str, size: f64) -> Option<Shaped> {
    fonts().db.with_face_data(id, |data, index| {
        let face = rustybuzz::Face::from_slice(data, index)?;
        let scale = size / face.units_per_em() as f64;
        let mut buffer = rustybuzz::UnicodeBuffer::new();
        buffer.push_str(text);
        let glyphs = rustybuzz::shape(&face, &[], buffer);
        let mut outline = BezPath::new();
        let mut pen = 0.0;
        for (info, pos) in glyphs.glyph_infos().iter().zip(glyphs.glyph_positions()) {
            let gx = pen + pos.x_offset as f64 * scale;
            let gy = pos.y_offset as f64 * scale;
            let mut sink = Sink {
                path: BezPath::new(),
            };
            if face
                .outline_glyph(ttf_parser::GlyphId(info.glyph_id as u16), &mut sink)
                .is_some()
            {
                let mut p = sink.path;
                p.apply_affine(Affine::new([scale, 0.0, 0.0, scale, gx, gy]));
                outline.extend(p);
            }
            pen += pos.x_advance as f64 * scale;
        }
        Some(Shaped {
            outline,
            advance: pen,
            ascent: face.ascender() as f64 * scale,
        })
    })?
}

struct Sink {
    path: BezPath,
}

impl ttf_parser::OutlineBuilder for Sink {
    fn move_to(&mut self, x: f32, y: f32) {
        self.path.move_to(Point::new(x as f64, y as f64));
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.path.line_to(Point::new(x as f64, y as f64));
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.path.quad_to(
            Point::new(x1 as f64, y1 as f64),
            Point::new(x as f64, y as f64),
        );
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.path.curve_to(
            Point::new(x1 as f64, y1 as f64),
            Point::new(x2 as f64, y2 as f64),
            Point::new(x as f64, y as f64),
        );
    }
    fn close(&mut self) {
        self.path.close_path();
    }
}

struct Piece {
    text: String,
    span: usize,
}

/// Diagrama o texto. Origem local = canto superior esquerdo do bloco.
pub fn layout_text(t: &TextData) -> TextLayout {
    let mut out = TextLayout::default();
    let frame_w = t.frame.map(|f| f[0]);
    let mut baseline = 0.0;
    let mut first_line = true;

    for para in &t.paragraphs {
        // Resolve fontes por trecho.
        let fonts: Vec<_> = para
            .spans
            .iter()
            .map(|s| {
                let r = resolve_font(&s.font.family, s.font.weight, s.font.italic);
                if let Some((_, _, true)) = &r {
                    out.missing_fonts.push(s.font.family.clone());
                }
                r
            })
            .collect();

        // Quebra em linhas (só texto de parágrafo com quadro).
        let wrap = t.text_kind == "paragraph" && frame_w.is_some();
        let mut lines: Vec<Vec<Piece>> = vec![Vec::new()];
        let mut line_width = 0.0;
        for (si, span) in para.spans.iter().enumerate() {
            for (li, segment) in span.text.split('\n').enumerate() {
                if li > 0 {
                    lines.push(Vec::new());
                    line_width = 0.0;
                }
                if !wrap {
                    lines.last_mut().unwrap().push(Piece {
                        text: segment.to_string(),
                        span: si,
                    });
                    continue;
                }
                for word in segment.split_inclusive(' ') {
                    let w = fonts[si]
                        .as_ref()
                        .and_then(|(id, _, _)| shape(*id, word, span.font.size))
                        .map(|s| s.advance)
                        .unwrap_or(0.0);
                    if line_width + w > frame_w.unwrap() && line_width > 0.0 {
                        lines.push(Vec::new());
                        line_width = 0.0;
                    }
                    line_width += w;
                    let cur = lines.last_mut().unwrap();
                    match cur.last_mut() {
                        Some(p) if p.span == si => p.text.push_str(word),
                        _ => cur.push(Piece {
                            text: word.to_string(),
                            span: si,
                        }),
                    }
                }
            }
        }

        for pieces in lines {
            let max_size = pieces.iter().map(|p| para.spans[p.span].font.size).fold(
                para.spans.first().map(|s| s.font.size).unwrap_or(0.0),
                f64::max,
            );
            let mut runs = Vec::new();
            let mut pen = 0.0;
            let mut ascent: f64 = 0.0;
            for p in &pieces {
                let span = &para.spans[p.span];
                let Some((id, used, _)) = fonts[p.span].clone() else {
                    continue;
                };
                let shaped = shape(id, &p.text, span.font.size).unwrap_or(Shaped {
                    outline: BezPath::new(),
                    advance: 0.0,
                    ascent: span.font.size * 0.8,
                });
                ascent = ascent.max(shaped.ascent);
                runs.push(Run {
                    text: p.text.clone(),
                    x: pen,
                    y: 0.0,
                    width: shaped.advance,
                    family: span.font.family.clone(),
                    used_family: used,
                    size: span.font.size,
                    weight: span.font.weight,
                    italic: span.font.italic,
                    fill: span.fill.clone(),
                    outline: shaped.outline,
                });
                pen += shaped.advance;
            }
            if first_line {
                baseline = -(if ascent > 0.0 { ascent } else { max_size * 0.8 });
                first_line = false;
            } else {
                baseline -= max_size * t.line_spacing;
            }
            let dx = match (para.align.as_str(), frame_w) {
                ("center", Some(w)) => (w - pen) / 2.0,
                ("right", Some(w)) => w - pen,
                ("center", None) => -pen / 2.0,
                ("right", None) => -pen,
                _ => 0.0,
            };
            for r in &mut runs {
                r.x += dx;
                r.y = baseline;
                r.outline.apply_affine(Affine::translate((r.x, r.y)));
            }
            out.lines.push(Line { runs });
        }
    }
    out.missing_fonts.sort();
    out.missing_fonts.dedup();
    out
}
