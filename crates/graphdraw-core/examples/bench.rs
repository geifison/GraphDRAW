//! Benchmark dos requisitos não funcionais (doc 01):
//! - GD-NF-01: abrir documento de ~50 MB / 100 mil objetos em < 5 s.
//! - GD-NF-02: redesenho ≥ 30 fps (≤ 33 ms) com 10 mil objetos visíveis.
//!
//! ```text
//! cargo run --release -p graphdraw-core --example bench            # mede
//! cargo run --release -p graphdraw-core --example bench -- --check # falha se acima do limite
//! ```
//!
//! Sempre em `--release`: o modo debug não representa o produto.

use std::time::{Duration, Instant};

use graphdraw_core::gdr::{self, WriteOptions};
use graphdraw_core::model::*;
use graphdraw_core::render::raster::{self, RasterOptions};
use graphdraw_core::render::{ImageCache, SceneOptions, build_scene};
use serde_json::Map;

const MM: f64 = 1000.0;

/// Gerador pseudoaleatório determinístico (xorshift).
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
}

fn object(rng: &mut Rng, i: usize, w: f64, h: f64) -> Object {
    let x = rng.next() * w;
    let y = rng.next() * h;
    let s = 2.0 * MM + rng.next() * 8.0 * MM;
    let color = Color::cmyk(rng.next(), rng.next(), rng.next(), 0.0);
    let mut o = match i % 4 {
        0 => Object::new(ObjectKind::Rectangle(RectData {
            width: s,
            height: s * 0.6,
            corners: None,
            extra: Map::new(),
        })),
        1 => Object::new(ObjectKind::Ellipse(EllipseData {
            rx: s / 2.0,
            ry: s / 3.0,
            mode: "ellipse".into(),
            start_angle: 0.0,
            end_angle: 360.0,
            extra: Map::new(),
        })),
        _ => {
            let mut b = SubpathBuilder::default();
            b.move_to([0.0, 0.0]);
            b.curve_to([s * 0.3, s], [s * 0.7, s], [s, 0.0]);
            b.line_to([s * 0.5, -s * 0.4]);
            b.close();
            Object::new(ObjectKind::Path(PathData {
                subpaths: b.finish(),
                fill_rule: FillRule::Evenodd,
                extra: Map::new(),
            }))
        }
    };
    o.transform = Transform::translate(x, y);
    o.fill = Some(Fill::solid(color));
    if i.is_multiple_of(3) {
        o.stroke = Some(Stroke::new(0.3 * MM, Color::black()));
    }
    o
}

/// Documento com `n` objetos espalhados por `pages` páginas A3.
pub fn synthetic(n: usize, pages: usize) -> Document {
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    let mut doc = Document::new();
    let (w, h) = (297.0 * MM, 420.0 * MM);
    for p in 0..pages {
        let mut page = Page::new(w, h);
        let per = n / pages + usize::from(p < n % pages);
        page.layers[0].objects = (0..per).map(|i| object(&mut rng, i, w, h)).collect();
        doc.data.pages.push(page);
    }
    doc
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

fn main() {
    let check = std::env::args().any(|a| a == "--check");
    let mut failed = false;

    // ---- GD-NF-01 ------------------------------------------------------
    let doc = synthetic(100_000, 10);
    let bytes = gdr::write_bytes(
        &doc,
        &WriteOptions {
            thumbnail: false,
            ..Default::default()
        },
    )
    .unwrap();
    let json_len = {
        let mut z = zip::ZipArchive::new(std::io::Cursor::new(&bytes)).unwrap();
        (0..z.len())
            .map(|i| z.by_index(i).unwrap().size())
            .sum::<u64>()
    };
    let t = Instant::now();
    let (opened, _) = graphdraw_core::open_bytes(&bytes, None).unwrap();
    let open = t.elapsed();
    let mut count = 0usize;
    opened.walk_objects(&mut |_| count += 1);
    let t = Instant::now();
    let cache = ImageCache::default();
    let scene = build_scene(
        &opened,
        &opened.data.pages[0],
        SceneOptions { print: false },
        &cache,
    );
    let first_scene = t.elapsed();
    println!(
        "GD-NF-01  abrir {count} objetos ({:.1} MB descompactados, {:.1} MB no disco): {:.0} ms (+ cena da 1ª página: {:.0} ms)  limite 5000 ms",
        json_len as f64 / 1e6,
        bytes.len() as f64 / 1e6,
        ms(open),
        ms(first_scene)
    );
    if open + first_scene > Duration::from_secs(5) {
        failed = true;
    }
    drop(scene);

    // ---- GD-NF-02 ------------------------------------------------------
    let doc = synthetic(10_000, 1);
    let page = &doc.data.pages[0];
    let scene = build_scene(&doc, page, SceneOptions { print: false }, &cache);
    let (w, h) = (1920u32, 1080u32);
    let s = (w as f64 / page.width).min(h as f64 / page.height);
    let view = kurbo::Affine::new([s, 0.0, 0.0, -s, 0.0, page.height * s]);
    let opts = RasterOptions::default();
    let _ = raster::render(&scene, view, w, h, &opts); // aquecimento
    let frames = 10;
    let t = Instant::now();
    for _ in 0..frames {
        let pm = raster::render(&scene, view, w, h, &opts).unwrap();
        std::hint::black_box(pm);
    }
    let per_frame = t.elapsed() / frames;
    println!(
        "GD-NF-02  redesenho de 10000 objetos em {w}×{h}: {:.1} ms/quadro ({:.0} fps)  limite 33,3 ms",
        ms(per_frame),
        1.0 / per_frame.as_secs_f64()
    );
    if per_frame > Duration::from_micros(33_333) {
        failed = true;
    }

    // ---- GD-NF-02 com recursos novos (conselho 2026-10-17) ---------------
    // Os mesmos 10 mil objetos, um em cada 10 com gradiente calculado
    // (elíptico/cônico/retangular), um em cada 15 com mesclagem e um em cada
    // 20 com ponta de seta; mais um fundo de página inteira com gradiente
    // cônico (o pior caso do cálculo pixel a pixel).
    let mut doc = synthetic(10_000, 1);
    {
        let page = &mut doc.data.pages[0];
        let (pw, ph) = (page.width, page.height);
        let kinds = ["elliptic", "conic", "rectangular"];
        let stop = |o: f64, c: Color| GradientStop {
            offset: o,
            color: c,
            opacity: 1.0,
            midpoint: 0.5,
            extra: Map::new(),
        };
        let grad = |kind: &str, c: [f64; 2], r: f64| Gradient {
            kind: kind.into(),
            stops: vec![
                stop(0.0, Color::black()),
                stop(1.0, Color::rgb(1.0, 0.8, 0.0)),
            ],
            start: None,
            end: None,
            center: Some(c),
            radius: Some(r),
            radius_y: Some(r * 0.6),
            angle: Some(30.0),
            spread: "pad".into(),
            steps: None,
            smooth: false,
            color_path: "direct".into(),
            extra: Map::new(),
        };
        for (i, o) in page.layers[0].objects.iter_mut().enumerate() {
            if i % 10 == 0 {
                o.fill = Some(Fill::Gradient(grad(
                    kinds[i / 10 % 3],
                    [0.0, 0.0],
                    5.0 * MM,
                )));
            }
            if i % 15 == 0 {
                o.blend = BlendMode::Multiply;
            }
            if i % 20 == 0 {
                let mut st = Stroke::new(0.3 * MM, Color::black());
                st.end_arrow = Some(Arrow::new("triangle"));
                o.stroke = Some(st);
            }
        }
        let mut bg = Object::new(ObjectKind::Rectangle(RectData {
            width: pw,
            height: ph,
            corners: None,
            extra: Map::new(),
        }));
        bg.fill = Some(Fill::Gradient(grad(
            "conic",
            [pw / 2.0, ph / 2.0],
            pw / 2.0,
        )));
        page.layers[0].objects.insert(0, bg);
    }
    let page = &doc.data.pages[0];
    let scene = build_scene(&doc, page, SceneOptions { print: false }, &cache);
    let _ = raster::render(&scene, view, w, h, &opts);
    let t = Instant::now();
    for _ in 0..frames {
        let pm = raster::render(&scene, view, w, h, &opts).unwrap();
        std::hint::black_box(pm);
    }
    let per_frame = t.elapsed() / frames;
    println!(
        "GD-NF-02+ redesenho com gradientes calculados, mesclagem e setas em {w}×{h}: {:.1} ms/quadro ({:.0} fps)  limite 33,3 ms",
        ms(per_frame),
        1.0 / per_frame.as_secs_f64()
    );
    if per_frame > Duration::from_micros(33_333) {
        failed = true;
    }

    if check && failed {
        eprintln!("ERRO: requisito de desempenho não atendido");
        std::process::exit(1);
    }
}
