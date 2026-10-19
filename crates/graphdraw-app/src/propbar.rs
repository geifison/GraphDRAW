//! Barra de propriedades — regras gerais (doc 31, GD-BPR-01..10).
//!
//! A barra é montada em **grupos** (os blocos do doc 31, como `B-TRF`) com
//! **itens** dentro. Cada contexto tem uma chave estável
//! `janela/ferramenta/seleção[/tarefa]` (D-31-09), que guarda a
//! personalização do usuário (GD-BPR-07, D-31-07). Grupos que não cabem
//! vão para o botão » (GD-BPR-08).

use std::collections::{BTreeMap, BTreeSet};

use eframe::egui;
use graphdraw_core::model::{Object, ObjectKind};
use serde::{Deserialize, Serialize};

use crate::i18n::tr;
use crate::keymap::Command;
use crate::{icons, theme};

/// Janela principal (a prévia de impressão terá a sua, GD-IMP-32).
pub const MAIN: &str = "main";

/// Chave do contexto (GD-BPR-01, D-31-09): `janela/ferramenta/seleção`,
/// mais `/tarefa` quando um modo especial sobrepõe a ferramenta.
pub fn context_key(window: &str, tool: &str, selection: &str, task: Option<&str>) -> String {
    let mut k = format!("{window}/{tool}/{selection}");
    if let Some(t) = task {
        k.push('/');
        k.push_str(t);
    }
    k
}

/// Nome da ferramenta na chave: o ícone sem o prefixo `tool-`.
pub fn tool_key(icon: &str) -> &str {
    icon.strip_prefix("tool-").unwrap_or(icon)
}

/// Tipo da seleção na chave: `none`, o tipo comum a todos os objetos ou
/// `multi` quando os tipos diferem (GD-BPR-02).
pub fn selection_kind(objs: &[&Object]) -> &'static str {
    let mut kinds = objs.iter().map(|o| object_kind(o));
    match kinds.next() {
        None => "none",
        Some(first) if kinds.all(|k| k == first) => first,
        Some(_) => "multi",
    }
}

/// Valor de um campo sobre vários objetos (GD-BPR-02): igual em todos,
/// diferente ("vários") ou ausente (nenhum objeto tem o atributo).
#[derive(Debug, Clone, PartialEq)]
pub enum Mixed<T> {
    None,
    Same(T),
    Various,
}

impl<T: PartialEq> Mixed<T> {
    pub fn of(values: impl IntoIterator<Item = T>) -> Mixed<T> {
        let mut it = values.into_iter();
        match it.next() {
            None => Mixed::None,
            Some(first) if it.all(|v| v == first) => Mixed::Same(first),
            Some(_) => Mixed::Various,
        }
    }

    /// Valor a mostrar no campo; `None` = campo vazio.
    pub fn value(&self) -> Option<&T> {
        match self {
            Mixed::Same(v) => Some(v),
            _ => None,
        }
    }
}

/// Tipo de um objeto na chave de contexto (doc 31, seção 2).
fn object_kind(o: &Object) -> &'static str {
    match &o.kind {
        ObjectKind::Path(_) | ObjectKind::BSpline(_) => "curve",
        ObjectKind::Rectangle(_) => "rect",
        ObjectKind::Ellipse(_) => "ellipse",
        ObjectKind::Polygon(p) if p.mode == "polygon" => "polygon",
        ObjectKind::Polygon(_) => "star",
        ObjectKind::Spiral(_) => "spiral",
        ObjectKind::Group(_) => "group",
        ObjectKind::Text(_) => "text",
        ObjectKind::Image(_) => "image",
        ObjectKind::Dimension(_) => "dimension",
        ObjectKind::Basic(_) => "basic",
        ObjectKind::Connector(_) => "connector",
        ObjectKind::Unknown { .. } => "unknown",
    }
}

/// Grupo como a barra o vê para decidir o transbordo.
#[derive(Debug, Clone, PartialEq)]
pub struct GroupSpec {
    pub id: String,
    pub width: f32,
    /// Essencial: é o último a ir para o » (GD-BPR-08).
    pub essential: bool,
}

/// Quais grupos ficam na barra (`true`) e quais vão para o » (GD-BPR-08):
/// corta da direita para a esquerda, os essenciais por último. `gap` é o
/// espaço entre grupos e `more` a largura do botão ».
pub fn fit(groups: &[GroupSpec], avail: f32, gap: f32, more: f32) -> Vec<bool> {
    let mut shown = vec![true; groups.len()];
    let used = |shown: &[bool]| -> f32 {
        let ws: Vec<f32> = groups
            .iter()
            .zip(shown)
            .filter(|(_, s)| **s)
            .map(|(g, _)| g.width)
            .collect();
        ws.iter().sum::<f32>() + gap * ws.len().saturating_sub(1) as f32
    };
    if used(&shown) <= avail {
        return shown;
    }
    // Ordem de corte: não essenciais da direita para a esquerda, depois os
    // essenciais; o primeiro grupo nunca sai.
    let order = (1..groups.len())
        .rev()
        .filter(|&i| !groups[i].essential)
        .chain((1..groups.len()).rev().filter(|&i| groups[i].essential));
    for i in order {
        shown[i] = false;
        if used(&shown) + gap + more <= avail {
            break;
        }
    }
    shown
}

/// Personalização de um contexto (GD-BPR-07, D-31-07).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Custom {
    /// Grupos e itens ocultos (`B-TRF` ou `B-TRF/angle`).
    pub hidden: BTreeSet<String>,
    /// Comandos acrescentados (`Command::id`), na ordem em que entraram.
    pub added: Vec<String>,
    /// Ordem dos grupos escolhida pelo usuário.
    pub order: Vec<String>,
}

/// Personalização de todos os contextos, pela chave.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Customs(pub BTreeMap<String, Custom>);

impl Customs {
    pub fn get(&self, ctx: &str) -> Option<&Custom> {
        self.0.get(ctx)
    }

    pub fn is_hidden(&self, ctx: &str, id: &str) -> bool {
        self.get(ctx).is_some_and(|c| c.hidden.contains(id))
    }

    /// Mostra ou oculta um grupo ou item só neste contexto.
    pub fn set_hidden(&mut self, ctx: &str, id: &str, hidden: bool) {
        self.edit(ctx, |c| {
            if hidden {
                c.hidden.insert(id.to_string());
            } else {
                c.hidden.remove(id);
            }
        });
    }

    /// Acrescenta um comando só neste contexto (sem repetir).
    pub fn add_command(&mut self, ctx: &str, command: &str) {
        self.edit(ctx, |c| {
            if !c.added.iter().any(|a| a == command) {
                c.added.push(command.to_string());
            }
        });
    }

    pub fn remove_command(&mut self, ctx: &str, command: &str) {
        self.edit(ctx, |c| c.added.retain(|a| a != command));
    }

    /// *Restaurar padrão deste contexto*.
    pub fn restore(&mut self, ctx: &str) {
        self.0.remove(ctx);
    }

    /// Grupos na ordem do usuário: os que ele ordenou primeiro (se ainda
    /// existem), os outros na ordem padrão, cada um logo depois do grupo
    /// que o precedia na ordem padrão.
    pub fn ordered(&self, ctx: &str, default: &[String]) -> Vec<String> {
        let Some(c) = self.get(ctx).filter(|c| !c.order.is_empty()) else {
            return default.to_vec();
        };
        let mut out: Vec<String> = c
            .order
            .iter()
            .filter(|id| default.contains(id))
            .cloned()
            .collect();
        for (i, id) in default.iter().enumerate() {
            if out.contains(id) {
                continue;
            }
            let at = default[..i]
                .iter()
                .rev()
                .find_map(|prev| out.iter().position(|o| o == prev))
                .map_or(0, |p| p + 1);
            out.insert(at, id.clone());
        }
        out
    }

    /// Move o grupo `id` para a posição `to` da ordem atual.
    pub fn move_group(&mut self, ctx: &str, current: &[String], id: &str, to: usize) {
        let mut order: Vec<String> = current.iter().filter(|g| *g != id).cloned().collect();
        if order.len() == current.len() {
            return;
        }
        order.insert(to.min(order.len()), id.to_string());
        self.edit(ctx, |c| c.order = order);
    }

    /// Altera a personalização do contexto; contexto que volta ao padrão
    /// sai do arquivo.
    fn edit(&mut self, ctx: &str, f: impl FnOnce(&mut Custom)) {
        let c = self.0.entry(ctx.to_string()).or_default();
        f(c);
        if *c == Custom::default() {
            self.0.remove(ctx);
        }
    }
}

// ---------------------------------------------------------------------
// Montagem na tela
// ---------------------------------------------------------------------

/// Espaço entre grupos: `space-200` de cada lado do divisor (DS 4.6).
const GAP: f32 = 2.0 * theme::SPACE_200;
/// Botões da própria barra (alça, », ⋯): Compact.
const BTN: f32 = theme::BUTTON_COMPACT;

/// Grupo dos comandos acrescentados pelo usuário (GD-BPR-07).
const USER_GROUP: &str = "B-USR";

/// Lado da janela em que a barra fica (GD-BPR-06, D-31-06).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Dock {
    #[default]
    Top,
    Bottom,
}

/// Passada do desenho: a linha da barra ou o popup do » com os grupos que
/// não couberam (GD-BPR-08). O contexto roda nas duas; cada grupo só
/// aparece numa delas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pass {
    Inline,
    Overflow,
}

/// Pedido da barra para a janela principal.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Hide {
        id: String,
        hidden: bool,
    },
    Add(String),
    Remove(String),
    Restore,
    Move {
        id: String,
        to: usize,
    },
    Run(Command),
    Dock(Dock),
    /// Abrir um painel lateral pela chave (ex.: `alinhar`).
    Panel(&'static str),
}

/// Grupo como a barra o mostrou no último quadro.
#[derive(Debug, Clone, PartialEq)]
pub struct SeenGroup {
    pub id: String,
    pub label: String,
    pub essential: bool,
    pub width: f32,
    /// Itens (id, rótulo) que o grupo registrou.
    pub items: Vec<(String, String)>,
}

/// Registro do contexto mostrado: base do » e do menu ⋯ no quadro seguinte.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Seen {
    pub key: String,
    /// Grupos na ordem padrão (a do código).
    pub groups: Vec<SeenGroup>,
    /// Grupos que foram para o ».
    pub overflow: Vec<String>,
}

fn seen_id() -> egui::Id {
    egui::Id::unique("barra-propriedades-registro")
}

fn more_id() -> egui::Id {
    egui::Id::unique("barra-propriedades-transbordo")
}

/// O que a barra mostrou no último quadro.
pub fn seen(ctx: &egui::Context) -> Seen {
    ctx.data(|d| d.get_temp::<Seen>(seen_id()))
        .unwrap_or_default()
}

/// Ponto (canto superior direito) onde abre o popup do », se aberto.
pub fn overflow_anchor(ctx: &egui::Context) -> Option<egui::Pos2> {
    ctx.data(|d| d.get_temp::<egui::Pos2>(more_id()))
}

/// Abre o popup do » num ponto (capturas de tela dos testes).
#[cfg(test)]
pub fn open_overflow(ctx: &egui::Context, at: egui::Pos2) {
    ctx.data_mut(|d| d.insert_temp(more_id(), at));
}

pub fn close_overflow(ctx: &egui::Context) {
    ctx.data_mut(|d| d.remove::<egui::Pos2>(more_id()));
}

/// Itens de um grupo: cada um entra no menu ⋯ e pode ser oculto.
pub struct Items<'b> {
    key: &'b str,
    group: &'b str,
    custom: &'b Customs,
    list: Vec<(String, String)>,
    actions: Vec<Action>,
}

impl Items<'_> {
    /// Pedido para a janela (rodar comando, abrir painel).
    pub fn act(&mut self, a: Action) {
        self.actions.push(a);
    }

    /// Registra o item e diz se ele aparece (GD-BPR-07).
    pub fn show(&mut self, id: &str, label: &str) -> bool {
        self.list.push((id.to_string(), label.to_string()));
        !self
            .custom
            .is_hidden(self.key, &format!("{}/{}", self.group, id))
    }
}

/// Barra de um quadro: o contexto chama [`Bar::group`] para cada grupo,
/// na ordem padrão, e [`Bar::end`] no fim.
pub struct Bar<'a> {
    key: String,
    pass: Pass,
    custom: &'a Customs,
    locked: bool,
    prev: Seen,
    now: Vec<SeenGroup>,
    /// Grupos na linha e onde começam.
    plan: Vec<(String, f32)>,
    overflow: Vec<String>,
    rect: egui::Rect,
    /// Onde começa o primeiro grupo (depois da alça).
    start: f32,
    /// Fim do último grupo desenhado na linha.
    right: f32,
    changed: bool,
    actions: Vec<Action>,
}

impl<'a> Bar<'a> {
    pub fn begin(
        ui: &egui::Ui,
        key: String,
        custom: &'a Customs,
        pass: Pass,
        locked: bool,
    ) -> Bar<'a> {
        let mut prev = seen(ui.ctx());
        let changed = prev.key != key;
        if changed {
            prev = Seen {
                key: key.clone(),
                ..Default::default()
            };
        }
        let rect = ui.max_rect();
        let start = rect.left() + BTN + GAP;
        let mut bar = Bar {
            key,
            pass,
            custom,
            locked,
            plan: Vec::new(),
            overflow: prev.overflow.clone(),
            now: Vec::new(),
            prev,
            rect,
            start,
            right: start,
            changed,
            actions: Vec::new(),
        };
        if pass == Pass::Inline {
            bar.layout();
        }
        bar
    }

    /// Posições do quadro a partir das larguras do anterior: ordem do
    /// usuário, ocultos fora, transbordo pelo [`fit`].
    fn layout(&mut self) {
        let ids: Vec<String> = self.prev.groups.iter().map(|g| g.id.clone()).collect();
        let order: Vec<&SeenGroup> = self
            .custom
            .ordered(&self.key, &ids)
            .iter()
            .filter(|id| !self.custom.is_hidden(&self.key, id))
            .filter_map(|id| self.prev.groups.iter().find(|g| &g.id == id))
            .collect();
        // Grupo vazio (não se aplica à seleção) não ocupa lugar nem vai
        // para o »; se passar a ter conteúdo, entra no quadro seguinte.
        let (order, empty): (Vec<&SeenGroup>, Vec<&SeenGroup>) =
            order.into_iter().partition(|g| g.width > 0.5);
        let specs: Vec<GroupSpec> = order
            .iter()
            .map(|g| GroupSpec {
                id: g.id.clone(),
                width: g.width,
                essential: g.essential,
            })
            .collect();
        // À direita ficam o ⋯ e o divisor antes dele.
        let avail = self.rect.right() - (BTN + GAP) - self.start;
        let shown = fit(&specs, avail, GAP, BTN);
        let mut x = self.start;
        self.overflow.clear();
        for (s, on) in specs.iter().zip(shown) {
            if on {
                self.plan.push((s.id.clone(), x));
                x += s.width + GAP;
            } else {
                self.overflow.push(s.id.clone());
            }
        }
        for g in empty {
            self.plan.push((g.id.clone(), x));
        }
    }

    /// Um grupo (bloco do doc 31). `essential` = último a ir para o ».
    pub fn group(
        &mut self,
        ui: &mut egui::Ui,
        id: &str,
        label: &str,
        essential: bool,
        f: impl FnOnce(&mut egui::Ui, &mut Items),
    ) {
        let prev = self.prev.groups.iter().find(|g| g.id == id).cloned();
        if prev.is_none() {
            self.changed = true;
        }
        let mut seen = SeenGroup {
            id: id.to_string(),
            label: label.to_string(),
            essential,
            width: prev.as_ref().map_or(0.0, |g| g.width),
            items: prev.map(|g| g.items).unwrap_or_default(),
        };
        let out = self.overflow.iter().any(|o| o == id);
        let here = !self.custom.is_hidden(&self.key, id)
            && match self.pass {
                Pass::Inline => !out,
                Pass::Overflow => out,
            };
        if here {
            let mut items = Items {
                key: &self.key,
                group: id,
                custom: self.custom,
                list: Vec::new(),
                actions: Vec::new(),
            };
            let width = match self.pass {
                Pass::Inline => {
                    let x = match self.plan.iter().find(|p| p.0 == id) {
                        Some(p) => p.1,
                        // Grupo novo: depois do último; acerta no próximo quadro.
                        None => {
                            self.changed = true;
                            if self.right > self.start {
                                self.right + GAP
                            } else {
                                self.start
                            }
                        }
                    };
                    let r = egui::Rect::from_min_max(
                        egui::pos2(x, self.rect.top()),
                        egui::pos2(self.rect.right().max(x), self.rect.bottom()),
                    );
                    let mut child = ui.new_child(
                        egui::UiBuilder::new()
                            .max_rect(r)
                            .layout(egui::Layout::left_to_right(egui::Align::Center)),
                    );
                    f(&mut child, &mut items);
                    let w = child.min_rect().width();
                    if w > 0.5 && x > self.start + 0.5 {
                        divider(ui, x - GAP / 2.0, self.rect);
                    }
                    self.right = self.right.max(x + w);
                    w
                }
                Pass::Overflow => {
                    // No popup, um grupo por linha, com divisor entre eles.
                    if self.right > self.start {
                        ui.separator();
                    }
                    self.right = self.start + 1.0;
                    ui.horizontal(|ui| f(ui, &mut items)).response.rect.width()
                }
            };
            if (width - seen.width).abs() > 0.5 {
                self.changed = true;
            }
            seen.width = width;
            seen.items = items.list;
            self.actions.extend(items.actions);
        }
        self.now.push(seen);
    }

    /// Fecha a barra: comandos acrescentados, dica da ferramenta (cortada no
    /// espaço que sobra), alça, » e ⋯. Devolve os pedidos do usuário.
    pub fn end(mut self, ui: &mut egui::Ui, hint: Option<&str>) -> Vec<Action> {
        let added: Vec<Command> = self
            .custom
            .get(&self.key)
            .map(|c| c.added.iter().filter_map(|a| Command::from_id(a)).collect())
            .unwrap_or_default();
        if !added.is_empty() {
            let mut run = None;
            self.group(
                ui,
                USER_GROUP,
                tr("Comandos acrescentados", "Added commands"),
                false,
                |ui, it| {
                    for c in &added {
                        if it.show(c.id(), c.label())
                            && icons::button(
                                ui,
                                c.icon(),
                                crate::keymap::hint(c.label(), *c),
                                false,
                                c.ready(),
                            )
                            .clicked()
                        {
                            run = Some(*c);
                        }
                    }
                },
            );
            if let Some(c) = run {
                self.actions.push(Action::Run(c));
            }
        }
        match self.pass {
            Pass::Inline => self.end_inline(ui, hint),
            Pass::Overflow => {
                // Larguras medidas no popup voltam ao registro.
                let now = std::mem::take(&mut self.now);
                ui.ctx().data_mut(|d| {
                    let s = d.get_temp_mut_or_default::<Seen>(seen_id());
                    for g in now.into_iter().filter(|g| self.overflow.contains(&g.id)) {
                        if let Some(old) = s.groups.iter_mut().find(|o| o.id == g.id) {
                            *old = g;
                        }
                    }
                });
            }
        }
        if self.changed {
            ui.ctx().request_repaint();
        }
        self.actions
    }

    fn end_inline(&mut self, ui: &mut egui::Ui, hint: Option<&str>) {
        let rect = self.rect;
        let cy = rect.center().y;
        let at =
            |x: f32| egui::Rect::from_min_size(egui::pos2(x, cy - BTN / 2.0), egui::vec2(BTN, BTN));
        // Alça (GD-BPR-06): arrastar para a metade de baixo leva a barra
        // para baixo, e vice-versa; bloqueada não move.
        let grip = at(rect.left());
        let sense = if self.locked {
            egui::Sense::hover()
        } else {
            egui::Sense::drag()
        };
        let g = ui
            .interact(grip, ui.scope_id().with("alca"), sense)
            .on_hover_text(if self.locked {
                tr(
                    "Barras bloqueadas (Janela › Barras › Bloquear barras)",
                    "Bars locked (Window › Bars › Lock bars)",
                )
            } else {
                tr(
                    "Arraste para levar a barra para cima ou para baixo",
                    "Drag to move the bar to the top or bottom",
                )
            });
        if !self.locked {
            g.clone().on_hover_cursor(egui::CursorIcon::Grab);
        }
        paint_icon(ui, grip, "bar-grip", g.hovered() && !self.locked);
        if g.drag_stopped()
            && let Some(p) = ui.ctx().pointer_latest_pos()
        {
            let screen = ui.ctx().content_rect();
            self.actions.push(Action::Dock(if p.y > screen.center().y {
                Dock::Bottom
            } else {
                Dock::Top
            }));
        }
        // ⋯ no fim; » antes dele quando há transbordo.
        let more = at(rect.right() - BTN);
        let over = at(more.left() - theme::SPACE_100 - BTN);
        let mut limit = more.left() - theme::SPACE_200;
        if self.overflow.is_empty() {
            close_overflow(ui.ctx());
        } else {
            limit = over.left() - theme::SPACE_200;
            let open = overflow_anchor(ui.ctx()).is_some();
            let tip = crate::keymap::intern_tip(format!(
                "{} ({})",
                tr("Mais controles", "More controls"),
                self.overflow.len()
            ));
            if ui
                .put(over, |ui: &mut egui::Ui| {
                    icons::button(ui, "bar-overflow", tip, open, true)
                })
                .clicked()
            {
                if open {
                    close_overflow(ui.ctx());
                } else {
                    let p = over.right_bottom() + egui::vec2(0.0, theme::SPACE_150);
                    ui.ctx().data_mut(|d| d.insert_temp(more_id(), p));
                }
            }
        }
        if self.right > self.start && limit > self.right + GAP {
            divider(ui, limit + theme::SPACE_100, rect);
        }
        // Dica da ferramenta no espaço que sobra.
        if let Some(t) = hint {
            let x = if self.right > self.start {
                self.right + GAP
            } else {
                self.start
            };
            if limit - x > theme::SPACE_600 {
                if self.right > self.start {
                    divider(ui, x - GAP / 2.0, rect);
                }
                let r = egui::Rect::from_min_max(
                    egui::pos2(x, rect.top()),
                    egui::pos2(limit, rect.bottom()),
                );
                let mut child = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(r)
                        .layout(egui::Layout::left_to_right(egui::Align::Center)),
                );
                child.add(egui::Label::new(t).truncate()).on_hover_text(t);
            }
        }
        let resp = ui.put(more, |ui: &mut egui::Ui| {
            icons::button(
                ui,
                "bar-more",
                tr(
                    "Personalizar esta barra (mostrar, ocultar, acrescentar comandos)",
                    "Customize this bar (show, hide, add commands)",
                ),
                false,
                true,
            )
        });
        let mut acts = Vec::new();
        egui::Popup::from_toggle_button_response(&resp)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| acts = self.menu(ui));
        self.actions.extend(acts);
        ui.advance_cursor_after_rect(rect);
        let record = Seen {
            key: self.key.clone(),
            groups: std::mem::take(&mut self.now),
            overflow: self.overflow.clone(),
        };
        ui.ctx().data_mut(|d| d.insert_temp(seen_id(), record));
    }

    /// Menu ⋯ (GD-BPR-07): grupos e itens com caixa de marcação,
    /// reordenar com Alt + arrastar, acrescentar comando e restaurar.
    fn menu(&self, ui: &mut egui::Ui) -> Vec<Action> {
        theme::menu_density(ui);
        let mut acts = Vec::new();
        let ids: Vec<String> = self.now.iter().map(|g| g.id.clone()).collect();
        let order = self.custom.ordered(&self.key, &ids);
        let alt = ui.input(|i| i.modifiers.alt);
        let mut rows = Vec::new();
        let mut dragged = None;
        for id in &order {
            let Some(g) = self.now.iter().find(|g| &g.id == id) else {
                continue;
            };
            let mut on = !self.custom.is_hidden(&self.key, &g.id);
            let r = ui.checkbox(&mut on, &g.label);
            if r.changed() {
                acts.push(Action::Hide {
                    id: g.id.clone(),
                    hidden: !on,
                });
            }
            let d = ui.interact(r.rect, r.id.with("arrastar"), egui::Sense::drag());
            if alt && d.drag_stopped() {
                dragged = Some(g.id.clone());
            }
            rows.push(r.rect);
            ui.indent(("itens", &g.id), |ui| {
                for (iid, label) in &g.items {
                    let full = format!("{}/{iid}", g.id);
                    let mut on = !self.custom.is_hidden(&self.key, &full);
                    if ui.checkbox(&mut on, label).changed() {
                        // Desmarcar um comando acrescentado o retira.
                        acts.push(if g.id == USER_GROUP {
                            Action::Remove(iid.clone())
                        } else {
                            Action::Hide {
                                id: full,
                                hidden: !on,
                            }
                        });
                    }
                }
            });
        }
        if let (Some(id), Some(p)) = (dragged, ui.ctx().pointer_latest_pos()) {
            let to = rows.iter().filter(|r| r.center().y < p.y).count();
            let from = order.iter().position(|o| *o == id).unwrap_or(0);
            acts.push(Action::Move {
                id,
                to: if to > from { to - 1 } else { to },
            });
        }
        ui.label(
            egui::RichText::new(tr(
                "Alt + arrastar muda a ordem dos grupos",
                "Alt + drag reorders the groups",
            ))
            .small(),
        );
        ui.separator();
        ui.menu_button(tr("Adicionar comando…", "Add command…"), |ui| {
            egui::ScrollArea::vertical()
                .max_height(theme::MENU_SCROLL_HEIGHT)
                .show(ui, |ui| {
                    for c in Command::ALL.iter().filter(|c| c.ready()) {
                        if ui.add(crate::keymap::item(ui, c.label(), *c)).clicked() {
                            acts.push(Action::Add(c.id().to_string()));
                            ui.close();
                        }
                    }
                });
        });
        if ui
            .add(crate::keymap::icon_item(
                ui,
                tr(
                    "Restaurar padrão deste contexto",
                    "Restore this context's default",
                ),
                "bar-more",
            ))
            .clicked()
        {
            acts.push(Action::Restore);
            ui.close();
        }
        acts
    }
}

/// Divisor vertical entre grupos.
fn divider(ui: &egui::Ui, x: f32, rect: egui::Rect) {
    let stroke = ui.visuals().widgets.noninteractive.bg_stroke;
    let y = rect.y_range().shrink(theme::SPACE_150);
    ui.painter().vline(x.round() + 0.5, y, stroke);
}

/// Ícone de 20 px centrado no retângulo, na cor do tema.
fn paint_icon(ui: &egui::Ui, r: egui::Rect, name: &str, hover: bool) {
    let p = theme::palette(ui);
    let tint = if hover { p.fg_strong } else { p.fg_muted };
    egui::Image::new(icons::get(name)).tint(tint).paint_at(
        ui,
        egui::Rect::from_center_size(r.center(), egui::Vec2::splat(theme::ICON_UI)),
    );
}

/// Aplica os pedidos do ⋯ à personalização (o resto fica com a janela).
pub fn apply(custom: &mut Customs, key: &str, actions: &[Action], current: &[String]) {
    for a in actions {
        match a {
            Action::Hide { id, hidden } => custom.set_hidden(key, id, *hidden),
            Action::Add(c) => custom.add_command(key, c),
            Action::Remove(c) => custom.remove_command(key, c),
            Action::Restore => custom.restore(key),
            Action::Move { id, to } => custom.move_group(key, current, id, *to),
            Action::Run(_) | Action::Dock(_) | Action::Panel(_) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use graphdraw_core::model::{EllipseData, GroupData, Object, ObjectKind, RectData};

    fn rect() -> Object {
        Object::new(ObjectKind::Rectangle(RectData {
            width: 1.0,
            height: 1.0,
            corners: None,
            extra: Default::default(),
        }))
    }

    fn ellipse() -> Object {
        Object::new(ObjectKind::Ellipse(EllipseData {
            rx: 1.0,
            ry: 1.0,
            mode: "ellipse".into(),
            start_angle: 0.0,
            end_angle: 360.0,
            extra: Default::default(),
        }))
    }

    fn group() -> Object {
        let g: GroupData = serde_json::from_value(serde_json::json!({ "children": [] })).unwrap();
        Object::new(ObjectKind::Group(g))
    }

    /// GD-BPR-01 / D-31-09: chave `janela/ferramenta/seleção[/tarefa]`.
    #[test]
    fn context_key_names_window_tool_selection_and_task() {
        assert_eq!(
            context_key(MAIN, "select", "rect", None),
            "main/select/rect"
        );
        assert_eq!(
            context_key(MAIN, "select", "none", None),
            "main/select/none"
        );
        assert_eq!(
            context_key("preview", "imposition", "none", Some("grid")),
            "preview/imposition/none/grid"
        );
        assert_eq!(tool_key("tool-select"), "select");
        assert_eq!(tool_key("tool-text"), "text");
        assert_eq!(tool_key("zoom"), "zoom");
    }

    /// GD-BPR-01/02: o tipo da seleção decide o contexto; tipos
    /// diferentes = "vários objetos".
    #[test]
    fn selection_kind_is_common_type_or_multi() {
        let (r1, r2, e) = (rect(), rect(), ellipse());
        let g = group();
        assert_eq!(selection_kind(&[]), "none");
        assert_eq!(selection_kind(&[&r1]), "rect");
        assert_eq!(selection_kind(&[&r1, &r2]), "rect");
        assert_eq!(selection_kind(&[&r1, &e]), "multi");
        assert_eq!(selection_kind(&[&e]), "ellipse");
        assert_eq!(selection_kind(&[&g]), "group");
    }

    /// GD-BPR-02: valor que difere entre objetos fica vazio ("vários").
    #[test]
    fn mixed_value_is_empty_when_objects_differ() {
        assert_eq!(Mixed::of([2.0, 2.0]), Mixed::Same(2.0));
        assert_eq!(Mixed::of([2.0, 3.0]), Mixed::Various);
        assert_eq!(Mixed::<f64>::of([]), Mixed::None);
        assert_eq!(Mixed::of([2.0, 3.0]).value(), None);
        assert_eq!(Mixed::of([5.0]).value(), Some(&5.0));
    }

    fn g(id: &str, width: f32, essential: bool) -> GroupSpec {
        GroupSpec {
            id: id.into(),
            width,
            essential,
        }
    }

    /// GD-BPR-08: tudo cabe → nada vai para o ».
    #[test]
    fn fit_keeps_everything_when_it_fits() {
        let gs = [g("a", 100.0, true), g("b", 100.0, false)];
        assert_eq!(fit(&gs, 210.0, 10.0, 24.0), vec![true, true]);
    }

    /// GD-BPR-08: corta da direita para a esquerda, essenciais por último,
    /// reservando o lugar do ».
    #[test]
    fn fit_cuts_from_the_right_essentials_last() {
        let gs = [
            g("trf", 200.0, true),
            g("ctn", 100.0, false),
            g("org", 100.0, false),
            g("foc", 100.0, true),
        ];
        // Sai primeiro o não essencial mais à direita ("org"):
        // 200 + 100 + 100 + 2 × 10 + » (24 + 10) = 454.
        assert_eq!(fit(&gs, 460.0, 10.0, 24.0), vec![true, true, false, true]);
        assert_eq!(fit(&gs, 350.0, 10.0, 24.0), vec![true, false, false, true]);
        // Só cabe um: o essencial mais à esquerda.
        assert_eq!(fit(&gs, 240.0, 10.0, 24.0), vec![true, false, false, false]);
        // Nem ele cabe: fica mesmo assim (a barra nunca fica vazia).
        assert_eq!(fit(&gs, 50.0, 10.0, 24.0), vec![true, false, false, false]);
    }

    /// GD-BPR-07 (aceite): ocultar "Rotação" na Seleção some só ali;
    /// a barra do Retângulo fica igual.
    #[test]
    fn hiding_is_per_context() {
        let mut c = Customs::default();
        c.set_hidden("main/select/rect", "B-TRF/angle", true);
        assert!(c.is_hidden("main/select/rect", "B-TRF/angle"));
        assert!(!c.is_hidden("main/rect/none", "B-TRF/angle"));
        assert!(!c.is_hidden("main/select/ellipse", "B-TRF/angle"));
        c.set_hidden("main/select/rect", "B-TRF/angle", false);
        assert!(!c.is_hidden("main/select/rect", "B-TRF/angle"));
        assert_eq!(c, Customs::default(), "contexto sem nada some do arquivo");
    }

    /// GD-BPR-07: comando acrescentado aparece só no contexto, sem repetir;
    /// *Restaurar padrão* limpa só aquele contexto.
    #[test]
    fn added_commands_and_restore_are_per_context() {
        let mut c = Customs::default();
        c.add_command("main/select/rect", "export");
        c.add_command("main/select/rect", "export");
        c.add_command("main/select/rect", "print");
        c.set_hidden("main/text/none", "B-FON", true);
        assert_eq!(
            c.get("main/select/rect").unwrap().added,
            ["export", "print"]
        );
        assert!(c.get("main/rect/none").is_none());
        c.remove_command("main/select/rect", "export");
        assert_eq!(c.get("main/select/rect").unwrap().added, ["print"]);
        c.restore("main/select/rect");
        assert!(c.get("main/select/rect").is_none());
        assert!(c.is_hidden("main/text/none", "B-FON"));
    }

    /// GD-BPR-07 / D-31-10: reordenar grupos vale só no contexto; grupos
    /// novos entram na posição padrão.
    #[test]
    fn group_order_is_per_context() {
        let def: Vec<String> = ["trf", "ctn", "org"].map(String::from).to_vec();
        let mut c = Customs::default();
        assert_eq!(c.ordered("k", &def), def);
        c.move_group("k", &def, "org", 0);
        assert_eq!(c.ordered("k", &def), ["org", "trf", "ctn"]);
        assert_eq!(c.ordered("outro", &def), def);
        // Um grupo que aparece depois (ex.: setas com curva aberta) entra
        // logo depois do que o precede na ordem padrão.
        let more: Vec<String> = ["trf", "ctn", "set", "org"].map(String::from).to_vec();
        assert_eq!(c.ordered("k", &more), ["org", "trf", "ctn", "set"]);
        // Grupo que sumiu do contexto é ignorado.
        let less: Vec<String> = ["trf", "ctn"].map(String::from).to_vec();
        assert_eq!(c.ordered("k", &less), ["trf", "ctn"]);
    }

    /// Desenha uma barra de grupos com larguras fixas, numa tela de
    /// `width` px, por alguns quadros. Devolve o registro, a altura do
    /// painel, onde cada grupo começou e quais grupos rodaram.
    fn draw(
        width: f32,
        custom: &Customs,
        groups: &[(&str, f32, bool)],
    ) -> (Seen, f32, Vec<(String, f32)>, Vec<String>) {
        let ctx = crate::theme::test_ctx();
        let (mut h, mut xs, mut ran) = (0.0, Vec::new(), Vec::new());
        for _ in 0..4 {
            xs.clear();
            ran.clear();
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(width, 768.0),
                )),
                ..Default::default()
            };
            let mut out = ctx.run_ui(input, |ui| {
                let c = ui.ctx().clone();
                h = crate::widgets::property_panel(&c)
                    .show(ui, |ui| {
                        let mut bar = Bar::begin(ui, "k".into(), custom, Pass::Inline, false);
                        for (id, w, ess) in groups {
                            bar.group(ui, id, id, *ess, |ui, it| {
                                ran.push(id.to_string());
                                xs.push((id.to_string(), ui.max_rect().left()));
                                for item in ["x", "y"] {
                                    if it.show(item, item) {
                                        ui.allocate_exact_size(
                                            egui::vec2(*w / 2.0, 20.0),
                                            egui::Sense::hover(),
                                        );
                                    }
                                }
                            });
                        }
                        bar.end(ui, Some("dica"));
                    })
                    .response
                    .rect
                    .height();
            });
            out.textures_delta.clear();
        }
        (seen(&ctx), h, xs, ran)
    }

    const GROUPS: [(&str, f32, bool); 4] = [
        ("trf", 300.0, true),
        ("ctn", 200.0, false),
        ("org", 200.0, false),
        ("foc", 120.0, true),
    ];

    /// GD-BPR-08: tela larga, tudo na barra; tela estreita, grupos da
    /// direita vão para o », essenciais por último, e a altura não muda.
    #[test]
    fn narrow_bar_sends_groups_to_more_button() {
        let c = Customs::default();
        let (s, h, _, _) = draw(1366.0, &c, &GROUPS);
        assert!(s.overflow.is_empty(), "{:?}", s.overflow);
        assert_eq!(h, theme::BAR_PROPERTY_HEIGHT);
        let (s, h, _, ran) = draw(700.0, &c, &GROUPS);
        assert_eq!(s.overflow, ["ctn", "org"]);
        assert_eq!(ran, ["trf", "foc"], "os do » não são desenhados na linha");
        assert_eq!(h, theme::BAR_PROPERTY_HEIGHT, "nunca quebra em duas linhas");
    }

    /// GD-BPR-07: grupo oculto não roda; item oculto não aparece; os dois
    /// continuam listados para o menu ⋯.
    #[test]
    fn hidden_group_and_item_are_skipped_but_listed() {
        let mut c = Customs::default();
        c.set_hidden("k", "org", true);
        c.set_hidden("k", "trf/y", true);
        let (s, _, _, ran) = draw(1366.0, &c, &GROUPS);
        assert!(!ran.contains(&"org".to_string()));
        assert!(s.groups.iter().any(|g| g.id == "org"), "segue no menu");
        let trf = s.groups.iter().find(|g| g.id == "trf").unwrap();
        assert_eq!(trf.items.len(), 2, "o item oculto segue no menu");
        assert!(
            (trf.width - 150.0).abs() < 1.0,
            "só o item x: {}",
            trf.width
        );
    }

    /// GD-BPR-07 / D-31-10: a ordem do usuário muda a posição dos grupos.
    #[test]
    fn custom_order_moves_groups() {
        let mut c = Customs::default();
        let def: Vec<String> = GROUPS.iter().map(|g| g.0.to_string()).collect();
        c.move_group("k", &def, "foc", 0);
        let (_, _, xs, _) = draw(1366.0, &c, &GROUPS);
        let x = |id: &str| xs.iter().find(|p| p.0 == id).unwrap().1;
        assert!(x("foc") < x("trf"), "{xs:?}");
        assert!(x("trf") < x("ctn"));
    }

    /// GD-BPR-07: os pedidos do menu ⋯ mudam só o contexto da barra.
    #[test]
    fn menu_actions_apply_to_context() {
        let mut c = Customs::default();
        let cur: Vec<String> = ["a", "b"].map(String::from).to_vec();
        apply(
            &mut c,
            "k",
            &[
                Action::Hide {
                    id: "a/angle".into(),
                    hidden: true,
                },
                Action::Add("export".into()),
                Action::Move {
                    id: "b".into(),
                    to: 0,
                },
                Action::Run(Command::Undo),
            ],
            &cur,
        );
        assert!(c.is_hidden("k", "a/angle"));
        assert_eq!(c.get("k").unwrap().added, ["export"]);
        assert_eq!(c.ordered("k", &cur), ["b", "a"]);
        apply(&mut c, "k", &[Action::Restore], &cur);
        assert_eq!(c, Customs::default());
    }

    /// D-31-07: a personalização vai para o arquivo da área de trabalho.
    #[test]
    fn customs_round_trip_json() {
        let mut c = Customs::default();
        c.set_hidden("main/select/rect", "B-TRF/angle", true);
        c.add_command("main/select/rect", "export");
        let j = serde_json::to_string(&c).unwrap();
        assert_eq!(serde_json::from_str::<Customs>(&j).unwrap(), c);
    }
}
