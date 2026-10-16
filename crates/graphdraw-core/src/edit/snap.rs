//! Encaixe (GD-SNP-01..03, subconjunto do M1): grade, nós dos objetos,
//! cantos/meios/centro das caixas, centros de elipses e página (bordas,
//! cantos, centro). O raio é dado em µm pela interface a partir dos pixels
//! de tela (GD-SNP-02).

use kurbo::{Affine, Point, Rect};

use super::query::bounds;
use crate::model::{ObjectKind, Page};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapKind {
    Grid,
    Node,
    BoxCorner,
    BoxMidpoint,
    Center,
    PageCorner,
    PageEdge,
    PageCenter,
}

impl SnapKind {
    /// Rótulo curto para a indicação visual.
    pub fn label(self) -> &'static str {
        match self {
            SnapKind::Grid => "grade",
            SnapKind::Node => "nó",
            SnapKind::BoxCorner => "canto",
            SnapKind::BoxMidpoint => "ponto médio",
            SnapKind::Center => "centro",
            SnapKind::PageCorner => "canto da página",
            SnapKind::PageEdge => "borda da página",
            SnapKind::PageCenter => "centro da página",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SnapSettings {
    pub enabled: bool,
    /// Espaçamento da grade em µm; `None` desliga a grade.
    pub grid: Option<f64>,
    pub objects: bool,
    pub page: bool,
}

impl Default for SnapSettings {
    fn default() -> Self {
        SnapSettings {
            enabled: true,
            grid: None,
            objects: true,
            page: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Snapped {
    pub point: Point,
    pub kind: SnapKind,
}

/// Pontos de encaixe de uma página, ignorando objetos em `exclude`.
pub fn candidates(
    page: &Page,
    settings: &SnapSettings,
    exclude: &[String],
) -> Vec<(Point, SnapKind)> {
    let mut out = Vec::new();
    if settings.page {
        let r = Rect::new(0.0, 0.0, page.width, page.height);
        for p in [
            Point::new(r.x0, r.y0),
            Point::new(r.x1, r.y0),
            Point::new(r.x0, r.y1),
            Point::new(r.x1, r.y1),
        ] {
            out.push((p, SnapKind::PageCorner));
        }
        for p in [
            Point::new(r.center().x, r.y0),
            Point::new(r.center().x, r.y1),
            Point::new(r.x0, r.center().y),
            Point::new(r.x1, r.center().y),
        ] {
            out.push((p, SnapKind::PageEdge));
        }
        out.push((r.center(), SnapKind::PageCenter));
    }
    if settings.objects {
        for layer in page.layers.iter().filter(|l| l.visible) {
            for o in layer
                .objects
                .iter()
                .filter(|o| o.visible && !exclude.contains(&o.id))
            {
                let m = o.transform.affine();
                if let ObjectKind::Path(p) = &o.kind {
                    for sp in &p.subpaths {
                        for n in &sp.nodes {
                            out.push((m * Point::new(n.p[0], n.p[1]), SnapKind::Node));
                        }
                    }
                }
                if let ObjectKind::Ellipse(_) = &o.kind {
                    out.push((m * Point::ORIGIN, SnapKind::Center));
                }
                if let Some(b) = bounds(o, Affine::IDENTITY, false) {
                    for p in [
                        Point::new(b.x0, b.y0),
                        Point::new(b.x1, b.y0),
                        Point::new(b.x0, b.y1),
                        Point::new(b.x1, b.y1),
                    ] {
                        out.push((p, SnapKind::BoxCorner));
                    }
                    for p in [
                        Point::new(b.center().x, b.y0),
                        Point::new(b.center().x, b.y1),
                        Point::new(b.x0, b.center().y),
                        Point::new(b.x1, b.center().y),
                    ] {
                        out.push((p, SnapKind::BoxMidpoint));
                    }
                    out.push((b.center(), SnapKind::Center));
                }
            }
        }
    }
    out
}

/// Encaixa `p` no candidato mais próximo dentro de `radius`; a grade entra
/// como último recurso (pontos específicos têm prioridade).
pub fn snap(
    p: Point,
    cands: &[(Point, SnapKind)],
    settings: &SnapSettings,
    radius: f64,
) -> Option<Snapped> {
    if !settings.enabled {
        return None;
    }
    let best = cands
        .iter()
        .map(|(c, k)| ((*c - p).hypot(), *c, *k))
        .filter(|(d, _, _)| *d <= radius)
        .min_by(|a, b| a.0.total_cmp(&b.0));
    if let Some((_, point, kind)) = best {
        return Some(Snapped { point, kind });
    }
    let g = settings.grid.filter(|g| *g > 0.0)?;
    let q = Point::new((p.x / g).round() * g, (p.y / g).round() * g);
    ((q - p).hypot() <= radius).then_some(Snapped {
        point: q,
        kind: SnapKind::Grid,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Object, RectData, Transform};
    use serde_json::Map;

    #[test]
    fn gd_snp_01_objects_page_grid_priority() {
        let mut page = Page::new(100.0, 100.0);
        let mut r = Object::new(ObjectKind::Rectangle(RectData {
            width: 20.0,
            height: 10.0,
            corners: None,
            extra: Map::new(),
        }));
        r.transform = Transform::translate(30.0, 30.0);
        let id = r.id.clone();
        page.layers[0].objects.push(r);
        let mut s = SnapSettings {
            grid: Some(7.0),
            ..Default::default()
        };
        let c = candidates(&page, &s, &[]);
        let hit = snap(Point::new(50.8, 40.5), &c, &s, 2.0).unwrap();
        assert_eq!(
            (hit.point, hit.kind),
            (Point::new(50.0, 40.0), SnapKind::BoxCorner)
        );
        let hit = snap(Point::new(40.3, 35.2), &c, &s, 2.0).unwrap();
        assert_eq!(hit.kind, SnapKind::Center);
        let hit = snap(Point::new(99.5, 50.4), &c, &s, 2.0).unwrap();
        assert_eq!(
            (hit.point, hit.kind),
            (Point::new(100.0, 50.0), SnapKind::PageEdge)
        );
        let hit = snap(Point::new(70.4, 14.3), &c, &s, 2.0).unwrap();
        assert_eq!(
            (hit.point, hit.kind),
            (Point::new(70.0, 14.0), SnapKind::Grid)
        );
        assert!(
            snap(Point::new(73.5, 17.5), &c, &s, 2.0).is_none(),
            "fora do raio"
        );
        // Objeto arrastado não encaixa nele mesmo.
        let c2 = candidates(&page, &s, &[id]);
        assert_ne!(
            snap(Point::new(50.8, 40.5), &c2, &s, 2.0).map(|h| h.kind),
            Some(SnapKind::BoxCorner)
        );
        // GD-SNP-03: desligar global.
        s.enabled = false;
        assert!(snap(Point::new(50.8, 40.5), &c, &s, 2.0).is_none());
    }
}
