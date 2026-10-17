//! Objetos dentro das camadas no painel Camadas (GD-UI-02, GD-CAM-03):
//! cada objeto numa linha com miniatura 1:1, nome editável (duplo clique),
//! olho e cadeado; arrastar e soltar muda a ordem de empilhamento, a camada
//! e entra/sai de grupos.

use std::collections::HashMap;

use eframe::egui::{self, Rect, Vec2};
use graphdraw_core::edit::ops::{self, DropAt};
use graphdraw_core::model::{Object, ObjectKind};
use kurbo::Affine;

use crate::doc::DocState;
use crate::i18n::tr;
use crate::icons;
use crate::theme;

/// O que o usuário fez numa linha de objeto.
#[derive(Debug, Clone, PartialEq)]
pub enum TreeAction {
    /// Selecionar; `true` = Shift (soma ou tira).
    Select(String, bool),
    Visible(String, bool),
    Locked(String, bool),
    /// Novo nome; `None` volta ao nome do tipo.
    Rename(String, Option<String>),
    /// Objeto arrastado e solto num destino.
    Drop(String, DropAt),
}

/// Onde uma linha aceita o objeto arrastado.
#[derive(Clone)]
pub enum Spot {
    Object(String),
    Layer { page: usize, layer: usize },
}

#[derive(Default)]
pub struct LayerTree {
    /// Miniaturas por ID: o objeto e a matriz dos pais usados no desenho,
    /// para refazer só quando mudam.
    thumbs: HashMap<String, (Object, Affine, egui::TextureHandle)>,
    /// Objeto em renomeação e o texto digitado.
    rename: Option<(String, String)>,
    /// Objeto sendo arrastado.
    drag: Option<String>,
    /// Linhas desenhadas neste quadro, para achar o destino do arrasto.
    spots: Vec<(Rect, Spot)>,
    /// Miniaturas refeitas neste quadro (limite para não travar a interface).
    rendered: usize,
}

/// Miniaturas novas por quadro; as demais ficam para os próximos.
const THUMBS_PER_FRAME: usize = 8;

/// Nome mostrado na linha: o nome dado ou o tipo.
pub fn display_name(o: &Object) -> String {
    o.name
        .clone()
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| crate::app::kind_label(&o.kind))
}

impl LayerTree {
    /// Começo do quadro do painel.
    pub fn begin(&mut self) {
        self.spots.clear();
        self.rendered = 0;
    }

    /// Registra a linha de uma camada como destino (solta no topo dela).
    pub fn layer_spot(&mut self, rect: Rect, page: usize, layer: usize) {
        self.spots.push((rect, Spot::Layer { page, layer }));
    }

    /// Objetos de uma camada, do topo (frente) para baixo.
    pub fn layer_objects(
        &mut self,
        ui: &mut egui::Ui,
        doc: &DocState,
        layer: usize,
        act: &mut Option<TreeAction>,
    ) {
        let objects = &doc.page().layers[layer].objects;
        if objects.is_empty() {
            ui.label(egui::RichText::new(tr("(vazia)", "(empty)")).weak());
        }
        for o in objects.iter().rev() {
            self.object_row(ui, doc, o, act);
        }
    }

    fn object_row(
        &mut self,
        ui: &mut egui::Ui,
        doc: &DocState,
        o: &Object,
        act: &mut Option<TreeAction>,
    ) {
        match &o.kind {
            ObjectKind::Group(g) => {
                let id = ui.make_persistent_id(("camadas-grupo", &o.id));
                let (_, head, _) =
                    egui::collapsing_header::CollapsingState::load_with_default_open(
                        ui.ctx(),
                        id,
                        false,
                    )
                    .show_header(ui, |ui| self.row_content(ui, doc, o, act))
                    .body(|ui| {
                        for c in g.children.iter().rev() {
                            self.object_row(ui, doc, c, act);
                        }
                    });
                self.spots
                    .push((head.response.rect, Spot::Object(o.id.clone())));
            }
            _ => {
                let r = ui.horizontal(|ui| {
                    // Mesmo recuo da seta dos grupos, para alinhar as linhas.
                    ui.add_space(ui.spacing().indent);
                    self.row_content(ui, doc, o, act);
                });
                self.spots
                    .push((r.response.rect, Spot::Object(o.id.clone())));
            }
        }
    }

    fn row_content(
        &mut self,
        ui: &mut egui::Ui,
        doc: &DocState,
        o: &Object,
        act: &mut Option<TreeAction>,
    ) {
        let (eye, eye_tip) = if o.visible {
            (
                "eye-on",
                tr("Visível (clique para ocultar)", "Visible (click to hide)"),
            )
        } else {
            (
                "eye-off",
                tr("Oculto (clique para mostrar)", "Hidden (click to show)"),
            )
        };
        if icons::small_button(ui, eye, eye_tip, true).clicked() {
            *act = Some(TreeAction::Visible(o.id.clone(), !o.visible));
        }
        let (lk, lk_tip) = if o.locked {
            (
                "lock-on",
                tr(
                    "Travado (clique para destravar)",
                    "Locked (click to unlock)",
                ),
            )
        } else {
            (
                "lock-off",
                tr(
                    "Destravado (clique para travar)",
                    "Unlocked (click to lock)",
                ),
            )
        };
        if icons::small_button(ui, lk, lk_tip, true).clicked() {
            *act = Some(TreeAction::Locked(o.id.clone(), !o.locked));
        }
        self.thumb(ui, doc, o);
        // Renomeando: campo de texto no lugar do nome.
        if let Some((rid, buf)) = &mut self.rename
            && *rid == o.id
        {
            let r = ui.add(egui::TextEdit::singleline(buf).desired_width(ui.available_width()));
            if !r.has_focus() && !r.lost_focus() {
                r.request_focus();
            }
            let esc = ui.input(|i| i.key_pressed(egui::Key::Escape));
            if esc {
                self.rename = None;
            } else if r.lost_focus() {
                let t = buf.trim().to_string();
                *act = Some(TreeAction::Rename(
                    o.id.clone(),
                    (!t.is_empty()).then_some(t),
                ));
                self.rename = None;
            }
            return;
        }
        let selected = doc.selection.contains(&o.id);
        let text = if o.visible {
            egui::RichText::new(display_name(o))
        } else {
            egui::RichText::new(display_name(o)).weak()
        };
        let r = ui
            .add(egui::Button::selectable(selected, text))
            .interact(egui::Sense::click_and_drag())
            .on_hover_text(tr(
                "Clique seleciona · Shift soma · duplo clique renomeia · arraste para mudar a ordem ou a camada",
                "Click selects · Shift adds · double-click renames · drag to change order or layer",
            ));
        if r.double_clicked() {
            self.rename = Some((o.id.clone(), o.name.clone().unwrap_or_default()));
        } else if r.clicked() && !o.locked {
            let add = ui.input(|i| i.modifiers.shift);
            *act = Some(TreeAction::Select(o.id.clone(), add));
        }
        if r.drag_started() {
            self.drag = Some(o.id.clone());
        }
        r.context_menu(|ui| {
            if let Some(a) = object_menu(ui, o) {
                *act = Some(a);
            }
        });
    }

    /// Quadrado 1:1 com a miniatura do objeto.
    fn thumb(&mut self, ui: &mut egui::Ui, doc: &DocState, o: &Object) {
        let side = theme::OBJECT_THUMB;
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(side), egui::Sense::hover());
        let pal = theme::palette(ui);
        ui.painter()
            .rect_filled(rect, theme::RADIUS_XS, theme::PAGE_PAPER);
        let Some(r) = doc.editor.find(&o.id) else {
            return;
        };
        let parent = doc.editor.parent_affine(&r);
        let fresh = self
            .thumbs
            .get(&o.id)
            .is_some_and(|(c, p, _)| c == o && *p == parent);
        if !fresh {
            if self.rendered < THUMBS_PER_FRAME {
                self.rendered += 1;
                let px = (side * ui.ctx().pixels_per_point()).round().max(1.0) as u32;
                if let Some(img) = doc.object_thumbnail(&r, px) {
                    let name = format!("miniatura-objeto-{}", o.id);
                    match self.thumbs.get_mut(&o.id) {
                        Some((c, p, t)) => {
                            t.set(img, egui::TextureOptions::LINEAR);
                            *c = o.clone();
                            *p = parent;
                        }
                        None => {
                            let t = ui
                                .ctx()
                                .load_texture(name, img, egui::TextureOptions::LINEAR);
                            self.thumbs.insert(o.id.clone(), (o.clone(), parent, t));
                        }
                    }
                }
            } else {
                ui.ctx().request_repaint();
            }
        }
        if let Some((_, _, t)) = self.thumbs.get(&o.id) {
            egui::Image::new((t.id(), Vec2::splat(side))).paint_at(ui, rect);
        }
        ui.painter().rect_stroke(
            rect,
            theme::RADIUS_XS,
            egui::Stroke::new(theme::BORDER_WIDTH, pal.border_subtle),
            egui::StrokeKind::Inside,
        );
    }

    /// Fim do quadro: desenha o indicador do arrasto e, ao soltar, devolve
    /// o destino.
    pub fn finish(&mut self, ui: &mut egui::Ui, act: &mut Option<TreeAction>) {
        let Some(id) = self.drag.clone() else {
            return;
        };
        let (pos, released) = ui.input(|i| (i.pointer.interact_pos(), i.pointer.any_released()));
        let target = pos.and_then(|p| {
            self.spots
                .iter()
                .find(|(r, _)| r.contains(p))
                .map(|(r, s)| (*r, s.clone(), p))
        });
        let drop = target.and_then(|(r, s, p)| match s {
            Spot::Object(t) if t == id => None,
            Spot::Object(t) => Some((
                r,
                if p.y < r.center().y {
                    DropAt::Above(t)
                } else {
                    DropAt::Below(t)
                },
            )),
            Spot::Layer { page, layer } => Some((r, DropAt::LayerTop { page, layer })),
        });
        let painter = ui.ctx().layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            ui.scope_id().with("arrasto-camadas"),
        ));
        let line = egui::Stroke::new(theme::FOCUS_WIDTH, theme::ACCENT_PRIMARY);
        match &drop {
            Some((r, DropAt::Above(_))) => {
                painter.hline(r.x_range(), r.top(), line);
            }
            Some((r, DropAt::Below(_))) => {
                painter.hline(r.x_range(), r.bottom(), line);
            }
            Some((r, DropAt::LayerTop { .. })) => {
                painter.rect_stroke(*r, theme::RADIUS_XS, line, egui::StrokeKind::Inside);
            }
            None => {}
        }
        if released {
            self.drag = None;
            if let Some((_, at)) = drop {
                *act = Some(TreeAction::Drop(id, at));
            }
        } else {
            ui.ctx().request_repaint();
        }
    }
}

/// Menu de edição rápida do objeto (botão direito no painel ou no canvas):
/// travar/destravar, mostrar/ocultar e o nome (Enter confirma).
pub fn object_menu(ui: &mut egui::Ui, o: &Object) -> Option<TreeAction> {
    let mut act = None;
    let key = egui::Id::unique(("menu-nome-objeto", &o.id));
    let mut buf: String = ui
        .data_mut(|d| d.get_temp::<String>(key))
        .unwrap_or_else(|| o.name.clone().unwrap_or_default());
    ui.horizontal(|ui| {
        ui.label(tr("Nome", "Name"));
        let r = ui.add(
            egui::TextEdit::singleline(&mut buf)
                .hint_text(display_name(o))
                .desired_width(theme::MENU_FIELD_WIDTH),
        );
        ui.data_mut(|d| d.insert_temp(key, buf.clone()));
        if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            let t = buf.trim().to_string();
            act = Some(TreeAction::Rename(
                o.id.clone(),
                (!t.is_empty()).then_some(t),
            ));
            ui.data_mut(|d| d.remove::<String>(key));
            ui.close();
        }
    });
    ui.separator();
    let lock = if o.locked {
        tr("Destravar", "Unlock")
    } else {
        tr("Travar", "Lock")
    };
    if ui.button(lock).clicked() {
        act = Some(TreeAction::Locked(o.id.clone(), !o.locked));
        ui.close();
    }
    let vis = if o.visible {
        tr("Ocultar", "Hide")
    } else {
        tr("Mostrar", "Show")
    };
    if ui.button(vis).clicked() {
        act = Some(TreeAction::Visible(o.id.clone(), !o.visible));
        ui.close();
    }
    act
}

/// Aplica a ação de uma linha de objeto.
pub fn apply(doc: &mut DocState, act: TreeAction) {
    match act {
        TreeAction::Select(id, add) => {
            if add {
                if let Some(i) = doc.selection.iter().position(|s| *s == id) {
                    doc.selection.remove(i);
                } else {
                    doc.selection.push(id);
                }
            } else {
                doc.selection = vec![id];
            }
        }
        TreeAction::Visible(id, v) => {
            let name = if v {
                tr("Mostrar objeto", "Show object")
            } else {
                tr("Ocultar objeto", "Hide object")
            };
            ops::modify(&mut doc.editor, std::slice::from_ref(&id), name, |o| {
                o.visible = v;
                true
            });
        }
        TreeAction::Locked(id, v) => {
            let name = if v {
                tr("Travar objeto", "Lock object")
            } else {
                tr("Destravar objeto", "Unlock object")
            };
            ops::modify(&mut doc.editor, std::slice::from_ref(&id), name, |o| {
                o.locked = v;
                true
            });
            // Objeto travado sai da seleção (não pode ser editado).
            if v {
                doc.selection.retain(|s| *s != id);
            }
        }
        TreeAction::Rename(id, name) => {
            ops::modify(
                &mut doc.editor,
                std::slice::from_ref(&id),
                tr("Renomear objeto", "Rename object"),
                |o| {
                    let changed = o.name != name;
                    o.name = name.clone();
                    changed
                },
            );
        }
        TreeAction::Drop(id, at) => {
            ops::move_to(&mut doc.editor, &id, &at);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use graphdraw_core::model::{GroupData, Layer, RectData};

    fn rect() -> Object {
        Object::new(ObjectKind::Rectangle(RectData {
            width: 10_000.0,
            height: 10_000.0,
            corners: None,
            extra: serde_json::Map::new(),
        }))
    }

    /// Página com um grupo (2 retângulos) na camada 0 e uma camada vazia.
    fn sample() -> (DocState, [String; 3]) {
        let mut doc = DocState::new_blank();
        let (a, b) = (rect(), rect());
        let ids = [a.id.clone(), b.id.clone(), String::new()];
        let group = Object::new(ObjectKind::Group(GroupData {
            children: vec![a, b],
            extra: serde_json::Map::new(),
        }));
        let g = group.id.clone();
        let loc = graphdraw_core::edit::ObjLoc {
            page: 0,
            layer: 0,
            index: 0,
        };
        doc.editor.insert_object(loc, group);
        doc.editor.insert_layer(0, 1, Layer::new("B"));
        (doc, [ids[0].clone(), ids[1].clone(), g])
    }

    /// O painel desenha camadas com os objetos dentro e as miniaturas.
    #[test]
    fn tree_draws_objects_inside_layers_with_thumbnails() {
        let (doc, ids) = sample();
        let ctx = crate::theme::test_ctx();
        let mut tree = LayerTree::default();
        for _ in 0..2 {
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(
                    egui::Pos2::ZERO,
                    Vec2::new(1366.0, 768.0),
                )),
                ..Default::default()
            };
            let mut out = ctx.run_ui(input, |ui| {
                egui::CentralPanel::default().show(ui, |ui| {
                    tree.begin();
                    let mut act = None;
                    tree.layer_objects(ui, &doc, 0, &mut act);
                    tree.finish(ui, &mut act);
                    assert!(act.is_none());
                });
            });
            out.textures_delta.clear();
        }
        // Grupo recolhido: só a linha dele, com miniatura.
        assert!(tree.thumbs.contains_key(&ids[2]));
        assert!(
            tree.spots
                .iter()
                .any(|(_, s)| matches!(s, Spot::Object(i) if *i == ids[2]))
        );
    }

    /// A miniatura mostra o objeto inteiro, centrado no quadrado.
    #[test]
    fn thumbnail_shows_whole_object_centered() {
        use graphdraw_core::model::{Color, Fill, Transform};
        let mut doc = DocState::new_blank();
        let mut o = Object::new(ObjectKind::Rectangle(RectData {
            width: 40_000.0,
            height: 10_000.0,
            corners: None,
            extra: serde_json::Map::new(),
        }));
        o.transform = Transform::translate(150_000.0, 200_000.0);
        o.fill = Some(Fill::solid(Color::black()));
        let id = o.id.clone();
        let loc = graphdraw_core::edit::ObjLoc {
            page: 0,
            layer: 0,
            index: 0,
        };
        doc.editor.insert_object(loc, o);
        let r = doc.editor.find(&id).unwrap();
        let img = doc.object_thumbnail(&r, 24).unwrap();
        let alpha = |x: usize, y: usize| img.pixels[y * 24 + x].a();
        assert!(alpha(12, 12) > 200, "centro preenchido");
        assert!(alpha(1, 12) > 200 && alpha(22, 12) > 200, "largura inteira");
        assert_eq!(alpha(12, 2), 0, "faixa larga: sobra em cima");
    }

    /// Renomear, soltar em outra camada, ocultar e travar; cada um desfaz.
    #[test]
    fn actions_rename_move_hide_and_lock() {
        let (mut doc, [a, _, g]) = sample();
        apply(&mut doc, TreeAction::Rename(g.clone(), Some("Logo".into())));
        let name = |doc: &DocState, id: &str| {
            let r = doc.editor.find(id).unwrap();
            display_name(doc.editor.get(&r).unwrap())
        };
        assert_eq!(name(&doc, &g), "Logo");
        apply(&mut doc, TreeAction::Rename(g.clone(), None));
        assert_eq!(
            name(&doc, &g),
            crate::app::kind_label(&ObjectKind::Group(GroupData {
                children: vec![],
                extra: serde_json::Map::new(),
            }))
        );
        // Filho do grupo solto no topo da camada B.
        apply(
            &mut doc,
            TreeAction::Drop(a.clone(), DropAt::LayerTop { page: 0, layer: 1 }),
        );
        assert_eq!(doc.page().layers[1].objects[0].id, a);
        apply(&mut doc, TreeAction::Select(a.clone(), false));
        apply(&mut doc, TreeAction::Visible(a.clone(), false));
        apply(&mut doc, TreeAction::Locked(a.clone(), true));
        let o = &doc.page().layers[1].objects[0];
        assert!(!o.visible && o.locked);
        assert!(doc.selection.is_empty(), "travado sai da seleção");
        doc.editor.undo();
        doc.editor.undo();
        doc.editor.undo();
        assert!(
            doc.page().layers[1].objects.is_empty(),
            "desfazer volta o objeto ao grupo"
        );
    }
}
