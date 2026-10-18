//! Objetos em qualquer nível da árvore (GD-SEL-02: editar dentro de grupo
//! sem desagrupar).
//!
//! Uma `ObjRef` é a posição do objeto de primeiro nível mais o caminho de
//! índices até o filho. Alterar um filho substitui o objeto de primeiro nível
//! inteiro no histórico; assim o registro de alterações (doc 12) continua
//! com um único tipo de troca por objeto e arrastos seguem virando um passo.

use kurbo::Affine;

use super::history::{Editor, ObjLoc};
use crate::model::{Object, ObjectKind};

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ObjRef {
    pub loc: ObjLoc,
    /// Índices nos filhos, do grupo de primeiro nível para baixo; vazio =
    /// o próprio objeto de primeiro nível.
    pub path: Vec<usize>,
}

/// Onde um objeto mora: direto na camada ou entre os filhos de um grupo.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Container {
    Layer { page: usize, layer: usize },
    Group(ObjRef),
}

impl ObjRef {
    pub fn top(loc: ObjLoc) -> ObjRef {
        ObjRef {
            loc,
            path: Vec::new(),
        }
    }

    pub fn is_top(&self) -> bool {
        self.path.is_empty()
    }

    /// Índice entre os irmãos (ordem de empilhamento).
    pub fn index(&self) -> usize {
        *self.path.last().unwrap_or(&self.loc.index)
    }

    pub fn container(&self) -> Container {
        if self.path.is_empty() {
            Container::Layer {
                page: self.loc.page,
                layer: self.loc.layer,
            }
        } else {
            Container::Group(ObjRef {
                loc: self.loc,
                path: self.path[..self.path.len() - 1].to_vec(),
            })
        }
    }

    /// Mesma posição, outro índice entre os irmãos.
    pub fn with_index(&self, i: usize) -> ObjRef {
        let mut r = self.clone();
        match r.path.last_mut() {
            Some(last) => *last = i,
            None => r.loc.index = i,
        }
        r
    }

    /// `self` está dentro de `other` (ou é o próprio)?
    pub fn is_within(&self, other: &ObjRef) -> bool {
        self.loc == other.loc && self.path.starts_with(&other.path)
    }
}

fn descend<'a>(o: &'a Object, path: &[usize]) -> Option<&'a Object> {
    let mut cur = o;
    for &i in path {
        match &cur.kind {
            ObjectKind::Group(g) => cur = g.children.get(i)?,
            _ => return None,
        }
    }
    Some(cur)
}

fn descend_mut<'a>(o: &'a mut Object, path: &[usize]) -> Option<&'a mut Object> {
    let mut cur = o;
    for &i in path {
        match &mut cur.kind {
            ObjectKind::Group(g) => cur = g.children.get_mut(i)?,
            _ => return None,
        }
    }
    Some(cur)
}

fn children_mut<'a>(o: &'a mut Object, path: &[usize]) -> Option<&'a mut Vec<Object>> {
    match &mut descend_mut(o, path)?.kind {
        ObjectKind::Group(g) => Some(&mut g.children),
        _ => None,
    }
}

fn search(o: &Object, id: &str, path: &mut Vec<usize>) -> bool {
    if let ObjectKind::Group(g) = &o.kind {
        for (i, c) in g.children.iter().enumerate() {
            path.push(i);
            if c.id == id || search(c, id, path) {
                return true;
            }
            path.pop();
        }
    }
    false
}

impl Editor {
    /// Localiza um objeto pelo ID em qualquer profundidade.
    pub fn find(&self, id: &str) -> Option<ObjRef> {
        for p in self.page_indices() {
            for (l, layer) in self.layers(p).iter().enumerate() {
                for (i, o) in layer.objects.iter().enumerate() {
                    let loc = ObjLoc {
                        page: p,
                        layer: l,
                        index: i,
                    };
                    if o.id == id {
                        return Some(ObjRef::top(loc));
                    }
                    let mut path = Vec::new();
                    if search(o, id, &mut path) {
                        return Some(ObjRef { loc, path });
                    }
                }
            }
        }
        None
    }

    pub fn get(&self, r: &ObjRef) -> Option<&Object> {
        descend(self.object(r.loc)?, &r.path)
    }

    /// Matriz dos ancestrais (local do pai → página); identidade no primeiro nível.
    pub fn parent_affine(&self, r: &ObjRef) -> Affine {
        let Some(mut cur) = self.object(r.loc) else {
            return Affine::IDENTITY;
        };
        let mut m = Affine::IDENTITY;
        for &i in &r.path {
            m *= cur.transform.affine();
            match &cur.kind {
                ObjectKind::Group(g) => match g.children.get(i) {
                    Some(c) => cur = c,
                    None => break,
                },
                _ => break,
            }
        }
        m
    }

    pub fn container_len(&self, c: &Container) -> usize {
        match c {
            Container::Layer { page, layer } => self.objects(*page, *layer).len(),
            Container::Group(g) => match self.get(g).map(|o| &o.kind) {
                Some(ObjectKind::Group(g)) => g.children.len(),
                _ => 0,
            },
        }
    }

    /// Objeto `index` do contêiner.
    pub fn child_ref(&self, c: &Container, index: usize) -> ObjRef {
        match c {
            Container::Layer { page, layer } => ObjRef::top(ObjLoc {
                page: *page,
                layer: *layer,
                index,
            }),
            Container::Group(g) => {
                let mut r = g.clone();
                r.path.push(index);
                r
            }
        }
    }

    pub fn replace_at(&mut self, r: &ObjRef, obj: Object) {
        if r.is_top() {
            self.replace_object(r.loc, obj);
            return;
        }
        let Some(mut top) = self.object(r.loc).cloned() else {
            return;
        };
        if let Some(slot) = descend_mut(&mut top, &r.path) {
            *slot = obj;
            self.replace_object(r.loc, top);
        }
    }

    /// Remove o objeto; um grupo que fica vazio também é removido.
    pub fn remove_at(&mut self, r: &ObjRef) -> Option<Object> {
        if r.is_top() {
            return Some(self.remove_object(r.loc));
        }
        let mut top = self.object(r.loc)?.clone();
        let (last, parent) = r.path.split_last()?;
        let list = children_mut(&mut top, parent)?;
        if *last >= list.len() {
            return None;
        }
        let removed = list.remove(*last);
        if list.is_empty() {
            self.remove_at(&ObjRef {
                loc: r.loc,
                path: parent.to_vec(),
            });
        } else {
            self.replace_object(r.loc, top);
        }
        Some(removed)
    }

    /// ID do objeto `index` do contêiner.
    pub fn id_in(&self, c: &Container, index: usize) -> Option<&str> {
        self.get(&self.child_ref(c, index)).map(|o| o.id.as_str())
    }

    /// Posição de `id` entre os objetos do contêiner.
    pub fn position_in(&self, c: &Container, id: &str) -> Option<usize> {
        (0..self.container_len(c)).find(|i| self.id_in(c, *i) == Some(id))
    }

    /// Muda a posição de empilhamento dentro do contêiner.
    pub fn move_in(&mut self, c: &Container, from: usize, to: usize) {
        if from == to {
            return;
        }
        match c {
            Container::Layer { page, layer } => self.move_object(
                ObjLoc {
                    page: *page,
                    layer: *layer,
                    index: from,
                },
                to,
            ),
            Container::Group(g) => self.edit_children(g, |list| {
                let o = list.remove(from);
                list.insert(to.min(list.len()), o);
            }),
        }
    }

    /// Edita a lista de filhos do grupo `g` numa única troca no histórico.
    pub fn edit_children(&mut self, g: &ObjRef, f: impl FnOnce(&mut Vec<Object>)) {
        let Some(mut top) = self.object(g.loc).cloned() else {
            return;
        };
        if let Some(list) = children_mut(&mut top, &g.path) {
            f(list);
            self.replace_object(g.loc, top);
        }
    }

    /// Insere no contêiner na posição `index` (limitada ao tamanho).
    pub fn insert_at(&mut self, c: &Container, index: usize, obj: Object) {
        match c {
            Container::Layer { page, layer } => self.insert_object(
                ObjLoc {
                    page: *page,
                    layer: *layer,
                    index,
                },
                obj,
            ),
            Container::Group(g) => self.edit_children(g, |list| {
                let i = index.min(list.len());
                list.insert(i, obj);
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Document, GroupData, Page, RectData, Transform};
    use serde_json::Map;

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

    fn group(children: Vec<Object>, x: f64) -> Object {
        let mut g = Object::new(ObjectKind::Group(GroupData {
            children,
            boolean: None,
            symmetry: None,
            extra: Map::new(),
        }));
        g.transform = Transform::translate(x, 0.0);
        g
    }

    #[test]
    fn find_get_replace_remove_nested() {
        let inner = rect(1.0);
        let inner_id = inner.id.clone();
        let other = rect(2.0);
        let other_id = other.id.clone();
        let g = group(vec![group(vec![inner, other], 100.0), rect(3.0)], 1000.0);
        let mut d = Document::new();
        let mut p = Page::new(10.0, 10.0);
        p.layers[0].objects.push(rect(0.0));
        p.layers[0].objects.push(g);
        d.data.pages.push(p);
        let mut ed = Editor::new(d);

        let r = ed.find(&inner_id).unwrap();
        assert_eq!(r.path, vec![0, 0]);
        assert_eq!(r.loc.index, 1);
        assert_eq!(
            ed.parent_affine(&r),
            Affine::translate((1100.0, 0.0)),
            "grupo externo × grupo interno"
        );

        let mut o = ed.get(&r).unwrap().clone();
        o.transform = Transform::translate(5.0, 5.0);
        ed.replace_at(&r, o.clone());
        assert_eq!(ed.get(&r), Some(&o));
        ed.undo();
        assert_eq!(
            ed.get(&r).unwrap().transform,
            Transform::translate(1.0, 0.0)
        );

        // Remover os dois filhos do grupo interno remove também o grupo.
        ed.transaction("Excluir", |ed| {
            let a = ed.find(&other_id).unwrap();
            ed.remove_at(&a);
            let b = ed.find(&inner_id).unwrap();
            ed.remove_at(&b);
        });
        let top = ed.object(ObjLoc {
            page: 0,
            layer: 0,
            index: 1,
        });
        match &top.unwrap().kind {
            ObjectKind::Group(g) => assert_eq!(g.children.len(), 1),
            _ => panic!(),
        }
        ed.undo();
        assert!(ed.find(&inner_id).is_some() && ed.find(&other_id).is_some());
    }
}
