//! Guias (GD-GUI-01/02): da página ou do documento (todas as páginas).
//! Cada operação é um passo do histórico.

use super::history::Editor;
use crate::model::Guide;

/// Onde a guia mora.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuideRef {
    /// Guia do documento (vale em todas as páginas), pelo índice.
    Document(usize),
    /// Guia de uma página, pelo índice.
    Page(usize, usize),
}

/// Lista de guias de uma página na mesma ordem de
/// [`crate::edit::snap::page_guides`]: primeiro as do documento.
pub fn refs(ed: &Editor, page: usize) -> Vec<(GuideRef, Guide)> {
    let mut out: Vec<(GuideRef, Guide)> = ed
        .doc
        .data
        .settings
        .guides
        .iter()
        .cloned()
        .enumerate()
        .map(|(i, g)| (GuideRef::Document(i), g))
        .collect();
    if let Some(p) = ed.doc.data.pages.get(page) {
        out.extend(
            p.guides
                .iter()
                .cloned()
                .enumerate()
                .map(|(i, g)| (GuideRef::Page(page, i), g)),
        );
    }
    out
}

/// Acrescenta guias à página (`all_pages` = do documento).
pub fn add(ed: &mut Editor, page: usize, all_pages: bool, guides: Vec<Guide>, name: &str) {
    if guides.is_empty() {
        return;
    }
    ed.transaction(name, |ed| {
        if all_pages {
            ed.edit_settings(|s| s.guides.extend(guides));
        } else {
            ed.edit_page(page, |p| p.guides.extend(guides));
        }
    });
}

/// Troca uma guia (mover, travar, cor…).
pub fn set(ed: &mut Editor, r: GuideRef, g: Guide, name: &str) {
    ed.transaction(name, |ed| match r {
        GuideRef::Document(i) => ed.edit_settings(|s| {
            if let Some(slot) = s.guides.get_mut(i) {
                *slot = g;
            }
        }),
        GuideRef::Page(p, i) => ed.edit_page(p, |pg| {
            if let Some(slot) = pg.guides.get_mut(i) {
                *slot = g;
            }
        }),
    });
}

/// Remove uma guia.
pub fn remove(ed: &mut Editor, r: GuideRef) {
    ed.transaction("Remover guia", |ed| match r {
        GuideRef::Document(i) => ed.edit_settings(|s| {
            if i < s.guides.len() {
                s.guides.remove(i);
            }
        }),
        GuideRef::Page(p, i) => ed.edit_page(p, |pg| {
            if i < pg.guides.len() {
                pg.guides.remove(i);
            }
        }),
    });
}

/// Passa uma guia da página para o documento ou vice-versa.
pub fn move_scope(ed: &mut Editor, r: GuideRef, page: usize) {
    let g = match r {
        GuideRef::Document(i) => ed.doc.data.settings.guides.get(i).cloned(),
        GuideRef::Page(p, i) => ed
            .doc
            .data
            .pages
            .get(p)
            .and_then(|pg| pg.guides.get(i).cloned()),
    };
    let Some(g) = g else { return };
    ed.transaction("Mudar alcance da guia", |ed| {
        remove(ed, r);
        match r {
            GuideRef::Document(_) => ed.edit_page(page, |p| p.guides.push(g)),
            GuideRef::Page(..) => ed.edit_settings(|s| s.guides.push(g)),
        }
    });
}

/// Remove todas as guias da página (e, com `all_pages`, as do documento).
pub fn clear(ed: &mut Editor, page: usize, all_pages: bool) {
    ed.transaction("Remover guias", |ed| {
        ed.edit_page(page, |p| p.guides.clear());
        if all_pages {
            ed.edit_settings(|s| s.guides.clear());
        }
    });
}

/// Predefinições (GD-GUI-02) para uma página de `w` × `h` µm: margens,
/// colunas (com medianiz) e centro.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Presets {
    /// Margem em µm (todas as bordas).
    pub margin: Option<f64>,
    /// Colunas e medianiz (µm), dentro das margens.
    pub columns: Option<(u32, f64)>,
    pub center: bool,
}

pub fn presets(w: f64, h: f64, p: &Presets) -> Vec<Guide> {
    let mut out = Vec::new();
    let m = p.margin.unwrap_or(0.0).max(0.0);
    if p.margin.is_some() {
        out.extend([
            Guide::vertical(m),
            Guide::vertical(w - m),
            Guide::horizontal(m),
            Guide::horizontal(h - m),
        ]);
    }
    if let Some((n, gutter)) = p.columns.filter(|(n, _)| *n >= 2) {
        let inner = w - 2.0 * m;
        let col = (inner - gutter * (n - 1) as f64) / n as f64;
        if col > 0.0 {
            for i in 1..n {
                let x = m + i as f64 * col + (i - 1) as f64 * gutter;
                out.push(Guide::vertical(x));
                if gutter > 0.0 {
                    out.push(Guide::vertical(x + gutter));
                }
            }
        }
    }
    if p.center {
        out.extend([Guide::vertical(w / 2.0), Guide::horizontal(h / 2.0)]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Document, Page};

    fn ed() -> Editor {
        let mut d = Document::new();
        d.data.pages.push(Page::new(200_000.0, 100_000.0));
        d.data.pages.push(Page::new(200_000.0, 100_000.0));
        Editor::new(d)
    }

    #[test]
    fn gd_gui_01_add_move_remove_scope_and_undo() {
        let mut e = ed();
        add(&mut e, 0, false, vec![Guide::vertical(10_000.0)], "g");
        add(&mut e, 0, true, vec![Guide::horizontal(5_000.0)], "g");
        let r = refs(&e, 0);
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].0, GuideRef::Document(0));
        assert_eq!(refs(&e, 1).len(), 1, "a do documento vale na página 2");
        // Mover a guia da página.
        let mut g = r[1].1.clone();
        g.position = 20_000.0;
        set(&mut e, r[1].0, g, "mover");
        assert_eq!(e.doc.data.pages[0].guides[0].position, 20_000.0);
        e.undo();
        assert_eq!(e.doc.data.pages[0].guides[0].position, 10_000.0);
        // Passar a da página para todas.
        move_scope(&mut e, GuideRef::Page(0, 0), 0);
        assert_eq!(refs(&e, 1).len(), 2);
        e.undo();
        assert_eq!(refs(&e, 1).len(), 1, "desfazer volta num passo");
        remove(&mut e, GuideRef::Document(0));
        assert!(e.doc.data.settings.guides.is_empty());
        clear(&mut e, 0, true);
        assert!(refs(&e, 0).is_empty());
    }

    #[test]
    fn gd_gui_02_presets_margins_columns_center() {
        let g = presets(
            200.0,
            100.0,
            &Presets {
                margin: Some(10.0),
                columns: Some((3, 6.0)),
                center: true,
            },
        );
        let xs: Vec<f64> = g
            .iter()
            .filter(|g| g.kind == "vertical")
            .map(|g| g.position)
            .collect();
        // Margens 10 e 190; colunas de (180 − 12)/3 = 56: 66|72, 128|134; centro 100.
        for x in [10.0, 190.0, 66.0, 72.0, 128.0, 134.0, 100.0] {
            assert!(xs.iter().any(|v| (v - x).abs() < 1e-9), "{x} em {xs:?}");
        }
        assert_eq!(g.iter().filter(|g| g.kind == "horizontal").count(), 3);
    }
}
