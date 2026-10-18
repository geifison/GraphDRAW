//! Barra de propriedades contextual (GD-UI-01) e painéis: Transformação,
//! Alinhar, Camadas, Histórico, Páginas e Relatório de importação (GD-UI-02).

use eframe::egui::{self, Vec2};
use graphdraw_core::edit::nodes::{self, NodeRef};
use graphdraw_core::edit::ops::{self, Align, AlignTo, Anchor, Order};
use graphdraw_core::geometry::SegRef;
use graphdraw_core::model::{Corner, Fill, NodeKind, ObjectKind};
use graphdraw_core::report::Severity;
use kurbo::{Point, Vec2 as KVec2};

use crate::app::{App, RightTab};
use crate::canvas::{Tool, single_path};
use crate::doc::DocState;
use crate::i18n::tr;
use crate::icons;
use crate::theme;
use crate::widgets::{self, fmt_mm, labeled_length, length_field, number_field, swatch};

/// Célula do seletor de âncora e o vão entre células.
const ANCHOR_CELL: f32 = 7.0;
const ANCHOR_GAP: f32 = 1.0;

/// Seletor do ponto de referência (9 âncoras, GD-TRF-01).
/// As 9 células são desenhadas num quadrado só (3 × 7 px + vãos = 23 px):
/// uma grade do egui daria a cada linha a altura mínima de controle (24 px)
/// e o seletor passaria da altura da barra de propriedades.
pub fn anchor_picker(ui: &mut egui::Ui, anchor: &mut Anchor) {
    let step = ANCHOR_CELL + ANCHOR_GAP;
    let side = 3.0 * ANCHOR_CELL + 2.0 * ANCHOR_GAP;
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(side), egui::Sense::click());
    let cell_at = |p: egui::Pos2| {
        let d = p - rect.min;
        let (col, row) = ((d.x / step) as usize, (d.y / step) as usize);
        (rect.contains(p) && col < 3 && row < 3).then_some(row * 3 + col)
    };
    let hovered = resp.hover_pos().and_then(cell_at);
    let pal = theme::palette(ui);
    for (i, a) in Anchor::ALL.into_iter().enumerate() {
        let min = rect.min + Vec2::new((i % 3) as f32 * step, (i / 3) as f32 * step);
        let c = if *anchor == a {
            theme::ACCENT_PRIMARY
        } else if hovered == Some(i) {
            pal.fg_muted
        } else {
            pal.border_default
        };
        ui.painter().rect_filled(
            egui::Rect::from_min_size(min, Vec2::splat(ANCHOR_CELL)),
            theme::RADIUS_XS,
            c,
        );
    }
    let resp = resp.on_hover_text(tr("Ponto de referência", "Reference point"));
    if resp.clicked()
        && let Some(i) = resp.interact_pointer_pos().and_then(cell_at)
    {
        *anchor = Anchor::ALL[i];
    }
}

impl App {
    /// Barra de propriedades: muda conforme a ferramenta e a seleção.
    pub fn property_bar(&mut self, ui: &mut egui::Ui) {
        let text_targets = self.text_targets();
        let text_at = self.text_style_index();
        let picker = &mut self.font_picker;
        let defaults = &mut self.defaults;
        let Some(doc) = self.doc.as_mut() else {
            ui.label(tr("Nenhum documento aberto", "No document open"));
            return;
        };
        let mut refit = false;
        let mut pending_style = None;
        // Altura fixa e uma linha só (DESIGN-SYSTEM 4.6): dicas longas são
        // cortadas; controles que não cabem rolam na horizontal.
        let hint = |ui: &mut egui::Ui, t: &str| widgets::hint_line(ui, t);
        match self.tool {
            Tool::Shape => widgets::single_line_bar(ui, "barra-forma", |ui| shape_bar(ui, doc)),
            Tool::SolidFill => hint(
                ui,
                tr(
                    "Clique num objeto para escolher a cor sólida do preenchimento",
                    "Click an object to choose its solid fill color",
                ),
            ),
            Tool::Freehand | Tool::Polyline => {
                widgets::single_line_bar(ui, "barra-mao-livre", |ui| {
                    ui.label(tr("Suavização", "Smoothing"));
                    ui.add(
                        egui::Slider::new(&mut self.canvas.freehand_smooth, 0.0..=100.0)
                            .show_value(true),
                    );
                    if self.tool == Tool::Freehand {
                        ui.separator();
                        ui.checkbox(&mut self.canvas.sketch, tr("Esboço contínuo", "Continuous sketch"))
                            .on_hover_text(tr(
                                "Traços seguidos que começam perto do fim do anterior viram uma curva só",
                                "Successive strokes starting near the previous end become a single curve",
                            ));
                        ui.add_enabled_ui(self.canvas.sketch, |ui| {
                            ui.add(
                                egui::DragValue::new(&mut self.canvas.sketch_time)
                                    .range(0.2..=10.0)
                                    .speed(0.1)
                                    .suffix(" s"),
                            )
                            .on_hover_text(tr("Tempo para emendar", "Time to join"));
                            ui.add(
                                egui::DragValue::new(&mut self.canvas.sketch_dist)
                                    .range(2.0..=60.0)
                                    .suffix(" px"),
                            )
                            .on_hover_text(tr("Distância para emendar", "Distance to join"));
                        });
                    }
                    ui.separator();
                    widgets::hint_line(
                        ui,
                        if self.tool == Tool::Polyline {
                            tr(
                                "Cliques fazem retas, arrastar faz trecho livre · Enter ou duplo clique termina · clique no primeiro nó fecha",
                                "Clicks make lines, dragging a free stretch · Enter or double-click ends · click the first node to close",
                            )
                        } else {
                            tr(
                                "Arraste para desenhar; volte ao início para fechar",
                                "Drag to draw; return to the start to close",
                            )
                        },
                    );
                })
            }
            Tool::Width => widgets::single_line_bar(ui, "barra-largura", |ui| {
                let sel = doc.selection.clone();
                let cur = sel
                    .first()
                    .and_then(|id| doc.editor.find(id))
                    .and_then(|r| doc.editor.get(&r))
                    .and_then(|o| o.stroke.as_ref())
                    .map(|s| s.widths.clone());
                let mut pick: Option<Vec<graphdraw_core::model::WidthNode>> = None;
                ui.add_enabled_ui(cur.is_some(), |ui| {
                    if ui
                        .selectable_label(
                            cur.as_ref().is_some_and(|w| w.is_empty()),
                            tr("Uniforme", "Uniform"),
                        )
                        .clicked()
                    {
                        pick = Some(Vec::new());
                    }
                    for name in graphdraw_core::render::varwidth::PRESETS {
                        let nodes = graphdraw_core::render::varwidth::preset(name);
                        if ui
                            .selectable_label(
                                cur.as_ref() == Some(&nodes),
                                crate::dialogs::width_label(name),
                            )
                            .clicked()
                        {
                            pick = Some(nodes);
                        }
                    }
                });
                if let Some(nodes) = pick {
                    ops::modify(
                        &mut doc.editor,
                        &sel,
                        tr("Largura variável", "Variable width"),
                        |o| {
                            if let Some(s) = &mut o.stroke {
                                s.widths = nodes.clone();
                                return true;
                            }
                            false
                        },
                    );
                }
                ui.separator();
                widgets::hint_line(
                    ui,
                    tr(
                        "Arraste no contorno: longe da linha engrossa, perto afina · os nós ficam marcados",
                        "Drag on the outline: away from the line thickens, close thins · nodes are marked",
                    ),
                );
            }),
            Tool::BSpline => widgets::single_line_bar(ui, "barra-bspline", |ui| {
                widgets::hint_line(
                    ui,
                    tr(
                        "Cliques põem pontos de controle · Ctrl+clique = fixado (a curva passa por ele) · Enter ou duplo clique termina · clique no primeiro fecha · na ferramenta Formato, arraste os pontos e duplo clique fixa/solta",
                        "Clicks place control points · Ctrl+click = fixed (the curve goes through it) · Enter or double-click ends · click the first to close · with the Shape tool, drag the points and double-click to fix/release",
                    ),
                );
            }),
            Tool::Connector => widgets::single_line_bar(ui, "barra-conector", |ui| {
                connector_bar(ui, doc, &mut self.canvas)
            }),
            Tool::SmartFill => widgets::single_line_bar(ui, "barra-preench-intel", |ui| {
                ui.checkbox(
                    &mut self.canvas.smart_outline,
                    tr("Com contorno", "With outline"),
                );
                ui.separator();
                widgets::hint_line(
                    ui,
                    tr(
                        "Clique numa área cercada; o objeto novo usa o preenchimento padrão",
                        "Click an enclosed area; the new object uses the default fill",
                    ),
                );
            }),
            Tool::Dimension => widgets::single_line_bar(ui, "barra-cota", |ui| {
                dimension_bar(ui, doc, &mut self.canvas)
            }),
            Tool::FreeTransform => widgets::single_line_bar(ui, "barra-transf-livre", |ui| {
                use graphdraw_core::edit::freexf::Mode;
                for (m, icon, label) in [
                    (Mode::Rotate, "rotate-ccw", tr("Girar", "Rotate")),
                    (Mode::Reflect, "mirror-h", tr("Refletir", "Reflect")),
                    (Mode::Scale, "fit-selection", tr("Escalar", "Scale")),
                    (Mode::Skew, "clear-transform", tr("Inclinar", "Skew")),
                ] {
                    if icons::button(ui, icon, label, self.canvas.free_mode == m, true).clicked() {
                        self.canvas.free_mode = m;
                    }
                }
                ui.separator();
                widgets::hint_line(
                    ui,
                    tr(
                        "Arraste a partir do pivô · Shift: 15° (girar/refletir) ou x e y separados (escalar) · Esc cancela",
                        "Drag from the pivot · Shift: 15° (rotate/reflect) or separate x and y (scale) · Esc cancels",
                    ),
                );
            }),
            Tool::Line2 => widgets::single_line_bar(ui, "barra-linha2", |ui| {
                use crate::canvas::LineMode;
                for (m, label, tip) in [
                    (
                        LineMode::Free,
                        tr("Livre", "Free"),
                        tr("Shift prende a 15°", "Shift locks to 15°"),
                    ),
                    (
                        LineMode::Tangent,
                        tr("Tangente", "Tangent"),
                        tr(
                            "O fim encosta tangente no objeto sob o cursor",
                            "The end touches the object under the cursor tangentially",
                        ),
                    ),
                    (
                        LineMode::Perpendicular,
                        tr("Perpendicular", "Perpendicular"),
                        tr(
                            "Começando num contorno, sai perpendicular a ele",
                            "Starting on an outline, leaves perpendicular to it",
                        ),
                    ),
                ] {
                    if ui
                        .selectable_label(self.canvas.line_mode == m, label)
                        .on_hover_text(tip)
                        .clicked()
                    {
                        self.canvas.line_mode = m;
                    }
                }
                ui.separator();
                widgets::hint_line(
                    ui,
                    tr(
                        "Arraste de um ponto ao outro",
                        "Drag from one point to the other",
                    ),
                );
            }),
            Tool::Curve3 => hint(
                ui,
                tr(
                    "Arraste do início ao fim; depois clique onde a curva deve passar (Esc cancela)",
                    "Drag from start to end; then click where the curve should pass (Esc cancels)",
                ),
            ),
            Tool::Knife => widgets::single_line_bar(ui, "barra-faca", |ui| {
                ui.label(tr("Lacuna", "Gap"));
                if let Some(v) =
                    widgets::length_field(ui, "faca-lacuna", self.canvas.knife_gap, None)
                {
                    self.canvas.knife_gap = v.max(0.0);
                }
                ui.separator();
                widgets::hint_line(
                    ui,
                    tr(
                        "Arraste através dos objetos (com seleção, só ela) · Alt: à mão livre",
                        "Drag across objects (with a selection, only it) · Alt: freehand",
                    ),
                );
            }),
            Tool::Eraser => widgets::single_line_bar(ui, "barra-borracha", |ui| {
                ui.label(tr("Largura", "Width"));
                if let Some(v) =
                    widgets::length_field(ui, "borracha-largura", self.canvas.eraser_width, None)
                {
                    self.canvas.eraser_width = v.max(100.0);
                }
                ui.separator();
                widgets::hint_line(
                    ui,
                    tr(
                        "Arraste para apagar (com seleção, só nela)",
                        "Drag to erase (with a selection, only in it)",
                    ),
                );
            }),
            Tool::Crop => widgets::single_line_bar(ui, "barra-recorte", |ui| {
                let pending = self.canvas.crop_pending.is_some();
                if ui
                    .add_enabled(pending, egui::Button::new(tr("Recortar", "Crop")))
                    .clicked()
                {
                    self.canvas.apply_crop(doc);
                }
                if ui
                    .add_enabled(pending, egui::Button::new(tr("Cancelar", "Cancel")))
                    .clicked()
                {
                    self.canvas.crop_pending = None;
                }
                ui.separator();
                widgets::hint_line(
                    ui,
                    tr(
                        "Arraste o retângulo; Enter ou duplo clique apaga o que está fora",
                        "Drag the rectangle; Enter or double-click removes what is outside",
                    ),
                );
            }),
            Tool::VirtualDelete => hint(
                ui,
                tr(
                    "Clique no trecho entre cruzamentos para apagá-lo; sem cruzamento, sai o segmento",
                    "Click the stretch between crossings to delete it; without crossings, the segment goes",
                ),
            ),
            Tool::Eyedropper => widgets::single_line_bar(ui, "barra-conta-gotas", |ui| {
                eyedrop_bar(ui, &mut self.canvas.eyedrop)
            }),
            Tool::Gradient => {
                widgets::single_line_bar(ui, "barra-gradiente", |ui| gradient_bar(ui, doc))
            }
            Tool::Bezier => {
                hint(
                    ui,
                    tr(
                        "Clique = nó de canto · clique e arraste = nó suave · clique no primeiro nó fecha · Enter/duplo clique termina · Backspace desfaz o último nó · Esc cancela",
                        "Click = corner node · click-drag = smooth node · click first node to close · Enter/double-click finishes · Backspace removes last · Esc cancels",
                    ),
                );
            }
            Tool::Rect | Tool::Ellipse => {
                // Modo de cada ferramenta (o mesmo da gaveta da caixa).
                let tool = self.tool;
                let mut on = self.defaults.three_point(tool);
                let three = &mut on;
                widgets::single_line_bar(ui, "barra-forma-basica", |ui| {
                    // Modo de desenho (GD-FER-30/31): 2 pontos (caixa) ou 3 pontos
                    // (base em qualquer ângulo + altura).
                    if ui
                        .selectable_label(!*three, tr("2 pontos", "2 points"))
                        .clicked()
                    {
                        *three = false;
                    }
                    if ui
                        .selectable_label(*three, tr("3 pontos", "3 points"))
                        .clicked()
                    {
                        *three = true;
                    }
                    ui.separator();
                    widgets::hint_line(
                        ui,
                        if *three {
                            tr(
                                "Arraste a base · mova para a altura e clique · Shift = ângulo em 15° · Esc cancela",
                                "Drag the base · move for height and click · Shift = 15° steps · Esc cancels",
                            )
                        } else {
                            tr(
                                "Arraste para desenhar · Shift = proporcional · Ctrl = a partir do centro",
                                "Drag to draw · Shift = proportional · Ctrl = from center",
                            )
                        },
                    );
                });
                if on != self.defaults.three_point(tool) {
                    self.defaults.set_three_point(tool, on);
                    self.canvas.three_point = on;
                    self.canvas.drop_three();
                }
            }
            Tool::Zoom => {
                hint(
                    ui,
                    tr(
                        "Clique aproxima · botão direito afasta · arraste um retângulo para enquadrar",
                        "Click zooms in · right-click zooms out · drag a rectangle to frame it",
                    ),
                );
            }
            Tool::Pan => {
                hint(
                    ui,
                    tr(
                        "Arraste para mover a vista (espaço ativa temporariamente)",
                        "Drag to pan (hold space for temporary hand)",
                    ),
                );
            }
            // Prancheta: dados da página; arrastar as alças muda o tamanho
            // (GD-DIM-23). Vale para todas as páginas (GD-PAG-01).
            Tool::Polygon => {
                widgets::single_line_bar(ui, "barra-poligono", |ui| {
                    // A família (polígono/estrelas, espiral, papel gráfico) é
                    // escolhida na gaveta da caixa de ferramentas (DS 4.8).
                    match defaults.shape_family.as_str() {
                        "spiral" => spiral_controls(ui, defaults),
                        "basic" => basic_controls(ui, defaults),
                        "graph" => graph_controls(ui, defaults),
                        _ => {
                            if let Some((n, m, k)) = polygon_controls(
                                ui,
                                defaults.polygon_sides,
                                &defaults.polygon_mode,
                                defaults.polygon_sharpness,
                            ) {
                                defaults.polygon_sides = n;
                                defaults.polygon_mode = m;
                                defaults.polygon_sharpness = k;
                            }
                        }
                    }
                    ui.separator();
                    widgets::hint_line(
                        ui,
                        tr(
                            "Arraste para desenhar · Shift = proporcional · Ctrl = a partir do centro",
                            "Drag to draw · Shift = proportional · Ctrl = from center",
                        ),
                    );
                });
            }
            Tool::Text => {
                widgets::single_line_bar(ui, "barra-texto", |ui| {
                    if let Some(st) = text_bar(ui, doc, &text_targets, text_at, picker) {
                        pending_style = Some(st);
                    }
                    path_text_bar(ui, doc, &text_targets);
                    hyphen_bar(ui, doc, &text_targets);
                });
            }
            Tool::Artboard => {
                widgets::single_line_bar(ui, "barra-prancheta", |ui| {
                    refit = page_bar_props(ui, doc);
                    widgets::hint_line(
                        ui,
                        tr(
                            "Arraste as alças da página · gruda nos formatos ISO · Alt solta o encaixe",
                            "Drag the page handles · snaps to ISO formats · Alt disables snapping",
                        ),
                    );
                });
            }
            Tool::Select => {
                let texts = &text_targets;
                widgets::single_line_bar(ui, "barra-selecao", |ui| {
                    // Só textos selecionados: os controles de texto entram como
                    // parâmetros do objeto (grupo 2 da ordem fixa, DS 4.6).
                    let only_texts = !texts.is_empty() && texts.len() == doc.selection.len();
                    let mut text_params = |ui: &mut egui::Ui, doc: &mut DocState| {
                        if only_texts {
                            ui.separator();
                            if let Some(st) = text_bar(ui, doc, texts, None, picker) {
                                pending_style = Some(st);
                            }
                            path_text_bar(ui, doc, texts);
                            hyphen_bar(ui, doc, texts);
                        }
                    };
                    refit = select_bar(
                        ui,
                        doc,
                        &mut self.anchor,
                        &mut self.lock_ratio,
                        &mut self.corners_sel,
                        &mut text_params,
                    );
                    symmetry_bar(ui, doc);
                    wrap_bar(ui, doc);
                    live_boolean_bar(ui, doc);
                });
            }
        }
        if refit {
            self.canvas.fit_pending = true;
        }
        if let Some(st) = pending_style {
            self.text_style(st);
        }
    }

    pub fn right_panel(&mut self, ui: &mut egui::Ui) {
        // Painéis laterais em densidade Default (28 px).
        theme::panel_density(ui);
        ui.horizontal(|ui| {
            // Recolher e soltar a aba ativa ficam à direita das abas.
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                if icons::button(
                    ui,
                    "panel-collapse",
                    tr(
                        "Recolher painéis (duplo clique na aba)",
                        "Collapse panels (double-click a tab)",
                    ),
                    false,
                    true,
                )
                .clicked()
                {
                    self.ws.collapsed = true;
                }
                if icons::button(
                    ui,
                    "panel-undock",
                    tr(
                        "Soltar este painel numa janela",
                        "Float this panel in a window",
                    ),
                    false,
                    true,
                )
                .clicked()
                {
                    self.float_tab(self.right_tab, ui.ctx());
                }
                ui.with_layout(
                    egui::Layout::left_to_right(egui::Align::Min).with_main_wrap(true),
                    |ui| {
                        for tab in self.ws.docked() {
                            let r = ui.selectable_label(self.right_tab == tab, tab.label());
                            if r.double_clicked() {
                                self.ws.collapsed = true;
                            } else if r.clicked() {
                                self.right_tab = tab;
                            }
                            r.context_menu(|ui| {
                                if ui
                                    .button(tr("Soltar numa janela", "Float in a window"))
                                    .clicked()
                                {
                                    self.float_tab(tab, ui.ctx());
                                    ui.close();
                                }
                                if ui
                                    .button(tr("Recolher painéis", "Collapse panels"))
                                    .clicked()
                                {
                                    self.ws.collapsed = true;
                                    ui.close();
                                }
                            });
                        }
                    },
                );
            });
        });
        ui.separator();
        let tab = self.right_tab;
        egui::ScrollArea::vertical().show(ui, |ui| self.tab_content(ui, tab));
    }

    /// Faixa de ícones da coluna recolhida (DESIGN-SYSTEM 6).
    pub fn panel_rail(&mut self, ui: &mut egui::Ui, docked: &[RightTab]) {
        ui.vertical_centered(|ui| {
            if icons::button(
                ui,
                "panel-expand",
                tr("Abrir painéis", "Expand panels"),
                false,
                true,
            )
            .clicked()
            {
                self.ws.collapsed = false;
            }
            ui.separator();
            for &tab in docked {
                if icons::button(ui, tab.icon(), tab.label(), false, true).clicked() {
                    self.right_tab = tab;
                    self.ws.collapsed = false;
                }
            }
        });
    }

    /// Solta a aba numa janela própria, perto da coluna.
    fn float_tab(&mut self, tab: RightTab, ctx: &egui::Context) {
        if self.ws.is_floating(tab) {
            return;
        }
        let screen = ctx.content_rect();
        let n = self.ws.floating.len() as f32;
        let [w, h] = theme::PANEL_FLOAT_SIZE;
        let x = (screen.right() - self.ws.width - w - theme::SPACE_600).max(screen.left());
        let y = screen.top() + theme::SPACE_600 * (3.0 + n);
        self.ws.floating.push(crate::workspace::Floating {
            tab: tab.key().into(),
            pos: [x, y],
            size: [w, h],
        });
    }

    /// Painéis soltos em janelas; fechar a janela devolve o painel à coluna.
    pub fn floating_panels(&mut self, ctx: &egui::Context) {
        let list = self.ws.floating.clone();
        for f in list {
            let Some(tab) = RightTab::from_key(&f.tab) else {
                continue;
            };
            let mut open = true;
            let mut inner = None;
            let shown = egui::Window::new(tab.label())
                .id(egui::Id::unique(("painel-solto", tab.key())))
                .default_pos(f.pos)
                .default_size(f.size)
                .collapsible(false)
                .open(&mut open)
                .show(ctx, |ui| {
                    theme::panel_density(ui);
                    inner = Some(ui.available_size());
                    egui::ScrollArea::vertical().show(ui, |ui| self.tab_content(ui, tab));
                });
            if let Some(slot) = self.ws.floating.iter_mut().find(|x| x.tab == f.tab) {
                if let Some(r) = &shown {
                    slot.pos = [
                        r.response.rect.left().round(),
                        r.response.rect.top().round(),
                    ];
                }
                if let Some(sz) = inner {
                    slot.size = [sz.x.round(), sz.y.round()];
                }
            }
            if !open {
                self.ws.floating.retain(|x| x.tab != f.tab);
                self.show_right = true;
                self.ws.collapsed = false;
                self.right_tab = tab;
            }
        }
    }

    /// Grava a área de trabalho quando muda (depois de soltar o mouse, para
    /// não gravar a cada quadro de um arrasto).
    pub fn save_workspace(&mut self, ctx: &egui::Context) {
        self.ws.panels = self.show_right;
        self.ws.tab = self.right_tab.key().into();
        self.ws.tool = self.tool.info().0.into();
        self.ws.density = theme::density(ctx).key().into();
        if self.ws != self.ws_saved && !ctx.input(|i| i.pointer.any_down()) {
            if !cfg!(test) {
                let _ = self.ws.save();
            }
            self.ws_saved = self.ws.clone();
        }
    }

    /// Conteúdo de uma aba (na coluna ou numa janela solta).
    pub(crate) fn tab_content(&mut self, ui: &mut egui::Ui, tab: RightTab) {
        let mut pal_act = None;
        let Some(doc) = self.doc.as_mut() else { return };
        match tab {
            RightTab::Transform => transform_panel(ui, doc, &mut self.xf),
            RightTab::Align => align_panel(ui, doc, &mut self.align),
            RightTab::Layers => {
                layers_panel(ui, doc, &mut self.layer_tree);
                look_section(ui, doc);
                user_data_section(ui, doc);
            }
            RightTab::History => history_panel(ui, doc),
            RightTab::Pages => {
                if pages_panel(ui, doc) {
                    self.canvas.fit_pending = true;
                }
            }
            RightTab::Report => report_panel(ui, doc),
            RightTab::Preflight => preflight_panel(ui, doc),
            RightTab::Palettes => pal_act = self.palettes.panel(ui, doc),
        }
        if let Some(s) = self.palettes.status.take() {
            self.status = s;
        }
        if let Some(a) = pal_act {
            self.apply_pal(a);
        }
    }

    /// Ação de amostra (clique, botão direito, Ctrl + clique).
    pub(crate) fn apply_pal(&mut self, a: crate::paletteui::PalAction) {
        use crate::paletteui::PalAction;
        // Texto em edição com trecho selecionado: a cor vai só para ele.
        if let PalAction::Fill(c) = &a
            && let Some((id, x, y)) = self.text_range()
            && let Some(d) = &mut self.doc
        {
            let fill = c.clone().map(Fill::solid);
            ops::modify(
                &mut d.editor,
                &[id],
                tr("Cor do trecho", "Range color"),
                |o| match &mut o.kind {
                    ObjectKind::Text(t) => {
                        graphdraw_core::edit::text::fill_range(t, x, y, fill.clone());
                        true
                    }
                    _ => false,
                },
            );
            return;
        }
        // Preenchimento interativo: a cor vai para a parada escolhida.
        if let PalAction::Fill(Some(c)) = &a
            && self.tool == Tool::Gradient
            && let Some(d) = &mut self.doc
            && self.canvas.set_grad_stop_color(d, c.clone())
        {
            return;
        }
        match a {
            PalAction::Fill(c) => self.apply_fill(c.map(Fill::solid)),
            PalAction::FillWith(f) => self.apply_fill(Some(f)),
            PalAction::Stroke(c) => self.apply_stroke_color(c),
            PalAction::Tint(c) => {
                let current = self
                    .doc
                    .as_ref()
                    .and_then(|d| d.selected_objects().first().and_then(|o| o.fill.clone()))
                    .or_else(|| self.defaults.fill.clone());
                let base = match current {
                    Some(Fill::Solid { color, .. }) => color,
                    _ => graphdraw_core::model::Color::cmyk(0.0, 0.0, 0.0, 0.0),
                };
                self.apply_fill(Some(Fill::solid(crate::paletteui::tint(&base, &c, 0.1))));
            }
        }
    }

    /// Barra de paleta (GD-PAL-05): clique = preenchimento, botão direito = contorno.
    pub fn palette_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            // Qual paleta a barra mostra (GD-PAL-05).
            let sources = self.palettes.sources();
            let cur = sources.get(self.palettes.bar_source).cloned().unwrap_or_default();
            egui::ComboBox::from_id_salt("paleta-barra")
                .selected_text(cur)
                .width(theme::PALETTE_SOURCE_WIDTH)
                .show_ui(ui, |ui| {
                    for (k, s) in sources.iter().enumerate() {
                        ui.selectable_value(&mut self.palettes.bar_source, k, s);
                    }
                });
            ui.spacing_mut().item_spacing.x = theme::SPACE_050;
            let none = swatch(ui, None, theme::SWATCH).on_hover_text(tr(
                "Sem cor (clique: sem preenchimento · direito: sem contorno)",
                "No color (click: no fill · right: no outline)",
            ));
            if none.clicked() {
                self.apply_fill(None);
            }
            if none.secondary_clicked() {
                self.apply_stroke_color(None);
            }
            let mut act = None;
            for (name, c) in self.palettes.bar_colors(self.doc.as_ref()) {
                let tip = if name.is_empty() {
                    tr(
                        "Clique: preenchimento · botão direito: contorno · Ctrl + clique: mistura · arraste até um objeto",
                        "Click: fill · right-click: outline · Ctrl + click: mix · drag onto an object",
                    )
                    .to_string()
                } else {
                    name
                };
                let r = crate::paletteui::swatch_drag(ui, &c, theme::SWATCH).on_hover_text(tip);
                if let Some(a) = crate::paletteui::swatch_action(ui, &r, c) {
                    act = Some(a);
                }
            }
            if let Some(a) = crate::paletteui::show_mosaic(ui.ctx()) {
                act = Some(a);
            }
            if let Some(a) = act {
                self.apply_pal(a);
            }
        });
    }

    fn apply_fill(&mut self, fill: Option<Fill>) {
        match self.doc.as_mut() {
            Some(doc) if !doc.selection.is_empty() => ops::set_fill(
                &mut doc.editor,
                &doc.selection.clone(),
                fill.or(Some(Fill::None)),
            ),
            // GD-BDR-10: sem seleção, pergunta se vira padrão.
            _ if !self.ws.default_color_no_ask => {
                self.ask_default = Some(crate::app::DefaultAsk::Fill(fill));
            }
            _ => self.defaults.fill = fill,
        }
    }

    fn apply_stroke_color(&mut self, color: Option<graphdraw_core::model::Color>) {
        match self.doc.as_mut() {
            Some(doc) if !doc.selection.is_empty() => {
                let sel = doc.selection.clone();
                match color {
                    Some(c) => ops::edit_stroke(&mut doc.editor, &sel, |s| s.color = c.clone()),
                    None => ops::set_stroke(&mut doc.editor, &sel, None),
                }
            }
            _ if !self.ws.default_color_no_ask => {
                self.ask_default = Some(crate::app::DefaultAsk::Stroke(color));
            }
            _ => {
                self.defaults.stroke = color.map(|c| {
                    let mut s = self
                        .defaults
                        .stroke
                        .clone()
                        .unwrap_or_else(crate::doc::default_stroke);
                    s.color = c;
                    s
                })
            }
        }
    }
}

/// Sem nada selecionado, a barra mostra as propriedades da página
/// (GD-UI-01): formato, medidas, orientação, sangria e ajustar à arte. Tudo
/// vale para todas as páginas do documento (GD-PAG-01). Devolve `true` se o
/// tamanho mudou (a vista é reajustada).
/// Valor ao ligar a sangria (o usuário pode mudar no campo).
const DEFAULT_BLEED: f64 = 3_000.0;

fn page_bar_props(ui: &mut egui::Ui, doc: &mut DocState) -> bool {
    use graphdraw_core::edit::{pages, pagesize};
    let (w, h) = doc.page_size();
    let bleed = doc.page().bleed.unwrap_or(doc.doc().data.settings.bleed);
    let mut new_size: Option<(f64, f64)> = None;
    let mut new_bleed: Option<f64> = None;
    let mut changed = false;

    ui.label(egui::RichText::new(tr("Página", "Page")).text_style(theme::section_style()));
    // Formato: o nome do formato conhecido, se as medidas baterem.
    let current = pagesize::snap_format(w, h, 500.0)
        .map(|(p, ..)| p.name)
        .or_else(|| {
            pagesize::PRESETS
                .iter()
                .find(|p| {
                    ((p.width - w).abs() < 500.0 && (p.height - h).abs() < 500.0)
                        || ((p.width - h).abs() < 500.0 && (p.height - w).abs() < 500.0)
                })
                .map(|p| p.name)
        })
        .unwrap_or(tr("Personalizado", "Custom"));
    egui::ComboBox::from_id_salt("formato-pagina")
        .selected_text(current)
        .width(140.0)
        .show_ui(ui, |ui| {
            for (cat, title) in [
                (pagesize::Category::Print, tr("Impressão", "Print")),
                (pagesize::Category::Screen, tr("Telas", "Screens")),
                (pagesize::Category::Social, tr("Redes sociais", "Social")),
            ] {
                ui.label(egui::RichText::new(title).weak());
                for p in pagesize::PRESETS.iter().filter(|p| p.category == cat) {
                    let dims = if p.pixels {
                        format!(
                            "{:.0} × {:.0} px",
                            p.width / (25_400.0 / 96.0),
                            p.height / (25_400.0 / 96.0)
                        )
                    } else {
                        format!("{} × {} mm", fmt_mm(p.width), fmt_mm(p.height))
                    };
                    if ui
                        .selectable_label(current == p.name, format!("{}   {dims}", p.name))
                        .clicked()
                    {
                        // Mantém a orientação atual.
                        let land = w > h && p.width != p.height;
                        new_size = Some(if land == (p.width > p.height) {
                            (p.width, p.height)
                        } else {
                            (p.height, p.width)
                        });
                    }
                }
            }
        })
        .response
        .on_hover_text(tr("Formato das páginas", "Page format"));
    if let Some(v) = labeled_length(ui, tr("L", "W"), "pag-l", w, None) {
        new_size = Some((v.max(1000.0), h));
    }
    if let Some(v) = labeled_length(ui, tr("A", "H"), "pag-a", h, None) {
        new_size = Some((w, v.max(1000.0)));
    }
    let portrait = h >= w;
    if icons::button(
        ui,
        "page-portrait",
        tr("Retrato", "Portrait"),
        portrait,
        true,
    )
    .clicked()
        && !portrait
    {
        new_size = Some((h, w));
    }
    if icons::button(
        ui,
        "page-landscape",
        tr("Paisagem", "Landscape"),
        !portrait,
        true,
    )
    .clicked()
        && portrait
    {
        new_size = Some((h, w));
    }
    ui.separator();
    // Sangria opcional: o botão liga/desliga; o campo só aparece ligada.
    let on = bleed > 0.0;
    if icons::button(
        ui,
        "bleed",
        if on {
            tr(
                "Sangria ligada (clique para desligar)",
                "Bleed on (click to turn off)",
            )
        } else {
            tr(
                "Sem sangria (clique para ligar)",
                "No bleed (click to turn on)",
            )
        },
        on,
        true,
    )
    .clicked()
    {
        new_bleed = Some(if on { 0.0 } else { DEFAULT_BLEED });
    }
    if on && let Some(v) = length_field(ui, "pag-sangria", bleed, None) {
        new_bleed = Some(v.max(0.0));
    }
    ui.separator();
    let has_art = pagesize::art_bounds(&doc.editor, doc.page).is_some();
    if icons::button(
        ui,
        "fit-art",
        tr(
            "Ajustar a página à arte (todas as páginas ficam com esse tamanho)",
            "Fit page to artwork (every page gets that size)",
        ),
        false,
        has_art && !doc.page_locked(),
    )
    .clicked()
    {
        changed |= pagesize::fit_to_art(&mut doc.editor, doc.page);
    }
    ui.weak(format!(
        "{} {}",
        doc.doc().data.pages.len(),
        tr("página(s) com este tamanho", "page(s) with this size")
    ));
    if new_size.is_some() || new_bleed.is_some() {
        let (nw, nh) = new_size.unwrap_or((w, h));
        let b = new_bleed.unwrap_or(bleed);
        pages::set_size_all(
            &mut doc.editor,
            nw,
            nh,
            (b > 0.0).then_some(b),
            tr("Tamanho das páginas", "Page size"),
        );
        changed |= new_size.is_some();
    }
    changed
}

/// Barra de texto (doc 06): família, corpo em pontos, negrito, itálico e
/// alinhamento. Sem alvo, os valores são os do texto novo. Com `at` (na
/// edição), mostra o estilo daquele ponto do texto. Devolve o estilo a
/// aplicar.
fn text_bar(
    ui: &mut egui::Ui,
    doc: &DocState,
    targets: &[String],
    at: Option<usize>,
    picker: &mut crate::fontpicker::FontPicker,
) -> Option<graphdraw_core::edit::text::TextStyle> {
    use graphdraw_core::edit::text::{self as tedit, TextStyle};
    let current = targets
        .first()
        .and_then(|id| doc.editor.find(id))
        .and_then(|r| doc.editor.get(&r))
        .and_then(|o| match &o.kind {
            ObjectKind::Text(t) => Some(match at {
                Some(i) => tedit::style_at(t, i),
                None => tedit::current_style(t),
            }),
            _ => None,
        });
    let Some((family, size, bold, italic, align)) = current else {
        widgets::hint_line(
            ui,
            tr(
                "Clique para texto artístico · arraste para um quadro de parágrafo · Esc termina",
                "Click for artistic text · drag for a paragraph frame · Esc finishes",
            ),
        );
        return None;
    };
    let mut out = None;
    // Só os estilos que a fonte tem instalados (pesos e itálicos).
    let weight = targets
        .first()
        .and_then(|id| doc.editor.find(id))
        .and_then(|r| doc.editor.get(&r))
        .and_then(|o| match &o.kind {
            ObjectKind::Text(t) => Some(match at {
                Some(i) => tedit::weight_at(t, i),
                None => tedit::current_weight(t),
            }),
            _ => None,
        })
        .unwrap_or(400);
    let styles = graphdraw_core::render::text::font_styles(&family);
    let has_bold = styles.iter().any(|(w, i)| *w >= 600 && *i == italic);
    let has_italic = styles.iter().any(|(w, i)| *i && (*w >= 600) == bold);
    let pt = 25_400.0 / 72.0;
    let mut points = (size / pt * 10.0).round() / 10.0;
    widgets::stack(ui, |ui, row| {
        if row == 0 {
            if let Some(fam) = picker.show(ui, &family)
                && fam != family
            {
                out = Some(TextStyle {
                    family: Some(fam),
                    ..Default::default()
                });
            };
        } else {
            let name = graphdraw_core::render::text::style_name(weight, italic);
            egui::ComboBox::from_id_salt("texto-estilo")
                .selected_text(name)
                .width(theme::FONT_STYLE_WIDTH)
                .show_ui(ui, |ui| {
                    for (w, i) in &styles {
                        let label = graphdraw_core::render::text::style_name(*w, *i);
                        if ui
                            .selectable_label(*w == weight && *i == italic, label)
                            .clicked()
                        {
                            out = Some(TextStyle {
                                weight: Some(*w),
                                italic: Some(*i),
                                ..Default::default()
                            });
                        }
                    }
                })
                .response
                .on_hover_text(tr(
                    "Estilos que esta fonte tem instalados",
                    "Styles installed for this font",
                ));
            if ui
                .add(
                    egui::DragValue::new(&mut points)
                        .range(1.0..=1000.0)
                        .speed(0.2)
                        .suffix(" pt"),
                )
                .on_hover_text(tr("Corpo", "Size"))
                .changed()
            {
                out = Some(TextStyle {
                    size: Some(points * pt),
                    ..Default::default()
                });
            };
        }
    });
    widgets::stack(ui, |ui, row| {
        if row == 0 {
            let tip = if has_bold || bold {
                tr("Negrito", "Bold")
            } else {
                tr("Esta fonte não tem negrito", "This font has no bold")
            };
            if icons::button(ui, "text-bold", tip, bold, has_bold || bold).clicked() {
                out = Some(TextStyle {
                    bold: Some(!bold),
                    ..Default::default()
                });
            };
        } else {
            let tip = if has_italic || italic {
                tr("Itálico", "Italic")
            } else {
                tr("Esta fonte não tem itálico", "This font has no italic")
            };
            if icons::button(ui, "text-italic", tip, italic, has_italic || italic).clicked() {
                out = Some(TextStyle {
                    italic: Some(!italic),
                    ..Default::default()
                });
            };
        }
    });
    ui.separator();
    let mut aligns = Vec::new();
    for (icon, tip, a, cmd) in [
        (
            "text-left",
            tr("À esquerda", "Left"),
            "left",
            crate::keymap::Command::TextAlignLeft,
        ),
        (
            "text-center",
            tr("Centralizado", "Center"),
            "center",
            crate::keymap::Command::TextAlignCenter,
        ),
        (
            "text-right",
            tr("À direita", "Right"),
            "right",
            crate::keymap::Command::TextAlignRight,
        ),
        (
            "text-justify",
            tr(
                "Justificado (texto de parágrafo)",
                "Justified (paragraph text)",
            ),
            "justify",
            crate::keymap::Command::TextJustify,
        ),
    ] {
        aligns.push((icon, crate::keymap::hint(tip, cmd), a));
    }
    let mut picked = None;
    let mut buttons = |ui: &mut egui::Ui, items: &[(&str, &str, &str)]| {
        for (icon, tip, a) in items {
            if icons::button(ui, icon, tip, align == *a, true).clicked() {
                picked = Some(a.to_string());
            }
        }
    };
    widgets::stack(ui, |ui, row| {
        if row == 0 {
            buttons(ui, &aligns[..2]);
        } else {
            buttons(ui, &aligns[2..]);
        }
    });
    if let Some(a) = picked {
        out = Some(TextStyle {
            align: Some(a),
            ..Default::default()
        });
    }
    out
}

/// Ângulo de rotação (graus, −180..180) do primeiro objeto selecionado, na
/// página (inclui a rotação dos grupos pais).
fn selection_angle(doc: &DocState, sel: &[String]) -> f64 {
    let Some(r) = sel.first().and_then(|id| doc.editor.find(id)) else {
        return 0.0;
    };
    let Some(o) = doc.editor.get(&r) else {
        return 0.0;
    };
    let m = (doc.editor.parent_affine(&r) * o.transform.affine()).as_coeffs();
    let a = m[1].atan2(m[0]).to_degrees();
    // Valor limpo: arredonda ruído numérico (89,9999999 → 90).
    let a = (a * 1e6).round() / 1e6;
    if a <= -180.0 { a + 360.0 } else { a }
}

/// Lados/pontas, modo (polígono, estrela, estrela complexa) e nitidez
/// (GD-PRI-03/04). Devolve os novos valores quando algum muda.
/// Espiral (GD-PRI-05): voltas, simétrica ou logarítmica e expansão.
/// Nome da forma básica na interface (GD-PRI-07).
pub(crate) fn basic_label(s: &str) -> &'static str {
    match s {
        "arrow" => tr("Seta", "Arrow"),
        "double-arrow" => tr("Seta dupla", "Double arrow"),
        "chevron" => tr("Divisa", "Chevron"),
        "flow-process" => tr("Processo", "Process"),
        "flow-decision" => tr("Decisão", "Decision"),
        "flow-terminator" => tr("Terminal", "Terminator"),
        "flow-data" => tr("Dados", "Data"),
        "flow-document" => tr("Documento", "Document"),
        "banner" => tr("Faixa", "Banner"),
        "speech" => tr("Balão de fala (chamada)", "Speech balloon (callout)"),
        "thought" => tr("Balão de pensamento", "Thought balloon"),
        "plus" => tr("Cruz", "Plus"),
        "heart" => tr("Coração", "Heart"),
        _ => tr("Forma", "Shape"),
    }
}

/// Escolha da forma básica (GD-PRI-07); ajuste pelas alças na ferramenta Forma.
fn basic_controls(ui: &mut egui::Ui, d: &mut crate::canvas::Defaults) {
    egui::ComboBox::from_id_salt("forma-basica")
        .selected_text(basic_label(&d.basic_shape))
        .show_ui(ui, |ui| {
            for s in graphdraw_core::render::basic::SHAPES {
                ui.selectable_value(&mut d.basic_shape, s.to_string(), basic_label(s));
            }
        });
    ui.separator();
    widgets::hint_line(
        ui,
        tr(
            "Arraste para desenhar · na ferramenta Forma, as alças em losango ajustam (a ponta do balão aponta a chamada)",
            "Drag to draw · with the Shape tool, the diamond handles adjust it (the balloon tip points the callout)",
        ),
    );
}

fn spiral_controls(ui: &mut egui::Ui, d: &mut crate::canvas::Defaults) {
    widgets::stack(ui, |ui, row| {
        if row == 0 {
            ui.label(tr("Voltas", "Turns"));
            ui.add(
                egui::DragValue::new(&mut d.spiral_turns)
                    .range(0.25..=100.0)
                    .speed(0.05),
            );
        } else {
            ui.selectable_value(&mut d.spiral_log, false, tr("Simétrica", "Symmetric"));
            ui.selectable_value(&mut d.spiral_log, true, tr("Logarítmica", "Logarithmic"));
            if d.spiral_log {
                let mut pct = ((d.spiral_expansion - 1.0) * 100.0).round();
                if ui
                    .add(
                        egui::DragValue::new(&mut pct)
                            .range(1.0..=900.0)
                            .suffix("%"),
                    )
                    .on_hover_text(tr(
                        "Quanto o raio cresce a cada volta",
                        "How much the radius grows per turn",
                    ))
                    .changed()
                {
                    d.spiral_expansion = 1.0 + pct / 100.0;
                }
            }
        }
    });
}

/// Papel gráfico (GD-PRI-06): colunas × linhas.
fn graph_controls(ui: &mut egui::Ui, d: &mut crate::canvas::Defaults) {
    widgets::stack(ui, |ui, row| {
        if row == 0 {
            ui.label(tr("Colunas", "Columns"));
            ui.add(
                egui::DragValue::new(&mut d.grid_cols)
                    .range(1..=200)
                    .speed(0.1),
            );
        } else {
            ui.label(tr("Linhas", "Rows"));
            ui.add(
                egui::DragValue::new(&mut d.grid_rows)
                    .range(1..=200)
                    .speed(0.1),
            );
        }
    });
}

pub fn polygon_controls(
    ui: &mut egui::Ui,
    sides: u32,
    mode: &str,
    sharpness: f64,
) -> Option<(u32, String, f64)> {
    let mut out = None;
    let (mut n, mut m, mut k) = (sides, mode.to_string(), sharpness);
    widgets::stack(ui, |ui, row| {
        if row == 0 {
            for (key, icon, tip) in [
                ("polygon", "tool-polygon", tr("Polígono", "Polygon")),
                ("star", "poly-star", tr("Estrela", "Star")),
                (
                    "complex",
                    "poly-complex",
                    tr(
                        "Estrela complexa (linhas cruzadas)",
                        "Complex star (crossing lines)",
                    ),
                ),
            ] {
                if icons::button(ui, icon, tip, m == key, true).clicked() && m != key {
                    m = key.to_string();
                    out = Some(());
                }
            }
        } else {
            let label = if m == "polygon" {
                tr("Lados", "Sides")
            } else {
                tr("Pontas", "Points")
            };
            ui.label(label);
            let min = if m == "complex" { 5 } else { 3 };
            if ui
                .add(egui::DragValue::new(&mut n).range(min..=500).speed(0.1))
                .changed()
            {
                out = Some(());
            }
            if m != "polygon" {
                ui.label(tr("Nitidez", "Sharpness"));
                let mut pct = (k * 100.0).round();
                if ui
                    .add(
                        egui::DragValue::new(&mut pct)
                            .range(0.0..=100.0)
                            .suffix("%"),
                    )
                    .changed()
                {
                    k = pct / 100.0;
                    out = Some(());
                }
            }
        }
    });
    if m == "complex" {
        n = n.max(5);
    }
    out.map(|_| (n, m, k))
}

/// Seletor dos cantos do retângulo (2 × 2, como na tela): cada célula
/// mostra o canto arredondado e alterna se aquele canto entra na
/// alteração. Índices do modelo: 0 inf.-esq., 1 inf.-dir., 2 sup.-dir.,
/// 3 sup.-esq.
pub fn corner_picker(ui: &mut egui::Ui, sel: &mut [bool; 4]) {
    let (cell, gap) = (theme::CORNER_CELL, theme::CORNER_GAP);
    let side = 2.0 * cell + gap;
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(side), egui::Sense::click());
    // Posição na tela (linha, coluna) → índice do modelo.
    let at = |row: usize, col: usize| match (row, col) {
        (0, 0) => 3,
        (0, _) => 2,
        (_, 0) => 0,
        _ => 1,
    };
    let cell_at = |p: egui::Pos2| {
        let d = p - rect.min;
        rect.contains(p).then(|| {
            at(
                (d.y > cell + gap / 2.0) as usize,
                (d.x > cell + gap / 2.0) as usize,
            )
        })
    };
    let hovered = resp.hover_pos().and_then(cell_at);
    let pal = theme::palette(ui);
    for row in 0..2 {
        for col in 0..2 {
            let i = at(row, col);
            let min = rect.min + Vec2::new(col as f32 * (cell + gap), row as f32 * (cell + gap));
            let r = egui::Rect::from_min_size(min, Vec2::splat(cell));
            let c = if sel[i] {
                theme::ACCENT_PRIMARY
            } else if hovered == Some(i) {
                pal.fg_muted
            } else {
                pal.border_default
            };
            // Só o canto de fora fica arredondado: mostra qual canto é.
            let big = (cell * 0.8) as u8;
            let radius = egui::CornerRadius {
                nw: if (row, col) == (0, 0) { big } else { 0 },
                ne: if (row, col) == (0, 1) { big } else { 0 },
                sw: if (row, col) == (1, 0) { big } else { 0 },
                se: if (row, col) == (1, 1) { big } else { 0 },
            };
            ui.painter().rect_filled(r, radius, c);
        }
    }
    let resp = resp.on_hover_text(tr(
        "Cantos alterados (clique para marcar ou desmarcar cada um)",
        "Corners to change (click to toggle each one)",
    ));
    if resp.clicked()
        && let Some(i) = resp.interact_pointer_pos().and_then(cell_at)
    {
        sel[i] = !sel[i];
        // Nenhum marcado não faria nada: volta a marcar só este.
        if !sel.iter().any(|v| *v) {
            sel[i] = true;
        }
    }
}

/// Aplica tipo e raio só aos cantos marcados (os outros ficam como estão).
pub fn set_corners(corners: &mut Option<Vec<Corner>>, sel: &[bool; 4], kind: &str, radius: f64) {
    let mut cs = corners.clone().unwrap_or_else(|| {
        vec![
            Corner {
                kind: "round".into(),
                radius: 0.0,
            };
            4
        ]
    });
    cs.resize(
        4,
        Corner {
            kind: "round".into(),
            radius: 0.0,
        },
    );
    for (i, c) in cs.iter_mut().enumerate() {
        if sel[i] {
            *c = Corner {
                kind: kind.to_string(),
                radius,
            };
        }
    }
    *corners = Some(cs);
}

fn select_bar(
    ui: &mut egui::Ui,
    doc: &mut DocState,
    anchor: &mut Anchor,
    lock: &mut bool,
    corners_sel: &mut [bool; 4],
    object_params: &mut dyn FnMut(&mut egui::Ui, &mut DocState),
) -> bool {
    let sel = doc.selection.clone();
    let Some(b) = ops::selection_bounds(&doc.editor, &sel, false) else {
        return page_bar_props(ui, doc);
    };
    anchor_picker(ui, anchor);
    let p = anchor.point(b);
    let (mut nx, mut ny) = (None, None);
    widgets::stack(ui, |ui, row| {
        if row == 0 {
            nx = labeled_length(ui, "X", "x", p.x, None);
        } else {
            ny = labeled_length(ui, "Y", "y", p.y, None);
        }
    });
    if let Some(x) = nx {
        ops::set_position(&mut doc.editor, &sel, *anchor, Point::new(x, p.y));
    }
    if let Some(y) = ny {
        ops::set_position(&mut doc.editor, &sel, *anchor, Point::new(p.x, y));
    }
    ui.separator();
    let (mut nw, mut nh) = (None, None);
    widgets::stack(ui, |ui, row| {
        if row == 0 {
            nw = labeled_length(ui, tr("L", "W"), "w", b.width(), Some(b.width()));
        } else {
            nh = labeled_length(ui, tr("A", "H"), "h", b.height(), Some(b.height()));
        }
    });
    if let Some(w) = nw {
        ops::set_size(&mut doc.editor, &sel, *anchor, Some(w), None, *lock);
    }
    if let Some(h) = nh {
        ops::set_size(&mut doc.editor, &sel, *anchor, None, Some(h), *lock);
    }
    // Cadeado fechado = proporção travada; aberto = largura e altura livres.
    let (icon, tip) = if *lock {
        (
            "lock-on",
            tr(
                "Proporção mantida (clique para destravar)",
                "Proportions kept (click to unlock)",
            ),
        )
    } else {
        (
            "lock-off",
            tr(
                "Proporção livre (clique para manter)",
                "Proportions free (click to keep)",
            ),
        )
    };
    if icons::button(ui, icon, tip, *lock, true).clicked() {
        *lock = !*lock;
    }
    ui.separator();
    // Ângulo atual do (primeiro) objeto; digitar gira até o valor.
    let current = selection_angle(doc, &sel);
    if let Some(a) = widgets::icon_number(
        ui,
        "rotate-ccw",
        tr(
            "Ângulo de rotação (arraste o ícone ou digite)",
            "Rotation angle (drag the icon or type)",
        ),
        "angulo",
        current,
        "°",
    ) {
        let delta = a - current;
        if delta.abs() > 1e-9 {
            ops::transform(
                &mut doc.editor,
                &sel,
                ops::rotate_about(anchor.point(b), delta),
                tr("Girar", "Rotate"),
            );
        }
    }
    let mut mirror = None;
    widgets::stack(ui, |ui, row| {
        if row == 0 {
            if icons::button(
                ui,
                "mirror-h",
                tr("Espelhar na horizontal", "Mirror horizontally"),
                false,
                true,
            )
            .clicked()
            {
                mirror = Some(true);
            };
        } else {
            if icons::button(
                ui,
                "mirror-v",
                tr("Espelhar na vertical", "Mirror vertically"),
                false,
                true,
            )
            .clicked()
            {
                mirror = Some(false);
            };
        }
    });
    if let Some(h) = mirror {
        ops::transform(
            &mut doc.editor,
            &sel,
            ops::mirror_about(b.center(), h),
            tr("Espelhar", "Mirror"),
        );
    }
    // ---- depois do ângulo: grupos do contexto (DESIGN-SYSTEM 4.6) ----
    // 1. Parâmetros do objeto (primitivas).
    // Primitivas (GD-PRI-01/02): parâmetros editáveis até converter em curvas.
    let objs = doc.selected_objects();
    let has_primitive = objs.iter().any(|o| {
        matches!(
            o.kind,
            ObjectKind::Rectangle(_) | ObjectKind::Ellipse(_) | ObjectKind::Polygon(_)
        )
    });
    let single = (objs.len() == 1).then(|| objs[0].clone());
    if has_primitive
        && icons::button(
            ui,
            "to-curves",
            crate::keymap::hint(
                tr("Converter em curvas", "Convert to curves"),
                crate::keymap::Command::ToCurves,
            ),
            false,
            true,
        )
        .clicked()
    {
        ops::convert_to_curves(&mut doc.editor, &sel);
    }
    if let Some(o) = single {
        match &o.kind {
            ObjectKind::Rectangle(r) => {
                ui.separator();
                // Valores mostrados: os do primeiro canto marcado.
                let first = (0..4).find(|i| corners_sel[*i]).unwrap_or(0);
                let corner = r
                    .corners
                    .as_ref()
                    .and_then(|c| c.get(first))
                    .cloned()
                    .unwrap_or(Corner {
                        kind: "round".into(),
                        radius: 0.0,
                    });
                corner_picker(ui, corners_sel);
                let mut kind = corner.kind.clone();
                let mut new_radius = None;
                let mut all = false;
                widgets::stack(ui, |ui, row| {
                    if row == 0 {
                        egui::ComboBox::from_id_salt("canto")
                            .selected_text(corner_label(&kind))
                            .width(theme::CORNER_KIND_WIDTH)
                            .show_ui(ui, |ui| {
                                for k in ["round", "chamfer", "scallop"] {
                                    ui.selectable_value(&mut kind, k.to_string(), corner_label(k));
                                }
                            });
                        all = ui
                            .add_enabled(
                                corners_sel.iter().any(|v| !*v),
                                egui::Button::new(tr("Todos", "All")).small(),
                            )
                            .on_hover_text(tr("Marcar os 4 cantos", "Select all 4 corners"))
                            .clicked();
                    } else {
                        ui.label(tr("Raio", "Radius"));
                        new_radius = length_field(
                            ui,
                            "raio",
                            corner.radius,
                            Some(r.width.min(r.height) / 2.0),
                        );
                    }
                });
                if all {
                    *corners_sel = [true; 4];
                }
                if kind != corner.kind || new_radius.is_some() {
                    let radius = new_radius.unwrap_or(corner.radius).max(0.0);
                    let which = *corners_sel;
                    ops::modify(
                        &mut doc.editor,
                        &sel,
                        tr("Alterar cantos", "Change corners"),
                        |o| {
                            if let ObjectKind::Rectangle(r) = &mut o.kind {
                                set_corners(&mut r.corners, &which, &kind, radius);
                                return true;
                            }
                            false
                        },
                    );
                }
            }
            ObjectKind::Polygon(pd) => {
                ui.separator();
                if let Some((n, m, k)) = polygon_controls(ui, pd.sides, &pd.mode, pd.sharpness) {
                    ops::modify(
                        &mut doc.editor,
                        &sel,
                        tr("Alterar polígono", "Change polygon"),
                        |o| {
                            if let ObjectKind::Polygon(p) = &mut o.kind {
                                p.sides = n;
                                p.mode = m.clone();
                                p.sharpness = k;
                                return true;
                            }
                            false
                        },
                    );
                }
            }
            ObjectKind::Ellipse(e) => {
                ui.separator();
                let mut mode = e.mode.clone();
                egui::ComboBox::from_id_salt("modo-elipse")
                    .selected_text(ellipse_label(&mode))
                    .width(80.0)
                    .show_ui(ui, |ui| {
                        for m in ["ellipse", "arc", "pie"] {
                            ui.selectable_value(&mut mode, m.to_string(), ellipse_label(m));
                        }
                    });
                let mut start = None;
                let mut end = None;
                if e.mode != "ellipse" {
                    start = number_field(ui, "ini", e.start_angle, "°");
                    end = number_field(ui, "fim", e.end_angle, "°");
                }
                if mode != e.mode || start.is_some() || end.is_some() {
                    ops::modify(
                        &mut doc.editor,
                        &sel,
                        tr("Alterar elipse", "Change ellipse"),
                        |o| {
                            if let ObjectKind::Ellipse(e) = &mut o.kind {
                                e.mode = mode.clone();
                                if let Some(s) = start {
                                    e.start_angle = s;
                                }
                                if let Some(v) = end {
                                    e.end_angle = v;
                                }
                                return true;
                            }
                            false
                        },
                    );
                }
            }
            _ => {}
        }
    }
    object_params(ui, doc);
    // 2. Contorno (edição rápida).
    if stroke_bar(ui, doc, &sel) {
        return false;
    }
    // 3. Combinar: booleanas (2 ou mais objetos).
    // Booleanas com dois ou mais objetos (doc 09): alvo = último selecionado.
    if sel.len() >= 2 {
        ui.separator();
        use crate::keymap::{Command as K, hint};
        use graphdraw_core::edit::shapeops::{self, BoolOp, ShapeOp};
        #[derive(Clone, Copy)]
        enum B {
            Bool(BoolOp),
            Shape(ShapeOp),
        }
        let buttons = [
            (
                "bool-weld",
                hint(tr("Soldar", "Weld"), K::Weld),
                B::Bool(BoolOp::Weld),
            ),
            (
                "bool-trim",
                hint(
                    tr(
                        "Aparar: o último selecionado perde a área dos outros",
                        "Trim: the last selected loses the others' area",
                    ),
                    K::Trim,
                ),
                B::Bool(BoolOp::Trim),
            ),
            (
                "bool-intersect",
                hint(
                    tr(
                        "Interseção: novo objeto com a área comum",
                        "Intersect: new object with the common area",
                    ),
                    K::Intersect,
                ),
                B::Bool(BoolOp::Intersect),
            ),
            (
                "bool-simplify",
                hint(
                    tr(
                        "Simplificar: cada objeto fica só com a área visível",
                        "Simplify: each object keeps only its visible area",
                    ),
                    K::Simplify,
                ),
                B::Shape(ShapeOp::Simplify),
            ),
            (
                "bool-exclude",
                hint(
                    tr(
                        "Excluir: só a área sem sobreposição",
                        "Exclude: only the non-overlapping area",
                    ),
                    K::Exclude,
                ),
                B::Shape(ShapeOp::Exclude),
            ),
            (
                "bool-front-minus",
                hint(
                    tr(
                        "Frente menos trás: o da frente perde a área dos demais",
                        "Front minus back: the front object loses the others' area",
                    ),
                    K::FrontMinusBack,
                ),
                B::Shape(ShapeOp::FrontMinusBack),
            ),
            (
                "bool-back-minus",
                hint(
                    tr(
                        "Trás menos frente: o de trás perde a área dos demais",
                        "Back minus front: the back object loses the others' area",
                    ),
                    K::BackMinusFront,
                ),
                B::Shape(ShapeOp::BackMinusFront),
            ),
            (
                "bool-divide",
                hint(
                    tr(
                        "Dividir: cada região de sobreposição vira um objeto",
                        "Divide: each overlap region becomes an object",
                    ),
                    K::Divide,
                ),
                B::Shape(ShapeOp::Divide),
            ),
            (
                "bool-boundary",
                hint(
                    tr(
                        "Limite: novo contorno em volta da seleção",
                        "Boundary: new outline around the selection",
                    ),
                    K::Boundary,
                ),
                B::Shape(ShapeOp::Boundary),
            ),
        ];
        let mut picked = None;
        widgets::stack(ui, |ui, row| {
            let range = if row == 0 { 0..5 } else { 5..buttons.len() };
            for (icon, tip, op) in &buttons[range] {
                if icons::button(ui, icon, tip, false, true).clicked() {
                    picked = Some(*op);
                }
            }
        });
        let result = match picked {
            Some(B::Bool(op)) => shapeops::boolean(&mut doc.editor, &sel, op, op.default_options()),
            Some(B::Shape(op)) => shapeops::shape_op(&mut doc.editor, &sel, op),
            None => None,
        };
        if let Some(n) = result {
            doc.selection = n;
            return false;
        }
    }
    // 4. Ordem e agrupar.
    ui.separator();
    let mut order_buttons = Vec::new();
    for (icon, tip, order) in [
        (
            "order-front",
            crate::keymap::hint(
                tr("Trazer para frente", "Bring to front"),
                crate::keymap::Command::ToFront,
            ),
            Order::ToFront,
        ),
        (
            "order-forward",
            crate::keymap::hint(
                tr("Um passo à frente", "Bring forward"),
                crate::keymap::Command::Forward,
            ),
            Order::Forward,
        ),
        (
            "order-backward",
            crate::keymap::hint(
                tr("Um passo atrás", "Send backward"),
                crate::keymap::Command::Backward,
            ),
            Order::Backward,
        ),
        (
            "order-back",
            crate::keymap::hint(
                tr("Enviar para trás", "Send to back"),
                crate::keymap::Command::ToBack,
            ),
            Order::ToBack,
        ),
    ] {
        order_buttons.push((icon, tip, order));
    }
    let mut reorder = None;
    let mut buttons = |ui: &mut egui::Ui, items: &[(&str, &str, Order)]| {
        for (icon, tip, order) in items {
            if icons::button(ui, icon, tip, false, true).clicked() {
                reorder = Some(*order);
            }
        }
    };
    widgets::stack(ui, |ui, row| {
        if row == 0 {
            buttons(ui, &order_buttons[..2]);
        } else {
            buttons(ui, &order_buttons[2..]);
        }
    });
    if let Some(o) = reorder {
        ops::reorder(&mut doc.editor, &sel, o);
    }
    ui.separator();
    let any_group = doc
        .selected_objects()
        .iter()
        .any(|o| matches!(o.kind, ObjectKind::Group(_)));
    let (mut do_group, mut do_ungroup) = (false, false);
    widgets::stack(ui, |ui, row| {
        if row == 0 {
            do_group = icons::button(
                ui,
                "group",
                crate::keymap::hint(tr("Agrupar", "Group"), crate::keymap::Command::Group_),
                false,
                sel.len() > 1,
            )
            .clicked();
        } else {
            do_ungroup = icons::button(
                ui,
                "ungroup",
                crate::keymap::hint(tr("Desagrupar", "Ungroup"), crate::keymap::Command::Ungroup),
                false,
                any_group,
            )
            .clicked();
        }
    });
    if do_group && let Some(id) = ops::group(&mut doc.editor, &sel) {
        doc.selection = vec![id];
    }
    if do_ungroup {
        doc.selection = ops::ungroup(&mut doc.editor, &sel);
    }

    false
}

/// Contorno na barra de propriedades (edição rápida; a janela de contorno
/// fica para o ajuste fino): espessura, estilo de linha e, quando cabe,
/// pontas de seta (curvas abertas) e caneta caligráfica. Devolve `true` se
/// a seleção mudou.
fn stroke_bar(ui: &mut egui::Ui, doc: &mut DocState, sel: &[String]) -> bool {
    use crate::dialogs::{DASHES, arrow_label, dash_for, detect_dash};
    use graphdraw_core::model::{Arrow, Stroke};
    let objs = doc.selected_objects();
    let eligible: Vec<_> = objs
        .iter()
        .filter(|o| !matches!(o.kind, ObjectKind::Image(_)))
        .collect();
    if eligible.is_empty() {
        return false;
    }
    let current: Option<Stroke> = eligible.iter().find_map(|o| o.stroke.clone());
    let open_path = eligible.iter().any(|o| match &o.kind {
        ObjectKind::Path(p) => p.subpaths.iter().any(|s| !s.closed),
        _ => false,
    });
    drop(objs);
    type Edit = Box<dyn Fn(&mut Stroke)>;
    let mut edit: Option<(&str, Edit)> = None;
    let mut remove = false;
    let width = current.as_ref().map_or(0.0, |s| s.width);
    ui.separator();
    widgets::stack(ui, |ui, row| {
        if row == 0 {
            if let Some(v) = widgets::labeled_length(
                ui,
                tr("Contorno", "Outline"),
                "contorno-espessura",
                width,
                None,
            ) {
                let v = v.max(0.0);
                edit = Some((
                    tr("Espessura do contorno", "Outline width"),
                    Box::new(move |s| s.width = v),
                ));
            }
        } else {
            let preset = current.as_ref().map_or(0, detect_dash);
            let text = if current.is_none() {
                tr("Sem contorno", "No outline")
            } else {
                DASHES
                    .get(preset)
                    .map_or(tr("Personalizado", "Custom"), |d| tr(d.0, d.1))
            };
            egui::ComboBox::from_id_salt("contorno-estilo")
                .selected_text(text)
                .width(theme::ARROW_COMBO_WIDTH)
                .show_ui(ui, |ui| {
                    if ui
                        .selectable_label(current.is_none(), tr("Sem contorno", "No outline"))
                        .clicked()
                    {
                        remove = true;
                    }
                    for (i, d) in DASHES.iter().enumerate() {
                        if ui
                            .selectable_label(current.is_some() && preset == i, tr(d.0, d.1))
                            .clicked()
                        {
                            edit = Some((
                                tr("Estilo do contorno", "Outline style"),
                                Box::new(move |s| s.dash = dash_for(i, s.width)),
                            ));
                        }
                    }
                });
        }
    });
    // Pontas de seta: só com curva aberta.
    if open_path {
        let (start, end) = current.as_ref().map_or((None, None), |s| {
            (s.start_arrow.clone(), s.end_arrow.clone())
        });
        widgets::stack(ui, |ui, row| {
            let (label, id, cur, is_start) = if row == 0 {
                (tr("Início", "Start"), "barra-seta-inicio", &start, true)
            } else {
                (tr("Fim", "End"), "barra-seta-fim", &end, false)
            };
            ui.label(label);
            let text = cur
                .as_ref()
                .map_or(tr("Nenhuma", "None"), |a| arrow_label(&a.shape));
            egui::ComboBox::from_id_salt(id)
                .selected_text(text)
                .width(theme::ARROW_COMBO_WIDTH)
                .show_ui(ui, |ui| {
                    let mut pick = |shape: Option<&'static str>| {
                        let f: Edit = Box::new(move |s| {
                            let a = shape.map(Arrow::new);
                            if is_start {
                                s.start_arrow = a;
                            } else {
                                s.end_arrow = a;
                            }
                        });
                        edit = Some((tr("Ponta de seta", "Arrowhead"), f));
                    };
                    if ui
                        .selectable_label(cur.is_none(), tr("Nenhuma", "None"))
                        .clicked()
                    {
                        pick(None);
                    }
                    for shape in graphdraw_core::render::arrows::SHAPES {
                        let on = cur.as_ref().is_some_and(|a| a.shape == shape);
                        if ui.selectable_label(on, arrow_label(shape)).clicked() {
                            pick(Some(shape));
                        }
                    }
                });
        });
    }
    // Caneta caligráfica: com contorno.
    if let Some(s) = &current {
        let mut pct = (s.nib_stretch * 100.0).round();
        let mut angle = s.nib_angle;
        widgets::stack(ui, |ui, row| {
            if row == 0 {
                ui.label(tr("Bico", "Nib"));
                if ui
                    .add(
                        egui::DragValue::new(&mut pct)
                            .range(1.0..=100.0)
                            .suffix("%"),
                    )
                    .on_hover_text(tr(
                        "Caneta caligráfica: largura do bico (100% = redondo)",
                        "Calligraphic pen: nib width (100% = round)",
                    ))
                    .changed()
                {
                    let v = pct / 100.0;
                    edit = Some((
                        tr("Caneta caligráfica", "Calligraphic pen"),
                        Box::new(move |s| s.nib_stretch = v),
                    ));
                }
            } else {
                ui.label(tr("Âng.", "Ang."));
                if ui
                    .add_enabled(
                        s.nib_stretch < 0.999,
                        egui::DragValue::new(&mut angle)
                            .range(-360.0..=360.0)
                            .suffix("°"),
                    )
                    .changed()
                {
                    edit = Some((
                        tr("Caneta caligráfica", "Calligraphic pen"),
                        Box::new(move |s| s.nib_angle = angle),
                    ));
                }
            }
        });
    }
    if remove {
        ops::set_stroke(&mut doc.editor, sel, None);
    } else if let Some((_, f)) = edit {
        ops::edit_stroke(&mut doc.editor, sel, f);
    }
    false
}

fn corner_label(k: &str) -> &'static str {
    match k {
        "chamfer" => tr("Chanfro", "Chamfer"),
        "scallop" => tr("Recorte", "Scallop"),
        _ => tr("Redondo", "Round"),
    }
}

fn ellipse_label(m: &str) -> &'static str {
    match m {
        "arc" => tr("Arco", "Arc"),
        "pie" => tr("Fatia", "Pie"),
        _ => tr("Elipse", "Ellipse"),
    }
}

/// Operação sobre o caminho e os nós selecionados (que ela pode atualizar).
type NodeOp = Box<dyn FnOnce(&mut graphdraw_core::model::PathData, &mut Vec<NodeRef>)>;

/// Ferramenta Forma: operações de nós (GD-NOD-01..05).
fn shape_bar(ui: &mut egui::Ui, doc: &mut DocState) {
    if let Some(o) = doc
        .selected_objects()
        .first()
        .filter(|_| doc.selection.len() == 1)
        && matches!(o.kind, ObjectKind::Rectangle(_) | ObjectKind::Ellipse(_))
    {
        ui.label(tr(
            "Primitivas mantêm parâmetros; converta em curvas para editar nós.",
            "Primitives keep parameters; convert to curves to edit nodes.",
        ));
        if icons::button(
            ui,
            "to-curves",
            crate::keymap::hint(
                tr("Converter em curvas", "Convert to curves"),
                crate::keymap::Command::ToCurves,
            ),
            false,
            true,
        )
        .clicked()
        {
            let sel = doc.selection.clone();
            ops::convert_to_curves(&mut doc.editor, &sel);
        }
        return;
    }
    let Some((loc, o, path)) = single_path(doc).map(|s| (s.r, s.obj.clone(), s.path.clone()))
    else {
        ui.label(tr(
            "Selecione uma curva para editar seus nós",
            "Select a curve to edit its nodes",
        ));
        return;
    };
    let sel = doc.nodes.clone();
    let n = sel.len();
    ui.label(format!(
        "{} {}",
        n,
        tr("nó(s) selecionado(s)", "node(s) selected")
    ));
    let mut op: Option<(&str, NodeOp)> = None;
    let seg_after = |path: &graphdraw_core::model::PathData, r: &NodeRef| -> Option<SegRef> {
        let sp = &path.subpaths[r.subpath];
        (r.index < graphdraw_core::geometry::segment_count(sp)).then_some(SegRef {
            subpath: r.subpath,
            index: r.index,
        })
    };
    let has_seg = sel.iter().any(|r| seg_after(&path, r).is_some());
    let two_ends = n == 2 && sel.iter().all(|r| nodes::is_endpoint(&path, *r));
    if icons::button(
        ui,
        "node-add",
        tr(
            "Adicionar nó no meio do segmento",
            "Add node at segment middle",
        ),
        false,
        has_seg,
    )
    .clicked()
    {
        op = Some((
            tr("Adicionar nó", "Add node"),
            Box::new(move |p, s| {
                let mut segs: Vec<SegRef> = s.iter().filter_map(|r| seg_after(p, r)).collect();
                segs.sort();
                s.clear();
                for seg in segs.into_iter().rev() {
                    if let Some(r) = nodes::add_node(p, seg, 0.5) {
                        s.push(r);
                    }
                }
            }),
        ));
    }
    if icons::button(
        ui,
        "node-delete",
        tr("Excluir nós (Del)", "Delete nodes (Del)"),
        false,
        n > 0,
    )
    .clicked()
    {
        op = Some((
            tr("Excluir nós", "Delete nodes"),
            Box::new(|p, s| {
                nodes::delete_nodes(p, s);
                s.clear();
            }),
        ));
    }
    ui.separator();
    if icons::button(
        ui,
        "node-join",
        tr("Unir dois nós extremos", "Join two end nodes"),
        false,
        two_ends,
    )
    .clicked()
    {
        op = Some((
            tr("Unir nós", "Join nodes"),
            Box::new(|p, s| {
                nodes::join(p, s[0], s[1], true);
                s.clear();
            }),
        ));
    }
    if icons::button(
        ui,
        "node-connect",
        tr("Unir com segmento", "Connect with segment"),
        false,
        two_ends,
    )
    .clicked()
    {
        op = Some((
            tr("Unir com segmento", "Connect"),
            Box::new(|p, s| {
                nodes::join(p, s[0], s[1], false);
                s.clear();
            }),
        ));
    }
    if icons::button(
        ui,
        "node-break",
        tr("Quebrar caminho no nó", "Break path at node"),
        false,
        n == 1,
    )
    .clicked()
    {
        op = Some((
            tr("Quebrar caminho", "Break path"),
            Box::new(|p, s| {
                nodes::break_at(p, s[0]);
                s.clear();
            }),
        ));
    }
    let sub = sel.first().map(|r| r.subpath);
    let closed = sub.is_some_and(|k| path.subpaths[k].closed);
    if icons::button(
        ui,
        if closed { "path-open" } else { "path-close" },
        if closed {
            tr("Abrir subcaminho", "Open subpath")
        } else {
            tr("Fechar subcaminho", "Close subpath")
        },
        false,
        sub.is_some(),
    )
    .clicked()
    {
        let k = sub.unwrap();
        op = Some((
            if closed {
                tr("Abrir caminho", "Open path")
            } else {
                tr("Fechar caminho", "Close path")
            },
            Box::new(move |p, _| nodes::set_closed(p, k, !closed)),
        ));
    }
    if icons::button(
        ui,
        "path-reverse",
        tr("Inverter direção", "Reverse direction"),
        false,
        true,
    )
    .clicked()
    {
        let subs: Vec<usize> = if sel.is_empty() {
            (0..path.subpaths.len()).collect()
        } else {
            sel.iter().map(|r| r.subpath).collect()
        };
        op = Some((
            tr("Inverter direção", "Reverse direction"),
            Box::new(move |p, s| {
                let mut subs = subs;
                subs.sort();
                subs.dedup();
                for k in subs {
                    nodes::reverse(p, k);
                }
                s.clear();
            }),
        ));
    }
    ui.separator();
    if icons::button(
        ui,
        "seg-line",
        tr("Converter segmento em reta", "Segment to line"),
        false,
        has_seg,
    )
    .clicked()
    {
        op = Some((
            tr("Converter em reta", "To line"),
            Box::new(move |p, s| {
                for r in s.clone() {
                    if let Some(seg) = seg_after(p, &r) {
                        nodes::to_line(p, seg);
                    }
                }
            }),
        ));
    }
    if icons::button(
        ui,
        "seg-curve",
        tr("Converter segmento em curva", "Segment to curve"),
        false,
        has_seg,
    )
    .clicked()
    {
        op = Some((
            tr("Converter em curva", "To curve"),
            Box::new(move |p, s| {
                for r in s.clone() {
                    if let Some(seg) = seg_after(p, &r) {
                        nodes::to_curve(p, seg);
                    }
                }
            }),
        ));
    }
    ui.separator();
    for (icon, tip, kind) in [
        (
            "node-corner",
            tr("Nó de canto", "Corner node"),
            NodeKind::Corner,
        ),
        (
            "node-smooth",
            tr("Nó suave", "Smooth node"),
            NodeKind::Smooth,
        ),
        (
            "node-symmetric",
            tr("Nó simétrico", "Symmetric node"),
            NodeKind::Symmetric,
        ),
    ] {
        let all_kind = n > 0
            && sel
                .iter()
                .all(|r| nodes::node(&path, *r).is_some_and(|nd| nd.kind == kind));
        if icons::button(ui, icon, tip, all_kind, n > 0).clicked() {
            op = Some((
                tr("Alterar tipo de nó", "Change node type"),
                Box::new(move |p, s| {
                    for r in s.iter() {
                        nodes::set_kind(p, *r, kind);
                    }
                }),
            ));
        }
    }
    if ui
        .button(tr("Todos", "All"))
        .on_hover_text(tr("Selecionar todos os nós", "Select all nodes"))
        .clicked()
    {
        doc.nodes = nodes::all_nodes(&path);
    }
    // Alinhar nós (GD-NOD-06) e extrair subcaminho (GD-NOD-08).
    for (icon, tip, horizontal) in [
        (
            "node-align-h",
            tr(
                "Alinhar nós na horizontal (mesma altura)",
                "Align nodes horizontally (same height)",
            ),
            true,
        ),
        (
            "node-align-v",
            tr(
                "Alinhar nós na vertical (mesma coluna)",
                "Align nodes vertically (same column)",
            ),
            false,
        ),
    ] {
        if icons::button(ui, icon, tip, false, n >= 2).clicked() {
            op = Some((
                tr("Alinhar nós", "Align nodes"),
                Box::new(move |p, s| nodes::align_nodes(p, s, horizontal)),
            ));
        }
    }
    let subs: Vec<usize> = {
        let mut v: Vec<usize> = sel.iter().map(|r| r.subpath).collect();
        v.sort();
        v.dedup();
        v
    };
    if icons::button(
        ui,
        "node-extract",
        tr(
            "Extrair subcaminho: os subcaminhos dos nós escolhidos viram outro objeto",
            "Extract subpath: the chosen nodes' subpaths become another object",
        ),
        false,
        !subs.is_empty() && subs.len() < path.subpaths.len(),
    )
    .clicked()
    {
        let id = o.id.clone();
        if let Some(new) = ops::extract_subpaths(&mut doc.editor, &id, &subs) {
            doc.selection = vec![new];
            doc.nodes.clear();
            return;
        }
    }
    // Reduzir nós (GD-NOD-07): nos subcaminhos dos nós escolhidos, ou em
    // todos; a tolerância fica guardada na sessão.
    ui.separator();
    let tol_id = egui::Id::unique("reduzir-nos-tolerancia");
    let mut tol = ui.data(|d| d.get_temp::<f64>(tol_id)).unwrap_or(100.0);
    if let Some(v) = widgets::labeled_length(ui, tr("Tol.", "Tol."), "reduzir-tol", tol, None) {
        tol = v.max(1.0);
        ui.data_mut(|d| d.insert_temp(tol_id, tol));
    }
    if icons::button(
        ui,
        "node-reduce",
        tr(
            "Reduzir nós: menos nós, desvio até a tolerância; cantos ficam",
            "Reduce nodes: fewer nodes within the tolerance; corners stay",
        ),
        false,
        true,
    )
    .clicked()
    {
        let which: Vec<usize> = {
            let mut v: Vec<usize> = sel.iter().map(|r| r.subpath).collect();
            v.sort();
            v.dedup();
            v
        };
        op = Some((
            tr("Reduzir nós", "Reduce nodes"),
            Box::new(move |p, s| {
                nodes::reduce(p, &which, tol, 10.0);
                s.clear();
            }),
        ));
    }
    if let Some((name, f)) = op {
        let mut no = o.clone();
        let mut s = sel;
        if let ObjectKind::Path(p) = &mut no.kind {
            f(p, &mut s);
            if p.subpaths.is_empty() {
                doc.editor.transaction(name, |ed| {
                    ed.remove_at(&loc);
                });
                doc.selection.clear();
                doc.nodes.clear();
                return;
            }
        }
        doc.editor.transaction(name, |ed| ed.replace_at(&loc, no));
        doc.nodes = s;
    }
}

/// Estado do painel Transformação.
pub struct TransformUi {
    pub mode: usize,
    pub a: f64,
    pub b: f64,
    pub anchor: Anchor,
    pub each: bool,
    pub copies: usize,
    pub proportional: bool,
}

impl Default for TransformUi {
    fn default() -> Self {
        TransformUi {
            mode: 0,
            a: 0.0,
            b: 0.0,
            anchor: Anchor::Center,
            each: false,
            copies: 0,
            proportional: true,
        }
    }
}

/// Painel Transformação (GD-TRF-01/02).
fn transform_panel(ui: &mut egui::Ui, doc: &mut DocState, xf: &mut TransformUi) {
    let modes = [
        tr("Mover", "Move"),
        tr("Girar", "Rotate"),
        tr("Escala %", "Scale %"),
        tr("Tamanho", "Size"),
        tr("Inclinar", "Skew"),
    ];
    ui.horizontal_wrapped(|ui| {
        for (i, m) in modes.iter().enumerate() {
            if ui.selectable_label(xf.mode == i, *m).clicked() && xf.mode != i {
                xf.mode = i;
                (xf.a, xf.b) = match i {
                    2 => (100.0, 100.0),
                    _ => (0.0, 0.0),
                };
            }
        }
    });
    ui.separator();
    let sel = doc.selection.clone();
    let bounds = ops::selection_bounds(&doc.editor, &sel, false);
    if xf.mode == 3 && sel.is_empty() {
        ui.label(tr("Selecione objetos", "Select objects"));
    }
    egui::Grid::new("xf").num_columns(2).show(ui, |ui| {
        let (la, lb, len) = match xf.mode {
            0 => ("ΔX", "ΔY", true),
            1 => (tr("Ângulo (°)", "Angle (°)"), "", false),
            2 => (
                tr("Horizontal %", "Horizontal %"),
                tr("Vertical %", "Vertical %"),
                false,
            ),
            3 => (tr("Largura", "Width"), tr("Altura", "Height"), true),
            _ => (
                tr("Horizontal (°)", "Horizontal (°)"),
                tr("Vertical (°)", "Vertical (°)"),
                false,
            ),
        };
        if xf.mode == 3
            && xf.a == 0.0
            && let Some(b) = bounds
        {
            xf.a = b.width();
            xf.b = b.height();
        }
        ui.label(la);
        ui.horizontal(|ui| {
            let v = if len {
                length_field(ui, "xa", xf.a, bounds.map(|b| b.width()))
            } else {
                number_field(ui, "xa", xf.a, "")
            };
            if let Some(v) = v {
                xf.a = v;
            }
        });
        ui.end_row();
        if !lb.is_empty() {
            ui.label(lb);
            ui.horizontal(|ui| {
                let v = if len {
                    length_field(ui, "xb", xf.b, bounds.map(|b| b.height()))
                } else {
                    number_field(ui, "xb", xf.b, "")
                };
                if let Some(v) = v {
                    xf.b = v;
                }
            });
            ui.end_row();
        }
        ui.label(tr("Referência", "Reference"));
        anchor_picker(ui, &mut xf.anchor);
        ui.end_row();
    });
    if xf.mode == 2 || xf.mode == 3 {
        ui.horizontal(|ui| {
            let icon = if xf.proportional {
                "lock-on"
            } else {
                "lock-off"
            };
            if icons::button(
                ui,
                icon,
                tr("Manter proporção", "Keep proportions"),
                xf.proportional,
                true,
            )
            .clicked()
            {
                xf.proportional = !xf.proportional;
            }
            ui.label(if xf.proportional {
                tr("Proporção mantida", "Proportions kept")
            } else {
                tr("Proporção livre", "Proportions free")
            });
        });
    }
    ui.checkbox(
        &mut xf.each,
        tr("Aplicar a cada objeto", "Apply to each object"),
    );
    ui.horizontal(|ui| {
        ui.label(tr("Cópias", "Copies"));
        ui.add(egui::DragValue::new(&mut xf.copies).range(0..=999));
    });
    let mode = xf.mode;
    let (a, b, anchor, prop) = (xf.a, xf.b, xf.anchor, xf.proportional);
    let make = move |r: kurbo::Rect| -> kurbo::Affine {
        let o = anchor.point(r);
        match mode {
            0 => kurbo::Affine::translate(KVec2::new(a, b)),
            1 => ops::rotate_about(o, a),
            2 => {
                let sy = if prop { a } else { b };
                ops::scale_about(o, a / 100.0, sy / 100.0)
            }
            3 => {
                let sx = if r.width() > 0.0 { a / r.width() } else { 1.0 };
                let sy = if prop {
                    sx
                } else if r.height() > 0.0 {
                    b / r.height()
                } else {
                    1.0
                };
                ops::scale_about(o, sx, sy)
            }
            _ => ops::skew_about(o, a, b),
        }
    };
    let can = !sel.is_empty();
    if ui
        .add_enabled(can, egui::Button::new(tr("Aplicar", "Apply")))
        .clicked()
    {
        let name = modes[mode];
        if xf.copies > 0 {
            if let Some(b) = bounds {
                let ids = ops::transform_copies(&mut doc.editor, &sel, make(b), xf.copies);
                doc.selection = ids;
            }
        } else {
            ops::transform_each(&mut doc.editor, &sel, xf.each, name, make);
        }
    }
}

#[derive(Default)]
pub struct AlignUi {
    pub to: usize,
    pub visual: bool,
    /// Distribuir na extensão da página (senão, da seleção).
    pub dist_page: bool,
    /// Espaçamento fixo (µm) ao distribuir, se ligado.
    pub dist_fixed: bool,
    pub dist_gap: f64,
}

/// Painel Alinhar (GD-ALI-01/02).
fn align_panel(ui: &mut egui::Ui, doc: &mut DocState, al: &mut AlignUi) {
    let labels = [
        tr("Último selecionado", "Last selected"),
        tr("Primeiro selecionado", "First selected"),
        tr("Página", "Page"),
        tr("Seleção", "Selection"),
    ];
    ui.label(tr("Relativo a", "Relative to"));
    for (i, l) in labels.iter().enumerate() {
        ui.radio_value(&mut al.to, i, *l);
    }
    ui.checkbox(
        &mut al.visual,
        tr(
            "Usar caixa visual (com contorno)",
            "Use visual box (with outline)",
        ),
    );
    ui.separator();
    let sel = doc.selection.clone();
    let enabled = !sel.is_empty();
    let (pw, ph) = doc.page_size();
    let page = kurbo::Rect::new(0.0, 0.0, pw, ph);
    // Um objeto só: alinha pela página (GD-ALI-06).
    let to = ops::align_target(
        match al.to {
            0 => AlignTo::LastSelected,
            1 => AlignTo::FirstSelected,
            2 => AlignTo::Page(page),
            _ => AlignTo::Selection,
        },
        sel.len(),
        page,
    );
    ui.horizontal(|ui| {
        for (icon, tip, how) in [
            (
                "align-left",
                tr("Alinhar à esquerda", "Align left"),
                Align::Left,
            ),
            (
                "align-center-h",
                tr("Centralizar na horizontal", "Center horizontally"),
                Align::CenterH,
            ),
            (
                "align-right",
                tr("Alinhar à direita", "Align right"),
                Align::Right,
            ),
        ] {
            if icons::button(ui, icon, tip, false, enabled).clicked() {
                ops::align(&mut doc.editor, &sel, how, to, al.visual);
            }
        }
    });
    ui.horizontal(|ui| {
        for (icon, tip, how) in [
            ("align-top", tr("Alinhar ao topo", "Align top"), Align::Top),
            (
                "align-center-v",
                tr("Centralizar na vertical", "Center vertically"),
                Align::CenterV,
            ),
            (
                "align-bottom",
                tr("Alinhar à base", "Align bottom"),
                Align::Bottom,
            ),
        ] {
            if icons::button(ui, icon, tip, false, enabled).clicked() {
                ops::align(&mut doc.editor, &sel, how, to, al.visual);
            }
        }
        // GD-ALI-04: textos pela primeira linha de base.
        if icons::button(
            ui,
            "align-baseline",
            tr(
                "Alinhar pela linha de base (textos; outros objetos pela base)",
                "Align by baseline (texts; other objects by their bottom)",
            ),
            false,
            enabled,
        )
        .clicked()
        {
            ops::align_baseline(&mut doc.editor, &sel, to, al.visual);
        }
    });
    if !enabled {
        ui.weak(tr("Selecione objetos", "Select objects"));
    } else if sel.len() == 1 && al.to != 2 {
        ui.weak(tr(
            "Um objeto só: alinha pela página",
            "Single object: aligns to the page",
        ));
    }
    // Distribuir (GD-ALI-03).
    ui.separator();
    ui.label(egui::RichText::new(tr("Distribuir", "Distribute")).strong());
    ui.horizontal(|ui| {
        ui.radio_value(&mut al.dist_page, false, tr("Na seleção", "In selection"));
        ui.radio_value(&mut al.dist_page, true, tr("Na página", "On page"));
    });
    ui.horizontal(|ui| {
        ui.checkbox(&mut al.dist_fixed, tr("Espaçamento fixo", "Fixed spacing"));
        if al.dist_fixed
            && let Some(v) = length_field(ui, "dist-gap", al.dist_gap, None)
        {
            al.dist_gap = v.max(0.0);
        }
    });
    let within = al.dist_page.then(|| kurbo::Rect::new(0.0, 0.0, pw, ph));
    let gap = al.dist_fixed.then_some(al.dist_gap);
    let need = if within.is_some() || gap.is_some() {
        2
    } else {
        3
    };
    let can = sel.len() >= need;
    use ops::Distribute as D;
    for row in [
        [
            (
                "dist-left",
                tr("Pelas bordas esquerdas", "By left edges"),
                D::Left,
            ),
            (
                "dist-center-h",
                tr("Pelos centros (horizontal)", "By centers (horizontal)"),
                D::CenterH,
            ),
            (
                "dist-right",
                tr("Pelas bordas direitas", "By right edges"),
                D::Right,
            ),
            (
                "dist-space-h",
                tr("Vãos iguais na horizontal", "Equal gaps horizontally"),
                D::SpaceH,
            ),
        ],
        [
            ("dist-top", tr("Pelos topos", "By top edges"), D::Top),
            (
                "dist-center-v",
                tr("Pelos centros (vertical)", "By centers (vertical)"),
                D::CenterV,
            ),
            (
                "dist-bottom",
                tr("Pelas bases", "By bottom edges"),
                D::Bottom,
            ),
            (
                "dist-space-v",
                tr("Vãos iguais na vertical", "Equal gaps vertically"),
                D::SpaceV,
            ),
        ],
    ] {
        ui.horizontal(|ui| {
            for (icon, tip, how) in row {
                if icons::button(ui, icon, tip, false, can).clicked() {
                    ops::distribute(&mut doc.editor, &sel, how, within, gap, al.visual);
                }
            }
        });
    }
    if !can {
        ui.weak(if need == 3 {
            tr("Selecione ao menos 3 objetos", "Select at least 3 objects")
        } else {
            tr("Selecione ao menos 2 objetos", "Select at least 2 objects")
        });
    }
}

/// Painel Histórico (GD-UND-04): clique volta a qualquer ponto.
fn history_panel(ui: &mut egui::Ui, doc: &mut DocState) {
    let items = doc.editor.history();
    let applied = doc.editor.undo_len();
    let mut goto = None;
    if ui
        .selectable_label(applied == 0, tr("(estado inicial)", "(initial state)"))
        .clicked()
    {
        goto = Some(0);
    }
    for (i, (name, done)) in items.iter().enumerate() {
        let text = if *done {
            egui::RichText::new(name)
        } else {
            egui::RichText::new(name).weak().italics()
        };
        if ui.selectable_label(applied == i + 1, text).clicked() {
            goto = Some(i + 1);
        }
    }
    if let Some(n) = goto {
        doc.editor.goto(n);
        doc.prune_selection();
    }
}

/// Páginas (GD-PAG-01, GD-PAG-03). Usa as mesmas operações da barra de
/// abas. Devolve `true` ao trocar de página.
fn pages_panel(ui: &mut egui::Ui, doc: &mut DocState) -> bool {
    use graphdraw_core::edit::pages;
    let mut switch = None;
    let n = doc.doc().data.pages.len();
    for (i, p) in doc.doc().data.pages.iter().enumerate() {
        let label = format!(
            "{}. {}\n{} × {} mm",
            i + 1,
            pages::display_name(p, i),
            fmt_mm(p.width),
            fmt_mm(p.height)
        );
        if ui.selectable_label(i == doc.page, label).clicked() {
            switch = Some(i);
        }
    }
    ui.separator();
    let cur = doc.page;
    ui.horizontal_wrapped(|ui| {
        if ui.button(tr("+ Página", "+ Page")).clicked() {
            switch = Some(pages::add_after(&mut doc.editor, cur));
        }
        if ui.button(tr("Duplicar", "Duplicate")).clicked() {
            switch = Some(pages::duplicate(&mut doc.editor, cur, cur + 1));
        }
        if ui
            .add_enabled(n > 1, egui::Button::new(tr("Excluir", "Delete")))
            .clicked()
        {
            pages::delete(&mut doc.editor, &[cur]);
            doc.sync();
            switch = Some(doc.page);
        }
    });
    // Tamanho das páginas: um só por documento (GD-PAG-01).
    ui.separator();
    let (w, h) = doc.page_size();
    let mut nw = None;
    let mut nh = None;
    ui.horizontal(|ui| {
        ui.label(tr("L", "W"));
        nw = length_field(ui, "pw", w, None);
    });
    ui.horizontal(|ui| {
        ui.label(tr("A", "H"));
        nh = length_field(ui, "ph", h, None);
    });
    ui.horizontal_wrapped(|ui| {
        for (name, pw, ph) in [
            ("A4", 210_000.0, 297_000.0),
            ("A3", 297_000.0, 420_000.0),
            ("A5", 148_000.0, 210_000.0),
            (tr("Carta", "Letter"), 215_900.0, 279_400.0),
        ] {
            if ui.small_button(name).clicked() {
                nw = Some(pw);
                nh = Some(ph);
            }
        }
        if ui
            .small_button(tr("⟲ Orientação", "⟲ Orientation"))
            .clicked()
        {
            nw = Some(h);
            nh = Some(w);
        }
    });
    if nw.is_some() || nh.is_some() {
        let (w2, h2) = (nw.unwrap_or(w).max(1000.0), nh.unwrap_or(h).max(1000.0));
        let bleed = doc.page().bleed;
        pages::set_size_all(
            &mut doc.editor,
            w2,
            h2,
            bleed,
            tr("Tamanho das páginas", "Page size"),
        );
    }
    if let Some(i) = switch {
        doc.set_page(i.min(doc.doc().data.pages.len() - 1));
        return true;
    }
    false
}

enum LayerAction {
    Color(usize, Option<graphdraw_core::model::Color>),
    Visible(usize, bool),
    Printable(usize, bool),
    Locked(usize, bool),
    Wireframe(usize, bool),
    Rename(usize, String),
    Choose(String),
    Add,
    Delete(usize),
    Move(usize, usize),
    MoveSelection(usize),
}

/// Camadas da página atual (GD-CAM-01, GD-CAM-03) com os objetos dentro
/// (GD-UI-02). O topo da lista é a camada da frente; a marcada recebe os
/// objetos novos.
fn layers_panel(ui: &mut egui::Ui, doc: &mut DocState, tree: &mut crate::layertree::LayerTree) {
    let page = doc.page;
    let n = doc.page().layers.len();
    let chosen = doc.chosen_layer();
    let sel_layers: Vec<usize> = ops::refs(&doc.editor, &doc.selection)
        .iter()
        .map(|r| r.loc.layer)
        .collect();
    let mut act = None;
    let mut obj_act = None;
    tree.begin();
    for i in (0..n).rev() {
        let l = &doc.page().layers[i];
        let open_id = ui.make_persistent_id(("camada-aberta", &l.id));
        let state = egui::collapsing_header::CollapsingState::load_with_default_open(
            ui.ctx(),
            open_id,
            true,
        );
        let header = state.show_header(ui, |ui| {
            // Cor de identificação: contorno da seleção dos objetos da camada.
            let sw = match &l.color {
                Some(c) => swatch(ui, Some(c), theme::SWATCH * 0.7),
                None => swatch(ui, None, theme::SWATCH * 0.7),
            }
            .on_hover_text(tr(
                "Cor de identificação da camada",
                "Layer identification color",
            ));
            egui::Popup::from_toggle_button_response(&sw).show(|ui| {
                ui.horizontal(|ui| {
                    for [r, g, b] in theme::LAYER_COLORS {
                        let c = graphdraw_core::model::Color::rgb(
                            r as f64 / 255.0,
                            g as f64 / 255.0,
                            b as f64 / 255.0,
                        );
                        if swatch(ui, Some(&c), theme::SWATCH).clicked() {
                            act = Some(LayerAction::Color(i, Some(c)));
                            ui.close();
                        }
                    }
                    if swatch(ui, None, theme::SWATCH)
                        .on_hover_text(tr("Sem cor", "No color"))
                        .clicked()
                    {
                        act = Some(LayerAction::Color(i, None));
                        ui.close();
                    }
                });
            });
            let (eye, eye_tip) = if l.visible {
                (
                    "eye-on",
                    tr("Visível (clique para ocultar)", "Visible (click to hide)"),
                )
            } else {
                (
                    "eye-off",
                    tr("Oculta (clique para mostrar)", "Hidden (click to show)"),
                )
            };
            if icons::small_button(ui, eye, eye_tip, true).clicked() {
                act = Some(LayerAction::Visible(i, !l.visible));
            }
            let (pr, pr_tip) = if l.printable {
                ("print-on", tr("Imprimível", "Printable"))
            } else {
                (
                    "print-off",
                    tr("Não imprime (só tela)", "Not printed (screen only)"),
                )
            };
            if icons::small_button(ui, pr, pr_tip, true).clicked() {
                act = Some(LayerAction::Printable(i, !l.printable));
            }
            let (lk, lk_tip) = if l.locked {
                (
                    "lock-on",
                    tr("Bloqueada (não edita)", "Locked (not editable)"),
                )
            } else {
                ("lock-off", tr("Editável", "Editable"))
            };
            if icons::small_button(ui, lk, lk_tip, true).clicked() {
                act = Some(LayerAction::Locked(i, !l.locked));
            }
            let mark = if sel_layers.contains(&i) { " •" } else { "" };
            let label = format!("{}  ({}){mark}", l.name, l.objects.len());
            let text = if l.visible {
                egui::RichText::new(label)
            } else {
                egui::RichText::new(label).weak()
            };
            if ui
                .selectable_label(chosen == Some(i), text)
                .on_hover_text(tr(
                    "Camada ativa: recebe os objetos novos · • = tem objetos selecionados",
                    "Active layer: receives new objects · • = has selected objects",
                ))
                .clicked()
            {
                act = Some(LayerAction::Choose(l.id.clone()));
            }
        });
        let (_, head, _) = header.body(|ui| tree.layer_objects(ui, doc, i, &mut obj_act));
        tree.layer_spot(head.response.rect, page, i);
    }
    tree.finish(ui, &mut obj_act);
    if let Some(a) = obj_act {
        crate::layertree::apply(doc, a);
    }
    ui.separator();
    ui.horizontal_wrapped(|ui| {
        if icons::button(
            ui,
            "layer-add",
            tr(
                "Nova camada acima da ativa",
                "New layer above the active one",
            ),
            false,
            true,
        )
        .clicked()
        {
            act = Some(LayerAction::Add);
        }
        if let Some(c) = chosen {
            if icons::button(
                ui,
                "order-forward",
                tr("Subir camada", "Move layer up"),
                false,
                c + 1 < n,
            )
            .clicked()
            {
                act = Some(LayerAction::Move(c, c + 1));
            }
            if icons::button(
                ui,
                "order-backward",
                tr("Descer camada", "Move layer down"),
                false,
                c > 0,
            )
            .clicked()
            {
                act = Some(LayerAction::Move(c, c - 1));
            }
            if icons::button(
                ui,
                "layer-move-to",
                tr(
                    "Mover a seleção para esta camada",
                    "Move selection to this layer",
                ),
                false,
                !doc.selection.is_empty(),
            )
            .clicked()
            {
                act = Some(LayerAction::MoveSelection(c));
            }
            let wf = doc.page().layers[c].wireframe;
            if icons::button(
                ui,
                "wireframe",
                tr(
                    "Esboço: mostra só os contornos finos (não afeta a impressão)",
                    "Wireframe: shows thin outlines only (printing unaffected)",
                ),
                wf,
                true,
            )
            .clicked()
            {
                act = Some(LayerAction::Wireframe(c, !wf));
            }
            if icons::button(
                ui,
                "delete",
                tr(
                    "Excluir camada (e seus objetos)",
                    "Delete layer (and its objects)",
                ),
                false,
                n > 1,
            )
            .clicked()
            {
                act = Some(LayerAction::Delete(c));
            }
        }
    });
    if let Some(c) = chosen {
        let l = &doc.page().layers[c];
        let id = ui.scope_id().with(("nome-camada", l.id.clone()));
        let mut buf: String = ui
            .data_mut(|d| d.get_temp::<String>(id))
            .unwrap_or_else(|| l.name.clone());
        ui.horizontal(|ui| {
            ui.label(tr("Nome", "Name"));
            let r = ui.text_edit_singleline(&mut buf);
            if r.has_focus() {
                ui.data_mut(|d| d.insert_temp(id, buf.clone()));
            } else {
                ui.data_mut(|d| d.remove::<String>(id));
            }
            if r.lost_focus() && !buf.trim().is_empty() && buf != l.name {
                act = Some(LayerAction::Rename(c, buf.trim().to_string()));
            }
        });
    }
    let Some(act) = act else { return };
    let ed = &mut doc.editor;
    match act {
        LayerAction::Color(i, c) => ed.transaction(tr("Cor da camada", "Layer color"), |ed| {
            ed.edit_layer(page, i, |l| l.color = c)
        }),
        LayerAction::Visible(i, v) => ed.transaction(
            if v {
                tr("Mostrar camada", "Show layer")
            } else {
                tr("Ocultar camada", "Hide layer")
            },
            |ed| ed.edit_layer(page, i, |l| l.visible = v),
        ),
        LayerAction::Printable(i, v) => ed
            .transaction(tr("Camada imprimível", "Layer printable"), |ed| {
                ed.edit_layer(page, i, |l| l.printable = v)
            }),
        LayerAction::Locked(i, v) => ed.transaction(
            if v {
                tr("Bloquear camada", "Lock layer")
            } else {
                tr("Desbloquear camada", "Unlock layer")
            },
            |ed| ed.edit_layer(page, i, |l| l.locked = v),
        ),
        LayerAction::Wireframe(i, v) => ed
            .transaction(tr("Modo esboço da camada", "Layer wireframe"), |ed| {
                ed.edit_layer(page, i, |l| l.wireframe = v)
            }),
        LayerAction::Rename(i, name) => ed
            .transaction(tr("Renomear camada", "Rename layer"), |ed| {
                ed.edit_layer(page, i, |l| l.name = name)
            }),
        LayerAction::Choose(id) => doc.layer_id = Some(id),
        LayerAction::Add => {
            let layer =
                graphdraw_core::model::Layer::new(&format!("{} {}", tr("Camada", "Layer"), n + 1));
            doc.layer_id = Some(layer.id.clone());
            let at = chosen.map_or(n, |c| c + 1);
            ed.transaction(tr("Nova camada", "New layer"), |ed| {
                ed.insert_layer(page, at, layer)
            });
        }
        LayerAction::Delete(i) => {
            ed.transaction(tr("Excluir camada", "Delete layer"), |ed| {
                ed.remove_layer(page, i);
            });
            doc.layer_id = None;
            doc.prune_selection();
        }
        LayerAction::Move(from, to) => ed.move_layer(page, from, to),
        LayerAction::MoveSelection(i) => {
            let sel = doc.selection.clone();
            ops::move_to_layer(ed, &sel, page, i);
        }
    }
}

/// Relatório de importação (GD-CDR-03, GD-UI-02).
fn report_panel(ui: &mut egui::Ui, doc: &mut DocState) {
    let r = &doc.report;
    ui.label(format!(
        "{}: {}",
        tr("Formato", "Format"),
        r.source_format.to_uppercase()
    ));
    if let Some(v) = &r.detected_version {
        ui.label(format!(
            "{}: {v}",
            tr("Versão detectada", "Detected version")
        ));
    }
    ui.separator();
    if r.items.is_empty() {
        ui.label(tr("Nenhum aviso.", "No warnings."));
    }
    let mut goto = None;
    for item in &r.items {
        let (tag, color) = match item.severity {
            Severity::Info => (tr("info", "info"), ui.visuals().weak_text_color()),
            Severity::Warning => (tr("aviso", "warning"), theme::ACCENT_WARNING),
            Severity::Error => (tr("erro", "error"), theme::ACCENT_DANGER),
        };
        ui.horizontal_wrapped(|ui| {
            ui.colored_label(color, format!("[{tag}]"));
            if let Some(p) = item.page
                && ui
                    .small_button(format!("{} {p}", tr("pág.", "p.")))
                    .clicked()
            {
                goto = Some(p - 1);
            }
            let count = if item.count > 1 {
                format!(" (×{})", item.count)
            } else {
                String::new()
            };
            ui.label(format!("{}{count}", item.message));
        });
    }
    if let Some(p) = goto {
        doc.set_page(p);
    }
}

/// Opções do texto em caminho (GD-TXT-09), quando o primeiro texto alvo
/// corre num caminho: posição, distância, lado, orientação e espelho.
/// Idiomas oferecidos (hifenização e corretor).
pub(crate) const LANGS: [(&str, &str); 6] = [
    ("pt-BR", "Português (Brasil)"),
    ("pt-PT", "Português (Portugal)"),
    ("en-US", "English (US)"),
    ("en-GB", "English (UK)"),
    ("es-ES", "Español"),
    ("fr-FR", "Français"),
];

/// Hifenização e idioma do texto de parágrafo (GD-TXT-07).
fn hyphen_bar(ui: &mut egui::Ui, doc: &mut DocState, targets: &[String]) {
    let Some((on, lang)) = targets
        .first()
        .and_then(|id| doc.editor.find(id))
        .and_then(|r| doc.editor.get(&r))
        .and_then(|o| match &o.kind {
            ObjectKind::Text(t) if t.text_kind == "paragraph" => t.paragraphs.first().map(|p| {
                (
                    p.hyphenate,
                    p.lang.clone().unwrap_or_else(|| "pt-BR".into()),
                )
            }),
            _ => None,
        })
    else {
        return;
    };
    ui.separator();
    let (mut h, mut l) = (on, lang.clone());
    ui.checkbox(&mut h, tr("Hifenizar", "Hyphenate")).on_hover_text(tr(
        "Parte as palavras no fim da linha pelas regras do idioma (ou pelo dicionário em %APPDATA%\\GraphDraw\\hifenizacao)",
        "Splits words at the end of the line by the language rules (or the dictionary in %APPDATA%\\GraphDraw\\hifenizacao)",
    ));
    egui::ComboBox::from_id_salt("texto-idioma")
        .selected_text(&l)
        .width(theme::UNIT_COMBO_WIDTH + theme::SPACE_400)
        .show_ui(ui, |ui| {
            for (k, name) in LANGS {
                ui.selectable_value(&mut l, k.to_string(), format!("{k} — {name}"));
            }
        })
        .response
        .on_hover_text(tr("Idioma do texto", "Text language"));
    if h != on || l != lang {
        ops::modify(
            &mut doc.editor,
            targets,
            tr("Hifenização", "Hyphenation"),
            |o| {
                if let ObjectKind::Text(t) = &mut o.kind {
                    for p in &mut t.paragraphs {
                        p.hyphenate = h;
                        p.lang = (l != "pt-BR").then(|| l.clone());
                    }
                    return true;
                }
                false
            },
        );
    }
}

fn path_text_bar(ui: &mut egui::Ui, doc: &mut DocState, targets: &[String]) {
    let Some(tp) = targets
        .first()
        .and_then(|id| doc.editor.find(id))
        .and_then(|r| doc.editor.get(&r))
        .and_then(|o| match &o.kind {
            ObjectKind::Text(t) => t.path.clone(),
            _ => None,
        })
    else {
        return;
    };
    ui.separator();
    let mut next = tp.clone();
    widgets::stack(ui, |ui, row| {
        if row == 0 {
            if let Some(v) = widgets::labeled_length(
                ui,
                tr("Início", "Start"),
                "texto-caminho-inicio",
                next.offset,
                None,
            ) {
                next.offset = v;
            }
            if let Some(v) = widgets::labeled_length(
                ui,
                tr("Dist.", "Dist."),
                "texto-caminho-dist",
                next.distance,
                None,
            ) {
                next.distance = v;
            }
        } else {
            ui.selectable_value(&mut next.side, "above".to_string(), tr("Acima", "Above"));
            ui.selectable_value(&mut next.side, "below".to_string(), tr("Abaixo", "Below"));
            egui::ComboBox::from_id_salt("texto-caminho-orientacao")
                .selected_text(match next.orientation.as_str() {
                    "skew-v" => tr("Inclinar vertical", "Skew vertical"),
                    "skew-h" => tr("Inclinar horizontal", "Skew horizontal"),
                    "upright" => tr("De pé", "Upright"),
                    _ => tr("Girar letras", "Rotate letters"),
                })
                .width(theme::BLEND_COMBO_WIDTH)
                .show_ui(ui, |ui| {
                    for (k, l) in [
                        ("rotate", tr("Girar letras", "Rotate letters")),
                        ("skew-v", tr("Inclinar vertical", "Skew vertical")),
                        ("skew-h", tr("Inclinar horizontal", "Skew horizontal")),
                        ("upright", tr("De pé", "Upright")),
                    ] {
                        ui.selectable_value(&mut next.orientation, k.to_string(), l);
                    }
                });
            ui.checkbox(&mut next.mirror, tr("Espelhar", "Mirror"));
        }
    });
    if next != tp {
        let ids = targets[..1].to_vec();
        ops::edit_text_path(&mut doc.editor, &ids, move |p| *p = next.clone());
    }
}

/// Barra do preenchimento interativo: tipo do gradiente do objeto e dicas.
fn gradient_bar(ui: &mut egui::Ui, doc: &mut DocState) {
    use graphdraw_core::edit::ops::{gradient_spec, set_gradient};
    let current = match doc.selection.as_slice() {
        [id] => doc
            .editor
            .find(id)
            .and_then(|r| doc.editor.get(&r))
            .and_then(|o| match &o.fill {
                Some(Fill::Gradient(g)) => Some(g.clone()),
                _ => None,
            }),
        _ => None,
    };
    if let Some(g) = &current {
        for (kind, label) in [
            ("linear", tr("Linear", "Linear")),
            ("radial", tr("Radial", "Radial")),
        ] {
            if ui.selectable_label(g.kind == kind, label).clicked() && g.kind != kind {
                let mut spec = gradient_spec(g);
                spec.kind = kind.into();
                let sel = doc.selection.clone();
                set_gradient(&mut doc.editor, &sel, &spec);
            }
        }
        ui.separator();
    }
    widgets::hint_line(
        ui,
        tr(
            "Arraste sobre o objeto para criar · arraste alças e paradas · duplo clique na linha cria parada · Delete tira a parada · Ctrl = ângulo em 15°",
            "Drag over the object to create · drag handles and stops · double-click the line to add a stop · Delete removes the stop · Ctrl = 15° angle steps",
        ),
    );
}

/// Nome do modo de mesclagem na interface.
fn blend_label(b: graphdraw_core::model::BlendMode) -> &'static str {
    use graphdraw_core::model::BlendMode as B;
    match b {
        B::Normal => tr("Normal", "Normal"),
        B::Multiply => tr("Multiplicar", "Multiply"),
        B::Screen => tr("Tela", "Screen"),
        B::Overlay => tr("Sobrepor", "Overlay"),
        B::Darken => tr("Escurecer", "Darken"),
        B::Lighten => tr("Clarear", "Lighten"),
        B::ColorDodge => tr("Subexposição de cor", "Color dodge"),
        B::ColorBurn => tr("Superexposição de cor", "Color burn"),
        B::HardLight => tr("Luz forte", "Hard light"),
        B::SoftLight => tr("Luz suave", "Soft light"),
        B::Difference => tr("Diferença", "Difference"),
        B::Exclusion => tr("Exclusão", "Exclusion"),
        B::Hue => tr("Matiz", "Hue"),
        B::Saturation => tr("Saturação", "Saturation"),
        B::Color => tr("Cor", "Color"),
        B::Luminosity => tr("Luminosidade", "Luminosity"),
    }
}

/// Aparência dos objetos selecionados: opacidade do objeto inteiro e modo
/// de mesclagem (GD-TRA-02). Cada mudança é um passo de desfazer.
fn look_section(ui: &mut egui::Ui, doc: &mut DocState) {
    let sel = doc.selection.clone();
    let Some((opacity, blend)) = doc.selected_objects().first().map(|o| (o.opacity, o.blend))
    else {
        return;
    };
    ui.separator();
    egui::CollapsingHeader::new(tr("Aparência", "Appearance"))
        .id_salt("aparencia-objeto")
        .default_open(true)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(tr("Opacidade", "Opacity"));
                let mut pct = (opacity.unwrap_or(1.0) * 100.0).round();
                let r = ui.add(
                    egui::DragValue::new(&mut pct)
                        .range(0.0..=100.0)
                        .suffix("%"),
                );
                // Arrastar o valor é um passo só de desfazer.
                if r.drag_started() {
                    doc.editor
                        .begin(tr("Opacidade do objeto", "Object opacity"));
                }
                if r.changed() {
                    ops::set_opacity(&mut doc.editor, &sel, pct / 100.0);
                }
                if r.drag_stopped() {
                    doc.editor.commit();
                }
            });
            ui.horizontal(|ui| {
                ui.label(tr("Mesclagem", "Blend"));
                egui::ComboBox::from_id_salt("modo-mesclagem")
                    .selected_text(blend_label(blend))
                    .width(theme::BLEND_COMBO_WIDTH)
                    .show_ui(ui, |ui| {
                        for b in graphdraw_core::model::BlendMode::ALL {
                            if ui.selectable_label(blend == b, blend_label(b)).clicked() {
                                ops::set_blend(&mut doc.editor, &sel, b);
                            }
                        }
                    });
            });
        });
}

/// Dados do objeto (GD-OBJ-05): pares chave/valor do objeto selecionado
/// sozinho (orçamento, código de produção). Cada alteração é um passo.
fn user_data_section(ui: &mut egui::Ui, doc: &mut DocState) {
    let [id] = doc.selection.as_slice() else {
        return;
    };
    let id = id.clone();
    let Some(data) = doc
        .editor
        .find(&id)
        .and_then(|r| doc.editor.get(&r))
        .map(|o| o.user_data.clone().unwrap_or_default())
    else {
        return;
    };
    ui.separator();
    let mut next: Option<std::collections::BTreeMap<String, String>> = None;
    egui::CollapsingHeader::new(tr("Dados do objeto", "Object data"))
        .id_salt("dados-objeto")
        .default_open(true)
        .show(ui, |ui| {
            egui::Grid::new("dados-objeto-grade")
                .num_columns(3)
                .spacing([theme::SPACE_100, theme::SPACE_050])
                .show(ui, |ui| {
                    for (k, v) in &data {
                        ui.label(k);
                        // Texto em edição fica guardado até sair do campo.
                        let buf_id = ui.make_persistent_id(("dado", &id, k));
                        let mut val = ui
                            .data(|d| d.get_temp::<String>(buf_id))
                            .unwrap_or_else(|| v.clone());
                        let r = ui.add(
                            egui::TextEdit::singleline(&mut val)
                                .desired_width(theme::USER_DATA_FIELD_WIDTH),
                        );
                        if r.changed() {
                            ui.data_mut(|d| d.insert_temp(buf_id, val.clone()));
                        }
                        if r.lost_focus() {
                            ui.data_mut(|d| d.remove::<String>(buf_id));
                            if val != *v {
                                let mut m = data.clone();
                                m.insert(k.clone(), val);
                                next = Some(m);
                            }
                        }
                        if icons::button(
                            ui,
                            "delete",
                            tr("Tirar este dado", "Remove this entry"),
                            false,
                            true,
                        )
                        .clicked()
                        {
                            let mut m = data.clone();
                            m.remove(k);
                            next = Some(m);
                        }
                        ui.end_row();
                    }
                });
            // Novo par.
            let new_id = ui.make_persistent_id(("dado-novo", &id));
            let (mut key, mut val) = ui
                .data(|d| d.get_temp::<(String, String)>(new_id))
                .unwrap_or_default();
            let mut add = false;
            ui.horizontal(|ui| {
                let k = ui.add(
                    egui::TextEdit::singleline(&mut key)
                        .hint_text(tr("chave", "key"))
                        .desired_width(theme::USER_DATA_FIELD_WIDTH),
                );
                let v = ui.add(
                    egui::TextEdit::singleline(&mut val)
                        .hint_text(tr("valor", "value"))
                        .desired_width(theme::USER_DATA_FIELD_WIDTH),
                );
                let enter = (k.lost_focus() || v.lost_focus())
                    && ui.input(|i| i.key_pressed(egui::Key::Enter));
                add = icons::button(
                    ui,
                    "layer-add",
                    tr("Adicionar dado", "Add entry"),
                    false,
                    !key.trim().is_empty(),
                )
                .clicked()
                    || (enter && !key.trim().is_empty());
            });
            if add {
                let mut m = data.clone();
                m.insert(key.trim().to_string(), val.clone());
                next = Some(m);
                ui.data_mut(|d| d.remove::<(String, String)>(new_id));
            } else {
                ui.data_mut(|d| d.insert_temp(new_id, (key, val)));
            }
        });
    if let Some(m) = next {
        ops::set_user_data(&mut doc.editor, &id, m);
    }
}

/// Barra do conta-gotas (doc 27): modo, raio ou filtros, o balde e a
/// dica dos gestos.
fn eyedrop_bar(ui: &mut egui::Ui, e: &mut crate::eyedrop::Eyedrop) {
    use crate::eyedrop::{Bucket, Mode, Radius};
    use graphdraw_core::edit::eyedrop::Paint;
    for (m, label) in [
        (Mode::Color, tr("Cor", "Color")),
        (Mode::Attributes, tr("Atributos", "Attributes")),
    ] {
        if ui.selectable_label(e.mode == m, label).clicked() && e.mode != m {
            e.mode = m;
            e.empty();
        }
    }
    ui.separator();
    match e.mode {
        Mode::Color => {
            egui::ComboBox::from_id_salt("conta-gotas-raio")
                .selected_text(e.radius.label())
                .show_ui(ui, |ui| {
                    for r in Radius::ALL {
                        ui.selectable_value(&mut e.radius, r, r.label());
                    }
                });
            ui.add_enabled(
                false,
                egui::Button::new(tr("Fora da janela", "Outside the window")),
            )
            .on_disabled_hover_text(tr(
                "Beta: amostrar fora da janela precisa de captura de tela do sistema",
                "Beta: sampling outside the window needs system screen capture",
            ));
        }
        Mode::Attributes => {
            let f = &mut e.filter;
            ui.toggle_value(&mut f.fill, tr("Preenchimento", "Fill"));
            ui.toggle_value(&mut f.stroke, tr("Contorno", "Outline"));
            ui.add_enabled(false, egui::Button::new(tr("Efeitos", "Effects")))
                .on_disabled_hover_text(tr(
                    "Beta: o documento ainda não tem efeitos (sombra, brilho, extrusão)",
                    "Beta: the document has no effects yet (shadow, glow, extrude)",
                ));
            ui.toggle_value(&mut f.typography, tr("Tipografia", "Typography"));
            ui.toggle_value(&mut f.transform, tr("Transformações", "Transformations"));
        }
    }
    ui.separator();
    match &e.bucket {
        Some(Bucket::Paint(Paint::Color(c))) => {
            widgets::swatch(ui, Some(c), theme::EYEDROP_DOT);
        }
        Some(Bucket::Paint(Paint::Ramp(..))) => {
            ui.label(tr("Rampa", "Ramp"));
        }
        Some(Bucket::Attrs(o)) => {
            ui.label(
                o.name
                    .clone()
                    .unwrap_or_else(|| tr("Atributos", "Attributes").into()),
            );
        }
        None => {}
    }
    if e.bucket.is_some()
        && ui
            .button(tr("Esvaziar", "Empty"))
            .on_hover_text(tr("Esvaziar o balde (Esc)", "Empty the bucket (Esc)"))
            .clicked()
    {
        e.empty();
    }
    widgets::hint_line(
        ui,
        tr(
            "Clique amostra e vira balde · clique aplica (direito: contorno) · Shift reamostra · Alt alterna · Ctrl dentro de grupos · Alt+clique no gradiente clona a rampa · Esc esvazia",
            "Click samples and becomes a bucket · click applies (right: outline) · Shift resamples · Alt toggles · Ctrl inside groups · Alt+click on a gradient clones the ramp · Esc empties",
        ),
    );
}

/// Barra da ferramenta Cota (GD-COT-01/03): tipo e formato. Com cotas
/// selecionadas, o formato vale para elas (um passo de desfazer); senão,
/// para as próximas.
fn dimension_bar(ui: &mut egui::Ui, doc: &mut DocState, cs: &mut crate::canvas::CanvasState) {
    use graphdraw_core::model::DimFormat;
    for (k, label) in [
        ("horizontal", tr("Horizontal", "Horizontal")),
        ("vertical", tr("Vertical", "Vertical")),
        ("aligned", tr("Paralela", "Aligned")),
        ("angular", tr("Angular", "Angular")),
        ("radius", tr("Raio", "Radius")),
        ("diameter", tr("Diâmetro", "Diameter")),
        ("segment", tr("Segmento", "Segment")),
    ] {
        if ui.selectable_label(cs.dim_kind == k, label).clicked() && cs.dim_kind != k {
            cs.dim_kind = k.into();
            cs.dim_draft = None;
        }
    }
    ui.separator();
    let selected: Vec<String> = doc
        .selection
        .iter()
        .filter(|id| {
            doc.editor
                .find(id)
                .and_then(|r| doc.editor.get(&r))
                .is_some_and(|o| matches!(o.kind, ObjectKind::Dimension(_)))
        })
        .cloned()
        .collect();
    let mut f: DimFormat = selected
        .first()
        .and_then(|id| doc.editor.find(id).and_then(|r| doc.editor.get(&r)))
        .and_then(|o| match &o.kind {
            ObjectKind::Dimension(d) => Some(d.format.clone()),
            _ => None,
        })
        .unwrap_or_else(|| cs.dim_format.clone());
    let before = f.clone();
    egui::ComboBox::from_id_salt("cota-unidade")
        .selected_text(f.unit.clone())
        .show_ui(ui, |ui| {
            for u in ["mm", "cm", "m", "in", "pt"] {
                ui.selectable_value(&mut f.unit, u.into(), u);
            }
        });
    ui.add(
        egui::DragValue::new(&mut f.precision)
            .range(0..=6)
            .prefix(tr("casas ", "decimals ")),
    );
    ui.checkbox(&mut f.show_unit, tr("Unidade", "Unit"));
    let mut size_pt = f.text_size / graphdraw_core::units::UM_PER_PT;
    if ui
        .add(
            egui::DragValue::new(&mut size_pt)
                .range(2.0..=144.0)
                .speed(0.2)
                .suffix(" pt"),
        )
        .changed()
    {
        f.text_size = size_pt * graphdraw_core::units::UM_PER_PT;
    }
    egui::ComboBox::from_id_salt("cota-ponta")
        .selected_text(crate::dialogs::arrow_label(&f.arrow))
        .show_ui(ui, |ui| {
            for a in ["triangle", "open", "bar", "circle", "none"] {
                let label = if a == "none" {
                    tr("Sem ponta", "No arrow")
                } else {
                    crate::dialogs::arrow_label(a)
                };
                ui.selectable_value(&mut f.arrow, a.into(), label);
            }
        });
    ui.checkbox(&mut f.extension_lines, tr("Extensão", "Extension"));
    let mut inline = f.text_position == "inline";
    if ui
        .checkbox(&mut inline, tr("Texto sobre a linha", "Text on the line"))
        .changed()
    {
        f.text_position = if inline { "inline" } else { "above" }.into();
    }
    if f != before {
        if selected.is_empty() {
            cs.dim_format = f;
        } else if !widgets::field_dragging(ui.ctx()) {
            ops::modify(
                &mut doc.editor,
                &selected,
                tr("Formato da cota", "Dimension format"),
                |o| {
                    if let ObjectKind::Dimension(d) = &mut o.kind {
                        d.format = f.clone();
                        return true;
                    }
                    false
                },
            );
        }
    }
}

/// Booleana não destrutiva selecionada: trocar a operação.
fn live_boolean_bar(ui: &mut egui::Ui, doc: &mut DocState) {
    let [gid] = doc.selection.as_slice() else {
        return;
    };
    let gid = gid.clone();
    let Some(ObjectKind::Group(g)) = doc
        .editor
        .find(&gid)
        .and_then(|r| doc.editor.get(&r))
        .map(|o| &o.kind)
    else {
        return;
    };
    let Some(cur) = g.boolean.clone() else {
        return;
    };
    ui.separator();
    let mut next = cur.clone();
    for (k, label) in [
        ("weld", tr("Soldar", "Weld")),
        ("trim", tr("Aparar", "Trim")),
        ("intersect", tr("Interseção", "Intersect")),
    ] {
        if ui
            .selectable_label(next == k, label)
            .on_hover_text(tr(
                "Booleana ao vivo: as formas continuam editáveis (Ctrl+clique); Converter em curvas fecha o resultado; Desagrupar devolve as formas",
                "Live boolean: the shapes stay editable (Ctrl+click); Convert to curves closes the result; Ungroup gives the shapes back",
            ))
            .clicked()
        {
            next = k.into();
        }
    }
    if next != cur {
        ops::modify(
            &mut doc.editor,
            std::slice::from_ref(&gid),
            tr("Booleana", "Boolean"),
            |o| {
                if let ObjectKind::Group(g) = &mut o.kind {
                    g.boolean = Some(next.clone());
                    return true;
                }
                false
            },
        );
    }
}

/// Contornar texto (GD-TXT-10): com objetos (não textos) selecionados, o
/// estilo, o lado e a distância com que afastam o texto de parágrafo.
fn wrap_bar(ui: &mut egui::Ui, doc: &mut DocState) {
    let sel: Vec<String> = doc
        .selection
        .iter()
        .filter(|id| {
            doc.editor
                .find(id)
                .and_then(|r| doc.editor.get(&r))
                .is_some_and(|o| !matches!(o.kind, ObjectKind::Text(_)))
        })
        .cloned()
        .collect();
    let Some(first) = sel
        .first()
        .and_then(|id| doc.editor.find(id))
        .and_then(|r| doc.editor.get(&r))
    else {
        return;
    };
    let cur = first.wrap.clone();
    let mut next = cur.clone();
    ui.separator();
    ui.menu_button(tr("Contornar texto", "Wrap text"), |ui| {
        let style = next.as_ref().map(|w| w.style.clone());
        for (k, label) in [
            (None, tr("Nenhum", "None")),
            (Some("contour"), tr("Contorno", "Contour")),
            (Some("square"), tr("Quadrado", "Square")),
        ] {
            if ui.radio(style.as_deref() == k, label).clicked() {
                next = k.map(|k| graphdraw_core::model::TextWrap {
                    style: k.into(),
                    side: next.as_ref().map_or("both".into(), |w| w.side.clone()),
                    distance: next.as_ref().map_or(2_000.0, |w| w.distance),
                });
            }
        }
        if let Some(w) = &mut next {
            ui.separator();
            for (k, label) in [
                ("both", tr("Dos dois lados", "Both sides")),
                ("left", tr("À esquerda", "Left")),
                ("right", tr("À direita", "Right")),
                ("above-below", tr("Acima e abaixo", "Above and below")),
            ] {
                if ui.radio(w.side == k, label).clicked() {
                    w.side = k.into();
                }
            }
            ui.horizontal(|ui| {
                ui.label(tr("Distância", "Distance"));
                if let Some(v) = widgets::length_field(ui, "contornar-dist", w.distance, None) {
                    w.distance = v.max(0.0);
                }
            });
        }
    });
    if next != cur {
        ops::modify(
            &mut doc.editor,
            &sel,
            tr("Contornar texto", "Wrap text"),
            |o| {
                o.wrap = next.clone();
                true
            },
        );
    }
}

/// Controles da simetria (GD-FER-33) com um grupo de simetria selecionado:
/// tipo, número de eixos/cópias, ângulo, desenhar dentro e assar.
fn symmetry_bar(ui: &mut egui::Ui, doc: &mut DocState) {
    let [gid] = doc.selection.as_slice() else {
        return;
    };
    let gid = gid.clone();
    let Some(ObjectKind::Group(g)) = doc
        .editor
        .find(&gid)
        .and_then(|r| doc.editor.get(&r))
        .map(|o| &o.kind)
    else {
        return;
    };
    let Some(s0) = g.symmetry.clone() else {
        return;
    };
    let mut s = s0.clone();
    ui.separator();
    for (k, label) in [
        ("mirror", tr("Espelho", "Mirror")),
        ("radial", tr("Radial", "Radial")),
    ] {
        if ui.selectable_label(s.kind == k, label).clicked() {
            s.kind = k.into();
        }
    }
    ui.label(if s.kind == "mirror" {
        tr("Eixos", "Axes")
    } else {
        tr("Cópias", "Copies")
    });
    ui.add(egui::DragValue::new(&mut s.count).range(1..=32));
    ui.label(tr("Ângulo", "Angle"));
    ui.add(
        egui::DragValue::new(&mut s.angle)
            .range(-360.0..=360.0)
            .suffix("°")
            .speed(1.0),
    );
    if s != s0 {
        let s = s.clone();
        ops::modify(
            &mut doc.editor,
            std::slice::from_ref(&gid),
            tr("Simetria", "Symmetry"),
            |o| {
                if let ObjectKind::Group(g) = &mut o.kind {
                    g.symmetry = Some(s.clone());
                    return true;
                }
                false
            },
        );
    }
    let mut inside = doc.symmetry.as_deref() == Some(gid.as_str());
    if ui
        .checkbox(&mut inside, tr("Desenhar dentro", "Draw inside"))
        .on_hover_text(tr(
            "O que for desenhado entra na simetria e ganha as cópias",
            "What you draw goes into the symmetry and gets the copies",
        ))
        .changed()
    {
        doc.symmetry = inside.then(|| gid.clone());
    }
    if ui
        .button(tr("Assar", "Bake"))
        .on_hover_text(tr(
            "Desagrupa, transformando as cópias em objetos",
            "Ungroups, turning the copies into objects",
        ))
        .clicked()
    {
        doc.selection = ops::ungroup(&mut doc.editor, std::slice::from_ref(&gid));
        if doc.symmetry.as_deref() == Some(gid.as_str()) {
            doc.symmetry = None;
        }
    }
}

/// Barra do Conector (GD-CON-01): estilo, raio dos cantos e seta no fim;
/// com conectores selecionados, vale para eles.
fn connector_bar(ui: &mut egui::Ui, doc: &mut DocState, cs: &mut crate::canvas::CanvasState) {
    let sel: Vec<String> = doc
        .selection
        .iter()
        .filter(|id| {
            doc.editor
                .find(id)
                .and_then(|r| doc.editor.get(&r))
                .is_some_and(|o| matches!(o.kind, ObjectKind::Connector(_)))
        })
        .cloned()
        .collect();
    let mut style = cs.conn_style.clone();
    let mut radius = cs.conn_radius;
    if let Some(ObjectKind::Connector(c)) = sel
        .first()
        .and_then(|id| doc.editor.find(id).and_then(|r| doc.editor.get(&r)))
        .map(|o| &o.kind)
    {
        style = c.style.clone();
        radius = c.radius;
    }
    let (s0, r0) = (style.clone(), radius);
    for (k, label) in [
        ("straight", tr("Reto", "Straight")),
        ("elbow", tr("Ângulo reto", "Elbow")),
        ("curved", tr("Curvo", "Curved")),
    ] {
        if ui.selectable_label(style == k, label).clicked() {
            style = k.into();
        }
    }
    if style == "elbow" {
        ui.label(tr("Cantos", "Corners"));
        if let Some(v) = widgets::length_field(ui, "conector-raio", radius, None) {
            radius = v.max(0.0);
        }
    }
    ui.checkbox(&mut cs.conn_arrow, tr("Seta no fim", "Arrow at the end"));
    if style != s0 || radius != r0 {
        if sel.is_empty() {
            cs.conn_style = style;
            cs.conn_radius = radius;
        } else {
            ops::modify(
                &mut doc.editor,
                &sel,
                tr("Estilo do conector", "Connector style"),
                |o| {
                    if let ObjectKind::Connector(c) = &mut o.kind {
                        c.style = style.clone();
                        c.radius = radius;
                        return true;
                    }
                    false
                },
            );
        }
    }
    ui.separator();
    widgets::hint_line(
        ui,
        tr(
            "Arraste de um objeto a outro; as pontas encaixam nos cantos, lados e centro",
            "Drag from one object to another; the ends snap to corners, sides and center",
        ),
    );
}

/// Texto do problema da verificação prévia (GD-PRE-01).
pub(crate) fn issue_text(k: &graphdraw_core::preflight::IssueKind) -> String {
    use graphdraw_core::preflight::{IssueKind, PT};
    match k {
        IssueKind::Ink { tac, stroke } => format!(
            "{} {:.0}% {}",
            tr("Tinta", "Ink"),
            tac,
            if *stroke {
                tr("no contorno", "in the outline")
            } else {
                tr("no preenchimento", "in the fill")
            }
        ),
        IssueKind::ThinLine { width, min } => format!(
            "{} {:.2} pt ({} {:.2} pt)",
            tr("Linha fina:", "Thin line:"),
            width / PT,
            tr("mínimo", "minimum"),
            min / PT
        ),
        IssueKind::SafeZone => tr(
            "Texto fora da zona de segurança",
            "Text outside the safe zone",
        )
        .into(),
        IssueKind::NearEdge => tr(
            "Encostado no corte sem sangria",
            "Touching the trim without bleed",
        )
        .into(),
        IssueKind::BlackKnockout => tr(
            "Preto 100% K sem sobreimpressão",
            "100% K black without overprint",
        )
        .into(),
    }
}

/// Painel Verificação (GD-PRE-01/02): limites do documento e problemas da
/// página, com seleção e correção.
fn preflight_panel(ui: &mut egui::Ui, doc: &mut DocState) {
    use graphdraw_core::preflight::{self as pf, PT};
    let mut s = doc.doc().data.settings.preflight.clone();
    let before = s.clone();
    egui::CollapsingHeader::new(tr("Limites do documento", "Document limits"))
        .id_salt("verificacao-limites")
        .show(ui, |ui| {
            egui::Grid::new("verificacao-grade")
                .num_columns(2)
                .show(ui, |ui| {
                    ui.label(tr("Papel", "Paper"));
                    egui::ComboBox::from_id_salt("verificacao-papel")
                        .selected_text(if s.tac == pf::TAC_UNCOATED {
                            tr("Não revestido", "Uncoated")
                        } else if s.tac == pf::TAC_COATED {
                            tr("Revestido", "Coated")
                        } else {
                            tr("Personalizado", "Custom")
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut s.tac,
                                pf::TAC_COATED,
                                tr("Revestido (300%)", "Coated (300%)"),
                            );
                            ui.selectable_value(
                                &mut s.tac,
                                pf::TAC_UNCOATED,
                                tr("Não revestido (260%)", "Uncoated (260%)"),
                            );
                        });
                    ui.end_row();
                    ui.label(tr("Tinta máxima", "Max ink"));
                    ui.add(
                        egui::DragValue::new(&mut s.tac)
                            .range(100.0..=400.0)
                            .suffix(" %"),
                    );
                    ui.end_row();
                    let pt_field = |ui: &mut egui::Ui, label: &str, v: &mut f64| {
                        ui.label(label);
                        let mut p = *v / PT;
                        if ui
                            .add(
                                egui::DragValue::new(&mut p)
                                    .range(0.0..=72.0)
                                    .speed(0.05)
                                    .suffix(" pt"),
                            )
                            .changed()
                        {
                            *v = p * PT;
                        }
                        ui.end_row();
                    };
                    pt_field(ui, tr("Linha mínima", "Min line"), &mut s.min_line);
                    pt_field(
                        ui,
                        tr("Linha clara mínima", "Min light line"),
                        &mut s.min_line_negative,
                    );
                    pt_field(
                        ui,
                        tr("Texto preto até", "Black text up to"),
                        &mut s.black_text_size,
                    );
                    ui.label(tr("Zona de segurança", "Safe zone"));
                    if let Some(v) =
                        widgets::length_field(ui, "verificacao-zona", s.safe_zone, None)
                    {
                        s.safe_zone = v.max(0.0);
                    }
                    ui.end_row();
                    ui.label(tr("Encostado a menos de", "Touching within"));
                    if let Some(v) =
                        widgets::length_field(ui, "verificacao-borda", s.near_edge, None)
                    {
                        s.near_edge = v.max(0.0);
                    }
                    ui.end_row();
                });
        });
    if s != before && !widgets::field_dragging(ui.ctx()) {
        doc.editor.edit_settings(|st| st.preflight = s.clone());
    }
    ui.separator();
    let issues = doc.preflight();
    if issues.is_empty() {
        ui.colored_label(
            theme::ACCENT_SUCCESS,
            tr("Nenhum problema nesta página.", "No problems on this page."),
        );
        return;
    }
    let fixable = issues.iter().filter(|i| i.fixable()).count();
    let mut fix: Vec<pf::Issue> = Vec::new();
    ui.horizontal(|ui| {
        ui.label(format!(
            "{} {}",
            issues.len(),
            tr("problema(s)", "problem(s)")
        ));
        if fixable > 0 && ui.button(tr("Corrigir todos", "Fix all")).clicked() {
            fix = issues.iter().filter(|i| i.fixable()).cloned().collect();
        }
    });
    let mut select = None;
    egui::ScrollArea::vertical()
        .id_salt("verificacao-lista")
        .show(ui, |ui| {
            for issue in issues.iter() {
                let name = doc
                    .editor
                    .find(&issue.id)
                    .and_then(|r| doc.editor.get(&r))
                    .map(|o| {
                        o.name
                            .clone()
                            .unwrap_or_else(|| crate::app::kind_label(&o.kind))
                    })
                    .unwrap_or_default();
                ui.horizontal(|ui| {
                    ui.colored_label(theme::ACCENT_WARNING, "⚠");
                    if ui
                        .link(format!("{name}: {}", issue_text(&issue.kind)))
                        .on_hover_text(tr("Selecionar o objeto", "Select the object"))
                        .clicked()
                    {
                        select = Some(issue.id.clone());
                    }
                    if issue.fixable() && ui.small_button(tr("Corrigir", "Fix")).clicked() {
                        fix.push(issue.clone());
                    }
                });
            }
        });
    if let Some(id) = select {
        doc.selection = vec![id];
    }
    if !fix.is_empty() {
        let page = doc.page;
        doc.editor
            .transaction(tr("Corrigir verificação", "Fix preflight"), |ed| {
                for i in &fix {
                    pf::fix(ed, page, i);
                }
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Sem seleção, a barra de propriedades mostra a página e cabe numa linha.
    #[test]
    fn page_properties_show_without_selection() {
        let ctx = crate::theme::test_ctx();
        let mut doc = DocState::new_blank();
        graphdraw_core::edit::pages::add_after(&mut doc.editor, 0);
        let mut h = 0.0;
        for _ in 0..2 {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::Vec2::new(1920.0, 1080.0),
                )),
                ..Default::default()
            };
            let mut out = ctx.run_ui(input, |ui| {
                egui::Panel::top("propriedades").show(ui, |ui| {
                    let r = ui.horizontal_wrapped(|ui| page_bar_props(ui, &mut doc));
                    h = r.response.rect.height();
                });
            });
            out.textures_delta.clear();
        }
        assert!(h > 0.0 && h < 45.0, "uma linha: {h}");
        // Mudar o tamanho pela barra vale para todas as páginas.
        graphdraw_core::edit::pages::set_size_all(&mut doc.editor, 148_000.0, 210_000.0, None, "t");
        assert!(doc.doc().data.pages.iter().all(|p| p.width == 148_000.0));
    }

    /// Desenha a barra de propriedades em 1366 × 768 e devolve a altura do
    /// painel e a área ocupada pelos controles.
    fn bar(doc: &mut DocState, f: impl Fn(&mut egui::Ui, &mut DocState)) -> (f32, egui::Rect) {
        let ctx = crate::theme::test_ctx();
        let (mut panel, mut content) = (0.0, egui::Rect::NOTHING);
        for _ in 0..3 {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::Vec2::new(1366.0, 768.0),
                )),
                ..Default::default()
            };
            let mut out = ctx.run_ui(input, |ui| {
                let c = ui.ctx().clone();
                panel = widgets::property_panel(&c)
                    .show(ui, |ui| {
                        widgets::single_line_bar(ui, "t", |ui| {
                            f(ui, doc);
                            content = ui.min_rect();
                        });
                    })
                    .response
                    .rect
                    .height();
            });
            out.textures_delta.clear();
        }
        (panel, content)
    }

    /// Cantos: alterar com só alguns marcados mexe só neles; "Todos" = os 4.
    #[test]
    fn corners_change_only_selected_ones() {
        let mut cs = None;
        set_corners(&mut cs, &[true; 4], "round", 2_000.0);
        assert!(cs.as_ref().unwrap().iter().all(|c| c.radius == 2_000.0));
        // Só o superior direito (índice 2) vira chanfro de 5 mm.
        set_corners(&mut cs, &[false, false, true, false], "chamfer", 5_000.0);
        let c = cs.unwrap();
        assert_eq!((c[2].kind.as_str(), c[2].radius), ("chamfer", 5_000.0));
        for i in [0, 1, 3] {
            assert_eq!((c[i].kind.as_str(), c[i].radius), ("round", 2_000.0));
        }
    }

    /// Só mostrar a barra de seleção não altera o documento.
    #[test]
    fn showing_select_bar_does_not_modify() {
        use graphdraw_core::model::{Object, ObjectKind, RectData};
        let mut doc = DocState::new_blank();
        for k in 0..3 {
            let mut o = Object::new(ObjectKind::Rectangle(RectData {
                width: 10_000.0 + k as f64,
                height: 30_000.0,
                corners: None,
                extra: Default::default(),
            }));
            o.transform = graphdraw_core::model::Transform::translate(k as f64 * 20_000.0, 0.0);
            ops::add_object(&mut doc.editor, 0, 0, o, "t");
        }
        doc.editor.mark_saved();
        doc.selection = doc.page().layers[0]
            .objects
            .iter()
            .map(|o| o.id.clone())
            .collect();
        let rev = doc.editor.revision();
        let select = |ui: &mut egui::Ui, d: &mut DocState| {
            let (mut a, mut l) = (Anchor::Center, true);
            select_bar(ui, d, &mut a, &mut l, &mut [true; 4], &mut |_, _| {});
        };
        let _ = bar(&mut doc, select);
        assert_eq!(doc.editor.revision(), rev, "nada mudou");
        assert!(!doc.editor.is_modified());
    }

    /// O campo de ângulo mostra a rotação atual e girar até um valor
    /// deixa o objeto naquele ângulo.
    #[test]
    fn angle_field_shows_current_rotation() {
        use graphdraw_core::model::{Object, ObjectKind, RectData};
        let mut doc = DocState::new_blank();
        let o = Object::new(ObjectKind::Rectangle(RectData {
            width: 50_000.0,
            height: 30_000.0,
            corners: None,
            extra: Default::default(),
        }));
        let id = ops::add_object(&mut doc.editor, 0, 0, o, "t");
        let sel = vec![id];
        assert_eq!(selection_angle(&doc, &sel), 0.0);
        let c = kurbo::Point::new(25_000.0, 15_000.0);
        ops::transform(&mut doc.editor, &sel, ops::rotate_about(c, 30.0), "g");
        assert!((selection_angle(&doc, &sel) - 30.0).abs() < 1e-6);
        let cur = selection_angle(&doc, &sel);
        ops::transform(&mut doc.editor, &sel, ops::rotate_about(c, 90.0 - cur), "g");
        assert!((selection_angle(&doc, &sel) - 90.0).abs() < 1e-6);
        ops::transform(&mut doc.editor, &sel, ops::rotate_about(c, 100.0), "g");
        assert!(
            (selection_angle(&doc, &sel) + 170.0).abs() < 1e-6,
            "faixa −180..180"
        );
    }

    /// DESIGN-SYSTEM 4.6: em todo contexto (página, objeto selecionado,
    /// edição de nós) os controles cabem na altura da barra, sem nada
    /// cortado em cima ou embaixo.
    #[test]
    fn bar_contexts_fit_bar_height() {
        use graphdraw_core::model::{Object, ObjectKind, RectData};
        let mut doc = DocState::new_blank();
        let o = Object::new(ObjectKind::Rectangle(RectData {
            width: 50_000.0,
            height: 30_000.0,
            corners: None,
            extra: Default::default(),
        }));
        let id = ops::add_object(&mut doc.editor, 0, 0, o, "t");
        let fits = |(panel, content): (f32, egui::Rect), what: &str| {
            assert_eq!(panel, crate::theme::BAR_PROPERTY_HEIGHT, "{what}");
            assert!(
                // Duas linhas de controles + o vão entre elas (4.6).
                content.height()
                    <= 2.0 * crate::theme::BUTTON_COMPACT + crate::theme::BAR_ROW_GAP + 0.5,
                "{what}: {content:?}"
            );
        };
        let page = |ui: &mut egui::Ui, d: &mut DocState| {
            page_bar_props(ui, d);
        };
        fits(bar(&mut doc, page), "página");
        doc.selection = vec![id.clone()];
        let select = |ui: &mut egui::Ui, d: &mut DocState| {
            let (mut a, mut l) = (Anchor::Center, true);
            select_bar(ui, d, &mut a, &mut l, &mut [true; 4], &mut |_, _| {});
        };
        fits(bar(&mut doc, select), "seleção");
        // Todos os grupos de uma vez: dois caminhos abertos com contorno
        // (booleanas, contorno, setas e caneta).
        let line = |x: f64| {
            let mut o = Object::new(ObjectKind::Path(graphdraw_core::model::PathData {
                subpaths: vec![graphdraw_core::model::Subpath {
                    closed: false,
                    nodes: vec![
                        graphdraw_core::model::Node::corner(x, 0.0),
                        graphdraw_core::model::Node::corner(x + 10_000.0, 5_000.0),
                    ],
                    extra: Default::default(),
                }],
                fill_rule: Default::default(),
                extra: Default::default(),
            }));
            o.stroke = Some(graphdraw_core::model::Stroke::new(
                500.0,
                graphdraw_core::model::Color::black(),
            ));
            o
        };
        let a = ops::add_object(&mut doc.editor, 0, 0, line(0.0), "a");
        let b = ops::add_object(&mut doc.editor, 0, 0, line(20_000.0), "b");
        doc.selection = vec![a, b];
        let rev = doc.editor.revision();
        fits(bar(&mut doc, select), "dois caminhos abertos");
        assert_eq!(doc.editor.revision(), rev, "mostrar a barra não edita");
        ops::convert_to_curves(&mut doc.editor, std::slice::from_ref(&id));
        doc.selection = vec![id];
        fits(bar(&mut doc, shape_bar), "nós");
    }
}
