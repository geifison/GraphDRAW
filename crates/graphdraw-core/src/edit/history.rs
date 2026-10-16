//! Editor com transações e desfazer/refazer (doc 12).
//!
//! Escolha de implementação (GD-UND-08): **registro de alterações
//! reversíveis**. Cada transação guarda, para cada objeto ou página tocado,
//! o valor anterior e o novo — e só deles, não do documento inteiro. Desfazer
//! aplica os inversos em ordem contrária.
//!
//! - GD-UND-01: toda alteração passa por uma transação com nome; arrastos
//!   longos substituem o mesmo objeto várias vezes e o registro guarda só o
//!   primeiro "antes" e o último "depois" (um passo ao soltar).
//! - GD-UND-02: limite configurável (padrão 100; 0 = ilimitado).
//! - GD-UND-03: nova ação após desfazer descarta o refazer.
//! - GD-UND-04: lista de nomes e salto para qualquer ponto (`goto`).
//! - GD-UND-05: transações aninhadas viram um passo só.
//! - GD-UND-06: seleção, zoom e rolagem ficam fora daqui (estado da interface).
//! - GD-UND-07: "modificado" compara a posição atual com a do último salvamento.

use crate::model::{Document, Object, Page};

/// Posição de um objeto de primeiro nível: página, camada e índice (z-order).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ObjLoc {
    pub page: usize,
    pub layer: usize,
    pub index: usize,
}

#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
enum Change {
    InsertObject(ObjLoc, Object),
    RemoveObject(ObjLoc, Object),
    ReplaceObject(ObjLoc, Object, Object),
    InsertPage(usize, Page),
    RemovePage(usize, Page),
    /// Página inteira (propriedades de página/camadas).
    ReplacePage(usize, Page, Page),
}

#[derive(Debug, Clone)]
struct Transaction {
    id: u64,
    name: String,
    changes: Vec<Change>,
}

pub const DEFAULT_UNDO_LIMIT: usize = 100;

pub struct Editor {
    pub doc: Document,
    undo: Vec<Transaction>,
    redo: Vec<Transaction>,
    open: Option<Transaction>,
    depth: usize,
    next_id: u64,
    saved_state: u64,
    limit: usize,
    revision: u64,
}

impl Editor {
    pub fn new(doc: Document) -> Editor {
        Editor {
            doc,
            undo: Vec::new(),
            redo: Vec::new(),
            open: None,
            depth: 0,
            next_id: 1,
            saved_state: 0,
            limit: DEFAULT_UNDO_LIMIT,
            revision: 0,
        }
    }

    /// Número máximo de passos (0 = ilimitado).
    pub fn set_undo_limit(&mut self, limit: usize) {
        self.limit = limit;
        self.trim();
    }

    // ---- transações -----------------------------------------------------

    /// Abre uma transação. Chamadas aninhadas só aumentam a profundidade; o
    /// nome que vale é o da mais externa.
    pub fn begin(&mut self, name: impl Into<String>) {
        if self.depth == 0 {
            self.open = Some(Transaction {
                id: 0,
                name: name.into(),
                changes: Vec::new(),
            });
        }
        self.depth += 1;
    }

    /// Fecha a transação; ao fechar a mais externa, ela entra na pilha (se
    /// tiver alterações).
    pub fn commit(&mut self) {
        if self.depth == 0 {
            return;
        }
        self.depth -= 1;
        if self.depth > 0 {
            return;
        }
        let Some(mut t) = self.open.take() else {
            return;
        };
        if t.changes.is_empty() {
            return;
        }
        t.id = self.next_id;
        self.next_id += 1;
        self.undo.push(t);
        self.redo.clear();
        self.trim();
    }

    /// Desfaz as alterações da transação aberta e a descarta.
    pub fn cancel(&mut self) {
        if let Some(t) = self.open.take() {
            for c in t.changes.iter().rev() {
                self.apply_inverse(c);
            }
        }
        self.depth = 0;
    }

    /// Executa `f` dentro de uma transação.
    pub fn transaction<R>(
        &mut self,
        name: impl Into<String>,
        f: impl FnOnce(&mut Editor) -> R,
    ) -> R {
        self.begin(name);
        let r = f(self);
        self.commit();
        r
    }

    /// Muda a cada alteração do documento (inclusive desfazer/refazer);
    /// usado pela interface para invalidar caches.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn in_transaction(&self) -> bool {
        self.depth > 0
    }

    fn trim(&mut self) {
        if self.limit > 0 && self.undo.len() > self.limit {
            let extra = self.undo.len() - self.limit;
            self.undo.drain(..extra);
        }
    }

    fn record(&mut self, c: Change) {
        self.revision += 1;
        let auto = self.open.is_none();
        if auto {
            self.begin("Editar");
        }
        let t = self.open.as_mut().unwrap();
        // Arrasto: substituições seguidas do mesmo objeto viram uma só.
        if let (
            Change::ReplaceObject(loc, _, new),
            Some(Change::ReplaceObject(last_loc, _, last_new)),
        ) = (&c, t.changes.last_mut())
            && loc == last_loc
        {
            *last_new = new.clone();
            if auto {
                self.commit();
            }
            return;
        }
        t.changes.push(c);
        if auto {
            self.commit();
        }
    }

    // ---- alterações -----------------------------------------------------

    pub fn objects(&self, page: usize, layer: usize) -> &[Object] {
        &self.doc.data.pages[page].layers[layer].objects
    }

    pub fn object(&self, loc: ObjLoc) -> Option<&Object> {
        self.doc
            .data
            .pages
            .get(loc.page)?
            .layers
            .get(loc.layer)?
            .objects
            .get(loc.index)
    }

    /// Localiza um objeto de primeiro nível pelo ID.
    pub fn locate(&self, id: &str) -> Option<ObjLoc> {
        for (p, page) in self.doc.data.pages.iter().enumerate() {
            for (l, layer) in page.layers.iter().enumerate() {
                if let Some(i) = layer.objects.iter().position(|o| o.id == id) {
                    return Some(ObjLoc {
                        page: p,
                        layer: l,
                        index: i,
                    });
                }
            }
        }
        None
    }

    pub fn insert_object(&mut self, loc: ObjLoc, obj: Object) {
        let list = &mut self.doc.data.pages[loc.page].layers[loc.layer].objects;
        let i = loc.index.min(list.len());
        list.insert(i, obj.clone());
        self.record(Change::InsertObject(ObjLoc { index: i, ..loc }, obj));
    }

    pub fn remove_object(&mut self, loc: ObjLoc) -> Object {
        let obj = self.doc.data.pages[loc.page].layers[loc.layer]
            .objects
            .remove(loc.index);
        self.record(Change::RemoveObject(loc, obj.clone()));
        obj
    }

    pub fn replace_object(&mut self, loc: ObjLoc, obj: Object) {
        let slot = &mut self.doc.data.pages[loc.page].layers[loc.layer].objects[loc.index];
        if *slot == obj {
            return;
        }
        let old = std::mem::replace(slot, obj.clone());
        self.record(Change::ReplaceObject(loc, old, obj));
    }

    /// Move um objeto para outra posição de z-order na mesma camada.
    pub fn move_object(&mut self, loc: ObjLoc, to_index: usize) {
        if loc.index == to_index {
            return;
        }
        let obj = self.remove_object(loc);
        self.insert_object(
            ObjLoc {
                index: to_index,
                ..loc
            },
            obj,
        );
    }

    pub fn insert_page(&mut self, index: usize, page: Page) {
        let i = index.min(self.doc.data.pages.len());
        self.doc.data.pages.insert(i, page.clone());
        self.record(Change::InsertPage(i, page));
    }

    pub fn remove_page(&mut self, index: usize) -> Page {
        let page = self.doc.data.pages.remove(index);
        self.record(Change::RemovePage(index, page.clone()));
        page
    }

    pub fn replace_page(&mut self, index: usize, page: Page) {
        if self.doc.data.pages[index] == page {
            return;
        }
        let old = std::mem::replace(&mut self.doc.data.pages[index], page.clone());
        self.record(Change::ReplacePage(index, old, page));
    }

    // ---- desfazer/refazer -------------------------------------------------

    fn apply_inverse(&mut self, c: &Change) {
        self.revision += 1;
        let pages = &mut self.doc.data.pages;
        match c {
            Change::InsertObject(l, _) => {
                pages[l.page].layers[l.layer].objects.remove(l.index);
            }
            Change::RemoveObject(l, o) => pages[l.page].layers[l.layer]
                .objects
                .insert(l.index, o.clone()),
            Change::ReplaceObject(l, old, _) => {
                pages[l.page].layers[l.layer].objects[l.index] = old.clone()
            }
            Change::InsertPage(i, _) => {
                pages.remove(*i);
            }
            Change::RemovePage(i, p) => pages.insert(*i, p.clone()),
            Change::ReplacePage(i, old, _) => pages[*i] = old.clone(),
        }
    }

    fn apply(&mut self, c: &Change) {
        self.revision += 1;
        let pages = &mut self.doc.data.pages;
        match c {
            Change::InsertObject(l, o) => pages[l.page].layers[l.layer]
                .objects
                .insert(l.index, o.clone()),
            Change::RemoveObject(l, _) => {
                pages[l.page].layers[l.layer].objects.remove(l.index);
            }
            Change::ReplaceObject(l, _, new) => {
                pages[l.page].layers[l.layer].objects[l.index] = new.clone()
            }
            Change::InsertPage(i, p) => pages.insert(*i, p.clone()),
            Change::RemovePage(i, _) => {
                pages.remove(*i);
            }
            Change::ReplacePage(i, _, new) => pages[*i] = new.clone(),
        }
    }

    /// Desfaz o último passo; devolve seu nome.
    pub fn undo(&mut self) -> Option<String> {
        if self.depth > 0 {
            return None;
        }
        let t = self.undo.pop()?;
        for c in t.changes.iter().rev() {
            self.apply_inverse(c);
        }
        let name = t.name.clone();
        self.redo.push(t);
        Some(name)
    }

    pub fn redo(&mut self) -> Option<String> {
        if self.depth > 0 {
            return None;
        }
        let t = self.redo.pop()?;
        for c in &t.changes {
            self.apply(c);
        }
        let name = t.name.clone();
        self.undo.push(t);
        Some(name)
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Histórico em ordem cronológica: (nome, já aplicado?). Posições
    /// `0..undo_len()` estão aplicadas; as seguintes podem ser refeitas.
    pub fn history(&self) -> Vec<(String, bool)> {
        self.undo
            .iter()
            .map(|t| (t.name.clone(), true))
            .chain(self.redo.iter().rev().map(|t| (t.name.clone(), false)))
            .collect()
    }

    pub fn undo_len(&self) -> usize {
        self.undo.len()
    }

    /// Volta (ou avança) até que exatamente `applied` passos estejam aplicados.
    pub fn goto(&mut self, applied: usize) {
        while self.undo.len() > applied && self.undo().is_some() {}
        while self.undo.len() < applied && self.redo().is_some() {}
    }

    // ---- estado salvo -------------------------------------------------------

    fn state(&self) -> u64 {
        self.undo.last().map_or(0, |t| t.id)
    }

    pub fn mark_saved(&mut self) {
        self.saved_state = self.state();
    }

    pub fn is_modified(&self) -> bool {
        self.state() != self.saved_state
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Fill, ObjectKind, RectData, Transform};
    use serde_json::Map;

    fn doc() -> Document {
        let mut d = Document::new();
        d.data.pages.push(Page::new(100.0, 100.0));
        d
    }

    fn rect(x: f64) -> Object {
        let mut o = Object::new(ObjectKind::Rectangle(RectData {
            width: 10.0,
            height: 10.0,
            corners: None,
            extra: Map::new(),
        }));
        o.transform = Transform::translate(x, 0.0);
        o
    }

    fn at(i: usize) -> ObjLoc {
        ObjLoc {
            page: 0,
            layer: 0,
            index: i,
        }
    }

    /// Aceite do doc 12: criar 3 objetos, mover 1, desfazer 2× → só os 3
    /// objetos na posição original de criação, documento "modificado";
    /// refazer 1× → objeto movido de novo.
    #[test]
    fn gd_und_acceptance() {
        let mut ed = Editor::new(doc());
        ed.mark_saved();
        for (i, x) in [0.0, 20.0, 40.0].into_iter().enumerate() {
            ed.transaction(format!("Criar retângulo {i}"), |ed| {
                ed.insert_object(at(i), rect(x))
            });
        }
        let original = ed.objects(0, 0).to_vec();
        ed.transaction("Mover 1 objeto", |ed| {
            let mut o = ed.object(at(1)).unwrap().clone();
            o.transform = Transform::translate(25.0, 5.0);
            ed.replace_object(at(1), o);
        });
        let moved = ed.objects(0, 0)[1].clone();
        assert_eq!(ed.undo().as_deref(), Some("Mover 1 objeto"));
        assert_eq!(ed.undo().as_deref(), Some("Criar retângulo 2"));
        assert_eq!(ed.objects(0, 0), &original[..2]);
        assert!(ed.is_modified());
        assert_eq!(ed.redo().as_deref(), Some("Criar retângulo 2"));
        assert_eq!(ed.objects(0, 0), &original[..]);
        assert_eq!(ed.redo().as_deref(), Some("Mover 1 objeto"));
        assert_eq!(ed.objects(0, 0)[1], moved);
    }

    #[test]
    fn gd_und_01_drag_is_one_step() {
        let mut ed = Editor::new(doc());
        ed.insert_object(at(0), rect(0.0));
        let before = ed.objects(0, 0)[0].clone();
        ed.begin("Mover 1 objeto");
        for x in 1..50 {
            let mut o = ed.object(at(0)).unwrap().clone();
            o.transform = Transform::translate(x as f64, 0.0);
            ed.replace_object(at(0), o);
        }
        ed.commit();
        assert_eq!(ed.history().len(), 2);
        ed.undo();
        assert_eq!(ed.objects(0, 0)[0], before);
    }

    #[test]
    fn gd_und_02_limit_and_03_redo_discarded() {
        let mut ed = Editor::new(doc());
        ed.set_undo_limit(3);
        for i in 0..5 {
            ed.insert_object(at(i), rect(i as f64));
        }
        assert_eq!(ed.history().len(), 3);
        ed.undo();
        assert!(ed.can_redo());
        ed.insert_object(at(0), rect(99.0));
        assert!(!ed.can_redo(), "nova ação descarta o refazer");
    }

    #[test]
    fn gd_und_04_goto_and_05_nested() {
        let mut ed = Editor::new(doc());
        ed.transaction("Composto", |ed| {
            ed.insert_object(at(0), rect(0.0));
            ed.transaction("Sub", |ed| ed.insert_object(at(1), rect(1.0)));
            let mut o = ed.object(at(0)).unwrap().clone();
            o.fill = Some(Fill::solid(crate::model::Color::black()));
            ed.replace_object(at(0), o);
        });
        assert_eq!(ed.history(), vec![("Composto".to_string(), true)]);
        ed.transaction("A", |ed| ed.remove_object(at(1)));
        ed.transaction("B", |ed| ed.insert_object(at(0), rect(5.0)));
        ed.goto(1);
        assert_eq!(ed.objects(0, 0).len(), 2);
        assert_eq!(ed.history()[2], ("B".to_string(), false));
        ed.goto(0);
        assert!(ed.objects(0, 0).is_empty());
        ed.goto(3);
        assert_eq!(ed.objects(0, 0).len(), 2);
    }

    #[test]
    fn gd_und_07_modified_tracks_saved_position() {
        let mut ed = Editor::new(doc());
        assert!(!ed.is_modified());
        ed.insert_object(at(0), rect(0.0));
        assert!(ed.is_modified());
        ed.mark_saved();
        assert!(!ed.is_modified());
        ed.undo();
        assert!(ed.is_modified());
        ed.redo();
        assert!(!ed.is_modified());
    }

    #[test]
    fn cancel_rolls_back_open_transaction() {
        let mut ed = Editor::new(doc());
        ed.begin("Desenhar");
        ed.insert_object(at(0), rect(0.0));
        ed.cancel();
        assert!(ed.objects(0, 0).is_empty());
        assert!(!ed.can_undo());
    }

    #[test]
    fn pages_undo() {
        let mut ed = Editor::new(doc());
        ed.insert_page(1, Page::new(50.0, 50.0));
        let mut p = ed.doc.data.pages[0].clone();
        p.width = 300.0;
        ed.replace_page(0, p);
        ed.remove_page(1);
        assert_eq!(ed.doc.data.pages.len(), 1);
        ed.goto(0);
        assert_eq!(ed.doc.data.pages.len(), 1);
        assert_eq!(ed.doc.data.pages[0].width, 100.0);
    }
}
