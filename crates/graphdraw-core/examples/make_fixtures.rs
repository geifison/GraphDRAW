//! Gera os arquivos de teste `.gdr` em `tests/fixtures/gdr/`.
//!
//!     cargo run -p graphdraw-core --example make_fixtures
//!
//! Os IDs são fixos para que a saída seja reprodutível.

use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};

use graphdraw_core::gdr::{self, WriteOptions};
use graphdraw_core::model::*;
use serde_json::{Map, json};

const MM: f64 = 1000.0;
const PT: f64 = graphdraw_core::units::UM_PER_PT;

struct Ids {
    prefix: u32,
    n: u64,
}

impl Ids {
    fn new(prefix: u32) -> Ids {
        Ids { prefix, n: 0 }
    }
    fn next(&mut self) -> String {
        self.n += 1;
        format!("0192f{:03x}-0000-7000-8000-{:012x}", self.prefix, self.n)
    }
}

fn obj(ids: &mut Ids, kind: ObjectKind) -> Object {
    let mut o = Object::new(kind);
    o.id = ids.next();
    o
}

fn page(ids: &mut Ids, w: f64, h: f64) -> Page {
    let mut p = Page::new(w, h);
    p.id = ids.next();
    p.layers[0].id = ids.next();
    p
}

fn layer(ids: &mut Ids, name: &str) -> Layer {
    let mut l = Layer::new(name);
    l.id = ids.next();
    l
}

fn rect(ids: &mut Ids, x: f64, y: f64, w: f64, h: f64, fill: Fill) -> Object {
    let mut o = obj(
        ids,
        ObjectKind::Rectangle(RectData {
            width: w,
            height: h,
            corners: None,
            extra: Map::new(),
        }),
    );
    o.transform = Transform::translate(x, y);
    o.fill = Some(fill);
    o
}

fn path(ids: &mut Ids, subpaths: Vec<Subpath>) -> Object {
    obj(
        ids,
        ObjectKind::Path(PathData {
            subpaths,
            fill_rule: FillRule::Evenodd,
            extra: Map::new(),
        }),
    )
}

fn node(x: f64, y: f64) -> Node {
    Node::corner(x * MM, y * MM)
}

fn smooth(x: f64, y: f64, inn: [f64; 2], out: [f64; 2]) -> Node {
    let mut n = node(x, y);
    n.handle_in = Some([inn[0] * MM, inn[1] * MM]);
    n.handle_out = Some([out[0] * MM, out[1] * MM]);
    n.kind = NodeKind::Smooth;
    n
}

fn doc_with(pages: Vec<Page>, title: &str) -> Document {
    let mut d = Document::new();
    d.data.pages = pages;
    d.metadata.title = Some(title.into());
    d.metadata.author = Some("Equipe GraphDraw".into());
    d.metadata.created = Some("2026-10-16T00:00:00Z".into());
    d
}

fn span(text: &str, size_pt: f64) -> TextSpan {
    TextSpan {
        text: text.into(),
        font: FontSpec {
            family: "Arial".into(),
            size: size_pt * PT,
            weight: 400,
            italic: false,
            extra: Map::new(),
        },
        fill: None,
        extra: Map::new(),
    }
}

/// GD-GEO-01..04, GD-PRI-01/02, GD-OUT-01..03, GD-OBJ-02: formas básicas.
fn basic_shapes() -> Document {
    let mut ids = Ids::new(1);
    let mut p = page(&mut ids, 210.0 * MM, 297.0 * MM);
    let mut objects = Vec::new();

    let mut r = rect(
        &mut ids,
        20.0 * MM,
        230.0 * MM,
        60.0 * MM,
        40.0 * MM,
        Fill::solid(Color::rgb(0.95, 0.75, 0.1)),
    );
    r.name = Some("Retângulo arredondado".into());
    if let ObjectKind::Rectangle(d) = &mut r.kind {
        d.corners = Some(vec![
            Corner {
                kind: "round".into(),
                radius: 8.0 * MM,
            },
            Corner {
                kind: "chamfer".into(),
                radius: 8.0 * MM,
            },
            Corner {
                kind: "scallop".into(),
                radius: 8.0 * MM,
            },
            Corner {
                kind: "round".into(),
                radius: 0.0,
            },
        ]);
    }
    r.stroke = Some(Stroke::new(1.0 * MM, Color::black()));
    objects.push(r);

    let mut e = obj(
        &mut ids,
        ObjectKind::Ellipse(EllipseData {
            rx: 30.0 * MM,
            ry: 20.0 * MM,
            mode: "ellipse".into(),
            start_angle: 0.0,
            end_angle: 360.0,
            extra: Map::new(),
        }),
    );
    e.transform = Transform::translate(150.0 * MM, 250.0 * MM);
    e.fill = Some(Fill::solid(Color::rgb(0.2, 0.5, 0.9)));
    objects.push(e);

    let mut pie = obj(
        &mut ids,
        ObjectKind::Ellipse(EllipseData {
            rx: 25.0 * MM,
            ry: 25.0 * MM,
            mode: "pie".into(),
            start_angle: 30.0,
            end_angle: 300.0,
            extra: Map::new(),
        }),
    );
    pie.transform = Transform::translate(50.0 * MM, 170.0 * MM);
    pie.fill = Some(Fill::solid(Color::rgb(0.9, 0.3, 0.3)));
    objects.push(pie);

    let mut arc = obj(
        &mut ids,
        ObjectKind::Ellipse(EllipseData {
            rx: 25.0 * MM,
            ry: 15.0 * MM,
            mode: "arc".into(),
            start_angle: 0.0,
            end_angle: 180.0,
            extra: Map::new(),
        }),
    );
    arc.transform = Transform::translate(150.0 * MM, 170.0 * MM);
    let mut st = Stroke::new(2.0 * MM, Color::rgb(0.1, 0.6, 0.2));
    st.cap = "round".into();
    st.dash = Some(vec![6.0 * MM, 3.0 * MM]);
    arc.stroke = Some(st);
    objects.push(arc);

    // Caminho com furo (par-ímpar) e curvas.
    let outer = Subpath {
        closed: true,
        nodes: vec![
            node(20.0, 60.0),
            node(90.0, 60.0),
            node(90.0, 120.0),
            node(20.0, 120.0),
        ],
        extra: Map::new(),
    };
    let inner = Subpath {
        closed: true,
        nodes: vec![
            smooth(55.0, 75.0, [45.0, 75.0], [65.0, 75.0]),
            smooth(75.0, 90.0, [75.0, 80.0], [75.0, 100.0]),
            smooth(55.0, 105.0, [65.0, 105.0], [45.0, 105.0]),
            smooth(35.0, 90.0, [35.0, 100.0], [35.0, 80.0]),
        ],
        extra: Map::new(),
    };
    let mut donut = path(&mut ids, vec![outer, inner]);
    donut.name = Some("Quadro com furo".into());
    donut.fill = Some(Fill::solid(Color::rgb(0.5, 0.2, 0.6)));
    let mut st = Stroke::new(0.0, Color::black());
    st.join = "round".into();
    donut.stroke = Some(st);
    objects.push(donut);

    // Grupo girado com linha aberta.
    let line = {
        let mut l = path(
            &mut ids,
            vec![Subpath {
                closed: false,
                nodes: vec![node(0.0, 0.0), node(40.0, 0.0), node(40.0, 20.0)],
                extra: Map::new(),
            }],
        );
        let mut st = Stroke::new(1.5 * MM, Color::rgb(0.0, 0.0, 0.0));
        st.join = "bevel".into();
        st.cap = "square".into();
        l.stroke = Some(st);
        l
    };
    let small = rect(
        &mut ids,
        0.0,
        25.0 * MM,
        15.0 * MM,
        10.0 * MM,
        Fill::solid(Color::rgb(0.0, 0.7, 0.7)),
    );
    let mut g = obj(
        &mut ids,
        ObjectKind::Group(GroupData {
            children: vec![line, small],
            extra: Map::new(),
        }),
    );
    let a = std::f64::consts::FRAC_PI_6;
    g.transform = Transform([a.cos(), a.sin(), -a.sin(), a.cos(), 130.0 * MM, 70.0 * MM]);
    g.opacity = Some(0.8);
    g.user_data = Some([("cliente".to_string(), "Gráfica Exemplo".to_string())].into());
    objects.push(g);

    p.layers[0].objects = objects;
    doc_with(vec![p], "Formas básicas")
}

/// GD-COR-01/02, GD-GDR-07, GD-PAL-01: modelos de cor preservados.
fn colors() -> Document {
    let mut ids = Ids::new(2);
    let mut p = page(&mut ids, 200.0 * MM, 100.0 * MM);
    let mut spot = Color::new("spot", vec![1.0]);
    spot.name = Some("Laranja Especial 021".into());
    spot.alternate = Some(Box::new(Color::cmyk(0.0, 0.53, 1.0, 0.0)));
    let mut spot_tint = spot.clone();
    spot_tint.values = vec![0.4];
    let mut global = Color::cmyk(1.0, 0.0, 0.0, 0.0);
    global.reference = Some("cor-ciano".into());
    let swatches = [
        Color::rgb(1.0, 0.0, 0.0),
        Color::cmyk(0.0, 1.0, 1.0, 0.0),
        Color::cmyk(0.6, 0.4, 0.4, 1.0),
        Color::gray(0.5),
        Color::new("lab", vec![60.0, 40.0, -30.0]),
        spot,
        spot_tint,
        Color::new("registration", vec![1.0]),
        global,
    ];
    for (i, c) in swatches.into_iter().enumerate() {
        let x = 10.0 * MM + (i % 5) as f64 * 38.0 * MM;
        let y = if i < 5 { 55.0 * MM } else { 10.0 * MM };
        let mut r = rect(&mut ids, x, y, 30.0 * MM, 30.0 * MM, Fill::solid(c.clone()));
        r.name = Some(format!("Amostra {}", c.model));
        r.stroke = Some(Stroke::new(0.5 * MM, Color::cmyk(0.0, 0.0, 0.0, 1.0)));
        p.layers[0].objects.push(r);
    }
    let mut d = doc_with(vec![p], "Modelos de cor");
    d.colors.colors.push(NamedColor {
        id: "cor-ciano".into(),
        name: "Ciano puro".into(),
        color: Color::cmyk(1.0, 0.0, 0.0, 0.0),
        extra: Map::new(),
    });
    d
}

/// GD-FIL-03 (subconjunto linear/radial), GD-TRA-01: gradientes e transparência.
fn gradients() -> Document {
    let mut ids = Ids::new(3);
    let mut p = page(&mut ids, 200.0 * MM, 100.0 * MM);
    let stops = |a: Color, b: Color| {
        vec![
            GradientStop {
                offset: 0.0,
                color: a,
                opacity: 1.0,
                extra: Map::new(),
            },
            GradientStop {
                offset: 1.0,
                color: b,
                opacity: 1.0,
                extra: Map::new(),
            },
        ]
    };
    let lin = Gradient {
        kind: "linear".into(),
        stops: {
            let mut s = stops(Color::rgb(1.0, 0.0, 0.0), Color::rgb(0.0, 0.0, 1.0));
            s.insert(
                1,
                GradientStop {
                    offset: 0.5,
                    color: Color::rgb(1.0, 1.0, 0.0),
                    opacity: 1.0,
                    extra: Map::new(),
                },
            );
            s
        },
        start: Some([0.0, 0.0]),
        end: Some([80.0 * MM, 0.0]),
        center: None,
        radius: None,
        spread: "pad".into(),
        extra: Map::new(),
    };
    p.layers[0].objects.push(rect(
        &mut ids,
        10.0 * MM,
        10.0 * MM,
        80.0 * MM,
        80.0 * MM,
        Fill::Gradient(lin),
    ));
    let rad = Gradient {
        kind: "radial".into(),
        stops: stops(Color::rgb(1.0, 1.0, 1.0), Color::cmyk(1.0, 0.5, 0.0, 0.2)),
        start: None,
        end: None,
        center: Some([40.0 * MM, 40.0 * MM]),
        radius: Some(40.0 * MM),
        spread: "pad".into(),
        extra: Map::new(),
    };
    p.layers[0].objects.push(rect(
        &mut ids,
        110.0 * MM,
        10.0 * MM,
        80.0 * MM,
        80.0 * MM,
        Fill::Gradient(rad),
    ));
    let mut veil = rect(
        &mut ids,
        50.0 * MM,
        30.0 * MM,
        100.0 * MM,
        40.0 * MM,
        Fill::Solid {
            color: Color::rgb(0.0, 0.0, 0.0),
            opacity: 0.4,
            overprint: false,
            extra: Map::new(),
        },
    );
    veil.name = Some("Véu 40%".into());
    p.layers[0].objects.push(veil);
    doc_with(vec![p], "Gradientes")
}

/// GD-TXT-01/02 (subconjunto), GD-FNT-02: texto artístico e de parágrafo.
fn text() -> Document {
    let mut ids = Ids::new(4);
    let mut p = page(&mut ids, 200.0 * MM, 120.0 * MM);
    let para = |align: &str, spans: Vec<TextSpan>| Paragraph {
        align: align.into(),
        spans,
        extra: Map::new(),
    };
    let mut title = obj(
        &mut ids,
        ObjectKind::Text(TextData {
            text_kind: "artistic".into(),
            frame: Some([180.0 * MM, 20.0 * MM]),
            paragraphs: vec![para(
                "center",
                vec![span("GraphDraw ", 36.0), {
                    let mut s = span("M0", 36.0);
                    s.font.weight = 700;
                    s.fill = Some(Fill::solid(Color::rgb(0.8, 0.1, 0.1)));
                    s
                }],
            )],
            line_spacing: 1.2,
            extra: Map::new(),
        }),
    );
    title.transform = Transform::translate(10.0 * MM, 110.0 * MM);
    title.fill = Some(Fill::solid(Color::rgb(0.1, 0.1, 0.3)));
    p.layers[0].objects.push(title);

    let mut body = obj(
        &mut ids,
        ObjectKind::Text(TextData {
            text_kind: "paragraph".into(),
            frame: Some([90.0 * MM, 60.0 * MM]),
            paragraphs: vec![
                para(
                    "left",
                    vec![span(
                        "Texto de parágrafo com quebra automática dentro do quadro, acentuação e símbolos: ação, coração, R$ 10,00.",
                        12.0,
                    )],
                ),
                para("right", vec![span("Alinhado à direita.", 12.0)]),
            ],
            line_spacing: 1.3,
            extra: Map::new(),
        }),
    );
    body.transform = Transform::translate(10.0 * MM, 80.0 * MM);
    p.layers[0].objects.push(body);

    let mut missing = obj(
        &mut ids,
        ObjectKind::Text(TextData {
            text_kind: "artistic".into(),
            frame: None,
            paragraphs: vec![para(
                "left",
                vec![{
                    let mut s = span("Fonte ausente → substituta", 14.0);
                    s.font.family = "Fonte Que Nao Existe GD".into();
                    s
                }],
            )],
            line_spacing: 1.2,
            extra: Map::new(),
        }),
    );
    missing.transform = Transform::translate(110.0 * MM, 60.0 * MM);
    p.layers[0].objects.push(missing);
    doc_with(vec![p], "Texto")
}

fn checker_png() -> Vec<u8> {
    let (w, h) = (16u32, 16u32);
    let mut px = Vec::new();
    for y in 0..h {
        for x in 0..w {
            let on = (x / 4 + y / 4) % 2 == 0;
            let a = if x < 2 { 128 } else { 255 };
            px.extend_from_slice(if on { &[230, 60, 30] } else { &[30, 60, 230] });
            px.push(a);
        }
    }
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, w, h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header().unwrap().write_image_data(&px).unwrap();
    }
    out
}

/// Imagens incorporadas (doc 02 "Imagem", GD-GDR-02 resources/images).
fn image() -> Document {
    let mut ids = Ids::new(5);
    let mut p = page(&mut ids, 100.0 * MM, 100.0 * MM);
    let mut d = doc_with(vec![], "Imagem");
    let src = d.add_image(checker_png(), "png");
    let mut img = obj(
        &mut ids,
        ObjectKind::Image(ImageData {
            width: 60.0 * MM,
            height: 40.0 * MM,
            src: src.clone(),
            extra: Map::new(),
        }),
    );
    img.transform = Transform::translate(20.0 * MM, 50.0 * MM);
    p.layers[0].objects.push(img);
    // Mesma imagem girada e semitransparente (deduplicada por hash).
    let mut img2 = obj(
        &mut ids,
        ObjectKind::Image(ImageData {
            width: 30.0 * MM,
            height: 30.0 * MM,
            src,
            extra: Map::new(),
        }),
    );
    let a = 0.3f64;
    img2.transform = Transform([a.cos(), a.sin(), -a.sin(), a.cos(), 55.0 * MM, 5.0 * MM]);
    img2.opacity = Some(0.6);
    p.layers[0].objects.push(img2);
    d.data.pages.push(p);
    d
}

/// GD-PAG-01, GD-PAG-05, GD-CAM-02: várias páginas, tamanhos, página mestra.
fn multipage() -> Document {
    let mut ids = Ids::new(6);
    let mut master = layer(&mut ids, "Rodapé mestre");
    master.objects.push(rect(
        &mut ids,
        0.0,
        0.0,
        216.0 * MM,
        8.0 * MM,
        Fill::solid(Color::cmyk(0.0, 0.0, 0.0, 0.3)),
    ));
    let mut a4 = page(&mut ids, 210.0 * MM, 297.0 * MM);
    a4.name = Some("A4 retrato".into());
    a4.layers[0].objects.push(rect(
        &mut ids,
        20.0 * MM,
        20.0 * MM,
        170.0 * MM,
        257.0 * MM,
        Fill::solid(Color::rgb(0.9, 0.9, 1.0)),
    ));
    let mut letter = page(&mut ids, 279.4 * MM, 215.9 * MM);
    letter.name = Some("Carta paisagem".into());
    letter.background = Some(Color::rgb(1.0, 1.0, 0.85));
    letter.layers[0].objects.push(rect(
        &mut ids,
        20.0 * MM,
        20.0 * MM,
        100.0 * MM,
        100.0 * MM,
        Fill::solid(Color::rgb(0.2, 0.7, 0.3)),
    ));
    let mut custom = page(&mut ids, 90.0 * MM, 50.0 * MM);
    custom.name = Some("Cartão de visita".into());
    custom.hide_master = true;
    custom.layers[0].objects.push(rect(
        &mut ids,
        5.0 * MM,
        5.0 * MM,
        80.0 * MM,
        40.0 * MM,
        Fill::solid(Color::rgb(0.1, 0.1, 0.1)),
    ));
    let mut d = doc_with(vec![a4, letter, custom], "Várias páginas");
    d.data.master_page.layers.push(master);
    d.data.settings.bleed = 3.0 * MM;
    d
}

/// GD-CAM-01, GD-OBJ-04: camadas não imprimíveis/ocultas e objeto não imprimível.
fn layers_print() -> Document {
    let mut ids = Ids::new(7);
    let mut p = page(&mut ids, 100.0 * MM, 100.0 * MM);
    p.layers[0].name = "Arte".into();
    p.layers[0].objects.push(rect(
        &mut ids,
        10.0 * MM,
        10.0 * MM,
        30.0 * MM,
        30.0 * MM,
        Fill::solid(Color::rgb(1.0, 0.0, 0.0)),
    ));
    let mut np = rect(
        &mut ids,
        60.0 * MM,
        10.0 * MM,
        30.0 * MM,
        30.0 * MM,
        Fill::solid(Color::rgb(0.0, 1.0, 0.0)),
    );
    np.name = Some("Objeto não imprimível".into());
    np.printable = Some(false);
    p.layers[0].objects.push(np);
    let mut guides = layer(&mut ids, "Anotações (não imprime)");
    guides.printable = false;
    guides.color = Some(Color::rgb(0.0, 0.6, 1.0));
    guides.objects.push(rect(
        &mut ids,
        10.0 * MM,
        60.0 * MM,
        30.0 * MM,
        30.0 * MM,
        Fill::solid(Color::rgb(0.0, 0.0, 1.0)),
    ));
    let mut hidden = layer(&mut ids, "Oculta");
    hidden.visible = false;
    hidden.objects.push(rect(
        &mut ids,
        60.0 * MM,
        60.0 * MM,
        30.0 * MM,
        30.0 * MM,
        Fill::solid(Color::rgb(1.0, 0.0, 1.0)),
    ));
    p.layers.push(guides);
    p.layers.push(hidden);
    doc_with(vec![p], "Camadas e impressão")
}

/// GD-GDR-03: campos, tipos e membros desconhecidos preservados.
fn unknown_fields() -> Document {
    let mut ids = Ids::new(8);
    let mut p = page(&mut ids, 100.0 * MM, 100.0 * MM);
    p.extra
        .insert("futurePageSetting".into(), json!({ "fold": "z" }));
    let mut r = rect(
        &mut ids,
        10.0 * MM,
        10.0 * MM,
        40.0 * MM,
        40.0 * MM,
        Fill::solid(Color::rgb(0.3, 0.3, 0.3)),
    );
    if let ObjectKind::Rectangle(d) = &mut r.kind {
        d.extra
            .insert("futureBevel".into(), json!({ "depth": 2500 }));
    }
    p.layers[0].objects.push(r);
    let alien = json!({ "id": ids.next(), "type": "hologram", "transform": [1, 0, 0, 1, 0, 0], "beams": 3 });
    p.layers[0].objects.push(Object::from_value(alien));
    let mut d = doc_with(vec![p], "Campos desconhecidos");
    d.data
        .extra
        .insert("futureDocumentField".into(), json!([1, 2, 3]));
    d.extra_members.insert(
        "plugins/exemplo.json".into(),
        std::sync::Arc::new(br#"{"plugin":"exemplo"}"#.to_vec()),
    );
    d
}

/// Reescreve um membro de um `.gdr` (para simular corrupção, GD-GDR-10).
fn tamper(bytes: &[u8], member: &str, new: Option<&[u8]>) -> Vec<u8> {
    let mut zin = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut zout = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for i in 0..zin.len() {
        let mut f = zin.by_index(i).unwrap();
        let name = f.name().unwrap().to_string();
        let mut data = Vec::new();
        f.read_to_end(&mut data).unwrap();
        let method = if name == "mimetype" {
            zip::CompressionMethod::Stored
        } else {
            zip::CompressionMethod::Deflated
        };
        if name == member {
            match new {
                Some(n) => data = n.to_vec(),
                None => continue,
            }
        }
        zout.start_file(
            name,
            zip::write::SimpleFileOptions::default().compression_method(method),
        )
        .unwrap();
        zout.write_all(&data).unwrap();
    }
    zout.finish().unwrap().into_inner()
}

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/gdr")
}

fn main() {
    let dir = fixtures_dir();
    std::fs::create_dir_all(&dir).unwrap();
    let opts = WriteOptions::default();
    let docs: Vec<(&str, Document)> = vec![
        ("basic-shapes", basic_shapes()),
        ("colors", colors()),
        ("gradients", gradients()),
        ("text", text()),
        ("image", image()),
        ("multipage", multipage()),
        ("layers-print", layers_print()),
        ("unknown-fields", unknown_fields()),
    ];
    for (name, doc) in &docs {
        let path = dir.join(format!("{name}.gdr"));
        gdr::write_file(doc, &path, &opts).unwrap();
        println!("{}", path.display());
    }
    // GD-GDR-11: páginas em membros separados.
    let split = gdr::write_bytes(
        &multipage(),
        &WriteOptions {
            split_pages: true,
            ..Default::default()
        },
    )
    .unwrap();
    std::fs::write(dir.join("split-pages.gdr"), split).unwrap();

    // GD-GDR-10: imagem corrompida + objeto inválido + metadata ilegível.
    let mut doc = image();
    let mut bad = Object::from_value(
        json!({ "id": "0192f900-0000-7000-8000-000000000001", "type": "rectangle", "width": "largo" }),
    );
    bad.name = None;
    doc.data.pages[0].layers[0].objects.push(bad);
    let bytes = gdr::write_bytes(&doc, &opts).unwrap();
    let img_member = doc.images.keys().next().unwrap().clone();
    let bytes = tamper(&bytes, &img_member, Some(b"isto nao e um PNG"));
    let bytes = tamper(&bytes, "metadata.json", Some(b"{ quebrado"));
    std::fs::write(dir.join("corrupt-member.gdr"), bytes).unwrap();

    // Versão maior futura: deve ser recusada com mensagem clara (GD-GDR-03).
    let mut future = colors();
    future.data.format_version = "2.0".into();
    let bytes = gdr::write_bytes(
        &future,
        &WriteOptions {
            thumbnail: false,
            ..Default::default()
        },
    )
    .unwrap();
    let doc_json = {
        let mut z = zip::ZipArchive::new(Cursor::new(&bytes)).unwrap();
        let mut s = String::new();
        z.by_name("document.json")
            .unwrap()
            .read_to_string(&mut s)
            .unwrap();
        s.replace("\"formatVersion\":\"1.0\"", "\"formatVersion\":\"2.0\"")
    };
    std::fs::write(
        dir.join("future-version.gdr"),
        tamper(&bytes, "document.json", Some(doc_json.as_bytes())),
    )
    .unwrap();
    println!("ok");
}
