//! Janelas da saída para corte (doc 17): exportar PLT/DXF com as opções de
//! corte e prévia animada do percurso (GD-CRT-06), e gerar linha de corte ao
//! redor da seleção (GD-CRT-02).

use eframe::egui::{self, Pos2, Rect, Vec2};
use graphdraw_core::export::cut::{self, CutOptions, Direction};
use kurbo::Point;

use crate::doc::DocState;
use crate::i18n::tr;
use crate::theme;
use crate::widgets;

/// Formato do arquivo de corte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CutFormat {
    Plt,
    Dxf,
}

impl CutFormat {
    pub fn extension(self) -> &'static str {
        match self {
            CutFormat::Plt => "plt",
            CutFormat::Dxf => "dxf",
        }
    }
}

pub enum Outcome {
    Open,
    Cancel,
    /// Salvar com estas opções.
    Save(CutFormat, CutOptions),
}

pub struct CutExport {
    pub format: CutFormat,
    pub opts: CutOptions,
    weed: bool,
    weed_margin: f64,
    /// Início da animação da prévia (tempo do egui, em segundos).
    started: Option<f64>,
}

impl CutExport {
    pub fn new(format: CutFormat) -> CutExport {
        CutExport {
            format,
            opts: CutOptions::default(),
            weed: false,
            weed_margin: 5_000.0,
            started: None,
        }
    }

    pub fn show(&mut self, ctx: &egui::Context, doc: &DocState) -> Outcome {
        let mut out = Outcome::Open;
        let title = match self.format {
            CutFormat::Plt => tr(
                "Exportar para plotter (PLT/HPGL)",
                "Export for plotter (PLT/HPGL)",
            ),
            CutFormat::Dxf => tr(
                "Exportar para laser/CNC (DXF)",
                "Export for laser/CNC (DXF)",
            ),
        };
        widgets::modal(ctx, egui::Id::unique("exportar-corte")).show(ctx, |ui| {
            ui.heading(title);
            ui.add_space(theme::SPACE_300);
            let o = &mut self.opts;
            ui.checkbox(
                &mut o.only_marked,
                format!(
                    "{} \"{}\"",
                    tr(
                        "Só o marcado para corte: cor especial ou camada",
                        "Only what is marked for cut: spot color or layer"
                    ),
                    o.cut_name
                ),
            );
            ui.checkbox(
                &mut o.inside_first,
                tr(
                    "Furos antes do contorno externo",
                    "Holes before the outer contour",
                ),
            );
            ui.horizontal(|ui| {
                ui.label(tr("Sentido", "Direction"));
                ui.radio_value(
                    &mut o.direction,
                    Direction::Keep,
                    tr("Como desenhado", "As drawn"),
                );
                ui.radio_value(
                    &mut o.direction,
                    Direction::Clockwise,
                    tr("Horário", "Clockwise"),
                );
                ui.radio_value(
                    &mut o.direction,
                    Direction::CounterClockwise,
                    tr("Anti-horário", "Counter-clockwise"),
                );
            });
            egui::Grid::new("opcoes-corte")
                .num_columns(2)
                .show(ui, |ui| {
                    ui.label(tr("Sobreposição (overcut)", "Overcut"));
                    if let Some(v) = widgets::length_field(ui, "corte-overcut", o.overcut, None) {
                        o.overcut = v.max(0.0);
                    }
                    ui.end_row();
                    ui.label(tr("Compensação de lâmina", "Blade offset"));
                    if let Some(v) = widgets::length_field(ui, "corte-lamina", o.blade_offset, None)
                    {
                        o.blade_offset = v.max(0.0);
                    }
                    ui.end_row();
                    ui.label(tr("Tolerância das curvas", "Curve tolerance"));
                    if let Some(v) = widgets::length_field(ui, "corte-tol", o.tolerance, None) {
                        o.tolerance = v.max(1.0);
                    }
                    ui.end_row();
                    ui.checkbox(
                        &mut self.weed,
                        tr("Caixa de descasque, margem", "Weed box, margin"),
                    );
                    if let Some(v) = widgets::length_field(ui, "corte-weed", self.weed_margin, None)
                    {
                        self.weed_margin = v.max(0.0);
                    }
                    ui.end_row();
                    if self.format == CutFormat::Plt {
                        ui.label(tr("Unidades por mm", "Units per mm"));
                        ui.add(egui::DragValue::new(&mut o.units_per_mm).range(1.0..=1016.0));
                        ui.end_row();
                        let mut vs = o.velocity.is_some();
                        ui.checkbox(&mut vs, tr("Velocidade (VS)", "Speed (VS)"));
                        let mut v = o.velocity.unwrap_or(20.0);
                        ui.add_enabled(vs, egui::DragValue::new(&mut v).range(1.0..=200.0));
                        o.velocity = vs.then_some(v);
                        ui.end_row();
                        let mut fs = o.force.is_some();
                        ui.checkbox(&mut fs, tr("Força (FS)", "Force (FS)"));
                        let mut f = o.force.unwrap_or(80.0);
                        ui.add_enabled(fs, egui::DragValue::new(&mut f).range(1.0..=1000.0));
                        o.force = fs.then_some(f);
                        ui.end_row();
                    }
                });
            ui.checkbox(
                &mut o.mirror,
                tr("Espelhar (corte pelo verso)", "Mirror (cut from the back)"),
            );
            o.weed_box = self.weed.then_some(self.weed_margin);
            ui.add_space(theme::SPACE_300);
            let strokes = cut::cut_strokes(doc.doc(), doc.page, o);
            let total: usize = strokes.iter().map(|s| s.len()).sum();
            ui.label(format!(
                "{} {} · {} {}",
                strokes.len(),
                tr("traços de corte", "cut strokes"),
                total,
                tr("pontos", "points")
            ));
            self.preview(ui, doc, &strokes);
            ui.add_space(theme::SPACE_300);
            widgets::footer(ui, |ui| {
                if ui
                    .add_enabled(
                        !strokes.is_empty(),
                        widgets::primary_button(tr("Salvar…", "Save…")),
                    )
                    .clicked()
                {
                    out = Outcome::Save(self.format, self.opts.clone());
                }
                if ui
                    .add(widgets::secondary_button(tr("Cancelar", "Cancel")))
                    .clicked()
                    || ui.input(|i| i.key_pressed(egui::Key::Escape))
                {
                    out = Outcome::Cancel;
                }
            });
        });
        out
    }

    /// Prévia animada (GD-CRT-06): o percurso aparece na ordem de corte;
    /// deslocamentos com a lâmina levantada em tracejado.
    fn preview(&mut self, ui: &mut egui::Ui, doc: &DocState, strokes: &[Vec<Point>]) {
        let (pw, ph) = doc.page_size();
        let [w, h] = theme::CUT_PREVIEW;
        let (rect, _) = ui.allocate_exact_size(Vec2::new(w, h), egui::Sense::hover());
        let pal = theme::palette(ui);
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, theme::RADIUS_SM, pal.bg_surface_0);
        let s = ((w - 2.0 * theme::SPACE_300) as f64 / pw)
            .min((h - 2.0 * theme::SPACE_300) as f64 / ph);
        let origin = rect.center() - Vec2::new((pw * s / 2.0) as f32, -(ph * s / 2.0) as f32);
        let to = |p: Point| Pos2::new(origin.x + (p.x * s) as f32, origin.y - (p.y * s) as f32);
        let page = Rect::from_two_pos(to(Point::ZERO), to(Point::new(pw, ph)));
        painter.rect_stroke(
            page,
            theme::RADIUS_NONE,
            egui::Stroke::new(theme::BORDER_WIDTH, pal.border_default),
            egui::StrokeKind::Middle,
        );
        let now = ui.input(|i| i.time);
        let t0 = *self.started.get_or_insert(now);
        let len: f64 = strokes
            .iter()
            .map(|st| st.windows(2).map(|w| (w[1] - w[0]).hypot()).sum::<f64>())
            .sum();
        // A animação inteira dura alguns segundos, qualquer que seja o tamanho.
        let shown = ((now - t0) / theme::CUT_PREVIEW_SECONDS as f64 * len).min(len);
        let cut = egui::Stroke::new(theme::FOCUS_WIDTH, theme::ACCENT_PRIMARY);
        let travel = egui::Stroke::new(theme::GUIDE_WIDTH, pal.fg_muted);
        let mut left = shown;
        let mut pen = to(Point::ZERO);
        for st in strokes {
            if left <= 0.0 {
                break;
            }
            let start = to(st[0]);
            painter.extend(egui::Shape::dashed_line(
                &[pen, start],
                travel,
                theme::BLEED_DASH,
                theme::BLEED_GAP,
            ));
            let mut line = vec![start];
            for w in st.windows(2) {
                let d = (w[1] - w[0]).hypot();
                if d >= left {
                    let q = w[0] + (w[1] - w[0]) * (left / d.max(1e-9));
                    line.push(to(q));
                    left = 0.0;
                    break;
                }
                line.push(to(w[1]));
                left -= d;
            }
            pen = *line.last().unwrap();
            painter.add(egui::Shape::line(line, cut));
        }
        if shown < len {
            ui.ctx().request_repaint();
        }
        if ui
            .small_button(tr("Repetir prévia", "Replay preview"))
            .clicked()
        {
            self.started = None;
        }
    }
}

/// Opções de "Gerar linha de corte" (GD-CRT-02).
pub struct CutLineDialog {
    pub distance: f64,
    pub round: bool,
    pub remove_holes: bool,
}

impl Default for CutLineDialog {
    fn default() -> Self {
        CutLineDialog {
            distance: 2_000.0,
            round: true,
            remove_holes: true,
        }
    }
}

impl CutLineDialog {
    /// `Some(true)` = criar; `Some(false)` = cancelar.
    pub fn show(&mut self, ctx: &egui::Context) -> Option<bool> {
        let mut out = None;
        widgets::modal(ctx, egui::Id::unique("gerar-linha-corte")).show(ctx, |ui| {
            ui.heading(tr("Gerar linha de corte", "Create cut line"));
            ui.add_space(theme::SPACE_300);
            ui.horizontal(|ui| {
                ui.label(tr("Distância da arte", "Distance from art"));
                if let Some(v) = widgets::length_field(ui, "linha-corte-d", self.distance, None) {
                    self.distance = v.max(0.0);
                }
            });
            ui.checkbox(&mut self.round, tr("Cantos arredondados", "Round corners"));
            ui.checkbox(
                &mut self.remove_holes,
                tr("Remover furos internos", "Remove inner holes"),
            );
            ui.label(
                egui::RichText::new(format!(
                    "{} \"{}\"",
                    tr(
                        "Vai para a camada e a cor especial",
                        "Goes to the layer and spot color"
                    ),
                    cut::CUT_NAME
                ))
                .weak(),
            );
            ui.add_space(theme::SPACE_300);
            widgets::footer(ui, |ui| {
                if ui
                    .add(widgets::primary_button(tr("Criar", "Create")))
                    .clicked()
                    || ui.input(|i| i.key_pressed(egui::Key::Enter))
                {
                    out = Some(true);
                }
                if ui
                    .add(widgets::secondary_button(tr("Cancelar", "Cancel")))
                    .clicked()
                    || ui.input(|i| i.key_pressed(egui::Key::Escape))
                {
                    out = Some(false);
                }
            });
        });
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A janela de exportação cabe em 1366 × 768 e mostra os traços.
    #[test]
    fn cut_export_window_fits_small_screen() {
        let ctx = crate::theme::test_ctx();
        let doc = DocState::new_blank();
        let mut dlg = CutExport::new(CutFormat::Plt);
        let mut size = Vec2::ZERO;
        for _ in 0..3 {
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1366.0, 768.0))),
                ..Default::default()
            };
            let mut out = ctx.run_ui(input, |ui| {
                assert!(matches!(dlg.show(ui.ctx(), &doc), Outcome::Open));
            });
            out.textures_delta.clear();
            size = ctx.memory(|m| {
                m.area_rect(egui::Id::unique("exportar-corte"))
                    .map_or(Vec2::ZERO, |r| r.size())
            });
        }
        assert!(
            size.y > 0.0 && size.y <= 768.0 && size.x <= 1366.0,
            "{size:?}"
        );
    }
}
