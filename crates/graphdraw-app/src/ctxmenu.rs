//! Menus do botão direito (doc 26): itens como dados — o que aparece depende
//! do alvo, do tipo e da quantidade selecionada (GD-BDR-43/45) — e o desenho
//! do menu no ponto do cursor (GD-BDR-04). Cada comando vira uma transação;
//! abrir, fechar e cancelar não entram no histórico (GD-BDR-05).

use eframe::egui;
use graphdraw_core::edit::nodes::NodeRef;
use graphdraw_core::geometry::SegRef;
use graphdraw_core::model::{NodeKind, ObjectKind};

use crate::doc::DocState;
use crate::i18n::tr;
use crate::keymap::Command;

/// Onde o menu foi pedido.
#[derive(Debug, Clone, PartialEq)]
pub enum Target {
    /// Objeto (já selecionado).
    Object(String),
    /// Área vazia.
    Empty,
    /// Nó (ferramenta Forma).
    Node(NodeRef),
    /// Segmento no parâmetro `t`.
    Segment(SegRef, f64),
    /// Arrasto com o botão direito solto: sobre outro objeto (`over`) ou na
    /// área vazia; `delta` é o deslocamento (página).
    Drop {
        over: Option<String>,
        delta: kurbo::Vec2,
    },
}

/// Ação de um item.
#[derive(Debug, Clone, PartialEq)]
pub enum Cmd {
    /// Comando comum (também no menu principal e em atalho, GD-BDR-06).
    Run(Command),
    NodeKind(NodeKind),
    BreakNode,
    JoinEnds,
    DeleteNode,
    SegToCurve,
    SegToLine,
    AddNodeHere,
    Reverse,
    SampleText,
    UseAsTextFrame,
    ExitFocus,
    MoveHere,
    CopyHere,
    CopyAll,
    CopyStroke,
    CopyFill,
    Weld,
    Trim,
    Intersect,
    CenterOnTarget,
    Cancel,
    /// Itens Beta (desabilitados; nada a executar).
    ClipInside,
    ImageFrame,
    LinkedInstance,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Entry {
    Item {
        label: &'static str,
        cmd: Cmd,
        /// `Some(motivo)` = desabilitado, com o motivo na dica (GD-BDR-04).
        disabled: Option<&'static str>,
        checked: bool,
    },
    Sep,
    Sub(&'static str, Vec<Entry>),
}

fn item(label: &'static str, cmd: Cmd) -> Entry {
    Entry::Item {
        label,
        cmd,
        disabled: None,
        checked: false,
    }
}

fn beta(label: &'static str, cmd: Cmd) -> Entry {
    Entry::Item {
        label,
        cmd,
        disabled: Some(tr(
            "Ainda não disponível nesta versão",
            "Not available in this version yet",
        )),
        checked: false,
    }
}

fn order_menu() -> Entry {
    Entry::Sub(
        tr("Ordem", "Order"),
        vec![
            item(
                tr("Trazer para frente", "Bring to front"),
                Cmd::Run(Command::ToFront),
            ),
            item(
                tr("Enviar para trás", "Send to back"),
                Cmd::Run(Command::ToBack),
            ),
            item(
                tr("Um passo à frente", "Forward one"),
                Cmd::Run(Command::Forward),
            ),
            item(
                tr("Um passo para trás", "Backward one"),
                Cmd::Run(Command::Backward),
            ),
            item(
                tr("À frente de…", "In front of…"),
                Cmd::Run(Command::InFrontOf),
            ),
            item(tr("Atrás de…", "Behind…"), Cmd::Run(Command::BehindOf)),
        ],
    )
}

/// Menu sobre objeto (GD-BDR-40..45, 60..64, 80..82). A seleção já inclui o
/// objeto clicado.
pub fn object_entries(doc: &DocState, clicked: &str, focus: bool) -> Vec<Entry> {
    let objs = doc.selected_objects();
    let Some(o) = doc.editor.find(clicked).and_then(|r| doc.editor.get(&r)) else {
        return Vec::new();
    };
    // GD-BDR-45: bloqueado mostra só desbloquear e ordem.
    if o.locked {
        return vec![
            item(
                tr("Desbloquear", "Unlock"),
                Cmd::Run(Command::UnlockSelection),
            ),
            Entry::Sep,
            order_menu(),
        ];
    }
    let n = objs.len();
    let any_group = objs.iter().any(|o| matches!(o.kind, ObjectKind::Group(_)));
    let any_shape = objs.iter().any(|o| {
        matches!(
            o.kind,
            ObjectKind::Rectangle(_)
                | ObjectKind::Ellipse(_)
                | ObjectKind::Polygon(_)
                | ObjectKind::Text(_)
        )
    });
    let multi_sub = objs
        .iter()
        .any(|o| matches!(&o.kind, ObjectKind::Path(p) if p.subpaths.len() > 1));
    let any_stroke = objs.iter().any(|o| o.stroke.is_some());
    let texts: Vec<_> = objs
        .iter()
        .filter(|o| matches!(o.kind, ObjectKind::Text(_)))
        .collect();
    let paragraph = texts
        .iter()
        .any(|o| matches!(&o.kind, ObjectKind::Text(t) if t.text_kind == "paragraph"));
    let closed_shape = n == 1
        && match &o.kind {
            ObjectKind::Rectangle(_) | ObjectKind::Ellipse(_) | ObjectKind::Polygon(_) => true,
            ObjectKind::Path(p) => p.subpaths.iter().all(|s| s.closed),
            _ => false,
        };
    let mut out = Vec::new();
    // Geometria (GD-BDR-40/43).
    if any_shape {
        out.push(item(
            tr("Converter em curvas", "Convert to curves"),
            Cmd::Run(Command::ToCurves),
        ));
    }
    if multi_sub {
        out.push(item(
            tr("Separar", "Break apart"),
            Cmd::Run(Command::BreakApart),
        ));
    }
    if any_stroke {
        out.push(item(
            tr("Converter contorno em objeto", "Convert outline to object"),
            Cmd::Run(Command::OutlineToObject),
        ));
    }
    out.push(Entry::Sep);
    out.push(order_menu());
    // Organização (GD-BDR-42).
    out.push(Entry::Sep);
    if n >= 2 {
        out.push(item(tr("Agrupar", "Group"), Cmd::Run(Command::Group_)));
    }
    if any_group {
        out.push(item(
            tr("Desagrupar", "Ungroup"),
            Cmd::Run(Command::Ungroup),
        ));
        out.push(item(
            tr("Desagrupar tudo", "Ungroup all"),
            Cmd::Run(Command::UngroupAll),
        ));
    }
    out.push(item(
        tr("Bloquear", "Lock"),
        Cmd::Run(Command::LockSelection),
    ));
    out.push(item(
        tr("Ocultar", "Hide"),
        Cmd::Run(Command::HideSelection),
    ));
    // Texto e quadros (GD-BDR-60..64).
    let mut text = Vec::new();
    if n == 1 && paragraph {
        text.push(item(
            tr("Inserir texto de exemplo", "Insert sample text"),
            Cmd::SampleText,
        ));
        text.push(item(
            tr("Converter em texto artístico", "Convert to artistic text"),
            Cmd::Run(Command::ConvertTextKind),
        ));
    }
    if closed_shape {
        text.push(item(
            tr("Usar como quadro de texto", "Use as text frame"),
            Cmd::UseAsTextFrame,
        ));
        text.push(beta(
            tr("Usar como quadro de imagem", "Use as image frame"),
            Cmd::ImageFrame,
        ));
    }
    if n == 2 && texts.len() == 1 {
        text.push(item(
            tr("Colocar texto no caminho", "Put text on path"),
            Cmd::Run(Command::FitTextToPath),
        ));
    }
    if !text.is_empty() {
        out.push(Entry::Sep);
        out.extend(text);
    }
    // Produção (GD-BDR-80..82).
    out.push(Entry::Sep);
    out.push(item(
        tr(
            "Gerar contorno de sangria/corte…",
            "Create bleed/cut outline…",
        ),
        Cmd::Run(Command::CutLine),
    ));
    out.push(item(
        tr("Aninhar na chapa…", "Nest on sheet…"),
        Cmd::Run(Command::Nest),
    ));
    out.push(item(
        tr("Exportar seleção…", "Export selection…"),
        Cmd::Run(Command::ExportSelection),
    ));
    out.push(item(
        tr("Estender até a sangria", "Extend to bleed"),
        Cmd::Run(Command::ExtendToBleed),
    ));
    out.push(item(
        tr("Converter em guia", "Convert to guide"),
        Cmd::Run(Command::ToGuides),
    ));
    out.push(item(
        tr("Simetria", "Symmetry"),
        Cmd::Run(Command::Symmetry),
    ));
    out.push(if focus {
        item(tr("Sair do modo foco", "Leave focus mode"), Cmd::ExitFocus)
    } else {
        item(tr("Modo foco", "Focus mode"), Cmd::Run(Command::FocusMode))
    });
    tidy(out)
}

/// Menu na área vazia (GD-BDR-44).
pub fn empty_entries(can_paste: bool, focus: bool) -> Vec<Entry> {
    let mut out = vec![
        Entry::Item {
            label: tr("Colar", "Paste"),
            cmd: Cmd::Run(Command::Paste),
            disabled: (!can_paste).then_some(tr(
                "A área de transferência está vazia",
                "The clipboard is empty",
            )),
            checked: false,
        },
        item(
            tr("Selecionar tudo", "Select all"),
            Cmd::Run(Command::SelectAll),
        ),
        item(tr("Mostrar todos", "Show all"), Cmd::Run(Command::ShowAll)),
        item(
            tr("Desbloquear todos", "Unlock all"),
            Cmd::Run(Command::UnlockAll),
        ),
    ];
    if focus {
        out.push(item(
            tr("Sair do modo foco", "Leave focus mode"),
            Cmd::ExitFocus,
        ));
    }
    out
}

/// Menu de nó (GD-BDR-50): tipo atual marcado.
pub fn node_entries(kind: NodeKind, ends_selected: bool) -> Vec<Entry> {
    let k = |label, nk: NodeKind| Entry::Item {
        label,
        cmd: Cmd::NodeKind(nk),
        disabled: None,
        checked: kind == nk,
    };
    vec![
        k(tr("Canto", "Corner"), NodeKind::Corner),
        k(tr("Suave", "Smooth"), NodeKind::Smooth),
        k(tr("Simétrico", "Symmetric"), NodeKind::Symmetric),
        Entry::Sep,
        item(tr("Quebrar no nó", "Break at node"), Cmd::BreakNode),
        Entry::Item {
            label: tr("Unir extremos", "Join ends"),
            cmd: Cmd::JoinEnds,
            disabled: (!ends_selected)
                .then_some(tr("Selecione os dois extremos", "Select both ends")),
            checked: false,
        },
        item(tr("Excluir nó", "Delete node"), Cmd::DeleteNode),
    ]
}

/// Menu de segmento (GD-BDR-51).
pub fn segment_entries() -> Vec<Entry> {
    vec![
        item(
            tr("Converter em curva", "Convert to curve"),
            Cmd::SegToCurve,
        ),
        item(tr("Converter em reta", "Convert to line"), Cmd::SegToLine),
        item(tr("Adicionar nó aqui", "Add node here"), Cmd::AddNodeHere),
        item(tr("Inverter direção", "Reverse direction"), Cmd::Reverse),
    ]
}

/// Menu ao soltar o arrasto direito (GD-BDR-22..34).
pub fn drop_entries(over_object: bool) -> Vec<Entry> {
    let mut out = Vec::new();
    if over_object {
        out.push(beta(
            tr("Colocar dentro (recorte)", "Place inside (clip)"),
            Cmd::ClipInside,
        ));
        out.push(item(
            tr("Copiar todas as propriedades", "Copy all properties"),
            Cmd::CopyAll,
        ));
        out.push(item(
            tr("Copiar só contorno", "Copy outline only"),
            Cmd::CopyStroke,
        ));
        out.push(item(
            tr("Copiar só preenchimento", "Copy fill only"),
            Cmd::CopyFill,
        ));
        out.push(Entry::Sep);
        out.push(item(tr("Soldar", "Weld"), Cmd::Weld));
        out.push(item(tr("Aparar", "Trim"), Cmd::Trim));
        out.push(item(tr("Interseção", "Intersect"), Cmd::Intersect));
        out.push(item(
            tr("Centralizar no alvo", "Center on target"),
            Cmd::CenterOnTarget,
        ));
        out.push(Entry::Sep);
    }
    out.push(item(tr("Mover para cá", "Move here"), Cmd::MoveHere));
    out.push(item(tr("Copiar para cá", "Copy here"), Cmd::CopyHere));
    if !over_object {
        out.push(beta(
            tr("Criar instância vinculada", "Create linked instance"),
            Cmd::LinkedInstance,
        ));
    }
    out.push(Entry::Sep);
    out.push(item(tr("Cancelar", "Cancel"), Cmd::Cancel));
    out
}

/// Sem separadores no começo, no fim nem repetidos.
fn tidy(v: Vec<Entry>) -> Vec<Entry> {
    let mut out: Vec<Entry> = Vec::new();
    for e in v {
        if e == Entry::Sep && (out.is_empty() || out.last() == Some(&Entry::Sep)) {
            continue;
        }
        out.push(e);
    }
    while out.last() == Some(&Entry::Sep) {
        out.pop();
    }
    out
}

/// Ícone do item (assets/icons): o mesmo da função onde ela já tem um
/// (barra de propriedades, painéis); `""` = sem ícone.
pub fn icon_of(c: &Cmd) -> &'static str {
    use Command as K;
    match c {
        Cmd::Run(k) => match k {
            K::ToCurves => "to-curves",
            K::BreakApart => "break-apart",
            K::OutlineToObject => "outline-to-object",
            K::ToFront => "order-front",
            K::ToBack => "order-back",
            K::Forward => "order-forward",
            K::Backward => "order-backward",
            K::InFrontOf => "order-in-front-of",
            K::BehindOf => "order-behind-of",
            K::Group_ => "group",
            K::Ungroup => "ungroup",
            K::UngroupAll => "ungroup-all",
            K::LockSelection => "lock-on",
            K::UnlockSelection => "lock-off",
            K::HideSelection => "eye-off",
            K::ShowAll => "show-all",
            K::UnlockAll => "unlock-all",
            K::ConvertTextKind => "text-convert",
            K::FitTextToPath => "text-path",
            K::CutLine => "cut-line",
            K::Nest => "cut-line",
            K::ExportSelection => "export-selection",
            K::ExtendToBleed => "bleed",
            K::ToGuides => "guide-object",
            K::Symmetry => "symmetry",
            K::FocusMode => "focus",
            K::Paste => "paste",
            K::SelectAll => "select-all",
            _ => "",
        },
        Cmd::NodeKind(NodeKind::Corner) => "node-corner",
        Cmd::NodeKind(NodeKind::Smooth) => "node-smooth",
        Cmd::NodeKind(NodeKind::Symmetric) => "node-symmetric",
        Cmd::BreakNode => "node-break",
        Cmd::JoinEnds => "node-join",
        Cmd::DeleteNode => "node-delete",
        Cmd::SegToCurve => "seg-curve",
        Cmd::SegToLine => "seg-line",
        Cmd::AddNodeHere => "node-add",
        Cmd::Reverse => "path-reverse",
        Cmd::SampleText => "text-sample",
        Cmd::UseAsTextFrame => "text-frame",
        Cmd::ImageFrame => "image-frame",
        Cmd::ExitFocus => "focus",
        Cmd::MoveHere => "move-here",
        Cmd::CopyHere => "duplicate",
        Cmd::CopyAll => "copy-props",
        Cmd::CopyStroke => "copy-stroke",
        Cmd::CopyFill => "copy-fill",
        Cmd::Weld => "bool-weld",
        Cmd::Trim => "bool-trim",
        Cmd::Intersect => "bool-intersect",
        Cmd::CenterOnTarget => "center-on",
        Cmd::ClipInside => "clip-inside",
        Cmd::LinkedInstance => "instance",
        Cmd::Cancel => "cancel",
    }
}

/// Ícone de 16 px na cor do texto do tema.
fn menu_icon(ui: &egui::Ui, name: &str) -> egui::Image<'static> {
    egui::Image::new(crate::icons::get_micro(name))
        .tint(crate::theme::palette(ui).fg_primary)
        .fit_to_exact_size(egui::Vec2::splat(crate::theme::ICON_MICRO))
}

/// Desenha os itens; devolve o escolhido.
pub fn show(ui: &mut egui::Ui, entries: &[Entry]) -> Option<Cmd> {
    let mut picked = None;
    for e in entries {
        match e {
            Entry::Sep => {
                ui.separator();
            }
            Entry::Item {
                label,
                cmd,
                disabled,
                checked,
            } => {
                let text = if *checked {
                    format!("✓ {label}")
                } else {
                    label.to_string()
                };
                let button = match icon_of(cmd) {
                    "" => egui::Button::new(text),
                    name => egui::Button::image_and_text(menu_icon(ui, name), text),
                };
                let r = ui.add_enabled(disabled.is_none(), button);
                let r = match disabled {
                    Some(why) => r.on_disabled_hover_text(*why),
                    None => r,
                };
                if r.clicked() {
                    picked = Some(cmd.clone());
                }
            }
            Entry::Sub(label, items) => {
                ui.menu_image_text_button(menu_icon(ui, "order-front"), *label, |ui| {
                    if let Some(c) = show(ui, items) {
                        picked = Some(c);
                    }
                });
            }
        }
    }
    picked
}

#[cfg(test)]
mod tests {
    use super::*;
    use graphdraw_core::edit::ops;
    use graphdraw_core::model::{Object, RectData};

    fn labels(v: &[Entry]) -> Vec<&'static str> {
        v.iter()
            .filter_map(|e| match e {
                Entry::Item { label, .. } | Entry::Sub(label, _) => Some(*label),
                Entry::Sep => None,
            })
            .collect()
    }

    /// GD-BDR-43/45: itens pelo tipo e pela quantidade; bloqueado mostra só
    /// desbloquear e ordem.
    #[test]
    fn gd_bdr_43_45_entries_follow_selection() {
        let mut doc = DocState::new_blank();
        let rect = || {
            Object::new(ObjectKind::Rectangle(RectData {
                width: 10.0,
                height: 10.0,
                corners: None,
                extra: Default::default(),
            }))
        };
        let a = ops::add_object(&mut doc.editor, 0, 0, rect(), "a");
        let b = ops::add_object(&mut doc.editor, 0, 0, rect(), "b");
        doc.selection = vec![a.clone()];
        let one = labels(&object_entries(&doc, &a, false));
        assert!(one.contains(&tr("Converter em curvas", "Convert to curves")));
        assert!(
            !one.contains(&tr("Agrupar", "Group")),
            "Agrupar só com 2 ou mais"
        );
        assert!(!one.contains(&tr("Separar", "Break apart")));
        assert!(one.contains(&tr("Usar como quadro de texto", "Use as text frame")));
        doc.selection = vec![a.clone(), b.clone()];
        assert!(labels(&object_entries(&doc, &a, false)).contains(&tr("Agrupar", "Group")));
        ops::set_locked(&mut doc.editor, std::slice::from_ref(&a), true);
        doc.selection = vec![a.clone()];
        let locked = labels(&object_entries(&doc, &a, false));
        assert_eq!(
            locked,
            vec![tr("Desbloquear", "Unlock"), tr("Ordem", "Order")]
        );
    }

    /// Todo item dos menus tem ícone, e o desenho existe em assets/icons.
    #[test]
    fn every_menu_item_has_an_icon() {
        let mut doc = DocState::new_blank();
        let mut o = Object::new(ObjectKind::Rectangle(RectData {
            width: 10.0,
            height: 10.0,
            corners: None,
            extra: Default::default(),
        }));
        o.stroke = Some(graphdraw_core::model::Stroke::new(
            1.0,
            graphdraw_core::model::Color::black(),
        ));
        let a = ops::add_object(&mut doc.editor, 0, 0, o.clone(), "a");
        let b = ops::add_object(&mut doc.editor, 0, 0, o, "b");
        doc.selection = vec![a.clone(), b];
        fn walk(v: &[Entry], out: &mut Vec<Cmd>) {
            for e in v {
                match e {
                    Entry::Item { cmd, .. } => out.push(cmd.clone()),
                    Entry::Sub(_, s) => walk(s, out),
                    Entry::Sep => {}
                }
            }
        }
        let mut cmds = Vec::new();
        for v in [
            object_entries(&doc, &a, false),
            object_entries(&doc, &a, true),
            empty_entries(true, true),
            node_entries(NodeKind::Corner, true),
            segment_entries(),
            drop_entries(true),
            drop_entries(false),
        ] {
            walk(&v, &mut cmds);
        }
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/icons");
        for c in cmds {
            let name = icon_of(&c);
            assert!(!name.is_empty(), "sem ícone: {c:?}");
            assert!(dir.join(format!("{name}.svg")).exists(), "falta {name}.svg");
        }
    }

    #[test]
    fn drop_menu_has_beta_items_disabled() {
        let v = drop_entries(true);
        let disabled: Vec<_> = v
            .iter()
            .filter_map(|e| match e {
                Entry::Item {
                    label,
                    disabled: Some(_),
                    ..
                } => Some(*label),
                _ => None,
            })
            .collect();
        assert_eq!(
            disabled,
            vec![tr("Colocar dentro (recorte)", "Place inside (clip)")]
        );
        assert!(labels(&drop_entries(false)).contains(&tr("Mover para cá", "Move here")));
        assert!(!labels(&drop_entries(false)).contains(&tr("Soldar", "Weld")));
    }
}
