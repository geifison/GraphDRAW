//! Geometria básica: matriz afim (GD-OBJ-02) e caminhos de nós (GD-GEO-01..04, GD-GDR-06).

use kurbo::{Affine, BezPath, Point};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Matriz afim 2×3 `[a, b, c, d, e, f]`:
/// `x' = a·x + c·y + e`, `y' = b·x + d·y + f` (mesma convenção de PDF/SVG).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Transform(pub [f64; 6]);

impl Default for Transform {
    fn default() -> Self {
        Transform::IDENTITY
    }
}

impl Transform {
    pub const IDENTITY: Transform = Transform([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);

    pub fn translate(x: f64, y: f64) -> Transform {
        Transform([1.0, 0.0, 0.0, 1.0, x, y])
    }

    pub fn is_identity(&self) -> bool {
        *self == Transform::IDENTITY
    }

    pub fn affine(&self) -> Affine {
        Affine::new(self.0)
    }

    pub fn from_affine(a: Affine) -> Transform {
        Transform(a.as_coeffs())
    }

    /// `self ∘ other` (aplica `other` primeiro).
    pub fn then_inner(&self, other: &Transform) -> Transform {
        Transform::from_affine(self.affine() * other.affine())
    }

    /// Fator de escala médio (para espessura de contorno).
    pub fn mean_scale(&self) -> f64 {
        self.affine().determinant().abs().sqrt()
    }
}

/// Tipo de nó (GD-GEO-03).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum NodeKind {
    #[default]
    Corner,
    Smooth,
    Symmetric,
}

/// Nó de caminho. `in`/`out` são as alças (pontos de controle absolutos,
/// em coordenadas locais). Segmento entre dois nós é reto se nenhuma das
/// alças envolvidas existir; senão é Bézier cúbica.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub p: [f64; 2],
    #[serde(default, rename = "in", skip_serializing_if = "Option::is_none")]
    pub handle_in: Option<[f64; 2]>,
    #[serde(default, rename = "out", skip_serializing_if = "Option::is_none")]
    pub handle_out: Option<[f64; 2]>,
    #[serde(default)]
    pub kind: NodeKind,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Node {
    pub fn corner(x: f64, y: f64) -> Node {
        Node {
            p: [x, y],
            handle_in: None,
            handle_out: None,
            kind: NodeKind::Corner,
            extra: Map::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Subpath {
    pub closed: bool,
    pub nodes: Vec<Node>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Regra de preenchimento (GD-GEO-04). Padrão: par-ímpar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum FillRule {
    #[default]
    Evenodd,
    Nonzero,
}

fn pt(p: [f64; 2]) -> Point {
    Point::new(p[0], p[1])
}

/// Converte subcaminhos de nós para um `BezPath` (coordenadas locais).
pub fn subpaths_to_bez(subpaths: &[Subpath]) -> BezPath {
    let mut path = BezPath::new();
    for sp in subpaths {
        let Some(first) = sp.nodes.first() else {
            continue;
        };
        path.move_to(pt(first.p));
        let n = sp.nodes.len();
        let seg_count = if sp.closed { n } else { n - 1 };
        for i in 0..seg_count {
            let a = &sp.nodes[i];
            let b = &sp.nodes[(i + 1) % n];
            if a.handle_out.is_none() && b.handle_in.is_none() {
                path.line_to(pt(b.p));
            } else {
                path.curve_to(
                    pt(a.handle_out.unwrap_or(a.p)),
                    pt(b.handle_in.unwrap_or(b.p)),
                    pt(b.p),
                );
            }
        }
        if sp.closed {
            path.close_path();
        }
    }
    path
}

/// Construtor incremental de subcaminhos a partir de comandos estilo SVG
/// (M/L/C/Z), usado pela importação.
#[derive(Default)]
pub struct SubpathBuilder {
    pub subpaths: Vec<Subpath>,
    current: Option<Subpath>,
}

impl SubpathBuilder {
    pub fn move_to(&mut self, p: [f64; 2]) {
        self.flush();
        self.current = Some(Subpath {
            closed: false,
            nodes: vec![Node::corner(p[0], p[1])],
            extra: Map::new(),
        });
    }

    fn cur(&mut self, p: [f64; 2]) -> &mut Subpath {
        if self.current.is_none() {
            self.move_to(p);
        }
        self.current.as_mut().unwrap()
    }

    pub fn line_to(&mut self, p: [f64; 2]) {
        self.cur(p).nodes.push(Node::corner(p[0], p[1]));
    }

    pub fn curve_to(&mut self, c1: [f64; 2], c2: [f64; 2], p: [f64; 2]) {
        let sp = self.cur(c1);
        if let Some(last) = sp.nodes.last_mut() {
            last.handle_out = Some(c1);
        }
        let mut n = Node::corner(p[0], p[1]);
        n.handle_in = Some(c2);
        sp.nodes.push(n);
    }

    pub fn quad_to(&mut self, c: [f64; 2], p: [f64; 2]) {
        let p0 = self
            .current
            .as_ref()
            .and_then(|s| s.nodes.last())
            .map(|n| n.p)
            .unwrap_or(c);
        let c1 = [
            p0[0] + 2.0 / 3.0 * (c[0] - p0[0]),
            p0[1] + 2.0 / 3.0 * (c[1] - p0[1]),
        ];
        let c2 = [
            p[0] + 2.0 / 3.0 * (c[0] - p[0]),
            p[1] + 2.0 / 3.0 * (c[1] - p[1]),
        ];
        self.curve_to(c1, c2, p);
    }

    pub fn close(&mut self) {
        if let Some(mut sp) = self.current.take() {
            sp.closed = true;
            // Nó final coincidente com o inicial: funde os dois.
            if sp.nodes.len() > 1 {
                let first = sp.nodes[0].p;
                let last = sp.nodes[sp.nodes.len() - 1].p;
                if (first[0] - last[0]).abs() < 1e-6 && (first[1] - last[1]).abs() < 1e-6 {
                    let last = sp.nodes.pop().unwrap();
                    if last.handle_in.is_some() {
                        sp.nodes[0].handle_in = last.handle_in;
                    }
                }
            }
            self.subpaths.push(sp);
        }
    }

    fn flush(&mut self) {
        if let Some(sp) = self.current.take() {
            self.subpaths.push(sp);
        }
    }

    pub fn finish(mut self) -> Vec<Subpath> {
        self.flush();
        self.subpaths
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kurbo::Shape;

    #[test]
    fn closed_square_has_area() {
        let mut b = SubpathBuilder::default();
        b.move_to([0.0, 0.0]);
        b.line_to([10.0, 0.0]);
        b.line_to([10.0, 10.0]);
        b.line_to([0.0, 10.0]);
        b.line_to([0.0, 0.0]);
        b.close();
        let sps = b.finish();
        assert_eq!(sps.len(), 1);
        assert_eq!(
            sps[0].nodes.len(),
            4,
            "nó final coincidente deve ser fundido"
        );
        assert!((subpaths_to_bez(&sps).area().abs() - 100.0).abs() < 1e-9);
    }

    #[test]
    fn transform_convention() {
        let t = Transform([2.0, 0.0, 0.0, 3.0, 5.0, 7.0]);
        let p = t.affine() * Point::new(1.0, 1.0);
        assert_eq!((p.x, p.y), (7.0, 10.0));
    }
}
