//! Conta-gotas (doc 27): estado do ciclo amostrar → despejar, lupa (HUD)
//! que acompanha o cursor, histórico dos últimos tons e o realce de
//! aplicação. A leitura nativa e a aplicação ficam no núcleo
//! (`graphdraw_core::edit::eyedrop`).

use eframe::egui::{self, Color32, Pos2, Rect, Sense, Vec2};
use graphdraw_core::edit::eyedrop::{AttrFilter, Paint};
use graphdraw_core::model::{Color, Object, Stroke};

use crate::i18n::tr;
use crate::theme;

/// Modo da ferramenta (GD-CTG-01).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    #[default]
    Color,
    Attributes,
}

/// Raio de coleta (GD-CTG-03).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Radius {
    /// Valor do modelo vetorial; imagens e áreas vazias pela tela.
    #[default]
    Native,
    Px1,
    Px3,
    Px5,
}

impl Radius {
    pub const ALL: [Radius; 4] = [Radius::Native, Radius::Px1, Radius::Px3, Radius::Px5];

    pub fn side(self) -> usize {
        match self {
            Radius::Native | Radius::Px1 => 1,
            Radius::Px3 => 3,
            Radius::Px5 => 5,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Radius::Native => tr("Vetor nativo", "Native vector"),
            Radius::Px1 => tr("Ponto 1×1", "Point 1×1"),
            Radius::Px3 => tr("Média 3×3", "Average 3×3"),
            Radius::Px5 => tr("Média 5×5", "Average 5×5"),
        }
    }
}

/// O que está no balde.
#[derive(Debug, Clone, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum Bucket {
    Paint(Paint),
    Attrs(Box<Object>),
}

/// Cor sob o cursor e a vizinhança (lupa).
#[derive(Debug, Clone, PartialEq)]
pub struct HoverInfo {
    pub color: Color,
    /// `GRID × GRID` pixels físicos ao redor do cursor (sRGB).
    pub grid: Vec<[u8; 3]>,
    /// Nome da cor na paleta do documento, se for igual.
    pub name: Option<String>,
}

/// Lupa: posição, se está parada esperando o cursor e a caixa desenhada.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hud {
    pub pos: Pos2,
    pub frozen: bool,
    pub placed: bool,
    last: Pos2,
    moved_at: f64,
    /// Caixa do último quadro (vazia = não desenhada).
    pub rect: Rect,
}

impl Default for Hud {
    fn default() -> Self {
        Hud {
            pos: Pos2::ZERO,
            frozen: false,
            placed: false,
            last: Pos2::ZERO,
            moved_at: 0.0,
            rect: Rect::NOTHING,
        }
    }
}

/// Lado da grade da lupa em pixels físicos.
pub const GRID: usize = 11;

/// Pedido feito na lupa.
#[derive(Debug, Clone, PartialEq)]
pub enum HudAction {
    /// Botão direito na lupa (GD-CTG-10).
    AddToPalette(Color),
    /// Clique num tom recente (GD-CTG-11).
    Recent(Color),
}

#[derive(Debug, Clone, Default)]
pub struct Eyedrop {
    pub mode: Mode,
    pub radius: Radius,
    pub filter: AttrFilter,
    pub bucket: Option<Bucket>,
    /// Fase de despejo (balde); `false` = pipeta.
    pub applying: bool,
    /// Últimos tons, o mais novo primeiro (GD-CTG-11).
    pub recent: Vec<Color>,
    /// Objetos que acabaram de receber e quando (GD-CTG-12).
    pub ripple: Option<(Vec<String>, f64)>,
    pub hud: Hud,
    pub info: Option<HoverInfo>,
    /// Ferramenta ativa neste quadro.
    pub active: bool,
    pub shift: bool,
    pub now: f64,
    /// Contorno dado a quem não tinha (o padrão dos novos objetos).
    pub new_stroke: Option<Stroke>,
    /// Fundo do canvas fora da folha (sRGB).
    pub canvas_bg: [u8; 3],
}

impl Eyedrop {
    /// O próximo clique amostra (pipeta) em vez de despejar? Shift
    /// reamostra sem soltar o balde (GD-CTG-05).
    pub fn sampling(&self) -> bool {
        !self.applying || self.bucket.is_none() || self.shift
    }

    /// Esc: esvazia o balde e volta à pipeta (GD-CTG-05).
    pub fn empty(&mut self) -> bool {
        let had = self.bucket.is_some() || self.applying;
        self.bucket = None;
        self.applying = false;
        had
    }

    /// Guarda o que foi coletado e passa ao balde.
    pub fn load(&mut self, b: Bucket) {
        if let Bucket::Paint(Paint::Color(c)) = &b {
            self.push_recent(c.clone());
        }
        if let Bucket::Paint(Paint::Ramp(g, _)) = &b
            && let Some(s) = g.stops.first()
        {
            self.push_recent(s.color.clone());
        }
        self.bucket = Some(b);
        self.applying = true;
    }

    /// Tom no topo do histórico, sem repetir; no máximo 5.
    pub fn push_recent(&mut self, c: Color) {
        let hex = c.to_hex();
        self.recent.retain(|r| r.to_hex() != hex);
        self.recent.insert(0, c);
        self.recent.truncate(theme::EYEDROP_RECENT);
    }

    pub fn over_hud(&self, p: Pos2) -> bool {
        self.active && self.hud.rect.is_positive() && self.hud.rect.contains(p)
    }

    /// A lupa segue o cursor; parado por um instante, ela para e espera o
    /// cursor ir até ela (para o direito e os tons recentes); só volta a
    /// seguir quando o cursor se afasta.
    pub fn follow(&mut self, pointer: Pos2, now: f64, size: Vec2, bounds: Rect) {
        let h = &mut self.hud;
        if pointer != h.last {
            h.last = pointer;
            h.moved_at = now;
        }
        let rect = Rect::from_min_size(h.pos, size);
        if h.frozen {
            if rect.expand(theme::EYEDROP_RELEASE).contains(pointer) {
                return;
            }
            h.frozen = false;
        } else if h.placed && now - h.moved_at >= theme::EYEDROP_DWELL_S {
            h.frozen = true;
            return;
        }
        let off = theme::EYEDROP_OFFSET;
        let mut p = pointer + Vec2::splat(off);
        if p.x + size.x > bounds.max.x {
            p.x = pointer.x - off - size.x;
        }
        if p.y + size.y > bounds.max.y {
            p.y = pointer.y - off - size.y;
        }
        h.pos = p;
        h.placed = true;
    }

    /// Ripple em andamento: fração 0..1 já passada.
    pub fn ripple_phase(&self) -> Option<f64> {
        let (_, t0) = self.ripple.as_ref()?;
        let k = (self.now - t0) / theme::EYEDROP_RIPPLE_S;
        (0.0..1.0).contains(&k).then_some(k)
    }
}

/// Grade `n × n` de pixels em volta de `(cx, cy)` num quadro RGBA
/// pré-multiplicado, composta sobre o fundo `bg(x, y)` (fora do quadro: só
/// o fundo).
pub fn grid_from(
    pixels: &[u8],
    size: [usize; 2],
    cx: i64,
    cy: i64,
    n: usize,
    bg: impl Fn(i64, i64) -> [u8; 3],
) -> Vec<[u8; 3]> {
    let h = n as i64 / 2;
    let mut out = Vec::with_capacity(n * n);
    for y in cy - h..=cy + h {
        for x in cx - h..=cx + h {
            let b = bg(x, y);
            let inside = x >= 0 && y >= 0 && (x as usize) < size[0] && (y as usize) < size[1];
            if !inside {
                out.push(b);
                continue;
            }
            let i = (y as usize * size[0] + x as usize) * 4;
            let Some(px) = pixels.get(i..i + 4) else {
                out.push(b);
                continue;
            };
            let a = px[3] as u32;
            let c =
                |k: usize| (px[k] as u32 + (b[k] as u32 * (255 - a) + 127) / 255).min(255) as u8;
            out.push([c(0), c(1), c(2)]);
        }
    }
    out
}

/// Média do quadrado central `side × side` da grade (GD-CTG-03).
pub fn average(grid: &[[u8; 3]], n: usize, side: usize) -> Option<Color> {
    if grid.len() != n * n || side == 0 || side > n {
        return None;
    }
    let (c, h) = (n / 2, side / 2);
    let mut sum = [0.0f64; 3];
    for y in c - h..=c + h {
        for x in c - h..=c + h {
            let p = grid[y * n + x];
            for k in 0..3 {
                sum[k] += p[k] as f64;
            }
        }
    }
    let d = (side * side) as f64 * 255.0;
    Some(Color::rgb(sum[0] / d, sum[1] / d, sum[2] / d))
}

/// Valores da cor para a pílula: CMYK e RGB 0–255 (o CMYK do próprio
/// modelo quando a cor é CMYK).
pub fn values_text(c: &Color) -> (String, String) {
    let cmyk = if c.model == "cmyk" {
        c.clone()
    } else {
        graphdraw_core::export::pdf::rgb_to_cmyk(c.to_srgb())
    };
    let pc = |i: usize| (cmyk.values.get(i).copied().unwrap_or(0.0) * 100.0).round() as i64;
    let [r, g, b] = c.to_srgb();
    let b8 = |v: f64| (v.clamp(0.0, 1.0) * 255.0).round() as i64;
    (
        format!("C:{} M:{} Y:{} K:{}", pc(0), pc(1), pc(2), pc(3)),
        format!("R:{} G:{} B:{}", b8(r), b8(g), b8(b)),
    )
}

/// Lupa flutuante (GD-CTG-02, 10, 11): grade ampliada com retículo, pílula
/// de dados, o balde e os últimos tons.
pub fn hud_ui(ctx: &egui::Context, e: &mut Eyedrop, info: &HoverInfo) -> Option<HudAction> {
    let mut act = None;
    let area = egui::Area::new(egui::Id::unique("conta-gotas-lupa"))
        .order(egui::Order::Foreground)
        .fixed_pos(e.hud.pos)
        .interactable(true)
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.spacing_mut().item_spacing = Vec2::splat(theme::SPACE_100);
                let cell = theme::EYEDROP_CELL;
                let side = cell * GRID as f32;
                let (r, resp) = ui.allocate_exact_size(Vec2::splat(side), Sense::click());
                let p = ui.painter_at(r);
                for (i, c) in info.grid.iter().enumerate() {
                    let (x, y) = ((i % GRID) as f32, (i / GRID) as f32);
                    let cr = Rect::from_min_size(
                        r.min + Vec2::new(x * cell, y * cell),
                        Vec2::splat(cell),
                    );
                    p.rect_filled(cr, 0, Color32::from_rgb(c[0], c[1], c[2]));
                }
                let line = egui::Stroke::new(theme::BORDER_WIDTH, Color32::from_black_alpha(28));
                for k in 1..GRID {
                    let d = k as f32 * cell;
                    p.vline(r.min.x + d, r.y_range(), line);
                    p.hline(r.x_range(), r.min.y + d, line);
                }
                let c0 = (GRID / 2) as f32 * cell;
                let center = Rect::from_min_size(r.min + Vec2::splat(c0), Vec2::splat(cell));
                let span = e.radius.side() as f32;
                let reticle = center.expand((span - 1.0) * cell / 2.0);
                p.rect_stroke(
                    reticle,
                    0,
                    egui::Stroke::new(theme::FOCUS_WIDTH, Color32::WHITE),
                    egui::StrokeKind::Outside,
                );
                p.rect_stroke(
                    reticle.expand(theme::FOCUS_WIDTH),
                    0,
                    egui::Stroke::new(theme::BORDER_WIDTH, Color32::BLACK),
                    egui::StrokeKind::Outside,
                );
                let resp = resp.on_hover_text(tr(
                    "Botão direito: adicionar à paleta do documento",
                    "Right click: add to the document palette",
                ));
                if resp.secondary_clicked() {
                    act = Some(HudAction::AddToPalette(info.color.clone()));
                }
                ui.horizontal(|ui| {
                    crate::widgets::swatch(ui, Some(&info.color), theme::EYEDROP_DOT);
                    ui.strong(info.color.to_hex().to_uppercase());
                });
                let (cmyk, rgb) = values_text(&info.color);
                ui.label(egui::RichText::new(cmyk).size(theme::TYPE_MICRO));
                ui.label(egui::RichText::new(rgb).size(theme::TYPE_MICRO));
                if let Some(n) = &info.name {
                    ui.label(egui::RichText::new(n).size(theme::TYPE_MICRO).weak());
                }
                if let Some(b) = &e.bucket {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(tr("Balde:", "Bucket:")).size(theme::TYPE_MICRO),
                        );
                        match b {
                            Bucket::Paint(Paint::Color(c)) => {
                                crate::widgets::swatch(ui, Some(c), theme::EYEDROP_DOT);
                            }
                            Bucket::Paint(Paint::Ramp(..)) => {
                                ui.label(
                                    egui::RichText::new(tr("rampa", "ramp"))
                                        .size(theme::TYPE_MICRO),
                                );
                            }
                            Bucket::Attrs(o) => {
                                let name = o
                                    .name
                                    .clone()
                                    .unwrap_or_else(|| tr("atributos", "attributes").into());
                                ui.label(egui::RichText::new(name).size(theme::TYPE_MICRO));
                            }
                        }
                    });
                }
                if !e.recent.is_empty() {
                    ui.horizontal(|ui| {
                        for c in e.recent.clone() {
                            let (r, resp) = ui.allocate_exact_size(
                                Vec2::splat(theme::EYEDROP_DOT),
                                Sense::click(),
                            );
                            ui.painter().circle_filled(
                                r.center(),
                                r.width() / 2.0,
                                crate::widgets::to_color32(&c),
                            );
                            ui.painter().circle_stroke(
                                r.center(),
                                r.width() / 2.0,
                                theme::border_subtle(ui),
                            );
                            if resp.on_hover_text(c.to_hex()).clicked() {
                                act = Some(HudAction::Recent(c));
                            }
                        }
                    });
                }
            });
        });
    e.hud.rect = area.response.rect;
    act
}

#[cfg(test)]
mod tests {
    use super::*;

    /// GD-CTG-03: a média 3×3 mistura a vizinhança; 1×1 é o pixel do meio;
    /// o transparente vira o fundo.
    #[test]
    fn gd_ctg_03_grid_and_average() {
        // 3×1 pixels: preto opaco, branco opaco, transparente.
        let px = [0, 0, 0, 255, 255, 255, 255, 255, 0, 0, 0, 0];
        let g = grid_from(&px, [3, 1], 1, 0, 3, |_, _| [255, 0, 0]);
        assert_eq!(g[4], [255, 255, 255], "meio");
        assert_eq!(g[3], [0, 0, 0]);
        assert_eq!(g[5], [255, 0, 0], "transparente = fundo");
        assert_eq!(g[0], [255, 0, 0], "fora do quadro = fundo");
        assert_eq!(average(&g, 3, 1).unwrap().to_hex(), "#ffffff");
        let avg = average(&g, 3, 3).unwrap().to_srgb();
        assert!((avg[0] - (255.0 * 7.0 + 255.0) / 9.0 / 255.0).abs() < 1e-9);
    }

    /// GD-CTG-11: até 5 tons, sem repetir, o mais novo primeiro.
    #[test]
    fn gd_ctg_11_recent_five() {
        let mut e = Eyedrop::default();
        for i in 0..7 {
            e.push_recent(Color::rgb(i as f64 / 10.0, 0.0, 0.0));
        }
        e.push_recent(Color::rgb(0.4, 0.0, 0.0));
        assert_eq!(e.recent.len(), 5);
        assert_eq!(e.recent[0].to_hex(), Color::rgb(0.4, 0.0, 0.0).to_hex());
        let hexes: std::collections::HashSet<_> = e.recent.iter().map(|c| c.to_hex()).collect();
        assert_eq!(hexes.len(), 5);
    }

    /// A lupa segue, para quando o cursor para e espera o cursor chegar;
    /// afastando-se, volta a seguir.
    #[test]
    fn hud_freezes_after_dwell_and_releases() {
        let mut e = Eyedrop::default();
        let bounds = Rect::from_min_size(Pos2::ZERO, Vec2::splat(1000.0));
        let size = Vec2::new(120.0, 200.0);
        e.follow(Pos2::new(100.0, 100.0), 0.0, size, bounds);
        assert_eq!(
            e.hud.pos,
            Pos2::new(100.0 + theme::EYEDROP_OFFSET, 100.0 + theme::EYEDROP_OFFSET)
        );
        e.follow(Pos2::new(100.0, 100.0), 1.0, size, bounds);
        assert!(e.hud.frozen);
        let at = e.hud.pos;
        // Indo até a lupa: ela fica.
        e.follow(at + Vec2::splat(10.0), 1.1, size, bounds);
        assert_eq!(e.hud.pos, at);
        // Longe: volta a seguir.
        e.follow(Pos2::new(600.0, 600.0), 1.2, size, bounds);
        assert!(!e.hud.frozen);
        assert_ne!(e.hud.pos, at);
        // Perto da borda: vai para o outro lado do cursor.
        e.follow(Pos2::new(990.0, 990.0), 1.3, size, bounds);
        assert!(e.hud.pos.x < 990.0 && e.hud.pos.y < 990.0);
    }
}
