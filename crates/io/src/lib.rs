//! graphdraw-io: export the document model to interchange formats and read the
//! native `.gdr` file.

#![allow(clippy::field_reassign_with_default)]
pub mod dxf;
pub mod emf;
pub mod eps;
pub mod eps_import;
pub mod ggr;
pub mod html;
pub mod pdf;
pub mod pdf_crypt;
pub mod pdf_import;
pub mod plt;
pub mod psd;
pub mod svg;
pub mod svg_import;
pub mod text_import;

use std::path::Path;
use graphdraw_core::Document;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Core(#[from] graphdraw_core::Error),
    #[error("{0}")]
    Format(String),
}

pub type Result<T> = std::result::Result<T, Error>;

/// The document as vector exports should see it: unless the document
/// fills open curves, an open curve's fill is kept for its closed subpaths
/// only (the curve becomes a group of the filled closed part and the
/// outlined whole), or dropped when it has none, as the screen shows it.
pub fn resolve_open_fills(doc: &Document) -> std::borrow::Cow<'_, Document> {
    use graphdraw_core::document::{Shape, ShapeKind};
    use graphdraw_core::Fill;
    fn needs(s: &Shape) -> bool {
        match &s.kind {
            ShapeKind::Path { closed: false, .. } => !matches!(s.fill, Fill::None),
            ShapeKind::Group { children } => children.iter().any(needs),
            ShapeKind::ClipFrame { frame, contents } => needs(frame) || contents.iter().any(needs),
            _ => false,
        }
    }
    fn fix(s: &mut Shape) {
        match &mut s.kind {
            ShapeKind::Path {
                path,
                closed: false,
            } if !matches!(s.fill, Fill::None) => {
                let closed = graphdraw_render::closed_subpaths(path);
                if closed.elements().is_empty() {
                    s.fill = Fill::None;
                    return;
                }
                let mut filled = s.clone();
                filled.kind = ShapeKind::Path {
                    path: closed,
                    closed: true,
                };
                filled.stroke = None;
                filled.effects.clear();
                filled.transform = graphdraw_core::geometry::Affine::IDENTITY;
                let mut outlined = s.clone();
                outlined.fill = Fill::None;
                outlined.effects.clear();
                outlined.transform = graphdraw_core::geometry::Affine::IDENTITY;
                s.kind = ShapeKind::Group {
                    children: vec![filled, outlined],
                };
                s.fill = Fill::None;
                s.stroke = None;
            }
            ShapeKind::Group { children } => children.iter_mut().for_each(fix),
            ShapeKind::ClipFrame { frame, contents } => {
                fix(frame);
                contents.iter_mut().for_each(fix);
            }
            _ => {}
        }
    }
    if doc.metadata.fill_open_curves || !doc.all_layers().flat_map(|l| &l.shapes).any(needs) {
        return std::borrow::Cow::Borrowed(doc);
    }
    let mut out = doc.clone();
    for page in &mut out.pages {
        for layer in &mut page.layers {
            layer.shapes.iter_mut().for_each(fix);
        }
    }
    for layer in &mut out.master {
        layer.shapes.iter_mut().for_each(fix);
    }
    std::borrow::Cow::Owned(out)
}

/// Native format: pretty JSON with a `.gdr` extension.
pub fn save_native(doc: &Document, path: impl AsRef<Path>) -> Result<()> {
    std::fs::write(path, doc.to_json()?)?;
    Ok(())
}

pub fn load_native(path: impl AsRef<Path>) -> Result<Document> {
    let s = std::fs::read_to_string(path)?;
    Ok(Document::from_json(&s)?)
}

/// Write every page to a PDF file.
pub fn save_pdf(doc: &Document, path: impl AsRef<Path>) -> Result<()> {
    std::fs::write(path, pdf::document_to_pdf(doc))?;
    Ok(())
}

/// Write one page as an ASCII DXF file.
pub fn save_dxf(doc: &Document, page_index: usize, path: impl AsRef<Path>) -> Result<()> {
    std::fs::write(path, dxf::page_to_dxf(doc, page_index))?;
    Ok(())
}

/// Write one page as an HPGL plotter file.
pub fn save_plt(doc: &Document, page_index: usize, path: impl AsRef<Path>) -> Result<()> {
    std::fs::write(path, plt::page_to_plt(doc, page_index))?;
    Ok(())
}

/// Write one page as a Photoshop file (one raster layer per document layer).
pub fn save_psd(doc: &Document, page_index: usize, dpi: f64, path: impl AsRef<Path>) -> Result<()> {
    let bytes = psd::page_to_psd(doc, page_index, dpi)
        .ok_or_else(|| Error::Format("page could not be rendered".into()))?;
    std::fs::write(path, bytes)?;
    Ok(())
}

/// Write one page as an enhanced metafile (EMF).
pub fn save_emf(doc: &Document, page_index: usize, path: impl AsRef<Path>) -> Result<()> {
    std::fs::write(path, emf::page_to_emf(doc, page_index))?;
    Ok(())
}

/// Write one page as a placeable Windows metafile (WMF).
pub fn save_wmf(doc: &Document, page_index: usize, path: impl AsRef<Path>) -> Result<()> {
    std::fs::write(path, emf::page_to_wmf(doc, page_index))?;
    Ok(())
}

/// Write the first page (or `page_index`) as an SVG file.
pub fn save_svg(doc: &Document, page_index: usize, path: impl AsRef<Path>) -> Result<()> {
    std::fs::write(path, svg::page_to_svg(doc, page_index))?;
    Ok(())
}

/// A PNG picture of each page, its longer side `max_side` pixels on a
/// white background, as `(png, width, height)` in page order (the
/// pictures `.cdr` packages store with the drawing). Pages that cannot
/// be rendered are left out of the list's end.
pub fn page_pictures(doc: &Document, max_side: u32) -> Vec<(Vec<u8>, u32, u32)> {
    let mut out = Vec::new();
    for page in &doc.pages {
        let longest = page.size.width.max(page.size.height);
        if !longest.is_finite() || longest <= 0.0 {
            break;
        }
        // Pixels per inch that make the longer side `max_side` pixels.
        let dpi = f64::from(max_side.max(1)) / (longest / 25.4);
        let Some(pm) = graphdraw_render::render_page_image(doc, page.id, dpi) else {
            break;
        };
        let (w, h) = (pm.width(), pm.height());
        let mut rgba = Vec::with_capacity((w * h * 4) as usize);
        for p in pm.pixels() {
            let c = p.demultiply();
            rgba.extend_from_slice(&[c.red(), c.green(), c.blue(), c.alpha()]);
        }
        let png = svg::encode_png(w, h, &rgba);
        if png.is_empty() {
            break;
        }
        out.push((png, w, h));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use graphdraw_core::{
        document::{Shape, ShapeKind},
        geometry::{Rect, Size},
        Color, Fill, Pattern, Stroke,
    };

    #[test]
    fn open_curves_lose_their_fill_in_vector_exports() {
        use graphdraw_core::geometry::{BezPath, Point};
        let mut doc = Document::default();
        let layer = doc.pages[0].layers[0].id;
        let mut path = BezPath::new();
        path.move_to(Point::new(10.0, 10.0));
        path.line_to(Point::new(50.0, 10.0));
        path.line_to(Point::new(50.0, 50.0));
        let mut open = Shape::new(
            graphdraw_core::ShapeId(5),
            ShapeKind::Path {
                path,
                closed: false,
            },
        );
        open.fill = Fill::Solid(Color::rgb8(255, 0, 0));
        open.stroke = Some(Stroke::hairline(Color::BLACK));
        doc.layer_mut(layer).unwrap().shapes.push(open);
        let resolved = resolve_open_fills(&doc);
        let s = &resolved.pages[0].layers[0].shapes[0];
        assert!(matches!(s.fill, Fill::None));
        assert!(s.stroke.is_some());
        let svg = svg::page_to_svg(&doc, 0);
        assert!(!svg.contains("fill=\"#ff0000\""), "{svg}");
        // A document that fills open curves exports the fill.
        doc.metadata.fill_open_curves = true;
        assert!(matches!(
            resolve_open_fills(&doc),
            std::borrow::Cow::Borrowed(_)
        ));
    }

    #[test]
    fn vector_pattern_fill_round_trips_through_the_native_format() {
        let mut doc = Document::default();
        let layer = doc.pages[0].layers[0].id;
        // A tile with a filled square and an outlined group, built from
        // page-space shapes so the helper has to move them to the origin.
        let mut square = Shape::new(
            graphdraw_core::ShapeId(7),
            ShapeKind::Rect {
                rect: Rect::new(40.0, 40.0, 45.0, 45.0),
                radius: 0.0,
                corners: None,
            },
        );
        square.fill = Fill::Solid(Color::rgb8(255, 0, 0));
        square.stroke = None;
        let mut circle = Shape::new(
            graphdraw_core::ShapeId(8),
            ShapeKind::Ellipse {
                rect: Rect::new(45.0, 45.0, 50.0, 50.0),
                arc: None,
            },
        );
        circle.fill = Fill::linear(Color::rgb8(0, 0, 255), Color::WHITE, 45.0);
        circle.stroke = Some(Stroke::new(Color::BLACK, 0.5));
        let group = Shape::new(
            graphdraw_core::ShapeId(9),
            ShapeKind::Group {
                children: vec![circle],
            },
        );
        let pattern = Pattern::vector_from_shapes(&[square, group]);
        assert!(matches!(&pattern, Pattern::Vector { tile, .. } if *tile == Size::new(10.0, 10.0)));
        let id = doc.ids_mut().shape();
        let mut s = Shape::new(
            id,
            ShapeKind::Rect {
                rect: Rect::new(10.0, 10.0, 60.0, 60.0),
                radius: 0.0,
                corners: None,
            },
        );
        s.fill = Fill::Pattern(pattern);
        doc.layer_mut(layer).unwrap().shapes.push(s);

        let path = std::env::temp_dir().join(format!(
            "graphdraw-vector-pattern-{}-{:?}.gdr",
            std::process::id(),
            std::thread::current().id()
        ));
        save_native(&doc, &path).unwrap();
        let back = load_native(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!(back, doc);
        let shapes = &back.pages[0].layers[0].shapes;
        assert!(matches!(
            &shapes[0].fill,
            Fill::Pattern(Pattern::Vector { shapes, .. }) if shapes.len() == 2
        ));
    }

    #[test]
    fn custom_arrowhead_round_trips_through_the_native_format() {
        let mut doc = Document::default();
        let layer = doc.pages[0].layers[0].id;
        let mut tri = graphdraw_core::BezPath::new();
        tri.move_to((0.0, 0.0));
        tri.line_to((20.0, 10.0));
        tri.line_to((0.0, 20.0));
        tri.close_path();
        let id = doc.ids_mut().shape();
        let mut line = Shape::new(
            id,
            ShapeKind::Path {
                path: {
                    let mut p = graphdraw_core::BezPath::new();
                    p.move_to((10.0, 50.0));
                    p.line_to((60.0, 50.0));
                    p
                },
                closed: false,
            },
        );
        line.stroke = Some(Stroke {
            start_arrow: graphdraw_core::Arrowhead::Circle,
            end_arrow: graphdraw_core::Arrowhead::from_shape_path(&tri, "My head"),
            ..Stroke::new(Color::BLACK, 1.0)
        });
        doc.layer_mut(layer).unwrap().shapes.push(line);
        let json = doc.to_json().unwrap();
        assert!(json.contains("\"custom\""));
        assert!(json.contains("My head"));
        let back = Document::from_json(&json).unwrap();
        assert_eq!(back, doc);
        let s = &back.pages[0].layers[0].shapes[0];
        let st = s.stroke.as_ref().unwrap();
        assert!(st.end_arrow.is_custom());
        assert_eq!(st.end_arrow.name(), "My head");
        assert_eq!(st.start_arrow, graphdraw_core::Arrowhead::Circle);
    }
}
