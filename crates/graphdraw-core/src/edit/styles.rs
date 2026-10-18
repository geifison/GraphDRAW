//! Estilos e símbolos do documento (doc 11, primeiro lote):
//!
//! - **Estilo de objeto**: preenchimento e contorno com nome; os objetos
//!   ligados (`style`) mudam juntos quando o estilo é redefinido.
//! - **Estilo de texto**: família, corpo, peso, itálico, alinhamento e
//!   cor; os textos ligados (`textStyle`) mudam juntos.
//! - **Símbolo**: um conjunto de objetos mestre; cada instância é um grupo
//!   ligado (`symbol`) com cópias dele. Redefinir o símbolo troca as cópias
//!   em todas as instâncias.
//!
//! Tudo num passo do histórico; os dados ficam em `resources/styles.json`.

use kurbo::{Affine, Point};

use super::history::Editor;
use crate::model::{
    Document, GroupData, Object, ObjectKind, ObjectStyle, SymbolDef, TextStyleDef, Transform,
    new_id,
};

/// IDs dos objetos (em qualquer nível) que satisfazem `f`.
fn ids_where(doc: &Document, f: impl Fn(&Object) -> bool) -> Vec<String> {
    let mut out = Vec::new();
    doc.walk_objects(&mut |o| {
        if f(o) {
            out.push(o.id.clone());
        }
    });
    out
}

// ---- estilos de objeto ---------------------------------------------------------

/// Novo estilo com o preenchimento e o contorno de `from`; `from` fica ligado.
pub fn create_object_style(ed: &mut Editor, name: &str, from: &str) -> Option<String> {
    let o = ed.get(&ed.find(from)?)?.clone();
    let id = new_id();
    let st = ObjectStyle {
        id: id.clone(),
        name: name.into(),
        fill: o.fill.clone(),
        stroke: o.stroke.clone(),
    };
    ed.begin("Novo estilo de objeto");
    ed.edit_styles(|s| s.object.push(st));
    super::ops::modify(ed, &[from.to_string()], "Ligar ao estilo", |o| {
        o.style = Some(id.clone());
        true
    });
    ed.commit();
    Some(id)
}

/// Aplica o estilo `id` à seleção (preenchimento, contorno e ligação).
pub fn apply_object_style(ed: &mut Editor, sel: &[String], id: &str) {
    let Some(st) = ed.doc.styles.object.iter().find(|s| s.id == id).cloned() else {
        return;
    };
    super::ops::modify(ed, sel, "Aplicar estilo", |o| {
        o.fill = st.fill.clone();
        o.stroke = st.stroke.clone();
        o.style = Some(st.id.clone());
        true
    });
}

/// Redefine o estilo pelo objeto `from` e atualiza todos os ligados.
pub fn redefine_object_style(ed: &mut Editor, id: &str, from: &str) {
    let Some(o) = ed.find(from).and_then(|r| ed.get(&r)).cloned() else {
        return;
    };
    ed.begin("Redefinir estilo");
    ed.edit_styles(|s| {
        if let Some(st) = s.object.iter_mut().find(|s| s.id == id) {
            st.fill = o.fill.clone();
            st.stroke = o.stroke.clone();
        }
    });
    let linked = ids_where(&ed.doc, |x| x.style.as_deref() == Some(id));
    apply_object_style(ed, &linked, id);
    ed.commit();
}

/// Exclui o estilo; os objetos ficam com a aparência e sem a ligação.
pub fn delete_object_style(ed: &mut Editor, id: &str) {
    let linked = ids_where(&ed.doc, |x| x.style.as_deref() == Some(id));
    ed.begin("Excluir estilo");
    ed.edit_styles(|s| s.object.retain(|x| x.id != id));
    super::ops::modify(ed, &linked, "Desligar do estilo", |o| {
        o.style = None;
        true
    });
    ed.commit();
}

// ---- estilos de texto ----------------------------------------------------------

fn text_style_of(
    o: &Object,
) -> Option<(String, f64, u16, bool, String, Option<crate::model::Fill>)> {
    let ObjectKind::Text(t) = &o.kind else {
        return None;
    };
    let p = t.paragraphs.first()?;
    let s = p.spans.first()?;
    Some((
        s.font.family.clone(),
        s.font.size,
        s.font.weight,
        s.font.italic,
        p.align.clone(),
        s.fill.clone().or_else(|| o.fill.clone()),
    ))
}

/// Novo estilo de texto pelo primeiro trecho de `from` (que fica ligado).
pub fn create_text_style(ed: &mut Editor, name: &str, from: &str) -> Option<String> {
    let o = ed.get(&ed.find(from)?)?.clone();
    let (family, size, weight, italic, align, fill) = text_style_of(&o)?;
    let id = new_id();
    let st = TextStyleDef {
        id: id.clone(),
        name: name.into(),
        family,
        size,
        weight,
        italic,
        align: Some(align),
        fill,
    };
    ed.begin("Novo estilo de texto");
    ed.edit_styles(|s| s.text.push(st));
    super::ops::modify(ed, &[from.to_string()], "Ligar ao estilo", |o| {
        o.text_style = Some(id.clone());
        true
    });
    ed.commit();
    Some(id)
}

/// Aplica o estilo de texto aos textos da seleção.
pub fn apply_text_style(ed: &mut Editor, sel: &[String], id: &str) {
    let Some(st) = ed.doc.styles.text.iter().find(|s| s.id == id).cloned() else {
        return;
    };
    super::ops::modify(ed, sel, "Aplicar estilo de texto", |o| {
        let ObjectKind::Text(t) = &mut o.kind else {
            return false;
        };
        for p in &mut t.paragraphs {
            if let Some(a) = &st.align {
                p.align = a.clone();
            }
            for s in &mut p.spans {
                s.font.family = st.family.clone();
                s.font.size = st.size;
                s.font.weight = st.weight;
                s.font.italic = st.italic;
                if st.fill.is_some() {
                    s.fill = None;
                }
            }
        }
        if st.fill.is_some() {
            o.fill = st.fill.clone();
        }
        o.text_style = Some(st.id.clone());
        true
    });
}

/// Redefine o estilo de texto pelo texto `from` e atualiza os ligados.
pub fn redefine_text_style(ed: &mut Editor, id: &str, from: &str) {
    let Some((family, size, weight, italic, align, fill)) = ed
        .find(from)
        .and_then(|r| ed.get(&r))
        .and_then(text_style_of)
    else {
        return;
    };
    ed.begin("Redefinir estilo de texto");
    ed.edit_styles(|s| {
        if let Some(st) = s.text.iter_mut().find(|s| s.id == id) {
            (st.family, st.size, st.weight, st.italic, st.align, st.fill) =
                (family, size, weight, italic, Some(align), fill);
        }
    });
    let linked = ids_where(&ed.doc, |x| x.text_style.as_deref() == Some(id));
    apply_text_style(ed, &linked, id);
    ed.commit();
}

pub fn delete_text_style(ed: &mut Editor, id: &str) {
    let linked = ids_where(&ed.doc, |x| x.text_style.as_deref() == Some(id));
    ed.begin("Excluir estilo de texto");
    ed.edit_styles(|s| s.text.retain(|x| x.id != id));
    super::ops::modify(ed, &linked, "Desligar do estilo", |o| {
        o.text_style = None;
        true
    });
    ed.commit();
}

pub fn rename_style(ed: &mut Editor, id: &str, name: &str) {
    ed.transaction("Renomear", |ed| {
        ed.edit_styles(|s| {
            for st in &mut s.object {
                if st.id == id {
                    st.name = name.into();
                }
            }
            for st in &mut s.text {
                if st.id == id {
                    st.name = name.into();
                }
            }
            for st in &mut s.symbols {
                if st.id == id {
                    st.name = name.into();
                }
            }
        })
    });
}

// ---- símbolos ------------------------------------------------------------------

/// Cópias do mestre com IDs novos.
fn copies(def: &SymbolDef) -> Vec<Object> {
    def.objects
        .iter()
        .map(|o| {
            let mut c = o.clone();
            super::ops::renew_ids(&mut c);
            c
        })
        .collect()
}

/// Cria o símbolo com a seleção: o mestre fica com os objetos levados à
/// origem pelo canto de baixo à esquerda; a seleção vira a primeira
/// instância. Devolve (símbolo, instância).
pub fn create_symbol(ed: &mut Editor, sel: &[String], name: &str) -> Option<(String, String)> {
    let b = super::ops::selection_bounds(ed, sel, false)?;
    let objs: Vec<Object> = sel
        .iter()
        .filter_map(|id| {
            let r = ed.find(id)?;
            let mut o = ed.get(&r)?.clone();
            let m = ed.parent_affine(&r) * o.transform.affine();
            o.transform = Transform::from_affine(Affine::translate((-b.x0, -b.y0)) * m);
            Some(o)
        })
        .collect();
    if objs.is_empty() {
        return None;
    }
    let sid = new_id();
    let def = SymbolDef {
        id: sid.clone(),
        name: name.into(),
        objects: objs,
    };
    let page = ed.find(&sel[0])?.loc.page;
    let layer = ed.find(&sel[0])?.loc.layer;
    ed.begin("Criar símbolo");
    ed.edit_styles(|s| s.symbols.push(def));
    super::ops::delete(ed, sel);
    let inst = instance_at(ed, page, layer, &sid, Point::new(b.x0, b.y0));
    ed.commit();
    Some((sid, inst?))
}

fn instance_at(ed: &mut Editor, page: usize, layer: usize, sid: &str, at: Point) -> Option<String> {
    let def = ed.doc.styles.symbols.iter().find(|s| s.id == sid)?.clone();
    let mut g = Object::new(ObjectKind::Group(GroupData {
        children: copies(&def),
        boolean: None,
        symmetry: None,
        effect: None,
        symbol: Some(sid.to_string()),
        extra: Default::default(),
    }));
    g.transform = Transform::translate(at.x, at.y);
    g.name = Some(def.name.clone());
    Some(super::ops::add_object(
        ed,
        page,
        layer,
        g,
        "Inserir símbolo",
    ))
}

/// Nova instância do símbolo com o canto em `at` (página).
pub fn insert_symbol(
    ed: &mut Editor,
    page: usize,
    layer: usize,
    sid: &str,
    at: Point,
) -> Option<String> {
    instance_at(ed, page, layer, sid, at)
}

/// Redefine o símbolo pelos filhos da instância `from` (editada à mão) e
/// troca as cópias de todas as instâncias.
pub fn redefine_symbol(ed: &mut Editor, sid: &str, from: &str) {
    let Some(ObjectKind::Group(g)) = ed
        .find(from)
        .and_then(|r| ed.get(&r))
        .map(|o| o.kind.clone())
    else {
        return;
    };
    ed.begin("Redefinir símbolo");
    ed.edit_styles(|s| {
        if let Some(d) = s.symbols.iter_mut().find(|d| d.id == sid) {
            d.objects = g.children.clone();
        }
    });
    let Some(def) = ed.doc.styles.symbols.iter().find(|d| d.id == sid).cloned() else {
        ed.commit();
        return;
    };
    let linked = ids_where(
        &ed.doc,
        |x| matches!(&x.kind, ObjectKind::Group(g) if g.symbol.as_deref() == Some(sid)),
    );
    super::ops::modify(ed, &linked, "Atualizar instâncias", |o| {
        if let ObjectKind::Group(g) = &mut o.kind {
            g.children = copies(&def);
            return true;
        }
        false
    });
    ed.commit();
}

/// Exclui o símbolo; as instâncias viram grupos comuns.
pub fn delete_symbol(ed: &mut Editor, sid: &str) {
    let linked = ids_where(
        &ed.doc,
        |x| matches!(&x.kind, ObjectKind::Group(g) if g.symbol.as_deref() == Some(sid)),
    );
    ed.begin("Excluir símbolo");
    ed.edit_styles(|s| s.symbols.retain(|d| d.id != sid));
    super::ops::modify(ed, &linked, "Desligar do símbolo", |o| {
        if let ObjectKind::Group(g) = &mut o.kind {
            g.symbol = None;
            return true;
        }
        false
    });
    ed.commit();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Color, Fill, Page, RectData, Stroke};

    fn sq(x: f64) -> Object {
        let mut o = Object::new(ObjectKind::Rectangle(RectData {
            width: 10_000.0,
            height: 10_000.0,
            corners: None,
            extra: Default::default(),
        }));
        o.transform = Transform::translate(x, 0.0);
        o.fill = Some(Fill::solid(Color::black()));
        o
    }

    fn editor() -> Editor {
        let mut d = Document::new();
        d.data.pages.push(Page::new(200_000.0, 200_000.0));
        Editor::new(d)
    }

    fn get(ed: &Editor, id: &str) -> Object {
        ed.get(&ed.find(id).unwrap()).unwrap().clone()
    }

    #[test]
    fn object_style_links_and_updates() {
        let mut ed = editor();
        let a = super::super::ops::add_object(&mut ed, 0, 0, sq(0.0), "a");
        let b = super::super::ops::add_object(&mut ed, 0, 0, sq(20_000.0), "b");
        let sid = create_object_style(&mut ed, "Marca", &a).unwrap();
        apply_object_style(&mut ed, std::slice::from_ref(&b), &sid);
        assert_eq!(get(&ed, &b).style.as_deref(), Some(sid.as_str()));
        // Muda o a e redefine: o b acompanha, num passo.
        super::super::ops::modify(&mut ed, std::slice::from_ref(&a), "cor", |o| {
            o.fill = Some(Fill::solid(Color::rgb(1.0, 0.0, 0.0)));
            o.stroke = Some(Stroke::new(500.0, Color::black()));
            true
        });
        let n = ed.undo_len();
        redefine_object_style(&mut ed, &sid, &a);
        assert_eq!(ed.undo_len(), n + 1);
        assert_eq!(
            get(&ed, &b).fill,
            Some(Fill::solid(Color::rgb(1.0, 0.0, 0.0)))
        );
        assert!(get(&ed, &b).stroke.is_some());
        delete_object_style(&mut ed, &sid);
        assert!(get(&ed, &b).style.is_none() && ed.doc.styles.object.is_empty());
        ed.undo();
        assert_eq!(ed.doc.styles.object.len(), 1, "desfaz a exclusão");
        let o = get(&ed, &b);
        assert_eq!(Object::from_value(o.to_value()), o);
    }

    #[test]
    fn text_style_applies_font() {
        let mut ed = editor();
        let t1 = super::super::text::new_text(0.0, 50_000.0, None, "Arial", 12_000.0);
        let mut t2 = super::super::text::new_text(0.0, 80_000.0, None, "Times New Roman", 30_000.0);
        if let ObjectKind::Text(t) = &mut t2.kind {
            super::super::text::insert(t, 0, "Outro");
        }
        let a = super::super::ops::add_object(&mut ed, 0, 0, t1, "t1");
        let b = super::super::ops::add_object(&mut ed, 0, 0, t2, "t2");
        let sid = create_text_style(&mut ed, "Título", &a).unwrap();
        apply_text_style(&mut ed, std::slice::from_ref(&b), &sid);
        let ObjectKind::Text(t) = get(&ed, &b).kind else {
            panic!()
        };
        assert!(
            t.paragraphs
                .iter()
                .flat_map(|p| &p.spans)
                .all(|s| s.font.family == "Arial" && s.font.size == 12_000.0)
        );
    }

    #[test]
    fn symbols_instances_follow_master() {
        let mut ed = editor();
        let a = super::super::ops::add_object(&mut ed, 0, 0, sq(50_000.0), "a");
        let (sid, inst) = create_symbol(&mut ed, std::slice::from_ref(&a), "Logo").unwrap();
        assert!(ed.find(&a).is_none(), "a seleção virou instância");
        let second = insert_symbol(&mut ed, 0, 0, &sid, Point::new(0.0, 100_000.0)).unwrap();
        // Edita a primeira instância e redefine: a segunda muda.
        super::super::ops::modify(&mut ed, std::slice::from_ref(&inst), "cor", |o| {
            if let ObjectKind::Group(g) = &mut o.kind {
                g.children[0].fill = Some(Fill::solid(Color::rgb(0.0, 1.0, 0.0)));
            }
            true
        });
        redefine_symbol(&mut ed, &sid, &inst);
        let ObjectKind::Group(g) = get(&ed, &second).kind else {
            panic!()
        };
        assert_eq!(
            g.children[0].fill,
            Some(Fill::solid(Color::rgb(0.0, 1.0, 0.0)))
        );
        delete_symbol(&mut ed, &sid);
        let ObjectKind::Group(g) = get(&ed, &second).kind else {
            panic!()
        };
        assert!(g.symbol.is_none());
        // Ida e volta no .gdr com o recurso de estilos.
        ed.undo();
        let bytes = crate::gdr::write_bytes(&ed.doc, &Default::default()).unwrap();
        let (back, _) = crate::gdr::read_bytes(&bytes).unwrap();
        assert_eq!(back.styles, ed.doc.styles);
    }
}
