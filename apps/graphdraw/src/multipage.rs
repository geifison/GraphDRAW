//! Multipage View (View > Multipage View): every page of the drawing laid
//! out in a grid on the desktop. The current page is edited as always;
//! the others are drawn beside it, and a click on one of them makes it
//! the current page without moving anything on screen.

use crate::app::App;
use crate::view::View;
use egui::{Color32, Painter, Pos2, Rect as ERect};
use tracedraw_core::geometry::{Rect, Size};
use tracedraw_core::PageId;

/// Space between pages, in millimetres.
pub const GAP_MM: f64 = 30.0;

/// The pages' places in the current page's coordinates (mm, Y up): a grid
/// with as many columns as the square root of the page count (rounded
/// up), left to right then down, each page at the top left of its cell.
/// The current page is at the origin; an unknown `active` keeps the grid
/// where the first page is at the origin.
pub fn layout(pages: &[(PageId, Size)], active: PageId) -> Vec<(PageId, Rect)> {
    let n = pages.len();
    if n == 0 {
        return Vec::new();
    }
    let cols = ((n as f64).sqrt().ceil() as usize).max(1);
    let rows = n.div_ceil(cols);
    let mut col_w = vec![0.0f64; cols];
    let mut row_h = vec![0.0f64; rows];
    for (i, (_, size)) in pages.iter().enumerate() {
        let (r, c) = (i / cols, i % cols);
        if let Some(w) = col_w.get_mut(c) {
            *w = w.max(size.width.max(0.0));
        }
        if let Some(h) = row_h.get_mut(r) {
            *h = h.max(size.height.max(0.0));
        }
    }
    let mut placed: Vec<(PageId, Rect)> = Vec::with_capacity(n);
    for (i, (id, size)) in pages.iter().enumerate() {
        let (r, c) = (i / cols, i % cols);
        let x0: f64 = col_w.iter().take(c).sum::<f64>() + c as f64 * GAP_MM;
        let top: f64 = -(row_h.iter().take(r).sum::<f64>() + r as f64 * GAP_MM);
        let w = size.width.max(0.0);
        let h = size.height.max(0.0);
        placed.push((*id, Rect::new(x0, top - h, x0 + w, top)));
    }
    let (dx, dy) = placed
        .iter()
        .find(|(id, _)| *id == active)
        .or(placed.first())
        .map(|(_, r)| (r.x0, r.y0))
        .unwrap_or((0.0, 0.0));
    placed
        .into_iter()
        .map(|(id, r)| (id, Rect::new(r.x0 - dx, r.y0 - dy, r.x1 - dx, r.y1 - dy)))
        .collect()
}

/// The page under a page-space point, if any.
pub fn page_at(placed: &[(PageId, Rect)], p: tracedraw_core::geometry::Point) -> Option<PageId> {
    placed
        .iter()
        .find(|(_, r)| p.x >= r.x0 && p.x <= r.x1 && p.y >= r.y0 && p.y <= r.y1)
        .map(|(id, _)| *id)
}

/// Everything the grid covers, for Zoom to Fit in this view.
pub fn bounds(placed: &[(PageId, Rect)]) -> Option<Rect> {
    let mut it = placed.iter().map(|(_, r)| *r);
    let first = it.next()?;
    Some(it.fold(first, |a, b| a.union(b)))
}

impl App {
    /// The grid of pages around the current one.
    pub fn multipage_layout(&self) -> Vec<(PageId, Rect)> {
        let pages: Vec<(PageId, Size)> = self.doc().pages.iter().map(|p| (p.id, p.size)).collect();
        layout(&pages, self.page)
    }

    /// Turn the Multipage View on or off; turning it on shows every page.
    pub fn set_multipage(&mut self, on: bool) {
        self.multipage = on;
        if on {
            self.page_sorter = false;
        }
        self.fit_pending = true;
    }

    /// Make `page` the current page and keep every page where it is on
    /// screen: the view moves by the page's place in the grid.
    pub fn multipage_activate(&mut self, page: PageId) {
        if page == self.page {
            return;
        }
        let Some(r) = self
            .multipage_layout()
            .into_iter()
            .find(|(id, _)| *id == page)
            .map(|(_, r)| r)
        else {
            return;
        };
        self.view.origin = self
            .view
            .to_screen(tracedraw_core::geometry::Point::new(r.x0, r.y0));
        self.page = page;
        self.selection.clear();
        self.focus.clear();
        self.picked_layer = None;
    }

    /// In the Multipage View, a press on another page makes it current
    /// before the tool sees the press. True when the page changed.
    pub fn multipage_press(&mut self, s: Pos2) -> bool {
        if !self.multipage {
            return false;
        }
        let p = self.view.to_page(s);
        match page_at(&self.multipage_layout(), p) {
            Some(id) if id != self.page => {
                self.multipage_activate(id);
                true
            }
            _ => false,
        }
    }

    /// What Zoom to Fit and a new view fit: the page, or every page in the
    /// Multipage View.
    pub fn fit_rect(&self) -> Rect {
        if self.multipage {
            if let Some(b) = bounds(&self.multipage_layout()) {
                return b;
            }
        }
        self.page_rect()
    }
}

/// Paints the pages other than the current one: shadow, paper, objects
/// (each page kept as its own texture) and the page name below.
pub fn draw_other_pages(app: &App, painter: &Painter, rect: ERect) {
    let placed = app.multipage_layout();
    let doc = app.doc();
    let ctx = painter.ctx();
    let mut rasters = app.page_rasters.borrow_mut();
    rasters.retain(|id, _| doc.pages.iter().any(|p| p.id == *id));
    for (index, (id, r)) in placed.iter().enumerate() {
        let paper = app.view.rect_to_screen(*r);
        let paper = ERect::from_min_max(paper.min.round(), paper.max.round());
        if *id != app.page && paper.expand(30.0).intersects(rect) {
            if app.show_page_border {
                painter.rect_filled(
                    paper
                        .expand(1.0)
                        .translate(crate::theme::Tokens::PAGE_SHADOW_OFFSET),
                    0.0,
                    crate::theme::Tokens::page_shadow(),
                );
            }
            painter.rect_filled(paper, 0.0, crate::theme::Tokens::PAGE);
            let clip = paper.intersect(rect);
            if clip.width() >= 1.0 && clip.height() >= 1.0 {
                let view = View {
                    zoom: app.view.zoom,
                    origin: paper.left_bottom(),
                };
                let raster = rasters.entry(*id).or_default();
                if let Some(tex) = raster.texture(
                    ctx,
                    doc,
                    *id,
                    &view,
                    clip,
                    None,
                    app.engine.revision(),
                    app.wireframe,
                    app.simulate_overprints,
                    app.rasterize_complex_effects,
                    app.proof_colors,
                    None,
                    None,
                ) {
                    let (tex, _) = tex;
                    painter.image(
                        tex,
                        clip,
                        ERect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                        Color32::WHITE,
                    );
                }
            }
            if app.show_page_border {
                painter.rect_stroke(
                    paper,
                    0.0,
                    egui::Stroke::new(1.0, crate::theme::Tokens::page_border()),
                    egui::StrokeKind::Outside,
                );
            }
        }
        if paper.expand(30.0).intersects(rect) {
            let name = doc
                .pages
                .get(index)
                .map(|p| p.name.clone())
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| {
                    crate::i18n::trf("doc.page_n", &[("n", &(index + 1).to_string())])
                });
            let color = if *id == app.page {
                crate::theme::Tokens::text()
            } else {
                crate::theme::Tokens::text_dim()
            };
            painter.text(
                paper.left_bottom() + egui::vec2(0.0, 6.0),
                egui::Align2::LEFT_TOP,
                format!("{}: {name}", index + 1),
                egui::FontId::proportional(12.0),
                color,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracedraw_core::geometry::Point;

    fn pages(n: usize) -> Vec<(PageId, Size)> {
        (0..n)
            .map(|i| (PageId(i as u64 + 1), Size::new(100.0, 50.0)))
            .collect()
    }

    #[test]
    fn pages_sit_in_a_grid_with_the_current_one_at_the_origin() {
        let ps = pages(4);
        let placed = layout(&ps, PageId(1));
        // Two columns, two rows, 30 mm apart, rows going down.
        assert_eq!(placed[0].1, Rect::new(0.0, 0.0, 100.0, 50.0));
        assert_eq!(placed[1].1, Rect::new(130.0, 0.0, 230.0, 50.0));
        assert_eq!(placed[2].1, Rect::new(0.0, -80.0, 100.0, -30.0));
        // Another page current: the same grid, moved so it is at the origin.
        let placed = layout(&ps, PageId(4));
        assert_eq!(placed[3].1, Rect::new(0.0, 0.0, 100.0, 50.0));
        assert_eq!(placed[0].1, Rect::new(-130.0, 80.0, -30.0, 130.0));
        assert_eq!(page_at(&placed, Point::new(-50.0, 100.0)), Some(PageId(1)));
        assert_eq!(page_at(&placed, Point::new(-20.0, 100.0)), None);
        assert_eq!(bounds(&placed), Some(Rect::new(-130.0, 0.0, 100.0, 130.0)));
        assert!(layout(&[], PageId(1)).is_empty());
    }

    #[test]
    fn pages_of_other_sizes_take_the_widest_and_tallest_cell() {
        let ps = vec![
            (PageId(1), Size::new(100.0, 50.0)),
            (PageId(2), Size::new(40.0, 80.0)),
            (PageId(3), Size::new(150.0, 30.0)),
        ];
        let placed = layout(&ps, PageId(1));
        // Column 0 is as wide as page 3; row 0 as tall as page 2, whose
        // top lines up with page 1's.
        assert_eq!(placed[1].1, Rect::new(180.0, -30.0, 220.0, 50.0));
        assert_eq!(placed[2].1, Rect::new(0.0, -90.0, 150.0, -60.0));
    }

    #[test]
    fn a_click_on_another_page_makes_it_current_without_moving_the_view() {
        let mut app = App::headless();
        app.add_page();
        let first = app.doc().pages[0].id;
        let second = app.doc().pages[1].id;
        app.page = first;
        app.set_multipage(true);
        app.fit_pending = false;
        let placed = app.multipage_layout();
        let r = placed[1].1;
        let on_second = app.view.to_screen(Point::new(r.x0 + 5.0, r.y0 + 5.0));
        let before = app.view.to_screen(Point::new(r.x0, r.y0));
        // A press on the current page changes nothing.
        let on_first = app.view.to_screen(Point::new(5.0, 5.0));
        assert!(!app.multipage_press(on_first));
        assert!(app.multipage_press(on_second));
        assert_eq!(app.page, second);
        // The second page's corner is now the origin, at the same place.
        let after = app.view.to_screen(Point::new(0.0, 0.0));
        assert!((after - before).length() < 1e-3);
        let p = app.view.to_page(on_second);
        assert!((p.x - 5.0).abs() < 1e-3 && (p.y - 5.0).abs() < 1e-3);
        // Fit covers both pages.
        assert!(app.fit_rect().width() > app.page_rect().width());
        // Off: a press does nothing, Fit is the page again.
        app.set_multipage(false);
        assert!(!app.multipage_press(on_first));
        assert_eq!(app.fit_rect(), app.page_rect());
    }
}
