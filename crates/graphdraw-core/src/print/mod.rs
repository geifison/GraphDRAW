//! Impressão (doc 29): o plano de folhas (escala, posição, giro, ladrilhos,
//! imposição N-up, separações), as marcas de produção e a rasterização da
//! folha em faixas. Não depende do Windows: o envio ao spooler fica no app.
//!
//! A folha tem a origem no canto de baixo à esquerda e Y para cima (µm),
//! como a página.

use std::collections::HashMap;

use kurbo::{Affine, Point, Rect};
use serde_json::Map;
use tiny_skia as sk;

use crate::Error;
use crate::export::ExportOptions;
use crate::export::pdf::{PdfReport, PdfSheet};
use crate::model::{
    Color, Document, EllipseData, Fill, Object, ObjectKind, Page, RectData, Stroke, Transform,
};
use crate::render::raster::{self, RasterOptions};
use crate::render::scene::{ImageCache, Scene, SceneOptions, build_scene};
use crate::render::separation::{self, Plate};

const MM: f64 = 1000.0;
/// µm por ponto tipográfico.
const PT: f64 = 25_400.0 / 72.0;
/// Espaço entre a sangria e as marcas (GD-PRT-30).
const MARK_GAP: f64 = 2.0 * MM;
/// Faixa das marcas de corte e de registro além da sangria.
const MARK_BAND: f64 = 8.0 * MM;
const BAR_BAND: f64 = 7.0 * MM;
const INFO_BAND: f64 = 6.0 * MM;
const EPS: f64 = 1.0;

/// Folha da impressora: tamanho e área imprimível (margens físicas).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Paper {
    pub width: f64,
    pub height: f64,
    pub printable: Rect,
}

impl Paper {
    /// Folha sem margens físicas (imprimir para arquivo).
    pub fn full(width: f64, height: f64) -> Paper {
        Paper {
            width,
            height,
            printable: Rect::new(0.0, 0.0, width, height),
        }
    }

    pub const A4: Paper = Paper {
        width: 210_000.0,
        height: 297_000.0,
        printable: Rect::new(0.0, 0.0, 210_000.0, 297_000.0),
    };
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Scale {
    /// Tamanho real (100%).
    Actual,
    /// Maior escala que cabe na área imprimível.
    Fit,
    /// Porcentagem (100 = tamanho real).
    Percent(f64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Anchor {
    Center,
    /// Canto de cima à esquerda da área imprimível.
    TopLeft,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PageRange {
    All,
    Current,
    /// `1,3-5` (base 1, GD-EXP-01).
    Custom(String),
}

/// Opções da janela de impressão (GD-PRT-10..42).
#[derive(Debug, Clone, PartialEq)]
pub struct PrintSettings {
    pub range: PageRange,
    pub copies: u32,
    pub collate: bool,
    pub scale: Scale,
    pub anchor: Anchor,
    /// Deslocamento (µm): X para a direita, Y para baixo.
    pub offset: [f64; 2],
    pub auto_rotate: bool,
    pub tile: bool,
    pub overlap: f64,
    pub nup: bool,
    /// 0 = o máximo que cabe.
    pub nup_cols: u32,
    pub nup_rows: u32,
    pub nup_gap: f64,
    /// Páginas seguidas nas células, em vez de cópias da mesma.
    pub nup_distinct: bool,
    pub crop_marks: bool,
    pub reg_marks: bool,
    pub color_bar: bool,
    pub info: bool,
    pub print_bleed: bool,
    pub mirror: bool,
    pub negative: bool,
    pub separations: bool,
    /// Chapas escolhidas; vazio = todas as do documento.
    pub plates: Vec<Plate>,
    /// Nome que aparece nas informações (nome do arquivo).
    pub job_name: String,
}

impl Default for PrintSettings {
    fn default() -> Self {
        PrintSettings {
            range: PageRange::All,
            copies: 1,
            collate: true,
            scale: Scale::Actual,
            anchor: Anchor::Center,
            offset: [0.0, 0.0],
            auto_rotate: true,
            tile: false,
            overlap: 10.0 * MM,
            nup: false,
            nup_cols: 0,
            nup_rows: 0,
            nup_gap: 3.0 * MM,
            nup_distinct: false,
            crop_marks: false,
            reg_marks: false,
            color_bar: false,
            info: false,
            print_bleed: false,
            mirror: false,
            negative: false,
            separations: false,
            plates: Vec::new(),
            job_name: String::new(),
        }
    }
}

impl PrintSettings {
    fn any_marks(&self) -> bool {
        self.crop_marks || self.reg_marks || self.color_bar || self.info
    }
}

/// Marca de produção, em coordenadas da folha.
#[derive(Debug, Clone, PartialEq)]
pub enum Mark {
    /// Traço (retângulo fino) na cor de registro.
    Line(Rect),
    /// Círculo vazado na cor de registro.
    Ring {
        center: Point,
        radius: f64,
        width: f64,
    },
    /// Quadrado da barra de cores.
    Swatch { rect: Rect, color: Color },
    /// Linha de informações (canto de cima à esquerda do texto).
    Info { at: Point, size: f64 },
}

/// Página posicionada na folha.
#[derive(Debug, Clone, PartialEq)]
pub struct Placement {
    pub page: usize,
    /// Página → folha (µm).
    pub m: Affine,
    /// Recorte no espaço da página (corte ou corte + sangria).
    pub clip: Rect,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Sheet {
    pub placements: Vec<Placement>,
    pub marks: Vec<Mark>,
    /// Chapa da separação (GD-PRT-42).
    pub plate: Option<Plate>,
    /// Página e ladrilho, para as informações.
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Plan {
    pub sheets: Vec<Sheet>,
    /// Escala efetiva (1 = tamanho real).
    pub scale: f64,
    pub rotated: bool,
    pub warnings: Vec<String>,
    /// Data e hora das informações.
    pub stamp: String,
}

impl Plan {
    /// Ordem das folhas com as cópias (GD-PRT-12): agrupadas, a sequência
    /// inteira se repete; sem agrupar, cada folha se repete.
    pub fn print_order(&self, copies: u32, collate: bool) -> Vec<usize> {
        let n = self.sheets.len();
        let c = copies.max(1) as usize;
        if collate {
            (0..c).flat_map(|_| 0..n).collect()
        } else {
            (0..n).flat_map(|i| std::iter::repeat_n(i, c)).collect()
        }
    }
}

/// Nome da chapa como aparece na janela e nas informações.
pub fn plate_name(p: &Plate) -> String {
    match p {
        Plate::Cyan => "Ciano".into(),
        Plate::Magenta => "Magenta".into(),
        Plate::Yellow => "Amarelo".into(),
        Plate::Black => "Preto".into(),
        Plate::Spot(n) => n.clone(),
    }
}

/// Cor de registro (sai em todas as chapas).
pub fn registration() -> Color {
    Color {
        model: "registration".into(),
        values: vec![1.0],
        name: Some("Registro".into()),
        reference: None,
        alternate: None,
        extra: Map::new(),
    }
}

/// Faixas das marcas além da sangria: [esquerda, baixo, direita, cima].
fn margins(s: &PrintSettings) -> [f64; 4] {
    if !s.any_marks() {
        return [0.0; 4];
    }
    let side = if s.crop_marks || s.reg_marks {
        MARK_BAND
    } else {
        MARK_GAP
    };
    let top = side + if s.color_bar { BAR_BAND } else { 0.0 };
    let bottom = side + if s.info { INFO_BAND } else { 0.0 };
    [side, bottom, side, top]
}

fn fits(w: f64, h: f64, area: Rect) -> bool {
    w <= area.width() + EPS && h <= area.height() + EPS
}

/// Sangria impressa da página (µm, no espaço da página).
fn page_bleed(doc: &Document, page: &Page, s: &PrintSettings) -> f64 {
    if s.print_bleed {
        page.bleed.unwrap_or(doc.data.settings.bleed).max(0.0)
    } else {
        0.0
    }
}

/// Página → folha: escala `k`, girada 90° (anti-horário) ou não, com o
/// canto de baixo à esquerda do corte em `at`.
fn page_matrix(page: &Page, k: f64, rotated: bool, at: Point) -> Affine {
    if rotated {
        Affine::translate((at.x + page.height * k, at.y))
            * Affine::scale(k)
            * Affine::rotate(std::f64::consts::FRAC_PI_2)
    } else {
        Affine::translate((at.x, at.y)) * Affine::scale(k)
    }
}

/// Marcas em volta do corte `t` (folha), com a sangria impressa `bs`.
fn marks_around(t: Rect, bs: f64, s: &PrintSettings, cuts_x: &[f64], cuts_y: &[f64]) -> Vec<Mark> {
    let mut out = Vec::new();
    let hw = 0.25 * PT / 2.0;
    let (near, far) = (bs + MARK_GAP, bs + MARK_GAP + 5.0 * MM);
    if s.crop_marks {
        // Verticais acima e abaixo de cada linha de corte X.
        for &x in cuts_x {
            out.push(Mark::Line(Rect::new(
                x - hw,
                t.y1 + near,
                x + hw,
                t.y1 + far,
            )));
            out.push(Mark::Line(Rect::new(
                x - hw,
                t.y0 - far,
                x + hw,
                t.y0 - near,
            )));
        }
        for &y in cuts_y {
            out.push(Mark::Line(Rect::new(
                t.x0 - far,
                y - hw,
                t.x0 - near,
                y + hw,
            )));
            out.push(Mark::Line(Rect::new(
                t.x1 + near,
                y - hw,
                t.x1 + far,
                y + hw,
            )));
        }
    }
    if s.reg_marks {
        let d = bs + MARK_GAP + 3.0 * MM;
        // No meio do trecho entre cortes mais perto do centro (na imposição,
        // o centro pode cair numa linha de corte).
        let mid = |cuts: &[f64], lo: f64, hi: f64| {
            let c = (lo + hi) / 2.0;
            let mut v: Vec<f64> = cuts.iter().copied().chain([lo, hi]).collect();
            v.sort_by(f64::total_cmp);
            v.windows(2)
                .filter(|w| w[1] - w[0] >= 8.0 * MM)
                .map(|w| (w[0] + w[1]) / 2.0)
                .min_by(|a, b| (a - c).abs().total_cmp(&(b - c).abs()))
                .unwrap_or(c)
        };
        let (cx, cy) = (mid(cuts_x, t.x0, t.x1), mid(cuts_y, t.y0, t.y1));
        for p in [
            Point::new(cx, t.y1 + d),
            Point::new(cx, t.y0 - d),
            Point::new(t.x0 - d, cy),
            Point::new(t.x1 + d, cy),
        ] {
            out.push(Mark::Ring {
                center: p,
                radius: 2.0 * MM,
                width: 0.25 * PT,
            });
            out.push(Mark::Line(Rect::new(
                p.x - 3.0 * MM,
                p.y - hw,
                p.x + 3.0 * MM,
                p.y + hw,
            )));
            out.push(Mark::Line(Rect::new(
                p.x - hw,
                p.y - 3.0 * MM,
                p.x + hw,
                p.y + 3.0 * MM,
            )));
        }
    }
    let band = if s.crop_marks || s.reg_marks {
        MARK_BAND
    } else {
        MARK_GAP
    };
    if s.color_bar {
        let y0 = t.y1 + bs + band + 1.0 * MM;
        let cmyk = Color::cmyk;
        let colors = [
            cmyk(1.0, 0.0, 0.0, 0.0),
            cmyk(0.0, 1.0, 0.0, 0.0),
            cmyk(0.0, 0.0, 1.0, 0.0),
            cmyk(0.0, 0.0, 0.0, 1.0),
            cmyk(1.0, 1.0, 0.0, 0.0),
            cmyk(1.0, 0.0, 1.0, 0.0),
            cmyk(0.0, 1.0, 1.0, 0.0),
            cmyk(0.0, 0.0, 0.0, 0.25),
            cmyk(0.0, 0.0, 0.0, 0.5),
            cmyk(0.0, 0.0, 0.0, 0.75),
        ];
        for (i, color) in colors.into_iter().enumerate() {
            let x = t.x0 + i as f64 * 5.0 * MM;
            out.push(Mark::Swatch {
                rect: Rect::new(x, y0, x + 5.0 * MM, y0 + 5.0 * MM),
                color,
            });
        }
    }
    if s.info {
        out.push(Mark::Info {
            at: Point::new(t.x0, t.y0 - bs - band - 1.0 * MM),
            size: 7.0 * PT,
        });
    }
    out
}

/// Caixa ocupada pelas marcas (para saber se cabem na área imprimível).
fn marks_bounds(marks: &[Mark]) -> Option<Rect> {
    marks
        .iter()
        .map(|m| match m {
            Mark::Line(r) => *r,
            Mark::Ring { center, radius, .. } => {
                Rect::from_center_size(*center, (2.0 * radius, 2.0 * radius))
            }
            Mark::Swatch { rect, .. } => *rect,
            // Uma linha de texto de ~80 caracteres.
            Mark::Info { at, size } => Rect::new(at.x, at.y - size * 1.2, at.x + size * 0.5, at.y),
        })
        .reduce(|a, b| a.union(b))
}

fn contains(outer: Rect, inner: Rect) -> bool {
    inner.x0 >= outer.x0 - EPS
        && inner.y0 >= outer.y0 - EPS
        && inner.x1 <= outer.x1 + EPS
        && inner.y1 <= outer.y1 + EPS
}

/// Monta as folhas (GD-PRT-20..42). `current` é a página em edição.
pub fn plan(
    doc: &Document,
    current: usize,
    s: &PrintSettings,
    paper: &Paper,
) -> Result<Plan, Error> {
    let n = doc.data.pages.len();
    if n == 0 {
        return Err(Error::Other("documento sem páginas".into()));
    }
    let pages: Vec<usize> = match &s.range {
        PageRange::All => (0..n).collect(),
        PageRange::Current => vec![current.min(n - 1)],
        PageRange::Custom(spec) => crate::export::parse_pages(spec, n)?,
    };
    if pages.is_empty() {
        return Err(Error::Other("nenhuma página no intervalo".into()));
    }
    let pr = paper.printable;
    let mut plan = Plan {
        scale: 1.0,
        stamp: stamp_now(),
        ..Default::default()
    };
    let warn = |plan: &mut Plan, w: &str| {
        if !plan.warnings.iter().any(|x| x == w) {
            plan.warnings.push(w.into());
        }
    };
    let mg = margins(s);
    let (mx, my) = (mg[0] + mg[2], mg[1] + mg[3]);
    let mut sheets: Vec<Sheet> = Vec::new();

    if s.nup && !s.tile {
        // Imposição (GD-PRT-24): grade de células corte + sangria, marcas em
        // volta da grade.
        let p0 = &doc.data.pages[pages[0]];
        let k = match s.scale {
            Scale::Percent(p) => (p / 100.0).max(0.01),
            _ => 1.0,
        };
        let b = page_bleed(doc, p0, s);
        let gap = s.nup_gap.max(0.0);
        let grid = |rot: bool| {
            let (w, h) = if rot {
                (p0.height, p0.width)
            } else {
                (p0.width, p0.height)
            };
            let (cw, ch) = ((w + 2.0 * b) * k, (h + 2.0 * b) * k);
            let auto = |avail: f64, c: f64| (((avail + gap) / (c + gap)).floor() as u32).max(1);
            let cols = if s.nup_cols > 0 {
                s.nup_cols
            } else {
                auto(pr.width() - mx, cw)
            };
            let rows = if s.nup_rows > 0 {
                s.nup_rows
            } else {
                auto(pr.height() - my, ch)
            };
            (cols, rows, cw, ch)
        };
        let (c0, r0, ..) = grid(false);
        let (c1, r1, ..) = grid(true);
        let rotated = s.auto_rotate && c1 * r1 > c0 * r0;
        let (cols, rows, cw, ch) = grid(rotated);
        plan.scale = k;
        plan.rotated = rotated;
        let per = (cols * rows) as usize;
        let gw = cols as f64 * cw + (cols - 1) as f64 * gap;
        let gh = rows as f64 * ch + (rows - 1) as f64 * gap;
        let unit = (gw + mx, gh + my);
        let ux0 = match s.anchor {
            Anchor::Center => pr.center().x - unit.0 / 2.0,
            Anchor::TopLeft => pr.x0,
        } + s.offset[0];
        let uy1 = match s.anchor {
            Anchor::Center => pr.center().y + unit.1 / 2.0,
            Anchor::TopLeft => pr.y1,
        } - s.offset[1];
        let g = Rect::new(ux0 + mg[0], uy1 - mg[3] - gh, ux0 + mg[0] + gw, uy1 - mg[3]);
        if !contains(pr, Rect::new(ux0, uy1 - unit.1, ux0 + unit.0, uy1)) {
            warn(&mut plan, "a grade não cabe na folha; será cortada");
        }
        let groups: Vec<Vec<usize>> = if s.nup_distinct {
            pages.chunks(per).map(|c| c.to_vec()).collect()
        } else {
            pages.iter().map(|&p| vec![p; per]).collect()
        };
        let bs = b * k;
        for group in groups {
            let mut placements = Vec::new();
            let (mut cuts_x, mut cuts_y) = (Vec::new(), Vec::new());
            for (i, &p) in group.iter().enumerate() {
                let (r, c) = ((i as u32) / cols, (i as u32) % cols);
                let cell_x = g.x0 + c as f64 * (cw + gap);
                let cell_y1 = g.y1 - r as f64 * (ch + gap);
                let page = &doc.data.pages[p];
                let at = Point::new(cell_x + bs, cell_y1 - ch + bs);
                let (tw, th) = if rotated {
                    (page.height * k, page.width * k)
                } else {
                    (page.width * k, page.height * k)
                };
                for x in [at.x, at.x + tw] {
                    if !cuts_x.iter().any(|v: &f64| (v - x).abs() < EPS) {
                        cuts_x.push(x);
                    }
                }
                for y in [at.y, at.y + th] {
                    if !cuts_y.iter().any(|v: &f64| (v - y).abs() < EPS) {
                        cuts_y.push(y);
                    }
                }
                let bp = page_bleed(doc, page, s);
                placements.push(Placement {
                    page: p,
                    m: page_matrix(page, k, rotated, at),
                    clip: Rect::new(-bp, -bp, page.width + bp, page.height + bp),
                });
            }
            // Marcas em volta da grade: corte nas linhas de cada célula.
            let t = g.inset(-bs);
            let marks = marks_around(t, bs, s, &cuts_x, &cuts_y);
            let label = page_label(&group);
            sheets.push(Sheet {
                placements,
                marks,
                plate: None,
                label,
            });
        }
    } else {
        for &p in &pages {
            let page = &doc.data.pages[p];
            let b = page_bleed(doc, page, s);
            let size = |rot: bool, k: f64| {
                let (w, h) = if rot {
                    (page.height, page.width)
                } else {
                    (page.width, page.height)
                };
                ((w + 2.0 * b) * k, (h + 2.0 * b) * k)
            };
            let fit_k = |rot: bool| {
                let (w, h) = size(rot, 1.0);
                ((pr.width() - mx) / w)
                    .min((pr.height() - my) / h)
                    .max(0.01)
            };
            let (k, rotated) = match s.scale {
                Scale::Fit if !s.tile => {
                    let (a, r) = (fit_k(false), fit_k(true));
                    if s.auto_rotate && r > a * 1.0001 {
                        (r, true)
                    } else {
                        (a, false)
                    }
                }
                Scale::Percent(pc) => {
                    let k = (pc / 100.0).max(0.01);
                    (k, rotate_if_better(s, &size, k, pr, mx, my))
                }
                _ => (1.0, rotate_if_better(s, &size, 1.0, pr, mx, my)),
            };
            plan.scale = k;
            plan.rotated = rotated;
            let (cw, ch) = size(rotated, k);
            let bs = b * k;
            let clip = Rect::new(-b, -b, page.width + b, page.height + b);
            if s.tile && !fits(cw, ch, pr) {
                // Ladrilhos (GD-PRT-23), sem giro.
                let (cw, ch) = size(false, k);
                let ov = s.overlap.clamp(0.0, pr.width().min(pr.height()) / 2.0);
                let (sx, sy) = (pr.width() - ov, pr.height() - ov);
                let count = |c: f64, step: f64, full: f64| {
                    if c <= full + EPS {
                        1
                    } else {
                        ((c - ov) / step).ceil() as u32
                    }
                };
                let cols = count(cw, sx, pr.width());
                let rows = count(ch, sy, pr.height());
                for r in 0..rows {
                    for c in 0..cols {
                        let at = Point::new(
                            pr.x0 - c as f64 * sx + bs,
                            pr.y1 - (ch - r as f64 * sy) + bs,
                        );
                        let mut marks = Vec::new();
                        if s.any_marks() {
                            // Alinhamento nos cantos da área imprimível.
                            let hw = 0.25 * PT / 2.0;
                            let arm = 5.0 * MM;
                            for (x, y) in [
                                (pr.x0, pr.y0),
                                (pr.x1, pr.y0),
                                (pr.x0, pr.y1),
                                (pr.x1, pr.y1),
                            ] {
                                let dx = if x == pr.x0 { 1.0 } else { -1.0 };
                                let dy = if y == pr.y0 { 1.0 } else { -1.0 };
                                marks.push(Mark::Line(
                                    Rect::new(x, y - hw, x + dx * arm, y + hw).abs(),
                                ));
                                marks.push(Mark::Line(
                                    Rect::new(x - hw, y, x + hw, y + dy * arm).abs(),
                                ));
                            }
                            marks.push(Mark::Info {
                                at: Point::new(pr.x0 + 7.0 * MM, pr.y0 + 4.0 * MM),
                                size: 7.0 * PT,
                            });
                        }
                        sheets.push(Sheet {
                            placements: vec![Placement {
                                page: p,
                                m: page_matrix(page, k, false, at),
                                clip,
                            }],
                            marks,
                            plate: None,
                            label: format!(
                                "{} — linha {}, coluna {}",
                                page_label(&[p]),
                                r + 1,
                                c + 1
                            ),
                        });
                    }
                }
                plan.rotated = false;
                continue;
            }
            let (uw, uh) = (cw + mx, ch + my);
            let ux0 = match s.anchor {
                Anchor::Center => pr.center().x - uw / 2.0,
                Anchor::TopLeft => pr.x0,
            } + s.offset[0];
            let uy1 = match s.anchor {
                Anchor::Center => pr.center().y + uh / 2.0,
                Anchor::TopLeft => pr.y1,
            } - s.offset[1];
            let at = Point::new(ux0 + mg[0] + bs, uy1 - mg[3] - ch + bs);
            let (tw, th) = (cw - 2.0 * bs, ch - 2.0 * bs);
            let t = Rect::new(at.x, at.y, at.x + tw, at.y + th);
            let mut marks = marks_around(t, bs, s, &[t.x0, t.x1], &[t.y0, t.y1]);
            let content = t.inflate(bs, bs);
            if !contains(pr, content) {
                warn(&mut plan, "a página não cabe na folha; será cortada");
            } else if marks_bounds(&marks).is_some_and(|m| !contains(pr, m)) {
                // GD-PRT-32: sem lugar para as marcas.
                marks.clear();
                warn(
                    &mut plan,
                    "as marcas não cabem; reduza a escala ou use papel maior",
                );
            }
            sheets.push(Sheet {
                placements: vec![Placement {
                    page: p,
                    m: page_matrix(page, k, rotated, at),
                    clip,
                }],
                marks,
                plate: None,
                label: page_label(&[p]),
            });
        }
    }

    if s.separations {
        let plates = if s.plates.is_empty() {
            separation::plates(doc)
        } else {
            s.plates.clone()
        };
        sheets = sheets
            .into_iter()
            .flat_map(|sh| {
                plates.iter().map(move |p| Sheet {
                    plate: Some(p.clone()),
                    ..sh.clone()
                })
            })
            .collect();
    }
    plan.sheets = sheets;
    Ok(plan)
}

fn rotate_if_better(
    s: &PrintSettings,
    size: &dyn Fn(bool, f64) -> (f64, f64),
    k: f64,
    pr: Rect,
    mx: f64,
    my: f64,
) -> bool {
    if !s.auto_rotate {
        return false;
    }
    let (a, b) = (size(false, k), size(true, k));
    !fits(a.0 + mx, a.1 + my, pr) && fits(b.0 + mx, b.1 + my, pr)
}

fn page_label(pages: &[usize]) -> String {
    match pages {
        [p] => format!("Página {}", p + 1),
        [a, .., b] if a != b => format!("Páginas {}–{}", a + 1, b + 1),
        [a, ..] => format!("Página {}", a + 1),
        [] => String::new(),
    }
}

/// Data e hora (UTC) das informações.
fn stamp_now() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs()) as i64;
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // Dias desde 1970 → data civil (algoritmo de Howard Hinnant).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{d:02}/{m:02}/{y} {:02}:{:02} UTC",
        rem / 3600,
        rem % 3600 / 60
    )
}

/// Texto das informações da folha (GD-PRT-30).
pub fn info_text(plan: &Plan, sheet: &Sheet, s: &PrintSettings) -> String {
    let mut parts = Vec::new();
    if !s.job_name.is_empty() {
        parts.push(s.job_name.clone());
    }
    parts.push(sheet.label.clone());
    if let Some(p) = &sheet.plate {
        parts.push(format!("Chapa {}", plate_name(p)));
    }
    parts.push(plan.stamp.clone());
    parts.join(" — ")
}

/// Cena das marcas da folha (objetos comuns na cor de registro).
pub fn marks_scene(plan: &Plan, sheet: &Sheet, s: &PrintSettings, paper: &Paper) -> Scene {
    let mut doc = Document::new();
    let mut page = Page::new(paper.width, paper.height);
    let reg = registration();
    let objs = &mut page.layers[0].objects;
    for m in &sheet.marks {
        let o = match m {
            Mark::Line(r) | Mark::Swatch { rect: r, .. } => {
                let mut o = Object::new(ObjectKind::Rectangle(RectData {
                    width: r.width(),
                    height: r.height(),
                    corners: None,
                    extra: Map::new(),
                }));
                o.transform = Transform::translate(r.x0, r.y0);
                o.fill = Some(Fill::solid(match m {
                    Mark::Swatch { color, .. } => color.clone(),
                    _ => reg.clone(),
                }));
                o
            }
            Mark::Ring {
                center,
                radius,
                width,
            } => {
                let mut o = Object::new(ObjectKind::Ellipse(EllipseData {
                    rx: *radius,
                    ry: *radius,
                    mode: "ellipse".into(),
                    start_angle: 0.0,
                    end_angle: 360.0,
                    extra: Map::new(),
                }));
                o.transform = Transform::translate(center.x, center.y);
                o.stroke = Some(Stroke::new(*width, reg.clone()));
                o
            }
            Mark::Info { at, size } => {
                let mut o = crate::edit::text::new_text(at.x, at.y, None, "Arial", *size);
                if let ObjectKind::Text(t) = &mut o.kind {
                    crate::edit::text::insert(t, 0, &info_text(plan, sheet, s));
                }
                o.fill = Some(Fill::solid(reg.clone()));
                o
            }
        };
        objs.push(o);
    }
    doc.data.pages.push(page);
    build_scene(
        &doc,
        &doc.data.pages[0],
        SceneOptions { print: true },
        &ImageCache::default(),
    )
}

/// Cenas de impressão das páginas usadas no plano.
pub fn page_scenes(doc: &Document, plan: &Plan) -> HashMap<usize, Scene> {
    let cache = ImageCache::default();
    let mut out = HashMap::new();
    for sh in &plan.sheets {
        for p in &sh.placements {
            out.entry(p.page).or_insert_with(|| {
                crate::export::scene_for(doc, p.page, &cache)
                    .unwrap_or_else(|| empty_scene(1.0, 1.0))
            });
        }
    }
    out
}

/// Cenas das páginas de uma folha (prévia).
pub fn page_scenes_for(doc: &Document, sheet: &Sheet) -> HashMap<usize, Scene> {
    let cache = ImageCache::default();
    let mut out = HashMap::new();
    for p in &sheet.placements {
        out.entry(p.page).or_insert_with(|| {
            crate::export::scene_for(doc, p.page, &cache).unwrap_or_else(|| empty_scene(1.0, 1.0))
        });
    }
    out
}

fn empty_scene(w: f64, h: f64) -> Scene {
    Scene {
        width: w,
        height: h,
        background: None,
        bleed: 0.0,
        items: Vec::new(),
        missing_fonts: Vec::new(),
        blends: Default::default(),
        overprints: Default::default(),
        item_bounds: Default::default(),
    }
}

/// Folha → pixels a `dpi` (X, Y), com o espelho.
pub fn sheet_view(paper: &Paper, dpi: [f64; 2], mirror: bool) -> Affine {
    let (sx, sy) = (
        dpi[0] / crate::units::UM_PER_INCH,
        dpi[1] / crate::units::UM_PER_INCH,
    );
    let v = Affine::new([sx, 0.0, 0.0, -sy, 0.0, paper.height * sy]);
    if mirror {
        v * Affine::new([-1.0, 0.0, 0.0, 1.0, paper.width, 0.0])
    } else {
        v
    }
}

/// Renderiza o retângulo de pixels `px` (x0, y0, largura, altura) da folha:
/// papel branco, as páginas recortadas, as marcas; separação em cinza e
/// negativo quando pedidos.
#[allow(clippy::too_many_arguments)]
pub fn render_region(
    scenes: &HashMap<usize, Scene>,
    marks: &Scene,
    sheet: &Sheet,
    paper: &Paper,
    s: &PrintSettings,
    dpi: [f64; 2],
    px: (i64, i64, u32, u32),
) -> Option<sk::Pixmap> {
    let (x0, y0, w, h) = px;
    let mut pm = sk::Pixmap::new(w.max(1), h.max(1))?;
    pm.fill(sk::Color::WHITE);
    let view = Affine::translate((-x0 as f64, -y0 as f64)) * sheet_view(paper, dpi, s.mirror);
    let opts = RasterOptions {
        antialias: true,
        background: None,
    };
    let band = Rect::new(0.0, 0.0, w as f64, h as f64);
    for pl in &sheet.placements {
        let Some(scene) = scenes.get(&pl.page) else {
            continue;
        };
        let dev = (view * pl.m).transform_rect_bbox(pl.clip).intersect(band);
        let (ix0, iy0) = (dev.x0.floor(), dev.y0.floor());
        let (iw, ih) = ((dev.x1.ceil() - ix0) as i64, (dev.y1.ceil() - iy0) as i64);
        if iw <= 0 || ih <= 0 {
            continue;
        }
        let sub_view = Affine::translate((-ix0, -iy0)) * view * pl.m;
        let plated;
        let scene = match &sheet.plate {
            Some(p) => {
                plated = separation::plate_scene(scene, p);
                &plated
            }
            None => scene,
        };
        // Recorte: o plano só gira 90°, então a caixa é um retângulo em
        // pixels e a cena pinta num pixmap do tamanho dela.
        if let Some(part) = raster::render(scene, sub_view, iw as u32, ih as u32, &opts) {
            pm.draw_pixmap(
                ix0 as i32,
                iy0 as i32,
                part.as_ref(),
                &sk::PixmapPaint::default(),
                sk::Transform::identity(),
                None,
            );
        }
    }
    let marks = match &sheet.plate {
        Some(p) => separation::plate_scene(marks, p),
        None => marks.clone(),
    };
    if !marks.items.is_empty() {
        let mp = raster::render(&marks, view, w, h, &opts)?;
        pm.draw_pixmap(
            0,
            0,
            mp.as_ref(),
            &sk::PixmapPaint::default(),
            sk::Transform::identity(),
            None,
        );
    }
    if s.negative {
        for p in pm.pixels_mut() {
            let c = p.demultiply();
            *p = sk::ColorU8::from_rgba(255 - c.red(), 255 - c.green(), 255 - c.blue(), 255)
                .premultiply();
        }
    }
    Some(pm)
}

/// PDF com uma página por folha (GD-PRT-03). Negativo não se aplica.
pub fn sheets_pdf(
    doc: &Document,
    plan: &Plan,
    paper: &Paper,
    s: &PrintSettings,
    opts: &ExportOptions,
) -> Result<(Vec<u8>, PdfReport), Error> {
    let base = page_scenes(doc, plan);
    // Cenas por folha (chapas e marcas) guardadas antes das referências.
    let mut store: Vec<Scene> = Vec::new();
    let mut idx: Vec<Vec<(usize, Affine, Option<Rect>)>> = Vec::new();
    let mut base_idx: HashMap<usize, usize> = HashMap::new();
    for (p, sc) in base {
        base_idx.insert(p, store.len());
        store.push(sc);
    }
    for sh in &plan.sheets {
        let mut parts = Vec::new();
        for pl in &sh.placements {
            let Some(&i) = base_idx.get(&pl.page) else {
                continue;
            };
            let i = match &sh.plate {
                Some(p) => {
                    let sc = separation::plate_scene(&store[i], p);
                    store.push(sc);
                    store.len() - 1
                }
                None => i,
            };
            parts.push((i, pl.m, Some(pl.clip)));
        }
        let mut marks = marks_scene(plan, sh, s, paper);
        if let Some(p) = &sh.plate {
            marks = separation::plate_scene(&marks, p);
        }
        if !marks.items.is_empty() {
            store.push(marks);
            parts.push((store.len() - 1, Affine::IDENTITY, None));
        }
        idx.push(parts);
    }
    let sheets: Vec<PdfSheet> = idx
        .iter()
        .map(|parts| {
            PdfSheet::new(
                paper.width,
                paper.height,
                s.mirror,
                parts.iter().map(|(i, m, c)| (&store[*i], *m, *c)).collect(),
            )
        })
        .collect();
    let cms = crate::cms::Cms::new(&doc.data.settings.color);
    Ok(crate::export::pdf::write_sheets_pdf(
        &sheets,
        doc.metadata.title.as_deref(),
        opts,
        Some(&cms),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(pages: &[(f64, f64)]) -> Document {
        let mut d = Document::new();
        for &(w, h) in pages {
            let mut p = Page::new(w * MM, h * MM);
            let mut o = Object::new(ObjectKind::Rectangle(RectData {
                width: w * MM,
                height: h * MM,
                corners: None,
                extra: Map::new(),
            }));
            o.fill = Some(Fill::solid(Color::rgb(1.0, 0.0, 0.0)));
            p.layers[0].objects.push(o);
            d.data.pages.push(p);
        }
        d
    }

    fn a4() -> Paper {
        Paper {
            width: 210.0 * MM,
            height: 297.0 * MM,
            printable: Rect::new(5.0 * MM, 5.0 * MM, 205.0 * MM, 292.0 * MM),
        }
    }

    fn trim(pl: &Placement, d: &Document) -> Rect {
        let p = &d.data.pages[pl.page];
        pl.m.transform_rect_bbox(Rect::new(0.0, 0.0, p.width, p.height))
    }

    #[test]
    fn actual_size_centered() {
        let d = doc(&[(90.0, 50.0)]);
        let plan = plan(&d, 0, &PrintSettings::default(), &a4()).unwrap();
        assert_eq!(plan.sheets.len(), 1);
        let t = trim(&plan.sheets[0].placements[0], &d);
        assert!((t.center().x - 105.0 * MM).abs() < 1.0 && (t.center().y - 148.5 * MM).abs() < 1.0);
        assert!((t.width() - 90.0 * MM).abs() < 1.0);
        assert!(plan.warnings.is_empty());
    }

    #[test]
    fn fit_rotates_landscape_page() {
        let d = doc(&[(400.0, 200.0)]);
        let s = PrintSettings {
            scale: Scale::Fit,
            ..Default::default()
        };
        let plan = plan(&d, 0, &s, &a4()).unwrap();
        assert!(plan.rotated, "paisagem gira no retrato");
        let t = trim(&plan.sheets[0].placements[0], &d);
        assert!(contains(a4().printable, t));
        assert!((plan.scale - 287.0 / 400.0).abs() < 1e-3, "{}", plan.scale);
        // Tamanho real que não cabe: aviso.
        let plan = plan_fn(
            &d,
            &PrintSettings {
                auto_rotate: false,
                ..Default::default()
            },
        );
        assert!(!plan.warnings.is_empty());
    }

    fn plan_fn(d: &Document, s: &PrintSettings) -> Plan {
        plan(d, 0, s, &a4()).unwrap()
    }

    #[test]
    fn tiles_cover_banner_with_overlap() {
        let d = doc(&[(1000.0, 500.0)]);
        let s = PrintSettings {
            tile: true,
            ..Default::default()
        };
        let plan = plan_fn(&d, &s);
        // 200 mm úteis, 190 de passo: 6 colunas e 2 linhas (passo 277 mm).
        let n = plan.sheets.len();
        assert_eq!(n, 6 * 2, "{n}");
        // A união dos ladrilhos (vistos na página) cobre a página inteira.
        let mut cover: Option<Rect> = None;
        for sh in &plan.sheets {
            let inv = sh.placements[0].m.inverse();
            let r = inv.transform_rect_bbox(a4().printable);
            cover = Some(cover.map_or(r, |c| c.union(r)));
        }
        let c = cover.unwrap();
        assert!(
            c.x0 <= 0.0 && c.y0 <= 0.0 && c.x1 >= 1000.0 * MM && c.y1 >= 500.0 * MM,
            "{c:?}"
        );
    }

    #[test]
    fn nup_fills_sheet_and_gang_marks() {
        let d = doc(&[(90.0, 50.0)]);
        let s = PrintSettings {
            nup: true,
            crop_marks: true,
            ..Default::default()
        };
        let plan = plan_fn(&d, &s);
        let sh = &plan.sheets[0];
        // Retrato: 2 colunas × 5 linhas; girado: 3 × 3 = 9 → fica 10.
        assert_eq!(sh.placements.len(), 10, "{}", sh.placements.len());
        for pl in &sh.placements {
            assert!(contains(a4().printable, trim(pl, &d)));
        }
        // Marcas de corte só fora da grade: nenhuma dentro de uma célula.
        for m in &sh.marks {
            if let Mark::Line(r) = m {
                assert!(
                    sh.placements
                        .iter()
                        .all(|pl| trim(pl, &d).intersect(*r).area() <= 0.0)
                );
            }
        }
        assert!(sh.marks.len() >= 4 * 4);
    }

    #[test]
    fn separations_and_copies() {
        let d = doc(&[(90.0, 50.0), (90.0, 50.0)]);
        let s = PrintSettings {
            separations: true,
            ..Default::default()
        };
        let plan = plan_fn(&d, &s);
        assert_eq!(plan.sheets.len(), 2 * 4, "4 chapas de processo por página");
        assert_eq!(plan.print_order(2, true)[..9], [0, 1, 2, 3, 4, 5, 6, 7, 0]);
        assert_eq!(plan.print_order(2, false)[..4], [0, 0, 1, 1]);
        let r = PrintSettings {
            range: PageRange::Custom("2".into()),
            ..Default::default()
        };
        assert_eq!(plan_fn(&d, &r).sheets[0].placements[0].page, 1);
    }

    #[test]
    fn marks_dropped_when_no_room() {
        let d = doc(&[(200.0, 287.0)]);
        let s = PrintSettings {
            crop_marks: true,
            ..Default::default()
        };
        let plan = plan_fn(&d, &s);
        assert!(plan.sheets[0].marks.is_empty());
        assert_eq!(plan.warnings.len(), 1);
    }

    #[test]
    fn raster_matches_page_png() {
        // Folha do tamanho da página, sem marcas: igual à exportação PNG.
        let d = doc(&[(20.0, 10.0)]);
        let paper = Paper::full(20.0 * MM, 10.0 * MM);
        let s = PrintSettings::default();
        let plan = plan(&d, 0, &s, &paper).unwrap();
        let scenes = page_scenes(&d, &plan);
        let marks = marks_scene(&plan, &plan.sheets[0], &s, &paper);
        let dpi = 72.0;
        let (w, h) = (57, 28);
        let pm = render_region(
            &scenes,
            &marks,
            &plan.sheets[0],
            &paper,
            &s,
            [dpi, dpi],
            (0, 0, w, h),
        )
        .unwrap();
        let px = pm.pixel(w / 2, h / 2).unwrap().demultiply();
        assert_eq!((px.red(), px.green(), px.blue()), (255, 0, 0));
        // Espelho e negativo.
        let n = PrintSettings {
            negative: true,
            mirror: true,
            ..Default::default()
        };
        let pm = render_region(
            &scenes,
            &marks,
            &plan.sheets[0],
            &paper,
            &n,
            [dpi, dpi],
            (0, 0, w, h),
        )
        .unwrap();
        let px = pm.pixel(w / 2, h / 2).unwrap().demultiply();
        assert_eq!((px.red(), px.green(), px.blue()), (0, 255, 255));
        // Faixa: a metade de baixo sozinha.
        let band = render_region(
            &scenes,
            &marks,
            &plan.sheets[0],
            &paper,
            &s,
            [dpi, dpi],
            (0, 14, w, 14),
        )
        .unwrap();
        assert_eq!(band.height(), 14);
    }

    #[test]
    fn pdf_one_page_per_sheet() {
        let d = doc(&[(90.0, 50.0)]);
        let s = PrintSettings {
            crop_marks: true,
            reg_marks: true,
            color_bar: true,
            info: true,
            separations: true,
            ..Default::default()
        };
        let plan = plan_fn(&d, &s);
        let (pdf, _) = sheets_pdf(&d, &plan, &a4(), &s, &ExportOptions::default()).unwrap();
        let doc = lopdf::Document::load_mem(&pdf).unwrap();
        assert_eq!(doc.get_pages().len(), 4);
    }
}
