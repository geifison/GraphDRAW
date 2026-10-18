//! Aninhamento em chapa (GD-PRE-30): encaixa as peças selecionadas em
//! chapas de tamanho dado, com as rotações permitidas, o sentido da fibra
//! e a folga da ferramenta.
//!
//! Método: a chapa vira uma grade de células; cada peça (forma real, furos
//! contados como cheios) é rasterizada em cada rotação, alargada pela
//! folga, e vai para a posição mais baixa e mais à esquerda em que cabe
//! (preenchimento inferior-esquerdo), peças maiores primeiro. Peça que não
//! cabe na chapa aberta abre outra ao lado; maior que a chapa fica onde
//! está.

use kurbo::{Affine, PathEl, Point, Rect, Shape};

use super::history::Editor;
use super::query::{bounds, local_outline};
use crate::model::{Color, Object, ObjectKind, RectData, Stroke, Transform};

/// Opções do aninhamento. Medidas em µm; `step` = passo da rotação livre
/// em graus.
#[derive(Debug, Clone, PartialEq)]
pub struct NestOptions {
    pub sheet_w: f64,
    pub sheet_h: f64,
    /// Folga entre peças (ferramenta) e margem da borda da chapa.
    pub gap: f64,
    pub margin: f64,
    /// `none`, `180`, `90` ou `free`.
    pub rotation: String,
    pub step: f64,
    /// Sentido da fibra: só 0° e 180° (a fibra fica alinhada).
    pub grain: bool,
    /// Canto inferior esquerdo da primeira chapa (página).
    pub origin: Point,
    /// Desenha o contorno de cada chapa (não imprime).
    pub outline: bool,
}

impl Default for NestOptions {
    fn default() -> Self {
        NestOptions {
            sheet_w: 1_000_000.0,
            sheet_h: 500_000.0,
            gap: 3_000.0,
            margin: 5_000.0,
            rotation: "90".into(),
            step: 15.0,
            grain: false,
            origin: Point::ORIGIN,
            outline: true,
        }
    }
}

/// Resultado: peças postas, chapas usadas, peças que não cabem e o
/// aproveitamento (área das peças / área das chapas usadas, 0–1).
#[derive(Debug, Clone, PartialEq)]
pub struct NestResult {
    pub placed: usize,
    pub sheets: usize,
    pub unplaced: Vec<String>,
    pub usage: f64,
}

/// Ângulos permitidos (graus).
pub fn angles(o: &NestOptions) -> Vec<f64> {
    if o.grain {
        return match o.rotation.as_str() {
            "none" => vec![0.0],
            _ => vec![0.0, 180.0],
        };
    }
    match o.rotation.as_str() {
        "none" => vec![0.0],
        "180" => vec![0.0, 180.0],
        "free" => {
            let s = o.step.clamp(1.0, 180.0);
            let n = (360.0 / s).floor() as usize;
            (0..n.max(1)).map(|k| k as f64 * s).collect()
        }
        _ => vec![0.0, 90.0, 180.0, 270.0],
    }
}

/// Contornos da peça na página (grupos pelos filhos).
fn polys(o: &Object, parent: Affine, out: &mut Vec<Vec<Point>>) {
    let m = parent * o.transform.affine();
    if let ObjectKind::Group(g) = &o.kind {
        for c in &g.children {
            polys(c, m, out);
        }
        return;
    }
    let Some(mut p) = local_outline(o) else {
        if let Some(b) = bounds(o, parent, false) {
            out.push(vec![
                Point::new(b.x0, b.y0),
                Point::new(b.x1, b.y0),
                Point::new(b.x1, b.y1),
                Point::new(b.x0, b.y1),
            ]);
        }
        return;
    };
    p.apply_affine(m);
    let mut cur = Vec::new();
    kurbo::flatten(&p, 100.0, |el| match el {
        PathEl::MoveTo(q) => {
            if cur.len() >= 2 {
                out.push(std::mem::take(&mut cur));
            }
            cur = vec![q];
        }
        PathEl::LineTo(q) => cur.push(q),
        PathEl::ClosePath if cur.len() >= 2 => out.push(std::mem::take(&mut cur)),
        _ => {}
    });
    if cur.len() >= 2 {
        out.push(cur);
    }
}

/// Peça rasterizada: linhas de células cheias (trechos [x0, x1)) a partir
/// do canto da caixa, tamanho em células e a caixa (página) já girada.
struct Raster {
    rows: Vec<Vec<(usize, usize)>>,
    w: usize,
    h: usize,
    bbox: Rect,
}

fn inside(poly: &[Point], p: Point) -> bool {
    let mut c = false;
    let n = poly.len();
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        if (a.y > p.y) != (b.y > p.y) && p.x < a.x + (p.y - a.y) / (b.y - a.y) * (b.x - a.x) {
            c = !c;
        }
    }
    c
}

/// Rasteriza os contornos (cada um cheio) e alarga `grow` células.
fn rasterize(ps: &[Vec<Point>], cell: f64, grow: usize) -> Option<Raster> {
    let bbox = ps.iter().flatten().fold(None::<Rect>, |r, p| {
        Some(r.map_or(Rect::from_points(*p, *p), |r| r.union_pt(*p)))
    })?;
    let w0 = ((bbox.width() / cell).ceil() as usize).max(1);
    let h0 = ((bbox.height() / cell).ceil() as usize).max(1);
    let mut grid = vec![vec![false; w0]; h0];
    for (y, row) in grid.iter_mut().enumerate() {
        for (x, c) in row.iter_mut().enumerate() {
            let q = Point::new(
                bbox.x0 + (x as f64 + 0.5) * cell,
                bbox.y0 + (y as f64 + 0.5) * cell,
            );
            *c = ps.iter().any(|p| p.len() >= 3 && inside(p, q));
        }
    }
    // Bordas (e linhas abertas) marcam as células que atravessam.
    for p in ps {
        let n = p.len();
        for i in 0..n {
            let (a, b) = (p[i], p[(i + 1) % n]);
            if i + 1 == n && n < 3 {
                break;
            }
            let k = (((b - a).hypot() / (cell * 0.5)).ceil() as usize).max(1);
            for j in 0..=k {
                let q = a + (b - a) * (j as f64 / k as f64);
                let x = (((q.x - bbox.x0) / cell) as usize).min(w0 - 1);
                let y = (((q.y - bbox.y0) / cell) as usize).min(h0 - 1);
                grid[y][x] = true;
            }
        }
    }
    let (w, h) = (w0 + 2 * grow, h0 + 2 * grow);
    let mut big = vec![vec![false; w]; h];
    let g2 = (grow * grow) as i64;
    for (y, row) in grid.iter().enumerate() {
        for (x, c) in row.iter().enumerate() {
            if !*c {
                continue;
            }
            for dy in 0..=2 * grow {
                for dx in 0..=2 * grow {
                    let (ex, ey) = (dx as i64 - grow as i64, dy as i64 - grow as i64);
                    if ex * ex + ey * ey <= g2 {
                        big[y + dy][x + dx] = true;
                    }
                }
            }
        }
    }
    let rows = big
        .iter()
        .map(|row| {
            let mut spans = Vec::new();
            let mut start = None;
            for (x, c) in row.iter().enumerate() {
                match (c, start) {
                    (true, None) => start = Some(x),
                    (false, Some(s)) => {
                        spans.push((s, x));
                        start = None;
                    }
                    _ => {}
                }
            }
            if let Some(s) = start {
                spans.push((s, row.len()));
            }
            spans
        })
        .collect();
    Some(Raster { rows, w, h, bbox })
}

/// Chapa: ocupação por linha em somas acumuladas.
struct Sheet {
    occ: Vec<Vec<bool>>,
    pre: Vec<Vec<u32>>,
}

impl Sheet {
    fn new(w: usize, h: usize) -> Sheet {
        Sheet {
            occ: vec![vec![false; w]; h],
            pre: vec![vec![0; w + 1]; h],
        }
    }
    fn fits(&self, r: &Raster, cx: usize, cy: usize) -> bool {
        r.rows.iter().enumerate().all(|(y, spans)| {
            let pre = &self.pre[cy + y];
            spans.iter().all(|(a, b)| pre[cx + b] == pre[cx + a])
        })
    }
    fn mark(&mut self, r: &Raster, cx: usize, cy: usize) {
        for (y, spans) in r.rows.iter().enumerate() {
            let row = &mut self.occ[cy + y];
            for (a, b) in spans {
                for c in &mut row[cx + a..cx + b] {
                    *c = true;
                }
            }
            let pre = &mut self.pre[cy + y];
            for x in 0..row.len() {
                pre[x + 1] = pre[x] + row[x] as u32;
            }
        }
    }
    /// Posição mais baixa e mais à esquerda em que `r` cabe.
    fn find(&self, r: &Raster) -> Option<(usize, usize)> {
        let (h, w) = (self.occ.len(), self.occ.first()?.len());
        if r.w > w || r.h > h {
            return None;
        }
        for cy in 0..=h - r.h {
            for cx in 0..=w - r.w {
                if self.fits(r, cx, cy) {
                    return Some((cx, cy));
                }
            }
        }
        None
    }
}

/// Encaixa as peças `sel` (de primeiro nível) e move-as num passo só.
pub fn nest(ed: &mut Editor, page: usize, sel: &[String], o: &NestOptions) -> NestResult {
    let mut res = NestResult {
        placed: 0,
        sheets: 0,
        unplaced: Vec::new(),
        usage: 0.0,
    };
    let (uw, uh) = (o.sheet_w - 2.0 * o.margin, o.sheet_h - 2.0 * o.margin);
    if uw <= 0.0 || uh <= 0.0 {
        res.unplaced = sel.to_vec();
        return res;
    }
    // Célula: ~400 na maior medida da chapa, nunca menor que 0,25 mm nem
    // que um terço da folga.
    let cell = (uw.max(uh) / 400.0)
        .max(250.0)
        .min((o.gap / 3.0).max(250.0))
        .max(uw.max(uh) / 1500.0);
    let (gw, gh) = ((uw / cell).floor() as usize, (uh / cell).floor() as usize);
    let grow = (o.gap / cell).ceil() as usize;
    let mut pieces: Vec<(String, Vec<Vec<Point>>, f64)> = sel
        .iter()
        .filter_map(|id| {
            let r = ed.find(id)?;
            let ob = ed.get(&r)?;
            let mut ps = Vec::new();
            polys(ob, ed.parent_affine(&r), &mut ps);
            let area: f64 = ps
                .iter()
                .filter(|p| p.len() >= 3)
                .map(|p| {
                    let mut b = kurbo::BezPath::new();
                    b.move_to(p[0]);
                    for q in &p[1..] {
                        b.line_to(*q);
                    }
                    b.close_path();
                    b.area().abs()
                })
                .sum();
            (!ps.is_empty()).then(|| (id.clone(), ps, area))
        })
        .collect();
    pieces.sort_by(|a, b| b.2.total_cmp(&a.2));
    let mut sheets: Vec<Sheet> = Vec::new();
    let mut moves: Vec<(String, Affine)> = Vec::new();
    let mut placed_area = 0.0;
    for (id, ps, area) in &pieces {
        let center = ps
            .iter()
            .flatten()
            .fold(Rect::from_points(ps[0][0], ps[0][0]), |r, p| r.union_pt(*p))
            .center();
        let mut best: Option<(usize, usize, usize, f64, Affine, Rect)> = None;
        for &ang in &angles(o) {
            let rot = Affine::rotate_about(ang.to_radians(), center);
            let rp: Vec<Vec<Point>> = ps
                .iter()
                .map(|p| p.iter().map(|q| rot * *q).collect())
                .collect();
            let Some(r) = rasterize(&rp, cell, grow) else {
                continue;
            };
            let mut found = None;
            for (si, s) in sheets.iter().enumerate() {
                if let Some((cx, cy)) = s.find(&r) {
                    found = Some((si, cx, cy));
                    break;
                }
            }
            let key = |f: &(usize, usize, usize)| (f.0, f.2, f.1);
            if let Some(f) = found
                && best
                    .as_ref()
                    .is_none_or(|b| key(&f) < key(&(b.0, b.1, b.2)))
            {
                best = Some((f.0, f.1, f.2, ang, rot, r.bbox));
            }
        }
        // Não coube nas chapas abertas: abre uma nova.
        if best.is_none() {
            let mut s = Sheet::new(gw, gh);
            for &ang in &angles(o) {
                let rot = Affine::rotate_about(ang.to_radians(), center);
                let rp: Vec<Vec<Point>> = ps
                    .iter()
                    .map(|p| p.iter().map(|q| rot * *q).collect())
                    .collect();
                if let Some(r) = rasterize(&rp, cell, grow)
                    && let Some((cx, cy)) = s.find(&r)
                    && best.as_ref().is_none_or(|b| (cy, cx) < (b.2, b.1))
                {
                    best = Some((sheets.len(), cx, cy, ang, rot, r.bbox));
                }
            }
            if best.is_some() {
                sheets.push(std::mem::replace(&mut s, Sheet::new(0, 0)));
            }
        }
        let Some((si, cx, cy, _ang, rot, bbox)) = best else {
            res.unplaced.push(id.clone());
            continue;
        };
        // Marca a peça sem a folga (a folga só vale na procura, para que a
        // distância entre duas peças seja uma folga, não duas).
        let rp: Vec<Vec<Point>> = ps
            .iter()
            .map(|p| p.iter().map(|q| rot * *q).collect())
            .collect();
        if let Some(r) = rasterize(&rp, cell, 0) {
            sheets[si].mark(&r, cx + grow, cy + grow);
        }
        let sheet_x = o.origin.x + si as f64 * (o.sheet_w + 10_000.0);
        // O canto da raster alargada fica `grow` células antes da caixa.
        let target = Point::new(
            sheet_x + o.margin + (cx + grow) as f64 * cell,
            o.origin.y + o.margin + (cy + grow) as f64 * cell,
        );
        let m = Affine::translate(target - bbox.origin()) * rot;
        moves.push((id.clone(), m));
        placed_area += area;
        res.placed += 1;
    }
    res.sheets = sheets.len();
    if res.sheets > 0 {
        res.usage = placed_area / (res.sheets as f64 * o.sheet_w * o.sheet_h);
    }
    if moves.is_empty() {
        return res;
    }
    ed.begin("Aninhar na chapa");
    for (id, m) in &moves {
        super::ops::transform(ed, std::slice::from_ref(id), *m, "Aninhar");
    }
    if o.outline {
        for si in 0..res.sheets {
            let mut r = Object::new(ObjectKind::Rectangle(RectData {
                width: o.sheet_w,
                height: o.sheet_h,
                corners: None,
                extra: Default::default(),
            }));
            r.transform =
                Transform::translate(o.origin.x + si as f64 * (o.sheet_w + 10_000.0), o.origin.y);
            r.stroke = Some(Stroke::new(0.0, Color::rgb(0.0, 0.6, 0.9)));
            r.printable = Some(false);
            r.locked = true;
            r.name = Some(format!("Chapa {}", si + 1));
            let layer = ed.find(&moves[0].0).map_or(0, |r| r.loc.layer);
            ed.insert_object(
                super::history::ObjLoc {
                    page,
                    layer,
                    index: 0,
                },
                r,
            );
        }
    }
    ed.commit();
    res
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Document, Page};

    fn rect(w: f64, h: f64, x: f64, y: f64) -> Object {
        let mut o = Object::new(ObjectKind::Rectangle(RectData {
            width: w,
            height: h,
            corners: None,
            extra: Default::default(),
        }));
        o.transform = Transform::translate(x, y);
        o
    }

    fn setup(objs: Vec<Object>) -> (Editor, Vec<String>) {
        let mut d = Document::new();
        d.data.pages.push(Page::new(2_000_000.0, 2_000_000.0));
        let mut ed = Editor::new(d);
        let ids = objs
            .into_iter()
            .map(|o| super::super::ops::add_object(&mut ed, 0, 0, o, "o"))
            .collect();
        (ed, ids)
    }

    fn page_bounds(ed: &Editor, id: &str) -> Rect {
        let r = ed.find(id).unwrap();
        bounds(ed.get(&r).unwrap(), Affine::IDENTITY, false).unwrap()
    }

    /// GD-PRE-30: quatro peças de 40 × 20 cm numa chapa de 100 × 50 cm (sem
    /// margem, folga 1 cm): cabem quatro, sem sobrepor e com a folga.
    #[test]
    fn gd_pre_30_fits_without_overlap() {
        let (mut ed, ids) = setup(
            (0..4)
                .map(|i| rect(400_000.0, 200_000.0, i as f64 * 500_000.0, 1_000_000.0))
                .collect(),
        );
        let o = NestOptions {
            sheet_w: 1_000_000.0,
            sheet_h: 500_000.0,
            gap: 10_000.0,
            margin: 0.0,
            rotation: "none".into(),
            outline: false,
            ..NestOptions::default()
        };
        let undo = ed.undo_len();
        let r = nest(&mut ed, 0, &ids, &o);
        assert_eq!((r.placed, r.sheets), (4, 1), "{r:?}");
        assert_eq!(ed.undo_len(), undo + 1, "um passo");
        let bs: Vec<Rect> = ids.iter().map(|id| page_bounds(&ed, id)).collect();
        for b in &bs {
            assert!(
                b.x0 >= -1.0 && b.y0 >= -1.0 && b.x1 <= 1_000_001.0 && b.y1 <= 500_001.0,
                "{b:?}"
            );
        }
        for i in 0..4 {
            for j in i + 1..4 {
                let k = bs[i]
                    .inflate(4_999.0, 4_999.0)
                    .intersect(bs[j].inflate(4_999.0, 4_999.0));
                assert!(
                    k.width() <= 0.0 || k.height() <= 0.0,
                    "folga entre {i} e {j}"
                );
            }
        }
        assert!(r.usage > 0.6);
    }

    /// Peça comprida só cabe girada; com a fibra, não gira e abre falha;
    /// maior que a chapa fica onde está.
    #[test]
    fn rotation_grain_and_too_big() {
        let tall = rect(100_000.0, 800_000.0, 0.0, 1_000_000.0);
        let (mut ed, ids) = setup(vec![tall]);
        let mut o = NestOptions {
            sheet_w: 1_000_000.0,
            sheet_h: 500_000.0,
            gap: 5_000.0,
            margin: 5_000.0,
            outline: false,
            ..NestOptions::default()
        };
        let r = nest(&mut ed, 0, &ids, &o);
        assert_eq!(r.placed, 1);
        let b = page_bounds(&ed, &ids[0]);
        assert!(b.width() > b.height(), "girou 90°: {b:?}");
        let (mut ed, ids) = setup(vec![rect(100_000.0, 800_000.0, 0.0, 1_000_000.0)]);
        o.grain = true;
        let r = nest(&mut ed, 0, &ids, &o);
        assert_eq!((r.placed, r.unplaced.len()), (0, 1));
        assert_eq!(angles(&o), vec![0.0, 180.0]);
        o.grain = false;
        o.rotation = "free".into();
        o.step = 45.0;
        assert_eq!(angles(&o).len(), 8);
    }

    /// Mais peças que uma chapa: abre a segunda ao lado, com contorno.
    #[test]
    fn second_sheet() {
        let (mut ed, ids) = setup(
            (0..3)
                .map(|i| rect(450_000.0, 450_000.0, i as f64 * 600_000.0, 1_000_000.0))
                .collect(),
        );
        let o = NestOptions {
            sheet_w: 1_000_000.0,
            sheet_h: 500_000.0,
            gap: 10_000.0,
            margin: 10_000.0,
            ..NestOptions::default()
        };
        let r = nest(&mut ed, 0, &ids, &o);
        assert_eq!((r.placed, r.sheets), (3, 2), "{r:?}");
        let names: Vec<_> = ed.doc.data.pages[0].layers[0]
            .objects
            .iter()
            .filter_map(|o| o.name.clone())
            .collect();
        assert!(names.contains(&"Chapa 2".to_string()));
        assert!(page_bounds(&ed, &ids[2]).x0 > 1_000_000.0);
    }
}
