//! Formatos de página e redimensionamento (doc 22, GD-DIM-*): biblioteca de
//! predefinições, encaixe em formatos conhecidos, aproveitamento por folha,
//! redimensionar a página mantendo ou escalando o conteúdo e ajustar à arte.

use kurbo::{Affine, Rect};

use super::history::Editor;
use super::query::bounds;
use crate::model::Transform;
use crate::units::{UM_PER_INCH, UM_PER_MM};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    Print,
    Screen,
    Social,
}

/// Predefinição de formato. Medidas em µm, na orientação "natural".
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Preset {
    pub name: &'static str,
    pub category: Category,
    pub width: f64,
    pub height: f64,
    /// Resolução recomendada (ppp).
    pub dpi: f64,
    /// `cmyk` para impressão, `rgb` para tela.
    pub color: &'static str,
    /// Entra no encaixe da ferramenta Prancheta (GD-DIM-24).
    pub snaps: bool,
    /// Formato medido em pixels (a interface mostra px).
    pub pixels: bool,
}

const MM: f64 = UM_PER_MM;
const PX: f64 = UM_PER_INCH / 96.0;

const fn print(name: &'static str, w: f64, h: f64, snaps: bool) -> Preset {
    Preset {
        name,
        category: Category::Print,
        width: w * MM,
        height: h * MM,
        dpi: 300.0,
        color: "cmyk",
        snaps,
        pixels: false,
    }
}

const fn screen(name: &'static str, category: Category, w: f64, h: f64) -> Preset {
    Preset {
        name,
        category,
        width: w * PX,
        height: h * PX,
        dpi: 96.0,
        color: "rgb",
        snaps: false,
        pixels: true,
    }
}

pub const PRESETS: &[Preset] = &[
    print("A0", 841.0, 1189.0, true),
    print("A1", 594.0, 841.0, true),
    print("A2", 420.0, 594.0, true),
    print("A3", 297.0, 420.0, true),
    print("A4", 210.0, 297.0, true),
    print("A5", 148.0, 210.0, true),
    print("A6", 105.0, 148.0, true),
    print("B4", 250.0, 353.0, true),
    print("B5", 176.0, 250.0, true),
    print("Carta", 215.9, 279.4, true),
    print("Ofício", 215.9, 355.6, true),
    print("Tabloide", 279.4, 431.8, false),
    print("Cartão de visita", 90.0, 50.0, false),
    print("Cartão 85 × 55", 85.0, 55.0, false),
    print("Envelope DL", 220.0, 110.0, false),
    print("Banner 1 × 2 m", 1000.0, 2000.0, false),
    screen("Full HD", Category::Screen, 1920.0, 1080.0),
    screen("HD", Category::Screen, 1280.0, 720.0),
    screen("Notebook", Category::Screen, 1366.0, 768.0),
    screen("Desktop", Category::Screen, 1440.0, 900.0),
    screen("Celular", Category::Screen, 390.0, 844.0),
    screen("Post quadrado", Category::Social, 1080.0, 1080.0),
    screen("Post retrato", Category::Social, 1080.0, 1350.0),
    screen("Story / vertical", Category::Social, 1080.0, 1920.0),
    screen("Capa de vídeo", Category::Social, 1280.0, 720.0),
];

/// Encontra a predefinição cujo nome contém o texto (sem diferenciar
/// maiúsculas) ou cujas medidas em mm/px contêm o número digitado.
pub fn search(query: &str) -> Vec<&'static Preset> {
    let q = query.trim().to_lowercase();
    PRESETS
        .iter()
        .filter(|p| {
            if q.is_empty() {
                return true;
            }
            let unit = if p.pixels { PX } else { MM };
            let dims = format!(
                "{} {} {} {}x{}",
                p.name.to_lowercase(),
                fmt(p.width / unit),
                fmt(p.height / unit),
                fmt(p.width / unit),
                fmt(p.height / unit)
            );
            q.split_whitespace().all(|w| dims.contains(w))
        })
        .collect()
}

fn fmt(v: f64) -> String {
    let s = format!("{v:.1}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// Quantas peças `w×h` cabem numa folha `sw×sh`, com todas na mesma
/// orientação, escolhendo a melhor entre normal e girada (GD-DIM-13).
pub fn fit_count(sw: f64, sh: f64, w: f64, h: f64) -> usize {
    if w <= 0.0 || h <= 0.0 {
        return 0;
    }
    let eps = 1e-6;
    let n = |a: f64, b: f64| ((a + eps) / b).floor().max(0.0) as usize;
    let straight = n(sw, w) * n(sh, h);
    let rotated = n(sw, h) * n(sh, w);
    straight.max(rotated)
}

/// Formato conhecido mais próximo de `w×h` (em qualquer orientação), se as
/// duas medidas estiverem a menos de `tol` (µm). Devolve o preset e as
/// medidas já na orientação pedida (GD-DIM-24).
pub fn snap_format(w: f64, h: f64, tol: f64) -> Option<(&'static Preset, f64, f64)> {
    PRESETS
        .iter()
        .filter(|p| p.snaps)
        .flat_map(|p| [(p, p.width, p.height), (p, p.height, p.width)])
        .map(|(p, pw, ph)| (p, pw, ph, (pw - w).abs().max((ph - h).abs())))
        .filter(|(.., d)| *d <= tol)
        .min_by(|a, b| a.3.total_cmp(&b.3))
        .map(|(p, pw, ph, _)| (p, pw, ph))
}

/// O que fazer com os objetos ao redimensionar a página (GD-DIM-25).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResizeMode {
    /// Objetos ficam no mesmo lugar em relação ao papel.
    KeepPosition,
    /// Escala uniforme e centralização para caber na nova página.
    ScaleContent,
}

/// Redimensiona as páginas do documento para `new_rect`, dado em
/// coordenadas da página `index` (ex.: `(-10, 0, w, h)` cresce 10 µm à
/// esquerda). O documento tem um tamanho só (GD-PAG-01), então todas as
/// páginas mudam e os objetos de cada uma recebem o mesmo ajuste. Um passo
/// no histórico.
pub fn resize(ed: &mut Editor, index: usize, new_rect: Rect, mode: ResizeMode, name: &str) {
    let (w, h) = {
        let p = &ed.doc.data.pages[index];
        (p.width, p.height)
    };
    let (nw, nh) = (new_rect.width().abs(), new_rect.height().abs());
    if nw < 1.0 || nh < 1.0 {
        return;
    }
    let r = new_rect.abs();
    let m = match mode {
        ResizeMode::KeepPosition => Affine::translate((-r.x0, -r.y0)),
        ResizeMode::ScaleContent => {
            let s = (nw / w).min(nh / h);
            Affine::translate((nw / 2.0, nh / 2.0))
                * Affine::scale(s)
                * Affine::translate((-w / 2.0, -h / 2.0))
        }
    };
    ed.transaction(name, |ed| {
        for page in 0..ed.doc.data.pages.len() {
            ed.edit_page(page, |p| {
                p.width = nw;
                p.height = nh;
            });
            if m == Affine::IDENTITY {
                continue;
            }
            for l in 0..ed.doc.data.pages[page].layers.len() {
                for i in 0..ed.objects(page, l).len() {
                    let loc = super::ObjLoc {
                        page,
                        layer: l,
                        index: i,
                    };
                    let mut o = ed.object(loc).unwrap().clone();
                    o.transform = Transform::from_affine(m * o.transform.affine());
                    ed.replace_object(loc, o);
                }
            }
        }
    });
}

/// Caixa visual de todos os objetos visíveis da página.
pub fn art_bounds(ed: &Editor, index: usize) -> Option<Rect> {
    ed.doc.data.pages[index]
        .layers
        .iter()
        .filter(|l| l.visible)
        .flat_map(|l| l.objects.iter())
        .filter(|o| o.visible)
        .filter_map(|o| bounds(o, Affine::IDENTITY, true))
        .reduce(|a, b| a.union(b))
}

/// Ajusta a página ao tamanho da arte (GD-DIM-22). `false` se não há arte.
pub fn fit_to_art(ed: &mut Editor, index: usize) -> bool {
    match art_bounds(ed, index) {
        Some(b) if b.width() > 0.0 && b.height() > 0.0 => {
            resize(
                ed,
                index,
                b,
                ResizeMode::KeepPosition,
                "Ajustar página à arte",
            );
            true
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Color, Document, Fill, Object, ObjectKind, Page, RectData};
    use serde_json::Map;

    fn rect(x: f64, y: f64, w: f64, h: f64) -> Object {
        let mut o = Object::new(ObjectKind::Rectangle(RectData {
            width: w,
            height: h,
            corners: None,
            extra: Map::new(),
        }));
        o.transform = Transform::translate(x, y);
        o.fill = Some(Fill::solid(Color::black()));
        o
    }

    fn editor(w: f64, h: f64) -> Editor {
        let mut d = Document::new();
        d.data.pages.push(Page::new(w, h));
        Editor::new(d)
    }

    /// GD-PAG-01: redimensionar vale para todas as páginas e seus objetos.
    #[test]
    fn resize_applies_to_every_page() {
        let mut ed = editor(100.0, 100.0);
        ed.doc.data.pages.push(Page::new(100.0, 100.0));
        for page in 0..2 {
            ed.insert_object(
                super::super::ObjLoc {
                    page,
                    layer: 0,
                    index: 0,
                },
                rect(10.0, 10.0, 5.0, 5.0),
            );
        }
        resize(
            &mut ed,
            1,
            Rect::new(-20.0, 0.0, 100.0, 100.0),
            ResizeMode::KeepPosition,
            "Redimensionar",
        );
        for p in &ed.doc.data.pages {
            assert_eq!(p.width, 120.0);
            let b = bounds(&p.layers[0].objects[0], Affine::IDENTITY, false).unwrap();
            assert_eq!(b.x0, 30.0);
        }
    }

    fn obj_bounds(ed: &Editor) -> Rect {
        bounds(
            &ed.doc.data.pages[0].layers[0].objects[0],
            Affine::IDENTITY,
            false,
        )
        .unwrap()
    }

    #[test]
    fn gd_dim_11_search_by_name_and_size() {
        assert!(search("A5").iter().any(|p| p.name == "A5"));
        assert!(search("1080").iter().all(|p| p.pixels));
        assert!(search("1080").len() >= 3);
        assert!(
            search("cartão")
                .iter()
                .any(|p| p.name == "Cartão de visita")
        );
        assert_eq!(search("").len(), PRESETS.len());
    }

    /// Aceite: A5 cabe 2× em A4; A6 cabe 4× em A4 e 8× em A3.
    #[test]
    fn gd_dim_13_fit_count() {
        let a = |n: &str| PRESETS.iter().find(|p| p.name == n).unwrap();
        let (a3, a4, a5, a6) = (a("A3"), a("A4"), a("A5"), a("A6"));
        assert_eq!(fit_count(a4.width, a4.height, a5.width, a5.height), 2);
        assert_eq!(fit_count(a4.width, a4.height, a6.width, a6.height), 4);
        assert_eq!(fit_count(a3.width, a3.height, a6.width, a6.height), 8);
        assert_eq!(fit_count(a4.width, a4.height, a3.width, a3.height), 0);
    }

    /// Aceite: perto de 148 × 210 mm encaixa em A5, nas duas orientações.
    #[test]
    fn gd_dim_24_snap_to_iso() {
        let (p, w, h) = snap_format(149_500.0, 209_000.0, 3_000.0).unwrap();
        assert_eq!((p.name, w, h), ("A5", 148_000.0, 210_000.0));
        let (p, w, h) = snap_format(211_000.0, 147_000.0, 3_000.0).unwrap();
        assert_eq!((p.name, w, h), ("A5", 210_000.0, 148_000.0));
        assert!(snap_format(160_000.0, 210_000.0, 3_000.0).is_none());
    }

    /// Aceite: manter posição com o canto inferior esquerdo fixo e com
    /// crescimento à esquerda; desfazer é um passo só.
    #[test]
    fn gd_dim_25_resize_keep_position() {
        let mut ed = editor(100.0, 100.0);
        ed.insert_object(
            super::super::ObjLoc {
                page: 0,
                layer: 0,
                index: 0,
            },
            rect(10.0, 10.0, 20.0, 20.0),
        );
        let before = ed.history().len();
        resize(
            &mut ed,
            0,
            Rect::new(-50.0, 0.0, 100.0, 80.0),
            ResizeMode::KeepPosition,
            "Redimensionar página",
        );
        let p = &ed.doc.data.pages[0];
        assert_eq!((p.width, p.height), (150.0, 80.0));
        assert_eq!(obj_bounds(&ed).origin(), kurbo::Point::new(60.0, 10.0));
        assert_eq!(ed.history().len(), before + 1);
        ed.undo();
        assert_eq!(ed.doc.data.pages[0].width, 100.0);
        assert_eq!(obj_bounds(&ed).origin(), kurbo::Point::new(10.0, 10.0));
    }

    #[test]
    fn gd_dim_25_resize_scale_content() {
        let mut ed = editor(100.0, 100.0);
        ed.insert_object(
            super::super::ObjLoc {
                page: 0,
                layer: 0,
                index: 0,
            },
            rect(0.0, 0.0, 100.0, 100.0),
        );
        resize(
            &mut ed,
            0,
            Rect::new(0.0, 0.0, 200.0, 50.0),
            ResizeMode::ScaleContent,
            "Redimensionar página",
        );
        let b = obj_bounds(&ed);
        assert!(
            (b.width() - 50.0).abs() < 1e-9,
            "escala uniforme pelo menor lado"
        );
        assert!(
            (b.center() - kurbo::Point::new(100.0, 25.0)).hypot() < 1e-9,
            "centralizado"
        );
    }

    /// Aceite: retângulo 50×30 mm → página 50×30 mm com o retângulo nas bordas.
    #[test]
    fn gd_dim_22_fit_to_art() {
        let mut ed = editor(210_000.0, 297_000.0);
        ed.insert_object(
            super::super::ObjLoc {
                page: 0,
                layer: 0,
                index: 0,
            },
            rect(40_000.0, 60_000.0, 50_000.0, 30_000.0),
        );
        assert!(fit_to_art(&mut ed, 0));
        let p = &ed.doc.data.pages[0];
        assert_eq!((p.width, p.height), (50_000.0, 30_000.0));
        assert_eq!(obj_bounds(&ed), Rect::new(0.0, 0.0, 50_000.0, 30_000.0));
        let mut empty = editor(100.0, 100.0);
        assert!(!fit_to_art(&mut empty, 0));
    }
}
