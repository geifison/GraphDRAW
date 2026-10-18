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
            hyphenate: false,
            lang: None,
            extra: Map::new(),
        }],
        line_spacing: 1.2,
        path: None,
        exclusions: Default::default(),
        path_memory: None,
        tracking: 0.0,
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
            hyphenate: false,
            lang: None,
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
        hyphenate: false,
        lang: None,
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
    /// Peso exato (100–900), vindo dos estilos que a fonte tem.
    pub weight: Option<u16>,
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
            if let Some(w) = st.weight {
                s.font.weight = w;
            }
            if let Some(i) = st.italic {
                s.font.italic = i;
            }
        }
    }
}

/// Garante um limite de trecho no índice `idx` do texto corrido.
fn cut_at(t: &mut TextData, idx: usize) {
    let (p, k) = locate_para(t, idx);
    let Some(para) = t.paragraphs.get_mut(p) else {
        return;
    };
    let mut start = 0;
    for i in 0..para.spans.len() {
        let n = para.spans[i].text.chars().count();
        if k > start && k < start + n {
            let b = byte_at(&para.spans[i].text, k - start);
            let mut tail = para.spans[i].clone();
            tail.text = para.spans[i].text.split_off(b);
            para.spans.insert(i + 1, tail);
            return;
        }
        start += n;
    }
}

/// Junta trechos vizinhos de mesmo estilo e tira os vazios que sobram.
fn merge_spans(t: &mut TextData) {
    for p in &mut t.paragraphs {
        let mut out: Vec<TextSpan> = Vec::with_capacity(p.spans.len());
        for s in p.spans.drain(..) {
            match out.last_mut() {
                Some(l) if l.font == s.font && l.fill == s.fill && l.extra == s.extra => {
                    l.text.push_str(&s.text)
                }
                Some(_) if s.text.is_empty() => {}
                Some(l) if l.text.is_empty() => *l = s,
                _ => out.push(s),
            }
        }
        if out.is_empty() {
            out.push(span("", "Arial", DEFAULT_SIZE));
        }
        p.spans = out;
    }
}

/// Aplica `f` a cada trecho dentro de `[a, b)` (cortando os trechos nas
/// pontas) e a `g` a cada parágrafo tocado.
fn for_range(
    t: &mut TextData,
    a: usize,
    b: usize,
    mut f: impl FnMut(&mut TextSpan),
    mut g: impl FnMut(&mut Paragraph),
) {
    let (a, b) = (a.min(b), a.max(b).min(len(t)));
    if a == b {
        return;
    }
    cut_at(t, a);
    cut_at(t, b);
    let mut start = 0;
    for p in &mut t.paragraphs {
        let n = para_len(p);
        let (p0, p1) = (start, start + n);
        if p1 >= a && p0 < b {
            g(p);
        }
        let mut s0 = p0;
        for s in &mut p.spans {
            let s1 = s0 + s.text.chars().count();
            if s0 >= a && s1 <= b && s1 > s0 {
                f(s);
            }
            s0 = s1;
        }
        start = p1 + 1;
    }
    merge_spans(t);
}

fn style_span(s: &mut TextSpan, st: &TextStyle) {
    if let Some(f) = &st.family {
        s.font.family = f.clone();
    }
    if let Some(z) = st.size {
        s.font.size = z;
    }
    if let Some(b) = st.bold {
        s.font.weight = if b { 700 } else { 400 };
    }
    if let Some(w) = st.weight {
        s.font.weight = w;
    }
    if let Some(i) = st.italic {
        s.font.italic = i;
    }
}

/// Estilo só nos caracteres `[a, b)` (formatação por trecho, doc 06); o
/// alinhamento vale para os parágrafos tocados. Intervalo vazio = nada.
pub fn apply_style_range(t: &mut TextData, a: usize, b: usize, st: &TextStyle) {
    for_range(
        t,
        a,
        b,
        |s| style_span(s, st),
        |p| {
            if let Some(al) = &st.align {
                p.align = al.clone();
            }
        },
    );
}

/// Preenchimento por caractere em `[a, b)`; `None` volta ao do objeto.
pub fn fill_range(t: &mut TextData, a: usize, b: usize, fill: Option<crate::model::Fill>) {
    for_range(t, a, b, |s| s.fill = fill.clone(), |_| {});
}

/// Trecho do caractere logo antes de `idx` (o estilo que a digitação
/// continua), e o parágrafo dele.
fn span_at(t: &TextData, idx: usize) -> Option<(&Paragraph, &TextSpan)> {
    let (p, k) = locate_para(t, idx);
    let para = t.paragraphs.get(p)?;
    let (s, _) = locate_span(para, k);
    // No começo do parágrafo vale o primeiro trecho com texto.
    let s = if k == 0 {
        para.spans
            .iter()
            .position(|s| !s.text.is_empty())
            .unwrap_or(s)
    } else {
        s
    };
    Some((para, para.spans.get(s)?))
}

/// Estilo no índice (para a barra durante a edição); mesmo formato de
/// [`current_style`].
pub fn style_at(t: &TextData, idx: usize) -> (String, f64, bool, bool, String) {
    match span_at(t, idx) {
        Some((p, s)) => (
            s.font.family.clone(),
            s.font.size,
            s.font.weight >= 600,
            s.font.italic,
            p.align.clone(),
        ),
        None => current_style(t),
    }
}

/// Peso no índice.
pub fn weight_at(t: &TextData, idx: usize) -> u16 {
    span_at(t, idx).map_or_else(|| current_weight(t), |(_, s)| s.font.weight)
}

/// Peso do primeiro trecho.
pub fn current_weight(t: &TextData) -> u16 {
    t.paragraphs
        .first()
        .and_then(|p| p.spans.first())
        .map_or(400, |s| s.font.weight)
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

/// Menor x de caractere do texto (início visual das linhas).
fn left_edge(t: &TextData) -> f64 {
    let l = crate::render::text::layout_text(t);
    let m = l
        .lines
        .iter()
        .flat_map(|l| l.carets.iter().map(|c| c.1))
        .fold(f64::INFINITY, f64::min);
    if m.is_finite() { m } else { 0.0 }
}

/// Troca artístico ↔ parágrafo (GD-TXT-03) mantendo o texto no mesmo lugar
/// da página, inclusive centrado ou à direita.
pub fn convert_kind(o: &mut Object) -> bool {
    let ObjectKind::Text(t) = &mut o.kind else {
        return false;
    };
    let before = left_edge(t);
    if !(to_artistic(t) || to_paragraph(t)) {
        return false;
    }
    let dx = before - left_edge(t);
    if dx.abs() > 1e-9 {
        o.transform =
            Transform::from_affine(o.transform.affine() * kurbo::Affine::translate((dx, 0.0)));
    }
    true
}

/// Texto artístico → parágrafo (GD-TXT-03): o quadro fica do tamanho do
/// texto, então as linhas continuam as mesmas; trechos e estilos ficam.
pub fn to_paragraph(t: &mut TextData) -> bool {
    if t.text_kind == "paragraph" {
        return false;
    }
    let layout = crate::render::text::layout_text(t);
    let xs = || {
        layout
            .lines
            .iter()
            .flat_map(|l| l.carets.iter().map(|c| c.1))
    };
    // Centrado ou à direita, o texto artístico fica dos dois lados de x = 0.
    let width = xs().fold(f64::NEG_INFINITY, f64::max) - xs().fold(f64::INFINITY, f64::min);
    let width = if width.is_finite() { width } else { 0.0 };
    let height = layout
        .lines
        .last()
        .map_or(DEFAULT_SIZE, |l| -l.baseline + l.size * 0.3);
    // Folga para a quebra não mudar por arredondamento.
    t.frame = Some([width * 1.001 + 1.0, height.max(1.0)]);
    t.text_kind = "paragraph".into();
    true
}

/// Texto de parágrafo → artístico (GD-TXT-03): cada quebra automática vira
/// quebra de linha de verdade, para o texto ficar com as mesmas linhas.
pub fn to_artistic(t: &mut TextData) -> bool {
    if t.text_kind != "paragraph" {
        return false;
    }
    let layout = crate::render::text::layout_text(t);
    let chars: Vec<char> = crate::render::text::plain_text(t).chars().collect();
    let soft: Vec<usize> = layout
        .lines
        .iter()
        .map(|l| l.end)
        .filter(|&e| e < chars.len() && chars[e] != '\n')
        .collect();
    for &e in soft.iter().rev() {
        // O espaço onde a linha quebrou some.
        if chars.get(e) == Some(&' ') {
            delete(t, e, e + 1);
        } else if e > 0 && chars.get(e - 1) == Some(&' ') {
            delete(t, e - 1, e);
            insert(t, e - 1, "\n");
            continue;
        }
        insert(t, e, "\n");
    }
    t.text_kind = "artistic".into();
    t.frame = None;
    true
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
    /// Doc 06 (atributos de caractere): estilo e preenchimento só no trecho
    /// escolhido; trechos iguais voltam a se juntar.
    #[test]
    fn style_and_fill_by_range() {
        let mut o = new_text(0.0, 0.0, None, "Arial", 10.0);
        let t = data(&mut o);
        insert(t, 0, "abcdef\nghi");
        apply_style_range(
            t,
            2,
            4,
            &TextStyle {
                bold: Some(true),
                ..Default::default()
            },
        );
        let spans: Vec<(&str, u16)> = t.paragraphs[0]
            .spans
            .iter()
            .map(|s| (s.text.as_str(), s.font.weight))
            .collect();
        assert_eq!(spans, vec![("ab", 400), ("cd", 700), ("ef", 400)]);
        assert_eq!(plain_text(t), "abcdef\nghi");
        assert!(style_at(t, 3).2 && !style_at(t, 2).2, "antes do cursor");
        assert!(style_at(t, 4).2);
        // Atravessa o parágrafo: alinhamento nos dois, corpo só no trecho.
        apply_style_range(
            t,
            5,
            8,
            &TextStyle {
                size: Some(20.0),
                align: Some("center".into()),
                ..Default::default()
            },
        );
        assert_eq!(t.paragraphs[0].spans.last().unwrap().text, "f");
        assert_eq!(t.paragraphs[0].spans.last().unwrap().font.size, 20.0);
        assert_eq!(t.paragraphs[1].spans[0].text, "g");
        assert_eq!(t.paragraphs[1].spans[1].font.size, 10.0);
        assert!(t.paragraphs.iter().all(|p| p.align == "center"));
        // Desfazer o negrito junta de novo.
        apply_style_range(
            t,
            0,
            6,
            &TextStyle {
                bold: Some(false),
                size: Some(10.0),
                ..Default::default()
            },
        );
        assert_eq!(t.paragraphs[0].spans.len(), 1);
        // Preenchimento por caractere.
        let red = crate::model::Fill::solid(crate::model::Color::from_hex("#ff0000").unwrap());
        fill_range(t, 7, 8, Some(red.clone()));
        assert_eq!(t.paragraphs[1].spans[0].fill, Some(red));
        assert_eq!(t.paragraphs[1].spans[1].fill, None);
        assert_eq!(plain_text(t), "abcdef\nghi");
    }

    /// GD-TXT-03: parágrafo → artístico mantém as linhas (quebras viram
    /// quebras de verdade) e volta a parágrafo sem quebrar diferente.
    #[test]
    fn gd_txt_03_convert_artistic_paragraph() {
        if crate::render::text::resolve_family("Arial", 400, false).is_none() {
            return;
        }
        let mut o = new_text(0.0, 0.0, Some([30_000.0, 100_000.0]), "Arial", 5_000.0);
        let t = data(&mut o);
        insert(t, 0, "uma frase comprida que quebra em linhas");
        let lines = crate::render::text::layout_text(t).lines.len();
        assert!(lines > 1);
        assert!(to_artistic(t));
        assert_eq!(t.text_kind, "artistic");
        assert_eq!(t.paragraphs.len(), lines, "{:?}", plain_text(t));
        assert_eq!(crate::render::text::layout_text(t).lines.len(), lines);
        assert!(to_paragraph(t));
        let l = crate::render::text::layout_text(t);
        assert_eq!((l.lines.len(), l.overflow), (lines, false));
        assert!(!to_paragraph(t), "já é parágrafo");
    }

    /// Revisão de 2026-10-17: centrado e à direita, a conversão mantém a
    /// largura e o lugar do texto.
    #[test]
    fn convert_kind_keeps_place_when_centered() {
        if crate::render::text::resolve_family("Arial", 400, false).is_none() {
            return;
        }
        for align in ["center", "right"] {
            let mut o = new_text(50_000.0, 50_000.0, None, "Arial", 5_000.0);
            let t = data(&mut o);
            insert(
                t,
                0,
                "uma linha
outra linha maior",
            );
            apply_style(
                t,
                &TextStyle {
                    align: Some(align.into()),
                    ..Default::default()
                },
            );
            let page_left = |o: &Object| {
                let ObjectKind::Text(t) = &o.kind else {
                    unreachable!()
                };
                o.transform.affine().as_coeffs()[4] + left_edge(t)
            };
            let x0 = page_left(&o);
            let lines0 = crate::render::text::layout_text(data(&mut o)).lines.len();
            assert!(convert_kind(&mut o), "{align}");
            assert!((page_left(&o) - x0).abs() < 1.0, "{align}: pulou");
            let l = crate::render::text::layout_text(data(&mut o));
            assert_eq!(l.lines.len(), lines0, "{align}: mesma quebra");
            assert!(convert_kind(&mut o));
            assert!((page_left(&o) - x0).abs() < 1.0, "{align}: volta");
        }
    }
}
