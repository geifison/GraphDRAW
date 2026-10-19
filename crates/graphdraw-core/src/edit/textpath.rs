//! Texto em caminho (GD-TXT-09): vínculo vivo com a curva-guia, endireitar
//! guardando o caminho ("caminho fantasma"), reativar o caminho de origem,
//! o modo selo (superior e inferior no mesmo caminho fechado) e a guia como
//! linha de construção.

use super::history::Editor;
use super::{ObjLoc, ObjRef};
use crate::model::{Color, Object, ObjectKind, PathData, Stroke};

/// Caminho da guia `guide` no espaço local do texto em `r`.
fn guide_subpaths(
    ed: &Editor,
    r: &ObjRef,
    text: &Object,
    guide: &str,
) -> Option<Vec<crate::model::Subpath>> {
    let gr = ed.find(guide)?;
    let g = ed.get(&gr)?;
    if matches!(g.kind, ObjectKind::Text(_) | ObjectKind::Group(_)) {
        return None;
    }
    let mut outline = super::query::local_outline(g)?;
    let to_page = ed.parent_affine(&gr) * g.transform.affine();
    let text_m = ed.parent_affine(r) * text.transform.affine();
    outline.apply_affine(text_m.inverse() * to_page);
    Some(super::convert::bez_to_subpaths(&outline))
}

/// Refaz, dentro da transação aberta, os textos cuja curva-guia mudou (ou
/// que se moveram sem ela): o texto segue a guia.
pub fn refresh(ed: &mut Editor) {
    let mut out: Vec<(ObjLoc, Object)> = Vec::new();
    for (pi, page) in ed.doc.data.pages.iter().enumerate() {
        for (li, layer) in page.layers.iter().enumerate() {
            for (oi, o) in layer.objects.iter().enumerate() {
                let ObjectKind::Text(t) = &o.kind else {
                    continue;
                };
                let Some(tp) = &t.path else { continue };
                let Some(guide) = tp.guide.as_deref().filter(|_| !tp.detached) else {
                    continue;
                };
                let loc = ObjLoc {
                    page: pi,
                    layer: li,
                    index: oi,
                };
                let Some(sp) = guide_subpaths(ed, &ObjRef::top(loc), o, guide) else {
                    continue;
                };
                if sp != tp.subpaths {
                    let mut n = o.clone();
                    if let ObjectKind::Text(nt) = &mut n.kind
                        && let Some(np) = &mut nt.path
                    {
                        np.subpaths = sp;
                    }
                    out.push((loc, n));
                }
            }
        }
    }
    for (loc, n) in out {
        ed.replace_object_always(loc, n);
    }
}

/// Texto em caminho justificado vira alinhado no início com o espaçamento
/// que a justificação dava (a mesma aparência): assim o espaçamento entre
/// letras pode diminuir — justificado, ele sempre ocupa o caminho todo.
/// Devolve se mudou.
pub fn unjustify(t: &mut crate::model::TextData) -> bool {
    let Some(tp) = &t.path else { return false };
    if tp.align != "justify" {
        return false;
    }
    let now = crate::render::text::layout_text(t);
    let Some(span) = now.path_span else {
        return false;
    };
    let glyphs = now.lines.first().map_or(0, |l| {
        l.runs
            .iter()
            .flat_map(|r| &r.glyphs)
            .filter(|g| g.advance > 0.0)
            .count()
    });
    let size = t
        .paragraphs
        .first()
        .and_then(|p| p.spans.first())
        .map_or(10_000.0, |s| s.font.size);
    let mut n = t.clone();
    if let Some(p) = &mut n.path {
        p.align = "start".into();
        p.offset = span.start;
    }
    let Some(nat) = crate::render::text::layout_text(&n).path_span else {
        return false;
    };
    if glyphs > 0 {
        n.tracking += ((span.end - span.start) - (nat.end - nat.start)) / (glyphs as f64 * size);
    }
    *t = n;
    true
}

/// Espelhar na horizontal: inverte o sentido de leitura sem tirar o texto
/// do lugar nem da faixa em que está junto ao traço. É o que põe de pé o
/// texto da base de um círculo que saiu de cabeça para baixo (lendo da
/// direita para a esquerda). Devolve se mudou.
pub fn reverse_reading(t: &mut crate::model::TextData) -> bool {
    use crate::render::text::{layout_text, vertical_shift};
    let Some(tp) = t.path.clone() else {
        return false;
    };
    let Some(span) = layout_text(t).path_span else {
        return false;
    };
    let size = t
        .paragraphs
        .first()
        .and_then(|p| p.spans.first())
        .map_or(10_000.0, |s| s.font.size);
    let below = |side: &str| if side == "below" { -0.72 * size } else { 0.0 };
    // Faixa das letras: da linha de base (off) até a altura das maiúsculas,
    // no sentido "para cima" do glifo. Invertido o sentido, o "para cima"
    // troca de lado: a mesma faixa começa em −(off + maiúsculas).
    let off = tp.distance + vertical_shift(&tp.vertical, size) + below(&tp.side);
    let mut n = tp.clone();
    n.mirror = !tp.mirror;
    n.distance = -(off + 0.72 * size) - vertical_shift(&n.vertical, size) - below(&n.side);
    // O meio do texto fica onde estava (no caminho invertido, total − s).
    let w = span.end - span.start;
    let mid = span.total - (span.start + span.end) / 2.0;
    n.offset = match n.align.as_str() {
        "center" => mid - span.total / 2.0,
        "end" => mid + w / 2.0 - span.total,
        _ => mid - w / 2.0,
    };
    t.path = Some(n);
    true
}

/// Endireita o texto e guarda o caminho para reativar depois.
pub fn straighten(ed: &mut Editor, sel: &[String]) {
    super::ops::modify(ed, sel, "Endireitar texto", |o| match &mut o.kind {
        ObjectKind::Text(t) if t.path.is_some() => {
            t.path_memory = t.path.take();
            true
        }
        _ => false,
    });
}

/// O texto pode reativar o caminho de origem (endireitado ou separado).
pub fn can_reattach(o: &Object) -> bool {
    match &o.kind {
        ObjectKind::Text(t) => {
            t.path_memory.is_some()
                || t.path
                    .as_ref()
                    .is_some_and(|p| p.detached || p.guide.is_none())
        }
        _ => false,
    }
}

/// Reativar caminho de origem: o texto volta ao caminho guardado e ao
/// vínculo com a guia; se a guia não existe mais, ela é recriada abaixo do
/// texto, com a forma guardada. Devolve o ID da guia.
pub fn reattach(ed: &mut Editor, id: &str) -> Option<String> {
    let r = ed.find(id)?;
    let o = ed.get(&r)?.clone();
    let ObjectKind::Text(t) = &o.kind else {
        return None;
    };
    let mut tp = t.path.clone().or_else(|| t.path_memory.clone())?;
    let live = tp.guide.clone().filter(|g| ed.find(g).is_some());
    ed.begin("Reativar caminho de origem");
    let guide = match live {
        Some(g) => g,
        None => {
            let mut line = Object::new(ObjectKind::Path(PathData {
                subpaths: tp.subpaths.clone(),
                fill_rule: Default::default(),
                extra: Default::default(),
            }));
            line.transform = o.transform;
            line.stroke = Some(Stroke::new(250.0, Color::black()));
            let gid = line.id.clone();
            let mut loc = r.loc;
            ed.insert_object(loc, line);
            loc.index += 1;
            let _ = loc;
            gid
        }
    };
    tp.guide = Some(guide.clone());
    tp.detached = false;
    super::ops::modify(ed, &[id.to_string()], "Reativar caminho de origem", |o| {
        if let ObjectKind::Text(t) = &mut o.kind {
            t.path = Some(tp.clone());
            t.path_memory = None;
            return true;
        }
        false
    });
    ed.commit();
    Some(guide)
}

/// Modo selo (caminho fechado): o texto vira o de cima, lido da esquerda
/// para a direita por fora, e ganha um irmão embaixo, lido da esquerda para
/// a direita por dentro, com a altura das maiúsculas no traço (os dois na
/// mesma faixa). Devolve o ID do texto de baixo.
pub fn split_badge(ed: &mut Editor, id: &str, bottom_text: &str) -> Option<String> {
    let r = ed.find(id)?;
    let o = ed.get(&r)?.clone();
    let ObjectKind::Text(t) = &o.kind else {
        return None;
    };
    let tp = t.path.clone()?;
    let size = t
        .paragraphs
        .first()
        .and_then(|p| p.spans.first())
        .map_or(10_000.0, |s| s.font.size);
    let mut top = tp.clone();
    top.mirror = true;
    top.align = "center".into();
    top.vertical = "baseline".into();
    top.side = "above".into();
    let track = crate::render::text::path_track(&top, size)?;
    if !track.closed {
        return None;
    }
    top.offset = track.extreme(true) - track.total() / 2.0;
    let mut bottom = tp;
    bottom.mirror = false;
    bottom.align = "center".into();
    bottom.vertical = "cap".into();
    bottom.side = "above".into();
    let track = crate::render::text::path_track(&bottom, size)?;
    bottom.offset = track.extreme(false) - track.total() / 2.0;
    let mut twin = o.clone();
    twin.id = crate::model::new_id();
    twin.name = None;
    if let ObjectKind::Text(tt) = &mut twin.kind {
        tt.path = Some(bottom);
        for (i, p) in tt.paragraphs.iter_mut().enumerate() {
            for (j, s) in p.spans.iter_mut().enumerate() {
                s.text = if i == 0 && j == 0 {
                    bottom_text.to_string()
                } else {
                    String::new()
                };
            }
        }
        tt.paragraphs.truncate(1);
    }
    let twin_id = twin.id.clone();
    ed.begin("Dividir em superior e inferior");
    super::ops::modify(ed, &[id.to_string()], "Texto de cima", |o| {
        if let ObjectKind::Text(t) = &mut o.kind {
            t.path = Some(top.clone());
            return true;
        }
        false
    });
    let mut loc = r.loc;
    loc.index += 1;
    if r.path.is_empty() {
        ed.insert_object(loc, twin);
    }
    ed.commit();
    ed.find(&twin_id).map(|_| twin_id)
}

/// Guia do texto como linha de construção (não sai na impressão, no PDF
/// nem no corte), ou de volta ao normal.
pub fn set_construction(ed: &mut Editor, text_id: &str, on: bool) -> bool {
    let Some(guide) = ed
        .find(text_id)
        .and_then(|r| ed.get(&r))
        .and_then(|o| match &o.kind {
            ObjectKind::Text(t) => t.path.as_ref().and_then(|p| p.guide.clone()),
            _ => None,
        })
        .filter(|g| ed.find(g).is_some())
    else {
        return false;
    };
    super::ops::modify(ed, &[guide], "Linha de construção", |o| {
        let changed = o.construction != on;
        o.construction = on;
        changed
    });
    true
}

/// A guia do texto é linha de construção?
pub fn is_construction(ed: &Editor, text_id: &str) -> Option<bool> {
    let guide = ed
        .find(text_id)
        .and_then(|r| ed.get(&r))
        .and_then(|o| match &o.kind {
            ObjectKind::Text(t) => t.path.as_ref().and_then(|p| p.guide.clone()),
            _ => None,
        })?;
    ed.find(&guide)
        .and_then(|r| ed.get(&r))
        .map(|o| o.construction)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Document, EllipseData, Page, TextPath};
    use kurbo::Affine;

    fn setup() -> (Editor, String, String) {
        let mut d = Document::new();
        d.data.pages.push(Page::new(200_000.0, 200_000.0));
        let mut ed = Editor::new(d);
        let mut c = Object::new(ObjectKind::Ellipse(EllipseData {
            rx: 50_000.0,
            ry: 50_000.0,
            mode: "ellipse".into(),
            start_angle: 0.0,
            end_angle: 360.0,
            extra: Default::default(),
        }));
        c.transform = crate::model::Transform::translate(100_000.0, 100_000.0);
        let cid = super::super::ops::add_object(&mut ed, 0, 0, c, "c");
        let mut t = super::super::text::new_text(10_000.0, 10_000.0, None, "Arial", 8_000.0);
        if let ObjectKind::Text(td) = &mut t.kind {
            super::super::text::insert(td, 0, "SELO DE QUALIDADE");
        }
        let tid = super::super::ops::add_object(&mut ed, 0, 0, t, "t");
        assert!(super::super::ops::fit_text_to_path(&mut ed, &tid, &cid));
        (ed, tid, cid)
    }

    fn tp(ed: &Editor, id: &str) -> Option<TextPath> {
        match &ed.get(&ed.find(id)?)?.kind {
            ObjectKind::Text(t) => t.path.clone(),
            _ => None,
        }
    }

    /// Justificado não deixa apertar: unjustify troca por início com o mesmo
    /// espaçamento; depois o espaçamento menor encolhe o texto.
    #[test]
    fn unjustify_keeps_look_and_lets_tighten() {
        let (mut ed, tid, _cid) = setup();
        super::super::ops::edit_text_path(&mut ed, std::slice::from_ref(&tid), |p| {
            p.align = "justify".into()
        });
        let text = |ed: &Editor| match &ed.get(&ed.find(&tid).unwrap()).unwrap().kind {
            ObjectKind::Text(t) => t.clone(),
            _ => panic!(),
        };
        let mut t = text(&ed);
        let before = crate::render::text::layout_text(&t).path_span.unwrap();
        assert!(unjustify(&mut t));
        let after = crate::render::text::layout_text(&t).path_span.unwrap();
        assert!(
            (after.start - before.start).abs() < 50.0 && (after.end - before.end).abs() < 300.0,
            "{before:?} {after:?}"
        );
        t.tracking -= 0.05;
        let tight = crate::render::text::layout_text(&t).path_span.unwrap();
        assert!(tight.end - tight.start < after.end - after.start - 1_000.0);
    }

    /// Base do círculo de cabeça para baixo: espelhar na horizontal põe de pé
    /// no mesmo lugar (o meio do texto quase não sai do lugar).
    #[test]
    fn reverse_reading_keeps_place() {
        let (ed, tid, _cid) = setup();
        let mut t = match &ed.get(&ed.find(&tid).unwrap()).unwrap().kind {
            ObjectKind::Text(t) => t.clone(),
            _ => panic!(),
        };
        // Leva o texto para a base do círculo (de cabeça para baixo).
        if let Some(p) = &mut t.path {
            p.offset += std::f64::consts::PI * 50_000.0;
            // Sem o anticolisão: de pé, os topos apontam para o centro e ele
            // abriria o espaço (aqui só a posição importa).
            p.relax = false;
        }
        let bounds = |t: &crate::model::TextData| {
            let l = crate::render::text::layout_text(t);
            l.lines
                .iter()
                .flat_map(|l| &l.runs)
                .map(|r| kurbo::Shape::bounding_box(&r.outline))
                .reduce(|a, b| a.union(b))
                .unwrap()
        };
        let before = bounds(&t);
        let mirror = t.path.as_ref().unwrap().mirror;
        assert!(reverse_reading(&mut t));
        let after = bounds(&t);
        assert_ne!(t.path.as_ref().unwrap().mirror, mirror);
        assert!(
            (after.center() - before.center()).hypot() < 3_000.0,
            "{before:?} {after:?}"
        );
        // Duas vezes volta ao começo.
        assert!(reverse_reading(&mut t));
        assert!((bounds(&t).center() - before.center()).hypot() < 3_000.0);
    }

    #[test]
    fn live_link_follows_the_guide() {
        let (mut ed, tid, cid) = setup();
        let before = tp(&ed, &tid).unwrap().subpaths;
        super::super::ops::modify(&mut ed, std::slice::from_ref(&cid), "mover", |o| {
            o.transform = crate::model::Transform::translate(120_000.0, 100_000.0);
            true
        });
        assert_ne!(
            tp(&ed, &tid).unwrap().subpaths,
            before,
            "o texto seguiu a guia"
        );
    }

    #[test]
    fn straighten_and_reattach() {
        let (mut ed, tid, cid) = setup();
        straighten(&mut ed, std::slice::from_ref(&tid));
        assert!(tp(&ed, &tid).is_none());
        let o = ed.get(&ed.find(&tid).unwrap()).unwrap().clone();
        assert!(can_reattach(&o));
        // Guia apagada: reativar recria a guia com a forma guardada.
        super::super::ops::delete(&mut ed, std::slice::from_ref(&cid));
        let g = reattach(&mut ed, &tid).unwrap();
        assert!(ed.find(&g).is_some() && g != cid);
        let p = tp(&ed, &tid).unwrap();
        assert_eq!(p.guide.as_deref(), Some(g.as_str()));
    }

    #[test]
    fn badge_split_and_construction() {
        let (mut ed, tid, _cid) = setup();
        let bottom = split_badge(&mut ed, &tid, "DESDE 1990").unwrap();
        let (t, b) = (tp(&ed, &tid).unwrap(), tp(&ed, &bottom).unwrap());
        assert!(t.mirror && !b.mirror && b.vertical == "cap" && t.guide == b.guide);
        // O de cima fica em cima e o de baixo embaixo.
        let lay = |id: &str| {
            let o = ed.get(&ed.find(id).unwrap()).unwrap();
            super::super::query::bounds(o, Affine::IDENTITY, false).unwrap()
        };
        let (bt, bb) = (lay(&tid), lay(&bottom));
        assert!(bt.center().y > bb.center().y + 40_000.0, "{bt:?} {bb:?}");
        assert!(set_construction(&mut ed, &tid, true));
        assert_eq!(is_construction(&ed, &tid), Some(true));
    }
}
