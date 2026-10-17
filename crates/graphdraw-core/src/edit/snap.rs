//! Encaixe (GD-SNP-01..03, subconjunto do M1): grade, nós dos objetos,
//! cantos/meios/centro das caixas, centros de elipses e página (bordas,
//! cantos, centro). O raio é dado em µm pela interface a partir dos pixels
//! de tela (GD-SNP-02).

use kurbo::{Affine, Point, Rect};

use super::query::bounds;
use crate::model::{Document, Guide, ObjectKind, Page};

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
    /// Sobre uma guia.
    Guide,
    /// Cruzamento de duas guias.
    GuideCross,
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
            SnapKind::Guide => "guia",
            SnapKind::GuideCross => "cruzamento de guias",
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
    /// Encaixar nas guias (GD-SNP-01).
    pub guides: bool,
}

impl Default for SnapSettings {
    fn default() -> Self {
        SnapSettings {
            enabled: true,
            grid: None,
            objects: true,
            page: true,
            guides: true,
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

/// Guias que valem na página: as dela e as do documento.
pub fn page_guides(doc: &Document, page: usize) -> Vec<Guide> {
    let mut g = doc.data.settings.guides.clone();
    if let Some(p) = doc.data.pages.get(page) {
        g.extend(p.guides.iter().cloned());
    }
    g
}

/// Candidatos da página mais os cruzamentos das guias (documento + página).
pub fn candidates_doc(
    doc: &Document,
    page: usize,
    settings: &SnapSettings,
    exclude: &[String],
) -> Vec<(Point, SnapKind)> {
    let Some(pg) = doc.data.pages.get(page) else {
        return Vec::new();
    };
    let mut out = candidates(pg, settings, exclude);
    if settings.guides {
        let g = page_guides(doc, page);
        for i in 0..g.len() {
            for j in i + 1..g.len() {
                if let Some(p) = g[i].intersect(&g[j]) {
                    out.push((p, SnapKind::GuideCross));
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
    snap_guided(p, cands, &[], settings, radius)
}

/// [`snap_guided`] com as guias primeiro (forma usada pela interface).
pub fn snap_guided_on(
    guides: &[Guide],
    p: Point,
    cands: &[(Point, SnapKind)],
    settings: &SnapSettings,
    radius: f64,
) -> Option<Snapped> {
    snap_guided(p, cands, guides, settings, radius)
}

/// Como [`snap`], considerando também as linhas das guias: pontos
/// primeiro, depois guias, por fim a grade.
pub fn snap_guided(
    p: Point,
    cands: &[(Point, SnapKind)],
    guides: &[Guide],
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
    if settings.guides
        && let Some((_, point)) = guides
            .iter()
            .map(|g| (g.distance(p), g.project(p)))
            .filter(|(d, _)| *d <= radius)
            .min_by(|a, b| a.0.total_cmp(&b.0))
    {
        return Some(Snapped {
            point,
            kind: SnapKind::Guide,
        });
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

    /// GD-GUI-01 + GD-SNP-01: guias h/v/inclinada, cruzamento e prioridade.
    #[test]
    fn guides_snap_lines_and_crossings() {
        let mut d = Document::new();
        let mut p = Page::new(100.0, 100.0);
        p.guides.push(Guide::vertical(40.0));
        d.data.pages.push(p);
        d.data.settings.guides.push(Guide::horizontal(60.0));
        let s = SnapSettings {
            page: false,
            objects: false,
            ..Default::default()
        };
        let g = page_guides(&d, 0);
        let c = candidates_doc(&d, 0, &s, &[]);
        assert_eq!(c, vec![(Point::new(40.0, 60.0), SnapKind::GuideCross)]);
        // Perto do cruzamento: o ponto vence a linha.
        let hit = snap_guided(Point::new(41.0, 59.5), &c, &g, &s, 2.0).unwrap();
        assert_eq!(hit.kind, SnapKind::GuideCross);
        // Longe do cruzamento: projeta na guia vertical.
        let hit = snap_guided(Point::new(41.0, 10.0), &c, &g, &s, 2.0).unwrap();
        assert_eq!(
            (hit.point, hit.kind),
            (Point::new(40.0, 10.0), SnapKind::Guide)
        );
        // Inclinada a 45° pela origem.
        let a = Guide::angled(Point::ORIGIN, 45.0);
        let q = a.project(Point::new(10.0, 0.0));
        assert!((q.x - 5.0).abs() < 1e-9 && (q.y - 5.0).abs() < 1e-9);
        let (p0, p1) = a.clip(kurbo::Rect::new(0.0, 0.0, 100.0, 50.0)).unwrap();
        assert!(
            (p0 - Point::ORIGIN).hypot() < 1e-9 && (p1 - Point::new(50.0, 50.0)).hypot() < 1e-9
        );
        // Sem guias ligadas: nada.
        let off = SnapSettings { guides: false, ..s };
        assert!(snap_guided(Point::new(41.0, 10.0), &[], &g, &off, 2.0).is_none());
    }
}
