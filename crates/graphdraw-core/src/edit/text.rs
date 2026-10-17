//! Edição de texto artístico e de parágrafo (GD-TXT-01/02): inserir e
//! apagar no texto corrido (parágrafos unidos por `\n`, ver
//! [`crate::render::text::plain_text`]) preservando os trechos e seus
//! estilos, e aplicar fonte, corpo, peso, itálico e alinhamento.

use serde_json::Map;

use crate::model::{FontSpec, Object, ObjectKind, Paragraph, TextData, TextSpan, Transform};

/// Corpo padrão de texto novo: 24 pt em µm.
pub const DEFAULT_SIZE: f64 = 24.0 * 25_400.0 / 72.0;

/// Objeto de texto novo, vazio, com origem (canto superior esquerdo) em
/// `(x, y)`. Com `frame`, é texto de parágrafo naquele quadro.
pub fn new_text(x: f64, y: f64, frame: Option<[f64; 2]>, family: &str, size: f64) -> Object {
    let mut o = Object::new(ObjectKind::Text(TextData {
        text_kind: if frame.is_some() {
            "paragraph"
        } else {
            "artistic"
        }
        .into(),
        frame,
        paragraphs: vec![Paragraph {
            align: "left".into(),
            spans: vec![span("", family, size)],
            extra: Map::new(),
        }],
        line_spacing: 1.2,
        extra: Map::new(),
    }));
    o.transform = Transform::translate(x, y);
    o.fill = Some(crate::model::Fill::solid(crate::model::Color::black()));
    o
}

fn span(text: &str, family: &str, size: f64) -> TextSpan {
    TextSpan {
        text: text.into(),
        font: FontSpec {
            family: family.into(),
            size,
            weight: 400,
            italic: false,
            extra: Map::new(),
        },
        fill: None,
        extra: Map::new(),
    }
}

fn para_len(p: &Paragraph) -> usize {
    p.spans.iter().map(|s| s.text.chars().count()).sum()
}

/// Número de caracteres do texto corrido.
pub fn len(t: &TextData) -> usize {
    let n: usize = t.paragraphs.iter().map(para_len).sum();
    n + t.paragraphs.len().saturating_sub(1)
}

/// (parágrafo, caractere no parágrafo) do índice no texto corrido.
fn locate_para(t: &TextData, idx: usize) -> (usize, usize) {
    let mut start = 0;
    for (i, p) in t.paragraphs.iter().enumerate() {
        let n = para_len(p);
        if idx <= start + n {
            return (i, idx - start);
        }
        start += n + 1;
    }
    let last = t.paragraphs.len().saturating_sub(1);
    (last, t.paragraphs.get(last).map_or(0, para_len))
}

/// (trecho, caractere no trecho) dentro do parágrafo; no limite entre dois
/// trechos fica no anterior (digitar continua o estilo de antes do cursor).
fn locate_span(p: &Paragraph, k: usize) -> (usize, usize) {
    let mut start = 0;
    for (i, s) in p.spans.iter().enumerate() {
        let n = s.text.chars().count();
        if k <= start + n {
            return (i, k - start);
        }
        start += n;
    }
    let last = p.spans.len().saturating_sub(1);
    (
        last,
        p.spans.get(last).map_or(0, |s| s.text.chars().count()),
    )
}

fn byte_at(s: &str, chars: usize) -> usize {
    s.char_indices().nth(chars).map_or(s.len(), |(b, _)| b)
}

/// Insere `text` antes do caractere `idx`; `\n` abre parágrafo novo com o
/// mesmo estilo. Devolve o índice logo depois do inserido.
pub fn insert(t: &mut TextData, idx: usize, text: &str) -> usize {
    if t.paragraphs.is_empty() {
        t.paragraphs.push(Paragraph {
            align: "left".into(),
            spans: vec![span("", "Arial", DEFAULT_SIZE)],
            extra: Map::new(),
        });
    }
    let mut at = idx.min(len(t));
    for (i, piece) in text.split('\n').enumerate() {
        if i > 0 {
            split_paragraph(t, at);
            at += 1;
        }
        if piece.is_empty() {
            continue;
        }
        let (p, k) = locate_para(t, at);
        let para = &mut t.paragraphs[p];
        if para.spans.is_empty() {
            para.spans.push(span("", "Arial", DEFAULT_SIZE));
        }
        let (s, c) = locate_span(para, k);
        let sp = &mut para.spans[s].text;
        let b = byte_at(sp, c);
        sp.insert_str(b, piece);
        at += piece.chars().count();
    }
    at
}

fn split_paragraph(t: &mut TextData, idx: usize) {
    let (p, k) = locate_para(t, idx);
    let para = &mut t.paragraphs[p];
    let (s, c) = locate_span(para, k);
    let mut right: Vec<TextSpan> = para.spans.split_off(s + 1);
    let cut = &mut para.spans[s];
    let b = byte_at(&cut.text, c);
    let mut tail = cut.clone();
    tail.text = cut.text.split_off(b);
    right.insert(0, tail);
    let new = Paragraph {
        align: para.align.clone(),
        spans: right,
        extra: para.extra.clone(),
    };
    t.paragraphs.insert(p + 1, new);
}

/// Apaga os caracteres `[a, b)` (juntando parágrafos quando passa por `\n`).
pub fn delete(t: &mut TextData, a: usize, b: usize) {
    let (a, b) = (a.min(b), a.max(b).min(len(t)));
    for _ in a..b {
        delete_char(t, a);
    }
}

fn delete_char(t: &mut TextData, idx: usize) {
    let (p, k) = locate_para(t, idx);
    let n = para_len(&t.paragraphs[p]);
    if k >= n {
        // Fim do parágrafo: junta o seguinte.
        if p + 1 < t.paragraphs.len() {
            let next = t.paragraphs.remove(p + 1);
            let para = &mut t.paragraphs[p];
            para.spans.retain(|s| !s.text.is_empty());
            para.spans.extend(next.spans);
            if para.spans.is_empty() {
                para.spans.push(span("", "Arial", DEFAULT_SIZE));
            }
        }
        return;
    }
    let para = &mut t.paragraphs[p];
    // Caractere k: o trecho onde ele está (não o anterior no limite).
    let mut start = 0;
    for i in 0..para.spans.len() {
        let len = para.spans[i].text.chars().count();
        if k < start + len {
            let s = &mut para.spans[i].text;
            let b = byte_at(s, k - start);
            s.remove(b);
            if s.is_empty() && para.spans.len() > 1 {
                para.spans.remove(i);
            }
            return;
        }
        start += len;
    }
}

/// Atributos de caractere/parágrafo aplicados ao objeto inteiro.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TextStyle {
    pub family: Option<String>,
    pub size: Option<f64>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    /// `left`, `center`, `right`, `justify`.
    pub align: Option<String>,
}

pub fn apply_style(t: &mut TextData, st: &TextStyle) {
    for p in &mut t.paragraphs {
        if let Some(a) = &st.align {
            p.align = a.clone();
        }
        for s in &mut p.spans {
            if let Some(f) = &st.family {
                s.font.family = f.clone();
            }
            if let Some(z) = st.size {
                s.font.size = z;
            }
            if let Some(b) = st.bold {
                s.font.weight = if b { 700 } else { 400 };
            }
            if let Some(i) = st.italic {
                s.font.italic = i;
            }
        }
    }
}

/// Estilo do primeiro trecho (para mostrar na barra de propriedades).
pub fn current_style(t: &TextData) -> (String, f64, bool, bool, String) {
    let p = t.paragraphs.first();
    let s = p.and_then(|p| p.spans.first());
    (
        s.map_or_else(|| "Arial".into(), |s| s.font.family.clone()),
        s.map_or(DEFAULT_SIZE, |s| s.font.size),
        s.is_some_and(|s| s.font.weight >= 600),
        s.is_some_and(|s| s.font.italic),
        p.map_or_else(|| "left".into(), |p| p.align.clone()),
    )
}

/// Texto sem nenhum caractere (sai ao terminar a edição).
pub fn is_empty(t: &TextData) -> bool {
    len(t) == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::text::plain_text;

    fn data(o: &mut Object) -> &mut TextData {
        match &mut o.kind {
            ObjectKind::Text(t) => t,
            _ => unreachable!(),
        }
    }

    #[test]
    fn insert_delete_and_paragraphs() {
        let mut o = new_text(0.0, 0.0, None, "Arial", DEFAULT_SIZE);
        let t = data(&mut o);
        let at = insert(t, 0, "Olá mundo");
        assert_eq!(at, 9);
        insert(t, 3, "\n");
        assert_eq!(plain_text(t), "Olá\n mundo");
        assert_eq!(t.paragraphs.len(), 2);
        delete(t, 3, 5); // \n e o espaço
        assert_eq!(plain_text(t), "Olámundo");
        assert_eq!(t.paragraphs.len(), 1);
        insert(t, 8, "!\nfim");
        assert_eq!(plain_text(t), "Olámundo!\nfim");
        assert_eq!(len(t), 13);
        delete(t, 0, len(t));
        assert!(is_empty(t));
    }

    #[test]
    fn spans_keep_their_style() {
        let mut o = new_text(0.0, 0.0, None, "Arial", 1_000.0);
        let t = data(&mut o);
        insert(t, 0, "ab");
        let mut bold = t.paragraphs[0].spans[0].clone();
        bold.text = "CD".into();
        bold.font.weight = 700;
        t.paragraphs[0].spans.push(bold);
        // Digitar no fim do primeiro trecho continua o estilo dele.
        insert(t, 2, "x");
        assert_eq!(t.paragraphs[0].spans[0].text, "abx");
        assert_eq!(t.paragraphs[0].spans[1].text, "CD");
        delete(t, 3, 4); // "C"
        assert_eq!(t.paragraphs[0].spans[1].text, "D");
        apply_style(
            t,
            &TextStyle {
                size: Some(2_000.0),
                align: Some("center".into()),
                ..Default::default()
            },
        );
        assert!(t.paragraphs[0].spans.iter().all(|s| s.font.size == 2_000.0));
        assert_eq!(current_style(t).4, "center");
    }
}
