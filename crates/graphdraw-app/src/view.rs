//! Estado de zoom/pan da área de desenho, separado da interface para ser
//! testável. Coordenadas de tela em pontos lógicos, relativas ao canto
//! superior esquerdo da área de desenho; página em µm com Y para cima.

use kurbo::Affine;

/// Pontos lógicos por µm em "tamanho real" (96 pontos por polegada).
pub const ACTUAL_SIZE: f64 = 96.0 / 25_400.0;
pub const MIN_ZOOM: f64 = ACTUAL_SIZE * 0.01;
pub const MAX_ZOOM: f64 = ACTUAL_SIZE * 640.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct View {
    /// Pontos lógicos por µm.
    pub zoom: f64,
    /// Posição, na tela, do canto superior esquerdo da página.
    pub pan: (f64, f64),
}

impl Default for View {
    fn default() -> Self {
        View {
            zoom: ACTUAL_SIZE,
            pan: (0.0, 0.0),
        }
    }
}

impl View {
    /// Enquadra a página inteira na área, com margem.
    pub fn fit(page_w: f64, page_h: f64, canvas_w: f64, canvas_h: f64) -> View {
        let margin = 24.0;
        let avail_w = (canvas_w - 2.0 * margin).max(10.0);
        let avail_h = (canvas_h - 2.0 * margin).max(10.0);
        let zoom = (avail_w / page_w)
            .min(avail_h / page_h)
            .clamp(MIN_ZOOM, MAX_ZOOM);
        View {
            zoom,
            pan: (
                (canvas_w - page_w * zoom) / 2.0,
                (canvas_h - page_h * zoom) / 2.0,
            ),
        }
    }

    /// Tamanho real (1 mm na tela = 1 mm a 96 ppp), centrado na área.
    pub fn actual_size(page_w: f64, page_h: f64, canvas_w: f64, canvas_h: f64) -> View {
        let zoom = ACTUAL_SIZE;
        View {
            zoom,
            pan: (
                (canvas_w - page_w * zoom) / 2.0,
                (canvas_h - page_h * zoom) / 2.0,
            ),
        }
    }

    /// Página (µm, Y↑) → tela (pontos, Y↓).
    pub fn page_to_screen(&self, page_h: f64, x: f64, y: f64) -> (f64, f64) {
        (
            self.pan.0 + x * self.zoom,
            self.pan.1 + (page_h - y) * self.zoom,
        )
    }

    /// Tela → página.
    pub fn screen_to_page(&self, page_h: f64, sx: f64, sy: f64) -> (f64, f64) {
        (
            (sx - self.pan.0) / self.zoom,
            page_h - (sy - self.pan.1) / self.zoom,
        )
    }

    /// Aplica um fator de zoom mantendo fixo o ponto da página sob `anchor`.
    pub fn zoom_at(&mut self, factor: f64, anchor: (f64, f64)) {
        let new_zoom = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        let k = new_zoom / self.zoom;
        self.pan = (
            anchor.0 - (anchor.0 - self.pan.0) * k,
            anchor.1 - (anchor.1 - self.pan.1) * k,
        );
        self.zoom = new_zoom;
    }

    pub fn pan_by(&mut self, dx: f64, dy: f64) {
        self.pan.0 += dx;
        self.pan.1 += dy;
    }

    /// Matriz página→pixels físicos (para o rasterizador).
    pub fn raster_affine(&self, page_h: f64, pixels_per_point: f64) -> Affine {
        let s = self.zoom * pixels_per_point;
        Affine::new([
            s,
            0.0,
            0.0,
            -s,
            self.pan.0 * pixels_per_point,
            (self.pan.1 + page_h * self.zoom) * pixels_per_point,
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A4: (f64, f64) = (210_000.0, 297_000.0);

    #[test]
    fn fit_centers_page_inside_canvas() {
        let v = View::fit(A4.0, A4.1, 800.0, 600.0);
        let (x0, y0) = v.page_to_screen(A4.1, 0.0, A4.1);
        let (x1, y1) = v.page_to_screen(A4.1, A4.0, 0.0);
        assert!(x0 >= 0.0 && y0 >= 0.0 && x1 <= 800.0 && y1 <= 600.0);
        assert!(((x0 + x1) / 2.0 - 400.0).abs() < 1e-6);
        assert!(((y0 + y1) / 2.0 - 300.0).abs() < 1e-6);
    }

    #[test]
    fn zoom_keeps_point_under_cursor() {
        let mut v = View::fit(A4.0, A4.1, 800.0, 600.0);
        let cursor = (123.0, 456.0);
        let before = v.screen_to_page(A4.1, cursor.0, cursor.1);
        v.zoom_at(3.7, cursor);
        let after = v.screen_to_page(A4.1, cursor.0, cursor.1);
        assert!((before.0 - after.0).abs() < 1e-6 && (before.1 - after.1).abs() < 1e-6);
    }

    #[test]
    fn y_axis_up_and_roundtrip() {
        let v = View::actual_size(A4.0, A4.1, 1000.0, 1000.0);
        // Canto inferior esquerdo da página fica embaixo na tela.
        let (_, y_bottom) = v.page_to_screen(A4.1, 0.0, 0.0);
        let (_, y_top) = v.page_to_screen(A4.1, 0.0, A4.1);
        assert!(y_bottom > y_top);
        let (px, py) = v.screen_to_page(A4.1, 321.0, 654.0);
        let (sx, sy) = v.page_to_screen(A4.1, px, py);
        assert!((sx - 321.0).abs() < 1e-9 && (sy - 654.0).abs() < 1e-9);
        // 10 mm em tamanho real = 10 mm a 96 ppp ≈ 37,8 pontos.
        let (a, _) = v.page_to_screen(A4.1, 0.0, 0.0);
        let (b, _) = v.page_to_screen(A4.1, 10_000.0, 0.0);
        assert!((b - a - 37.795).abs() < 1e-3);
    }

    #[test]
    fn raster_affine_matches_screen_mapping() {
        let mut v = View::fit(A4.0, A4.1, 800.0, 600.0);
        v.pan_by(15.0, -7.0);
        let ppp = 1.5;
        let m = v.raster_affine(A4.1, ppp);
        let p = m * kurbo::Point::new(50_000.0, 80_000.0);
        let (sx, sy) = v.page_to_screen(A4.1, 50_000.0, 80_000.0);
        assert!((p.x - sx * ppp).abs() < 1e-6 && (p.y - sy * ppp).abs() < 1e-6);
    }

    #[test]
    fn zoom_is_clamped() {
        let mut v = View::default();
        v.zoom_at(1e9, (0.0, 0.0));
        assert_eq!(v.zoom, MAX_ZOOM);
        v.zoom_at(1e-12, (0.0, 0.0));
        assert_eq!(v.zoom, MIN_ZOOM);
    }
}
