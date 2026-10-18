//! Formas geradas (doc 03): espiral (GD-PRI-05) e papel gráfico
//! (GD-PRI-06), no quadro local `(0, 0)–(w, h)`.

use kurbo::{Point, Vec2};
use serde_json::Map;

use super::convert::bez_to_subpaths;
use crate::model::{GroupData, Object, ObjectKind, PathData, RectData, Transform};

/// Espiral aberta que começa no centro do quadro e termina na borda depois
/// de `turns` voltas. Simétrica (raio cresce igual a cada volta) ou
/// logarítmica (a cada volta o raio multiplica por `expansion` > 1).
pub fn spiral(w: f64, h: f64, turns: f64, logarithmic: bool, expansion: f64) -> PathData {
    let pts = spiral_points(w, h, turns, logarithmic, expansion);
    let n = pts.len() - 1;
    let tol = (w.max(h) * 2e-4).max(1e-3);
    let dir = |a: Point, b: Point| -> Vec2 {
        let d = b - a;
        if d.hypot() > 0.0 {
            d.normalize()
        } else {
            Vec2::new(1.0, 0.0)
        }
    };
    let t1 = dir(pts[0], pts[1]);
    let t2 = dir(pts[n], pts[n - 1]);
    let path = super::fit::fit_open(&pts, t1, t2, tol);
    PathData {
        subpaths: bez_to_subpaths(&path),
        fill_rule: Default::default(),
        extra: Map::new(),
    }
}

/// Pontos da espiral (96 por volta), do centro à borda.
pub fn spiral_points(w: f64, h: f64, turns: f64, logarithmic: bool, expansion: f64) -> Vec<Point> {
    let turns = turns.clamp(0.25, 100.0);
    let total = turns * std::f64::consts::TAU;
    let k = expansion.max(1.0001);
    // Raio relativo 0..1 em função do ângulo.
    let rel = |a: f64| -> f64 {
        if logarithmic {
            // r = k^(a / 2π), normalizado para começar em 0 e acabar em 1.
            let r = k.powf(a / std::f64::consts::TAU);
            let (r0, r1) = (1.0, k.powf(turns));
            (r - r0) / (r1 - r0)
        } else {
            a / total
        }
    };
    let (cx, cy, rx, ry) = (w / 2.0, h / 2.0, w / 2.0, h / 2.0);
    let n = ((turns * 96.0).ceil() as usize).clamp(24, 9600);
    (0..=n)
        .map(|i| {
            let a = total * i as f64 / n as f64;
            let r = rel(a);
            Point::new(cx + rx * r * a.cos(), cy + ry * r * a.sin())
        })
        .collect()
}

/// Caminho da espiral viva: a curva aberta ou, com a fita, o contorno
/// fechado de largura variando do centro à borda.
pub fn spiral_path(s: &crate::model::SpiralData) -> kurbo::BezPath {
    let Some([w0, w1]) = s.ribbon else {
        return crate::model::geom::subpaths_to_bez(
            &spiral(s.width, s.height, s.turns, s.logarithmic, s.expansion).subpaths,
        );
    };
    let pts = spiral_points(s.width, s.height, s.turns, s.logarithmic, s.expansion);
    let n = pts.len();
    let normal = |i: usize| {
        let (a, b) = (pts[i.saturating_sub(1)], pts[(i + 1).min(n - 1)]);
        let d = b - a;
        let l = d.hypot().max(1e-9);
        Vec2::new(-d.y / l, d.x / l)
    };
    let half = |i: usize| (w0 + (w1 - w0) * i as f64 / (n - 1).max(1) as f64).max(0.0) / 2.0;
    let mut p = kurbo::BezPath::new();
    for (i, &pt) in pts.iter().enumerate() {
        let q = pt + normal(i) * half(i);
        if i == 0 {
            p.move_to(q);
        } else {
            p.line_to(q);
        }
    }
    for i in (0..n).rev() {
        p.line_to(pts[i] - normal(i) * half(i));
    }
    p.close_path();
    p
}

/// Papel gráfico: grupo de `cols × rows` retângulos que dividem o quadro,
/// com a calha `gap` (µm, horizontal e vertical) entre as células — zero =
/// células coladas. `like` dá preenchimento e contorno às células.
pub fn graph_paper(w: f64, h: f64, cols: u32, rows: u32, gap: [f64; 2], like: &Object) -> Object {
    let (cols, rows) = (cols.clamp(1, 200), rows.clamp(1, 200));
    let gx = gap[0].max(0.0).min(w / (cols.max(2) - 1) as f64 * 0.9);
    let gy = gap[1].max(0.0).min(h / (rows.max(2) - 1) as f64 * 0.9);
    let cw = (w - gx * (cols - 1) as f64) / cols as f64;
    let ch = (h - gy * (rows - 1) as f64) / rows as f64;
    let mut children = Vec::with_capacity((cols * rows) as usize);
    for r in 0..rows {
        for c in 0..cols {
            let mut o = Object::new(ObjectKind::Rectangle(RectData {
                width: cw,
                height: ch,
                corners: None,
                extra: Map::new(),
            }));
            o.transform = Transform::translate(c as f64 * (cw + gx), r as f64 * (ch + gy));
            o.fill = like.fill.clone();
            o.stroke = like.stroke.clone();
            children.push(o);
        }
    }
    let mut g = Object::new(ObjectKind::Group(GroupData {
        children,
        boolean: None,
        symmetry: None,
        effect: None,
        symbol: None,
        extra: Map::new(),
    }));
    g.name = Some(format!("Papel gráfico {cols} × {rows}"));
    g
}

#[cfg(test)]
mod tests {
    use super::*;
    use kurbo::{ParamCurve, Shape};

    fn bez(p: &PathData) -> kurbo::BezPath {
        crate::model::geom::subpaths_to_bez(&p.subpaths)
    }

    /// GD-PRI-05: caminho aberto do centro à borda do quadro; na simétrica
    /// as voltas são igualmente espaçadas.
    #[test]
    fn gd_pri_05_spiral() {
        for log in [false, true] {
            let p = spiral(100.0, 60.0, 3.0, log, 2.0);
            assert_eq!(p.subpaths.len(), 1);
            assert!(!p.subpaths[0].closed);
            let first = p.subpaths[0].nodes[0].p;
            assert!((first[0] - 50.0).abs() < 1e-6 && (first[1] - 30.0).abs() < 1e-6);
            let last = p.subpaths[0].nodes.last().unwrap().p;
            assert!(
                (last[0] - 100.0).abs() < 0.05 && (last[1] - 30.0).abs() < 0.05,
                "{last:?}"
            );
            let b = bez(&p).bounding_box();
            assert!(
                b.x0 >= -0.1 && b.x1 <= 100.1 && b.y0 >= -0.1 && b.y1 <= 60.1,
                "{b:?}"
            );
            assert!(
                p.subpaths[0].nodes.len() < 60,
                "poucas curvas: {}",
                p.subpaths[0].nodes.len()
            );
        }
        // Simétrica: cruzamentos do eixo +x a cada volta em 1/3, 2/3 e 1 do raio.
        let p = spiral(120.0, 120.0, 3.0, false, 2.0);
        let path = bez(&p);
        let line = kurbo::Line::new((60.0, 60.0), (121.0, 60.0));
        let mut xs: Vec<f64> = path
            .segments()
            .flat_map(|s| {
                s.intersect_line(line)
                    .into_iter()
                    .map(move |i| s.eval(i.segment_t).x)
            })
            .filter(|x| *x > 61.0)
            .collect();
        xs.sort_by(f64::total_cmp);
        xs.dedup_by(|a, b| (*a - *b).abs() < 0.5);
        assert_eq!(xs.len(), 3, "{xs:?}");
        for (x, want) in xs.iter().zip([80.0, 100.0, 120.0]) {
            assert!((x - want).abs() < 0.2, "{xs:?}");
        }
    }

    /// Espiral viva com fita: contorno fechado mais largo na borda.
    #[test]
    fn live_spiral_ribbon() {
        use kurbo::Shape;
        let s = crate::model::SpiralData {
            width: 100.0,
            height: 100.0,
            turns: 3.0,
            logarithmic: false,
            expansion: 2.0,
            ribbon: Some([1.0, 8.0]),
            extra: Map::new(),
        };
        let p = spiral_path(&s);
        assert!(matches!(
            p.elements().last(),
            Some(kurbo::PathEl::ClosePath)
        ));
        let b = p.bounding_box();
        assert!(b.width() > 95.0 && b.width() < 110.0, "{b:?}");
        let o = Object::new(ObjectKind::Spiral(s));
        assert_eq!(Object::from_value(o.to_value()), o);
        // Calha: as células se afastam.
        let g = graph_paper(
            100.0,
            50.0,
            2,
            1,
            [10.0, 0.0],
            &Object::new(ObjectKind::Group(GroupData {
                children: vec![],
                boolean: None,
                symmetry: None,
                effect: None,
                symbol: None,
                extra: Map::new(),
            })),
        );
        let ObjectKind::Group(gd) = &g.kind else {
            panic!()
        };
        let ObjectKind::Rectangle(r) = &gd.children[0].kind else {
            panic!()
        };
        assert!((r.width - 45.0).abs() < 1e-9);
        assert!((gd.children[1].transform.affine().translation().x - 55.0).abs() < 1e-9);
    }

    /// GD-PRI-06: grupo de colunas × linhas que divide o quadro.
    #[test]
    fn gd_pri_06_graph_paper() {
        let g = graph_paper(
            90.0,
            40.0,
            3,
            2,
            [0.0, 0.0],
            &Object::new(ObjectKind::Group(GroupData {
                children: vec![],
                boolean: None,
                symmetry: None,
                effect: None,
                symbol: None,
                extra: Map::new(),
            })),
        );
        let ObjectKind::Group(d) = &g.kind else {
            panic!()
        };
        assert_eq!(d.children.len(), 6);
        let last = &d.children[5];
        let ObjectKind::Rectangle(r) = &last.kind else {
            panic!()
        };
        assert_eq!((r.width, r.height), (30.0, 20.0));
        assert_eq!(last.transform, Transform::translate(60.0, 20.0));
    }
}
