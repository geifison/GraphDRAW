//! GD-FIL-03: gradientes elíptico, cônico e retangular na tela/PNG, no PDF
//! (sombreamento tipo 1 com função PostScript, conferido pelo Poppler) e no
//! SVG.

use graphdraw_core::export::{self, ExportOptions};
use graphdraw_core::fidelity;
use graphdraw_core::model::{
    Color, Document, Fill, Gradient, GradientStop, Object, ObjectKind, Page, RectData,
};
use serde_json::Map;

const MM: f64 = 1000.0;

fn doc(kind: &str, ry: f64) -> Document {
    let mut d = Document::new();
    let mut p = Page::new(40.0 * MM, 40.0 * MM);
    let mut o = Object::new(ObjectKind::Rectangle(RectData {
        width: 40.0 * MM,
        height: 40.0 * MM,
        corners: None,
        extra: Map::new(),
    }));
    let stop = |offset: f64, c: Color| GradientStop {
        offset,
        color: c,
        opacity: 1.0,
        midpoint: 0.5,
        extra: Map::new(),
    };
    o.fill = Some(Fill::Gradient(Gradient {
        kind: kind.into(),
        stops: vec![
            stop(0.0, Color::black()),
            stop(1.0, Color::rgb(1.0, 1.0, 1.0)),
        ],
        start: None,
        end: None,
        center: Some([20.0 * MM, 20.0 * MM]),
        radius: Some(20.0 * MM),
        radius_y: Some(ry * MM),
        angle: None,
        spread: "pad".into(),
        steps: None,
        smooth: false,
        color_path: "direct".into(),
        extra: Map::new(),
    }));
    p.layers[0].objects.push(o);
    d.data.pages.push(p);
    d
}

/// Pontos da página (mm) e o cinza esperado: (x, y, valor 0–255).
type Points = Vec<(f64, f64, f64)>;

/// Tipo, raio em y (mm) e pontos.
fn cases() -> Vec<(&'static str, f64, Points)> {
    vec![
        (
            "conic",
            20.0,
            vec![
                (35.0, 20.2, 0.0),
                (20.0, 35.0, 64.0),
                (20.0, 5.0, 191.0),
                (5.0, 20.5, 128.0),
            ],
        ),
        (
            "rectangular",
            10.0,
            vec![
                (30.0, 20.0, 128.0),
                (20.0, 25.0, 128.0),
                (39.0, 39.0, 255.0),
            ],
        ),
        (
            "elliptic",
            10.0,
            vec![(30.0, 20.0, 128.0), (20.0, 25.0, 128.0), (20.0, 20.0, 0.0)],
        ),
    ]
}

fn gray(img: &image::RgbaImage, x_mm: f64, y_mm: f64, px_per_mm: f64) -> f64 {
    let px = (x_mm * px_per_mm) as u32;
    let py = ((40.0 - y_mm) * px_per_mm) as u32;
    img.get_pixel(px, py).0[0] as f64
}

#[test]
fn gd_fil_03_elliptic_conic_rectangular() {
    let k = 10.0;
    for (kind, ry, pts) in cases() {
        let d = doc(kind, ry);
        let png = export::png_page(
            &d,
            0,
            &ExportOptions {
                dpi: 254.0,
                ..Default::default()
            },
        )
        .unwrap();
        let img = image::load_from_memory(&png).unwrap().to_rgba8();
        for (x, y, want) in &pts {
            let got = gray(&img, *x, *y, k);
            assert!(
                (got - want).abs() <= 8.0,
                "{kind} PNG em ({x},{y}): {got} × {want}"
            );
        }
        let svg = export::svg_page(&d, 0, &ExportOptions::default()).unwrap();
        assert!(
            svg.contains("<pattern") || svg.contains("<radialGradient"),
            "{kind}"
        );
        let pdf = export::pdf_document(&d, &ExportOptions::default()).unwrap();
        let p = lopdf::Document::load_mem(&pdf).unwrap();
        let has_type1 = p.objects.values().any(|o| {
            o.as_dict()
                .ok()
                .and_then(|d| d.get(b"ShadingType").ok())
                .and_then(|t| t.as_i64().ok())
                == Some(1)
        });
        assert!(has_type1, "{kind}: sombreamento tipo 1");
        // A função PostScript tem que dar as mesmas cores no Poppler.
        if fidelity::poppler_tool("pdftoppm").is_some() {
            let tmp = tempfile::tempdir().unwrap();
            let f = tmp.path().join("g.pdf");
            std::fs::write(&f, &pdf).unwrap();
            let pages = fidelity::rasterize(&f, 254, &tmp.path().join("r")).unwrap();
            let r = &pages[0];
            let img = image::RgbaImage::from_fn(r.width, r.height, |x, y| {
                let i = ((y * r.width + x) * 3) as usize;
                image::Rgba([r.rgb[i], r.rgb[i + 1], r.rgb[i + 2], 255])
            });
            for (x, y, want) in &pts {
                let got = gray(&img, *x, *y, k);
                assert!(
                    (got - want).abs() <= 10.0,
                    "{kind} PDF em ({x},{y}): {got} × {want}"
                );
            }
        }
    }
}

/// Revisão de 2026-10-17: parada dura no início e o centro do cônico não
/// geram erro na função PostScript (o Poppler não reclama).
#[test]
fn postscript_function_edge_cases() {
    let Some(tool) = fidelity::poppler_tool("pdftoppm") else {
        return;
    };
    for kind in ["conic", "elliptic"] {
        let mut d = doc(kind, 20.0);
        if let Some(Fill::Gradient(g)) = &mut d.data.pages[0].layers[0].objects[0].fill {
            g.stops.insert(
                1,
                GradientStop {
                    offset: 0.0,
                    color: Color::rgb(1.0, 0.0, 0.0),
                    opacity: 1.0,
                    midpoint: 0.5,
                    extra: Map::new(),
                },
            );
        }
        let pdf = export::pdf_document(&d, &ExportOptions::default()).unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let f = tmp.path().join("g.pdf");
        std::fs::write(&f, &pdf).unwrap();
        // 1 ppp por mm com centro exato num pixel: 25,4 ppp, página 40 mm.
        let out = std::process::Command::new(&tool)
            .args(["-r", "254", "-png"])
            .arg(&f)
            .arg(tmp.path().join("p"))
            .output()
            .unwrap();
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(
            out.status.success() && !err.to_lowercase().contains("error"),
            "{kind}: {err}"
        );
    }
}
