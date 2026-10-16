//! Barra de propriedades contextual (GD-UI-01) e painéis: Transformação,
//! Alinhar, Camadas, Histórico, Páginas e Relatório de importação (GD-UI-02).

use eframe::egui::{self, Vec2};
use graphdraw_core::edit::nodes::{self, NodeRef};
use graphdraw_core::edit::ops::{self, Align, AlignTo, Anchor, Order};
use graphdraw_core::geometry::SegRef;
use graphdraw_core::model::{Corner, Fill, NodeKind, ObjectKind, Page, Stroke};
use graphdraw_core::report::Severity;
use kurbo::{Point, Vec2 as KVec2};

use crate::app::{App, RightTab};
use crate::canvas::{Tool, single_path};
use crate::doc::DocState;
use crate::i18n::tr;
use crate::icons;
use crate::widgets::{self, color_button, fmt_mm, length_field, number_field, swatch};

/// Seletor do ponto de referência (9 âncoras, GD-TRF-01).
pub fn anchor_picker(ui: &mut egui::Ui, anchor: &mut Anchor) {
    egui::Grid::new(ui.scope_id().with("ancora"))
        .spacing(Vec2::splat(1.0))
        .show(ui, |ui| {
            for (i, a) in Anchor::ALL.into_iter().enumerate() {
                let (rect, resp) = ui.allocate_exact_size(Vec2::splat(7.0), egui::Sense::click());
                let on = *anchor == a;
                let c = if on {
                    ui.visuals().selection.bg_fill
                } else {
                    ui.visuals().widgets.inactive.bg_fill
                };
                ui.painter().rect_filled(rect, 1.0, c);
                if resp
                    .on_hover_text(tr("Ponto de referência", "Reference point"))
                    .clicked()
                {
                    *anchor = a;
                }
                if i % 3 == 2 {
                    ui.end_row();
                }
            }
        });
}

impl App {
    /// Barra de propriedades: muda conforme a ferramenta e a seleção.
    pub fn property_bar(&mut self, ui: &mut egui::Ui) {
        let Some(doc) = self.doc.as_mut() else {
            ui.label(tr("Nenhum documento aberto", "No document open"));
            return;
        };
        ui.horizontal_wrapped(|ui| {
            match self.tool {
                Tool::Shape => shape_bar(ui, doc),
                Tool::Bezier => {
                    ui.label(tr(
                        "Clique = nó de canto · clique e arraste = nó suave · clique no primeiro nó fecha · Enter/duplo clique termina · Backspace desfaz o último nó · Esc cancela",
                        "Click = corner node · click-drag = smooth node · click first node to close · Enter/double-click finishes · Backspace removes last · Esc cancels",
                    ));
                }
                Tool::Rect | Tool::Ellipse => {
                    ui.label(tr("Arraste para desenhar · Shift = proporcional · Ctrl = a partir do centro", "Drag to draw · Shift = proportional · Ctrl = from center"));
                }
                Tool::Zoom => {
                    ui.label(tr("Clique aproxima · botão direito afasta · arraste um retângulo para enquadrar", "Click zooms in · right-click zooms out · drag a rectangle to frame it"));
                }
                Tool::Pan => {
                    ui.label(tr("Arraste para mover a vista (espaço ativa temporariamente)", "Drag to pan (hold space for temporary hand)"));
                }
                Tool::Select => select_bar(ui, doc, &mut self.anchor, &mut self.lock_ratio),
            }
            ui.separator();
            style_controls(ui, doc, &mut self.defaults);
        });
    }

    pub fn right_panel(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            for (tab, label) in [
                (RightTab::Transform, tr("Transformar", "Transform")),
                (RightTab::Align, tr("Alinhar", "Align")),
                (RightTab::Layers, tr("Camadas", "Layers")),
                (RightTab::History, tr("Histórico", "History")),
                (RightTab::Pages, tr("Páginas", "Pages")),
                (RightTab::Report, tr("Relatório", "Report")),
            ] {
                if ui.selectable_label(self.right_tab == tab, label).clicked() {
                    self.right_tab = tab;
                }
            }
        });
        ui.separator();
        let Some(doc) = self.doc.as_mut() else { return };
        egui::ScrollArea::vertical().show(ui, |ui| match self.right_tab {
            RightTab::Transform => transform_panel(ui, doc, &mut self.xf),
            RightTab::Align => align_panel(ui, doc, &mut self.align),
            RightTab::Layers => layers_panel(ui, doc),
            RightTab::History => history_panel(ui, doc),
            RightTab::Pages => {
                if pages_panel(ui, doc) {
                    self.canvas.fit_pending = true;
                }
            }
            RightTab::Report => report_panel(ui, doc),
        });
    }

    /// Barra de paleta (GD-PAL-05): clique = preenchimento, botão direito = contorno.
    pub fn palette_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            let none = swatch(ui, None, 18.0).on_hover_text(tr(
                "Sem cor (clique: sem preenchimento · direito: sem contorno)",
                "No color (click: no fill · right: no outline)",
            ));
            if none.clicked() {
                self.apply_fill(None);
            }
            if none.secondary_clicked() {
                self.apply_stroke_color(None);
            }
            for c in widgets::palette() {
                let r = swatch(ui, Some(&c), 18.0).on_hover_text(tr(
                    "Clique: preenchimento · botão direito: contorno",
                    "Click: fill · right-click: outline",
                ));
                if r.clicked() {
                    self.apply_fill(Some(Fill::solid(c.clone())));
                }
                if r.secondary_clicked() {
                    self.apply_stroke_color(Some(c));
                }
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

fn select_bar(ui: &mut egui::Ui, doc: &mut DocState, anchor: &mut Anchor, lock: &mut bool) {
    let sel = doc.selection.clone();
    let Some(b) = ops::selection_bounds(&doc.editor, &sel, false) else {
        ui.label(tr(
            "Clique num objeto para selecionar; arraste para selecionar por área",
            "Click an object to select it; drag to select an area",
        ));
        return;
    };
    anchor_picker(ui, anchor);
    let p = anchor.point(b);
    ui.label("X");
    if let Some(x) = length_field(ui, "x", p.x, None) {
        ops::set_position(&mut doc.editor, &sel, *anchor, Point::new(x, p.y));
    }
    ui.label("Y");
    if let Some(y) = length_field(ui, "y", p.y, None) {
        ops::set_position(&mut doc.editor, &sel, *anchor, Point::new(p.x, y));
    }
    ui.separator();
    ui.label(tr("L", "W"));
    if let Some(w) = length_field(ui, "w", b.width(), Some(b.width())) {
        ops::set_size(&mut doc.editor, &sel, *anchor, Some(w), None, *lock);
    }
    ui.label(tr("A", "H"));
    if let Some(h) = length_field(ui, "h", b.height(), Some(b.height())) {
        ops::set_size(&mut doc.editor, &sel, *anchor, None, Some(h), *lock);
    }
    ui.toggle_value(lock, "🔗")
        .on_hover_text(tr("Manter proporção", "Keep proportions"));
    ui.separator();
    ui.label("∠");
    if let Some(a) = number_field(ui, "angulo", 0.0, "°") {
        ops::transform(
            &mut doc.editor,
            &sel,
            ops::rotate_about(anchor.point(b), a),
            tr("Girar", "Rotate"),
        );
    }
    if icons::button(
        ui,
        "mirror-h",
        tr("Espelhar na horizontal", "Mirror horizontally"),
        false,
        true,
    )
    .clicked()
    {
        ops::transform(
            &mut doc.editor,
            &sel,
            ops::mirror_about(b.center(), true),
            tr("Espelhar", "Mirror"),
        );
    }
    if icons::button(
        ui,
        "mirror-v",
        tr("Espelhar na vertical", "Mirror vertically"),
        false,
        true,
    )
    .clicked()
    {
        ops::transform(
            &mut doc.editor,
            &sel,
            ops::mirror_about(b.center(), false),
            tr("Espelhar", "Mirror"),
        );
    }
    ui.separator();
    for (icon, tip, order) in [
        (
            "order-front",
            tr(
                "Trazer para frente (Shift+PgUp)",
                "Bring to front (Shift+PgUp)",
            ),
            Order::ToFront,
        ),
        (
            "order-forward",
            tr("Um passo à frente (Ctrl+PgUp)", "Bring forward (Ctrl+PgUp)"),
            Order::Forward,
        ),
        (
            "order-backward",
            tr("Um passo atrás (Ctrl+PgDn)", "Send backward (Ctrl+PgDn)"),
            Order::Backward,
        ),
        (
            "order-back",
            tr("Enviar para trás (Shift+PgDn)", "Send to back (Shift+PgDn)"),
            Order::ToBack,
        ),
    ] {
        if icons::button(ui, icon, tip, false, true).clicked() {
            ops::reorder(&mut doc.editor, &sel, order);
        }
    }
    ui.separator();
    if icons::button(
        ui,
        "group",
        tr("Agrupar (Ctrl+G)", "Group (Ctrl+G)"),
        false,
        sel.len() > 1,
    )
    .clicked()
        && let Some(id) = ops::group(&mut doc.editor, &sel)
    {
        doc.selection = vec![id];
    }
    let any_group = doc
        .selected_objects()
        .iter()
        .any(|o| matches!(o.kind, ObjectKind::Group(_)));
    if icons::button(
        ui,
        "ungroup",
        tr("Desagrupar (Ctrl+U)", "Ungroup (Ctrl+U)"),
        false,
        any_group,
    )
    .clicked()
    {
        doc.selection = ops::ungroup(&mut doc.editor, &sel);
    }
    // Primitivas (GD-PRI-01/02): parâmetros editáveis até converter em curvas.
    let objs = doc.selected_objects();
    let has_primitive = objs
        .iter()
        .any(|o| matches!(o.kind, ObjectKind::Rectangle(_) | ObjectKind::Ellipse(_)));
    let single = (objs.len() == 1).then(|| objs[0].clone());
    if has_primitive
        && icons::button(
            ui,
            "to-curves",
            tr("Converter em curvas (Ctrl+Q)", "Convert to curves (Ctrl+Q)"),
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
                let corner = r
                    .corners
                    .as_ref()
                    .and_then(|c| c.first())
                    .cloned()
                    .unwrap_or(Corner {
                        kind: "round".into(),
                        radius: 0.0,
                    });
                let mut kind = corner.kind.clone();
                egui::ComboBox::from_id_salt("canto")
                    .selected_text(corner_label(&kind))
                    .width(90.0)
                    .show_ui(ui, |ui| {
                        for k in ["round", "chamfer", "scallop"] {
                            ui.selectable_value(&mut kind, k.to_string(), corner_label(k));
                        }
                    });
                ui.label(tr("Raio", "Radius"));
                let new_radius =
                    length_field(ui, "raio", corner.radius, Some(r.width.min(r.height) / 2.0));
                if kind != corner.kind || new_radius.is_some() {
                    let radius = new_radius.unwrap_or(corner.radius).max(0.0);
                    ops::modify(
                        &mut doc.editor,
                        &sel,
                        tr("Alterar cantos", "Change corners"),
                        |o| {
                            if let ObjectKind::Rectangle(r) = &mut o.kind {
                                r.corners = Some(vec![
                                    Corner {
                                        kind: kind.clone(),
                                        radius
                                    };
                                    4
                                ]);
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

/// Preenchimento e contorno da seleção — ou dos próximos objetos, se nada
/// estiver selecionado (GD-FIL-01/02, GD-OUT-01..04).
fn style_controls(ui: &mut egui::Ui, doc: &mut DocState, defaults: &mut crate::canvas::Defaults) {
    let sel = doc.selection.clone();
    let first = doc
        .selected_objects()
        .into_iter()
        .find(|o| !matches!(o.kind, ObjectKind::Group(_) | ObjectKind::Image(_)))
        .cloned();
    let editing_defaults = sel.is_empty();
    let (fill, stroke) = match &first {
        Some(o) => (o.fill.clone(), o.stroke.clone()),
        None => (defaults.fill.clone(), defaults.stroke.clone()),
    };
    if editing_defaults {
        ui.label(tr("Novos objetos:", "New objects:"));
    }
    // Preenchimento.
    ui.label(tr("Preench.", "Fill"));
    let fill_color = match &fill {
        Some(Fill::Solid { color, .. }) => Some(color.clone()),
        _ => None,
    };
    if let Some(c) = color_button(
        ui,
        fill_color.as_ref(),
        tr("Cor do preenchimento", "Fill color"),
    ) {
        let f = Some(Fill::solid(c));
        if editing_defaults {
            defaults.fill = f
        } else {
            ops::set_fill(&mut doc.editor, &sel, f)
        }
    }
    if icons::button(
        ui,
        "fill-none",
        tr("Sem preenchimento", "No fill"),
        fill_color.is_none(),
        true,
    )
    .clicked()
    {
        if editing_defaults {
            defaults.fill = None
        } else {
            ops::set_fill(&mut doc.editor, &sel, Some(Fill::None))
        }
    }
    ui.separator();
    // Contorno.
    ui.label(tr("Contorno", "Outline"));
    let s = stroke.clone();
    if let Some(c) = color_button(
        ui,
        s.as_ref().map(|s| &s.color),
        tr("Cor do contorno", "Outline color"),
    ) {
        apply_stroke(doc, defaults, &sel, editing_defaults, move |st| {
            st.color = c.clone()
        });
    }
    if icons::button(
        ui,
        "stroke-none",
        tr("Sem contorno", "No outline"),
        s.is_none(),
        true,
    )
    .clicked()
    {
        if editing_defaults {
            defaults.stroke = None
        } else {
            ops::set_stroke(&mut doc.editor, &sel, None)
        }
    }
    let width = s.as_ref().map_or(0.0, |s| s.width);
    if let Some(w) = length_field(ui, "espessura", width, None) {
        apply_stroke(doc, defaults, &sel, editing_defaults, move |st| {
            st.width = w.max(0.0)
        });
    }
    let Some(st) = s else { return };
    let dash_kind = match &st.dash {
        None => 0,
        Some(d) if d.first().is_some_and(|v| *v <= st.width.max(1.0) * 1.01) => 2,
        Some(_) => 1,
    };
    let mut dk = dash_kind;
    egui::ComboBox::from_id_salt("tracejado")
        .selected_text(
            [
                tr("Sólido", "Solid"),
                tr("Tracejado", "Dashed"),
                tr("Pontilhado", "Dotted"),
            ][dk],
        )
        .width(80.0)
        .show_ui(ui, |ui| {
            ui.selectable_value(&mut dk, 0, tr("Sólido", "Solid"));
            ui.selectable_value(&mut dk, 1, tr("Tracejado", "Dashed"));
            ui.selectable_value(&mut dk, 2, tr("Pontilhado", "Dotted"));
        });
    if dk != dash_kind {
        apply_stroke(doc, defaults, &sel, editing_defaults, move |st| {
            let w = st.width.max(250.0);
            st.dash = match dk {
                1 => Some(vec![w * 4.0, w * 2.0]),
                2 => Some(vec![w, w * 2.0]),
                _ => None,
            };
            if dk == 2 {
                st.cap = "round".into();
            }
        });
    }
    let mut join = st.join.clone();
    egui::ComboBox::from_id_salt("juncao")
        .selected_text(join_label(&join))
        .width(84.0)
        .show_ui(ui, |ui| {
            for j in ["miter", "round", "bevel"] {
                ui.selectable_value(&mut join, j.to_string(), join_label(j));
            }
        })
        .response
        .on_hover_text(tr("Junção", "Join"));
    if join != st.join {
        apply_stroke(doc, defaults, &sel, editing_defaults, move |s| {
            s.join = join.clone()
        });
    }
    let mut cap = st.cap.clone();
    egui::ComboBox::from_id_salt("terminacao")
        .selected_text(cap_label(&cap))
        .width(84.0)
        .show_ui(ui, |ui| {
            for c in ["butt", "round", "square"] {
                ui.selectable_value(&mut cap, c.to_string(), cap_label(c));
            }
        })
        .response
        .on_hover_text(tr("Terminação", "Cap"));
    if cap != st.cap {
        apply_stroke(doc, defaults, &sel, editing_defaults, move |s| {
            s.cap = cap.clone()
        });
    }
    let mut pos = st.position.clone();
    egui::ComboBox::from_id_salt("posicao")
        .selected_text(pos_label(&pos))
        .width(84.0)
        .show_ui(ui, |ui| {
            for p in ["center", "inside", "outside"] {
                ui.selectable_value(&mut pos, p.to_string(), pos_label(p));
            }
        })
        .response
        .on_hover_text(tr(
            "Posição do contorno em relação à borda (curvas abertas usam sempre centrado)",
            "Outline position relative to the edge (open curves always use center)",
        ));
    if pos != st.position {
        apply_stroke(doc, defaults, &sel, editing_defaults, move |s| {
            s.position = pos.clone()
        });
    }
}

fn apply_stroke(
    doc: &mut DocState,
    defaults: &mut crate::canvas::Defaults,
    sel: &[String],
    editing_defaults: bool,
    f: impl Fn(&mut Stroke),
) {
    if editing_defaults {
        let mut s = defaults
            .stroke
            .clone()
            .unwrap_or_else(crate::doc::default_stroke);
        f(&mut s);
        defaults.stroke = Some(s);
    } else {
        ops::edit_stroke(&mut doc.editor, sel, f);
    }
}

fn join_label(j: &str) -> &'static str {
    match j {
        "round" => tr("Redonda", "Round"),
        "bevel" => tr("Chanfrada", "Bevel"),
        _ => tr("Aguda", "Miter"),
    }
}

fn cap_label(c: &str) -> &'static str {
    match c {
        "round" => tr("Redonda", "Round"),
        "square" => tr("Quadrada", "Square"),
        _ => tr("Reta", "Butt"),
    }
}

fn pos_label(p: &str) -> &'static str {
    match p {
        "inside" => tr("Interno", "Inside"),
        "outside" => tr("Externo", "Outside"),
        _ => tr("Centrado", "Center"),
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
            tr("Converter em curvas (Ctrl+Q)", "Convert to curves (Ctrl+Q)"),
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
        ui.checkbox(&mut xf.proportional, tr("Proporcional", "Proportional"));
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
    let min = if al.to < 2 { 2 } else { 1 };
    let enabled = sel.len() >= min;
    let (pw, ph) = doc.page_size();
    let to = match al.to {
        0 => AlignTo::LastSelected,
        1 => AlignTo::FirstSelected,
        2 => AlignTo::Page(kurbo::Rect::new(0.0, 0.0, pw, ph)),
        _ => AlignTo::Selection,
    };
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
    });
    if !enabled {
        ui.weak(if min == 2 {
            tr("Selecione ao menos 2 objetos", "Select at least 2 objects")
        } else {
            tr("Selecione objetos", "Select objects")
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

/// Páginas (GD-PAG-01, GD-PAG-03 básico). Devolve `true` ao trocar de página.
fn pages_panel(ui: &mut egui::Ui, doc: &mut DocState) -> bool {
    let mut switch = None;
    let n = doc.doc().data.pages.len();
    for (i, p) in doc.doc().data.pages.iter().enumerate() {
        let label = format!(
            "{}. {}\n{} × {} mm",
            i + 1,
            p.name
                .clone()
                .unwrap_or_else(|| format!("{} {}", tr("Página", "Page"), i + 1)),
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
            let (w, h) = doc.page_size();
            let mut p = Page::new(w, h);
            p.name = Some(format!("{} {}", tr("Página", "Page"), n + 1));
            doc.editor
                .transaction(tr("Inserir página", "Insert page"), |ed| {
                    ed.insert_page(cur + 1, p)
                });
            switch = Some(cur + 1);
        }
        if ui.button(tr("Duplicar", "Duplicate")).clicked() {
            let mut p = doc.page().clone();
            p.id = graphdraw_core::model::new_id();
            for l in &mut p.layers {
                l.id = graphdraw_core::model::new_id();
                for o in &mut l.objects {
                    ops::renew_ids(o);
                }
            }
            doc.editor
                .transaction(tr("Duplicar página", "Duplicate page"), |ed| {
                    ed.insert_page(cur + 1, p)
                });
            switch = Some(cur + 1);
        }
        if ui
            .add_enabled(n > 1, egui::Button::new(tr("Excluir", "Delete")))
            .clicked()
        {
            doc.editor
                .transaction(tr("Excluir página", "Delete page"), |ed| {
                    ed.remove_page(cur);
                });
            doc.page = cur.min(n - 2);
            doc.selection.clear();
            switch = Some(doc.page);
        }
    });
    // Tamanho da página atual (GD-PAG-01).
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
        let mut p = doc.page().clone();
        p.width = nw.unwrap_or(w).max(1000.0);
        p.height = nh.unwrap_or(h).max(1000.0);
        doc.editor
            .transaction(tr("Tamanho da página", "Page size"), |ed| {
                ed.replace_page(cur, p)
            });
    }
    if let Some(i) = switch {
        doc.set_page(i.min(doc.doc().data.pages.len() - 1));
        doc.page = i.min(doc.doc().data.pages.len() - 1);
        return true;
    }
    false
}

enum LayerAction {
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

/// Camadas da página atual (GD-CAM-01, GD-CAM-03). O topo da lista é a
/// camada da frente; a marcada recebe os objetos novos.
fn layers_panel(ui: &mut egui::Ui, doc: &mut DocState) {
    let page = doc.page;
    let n = doc.page().layers.len();
    let chosen = doc.chosen_layer();
    let sel_layers: Vec<usize> = ops::refs(&doc.editor, &doc.selection)
        .iter()
        .map(|r| r.loc.layer)
        .collect();
    let mut act = None;
    for i in (0..n).rev() {
        let l = &doc.page().layers[i];
        ui.horizontal(|ui| {
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
            Severity::Warning => (
                tr("aviso", "warning"),
                egui::Color32::from_rgb(0xd0, 0x8a, 0x00),
            ),
            Severity::Error => (
                tr("erro", "error"),
                egui::Color32::from_rgb(0xd0, 0x30, 0x30),
            ),
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
