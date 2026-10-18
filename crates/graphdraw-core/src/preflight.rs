//! Verificação prévia de impressão (doc 28, GD-PRE-01..10): cobertura de
//! tinta, linha fina, zona de segurança, encostado sem sangria e preto sem
//! sobreimpressão, com as correções automáticas e "estender até a sangria".

use kurbo::{Affine, Rect};
use serde::{Deserialize, Serialize};

use crate::edit::history::Editor;
use crate::edit::{ops, query};
use crate::model::{Color, Document, Fill, Object, ObjectKind};

/// µm por ponto tipográfico.
pub const PT: f64 = 25_400.0 / 72.0;

/// Limites por documento (GD-PRE-02).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PreflightSettings {
    /// Cobertura total de tinta máxima, em % (C+M+Y+K).
    pub tac: f64,
    /// Espessura mínima de linha positiva (µm).
    pub min_line: f64,
    /// Espessura mínima de linha negativa (clara) (µm).
    pub min_line_negative: f64,
    /// Zona de segurança do texto até o corte (µm).
    pub safe_zone: f64,
    /// Distância da borda tida como "encostada" sem sangria (µm).
    pub near_edge: f64,
    /// Corpo até o qual texto preto exige sobreimpressão (µm).
    pub black_text_size: f64,
}

/// TAC para papel revestido e não revestido (predefinições).
pub const TAC_COATED: f64 = 300.0;
pub const TAC_UNCOATED: f64 = 260.0;

impl Default for PreflightSettings {
    fn default() -> Self {
        PreflightSettings {
            tac: TAC_COATED,
            min_line: 0.25 * PT,
            min_line_negative: 0.5 * PT,
            safe_zone: 5_000.0,
            near_edge: 1_500.0,
            black_text_size: 12.0 * PT,
        }
    }
}

/// Tipo de problema.
#[derive(Debug, Clone, PartialEq)]
pub enum IssueKind {
    /// Cobertura total (%) do preenchimento ou do contorno (`stroke`).
    Ink { tac: f64, stroke: bool },
    /// Espessura efetiva e o mínimo que valia (µm).
    ThinLine { width: f64, min: f64 },
    /// Texto dentro da faixa entre o corte e a zona de segurança.
    SafeZone,
    /// Borda a menos da distância de encostado sem chegar à sangria.
    NearEdge,
    /// Preto 100% K sem sobreimpressão (texto pequeno ou linha).
    BlackKnockout,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Issue {
    pub id: String,
    pub kind: IssueKind,
}

impl Issue {
    /// Há correção automática?
    pub fn fixable(&self) -> bool {
        !matches!(self.kind, IssueKind::SafeZone)
    }
}

/// CMYK 0–1 de uma cor: a do próprio modelo, ou convertida pelo perfil do
/// documento (sem perfil, por fórmula). Cor especial conta 0 (chapa à parte).
pub fn cmyk_of(c: &Color) -> [f64; 4] {
    let v = |i: usize| c.values.get(i).copied().unwrap_or(0.0).clamp(0.0, 1.0);
    match c.model.as_str() {
        "cmyk" => [v(0), v(1), v(2), v(3)],
        "spot" | "registration" => [0.0; 4],
        "gray" => [0.0, 0.0, 0.0, 1.0 - v(0)],
        _ => {
            let rgb = c.to_srgb();
            if let Some(k) = crate::cms::current().and_then(|m| m.rgb_to_cmyk(rgb)) {
                return k;
            }
            let k = crate::export::pdf::formula_cmyk(rgb);
            [k.values[0], k.values[1], k.values[2], k.values[3]]
        }
    }
}

/// Cobertura total em %.
pub fn tac(c: &Color) -> f64 {
    cmyk_of(c).iter().sum::<f64>() * 100.0
}

fn is_black_100k(c: &Color) -> bool {
    let [cc, m, y, k] = cmyk_of(c);
    cc < 0.01 && m < 0.01 && y < 0.01 && k > 0.99
}

/// Cor clara (linha negativa): luminância ≥ 0,8.
fn is_light(c: &Color) -> bool {
    let [r, g, b] = c.to_srgb();
    0.2126 * r + 0.7152 * g + 0.0722 * b >= 0.8
}

fn fill_tac(f: &Fill) -> f64 {
    match f {
        Fill::Solid { color, .. } => tac(color),
        Fill::Gradient(g) => g.stops.iter().map(|s| tac(&s.color)).fold(0.0, f64::max),
        _ => 0.0,
    }
}

/// Espessura efetiva do contorno (µm) sob a matriz `m`.
fn effective_width(s: &crate::model::Stroke, m: Affine) -> f64 {
    if s.scale_with_object {
        s.width * m.determinant().abs().sqrt()
    } else {
        s.width
    }
}

fn max_font(o: &Object) -> f64 {
    match &o.kind {
        ObjectKind::Text(t) => t
            .paragraphs
            .iter()
            .flat_map(|p| &p.spans)
            .map(|s| s.font.size)
            .fold(0.0, f64::max),
        _ => 0.0,
    }
}

struct Ctx<'a> {
    s: &'a PreflightSettings,
    trim: Rect,
    bleed: f64,
    out: Vec<Issue>,
}

impl Ctx<'_> {
    fn push(&mut self, id: &str, kind: IssueKind) {
        self.out.push(Issue {
            id: id.to_string(),
            kind,
        });
    }

    fn object(&mut self, o: &Object, parent: Affine) {
        if !o.visible {
            return;
        }
        let m = parent * o.transform.affine();
        if let ObjectKind::Group(g) = &o.kind {
            for c in &g.children {
                self.object(c, m);
            }
            return;
        }
        let limit = self.s.tac + 0.5;
        if let Some(f) = &o.fill {
            let t = fill_tac(f);
            if t > limit {
                self.push(
                    &o.id,
                    IssueKind::Ink {
                        tac: t,
                        stroke: false,
                    },
                );
            }
        }
        let text = matches!(o.kind, ObjectKind::Text(_));
        if let Some(st) = &o.stroke {
            let t = tac(&st.color);
            if t > limit {
                self.push(
                    &o.id,
                    IssueKind::Ink {
                        tac: t,
                        stroke: true,
                    },
                );
            }
            let w = effective_width(st, m);
            let min = if is_light(&st.color) {
                self.s.min_line_negative
            } else {
                self.s.min_line
            };
            if w + 1e-6 < min {
                self.push(&o.id, IssueKind::ThinLine { width: w, min });
            }
        }
        let black_fill = matches!(&o.fill, Some(Fill::Solid { color, overprint: false, .. }) if is_black_100k(color));
        let black_line = o
            .stroke
            .as_ref()
            .is_some_and(|s| !s.overprint && is_black_100k(&s.color));
        if (text && black_fill && max_font(o) <= self.s.black_text_size + 1e-6) || black_line {
            self.push(&o.id, IssueKind::BlackKnockout);
        }
        let Some(b) = query::bounds(o, parent, false) else {
            return;
        };
        if !b.overlaps(self.trim) {
            return;
        }
        if text {
            let safe = self.trim.inset(-self.s.safe_zone);
            if !(safe.contains_rect(b)) {
                self.push(&o.id, IssueKind::SafeZone);
            }
        } else if self.bleed > 0.0
            && !near_sides(b, self.trim, self.bleed, self.s.near_edge).is_empty()
        {
            self.push(&o.id, IssueKind::NearEdge);
        }
    }
}

/// Lados (0 esq., 1 base, 2 dir., 3 topo) encostados no corte sem chegar à
/// sangria.
fn near_sides(b: Rect, trim: Rect, bleed: f64, near: f64) -> Vec<usize> {
    let eps = 1.0;
    let mut v = Vec::new();
    let gap = [
        b.x0 - trim.x0,
        b.y0 - trim.y0,
        trim.x1 - b.x1,
        trim.y1 - b.y1,
    ];
    for (i, g) in gap.iter().enumerate() {
        if *g > -bleed + eps && *g < near {
            v.push(i);
        }
    }
    v
}

/// Problemas da página (GD-PRE-01).
pub fn check(doc: &Document, page: usize) -> Vec<Issue> {
    let Some(pg) = doc.data.pages.get(page) else {
        return Vec::new();
    };
    let mut cx = Ctx {
        s: &doc.data.settings.preflight,
        trim: Rect::new(0.0, 0.0, pg.width, pg.height),
        bleed: pg.bleed.unwrap_or(doc.data.settings.bleed).max(0.0),
        out: Vec::new(),
    };
    for layer in &pg.layers {
        if !layer.visible || !layer.printable {
            continue;
        }
        for o in &layer.objects {
            cx.object(o, Affine::IDENTITY);
        }
    }
    cx.out
}

/// Reduz a cor até `limit` % tirando do CMY (o K fica, salvo se ele sozinho
/// passar do limite).
pub fn reduce_ink(c: &Color, limit: f64) -> Color {
    let [cc, m, y, k] = cmyk_of(c);
    let max = limit / 100.0;
    let k2 = k.min(max);
    let cmy = cc + m + y;
    let room = (max - k2).max(0.0);
    let f = if cmy > room && cmy > 0.0 {
        room / cmy
    } else {
        1.0
    };
    let mut out = Color::cmyk(cc * f, m * f, y * f, k2);
    out.name = c.name.clone();
    out
}

/// Correção automática do problema (GD-PRE-03/04/06/07). Um passo de
/// desfazer; `false` se não havia o que fazer.
pub fn fix(ed: &mut Editor, page: usize, issue: &Issue) -> bool {
    let s = ed.doc.data.settings.preflight.clone();
    let ids = [issue.id.clone()];
    match &issue.kind {
        IssueKind::Ink { stroke, .. } => {
            let mut done = false;
            ops::modify(ed, &ids, "Reduzir cobertura de tinta", |o| {
                if *stroke {
                    if let Some(st) = &mut o.stroke {
                        st.color = reduce_ink(&st.color, s.tac);
                        done = true;
                    }
                } else {
                    match &mut o.fill {
                        Some(Fill::Solid { color, .. }) => {
                            *color = reduce_ink(color, s.tac);
                            done = true;
                        }
                        Some(Fill::Gradient(g)) => {
                            for st in &mut g.stops {
                                st.color = reduce_ink(&st.color, s.tac);
                            }
                            done = true;
                        }
                        _ => {}
                    }
                }
                done
            });
            done
        }
        IssueKind::ThinLine { min, .. } => {
            let Some(r) = ed.find(&issue.id) else {
                return false;
            };
            let parent = ed.parent_affine(&r);
            let mut done = false;
            ops::modify(ed, &ids, "Engrossar linha", |o| {
                let m = parent * o.transform.affine();
                let scale = if o.stroke.as_ref().is_some_and(|s| s.scale_with_object) {
                    m.determinant().abs().sqrt().max(1e-12)
                } else {
                    1.0
                };
                if let Some(st) = &mut o.stroke {
                    st.width = min / scale;
                    done = true;
                }
                done
            });
            done
        }
        IssueKind::BlackKnockout => {
            let mut done = false;
            ops::modify(ed, &ids, "Sobreimprimir preto", |o| {
                if let Some(Fill::Solid { overprint, .. }) = &mut o.fill {
                    *overprint = true;
                    done = true;
                }
                if let Some(st) = &mut o.stroke {
                    st.overprint = true;
                    done = true;
                }
                done
            });
            done
        }
        IssueKind::NearEdge => !extend_to_bleed(ed, page, &ids).is_empty(),
        IssueKind::SafeZone => false,
    }
}

/// Estender até a sangria (GD-PRE-10): o lado encostado no corte vai até o
/// corte mais a sangria; o lado oposto fica. Devolve os IDs mudados; um
/// passo de desfazer.
pub fn extend_to_bleed(ed: &mut Editor, page: usize, ids: &[String]) -> Vec<String> {
    let Some(pg) = ed.doc.data.pages.get(page) else {
        return Vec::new();
    };
    let trim = Rect::new(0.0, 0.0, pg.width, pg.height);
    let bleed = pg.bleed.unwrap_or(ed.doc.data.settings.bleed).max(0.0);
    let near = ed.doc.data.settings.preflight.near_edge;
    if bleed <= 0.0 {
        return Vec::new();
    }
    let mut plan = Vec::new();
    for id in ids {
        let Some(b) = ed.find(id).and_then(|r| ops::object_bounds(ed, &r, false)) else {
            continue;
        };
        let sides = near_sides(b, trim, bleed, near);
        if sides.is_empty() || b.width() <= 0.0 || b.height() <= 0.0 {
            continue;
        }
        let out = trim.inflate(bleed, bleed);
        let mut n = b;
        for s in sides {
            match s {
                0 => n.x0 = out.x0,
                1 => n.y0 = out.y0,
                2 => n.x1 = out.x1,
                _ => n.y1 = out.y1,
            }
        }
        let sx = n.width() / b.width();
        let sy = n.height() / b.height();
        let m = Affine::translate((n.x0, n.y0))
            * Affine::scale_non_uniform(sx, sy)
            * Affine::translate((-b.x0, -b.y0));
        plan.push((id.clone(), m));
    }
    if plan.is_empty() {
        return Vec::new();
    }
    let done: Vec<String> = plan.iter().map(|(id, _)| id.clone()).collect();
    ed.transaction("Estender até a sangria", |ed| {
        for (id, m) in plan {
            ops::transform(ed, &[id], m, "Estender");
        }
    });
    done
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        FontSpec, Page, Paragraph, RectData, Stroke, TextData, TextSpan, Transform,
    };
    use serde_json::Map;

    fn setup(objs: Vec<Object>, bleed: f64) -> (Editor, Vec<String>) {
        let mut d = Document::new();
        let mut p = Page::new(210_000.0, 297_000.0);
        p.bleed = Some(bleed);
        d.data.pages.push(p);
        let mut ed = Editor::new(d);
        let ids = objs
            .into_iter()
            .map(|o| ops::add_object(&mut ed, 0, 0, o, "o"))
            .collect();
        (ed, ids)
    }

    fn rect(x: f64, y: f64, w: f64, h: f64, fill: Option<Fill>) -> Object {
        let mut o = Object::new(ObjectKind::Rectangle(RectData {
            width: w,
            height: h,
            corners: None,
            extra: Map::new(),
        }));
        o.transform = Transform::translate(x, y);
        o.fill = fill;
        o
    }

    fn text(x: f64, y: f64, size: f64) -> Object {
        let mut o = Object::new(ObjectKind::Text(TextData {
            text_kind: "paragraph".into(),
            frame: Some([50_000.0, 10_000.0]),
            paragraphs: vec![Paragraph {
                align: "left".into(),
                spans: vec![TextSpan {
                    text: "Oferta".into(),
                    font: FontSpec {
                        family: "Sans".into(),
                        size,
                        weight: 400,
                        italic: false,
                        extra: Map::new(),
                    },
                    fill: None,
                    extra: Map::new(),
                }],
                extra: Map::new(),
            }],
            line_spacing: 1.2,
            path: None,
            exclusions: Default::default(),
            extra: Map::new(),
        }));
        o.transform = Transform::translate(x, y);
        o.fill = Some(Fill::solid(Color::cmyk(0.0, 0.0, 0.0, 1.0)));
        o
    }

    fn kinds(ed: &Editor) -> Vec<IssueKind> {
        check(&ed.doc, 0).into_iter().map(|i| i.kind).collect()
    }

    /// GD-PRE-03: CMYK 400% → problema; corrigir → ≤ 300% com K 100%.
    #[test]
    fn gd_pre_03_ink_coverage_and_fix() {
        let f = Fill::solid(Color::cmyk(1.0, 1.0, 1.0, 1.0));
        let (mut ed, ids) = setup(
            vec![rect(20_000.0, 20_000.0, 50_000.0, 50_000.0, Some(f))],
            0.0,
        );
        let issues = check(&ed.doc, 0);
        assert!(
            matches!(issues[0].kind, IssueKind::Ink { tac, stroke: false } if (tac - 400.0).abs() < 1e-6)
        );
        assert!(fix(&mut ed, 0, &issues[0]));
        let o = ed.get(&ed.find(&ids[0]).unwrap()).unwrap();
        let Some(Fill::Solid { color, .. }) = &o.fill else {
            panic!()
        };
        assert!(tac(color) <= 300.0 + 1e-6);
        assert!((cmyk_of(color)[3] - 1.0).abs() < 1e-9, "K fica");
        assert!(kinds(&ed).is_empty());
    }

    /// GD-PRE-04: 0,1 pt preta → fina (corrige para 0,25 pt); branca de
    /// 0,3 pt → fina negativa.
    #[test]
    fn gd_pre_04_thin_lines() {
        let mut a = rect(20_000.0, 20_000.0, 50_000.0, 50_000.0, None);
        a.stroke = Some(Stroke::new(0.1 * PT, Color::cmyk(0.0, 0.0, 0.0, 0.5)));
        let mut b = rect(100_000.0, 20_000.0, 50_000.0, 50_000.0, None);
        b.stroke = Some(Stroke::new(0.3 * PT, Color::rgb(1.0, 1.0, 1.0)));
        let (mut ed, ids) = setup(vec![a, b], 0.0);
        let issues = check(&ed.doc, 0);
        let thin: Vec<&Issue> = issues
            .iter()
            .filter(|i| matches!(i.kind, IssueKind::ThinLine { .. }))
            .collect();
        assert_eq!(thin.len(), 2);
        assert!(thin.iter().any(|i| i.id == ids[1]
            && matches!(i.kind, IssueKind::ThinLine { min, .. } if (min - 0.5 * PT).abs() < 1e-6)));
        let first = thin.iter().find(|i| i.id == ids[0]).unwrap();
        assert!(fix(&mut ed, 0, first));
        let w = ed
            .get(&ed.find(&ids[0]).unwrap())
            .unwrap()
            .stroke
            .as_ref()
            .unwrap()
            .width;
        assert!((w - 0.25 * PT).abs() < 1e-6);
    }

    /// GD-PRE-05/07: texto preto de 8 pt sem sobreimpressão → problema;
    /// corrigir liga a sobreimpressão; texto encostado no corte → zona.
    #[test]
    fn gd_pre_05_07_black_text_and_safe_zone() {
        let (mut ed, ids) = setup(
            vec![
                text(30_000.0, 150_000.0, 8.0 * PT),
                text(1_000.0, 290_000.0, 30.0 * PT),
            ],
            0.0,
        );
        let issues = check(&ed.doc, 0);
        let black = issues
            .iter()
            .find(|i| i.kind == IssueKind::BlackKnockout)
            .unwrap();
        assert_eq!(black.id, ids[0], "só o pequeno");
        assert!(
            issues
                .iter()
                .any(|i| i.id == ids[1] && i.kind == IssueKind::SafeZone)
        );
        assert!(
            !issues
                .iter()
                .any(|i| i.id == ids[0] && i.kind == IssueKind::SafeZone)
        );
        assert!(fix(&mut ed, 0, black));
        assert!(
            !check(&ed.doc, 0)
                .iter()
                .any(|i| i.kind == IssueKind::BlackKnockout)
        );
        let o = ed.get(&ed.find(&ids[0]).unwrap()).unwrap();
        assert!(matches!(
            o.fill,
            Some(Fill::Solid {
                overprint: true,
                ..
            })
        ));
    }

    /// GD-PRE-06/10: fundo a 1 mm da borda com sangria de 3 mm → encostado;
    /// estender leva a borda a −3 mm; um passo de desfazer.
    #[test]
    fn gd_pre_06_10_extend_to_bleed() {
        let f = Some(Fill::solid(Color::cmyk(0.2, 0.0, 0.0, 0.0)));
        let (mut ed, ids) = setup(
            vec![rect(1_000.0, 50_000.0, 100_000.0, 100_000.0, f)],
            3_000.0,
        );
        let issues = check(&ed.doc, 0);
        assert!(issues.iter().any(|i| i.kind == IssueKind::NearEdge));
        let undo = ed.undo_len();
        assert_eq!(extend_to_bleed(&mut ed, 0, &ids), ids);
        assert_eq!(ed.undo_len(), undo + 1);
        let b = ops::object_bounds(&ed, &ed.find(&ids[0]).unwrap(), false).unwrap();
        assert!(
            (b.x0 + 3_000.0).abs() < 1e-6 && (b.x1 - 101_000.0).abs() < 1e-6,
            "{b:?}"
        );
        assert!(
            !check(&ed.doc, 0)
                .iter()
                .any(|i| i.kind == IssueKind::NearEdge)
        );
        // Sem sangria, nada a estender.
        let (mut ed2, ids2) = setup(vec![rect(1_000.0, 0.0, 10.0, 10.0, None)], 0.0);
        assert!(extend_to_bleed(&mut ed2, 0, &ids2).is_empty());
    }

    /// GD-PRE-08: a sobreimpressão vai ao PDF no estado gráfico (op/OP, OPM 1).
    #[test]
    fn gd_pre_08_overprint_in_pdf() {
        let mut o = rect(
            20_000.0,
            20_000.0,
            50_000.0,
            50_000.0,
            Some(Fill::solid(Color::cmyk(0.0, 0.0, 0.0, 1.0))),
        );
        o.stroke = Some(Stroke::new(PT, Color::cmyk(0.0, 0.0, 0.0, 1.0)));
        let (mut ed, ids) = setup(vec![o], 0.0);
        let plain = crate::export::pdf_document(&ed.doc, &Default::default()).unwrap();
        assert!(!plain.windows(4).any(|w| w == b"/OPM"));
        let issue = Issue {
            id: ids[0].clone(),
            kind: IssueKind::BlackKnockout,
        };
        assert!(fix(&mut ed, 0, &issue));
        let pdf = crate::export::pdf_document(&ed.doc, &Default::default()).unwrap();
        let has = |pat: &[u8]| pdf.windows(pat.len()).any(|w| w == pat);
        assert!(has(b"/op true") && has(b"/OP true") && has(b"/OPM 1"));
    }

    /// GD-PRE-02: os limites ficam no documento e voltam do JSON.
    #[test]
    fn gd_pre_02_settings_roundtrip() {
        let mut d = Document::new();
        d.data.settings.preflight.tac = TAC_UNCOATED;
        let json = serde_json::to_value(&d.data.settings).unwrap();
        assert_eq!(json["preflight"]["tac"], 260.0);
        let back: crate::model::Settings = serde_json::from_value(json).unwrap();
        assert_eq!(back.preflight.tac, TAC_UNCOATED);
        let plain = serde_json::to_value(crate::model::Settings::default()).unwrap();
        assert!(plain.get("preflight").is_none(), "padrão não grava");
    }
}
