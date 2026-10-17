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
//!
//! Cada transação também guarda os IDs das páginas que tocou, para o
//! marcador de "página alterada" nas abas (GD-PGT-05).
//!
//! A página mestra (GD-CAM-02, GD-PGT-21) é endereçada pelo índice
//! especial [`MASTER`]: objetos e camadas dela passam pelo mesmo registro.

use std::collections::BTreeSet;

use crate::model::{Document, Layer, Object, Page};

/// Índice de página que designa a página mestra.
pub const MASTER: usize = usize::MAX;

/// Chave da página mestra no conjunto de páginas alteradas.
pub const MASTER_KEY: &str = "master";

fn layers_of(doc: &Document, page: usize) -> Option<&Vec<Layer>> {
    if page == MASTER {
        Some(&doc.data.master_page.layers)
    } else {
        doc.data.pages.get(page).map(|p| &p.layers)
    }
}

fn layers_mut(doc: &mut Document, page: usize) -> &mut Vec<Layer> {
    if page == MASTER {
        &mut doc.data.master_page.layers
    } else {
        &mut doc.data.pages[page].layers
    }
}

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
    InsertLayer(usize, usize, Layer),
    RemoveLayer(usize, usize, Layer),
    /// Propriedades da camada (GD-CAM-01); os objetos não entram na troca.
    LayerProps(usize, usize, Layer, Layer),
    /// Propriedades da página (nome, tamanho, sangria, bloqueio, etiqueta);
    /// as camadas não entram na troca.
    PageProps(usize, Page, Page),
    /// Configurações do documento (cor, unidade, resolução…).
    Settings(Box<crate::model::Settings>, Box<crate::model::Settings>),
    /// Cores nomeadas do documento (paleta, GD-PAL-01).
    Colors(
        Box<crate::model::ColorsResource>,
        Box<crate::model::ColorsResource>,
    ),
}

/// A página sem as camadas (o que `PageProps` guarda).
fn page_props(p: &Page) -> Page {
    p.props_only()
}

/// Copia as propriedades de `src` para `dst`, mantendo as camadas de `dst`.
fn copy_page_props(dst: &mut Page, src: &Page) {
    let layers = std::mem::take(&mut dst.layers);
    *dst = Page {
        layers,
        ..src.props_only()
    };
}

/// Copia as propriedades de `src` para `dst`, mantendo os objetos de `dst`.
fn copy_layer_props(dst: &mut Layer, src: &Layer) {
    let objects = std::mem::take(&mut dst.objects);
    *dst = Layer {
        objects,
        ..layer_props(src)
    };
}

/// A camada sem os objetos (o que `LayerProps` guarda).
fn layer_props(l: &Layer) -> Layer {
    // Campo a campo: `..l.clone()` copiaria todos os objetos só para descartá-los.
    Layer {
        id: l.id.clone(),
        name: l.name.clone(),
        visible: l.visible,
        printable: l.printable,
        locked: l.locked,
        color: l.color.clone(),
        wireframe: l.wireframe,
        objects: Vec::new(),
        extra: l.extra.clone(),
    }
}

#[derive(Debug, Clone)]
struct Transaction {
    id: u64,
    name: String,
    changes: Vec<Change>,
    /// IDs das páginas tocadas (`MASTER_KEY` para a mestra).
    pages: BTreeSet<String>,
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
    /// Páginas de passos já salvos que foram desfeitos e descartados por uma
    /// ação nova: o documento salvo e o atual diferem nelas.
    discarded: BTreeSet<String>,
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
            discarded: BTreeSet::new(),
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
                pages: BTreeSet::new(),
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
        for r in std::mem::take(&mut self.redo) {
            if r.id <= self.saved_state {
                self.discarded.extend(r.pages);
            }
        }
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
    /// Troca o nome da transação aberta (ex.: "Mover" vira "Mover e copiar 2").
    pub fn rename_open(&mut self, name: impl Into<String>) {
        if let Some(t) = &mut self.open {
            t.name = name.into();
        }
    }

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

    fn page_key(&self, page: usize) -> Option<String> {
        if page == MASTER {
            Some(MASTER_KEY.into())
        } else {
            self.doc.data.pages.get(page).map(|p| p.id.clone())
        }
    }

    fn record(&mut self, c: Change) {
        self.revision += 1;
        let key = match &c {
            Change::InsertObject(l, _)
            | Change::RemoveObject(l, _)
            | Change::ReplaceObject(l, _, _) => self.page_key(l.page),
            Change::InsertPage(_, p)
            | Change::RemovePage(_, p)
            | Change::ReplacePage(_, _, p)
            | Change::PageProps(_, _, p) => Some(p.id.clone()),
            Change::InsertLayer(p, _, _)
            | Change::RemoveLayer(p, _, _)
            | Change::LayerProps(p, _, _, _) => self.page_key(*p),
            Change::Settings(..) | Change::Colors(..) => None,
        };
        let auto = self.open.is_none();
        if auto {
            self.begin("Editar");
        }
        let t = self.open.as_mut().unwrap();
        t.pages.extend(key);
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

    /// Camadas da página (ou da mestra, com `MASTER`).
    pub fn layers(&self, page: usize) -> &[Layer] {
        layers_of(&self.doc, page).map_or(&[], |l| l.as_slice())
    }

    pub fn objects(&self, page: usize, layer: usize) -> &[Object] {
        &self.layers(page)[layer].objects
    }

    pub fn object(&self, loc: ObjLoc) -> Option<&Object> {
        layers_of(&self.doc, loc.page)?
            .get(loc.layer)?
            .objects
            .get(loc.index)
    }

    /// Índices de todas as páginas, com a mestra por último.
    pub fn page_indices(&self) -> impl Iterator<Item = usize> + use<> {
        (0..self.doc.data.pages.len()).chain(std::iter::once(MASTER))
    }

    /// Localiza um objeto de primeiro nível pelo ID.
    pub fn locate(&self, id: &str) -> Option<ObjLoc> {
        for p in self.page_indices() {
            for (l, layer) in self.layers(p).iter().enumerate() {
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
        let list = &mut layers_mut(&mut self.doc, loc.page)[loc.layer].objects;
        let i = loc.index.min(list.len());
        list.insert(i, obj.clone());
        self.record(Change::InsertObject(ObjLoc { index: i, ..loc }, obj));
    }

    pub fn remove_object(&mut self, loc: ObjLoc) -> Object {
        let obj = layers_mut(&mut self.doc, loc.page)[loc.layer]
            .objects
            .remove(loc.index);
        self.record(Change::RemoveObject(loc, obj.clone()));
        obj
    }

    pub fn replace_object(&mut self, loc: ObjLoc, obj: Object) {
        let slot = &mut layers_mut(&mut self.doc, loc.page)[loc.layer].objects[loc.index];
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

    /// Move a página para outra posição (um passo só no histórico).
    pub fn move_page(&mut self, from: usize, to: usize) {
        if from == to || from >= self.doc.data.pages.len() {
            return;
        }
        self.transaction("Mover página", |ed| {
            let p = ed.remove_page(from);
            ed.insert_page(to, p);
        });
    }

    /// Altera propriedades da página sem tocar nas camadas e objetos.
    pub fn edit_page(&mut self, index: usize, f: impl FnOnce(&mut Page)) {
        let page = &mut self.doc.data.pages[index];
        let old = page_props(page);
        let mut new = old.clone();
        f(&mut new);
        new.layers = Vec::new();
        if new == old {
            return;
        }
        copy_page_props(page, &new);
        self.record(Change::PageProps(index, old, new));
    }

    /// Altera as cores nomeadas do documento.
    pub fn edit_colors(&mut self, f: impl FnOnce(&mut crate::model::ColorsResource)) {
        let old = self.doc.colors.clone();
        let mut new = old.clone();
        f(&mut new);
        if new == old {
            return;
        }
        self.doc.colors = new.clone();
        self.record(Change::Colors(Box::new(old), Box::new(new)));
    }

    /// Altera as configurações do documento (um passo no histórico).
    pub fn edit_settings(&mut self, f: impl FnOnce(&mut crate::model::Settings)) {
        let old = self.doc.data.settings.clone();
        let mut new = old.clone();
        f(&mut new);
        if new == old {
            return;
        }
        self.doc.data.settings = new.clone();
        self.record(Change::Settings(Box::new(old), Box::new(new)));
    }

    pub fn replace_page(&mut self, index: usize, page: Page) {
        if self.doc.data.pages[index] == page {
            return;
        }
        let old = std::mem::replace(&mut self.doc.data.pages[index], page.clone());
        self.record(Change::ReplacePage(index, old, page));
    }

    // ---- camadas (GD-CAM-01, GD-CAM-03) --------------------------------------

    pub fn insert_layer(&mut self, page: usize, index: usize, layer: Layer) {
        let layers = layers_mut(&mut self.doc, page);
        let i = index.min(layers.len());
        layers.insert(i, layer.clone());
        self.record(Change::InsertLayer(page, i, layer));
    }

    pub fn remove_layer(&mut self, page: usize, index: usize) -> Layer {
        let layer = layers_mut(&mut self.doc, page).remove(index);
        self.record(Change::RemoveLayer(page, index, layer.clone()));
        layer
    }

    /// Altera propriedades da camada (nome, visível, imprimível, bloqueada,
    /// cor, esboço) sem tocar nos objetos.
    pub fn edit_layer(&mut self, page: usize, index: usize, f: impl FnOnce(&mut Layer)) {
        let layer = &mut layers_mut(&mut self.doc, page)[index];
        let old = layer_props(layer);
        let mut new = old.clone();
        f(&mut new);
        new.objects = Vec::new();
        if new == old {
            return;
        }
        copy_layer_props(layer, &new);
        self.record(Change::LayerProps(page, index, old, new));
    }

    /// Move a camada para outra posição de empilhamento.
    pub fn move_layer(&mut self, page: usize, from: usize, to: usize) {
        if from == to {
            return;
        }
        self.transaction("Mover camada", |ed| {
            let l = ed.remove_layer(page, from);
            ed.insert_layer(page, to, l);
        });
    }

    // ---- desfazer/refazer -------------------------------------------------

    fn apply_inverse(&mut self, c: &Change) {
        self.revision += 1;
        let doc = &mut self.doc;
        match c {
            Change::InsertObject(l, _) => {
                layers_mut(doc, l.page)[l.layer].objects.remove(l.index);
            }
            Change::RemoveObject(l, o) => layers_mut(doc, l.page)[l.layer]
                .objects
                .insert(l.index, o.clone()),
            Change::ReplaceObject(l, old, _) => {
                layers_mut(doc, l.page)[l.layer].objects[l.index] = old.clone()
            }
            Change::InsertPage(i, _) => {
                doc.data.pages.remove(*i);
            }
            Change::RemovePage(i, p) => doc.data.pages.insert(*i, p.clone()),
            Change::ReplacePage(i, old, _) => doc.data.pages[*i] = old.clone(),
            Change::InsertLayer(p, i, _) => {
                layers_mut(doc, *p).remove(*i);
            }
            Change::RemoveLayer(p, i, l) => layers_mut(doc, *p).insert(*i, l.clone()),
            Change::LayerProps(p, i, old, _) => copy_layer_props(&mut layers_mut(doc, *p)[*i], old),
            Change::PageProps(i, old, _) => copy_page_props(&mut doc.data.pages[*i], old),
            Change::Settings(old, _) => doc.data.settings = (**old).clone(),
            Change::Colors(old, _) => doc.colors = (**old).clone(),
        }
    }

    fn apply(&mut self, c: &Change) {
        self.revision += 1;
        let doc = &mut self.doc;
        match c {
            Change::InsertObject(l, o) => layers_mut(doc, l.page)[l.layer]
                .objects
                .insert(l.index, o.clone()),
            Change::RemoveObject(l, _) => {
                layers_mut(doc, l.page)[l.layer].objects.remove(l.index);
            }
            Change::ReplaceObject(l, _, new) => {
                layers_mut(doc, l.page)[l.layer].objects[l.index] = new.clone()
            }
            Change::InsertPage(i, p) => doc.data.pages.insert(*i, p.clone()),
            Change::RemovePage(i, _) => {
                doc.data.pages.remove(*i);
            }
            Change::ReplacePage(i, _, new) => doc.data.pages[*i] = new.clone(),
            Change::InsertLayer(p, i, l) => layers_mut(doc, *p).insert(*i, l.clone()),
            Change::RemoveLayer(p, i, _) => {
                layers_mut(doc, *p).remove(*i);
            }
            Change::LayerProps(p, i, _, new) => copy_layer_props(&mut layers_mut(doc, *p)[*i], new),
            Change::PageProps(i, _, new) => copy_page_props(&mut doc.data.pages[*i], new),
            Change::Settings(_, new) => doc.data.settings = (**new).clone(),
            Change::Colors(_, new) => doc.colors = (**new).clone(),
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

    /// Documento recuperado: conta como alterado até salvar.
    pub fn mark_unsaved(&mut self) {
        self.saved_state = u64::MAX;
    }

    pub fn mark_saved(&mut self) {
        self.saved_state = self.state();
        self.discarded.clear();
    }

    /// IDs das páginas (e `MASTER_KEY`) em que o documento atual difere do
    /// último salvo (GD-PGT-05).
    pub fn dirty_pages(&self) -> BTreeSet<String> {
        let saved = self.saved_state;
        let mut out = self.discarded.clone();
        for t in self.undo.iter().filter(|t| t.id > saved) {
            out.extend(t.pages.iter().cloned());
        }
        for t in self.redo.iter().filter(|t| t.id <= saved) {
            out.extend(t.pages.iter().cloned());
        }
        out
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
    fn layers_undo() {
        let mut ed = Editor::new(doc());
        ed.insert_object(at(0), rect(0.0));
        ed.insert_layer(0, 1, Layer::new("Camada 2"));
        ed.edit_layer(0, 0, |l| {
            l.name = "Fundo".into();
            l.locked = true;
        });
        assert_eq!(ed.doc.data.pages[0].layers[0].objects.len(), 1);
        ed.move_layer(0, 0, 1);
        assert_eq!(ed.doc.data.pages[0].layers[1].name, "Fundo");
        assert_eq!(ed.doc.data.pages[0].layers[1].objects.len(), 1);
        ed.remove_layer(0, 0);
        ed.goto(1);
        let layers = &ed.doc.data.pages[0].layers;
        assert_eq!(layers.len(), 1);
        assert_eq!(layers[0].name, "Camada 1");
        assert!(!layers[0].locked);
        assert_eq!(layers[0].objects.len(), 1, "objetos intactos");
        ed.goto(5);
        assert_eq!(ed.doc.data.pages[0].layers[0].name, "Fundo");
    }

    #[test]
    fn page_props_keep_objects() {
        let mut ed = Editor::new(doc());
        ed.insert_object(at(0), rect(0.0));
        ed.edit_page(0, |p| {
            p.name = Some("Capa".into());
            p.locked = true;
            p.width = 500.0;
        });
        let p = &ed.doc.data.pages[0];
        assert_eq!(p.name.as_deref(), Some("Capa"));
        assert_eq!(p.object_count(), 1);
        ed.undo();
        let p = &ed.doc.data.pages[0];
        assert!(!p.locked && p.name.is_none() && p.width == 100.0);
        assert_eq!(p.object_count(), 1);
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
