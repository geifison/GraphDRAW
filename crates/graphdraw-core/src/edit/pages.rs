//! Operações de página para a barra de abas (doc 21, GD-PGT-*): criar,
//! duplicar, mover em bloco e excluir, cada uma num passo do histórico.

use super::history::Editor;
use super::ops::renew_ids;
use crate::model::{Page, new_id};

/// Nome exibido da página (o salvo ou "Página n").
pub fn display_name(page: &Page, index: usize) -> String {
    page.name
        .clone()
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| format!("Página {}", index + 1))
}

/// Nova página logo depois de `after`, com o mesmo tamanho (GD-PGT-11).
/// Devolve o índice dela.
pub fn add_after(ed: &mut Editor, after: usize) -> usize {
    let n = ed.doc.data.pages.len();
    let (w, h) = ed
        .doc
        .data
        .pages
        .get(after.min(n.saturating_sub(1)))
        .map_or((210_000.0, 297_000.0), |p| (p.width, p.height));
    let mut p = Page::new(w, h);
    p.name = Some(format!("Página {}", n + 1));
    let at = (after + 1).min(n);
    ed.transaction("Nova página", |ed| ed.insert_page(at, p));
    at
}

/// Cópia da página `index` inserida em `at` (posição na lista antes da
/// inserção), com IDs novos e nome "<nome> (Cópia)" (GD-PGT-13).
pub fn duplicate(ed: &mut Editor, index: usize, at: usize) -> usize {
    let mut p = ed.doc.data.pages[index].clone();
    p.name = Some(format!("{} (Cópia)", display_name(&p, index)));
    p.id = new_id();
    for l in &mut p.layers {
        l.id = new_id();
        for o in &mut l.objects {
            renew_ids(o);
        }
    }
    let at = at.min(ed.doc.data.pages.len());
    ed.transaction("Duplicar página", |ed| ed.insert_page(at, p));
    at
}

/// Move as páginas `sel` (em qualquer ordem) para antes da página que está
/// em `to` — ou para o fim, se `to` for o total —, mantendo a ordem relativa
/// delas (GD-PGT-12). Devolve os novos índices das páginas movidas.
pub fn move_block(ed: &mut Editor, sel: &[usize], to: usize) -> Vec<usize> {
    let n = ed.doc.data.pages.len();
    let mut sel: Vec<usize> = sel.iter().copied().filter(|i| *i < n).collect();
    sel.sort();
    sel.dedup();
    if sel.is_empty() {
        return Vec::new();
    }
    let ids: Vec<String> = ed.doc.data.pages.iter().map(|p| p.id.clone()).collect();
    let moving: Vec<String> = sel.iter().map(|i| ids[*i].clone()).collect();
    let rest: Vec<String> = ids
        .iter()
        .enumerate()
        .filter(|(i, _)| !sel.contains(i))
        .map(|(_, id)| id.clone())
        .collect();
    let insert_at = (0..to.min(n)).filter(|i| !sel.contains(i)).count();
    let mut target = rest;
    for (k, id) in moving.iter().enumerate() {
        target.insert(insert_at + k, id.clone());
    }
    let out: Vec<usize> = (insert_at..insert_at + moving.len()).collect();
    if target == ids {
        return out;
    }
    let name = if moving.len() == 1 {
        "Mover página".to_string()
    } else {
        format!("Mover {} páginas", moving.len())
    };
    ed.transaction(name, |ed| {
        for (k, id) in target.iter().enumerate() {
            let cur = ed.doc.data.pages.iter().position(|p| &p.id == id).unwrap();
            if cur != k {
                let p = ed.remove_page(cur);
                ed.insert_page(k, p);
            }
        }
    });
    out
}

/// Exclui as páginas, mantendo pelo menos uma (GD-PGT-15). Devolve quantas
/// saíram.
pub fn delete(ed: &mut Editor, sel: &[usize]) -> usize {
    let n = ed.doc.data.pages.len();
    let mut sel: Vec<usize> = sel.iter().copied().filter(|i| *i < n).collect();
    sel.sort();
    sel.dedup();
    if sel.len() >= n {
        sel.truncate(n - 1);
    }
    if sel.is_empty() {
        return 0;
    }
    let name = if sel.len() == 1 {
        "Excluir página".to_string()
    } else {
        format!("Excluir {} páginas", sel.len())
    };
    ed.transaction(name, |ed| {
        for i in sel.iter().rev() {
            ed.remove_page(*i);
        }
    });
    sel.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Document, Object, ObjectKind, RectData};
    use serde_json::Map;

    fn doc(names: &[&str]) -> Editor {
        let mut d = Document::new();
        for n in names {
            let mut p = Page::new(100.0, 100.0);
            p.name = Some((*n).into());
            d.data.pages.push(p);
        }
        Editor::new(d)
    }

    fn names(ed: &Editor) -> Vec<String> {
        ed.doc
            .data
            .pages
            .iter()
            .enumerate()
            .map(|(i, p)| format!("[{}] {}", i + 1, display_name(p, i)))
            .collect()
    }

    fn rect() -> Object {
        Object::new(ObjectKind::Rectangle(RectData {
            width: 1.0,
            height: 1.0,
            corners: None,
            extra: Map::new(),
        }))
    }

    /// Aceite: mover "Capa" da posição 2 para a 5 → "[5] Capa"; desfazer → "[2] Capa".
    #[test]
    fn gd_pgt_03_12_move_keeps_name_and_renumbers() {
        let mut ed = doc(&["A", "Capa", "B", "C", "D", "E"]);
        let out = move_block(&mut ed, &[1], 5);
        assert_eq!(out, vec![4]);
        assert_eq!(names(&ed)[4], "[5] Capa");
        assert_eq!(ed.history().len(), 1, "um passo só");
        ed.undo();
        assert_eq!(names(&ed)[1], "[2] Capa");
        // Bloco: B e D vão para o início, nessa ordem.
        let out = move_block(&mut ed, &[4, 2], 0);
        assert_eq!(out, vec![0, 1]);
        assert_eq!(
            names(&ed),
            ["[1] B", "[2] D", "[3] A", "[4] Capa", "[5] C", "[6] E"]
        );
        // Para o fim.
        move_block(&mut ed, &[0], 6);
        assert_eq!(names(&ed)[5], "[6] B");
    }

    /// Aceite: Alt+arrastar "Base" para depois da última → "Base (Cópia)" com
    /// os mesmos objetos e IDs diferentes.
    #[test]
    fn gd_pgt_13_duplicate_renews_ids() {
        let mut ed = doc(&["Capa", "Base"]);
        ed.doc.data.pages[1].layers[0].objects = vec![rect(), rect()];
        let i = duplicate(&mut ed, 1, 2);
        assert_eq!(i, 2);
        assert_eq!(names(&ed)[2], "[3] Base (Cópia)");
        let (a, b) = (&ed.doc.data.pages[1], &ed.doc.data.pages[2]);
        assert_eq!(b.object_count(), 2);
        assert_ne!(a.id, b.id);
        assert_ne!(a.layers[0].id, b.layers[0].id);
        assert_ne!(a.layers[0].objects[0].id, b.layers[0].objects[0].id);
    }

    /// Aceite: excluir página com 3 objetos e desfazer restaura os 3; a
    /// última página não sai.
    #[test]
    fn gd_pgt_15_delete_is_undoable_and_keeps_one() {
        let mut ed = doc(&["A", "B"]);
        ed.doc.data.pages[1].layers[0].objects = vec![rect(), rect(), rect()];
        assert_eq!(delete(&mut ed, &[1]), 1);
        assert_eq!(ed.doc.data.pages.len(), 1);
        ed.undo();
        assert_eq!(ed.doc.data.pages[1].object_count(), 3);
        assert_eq!(delete(&mut ed, &[0, 1]), 1, "fica uma página");
        assert_eq!(ed.doc.data.pages.len(), 1);
    }

    #[test]
    fn gd_pgt_11_add_after_uses_active_size() {
        let mut ed = doc(&["A", "B"]);
        ed.doc.data.pages[0].width = 300.0;
        let i = add_after(&mut ed, 0);
        assert_eq!(i, 1);
        assert_eq!(ed.doc.data.pages[1].width, 300.0);
        assert_eq!(ed.doc.data.pages.len(), 3);
    }

    /// Aceite: editar objeto da página 2 → só ela fica marcada; salvar limpa;
    /// desfazer até o salvo também limpa.
    #[test]
    fn gd_pgt_05_dirty_pages_follow_history() {
        use crate::edit::ObjLoc;
        let mut ed = doc(&["A", "B", "C"]);
        ed.mark_saved();
        let id_b = ed.doc.data.pages[1].id.clone();
        ed.insert_object(
            ObjLoc {
                page: 1,
                layer: 0,
                index: 0,
            },
            rect(),
        );
        assert_eq!(
            ed.dirty_pages().into_iter().collect::<Vec<_>>(),
            std::slice::from_ref(&id_b)
        );
        ed.mark_saved();
        assert!(ed.dirty_pages().is_empty());
        ed.undo();
        assert!(
            ed.dirty_pages().contains(&id_b),
            "desfazer depois de salvar"
        );
        ed.redo();
        assert!(ed.dirty_pages().is_empty());
        // Desfazer um passo salvo e fazer outra coisa: a página continua marcada.
        ed.undo();
        let id_a = ed.doc.data.pages[0].id.clone();
        ed.insert_object(
            ObjLoc {
                page: 0,
                layer: 0,
                index: 0,
            },
            rect(),
        );
        let d = ed.dirty_pages();
        assert!(d.contains(&id_a) && d.contains(&id_b));
    }

    #[test]
    fn gd_pgt_21_master_page_is_editable_and_undoable() {
        use crate::edit::{MASTER, ObjLoc};
        let mut ed = doc(&["A"]);
        ed.insert_layer(MASTER, 0, crate::model::Layer::new("Fundo"));
        let o = rect();
        let id = o.id.clone();
        ed.insert_object(
            ObjLoc {
                page: MASTER,
                layer: 0,
                index: 0,
            },
            o,
        );
        assert_eq!(ed.find(&id).unwrap().loc.page, MASTER);
        assert!(ed.dirty_pages().contains(crate::edit::history::MASTER_KEY));
        ed.goto(0);
        assert!(ed.doc.data.master_page.layers.is_empty());
    }
}
