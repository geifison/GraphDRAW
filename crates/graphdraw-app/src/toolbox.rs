//! Caixa de ferramentas com grupos em gaveta (DESIGN-SYSTEM 4.8): um botão
//! mostra a ferramenta escolhida do grupo; o triângulo no canto inferior
//! direito (ou o botão direito) abre a gaveta com as outras.

use eframe::egui;

use crate::canvas::{Defaults, Tool};
use crate::i18n::tr;

/// Uma escolha dentro de um grupo: a ferramenta e, nas formas, a família.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Variant {
    pub icon: &'static str,
    pub tip: &'static str,
    pub tool: Tool,
    /// Família e modo da ferramenta Polígono (`shape_family`, `polygon_mode`).
    pub shape: Option<(&'static str, &'static str)>,
}

impl Variant {
    fn of(tool: Tool) -> Variant {
        let (icon, tip, _) = tool.info();
        Variant {
            icon,
            tip,
            tool,
            shape: None,
        }
    }

    /// Está em uso com a ferramenta e os padrões atuais.
    pub fn active(&self, tool: Tool, d: &Defaults) -> bool {
        tool == self.tool
            && self.shape.is_none_or(|(family, mode)| {
                d.shape_family == family && (family != "polygon" || d.polygon_mode == mode)
            })
    }

    /// Passa a usar esta escolha (a ferramenta é trocada por quem chama).
    pub fn apply(&self, d: &mut Defaults) {
        if let Some((family, mode)) = self.shape {
            d.shape_family = family.into();
            if family == "polygon" {
                d.polygon_mode = mode.into();
            }
        }
    }
}

pub enum Slot {
    One(Tool),
    Group(&'static str, Vec<Variant>),
}

/// Ordem da caixa de ferramentas; separadores depois do grupo Corte (que
/// vem após Forma) e de Mão.
pub fn slots() -> Vec<Slot> {
    let shape = |icon, tip, family, mode| Variant {
        icon,
        tip,
        tool: Tool::Polygon,
        shape: Some((family, mode)),
    };
    vec![
        Slot::Group(
            "selecao",
            vec![Variant::of(Tool::Select), Variant::of(Tool::FreeTransform)],
        ),
        Slot::One(Tool::Shape),
        Slot::Group(
            "corte",
            vec![
                Variant::of(Tool::Knife),
                Variant::of(Tool::Crop),
                Variant::of(Tool::Eraser),
                Variant::of(Tool::VirtualDelete),
            ],
        ),
        Slot::One(Tool::Zoom),
        Slot::One(Tool::Pan),
        Slot::Group(
            "curvas",
            vec![
                Variant::of(Tool::Bezier),
                Variant::of(Tool::Freehand),
                Variant::of(Tool::Polyline),
                Variant::of(Tool::Line2),
                Variant::of(Tool::Curve3),
            ],
        ),
        Slot::One(Tool::Rect),
        Slot::One(Tool::Ellipse),
        Slot::Group(
            "formas",
            vec![
                shape(
                    "tool-polygon",
                    tr("Polígono", "Polygon"),
                    "polygon",
                    "polygon",
                ),
                shape("poly-star", tr("Estrela", "Star"), "polygon", "star"),
                shape(
                    "poly-complex",
                    tr("Estrela complexa", "Complex star"),
                    "polygon",
                    "complex",
                ),
                shape("tool-spiral", tr("Espiral", "Spiral"), "spiral", ""),
                shape(
                    "tool-graph",
                    tr("Papel gráfico", "Graph paper"),
                    "graph",
                    "",
                ),
            ],
        ),
        Slot::One(Tool::Text),
        Slot::One(Tool::Dimension),
        Slot::Group(
            "preenchimento",
            vec![
                Variant::of(Tool::Gradient),
                Variant::of(Tool::SolidFill),
                Variant::of(Tool::Eyedropper),
            ],
        ),
        Slot::One(Tool::Artboard),
    ]
}

/// Separador depois desta ferramenta.
pub fn separator_after(t: Tool) -> bool {
    matches!(t, Tool::Pan)
}

/// Separador depois deste grupo (as ferramentas de edição de forma).
pub fn separator_after_group(name: &str) -> bool {
    name == "corte"
}

/// Desenha o triângulo de gaveta no canto inferior direito do botão.
pub fn drawer_mark(ui: &egui::Ui, r: egui::Rect) {
    let k = crate::theme::TOOL_DRAWER_MARK;
    let p = r.right_bottom() - egui::vec2(crate::theme::SPACE_050, crate::theme::SPACE_050);
    ui.painter().add(egui::Shape::convex_polygon(
        vec![p, p - egui::vec2(k, 0.0), p - egui::vec2(0.0, k)],
        crate::theme::palette(ui).fg_muted,
        egui::Stroke::NONE,
    ));
}

/// O clique caiu no canto do triângulo (abre a gaveta em vez de escolher).
pub fn hits_mark(r: egui::Rect, pos: Option<egui::Pos2>) -> bool {
    let k = crate::theme::TOOL_DRAWER_HIT;
    pos.is_some_and(|p| p.x >= r.max.x - k && p.y >= r.max.y - k)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Toda ferramenta aparece uma vez na caixa (sozinha ou num grupo).
    #[test]
    fn every_tool_is_reachable_once() {
        let mut seen: Vec<Tool> = Vec::new();
        for s in slots() {
            match s {
                Slot::One(t) => seen.push(t),
                Slot::Group(_, v) => {
                    let mut tools: Vec<Tool> = v.iter().map(|x| x.tool).collect();
                    tools.dedup();
                    seen.extend(tools);
                }
            }
        }
        for t in Tool::ALL {
            assert_eq!(seen.iter().filter(|x| **x == t).count(), 1, "{t:?}");
        }
    }

    /// Escolher estrela ou espiral na gaveta ajusta a ferramenta Polígono.
    #[test]
    fn shape_variants_set_family() {
        let Some(Slot::Group(_, v)) = slots()
            .into_iter()
            .find(|s| matches!(s, Slot::Group("formas", _)))
        else {
            panic!()
        };
        let mut d = Defaults::default();
        v[1].apply(&mut d);
        assert!(v[1].active(Tool::Polygon, &d) && !v[0].active(Tool::Polygon, &d));
        assert_eq!(
            (d.shape_family.as_str(), d.polygon_mode.as_str()),
            ("polygon", "star")
        );
        v[3].apply(&mut d);
        assert_eq!(d.shape_family, "spiral");
        assert!(v[3].active(Tool::Polygon, &d) && !v[1].active(Tool::Polygon, &d));
        assert!(!v[3].active(Tool::Select, &d));
    }
}
