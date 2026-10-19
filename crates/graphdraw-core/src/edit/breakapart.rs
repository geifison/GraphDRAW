//! Separar (GD-CMB-02..05): desfaz um agrupamento lógico, um nível por vez,
//! conforme o tipo do objeto.
//!
//! - Caminho com vários subcaminhos: cada subcaminho vira um objeto; os
//!   furos viram objetos preenchidos por cima (GD-CMB-02).
//! - Texto, em ordem (GD-CMB-03): texto em caminho solta o caminho guia
//!   (o texto continua curvado); bloco de parágrafo com vários parágrafos →
//!   um bloco por parágrafo; várias linhas → uma linha por objeto; uma
//!   linha → palavras; uma palavra → letras.
//!
//! Cada pedaço fica exatamente onde estava; tudo numa só transação.

use kurbo::Affine;

use super::Editor;
use super::tree::ObjRef;
use crate::model::{Color, Object, ObjectKind, Paragraph, Stroke, TextData, Transform, new_id};
use crate::render::text::{TextLayout, layout_text};

/// Resultado de separar um objeto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Nada a separar: o objeto continua selecionado.
    Unchanged,
    /// O objeto foi trocado por estes (IDs, na ordem de empilhamento).
    Split(Vec<String>),
}

/// Separa cada objeto da seleção um nível. Devolve a nova seleção (os
/// pedaços e os objetos que não mudaram), ou `None` se nada mudou.
pub fn break_apart(ed: &mut Editor, sel: &[String]) -> Option<Vec<String>> {
    let mut out = Vec::new();
    let mut changed = false;
    ed.transaction("Separar", |ed| {
        for id in sel {
            match break_one(ed, id) {
                Outcome::Split(ids) => {
                    changed = true;
                    out.extend(ids);
                }
                Outcome::Unchanged => {
                    if ed.find(id).is_some() {
                        out.push(id.clone());
                    }
                }
            }
        }
    });
    changed.then_some(out)
}

/// Separa um objeto um nível (ver o cabeçalho do módulo).
pub fn break_one(ed: &mut Editor, id: &str) -> Outcome {
    let Some(r) = ed.find(id) else {
        return Outcome::Unchanged;
    };
    let Some(o) = ed.get(&r).cloned() else {
        return Outcome::Unchanged;
    };
    match &o.kind {
        ObjectKind::Path(_) => match super::shapeops::separate(ed, id, false) {
            Some(ids) => Outcome::Split(ids),
            None => Outcome::Unchanged,
        },
        ObjectKind::Text(t) => {
            if t.path.as_ref().is_some_and(|p| !p.detached) {
                return detach_guide(ed, &r, &o);
            }
            match split_text(&o) {
                Some(pieces) => {
                    let ids = pieces.iter().map(|p| p.id.clone()).collect();
                    replace_with(ed, &r, pieces);
                    Outcome::Split(ids)
                }
                None => Outcome::Unchanged,
            }
        }
        _ => Outcome::Unchanged,
    }
}

/// Texto em caminho (GD-CMB-03): o texto deixa de depender do caminho e
/// mantém a curvatura. Se o caminho de origem não existe mais no
/// documento, ele é recriado (com a forma que o texto segue) abaixo do
/// texto. Devolve texto e caminho.
fn detach_guide(ed: &mut Editor, r: &ObjRef, o: &Object) -> Outcome {
    let ObjectKind::Text(t) = &o.kind else {
        return Outcome::Unchanged;
    };
    let Some(tp) = &t.path else {
        return Outcome::Unchanged;
    };
    let existing = tp.guide.as_deref().filter(|g| ed.find(g).is_some());
    let mut text = o.clone();
    if let ObjectKind::Text(td) = &mut text.kind
        && let Some(p) = &mut td.path
    {
        p.detached = true;
        p.guide = None;
    }
    let mut ids = Vec::new();
    match existing {
        Some(g) => {
            ids.push(g.to_string());
            ids.push(text.id.clone());
            ed.replace_at(r, text);
        }
        None => {
            let mut line = Object::new(ObjectKind::Path(crate::model::PathData {
                subpaths: tp.subpaths.clone(),
                fill_rule: Default::default(),
                extra: Default::default(),
            }));
            line.transform = o.transform;
            line.stroke = Some(Stroke::new(250.0, Color::black()));
            ids.push(line.id.clone());
            ids.push(text.id.clone());
            replace_with(ed, r, vec![line, text]);
        }
    }
    Outcome::Split(ids)
}

/// Troca o objeto em `r` pelos `pieces`, no mesmo contêiner e posição.
fn replace_with(ed: &mut Editor, r: &ObjRef, pieces: Vec<Object>) {
    let c = r.container();
    let at = r.index();
    ed.remove_at(r);
    for (k, p) in pieces.into_iter().enumerate() {
        ed.insert_at(&c, at + k, p);
    }
}

/// Nível em que o texto será separado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextLevel {
    Paragraphs,
    Lines,
    Words,
    Letters,
}

/// Faixas de caracteres (no texto corrido) de cada pedaço do próximo nível
/// de separação, ou `None` se o texto já é uma letra só (ou vazio).
pub fn text_pieces(t: &TextData) -> Option<(TextLevel, Vec<(usize, usize)>)> {
    let text: Vec<char> = crate::render::text::plain_text(t).chars().collect();
    if t.text_kind == "paragraph" {
        let mut ranges = Vec::new();
        let mut start = 0;
        for p in &t.paragraphs {
            let n: usize = p.spans.iter().map(|s| s.text.chars().count()).sum();
            if text[start..start + n].iter().any(|c| !c.is_whitespace()) {
                ranges.push((start, start + n));
            }
            start += n + 1;
        }
        if ranges.len() > 1 {
            return Some((TextLevel::Paragraphs, ranges));
        }
    }
    let lay = layout_text(&TextData {
        path: None,
        ..t.clone()
    });
    let lines: Vec<(usize, usize)> = lay
        .lines
        .iter()
        .map(|l| trim(&text, l.start, l.end))
        .filter(|(a, b)| a < b)
        .collect();
    if lines.len() > 1 {
        return Some((TextLevel::Lines, lines));
    }
    let (a, b) = lines.first().copied()?;
    let words = words(&text, a, b);
    if words.len() > 1 {
        return Some((TextLevel::Words, words));
    }
    let letters = letters(&text, a, b);
    (letters.len() > 1).then_some((TextLevel::Letters, letters))
}

/// `[a, b)` sem espaços nas pontas.
fn trim(text: &[char], mut a: usize, mut b: usize) -> (usize, usize) {
    b = b.min(text.len());
    while a < b && text[a].is_whitespace() {
        a += 1;
    }
    while b > a && text[b - 1].is_whitespace() {
        b -= 1;
    }
    (a, b)
}

fn words(text: &[char], a: usize, b: usize) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut start = None;
    for (i, c) in text.iter().enumerate().take(b).skip(a) {
        match (c.is_whitespace(), start) {
            (false, None) => start = Some(i),
            (true, Some(s)) => {
                out.push((s, i));
                start = None;
            }
            _ => {}
        }
    }
    if let Some(s) = start {
        out.push((s, b));
    }
    out
}

/// Marcas que se juntam ao caractere anterior (acentos combinantes,
/// seletores de variação, junção de emoji).
fn joins_previous(c: char) -> bool {
    matches!(c as u32,
        0x0300..=0x036F | 0x1AB0..=0x1AFF | 0x1DC0..=0x1DFF | 0x20D0..=0x20FF
        | 0xFE00..=0xFE0F | 0xFE20..=0xFE2F | 0x200D | 0x1F3FB..=0x1F3FF
        | 0xE0100..=0xE01EF)
}

fn letters(text: &[char], a: usize, b: usize) -> Vec<(usize, usize)> {
    let mut out: Vec<(usize, usize)> = Vec::new();
    for i in a..b {
        let after_zwj = i > a && text[i - 1] == '\u{200D}';
        if (joins_previous(text[i]) || after_zwj) && !out.is_empty() {
            out.last_mut().unwrap().1 = i + 1;
        } else if !text[i].is_whitespace() {
            out.push((i, i + 1));
        }
    }
    out
}

/// Trecho `[a, b)` do texto corrido, com os estilos de cada trecho e o
/// alinhamento dos parágrafos.
pub fn slice_text(t: &TextData, a: usize, b: usize) -> TextData {
    let mut paragraphs = Vec::new();
    let mut start = 0;
    for p in &t.paragraphs {
        let n: usize = p.spans.iter().map(|s| s.text.chars().count()).sum();
        let (pa, pb) = (a.max(start), b.min(start + n));
        if pa < pb || (pa == pb && a <= start && start + n <= b) {
            let mut spans = Vec::new();
            let mut s0 = start;
            for s in &p.spans {
                let len = s.text.chars().count();
                let (sa, sb) = (pa.max(s0), pb.min(s0 + len));
                if sa < sb {
                    let mut ns = s.clone();
                    ns.text = s.text.chars().skip(sa - s0).take(sb - sa).collect();
                    spans.push(ns);
                }
                s0 += len;
            }
            if spans.is_empty() {
                let mut ns = p.spans[0].clone();
                ns.text.clear();
                spans.push(ns);
            }
            paragraphs.push(Paragraph {
                align: p.align.clone(),
                spans,
                hyphenate: false,
                lang: None,
                extra: p.extra.clone(),
            });
        }
        start += n + 1;
    }
    if paragraphs.is_empty() {
        paragraphs.push(t.paragraphs[0].clone());
    }
    TextData {
        paragraphs,
        ..t.clone()
    }
}

/// Posição (x, linha de base) do caractere `idx` na diagramação.
fn pos_of(lay: &TextLayout, idx: usize) -> Option<(f64, f64)> {
    let line = lay
        .lines
        .iter()
        .find(|l| idx >= l.start && idx < l.end)
        .or_else(|| lay.lines.iter().find(|l| idx >= l.start && idx <= l.end))?;
    let x = line.carets.iter().find(|(i, _)| *i == idx)?.1;
    Some((x, line.baseline))
}

/// Pedaços do texto no próximo nível, cada um no lugar exato em que
/// estava; `None` se não há o que separar.
pub fn split_text(o: &Object) -> Option<Vec<Object>> {
    let ObjectKind::Text(t) = &o.kind else {
        return None;
    };
    let (level, ranges) = text_pieces(t)?;
    let flat = TextData {
        path: None,
        ..t.clone()
    };
    let lay = layout_text(&flat);
    let b0 = lay.lines.first().map_or(0.0, |l| l.baseline);
    let mut out = Vec::new();
    for (a, b) in ranges {
        let mut piece = slice_text(t, a, b);
        if level != TextLevel::Paragraphs {
            // Linha, palavra e letra viram texto artístico de uma linha.
            piece.text_kind = "artistic".into();
            piece.frame = None;
            for p in &mut piece.paragraphs {
                if level != TextLevel::Lines
                    || matches!(p.align.as_str(), "justify" | "forceJustify")
                {
                    p.align = "left".into();
                }
            }
        }
        let (Some((xa, ya)), Some((x0, y0))) = (
            pos_of(&lay, a),
            pos_of(
                &layout_text(&TextData {
                    path: None,
                    ..piece.clone()
                }),
                0,
            ),
        ) else {
            continue;
        };
        let mut n = o.clone();
        n.id = new_id();
        if level == TextLevel::Paragraphs
            && let Some([w, _]) = piece.frame
        {
            // Quadro com a altura do parágrafo (mesma largura: mesma quebra).
            let pl = layout_text(&TextData {
                path: None,
                frame: Some([w, f64::MAX]),
                ..piece.clone()
            });
            if let Some(last) = pl.lines.last() {
                piece.frame = Some([w, -last.baseline + last.size * 0.3]);
            }
        }
        match &mut piece.path {
            // Texto em caminho: o pedaço continua no mesmo caminho,
            // deslocado ao longo dele e afastado como a linha original.
            Some(tp) => {
                tp.offset += xa - x0;
                tp.distance += ya - b0;
            }
            None => {
                n.transform = Transform::from_affine(
                    o.transform.affine() * Affine::translate((xa - x0, ya - y0)),
                );
            }
        }
        n.kind = ObjectKind::Text(piece);
        out.push(n);
    }
    (out.len() > 1).then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit::text::{insert, new_text};
    use crate::model::{Document, Node, Page, PathData, Subpath};

    fn has_font() -> bool {
        crate::render::text::resolve_family("Arial", 400, false).is_some()
    }

    fn editor() -> Editor {
        let mut d = Document::new();
        d.data.pages.push(Page::new(300_000.0, 300_000.0));
        Editor::new(d)
    }

    fn add(ed: &mut Editor, o: Object) -> String {
        let id = o.id.clone();
        ed.transaction("+", |ed| {
            let n = ed.objects(0, 0).len();
            ed.insert_object(
                super::super::ObjLoc {
                    page: 0,
                    layer: 0,
                    index: n,
                },
                o,
            );
        });
        id
    }

    fn text(ed: &mut Editor, s: &str, frame: Option<[f64; 2]>) -> String {
        let mut o = new_text(10_000.0, 200_000.0, frame, "Arial", 10_000.0);
        if let ObjectKind::Text(td) = &mut o.kind {
            insert(td, 0, s);
        }
        add(ed, o)
    }

    fn plain(ed: &Editor, id: &str) -> String {
        match &ed.get(&ed.find(id).unwrap()).unwrap().kind {
            ObjectKind::Text(t) => crate::render::text::plain_text(t),
            _ => panic!("não é texto"),
        }
    }

    /// Caixa do objeto em coordenadas de página.
    fn page_box(ed: &Editor, id: &str) -> kurbo::Rect {
        super::super::ops::object_bounds(ed, &ed.find(id).unwrap(), false).unwrap()
    }

    fn texts(ed: &Editor, ids: &[String]) -> Vec<String> {
        ids.iter().map(|i| plain(ed, i)).collect()
    }

    #[test]
    fn gd_cmb_03_text_hierarchy() {
        if !has_font() {
            return;
        }
        let mut ed = editor();
        let id = text(&mut ed, "Um dois\ntrês", Some([150_000.0, 100_000.0]));
        // Segundo parágrafo.
        if let Some(r) = ed.find(&id) {
            let mut o = ed.get(&r).unwrap().clone();
            if let ObjectKind::Text(t) = &mut o.kind {
                t.paragraphs[1].align = "right".into();
            }
            ed.transaction("x", |ed| ed.replace_at(&r, o));
        }
        let all = page_box(&ed, &id);
        // 1. Parágrafos.
        let paras = break_apart(&mut ed, std::slice::from_ref(&id)).unwrap();
        assert_eq!(texts(&ed, &paras), ["Um dois", "três"]);
        let g = ed.get(&ed.find(&paras[1]).unwrap()).unwrap();
        match &g.kind {
            ObjectKind::Text(t) => {
                assert_eq!(t.text_kind, "paragraph");
                assert_eq!(t.paragraphs[0].align, "right");
            }
            _ => unreachable!(),
        }
        let union = page_box(&ed, &paras[0]).union(page_box(&ed, &paras[1]));
        assert!(
            (union.x0 - all.x0).abs() < 1.0 && (union.y0 - all.y0).abs() < 1.0,
            "{union:?} {all:?}"
        );
        assert!(
            (union.x1 - all.x1).abs() < 1.0 && (union.y1 - all.y1).abs() < 1.0,
            "{union:?} {all:?}"
        );
        // 2. Palavras (a linha tem duas palavras).
        let before = page_box(&ed, &paras[0]);
        let words = break_apart(&mut ed, &paras[..1]).unwrap();
        assert_eq!(texts(&ed, &words), ["Um", "dois"]);
        let w0 = page_box(&ed, &words[0]);
        assert!(
            (w0.x0 - before.x0).abs() < 1.0 && (w0.y0 - before.y0).abs() < 50.0,
            "{w0:?} {before:?}"
        );
        let w1 = page_box(&ed, &words[1]);
        assert!((w1.x1 - before.x1).abs() < 1.0, "{w1:?} {before:?}");
        // 3. Letras.
        let letters = break_apart(&mut ed, &words[1..2]).unwrap();
        assert_eq!(texts(&ed, &letters), ["d", "o", "i", "s"]);
        let l3 = page_box(&ed, &letters[3]);
        assert!((l3.x1 - w1.x1).abs() < 1.0, "{l3:?} {w1:?}");
        // 4. Uma letra: nada a fazer.
        assert!(break_apart(&mut ed, &letters[..1]).is_none());
        // Tudo numa transação por comando.
        ed.undo();
        assert_eq!(plain(&ed, &words[1]), "dois");
    }

    #[test]
    fn gd_cmb_03_lines_keep_place() {
        if !has_font() {
            return;
        }
        let mut ed = editor();
        // Artístico com três linhas centralizadas.
        let id = text(&mut ed, "a\nlinha longa\nfim", None);
        if let Some(r) = ed.find(&id) {
            let mut o = ed.get(&r).unwrap().clone();
            if let ObjectKind::Text(t) = &mut o.kind {
                for p in &mut t.paragraphs {
                    p.align = "center".into();
                }
            }
            ed.transaction("x", |ed| ed.replace_at(&r, o));
        }
        let all = page_box(&ed, &id);
        let lines = break_apart(&mut ed, std::slice::from_ref(&id)).unwrap();
        assert_eq!(texts(&ed, &lines), ["a", "linha longa", "fim"]);
        let mut u = page_box(&ed, &lines[0]);
        for l in &lines[1..] {
            u = u.union(page_box(&ed, l));
        }
        assert!(
            (u.x0 - all.x0).abs() < 1.0 && (u.x1 - all.x1).abs() < 1.0,
            "{u:?} {all:?}"
        );
        assert!(
            (u.y0 - all.y0).abs() < 1.0 && (u.y1 - all.y1).abs() < 1.0,
            "{u:?} {all:?}"
        );
        // Linhas de um parágrafo quebrado no quadro.
        let wrapped = text(&mut ed, "aaa bbb ccc ddd", Some([25_000.0, 100_000.0]));
        let ls = break_apart(&mut ed, std::slice::from_ref(&wrapped)).unwrap();
        assert!(ls.len() > 1, "{:?}", texts(&ed, &ls));
        assert!(texts(&ed, &ls).iter().all(|s| !s.ends_with(' ')));
    }

    #[test]
    fn gd_cmb_03_text_on_path_detaches_then_splits() {
        if !has_font() {
            return;
        }
        let mut ed = editor();
        let tid = text(&mut ed, "AB CD", None);
        let mut line = Object::new(ObjectKind::Path(PathData {
            subpaths: vec![Subpath {
                closed: false,
                nodes: vec![
                    Node::corner(100_000.0, 20_000.0),
                    Node::corner(100_000.0, 180_000.0),
                ],
                extra: Default::default(),
            }],
            fill_rule: Default::default(),
            extra: Default::default(),
        }));
        line.stroke = Some(Stroke::new(250.0, Color::black()));
        let pid = add(&mut ed, line);
        assert!(super::super::ops::fit_text_to_path(&mut ed, &tid, &pid));
        let curved = page_box(&ed, &tid);
        // 1. Solta do caminho guia, que continua lá (não duplica).
        let a = break_apart(&mut ed, std::slice::from_ref(&tid)).unwrap();
        assert_eq!(a, [pid.clone(), tid.clone()]);
        assert_eq!(ed.objects(0, 0).len(), 2);
        let b = page_box(&ed, &tid);
        assert!(
            (b.y0 - curved.y0).abs() < 1.0 && (b.x0 - curved.x0).abs() < 1.0,
            "mantém a curva"
        );
        // Mover o caminho não mexe no texto.
        super::super::ops::modify(&mut ed, std::slice::from_ref(&pid), "m", |o| {
            o.transform = Transform::translate(50_000.0, 0.0);
            true
        });
        assert_eq!(page_box(&ed, &tid), b);
        // 2. Depois: palavras, ainda em curva, no mesmo lugar.
        let w = break_apart(&mut ed, std::slice::from_ref(&tid)).unwrap();
        assert_eq!(texts(&ed, &w), ["AB", "CD"]);
        let u = page_box(&ed, &w[0]).union(page_box(&ed, &w[1]));
        assert!(
            (u.y0 - b.y0).abs() < 50.0 && (u.y1 - b.y1).abs() < 50.0,
            "{u:?} {b:?}"
        );
        assert!(page_box(&ed, &w[1]).height() > page_box(&ed, &w[1]).width());
    }

    #[test]
    fn gd_cmb_03_orphan_text_on_path_recreates_guide() {
        if !has_font() {
            return;
        }
        let mut ed = editor();
        let tid = text(&mut ed, "AB", None);
        let r = ed.find(&tid).unwrap();
        let mut o = ed.get(&r).unwrap().clone();
        if let ObjectKind::Text(t) = &mut o.kind {
            t.path = Some(crate::model::TextPath {
                subpaths: vec![Subpath {
                    closed: false,
                    nodes: vec![Node::corner(0.0, 0.0), Node::corner(80_000.0, 0.0)],
                    extra: Default::default(),
                }],
                offset: 0.0,
                distance: 0.0,
                side: "above".into(),
                orientation: "rotate".into(),
                mirror: false,
                guide: None,
                detached: false,
                align: "start".into(),
                vertical: "baseline".into(),
                corners: "jump".into(),
                relax: true,
                extra: Default::default(),
            });
        }
        ed.transaction("x", |ed| ed.replace_at(&r, o));
        let ids = break_apart(&mut ed, std::slice::from_ref(&tid)).unwrap();
        assert_eq!(ids.len(), 2);
        assert_eq!(ids[1], tid);
        let guide = ed.get(&ed.find(&ids[0]).unwrap()).unwrap();
        assert!(matches!(guide.kind, ObjectKind::Path(_)) && guide.stroke.is_some());
        assert_eq!(ed.objects(0, 0).len(), 2);
    }

    #[test]
    fn gd_cmb_02_holes_become_filled_objects() {
        let mut ed = editor();
        let sq = |x: f64, y: f64, s: f64| Subpath {
            closed: true,
            nodes: vec![
                Node::corner(x, y),
                Node::corner(x + s, y),
                Node::corner(x + s, y + s),
                Node::corner(x, y + s),
            ],
            extra: Default::default(),
        };
        let mut o = Object::new(ObjectKind::Path(PathData {
            subpaths: vec![sq(0.0, 0.0, 30_000.0), sq(10_000.0, 10_000.0, 10_000.0)],
            fill_rule: crate::model::FillRule::Evenodd,
            extra: Default::default(),
        }));
        o.fill = Some(crate::model::Fill::solid(Color::black()));
        let id = add(&mut ed, o);
        let ids = break_apart(&mut ed, std::slice::from_ref(&id)).unwrap();
        assert_eq!(ids.len(), 2, "a arruela vira dois objetos cheios");
        for i in &ids {
            let o = ed.get(&ed.find(i).unwrap()).unwrap();
            assert!(o.fill.is_some());
            match &o.kind {
                ObjectKind::Path(p) => assert_eq!(p.subpaths.len(), 1),
                _ => unreachable!(),
            }
        }
        // O furo fica por cima da parte externa.
        let top = ed.objects(0, 0).last().unwrap();
        match &top.kind {
            ObjectKind::Path(p) => assert_eq!(p.subpaths[0].nodes[0].p, [10_000.0, 10_000.0]),
            _ => unreachable!(),
        }
    }

    #[test]
    fn nothing_to_break_keeps_selection() {
        let mut ed = editor();
        let id = add(
            &mut ed,
            Object::new(ObjectKind::Rectangle(crate::model::RectData {
                width: 1.0,
                height: 1.0,
                corners: None,
                extra: Default::default(),
            })),
        );
        let undo = ed.undo_len();
        assert!(break_apart(&mut ed, std::slice::from_ref(&id)).is_none());
        assert_eq!(ed.undo_len(), undo, "sem entrada vazia no histórico");
    }
}
