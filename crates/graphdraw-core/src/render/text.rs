//! Diagramação simples de texto (doc 06, subconjunto do M0): seleção de
//! fonte do sistema, substituição de fonte ausente (GD-FNT-02), shaping com
//! rustybuzz (porte Rust do HarfBuzz, GD-TXT-05) e conversão de glifos em
//! curvas (GD-TXT-04).
//!
//! Parágrafo: quebra automática no quadro, justificado (o espaço extra vai
//! para os espaços entre palavras) e indicação de texto excedente. Sem
//! bidi nem hifenização.

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
    /// Face usada e glifos posicionados (texto vivo no PDF).
    pub font: fontdb::ID,
    pub glyphs: Vec<GlyphPos>,
    /// Contorno de cada glifo (mesma ordem de `glyphs`), para o texto em
    /// caminho reposicionar um a um.
    pub glyph_outlines: Vec<BezPath>,
}

#[derive(Debug, Clone)]
pub struct Line {
    pub runs: Vec<Run>,
    /// Caracteres [start, end) do texto corrido nesta linha.
    pub start: usize,
    pub end: usize,
    pub baseline: f64,
    /// Maior corpo da linha (altura do cursor).
    pub size: f64,
    /// x de cada fronteira de caractere (índice no texto corrido).
    pub carets: Vec<(usize, f64)>,
}

#[derive(Debug, Clone, Default)]
pub struct TextLayout {
    pub lines: Vec<Line>,
    pub missing_fonts: Vec<String>,
    /// Texto de parágrafo que não coube no quadro (GD-TXT-02).
    pub overflow: bool,
    /// Glifos colocados num caminho (GD-TXT-09): no PDF/SVG saem em curvas.
    pub on_path: bool,
}

fn font_lock() -> &'static std::sync::RwLock<fontdb::Database> {
    static FONTS: OnceLock<std::sync::RwLock<fontdb::Database>> = OnceLock::new();
    FONTS.get_or_init(|| {
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        if let Ok(dir) = std::env::var("GRAPHDRAW_FONT_DIR") {
            db.load_fonts_dir(dir);
        }
        std::sync::RwLock::new(db)
    })
}

/// Banco de fontes (as do sistema e as incorporadas nos documentos abertos).
fn fonts() -> std::sync::RwLockReadGuard<'static, fontdb::Database> {
    font_lock().read().unwrap_or_else(|e| e.into_inner())
}

/// Fontes incorporadas já registradas (pelo hash), para não repetir.
fn registered() -> &'static std::sync::Mutex<std::collections::HashSet<String>> {
    static R: OnceLock<std::sync::Mutex<std::collections::HashSet<String>>> = OnceLock::new();
    R.get_or_init(Default::default)
}

/// Registra uma fonte incorporada no documento (GD-FNT-03); `key` é o nome
/// do membro (com o hash). Devolve as famílias que ela trouxe.
pub fn register_font(key: &str, data: Vec<u8>) -> Vec<String> {
    if let Ok(mut r) = registered().lock()
        && !r.insert(key.to_string())
    {
        return Vec::new();
    }
    let mut db = font_lock().write().unwrap_or_else(|e| e.into_inner());
    let before: std::collections::HashSet<fontdb::ID> = db.faces().map(|f| f.id).collect();
    db.load_font_data(data);
    db.faces()
        .filter(|f| !before.contains(&f.id))
        .filter_map(|f| f.families.first().map(|(n, _)| n.clone()))
        .collect()
}

/// Permissão de incorporação (fsType do OS/2): pode ir inteira para o
/// arquivo se for instalável ou editável (sem OS/2, instalável).
pub fn embeddable(data: &[u8], index: u32) -> bool {
    match ttf_parser::Face::parse(data, index)
        .ok()
        .and_then(|f| f.permissions())
    {
        None => true,
        Some(p) => matches!(
            p,
            ttf_parser::Permissions::Installable | ttf_parser::Permissions::Editable
        ),
    }
}

/// Arquivo da fonte (inteiro) da família/estilo pedidos, se instalada sem
/// substituição: bytes e extensão.
pub fn font_file(family: &str, weight: u16, italic: bool) -> Option<(Vec<u8>, &'static str, bool)> {
    let (id, _, substituted) = resolve_font(family, weight, italic)?;
    if substituted {
        return None;
    }
    fonts().with_face_data(id, |data, index| {
        let ext = if data.starts_with(b"ttcf") {
            "ttc"
        } else if data.starts_with(b"OTTO") {
            "otf"
        } else {
            "ttf"
        };
        (data.to_vec(), ext, embeddable(data, index))
    })
}

/// Fonte resolvida: (id da face, nome usado, era substituta?).
fn resolve_font(family: &str, weight: u16, italic: bool) -> Option<(fontdb::ID, String, bool)> {
    let db = fonts();
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

/// Glifo já posicionado (coordenadas locais do objeto de texto).
#[derive(Debug, Clone, PartialEq)]
pub struct GlyphPos {
    pub id: u16,
    pub x: f64,
    pub y: f64,
    /// Avanço horizontal.
    pub advance: f64,
    /// Caracteres que o glifo representa (vazio nos glifos seguintes de
    /// um mesmo grupo); vai para o ToUnicode do PDF.
    pub text: String,
}

struct ShapedGlyph {
    id: u16,
    x: f64,
    y: f64,
    advance: f64,
    /// Byte do texto do trecho onde o glifo começa.
    cluster: usize,
    outline: BezPath,
}

struct Shaped {
    glyphs: Vec<ShapedGlyph>,
    advance: f64,
    ascent: f64,
}

/// Faz o shaping de `text`; glifos com origem na linha de base em (0,0).
fn shape(id: fontdb::ID, text: &str, size: f64) -> Option<Shaped> {
    fonts().with_face_data(id, |data, index| {
        let face = rustybuzz::Face::from_slice(data, index)?;
        let scale = size / face.units_per_em() as f64;
        let mut buffer = rustybuzz::UnicodeBuffer::new();
        buffer.push_str(text);
        let shaped = rustybuzz::shape(&face, &[], buffer);
        let mut glyphs = Vec::new();
        let mut pen = 0.0;
        for (info, pos) in shaped.glyph_infos().iter().zip(shaped.glyph_positions()) {
            let gx = pen + pos.x_offset as f64 * scale;
            let gy = pos.y_offset as f64 * scale;
            let mut sink = Sink {
                path: BezPath::new(),
            };
            let gid = info.glyph_id as u16;
            let mut outline = BezPath::new();
            if face
                .outline_glyph(ttf_parser::GlyphId(gid), &mut sink)
                .is_some()
            {
                outline = sink.path;
                outline.apply_affine(Affine::new([scale, 0.0, 0.0, scale, gx, gy]));
            }
            let advance = pos.x_advance as f64 * scale;
            glyphs.push(ShapedGlyph {
                id: gid,
                x: gx,
                y: gy,
                advance,
                cluster: info.cluster as usize,
                outline,
            });
            pen += advance;
        }
        Some(Shaped {
            glyphs,
            advance: pen,
            ascent: face.ascender() as f64 * scale,
        })
    })?
}

/// Contorno de `text` na família pedida (sem substituir), com corpo
/// `size`, linha de base em y = 0 e Y para cima; devolve também a ascendente.
pub fn family_outline(family: &str, text: &str, size: f64) -> Option<(BezPath, f64)> {
    let (id, _, substituted) = resolve_font(family, 400, false)?;
    if substituted {
        return None;
    }
    let s = shape(id, text, size)?;
    let mut out = BezPath::new();
    for g in s.glyphs {
        out.extend(g.outline);
    }
    Some((out, s.ascent))
}

/// Estilos instalados de uma família: (peso, itálico), sem repetição, do
/// mais leve ao mais pesado e o reto antes do itálico.
pub fn font_styles(family: &str) -> Vec<(u16, bool)> {
    let mut v: Vec<(u16, bool)> = fonts()
        .faces()
        .filter(|f| {
            f.families
                .iter()
                .any(|(n, _)| n.eq_ignore_ascii_case(family))
        })
        .map(|f| (f.weight.0, f.style != fontdb::Style::Normal))
        .collect();
    v.sort_unstable();
    v.dedup();
    v
}

/// Nome do estilo para a interface ("Regular", "Bold Italic"…).
pub fn style_name(weight: u16, italic: bool) -> String {
    let w = match weight {
        0..=150 => "Thin",
        151..=250 => "ExtraLight",
        251..=350 => "Light",
        351..=450 => "Regular",
        451..=550 => "Medium",
        551..=650 => "SemiBold",
        651..=750 => "Bold",
        751..=850 => "ExtraBold",
        _ => "Black",
    };
    match (w, italic) {
        ("Regular", true) => "Italic".into(),
        (w, true) => format!("{w} Italic"),
        (w, false) => w.into(),
    }
}

/// Famílias de fonte instaladas, em ordem alfabética (GD-FNT-01).
pub fn font_families() -> Vec<String> {
    let mut v: Vec<String> = fonts()
        .faces()
        .filter_map(|f| f.families.first().map(|(n, _)| n.clone()))
        .collect();
    v.sort_by_key(|a| a.to_lowercase());
    v.dedup();
    v
}

/// Dados da face da fonte usada num trecho (para incorporar no PDF).
pub fn with_font_data<R>(id: fontdb::ID, f: impl FnOnce(&[u8], u32) -> R) -> Option<R> {
    fonts().with_face_data(id, f)
}

/// Nome PostScript da face (ou a família), para o PDF.
pub fn font_postscript_name(id: fontdb::ID) -> String {
    fonts()
        .face(id)
        .map(|f| {
            if f.post_script_name.is_empty() {
                f.families
                    .first()
                    .map(|(n, _)| n.replace(' ', ""))
                    .unwrap_or_default()
            } else {
                f.post_script_name.clone()
            }
        })
        .unwrap_or_else(|| "Fonte".into())
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

/// Pedaço de linha de um mesmo trecho; `start` = índice (em caracteres) no
/// texto corrido do objeto (ver [`plain_text`]).
struct Piece {
    text: String,
    span: usize,
    start: usize,
}

/// Texto corrido do objeto: parágrafos unidos por `\n`. É a base dos índices
/// de caractere do cursor de edição.
pub fn plain_text(t: &TextData) -> String {
    t.paragraphs
        .iter()
        .map(|p| p.spans.iter().map(|s| s.text.as_str()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Diagrama o texto. Origem local = canto superior esquerdo do bloco.
pub fn layout_text(t: &TextData) -> TextLayout {
    let mut out = TextLayout::default();
    let frame_w = t.frame.map(|f| f[0]);
    let frame_h = t.frame.map(|f| f[1]);
    let wrap = t.text_kind == "paragraph" && frame_w.is_some();
    // Texto contornando objetos (GD-TXT-10): a posição de cada linha (linha
    // de base, início, largura) sai na quebra, pelos trechos livres.
    let flow = wrap && !t.exclusions.0.is_empty();
    let mut flow_base: Option<f64> = None;
    let mut flow_slots: std::collections::VecDeque<(f64, f64)> = Default::default();
    let mut next_slot = |size: f64, clear: bool| -> Option<(f64, f64, f64)> {
        if !flow {
            return None;
        }
        if clear {
            flow_slots.clear();
        }
        let (w, h) = (frame_w.unwrap_or(0.0), frame_h.unwrap_or(f64::INFINITY));
        for _ in 0..10_000 {
            if let Some((a, b)) = flow_slots.pop_front() {
                return Some((flow_base.unwrap_or(0.0), a, b - a));
            }
            let b = match flow_base {
                None => -size * 0.8,
                Some(b) => b - size * t.line_spacing,
            };
            flow_base = Some(b);
            if b < -h - size {
                return Some((b, 0.0, w));
            }
            flow_slots.extend(crate::edit::textwrap::free_slots(
                &t.exclusions.0,
                w,
                b - size * 0.25,
                b + size * 0.8,
                size * 1.5,
            ));
        }
        Some((flow_base.unwrap_or(0.0), 0.0, w))
    };
    let mut baseline = 0.0;
    let mut first_line = true;
    // Índice do primeiro caractere do parágrafo no texto corrido.
    let mut para_start = 0usize;

    for para in &t.paragraphs {
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

        // Quebra em linhas; `hard` = a linha termina por \n ou fim de parágrafo;
        // a última parte é a posição fixa no fluxo em volta de objetos.
        type Slot = Option<(f64, f64, f64)>;
        let fsz = para.spans.first().map(|s| s.font.size).unwrap_or(0.0);
        let mut lines: Vec<(Vec<Piece>, usize, bool, Slot)> =
            vec![(Vec::new(), para_start, false, next_slot(fsz, true))];
        let mut line_width = 0.0;
        let mut cursor = para_start;
        for (si, span) in para.spans.iter().enumerate() {
            for (li, segment) in span.text.split('\n').enumerate() {
                if li > 0 {
                    lines.last_mut().unwrap().2 = true;
                    cursor += 1; // o \n
                    lines.push((Vec::new(), cursor, false, next_slot(fsz, true)));
                    line_width = 0.0;
                }
                let words: Vec<&str> = if wrap {
                    segment.split_inclusive(' ').collect()
                } else {
                    vec![segment]
                };
                for word in words {
                    if word.is_empty() {
                        continue;
                    }
                    if wrap {
                        let w = fonts[si]
                            .as_ref()
                            .and_then(|(id, _, _)| shape(*id, word.trim_end(), span.font.size))
                            .map(|s| s.advance)
                            .unwrap_or(0.0);
                        let avail = lines.last().unwrap().3.map_or(frame_w.unwrap(), |s| s.2);
                        if line_width + w > avail && line_width > 0.0 {
                            lines.push((Vec::new(), cursor, false, next_slot(fsz, false)));
                            line_width = 0.0;
                        }
                        let full = fonts[si]
                            .as_ref()
                            .and_then(|(id, _, _)| shape(*id, word, span.font.size))
                            .map(|s| s.advance)
                            .unwrap_or(0.0);
                        line_width += full;
                    }
                    let cur = &mut lines.last_mut().unwrap().0;
                    match cur.last_mut() {
                        Some(p) if p.span == si => p.text.push_str(word),
                        _ => cur.push(Piece {
                            text: word.to_string(),
                            span: si,
                            start: cursor,
                        }),
                    }
                    cursor += word.chars().count();
                }
            }
        }
        lines.last_mut().unwrap().2 = true;

        for (pieces, line_start, hard, slot) in lines {
            let first_size = para.spans.first().map(|s| s.font.size).unwrap_or(0.0);
            let max_size = pieces
                .iter()
                .map(|p| para.spans[p.span].font.size)
                .fold(first_size, f64::max);
            let mut runs = Vec::new();
            let mut pen = 0.0;
            let mut ascent: f64 = 0.0;
            // Fronteiras de caractere: (índice no texto corrido, x sem justificar).
            let mut bounds: Vec<(usize, f64)> = Vec::new();
            let mut shaped_runs = Vec::new();
            for p in &pieces {
                let span = &para.spans[p.span];
                let Some((id, used, _)) = fonts[p.span].clone() else {
                    continue;
                };
                let shaped = shape(id, &p.text, span.font.size).unwrap_or(Shaped {
                    glyphs: Vec::new(),
                    advance: 0.0,
                    ascent: span.font.size * 0.8,
                });
                ascent = ascent.max(shaped.ascent);
                for (k, (byte, _)) in p.text.char_indices().enumerate() {
                    let x = shaped
                        .glyphs
                        .iter()
                        .find(|g| g.cluster >= byte)
                        .map_or(shaped.advance, |g| g.x);
                    bounds.push((p.start + k, pen + x));
                }
                shaped_runs.push((p, span, id, used, pen, shaped));
                pen += shaped_runs.last().unwrap().5.advance;
            }
            let line_end = pieces
                .last()
                .map_or(line_start, |p| p.start + p.text.chars().count());
            bounds.push((line_end, pen));
            if first_line {
                baseline = -(if ascent > 0.0 { ascent } else { max_size * 0.8 });
                first_line = false;
            } else {
                baseline -= max_size * t.line_spacing;
            }
            if let Some((b, _, _)) = slot {
                baseline = b;
            }
            let line_w = slot.map(|s| s.2).or(frame_w);
            let x0 = slot.map_or(0.0, |s| s.1);
            // Justificar: espaço extra repartido entre os espaços da linha
            // (não na última linha do parágrafo nem antes de \n).
            let line_text: String = pieces.iter().map(|p| p.text.as_str()).collect();
            let trailing = line_text.len() - line_text.trim_end().len();
            let content_w = if trailing > 0 {
                // Largura sem os espaços do fim.
                bounds.iter().rev().nth(trailing).map_or(pen, |b| b.1)
            } else {
                pen
            };
            let justify = matches!(para.align.as_str(), "justify" | "forceJustify")
                && wrap
                && (!hard || para.align == "forceJustify");
            let spaces = line_text.trim_end().matches(' ').count();
            let extra = match (justify, line_w) {
                (true, Some(w)) if spaces > 0 && w > content_w => (w - content_w) / spaces as f64,
                _ => 0.0,
            };
            let dx = x0
                + match (para.align.as_str(), line_w) {
                    ("center", Some(w)) => (w - content_w) / 2.0,
                    ("right", Some(w)) => w - content_w,
                    ("center", None) => -content_w / 2.0,
                    ("right", None) => -content_w,
                    _ => 0.0,
                };
            // Espaços antes de cada caractere da linha (para o deslocamento).
            let chars: Vec<char> = line_text.chars().collect();
            let spaces_before = |idx: usize| -> f64 {
                let k = idx.saturating_sub(line_start).min(chars.len());
                chars[..k].iter().filter(|c| **c == ' ').count().min(spaces) as f64
            };
            for (p, span, id, used, run_x, shaped) in shaped_runs {
                let mut outline = BezPath::new();
                let mut glyphs = Vec::new();
                let mut glyph_outlines = Vec::new();
                let mut clusters: Vec<usize> = shaped.glyphs.iter().map(|g| g.cluster).collect();
                clusters.sort_unstable();
                clusters.dedup();
                let mut seen = std::collections::HashSet::new();
                for g in &shaped.glyphs {
                    let next = clusters
                        .iter()
                        .find(|&&c| c > g.cluster)
                        .copied()
                        .unwrap_or(p.text.len());
                    let text = if seen.insert(g.cluster) {
                        p.text.get(g.cluster..next).unwrap_or("").to_string()
                    } else {
                        String::new()
                    };
                    let char_idx = p.start + p.text[..g.cluster.min(p.text.len())].chars().count();
                    let shift = dx + run_x + extra * spaces_before(char_idx);
                    let mut o = g.outline.clone();
                    o.apply_affine(Affine::translate((shift, baseline)));
                    glyph_outlines.push(o.clone());
                    outline.extend(o);
                    glyphs.push(GlyphPos {
                        id: g.id,
                        x: shift + g.x,
                        y: baseline + g.y,
                        advance: g.advance,
                        text,
                    });
                }
                runs.push(Run {
                    text: p.text.clone(),
                    x: dx + run_x,
                    y: baseline,
                    width: shaped.advance,
                    family: span.font.family.clone(),
                    used_family: used,
                    size: span.font.size,
                    weight: span.font.weight,
                    italic: span.font.italic,
                    fill: span.fill.clone(),
                    outline,
                    font: id,
                    glyphs,
                    glyph_outlines,
                });
            }
            let carets = bounds
                .iter()
                .map(|(i, x)| (*i, dx + x + extra * spaces_before(*i)))
                .collect();
            // Quadro de parágrafo: linhas que passam do fundo ficam de fora.
            if let Some(h) = frame_h
                && wrap
                && baseline < -h
            {
                out.overflow = true;
                continue;
            }
            out.lines.push(Line {
                runs,
                start: line_start,
                end: line_end,
                baseline,
                size: max_size,
                carets,
            });
        }
        para_start = cursor + 1;
    }
    out.missing_fonts.sort();
    out.missing_fonts.dedup();
    if let Some(tp) = &t.path {
        place_on_path(&mut out, tp);
    }
    out
}

/// Polilinha do primeiro subcaminho com o comprimento acumulado.
struct Track {
    pts: Vec<kurbo::Point>,
    len: Vec<f64>,
}

impl Track {
    fn new(path: &BezPath, tol: f64) -> Option<Track> {
        let mut pts: Vec<kurbo::Point> = Vec::new();
        let mut started = false;
        let mut done = false;
        kurbo::flatten(path.iter(), tol, |el| {
            if done {
                return;
            }
            match el {
                kurbo::PathEl::MoveTo(p) => {
                    if started {
                        done = true;
                    } else {
                        started = true;
                        pts.push(p);
                    }
                }
                kurbo::PathEl::LineTo(p) => pts.push(p),
                kurbo::PathEl::ClosePath => {
                    if let Some(f) = pts.first().copied() {
                        pts.push(f);
                    }
                    done = true;
                }
                _ => {}
            }
        });
        pts.dedup_by(|a, b| (*a - *b).hypot() < 1e-9);
        if pts.len() < 2 {
            return None;
        }
        let mut len = vec![0.0];
        for w in pts.windows(2) {
            len.push(len.last().unwrap() + (w[1] - w[0]).hypot());
        }
        Some(Track { pts, len })
    }

    /// Ponto e ângulo da tangente no comprimento `s` (fora das pontas,
    /// segue reto pela tangente da ponta).
    fn at(&self, s: f64) -> (kurbo::Point, f64) {
        let n = self.pts.len();
        let total = self.len[n - 1];
        let seg = if s <= 0.0 {
            0
        } else if s >= total {
            n - 2
        } else {
            self.len
                .partition_point(|l| *l <= s)
                .saturating_sub(1)
                .min(n - 2)
        };
        let (a, b) = (self.pts[seg], self.pts[seg + 1]);
        let d = b - a;
        let t = (s - self.len[seg]) / d.hypot().max(1e-12);
        (a + d * t, d.y.atan2(d.x))
    }
}

/// Texto em caminho (GD-TXT-09): cada glifo vai para o ponto do caminho no
/// meio do seu avanço, com a orientação pedida; linhas seguintes ficam
/// paralelas, afastadas pela entrelinha.
fn place_on_path(out: &mut TextLayout, tp: &crate::model::TextPath) {
    let mut path = crate::model::geom::subpaths_to_bez(&tp.subpaths);
    if tp.mirror {
        path = path.reverse_subpaths();
    }
    let size = out.lines.first().map_or(1000.0, |l| l.size);
    let Some(track) = Track::new(&path, (size / 400.0).max(0.5)) else {
        return;
    };
    let b0 = out.lines.first().map_or(0.0, |l| l.baseline);
    for line in &mut out.lines {
        for run in &mut line.runs {
            let mut outline = BezPath::new();
            for (g, go) in run.glyphs.iter_mut().zip(&run.glyph_outlines) {
                let cx = g.x + g.advance / 2.0;
                let (p, a) = track.at(tp.offset + cx);
                let normal = kurbo::Vec2::new(-a.sin(), a.cos());
                let below = if tp.side == "below" {
                    -run.size * 0.75
                } else {
                    0.0
                };
                let base = p + normal * (tp.distance + below + (line.baseline - b0));
                let tan = a.tan().clamp(-10.0, 10.0);
                let orient = match tp.orientation.as_str() {
                    "upright" => Affine::IDENTITY,
                    "skew-v" => Affine::new([1.0, tan, 0.0, 1.0, 0.0, 0.0]),
                    "skew-h" => Affine::new([1.0, 0.0, -tan, 1.0, 0.0, 0.0]),
                    _ => Affine::rotate(a),
                };
                let m = Affine::translate(base.to_vec2())
                    * orient
                    * Affine::translate((-cx, -line.baseline));
                let mut o = go.clone();
                o.apply_affine(m);
                outline.extend(o);
                let q = m * kurbo::Point::new(g.x, line.baseline);
                g.x = q.x;
                g.y = q.y;
            }
            run.outline = outline;
        }
    }
    out.on_path = true;
}

impl TextLayout {
    /// Posição do cursor antes do caractere `idx` do texto corrido:
    /// (x, linha de base, corpo), em coordenadas locais.
    pub fn caret(&self, idx: usize) -> Option<(f64, f64, f64)> {
        let line = self
            .lines
            .iter()
            .find(|l| idx >= l.start && idx <= l.end)
            .or_else(|| self.lines.last())?;
        let x = line
            .carets
            .iter()
            .find(|(i, _)| *i == idx)
            .or_else(|| line.carets.last())
            .map_or(0.0, |c| c.1);
        Some((x, line.baseline, line.size))
    }

    /// Caractere mais próximo de um ponto local (clique no texto).
    pub fn index_at(&self, p: Point) -> usize {
        let Some(line) = self.lines.iter().min_by(|a, b| {
            let da = (a.baseline + a.size * 0.3 - p.y).abs();
            let db = (b.baseline + b.size * 0.3 - p.y).abs();
            da.total_cmp(&db)
        }) else {
            return 0;
        };
        line.carets
            .iter()
            .min_by(|a, b| (a.1 - p.x).abs().total_cmp(&(b.1 - p.x).abs()))
            .map_or(line.start, |c| c.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{FontSpec, Paragraph, TextSpan};
    use serde_json::Map;

    fn text(kind: &str, align: &str, s: &str, frame: Option<[f64; 2]>) -> TextData {
        TextData {
            text_kind: kind.into(),
            frame,
            paragraphs: s
                .split('\n')
                .map(|p| Paragraph {
                    align: align.into(),
                    spans: vec![TextSpan {
                        text: p.into(),
                        font: FontSpec {
                            family: "Arial".into(),
                            size: 10_000.0,
                            weight: 400,
                            italic: false,
                            extra: Map::new(),
                        },
                        fill: None,
                        extra: Map::new(),
                    }],
                    extra: Map::new(),
                })
                .collect(),
            line_spacing: 1.2,
            path: None,
            exclusions: Default::default(),
            extra: Map::new(),
        }
    }

    fn has_font() -> bool {
        resolve_font("Arial", 400, false).is_some()
    }

    #[test]
    fn carets_cover_every_character_and_paragraphs() {
        if !has_font() {
            return;
        }
        let t = text("artistic", "left", "ab\ncd", None);
        assert_eq!(plain_text(&t), "ab\ncd");
        let l = layout_text(&t);
        assert_eq!(l.lines.len(), 2);
        assert_eq!((l.lines[0].start, l.lines[0].end), (0, 2));
        assert_eq!((l.lines[1].start, l.lines[1].end), (3, 5));
        let (x0, b0, _) = l.caret(0).unwrap();
        let (x2, _, _) = l.caret(2).unwrap();
        let (x3, b3, _) = l.caret(3).unwrap();
        assert!(x0 == 0.0 && x2 > 0.0 && x3 == 0.0 && b3 < b0);
        assert_eq!(l.index_at(Point::new(x2 + 1.0, b0)), 2);
        // Glifos guardados para o PDF.
        assert_eq!(l.lines[0].runs[0].glyphs.len(), 2);
    }

    #[test]
    fn wrap_flows_around_zone() {
        if !has_font() {
            return;
        }
        let words = "palavra ".repeat(80);
        let w = 100_000.0;
        let mut t = text("paragraph", "left", &words, Some([w, 200_000.0]));
        let plain = layout_text(&t);
        assert!(
            plain
                .lines
                .iter()
                .all(|l| l.runs.first().is_none_or(|r| r.x.abs() < 1e-6))
        );
        // Faixa no meio, de lado a lado à esquerda: linhas ali começam depois.
        t.exclusions.0 = vec![crate::model::WrapZone {
            polys: vec![vec![
                [0.0, -40_000.0],
                [50_000.0, -40_000.0],
                [50_000.0, -20_000.0],
                [0.0, -20_000.0],
            ]],
            side: "both".into(),
            distance: 1_000.0,
        }];
        let l = layout_text(&t);
        let inside: Vec<_> = l
            .lines
            .iter()
            .filter(|l| l.baseline < -22_000.0 && l.baseline > -38_000.0)
            .collect();
        assert!(!inside.is_empty());
        for line in inside {
            assert!(line.runs[0].x >= 50_000.0, "{}", line.runs[0].x);
        }
        assert!(
            l.lines.len() > plain.lines.len(),
            "mais linhas por causa do desvio"
        );
    }

    #[test]
    fn justify_fills_the_frame_and_overflow_is_flagged() {
        if !has_font() {
            return;
        }
        let words = "um dois tres quatro cinco seis sete oito nove dez onze doze";
        let w = 60_000.0;
        let t = text("paragraph", "justify", words, Some([w, 1_000_000.0]));
        let l = layout_text(&t);
        assert!(l.lines.len() >= 2);
        // Linhas cheias terminam na borda do quadro; a última, não.
        let end = |line: &Line| {
            line.runs
                .iter()
                .flat_map(|r| r.glyphs.iter())
                .filter(|g| g.advance > 0.0)
                .map(|g| g.x + g.advance)
                .fold(0.0, f64::max)
        };
        let first = &l.lines[0];
        let trimmed_end = first.carets.iter().rev().find(|c| c.1 < w + 1.0).unwrap().1;
        assert!(
            (end(first) - w).abs() < 50.0 || (trimmed_end - w).abs() < 50.0,
            "{}",
            end(first)
        );
        assert!(end(l.lines.last().unwrap()) < w - 1_000.0);
        assert!(!l.overflow);
        // Quadro baixo: só cabe uma linha.
        let short = layout_text(&text("paragraph", "left", words, Some([w, 12_000.0])));
        assert!(short.overflow);
        assert_eq!(short.lines.len(), 1);
    }
}
