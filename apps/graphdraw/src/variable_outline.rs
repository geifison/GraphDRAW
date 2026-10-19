//! The Variable Outline tool: width nodes along an object's outline.
//! A drag across the outline of the selected object adds a node there (or
//! takes the node under the pointer) and sets how far the outline reaches
//! to the pointer; with Alt only the side the pointer is on changes. The
//! property bar edits the selected node's reach and removes nodes.

use crate::app::App;
use egui::{Color32, Modifiers, Painter, PointerButton, Response, Stroke as EStroke};
use tracedraw_core::geometry::{Affine, BezPath, Point};
use tracedraw_core::{Command, ShapeId, Stroke, WidthNode};

/// Screen distance (pixels) within which a press takes a node or the
/// outline.
const GRAB_PX: f64 = 7.0;

/// The tool's state: the node picked on the selected object and the drag
/// under way.
#[derive(Default, Clone, Debug)]
pub struct VariableOutlineState {
    pub node: Option<usize>,
    drag: Option<NodeDrag>,
}

#[derive(Clone, Debug)]
struct NodeDrag {
    shape: ShapeId,
    index: usize,
    /// Alt held at the start: only the side the pointer is on changes.
    one_side: bool,
    /// The first change of the drag has been recorded (later ones amend
    /// it, so the drag is one undo step).
    recorded: bool,
}

/// Sorted nodes, with the index the node at `keep` ends up at.
fn sorted(mut nodes: Vec<WidthNode>, keep: usize) -> (Vec<WidthNode>, usize) {
    let marked: Vec<(usize, WidthNode)> = nodes.drain(..).enumerate().collect();
    let mut marked = marked;
    marked.sort_by(|a, b| a.1.at.total_cmp(&b.1.at));
    let at = marked.iter().position(|(i, _)| *i == keep).unwrap_or(0);
    (marked.into_iter().map(|(_, n)| n).collect(), at)
}

impl App {
    /// The object the tool works on: the first selected one with a path.
    pub fn variable_outline_target(&self) -> Option<ShapeId> {
        self.selection.first().copied().filter(|id| {
            self.doc()
                .find_shape(*id)
                .map(|s| !s.page_path().elements().is_empty())
                .unwrap_or(false)
        })
    }

    /// The target's outline in page space, its stroke and the factor its
    /// widths are drawn at.
    pub fn variable_outline_geometry(&self, id: ShapeId) -> Option<(BezPath, Stroke, f64)> {
        let doc = self.doc();
        let shape = doc.find_shape(id)?;
        let parent = doc.parent_transform(id).unwrap_or(Affine::IDENTITY);
        let path = parent * shape.page_path();
        let stroke = shape.stroke.clone().unwrap_or_default();
        let scale = if stroke.scale_with_object {
            let c = (parent * shape.transform).as_coeffs();
            ((c[0] * c[3] - c[1] * c[2]).abs()).sqrt().max(1e-6)
        } else {
            1.0
        };
        Some((path, stroke, scale))
    }

    /// Replace the target's width nodes; the first change of a drag is a
    /// new undo step, later ones join it.
    fn set_width_nodes(&mut self, id: ShapeId, nodes: Vec<WidthNode>, amend: bool) {
        let Some((_, mut stroke, _)) = self.variable_outline_geometry(id) else {
            return;
        };
        stroke.widths = nodes;
        let cmd = Command::SetStroke {
            shapes: vec![id],
            stroke: Some(stroke),
        };
        if amend {
            let _ = self.engine.amend(&cmd);
        } else {
            self.run(cmd);
        }
    }

    pub fn variable_outline_input(&mut self, response: &Response, p: Point, mods: Modifiers) {
        if response.drag_started_by(PointerButton::Primary)
            || response.clicked_by(PointerButton::Primary)
        {
            self.variable_outline_press(p, mods.alt);
        }
        if response.dragged_by(PointerButton::Primary) {
            self.variable_outline_drag(p);
        }
        if response.drag_stopped() || response.clicked_by(PointerButton::Primary) {
            self.vo.drag = None;
        }
    }

    /// A press: on a node, it is picked; on the outline, a node is added
    /// there with the reach the outline has; elsewhere, the object under
    /// the pointer is selected.
    pub fn variable_outline_press(&mut self, p: Point, one_side: bool) {
        let grab = GRAB_PX / self.view.zoom.max(1e-6) as f64;
        if let Some(id) = self.variable_outline_target() {
            if let Some((path, stroke, scale)) = self.variable_outline_geometry(id) {
                // A node under the pointer.
                let hit = stroke.widths.iter().enumerate().find_map(|(i, n)| {
                    tracedraw_core::shaping::point_on_path(&path, n.at)
                        .filter(|(q, _)| (*q - p).hypot() <= grab)
                        .map(|_| i)
                });
                if let Some(i) = hit {
                    self.vo.node = Some(i);
                    self.vo.drag = Some(NodeDrag {
                        shape: id,
                        index: i,
                        one_side,
                        recorded: false,
                    });
                    return;
                }
                // The outline under the pointer.
                if let Some((t, d, _)) = tracedraw_core::shaping::locate_on_path(&path, p) {
                    let (l, r) = tracedraw_core::style::width_at(stroke.width, &stroke.widths, t);
                    if d <= (l.max(r) * scale) + grab {
                        let mut nodes = stroke.widths.clone();
                        nodes.push(WidthNode {
                            at: t,
                            left: l,
                            right: r,
                        });
                        let last = nodes.len() - 1;
                        let (nodes, index) = sorted(nodes, last);
                        self.set_width_nodes(id, nodes, false);
                        self.vo.node = Some(index);
                        self.vo.drag = Some(NodeDrag {
                            shape: id,
                            index,
                            one_side,
                            recorded: true,
                        });
                        return;
                    }
                }
            }
        }
        self.vo.node = None;
        self.vo.drag = None;
        match self.hit_test(p) {
            Some(id) => self.select(vec![id]),
            None => self.selection.clear(),
        }
    }

    /// A drag: the picked node reaches to the pointer (one side or both).
    pub fn variable_outline_drag(&mut self, p: Point) {
        let Some(drag) = self.vo.drag.clone() else {
            return;
        };
        let Some((path, stroke, scale)) = self.variable_outline_geometry(drag.shape) else {
            return;
        };
        let Some(node) = stroke.widths.get(drag.index).copied() else {
            return;
        };
        let Some((q, n)) = tracedraw_core::shaping::point_on_path(&path, node.at) else {
            return;
        };
        // Distance across the outline, measured along the normal.
        let across = (p.x - q.x) * n[0] + (p.y - q.y) * n[1];
        let reach = across.abs() / scale;
        let mut nodes = stroke.widths.clone();
        if let Some(m) = nodes.get_mut(drag.index) {
            if drag.one_side {
                if across >= 0.0 {
                    m.left = reach;
                } else {
                    m.right = reach;
                }
            } else {
                m.left = reach;
                m.right = reach;
            }
        }
        self.set_width_nodes(drag.shape, nodes, drag.recorded);
        if let Some(d) = self.vo.drag.as_mut() {
            d.recorded = true;
        }
    }

    /// The picked node's left and right reach (mm).
    pub fn variable_outline_node(&self) -> Option<WidthNode> {
        let id = self.variable_outline_target()?;
        let (_, stroke, _) = self.variable_outline_geometry(id)?;
        stroke.widths.get(self.vo.node?).copied()
    }

    /// Set the picked node's reach (property bar).
    pub fn set_variable_outline_node(&mut self, left: f64, right: f64) {
        let (Some(id), Some(i)) = (self.variable_outline_target(), self.vo.node) else {
            return;
        };
        let Some((_, stroke, _)) = self.variable_outline_geometry(id) else {
            return;
        };
        let mut nodes = stroke.widths.clone();
        if let Some(n) = nodes.get_mut(i) {
            n.left = left.max(0.0);
            n.right = right.max(0.0);
            self.set_width_nodes(id, nodes, false);
        }
    }

    /// Remove the picked node (Delete or the property bar).
    pub fn delete_variable_outline_node(&mut self) -> bool {
        let (Some(id), Some(i)) = (self.variable_outline_target(), self.vo.node) else {
            return false;
        };
        let Some((_, stroke, _)) = self.variable_outline_geometry(id) else {
            return false;
        };
        if i >= stroke.widths.len() {
            return false;
        }
        let mut nodes = stroke.widths.clone();
        nodes.remove(i);
        self.set_width_nodes(id, nodes, false);
        self.vo.node = None;
        true
    }

    /// Remove every width node of the selected objects: a plain outline
    /// again.
    pub fn clear_variable_outline(&mut self) {
        let shapes: Vec<_> = self
            .selected_shapes()
            .into_iter()
            .filter(|s| s.stroke.as_ref().is_some_and(|st| !st.widths.is_empty()))
            .collect();
        let cmds: Vec<Command> = shapes
            .iter()
            .filter_map(|s| {
                let mut st = s.stroke.clone()?;
                st.widths.clear();
                Some(Command::SetStroke {
                    shapes: vec![s.id],
                    stroke: Some(st),
                })
            })
            .collect();
        for cmd in cmds {
            self.run(cmd);
        }
        self.vo.node = None;
    }
}

/// The target's width nodes: a dot on the path and a line across the
/// reach on each side; the picked node filled.
pub fn draw_width_nodes(app: &App, painter: &Painter) {
    if app.tool != crate::tools::Tool::VariableOutline {
        return;
    }
    let Some(id) = app.variable_outline_target() else {
        return;
    };
    let Some((path, stroke, scale)) = app.variable_outline_geometry(id) else {
        return;
    };
    let blue = crate::theme::Tokens::SELECTION;
    // The path itself, thin, so the line the nodes sit on shows.
    for (pts, closed) in crate::canvas::flatten(&path, &app.view) {
        let line = EStroke::new(1.0, blue.gamma_multiply(0.6));
        if closed {
            painter.add(egui::epaint::PathShape::closed_line(pts, line));
        } else {
            painter.add(egui::epaint::PathShape::line(pts, line));
        }
    }
    for (i, node) in stroke.widths.iter().enumerate() {
        let Some((q, n)) = tracedraw_core::shaping::point_on_path(&path, node.at) else {
            continue;
        };
        let at = |d: f64| {
            app.view
                .to_screen(Point::new(q.x + n[0] * d, q.y + n[1] * d))
        };
        let c = app.view.to_screen(q);
        let a = at(node.left * scale);
        let b = at(-node.right * scale);
        painter.line_segment([a, b], EStroke::new(1.0, blue));
        for e in [a, b] {
            painter.circle_filled(e, 2.5, blue);
        }
        let picked = app.vo.node == Some(i);
        painter.circle_filled(c, 4.0, if picked { blue } else { Color32::WHITE });
        painter.circle_stroke(c, 4.0, EStroke::new(1.2, blue));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracedraw_core::geometry::Shape as _;
    use tracedraw_core::{Shape, ShapeKind};

    fn line_app() -> (App, ShapeId) {
        let mut app = App::headless();
        let layer = app.active_layer().expect("a layer");
        let id = app.engine.new_shape_id();
        let mut path = BezPath::new();
        path.move_to(Point::new(10.0, 50.0));
        path.line_to(Point::new(110.0, 50.0));
        let mut s = Shape::new(
            id,
            ShapeKind::Path {
                path,
                closed: false,
            },
        );
        s.stroke = Some(Stroke::new(tracedraw_core::Color::BLACK, 2.0));
        app.run(Command::AddShape { layer, shape: s });
        app.select(vec![id]);
        app.tool = crate::tools::Tool::VariableOutline;
        (app, id)
    }

    fn widths(app: &App, id: ShapeId) -> Vec<WidthNode> {
        app.doc()
            .find_shape(id)
            .and_then(|s| s.stroke.clone())
            .map(|s| s.widths)
            .unwrap_or_default()
    }

    #[test]
    fn a_drag_across_the_outline_adds_a_node_and_sets_its_reach_in_one_step() {
        let (mut app, id) = line_app();
        let undo_before = app.engine.undo_label().map(str::to_string);
        // Press on the outline halfway, drag 5 mm up.
        app.variable_outline_press(Point::new(60.0, 50.5), false);
        assert_eq!(widths(&app, id).len(), 1);
        app.variable_outline_drag(Point::new(60.0, 53.0));
        app.variable_outline_drag(Point::new(60.0, 55.0));
        let w = widths(&app, id);
        assert!((w[0].at - 0.5).abs() < 0.01, "{w:?}");
        assert!((w[0].left - 5.0).abs() < 1e-6 && (w[0].right - 5.0).abs() < 1e-6);
        // The outline is 10 mm thick there now.
        let shape = app.doc().find_shape(id).cloned();
        let band = shape
            .and_then(|s| {
                let st = s.stroke.clone()?;
                tracedraw_core::shaping::outline_band(&s.page_path(), &st, 1.0)
            })
            .map(|b| b.bounding_box())
            .unwrap_or_default();
        assert!((band.height() - 10.0).abs() < 1e-3, "{band:?}");
        // One undo takes the whole drag back.
        app.undo();
        assert!(widths(&app, id).is_empty());
        assert_eq!(app.engine.undo_label().map(str::to_string), undo_before);
    }

    #[test]
    fn alt_changes_one_side_and_the_bar_edits_and_removes_nodes() {
        let (mut app, id) = line_app();
        app.variable_outline_press(Point::new(35.0, 50.0), true);
        app.variable_outline_drag(Point::new(35.0, 46.0));
        let w = widths(&app, id);
        assert!((w[0].right - 4.0).abs() < 1e-6 && (w[0].left - 1.0).abs() < 1e-6);
        // A press on the node picks it again rather than adding one.
        app.vo.node = None;
        app.variable_outline_press(Point::new(35.0, 50.0), false);
        assert_eq!(widths(&app, id).len(), 1);
        assert_eq!(app.vo.node, Some(0));
        app.set_variable_outline_node(3.0, 0.5);
        assert_eq!(
            app.variable_outline_node().map(|n| (n.left, n.right)),
            Some((3.0, 0.5))
        );
        // A second node further along keeps the nodes in path order.
        app.vo.drag = None;
        app.variable_outline_press(Point::new(20.0, 50.0), false);
        let w = widths(&app, id);
        assert_eq!(w.len(), 2);
        assert!(w[0].at < w[1].at);
        assert_eq!(app.vo.node, Some(0));
        assert!(app.delete_variable_outline_node());
        assert_eq!(widths(&app, id).len(), 1);
        app.clear_variable_outline();
        assert!(widths(&app, id).is_empty());
        // Away from the outline: the object under the pointer, or nothing.
        app.variable_outline_press(Point::new(60.0, 150.0), false);
        assert!(app.selection.is_empty());
    }
}

/// The Variable Outline tool's property bar: the picked node's width on
/// each side, Delete Node and Clear.
pub fn variable_outline_bar(app: &mut App, ui: &mut egui::Ui) {
    use crate::i18n::tr;
    use crate::ui::field::NumField;
    let node = app.variable_outline_node();
    let has_nodes = app
        .selected_shapes()
        .iter()
        .any(|s| s.stroke.as_ref().is_some_and(|st| !st.widths.is_empty()));
    let field = |ui: &mut egui::Ui, key: &str, mm: f64| -> Option<f64> {
        ui.label(tr(key));
        let mut v = app.units.world_from_mm(mm);
        let r = ui.add_enabled(
            node.is_some(),
            NumField::new(&mut v)
                .range(0.0..=app.units.world_from_mm(500.0))
                .fixed_decimals(app.settings.precision.min(10) as usize)
                .unit_mm(app.units.world_to_mm(1.0))
                .suffix(format!(" {}", app.units.short()))
                .width(76.0),
        );
        r.changed().then(|| app.units.world_to_mm(v))
    };
    let (l, r) = node.map(|n| (n.left, n.right)).unwrap_or((0.0, 0.0));
    let new_l = field(ui, "vo.left", l);
    let new_r = field(ui, "vo.right", r);
    if new_l.is_some() || new_r.is_some() {
        app.set_variable_outline_node(new_l.unwrap_or(l), new_r.unwrap_or(r));
    }
    crate::ui::propbar::sep(ui);
    if ui
        .add_enabled(node.is_some(), egui::Button::new(tr("vo.delete_node")))
        .clicked()
    {
        app.delete_variable_outline_node();
    }
    if ui
        .add_enabled(has_nodes, egui::Button::new(tr("vo.clear")))
        .on_hover_text(tr("vo.clear_hint"))
        .clicked()
    {
        app.clear_variable_outline();
    }
}
