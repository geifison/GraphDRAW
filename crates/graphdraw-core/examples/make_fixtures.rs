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
            boolean: None,
            symmetry: None,
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
                midpoint: 0.5,
                extra: Map::new(),
            },
            GradientStop {
                offset: 1.0,
                color: b,
                opacity: 1.0,
                midpoint: 0.5,
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
                    midpoint: 0.5,
                    extra: Map::new(),
                },
            );
            s
        },
        start: Some([0.0, 0.0]),
        end: Some([80.0 * MM, 0.0]),
        center: None,
        radius: None,
        radius_y: None,
        angle: None,
        spread: "pad".into(),
        steps: None,
        smooth: false,
        color_path: "direct".into(),
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
        radius_y: None,
        angle: None,
        spread: "pad".into(),
        steps: None,
        smooth: false,
        color_path: "direct".into(),
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
        hyphenate: false,
        lang: None,
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
            path: None,
            exclusions: Default::default(),
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
            path: None,
            exclusions: Default::default(),
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
            path: None,
            exclusions: Default::default(),
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

/// GD-OUT-04, GD-SEL-02, GD-CAM-01: contorno centrado/interno/externo,
/// grupo dentro de grupo (com matrizes próprias) e camada em modo esboço.
fn m1_editing() -> Document {
    let mut ids = Ids::new(10);
    let mut p = page(&mut ids, 100.0 * MM, 100.0 * MM);
    p.layers[0].name = "Arte".into();
    for (k, pos) in ["center", "inside", "outside"].into_iter().enumerate() {
        let mut r = rect(
            &mut ids,
            (10.0 + 30.0 * k as f64) * MM,
            65.0 * MM,
            20.0 * MM,
            20.0 * MM,
            Fill::solid(Color::rgb(1.0, 0.9, 0.2)),
        );
        let mut s = Stroke::new(3.0 * MM, Color::rgb(0.8, 0.1, 0.1));
        s.position = pos.into();
        r.stroke = Some(s);
        r.name = Some(format!("Contorno {pos}"));
        p.layers[0].objects.push(r);
    }
    let mut circle = obj(
        &mut ids,
        ObjectKind::Ellipse(EllipseData {
            rx: 6.0 * MM,
            ry: 6.0 * MM,
            mode: "ellipse".into(),
            start_angle: 0.0,
            end_angle: 360.0,
            extra: Map::new(),
        }),
    );
    circle.name = Some("Círculo no grupo interno".into());
    circle.transform = Transform::translate(6.0 * MM, 6.0 * MM);
    circle.fill = Some(Fill::solid(Color::rgb(0.1, 0.4, 0.9)));
    let square = rect(
        &mut ids,
        14.0 * MM,
        0.0,
        12.0 * MM,
        12.0 * MM,
        Fill::solid(Color::rgb(0.1, 0.7, 0.3)),
    );
    let mut inner = obj(
        &mut ids,
        ObjectKind::Group(GroupData {
            children: vec![circle, square],
            boolean: None,
            symmetry: None,
            extra: Map::new(),
        }),
    );
    inner.name = Some("Grupo interno".into());
    inner.transform = Transform::translate(0.0, 15.0 * MM);
    let base = rect(
        &mut ids,
        0.0,
        0.0,
        26.0 * MM,
        10.0 * MM,
        Fill::solid(Color::rgb(0.5, 0.5, 0.5)),
    );
    let mut outer = obj(
        &mut ids,
        ObjectKind::Group(GroupData {
            children: vec![base, inner],
            boolean: None,
            symmetry: None,
            extra: Map::new(),
        }),
    );
    outer.name = Some("Grupo externo".into());
    outer.transform = Transform::translate(10.0 * MM, 10.0 * MM);
    p.layers[0].objects.push(outer);
    let mut sketch = layer(&mut ids, "Esboço");
    sketch.wireframe = true;
    let mut r = rect(
        &mut ids,
        55.0 * MM,
        10.0 * MM,
        35.0 * MM,
        35.0 * MM,
        Fill::solid(Color::rgb(0.6, 0.2, 0.8)),
    );
    r.stroke = Some(Stroke::new(2.0 * MM, Color::black()));
    sketch.objects.push(r);
    p.layers.push(sketch);
    doc_with(vec![p], "Edição do M1")
}

/// Reescreve um membro de um `.gdr` (para simular corrupção, GD-GDR-10).
/// GD-FIL-04..06/10: os três tipos de padrão (vetorial com um círculo,
/// imagem, duas cores com deslocamento e espelhamento, um preso à página).
fn patterns() -> Document {
    let mut ids = Ids::new(12);
    let mut p = page(&mut ids, 200.0 * MM, 80.0 * MM);
    let mut d = doc_with(vec![], "Padrões");
    let src = d.add_image(checker_png(), "png");
    let mut dot = obj(
        &mut ids,
        ObjectKind::Ellipse(EllipseData {
            rx: 2.0 * MM,
            ry: 2.0 * MM,
            mode: "ellipse".into(),
            start_angle: 0.0,
            end_angle: 360.0,
            extra: Map::new(),
        }),
    );
    dot.transform = Transform::translate(3.0 * MM, 3.0 * MM);
    dot.fill = Some(Fill::solid(Color::cmyk(1.0, 0.0, 0.0, 0.0)));
    let vector = Pattern {
        kind: "vector".into(),
        tile: vec![dot],
        width: 6.0 * MM,
        height: 6.0 * MM,
        ..Pattern::default()
    };
    let bitmap = Pattern {
        kind: "bitmap".into(),
        src: Some(src),
        width: 10.0 * MM,
        height: 10.0 * MM,
        rotation: 15.0,
        ..Pattern::default()
    };
    let two = Pattern {
        kind: "two-color".into(),
        cells: vec!["1100".into(), "1100".into(), "0011".into(), "0011".into()],
        fg: Some(Color::cmyk(0.0, 0.0, 0.0, 1.0)),
        bg: Some(Color::cmyk(0.0, 0.0, 0.2, 0.0)),
        width: 8.0 * MM,
        height: 8.0 * MM,
        row_offset: 0.5,
        mirror: true,
        transform_with_object: false,
        ..Pattern::default()
    };
    for (i, pat) in [vector, bitmap, two].into_iter().enumerate() {
        let x = 5.0 + 65.0 * i as f64;
        p.layers[0].objects.push(rect(
            &mut ids,
            x * MM,
            10.0 * MM,
            60.0 * MM,
            60.0 * MM,
            Fill::Pattern(Box::new(pat)),
        ));
    }
    d.data.pages.push(p);
    d
}

/// GD-PRI-03..07, GD-FER-32: polígono, estrela, estrela complexa, espiral,
/// papel gráfico e as 13 formas básicas.
fn shapes() -> Document {
    let mut ids = Ids::new(20);
    let mut p = page(&mut ids, 297.0 * MM, 210.0 * MM);
    let mut d = doc_with(vec![], "Formas paramétricas");
    let fill = |c: f64| Fill::solid(Color::cmyk(c, 0.3, 0.0, 0.0));
    for (i, (mode, sides, sharp)) in [("polygon", 6, 0.5), ("star", 5, 0.5), ("complex", 7, 0.6)]
        .iter()
        .enumerate()
    {
        let mut o = obj(
            &mut ids,
            ObjectKind::Polygon(PolygonData {
                rx: 20.0 * MM,
                ry: 20.0 * MM,
                sides: *sides,
                mode: (*mode).into(),
                sharpness: *sharp,
                extra: Map::new(),
            }),
        );
        o.transform = Transform::translate((30.0 + 50.0 * i as f64) * MM, 180.0 * MM);
        o.fill = Some(fill(0.2 * i as f64 + 0.3));
        o.stroke = Some(Stroke::new(0.5 * MM, Color::cmyk(0.0, 0.0, 0.0, 1.0)));
        p.layers[0].objects.push(o);
    }
    let mut sp = obj(
        &mut ids,
        ObjectKind::Path(graphdraw_core::edit::generate::spiral(
            40.0 * MM,
            40.0 * MM,
            4.0,
            false,
            1.5,
        )),
    );
    sp.transform = Transform::translate(175.0 * MM, 160.0 * MM);
    sp.stroke = Some(Stroke::new(0.5 * MM, Color::cmyk(0.0, 0.0, 0.0, 1.0)));
    p.layers[0].objects.push(sp);
    let like = {
        let mut o = obj(
            &mut ids,
            ObjectKind::Rectangle(RectData {
                width: 1.0,
                height: 1.0,
                corners: None,
                extra: Map::new(),
            }),
        );
        o.stroke = Some(Stroke::new(0.3 * MM, Color::cmyk(0.0, 0.0, 0.0, 1.0)));
        o
    };
    let mut gp = graphdraw_core::edit::generate::graph_paper(50.0 * MM, 40.0 * MM, 5, 4, &like);
    gp.transform = Transform::translate(230.0 * MM, 160.0 * MM);
    p.layers[0].objects.push(gp);
    for (i, shape) in graphdraw_core::render::basic::SHAPES.iter().enumerate() {
        let (col, row) = (i % 7, i / 7);
        let mut o = obj(
            &mut ids,
            ObjectKind::Basic(BasicShapeData {
                shape: (*shape).into(),
                width: 32.0 * MM,
                height: 26.0 * MM,
                params: graphdraw_core::render::basic::default_params(shape),
                extra: Map::new(),
            }),
        );
        o.transform = Transform::translate(
            (10.0 + 40.0 * col as f64) * MM,
            (95.0 - 50.0 * row as f64) * MM,
        );
        o.fill = Some(fill(0.6));
        o.stroke = Some(Stroke::new(0.4 * MM, Color::cmyk(0.0, 0.0, 0.0, 1.0)));
        o.name = Some((*shape).into());
        p.layers[0].objects.push(o);
    }
    d.data.pages.push(p);
    d
}

/// GD-OUT-01..06: as seis pontas de seta, caneta caligráfica, posição do
/// contorno, cantos, pontas da linha e atrás do preenchimento.
fn strokes() -> Document {
    let mut ids = Ids::new(21);
    let mut p = page(&mut ids, 297.0 * MM, 210.0 * MM);
    let mut d = doc_with(vec![], "Contornos");
    let black = || Color::cmyk(0.0, 0.0, 0.0, 1.0);
    let line = |ids: &mut Ids, a: (f64, f64), b: (f64, f64)| {
        path(
            ids,
            vec![Subpath {
                nodes: vec![node(a.0, a.1), node(b.0, b.1)],
                closed: false,
                extra: Map::new(),
            }],
        )
    };
    for (i, shape) in graphdraw_core::render::arrows::SHAPES.iter().enumerate() {
        let y = 195.0 - 12.0 * i as f64;
        let mut o = line(&mut ids, (15.0, y), (95.0, y));
        let mut st = Stroke::new(0.8 * MM, black());
        st.start_arrow = Some(Arrow::new(shape));
        st.end_arrow = Some(Arrow::new(shape));
        o.stroke = Some(st);
        o.name = Some(format!("Seta {shape}"));
        p.layers[0].objects.push(o);
    }
    for (i, (stretch, angle)) in [(1.0, 0.0), (0.3, 45.0), (0.15, 90.0)].iter().enumerate() {
        let y = 195.0 - 25.0 * i as f64;
        let mut o = path(
            &mut ids,
            vec![Subpath {
                nodes: vec![
                    node(115.0, y),
                    smooth(150.0, y - 15.0, [140.0, y - 15.0], [160.0, y - 15.0]),
                    node(185.0, y),
                ],
                closed: false,
                extra: Map::new(),
            }],
        );
        let mut st = Stroke::new(4.0 * MM, Color::cmyk(1.0, 0.5, 0.0, 0.0));
        st.nib_stretch = *stretch;
        st.nib_angle = *angle;
        st.cap = "round".into();
        o.stroke = Some(st);
        o.name = Some(format!("Bico {}%", (stretch * 100.0) as i32));
        p.layers[0].objects.push(o);
    }
    for (i, pos) in ["inside", "center", "outside"].iter().enumerate() {
        let mut r = rect(
            &mut ids,
            (205.0 + 30.0 * i as f64) * MM,
            160.0 * MM,
            22.0 * MM,
            22.0 * MM,
            Fill::solid(Color::cmyk(0.0, 0.0, 0.6, 0.0)),
        );
        let mut st = Stroke::new(3.0 * MM, black());
        st.position = (*pos).into();
        r.stroke = Some(st);
        r.name = Some(format!("Posição {pos}"));
        p.layers[0].objects.push(r);
    }
    for (i, (join, cap)) in [("miter", "butt"), ("round", "round"), ("bevel", "square")]
        .iter()
        .enumerate()
    {
        let x = 20.0 + 60.0 * i as f64;
        let mut o = path(
            &mut ids,
            vec![Subpath {
                nodes: vec![node(x, 40.0), node(x + 20.0, 80.0), node(x + 40.0, 40.0)],
                closed: false,
                extra: Map::new(),
            }],
        );
        let mut st = Stroke::new(6.0 * MM, Color::cmyk(0.0, 0.8, 0.8, 0.0));
        st.join = (*join).into();
        st.cap = (*cap).into();
        o.stroke = Some(st);
        o.name = Some(format!("Canto {join}, ponta {cap}"));
        p.layers[0].objects.push(o);
    }
    for (i, behind) in [false, true].iter().enumerate() {
        let mut r = rect(
            &mut ids,
            (205.0 + 40.0 * i as f64) * MM,
            40.0 * MM,
            30.0 * MM,
            30.0 * MM,
            Fill::solid(Color::cmyk(0.0, 0.0, 1.0, 0.0)),
        );
        let mut st = Stroke::new(5.0 * MM, black());
        st.behind_fill = *behind;
        r.stroke = Some(st);
        r.name = Some(if *behind {
            "Atrás do preenchimento".into()
        } else {
            "Na frente".into()
        });
        p.layers[0].objects.push(r);
    }
    d.data.pages.push(p);
    d
}

/// GD-TRA-02, GD-OBJ-05, GD-GUI-03, GD-TXT-09, GD-FIL-03 (opções) e
/// textura: os 16 modos de mesclagem, dados do objeto, camada de guias,
/// texto em caminho, gradiente com ponto médio, faixas e rota de cor e o
/// padrão de textura.
fn appearance() -> Document {
    let mut ids = Ids::new(22);
    let mut p = page(&mut ids, 297.0 * MM, 210.0 * MM);
    let mut d = doc_with(vec![], "Aparência");
    let back = rect(
        &mut ids,
        10.0 * MM,
        120.0 * MM,
        170.0 * MM,
        80.0 * MM,
        Fill::solid(Color::cmyk(0.0, 0.6, 1.0, 0.0)),
    );
    p.layers[0].objects.push(back);
    for (i, b) in BlendMode::ALL.iter().enumerate() {
        let (col, row) = (i % 8, i / 8);
        let mut r = rect(
            &mut ids,
            (15.0 + 20.0 * col as f64) * MM,
            (165.0 - 35.0 * row as f64) * MM,
            16.0 * MM,
            30.0 * MM,
            Fill::solid(Color::cmyk(0.8, 0.0, 0.3, 0.0)),
        );
        r.blend = *b;
        p.layers[0].objects.push(r);
    }
    let mut tagged = rect(
        &mut ids,
        200.0 * MM,
        170.0 * MM,
        30.0 * MM,
        20.0 * MM,
        Fill::solid(Color::cmyk(0.0, 0.0, 0.0, 0.3)),
    );
    tagged.user_data = Some(
        [
            ("cliente".to_string(), "Exemplo".to_string()),
            ("pedido".to_string(), "123".to_string()),
        ]
        .into_iter()
        .collect(),
    );
    tagged.name = Some("Com dados do objeto".into());
    p.layers[0].objects.push(tagged);
    let mut stops = vec![
        GradientStop {
            offset: 0.0,
            color: Color::rgb(1.0, 0.0, 0.0),
            opacity: 1.0,
            midpoint: 0.25,
            extra: Map::new(),
        },
        GradientStop {
            offset: 1.0,
            color: Color::rgb(0.0, 0.0, 1.0),
            opacity: 1.0,
            midpoint: 0.5,
            extra: Map::new(),
        },
    ];
    for (i, (steps, path)) in [(None, "direct"), (Some(8), "direct"), (None, "cw")]
        .iter()
        .enumerate()
    {
        let spec = graphdraw_core::edit::ops::GradientSpec {
            kind: "linear".into(),
            stops: stops.clone(),
            steps: *steps,
            color_path: (*path).into(),
            ..Default::default()
        };
        let g = graphdraw_core::edit::ops::gradient_for(
            &spec,
            kurbo::Rect::new(0.0, 0.0, 80.0 * MM, 20.0 * MM),
        );
        let mut r = rect(
            &mut ids,
            200.0 * MM,
            (130.0 - 25.0 * i as f64) * MM,
            80.0 * MM,
            20.0 * MM,
            Fill::Gradient(g),
        );
        r.name = Some(format!(
            "Gradiente {path} {}",
            steps.map_or("contínuo".to_string(), |n| format!("{n} faixas"))
        ));
        p.layers[0].objects.push(r);
    }
    stops.clear();
    let tex = Pattern {
        kind: "texture".into(),
        texture: Some(Texture {
            style: "marble".into(),
            seed: 7,
            ..Texture::default()
        }),
        fg: Some(Color::cmyk(0.0, 0.0, 0.0, 0.8)),
        bg: Some(Color::cmyk(0.0, 0.0, 0.0, 0.0)),
        width: 40.0 * MM,
        height: 40.0 * MM,
        resolution: 150.0,
        ..Pattern::default()
    };
    p.layers[0].objects.push(rect(
        &mut ids,
        200.0 * MM,
        20.0 * MM,
        80.0 * MM,
        40.0 * MM,
        Fill::Pattern(Box::new(tex)),
    ));
    // Texto em caminho sobre um arco.
    let arc = vec![Subpath {
        nodes: vec![
            node(20.0, 40.0),
            smooth(90.0, 90.0, [55.0, 90.0], [125.0, 90.0]),
            node(160.0, 40.0),
        ],
        closed: false,
        extra: Map::new(),
    }];
    let t = obj(
        &mut ids,
        ObjectKind::Text(TextData {
            text_kind: "artistic".into(),
            frame: None,
            paragraphs: vec![Paragraph {
                align: "left".into(),
                spans: vec![span("Texto que segue o caminho", 18.0)],
                hyphenate: false,
                lang: None,
                extra: Map::new(),
            }],
            line_spacing: 1.2,
            path: Some(TextPath {
                subpaths: arc,
                offset: 0.0,
                distance: 1.0 * MM,
                side: "above".into(),
                orientation: "rotate".into(),
                mirror: false,
                guide: None,
                detached: true,
                extra: Map::new(),
            }),
            exclusions: Default::default(),
            extra: Map::new(),
        }),
    );
    p.layers[0].objects.push(t);
    // Camada de guias: um círculo que vira guia (não imprime).
    let mut g = layer(&mut ids, "Guias");
    g.guides = true;
    g.printable = false;
    let mut c = obj(
        &mut ids,
        ObjectKind::Ellipse(EllipseData {
            rx: 30.0 * MM,
            ry: 30.0 * MM,
            mode: "ellipse".into(),
            start_angle: 0.0,
            end_angle: 360.0,
            extra: Map::new(),
        }),
    );
    c.transform = Transform::translate(90.0 * MM, 60.0 * MM);
    g.objects.push(c);
    p.layers.push(g);
    d.data.pages.push(p);
    d
}

/// GD-OUT-03: tracejado nos cantos — padrão, alinhado e fixo.
fn dash_corners() -> Document {
    let mut ids = Ids::new(19);
    let mut p = page(&mut ids, 200.0 * MM, 80.0 * MM);
    let mut d = doc_with(vec![], "Tracejado nos cantos");
    for (i, mode) in ["standard", "align", "fixed"].iter().enumerate() {
        let x = 10.0 + 65.0 * i as f64;
        let mut r = rect(
            &mut ids,
            x * MM,
            15.0 * MM,
            50.0 * MM,
            50.0 * MM,
            Fill::None,
        );
        r.fill = None;
        let mut st = Stroke::new(1.5 * MM, Color::cmyk(0.0, 0.0, 0.0, 1.0));
        st.dash = Some(vec![7.0 * MM, 4.0 * MM]);
        st.dash_corners = (*mode).into();
        r.stroke = Some(st);
        r.name = Some(format!("Tracejado {mode}"));
        p.layers[0].objects.push(r);
    }
    d.data.pages.push(p);
    d
}

/// Booleanas não destrutivas: soldar, aparar e interseção de um quadrado
/// e um círculo, com as formas originais nos filhos.
fn live_boolean() -> Document {
    let mut ids = Ids::new(18);
    let mut p = page(&mut ids, 200.0 * MM, 80.0 * MM);
    let mut d = doc_with(vec![], "Booleanas ao vivo");
    for (i, op) in ["weld", "trim", "intersect"].iter().enumerate() {
        let x = 10.0 + 65.0 * i as f64;
        let sq = rect(
            &mut ids,
            x * MM,
            15.0 * MM,
            40.0 * MM,
            40.0 * MM,
            Fill::solid(Color::cmyk(0.0, 0.7, 1.0, 0.0)),
        );
        let mut c = obj(
            &mut ids,
            ObjectKind::Ellipse(EllipseData {
                rx: 15.0 * MM,
                ry: 15.0 * MM,
                mode: "ellipse".into(),
                start_angle: 0.0,
                end_angle: 360.0,
                extra: Map::new(),
            }),
        );
        c.transform = Transform::translate((x + 40.0) * MM, 55.0 * MM);
        c.fill = Some(Fill::solid(Color::cmyk(1.0, 0.0, 0.0, 0.0)));
        let mut g = obj(
            &mut ids,
            ObjectKind::Group(GroupData {
                children: vec![sq, c],
                boolean: Some((*op).into()),
                symmetry: None,
                extra: Map::new(),
            }),
        );
        g.name = Some(format!("Booleana {op}"));
        p.layers[0].objects.push(g);
    }
    d.data.pages.push(p);
    d
}

/// GD-TXT-10: parágrafo contornando um círculo (contorno, dos dois lados)
/// e um quadrado à direita (quadrado, texto à esquerda).
fn text_wrap() -> Document {
    let mut ids = Ids::new(17);
    let mut p = page(&mut ids, 150.0 * MM, 120.0 * MM);
    let mut d = doc_with(vec![], "Texto contornando");
    let words = "Texto de parágrafo que contorna os objetos com afastamento. ".repeat(14);
    let mut body = obj(
        &mut ids,
        ObjectKind::Text(TextData {
            text_kind: "paragraph".into(),
            frame: Some([130.0 * MM, 105.0 * MM]),
            paragraphs: vec![Paragraph {
                align: "justify".into(),
                spans: vec![span(&words, 10.0)],
                hyphenate: false,
                lang: None,
                extra: Map::new(),
            }],
            line_spacing: 1.2,
            path: None,
            exclusions: Default::default(),
            extra: Map::new(),
        }),
    );
    body.transform = Transform::translate(10.0 * MM, 112.0 * MM);
    p.layers[0].objects.push(body);
    let mut circle = obj(
        &mut ids,
        ObjectKind::Ellipse(EllipseData {
            rx: 20.0 * MM,
            ry: 20.0 * MM,
            mode: "ellipse".into(),
            start_angle: 0.0,
            end_angle: 360.0,
            extra: Map::new(),
        }),
    );
    circle.transform = Transform::translate(60.0 * MM, 70.0 * MM);
    circle.fill = Some(Fill::solid(Color::cmyk(0.0, 0.5, 1.0, 0.0)));
    circle.wrap = Some(TextWrap {
        style: "contour".into(),
        side: "both".into(),
        distance: 3.0 * MM,
    });
    p.layers[0].objects.push(circle);
    let mut sq = rect(
        &mut ids,
        105.0 * MM,
        15.0 * MM,
        30.0 * MM,
        30.0 * MM,
        Fill::solid(Color::cmyk(0.8, 0.0, 0.3, 0.0)),
    );
    sq.wrap = Some(TextWrap {
        style: "square".into(),
        side: "left".into(),
        distance: 2.0 * MM,
    });
    p.layers[0].objects.push(sq);
    d.data.pages.push(p);
    d
}

/// GD-VAR-01: os quatro perfis prontos e um anel fechado com bojo.
fn variable_width() -> Document {
    let mut ids = Ids::new(16);
    let mut p = page(&mut ids, 200.0 * MM, 120.0 * MM);
    let mut d = doc_with(vec![], "Largura variável");
    let profiles = graphdraw_core::render::varwidth::PRESETS;
    for (i, name) in profiles.iter().enumerate() {
        let y = (100.0 - 20.0 * i as f64) * MM;
        let mut o = obj(
            &mut ids,
            ObjectKind::Path(PathData {
                subpaths: vec![Subpath {
                    nodes: vec![Node::corner(10.0 * MM, y), Node::corner(110.0 * MM, y)],
                    closed: false,
                    extra: Map::new(),
                }],
                fill_rule: FillRule::Nonzero,
                extra: Map::new(),
            }),
        );
        let mut st = Stroke::new(6.0 * MM, Color::cmyk(0.0, 0.0, 0.0, 1.0));
        st.widths = graphdraw_core::render::varwidth::preset(name);
        o.stroke = Some(st);
        o.name = Some((*name).into());
        p.layers[0].objects.push(o);
    }
    let mut ring = rect(
        &mut ids,
        130.0 * MM,
        30.0 * MM,
        60.0 * MM,
        60.0 * MM,
        Fill::None,
    );
    ring.fill = None;
    let mut st = Stroke::new(4.0 * MM, Color::cmyk(1.0, 0.0, 0.0, 0.0));
    st.widths = graphdraw_core::render::varwidth::preset("bulge");
    ring.stroke = Some(st);
    p.layers[0].objects.push(ring);
    d.data.pages.push(p);
    d
}

/// GD-FER-33: espelho de um eixo e radial de seis cópias.
fn symmetry() -> Document {
    let mut ids = Ids::new(15);
    let mut p = page(&mut ids, 200.0 * MM, 100.0 * MM);
    let mut d = doc_with(vec![], "Simetria");
    for (kind, count, cx, child) in [
        ("mirror", 1, 50.0, (20.0, 30.0, 25.0, 40.0)),
        ("radial", 6, 150.0, (150.0, 60.0, 8.0, 30.0)),
    ] {
        let (x, y, w, h) = child;
        let r = rect(
            &mut ids,
            x * MM,
            y * MM,
            w * MM,
            h * MM,
            Fill::solid(Color::cmyk(0.0, 0.8, 0.4, 0.0)),
        );
        let mut g = obj(
            &mut ids,
            ObjectKind::Group(GroupData {
                children: vec![r],
                boolean: None,
                symmetry: Some(Symmetry {
                    kind: kind.into(),
                    count,
                    center: [cx * MM, 50.0 * MM],
                    angle: 90.0,
                }),
                extra: Map::new(),
            }),
        );
        g.name = Some(format!("Simetria {kind}"));
        p.layers[0].objects.push(g);
    }
    d.data.pages.push(p);
    d
}

/// GD-BSP-01: B-spline aberta com um ponto fixado e uma fechada só de
/// flutuantes.
fn bspline() -> Document {
    let mut ids = Ids::new(14);
    let mut p = page(&mut ids, 200.0 * MM, 100.0 * MM);
    let mut d = doc_with(vec![], "B-spline");
    let pt = |x: f64, y: f64, fixed: bool| BSplinePoint {
        p: [x * MM, y * MM],
        fixed,
    };
    for (points, closed) in [
        (
            vec![
                pt(10.0, 10.0, false),
                pt(30.0, 90.0, false),
                pt(60.0, 20.0, true),
                pt(90.0, 90.0, false),
            ],
            false,
        ),
        (
            vec![
                pt(120.0, 20.0, false),
                pt(190.0, 20.0, false),
                pt(190.0, 80.0, false),
                pt(120.0, 80.0, false),
            ],
            true,
        ),
    ] {
        let mut o = obj(
            &mut ids,
            ObjectKind::BSpline(BSplineData {
                points,
                closed,
                extra: Map::new(),
            }),
        );
        o.stroke = Some(Stroke::new(1.0 * PT, Color::cmyk(0.0, 0.0, 0.0, 1.0)));
        if closed {
            o.fill = Some(Fill::solid(Color::cmyk(0.6, 0.0, 0.0, 0.0)));
        }
        p.layers[0].objects.push(o);
    }
    d.data.pages.push(p);
    d
}

/// GD-CON-01: três caixas ligadas por conector reto, em ângulo reto
/// (cantos arredondados) e curvo, com seta no fim.
fn connectors() -> Document {
    let mut ids = Ids::new(13);
    let mut p = page(&mut ids, 200.0 * MM, 120.0 * MM);
    let d = doc_with(vec![], "Conectores");
    let boxes = [(10.0, 10.0), (130.0, 10.0), (10.0, 80.0), (130.0, 80.0)];
    let mut bids = Vec::new();
    for (x, y) in boxes {
        let r = rect(
            &mut ids,
            x * MM,
            y * MM,
            50.0 * MM,
            30.0 * MM,
            Fill::solid(Color::cmyk(0.0, 0.0, 0.2, 0.0)),
        );
        bids.push((r.id.clone(), x * MM, y * MM));
        p.layers[0].objects.push(r);
    }
    let end = |i: usize, a: [f64; 2]| {
        let (id, x, y) = &bids[i];
        ConnEnd {
            point: [x + a[0] * 50.0 * MM, y + a[1] * 30.0 * MM],
            object: Some(id.clone()),
            anchor: Some(a),
        }
    };
    for (style, from, to, radius) in [
        ("straight", end(0, [1.0, 0.5]), end(1, [0.0, 0.5]), 0.0),
        ("elbow", end(0, [0.5, 1.0]), end(3, [0.5, 0.0]), 3.0 * MM),
        ("curved", end(2, [1.0, 0.5]), end(1, [0.5, 1.0]), 0.0),
    ] {
        let mut c = obj(
            &mut ids,
            ObjectKind::Connector(ConnectorData {
                style: style.into(),
                from,
                to,
                radius,
                extra: Map::new(),
            }),
        );
        let mut st = Stroke::new(0.75 * PT, Color::cmyk(0.0, 0.0, 0.0, 1.0));
        st.end_arrow = Some(Arrow::new("triangle"));
        c.stroke = Some(st);
        p.layers[0].objects.push(c);
    }
    let mut d = d;
    d.data.pages.push(p);
    d
}

/// Doc 28 e GD-COT-01..03: sangria, limites da verificação prévia,
/// sobreimpressão no contorno e uma cota ligada aos nós de um retângulo.
fn prepress() -> Document {
    let mut ids = Ids::new(11);
    let mut p = page(&mut ids, 210.0 * MM, 297.0 * MM);
    p.bleed = Some(3.0 * MM);
    let mut r = rect(
        &mut ids,
        20.0 * MM,
        200.0 * MM,
        100.0 * MM,
        50.0 * MM,
        Fill::solid(Color::cmyk(0.0, 0.6, 1.0, 0.0)),
    );
    let mut st = Stroke::new(0.5 * PT, Color::cmyk(0.0, 0.0, 0.0, 1.0));
    st.overprint = true;
    r.stroke = Some(st);
    let rid = r.id.clone();
    let link = |node| {
        Some(DimLink {
            object: rid.clone(),
            subpath: 0,
            node,
        })
    };
    let mut d = obj(
        &mut ids,
        ObjectKind::Dimension(DimensionData {
            dim_kind: "horizontal".into(),
            points: vec![[20.0 * MM, 200.0 * MM], [120.0 * MM, 200.0 * MM]],
            offset: -10.0 * MM,
            links: vec![link(0), link(1)],
            segment: None,
            format: DimFormat::default(),
            extra: Map::new(),
        }),
    );
    d.stroke = Some(Stroke::new(0.25 * MM, Color::cmyk(0.0, 0.0, 0.0, 1.0)));
    p.layers[0].objects = vec![r, d];
    let mut doc = doc_with(vec![p], "Pré-impressão");
    doc.data.settings.preflight.tac = graphdraw_core::preflight::TAC_UNCOATED;
    doc
}

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
        ("m1-editing", m1_editing()),
        ("prepress", prepress()),
        ("patterns", patterns()),
        ("connectors", connectors()),
        ("bspline", bspline()),
        ("symmetry", symmetry()),
        ("variable-width", variable_width()),
        ("text-wrap", text_wrap()),
        ("live-boolean", live_boolean()),
        ("dash-corners", dash_corners()),
        ("shapes", shapes()),
        ("strokes", strokes()),
        ("appearance", appearance()),
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
